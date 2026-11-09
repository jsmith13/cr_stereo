; Circuit Rhythm build-5706 core-B grain engine (granular looper builds only).
;
; Core B (physical Core-1) has 4K words of internal P, and the stock image, the
; capture extension and the declick leave only P:$0F9E-$0FFF. The engine
; therefore runs from shared memory, which is one physical array mapped into P
; (1 wait state), X and Y alike: the loader puts a master copy in internal
; Y:$1800 and the hook copies it to shared $37400 whenever the magic word there
; is missing (core A clears $30000-$37FFF at its own init). Only the hook and
; the two per-sample loops live in internal P.
;
; Hook: the main loop's `jsr $0221` (voice pass, P:$011D) becomes a short jsr to
; ge_hook, which runs the voice pass and then the engine. The voice pass has
; just cleared and mixed every voice's 32 output frames at
; y:$304F0 + voice*64 + bank*32 (bank = y:$FFFFD0, as stock $06AB); grains add
; to them, and core A applies the track level, pan and FX as for stock output.
;
; Interface: per voice, parameter block y:$307C8 + 32*voice, words stock never
; uses (the ARM shadow-writes each as a plain 24-bit value):
;   +26 position   grain centre as a positive fraction of the sample ($7FFFFF = end)
;   +27 grain dt   window advance per 32-frame block, 1.0 = $800000 (2^23 / blocks)
;   +28 interval   bits 0-15 frames between grain onsets, 0 = no new grains;
;                  bits 16-17 pitch scatter (0 off, 1 random octave down/up,
;                  2 also a fifth up), bits 18-19 shape (0 smooth, 1
;                  percussive, 2 swell); the whole word is 0 to stop
;   +29 rate       signed playback rate, 1.0 = $100000 (clamped to +/-4)
;   +30 spray/gain bits 0-15 position spray in frames, bits 16-23 gain / 256
; The sample is the voice's own slot (+0); its pages are read as stock $073D
; does, and grains wrap within the slot's [base, base + length).
;
; Each grain's window (computed per block, ramped per frame) is smooth
; (4t(1-t))^2, percussive min(8t, 1)(1-t)^2, or swell (percussive reversed),
; over a linearly interpolated read. A grain starting mid-block begins with a
; negative window phase, so its onset is placed to the frame within the
; block-rate ramp (the window is near zero there). At most 32 grains play.
;
; Telemetry (shared Y): $37F10 grains playing, $37F11 onsets dropped because
; all 32 were busy, $37F12 blocks run, $37F13 = magic while the engine runs.

ge_code         equ     $037400         ; shared P run copy (magic, then code)
ge_master       equ     $001800         ; internal Y master copy
ge_magic        equ     $47524e
ge_pool         equ     $002b00         ; X: 32 grains x 12 words
ge_grains       equ     32
ge_gsize        equ     12
ge_pool_end     equ     ge_pool+ge_grains*ge_gsize
ge_trk          equ     $002c80         ; X: per-voice frames to the next onset
ge_rnd          equ     $002c88
ge_v            equ     $002c89
ge_off          equ     $002c8a
ge_iv           equ     $002c8b
ge_slot         equ     $002c8c
ge_c            equ     $002c8d
ge_out          equ     $002c8e
ge_w            equ     $002c8f
ge_dw           equ     $002c90
ge_n            equ     $002c91
ge_s            equ     $002c92
ge_chunk        equ     $002c93
ge_t1           equ     $002c94
ge_dead         equ     $002c95
ge_pe_i         equ     $002c96
ge_pe_f         equ     $002c97
ge_loc          equ     $002c98
ge_left         equ     $002c99
ge_live         equ     $002c9a
ge_xmagic       equ     $002c9b
ge_shp          equ     $002c9c         ; the rendering grain's shape
ge_xend         equ     $002c9d
ge_scr          equ     $002d00         ; X: fetched source frames (<= 130)
ge_tmp          equ     $002e00         ; X: one grain's interpolated block
ge_rate_i       equ     $34             ; X (short): rate, whole part
ge_rate_f       equ     $35             ; X (short): rate, fraction
ge_params       equ     $0307c8
ge_meta         equ     $030000
ge_pages        equ     $0308c8
ge_mix          equ     $0304f0
ge_stat_live    equ     $037f10
ge_stat_drop    equ     $037f11
ge_stat_blocks  equ     $037f12
ge_stat_magic   equ     $037f13

; Grain record (X): +0 voice + 1 (0 free), +1 page-table row, +2 base, +3 end,
; +4 length, +5/+6 position (whole, fraction), +7/+8 rate (whole, fraction),
; +9 window phase t, +10 dt per block, +11 gain (bits 0-1 hold the shape; the
; gain is a multiple of 2^15, so they are inaudible).

; ---------------------------------------------------------------- shared P
org p:ge_code
ge_start:
        dc      ge_magic

ge_run:
        move    x:$0001,a
        cmp     #>32,a
        jne     ge_exit
        move    x:ge_xmagic,a
        cmp     #>ge_magic,a
        jeq     ge_ready
        clr     a
        move    #ge_pool,r1
        do      #ge_xend-ge_pool,ge_clear_end
        move    a,x:(r1)+
ge_clear_end:
        move    #>$5a5a5a,a
        move    a1,x:ge_rnd
        move    #>ge_magic,a
        move    a1,x:ge_xmagic
ge_ready:
        jsr     ge_spawn_all
        jsr     ge_render_all
        move    y:ge_stat_blocks,a
        add     #>1,a
        move    a1,y:ge_stat_blocks
        move    #>ge_magic,a
        move    a1,y:ge_stat_magic
ge_exit:
        rts

; Per voice: count down to the next onset; at most one per block (the
; interval is at least 64 frames).
ge_spawn_all:
        clr     a
        move    a1,x:ge_v
ge_sa_loop:
        move    x:ge_v,a
        asl     #5,a,a
        add     #>ge_params,a
        move    a1,r6
        move    x:ge_v,a
        add     #>ge_trk,a
        move    a1,r3
        move    y:(r6+28),b
        tst     b
        jgt     ge_sa_on
        clr     a
        move    a1,x:(r3)
        jmp     ge_sa_next
ge_sa_on:
        and     #>$ffff,b
        move    #>64,x0
        cmp     x0,b
        tlt     x0,b
        move    b1,x:ge_iv
        move    x:(r3),a
        tst     a
        jge     ge_sa_wait
        clr     a
ge_sa_wait:
        move    #>32,x0
        cmp     x0,a
        jge     ge_sa_count
        move    a1,x:ge_off
        jsr     ge_spawn
        move    x:ge_v,a
        add     #>ge_trk,a
        move    a1,r3
        move    x:ge_off,a
        move    x:ge_iv,x0
        add     x0,a
ge_sa_count:
        move    #>32,x0
        sub     x0,a
        move    a1,x:(r3)
ge_sa_next:
        move    x:ge_v,a
        add     #>1,a
        move    a1,x:ge_v
        cmp     #>8,a
        jlt     ge_sa_loop
        rts

; r6 = parameter block, x:ge_v = voice, x:ge_off = onset frame in this block.
ge_spawn:
        move    y:(r6),a
        tst     a
        jlt     ge_sp_done
        cmp     #>128,a
        jge     ge_sp_done
        move    a1,x:ge_slot
        asl     #2,a,a
        add     #>ge_meta,a
        move    a1,r4
        move    x:(r4+1),b
        tst     b
        jle     ge_sp_done
        move    x:(r4+2),a
        tst     a
        jeq     ge_sp_done
        move    #ge_pool,r1
        move    #ge_gsize,n1
ge_sp_find:
        move    x:(r1),a
        tst     a
        jeq     ge_sp_found
        move    (r1)+n1
        move    r1,a
        cmp     #>ge_pool_end,a
        jlt     ge_sp_find
        move    y:ge_stat_drop,a
        add     #>1,a
        move    a1,y:ge_stat_drop
ge_sp_done:
        rts
ge_sp_found:
        ; page-table row = $308C8 + slot * $BC (b = length is kept to the end)
        move    x:ge_slot,x0
        move    #>$bc,y0
        mpy     x0,y0,a
        asr     #1,a,a
        move    a0,x0
        move    #>ge_pages,a
        add     x0,a
        move    a1,x:(r1+1)
        move    x:(r4),a
        move    a1,x:(r1+2)
        move    b1,x:(r1+4)
        add     b,a
        move    a1,x:(r1+3)
        ; centre = base + position * length
        move    y:(r6+26),x0
        move    b1,y0
        mpy     x0,y0,a
        move    x:(r4),x0
        add     x0,a
        move    a1,x:ge_c
        ; spray (at most the length) times a random signed fraction
        move    y:(r6+30),x0
        move    #>$ffff,a
        and     x0,a
        cmp     b,a
        tgt     b,a
        move    a1,y0
        jsr     ge_rand
        move    a1,x0
        mpy     x0,y0,a
        move    x:ge_c,x0
        add     x0,a
        move    x:(r1+2),x0
        cmp     x0,a
        jge     ge_sp_above
        add     b,a
ge_sp_above:
        move    x:(r1+3),x0
        cmp     x0,a
        jlt     ge_sp_below
        sub     b,a
ge_sp_below:
        move    a1,x:(r1+5)
        move    a0,x:(r1+6)
        ; rate, optionally scattered to x0.5 / x2 (and x1.5) by two random
        ; bits, then clamped to +/-4 as a 48-bit whole.fraction step
        move    y:(r6+28),b
        lsr     #16,b
        and     #>3,b
        move    b1,y1
        move    y:(r6+29),a
        move    y1,b
        tst     b
        jeq     ge_sp_rate
        jsr     ge_rand
        lsr     #22,a
        move    a1,x1
        move    y:(r6+29),a
        move    x1,b
        tst     b
        jne     ge_sp_up
        asr     a                               ; 00: an octave down
        jmp     ge_sp_rate
ge_sp_up:
        cmp     #>3,b
        jne     ge_sp_fifth
        asl     a                               ; 11: an octave up
        jmp     ge_sp_rate
ge_sp_fifth:
        cmp     #>2,b
        jne     ge_sp_rate
        move    y1,b
        cmp     #>2,b
        jne     ge_sp_rate
        tfr     a,b                             ; 10 in mode 2: a fifth up
        asr     b
        add     b,a
ge_sp_rate:
        move    #>$400000,x0
        cmp     x0,a
        tgt     x0,a
        move    #>$c00000,x0
        cmp     x0,a
        tlt     x0,a
        asr     #20,a,a
        move    a1,x:(r1+7)
        move    a0,x:(r1+8)
        ; dt per block (default 1/256, at most 1/2)
        move    y:(r6+27),a
        tst     a
        jgt     ge_sp_dt
        move    #>$008000,a
ge_sp_dt:
        move    #>$400000,x0
        cmp     x0,a
        tgt     x0,a
        move    a1,x:(r1+10)
        ; window phase at the block start = -offset * dt / 32
        move    a1,y0
        move    x:ge_off,x0
        mpy     x0,y0,a
        asl     #18,a,a
        neg     a
        move    a1,x:(r1+9)
        ; gain = bits 16-23 / 256, with the shape (p28 bits 18-19) in bits 0-1
        move    y:(r6+30),a
        lsr     #16,a
        lsl     #15,a
        move    y:(r6+28),b
        lsr     #18,b
        and     #>3,b
        move    b1,x0
        or      x0,a
        move    a1,x:(r1+11)
        move    x:ge_v,a
        add     #>1,a
        move    a1,x:(r1)
        rts

; 24-bit Galois LFSR; returns the new state in a1 (uses a, x0).
ge_rand:
        move    x:ge_rnd,a
        lsl     a
        jcc     ge_rand_keep
        move    #>$c20001,x0
        eor     x0,a
ge_rand_keep:
        move    a1,x:ge_rnd
        rts

ge_render_all:
        clr     a
        move    a1,x:ge_live
        move    #>ge_grains,a
        move    a1,x:ge_left
        move    #ge_pool,r0
ge_ra_loop:
        move    x:(r0),a
        tst     a
        jeq     ge_ra_next
        jsr     ge_render
        move    x:(r0),a
        tst     a
        jeq     ge_ra_next
        move    x:ge_live,a
        add     #>1,a
        move    a1,x:ge_live
ge_ra_next:
        move    #ge_gsize,n0
        move    x:ge_left,a
        move    (r0)+n0
        sub     #>1,a
        move    a1,x:ge_left
        jgt     ge_ra_loop
        move    x:ge_live,a
        move    a1,y:ge_stat_live
        rts

; One block of the grain at r0 (r0 is kept).
ge_render:
        ; output = mix + voice * 64 + bank * 32
        move    x:(r0),a
        sub     #>1,a
        asl     #6,a,a
        add     #>ge_mix,a
        movep   y:$ffffd0,b
        asl     #5,b,b
        add     b,a
        move    a1,x:ge_out
        ; window at the block start and end, times the gain
        move    x:(r0+11),a
        and     #>3,a
        move    a1,x:ge_shp
        move    x:(r0+9),a
        jsr     ge_win
        move    a,x0
        move    x:(r0+11),y0
        mpy     x0,y0,a
        move    a,x:ge_w
        clr     b
        move    b1,x:ge_dead
        move    x:(r0+9),a
        move    x:(r0+10),x0
        add     x0,a
        move    #>$7fffff,x0
        cmp     x0,a
        jlt     ge_r_alive
        move    #>1,b
        move    b1,x:ge_dead
        clr     a
ge_r_alive:
        move    a1,x:ge_t1
        jsr     ge_win
        move    a,x0
        move    x:(r0+11),y0
        mpy     x0,y0,a
        move    x:ge_w,x0
        sub     x0,a
        asr     #5,a,a
        move    a,x:ge_dw
        ; position after the block = position + 32 * rate
        move    x:(r0+7),b
        move    x:(r0+8),b0
        asl     #5,b,b
        move    x:(r0+5),a
        move    x:(r0+6),a0
        add     a,b
        move    b1,x:ge_pe_i
        move    b0,x:ge_pe_f
        ; frames to read: [lo, hi + 1], lo = the lower of the two positions
        move    x:(r0+7),a
        tst     a
        jlt     ge_r_back
        move    x:(r0+5),x0
        move    x:ge_pe_i,a
        jmp     ge_r_span
ge_r_back:
        move    x:ge_pe_i,x0
        move    x:(r0+5),a
ge_r_span:
        sub     x0,a
        add     #>2,a
        move    a1,x:ge_n
        move    x:(r0+5),a
        sub     x0,a
        add     #>ge_scr,a
        move    a1,x:ge_loc
        tfr     x0,a
        jsr     ge_wrap
        move    a1,x:ge_s
        jsr     ge_fetch
        ; interpolate, then window into the voice's output
        move    x:(r0+7),a
        move    a1,x:ge_rate_i
        move    x:(r0+8),a
        move    a1,x:ge_rate_f
        move    x:ge_loc,a
        move    x:(r0+6),a0
        jsr     >ge_interp
        jsr     >ge_window
        ; advance
        move    x:ge_pe_i,a
        jsr     ge_wrap
        move    a1,x:(r0+5)
        move    x:ge_pe_f,a
        move    a1,x:(r0+6)
        move    x:ge_t1,a
        move    a1,x:(r0+9)
        move    x:ge_dead,a
        tst     a
        jeq     ge_r_done
        clr     a
        move    a1,x:(r0)
ge_r_done:
        rts

; a1 = a frame position; returns it wrapped into [base, end) (uses x0, x1, y0).
ge_wrap:
        move    x:(r0+2),x0
        move    x:(r0+4),x1
        move    x:(r0+3),y0
ge_wrap_low:
        cmp     x0,a
        jge     ge_wrap_high
        add     x1,a
        jmp     ge_wrap_low
ge_wrap_high:
        cmp     y0,a
        jlt     ge_wrap_done
        sub     x1,a
        jmp     ge_wrap_high
ge_wrap_done:
        rts

; a = window phase t (1.0 = $800000); returns a = the shape's window, 0
; outside (0, 1) (uses b, x0, y0).
ge_win:
        tst     a
        jle     ge_win_zero
        move    #>$7fffff,x0
        cmp     x0,a
        jge     ge_win_zero
        move    x:ge_shp,b
        tst     b
        jne     ge_win_shaped
        ; smooth: (4t(1-t))^2
        move    a1,x0
        move    #>$7fffff,b
        sub     x0,b
        move    b1,y0
        mpy     x0,y0,a
        asl     #2,a,a
        move    a,x0
        mpy     x0,x0,a
        rts
ge_win_shaped:
        cmp     #>2,b
        jne     ge_win_perc
        ; swell: the percussive window at 1 - t
        move    a1,x0
        move    #>$7fffff,a
        sub     x0,a
ge_win_perc:
        ; percussive: min(8t, 1) * (1-t)^2
        move    a1,x0
        move    #>$7fffff,b
        sub     x0,b
        move    b1,y0
        mpy     y0,y0,b
        move    b,y0
        move    #>$100000,x0
        cmp     x0,a
        jge     ge_win_full
        asl     #3,a,a
        move    a,x0
        mpy     x0,y0,a
        rts
ge_win_full:
        tfr     y0,a
        rts
ge_win_zero:
        clr     a
        rts

; Copies x:ge_n frames from slot position x:ge_s (in [base, end)) of the grain
; at r0 to x:ge_scr, a page (and the slot end) at a time, as stock $073D reads.
ge_fetch:
        move    #ge_scr,r2
ge_f_loop:
        ; chunk = min(left, $1000 - (s & $FFF), end - s)
        move    x:ge_s,a
        and     #>$fff,a
        move    #>$1000,b
        sub     a,b
        move    x:ge_n,a
        cmp     a,b
        tgt     a,b
        move    x:ge_s,x0
        move    x:(r0+3),a
        sub     x0,a
        cmp     a,b
        tgt     a,b
        move    b1,x:ge_chunk
        ; physical = page << 12 | (s & $FFF); the row holds two 12-bit pages
        move    x:ge_s,a
        asr     #13,a,a
        move    x:(r0+1),x0
        add     x0,a
        move    a1,r1
        move    x:ge_s,a
        jset    #12,a1,ge_f_odd
        move    y:(r1),a
        lsr     #12,a
        jmp     ge_f_page
ge_f_odd:
        move    y:(r1),a
ge_f_page:
        and     #>$fff,a
        lsl     #12,a
        move    a1,x0
        move    x:ge_s,a
        and     #>$fff,a
        or      x0,a
        move    a1,r1
        move    x:ge_chunk,b
        do      b1,ge_f_copy
        move    y:(r1)+,x0
        move    x0,x:(r2)+
ge_f_copy:
        move    x:ge_s,a
        add     b,a
        move    x:(r0+3),x0
        cmp     x0,a
        jlt     ge_f_inside
        move    x:(r0+2),a
ge_f_inside:
        move    a1,x:ge_s
        move    x:ge_n,a
        sub     b,a
        move    a1,x:ge_n
        tst     a
        jgt     ge_f_loop
        rts
ge_end:

ge_len          equ     ge_end-ge_start

; ---------------------------------------------------------------- internal P
org p:$0fa0
; Replaces the main loop's `jsr $0221`.
ge_hook:
        jsr     $0221
        move    y:ge_code,a
        move    #>ge_magic,x0
        cmp     x0,a
        jeq     ge_go
        move    #ge_master,r4
        move    #ge_code,r5
        do      #ge_len,ge_copy_end
        move    y:(r4)+,x0
        move    x0,y:(r5)+
ge_copy_end:
ge_go:
        jsr     >ge_run
        rts

; a1 = x:ge_scr + frame, a0 = fraction; rate at x:$34/$35. Writes 32 linearly
; interpolated frames to x:ge_tmp.
ge_interp:
        move    #ge_tmp,r2
        do      #32,ge_interp_end
        move    a1,r1
        move    a0,b
        lsr     b                               ; as a positive Q23 (= the fraction)
        move    b1,y1
        move    x:(r1)+,x0
        mpy     -x0,y1,b        x:(r1),x1
        mac     x1,y1,b         x:<ge_rate_i,x1
        add     x0,b            x:<ge_rate_f,x0
        add     x,a             b,x:(r2)+
ge_interp_end:
        rts

; Adds x:ge_tmp times the ramp (x:ge_w, + x:ge_dw per frame) into x:ge_out.
ge_window:
        move    #ge_tmp,r2
        move    x:ge_out,r5
        move    x:ge_w,a
        move    x:ge_dw,y0
        move    r5,r6
        do      #32,ge_window_end
        move    a,y1
        move    x:(r2)+,x0      y:(r5)+,b
        mac     x0,y1,b
        add     y0,a            b,y:(r6)+
ge_window_end:
        rts
ge_internal_end:

; Circuit Rhythm build-5706 core-B looper declick (granular looper builds only).
;
; The voice refill ($016E) copies source samples into the voice's 512-word ring
; through $073D (forward) or $0798 (reverse), entered with r1 = source position,
; b1 = count, r5 = ring write pointer and r4 = voice struct + 3 (y:(r4) = end,
; y:(r4+1) = loop start, x:(r4+2) = parameter block). Both routines open with
; the one-word `move a,n5`; that word is replaced by a short jump here.
;
; A voice opts in through parameter 31 (y:(param block + 31) != 0), a slot no
; stock DSP image reads and the stock ARM never writes. For those voices the
; samples just copied are scaled by min(|pos - end|, |pos - loop start|, 128)
; / 128, a 128-sample (2.7 ms at unity) fade at the note start, on both sides
; of the loop seam and into a one-shot end. Other voices run stock unchanged.
;
; Retrigger: a trigger command sets the old voice to envelope stage 4, which
; stock fades linearly over one 32-frame block (0.67 ms, an audible click)
; before the new note starts in the same block ($02E4). For flagged voices the
; stage-4 table entry ($05D2) runs dk_stage4, which instead decays the level
; by a quarter per block until it is under 1/16 and then lets the stock block
; finish it; the voice start hook ($02F2) holds the pending trigger while a
; flagged, active voice is still in stage 4. A sounding voice so fades over
; about 7 ms; a silent one starts exactly as stock.
;
; The wrapper keeps every register the refill relies on (a via n5 as stock,
; r2, n2, r4, the final r5, m5 = m0). The ALU inputs, b, r1 and r3 end with
; different values than stock leaves; the copy already leaves them data
; dependent and no caller reads them afterwards.

dk_pos          equ     $30             ; x: source position on entry
dk_count        equ     $31             ; x: samples copied
dk_ring         equ     $32             ; x: ring write pointer on entry
dk_ring_end     equ     $33             ; x: ring write pointer after the copy
dk_param        equ     31              ; opt-in parameter slot
dk_fade         equ     $100            ; fade length in source samples (2^(23-dk_shift))
dk_floor        equ     $080000         ; stage-4 level handed to the stock fade (1/16)
dk_shift        equ     15              ; gain = min(distance, dk_fade) << dk_shift

org p:$0f00
dk_forward:
        move    a,n5
        move    x:(r4+2),r3
        move    y:(r3+31),a
        tst     a
        jeq     >$073e
        move    r1,x:dk_pos
        move    b1,x:dk_count
        move    r5,x:dk_ring
        jsr     >$073e
        jsr     dk_setup
        jeq     dk_done
        do      x0,dk_forward_end
        tfr     b,a
        abs     a
        cmp     y1,a
        tgt     y1,a
        move    a1,x0
        tfr     b,a
        add     x1,a
        abs     a
        cmp     x0,a
        tgt     x0,a
        asl     #dk_shift,a,a
        move    a,x0
        move    y:(r5),y0
        mpyr    x0,y0,a
        move    a,y:(r5)+
        inc     b
dk_forward_end:
        jmp     dk_done

dk_reverse:
        move    a,n5
        move    x:(r4+2),r3
        move    y:(r3+31),a
        tst     a
        jeq     >$0799
        move    r1,a
        sub     #>1,a
        move    a1,x:dk_pos
        move    b1,x:dk_count
        move    r5,x:dk_ring
        jsr     >$0799
        jsr     dk_setup
        jeq     dk_done
        do      x0,dk_reverse_end
        tfr     b,a
        abs     a
        cmp     y1,a
        tgt     y1,a
        move    a1,x0
        tfr     b,a
        add     x1,a
        abs     a
        cmp     x0,a
        tgt     x0,a
        asl     #dk_shift,a,a
        move    a,x0
        move    y:(r5),y0
        mpyr    x0,y0,a
        move    a,y:(r5)+
        dec     b
dk_reverse_end:

dk_done:
        movec   m0,m5
        move    x:dk_ring_end,r5
        move    n5,a
        rts

; After the copy: b = first position - end, x1 = end - loop start (so b + x1
; = position - loop start), y1 = fade length, x0 = count, r5 = ring start with
; m5 = $1FF. Returns Z set when nothing was copied.
dk_setup:
        move    r5,x:dk_ring_end
        clr     a
        clr     b
        move    y:(r4),a1
        move    y:(r4+1),b1
        sub     b,a
        move    a,x1
        clr     b
        clr     a
        move    x:dk_pos,b1
        move    y:(r4),a1
        sub     a,b
        move    #>dk_fade,y1
        move    x:dk_ring,r5
        movec   #$1ff,m5
        move    x:dk_count,a
        move    a1,x0
        tst     a
        rts


; Stage-4 table entry (jmp (r3+n3) from $0533): a = level x:(r6+11), r1 = the
; block buffer x:$2A02, r4 = r1 - 1, r6 = voice struct, r7 = parameter block.
dk_stage4:
        move    y:(r7+31),b
        tst     b
        jeq     >$0636
        move    #>dk_floor,x0
        cmp     x0,a
        jlt     >$0636
        tfr     a,b
        asr     #7,b,b
        move    b,y1
        sub     y1,a
        move    a,x1
        dor     x:$0001,dk_stage4_end
        move    x:(r1)+,x0
        mpy     x1,x0,b b,x:(r4)+
        sub     y1,a
        move    a,x1
dk_stage4_end:
        move    b,x:(r4)
        move    a,x:(r6+11)
        rts

; Replaces `jsne >$02FD` at $02F2: y1 = pending word0 (its low bits are the
; voice), y0 = word1, r4 = &word1. y0, y1 are kept for the stock start.
dk_start:
        clr     a
        move    #>7,a1
        and     y1,a
        asl     #4,a,a
        add     #>$40,a
        move    a1,r5
        move    x:(r5+5),r6
        move    y:(r6+31),b
        tst     b
        jeq     >$02fd
        move    x:(r5+4),b
        tst     b
        jeq     >$02fd
        move    x:(r5+10),b
        move    #>4,x0
        cmp     x0,b
        jne     >$02fd
        move    y1,y:-(r4)
        rts
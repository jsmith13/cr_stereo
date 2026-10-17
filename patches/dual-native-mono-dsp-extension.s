; Circuit Rhythm build-5706 native dual-mono capture extension.
; Capture-overlay raw/LMA addresses are relocated by -$2A33 at runtime.
; Mono enters the displaced stock instruction and resumes stock code exactly.

org p:$0d32
capture_finalize_entry:
        move y:$371d4,a
        brclr #2,a1,finalize_mono
        move y:$371d5,a
        cmp #$000080,a
        bge finalize_done

        ; Fix the right slot's final producer/readable extents first.
        add #>$000001,a
        move a1,x1
        asl #$02,a,a
        add #$030001,a
        move a1,r1
        move y:$371d7,a
        move a,y:(r1)+
        move a,y:(r1)+
        clr b
        move b,y:(r1)

        ; Publish the final shared peak to the right slot without relying on
        ; the active modulo state of an incremented address register.
        move y:$371d5,a
        add #$0371f2,a
        move a1,r0
        move y:(r0),b
        move y:$371d5,a
        add #$0371f3,a
        move a1,r0
        move b,y:(r0)

        ; Run the unmodified stock left finalizer and ordinary queue enqueue.
        jsr >stock_finalize_once

        ; Enqueue the adjacent right slot through the stock queue routine.
        move y:$371d5,a
        add #>$000001,a
        move a1,x1
        jsr >$0c97
finalize_done:
        rts
finalize_mono:
        move y:$371d5,a
        jmp >$0868

org p:$0d64
capture_peak_entry:
        move y:$371d4,a
        brclr #2,a1,peak_mono
        move #$030710,r0
        move y:$371e8,a
        tst a
        beq peak_source_selected
        move #$030750,r0
peak_source_selected:
        clr b
        move x:$0001,x1
        dor x1,peak_scan_done
        move x:(r0)+,a
        abs a
        maxm a,b
        move x:(r0)+,a
        abs a
        maxm a,b
peak_scan_done:
        move y:$371e9,x1
        cmp x1,b
        blt peak_update
        move y:$371d4,a
        move a,y1
        brclr #0,y1,peak_update
        asr #$10,a,a
        cmp #$00007f,a
        bge peak_reject
        move a1,y:$371d5

        ; Publish the triggering block's shared peak before reusing B.
        add #$0371f2,a
        move a1,r0
        move b,y:(r0)

        ; Materialize an empty adjacent right slot.
        move y:$371d5,a
        add #>$000001,a
        move a1,x1
        asl #$02,a,a
        add #$030000,a
        move a1,r1
        clr a
        move a,y:(r1)+
        move a,y:(r1)+
        move a,y:(r1)+
        move a,y:(r1)
        move x1,b
        move #$0366c8,a
        add b,a
        move a1,r1
        clr a
        move a,y:(r1)

        ; Retain only the stereo-active bit and initialize one shared peak.
stereo_active_state:
        move #>$000004,a
        move a,y:$371d4
        rts
peak_reject:
        clr a
        move a,y:$371d4
        rts

peak_update:
        move y:$371d5,a
        cmp #$000080,a
        bge peak_done
        add #$0371f2,a
        move a1,r0
        move y:(r0),a
        maxm a,b
        move b,y:(r0)
peak_done:
        rts
peak_mono:
        move #$030790,r0
        jmp >$087d

org p:$0dc0
capture_average_entry:
        move y:$371d4,a
        brclr #2,a1,average_mono
        rts
average_mono:
        move #$030710,r0
        jmp >$08a8

org p:$0dc9
capture_writer_entry:
        move y:$371d4,a
        brclr #2,a1,writer_mono
        move y:$371d5,a
        cmp #$000080,a
        bge writer_done

        move y:$371d7,a
        and #$000fff,a
        tst a
        bne writer_pages_ready
        jsr >capture_allocator_entry
        move y:$371d5,a
        cmp #$000080,a
        bge writer_done
writer_pages_ready:
        ; Left destination uses the stock current-page base.
        move y:$371d7,b
        and #$000fff,b
        move y:$371d6,a
        add b,a
        move a1,r4

        ; Resolve the adjacent right page from its page table and shared cursor.
        move y:$371d5,a
        add #>$000001,a
        move a1,x1
        move y:$371d7,y0
        jsr >resolve_slot_page
        move y:$371d7,b
        and #$000fff,b
        add b,a
        move a1,r5
        movec m0,m4
        movec m0,m5

        move #$030710,r0
        move y:$371e8,a
        tst a
        beq writer_source_selected
        move #$030750,r0
writer_source_selected:
        move x:$0001,x1
        dor x1,writer_copy_done
        move x:(r0)+,a
        move a,y:(r4)+
        move x:(r0)+,a
        move a,y:(r5)+
writer_copy_done:
        move y:$371d7,a
        add x1,a
        move a,y:$371d7

        ; Publish equal producer extents to N and N+1.
        move y:$371d5,b
        asl #$02,b,b
        move #$030001,x1
        add x1,b
        move b1,r1
        move a,y:(r1)+
        move a,y:(r1)
        move (r1)+,r1
        move (r1)+,r1
        move (r1)+,r1
        move a,y:(r1)+
        move a,y:(r1)
writer_done:
        rts
writer_mono:
        move y:$371d5,a
        jmp >$08bf

org p:$0e23
capture_allocator_entry:
        move y:$371d4,a
        brclr #2,a1,allocator_mono

        ; Validate the left slot and reserve two free pages before mutation.
        move y:$371d5,a
        cmp #$00007f,a
        bge allocator_fail
        move y:$371d1,b
        cmp #$000002,b
        blt allocator_fail

        ; Both per-slot page indices must remain below 376.
        move a1,x1
        move #$0366c8,a
        add x1,a
        move a1,r4
        move y:(r4),a
        asr #$01,a,a
        cmp #$0000bc,a
        bge allocator_fail
        move y:$371d5,a
        add #>$000001,a
        move #$0366c8,b
        add b,a
        move a1,r4
        move y:(r4),a
        asr #$01,a,a
        cmp #$0000bc,a
        bge allocator_fail

        ; Validate both selected free-list entries before either stock pop.
        move y:$371d0,a
        move a1,r4
        move x:(r4),a
        tst a
        beq allocator_fail
        move a1,b
        and #$000fff,b
        tst b
        bne allocator_fail
        move y:$371cf,a
        add #>$000001,a
        clr b
        cmp #$000a87,a
        tfr b,a ifge
        add #$036748,a
        move a1,r4
        move x:(r4),a
        tst a
        beq allocator_fail
        move a1,b
        and #$000fff,b
        tst b
        bne allocator_fail

        ; Sequential stock pops are atomic after complete prevalidation.
        jsr >stock_allocator_once
        move y:$371d5,a
        add #>$000001,a
        move a1,y:$371d5
        jsr >stock_allocator_once
        move y:$371d5,a
        sub #>$000001,a
        move a1,y:$371d5

        ; Restore the stock left current-page base expected by the writer.
        move a1,x1
        move y:$371d7,y0
        jsr >resolve_slot_page
allocator_restore_left_page:
        move a1,y:$371d6
        rts

allocator_fail:
        jmp >$0973
allocator_mono:
        move y:$371d1,a
        jmp >$0b85

org p:$0e8b
stock_finalize_once:
        move y:$371d5,a
        jmp >$0868

stock_allocator_once:
        move y:$371d1,a
        jmp >$0b85

; Input: X1=slot, Y0=shared frame cursor. Output: A1=physical page base.
resolve_slot_page:
        move #$5e,y1
        mpy +y1,x1,a
        asl #$08,a,a
        add #$0308c8,a
        move y0,b
        asr #$0c,b,b
        move b1,b
        move b,y1
        asr #$01,b,b
        add b,a
        move a1,r0
        move y:(r0),a
        brset #0,y1,resolve_high_half
        asr #$0c,a,a
resolve_high_half:
        and #$000fff,a
        move a1,a
        asl #$0c,a,a
        rts

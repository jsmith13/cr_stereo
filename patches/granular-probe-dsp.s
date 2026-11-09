; Circuit Rhythm build-5706 core-B load telemetry (granular probe builds only).
;
; Two one-word stock calls in the core-B main loop are redirected here:
;   P:$0111 jsr $07F8 (the idle spin's background poll) -> gp_idle_spin
;   P:$0126 jsr $0A79 (the last call of each block)     -> gp_block_end
; Both resume stock exactly: the spin tail-jumps to $07F8, whose rts returns to
; P:$0112; the block hook calls $0A79 first. A is dead at both sites ($07F8
; reloads it first); the block hook still restores X0 and B, so stock sees
; exactly what $0A79 left.
;
; Telemetry lives in shared Y:$37F00.. (core A clears $30000-$37FFF at init;
; no DSP image references anything above $37301). The ARM reads it through
; DSP_READ and writes only GP_BURN.

gp_spin         equ     $037f00         ; idle spins since the last block
gp_last         equ     $037f01         ; spins before the last block
gp_runmin       equ     $037f02         ; running minimum this window (0 unset)
gp_min          equ     $037f03         ; minimum of the last 1024-block window
gp_runsum       equ     $037f04         ; running sum this window
gp_sum          equ     $037f05         ; sum of the last 1024-block window
gp_blocks       equ     $037f06         ; blocks processed (wraps)
gp_burn         equ     $037f07         ; ARM: 4-cycle burn iterations per block
gp_magic        equ     $037f08         ; $47504B once a window has completed
gp_save         equ     $037f09         ; X0, B2, B1, B0 kept across the block hook

org p:$0ec0
gp_idle_spin:
        move y:gp_spin,a
        add #>$000001,a
        move a1,y:gp_spin
        jmp >$07f8

gp_block_end:
        jsr >$0a79
        move x0,y:gp_save
        move b2,y:gp_save+1
        move b1,y:gp_save+2
        move b0,y:gp_save+3
        move y:gp_spin,a
        move a1,y:gp_last
        move y:gp_runsum,b
        add a,b
        move b1,y:gp_runsum
        move y:gp_runmin,b
        tst b
        beq gp_new_min
        cmp a,b
        ble gp_keep_min
gp_new_min:
        move a1,y:gp_runmin
gp_keep_min:
        clr a
        move a1,y:gp_spin
        move y:gp_blocks,a
        add #>$000001,a
        move a1,y:gp_blocks
        and #>$0003ff,a
        bne gp_burn_check
        move y:gp_runsum,b
        move b1,y:gp_sum
        move y:gp_runmin,b
        move b1,y:gp_min
        clr b
        move b1,y:gp_runsum
        move b1,y:gp_runmin
        move #>$47504b,b
        move b1,y:gp_magic
gp_burn_check:
        move y:gp_burn,a
        tst a
        beq gp_done
        move a1,x0
        do x0,gp_burn_end
        nop
        nop
        nop
        nop
gp_burn_end:
        nop
gp_done:
        move y:gp_save,x0
        move y:gp_save+1,b2
        move y:gp_save+2,b1
        move y:gp_save+3,b0
        rts

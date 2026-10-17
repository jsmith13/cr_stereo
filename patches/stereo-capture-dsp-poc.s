org p:$32d9
        move #$030710,r0
        move y:$371e8,a
        tst a
        beq source_selected
        move #$030750,r0
source_selected:
        move r0,x:$0004
        movec m0,m5
        rts
        nop
        nop
        nop
        nop
        nop
        nop
        nop
        nop
        nop
        nop
        nop

org p:$330a
        move x:$0004,r0
        move a1,r5
        movec m0,m5
        move x:$0001,a
        asl a
        move a1,x1
        dor x1,copy_done
        move x:(r0)+,a
        nop
        move a,y:(r5)+
copy_done:

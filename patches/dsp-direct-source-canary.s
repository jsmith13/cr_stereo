org p:$32d9
        move #$030710,r0
        move y:$371e8,a
        movec m0,m5
        tst a
        beq source_selected
        move #$030750,r0
source_selected:
        move #$030790,r5
        move r0,x:$0004
        move x:$0001,x1
        move x:(r0)+,a
        move x:(r0)+,b
        dor x1,average_done
        add a,b x:(r0)+,a
        asr #$01,b,b
        move x:(r0)+,b b,y:(r5)+
        nop
average_done:
        rts

org p:$330a
        move x:$0004,r0
        nop
        asr #$18,a,a
        movec m0,m5
        move a0,r5
        move x:$0001,x1
        dor x1,copy_done
        move x:(r0)+,a
        nop
        move a,y:(r5)+
copy_done:

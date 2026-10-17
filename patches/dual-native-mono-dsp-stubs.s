; Runtime/VMA hooks for the capture overlay stored at raw/LMA +$2A33.
; Each hook is exactly two words and replaces one two-word stock instruction.

org p:$0866
        jmp >$0d32

org p:$087b
        jmp >$0d64

org p:$08a6
        jmp >$0dc0

org p:$08bd
        jmp >$0dc9

org p:$0b83
        jmp >$0e23

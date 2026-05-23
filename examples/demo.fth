\ Demo — exercises every feature of the compiler:
\   constant, variable, : ... ;, if/else/then, begin/until, begin/again

constant status-buf  0x9000
constant score-off   0x14         \ offset of `score` in Status struct

variable tick-counter             \ 4 bytes, initialized to 0 in binary

: read-status  ( -- )
    status-buf 1 1 syscall drop ; \ ( result_addr argc syscall# -- ret ) → drop

: my-score  ( -- score )
    status-buf score-off + @ ;

\ if/else demo: set tick-counter to 1 if score>100, else 0
: classify  ( -- )
    my-score 100 >
    if
        1 tick-counter !
    else
        0 tick-counter !
    then ;

\ begin/until demo: count down from 3, exits when counter reaches 0
: countdown  ( -- )
    3 tick-counter !
    begin
        tick-counter @  1 -    ( -- n-1 )
        dup tick-counter !     ( -- n-1 )
        0 =                    ( -- bool )
    until ;

\ Move syscall helper:  ( x y dir triggers -- packed-pos )
: move  4 2 syscall ;

\ `exit` demo: bail out of the function early if score is huge.
: bail-if-winning  ( -- )
    my-score 10000 >
    if exit then ;

\ Force long-form back jump by padding the loop body past 127 bytes.
\ Each `1 drop` is 3 bytes; we need ~50 iterations to push the back-jump
\ offset out of i8 range.  This exercises the i32 fallback in `until`.
: long-loop  ( -- )
    0
    begin
        1 drop  1 drop  1 drop  1 drop  1 drop
        1 drop  1 drop  1 drop  1 drop  1 drop
        1 drop  1 drop  1 drop  1 drop  1 drop
        1 drop  1 drop  1 drop  1 drop  1 drop
        1 drop  1 drop  1 drop  1 drop  1 drop
        1 drop  1 drop  1 drop  1 drop  1 drop
        1 drop  1 drop  1 drop  1 drop  1 drop
        1 drop  1 drop  1 drop  1 drop  1 drop
        1 +
        dup 10 =
    until
    drop ;

: main
    read-status
    bail-if-winning
    classify
    countdown
    long-loop
    \ Main game loop: forever read status + fire east at (5,3).
    \ Each iteration spends 2 ticks (one syscall per Status, one per Input).
    begin
        read-status
        5 3 0 1 move drop
    again ;

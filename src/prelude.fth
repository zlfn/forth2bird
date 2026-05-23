\ Prelude — stack manipulation words built on a scratch memory region.
\
\ Scratch layout: 16 bytes at the top of the code/data region, immediately
\ below the stack base (0x8000).  The stack would have to grow ~28 KB
\ downward... wait, this VM's stack grows UPWARD from 0x8000, so anything
\ below 0x8000 is untouched by the stack.  Code grows from 0x0000 upward;
\ reaching 0x7FF0 means a ~32 KB binary, which is well outside the regime
\ where bot authors actually live.  So 0x7FF0..0x7FFF is a safe scratch.
\
\   0x7FF0  S0
\   0x7FF4  S1
\   0x7FF8  S2
\   0x7FFC  (unused, reserved)
\
\ Convention: `!` is `( value addr -- )` (addr on top), `@` is `( addr -- value )`.

: swap ( a b -- b a )
    0x7FF0 !   0x7FF4 !
    0x7FF0 @   0x7FF4 @ ;

: nip ( a b -- b )
    0x7FF0 !   drop   0x7FF0 @ ;

: tuck ( a b -- b a b )
    swap   over ;

: rot ( a b c -- b c a )
    0x7FF0 !   0x7FF4 !   0x7FF8 !
    0x7FF4 @   0x7FF0 @   0x7FF8 @ ;

: -rot ( a b c -- c a b )
    rot   rot ;

: 2dup ( a b -- a b a b )
    over   over ;

: 2drop ( a b -- )
    drop   drop ;

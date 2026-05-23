\ samira_v2.fth — diagonal drift with periodic re-scan and big move.
\
\ Time budget per outer cycle (~201 ticks):
\   Each sub-cycle (50 ticks):
\     - read-status + scan-walls         (5 ticks)
\     - pick-diagonal: choose drift (drift-x, drift-y) from a wall-free
\       corner direction
\     - drift 45 ticks: each tick is a charge — move (drift-x, drift-y)
\       and fire in a cycling cardinal direction (cycles 0..3)
\   After 4 sub-cycles (=200 ticks), one big random move (1 tick).
\
\ The cycling fire-dir means each tick fires in a different cardinal,
\ so across the burst all 4 directions get sprayed even though the bot
\ is drifting in a single diagonal.

constant status-buf   0x9000
constant bounds-buf   0x9040
constant walls-buf    0x9050
constant FIRE         1

constant DRIFT-LEN    45            \ shots per sub-cycle (~45 ticks)
constant SUB-CYCLES   4             \ sub-cycles per big move (4 × 50 = 200)

variable wall-pX   variable wall-pY
variable wall-mX   variable wall-mY
variable drift-x                    \ ±1 — X component of current diagonal
variable drift-y                    \ ±1 — Y component
variable fire-dir                   \ 0..3 — cycles each shot

\ ── helpers ───────────────────────────────────────────────────────────────
: sx16  ( u32 -- i32 )  0xFFFF and  dup 0x8000 and if 0x10000 - then ;
: pack16  ( lo hi -- u32 )  0xFFFF and 16 lshift  swap 0xFFFF and  or ;

\ ── Status accessors ──────────────────────────────────────────────────────
: my-min-x   status-buf 0x0c + @           sx16 ;
: my-min-y   status-buf 0x0c + @ 16 rshift sx16 ;

\ ── syscalls ──────────────────────────────────────────────────────────────
: read-status   status-buf 1 1 syscall drop ;
: read-walls    bounds-buf walls-buf 2 3 syscall drop ;

\ ── 4-direction bounds (arag5 shape: 4×2 or 2×4 strip) ───────────────────
: bounds-pX
    my-min-x 1 +  my-min-y       pack16  bounds-buf !
    my-min-x 4 +  my-min-y 1 +   pack16  bounds-buf 4 + ! ;

: bounds-pY
    my-min-x      my-min-y 1 +   pack16  bounds-buf !
    my-min-x 1 +  my-min-y 4 +   pack16  bounds-buf 4 + ! ;

: bounds-mX
    my-min-x 3 -  my-min-y       pack16  bounds-buf !
    my-min-x      my-min-y 1 +   pack16  bounds-buf 4 + ! ;

: bounds-mY
    my-min-x      my-min-y 3 -   pack16  bounds-buf !
    my-min-x 1 +  my-min-y       pack16  bounds-buf 4 + ! ;

\ ── wall scan ─────────────────────────────────────────────────────────────
: scan-dir  ( -- bool )
    7 walls-buf !
    read-walls
    walls-buf @ 7 and  0 <> ;

: scan-walls
    bounds-pX scan-dir  wall-pX !
    bounds-pY scan-dir  wall-pY !
    bounds-mX scan-dir  wall-mX !
    bounds-mY scan-dir  wall-mY ! ;

\ Surrounded?  Reset all 4 flags so pick-diagonal/pick-move have options.
: maybe-unblock
    wall-pX @  wall-pY @  and
    wall-mX @  wall-mY @  and
    and
    if  0 wall-pX !  0 wall-pY !  0 wall-mX !  0 wall-mY !  then ;

\ ── choose diagonal whose two components are both wall-free ──────────────
: pick-diagonal
    wall-pX @ 0 =  wall-pY @ 0 =  and
    if   1 drift-x !   1 drift-y !  exit then
    wall-pX @ 0 =  wall-mY @ 0 =  and
    if   1 drift-x !  -1 drift-y !  exit then
    wall-mX @ 0 =  wall-pY @ 0 =  and
    if  -1 drift-x !   1 drift-y !  exit then
    wall-mX @ 0 =  wall-mY @ 0 =  and
    if  -1 drift-x !  -1 drift-y !  exit then
    1 drift-x !   1 drift-y ! ;

\ ── shot: charge along the diagonal + fire in cycling cardinal ───────────
\ Push order: triggers, dir (= fire-dir), y (= drift-y), x (= drift-x).
: do-shot
    FIRE  fire-dir @  drift-y @  drift-x @  4 2 syscall drop
    fire-dir @ 1 +  3 and  fire-dir ! ;

: drift-phase
    DRIFT-LEN 0 do  do-shot  loop ;

\ ── sub-cycle: scan + drift for ~50 ticks ────────────────────────────────
: sub-cycle
    read-status
    scan-walls
    maybe-unblock
    pick-diagonal
    drift-phase ;

\ ── big move (samira-style: random + CCW wall fallthrough) ───────────────
: move-+x   0 0 0       0x7FFF  4 2 syscall drop ;
: move-+y   0 2  0x7FFF 0       4 2 syscall drop ;
: move--x   0 1 0      -0x7FFF  4 2 syscall drop ;
: move--y   0 3 -0x7FFF 0       4 2 syscall drop ;

: move-rand-0
    wall-pX @ 0 = if move-+x exit then
    wall-pY @ 0 = if move-+y exit then
    wall-mX @ 0 = if move--x exit then
    wall-mY @ 0 = if move--y exit then
    move-+x ;

: move-rand-1
    wall-pY @ 0 = if move-+y exit then
    wall-mX @ 0 = if move--x exit then
    wall-mY @ 0 = if move--y exit then
    wall-pX @ 0 = if move-+x exit then
    move-+y ;

: move-rand-2
    wall-mX @ 0 = if move--x exit then
    wall-mY @ 0 = if move--y exit then
    wall-pX @ 0 = if move-+x exit then
    wall-pY @ 0 = if move-+y exit then
    move--x ;

: move-rand-3
    wall-mY @ 0 = if move--y exit then
    wall-pX @ 0 = if move-+x exit then
    wall-pY @ 0 = if move-+y exit then
    wall-mX @ 0 = if move--x exit then
    move--y ;

: pick-move
    read-status                       \ refresh random_number
    status-buf @ 3 and
    dup 0 = if drop move-rand-0 exit then
    dup 1 = if drop move-rand-1 exit then
    dup 2 = if drop move-rand-2 exit then
    drop  move-rand-3 ;

\ ── main ──────────────────────────────────────────────────────────────────
: main
    begin
        SUB-CYCLES 0 do  sub-cycle  loop
        pick-move
    again ;

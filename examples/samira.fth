\ aragport.fth — Forth port of arag5__v02.bin.
\
\ Strategy: stay put and spray 100 shots in directions without walls, then
\ pick a random escape direction and move far.  Repeat.
\
\ Per cycle (~107 ticks):
\   1. read-status                                              (1 tick)
\   2. ReadWalls in 4 directions, set wall-pX/pY/mX/mY flags    (4 ticks)
\      - if all 4 walls are present, clear flags (surrounded → spray anyway)
\   3. Fire burst: fire in each open direction, repeat until 100 shots fired
\   4. read-status (refresh random_number), pick dir = rand & 3 (1 tick)
\   5. Move 32767 cells in that direction; if blocked, try the next CCW
\      direction, finally fall back to the starting one                 (1 tick)

constant status-buf   0x9000
constant bounds-buf   0x9040
constant walls-buf    0x9050
constant FIRE         1
constant FIRE-COUNT   100

variable wall-pX   variable wall-pY
variable wall-mX   variable wall-mY
variable shot-counter

\ ── helpers ───────────────────────────────────────────────────────────────
: sx16  ( u32 -- i32 )  0xFFFF and  dup 0x8000 and if 0x10000 - then ;
: pack16  ( lo hi -- u32 )  0xFFFF and 16 lshift  swap 0xFFFF and  or ;

\ ── Status accessors ──────────────────────────────────────────────────────
: my-min-x   status-buf 0x0c + @           sx16 ;
: my-min-y   status-buf 0x0c + @ 16 rshift sx16 ;

\ ── syscalls ──────────────────────────────────────────────────────────────
: read-status   status-buf 1 1 syscall drop ;
: read-walls    bounds-buf walls-buf 2 3 syscall drop ;

\ ── 4-direction bounds setup ──────────────────────────────────────────────
\ Each strip's low 3 bits (cells nearest the player on the queried axis)
\ tell us whether the immediate path is blocked.  Same shape as arag5:
\   +X / -X : 4-wide, 2-tall strip
\   +Y / -Y : 2-wide, 4-tall strip

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
\ Pre-seed walls-buf with 7 so a failed ReadWalls (out-of-bounds region at
\ the map edge) defaults to "wall present", preventing fire that direction.
: scan-dir  ( -- bool )
    7 walls-buf !
    read-walls
    walls-buf @ 7 and  0 <> ;

: scan-walls
    bounds-pX scan-dir  wall-pX !
    bounds-pY scan-dir  wall-pY !
    bounds-mX scan-dir  wall-mX !
    bounds-mY scan-dir  wall-mY ! ;

\ Surrounded?  Reset all 4 flags so fire-burst doesn't infinite-loop.
: maybe-unblock
    wall-pX @  wall-pY @  and
    wall-mX @  wall-mY @  and
    and
    if  0 wall-pX !  0 wall-pY !  0 wall-mX !  0 wall-mY !  then ;

\ ── stationary fires ──────────────────────────────────────────────────────
: fire-+x   FIRE 0 0 0  4 2 syscall drop ;
: fire-+y   FIRE 2 0 0  4 2 syscall drop ;
: fire--x   FIRE 1 0 0  4 2 syscall drop ;
: fire--y   FIRE 3 0 0  4 2 syscall drop ;

\ ── long-distance moves (x or y = ±32767) ─────────────────────────────────
: move-+x   0 0 0       0x7FFF  4 2 syscall drop ;
: move-+y   0 2  0x7FFF 0       4 2 syscall drop ;
: move--x   0 1 0      -0x7FFF  4 2 syscall drop ;
: move--y   0 3 -0x7FFF 0       4 2 syscall drop ;

\ ── fire phase ────────────────────────────────────────────────────────────
: bump-counter   shot-counter @ 1 +  shot-counter ! ;

: fire-pass
    wall-pX @ 0 = if fire-+x bump-counter then
    wall-pY @ 0 = if fire-+y bump-counter then
    wall-mX @ 0 = if fire--x bump-counter then
    wall-mY @ 0 = if fire--y bump-counter then ;

: fire-burst
    begin
        fire-pass
        shot-counter @ FIRE-COUNT >=
    until ;

\ ── move phase: random starting direction, CCW fall-through ───────────────

: move-rand-0           \ +X first
    wall-pX @ 0 = if move-+x exit then
    wall-pY @ 0 = if move-+y exit then
    wall-mX @ 0 = if move--x exit then
    wall-mY @ 0 = if move--y exit then
    move-+x ;

: move-rand-1           \ +Y first
    wall-pY @ 0 = if move-+y exit then
    wall-mX @ 0 = if move--x exit then
    wall-mY @ 0 = if move--y exit then
    wall-pX @ 0 = if move-+x exit then
    move-+y ;

: move-rand-2           \ -X first
    wall-mX @ 0 = if move--x exit then
    wall-mY @ 0 = if move--y exit then
    wall-pX @ 0 = if move-+x exit then
    wall-pY @ 0 = if move-+y exit then
    move--x ;

: move-rand-3           \ -Y first
    wall-mY @ 0 = if move--y exit then
    wall-pX @ 0 = if move-+x exit then
    wall-pY @ 0 = if move-+y exit then
    wall-mX @ 0 = if move--x exit then
    move--y ;

: pick-move
    read-status                       \ refresh random_number for fresh choice
    status-buf @ 3 and                \ random_number & 3 → 0..3
    dup 0 = if drop move-rand-0 exit then
    dup 1 = if drop move-rand-1 exit then
    dup 2 = if drop move-rand-2 exit then
    drop  move-rand-3 ;

\ ── main ──────────────────────────────────────────────────────────────────
: main
    begin
        read-status
        scan-walls
        maybe-unblock
        fire-burst
        pick-move
        0 shot-counter !
    again ;

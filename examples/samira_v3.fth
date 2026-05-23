\ samira_v3.fth — samira + ReadPlayers-weighted fire.
\
\ Like samira (scan walls → spray burst → big move) but adds a player
\ scan after the wall scan.  For each non-self enemy that's row- or
\ column-aligned with us, increment a per-direction counter.  In the
\ fire burst, each cardinal fires max(enemy_count, 1) times per pass —
\ so the side with the most aligned enemies gets the most bullets.
\
\ Per cycle (~108 ticks):
\   1. read-status                                              (1 tick)
\   2. ReadWalls × 4 → wall flags                               (4 ticks)
\   3. ReadPlayers → enemy counts per direction                 (1 tick)
\   4. Fire burst: each pass fires max(count, 1) shots per open
\      direction; loop until 100 total shots
\   5. pick-move: samira-style random + CCW wall fallthrough    (1 tick)
\
\ NB: ReadWalls is sysnum 4, ReadPlayers is sysnum 5 in this build of
\ the driver.

constant status-buf   0x9000
constant bounds-buf   0x9040
constant walls-buf    0x9050
constant pcount-buf   0x9060
constant players-buf  0x9070      \ 0x9070..0x9270 (32 × 16 bytes)
constant max-players  32
constant scan-radius  120         \ for ReadPlayers (245-wide box, < 256 limit)
constant FIRE         1
constant FIRE-COUNT   100

variable wall-pX   variable wall-pY
variable wall-mX   variable wall-mY

variable enemy-pX  variable enemy-pY
variable enemy-mX  variable enemy-mY

variable shot-counter

\ ── helpers ───────────────────────────────────────────────────────────────
: sx16  ( u32 -- i32 )  0xFFFF and  dup 0x8000 and if 0x10000 - then ;
: pack16  ( lo hi -- u32 )  0xFFFF and 16 lshift  swap 0xFFFF and  or ;
: max  ( a b -- m )   2dup < if swap then drop ;
: min  ( a b -- m )   2dup > if swap then drop ;

\ ── Status accessors ──────────────────────────────────────────────────────
: my-min-x   status-buf 0x0c + @           sx16 ;
: my-min-y   status-buf 0x0c + @ 16 rshift sx16 ;
: my-max-x   status-buf 0x10 + @           sx16 ;
: my-max-y   status-buf 0x10 + @ 16 rshift sx16 ;
: map-min-x  status-buf 0x04 + @           sx16 ;
: map-min-y  status-buf 0x04 + @ 16 rshift sx16 ;
: map-max-x  status-buf 0x08 + @           sx16 ;
: map-max-y  status-buf 0x08 + @ 16 rshift sx16 ;

\ ── Player record accessors ───────────────────────────────────────────────
: p-min-x      @ sx16 ;
: p-min-y      @ 16 rshift sx16 ;
: p-max-x  4 + @ sx16 ;
: p-max-y  4 + @ 16 rshift sx16 ;

\ ── syscalls ──────────────────────────────────────────────────────────────
: read-status   status-buf 1 1 syscall drop ;
\ ReadWalls is sysnum 4 in this driver build.
: read-walls    bounds-buf walls-buf 2 4 syscall drop ;
\ ReadPlayers is sysnum 5 in this driver build.
: read-players  bounds-buf players-buf pcount-buf 3 5 syscall drop ;

\ ── 4-direction wall bounds (arag5 shape) ────────────────────────────────
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

: maybe-unblock
    wall-pX @  wall-pY @  and
    wall-mX @  wall-mY @  and
    and
    if  0 wall-pX !  0 wall-pY !  0 wall-mX !  0 wall-mY !  then ;

\ ── player scan: classify aligned enemies per direction ──────────────────

\ Big bounding box around me, clamped to map.
: set-player-bounds
    my-min-x scan-radius -  map-min-x max
    my-min-y scan-radius -  map-min-y max
    pack16  bounds-buf !
    my-max-x scan-radius +  map-max-x min
    my-max-y scan-radius +  map-max-y min
    pack16  bounds-buf 4 + ! ;

: clear-enemy-counts
    0 enemy-pX !  0 enemy-pY !
    0 enemy-mX !  0 enemy-mY ! ;

\ One player: skip if it's us, then bump the right side's counters.
\ A single enemy may be both row- and column-aligned (rare; touching us).
: classify-player  ( idx -- )
    4 lshift  players-buf +              ( addr )
    \ Skip self: identical min corner.
    dup  p-min-x my-min-x =
    over p-min-y my-min-y =
    and  if drop exit then
    \ Row aligned? y ranges overlap → +X or -X bucket.
    dup  p-max-y my-min-y >=
    over p-min-y my-max-y <=
    and  if
        dup p-min-x my-max-x >
        if    enemy-pX @ 1 + enemy-pX !
        else  enemy-mX @ 1 + enemy-mX !
        then
    then
    \ Col aligned? x ranges overlap → +Y or -Y bucket.
    dup  p-max-x my-min-x >=
    over p-min-x my-max-x <=
    and  if
        dup p-min-y my-max-y >
        if    enemy-pY @ 1 + enemy-pY !
        else  enemy-mY @ 1 + enemy-mY !
        then
    then
    drop ;

: scan-players
    clear-enemy-counts
    max-players pcount-buf !
    set-player-bounds
    read-players
    pcount-buf @  dup 0 = if drop exit then
    0 do  i classify-player  loop ;

\ ── stationary fires ──────────────────────────────────────────────────────
: fire-+x   FIRE 0 0 0  4 2 syscall drop ;
: fire-+y   FIRE 2 0 0  4 2 syscall drop ;
: fire--x   FIRE 1 0 0  4 2 syscall drop ;
: fire--y   FIRE 3 0 0  4 2 syscall drop ;

: bump-counter   shot-counter @ 1 +  shot-counter ! ;

\ Fire one direction N times.  Inputs are guarded to N≥1 by callers
\ (max with 1), so the off-by-one in do/loop with N=0 never triggers.
: fire-+x-n  ( n -- )   0 do  fire-+x bump-counter  loop ;
: fire-+y-n  ( n -- )   0 do  fire-+y bump-counter  loop ;
: fire--x-n  ( n -- )   0 do  fire--x bump-counter  loop ;
: fire--y-n  ( n -- )   0 do  fire--y bump-counter  loop ;

\ Long-distance moves for pick-move.
: move-+x   0 0 0       0x7FFF  4 2 syscall drop ;
: move-+y   0 2  0x7FFF 0       4 2 syscall drop ;
: move--x   0 1 0      -0x7FFF  4 2 syscall drop ;
: move--y   0 3 -0x7FFF 0       4 2 syscall drop ;

\ ── weighted fire pass ────────────────────────────────────────────────────
\ For each open direction, fire max(enemy_count, 1) shots.
\ - 0 enemies aligned: 1 shot (samira default — keeps the open direction
\   under pressure even when no specific target is in line).
\ - N enemies aligned: N shots, biasing the burst toward dense lanes.
: fire-pass
    wall-pX @ 0 = if  enemy-pX @ 1 max  fire-+x-n  then
    wall-pY @ 0 = if  enemy-pY @ 1 max  fire-+y-n  then
    wall-mX @ 0 = if  enemy-mX @ 1 max  fire--x-n  then
    wall-mY @ 0 = if  enemy-mY @ 1 max  fire--y-n  then ;

: fire-burst
    begin
        fire-pass
        shot-counter @ FIRE-COUNT >=
    until ;

\ ── big move (samira-style: random + CCW wall fallthrough) ───────────────
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
        read-status
        scan-walls
        maybe-unblock
        scan-players                  \ +1 tick over samira; populates buckets
        fire-burst                    \ now weighted by enemy-* counters
        pick-move
        0 shot-counter !
    again ;

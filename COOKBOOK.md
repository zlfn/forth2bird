# Cookbook

Recipes for common patterns in this Forth dialect. Every snippet is extracted
from the test suite — search `tests/run.rs` for the named test to see it
actually executed and asserted.

Read [LANGUAGE.md](LANGUAGE.md) first for the token reference.

---

## Bot skeleton

The simplest bot main loop. Each `syscall` yields one tick. Syscall
numbers shift between phases — replace `STATUS-NUM` with the current
number from the live VM spec.

```forth
: read-status   ( buf -- )  1 STATUS-NUM syscall drop ;
: act           \ decide based on status, then fire/move
                ... ;

: main
    begin
        status-buf read-status
        act
    again ;
```

## Position pack/unpack (Input syscall return value)

The Input syscall returns `(x & 0xFFFF) | (y << 16)`, with **x and y both
i16**. Unpacking requires sign extension on each half — this is the single
most error-prone step in working with positions.

```forth
: sign-ext16    dup 0x8000 and if 0xFFFF0000 or then ;
: unpack-x      0xFFFF and  sign-ext16 ;
: unpack-y      16 rshift   sign-ext16 ;

: pack-pos      16 lshift  swap  0xFFFF and  or ;     \ x y -- packed
```

After the Input syscall returns its packed position:

```forth
\ … your Input syscall call here …
dup unpack-x  px !                \ stash x
unpack-y      py !                \ stash y
```

(`position_pack_unpack_round_trip`, `position_unpack_y_sign_extends_negative`,
`i16_sign_extension_via_high_bit_check`)

## Clamp to bounds

Keep a value inside a min/max range — useful for clamping a move target
to `map_extent`.

```forth
: clamp-x
    dup MAP_MIN_X < if drop MAP_MIN_X exit then
    dup MAP_MAX_X > if drop MAP_MAX_X exit then ;
```

(`clamp_value_into_bounds`)

## Pseudo-array via consecutive variables

`variable` declarations get consecutive 4-byte slots, so you can treat a
block of them as an i32 array:

```forth
variable a0 variable a1 variable a2 variable a3 variable a4
: read   4 * a0 + @ ;       \ idx -- value
: write  4 * a0 + ! ;       \ value idx --

\ Sum elements:
: sum-all
    0
    5 0 do  i read  +  loop ;
```

This is the foundation for ring buffers, lookup tables, and small arrays.

(`array_sum_via_computed_address`)

## Linear search with sentinel

Return the index of the first match, or `-1` if absent.

```forth
: find  ( target -- index_or_-1 )
    5 0 do
        dup  a0 i 4 * + @  =
        if drop i exit then       \ drop target, push idx, exit word
    loop
    drop -1 ;
```

The trick is `drop i exit` inside the matching branch: drop the spare copy
of the target, push the index, then `exit` leaves the entire word cleanly.

(`linear_search_finds_target_index`)

## Argmin / argmax — "best target so far"

```forth
variable best-d variable best-i
: argmin
    d0 @ best-d !  0 best-i !
    4 1 do
        i read  dup best-d @ <
        if   best-d !  i best-i !
        else drop
        then
    loop
    best-i @ ;
```

`if dup VAR ! then` is the canonical "update only when extreme" pattern.

(`argmin_of_four_distances_picks_closest`, `argmax_of_four_values_picks_largest_index`)

## Ring buffer (FIFO queue)

```forth
variable q0 variable q1 variable q2 variable q3 variable q4
variable head variable tail variable count

: enqueue
    tail @  4 * q0 + !
    tail @ 1 + 5 mod tail !
    count @ 1 + count ! ;

: dequeue
    head @  4 * q0 + @
    head @ 1 + 5 mod head !
    count @ 1 - count ! ;
```

(`ring_buffer_queue_preserves_fifo_order`)

## Cooldown timer

Fire once every N ticks. Useful for weapon cadence, periodic scans, etc.

```forth
variable cooldown
: tick
    cooldown @ 1 - cooldown !
    cooldown @ 0 = if
        fire
        3 cooldown !            \ reset
    then ;
```

(`cooldown_timer_fires_every_n_ticks`)

## State machine

```forth
constant SCAN 0
constant MOVE 1
constant FIRE 2

variable state
: advance   state @ 1 + 3 mod state ! ;
: tick      advance  do-state-action ;
```

For dispatching on the current state, chain `if ... then`:

```forth
: do-state-action
    state @ SCAN = if scan-action  exit then
    state @ MOVE = if move-action  exit then
    fire-action ;
```

(`state_machine_counts_visits_to_target_state`)

## Safe recursion with `do`/`loop`

The `do`/`loop` scratch slots are **global** at compile-time depth 0. If a
word A's `do`/`loop` body calls a word B that also has its own `do`/`loop`
at the top level of its body, B overwrites A's loop index/limit and chaos
ensues.

**SAFE**: complete the loop before recursing or calling another loop word.

```forth
: f
    dup 0 = if drop exit then
    3 0 do  i counter @ +  counter !  loop     \ loop finishes
    1 - f ;                                     \ now safe to recurse
```

**UNSAFE** — outer index gets clobbered on the recursive entry:

```forth
: g
    3 0 do
        i 0 > if g then                         \ DON'T
    loop ;
```

(`recursive_word_with_do_loop_before_self_call`)

## Tail-recursive accumulator

The compiler doesn't optimize tail calls (each call adds a return-stack
frame), but the retstack holds ~1000 frames so depths up to several hundred
are fine.

```forth
: tail-sum  ( acc n -- result )
    dup 0 = if drop exit then
    swap over + swap 1 -
    tail-sum ;

\ usage:  0 100 tail-sum  → 5050
```

(`tail_recursive_sum_one_to_hundred`)

## Multi-way switch via chained `if`s

Each branch short-circuits with `exit`. The final case is the unconditional
fall-through.

```forth
: classify
    dup 0 <  if drop -1 exit then
    dup 0 =  if drop  0 exit then
    dup 10 < if drop  1 exit then
    drop  2 ;
```

(`chained_ifs_classify_negative_zero_small_large`)

## Multi-value returns

A word may leave several values; the caller drains TOS-first.

```forth
: div-mod   2dup / -rot mod ;       \ ( a b -- q r )

\ caller:
17 5 div-mod
rem !  q !                          \ rem gets 2 (TOS), then q gets 3
```

If you want a different binding order, `swap` before storing.

(`word_returns_quotient_and_remainder`)

## Conditional swap (min/max of two)

```forth
: sort2  2dup >  if swap then ;     \ ( a b -- min max )
```

(`conditional_swap_orders_top_two`)

## Numeric algorithms

### GCD (Euclidean)

```forth
: gcd
    dup 0 = if drop exit then
    swap over mod
    gcd ;
```

(`gcd_recursive_euclidean`)

### Integer sqrt (Newton's method)

```forth
: isqrt
    dup 0 = if exit then
    dup                                  \ initial guess = n
    begin
        2dup / over + 2 /                \ new = (guess + n/guess) / 2
        over over =  if nip nip exit then    \ converged
        over over <  if drop nip exit then   \ oscillation → return guess
        nip
    again ;
```

(`integer_sqrt_via_newtons_method`)

### Integer log2

```forth
: log2
    0 swap
    begin
        dup 1 <= if drop exit then
        1 rshift  swap 1 + swap
    again ;
```

(`integer_log2_for_powers_of_two`)

### Manhattan distance

Needs scratch variables because the four args (`x1 y1 x2 y2`) are
interleaved on the stack — pulling out absolute differences would otherwise
require painful stack juggling.

```forth
variable x1v variable y1v variable x2v variable y2v

: abs  dup 0 < if negate then ;

: manhattan       \ ( x1 y1 x2 y2 -- d )
    y2v ! x2v ! y1v ! x1v !
    x1v @ x2v @ - abs
    y1v @ y2v @ - abs
    + ;
```

(`manhattan_distance_via_variables`)

## Bit twiddling

### popcount — Brian Kernighan

```forth
: popcount
    0
    begin
        over 0 = if nip exit then
        swap  dup 1 - and  swap          \ clear lowest set bit
        1 +
    again ;
```

(`brian_kernighan_popcount_matches_naive`)

### Parity — XOR fold

```forth
: parity
    dup 4 rshift xor
    dup 2 rshift xor
    dup 1 rshift xor
    1 and ;
```

(`parity_via_xor_fold`)

### Byte swap (32-bit)

```forth
: byte-swap32
    dup  0xFF       and  24 lshift
    over 0xFF00     and   8 lshift  or
    over 0xFF0000   and   8 rshift  or
    swap 0xFF000000 and  24 rshift  or ;
```

(`byte_swap_32bit_reverses_byte_order`)

### Power-of-two check

```forth
: pow2?  dup 1 - and  0 = ;        \ ( n -- bool )
```

(`power_of_two_detection`)

### Hamming distance

```forth
: hamming           \ ( a b -- distance )
    xor
    popcount ;
```

(`hamming_distance_via_xor_then_popcount`)

## Lookup table (precomputed)

16 consecutive vars hold popcount(0..15). Use it to compute popcount(byte)
in two table lookups + one add.

```forth
variable t0 variable t1 variable t2 variable t3
variable t4 variable t5 variable t6 variable t7
variable t8 variable t9 variable tA variable tB
variable tC variable tD variable tE variable tF

: lookup  4 * t0 + @ ;

: init-table
    0 t0 !  1 t1 !  1 t2 !  2 t3 !
    1 t4 !  2 t5 !  2 t6 !  3 t7 !
    1 t8 !  2 t9 !  2 tA !  3 tB !
    2 tC !  3 tD !  3 tE !  4 tF ! ;

: byte-popcount         \ ( n -- count )
    dup 0xF and  lookup
    swap 4 rshift 0xF and  lookup
    + ;
```

(`popcount_via_4bit_lookup_table`)

## Function composition

A sub-word leaves its result on the stack; the caller folds it into a
further computation.

```forth
: square  dup * ;
: cube    dup square * ;          \ 3 cube → 27
```

(`function_composition_via_chained_words`)

## Mutual recursion

The compiler resolves forward references between words automatically, so
`even?`/`odd?` and longer chains compile in one pass.

```forth
: even?   dup 0 = if drop 1 exit then  1 - odd? ;
: odd?    dup 0 = if drop 0 exit then  1 - even? ;
```

3-way and longer chains work the same way.

(`mutual_recursion_even_odd`, `three_way_mutual_recursion_cycles_correctly`)

---

## Common pitfalls

- **Forgetting the trailing `;`.** Every `:` needs one.
- **Putting `variable r` after `: main ... r ! ;`.** Move it before.
- **Expecting `1+`, `2*`, `?dup`, `min`, `max`, `abs`.** Write them yourself
  or use `1 +`, `1 lshift`, etc.
- **Using `'A'` for a char literal.** Doesn't parse; write `65`.
- **Using `>r`/`r>` for temporary storage.** Use a `variable` instead.
- **Trying `j` for the outer loop index.** Save outer `i` to a `variable`
  before entering the inner loop.
- **Assuming `true = -1`.** Booleans are 0/1.
- **Dropping when nothing's on the stack.** Trace the stack carefully —
  underflow panics in tests, may halt silently in the real VM.
- **Calling a do-loop word from inside another do-loop body.** Scratch
  slot conflict; complete the outer loop first, or use `begin`/`until`
  with a counter variable instead.
- **Forgetting to sign-extend `unpack-x`/`unpack-y`.** A position
  component in the range `0x8000..0xFFFF` will read as a large positive
  i32 instead of a small negative i16.

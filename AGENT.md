# Agent guide — writing LiveCTF bots in Forth

You are an AI agent tasked with writing a bot for the [LiveCTF Combat Drone
Bird Training Program](https://play.livectf.com/). Your output is a `.fth`
source file that this directory's compiler (`fthc`) turns into raw bytecode
for the game's stack-machine VM. This file gets you producing a runnable bot
fast, then points to the deeper references.

## Read-this-first

1. **`LANGUAGE.md`** — full token reference, what's missing vs standard
   Forth, and **14 documented gotchas** that bite newcomers. Skim the
   gotchas list before writing your first definition.
2. **`COOKBOOK.md`** — verified idioms (position pack/unpack, clamp, ring
   buffer, argmin, popcount, …). Every snippet has a matching test in
   `tests/run.rs` — copy-paste freely.
3. **[LiveCTF VM spec](https://play.livectf.com/docs)** — opcodes,
   syscall numbers, packed-position layout, memory map.

You don't need to read the Rust compiler source to write a bot.

## The dialect, in 60 seconds

This is **not** ANS Forth. It's a deliberate subset with:

- `:` … `;` definitions, `variable`, `constant`
- `if`/`else`/`then`, `begin`/`until`, `begin`/`again`, `do`/`loop` (with
  `i` for the innermost index — **no `j`**), `exit` for early return
- 4-byte memory: `@` `!` only (no `c@`/`c!`)
- Prelude stack ops: `swap nip tuck rot -rot 2dup 2drop` (plus inlined
  `dup over drop`)
- Standard arithmetic / bitwise / comparison; non-commutative ops
  (`/` `mod` `lshift` `rshift` `<` `>` `<=` `>=` `-`) follow Forth direction
- `syscall` emits opcode 0x0E; you push args + argc + sysnum yourself
- **No** `>r` `r>` `leave` `?do` `+loop` `recurse` `."` `s"` `c@` `c!`
  `1+` `2*` `min` `max` `abs` `?dup` `pick` `roll` `create … does>`,
  char literals, or string literals

Booleans are **0/1** (not 0/-1). DIV/REM by zero returns **0** (per spec,
not error). Memory addresses wrap mod 64 KB.

## Workflow

1. **Write** `bots/mybot.fth`. Put `: main … ;` as the entry point.
2. **Compile**: `cargo run --bin fthc bots/mybot.fth bots/mybot.bin`
3. **Disassemble to sanity-check**: `cargo run --bin fthd bots/mybot.bin`
4. **Run locally** (optional): use the `driver` from the game tarball with
   your `.bin` in `bots/` — see the upstream README on play.livectf.com.
5. **Upload** `bots/mybot.bin` on the LiveCTF Bots page.

Working examples live in `examples/` — they're real bots that have been
deployed. Read `samira_v3.fth` for a complete worked example with status
parsing, wall queries, target selection, and a main loop.

## Skeleton

```forth
\ Buffers in data-stack-safe high memory (above your code, below the stack).
constant status-buf  0x9000
constant bounds-buf  0x9040

\ Constants for triggers (see spec for the bit layout).
constant FIRE        1

\ State.
variable cooldown variable mode

\ Syscalls — wrap them so the call sites read naturally.
: read-status   status-buf 1 1 syscall drop ;
: fire          ( x y dir -- )  FIRE 4 2 syscall drop ;

\ Per-tick logic.
: tick
    read-status
    \ … decide based on status buffer …
    0 0 0 fire ;        \ shoot east at origin (placeholder)

: main  begin tick again ;
```

Notice:

- **`variable` declarations come BEFORE `: main`**. Forward references
  resolve to words only — referring to a variable before it's defined
  fails at compile time with a specific error.
- The infinite `begin … again` loop is the canonical bot shape. The game
  yields your bot when a syscall is invoked; you implicitly halt when the
  game tells you to.

## Syscall recipes

Every syscall pushes args in **reverse order** (first arg ends up on TOS),
then `argc`, then sysnum, then `syscall`. Returns one i32 on TOS.

| Syscall | Wrapper |
|---------|---------|
| 0 Debug | `: dbg  ( vals… count -- )  0 syscall drop ;` |
| 1 Status | `: read-status  ( buf -- )  1 1 syscall drop ;` |
| 2 Input | `: fire  ( x y dir triggers -- )  4 2 syscall drop ;` |
| 3 ReadWalls | `: read-walls  ( bounds-addr result-addr -- )  2 3 syscall drop ;` |
| 4 ReadPlayers | `: read-players  ( bounds-addr result-addr result-count-addr -- )  3 4 syscall drop ;` |

The Input syscall (2) returns a **packed position** `(x & 0xFFFF) | (y << 16)`,
both halves i16. To unpack to i32:

```forth
: sign-ext16  dup 0x8000 and if 0xFFFF0000 or then ;
: unpack-x    0xFFFF and  sign-ext16 ;
: unpack-y    16 rshift   sign-ext16 ;
```

Forgetting `sign-ext16` is the #1 newcomer bug.

**One syscall per tick.** Plan your tick budget — a bot that fires plus
reads status plus reads walls uses 3 ticks. The game runs 3600 ticks
per round.

## Five rules you'll hit if you write fluently

1. **`variable r` BEFORE `: main … r ! ;`** — not after. Forward refs are
   word-only.
2. **`exit` leaves the WHOLE word**, not just the loop. There is no `leave`.
3. **No `j` for outer-loop index** in nested `do/loop`. Save outer `i` to a
   `variable` before entering the inner loop.
4. **Avoid `do/loop` inside `do/loop` via a sub-word.** The scratch slot is
   shared (compile-time depth, not runtime). Either complete the loop
   before calling, or replace the inner loop with `begin`/`until` + a
   counter variable.
5. **Drain order**: `7 9 r ! q !` stores **9** at `r` (TOS first), then 7
   at `q`. If you want the opposite, `swap` or `-rot` first.

The full list of 14 gotchas is in `LANGUAGE.md`.

## Verifying behavior before you upload

The test suite (`cargo test`) compiles and runs **267+ unit tests** covering
the compiler and the VM spec. If you discover a Forth pattern that
surprised you, write the test the same way `tests/run.rs` does:

```rust
let src = "
    variable r
    : my-word  /* … */ ;
    : main  /* … */  r ! ;
";
assert_eq!(get_var(src, "r").unwrap(), expected);
```

Then `cargo test --test run my_word_name` runs just yours.

If you're writing a one-off algorithm (popcount, isqrt, …), check
`COOKBOOK.md` first — there's likely already a verified version.

## When in doubt

- **"How do I X?"** — search `COOKBOOK.md`; if not there, `tests/run.rs`.
- **"Does the dialect have X?"** — `LANGUAGE.md` § "What standard Forth has
  that this dialect doesn't".
- **"My bytecode looks weird"** — `cargo run --bin fthd mybot.bin` shows
  labeled disassembly with conditional-jump pattern recognition.
- **"What does opcode 0x?? do?"** — VM spec at play.livectf.com/docs.

You generally don't need to modify the Rust compiler to ship a bot. If you
do (because of a missing language feature), read the compiler's structure
in `src/compiler.rs` — it's organized into `impl Compiler` blocks by
responsibility (low-level emit, preamble, primitives, control flow,
top-level dispatch, finalization). Tests in `tests/compile.rs` lock the
bytecode shape; tests in `tests/run.rs` lock the runtime semantics. Run
`cargo test` after any change.

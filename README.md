# livectf-forth

Forth-style compiler (`fthc`) and disassembler (`fthd`) for the
[LiveCTF](https://play.livectf.com/) VM — a stack machine with a tiny custom
bytecode ([docs](https://play.livectf.com/docs)).

## Build

```sh
cargo build --release
```

Produces two binaries under `target/release/`:

- `fthc` — compiles a `.fth` source file to raw bot bytecode
- `fthd` — disassembles a `.bin` bot back to a labeled listing

## Compile a bot

```sh
./target/release/fthc examples/demo.fth bot.bin
```

Upload `bot.bin` on the LiveCTF Bots page, or run it locally with the
`driver` executable from the game tarball.

## Disassemble a bot

```sh
./target/release/fthd bot.bin
./target/release/fthd path/to/some/other.bin > listing.asm
```

Output includes labels at every call/jump target, immediate values in hex
and decimal, and conditional-jump annotations for the Forth `if`/`until`
idiom (`PUSH X; MUL; JUMP_REL`).

`examples/wander.asm` and `examples/square.asm` are reference dumps of the
two NPC bots that ship with the LiveCTF tarball — good for seeing what
hand-tuned bot bytecode looks like.

## Forth dialect cheat sheet

This is a **subset** of standard Forth — see [LANGUAGE.md](LANGUAGE.md) for
the precise contract and the missing-word list, and [COOKBOOK.md](COOKBOOK.md)
for canonical idioms.

```forth
\ Literals
42 -7 0xFF        \ pushed onto stack (smallest encoding chosen)

\ Definitions
: square ( n -- n² )  dup * ;

\ Top-level data
variable counter      \ allocates 4 bytes; pushes its address
constant SCRATCH 0x7FF0

\ Arithmetic / bit ops
+ - * / mod   and or xor   lshift rshift
not invert negate

\ Comparisons (each pops 2, pushes 0/1)
<  <=  >  >=  =  <>

\ Stack manipulation (from prelude)
dup drop over swap nip tuck rot -rot 2dup 2drop

\ Memory
@      \ ( addr -- value )       load 4 bytes
!      \ ( value addr -- )       store 4 bytes

\ Syscalls (push args reverse, then argc, then sysnum)
\   Status:  result-addr 1 1 syscall
\   Input:   x y dir triggers 4 2 syscall
syscall

\ Control flow
cond if … then
cond if … else … then
begin … cond until        \ loops while cond is falsy
begin … again             \ infinite loop
limit start do … loop     \ counted loop; `i` reads the innermost index
exit                      \ early return from a word

\ Misc primitives
halt skip
```

`main` is the entry point — bootstrap calls it and halts on return.

## Layout

```
src/main.rs         compiler driver (fthc)
src/bin/fthd.rs     disassembler driver (fthd)
src/lib.rs          shared opcode constants
src/prelude.fth     built-in stack words (swap, rot, 2dup, ...)
examples/demo.fth   example bot exercising every compiler feature
examples/*.asm      reference disassembly of the NPC bots
```

## Memory map (this compiler's convention)

```
0x0000..0x0011    bootstrap (init retstack pointer, call main, halt)
0x0012..0x0031    PROLOG_HELPER (32 B; shared trampoline for `:` entry)
0x0032..0x0045    EPILOG_HELPER (20 B; shared trampoline for `;`/`exit`)
0x0046..< 0x7000  user code + variables (max ~28 KB)
0x7000..0x7FC4    return stack (grows down, ~1010 frames)
0x7FC8..0x7FCB    PROLOG_SCRATCH (helper temp)
0x7FCC..0x7FCF    return-stack pointer storage
0x7FD0..0x7FEF    do/loop scratch (4 nesting levels × 8 bytes)
0x7FF0..0x7FFF    prelude scratch (swap/nip/rot temps)
0x8000..0xFFFF    data stack (grows up from 0x8000, per VM spec)
```

The 64 KB address space is flat with no protection; the layout above is
the compiler's convention. Per-`:` overhead is 12 bytes (6-byte prolog +
6-byte epilog thunks calling the shared helpers); the helpers themselves
are emitted once in the preamble.

## Further reading

- **[AGENT.md](AGENT.md)** — fast onboarding for AI agents writing bots:
  workflow, syscall recipes, five rules-of-thumb, where to look things up.
  Start here if you're writing your first bot.
- **[LANGUAGE.md](LANGUAGE.md)** — complete reference for every supported
  token, semantics, what's missing vs. standard Forth, and the full
  gotchas list.
- **[COOKBOOK.md](COOKBOOK.md)** — verified idioms extracted from the test
  suite: position pack/unpack, pseudo-arrays, ring buffers, argmin/argmax,
  bit twiddling, numeric algorithms, etc.
- **[LiveCTF VM spec](https://play.livectf.com/docs)** — opcode table,
  syscall numbers and arg layouts, memory layout.

## Status

Stable. ~200 unit and integration tests cover the compiler and the spec.

Working: stack ops, arithmetic, comparisons, syscalls, `:`/`;`,
`variable`/`constant`, `if`/`else`/`then`, `begin`/`until`,
`begin`/`again`, `do`/`loop` (with `i`, max 4 nesting levels), `exit`,
forward references between words, automatic short/long jump encoding,
auto-emitted prolog/epilog calling convention with shared trampoline
helpers, return stack for deep / mutual recursion, disassembler with
labels and conditional-jump pattern recognition.

Not yet (see LANGUAGE.md for the full gap list): `+loop`/`leave`/`?do`,
`j` (outer-loop index), return-stack manipulation (`>r`/`r>`/`r@`),
strings / char literals, byte-level memory (`c@`/`c!`), convenience math
words (`1+`, `2*`, `min`, `max`, `abs`, `?dup`), compile-time arithmetic,
named labels in disassembly (a `.sym` file would solve this).

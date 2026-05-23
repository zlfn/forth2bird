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
0x0000..0x0006   bootstrap: Push <main>; CallAbsolute; Halt
0x0007..         prelude word bodies, then user word bodies & data
...
0x7FD0..0x7FEF   do/loop scratch (index+limit per nesting level, max 4)
0x7FF0..0x7FFF   scratch (used by swap/nip/rot)
0x8000..0xFFFF   stack (grows upward, per the VM spec)
```

The 64 KB address space is flat with no protection; the layout above is
just a convention the compiler follows. Variables and constants are
allocated in the code/data region right after the prelude.

## Status

This is a skeleton. Working: stack ops, arithmetic, comparisons,
syscalls, `:`/`;`, `variable`/`constant`, `if`/`else`/`then`,
`begin`/`until`, `begin`/`again`, `do`/`loop` (with `i`, max 4
nesting levels), `exit`, forward references between words,
automatic short/long jump encoding, disassembler with labels and
conditional-jump pattern recognition.

Not yet: `+loop`/`leave`/`?do`, `j` (outer-loop index), compile-time
arithmetic, named labels in disassembly (a `.sym` file would solve
this).

# Forth2Bird

Forth compiler (`fthc`) and disassembler (`fthd`) for the
DEF CON Quals 34 [LiveCTF](https://play.livectf.com/) VM.
a stack machine with a tiny custom bytecode ([docs](https://play.livectf.com/docs)).

> The LiveCTF ~~Combat Drone~~ Bird Training Program is a real-time Eagle-of-the-Roost style sHOOT-tern. Teams upload canditate birds, and The Program pits them against each other, to determine who is highest in The Pecking Order. 

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
two NPC bots that ship with the LiveCTF handout.

## Memory map (this compiler's convention)

```
0x0000..0x0011    bootstrap (init retstack pointer, call main, halt)
0x0012..0x0031    PROLOG_HELPER (32 B; shared trampoline for `:` entry)
0x0032..0x0045    EPILOG_HELPER (20 B; shared trampoline for `;`/`exit`)
0x0046..< 0x7000  user code + variables (max ~28 KB)
0x7000..0x7FC7    return-stack region (grows down from 0x7FC4, ~1010 frames)
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

# Language reference

The Forth dialect compiled by `fthc`. This is **not** a standard ANS Forth — it
is a deliberate subset tailored to the LiveCTF VM. Read alongside the
[LiveCTF VM spec](https://play.livectf.com/docs) for syscall details.

See [COOKBOOK.md](COOKBOOK.md) for verified idioms once you've read this.

---

## Source form

- Whitespace-separated tokens (spaces, tabs, newlines — all equivalent).
- Line comment: `\` followed by a space, through end-of-line.
- Block comment: `( anything until close paren )`. Spans lines.
- No string literals. No char literals.
- Identifiers can contain any non-whitespace characters (`add-ten`, `even?`,
  `q0`, `MAP_MAX_X` are all valid names).
- Case-sensitive: `Main` ≠ `main`.

## Numbers

- Decimal: `42`, `-7`, `0`.
- Hex: `0xFF`, `-0x100`. Upper- or lower-case `a-f` both work.
- All numbers parse to i64 then are truncated to i32 when emitted.
- The compiler picks the smallest encoding (`PushZero`, `PushShort`, `Push`)
  automatically.

## Word definitions

```forth
: name ( stack-effect ) body ;
```

- `:` opens a definition, `;` closes it.
- `name` may be any non-whitespace string. Don't use a name that's already
  a primitive (`+`, `dup`, `if`, ...) — your definition will be unreachable
  because primitives are looked up first.
- Nested `:` is rejected.
- Redefining a name silently overwrites the dictionary entry. Old call sites
  keep the old target address. Don't rely on this.

`main` is the entry point. The compiler emits a bootstrap that calls `main`
and halts on return.

## Variables and constants

```forth
variable score              \ reserves 4 bytes; the name pushes its address
constant THRESHOLD 100      \ inline literal; the name pushes the value
```

- Both are **top-level only** — they cannot appear inside `: ... ;`.
- `variable` **must be declared before its first use**. Forward references
  to variables are rejected (variable refs and word refs lower differently;
  a forward variable ref would emit the wrong bytecode shape).
- Each `variable` consumes 4 bytes of code/data space.
- Successive `variable` declarations are 4-byte-aligned and **consecutive**
  in memory, enabling pseudo-array indexing (`base i 4 * + @`).

## Stack manipulation

| word    | effect              | source                            |
|---------|---------------------|-----------------------------------|
| `dup`   | `( x -- x x )`      | inlined primitive                 |
| `drop`  | `( x -- )`          | primitive                         |
| `over`  | `( x y -- x y x )`  | inlined primitive                 |
| `swap`  | `( a b -- b a )`    | prelude word                      |
| `nip`   | `( a b -- b )`      | prelude word                      |
| `tuck`  | `( a b -- b a b )`  | prelude word (calls `swap`)       |
| `rot`   | `( a b c -- b c a )`| prelude word                      |
| `-rot`  | `( a b c -- c a b )`| prelude word (calls `rot` twice)  |
| `2dup`  | `( a b -- a b a b )`| prelude word                      |
| `2drop` | `( a b -- )`        | prelude word                      |

You don't need to know which are inlined vs. called — the compiler handles
both transparently.

## Arithmetic / bitwise / comparison / logical

All binary ops follow Forth direction: `a b OP` computes `a OP b`.

| token              | meaning                                       |
|--------------------|-----------------------------------------------|
| `+ - * / mod`      | arithmetic, wrapping at i32 boundaries        |
| `negate`           | unary minus                                   |
| `lshift rshift`    | left/right shift (`rshift` is **logical**)    |
| `and or xor`       | bitwise binary                                |
| `invert`           | bitwise NOT (unary)                           |
| `< <= > >= = <>`   | comparison, push 0 or 1                       |
| `not`              | logical NOT (`0 → 1`, nonzero → `0`)          |

Spec-defined edge cases (all guaranteed):

- `x 0 / = 0`, `x 0 mod = 0` (no error, no exception)
- `0x7FFFFFFF 1 + = i32::MIN` (wrapping)
- `negate` of `i32::MIN` = `i32::MIN` (wrapping fixed point)
- `rshift` zero-fills high bits; `-1 4 rshift = 0x0FFFFFFF`, not `-1`

## Control flow

```forth
\ if/else/then
cond if true-body then
cond if true-body else false-body then

\ begin/until — loops while cond is FALSY
begin loop-body cond until

\ begin/again — infinite loop; escape with `exit`
begin loop-body again

\ do/loop — counted loop; `i` reads the innermost index
limit start do body loop
```

- `if`/`then` chains nest freely.
- `do`/`loop` runs the body **at least once** even when `start == limit`
  (the compare-and-back-branch happens at the end). There's no `?do`.
- `i` reads the innermost `do`/`loop` index slot. There is **no `j`** — save
  outer `i` to a `variable` if you need it nested.
- Max 4 nested `do`/`loop` levels per word body.
- **No `leave`.** To exit a loop early, use `exit` (which leaves the entire
  word) or restructure with `begin`/`until` + a sentinel.

## `exit`

Ends the current word immediately. Emits the same epilog as `;`. Useful for
early returns from inside `if`/`then` or `do`/`loop`.

## Memory access

```forth
addr @         ( -- value )       load 4 bytes
val addr !     ( -- )             store 4 bytes
```

- 4-byte access only. No `c@`/`c!`.
- Addresses wrap modulo 65536. A store at `0xFFFE` straddles into `0x0000`.
- No bounds checking.

## Syscalls

```forth
\ Push args in REVERSE spec order so the first spec arg lands on TOS,
\ then argc, then the syscall number, then `syscall`. One return value
\ ends up on TOS.

\ Schematic for a syscall with spec args (a, b, c, d):
\   d c b a   4 N   syscall            \ pushes d first, a last; N = sysnum
```

Syscall numbers and arg layouts are defined by the [LiveCTF VM
spec](https://play.livectf.com/docs) and **shift between phases** — look
them up there rather than hard-coding from older sources. The `syscall`
token compiles to opcode `0x0E`; everything above it is just stack
preparation.

## Other primitives

| token  | meaning                                             |
|--------|-----------------------------------------------------|
| `halt` | stop execution (also triggered by unknown opcodes)  |
| `skip` | pop cond; if non-zero, skip the next code byte      |

`skip` is mostly there for inspecting hand-rolled bytecode from the NPC
samples — Forth code rarely needs it.

## Forward references

A `:` definition's body may reference any name. If the name isn't in the
dictionary yet, the compiler emits a placeholder and patches it when the
name is later defined:

```forth
: even? dup 0 = if drop 1 exit then 1 - odd? ;   \ odd? declared later
: odd?  dup 0 = if drop 0 exit then 1 - even? ;
```

Forward references resolve only to **words**. If the name later turns out
to be a `variable` or `constant`, compilation fails — the placeholder
shape (`PUSH addr; CALL_ABS`) doesn't match what a literal push needs.

## Calling convention (background)

Each `:` definition gets an auto-emitted 6-byte prolog and 6-byte epilog.
The prolog moves the return address (which `CALL_ABS` pushes on the data
stack) onto a dedicated return stack in memory; the epilog restores it.
This is why Forth primitives like `+`, `dup`, `swap` see clean args at the
top of the data stack instead of having to step over the return address.

The two helper routines (`PROLOG_HELPER` at `0x12`, `EPILOG_HELPER` at
`0x32`) are shared by every word — per-word overhead is just 12 bytes.

## Memory layout

```
0x0000 — 0x0011    bootstrap (init RSP, call main, halt)        18 B
0x0012 — 0x0031    PROLOG_HELPER                                 32 B
0x0032 — 0x0045    EPILOG_HELPER                                 20 B
0x0046 — < 0x7000  user code + variables                       ≤28 KB
0x7000 — 0x7FC7    return-stack region (1010 4-byte slots,
                   topmost slot at 0x7FC4..0x7FC7, grows down)
0x7FC8 — 0x7FCB    PROLOG_SCRATCH (helper temp)
0x7FCC — 0x7FCF    RSP storage (current return-stack pointer)
0x7FD0 — 0x7FEF    do/loop scratch (4 levels × 8 bytes)
0x7FF0 — 0x7FFF    prelude scratch (swap/nip/rot temps)
0x8000 — 0xFFFF    data stack (grows up from 0x8000)
```

Code+vars beyond `0x7000` is rejected at `finalize` time — the return
stack would grow into it and corrupt code at runtime.

## What standard Forth has that this dialect doesn't

If you're coming from ANS Forth, these will NOT compile here. Replacements
are in [COOKBOOK.md](COOKBOOK.md).

- Loop control: `leave`, `?do`, `+loop`, `j`, `unloop`
- Return-stack ops: `>r`, `r>`, `r@`, `2>r`, `2r>`, `2r@`
- Strings / chars: `."`, `s"`, `c"`, `'A'` char literal, `c@`, `c!`,
  `count`, `cmove`
- Convenience math: `1+`, `1-`, `2*`, `2/`, `min`, `max`, `abs`, `?dup`,
  `/mod`, `*/`, `*/mod`
- Memory helpers: `cells`, `cell+`, `chars`, `char+`, `allot`, `,`, `c,`
- Defining words: `create … does>`, `value`, `to`, `defer`, `is`
- Dictionary nav: `'`, `[']`, `execute`, `find`, `compile,`, `postpone`
- Compile-time: `[`, `]`, `literal`
- Locals: `locals|`
- I/O: `.`, `emit`, `cr`, `space`, `key`, `accept` — use `syscall` instead
- Double-cell numbers, floating point
- `case`/`of`/`endof`/`endcase`
- `recurse` — but a definition's name is in scope inside its own body, so
  `: f ... f ;` works directly

## Gotchas

1. **`variable` must precede first use.** Forward refs are word-only.
2. **Booleans are 0/1**, not `0`/`-1`. `cond -1 and = cond` is FALSE.
3. **`exit` leaves the entire word**, not just the loop. No `leave`.
4. **`do`/`loop` scratch is global at depth 0.** A word that uses
   `do`/`loop` called from inside another `do`/`loop` will clobber the
   outer index. Safe pattern: complete the loop first, then recurse / call.
5. **No `j`.** Save outer `i` to a `variable`.
6. **No char literals.** `'A'` doesn't parse; write `65`.
7. **`rshift` is logical** (zero-fill). For arithmetic shift on a negative,
   handle the sign yourself.
8. **`negate` of `i32::MIN` is `i32::MIN`.** Fixed point of wrapping NEG.
9. **DIV/REM by zero return 0.** Spec-defined.
10. **Addresses wrap mod 65536.** Be careful with computed addresses.
11. **`!` and `@` are 4-byte only.** Pack/unpack bytes yourself.
12. **Max binary is ~28 KB**, not 64 KB. The retstack reservation limits it.
13. **Position from the Input syscall needs sign extension.** Both halves
    are i16. See COOKBOOK for the canonical unpack words.
14. **Drain order for multi-value returns.** `7 9 r ! q !` stores 9 at r
    (TOS first), then 7 at q. Plan the order or use `swap`/`-rot` first.

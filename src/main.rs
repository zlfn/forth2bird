// LiveCTF Forth-style frontend.
//
// Input  : a .fth source file with Forth-like syntax.
// Output : raw bytecode for the LiveCTF VM (loaded verbatim at 0x0000).
//
// Layout of the emitted binary:
//
//   0x0000  Push <main_addr>     ; bootstrap (5 bytes, patched at end)
//   0x0005  CallAbsolute          ; call into `main`
//   0x0006  Halt                  ; reached when main returns
//   0x0007  <user word bodies, in source order>
//
// Each user word body ends with JumpAbsolute (0x0A), which pops the saved
// return IP that CallAbsolute pushed, jumping back to the caller. Effectively
// `;` is the Forth return primitive.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::process::ExitCode;

// ===========================================================================
// VM opcodes — shared via crate lib (see src/lib.rs)
// ===========================================================================
use livectf_forth::op;

// ===========================================================================
// Tokenizer
// ===========================================================================
//
// Whitespace separated. `\ ...\n` is a line comment. `( ... )` is a block
// comment (often used for stack-effect notes like `( a b -- c )`).
fn tokenize(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut it = src.chars().peekable();
    while let Some(&c) = it.peek() {
        if c.is_whitespace() {
            it.next();
            continue;
        }
        if c == '\\' {
            while let Some(c) = it.next() {
                if c == '\n' { break; }
            }
            continue;
        }
        if c == '(' {
            it.next();
            while let Some(c) = it.next() {
                if c == ')' { break; }
            }
            continue;
        }
        let mut tok = String::new();
        while let Some(&c) = it.peek() {
            if c.is_whitespace() { break; }
            tok.push(c);
            it.next();
        }
        if !tok.is_empty() {
            out.push(tok);
        }
    }
    out
}

// ===========================================================================
// Control-flow frames (open if/begin/...)
// ===========================================================================
enum CtrlFrame {
    // After `if`: we've emitted   LogicalNot; PushShort <placeholder>; Mul; JumpRelative
    // and either `else` or `then` will backpatch the placeholder.
    If { placeholder_pos: usize, body_start: usize },
    // After `else`: the true-body's exit jump (PushShort <placeholder>; JumpRel)
    // has been emitted and `then` will backpatch it with the else-body length.
    IfElse { placeholder_pos: usize, else_body_start: usize },
    // After `begin`: just records the back-branch target.
    Begin { begin_pos: usize },
}

// ===========================================================================
// Dictionary entries
// ===========================================================================
//
// A name in the dictionary is either a callable word (compiles to a call) or
// an inlined value (compiles to a push of the value).  Variables and constants
// produce Value entries; `:` definitions produce Word entries.
#[derive(Clone, Copy)]
enum DictEntry {
    Word(u16),
    Value(i64),
}

// ===========================================================================
// Compiler
// ===========================================================================
struct Compiler {
    code: Vec<u8>,
    dict: HashMap<String, DictEntry>,
    compiling: Option<String>,
    ctrl: Vec<CtrlFrame>,
    main_patch_pos: usize,
}

impl Compiler {
    fn new() -> Self {
        let mut c = Compiler {
            code: Vec::new(),
            dict: HashMap::new(),
            compiling: None,
            ctrl: Vec::new(),
            main_patch_pos: 0,
        };
        // Bootstrap: Push <main_addr>; CallAbsolute; Halt.
        c.code.push(op::PUSH);
        c.main_patch_pos = c.code.len();
        c.code.extend_from_slice(&[0, 0, 0, 0]);
        c.code.push(op::CALL_ABS);
        c.code.push(op::HALT);
        c
    }

    fn here(&self) -> usize { self.code.len() }

    fn emit(&mut self, bytes: &[u8]) {
        self.code.extend_from_slice(bytes);
    }

    fn emit_literal(&mut self, n: i64) {
        if n == 0 {
            self.emit(&[op::PUSH_ZERO]);
        } else if (-128..=127).contains(&n) {
            self.emit(&[op::PUSH_SHORT, n as i8 as u8]);
        } else {
            // i64 → i32 → u32 LE. Overflow is the user's problem.
            self.emit(&[op::PUSH]);
            let v = (n as i32) as u32;
            self.emit(&v.to_le_bytes());
        }
    }

    // Write a 4-byte little-endian i32 at the given position. Used to backpatch
    // long-form branch placeholders.
    fn write_i32_at(&mut self, pos: usize, value: i32) -> Result<(), String> {
        if pos + 4 > self.code.len() {
            return Err(format!("write_i32_at out of range: pos={}, len={}", pos, self.code.len()));
        }
        let bytes = (value as u32).to_le_bytes();
        self.code[pos..pos + 4].copy_from_slice(&bytes);
        Ok(())
    }

    fn emit_call(&mut self, addr: u16) {
        // Push <addr>; CallAbsolute.  6 bytes.
        // Could compress to PushShort if addr fits, but addresses fit in 16
        // bits and PushShort is only 8-bit, so this is the typical case.
        self.emit(&[op::PUSH]);
        self.emit(&(addr as u32).to_le_bytes());
        self.emit(&[op::CALL_ABS]);
    }

    // ---- primitives ----
    //
    // Returns true if `tok` was a primitive and bytes have been emitted.
    //
    // Operand-order quirk: Forth `a b OP` means "left operand a, right
    // operand b", i.e. result = NOS OP TOS. The VM's binary ops use the
    // opposite convention — per the spec, `left = pop()` (TOS), then
    // `right = pop()` (NOS), so e.g. SUB = TOS - NOS. Commutative ops
    // hide this disagreement; non-commutative ones need a fixup, handled
    // in the blocks below. Don't drop `-`/`/`/`mod`/`lshift`/`rshift`/
    // ordered-comparisons back into the single-opcode table without
    // re-deriving why they're called out — verified by the glibc LCG
    // pattern in examples/wander.asm at L00e3.
    fn try_primitive(&mut self, tok: &str) -> bool {
        // Single-opcode primitives — commutative binaries plus everything
        // that isn't a two-operand arithmetic/compare op.
        let single: Option<u8> = match tok {
            "+"       => Some(op::ADD),
            "*"       => Some(op::MUL),
            "and"     => Some(op::AND),
            "or"      => Some(op::OR),
            "xor"     => Some(op::XOR),
            "="       => Some(op::EQ),
            "<>"      => Some(op::NE),
            "not"     => Some(op::LOGICAL_NOT),
            "invert"  => Some(op::BITWISE_NOT),
            "negate"  => Some(op::NEG),
            "drop"    => Some(op::POP),
            "@"       => Some(op::LOAD_ABS),
            "!"       => Some(op::STORE_ABS),
            "halt"    => Some(op::HALT),
            "syscall" => Some(op::SYSCALL),
            "skip"    => Some(op::SKIP),
            "exit"    => Some(op::JUMP_ABS),
            _ => None,
        };
        if let Some(b) = single {
            self.emit(&[b]);
            return true;
        }

        // Ordered comparisons: emit the reflected opcode so VM's
        // `left CMP right` = TOS CMP NOS ends up computing what Forth's
        // `NOS CMP TOS` would. Costs nothing extra.
        let reflected: Option<u8> = match tok {
            "<"  => Some(op::GT),
            "<=" => Some(op::GE),
            ">"  => Some(op::LT),
            ">=" => Some(op::LE),
            _ => None,
        };
        if let Some(b) = reflected {
            self.emit(&[b]);
            return true;
        }

        // Subtraction: VM SUB yields TOS - NOS; NEG turns that into the
        // NOS - TOS that Forth `a b -` expects. 2 bytes, no scratch.
        if tok == "-" {
            self.emit(&[op::SUB, op::NEG]);
            return true;
        }

        // `/`, `mod`, `lshift`, `rshift` have no single-opcode reflection,
        // so swap TOS↔NOS via the prelude's `swap` (scratch at 0x7FF0/
        // 0x7FF4) before emitting the raw opcode.
        let swap_then: Option<u8> = match tok {
            "/"      => Some(op::DIV),
            "mod"    => Some(op::REM),
            "lshift" => Some(op::SHL),
            "rshift" => Some(op::SHR),
            _ => None,
        };
        if let Some(b) = swap_then {
            self.emit_swap_call();
            self.emit(&[b]);
            return true;
        }

        // Multi-byte primitives synthesized from SP-relative loads.
        // dup: ( x -- x x ).  After PushShort -4, SP advanced by 4 and -4 is
        // on top.  LoadSpRelative pops -4 (SP back to original), then reads
        // mem[SP + -4] which is the original top, and pushes it.
        if tok == "dup" {
            self.emit(&[op::PUSH_SHORT, 0xFC, op::LOAD_SP_REL]); // -4
            return true;
        }
        // over: ( a b -- a b a ).  Same trick with offset -8.
        if tok == "over" {
            self.emit(&[op::PUSH_SHORT, 0xF8, op::LOAD_SP_REL]); // -8
            return true;
        }
        // TODO: swap, nip, tuck, rot — these need either scratch memory or
        // careful StoreSpRelative sequences.  Left as exercises so the
        // skeleton stays small.
        false
    }

    // Emit a call to the prelude's `swap`. Invariant: the prelude defines
    // `swap` as the first user-visible word, so the lookup must succeed by
    // the time any user code is compiled. If it doesn't, the prelude has
    // been broken — fail loudly rather than silently miscompiling.
    fn emit_swap_call(&mut self) {
        match self.dict.get("swap").copied() {
            Some(DictEntry::Word(addr)) => self.emit_call(addr),
            _ => panic!("internal: `swap` missing from dictionary; prelude is required for non-commutative binary ops"),
        }
    }

    // ---- main per-token compile ----
    fn compile_token(&mut self, tok: &str) -> Result<(), String> {
        // Number literals (decimal or 0x hex; optional leading `-`).
        if let Some(n) = parse_number(tok) {
            self.emit_literal(n);
            return Ok(());
        }
        // Control flow.
        match tok {
            "if" => {
                // Long form (8 bytes): LogicalNot; Push <i32 placeholder>; Mul; JumpRel
                // i8 immediate is too small for arbitrary bodies, so always emit 4-byte
                // placeholder.  3-byte overhead vs the tightest possible encoding;
                // simpler and predictable than short→long auto-promotion.
                self.emit(&[op::LOGICAL_NOT, op::PUSH]);
                let placeholder_pos = self.code.len();
                self.emit(&[0, 0, 0, 0]);
                self.emit(&[op::MUL, op::JUMP_REL]);
                let body_start = self.code.len();
                self.ctrl.push(CtrlFrame::If { placeholder_pos, body_start });
                return Ok(());
            }
            "else" => {
                let frame = self.ctrl.pop()
                    .ok_or_else(|| "`else` with no matching `if`".to_string())?;
                let (placeholder_pos, body_start) = match frame {
                    CtrlFrame::If { placeholder_pos, body_start } => (placeholder_pos, body_start),
                    _ => return Err("`else` requires `if` on top of control stack".into()),
                };
                let true_len = self.code.len() - body_start;
                // Emit unconditional long skip-over-else: Push <i32 placeholder>; JumpRel (6 bytes)
                self.emit(&[op::PUSH]);
                let ph2 = self.code.len();
                self.emit(&[0, 0, 0, 0]);
                self.emit(&[op::JUMP_REL]);
                let else_body_start = self.code.len();
                // If's jump target now lands at else_body_start.
                // Offset (from body_start) = true_len + 6 (size of long skip-over-else).
                let if_offset = (true_len as i64) + 6;
                if !(i32::MIN as i64..=i32::MAX as i64).contains(&if_offset) {
                    return Err(format!("if-true-body too long: {} bytes", true_len));
                }
                self.write_i32_at(placeholder_pos, if_offset as i32)?;
                self.ctrl.push(CtrlFrame::IfElse { placeholder_pos: ph2, else_body_start });
                return Ok(());
            }
            "then" => {
                let frame = self.ctrl.pop()
                    .ok_or_else(|| "`then` with no matching `if`/`else`".to_string())?;
                let (placeholder_pos, body_start) = match frame {
                    CtrlFrame::If { placeholder_pos, body_start } => (placeholder_pos, body_start),
                    CtrlFrame::IfElse { placeholder_pos, else_body_start } => (placeholder_pos, else_body_start),
                    _ => return Err("`then` requires `if`/`else` on top".into()),
                };
                let len = (self.code.len() - body_start) as i64;
                if !(i32::MIN as i64..=i32::MAX as i64).contains(&len) {
                    return Err(format!("branch body too long: {} bytes", len));
                }
                self.write_i32_at(placeholder_pos, len as i32)?;
                return Ok(());
            }
            "begin" => {
                self.ctrl.push(CtrlFrame::Begin { begin_pos: self.code.len() });
                return Ok(());
            }
            "until" => {
                let frame = self.ctrl.pop()
                    .ok_or_else(|| "`until` with no matching `begin`".to_string())?;
                let begin_pos = match frame {
                    CtrlFrame::Begin { begin_pos } => begin_pos,
                    _ => return Err("`until` requires `begin` on top".into()),
                };
                // Short pattern (5 bytes): LogicalNot; PushShort <off>; Mul; JumpRel
                // Long pattern  (8 bytes): LogicalNot; Push <off i32>;  Mul; JumpRel
                // Loops back to begin_pos when cond is falsy (!cond=1, off*1=off).
                let off_short = (begin_pos as i64) - ((self.code.len() + 5) as i64);
                if (-128..=127).contains(&off_short) {
                    self.emit(&[
                        op::LOGICAL_NOT,
                        op::PUSH_SHORT, (off_short as i8) as u8,
                        op::MUL,
                        op::JUMP_REL,
                    ]);
                } else {
                    let off_long = (begin_pos as i64) - ((self.code.len() + 8) as i64);
                    if !(i32::MIN as i64..=i32::MAX as i64).contains(&off_long) {
                        return Err(format!("`until` jump out of i32 range: {}", off_long));
                    }
                    self.emit(&[op::LOGICAL_NOT, op::PUSH]);
                    self.emit(&((off_long as i32) as u32).to_le_bytes());
                    self.emit(&[op::MUL, op::JUMP_REL]);
                }
                return Ok(());
            }
            "again" => {
                let frame = self.ctrl.pop()
                    .ok_or_else(|| "`again` with no matching `begin`".to_string())?;
                let begin_pos = match frame {
                    CtrlFrame::Begin { begin_pos } => begin_pos,
                    _ => return Err("`again` requires `begin` on top".into()),
                };
                // Short pattern (3 bytes): PushShort <off>; JumpRel
                // Long pattern  (6 bytes): Push <off i32>;  JumpRel
                let off_short = (begin_pos as i64) - ((self.code.len() + 3) as i64);
                if (-128..=127).contains(&off_short) {
                    self.emit(&[op::PUSH_SHORT, (off_short as i8) as u8, op::JUMP_REL]);
                } else {
                    let off_long = (begin_pos as i64) - ((self.code.len() + 6) as i64);
                    if !(i32::MIN as i64..=i32::MAX as i64).contains(&off_long) {
                        return Err(format!("`again` jump out of i32 range: {}", off_long));
                    }
                    self.emit(&[op::PUSH]);
                    self.emit(&((off_long as i32) as u32).to_le_bytes());
                    self.emit(&[op::JUMP_REL]);
                }
                return Ok(());
            }
            _ => {}
        }
        if self.try_primitive(tok) {
            return Ok(());
        }
        if let Some(entry) = self.dict.get(tok).copied() {
            match entry {
                DictEntry::Word(addr) => self.emit_call(addr),
                DictEntry::Value(v) => self.emit_literal(v),
            }
            return Ok(());
        }
        Err(format!("unknown word: `{}`", tok))
    }

    // Compile a chunk of source.  Can be called multiple times to append.
    // Call `finalize` once after all chunks to patch `main` and check size.
    fn compile(&mut self, src: &str) -> Result<(), String> {
        let toks = tokenize(src);
        let mut i = 0;
        while i < toks.len() {
            let tok = &toks[i];
            match tok.as_str() {
                ":" => {
                    if self.compiling.is_some() {
                        return Err("nested `:` is not allowed".into());
                    }
                    i += 1;
                    let name = toks.get(i)
                        .ok_or_else(|| "`:` needs a name".to_string())?
                        .clone();
                    if self.here() > 0xFFFF {
                        return Err("binary already exceeds 64 KB before defining word".into());
                    }
                    self.dict.insert(name.clone(), DictEntry::Word(self.here() as u16));
                    self.compiling = Some(name);
                }
                "variable" => {
                    if self.compiling.is_some() {
                        return Err("`variable` not allowed inside a word definition".into());
                    }
                    i += 1;
                    let name = toks.get(i)
                        .ok_or_else(|| "`variable` needs a name".to_string())?
                        .clone();
                    let addr = self.code.len();
                    if addr + 4 > 0xFFFF {
                        return Err("not enough space to allocate variable".into());
                    }
                    self.emit(&[0, 0, 0, 0]);
                    self.dict.insert(name, DictEntry::Value(addr as i64));
                }
                "constant" => {
                    if self.compiling.is_some() {
                        return Err("`constant` not allowed inside a word definition".into());
                    }
                    i += 1;
                    let name = toks.get(i)
                        .ok_or_else(|| "`constant` needs a name".to_string())?
                        .clone();
                    i += 1;
                    let value_tok = toks.get(i)
                        .ok_or_else(|| "`constant` needs a value".to_string())?;
                    let value = parse_number(value_tok)
                        .ok_or_else(|| format!("`constant`: bad value `{}`", value_tok))?;
                    self.dict.insert(name, DictEntry::Value(value));
                }
                ";" => {
                    if self.compiling.is_none() {
                        return Err("`;` with no matching `:`".into());
                    }
                    if !self.ctrl.is_empty() {
                        return Err("`;` while a control block is open (if without then?)".into());
                    }
                    // Return: pop saved IP (pushed by CallAbsolute) and jump to it.
                    self.emit(&[op::JUMP_ABS]);
                    self.compiling = None;
                }
                _ => {
                    if self.compiling.is_none() {
                        return Err(format!(
                            "token `{}` outside of a word definition (wrap your code in `: main ... ;`)",
                            tok,
                        ));
                    }
                    self.compile_token(tok)?;
                }
            }
            i += 1;
        }
        Ok(())
    }

    fn finalize(&mut self) -> Result<(), String> {
        if let Some(name) = &self.compiling {
            return Err(format!("unclosed definition: `{}` missing `;`", name));
        }
        let main_addr = match self.dict.get("main").copied() {
            Some(DictEntry::Word(a)) => a,
            Some(DictEntry::Value(_)) => return Err("`main` must be a word (`: main ... ;`), not a constant/variable".into()),
            None => return Err("no `main` word defined".into()),
        };
        let bytes = (main_addr as u32).to_le_bytes();
        self.code[self.main_patch_pos .. self.main_patch_pos + 4]
            .copy_from_slice(&bytes);

        if self.code.len() > 0x10000 {
            return Err(format!(
                "binary too large: {} bytes (max 65536)", self.code.len()
            ));
        }
        Ok(())
    }

    fn into_bytes(self) -> Vec<u8> { self.code }
}

fn parse_number(tok: &str) -> Option<i64> {
    if let Some(rest) = tok.strip_prefix("0x") {
        return i64::from_str_radix(rest, 16).ok();
    }
    if let Some(rest) = tok.strip_prefix("-0x") {
        return i64::from_str_radix(rest, 16).ok().map(|n| -n);
    }
    tok.parse::<i64>().ok()
}

// ===========================================================================
// Prelude — stack-manipulation words built on a scratch memory region.
// ===========================================================================
const PRELUDE: &str = include_str!("prelude.fth");

// ===========================================================================
// CLI
// ===========================================================================
fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: {} <input.fth> <output.bin>", args[0]);
        return ExitCode::from(2);
    }
    let src = match fs::read_to_string(&args[1]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {}: {}", args[1], e);
            return ExitCode::from(1);
        }
    };
    let mut c = Compiler::new();
    if let Err(e) = c.compile(PRELUDE) {
        eprintln!("error (in prelude): {}", e);
        return ExitCode::from(1);
    }
    if let Err(e) = c.compile(&src) {
        eprintln!("error: {}", e);
        return ExitCode::from(1);
    }
    if let Err(e) = c.finalize() {
        eprintln!("error: {}", e);
        return ExitCode::from(1);
    }
    let bytes = c.into_bytes();
    if let Err(e) = fs::write(&args[2], &bytes) {
        eprintln!("cannot write {}: {}", args[2], e);
        return ExitCode::from(1);
    }
    eprintln!("wrote {} bytes to {}", bytes.len(), args[2]);
    ExitCode::SUCCESS
}

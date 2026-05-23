// LiveCTF Forth-style compiler — library crate.
//
// The opcode constants in `op` are shared by the compiler (`fthc`) and the
// disassembler (`fthd`). Everything else is the compiler implementation,
// exposed publicly so integration tests in `tests/` can exercise it.

use std::collections::HashMap;

// ===========================================================================
// VM opcodes
// ===========================================================================
#[allow(dead_code)]
pub mod op {
    pub const HALT:         u8 = 0x00;
    pub const PUSH:         u8 = 0x01; // [u32 LE]
    pub const PUSH_ZERO:    u8 = 0x02;
    pub const PUSH_SHORT:   u8 = 0x03; // [i8]
    pub const POP:          u8 = 0x04;
    pub const LOAD_SP_REL:  u8 = 0x05;
    pub const STORE_SP_REL: u8 = 0x06;
    pub const LOAD_ABS:     u8 = 0x07;
    pub const STORE_ABS:    u8 = 0x08;
    pub const JUMP_REL:     u8 = 0x09;
    pub const JUMP_ABS:     u8 = 0x0A;
    pub const CALL_REL:     u8 = 0x0B;
    pub const CALL_ABS:     u8 = 0x0C;
    pub const SKIP:         u8 = 0x0D;
    pub const SYSCALL:      u8 = 0x0E;

    pub const LT:           u8 = 0x10;
    pub const LE:           u8 = 0x11;
    pub const GT:           u8 = 0x12;
    pub const GE:           u8 = 0x13;
    pub const EQ:           u8 = 0x14;
    pub const NE:           u8 = 0x15;

    pub const ADD:          u8 = 0x20;
    pub const SUB:          u8 = 0x21;
    pub const MUL:          u8 = 0x22;
    pub const DIV:          u8 = 0x23;
    pub const REM:          u8 = 0x24;
    pub const AND:          u8 = 0x25;
    pub const OR:           u8 = 0x26;
    pub const XOR:          u8 = 0x27;
    pub const SHL:          u8 = 0x28;
    pub const SHR:          u8 = 0x29;
    pub const LOGICAL_NOT:  u8 = 0x2A;
    pub const BITWISE_NOT:  u8 = 0x2B;
    pub const NEG:          u8 = 0x2C;
}

// ===========================================================================
// Tokenizer
// ===========================================================================
//
// Whitespace separated. `\ ...\n` is a line comment. `( ... )` is a block
// comment (often used for stack-effect notes like `( a b -- c )`).
pub fn tokenize(src: &str) -> Vec<String> {
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

pub fn parse_number(tok: &str) -> Option<i64> {
    if let Some(rest) = tok.strip_prefix("0x") {
        return i64::from_str_radix(rest, 16).ok();
    }
    if let Some(rest) = tok.strip_prefix("-0x") {
        return i64::from_str_radix(rest, 16).ok().map(|n| -n);
    }
    tok.parse::<i64>().ok()
}

// ===========================================================================
// Control-flow frames (open if/begin/do/...)
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
    // After `do`: records branch-back target and which scratch slot pair
    // (DO_LOOP_BASE + depth*8) holds this loop's index/limit. `loop` pops
    // the frame, increments the index, and back-branches while index<limit.
    Do { begin_pos: usize, depth: usize },
}

// do/loop scratch: per-nesting-level index + limit (4 bytes each, 8 bytes
// per level) live at DO_LOOP_BASE + depth*8 (index) and +4 (limit). Sits
// just below the prelude's swap/nip/rot scratch (0x7FF0..0x7FFF).
pub const DO_LOOP_BASE: u32 = 0x7FD0;
pub const DO_LOOP_MAX_DEPTH: usize = 4;

// Return-stack support.
//
// CALL_ABS pushes the return address onto the data stack, which would clash
// with Forth primitives that operate on TOS (e.g. `+`, `dup`, `swap`). To
// keep the data stack clean for primitive operations, every `:` definition
// gets a prolog that moves the freshly-pushed RA off the data stack onto a
// dedicated return stack in memory, and every `;`/`exit` gets an epilog that
// pops the RA back and `JUMP_ABS`es to it.
//
// Memory layout:
//   0x0000..0x0011   bootstrap (18 B)
//   0x0012..0x0031   PROLOG_HELPER (32 B)
//   0x0032..0x0045   EPILOG_HELPER (20 B)
//   0x0046..0x6FFF   user code + variables
//   0x7000..0x7FC7   return-stack region (grows down from 0x7FC4)
//   0x7FC8..0x7FCB   PROLOG_SCRATCH_ADDR — used by PROLOG_HELPER to stash the
//                    return-to-body address while it juggles the caller RA
//   0x7FCC..0x7FCF   RSP_STORAGE_ADDR  — current RSP (mutable, init'd in bootstrap)
//   0x7FD0..0x7FEF   do/loop scratch
//   0x7FF0..0x7FFF   prelude scratch (swap/nip/rot temps)
//
// Each `:` definition emits a 6-byte prolog that calls PROLOG_HELPER, and
// each `;`/`exit` emits a 6-byte epilog that JUMP_ABSes to EPILOG_HELPER.
// The helpers contain the actual RSP-juggling sequence (32 + 20 bytes once),
// rather than inlining it (39 bytes) at every word — saves ~27 B per word
// past a 2-word break-even.
//
// RETSTACK_INITIAL is the "empty" RSP value (just past the top). RSP shrinks
// by 4 on push and grows by 4 on pop:
//   push: RSP -= 4;  mem[RSP] = val
//   pop:  val = mem[RSP]; RSP += 4
pub const PROLOG_SCRATCH_ADDR: u32 = 0x7FC8;
pub const RSP_STORAGE_ADDR:    u32 = 0x7FCC;
pub const RETSTACK_INITIAL:    u32 = 0x7FC8;
pub const PROLOG_HELPER_ADDR:  u16 = 18;
pub const EPILOG_HELPER_ADDR:  u16 = 50;
pub const PROLOG_LEN: usize = 6;
pub const EPILOG_LEN: usize = 6;
// Bootstrap + both helper bodies — user-visible code begins at this offset.
pub const PREAMBLE_LEN: usize = 70;

// Highest legal end-of-code address. The spec only requires the binary to
// fit in 64 KB, but at runtime we use 0x7000..0x7FC4 as a return stack that
// grows downward from RETSTACK_INITIAL. If code/data crossed 0x7000, a
// sufficiently deep call chain would push the return stack down into the
// code region and silently corrupt it. Cap code+vars at 0x7000 so the
// retstack always has ~1000 frames of clearance.
pub const MAX_CODE_END: usize = 0x7000;

// ===========================================================================
// Dictionary entries
// ===========================================================================
//
// A name in the dictionary is either a callable word (compiles to a call) or
// an inlined value (compiles to a push of the value).  Variables and constants
// produce Value entries; `:` definitions produce Word entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DictEntry {
    Word(u16),
    Value(i64),
}

// ===========================================================================
// Compiler
// ===========================================================================
pub struct Compiler {
    code: Vec<u8>,
    dict: HashMap<String, DictEntry>,
    compiling: Option<String>,
    ctrl: Vec<CtrlFrame>,
    main_patch_pos: usize,
    // Count of currently-open `do` blocks. Used to assign scratch slots and
    // to validate `i` placement.
    do_depth: usize,
    // Pending forward references: name → list of 4-byte placeholder slots
    // (inside `PUSH <addr>; CALL_ABS` sequences) waiting to be patched once
    // the name is defined. An entry that survives `finalize` is an error.
    fwd_refs: HashMap<String, Vec<usize>>,
}

impl Compiler {
    pub fn new() -> Self {
        let mut c = Compiler {
            code: Vec::new(),
            dict: HashMap::new(),
            compiling: None,
            ctrl: Vec::new(),
            main_patch_pos: 0,
            do_depth: 0,
            fwd_refs: HashMap::new(),
        };
        // Bootstrap (18 bytes):
        //   PUSH RETSTACK_INITIAL; PUSH RSP_STORAGE_ADDR; STORE_ABS   (11)
        //   PUSH <main_addr>; CALL_ABS; HALT                          (7)
        // First three instructions initialize mem[RSP_STORAGE_ADDR] so the
        // very first `:` prolog (typically main's) sees a valid RSP.
        c.code.push(op::PUSH);
        c.code.extend_from_slice(&RETSTACK_INITIAL.to_le_bytes());
        c.code.push(op::PUSH);
        c.code.extend_from_slice(&RSP_STORAGE_ADDR.to_le_bytes());
        c.code.push(op::STORE_ABS);
        c.code.push(op::PUSH);
        c.main_patch_pos = c.code.len();
        c.code.extend_from_slice(&[0, 0, 0, 0]);
        c.code.push(op::CALL_ABS);
        c.code.push(op::HALT);
        debug_assert_eq!(c.code.len(), PROLOG_HELPER_ADDR as usize);
        c.emit_prolog_helper();
        debug_assert_eq!(c.code.len(), EPILOG_HELPER_ADDR as usize);
        c.emit_epilog_helper();
        debug_assert_eq!(c.code.len(), PREAMBLE_LEN);
        c
    }

    // PROLOG_HELPER body (32 bytes). Each word's 6-byte prolog `PUSH addr;
    // CALL_ABS` lands here with the data stack holding [caller_RA, body_RA],
    // where body_RA is the position right after the word's CALL_ABS — i.e.,
    // the start of its actual body. We stash body_RA in PROLOG_SCRATCH so we
    // can run the inline-prolog sequence on caller_RA (move it from data
    // stack to retstack), then JUMP_ABS body_RA to enter the body.
    fn emit_prolog_helper(&mut self) {
        let start = self.code.len();
        // Stash body_RA (TOS) into PROLOG_SCRATCH so it survives the RA push.
        self.emit(&[op::PUSH]);
        self.emit(&PROLOG_SCRATCH_ADDR.to_le_bytes());
        self.emit(&[op::STORE_ABS]);             // [caller_RA]
        // Push caller_RA onto the retstack (same 19-byte sequence as the old
        // inline prolog).
        self.emit(&[op::PUSH]);
        self.emit(&RSP_STORAGE_ADDR.to_le_bytes());
        self.emit(&[op::LOAD_ABS]);              // [caller_RA, old_RSP]
        self.emit(&[op::PUSH_SHORT, 0xFC, op::ADD]);
        self.emit(&[op::PUSH_SHORT, 0xFC, op::LOAD_SP_REL]);
        self.emit(&[op::PUSH]);
        self.emit(&RSP_STORAGE_ADDR.to_le_bytes());
        self.emit(&[op::STORE_ABS]);             // [caller_RA, new_RSP]; mem[RSP]=new_RSP
        self.emit(&[op::STORE_ABS]);             // []; mem[new_RSP]=caller_RA
        // Reload body_RA and jump into the actual body.
        self.emit(&[op::PUSH]);
        self.emit(&PROLOG_SCRATCH_ADDR.to_le_bytes());
        self.emit(&[op::LOAD_ABS, op::JUMP_ABS]);
        debug_assert_eq!(self.code.len() - start, 32);
    }

    // EPILOG_HELPER body (20 bytes). Each word's 6-byte epilog `PUSH addr;
    // JUMP_ABS` lands here with the data stack untouched (no RA pushed —
    // JUMP_ABS is a tail jump, not a call). We pop the saved caller RA off
    // the retstack and JUMP_ABS straight back to it.
    fn emit_epilog_helper(&mut self) {
        let start = self.code.len();
        self.emit(&[op::PUSH]);
        self.emit(&RSP_STORAGE_ADDR.to_le_bytes());
        self.emit(&[op::LOAD_ABS]);              // [old_RSP]
        self.emit(&[op::PUSH_SHORT, 0xFC, op::LOAD_SP_REL]);  // dup old_RSP
        self.emit(&[op::PUSH_SHORT, 0x04, op::ADD]);          // [old_RSP, new_RSP]
        self.emit(&[op::PUSH]);
        self.emit(&RSP_STORAGE_ADDR.to_le_bytes());
        self.emit(&[op::STORE_ABS]);             // [old_RSP]; mem[RSP]=new_RSP
        self.emit(&[op::LOAD_ABS, op::JUMP_ABS]);
        debug_assert_eq!(self.code.len() - start, 20);
    }

    pub fn here(&self) -> usize { self.code.len() }

    pub fn code(&self) -> &[u8] { &self.code }

    pub fn dict_get(&self, name: &str) -> Option<DictEntry> { self.dict.get(name).copied() }

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

    // Per-word prolog thunk (PROLOG_LEN = 6 bytes): `PUSH PROLOG_HELPER_ADDR;
    // CALL_ABS`. CALL_ABS pushes the address of the byte after itself (i.e.,
    // the start of the real body), so the helper enters with stack
    // [caller_RA, body_RA] — exactly what it needs to stash body_RA and move
    // caller_RA onto the retstack.
    fn emit_prolog(&mut self) {
        let start = self.code.len();
        self.emit(&[op::PUSH]);
        self.emit(&(PROLOG_HELPER_ADDR as u32).to_le_bytes());
        self.emit(&[op::CALL_ABS]);
        debug_assert_eq!(self.code.len() - start, PROLOG_LEN);
    }

    // Per-word epilog thunk (EPILOG_LEN = 6 bytes): `PUSH EPILOG_HELPER_ADDR;
    // JUMP_ABS`. We use JUMP_ABS (not CALL_ABS) so no extra RA is left on the
    // data stack — the helper just pops the saved caller_RA off the retstack
    // and JUMP_ABSes back to it.
    fn emit_epilog(&mut self) {
        let start = self.code.len();
        self.emit(&[op::PUSH]);
        self.emit(&(EPILOG_HELPER_ADDR as u32).to_le_bytes());
        self.emit(&[op::JUMP_ABS]);
        debug_assert_eq!(self.code.len() - start, EPILOG_LEN);
    }

    fn emit_call(&mut self, addr: u16) {
        // Push <addr>; CallAbsolute.  6 bytes.
        // Could compress to PushShort if addr fits, but addresses fit in 16
        // bits and PushShort is only 8-bit, so this is the typical case.
        self.emit(&[op::PUSH]);
        self.emit(&(addr as u32).to_le_bytes());
        self.emit(&[op::CALL_ABS]);
    }

    // Emit `PUSH <0; placeholder>; CALL_ABS`, return the position of the
    // 4-byte placeholder slot so the caller can patch it later. Used for
    // forward references: the address is unknown at emit time and the
    // caller records the slot in fwd_refs (or any other fixup map).
    fn emit_call_placeholder(&mut self) -> usize {
        self.emit(&[op::PUSH]);
        let pos = self.code.len();
        self.emit(&[0, 0, 0, 0]);
        self.emit(&[op::CALL_ABS]);
        pos
    }

    // Walk fwd_refs for `name` and patch every recorded placeholder slot
    // with the freshly-known address. Called when a forward-referenced
    // word finally gets a `:` definition.
    fn resolve_fwd_refs(&mut self, name: &str, addr: u16) -> Result<(), String> {
        if let Some(positions) = self.fwd_refs.remove(name) {
            let bytes = (addr as u32).to_le_bytes();
            for pos in positions {
                if pos + 4 > self.code.len() {
                    return Err(format!(
                        "internal: fwd-ref slot for `{}` at {} is out of bounds", name, pos
                    ));
                }
                self.code[pos..pos + 4].copy_from_slice(&bytes);
            }
        }
        Ok(())
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
            // NOTE: `exit` is NOT a single opcode — it must run the same
            // epilog as `;` (pop RA from the return stack, then JUMP_ABS).
            // Handled in `compile_token`.
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
            "exit" => {
                // Early return: same epilog as `;`, but the definition stays
                // open so later tokens (e.g. an `else` branch) can still emit
                // code that's only reached when the `if` was false.
                self.emit_epilog();
                return Ok(());
            }
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
            "do" => {
                // `limit start do ... loop` — body runs for index in
                // [start, limit). Stash start as the initial index and
                // limit alongside it in this nesting level's scratch pair,
                // then mark the loop top so `loop` can back-branch to it.
                if self.do_depth >= DO_LOOP_MAX_DEPTH {
                    return Err(format!(
                        "`do` nesting too deep (max {}); raise DO_LOOP_MAX_DEPTH if you need more",
                        DO_LOOP_MAX_DEPTH
                    ));
                }
                let depth = self.do_depth;
                let index_addr = DO_LOOP_BASE + (depth as u32) * 8;
                let limit_addr = index_addr + 4;
                // Stack before do: [limit, start], TOS=start. STORE_ABS
                // pops addr (TOS), then value (NOS) and stores value at
                // addr — so pushing index_addr stores `start` there, then
                // pushing limit_addr stores `limit`.
                self.emit(&[op::PUSH]);
                self.emit(&index_addr.to_le_bytes());
                self.emit(&[op::STORE_ABS, op::PUSH]);
                self.emit(&limit_addr.to_le_bytes());
                self.emit(&[op::STORE_ABS]);
                self.do_depth += 1;
                let begin_pos = self.code.len();
                self.ctrl.push(CtrlFrame::Do { begin_pos, depth });
                return Ok(());
            }
            "loop" => {
                let frame = self.ctrl.pop()
                    .ok_or_else(|| "`loop` with no matching `do`".to_string())?;
                let (begin_pos, depth) = match frame {
                    CtrlFrame::Do { begin_pos, depth } => (begin_pos, depth),
                    _ => return Err("`loop` requires `do` on top".into()),
                };
                self.do_depth -= 1;
                let index_addr = DO_LOOP_BASE + (depth as u32) * 8;
                let limit_addr = index_addr + 4;
                // Increment the index, write it back, and compare against
                // limit. Sequence (stack effect annotated):
                //   PUSH index_addr; LOAD_ABS         ( -- old )
                //   PUSH_SHORT 1; ADD                 ( -- new )
                //   PUSH_SHORT -4; LOAD_SP_REL        ( -- new new )   inline dup
                //   PUSH index_addr; STORE_ABS        ( -- new )
                //   PUSH limit_addr; LOAD_ABS         ( -- new limit )
                //   GT                                ( -- new<limit )  reflected `<`
                self.emit(&[op::PUSH]);
                self.emit(&index_addr.to_le_bytes());
                self.emit(&[op::LOAD_ABS, op::PUSH_SHORT, 1, op::ADD]);
                self.emit(&[op::PUSH_SHORT, 0xFC, op::LOAD_SP_REL]);
                self.emit(&[op::PUSH]);
                self.emit(&index_addr.to_le_bytes());
                self.emit(&[op::STORE_ABS, op::PUSH]);
                self.emit(&limit_addr.to_le_bytes());
                self.emit(&[op::LOAD_ABS, op::GT]);
                // Conditional back-branch to begin_pos. Mirrors `until`
                // but without the LOGICAL_NOT — here we want to loop while
                // the condition is true (index < limit), not while false.
                // Short pattern (4 bytes): PushShort <off>; Mul; JumpRel
                // Long pattern  (7 bytes): Push <off i32>;  Mul; JumpRel
                let off_short = (begin_pos as i64) - ((self.code.len() + 4) as i64);
                if (-128..=127).contains(&off_short) {
                    self.emit(&[op::PUSH_SHORT, (off_short as i8) as u8, op::MUL, op::JUMP_REL]);
                } else {
                    let off_long = (begin_pos as i64) - ((self.code.len() + 7) as i64);
                    if !(i32::MIN as i64..=i32::MAX as i64).contains(&off_long) {
                        return Err(format!("`loop` jump out of i32 range: {}", off_long));
                    }
                    self.emit(&[op::PUSH]);
                    self.emit(&((off_long as i32) as u32).to_le_bytes());
                    self.emit(&[op::MUL, op::JUMP_REL]);
                }
                return Ok(());
            }
            "i" => {
                if self.do_depth == 0 {
                    return Err("`i` used outside of a `do`/`loop`".into());
                }
                // Innermost loop's index slot.
                let index_addr = DO_LOOP_BASE + ((self.do_depth - 1) as u32) * 8;
                self.emit(&[op::PUSH]);
                self.emit(&index_addr.to_le_bytes());
                self.emit(&[op::LOAD_ABS]);
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
        // Forward reference: assume `tok` will be `:`-defined later as a
        // word. Emit a placeholder call and stash the patch site. If it
        // turns out to be `variable`/`constant`, those branches reject the
        // definition; if it's never defined, `finalize` reports it.
        let pos = self.emit_call_placeholder();
        self.fwd_refs.entry(tok.to_string()).or_default().push(pos);
        Ok(())
    }

    // Compile a chunk of source.  Can be called multiple times to append.
    // Call `finalize` once after all chunks to patch `main` and check size.
    pub fn compile(&mut self, src: &str) -> Result<(), String> {
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
                    let addr = self.here() as u16;
                    self.dict.insert(name.clone(), DictEntry::Word(addr));
                    // Backfill any earlier callers that referenced this name
                    // before its definition.
                    self.resolve_fwd_refs(&name, addr)?;
                    self.compiling = Some(name);
                    // Stash the RA (pushed by CALL_ABS) onto the return stack
                    // so the body can use the data stack for Forth primitives
                    // without disturbing the caller's link.
                    self.emit_prolog();
                }
                "variable" => {
                    if self.compiling.is_some() {
                        return Err("`variable` not allowed inside a word definition".into());
                    }
                    i += 1;
                    let name = toks.get(i)
                        .ok_or_else(|| "`variable` needs a name".to_string())?
                        .clone();
                    // Forward refs emit a call (PUSH placeholder; CALL_ABS).
                    // A variable would have to emit a literal push instead,
                    // so the earlier emitted shape is wrong — reject.
                    if self.fwd_refs.contains_key(&name) {
                        return Err(format!(
                            "`{}` was forward-referenced as a word but is being defined as a variable",
                            name
                        ));
                    }
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
                    if self.fwd_refs.contains_key(&name) {
                        return Err(format!(
                            "`{}` was forward-referenced as a word but is being defined as a constant",
                            name
                        ));
                    }
                    self.dict.insert(name, DictEntry::Value(value));
                }
                ";" => {
                    if self.compiling.is_none() {
                        return Err("`;` with no matching `:`".into());
                    }
                    if !self.ctrl.is_empty() {
                        return Err("`;` while a control block is open (if without then?)".into());
                    }
                    // Return: epilog pops RA from the return stack and JUMP_ABSes
                    // back to the caller.
                    self.emit_epilog();
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

    pub fn finalize(&mut self) -> Result<(), String> {
        if let Some(name) = &self.compiling {
            return Err(format!("unclosed definition: `{}` missing `;`", name));
        }
        if !self.fwd_refs.is_empty() {
            let mut names: Vec<&String> = self.fwd_refs.keys().collect();
            names.sort();
            return Err(format!(
                "unresolved forward references: {}",
                names.iter().map(|s| format!("`{}`", s)).collect::<Vec<_>>().join(", ")
            ));
        }
        let main_addr = match self.dict.get("main").copied() {
            Some(DictEntry::Word(a)) => a,
            Some(DictEntry::Value(_)) => return Err("`main` must be a word (`: main ... ;`), not a constant/variable".into()),
            None => return Err("no `main` word defined".into()),
        };
        let bytes = (main_addr as u32).to_le_bytes();
        self.code[self.main_patch_pos .. self.main_patch_pos + 4]
            .copy_from_slice(&bytes);

        if self.code.len() > MAX_CODE_END {
            return Err(format!(
                "binary too large: {} bytes (max {} = 0x{:04x}; \
                 higher would let the return stack at 0x7000..0x7FC4 \
                 overwrite code/data at runtime)",
                self.code.len(), MAX_CODE_END, MAX_CODE_END,
            ));
        }
        Ok(())
    }

    pub fn into_bytes(self) -> Vec<u8> { self.code }
}

impl Default for Compiler {
    fn default() -> Self { Self::new() }
}

// ===========================================================================
// Prelude — stack-manipulation words built on a scratch memory region.
// ===========================================================================
pub const PRELUDE: &str = include_str!("prelude.fth");

// One-shot helper: prelude + user source → finalized bytecode.
pub fn compile_program(src: &str) -> Result<Vec<u8>, String> {
    let mut c = Compiler::new();
    c.compile(PRELUDE).map_err(|e| format!("prelude: {}", e))?;
    c.compile(src)?;
    c.finalize()?;
    Ok(c.into_bytes())
}

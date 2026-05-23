//! Compiler — produces LiveCTF VM bytecode from a Forth-style source.
//!
//! The compiler is single-pass with a deferred-patch table for forward
//! references between words. Top-level constants for the memory layout
//! (return-stack region, helper addresses, do/loop scratch) live in
//! `lib.rs` and are re-imported here.

use std::collections::HashMap;

use crate::lexer::{parse_number, tokenize};
use crate::op;
use crate::{
    DO_LOOP_BASE, DO_LOOP_MAX_DEPTH, EPILOG_HELPER_ADDR, EPILOG_LEN, MAX_CODE_END,
    PREAMBLE_LEN, PROLOG_HELPER_ADDR, PROLOG_LEN, PROLOG_SCRATCH_ADDR,
    RETSTACK_INITIAL, RSP_STORAGE_ADDR,
};

// ===========================================================================
// Dictionary
// ===========================================================================

/// A name's meaning in the dictionary. Words compile to a call; values
/// (variables and constants) compile to a literal push.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DictEntry {
    Word(u16),
    Value(i64),
}

// ===========================================================================
// Open control structures
// ===========================================================================

/// One open control structure on the compile-time control stack. Each
/// opening keyword (`if`, `begin`, `do`) pushes a frame; the matching
/// closing keyword (`then`/`else`, `until`/`again`, `loop`) pops it.
enum CtrlFrame {
    /// After `if`: a `LogicalNot; Push <i32 ph>; Mul; JumpRel` sequence has
    /// been emitted with `placeholder_pos` at the 4-byte slot. `else` or
    /// `then` will backpatch it with the body length.
    If { placeholder_pos: usize, body_start: usize },
    /// After `else`: a `Push <i32 ph>; JumpRel` (skip-over-else) has been
    /// emitted; `then` will patch it with the else-body length.
    IfElse { placeholder_pos: usize, else_body_start: usize },
    /// After `begin`: just records the position to back-branch to.
    Begin { begin_pos: usize },
    /// After `do`: back-branch target plus this loop's scratch-slot depth.
    Do { begin_pos: usize, depth: usize },
}

// ===========================================================================
// Compiler
// ===========================================================================

pub struct Compiler {
    code: Vec<u8>,
    dict: HashMap<String, DictEntry>,
    /// `Some(name)` while a `: name ... ;` body is open; `None` at top level.
    compiling: Option<String>,
    ctrl: Vec<CtrlFrame>,
    /// Position of the 4-byte main-address slot in the bootstrap; patched
    /// at `finalize()` once `main`'s address is known.
    main_patch_pos: usize,
    /// Count of currently-open `do` blocks. Used both to assign
    /// scratch-slot indices and to validate `i` placement.
    do_depth: usize,
    /// Pending forward references: name → list of 4-byte placeholder slots
    /// (inside `PUSH <addr>; CALL_ABS` sequences) awaiting a definition.
    /// An entry that survives `finalize` is an error.
    fwd_refs: HashMap<String, Vec<usize>>,
}

impl Default for Compiler {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// Construction / public accessors
// ---------------------------------------------------------------------------

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
        c.emit_bootstrap();
        debug_assert_eq!(c.code.len(), PROLOG_HELPER_ADDR as usize);
        c.emit_prolog_helper();
        debug_assert_eq!(c.code.len(), EPILOG_HELPER_ADDR as usize);
        c.emit_epilog_helper();
        debug_assert_eq!(c.code.len(), PREAMBLE_LEN);
        c
    }

    pub fn here(&self) -> usize { self.code.len() }
    pub fn code(&self) -> &[u8] { &self.code }
    pub fn dict_get(&self, name: &str) -> Option<DictEntry> { self.dict.get(name).copied() }
    pub fn into_bytes(self) -> Vec<u8> { self.code }
}

// ---------------------------------------------------------------------------
// Low-level emit helpers
// ---------------------------------------------------------------------------

impl Compiler {
    fn emit(&mut self, bytes: &[u8]) {
        self.code.extend_from_slice(bytes);
    }

    /// Emit `PUSH v` (5 bytes: opcode + u32 LE).
    fn emit_push_long(&mut self, v: u32) {
        self.emit(&[op::PUSH]);
        self.emit(&v.to_le_bytes());
    }

    /// Emit `PUSH_SHORT v` (2 bytes: opcode + i8).
    fn emit_push_short(&mut self, v: i8) {
        self.emit(&[op::PUSH_SHORT, v as u8]);
    }

    /// Emit a literal push using the smallest encoding that fits `n`:
    /// `PushZero` (1 B) for 0, `PushShort` (2 B) for i8 range, otherwise
    /// `Push` (5 B). Truncates to i32 if `n` overflows; the user is
    /// responsible for keeping literals in range.
    fn emit_literal(&mut self, n: i64) {
        if n == 0 {
            self.emit(&[op::PUSH_ZERO]);
        } else if (-128..=127).contains(&n) {
            self.emit_push_short(n as i8);
        } else {
            self.emit_push_long((n as i32) as u32);
        }
    }

    /// Emit a 6-byte `PUSH addr; CALL_ABS` call sequence.
    fn emit_call(&mut self, addr: u16) {
        self.emit_push_long(addr as u32);
        self.emit(&[op::CALL_ABS]);
    }

    /// Emit a 6-byte call with a placeholder address. Returns the
    /// 4-byte placeholder slot's offset so the caller can patch it later
    /// (used for forward references).
    fn emit_call_placeholder(&mut self) -> usize {
        self.emit(&[op::PUSH]);
        let pos = self.code.len();
        self.emit(&[0, 0, 0, 0]);
        self.emit(&[op::CALL_ABS]);
        pos
    }

    /// Patch a 4-byte little-endian u32 at the given position.
    fn write_u32_at(&mut self, pos: usize, value: u32) -> Result<(), String> {
        if pos + 4 > self.code.len() {
            return Err(format!(
                "write_u32_at out of range: pos={}, len={}", pos, self.code.len()));
        }
        self.code[pos..pos + 4].copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn write_i32_at(&mut self, pos: usize, value: i32) -> Result<(), String> {
        self.write_u32_at(pos, value as u32)
    }
}

// ---------------------------------------------------------------------------
// Preamble: bootstrap + helper routines
// ---------------------------------------------------------------------------

impl Compiler {
    /// Bootstrap (18 bytes): init the return-stack pointer, then
    /// `PUSH main; CALL_ABS; HALT`. The main address is patched at finalize.
    fn emit_bootstrap(&mut self) {
        self.emit_push_long(RETSTACK_INITIAL);
        self.emit_push_long(RSP_STORAGE_ADDR);
        self.emit(&[op::STORE_ABS]);
        self.emit(&[op::PUSH]);
        self.main_patch_pos = self.code.len();
        self.emit(&[0, 0, 0, 0]);
        self.emit(&[op::CALL_ABS, op::HALT]);
    }

    /// PROLOG_HELPER body (32 bytes). Each word's 6-byte prolog `PUSH addr;
    /// CALL_ABS` lands here with the data stack holding `[caller_RA, body_RA]`.
    /// We stash `body_RA` in `PROLOG_SCRATCH`, push `caller_RA` onto the
    /// retstack, then JUMP_ABS to `body_RA`.
    fn emit_prolog_helper(&mut self) {
        let start = self.code.len();
        // Stash body_RA in scratch.
        self.emit_push_long(PROLOG_SCRATCH_ADDR);
        self.emit(&[op::STORE_ABS]);
        // Push caller_RA onto the retstack: RSP -= 4; mem[RSP] = caller_RA.
        self.emit_push_long(RSP_STORAGE_ADDR);
        self.emit(&[op::LOAD_ABS]);                         // [caller_RA, old_RSP]
        self.emit(&[op::PUSH_SHORT, 0xFC, op::ADD]);        // ... new_RSP
        self.emit(&[op::PUSH_SHORT, 0xFC, op::LOAD_SP_REL]); // dup new_RSP
        self.emit_push_long(RSP_STORAGE_ADDR);
        self.emit(&[op::STORE_ABS]);                        // mem[RSP_STORAGE]=new_RSP
        self.emit(&[op::STORE_ABS]);                        // mem[new_RSP]=caller_RA
        // Reload body_RA from scratch and jump to it.
        self.emit_push_long(PROLOG_SCRATCH_ADDR);
        self.emit(&[op::LOAD_ABS, op::JUMP_ABS]);
        debug_assert_eq!(self.code.len() - start, 32);
    }

    /// EPILOG_HELPER body (20 bytes). Each word's 6-byte epilog `PUSH addr;
    /// JUMP_ABS` lands here; no RA is pushed (JUMP_ABS is a tail jump, not
    /// a call). We pop the saved `caller_RA` off the retstack and JUMP_ABS.
    fn emit_epilog_helper(&mut self) {
        let start = self.code.len();
        self.emit_push_long(RSP_STORAGE_ADDR);
        self.emit(&[op::LOAD_ABS]);                              // [old_RSP]
        self.emit(&[op::PUSH_SHORT, 0xFC, op::LOAD_SP_REL]);     // dup old_RSP
        self.emit(&[op::PUSH_SHORT, 0x04, op::ADD]);             // [old_RSP, new_RSP]
        self.emit_push_long(RSP_STORAGE_ADDR);
        self.emit(&[op::STORE_ABS]);                             // mem[RSP_STORAGE]=new_RSP
        self.emit(&[op::LOAD_ABS, op::JUMP_ABS]);                // pop & jump RA
        debug_assert_eq!(self.code.len() - start, 20);
    }

    /// Per-word prolog thunk (6 B): `PUSH PROLOG_HELPER_ADDR; CALL_ABS`.
    /// CALL_ABS pushes the body's start address as the RA — exactly what
    /// the helper needs to stash and jump to.
    fn emit_prolog(&mut self) {
        let start = self.code.len();
        self.emit_call(PROLOG_HELPER_ADDR);
        debug_assert_eq!(self.code.len() - start, PROLOG_LEN);
    }

    /// Per-word epilog thunk (6 B): `PUSH EPILOG_HELPER_ADDR; JUMP_ABS`.
    /// Tail jump (no RA push) so the helper just pops the saved caller_RA.
    fn emit_epilog(&mut self) {
        let start = self.code.len();
        self.emit_push_long(EPILOG_HELPER_ADDR as u32);
        self.emit(&[op::JUMP_ABS]);
        debug_assert_eq!(self.code.len() - start, EPILOG_LEN);
    }

    /// Walk `fwd_refs` for `name` and patch every recorded slot to `addr`.
    /// Called when a forward-referenced word finally gets a `:` body.
    fn resolve_fwd_refs(&mut self, name: &str, addr: u16) -> Result<(), String> {
        if let Some(positions) = self.fwd_refs.remove(name) {
            for pos in positions {
                self.write_u32_at(pos, addr as u32)
                    .map_err(|e| format!("fwd-ref `{}`: {}", name, e))?;
            }
        }
        Ok(())
    }

    /// Emit a call to the prelude's `swap`. Prelude must be compiled first
    /// — non-commutative binary ops (`/`, `mod`, `lshift`, `rshift`) all
    /// lower to `swap; OP`, so a missing `swap` is an internal bug.
    fn emit_swap_call(&mut self) {
        match self.dict.get("swap").copied() {
            Some(DictEntry::Word(addr)) => self.emit_call(addr),
            _ => panic!(
                "internal: `swap` missing from dictionary — \
                 prelude must be compiled before user code that uses \
                 non-commutative binary ops"
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Back-jump helpers (shared by `until`, `again`, `loop`)
// ---------------------------------------------------------------------------
//
// All three keywords emit a back-jump to `begin_pos`. The short form uses
// a 1-byte signed offset; the long form falls back to a 4-byte i32. The
// difference between them is just the surrounding ops:
//
//   `until`: LOGICAL_NOT; <push off>; MUL; JUMP_REL    (5 / 8 bytes)
//   `loop`:                <push off>; MUL; JUMP_REL   (4 / 7 bytes)
//   `again`:               <push off>;      JUMP_REL   (3 / 6 bytes)
//
// `until` flips the condition before MUL (loop while falsy); `loop` doesn't
// (loop while index<limit is truthy); `again` is unconditional and skips
// the MUL entirely.

impl Compiler {
    /// Conditional back-jump: `[LOGICAL_NOT?]; <push off>; MUL; JUMP_REL`.
    /// `flip` adds the LOGICAL_NOT prefix used by `until`.
    fn emit_back_jump_mul(&mut self, begin_pos: usize, flip: bool) -> Result<(), String> {
        if flip { self.emit(&[op::LOGICAL_NOT]); }
        let here = self.code.len();
        let off_short = (begin_pos as i64) - ((here + 4) as i64);
        if (-128..=127).contains(&off_short) {
            self.emit(&[op::PUSH_SHORT, off_short as i8 as u8, op::MUL, op::JUMP_REL]);
            return Ok(());
        }
        let off_long = (begin_pos as i64) - ((here + 7) as i64);
        if !(i32::MIN as i64..=i32::MAX as i64).contains(&off_long) {
            return Err(format!("back-jump out of i32 range: {}", off_long));
        }
        self.emit_push_long((off_long as i32) as u32);
        self.emit(&[op::MUL, op::JUMP_REL]);
        Ok(())
    }

    /// Unconditional back-jump: `<push off>; JUMP_REL`.
    fn emit_back_jump_uncond(&mut self, begin_pos: usize) -> Result<(), String> {
        let here = self.code.len();
        let off_short = (begin_pos as i64) - ((here + 3) as i64);
        if (-128..=127).contains(&off_short) {
            self.emit(&[op::PUSH_SHORT, off_short as i8 as u8, op::JUMP_REL]);
            return Ok(());
        }
        let off_long = (begin_pos as i64) - ((here + 6) as i64);
        if !(i32::MIN as i64..=i32::MAX as i64).contains(&off_long) {
            return Err(format!("back-jump out of i32 range: {}", off_long));
        }
        self.emit_push_long((off_long as i32) as u32);
        self.emit(&[op::JUMP_REL]);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Primitive table — single-byte ops, reflected comparisons, swap-then-op
// ---------------------------------------------------------------------------

impl Compiler {
    /// Try to compile `tok` as a primitive. Returns `true` if bytes were
    /// emitted, `false` if `tok` is not a primitive (caller falls through
    /// to dictionary lookup, then forward reference).
    ///
    /// **Operand order**: Forth `a b OP` means "left operand a, right
    /// operand b" (result = NOS OP TOS). The VM does the opposite per the
    /// spec — `left = pop()` is TOS, then `right = pop()` is NOS. Commutative
    /// ops hide the disagreement; non-commutative ones need a fixup
    /// (NEG for `-`, swap for `/`/`mod`/`lshift`/`rshift`, opcode reflection
    /// for ordered comparisons). Verified against examples/wander.asm L00e3
    /// (glibc LCG pattern).
    fn try_primitive(&mut self, tok: &str) -> bool {
        // (a) Single-byte ops: commutative binaries + everything that isn't
        // a non-commutative two-operand arithmetic/compare op. The reflected
        // comparisons (`<` → GT, etc.) live here too because the reflection
        // is itself just a different single opcode.
        let single = match tok {
            "+"        => op::ADD,
            "*"        => op::MUL,
            "and"      => op::AND,
            "or"       => op::OR,
            "xor"      => op::XOR,
            "="        => op::EQ,
            "<>"       => op::NE,
            "not"      => op::LOGICAL_NOT,
            "invert"   => op::BITWISE_NOT,
            "negate"   => op::NEG,
            "drop"     => op::POP,
            "@"        => op::LOAD_ABS,
            "!"        => op::STORE_ABS,
            "halt"     => op::HALT,
            "syscall"  => op::SYSCALL,
            "skip"     => op::SKIP,
            // Reflected comparisons: emit the opposite opcode so VM's
            // `left CMP right` (TOS CMP NOS) computes Forth's NOS CMP TOS.
            "<"        => op::GT,
            "<="       => op::GE,
            ">"        => op::LT,
            ">="       => op::LE,
            _          => return self.try_compound_primitive(tok),
        };
        self.emit(&[single]);
        true
    }

    /// Primitives that need more than one byte: `-` (SUB+NEG to flip
    /// subtraction direction), swap-then-op for the non-reflectable
    /// non-commutative ops, and SP-relative synthesis of `dup`/`over`.
    fn try_compound_primitive(&mut self, tok: &str) -> bool {
        // `-`: VM SUB is TOS-NOS; NEG flips to NOS-TOS = Forth's `a b -`.
        if tok == "-" {
            self.emit(&[op::SUB, op::NEG]);
            return true;
        }
        // `swap; OP` for the non-commutative ops with no reflected variant.
        if let Some(opcode) = match tok {
            "/"      => Some(op::DIV),
            "mod"    => Some(op::REM),
            "lshift" => Some(op::SHL),
            "rshift" => Some(op::SHR),
            _        => None,
        } {
            self.emit_swap_call();
            self.emit(&[opcode]);
            return true;
        }
        // SP-relative synthesis. `dup`: read mem[SP - 4]; `over`: mem[SP - 8].
        // Pushing the offset advances SP by 4, so the LOAD_SP_REL reads from
        // the slot just below the new SP — which is the original top (`dup`)
        // or the original NOS (`over`).
        let bytes: &[u8] = match tok {
            "dup"  => &[op::PUSH_SHORT, 0xFC, op::LOAD_SP_REL], // -4
            "over" => &[op::PUSH_SHORT, 0xF8, op::LOAD_SP_REL], // -8
            _      => return false,
        };
        self.emit(bytes);
        true
    }
}

// ---------------------------------------------------------------------------
// Control-flow keyword handlers
// ---------------------------------------------------------------------------
//
// Each compiles one structural keyword. They share a common discipline:
//
//   - opening keywords push a CtrlFrame
//   - closing keywords pop and pattern-match the expected frame variant,
//     erroring with a typed message on mismatch
//   - back-jump emission goes through the shared helpers above
//
// Branch placeholders (for `if`/`else`/`then`) use absolute byte positions
// in `self.code` — these survive intervening emissions because we never
// truncate or shift the buffer.

impl Compiler {
    /// `if`: emit conditional skip to be patched by `else`/`then`.
    /// Long form (8 B): `LOGICAL_NOT; PUSH <i32 ph>; MUL; JUMP_REL`. We
    /// always use the long form — i8 immediates are too small for arbitrary
    /// bodies, and the 3-byte overhead is negligible.
    fn compile_if(&mut self) {
        self.emit(&[op::LOGICAL_NOT, op::PUSH]);
        let placeholder_pos = self.code.len();
        self.emit(&[0, 0, 0, 0]);
        self.emit(&[op::MUL, op::JUMP_REL]);
        let body_start = self.code.len();
        self.ctrl.push(CtrlFrame::If { placeholder_pos, body_start });
    }

    /// `else`: patch the `if` placeholder to skip past the unconditional
    /// jump that we now emit, which will itself be patched by `then`.
    fn compile_else(&mut self) -> Result<(), String> {
        let (placeholder_pos, body_start) = match self.pop_ctrl("`else` with no matching `if`")? {
            CtrlFrame::If { placeholder_pos, body_start } => (placeholder_pos, body_start),
            _ => return Err("`else` requires `if` on top of control stack".into()),
        };
        let true_len = self.code.len() - body_start;
        // Unconditional long skip-over-else (6 B): PUSH <i32 ph>; JUMP_REL.
        self.emit(&[op::PUSH]);
        let ph2 = self.code.len();
        self.emit(&[0, 0, 0, 0]);
        self.emit(&[op::JUMP_REL]);
        let else_body_start = self.code.len();
        // Patch the `if` placeholder: skip the true body PLUS the 6-byte
        // skip-over-else we just emitted, landing at else_body_start.
        let if_offset = (true_len as i64) + 6;
        if !(i32::MIN as i64..=i32::MAX as i64).contains(&if_offset) {
            return Err(format!("if-true-body too long: {} bytes", true_len));
        }
        self.write_i32_at(placeholder_pos, if_offset as i32)?;
        self.ctrl.push(CtrlFrame::IfElse { placeholder_pos: ph2, else_body_start });
        Ok(())
    }

    /// `then`: patch the outstanding `if` or `else` placeholder with the
    /// branch-body length (so the JUMP_REL lands right after `then`).
    fn compile_then(&mut self) -> Result<(), String> {
        let (placeholder_pos, body_start) = match self.pop_ctrl("`then` with no matching `if`/`else`")? {
            CtrlFrame::If { placeholder_pos, body_start } => (placeholder_pos, body_start),
            CtrlFrame::IfElse { placeholder_pos, else_body_start } => (placeholder_pos, else_body_start),
            _ => return Err("`then` requires `if`/`else` on top".into()),
        };
        let len = (self.code.len() - body_start) as i64;
        if !(i32::MIN as i64..=i32::MAX as i64).contains(&len) {
            return Err(format!("branch body too long: {} bytes", len));
        }
        self.write_i32_at(placeholder_pos, len as i32)
    }

    /// `begin`: just records the back-branch target.
    fn compile_begin(&mut self) {
        self.ctrl.push(CtrlFrame::Begin { begin_pos: self.code.len() });
    }

    /// `until`: back-branch to `begin` while the cond on the stack is falsy.
    fn compile_until(&mut self) -> Result<(), String> {
        let begin_pos = match self.pop_ctrl("`until` with no matching `begin`")? {
            CtrlFrame::Begin { begin_pos } => begin_pos,
            _ => return Err("`until` requires `begin` on top".into()),
        };
        self.emit_back_jump_mul(begin_pos, /* flip = */ true)
    }

    /// `again`: unconditional back-branch to `begin`.
    fn compile_again(&mut self) -> Result<(), String> {
        let begin_pos = match self.pop_ctrl("`again` with no matching `begin`")? {
            CtrlFrame::Begin { begin_pos } => begin_pos,
            _ => return Err("`again` requires `begin` on top".into()),
        };
        self.emit_back_jump_uncond(begin_pos)
    }

    /// `do`: store start as the current index and limit alongside it in
    /// this nesting level's scratch pair. Pushes a `Do` frame so `loop` can
    /// find both the back-branch target and the slot indices.
    fn compile_do(&mut self) -> Result<(), String> {
        if self.do_depth >= DO_LOOP_MAX_DEPTH {
            return Err(format!(
                "`do` nesting too deep (max {}); raise DO_LOOP_MAX_DEPTH if you need more",
                DO_LOOP_MAX_DEPTH));
        }
        let depth = self.do_depth;
        let (index_addr, limit_addr) = do_slot_addrs(depth);
        // Stack before: [limit, start]. STORE_ABS pops addr (TOS), then val
        // (NOS) — so pushing index_addr stores `start` there, then pushing
        // limit_addr stores `limit`.
        self.emit_push_long(index_addr);
        self.emit(&[op::STORE_ABS]);
        self.emit_push_long(limit_addr);
        self.emit(&[op::STORE_ABS]);
        self.do_depth += 1;
        self.ctrl.push(CtrlFrame::Do { begin_pos: self.code.len(), depth });
        Ok(())
    }

    /// `loop`: increment the index, store back, compare against limit, and
    /// back-branch to `begin_pos` while index < limit.
    fn compile_loop(&mut self) -> Result<(), String> {
        let (begin_pos, depth) = match self.pop_ctrl("`loop` with no matching `do`")? {
            CtrlFrame::Do { begin_pos, depth } => (begin_pos, depth),
            _ => return Err("`loop` requires `do` on top".into()),
        };
        self.do_depth -= 1;
        let (index_addr, limit_addr) = do_slot_addrs(depth);
        // Increment-and-compare. Stack-effect annotation:
        //   PUSH index_addr; LOAD_ABS            ( -- old_idx )
        //   PUSH_SHORT 1; ADD                    ( -- new_idx )
        //   PUSH_SHORT -4; LOAD_SP_REL           ( -- new_idx new_idx )  inline dup
        //   PUSH index_addr; STORE_ABS           ( -- new_idx )
        //   PUSH limit_addr; LOAD_ABS            ( -- new_idx limit )
        //   GT                                   ( -- new_idx<limit )    reflected `<`
        self.emit_push_long(index_addr);
        self.emit(&[op::LOAD_ABS, op::PUSH_SHORT, 1, op::ADD]);
        self.emit(&[op::PUSH_SHORT, 0xFC, op::LOAD_SP_REL]);
        self.emit_push_long(index_addr);
        self.emit(&[op::STORE_ABS]);
        self.emit_push_long(limit_addr);
        self.emit(&[op::LOAD_ABS, op::GT]);
        // Back-branch while the comparison was truthy. Same MUL trick as
        // `until` but without the LOGICAL_NOT (we want truthy, not falsy).
        self.emit_back_jump_mul(begin_pos, /* flip = */ false)
    }

    /// `i`: push the innermost do-loop's current index.
    fn compile_i(&mut self) -> Result<(), String> {
        if self.do_depth == 0 {
            return Err("`i` used outside of a `do`/`loop`".into());
        }
        let (index_addr, _) = do_slot_addrs(self.do_depth - 1);
        self.emit_push_long(index_addr);
        self.emit(&[op::LOAD_ABS]);
        Ok(())
    }

    /// Pop the top control frame or return a typed error message.
    fn pop_ctrl(&mut self, msg: &'static str) -> Result<CtrlFrame, String> {
        self.ctrl.pop().ok_or_else(|| msg.to_string())
    }
}

/// (index_addr, limit_addr) for a given do-loop nesting depth.
fn do_slot_addrs(depth: usize) -> (u32, u32) {
    let index = DO_LOOP_BASE + (depth as u32) * 8;
    (index, index + 4)
}

// ---------------------------------------------------------------------------
// Per-token dispatch (within a word body)
// ---------------------------------------------------------------------------

impl Compiler {
    fn compile_token(&mut self, tok: &str) -> Result<(), String> {
        // 1. Numeric literal.
        if let Some(n) = parse_number(tok) {
            self.emit_literal(n);
            return Ok(());
        }
        // 2. Control-flow keyword.
        match tok {
            "exit"  => { self.emit_epilog();        return Ok(()); }
            "if"    => { self.compile_if();         return Ok(()); }
            "else"  => { return self.compile_else(); }
            "then"  => { return self.compile_then(); }
            "begin" => { self.compile_begin();      return Ok(()); }
            "until" => { return self.compile_until(); }
            "again" => { return self.compile_again(); }
            "do"    => { return self.compile_do(); }
            "loop"  => { return self.compile_loop(); }
            "i"     => { return self.compile_i(); }
            _ => {}
        }
        // 3. Primitive.
        if self.try_primitive(tok) {
            return Ok(());
        }
        // 4. Dictionary lookup — already defined word or value.
        if let Some(entry) = self.dict.get(tok).copied() {
            match entry {
                DictEntry::Word(addr) => self.emit_call(addr),
                DictEntry::Value(v)   => self.emit_literal(v),
            }
            return Ok(());
        }
        // 5. Forward reference: assume `tok` will be `:`-defined later as a
        // word. If it later becomes `variable`/`constant`, those branches
        // reject the conflict; if it's never defined, `finalize` reports it.
        let pos = self.emit_call_placeholder();
        self.fwd_refs.entry(tok.to_string()).or_default().push(pos);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Top-level token loop
// ---------------------------------------------------------------------------

impl Compiler {
    /// Compile a chunk of source. May be called repeatedly to append to the
    /// same compiler; call `finalize` once after all chunks to patch `main`
    /// and verify size.
    pub fn compile(&mut self, src: &str) -> Result<(), String> {
        let toks = tokenize(src);
        let mut i = 0;
        while i < toks.len() {
            let tok = &toks[i];
            match tok.as_str() {
                ":" => {
                    i += 1;
                    self.handle_colon(toks.get(i).map(String::as_str))?;
                }
                "variable" => {
                    i += 1;
                    self.handle_variable(toks.get(i).map(String::as_str))?;
                }
                "constant" => {
                    let name = toks.get(i + 1).map(String::as_str);
                    let value = toks.get(i + 2).map(String::as_str);
                    self.handle_constant(name, value)?;
                    i += 2;
                }
                ";" => self.handle_semicolon()?,
                other => {
                    if self.compiling.is_none() {
                        return Err(format!(
                            "token `{}` outside of a word definition \
                             (wrap your code in `: main ... ;`)",
                            other));
                    }
                    self.compile_token(other)?;
                }
            }
            i += 1;
        }
        Ok(())
    }

    fn handle_colon(&mut self, name: Option<&str>) -> Result<(), String> {
        if self.compiling.is_some() {
            return Err("nested `:` is not allowed".into());
        }
        let name = name.ok_or("`:` needs a name")?.to_string();
        if self.here() > 0xFFFF {
            return Err("binary already exceeds 64 KB before defining word".into());
        }
        let addr = self.here() as u16;
        self.dict.insert(name.clone(), DictEntry::Word(addr));
        self.resolve_fwd_refs(&name, addr)?;
        self.compiling = Some(name);
        self.emit_prolog();
        Ok(())
    }

    fn handle_variable(&mut self, name: Option<&str>) -> Result<(), String> {
        if self.compiling.is_some() {
            return Err("`variable` not allowed inside a word definition".into());
        }
        let name = name.ok_or("`variable` needs a name")?.to_string();
        // Forward refs emit a call, not a literal push — incompatible shape.
        if self.fwd_refs.contains_key(&name) {
            return Err(format!(
                "`{}` was forward-referenced as a word but is being defined as a variable",
                name));
        }
        let addr = self.code.len();
        if addr + 4 > 0xFFFF {
            return Err("not enough space to allocate variable".into());
        }
        self.emit(&[0, 0, 0, 0]);
        self.dict.insert(name, DictEntry::Value(addr as i64));
        Ok(())
    }

    fn handle_constant(&mut self, name: Option<&str>, value: Option<&str>) -> Result<(), String> {
        if self.compiling.is_some() {
            return Err("`constant` not allowed inside a word definition".into());
        }
        let name = name.ok_or("`constant` needs a name")?.to_string();
        let value_tok = value.ok_or("`constant` needs a value")?;
        let value = parse_number(value_tok)
            .ok_or_else(|| format!("`constant`: bad value `{}`", value_tok))?;
        if self.fwd_refs.contains_key(&name) {
            return Err(format!(
                "`{}` was forward-referenced as a word but is being defined as a constant",
                name));
        }
        self.dict.insert(name, DictEntry::Value(value));
        Ok(())
    }

    fn handle_semicolon(&mut self) -> Result<(), String> {
        if self.compiling.is_none() {
            return Err("`;` with no matching `:`".into());
        }
        if !self.ctrl.is_empty() {
            return Err("`;` while a control block is open (if without then?)".into());
        }
        self.emit_epilog();
        self.compiling = None;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Finalization
// ---------------------------------------------------------------------------

impl Compiler {
    /// Patch `main`'s address into the bootstrap and validate the binary
    /// size and forward-ref completeness. Call once after all `compile()`
    /// chunks.
    pub fn finalize(&mut self) -> Result<(), String> {
        if let Some(name) = &self.compiling {
            return Err(format!("unclosed definition: `{}` missing `;`", name));
        }
        if !self.fwd_refs.is_empty() {
            let mut names: Vec<&String> = self.fwd_refs.keys().collect();
            names.sort();
            return Err(format!(
                "unresolved forward references: {}",
                names.iter().map(|s| format!("`{}`", s)).collect::<Vec<_>>().join(", ")));
        }
        let main_addr = match self.dict.get("main").copied() {
            Some(DictEntry::Word(a)) => a,
            Some(DictEntry::Value(_)) => return Err(
                "`main` must be a word (`: main ... ;`), not a constant/variable".into()),
            None => return Err("no `main` word defined".into()),
        };
        self.write_u32_at(self.main_patch_pos, main_addr as u32)?;
        if self.code.len() > MAX_CODE_END {
            return Err(format!(
                "binary too large: {} bytes (max {} = 0x{:04x}; \
                 higher would let the return stack at 0x7000..0x7FC4 \
                 overwrite code/data at runtime)",
                self.code.len(), MAX_CODE_END, MAX_CODE_END));
        }
        Ok(())
    }
}

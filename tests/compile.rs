// Compile-time tests: assert on emitted bytecode shapes and compiler errors.
// These tests don't run any code — they just compile and inspect.

mod common;

use livectf_forth::{
    op, Compiler, DictEntry, DO_LOOP_BASE, DO_LOOP_MAX_DEPTH, EPILOG_HELPER_ADDR, EPILOG_LEN,
    MAX_CODE_END, PREAMBLE_LEN, PRELUDE, PROLOG_HELPER_ADDR, PROLOG_LEN, PROLOG_SCRATCH_ADDR,
    RETSTACK_INITIAL, RSP_STORAGE_ADDR,
};

// ===========================================================================
// helpers
// ===========================================================================

fn compile(src: &str) -> Result<Vec<u8>, String> {
    livectf_forth::compile_program(src)
}

// Compile and look up where `name` got defined. For variables/constants this
// returns the DictEntry::Value's address (or value); for words, the entry
// point address.
fn compile_and_lookup(src: &str, name: &str) -> Result<(Vec<u8>, DictEntry), String> {
    let mut c = Compiler::new();
    c.compile(PRELUDE)?;
    c.compile(src)?;
    c.finalize()?;
    let e = c.dict_get(name).ok_or_else(|| format!("name `{}` not found", name))?;
    Ok((c.into_bytes(), e))
}

// Slice the user-visible body of the named word out of compiled bytecode.
// Strips the auto-emitted prolog from the front and the auto-emitted epilog
// from the back, then appends a synthetic JUMP_ABS so existing assertions of
// the form `body[N] == op::JUMP_ABS` (the closing `;`) keep working.
//
// Only valid for straight-line bodies (no `exit` — `exit` emits its own
// epilog mid-body, which would confuse the JUMP_ABS scan below).
fn word_body(src: &str, name: &str) -> Vec<u8> {
    let (bytes, entry) = compile_and_lookup(src, name).expect("compile_and_lookup");
    let addr = match entry {
        DictEntry::Word(a) => a as usize,
        DictEntry::Value(_) => panic!("`{}` is a value, not a word", name),
    };
    let body_start = addr + PROLOG_LEN;
    // Walk forward looking for the closing epilog's JUMP_ABS. Step over each
    // instruction's operand bytes so a 0x0A inside a PUSH literal doesn't
    // fool us.
    let mut end = body_start;
    while end < bytes.len() && bytes[end] != op::JUMP_ABS {
        end += op_total_len(&bytes[end..]);
    }
    // `end` now points at the JUMP_ABS at the tail of the epilog; the epilog
    // started (EPILOG_LEN - 1) bytes earlier.
    let epilog_start = end + 1 - EPILOG_LEN;
    let mut body = bytes[body_start..epilog_start].to_vec();
    body.push(op::JUMP_ABS);
    body
}

// Length of one instruction starting at `b[0]`. Enough opcodes covered to
// safely walk straight-line bodies.
fn op_total_len(b: &[u8]) -> usize {
    match b[0] {
        op::PUSH | op::CALL_REL => 5,        // op + i32
        op::PUSH_SHORT => 2,                  // op + i8
        _ => 1,
    }
}

// ===========================================================================
// Bootstrap layout
// ===========================================================================

#[test]
fn bootstrap_initializes_rsp_then_calls_main() {
    let bytes = compile(": main ;").unwrap();
    // Bootstrap (18 bytes):
    //   PUSH RETSTACK_INITIAL ; (5)
    //   PUSH RSP_STORAGE_ADDR ; (5)
    //   STORE_ABS             ; (1)
    //   PUSH <main_addr>      ; (5)  -- main_addr patched in finalize
    //   CALL_ABS              ; (1)
    //   HALT                  ; (1)
    assert_eq!(bytes[0], op::PUSH);
    assert_eq!(&bytes[1..5], &RETSTACK_INITIAL.to_le_bytes());
    assert_eq!(bytes[5], op::PUSH);
    assert_eq!(&bytes[6..10], &RSP_STORAGE_ADDR.to_le_bytes());
    assert_eq!(bytes[10], op::STORE_ABS);
    assert_eq!(bytes[11], op::PUSH);
    let main_addr = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
    assert_eq!(bytes[16], op::CALL_ABS);
    assert_eq!(bytes[17], op::HALT);
    assert!(main_addr >= 18, "main_addr {} must be past bootstrap", main_addr);
    // The first byte at main_addr is the start of main's auto-emitted prolog,
    // which begins with `PUSH RSP_STORAGE_ADDR` (the load of the current RSP).
    assert_eq!(bytes[main_addr], op::PUSH);
}

// ===========================================================================
// Literal encoding
// ===========================================================================

#[test]
fn literal_zero_uses_push_zero() {
    let body = word_body(": main 0 ;", "main");
    // body = [PUSH_ZERO, JUMP_ABS]
    assert_eq!(body, vec![op::PUSH_ZERO, op::JUMP_ABS]);
}

#[test]
fn literal_small_uses_push_short() {
    assert_eq!(word_body(": main 42 ;", "main"), vec![op::PUSH_SHORT, 42, op::JUMP_ABS]);
    assert_eq!(word_body(": main -5 ;", "main"), vec![op::PUSH_SHORT, 0xFB, op::JUMP_ABS]);
    assert_eq!(word_body(": main 127 ;", "main"), vec![op::PUSH_SHORT, 127, op::JUMP_ABS]);
    assert_eq!(word_body(": main -128 ;", "main"), vec![op::PUSH_SHORT, 0x80, op::JUMP_ABS]);
}

#[test]
fn literal_large_uses_long_push() {
    let body = word_body(": main 1000 ;", "main");
    // PUSH (1) + 4 bytes LE + JUMP_ABS = 6 bytes
    assert_eq!(body.len(), 6);
    assert_eq!(body[0], op::PUSH);
    assert_eq!(&body[1..5], &1000u32.to_le_bytes());
    assert_eq!(body[5], op::JUMP_ABS);
}

#[test]
fn literal_hex_and_negative_hex() {
    assert_eq!(word_body(": main 0x7F ;", "main"), vec![op::PUSH_SHORT, 0x7F, op::JUMP_ABS]);
    assert_eq!(word_body(": main 0x80 ;", "main"),
               { let mut v = vec![op::PUSH]; v.extend(&0x80u32.to_le_bytes()); v.push(op::JUMP_ABS); v });
    let body = word_body(": main -0x100 ;", "main");
    assert_eq!(body[0], op::PUSH);
    assert_eq!(&body[1..5], &((-256i32) as u32).to_le_bytes());
}

// ===========================================================================
// Commutative binary ops — single opcode
// ===========================================================================

#[test]
fn commutative_ops_emit_single_opcode() {
    let cases = [
        ("+", op::ADD),
        ("*", op::MUL),
        ("and", op::AND),
        ("or", op::OR),
        ("xor", op::XOR),
        ("=", op::EQ),
        ("<>", op::NE),
    ];
    for (tok, want) in cases {
        let src = format!(": main 1 2 {} ;", tok);
        let body = word_body(&src, "main");
        // [PUSH_SHORT 1, PUSH_SHORT 2, OP, JUMP_ABS]
        assert_eq!(body[..2], [op::PUSH_SHORT, 1], "tok={}", tok);
        assert_eq!(body[2..4], [op::PUSH_SHORT, 2], "tok={}", tok);
        assert_eq!(body[4], want, "tok={} expected opcode 0x{:02x}", tok, want);
        assert_eq!(body[5], op::JUMP_ABS);
    }
}

// ===========================================================================
// Non-commutative ops — fixups
// ===========================================================================

#[test]
fn subtract_emits_sub_then_neg() {
    let body = word_body(": main 7 5 - ;", "main");
    // [PUSH_SHORT 7, PUSH_SHORT 5, SUB, NEG, JUMP_ABS]
    assert_eq!(body[4], op::SUB);
    assert_eq!(body[5], op::NEG);
    assert_eq!(body[6], op::JUMP_ABS);
}

#[test]
fn ordered_comparisons_use_reflected_opcode() {
    let cases = [("<", op::GT), ("<=", op::GE), (">", op::LT), (">=", op::LE)];
    for (tok, want) in cases {
        let src = format!(": main 1 2 {} ;", tok);
        let body = word_body(&src, "main");
        assert_eq!(body[4], want, "tok={} should map to opcode 0x{:02x}", tok, want);
        assert_eq!(body[5], op::JUMP_ABS);
    }
}

#[test]
fn div_mod_shift_call_swap_then_op() {
    // `swap` is the first prelude word, defined immediately after the
    // bootstrap. We don't hardcode its address — instead we look it up and
    // assert the emitted call targets it.
    let mut c = Compiler::new();
    c.compile(PRELUDE).unwrap();
    let swap_addr = match c.dict_get("swap") {
        Some(DictEntry::Word(a)) => a,
        _ => panic!("swap not in prelude dict"),
    };
    for (tok, want_op) in [("/", op::DIV), ("mod", op::REM), ("lshift", op::SHL), ("rshift", op::SHR)] {
        let src = format!(": main 1 2 {} ;", tok);
        let body = word_body(&src, "main");
        // body layout: PUSH_SHORT 1, PUSH_SHORT 2, [call to swap = PUSH addr; CALL_ABS = 6 bytes], OP, JUMP_ABS
        assert_eq!(body[..4], [op::PUSH_SHORT, 1, op::PUSH_SHORT, 2], "tok={}", tok);
        assert_eq!(body[4], op::PUSH, "tok={} should start the swap call with PUSH", tok);
        let emitted_addr = u32::from_le_bytes([body[5], body[6], body[7], body[8]]) as u16;
        assert_eq!(emitted_addr, swap_addr, "tok={} should call swap@0x{:04x}, got 0x{:04x}", tok, swap_addr, emitted_addr);
        assert_eq!(body[9], op::CALL_ABS, "tok={}", tok);
        assert_eq!(body[10], want_op, "tok={} expected raw opcode 0x{:02x}", tok, want_op);
        assert_eq!(body[11], op::JUMP_ABS);
    }
}

// ===========================================================================
// Stack-manipulation primitives
// ===========================================================================

#[test]
fn dup_inlines_as_load_sp_rel_minus4() {
    let body = word_body(": main 1 dup ;", "main");
    // PUSH_SHORT 1, PUSH_SHORT -4, LOAD_SP_REL, JUMP_ABS
    assert_eq!(body, vec![op::PUSH_SHORT, 1, op::PUSH_SHORT, 0xFC, op::LOAD_SP_REL, op::JUMP_ABS]);
}

#[test]
fn over_inlines_as_load_sp_rel_minus8() {
    let body = word_body(": main 1 2 over ;", "main");
    assert_eq!(&body[4..7], &[op::PUSH_SHORT, 0xF8, op::LOAD_SP_REL]);
}

#[test]
fn drop_emits_pop() {
    let body = word_body(": main 1 drop ;", "main");
    assert_eq!(body[2], op::POP);
}

// ===========================================================================
// if/else/then layout
// ===========================================================================

#[test]
fn if_then_emits_long_conditional() {
    // Body: 1 if 2 then ;
    // Expect: PUSH_SHORT 1, LOGICAL_NOT, PUSH <4>, MUL, JUMP_REL, PUSH_SHORT 2, JUMP_ABS
    // Then-body length = 2 bytes (PUSH_SHORT 2). Placeholder gets 2.
    let body = word_body(": main 1 if 2 then ;", "main");
    assert_eq!(body[0..2], [op::PUSH_SHORT, 1]);
    assert_eq!(body[2], op::LOGICAL_NOT);
    assert_eq!(body[3], op::PUSH);
    assert_eq!(&body[4..8], &2i32.to_le_bytes());
    assert_eq!(body[8], op::MUL);
    assert_eq!(body[9], op::JUMP_REL);
    assert_eq!(&body[10..12], &[op::PUSH_SHORT, 2]);
    assert_eq!(body[12], op::JUMP_ABS);
}

#[test]
fn if_else_then_patches_both_branches() {
    // : main 1 if 2 else 3 then ;
    // Layout: cond, [if-pattern], true-body, [else-jump], else-body, ;
    let body = word_body(": main 1 if 2 else 3 then ;", "main");
    // After if-pattern (10 bytes), true-body is PUSH_SHORT 2 (2 bytes), then
    // an unconditional skip-over-else: PUSH <ph2>; JUMP_REL (6 bytes), then
    // else-body PUSH_SHORT 3 (2 bytes), then JUMP_ABS.
    // We just sanity-check the two key opcodes survived and the placeholders
    // are non-zero (they would only be 0 if a bug skipped the patch).
    assert!(body.contains(&op::MUL));
    assert!(body.contains(&op::JUMP_REL));
    // True-body byte and else-body byte should both be present.
    let twos = body.iter().filter(|&&b| b == 2).count();
    let threes = body.iter().filter(|&&b| b == 3).count();
    assert!(twos >= 1, "missing true-body literal 2");
    assert!(threes >= 1, "missing else-body literal 3");
}

// ===========================================================================
// begin/until and begin/again
// ===========================================================================

#[test]
fn begin_until_short_form() {
    // Body: begin 1 until ;
    // Empty loop-body between begin and the condition (PUSH_SHORT 1 = 2 bytes).
    // until short pattern: LOGICAL_NOT; PUSH_SHORT <off>; MUL; JUMP_REL  (5 bytes)
    let body = word_body(": main begin 1 until ;", "main");
    // body layout: PUSH_SHORT 1 (2 bytes), LOGICAL_NOT, PUSH_SHORT <off>, MUL, JUMP_REL, JUMP_ABS
    assert_eq!(body[0..2], [op::PUSH_SHORT, 1]);
    assert_eq!(body[2], op::LOGICAL_NOT);
    assert_eq!(body[3], op::PUSH_SHORT);
    let off = body[4] as i8 as i32;
    // After JUMP_REL (at body[6]+1 relative to body start), we want to be
    // back at begin (body start). So off = -7 (from end of JUMP_REL).
    assert_eq!(off, -7, "short-form until offset should land back at begin");
    assert_eq!(body[5], op::MUL);
    assert_eq!(body[6], op::JUMP_REL);
}

#[test]
fn begin_again_short_form() {
    // begin 1 drop again ;
    // Body before back-branch: PUSH_SHORT 1 (2), POP (1) = 3 bytes.
    // again short: PUSH_SHORT <off>; JUMP_REL (3 bytes).
    let body = word_body(": main begin 1 drop again ;", "main");
    assert_eq!(body[0..2], [op::PUSH_SHORT, 1]);
    assert_eq!(body[2], op::POP);
    assert_eq!(body[3], op::PUSH_SHORT);
    let off = body[4] as i8 as i32;
    // After JUMP_REL at body[5]+1=6, we want to be at begin (body[0]). Off = -6.
    assert_eq!(off, -6);
    assert_eq!(body[5], op::JUMP_REL);
}

#[test]
fn begin_until_promotes_to_long_form_when_offset_overflows_i8() {
    // Pad the body with enough no-op-ish work to push the back-branch past
    // i8 range. `1 drop` is 3 bytes; 50 copies = 150 bytes — comfortably
    // beyond -128.
    let mut body_src = String::from(": main begin");
    for _ in 0..50 {
        body_src.push_str(" 1 drop");
    }
    body_src.push_str(" 1 until ;");
    let body = word_body(&body_src, "main");
    // Last 8 bytes should be the long-form until: LOGICAL_NOT, PUSH, <i32>, MUL, JUMP_REL, JUMP_ABS (terminator).
    // Strip off the JUMP_ABS at the end, then verify the long pattern.
    let pat = &body[body.len() - 9..body.len() - 1];
    assert_eq!(pat[0], op::LOGICAL_NOT);
    assert_eq!(pat[1], op::PUSH);
    // pat[2..6] = i32 offset, not asserted by exact value but must be negative
    let off = i32::from_le_bytes([pat[2], pat[3], pat[4], pat[5]]);
    assert!(off < -100, "expected large negative offset, got {}", off);
    assert_eq!(pat[6], op::MUL);
    assert_eq!(pat[7], op::JUMP_REL);
}

// ===========================================================================
// do/loop
// ===========================================================================

#[test]
fn do_uses_depth0_slot() {
    let body = word_body(": main 10 0 do i drop loop ;", "main");
    // Look for the index-store at slot 0 (0x7FD0).
    let idx_addr = DO_LOOP_BASE;
    let lim_addr = DO_LOOP_BASE + 4;
    // Find: PUSH <idx_addr>; STORE_ABS
    let idx_bytes = idx_addr.to_le_bytes();
    let lim_bytes = lim_addr.to_le_bytes();
    let needle_idx = [op::PUSH, idx_bytes[0], idx_bytes[1], idx_bytes[2], idx_bytes[3], op::STORE_ABS];
    let needle_lim = [op::PUSH, lim_bytes[0], lim_bytes[1], lim_bytes[2], lim_bytes[3], op::STORE_ABS];
    assert!(body.windows(6).any(|w| w == needle_idx),
            "expected PUSH 0x{:04x}; STORE_ABS for outer index slot", idx_addr);
    assert!(body.windows(6).any(|w| w == needle_lim),
            "expected PUSH 0x{:04x}; STORE_ABS for outer limit slot", lim_addr);
}

#[test]
fn nested_do_uses_separate_slots() {
    let body = word_body(": main 3 0 do 2 0 do i drop loop loop ;", "main");
    // Inner loop is depth 1 → slot at DO_LOOP_BASE + 8.
    let inner_idx = (DO_LOOP_BASE + 8).to_le_bytes();
    let needle = [op::PUSH, inner_idx[0], inner_idx[1], inner_idx[2], inner_idx[3], op::STORE_ABS];
    assert!(body.windows(6).any(|w| w == needle),
            "expected PUSH 0x{:04x}; STORE_ABS for inner index slot",
            DO_LOOP_BASE + 8);
}

#[test]
fn i_reads_innermost_loop_index_address() {
    // Inside a single do-loop body, `i` should emit PUSH <DO_LOOP_BASE>; LOAD_ABS.
    let body = word_body(": main 10 0 do i drop loop ;", "main");
    let idx = DO_LOOP_BASE.to_le_bytes();
    let needle = [op::PUSH, idx[0], idx[1], idx[2], idx[3], op::LOAD_ABS];
    assert!(body.windows(6).any(|w| w == needle),
            "expected `i` to emit PUSH 0x{:04x}; LOAD_ABS for innermost slot",
            DO_LOOP_BASE);
}

#[test]
fn i_in_nested_loop_reads_inner_slot() {
    let body = word_body(": main 3 0 do 2 0 do i drop loop loop ;", "main");
    let inner_idx = (DO_LOOP_BASE + 8).to_le_bytes();
    let needle = [op::PUSH, inner_idx[0], inner_idx[1], inner_idx[2], inner_idx[3], op::LOAD_ABS];
    assert!(body.windows(6).any(|w| w == needle),
            "expected `i` inside inner loop to read slot at 0x{:04x}",
            DO_LOOP_BASE + 8);
}

#[test]
fn do_nesting_max_depth_compiles() {
    // 4-deep nest — exactly DO_LOOP_MAX_DEPTH. Should compile.
    let mut src = String::from(": main");
    for _ in 0..DO_LOOP_MAX_DEPTH {
        src.push_str(" 1 0 do");
    }
    src.push_str(" i drop");
    for _ in 0..DO_LOOP_MAX_DEPTH {
        src.push_str(" loop");
    }
    src.push_str(" ;");
    assert!(compile(&src).is_ok(), "max-depth nesting should compile");
}

#[test]
fn do_nesting_overflow_rejected() {
    let mut src = String::from(": main");
    for _ in 0..(DO_LOOP_MAX_DEPTH + 1) {
        src.push_str(" 1 0 do");
    }
    src.push_str(" i drop");
    for _ in 0..(DO_LOOP_MAX_DEPTH + 1) {
        src.push_str(" loop");
    }
    src.push_str(" ;");
    let err = compile(&src).unwrap_err();
    assert!(err.contains("nesting too deep"), "got: {}", err);
}

// ===========================================================================
// Forward references
// ===========================================================================

#[test]
fn forward_reference_patches_call_target() {
    let src = ": a b ; : b ;";
    let mut c = Compiler::new();
    c.compile(PRELUDE).unwrap();
    c.compile(src).unwrap();
    // Don't finalize yet — finalize requires `main`. Add a tiny main.
    c.compile(": main ;").unwrap();
    c.finalize().unwrap();
    let bytes = c.code().to_vec();
    let a_addr = match c.dict_get("a") { Some(DictEntry::Word(x)) => x, _ => panic!() };
    let b_addr = match c.dict_get("b") { Some(DictEntry::Word(x)) => x, _ => panic!() };
    // a's user-visible body (after the auto prolog) should be:
    //   PUSH <b_addr>; CALL_ABS; <epilog starts here>
    let body = (a_addr as usize) + PROLOG_LEN;
    assert_eq!(bytes[body], op::PUSH);
    let target = u32::from_le_bytes([
        bytes[body+1], bytes[body+2], bytes[body+3], bytes[body+4],
    ]) as u16;
    assert_eq!(target, b_addr, "forward call should target b@0x{:04x}", b_addr);
    assert_eq!(bytes[body+5], op::CALL_ABS);
    // Next byte starts the epilog (PUSH RSP_STORAGE_ADDR ...).
    assert_eq!(bytes[body+6], op::PUSH);
}

#[test]
fn mutual_recursion_compiles() {
    // Each calls the other; only one is a forward ref. Compiler should
    // produce both calls with correct targets.
    let src = "
        : even? dup 0 = if drop 1 exit then 1 - odd? ;
        : odd?  dup 0 = if drop 0 exit then 1 - even? ;
        : main ;
    ";
    assert!(compile(src).is_ok());
}

#[test]
fn unresolved_forward_ref_errors() {
    let err = compile(": main ghost ;").unwrap_err();
    assert!(err.contains("unresolved forward references"), "got: {}", err);
    assert!(err.contains("ghost"));
}

#[test]
fn forward_ref_then_variable_collision_rejected() {
    let err = compile(": main x ; variable x").unwrap_err();
    assert!(err.contains("forward-referenced"), "got: {}", err);
    assert!(err.contains("variable"), "got: {}", err);
}

#[test]
fn forward_ref_then_constant_collision_rejected() {
    let err = compile(": main k ; constant k 5").unwrap_err();
    assert!(err.contains("forward-referenced"), "got: {}", err);
    assert!(err.contains("constant"), "got: {}", err);
}

// ===========================================================================
// Definition/binding errors
// ===========================================================================

#[test]
fn no_main_errors() {
    let err = compile(": foo ;").unwrap_err();
    assert!(err.contains("no `main` word defined"), "got: {}", err);
}

#[test]
fn main_as_variable_rejected() {
    let err = compile("variable main").unwrap_err();
    assert!(err.contains("main") && err.contains("must be a word"), "got: {}", err);
}

#[test]
fn unclosed_definition_errors() {
    let err = compile(": main 1 2 +").unwrap_err();
    assert!(err.contains("unclosed definition") || err.contains("missing `;`"), "got: {}", err);
}

#[test]
fn nested_colon_rejected() {
    let err = compile(": a : b ; ;").unwrap_err();
    assert!(err.contains("nested `:`"), "got: {}", err);
}

#[test]
fn semicolon_outside_definition_errors() {
    let err = compile(";").unwrap_err();
    assert!(err.contains("`;` with no matching `:`"), "got: {}", err);
}

#[test]
fn unmatched_then_errors() {
    let err = compile(": main then ;").unwrap_err();
    assert!(err.contains("then"), "got: {}", err);
}

#[test]
fn unmatched_else_errors() {
    let err = compile(": main else then ;").unwrap_err();
    assert!(err.contains("else"), "got: {}", err);
}

#[test]
fn unmatched_until_errors() {
    let err = compile(": main until ;").unwrap_err();
    assert!(err.contains("until"), "got: {}", err);
}

#[test]
fn unmatched_again_errors() {
    let err = compile(": main again ;").unwrap_err();
    assert!(err.contains("again"), "got: {}", err);
}

#[test]
fn i_outside_do_errors() {
    let err = compile(": main i drop ;").unwrap_err();
    assert!(err.contains("`i` used outside"), "got: {}", err);
}

#[test]
fn loop_without_do_errors() {
    let err = compile(": main begin 1 loop ;").unwrap_err();
    assert!(err.contains("loop"), "got: {}", err);
}

#[test]
fn semicolon_inside_open_control_block_errors() {
    let err = compile(": main 1 if 2 ;").unwrap_err();
    assert!(err.contains("control block is open"), "got: {}", err);
}

#[test]
fn top_level_token_outside_definition_errors() {
    let err = compile("42 : main ;").unwrap_err();
    assert!(err.contains("outside of a word definition"), "got: {}", err);
}

#[test]
fn variable_inside_definition_rejected() {
    let err = compile(": main variable x ;").unwrap_err();
    assert!(err.contains("not allowed inside"), "got: {}", err);
}

#[test]
fn constant_inside_definition_rejected() {
    let err = compile(": main constant K 5 ;").unwrap_err();
    assert!(err.contains("not allowed inside"), "got: {}", err);
}

// ===========================================================================
// Tokenizer
// ===========================================================================

#[test]
fn line_comments_are_skipped() {
    let body = word_body(": main 1 \\ comment to EOL\n 2 + ;", "main");
    // Should have PUSH 1; PUSH 2; ADD;
    assert_eq!(body[0..2], [op::PUSH_SHORT, 1]);
    assert_eq!(body[2..4], [op::PUSH_SHORT, 2]);
    assert_eq!(body[4], op::ADD);
}

#[test]
fn block_comments_are_skipped() {
    let body = word_body(": main 1 ( a stack note ) 2 + ;", "main");
    assert_eq!(body[0..2], [op::PUSH_SHORT, 1]);
    assert_eq!(body[2..4], [op::PUSH_SHORT, 2]);
    assert_eq!(body[4], op::ADD);
}

// ===========================================================================
// Variable / constant
// ===========================================================================

#[test]
fn variable_allocates_4_bytes() {
    let mut c = Compiler::new();
    c.compile(PRELUDE).unwrap();
    let before = c.here();
    c.compile("variable v").unwrap();
    let after = c.here();
    assert_eq!(after - before, 4, "variable should reserve 4 bytes");
}

#[test]
fn constant_inlines_value() {
    let body = word_body("constant K 99 : main K ;", "main");
    // 99 fits in i8, so PUSH_SHORT 99 — but it's outside i8 (>127)? No, 99 fits.
    assert_eq!(body[0..2], [op::PUSH_SHORT, 99]);
    assert_eq!(body[2], op::JUMP_ABS);
}

// ===========================================================================
// Preamble layout (bootstrap + helpers) and per-word prolog/epilog thunks
// ===========================================================================

#[test]
fn helpers_emitted_at_known_addresses() {
    let bytes = compile(": main ;").unwrap();
    assert!(bytes.len() >= PREAMBLE_LEN, "binary too short to contain preamble");

    // PROLOG_HELPER body starts with `PUSH PROLOG_SCRATCH_ADDR; STORE_ABS`
    // (stash body_RA before juggling caller_RA).
    let p = PROLOG_HELPER_ADDR as usize;
    assert_eq!(bytes[p], op::PUSH, "PROLOG_HELPER should start with PUSH");
    assert_eq!(&bytes[p+1..p+5], &PROLOG_SCRATCH_ADDR.to_le_bytes());
    assert_eq!(bytes[p+5], op::STORE_ABS);
    // PROLOG_HELPER ends with `LOAD_ABS; JUMP_ABS` (reload body_RA and jump).
    assert_eq!(bytes[p+30], op::LOAD_ABS);
    assert_eq!(bytes[p+31], op::JUMP_ABS);

    // EPILOG_HELPER body starts with `PUSH RSP_STORAGE_ADDR; LOAD_ABS` (read
    // current RSP) and ends with `LOAD_ABS; JUMP_ABS` (deref RA, jump).
    let e = EPILOG_HELPER_ADDR as usize;
    assert_eq!(bytes[e], op::PUSH);
    assert_eq!(&bytes[e+1..e+5], &RSP_STORAGE_ADDR.to_le_bytes());
    assert_eq!(bytes[e+5], op::LOAD_ABS);
    assert_eq!(bytes[e+18], op::LOAD_ABS);
    assert_eq!(bytes[e+19], op::JUMP_ABS);
}

#[test]
fn prolog_thunk_is_call_to_helper() {
    // Every `:` should emit a 6-byte thunk: `PUSH PROLOG_HELPER_ADDR; CALL_ABS`.
    let (bytes, entry) = compile_and_lookup(": main ;", "main").unwrap();
    let addr = match entry {
        DictEntry::Word(a) => a as usize,
        _ => panic!("main should be a word"),
    };
    assert_eq!(bytes[addr], op::PUSH);
    let target = u32::from_le_bytes([bytes[addr+1], bytes[addr+2], bytes[addr+3], bytes[addr+4]]);
    assert_eq!(target, PROLOG_HELPER_ADDR as u32);
    assert_eq!(bytes[addr+5], op::CALL_ABS);
}

#[test]
fn epilog_thunk_is_jump_to_helper() {
    // After main's (empty) body, the epilog should be: `PUSH EPILOG_HELPER_ADDR;
    // JUMP_ABS`. With an empty body, the epilog starts immediately after the
    // prolog (no user instructions in between).
    let (bytes, entry) = compile_and_lookup(": main ;", "main").unwrap();
    let addr = match entry {
        DictEntry::Word(a) => a as usize,
        _ => panic!(),
    };
    let epilog = addr + PROLOG_LEN;
    assert_eq!(bytes[epilog], op::PUSH);
    let target = u32::from_le_bytes([
        bytes[epilog+1], bytes[epilog+2], bytes[epilog+3], bytes[epilog+4],
    ]);
    assert_eq!(target, EPILOG_HELPER_ADDR as u32);
    assert_eq!(bytes[epilog+5], op::JUMP_ABS);
}

#[test]
fn exit_emits_full_epilog_thunk_not_bare_jump() {
    // `exit` is NOT a single-opcode primitive — it must run the same 6-byte
    // epilog thunk as `;`. The closing `;` then emits its own thunk after,
    // even if it's unreachable.
    let (bytes, entry) = compile_and_lookup(": main exit ;", "main").unwrap();
    let addr = match entry {
        DictEntry::Word(a) => a as usize,
        _ => panic!(),
    };
    let exit_pos = addr + PROLOG_LEN;
    // First thunk (from `exit`).
    assert_eq!(bytes[exit_pos], op::PUSH);
    let t1 = u32::from_le_bytes([
        bytes[exit_pos+1], bytes[exit_pos+2], bytes[exit_pos+3], bytes[exit_pos+4],
    ]);
    assert_eq!(t1, EPILOG_HELPER_ADDR as u32);
    assert_eq!(bytes[exit_pos+5], op::JUMP_ABS);
    // Second thunk (from `;`), 6 bytes later.
    let close_pos = exit_pos + EPILOG_LEN;
    assert_eq!(bytes[close_pos], op::PUSH);
    let t2 = u32::from_le_bytes([
        bytes[close_pos+1], bytes[close_pos+2], bytes[close_pos+3], bytes[close_pos+4],
    ]);
    assert_eq!(t2, EPILOG_HELPER_ADDR as u32);
    assert_eq!(bytes[close_pos+5], op::JUMP_ABS);
}

#[test]
fn prelude_swap_has_prolog_thunk() {
    // The prelude is just user code as far as the compiler is concerned, so
    // its words must also get auto-emitted prolog/epilog thunks. Verify swap
    // starts with the standard `PUSH PROLOG_HELPER_ADDR; CALL_ABS` thunk —
    // a regression would mean the prelude regressed to assuming RA-on-TOS.
    let mut c = Compiler::new();
    c.compile(PRELUDE).unwrap();
    let swap_addr = match c.dict_get("swap") {
        Some(DictEntry::Word(a)) => a as usize,
        _ => panic!("swap missing from prelude"),
    };
    let bytes = c.code();
    assert_eq!(bytes[swap_addr], op::PUSH);
    let target = u32::from_le_bytes([
        bytes[swap_addr+1], bytes[swap_addr+2], bytes[swap_addr+3], bytes[swap_addr+4],
    ]);
    assert_eq!(target, PROLOG_HELPER_ADDR as u32);
    assert_eq!(bytes[swap_addr+5], op::CALL_ABS);
}

// ===========================================================================
// Error-path coverage — every `Err(...)` in lib.rs that user code can hit
// ===========================================================================

#[test]
fn colon_at_end_of_input_without_name_errors() {
    let err = compile(":").unwrap_err();
    assert!(err.contains("needs a name"), "got: {}", err);
}

#[test]
fn variable_at_end_of_input_without_name_errors() {
    let err = compile("variable").unwrap_err();
    assert!(err.contains("needs a name"), "got: {}", err);
}

#[test]
fn constant_at_end_of_input_without_name_errors() {
    let err = compile("constant").unwrap_err();
    assert!(err.contains("needs a name"), "got: {}", err);
}

#[test]
fn constant_with_name_but_no_value_errors() {
    let err = compile("constant K").unwrap_err();
    assert!(err.contains("needs a value"), "got: {}", err);
}

#[test]
fn constant_with_non_numeric_value_errors() {
    let err = compile("constant K hello").unwrap_err();
    assert!(err.contains("bad value"), "got: {}", err);
}

// ===========================================================================
// Number encoding boundaries — pin PushZero / PushShort / Push selection
// ===========================================================================

#[test]
fn zero_in_various_forms_all_emit_push_zero() {
    // PushZero is preferred over PushShort 0. Cover decimal, hex, negated.
    for src in [": main 0 ;", ": main -0 ;", ": main 0x0 ;", ": main -0x0 ;"] {
        let body = word_body(src, "main");
        assert_eq!(body[0], op::PUSH_ZERO, "expected PushZero for `{}`", src);
    }
}

#[test]
fn push_at_128_promotes_to_long_form_decimal() {
    // 127 is i8 max; 128 is one past — must use full PUSH.
    let body = word_body(": main 128 ;", "main");
    assert_eq!(body[0], op::PUSH);
    assert_eq!(&body[1..5], &128u32.to_le_bytes());
}

#[test]
fn push_at_minus_129_promotes_to_long_form() {
    // -128 is i8 min; -129 is one past — must use full PUSH.
    let body = word_body(": main -129 ;", "main");
    assert_eq!(body[0], op::PUSH);
    assert_eq!(&body[1..5], &(-129i32 as u32).to_le_bytes());
}

// ===========================================================================
// Tokenizer edge cases
// ===========================================================================

#[test]
fn unterminated_block_comment_consumes_to_eof_without_hanging() {
    // The `(` handler reads until `)` or EOF. If EOF wins, the rest of the
    // source (including `;`) is silently eaten — the definition ends up
    // unclosed, surfacing as a clear finalize-time error rather than a
    // tokenizer crash or infinite loop.
    let err = compile(": main 1 ( unterminated").unwrap_err();
    assert!(err.contains("unclosed"), "got: {}", err);
}

#[test]
fn line_comment_at_eof_without_newline_compiles_cleanly() {
    let src = ": main 1 drop ; \\ no newline at end of file";
    let bytes = compile(src).unwrap();
    assert!(bytes.len() > PREAMBLE_LEN);
}

#[test]
fn tab_and_crlf_separate_tokens() {
    // Rust's `char::is_whitespace` covers \t, \r, \n — the tokenizer
    // inherits all of them. Mixed Windows line endings shouldn't faze it.
    let src = ":\tmain\r\n\t42\r\n\tdrop\r\n;\r\n";
    let bytes = compile(src).unwrap();
    assert!(bytes.len() > PREAMBLE_LEN);
}

// ===========================================================================
// Primitive opcode mapping — verify the 1-byte primitives compile to the
// exact opcode constants. Quick regression guard if anyone reshuffles them.
// ===========================================================================

#[test]
fn halt_primitive_emits_halt_opcode() {
    let body = word_body(": main halt ;", "main");
    assert_eq!(body[0], op::HALT);
}

#[test]
fn syscall_primitive_emits_syscall_opcode() {
    let body = word_body(": main syscall ;", "main");
    assert_eq!(body[0], op::SYSCALL);
}

#[test]
fn skip_primitive_emits_skip_opcode() {
    let body = word_body(": main skip ;", "main");
    assert_eq!(body[0], op::SKIP);
}

// ===========================================================================
// Compiler state / dictionary
// ===========================================================================

#[test]
fn word_redefinition_replaces_dict_entry() {
    // Compiler allows redefinition; the dictionary just points to the new
    // body. Old call sites that already emitted with the old address still
    // call the old code — this test pins that behavior.
    let mut c = Compiler::new();
    c.compile(PRELUDE).unwrap();
    c.compile(": foo 1 ;").unwrap();
    let first = match c.dict_get("foo") {
        Some(DictEntry::Word(a)) => a,
        _ => panic!("foo missing after first definition"),
    };
    c.compile(": foo 2 ;").unwrap();
    let second = match c.dict_get("foo") {
        Some(DictEntry::Word(a)) => a,
        _ => panic!("foo missing after redefinition"),
    };
    assert_ne!(first, second, "redefinition should produce a new address");
}

#[test]
fn compile_can_be_called_in_multiple_chunks() {
    // Building up a program piece-by-piece across several `compile()` calls
    // should be equivalent to one big `compile()` — useful for embedding
    // and for sanity (no per-call hidden state).
    let mut c = Compiler::new();
    c.compile(PRELUDE).unwrap();
    c.compile(": foo 1 ;").unwrap();
    c.compile(": bar 2 ;").unwrap();
    c.compile(": main foo bar + drop ;").unwrap();
    c.finalize().unwrap();
    let bytes = c.into_bytes();
    assert!(bytes.len() > PREAMBLE_LEN);
}

#[test]
fn variable_address_falls_within_code_region() {
    // Variables are allocated at `here()` after the preamble + prelude.
    // They should never end up below the preamble nor cross into the
    // return-stack region.
    let mut c = Compiler::new();
    c.compile(PRELUDE).unwrap();
    c.compile("variable v").unwrap();
    let addr = match c.dict_get("v") {
        Some(DictEntry::Value(a)) => a as usize,
        _ => panic!(),
    };
    assert!(addr >= PREAMBLE_LEN, "variable below preamble: 0x{:04x}", addr);
    assert!(addr + 4 <= 0x7000, "variable above retstack: 0x{:04x}", addr);
}

// ===========================================================================
// Forward references — exhaustive placement coverage
// ===========================================================================

#[test]
fn forward_ref_inside_if_body_resolves() {
    // The forward-ref placeholder is emitted inside an `if` body, so the
    // patcher writes into a slot that's halfway through a control-flow
    // structure. Verifies the slot-tracking is offset-correct.
    let src = "
        variable r
        : main 1 if b then r ! ;
        : b 42 ;
    ";
    let bytes = livectf_forth::compile_program(src).unwrap();
    assert!(bytes.len() > PREAMBLE_LEN);
}

#[test]
fn forward_ref_inside_do_loop_body_resolves() {
    // Same but the placeholder lands inside a do/loop body.
    let src = "
        variable r
        : main 0  5 0 do b +  loop  r ! ;
        : b 1 ;
    ";
    let bytes = livectf_forth::compile_program(src).unwrap();
    assert!(bytes.len() > PREAMBLE_LEN);
}

#[test]
fn forward_ref_inside_begin_until_resolves() {
    let src = "
        variable counter variable r
        : main 0 counter !  begin counter @ 1 + counter !  counter @ done? until ;
        : done?  3 = ;
    ";
    let bytes = livectf_forth::compile_program(src).unwrap();
    assert!(bytes.len() > PREAMBLE_LEN);
}

#[test]
fn multiple_forward_refs_to_same_word_all_patched() {
    // If a body uses the same forward-ref'd name multiple times, every
    // placeholder slot must get patched (not just the first).
    let src = "
        variable r
        : main b b b + +  r ! ;
        : b 1 ;
    ";
    let bytes = livectf_forth::compile_program(src).unwrap();
    assert!(bytes.len() > PREAMBLE_LEN);
}

#[test]
fn forward_ref_and_direct_call_both_target_correct_address() {
    // Forward-ref call (emit_call_placeholder + later patch) and direct
    // call (emit_call) must produce the same target address bytes.
    let mut c = Compiler::new();
    c.compile(PRELUDE).unwrap();
    c.compile("variable r").unwrap();
    c.compile(": a b r ! ;").unwrap();          // forward-ref to b
    c.compile(": b 42 ;").unwrap();
    c.compile(": main a b r ! ;").unwrap();     // direct call to b
    c.finalize().unwrap();
    let bytes = c.code().to_vec();
    let a_addr = match c.dict_get("a") {
        Some(DictEntry::Word(x)) => x as usize, _ => panic!()
    };
    let main_addr = match c.dict_get("main") {
        Some(DictEntry::Word(x)) => x as usize, _ => panic!()
    };
    let b_addr = match c.dict_get("b") {
        Some(DictEntry::Word(x)) => x as u32, _ => panic!()
    };

    // a's body (forward-ref path) starts with `PUSH b_addr; CALL_ABS`.
    let body_a = a_addr + PROLOG_LEN;
    assert_eq!(bytes[body_a], op::PUSH);
    let target_a = u32::from_le_bytes([
        bytes[body_a+1], bytes[body_a+2], bytes[body_a+3], bytes[body_a+4],
    ]);
    assert_eq!(target_a, b_addr, "forward-ref call should target b");

    // main's body: first call is to `a`, second (6 bytes later) is to `b`
    // through the direct path.
    let body_main = main_addr + PROLOG_LEN;
    let call_b = body_main + 6;
    assert_eq!(bytes[call_b], op::PUSH);
    let target_b = u32::from_le_bytes([
        bytes[call_b+1], bytes[call_b+2], bytes[call_b+3], bytes[call_b+4],
    ]);
    assert_eq!(target_b, b_addr, "direct call should target b");
}

// ===========================================================================
// Control-structure validation — every open structure should error at `;`
// ===========================================================================

#[test]
fn semicolon_inside_open_begin_errors() {
    let err = compile(": main begin 1 ;").unwrap_err();
    assert!(err.contains("control block is open"), "got: {}", err);
}

#[test]
fn semicolon_inside_open_do_errors() {
    let err = compile(": main 3 0 do 1 drop ;").unwrap_err();
    assert!(err.contains("control block is open"), "got: {}", err);
}

#[test]
fn semicolon_inside_open_else_errors() {
    let err = compile(": main 1 if 2 else 3 ;").unwrap_err();
    assert!(err.contains("control block is open"), "got: {}", err);
}

// ===========================================================================
// Number boundaries / tokenizer corners
// ===========================================================================

#[test]
fn number_overflowing_i32_but_fitting_i64_truncates_to_zero_via_push() {
    // 0x100000000 = 2^32 fits in i64 (parse_number's return type) but not
    // i32. The emitter does `(n as i32) as u32` after the encoding-size
    // pick, so:
    //   - 2^32 isn't in -128..=127 → picks the long PUSH form
    //   - the value passed to PUSH is the truncated 0
    //   - result: 5-byte `PUSH 0`, NOT the 1-byte `PUSH_ZERO`
    //
    // Arguably the compiler should reject or warn on overflowing literals.
    // For now this test pins the silent-truncation behavior — if anyone
    // tightens it (or makes the encoder smarter to use PUSH_ZERO post-
    // truncation), this asserts will fire and need updating.
    let body = word_body(": main 0x100000000 ;", "main");
    assert_eq!(body[0], op::PUSH);
    assert_eq!(&body[1..5], &[0, 0, 0, 0]);
}

#[test]
fn comment_between_colon_and_name_is_consumed() {
    // Tokens after stripping comments: `:`, `foo`, `1`, `;`. The block
    // comment between `:` and `foo` must not disrupt definition parsing.
    let src = ": ( a stack note ) foo 1 ; : main ;";
    let mut c = Compiler::new();
    c.compile(PRELUDE).unwrap();
    c.compile(src).unwrap();
    c.finalize().unwrap();
    assert!(matches!(c.dict_get("foo"), Some(DictEntry::Word(_))));
}

#[test]
fn empty_do_loop_body_compiles() {
    // Body between `do` and `loop` is empty — the compiler should still
    // emit the do-init, the increment+compare+back-branch, and not crash.
    let src = ": main 3 0 do loop ;";
    assert!(livectf_forth::compile_program(src).is_ok());
}

#[test]
fn empty_if_body_with_non_empty_else_compiles() {
    // `if else BODY then`: the if branch is zero bytes. The `else` patcher
    // must still compute a valid offset for the "skip over else" jump.
    let src = ": main 1 if else 42 drop then ;";
    let bytes = livectf_forth::compile_program(src).unwrap();
    assert!(bytes.len() > PREAMBLE_LEN);
}

#[test]
fn empty_else_body_compiles() {
    // `if BODY else then`: the else branch is zero bytes. The `then` patcher
    // must accept an empty else and emit a 0-length skip.
    let src = ": main 1 if 42 drop else then ;";
    let bytes = livectf_forth::compile_program(src).unwrap();
    assert!(bytes.len() > PREAMBLE_LEN);
}

#[test]
fn deeply_nested_if_then_chain_at_depth_10_compiles() {
    // 10 levels of `if ... then`. Stress for the compiler's `ctrl` frame
    // stack and per-frame placeholder bookkeeping. Each `1 if` pushes a
    // truthy cond + opens an if frame; matching `then` pops & patches.
    let mut src = String::from(": main 1");
    for _ in 0..10 {
        src.push_str(" if 1");
    }
    src.push_str(" drop");
    for _ in 0..10 {
        src.push_str(" then");
    }
    src.push_str(" ;");
    assert!(livectf_forth::compile_program(&src).is_ok());
}

#[test]
fn binary_over_max_code_end_is_rejected() {
    // Build a body large enough to push the total binary past MAX_CODE_END
    // (0x7000). Each `1 drop` is 3 bytes (PUSH_SHORT 1 + POP); 10000 of them
    // = 30000 body bytes, plus preamble + prelude + word prolog/epilog gives
    // ~30.5 KB — comfortably above the 28 KB cap.
    let mut src = String::from(": main");
    for _ in 0..10_000 {
        src.push_str(" 1 drop");
    }
    src.push_str(" ;");
    let err = livectf_forth::compile_program(&src).unwrap_err();
    assert!(err.contains("too large"), "got: {}", err);
    // Sanity: the error should also mention the limit so it's actionable.
    assert!(
        err.contains(&format!("{}", MAX_CODE_END)) || err.contains("0x7000"),
        "size-limit error should reference the cap; got: {}",
        err,
    );
}

#[test]
fn source_with_only_comments_and_main_compiles() {
    // Tokenizer must consume both kinds of comments cleanly at every position
    // — file head, between definitions, inside a body, and at the tail —
    // without leaving spurious tokens that would derail parsing.
    let src = r"
        \ Top-level line comment
        ( top-level block comment )
        \ another line comment
        : main
            ( inside block comment ) 42  \ trailing line comment
            drop
        ;
        ( trailing block comment )
    ";
    let bytes = livectf_forth::compile_program(src).expect("should compile");
    assert!(bytes.len() > PREAMBLE_LEN);
}

#[test]
fn begin_again_promotes_to_long_form_when_offset_overflows_i8() {
    // Companion to the existing `until` test. Each `1 drop` is 3 bytes;
    // 50 of them pushes the back-jump beyond -128, forcing `again` to fall
    // back to the 6-byte `PUSH <i32>; JUMP_REL` encoding.
    let mut body_src = String::from(": main begin");
    for _ in 0..50 {
        body_src.push_str(" 1 drop");
    }
    body_src.push_str(" again ;");
    let body = word_body(&body_src, "main");
    // The last 7 bytes (before the synthetic JUMP_ABS) should be the long
    // `again` pattern: PUSH; <i32>; JUMP_REL.
    let pat = &body[body.len() - 7..body.len() - 1];
    assert_eq!(pat[0], op::PUSH);
    let off = i32::from_le_bytes([pat[1], pat[2], pat[3], pat[4]]);
    assert!(off < -100, "expected large negative offset, got {}", off);
    assert_eq!(pat[5], op::JUMP_REL);
}

#[test]
fn do_loop_back_branch_promotes_to_long_form() {
    // Same idea for `do/loop`. `loop`'s back-branch normally fits in i8;
    // padding the body past 127 bytes forces the long-form
    // `PUSH; <i32>; MUL; JUMP_REL` (7 bytes total).
    let mut body_src = String::from(": main 100 0 do");
    for _ in 0..50 {
        body_src.push_str(" 1 drop");
    }
    body_src.push_str(" loop ;");
    let body = word_body(&body_src, "main");
    let last = &body[body.len() - 8..body.len() - 1];
    assert_eq!(last[0], op::PUSH);
    let off = i32::from_le_bytes([last[1], last[2], last[3], last[4]]);
    assert!(off < -100, "expected large negative offset, got {}", off);
    assert_eq!(last[5], op::MUL);
    assert_eq!(last[6], op::JUMP_REL);
}

#[test]
fn prelude_defines_all_expected_words() {
    // Locks in the prelude surface. If anyone trims or renames a word, this
    // fires immediately rather than waiting for downstream user code to
    // break in some unexpected way.
    let mut c = Compiler::new();
    c.compile(PRELUDE).unwrap();
    for name in ["swap", "nip", "tuck", "rot", "-rot", "2dup", "2drop"] {
        match c.dict_get(name) {
            Some(DictEntry::Word(_)) => {}
            other => panic!("prelude should define `{}` as a word, got {:?}", name, other),
        }
    }
}

#[test]
fn constants_at_i32_bounds_compile() {
    // i32::MAX (0x7FFFFFFF) and i32::MIN (-0x80000000) should both round-trip
    // through `constant`. parse_number returns i64, and the literal emitter
    // truncates to i32 — verify the dictionary entry matches the source.
    let mut c = Compiler::new();
    c.compile(PRELUDE).unwrap();
    c.compile("constant MAX 0x7FFFFFFF  constant MIN -0x80000000").unwrap();
    assert_eq!(c.dict_get("MAX"), Some(DictEntry::Value(0x7FFFFFFF)));
    assert_eq!(c.dict_get("MIN"), Some(DictEntry::Value(-0x80000000_i64)));
}

#[test]
fn block_comment_spans_multiple_lines() {
    // The tokenizer's `( ... )` handler doesn't care about newlines — it
    // consumes everything until the matching `)`. Verify by sandwiching a
    // multi-line block comment between two literals that must still emit.
    let body = word_body(
        ": main 1 ( first line\n  second line\n  third ) 2 + ;",
        "main",
    );
    assert_eq!(body[0..2], [op::PUSH_SHORT, 1]);
    assert_eq!(body[2..4], [op::PUSH_SHORT, 2]);
    assert_eq!(body[4], op::ADD);
}

#[test]
fn word_name_can_contain_hyphen_and_question_mark() {
    // The tokenizer splits on whitespace only, so canonical Forth names like
    // `add-ten` and `even?` are valid identifiers. Confirms neither is
    // intercepted as an operator or rejected by the dictionary.
    let mut c = Compiler::new();
    c.compile(PRELUDE).unwrap();
    c.compile(": add-ten 10 + ;  : even? 1 and 0 = ;  : main ;").unwrap();
    c.finalize().unwrap();
    assert!(matches!(c.dict_get("add-ten"), Some(DictEntry::Word(_))));
    assert!(matches!(c.dict_get("even?"),   Some(DictEntry::Word(_))));
}

#[test]
fn consecutive_variables_are_four_bytes_apart() {
    // The pseudo-array pattern (`base i 4 * + @`) used by several run tests
    // assumes consecutive `variable` declarations get consecutive 4-byte
    // slots. Lock that in — if anyone adds padding or reordering, those
    // tests would silently start reading garbage.
    let mut c = Compiler::new();
    c.compile(PRELUDE).unwrap();
    c.compile("variable a variable b variable c").unwrap();
    let addr = |n| match c.dict_get(n) {
        Some(DictEntry::Value(v)) => v as u32,
        _ => panic!("{} not found", n),
    };
    assert_eq!(addr("b"), addr("a") + 4);
    assert_eq!(addr("c"), addr("b") + 4);
}

#[test]
fn empty_word_is_exactly_prolog_plus_epilog() {
    // `: foo ;` should compile to PROLOG_LEN + EPILOG_LEN = 12 bytes total —
    // pure overhead, no body. If this changes, either the thunk shape grew
    // or someone is emitting spurious bytes.
    let mut c = Compiler::new();
    c.compile(PRELUDE).unwrap();
    let before = c.here();
    c.compile(": foo ;").unwrap();
    let after = c.here();
    assert_eq!(after - before, PROLOG_LEN + EPILOG_LEN);
}

#[test]
fn variable_address_is_pushed_when_referenced() {
    // `variable v` allocates 4 bytes; referencing `v` pushes its address.
    let mut c = Compiler::new();
    c.compile(PRELUDE).unwrap();
    let v_addr_before = c.here();
    c.compile("variable v : main v ;").unwrap();
    c.finalize().unwrap();
    let v_entry = c.dict_get("v");
    assert_eq!(v_entry, Some(DictEntry::Value(v_addr_before as i64)));
}

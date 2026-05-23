// Runtime tests: compile a snippet, execute it on the reference VM, assert
// the observable result.
//
// Most tests use `eval("BODY")`, which wraps the body in `: main BODY r ! ;`
// and returns `r`'s final value. Tests that need richer setup (multiple
// variables, control flow across word boundaries, forward references)
// use `get_var(full_src, "var_name")`.

mod common;

use common::{eval, eval_with_defs, get_var, run_src};

// ===========================================================================
// Literal push
// ===========================================================================

#[test] fn push_zero()              { assert_eq!(eval("0"), 0); }
#[test] fn push_small_positive()    { assert_eq!(eval("42"), 42); }
#[test] fn push_small_negative()    { assert_eq!(eval("-5"), -5); }
#[test] fn push_at_i8_boundary()    { assert_eq!(eval("127"), 127); assert_eq!(eval("-128"), -128); }
#[test] fn push_at_i8_overflow()    { assert_eq!(eval("128"), 128); assert_eq!(eval("-129"), -129); }
#[test] fn push_large_positive()    { assert_eq!(eval("100000"), 100_000); }
#[test] fn push_large_negative()    { assert_eq!(eval("-100000"), -100_000); }
#[test] fn push_hex()               { assert_eq!(eval("0xCAFE"), 0xCAFE); }
#[test] fn push_hex_negative()      { assert_eq!(eval("-0x100"), -0x100); }

// ===========================================================================
// Arithmetic — Forth direction (a OP b)
// ===========================================================================

#[test] fn add()       { assert_eq!(eval("7 5 +"), 12); }
#[test] fn add_negative() { assert_eq!(eval("7 -5 +"), 2); }

#[test] fn sub_forth_direction() {
    // CRITICAL: confirms the SUB+NEG lowering. Spec's SUB is TOS-NOS;
    // Forth's `a b -` is a-b. If this fails, the `-` peephole regressed.
    assert_eq!(eval("7 5 -"), 2);
    assert_eq!(eval("5 7 -"), -2);
    assert_eq!(eval("0 0 -"), 0);
    assert_eq!(eval("100 1 -"), 99);
}

#[test] fn mul()       { assert_eq!(eval("7 5 *"), 35); }
#[test] fn mul_negative() { assert_eq!(eval("-7 5 *"), -35); }

#[test] fn div_forth_direction() {
    // CRITICAL: a/b. Spec DIV is TOS/NOS; Forth wants NOS/TOS, lowered via swap-call.
    assert_eq!(eval("20 4 /"), 5);
    assert_eq!(eval("21 4 /"), 5);  // truncates toward zero
    assert_eq!(eval("4 20 /"), 0);  // smaller / larger
}

#[test] fn mod_forth_direction() {
    assert_eq!(eval("23 5 mod"), 3);
    assert_eq!(eval("5 23 mod"), 5);
    assert_eq!(eval("10 5 mod"), 0);
}

#[test] fn lshift_forth_direction() {
    // `x u lshift` = x << u, Forth direction.
    assert_eq!(eval("1 4 lshift"), 16);
    assert_eq!(eval("3 2 lshift"), 12);
    assert_eq!(eval("0xFF 8 lshift"), 0xFF00);
}

#[test] fn rshift_forth_direction() {
    assert_eq!(eval("32 2 rshift"), 8);
    assert_eq!(eval("0xFF 4 rshift"), 0x0F);
    assert_eq!(eval("1 0 rshift"), 1);  // shift by 0
}

#[test] fn negate() { assert_eq!(eval("5 negate"), -5); assert_eq!(eval("-5 negate"), 5); }

// ===========================================================================
// Bitwise
// ===========================================================================

#[test] fn bit_and() { assert_eq!(eval("0xF0 0x0F and"), 0x00); assert_eq!(eval("0xFF 0x33 and"), 0x33); }
#[test] fn bit_or()  { assert_eq!(eval("0xF0 0x0F or"), 0xFF); }
#[test] fn bit_xor() { assert_eq!(eval("0xFF 0xAA xor"), 0x55); }
#[test] fn bit_invert() {
    // `invert` is bitwise NOT; result interpreted as signed i32 is -(x+1).
    assert_eq!(eval("0 invert"), -1);
    assert_eq!(eval("-1 invert"), 0);
}

// ===========================================================================
// Comparisons — Forth direction
// ===========================================================================

#[test]
fn less_than_forth_direction() {
    // CRITICAL: Forth `a b <` = (a < b). Spec's LT is TOS<NOS, so compiler
    // maps `<` → GT to flip the operand interpretation. If the reflected
    // mapping regressed, these will fail.
    assert_eq!(eval("3 5 <"), 1);
    assert_eq!(eval("5 3 <"), 0);
    assert_eq!(eval("5 5 <"), 0);
    assert_eq!(eval("-1 0 <"), 1);
}

#[test]
fn greater_than_forth_direction() {
    assert_eq!(eval("5 3 >"), 1);
    assert_eq!(eval("3 5 >"), 0);
    assert_eq!(eval("5 5 >"), 0);
}

#[test]
fn less_equal_forth_direction() {
    assert_eq!(eval("3 5 <="), 1);
    assert_eq!(eval("5 5 <="), 1);
    assert_eq!(eval("5 3 <="), 0);
}

#[test]
fn greater_equal_forth_direction() {
    assert_eq!(eval("5 3 >="), 1);
    assert_eq!(eval("5 5 >="), 1);
    assert_eq!(eval("3 5 >="), 0);
}

#[test]
fn equality_and_inequality() {
    assert_eq!(eval("5 5 ="), 1);
    assert_eq!(eval("5 6 ="), 0);
    assert_eq!(eval("5 5 <>"), 0);
    assert_eq!(eval("5 6 <>"), 1);
}

#[test]
fn logical_not() {
    assert_eq!(eval("0 not"), 1);
    assert_eq!(eval("1 not"), 0);
    assert_eq!(eval("42 not"), 0);
}

// ===========================================================================
// Stack-manipulation primitives (inline) and prelude words
// ===========================================================================

#[test] fn dup_doubles_tos() { assert_eq!(eval("7 dup +"), 14); }
#[test] fn drop_pops() { assert_eq!(eval("99 7 drop"), 99); }
// `3 5 over -`:
//   [3, 5]              after pushes
//   [3, 5, 3]           over copies NOS to top
//   [3, 5-3] = [3, 2]   `-` is Forth-direction (NOS-TOS) via SUB+NEG
//   eval saves TOS = 2.
#[test] fn over_brings_nos_to_top() { assert_eq!(eval("3 5 over -"), 2); }

#[test]
fn swap_exchanges_top_two() {
    // After swap, TOS becomes the original NOS. eval saves TOS, so:
    // `7 9 swap` → stack [9, 7], TOS=7.
    assert_eq!(eval("7 9 swap"), 7);
}

#[test]
fn nip_drops_nos() {
    // `7 9 nip` → [9], TOS=9.
    assert_eq!(eval("7 9 nip"), 9);
}

#[test]
fn tuck_inserts_top_below_nos() {
    // `7 9 tuck` → [9, 7, 9]. TOS = 9.
    assert_eq!(eval("7 9 tuck"), 9);
    // Drain the full [9, 7, 9] into two variables. `b !` consumes TOS=9,
    // then `a !` consumes the new TOS=7. So a ends up holding 7, b holding 9.
    let src = "
        variable a variable b
        : main 7 9 tuck b ! a ! ;
    ";
    assert_eq!(get_var(src, "a").unwrap(), 7);
    assert_eq!(get_var(src, "b").unwrap(), 9);
}

#[test]
fn rot_rotates_three() {
    // `1 2 3 rot` → ( a b c -- b c a ) = [2, 3, 1]. TOS = 1.
    assert_eq!(eval("1 2 3 rot"), 1);
}

#[test]
fn minus_rot_rotates_three_other_way() {
    // `1 2 3 -rot` → ( a b c -- c a b ) = [3, 1, 2]. TOS = 2.
    assert_eq!(eval("1 2 3 -rot"), 2);
}

#[test]
fn twodup_duplicates_pair() {
    // `7 9 2dup +` → [7, 9, 7, 9] → [7, 9, 16]. TOS = 16.
    assert_eq!(eval("7 9 2dup +"), 16);
}

#[test]
fn twodrop_drops_pair() {
    // `7 9 99 100 2drop` → [7, 9]. TOS = 9.
    assert_eq!(eval("7 9 99 100 2drop"), 9);
}

// ===========================================================================
// Variable @ and !
// ===========================================================================

#[test]
fn variable_store_and_load() {
    let src = "
        variable v variable r
        : main 42 v ! v @ r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 42);
}

#[test]
fn variable_persists_across_words() {
    let src = "
        variable v variable r
        : inc v @ 1 + v ! ;
        : main 0 v ! inc inc inc v @ r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 3);
}

// ===========================================================================
// Constants
// ===========================================================================

#[test]
fn constant_inlines_at_use_site() {
    let src = "
        constant K 100
        variable r
        : main K 50 - r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 50);
}

// ===========================================================================
// if / else / then
// ===========================================================================

#[test]
fn if_then_true_branch_runs() {
    assert_eq!(eval("1 if 99 else 11 then"), 99);
}

#[test]
fn if_then_false_branch_runs() {
    assert_eq!(eval("0 if 99 else 11 then"), 11);
}

#[test]
fn if_without_else_skips_when_false() {
    // 0 if ... then leaves a 7 we pre-pushed.
    assert_eq!(eval("7 0 if drop 99 then"), 7);
}

#[test]
fn if_without_else_runs_when_true() {
    assert_eq!(eval("7 1 if drop 99 then"), 99);
}

#[test]
fn nested_if_then_else() {
    // Outer: if 1, inner runs. Inner: if 0, else → 22.
    assert_eq!(eval("1 if 0 if 11 else 22 then else 33 then"), 22);
    assert_eq!(eval("0 if 11 else 1 if 22 else 33 then then"), 22);
}

// ===========================================================================
// begin / until / again
// ===========================================================================

#[test]
fn begin_until_counts() {
    // Count `tick` from 0 up; loop exits when tick reaches 5.
    let src = "
        variable tick variable r
        : main
            0 tick !
            begin
                tick @ 1 + tick !
                tick @ 5 =
            until
            tick @ r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 5);
}

#[test]
fn begin_again_with_exit() {
    // Infinite loop with early exit. Each iteration bumps `tick`; when it
    // hits 7 we stash it in `r` and `exit` straight out of `main`. Confirms
    // that begin/again is reachable and that `exit` mid-loop correctly runs
    // the epilog (pops RA from the return stack and JUMP_ABSes to it).
    let src = "
        variable tick variable r
        : main
            0 tick !
            begin
                tick @ 1 + tick !
                tick @ 7 = if tick @ r ! exit then
            again ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 7);
}

// ===========================================================================
// do / loop / i
// ===========================================================================

#[test]
fn do_loop_sum_0_to_10() {
    // Σ i for i in [0, 10) = 45.
    let src = "
        variable sum variable r
        : main
            0 sum !
            10 0 do
                sum @ i + sum !
            loop
            sum @ r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 45);
}

#[test]
fn do_loop_iterates_exactly_limit_minus_start_times() {
    let src = "
        variable n variable r
        : main
            0 n !
            7 2 do
                n @ 1 + n !
            loop
            n @ r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 5);  // 7-2 = 5 iterations
}

#[test]
fn do_loop_with_zero_iterations_when_start_equals_limit() {
    // `5 5 do … loop` should not execute the body because start==limit.
    // Our implementation increments index first, then compares; so on entry
    // the index = 5, limit = 5. Body runs once (with i=5), then i becomes 6,
    // 6<5 false → exit. So with start==limit, body runs ONCE in our
    // implementation. This documents that behavior (different from strict
    // Forth, which would skip entirely — that requires `?do`).
    let src = "
        variable n variable r
        : main 0 n ! 5 5 do n @ 1 + n ! loop n @ r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 1,
        "documenting: our `do` runs the body at least once even when start==limit (use `?do` for true zero-trip)");
}

#[test]
fn nested_do_loop_product() {
    // Outer i=1..3, inner j=1..4; sum i*j accumulated.
    // i=1: 1+2+3 = 6
    // i=2: 2+4+6 = 12
    // i=3: 3+6+9 = 18
    // total = 36
    let src = "
        variable s variable r
        : main
            0 s !
            4 1 do
                4 1 do
                    s @ i + s !
                loop
            loop
            s @ r ! ;
    ";
    // Inner loop runs i in [1,4) = 3 iterations; each adds inner's `i`
    // value (which is the inner index, the innermost loop's variable).
    // i_inner takes 1,2,3, sum = 6 per outer iteration. Outer runs 3
    // iterations → 18 total.
    assert_eq!(get_var(src, "r").unwrap(), 18);
}

#[test]
fn i_inside_loop_takes_each_value() {
    // Capture the maximum value `i` reaches.
    let src = "
        variable last variable r
        : main
            0 last !
            5 0 do
                i last !
            loop
            last @ r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 4);  // last value of i was 4 (loop exits before 5)
}

#[test]
fn deeply_nested_do_loops() {
    // 4 levels of `do`/`loop`, each one iteration: confirms slot
    // assignment doesn't collide.
    let src = "
        variable n variable r
        : main
            0 n !
            2 0 do
                2 0 do
                    2 0 do
                        2 0 do
                            n @ 1 + n !
                        loop
                    loop
                loop
            loop
            n @ r ! ;
    ";
    // 2*2*2*2 = 16
    assert_eq!(get_var(src, "r").unwrap(), 16);
}

// ===========================================================================
// Forward references and mutual recursion
// ===========================================================================

#[test]
fn forward_reference_resolves_at_runtime() {
    let src = "
        variable r
        : main 5 add-ten r ! ;
        : add-ten 10 + ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 15);
}

#[test]
fn mutual_recursion_even_odd() {
    let src = "
        variable r
        : even? dup 0 = if drop 1 exit then 1 - odd? ;
        : odd?  dup 0 = if drop 0 exit then 1 - even? ;
        : main 6 even? r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 1, "even?(6) should be 1");
}

#[test]
fn mutual_recursion_odd_path() {
    let src = "
        variable r
        : even? dup 0 = if drop 1 exit then 1 - odd? ;
        : odd?  dup 0 = if drop 0 exit then 1 - even? ;
        : main 7 even? r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 0, "even?(7) should be 0");
}

#[test]
fn self_recursion_factorial() {
    let src = "
        variable r
        : fact dup 1 <= if drop 1 exit then dup 1 - fact * ;
        : main 5 fact r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 120);
}

// ===========================================================================
// SPEC-LEVEL operand-order verification: glibc-style LCG
// ===========================================================================
//
// This is the end-to-end version of the operand-order check the user flagged
// in examples/wander.asm at L00e3. The LCG `state = (a * state + c) % m`
// only produces the canonical glibc sequence if MUL/ADD are commutative
// (no compiler fixup needed) and REM does the *correct-direction* modulus —
// which in our compiler comes from emitting `swap; REM` for the `mod` word.
//
// Reference: glibc's TYPE_0 rand() uses a=1103515245, c=12345, m=2^31.
// Starting from seed 1, the first three outputs are:
//   s1 = (1*1 + 12345) mod 2^31 = 12345 wait that's wrong
//   Actually: s_{n+1} = (a*s_n + c) mod m, returning s_{n+1}.
//   From s_0 = 1: s_1 = (1103515245 + 12345) mod 2^31 = 1103527590
//   From s_1: s_2 = (1103515245*1103527590 + 12345) mod 2^31 = ?
// We just check the first step here, which is enough to confirm the
// operand-order chain (mul, add, then mod) lowers correctly.

#[test]
fn lcg_first_step_matches_glibc() {
    let src = "
        variable r
        constant a 0x41c64e6d
        constant c 12345
        constant m 0x80000000
        \\ state = (a * state + c) mod m, starting from state = 1.
        : main
            1                  \\ state
            a *                \\ a * state
            c +                \\ + c
            m mod              \\ mod m
            r ! ;
    ";
    // m is 0x80000000 = 2^31. As an i32 this is INT32_MIN. The compiler
    // pushes it as a 4-byte literal; the VM reads it back as i32. Forth
    // `mod` lowers to `swap; REM` so the VM ends up computing
    //   ((a * state + c) as i32) % (m as i32 = -2^31).
    //
    // i32 wrapping: 1103515245 + 12345 = 1103527590, well within i32, so
    // (1103527590) % (-2147483648) — Rust `wrapping_rem` returns
    // 1103527590 (same sign as dividend, magnitude < divisor magnitude).
    assert_eq!(get_var(src, "r").unwrap(), 1103527590,
        "operand ordering through MUL/ADD/REM must produce glibc's first LCG output");
}

// ===========================================================================
// Misc end-to-end programs
// ===========================================================================

#[test]
fn sum_of_squares() {
    // Σ i² for i in [1, 6) = 1+4+9+16+25 = 55
    let src = "
        variable s variable r
        : main
            0 s !
            6 1 do
                i i * s @ + s !
            loop
            s @ r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 55);
}

#[test]
fn fibonacci_via_loop() {
    // Compute fib(10) using do/loop and variables. fib(10) = 55.
    let src = "
        variable a variable b variable tmp variable r
        : main
            0 a !  1 b !
            10 0 do
                b @ tmp !
                a @ b @ + b !
                tmp @ a !
            loop
            a @ r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 55);
}

#[test]
fn collatz_steps_to_one_for_27() {
    // Classic Collatz from 27 takes 111 steps to reach 1.
    let src = "
        variable n variable steps variable r
        : main
            27 n !
            0 steps !
            begin
                n @ 1 = if steps @ r ! exit then
                n @ 2 mod 0 = if
                    n @ 2 / n !
                else
                    n @ 3 * 1 + n !
                then
                steps @ 1 + steps !
            again ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 111);
}

#[test]
fn gcd_via_loop() {
    // Euclidean GCD(48, 18) = 6.
    let src = "
        variable a variable b variable tmp variable r
        : main
            48 a !  18 b !
            begin
                b @ 0 = if a @ r ! exit then
                a @ b @ mod tmp !
                b @ a !
                tmp @ b !
            again ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 6);
}

#[test]
fn deeply_recursive_word_returns_correctly() {
    // Exercise the call/return stack discipline. Recurse 20 deep, each
    // adding 1; final value should equal depth.
    let src = "
        variable r
        : rec dup 0 = if exit then 1 - rec ;
        \\ rec leaves count on stack at base, but each call also leaves
        \\ extra junk. Use a simpler counter:
        variable cnt
        : count-rec dup 0 = if drop exit then cnt @ 1 + cnt ! 1 - count-rec ;
        : main 0 cnt ! 20 count-rec cnt @ r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 20);
}

// ===========================================================================
// Run-doesn't-explode smoke tests
// ===========================================================================

#[test]
fn empty_main_halts_cleanly() {
    let vm = run_src(": main ;").unwrap();
    assert!(vm.halted);
}

#[test]
fn demo_fth_compiles_and_runs_minus_syscalls() {
    // Strip syscalls so the demo runs in the test VM. We rebuild a tiny
    // version that exercises if/else/then, do/loop, variables, constants.
    let src = "
        variable counter variable r
        constant THRESHOLD 100
        : classify
            counter @ THRESHOLD > if 1 else 0 then ;
        : main
            150 counter !
            classify r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 1);
}

// ===========================================================================
// Forward references with extra arguments (eval_with_defs convenience)
// ===========================================================================

#[test]
fn eval_with_defs_helper_works() {
    // body uses `square`, defined after main via defs_after.
    assert_eq!(eval_with_defs("7 square", ": square dup * ;"), 49);
}

#[test]
fn many_forward_refs_resolve_in_one_finalize() {
    let src = "
        variable r
        : main a b c + + r ! ;
        : a 1 ;
        : b 2 ;
        : c 3 ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 6);
}

// ===========================================================================
// Return-stack stress: deep recursion, long mutual ping-pong, doubly-recursive
// fib. These exercise the trampoline helpers under sustained push/pop and
// confirm the retstack region (0x7000..0x7FC4, ~1010 slots) doesn't overflow
// at realistic depths.
// ===========================================================================

#[test]
fn factorial_recursive_n10() {
    // 10! = 3,628,800. Depth-10 recursion using the same self-call pattern as
    // self_recursion_factorial but reaching further to confirm correctness
    // doesn't degrade with depth.
    let src = "
        variable r
        : fact dup 1 <= if drop 1 exit then dup 1 - fact * ;
        : main 10 fact r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 3_628_800);
}

#[test]
fn deep_recursion_300_levels_returns_correctly() {
    // Recurse 300 deep, incrementing a side-counter on each entry. Verifies
    // that the retstack handles ~300 frames (1.2 KB) without clobbering code
    // or scratch slots, and that the unwind correctly restores execution.
    let src = "
        variable count variable r
        : descend
            dup 0 = if drop exit then
            count @ 1 + count !
            1 - descend ;
        : main
            0 count !
            300 descend
            count @ r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 300);
}

#[test]
fn mutual_recursion_100_pings_pongs() {
    // ping/pong each bump a shared counter and tail-call the other. Counter
    // reaches 100 only if every call/return correctly preserves and restores
    // the return chain across word boundaries.
    let src = "
        variable depth variable r
        : ping  dup 0 = if exit then  depth @ 1 + depth !  1 - pong ;
        : pong  dup 0 = if exit then  depth @ 1 + depth !  1 - ping ;
        : main  0 depth !  100 ping  depth @ r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 100);
}

#[test]
fn fib_doubly_recursive_n10() {
    // Doubly-recursive fib(10) = 55. Produces a branching call tree (177
    // calls, max stack depth 10), which exercises retstack push/pop in a
    // non-linear pattern unlike the iterative or singly-recursive tests.
    let src = "
        variable r
        : fib
            dup 2 < if exit then
            dup 1 - fib  swap 2 - fib  + ;
        : main 10 fib r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 55);
}

// ===========================================================================
// Call composability: words calling words, do-loops calling words, control
// flow inside called words. These verify the calling convention end-to-end
// for realistic program shapes.
// ===========================================================================

#[test]
fn do_loop_calls_word_each_iteration() {
    // Inside a `do/loop`, call a user word that updates a variable. The body
    // runs 100 times. add-to-sum's prolog/epilog must execute cleanly on
    // every iteration without disturbing the do/loop scratch slots.
    let src = "
        variable sum variable r
        : add-to-sum  sum @ + sum ! ;
        : main
            0 sum !
            100 0 do  i add-to-sum  loop
            sum @ r ! ;
    ";
    // Σ i for i in [0, 100) = 100*99/2 = 4950.
    assert_eq!(get_var(src, "r").unwrap(), 4950);
}

#[test]
fn four_deep_word_call_chain_returns_value() {
    // a → b → c → d. Each adds 1 to the value it received. The final value
    // is 100 + 4. Confirms that retstack frames stack and unwind correctly
    // for a chain of plain user-word calls (no recursion).
    let src = "
        variable r
        : d  1 + ;
        : c  1 + d ;
        : b  1 + c ;
        : a  1 + b ;
        : main  100 a r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 104);
}

// ===========================================================================
// `exit` from inside nested control structures
// ===========================================================================

#[test]
fn exit_from_if_inside_do_loop_short_circuits_word() {
    // On iteration i=3, the inner `if` fires `exit`, which should abandon
    // both the remaining loop iterations and the post-loop instruction
    // `-1 result !`. Verifies that `exit`'s epilog jumps cleanly out of
    // `do/loop` and back to the caller without unwinding scratch slots.
    let src = "
        variable result variable r
        : nest
            5 0 do
                i 3 = if 999 result ! exit then
                i 100 * result !
            loop
            -1 result ! ;
        : main  nest  result @ r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 999);
}

#[test]
fn exit_from_begin_again_with_side_effect() {
    // begin/again is an infinite loop; the only way out is `exit`. The
    // counter should land on exactly 42 if the early exit consumed the
    // expected number of iterations.
    let src = "
        variable n variable r
        : run
            0 n !
            begin
                n @ 1 + n !
                n @ 42 = if exit then
            again ;
        : main  run  n @ r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 42);
}

// ===========================================================================
// Bit-twiddling: confirms shift/and/or compose correctly in a realistic loop
// ===========================================================================

#[test]
fn popcount_of_0xcafe() {
    // popcount(0xCAFE) = popcount(1100_1010_1111_1110b) = 11.
    // The loop pattern uses `over`, `nip`, `swap`, `rshift`, `and` together
    // — a good integration check for the prelude after the prolog/epilog
    // rework.
    let src = "
        variable r
        : popcount
            0
            begin
                over 0 = if nip exit then
                over 1 and +
                swap 1 rshift swap
            again ;
        : main  0xCAFE popcount r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 11);
}

// ===========================================================================
// Multi-variable state mutation: bubble-sort 3 stored values in place
// ===========================================================================

// ===========================================================================
// Classic recursive algorithms
// ===========================================================================

#[test]
fn gcd_recursive_euclidean() {
    // Recursive form of Euclid's algorithm: gcd(a, 0) = a; gcd(a, b) =
    // gcd(b, a mod b). Verifies that `mod` (which lowers to swap+REM) and
    // `swap`/`over` (prelude words) all compose under recursion.
    let src = "
        variable r
        : gcd
            dup 0 = if drop exit then
            swap over mod
            gcd ;
        : main 48 18 gcd r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 6);
}

#[test]
fn integer_power_recursive() {
    // Naive recursive x^n: pow(x, 0) = 1; pow(x, n) = x * pow(x, n-1).
    // 2^10 = 1024.
    let src = "
        variable r
        : pow
            dup 0 = if drop drop 1 exit then
            1 - over swap pow * ;
        : main 2 10 pow r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 1024);
}

#[test]
fn digit_sum_via_mod_and_div_loop() {
    // Repeatedly pull off the low digit via `n 10 mod`, add to accumulator,
    // then `n 10 /`. Exercises rot (prelude), mod/div (non-commutative
    // lowering with swap), and a tight begin/again loop with exit.
    let src = "
        variable r
        : digit-sum
            0 swap
            begin
                dup 0 = if drop exit then
                dup 10 mod  rot +  swap
                10 /
            again ;
        : main 1234567 digit-sum r ! ;
    ";
    // 1+2+3+4+5+6+7 = 28
    assert_eq!(get_var(src, "r").unwrap(), 28);
}

#[test]
fn reverse_digits_detects_palindrome() {
    // Reverse the decimal digits of `n`, then compare with the original.
    // 12321 reversed is 12321 → palindrome → result 1.
    let src = "
        variable r
        : reverse-digits
            0 swap
            begin
                dup 0 = if drop exit then
                swap 10 *  over 10 mod  +
                swap 10 /
            again ;
        : main  12321 dup reverse-digits = r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 1);
}

#[test]
fn reverse_digits_rejects_non_palindrome() {
    let src = "
        variable r
        : reverse-digits
            0 swap
            begin
                dup 0 = if drop exit then
                swap 10 *  over 10 mod  +
                swap 10 /
            again ;
        : main  12345 dup reverse-digits = r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 0);
}

// ===========================================================================
// Multi-way classification via chained if/then (the Forth equivalent of
// a switch statement). Each branch short-circuits with `exit`.
// ===========================================================================

#[test]
fn chained_ifs_classify_negative_zero_small_large() {
    let src = "
        variable r
        : classify
            dup 0 <  if drop -1 exit then
            dup 0 =  if drop  0 exit then
            dup 10 < if drop  1 exit then
            drop  2 ;
        : main  42 classify r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 2);
}

#[test]
fn chained_ifs_classify_negative_branch() {
    let src = "
        variable r
        : classify
            dup 0 <  if drop -1 exit then
            dup 0 =  if drop  0 exit then
            dup 10 < if drop  1 exit then
            drop  2 ;
        : main  -7 classify r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), -1);
}

// ===========================================================================
// Pseudo-array via consecutive `variable` declarations. Variables are
// allocated 4 bytes apart in code memory, so `base i 4 * + @` indexes them
// like a contiguous i32 array. This is the realistic pattern for buffers in
// LiveCTF bots until/unless we add a `create N cells allot` style primitive.
// ===========================================================================

#[test]
fn array_sum_via_computed_address() {
    let src = "
        variable a0 variable a1 variable a2 variable a3 variable a4
        variable sum variable r
        : main
            10 a0 !  20 a1 !  30 a2 !  40 a3 !  50 a4 !
            0 sum !
            5 0 do
                a0 i 4 * + @  sum @ + sum !
            loop
            sum @ r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 150);
}

#[test]
fn linear_search_finds_target_index() {
    // Search a 5-slot pseudo-array for `target`, return the matching index
    // (or -1). Tests the `dup ... if drop i exit then ... drop -1` shape —
    // an idiomatic Forth pattern for "find or sentinel".
    let src = "
        variable a0 variable a1 variable a2 variable a3 variable a4
        variable r
        : find
            5 0 do
                dup a0 i 4 * + @ = if drop i exit then
            loop
            drop -1 ;
        : main
            10 a0 !  20 a1 !  30 a2 !  40 a3 !  50 a4 !
            30 find r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 2);
}

#[test]
fn linear_search_returns_minus_one_when_absent() {
    let src = "
        variable a0 variable a1 variable a2 variable a3 variable a4
        variable r
        : find
            5 0 do
                dup a0 i 4 * + @ = if drop i exit then
            loop
            drop -1 ;
        : main
            10 a0 !  20 a1 !  30 a2 !  40 a3 !  50 a4 !
            99 find r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), -1);
}

// ===========================================================================
// Calling conventions: many arguments and multiple return values
// ===========================================================================

#[test]
fn word_with_five_args_returns_sum() {
    // Confirms the calling convention scales: the caller pushes 5 values
    // before the call; the callee's prolog must skim only the RA, leaving
    // all 5 args untouched.
    let src = "
        variable r
        : sum-5  + + + + ;
        : main  10 20 30 40 50 sum-5 r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 150);
}

#[test]
fn word_returns_quotient_and_remainder() {
    // div-mod consumes (a, b) and produces (a/b, a%b). The caller drains
    // both return values into separate variables, which exercises a
    // multi-value return through the same epilog mechanism.
    let src = "
        variable q variable rem variable r
        : div-mod  2dup / -rot mod ;
        : main
            17 5 div-mod
            rem !  q !
            q @ 100 *  rem @ +  r ! ;
    ";
    // q = 17/5 = 3, rem = 17%5 = 2. Encoded as 3*100 + 2 = 302.
    assert_eq!(get_var(src, "q").unwrap(), 3);
    assert_eq!(get_var(src, "rem").unwrap(), 2);
    assert_eq!(get_var(src, "r").unwrap(), 302);
}

// ===========================================================================
// do/loop with non-trivial ranges + parity-driven branching inside the body
// ===========================================================================

#[test]
fn do_loop_with_negative_start_iterates_full_range() {
    // -3 to 4 inclusive of -3, exclusive of 5 = 8 iterations.
    let src = "
        variable n variable r
        : main
            0 n !
            5 -3 do  n @ 1 + n !  loop
            n @ r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 8);
}

#[test]
fn alternating_sign_sum_via_parity_check() {
    // Σ ((-1)^i) * i for i in [0, 10) = 0 - 1 + 2 - 3 + 4 - 5 + 6 - 7 + 8 - 9 = -5.
    // Tests if/else nested inside do/loop, with `i 1 and 0 =` as the parity
    // probe — confirms `and` (commutative single-op) and `i` cooperate.
    let src = "
        variable sum variable r
        : main
            0 sum !
            10 0 do
                i 1 and 0 = if
                    sum @ i + sum !
                else
                    sum @ i - sum !
                then
            loop
            sum @ r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), -5);
}

// ===========================================================================
// Signed arithmetic edges
// ===========================================================================

#[test]
fn division_with_negative_operands_truncates_toward_zero() {
    // Rust's wrapping_div truncates toward zero, matching the VM's `DIV` op.
    // `a b /` lowers to `swap; DIV` so the result is a/b (Forth direction).
    assert_eq!(eval("-7 2 /"), -3);   // -7/2 = -3 (toward zero), not -4 (floor)
    assert_eq!(eval("7 -2 /"), -3);
    assert_eq!(eval("-7 -2 /"), 3);
}

#[test]
fn modulo_takes_sign_of_dividend() {
    // Rust's wrapping_rem matches the dividend's sign — the test VM uses
    // wrapping_rem and the compiler emits `swap; REM`, so `a b mod` = a%b
    // with sign(result) = sign(a).
    assert_eq!(eval("-7 3 mod"), -1);
    assert_eq!(eval("7 -3 mod"), 1);
    assert_eq!(eval("-7 -3 mod"), -1);
}

#[test]
fn add_wraps_at_i32_overflow() {
    // 0x7FFFFFFF + 1 wraps to i32::MIN. The VM uses wrapping_add, so this
    // produces a defined (if surprising) value rather than panicking.
    let src = "
        variable r
        : main  0x7FFFFFFF 1 + r ! ;
    ";
    // i32::MIN = -2_147_483_648
    assert_eq!(get_var(src, "r").unwrap(), i32::MIN);
}

#[test]
fn mul_wraps_at_i32_overflow() {
    // 0x10000 * 0x10000 = 2^32, which wraps to 0 in i32 arithmetic.
    let src = "
        variable r
        : main  0x10000 0x10000 * r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 0);
}

// ===========================================================================
// Non-trivial classical algorithms
// ===========================================================================

#[test]
fn is_prime_for_small_numbers() {
    // Trial division up to sqrt(n) (approximated by `d*d > n`). Exercises a
    // nested control structure: fast-path checks (<2, =2, even), then a
    // begin/again loop with two `exit`s in different branches.
    fn make_src(n: i32) -> String {
        format!(r#"
            variable r
            : is-prime
                dup 2 <  if drop 0 exit then
                dup 2 =  if drop 1 exit then
                dup 1 and 0 = if drop 0 exit then
                3
                begin
                    2dup dup * swap > if 2drop 1 exit then
                    2dup mod 0 = if 2drop 0 exit then
                    2 +
                again ;
            : main {} is-prime r ! ;
        "#, n)
    }
    for &(n, expected) in &[
        (0, 0), (1, 0), (2, 1), (3, 1), (4, 0),
        (5, 1), (7, 1), (9, 0), (13, 1), (15, 0),
        (25, 0), (29, 1), (49, 0), (53, 1),
    ] {
        let src = make_src(n);
        let got = get_var(&src, "r").unwrap();
        assert_eq!(got, expected, "is-prime({}) expected {}, got {}", n, expected, got);
    }
}

#[test]
fn binary_search_finds_value_in_sorted_array() {
    // 8-element sorted pseudo-array. Search converges in <= 3 iterations.
    // `target` is kept on the data stack across iterations (one `dup` per
    // iter to spawn a comparison copy); the loop's invariant is that the
    // stack holds exactly [target] at every `begin`/`again` boundary.
    let src = "
        variable a0 variable a1 variable a2 variable a3
        variable a4 variable a5 variable a6 variable a7
        variable lo variable hi variable mid variable r
        : array-at  4 * a0 + @ ;
        : binary-search
            0 lo !  7 hi !
            begin
                lo @ hi @ > if drop -1 exit then
                lo @ hi @ + 2 / mid !
                dup mid @ array-at
                2dup = if 2drop drop mid @ exit then
                <
                if   mid @ 1 - hi !
                else mid @ 1 + lo !
                then
            again ;
        : main
            10 a0 !  20 a1 !  30 a2 !  40 a3 !
            50 a4 !  60 a5 !  70 a6 !  80 a7 !
            50 binary-search r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 4);
}

#[test]
fn binary_search_returns_minus_one_when_absent() {
    let src = "
        variable a0 variable a1 variable a2 variable a3
        variable a4 variable a5 variable a6 variable a7
        variable lo variable hi variable mid variable r
        : array-at  4 * a0 + @ ;
        : binary-search
            0 lo !  7 hi !
            begin
                lo @ hi @ > if drop -1 exit then
                lo @ hi @ + 2 / mid !
                dup mid @ array-at
                2dup = if 2drop drop mid @ exit then
                <
                if   mid @ 1 - hi !
                else mid @ 1 + lo !
                then
            again ;
        : main
            10 a0 !  20 a1 !  30 a2 !  40 a3 !
            50 a4 !  60 a5 !  70 a6 !  80 a7 !
            99 binary-search r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), -1);
}

#[test]
fn iterative_factorial_via_do_loop() {
    // Σ i for i in [1, n] via `do/loop`. Alternative to the recursive form
    // — verifies that the do/loop scratch and the multiplicative
    // accumulator on the data stack don't interfere.
    let src = "
        variable r
        : factorial  1 swap 1 + 1 do  i *  loop ;
        : main  6 factorial r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 720);  // 6! = 720
}

#[test]
fn tail_recursive_sum_one_to_hundred() {
    // Accumulator-passing recursion. 100-deep recursion — well within the
    // ~1010-slot retstack but enough to confirm sustained push/pop is clean.
    let src = "
        variable r
        : tail-sum
            dup 0 = if drop exit then
            swap over + swap 1 -
            tail-sum ;
        : main 0 100 tail-sum r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 5050);
}

#[test]
fn max_of_four_via_max_of_two() {
    // Repeated application of a 2-ary helper to a 4-element data stack.
    // `max2` uses `2dup > if ... else ...` (a classic Forth idiom) which
    // mixes a primitive comparison with `nip` from the prelude.
    let src = "
        variable r
        : max2  2dup > if drop else nip then ;
        : main  3 7 2 8 max2 max2 max2 r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 8);
}

#[test]
fn hamming_distance_via_xor_then_popcount() {
    // Hamming distance = popcount(a XOR b). 0x0F = 0b0000_1111,
    // 0xA5 = 0b1010_0101, XOR = 0b1010_1010 → popcount = 4. Uses the same
    // popcount loop shape as the popcount_of_0xcafe test (acc on TOS,
    // remaining bits at NOS).
    let src = "
        variable r
        : hamming
            xor
            0
            begin
                over 0 = if nip exit then
                over 1 and +
                swap 1 rshift swap
            again ;
        : main 0x0F 0xA5 hamming r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 4);
}

#[test]
fn bit_reverse_low_8_bits() {
    // Reverse the low byte of `n`. 0b0000_1011 (11) → 0b1101_0000 (208).
    // 8 iterations of shift-left-out, shift-right-in.
    let src = "
        variable r
        : rev8
            0 swap
            8 0 do
                swap 1 lshift
                over 1 and +
                swap 1 rshift
            loop
            drop ;
        : main  11 rev8 r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 208);
}

#[test]
fn reverse_four_element_array_in_place() {
    // Two-pointer swap: (a0,a3) then (a1,a2). After reversing [10,20,30,40]
    // the array is [40,30,20,10]. Pack into a decimal `wxyz` for one-shot
    // verification.
    let src = "
        variable a0 variable a1 variable a2 variable a3
        variable r
        : reverse4
            a0 @ a3 @  a0 ! a3 !
            a1 @ a2 @  a1 ! a2 ! ;
        : main
            10 a0 !  20 a1 !  30 a2 !  40 a3 !
            reverse4
            a0 @ 1000 *  a1 @ 100 *  a2 @ 10 *  a3 @  + + +  r ! ;
    ";
    // Packed: 40000 + 3000 + 200 + 10 = 43210.
    assert_eq!(get_var(src, "r").unwrap(), 43210);
}

#[test]
fn state_machine_counts_visits_to_target_state() {
    // Bot-like: cycle state through 0→1→2→3→0 over 10 ticks, count how
    // many times we land on state 2. Combines constant, variable, sub-word,
    // if-without-else, and word call inside do/loop — a representative
    // shape for a real LiveCTF bot's main loop.
    let src = "
        constant TARGET 2
        variable state variable count variable r
        : advance  state @ 1 + 4 mod state ! ;
        : tick     advance  state @ TARGET = if count @ 1 + count ! then ;
        : main
            0 state !  0 count !
            10 0 do tick loop
            count @ r ! ;
    ";
    // Ticks land on 1,2,3,0,1,2,3,0,1,2 → state==2 on ticks 2, 6, 10.
    assert_eq!(get_var(src, "r").unwrap(), 3);
}

// ===========================================================================
// Spec-defined defined behaviors (not error paths)
//
// The VM spec explicitly defines several "would-be UB" cases. These tests
// lock that contract in: real bots can rely on these behaviors, so the
// compiler's lowering and the test VM's evaluation must both agree.
// ===========================================================================

#[test]
fn division_by_zero_returns_zero_per_spec() {
    // Spec (Arithmetic table, opcode 0x23):
    //   "Div | left / right (integer; returns 0 if right == 0)"
    // A real bot can use `x 0 /` deliberately as a "produce 0" idiom; the
    // VM must NOT raise an error here.
    let src = "
        variable r
        : main 10 0 / r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 0);
}

#[test]
fn modulo_by_zero_returns_zero_per_spec() {
    // Spec (Arithmetic table, opcode 0x24):
    //   "Rem | left % right (returns 0 if right == 0)"
    let src = "
        variable r
        : main 10 0 mod r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 0);
}

#[test]
fn memory_addresses_wrap_modulo_64k() {
    // Spec (Memory section): "Addresses wrap modulo 65536." A 4-byte write
    // at 0xFFFE straddles 0xFFFE, 0xFFFF, 0x0000, 0x0001 — bytes 2 and 3 of
    // the value land at the low addresses 0x0000 and 0x0001.
    //
    // We test this directly by running a tiny program that writes
    // 0xDEADBEEF at 0xFFFE, then reads back from 0xFFFE; the value must
    // round-trip correctly via the wrap path.
    let src = "
        variable r
        constant ADDR 0xFFFE
        : main  0xDEADBEEF  ADDR !  ADDR @  r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap() as u32, 0xDEADBEEF);
}

// ===========================================================================
// Bitwise corner cases
// ===========================================================================

#[test]
fn rshift_is_logical_not_arithmetic() {
    // SHR uses u32::wrapping_shr — zero-fill, no sign extension. So
    // -1 (0xFFFFFFFF) >> 4 = 0x0FFFFFFF, not -1. Important spec-level
    // distinction; arithmetic shift would give -1.
    let src = "
        variable r
        : main  -1 4 rshift r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 0x0FFFFFFF);
}

#[test]
fn bitwise_invert_is_self_inverse() {
    // ~~x == x for all x. Covers positive, negative, and zero to make sure
    // the BITWISE_NOT lowering doesn't sneak in a sign-aware variant.
    assert_eq!(eval("0xCAFE invert invert"), 0xCAFE);
    assert_eq!(eval("-42 invert invert"), -42);
    assert_eq!(eval("0 invert invert"), 0);
    assert_eq!(eval("0x7FFFFFFF invert invert"), 0x7FFFFFFF);
}

// ===========================================================================
// Min/max via repeated 2-ary reduction
// ===========================================================================

#[test]
fn min_of_five_vars_via_repeated_2dup() {
    // `2dup < if drop else nip then` is the canonical min-of-two combinator.
    // Chained 4 times to fold across 5 elements.
    let src = "
        variable a0 variable a1 variable a2 variable a3 variable a4 variable r
        : min-of-5
            a0 @ a1 @  2dup < if drop else nip then
                 a2 @  2dup < if drop else nip then
                 a3 @  2dup < if drop else nip then
                 a4 @  2dup < if drop else nip then ;
        : main
            30 a0 !  10 a1 !  50 a2 !  20 a3 !  40 a4 !
            min-of-5 r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 10);
}

#[test]
fn max_of_five_vars_via_repeated_2dup() {
    let src = "
        variable a0 variable a1 variable a2 variable a3 variable a4 variable r
        : max-of-5
            a0 @ a1 @  2dup > if drop else nip then
                 a2 @  2dup > if drop else nip then
                 a3 @  2dup > if drop else nip then
                 a4 @  2dup > if drop else nip then ;
        : main
            30 a0 !  10 a1 !  50 a2 !  20 a3 !  40 a4 !
            max-of-5 r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 50);
}

#[test]
fn min_max_of_array_in_single_pass() {
    // Walk a 5-element pseudo-array once, updating both min and max in
    // place. Tests `if dup var !` (update-only-if-extreme), which is a
    // common bot pattern for "best target seen so far".
    let src = "
        variable a0 variable a1 variable a2 variable a3 variable a4
        variable min-v variable max-v variable r
        : read  4 * a0 + @ ;
        : minmax
            a0 @ dup min-v ! max-v !
            5 1 do
                i read
                dup min-v @ < if dup min-v ! then
                dup max-v @ > if dup max-v ! then
                drop
            loop ;
        : main
            30 a0 !  10 a1 !  50 a2 !  20 a3 !  40 a4 !
            minmax
            min-v @ 1000 *  max-v @  +  r ! ;
    ";
    // Packed: min=10, max=50 → 10*1000 + 50 = 10050.
    assert_eq!(get_var(src, "r").unwrap(), 10050);
}

// ===========================================================================
// Buffer / string-style patterns: pseudo-array indexed by computed address
// ===========================================================================

#[test]
fn string_length_with_zero_sentinel() {
    // Count cells from c0 onward until hitting a zero. The classic Forth
    // C-string idiom; produces 3 for "HI!" + sentinel.
    let src = "
        variable c0 variable c1 variable c2 variable c3 variable c4 variable r
        : strlen
            0
            begin
                dup 4 * c0 + @  0 = if exit then
                1 +
            again ;
        : main
            72 c0 !  73 c1 !  33 c2 !  0 c3 !  0 c4 !
            strlen r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 3);
}

#[test]
fn fibonacci_array_via_computed_address() {
    // Fill f[0..10] in place using the recurrence f[i] = f[i-1] + f[i-2].
    // Exercises the read/write helper pair (each a sub-word with its own
    // prolog/epilog) called from inside a do/loop.
    let src = "
        variable f0 variable f1 variable f2 variable f3 variable f4
        variable f5 variable f6 variable f7 variable f8 variable f9
        variable r
        : read   4 * f0 + @ ;
        : write  4 * f0 + ! ;
        : fill-fib
            0 0 write  1 1 write
            10 2 do
                i 1 - read  i 2 - read  +  i write
            loop ;
        : main  fill-fib  9 read r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 34);  // fib(9) = 34
}

// ===========================================================================
// Bot-shaped patterns: cooldown timer and ring-buffer queue
// ===========================================================================

#[test]
fn cooldown_timer_fires_every_n_ticks() {
    // Bot pattern: a per-tick action fires only when its cooldown reaches 0,
    // then resets. With cooldown=3 and 20 ticks we expect 6 fires.
    let src = "
        variable cooldown variable hits variable r
        : tick
            cooldown @ 1 - cooldown !
            cooldown @ 0 = if hits @ 1 + hits !  3 cooldown ! then ;
        : main
            3 cooldown !  0 hits !
            20 0 do tick loop
            hits @ r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 6);
}

#[test]
fn ring_buffer_queue_preserves_fifo_order() {
    // Push 10/20/30, dequeue all three and pack: 10*100 + 20*10 + 30 = 1230.
    // Head/tail wrap with mod, so the buffer is genuinely circular even
    // though we don't exercise wrap here.
    let src = "
        variable q0 variable q1 variable q2 variable q3 variable q4
        variable head variable tail variable count variable r
        : enqueue
            tail @ 4 * q0 + !
            tail @ 1 + 5 mod tail !
            count @ 1 + count ! ;
        : dequeue
            head @ 4 * q0 + @
            head @ 1 + 5 mod head !
            count @ 1 - count ! ;
        : main
            0 head !  0 tail !  0 count !
            10 enqueue  20 enqueue  30 enqueue
            dequeue 100 *  dequeue 10 *  +  dequeue  +  r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 1230);
}

#[test]
fn two_sum_finds_pair_indices_in_array() {
    // O(n²) search for a pair summing to `target`. The outer-loop index is
    // stashed in a variable since the compiler exposes only `i` (innermost).
    // Returns the packed index (outer*10 + inner) of the first match.
    let src = "
        variable a0 variable a1 variable a2 variable a3 variable a4
        variable target variable outer-i variable r
        : read  4 * a0 + @ ;
        : two-sum
            5 0 do
                i outer-i !
                5 0 do
                    outer-i @ i <> if
                        outer-i @ read  i read  +  target @ =
                        if  outer-i @ 10 *  i +  r !  exit  then
                    then
                loop
            loop ;
        : main
            1 a0 !  3 a1 !  5 a2 !  7 a3 !  9 a4 !
            8 target !  -1 r !
            two-sum ;
    ";
    // a[0]+a[3] = 1+7 = 8 → packed = 0*10 + 3 = 3.
    assert_eq!(get_var(src, "r").unwrap(), 3);
}

// ===========================================================================
// Combinatorial / recursive computation with branching call tree
// ===========================================================================

#[test]
fn binomial_coefficient_5_choose_2() {
    // C(n, k) = C(n-1, k-1) + C(n-1, k), base cases C(n, 0) = C(n, n) = 1.
    // Doubly-recursive; for (5,2) it explores ~21 calls, max depth 4. Pure
    // stack-based recursion (no scratch vars) — confirms the calling
    // convention handles two consecutive recursive calls with intermediate
    // state preserved on the data stack.
    let src = "
        variable r
        : C
            dup 0 =        if 2drop 1 exit then
            over over =    if 2drop 1 exit then
            2dup
            1 -
            swap 1 - swap
            C
            -rot
            swap 1 - swap
            C
            + ;
        : main 5 2 C r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 10);
}

// ===========================================================================
// Loops on number representations
// ===========================================================================

#[test]
fn digit_count_for_various_positive_inputs() {
    // Edge case: 0 has 1 digit (special-cased at the top). Otherwise count
    // /10 iterations until n reaches 0.
    fn make_src(n: i32) -> String {
        format!(r#"
            variable r
            : digit-count
                dup 0 = if drop 1 exit then
                0 swap
                begin
                    dup 0 = if drop exit then
                    swap 1 + swap
                    10 /
                again ;
            : main {} digit-count r ! ;
        "#, n)
    }
    for &(n, expected) in &[
        (0, 1), (1, 1), (9, 1), (10, 2), (99, 2),
        (100, 3), (1000, 4), (12345, 5),
    ] {
        let src = make_src(n);
        let got = get_var(&src, "r").unwrap();
        assert_eq!(got, expected, "digit-count({}) expected {}, got {}", n, expected, got);
    }
}

#[test]
fn power_of_two_detection() {
    // Classic trick: n is a power of 2 iff (n & (n-1)) == 0 (for n > 0).
    fn make_src(n: i32) -> String {
        format!("
            variable r
            : pow2?  dup 1 - and 0 = ;
            : main  {} pow2? r ! ;
        ", n)
    }
    for &(n, expected) in &[
        (1, 1), (2, 1), (4, 1), (8, 1), (16, 1), (1024, 1),
        (3, 0), (5, 0), (6, 0), (7, 0), (100, 0), (1023, 0),
    ] {
        let src = make_src(n);
        let got = get_var(&src, "r").unwrap();
        assert_eq!(got, expected, "pow2?({}) expected {}, got {}", n, expected, got);
    }
}

#[test]
fn brian_kernighan_popcount_matches_naive() {
    // BK popcount: while n != 0 { n &= n-1; count++ }. Each iteration
    // clears the lowest set bit. Runs in O(popcount) instead of O(bitwidth),
    // and should agree with the naive shift-and-test version on 0xCAFE.
    let src = "
        variable r
        : bk-popcount
            0
            begin
                over 0 = if nip exit then
                swap dup 1 - and swap
                1 +
            again ;
        : main  0xCAFE bk-popcount r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 11);
}

// ===========================================================================
// More numeric algorithms: Newton's sqrt, log2, parity, byte-swap, ...
// ===========================================================================

#[test]
fn integer_sqrt_via_newtons_method() {
    // Newton iteration with two stopping conditions: new == guess (clean
    // convergence) or new > guess (oscillation, return the smaller guess).
    // Tests `2dup`, repeated SP-relative compares, and `nip nip` cleanup.
    fn make_src(n: i32) -> String {
        format!("
            variable r
            : isqrt
                dup 0 = if exit then
                dup
                begin
                    2dup / over + 2 /
                    over over =  if nip nip exit then
                    over over <  if drop nip exit then
                    nip
                again ;
            : main  {} isqrt r ! ;
        ", n)
    }
    for &(n, expected) in &[
        (0, 0), (1, 1), (2, 1), (3, 1), (4, 2),
        (8, 2), (9, 3), (10, 3), (15, 3), (16, 4),
        (24, 4), (25, 5), (100, 10), (101, 10), (1024, 32),
    ] {
        let src = make_src(n);
        let got = get_var(&src, "r").unwrap();
        assert_eq!(got, expected, "isqrt({}) expected {}, got {}", n, expected, got);
    }
}

#[test]
fn integer_log2_for_powers_of_two() {
    // Floor(log2(n)) via shift-and-count. Exits the loop when n shrinks to
    // ≤1, returning the shift count.
    fn make_src(n: i32) -> String {
        format!("
            variable r
            : log2
                0 swap
                begin
                    dup 1 <= if drop exit then
                    1 rshift  swap 1 + swap
                again ;
            : main  {} log2 r ! ;
        ", n)
    }
    for &(n, expected) in &[
        (1, 0), (2, 1), (4, 2), (8, 3), (16, 4),
        (256, 8), (1024, 10), (65536, 16),
    ] {
        let src = make_src(n);
        let got = get_var(&src, "r").unwrap();
        assert_eq!(got, expected, "log2({}) expected {}", n, expected);
    }
}

#[test]
fn parity_via_xor_fold() {
    // XOR-fold halves the bit width each step; the final LSB is the parity.
    // The fold works for any width since XOR is addition mod 2.
    fn make_src(n: u32) -> String {
        format!("
            variable r
            : parity
                dup 4 rshift xor
                dup 2 rshift xor
                dup 1 rshift xor
                1 and ;
            : main  0x{:X} parity r ! ;
        ", n)
    }
    for &(n, expected) in &[
        (0x00, 0), (0x01, 1), (0x03, 0), (0x07, 1), (0x0F, 0),
        (0x55, 0), (0xAA, 0), (0xFF, 0), (0x80, 1), (0x88, 0),
        (0xCAFE, 1),
    ] {
        let src = make_src(n);
        let got = get_var(&src, "r").unwrap();
        assert_eq!(got, expected, "parity(0x{:X}) expected {}", n, expected);
    }
}

#[test]
fn byte_swap_32bit_reverses_byte_order() {
    // Classic 4-mask byte swap. Stresses 0xFF000000 as a large literal
    // (sign-extends to negative i32 but rounds-trips through u32 bitwise
    // AND correctly).
    let src = "
        variable r
        : byte-swap32
            dup 0xFF and 24 lshift
            over 0xFF00 and 8 lshift or
            over 0xFF0000 and 8 rshift or
            swap 0xFF000000 and 24 rshift or ;
        : main  0x12345678 byte-swap32 r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap() as u32, 0x78563412);
}

// ===========================================================================
// Stack-shape patterns: composition, conditional swap, 3+ args
// ===========================================================================

#[test]
fn function_composition_via_chained_words() {
    // cube(x) = square(x) * x. Confirms a sub-word can compute and return
    // an intermediate that the caller then folds into a further computation
    // — basic functional composition over the data stack.
    let src = "
        variable r
        : square  dup * ;
        : cube    dup square * ;
        : main    3 cube r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 27);
}

#[test]
fn three_way_mutual_recursion_cycles_correctly() {
    // rock → paper → scissors → rock, decrementing n by 1 each transition.
    // Exercises 3-way forward references (the first two `:` definitions are
    // both forward refs to words not yet defined).
    let src = "
        variable depth variable r
        : rock      dup 0 = if drop exit then  depth @ 1 + depth !  1 - paper ;
        : paper     dup 0 = if drop exit then  depth @ 1 + depth !  1 - scissors ;
        : scissors  dup 0 = if drop exit then  depth @ 1 + depth !  1 - rock ;
        : main  0 depth !  9 rock  depth @ r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 9);
}

#[test]
fn conditional_swap_orders_top_two() {
    // sort2(a, b) leaves (min, max) on the stack. The caller drains them
    // into separate variables to verify the ordering.
    let src = "
        variable q variable r
        : sort2  2dup > if swap then ;
        : main   7 3 sort2 r ! q ! ;
    ";
    // sort2(7, 3): 7>3 true → swap → [3, 7]. TOS=7 → r=7, then NOS=3 → q=3.
    assert_eq!(get_var(src, "r").unwrap(), 7);
    assert_eq!(get_var(src, "q").unwrap(), 3);
}

// ===========================================================================
// Distance metrics + alternative algorithms
// ===========================================================================

#[test]
fn manhattan_distance_via_variables() {
    // |x1-x2| + |y1-y2|. Stashes 4 args into globals so we can read them in
    // any order; `abs` is a sub-word that calls `negate` (a primitive).
    let src = "
        variable x1v variable y1v variable x2v variable y2v variable r
        : abs  dup 0 < if negate then ;
        : manhattan
            y2v ! x2v ! y1v ! x1v !
            x1v @ x2v @ - abs
            y1v @ y2v @ - abs
            + ;
        : main  1 1 4 5 manhattan r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 7);  // |1-4| + |1-5| = 3 + 4
}

#[test]
fn gcd_via_subtraction_alternative() {
    // Subtraction-only GCD: repeatedly replace the larger of (a, b) with
    // their difference until they're equal. Slower than the mod-based form
    // but uses only `-` (no `mod`/`/`), so it exercises a different lowering
    // path.
    let src = "
        variable av variable bv variable r
        : abs-diff
            av @ bv @ > if  av @ bv @ -  av !
            else            bv @ av @ -  bv !
            then ;
        : gcd
            bv ! av !
            begin
                av @ bv @ = if av @ exit then
                abs-diff
            again ;
        : main 48 18 gcd r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 6);
}

// ===========================================================================
// String-shaped: Caesar cipher over a 5-cell pseudo-array
// ===========================================================================

#[test]
fn caesar_cipher_shifts_uppercase_letters() {
    // shift+3 over HELLO → KHOOR. Pack the first 4 cells into one i32 as a
    // decimal `wxyz` for a single round-trip check.
    let src = "
        variable c0 variable c1 variable c2 variable c3 variable c4
        variable shift-by variable r
        : shift  65 -  shift-by @ +  26 mod  65 + ;
        : encrypt
            5 0 do
                i 4 * c0 +  dup @  shift  swap !
            loop ;
        : main
            72 c0 !  69 c1 !  76 c2 !  76 c3 !  79 c4 !
            3 shift-by !
            encrypt
            c0 @ 1000 *  c1 @ 100 *  c2 @ 10 *  c3 @  + + +  r ! ;
    ";
    // K=75, H=72, O=79, O=79 → 75*1000 + 72*100 + 79*10 + 79 = 83069.
    assert_eq!(get_var(src, "r").unwrap(), 83069);
}

// ===========================================================================
// Spec-defined wrapping edge: NEG of i32::MIN
// ===========================================================================

#[test]
fn negate_i32_min_wraps_to_itself() {
    // Spec: NEG is "-top wrapping". i32::MIN's positive counterpart doesn't
    // fit in i32, so wrapping_neg returns i32::MIN. A bot using `negate` on
    // possibly-extreme values must be prepared for this fixed point.
    let src = "
        variable r
        : main  -0x80000000  negate  r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), i32::MIN);
}

// ===========================================================================
// LiveCTF spec patterns: packed position with i16 fields
//
// Syscall 2 (Input) returns: (current_x & 0xFFFF) | (current_y << 16),
// where x and y are i16. Unpacking requires sign extension of each 16-bit
// half. These tests lock in the canonical pack/unpack idioms a bot will use.
// ===========================================================================

#[test]
fn i16_sign_extension_via_high_bit_check() {
    // Extend a u16-shaped value into an i32 by checking bit 15 and OR-ing
    // 0xFFFF0000 if set. 0xFFFF → -1, 0x7FFF → 32767, 0x8000 → -32768.
    fn make_src(v: u32) -> String {
        format!("
            variable r
            : sign-ext16  dup 0x8000 and if 0xFFFF0000 or then ;
            : main  0x{:X} sign-ext16 r ! ;
        ", v)
    }
    for &(v, expected) in &[
        (0x0000_u32, 0_i32), (0x0001, 1), (0x7FFF, 32767),
        (0x8000, -32768), (0xFFFF, -1), (0xC000, -16384),
    ] {
        let src = make_src(v);
        assert_eq!(get_var(&src, "r").unwrap(), expected, "sign-ext16(0x{:X})", v);
    }
}

#[test]
fn position_pack_unpack_round_trip() {
    // Pack (x=5, y=-3) the same way Syscall 2 reports a position, then
    // unpack both halves and recombine into a single i32 to verify
    // round-trip correctness. r = 100*y + x = 100*(-3) + 5 = -295.
    let src = "
        variable r
        : pack-pos    16 lshift  swap  0xFFFF and  or ;
        : sign-ext16  dup 0x8000 and if 0xFFFF0000 or then ;
        : unpack-x    0xFFFF and  sign-ext16 ;
        : unpack-y    16 rshift  sign-ext16 ;
        : main
            5 -3 pack-pos
            dup unpack-x  swap unpack-y
            100 *  +  r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), -295);
}

#[test]
fn position_unpack_y_sign_extends_negative() {
    // For a packed value with y=-3 in the high 16 bits, unpack-y should
    // give -3 (not 65533). This is the single most error-prone step in
    // working with the Input syscall return value.
    let src = "
        variable r
        : unpack-y  16 rshift  dup 0x8000 and if 0xFFFF0000 or then ;
        : main  0xFFFD0005 unpack-y r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), -3);
}

// ===========================================================================
// Bounds clamping (bot pattern for keeping moves inside map_extent)
// ===========================================================================

#[test]
fn clamp_value_into_bounds() {
    // Clamp to [-10, 10]. Tests `dup -10 < if drop -10 exit then` + same
    // for upper bound. Covers below, in-range, equal-to-bound, and above.
    fn make_src(v: i32) -> String {
        format!("
            variable r
            : clamp
                dup -10 < if drop -10 exit then
                dup  10 > if drop  10 exit then ;
            : main  {} clamp r ! ;
        ", v)
    }
    for &(v, expected) in &[
        (15, 10), (-20, -10), (5, 5), (0, 0), (10, 10), (-10, -10),
    ] {
        let src = make_src(v);
        let got = get_var(&src, "r").unwrap();
        assert_eq!(got, expected, "clamp({}) expected {}", v, expected);
    }
}

// ===========================================================================
// Target-selection patterns (closest-enemy / best-target)
// ===========================================================================

#[test]
fn argmin_of_four_distances_picks_closest() {
    // Walk a 4-element pseudo-array of distances and return the index of
    // the minimum. The body inside `do` does a conditional store-and-update
    // — the "remember best so far" idiom.
    let src = "
        variable d0 variable d1 variable d2 variable d3
        variable min-d variable min-i variable r
        : read  4 * d0 + @ ;
        : argmin
            d0 @ min-d !  0 min-i !
            4 1 do
                i read  dup min-d @ <
                if   min-d !  i min-i !
                else drop
                then
            loop
            min-i @ ;
        : main
            20 d0 !  5 d1 !  30 d2 !  15 d3 !
            argmin r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 1);  // d1 = 5 is the smallest
}

#[test]
fn argmax_of_four_values_picks_largest_index() {
    let src = "
        variable v0 variable v1 variable v2 variable v3
        variable max-v variable max-i variable r
        : read  4 * v0 + @ ;
        : argmax
            v0 @ max-v !  0 max-i !
            4 1 do
                i read  dup max-v @ >
                if   max-v !  i max-i !
                else drop
                then
            loop
            max-i @ ;
        : main
            10 v0 !  50 v1 !  30 v2 !  20 v3 !
            argmax r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 1);  // v1 = 50 is the largest
}

// ===========================================================================
// Lookup-table popcount (alternative algorithm using consecutive vars)
// ===========================================================================

#[test]
fn popcount_via_4bit_lookup_table() {
    // 16 vars t0..tF hold popcount(0)..popcount(15). The pseudo-array
    // indexing (`4 * t0 + @`) lets us treat them as a 16-cell i32 array.
    // popcount(byte) = lookup[byte & 0xF] + lookup[(byte >> 4) & 0xF].
    // For 0xCA = 0b1100_1010: lookup[0xA] + lookup[0xC] = 2 + 2 = 4.
    let src = "
        variable t0 variable t1 variable t2 variable t3
        variable t4 variable t5 variable t6 variable t7
        variable t8 variable t9 variable tA variable tB
        variable tC variable tD variable tE variable tF
        variable r
        : lookup  4 * t0 + @ ;
        : main
            0 t0 !  1 t1 !  1 t2 !  2 t3 !
            1 t4 !  2 t5 !  2 t6 !  3 t7 !
            1 t8 !  2 t9 !  2 tA !  3 tB !
            2 tC !  3 tD !  3 tE !  4 tF !
            0xCA dup 0xF and lookup
            swap 4 rshift 0xF and lookup
            +  r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 4);
}

// ===========================================================================
// Composition with do/loop scratch: a recursive word whose body contains a
// `do/loop`. Safe because the inner loop completes before the self-call;
// scratch slot 0 is dead by the time we recurse.
// ===========================================================================

#[test]
fn recursive_word_with_do_loop_before_self_call() {
    // f(n) runs a 3-iter loop accumulating 0+1+2=3 into `counter`, then
    // tail-calls f(n-1). f(0) exits without looping. For input 5, the body
    // runs 5 times, adding 15 total.
    let src = "
        variable counter variable r
        : f
            dup 0 = if drop exit then
            3 0 do  i counter @ + counter !  loop
            1 - f ;
        : main  0 counter !  5 f  counter @ r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 15);
}

// ===========================================================================
// Array-shaped analyses: longest run, prefix sum
// ===========================================================================

#[test]
fn longest_consecutive_run_length() {
    // Walk a 5-element array tracking current and max run lengths. The
    // `if ... if ... then ... else ... then` shows nested conditionals
    // inside a do/loop.
    let src = "
        variable a0 variable a1 variable a2 variable a3 variable a4
        variable max-run variable cur-run variable r
        : read  4 * a0 + @ ;
        : main
            1 a0 !  1 a1 !  1 a2 !  2 a3 !  3 a4 !
            1 max-run !  1 cur-run !
            5 1 do
                i read  i 1 - read  =
                if cur-run @ 1 + cur-run !
                   cur-run @ max-run @ > if cur-run @ max-run ! then
                else 1 cur-run !
                then
            loop
            max-run @ r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 3);  // three 1's at the start
}

#[test]
fn prefix_sum_in_place_via_array() {
    // Compute a[i] = a[i-1] + a[i] for i in [1, 5). Input [1,2,3,4,5]
    // becomes [1,3,6,10,15]. The final cell holds the total. Tests
    // sub-words (`read`/`write`) called from inside a do/loop where the
    // loop index is BOTH a read source AND a write target.
    let src = "
        variable a0 variable a1 variable a2 variable a3 variable a4
        variable r
        : read   4 * a0 + @ ;
        : write  4 * a0 + ! ;
        : main
            1 a0 !  2 a1 !  3 a2 !  4 a3 !  5 a4 !
            5 1 do
                i 1 - read  i read  +  i write
            loop
            a4 @ r ! ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 15);
}

// ===========================================================================
// Compile-time structural edges that we also want to runtime-validate
// ===========================================================================

#[test]
fn deeply_nested_if_then_chain_runs_to_innermost() {
    // 5-deep nesting where all conditions are truthy. Final write happens
    // at the bottom of the cone; we verify r got set.
    let src = "
        variable r
        : main
            1 if 1 if 1 if 1 if 1 if
                42 r !
            then then then then then ;
    ";
    assert_eq!(get_var(src, "r").unwrap(), 42);
}

#[test]
fn long_sequential_word_calls_stress_trampoline() {
    // 200 sequential calls to a tiny leaf word. Each call exercises the full
    // PROLOG_HELPER + EPILOG_HELPER round-trip; if any frame leaks on the
    // return stack the counter will diverge.
    let mut src = String::from("
        variable counter variable r
        : tick counter @ 1 + counter ! ;
        : main 0 counter !
    ");
    for _ in 0..200 {
        src.push_str(" tick");
    }
    src.push_str(" counter @ r ! ;");
    assert_eq!(get_var(&src, "r").unwrap(), 200);
}

#[test]
fn bubble_sort_three_vars_in_place() {
    // Sort (8, 5, 2) in three passes. Each pass conditionally swaps an
    // adjacent pair via 2dup + > + if/else. Final order should be a≤b≤c.
    let src = "
        variable a variable b variable c
        : main
            8 a !  5 b !  2 c !
            a @ b @ 2dup > if a ! b ! else 2drop then
            b @ c @ 2dup > if b ! c ! else 2drop then
            a @ b @ 2dup > if a ! b ! else 2drop then ;
    ";
    assert_eq!(get_var(src, "a").unwrap(), 2);
    assert_eq!(get_var(src, "b").unwrap(), 5);
    assert_eq!(get_var(src, "c").unwrap(), 8);
}

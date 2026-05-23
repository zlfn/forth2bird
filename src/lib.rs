//! LiveCTF Forth-style compiler — library crate.
//!
//! Public surface:
//!   - [`op`]: VM opcode constants (shared with the `fthd` disassembler).
//!   - [`lexer`]: `tokenize` and `parse_number` (re-exported at the crate root).
//!   - [`compiler::Compiler`]: the compiler itself; build a `.bin` with
//!     [`compile_program`] for the one-shot case.
//!   - Memory-layout constants ([`PROLOG_HELPER_ADDR`], etc.): exposed for
//!     integration tests that assert bytecode shape.
//!
//! See `LANGUAGE.md` for the dialect reference and `COOKBOOK.md` for verified
//! idioms.

pub mod op;
pub mod lexer;
pub mod compiler;

pub use compiler::{Compiler, DictEntry};
pub use lexer::{parse_number, tokenize};

// ===========================================================================
// Top-level memory layout
// ===========================================================================
//
// The full layout (also documented in `LANGUAGE.md` / `README.md`):
//
//   0x0000..0x0011    bootstrap (18 B)
//   0x0012..0x0031    PROLOG_HELPER  (32 B)
//   0x0032..0x0045    EPILOG_HELPER  (20 B)
//   0x0046..0x6FFF    user code + variables (≤ 28 KB)
//   0x7000..0x7FC7    return stack (grows down from 0x7FC4)
//   0x7FC8..0x7FCB    PROLOG_SCRATCH
//   0x7FCC..0x7FCF    RSP storage
//   0x7FD0..0x7FEF    do/loop scratch (4 levels × 8 B)
//   0x7FF0..0x7FFF    prelude scratch (swap/nip/rot temps)
//   0x8000..0xFFFF    data stack (grows up from 0x8000)
//
// CALL_ABS pushes the return address onto the data stack, which would clash
// with Forth primitives operating on TOS. Every `:` body therefore gets a
// 6-byte prolog (`PUSH PROLOG_HELPER_ADDR; CALL_ABS`) that hops through the
// shared helper to move the RA onto the return stack, and every `;`/`exit`
// gets a matching 6-byte epilog. RSP grows DOWN: push decrements by 4, pop
// increments by 4.

/// do/loop scratch base. Each nesting level reserves 8 bytes here
/// (index + limit at offsets 0 and 4).
pub const DO_LOOP_BASE: u32 = 0x7FD0;
pub const DO_LOOP_MAX_DEPTH: usize = 4;

/// 4-byte temp used by PROLOG_HELPER to stash the body-entry address
/// while it juggles the caller RA.
pub const PROLOG_SCRATCH_ADDR: u32 = 0x7FC8;

/// 4 bytes holding the current return-stack pointer.
pub const RSP_STORAGE_ADDR: u32 = 0x7FCC;

/// Initial value written into `RSP_STORAGE_ADDR` by the bootstrap — points
/// just past the top of the return-stack region.
pub const RETSTACK_INITIAL: u32 = 0x7FC8;

/// Fixed entry points for the shared trampoline helpers.
pub const PROLOG_HELPER_ADDR: u16 = 18;
pub const EPILOG_HELPER_ADDR: u16 = 50;

/// Per-`:` overhead: 6-byte prolog thunk + 6-byte epilog thunk.
pub const PROLOG_LEN: usize = 6;
pub const EPILOG_LEN: usize = 6;

/// Bootstrap + both helper bodies. User-visible code begins at this offset.
pub const PREAMBLE_LEN: usize = 70;

/// Highest legal end-of-code address. The spec only requires the binary
/// to fit in 64 KB, but at runtime we use 0x7000..0x7FC7 as a return
/// stack (topmost slot at 0x7FC4..0x7FC7, growing down). If code crossed 0x7000, a sufficiently
/// deep call chain would push the return stack down into the code region
/// and silently corrupt it. Cap code+vars at 0x7000 so the retstack
/// always has ~1000 frames of clearance.
pub const MAX_CODE_END: usize = 0x7000;

// ===========================================================================
// Prelude — stack-manipulation words built on a scratch memory region
// ===========================================================================

pub const PRELUDE: &str = include_str!("prelude.fth");

// ===========================================================================
// One-shot helper
// ===========================================================================

/// Compile prelude + user source and return finalized bytecode. Equivalent
/// to manually `Compiler::new()` + `compile(PRELUDE)` + `compile(src)` +
/// `finalize()` + `into_bytes()`.
pub fn compile_program(src: &str) -> Result<Vec<u8>, String> {
    let mut c = Compiler::new();
    c.compile(PRELUDE).map_err(|e| format!("prelude: {}", e))?;
    c.compile(src)?;
    c.finalize()?;
    Ok(c.into_bytes())
}

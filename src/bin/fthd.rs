// fthd — LiveCTF VM disassembler.
//
// Linear-sweep decoder. For each Push immediately followed by an absolute
// call/jump (or any push followed by a relative call/jump), computes the
// target address and emits a label there.  Labels make calls and branches
// navigable; raw bytes alone are pretty inscrutable for anything but the
// smallest binaries.

use livectf_forth::op;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::process::ExitCode;

struct Line {
    addr: usize,
    bytes: Vec<u8>,
    mnemonic: &'static str,
    operand: String,
    target: Option<u16>,
    immediate: Option<u32>, // value pushed (for PUSH/PUSH_SHORT/PUSH_ZERO)
}

/// Decode the instruction at `code[ip]`. `last_push` is the value pushed by
/// the *immediately previous* instruction, if any — used to resolve targets
/// for Push+Call/Jump idioms.
fn decode_one(code: &[u8], ip: usize, last_push: &mut Option<u32>) -> Line {
    let opcode = code[ip];
    let prev_push = *last_push;
    let mut len = 1;
    let mut mnemonic: &'static str = "???";
    let mut operand = String::new();
    let mut target: Option<u16> = None;
    let mut next_push: Option<u32> = None;

    let rel_target = |off: u32| -> u16 {
        let off = off as i32;
        (((ip + 1) as i32).wrapping_add(off) as u32 & 0xFFFF) as u16
    };

    match opcode {
        op::HALT         => mnemonic = "HALT",
        op::PUSH         => {
            mnemonic = "PUSH";
            if ip + 5 <= code.len() {
                let v = u32::from_le_bytes(code[ip + 1..ip + 5].try_into().unwrap());
                len = 5;
                operand = format!("0x{:x} ({})", v, v as i32);
                next_push = Some(v);
            } else {
                operand = "(truncated)".into();
            }
        }
        op::PUSH_ZERO    => {
            mnemonic = "PUSH_ZERO";
            next_push = Some(0);
        }
        op::PUSH_SHORT   => {
            mnemonic = "PUSH_SHORT";
            if ip + 2 <= code.len() {
                let v = code[ip + 1] as i8;
                len = 2;
                operand = format!("{}", v);
                next_push = Some(v as i32 as u32);
            } else {
                operand = "(truncated)".into();
            }
        }
        op::POP          => mnemonic = "POP",
        op::LOAD_SP_REL  => mnemonic = "LOAD_SP_REL",
        op::STORE_SP_REL => mnemonic = "STORE_SP_REL",
        op::LOAD_ABS     => mnemonic = "LOAD_ABS",
        op::STORE_ABS    => mnemonic = "STORE_ABS",
        op::JUMP_REL     => {
            mnemonic = "JUMP_REL";
            target = prev_push.map(rel_target);
        }
        op::JUMP_ABS     => {
            mnemonic = "JUMP_ABS";
            target = prev_push.map(|v| v as u16);
        }
        op::CALL_REL     => {
            mnemonic = "CALL_REL";
            target = prev_push.map(rel_target);
        }
        op::CALL_ABS     => {
            mnemonic = "CALL_ABS";
            target = prev_push.map(|v| v as u16);
        }
        op::SKIP         => mnemonic = "SKIP",
        op::SYSCALL      => mnemonic = "SYSCALL",
        op::LT           => mnemonic = "LT",
        op::LE           => mnemonic = "LE",
        op::GT           => mnemonic = "GT",
        op::GE           => mnemonic = "GE",
        op::EQ           => mnemonic = "EQ",
        op::NE           => mnemonic = "NE",
        op::ADD          => mnemonic = "ADD",
        op::SUB          => mnemonic = "SUB",
        op::MUL          => mnemonic = "MUL",
        op::DIV          => mnemonic = "DIV",
        op::REM          => mnemonic = "REM",
        op::AND          => mnemonic = "AND",
        op::OR           => mnemonic = "OR",
        op::XOR          => mnemonic = "XOR",
        op::SHL          => mnemonic = "SHL",
        op::SHR          => mnemonic = "SHR",
        op::LOGICAL_NOT  => mnemonic = "LOGICAL_NOT",
        op::BITWISE_NOT  => mnemonic = "BITWISE_NOT",
        op::NEG          => mnemonic = "NEG",
        _ => {
            // Unknown opcode — the VM treats these as HALT. Could be data
            // (variable storage, immediate-byte tail of a Push, etc.).
            operand = format!("0x{:02x}", opcode);
        }
    }

    *last_push = next_push;
    let end = (ip + len).min(code.len());
    Line {
        addr: ip,
        bytes: code[ip..end].to_vec(),
        mnemonic,
        operand,
        target,
        immediate: next_push,
    }
}

/// Recognise `PUSH/PUSH_SHORT <X>; MUL; JUMP_REL` as a conditional jump whose
/// max distance is X (the Forth-emitted `if`/`until` idiom).  The JUMP_REL's
/// `last_push` was reset by the MUL, so the basic decoder misses the target.
fn annotate_conditional_jumps(lines: &mut [Line]) {
    for i in 2..lines.len() {
        if lines[i].target.is_some() {
            continue;
        }
        let is_rel_jump = matches!(lines[i].mnemonic, "JUMP_REL" | "CALL_REL");
        if !is_rel_jump {
            continue;
        }
        if lines[i - 1].mnemonic != "MUL" {
            continue;
        }
        let pusher = &lines[i - 2];
        if !matches!(pusher.mnemonic, "PUSH" | "PUSH_SHORT" | "PUSH_ZERO") {
            continue;
        }
        if let Some(off) = pusher.immediate {
            let off = off as i32;
            let target = (((lines[i].addr + 1) as i32).wrapping_add(off) as u32 & 0xFFFF) as u16;
            lines[i].target = Some(target);
        }
    }
}

fn disassemble(code: &[u8]) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut ip = 0;
    let mut last_push: Option<u32> = None;
    while ip < code.len() {
        let line = decode_one(code, ip, &mut last_push);
        let step = line.bytes.len().max(1);
        lines.push(line);
        ip += step;
    }
    lines
}

fn render(lines: &[Line]) -> String {
    // Collect all referenced targets so we can emit labels at those addresses.
    let mut targets: HashSet<u16> = HashSet::new();
    for l in lines {
        if let Some(t) = l.target {
            targets.insert(t);
        }
    }
    // 0x0000 is always the entry point — useful to label too.
    targets.insert(0);

    let mut out = String::new();
    for l in lines {
        if targets.contains(&(l.addr as u16)) {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&format!("L{:04x}:\n", l.addr));
        }
        let hex: String = l.bytes
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(" ");
        let comment = if let Some(t) = l.target {
            format!("    ; -> L{:04x}", t)
        } else {
            String::new()
        };
        out.push_str(&format!(
            "{:04x}:  {:<14}  {:<12} {:<18}{}\n",
            l.addr, hex, l.mnemonic, l.operand, comment,
        ));
    }
    out
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: {} <bot.bin>", args[0]);
        return ExitCode::from(2);
    }
    let code = match fs::read(&args[1]) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("cannot read {}: {}", args[1], e);
            return ExitCode::from(1);
        }
    };
    let mut lines = disassemble(&code);
    annotate_conditional_jumps(&mut lines);
    print!("{}", render(&lines));
    ExitCode::SUCCESS
}

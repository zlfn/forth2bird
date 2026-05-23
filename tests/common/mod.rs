// Shared test infrastructure: a small reference VM that models the LiveCTF
// bytecode per the spec, plus convenience helpers for compile-and-run tests.
//
// The VM exists so that runtime tests verify *Forth semantics* — e.g. a
// failing `7 5 -` test points at the compiler's operand ordering, not at
// the VM. Therefore the VM's binary ops follow the spec literally:
//
//     left  = pop();   // TOS
//     right = pop();   // NOS
//     push(left OP right);
//
// If you find yourself "fixing" the VM to make a compiler test pass, stop
// — the compiler is probably wrong. The VM is intentionally a dumb mirror
// of the spec.

#![allow(dead_code)]

use livectf_forth::{op, Compiler, DictEntry, PRELUDE};

pub const STACK_BASE: u32 = 0x8000;
pub const MEM_SIZE: usize = 0x10000;
pub const DEFAULT_STEP_LIMIT: usize = 200_000;

pub struct Vm {
    pub mem: Vec<u8>,
    pub sp: u32,
    pub ip: u32,
    pub halted: bool,
    pub steps: usize,
}

impl Vm {
    pub fn load(bytecode: &[u8]) -> Self {
        assert!(bytecode.len() <= MEM_SIZE, "bytecode too large: {}", bytecode.len());
        let mut mem = vec![0u8; MEM_SIZE];
        mem[..bytecode.len()].copy_from_slice(bytecode);
        Vm { mem, sp: STACK_BASE, ip: 0, halted: false, steps: 0 }
    }

    pub fn run(&mut self, max_steps: usize) -> Result<(), String> {
        while !self.halted {
            if self.steps >= max_steps {
                return Err(format!(
                    "step limit ({}) exceeded at ip=0x{:04x}, sp=0x{:04x}",
                    max_steps, self.ip, self.sp
                ));
            }
            self.step()?;
            self.steps += 1;
        }
        Ok(())
    }

    pub fn run_default(&mut self) -> Result<(), String> {
        self.run(DEFAULT_STEP_LIMIT)
    }

    // Per spec: "Addresses wrap modulo 65536." Byte indices mask with 0xFFFF
    // so a 4-byte read or write starting near the top of memory wraps cleanly
    // into the low addresses rather than panicking on out-of-bounds.
    pub fn read_u32(&self, addr: u32) -> u32 {
        let b = |off: u32| self.mem[((addr.wrapping_add(off)) & 0xFFFF) as usize];
        u32::from_le_bytes([b(0), b(1), b(2), b(3)])
    }

    pub fn read_i32(&self, addr: u32) -> i32 {
        self.read_u32(addr) as i32
    }

    pub fn write_u32(&mut self, addr: u32, v: u32) {
        let b = v.to_le_bytes();
        for i in 0..4u32 {
            self.mem[(addr.wrapping_add(i) & 0xFFFF) as usize] = b[i as usize];
        }
    }

    fn fetch_u8(&mut self) -> u8 {
        let b = self.mem[self.ip as usize];
        self.ip += 1;
        b
    }

    fn fetch_u32(&mut self) -> u32 {
        let v = self.read_u32(self.ip);
        self.ip += 4;
        v
    }

    fn push(&mut self, v: u32) {
        self.write_u32(self.sp, v);
        self.sp += 4;
    }

    fn pop(&mut self) -> u32 {
        if self.sp < STACK_BASE + 4 {
            panic!("stack underflow at ip=0x{:04x}", self.ip);
        }
        self.sp -= 4;
        self.read_u32(self.sp)
    }

    fn step(&mut self) -> Result<(), String> {
        let opcode_ip = self.ip;
        let opcode = self.fetch_u8();
        match opcode {
            op::HALT => self.halted = true,
            op::PUSH => {
                let v = self.fetch_u32();
                self.push(v);
            }
            op::PUSH_ZERO => self.push(0),
            op::PUSH_SHORT => {
                let v = self.fetch_u8() as i8 as i32 as u32;
                self.push(v);
            }
            op::POP => { self.pop(); }
            op::LOAD_SP_REL => {
                // Pop offset (as i32, byte offset), then read mem[SP + offset]
                // — SP is the post-pop SP, matching the convention used by the
                // wander.asm "function return" idiom.
                let off = self.pop() as i32;
                let addr = (self.sp as i64 + off as i64) as u32;
                let v = self.read_u32(addr);
                self.push(v);
            }
            op::STORE_SP_REL => {
                let off = self.pop() as i32;
                let val = self.pop();
                let addr = (self.sp as i64 + off as i64) as u32;
                self.write_u32(addr, val);
            }
            op::LOAD_ABS => {
                let addr = self.pop();
                let v = self.read_u32(addr);
                self.push(v);
            }
            op::STORE_ABS => {
                let addr = self.pop();
                let val = self.pop();
                self.write_u32(addr, val);
            }
            op::JUMP_REL => {
                let off = self.pop() as i32;
                self.ip = (self.ip as i64 + off as i64) as u32;
            }
            op::JUMP_ABS => {
                let addr = self.pop();
                self.ip = addr;
            }
            op::CALL_REL => {
                let off = self.pop() as i32;
                let ret = self.ip;
                self.push(ret);
                self.ip = (self.ip as i64 + off as i64) as u32;
            }
            op::CALL_ABS => {
                let addr = self.pop();
                let ret = self.ip;
                self.push(ret);
                self.ip = addr;
            }
            op::SKIP => {
                // pop cond; if non-zero, skip exactly one byte of the next
                // instruction. Matches the `... SKIP JUMP_REL` and `... SKIP
                // JUMP_ABS` idioms observed in examples/wander.asm.
                let cond = self.pop();
                if cond != 0 {
                    self.ip += 1;
                }
            }
            op::SYSCALL => {
                return Err(format!(
                    "SYSCALL at ip=0x{:04x} is not implemented in the test VM",
                    opcode_ip
                ));
            }
            op::LT | op::LE | op::GT | op::GE | op::EQ | op::NE => {
                let left = self.pop() as i32;
                let right = self.pop() as i32;
                let r = match opcode {
                    op::LT => left < right,
                    op::LE => left <= right,
                    op::GT => left > right,
                    op::GE => left >= right,
                    op::EQ => left == right,
                    op::NE => left != right,
                    _ => unreachable!(),
                };
                self.push(if r { 1 } else { 0 });
            }
            op::ADD | op::SUB | op::MUL | op::DIV | op::REM
            | op::AND | op::OR  | op::XOR | op::SHL | op::SHR => {
                let left = self.pop() as i32;
                let right = self.pop() as i32;
                let r: i32 = match opcode {
                    op::ADD => left.wrapping_add(right),
                    op::SUB => left.wrapping_sub(right),
                    op::MUL => left.wrapping_mul(right),
                    // Per spec: "0x23 Div | left / right (integer; returns 0
                    // if right == 0)" and same for 0x24 Rem. The real game VM
                    // pushes 0 silently; matching that here lets test bots
                    // rely on `x 0 /` as a deliberate "produce 0" idiom.
                    op::DIV => {
                        if right == 0 { 0 } else { left.wrapping_div(right) }
                    }
                    op::REM => {
                        if right == 0 { 0 } else { left.wrapping_rem(right) }
                    }
                    op::AND => left & right,
                    op::OR  => left | right,
                    op::XOR => left ^ right,
                    op::SHL => (left as u32).wrapping_shl(right as u32) as i32,
                    op::SHR => (left as u32).wrapping_shr(right as u32) as i32,
                    _ => unreachable!(),
                };
                self.push(r as u32);
            }
            op::LOGICAL_NOT => {
                let v = self.pop();
                self.push(if v == 0 { 1 } else { 0 });
            }
            op::BITWISE_NOT => {
                let v = self.pop();
                self.push(!v);
            }
            op::NEG => {
                let v = self.pop() as i32;
                self.push(v.wrapping_neg() as u32);
            }
            _ => {
                // Spec says unknown opcodes Halt. We surface them as an error
                // in the test VM instead — silent halts make compiler bugs
                // and miscompiled jumps look like clean termination, which
                // would silently pass tests. Real bots get spec behavior;
                // tests get loud failures.
                return Err(format!(
                    "unknown opcode 0x{:02x} at ip=0x{:04x}",
                    opcode, opcode_ip
                ));
            }
        }
        Ok(())
    }

    pub fn stack_depth(&self) -> u32 {
        (self.sp - STACK_BASE) / 4
    }

    pub fn stack_at(&self, from_top_1based: usize) -> i32 {
        let addr = self.sp - (from_top_1based as u32) * 4;
        self.read_i32(addr)
    }
}

// ---------------------------------------------------------------------------
// Compile/run helpers
// ---------------------------------------------------------------------------

// Compile prelude+src and return bytecode + dictionary lookup closure.
pub fn build(src: &str) -> Result<(Vec<u8>, std::collections::HashMap<String, DictEntry>), String> {
    let mut c = Compiler::new();
    c.compile(PRELUDE).map_err(|e| format!("prelude: {}", e))?;
    c.compile(src)?;
    c.finalize()?;
    // Snapshot the few names we care about. Compiler doesn't expose the
    // whole dict, so resolve via dict_get for each requested name later.
    // Easiest: collect the names callers might look up by re-walking the
    // source. But simplest is to return a tiny lookup helper instead.
    // For our needs, just stash addresses for "result variables" by name.
    let mut out = std::collections::HashMap::new();
    for line in src.lines() {
        for tok in line.split_whitespace() {
            if let Some(entry) = c.dict_get(tok) {
                out.insert(tok.to_string(), entry);
            }
        }
    }
    let bytes = c.code().to_vec();
    // need to drop c after extracting; can't both call into_bytes and use after
    let _ = c;
    Ok((bytes, out))
}

// Compile prelude+src, run with default step limit, return the VM so callers
// can inspect memory.
pub fn run_src(src: &str) -> Result<Vm, String> {
    let bytes = livectf_forth::compile_program(src)?;
    let mut vm = Vm::load(&bytes);
    vm.run_default()?;
    Ok(vm)
}

// Compile prelude+src, look up `var` as a `variable`/`constant`, run, and
// return the i32 value stored at that address. Useful pattern:
//
//     variable r
//     : main ... r ! ;
//
// Then `get_var(src, "r")` returns the final value of r.
pub fn get_var(src: &str, var: &str) -> Result<i32, String> {
    let mut c = Compiler::new();
    c.compile(PRELUDE).map_err(|e| format!("prelude: {}", e))?;
    c.compile(src)?;
    c.finalize()?;
    let addr = match c.dict_get(var) {
        Some(DictEntry::Value(a)) => a as u32,
        Some(DictEntry::Word(_)) => return Err(format!("`{}` is a word, not a variable", var)),
        None => return Err(format!("`{}` is not defined", var)),
    };
    let bytes = c.into_bytes();
    let mut vm = Vm::load(&bytes);
    vm.run_default()?;
    Ok(vm.read_i32(addr))
}

// Tightest convenience: wrap `body` in `: main BODY r ! ;` (with a `variable
// r` declared first) and return r's final value. For one-shot expression
// tests like `eval("7 5 -") == 2`.
pub fn eval(body: &str) -> i32 {
    let src = format!("variable r : main {} r ! ;", body);
    get_var(&src, "r").unwrap_or_else(|e| panic!("eval({:?}): {}", body, e))
}

// Like `eval`, but also lets you tack on extra word definitions after main
// (useful for forward-reference tests where the referenced word is defined
// later in the source).
pub fn eval_with_defs(body: &str, defs_after: &str) -> i32 {
    let src = format!("variable r : main {} r ! ; {}", body, defs_after);
    get_var(&src, "r").unwrap_or_else(|e| panic!("eval_with_defs({:?}): {}", body, e))
}

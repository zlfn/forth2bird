//! VM opcode constants. Shared by the compiler (`fthc`) and disassembler
//! (`fthd`); also used by integration tests to assert emitted bytecode shape.

#![allow(dead_code)]

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

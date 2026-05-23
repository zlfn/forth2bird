// LiveCTF Forth-style frontend — CLI wrapper around the library compiler.

use std::env;
use std::fs;
use std::process::ExitCode;

use livectf_forth::{Compiler, PRELUDE};

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

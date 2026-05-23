//! Tokenizer and number parser. Both are pure functions over `&str` — they
//! hold no state and don't touch the dictionary or emit bytecode. The
//! compiler calls them up front and then walks the resulting token list.

/// Whitespace-separated tokens. `\ ...\n` is a line comment; `( ... )` is a
/// block comment (often used for stack-effect notes like `( a b -- c )`).
/// Both kinds of comments may appear anywhere whitespace is legal and span
/// to the matching closing delimiter or EOF.
pub fn tokenize(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut it = src.chars().peekable();
    while let Some(&c) = it.peek() {
        if c.is_whitespace() {
            it.next();
            continue;
        }
        if c == '\\' {
            // Line comment: consume to newline (or EOF).
            for c in it.by_ref() {
                if c == '\n' { break; }
            }
            continue;
        }
        if c == '(' {
            // Block comment: consume to ')' (or EOF).
            it.next();
            for c in it.by_ref() {
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

/// Parse a numeric literal. Returns `None` for non-numbers so the caller can
/// fall through to identifier handling. Accepts decimal, `0x` hex, and
/// `-0x` hex. Returns i64 (truncated to i32 at emission time).
pub fn parse_number(tok: &str) -> Option<i64> {
    if let Some(rest) = tok.strip_prefix("0x") {
        return i64::from_str_radix(rest, 16).ok();
    }
    if let Some(rest) = tok.strip_prefix("-0x") {
        return i64::from_str_radix(rest, 16).ok().map(|n| -n);
    }
    tok.parse::<i64>().ok()
}

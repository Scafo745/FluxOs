//! A small structural checker for Nix source: balanced brackets, terminated
//! strings and comments, valid interpolations. It catches renderer bugs on
//! targets without Nix; real evaluation happens via `nixos-rebuild`.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub line: usize,
    pub message: String,
}

pub fn check(src: &str) -> Result<(), SyntaxError> {
    let b = src.as_bytes();
    let mut i = 0usize;
    let mut line = 1usize;
    // Bracket stack; `$` marks an interpolation opened inside a string, with
    // the kind of string to resume afterwards.
    #[derive(Clone, Copy, PartialEq)]
    enum Frame {
        Brace,
        Bracket,
        Paren,
        InterpDq,
        InterpInd,
    }
    let mut stack: Vec<(Frame, usize)> = Vec::new();
    let err = |line: usize, m: &str| Err(SyntaxError { line, message: m.to_string() });

    // Scans a string body starting at `i`; returns the new index and whether
    // an interpolation `${` was entered (string resumes after the `}`).
    enum StrEnd {
        Closed(usize),
        Interp(usize),
    }

    let scan_dq = |mut i: usize, line: &mut usize| -> Result<StrEnd, SyntaxError> {
        while i < b.len() {
            match b[i] {
                b'\n' => *line += 1,
                b'\\' => {
                    i += 2;
                    continue;
                }
                b'"' => return Ok(StrEnd::Closed(i + 1)),
                b'$' if i + 1 < b.len() && b[i + 1] == b'{' => return Ok(StrEnd::Interp(i + 2)),
                _ => {}
            }
            i += 1;
        }
        Err(SyntaxError { line: *line, message: "unterminated \"string\"".into() })
    };

    let scan_ind = |mut i: usize, line: &mut usize| -> Result<StrEnd, SyntaxError> {
        while i < b.len() {
            match b[i] {
                b'\n' => *line += 1,
                b'\'' if i + 1 < b.len() && b[i + 1] == b'\'' => {
                    // Escapes: ''' , ''$ , ''\x
                    if i + 2 < b.len() && (b[i + 2] == b'\'' || b[i + 2] == b'$') {
                        i += 3;
                        continue;
                    }
                    if i + 2 < b.len() && b[i + 2] == b'\\' {
                        i += 4;
                        continue;
                    }
                    return Ok(StrEnd::Closed(i + 2));
                }
                b'$' if i + 1 < b.len() && b[i + 1] == b'{' => return Ok(StrEnd::Interp(i + 2)),
                _ => {}
            }
            i += 1;
        }
        Err(SyntaxError { line: *line, message: "unterminated ''indented string''".into() })
    };

    while i < b.len() {
        let c = b[i];
        match c {
            b'\n' => {
                line += 1;
                i += 1;
            }
            b'#' => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < b.len() && b[i + 1] == b'*' => {
                let start = line;
                i += 2;
                loop {
                    if i + 1 >= b.len() {
                        return err(start, "unterminated /* comment */");
                    }
                    if b[i] == b'\n' {
                        line += 1;
                    }
                    if b[i] == b'*' && b[i + 1] == b'/' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
            }
            b'"' => match scan_dq(i + 1, &mut line)? {
                StrEnd::Closed(n) => i = n,
                StrEnd::Interp(n) => {
                    stack.push((Frame::InterpDq, line));
                    i = n;
                }
            },
            b'\'' if i + 1 < b.len() && b[i + 1] == b'\'' => match scan_ind(i + 2, &mut line)? {
                StrEnd::Closed(n) => i = n,
                StrEnd::Interp(n) => {
                    stack.push((Frame::InterpInd, line));
                    i = n;
                }
            },
            b'{' | b'[' | b'(' => {
                stack.push((
                    match c {
                        b'{' => Frame::Brace,
                        b'[' => Frame::Bracket,
                        _ => Frame::Paren,
                    },
                    line,
                ));
                i += 1;
            }
            b'}' | b']' | b')' => {
                let Some((top, _)) = stack.pop() else {
                    return err(line, &format!("unexpected '{}'", c as char));
                };
                match (top, c) {
                    (Frame::Brace, b'}') | (Frame::Bracket, b']') | (Frame::Paren, b')') => i += 1,
                    (Frame::InterpDq, b'}') => match scan_dq(i + 1, &mut line)? {
                        StrEnd::Closed(n) => i = n,
                        StrEnd::Interp(n) => {
                            stack.push((Frame::InterpDq, line));
                            i = n;
                        }
                    },
                    (Frame::InterpInd, b'}') => match scan_ind(i + 1, &mut line)? {
                        StrEnd::Closed(n) => i = n,
                        StrEnd::Interp(n) => {
                            stack.push((Frame::InterpInd, line));
                            i = n;
                        }
                    },
                    _ => return err(line, &format!("mismatched '{}'", c as char)),
                }
            }
            _ => i += 1,
        }
    }
    if let Some((_, open_line)) = stack.last() {
        return err(*open_line, "unclosed bracket or interpolation");
    }
    Ok(())
}

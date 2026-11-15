//! From text to statements: one statement per line, with indented lines and open brackets
//! continuing the statement above.

use crate::num::Rat;
use crate::value::Sym;
use std::fmt;

#[derive(Clone, PartialEq, Debug)]
pub enum Tok {
    Ident(Sym),
    Num(Rat),
    Text(Sym),
    LBracket,
    RBracket,
    LParen,
    RParen,
    /// `|`, or a new line inside brackets.
    Sep,
    Colon,
    Dot,
    Comma,
    Question,
    Plus,
    Minus,
    /// A `-` written directly before a digit and not directly after a value, as in `age -2`.
    TightMinus,
    Star,
    Slash,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

impl fmt::Display for Tok {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let symbol = match self {
            Tok::Ident(name) => return write!(f, "{name}"),
            Tok::Num(n) => return write!(f, "{n}"),
            Tok::Text(t) => return write!(f, "{t:?}"),
            Tok::LBracket => "[",
            Tok::RBracket => "]",
            Tok::LParen => "(",
            Tok::RParen => ")",
            Tok::Sep => "|",
            Tok::Colon => ":",
            Tok::Dot => ".",
            Tok::Comma => ",",
            Tok::Question => "?",
            Tok::Plus => "+",
            Tok::Minus | Tok::TightMinus => "-",
            Tok::Star => "*",
            Tok::Slash => "/",
            Tok::Eq => "=",
            Tok::Ne => "!=",
            Tok::Lt => "<",
            Tok::Gt => ">",
            Tok::Le => "<=",
            Tok::Ge => ">=",
        };
        f.write_str(symbol)
    }
}

/// One statement: its tokens, the line it starts on, and its text without comments.
#[derive(Clone, Debug)]
pub struct Line {
    pub tokens: Vec<Tok>,
    pub number: u32,
    pub text: String,
}

#[derive(Debug)]
pub enum LexError {
    /// A bracket opened on this line is never closed. More text could still close it.
    Unclosed(u32),
    Bad(u32, String),
}

pub fn lex(source: &str) -> Result<Vec<Line>, LexError> {
    let mut lines: Vec<Line> = Vec::new();
    let mut open: Vec<(Tok, u32)> = Vec::new();
    for (i, raw) in source.lines().enumerate() {
        let number = i as u32 + 1;
        let (tokens, code) = lex_line(raw).map_err(|m| LexError::Bad(number, m))?;
        if tokens.is_empty() {
            continue;
        }
        let indented = raw.starts_with([' ', '\t']);
        if open.is_empty() && !(indented && !lines.is_empty()) {
            lines.push(Line { tokens: Vec::new(), number, text: String::new() });
        }
        let line = lines.last_mut().unwrap();
        // Inside brackets a new line separates, unless the line above was left unfinished.
        let unfinished = matches!(
            line.tokens.last(),
            Some(Tok::Comma | Tok::Colon | Tok::LBracket | Tok::Sep | Tok::Plus | Tok::Minus | Tok::Star | Tok::Slash)
        );
        if matches!(open.last(), Some((Tok::LBracket, _))) && !unfinished {
            line.tokens.push(Tok::Sep);
        }
        if !line.text.is_empty() {
            line.text.push('\n');
        }
        line.text.push_str(code.trim_end());
        for token in tokens {
            match &token {
                Tok::LBracket | Tok::LParen => open.push((token.clone(), number)),
                Tok::RBracket | Tok::RParen => {
                    let opener = if token == Tok::RBracket { Tok::LBracket } else { Tok::LParen };
                    if open.pop().map(|(t, _)| t) != Some(opener) {
                        return Err(LexError::Bad(number, format!("this `{token}` closes nothing")));
                    }
                }
                _ => {}
            }
            line.tokens.push(token);
        }
    }
    match open.first() {
        Some((_, number)) => Err(LexError::Unclosed(*number)),
        None => Ok(lines),
    }
}

/// The tokens of one physical line, and the line without its comment.
fn lex_line(raw: &str) -> Result<(Vec<Tok>, &str), String> {
    let chars: Vec<(usize, char)> = raw.char_indices().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let (at, c) = chars[i];
        let next = chars.get(i + 1).map(|p| p.1);
        let end_of = |j: usize| chars.get(j).map_or(raw.len(), |p| p.0);
        i += 1;
        let token = match c {
            ' ' | '\t' | '\r' => continue,
            '-' if next == Some('-') => return Ok((tokens, &raw[..at])),
            '-' => {
                let after_value = i >= 2 && {
                    let prev = chars[i - 2].1;
                    prev.is_alphanumeric() || matches!(prev, '_' | ')' | ']' | '"' | '.')
                };
                if next.is_some_and(|n| n.is_ascii_digit()) && !after_value { Tok::TightMinus } else { Tok::Minus }
            }
            '"' => {
                let mut text = String::new();
                loop {
                    let Some(&(_, c)) = chars.get(i) else { return Err("this text never closes its `\"`".into()) };
                    i += 1;
                    match c {
                        '"' => break,
                        '\\' => {
                            let escaped = chars.get(i).map(|p| p.1);
                            i += 1;
                            text.push(match escaped {
                                Some('n') => '\n',
                                Some(c @ ('"' | '\\')) => c,
                                _ => return Err("after `\\` in text, write `\"`, `\\` or `n`".into()),
                            });
                        }
                        c => text.push(c),
                    }
                }
                Tok::Text(text.into())
            }
            c if c.is_ascii_digit() => {
                while chars.get(i).is_some_and(|p| p.1.is_ascii_digit()) {
                    i += 1;
                }
                let fraction = chars.get(i).is_some_and(|p| p.1 == '.') && chars.get(i + 1).is_some_and(|p| p.1.is_ascii_digit());
                if fraction {
                    i += 1;
                    while chars.get(i).is_some_and(|p| p.1.is_ascii_digit()) {
                        i += 1;
                    }
                }
                let literal = &raw[at..end_of(i)];
                Tok::Num(Rat::parse(literal).ok_or_else(|| format!("{literal} is too large a number"))?)
            }
            c if c.is_alphabetic() || c == '_' => {
                while chars.get(i).is_some_and(|p| p.1.is_alphanumeric() || p.1 == '_') {
                    i += 1;
                }
                Tok::Ident(raw[at..end_of(i)].into())
            }
            '!' | '<' | '>' if next == Some('=') => {
                i += 1;
                match c {
                    '!' => Tok::Ne,
                    '<' => Tok::Le,
                    _ => Tok::Ge,
                }
            }
            '[' => Tok::LBracket,
            ']' => Tok::RBracket,
            '(' => Tok::LParen,
            ')' => Tok::RParen,
            '|' => Tok::Sep,
            ':' => Tok::Colon,
            '.' => Tok::Dot,
            ',' => Tok::Comma,
            '?' => Tok::Question,
            '+' => Tok::Plus,
            '*' => Tok::Star,
            '/' => Tok::Slash,
            '=' => Tok::Eq,
            '<' => Tok::Lt,
            '>' => Tok::Gt,
            other => return Err(format!("`{other}` means nothing in Grant")),
        };
        tokens.push(token);
    }
    Ok((tokens, raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_continue_on_indented_lines_and_in_brackets() {
        let lines =
            lex("kind person [\n  age: one number -- years\n  knows: person\n]\nrule a: person p is b if\n  p.age >= 18\n\nperson liz")
                .unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].tokens.iter().filter(|t| **t == Tok::Sep).count(), 2);
        assert_eq!(lines[1].number, 5);
        assert!(lines[1].text.ends_with("p.age >= 18"));
    }

    #[test]
    fn minus_is_tight_only_before_a_literal() {
        let kinds =
            |s: &str| lex(s).unwrap()[0].tokens.iter().filter(|t| matches!(t, Tok::Minus | Tok::TightMinus)).cloned().collect::<Vec<_>>();
        assert_eq!(kinds("x age -2"), [Tok::TightMinus]);
        assert_eq!(kinds("? x.age - 2"), [Tok::Minus]);
        assert_eq!(kinds("? x.age-2"), [Tok::Minus]);
    }

    #[test]
    fn unclosed_brackets_ask_for_more() {
        assert!(matches!(lex("person liz [ age: 3"), Err(LexError::Unclosed(1))));
        assert!(matches!(lex("person liz ]"), Err(LexError::Bad(1, _))));
    }
}

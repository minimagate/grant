//! From text to statements: one statement per line, with indented lines and open brackets
//! continuing the statement above.

use crate::num::Rat;
use crate::value::Sym;
use std::fmt;

/// The words that are never names.
const KEYWORDS: [&str; 19] = [
    "from", "kind", "rule", "never", "if", "where", "and", "or", "not", "the", "each", "in", "count", "sum", "min", "max", "div", "mod",
    "id",
];

/// Punctuation, with the two-character marks first.
const MARKS: [&str; 20] = ["!=", "<=", ">=", "[", "]", "(", ")", "|", ":", ".", "@", ",", "?", "+", "-", "*", "/", "=", "<", ">"];

#[derive(Clone, PartialEq, Debug)]
pub enum Tok {
    Name(Sym),
    Num(Rat),
    Text(Sym),
    /// A keyword or a punctuation mark, as written. `|` is also a new line inside brackets.
    Sign(&'static str),
    /// A `-` written directly before a digit and not directly after a value, as in `age -2`.
    TightMinus,
}

impl fmt::Display for Tok {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Tok::Name(name) => write!(f, "{name}"),
            Tok::Num(n) => write!(f, "{n}"),
            Tok::Text(t) => write!(f, "{t:?}"),
            Tok::Sign(sign) => f.write_str(sign),
            Tok::TightMinus => f.write_str("-"),
        }
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
    let mut open: Vec<(&str, u32)> = Vec::new();
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
        let unfinished = matches!(line.tokens.last(), Some(Tok::Sign("," | ":" | "[" | "|" | "+" | "-" | "*" | "/")));
        if matches!(open.last(), Some(("[", _))) && !unfinished {
            line.tokens.push(Tok::Sign("|"));
        }
        if !line.text.is_empty() {
            line.text.push('\n');
        }
        line.text.push_str(code.trim_end());
        for token in tokens {
            match token {
                Tok::Sign(opener @ ("[" | "(")) => open.push((opener, number)),
                Tok::Sign(closer @ ("]" | ")")) => {
                    let opener = if closer == "]" { "[" } else { "(" };
                    if open.pop().map(|(o, _)| o) != Some(opener) {
                        return Err(LexError::Bad(number, format!("this `{closer}` closes nothing")));
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
    // How much of the start of a text is made of these characters.
    let run = |text: &str, of: fn(char) -> bool| text.find(|c| !of(c)).unwrap_or(text.len());
    let mut tokens = Vec::new();
    let mut at = 0;
    while let Some(c) = raw[at..].chars().next() {
        let rest = &raw[at..];
        let (token, len) = match c {
            ' ' | '\t' | '\r' => {
                at += 1;
                continue;
            }
            '-' if rest.starts_with("--") => return Ok((tokens, &raw[..at])),
            '-' if rest[1..].starts_with(|c: char| c.is_ascii_digit())
                && !raw[..at].ends_with(|c: char| c.is_alphanumeric() || matches!(c, '_' | ')' | ']' | '"' | '.')) =>
            {
                (Tok::TightMinus, 1)
            }
            '"' => {
                let mut text = String::new();
                let mut chars = rest[1..].char_indices();
                loop {
                    match chars.next() {
                        None => return Err("this text never closes its `\"`".into()),
                        Some((i, '"')) => break (Tok::Text(text.into()), i + 2),
                        Some((_, '\\')) => text.push(match chars.next() {
                            Some((_, 'n')) => '\n',
                            Some((_, c @ ('"' | '\\'))) => c,
                            _ => return Err("after `\\` in text, write `\"`, `\\` or `n`".into()),
                        }),
                        Some((_, c)) => text.push(c),
                    }
                }
            }
            _ if c.is_ascii_digit() => {
                let mut len = run(rest, |c| c.is_ascii_digit());
                if let Some(fraction) = rest[len..].strip_prefix('.')
                    && fraction.starts_with(|c: char| c.is_ascii_digit())
                {
                    len += 1 + run(fraction, |c| c.is_ascii_digit());
                }
                let literal = &rest[..len];
                (Tok::Num(Rat::parse(literal).ok_or_else(|| format!("{literal} is too large a number"))?), len)
            }
            _ if c.is_alphabetic() || c == '_' => {
                let word = &rest[..run(rest, |c| c.is_alphanumeric() || c == '_')];
                (KEYWORDS.iter().find(|k| **k == word).map_or_else(|| Tok::Name(word.into()), |k| Tok::Sign(k)), word.len())
            }
            _ => match MARKS.iter().find(|mark| rest.starts_with(**mark)) {
                Some(mark) => (Tok::Sign(mark), mark.len()),
                None => return Err(format!("`{c}` means nothing in Grant")),
            },
        };
        tokens.push(token);
        at += len;
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
        assert_eq!(lines[0].tokens.iter().filter(|t| **t == Tok::Sign("|")).count(), 2);
        assert_eq!(lines[1].number, 5);
        assert!(lines[1].text.ends_with("p.age >= 18"));
    }

    #[test]
    fn minus_is_tight_only_before_a_literal() {
        let minuses = |s: &str| {
            lex(s).unwrap()[0].tokens.iter().filter(|t| matches!(t, Tok::Sign("-") | Tok::TightMinus)).cloned().collect::<Vec<_>>()
        };
        assert_eq!(minuses("x age -2"), [Tok::TightMinus]);
        assert_eq!(minuses("? x.age - 2"), [Tok::Sign("-")]);
        assert_eq!(minuses("? x.age-2"), [Tok::Sign("-")]);
    }

    #[test]
    fn unclosed_brackets_ask_for_more() {
        assert!(matches!(lex("person liz [ age: 3"), Err(LexError::Unclosed(1))));
        assert!(matches!(lex("person liz ]"), Err(LexError::Bad(1, _))));
    }
}

//! Errors: what Grant says, and where.

use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;

/// A result whose error is the sentence Grant says; the line it belongs to is added later.
pub type Res<T> = Result<T, String>;

/// An error placed in a world: on one of its lines, or at its end.
#[derive(Debug, Clone)]
pub struct Error {
    pub world: String,
    pub file: Option<PathBuf>,
    /// The line of the statement, or `None` for what is checked when the world ends.
    pub line: Option<u32>,
    pub source: Option<String>,
    pub messages: Vec<String>,
    /// The parent's error, when this one only reports that a parent has errors.
    pub cause: Option<Rc<Error>>,
}

/// Why a statement failed: a sentence, or a parent world's own error.
pub(crate) enum Fail {
    Message(String),
    Parent(String, Rc<Error>),
}

impl From<String> for Fail {
    fn from(message: String) -> Fail {
        Fail::Message(message)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let file = self.file.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy());
        match (self.line, self.messages.as_slice()) {
            (Some(line), [message]) => {
                match &file {
                    Some(file) => write!(f, "error in {} ({file}, line {line})", self.world)?,
                    None => write!(f, "error in {} (line {line})", self.world)?,
                }
                write!(f, ": {message}")?;
                for (i, text) in self.source.iter().flat_map(|s| s.lines()).enumerate() {
                    write!(f, "\n{:>5} | {text}", line as usize + i)?;
                }
            }
            (_, messages) => {
                write!(f, "error at the end of {}", self.world)?;
                if let Some(file) = &file {
                    write!(f, " ({file})")?;
                }
                write!(f, ":")?;
                for message in messages {
                    write!(f, "\n  - {message}")?;
                }
            }
        }
        if let Some(cause) = &self.cause {
            write!(f, "\n{cause}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

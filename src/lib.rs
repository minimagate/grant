//! Grant: a language for describing worlds and changing them.
//!
//! A world is a finite relational structure, W = (K, O, F, R): kinds, objects, facts and rules.
//! This crate runs worlds written in `.grant` files, following the Foundation 1 specification
//! in `docs/foundation-1.md`.
//!
//! ```
//! use grant::{Loader, Session, lex};
//!
//! let mut loader = Loader::new();
//! let dir = std::env::temp_dir();
//! let mut world = Session::new("example", &dir, None);
//! let source = "from numbers\nkind person [ age: one number ]\nperson harbor [ age: 24 ]\n? harbor.age + 1";
//! let mut shown = Vec::new();
//! for line in lex(source).unwrap() {
//!     shown.extend(world.run(&mut loader, &line).unwrap());
//! }
//! assert_eq!(shown[0].text, "25");
//! world.finish().unwrap();
//! ```

mod ast;
mod compile;
mod equal;
mod error;
mod eval;
mod exec;
mod lexer;
mod loader;
mod logic;
mod model;
mod num;
mod parser;
pub mod show;
mod value;
mod world;

pub use error::Error;
pub use exec::{Output, Session};
pub use lexer::{LexError, Line, lex};
pub use loader::Loader;
pub use model::check;
pub use world::World;

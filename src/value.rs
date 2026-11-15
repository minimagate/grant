//! The values a fact can hold, and the words used to talk about them.

use crate::num::Rat;
use std::rc::Rc;

/// A name: a kind, relation, handle, variable, rule or world.
pub type Sym = Rc<str>;

/// An object's identity: the world that created it and its place in that world's order of
/// creation. A held world is its own origin and takes place 0.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ObjId {
    pub world: u32,
    pub seq: u32,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Value {
    Num(Rat),
    Text(Sym),
    Obj(ObjId),
}

/// The kinds that come from primitive worlds rather than from statements.
pub const PRIMITIVES: [(&str, &str); 3] = [("number", "numbers"), ("text", "text"), ("id", "id")];

pub fn is_primitive(kind: &str) -> bool {
    PRIMITIVES.iter().any(|(k, _)| *k == kind)
}

pub fn plural(noun: &str) -> String {
    if noun == "person" {
        return "people".into();
    }
    if let Some(stem) = noun.strip_suffix('y')
        && !stem.ends_with(['a', 'e', 'i', 'o', 'u'])
    {
        return format!("{stem}ies");
    }
    if noun.ends_with(['s', 'x']) || noun.ends_with("ch") || noun.ends_with("sh") {
        return format!("{noun}es");
    }
    format!("{noun}s")
}

/// `a city`, `an animal`.
pub fn a(noun: &str) -> String {
    let article = if noun.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
    format!("{article} {noun}")
}

/// `1 city`, `3 people`.
pub fn some(n: usize, noun: &str) -> String {
    if n == 1 { format!("1 {noun}") } else { format!("{n} {}", plural(noun)) }
}

//! Statements as written, before names are given their meaning.

use crate::num::Rat;
use crate::value::Sym;
use std::fmt;

/// An operator, as written: `+`, `div`, `<=`, `sum`.
pub type Op = &'static str;

#[derive(Clone, Debug)]
pub enum Expr {
    Num(Rat),
    Text(Sym),
    /// A handle or a variable; which one depends on what is in scope.
    Name(Sym),
    The(Box<The>),
    /// `id.3`: every object numbered 3.
    Index(u64),
    Dot(Box<Expr>, Sym),
    Neg(Box<Expr>),
    Bin(Op, Box<Expr>, Box<Expr>),
    /// `count`, `sum`, `min` or `max` of a value.
    Agg(Op, Box<Expr>),
    /// `count ( condition )`: how many ways it holds.
    Count(Box<Cond>),
    /// `in w: expr`
    In(Box<Expr>, Box<Expr>),
}

/// `the [kind] handle [ description ]`
#[derive(Clone, Debug)]
pub struct The {
    pub kind: Option<Sym>,
    pub handle: Sym,
    pub desc: Vec<(Sym, Vec<Expr>)>,
}

/// One side of a fact pattern.
#[derive(Clone, Debug)]
pub enum PTerm {
    /// `person p`: a variable introduced with its kind.
    Intro(Sym, Sym),
    Expr(Expr),
}

#[derive(Clone, Debug)]
pub enum Cond {
    And(Vec<Cond>),
    Or(Vec<Cond>),
    Not(Box<Cond>),
    In(Expr, Box<Cond>),
    /// `person p`: any object of a kind.
    Any(Sym, Sym),
    Pattern {
        subj: PTerm,
        rel: Sym,
        val: PTerm,
    },
    /// Four names in a row read two ways: `person p knows liz` or `liz knows person p`.
    /// Which one is meant depends on whether the first name is a kind where the line runs.
    Either {
        name: Sym,
        as_kind: Box<Cond>,
        as_term: Box<Cond>,
    },
    Cmp(Op, Expr, Expr),
    /// A bare value. Only a whole question may be one.
    Value(Expr),
}

/// What a bracket entry does with its values.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Change {
    /// `r: a, b` — these and no others.
    Set,
    /// `r: + a`
    Add,
    /// `r: - a`
    Remove,
}

/// One bracket entry, with its values as written or as they later become. `r: + a, - b` is two.
#[derive(Clone, Debug)]
pub struct Assign<V = Expr> {
    pub rel: Sym,
    pub change: Change,
    pub values: Vec<V>,
}

/// How many values a relation holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum How {
    One,
    Maybe,
    Some,
    Many,
}

/// The right-hand side of a shape entry: how many values, and of what kind.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Field {
    pub how: How,
    pub kind: Sym,
    /// `person: person by name`: read from the other end of `name`, never stated.
    pub by: Option<Sym>,
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let word = match self.how {
            How::One => "one ",
            How::Maybe => "maybe ",
            How::Some => "some ",
            How::Many => "",
        };
        write!(f, "{word}{}", self.kind)?;
        match &self.by {
            Some(by) => write!(f, " by {by}"),
            None => Ok(()),
        }
    }
}

#[derive(Clone, Debug)]
pub enum ShapeEdit {
    /// `r: field` — states the relation; it must not contradict the shape.
    Declare(Sym, Field),
    /// `+ r: field` — declares the relation or sets how many and its value kind.
    Set(Sym, Field),
    /// `- r`
    Remove(Sym),
}

#[derive(Clone, Debug)]
pub enum EachAction {
    Set(Vec<Assign>),
    Fact(Sym, Expr),
    RemoveObjects,
    RemoveRel(Sym, Option<Expr>),
}

/// What a rule produces: `kind var rel val`.
#[derive(Clone, Debug)]
pub struct Head {
    pub kind: Sym,
    pub var: Sym,
    pub rel: Sym,
    pub val: PTerm,
}

#[derive(Clone, Debug)]
pub enum Stmt {
    From(Vec<String>),
    Kind {
        name: Sym,
        edits: Vec<ShapeEdit>,
    },
    RemoveKind(Sym),
    /// `path` is set for `world`, whose objects are other worlds.
    Create {
        kind: Sym,
        handle: Sym,
        path: Option<String>,
        values: Vec<Assign>,
    },
    Fact {
        subj: Expr,
        rel: Sym,
        value: Expr,
    },
    Set {
        subj: Expr,
        values: Vec<Assign>,
    },
    Remove {
        subj: Expr,
        rel: Option<Sym>,
        value: Option<Expr>,
    },
    Each {
        kind: Sym,
        var: Sym,
        cond: Cond,
        action: EachAction,
    },
    Rule {
        name: Sym,
        head: Head,
        cond: Cond,
    },
    Never {
        name: Sym,
        kind: Sym,
        var: Sym,
        cond: Cond,
    },
    /// `- rule name` or `- never name`
    Forget(&'static str, Sym),
    Ask(Cond),
    /// A line that tries to change a held world from outside.
    InChange(Sym),
}

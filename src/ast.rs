//! Statements as written, before names are given their meaning.

use crate::num::Rat;
use crate::value::Sym;
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    IntDiv,
    Mod,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AggOp {
    Sum,
    Min,
    Max,
}

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::IntDiv => "div",
            BinOp::Mod => "mod",
        })
    }
}

impl fmt::Display for CmpOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            CmpOp::Eq => "=",
            CmpOp::Ne => "!=",
            CmpOp::Lt => "<",
            CmpOp::Gt => ">",
            CmpOp::Le => "<=",
            CmpOp::Ge => ">=",
        })
    }
}

impl fmt::Display for AggOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            AggOp::Sum => "sum",
            AggOp::Min => "min",
            AggOp::Max => "max",
        })
    }
}

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
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Count(Box<Expr>),
    CountCond(Box<Cond>),
    Agg(AggOp, Box<Expr>),
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
    Cmp(CmpOp, Expr, Expr),
    /// A bare value. Only a whole question may be one.
    Value(Expr),
}

#[derive(Clone, Debug)]
pub struct Assign {
    pub rel: Sym,
    pub values: Values,
}

#[derive(Clone, Debug)]
pub enum Values {
    /// `r: a, b` — these and no others.
    Set(Vec<Expr>),
    /// `r: + a, - b` — `true` adds, `false` removes.
    Change(Vec<(bool, Expr)>),
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
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let word = match self.how {
            How::One => "one ",
            How::Maybe => "maybe ",
            How::Some => "some ",
            How::Many => "",
        };
        write!(f, "{word}{}", self.kind)
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
        cond: Option<Cond>,
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
    RemoveRule(Sym),
    RemoveNever(Sym),
    Ask(Cond),
    /// A line that tries to change a held world from outside.
    InChange(Sym),
}

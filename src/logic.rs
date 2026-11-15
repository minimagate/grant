//! Conditions and values with their names resolved: variables are numbered slots, and handles
//! are the objects they pointed at when the line was stated.

use crate::ast::Op;
use crate::value::{ObjId, Sym, Value};

#[derive(Clone, PartialEq, Debug)]
pub struct Var {
    pub name: Sym,
    pub kind: Sym,
    /// Introduced inside `in w:`, so its kind is one of w's kinds.
    pub foreign: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub enum CExpr {
    /// A number or a text.
    Const(Value),
    /// An object the statement names, by its place among those it names.
    Obj(usize),
    Var(usize),
    /// A handle inside `in w:`, looked up in whichever world is being looked into.
    Handle(Sym),
    The(Box<CThe>),
    Index(u64),
    Dot(Box<CExpr>, Sym),
    /// A `by` relation followed: every object of the kind (the second name) whose relation (the
    /// first) holds the value.
    Back(Box<CExpr>, Sym, Sym),
    Neg(Box<CExpr>),
    Bin(Op, Box<CExpr>, Box<CExpr>),
    Agg(Op, Box<CExpr>),
    Count(Box<Scope>),
    In(Box<CExpr>, Box<CExpr>),
}

#[derive(Clone, PartialEq, Debug)]
pub struct CThe {
    pub kind: Option<Sym>,
    pub handle: Sym,
    pub desc: Vec<(Sym, Vec<CExpr>)>,
}

/// A condition with variables of its own: the inside of `not` or of `count ( … )`.
#[derive(Clone, PartialEq, Debug)]
pub struct Scope {
    /// Variables introduced inside, which mean "there is some".
    pub locals: Vec<usize>,
    /// Outer variables used inside, which need values before the scope can be judged.
    pub needs: Vec<usize>,
    pub cond: CCond,
}

#[derive(Clone, PartialEq, Debug)]
pub enum CCond {
    And(Vec<CCond>),
    Or(Vec<CCond>),
    Not(Box<Scope>),
    In(CExpr, Box<CCond>),
    Any(usize),
    /// `needs` are the variables inside computed terms; a bare variable is bound by matching.
    Pattern {
        subj: CExpr,
        rel: Sym,
        val: CExpr,
        needs: Vec<usize>,
    },
    Cmp {
        op: Op,
        left: CExpr,
        right: CExpr,
        needs: Vec<usize>,
    },
}

/// A rule holds the objects it names, not their names: `objects` are what `CExpr::Obj` points at.
#[derive(Clone, Debug)]
pub struct Rule {
    pub name: Sym,
    pub source: String,
    pub vars: Vec<Var>,
    pub objects: Vec<ObjId>,
    pub subj: usize,
    pub rel: Sym,
    pub val: CExpr,
    pub cond: CCond,
}

#[derive(Clone, Debug)]
pub struct Never {
    pub name: Sym,
    pub source: String,
    pub vars: Vec<Var>,
    pub objects: Vec<ObjId>,
    pub subj: usize,
    pub cond: CCond,
}

impl Rule {
    /// What the rule says, whichever objects it names and however it was spaced or commented.
    pub fn shape(&self) -> impl PartialEq + '_ {
        (&self.vars, self.subj, &self.rel, &self.val, &self.cond)
    }
}

impl Never {
    pub fn shape(&self) -> impl PartialEq + '_ {
        (&self.vars, self.subj, &self.cond)
    }
}

impl PartialEq for Rule {
    fn eq(&self, o: &Rule) -> bool {
        self.shape() == o.shape() && self.objects == o.objects
    }
}

impl PartialEq for Never {
    fn eq(&self, o: &Never) -> bool {
        self.shape() == o.shape() && self.objects == o.objects
    }
}

/// Every variable a value mentions, except those local to a scope inside it.
pub fn expr_slots(e: &CExpr, out: &mut Vec<usize>) {
    match e {
        CExpr::Const(_) | CExpr::Obj(_) | CExpr::Handle(_) | CExpr::Index(_) => {}
        CExpr::Var(i) => out.push(*i),
        CExpr::The(the) => the.desc.iter().flat_map(|(_, es)| es).for_each(|e| expr_slots(e, out)),
        CExpr::Dot(e, _) | CExpr::Back(e, ..) | CExpr::Neg(e) | CExpr::Agg(_, e) => expr_slots(e, out),
        CExpr::Bin(_, l, r) | CExpr::In(l, r) => {
            expr_slots(l, out);
            expr_slots(r, out);
        }
        CExpr::Count(scope) => out.extend(&scope.needs),
    }
}

/// Every variable a condition mentions, except those local to a scope inside it.
pub fn cond_slots(c: &CCond, out: &mut Vec<usize>) {
    match c {
        CCond::And(cs) | CCond::Or(cs) => cs.iter().for_each(|c| cond_slots(c, out)),
        CCond::Not(scope) => out.extend(&scope.needs),
        CCond::In(world, body) => {
            expr_slots(world, out);
            cond_slots(body, out);
        }
        CCond::Any(i) => out.push(*i),
        CCond::Pattern { subj: l, val: r, .. } | CCond::Cmp { left: l, right: r, .. } => {
            expr_slots(l, out);
            expr_slots(r, out);
        }
    }
}

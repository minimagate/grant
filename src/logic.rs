//! Conditions and values with their names resolved: variables are numbered slots, and handles
//! are the objects they pointed at when the line was stated.

use crate::ast::{AggOp, BinOp, CmpOp};
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
    Const(Value),
    Var(usize),
    /// A handle inside `in w:`, looked up in whichever world is being looked into.
    Handle(Sym),
    The(Box<CThe>),
    Index(u64),
    Dot(Box<CExpr>, Sym),
    Neg(Box<CExpr>),
    Bin(BinOp, Box<CExpr>, Box<CExpr>),
    Count(Box<CExpr>),
    CountCond(Box<Scope>),
    Agg(AggOp, Box<CExpr>),
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
        op: CmpOp,
        left: CExpr,
        right: CExpr,
        needs: Vec<usize>,
    },
}

#[derive(Clone, Debug)]
pub struct Rule {
    pub name: Sym,
    pub source: String,
    pub vars: Vec<Var>,
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
    pub subj: usize,
    pub cond: CCond,
}

/// Rules are the same when they mean the same, however they were spaced or commented.
impl PartialEq for Rule {
    fn eq(&self, o: &Rule) -> bool {
        (&self.vars, self.subj, &self.rel, &self.val, &self.cond) == (&o.vars, o.subj, &o.rel, &o.val, &o.cond)
    }
}

impl PartialEq for Never {
    fn eq(&self, o: &Never) -> bool {
        (&self.vars, self.subj, &self.cond) == (&o.vars, o.subj, &o.cond)
    }
}

impl Rule {
    /// The variables the head needs values for.
    pub fn head_slots(&self) -> Vec<usize> {
        let mut slots = vec![self.subj];
        expr_slots(&self.val, &mut slots);
        slots
    }

    /// The rule with each object it names replaced by a numbered placeholder, and those objects
    /// in order. Two rules have the same shape when the first parts are equal.
    pub fn abstracted(&self) -> (Rule, Vec<ObjId>) {
        let mut rule = self.clone();
        let mut objects = Vec::new();
        {
            let mut replace = placeholder(&mut objects);
            expr_consts(&mut rule.val, &mut replace);
            cond_consts(&mut rule.cond, &mut replace);
        }
        (rule, objects)
    }

    pub fn objects(&self) -> Vec<ObjId> {
        self.abstracted().1
    }
}

impl Never {
    pub fn abstracted(&self) -> (Never, Vec<ObjId>) {
        let mut never = self.clone();
        let mut objects = Vec::new();
        cond_consts(&mut never.cond, &mut placeholder(&mut objects));
        (never, objects)
    }

    pub fn objects(&self) -> Vec<ObjId> {
        self.abstracted().1
    }
}

fn placeholder(objects: &mut Vec<ObjId>) -> impl FnMut(&mut Value) + '_ {
    |value| {
        if let Value::Obj(id) = value {
            objects.push(*id);
            *id = ObjId { world: u32::MAX, seq: objects.len() as u32 };
        }
    }
}

/// Every variable a value mentions, except those local to a scope inside it.
pub fn expr_slots(e: &CExpr, out: &mut Vec<usize>) {
    match e {
        CExpr::Const(_) | CExpr::Handle(_) | CExpr::Index(_) => {}
        CExpr::Var(i) => out.push(*i),
        CExpr::The(the) => the.desc.iter().flat_map(|(_, es)| es).for_each(|e| expr_slots(e, out)),
        CExpr::Dot(e, _) | CExpr::Neg(e) | CExpr::Count(e) | CExpr::Agg(_, e) => expr_slots(e, out),
        CExpr::Bin(_, l, r) | CExpr::In(l, r) => {
            expr_slots(l, out);
            expr_slots(r, out);
        }
        CExpr::CountCond(scope) => out.extend(&scope.needs),
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
        CCond::Pattern { subj, val, .. } => {
            expr_slots(subj, out);
            expr_slots(val, out);
        }
        CCond::Cmp { left, right, .. } => {
            expr_slots(left, out);
            expr_slots(right, out);
        }
    }
}

fn expr_consts(e: &mut CExpr, f: &mut dyn FnMut(&mut Value)) {
    match e {
        CExpr::Const(value) => f(value),
        CExpr::Var(_) | CExpr::Handle(_) | CExpr::Index(_) => {}
        CExpr::The(the) => the.desc.iter_mut().flat_map(|(_, es)| es).for_each(|e| expr_consts(e, f)),
        CExpr::Dot(e, _) | CExpr::Neg(e) | CExpr::Count(e) | CExpr::Agg(_, e) => expr_consts(e, f),
        CExpr::Bin(_, l, r) | CExpr::In(l, r) => {
            expr_consts(l, f);
            expr_consts(r, f);
        }
        CExpr::CountCond(scope) => cond_consts(&mut scope.cond, f),
    }
}

fn cond_consts(c: &mut CCond, f: &mut dyn FnMut(&mut Value)) {
    match c {
        CCond::And(cs) | CCond::Or(cs) => cs.iter_mut().for_each(|c| cond_consts(c, f)),
        CCond::Not(scope) => cond_consts(&mut scope.cond, f),
        CCond::In(world, body) => {
            expr_consts(world, f);
            cond_consts(body, f);
        }
        CCond::Any(_) => {}
        CCond::Pattern { subj, val, .. } => {
            expr_consts(subj, f);
            expr_consts(val, f);
        }
        CCond::Cmp { left, right, .. } => {
            expr_consts(left, f);
            expr_consts(right, f);
        }
    }
}

//! Gives names their meaning: a name is a variable if the statement introduced it with a kind,
//! and otherwise the handle of an object, which must exist when the line is stated.

use crate::ast::{Cond, Expr, Field, Head, PTerm};
use crate::error::Res;
use crate::eval::{Eval, View};
use crate::logic::{CCond, CExpr, CThe, Never, Rule, Scope, Var, cond_slots, expr_slots};
use crate::value::{ObjId, Sym, Value, a, is_primitive};
use crate::world::World;
use std::collections::HashMap;
use std::rc::Rc;

pub struct Compiler<'w> {
    pub world: &'w World,
    pub vars: Vec<Var>,
    /// The objects the statement names, in the order it names them.
    pub objects: Vec<ObjId>,
    /// Variables by name, innermost scope last.
    scopes: Vec<HashMap<Sym, usize>>,
    /// The worlds each enclosing `in` may look into, and whether `in` named exactly one.
    inside: Vec<(Vec<Rc<World>>, bool)>,
}

impl<'w> Compiler<'w> {
    pub fn new(world: &'w World) -> Self {
        Compiler { world, vars: Vec::new(), objects: Vec::new(), scopes: vec![HashMap::new()], inside: Vec::new() }
    }

    /// What evaluates the values and conditions compiled so far.
    pub fn eval(&self) -> Eval<'_> {
        Eval { vars: &self.vars, objects: &self.objects }
    }

    /// Opens the statement: declares the variables it names up front, then those its condition
    /// introduces. Returns their slots in order.
    pub fn open(&mut self, named: &[(&Sym, &Sym)], cond: &Cond) -> Res<Vec<usize>> {
        let mut fresh = Vec::new();
        for (kind, var) in named {
            self.declare(kind, var, &mut fresh)?;
        }
        self.introduce(cond, &mut fresh)?;
        Ok(fresh)
    }

    fn slot(&self, name: &str) -> Option<usize> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name).copied())
    }

    fn here(&self) -> bool {
        self.inside.is_empty()
    }

    /// Checks that a primitive kind was brought in with `from`. Inside `in w:` that is w's affair.
    fn need(&self, primitive: &str) -> Res<()> {
        if self.here() && !self.world.kinds.contains_key(primitive) { Err(self.world.no_kind(primitive)) } else { Ok(()) }
    }

    fn object(&mut self, id: ObjId) -> CExpr {
        self.objects.push(id);
        CExpr::Obj(self.objects.len() - 1)
    }

    /// Compiles what is inside `in w:`. A named world is looked into alone; a variable may be any
    /// held world.
    fn within<T>(&mut self, world: &Expr, body: impl FnOnce(&mut Self) -> Res<T>) -> Res<T> {
        let pick = |from: &World| -> (Vec<Rc<World>>, bool) {
            let named = match world {
                Expr::Name(name) if self.slot(name).is_none() => match from.find(name).as_slice() {
                    [id] => from.objects[id].held.clone(),
                    _ => None,
                },
                _ => None,
            };
            match named {
                Some(world) => (vec![world], true),
                None => (from.worlds().cloned().collect(), false),
            }
        };
        let next = match self.inside.last() {
            None => pick(self.world),
            Some((worlds, _)) => {
                let picks: Vec<_> = worlds.iter().map(|w| pick(w)).collect();
                let exact = picks.len() == 1 && picks[0].1;
                (picks.into_iter().flat_map(|p| p.0).collect(), exact)
            }
        };
        self.inside.push(next);
        let result = body(self);
        self.inside.pop();
        result
    }

    fn declare(&mut self, kind: &Sym, var: &Sym, fresh: &mut Vec<usize>) -> Res<()> {
        if let Some(slot) = self.slot(var) {
            let known = &self.vars[slot].kind;
            return if known == kind { Ok(()) } else { Err(format!("`{var}` is {}, not {}", a(known), a(kind))) };
        }
        if self.here() {
            if !self.world.kinds.contains_key(kind) {
                return Err(self.world.no_kind(kind));
            }
            if !self.world.find(var).is_empty() {
                return Err(format!("`{var}` is already an object"));
            }
            if self.world.kinds.contains_key(var) {
                return Err(format!("`{var}` is already a kind"));
            }
        }
        self.vars.push(Var { name: var.clone(), kind: kind.clone(), foreign: !self.here() });
        self.scopes.last_mut().unwrap().insert(var.clone(), self.vars.len() - 1);
        fresh.push(self.vars.len() - 1);
        Ok(())
    }

    /// Four names in a row: the reading that starts with a kind, if the first name is one.
    fn meant<'c>(&self, name: &str, as_kind: &'c Cond, as_term: &'c Cond) -> &'c Cond {
        if self.is_kind(name) { as_kind } else { as_term }
    }

    /// The relation a `by` relation reads backwards, wherever the subject's kind is known.
    fn by(&self, subj: &CExpr, rel: &Sym) -> Option<Sym> {
        let kind = self.subject_kind(subj)?;
        let field = match self.inside.last() {
            None => self.world.field(&kind, rel),
            Some((worlds, _)) => worlds.iter().find_map(|w| w.field(&kind, rel)),
        };
        field?.by.clone()
    }

    /// Whether a name is a kind where the line runs.
    fn is_kind(&self, name: &str) -> bool {
        match self.inside.last() {
            None => self.world.kinds.contains_key(name),
            Some((worlds, _)) => worlds.iter().any(|w| w.kinds.contains_key(name)),
        }
    }

    /// Declares every variable this condition introduces in the current scope. Scopes inside it
    /// (`not`, `count`) declare their own.
    fn introduce(&mut self, cond: &Cond, fresh: &mut Vec<usize>) -> Res<()> {
        match cond {
            Cond::And(cs) | Cond::Or(cs) => cs.iter().try_for_each(|c| self.introduce(c, fresh))?,
            Cond::In(world, body) => self.within(world, |c| c.introduce(body, fresh))?,
            Cond::Any(kind, var) => self.declare(kind, var, fresh)?,
            Cond::Pattern { subj, val, .. } => {
                for term in [subj, val] {
                    if let PTerm::Intro(kind, var) = term {
                        self.declare(kind, var, fresh)?;
                    }
                }
            }
            Cond::Either { name, as_kind, as_term } => self.introduce(self.meant(name, as_kind, as_term), fresh)?,
            Cond::Not(_) | Cond::Cmp(..) | Cond::Value(_) => {}
        }
        Ok(())
    }

    fn scope(&mut self, inner: &Cond) -> Res<Scope> {
        self.scopes.push(HashMap::new());
        let mut locals = Vec::new();
        let cond = self.introduce(inner, &mut locals).and_then(|()| self.cond(inner));
        self.scopes.pop();
        let cond = cond?;
        let mut needs = Vec::new();
        cond_slots(&cond, &mut needs);
        needs.retain(|slot| !locals.contains(slot));
        Ok(Scope { locals, needs, cond })
    }

    pub fn cond(&mut self, cond: &Cond) -> Res<CCond> {
        Ok(match cond {
            Cond::And(cs) => CCond::And(cs.iter().map(|c| self.cond(c)).collect::<Res<_>>()?),
            Cond::Or(cs) => CCond::Or(cs.iter().map(|c| self.cond(c)).collect::<Res<_>>()?),
            Cond::Not(inner) => CCond::Not(Box::new(self.scope(inner)?)),
            Cond::In(world, body) => CCond::In(self.expr(world)?, Box::new(self.within(world, |c| c.cond(body))?)),
            Cond::Any(_, var) => CCond::Any(self.introduced(var)?),
            Cond::Pattern { subj, rel, val } => {
                let subj = match subj {
                    PTerm::Expr(e) => self.subject_with(e, &[rel])?,
                    intro => self.term(intro)?,
                };
                let val = match val {
                    PTerm::Expr(e) => self.value_of(e, self.subject_kind(&subj).as_ref(), rel)?,
                    intro => self.term(intro)?,
                };
                self.check_fact(&subj, rel, &val)?;
                // A `by` relation is its other end read backwards.
                let by = self.by(&subj, rel);
                let (subj, rel, val) = match &by {
                    Some(by) => (val, by, subj),
                    None => (subj, rel, val),
                };
                let mut needs = Vec::new();
                for term in [&subj, &val] {
                    if !matches!(term, CExpr::Var(_)) {
                        expr_slots(term, &mut needs);
                    }
                }
                CCond::Pattern { subj, rel: rel.clone(), val, needs }
            }
            Cond::Either { name, as_kind, as_term } => self.cond(self.meant(name, as_kind, as_term))?,
            Cond::Cmp(op, left, right) => {
                // A handle takes the kind of what it is compared with.
                let (left, right) = if matches!(left, Expr::Name(_)) && !matches!(right, Expr::Name(_)) {
                    let right = self.expr(right)?;
                    (self.like(left, &right)?, right)
                } else {
                    let left = self.expr(left)?;
                    let right = self.like(right, &left)?;
                    (left, right)
                };
                let mut needs = Vec::new();
                expr_slots(&left, &mut needs);
                expr_slots(&right, &mut needs);
                CCond::Cmp { op, left, right, needs }
            }
            Cond::Value(_) => return Err("a condition is a fact or a comparison, not a value alone".into()),
        })
    }

    fn introduced(&self, var: &Sym) -> Res<usize> {
        self.slot(var).ok_or_else(|| format!("`{var}` was never introduced"))
    }

    pub fn term(&mut self, term: &PTerm) -> Res<CExpr> {
        match term {
            PTerm::Intro(_, var) => Ok(CExpr::Var(self.introduced(var)?)),
            PTerm::Expr(e) => self.expr(e),
        }
    }

    /// A value where the place settles which kind it takes: a handle shared across kinds names
    /// the object that fits.
    fn expr_where(&mut self, e: &Expr, fits: impl Fn(&Sym) -> bool) -> Res<CExpr> {
        match e {
            Expr::Name(name) if self.slot(name).is_none() && self.here() => {
                let id = self.world.resolve_where(name, fits)?;
                Ok(self.object(id))
            }
            e => self.expr(e),
        }
    }

    /// A value of the same kind as another, when the shapes settle that kind.
    fn like(&mut self, e: &Expr, other: &CExpr) -> Res<CExpr> {
        let kind = self.kind_of(other);
        self.expr_where(e, |k| kind.as_ref() == Some(k))
    }

    /// A value of `relation`'s kind, as the shape of `kind` declares it.
    pub fn value_of(&mut self, e: &Expr, kind: Option<&Sym>, rel: &Sym) -> Res<CExpr> {
        let takes = kind.and_then(|k| self.world.field(k, rel)).map(|f| f.kind.clone());
        self.expr_where(e, |k| takes.as_ref() == Some(k))
    }

    /// A subject that has every one of these relations.
    pub fn subject_with(&mut self, e: &Expr, rels: &[&Sym]) -> Res<CExpr> {
        let world = self.world;
        self.expr_where(e, |k| rels.iter().all(|r| world.field(k, r).is_some()))
    }

    /// The kind of a subject: a variable's, or its value's when the shapes settle it.
    pub fn subject_kind(&self, e: &CExpr) -> Option<Sym> {
        match e {
            CExpr::Var(slot) => Some(self.vars[*slot].kind.clone()),
            e => self.kind_of(e),
        }
    }

    pub fn expr(&mut self, e: &Expr) -> Res<CExpr> {
        Ok(match e {
            Expr::Num(n) => {
                self.need("number")?;
                CExpr::Const(Value::Num(*n))
            }
            Expr::Text(t) => {
                self.need("text")?;
                CExpr::Const(Value::Text(t.clone()))
            }
            Expr::Name(name) => match (self.slot(name), self.inside.last()) {
                (Some(slot), _) => CExpr::Var(slot),
                (None, None) => self.object(self.world.resolve(name)?),
                (None, Some((worlds, exact))) => {
                    if *exact && worlds[0].find(name).is_empty() {
                        return Err(format!("`{name}` doesn't exist in {}", worlds[0].name));
                    }
                    CExpr::Handle(name.clone())
                }
            },
            Expr::The(the) => {
                if let Some(kind) = the.kind.as_ref().filter(|k| !self.is_kind(k)) {
                    return Err(format!("kind `{kind}` doesn't exist yet"));
                }
                let named = self.objects.len();
                let mut desc = Vec::new();
                let mut slots = Vec::new();
                // The objects the description may fit say which kind each of its values takes.
                let world = self.world;
                let fits = |id: &ObjId| the.kind.as_ref().is_none_or(|k| world.objects[id].kind == *k);
                let kinds: Vec<&Sym> = world.find(&the.handle).iter().filter(|id| fits(id)).map(|id| &world.objects[id].kind).collect();
                for (rel, values) in &the.desc {
                    let takes = |k: &Sym| kinds.iter().any(|kind| world.field(kind, rel).is_some_and(|f| f.kind == *k));
                    let values = values.iter().map(|e| self.expr_where(e, takes)).collect::<Res<Vec<_>>>()?;
                    values.iter().for_each(|e| expr_slots(e, &mut slots));
                    desc.push((rel.clone(), values));
                }
                let the = CThe { kind: the.kind.clone(), handle: the.handle.clone(), desc };
                if self.here() && slots.is_empty() {
                    // A description without variables points at one object, now, and only that
                    // object is named.
                    let id = self.eval().the(&the, &mut vec![None; self.vars.len()], &View::of(self.world)?)?;
                    self.objects.truncate(named);
                    self.object(id)
                } else {
                    CExpr::The(Box::new(the))
                }
            }
            Expr::Index(n) => {
                self.need("id")?;
                CExpr::Index(*n)
            }
            Expr::Dot(base, rel) => {
                let base = self.subject_with(base, &[rel])?;
                if let Some(kind) = self.kind_of(&base) {
                    self.takes(&kind, rel)?;
                    if let Some(Field { kind, by: Some(by), .. }) = self.world.field(&kind, rel) {
                        return Ok(CExpr::Back(Box::new(base), by.clone(), kind.clone()));
                    }
                }
                CExpr::Dot(Box::new(base), rel.clone())
            }
            Expr::Neg(e) => CExpr::Neg(Box::new(self.expr(e)?)),
            Expr::Bin(op, l, r) => CExpr::Bin(op, Box::new(self.expr(l)?), Box::new(self.expr(r)?)),
            Expr::Agg(op, e) => CExpr::Agg(op, Box::new(self.expr(e)?)),
            Expr::Count(cond) => CExpr::Count(Box::new(self.scope(cond)?)),
            Expr::In(world, body) => CExpr::In(Box::new(self.expr(world)?), Box::new(self.within(world, |c| c.expr(body))?)),
        })
    }

    /// The kind of a value, when the shapes of this world settle it.
    fn kind_of(&self, e: &CExpr) -> Option<Sym> {
        if !self.here() {
            return None;
        }
        match e {
            CExpr::Const(value) => Some(self.world.kind_of(value).into()),
            CExpr::Obj(i) => Some(self.world.objects[&self.objects[*i]].kind.clone()),
            CExpr::Var(slot) => (!self.vars[*slot].foreign).then(|| self.vars[*slot].kind.clone()),
            CExpr::Dot(base, rel) => self.takes(&self.kind_of(base)?, rel).ok()?,
            CExpr::Back(_, _, kind) => Some(kind.clone()),
            CExpr::Neg(_) | CExpr::Count(_) | CExpr::Agg("count" | "sum", _) => Some("number".into()),
            CExpr::Bin(_, e, _) | CExpr::Agg(_, e) => self.kind_of(e),
            CExpr::Handle(_) | CExpr::The(_) | CExpr::Index(_) | CExpr::In(..) => None,
        }
    }

    /// The kind of value `kind.rel` takes; `None` while parents disagree on it.
    fn takes(&self, kind: &Sym, rel: &Sym) -> Res<Option<Sym>> {
        if &**rel == "id" {
            self.need("id")?;
            return Ok(Some("number".into()));
        }
        if is_primitive(kind) && self.world.field(kind, rel).is_none() {
            return Err(format!("{} has no relations", a(kind)));
        }
        let field = self.world.relation(kind, rel)?;
        Ok((!self.world.is_disputed(kind, rel)).then(|| field.kind.clone()))
    }

    /// Checks a fact pattern against the shapes, as far as the kinds are known.
    fn check_fact(&self, subj: &CExpr, rel: &Sym, val: &CExpr) -> Res<()> {
        let Some(kind) = self.kind_of(subj) else { return Ok(()) };
        let Some(takes) = self.takes(&kind, rel)? else { return Ok(()) };
        match self.kind_of(val) {
            Some(given) if given != takes && !(&*takes == "id" && &*given == "number") => {
                Err(format!("`{rel}` takes {}, not {}", a(&takes), a(&given)))
            }
            _ => Ok(()),
        }
    }
}

/// `rule name: kind var rel val if cond`
pub fn rule(world: &World, name: &Sym, source: &str, head: &Head, cond: &Cond) -> Res<Rule> {
    let Head { kind, var, rel, val } = head;
    if world.has_primitive(kind) {
        return Err(format!("a rule states a fact about a thing, not about {}", a(kind)));
    }
    let mut c = Compiler::new(world);
    let mut named = vec![(kind, var)];
    if let PTerm::Intro(kind, var) = val {
        named.push((kind, var));
    }
    let subj = c.open(&named, cond)?[0];
    match world.field(kind, rel) {
        None => return Err(format!("{kind} doesn't declare `{rel}`")),
        Some(Field { by: Some(by), .. }) => return Err(format!("`{rel}` is read from `{by}`; a rule can't state it")),
        Some(_) => {}
    }
    let val = match val {
        PTerm::Expr(e) => c.value_of(e, Some(kind), rel)?,
        intro => c.term(intro)?,
    };
    c.check_fact(&CExpr::Var(subj), rel, &val)?;
    let cond = c.cond(cond)?;
    Ok(Rule { name: name.clone(), source: source.into(), vars: c.vars, objects: c.objects, subj, rel: rel.clone(), val, cond })
}

/// `never name: kind var where cond`
pub fn never(world: &World, name: &Sym, source: &str, kind: &Sym, var: &Sym, cond: &Cond) -> Res<Never> {
    if world.has_primitive(kind) {
        return Err(format!("a constraint is about things, not about {}", a(kind)));
    }
    let mut c = Compiler::new(world);
    let subj = c.open(&[(kind, var)], cond)?[0];
    let cond = c.cond(cond)?;
    Ok(Never { name: name.clone(), source: source.into(), vars: c.vars, objects: c.objects, subj, cond })
}

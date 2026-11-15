//! Evaluation: what a value is, and which bindings of its variables make a condition true.

use crate::ast::{AggOp, BinOp, CmpOp};
use crate::equal::same_shape;
use crate::error::Res;
use crate::logic::{CCond, CExpr, CThe, Var};
use crate::num::Rat;
use crate::value::{ObjId, Value, a, plural};
use crate::world::{Facts, World};
use std::collections::BTreeSet;

/// A world together with the facts that hold in it: stated, and derived so far.
pub struct View<'a> {
    pub world: &'a World,
    pub facts: &'a Facts,
}

impl View<'_> {
    fn indexed(&self) -> bool {
        self.world.kinds.contains_key("id")
    }

    /// The values of a relation of a subject. `id` is every object's index.
    fn values(&self, subj: ObjId, rel: &str) -> Vec<Value> {
        if rel == "id" {
            let index = self.world.objects.get(&subj).filter(|_| self.indexed()).map(|o| o.index);
            return index.map(|i| Value::Num(Rat::int(i as i128))).into_iter().collect();
        }
        self.facts.values(subj, rel).cloned().collect()
    }

    fn subjects(&self, rel: &str, value: &Value) -> Vec<ObjId> {
        if rel == "id" {
            let Value::Num(n) = value else { return Vec::new() };
            return n.as_index().map(|index| self.numbered(index)).unwrap_or_default();
        }
        self.facts.subjects(rel, value).collect()
    }

    fn pairs(&self, rel: &str) -> Vec<(ObjId, Value)> {
        if rel == "id" {
            return self.world.objects.keys().flat_map(|id| self.values(*id, rel).into_iter().map(|v| (*id, v))).collect();
        }
        self.facts.pairs(rel).map(|(s, v)| (s, v.clone())).collect()
    }

    pub fn has(&self, subj: ObjId, rel: &str, value: &Value) -> bool {
        if rel == "id" { self.values(subj, rel).contains(value) } else { self.facts.contains(subj, rel, value) }
    }

    /// Every object with this index.
    fn numbered(&self, index: u64) -> Vec<ObjId> {
        if !self.indexed() {
            return Vec::new();
        }
        self.world.objects.iter().filter(|(_, o)| o.index == index).map(|(id, _)| *id).collect()
    }
}

/// One value per variable, once it has one.
pub type Env = Vec<Option<Value>>;

/// What to do with each solution; `false` stops the search.
type Next<'k> = &'k mut dyn FnMut(&mut Env) -> Res<bool>;

pub struct Eval<'a> {
    pub vars: &'a [Var],
}

impl Eval<'_> {
    /// Calls `next` for every way the condition holds, with the variables it binds bound.
    /// Returns `false` if `next` stopped the search.
    pub fn solve(&self, cond: &CCond, env: &mut Env, view: &View, next: Next) -> Res<bool> {
        match cond {
            CCond::And(all) => self.all(&all.iter().collect::<Vec<_>>(), env, view, next),
            CCond::Or(any) => {
                for cond in any {
                    if !self.solve(cond, env, view, next)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            CCond::Not(scope) => self.ground(&scope.needs, env, view, &mut |env| {
                if self.holds(&scope.cond, env, view)? { Ok(true) } else { next(env) }
            }),
            CCond::In(world, body) => {
                let mut needs = Vec::new();
                crate::logic::expr_slots(world, &mut needs);
                self.ground(&needs, env, view, &mut |env| {
                    for held in self.expr(world, env, view, false)? {
                        let inner = held_world(&held, view)?;
                        let model = inner.model()?;
                        if !self.solve(body, env, &View { world: inner, facts: &model.facts }, next)? {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                })
            }
            CCond::Any(slot) => match &env[*slot] {
                Some(value) if self.fits(*slot, value, view) => next(env),
                Some(_) => Ok(true),
                None => self.ground(&[*slot], env, view, next),
            },
            CCond::Pattern { subj, rel, val, needs } => {
                self.ground(needs, env, view, &mut |env| self.pattern(subj, rel, val, env, view, next))
            }
            CCond::Cmp { op, left, right, needs } => self.ground(needs, env, view, &mut |env| {
                let left = self.expr(left, env, view, false)?;
                let right = self.expr(right, env, view, false)?;
                // `!=` is the one comparison that every pair must pass: nothing on one side
                // may equal anything on the other.
                let mut pairs = left.iter().flat_map(|l| right.iter().map(move |r| (l, r)));
                let holds = if *op == CmpOp::Ne {
                    !left.is_empty()
                        && !right.is_empty()
                        && pairs.try_fold(true, |all, (l, r)| Ok::<_, String>(all && compare(*op, l, r, view)?))?
                } else {
                    pairs.try_fold(false, |any, (l, r)| Ok::<_, String>(any || compare(*op, l, r, view)?))?
                };
                if holds { next(env) } else { Ok(true) }
            }),
        }
    }

    /// Whether the condition holds at all.
    pub fn holds(&self, cond: &CCond, env: &mut Env, view: &View) -> Res<bool> {
        let mut found = false;
        self.solve(cond, env, view, &mut |_| {
            found = true;
            Ok(false)
        })?;
        Ok(found)
    }

    /// A conjunction. Conditions that can be judged with what is bound so far go first, so a
    /// pattern narrows the search before a comparison or a `not` needs its variables.
    fn all(&self, conds: &[&CCond], env: &mut Env, view: &View, next: Next) -> Res<bool> {
        if conds.is_empty() {
            return next(env);
        }
        let ready = |cond: &&CCond| {
            let needs: &[usize] = match cond {
                CCond::Not(scope) => &scope.needs,
                CCond::Pattern { needs, .. } | CCond::Cmp { needs, .. } => needs,
                _ => &[],
            };
            needs.iter().all(|slot| env[*slot].is_some())
        };
        let mut rest = conds.to_vec();
        let first = rest.remove(conds.iter().position(ready).unwrap_or(0));
        self.solve(first, env, view, &mut |env| self.all(&rest, env, view, next))
    }

    /// Gives every one of these variables a value, trying each object of its kind in turn.
    pub fn ground(&self, slots: &[usize], env: &mut Env, view: &View, next: Next) -> Res<bool> {
        let Some(&slot) = slots.iter().find(|slot| env[**slot].is_none()) else { return next(env) };
        let var = &self.vars[slot];
        if crate::value::is_primitive(&var.kind) {
            return Err(format!("`{0}` has no value here; give it one with a fact, like `p r {1} {0}`", var.name, var.kind));
        }
        for id in view.world.of_kind(&var.kind).collect::<Vec<_>>() {
            env[slot] = Some(Value::Obj(id));
            let goes_on = self.ground(slots, env, view, next);
            env[slot] = None;
            if !goes_on? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn fits(&self, slot: usize, value: &Value, view: &View) -> bool {
        view.world.fits(&self.vars[slot].kind, value)
    }

    fn bind(&self, slot: usize, value: Value, env: &mut Env, next: Next) -> Res<bool> {
        env[slot] = Some(value);
        let goes_on = next(env);
        env[slot] = None;
        goes_on
    }

    /// A fact pattern. A variable without a value is bound by the facts that match.
    fn pattern(&self, subj: &CExpr, rel: &str, val: &CExpr, env: &mut Env, view: &View, next: Next) -> Res<bool> {
        let free = |e: &CExpr, env: &Env| match e {
            CExpr::Var(slot) if env[*slot].is_none() => Some(*slot),
            _ => None,
        };
        match (free(subj, env), free(val, env)) {
            (None, None) => {
                let subjects = self.expr(subj, env, view, false)?;
                let values = self.expr(val, env, view, false)?;
                let holds = subjects.iter().any(|s| matches!(s, Value::Obj(id) if values.iter().any(|v| view.has(*id, rel, v))));
                if holds { next(env) } else { Ok(true) }
            }
            (None, Some(slot)) => {
                let mut found = BTreeSet::new();
                for subject in self.expr(subj, env, view, false)? {
                    let Value::Obj(id) = subject else { continue };
                    found.extend(view.values(id, rel).into_iter().filter(|v| self.fits(slot, v, view)));
                }
                for value in found {
                    if !self.bind(slot, value, env, next)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (Some(slot), None) => {
                let mut found = BTreeSet::new();
                for value in self.expr(val, env, view, false)? {
                    found.extend(view.subjects(rel, &value).into_iter().map(Value::Obj).filter(|s| self.fits(slot, s, view)));
                }
                for subject in found {
                    if !self.bind(slot, subject, env, next)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (Some(s), Some(v)) => {
                for (subject, value) in view.pairs(rel) {
                    let subject = Value::Obj(subject);
                    if (s == v && subject != value) || !self.fits(s, &subject, view) || !self.fits(v, &value, view) {
                        continue;
                    }
                    env[s] = Some(subject);
                    let goes_on = self.bind(v, value, env, next);
                    env[s] = None;
                    if !goes_on? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
        }
    }

    /// The values of an expression: none, one or several, one per fact that gives one.
    ///
    /// When `strict`, computing with a missing value is an error; in a condition it just
    /// doesn't hold.
    pub fn expr(&self, e: &CExpr, env: &mut Env, view: &View, strict: bool) -> Res<Vec<Value>> {
        Ok(match e {
            CExpr::Const(value) => vec![value.clone()],
            CExpr::Var(slot) => match &env[*slot] {
                Some(value) => vec![value.clone()],
                None => return Err(format!("`{}` has no value here", self.vars[*slot].name)),
            },
            CExpr::Handle(handle) => match view.world.find(handle).as_slice() {
                [] => Vec::new(),
                [id] => vec![Value::Obj(*id)],
                several => return Err(format!("{} objects called {handle} in {}; use `the`", several.len(), view.world.name)),
            },
            CExpr::The(the) => vec![Value::Obj(self.the(the, env, view)?)],
            CExpr::Index(index) => view.numbered(*index).into_iter().map(Value::Obj).collect(),
            CExpr::Dot(base, rel) => {
                // The base is a set of things here, so each thing gives its values once.
                let mut values = Vec::new();
                for subject in self.expr(base, env, view, strict)? {
                    match subject {
                        Value::Obj(id) => values.extend(view.values(id, rel)),
                        other => return Err(format!("{} has no relations", a(view.world.kind_of(&other)))),
                    }
                }
                things_once(values)
            }
            CExpr::Neg(e) => {
                let negate = |v: Value| match v {
                    Value::Num(n) => n.neg().map(Value::Num).ok_or_else(too_large),
                    other => Err(format!("`-` needs a number, not {}", a(view.world.kind_of(&other)))),
                };
                let values = self.expr(e, env, view, strict)?;
                if strict && values.is_empty() {
                    return Err(self.missing(e, env, view));
                }
                values.into_iter().map(negate).collect::<Res<_>>()?
            }
            CExpr::Bin(op, left, right) => {
                let l = self.expr(left, env, view, strict)?;
                let r = self.expr(right, env, view, strict)?;
                if strict && let Some(side) = [(left, &l), (right, &r)].iter().find(|(_, values)| values.is_empty()) {
                    return Err(self.missing(side.0, env, view));
                }
                if l.len().saturating_mul(r.len()) > PAIRS_LIMIT {
                    return Err(format!("`{op}` has too many values to combine: {} and {}", l.len(), r.len()));
                }
                let mut values = Vec::new();
                for a in &l {
                    for b in &r {
                        values.push(compute(*op, a, b, view)?);
                    }
                }
                values
            }
            CExpr::Count(e) => vec![count(self.expr(e, env, view, false)?.len())],
            CExpr::CountCond(scope) => {
                let mut matches = BTreeSet::new();
                self.solve(&scope.cond, env, view, &mut |env| {
                    self.ground(&scope.locals, env, view, &mut |env| {
                        matches.insert(scope.locals.iter().map(|slot| env[*slot].clone()).collect::<Vec<_>>());
                        Ok(true)
                    })
                })?;
                vec![count(matches.len())]
            }
            CExpr::Agg(op, e) => {
                let values = self.expr(e, env, view, strict)?;
                let kinds: BTreeSet<&str> = values.iter().map(|v| view.world.kind_of(v)).collect();
                match (op, values.first()) {
                    (AggOp::Sum, _) => {
                        let mut total = Rat::ZERO;
                        for value in &values {
                            let Value::Num(n) = value else {
                                return Err(format!("`sum` adds numbers, not {}", plural(view.world.kind_of(value))));
                            };
                            total = total.add(*n).ok_or_else(too_large)?;
                        }
                        vec![Value::Num(total)]
                    }
                    (_, None) if strict => return Err(format!("there is nothing to take the `{op}` of")),
                    (_, None) => Vec::new(),
                    (_, Some(Value::Obj(_))) => {
                        return Err(format!("`{op}` orders numbers or text, not {}", plural(view.world.kind_of(&values[0]))));
                    }
                    (_, Some(_)) if kinds.len() > 1 => return Err(format!("`{op}` can't order numbers against text")),
                    (AggOp::Min, Some(_)) => values.into_iter().min_by(in_order).into_iter().collect(),
                    (AggOp::Max, Some(_)) => values.into_iter().max_by(in_order).into_iter().collect(),
                }
            }
            CExpr::In(world, body) => {
                let mut values = Vec::new();
                for held in self.expr(world, env, view, strict)? {
                    let inner = held_world(&held, view)?;
                    let model = inner.model()?;
                    values.extend(self.expr(body, env, &View { world: inner, facts: &model.facts }, strict)?);
                }
                values
            }
        })
    }

    /// Names the value that is missing, as in "harbor has no `age`".
    fn missing(&self, e: &CExpr, env: &mut Env, view: &View) -> String {
        if let CExpr::Dot(base, rel) = e
            && let Ok(owners) = self.expr(base, env, view, false)
        {
            return match owners.as_slice() {
                [owner] => format!("{} has no `{rel}`", view.world.show(owner)),
                _ => format!("nothing here has `{rel}`"),
            };
        }
        "a value is missing here".into()
    }

    /// The one object a description fits.
    pub fn the(&self, the: &CThe, env: &mut Env, view: &View) -> Res<ObjId> {
        let mut fitting = Vec::new();
        'candidates: for id in view.world.find(&the.handle) {
            if the.kind.as_ref().is_some_and(|kind| *kind != view.world.objects[&id].kind) {
                continue;
            }
            for (rel, described) in &the.desc {
                for e in described {
                    let values = self.expr(e, env, view, false)?;
                    if values.is_empty() || !values.iter().all(|v| view.has(id, rel, v)) {
                        continue 'candidates;
                    }
                }
            }
            fitting.push(id);
        }
        let noun = the.kind.as_deref().map_or("objects".into(), plural);
        match fitting.as_slice() {
            [id] => Ok(*id),
            [] => Err(format!("no {noun} match this description")),
            several => Err(format!("{} {noun} match this description", several.len())),
        }
    }
}

/// The most pairs of values one operator will combine.
const PAIRS_LIMIT: usize = 1_000_000;

/// The longest text a join may make, in bytes.
const TEXT_LIMIT: usize = 1 << 20;

/// A collection with each thing in it once. Numbers and text stay one per fact that gave
/// them, so two friends of the same age are still two ages.
fn things_once(values: Vec<Value>) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    values.into_iter().filter(|v| !matches!(v, Value::Obj(_)) || seen.insert(v.clone())).collect()
}

/// Alphabetical order: letters first whatever their case, then capitals before small letters.
fn alphabetical(left: &str, right: &str) -> std::cmp::Ordering {
    left.to_lowercase().cmp(&right.to_lowercase()).then_with(|| left.cmp(right))
}

/// The order `min` and `max` use: numbers by size, text alphabetically.
fn in_order(left: &Value, right: &Value) -> std::cmp::Ordering {
    match (left, right) {
        (Value::Text(l), Value::Text(r)) => alphabetical(l, r),
        _ => left.cmp(right),
    }
}

fn too_large() -> String {
    "this number is too large to keep exact".into()
}

fn count(n: usize) -> Value {
    Value::Num(Rat::int(n as i128))
}

fn held_world<'a>(value: &Value, view: &View<'a>) -> Res<&'a World> {
    let Value::Obj(id) = value else { return Err(format!("`in` looks inside a world, not {}", a(view.world.kind_of(value)))) };
    match view.world.objects.get(id).and_then(|o| o.held.as_deref()) {
        Some(world) => Ok(world),
        None => Err(format!("`in` looks inside a world, and {} is not one", view.world.show(value))),
    }
}

fn compute(op: BinOp, left: &Value, right: &Value, view: &View) -> Res<Value> {
    match (op, left, right) {
        (BinOp::Add, Value::Text(l), Value::Text(r)) if l.len() + r.len() > TEXT_LIMIT => Err("this text is too long to keep".into()),
        (BinOp::Add, Value::Text(l), Value::Text(r)) => Ok(Value::Text(format!("{l}{r}").into())),
        (_, Value::Num(l), Value::Num(r)) => {
            if r.is_zero() && matches!(op, BinOp::Div | BinOp::IntDiv | BinOp::Mod) {
                return Err("division by zero".into());
            }
            let result = match op {
                BinOp::Add => l.add(*r),
                BinOp::Sub => l.sub(*r),
                BinOp::Mul => l.mul(*r),
                BinOp::Div => l.div(*r),
                BinOp::IntDiv => l.div_floor(*r),
                BinOp::Mod => l.modulo(*r),
            };
            result.map(Value::Num).ok_or_else(too_large)
        }
        _ => {
            let (l, r) = (view.world.kind_of(left), view.world.kind_of(right));
            let needs = if op == BinOp::Add { "two numbers or two texts" } else { "two numbers" };
            Err(format!("`{op}` needs {needs}, not {} and {}", a(l), a(r)))
        }
    }
}

/// Compares two values. Worlds are equal when they have the same shape.
pub fn compare(op: CmpOp, left: &Value, right: &Value, view: &View) -> Res<bool> {
    let same = || {
        if left == right {
            return true;
        }
        let held = |v: &Value| match v {
            Value::Obj(id) => view.world.objects.get(id).and_then(|o| o.held.as_deref()),
            _ => None,
        };
        matches!((held(left), held(right)), (Some(l), Some(r)) if same_shape(l, r))
    };
    let order = match (op, left, right) {
        (CmpOp::Eq, ..) => return Ok(same()),
        (CmpOp::Ne, ..) => return Ok(!same()),
        (_, Value::Num(l), Value::Num(r)) => l.cmp(r),
        (_, Value::Text(l), Value::Text(r)) => alphabetical(l, r),
        _ => {
            let (l, r) = (view.world.kind_of(left), view.world.kind_of(right));
            return Err(format!("`{op}` orders two numbers or two texts, not {} and {}", a(l), a(r)));
        }
    };
    Ok(match op {
        CmpOp::Lt => order.is_lt(),
        CmpOp::Gt => order.is_gt(),
        CmpOp::Le => order.is_le(),
        _ => order.is_ge(),
    })
}

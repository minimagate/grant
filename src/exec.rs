//! Running statements: each line changes K, O, F or R of its world, or asks about it.

use crate::ast::{Assign, Change, Cond, EachAction, Expr, ShapeEdit, Stmt};
use crate::compile::{self, Compiler};
use crate::error::{Error, Fail, Res};
use crate::eval::{Env, Eval, View};
use crate::lexer::Line;
use crate::loader::Loader;
use crate::logic::{CExpr, Var, expr_slots};
use crate::model;
use crate::parser::parse;
use crate::value::{ObjId, PRIMITIVES, Sym, Value, a, is_primitive, plural, some};
use crate::world::{World, next_id};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// What a line showed: the answer to a question, or a remark about a change.
#[derive(Clone, Debug, PartialEq)]
pub struct Output {
    pub line: u32,
    pub source: String,
    pub text: String,
}

/// A world being stated, line by line. It has no identity until a line of it runs.
#[derive(Clone)]
pub struct Session {
    pub world: World,
    dir: PathBuf,
    file: Option<PathBuf>,
}

/// A bracket entry ready to compute, and what it then does to one subject.
type Plan = Assign<CExpr>;
type Edit = (ObjId, Assign<Value>);

impl Session {
    /// Starts an empty world. `dir` is where the names of other worlds resolve from.
    pub fn new(name: &str, dir: &Path, file: Option<&Path>) -> Session {
        Session { world: World::new(0, name), dir: dir.into(), file: file.map(Into::into) }
    }

    /// Runs one statement and returns what it showed.
    pub fn run(&mut self, loader: &mut Loader, line: &Line) -> Result<Vec<Output>, Error> {
        let mut said = Vec::new();
        match parse(&line.tokens).map_err(Fail::from).and_then(|stmt| self.exec(loader, stmt, line, &mut said)) {
            Ok(()) => Ok(said.into_iter().map(|text| Output { line: line.number, source: line.text.clone(), text }).collect()),
            Err(Fail(message, cause)) => Err(self.error(Some(line), vec![message], cause)),
        }
    }

    /// Ends the world: checks every constraint, shape word and leftover disagreement.
    pub fn finish(mut self) -> Result<World, Error> {
        if self.world.id == 0 {
            // A world with no lines is still a world of its own.
            self.world.id = next_id();
        }
        let problems = model::check(&self.world).unwrap_or_else(|message| vec![message]);
        if problems.is_empty() { Ok(self.world) } else { Err(self.error(None, problems, None)) }
    }

    fn error(&self, line: Option<&Line>, messages: Vec<String>, cause: Option<Rc<Error>>) -> Error {
        let line = line.map(|l| (l.number, l.text.clone()));
        Error { world: self.world.name.to_string(), file: self.file.clone(), line, messages, cause }
    }

    fn exec(&mut self, loader: &mut Loader, stmt: Stmt, line: &Line, said: &mut Vec<String>) -> Result<(), Fail> {
        let first = self.world.id == 0;
        if first && !matches!(stmt, Stmt::From(_)) {
            self.world.id = next_id();
        }
        match stmt {
            Stmt::From(_) if !first => return Err("`from` comes first, and lists every parent on one line".to_string().into()),
            Stmt::From(names) => {
                let (mut parents, mut primitives) = (Vec::new(), Vec::new());
                for name in &names {
                    match PRIMITIVES.iter().find(|(_, world)| world == name) {
                        Some((kind, _)) => primitives.push(Sym::from(*kind)),
                        None => parents.push(loader.named(&self.dir, name)?),
                    }
                }
                self.world = World::merge(next_id(), &self.world.name, &parents);
                primitives.iter().for_each(|kind| self.world.declare_kind(kind));
            }
            Stmt::Kind { name, edits } => self.kind(&name, edits)?,
            Stmt::RemoveKind(name) => self.remove_kind(&name)?,
            Stmt::Create { kind, handle, path, values } => {
                if self.world.kinds.contains_key(&handle) {
                    return Err(format!("`{handle}` is already a kind").into());
                }
                let id = match path {
                    Some(path) => self.world.hold(&handle, loader.named(&self.dir, &path)?),
                    None if self.world.has_primitive(&kind) => {
                        return Err(format!("{} isn't created; it is written, like `24` or `\"text\"`", a(&kind)).into());
                    }
                    None if !self.world.kinds.contains_key(&kind) => return Err(self.world.no_kind(&kind).into()),
                    None => self.world.create(&kind, &handle),
                };
                self.set(id, &values)?;
            }
            Stmt::Fact { subj, rel, value } => {
                let mut c = Compiler::new(&self.world);
                let (subj, value) = (c.expr(&subj)?, c.expr(&value)?);
                let subj = self.one(c.eval(), &subj)?;
                let kind = &self.world.objects[&subj].kind;
                self.world.relation(kind, &rel)?;
                let values = self.values(c.eval(), &value, &mut Vec::new())?;
                values.iter().try_for_each(|v| self.world.check_value(kind, &rel, v))?;
                values.into_iter().for_each(|v| self.world.state(subj, &rel, v));
            }
            Stmt::Set { subj, values } => {
                let subj = self.object(&subj)?;
                self.set(subj, &values)?;
            }
            Stmt::Remove { subj, rel, value } => self.remove(&subj, rel, value.as_ref(), said)?,
            Stmt::Each { kind, var, cond, action } => self.each(&kind, &var, &cond, &action, said)?,
            Stmt::Rule { name, head, cond } => {
                let rule = compile::rule(&self.world, &name, &one_line(&line.text), &head, &cond)?;
                if let Some(problem) = model::tangle_with(&self.world, &rule) {
                    return Err(problem.into());
                }
                self.world.set_rule(&name, Some(rule));
                // Derive now, so a rule that can't settle fails on its own line.
                self.world.model()?;
            }
            Stmt::Never { name, kind, var, cond } => {
                let never = compile::never(&self.world, &name, &one_line(&line.text), &kind, &var, &cond)?;
                self.world.set_never(&name, Some(never));
            }
            Stmt::Forget(what, name) => {
                let found = if what == "rule" { self.world.set_rule(&name, None) } else { self.world.set_never(&name, None) };
                if !found {
                    said.push(format!("nothing removed: there is no {what} `{name}`"));
                }
            }
            Stmt::Ask(cond) => said.push(self.ask(&cond)?),
            Stmt::InChange(name) => {
                let held = matches!(self.world.find(&name).as_slice(), [id] if self.world.objects[id].held.is_some());
                return Err(if held {
                    format!("{name} is held, not built on; write a world `from {name}`").into()
                } else {
                    "`in` looks inside a world; it starts a question, not a line".to_string().into()
                });
            }
        }
        Ok(())
    }

    // --- evaluating ---

    /// The values of an expression on this line. Missing values don't compute.
    fn values(&self, eval: Eval, e: &CExpr, env: &mut Env) -> Res<Vec<Value>> {
        env.resize(eval.vars.len(), None);
        // A value that is written out needs nothing derived.
        let stated = View { world: &self.world, facts: &self.world.facts };
        let view = if matches!(e, CExpr::Const(_) | CExpr::Obj(_)) { stated } else { View::of(&self.world)? };
        eval.expr(e, env, &view, true)
    }

    fn one(&self, eval: Eval, e: &CExpr) -> Res<ObjId> {
        match self.values(eval, e, &mut Vec::new())?.as_slice() {
            [Value::Obj(id)] => Ok(*id),
            _ => Err("a fact is about one thing".into()),
        }
    }

    /// The object a reference points at.
    fn object(&self, reference: &Expr) -> Res<ObjId> {
        let mut c = Compiler::new(&self.world);
        let e = c.expr(reference)?;
        self.one(c.eval(), &e)
    }

    // --- brackets ---

    fn plan(c: &mut Compiler, kind: &Sym, assigns: &[Assign]) -> Res<Vec<Plan>> {
        let mut plans = Vec::new();
        for Assign { rel, change, values } in assigns {
            c.world.relation(kind, rel)?;
            let values = values.iter().map(|e| c.expr(e)).collect::<Res<_>>()?;
            plans.push(Assign { rel: rel.clone(), change: *change, values });
        }
        Ok(plans)
    }

    /// Computes what the plans change for one subject, against the world as it stands.
    fn compute(&self, eval: Eval, plans: &[Plan], subj: ObjId, env: &mut Env) -> Res<Vec<Edit>> {
        let kind = &self.world.objects[&subj].kind;
        let mut edits = Vec::new();
        for plan in plans {
            let mut values = Vec::new();
            for e in &plan.values {
                values.extend(self.values(eval, e, env)?);
            }
            if plan.change != Change::Remove {
                values.iter().try_for_each(|v| self.world.check_value(kind, &plan.rel, v))?;
            }
            edits.push((subj, Assign { rel: plan.rel.clone(), change: plan.change, values }));
        }
        Ok(edits)
    }

    /// Applies computed edits. Everything a set replaces goes first, so sets on the same
    /// relation add up.
    fn apply(&mut self, edits: Vec<Edit>) {
        for (subj, edit) in edits.iter().filter(|(_, e)| e.change == Change::Set) {
            self.world.clear(*subj, &edit.rel);
        }
        for (subj, edit) in edits {
            for value in edit.values {
                match edit.change {
                    Change::Remove => drop(self.world.unstate(subj, &edit.rel, &value)),
                    _ => self.world.state(subj, &edit.rel, value),
                }
            }
        }
    }

    fn set(&mut self, subj: ObjId, assigns: &[Assign]) -> Res<()> {
        if assigns.is_empty() {
            return Ok(());
        }
        let mut c = Compiler::new(&self.world);
        let plans = Self::plan(&mut c, &self.world.objects[&subj].kind, assigns)?;
        let edits = self.compute(c.eval(), &plans, subj, &mut Vec::new())?;
        self.apply(edits);
        Ok(())
    }

    // --- kinds ---

    fn kind(&mut self, name: &Sym, edits: Vec<ShapeEdit>) -> Res<()> {
        if is_primitive(name) {
            return Err(if self.world.has_primitive(name) { format!("{} has no relations", a(name)) } else { self.world.no_kind(name) });
        }
        if !self.world.find(name).is_empty() {
            return Err(format!("`{name}` is already an object"));
        }
        self.world.declare_kind(name);
        for edit in edits {
            match edit {
                ShapeEdit::Declare(_, field) | ShapeEdit::Set(_, field) if !self.world.kinds.contains_key(&field.kind) => {
                    return Err(self.world.no_kind(&field.kind));
                }
                ShapeEdit::Declare(rel, field) => match self.world.field(name, &rel) {
                    Some(old) if *old != field || self.world.is_disputed(name, &rel) => {
                        return Err(format!("{name} already declares `{rel}: {old}`; write `+ {rel}: {field}` to change it"));
                    }
                    _ => self.world.set_field(name, &rel, Some(field)),
                },
                ShapeEdit::Set(rel, field) => self.world.set_field(name, &rel, Some(field)),
                ShapeEdit::Remove(rel) => {
                    let using = self.world.of_kind(name).filter(|id| self.world.facts.values(*id, &rel).next().is_some()).count();
                    if using > 0 {
                        let verb = if using == 1 { "has" } else { "have" };
                        return Err(format!("{} still {verb} `{rel}`", some(using, name)));
                    }
                    if let Some(rule) = self.world.rules.values().find(|r| r.rel == rel && r.vars[r.subj].kind == *name) {
                        return Err(format!("rule `{}` still produces `{rel}`", rule.name));
                    }
                    self.world.set_field(name, &rel, None);
                }
            }
        }
        Ok(())
    }

    fn remove_kind(&mut self, name: &Sym) -> Res<()> {
        if is_primitive(name) || &**name == "world" {
            return Err(format!("`{name}` is not this world's to remove"));
        }
        if !self.world.kinds.contains_key(name) {
            return Ok(());
        }
        let objects = self.world.of_kind(name).count();
        if objects > 0 {
            let verb = if objects == 1 { "uses" } else { "use" };
            return Err(format!("{} still {verb} `{name}`", some(objects, name)));
        }
        for (other, shape) in &self.world.kinds {
            if let Some((rel, field)) = shape.iter().find(|(_, f)| f.kind == *name)
                && other != name
            {
                return Err(format!("{other} still uses `{name}` (`{rel}: {field}`)"));
            }
        }
        let uses = |vars: &[Var]| vars.iter().any(|v| !v.foreign && v.kind == *name);
        if let Some(rule) = self.world.rules.values().find(|r| uses(&r.vars)) {
            return Err(format!("rule `{}` still uses `{name}`", rule.name));
        }
        if let Some(never) = self.world.nevers.values().find(|n| uses(&n.vars)) {
            return Err(format!("never `{}` still uses `{name}`", never.name));
        }
        self.world.remove_kind(name);
        Ok(())
    }

    // --- removing ---

    /// Removes an object, and remarks on the rules and constraints that name it: they stay,
    /// and no longer match anything through it.
    fn remove_object(&mut self, id: ObjId, said: &mut Vec<String>) {
        let name = self.world.show(&Value::Obj(id));
        let rules = self.world.rules.values().filter(|r| r.objects.contains(&id)).map(|r| format!("rule `{}`", r.name));
        let nevers = self.world.nevers.values().filter(|n| n.objects.contains(&id)).map(|n| format!("never `{}`", n.name));
        said.extend(rules.chain(nevers).map(|naming| format!("{naming} names {name}, and matches nothing through it now")));
        self.world.remove_object(id);
    }

    /// Remarks on the values of a relation that still hold after their removal.
    fn still_follows(&self, subj: ObjId, rel: &Sym, said: &mut Vec<String>) -> Res<()> {
        let model = self.world.model()?;
        let name = self.world.show(&Value::Obj(subj));
        for value in model.facts.values(subj, rel) {
            if let Some(rule) = model.why.get(&(subj, rel.clone(), value.clone())) {
                said.push(format!("{name} {rel} {} still follows from rule {rule}", self.world.show(value)));
            }
        }
        Ok(())
    }

    fn remove(&mut self, subj: &Expr, rel: Option<Sym>, value: Option<&Expr>, said: &mut Vec<String>) -> Res<()> {
        // Removing what isn't there does nothing.
        for e in [Some(subj), value].into_iter().flatten() {
            if let Expr::Name(name) = e
                && self.world.find(name).is_empty()
            {
                said.push(format!("nothing removed: `{name}` isn't here"));
                return Ok(());
            }
        }
        let subj = self.object(subj)?;
        let Some(rel) = rel else {
            self.remove_object(subj, said);
            return Ok(());
        };
        let kind = self.world.objects[&subj].kind.clone();
        if self.world.field(&kind, &rel).is_none() {
            said.push(format!("nothing removed: {kind} has no relation `{rel}`"));
            return Ok(());
        }
        let Some(value) = value else {
            self.world.clear(subj, &rel);
            return self.still_follows(subj, &rel, said);
        };
        let mut c = Compiler::new(&self.world);
        let value = c.expr(value)?;
        let values = self.values(c.eval(), &value, &mut Vec::new())?;
        let stated: Vec<bool> = values.iter().map(|v| self.world.unstate(subj, &rel, v)).collect();
        let model = self.world.model()?;
        let name = self.world.show(&Value::Obj(subj));
        for (value, stated) in values.iter().zip(stated) {
            if let Some(rule) = model.why.get(&(subj, rel.clone(), value.clone())) {
                said.push(match (stated, values.len()) {
                    (true, 1) => format!("removed, but it still follows from rule {rule}"),
                    (false, 1) => format!("it isn't stated here; it follows from rule {rule}"),
                    (true, _) => format!("removed, but {name} {rel} {} still follows from rule {rule}", self.world.show(value)),
                    (false, _) => format!("{name} {rel} {} isn't stated here; it follows from rule {rule}", self.world.show(value)),
                });
            }
        }
        Ok(())
    }

    // --- each ---

    fn each(&mut self, kind: &Sym, var: &Sym, cond: &Cond, action: &EachAction, said: &mut Vec<String>) -> Res<()> {
        if self.world.has_primitive(kind) {
            return Err(format!("`each` goes through things, and there is no end of {}", plural(kind)));
        }
        let mut c = Compiler::new(&self.world);
        let subj = c.open(&[(kind, var)], cond)?[0];
        let cond = c.cond(cond)?;
        // `None` removes the objects themselves.
        let one = |rel: &Sym, change, values| Some(vec![Assign { rel: rel.clone(), change, values }]);
        let plans = match action {
            EachAction::Set(assigns) => Some(Self::plan(&mut c, kind, assigns)?),
            EachAction::Fact(rel, value) => {
                self.world.relation(kind, rel)?;
                one(rel, Change::Add, vec![c.expr(value)?])
            }
            EachAction::RemoveObjects => None,
            EachAction::RemoveRel(rel, _) if self.world.field(kind, rel).is_none() => {
                said.push(format!("nothing removed: {kind} has no relation `{rel}`"));
                return Ok(());
            }
            EachAction::RemoveRel(rel, None) => one(rel, Change::Set, Vec::new()),
            EachAction::RemoveRel(rel, Some(value)) => one(rel, Change::Remove, vec![c.expr(value)?]),
        };
        let mut needs = vec![subj];
        for plan in plans.iter().flatten() {
            plan.values.iter().for_each(|e| expr_slots(e, &mut needs));
        }
        let (eval, view) = (c.eval(), View::of(&self.world)?);
        let mut matches = BTreeSet::new();
        eval.solve(&cond, &mut vec![None; c.vars.len()], &view, &mut |env| {
            eval.ground(&needs, env, &view, &mut |env| {
                matches.insert(env.clone());
                Ok(true)
            })
        })?;
        // Every value is computed here, before anything changes.
        let (mut edits, mut doomed) = (Vec::new(), BTreeSet::new());
        for mut env in matches {
            let Some(Value::Obj(id)) = env[subj] else { continue };
            match &plans {
                Some(plans) => edits.extend(self.compute(eval, plans, id, &mut env)?),
                None => drop(doomed.insert(id)),
            }
        }
        doomed.into_iter().for_each(|id| self.remove_object(id, said));
        let emptied: BTreeSet<(ObjId, Sym)> = match action {
            EachAction::RemoveRel(..) => edits.iter().map(|(subj, e)| (*subj, e.rel.clone())).collect(),
            _ => BTreeSet::new(),
        };
        self.apply(edits);
        emptied.iter().try_for_each(|(subj, rel)| self.still_follows(*subj, rel, said))
    }

    // --- asking ---

    fn ask(&self, question: &Cond) -> Res<String> {
        let mut c = Compiler::new(&self.world);
        let list = |values: &[Value]| -> String {
            let mut shown: Vec<&Value> = values.iter().collect::<BTreeSet<_>>().into_iter().collect();
            // Things of one kind are shown in the order of their indexes.
            let index = |v: &Value| match v {
                Value::Obj(id) => self.world.objects.get(id).map(|o| (&o.kind, o.index)),
                _ => None,
            };
            if shown.iter().all(|v| index(v).is_some_and(|(kind, _)| Some(kind) == index(shown[0]).map(|i| i.0))) {
                shown.sort_by_key(|v| index(v).map(|i| i.1));
            }
            if shown.is_empty() { "nothing".into() } else { shown.iter().map(|v| self.world.show(v)).collect::<Vec<_>>().join(", ") }
        };
        if let Cond::Value(value) = question {
            let value = c.expr(value)?;
            return Ok(list(&self.values(c.eval(), &value, &mut Vec::new())?));
        }
        let asked = c.open(&[], question)?;
        let cond = c.cond(question)?;
        let (eval, view) = (c.eval(), View::of(&self.world)?);
        let mut env = vec![None; c.vars.len()];
        if asked.is_empty() {
            return Ok(if eval.holds(&cond, &mut env, &view)? { "yes" } else { "no" }.into());
        }
        let mut rows = BTreeSet::new();
        eval.solve(&cond, &mut env, &view, &mut |env| {
            eval.ground(&asked, env, &view, &mut |env| {
                rows.insert(asked.iter().filter_map(|slot| env[*slot].clone()).collect::<Vec<_>>());
                Ok(true)
            })
        })?;
        Ok(match asked.as_slice() {
            _ if rows.is_empty() => "nothing".into(),
            [_] => list(&rows.into_iter().flatten().collect::<Vec<_>>()),
            _ => {
                let row = |row: &Vec<Value>| {
                    let cells = asked.iter().zip(row).map(|(slot, v)| format!("{}: {}", c.vars[*slot].name, self.world.show(v)));
                    cells.collect::<Vec<_>>().join(", ")
                };
                rows.iter().map(row).collect::<Vec<_>>().join("\n")
            }
        })
    }
}

/// A statement on one line, as rules and constraints are kept for showing.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

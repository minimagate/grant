//! Running statements: each line changes K, O, F or R of its world, or asks about it.

use crate::ast::{Assign, Cond, EachAction, Expr, ShapeEdit, Stmt, Values};
use crate::compile::{self, Compiler};
use crate::error::{Error, Fail, Res};
use crate::eval::{Env, Eval, View};
use crate::lexer::Line;
use crate::loader::Loader;
use crate::logic::{CCond, CExpr, Var, expr_slots};
use crate::model;
use crate::parser::parse;
use crate::value::{ObjId, Sym, Value, a, is_primitive, some};
use crate::world::{World, next_id};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// What a line showed: the answer to a question, or a remark about a change.
#[derive(Clone, Debug, PartialEq)]
pub struct Output {
    pub line: u32,
    pub source: String,
    pub text: String,
}

/// A world being stated, line by line.
#[derive(Clone)]
pub struct Session {
    pub world: World,
    dir: PathBuf,
    file: Option<PathBuf>,
    begun: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Op {
    Set,
    Add,
    Remove,
}

/// One bracket entry, ready to compute: `rel: values`.
struct Plan {
    rel: Sym,
    op: Op,
    values: Vec<CExpr>,
}

struct Change {
    subj: ObjId,
    rel: Sym,
    op: Op,
    values: Vec<Value>,
}

impl Session {
    /// Starts an empty world. `dir` is where the names of other worlds resolve from.
    pub fn new(name: &str, dir: &Path, file: Option<&Path>) -> Session {
        Session { world: World::new(0, name), dir: dir.into(), file: file.map(Into::into), begun: false }
    }

    /// Runs one statement and returns what it showed.
    pub fn run(&mut self, loader: &mut Loader, line: &Line) -> Result<Vec<Output>, Error> {
        let mut said = Vec::new();
        let result = parse(&line.tokens).map_err(Fail::from).and_then(|stmt| self.exec(loader, stmt, line, &mut said));
        self.begun = true;
        match result {
            Ok(()) => Ok(said.into_iter().map(|text| Output { line: line.number, source: line.text.clone(), text }).collect()),
            Err(fail) => {
                let (message, cause) = match fail {
                    Fail::Message(message) => (message, None),
                    Fail::Parent(message, cause) => (message, Some(cause)),
                };
                Err(self.error(Some(line), vec![message], cause))
            }
        }
    }

    /// Ends the world: checks every constraint, shape word and leftover disagreement.
    pub fn finish(mut self) -> Result<World, Error> {
        if !self.begun {
            // A world with no lines is still a world of its own.
            self.world.id = next_id();
        }
        match model::check(&self.world) {
            Ok(problems) if problems.is_empty() => Ok(self.world),
            Ok(problems) => Err(self.error(None, problems, None)),
            Err(message) => Err(self.error(None, vec![message], None)),
        }
    }

    fn error(&self, line: Option<&Line>, messages: Vec<String>, cause: Option<std::rc::Rc<Error>>) -> Error {
        Error {
            world: self.world.name.to_string(),
            file: self.file.clone(),
            line: line.map(|l| l.number),
            source: line.map(|l| l.text.clone()),
            messages,
            cause,
        }
    }

    fn exec(&mut self, loader: &mut Loader, stmt: Stmt, line: &Line, said: &mut Vec<String>) -> Result<(), Fail> {
        if !self.begun && !matches!(stmt, Stmt::From(_)) {
            self.world.id = next_id();
        }
        match stmt {
            Stmt::From(names) => {
                if self.begun {
                    return Err("`from` comes first, and lists every parent on one line".to_string().into());
                }
                let (mut parents, mut primitives) = (Vec::new(), Vec::new());
                for name in &names {
                    match crate::value::PRIMITIVES.iter().find(|(_, world)| world == name) {
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
                    Some(path) => {
                        let held = loader.named(&self.dir, &path)?;
                        self.world.hold(&handle, held)
                    }
                    None if is_primitive(&kind) && self.world.kinds.contains_key(&kind) => {
                        return Err(format!("{} isn't created; it is written, like `24` or `\"text\"`", a(&kind)).into());
                    }
                    None if !self.world.kinds.contains_key(&kind) => return Err(self.world.no_kind(&kind).into()),
                    None => self.world.create(&kind, &handle),
                };
                self.set(id, &values)?;
            }
            Stmt::Fact { subj, rel, value } => {
                let (subj, values) = {
                    let mut c = Compiler::new(&self.world);
                    let (subj, value) = (c.expr(&subj)?, c.expr(&value)?);
                    let subj = self.one(&c.vars, &subj)?;
                    let kind = &self.world.objects[&subj].kind;
                    self.world.relation(kind, &rel)?;
                    let values = self.values(&c.vars, &value, &mut Vec::new())?;
                    values.iter().try_for_each(|v| self.world.check_value(kind, &rel, v))?;
                    (subj, values)
                };
                values.into_iter().for_each(|v| self.world.state(subj, &rel, v));
            }
            Stmt::Set { subj, values } => {
                let subj = self.object(&subj)?;
                self.set(subj, &values)?;
            }
            Stmt::Remove { subj, rel, value } => self.remove(&subj, rel, value.as_ref(), said)?,
            Stmt::Each { kind, var, cond, action } => self.each(&kind, &var, cond.as_ref(), &action, said)?,
            Stmt::Rule { name, head, cond } => {
                let rule = compile::rule(&self.world, &name, &one_line(&line.text), &head, &cond)?;
                if let Some(problem) = model::tangle_with(&self.world, &rule) {
                    return Err(problem.into());
                }
                self.world.set_rule(rule);
                // Derive now, so a rule that can't settle fails on its own line.
                self.world.model()?;
            }
            Stmt::Never { name, kind, var, cond } => {
                if !self.world.is_thing(&kind) && self.world.kinds.contains_key(&kind) {
                    return Err(format!("a constraint is about things, not about {}", a(&kind)).into());
                }
                let never = compile::never(&self.world, &name, &one_line(&line.text), &kind, &var, &cond)?;
                self.world.set_never(never);
            }
            Stmt::RemoveRule(name) => {
                if !self.world.remove_rule(&name) {
                    said.push(format!("nothing removed: there is no rule `{name}`"));
                }
            }
            Stmt::RemoveNever(name) => {
                if !self.world.remove_never(&name) {
                    said.push(format!("nothing removed: there is no never `{name}`"));
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
    fn values(&self, vars: &[Var], e: &CExpr, env: &mut Env) -> Res<Vec<Value>> {
        if let CExpr::Const(value) = e {
            return Ok(vec![value.clone()]);
        }
        env.resize(vars.len(), None);
        let model = self.world.model()?;
        Eval { vars }.expr(e, env, &View { world: &self.world, facts: &model.facts }, true)
    }

    fn one(&self, vars: &[Var], e: &CExpr) -> Res<ObjId> {
        match self.values(vars, e, &mut Vec::new())?.as_slice() {
            [Value::Obj(id)] => Ok(*id),
            _ => Err("a fact is about one thing".into()),
        }
    }

    /// The object a reference points at.
    fn object(&self, reference: &Expr) -> Res<ObjId> {
        let mut c = Compiler::new(&self.world);
        let e = c.expr(reference)?;
        self.one(&c.vars, &e)
    }

    // --- brackets ---

    fn plan(c: &mut Compiler, kind: &Sym, assigns: &[Assign]) -> Res<Vec<Plan>> {
        let mut plans = Vec::new();
        for assign in assigns {
            c.world.relation(kind, &assign.rel)?;
            let rel = assign.rel.clone();
            match &assign.values {
                Values::Set(values) => {
                    let values = values.iter().map(|e| c.expr(e)).collect::<Res<_>>()?;
                    plans.push(Plan { rel, op: Op::Set, values });
                }
                Values::Change(changes) => {
                    for (adds, e) in changes {
                        let op = if *adds { Op::Add } else { Op::Remove };
                        plans.push(Plan { rel: rel.clone(), op, values: vec![c.expr(e)?] });
                    }
                }
            }
        }
        Ok(plans)
    }

    /// Computes what the plans change for one subject, against the world as it stands.
    fn compute(&self, vars: &[Var], plans: &[Plan], subj: ObjId, env: &mut Env) -> Res<Vec<Change>> {
        let kind = &self.world.objects[&subj].kind;
        let mut changes = Vec::new();
        for plan in plans {
            let mut values = Vec::new();
            for e in &plan.values {
                values.extend(self.values(vars, e, env)?);
            }
            if plan.op != Op::Remove {
                values.iter().try_for_each(|v| self.world.check_value(kind, &plan.rel, v))?;
            }
            changes.push(Change { subj, rel: plan.rel.clone(), op: plan.op, values });
        }
        Ok(changes)
    }

    /// Applies computed changes. Everything a set replaces goes first, so sets on the same
    /// relation add up.
    fn apply(&mut self, changes: Vec<Change>) {
        for change in changes.iter().filter(|c| c.op == Op::Set) {
            self.world.clear(change.subj, &change.rel);
        }
        for change in changes {
            for value in change.values {
                match change.op {
                    Op::Remove => drop(self.world.unstate(change.subj, &change.rel, &value)),
                    _ => self.world.state(change.subj, &change.rel, value),
                }
            }
        }
    }

    fn set(&mut self, subj: ObjId, assigns: &[Assign]) -> Res<()> {
        if assigns.is_empty() {
            return Ok(());
        }
        let changes = {
            let mut c = Compiler::new(&self.world);
            let plans = Self::plan(&mut c, &self.world.objects[&subj].kind, assigns)?;
            self.compute(&c.vars, &plans, subj, &mut Vec::new())?
        };
        self.apply(changes);
        Ok(())
    }

    // --- kinds ---

    fn kind(&mut self, name: &Sym, edits: Vec<ShapeEdit>) -> Res<()> {
        if is_primitive(name) {
            return Err(match self.world.kinds.contains_key(name) {
                true => format!("{} has no relations", a(name)),
                false => self.world.no_kind(name),
            });
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
                ShapeEdit::Declare(rel, field) => {
                    match self.world.field(name, &rel) {
                        Some(old) if *old != field || self.world.is_disputed(name, &rel) => {
                            return Err(format!("{name} already declares `{rel}: {old}`; write `+ {rel}: {field}` to change it"));
                        }
                        _ => self.world.set_field(name, &rel, field),
                    };
                }
                ShapeEdit::Set(rel, field) => self.world.set_field(name, &rel, field),
                ShapeEdit::Remove(rel) => {
                    let using = self.world.of_kind(name).filter(|id| self.world.facts.values(*id, &rel).next().is_some()).count();
                    if using > 0 {
                        let verb = if using == 1 { "has" } else { "have" };
                        return Err(format!("{} still {verb} `{rel}`", some(using, name)));
                    }
                    if let Some(rule) = self.world.rules.values().find(|r| r.rel == rel && r.vars[r.subj].kind == *name) {
                        return Err(format!("rule `{}` still produces `{rel}`", rule.name));
                    }
                    self.world.remove_field(name, &rel);
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
        for (other, kind) in &self.world.kinds {
            if let Some((rel, field)) = kind.fields.iter().find(|(_, f)| f.kind == *name)
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
        let rules = self.world.rules.values().filter(|r| r.objects().contains(&id)).map(|r| format!("rule `{}`", r.name));
        let nevers = self.world.nevers.values().filter(|n| n.objects().contains(&id)).map(|n| format!("never `{}`", n.name));
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
        let name = self.world.show(&Value::Obj(subj));
        match value {
            Some(value) => {
                let values = {
                    let mut c = Compiler::new(&self.world);
                    let value = c.expr(value)?;
                    self.values(&c.vars, &value, &mut Vec::new())?
                };
                let stated: Vec<bool> = values.iter().map(|v| self.world.unstate(subj, &rel, v)).collect();
                let model = self.world.model()?;
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
            }
            None => {
                self.world.clear(subj, &rel);
                self.still_follows(subj, &rel, said)?;
            }
        }
        Ok(())
    }

    // --- each ---

    fn each(&mut self, kind: &Sym, var: &Sym, cond: Option<&Cond>, action: &EachAction, said: &mut Vec<String>) -> Res<()> {
        if is_primitive(kind) && self.world.kinds.contains_key(kind) {
            return Err(format!("`each` goes through things, and there is no end of {}", crate::value::plural(kind)));
        }
        let mut changes = Vec::new();
        let mut doomed = BTreeSet::new();
        {
            let mut c = Compiler::new(&self.world);
            let subj = c.open(&[(kind, var)], &cond.into_iter().collect::<Vec<_>>())?[0];
            let cond = match cond {
                Some(cond) => c.cond(cond)?,
                None => CCond::And(Vec::new()),
            };
            // `None` removes the objects themselves.
            let plans = match action {
                EachAction::Set(assigns) => Some(Self::plan(&mut c, kind, assigns)?),
                EachAction::Fact(rel, value) => {
                    self.world.relation(kind, rel)?;
                    Some(vec![Plan { rel: rel.clone(), op: Op::Add, values: vec![c.expr(value)?] }])
                }
                EachAction::RemoveObjects => None,
                EachAction::RemoveRel(rel, _) if self.world.field(kind, rel).is_none() => {
                    said.push(format!("nothing removed: {kind} has no relation `{rel}`"));
                    return Ok(());
                }
                EachAction::RemoveRel(rel, None) => Some(vec![Plan { rel: rel.clone(), op: Op::Set, values: Vec::new() }]),
                EachAction::RemoveRel(rel, Some(value)) => {
                    Some(vec![Plan { rel: rel.clone(), op: Op::Remove, values: vec![c.expr(value)?] }])
                }
            };
            let mut needs = vec![subj];
            for plan in plans.iter().flatten() {
                plan.values.iter().for_each(|e| expr_slots(e, &mut needs));
            }
            let model = self.world.model()?;
            let view = View { world: &self.world, facts: &model.facts };
            let eval = Eval { vars: &c.vars };
            let mut matches = BTreeSet::new();
            eval.solve(&cond, &mut vec![None; c.vars.len()], &view, &mut |env| {
                eval.ground(&needs, env, &view, &mut |env| {
                    matches.insert(env.clone());
                    Ok(true)
                })
            })?;
            // Every value is computed here, before anything changes.
            for mut env in matches {
                let Some(Value::Obj(id)) = env[subj] else { continue };
                match &plans {
                    Some(plans) => changes.extend(self.compute(&c.vars, plans, id, &mut env)?),
                    None => drop(doomed.insert(id)),
                }
            }
        }
        doomed.into_iter().for_each(|id| self.remove_object(id, said));
        let emptied: BTreeSet<(ObjId, Sym)> = match action {
            EachAction::RemoveRel(..) => changes.iter().map(|c| (c.subj, c.rel.clone())).collect(),
            _ => BTreeSet::new(),
        };
        self.apply(changes);
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
            return Ok(list(&self.values(&c.vars, &value, &mut Vec::new())?));
        }
        let asked = c.open(&[], &[question])?;
        let cond = c.cond(question)?;
        let model = self.world.model()?;
        let view = View { world: &self.world, facts: &model.facts };
        let eval = Eval { vars: &c.vars };
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

//! What follows: D(F, R), and what must hold when the world ends.

use crate::ast::{AggOp, How};
use crate::error::Res;
use crate::eval::{Eval, View};
use crate::logic::{CCond, CExpr, Rule};
use crate::value::{ObjId, Sym, Value, a};
use crate::world::{Facts, World};
use std::collections::{BTreeSet, HashMap};

/// M = F ∪ D: every fact that holds, and for the derived ones, a rule that produced them.
pub struct Model {
    pub facts: Facts,
    pub why: HashMap<(ObjId, Sym, Value), Sym>,
    /// Rules that depend on their own negation or aggregate. They can only arrive by merging,
    /// produce nothing, and must be settled before the world ends.
    pub tangled: Vec<String>,
}

/// Rules over existing objects always settle. Rules that compute their value can go on
/// forever (`p age p.age + 1`); after this many rounds they are taken to.
const ROUNDS: usize = 1_000;
const DERIVED: usize = 20_000;
const NEVER_SETTLES: &str = "never settles: it keeps producing new values";

/// How a rule uses a fact it reads.
#[derive(Clone, Copy, PartialEq)]
enum Use {
    /// More facts can only make more follow.
    Holds,
    Negated,
    /// Under an aggregate or a description; the word says which, as in "counts".
    Gathered(&'static str),
}

struct Read {
    rel: Sym,
    /// The value, when the rule names it: `p is adult` reads only `is adult`.
    value: Option<Value>,
    how: Use,
}

fn cond_reads(c: &CCond, how: Use, out: &mut Vec<Read>) {
    match c {
        CCond::And(cs) | CCond::Or(cs) => cs.iter().for_each(|c| cond_reads(c, how, out)),
        CCond::Not(scope) => cond_reads(&scope.cond, if how == Use::Holds { Use::Negated } else { how }, out),
        // What holds inside a held world never changes.
        CCond::In(world, _) => expr_reads(world, how, out),
        CCond::Any(_) => {}
        CCond::Pattern { subj, rel, val, .. } => {
            let value = match val {
                CExpr::Const(value) => Some(value.clone()),
                _ => None,
            };
            out.push(Read { rel: rel.clone(), value, how });
            expr_reads(subj, how, out);
            expr_reads(val, how, out);
        }
        CCond::Cmp { op, left, right, .. } => {
            // `!=` stops holding when an equal value appears, as `not` does.
            let how = if *op == crate::ast::CmpOp::Ne && how == Use::Holds { Use::Negated } else { how };
            expr_reads(left, how, out);
            expr_reads(right, how, out);
        }
    }
}

fn expr_reads(e: &CExpr, how: Use, out: &mut Vec<Read>) {
    let gathered = |word| if how == Use::Holds { Use::Gathered(word) } else { how };
    match e {
        CExpr::Const(_) | CExpr::Var(_) | CExpr::Handle(_) | CExpr::Index(_) => {}
        CExpr::The(the) => {
            for (rel, described) in &the.desc {
                out.push(Read { rel: rel.clone(), value: None, how: gathered("picks from") });
                described.iter().for_each(|e| expr_reads(e, gathered("picks from"), out));
            }
        }
        CExpr::Dot(base, rel) => {
            out.push(Read { rel: rel.clone(), value: None, how });
            expr_reads(base, how, out);
        }
        CExpr::Neg(e) => expr_reads(e, how, out),
        CExpr::Bin(_, l, r) => {
            expr_reads(l, how, out);
            expr_reads(r, how, out);
        }
        CExpr::Count(e) => expr_reads(e, gathered("counts"), out),
        CExpr::CountCond(scope) => cond_reads(&scope.cond, gathered("counts"), out),
        CExpr::Agg(op, e) => {
            let word = match op {
                AggOp::Sum => "sums",
                AggOp::Min => "takes the min of",
                AggOp::Max => "takes the max of",
            };
            expr_reads(e, gathered(word), out);
        }
        CExpr::In(world, _) => expr_reads(world, how, out),
    }
}

/// For each rule, the rules whose facts it reads, and how.
fn dependencies(rules: &[&Rule]) -> Vec<Vec<(usize, Use)>> {
    let heads: Vec<Option<&Value>> = rules.iter().map(|r| if let CExpr::Const(v) = &r.val { Some(v) } else { None }).collect();
    rules
        .iter()
        .map(|rule| {
            let mut reads = Vec::new();
            cond_reads(&rule.cond, Use::Holds, &mut reads);
            expr_reads(&rule.val, Use::Holds, &mut reads);
            let mut edges = Vec::new();
            for read in &reads {
                for (j, other) in rules.iter().enumerate() {
                    let overlaps = other.rel == read.rel && (read.value.is_none() || heads[j].is_none() || read.value.as_ref() == heads[j]);
                    if overlaps && !edges.contains(&(j, read.how)) {
                        edges.push((j, read.how));
                    }
                }
            }
            edges
        })
        .collect()
}

/// Groups of rules that depend on each other, each group after the groups it reads from.
fn components(edges: &[Vec<(usize, Use)>]) -> Vec<Vec<usize>> {
    struct Walk<'a> {
        edges: &'a [Vec<(usize, Use)>],
        index: Vec<Option<usize>>,
        low: Vec<usize>,
        stack: Vec<usize>,
        on_stack: Vec<bool>,
        out: Vec<Vec<usize>>,
        next: usize,
    }
    fn visit(w: &mut Walk, v: usize) {
        w.index[v] = Some(w.next);
        w.low[v] = w.next;
        w.next += 1;
        w.stack.push(v);
        w.on_stack[v] = true;
        let edges = w.edges;
        for &(to, _) in &edges[v] {
            match w.index[to] {
                None => {
                    visit(w, to);
                    w.low[v] = w.low[v].min(w.low[to]);
                }
                Some(index) if w.on_stack[to] => w.low[v] = w.low[v].min(index),
                Some(_) => {}
            }
        }
        if w.index[v] == Some(w.low[v]) {
            let at = w.stack.iter().rposition(|&x| x == v).unwrap();
            let group = w.stack.split_off(at);
            group.iter().for_each(|&x| w.on_stack[x] = false);
            w.out.push(group);
        }
    }
    let n = edges.len();
    let mut walk =
        Walk { edges, index: vec![None; n], low: vec![0; n], stack: Vec::new(), on_stack: vec![false; n], out: Vec::new(), next: 0 };
    for v in 0..n {
        if walk.index[v].is_none() {
            visit(&mut walk, v);
        }
    }
    walk.out
}

/// What is wrong with a group of rules, if one of them reads the group's own facts under `not`
/// or an aggregate: neither has a well-defined answer.
fn tangle(rules: &[&Rule], edges: &[Vec<(usize, Use)>], group: &[usize]) -> Option<String> {
    for &i in group {
        for &(j, how) in &edges[i] {
            let what = match how {
                Use::Holds => continue,
                _ if !group.contains(&j) => continue,
                Use::Negated => "depends on its own negation".to_string(),
                Use::Gathered(word) => format!("{word} itself"),
            };
            let through = if i == j { String::new() } else { format!(" through rule `{}`", rules[j].name) };
            return Some(format!("rule `{}` {what}{through}", rules[i].name));
        }
    }
    None
}

/// Whether adding this rule (in place of any with its name) would tangle the rules.
pub fn tangle_with(world: &World, rule: &Rule) -> Option<String> {
    let mut rules: Vec<&Rule> = world.active_rules().map(|r| &**r).filter(|r| r.name != rule.name).collect();
    rules.push(rule);
    let edges = dependencies(&rules);
    let new = rules.len() - 1;
    let group = components(&edges).into_iter().find(|group| group.contains(&new))?;
    // Name the rule being stated first.
    let group: Vec<usize> = [new].into_iter().chain(group.into_iter().filter(|i| *i != new)).collect();
    tangle(&rules, &edges, &group)
}

/// Computes everything the rules make follow from the stated facts.
pub fn derive(world: &World) -> Res<Model> {
    let rules: Vec<&Rule> = world.active_rules().map(|r| &**r).collect();
    let edges = dependencies(&rules);
    let mut model = Model { facts: world.facts.clone(), why: HashMap::new(), tangled: Vec::new() };
    for group in components(&edges) {
        if let Some(problem) = tangle(&rules, &edges, &group) {
            model.tangled.push(problem);
            continue;
        }
        let recursive = group.len() > 1 || edges[group[0]].iter().any(|(j, _)| *j == group[0]);
        let computes = group.iter().any(|i| !matches!(rules[*i].val, CExpr::Var(_) | CExpr::Const(_)));
        for round in 1.. {
            let mut produced = Vec::new();
            for &i in &group {
                fire(world, &model.facts, rules[i], &mut produced)
                    .map_err(|e| format!("rule `{}`{}{e}", rules[i].name, if e == NEVER_SETTLES { " " } else { ": " }))?;
            }
            let before = model.facts.len();
            for (rule, subj, value) in produced {
                if model.facts.insert(subj, &rule.rel, value.clone()) {
                    model.why.insert((subj, rule.rel.clone(), value), rule.name.clone());
                }
            }
            if model.facts.len() == before || !recursive {
                break;
            }
            if computes && (round == ROUNDS || model.facts.len() - world.facts.len() > DERIVED) {
                return Err(format!("rule `{}` {NEVER_SETTLES}", rules[group[0]].name));
            }
        }
    }
    Ok(model)
}

/// Every fact one rule produces from the facts so far.
fn fire<'r>(world: &World, facts: &Facts, rule: &'r Rule, out: &mut Vec<(&'r Rule, ObjId, Value)>) -> Res<()> {
    let eval = Eval { vars: &rule.vars };
    let view = View { world, facts };
    let head = rule.head_slots();
    let kind = &rule.vars[rule.subj].kind;
    let mut env = vec![None; rule.vars.len()];
    eval.solve(&rule.cond, &mut env, &view, &mut |env| {
        eval.ground(&head, env, &view, &mut |env| {
            // Only things of this world have facts here.
            let Some(Value::Obj(subj)) = env[rule.subj].clone() else { return Ok(true) };
            if !world.fits(kind, &Value::Obj(subj)) {
                return Ok(true);
            }
            for value in eval.expr(&rule.val, env, &view, false)? {
                // A rule that names an object since removed produces nothing with it.
                let gone = matches!(&value, Value::Obj(id) if !world.objects.contains_key(id));
                if !gone && !facts.contains(subj, &rule.rel, &value) {
                    world.check_value(kind, &rule.rel, &value)?;
                    out.push((rule, subj, value));
                    if out.len() > DERIVED {
                        return Err(NEVER_SETTLES.into());
                    }
                }
            }
            Ok(true)
        })
    })?;
    Ok(())
}

/// Everything that doesn't fit when the world ends: disagreements left by merging, relations
/// holding the wrong number or kind of values, and constraints with matches.
pub fn check(world: &World) -> Res<Vec<String>> {
    let mut problems: Vec<String> = world.disputes.values().cloned().collect();
    let model = world.model()?;
    problems.extend(model.tangled.iter().map(|t| format!("{t}; restate it")));
    for (subj, rel, value) in model.facts.iter() {
        let Some(object) = world.objects.get(&subj) else { continue };
        let (name, kind) = (world.show(&Value::Obj(subj)), &object.kind);
        match world.field(kind, rel) {
            None => problems.push(format!("{name} has a `{rel}`, but {kind} has no relation `{rel}`")),
            Some(field) if !world.is_disputed(kind, rel) && !world.fits(&field.kind, value) => {
                let given = if matches!(value, Value::Obj(id) if !world.objects.contains_key(id)) {
                    "something that is not in this world".into()
                } else {
                    format!("{}, {}", a(world.kind_of(value)), world.show(value))
                };
                problems.push(format!("{name} has {given} as `{rel}` ({kind}: `{rel}: {field}`)"));
            }
            Some(_) => {}
        }
    }
    for (id, object) in &world.objects {
        let Some(kind) = world.kinds.get(&object.kind) else { continue };
        for (rel, field) in &kind.fields {
            let n = model.facts.values(*id, rel).count();
            let broken = match field.how {
                How::One => n != 1,
                How::Maybe => n > 1,
                How::Some => n == 0,
                How::Many => false,
            };
            if broken && !world.is_disputed(&object.kind, rel) {
                let (name, has) = (world.show(&Value::Obj(*id)), if n == 0 { "no".into() } else { n.to_string() });
                problems.push(format!("{name} has {has} `{rel}` ({}: `{rel}: {field}`)", object.kind));
            }
        }
    }
    let view = View { world, facts: &model.facts };
    for never in world.active_nevers() {
        let eval = Eval { vars: &never.vars };
        let mut breaking = BTreeSet::new();
        let mut env = vec![None; never.vars.len()];
        let found = eval.solve(&never.cond, &mut env, &view, &mut |env| {
            eval.ground(&[never.subj], env, &view, &mut |env| {
                breaking.extend(env[never.subj].clone());
                Ok(true)
            })
        });
        found.map_err(|e| format!("never `{}`: {e}", never.name))?;
        problems.extend(breaking.iter().map(|value| format!("{} breaks `{}` ({})", world.show(value), never.name, never.source)));
    }
    Ok(problems)
}

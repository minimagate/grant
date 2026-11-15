//! A world written back as Grant: what has been stated in it, and nothing else.

use crate::value::{ObjId, PRIMITIVES, Sym, Value, is_primitive, some};
use crate::world::World;
use std::collections::{BTreeMap, BTreeSet};

/// The world as one flat list of statements: its parents are folded in, and derived facts are
/// left to its rules.
pub fn source(world: &World) -> String {
    let mut lines: Vec<String> = Vec::new();
    let primitives: Vec<&str> = PRIMITIVES.iter().filter(|(kind, _)| world.kinds.contains_key(*kind)).map(|(_, w)| *w).collect();
    if !primitives.is_empty() {
        lines.push(format!("from {}\n", primitives.join(", ")));
    }
    // Objects of different kinds sharing a handle are written `kind@handle`; only objects of one
    // kind sharing it can't be told apart in a listing.
    let shared = |o: &crate::world::Object| world.find(&o.handle).iter().filter(|id| world.objects[id].kind == o.kind).count() > 1;
    if world.objects.values().any(shared) {
        lines.push("-- some handles are shared by several objects of one kind, so this listing can't be run as it is\n".into());
    }

    // A kind is declared before anything mentions it: one that an earlier kind's shape names is
    // declared bare first, and its own shape is added where it comes.
    let kinds: Vec<(&Sym, _)> = world.kinds.iter().filter(|(name, _)| !is_primitive(name) && &***name != "world").collect();
    let mut early = BTreeSet::new();
    for (i, (name, shape)) in kinds.iter().enumerate() {
        let later = |k: &Sym| k != *name && kinds[i + 1..].iter().any(|(other, _)| *other == k);
        early.extend(shape.values().filter(|field| field.by.is_none()).map(|field| &field.kind).filter(|k| later(k)));
    }
    lines.extend(early.iter().map(|name| format!("kind {name}")));
    for (name, shape) in world.kinds.iter().filter(|(name, _)| !is_primitive(name)) {
        let plus = if &**name == "world" || early.contains(name) { "+ " } else { "" };
        let stated = shape.iter().filter(|(_, field)| field.by.is_none());
        let fields: Vec<String> = stated.map(|(rel, field)| format!("{plus}{rel}: {field}")).collect();
        match fields.as_slice() {
            [] if &**name == "world" || early.contains(name) => {}
            [] => lines.push(format!("kind {name}")),
            [field] => lines.push(format!("kind {name} [ {field} ]")),
            _ => lines.push(format!("kind {name} [\n  {}\n]", fields.join("\n  "))),
        }
    }
    // A relation read with `by` comes once the relation it reads is declared: after every shape.
    for (name, shape) in &world.kinds {
        let read: Vec<String> = shape.iter().filter(|(_, f)| f.by.is_some()).map(|(rel, field)| format!("{rel}: {field}")).collect();
        if !read.is_empty() {
            lines.push(format!("kind {name} [ {} ]", read.join(" | ")));
        }
    }
    lines.push(String::new());

    // An object is stated with the facts whose values exist by then; the rest follow.
    let mut stated: BTreeSet<ObjId> = BTreeSet::new();
    let mut later = Vec::new();
    let mut facts: BTreeMap<ObjId, Vec<(&Sym, &Value)>> = BTreeMap::new();
    world.facts.iter().for_each(|(subj, rel, value)| facts.entry(subj).or_default().push((rel, value)));
    for (id, object) in &world.objects {
        stated.insert(*id);
        let mut values: BTreeMap<&Sym, Vec<String>> = BTreeMap::new();
        for (rel, value) in facts.remove(id).unwrap_or_default() {
            match value {
                Value::Obj(other) if !stated.contains(other) => {
                    later.push(format!("{} {rel} {}", world.show(&Value::Obj(*id)), world.show(value)))
                }
                _ => values.entry(rel).or_default().push(world.show(value)),
            }
        }
        let brackets: Vec<String> = values.iter().map(|(rel, vs)| format!("{rel}: {}", vs.join(", "))).collect();
        let brackets = if brackets.is_empty() { String::new() } else { format!(" [ {} ]", brackets.join(" | ")) };
        lines.push(format!("{} {}{brackets}", object.kind, object.handle));
    }
    lines.append(&mut later);
    lines.push(String::new());

    lines.extend(world.rules.values().map(|r| r.source.clone()));
    lines.extend(world.nevers.values().map(|n| n.source.clone()));

    let mut text = lines.join("\n");
    while text.contains("\n\n\n") {
        text = text.replace("\n\n\n", "\n\n");
    }
    format!("{}\n", text.trim())
}

/// One line that sums a world up.
pub fn summary(world: &World) -> String {
    let kinds = world.kinds.keys().filter(|k| !is_primitive(k) && &***k != "world").count();
    let counts = [
        (kinds, "kind"),
        (world.objects.len(), "object"),
        (world.facts.len(), "fact"),
        (world.rules.len(), "rule"),
        (world.nevers.len(), "constraint"),
    ];
    counts.map(|(n, noun)| some(n, noun)).join(", ")
}

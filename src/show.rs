//! A world written back as Grant: what has been stated in it, and nothing else.

use crate::value::{ObjId, PRIMITIVES, Sym, Value, is_primitive};
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
    if world.objects.values().any(|o| world.find(&o.handle).len() > 1) {
        lines.push("-- some handles are shared by several objects, so this listing can't be run as it is\n".into());
    }

    for (name, shape) in world.kinds.iter().filter(|(name, _)| !is_primitive(name)) {
        let plus = if &**name == "world" { "+ " } else { "" };
        let fields: Vec<String> = shape.iter().map(|(rel, field)| format!("{plus}{rel}: {field}")).collect();
        match fields.as_slice() {
            [] if &**name == "world" => {}
            [] => lines.push(format!("kind {name}")),
            [field] => lines.push(format!("kind {name} [ {field} ]")),
            _ => lines.push(format!("kind {name} [\n  {}\n]", fields.join("\n  "))),
        }
    }
    lines.push(String::new());

    // An object is stated with the facts whose values exist by then; the rest follow.
    let mut stated: BTreeSet<ObjId> = BTreeSet::new();
    let mut later = Vec::new();
    for (id, object) in &world.objects {
        stated.insert(*id);
        let mut values: BTreeMap<&Sym, Vec<String>> = BTreeMap::new();
        for (_, rel, value) in world.facts.iter().filter(|(subj, ..)| subj == id) {
            match value {
                Value::Obj(other) if !stated.contains(other) => later.push(format!("{} {rel} {}", object.handle, world.show(value))),
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
    let count = |n: usize, noun: &str| format!("{n} {noun}{}", if n == 1 { "" } else { "s" });
    [
        count(kinds, "kind"),
        count(world.objects.len(), "object"),
        count(world.facts.len(), "fact"),
        count(world.rules.len(), "rule"),
        count(world.nevers.len(), "constraint"),
    ]
    .join(", ")
}

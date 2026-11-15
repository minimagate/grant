//! The world structure: W = (K, O, F, R).

use crate::ast::Field;
use crate::error::Res;
use crate::logic::{Never, Rule};
use crate::model::{self, Model};
use crate::value::{ObjId, PRIMITIVES, Sym, Value, a, is_primitive};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

/// A kind's shape: every relation its objects can have.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Kind {
    pub fields: BTreeMap<Sym, Field>,
}

#[derive(Clone)]
pub struct Object {
    pub kind: Sym,
    pub handle: Sym,
    pub index: u64,
    /// The world that created it.
    pub made_in: Sym,
    /// For an object of kind `world`: the world it is.
    pub held: Option<Rc<World>>,
}

/// A set of facts, indexed from both ends.
#[derive(Clone, Default, PartialEq)]
pub struct Facts {
    rels: BTreeMap<Sym, Rel>,
    len: usize,
}

#[derive(Clone, Default, PartialEq)]
struct Rel {
    by_subject: BTreeMap<ObjId, BTreeSet<Value>>,
    by_value: BTreeMap<Value, BTreeSet<ObjId>>,
}

impl Facts {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Adds a fact; `false` if it was already there.
    pub fn insert(&mut self, subj: ObjId, rel: &Sym, value: Value) -> bool {
        let r = self.rels.entry(rel.clone()).or_default();
        if !r.by_subject.entry(subj).or_default().insert(value.clone()) {
            return false;
        }
        r.by_value.entry(value).or_default().insert(subj);
        self.len += 1;
        true
    }

    /// Removes a fact; `false` if it wasn't there.
    pub fn remove(&mut self, subj: ObjId, rel: &str, value: &Value) -> bool {
        let Some(r) = self.rels.get_mut(rel) else { return false };
        let Some(values) = r.by_subject.get_mut(&subj) else { return false };
        if !values.remove(value) {
            return false;
        }
        if values.is_empty() {
            r.by_subject.remove(&subj);
        }
        if let Some(subjects) = r.by_value.get_mut(value) {
            subjects.remove(&subj);
            if subjects.is_empty() {
                r.by_value.remove(value);
            }
        }
        self.len -= 1;
        true
    }

    pub fn contains(&self, subj: ObjId, rel: &str, value: &Value) -> bool {
        self.rels.get(rel).and_then(|r| r.by_subject.get(&subj)).is_some_and(|values| values.contains(value))
    }

    pub fn values(&self, subj: ObjId, rel: &str) -> impl Iterator<Item = &Value> {
        self.rels.get(rel).and_then(|r| r.by_subject.get(&subj)).into_iter().flatten()
    }

    pub fn subjects(&self, rel: &str, value: &Value) -> impl Iterator<Item = ObjId> {
        self.rels.get(rel).and_then(|r| r.by_value.get(value)).into_iter().flatten().copied()
    }

    pub fn pairs(&self, rel: &str) -> impl Iterator<Item = (ObjId, &Value)> {
        let by_subject = self.rels.get(rel).map(|r| &r.by_subject);
        by_subject.into_iter().flatten().flat_map(|(subj, values)| values.iter().map(|v| (*subj, v)))
    }

    /// Every fact with this subject.
    pub fn of(&self, subj: ObjId) -> impl Iterator<Item = (&Sym, &Value)> {
        self.rels.iter().flat_map(move |(rel, r)| r.by_subject.get(&subj).into_iter().flatten().map(move |v| (rel, v)))
    }

    pub fn iter(&self) -> impl Iterator<Item = (ObjId, &Sym, &Value)> {
        self.rels.iter().flat_map(|(rel, r)| r.by_subject.iter().flat_map(move |(s, vs)| vs.iter().map(move |v| (*s, rel, v))))
    }

    /// Removes every value of one relation of one subject, and returns them.
    pub fn clear(&mut self, subj: ObjId, rel: &str) -> Vec<Value> {
        let values: Vec<Value> = self.values(subj, rel).cloned().collect();
        for value in &values {
            self.remove(subj, rel, value);
        }
        values
    }

    /// Removes every fact with this object as subject or value.
    fn forget(&mut self, id: ObjId) {
        let object = Value::Obj(id);
        let touching: Vec<_> =
            self.iter().filter(|(s, _, v)| *s == id || **v == object).map(|(s, r, v)| (s, r.clone(), v.clone())).collect();
        for (subj, rel, value) in touching {
            self.remove(subj, &rel, &value);
        }
    }
}

/// Something parents disagree on, which the child must settle before it ends.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Dispute {
    Field(Sym, Sym),
    Rule(Sym),
    Never(Sym),
}

#[derive(Clone)]
pub struct World {
    pub id: u32,
    pub name: Sym,
    pub kinds: BTreeMap<Sym, Kind>,
    pub objects: BTreeMap<ObjId, Object>,
    /// The stated facts, F. What rules add is in `model()`.
    pub facts: Facts,
    pub rules: BTreeMap<Sym, Rc<Rule>>,
    pub nevers: BTreeMap<Sym, Rc<Never>>,
    /// What is still disputed, with the sentence that names it.
    pub disputes: BTreeMap<Dispute, String>,
    by_handle: BTreeMap<Sym, BTreeSet<ObjId>>,
    by_kind: BTreeMap<Sym, BTreeSet<ObjId>>,
    /// The last index given out for each kind. Indexes are never reused.
    indexes: BTreeMap<Sym, u64>,
    /// How many objects this world itself has created.
    created: u32,
    /// Handles of removed objects, and the world that removed them.
    removed: BTreeMap<Sym, Sym>,
    model: RefCell<Option<Rc<Model>>>,
}

/// A number no other world has. It stands for the world in the identity of the objects it
/// creates, and is given once the world's parents are known, so parents come first in order.
pub fn next_id() -> u32 {
    static NEXT: AtomicU32 = AtomicU32::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

impl World {
    /// The empty world: nothing but the kind `world`.
    pub fn new(id: u32, name: &str) -> World {
        World {
            id,
            name: name.into(),
            kinds: BTreeMap::from([("world".into(), Kind::default())]),
            objects: BTreeMap::new(),
            facts: Facts::default(),
            rules: BTreeMap::new(),
            nevers: BTreeMap::new(),
            disputes: BTreeMap::new(),
            by_handle: BTreeMap::new(),
            by_kind: BTreeMap::new(),
            indexes: BTreeMap::new(),
            created: 0,
            removed: BTreeMap::new(),
            model: RefCell::new(None),
        }
    }

    /// The union of the parents: everything any of them has. What they disagree on is recorded
    /// as disputes rather than refused.
    pub fn merge(id: u32, name: &str, parents: &[Rc<World>]) -> World {
        let Some((first, rest)) = parents.split_first() else { return World::new(id, name) };
        let mut world = World { id, name: name.into(), created: 0, ..first.as_ref().clone() };
        for (n, parent) in rest.iter().enumerate() {
            let earlier = parents[..=n].iter().map(|p| &*p.name).collect::<Vec<_>>().join(", ");
            for (kind_name, kind) in &parent.kinds {
                let ours = world.kinds.entry(kind_name.clone()).or_default();
                for (rel, field) in &kind.fields {
                    match ours.fields.get(rel) {
                        None => {
                            ours.fields.insert(rel.clone(), field.clone());
                        }
                        Some(old) if old != field => {
                            let message = format!(
                                "`{rel}` of {kind_name} is declared two ways (`{rel}: {old}` in {earlier}, `{rel}: {field}` in {}); restate it with `kind {kind_name} [ + {rel}: … ]`",
                                parent.name
                            );
                            world.disputes.entry(Dispute::Field(kind_name.clone(), rel.clone())).or_insert(message);
                        }
                        Some(_) => {}
                    }
                }
            }
            for (id, object) in &parent.objects {
                if !world.objects.contains_key(id) {
                    world.insert(*id, object.clone());
                }
            }
            for (subj, rel, value) in parent.facts.iter() {
                world.facts.insert(subj, rel, value.clone());
            }
            for (rule_name, rule) in &parent.rules {
                match world.rules.get(rule_name) {
                    None => {
                        world.rules.insert(rule_name.clone(), rule.clone());
                    }
                    Some(old) if old != rule => {
                        let message = format!("rule `{rule_name}` differs between {earlier} and {}; restate it", parent.name);
                        world.disputes.entry(Dispute::Rule(rule_name.clone())).or_insert(message);
                    }
                    Some(_) => {}
                }
            }
            for (never_name, never) in &parent.nevers {
                match world.nevers.get(never_name) {
                    None => {
                        world.nevers.insert(never_name.clone(), never.clone());
                    }
                    Some(old) if old != never => {
                        let message = format!("never `{never_name}` differs between {earlier} and {}; restate it", parent.name);
                        world.disputes.entry(Dispute::Never(never_name.clone())).or_insert(message);
                    }
                    Some(_) => {}
                }
            }
            for (kind, index) in &parent.indexes {
                let ours = world.indexes.entry(kind.clone()).or_default();
                *ours = (*ours).max(*index);
            }
            world.removed.extend(parent.removed.iter().map(|(h, w)| (h.clone(), w.clone())));
        }
        world.touch();
        world
    }

    /// Forgets the derived facts. Every change to the world must end here.
    fn touch(&self) {
        *self.model.borrow_mut() = None;
    }

    /// F ∪ D: the stated facts and everything the rules make follow from them.
    pub fn model(&self) -> Res<Rc<Model>> {
        if let Some(model) = &*self.model.borrow() {
            return Ok(model.clone());
        }
        let model = Rc::new(model::derive(self)?);
        *self.model.borrow_mut() = Some(model.clone());
        Ok(model)
    }

    // --- kinds ---

    /// Whether objects of this kind are made by statements (not numbers, text or indexes).
    pub fn is_thing(&self, kind: &str) -> bool {
        self.kinds.contains_key(kind) && !is_primitive(kind)
    }

    /// What Grant says when a kind is missing.
    pub fn no_kind(&self, kind: &str) -> String {
        match PRIMITIVES.iter().find(|(k, _)| *k == kind) {
            Some((_, world)) => format!("this world has no {kind}; add `from {world}`"),
            None => format!("kind `{kind}` doesn't exist yet"),
        }
    }

    /// Checks that a primitive kind was brought in with `from`.
    pub fn need(&self, primitive: &str) -> Res<()> {
        if self.kinds.contains_key(primitive) { Ok(()) } else { Err(self.no_kind(primitive)) }
    }

    pub fn declare_kind(&mut self, kind: &Sym) {
        self.kinds.entry(kind.clone()).or_default();
        self.touch();
    }

    pub fn remove_kind(&mut self, kind: &str) {
        self.kinds.remove(kind);
        self.disputes.retain(|d, _| !matches!(d, Dispute::Field(k, _) if &**k == kind));
        self.touch();
    }

    /// The declaration of `kind.rel`, if the shape has it.
    pub fn field(&self, kind: &str, rel: &str) -> Option<&Field> {
        self.kinds.get(kind)?.fields.get(rel)
    }

    /// The declaration of `kind.rel`, or what Grant says when the shape lacks it.
    pub fn relation(&self, kind: &str, rel: &str) -> Res<&Field> {
        self.field(kind, rel).ok_or_else(|| format!("{kind} has no relation `{rel}`"))
    }

    pub fn is_disputed(&self, kind: &Sym, rel: &Sym) -> bool {
        self.disputes.contains_key(&Dispute::Field(kind.clone(), rel.clone()))
    }

    /// Declares a relation or replaces its declaration, which settles any dispute over it.
    pub fn set_field(&mut self, kind: &Sym, rel: &Sym, field: Field) {
        self.kinds.entry(kind.clone()).or_default().fields.insert(rel.clone(), field);
        self.disputes.remove(&Dispute::Field(kind.clone(), rel.clone()));
        self.touch();
    }

    pub fn remove_field(&mut self, kind: &Sym, rel: &Sym) {
        if let Some(k) = self.kinds.get_mut(kind) {
            k.fields.remove(rel);
        }
        self.disputes.remove(&Dispute::Field(kind.clone(), rel.clone()));
        self.touch();
    }

    /// Whether a value can stand where this kind is expected.
    pub fn fits(&self, kind: &str, value: &Value) -> bool {
        match value {
            Value::Num(n) => kind == "number" || (kind == "id" && n.as_index().is_some()),
            Value::Text(_) => kind == "text",
            Value::Obj(id) => self.objects.get(id).is_some_and(|o| &*o.kind == kind),
        }
    }

    pub fn kind_of(&self, value: &Value) -> &str {
        match value {
            Value::Num(_) => "number",
            Value::Text(_) => "text",
            Value::Obj(id) => self.objects.get(id).map_or("thing", |o| &o.kind),
        }
    }

    /// Checks a value against the shape before it is stated.
    pub fn check_value(&self, kind: &Sym, rel: &Sym, value: &Value) -> Res<()> {
        let field = self.relation(kind, rel)?;
        if self.is_disputed(kind, rel) || self.fits(&field.kind, value) {
            return Ok(());
        }
        Err(format!("`{rel}` takes {}, not {}", a(&field.kind), a(self.kind_of(value))))
    }

    // --- objects ---

    /// The objects with this handle.
    pub fn find(&self, handle: &str) -> Vec<ObjId> {
        self.by_handle.get(handle).into_iter().flatten().copied().collect()
    }

    /// The one object with this handle, or what Grant says when there isn't exactly one.
    pub fn resolve(&self, handle: &str) -> Res<ObjId> {
        match self.find(handle).as_slice() {
            [id] => Ok(*id),
            [] => Err(match self.removed.get(handle) {
                Some(world) => format!("{handle} doesn't exist (removed in {world})"),
                None => format!("`{handle}` doesn't exist yet"),
            }),
            several => Err(format!("{} objects called {handle}; use `the`", several.len())),
        }
    }

    pub fn of_kind(&self, kind: &str) -> impl Iterator<Item = ObjId> {
        self.by_kind.get(kind).into_iter().flatten().copied()
    }

    /// The worlds this world holds.
    pub fn held(&self) -> impl Iterator<Item = &Rc<World>> {
        self.of_kind("world").filter_map(|id| self.objects[&id].held.as_ref())
    }

    fn insert(&mut self, id: ObjId, object: Object) {
        self.by_handle.entry(object.handle.clone()).or_default().insert(id);
        self.by_kind.entry(object.kind.clone()).or_default().insert(id);
        self.objects.insert(id, object);
    }

    fn number(&mut self, kind: &Sym) -> u64 {
        let index = self.indexes.entry(kind.clone()).or_default();
        *index += 1;
        *index
    }

    /// Creates an object here: its origin is this world and its place in this world's creations.
    pub fn create(&mut self, kind: &Sym, handle: &Sym) -> ObjId {
        self.created += 1;
        let id = ObjId { world: self.id, seq: self.created };
        let index = self.number(kind);
        self.insert(id, Object { kind: kind.clone(), handle: handle.clone(), index, made_in: self.name.clone(), held: None });
        self.touch();
        id
    }

    /// Holds a world as an object. A world is its own origin, so holding it twice holds it once.
    pub fn hold(&mut self, handle: &Sym, world: Rc<World>) -> ObjId {
        let id = ObjId { world: world.id, seq: 0 };
        if !self.objects.contains_key(&id) {
            let kind: Sym = "world".into();
            let index = self.number(&kind);
            self.insert(id, Object { kind, handle: handle.clone(), index, made_in: self.name.clone(), held: Some(world) });
            self.touch();
        }
        id
    }

    /// Removes an object and every fact touching it.
    pub fn remove_object(&mut self, id: ObjId) {
        let Some(object) = self.objects.remove(&id) else { return };
        for (index, key) in [(&mut self.by_handle, &object.handle), (&mut self.by_kind, &object.kind)] {
            if let Some(ids) = index.get_mut(key) {
                ids.remove(&id);
                if ids.is_empty() {
                    index.remove(key);
                }
            }
        }
        self.facts.forget(id);
        self.removed.insert(object.handle, self.name.clone());
        self.touch();
    }

    // --- facts ---

    pub fn state(&mut self, subj: ObjId, rel: &Sym, value: Value) {
        self.facts.insert(subj, rel, value);
        self.touch();
    }

    /// Removes a stated fact; `false` if it wasn't stated.
    pub fn unstate(&mut self, subj: ObjId, rel: &str, value: &Value) -> bool {
        let removed = self.facts.remove(subj, rel, value);
        self.touch();
        removed
    }

    /// Removes every stated value of one relation of one subject, and returns them.
    pub fn clear(&mut self, subj: ObjId, rel: &str) -> Vec<Value> {
        let values = self.facts.clear(subj, rel);
        self.touch();
        values
    }

    // --- rules and constraints ---

    /// The rules that produce facts: all but those parents still disagree on.
    pub fn active_rules(&self) -> impl Iterator<Item = &Rc<Rule>> {
        self.rules.iter().filter(|(name, _)| !self.disputes.contains_key(&Dispute::Rule((*name).clone()))).map(|(_, r)| r)
    }

    pub fn active_nevers(&self) -> impl Iterator<Item = &Rc<Never>> {
        self.nevers.iter().filter(|(name, _)| !self.disputes.contains_key(&Dispute::Never((*name).clone()))).map(|(_, n)| n)
    }

    /// Adds a rule, or replaces the one with its name. Restating settles a dispute.
    pub fn set_rule(&mut self, rule: Rule) {
        self.disputes.remove(&Dispute::Rule(rule.name.clone()));
        self.rules.insert(rule.name.clone(), Rc::new(rule));
        self.touch();
    }

    pub fn remove_rule(&mut self, name: &Sym) -> bool {
        self.disputes.remove(&Dispute::Rule(name.clone()));
        self.touch();
        self.rules.remove(name).is_some()
    }

    pub fn set_never(&mut self, never: Never) {
        self.disputes.remove(&Dispute::Never(never.name.clone()));
        self.nevers.insert(never.name.clone(), Rc::new(never));
        self.touch();
    }

    pub fn remove_never(&mut self, name: &Sym) -> bool {
        self.disputes.remove(&Dispute::Never(name.clone()));
        self.touch();
        self.nevers.remove(name).is_some()
    }

    // --- showing ---

    /// A value as Grant shows it: numbers and text as written, objects by their handle.
    pub fn show(&self, value: &Value) -> String {
        match value {
            Value::Num(n) => n.to_string(),
            Value::Text(t) => format!("{t:?}"),
            Value::Obj(id) => match self.locate(*id) {
                Some((world, object)) if world.find(&object.handle).len() > 1 => {
                    // Indexes can repeat after a merge; then the world that made it tells them apart.
                    let twins = world
                        .find(&object.handle)
                        .iter()
                        .filter(|o| world.objects[*o].kind == object.kind && world.objects[*o].index == object.index)
                        .count();
                    let origin = if twins > 1 { format!(" from {}", object.made_in) } else { String::new() };
                    format!("{} ({} {}{origin})", object.handle, object.kind, object.index)
                }
                Some((_, object)) => object.handle.to_string(),
                None => "something from another world".into(),
            },
        }
    }

    /// Finds an object here or inside the worlds held here.
    fn locate(&self, id: ObjId) -> Option<(&World, &Object)> {
        match self.objects.get(&id) {
            Some(object) => Some((self, object)),
            None => self.held().find_map(|w| w.locate(id)),
        }
    }
}

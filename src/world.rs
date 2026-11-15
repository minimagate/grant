//! The world structure: W = (K, O, F, R).

use crate::ast::Field;
use crate::error::Res;
use crate::logic::{Never, Rule};
use crate::model::{self, Model};
use crate::value::{ObjId, PRIMITIVES, Sym, Value, a};
use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

/// A kind's shape: every relation its objects can have.
pub type Shape = BTreeMap<Sym, Field>;

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

/// Takes `item` out of the set under `key`, and the set with it once it is empty.
fn take<K: Ord, T: Ord>(index: &mut BTreeMap<K, BTreeSet<T>>, key: &K, item: &T) -> bool {
    let Some(set) = index.get_mut(key) else { return false };
    let found = set.remove(item);
    if set.is_empty() {
        index.remove(key);
    }
    found
}

impl Facts {
    pub fn len(&self) -> usize {
        self.len
    }

    /// Adds a fact; `false` if it was already there.
    pub fn insert(&mut self, subj: ObjId, rel: &Sym, value: Value) -> bool {
        let r = self.rels.entry(rel.clone()).or_default();
        let new = r.by_subject.entry(subj).or_default().insert(value.clone());
        r.by_value.entry(value).or_default().insert(subj);
        self.len += new as usize;
        new
    }

    /// Removes a fact; `false` if it wasn't there.
    pub fn remove(&mut self, subj: ObjId, rel: &str, value: &Value) -> bool {
        let Some(r) = self.rels.get_mut(rel) else { return false };
        let found = take(&mut r.by_subject, &subj, value) && take(&mut r.by_value, value, &subj);
        self.len -= found as usize;
        found
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

    pub fn iter(&self) -> impl Iterator<Item = (ObjId, &Sym, &Value)> {
        self.rels.keys().flat_map(|rel| self.pairs(rel).map(move |(subj, value)| (subj, rel, value)))
    }
}

/// Something parents disagree on, which the child must settle before it ends.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Dispute {
    Field(Sym, Sym),
    Rule(Sym),
    Never(Sym),
}

#[derive(Clone, Default)]
pub struct World {
    pub id: u32,
    pub name: Sym,
    pub kinds: BTreeMap<Sym, Shape>,
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
    model: OnceCell<Rc<Model>>,
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
        World { id, name: name.into(), kinds: BTreeMap::from([("world".into(), Shape::new())]), ..World::default() }
    }

    /// The union of the parents: everything any of them has. What they disagree on is recorded
    /// as disputes rather than refused.
    pub fn merge(id: u32, name: &str, parents: &[Rc<World>]) -> World {
        let Some((first, rest)) = parents.split_first() else { return World::new(id, name) };
        let mut world = World { id, name: name.into(), created: 0, ..first.as_ref().clone() };
        for (n, parent) in rest.iter().enumerate() {
            let earlier = parents[..=n].iter().map(|p| &*p.name).collect::<Vec<_>>().join(", ");
            let differs = |what: &str, name: &Sym| format!("{what} `{name}` differs between {earlier} and {}; restate it", parent.name);
            for (kind, shape) in &parent.kinds {
                let ours = world.kinds.entry(kind.clone()).or_default();
                for (rel, field) in shape {
                    let old = ours.entry(rel.clone()).or_insert_with(|| field.clone());
                    if old != field {
                        let message = format!(
                            "`{rel}` of {kind} is declared two ways (`{rel}: {old}` in {earlier}, `{rel}: {field}` in {}); restate it with `kind {kind} [ + {rel}: … ]`",
                            parent.name
                        );
                        world.disputes.entry(Dispute::Field(kind.clone(), rel.clone())).or_insert(message);
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
            for (name, rule) in &parent.rules {
                if world.rules.entry(name.clone()).or_insert_with(|| rule.clone()) != rule {
                    world.disputes.entry(Dispute::Rule(name.clone())).or_insert_with(|| differs("rule", name));
                }
            }
            for (name, never) in &parent.nevers {
                if world.nevers.entry(name.clone()).or_insert_with(|| never.clone()) != never {
                    world.disputes.entry(Dispute::Never(name.clone())).or_insert_with(|| differs("never", name));
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
    fn touch(&mut self) {
        self.model.take();
    }

    /// F ∪ D: the stated facts and everything the rules make follow from them.
    pub fn model(&self) -> Res<&Model> {
        if let Some(model) = self.model.get() {
            return Ok(model);
        }
        let model = Rc::new(model::derive(self)?);
        Ok(self.model.get_or_init(|| model))
    }

    // --- kinds ---

    /// Whether this kind is here and is numbers, text or indexes, which no statement makes.
    pub fn has_primitive(&self, kind: &str) -> bool {
        self.kinds.contains_key(kind) && PRIMITIVES.iter().any(|(k, _)| *k == kind)
    }

    /// What Grant says when a kind is missing.
    pub fn no_kind(&self, kind: &str) -> String {
        match PRIMITIVES.iter().find(|(k, _)| *k == kind) {
            Some((_, world)) => format!("this world has no {kind}; add `from {world}`"),
            None => format!("kind `{kind}` doesn't exist yet"),
        }
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
        self.kinds.get(kind)?.get(rel)
    }

    /// The declaration of `kind.rel`, or what Grant says when the shape lacks it.
    pub fn relation(&self, kind: &str, rel: &str) -> Res<&Field> {
        self.field(kind, rel).ok_or_else(|| format!("{kind} has no relation `{rel}`"))
    }

    pub fn is_disputed(&self, kind: &Sym, rel: &Sym) -> bool {
        self.disputes.contains_key(&Dispute::Field(kind.clone(), rel.clone()))
    }

    /// Declares a relation, replaces its declaration or (with `None`) removes it. Each settles
    /// any dispute over it.
    pub fn set_field(&mut self, kind: &Sym, rel: &Sym, field: Option<Field>) {
        let shape = self.kinds.entry(kind.clone()).or_default();
        match field {
            Some(field) => shape.insert(rel.clone(), field),
            None => shape.remove(rel),
        };
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
            several => Err(format!("{} objects called {handle}; {}", several.len(), self.tell_apart(handle, several))),
        }
    }

    /// How to point at one of several objects sharing a handle: by kind if the kinds differ.
    pub fn tell_apart(&self, handle: &str, ids: &[ObjId]) -> String {
        let kinds: BTreeSet<&Sym> = ids.iter().map(|id| &self.objects[id].kind).collect();
        if kinds.len() < ids.len() {
            return "use `the`".into();
        }
        let ways: Vec<String> = kinds.iter().map(|kind| format!("`{kind}@{handle}`")).collect();
        format!("write {}", ways.join(" or "))
    }

    pub fn of_kind(&self, kind: &str) -> impl Iterator<Item = ObjId> {
        self.by_kind.get(kind).into_iter().flatten().copied()
    }

    /// The world this object is, if it is a held world.
    pub fn held(&self, value: &Value) -> Option<&Rc<World>> {
        match value {
            Value::Obj(id) => self.objects.get(id)?.held.as_ref(),
            _ => None,
        }
    }

    /// The worlds this world holds.
    pub fn worlds(&self) -> impl Iterator<Item = &Rc<World>> {
        self.of_kind("world").filter_map(|id| self.objects[&id].held.as_ref())
    }

    fn insert(&mut self, id: ObjId, object: Object) {
        self.by_handle.entry(object.handle.clone()).or_default().insert(id);
        self.by_kind.entry(object.kind.clone()).or_default().insert(id);
        self.objects.insert(id, object);
        self.touch();
    }

    /// Creates an object here: its origin is this world and its place in this world's creations.
    pub fn create(&mut self, kind: &Sym, handle: &Sym) -> ObjId {
        self.created += 1;
        self.add(ObjId { world: self.id, seq: self.created }, kind, handle, None)
    }

    /// Holds a world as an object. A world is its own origin, so holding it twice holds it once.
    pub fn hold(&mut self, handle: &Sym, world: Rc<World>) -> ObjId {
        self.add(ObjId { world: world.id, seq: 0 }, &"world".into(), handle, Some(world))
    }

    fn add(&mut self, id: ObjId, kind: &Sym, handle: &Sym, held: Option<Rc<World>>) -> ObjId {
        if !self.objects.contains_key(&id) {
            let index = self.indexes.entry(kind.clone()).or_default();
            *index += 1;
            let object = Object { kind: kind.clone(), handle: handle.clone(), index: *index, made_in: self.name.clone(), held };
            self.insert(id, object);
        }
        id
    }

    /// Removes an object and every fact touching it.
    pub fn remove_object(&mut self, id: ObjId) {
        let Some(object) = self.objects.remove(&id) else { return };
        take(&mut self.by_handle, &object.handle, &id);
        take(&mut self.by_kind, &object.kind, &id);
        let touches = |(s, _, v): &(ObjId, &Sym, &Value)| *s == id || **v == Value::Obj(id);
        let touching: Vec<_> = self.facts.iter().filter(touches).map(|(s, r, v)| (s, r.clone(), v.clone())).collect();
        for (subj, rel, value) in touching {
            self.facts.remove(subj, &rel, &value);
        }
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
        self.touch();
        self.facts.remove(subj, rel, value)
    }

    /// Removes every stated value of one relation of one subject.
    pub fn clear(&mut self, subj: ObjId, rel: &str) {
        for value in self.facts.values(subj, rel).cloned().collect::<Vec<_>>() {
            self.unstate(subj, rel, &value);
        }
    }

    // --- rules and constraints ---

    /// The rules that produce facts: all but those parents still disagree on.
    pub fn active_rules(&self) -> impl Iterator<Item = &Rc<Rule>> {
        self.rules.values().filter(|r| !self.disputes.contains_key(&Dispute::Rule(r.name.clone())))
    }

    pub fn active_nevers(&self) -> impl Iterator<Item = &Rc<Never>> {
        self.nevers.values().filter(|n| !self.disputes.contains_key(&Dispute::Never(n.name.clone())))
    }

    /// Adds a rule, replaces the one with its name or (with `None`) removes it; `false` if there
    /// was none. Restating settles a dispute.
    pub fn set_rule(&mut self, name: &Sym, rule: Option<Rule>) -> bool {
        self.disputes.remove(&Dispute::Rule(name.clone()));
        self.touch();
        match rule {
            Some(rule) => self.rules.insert(name.clone(), Rc::new(rule)),
            None => self.rules.remove(name),
        }
        .is_some()
    }

    pub fn set_never(&mut self, name: &Sym, never: Option<Never>) -> bool {
        self.disputes.remove(&Dispute::Never(name.clone()));
        self.touch();
        match never {
            Some(never) => self.nevers.insert(name.clone(), Rc::new(never)),
            None => self.nevers.remove(name),
        }
        .is_some()
    }

    // --- showing ---

    /// A value as Grant shows it: numbers and text as written, objects by their handle.
    pub fn show(&self, value: &Value) -> String {
        match value {
            Value::Num(n) => n.to_string(),
            Value::Text(t) => format!("{t:?}"),
            Value::Obj(id) => match self.locate(*id) {
                Some((world, object)) if world.find(&object.handle).len() > 1 => {
                    let same_kind = |o: &ObjId| world.objects[o].kind == object.kind;
                    if world.find(&object.handle).iter().filter(|o| same_kind(o)).count() == 1 {
                        return format!("{}@{}", object.kind, object.handle);
                    }
                    // Indexes can repeat after a merge; then the world that made it tells them apart.
                    let twin = |o: &ObjId| same_kind(o) && world.objects[o].index == object.index;
                    let twins = world.find(&object.handle).into_iter().filter(twin).count();
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
            None => self.worlds().find_map(|w| w.locate(id)),
        }
    }
}

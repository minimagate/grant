//! Equality of worlds compares shape: the same kinds, rules and constraints, and objects that
//! can be matched one to one so that every kind and every fact corresponds. Handles, identities
//! and indexes don't matter.

use crate::value::{ObjId, Sym, Value};
use crate::world::World;
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

thread_local! {
    /// Answers already worked out, by the two worlds' ids. Only finished worlds are compared.
    static KNOWN: RefCell<HashMap<(u32, u32), bool>> = RefCell::new(HashMap::new());
}

pub fn same_shape(a: &World, b: &World) -> bool {
    if a.id == b.id {
        return true;
    }
    let key = (a.id.min(b.id), a.id.max(b.id));
    if let Some(known) = KNOWN.with(|k| k.borrow().get(&key).copied()) {
        return known;
    }
    let same = compare(a, b);
    KNOWN.with(|k| k.borrow_mut().insert(key, same));
    same
}

/// One world's objects as a graph: numbered, with their facts as edges.
struct Graph<'a> {
    world: &'a World,
    ids: Vec<ObjId>,
    /// For each object: facts to other objects (relation, target), and facts from them.
    out: Vec<Vec<(&'a Sym, usize)>>,
    into: Vec<Vec<(&'a Sym, usize)>>,
    colour: Vec<u64>,
}

fn hash(value: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

impl<'a> Graph<'a> {
    fn new(world: &'a World) -> Graph<'a> {
        let ids: Vec<ObjId> = world.objects.keys().copied().collect();
        let number: BTreeMap<ObjId, usize> = ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
        let mut graph = Graph { world, out: vec![Vec::new(); ids.len()], into: vec![Vec::new(); ids.len()], colour: Vec::new(), ids };
        // An object starts coloured by its kind and the numbers and text it holds.
        let mut plain: Vec<Vec<(&Sym, &Value)>> = vec![Vec::new(); graph.ids.len()];
        for (subj, rel, value) in world.facts.iter() {
            let Some(&from) = number.get(&subj) else { continue };
            match value {
                Value::Obj(id) => {
                    if let Some(&to) = number.get(id) {
                        graph.out[from].push((rel, to));
                        graph.into[to].push((rel, from));
                    }
                }
                value => plain[from].push((rel, value)),
            }
        }
        graph.colour = graph.ids.iter().zip(plain).map(|(id, plain)| hash((&world.objects[id].kind, plain))).collect();
        graph
    }

    /// The relations and colours on one side of an object, in a fixed order.
    fn side(&self, edges: &[(&'a Sym, usize)]) -> Vec<(&'a Sym, u64)> {
        let mut seen: Vec<_> = edges.iter().map(|(rel, j)| (*rel, self.colour[*j])).collect();
        seen.sort();
        seen
    }

    /// Recolours every object by its neighbours' colours, until that tells no more apart.
    fn refine(&mut self) {
        let classes = |colour: &[u64]| colour.iter().collect::<std::collections::BTreeSet<_>>().len();
        loop {
            let next: Vec<u64> =
                (0..self.ids.len()).map(|i| hash((self.colour[i], self.side(&self.out[i]), self.side(&self.into[i])))).collect();
            let told_apart = classes(&next) > classes(&self.colour);
            self.colour = next;
            if !told_apart {
                return;
            }
        }
    }
}

/// Compares two named sets of rules or constraints with their objects set aside, and collects
/// which object of `a` must match which of `b`.
fn same_vocabulary<T, S: PartialEq>(
    a: &BTreeMap<Sym, Rc<T>>,
    b: &BTreeMap<Sym, Rc<T>>,
    abstracted: impl Fn(&T) -> (S, Vec<ObjId>),
    forced: &mut Vec<(ObjId, ObjId)>,
) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|((name_a, rule_a), (name_b, rule_b))| {
            let ((shape_a, objects_a), (shape_b, objects_b)) = (abstracted(rule_a), abstracted(rule_b));
            forced.extend(objects_a.into_iter().zip(objects_b));
            name_a == name_b && shape_a == shape_b
        })
}

fn compare(a: &World, b: &World) -> bool {
    let mut forced = Vec::new();
    if a.kinds != b.kinds
        || a.objects.len() != b.objects.len()
        || a.facts.len() != b.facts.len()
        || !same_vocabulary(&a.rules, &b.rules, |r| r.abstracted(), &mut forced)
        || !same_vocabulary(&a.nevers, &b.nevers, |n| n.abstracted(), &mut forced)
    {
        return false;
    }
    // Worlds that share an ancestor are usually the same objects with the same facts.
    if a.objects.keys().eq(b.objects.keys()) && a.facts == b.facts && forced.iter().all(|(x, y)| x == y) {
        return true;
    }
    let (mut ga, mut gb) = (Graph::new(a), Graph::new(b));
    ga.refine();
    gb.refine();
    let (mut ca, mut cb) = (ga.colour.clone(), gb.colour.clone());
    ca.sort();
    cb.sort();
    if ca != cb {
        return false;
    }
    let n = ga.ids.len();
    let mut matching = Matching { a: &ga, b: &gb, to: vec![None; n], taken: vec![false; n] };
    for (x, y) in forced {
        let (i, j) = match (ga.ids.iter().position(|id| *id == x), gb.ids.iter().position(|id| *id == y)) {
            (Some(i), Some(j)) => (i, j),
            // Both rules name an object since removed: nothing to match.
            (None, None) => continue,
            _ => return false,
        };
        match matching.to[i] {
            Some(already) if already != j => return false,
            Some(_) => {}
            None if matching.taken[j] || !matching.fits(i, j) => return false,
            None => matching.pair(i, Some(j)),
        }
    }
    // Objects that fewer others look like go first: they leave the least to try.
    let mut class_size: HashMap<u64, usize> = HashMap::new();
    ga.colour.iter().for_each(|c| *class_size.entry(*c).or_default() += 1);
    let mut order: Vec<usize> = (0..n).filter(|i| matching.to[*i].is_none()).collect();
    order.sort_by_key(|i| class_size[&ga.colour[*i]]);
    matching.search(&order)
}

struct Matching<'g, 'w> {
    a: &'g Graph<'w>,
    b: &'g Graph<'w>,
    to: Vec<Option<usize>>,
    taken: Vec<bool>,
}

impl Matching<'_, '_> {
    fn pair(&mut self, i: usize, j: Option<usize>) {
        if let Some(old) = std::mem::replace(&mut self.to[i], j) {
            self.taken[old] = false;
        }
        if let Some(j) = j {
            self.taken[j] = true;
        }
    }

    /// Whether object `i` of `a` can be object `j` of `b`, given the matches made so far.
    fn fits(&self, i: usize, j: usize) -> bool {
        if self.a.colour[i] != self.b.colour[j] {
            return false;
        }
        let held = |g: &Graph, k: usize| g.world.objects[&g.ids[k]].held.clone();
        match (held(self.a, i), held(self.b, j)) {
            (Some(x), Some(y)) if !same_shape(&x, &y) => return false,
            (Some(_), None) | (None, Some(_)) => return false,
            _ => {}
        }
        // Every fact between `i` and an object already matched must be there between their matches.
        let matched = |k: usize| if k == i { Some(j) } else { self.to[k] };
        let fact = |from: usize, rel: &Sym, to: usize| self.b.world.facts.contains(self.b.ids[from], rel, &Value::Obj(self.b.ids[to]));
        self.a.out[i].iter().all(|(rel, k)| matched(*k).is_none_or(|k| fact(j, rel, k)))
            && self.a.into[i].iter().all(|(rel, k)| matched(*k).is_none_or(|k| fact(k, rel, j)))
    }

    fn search(&mut self, order: &[usize]) -> bool {
        let Some((&i, rest)) = order.split_first() else { return true };
        for j in 0..self.taken.len() {
            if !self.taken[j] && self.fits(i, j) {
                self.pair(i, Some(j));
                if self.search(rest) {
                    return true;
                }
                self.pair(i, None);
            }
        }
        false
    }
}

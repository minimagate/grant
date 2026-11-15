//! Worlds run end to end, checked against what the Foundation 1 specification says they show.

use grant::Loader;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

/// Writes the worlds to a fresh folder and runs the last one. Returns what it showed, or the
/// error as Grant prints it. Lines are moved to the margin first, so the worlds below can be
/// indented with the tests; continuation lines are covered by the examples.
fn run(worlds: &[(&str, &str)]) -> Result<Vec<String>, String> {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!("grant-test-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    for (name, source) in worlds {
        let path = dir.join(format!("{name}.grant"));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let source: Vec<&str> = source.lines().map(str::trim_start).collect();
        std::fs::write(path, source.join("\n")).unwrap();
    }
    let mut shown = Vec::new();
    let result = Loader::new().run(&dir.join(format!("{}.grant", worlds.last().unwrap().0)), &mut shown);
    std::fs::remove_dir_all(&dir).ok();
    result.map(|_| shown.into_iter().map(|o| o.text).collect()).map_err(|e| e.to_string())
}

fn shows(source: &str) -> Vec<String> {
    run(&[("w", source)]).unwrap_or_else(|e| panic!("{e}"))
}

/// The message of the error a single world fails with.
fn fails(source: &str) -> String {
    let error = run(&[("w", source)]).expect_err("this world should fail");
    error.lines().next().unwrap().split_once("): ").map_or(error.clone(), |(_, m)| m.to_string())
}

/// The problems a world ends with.
fn ends_with(worlds: &[(&str, &str)]) -> Vec<String> {
    let error = run(worlds).expect_err("this world should fail at its end");
    assert!(error.starts_with("error at the end of"), "{error}");
    error.lines().skip(1).map(|l| l.trim_start_matches("  - ").to_string()).collect()
}

const TOWN: &str = "
from numbers, text
kind city
kind feeling
kind condition
kind person [
  age:       one number
  lives_in:  one city
  knows:     person
  feels:     feeling
  is:        condition
]
city pisa
city rome
city florence
feeling happy
condition adult
person liz    [ age: 27 | lives_in: pisa ]
person laurie [ age: 30 | lives_in: rome ]
person harbor [ age: 24 | lives_in: pisa | knows: liz, laurie ]
";

fn town(lines: &str) -> String {
    format!("{TOWN}{lines}")
}

#[test]
fn the_examples_run() {
    let example = |name: &str| {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples").join(name);
        let mut shown = Vec::new();
        Loader::new().run(&path, &mut shown).unwrap_or_else(|e| panic!("{e}"));
        shown.into_iter().map(|o| o.text).collect::<Vec<_>>()
    };
    assert_eq!(
        example("harbor.grant"),
        ["yes", "harbor", "liz", "3", "pisa, happy, peace, liz", "liz, laurie", "28.5", "yes", "nothing", "no", "25", "laurie", "liz"]
    );
    assert_eq!(example("towns/both.grant"), ["rome, florence", "rome"]);
    assert_eq!(example("compare.grant"), ["yes", "move_rome", "rome", "liz", "no", "yes", "yes"]);
    assert_eq!(example("calendar.grant"), ["2026", "11", "14", "saturday"]);
    // The department project: a merge that is settled, a review that holds the plans, a year on.
    assert_eq!(
        example("department/plan_merged.grant")[..4],
        ["analysis_2, group_theory, galois_theory, differential_geometry", "cauchy, riemann", "nothing", "nothing"]
    );
    let review = example("department/review.grant");
    assert_eq!(review[2], "plan_merged");
    assert_eq!(review[5..], ["yes", "no", "yes"]);
    assert_eq!(example("department/next_year.grant")[..2], ["logic_1", "s: felix, c: group_theory"]);
}

#[test]
fn a_world_starts_empty() {
    assert_eq!(fails("kind person [ age: one number ]"), "this world has no number; add `from numbers`");
    assert_eq!(fails("kind person [ name: text ]\nperson liz\n? liz.name = \"Liz\""), "this world has no text; add `from text`");
    assert_eq!(fails("kind person\nperson liz\n? liz.id"), "this world has no id; add `from id`");
    assert_eq!(fails("person liz"), "kind `person` doesn't exist yet");
}

#[test]
fn values_and_expressions() {
    let shown = shows(&town(
        "? harbor.knows.lives_in
         ? (harbor.age + 1) * 2
         ? 17 div 5
         ? 17 mod 5
         ? 1 / 3
         ? count harbor.knows
         ? sum harbor.knows.age
         ? max harbor.knows.age
         ? min harbor.knows.age
         ? \"Linear\" + \" \" + \"Algebra\"
         ? harbor.feels
         ? count (person p lives_in pisa)
         ? harbor.age - 30
         ? 3.5 + 0.25
         ? \"alice\" < \"Zed\"",
    ));
    assert_eq!(
        shown,
        ["pisa, rome", "50", "3", "2", "1/3", "2", "57", "30", "27", "\"Linear Algebra\"", "nothing", "2", "-6", "3.75", "yes"]
    );
}

#[test]
fn aggregates_take_one_value_per_fact() {
    // Two friends of the same age are still two ages.
    let shown = shows(&town("laurie [ age: 27 ]\n? sum harbor.knows.age\n? sum harbor.knows.age / count harbor.knows\n? harbor.knows.age"));
    assert_eq!(shown, ["54", "27", "27"]);
}

#[test]
fn things_are_counted_once_and_values_once_per_thing() {
    // Two friends who know the same person: she is one person, with one age.
    let shown = shows(&town(
        "person eve [ age: 40 | lives_in: rome | knows: liz ]
         laurie knows liz
         harbor knows eve
         ? count harbor.knows.knows
         ? sum harbor.knows.knows.age
         ? count harbor.knows.lives_in
         ? harbor.knows = liz
         ? harbor.knows != liz
         ? harbor.knows != eve
         ? harbor.lives_in != rome
         ? harbor.feels != happy",
    ));
    assert_eq!(shown, ["1", "27", "2", "yes", "no", "no", "yes", "no"]);
}

#[test]
fn hostile_lines_are_refused_not_followed() {
    let deep = format!("? {}1{}", "(".repeat(5000), ")".repeat(5000));
    assert_eq!(fails(&town(&deep)), "this line nests too deeply");
    assert_eq!(fails(&town(&format!("? {} harbor knows liz", "not ".repeat(5000)))), "this line nests too deeply");
    let counts = format!("? {}person q){} > 0", "count (".repeat(40), " > 0)".repeat(39));
    assert_eq!(shows(&town(&counts)), ["yes"]);
    assert_eq!(
        fails("from text\nkind p [ name: text ]\np a [ name: \"ab\" ]\nrule dbl: p x name x.name + x.name if x.name >= \"a\""),
        "rule `dbl` never settles: it keeps producing new values"
    );
    assert_eq!(fails(&town("person zed\n? -zed.age")), "zed has no `age`");
}

#[test]
fn a_world_with_no_lines_is_still_its_own_world() {
    let shown = run(&[("e1", "-- nothing"), ("e2", ""), ("w", "world e1\nworld e2\n? count (world w)\n? e1 = e2")]).unwrap();
    assert_eq!(shown, ["2", "yes"]);
}

#[test]
fn questions_show_values_truth_and_matches() {
    let shown = shows(&town(
        "? harbor knows liz
         ? liz knows harbor
         ? person p lives_in pisa
         ? person p lives_in pisa and not p knows person q
         ? person a knows person b
         ? person p lives_in rome and p.age > 40
         ? harbor.age >= 18 and harbor.lives_in = pisa
         ? city c",
    ));
    assert_eq!(
        shown,
        ["yes", "no", "liz, harbor", "liz", "a: harbor, b: liz\na: harbor, b: laurie", "nothing", "yes", "pisa, rome, florence"]
    );
}

#[test]
fn rules_derive_live_and_recursively() {
    let shown = shows(&town(
        "kind person [ + can_reach: person ]
         person robin [ age: 12 | lives_in: rome ]
         rule mutual: person a knows person b if b knows a
         rule adult:  person p is adult if p.age >= 18
         rule reach:  person a can_reach person b if a knows b or (a knows person m and m can_reach b)
         ? liz.knows
         ? person p is adult
         ? liz can_reach laurie
         ? robin can_reach liz
         laurie knows robin
         ? robin can_reach liz
         robin [ age: 18 ]
         ? robin is adult
         - rule adult
         ? robin is adult",
    ));
    assert_eq!(shown, ["harbor", "liz, laurie, harbor", "yes", "no", "yes", "yes", "no"]);
}

#[test]
fn flags_built_from_each_other_are_not_circular() {
    // `is minor` reads `is adult`, not itself: values are part of what a rule depends on.
    let shown = shows(&town(
        "condition minor
         person kid [ age: 9 | lives_in: pisa ]
         rule adult: person p is adult if p.age >= 18
         rule minor: person p is minor if not p is adult
         ? person p is minor",
    ));
    assert_eq!(shown, ["kid"]);
}

#[test]
fn rules_cannot_depend_on_their_own_negation_or_aggregate() {
    assert_eq!(
        fails(&town("condition popular\nrule popular: person p is popular if count (person q is popular) < 2")),
        "rule `popular` counts itself"
    );
    assert_eq!(fails(&town("condition odd\nrule odd: person p is odd if not p is odd")), "rule `odd` depends on its own negation");
    assert_eq!(
        fails(&town("condition a\ncondition b\nrule ra: person p is a if not p is b\nrule rb: person p is b if not p is a")),
        "rule `rb` depends on its own negation through rule `ra`"
    );
    assert_eq!(
        fails(&town("rule older: person p age p.age + 1 if p.age > 0")),
        "rule `older` never settles: it keeps producing new values"
    );
}

#[test]
fn changing_facts() {
    let shown = shows(&town(
        "person robin [ age: 40 | lives_in: rome ]
         + harbor knows robin
         ? harbor.knows
         harbor [ lives_in: rome ]
         ? harbor.lives_in
         harbor [ knows: + liz, - robin ]
         ? harbor.knows
         harbor [ knows: + laurie.knows ]
         - harbor knows liz
         ? harbor.knows
         - harbor knows
         ? harbor.knows
         harbor [ age: harbor.age + 1 ]
         ? harbor.age
         - robin
         ? person p lives_in rome
         - harbor knows ghost
         - ghost",
    ));
    assert_eq!(
        shown,
        [
            "liz, laurie, robin",
            "rome",
            "liz, laurie",
            "laurie",
            "nothing",
            "25",
            "laurie, harbor",
            "nothing removed: `ghost` isn't here",
            "nothing removed: `ghost` isn't here"
        ]
    );
}

#[test]
fn each_changes_many_at_once() {
    let shown = shows(&town(
        "each person p [ age: p.age + 1 ]
         ? sum harbor.knows.age + harbor.age
         each person p where p.lives_in = pisa [ age: p.age + 1 ]
         ? liz.age
         ? laurie.age
         each person p where p.age > 30 feels happy
         ? person p feels happy
         - each person p feels
         ? person p feels happy
         - each person p where p.age < 27
         ? person p",
    ));
    assert_eq!(shown, ["84", "29", "31", "laurie", "nothing", "liz, laurie"]);
}

#[test]
fn removing_a_fact_a_rule_still_produces_is_reported() {
    let shown = shows(&town(
        "rule mutual: person a knows person b if b knows a
         liz knows harbor
         - harbor knows liz
         ? harbor knows liz
         - laurie knows harbor",
    ));
    assert_eq!(shown, ["removed, but it still follows from rule mutual", "yes", "it isn't stated here; it follows from rule mutual"]);
    // Removing from many reports the same, and removing a thing a rule names leaves the rule.
    let shown = shows(&town(
        "rule mutual: person a knows person b if b knows a
         rule glad: person p feels happy if p knows liz
         - each person p knows
         ? liz.knows
         harbor knows liz
         - each person p where p.age > 1 knows
         - happy
         ? harbor.feels",
    ));
    assert_eq!(shown, ["nothing", "rule `glad` names happy, and matches nothing through it now", "nothing"]);
}

#[test]
fn pointing_at_things() {
    let two = town("person harbor [ age: 50 | lives_in: rome ]\n");
    assert_eq!(fails(&format!("{two}harbor feels happy")), "2 objects called harbor; use `the`");
    assert_eq!(fails(&format!("{two}the harbor [ lives_in: florence ] feels happy")), "no objects match this description");
    assert_eq!(fails(&format!("{two}the person harbor [ age: harbor.age ] feels happy")), "2 objects called harbor; use `the`");
    assert_eq!(fails(&format!("{two}city harbor\nthe person harbor [ ] feels happy")), "2 people match this description");
    let shown = shows(&format!(
        "{two}the harbor [ lives_in: rome ] knows liz
         the person harbor [ lives_in: pisa ] feels happy
         ? the harbor [ lives_in: rome ] knows liz
         ? the harbor [ lives_in: rome ] feels happy
         the harbor [ age: 50 ] [ age: 51 ]
         ? person p and p.age > 50
         ? person p knows liz"
    ));
    assert_eq!(shown, ["yes", "no", "harbor (person 4)", "harbor (person 3), harbor (person 4)"]);
}

#[test]
fn errors_on_a_line() {
    assert_eq!(fails(&town("harbor lives_in venice")), "`venice` doesn't exist yet");
    assert_eq!(fails(&town("harbor favourite_colour pisa")), "person has no relation `favourite_colour`");
    assert_eq!(fails(&town("harbor lives_in happy")), "`lives_in` takes a city, not a feeling");
    assert_eq!(fails(&town("harbor [ age: \"old\" ]")), "`age` takes a number, not a text");
    assert_eq!(fails(&town("rule calm: person p is_at adult if p feels happy")), "person doesn't declare `is_at`");
    assert_eq!(fails(&town("rule r: person harbor is adult if harbor.age > 1")), "`harbor` is already an object");
    assert_eq!(fails(&town("rule r: person p is adult if p.colour = 1")), "person has no relation `colour`");
    assert_eq!(fails(&town("rule r: person p is adult if p lives_in happy")), "`lives_in` takes a city, not a feeling");
    assert_eq!(fails(&town("harbor.knows")), "this value isn't used");
    assert_eq!(fails(&town("person robin\n? robin.age + 1")), "robin has no `age`");
    assert_eq!(fails(&town("? harbor.age / 0")), "division by zero");
    assert_eq!(fails(&town("? harbor.age + \"x\"")), "`+` needs two numbers or two texts, not a number and a text");
    assert_eq!(fails(&town("? harbor < liz")), "`<` orders two numbers or two texts, not a person and a person");
    assert_eq!(fails(&town("kind person [ - age ]")), "3 people still have `age`");
    assert_eq!(fails(&town("- kind person")), "3 people still use `person`");
    assert_eq!(fails(&town("- kind feeling")), "1 feeling still uses `feeling`");
    assert_eq!(fails(&town("kind mood\nkind person [ + mood: mood ]\n- kind mood")), "person still uses `mood` (`mood: mood`)");
    assert_eq!(shows(&town("kind mood\n- kind mood\n- kind mood\n? count (city c)")), ["3"]);
    assert_eq!(
        fails(&town("kind person [ age: maybe text ]")),
        "person already declares `age: one number`; write `+ age: maybe text` to change it"
    );
    assert_eq!(fails(&town("from numbers")), "`from` comes first, and lists every parent on one line");
    assert_eq!(fails("from town_x"), "no world called `town_x` here");
    assert_eq!(fails(&town("person pisa [ age: 1 | lives_in: pisa ]")), "2 objects called pisa; use `the`");
    assert_eq!(fails(&town("harbor knows liz laurie")), "`laurie` doesn't belong here");
    assert_eq!(fails(&town("person x [ age: 1")), "a bracket opened here is never closed");
}

#[test]
fn the_end_of_the_world_is_the_checkpoint() {
    let problems = ends_with(&[(
        "w",
        &town(
            "never sensible_age: person p where p.age < 0 or p.age > 150
             never adults_feel:  person p where p.age >= 18 and not p feels feeling f
             + harbor lives_in rome
             person robin [ age: 200 ]
             laurie feels happy
             liz feels happy
             harbor feels happy",
        ),
    )]);
    assert_eq!(
        problems,
        [
            "harbor has 2 `lives_in` (person: `lives_in: one city`)",
            "robin has no `lives_in` (person: `lives_in: one city`)",
            "robin breaks `adults_feel` (never adults_feel: person p where p.age >= 18 and not p feels feeling f)",
            "robin breaks `sensible_age` (never sensible_age: person p where p.age < 0 or p.age > 150)",
        ]
    );
    // A world is its final structure, not the order of its lines.
    assert_eq!(shows(&town("+ harbor lives_in rome\n? count harbor.lives_in\nharbor [ lives_in: rome ]")), ["2"]);
    assert_eq!(shows(&town("kind person [ + age: maybe number ]\n- harbor age\n? harbor.age")), ["nothing"]);
    assert_eq!(ends_with(&[("w", &town("kind person [ + age: one text ]"))])[0], "liz has a number, 27 as `age` (person: `age: one text`)");
}

const BASE: &str = "
from numbers
kind city
kind feeling
kind person [ age: one number | lives_in: one city | feels: feeling | is_at: feeling ]
city pisa
city rome
city florence
feeling happy
person harbor [ age: 24 | lives_in: pisa | feels: happy ]
";

#[test]
fn a_child_is_instructions_on_its_parent() {
    let shown = run(&[("town", BASE), ("move", "from town\nharbor [ lives_in: rome ]\n? harbor.lives_in\n? harbor.age")]).unwrap();
    assert_eq!(shown, ["rome", "24"]);
    // The parent's questions are not repeated, and nothing reaches back into it.
    let parent = format!("{BASE}? harbor.lives_in\n");
    let shown =
        run(&[("town", &parent), ("move", "from town\nharbor [ lives_in: rome ]"), ("again", "from town\n? harbor.lives_in")]).unwrap();
    assert_eq!(shown, ["pisa"]);
    // Removing what the parent no longer says does nothing; pointing at what it removed fails.
    let shown =
        run(&[("town", &format!("{BASE}- harbor feels happy")), ("child", "from town\n- harbor feels happy\n? harbor.feels")]).unwrap();
    assert_eq!(shown, ["nothing"]);
    let error = run(&[("town", &format!("{BASE}- harbor")), ("child", "from town\nharbor [ lives_in: rome ]")]).unwrap_err();
    assert!(error.contains("harbor doesn't exist (removed in town)"), "{error}");
}

#[test]
fn a_parent_with_errors_is_named() {
    let error = run(&[("town", &format!("{BASE}harbor lives_in venice")), ("child", "from town")]).unwrap_err();
    assert!(error.contains("`town` has errors; fix it first"), "{error}");
    assert!(error.contains("error in town (town.grant, line 11): `venice` doesn't exist yet"), "{error}");
}

#[test]
fn several_parents_merge_by_union() {
    let rome = ("move_rome", "from town\nharbor [ lives_in: rome ]\nperson robin [ age: 3 | lives_in: rome ]\n- harbor feels happy");
    let florence = ("move_florence", "from town\nharbor [ lives_in: florence ]\nperson robin [ age: 4 | lives_in: florence ]");
    let both = |lines: &str| run(&[("town", BASE), rome, florence, ("both", &format!("from id, move_rome, move_florence\n{lines}"))]);
    // Same origin, same object; different origin, different objects; a merge keeps what either says.
    let shown = both(
        "? harbor.lives_in
         ? count (person p)
         ? harbor feels happy
         ? id.2
         harbor [ lives_in: rome ]
         ? the robin [ lives_in: florence ] age 4
         person quinn [ age: 1 | lives_in: pisa ]
         ? quinn.id",
    )
    .unwrap();
    assert_eq!(
        shown,
        ["rome, florence", "3", "yes", "rome, robin (person 2 from move_rome), robin (person 2 from move_florence)", "yes", "3"]
    );
    assert_eq!(
        ends_with(&[("town", BASE), rome, florence, ("both", "from move_florence, move_rome")]),
        ["harbor has 2 `lives_in` (person: `lives_in: one city`)"]
    );
    assert!(both("robin [ age: 5 ]").unwrap_err().contains("2 objects called robin; use `the`"));
}

#[test]
fn disagreement_is_settled_by_stating() {
    let a = ("a", "from town\nrule calm: person p is_at happy if p feels happy\nkind person [ + nick: one number ]\nharbor [ nick: 1 ]");
    let b = ("b", "from town\nrule calm: person p is_at happy if p.age > 1\nkind person [ + nick: maybe city ]");
    let same = ("c", "from town\nrule calm:   person p is_at happy   if p feels happy -- spaced differently");
    assert_eq!(run(&[("town", BASE), a, same, ("ac", "from a, c\n? harbor is_at happy")]).unwrap(), ["yes"]);
    let problems = ends_with(&[("town", BASE), a, b, ("ab", "from a, b")]);
    assert_eq!(
        problems,
        [
            "`nick` of person is declared two ways (`nick: one number` in a, `nick: maybe city` in b); restate it with `kind person [ + nick: … ]`",
            "rule `calm` differs between a and b; restate it",
        ]
    );
    // Until a disputed rule is restated it produces nothing.
    let settled = "from a, b\n? harbor is_at happy\nrule calm: person p is_at happy if p.age > 1\nkind person [ + nick: one number ]\n? harbor is_at happy";
    assert_eq!(run(&[("town", BASE), a, b, ("ab", settled)]).unwrap(), ["no", "yes"]);
    assert_eq!(
        run(&[("town", BASE), a, b, ("ab", "from a, b\n- rule calm\nkind person [ + nick: maybe number ]")]).unwrap(),
        Vec::<String>::new()
    );
}

#[test]
fn worlds_are_objects() {
    let happy = ("town_a", "from town\nperson liz [ age: 30 | lives_in: rome | feels: happy ]");
    let sad = ("town_b", "from town\n- harbor feels happy");
    let twin = (
        "town_c",
        "from numbers\nkind city\nkind feeling\nkind person [ age: one number | lives_in: one city | feels: feeling | is_at: feeling ]\ncity a\ncity b\ncity c\nfeeling glad\nperson june [ age: 24 | lives_in: a | feels: glad ]",
    );
    let compare = "from numbers
        world town_a
        world town_b
        world town_c
        world town
        world sub/deep
        kind world [ + is_happier_than: world ]
        rule happier: world a is_happier_than world b if count (in a: person p feels happy) > count (in b: person p feels happy)
        ? town_a is_happier_than town_b
        ? world w is_happier_than town_b
        ? in town_a: harbor.lives_in
        ? in town_a: person p feels happy
        ? town_a = town_b
        ? town = town_c
        ? world w and w = town
        ? (in town_a: harbor feels happy) and not (in town_b: harbor feels happy)
        ? count (world w)
        ? in deep: in town: harbor.age";
    let worlds = [("town", BASE), happy, sad, twin, ("sub/deep", "from numbers\nworld ../town"), ("compare", compare)];
    assert_eq!(run(&worlds).unwrap(), ["yes", "town_a, town", "pisa", "harbor, liz", "no", "yes", "town_c, town", "yes", "5", "24"]);

    let held = |lines: &str| run(&[("town", BASE), ("w", &format!("world town\n{lines}"))]).unwrap_err();
    assert!(held("in town: harbor [ lives_in: rome ]").contains("town is held, not built on; write a world `from town`"));
    assert!(held("harbor [ lives_in: rome ]").contains("`harbor` doesn't exist yet"));
    assert!(held("? in town: robin.age").contains("`robin` doesn't exist in town"));
}

#[test]
fn equality_compares_shape() {
    let ring = |names: [&str; 3]| {
        let [a, b, c] = names;
        format!(
            "kind node [ next: one node ]\nnode {a}\nnode {b}\nnode {c}\n{a} next {b}\n{b} next {c}\n{c} next {a}\nrule r: node n next {a} if n next {a}"
        )
    };
    let line = "kind node [ next: one node ]\nnode a\nnode b\nnode c\na next b\nb next c\nc next c\nrule r: node n next a if n next a";
    let worlds = [("one", ring(["a", "b", "c"])), ("two", ring(["z", "x", "y"])), ("three", line.to_string())];
    let mut all: Vec<(&str, &str)> = worlds.iter().map(|(n, s)| (*n, s.as_str())).collect();
    all.push(("w", "world one\nworld two\nworld three\n? one = two\n? one = three\n? one != three"));
    assert_eq!(run(&all).unwrap(), ["yes", "no", "yes"]);
}

#[test]
fn no_cycles() {
    let error = run(&[("town_a", "world europe"), ("europe", "world town_a")]).unwrap_err();
    assert!(error.contains("europe → town_a → europe"), "{error}");
    assert!(run(&[("selfish", "from selfish")]).unwrap_err().contains("selfish → selfish"));
}

# Grant — Foundation Specification

Foundation 1 · 14 Nov 2026 · Michi

Grant is a language for describing worlds and changing them.

A **world** is a finite relational structure: kinds of things, the things themselves, the facts that relate them, and the rules that make further facts follow.

$$\mathcal{W} = (K,\ O,\ F,\ R)$$

Worlds are also things. Worlds that hold no other worlds form the first level, and each level can hold the worlds of the levels below it:

$$\mathcal{S}_0 = \{\, \mathcal{W} : \mathcal{W} \text{ holds no worlds} \,\}, \qquad \mathcal{S}_{n+1} = \{\, \mathcal{W} : \text{every world held by } \mathcal{W} \text{ is in } \mathcal{S}_0 \cup \cdots \cup \mathcal{S}_n \,\}$$

$$\mathcal{S} = \bigcup_{n \ge 0} \mathcal{S}_n$$

In a world of level $n+1$, the kind `world` is one more kind of thing, whose objects are worlds from the levels below:

$$D_{\texttt{world}} \subseteq \mathcal{S}_0 \cup \cdots \cup \mathcal{S}_n$$

A relation between worlds is then an ordinary fact, $(w_1,\ \texttt{is\_happier\_than},\ w_2) \in F$, and rules can produce it like any other. To look inside a held world, Grant evaluates there:

$$\mathcal{W} \models (\texttt{in } w : \varphi) \iff w \models \varphi$$

Because a world only holds worlds from lower levels, no world can hold itself, and worlds about worlds form a well-founded tower: facts about worlds of level $n$ live in a world of level $n+1$, which can in turn be held and related at level $n+2$.

**Contents:** 1 Principles · 2 The primitive set · 3 The world structure · 4 Primitive worlds · 5 Kinds, shapes and constraints · 6 Objects and facts · 7 Values and expressions · 8 Rules · 9 Questions · 10 Changing a world · 11 Building worlds on worlds · 12 Worlds as objects · 13 What each line does · 14 A full world · 15 Errors · 16 Grammar summary · 17 Decisions in Foundation 1 · 18 Beyond the foundation

---

## 1. Principles

Grant rests on one act: **stating**. A world is made of what has been stated in it, and nothing else; its state is exactly its statements. The six principles follow from taking that seriously.

1. **Nothing exists unless it is stated.**
   A world starts empty. Kinds, relations and objects come into being only when they are stated, and even numbers and text must be brought in. Existence is a statement, never an assumption.

2. **If it can be built, build it.**
   If everything must be stated into existence, the language should give only what cannot be stated from inside a world: the primitives. Everything else (mutual relations, averages, ordered collections, time, merge strategies) is built with the same statements as any world, so there is one way of making things, not two.

3. **What isn't stated isn't true.**
   Existence and truth come from the same act. A fact holds because it was stated or because a rule states that it follows; absence is falsity. This is why Grant needs no booleans and no null.

4. **Each world is one state.**
   If a world is exactly what has been stated in it, then a world is one state: its parents, plus its own statements, and nothing else. Building on a world or holding it starts another world and never changes what was stated in the first.

5. **A name is not an identity.**
   Worlds are compared and combined as structures, so what a thing is cannot depend on what it is called. An object is what it is because of where it was stated into being. Names and descriptions point at it, and its index numbers it, but neither is it.

6. **Every line is a statement.**
   Writing Grant is the act of stating, so every line states something about the world: a fact, a change, a definition or a question. Each one reads as a sentence, and symbols appear only where they carry meaning: `[ | ]`, `:`, `.`, `?`, `+`, `-`.

---

## 2. The primitive set

This is everything Grant gives you. Anything not on this list is written in Grant itself.

| Area | Primitives |
| --- | --- |
| Building | `from` |
| Things | `kind` with a shape (relation → how many + value kind), objects with a handle, `the` |
| Facts | `subject relation value`, brackets (create and set), `+`, `-`, `each … where` |
| Logic | `rule … if`, `never … where`, `and`, `or`, `not`, `( )`, variables introduced with a kind |
| Values | dots, `=`, `!=`, `count`, `sum`, `min`, `max`; arithmetic and order come with `numbers` and `text` |
| Asking | `?` |
| Primitive worlds | `numbers`, `text`, `id` |
| Holding | `world`, `in w:`, `=` between worlds |

Built, not given:

| Instead of | Write |
| --- | --- |
| mutual relations | `rule mutual: person a knows person b if b knows a` |
| average | `sum x / count x` |
| flags | a value: `condition adult`, then `harbor is adult` |
| time, dates | a world built on `numbers` |
| ordered collections | relations like `comes_before` |
| merge strategies | ordinary lines in the child world |

---

## 3. The world structure

A world is four sets.

```
W = (K, O, F, R)
```

| Part | Holds | Each element |
| --- | --- | --- |
| K — kinds | The sorts of things that exist | name + shape (relation → how many + value kind) |
| O — objects | The things themselves | identity + kind + handle + index |
| F — facts | What is stated about things | (subject, relation, value) |
| R — rules | What follows, and what must never hold | name + head + condition, or name + `never` pattern |

- **Identity** is where an object was created: the world that created it and its place in that world's order of creation. It is never written and never seen. It is what makes harbor in one world the same harbor in another.
- **Vocabulary by name, things by origin.** Kinds, relations, rules and constraints are identified by their names. Objects are identified by their origin. Two objects can share a handle; two kinds can't share a name.
- **Facts are a set.** Stating the same fact twice is stating it once. Order is lost; only the structure remains.
- **A fact's value** is an object or a primitive value (`24`, `"Linear Algebra"`).
- **Derived facts** are not stored. D is everything the rules produce from F, recomputed whenever F or R changes. Questions see F and D together:

```
M = F ∪ D(F, R)
```

### Equality

Two worlds are equal when they have the same kinds, rules and constraints, and their objects can be matched one to one so that every object's kind and every fact correspond. Handles, identities and indexes don't matter: equality compares **shape**. A world with june in place of harbor, and everything else the same, is equal.

---

## 4. Primitive worlds

Something is a primitive only if it can't be created inside a finite world. Primitives live in built-in worlds, and a world gets them only by building on them:

```
from numbers, text, id
```

A world without `from numbers` has no numbers.

| World | Kind | Holds | Example |
| --- | --- | --- | --- |
| `numbers` | `number` | exact numbers, `+ - * / div mod`, `< > <= >=` | `24`, `3.5`, `-2` |
| `text` | `text` | written content, `+` to join, `< > <= >=` (alphabetical) | `"Linear Algebra"` |
| `id` | `id` | one index per number, related to every object with that number | `id.1` |

Time is not a primitive: a date is a number of days, and a calendar is a world built on `numbers`, using `div` and `mod`.

What Grant doesn't need: **no booleans** (truth is whether a fact exists), **no null** (a missing value is a missing fact), **no lists** (a relation can hold any number of values).

### Index

Every object is numbered within its kind, in the order it was created: the first person is 1, the first city is 1. The relation runs both ways. In the world of section 14, built `from numbers, text, id`:

```
? harbor.id                     -- 3   (liz 1, laurie 2, harbor 3)
? id.1                          -- every object numbered 1: york, happy, peace, liz
? person p and p.id < 3         -- liz, laurie
```

- **Indexes are never reused.** If harbor is removed, 3 stays empty.
- **Indexes cross worlds.** An object keeps its index in every world built on the one that created it.
- **Indexes can repeat.** Numbering is per kind, and two worlds that each create a person after a shared parent can both number theirs 4. When they merge, `id.4` gives both. An index numbers a thing; it never identifies it.

### `world`

`world` is a kind built into the language. Its objects are worlds (section 12).

---

## 5. Kinds, shapes and constraints

A kind is declared with `kind`. Its shape declares every relation its objects can have, stated or derived: how many values it holds and what kind they are.

```
kind city
kind feeling
kind condition

kind person [
  age:              one number
  lives_in:         one city
  knows:            person
  feels:            feeling
  is:               condition
  is_at:            condition
  is_neighbour_of:  person
  can_reach:        person
]
```

How many values a relation can hold is a constraint. The common ones have a word in the shape; with no word, a relation holds any number.

| Word | Means | The same constraint written out |
| --- | --- | --- |
| `one` | exactly one | `never …: person p where count p.r != 1` |
| `maybe` | zero or one | `never …: person p where count p.r > 1` |
| `some` | at least one | `never …: person p where count p.r = 0` |
| `many`, or no word | any number | none |

Every other limit is a constraint written out: `never` names a pattern that must have no matches when the world ends.

```
never adult_only_after_18: person p where p is adult and p.age < 18
never sensible_age:        person p where p.age < 0 or p.age > 150
```

- **Shapes are closed.** Reading a kind tells you every relation its objects can have and how many values each holds. A relation not in the shape is an error, and so is a value of the wrong kind.
- **Stated and derived share a shape.** `knows` holds stated facts, and the `mutual` rule adds the other direction; both are just `knows: person`.
- **Brackets can span lines.** A new line works like `|`; short shapes stay on one line.
- **The end of the world is the checkpoint.** Constraints, the words in shapes, and any disagreement left by merging parents (section 11) are checked once, on the finished structure.
- **Constraints have names**, so they can be replaced and removed like rules. A shape's words change with the shape: `kind person [ + age: maybe number ]`.

---

## 6. Objects and facts

An object is created with its kind and a handle. Characteristics and relationships are the same thing, and brackets are a compact way to state several:

```
city york
person liz
person laurie
person harbor [ age: 24 | lives_in: york | knows: liz, laurie ]
```

is exactly

```
city york
person liz
person laurie
person harbor
harbor age 24
harbor lives_in york
harbor knows liz
harbor knows laurie
```

- **A fact is** `subject relation value`. Relations are one word, with `_` between parts: `lives_in`, `is_friends_with`.
- **Things must exist before they are mentioned.** `harbor lives_in york` is an error if `york` doesn't exist yet.
- **Comments** start with `--` and run to the end of the line.
- **What things share is a thing, not a text.** A name is an object: `name Emmy`, then `noether [ name: Emmy ]`. One person has one name and a name can have many people. A fact is stated from one end; a rule gives the other: `rule bearer: name n person person p if p name n` makes `Emmy.person` everyone called Emmy.

### Pointing at things

A bare handle works when exactly one object has it. When several do, describe the one you mean with `the`:

```
person harbor [ lives_in: york ]
person harbor [ lives_in: bath ]

the harbor [ lives_in: bath ] knows liz
the person harbor [ lives_in: york ] feels happy
```

`the` means exactly one thing fits. If none or several fit, Grant stops and says so. After `the`, brackets describe which thing you mean; they don't change it.

---

## 7. Values and expressions

**Spaces state, dots refer, `?` shows.** `harbor knows liz` adds a fact. `harbor.knows` *is* liz and laurie, wherever it is written. `? harbor.knows` prints them.

### Dots

```
harbor.age                 -- 24
harbor.lives_in            -- york
harbor.knows               -- liz, laurie
harbor.knows.lives_in      -- york, bath
```

- A dot gives every value of the relation: none, one or several.
- Dots chain, and collections flatten: `harbor.knows.lives_in` is where everyone harbor knows lives.

### Operators

| Operator | Comes with | Example |
| --- | --- | --- |
| `=`, `!=` | every kind | `harbor.lives_in = york`, `a != b` |
| `+ - * /`, `( )` | `numbers` | `(harbor.age + 1) * 2` |
| `div`, `mod` | `numbers` | `17 div 5` → 3, `17 mod 5` → 2 |
| `< > <= >=` | `numbers`, `text` | `harbor.age >= 18` |
| `+` (joins) | `text` | `linear_algebra.title + " II"` |

`* / div mod` come before `+ -`, then comparisons, then `not`, then `and`, then `or`. Numbers are exact: `1 / 3` stays a third. A `-` at the start of a line removes; anywhere else it is minus.

### Collections

| Word | Gives | Example |
| --- | --- | --- |
| `count` | how many | `count harbor.knows` → 2 |
| `sum` | total | `sum harbor.knows.age` → 57 |
| `min`, `max` | smallest, largest | `max harbor.knows.age` → 30 |

`count` also counts the matches of a pattern: `count (person p feels happy)`. An average is built: `sum harbor.knows.age / count harbor.knows`. Membership is a fact: `harbor knows liz`.

- **A value alone is not a line.** `harbor.knows` by itself is an error: every line states, changes, defines or shows.
- **Missing values don't compute.** `harbor.age + 1` is an error if harbor has no age.

---

## 8. Rules

A rule has a name, one fact it produces (the head), and a condition:

```
rule <name>: <head> if <condition>
```

```
-- with condition peace, adult, lonely and popular created

rule calm:       person p is_at peace if p feels happy
rule mutual:     person a knows person b if b knows a
rule adult:      person p is adult if p.age >= 18
rule neighbours: person a is_neighbour_of person b if a.lives_in = b.lives_in and a != b
rule reach:      person a can_reach person b if a knows b or (a knows person m and m can_reach b)
rule lonely:     person p is lonely if not p knows person q
rule popular:    person p is popular if
  count (person q knows p) > 10
```

- **Variables are introduced with a kind** (`person p`, `number n`) and reused by name inside the same rule. A variable can't share a name with an existing object.
- **Conditions** join facts and comparisons with `and`, `or` and `not`, grouped with `( )`. A variable first introduced inside `not` means "there is none": `not p knows person q` is "p knows nobody".
- **Several rules can produce the same relation.** A fact holds if any of them produces it, so a long `or` can also be split into separate rules.
- **Long statements continue on indented lines** (`popular` above).
- **Recursion is allowed** (`reach`, `mutual`). A rule can't depend on its own negation, or on a `count`, `sum`, `min` or `max` of itself: neither has a well-defined answer.
- **A rule's relation must be declared** in the shape of its subject's kind. The same relation can also hold stated facts (`knows` with `mutual`).
- **Rules are live.** Derived facts update after every line, so a question halfway through a world sees it as it stands there.

---

## 9. Questions

`?` evaluates whatever follows and shows the result. It changes nothing. An expression shows its value, a fact shows yes or no, a pattern with variables shows its matches.

| Question | Shows |
| --- | --- |
| `? harbor.age + 1` | 25 |
| `? harbor.knows` | liz, laurie |
| `? count harbor.knows` | 2 |
| `? harbor is_at peace` | yes or no |
| `? person p is_at peace` | every p that fits |
| `? person p lives_in york and not p feels happy` | every p that fits |
| `? the harbor [ lives_in: bath ] knows liz` | yes or no |
| `? in town_a: harbor feels happy` | yes or no, inside town_a |

Running a world shows its own questions only. A parent's questions are not repeated in its children.

---

## 10. Changing a world

`+` at the start of a line adds, `-` removes. `+` is optional: a plain line adds.

### Facts and objects

```
+ harbor knows robin            -- adds: harbor knows liz, laurie, robin
harbor [ lives_in: bath ]       -- brackets set: bath replaces york
harbor [ knows: + robin, - liz ]
- harbor knows liz              -- one fact
- harbor knows                  -- every knows from harbor
- harbor                        -- harbor and every fact touching it
```

- **Sentences add, brackets set.** `+ harbor lives_in bath` would give harbor two homes and break `lives_in: one city`; `harbor [ lives_in: bath ]` replaces every value. Inside brackets, `+` and `-` add and remove single values.
- **`-` removes only what was stated.** If the fact still follows from a rule, it stays true and Grant says so: "removed, but it still follows from rule mutual".
- **Removing what isn't there does nothing.** Stating something about what isn't there is an error.

### Computed changes and many at once

```
harbor [ age: harbor.age + 1 ]                              -- his age becomes his age plus one
harbor [ knows: + laurie.knows ]                            -- harbor meets everyone laurie knows
each person p [ age: p.age + 1 ]                            -- a year passes
each person p where p.lives_in = york [ age: p.age + 1 ]
- each person p feels                                       -- nobody feels anything
- each person p where p.age < 18                            -- removes those people
```

- An expression is computed once, on its line. `harbor [ age: harbor.age + 1 ]` stores 25; it is not a formula that keeps updating. For values that follow other values, use a rule.
- `each <kind> <variable>` applies the change to every object of that kind; `where` narrows it, with the same conditions as rules.

Brackets mean three things, decided by what comes before them:

| Before the brackets | Meaning | Example |
| --- | --- | --- |
| kind + new handle | create | `person harbor [ age: 24 ]` |
| handle, or `each` | set | `harbor [ age: harbor.age + 1 ]` |
| `the` | describe which one | `the harbor [ lives_in: bath ]` |

### Rules and constraints

```
- rule calm                     -- the rule goes, and its derived facts with it
rule calm: person p is_at peace if p feels happy and p knows person q
- never sensible_age
```

Names are unique, so writing a rule or constraint with an existing name replaces it.

### Kinds

```
kind person [ + email: text ]        -- declares a new relation
kind person [ + age: maybe number ]  -- an existing relation: sets how many and its value kind
kind person [ - age ]                -- refused while any person has an age
- kind feeling                       -- refused while anything uses feeling
```

`- kind` is refused while anything still uses the kind: objects of it, another kind's shape, or a rule. A world never loses half its contents by surprise.

---

## 11. Building worlds on worlds

Each world is written in one `.grant` file and is named by it: `town.grant` is the world `town`. Names resolve from the world's own folder (`from towns/town_a`). `numbers`, `text` and `id` are always available by name.

### A world is its parents plus its lines

```
-- move
from town

harbor [ lives_in: bath ]
```

`from` comes first and lists every parent. The world starts as its parents' structures and then runs its own lines. It inherits everything: kinds, objects, facts, rules and constraints. Its lines can change any of it, including removing an inherited rule or constraint. Nothing it does reaches back into `town`.

A child is not a copy of its parent plus a diff. It is a **set of instructions applied to whatever its parent is**. That one idea answers the rest of this section.

### When a parent changes

Edits flow forward. When `town` changes, every world built on it runs again on the new `town`, and every world holding it sees the new state.

Because a child is instructions, most of it keeps its meaning:

- `harbor [ lives_in: bath ]` still sets bath, whatever town now says harbor's home is.
- `- harbor feels happy` does nothing if town no longer says it.
- `harbor knows robin` still adds.

Some lines can stop making sense. If town removes harbor, `harbor [ lives_in: bath ]` now points at nothing, and the child fails with an error that names the cause: "harbor doesn't exist (removed in town)". Grant never guesses what a broken line meant.

### Several parents

```
-- both
from move_bath, move_brighton
```

A world with several parents starts from their **union**: every kind, object, fact, rule and constraint that any parent has. Order doesn't matter: `from a, b` and `from b, a` are the same world.

- **Same origin, same object.** harbor in move_bath and harbor in move_brighton both come from town, so the child has one harbor carrying the facts of both.
- **Different origin, different objects.** If each parent created its own robin, the child has two robins, and `the` tells them apart.
- **Same name, same vocabulary.** A kind declared in both parents is one kind, and its shape has every relation either parent declared. Rules and constraints with the same name are one rule or constraint.
- **A merge keeps what either parent says.** If move_bath removed `harbor feels happy` and move_brighton still has it, the child has it. To remove it, the child removes it.

### Disagreement is settled at the end

A merge never fails on its own line. Parents can disagree in three ways, and all three are settled by the child's own lines and checked once, at the end of the world:

| Disagreement | Example | The child settles it with |
| --- | --- | --- |
| Facts break a constraint | harbor lives_in bath and brighton; `lives_in: one city` fails | `harbor [ lives_in: bath ]` |
| A rule or constraint has two definitions | `calm` differs between parents | `rule calm: …` (restating replaces both) |
| A relation is declared two ways | `age: one number` and `age: maybe text` | `kind person [ + age: one number ]` |

There is no merge strategy to learn. A conflict is something that doesn't fit, and you fix it the way you fix anything in Grant: by stating what is true. Until a disputed rule is restated, it produces nothing; if the world ends with any disagreement left, Grant names it.

### Example

```
-- town
from numbers, text

kind city
kind person [ age: one number | lives_in: one city ]

city york
city bath
city brighton
person harbor [ age: 24 | lives_in: york ]
```

```
-- move_bath
from town
harbor [ lives_in: bath ]
```

```
-- move_brighton
from town
harbor [ lives_in: brighton ]
```

```
-- both
from move_bath, move_brighton
-- harbor lives_in bath and brighton here: one city would fail at the end

harbor [ lives_in: bath ]
? harbor.lives_in             -- bath
```

Editing `town` to make harbor 25 changes all four worlds: every child runs again, and `? harbor.age` is 25 everywhere.

### No cycles

A world can't build on or hold itself, directly or indirectly. The graph of `from` and `world` has no loops.

---

## 12. Worlds as objects

A world relates to another world in one of two ways: it builds on it, or it holds it.

```
from town_a          -- this world is town_a, plus its own lines
world town_a         -- town_a is an object in this world
```

`world town_a` makes an object of kind `world`, the way `city york` makes a city. This world holds town_a; harbor lives inside town_a, not here. A held world can't be changed from outside: to change it, write a world that builds on it.

### True in a world, true about a world

- **In:** `in town_a: harbor feels happy` is true inside town_a. `in w:` switches where names resolve, so inside it `person` and `harbor` are town_a's.
- **About:** `town_a is_happier_than town_b` is a fact about two worlds. It lives in the world that holds them.

```
-- compare
from numbers

world town_a
world town_b

kind world [ + is_happier_than: world ]

rule happier: world a is_happier_than world b if
  count (in a: person p feels happy) > count (in b: person p feels happy)

? town_a is_happier_than town_b
? in town_a: harbor.knows
? town_a = town_b
```

`=` between worlds is equality of shape (section 3).

### The same object across worlds

Objects keep their identity in every world built from the one that created them. If town_c is built on town_a, harbor in both is the same harbor, so you can ask what changed for him:

```
? (in town_a: harbor feels happy) and not (in town_c: harbor feels happy)
```

Two worlds built separately have two unrelated harbors.

### Worlds of worlds

A world that holds worlds is itself a world, so another world can hold it:

```
-- earth
from numbers

world europe        -- holds towns, with is_happier_than between them
world asia

kind world [ + is_calmer_than: world ]

rule calmer: world x is_calmer_than world y if
  count (in x: world w is_happier_than world v) > count (in y: world w is_happier_than world v)
```

The relationships between towns become things to count by lifting the world they live in. No other mechanism is needed to treat relationships as objects.

Branch a world, change something, compare: the basic move of simulation and search. Every branch is its own world.

---

## 13. What each line does

Every statement changes K, O, F or R of its world. Derived facts follow after each line.

| Statement | Changes | Effect |
| --- | --- | --- |
| `from a, b` | K, O, F, R | starts the world as the union of its parents |
| `kind person [ … ]` | K | adds the kind and declares its relations, how many values each holds and of what kind |
| `kind person [ + r: k ]` / `[ - r ]` | K | declares a relation or sets how many and its value kind / removes it (refused while facts use it) |
| `- kind person` | K | removes the kind; refused if anything uses it |
| `person harbor [ … ]` | O, F | new object with its origin, kind, handle and index; one fact per bracket value |
| `harbor r v` | F | adds (harbor, r, v) |
| `harbor [ r: v ]` | F | sets: removes every stated (harbor, r, ·), then adds (harbor, r, v) |
| `harbor [ r: + v, - w ]` | F | adds v, removes w |
| `each person p where … [ … ]` | F | the same change on every match |
| `- harbor r v` | F | removes the stated (harbor, r, v); it stays true if a rule still produces it |
| `- harbor r` | F | removes every stated (harbor, r, ·) |
| `- each person p r` | F | removes r from every match |
| `- harbor` / `- each person p where …` | O, F | removes the objects and every fact with them as subject or value |
| `rule n: …` / `never n: …` | R | adds, or replaces the one named n |
| `- rule n` / `- never n` | R | removes it |
| `world town_a` | O | an object of kind `world` holding the world town_a |
| `? …` | nothing | evaluates against M = F ∪ D and shows the result |

At the end of the world, every `never`, every word in a shape and every disagreement left by a merge is checked against the final structure. That structure, not the order of lines, is the world.

---

## 14. A full world

```
-- harbor

from numbers, text

kind city
kind feeling
kind condition

kind person [
  age:              one number
  lives_in:         one city
  knows:            person
  feels:            feeling
  is_at:            condition
  is_neighbour_of:  person
]

city york
city bath
feeling happy
condition peace

person liz    [ age: 27 | lives_in: york ]
person laurie [ age: 30 | lives_in: bath ]
person harbor [ age: 24 | lives_in: york | knows: liz, laurie ]

harbor feels happy

rule calm:       person p is_at peace if p feels happy
rule mutual:     person a knows person b if b knows a
rule neighbours: person a is_neighbour_of person b if a.lives_in = b.lives_in and a != b

? harbor is_at peace                -- yes
? liz.knows                         -- harbor
? person p is_neighbour_of harbor    -- liz

harbor [ lives_in: bath | age: harbor.age + 1 ]
- harbor knows liz
- harbor feels happy

? harbor is_at peace                -- no
? harbor.age                        -- 25
? person p is_neighbour_of harbor    -- laurie
```

---

## 15. Errors

| When | Grant says |
| --- | --- |
| A world name doesn't resolve | no world called `town_x` here |
| A parent has errors | `town` has errors; fix it first |
| A name is used before it is created | `york` doesn't exist yet |
| A name pointed at something a parent removed | harbor doesn't exist (removed in town) |
| A bare handle fits several objects | 2 objects called harbor; use `the` |
| `the` fits none or several | no / 2 people match this description |
| A relation isn't in the kind's shape | person has no relation `favourite_colour` |
| A value is the wrong kind | `lives_in` takes a city, not a feeling |
| A rule produces a relation its subject's kind doesn't declare | person doesn't declare `is_at` |
| A rule variable shares an object's name | `harbor` is already an object |
| A rule depends on its own negation or aggregate | rule `popular` counts itself |
| A line is only a value | this value isn't used |
| A missing value is computed with | harbor has no `age` |
| Dividing by zero | division by zero |
| A constraint is broken at the end of the world | harbor has 2 `lives_in` (person: `lives_in: one city`) |
| A merge disagreement is left at the end | rule `calm` differs between move_bath and move_brighton; restate it |
| `- kind` or a shape removal while still in use | 3 people still use `person` |
| A held world is changed | town_a is held, not built on; write a world `from town_a` |
| A world builds on or holds itself | town_a → europe → town_a |

Not an error, but reported: removing a stated fact that a rule still produces ("still follows from rule mutual").

---

## 16. Grammar summary

An informal summary. Lowercase words are names; what a name means (kind, handle, relation, variable, world name) comes from where it stands and what has been declared.

```
world      = [ "from" world_name { "," world_name } ] { statement }

statement  = kind_decl | kind_edit | "-" "kind" kind
           | object | fact | set | removal | each
           | rule | never | "-" "rule" name | "-" "never" name
           | "world" world_name
           | "?" ( condition | expr )

kind_decl  = "kind" kind [ "[" field { sep field } "]" ]
kind_edit  = "kind" kind "[" edit { sep edit } "]"
edit       = "+" field | "-" relation
field      = relation ":" [ "one" | "maybe" | "some" | "many" ] kind

object     = kind handle [ values ]
fact       = [ "+" ] ref relation expr
set        = ref values
removal    = "-" ref [ relation [ expr ] ]
each       = [ "-" ] "each" kind var [ "where" condition ] [ values | relation ]
values     = "[" assign { sep assign } "]"
assign     = relation ":" ( expr { "," expr } | change { "," change } )
change     = ( "+" | "-" ) expr

rule       = "rule" name ":" kind var relation term "if" condition
never      = "never" name ":" kind var "where" condition

ref        = handle | "the" [ kind ] handle values
condition  = item { ( "and" | "or" ) item }
item       = "not" item | "(" condition ")" | "in" world_name ":" condition
           | kind var                          -- any object of a kind
           | term relation term                -- a fact pattern
           | expr ( "=" | "!=" | "<" | ">" | "<=" | ">=" ) expr
term       = kind var | var | ref | expr
expr       = number | text | ref | var | "id" "." number
           | expr "." relation
           | expr ( "+" | "-" | "*" | "/" | "div" | "mod" ) expr
           | "count" ( expr | "(" condition ")" ) | ( "sum" | "min" | "max" ) expr
           | "in" world_name ":" expr | "(" expr ")"
sep        = "|" | newline
```

- One statement per line. An indented line continues the statement above it; brackets can span lines.
- `--` starts a comment that runs to the end of the line.
- A `-` at the start of a line is removal; anywhere else it is minus.

---

## 17. Decisions in Foundation 1

| Question | Decision |
| --- | --- |
| What does a child store? | Its lines. It is re-run on its parents whenever they change. |
| What happens to a child line a parent change breaks? | It fails with an error that names the parent change. |
| How do several parents merge? | Union. Order doesn't matter. |
| What is a merge conflict? | Anything that doesn't fit at the end of the world: a broken constraint, a rule defined two ways, a relation declared two ways. The child settles it with ordinary lines. |
| What identifies an object? | Its origin: the world that created it and its place there. |
| What identifies a kind, relation, rule or constraint? | Its name. |
| Does equality compare names? | No. Equality compares shape. |
| Is the index per kind or per world? | Per kind, and it can repeat after a merge. It numbers; it never identifies. |

## 18. Beyond the foundation

- **Standard worlds written in Grant:** time and calendars, ordered collections, common constraints. They are ordinary worlds, built only from the primitives.
- **The interpreter:** a Rust implementation that runs worlds, re-runs children when parents change, and reports errors with their cause.
- **Comparing large worlds:** equality of shape is a matching problem. Worlds that share an ancestor can be compared by origin first, which is fast; unrelated worlds need the full match.

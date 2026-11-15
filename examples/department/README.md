# A department, in ten worlds

A mathematics department plans its teaching. The project is written from nothing: it starts
with the words, adds courses, people and rooms in separate worlds, joins them, proposes two
timetables, merges them, judges the results from a world that holds them, and then lets a
year pass.

```
                 vocabulary                    kinds and their shapes; no things yet
                 /    |    \
        catalogue     |     campus             courses and prerequisites · rooms and hours
            |   \     |       |
            |    people       |                lecturers and students
             \      |        /
               department                      the union, and what follows from it
               /        \
     plan_morning      plan_spread             two timetables that disagree
               \        /
               plan_merged                     both at once, then a decision
               /        \
        next_year      review                  a year later · the three plans held as objects
```

Run any of them from this folder:

```sh
grant department
grant review
grant watch review      # then edit people.grant, or a plan, and watch the verdict move
```

## What each world shows

| World | Shows |
| --- | --- |
| `vocabulary` | A world that only declares. Kinds, shapes with `one`, `some` and `maybe`, flags as values (`role afternoon`), a rule over numbers. |
| `catalogue` | Objects and facts in brackets. A recursive rule (`needs` is everything a course requires, all the way back). A constraint that forbids circular prerequisites. Sums over a chain of dots. |
| `campus` | A second world on the same vocabulary, written without knowing about the first. |
| `people` | A world with two parents. Brackets on one line for short entries, one entry per line for long ones. |
| `department` | Three parents with a common ancestor become one world: same kinds by name, same objects by origin. Shapes extended with `+`. Negation inside negation (`may_take`), a rule whose value is counted (`demand`), a rule whose value is summed (`load`), and five constraints that are only checked when a world ends. |
| `plan_morning`, `plan_spread` | Two children of the same parent. Sentences set in brackets. Each defines `overloaded` its own way. |
| `plan_merged` | A merge. Every course carries the facts of both plans until the lines settle it; the rule defined two ways produces nothing until it is restated. |
| `review` | Worlds as objects. Rules that count inside held worlds with `in`, a verdict built from two orderings, the same lecturer followed across worlds by origin, and equality of shape. |
| `next_year` | Changing a world: `each … [ r: + … ]` computed once, `- each`, removing objects, and constraints that hold again at the end. |

## Things to try

- In `people.grant`, let dario pass `group_theory`. He is no longer held back from Galois
  Theory, its demand rises, and every world below is run again.
- In `campus.grant`, give `seminar` 1 seat. Both plans now end with `too_small`, and
  `review` reports that `plan_morning` has errors.
- In `plan_merged.grant`, delete the line `rule overloaded: …`. The world ends by naming the
  disagreement: "rule `overloaded` differs between plan_morning and plan_spread; restate it".
- At the end of `catalogue.grant`, add `analysis_1 requires differential_geometry`. The
  catalogue ends with every course on the circle breaking `circular`.
- Write a world that holds `review` and another review of different plans, and compare them:
  the relations between plans become things to count, one level up.

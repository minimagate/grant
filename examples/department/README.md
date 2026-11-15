# A department, in eleven worlds

A mathematics department plans its teaching. The project is written from nothing: it starts
with the words, adds names, courses, people and rooms in separate worlds, joins them, proposes two
timetables, merges them, judges the results from a world that holds them, and then lets a
year pass.

```
                 vocabulary                    kinds and their shapes; no things yet
                 /    |    \
        catalogue     |     campus             courses and prerequisites · rooms and hours
            |       names     |                names, and who bears each
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

## The worlds, one by one

The files carry no comments; this is what each one does.

### `vocabulary`

Only declares. It builds on `numbers` and states the kinds everything else shares: `area`, `role`, `subject`, `room`, `slot`, `course` and `person`, each with its shape. Nothing exists yet except three roles, `lecturer`, `student` and `afternoon`, which are flags: values a relation can hold. One rule marks a slot as afternoon when its hour is 14 or later.

What things share is a thing, not a text, and a fact is stated from one end. Every shared thing reads its other end with `by`: a subject's `course: course by subject` is the course that teaches it, an area's `course: course by area` its courses, and a role's `person: person by is` and `slot: slot by is` who and what has it. Nothing is stated twice: `area@algebra.course` is every algebra course, and `role@lecturer.person` every lecturer.

A course has `requires`, which is stated, and `needs`, which is derived. Both are declared here, because stated and derived facts share a shape.

### `names`

Built on `vocabulary`. A name is a thing, not a text: the kind `name`, ten names written in small letters, and a new relation on people, `name: one name`. A person has one name, and a name can belong to several people.

A name reads its other end with `person: person by name`. So `noether.name` is emmy, and `name@emmy.person` is everyone called emmy: `@` goes from the kind to the name, `.` from the name along `person`.

Five students share a handle with their name: there is a person ada and a name ada. Where the place settles the kind, the handle needs nothing more: `name: ada` takes a name, and `ada [ wants: … ]` is about whoever has `wants`. Where nothing settles it, the kind is written: `next_year` removes `person@claire`. Subjects share handles with their courses, and areas with subjects, in the same way.

### `catalogue`

Four areas, eight subjects and eight courses, each course with its subject, credits, area and what it requires. A subject is a thing, not a text, so `subject@logic.course` is the course that teaches logic: logic_1.

- `rule needs` is recursive: a course needs what it requires, and what those need in turn.
- `never circular` forbids a course from standing, however far back, behind itself.

| Question | Shows |
| --- | --- |
| `? differential_geometry.needs` | linear_algebra, analysis_1, analysis_2, topology |
| `? course c and not c requires course p` | linear_algebra, analysis_1, logic_1: where a first-year student can start |
| `? sum galois_theory.needs.credits` | 15: the credits to earn before Galois Theory |
| `? area@algebra.course` | linear_algebra, group_theory, galois_theory: read from `area` |
| `? subject@logic.course` | logic_1 |

### `campus`

Three rooms with their seats and eight slots, each with a day (1 is Monday) and an hour. It is written on the same vocabulary as the catalogue without knowing about it. Its question shows the afternoon slots: mon_14, tue_14.

### `people`

Built on `names` and `catalogue`. Four lecturers with the areas they can teach, and five students with what they have passed and what they want. Every object is written with one relation per line.

| Question | Shows |
| --- | --- |
| `? noether.name` | emmy |
| `? name@emmy.person` | noether: read from `name` |
| `? role@lecturer.person` | noether, cauchy, riemann, godel |

### `department`

The union of `campus`, `people` and `catalogue`. All three build on `vocabulary`, so their kinds are the same kinds and their objects the same objects; the order of parents doesn't matter.

It extends two shapes with `+`: people get `may_take`, `teaches`, `load` and `clashes_on`; courses get `taught_by`, `meets_in`, `meets_at` and `demand`. It also creates the flag `overloaded`, which each plan will define.

| Rule | Says |
| --- | --- |
| `may_take` | a student may take a course they want once nothing it needs is left to pass (a `not` inside a `not`) |
| `demand` | a course's demand is the count of students who may take it |
| `teaches` | the other direction of `taught_by` |
| `load` | a lecturer's load is the sum of the credits they teach |
| `clash` | a student clashes on a slot when two courses they may take meet then |

| Constraint | Forbids |
| --- | --- |
| `unqualified` | a course taught by someone who can't teach its area |
| `room_taken` | two courses in the same room at the same hour |
| `two_places` | one lecturer teaching two courses at the same hour |
| `too_small` | a room with fewer seats than the course's demand |

Constraints are checked when a world ends, so a plan can be written in any order and pass through broken states on the way.

| Question | Shows |
| --- | --- |
| `? person s wants course c and not s may_take c` | devon, galois_theory: he hasn't passed group theory |
| `? course c and c.demand = 0` | nothing |

### `plan_morning` and `plan_spread`

Two children of `department`. Each gives every course a lecturer, a room and a slot, adds the constraint `untaught` (a course in demand with no lecturer), and defines `overloaded` its own way.

| | `plan_morning` | `plan_spread` |
| --- | --- | --- |
| Hours | everything before lunch | two courses in the afternoon |
| Analysis II | riemann, mon_9 | cauchy, mon_14 |
| Clashes | devon on tue_9, ellie on wed_9 | none |
| `overloaded` means | load above 24 | load above 20 |
| Overloaded | riemann (30) | noether (21), cauchy (24) |

### `plan_merged`

Built on both plans. Every course is the same course in both, so it carries the facts of both: four courses have two hours and Analysis II has two lecturers. Nothing fails; the first two questions show the disagreement.

Four lines settle it by stating what is true, taking the hours of `plan_spread`. The parents also define `overloaded` two ways. Until it is restated it produces nothing, which is why the question before the restated rule shows nothing. After it, with the threshold at 24, nobody is overloaded: cauchy carries exactly 24.

The last questions show no clashes and the loads: noether 21, cauchy 24, riemann 18, godel 6.

### `review`

Holds the three plans as objects with `world`. A plan can't be changed from here; it can only be looked into with `in`.

| Rule | Says |
| --- | --- |
| `clashes` | plan a clashes less than plan b when fewer student clashes are counted inside it |
| `spares` | plan a spares more than plan b when fewer lecturers are overloaded inside it |
| `best` | a plan is best when no other plan beats it on either count |

| Question | Shows |
| --- | --- |
| `? world a clashes_less_than world b` | plan_spread and plan_merged each beat plan_morning |
| `? world a spares_more_than world b` | plan_morning beats plan_spread; plan_merged beats both |
| `? world w is best` | plan_merged |
| `? in plan_morning: person s clashes_on slot k` | devon on tue_9, ellie on wed_9 |
| `? in plan_spread: cauchy.load` | 24 |
| `? (in plan_morning: riemann is overloaded) and not (in plan_merged: riemann is overloaded)` | yes: the same riemann, by origin, in two worlds |
| `? plan_merged = plan_spread` | no: the same hours, but a different rule |
| `? plan_merged = plan_merged` | yes |

### `next_year`

Built on `plan_merged`. It is instructions applied to its parent: if the plans change, this world is run again on whatever they have become.

1. Every student passes what they were ready to take (`each … [ passed: + s.may_take ]`, computed once), and their wishes are cleared.
2. claire and ellie graduate, godel retires, and felix arrives.
3. The remaining students state new wishes.

The first question finds a course in demand with no lecturer, logic_1, which would break `untaught` at the end; the next line gives it to noether. Then felix is held back from group theory, noether's load is 27, and nobody clashes.

## Things to try

- In `people.grant`, let devon pass `group_theory`. He is no longer held back from Galois
  Theory, its demand rises, and every world below is run again.
- In `campus.grant`, give `seminar` 1 seat. Both plans now end with `too_small`, and
  `review` reports that `plan_morning` has errors.
- In `plan_merged.grant`, delete the line `rule overloaded: …`. The world ends by naming the
  disagreement: "rule `overloaded` differs between plan_morning and plan_spread; restate it".
- At the end of `catalogue.grant`, add `analysis_1 requires differential_geometry`. The
  catalogue ends with every course on the circle breaking `circular`.
- Write a world that holds `review` and another review of different plans, and compare them:
  the relations between plans become things to count, one level up.

# Examples

The worlds here carry no comments: each is explained below. Run one with `grant examples/harbor`.

| Example | What it is |
| --- | --- |
| [`harbor.grant`](#harborgrant) | one world using most of the language |
| [`towns/`](#towns) | a parent, two children that disagree, and their merge |
| [`compare.grant`](#comparegrant) | worlds held as objects and compared |
| [`calendar.grant`](#calendargrant) | dates built on `numbers` |
| [`department/`](department) | a whole project in eleven worlds, with its own README |

## `harbor.grant`

The full world of section 14 of the specification, with a few additions.

It builds on `numbers`, `text` and `id`, declares cities, feelings, conditions and people, and creates three people in two cities. Six rules then make facts follow:

| Rule | Says |
| --- | --- |
| `calm` | whoever feels happy is at peace |
| `mutual` | knowing goes both ways |
| `adult` | whoever is 18 or older is an adult |
| `neighbours` | two different people in the same city are neighbours |
| `reach` | you can reach whoever you know, and whoever they can reach |
| `lonely` | whoever knows nobody is lonely |

The constraint `sensible_age` forbids ages below 0 or above 150.

The questions come in two groups, with a change between them.

| Question | Shows | Why |
| --- | --- | --- |
| `? harbor is_at peace` | yes | harbor feels happy, and `calm` follows |
| `? liz.knows` | harbor | nobody stated it; `mutual` derives it |
| `? person p is_neighbour_of harbor` | liz | both live in york |
| `? harbor.id` | 3 | harbor is the third person created |
| `? id.1` | york, happy, peace, liz | the first object of each kind |
| `? person p and p.id < 3` | liz, laurie | |
| `? sum harbor.knows.age / count harbor.knows` | 28.5 | an average is built, not given |
| `? liz can_reach laurie` | yes | through harbor, by recursion |
| `? person p is lonely` | nothing | everyone knows someone |

Then harbor moves to bath and turns 25 (`harbor [ lives_in: bath | age: harbor.age + 1 ]`), stops knowing liz, and stops feeling happy.

| Question | Shows | Why |
| --- | --- | --- |
| `? harbor is_at peace` | no | the fact it followed from is gone |
| `? harbor.age` | 25 | computed once, on its line |
| `? person p is_neighbour_of harbor` | laurie | he lives in bath now |
| `? person p is lonely` | liz | she only knew harbor through `mutual` |

## `towns/`

Four worlds that show building and merging.

| World | Does |
| --- | --- |
| `town` | declares cities, feelings and people; harbor lives in york |
| `move_bath` | builds on `town`; moves harbor to bath and makes him happy |
| `move_brighton` | builds on `town`; moves harbor to brighton |
| `both` | builds on both moves |

`both` starts as the union of its parents. harbor is the same harbor in each (same origin), so he carries the facts of both and lives in bath and brighton. The first question shows that: `bath, brighton`. The shape `lives_in: one city` would fail when the world ends, so `both` settles it with `harbor [ lives_in: bath ]`, and the second question shows `bath`.

Try `grant watch examples/towns/both` and edit `town.grant`: every world built on it is run again.

## `compare.grant`

Worlds as objects. It holds `move_bath`, `move_brighton` and `town` with `world`, gives the kind `world` a relation `is_happier_than`, and derives it with a rule that counts happy people inside each world.

| Question | Shows | Why |
| --- | --- | --- |
| `? move_bath is_happier_than move_brighton` | yes | harbor is happy only in `move_bath` |
| `? world w is_happier_than town` | move_bath | |
| `? in move_bath: harbor.lives_in` | bath | names inside `in` are that world's |
| `? in town: person p feels happy` | liz | |
| `? move_bath = move_brighton` | no | equality compares shape, and the facts differ |
| `? town = town` | yes | |
| `? (in move_bath: harbor feels happy) and not (in town: harbor feels happy)` | yes | the same harbor, by origin, in two worlds |

## `calendar.grant`

Time is not a primitive. A date is a number of days, and a calendar is a world built on `numbers` with `div` and `mod`.

A `date` states one thing, `days` (days since 1 January 1970). Nine rules derive the rest: the weekday from `(days + 3) mod 7` (day 0 was a Thursday, and the 3 makes Monday 0), then the era, the day and year within the era, and from those the day, month and year. The rules chain: `year` reads `month`, which reads `mp`, which reads `doy`, and so on back to `days`.

For `date today [ days: 20771 ]` the questions show 2026, 11, 14 and saturday.

## `department/`

A mathematics department plans its teaching, in eleven worlds: vocabulary, names, catalogue, campus, people, their union, two timetables, their merge, a review that holds the plans as objects, and the following year. See [its README](department).

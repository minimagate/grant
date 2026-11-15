# Examples

| World | Shows |
| --- | --- |
| `harbor.grant` | The full world of section 14: kinds, facts, rules, a constraint, indexes, changes |
| `towns/town.grant` | A parent world |
| `towns/move_rome.grant`, `towns/move_florence.grant` | Two children that disagree |
| `towns/both.grant` | A merge, settled by stating what is true |
| `compare.grant` | Worlds held as objects, compared with a rule, looked into with `in` |
| [`department/`](department) | A whole project in ten worlds: a department plans its teaching, merges two timetables, judges them and lets a year pass |
| `calendar.grant` | Time built on `numbers`: a date is a number of days |

Run one with `grant examples/harbor`. Edit `towns/town.grant` while `grant watch examples/towns/both`
is running to see edits flow forward.

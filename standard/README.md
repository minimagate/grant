# Standard worlds

Ordinary things that are tedious to state again: weekdays and months, dates, the clock, durations, units, currencies, order. Each is an ordinary world, written only with Grant's primitives, and a world gets one by building on it.

```grant
from ../standard/second/calendar

date launch [ year: 2026 | month: 11 | day: 16 ]

? launch.falls_on        -- monday
? launch.week            -- 47
```

They come in two orders. The primitive worlds `numbers`, `text` and `id` are the first.

| Order | Folder | Builds on |
| --- | --- | --- |
| second | [`second/`](second) | the primitive worlds only |
| third | [`third/`](third) | second-order worlds |

A world of your own that builds on any of them can be built on in turn, so the tower goes as high as you take it.

## Second order

| World | Gives |
| --- | --- |
| [`calendar`](#calendar) | weekdays, months, quarters, and dates that know their year, month, day, weekday and week |
| [`clock`](#clock) | times of day, in minutes, on the 24 and the 12 hour clock |
| [`durations`](#durations) | lengths of time stated in weeks, days, hours, minutes and seconds |
| [`measures`](#measures) | 53 units in 8 dimensions, and quantities converted between them |
| [`money`](#money) | 24 currencies, and sums of money converted between them |
| [`sequence`](#sequence) | places in an order: next, before, after, first, last and rank |

### `calendar`

A date is stated either way and read both.

```grant
date launch   [ year: 2026 | month: 11 | day: 16 ]
date deadline [ days: launch.days + 45 ]
```

`days` counts from 1 January 1970, which is day 0. Stating the year, month and day derives `days`; stating `days` derives the other three. Arithmetic on dates is arithmetic on `days`, and `a.days < b.days` is "a comes before b".

| A date's | Is |
| --- | --- |
| `days`, `year`, `month`, `day` | numbers; state `days`, or the other three |
| `falls_on` | its weekday, `monday` to `sunday` |
| `falls_in` | its month, `january` to `december` |
| `quarter` | `first_quarter` to `fourth_quarter` |
| `week` | its ISO week number, 1 to 53 |
| `day_of_year` | 1 to 366 |
| `in_a` | `common_year` or `leap_year` |
| `year_days`, `month_days` | how long its year and its month are |

The rest of the shape (`era`, `day_of_era`, `year_of_era`, `day_from_march`, `month_from_march`, `march_year`, `new_year`, `rough_week`) is working: steps of the conversion, declared because shapes are closed.

A weekday has a `number` (monday is 1), `next`, `previous`, and is a `workday` or the `weekend`. A month has a `number`, its `days` in a common year, its `quarter`, `next` and `previous`. Each reads its dates with `by`: `monday.date` is every date that falls on a Monday, `weekend.weekday` is saturday and sunday, `fourth_quarter.month` its three months.

The weekend is stated, not derived, so a child changes it by stating otherwise: `friday [ is: weekend ]`.

`date_on_the_calendar` refuses 30 February and month 13; `date_whole_day` refuses half a day.

### `clock`

```grant
time opening [ hour: 9 | minute: 30 ]
time closing [ since_midnight: opening.since_midnight + 465 ]
```

A time is stated as `hour` and `minute`, or as minutes `since_midnight`, and read as either. `hour_12` is the hour on the 12 hour clock, and a time `is` `am` or `pm`, and `night` (before 6), `morning`, `afternoon` (from 12) or `evening` (from 18). `pm.time` is every afternoon and evening time.

`time_within_a_day` and `time_whole_minute` keep a time between 00:00 and 23:59.

### `durations`

```grant
duration shift [ hours: 7 | minutes: 45 ]
```

State any of `weeks`, `days`, `hours`, `minutes` and `seconds`; what is left out counts as nothing. The total is read in one unit, exactly (`shift.in_hours` is 7.75): `in_seconds`, `in_minutes`, `in_hours`, `in_days`, `in_weeks`. It is also read as it would be said: `whole_days`, `and_hours`, `and_minutes`, `and_seconds`.

### `measures`

```grant
quantity marathon [ amount: 26.21875 | unit: mile | as: kilometre ]

? marathon.converted     -- 42.194988
```

A unit `measures` one dimension and has a `symbol`, a `factor` to the base unit of its dimension and, for temperatures, a `shift`. A quantity has an `amount` and a `unit`; `base` is the amount in the base unit, so quantities in different units compare with `a.base < b.base`. With `as`, `converted` is the amount in that unit. Numbers are exact, so conversions are too.

| Dimension | Base | Units |
| --- | --- | --- |
| `length` | metre | millimetre, centimetre, metre, kilometre, inch, foot, yard, mile, nautical_mile |
| `mass` | kilogram | milligram, gram, kilogram, tonne, ounce, pound, stone |
| `volume` | litre | millilitre, centilitre, litre, cubic_metre, us_fluid_ounce, us_pint, us_gallon, imperial_pint, imperial_gallon |
| `area` | square_metre | square_metre, hectare, square_kilometre, square_foot, acre, square_mile |
| `temperature` | kelvin | kelvin, celsius, fahrenheit |
| `timespan` | second | second, minute, hour, day, week |
| `speed` | metre_per_second | metre_per_second, kilometre_per_hour, mile_per_hour, knot |
| `data` | byte | bit, byte, kilobyte to terabyte, kibibyte to tebibyte |

A new unit is one line: `unit furlong [ measures: length | symbol: "fur" | factor: 201.168 ]`. `quantity_same_dimension` refuses miles as kilograms.

### `money`

```grant
eur [ worth: 1 ]
usd [ worth: 0.8 ]

money ticket [ amount: 120 | currency: usd | as: eur ]

? ticket.converted       -- 96
```

A currency has its ISO `code`, a `symbol`, and `minor`: how many of its smallest unit make one (100 for the euro, 1 for the yen, 1000 for the Kuwaiti dinar). The 24 are eur, usd, gbp, chf, jpy, cny, cad, aud, nzd, hkd, sgd, sek, nok, dkk, pln, czk, huf, inr, krw, brl, mxn, zar, try and kwd.

Rates change, so none is given. A world states what each currency it uses is `worth` in whichever it reckons in; then a sum of money has a `worth`, and with `as`, a `converted` amount. `in_minor` is the amount in the smallest unit.

`money_finer_than_minor` refuses half a cent; remove it (`- never money_finer_than_minor`) where prices are finer. `money_as_needs_worth` refuses a conversion between currencies with no stated worth.

### `sequence`

```grant
place gold
place silver
place bronze
gold   [ next: silver ]
silver [ next: bronze ]

kind runner [ finished: one place ]
```

Grant has no lists; order is a relation. A place states only what comes `next`. From that follow `previous`, everything `after` and `before` it, its `rank` (1 for the first), and which places are `first` and `last`. Your own things take their order by pointing at a place: `r.finished before ada.finished`.

`place_circle` refuses an order that returns to itself, `place_fork` a place with two before it.

## Third order

| World | Builds on | Gives |
| --- | --- | --- |
| [`moments`](#moments) | calendar, clock | a date and a time together, in a time zone |
| [`agenda`](#agenda) | calendar, clock, durations | events that overlap, and weekly routines |
| [`workweek`](#workweek) | calendar | holidays, working days and days off |
| [`ledger`](#ledger) | calendar, money | accounts, transfers between them, and balances |

### `moments`

```grant
zone rome [ offset: 60 ]
moment sent [ on: launch | at: opening | zone: rome ]
```

A moment is `on` a date and `at` a time. `local` is its minutes since the start of 1970 as its own clock shows them, and `universal` the same in UTC, so moments in different zones compare with `a.universal < b.universal`. A zone is an `offset` in minutes ahead of UTC; only `utc` is given, because offsets change with the season and the law.

### `agenda`

```grant
duration one_hour [ hours: 1 ]
event review  [ on: launch | starts: opening | lasts: one_hour ]
routine gym   [ every: monday, thursday | starts: opening | lasts: one_hour ]
```

An event is `on` a date, `starts` at a time and `lasts` a duration. `begins` and `finishes` are minutes since the start of 1970, so an event can run past midnight, and `overlaps` holds between two events that share any time. Events that only touch don't overlap.

A routine happens `every` given weekday. It `falls_on` each date of the world that is such a day, and `collides_with` each event it shares time with.

### `workweek`

```grant
feast christmas [ month: 12 | day: 25 ]
monday_28 is holiday
```

A feast is a holiday that returns every year on the same month and day; a single day off is stated on its date. A date is a `working_day` when it falls on a workday and is no holiday, and a `day_off` otherwise. `working_day.date` reads them back. No feasts are given: they differ from country to country.

### `ledger`

```grant
account current  [ currency: eur ]
account landlord [ currency: eur ]
transfer rent [ on: first_of_month | amount: 900 | paid_by: current | paid_to: landlord ]
```

Every transfer leaves one account and reaches another, so nothing is stated twice and the balances of a closed set of accounts sum to nothing. An account's `balance` is what came in minus what went out; `incoming` and `outgoing` read its transfers.

Four constraints: an amount is above zero, an account doesn't pay itself, both accounts hold the same currency, and no amount is finer than the currency's smallest unit.

## Using them

**By path.** Names resolve from a world's own folder, and `..` climbs out of it: `from ../standard/second/calendar` from a folder beside this one. Copying `standard/` next to a project works as well.

**Several at once.** Every standard world can be built on together with every other: [`examples/everything.grant`](examples/everything.grant) builds on all ten. Kinds are shared by name, so `calendar` reached through `agenda` and through `ledger` is one calendar.

**Changing one.** A standard world is a parent like any other. A child adds relations to its kinds (`kind date [ + deadline_of: project ]`), replaces a rule by restating it, and removes a constraint it doesn't want.

**Names taken.** Rules and constraints are identified by name across a whole merge, so every one here starts with its kind: `date_week`, `time_morning`, `transfer_positive`. A rule of yours with the same name replaces it. The handles of the things given (`monday`, `metre`, `eur`, `first`, `day`, …) can't also be kinds or rule variables in a world that builds on them.

**Finite worlds.** Nothing exists unless it is stated, so a calendar holds the dates you state and no others: `monday.date` is every Monday of the world, not every Monday there is. What a rule computes is a number (`launch.days + 45`); to read that number as a date, state a date with it.

**How many dates.** A date read both ways is a group of rules that feed each other, and the interpreter stops such a group once the world holds 20,000 derived facts ([decision 15](../docs/decisions.md)). A date has about seventeen, so a world reaches the limit a little past 1,100 dates and then fails with "rule `date_…` never settles". The rules do settle; the limit counts size, not rounds.

## Examples

[`examples/`](examples) has one world for each group. They are run by `cargo test`, which checks every answer.

| World | Builds on | Shows |
| --- | --- | --- |
| `dates` | calendar | dates stated both ways, weeks, quarters, leap years |
| `times` | clock, durations | a closing time computed from an opening time and a shift |
| `race` | sequence, measures | a podium, and conversions of length, temperature and data |
| `prices` | money | sums converted through stated rates |
| `week` | agenda, moments, workweek | overlaps, a routine, two time zones, holidays |
| `books` | ledger | salary, rent and savings, and balances that sum to nothing |
| `everything` | all ten | that they merge without disagreement |

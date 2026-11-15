# Decisions made while implementing Foundation 1

The specification is [`foundation-1.md`](foundation-1.md). Building it raised questions the text doesn't answer, and a few places where two of its statements pull against each other. Each was settled by going back to the six principles. This file lists every such decision, so each can be confirmed or changed in Foundation 2.

A decision marked **(extension)** adds something the grammar doesn't list.

## Values

1. **Things are counted once; numbers and text once per thing that has them.** `harbor.knows.knows` is a set of people, so someone two friends both know is one person. `harbor.knows.age` is one age per person harbor knows, so two friends aged 27 give `sum` 54 and `count` 2. With plain sets of values, the average of section 7 (`sum x / count x`) would be wrong whenever two values coincide. A question shows each distinct value once.

2. **Missing values fail a line but not a condition.** `harbor [ age: harbor.age + 1 ]` and `? harbor.age + 1` are errors when harbor has no age, as section 7 says. Inside a condition the comparison just doesn't hold. Rules are live after every line, so `rule adult: … if p.age >= 18` must not break in the moment between `person harbor` and `harbor age 24`.

3. **A comparison holds if some pair of values makes it hold**, so `harbor.knows = liz` is true when liz is one of the values. `!=` is the exception: it holds when both sides have values and no pair is equal, so `=` and `!=` are never both true. Every comparison but `!=` is monotone, which lets rules use dots freely without becoming circular; a rule that reads its own relation under `!=` is refused like one that reads it under `not`.

4. **`min` and `max` of nothing** are an error on a line and false in a condition. `sum` of nothing is 0 and `count` of nothing is 0.

5. **Numbers are exact rationals** held in 128 bits each for numerator and denominator. A result that doesn't fit is an error ("this number is too large to keep exact"), never a rounded value. Terminating decimals print as decimals (`28.5`), everything else as a fraction (`1/3`). `div` rounds down and `mod` takes the sign of the divisor, also for negative and fractional numbers, which is what calendars need.

6. **Text orders alphabetically**: letters first whatever their case (`"alice" < "Zed"`), and capitals before small letters only to break a tie. Text is shown in quotes, to tell `"york"` from the object york.

## Reading a line

7. **`-2` and `- 2`.** Section 10 gives `-` inside brackets to removal, and section 4 has negative literals. A `-` written directly before a digit, and not directly after a value, is a negative number: `[ balance: -5 ]` sets, `[ balance: - 5 ]` removes. The same rule separates `person p age -1` (the age is minus one) from `harbor owes debt - 1` (debt minus one).

8. **Four names in a row** (`person p knows liz`, `liz knows person p`) can't be told apart by position. Grant reads the first name as a kind if it is one where the line runs; inside `in w:`, that means in w.

9. **`in w:` reaches to the end of the condition.** `? in town_a: a and b` asks both inside town_a. Parentheses close it: `(in town_a: a) and b`.

10. **After `the`, brackets are required** and may be empty: `the person harbor [ ]` picks by kind alone. A description fits an object that has the described values, stated or derived.

11. **`- each … where <condition> relation`**: when the condition ends in a name, wrap it in parentheses (`- each person p where (p knows liz) feels`). Otherwise the trailing relation reads as part of the condition.

12. **Indentation continues a statement**, as the grammar says. A file indented as a whole therefore reads as one long statement; statements start at the margin.

## Rules

13. **A rule depends on a relation together with its value, when the rule names one.** `rule minor: person p is minor if not p is adult` reads `is adult`, not `is minor`, so it doesn't depend on its own negation. Without this, the flag idiom of section 2 (`harbor is adult`) could never be negated.

14. **A rule holds the objects it names, not their names.** `adult` in a rule is the object that handle pointed at when the rule was stated (principle 5). Section 10 lets `-` remove any object, so removing one a rule or constraint names is allowed: the rule stays, matches nothing through that object, and Grant remarks on it ("rule `glad` names happy, and matches nothing through it now"). Worlds are compared the same way: two worlds are equal only if the objects their rules name correspond.

15. **Rules that compute their value can run forever** (`rule older: person p age p.age + 1 if p.age > 0`). Grant stops such a group after 1000 rounds or 20,000 derived facts: "rule `older` never settles". A text that a rule keeps joining to itself is refused once it passes a megabyte. Rules over existing objects always settle and have no limit.

16. **Variables.** A variable introduced inside `not` or inside `count ( … )` belongs to it and means "there is some". Introduced anywhere else, it belongs to the whole statement, and a question shows it. A variable can't share a name with an object or a kind. A `number` or `text` variable gets its values from a fact pattern (`p age number n`); it can't range over all numbers.

17. **A rule's head value may be computed** (`rule year: date d year d.yoe + d.era * 400`). `examples/calendar.grant` builds a calendar this way. **(extension)** A rule may omit `if`, and then holds for every object of its kind.

18. **A merge can bring together rules that depend on each other's negation.** A merge never fails on its own line, so such rules produce nothing, like a disputed rule, and the world must restate one before it ends.

## Changing a world

19. **Removing is lenient, and says so.** `- harbor knows ghost` does nothing when ghost isn't there, and Grant remarks "nothing removed: `ghost` isn't here", so a typo doesn't pass silently. Removing a fact that was never stated but follows from a rule is also reported.

20. **Restating a kind must agree with it.** `kind person [ age: maybe text ]` on a person that already declares `age: one number` is an error that points at `+`. Restating the same thing is stating it once.

21. **The end of the world also checks value kinds.** `kind person [ + age: one text ]` is accepted while people have numeric ages; if they still do when the world ends, Grant names them. This is how a relation declared two ways is settled after a merge.

22. **`each` computes everything before changing anything.** `- each … relation` reports what still follows from a rule, as `- harbor relation` does. `each person p [ age: p.age + 1 ]` reads every age first. **(extension)** `each person p where … relation value` adds a fact to each match, mirroring `- each … relation`, and `- each … relation value` removes one value from each.

23. **An object can't be named after a kind**, and a kind can't be named after an object. `number`, `text`, `id` and `world` are not the world's to redefine or remove.

## Worlds on worlds

24. **A world's identity** is given when its parents are known, so objects order parent first. Removed handles are remembered with the world that removed them, which is where "harbor doesn't exist (removed in town)" comes from.

25. **A parent's questions are run but not shown.** A question that fails in `town` fails `town`, whether it is run directly or built on.

26. **What a world showed before an error is still shown**, followed by the error.

27. **After a merge, a new object's index continues from the highest any parent reached.**

## Worlds as objects

28. **A held world is its own identity.** `world town_a` stated in two parents is one object after a merge, and stating it twice holds it once. `world towns/town_a` holds the world at that path under the handle `town_a`.

29. **Names inside `in w:` are w's**, looked up when the question is asked. Looking into a named world for a handle it doesn't have is an error ("`robin` doesn't exist in town"). When w is a variable ranging over held worlds, a world that lacks the handle or the kind simply has no matches.

30. **A variable keeps its object across worlds.** `(in town_a: person p feels happy) and not (in town_c: p feels happy)` asks about the same p in both, by origin.

31. **`=` between worlds is equality of shape; between anything else it is identity.** Equality compares the stated facts: with the same rules, the derived ones follow.

32. **A line starting with `in`** can only be an attempt to change a held world, and gets the error of section 15.

## `id`

33. **An index is shown and compared as a number.** `harbor.id` works on any object once the world is built `from id`. Comparing an index with a literal needs `from numbers` for the literal.

## Showing

34. **Things of one kind are shown in index order**; mixed kinds in order of creation. When several objects share a handle they are shown with kind and index, `harbor (person 4)`, and with the world that made them when the index repeats after a merge, `robin (person 2 from move_bath)`.

35. **A line may nest 120 levels deep** (parentheses, `not`, `count ( … )`). Deeper is refused: "this line nests too deeply".

36. **Inside brackets, a line ending in `,`, `:` or an operator continues** on the next line instead of ending the entry.

## Not built

- **Standard worlds** (time, ordered collections) are section 18's "beyond the foundation". `examples/calendar.grant` shows one way.
- **Derivation is naive**: a recursive group of rules is re-run until nothing new follows. It is correct and simple, and slow for long chains over thousands of objects. Semi-naive evaluation is the next step if worlds grow.
- **Comparing unrelated worlds** uses colour refinement and then backtracking. Highly symmetric worlds with thousands of objects can be slow to compare.

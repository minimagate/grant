//! From tokens to a statement.
//!
//! Grant reads names by where they stand, so the grammar needs no declarations to parse. The one
//! case position can't settle is four names in a row, which is kept as `Cond::Either`.

use crate::ast::*;
use crate::error::Res;
use crate::lexer::Tok;
use crate::value::Sym;

pub fn parse(tokens: &[Tok]) -> Res<Stmt> {
    let mut parser = Parser { t: tokens, i: 0, depth: 0 };
    let stmt = parser.statement()?;
    if parser.peek().is_some() {
        return Err(format!("{} doesn't belong here", parser.found()));
    }
    Ok(stmt)
}

struct Parser<'a> {
    t: &'a [Tok],
    i: usize,
    /// How many values and conditions deep the parser is.
    depth: usize,
}

/// Deeper nesting than this is refused rather than followed.
const DEPTH: usize = 120;
const TOO_DEEP: &str = "this line nests too deeply";

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }

    fn at(&self, n: usize) -> Option<&Tok> {
        self.t.get(self.i + n)
    }

    fn found(&self) -> String {
        self.peek().map_or("the end of the line".into(), |t| format!("`{t}`"))
    }

    /// Whether the next token is this keyword or mark.
    fn is(&self, sign: &str) -> bool {
        matches!(self.peek(), Some(Tok::Sign(s)) if *s == sign)
    }

    fn eat(&mut self, sign: &str) -> bool {
        let found = self.is(sign);
        self.i += found as usize;
        found
    }

    /// Eats whichever of these operators is next, and says which.
    fn any(&mut self, ops: &[Op]) -> Option<Op> {
        ops.iter().copied().find(|op| self.eat(op))
    }

    /// Eats a `-`, tight or not.
    fn minus(&mut self) -> bool {
        let tight = self.peek() == Some(&Tok::TightMinus);
        self.i += tight as usize;
        tight || self.eat("-")
    }

    fn expect(&mut self, sign: &str) -> Res<()> {
        if self.eat(sign) { Ok(()) } else { Err(format!("expected `{sign}` but found {}", self.found())) }
    }

    fn name_at(&self, n: usize) -> Option<&Sym> {
        match self.at(n) {
            Some(Tok::Name(name)) => Some(name),
            _ => None,
        }
    }

    fn name(&mut self, what: &str) -> Res<Sym> {
        let name = self.name_at(0).cloned().ok_or_else(|| format!("expected {what} but found {}", self.found()))?;
        self.i += 1;
        Ok(name)
    }

    fn statement(&mut self) -> Res<Stmt> {
        if self.eat("?") {
            Ok(Stmt::Ask(self.condition()?))
        } else if self.eat("+") {
            self.change()
        } else if self.minus() {
            self.removal()
        } else if self.eat("from") {
            let mut parents = vec![self.world_path()?];
            while self.eat(",") {
                parents.push(self.world_path()?);
            }
            Ok(Stmt::From(parents))
        } else if self.eat("kind") {
            self.kind()
        } else if self.eat("rule") {
            let name = self.name("the rule's name")?;
            self.expect(":")?;
            let head =
                Head { kind: self.name("a kind")?, var: self.name("a variable")?, rel: self.name("a relation")?, val: self.pterm()? };
            let cond = if self.eat("if") { self.condition()? } else { Cond::And(Vec::new()) };
            Ok(Stmt::Rule { name, head, cond })
        } else if self.eat("never") {
            let name = self.name("the constraint's name")?;
            self.expect(":")?;
            let (kind, var) = (self.name("a kind")?, self.name("a variable")?);
            self.expect("where")?;
            Ok(Stmt::Never { name, kind, var, cond: self.condition()? })
        } else if self.eat("each") {
            self.each(false)
        } else if self.is("in") {
            let world = self.name_at(1).cloned().ok_or("`in` looks inside a world; it starts a question, not a line")?;
            self.i = self.t.len();
            Ok(Stmt::InChange(world))
        } else {
            self.change()
        }
    }

    /// `towns/town_a`, `../town`
    fn world_path(&mut self) -> Res<String> {
        let mut path = String::new();
        loop {
            if self.is(".") && self.at(1) == Some(&Tok::Sign(".")) {
                self.i += 2;
                path.push_str("..");
            } else if self.eat("id") {
                path.push_str("id");
            } else {
                path.push_str(&self.name("a world's name")?);
            }
            if !self.eat("/") {
                return Ok(path);
            }
            path.push('/');
        }
    }

    fn kind(&mut self) -> Res<Stmt> {
        let name = self.name("the kind's name")?;
        let mut edits = Vec::new();
        if self.is("[") {
            self.list(|p| {
                if p.eat("-") {
                    edits.push(ShapeEdit::Remove(p.name("a relation")?));
                    return Ok(());
                }
                let set = p.eat("+");
                let rel = p.name("a relation")?;
                p.expect(":")?;
                let word = match p.name_at(0).map(|w| &**w) {
                    Some("one") => How::One,
                    Some("maybe") => How::Maybe,
                    Some("some") => How::Some,
                    _ => How::Many,
                };
                // `knows: many` alone would name a kind called many.
                let worded = p.name_at(1).is_some() && (word != How::Many || p.name_at(0).is_some_and(|w| &**w == "many"));
                p.i += worded as usize;
                let how = if worded { word } else { How::Many };
                let kind = if p.eat("id") { "id".into() } else { p.name("a kind")? };
                let by = if p.name_at(0).is_some_and(|w| &**w == "by") {
                    p.i += 1;
                    Some(p.name("the relation after `by`")?)
                } else {
                    None
                };
                let field = Field { how, kind, by };
                edits.push(if set { ShapeEdit::Set(rel, field) } else { ShapeEdit::Declare(rel, field) });
                Ok(())
            })?;
        }
        Ok(Stmt::Kind { name, edits })
    }

    /// `[ entry | entry … ]`, where a new line separates too.
    fn list(&mut self, mut entry: impl FnMut(&mut Self) -> Res<()>) -> Res<()> {
        self.expect("[")?;
        loop {
            while self.eat("|") {}
            if self.eat("]") {
                return Ok(());
            }
            entry(self)?;
            if !self.is("|") && !self.is("]") {
                return Err(format!("expected `|` or `]` but found {}", self.found()));
            }
        }
    }

    /// `[ r: a, b | s: + c, - d ]`, or nothing where the brackets are left out.
    fn values(&mut self) -> Res<Vec<Assign>> {
        let mut assigns = Vec::new();
        if self.peek().is_none() {
            return Ok(assigns);
        }
        self.list(|p| {
            let rel = p.name("a relation")?;
            p.expect(":")?;
            let signed = p.is("+") || p.is("-");
            let mut set = Vec::new();
            loop {
                if signed != (p.is("+") || p.is("-")) {
                    return Err(format!("`{rel}` mixes values with `+` and `-`; sign every value or none"));
                }
                if signed {
                    let change = if p.eat("+") { Change::Add } else { Change::Remove };
                    p.i += (change == Change::Remove) as usize;
                    assigns.push(Assign { rel: rel.clone(), change, values: vec![p.expr()?] });
                } else {
                    set.push(p.expr()?);
                }
                if !p.eat(",") {
                    break;
                }
            }
            if !signed {
                assigns.push(Assign { rel, change: Change::Set, values: set });
            }
            Ok(())
        })?;
        Ok(assigns)
    }

    /// A line that states: an object, a fact, or values set in brackets.
    fn change(&mut self) -> Res<Stmt> {
        let start = self.i;
        if self.name_at(0).is_some_and(|n| &**n == "world") {
            self.i += 1;
            if let Ok(path) = self.world_path()
                && (self.peek().is_none() || self.is("["))
            {
                let handle = path.rsplit('/').next().unwrap_or(&path).into();
                return Ok(Stmt::Create { kind: "world".into(), handle, path: Some(path), values: self.values()? });
            }
            self.i = start;
        }
        if self.is("the") || self.name_at(0).is_some() {
            let subj = self.reference()?;
            if self.is("[") {
                return Ok(Stmt::Set { subj, values: self.values()? });
            }
            if let Some(rel) = self.name_at(0).cloned() {
                self.i += 1;
                return Ok(match subj {
                    Expr::Name(kind) if self.peek().is_none() || self.is("[") => {
                        Stmt::Create { kind, handle: rel, path: None, values: self.values()? }
                    }
                    subj => Stmt::Fact { subj, rel, value: self.expr()? },
                });
            }
        }
        // Not a statement. If it reads as a value, say what is wrong with that.
        self.i = start;
        self.expr()?;
        Err(if self.peek().is_none() { "this value isn't used".into() } else { format!("{} doesn't belong here", self.found()) })
    }

    fn removal(&mut self) -> Res<Stmt> {
        if self.eat("kind") {
            Ok(Stmt::RemoveKind(self.name("a kind")?))
        } else if let Some(what) = self.any(&["rule", "never"]) {
            Ok(Stmt::Forget(what, self.name(if what == "rule" { "a rule's name" } else { "a constraint's name" })?))
        } else if self.eat("each") {
            self.each(true)
        } else {
            let subj = self.reference()?;
            let rel = self.name_at(0).cloned();
            self.i += rel.is_some() as usize;
            let value = if rel.is_some() && self.peek().is_some() { Some(self.expr()?) } else { None };
            Ok(Stmt::Remove { subj, rel, value })
        }
    }

    fn each(&mut self, removes: bool) -> Res<Stmt> {
        let kind = self.name("a kind")?;
        let var = self.name("a variable")?;
        let cond = if self.eat("where") { self.condition()? } else { Cond::And(Vec::new()) };
        let action = if self.is("[") {
            if removes {
                return Err("`- each` removes things or relations; to change values write `each … [ r: - v ]`".into());
            }
            EachAction::Set(self.values()?)
        } else if let Some(rel) = self.name_at(0).cloned() {
            self.i += 1;
            let value = if self.peek().is_some() { Some(self.expr()?) } else { None };
            match (removes, value) {
                (true, value) => EachAction::RemoveRel(rel, value),
                (false, Some(value)) => EachAction::Fact(rel, value),
                (false, None) => return Err(format!("expected a value after `{rel}`")),
            }
        } else if removes {
            EachAction::RemoveObjects
        } else {
            return Err(format!("`each` needs a change in brackets, but found {}", self.found()));
        };
        Ok(Stmt::Each { kind, var, cond, action })
    }

    /// A handle, `kind@handle`, or `the` with a description.
    fn reference(&mut self) -> Res<Expr> {
        if !self.eat("the") {
            let name = self.name("a name")?;
            return self.at_kind(name);
        }
        let first = self.name("a handle")?;
        let (kind, handle) = match self.name_at(0) {
            Some(second) if self.at(1) == Some(&Tok::Sign("[")) => {
                let second = second.clone();
                self.i += 1;
                (Some(first), second)
            }
            _ => (None, first),
        };
        if !self.is("[") {
            return Err(format!("after `the {handle}`, describe which one in brackets"));
        }
        let mut desc = Vec::new();
        for assign in self.values()? {
            if assign.change != Change::Set {
                return Err("after `the`, brackets describe; they can't use `+` or `-`".into());
            }
            desc.push((assign.rel, assign.values));
        }
        Ok(Expr::The(Box::new(The { kind, handle, desc })))
    }

    /// `person@harbor`: the object of that kind with that handle, as `the person harbor [ ]`.
    fn at_kind(&mut self, name: Sym) -> Res<Expr> {
        if !self.eat("@") {
            return Ok(Expr::Name(name));
        }
        let handle = self.name("a handle after `@`")?;
        Ok(Expr::The(Box::new(The { kind: Some(name), handle, desc: Vec::new() })))
    }

    /// Runs a parser one level deeper.
    fn nested<T>(&mut self, parse: fn(&mut Self) -> Res<T>) -> Res<T> {
        if self.depth == DEPTH {
            return Err(TOO_DEEP.into());
        }
        self.depth += 1;
        let result = parse(self);
        self.depth -= 1;
        result
    }

    /// Parts joined by `and` or `or`.
    fn joined(&mut self, word: &str, part: fn(&mut Self) -> Res<Cond>, join: fn(Vec<Cond>) -> Cond) -> Res<Cond> {
        let mut parts = vec![part(self)?];
        while self.eat(word) {
            parts.push(part(self)?);
        }
        Ok(if parts.len() == 1 { parts.remove(0) } else { join(parts) })
    }

    fn condition(&mut self) -> Res<Cond> {
        self.nested(|p| p.joined("or", |p| p.joined("and", Self::negation, Cond::And), Cond::Or))
    }

    fn negation(&mut self) -> Res<Cond> {
        if self.eat("not") { Ok(Cond::Not(Box::new(self.nested(Self::negation)?))) } else { self.item() }
    }

    fn item(&mut self) -> Res<Cond> {
        if self.eat("in") {
            let world = self.reference()?;
            self.expect(":")?;
            return Ok(match self.condition()? {
                Cond::Value(value) => Cond::Value(Expr::In(Box::new(world), Box::new(value))),
                cond => Cond::In(world, Box::new(cond)),
            });
        }
        let names = (0..).take_while(|&n| self.name_at(n).is_some()).count();
        if names >= 2 {
            return self.names(names);
        }
        let start = self.i;
        let lead = match self.expr() {
            Ok(lead) => lead,
            // Not a value in parentheses, so a condition in parentheses.
            Err(e) if e != TOO_DEEP && self.t.get(start) == Some(&Tok::Sign("(")) => {
                self.i = start + 1;
                let cond = self.condition()?;
                self.expect(")")?;
                return Ok(cond);
            }
            Err(e) => return Err(e),
        };
        if let Some(op) = self.any(&["=", "!=", "<", ">", "<=", ">="]) {
            return Ok(Cond::Cmp(op, lead, self.expr()?));
        }
        if let Some(rel) = self.name_at(0).cloned() {
            self.i += 1;
            return Ok(Cond::Pattern { subj: PTerm::Expr(lead), rel, val: self.pterm()? });
        }
        Ok(Cond::Value(lead))
    }

    /// An item that starts with `n` names in a row (`n` is at least 2).
    ///
    /// The shapes are `K v`, `s r V`, `K v r V`, `s r K v` and `K v r K v`, where `V` is a value
    /// that may go on past the names (`harbor age harbor.age + 1`).
    fn names(&mut self, n: usize) -> Res<Cond> {
        let name = |p: &Self, k: usize| p.name_at(k).cloned().unwrap();
        let term = |p: &Self, k: usize| PTerm::Expr(Expr::Name(name(p, k)));
        let intro = |p: &Self, k: usize| PTerm::Intro(name(p, k), name(p, k + 1));
        let after = self.at(n);
        // Does the last name carry on as a value, or does a new value start after it?
        let carries_on = match after {
            Some(Tok::Sign("." | "@" | "+" | "-" | "*" | "/" | "div" | "mod")) => true,
            Some(Tok::TightMinus) => n >= 4,
            _ => false,
        };
        let value_starts = matches!(
            after,
            Some(
                Tok::Num(_) | Tok::Text(_) | Tok::TightMinus | Tok::Sign("(" | "-" | "the" | "count" | "sum" | "min" | "max" | "in" | "id")
            )
        );
        // Reads `skip` names, then the value that follows them.
        let pattern = |p: &mut Self, subj: PTerm, rel: usize, skip: usize| -> Res<Cond> {
            let rel = name(p, rel);
            p.i += skip;
            Ok(Cond::Pattern { subj, rel, val: PTerm::Expr(p.expr()?) })
        };
        let whole = match n {
            2 if !value_starts => Cond::Any(name(self, 0), name(self, 1)),
            3 if value_starts && !carries_on => return pattern(self, intro(self, 0), 2, 3),
            2 | 3 => return pattern(self, term(self, 0), 1, 2),
            4 if carries_on => return pattern(self, intro(self, 0), 2, 3),
            4 => {
                let as_kind = Cond::Pattern { subj: intro(self, 0), rel: name(self, 2), val: term(self, 3) };
                let as_term = Cond::Pattern { subj: term(self, 0), rel: name(self, 1), val: intro(self, 2) };
                Cond::Either { name: name(self, 0), as_kind: Box::new(as_kind), as_term: Box::new(as_term) }
            }
            5 => Cond::Pattern { subj: intro(self, 0), rel: name(self, 2), val: intro(self, 3) },
            _ => return Err(format!("{n} names in a row from `{}`; a fact has a subject, a relation and a value", name(self, 0))),
        };
        self.i += n;
        Ok(whole)
    }

    /// The value side of a fact pattern: `person p`, or a value.
    fn pterm(&mut self) -> Res<PTerm> {
        match (self.name_at(0), self.name_at(1)) {
            (Some(kind), Some(var)) => {
                let intro = PTerm::Intro(kind.clone(), var.clone());
                self.i += 2;
                Ok(intro)
            }
            _ => Ok(PTerm::Expr(self.expr()?)),
        }
    }

    fn expr(&mut self) -> Res<Expr> {
        self.nested(Self::sum)
    }

    fn sum(&mut self) -> Res<Expr> {
        let mut left = self.product()?;
        loop {
            let op = if self.eat("+") {
                "+"
            } else if self.minus() {
                "-"
            } else {
                return Ok(left);
            };
            left = Expr::Bin(op, Box::new(left), Box::new(self.product()?));
        }
    }

    fn product(&mut self) -> Res<Expr> {
        let mut left = self.unary()?;
        while let Some(op) = self.any(&["*", "/", "div", "mod"]) {
            left = Expr::Bin(op, Box::new(left), Box::new(self.unary()?));
        }
        Ok(left)
    }

    fn unary(&mut self) -> Res<Expr> {
        if self.minus() { Ok(Expr::Neg(Box::new(self.nested(Self::unary)?))) } else { self.dotted() }
    }

    fn dotted(&mut self) -> Res<Expr> {
        let value = self.primary()?;
        self.dots(value)
    }

    /// The relations followed from a value: `.knows.age`.
    fn dots(&mut self, mut value: Expr) -> Res<Expr> {
        while self.eat(".") {
            let rel = if self.eat("id") { "id".into() } else { self.name("a relation after `.`")? };
            value = Expr::Dot(Box::new(value), rel);
        }
        Ok(value)
    }

    fn primary(&mut self) -> Res<Expr> {
        if self.is("the") {
            return self.reference();
        }
        let Some(token) = self.peek().cloned() else { return Err("expected a value but the line ends".into()) };
        self.i += 1;
        match token {
            Tok::Num(n) => Ok(Expr::Num(n)),
            Tok::Text(t) => Ok(Expr::Text(t)),
            Tok::Name(name) => self.at_kind(name),
            Tok::Sign("(") => {
                let inner = self.expr()?;
                self.expect(")")?;
                Ok(inner)
            }
            // In parentheses: a pattern whose matches are counted, or a value.
            Tok::Sign("count") if self.eat("(") => {
                let cond = self.condition()?;
                self.expect(")")?;
                Ok(match cond {
                    Cond::Value(value) => Expr::Agg("count", Box::new(self.dots(value)?)),
                    cond => Expr::Count(Box::new(cond)),
                })
            }
            Tok::Sign(op @ ("count" | "sum" | "min" | "max")) => Ok(Expr::Agg(op, Box::new(self.dotted()?))),
            Tok::Sign("in") => {
                let world = self.reference()?;
                self.expect(":")?;
                Ok(Expr::In(Box::new(world), Box::new(self.expr()?)))
            }
            Tok::Sign("id") => {
                self.expect(".")?;
                let index = match self.peek() {
                    Some(Tok::Num(n)) => n.as_index(),
                    _ => None,
                };
                let index = index.ok_or_else(|| format!("after `id.` write an index like 1, not {}", self.found()))?;
                self.i += 1;
                Ok(Expr::Index(index))
            }
            other => Err(format!("expected a value but found `{other}`")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    fn stmt(source: &str) -> Res<Stmt> {
        parse(&lex(source).unwrap()[0].tokens)
    }

    #[test]
    fn lines_are_told_apart_by_position() {
        assert!(matches!(stmt("person harbor"), Ok(Stmt::Create { .. })));
        assert!(matches!(stmt("person harbor [ age: 24 | knows: liz, laurie ]"), Ok(Stmt::Create { .. })));
        assert!(matches!(stmt("harbor knows liz"), Ok(Stmt::Fact { .. })));
        assert!(matches!(stmt("+ harbor age -2"), Ok(Stmt::Fact { value: Expr::Neg(_), .. })));
        assert!(matches!(stmt("harbor [ knows: + robin, - liz ]"), Ok(Stmt::Set { .. })));
        assert!(matches!(stmt("- harbor knows"), Ok(Stmt::Remove { rel: Some(_), value: None, .. })));
        assert!(matches!(stmt("world towns/town_a"), Ok(Stmt::Create { path: Some(_), .. })));
        assert!(matches!(stmt("the harbor [ lives_in: bath ] knows liz"), Ok(Stmt::Fact { subj: Expr::The(_), .. })));
        assert_eq!(stmt("harbor.knows").unwrap_err(), "this value isn't used");
        assert_eq!(stmt("harbor").unwrap_err(), "this value isn't used");
    }

    #[test]
    fn conditions_read_names_in_a_row() {
        let ask = |s: &str| match stmt(s) {
            Ok(Stmt::Ask(cond)) => cond,
            other => panic!("{other:?}"),
        };
        assert!(matches!(ask("? person p"), Cond::Any(..)));
        assert!(matches!(ask("? harbor is_at peace"), Cond::Pattern { subj: PTerm::Expr(_), val: PTerm::Expr(_), .. }));
        assert!(matches!(ask("? person p age -1"), Cond::Pattern { subj: PTerm::Intro(..), .. }));
        assert!(matches!(ask("? harbor owes debt - 1"), Cond::Pattern { subj: PTerm::Expr(_), .. }));
        assert!(matches!(ask("? person p is_at peace"), Cond::Either { .. }));
        assert!(matches!(ask("? person a knows person b"), Cond::Pattern { subj: PTerm::Intro(..), val: PTerm::Intro(..), .. }));
        assert!(matches!(ask("? harbor.age + 1"), Cond::Value(Expr::Bin(..))));
        assert!(matches!(ask("? in town_a: harbor.knows"), Cond::Value(Expr::In(..))));
        assert!(matches!(ask("? (in a: x feels happy) and not (in c: x feels happy)"), Cond::And(_)));
        assert!(matches!(ask("? count (person q knows p) > 10"), Cond::Cmp(_, Expr::Count(_), _)));
        assert!(matches!(ask("? sum harbor.knows.age / count harbor.knows"), Cond::Value(Expr::Bin("/", ..))));
    }
}

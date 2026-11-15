//! From tokens to a statement.
//!
//! Grant reads names by where they stand, so the grammar needs no declarations to parse. The one
//! case position can't settle is four names in a row, which is kept as `Cond::Either`.

use crate::ast::*;
use crate::error::Res;
use crate::lexer::Tok;
use crate::value::Sym;

const KEYWORDS: [&str; 19] = [
    "from", "kind", "rule", "never", "if", "where", "and", "or", "not", "the", "each", "in", "count", "sum", "min", "max", "div", "mod",
    "id",
];

pub fn is_keyword(word: &str) -> bool {
    KEYWORDS.contains(&word)
}

pub fn parse(tokens: &[Tok]) -> Res<Stmt> {
    let mut parser = Parser { t: tokens, i: 0, depth: 0 };
    let stmt = parser.statement()?;
    match parser.peek() {
        None => Ok(stmt),
        Some(_) => Err(format!("{} doesn't belong here", parser.found())),
    }
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

    fn kw(&self, word: &str) -> bool {
        matches!(self.peek(), Some(Tok::Ident(w)) if &**w == word)
    }

    fn eat_kw(&mut self, word: &str) -> bool {
        let found = self.kw(word);
        self.i += found as usize;
        found
    }

    fn eat(&mut self, token: &Tok) -> bool {
        let found = self.peek() == Some(token);
        self.i += found as usize;
        found
    }

    fn expect(&mut self, token: Tok) -> Res<()> {
        if self.eat(&token) { Ok(()) } else { Err(format!("expected `{token}` but found {}", self.found())) }
    }

    /// The name `n` tokens ahead, if there is one and it isn't a keyword.
    fn name_at(&self, n: usize) -> Option<&Sym> {
        match self.at(n) {
            Some(Tok::Ident(w)) if !is_keyword(w) => Some(w),
            _ => None,
        }
    }

    fn name(&mut self, what: &str) -> Res<Sym> {
        let name = self.name_at(0).cloned().ok_or_else(|| format!("expected {what} but found {}", self.found()))?;
        self.i += 1;
        Ok(name)
    }

    fn statement(&mut self) -> Res<Stmt> {
        match () {
            _ if self.eat(&Tok::Question) => Ok(Stmt::Ask(self.condition()?)),
            _ if self.eat(&Tok::Plus) => self.change(),
            _ if self.eat(&Tok::Minus) || self.eat(&Tok::TightMinus) => self.removal(),
            _ if self.eat_kw("from") => {
                let mut parents = vec![self.world_path()?];
                while self.eat(&Tok::Comma) {
                    parents.push(self.world_path()?);
                }
                Ok(Stmt::From(parents))
            }
            _ if self.eat_kw("kind") => self.kind(),
            _ if self.eat_kw("rule") => {
                let name = self.name("the rule's name")?;
                self.expect(Tok::Colon)?;
                let head =
                    Head { kind: self.name("a kind")?, var: self.name("a variable")?, rel: self.name("a relation")?, val: self.pterm()? };
                let cond = if self.eat_kw("if") { self.condition()? } else { Cond::And(Vec::new()) };
                Ok(Stmt::Rule { name, head, cond })
            }
            _ if self.eat_kw("never") => {
                let name = self.name("the constraint's name")?;
                self.expect(Tok::Colon)?;
                let kind = self.name("a kind")?;
                let var = self.name("a variable")?;
                if !self.eat_kw("where") {
                    return Err(format!("expected `where` but found {}", self.found()));
                }
                Ok(Stmt::Never { name, kind, var, cond: self.condition()? })
            }
            _ if self.eat_kw("each") => self.each(false),
            _ if self.kw("in") => match self.name_at(1) {
                Some(world) => {
                    let world = world.clone();
                    self.i = self.t.len();
                    Ok(Stmt::InChange(world))
                }
                None => Err("`in` looks inside a world; it starts a question, not a line".into()),
            },
            _ => self.change(),
        }
    }

    /// `towns/town_a`, `../town`
    fn world_path(&mut self) -> Res<String> {
        let mut path = String::new();
        loop {
            if self.peek() == Some(&Tok::Dot) && self.at(1) == Some(&Tok::Dot) {
                self.i += 2;
                path.push_str("..");
            } else if self.eat_kw("id") {
                path.push_str("id");
            } else {
                path.push_str(&self.name("a world's name")?);
            }
            if !self.eat(&Tok::Slash) {
                return Ok(path);
            }
            path.push('/');
        }
    }

    fn kind(&mut self) -> Res<Stmt> {
        let name = self.name("the kind's name")?;
        let mut edits = Vec::new();
        if self.peek() == Some(&Tok::LBracket) {
            edits = self.list(|p| {
                if p.eat(&Tok::Minus) {
                    return Ok(ShapeEdit::Remove(p.name("a relation")?));
                }
                let set = p.eat(&Tok::Plus);
                let rel = p.name("a relation")?;
                p.expect(Tok::Colon)?;
                let word = match p.name_at(0).map(|w| &**w) {
                    Some("one") => How::One,
                    Some("maybe") => How::Maybe,
                    Some("some") => How::Some,
                    _ => How::Many,
                };
                // `knows: many` alone would name a kind called many.
                let worded = p.name_at(1).is_some() && (word != How::Many || p.name_at(0).is_some_and(|w| &**w == "many"));
                p.i += worded as usize;
                let field = Field {
                    how: if worded { word } else { How::Many },
                    kind: if p.eat_kw("id") { "id".into() } else { p.name("a kind")? },
                };
                Ok(if set { ShapeEdit::Set(rel, field) } else { ShapeEdit::Declare(rel, field) })
            })?;
        }
        Ok(Stmt::Kind { name, edits })
    }

    /// `[ item | item … ]`, where a new line separates too.
    fn list<T>(&mut self, mut item: impl FnMut(&mut Self) -> Res<T>) -> Res<Vec<T>> {
        self.expect(Tok::LBracket)?;
        let mut items = Vec::new();
        loop {
            while self.eat(&Tok::Sep) {}
            if self.eat(&Tok::RBracket) {
                return Ok(items);
            }
            items.push(item(self)?);
            if !matches!(self.peek(), Some(Tok::Sep | Tok::RBracket)) {
                return Err(format!("expected `|` or `]` but found {}", self.found()));
            }
        }
    }

    fn values(&mut self) -> Res<Vec<Assign>> {
        self.list(|p| {
            let rel = p.name("a relation")?;
            p.expect(Tok::Colon)?;
            let signed = |p: &Self| matches!(p.peek(), Some(Tok::Plus | Tok::Minus));
            let values = if signed(p) {
                let mut changes = Vec::new();
                loop {
                    if !signed(p) {
                        return Err(format!("`{rel}` mixes values with `+` and `-`; sign every value or none"));
                    }
                    let adds = p.eat(&Tok::Plus);
                    p.i += !adds as usize;
                    changes.push((adds, p.expr()?));
                    if !p.eat(&Tok::Comma) {
                        break Values::Change(changes);
                    }
                }
            } else {
                let mut set = vec![p.expr()?];
                while p.eat(&Tok::Comma) {
                    if signed(p) {
                        return Err(format!("`{rel}` mixes values with `+` and `-`; sign every value or none"));
                    }
                    set.push(p.expr()?);
                }
                Values::Set(set)
            };
            Ok(Assign { rel, values })
        })
    }

    /// A line that states: an object, a fact, or values set in brackets.
    fn change(&mut self) -> Res<Stmt> {
        let start = self.i;
        if self.name_at(0).is_some_and(|n| &**n == "world") {
            self.i += 1;
            if let Ok(path) = self.world_path()
                && matches!(self.peek(), None | Some(Tok::LBracket))
            {
                let handle = path.rsplit('/').next().unwrap_or(&path).into();
                let values = if self.peek().is_some() { self.values()? } else { Vec::new() };
                return Ok(Stmt::Create { kind: "world".into(), handle, path: Some(path), values });
            }
            self.i = start;
        }
        let subj = if self.kw("the") {
            Some(Expr::The(Box::new(self.the()?)))
        } else {
            self.name_at(0).cloned().map(|n| {
                self.i += 1;
                Expr::Name(n)
            })
        };
        if let Some(subj) = subj {
            if self.peek() == Some(&Tok::LBracket) {
                return Ok(Stmt::Set { subj, values: self.values()? });
            }
            if let Some(rel) = self.name_at(0).cloned() {
                self.i += 1;
                return match (&subj, self.peek()) {
                    (Expr::Name(kind), None) => Ok(Stmt::Create { kind: kind.clone(), handle: rel, path: None, values: Vec::new() }),
                    (Expr::Name(kind), Some(Tok::LBracket)) => {
                        Ok(Stmt::Create { kind: kind.clone(), handle: rel, path: None, values: self.values()? })
                    }
                    _ => Ok(Stmt::Fact { subj, rel, value: self.expr()? }),
                };
            }
        }
        // Not a statement. If it reads as a value, say what is wrong with that.
        self.i = start;
        self.expr()?;
        match self.peek() {
            None => Err("this value isn't used".into()),
            Some(_) => Err(format!("{} doesn't belong here", self.found())),
        }
    }

    fn removal(&mut self) -> Res<Stmt> {
        if self.eat_kw("kind") {
            Ok(Stmt::RemoveKind(self.name("a kind")?))
        } else if self.eat_kw("rule") {
            Ok(Stmt::RemoveRule(self.name("a rule's name")?))
        } else if self.eat_kw("never") {
            Ok(Stmt::RemoveNever(self.name("a constraint's name")?))
        } else if self.eat_kw("each") {
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
        let cond = if self.eat_kw("where") { Some(self.condition()?) } else { None };
        let action = if self.peek() == Some(&Tok::LBracket) {
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

    fn reference(&mut self) -> Res<Expr> {
        if self.kw("the") { Ok(Expr::The(Box::new(self.the()?))) } else { Ok(Expr::Name(self.name("a name")?)) }
    }

    fn the(&mut self) -> Res<The> {
        self.i += 1;
        let first = self.name("a handle")?;
        let (kind, handle) = match self.name_at(0) {
            Some(second) if self.at(1) == Some(&Tok::LBracket) => {
                let second = second.clone();
                self.i += 1;
                (Some(first), second)
            }
            _ => (None, first),
        };
        if self.peek() != Some(&Tok::LBracket) {
            return Err(format!("after `the {handle}`, describe which one in brackets"));
        }
        let mut desc = Vec::new();
        for assign in self.values()? {
            match assign.values {
                Values::Set(values) => desc.push((assign.rel, values)),
                Values::Change(_) => return Err("after `the`, brackets describe; they can't use `+` or `-`".into()),
            }
        }
        Ok(The { kind, handle, desc })
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

    fn condition(&mut self) -> Res<Cond> {
        self.nested(Self::disjunction)
    }

    fn disjunction(&mut self) -> Res<Cond> {
        let mut any = vec![self.conjunction()?];
        while self.eat_kw("or") {
            any.push(self.conjunction()?);
        }
        Ok(if any.len() == 1 { any.remove(0) } else { Cond::Or(any) })
    }

    fn conjunction(&mut self) -> Res<Cond> {
        let mut all = vec![self.negation()?];
        while self.eat_kw("and") {
            all.push(self.negation()?);
        }
        Ok(if all.len() == 1 { all.remove(0) } else { Cond::And(all) })
    }

    fn negation(&mut self) -> Res<Cond> {
        if self.eat_kw("not") { Ok(Cond::Not(Box::new(self.nested(Self::negation)?))) } else { self.item() }
    }

    fn item(&mut self) -> Res<Cond> {
        if self.eat_kw("in") {
            let world = self.reference()?;
            self.expect(Tok::Colon)?;
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
            Err(e) if e != TOO_DEEP && self.t.get(start) == Some(&Tok::LParen) => {
                self.i = start + 1;
                let cond = self.condition()?;
                self.expect(Tok::RParen)?;
                return Ok(cond);
            }
            Err(e) => return Err(e),
        };
        let op = match self.peek() {
            Some(Tok::Eq) => Some(CmpOp::Eq),
            Some(Tok::Ne) => Some(CmpOp::Ne),
            Some(Tok::Lt) => Some(CmpOp::Lt),
            Some(Tok::Gt) => Some(CmpOp::Gt),
            Some(Tok::Le) => Some(CmpOp::Le),
            Some(Tok::Ge) => Some(CmpOp::Ge),
            _ => None,
        };
        if let Some(op) = op {
            self.i += 1;
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
            Some(Tok::Dot | Tok::Plus | Tok::Minus | Tok::Star | Tok::Slash) => true,
            Some(Tok::TightMinus) => n >= 4,
            Some(Tok::Ident(w)) => matches!(&**w, "div" | "mod"),
            _ => false,
        };
        let value_starts = match after {
            Some(Tok::Num(_) | Tok::Text(_) | Tok::LParen | Tok::Minus | Tok::TightMinus) => true,
            Some(Tok::Ident(w)) => matches!(&**w, "the" | "count" | "sum" | "min" | "max" | "in" | "id"),
            _ => false,
        };
        // Reads `skip` names, then the value that follows them.
        let pattern = |p: &mut Self, subj: PTerm, rel: usize, skip: usize| -> Res<Cond> {
            let rel = name(p, rel);
            p.i += skip;
            Ok(Cond::Pattern { subj, rel, val: PTerm::Expr(p.expr()?) })
        };
        match n {
            2 if value_starts => pattern(self, term(self, 0), 1, 2),
            2 => {
                let any = Cond::Any(name(self, 0), name(self, 1));
                self.i += 2;
                Ok(any)
            }
            3 if carries_on => pattern(self, term(self, 0), 1, 2),
            3 if value_starts => pattern(self, intro(self, 0), 2, 3),
            3 => pattern(self, term(self, 0), 1, 2),
            4 if carries_on => pattern(self, intro(self, 0), 2, 3),
            4 => {
                let as_kind = Cond::Pattern { subj: intro(self, 0), rel: name(self, 2), val: term(self, 3) };
                let as_term = Cond::Pattern { subj: term(self, 0), rel: name(self, 1), val: intro(self, 2) };
                let either = Cond::Either { name: name(self, 0), as_kind: Box::new(as_kind), as_term: Box::new(as_term) };
                self.i += 4;
                Ok(either)
            }
            5 => {
                let both = Cond::Pattern { subj: intro(self, 0), rel: name(self, 2), val: intro(self, 3) };
                self.i += 5;
                Ok(both)
            }
            _ => Err(format!("{n} names in a row from `{}`; a fact has a subject, a relation and a value", name(self, 0))),
        }
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
            let op = match self.peek() {
                Some(Tok::Plus) => BinOp::Add,
                Some(Tok::Minus | Tok::TightMinus) => BinOp::Sub,
                _ => return Ok(left),
            };
            self.i += 1;
            left = Expr::Bin(op, Box::new(left), Box::new(self.product()?));
        }
    }

    fn product(&mut self) -> Res<Expr> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Star) => BinOp::Mul,
                Some(Tok::Slash) => BinOp::Div,
                Some(Tok::Ident(w)) if &**w == "div" => BinOp::IntDiv,
                Some(Tok::Ident(w)) if &**w == "mod" => BinOp::Mod,
                _ => return Ok(left),
            };
            self.i += 1;
            left = Expr::Bin(op, Box::new(left), Box::new(self.unary()?));
        }
    }

    fn unary(&mut self) -> Res<Expr> {
        if matches!(self.peek(), Some(Tok::Minus | Tok::TightMinus)) {
            self.i += 1;
            return Ok(Expr::Neg(Box::new(self.nested(Self::unary)?)));
        }
        self.dotted()
    }

    fn dotted(&mut self) -> Res<Expr> {
        let value = self.primary()?;
        self.dots(value)
    }

    /// The relations followed from a value: `.knows.age`.
    fn dots(&mut self, mut value: Expr) -> Res<Expr> {
        while self.eat(&Tok::Dot) {
            let rel = match self.peek() {
                Some(Tok::Ident(w)) if &**w == "id" || !is_keyword(w) => w.clone(),
                _ => return Err(format!("expected a relation after `.` but found {}", self.found())),
            };
            self.i += 1;
            value = Expr::Dot(Box::new(value), rel);
        }
        Ok(value)
    }

    fn primary(&mut self) -> Res<Expr> {
        let Some(token) = self.peek().cloned() else { return Err("expected a value but the line ends".into()) };
        match token {
            Tok::Num(n) => {
                self.i += 1;
                Ok(Expr::Num(n))
            }
            Tok::Text(t) => {
                self.i += 1;
                Ok(Expr::Text(t))
            }
            Tok::LParen => {
                self.i += 1;
                let inner = self.expr()?;
                self.expect(Tok::RParen)?;
                Ok(inner)
            }
            Tok::Ident(word) => match &*word {
                "the" => Ok(Expr::The(Box::new(self.the()?))),
                "count" => {
                    self.i += 1;
                    if !self.eat(&Tok::LParen) {
                        return Ok(Expr::Count(Box::new(self.dotted()?)));
                    }
                    // In parentheses: a pattern whose matches are counted, or a value.
                    let cond = self.condition()?;
                    self.expect(Tok::RParen)?;
                    Ok(match cond {
                        Cond::Value(value) => Expr::Count(Box::new(self.dots(value)?)),
                        cond => Expr::CountCond(Box::new(cond)),
                    })
                }
                "sum" | "min" | "max" => {
                    self.i += 1;
                    let op = match &*word {
                        "sum" => AggOp::Sum,
                        "min" => AggOp::Min,
                        _ => AggOp::Max,
                    };
                    Ok(Expr::Agg(op, Box::new(self.dotted()?)))
                }
                "in" => {
                    self.i += 1;
                    let world = self.reference()?;
                    self.expect(Tok::Colon)?;
                    Ok(Expr::In(Box::new(world), Box::new(self.expr()?)))
                }
                "id" => {
                    self.i += 1;
                    self.expect(Tok::Dot)?;
                    match self.peek().cloned() {
                        Some(Tok::Num(n)) if n.as_index().is_some() => {
                            self.i += 1;
                            Ok(Expr::Index(n.as_index().unwrap()))
                        }
                        _ => Err(format!("after `id.` write an index like 1, not {}", self.found())),
                    }
                }
                w if is_keyword(w) => Err(format!("expected a value but found `{w}`")),
                _ => {
                    self.i += 1;
                    Ok(Expr::Name(word))
                }
            },
            _ => Err(format!("expected a value but found {}", self.found())),
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
        assert!(matches!(stmt("harbor [ knows: + marco, - liz ]"), Ok(Stmt::Set { .. })));
        assert!(matches!(stmt("- harbor knows"), Ok(Stmt::Remove { rel: Some(_), value: None, .. })));
        assert!(matches!(stmt("world towns/town_a"), Ok(Stmt::Create { path: Some(_), .. })));
        assert!(matches!(stmt("the harbor [ lives_in: rome ] knows liz"), Ok(Stmt::Fact { subj: Expr::The(_), .. })));
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
        assert!(matches!(ask("? count (person q knows p) > 10"), Cond::Cmp(_, Expr::CountCond(_), _)));
        assert!(matches!(ask("? sum harbor.knows.age / count harbor.knows"), Cond::Value(Expr::Bin(BinOp::Div, ..))));
    }
}

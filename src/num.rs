//! Exact numbers: rationals that never round.

use std::cmp::Ordering;
use std::fmt;

/// An exact rational number, always in lowest terms with a positive denominator.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Rat {
    n: i128,
    d: i128,
}

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.unsigned_abs(), b.unsigned_abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a as i128
}

impl Rat {
    pub const ZERO: Rat = Rat { n: 0, d: 1 };

    pub fn int(n: i128) -> Rat {
        Rat { n, d: 1 }
    }

    /// `None` when the result doesn't fit. `d` must not be zero.
    fn new(n: i128, d: i128) -> Option<Rat> {
        if n == i128::MIN || d == i128::MIN {
            return None;
        }
        let g = gcd(n, d);
        let (n, d) = (n / g, d / g);
        Some(if d < 0 { Rat { n: -n, d: -d } } else { Rat { n, d } })
    }

    /// Reads a literal such as `24` or `3.5`.
    pub fn parse(literal: &str) -> Option<Rat> {
        let (whole, fraction) = literal.split_once('.').unwrap_or((literal, ""));
        let mut n: i128 = 0;
        for c in whole.chars().chain(fraction.chars()) {
            n = n.checked_mul(10)?.checked_add(c.to_digit(10)? as i128)?;
        }
        Rat::new(n, 10i128.checked_pow(fraction.len() as u32)?)
    }

    pub fn is_zero(self) -> bool {
        self.n == 0
    }

    /// The number as an index (a whole number from 1 up), if it is one.
    pub fn as_index(self) -> Option<u64> {
        (self.d == 1 && self.n >= 1).then(|| u64::try_from(self.n).ok())?
    }

    pub fn neg(self) -> Option<Rat> {
        Some(Rat { n: self.n.checked_neg()?, d: self.d })
    }

    pub fn add(self, o: Rat) -> Option<Rat> {
        let n = self.n.checked_mul(o.d)?.checked_add(o.n.checked_mul(self.d)?)?;
        Rat::new(n, self.d.checked_mul(o.d)?)
    }

    pub fn sub(self, o: Rat) -> Option<Rat> {
        self.add(o.neg()?)
    }

    pub fn mul(self, o: Rat) -> Option<Rat> {
        Rat::new(self.n.checked_mul(o.n)?, self.d.checked_mul(o.d)?)
    }

    /// `o` must not be zero.
    pub fn div(self, o: Rat) -> Option<Rat> {
        Rat::new(self.n.checked_mul(o.d)?, self.d.checked_mul(o.n)?)
    }

    /// `div`: the whole part of the quotient, rounded down.
    pub fn div_floor(self, o: Rat) -> Option<Rat> {
        let q = self.div(o)?;
        Some(Rat::int(q.n.div_euclid(q.d)))
    }

    /// `mod`: what `div` leaves over, with the sign of the divisor.
    pub fn modulo(self, o: Rat) -> Option<Rat> {
        self.sub(o.mul(self.div_floor(o)?)?)
    }
}

impl Ord for Rat {
    fn cmp(&self, o: &Rat) -> Ordering {
        if let (Some(a), Some(b)) = (self.n.checked_mul(o.d), o.n.checked_mul(self.d)) {
            return a.cmp(&b);
        }
        // Too large to cross-multiply: compare whole parts, then the remainders.
        let (wa, wb) = (self.n.div_euclid(self.d), o.n.div_euclid(o.d));
        let (ra, rb) = (self.n.rem_euclid(self.d), o.n.rem_euclid(o.d));
        if wa != wb {
            wa.cmp(&wb)
        } else if ra == 0 || rb == 0 {
            ra.cmp(&rb)
        } else {
            // ra/da < rb/db exactly when db/rb < da/ra.
            Rat { n: o.d, d: rb }.cmp(&Rat { n: self.d, d: ra })
        }
    }
}

impl PartialOrd for Rat {
    fn partial_cmp(&self, o: &Rat) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl fmt::Display for Rat {
    /// Whole numbers and terminating decimals print as written; anything else stays a fraction.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.d == 1 {
            return write!(f, "{}", self.n);
        }
        let mut rest = self.d;
        while rest % 2 == 0 {
            rest /= 2;
        }
        while rest % 5 == 0 {
            rest /= 5;
        }
        let (n, d) = (self.n.unsigned_abs(), self.d.unsigned_abs());
        let mut digits = String::new();
        let mut rem = n % d;
        while rest == 1 && rem != 0 {
            let Some(scaled) = rem.checked_mul(10) else { break };
            digits.push(char::from_digit((scaled / d) as u32, 10).unwrap());
            rem = scaled % d;
        }
        if rest == 1 && rem == 0 {
            write!(f, "{}{}.{}", if self.n < 0 { "-" } else { "" }, n / d, digits)
        } else {
            write!(f, "{}/{}", self.n, self.d)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(s: &str) -> Rat {
        Rat::parse(s).unwrap()
    }

    #[test]
    fn stays_exact() {
        let third = r("1").div(r("3")).unwrap();
        assert_eq!(third.to_string(), "1/3");
        assert_eq!(third.mul(r("3")).unwrap(), r("1"));
        assert_eq!(r("3.5").to_string(), "3.5");
        assert_eq!(r("0.1").add(r("0.2")).unwrap(), r("0.3"));
        assert_eq!(r("2").neg().unwrap().to_string(), "-2");
        assert_eq!(r("1").div(r("8")).unwrap().neg().unwrap().to_string(), "-0.125");
    }

    #[test]
    fn div_and_mod() {
        assert_eq!(r("17").div_floor(r("5")).unwrap(), r("3"));
        assert_eq!(r("17").modulo(r("5")).unwrap(), r("2"));
        assert_eq!(r("7").neg().unwrap().modulo(r("5")).unwrap(), r("3"));
    }

    #[test]
    fn orders_and_overflows() {
        assert!(r("1").div(r("3")).unwrap() < r("0.5"));
        let big = Rat::int(i128::MAX);
        assert!(big.add(big).is_none());
        assert!(Rat::new(i128::MAX, i128::MAX - 1).unwrap() > Rat::new(i128::MAX - 1, i128::MAX).unwrap());
    }
}

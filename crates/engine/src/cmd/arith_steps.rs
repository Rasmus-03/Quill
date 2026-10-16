//! Stepped arithmetic (pure numbers, no variables): school-order BODMAS
//! unfolding with exact rationals, entirely in Rust — no Maxima on this
//! path, so `3+5*7/23 = 104/23` steps instantly and verifiably.
//!
//! Contract shared with `mathcraft`:
//! * `Ok(Some((steps, answer)))` — a steps text plus the exact answer,
//!   where the final line ends with ` = answer` (the green painter).
//! * `Ok(None)` — not parseable arithmetic; the caller's equation
//!   diagnostics take over.
//! * `Err` — a real problem (a contradicting trailing answer, or a value
//!   too large to step exactly): never show steps that disagree.

use super::mathcraft::err;
use crate::CmdError;

/// Stepped arithmetic for a selection: one steps text or a clean fallback.
pub(crate) fn arithmetic_steps(raw: &str, expr: &str) -> Result<Option<(String, String)>, CmdError> {
    let (body, wanted) = match expr.split_once('=') {
        Some((l, r)) => (l.trim(), Some(r.trim())),
        None => (expr.trim(), None),
    };
    if body.is_empty() {
        return Ok(None);
    }
    let mut root = match parse_arith(body) {
        Some(root) => root,
        None => return Ok(None),
    };
    // The trailing `= …` answer must agree with the expression: a stale
    // glued answer that disagrees is an error, not silent truth.
    let want = match wanted {
        Some(text) => match parse_arith(text).and_then(|mut n| arith_eval(&mut n)) {
            Some(w) => Some(w),
            None => return Err(err("The answer after '=' is not plain arithmetic (numbers only); use Calculate".to_string())),
        },
        None => None,
    };
    let mut steps: Vec<String> = Vec::new();
    let value = fold_to_end(&mut root, &mut steps)?;
    if let Some(w) = want
        && w != value
    {
        return Err(err("The answer after '=' doesn't match the expression — recompute first (Calculate)".to_string()));
    }
    // A single number has no steps: the green ` = answer` semantics still
    // apply (the caller paints what trails ` = `).
    let steps_text = if steps.is_empty() { format!("{raw} = {}", value.show()) } else { format!("{raw}\n{}", steps.join("\n")) };
    Ok(Some((steps_text, value.show())))
}

/// Parse to the end; None when the text is not pure arithmetic.
fn parse_arith(expr: &str) -> Option<Arith> {
    let c: Vec<char> = expr.chars().collect();
    let mut p = ArithParser { c: &c, i: 0, used: 0 };
    let root = p.expr()?;
    p.ws();
    (p.i == c.len()).then_some(root)
}

/// Reduced exact rational: sign on the numerator, `d > 0`, coprime.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Rat {
    n: i64,
    d: i64,
}

fn rat_gcd(mut a: i128, mut b: i128) -> i128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    let g = a.unsigned_abs();
    i128::try_from(g).unwrap_or(i128::MAX)
}

/// Reduce over i128 back to a Rat; None on a zero denominator or overflow.
fn rat_from_big(num: i128, den: i128) -> Option<Rat> {
    if den == 0 {
        return None;
    }
    let (num, den) = if den < 0 { (-num, -den) } else { (num, den) };
    let g = rat_gcd(num, den).max(1);
    Some(Rat { n: i64::try_from(num / g).ok()?, d: i64::try_from(den / g).ok()? })
}

impl Rat {
    fn mul(self, o: Rat) -> Option<Rat> {
        rat_from_big((self.n as i128) * (o.n as i128), (self.d as i128) * (o.d as i128))
    }
    fn div(self, o: Rat) -> Option<Rat> {
        if o.n == 0 {
            return None;
        }
        rat_from_big((self.n as i128) * (o.d as i128), (self.d as i128) * (o.n as i128))
    }
    fn add(self, o: Rat) -> Option<Rat> {
        rat_from_big((self.n as i128) * (o.d as i128) + (o.n as i128) * (self.d as i128), (self.d as i128) * (o.d as i128))
    }
    fn sub(self, o: Rat) -> Option<Rat> {
        rat_from_big((self.n as i128) * (o.d as i128) - (o.n as i128) * (self.d as i128), (self.d as i128) * (o.d as i128))
    }
    /// Integer power only (school steps); a huge exponent is "cannot step".
    fn pow(self, e: i64) -> Option<Rat> {
        if e < 0 {
            let inv = rat_from_big(self.d as i128, self.n as i128)?;
            return inv.pow(-e);
        }
        let u = u32::try_from(e).ok()?;
        if u > 40 {
            return None;
        }
        rat_from_big((self.n as i128).checked_pow(u)?, (self.d as i128).checked_pow(u)?)
    }
    fn show(self) -> String {
        if self.d == 1 { self.n.to_string() } else { format!("{}/{}", self.n, self.d) }
    }
}

/// Arithmetic tree: numbers, `+ - * / ^`, parentheses, unary ±.
#[derive(Clone, Debug)]
enum Arith {
    V(Rat),
    Neg(Box<Arith>),
    B(char, Box<Arith>, Box<Arith>),
}

const ARITH_NODE_CAP: usize = 64;

struct ArithParser<'a> {
    c: &'a [char],
    i: usize,
    used: usize,
}

impl<'a> ArithParser<'a> {
    fn ws(&mut self) {
        while matches!(self.c.get(self.i), Some(' ')) {
            self.i += 1;
        }
    }
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }
    fn bin(&mut self, op: char, l: Arith, r: Arith) -> Option<Arith> {
        self.used += 1;
        (self.used <= ARITH_NODE_CAP).then_some(Arith::B(op, Box::new(l), Box::new(r)))
    }
    /// Sums, left-associative (`1-2-3` = `(1-2)-3`).
    fn expr(&mut self) -> Option<Arith> {
        let mut left = self.term()?;
        loop {
            self.ws();
            let op = match self.peek() {
                Some('+') => '+',
                Some('-') => '-',
                _ => break,
            };
            self.i += 1;
            let right = self.term()?;
            left = self.bin(op, left, right)?;
        }
        Some(left)
    }
    /// Products, left-associative (`100/25/2` = `(100/25)/2`).
    fn term(&mut self) -> Option<Arith> {
        let mut left = self.power()?;
        loop {
            self.ws();
            let op = match self.peek() {
                Some('*') => '*',
                Some('/') => '/',
                _ => break,
            };
            self.i += 1;
            let right = self.power()?;
            left = self.bin(op, left, right)?;
        }
        Some(left)
    }
    /// Exponent right-associative and tightest (`2^3^2` = `2^(3^2)`).
    fn power(&mut self) -> Option<Arith> {
        let base = self.unary()?;
        self.ws();
        if self.peek() == Some('^') {
            self.i += 1;
            let exp = self.power()?;
            return self.bin('^', base, exp);
        }
        Some(base)
    }
    fn unary(&mut self) -> Option<Arith> {
        self.ws();
        let mut minus = false;
        while matches!(self.peek(), Some('+') | Some('-')) {
            if self.peek() == Some('-') {
                minus = !minus;
            }
            self.i += 1;
            self.ws();
        }
        if minus {
            return Some(Arith::Neg(Box::new(self.unary()?)));
        }
        self.atom()
    }
    fn atom(&mut self) -> Option<Arith> {
        self.ws();
        if self.peek() == Some('(') {
            self.i += 1;
            let v = self.expr()?;
            self.ws();
            if self.peek() != Some(')') {
                return None;
            }
            self.i += 1;
            return Some(v);
        }
        self.number()
    }
    /// Digits with an optional decimal fraction (`2.5` → `5/2`).
    fn number(&mut self) -> Option<Arith> {
        self.ws();
        let start = self.i;
        while matches!(self.peek(), Some(ch) if ch.is_ascii_digit()) {
            self.i += 1;
        }
        if self.peek() == Some('.') {
            self.i += 1;
            while matches!(self.peek(), Some(ch) if ch.is_ascii_digit()) {
                self.i += 1;
            }
        }
        let text: String = self.c[start..self.i].iter().collect();
        let (int_part, frac) = match text.split_once('.') {
            Some((a, b)) => (a, b),
            None => (text.as_str(), ""),
        };
        if int_part.is_empty() || !int_part.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let num: i128 = format!("{int_part}{frac}").parse().ok()?;
        let den = 10i128.checked_pow(frac.len() as u32)?;
        rat_from_big(num, den).map(Arith::V)
    }
}

/// Display precedence: `^` above `* /` above `+ -`.
fn arith_prec(op: char) -> u8 {
    match op {
        '^' => 3,
        '*' | '/' => 2,
        _ => 1,
    }
}

fn arith_shown(n: &Arith) -> String {
    match n {
        Arith::V(r) => r.show(),
        Arith::Neg(k) => match &**k {
            Arith::V(_) => format!("-{}", arith_shown(k)),
            _ => format!("-({})", arith_shown(k)),
        },
        Arith::B(op, l, r) => {
            let ls = arith_child(l, arith_prec(*op), false);
            let rs = arith_child(r, arith_prec(*op), true);
            if *op == '^' { format!("{ls}^{rs}") } else { format!("{ls}{op}{rs}") }
        }
    }
}

/// A composite child lighter-bound than its parent is parenthesized; a
/// right child leading with `-` too, so `5-(-2)` never prints as `5--2`.
fn arith_child(n: &Arith, parent_prec: u8, right_side: bool) -> String {
    let s = arith_shown(n);
    let prec = match n {
        Arith::B(op, ..) => arith_prec(*op),
        Arith::Neg(_) => 1,
        Arith::V(_) => 9,
    };
    if prec < parent_prec || (right_side && s.starts_with('-')) { format!("({s})") } else { s }
}

/// Fold a whole tree to its value (for the trailing `= …` answer).
fn arith_eval(n: &mut Arith) -> Option<Rat> {
    let mut throwaway: Vec<String> = Vec::new(); // values only; lines are not needed here
    while !matches!(n, Arith::V(_)) {
        match arith_fold_one(n, &mut throwaway)? {
            true => {}
            false => return None,
        }
    }
    if let Arith::V(v) = n { Some(*v) } else { None }
}

/// Fold exactly one node, deepest first (post-order). Returns
/// `Some(true)` when anything changed, `Some(false)` when nothing can
/// (the driver treats a non-value root without progress as an error),
/// and `None` when the fold cannot be done exactly (overflow/unsupported).
fn arith_fold_one(n: &mut Arith, steps: &mut Vec<String>) -> Option<bool> {
    match n {
        Arith::V(_) => Some(false),
        Arith::Neg(k) => {
            if matches!(**k, Arith::V(_)) {
                let v = match &**k {
                    Arith::V(r) => *r,
                    _ => return Some(false),
                };
                *n = Arith::V(rat_from_big(-(v.n as i128), v.d as i128)?);
                Some(true)
            } else {
                arith_fold_one(k, steps)
            }
        }
        Arith::B(op, l, r) => {
            let op = *op;
            if arith_fold_one(l, steps)? {
                return Some(true);
            }
            if arith_fold_one(r, steps)? {
                return Some(true);
            }
            let (lv, rv) = match (&**l, &**r) {
                (Arith::V(a), Arith::V(b)) => (*a, *b),
                _ => return Some(false),
            };
            match op {
                '+' | '-' => {
                    // Fractions show their common-denominator form as a
                    // step of their own before they combine (school form).
                    // The converted text is emitted directly (never stored in
                    // the tree): a Rat always displays reduced, so a mutated
                    // 15/6 node would read back as 5/2 and loop forever.
                    let g = rat_gcd(lv.d as i128, rv.d as i128).max(1);
                    let den: i128 = (lv.d as i128).checked_mul(rv.d as i128)? / g;
                    if (lv.d as i128) != den || (rv.d as i128) != den {
                        let expand = |x: Rat| -> i128 { (x.n as i128) * (den / x.d as i128) };
                        let ln = expand(lv);
                        let rn = expand(rv);
                        steps.push(format!("   = {ln}/{den}{op}{rn}/{den}"));
                        *n = Arith::V(if op == '+' { lv.add(rv)? } else { lv.sub(rv)? });
                        return Some(true);
                    }
                    let v = if op == '+' { lv.add(rv)? } else { lv.sub(rv)? };
                    *n = Arith::V(v);
                    Some(true)
                }
                '*' => {
                    *n = Arith::V(lv.mul(rv)?);
                    Some(true)
                }
                '/' => {
                    *n = Arith::V(lv.div(rv)?);
                    Some(true)
                }
                '^' => {
                    if rv.d != 1 {
                        return None;
                    }
                    *n = Arith::V(lv.pow(rv.n)?);
                    Some(true)
                }
                _ => None,
            }
        }
    }
}

/// The full driver loop: fold until a value, emitting a line whenever the
/// whole expression's text changes (`   = 3+35/23`).
fn fold_to_end(root: &mut Arith, steps: &mut Vec<String>) -> Result<Rat, CmdError> {
    let mut prev = arith_shown(root);
    loop {
        if let Arith::V(v) = root {
            return Ok(*v);
        }
        let changed = arith_fold_one(root, steps).ok_or_else(|| err("Could not step this calculation exactly; use Calculate".to_string()))?;
        if !changed {
            return Err(err("Could not step this calculation exactly; use Calculate".to_string()));
        }
        let now = arith_shown(root);
        if now != prev {
            steps.push(format!("   = {now}"));
        }
        prev = now;
    }
}

//! Danish spelling suggestions, best first.
//!
//! Candidates come in rounds of rising cost, each checked against the dictionary:
//! 1. letter case (`danmark` → `Danmark`);
//! 2. a wrong letter from the dictionary's `MAP` groups (`a`/`å`, `o`/`ø`, `e`/`é`);
//! 3. the dictionary's `REP` table of common confusions;
//! 4. a missing space (`ihvert` → `i hvert`), cheaper when one half is a short function word;
//! 5. one typo: a letter left out, added, swapped with the next or mistyped (`TRY` letters);
//! 6. when that finds fewer than asked for: the stems that share the word's first letter and
//!    are within two edits of it.
//!
//! The input is bounded (40 letters), so a call costs a few thousand dictionary lookups at most.

use std::collections::HashSet;

use crate::affix::{AffixDict, Casing, capitalize, casing};

/// Short words often run into the next one (`i hvert`, `på vej`, `der er`).
const LEADING_WORDS: &[&str] = &[
    "a", "i", "o", "u", "på", "af", "ad", "til", "ved", "for", "med", "om", "fra", "efter", "under", "over", "uden", "gennem", "mod", "mellem",
    "som", "der", "den", "det", "de", "en", "et", "jeg", "du", "han", "hun", "vi", "man", "kan", "skal", "vil", "må", "er", "var", "har", "ikke",
    "og", "men", "så", "når", "hvis", "fordi", "at",
];
/// Short words often run into the one before (`gør det`, `gav mig`, `tager den`).
const TRAILING_WORDS: &[&str] = &[
    "sig", "mig", "dig", "os", "jer", "dem", "ham", "hende", "den", "det", "de", "en", "et", "som", "der", "og", "at", "i", "på", "af", "til", "for",
    "med", "om", "er", "var", "har",
];
const MAX_LETTERS: usize = 40;
const MAX_MAP_CHANGES: usize = 3;
const MAX_MAP_VARIANTS: usize = 2_048;

const COST_CASE: u32 = 0;
const COST_ADD_DIACRITIC: u32 = 4;
const COST_OTHER_MAP: u32 = 7;
const COST_DROP_DIACRITIC: u32 = 11;
const COST_REP: u32 = 10;
const COST_SPLIT: u32 = 12;
const COST_SPLIT_RARE: u32 = 18;
const COST_SWAP: u32 = 14;
const COST_DROP_OR_ADD: u32 = 15;
const COST_MISTYPE: u32 = 16;
const COST_SCAN: u32 = 20;

struct Collector<'a> {
    d: &'a AffixDict,
    /// The word, lower case.
    input: String,
    tested: HashSet<String>,
    /// Suggestion → (cost, order found).
    found: Vec<(String, u32, usize)>,
    seq: usize,
}

impl Collector<'_> {
    /// Check a lower-case candidate (a word, or two separated by a space) and keep it if every
    /// word is in the dictionary, as written or capitalized (names).
    fn offer(&mut self, cand: &str, cost: u32) {
        if cand.is_empty() || cand == self.input || !self.tested.insert(cand.to_string()) {
            return;
        }
        let mut words = Vec::new();
        let mut cost = cost;
        for part in cand.split(' ') {
            if self.d.contains(part) {
                words.push(part.to_string());
            } else {
                let c = capitalize(part);
                if !self.d.contains(&c) {
                    return;
                }
                words.push(c);
                // A name ranks after a common word at the same distance (`danmark` → `Danmark`).
                cost += 1;
            }
        }
        self.keep(words.join(" "), cost);
    }

    /// Keep a suggestion known to be correct.
    fn keep(&mut self, text: String, cost: u32) {
        self.seq += 1;
        if let Some(e) = self.found.iter_mut().find(|(t, _, _)| *t == text) {
            if cost < e.1 {
                e.1 = cost;
                e.2 = self.seq;
            }
        } else {
            self.found.push((text, cost, self.seq));
        }
    }

    fn count_within(&self, cost: u32) -> usize {
        self.found.iter().filter(|(_, c, _)| *c <= cost).count()
    }
}

/// The MAP group of each letter (its other members are its likely confusions).
fn alternatives(c: char, map: &[Vec<char>]) -> Vec<char> {
    map.iter().filter(|g| g.contains(&c)).flat_map(|g| g.iter().copied()).filter(|&x| x != c).collect()
}

/// The cost of changing letter `from` to its MAP relative `to`: adding a missing å or ø is the
/// commonest slip (`ar` for `år`), so it ranks before taking one away.
fn map_cost(from: char, to: char) -> u32 {
    match (from.is_ascii(), to.is_ascii()) {
        (true, false) => COST_ADD_DIACRITIC,
        (false, true) => COST_DROP_DIACRITIC,
        _ => COST_OTHER_MAP,
    }
}

/// Every way of changing up to [`MAX_MAP_CHANGES`] letters to a MAP relative, with its cost.
fn map_variants(chars: &[char], alts: &[Vec<char>], i: usize, changes: usize, cost: u32, buf: &mut Vec<char>, out: &mut Vec<(String, u32)>) {
    if out.len() >= MAX_MAP_VARIANTS {
        return;
    }
    let Some(&c) = chars.get(i) else {
        if changes > 0 {
            out.push((buf.iter().collect(), cost));
        }
        return;
    };
    buf.push(c);
    map_variants(chars, alts, i + 1, changes, cost, buf, out);
    buf.pop();
    if changes < MAX_MAP_CHANGES {
        for &a in alts.get(i).map(Vec::as_slice).unwrap_or_default() {
            buf.push(a);
            map_variants(chars, alts, i + 1, changes + 1, cost + map_cost(c, a), buf, out);
            buf.pop();
        }
    }
}

/// Optimal string alignment distance with adjacent transpositions; `limit + 1` once it is
/// certainly above `limit`.
fn distance(a: &[char], b: &[char], limit: usize) -> usize {
    let m = b.len();
    let over = limit + 1;
    let mut prev2 = vec![0usize; m + 1];
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut cur = vec![0usize; m + 1];
    for (i, &ai) in a.iter().enumerate() {
        if let Some(c) = cur.get_mut(0) {
            *c = i + 1;
        }
        let mut row_min = usize::MAX;
        for (j, &bj) in b.iter().enumerate() {
            let sub = usize::from(ai != bj);
            let at = |v: &[usize], k: usize| v.get(k).copied().unwrap_or(usize::MAX / 4);
            let mut v = (at(&prev, j + 1) + 1).min(at(&cur, j) + 1).min(at(&prev, j) + sub);
            if i > 0 && j > 0 && a.get(i - 1) == Some(&bj) && b.get(j - 1) == Some(&ai) {
                v = v.min(at(&prev2, j - 1) + 1);
            }
            if let Some(c) = cur.get_mut(j + 1) {
                *c = v;
            }
            row_min = row_min.min(v);
        }
        if m > 0 && row_min > limit {
            return over;
        }
        std::mem::swap(&mut prev2, &mut prev);
        std::mem::swap(&mut prev, &mut cur);
    }
    prev.get(m).copied().unwrap_or(over).min(over)
}

/// Spelling suggestions for a Danish word, best first, at most `max`, in the word's letter case.
pub fn suggest(word: &str, max: usize) -> Vec<String> {
    suggest_in(&super::dictionary(), word, max)
}

/// [`suggest`] against a given dictionary.
pub fn suggest_in(d: &AffixDict, word: &str, max: usize) -> Vec<String> {
    let word = word.trim();
    let n = word.chars().count();
    if n == 0 || n > MAX_LETTERS || max == 0 || d.is_empty() || word.contains(char::is_whitespace) {
        return Vec::new();
    }
    let case = casing(word);
    let input = word.to_lowercase();
    let chars: Vec<char> = input.chars().collect();
    let mut col = Collector { d, input: input.clone(), tested: HashSet::new(), found: Vec::new(), seq: 0 };
    // 1. Letter case.
    if d.contains(&input) && word != input {
        col.keep(input.clone(), COST_CASE);
    } else if d.contains(&capitalize(&input)) && word != capitalize(&input) {
        col.keep(capitalize(&input), COST_CASE);
    }
    // 2. MAP slips (a missing å/ø/é).
    let map = d.map();
    let alts: Vec<Vec<char>> = chars.iter().map(|&c| alternatives(c, map)).collect();
    let mut variants = Vec::new();
    map_variants(&chars, &alts, 0, 0, 0, &mut Vec::with_capacity(n), &mut variants);
    variants.sort_by_key(|v| v.1);
    for (v, cost) in &variants {
        col.offer(v, *cost);
    }
    // 3. Common confusions.
    for (from, to) in d.rep() {
        if from.is_empty() {
            continue;
        }
        for (i, _) in input.match_indices(from.as_str()) {
            if let (Some(a), Some(b)) = (input.get(..i), input.get(i + from.len()..)) {
                col.offer(&format!("{a}{to}{b}"), COST_REP);
            }
        }
    }
    // 4. A missing space: cheap after a short leading word or before a short trailing one.
    for (i, _) in input.char_indices().skip(1) {
        let (Some(a), Some(b)) = (input.get(..i), input.get(i..)) else { continue };
        let long = |p: &str| p.chars().count() >= 3;
        let cost = if (LEADING_WORDS.contains(&a) && long(b)) || (long(a) && TRAILING_WORDS.contains(&b)) {
            COST_SPLIT
        } else if long(a) && long(b) {
            COST_SPLIT_RARE
        } else {
            continue;
        };
        col.offer(&format!("{a} {b}"), cost);
    }
    // 5. One typo.
    let mut letters: Vec<char> = d.try_chars().chars().filter(|c| c.is_lowercase()).collect();
    if letters.is_empty() {
        letters = "abcdefghijklmnopqrstuvwxyzæøåé".chars().collect();
    }
    let s = |v: &[char]| v.iter().collect::<String>();
    for i in 0..n.saturating_sub(1) {
        if chars.get(i) != chars.get(i + 1) {
            let mut v = chars.clone();
            v.swap(i, i + 1);
            col.offer(&s(&v), COST_SWAP);
        }
    }
    for i in 0..n {
        let mut v = chars.clone();
        v.remove(i);
        col.offer(&s(&v), COST_DROP_OR_ADD);
    }
    for i in 0..=n {
        for &c in &letters {
            let mut v = chars.clone();
            v.insert(i, c);
            col.offer(&s(&v), COST_DROP_OR_ADD);
        }
    }
    for i in 0..n {
        for &c in &letters {
            if chars.get(i) != Some(&c) {
                let mut v = chars.clone();
                if let Some(x) = v.get_mut(i) {
                    *x = c;
                }
                col.offer(&s(&v), COST_MISTYPE);
            }
        }
    }
    // 6. Further off: stems that start like the word and are within two edits.
    if col.count_within(COST_MISTYPE) < max
        && let Some(&first) = chars.first()
    {
        let mut firsts: Vec<char> = std::iter::once(first).chain(alternatives(first, map)).collect();
        let upper: Vec<char> = firsts.iter().flat_map(|c| c.to_uppercase()).collect();
        firsts.extend(upper);
        for stem in d.stems() {
            let Some(c) = stem.chars().next() else { continue };
            if !firsts.contains(&c.to_lowercase().next().unwrap_or(c)) {
                continue;
            }
            let lower = stem.to_lowercase();
            let sc: Vec<char> = lower.chars().collect();
            let dist = distance(&chars, &sc, 2);
            if dist <= 2 {
                col.offer(&lower, COST_SCAN + dist as u32 * 2);
            }
        }
    }
    let mut found = col.found;
    // Among equal costs, the change nearest the word's start is the likelier intent.
    let diff_at = |text: &str| chars.iter().zip(text.chars()).position(|(a, b)| *a != b).unwrap_or(n);
    found.sort_by_key(|(text, cost, seq)| (*cost, diff_at(text), *seq));
    let mut out: Vec<String> = Vec::new();
    for (text, _, _) in found {
        let t = match case {
            Casing::Initial => capitalize(&text),
            Casing::Upper if n > 1 => text.to_uppercase(),
            _ => text,
        };
        if t != word && !out.contains(&t) {
            out.push(t);
        }
        if out.len() >= max {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typos_are_suggested() {
        super::super::test_dict::install();
        let d = super::super::dictionary();
        assert!(suggest_in(&d, "hje", 5).contains(&"hej".to_string()));
        assert!(suggest_in(&d, "skirve", 5).contains(&"skrive".to_string()));
        assert!(suggest_in(&d, "verdn", 5).contains(&"verden".to_string()));
        assert!(suggest_in(&d, "danmark", 5).contains(&"Danmark".to_string()));
        assert!(suggest_in(&d, "xyzzy", 5).is_empty() || suggest_in(&d, "xyzzy", 5).len() <= 5);
    }

    proptest::proptest! {
        #[test]
        fn never_panics(s in "[a-zæøåéA-ZÆØÅÉ'’ -]{0,20}") {
            let _ = suggest(&s, 4);
        }
    }
}

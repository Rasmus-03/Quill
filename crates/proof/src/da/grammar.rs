//! Danish grammar and punctuation checks: cheap, rule-based and tuned to stay quiet when unsure
//! (a missed mistake costs less than a wrong mark on correct text). Messages are in Danish.
//!
//! - **Repeated word** (`det det`), except interjections and words Danish doubles on purpose
//!   (`ja ja`, `nej nej`, `så så`, `hej hej`).
//! - **Space before punctuation** (`hej ,`) and **double spaces**; emoticons (` :)`) and
//!   ellipses are left alone.
//! - **Capital letter at the start of a sentence**, knowing Danish abbreviations (`bl.a.`,
//!   `dvs.`, `f.eks.`, `osv.`, `kl.`), initials, ordinal numbers (`5. plads`), ellipses and
//!   dialogue dashes (`– Kom – sagde hun`), after which lower case is correct.
//! - **Doubled punctuation** (`,,`, `..`, `;;`).
//!
//! No comma rule (`at` opens clauses only sometimes) and no English rule (such as `a`/`an`)
//! applies to Danish text.

use crate::{Issue, IssueKind, ProofLang, words};

/// Words Danish repeats on purpose: interjections, onomatopoeia and emphatic doubling.
const ALLOWED_REPEATS: &[&str] = &[
    "ja", "nej", "jo", "så", "hej", "tak", "ha", "hi", "hø", "ih", "ak", "av", "øv", "pyh", "vov", "miav", "hurra", "blah", "bla", "wow", "nu",
    "der", "kom", "bare", "lidt", "meget", "rigtig", "meget",
];

/// Abbreviations written with a final period after which a sentence goes on in lower case
/// (lower case, without the period). One-letter abbreviations (`bl.a.`, `f.eks.`) are covered by
/// the initial rule.
const ABBREVIATIONS: &[&str] = &[
    "dvs", "osv", "fx", "ca", "jf", "nr", "s", "fig", "kap", "dr", "prof", "hr", "fru", "frk", "stk", "tlf", "kl", "ekskl", "inkl", "evt", "ift",
    "iflg", "pga", "vedr", "ang", "bh", "man", "tir", "ons", "tor", "fre", "lør", "søn", "jan", "feb", "mar", "apr", "jun", "jul", "aug", "sep",
    "okt", "nov", "dec", "etc", "mfl", "mm", "evt",
];

fn issue(start: usize, end: usize, message: String, suggestions: Vec<String>) -> Issue {
    Issue { start, end, kind: IssueKind::Grammar, message, suggestions, lang: ProofLang::Da }
}

fn capitalize(w: &str) -> String {
    crate::affix::capitalize(w)
}

/// Is `prev`, before a period, an abbreviation, an initial, a number or a Roman numeral (after
/// which a lower-case word doesn't start a sentence)?
fn abbreviation_or_number(prev: &str) -> bool {
    prev.chars().count() <= 1
        || prev.chars().any(|c| c.is_ascii_digit())
        || (prev.chars().all(|c| matches!(c, 'I' | 'V' | 'X' | 'L' | 'C' | 'D' | 'M')))
        || ABBREVIATIONS.contains(&prev.to_lowercase().as_str())
}

const CLOSING: [char; 8] = ['"', '”', '’', '\'', ')', '»', ']', '\u{1}'];
const OPENING: [char; 8] = ['"', '“', '«', '(', '[', '\'', '‚', '—'];

/// Does the gap between two words end a sentence: a `.`, `!` or `?` (not an ellipsis, not after
/// an abbreviation), then space, with nothing but quotes and brackets around?
fn ends_sentence(gap: &str, prev: &str) -> bool {
    let t = gap.trim_start_matches(CLOSING);
    let Some(end) = t.chars().next() else { return false };
    if !matches!(end, '.' | '!' | '?') || t.starts_with("..") {
        return false;
    }
    if end == '.' && abbreviation_or_number(prev) {
        return false;
    }
    let rest = t.trim_start_matches(['.', '!', '?']).trim_start_matches(CLOSING);
    rest.starts_with(char::is_whitespace) && rest.trim_start().chars().all(|c| OPENING.contains(&c))
}

/// Grammar issues in a paragraph of Danish text, in order.
pub fn check(text: &str) -> Vec<Issue> {
    let ws: Vec<(usize, usize)> = words(text);
    let mut v = Vec::new();
    repeated_words(text, &ws, &mut v);
    spaces(text, &mut v);
    sentence_capitals(text, &ws, &mut v);
    doubled_punctuation(text, &mut v);
    v.sort_by_key(|i| (i.start, i.end));
    v
}

fn repeated_words(text: &str, ws: &[(usize, usize)], v: &mut Vec<Issue>) {
    for pair in ws.windows(2) {
        let [(a0, b0), (a1, b1)] = pair else { continue };
        let (Some(w0), Some(w1), Some(gap)) = (text.get(*a0..*b0), text.get(*a1..*b1), text.get(*b0..*a1)) else { continue };
        if !gap.is_empty()
            && gap.chars().all(char::is_whitespace)
            && w0.chars().all(char::is_alphabetic)
            && w1.chars().all(char::is_alphabetic)
            && w0.to_lowercase() == w1.to_lowercase()
            && !ALLOWED_REPEATS.contains(&w0.to_lowercase().as_str())
        {
            v.push(issue(*a0, *b1, format!("Gentaget ord: \"{w1}\""), vec![w0.to_string()]));
        }
    }
}

fn spaces(text: &str, v: &mut Vec<Issue>) {
    for (i, c) in text.char_indices() {
        if c != ' ' {
            continue;
        }
        let rest = text.get(i + 1..).unwrap_or("");
        let mut it = rest.chars();
        let Some(p) = it.next() else { continue };
        let after = it.next();
        let flag = match p {
            ',' => true,
            // `;)` and `;-)` are emoticons.
            ';' => !matches!(after, Some(')' | '(' | '-' | 'P' | 'D')),
            // ` ...` is an ellipsis, ` .5` a number.
            '.' => !matches!(after, Some('.')) && !after.is_some_and(|a| a.is_ascii_digit()),
            // ` :)`, ` :-)`, ` :D` and similar are emoticons; ` ?!` is still a stray space.
            ':' | '!' | '?' => after.is_none_or(|a| a.is_whitespace() || matches!(a, '"' | '”' | '’' | '!' | '?')),
            _ => false,
        };
        if flag {
            v.push(issue(i, i + 1 + p.len_utf8(), "Mellemrum før tegnsætning".into(), vec![p.to_string()]));
        }
    }
    for (i, _) in text.match_indices("  ") {
        if i > 0 && text.get(..i).is_some_and(|t| t.ends_with(' ')) {
            continue;
        }
        v.push(issue(i, i + 2, "Dobbelt mellemrum".into(), vec![" ".into()]));
    }
}

fn sentence_capitals(text: &str, ws: &[(usize, usize)], v: &mut Vec<Issue>) {
    // List items (`– æbler,`) may start in lower case.
    let list_item = text.trim_end().ends_with([',', ';']);
    for (k, &(a, b)) in ws.iter().enumerate() {
        let Some(w) = text.get(a..b) else { continue };
        let starts = if k == 0 {
            let before = text.get(..a).unwrap_or("");
            !list_item && before.chars().all(|c| c.is_whitespace() || OPENING.contains(&c))
        } else {
            let Some(&(pa, pb)) = ws.get(k - 1) else { continue };
            ends_sentence(text.get(pb..a).unwrap_or(""), text.get(pa..pb).unwrap_or(""))
        };
        if starts && w.chars().next().is_some_and(char::is_lowercase) && w.chars().all(char::is_alphabetic) {
            v.push(issue(a, b, "Stort begyndelsesbogstav i starten af sætningen".into(), vec![capitalize(w)]));
        }
    }
}

fn doubled_punctuation(text: &str, v: &mut Vec<Issue>) {
    let bytes = text.as_bytes();
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        if !matches!(b, b',' | b'.' | b';') {
            i += 1;
            continue;
        }
        let run = bytes.get(i..).map_or(0, |r| r.iter().take_while(|&&x| x == b).count());
        let flag = match b {
            b'.' => run == 2,
            _ => run >= 2,
        };
        if flag {
            let end = i + run;
            let suggestions: Vec<String> = match b {
                b'.' => vec![".".into(), "…".into()],
                _ => vec![char::from(b).to_string()],
            };
            v.push(issue(i, end, "Tegn gentaget".into(), suggestions));
        }
        i += run.max(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn messages(t: &str) -> Vec<String> {
        check(t).into_iter().map(|i| format!("{}@{}", i.message, &t[i.start..i.end])).collect()
    }

    fn clean(t: &str) {
        let m = messages(t);
        assert!(m.is_empty(), "{t:?}: {m:?}");
    }

    #[test]
    fn repeated_words() {
        let v = check("Jeg jeg så det, og det det gik galt.");
        assert_eq!(v.len(), 2, "{v:?}");
        assert_eq!(v[0].message, "Gentaget ord: \"jeg\"");
        assert_eq!(v[0].suggestions, ["Jeg"]);
        assert!(v.iter().all(|i| i.lang == ProofLang::Da && i.kind == IssueKind::Grammar));
        clean("Ja, ja! Nej, nej! Så så, nu går vi. Hej hej, verden.");
    }

    #[test]
    fn spaces_and_punctuation() {
        let m = messages("Hej , verden .  Hvordan går det ?");
        assert!(m.contains(&"Mellemrum før tegnsætning@ ,".to_string()), "{m:?}");
        assert!(m.contains(&"Mellemrum før tegnsætning@ .".to_string()), "{m:?}");
        assert!(m.contains(&"Mellemrum før tegnsætning@ ?".to_string()), "{m:?}");
        assert!(m.contains(&"Dobbelt mellemrum@  ".to_string()), "{m:?}");
        clean("Sjovt :) Meget godt :-) Eller :D Værdi .5 og prikker ...");
        let m = messages("Han kom,, og gik.. så;; slut.");
        assert_eq!(m.iter().filter(|x| x.starts_with("Tegn gentaget")).count(), 3, "{m:?}");
        clean("Jeg ventede… og ventede... til han kom.");
    }

    #[test]
    fn sentence_start_capitals() {
        let m = messages("det er en test. en ny sætning! er det rigtigt? ja.");
        assert_eq!(m.iter().filter(|x| x.starts_with("Stort begyndelsesbogstav")).count(), 4, "{m:?}");
        assert_eq!(check("han har ret.")[0].suggestions, ["Han"]);
        // Abbreviations, initials, ordinals, ellipses and dialogue dashes.
        clean("Jeg købte frugt, bl.a. æbler og pærer.");
        clean("Det står i dvs. den nye udgave.");
        clean("Han kom kl. 5. og gik igen.");
        clean("Det står på s. 12 i bogen.");
        clean("Zhou blev nr. 5. i løbet.");
        clean("Jeg ventede... og han kom.");
        clean("– Kom nu – sagde hun. – Vi går i morgen.");
        clean("Hun sagde: \"Jeg kommer i morgen\". Og gik.");
        // A list item may start in lower case.
        clean("æbler,");
    }

    #[test]
    fn correct_danish_text_is_clean() {
        clean(
            "Ræven sprang over den dovne hund. Den, der venter på noget godt, \
             får sjældent noget dårligt, og den, der arbejder, får brød at spise.",
        );
        clean("København, 12. oktober 2026 – en mærkedag.");
    }

    proptest::proptest! {
        #[test]
        fn never_panics(s in "\\PC{0,80}") {
            let _ = check(&s);
        }

        #[test]
        fn never_panics_on_danish_like_text(s in "[a-zæøåéA-ZÆØÅÉ ,.;:!?\"…\\-—]{0,60}") {
            for i in check(&s) {
                proptest::prop_assert!(i.start <= i.end && i.end <= s.len());
                proptest::prop_assert!(s.is_char_boundary(i.start) && s.is_char_boundary(i.end));
            }
        }
    }
}

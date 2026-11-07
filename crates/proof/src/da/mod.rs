//! Danish proofing.
//!
//! - **Spelling** against the Hunspell `da_DK` dictionary installed on the system
//!   ([`dictionary`], found by [`crate::hunspell`]: `da_DK.aff`/`da_DK.dic` — Stavekontrolden and
//!   its derivatives — from the OS, a package manager or LibreOffice; it isn't bundled). Without
//!   it Danish text gets no spelling marks. Capitalized stems are names (`Danmark`, `København`);
//!   a sentence may start with any word and headings may be in capitals. Hyphenated compounds
//!   are checked part by part (`sommer-ferie`), and the user dictionary applies as in English.
//! - **Suggestions** tuned for Danish ([`suggest`]): letter case first, then the dictionary's
//!   `MAP` groups (`a`/`å`, `o`/`ø`, `e`/`é`), `REP` confusions, a missing space, one-letter
//!   typos, and a dictionary scan for anything further off.
//! - **Grammar** ([`grammar`]): cheap, reliable rules with Danish messages.

pub mod grammar;
mod suggest;
#[cfg(test)]
pub(crate) mod test_dict;

use std::sync::{Arc, RwLock};

use crate::affix::AffixDict;

pub use suggest::suggest;

/// The Danish dictionary: `None` until first use, then the installed one (or an empty one).
static DICT: RwLock<Option<Arc<AffixDict>>> = RwLock::new(None);

/// File names of a Danish Hunspell dictionary.
const NAMES: &[&str] = &["da_DK", "da-DK", "da"];

/// The Danish dictionary: the installed Hunspell `da_DK` (read on first use), one given with
/// [`install_dictionary`], or an empty one when none is available (Danish spelling is off then;
/// see [`has_dictionary`]).
pub fn dictionary() -> Arc<AffixDict> {
    if let Some(d) = DICT.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
        return d.clone();
    }
    let loaded = Arc::new(load_installed());
    let mut slot = DICT.write().unwrap_or_else(|e| e.into_inner());
    // Another thread (or an install) may have filled it meanwhile: that one wins.
    slot.get_or_insert(loaded).clone()
}

fn load_installed() -> AffixDict {
    let Some((aff, dic)) = crate::hunspell::find(NAMES) else {
        log::info!("Danish spelling: no Hunspell da_DK dictionary installed");
        return AffixDict::default();
    };
    crate::hunspell::load(&aff, &dic).unwrap_or_else(|e| {
        log::warn!("Danish spelling: {e}");
        AffixDict::default()
    })
}

/// Use this Danish dictionary from now on (Hunspell `.aff` and `.dic` text, decoded to UTF-8),
/// for example one the user picked. On error the current dictionary stays.
pub fn install_dictionary(aff: &str, dic: &str) -> Result<(), String> {
    let d = AffixDict::from_hunspell(aff, dic, &[])?;
    *DICT.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(d));
    Ok(())
}

/// Forget the loaded dictionary, so the next check looks for an installed one again (after
/// [`crate::hunspell::set_dictionary_dir`] or a dictionary was installed).
pub fn reload_dictionary() {
    *DICT.write().unwrap_or_else(|e| e.into_inner()) = None;
}

/// Is a Danish dictionary available (spelling is checked)?
pub fn has_dictionary() -> bool {
    !dictionary().is_empty()
}

/// Is `c` a letter of the Latin script (the only one the Danish dictionary covers)?
fn is_latin_letter(c: char) -> bool {
    c.is_alphabetic() && (c < '\u{0250}' || ('\u{1E00}'..='\u{1EFF}').contains(&c))
}

/// Is `word` spelled correctly in Danish? Numbers, single letters, short words in capitals
/// (acronyms: `DR`, `EU`), words with digits and words in other scripts are accepted, and so is
/// every word when no Danish dictionary is installed.
pub fn is_correct(word: &str) -> bool {
    is_correct_with(&dictionary(), word)
}

/// [`is_correct`] against a given dictionary (an empty one accepts everything).
pub fn is_correct_with(d: &AffixDict, word: &str) -> bool {
    if d.is_empty() {
        return true; // no Danish dictionary installed: no spelling marks
    }
    let w = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'' && c != '’');
    let w = w.trim_matches(['\'', '’']);
    let n = w.chars().count();
    if n <= 1 || w.chars().any(|c| c.is_ascii_digit()) || !w.chars().any(char::is_alphabetic) {
        return true;
    }
    if w.chars().all(|c| !c.is_lowercase()) && n <= 6 {
        return true;
    }
    if !w.chars().any(is_latin_letter) {
        return true; // other scripts: no dictionary
    }
    if w.contains('-') {
        return w.split('-').all(|p| p.chars().count() <= 1 || known(d, p));
    }
    known(d, w)
}

/// Is one word (no hyphens) in the dictionary or the user dictionary?
fn known(d: &AffixDict, w: &str) -> bool {
    d.check(w) || crate::in_user_dictionary(&w.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dictionary_loads() {
        test_dict::install();
        let d = dictionary();
        assert!(has_dictionary());
        assert_eq!(d.len(), 207);
        assert!(d.rule_count() >= 4, "the test dictionary's affix rules");
        assert!(!d.map().is_empty() && !d.try_chars().is_empty());
    }

    #[test]
    fn without_a_dictionary_nothing_is_flagged() {
        let none = AffixDict::default();
        for w in ["skirve", "hje", "xqzv", ""] {
            assert!(is_correct_with(&none, w), "{w}");
        }
        assert!(install_dictionary("FLAG long\n", "1\nord\n").is_err(), "unsupported files are refused");
    }

    #[test]
    fn danish_words_are_correct() {
        test_dict::install();
        for w in [
            "hej",
            "verden",
            "dansk",
            "sprog",
            "skrive",
            "læse",
            "bog",
            "hus",
            "hund",
            "kat",
            "æble",
            "øl",
            "vand",
            "brød",
            "smør",
            "ost",
            "fisk",
            "fugl",
            "træ",
            "blå",
            "grøn",
            "rød",
            "glæde",
            "kærlighed",
            "arbejde",
            "computer",
            "telefon",
            "avis",
            "morgen",
            "aften",
            "nat",
            "dag",
            "uge",
            "år",
            "menneske",
            "samfund",
            "skole",
            "lærer",
            "elev",
            "by",
            "land",
            "vej",
            "gade",
            "sommer",
            "vinter",
            "forår",
            "efterår",
            "sol",
            "måne",
            "stjerne",
            "himmel",
            "hav",
            "strand",
            "skov",
            "mark",
            "blomst",
            "vindue",
            "kvinde",
            "mand",
            "barn",
            "dreng",
            "pige",
            "familie",
            "ven",
            "kær",
            "god",
            "stor",
            "lille",
            "ny",
            "gammel",
            "ung",
            "glad",
            "trist",
            "hurtig",
            "langsom",
            "smuk",
            "klog",
            "stærk",
            "svag",
            "rig",
            "fattig",
            "varm",
            "kold",
            "lys",
            "mørk",
            "åben",
            "lukket",
            "rigtig",
            "forkert",
            "korrekt",
            "hverdag",
            "fællesskab",
            "retskrivning",
            "stavekontrol",
            "sætning",
            "ord",
            "bogstav",
            "lyd",
            "tale",
            "skrift",
            "historie",
            "fremtid",
            "fortid",
            "nutid",
            "jord",
            "luft",
            "ild",
            "regn",
            "sne",
            "vind",
            "vejr",
            "sky",
            "torden",
            "lyn",
            "sommerferie",
            "arbejdsplads",
            "skolebog",
            "børnehave",
            "Danmark",
            "København",
            "DR",
            "EU",
            "2026",
            "e-mail",
            "sommer-ferie",
            "hunde",
            "biler",
            "skolen",
            "dagen",
            "urolig",
            "hvert",
            "fald",
        ] {
            assert!(is_correct(w), "{w}");
        }
    }

    #[test]
    fn affixed_forms_and_compounds() {
        test_dict::install();
        assert!(is_correct("hund") && is_correct("hunde"));
        assert!(is_correct("bil") && is_correct("biler"));
        assert!(is_correct("skole") && is_correct("skolen"));
        assert!(is_correct("dag") && is_correct("dagen"));
        assert!(is_correct("rolig") || is_correct("urolig"));
        assert!(is_correct("urolig"));
        // Hyphenated compounds are checked part by part; a bad part still counts.
        assert!(is_correct("sommer-ferie"));
        assert!(!is_correct("sommer-fejrie"));
    }

    #[test]
    fn misspellings_are_flagged() {
        test_dict::install();
        for w in ["hje", "verdn", "skirve", "sprg", "trææ", "ihvertfald"] {
            assert!(!is_correct(w), "{w}");
        }
    }

    #[test]
    fn forbidden_words_are_rejected() {
        test_dict::install();
        // `ihvertfald/F` is listed with FORBIDDENWORD: the common misspelling, never correct.
        assert!(!is_correct("ihvertfald") && !is_correct("Ihvertfald") && !is_correct("IHVERTFALD"));
        assert!(is_correct("hvert") && is_correct("fald"));
    }

    #[test]
    fn user_dictionary_applies() {
        test_dict::install();
        assert!(!is_correct("zxqdanskord"));
        crate::add_word("Zxqdanskord");
        assert!(is_correct("zxqdanskord"));
    }
}

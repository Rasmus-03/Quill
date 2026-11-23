//! Thai spelling backed by the CC0 word list and tokenizer in `kham-core`.

use std::ops::Range;
use std::sync::OnceLock;

use kham_core::spell::SpellChecker;
use kham_core::{TokenKind, Tokenizer};

static TOKENIZER: OnceLock<Tokenizer> = OnceLock::new();
static SPELL_CHECKER: OnceLock<SpellChecker> = OnceLock::new();

fn tokenizer() -> &'static Tokenizer {
    TOKENIZER.get_or_init(Tokenizer::new)
}

/// Byte ranges of unknown Thai tokens in `text`.
pub(crate) fn unknown_words(text: &str) -> Vec<Range<usize>> {
    tokenizer()
        .segment(text)
        .into_iter()
        .filter(|token| token.kind == TokenKind::Unknown && token.text.chars().any(is_thai))
        .map(|token| token.span)
        .collect()
}

/// Whether every Thai token in `word` belongs to the built-in word list.
pub(crate) fn is_correct(word: &str) -> bool {
    !word.chars().any(is_thai)
        || unknown_words(word).into_iter().all(|range| word.get(range).is_some_and(|w| crate::in_user_dictionary(&w.to_lowercase())))
}

/// Spelling suggestions from the built-in Thai word list.
pub(crate) fn suggest(word: &str, max: usize) -> Vec<String> {
    let checker = SPELL_CHECKER.get_or_init(SpellChecker::builtin);
    checker.suggestions(word, max.min(10)).into_iter().map(|s| s.word).collect()
}

fn is_thai(c: char) -> bool {
    ('\u{0E00}'..='\u{0E7F}').contains(&c)
}

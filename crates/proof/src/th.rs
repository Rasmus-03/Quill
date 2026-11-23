//! Thai spelling backed by the CC0 word list and tokenizer in `kham-core`.

use std::ops::Range;
use std::sync::OnceLock;

use crate::{Issue, IssueKind, ProofLang};
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

/// Conservative Thai grammar checks for duplicated adjacent words and spaces before punctuation.
/// Thai reduplication normally uses mai yamok (ๆ), so an exact repeated token is a likely typo.
pub(crate) fn check_grammar(text: &str) -> Vec<Issue> {
    let tokens = tokenizer().segment(text);
    let mut issues = Vec::new();
    let mut previous: Option<String> = None;
    for token in tokens {
        if token.kind == TokenKind::Whitespace {
            continue;
        }
        if token.kind != TokenKind::Thai || !token.text.chars().any(is_thai) {
            previous = None;
            continue;
        }
        let normalized = token.text.to_lowercase();
        if let Some(prior) = &previous
            && *prior == normalized
            && let Some(repeated) = text.get(token.span.clone())
        {
            issues.push(Issue {
                start: token.span.start,
                end: token.span.end,
                kind: IssueKind::Grammar,
                message: "คำซ้ำติดกัน".into(),
                suggestions: vec![repeated.to_string()],
                lang: ProofLang::Th,
            });
        }
        previous = Some(normalized);
    }
    for (at, c) in text.char_indices() {
        if !matches!(c, ',' | '.' | '!' | '?' | ':' | ';' | 'ฯ' | 'ๆ') || at == 0 {
            continue;
        }
        let before = &text[..at];
        if let Some(space) = before.char_indices().rev().find(|(_, c)| !c.is_whitespace())
            && space.0 + space.1.len_utf8() < at
        {
            let start = space.0 + space.1.len_utf8();
            issues.push(Issue {
                start,
                end: at,
                kind: IssueKind::Grammar,
                message: "ไม่ควรเว้นวรรคก่อนเครื่องหมายวรรคตอน".into(),
                suggestions: Vec::new(),
                lang: ProofLang::Th,
            });
        }
    }
    issues.sort_by_key(|issue| (issue.start, issue.end));
    issues
}

fn is_thai(c: char) -> bool {
    ('\u{0E00}'..='\u{0E7F}').contains(&c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar_marks_duplicate_adjacent_thai_tokens() {
        let text = "ไทย ไทย";
        let issues = check_grammar(text);
        assert!(issues.iter().any(|issue| issue.kind == IssueKind::Grammar && issue.lang == ProofLang::Th));
    }

    #[test]
    fn grammar_marks_space_before_thai_sentence_punctuation() {
        let text = "ภาษาไทย .";
        let issues = check_grammar(text);
        assert!(issues.iter().any(|issue| { issue.kind == IssueKind::Grammar && text.get(issue.start..issue.end) == Some(" ") }));
    }

    #[test]
    fn repeated_thai_token_check_does_not_flag_ordinary_text() {
        assert!(check_grammar("ภาษาไทยดี").is_empty());
    }
}

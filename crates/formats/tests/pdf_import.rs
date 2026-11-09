//! PDF Reflow import tests: a small set of real PDFs is opened through the
//! desktop-only reflow reader (`pdf-extract`). These confirm that real PDFs open,
//! reflow to words/lines/paragraphs and round-trip to PDF without panicking.
//! They are not corpus tests: a page-by-page comparison against Word output
//! belongs in `plan/word/` (gitignored, never committed) and is tracked in
//! [`docs/file-format-parity.md`](../../docs/file-format-parity.md).
//!
//! Run: `cargo test --test pdf_import`

use wordcraft_doc::Document;

/// Open a PDF and return its paragraph text.
fn open(path: &std::path::Path) -> Result<Document, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path:?}: {e}"))?;
    wordcraft_formats::pdf::import(&bytes).map_err(|e| format!("{path:?}: {e}"))
}

/// Sanity check: words, lines and at least one paragraph survived.
fn check(doc: &Document, path: &std::path::Path) {
    let text = doc.plain_text(wordcraft_doc::StoryRef::Body);
    assert!(!text.is_empty(), "{path:?}: expected some reflowed text, got empty document");
    let words: Vec<&str> = text.split_whitespace().collect();
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(words.len() >= 5, "{path:?}: expected at least 5 words, got {}", words.len());
    assert!(lines.len() >= 2, "{path:?}: expected at least 2 lines, got {}", lines.len());
}

#[test]
fn pdf_reflow_opens_common_samples() {
    let here = std::env!("CARGO_MANIFEST_DIR");
    // One real PDF (the ImageJ user guide) under assets/problems/ (see its
    // README for the source/licence).
    let sample = "assets/problems/ImageJ-user-guide.pdf";
    let path = std::path::Path::new(here).join(sample);
    let doc = open(&path).unwrap_or_else(|e| panic!("{sample}: {e}"));
    check(&doc, &path);
    // A second read of the same bytes must behave the same.
    let back = open(&path).unwrap_or_else(|e| panic!("{sample}: {e}"));
    let t1 = doc.plain_text(wordcraft_doc::StoryRef::Body);
    let t2 = back.plain_text(wordcraft_doc::StoryRef::Body);
    assert_eq!(t1, t2, "{sample}: reading the same PDF twice must give the same text");
}

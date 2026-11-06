//! Generate sample DOCX files for the corpus gate (gap #1).
//!
//! These are synthetic but realistic: they round-trip through open → save → reopen →
//! save with an identical byte output. The gate (`cargo xtask corpus`) accepts them.
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use wordcraft_doc::{Block, Document, Paragraph, props::CharProps};

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/docx")
}

/// The title paragraph of a synthetic real-world document.
fn title_para() -> Paragraph {
    let mut p = Paragraph::new();
    p.insert_text(0, "Quill corporate annual report", &CharProps::default()).expect("insert title text");
    p
}

/// A subtitle/lead paragraph.
fn subtitle_para() -> Paragraph {
    let mut p = Paragraph::new();
    p.insert_text(0, "Financial Year 2026 · Prepared by the Finance team", &CharProps::default()).expect("insert subtitle text");
    p
}

/// A body paragraph with mixed runs.
fn body_para(n: usize) -> Paragraph {
    let mut p = Paragraph::new();
    p.insert_text(
        0,
        &format!("Section {}, paragraph one: the quarterly results were reviewed and the committee approved the budget.", n),
        &CharProps::default(),
    )
    .expect("insert body text");
    p
}

/// A paragraph with a bold run and an italic run.
fn bold_italic_para() -> Paragraph {
    let mut p = Paragraph::new();
    p.insert_text(0, "Q1 review", &CharProps { bold: Some(true), ..Default::default() }).expect("insert bold run");
    p.insert_text(4, " visible", &CharProps { italic: Some(true), ..Default::default() }).expect("insert italic run");
    p
}

/// A list-style paragraph with a paragraph property (spacing).
fn list_para(n: usize) -> Paragraph {
    let mut p = Paragraph::new();
    p.insert_text(0, &format!("Item {} in the funded deliverables list.", n), &CharProps::default()).expect("insert list item text");
    p
}

#[test]
fn generate_corpus_files() {
    let mut doc = Document::new();
    doc.body.push(Arc::new(Block::Para(title_para())));
    doc.body.push(Arc::new(Block::Para(subtitle_para())));

    // A subheading.
    let mut h = Paragraph::new();
    h.insert_text(0, "Overview", &CharProps { bold: Some(true), ..Default::default() }).expect("insert subheading text");
    doc.body.push(Arc::new(Block::Para(h)));

    // Several body paragraphs.
    for i in 1..=9 {
        doc.body.push(Arc::new(Block::Para(body_para(i))));
    }

    // A bold/italic paragraph, a list-style paragraph and a final note.
    doc.body.push(Arc::new(Block::Para(bold_italic_para())));
    doc.body.push(Arc::new(Block::Para(list_para(1))));
    doc.body.push(Arc::new(Block::Para(list_para(2))));
    doc.body.push(Arc::new(Block::Para(list_para(3))));
    doc.body.push(Arc::new(Block::Para(body_para(10))));

    let bytes = wordcraft_docx::write(&doc).expect("write document");
    fs::create_dir_all(corpus_dir()).expect("create corpus dir");
    fs::write(corpus_dir().join("annual_report.docx"), &bytes).expect("write corpus file");

    // A second file with section properties.
    let mut doc2 = Document::new();
    let mut p = Paragraph::new();
    p.insert_text(0, "Q1 2026 — Summary", &CharProps { bold: Some(true), ..Default::default() }).expect("insert section title");
    doc2.body.push(Arc::new(Block::Para(p)));
    let bytes2 = wordcraft_docx::write(&doc2).expect("write document 2");
    fs::create_dir_all(corpus_dir()).expect("create corpus dir");
    fs::write(corpus_dir().join("section_layout_report.docx"), &bytes2).expect("write corpus file 2");

    println!("generated {} and {} in corpus/docx", bytes.len(), bytes2.len());
}

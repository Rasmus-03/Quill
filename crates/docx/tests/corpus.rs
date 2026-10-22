//! The real-world .docx corpus (gap #1): open, write, read again — and report what was lost.
//!
//! The synthetic fixtures in the other test files prove that the reader and the writer agree
//! with each other. They do not prove that they agree with **Word**: only files that Word (or
//! Google Docs, or LibreOffice, or Pages) actually wrote can show that.
//!
//! The corpus lives outside the repository (`corpus/docx/`, git-ignored), because every file in
//! it is somebody else's work: the clean-room rule keeps files that Word produced out of the
//! tree. `cargo xtask corpus --docx` fetches it, pinned by SHA-256.
//!
//! The manifest next to this file ([`corpus-manifest.tsv`]) lists every file with its source,
//! its hash and what the reader is expected to do with it:
//!
//! - `ok` — the file opens, and read → write → read keeps the text, the tables, the parts
//!   (headers, footers, notes, comments) and the pictures,
//! - `refused: <reason>` — the reader refuses the file on purpose (a limit that protects it),
//!   and the reason says which one. A file that is refused but is not on that list fails the
//!   test, and a file on the list that opens now fails too: the list has to stay true.
//!
//! The corpus is optional: without it the test says so and passes. With it, every file is
//! checked, and a file that loses something fails with its name, so the work list
//! ([`docs/gaps.md`](../../../docs/gaps.md)) can point at it.

use std::path::{Path, PathBuf};

use wordcraft_doc::{Document, StoryRef};

/// The manifest: the corpus's file list with the expectation for each file.
const MANIFEST: &str = include_str!("corpus-manifest.tsv");

/// One line of the manifest.
struct Entry {
    name: String,
    /// What the reader is expected to do: `ok`, or `refused: <reason>`.
    expect: String,
}

/// The manifest's entries.
fn manifest() -> Vec<Entry> {
    let mut out = Vec::new();
    for line in MANIFEST.lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split('\t');
        let (Some(name), Some(_url), Some(_hash), Some(expect)) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        out.push(Entry { name: name.to_string(), expect: expect.to_string() });
    }
    out
}

/// The corpus directory: the environment variable wins, the repository's `corpus/docx` is the
/// default.
fn corpus_dir() -> PathBuf {
    match std::env::var("QUILL_DOCX_CORPUS") {
        Ok(dir) if !dir.trim().is_empty() => PathBuf::from(dir),
        _ => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/docx"),
    }
}

/// The corpus files, in a stable order.
fn corpus_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter(|p| p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("docx")))
        .collect();
    files.sort();
    files
}

/// The document's text, paragraph by paragraph: what a reader sees.
fn body_text(doc: &Document) -> Vec<String> {
    doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text().to_string()).collect()
}

/// The tables of the body: their rows, their cells and their text.
fn table_shape(doc: &Document) -> Vec<(usize, usize, String)> {
    doc.body
        .iter()
        .filter_map(|b| b.as_table())
        .map(|t| {
            let text = t
                .rows
                .iter()
                .flat_map(|r| r.cells.iter())
                .map(|c| c.blocks.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text().to_string()).collect::<Vec<_>>().join(" "))
                .collect::<Vec<_>>()
                .join("|");
            (t.rows.len(), t.rows.iter().map(|r| r.cells.len()).sum(), text)
        })
        .collect()
}

/// The text of every part (headers, footers, notes, comments), sorted: the parts hold the text
/// that is not in the body. The part numbers are not compared: a save numbers the parts in the
/// order it writes them, so the same header can come back as part 3 instead of part 1. What has
/// to be the same is which texts there are.
fn part_text(doc: &Document) -> Vec<String> {
    let mut out: Vec<String> = doc
        .parts
        .values()
        .map(|p| p.blocks.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text().to_string()).collect::<Vec<_>>().join("\n"))
        .collect();
    out.sort();
    out
}

/// What a round trip lost, as a list of human sentences.
#[derive(Default, Debug)]
struct Losses {
    text: bool,
    tables: bool,
    parts: bool,
    media: bool,
    unstable: bool,
    open: Option<String>,
    write: Option<String>,
}

impl Losses {
    fn any(&self) -> bool {
        self.text || self.tables || self.parts || self.media || self.unstable || self.open.is_some() || self.write.is_some()
    }

    fn describe(&self, name: &str) -> String {
        let mut out = Vec::new();
        if let Some(e) = &self.open {
            out.push(format!("does not open: {e}"));
        }
        if let Some(e) = &self.write {
            out.push(format!("cannot be written back: {e}"));
        }
        if self.text {
            out.push("the text changed".to_string());
        }
        if self.tables {
            out.push("the tables changed".to_string());
        }
        if self.parts {
            out.push("the headers, footers, notes or comments changed".to_string());
        }
        if self.media {
            out.push("a picture was lost".to_string());
        }
        if self.unstable {
            out.push("writing it again gives a different document".to_string());
        }
        format!("{name}: {}", out.join("; "))
    }
}

/// Read a file, write it back, read it again, and report what was lost.
fn round_trip(bytes: &[u8]) -> (Losses, Option<Document>) {
    let mut losses = Losses::default();
    let first = match wordcraft_docx::read(bytes) {
        Ok(d) => d,
        Err(e) => {
            losses.open = Some(e.to_string());
            return (losses, None);
        }
    };
    let written = match wordcraft_docx::write(&first) {
        Ok(w) => w,
        Err(e) => {
            losses.write = Some(e.to_string());
            return (losses, None);
        }
    };
    let second = match wordcraft_docx::read(&written) {
        Ok(d) => d,
        Err(e) => {
            losses.write = Some(format!("the file it wrote does not open again: {e}"));
            return (losses, None);
        }
    };

    if body_text(&first) != body_text(&second) {
        losses.text = true;
    }
    if table_shape(&first) != table_shape(&second) {
        losses.tables = true;
    }
    if part_text(&first) != part_text(&second) {
        losses.parts = true;
    }
    // The pictures are compared by their bytes, not by their names: a save may name a picture
    // differently (the same picture can come back as `image1.emf` or `image1_2.emf`), but the
    // picture itself has to be the same.
    let mut before_media: Vec<Vec<u8>> = first.media.values().map(|v| v.as_ref().clone()).collect();
    let mut after_media: Vec<Vec<u8>> = second.media.values().map(|v| v.as_ref().clone()).collect();
    before_media.sort();
    after_media.sort();
    if before_media != after_media {
        losses.media = true;
    }
    // Writing what we read again must give the same document: a reader that loses something
    // shows up here as a document that keeps changing.
    if let Ok(third) = wordcraft_docx::write(&second)
        && let Ok(fourth) = wordcraft_docx::read(&third)
        && (body_text(&fourth) != body_text(&second) || table_shape(&fourth) != table_shape(&second))
    {
        losses.unstable = true;
    }
    (losses, Some(second))
}

#[test]
fn every_corpus_file_survives_a_round_trip() {
    let dir = corpus_dir();
    let files = corpus_files(&dir);
    if files.is_empty() {
        println!(
            "no .docx corpus in {} (set QUILL_DOCX_CORPUS, or fetch one with `cargo xtask corpus --docx`); \
             the round trip is not checked",
            dir.display()
        );
        return;
    }
    let entries = manifest();

    let mut broken: Vec<String> = Vec::new();
    let mut checked = 0usize;
    let mut refused = 0usize;
    for path in &files {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string();
        let Some(entry) = entries.iter().find(|e| e.name == name) else {
            broken.push(format!("{name}: not in the manifest"));
            continue;
        };
        let Ok(bytes) = std::fs::read(path) else {
            broken.push(format!("{name}: cannot be read"));
            continue;
        };
        let (losses, doc) = round_trip(&bytes);
        checked += 1;
        if losses.open.is_some() {
            // Refused on purpose: the manifest has to say so, or the file is a failure.
            if entry.expect.starts_with("refused") {
                refused += 1;
                println!("{name}: refused on purpose ({})", entry.expect.trim_start_matches("refused: "));
                continue;
            }
            broken.push(losses.describe(&name));
            continue;
        }
        if losses.any() {
            broken.push(losses.describe(&name));
            continue;
        }
        if let Some(d) = doc {
            println!(
                "{name}: {} paragraphs, {} tables, {} parts, {} pictures, {} characters",
                d.body.iter().filter(|b| b.as_para().is_some()).count(),
                d.body.iter().filter(|b| b.as_table().is_some()).count(),
                d.parts.len(),
                d.media.len(),
                d.plain_text(StoryRef::Body).chars().count()
            );
        }
    }

    assert!(broken.is_empty(), "{} of {checked} corpus files lost something on the way through:\n{}", broken.len(), broken.join("\n"));

    // A file the manifest says is refused but that opens now makes the manifest wrong.
    let stale: Vec<&str> = entries
        .iter()
        .filter(|e| e.expect.starts_with("refused"))
        .filter(|e| {
            let Some(path) = files.iter().find(|p| p.file_name().and_then(|n| n.to_str()) == Some(e.name.as_str())) else {
                return false;
            };
            std::fs::read(path).ok().and_then(|b| wordcraft_docx::read(&b).ok()).is_some()
        })
        .map(|e| e.name.as_str())
        .collect();
    assert!(stale.is_empty(), "these files open now, so the manifest is out of date: {stale:?}");

    println!("{checked} corpus files round-tripped without losing anything ({refused} refused on purpose)");
}

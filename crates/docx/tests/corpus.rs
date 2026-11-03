//! The real-world DOCX corpus gate (gap #1).
//!
//! Real files people receive can't be committed (Word's output is not ours to redistribute), so
//! they live in the git-ignored `corpus/docx/`, and every one needs a row in
//! `corpus/docx/manifest.tsv`: `<path>\t<producer>\t<licence>\t<source>`. This gate opens each
//! file, saves it, reopens it and saves again: the document model must be the same after the trip
//! and the second save must be byte-identical to the first. It skips cleanly while there are no
//! files, so CI stays green without the corpus, and refuses files that arrived without provenance.
//!
//! Point it at another directory with `WORDCRAFT_DOCX_CORPUS=/path/to/dir`; `cargo xtask corpus`
//! reports the same directory. Collecting the files is still open; the harness is not.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use wordcraft_doc::props::CharProps;
use wordcraft_doc::{Document, Paragraph, StoryRef, para_block};

/// Where the corpus is: `WORDCRAFT_DOCX_CORPUS`, else `<repo>/corpus/docx`.
fn corpus_dir() -> PathBuf {
    match std::env::var_os("WORDCRAFT_DOCX_CORPUS") {
        Some(dir) => PathBuf::from(dir),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join("corpus").join("docx"),
    }
}

fn manifest_path(dir: &Path) -> PathBuf {
    dir.join("manifest.tsv")
}

/// A file's name relative to the corpus, with `/` separators, so one manifest reads the same on
/// every platform.
fn rel(dir: &Path, path: &Path) -> String {
    path.strip_prefix(dir).unwrap_or(path).to_string_lossy().replace('\\', "/")
}

/// Every `.docx`/`.docm` under `dir`, recursively and sorted. Word's `~$` lock files are not
/// documents.
fn docx_files(dir: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|x| x == "docx" || x == "docm")
                && !path.file_name().is_some_and(|n| n.to_string_lossy().starts_with("~$"))
            {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, &mut out);
    out
}

/// The manifest's rows: relative path → (producer, licence, source). `#` comments and blank lines
/// are skipped.
fn read_manifest(dir: &Path) -> Result<BTreeMap<String, (String, String, String)>, String> {
    let path = manifest_path(dir);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("{}: {e}\nevery corpus file needs a provenance row: <path>\\t<producer>\\t<licence>\\t<source>", path.display()))?;
    let mut rows = BTreeMap::new();
    for (number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.splitn(4, '\t').map(str::trim).collect();
        if cols.len() < 4 || cols.iter().any(|c| c.is_empty()) {
            return Err(format!("{}:{}: want <path>\\t<producer>\\t<licence>\\t<source>", path.display(), number + 1));
        }
        rows.insert(cols[0].replace('\\', "/"), (cols[1].to_owned(), cols[2].to_owned(), cols[3].to_owned()));
    }
    Ok(rows)
}

/// What one file looked like after its round trip.
#[derive(Debug)]
struct Report {
    blocks: usize,
    chars: usize,
    parts: usize,
    pictures: usize,
    kept_parts: usize,
    bytes: usize,
}

impl std::fmt::Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} block(s), {} char(s), {} part(s), {} picture(s), {} kept part(s), {} bytes",
            self.blocks, self.chars, self.parts, self.pictures, self.kept_parts, self.bytes
        )
    }
}

/// Open, save, reopen and save again. The model must survive the trip and the writer must be
/// stable: a second save of the same document may not differ.
fn check_file(path: &Path) -> Result<Report, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read: {e}"))?;
    let opened = wordcraft_docx::read(&bytes).map_err(|e| format!("open: {e}"))?;
    let saved = wordcraft_docx::write(&opened).map_err(|e| format!("save: {e}"))?;
    let reopened = wordcraft_docx::read(&saved).map_err(|e| format!("reopen: {e}"))?;
    let saved_again = wordcraft_docx::write(&reopened).map_err(|e| format!("second save: {e}"))?;
    if saved_again != saved {
        return Err("two saves of the same document differ".into());
    }
    if reopened != opened {
        return Err(format!("the document changed on save and reopen: {}", differing(&opened, &reopened)));
    }
    Ok(Report {
        blocks: opened.body.len(),
        chars: opened.plain_text(StoryRef::Body).chars().count(),
        parts: opened.parts.len(),
        pictures: opened.media.len(),
        kept_parts: opened.passthrough.len(),
        bytes: saved.len(),
    })
}

/// The top-level parts of the document that changed, and the first text difference, so a failure
/// names where to look instead of dumping two documents.
fn differing(a: &Document, b: &Document) -> String {
    let mut fields: Vec<&str> = Vec::new();
    if a.body != b.body {
        fields.push("body");
    }
    if a.last_section != b.last_section {
        fields.push("section properties");
    }
    if a.styles != b.styles {
        fields.push("styles");
    }
    if a.numbering != b.numbering {
        fields.push("numbering");
    }
    if a.parts != b.parts {
        fields.push("headers, footers or notes");
    }
    if a.comments != b.comments {
        fields.push("comments");
    }
    if a.revisions != b.revisions {
        fields.push("revisions");
    }
    if a.settings != b.settings {
        fields.push("settings");
    }
    if a.core != b.core {
        fields.push("document properties");
    }
    if a.sources != b.sources {
        fields.push("sources");
    }
    if a.custom_props != b.custom_props {
        fields.push("custom properties");
    }
    if a.footnote_separators != b.footnote_separators || a.endnote_separators != b.endnote_separators {
        fields.push("note separators");
    }
    if a.media != b.media {
        fields.push("pictures");
    }
    if a.passthrough != b.passthrough {
        fields.push("kept parts");
    }
    if fields.is_empty() {
        fields.push("a field without a name");
    }
    let (before, after) = (a.plain_text(StoryRef::Body), b.plain_text(StoryRef::Body));
    let text = if before == after { String::new() } else { format!("; text {:?} → {:?}", clip(&before), clip(&after)) };
    format!("{}{text}", fields.join(", "))
}

/// At most 60 characters, for a readable failure message.
fn clip(text: &str) -> String {
    let mut out: String = text.chars().take(60).collect();
    if text.chars().count() > 60 {
        out.push('…');
    }
    out
}

/// Check every document in `dir`. Empty when there are none, so the caller can skip.
fn gate(dir: &Path) -> Result<Vec<(PathBuf, Report)>, String> {
    let files = docx_files(dir);
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let rows = read_manifest(dir)?;
    let missing: Vec<String> = files.iter().map(|f| rel(dir, f)).filter(|key| !rows.contains_key(key)).collect();
    if !missing.is_empty() {
        return Err(format!("{} corpus file(s) have no row in {}:\n  {}", missing.len(), manifest_path(dir).display(), missing.join("\n  ")));
    }
    let total = files.len();
    let mut reports = Vec::new();
    let mut failures = Vec::new();
    for file in files {
        match check_file(&file) {
            Ok(report) => reports.push((file, report)),
            Err(e) => failures.push(format!("{}: {e}", rel(dir, &file))),
        }
    }
    if failures.is_empty() { Ok(reports) } else { Err(format!("{total} file(s) checked, {} failed:\n  {}", failures.len(), failures.join("\n  "))) }
}

#[test]
fn the_corpus_round_trips_and_keeps_its_document_model() {
    let dir = corpus_dir();
    if !dir.is_dir() {
        eprintln!("corpus: {} is absent — skipping the DOCX corpus gate", dir.display());
        return;
    }
    let reports = match gate(&dir) {
        Ok(reports) => reports,
        Err(e) => panic!("{e}"),
    };
    if reports.is_empty() {
        eprintln!("corpus: {} holds no .docx/.docm files — skipping the DOCX corpus gate", dir.display());
        return;
    }
    let manifest = read_manifest(&dir).expect("the manifest the gate accepted");
    let mut producers: BTreeMap<&str, usize> = BTreeMap::new();
    for (producer, _, _) in manifest.values() {
        *producers.entry(producer.as_str()).or_default() += 1;
    }
    let tally: Vec<String> = producers.iter().map(|(producer, count)| format!("{producer} {count}")).collect();
    eprintln!("corpus: {} file(s) round-trip; producers: {}", reports.len(), tally.join(", "));
    for (path, report) in &reports {
        eprintln!("  ok  {}: {report}", rel(&dir, path));
    }
}

/// A directory of its own, removed when the test ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "wordcraft-docx-corpus-{tag}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch");
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A small document WordCraft wrote itself: the gate must accept its own output.
fn written_document() -> Vec<u8> {
    let mut para = Paragraph::new();
    para.insert_text(0, "Corpus gate", &CharProps::default()).expect("insert text");
    let mut doc = Document::new();
    doc.body = vec![para_block(para)];
    wordcraft_docx::write(&doc).expect("write")
}

const ROW: &str = "one.docx\tWordCraft (this test)\tsynthetic\twritten by the test\n";

#[test]
fn a_file_without_provenance_is_refused() {
    let dir = Scratch::new("no-manifest");
    std::fs::write(dir.path().join("one.docx"), written_document()).expect("write file");
    let e = gate(dir.path()).expect_err("a file without a manifest row must be refused");
    assert!(e.contains("manifest.tsv"), "{e}");
}

#[test]
fn a_manifest_row_lets_the_gate_run() {
    let dir = Scratch::new("manifest");
    std::fs::write(dir.path().join("one.docx"), written_document()).expect("write file");
    std::fs::write(manifest_path(dir.path()), format!("# file\tproducer\tlicence\tsource\n{ROW}")).expect("write manifest");
    let reports = gate(dir.path()).expect("its own output round-trips");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].1.blocks, 1);
    assert!(reports[0].1.chars >= "Corpus gate".len(), "{}", reports[0].1);
}

#[test]
fn a_damaged_file_is_reported_not_crashed() {
    let dir = Scratch::new("damaged");
    let bytes = written_document();
    std::fs::write(dir.path().join("one.docx"), &bytes[..4.min(bytes.len())]).expect("write file");
    std::fs::write(manifest_path(dir.path()), ROW).expect("write manifest");
    let e = gate(dir.path()).expect_err("a damaged package must fail the gate");
    assert!(e.contains("one.docx") && e.contains("1 failed"), "{e}");
}

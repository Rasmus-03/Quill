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

use serde_json::Value;
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

/// A stable name for each picture. A save may renumber the media keys (`image1.emf` comes back as
/// `image1_2.emf`), but the bytes are the picture — and the bytes are compared on their own.
fn media_ranks(doc: &Document) -> std::collections::HashMap<String, String> {
    let mut unique: Vec<&Vec<u8>> = doc.media.values().map(|v| v.as_ref()).collect();
    unique.sort();
    unique.dedup();
    doc.media
        .iter()
        .map(|(name, bytes)| {
            let rank = unique.iter().position(|u| *u == bytes.as_ref()).unwrap_or(usize::MAX);
            (name.clone(), format!("media#{rank}"))
        })
        .collect()
}

/// The names a fingerprint uses instead of the numbers a save invents: parts by content, pictures
/// by bytes.
struct Names {
    parts: std::collections::HashMap<u32, String>,
    media: std::collections::HashMap<String, String>,
}

impl Names {
    fn of(doc: &Document) -> Names {
        let media = media_ranks(doc);
        let sig = |p: &wordcraft_doc::Part| {
            let mut v = serde_json::to_value(&p.blocks).unwrap_or(Value::Null);
            // Normalised first, so two parts that differ only by a rounding error or a renumbered
            // picture get the same signature in both documents.
            normalize(&mut v, &std::collections::HashMap::new(), &media, "part");
            serde_json::to_string(&v).unwrap_or_default()
        };
        // Parts with the same content are the same story to a reader: they share a name. Naming
        // them apart would turn a reordering among identical parts into a false difference.
        let mut distinct: Vec<String> = doc.parts.values().map(sig).collect();
        distinct.sort();
        distinct.dedup();
        let mut names = std::collections::HashMap::new();
        for (i, s) in distinct.into_iter().enumerate() {
            names.insert(s, format!("part#{i}"));
        }
        let parts = doc.parts.iter().map(|(id, p)| (*id, names.get(&sig(p)).cloned().unwrap_or_default())).collect();
        Names { parts, media }
    }
}

/// A relationship id (`rId4`) as a save may renumber it: the same picture under another name. What
/// it points at is in the same string and is still compared.
fn relax_rels(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find("rId") {
        let (before, after) = rest.split_at(at + 3);
        out.push_str(before);
        let digits = after.len() - after.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        out.push_str("rId#");
        rest = &after[digits..];
    }
    out.push_str(rest);
    out
}

/// A document as comparable JSON: floats rounded to a hundredth of a point (a save writes twips, so
/// `273.59998` comes back as `273.6`), `-0` folded into `0`, relationship ids, picture names and
/// part numbers replaced by what they stand for. Two documents that match here carry the same
/// formatting, the same fields and their codes, the same sections, the same numbering.
fn normalize(v: &mut Value, parts: &std::collections::HashMap<u32, String>, media: &std::collections::HashMap<String, String>, under: &str) {
    match v {
        Value::Number(n) => {
            if let Some(f) = n.as_f64()
                && n.is_f64()
            {
                let r = (f * 100.0).round() / 100.0;
                *v = Value::from(if r == 0.0 { 0.0 } else { r });
            }
        }
        Value::String(s) => {
            if let Some(name) = media.get(s.as_str()) {
                *s = name.clone();
            } else if s.contains("rId") {
                *s = relax_rels(s);
            }
        }
        Value::Array(a) => {
            for x in a.iter_mut() {
                normalize(x, parts, media, under);
            }
            // Zero-width markers (a bookmark, the end of a content control) can be written in
            // another order at the same place; their order says nothing. An object with a size
            // keeps its place.
            let marker = |v: &Value| {
                v.as_object().is_some_and(|o| {
                    let name = o
                        .get("type")
                        .and_then(Value::as_str)
                        .or_else(|| o.keys().next().map(String::as_str))
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    name.contains("bookmark") || name.contains("control") || name.contains("field")
                })
            };
            if a.iter().all(marker) {
                a.sort_by_key(|v| serde_json::to_string(v).unwrap_or_default());
            }
        }
        Value::Object(o) => {
            if under == "parts" {
                // The part numbers are the save's ordering (see `Names`).
                let mut renamed: Vec<(String, Value)> = o
                    .iter()
                    .map(|(k, v)| (k.parse::<u32>().ok().and_then(|id| parts.get(&id).cloned()).unwrap_or_else(|| k.clone()), v.clone()))
                    .collect();
                renamed.sort_by(|a, b| a.0.cmp(&b.0));
                o.clear();
                for (k, mut v) in renamed {
                    normalize(&mut v, parts, media, &k);
                    o.insert(k, v);
                }
                return;
            }
            // A stroke width without a stroke is a number with no meaning: the writer sets it to
            // zero and the shape looks the same either way.
            if o.get("stroke").is_some_and(Value::is_null) {
                o.insert("stroke_width".into(), Value::from(0.0));
            }
            for (k, v) in o.iter_mut() {
                // A number that names a part, under the keys that hold one.
                let names_part =
                    matches!(k.as_str(), "default" | "first" | "even" | "box_part" | "part") || (k == "id" && under.eq_ignore_ascii_case("noteref"));
                if names_part
                    && let Some(id) = v.as_u64()
                    && let Some(name) = parts.get(&(id as u32))
                {
                    *v = Value::from(name.clone());
                    continue;
                }
                normalize(v, parts, media, k);
            }
        }
        _ => {}
    }
}

/// A whole document as the round trip compares it.
fn fingerprint(doc: &Document) -> Value {
    let n = Names::of(doc);
    let mut v = serde_json::to_value(doc).unwrap_or(Value::Null);
    normalize(&mut v, &n.parts, &n.media, "");
    v
}

/// The body: the text, the runs' formatting, the paragraphs' properties, the objects, the tables.
fn body_fingerprint(doc: &Document) -> Value {
    let n = Names::of(doc);
    let mut v = serde_json::to_value(&doc.body).unwrap_or(Value::Null);
    normalize(&mut v, &n.parts, &n.media, "body");
    v
}

/// The parts (headers, footers, notes, comments), sorted: which parts there are is what counts, not
/// the numbers they were given.
fn parts_fingerprint(doc: &Document) -> Value {
    let n = Names::of(doc);
    let mut out: Vec<Value> = doc
        .parts
        .values()
        .map(|p| {
            let mut v = serde_json::to_value(&p.blocks).unwrap_or(Value::Null);
            normalize(&mut v, &n.parts, &n.media, "part");
            v
        })
        .collect();
    out.sort_by_key(|v| serde_json::to_string(v).unwrap_or_default());
    Value::from(out)
}

/// Settings, styles, numbering, comments, tracked changes, sources, custom properties: everything
/// that is not a paragraph.
fn rest_fingerprint(doc: &Document) -> Value {
    let n = Names::of(doc);
    let mut v = serde_json::to_value(doc).unwrap_or(Value::Null);
    if let Some(o) = v.as_object_mut() {
        o.remove("body");
        o.remove("parts");
    }
    normalize(&mut v, &n.parts, &n.media, "rest");
    v
}

/// The first place two fingerprints part company, as a JSON path: "body.3.Para.runs.0.props.bold".
fn where_json(a: &Value, b: &Value) -> String {
    fn walk(a: &Value, b: &Value, path: &mut Vec<String>) -> Option<(String, String, String)> {
        match (a, b) {
            (Value::Object(x), Value::Object(y)) => {
                for (k, xv) in x {
                    path.push(k.clone());
                    match y.get(k) {
                        None => return Some((path.join("."), serde_json::to_string(xv).unwrap_or_default(), "(missing)".into())),
                        Some(yv) => {
                            if let Some(found) = walk(xv, yv, path) {
                                return Some(found);
                            }
                        }
                    }
                    path.pop();
                }
                None
            }
            (Value::Array(x), Value::Array(y)) => {
                for (i, (xv, yv)) in x.iter().zip(y.iter()).enumerate() {
                    path.push(i.to_string());
                    if let Some(found) = walk(xv, yv, path) {
                        return Some(found);
                    }
                    path.pop();
                }
                if x.len() != y.len() {
                    return Some((path.join("."), format!("{} items", x.len()), format!("{} items", y.len())));
                }
                None
            }
            _ => {
                if a == b {
                    None
                } else {
                    let short = |v: &Value| {
                        let s = serde_json::to_string(v).unwrap_or_default();
                        s.chars().take(160).collect::<String>()
                    };
                    Some((path.join("."), short(a), short(b)))
                }
            }
        }
    }
    let mut path = Vec::new();
    match walk(a, b, &mut path) {
        Some((p, x, y)) => format!("{p}: {x} -> {y}"),
        None => "(no difference found; the arrays are in another order)".into(),
    }
}

/// What a round trip lost, as a list of human sentences.
#[derive(Default, Debug)]
struct Losses {
    text: bool,
    tables: bool,
    parts: bool,
    media: bool,
    /// A paragraph's formatting, a run's emphasis, a cell's borders.
    formatting: bool,
    /// The settings, the numbering definitions, the comments' metadata, the tracked changes, a
    /// section's page setup, the styles, the sources, the custom properties.
    structure: bool,
    unstable: bool,
    open: Option<String>,
    write: Option<String>,
}

impl Losses {
    fn any(&self) -> bool {
        self.text
            || self.tables
            || self.parts
            || self.media
            || self.formatting
            || self.structure
            || self.unstable
            || self.open.is_some()
            || self.write.is_some()
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
        if self.formatting {
            out.push("the formatting of a paragraph, a run or a table changed".to_string());
        }
        if self.structure {
            out.push("the page setup, the numbering, the styles, a comment, a tracked change or a document property changed".to_string());
        }
        if self.unstable {
            out.push("writing it again gives a different document".to_string());
        }
        format!("{name}: {}", out.join("; "))
    }
}

/// Read a file, write it back, read it again, and report what was lost.
fn round_trip(name: &str, bytes: &[u8]) -> (Losses, Option<Document>) {
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
    // The words were compared above; this is the document around them — the runs' formatting, the
    // paragraphs' properties, the fields and their codes, the objects, the tables' cells, the
    // sections' page setup, the numbering, the styles, the comments, the tracked changes.
    if body_fingerprint(&first) != body_fingerprint(&second) {
        losses.formatting = true;
        if std::env::var("QUILL_CORPUS_DIAG").is_ok() {
            eprintln!("  {name} body differs: {}", where_json(&body_fingerprint(&first), &body_fingerprint(&second)));
        }
    }
    if parts_fingerprint(&first) != parts_fingerprint(&second) {
        losses.formatting = true;
        if std::env::var("QUILL_CORPUS_DIAG").is_ok() {
            eprintln!("  {name} a part differs: {}", where_json(&parts_fingerprint(&first), &parts_fingerprint(&second)));
        }
    }
    if rest_fingerprint(&first) != rest_fingerprint(&second) {
        losses.structure = true;
        if std::env::var("QUILL_CORPUS_DIAG").is_ok() {
            eprintln!("  {name} the rest differs: {}", where_json(&rest_fingerprint(&first), &rest_fingerprint(&second)));
        }
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
        && (body_text(&fourth) != body_text(&second) || table_shape(&fourth) != table_shape(&second) || fingerprint(&fourth) != fingerprint(&second))
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
    // The manifest is the list. A corpus that has lost a file — because a fetch refused it after an
    // upstream change, or because someone deleted it — would quietly check less than it says, so
    // the files have to be all there, or none of them (the test above returns when the directory is
    // empty).
    let present: Vec<&str> = files.iter().filter_map(|p| p.file_name().and_then(|n| n.to_str())).collect();
    let missing: Vec<&str> = entries.iter().map(|e| e.name.as_str()).filter(|n| !present.contains(n)).collect();
    assert!(
        missing.is_empty(),
        "the corpus is missing {} file(s) the manifest lists (fetch it with `cargo xtask corpus --docx`): {missing:?}",
        missing.len()
    );

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
        let (losses, doc) = round_trip(&name, &bytes);
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

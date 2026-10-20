//! Obsidian-flavoured Markdown: YAML frontmatter, `[[wikilinks]]`, `![[embeds]]` and source
//! fallback for syntax the flow model cannot represent.
//!
//! The ordinary Markdown importer intentionally handles a CommonMark/GFM subset. Obsidian notes
//! use more: callouts, comments, highlights, math, block references, task boxes and fenced blocks
//! with a language. Those blocks are kept as [`Kind::Raw`](crate::model::Kind::Raw) source and
//! written back byte-for-byte rather than escaped into different Markdown. Wikilinks and embeds
//! are real inline formatting, stored in the document's hyperlink field, so they survive save and
//! reopen in `.docx` too.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use wordcraft_doc::Document;

use crate::model::{self, FBlock, Inline};

/// Passthrough part holding the complete YAML frontmatter block, including both `---` lines.
pub const FRONTMATTER_KEY: &str = "wordcraft:obsidian.frontmatter";
/// Passthrough part holding the exact Markdown source, used when nothing was edited.
pub const SOURCE_KEY: &str = "wordcraft:obsidian.source";

/// One file written beside the Markdown note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    /// Path relative to the note, using `/`.
    pub path: String,
    pub bytes: Vec<u8>,
}

/// Markdown plus the image files it refers to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bundle {
    pub markdown: String,
    pub assets: Vec<Asset>,
}

/// Import an Obsidian note. Frontmatter is kept out of the document text and restored on export;
/// the original source is kept for a lossless save when the document was not edited.
pub fn import(text: &str) -> Document {
    let (front, body) = split_frontmatter(text);
    let mut doc = crate::markdown::import(body);
    if let Some(front) = front {
        doc.passthrough.insert(FRONTMATTER_KEY.into(), Arc::new(front.as_bytes().to_vec()));
    }
    if !text.is_empty() {
        doc.passthrough.insert(SOURCE_KEY.into(), Arc::new(text.as_bytes().to_vec()));
    }
    doc
}

/// Import Markdown from a file, keeping the file's own bytes for a lossless save when nothing was
/// edited. The text is decoded for parsing; a BOM or another encoding survives in the source.
pub fn import_bytes(bytes: &[u8]) -> Document {
    let mut doc = import(&crate::txt::decode(bytes));
    if !bytes.is_empty() {
        doc.passthrough.insert(SOURCE_KEY.into(), Arc::new(bytes.to_vec()));
    }
    doc
}

/// The exact source this note was opened from, if it was Obsidian Markdown.
pub fn original_source(doc: &Document) -> Option<&[u8]> {
    doc.passthrough.get(SOURCE_KEY).map(|b| b.as_slice())
}

/// Markdown with frontmatter, naming images in the note's own `<ext>` form.
pub fn export(doc: &Document) -> String {
    with_frontmatter(doc, &crate::markdown::export(doc))
}

/// Markdown with frontmatter plus every image as a file under `attachment_dir`.
///
/// Image names are derived from their media keys and kept unique, so two different pictures never
/// overwrite each other and the same picture keeps one name when it is used twice.
pub fn export_bundle(doc: &Document, attachment_dir: &str) -> Bundle {
    let flow = model::from_doc(doc);
    let dir = clean_dir(attachment_dir);
    let mut names = BTreeMap::new();
    let mut used = BTreeSet::new();
    let mut assets = Vec::new();
    walk_blocks(&flow.blocks, &dir, &mut names, &mut used, &mut assets);
    let body = crate::markdown::export_with_names(doc, &names);
    Bundle { markdown: with_frontmatter(doc, &body), assets }
}

fn with_frontmatter(doc: &Document, body: &str) -> String {
    let mut out = String::new();
    if let Some(front) = frontmatter(doc) {
        out.push_str(front);
        if !out.ends_with('\n') {
            out.push('\n');
        }
    }
    out.push_str(body);
    out
}

fn frontmatter(doc: &Document) -> Option<&str> {
    let bytes = doc.passthrough.get(FRONTMATTER_KEY)?;
    std::str::from_utf8(bytes.as_slice()).ok().filter(|s| !s.is_empty())
}

/// Split a leading YAML frontmatter block from the body. The returned block includes both `---`
/// lines (or the closing `...`) and its final newline.
fn split_frontmatter(text: &str) -> (Option<&str>, &str) {
    let t = text.strip_prefix('\u{feff}').unwrap_or(text);
    let rest = if let Some(r) = t.strip_prefix("---\n") {
        r
    } else if let Some(r) = t.strip_prefix("---\r\n") {
        r
    } else {
        return (None, text);
    };
    let open = t.len() - rest.len();
    let mut pos = 0;
    while pos <= rest.len() {
        let nl = rest.get(pos..).and_then(|r| r.find('\n')).map_or(rest.len(), |n| pos + n + 1);
        let line = rest.get(pos..nl).unwrap_or("").trim_end_matches(['\r', '\n']);
        if line == "---" || line == "..." {
            // Keep the blank separator after the block as part of it, so an untouched note keeps
            // the exact same bytes around the frontmatter.
            let mut end = nl;
            while end < rest.len() {
                let next = rest.get(end..).and_then(|r| r.find('\n')).map_or(rest.len(), |n| end + n + 1);
                if rest.get(end..next).is_some_and(|l| l.trim().is_empty()) {
                    end = next;
                } else {
                    break;
                }
            }
            let body = rest.get(end..).unwrap_or("");
            let front = t.get(..open + end).unwrap_or("");
            return (Some(front), body);
        }
        if nl >= rest.len() {
            break;
        }
        pos = nl;
    }
    (None, text)
}

fn clean_dir(dir: &str) -> String {
    let mut parts = Vec::new();
    for part in dir.split(['/', '\\']) {
        let part: String = part.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-')).collect();
        let part = part.trim_matches(['_', '-']);
        if part.is_empty() || part == "." || part == ".." {
            continue;
        }
        parts.push(part.to_string());
        if parts.len() == 4 {
            break;
        }
    }
    parts.join("/")
}

fn walk_blocks(blocks: &[FBlock], dir: &str, names: &mut BTreeMap<String, String>, used: &mut BTreeSet<String>, assets: &mut Vec<Asset>) {
    for block in blocks {
        match block {
            FBlock::Para(p) => {
                for inline in &p.inlines {
                    if let Inline::Image(img) = inline {
                        add_asset(img, dir, names, used, assets);
                    }
                }
            }
            FBlock::Table(t) => {
                for row in &t.rows {
                    for cell in row {
                        walk_blocks(&cell.blocks, dir, names, used, assets);
                    }
                }
            }
        }
    }
}

fn add_asset(img: &model::Img, dir: &str, names: &mut BTreeMap<String, String>, used: &mut BTreeSet<String>, assets: &mut Vec<Asset>) {
    let Some(key) = img.key.as_ref() else { return };
    if names.contains_key(key) {
        return;
    }
    let ext: String = img.ext.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let ext = if ext.is_empty() { "bin".to_string() } else { ext.to_ascii_lowercase() };
    // A picture decoded from a data URI has no file name of its own: number it like `image1.png`.
    // Pictures from a document keep the name their media key carries and never overwrite each
    // other (the same bytes used twice get one file, two different files get `name-2`).
    let data_uri = key.starts_with("data:");
    let raw_stem = key.rsplit('/').next().unwrap_or(key);
    let raw_stem = Path::new(raw_stem).file_stem().and_then(|s| s.to_str()).unwrap_or(raw_stem);
    let stem: String = raw_stem.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')).take(80).collect();
    let stem = stem.trim_matches('.').trim();
    let stem = if data_uri || stem.is_empty() { "image" } else { stem };
    let mut name = if data_uri { format!("image1.{ext}") } else { format!("{stem}.{ext}") };
    let mut n = 2;
    while used.contains(&name) {
        name = if data_uri { format!("image{n}.{ext}") } else { format!("{stem}-{n}.{ext}") };
        n += 1;
    }
    used.insert(name.clone());
    let path = if dir.is_empty() { name } else { format!("{dir}/{name}") };
    names.insert(key.clone(), path.clone());
    assets.push(Asset { path, bytes: img.data.as_ref().clone() });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FBlock, Flow, Kind, Para};

    #[test]
    fn frontmatter_is_kept_out_of_the_text_and_restored() {
        let src = "---\ntitle: A note\ntags:\n  - one\n---\n\n# Heading\n\nBody\n";
        let doc = import(src);
        assert!(!doc.plain_text(wordcraft_doc::StoryRef::Body).contains("title: A note"));
        assert_eq!(export(&doc), src);
        assert_eq!(original_source(&doc), Some(src.as_bytes()));
    }

    #[test]
    fn wikilinks_and_embeds_round_trip() {
        let src = "A [[Note|label]] and [[Other#Part|alias]] and ![[picture.png|300]].\n";
        let doc = import(src);
        assert_eq!(export(&doc), src);
        let text = doc.plain_text(wordcraft_doc::StoryRef::Body);
        assert!(text.contains("label"));
        assert!(text.contains("![[picture.png|300]]"));
    }

    #[test]
    fn unsupported_blocks_keep_their_source() {
        let src = "> [!note]\n> A callout\n\nInline $x + y$ math.\n\n```dataview\nLIST\n```\n\n#tag\n";
        let doc = import(src);
        let out = export(&doc);
        assert!(out.contains("> [!note]\n> A callout"), "{out}");
        assert!(out.contains("Inline $x + y$ math."), "{out}");
        assert!(out.contains("```dataview\nLIST\n```"), "{out}");
        assert!(out.contains("#tag"), "{out}");
    }

    #[test]
    fn bundle_writes_images_once() {
        let png =
            [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0, 0, 0, 0, 0, 0, 0];
        let img = model::make_img(png.to_vec(), None, None, "dot").expect("png header");
        let mut p = Para::default();
        p.inlines.push(Inline::Image(img.clone()));
        p.inlines.push(Inline::Image(img));
        let flow = Flow { blocks: vec![FBlock::Para(p)], ..Default::default() };
        let doc = model::to_doc(&flow);
        let bundle = export_bundle(&doc, "/assets/");
        assert_eq!(bundle.assets.len(), 1);
        assert_eq!(bundle.assets[0].path, "assets/image1.png");
        assert!(bundle.markdown.contains("![dot](assets/image1.png)"), "{}", bundle.markdown);
        assert!(!bundle.markdown.contains("](assets/image1-2.png)"));
    }

    #[test]
    fn raw_style_is_not_a_heading_or_code_block() {
        let mut p = Para::new(Kind::Raw);
        p.push_text("= not a setext heading", &model::Fmt::default());
        let flow = Flow { blocks: vec![FBlock::Para(p)], ..Default::default() };
        let doc = model::to_doc(&flow);
        let out = export(&doc);
        assert_eq!(out, "= not a setext heading\n");
    }
}

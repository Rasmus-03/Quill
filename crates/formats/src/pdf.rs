//! PDF Reflow: open a PDF as an editable document (Word for Windows' "Open a PDF as a Word
//! document"), for the common case — a document a word processor wrote and exported as usual:
//! text drawn in fonts with ToUnicode cmaps, pictures drawn whole.
//!
//! The text is read with `pdf-extract` (MIT), which decodes every glyph through the fonts'
//! embedded cmaps and reports the glyph's pen position, advance and size. The layout is rebuilt
//! from that: characters group into words (the gap the PDF draws between them), words into lines
//! (characters on one baseline), lines into paragraphs (vertical spacing and measure), and those
//! into headings (a size step up) and list items (a bullet or a number, when several in a row).
//! Pictures are located the same way: the page's content stream is walked with its graphics
//! state (`q/Q/cm/Do`) for image XObjects, decoded — JPEG kept as JPEG, the rest re-encoded —
//! and placed in the flow where they sit on the page.
//!
//! v1 keeps what a PDF states about its text layout: the page size (the MediaBox), the text,
//! its sizes and the pictures. Run colour, bold and italic faces (the font name, which
//! [`pdf_extract::OutputDev::output_character`] does not report), tables drawn from rules and
//! text without a ToUnicode map come back as plain text or are left out. Nothing here panics on
//! hostile input: every number from the file is clamped, pictures are capped, and parts that
//! cannot be read are skipped with a warning rather than failing the import.
#![cfg(not(target_arch = "wasm32"))]

use std::collections::HashMap;
use std::sync::Arc;

use wordcraft_doc::{Align, Document};

use crate::model::{FBlock, Flow, Img, Inline, Kind, ListInfo, Para};

/// Largest PDF read (checked before parsing).
const MAX_BYTES: usize = 256 << 20;
/// Most pages imported (a hostile PDF may claim any page count; a click-through file is bigger
/// than any book).
const MAX_PAGES: usize = 5_000;
/// Most glyphs collected per page (at 12 pt a dense page is ~15 000).
const MAX_GLYPHS_PER_PAGE: usize = 200_000;
/// Most pixels decoded across one document's pictures.
const MAX_PIXELS: u64 = 64_000_000;
/// Largest stream decoded for a content walk.
const MAX_STREAM: usize = 64 << 20;
/// Most content operations kept from one stream (Word writes a few thousand).
const MAX_OPS: usize = 100_000;
/// Deepest Form XObject recursion (a picture nested in drawings nested in drawings).
const MAX_FORM_DEPTH: u8 = 5;
/// Ancestor Pages nodes followed for an inherited MediaBox or Resources.
const MAX_INHERIT: usize = 16;
/// A form's or page's XObject dictionary walked at most this far.
const MAX_XOBJECT_ALLOCATIONS: usize = 4_096;
/// Largest drawn picture side, points.
const MAX_PICTURE_SIDE: f32 = 1_584.0;
/// Largest pixel side a picture's header may claim (hostile headers).
const MAX_PICTURE_PIXELS: u32 = 30_000;
/// Largest paragraph text kept, characters (a hostile PDF).
const MAX_PARA_TEXT: usize = 40_000;

/// Fine pitch considered "the same paragraph" (×the line's size; Word's line spacing is 1.0–1.5,
/// its visible paragraph spacing starts around 1.4).
const PITCH_SAME: f64 = 1.45;
/// A size ratio that makes a paragraph a heading (×the document's body size): H1, H2, H3.
const HEAD_STEPS: [f64; 3] = [1.65, 1.35, 1.2];
/// Bullet characters a list item may start with (the dash a salutation or a range has stays).
const BULLETS: &str = "•◦■▪·●‣";
/// Indent that takes a list item one level deeper, points.
#[allow(dead_code)]
const LIST_INDENT: f64 = 16.0;
/// A paragraph centred when its middle sits this close to the page's centre, points.
const CENTER_TOLERANCE: f64 = 8.0;

/// Open a PDF as an editable document: its text, sizes, pictures and page size.
pub fn import(bytes: &[u8]) -> Result<Document, String> {
    import_meta(bytes).map(|(doc, _)| doc)
}

/// [`import`], which also says whether the PDF carried a document title (whether the flow's
/// Title paragraph came from the file rather than being left to the user).
pub fn import_meta(bytes: &[u8]) -> Result<(Document, bool), String> {
    if bytes.len() > MAX_BYTES {
        return Err(format!("the PDF is larger than {} MB", MAX_BYTES >> 20));
    }
    if !bytes.starts_with(b"%PDF-") {
        return Err("not a PDF file".to_string());
    }
    let mut pdf = pdf_extract::Document::load_mem(bytes).map_err(|e| format!("not a PDF WordCraft can open ({e})"))?;
    if pdf.is_encrypted() {
        pdf.decrypt("").map_err(|_| "the PDF is password-protected".to_string())?;
    }
    let pages = pdf.get_pages();
    if pages.len() > MAX_PAGES {
        return Err(format!("the PDF has {} pages; at most {MAX_PAGES} are imported", pages.len()));
    }
    let mut collector = TextCollector::default();
    pdf_extract::output_doc(&pdf, &mut collector).map_err(|e| format!("the PDF's text could not be read ({e})"))?;

    // Pictures, page for page in the same order `output_doc` walked the pages.
    let mut budget = MAX_PIXELS;
    let mut pictures: Vec<Vec<Picture>> = Vec::with_capacity(collector.pages.len());
    for id in pages.values() {
        pictures.push(collect_images(&pdf, *id, &mut budget));
    }

    let (raw, page_geo) = assemble(&collector.pages, &pictures);
    let (flow, geo) = style_and_flow(raw, &page_geo, &pdf);
    let titled = !flow.meta.title.trim().is_empty();
    let mut doc = crate::model::to_doc_in(&flow, section_of(&geo));
    doc.ensure_nonempty();
    Ok((doc, titled))
}

// ---------------------------------------------------------------------------------------------
// Text: the glyphs the PDF draws, page for page.

/// One glyph's pen position and size, page points, y growing downward (the flipped view
/// [`pdf_extract::PlainTextOutput`] builds; the MediaBox offset rides along in both).
struct Glyph {
    x: f64,
    y: f64,
    /// Transformed size (points), and where the pen ends after this glyph.
    size: f64,
    end_x: f64,
    /// The glyph's Unicode text (a character from a cmap, a ligature's several).
    text: String,
}

/// One page's glyphs and its MediaBox sides, y growing downward.
struct PageText {
    glyphs: Vec<Glyph>,
    w: f64,
    /// Page height in view order (the flip; also the y of the page's top).
    h: f64,
}

/// A page height from a MediaBox, for the flip that turns PDF's y-up page into view order.
fn media_h(m: &pdf_extract::MediaBox) -> f64 {
    (m.ury - m.lly).abs().clamp(1.0, 400_000.0)
}

/// Input-derived numbers are hostile: a position off the page (or NaN) means the glyph is
/// skipped where it is used as a coordinate.
fn sane_pos(v: f64) -> bool {
    v.is_finite() && v.abs() < 200_000.0
}

/// Controls dropped (tabs cleaned by the caller), bounded (a glyph's own text: a few characters).
fn sanitize(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(64).collect()
}

/// Collects the glyphs `pdf_extract` decodes, page for page.
#[derive(Default)]
struct TextCollector {
    pages: Vec<PageText>,
    flip: f64,
}

impl pdf_extract::OutputDev for TextCollector {
    fn begin_page(
        &mut self,
        _page_num: u32,
        media_box: &pdf_extract::MediaBox,
        _art_box: Option<(f64, f64, f64, f64)>,
    ) -> Result<(), pdf_extract::OutputError> {
        self.flip = media_h(media_box);
        self.pages.push(PageText { glyphs: Vec::new(), w: (media_box.urx - media_box.llx).abs().clamp(1.0, 400_000.0), h: self.flip });
        Ok(())
    }

    fn end_page(&mut self) -> Result<(), pdf_extract::OutputError> {
        Ok(())
    }

    fn begin_word(&mut self) -> Result<(), pdf_extract::OutputError> {
        Ok(())
    }

    fn end_word(&mut self) -> Result<(), pdf_extract::OutputError> {
        Ok(())
    }

    fn end_line(&mut self) -> Result<(), pdf_extract::OutputError> {
        Ok(())
    }

    fn output_character(
        &mut self,
        trm: &pdf_extract::Transform,
        width: f64,
        _spacing: f64,
        font_size: f64,
        text: &str,
    ) -> Result<(), pdf_extract::OutputError> {
        // The same geometry `pdf_extract::PlainTextOutput` does, from the matrix fields directly:
        // euclid keeps the PDF operands as given and applies them column-wise, so the matrix's
        // translation field is the pen position and the flip is `page_h - y`.
        let (x, y) = (trm.m31, self.flip - trm.m32);
        let text = sanitize(text);
        let usable = sane_pos(x) && sane_pos(y) && !text.is_empty();
        if let Some(page) = self.pages.last_mut()
            && usable
            && page.glyphs.len() < MAX_GLYPHS_PER_PAGE
        {
            // Transformed size: the length of the matrix applied to a (size, size) vector.
            let (tx, ty) = ((trm.m11 + trm.m21) * font_size, (trm.m12 + trm.m22) * font_size);
            let size = (tx * tx + ty * ty).sqrt().clamp(0.05, 3_000.0);
            let end_x = x + width.clamp(-1.0e4, 1.0e4) * size;
            page.glyphs.push(Glyph { x, y, size, end_x, text });
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// Lines.

/// A line of the page: its baseline, the top of its box, its type size and what it says.
struct Line {
    y: f64,
    top: f64,
    x0: f64,
    x1: f64,
    size: f64,
    text: String,
}

/// Build a page's lines from its glyphs: glyphs on one baseline group (whatever order a
/// klutzy run draws them in), then run sorted by x, and the drawn gap between two glyphs that
/// was meant to be a space becomes one.
fn lines_of(page: &PageText) -> Vec<Line> {
    let mut glyphs: Vec<(usize, &Glyph)> = page.glyphs.iter().enumerate().collect();
    if glyphs.is_empty() {
        return Vec::new();
    }
    glyphs.sort_by(|a, b| a.1.y.total_cmp(&b.1.y).then(a.1.x.total_cmp(&b.1.x)));
    // Baseline groups: two glyphs on the same line when their baselines sit within a third of
    // the type size (superscripts and line-drawn equations fall into the next line).
    let mut groups: Vec<Vec<(usize, &Glyph)>> = Vec::new();
    for (idx, g) in glyphs {
        let joins = match groups.last().and_then(|line| line.first()) {
            Some((_, first)) => {
                let size = first.size.max(g.size).max(0.5);
                (g.y - first.y).abs() <= size * 0.35
            }
            None => false,
        };
        if joins {
            if let Some(line) = groups.last_mut() {
                line.push((idx, g));
            }
        } else {
            groups.push(vec![(idx, g)]);
        }
    }
    let mut out = Vec::new();
    for group in groups {
        let mut runs: Vec<(usize, &Glyph)> = group;
        // When a font's advance widths are missing, every glyph arrives near the same pen
        // position and sorting by x reverses the characters. A real line spans multiples of the
        // size, so sort by x only when the extent says the x values are positions; otherwise
        // re-store the drawing order (a right-to-left run keeps its own order).
        let (lo, hi, biggest) =
            runs.iter().fold((f64::INFINITY, f64::NEG_INFINITY, 0.0_f64), |(lo, hi, s), (_, g)| (lo.min(g.x), hi.max(g.x), s.max(g.size)));
        if hi - lo > biggest.max(0.5) * 0.5 {
            runs.sort_by(|a, b| a.1.x.total_cmp(&b.1.x));
        } else {
            // The glyphs arrived x-sorted; the drawing index restores what the PDF drew first.
            runs.sort_by_key(|(idx, _)| *idx);
        }
        let mut text = String::new();
        let mut last_end: Option<f64> = None;
        let mut size = 0.0_f64;
        for (_, g) in runs.iter() {
            // A gap wider than ~an eighth of the size is the space the PDF drew between words;
            // the kerning inside a word is thinner.
            if let Some(end) = last_end
                && g.x > end + g.size.max(1.0) * 0.14
                && !text.ends_with(' ')
            {
                text.push(' ');
            }
            text.push_str(&g.text);
            last_end = Some(g.end_x.max(g.x));
            size = size.max(g.size);
        }
        let text = text.trim().to_string();
        if text.is_empty() {
            continue;
        }
        let x0 = runs.first().map(|(_, g)| g.x).unwrap_or(0.0);
        let x1 = last_end.unwrap_or(x0);
        // One baseline per group: the first glyph's (they sit within a third of the size of
        // each other).
        let y = runs.first().map(|(_, g)| g.y).unwrap_or(0.0);
        // The line box's top (ascent above the baseline; approximated, enough for the order
        // against pictures and the paragraph pitch).
        let top = y - size * 0.85;
        out.push(Line { y, top, x0, x1, size, text });
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Pictures.

/// A picture in the flow: its bytes and where it was drawn (page points, y downward).
#[derive(Clone)]
struct Picture {
    data: Arc<Vec<u8>>,
    ext: String,
    /// Drawn size, points.
    w: f32,
    h: f32,
    /// Top of the drawn box.
    top: f64,
}

/// A graphics-state matrix, the PDF's six operands (`a b c d e f`).
type Matrix = [f64; 6];

/// Multiply two matrices the PDF way (points are row vectors): `m ⋅ base`.
fn mul(m: &Matrix, base: &Matrix) -> Matrix {
    let [a1, b1, c1, d1, e1, f1] = *m;
    let [a2, b2, c2, d2, e2, f2] = *base;
    [a1 * a2 + b1 * c2, a1 * b2 + b1 * d2, c1 * a2 + d1 * c2, c1 * b2 + d1 * d2, e1 * a2 + f1 * c2 + e2, e1 * b2 + f1 * d2 + f2]
}

/// The matrix of a matrix-taking operator (`cm`). The operands come as six numbers, or as one
/// array of six numbers, depending on how `pdf-extract` collected them.
fn op_matrix_of(op: &pdf_extract::content::Operation) -> Option<Matrix> {
    let operands = &op.operands;
    if operands.len() == 1 {
        return op_matrix(&operands[0]);
    }
    if operands.len() < 6 {
        return None;
    }
    let n = |x: &pdf_extract::Object| -> Option<f64> {
        match x {
            pdf_extract::Object::Integer(i) => sane_num(*i as f64),
            pdf_extract::Object::Real(f) => sane_num(f64::from(*f)),
            _ => None,
        }
    };
    Some([n(&operands[0])?, n(&operands[1])?, n(&operands[2])?, n(&operands[3])?, n(&operands[4])?, n(&operands[5])?])
}

/// Six numbers from a `cm` or `Matrix` operand array, finite and sane.
fn op_matrix(v: &pdf_extract::Object) -> Option<Matrix> {
    let a = v.as_array().ok()?;
    if a.len() < 6 {
        return None;
    }
    let n = |x: &pdf_extract::Object| -> Option<f64> {
        match x {
            pdf_extract::Object::Integer(i) => sane_num(*i as f64),
            pdf_extract::Object::Real(f) => sane_num(f64::from(*f)),
            _ => None,
        }
    };
    let m = [n(&a[0])?, n(&a[1])?, n(&a[2])?, n(&a[3])?, n(&a[4])?, n(&a[5])?];
    m.iter().all(|x| x.is_finite()).then_some(m)
}

/// The page's starting transform: its /Rotate (a rotated page keeps its pictures straight).
fn rotate_matrix(v: Option<&pdf_extract::Object>) -> Matrix {
    let rot = v.and_then(|v| v.as_i64().ok()).unwrap_or(0);
    match rot.rem_euclid(360) {
        90 => [0.0, 1.0, -1.0, 0.0, 0.0, 0.0],
        180 => [-1.0, 0.0, 0.0, -1.0, 0.0, 0.0],
        270 => [0.0, -1.0, 1.0, 0.0, 0.0, 0.0],
        _ => [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    }
}

/// One picture pass over a page: its content ops walked for image XObjects with their CTM.
fn collect_images(pdf: &pdf_extract::Document, page_id: pdf_extract::ObjectId, budget: &mut u64) -> Vec<Picture> {
    let mut out = Vec::new();
    let Ok(page_obj) = pdf.get_object(page_id) else { return out };
    let Ok(page) = page_obj.as_dict() else { return out };
    let Some(media) = media_box(pdf, page) else { return out };
    let flip = media_h(&media);
    let start = rotate_matrix(page.get(b"Rotate").ok());
    let mut seen: HashMap<pdf_extract::ObjectId, ()> = HashMap::new();
    let mut guard = 0usize;
    if let Ok(ops) = page_ops(pdf, page) {
        walk_ops(pdf, resources_of(pdf, page).as_ref(), ops, &start, flip, &mut out, budget, &mut seen, 0, &mut guard);
    }
    out.sort_by(|a, b| a.top.total_cmp(&b.top));
    out
}

/// A page's (or Node's) Resources dictionary: the page's own, else inherited from the ancestor
/// Pages nodes.
#[allow(clippy::option_option)]
fn resources_of(pdf: &pdf_extract::Document, page: &pdf_extract::Dictionary) -> Option<pdf_extract::Dictionary> {
    let mut node = page.clone();
    for _ in 0..MAX_INHERIT {
        if let Ok(res) = node.get(b"Resources")
            && let Some(d) = deref_dict(pdf, res)
        {
            return Some(d.clone());
        }
        match node.get(b"Parent").ok().and_then(|o| deref(pdf, o)) {
            Some(pdf_extract::Object::Reference(id)) => {
                node = pdf.get_object(*id).ok()?.as_dict().ok()?.clone();
            }
            _ => return None,
        }
    }
    None
}

/// Follow indirect references (and never return an owned page-sized `Object`).
fn deref<'a>(pdf: &'a pdf_extract::Document, o: &'a pdf_extract::Object) -> Option<&'a pdf_extract::Object> {
    pdf.dereference(o).ok().map(|(_, resolved)| resolved)
}

/// `o` as a dictionary (references followed).
fn deref_dict<'a>(pdf: &'a pdf_extract::Document, o: &'a pdf_extract::Object) -> Option<&'a pdf_extract::Dictionary> {
    deref(pdf, o).and_then(|v| v.as_dict().ok())
}

/// The page's MediaBox, from the page or inherited from its ancestor Pages nodes.
fn media_box(pdf: &pdf_extract::Document, page: &pdf_extract::Dictionary) -> Option<pdf_extract::MediaBox> {
    let mut node = page;
    for _ in 0..MAX_INHERIT {
        if let Ok(v) = node.get(b"MediaBox")
            && let Some(m) = box_of(v)
        {
            return Some(m);
        }
        match node.get(b"Parent").ok().and_then(|o| deref(pdf, o)) {
            Some(pdf_extract::Object::Reference(id)) => node = pdf.get_object(*id).ok()?.as_dict().ok()?,
            _ => return None,
        }
    }
    None
}

/// A number a matrix or box may hold: finite, bounded (hostile streams claim anything).
fn sane_num(v: f64) -> Option<f64> {
    (v.is_finite() && v.abs() < 1.0e6).then_some(v)
}

/// `llx lly urx ury`, sane: lows below highs, finite, page-sized.
fn box_of(v: &pdf_extract::Object) -> Option<pdf_extract::MediaBox> {
    let a = v.as_array().ok()?;
    if a.len() < 4 {
        return None;
    }
    let n = |x: &pdf_extract::Object| -> Option<f64> {
        match x {
            pdf_extract::Object::Integer(i) => sane_num(*i as f64),
            pdf_extract::Object::Real(f) => sane_num(f64::from(*f)),
            _ => None,
        }
    };
    let (llx, lly, urx, ury) = (n(&a[0])?, n(&a[1])?, n(&a[2])?, n(&a[3])?);
    let (llx, urx) = (llx.min(urx), llx.max(urx));
    let (lly, ury) = (lly.min(ury), lly.max(ury));
    (urx - llx < 4_096.0 && ury - lly < 4_096.0 && urx - llx > 0.0 && ury - lly > 0.0).then_some(pdf_extract::MediaBox { llx, lly, urx, ury })
}

/// A page's content operations, from its `Contents` (one stream or several streamed together).
fn page_ops(pdf: &pdf_extract::Document, page: &pdf_extract::Dictionary) -> Result<Vec<pdf_extract::content::Operation>, ()> {
    let entries: Vec<pdf_extract::Object> = match page.get(b"Contents") {
        Ok(pdf_extract::Object::Array(a)) => a.iter().map(|x| deref(pdf, x).cloned().unwrap_or_else(|| x.clone())).collect(),
        Ok(other) => vec![other.clone()],
        Err(_) => return Err(()),
    };
    let mut ops: Vec<pdf_extract::content::Operation> = Vec::new();
    for entry in entries {
        let Some(obj) = deref(pdf, &entry) else { continue };
        let Ok(stream) = obj.as_stream() else { continue };
        let bytes = stream.decompressed_content().unwrap_or_else(|_| stream.content.clone());
        if bytes.len() > MAX_STREAM {
            log::warn!("PDF: a content stream is too large; the pictures after it are left out");
            return Err(());
        }
        let Ok(c) = pdf_extract::content::Content::decode(&bytes) else { continue };
        ops.extend(c.operations);
        if ops.len() > MAX_OPS {
            log::warn!("PDF: the page draws with more operations than WordCraft keeps");
            return Err(());
        }
    }
    Ok(ops)
}

/// Walk content operations with their graphics state, recording every image `Do`.
#[allow(clippy::too_many_arguments)]
fn walk_ops(
    pdf: &pdf_extract::Document,
    resources: Option<&pdf_extract::Dictionary>,
    ops: Vec<pdf_extract::content::Operation>,
    ctm: &Matrix,
    flip: f64,
    out: &mut Vec<Picture>,
    budget: &mut u64,
    seen: &mut HashMap<pdf_extract::ObjectId, ()>,
    depth: u8,
    guard: &mut usize,
) {
    if depth > MAX_FORM_DEPTH || ops.len() > MAX_OPS {
        return;
    }
    let mut stack: Vec<Matrix> = Vec::new();
    let mut ctm = *ctm;
    for op in ops {
        match op.operator.as_str() {
            "q" => {
                if stack.len() < 64 {
                    stack.push(ctm);
                }
            }
            "Q" => {
                if let Some(up) = stack.pop() {
                    ctm = up;
                }
            }
            "cm" => {
                if let Some(m) = op_matrix_of(&op) {
                    ctm = mul(&m, &ctm);
                }
            }
            "Do" => {
                *guard += 1;
                if *guard > MAX_XOBJECT_ALLOCATIONS {
                    return;
                }
                let Some(name) = op.operands.first().and_then(|v| v.as_name().ok()) else { continue };
                let Some(res) = resources else { continue };
                let Some(xo) = res.get(b"XObject").ok().and_then(|v| deref_dict(pdf, v)) else { continue };
                let Some(target) = xo.get(name).ok() else { continue };
                let id = match target {
                    pdf_extract::Object::Reference(id) => Some(*id),
                    _ => None,
                };
                let Some(obj) = deref(pdf, target) else { continue };
                let Ok(stream) = obj.as_stream() else { continue };
                let dict = &stream.dict;
                let subtype = dict.get(b"Subtype").ok().and_then(|v| v.as_name().ok()).unwrap_or(&[]).to_vec();
                match subtype.as_slice() {
                    b"Image" => {
                        if let Some(id) = id
                            && seen.insert(id, ()).is_some()
                        {
                            continue; // this page already drew it
                        }
                        if let Some(pic) = picture(pdf, dict, stream, &ctm, flip, budget) {
                            out.push(pic);
                        }
                    }
                    b"Form" => {
                        if let Some(id) = id
                            && seen.insert(id, ()).is_some()
                        {
                            continue;
                        }
                        let m = dict.get(b"Matrix").ok().and_then(op_matrix).unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
                        let inner = mul(&m, &ctm);
                        let form_res = dict.get(b"Resources").ok().and_then(|v| deref_dict(pdf, v)).or(resources);
                        let bytes = stream.decompressed_content().unwrap_or_else(|_| stream.content.clone());
                        if bytes.len() > MAX_STREAM {
                            continue;
                        }
                        if let Ok(c) = pdf_extract::content::Content::decode(&bytes) {
                            walk_ops(pdf, form_res, c.operations, &inner, flip, out, budget, seen, depth + 1, guard);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

/// One image XObject: decoded (JPEG kept as JPEG, the rest re-encoded PNG) with the size it
/// was drawn at (`ctm` maps the unit square; the bounds of its corners are the drawn box).
fn picture(
    pdf: &pdf_extract::Document,
    dict: &pdf_extract::Dictionary,
    stream: &pdf_extract::Stream,
    ctm: &Matrix,
    flip: f64,
    budget: &mut u64,
) -> Option<Picture> {
    let (w, h) = dims(dict)?;
    let pixels = u64::from(w) * u64::from(h);
    if pixels > *budget {
        log::warn!("PDF: pictures beyond {} Mpx in one document are left out", MAX_PIXELS >> 20);
        return None;
    }
    *budget -= pixels;
    let filters: Vec<Vec<u8>> = match dict.get(b"Filter") {
        Ok(v) => match v {
            pdf_extract::Object::Array(a) => a.iter().filter_map(|x| x.as_name().ok()).map(|n| n.to_vec()).collect(),
            other => other.as_name().ok().map(|n| vec![n.to_vec()]).unwrap_or_default(),
        },
        Err(_) => Vec::new(),
    };
    // The covered cases: JPEG whole (byte-for-byte, as Word wrote it), the rest raw 8-bit
    // samples after `lopdf` has undone Flate/LZW and the PNG predictors.
    let (data, ext) = if filters.iter().any(|f| f == b"DCTDecode") {
        (stream.content.clone(), "jpeg")
    } else if filters.is_empty() || filters.iter().all(|f| f == b"FlateDecode" || f == b"LZWDecode" || f == b"ASCII85Decode") {
        let samples = stream.decompressed_content().ok()?;
        let png = samples_to_png(pdf, dict, &samples, w, h)?;
        (png, "png")
    } else {
        log::warn!("PDF: a picture is drawn in a filter WordCraft does not read (JPXDecode?) and is left out");
        return None;
    };
    // Where on the page: the unit square's corners through the CTM; the flip turns their
    // y-up values into view order.
    let [a, b, c, d, e, f] = *ctm;
    let xs = [e, e + a, e + a + c, e + c];
    let ys = [f, f + b, f + b + d, f + d];
    let (mut mins, mut maxs) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut mine, mut maxe) = (f64::INFINITY, f64::NEG_INFINITY);
    for &x in &xs {
        mins = mins.min(x);
        maxs = maxs.max(x);
    }
    for &y in &ys {
        mine = mine.min(y);
        maxe = maxe.max(y);
    }
    if !(sane_pos(mins) && sane_pos(maxe)) {
        return None;
    }
    let w = (maxs - mins).clamp(1.0, f64::from(MAX_PICTURE_SIDE)) as f32;
    let h = (maxe - mine).clamp(1.0, f64::from(MAX_PICTURE_SIDE)) as f32;
    let top = (flip - maxe).max(0.0);
    Some(Picture { data: Arc::new(data), ext: ext.to_string(), w, h, top })
}

/// `/Width` and `/Height`, sane.
fn dims(dict: &pdf_extract::Dictionary) -> Option<(u32, u32)> {
    let w = dict.get(b"Width").ok().and_then(|v| v.as_i64().ok())?;
    let h = dict.get(b"Height").ok().and_then(|v| v.as_i64().ok())?;
    let sides = |v: i64| u32::try_from(v).ok().filter(|v| *v >= 1 && *v <= MAX_PICTURE_PIXELS);
    Some((sides(w)?, sides(h)?))
}

/// Raw 8-bit samples in the documented colour space, as PNG bytes.
fn samples_to_png(pdf: &pdf_extract::Document, dict: &pdf_extract::Dictionary, samples: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    if dict.get(b"BitsPerComponent").ok().and_then(|v| v.as_i64().ok()) != Some(8) {
        log::warn!("PDF: a picture's samples are not 8-bit and are left out");
        return None;
    }
    // One pixel's worth of components after any palette (Indexed) has been opened.
    let comps = color_comps(pdf, dict.get(b"ColorSpace").ok())?;
    let need = u64::from(w) * u64::from(h) * comps as u64;
    if u64::try_from(samples.len()).ok()? < need {
        log::warn!("PDF: a picture holds fewer samples than its size needs and is left out");
        return None;
    }
    let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
    for px in samples[..need as usize].chunks_exact(comps) {
        match comps {
            1 => {
                let v = px[0];
                rgba.extend_from_slice(&[v, v, v, 255]);
            }
            3 => rgba.extend_from_slice(&[px[0], px[1], px[2], 255]),
            // CMYK to RGB, the ink-limiting colours Word writes.
            _ => {
                let (cy, mg, ye, k) = (px[0], px[1], px[2], px[3]);
                let r = (f64::from(255u8) * (1.0 - f64::from(cy) / 255.0) * (1.0 - f64::from(k) / 255.0)) as u8;
                let g = (f64::from(255u8) * (1.0 - f64::from(mg) / 255.0) * (1.0 - f64::from(k) / 255.0)) as u8;
                let bcol = (f64::from(255u8) * (1.0 - f64::from(ye) / 255.0) * (1.0 - f64::from(k) / 255.0)) as u8;
                rgba.extend_from_slice(&[r, g, bcol, 255]);
            }
        }
    }
    let img = image::RgbaImage::from_raw(w, h, rgba)?;
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).ok()?;
    Some(out)
}

/// How many components one pixel of a colour space has.
fn color_comps(pdf: &pdf_extract::Document, cs: Option<&pdf_extract::Object>) -> Option<usize> {
    let cs = cs?;
    match cs {
        pdf_extract::Object::Name(n) => match n.as_slice() {
            b"DeviceGray" | b"CalGray" | b"G" => Some(1),
            b"DeviceRGB" | b"CalRGB" | b"RGB" => Some(3),
            b"DeviceCMYK" | b"CMYK" => Some(4),
            _ => None,
        },
        pdf_extract::Object::Reference(_) => color_comps(pdf, deref(pdf, cs)),
        pdf_extract::Object::Array(a) => match a.first().and_then(|x| x.as_name().ok()) {
            Some(b"ICCBased") => {
                let n_stream = a.get(1)?;
                let stream = deref(pdf, n_stream)?.as_stream().ok()?;
                let n = stream.dict.get(b"N").ok().and_then(|v| v.as_i64().ok())?;
                matches!(n, 1 | 3 | 4).then(|| n as usize)
            }
            Some(b"Indexed") => {
                // A palette's indices themselves are not pixels; opening the palette needs the
                // base space's samples, which v1 leaves out.
                log::warn!("PDF: a picture's palette colour space is not read and it is left out");
                None
            }
            _ => None,
        },
        _ => None,
    }
}

// -------------------------------------------------------------------------------------------
// The flow: paragraphs from the lines, then headings, lists, alignment and the properties.

/// One paragraph as assembled from the lines, before the heading/list/alignment passes. A
/// picture paragraph carries the picture, an text paragraph the lines' text.
struct Raw {
    text: String,
    /// The tallest line's type size in the paragraph.
    size: f64,
    /// Left and right extent, page points, and the page width the first line sat on.
    x0: f64,
    x1: f64,
    page_w: f64,
    /// The paragraph's first line's top (page points, downward), for the page margins.
    top: f64,
    /// The picture when this paragraph is one (nothing else is set).
    pic: Option<Picture>,
}

/// State of the paragraph being assembled on a page.
struct Acc {
    text: String,
    size: f64,
    x0: f64,
    x1: f64,
    page_w: f64,
    top: f64,
    /// Last appended line's baseline, for the pitch to the next.
    last_y: f64,
}

impl Acc {
    fn start(line: &Line, page_w: f64) -> Acc {
        Acc { text: line.text.clone(), size: line.size, x0: line.x0, x1: line.x1, page_w, top: line.top, last_y: line.y }
    }

    /// Append the next line of the same paragraph: a drawn hyphen merges its word.
    fn append(&mut self, line: &Line) {
        if self.text.ends_with('-') && line.text.chars().next().is_some_and(|c| c.is_lowercase()) {
            self.text.pop();
            self.text.push_str(&line.text);
        } else {
            self.text.push(' ');
            self.text.push_str(&line.text);
        }
        self.size = self.size.max(line.size);
        self.x1 = self.x1.max(line.x1);
        self.last_y = line.y;
    }

    fn raw(self) -> Raw {
        Raw { text: self.text, size: self.size, x0: self.x0, x1: self.x1, page_w: self.page_w, top: self.top, pic: None }
    }
}

/// The page geometry the flow carries into the SectionProps: MediaBox of the first page and
/// the measured text margins.
#[derive(Default)]
struct Geo {
    w: f32,
    h: f32,
    left: f32,
    top: f32,
}

/// Merge each page's lines and pictures into the flow's paragraphs, in reading order.
fn assemble(pages: &[PageText], pictures: &[Vec<Picture>]) -> (Vec<Raw>, Geo) {
    let mut raw: Vec<Raw> = Vec::new();
    let mut geo = Geo::default();
    if let Some(page) = pages.first() {
        geo.w = page.w as f32;
        geo.h = page.h as f32;
    }
    for (i, page) in pages.iter().enumerate() {
        let lines = lines_of(page);
        if lines.is_empty() && pictures.get(i).is_none_or(|p| p.is_empty()) {
            continue;
        }
        enum Item {
            L(Line),
            P(Picture),
        }
        // Lines and pictures read page in order: pictures sorted into them by their drawn top.
        let mut items: Vec<(f64, Item)> = lines
            .into_iter()
            .map(|l| (l.top, Item::L(l)))
            .chain(pictures.get(i).map(|pics| pics.as_slice()).unwrap_or(&[]).iter().map(|p| (p.top, Item::P(p.clone()))))
            .collect();
        items.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut acc: Option<Acc> = None;
        for (_, item) in items {
            match item {
                Item::P(pic) => {
                    if let Some(done) = acc.take() {
                        raw.push(done.raw());
                    }
                    raw.push(Raw { text: String::new(), size: 0.0, x0: 0.0, x1: 0.0, page_w: page.w, top: pic.top, pic: Some(pic) });
                }
                Item::L(line) => {
                    match acc.as_mut() {
                        None => acc = Some(Acc::start(&line, page.w)),
                        Some(a) => {
                            // Same paragraph while the baseline advances no further than Word's
                            // visible paragraph spacing (and while the text stays bounded).
                            let pitch = line.y - a.last_y;
                            if pitch > PITCH_SAME * a.size.max(line.size).max(0.5) || a.text.len() + line.text.len() > MAX_PARA_TEXT {
                                let done = acc.take().unwrap_or_else(|| Acc::start(&line, page.w));
                                raw.push(done.raw());
                                acc = Some(Acc::start(&line, page.w));
                            } else {
                                a.append(&line);
                            }
                        }
                    }
                }
            }
        }
        if let Some(done) = acc.take() {
            raw.push(done.raw());
        }
    }
    (raw, geo)
}

/// Turn the raw paragraphs into the flow: sizes into headings and body, marks into list items,
/// extents into alignment, and the PDF's own properties into the flow's metadata.
fn style_and_flow(raw: Vec<Raw>, page_geo: &Geo, pdf: &pdf_extract::Document) -> (Flow, Geo) {
    // The body size: the median size of the text paragraphs.
    let mut sizes: Vec<f64> = raw.iter().filter(|r| r.pic.is_none() && !r.text.is_empty()).map(|r| r.size).collect();
    sizes.sort_by(|a, b| a.total_cmp(b));
    let body = sizes.get(sizes.len() / 2).copied().filter(|s| *s >= 4.0).unwrap_or(12.0);

    // The section: the first page's MediaBox, and the measured text margins (the minima of the
    // text paragraphs' extents; a list item's indent rides on the paragraph). Nothing text at
    // all: Word's own one-inch defaults.
    let mut geo = Geo { w: page_geo.w, h: page_geo.h, left: 72.0, top: 72.0 };
    for r in &raw {
        if r.pic.is_none() && !r.text.is_empty() {
            geo.left = geo.left.min(r.x0 as f32);
            geo.top = geo.top.min(r.top as f32);
        }
    }

    let mut flow = Flow::default();
    for r in raw {
        if let Some(pic) = r.pic {
            let img = Img { data: pic.data, ext: pic.ext, key: None, w: pic.w.round().max(1.0), h: pic.h.round().max(1.0), alt: String::new() };
            let mut p = Para::new(Kind::Normal);
            p.inlines.push(Inline::Image(img));
            flow.blocks.push(FBlock::Para(p));
            continue;
        }
        let c = r.text.trim();
        if c.is_empty() {
            continue;
        }
        let (kind, aligned) = classify(&r, body);
        let (list, text) = list_of(c);
        let mut p = Para::new(kind);
        p.list = list;
        p.align = aligned;
        p.inlines.push(Inline::Text(crate::model::clean_text(&text), crate::model::Fmt { size: Some(r.size as f32), ..Default::default() }));
        flow.blocks.push(FBlock::Para(p));
    }
    if let Some(info) = pdf_info(pdf) {
        flow.meta.title = info.0;
        flow.meta.author = info.1;
        flow.meta.subject = info.2;
        flow.meta.keywords = info.3;
    }
    (flow, geo)
}

/// Classify a paragraph: its size against the document's body size (the heading steps), and a
/// short line sitting on the page's centre as centred (a full line is centred by accident).
fn classify(r: &Raw, body: f64) -> (Kind, Option<Align>) {
    let steps = r.size / body.max(0.1);
    let kind = if steps >= HEAD_STEPS[0] {
        Kind::Heading(1)
    } else if steps >= HEAD_STEPS[1] {
        Kind::Heading(2)
    } else if steps >= HEAD_STEPS[2] {
        Kind::Heading(3)
    } else {
        Kind::Normal
    };
    let mid = (r.x0 + r.x1) / 2.0;
    let page_mid = r.page_w / 2.0;
    let short = (r.x1 - r.x0) < r.page_w * 0.85;
    let aligned = (short && (mid - page_mid).abs() <= CENTER_TOLERANCE).then_some(Align::Center);
    (kind, aligned)
}

/// A bullet or a counting number starts a list item; the marker is drawn off. (The flow's
/// plain numbered lists count from the run's start; homework and reports usually start at 1,
/// so the drawn numeral and the list's count agree.)
fn list_of(text: &str) -> (Option<ListInfo>, String) {
    let trimmed = text.trim_start();
    // Bullets.
    if trimmed.starts_with(|c: char| BULLETS.contains(c)) {
        let n = trimmed.chars().take_while(|c| BULLETS.contains(*c) || *c == ' ').count();
        let rest = trimmed.get(n..).unwrap_or(trimmed);
        return (Some(ListInfo { ordered: false, level: 0 }), rest.trim_start().to_string());
    }
    // Numbers: 1. / 1) / 1] at the start, followed by text.
    let mut chars = trimmed.chars().peekable();
    let mut digits = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() && digits.len() < 4 {
            digits.push(c);
            chars.next();
        } else {
            break;
        }
    }
    if !digits.is_empty() {
        let rest: String = chars.collect();
        let sep = rest.chars().next().unwrap_or(' ');
        if (sep == '.' || sep == ')' || sep == ']') && rest.len() > 1 && rest.chars().nth(1).is_some_and(|c| c.is_whitespace()) {
            let after = rest.split_at(1).1.trim_start();
            if !after.is_empty() {
                return (Some(ListInfo { ordered: true, level: 0 }), after.to_string());
            }
        }
    }
    (None, text.to_string())
}

/// The PDF's own document properties (Info dictionary strings).
fn pdf_info(pdf: &pdf_extract::Document) -> Option<(String, String, String, String)> {
    let info = pdf.trailer.get(b"Info").ok()?;
    let dict = match info {
        pdf_extract::Object::Reference(id) => pdf.get_object(*id).ok()?.as_dict().ok()?,
        pdf_extract::Object::Dictionary(_) => info.as_dict().ok()?,
        _ => return None,
    };
    let s = |k: &[u8]| -> String {
        let found = dict.get(k).ok();
        found.and_then(|v| deref(pdf, v)).and_then(|v| pdf_extract::decode_text_string(v).ok()).unwrap_or_default()
    };
    Some((s(b"Title"), s(b"Author"), s(b"Subject"), s(b"Keywords")))
}

/// The document's section: the MediaBox page and the measured margins (the text's own minima,
/// never below Word's printable inches).
fn section_of(geo: &Geo) -> wordcraft_doc::SectionProps {
    let mut s = wordcraft_doc::SectionProps::default();
    s.page_w = if geo.w.is_finite() && geo.w >= 36.0 { geo.w } else { 612.0 };
    s.page_h = if geo.h.is_finite() && geo.h >= 36.0 { geo.h } else { 792.0 };
    s.margin_left = geo.left.clamp(36.0, s.page_w / 3.0);
    s.margin_top = geo.top.clamp(36.0, s.page_h / 3.0);
    s.margin_right = s.margin_left;
    s.margin_bottom = s.margin_top;
    s
}

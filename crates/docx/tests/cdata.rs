//! CDATA text and kept unknown elements: what the reader accepts must read, write and read
//! again unchanged (the reader counts a part's *escaped* text, like the writer emits it).

use std::io::{Cursor, Write};

use wordcraft_doc::{Document, InlineObject, StoryRef};
use wordcraft_docx::{read, write};

const W_NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;
const ROOT_RELS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        zw.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(bytes).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

/// A one-part package whose body is `body`.
fn package(body: &str) -> Vec<u8> {
    let document = format!(r#"<w:document {W_NS}><w:body>{body}</w:body></w:document>"#);
    zip(&[("_rels/.rels", ROOT_RELS.as_bytes()), ("word/document.xml", document.as_bytes())])
}

fn rt(bytes: &[u8]) -> Document {
    let d = read(bytes).expect("read");
    let out = write(&d).expect("write");
    read(&out).expect("read again")
}

/// The first kept unknown element of the body, as it will be written back.
fn opaque_xml(d: &Document) -> String {
    d.body
        .iter()
        .filter_map(|b| b.as_para())
        .flat_map(|p| &p.objects)
        .find_map(|o| match o {
            InlineObject::Opaque { xml, .. } => Some(xml.clone()),
            _ => None,
        })
        .expect("a kept unknown element")
}

#[test]
fn cdata_text_in_a_w_t_round_trips_unchanged() {
    let text = "<&>\"'";
    let d = rt(&package(&format!(r#"<w:p><w:r><w:t><![CDATA[{text}]]></w:t></w:r></w:p>"#)));
    assert_eq!(d.plain_text(StoryRef::Body), text);
}

#[test]
fn a_kept_element_that_cannot_be_written_is_an_error_not_a_drop() {
    use wordcraft_doc::Paragraph;
    use wordcraft_doc::props::CharProps;

    // With no text to fall back on, an element the writer can't parse back would vanish from the
    // saved file; the save must fail instead of dropping it silently.
    let mut d = Document::new();
    let mut p = Paragraph::new();
    p.insert_object(0, InlineObject::Opaque { format: "docx".into(), xml: "<broken".into(), text: String::new() }, &CharProps::default()).unwrap();
    d.body = vec![wordcraft_doc::para_block(p)];
    let e = write(&d).expect_err("saving must not silently drop the element");
    assert!(matches!(e, wordcraft_docx::DocxError::Xml(_)), "{e}");
    // The same element with text to keep writes the text instead.
    let mut p = Paragraph::new();
    p.insert_object(0, InlineObject::Opaque { format: "docx".into(), xml: "<broken".into(), text: "kept".into() }, &CharProps::default()).unwrap();
    d.body = vec![wordcraft_doc::para_block(p)];
    assert!(rt(&write(&d).unwrap()).plain_text(StoryRef::Body).contains("kept"));
}

#[test]
fn cdata_text_in_a_kept_unknown_element_round_trips_unchanged() {
    // An element this reader doesn't model sits in the paragraph with its own run (so it holds
    // text the writer must parse back, the case `Opaque` writes verbatim).
    let d1 = read(&package(r#"<w:p><w:futureThing><w:r><w:t><![CDATA[a<b&c]]></w:t></w:r></w:futureThing></w:p>"#)).unwrap();
    let first = opaque_xml(&d1);
    assert!(first.contains("a&lt;b&amp;c"), "text is kept escaped: {first}");
    let out = write(&d1).expect("write");
    let d2 = read(&out).expect("read again");
    assert_eq!(opaque_xml(&d2), first);
}

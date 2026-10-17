//! Content-integrity regression tests: text and objects around constructs the importer does not
//! model must survive a read, and the document must still write.

use std::io::{Read, Write};

use wordcraft_doc::{Document, Paragraph};

const NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml""#;

fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        zw.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(bytes).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

/// A package whose body is `body`.
fn docx(body: &str) -> Vec<u8> {
    let root = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {NS}><w:body>{body}</w:body></w:document>"#);
    zip(&[("_rels/.rels", root.as_bytes()), ("word/document.xml", doc.as_bytes())])
}

fn document_xml(bytes: &[u8]) -> String {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    z.by_name("word/document.xml").unwrap().read_to_string(&mut s).unwrap();
    s
}

fn paragraphs(d: &Document) -> Vec<&Paragraph> {
    d.body.iter().filter_map(|b| b.as_para()).collect()
}

/// Reads `body`, writes it back, and returns the first paragraph's text after a second read.
fn text_after_round_trip(body: &str) -> String {
    let d = wordcraft_docx::read(&docx(body)).unwrap();
    let written = wordcraft_docx::write(&d).unwrap();
    let d2 = wordcraft_docx::read(&written).unwrap();
    paragraphs(&d2)[0].plain_text()
}

#[test]
fn text_around_a_sub_document_reference_survives() {
    let body = r#"<w:p><w:r><w:t>Before</w:t></w:r><w:subDoc r:id="rId9"/><w:r><w:t>After</w:t></w:r></w:p>"#;
    let text = text_after_round_trip(body);
    assert!(text.contains("Before") && text.contains("After"), "text lost: {text:?}");
}

#[test]
fn text_around_an_unmodelled_run_child_survives() {
    let body = r#"<w:p><w:r><w:t>Left</w:t><w:fldData/><w:t>Right</w:t></w:r></w:p>"#;
    let text = text_after_round_trip(body);
    assert!(text.contains("Left") && text.contains("Right"), "text lost: {text:?}");
}

#[test]
fn text_around_an_unmodelled_paragraph_child_survives() {
    let body = r#"<w:p><w:r><w:t>Start</w:t></w:r><w:customXmlDelRangeStart w:id="1"/><w:r><w:t>End</w:t></w:r></w:p>"#;
    let text = text_after_round_trip(body);
    assert!(text.contains("Start") && text.contains("End"), "text lost: {text:?}");
}

#[test]
fn text_in_a_table_cell_around_an_unmodelled_child_survives() {
    let d = wordcraft_docx::read(&docx(
        r#"<w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="100"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>Cell</w:t></w:r><w:subDoc r:id="rId9"/><w:r><w:t>Tail</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/>"#,
    ))
    .unwrap();
    let written = wordcraft_docx::write(&d).unwrap();
    let xml = document_xml(&written);
    assert!(xml.contains("Cell") && xml.contains("Tail"), "cell text lost: {xml}");
}

#[test]
fn header_text_around_an_unmodelled_child_survives() {
    let mut z = Vec::new();
    let root = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc_rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdH" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/></Relationships>"#;
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {NS}><w:body><w:p/><w:sectPr><w:headerReference w:type="default" r:id="rIdH"/></w:sectPr></w:body></w:document>"#
    );
    let header = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:hdr {NS}><w:p><w:r><w:t>HeadA</w:t></w:r><w:subDoc r:id="rId9"/><w:r><w:t>HeadB</w:t></w:r></w:p></w:hdr>"#
    );
    z.extend(zip(&[
        ("_rels/.rels", root.as_bytes()),
        ("word/_rels/document.xml.rels", doc_rels.as_bytes()),
        ("word/document.xml", doc.as_bytes()),
        ("word/header1.xml", header.as_bytes()),
    ]));
    let d = wordcraft_docx::read(&z).unwrap();
    let written = wordcraft_docx::write(&d).unwrap();
    let mut arc = zip::ZipArchive::new(std::io::Cursor::new(&written)).unwrap();
    let mut names: Vec<String> = arc.file_names().map(str::to_owned).collect();
    names.retain(|n| n.contains("header"));
    let mut all = String::new();
    for n in names {
        arc.by_name(&n).unwrap().read_to_string(&mut all).unwrap();
    }
    assert!(all.contains("HeadA") && all.contains("HeadB"), "header text lost");
}

#[test]
fn unmodelled_inline_xml_without_relationships_is_kept_on_save() {
    let body = r#"<w:p><w:r><w:t>Before</w:t></w:r><w:bogusMark w:val="7"/><w:r><w:t>After</w:t></w:r></w:p>"#;
    let d = wordcraft_docx::read(&docx(body)).unwrap();
    let xml = document_xml(&wordcraft_docx::write(&d).unwrap());
    assert!(xml.contains(r#"<w:bogusMark w:val="7"/>"#), "unmodelled element dropped on save: {xml}");
}

#[test]
fn unmodelled_inline_reference_is_not_written_with_a_dangling_id() {
    let body = r#"<w:p><w:r><w:t>Before</w:t></w:r><w:subDoc r:id="rId9"/><w:r><w:t>After</w:t></w:r></w:p>"#;
    let d = wordcraft_docx::read(&docx(body)).unwrap();
    let xml = document_xml(&wordcraft_docx::write(&d).unwrap());
    assert!(!xml.contains("rId9"), "reference to a part that is not written: {xml}");
}

#[test]
fn text_around_a_moved_range_marker_survives() {
    let body = r#"<w:p><w:r><w:t>One</w:t></w:r><w:moveFromRangeStart w:id="2" w:name="m"/><w:r><w:t>Two</w:t></w:r></w:p>"#;
    let text = text_after_round_trip(body);
    assert!(text.contains("One") && text.contains("Two"), "text lost: {text:?}");
}

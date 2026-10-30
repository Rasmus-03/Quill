//! Hostile part names: a name a Windows extractor would read as a path (`..\evil.bin`,
//! `C:evil`, `..`) must not be carried through a read → write cycle, or a zip built by WordCraft
//! would unpack outside the package folder.

use std::io::{Cursor, Read, Write};
use std::sync::Arc;

use quick_xml::events::Event;
use wordcraft_doc::StoryRef;
use wordcraft_docx::{Flavor, read, write_as};

const W_NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;
const PKG_RELS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const OFFICE_DOC: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
const CUSTOM_XML: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml";
const VBA_PROJECT: &str = "http://schemas.microsoft.com/office/2006/relationships/vbaProject";
const VBA_DATA: &str = "http://schemas.microsoft.com/office/2006/relationships/vbaData";

fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        zw.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(bytes).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

/// The forbidden shapes in a part name — what an extractor on Windows would read as a way out of
/// the package folder. An explicit list of each form rather than a copy of the reader's own rule,
/// so a blind spot in one check can't hide behind the other. Allowed: a colon after the first
/// segment (`word/sig:legacy.bin` is a Word part name) and root-level names (`store.xml`,
/// `_rels/store.xml.rels`).
fn clean(name: &str) -> bool {
    // An empty name, or one starting with `/`: absolute, not package-relative.
    if name.is_empty() || name.starts_with('/') {
        return false;
    }
    // A backslash is a path separator on Windows.
    if name.contains('\\') {
        return false;
    }
    // Control characters can't be represented in a file name.
    if name.bytes().any(|b| b < 0x20 || b == 0x7F) {
        return false;
    }
    // `^[A-Za-z]:` is a drive prefix, absolute on Windows.
    if matches!(name.as_bytes(), [c, b':', ..] if c.is_ascii_alphabetic()) {
        return false;
    }
    // An empty, `.` or `..` segment walks out of the folder (trimmed: Windows strips trailing dots
    // and spaces, so `.. ` is still `..`).
    !name.split('/').any(|seg| matches!(seg.trim(), "" | "." | ".."))
}

/// A package that names a malicious part as the target of a custom XML relationship and of the
/// macro project's relationships, with matching zip entries (`word/..\evil.bin` etc.). The
/// package-level custom XML relationship names a drive prefix (`C:evil.bin`), which is absolute
/// on Windows rather than package-relative.
fn hostile_package() -> Vec<u8> {
    let root_rels = format!(
        r#"<Relationships xmlns="{PKG_RELS}"><Relationship Id="rId1" Type="{OFFICE_DOC}" Target="word/document.xml"/><Relationship Id="rId2" Type="{CUSTOM_XML}" Target="C:evil.bin"/></Relationships>"#
    );
    let document_rels = format!(
        r#"<Relationships xmlns="{PKG_RELS}"><Relationship Id="rId1" Type="{CUSTOM_XML}" Target="..\evil.bin"/><Relationship Id="rId2" Type="{VBA_PROJECT}" Target="vbaProject.bin"/></Relationships>"#
    );
    let vba_rels = format!(r#"<Relationships xmlns="{PKG_RELS}"><Relationship Id="rId1" Type="{VBA_DATA}" Target="..\vba.bin"/></Relationships>"#);
    let document = format!(r#"<w:document {W_NS}><w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p></w:body></w:document>"#);
    zip(&[
        ("_rels/.rels", root_rels.as_bytes()),
        ("word/document.xml", document.as_bytes()),
        ("word/_rels/document.xml.rels", document_rels.as_bytes()),
        ("word/vbaProject.bin", b"the macros"),
        ("word/_rels/vbaProject.bin.rels", vba_rels.as_bytes()),
        (r"word/..\evil.bin", b"evil"),
        (r"word/..\vba.bin", b"vba"),
        ("C:evil.bin", b"evil"),
    ])
}

fn read_doc(bytes: &[u8]) -> wordcraft_doc::Document {
    read(bytes).expect("hostile part names are skipped, not fatal")
}

/// Every zip entry is a clean part name; `[Content_Types].xml` overrides too. Returns the names.
fn saved_names(bytes: &[u8]) -> Vec<String> {
    let mut a = zip::ZipArchive::new(Cursor::new(bytes.to_vec())).unwrap();
    let mut names = Vec::new();
    for i in 0..a.len() {
        let name = a.by_index(i).unwrap().name().expect("zip entry name").to_string();
        assert!(clean(&name), "hostile part name written: {name:?}");
        names.push(name);
    }
    let mut ct = Vec::new();
    a.by_name("[Content_Types].xml").unwrap().read_to_end(&mut ct).unwrap();
    let mut r = quick_xml::Reader::from_reader(ct.as_slice());
    let mut buf = Vec::new();
    let mut overrides = 0;
    loop {
        match r.read_event_into(&mut buf).unwrap() {
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == b"Override" => {
                let part = e.attributes().flatten().find(|a| a.key.local_name().as_ref() == b"PartName").expect("Override without PartName");
                let part = String::from_utf8_lossy(&part.value).into_owned();
                assert!(part.starts_with('/'), "override without a leading slash: {part:?}");
                assert!(clean(part.trim_start_matches('/')), "hostile part name in [Content_Types].xml: {part:?}");
                overrides += 1;
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    assert!(overrides > 0, "no part overrides written");
    names
}

#[test]
fn hostile_part_names_are_dropped_on_read() {
    let doc = read_doc(&hostile_package());
    assert_eq!(doc.plain_text(StoryRef::Body), "Hello");
    // The macro project is kept, but no hostile name reaches the model (the `wordcraft:` keys
    // are WordCraft's own bookkeeping, not package parts).
    assert!(doc.passthrough.contains_key("word/vbaProject.bin"), "{:?}", doc.passthrough.keys().collect::<Vec<_>>());
    for key in doc.passthrough.keys().filter(|k| !k.starts_with("wordcraft:")) {
        assert!(clean(key), "hostile part kept: {key:?}");
    }
    assert!(!doc.passthrough.contains_key("C:evil.bin"), "drive-prefixed part kept");
    let manifest = doc.passthrough.get("wordcraft:vbaProject.related").map(|m| String::from_utf8_lossy(m).into_owned()).unwrap_or_default();
    assert!(!manifest.contains('\\') && !manifest.contains(".."), "hostile VBA manifest kept: {manifest:?}");
}

#[test]
fn a_save_from_a_hostile_package_writes_only_clean_part_names() {
    let out = write_as(&read_doc(&hostile_package()), Flavor::MacroDocument).expect("write");
    let names = saved_names(&out);
    // The real macro project survives the trip; the hostile neighbours do not.
    assert!(names.iter().any(|n| n == "word/vbaProject.bin"), "{names:?}");
}

#[test]
fn a_hostile_manifest_cannot_make_the_writer_emit_a_hostile_name() {
    // What a reader without the part-name checks would have kept: hostile bytes (a traversal and
    // a Windows drive prefix), an embedded-object manifest, a custom-XML root and a VBA-related
    // line, all naming them.
    let mut doc = read_doc(&hostile_package());
    let hostile = r"word/..\escape.bin";
    let drive = "C:escape.bin";
    for name in [hostile, drive] {
        doc.passthrough.insert(name.into(), Arc::new(b"payload".to_vec()));
    }
    doc.passthrough
        .insert("wordcraft:customXml.roots".into(), Arc::new(format!("D\t{CUSTOM_XML}\t{hostile}\nD\t{CUSTOM_XML}\t{drive}\n").into_bytes()));
    doc.passthrough.insert(
        "wordcraft:embedded.parts".into(),
        Arc::new(format!("P\t{hostile}\tapplication/octet-stream\nP\t{drive}\tapplication/octet-stream\n").into_bytes()),
    );
    doc.passthrough.insert(
        "wordcraft:vbaProject.related".into(),
        Arc::new(format!("{VBA_DATA}\t{hostile}\tapplication/octet-stream\n{VBA_DATA}\t{drive}\tapplication/octet-stream\n").into_bytes()),
    );
    let out = write_as(&doc, Flavor::MacroDocument).expect("write");
    let names = saved_names(&out);
    assert!(names.iter().any(|n| n == "word/vbaProject.bin"), "{names:?}");
    assert!(!names.iter().any(|n| n.contains("escape")), "hostile part written: {names:?}");
    assert!(!names.iter().any(|n| matches!(n.as_bytes(), [c, b':', ..] if c.is_ascii_alphabetic())), "drive-prefixed part written: {names:?}");
    let mut a = zip::ZipArchive::new(Cursor::new(out)).unwrap();
    for i in 0..a.len() {
        let name = a.by_index(i).unwrap().name().expect("zip entry name").to_string();
        if !name.ends_with(".rels") {
            continue;
        }
        let mut s = String::new();
        a.by_name(&name).unwrap().read_to_string(&mut s).unwrap();
        assert!(!s.contains("escape") && !s.contains('\\'), "hostile relationship written in {name}: {s}");
        assert!(!s.contains(r#"Target="C:"#) && !s.contains(r#"Target="./C:"#), "drive-prefixed relationship written in {name}: {s}");
    }
}

/// A colon after the first part-name segment is not a drive and not a traversal: Word writes
/// names like `word/sig:legacy.bin` (a VBA signature part), and the part must survive read →
/// write, byte for byte, with the relationship to it kept relative (`./sig:legacy.bin`, or a
/// consumer reading the target as a URI would see the `sig:` scheme).
#[test]
fn a_colon_part_name_is_kept_and_stays_a_relative_reference() {
    let root_rels =
        format!(r#"<Relationships xmlns="{PKG_RELS}"><Relationship Id="rId1" Type="{OFFICE_DOC}" Target="word/document.xml"/></Relationships>"#);
    let document_rels =
        format!(r#"<Relationships xmlns="{PKG_RELS}"><Relationship Id="rId1" Type="{VBA_PROJECT}" Target="vbaProject.bin"/></Relationships>"#);
    let vba_rels =
        format!(r#"<Relationships xmlns="{PKG_RELS}"><Relationship Id="rId1" Type="{VBA_DATA}" Target="sig:legacy.bin"/></Relationships>"#);
    let document = format!(r#"<w:document {W_NS}><w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p></w:body></w:document>"#);
    let package = zip(&[
        ("_rels/.rels", root_rels.as_bytes()),
        ("word/document.xml", document.as_bytes()),
        ("word/_rels/document.xml.rels", document_rels.as_bytes()),
        ("word/vbaProject.bin", b"the macros"),
        ("word/_rels/vbaProject.bin.rels", vba_rels.as_bytes()),
        ("word/sig:legacy.bin", b"signature bytes"),
    ]);

    let doc = read_doc(&package);
    assert_eq!(doc.passthrough.get("word/sig:legacy.bin").map(|b| b.as_slice()), Some(b"signature bytes".as_slice()), "colon part dropped on read");

    let out = write_as(&doc, Flavor::MacroDocument).expect("write");
    let names = saved_names(&out);
    assert!(names.iter().any(|n| n == "word/sig:legacy.bin"), "colon part dropped on write: {names:?}");
    let mut a = zip::ZipArchive::new(Cursor::new(out.clone())).unwrap();
    let mut bytes = Vec::new();
    a.by_name("word/sig:legacy.bin").unwrap().read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"signature bytes");
    let mut rels = String::new();
    a.by_name("word/_rels/vbaProject.bin.rels").unwrap().read_to_string(&mut rels).unwrap();
    assert!(rels.contains(r#"Target="./sig:legacy.bin""#), "relationship to the colon part isn't relative: {rels}");

    // Generation two: the colon part survives another read → write byte for byte.
    let doc2 = read(&out).expect("read back");
    assert_eq!(doc2.passthrough.get("word/sig:legacy.bin").map(|b| b.as_slice()), Some(b"signature bytes".as_slice()));
    let out2 = write_as(&doc2, Flavor::MacroDocument).expect("write 2");
    assert!(saved_names(&out2).iter().any(|n| n == "word/sig:legacy.bin"));
}

/// A part at the package root is legal OPC: `store.xml` keeps its relationships in
/// `_rels/store.xml.rels`, the same mapping `Package::rels` reads. A naive join would produce
/// `/_rels/store.xml.rels`, which the writer's own zip-entry guard refuses (and a Flat OPC export
/// would then name `//_rels/store.xml.rels`).
#[test]
fn a_root_level_part_and_its_rels_are_written_without_a_leading_slash() {
    let root_rels =
        format!(r#"<Relationships xmlns="{PKG_RELS}"><Relationship Id="rId1" Type="{OFFICE_DOC}" Target="word/document.xml"/></Relationships>"#);
    let document_rels =
        format!(r#"<Relationships xmlns="{PKG_RELS}"><Relationship Id="rId1" Type="{CUSTOM_XML}" Target="../store.xml"/></Relationships>"#);
    let store_rels = format!(
        r#"<Relationships xmlns="{PKG_RELS}"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/" TargetMode="External"/></Relationships>"#
    );
    let document = format!(r#"<w:document {W_NS}><w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p></w:body></w:document>"#);
    let package = zip(&[
        ("_rels/.rels", root_rels.as_bytes()),
        ("word/document.xml", document.as_bytes()),
        ("word/_rels/document.xml.rels", document_rels.as_bytes()),
        ("store.xml", b"<store xmlns=\"urn:store\"/>"),
        ("_rels/store.xml.rels", store_rels.as_bytes()),
    ]);

    let doc = read_doc(&package);
    assert!(doc.passthrough.contains_key("store.xml"), "{:?}", doc.passthrough.keys().collect::<Vec<_>>());

    let out = write_as(&doc, Flavor::Document).expect("a root-level part with rels must save");
    let names = saved_names(&out);
    for want in ["store.xml", "_rels/store.xml.rels"] {
        assert!(names.iter().any(|n| n == want), "{want} missing from {names:?}");
    }
    assert!(!names.iter().any(|n| n.starts_with('/')), "absolute part name written: {names:?}");
    // The oracle used above allows root-level and colon names; this fixture is the root-level case.
    for ok in ["store.xml", "_rels/store.xml.rels", "word/sig:legacy.bin"] {
        assert!(clean(ok), "the oracle rejects the valid name {ok:?}");
    }
    let mut a = zip::ZipArchive::new(Cursor::new(out.clone())).unwrap();
    let mut ct = String::new();
    a.by_name("[Content_Types].xml").unwrap().read_to_string(&mut ct).unwrap();
    assert!(!ct.contains(r#"PartName="//"#), "content-type override with a double slash: {ct}");
    // The saved package opens again, with the root-level part still in it.
    let doc2 = read(&out).expect("the saved package opens");
    assert_eq!(doc2.plain_text(StoryRef::Body), "Hello");
    assert!(doc2.passthrough.contains_key("store.xml"), "{:?}", doc2.passthrough.keys().collect::<Vec<_>>());
}

/// CRC-32 (the polynomial ZIP uses), so a hand-built entry is a valid one.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// A stored ZIP whose entry names are raw bytes, because the `zip` writer takes `&str` and a name
/// that is not UTF-8 is exactly what this test needs to build.
fn raw_zip(entries: &[(&[u8], &[u8])]) -> Vec<u8> {
    raw_zip_flags(entries, 0)
}

fn raw_zip_flags(entries: &[(&[u8], &[u8])], flags: u16) -> Vec<u8> {
    fn put16(out: &mut Vec<u8>, v: u16) {
        out.extend_from_slice(&v.to_le_bytes());
    }
    fn put32(out: &mut Vec<u8>, v: u32) {
        out.extend_from_slice(&v.to_le_bytes());
    }

    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let crc = crc32(data);
        let size = u32::try_from(data.len()).unwrap();
        let name_len = u16::try_from(name.len()).unwrap();
        let offset = u32::try_from(out.len()).unwrap();

        // Local file header, then the stored bytes.
        put32(&mut out, 0x0403_4b50);
        put16(&mut out, 20); // version needed
        put16(&mut out, flags);
        put16(&mut out, 0); // stored
        put16(&mut out, 0); // time
        put16(&mut out, 0); // date
        put32(&mut out, crc);
        put32(&mut out, size);
        put32(&mut out, size);
        put16(&mut out, name_len);
        put16(&mut out, 0); // extra field length
        out.extend_from_slice(name);
        out.extend_from_slice(data);

        // The matching central directory record.
        put32(&mut central, 0x0201_4b50);
        put16(&mut central, 20); // version made by
        put16(&mut central, 20); // version needed
        put16(&mut central, flags);
        put16(&mut central, 0); // stored
        put16(&mut central, 0); // time
        put16(&mut central, 0); // date
        put32(&mut central, crc);
        put32(&mut central, size);
        put32(&mut central, size);
        put16(&mut central, name_len);
        put16(&mut central, 0); // extra
        put16(&mut central, 0); // comment
        put16(&mut central, 0); // disk
        put16(&mut central, 0); // internal attributes
        put32(&mut central, 0); // external attributes
        put32(&mut central, offset);
        central.extend_from_slice(name);
    }

    let cd_offset = u32::try_from(out.len()).unwrap();
    let cd_size = u32::try_from(central.len()).unwrap();
    let count = u16::try_from(entries.len()).unwrap();
    out.extend_from_slice(&central);
    put32(&mut out, 0x0605_4b50);
    put16(&mut out, 0); // this disk
    put16(&mut out, 0); // disk with the central directory
    put16(&mut out, count);
    put16(&mut out, count);
    put32(&mut out, cd_size);
    put32(&mut out, cd_offset);
    put16(&mut out, 0); // comment length
    out
}

/// An entry whose name is not UTF-8 must not take the document down with it.
///
/// `zip` 9 gives the name back as a `Result`; in 9.1.3 it decodes a non-UTF-8 name lossily (CP437,
/// per the spec) rather than failing, so today this entry survives with a substituted name. The
/// reader's arm for the `Err` case is defensive and unproven — what this test pins is the part
/// that must hold either way: one odd entry name, one document that still opens and still has its
/// text.
#[test]
fn a_part_name_that_is_not_utf8_does_not_take_the_document_down() {
    let root_rels =
        format!(r#"<Relationships xmlns="{PKG_RELS}"><Relationship Id="rId1" Type="{OFFICE_DOC}" Target="word/document.xml"/></Relationships>"#);
    let document = format!(r#"<w:document {W_NS}><w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p></w:body></w:document>"#);
    let package = raw_zip(&[
        (b"_rels/.rels", root_rels.as_bytes()),
        (b"word/document.xml", document.as_bytes()),
        // A name CP437 would spell and UTF-8 cannot: no encoding flag is set for this entry.
        (b"word/\xff\xfe.xml", b"not a part of this package"),
    ]);

    let d = read_doc(&package);
    assert_eq!(d.plain_text(StoryRef::Body), "Hello", "the good part is still read");
}

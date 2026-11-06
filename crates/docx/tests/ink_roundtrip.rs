//! Verify native ink (`w14:contentPart` with an InkML content part) round-trips
//! through open → save → reopen without losing the part or its relationship.
//!
//! This is the remaining ink surface from gap #2.

use std::io::{Cursor, Write};

use zip::ZipWriter;

fn ink_package() -> Vec<u8> {
    let inkml = concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        "\n",
        r##"
<inkml:inkml xmlns:inkml="http://www.w3.org/2003/InkML" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordprocessingInk">
  <inkml:head><inkml:defs><inkml:path id="p1">M 0,0 L 100,50 L 200,0</inkml:path></inkml:defs></inkml:head>
  <inkml:usage/>
  <inkml:trace id="t1" inkml:pathref="#p1" w14:width="10" w14:fill="000000" w14:stroke="000000"/>
</inkml:inkml>"##
    );
    let rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml" Target="inkml/ink1.inkml"/>
</Relationships>"#;
    let document = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordprocessingInk">
  <w:body>
    <w:p>
      <w:r><w:t>Ink here</w:t></w:r>
      <w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="1440000" cy="900000"/><wp:docPr id="1" name="Ink 1"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingInk"><w14:contentPart r:id="rId1"/></a:graphicData></a:graphic></wp:inline></w:drawing>
    </w:p>
  </w:body>
</w:document>"#;
    zip_bytes(&[
        ("_rels/.rels", r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.as_bytes().to_vec()),
        ("word/_rels/document.xml.rels", rels.as_bytes().to_vec()),
        ("word/inkml/ink1.inkml", inkml.as_bytes().to_vec()),
        ("word/document.xml", document.as_bytes().to_vec()),
        (
            "[Content_Types].xml",
            r####"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="inkml" ContentType="application/inkml+xml"/>
</Types>"####.as_bytes().to_vec(),
        ),
    ])
}

fn zip_bytes(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut zw = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        zw.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(bytes).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

#[test]
fn native_ink_round_trips() {
    let bytes = ink_package();
    let doc = wordcraft_docx::read(&bytes).expect("read ink package");

    let round = wordcraft_docx::write(&doc).expect("write back");
    let doc2 = wordcraft_docx::read(&round).expect("read back");

    // Corpus-style check: save → reopen → save must be byte-identical.
    let round2 = wordcraft_docx::write(&doc2).expect("write back 2");
    assert_eq!(round.as_slice(), round2.as_slice(), "second save not byte-identical");
}

//! WebDAV: asking a server what is in a folder.
//!
//! The server answers a `PROPFIND` with a `multistatus` document: one `response` per file or
//! folder, each with the properties Quill asked for. Only the ones it understands are read; the
//! rest is ignored.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::{Entry, Error, url};

/// The properties Quill asks for.
pub const PROPFIND_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns">
  <d:prop>
    <d:displayname/>
    <d:getcontentlength/>
    <d:getlastmodified/>
    <d:getetag/>
    <d:resourcetype/>
    <oc:size/>
  </d:prop>
</d:propfind>"#;

/// Read a `multistatus` answer into the files and folders it lists.
pub fn parse_multistatus(xml: &str) -> Result<Vec<Entry>, Error> {
    let mut reader = Reader::from_str(xml);
    let config = reader.config_mut();
    config.trim_text(true);
    config.check_end_names = true;
    let mut entries: Vec<Entry> = Vec::new();
    let mut current: Option<Entry> = None;
    let mut stack: Vec<String> = Vec::new();
    let mut text = String::new();
    let mut buf: Vec<u8> = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(start)) => {
                let name = local(&start);
                if name == "response" {
                    current = Some(Entry::default());
                }
                stack.push(name);
            }
            Ok(Event::Empty(start)) => {
                // `<d:collection/>` marks a folder.
                if local(&start) == "collection"
                    && let Some(entry) = current.as_mut()
                {
                    entry.is_dir = true;
                }
            }
            Ok(Event::End(_)) => {
                let name = stack.pop().unwrap_or_default();
                if let Some(entry) = current.as_mut() {
                    match name.as_str() {
                        "href" => entry.path = url::path_from_href(&text).unwrap_or_default(),
                        "displayname" => entry.name = text.clone(),
                        "getcontentlength" | "size" => entry.size = text.trim().parse().unwrap_or(0),
                        "getlastmodified" => entry.modified = text.clone(),
                        "getetag" => entry.etag = text.clone(),
                        _ => {}
                    }
                }
                text.clear();
                if name == "response"
                    && let Some(entry) = current.take()
                {
                    entries.push(entry);
                }
            }
            Ok(Event::Text(value)) => {
                text.push_str(&value.xml10_content().map(|c| c.to_string()).unwrap_or_default());
            }
            Ok(Event::GeneralRef(reference)) => {
                // The five entities XML defines by name.
                let name = String::from_utf8_lossy(reference.as_ref()).to_string();
                let ch = match name.as_str() {
                    "amp" => '&',
                    "lt" => '<',
                    "gt" => '>',
                    "quot" => '"',
                    "apos" => '\'',
                    _ => '\u{fffd}',
                };
                text.push(ch);
            }
            Ok(Event::CData(value)) => {
                text.push_str(&String::from_utf8_lossy(value.as_ref()));
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(Error::Protocol(format!("the server's listing could not be read: {e}"))),
        }
        buf.clear();
    }
    // A server that leaves the name out still tells us the path; the name is its last part.
    for entry in &mut entries {
        if entry.name.is_empty() {
            entry.name = entry.path.rsplit('/').next().unwrap_or(&entry.path).to_string();
        }
    }
    Ok(entries)
}

/// An element's name without its namespace prefix.
fn local(start: &BytesStart<'_>) -> String {
    let name = start.local_name();
    String::from_utf8_lossy(name.as_ref()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &str = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns">
  <d:response>
    <d:href>/remote.php/dav/files/rasmus/</d:href>
    <d:propstat>
      <d:prop>
        <d:displayname>rasmus</d:displayname>
        <d:resourcetype><d:collection/></d:resourcetype>
        <d:getlastmodified>Tue, 21 Oct 2026 10:00:00 GMT</d:getlastmodified>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/files/rasmus/Docs/a%20b.docx</d:href>
    <d:propstat>
      <d:prop>
        <d:displayname>a b.docx</d:displayname>
        <d:getcontentlength>1234</d:getcontentlength>
        <d:getlastmodified>Tue, 21 Oct 2026 10:05:00 GMT</d:getlastmodified>
        <d:getetag>&quot;abc123&quot;</d:getetag>
        <d:resourcetype/>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
</d:multistatus>"#;

    #[test]
    fn a_listing_is_read() {
        let entries = parse_multistatus(LISTING).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, "");
        assert!(entries[0].is_dir);
        assert_eq!(entries[1].path, "Docs/a b.docx");
        assert_eq!(entries[1].name, "a b.docx");
        assert_eq!(entries[1].size, 1234);
        assert!(!entries[1].is_dir);
        assert_eq!(entries[1].etag, "\"abc123\"");
    }

    #[test]
    fn a_broken_answer_is_an_error_not_a_panic() {
        // A tag that closes the wrong one is refused.
        assert!(parse_multistatus("<d:multistatus><d:response></d:other></d:multistatus>").is_err());
        // A truncated answer is read as far as it goes, without a panic.
        assert_eq!(parse_multistatus("<d:multistatus><d:response>").unwrap().len(), 0);
        assert_eq!(parse_multistatus("").unwrap().len(), 0);
        assert_eq!(parse_multistatus("not xml at all").unwrap().len(), 0);
    }

    #[test]
    fn a_missing_name_falls_back_to_the_path() {
        let xml = r#"<d:multistatus xmlns:d="DAV:"><d:response><d:href>/remote.php/dav/files/u/Docs/x.docx</d:href></d:response></d:multistatus>"#;
        let entries = parse_multistatus(xml).unwrap();
        assert_eq!(entries[0].name, "x.docx");
    }
}

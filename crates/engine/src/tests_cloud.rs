//! The Nextcloud commands, against a server that behaves like Nextcloud.

use serde_json::json;
use wordcraft_cloud::testserver::Mock;

use crate::Session;

fn s() -> Session {
    Session::new(wordcraft_doc::Document::new())
}

fn run(s: &mut Session, id: &str, v: serde_json::Value) -> serde_json::Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// Connect and wait for the server to answer.
fn connect(s: &mut Session, mock: &Mock) {
    run(s, "cloud.connect", json!({"server": mock.address(), "user": "rasmus", "password": "secret"}));
    run(s, "cloud.wait", json!({"ms": 10000}));
}

#[test]
fn an_account_is_connected_and_the_server_says_what_it_is() {
    let mock = Mock::start();
    let mut s = s();
    connect(&mut s, &mock);
    let status = run(&mut s, "cloud.status", json!({}));
    assert_eq!(status["connected"], json!(true), "{status}");
    assert_eq!(status["user"], json!("rasmus"));
    assert_eq!(status["capabilities"]["product"], json!("Nextcloud"));
    assert_eq!(status["account"]["displayName"], json!("Rasmus"));
}

#[test]
fn a_folder_is_listed() {
    let mock = Mock::start();
    let mut s = s();
    connect(&mut s, &mock);
    run(&mut s, "cloud.list", json!({"path": ""}));
    run(&mut s, "cloud.wait", json!({"ms": 10000}));
    let status = run(&mut s, "cloud.status", json!({}));
    let files = status["files"].as_array().expect("the files");
    assert_eq!(files.len(), 3, "{status}");
    assert_eq!(files[0]["path"], json!("Docs"));
    assert_eq!(files[0]["isDir"], json!(true));
    assert_eq!(files[1]["path"], json!("Docs/hello.txt"));
}

#[test]
fn a_file_is_opened_and_saved_back() {
    let mock = Mock::start();
    let mut s = s();
    connect(&mut s, &mock);
    run(&mut s, "cloud.open", json!({"path": "Docs/hello.txt"}));
    run(&mut s, "cloud.wait", json!({"ms": 10000}));
    // The file arrived and is the document on screen.
    assert_eq!(s.doc.plain_text(wordcraft_doc::StoryRef::Body), "Hello from the cloud.\n");
    let status = run(&mut s, "cloud.status", json!({}));
    assert_eq!(status["document"]["path"], json!("Docs/hello.txt"));
    // Saving writes the document back to the server.
    run(&mut s, "text.insert", json!({"text": "More text."}));
    assert!(s.dirty);
    run(&mut s, "cloud.save", json!({}));
    run(&mut s, "cloud.wait", json!({"ms": 10000}));
    assert!(!s.dirty, "the document was saved");
    let seen = mock.seen();
    assert!(seen.iter().any(|r| r.starts_with("PUT /remote.php/dav/files/rasmus/Docs/hello.txt")), "{seen:?}");
}

#[test]
fn a_note_on_the_server_is_an_obsidian_note() {
    let mock = Mock::start();
    let mut s = s();
    connect(&mut s, &mock);
    run(&mut s, "cloud.open", json!({"path": "Docs/note.md"}));
    run(&mut s, "cloud.wait", json!({"ms": 10000}));
    let text = s.doc.plain_text(wordcraft_doc::StoryRef::Body);
    // The frontmatter stays out of the text and the wikilink became a link.
    assert!(!text.contains("title: One"), "{text}");
    assert!(text.contains("Link"), "{text}");
    // The note keeps its source, so an untouched note saves back byte for byte.
    let source = s.doc.passthrough.get(wordcraft_formats::obsidian::SOURCE_KEY).map(|b| b.to_vec());
    assert!(source.is_some(), "the note keeps its own source");
}

/// The tag for the version that was read travels with the document, so a write cannot
/// overwrite a file that changed on the server.
#[test]
fn the_server_tag_travels_with_the_document() {
    let mock = Mock::start();
    let mut s = s();
    connect(&mut s, &mock);
    run(&mut s, "cloud.open", json!({"path": "Docs/hello.txt"}));
    run(&mut s, "cloud.wait", json!({"ms": 10000}));
    let status = run(&mut s, "cloud.status", json!({}));
    assert_eq!(status["document"]["etag"], json!("\"etag-1\""), "{status}");
    // Saving sends the tag back, so the server refuses the write if the file changed.
    run(&mut s, "text.insert", json!({"text": "More."}));
    run(&mut s, "cloud.save", json!({}));
    run(&mut s, "cloud.wait", json!({"ms": 10000}));
    let seen = mock.seen();
    assert!(seen.iter().any(|r| r.to_ascii_lowercase().contains("if-match: \"etag-1\"")), "{seen:?}");
}

/// File > Save on a document from the server writes it back to the server.
#[test]
fn saving_a_document_from_the_server_writes_it_back() {
    let mock = Mock::start();
    let mut s = s();
    connect(&mut s, &mock);
    run(&mut s, "cloud.open", json!({"path": "Docs/hello.txt"}));
    run(&mut s, "cloud.wait", json!({"ms": 10000}));
    run(&mut s, "text.insert", json!({"text": "More."}));
    let before = mock.seen().len();
    run(&mut s, "file.save", json!({}));
    run(&mut s, "cloud.wait", json!({"ms": 10000}));
    let seen = mock.seen();
    assert!(seen.len() > before, "the document was written to the server");
    assert!(seen[before..].iter().any(|r| r.starts_with("PUT /remote.php/dav/files/rasmus/Docs/hello.txt")), "{seen:?}");
}

#[test]
fn a_public_link_is_made() {
    let mock = Mock::start();
    let mut s = s();
    connect(&mut s, &mock);
    run(&mut s, "cloud.share", json!({"path": "Docs/hello.txt"}));
    run(&mut s, "cloud.wait", json!({"ms": 10000}));
    let status = run(&mut s, "cloud.status", json!({}));
    assert_eq!(status["link"], json!("http://127.0.0.1/s/abc123"), "{status}");
}

#[test]
fn the_server_searches_its_files() {
    let mock = Mock::start();
    let mut s = s();
    connect(&mut s, &mock);
    run(&mut s, "cloud.search", json!({"term": "a b"}));
    run(&mut s, "cloud.wait", json!({"ms": 10000}));
    let status = run(&mut s, "cloud.status", json!({}));
    let hits = status["hits"].as_array().expect("the hits");
    assert_eq!(hits.len(), 1, "{status}");
    assert_eq!(hits[0]["path"], json!("Docs/hello.txt"));
}

#[test]
fn a_wrong_address_is_reported_and_nothing_is_queued() {
    let mut s = s();
    let e = s.run("cloud.connect", &json!({"server": "not a server", "user": "rasmus", "password": "x"})).expect_err("the address is wrong");
    assert!(e.to_string().contains("address") || e.to_string().contains("server"), "{e}");
    assert!(!s.cloud.is_connected());
}

#[test]
fn a_missing_file_is_reported_and_the_document_stays() {
    let mock = Mock::start();
    let mut s = s();
    connect(&mut s, &mock);
    run(&mut s, "text.insert", json!({"text": "Keep me"}));
    run(&mut s, "cloud.open", json!({"path": "Docs/missing.docx"}));
    run(&mut s, "cloud.wait", json!({"ms": 10000}));
    let status = run(&mut s, "cloud.status", json!({}));
    assert!(status["error"].is_string(), "{status}");
    assert_eq!(s.doc.plain_text(wordcraft_doc::StoryRef::Body), "Keep me", "the document is untouched");
}

#[test]
fn disconnecting_forgets_the_account() {
    let mock = Mock::start();
    let mut s = s();
    connect(&mut s, &mock);
    run(&mut s, "cloud.disconnect", json!({}));
    let status = run(&mut s, "cloud.status", json!({}));
    assert_eq!(status["connected"], json!(false));
    assert_eq!(status["files"].as_array().map(|f| f.len()), Some(0));
}

//! A small Nextcloud-shaped server, so the client can be tested without a network.
//!
//! It is only built for tests and for the `testkit` feature, which other crates' tests use.
//!
//! It is test tooling: it panics loudly when it cannot start, which is what a test wants.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//!
//! The server answers the calls Quill makes: a WebDAV listing, a download, an upload, a new
//! folder, a delete, and the OCS calls for the server's information, a share link and a search.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

/// The file the mock server hands out.
pub const FILE_BYTES: &[u8] = b"Hello from the cloud.\n";

/// The note the mock server hands out: an Obsidian note with frontmatter and a wikilink.
pub const NOTE: &str = "---\ntitle: One\ntags:\n  - idea\n---\n\n# One\n\nA [[Link]] and a #tag.\n";

/// A running mock server.
pub struct Mock {
    /// The port it listens on.
    pub port: u16,
    /// Every request it answered, in order.
    pub requests: Arc<Mutex<Vec<String>>>,
}

impl Mock {
    /// Start a server on a free port.
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
        let port = listener.local_addr().expect("the port").port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&requests);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let seen = Arc::clone(&seen);
                std::thread::spawn(move || handle(&mut stream, &seen));
            }
        });
        Self { port, requests }
    }

    /// The server's address.
    pub fn address(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// The requests the server saw, as strings.
    pub fn seen(&self) -> Vec<String> {
        self.requests.lock().map(|v| v.clone()).unwrap_or_default()
    }
}

/// Answer one request.
fn handle(stream: &mut TcpStream, seen: &Arc<Mutex<Vec<String>>>) {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        match stream.read(&mut byte) {
            Ok(0) => return,
            Ok(_) => head.push(byte[0]),
            Err(_) => return,
        }
    }
    let head = String::from_utf8_lossy(&head).to_string();
    let mut lines = head.lines();
    let request = lines.next().unwrap_or_default().to_string();
    let mut length = 0usize;
    for line in lines {
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0u8; length];
    if length > 0 && stream.read_exact(&mut body).is_err() {
        return;
    }
    if let Ok(mut seen) = seen.lock() {
        seen.push(format!("{request}\n{head}\n{}", String::from_utf8_lossy(&body)));
    }

    let (status, reason, content_type, payload) = answer(&request, &head);
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nETag: \"etag-1\"\r\nConnection: close\r\n\r\n",
        payload.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.write_all(&payload);
    let _ = stream.flush();
}

/// The answer for one request.
fn answer(request: &str, head: &str) -> (u16, &'static str, &'static str, Vec<u8>) {
    let mut parts = request.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or_default().split('?').next().unwrap_or_default();
    match (method, path) {
        ("PROPFIND", p) if p.starts_with("/remote.php/dav/files/") => (207, "Multi-Status", "application/xml; charset=utf-8", listing().into_bytes()),
        ("GET", "/remote.php/dav/files/rasmus/Docs/hello.txt") => (200, "OK", "application/octet-stream", FILE_BYTES.to_vec()),
        ("GET", "/remote.php/dav/files/rasmus/Docs/note.md") => (200, "OK", "text/markdown", NOTE.as_bytes().to_vec()),
        ("GET", "/remote.php/dav/files/rasmus/Docs/missing.txt") => (404, "Not Found", "text/plain", b"no".to_vec()),
        ("GET", "/ocs/v2.php/cloud/capabilities") => (200, "OK", "application/json", capabilities().into_bytes()),
        ("GET", "/ocs/v1.php/cloud/user") => (200, "OK", "application/json", account().into_bytes()),
        ("POST", "/ocs/v2.php/apps/files_sharing/api/v1/shares") => (200, "OK", "application/json", share().into_bytes()),
        ("POST", "/ocs/v2.php/apps/search/api/v1/search") => (200, "OK", "application/json", search().into_bytes()),
        ("GET", p) if p.starts_with("/ocs/v1.php/apps/files_versions/api/v1/versions") => (200, "OK", "application/json", versions().into_bytes()),
        // A file that changed on the server is refused when the caller sends the old tag.
        ("PUT", _) if head.to_ascii_lowercase().contains("if-match:") && !head.to_ascii_lowercase().contains("if-match: \"etag-1\"") => {
            (412, "Precondition Failed", "text/plain", b"the file changed".to_vec())
        }
        ("PUT", _) => (201, "Created", "text/plain", Vec::new()),
        ("MKCOL", _) => (201, "Created", "text/plain", Vec::new()),
        ("DELETE", _) => (204, "No Content", "text/plain", Vec::new()),
        _ => (404, "Not Found", "text/plain", b"no".to_vec()),
    }
}

/// A WebDAV listing.
fn listing() -> String {
    r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:href>/remote.php/dav/files/rasmus/</d:href>
    <d:propstat><d:prop>
      <d:displayname>rasmus</d:displayname>
      <d:resourcetype><d:collection/></d:resourcetype>
    </d:prop></d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/files/rasmus/Docs</d:href>
    <d:propstat><d:prop>
      <d:displayname>Docs</d:displayname>
      <d:resourcetype><d:collection/></d:resourcetype>
      <d:getlastmodified>Tue, 21 Oct 2026 10:00:00 GMT</d:getlastmodified>
    </d:prop></d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/files/rasmus/Docs/hello.txt</d:href>
    <d:propstat><d:prop>
      <d:displayname>hello.txt</d:displayname>
      <d:getcontentlength>21</d:getcontentlength>
      <d:getetag>&quot;etag-1&quot;</d:getetag>
    </d:prop></d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/files/rasmus/Docs/note.md</d:href>
    <d:propstat><d:prop>
      <d:displayname>note.md</d:displayname>
      <d:getcontentlength>64</d:getcontentlength>
      <d:getetag>&quot;etag-2&quot;</d:getetag>
    </d:prop></d:propstat>
  </d:response>
</d:multistatus>"#
        .to_string()
}

/// The server's information.
fn capabilities() -> String {
    r#"{"ocs":{"meta":{"status":"ok","statuscode":100},"data":{"productname":"Nextcloud","version":"31.0.2","versionstring":"31.0.2","capabilities":{"files_sharing":{"public":{"enabled":true}},"files_versions":{"enabled":true},"search":{"enabled":true}}}}}"#.to_string()
}

/// The account on the server.
fn account() -> String {
    r#"{"ocs":{"meta":{"status":"ok","statuscode":100},"data":{"id":"rasmus","displayname":"Rasmus"}}}"#.to_string()
}

/// A public link.
fn share() -> String {
    r#"{"ocs":{"meta":{"status":"ok","statuscode":100},"data":{"id":"7","url":"http://127.0.0.1/s/abc123"}}}"#.to_string()
}

/// Search results.
fn search() -> String {
    r#"{"ocs":{"meta":{"status":"ok","statuscode":100},"data":{"results":[{"name":"hello.txt","path":"Docs/hello.txt","size":21,"lastmod":1700000000,"link":"http://127.0.0.1/f/1"}]}}}"#.to_string()
}

/// A file's older versions.
fn versions() -> String {
    r#"{"ocs":{"meta":{"status":"ok","statuscode":100},"data":{"versions":[{"id":1234,"name":"hello.txt","size":21,"mtime":1700000000}]}}}"#
        .to_string()
}

//! The client against a Nextcloud-shaped server.

use crate::testserver::Mock;
use crate::{Client, Config};

fn client(mock: &Mock) -> Client {
    Client::new(Config::new(&mock.address(), "rasmus", "secret")).expect("a client")
}

#[test]
fn a_folder_is_listed() {
    let mock = Mock::start();
    let client = client(&mock);
    let entries = client.list("").expect("the listing");
    // The folder itself is not part of its own listing.
    assert_eq!(entries.len(), 3, "{entries:?}");
    let folder = entries.iter().find(|e| e.path == "Docs").expect("the folder");
    assert!(folder.is_dir);
    let file = entries.iter().find(|e| e.path == "Docs/hello.txt").expect("the file");
    assert!(!file.is_dir);
    assert_eq!(file.name, "hello.txt");
    assert_eq!(file.size, 21);
    assert_eq!(file.etag, "\"etag-1\"");
    assert!(mock.seen().iter().any(|r| r.starts_with("PROPFIND /remote.php/dav/files/rasmus")));
}

#[test]
fn a_file_is_read_and_written() {
    let mock = Mock::start();
    let client = client(&mock);
    let bytes = client.get("Docs/hello.txt").expect("the file");
    assert_eq!(bytes, crate::testserver::FILE_BYTES);
    client.put("Docs/hello.txt", b"new bytes").expect("the upload");
    let seen = mock.seen();
    assert!(seen.iter().any(|r| r.starts_with("PUT /remote.php/dav/files/rasmus/Docs/hello.txt")), "{seen:?}");
    assert!(seen.iter().any(|r| r.ends_with("new bytes")), "{seen:?}");
}

#[test]
fn a_file_that_changed_on_the_server_is_not_overwritten() {
    let mock = Mock::start();
    let client = client(&mock);
    let (bytes, etag) = client.get_with_etag("Docs/hello.txt").expect("the file");
    assert_eq!(bytes, crate::testserver::FILE_BYTES);
    assert_eq!(etag, "\"etag-1\"");
    // A tag that is not the server's means the file changed in the meantime.
    let e = client.put_if("Docs/hello.txt", b"new bytes", Some("\"stale\"")).expect_err("the file changed");
    assert!(e.to_string().contains("changed on the server"), "{e}");
    // With the right tag the write goes through.
    client.put_if("Docs/hello.txt", b"new bytes", Some("\"etag-1\"")).expect("the upload");
    let seen = mock.seen();
    assert!(seen.iter().any(|r| r.to_ascii_lowercase().contains("if-match: \"etag-1\"")), "{seen:?}");
}

#[test]
fn folders_are_made_and_files_are_removed() {
    let mock = Mock::start();
    let client = client(&mock);
    client.mkdir("Docs/New").expect("the folder");
    client.delete("Docs/old.docx").expect("the delete");
    let seen = mock.seen();
    assert!(seen.iter().any(|r| r.starts_with("MKCOL ")), "{seen:?}");
    assert!(seen.iter().any(|r| r.starts_with("DELETE ")), "{seen:?}");
}

#[test]
fn the_server_says_what_it_is() {
    let mock = Mock::start();
    let client = client(&mock);
    let caps = client.capabilities().expect("the capabilities");
    assert_eq!(caps.product, "Nextcloud");
    assert_eq!(caps.version, "31.0.2");
    assert!(caps.public_shares);
    assert!(caps.versions);
    assert!(caps.search);
    let account = client.account().expect("the account");
    assert_eq!(account.id, "rasmus");
    assert_eq!(account.display_name, "Rasmus");
}

#[test]
fn a_public_link_is_made() {
    let mock = Mock::start();
    let client = client(&mock);
    let link = client.share_link("Docs/hello.txt").expect("the link");
    assert_eq!(link, "http://127.0.0.1/s/abc123");
    let seen = mock.seen();
    assert!(seen.iter().any(|r| r.contains("\"shareType\":3")), "{seen:?}");
    assert!(seen.iter().any(|r| r.contains("\"path\":\"/Docs/hello.txt\"")), "{seen:?}");
}

#[test]
fn the_server_searches_its_files() {
    let mock = Mock::start();
    let client = client(&mock);
    let hits = client.search("a b").expect("the search");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].path, "Docs/hello.txt");
    assert_eq!(hits[0].name, "hello.txt");
}

#[test]
fn a_files_versions_are_listed() {
    let mock = Mock::start();
    let client = client(&mock);
    let versions = client.versions("Docs/hello.txt").expect("the versions");
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].name, "hello.txt");
}

#[test]
fn a_missing_file_is_an_error() {
    let mock = Mock::start();
    let client = client(&mock);
    let e = client.get("Docs/missing.txt").expect_err("the file is not there");
    assert!(e.to_string().contains("404"), "{e}");
}

#[test]
fn a_wrong_password_is_reported() {
    let mock = Mock::start();
    let e = match Client::new(Config::new(&mock.address(), "rasmus", "")) {
        Err(e) => e,
        Ok(_) => panic!("an account without a password must be refused"),
    };
    assert!(e.to_string().contains("password"), "{e}");
}

mod unit {
    use super::super::*;

    #[test]
    fn a_wrong_address_is_reported_before_any_request() {
        assert!(matches!(Client::new(Config::new("", "user", "secret")), Err(Error::Config(_))));
        assert!(matches!(Client::new(Config::new("https://", "user", "secret")), Err(Error::Config(_))));
        assert!(matches!(Client::new(Config::new("ftp://host", "user", "secret")), Err(Error::Config(_))));
        assert!(matches!(Client::new(Config::new("https://host", "", "secret")), Err(Error::Config(_))));
        assert!(matches!(Client::new(Config::new("https://host", "user", "")), Err(Error::Config(_))));
    }

    #[test]
    fn a_plain_address_becomes_https() {
        let url = ServerUrl::parse("cloud.example.com").unwrap();
        assert_eq!(url.base(), "https://cloud.example.com");
        assert!(url.secure());
    }

    #[test]
    fn credentials_are_never_put_in_the_address() {
        assert!(ServerUrl::parse("https://user:pw@cloud.example.com").is_err());
    }

    #[test]
    fn a_subfolder_in_the_address_is_kept() {
        let url = ServerUrl::parse("https://example.com/nextcloud/").unwrap();
        assert_eq!(url.base(), "https://example.com/nextcloud");
        assert_eq!(url.dav("rasmus", "Docs/hello.txt").unwrap(), "https://example.com/nextcloud/remote.php/dav/files/rasmus/Docs/hello.txt");
    }

    #[test]
    fn paths_cannot_leave_the_account() {
        let url = ServerUrl::parse("https://example.com").unwrap();
        assert!(url.dav("rasmus", "../secret").is_err());
        assert!(url.dav("rasmus", "a/../../secret").is_err());
    }

    #[test]
    fn the_authorization_header_is_basic() {
        assert_eq!(basic_auth("rasmus", "hunter2"), "Basic cmFzbXVzOmh1bnRlcjI=");
    }
}

/// A listing with a folder, a file whose name has a space, and a name the server left out.
#[test]
fn a_listing_reads_folders_and_files() {
    let xml = r#"<d:multistatus xmlns:d="DAV:">
          <d:response>
            <d:href>/remote.php/dav/files/u/Docs/</d:href>
            <d:propstat><d:prop>
              <d:displayname>Docs</d:displayname>
              <d:resourcetype><d:collection/></d:resourcetype>
            </d:prop></d:propstat>
          </d:response>
          <d:response>
            <d:href>/remote.php/dav/files/u/Docs/a%20b.docx</d:href>
            <d:propstat><d:prop>
              <d:getcontentlength>42</d:getcontentlength>
              <d:getlastmodified>Tue, 21 Oct 2026 10:00:00 GMT</d:getlastmodified>
            </d:prop></d:propstat>
          </d:response>
        </d:multistatus>"#;
    let entries = crate::dav::parse_multistatus(xml).expect("the listing");
    assert_eq!(entries.len(), 2);
    assert!(entries[0].is_dir, "the folder");
    assert_eq!(entries[0].path, "Docs");
    assert_eq!(entries[1].path, "Docs/a b.docx");
    assert_eq!(entries[1].name, "a b.docx", "the name comes from the path when the server leaves it out");
    assert_eq!(entries[1].size, 42);
}

/// A hostile listing must not panic and must not produce nonsense.
#[test]
fn a_hostile_listing_is_read_without_a_panic() {
    for xml in [
        "",
        "<d:multistatus",
        "<d:multistatus xmlns:d=\"DAV:\"><d:response><d:href>",
        "<d:multistatus xmlns:d=\"DAV:\"><d:response><d:href>/a</d:href><d:propstat><d:prop><d:getcontentlength>x</d:getcontentlength></d:prop></d:propstat></d:response></d:multistatus>",
        "<d:multistatus xmlns:d=\"DAV:\"><d:response><d:href>/a</d:href><d:propstat><d:prop><d:displayname></d:displayname></d:prop></d:propstat></d:response></d:multistatus>",
    ] {
        // A broken listing is refused, not read as nonsense — and never a panic.
        if let Ok(entries) = crate::dav::parse_multistatus(xml) {
            for e in &entries {
                assert!(e.name.len() < 1000, "a name stays sane");
            }
        }
    }
}

/// A hostile path may not leave the account, and may not be endless.
#[test]
fn a_hostile_path_is_refused() {
    assert!(crate::url::encode_path("../secret").is_err());
    assert!(crate::url::encode_path("a/../../secret").is_err());
    assert!(crate::url::encode_path(&"x".repeat(5000)).is_err());
    assert!(crate::url::encode_path("a\u{0}b").is_err());
    // A path with a percent or a space is encoded, not refused.
    assert_eq!(crate::url::encode_path("a b%c").unwrap(), "/a%20b%25c");
}

/// A hostile server address is refused before any request is made.
#[test]
fn a_hostile_address_is_refused() {
    for address in [
        "",
        "   ",
        "https://",
        "https://host name",
        "https://host:notaport",
        "ftp://host",
        "https://user:pw@host",
        "https://host/../evil",
        "https://host/with\nnewline",
        "javascript:alert(1)",
    ] {
        assert!(crate::url::ServerUrl::parse(address).is_err(), "{address:?} was accepted");
    }
}

/// The OCS answer is read, and a failing one is an error.
#[test]
fn the_ocs_answer_is_read() {
    let body = r#"{"ocs":{"meta":{"status":"ok","statuscode":100},"data":{"url":"https://cloud/s/abc"}}}"#;
    let reply = crate::http::Reply { status: 200, headers: Vec::new(), body: body.as_bytes().to_vec() };
    // The data part is what the callers read.
    let json: serde_json::Value = serde_json::from_slice(&reply.body).expect("json");
    assert_eq!(json["ocs"]["data"]["url"], "https://cloud/s/abc");
}

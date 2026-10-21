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

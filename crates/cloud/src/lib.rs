//! Cloud files: a Nextcloud/WebDAV client for the files a server keeps.
//!
//! Quill talks to a Nextcloud server the way a desktop client does: files are listed over
//! WebDAV ([`dav`]), the server's own information, public share links, full-text search and
//! file versions come from the OCS API ([`ocs`]). The client is deliberately small and bounded:
//! every answer is capped, every path is checked, and every request has a timeout.
//!
//! Two ways to use it:
//!
//! * [`Client`] blocks and is meant for scripts, the CLI and the MCP server.
//! * [`Worker`] runs the same requests on a background thread, so the editor never waits for a
//!   server. The UI polls [`Worker::try_recv`] once per frame.
//!
//! The HTTP client itself is desktop-only. The browser build answers "not available here"
//! instead of pretending to be connected.
//!
//! Written from Nextcloud's public API documentation; no Nextcloud code is used.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod dav;
mod http;
mod ocs;
mod secret;
#[cfg(any(test, feature = "testkit"))]
pub mod testserver;
mod url;
pub mod worker;

#[cfg(test)]
mod tests;

pub use secret::{Secret, SecretStore};
pub use worker::{Answer, Job, JobKind, Outcome, Worker};

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::url::ServerUrl;

/// Largest listing one request may read (a folder with a hundred thousand files).
pub const MAX_LIST_BYTES: u64 = 16 * 1024 * 1024;
/// Most entries one listing may hold.
pub const MAX_ENTRIES: usize = 50_000;
/// Largest file one download may read.
pub const MAX_DOWNLOAD_BYTES: u64 = 512 * 1024 * 1024;
/// Largest body one request may send.
pub const MAX_UPLOAD_BYTES: u64 = 512 * 1024 * 1024;
/// Longest a server address may be.
pub const MAX_SERVER_CHARS: usize = 512;
/// Longest a remote path may be.
pub const MAX_PATH_CHARS: usize = 4096;

/// Something went wrong talking to the server.
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum Error {
    /// The settings are wrong (address, user name, path).
    #[error("{0}")]
    Config(String),
    /// The server could not be reached.
    #[error("{0}")]
    Network(String),
    /// The server answered with an error.
    #[error("the server answered {status}: {message}")]
    Status { status: u16, message: String },
    /// The server's answer could not be read.
    #[error("{0}")]
    Protocol(String),
    /// Reading or writing a local file failed.
    #[error("{0}")]
    Io(String),
    /// This build cannot do it.
    #[error("{0}")]
    Unsupported(&'static str),
}

/// How to reach a server.
///
/// The password is a Nextcloud *app password* (Settings › Security › Devices): it can be
/// revoked from the browser and grants access to this app only.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// The server's address, e.g. `https://cloud.example.com` (or with a subfolder).
    pub server: String,
    /// The account's user name.
    pub user: String,
    /// The app password.
    pub password: Zeroizing<String>,
    /// Skip the server's TLS certificate check (self-signed servers). Off by default.
    pub insecure: bool,
    /// A PEM certificate to trust instead of the public roots.
    pub ca_pem: Option<Zeroizing<String>>,
}

impl Config {
    /// A configuration for `server` and `user`.
    pub fn new(server: &str, user: &str, password: &str) -> Self {
        Self {
            server: server.trim().to_string(),
            user: user.trim().to_string(),
            password: Zeroizing::new(password.to_string()),
            insecure: false,
            ca_pem: None,
        }
    }

    /// Trust this certificate instead of the public roots.
    pub fn with_ca_pem(mut self, pem: &str) -> Self {
        self.ca_pem = Some(Zeroizing::new(pem.to_string()));
        self
    }

    /// Accept a certificate the server cannot prove (self-signed servers).
    pub fn with_insecure(mut self, insecure: bool) -> Self {
        self.insecure = insecure;
        self
    }
}

/// One file or folder on the server.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// The path relative to the account's files, without a leading slash (`""` is the root).
    pub path: String,
    /// The name of the file or folder.
    pub name: String,
    /// Is it a folder?
    pub is_dir: bool,
    /// Its size in bytes (folders report 0).
    pub size: u64,
    /// When it changed, as the server wrote it.
    pub modified: String,
    /// The server's tag for this version of the file.
    pub etag: String,
}

/// What the server says about itself.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    /// The product's name ("Nextcloud").
    pub product: String,
    /// The server's version.
    pub version: String,
    /// Can the account make public links?
    pub public_shares: bool,
    /// Does the server keep file versions?
    pub versions: bool,
    /// Can the server search file contents?
    pub search: bool,
}

/// The account on the server.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// The user name.
    pub id: String,
    /// The name to show.
    pub display_name: String,
}

/// One hit from the server's search.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    /// The file's path relative to the account's files.
    pub path: String,
    /// The file's name.
    pub name: String,
    /// Its size in bytes.
    pub size: u64,
    /// When it changed.
    pub modified: String,
    /// A link to the file on the server.
    pub link: String,
}

/// One older version of a file.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    /// The server's id for this version.
    pub id: String,
    /// The file's name.
    pub name: String,
    /// Its size in bytes.
    pub size: u64,
    /// When it was stored.
    pub modified: String,
}

/// A connection to one server.
pub struct Client {
    base: ServerUrl,
    user: String,
    http: http::Http,
}

impl Client {
    /// Open a connection. The settings are checked here, so a wrong address is reported before
    /// any request is made.
    pub fn new(config: Config) -> Result<Self, Error> {
        #[cfg(any(target_arch = "wasm32", feature = "wasm-check"))]
        {
            // The address is checked even here, so a wrong one is reported the same way.
            ServerUrl::parse(&config.server)?;
            let _ = http::Http::new(basic_auth(&config.user, &config.password), config.insecure, None)?;
            return Err(Error::Unsupported("cloud files need the desktop app"));
        }
        #[cfg(all(not(target_arch = "wasm32"), not(feature = "wasm-check")))]
        {
            let base = ServerUrl::parse(&config.server)?;
            let user = config.user.trim().to_string();
            if user.is_empty() {
                return Err(Error::Config("the user name is empty".into()));
            }
            if user.len() > 128 {
                return Err(Error::Config("the user name is too long".into()));
            }
            if config.password.is_empty() {
                return Err(Error::Config("the app password is empty".into()));
            }
            let auth = basic_auth(&user, &config.password);
            let http = http::Http::new(auth, config.insecure, config.ca_pem.as_deref().map(String::as_str))?;
            Ok(Self { base, user, http })
        }
    }

    /// The server's address, without a trailing slash.
    pub fn server(&self) -> &str {
        self.base.base()
    }

    /// The account's user name.
    pub fn user(&self) -> &str {
        &self.user
    }

    /// Is the connection over TLS?
    pub fn secure(&self) -> bool {
        self.base.secure()
    }

    /// What the server says about itself.
    pub fn capabilities(&self) -> Result<Capabilities, Error> {
        ocs::capabilities(&self.http, &self.base)
    }

    /// The account on the server.
    pub fn account(&self) -> Result<Account, Error> {
        ocs::account(&self.http, &self.base)
    }

    /// The files and folders in `path` (the account's root when the path is empty).
    pub fn list(&self, path: &str) -> Result<Vec<Entry>, Error> {
        let url = self.base.dav(&self.user, path)?;
        let reply = self.http.request(
            "PROPFIND",
            &url,
            &[("Depth", "1"), ("Content-Type", "application/xml; charset=utf-8")],
            Some(dav::PROPFIND_BODY.as_bytes()),
            MAX_LIST_BYTES,
        )?;
        if reply.status == 404 {
            return Err(Error::Status { status: 404, message: "there is no such folder".into() });
        }
        expect_ok(&reply)?;
        let mut entries = dav::parse_multistatus(&String::from_utf8_lossy(&reply.body))?;
        // The server answers with the folder itself first; the listing is about its contents.
        let wanted = url::normalise(path);
        entries.retain(|e| url::normalise(&e.path) != wanted);
        entries.truncate(MAX_ENTRIES);
        Ok(entries)
    }

    /// The file's bytes.
    pub fn get(&self, path: &str) -> Result<Vec<u8>, Error> {
        self.get_with_etag(path).map(|(bytes, _)| bytes)
    }

    /// The file's bytes and the server's tag for this version of it.
    ///
    /// The tag is what [`Client::put_if`] sends back, so a file that changed on the server in
    /// the meantime is not overwritten silently.
    pub fn get_with_etag(&self, path: &str) -> Result<(Vec<u8>, String), Error> {
        let url = self.base.dav(&self.user, path)?;
        let reply = self.http.request("GET", &url, &[], None, MAX_DOWNLOAD_BYTES)?;
        expect_ok(&reply)?;
        let etag = reply.header("ETag").unwrap_or_default().to_string();
        Ok((reply.body, etag))
    }

    /// Write the file, replacing what is there.
    pub fn put(&self, path: &str, bytes: &[u8]) -> Result<(), Error> {
        self.put_if(path, bytes, None).map(|_| ())
    }

    /// Write the file, but only if it is still the version that was read (`etag`).
    ///
    /// The server answers "the file changed on the server" when it is not, so a document is
    /// never quietly written over someone else's work.
    pub fn put_if(&self, path: &str, bytes: &[u8], etag: Option<&str>) -> Result<String, Error> {
        if bytes.len() as u64 > MAX_UPLOAD_BYTES {
            return Err(Error::Config("the file is too large to upload".into()));
        }
        let url = self.base.dav(&self.user, path)?;
        let mut headers: Vec<(&str, &str)> = vec![("Content-Type", "application/octet-stream")];
        if let Some(tag) = etag.filter(|t| !t.trim().is_empty()) {
            headers.push(("If-Match", tag));
        }
        let reply = self.http.request("PUT", &url, &headers, Some(bytes), 1024 * 1024)?;
        if reply.status == 412 {
            return Err(Error::Status { status: 412, message: "the file changed on the server".into() });
        }
        expect_ok(&reply)?;
        Ok(reply.header("ETag").unwrap_or_default().to_string())
    }

    /// Create a folder.
    pub fn mkdir(&self, path: &str) -> Result<(), Error> {
        let url = self.base.dav(&self.user, path)?;
        let reply = self.http.request("MKCOL", &url, &[], None, 64 * 1024)?;
        if reply.status == 405 {
            return Err(Error::Status { status: 405, message: "the folder is already there".into() });
        }
        expect_ok(&reply)?;
        Ok(())
    }

    /// Delete a file or folder.
    pub fn delete(&self, path: &str) -> Result<(), Error> {
        let url = self.base.dav(&self.user, path)?;
        let reply = self.http.request("DELETE", &url, &[], None, 64 * 1024)?;
        expect_ok(&reply)?;
        Ok(())
    }

    /// A public link to the file, created on the server.
    pub fn share_link(&self, path: &str) -> Result<String, Error> {
        ocs::share_link(&self.http, &self.base, path)
    }

    /// The server's own search over the account's files.
    pub fn search(&self, term: &str) -> Result<Vec<SearchHit>, Error> {
        ocs::search(&self.http, &self.base, term)
    }

    /// The older versions the server keeps for a file.
    pub fn versions(&self, path: &str) -> Result<Vec<Version>, Error> {
        ocs::versions(&self.http, &self.base, &self.user, path)
    }
}

/// The `Authorization` header for a user name and password.
fn basic_auth(user: &str, password: &str) -> String {
    use base64::Engine;
    let raw = format!("{user}:{password}");
    format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(raw))
}

/// Turn an answer that is not a success into an error.
fn expect_ok(reply: &http::Reply) -> Result<(), Error> {
    if (200..300).contains(&reply.status) {
        return Ok(());
    }
    let text = String::from_utf8_lossy(&reply.body);
    let message = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let message = if message.is_empty() { reply.status_text().to_string() } else { message.chars().take(200).collect() };
    Err(Error::Status { status: reply.status, message })
}

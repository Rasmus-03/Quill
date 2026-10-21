//! The server's address and the paths inside it.
//!
//! Everything that reaches a URL goes through here: the address is checked once, and every path
//! is checked and percent-encoded on the way. A path may not leave the account's own files
//! (`..` is refused), may not carry control characters and may not be endless.

use percent_encoding::{AsciiSet, CONTROLS, percent_decode_str, utf8_percent_encode};

use crate::{Error, MAX_PATH_CHARS, MAX_SERVER_CHARS};

/// The characters that are written as `%XX` inside a path segment.
const SEGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}')
    .add(b'/')
    .add(b'\\')
    .add(b'^')
    .add(b'|')
    .add(b'[')
    .add(b']')
    .add(b'&')
    .add(b';')
    .add(b'=');

/// A checked server address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerUrl {
    /// The address without a trailing slash, e.g. `https://example.com/nextcloud`.
    base: String,
    /// Is it over TLS?
    secure: bool,
}

impl ServerUrl {
    /// Check an address the user typed. A missing scheme means HTTPS.
    pub fn parse(input: &str) -> Result<Self, Error> {
        let raw = input.trim();
        if raw.is_empty() {
            return Err(Error::Config("the server address is empty".into()));
        }
        if raw.len() > MAX_SERVER_CHARS {
            return Err(Error::Config("the server address is too long".into()));
        }
        if raw.chars().any(|c| c.is_control()) {
            return Err(Error::Config("the server address holds a control character".into()));
        }
        let with_scheme = if raw.contains("://") { raw.to_string() } else { format!("https://{raw}") };
        let (scheme, rest) = with_scheme.split_once("://").ok_or_else(|| Error::Config("the server address is not an address".into()))?;
        let secure = match scheme.to_ascii_lowercase().as_str() {
            "https" => true,
            "http" => false,
            other => return Err(Error::Config(format!("`{other}` is not a server address; use https"))),
        };
        let (authority, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, ""),
        };
        if !valid_authority(authority) {
            return Err(Error::Config(format!("`{authority}` is not a server address")));
        }
        let base_path = path.trim_end_matches('/');
        for segment in base_path.split('/').filter(|s| !s.is_empty()) {
            if segment == "." || segment == ".." {
                return Err(Error::Config("the server address holds a `..`".into()));
            }
        }
        let base = format!("{scheme}://{authority}{base_path}");
        Ok(Self { base, secure })
    }

    /// The address without a trailing slash.
    pub fn base(&self) -> &str {
        &self.base
    }

    /// Is the address over TLS?
    pub fn secure(&self) -> bool {
        self.secure
    }

    /// The WebDAV address of a file or folder in the account.
    pub fn dav(&self, user: &str, path: &str) -> Result<String, Error> {
        let user = encode_segment(user)?;
        let path = encode_path(path)?;
        Ok(format!("{}/remote.php/dav/files/{user}{path}", self.base))
    }

    /// The address of an OCS endpoint, e.g. `ocs/v2.php/cloud/capabilities`.
    pub fn ocs(&self, rest: &str) -> String {
        format!("{}/ocs/{rest}", self.base)
    }
}

/// Check `host[:port]`, including an IPv6 literal in brackets.
fn valid_authority(authority: &str) -> bool {
    if authority.is_empty() || authority.len() > 256 {
        return false;
    }
    if let Some(rest) = authority.strip_prefix('[') {
        let Some(end) = rest.find(']') else { return false };
        let host = &rest[..end];
        if host.is_empty() || !host.chars().all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.') {
            return false;
        }
        let tail = &rest[end + 1..];
        return tail.is_empty() || valid_port(tail.strip_prefix(':').unwrap_or(""));
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (authority, None),
    };
    if host.is_empty() || !host.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.') {
        return false;
    }
    match port {
        Some(p) => valid_port(p),
        None => true,
    }
}

/// A port is digits only.
fn valid_port(port: &str) -> bool {
    !port.is_empty() && port.len() <= 5 && port.chars().all(|c| c.is_ascii_digit())
}

/// Percent-encode one path segment.
fn encode_segment(segment: &str) -> Result<String, Error> {
    if segment.is_empty() {
        return Err(Error::Config("the path holds an empty name".into()));
    }
    if segment.len() > 255 {
        return Err(Error::Config("a name in the path is too long".into()));
    }
    if segment.chars().any(|c| c.is_control()) {
        return Err(Error::Config("a name in the path holds a control character".into()));
    }
    Ok(utf8_percent_encode(segment, SEGMENT).to_string())
}

/// Percent-encode a path inside the account. The path may not leave it.
pub fn encode_path(path: &str) -> Result<String, Error> {
    if path.len() > MAX_PATH_CHARS {
        return Err(Error::Config("the path is too long".into()));
    }
    let mut out = String::new();
    for segment in path.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            return Err(Error::Config("the path leaves the account's files".into()));
        }
        out.push('/');
        out.push_str(&encode_segment(segment)?);
    }
    Ok(out)
}

/// A path without its leading and trailing slashes.
pub fn normalise(path: &str) -> String {
    path.trim_matches('/').to_string()
}

/// The path inside the account that a WebDAV address points at.
///
/// The server answers with the whole address of the file, sometimes absolute and sometimes
/// percent-encoded; both are understood.
pub fn path_from_href(href: &str) -> Option<String> {
    let href = href.split(['?', '#']).next().unwrap_or("");
    let marker = "/remote.php/dav/files/";
    let start = href.find(marker)? + marker.len();
    let rest = &href[start..];
    // The first segment is the account's name; the rest is the path.
    let rest = match rest.find('/') {
        Some(i) => &rest[i..],
        None => "",
    };
    let decoded = percent_decode_str(rest).decode_utf8_lossy().to_string();
    Some(normalise(&decoded))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_checked() {
        assert!(ServerUrl::parse("").is_err());
        assert!(ServerUrl::parse("   ").is_err());
        assert!(ServerUrl::parse("https://").is_err());
        assert!(ServerUrl::parse("https://host/../evil").is_err());
        assert!(ServerUrl::parse("https://host name").is_err());
        assert!(ServerUrl::parse("https://host:notaport").is_err());
        assert!(ServerUrl::parse("javascript:alert(1)").is_err());
        assert!(ServerUrl::parse("https://ho st").is_err());
    }

    #[test]
    fn a_good_address_is_kept() {
        assert_eq!(ServerUrl::parse("https://example.com").unwrap().base(), "https://example.com");
        assert_eq!(ServerUrl::parse("http://example.com:8080/next").unwrap().base(), "http://example.com:8080/next");
        assert_eq!(ServerUrl::parse("https://[2001:db8::1]:8443").unwrap().base(), "https://[2001:db8::1]:8443");
        assert!(!ServerUrl::parse("http://example.com").unwrap().secure());
    }

    #[test]
    fn paths_are_encoded_and_checked() {
        assert_eq!(encode_path("Docs/a b.docx").unwrap(), "/Docs/a%20b.docx");
        assert_eq!(encode_path("/Docs//a.docx").unwrap(), "/Docs/a.docx");
        assert_eq!(encode_path("").unwrap(), "");
        assert!(encode_path("../secret").is_err());
        assert!(encode_path("a/../../secret").is_err());
        assert!(encode_path(&"x".repeat(MAX_PATH_CHARS + 1)).is_err());
    }

    #[test]
    fn hrefs_are_read_back() {
        assert_eq!(path_from_href("/remote.php/dav/files/rasmus/Docs/a%20b.docx").unwrap(), "Docs/a b.docx");
        assert_eq!(path_from_href("https://example.com/remote.php/dav/files/rasmus/").unwrap(), "");
        assert_eq!(path_from_href("/other/path"), None);
    }
}

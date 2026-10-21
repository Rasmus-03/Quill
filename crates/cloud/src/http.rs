//! The HTTP transport under the WebDAV and OCS calls.
//!
//! One small wrapper keeps the rest of the crate free of HTTP details: it adds the
//! authorization header, keeps the answers within a limit and turns everything that goes wrong
//! into an [`Error`]. The browser build has no sockets of its own and answers
//! [`Error::Unsupported`].

use crate::Error;

/// What the server answered.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reply {
    /// The HTTP status.
    pub status: u16,
    /// The headers, in the order the server sent them.
    pub headers: Vec<(String, String)>,
    /// The body.
    pub body: Vec<u8>,
}

impl Reply {
    /// The value of a header, matched without regard to case.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    /// A short name for the status, for an error message.
    pub fn status_text(&self) -> &'static str {
        match self.status {
            400 => "the request was refused",
            401 => "the user name or the app password is wrong",
            403 => "the account may not do that",
            404 => "there is nothing there",
            405 => "the server does not allow that",
            409 => "the file changed on the server",
            412 => "the file changed on the server",
            423 => "the file is locked",
            507 => "the server is out of space",
            _ => "the request failed",
        }
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(feature = "wasm-check")))]
mod desktop {
    use std::time::Duration;

    use super::{Error, Reply};

    /// The HTTP client.
    pub struct Http {
        agent: ureq::Agent,
        auth: String,
    }

    impl Http {
        pub fn new(auth: String, insecure: bool, ca_pem: Option<&str>) -> Result<Self, Error> {
            let tls = if let Some(pem) = ca_pem {
                let cert =
                    ureq::tls::Certificate::from_pem(pem.as_bytes()).map_err(|e| Error::Config(format!("the certificate could not be read: {e}")))?;
                ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::new_with_certs(&[cert])).build()
            } else if insecure {
                ureq::tls::TlsConfig::builder().disable_verification(true).build()
            } else {
                ureq::tls::TlsConfig::default()
            };
            let config = ureq::Agent::config_builder()
                .allow_non_standard_methods(true)
                .http_status_as_error(false)
                .timeout_global(Some(Duration::from_secs(60)))
                .timeout_connect(Some(Duration::from_secs(15)))
                .max_redirects(5)
                .tls_config(tls)
                .build();
            Ok(Self { agent: ureq::Agent::new_with_config(config), auth })
        }

        /// Send a request and read the answer, at most `max` bytes.
        pub fn request(&self, method: &str, url: &str, headers: &[(&str, &str)], body: Option<&[u8]>, max: u64) -> Result<Reply, Error> {
            let mut builder =
                ureq::http::Request::builder().method(method).uri(url).header("Authorization", &self.auth).header("User-Agent", "Quill/1.1");
            for (name, value) in headers {
                builder = builder.header(*name, *value);
            }
            let empty: &[u8] = &[];
            let request = builder.body(body.unwrap_or(empty)).map_err(|e| Error::Protocol(format!("the request could not be built: {e}")))?;
            let response = self.agent.run(request).map_err(|e| Error::Network(describe(&e)))?;
            let status = response.status().as_u16();
            let headers =
                response.headers().iter().map(|(name, value)| (name.as_str().to_string(), value.to_str().unwrap_or_default().to_string())).collect();
            let body = response
                .into_body()
                .into_with_config()
                .limit(max)
                .read_to_vec()
                .map_err(|e| Error::Protocol(format!("the answer could not be read: {e}")))?;
            Ok(Reply { status, headers, body })
        }
    }

    /// A short description of a transport error.
    fn describe(e: &ureq::Error) -> String {
        match e {
            ureq::Error::StatusCode(code) => format!("the server answered {code}"),
            ureq::Error::Timeout(_) => "the server did not answer in time".to_string(),
            ureq::Error::HostNotFound => "the server's address could not be found".to_string(),
            other => other.to_string(),
        }
    }
}

#[cfg(any(target_arch = "wasm32", feature = "wasm-check"))]
mod browser {
    use super::{Error, Reply};

    /// The browser build has no sockets of its own.
    pub struct Http;

    impl Http {
        pub fn new(_auth: String, _insecure: bool, _ca_pem: Option<&str>) -> Result<Self, Error> {
            Ok(Self)
        }

        pub fn request(&self, _method: &str, _url: &str, _headers: &[(&str, &str)], _body: Option<&[u8]>, _max: u64) -> Result<Reply, Error> {
            Err(Error::Unsupported("cloud files need the desktop app"))
        }
    }
}

#[cfg(any(target_arch = "wasm32", feature = "wasm-check"))]
pub use browser::Http;
#[cfg(all(not(target_arch = "wasm32"), not(feature = "wasm-check")))]
pub use desktop::Http;

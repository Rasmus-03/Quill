//! The OCS API: what the server says about itself, share links, search and file versions.
//!
//! Every OCS call needs the `OCS-APIRequest` header and answers with a small envelope:
//! `{"ocs":{"meta":…,"data":…}}`. The meta part says whether the call worked; the data part
//! holds the answer.

use serde_json::Value;

use crate::http::{Http, Reply};
use crate::url::{self, ServerUrl};
use crate::{Account, Capabilities, Error, SearchHit, Version};

/// Largest answer one OCS call may read.
pub const MAX_OCS_BYTES: u64 = 8 * 1024 * 1024;

/// The headers every OCS call carries.
const OCS_HEADERS: [(&str, &str); 2] = [("OCS-APIRequest", "true"), ("Accept", "application/json")];

/// What the server says about itself.
pub fn capabilities(http: &Http, base: &ServerUrl) -> Result<Capabilities, Error> {
    let address = format!("{}?format=json", base.ocs("v2.php/cloud/capabilities"));
    let reply = http.request("GET", &address, &OCS_HEADERS, None, MAX_OCS_BYTES)?;
    crate::expect_ok(&reply)?;
    let data = ocs_data(&reply)?;
    let caps = data.get("capabilities");
    let flag = |name: &str| caps.and_then(|c| c.get(name)).and_then(|f| f.get("enabled")).and_then(Value::as_bool).unwrap_or(false);
    Ok(Capabilities {
        product: data.get("productname").and_then(Value::as_str).unwrap_or("Nextcloud").to_string(),
        version: data
            .get("versionstring")
            .and_then(Value::as_str)
            .or_else(|| data.get("version").and_then(Value::as_str))
            .unwrap_or_default()
            .to_string(),
        public_shares: caps
            .and_then(|c| c.get("files_sharing"))
            .and_then(|f| f.get("public"))
            .and_then(|p| p.get("enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        versions: flag("files_versions"),
        search: flag("search"),
    })
}

/// The account on the server.
pub fn account(http: &Http, base: &ServerUrl) -> Result<Account, Error> {
    let address = format!("{}?format=json", base.ocs("v1.php/cloud/user"));
    let reply = http.request("GET", &address, &OCS_HEADERS, None, MAX_OCS_BYTES)?;
    crate::expect_ok(&reply)?;
    let data = ocs_data(&reply)?;
    Ok(Account {
        id: data.get("id").and_then(Value::as_str).unwrap_or_default().to_string(),
        display_name: data.get("displayname").and_then(Value::as_str).unwrap_or_default().to_string(),
    })
}

/// A public link to a file, created on the server.
pub fn share_link(http: &Http, base: &ServerUrl, path: &str) -> Result<String, Error> {
    let path = path.trim_start_matches('/');
    if path.is_empty() {
        return Err(Error::Config("there is no file to share".into()));
    }
    let address = format!("{}?format=json", base.ocs("v2.php/apps/files_sharing/api/v1/shares"));
    let body = serde_json::json!({ "path": format!("/{path}"), "shareType": 3, "permissions": 1 }).to_string();
    let reply = http.request("POST", &address, &OCS_HEADERS, Some(body.as_bytes()), MAX_OCS_BYTES)?;
    crate::expect_ok(&reply)?;
    let data = ocs_data(&reply)?;
    data.get("url").and_then(Value::as_str).map(str::to_string).ok_or_else(|| Error::Protocol("the server did not return a link".into()))
}

/// The server's own search over the account's files.
pub fn search(http: &Http, base: &ServerUrl, term: &str) -> Result<Vec<SearchHit>, Error> {
    let term = term.trim();
    if term.is_empty() {
        return Ok(Vec::new());
    }
    if term.len() > 512 {
        return Err(Error::Config("the search term is too long".into()));
    }
    let address = format!("{}?format=json", base.ocs("v2.php/apps/search/api/v1/search"));
    let body = serde_json::json!({ "term": term }).to_string();
    let reply = http.request("POST", &address, &OCS_HEADERS, Some(body.as_bytes()), MAX_OCS_BYTES)?;
    crate::expect_ok(&reply)?;
    let data = ocs_data(&reply)?;
    let results = data.get("results").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut hits = Vec::new();
    for item in results {
        let name = item.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
        let path = item.get("path").and_then(Value::as_str).unwrap_or_default().to_string();
        let path = url::normalise(path.trim_start_matches('/'));
        if name.is_empty() && path.is_empty() {
            continue;
        }
        hits.push(SearchHit {
            name: if name.is_empty() { path.rsplit('/').next().unwrap_or_default().to_string() } else { name },
            path,
            size: item.get("size").and_then(Value::as_u64).unwrap_or(0),
            modified: item.get("lastmod").map(Value::to_string).unwrap_or_default(),
            link: item.get("link").and_then(Value::as_str).unwrap_or_default().to_string(),
        });
        if hits.len() >= crate::MAX_ENTRIES {
            break;
        }
    }
    Ok(hits)
}

/// The older versions the server keeps for a file.
pub fn versions(http: &Http, base: &ServerUrl, user: &str, path: &str) -> Result<Vec<Version>, Error> {
    let path = url::encode_path(path)?;
    let user = url::encode_path(user)?;
    let address = format!("{}?format=json", base.ocs(&format!("v1.php/apps/files_versions/api/v1/versions{user}{path}")));
    let reply = http.request("GET", &address, &OCS_HEADERS, None, MAX_OCS_BYTES)?;
    crate::expect_ok(&reply)?;
    let data = ocs_data(&reply)?;
    let list = match &data {
        Value::Array(items) => items.clone(),
        other => other.get("versions").and_then(Value::as_array).cloned().unwrap_or_default(),
    };
    let mut versions = Vec::new();
    for item in list {
        versions.push(Version {
            id: item.get("id").map(Value::to_string).unwrap_or_default(),
            name: item.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
            size: item.get("size").and_then(Value::as_u64).unwrap_or(0),
            modified: item.get("mtime").map(Value::to_string).unwrap_or_default(),
        });
        if versions.len() >= 1000 {
            break;
        }
    }
    Ok(versions)
}

/// The data part of an OCS answer.
fn ocs_data(reply: &Reply) -> Result<Value, Error> {
    let text = String::from_utf8_lossy(&reply.body);
    let json: Value = serde_json::from_str(&text).map_err(|e| Error::Protocol(format!("the server's answer is not JSON: {e}")))?;
    let meta = json.get("ocs").and_then(|o| o.get("meta"));
    let code = meta.and_then(|m| m.get("statuscode")).and_then(Value::as_i64).unwrap_or(100);
    if code >= 200 {
        let message = meta.and_then(|m| m.get("message")).and_then(Value::as_str).unwrap_or("the server refused the request");
        return Err(Error::Status { status: code as u16, message: message.to_string() });
    }
    json.get("ocs").and_then(|o| o.get("data")).cloned().ok_or_else(|| Error::Protocol("the server's answer holds no data".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(body: &str) -> Reply {
        Reply { status: 200, headers: Vec::new(), body: body.as_bytes().to_vec() }
    }

    #[test]
    fn a_failing_call_is_an_error() {
        let body = r#"{"ocs":{"meta":{"status":"failure","statuscode":404,"message":"not found"},"data":[]}}"#;
        let e = ocs_data(&reply(body)).unwrap_err();
        assert!(matches!(e, Error::Status { status: 404, .. }), "{e:?}");
    }

    #[test]
    fn the_data_part_is_read() {
        let body = r#"{"ocs":{"meta":{"status":"ok","statuscode":100},"data":{"url":"https://cloud/s/abc"}}}"#;
        let data = ocs_data(&reply(body)).unwrap();
        assert_eq!(data.get("url").and_then(Value::as_str), Some("https://cloud/s/abc"));
    }

    #[test]
    fn an_answer_that_is_not_json_is_an_error() {
        assert!(ocs_data(&reply("<html>oops</html>")).is_err());
    }
}

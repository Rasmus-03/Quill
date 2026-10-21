//! Nextcloud: the files on a server, opened and saved like local ones.
//!
//! The commands here never wait for a server. They hand the request to a worker thread
//! ([`wordcraft_cloud::Worker`]) and answer at once; the front end calls
//! [`Session::poll_cloud`] once per frame and the answers land in [`Session::cloud`].
//!
//! A Markdown file on the server is an Obsidian note: its frontmatter, wikilinks and embeds
//! survive the round trip, and its images are written beside it in an attachments folder.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use wordcraft_cloud::{Account, Answer, Capabilities, Config, Entry, Job, JobKind, Outcome, SearchHit, Version, Worker};

use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// The cloud account and what is known about the server.
pub struct Cloud {
    /// The server's address.
    pub server: String,
    /// The account's user name.
    pub user: String,
    /// The server's certificate is not checked.
    pub insecure: bool,
    /// The server answered: the account works.
    pub connected: bool,
    /// What the server says about itself.
    pub capabilities: Option<Capabilities>,
    /// The account on the server.
    pub account: Option<Account>,
    /// The folder the file list shows.
    pub folder: String,
    /// The files in that folder.
    pub files: Vec<Entry>,
    /// The last search's hits.
    pub hits: Vec<SearchHit>,
    /// The older versions the server keeps for a file.
    pub versions: Vec<Version>,
    /// The last public link that was made.
    pub link: Option<String>,
    /// The file open from the server.
    pub document: Option<CloudDocument>,
    /// The last thing that went wrong.
    pub error: Option<String>,
    /// The worker's settings. The password lives here and nowhere else.
    config: Option<Config>,
    /// The worker thread.
    worker: Option<Worker>,
    /// What to do when an answer arrives.
    pending: Vec<(u64, Pending)>,
    /// The number the next request gets.
    next_id: u64,
    /// How many requests are still waiting for an answer.
    pub in_flight: usize,
}

/// The file open from the server.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudDocument {
    /// Its path on the server.
    pub path: String,
    /// Its name.
    pub name: String,
    /// The server's tag for the version that was read.
    pub etag: String,
}

/// What to do with an answer when it arrives.
enum Pending {
    /// Open the file that arrives.
    Open { path: String },
    /// The file was written. The undo depth then, so edits made while the write was on its way
    /// are not mistaken for saved ones.
    Saved { depth: usize },
}

impl Default for Cloud {
    fn default() -> Self {
        Self {
            server: String::new(),
            user: String::new(),
            insecure: false,
            connected: false,
            capabilities: None,
            account: None,
            folder: String::new(),
            files: Vec::new(),
            hits: Vec::new(),
            versions: Vec::new(),
            link: None,
            document: None,
            error: None,
            config: None,
            worker: None,
            pending: Vec::new(),
            next_id: 1,
            in_flight: 0,
        }
    }
}

impl Cloud {
    /// Use this account from now on.
    pub fn connect(&mut self, config: Config) {
        self.server = config.server.clone();
        self.user = config.user.clone();
        self.insecure = config.insecure;
        self.config = Some(config.clone());
        let worker = self.worker.get_or_insert_with(Worker::spawn);
        worker.set_config(config);
        self.connected = false;
        self.error = None;
        self.files.clear();
        self.hits.clear();
        self.versions.clear();
        self.link = None;
        self.pending.clear();
        self.send(JobKind::Capabilities);
        self.send(JobKind::Account);
    }

    /// Forget the account. The password goes with it.
    pub fn disconnect(&mut self) {
        if let Some(worker) = &self.worker {
            worker.clear_config();
        }
        self.config = None;
        self.connected = false;
        self.capabilities = None;
        self.account = None;
        self.files.clear();
        self.hits.clear();
        self.versions.clear();
        self.link = None;
        self.document = None;
        self.error = None;
        self.pending.clear();
        self.in_flight = 0;
    }

    /// Is an account connected?
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// The account's settings, with the password.
    pub fn config(&self) -> Option<&Config> {
        self.config.as_ref()
    }

    /// Hand a request to the worker and remember what to do with the answer.
    ///
    /// `None` means the worker is not running: the browser build has no threads, so nothing
    /// would answer.
    pub fn send(&mut self, kind: JobKind) -> Option<u64> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let worker = self.worker.as_ref()?;
        if !worker.send(Job { id, kind }) {
            // The worker is not running (the browser build): nothing would answer.
            self.error = Some("the cloud is not available here".into());
            return None;
        }
        self.in_flight = self.in_flight.saturating_add(1);
        Some(id)
    }

    /// Is a server connected and able to answer?
    pub fn available(&self) -> bool {
        self.worker.as_ref().is_some_and(|w| w.running())
    }

    /// Remember what to do when a request comes back.
    fn expect(&mut self, id: u64, pending: Pending) {
        self.pending.push((id, pending));
    }
}

impl Session {
    /// Apply the answers the cloud worker has. Returns whether anything came in.
    ///
    /// The front end calls this once per frame; commands and scripts call it too, so the state
    /// is never stale.
    pub fn poll_cloud(&mut self) -> bool {
        let mut outcomes = Vec::new();
        if let Some(worker) = self.cloud.worker.as_ref() {
            while let Some(outcome) = worker.try_recv() {
                outcomes.push(outcome);
            }
        }
        let changed = !outcomes.is_empty();
        for outcome in outcomes {
            self.apply_cloud_outcome(outcome);
        }
        changed
    }

    /// Apply one answer.
    fn apply_cloud_outcome(&mut self, outcome: Outcome) {
        let id = outcome.id;
        let waiting = self.cloud.pending.iter().position(|(pid, _)| *pid == id).map(|at| self.cloud.pending.remove(at).1);
        self.cloud.in_flight = self.cloud.in_flight.saturating_sub(1);
        match outcome.result {
            Ok(answer) => {
                self.cloud.error = None;
                match answer {
                    Answer::Capabilities(capabilities) => {
                        self.cloud.capabilities = Some(capabilities);
                        self.cloud.connected = true;
                    }
                    Answer::Account(account) => self.cloud.account = Some(account),
                    Answer::Listing(files) => self.cloud.files = files,
                    Answer::Bytes { bytes, etag } => {
                        let path = match waiting {
                            Some(Pending::Open { path }) => path,
                            _ => String::new(),
                        };
                        self.open_cloud_file(path, bytes, etag);
                    }
                    Answer::Saved { path, etag } => {
                        if let Some(Pending::Saved { depth }) = waiting {
                            self.cloud.document = Some(CloudDocument { name: path.rsplit('/').next().unwrap_or(&path).to_string(), path, etag });
                            // Only the edits that were sent are saved: anything typed while the
                            // write was on its way is still unsaved.
                            if self.undo_depth() == depth {
                                self.dirty = false;
                                self.stamp_save();
                            }
                        }
                    }
                    Answer::Shared(link) => self.cloud.link = Some(link),
                    Answer::Search(hits) => self.cloud.hits = hits,
                    Answer::Versions(versions) => self.cloud.versions = versions,
                }
            }
            Err(e) => {
                log::warn!("cloud: {e}");
                self.cloud.error = Some(e);
            }
        }
    }

    /// Open a file that came from the server.
    fn open_cloud_file(&mut self, path: String, bytes: Vec<u8>, etag: String) {
        if path.is_empty() {
            return;
        }
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();
        let doc = if is_markdown(&name) {
            // A note on the server is an Obsidian note: frontmatter, wikilinks and embeds stay.
            wordcraft_formats::obsidian::import_bytes(&bytes)
        } else {
            match crate::io::open_bytes_with(&name, &bytes, None) {
                Ok((doc, _)) => doc,
                Err(e) => {
                    self.cloud.error = Some(e);
                    return;
                }
            }
        };
        self.set_document(doc);
        self.path = None;
        self.dirty = false;
        // The tag for the version that was read: a save sends it back, so the server refuses
        // the write when the file changed in the meantime.
        self.cloud.document = Some(CloudDocument { path, name, etag });
    }
}

/// Is this name a Markdown note?
fn is_markdown(name: &str) -> bool {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(ext.as_str(), "md" | "markdown")
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("cloud.connect", "Connect to Nextcloud", "File › Cloud", connect)
            .params(r#"{"server": string (the server's address), "user": string (the account's name), "password": string (an app password), "insecure"?: bool (do not check the server's certificate)}"#)
            .pure(),
        CommandSpec::new("cloud.disconnect", "Disconnect from Nextcloud", "File › Cloud", disconnect).params("{}").pure(),
        CommandSpec::new("cloud.status", "Nextcloud Status", "View › Nextcloud", status).params("{}").pure(),
        CommandSpec::new("cloud.list", "Nextcloud Files", "View › Nextcloud", list)
            .params(r#"{"path"?: string (a folder on the server; empty is the account's root)}"#)
            .pure(),
        CommandSpec::new("cloud.open", "Open from Nextcloud", "File › Open", open)
            .params(r#"{"path": string (the file on the server)}"#)
            .pure(),
        CommandSpec::new("cloud.save", "Save to Nextcloud", "File › Save As", save)
            .params(r#"{"path"?: string (the file on the server; the open cloud document when omitted)}"#)
            .pure(),
        CommandSpec::new("cloud.mkdir", "New Nextcloud Folder", "View › Nextcloud", mkdir).params(r#"{"path": string}"#).pure(),
        CommandSpec::new("cloud.delete", "Delete on Nextcloud", "View › Nextcloud", delete).params(r#"{"path": string}"#).pure(),
        CommandSpec::new("cloud.share", "Share on Nextcloud", "File › Share", share)
            .params(r#"{"path"?: string (the file on the server; the open cloud document when omitted)}"#)
            .pure(),
        CommandSpec::new("cloud.search", "Search Nextcloud Files", "View › Nextcloud", search).params(r#"{"term": string}"#).pure(),
        CommandSpec::new("cloud.versions", "Nextcloud File Versions", "File › Info", versions)
            .params(r#"{"path"?: string (the file on the server; the open cloud document when omitted)}"#)
            .pure(),
        CommandSpec::new("cloud.wait", "Wait for Nextcloud", "View › Nextcloud", wait)
            .params(r#"{"ms"?: number (how long to wait, default 5000, max 60000)}"#)
            .pure(),
    ]
}

fn connect(s: &mut Session, v: &Value) -> CmdResult {
    let server = p::req_str(v, "server")?.trim().to_string();
    let user = p::req_str(v, "user")?.trim().to_string();
    let password = p::str(v, "password").unwrap_or_default().to_string();
    let insecure = p::bool(v, "insecure").unwrap_or(false);
    let config = Config::new(&server, &user, &password).with_insecure(insecure);
    // The address and the account are checked here, so a wrong one is reported right away.
    wordcraft_cloud::Client::new(config.clone()).map_err(|e| CmdError::Failed(e.to_string()))?;
    s.cloud.connect(config);
    Ok(json!({"queued": true, "server": s.cloud.server.clone(), "user": s.cloud.user.clone()}))
}

fn disconnect(s: &mut Session, _: &Value) -> CmdResult {
    s.cloud.disconnect();
    Ok(json!({"connected": false}))
}

fn status(s: &mut Session, _: &Value) -> CmdResult {
    s.poll_cloud();
    Ok(json!({
        "connected": s.cloud.connected,
        "server": s.cloud.server.clone(),
        "user": s.cloud.user.clone(),
        "insecure": s.cloud.insecure,
        "capabilities": s.cloud.capabilities,
        "account": s.cloud.account,
        "folder": s.cloud.folder.clone(),
        "files": s.cloud.files,
        "hits": s.cloud.hits,
        "versions": s.cloud.versions,
        "link": s.cloud.link,
        "document": s.cloud.document,
        "error": s.cloud.error,
        "inFlight": s.cloud.in_flight,
    }))
}

fn list(s: &mut Session, v: &Value) -> CmdResult {
    let path = p::str(v, "path").unwrap_or("").trim().to_string();
    s.cloud.folder = path.clone();
    s.cloud.files.clear();
    s.cloud.send(JobKind::List(path.clone()));
    Ok(json!({"queued": true, "folder": path}))
}

fn open(s: &mut Session, v: &Value) -> CmdResult {
    let path = p::req_str(v, "path")?.trim().to_string();
    if path.is_empty() {
        return Err(CmdError::Params("`path` (a file on the server) is required".into()));
    }
    let Some(id) = s.cloud.send(JobKind::Get(path.clone())) else {
        return Err(CmdError::Failed("the cloud is not available here".into()));
    };
    s.cloud.expect(id, Pending::Open { path: path.clone() });
    Ok(json!({"queued": true, "path": path}))
}

fn save(s: &mut Session, v: &Value) -> CmdResult {
    let path = match p::str(v, "path").map(str::trim).filter(|p| !p.is_empty()) {
        Some(path) => path.to_string(),
        None => s.cloud.document.as_ref().map(|d| d.path.clone()).unwrap_or_default(),
    };
    if path.is_empty() {
        return Err(CmdError::Failed("there is no file on the server to save into".into()));
    }
    save_to(s, &path, v)
}

/// Write the document to the server, at `path`.
///
/// The write goes to a worker thread and the answer comes later, so the document is still
/// marked as changed until the server has taken it.
pub fn save_to(s: &mut Session, path: &str, v: &Value) -> CmdResult {
    let path = path.to_string();
    let files = cloud_files(s, &path, v)?;
    let etag = s.cloud.document.as_ref().filter(|d| d.path == path).map(|d| d.etag.clone());
    // The images first: a note never points at a file that is not there yet.
    let count = files.len();
    for (index, (remote, bytes)) in files.into_iter().enumerate() {
        let last = index + 1 == count;
        let Some(id) = s.cloud.send(JobKind::Put { path: remote.clone(), bytes, etag: if last { etag.clone() } else { None } }) else {
            return Err(CmdError::Failed("the cloud is not available here".into()));
        };
        if last {
            s.cloud.expect(id, Pending::Saved { depth: s.undo_depth() });
        }
    }
    Ok(json!({"queued": true, "path": path, "files": count}))
}

/// The files a save writes: the document itself, and for a note its images.
fn cloud_files(s: &mut Session, path: &str, v: &Value) -> Result<Vec<(String, Vec<u8>)>, CmdError> {
    let name = path.rsplit('/').next().unwrap_or(path);
    if !is_markdown(name) {
        let opts = crate::cmd::file::export_options(s, v);
        let bytes = crate::io::save_bytes_with(path, &s.doc, None, &opts).map_err(CmdError::Failed)?;
        return Ok(vec![(path.to_string(), bytes)]);
    }
    let folder = path.rsplit_once('/').map(|(folder, _)| folder.to_string()).unwrap_or_default();
    let bundle = wordcraft_formats::obsidian::export_bundle(&s.doc, &s.prefs.vault_attachments);
    let mut files = Vec::new();
    for asset in bundle.assets {
        let remote = if folder.is_empty() { asset.path.clone() } else { format!("{folder}/{}", asset.path) };
        files.push((remote, asset.bytes));
    }
    files.push((path.to_string(), bundle.markdown.into_bytes()));
    Ok(files)
}

fn mkdir(s: &mut Session, v: &Value) -> CmdResult {
    let path = p::req_str(v, "path")?.trim().to_string();
    if path.is_empty() {
        return Err(CmdError::Params("`path` (a folder on the server) is required".into()));
    }
    s.cloud.send(JobKind::MkDir(path.clone()));
    Ok(json!({"queued": true, "path": path}))
}

fn delete(s: &mut Session, v: &Value) -> CmdResult {
    let path = p::req_str(v, "path")?.trim().to_string();
    if path.is_empty() {
        return Err(CmdError::Params("`path` (a file on the server) is required".into()));
    }
    s.cloud.send(JobKind::Delete(path.clone()));
    Ok(json!({"queued": true, "path": path}))
}

fn share(s: &mut Session, v: &Value) -> CmdResult {
    let path = match p::str(v, "path").map(str::trim).filter(|p| !p.is_empty()) {
        Some(path) => path.to_string(),
        None => s.cloud.document.as_ref().map(|d| d.path.clone()).unwrap_or_default(),
    };
    if path.is_empty() {
        return Err(CmdError::Failed("there is no file on the server to share".into()));
    }
    s.cloud.send(JobKind::Share(path.clone()));
    Ok(json!({"queued": true, "path": path}))
}

fn search(s: &mut Session, v: &Value) -> CmdResult {
    let term = p::req_str(v, "term")?.trim().to_string();
    if term.is_empty() {
        return Err(CmdError::Params("`term` (what to look for) is required".into()));
    }
    s.cloud.hits.clear();
    s.cloud.send(JobKind::Search(term.clone()));
    Ok(json!({"queued": true, "term": term}))
}

fn versions(s: &mut Session, v: &Value) -> CmdResult {
    let path = match p::str(v, "path").map(str::trim).filter(|p| !p.is_empty()) {
        Some(path) => path.to_string(),
        None => s.cloud.document.as_ref().map(|d| d.path.clone()).unwrap_or_default(),
    };
    if path.is_empty() {
        return Err(CmdError::Failed("there is no file on the server to ask about".into()));
    }
    s.cloud.versions.clear();
    s.cloud.send(JobKind::Versions(path.clone()));
    Ok(json!({"queued": true, "path": path}))
}

fn wait(s: &mut Session, v: &Value) -> CmdResult {
    let ms = p::u64(v, "ms").unwrap_or(5000).clamp(0, 60_000);
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(ms);
    while s.cloud.in_flight > 0 && std::time::Instant::now() < deadline {
        s.poll_cloud();
        if s.cloud.in_flight == 0 {
            break;
        }
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    s.poll_cloud();
    Ok(json!({
        "connected": s.cloud.connected,
        "files": s.cloud.files.len(),
        "hits": s.cloud.hits.len(),
        "error": s.cloud.error,
        "inFlight": s.cloud.in_flight,
    }))
}

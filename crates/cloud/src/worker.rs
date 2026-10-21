//! A background worker: the same requests, off the editor's thread.
//!
//! The editor must never wait for a server, so the UI hands the worker a [`Job`] and collects
//! the [`Outcome`] once per frame. Every job carries a number, so the answer can be matched to
//! the request that asked for it. The worker keeps one connection and rebuilds it when the
//! account changes.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use crate::{Account, Capabilities, Config, Entry, SearchHit, Version};

/// What to ask the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobKind {
    /// Ask the server about itself.
    Capabilities,
    /// Ask which account is signed in.
    Account,
    /// List a folder (the account's root when the path is empty).
    List(String),
    /// Read a file.
    Get(String),
    /// Write a file, optionally only when it is still the version that was read.
    Put { path: String, bytes: Vec<u8>, etag: Option<String> },
    /// Create a folder.
    MkDir(String),
    /// Delete a file or folder.
    Delete(String),
    /// Make a public link to a file.
    Share(String),
    /// Search the account's files.
    Search(String),
    /// List a file's older versions.
    Versions(String),
}

/// A request for the worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Job {
    /// The number that tells this request apart from the others.
    pub id: u64,
    /// What to ask for.
    pub kind: JobKind,
}

/// The server's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    /// The server's own information.
    Capabilities(Capabilities),
    /// The account on the server.
    Account(Account),
    /// The contents of a folder.
    Listing(Vec<Entry>),
    /// A file's bytes.
    Bytes(Vec<u8>),
    /// A file was written, with the server's new tag for it.
    Saved { path: String, etag: String },
    /// A public link was made.
    Shared(String),
    /// The server's search results.
    Search(Vec<SearchHit>),
    /// A file's older versions.
    Versions(Vec<Version>),
}

/// What came back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// The number of the request this answers.
    pub id: u64,
    /// The answer, or why there is none.
    pub result: Result<Answer, String>,
}

impl Outcome {
    /// Did the request fail?
    pub fn failed(&self) -> bool {
        self.result.is_err()
    }

    /// Why the request failed, if it did.
    pub fn error(&self) -> Option<&str> {
        match &self.result {
            Err(e) => Some(e),
            Ok(_) => None,
        }
    }
}

/// The worker: a thread that answers the editor's questions about a server.
pub struct Worker {
    jobs: Sender<Job>,
    results: Receiver<Outcome>,
    config: Arc<Mutex<Option<Config>>>,
    next_id: std::sync::atomic::AtomicU64,
    /// The worker thread is running. The browser has no threads to run it on.
    alive: bool,
}

impl Default for Worker {
    fn default() -> Self {
        Self::spawn()
    }
}

impl Worker {
    /// Start the worker.
    pub fn spawn() -> Self {
        let (jobs, inbox) = mpsc::channel::<Job>();
        let (outbox, results) = mpsc::channel::<Outcome>();
        let config = Arc::new(Mutex::new(None));
        let thread_config = Arc::clone(&config);
        let running = std::thread::Builder::new().name("quill-cloud".into()).spawn(move || {
            let mut client: Option<(Config, crate::Client)> = None;
            while let Ok(job) = inbox.recv() {
                let current = thread_config.lock().ok().and_then(|c| c.clone());
                let Some(settings) = current else {
                    let _ = outbox.send(Outcome { id: job.id, result: Err("no server is connected".into()) });
                    continue;
                };
                let stale = match &client {
                    Some((used, _)) => used != &settings,
                    None => true,
                };
                if stale {
                    match crate::Client::new(settings.clone()) {
                        Ok(fresh) => client = Some((settings.clone(), fresh)),
                        Err(e) => {
                            let _ = outbox.send(Outcome { id: job.id, result: Err(e.to_string()) });
                            continue;
                        }
                    }
                }
                let Some((_, connection)) = client.as_ref() else { continue };
                let _ = outbox.send(run(job.id, connection, job.kind));
            }
        });
        let alive = running.is_ok();
        Self { jobs, results, config, next_id: std::sync::atomic::AtomicU64::new(1), alive }
    }

    /// Use this account from now on.
    pub fn set_config(&self, config: Config) {
        if let Ok(mut slot) = self.config.lock() {
            *slot = Some(config);
        }
    }

    /// Forget the account.
    pub fn clear_config(&self) {
        if let Ok(mut slot) = self.config.lock() {
            *slot = None;
        }
    }

    /// The account the worker uses.
    pub fn config(&self) -> Option<Config> {
        self.config.lock().ok().and_then(|c| c.clone())
    }

    /// A number no other request has used.
    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    /// Is the worker thread running? The browser has no threads to run it on.
    pub fn running(&self) -> bool {
        self.alive
    }

    /// Ask the worker something. The answer comes later through [`Worker::try_recv`].
    ///
    /// `false` means the worker is not running (the browser build), so nothing will answer.
    pub fn send(&self, job: Job) -> bool {
        if !self.alive {
            return false;
        }
        self.jobs.send(job).is_ok()
    }

    /// The next answer, if one is waiting.
    pub fn try_recv(&self) -> Option<Outcome> {
        self.results.try_recv().ok()
    }

    /// Wait for the next answer, up to `timeout`.
    pub fn recv_timeout(&self, timeout: std::time::Duration) -> Option<Outcome> {
        self.results.recv_timeout(timeout).ok()
    }
}

/// Do one job.
fn run(id: u64, client: &crate::Client, kind: JobKind) -> Outcome {
    let result = match kind {
        JobKind::Capabilities => client.capabilities().map(Answer::Capabilities),
        JobKind::Account => client.account().map(Answer::Account),
        JobKind::List(path) => client.list(&path).map(Answer::Listing),
        JobKind::Get(path) => client.get(&path).map(Answer::Bytes),
        JobKind::Put { path, bytes, etag } => client.put_if(&path, &bytes, etag.as_deref()).map(|etag| Answer::Saved { path, etag }),
        JobKind::MkDir(path) => client.mkdir(&path).map(|()| Answer::Saved { path, etag: String::new() }),
        JobKind::Delete(path) => client.delete(&path).map(|()| Answer::Saved { path, etag: String::new() }),
        JobKind::Share(path) => client.share_link(&path).map(Answer::Shared),
        JobKind::Search(term) => client.search(&term).map(Answer::Search),
        JobKind::Versions(path) => client.versions(&path).map(Answer::Versions),
    };
    Outcome { id, result: result.map_err(|e| e.to_string()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_worker_without_an_account_says_so() {
        let worker = Worker::spawn();
        worker.send(Job { id: 1, kind: JobKind::List("/".into()) });
        let outcome = worker.recv_timeout(std::time::Duration::from_secs(5));
        assert!(matches!(outcome, Some(Outcome { result: Err(_), .. })), "{outcome:?}");
    }

    #[test]
    fn the_account_can_be_set_and_forgotten() {
        let worker = Worker::spawn();
        assert!(worker.config().is_none());
        worker.set_config(Config::new("https://cloud.example.com", "rasmus", "secret"));
        assert_eq!(worker.config().map(|c| c.user), Some("rasmus".to_string()));
        worker.clear_config();
        assert!(worker.config().is_none());
    }

    #[test]
    fn every_request_gets_its_own_number() {
        let worker = Worker::spawn();
        let first = worker.next_id();
        let second = worker.next_id();
        assert_ne!(first, second);
    }
}

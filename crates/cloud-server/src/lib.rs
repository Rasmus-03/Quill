//! WordCraft real-time collaboration server.
//!
//! A Node.js/Express + WebSocket server that provides shared documents with
//! operational transform. Documents are identified by a document id; each
//! client connects over WebSocket and receives the full document snapshot,
//! then applies operations (insert, delete) in order. The server is the
//! single source of truth for the document state.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// A document in the collaboration server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Doc {
    /// The document id (the `.docx` path in the workspace).
    pub id: String,
    /// The current document text (plain text, with newline at end).
    pub text: String,
    /// Operations applied so far, in order.
    pub ops: Vec<Op>,
    /// The session ids currently viewing/editing this document.
    pub sessions: Vec<String>,
}

/// A document operation (operational transform).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Op {
    /// `insert` appends text at the end; `insert_<n>` inserts at character offset n.
    /// `delete_<n>` deletes n characters at the end; `delete_<n>` at character offset n.
    pub kind: String,
    /// The text to insert.
    pub text: Option<String>,
    /// The character offset for insert/delete.
    pub off: usize,
}

/// A session (client connection).
pub struct Session {
    pub id: String,
    pub doc_id: Option<String>,
}

/// The collaboration server state.
pub struct Server {
    /// All documents by id.
    pub docs: Arc<Mutex<HashMap<String, Doc>>>,
    /// Active sessions by id.
    pub sessions: Arc<Mutex<HashMap<String, Session>>>,
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

impl Server {
    /// Create a new collaboration server.
    pub fn new() -> Self {
        Self { docs: Arc::new(Mutex::new(HashMap::new())), sessions: Arc::new(Mutex::new(HashMap::new())) }
    }

    /// Create a new document with the given text.
    pub fn create_doc(&self, id: &str, text: &str) {
        let mut docs = self.docs.lock().unwrap();
        docs.insert(id.to_string(), Doc { id: id.to_string(), text: text.to_string(), ops: vec![], sessions: vec![] });
    }

    /// Get a document.
    pub fn get_doc(&self, id: &str) -> Option<Doc> {
        self.docs.lock().unwrap().get(id).cloned()
    }

    /// Add a session.
    pub fn add_session(&self, id: &str, doc_id: Option<String>) {
        let mut sessions = self.sessions.lock().unwrap();
        sessions.insert(id.to_string(), Session { id: id.to_string(), doc_id });
    }

    /// Remove a session.
    pub fn remove_session(&self, id: &str) {
        let mut sessions = self.sessions.lock().unwrap();
        sessions.remove(id);
    }

    /// Broadcast an operation to all sessions viewing this document.
    pub fn broadcast(&self, doc_id: &str, op: &Op) {
        let sessions =
            self.sessions.lock().unwrap().values().filter(|s| s.doc_id.as_deref() == Some(doc_id)).map(|s| s.id.clone()).collect::<Vec<_>>();
        for sid in sessions {
            // In a real implementation, each session has a WebSocket, and we send the op
            // over that socket. Here we just record that the op was sent.
            log::debug!("broadcast op to session {sid}: {:?}", op);
        }
    }
}

//! Obsidian vaults: a folder of Markdown notes, its note list and search, and saving a note with
//! its images beside it.
//!
//! Only the desktop app can reach a vault's files. The commands are still registered in every
//! build so the registry stays the same; on the web they fail with an ordinary error.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// Most notes one vault lists, so a hostile or enormous folder cannot exhaust memory.
pub const MAX_FILES: usize = 20_000;
/// Deepest folder nesting scanned.
pub const MAX_DEPTH: usize = 32;
/// Largest note read for search or opening.
pub const MAX_NOTE_BYTES: u64 = 32 * 1024 * 1024;
/// Most notes whose text one search opens.
const MAX_SEARCH_FILES: usize = 5_000;
/// Largest note whose text one search reads.
const MAX_SEARCH_NOTE_BYTES: u64 = 2 * 1024 * 1024;
/// Total bytes one search may read.
const MAX_SEARCH_BYTES: u64 = 64 * 1024 * 1024;

/// One Markdown note in the vault.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultFile {
    /// Relative to the vault root, with `/` separators.
    pub path: String,
    /// File name without `.md`, which is the note's title.
    pub title: String,
    /// Seconds since the Unix epoch, or 0 when unavailable.
    pub modified: u64,
    pub size: u64,
}

/// A search hit, with the text around the match when the title did not match.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultHit {
    pub path: String,
    pub title: String,
    pub modified: u64,
    pub snippet: String,
}

/// The links between the notes in a vault, and the tags they carry.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteIndex {
    /// For each note, the notes it links to.
    pub outgoing: BTreeMap<String, Vec<String>>,
    /// For each note, the notes that link to it.
    pub backlinks: BTreeMap<String, Vec<String>>,
    /// The tags and the notes that carry them.
    pub tags: BTreeMap<String, Vec<String>>,
}

/// Most links one note may hold.
pub const MAX_LINKS_PER_NOTE: usize = 2000;
/// Most tags one note may carry.
pub const MAX_TAGS_PER_NOTE: usize = 200;
/// Most notes one index reads.
const MAX_INDEX_FILES: usize = 5000;
/// How long one index may read notes, so the interface never waits on a big vault.
const MAX_INDEX_MILLIS: u64 = 400;
/// Largest note one index reads.
const MAX_INDEX_BYTES: u64 = 2 * 1024 * 1024;

/// The open vault in this session. The root is persisted in [`crate::session::Prefs`]; the file
/// list is rebuilt when the vault is opened or scanned.
#[derive(Clone, Debug, Default)]
pub struct Vault {
    pub root: Option<PathBuf>,
    pub files: Vec<VaultFile>,
    /// The note currently open, relative to the root.
    pub note: Option<PathBuf>,
    /// The scan stopped at [`MAX_FILES`] or [`MAX_DEPTH`].
    pub truncated: bool,
    /// Last search query and its results, so the Navigation pane redraws without reading files
    /// on every frame.
    pub query: String,
    pub results: Vec<VaultHit>,
    /// The links between the notes and their tags, built when they are first asked for and kept
    /// until the vault is read again.
    index: Option<NoteIndex>,
}

impl Vault {
    /// Open the folder at `path` and list its notes.
    pub fn scan_at(path: &Path) -> Result<Self, String> {
        let root = path.canonicalize().map_err(|e| format!("{}: {e}", path.display()))?;
        if !root.is_dir() {
            return Err(format!("{}: not a folder", path.display()));
        }
        let mut vault = Vault { root: Some(root), ..Default::default() };
        vault.scan()?;
        Ok(vault)
    }

    /// Rebuild the note list under the current root.
    pub fn scan(&mut self) -> Result<(), String> {
        let root = self.root.clone().ok_or_else(|| "no vault is open".to_string())?;
        self.files.clear();
        // The notes may have changed, so the links between them are read again when they are
        // asked for.
        self.index = None;
        self.truncated = false;
        let mut stack = vec![(root.clone(), 0usize)];
        'walk: while let Some((dir, depth)) = stack.pop() {
            let Ok(entries) = crate::io::vault_fs::read_dir(&dir) else { continue };
            let mut entries = entries;
            entries.sort_by_key(|e| e.file_name());
            for entry in entries {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with('.') {
                    continue;
                }
                let Ok(kind) = entry.file_type() else { continue };
                if kind.is_symlink() {
                    // A link may leave the vault or form a cycle.
                    continue;
                }
                let path = entry.path();
                if kind.is_dir() {
                    if matches!(name.as_ref(), "node_modules" | "target" | "dist" | "build") {
                        continue;
                    }
                    if depth + 1 < MAX_DEPTH {
                        stack.push((path, depth + 1));
                    } else {
                        self.truncated = true;
                    }
                    continue;
                }
                if !kind.is_file() || !name.to_ascii_lowercase().ends_with(".md") {
                    continue;
                }
                let Ok(meta) = entry.metadata() else { continue };
                if meta.len() > MAX_NOTE_BYTES {
                    continue;
                }
                let Ok(rel) = path.strip_prefix(&root) else { continue };
                let rel = rel.to_string_lossy().replace('\\', "/");
                let title = name.as_ref().rsplit_once('.').map(|(stem, _)| stem).unwrap_or(name.as_ref()).to_string();
                let modified = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
                self.files.push(VaultFile { path: rel, title, modified, size: meta.len() });
                if self.files.len() >= MAX_FILES {
                    self.truncated = true;
                    break 'walk;
                }
            }
        }
        self.files.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.path.to_lowercase().cmp(&b.path.to_lowercase())));
        if let Some(note) = &self.note
            && !self.files.iter().any(|f| Path::new(&f.path) == note.as_path())
        {
            self.note = None;
        }
        Ok(())
    }

    /// Is `path` inside this vault?
    pub fn contains(&self, path: &Path) -> bool {
        self.root.as_ref().is_some_and(|root| self.absolute(path).is_ok_and(|p| p.starts_with(root)))
    }

    /// An absolute path under the vault, refusing `..` and paths outside it.
    pub fn absolute(&self, path: &Path) -> Result<PathBuf, String> {
        let root = self.root.as_ref().ok_or_else(|| "no vault is open".to_string())?;
        let joined = if path.is_absolute() { path.to_path_buf() } else { root.join(path) };
        let mut clean = PathBuf::new();
        for part in joined.components() {
            match part {
                Component::Prefix(_) | Component::RootDir => clean.push(part.as_os_str()),
                Component::CurDir => {}
                Component::ParentDir => return Err(format!("{}: path leaves the vault", path.display())),
                Component::Normal(name) => clean.push(name),
            }
        }
        if !clean.starts_with(root) {
            return Err(format!("{}: path is outside the vault", path.display()));
        }
        Ok(clean)
    }

    /// An existing file under the vault, with links resolved and checked.
    pub fn resolve_read(&self, path: &Path) -> Result<PathBuf, String> {
        let root = self.root.as_ref().ok_or_else(|| "no vault is open".to_string())?;
        let abs = self.absolute(path)?;
        let real = abs.canonicalize().map_err(|e| format!("{}: {e}", abs.display()))?;
        if !real.starts_with(root) {
            return Err(format!("{}: path is outside the vault", abs.display()));
        }
        Ok(real)
    }

    /// The note's path relative to the root.
    pub fn relative(&self, path: &Path) -> Result<PathBuf, String> {
        let root = self.root.as_ref().ok_or_else(|| "no vault is open".to_string())?;
        let abs = self.absolute(path)?;
        abs.strip_prefix(root).map(Path::to_path_buf).map_err(|_| format!("{}: path is outside the vault", path.display()))
    }

    /// Search titles and note text, most recently modified first. Text matches carry a snippet.
    pub fn search(&self, query: &str, limit: usize) -> Vec<VaultHit> {
        let q = query.trim().to_lowercase();
        let mut order: Vec<&VaultFile> = self.files.iter().collect();
        order.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase())));
        let mut hits = Vec::new();
        let mut scanned = 0usize;
        let mut bytes = 0u64;
        let root = self.root.as_ref();
        for file in order {
            if hits.len() >= limit {
                break;
            }
            if q.is_empty() {
                hits.push(VaultHit { path: file.path.clone(), title: file.title.clone(), modified: file.modified, snippet: String::new() });
                continue;
            }
            let title_match = file.title.to_lowercase().contains(&q);
            let mut snippet = String::new();
            if !title_match {
                if scanned >= MAX_SEARCH_FILES || bytes >= MAX_SEARCH_BYTES {
                    break;
                }
                scanned += 1;
                if file.size > MAX_SEARCH_NOTE_BYTES {
                    continue;
                }
                let Some(root) = root else { break };
                let Ok(data) = crate::io::vault_fs::read(&root.join(&file.path)) else { continue };
                bytes = bytes.saturating_add(data.len() as u64);
                let text = wordcraft_formats::txt::decode(&data);
                let lower = text.to_lowercase();
                let Some(pos) = lower.find(&q) else { continue };
                snippet = snippet_around(&text, pos);
            }
            hits.push(VaultHit { path: file.path.clone(), title: file.title.clone(), modified: file.modified, snippet });
        }
        hits
    }

    /// The links between the notes, and the tags they carry.
    ///
    /// The index is built once and kept until the vault is read again, so the pane can ask for
    /// it on every frame without reading the notes again.
    pub fn index(&mut self) -> NoteIndex {
        if self.index.is_none() {
            let built = self.build_index();
            self.index = Some(built);
        }
        self.index.clone().unwrap_or_default()
    }

    /// Read every note and work out what links to what.
    ///
    /// Every note is read once. A vault that is too large to read is cut short — after
    /// [`MAX_INDEX_MILLIS`] so the interface never waits on a big vault — and the notes that
    /// were not read simply have no links.
    fn build_index(&self) -> NoteIndex {
        let mut index = NoteIndex::default();
        let Some(root) = self.root.as_ref() else { return index };
        let mut scanned = 0usize;
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(MAX_INDEX_MILLIS);
        for file in self.files.iter() {
            if scanned >= MAX_INDEX_FILES || std::time::Instant::now() >= deadline {
                break;
            }
            if file.size > MAX_INDEX_BYTES {
                continue;
            }
            let Ok(data) = crate::io::vault_fs::read(&root.join(&file.path)) else { continue };
            scanned += 1;
            let text = wordcraft_formats::txt::decode(&data);
            let targets = wikilinks(&text);
            for target in targets {
                let note = self.note_for(&target);
                if let Some(note) = note
                    && note != file.path
                {
                    index.outgoing.entry(file.path.clone()).or_default().push(note.clone());
                    let back = index.backlinks.entry(note).or_default();
                    if !back.contains(&file.path) {
                        back.push(file.path.clone());
                    }
                }
            }
            for tag in note_tags(&text) {
                let notes = index.tags.entry(tag).or_default();
                if !notes.contains(&file.path) {
                    notes.push(file.path.clone());
                }
            }
        }
        for notes in index.backlinks.values_mut() {
            notes.sort();
        }
        for notes in index.tags.values_mut() {
            notes.sort();
        }
        index
    }

    /// The note a `[[wikilink]]` points at: the note with that title, or with that path.
    pub fn note_for(&self, target: &str) -> Option<String> {
        let wanted = target.trim().trim_end_matches(".md").to_lowercase();
        if wanted.is_empty() {
            return None;
        }
        // A link names the note without its folder ("[[Note]]") or with it ("[[Folder/Note]]").
        for file in &self.files {
            if file.path.to_lowercase() == wanted {
                return Some(file.path.clone());
            }
        }
        for file in &self.files {
            if file.title.to_lowercase() == wanted {
                return Some(file.path.clone());
            }
        }
        None
    }
}

/// The notes a text links to: the targets of its `[[wikilinks]]`.
pub fn wikilinks(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("[[") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find("]]") else { break };
        let inner = &rest[..end];
        rest = &rest[end + 2..];
        // `[[Note#Heading|Alias]]` points at the note; the heading and the alias are not part of it.
        let target = inner.split('|').next().unwrap_or("").split('#').next().unwrap_or("").trim();
        if target.is_empty() || target.len() > 255 {
            continue;
        }
        if !out.iter().any(|t: &String| t == target) {
            out.push(target.to_string());
        }
        if out.len() >= MAX_LINKS_PER_NOTE {
            break;
        }
    }
    out
}

/// The tags a note carries: the ones in its frontmatter and the ones written in its text.
pub fn note_tags(text: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut body = text;
    // The frontmatter's own `tags:` list.
    if let Some(rest) = text.strip_prefix("---\n").or_else(|| text.strip_prefix("---\r\n")) {
        let end = rest.find("\n---").unwrap_or(rest.len());
        let front = &rest[..end];
        body = &rest[end.min(rest.len())..];
        let mut in_tags = false;
        for line in front.lines() {
            let line = line.trim_end();
            if line.trim_start().starts_with("tags:") {
                in_tags = true;
                let value = line.trim_start().trim_start_matches("tags:").trim();
                if value.starts_with('[') {
                    for tag in value.trim_matches(|c| c == '[' || c == ']').split(',') {
                        push_tag(&mut tags, tag.trim().trim_matches(|c| c == '"' || c == '\''));
                    }
                    in_tags = false;
                }
                continue;
            }
            if in_tags {
                if let Some(tag) = line.trim_start().strip_prefix("- ") {
                    push_tag(&mut tags, tag.trim().trim_matches(|c| c == '"' || c == '\''));
                } else if !line.trim().is_empty() {
                    in_tags = false;
                }
            }
        }
    }
    // The `#tags` written in the note itself.
    for (at, c) in body.char_indices() {
        if c != '#' {
            continue;
        }
        // A tag starts a word: the `#` may not follow a letter or a digit.
        if at > 0 {
            let before = body[..at].chars().next_back().unwrap_or(' ');
            if before.is_alphanumeric() {
                continue;
            }
        }
        let rest = &body[at + 1..];
        let end = rest.find(|c: char| !c.is_alphanumeric() && c != '_' && c != '-').unwrap_or(rest.len());
        let tag = &rest[..end];
        if !tag.is_empty() {
            push_tag(&mut tags, tag);
        }
    }
    tags
}

/// Add a tag, keeping the list short and free of duplicates.
fn push_tag(tags: &mut Vec<String>, tag: &str) {
    let tag = tag.trim();
    if tag.is_empty() || tag.len() > 64 || tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
        return;
    }
    if tags.len() >= MAX_TAGS_PER_NOTE {
        return;
    }
    tags.push(tag.to_string());
}

fn snippet_around(text: &str, pos: usize) -> String {
    let mut start = pos.saturating_sub(48);
    while start > 0 && !text.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = pos.saturating_add(96).min(text.len());
    while end < text.len() && !text.is_char_boundary(end) {
        end += 1;
    }
    let body = text.get(start..end).unwrap_or("").replace(['\n', '\r', '\t'], " ");
    let body = body.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{}{}{}", if start > 0 { "…" } else { "" }, body, if end < text.len() { "…" } else { "" })
}

/// Make `path`'s folder, and refuse a folder whose links lead outside the vault. The file itself
/// may be replaced but is never followed (`safe_save`), so checking the folder is enough.
fn prepare_parent(root: &Path, path: &Path) -> Result<(), CmdError> {
    let parent = path.parent().unwrap_or(root);
    crate::file_access::check_write(&parent.to_string_lossy()).map_err(CmdError::Failed)?;
    // Check the deepest existing ancestor before creating anything: creating through a symlink
    // that leaves the vault would make folders outside it even though the final check rejects.
    let mut existing = parent.to_path_buf();
    // `exists()` follows links, so a broken link would look absent and be skipped. Any entry at
    // the path — including a link — is a stopping point for the check.
    while !crate::io::vault_fs::exists_at_all(&existing) {
        if !existing.pop() {
            return Err(CmdError::Failed(format!("{}: path is outside the vault", path.display())));
        }
    }
    let real = existing.canonicalize().map_err(|e| CmdError::Failed(format!("{}: {e}", existing.display())))?;
    if !real.starts_with(root) {
        return Err(CmdError::Failed(format!("{}: path is outside the vault", path.display())));
    }
    crate::io::vault_fs::create_dir_all(parent).map_err(|e| CmdError::Failed(format!("{}: {e}", parent.display())))?;
    let real = parent.canonicalize().map_err(|e| CmdError::Failed(format!("{}: {e}", parent.display())))?;
    if !real.starts_with(root) {
        return Err(CmdError::Failed(format!("{}: path is outside the vault", path.display())));
    }
    Ok(())
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("vault.open", "Open Vault", "File › Open", open)
            .params(r#"{"path"?: string (a folder to open, or a note inside the vault; omit to ask the user)}"#)
            .pure(),
        CommandSpec::new("vault.scan", "Refresh Vault", "View › Vault", scan).params("{}").pure(),
        CommandSpec::new("vault.search", "Search Vault", "View › Vault", search)
            .params(r#"{"q": string (matches note titles and text), "limit"?: n (default 200, max 1000)}"#)
            .pure(),
        CommandSpec::new("vault.export", "Save to Vault", "File › Save As", export)
            .params(r#"{"path"?: string (relative to the vault, or absolute inside it; omit to ask), "attachments"?: string (folder for images, default the saved preference)}"#)
            .pure(),
        CommandSpec::new("vault.index", "Index Vault Notes", "View › Vault", index)
            .params("{}")
            .pure(),
        CommandSpec::new("vault.backlinks", "Vault Backlinks", "View › Vault", backlinks)
            .params(r#"{"note"?: string (the note; the open one when omitted)}"#)
            .pure(),
        CommandSpec::new("vault.tags", "Vault Tags", "View › Vault", tags).params("{}").pure(),
        CommandSpec::new("vault.insertLink", "Insert Note Link", "Insert › Link", insert_link)
            .params(r#"{"target": string (the note to link to), "label"?: string (the text shown instead of the note's name)}"#),
    ]
}

/// The links between the notes and the tags they carry.
fn index(s: &mut Session, _: &Value) -> CmdResult {
    let index = s.vault.index();
    Ok(json!({
        "notes": index.outgoing.len(),
        "outgoing": index.outgoing,
        "backlinks": index.backlinks,
        "tags": index.tags,
    }))
}

/// The notes that link to a note.
fn backlinks(s: &mut Session, v: &Value) -> CmdResult {
    let note = match p::str(v, "note") {
        Some(note) => note.to_string(),
        None => s.vault.note.as_ref().map(|n| n.to_string_lossy().replace('\\', "/")).unwrap_or_default(),
    };
    if note.is_empty() {
        return Ok(json!({"note": "", "backlinks": []}));
    }
    let index = s.vault.index();
    let links = index.backlinks.get(&note).cloned().unwrap_or_default();
    Ok(json!({"note": note, "backlinks": links}))
}

/// The tags the notes carry, and the notes that carry them.
fn tags(s: &mut Session, _: &Value) -> CmdResult {
    let index = s.vault.index();
    let mut out: Vec<(String, Vec<String>)> = index.tags.into_iter().collect();
    out.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));
    Ok(json!({"tags": out.into_iter().map(|(tag, notes)| json!({"tag": tag, "notes": notes})).collect::<Vec<_>>()}))
}

/// Write a link to a note at the caret, as Obsidian writes it.
fn insert_link(s: &mut Session, v: &Value) -> CmdResult {
    let target = p::req_str(v, "target")?.trim().to_string();
    if target.is_empty() {
        return Err(CmdError::Params("`target` (the note to link to) is required".into()));
    }
    if target.contains(['[', ']', '|', '\n', '\r']) {
        return Err(CmdError::Params("a note's name may not hold brackets or a bar".into()));
    }
    if target.chars().count() > 255 {
        return Err(CmdError::Params("the note's name is too long".into()));
    }
    let label = p::str(v, "label").map(str::trim).filter(|l| !l.is_empty());
    let text = match label {
        Some(label) if label.chars().count() <= 255 && !label.contains(['[', ']', '|', '\n', '\r']) => format!("[[{target}|{label}]]"),
        Some(_) => return Err(CmdError::Params("the label may not hold brackets or a bar".into())),
        None => format!("[[{target}]]"),
    };
    s.run("text.insert", &json!({"text": text, "raw": true}))
}

fn open(s: &mut Session, v: &Value) -> CmdResult {
    let Some(path) = p::str(v, "path") else {
        s.ui_requests.push(json!({"open": "vault"}));
        return Ok(json!({"open": true}));
    };
    let mut path = PathBuf::from(path);
    if !path.is_absolute()
        && let Some(root) = &s.vault.root
    {
        path = root.join(path);
    }
    // Resolve `..` and links before deciding which folder is the vault root or which note is
    // relative to it; a missing path is left for `scan_at` to report.
    let path = path.canonicalize().unwrap_or(path);
    let root = if let Some(root) = s.vault.root.clone().filter(|r| path.starts_with(r)) {
        root
    } else if path.is_file() {
        path.parent().unwrap_or(&path).to_path_buf()
    } else {
        path.clone()
    };
    let old_root = s.vault.root.clone();
    let old_query = s.vault.query.clone();
    let mut vault = Vault::scan_at(&root).map_err(CmdError::Failed)?;
    let note = if path.is_file() { vault.relative(&path).ok() } else { None };
    if let Some(note) = &note {
        let abs = vault.resolve_read(note).map_err(CmdError::Failed)?;
        let meta = crate::io::vault_fs::metadata(&abs).map_err(|e| CmdError::Failed(format!("{}: {e}", abs.display())))?;
        if meta.len() > MAX_NOTE_BYTES {
            return Err(CmdError::Failed(format!("{}: note is too large to open", abs.display())));
        }
        let (mut doc, encrypted) = crate::io::open_path_with(&abs, None).map_err(CmdError::Failed)?;
        let ext = abs.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        if let Some(lang) = &s.editing_lang
            && matches!(ext.as_str(), "txt" | "text" | "md" | "markdown")
        {
            doc.styles.default_chr.lang = Some(lang.clone());
        }
        s.set_document(doc);
        s.path = Some(abs);
        if encrypted {
            s.password = None;
        }
        s.dirty = false;
    }
    vault.note = note.clone();
    // Opening a note in the vault already on screen keeps its search; opening another vault
    // starts with an empty one.
    if old_root.as_ref() == vault.root.as_ref() {
        vault.query = old_query;
    }
    vault.results = vault.search(&vault.query, 200);
    s.vault = vault;
    s.prefs.vault_root = s.vault.root.clone();
    Ok(json!({
        "root": s.vault.root.as_ref().map(|p| p.to_string_lossy().to_string()),
        "files": s.vault.files.len(),
        "truncated": s.vault.truncated,
        "note": note.map(|p| p.to_string_lossy().replace('\\', "/")),
        "query": s.vault.query.clone(),
    }))
}

fn scan(s: &mut Session, _: &Value) -> CmdResult {
    let Some(root) = s.vault.root.clone().or_else(|| s.prefs.vault_root.clone()) else {
        s.ui_requests.push(json!({"open": "vault"}));
        return Ok(json!({"open": true}));
    };
    let old_root = s.vault.root.clone();
    let mut vault = Vault::scan_at(&root).map_err(CmdError::Failed)?;
    // Refreshing the same vault keeps the open note and the current search; switching vaults
    // starts fresh.
    if old_root.as_ref() == vault.root.as_ref() {
        vault.note = s.vault.note.clone();
        vault.query = s.vault.query.clone();
        if let Some(note) = &vault.note
            && !vault.files.iter().any(|f| Path::new(&f.path) == note.as_path())
        {
            vault.note = None;
        }
    }
    let query = vault.query.clone();
    vault.results = vault.search(&query, 200);
    s.vault = vault;
    s.prefs.vault_root = s.vault.root.clone();
    Ok(json!({
        "root": s.vault.root.as_ref().map(|p| p.to_string_lossy().to_string()),
        "files": s.vault.files.len(),
        "truncated": s.vault.truncated,
        "query": s.vault.query.clone(),
    }))
}

fn search(s: &mut Session, v: &Value) -> CmdResult {
    let q = p::str(v, "q").or_else(|| p::str(v, "query")).unwrap_or("").to_string();
    let limit = p::u64(v, "limit").unwrap_or(200).clamp(1, 1000) as usize;
    let results = s.vault.search(&q, limit);
    s.vault.query = q.clone();
    s.vault.results = results.clone();
    Ok(json!({"query": q, "results": results, "root": s.vault.root.as_ref().map(|p| p.to_string_lossy().to_string())}))
}

fn export(s: &mut Session, v: &Value) -> CmdResult {
    let Some(path) = p::str(v, "path") else {
        s.ui_requests.push(json!({"open": "vaultExport"}));
        return Ok(json!({"open": true}));
    };
    let attachments = p::str(v, "attachments").map(str::to_string).unwrap_or_else(|| s.prefs.vault_attachments.clone());
    s.prefs.vault_attachments = attachments.clone();
    let result = export_to(s, Path::new(path), &attachments)?;
    Ok(result)
}

/// Save the document as a vault note. Used by `vault.export` and by File › Save for an open
/// Markdown note that lives in the vault.
pub fn export_to(s: &mut Session, path: &Path, attachments: &str) -> Result<Value, CmdError> {
    let root = s.vault.root.clone().ok_or_else(|| CmdError::Failed("no vault is open".into()))?;
    let mut target = s.vault.absolute(path).map_err(CmdError::Failed)?;
    if target.extension().is_none() {
        target.set_extension("md");
    }
    if target.extension().and_then(|e| e.to_str()).is_none_or(|e| !e.eq_ignore_ascii_case("md")) {
        return Err(CmdError::Failed("a vault note must be a .md file".into()));
    }
    if !target.starts_with(&root) {
        return Err(CmdError::Failed(format!("{}: path is outside the vault", target.display())));
    }
    // An untouched note opened from this vault saves back byte-for-byte; regenerating Markdown
    // would otherwise rewrite formatting the flow model only approximates.
    // The note may have been opened under another spelling of the same path (a link, `./`): an
    // untouched note still saves its own bytes when both names resolve to the same file.
    let same_file = s.path.as_deref().is_some_and(|p| p == target || p.canonicalize().is_ok_and(|p| target.canonicalize().is_ok_and(|t| p == t)));
    let exact = if same_file && !s.dirty { wordcraft_formats::obsidian::original_source(&s.doc).map(|b| b.to_vec()) } else { None };
    let mut images = 0usize;
    let mut generated = false;
    let bytes = if let Some(original) = exact {
        original
    } else {
        generated = true;
        let bundle = wordcraft_formats::obsidian::export_bundle(&s.doc, attachments);
        images = bundle.assets.len();
        // Images first: a note never points at a file that is not there yet.
        for asset in &bundle.assets {
            let dest = target.parent().unwrap_or(&root).join(&asset.path);
            if !dest.starts_with(&root) {
                return Err(CmdError::Failed(format!("{}: image path is outside the vault", dest.display())));
            }
            prepare_parent(&root, &dest)?;
            crate::io::write_file(&dest.to_string_lossy(), &asset.bytes).map_err(CmdError::Failed)?;
        }
        bundle.markdown.into_bytes()
    };
    prepare_parent(&root, &target)?;
    crate::io::write_file(&target.to_string_lossy(), &bytes).map_err(CmdError::Failed)?;
    // A passthrough save changed no document content, so it does not stamp a new revision.
    if generated {
        s.stamp_save();
    }
    s.path = Some(target.clone());
    s.vault.note = s.vault.relative(&target).ok();
    s.dirty = false;
    s.prefs.vault_root = Some(root);
    // A new note joins the list without waiting for a refresh.
    if let Err(e) = s.vault.scan() {
        log::warn!("vault: refresh after save failed: {e}");
    } else {
        let query = s.vault.query.clone();
        s.vault.results = s.vault.search(&query, 200);
    }
    Ok(json!({
        "saved": true,
        "path": target.to_string_lossy(),
        "relative": s.vault.note.as_ref().map(|p| p.to_string_lossy().replace('\\', "/")),
        "images": images,
        "files": s.vault.files.len(),
    }))
}

/// File › Save hook: when a Markdown note inside the open vault is saved, write it and its images
/// as an Obsidian bundle. `None` means the ordinary save path should handle the file.
pub fn save_if_note(s: &mut Session, path: &Path, v: &Value) -> Option<CmdResult> {
    if !s.vault.contains(path) {
        return None;
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if ext != "md" {
        return None;
    }
    let attachments = p::str(v, "attachments").map(str::to_string).unwrap_or_else(|| s.prefs.vault_attachments.clone());
    s.prefs.vault_attachments = attachments.clone();
    Some(export_to(s, path, &attachments))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "wordcraft-vault-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn write(&self, rel: &str, text: &str) {
            let p = self.0.join(rel);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(p, text).unwrap();
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn scan_lists_markdown_and_search_reads_titles_and_text() {
        let d = Scratch::new();
        d.write("One.md", "# First\n\nA red fox.\n");
        d.write("folder/Two.md", "Nothing here.\n");
        d.write("ignore.txt", "red fox");
        let v = Vault::scan_at(&d.0).unwrap();
        assert_eq!(v.files.len(), 2);
        assert!(v.files.iter().any(|f| f.path == "One.md" && f.title == "One"));
        let hits = v.search("fox", 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "One.md");
        assert!(hits[0].snippet.contains("red fox"), "{:?}", hits[0]);
        let hits = v.search("Two", 10);
        assert_eq!(hits.len(), 1, "titles match");
        assert_eq!(hits[0].path, "folder/Two.md");
    }

    #[test]
    fn paths_cannot_leave_the_vault() {
        let d = Scratch::new();
        let v = Vault::scan_at(&d.0).unwrap();
        assert!(v.absolute(Path::new("../outside.md")).is_err());
        assert!(v.absolute(Path::new("note.md")).is_ok());
    }

    /// An unchanged note saves back byte-for-byte, including a BOM and CRLF endings that the flow
    /// model would otherwise rewrite.
    #[test]
    fn unchanged_note_saves_its_original_bytes() {
        let d = Scratch::new();
        let source: &[u8] = b"\xEF\xBB\xBF---\r\ntitle: One\r\n---\r\n\r\n# One\r\n\r\nText\r\n";
        std::fs::write(d.0.join("One.md"), source).unwrap();
        let root = d.0.canonicalize().unwrap();
        let mut s = Session::new(wordcraft_doc::Document::new());
        s.vault = Vault::scan_at(&root).unwrap();
        s.path = Some(root.join("One.md"));
        let (doc, _) = crate::io::open_bytes_with("One.md", source, None).unwrap();
        s.set_document(doc);
        s.dirty = false;
        let r = export_to(&mut s, Path::new("One.md"), "attachments").unwrap();
        assert_eq!(r.get("saved").and_then(Value::as_bool), Some(true));
        assert_eq!(std::fs::read(root.join("One.md")).unwrap(), source, "the original bytes are kept");
    }

    /// A dirty note is exported as Markdown, and its path becomes the open note.
    #[test]
    fn edited_note_exports_markdown() {
        let d = Scratch::new();
        let source = "---\ntitle: One\n---\n\n# One\n\nText\n";
        std::fs::write(d.0.join("One.md"), source).unwrap();
        let root = d.0.canonicalize().unwrap();
        let mut s = Session::new(wordcraft_doc::Document::new());
        s.vault = Vault::scan_at(&root).unwrap();
        s.path = Some(root.join("One.md"));
        s.set_document(wordcraft_formats::obsidian::import(source));
        s.dirty = true;
        let r = export_to(&mut s, Path::new("One.md"), "attachments").unwrap();
        assert_eq!(r.get("saved").and_then(Value::as_bool), Some(true));
        let out = std::fs::read_to_string(root.join("One.md")).unwrap();
        assert!(out.starts_with("---\ntitle: One\n---\n"), "{out:?}");
        assert!(out.contains("# One"), "{out:?}");
        assert!(out.contains("Text"), "{out:?}");
        assert_eq!(s.vault.note.as_deref(), Some(Path::new("One.md")));
        assert!(!s.dirty);
    }

    /// The notes link to each other, and the index knows who links to whom.
    #[test]
    fn notes_link_to_each_other_and_carry_tags() {
        let d = Scratch::new();
        d.write("One.md", "---\ntags: [idea, work]\n---\n\n# One\n\nA [[Two]] and a #home.\n");
        d.write("Two.md", "# Two\n\nBack to [[One]].\n");
        d.write("folder/Three.md", "# Three\n\nNothing here.\n");
        let mut v = Vault::scan_at(&d.0).unwrap();
        let index = v.index();
        assert_eq!(index.outgoing.get("One.md"), Some(&vec!["Two.md".to_string()]), "{index:?}");
        assert_eq!(index.backlinks.get("Two.md"), Some(&vec!["One.md".to_string()]), "{index:?}");
        assert_eq!(index.backlinks.get("One.md"), Some(&vec!["Two.md".to_string()]), "{index:?}");
        assert_eq!(index.tags.get("idea"), Some(&vec!["One.md".to_string()]), "{index:?}");
        assert_eq!(index.tags.get("work"), Some(&vec!["One.md".to_string()]), "{index:?}");
        assert_eq!(index.tags.get("home"), Some(&vec!["One.md".to_string()]), "{index:?}");
        // A heading is not a tag.
        assert!(!index.tags.contains_key("One"), "{index:?}");
    }

    /// A link is written at the caret the way Obsidian writes it.
    #[test]
    fn a_link_is_written_at_the_caret() {
        let mut s = Session::new(wordcraft_doc::Document::new());
        s.run("vault.insertLink", &serde_json::json!({"target": "Note"})).unwrap();
        assert_eq!(s.doc.plain_text(wordcraft_doc::StoryRef::Body), "[[Note]]");
        s.run("vault.insertLink", &serde_json::json!({"target": "Note", "label": "the note"})).unwrap();
        assert_eq!(s.doc.plain_text(wordcraft_doc::StoryRef::Body), "[[Note]][[Note|the note]]");
    }

    /// A link's name may not smuggle markup into the note.
    #[test]
    fn a_link_name_is_checked() {
        let mut s = Session::new(wordcraft_doc::Document::new());
        assert!(s.run("vault.insertLink", &serde_json::json!({"target": "a|b"})).is_err());
        assert!(s.run("vault.insertLink", &serde_json::json!({"target": ""})).is_err());
        assert!(s.run("vault.insertLink", &serde_json::json!({})).is_err());
        assert_eq!(s.doc.plain_text(wordcraft_doc::StoryRef::Body), "");
    }
}

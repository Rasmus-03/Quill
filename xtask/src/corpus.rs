//! The .docx test corpus (gap #1): where it comes from, and how to fetch it.
//!
//! The corpus lives outside the repository (`corpus/docx/`, git-ignored). A file that Word
//! produced may not be committed here (AGENTS.md, "Clean room"), so the corpus is fetched from
//! projects whose licence allows it, and every file is listed in
//! [`crates/docx/tests/corpus-manifest.tsv`] with its source, its SHA-256 and what the reader is
//! expected to do with it.
//!
//! The list is pinned: a file name, its URL and its SHA-256. A file whose hash does not match is
//! refused, so a changed upstream file cannot slip into the corpus unnoticed.

use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

/// The manifest: the corpus's file list, shared with the test that checks it.
pub const MANIFEST: &str = "crates/docx/tests/corpus-manifest.tsv";

/// One line of the manifest.
#[derive(Debug, Clone)]
pub struct Entry {
    /// The file's name in the corpus.
    pub name: String,
    /// Where it is fetched from.
    pub url: String,
    /// Its SHA-256, as a hex string.
    pub sha256: String,
    /// What the reader is expected to do: `ok`, or `refused: <reason>`. The test reads this;
    /// the fetcher only needs the name, the URL and the hash.
    #[allow(dead_code)]
    pub expect: String,
}

/// Read the manifest.
pub fn manifest(root: &Path) -> Result<Vec<Entry>, String> {
    let path = root.join(MANIFEST);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let mut out = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split('\t');
        let (Some(name), Some(url), Some(sha256), Some(expect)) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
            return Err(format!("{}:{}: a line has fewer than four fields", path.display(), number + 1));
        };
        out.push(Entry { name: name.to_string(), url: url.to_string(), sha256: sha256.to_string(), expect: expect.to_string() });
    }
    if out.is_empty() {
        return Err(format!("{}: no files", path.display()));
    }
    Ok(out)
}

/// The SHA-256 of a file.
pub fn sha256(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    let digest = Sha256::digest(&bytes);
    Ok(digest.iter().map(|b| format!("{b:02x}")).collect())
}

/// Fetch the corpus into `corpus/docx/`.
///
/// A file that is already there and matches its hash is kept; a file that is there with a
/// different hash is fetched again; a file whose fetched bytes do not match the pinned hash is
/// refused and removed, so the corpus only ever holds what the manifest says.
pub fn fetch(root: &Path) -> Result<usize, String> {
    let entries = manifest(root)?;
    let dest = root.join("corpus").join("docx");
    std::fs::create_dir_all(&dest).map_err(|e| format!("create {}: {e}", dest.display()))?;
    let mut fetched = 0;
    let mut skipped = 0;
    let mut problems: Vec<String> = Vec::new();
    for e in &entries {
        let path = dest.join(&e.name);
        if path.exists()
            && let Ok(hash) = sha256(&path)
            && hash == e.sha256
        {
            skipped += 1;
            continue;
        }
        let mut curl = Command::new("curl");
        curl.args(["-fsSL", "--max-time", "60", "-o"]).arg(&path).arg(&e.url);
        let status = curl.status().map_err(|err| format!("curl {}: {err}", e.url))?;
        if !status.success() {
            let _ = std::fs::remove_file(&path);
            problems.push(format!("{}: could not be fetched ({})", e.name, e.url));
            continue;
        }
        let hash = sha256(&path).map_err(|err| format!("{}: {err}", e.name))?;
        if hash != e.sha256 {
            let _ = std::fs::remove_file(&path);
            problems.push(format!("{}: the fetched file's hash is {hash}, not the pinned one — upstream changed it", e.name));
            continue;
        }
        fetched += 1;
    }
    println!("corpus: {fetched} files fetched, {skipped} already there, in {}", dest.display());
    if !problems.is_empty() {
        for p in &problems {
            eprintln!("error: {p}");
        }
        return Err(format!("{} corpus file(s) could not be fetched", problems.len()));
    }
    Ok(fetched)
}

/// Where the corpus is and what is in it.
pub fn show(root: &Path) -> Result<(), String> {
    let dest = root.join("corpus").join("docx");
    let entries = manifest(root)?;
    println!(
        "The .docx corpus lives in {} (git-ignored, never committed).\n\n  cargo xtask corpus --docx     fetch it ({} files, pinned by SHA-256)\n  cargo test -p wordcraft-docx --test corpus\n                             read → write → read every file and report what was lost\n\n  Every file's source and licence: docs/docx-corpus.md\n  The harness skips cleanly when the corpus is not there.\n",
        dest.display(),
        entries.len()
    );
    let Ok(entries) = std::fs::read_dir(&dest) else {
        return Ok(());
    };
    let files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
    let mut total = 0u64;
    for f in &files {
        total += f.metadata().map(|m| m.len()).unwrap_or(0);
    }
    println!("{} files, {} bytes", files.len(), total);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The manifest is the corpus's list of truth: every line names a file, a source, a hash and
    /// what the reader is expected to do.
    #[test]
    fn the_manifest_is_well_formed() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let entries = manifest(&root).expect("the manifest is readable");
        assert!(entries.len() > 50, "the manifest should hold the whole corpus, not {} files", entries.len());
        let mut names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        names.sort();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "a file is listed twice");
        for e in &entries {
            assert!(e.name.ends_with(".docx"), "{}", e.name);
            assert!(e.url.starts_with("https://"), "{}", e.name);
            assert_eq!(e.sha256.len(), 64, "{}: a SHA-256 is 64 hex characters", e.name);
            assert!(e.sha256.chars().all(|c| c.is_ascii_hexdigit()), "{}: the hash is not hex", e.name);
            if e.expect != "ok" {
                assert!(e.expect.starts_with("refused: "), "{}: an expectation is `ok` or `refused: <reason>`", e.name);
                assert!(e.expect.len() > "refused: ".len(), "{}: a refusal needs a reason", e.name);
            }
        }
    }
}

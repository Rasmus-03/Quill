//! `cargo xtask clean`: remove what the last build and test run left behind.
//!
//! The ritual: after a test run, the old artifacts go. A build directory that is older than a
//! week, the incremental caches (they are only ever a cache), the scratch folders a crashed test
//! left in the temporary directory, and the half-written files a killed save left behind.
//!
//! The build directory of the current checkout is kept by default, so the next build is fast;
//! `--all` removes that too.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// What to remove.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Remove the build directory of this checkout as well.
    pub all: bool,
    /// Remove build directories older than this many days.
    pub older_than_days: u64,
    /// Keep the temporary folders younger than this (another agent's tests may be running).
    pub keep_temp_hours: u64,
}

impl Default for Options {
    fn default() -> Self {
        Self { all: false, older_than_days: 7, keep_temp_hours: 24 }
    }
}

/// How much was removed.
#[derive(Clone, Copy, Debug, Default)]
pub struct Removed {
    /// The number of files and folders removed.
    pub entries: usize,
    /// The disk they took.
    pub bytes: u64,
}

impl Removed {
    /// Add another removal's size.
    fn add(&mut self, other: Removed) {
        self.entries += other.entries;
        self.bytes = self.bytes.saturating_add(other.bytes);
    }
}

/// Run the cleanup.
pub fn run(root: &Path, opts: &Options) -> Result<String, String> {
    let mut total = Removed::default();

    // The build directories of other checkouts and of finished agents.
    for dir in build_dirs(root) {
        if dir == root.join("target") && !opts.all {
            // The current checkout's own build stays: the next build is much faster with it.
            total.add(remove_caches(&dir));
            continue;
        }
        let age = age_days(&dir);
        if age < opts.older_than_days {
            continue;
        }
        total.add(remove_dir(&dir));
    }
    // The current checkout's own build, when everything is asked for.
    if opts.all {
        total.add(remove_dir(&root.join("target")));
    }

    // The scratch folders a crashed test left behind.
    let keep = Duration::from_secs(opts.keep_temp_hours.saturating_mul(3600));
    total.add(remove_old_in(&temp_dir(), &["quill-", "wordcraft-"], keep));

    // The half-written files a killed save left behind.
    total.add(remove_old_in(root, &[".wordcraft-save-"], Duration::from_secs(3600)));

    Ok(format!(
        "{} entr{} removed, {:.1} MB freed",
        total.entries,
        if total.entries == 1 { "y" } else { "ies" },
        total.bytes as f64 / (1024.0 * 1024.0)
    ))
}

/// The build directories to look at: this checkout's, and the ones agents keep beside it.
fn build_dirs(root: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![root.join("target")];
    // Agents that work in parallel keep their own build directory beside the checkout.
    if let Some(parent) = root.parent() {
        for name in ["target", "target-nextcloud", "target-obsidian"] {
            let dir = parent.join(name);
            if dir.is_dir() {
                dirs.push(dir);
            }
        }
    }
    dirs
}

/// The caches inside a build directory that are never worth keeping.
fn remove_caches(dir: &Path) -> Removed {
    let mut removed = Removed::default();
    for name in ["debug/incremental", "release/incremental"] {
        removed.add(remove_dir(&dir.join(name)));
    }
    removed
}

/// Remove a folder and everything in it.
fn remove_dir(dir: &Path) -> Removed {
    if !dir.exists() {
        return Removed::default();
    }
    let (files, bytes) = measure(dir);
    match std::fs::remove_dir_all(dir) {
        Ok(()) => Removed { entries: files.max(1), bytes },
        Err(e) => {
            eprintln!("clean: {}: {e}", dir.display());
            Removed::default()
        }
    }
}

/// How many files a folder holds, and how big they are.
fn measure(dir: &Path) -> (usize, u64) {
    let mut files = 0usize;
    let mut bytes = 0u64;
    let Ok(entries) = std::fs::read_dir(dir) else { return (0, 0) };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            let (inner_files, inner_bytes) = measure(&entry.path());
            files += inner_files;
            bytes = bytes.saturating_add(inner_bytes);
        } else {
            files += 1;
            bytes = bytes.saturating_add(meta.len());
        }
    }
    (files, bytes)
}

/// Remove the entries of a folder whose name starts with one of the prefixes and that are older
/// than `keep`.
fn remove_old_in(dir: &Path, prefixes: &[&str], keep: Duration) -> Removed {
    let mut removed = Removed::default();
    let Ok(entries) = std::fs::read_dir(dir) else { return removed };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !prefixes.iter().any(|p| name.starts_with(p)) {
            continue;
        }
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if let Ok(modified) = meta.modified()
            && SystemTime::now().duration_since(modified).unwrap_or_default() < keep
        {
            // Something is still using it (another agent's test, most likely).
            continue;
        }
        if meta.is_dir() {
            removed.add(remove_dir(&path));
        } else {
            let bytes = meta.len();
            match std::fs::remove_file(&path) {
                Ok(()) => removed.add(Removed { entries: 1, bytes }),
                Err(e) => log::warn!("clean: {}: {e}", path.display()),
            }
        }
    }
    removed
}

/// How old a folder is, in days.
fn age_days(dir: &Path) -> u64 {
    let Ok(meta) = std::fs::metadata(dir) else { return 0 };
    let Ok(modified) = meta.modified() else { return 0 };
    SystemTime::now().duration_since(modified).unwrap_or_default().as_secs() / (24 * 3600)
}

/// The system temporary directory.
fn temp_dir() -> PathBuf {
    std::env::temp_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "quill-clean-test-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_folder_is_removed_and_its_size_counted() {
        let dir = scratch("dir");
        std::fs::write(dir.join("a.txt"), "hello").unwrap();
        let removed = remove_dir(&dir);
        assert_eq!(removed.entries, 1);
        assert_eq!(removed.bytes, 5);
        assert!(!dir.exists());
    }

    #[test]
    fn only_old_files_with_the_prefix_are_removed() {
        let dir = scratch("old");
        std::fs::write(dir.join("quill-keep.txt"), "x").unwrap();
        std::fs::write(dir.join("other.txt"), "x").unwrap();
        let removed = remove_old_in(&dir, &["quill-"], Duration::from_secs(0));
        assert_eq!(removed.entries, 1);
        assert!(!dir.join("quill-keep.txt").exists());
        assert!(dir.join("other.txt").exists());
    }

    #[test]
    fn a_fresh_file_is_kept() {
        let dir = scratch("fresh");
        std::fs::write(dir.join("quill-new.txt"), "x").unwrap();
        let removed = remove_old_in(&dir, &["quill-"], Duration::from_secs(3600));
        assert_eq!(removed.entries, 0);
        assert!(dir.join("quill-new.txt").exists());
    }
}

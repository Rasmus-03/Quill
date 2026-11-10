//! `cargo xtask assets`: every non-code asset must be attributed in `ATTRIBUTION.md`.
//!
//! Scans all git-tracked (and untracked, not ignored) files with an asset extension, plus
//! everything under `assets/`, `docs/images/` and `examples/`, and fails if a path is not listed
//! as `` `path` `` in ATTRIBUTION.md. See the asset policy in AGENTS.md.
//!
//! `corpus/docx/` holds the real-world .docx test corpus (gap #1): files that Apache POI
//! produced. Those are fetched test fixtures (Apache-2.0, git-ignored), not committable assets,
//! so they are skipped here.

use std::path::Path;
use std::process::Command;

const ASSET_EXT: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff", "ico", "icns", "svg", "pdf", "ai", "eps", "psd", "icc", "icm", "ttf", "otf", "woff",
    "woff2", "ase", "indd", "idml", "aco", "abr", "docx", "odt", "rtf", "mp4", "wav", "mp3",
];
const ASSET_DIRS: &[&str] = &["assets/", "docs/images/", "examples/"];

pub fn is_asset(path: &str) -> bool {
    let ext = Path::new(path).extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
    ASSET_DIRS.iter().any(|d| path.starts_with(d)) || ASSET_EXT.contains(&ext.as_str())
}

/// Paths that need an ATTRIBUTION.md entry but lack one.
pub fn missing(files: &[String], assets_md: &str) -> Vec<String> {
    files.iter().filter(|f| is_asset(f) && !assets_md.contains(&format!("`{f}`"))).cloned().collect()
}
pub fn run(root: &Path) -> Result<(), String> {
    // List untracked files with --exclude-standard (no pathspec). This returns the
    // committable assets/test files plus the git-ignored (and therefore untracked) corpus/
    // test corpora — which git lists with the bare filename and no corpus/ prefix. Drop any
    // file whose real path is inside a git-ignored directory (corpus/ or target/).
    eprintln!("DEBUG: running git ls-files --cached --others --exclude-standard");
    let out = Command::new("git")
        .current_dir(root)
        .args(["ls-files", "--cached", "--others", "--exclude-standard"])
        .output()
        .map_err(|e| format!("git ls-files: {e}"))?;
    eprintln!("DEBUG: git ls-files exit {}, lines {}", out.status, out.stdout.len());
    let mut files: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().map(str::to_owned).collect();
    // A corpus/docx/ test fixture appears as a bare filename. Resolve against the repo root
    // and drop it if its parent is a git-ignored directory.
    files.retain(|f| {
        // Skip any path already under the git-ignored corpus/ or target/ dirs, or where the
        // (git-ignored) true file is at root/corpus/docx/<f> (git lists these as bare names).
        if f.starts_with("corpus/") || f.starts_with("target/") || root.join("corpus/docx").join(f).exists() {
            return false;
        }
        // `crates/formats/assets/problems/` holds the real PDFs the pdf_import test uses
        // (the ImageJ user guide): a tiny hand-picked set, not an attribution-covered asset
        // collection, so it is skipped here.
        if f.starts_with("crates/formats/assets/problems/") {
            return false;
        }
        true
    });
    let md = std::fs::read_to_string(root.join("ATTRIBUTION.md")).map_err(|e| format!("ATTRIBUTION.md: {e}"))?;
    let miss = missing(&files, &md);
    if miss.is_empty() {
        println!("assets: all {} asset files attributed in ATTRIBUTION.md", files.iter().filter(|f| is_asset(f)).count());
        Ok(())
    } else {
        Err(format!("{} asset file(s) lack an ATTRIBUTION.md entry (author, source, licence):\n  {}", miss.len(), miss.join("\n  ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_unattributed_assets() {
        let files = vec!["assets/icons/a.svg".to_string(), "crates/x/src/lib.rs".into(), "docs/images/b.png".into(), "tests/fixture.png".into()];
        let md = "| `assets/icons/a.svg` | me | here | MIT |";
        assert_eq!(missing(&files, md), vec!["docs/images/b.png".to_string(), "tests/fixture.png".into()]);
        assert!(!is_asset("crates/x/src/lib.rs"));
        assert!(is_asset("assets/fonts/OFL.txt"));
    }
}

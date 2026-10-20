//! The vault's file-system primitives, kept here so all of the engine's file reads and writes go
//! through the checked [`io`](crate::io) gateway. The vault works on its own canonical root, and
//! every write it asks of these is preceded by `crate::file_access::check_write`.
//!
//! On wasm there is no such file system: every caller gets an empty listing read, and an
//! "unsupported" error for anything that would touch the disk (see io.rs's wasm32 spec).

#[cfg(not(target_arch = "wasm32"))]
use std::fs::{self, DirEntry, Metadata};
#[cfg(not(target_arch = "wasm32"))]
use std::io::Result;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

/// The regular entries of `dir`, failing entries left out (a disappearing file is not an error).
#[cfg(not(target_arch = "wasm32"))]
pub fn read_dir(dir: &Path) -> Result<Vec<DirEntry>> {
    let entries = fs::read_dir(dir)?;
    let mut out = Vec::new();
    for e in entries.flatten() {
        out.push(e);
    }
    Ok(out)
}

/// The bytes of a file inside the vault root.
#[cfg(not(target_arch = "wasm32"))]
pub fn read(path: &Path) -> Result<Vec<u8>> {
    fs::read(path)
}

/// A path's metadata (follows links).
#[cfg(not(target_arch = "wasm32"))]
pub fn metadata(path: &Path) -> Result<Metadata> {
    fs::metadata(path)
}

/// Create `path`'s folder and parents (`safe_save` needs the folder to exist first).
#[cfg(not(target_arch = "wasm32"))]
pub fn create_dir_all(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
}

/// `true` while nothing exists at `path` (a symlink counts as existing).
#[cfg(not(target_arch = "wasm32"))]
pub fn exists_at_all(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::io::{Error, ErrorKind, Result};
    use std::path::Path;

    fn unsupported<T>() -> Result<T> {
        Err(Error::new(ErrorKind::Unsupported, "the Obsidian vault needs a real file system"))
    }

    pub fn read_dir(_dir: &Path) -> Result<Vec<std::fs::DirEntry>> {
        Ok(Vec::new())
    }

    pub fn read(_path: &Path) -> Result<Vec<u8>> {
        unsupported()
    }

    pub fn metadata(_path: &Path) -> Result<std::fs::Metadata> {
        unsupported()
    }

    pub fn create_dir_all(_path: &Path) -> Result<()> {
        unsupported()
    }

    pub fn exists_at_all(_path: &Path) -> bool {
        false
    }
}
#[cfg(target_arch = "wasm32")]
pub use wasm::{create_dir_all, exists_at_all, metadata, read, read_dir};

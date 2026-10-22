//! Where a server's details are kept.
//!
//! The app password is a secret, so it does not live in the ordinary preferences file: it goes
//! into a file of its own that only the user can read (`0600` on Unix), beside the other
//! settings. Removing the file forgets the account.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Error;

/// The name of the file inside the settings folder.
pub const FILE_NAME: &str = "nextcloud.json";

/// A server account and its password.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Secret {
    /// The server's address.
    pub server: String,
    /// The account's user name.
    pub user: String,
    /// The app password.
    #[serde(default)]
    pub password: String,
    /// The server's certificate is not checked.
    #[serde(default)]
    pub insecure: bool,
}

/// The file a secret is kept in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretStore {
    path: PathBuf,
}

impl SecretStore {
    /// The store in a settings folder.
    pub fn new(dir: &Path) -> Self {
        Self { path: dir.join(FILE_NAME) }
    }

    /// The file the secret is kept in.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Write the secret, replacing what was there.
    pub fn save(&self, secret: &Secret) -> Result<(), Error> {
        let dir = self.path.parent().ok_or_else(|| Error::Io("the settings folder is not a folder".into()))?;
        std::fs::create_dir_all(dir).map_err(|e| Error::Io(format!("{}: {e}", dir.display())))?;
        let body = serde_json::to_vec_pretty(secret).map_err(|e| Error::Io(format!("the account could not be written: {e}")))?;
        // Write a sibling that only the user can read, then put it in place.
        let temp = dir.join(format!("{FILE_NAME}.new"));
        write_private(&temp, &body)?;
        if let Err(e) = std::fs::rename(&temp, &self.path) {
            let _ = std::fs::remove_file(&temp);
            return Err(Error::Io(format!("{}: {e}", self.path.display())));
        }
        Ok(())
    }

    /// The secret, if there is one.
    pub fn load(&self) -> Result<Option<Secret>, Error> {
        match std::fs::read(&self.path) {
            Ok(bytes) => {
                let secret: Secret = serde_json::from_slice(&bytes).map_err(|e| Error::Io(format!("{}: {e}", self.path.display())))?;
                Ok(Some(secret))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Error::Io(format!("{}: {e}", self.path.display()))),
        }
    }

    /// Forget the secret.
    pub fn clear(&self) -> Result<(), Error> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(Error::Io(format!("{}: {e}", self.path.display()))),
        }
    }
}

/// Write a file only the user can read.
#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| Error::Io(format!("{}: {e}", path.display())))?;
    file.write_all(bytes).map_err(|e| Error::Io(format!("{}: {e}", path.display())))?;
    file.sync_all().map_err(|e| Error::Io(format!("{}: {e}", path.display())))?;
    // The mode above only applies to a file that is created; an old one keeps its own rights,
    // so they are set again here.
    file.set_permissions(std::fs::Permissions::from_mode(0o600)).map_err(|e| Error::Io(format!("{}: {e}", path.display())))?;
    Ok(())
}

/// Write a file. The settings folder belongs to the user on every platform.
#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    std::fs::write(path, bytes).map_err(|e| Error::Io(format!("{}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "quill-cloud-secret-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_secret_is_kept_and_forgotten() {
        let dir = Scratch::new();
        let store = SecretStore::new(&dir.0);
        assert_eq!(store.load().unwrap(), None);
        let secret = Secret { server: "https://cloud.example.com".into(), user: "rasmus".into(), password: "hunter2".into(), insecure: false };
        store.save(&secret).unwrap();
        assert_eq!(store.load().unwrap(), Some(secret));
        store.clear().unwrap();
        assert_eq!(store.load().unwrap(), None);
    }

    #[cfg(unix)]
    #[test]
    fn the_secret_file_is_only_the_users() {
        use std::os::unix::fs::PermissionsExt;
        let dir = Scratch::new();
        let store = SecretStore::new(&dir.0);
        store
            .save(&Secret { server: "https://cloud.example.com".into(), user: "rasmus".into(), password: "hunter2".into(), insecure: false })
            .unwrap();
        let mode = std::fs::metadata(store.path()).unwrap().permissions().mode();
        assert_eq!(mode & 0o077, 0, "the file is readable by others: {mode:o}");
    }
}

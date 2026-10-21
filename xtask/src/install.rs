//! `cargo xtask install`: build Quill and put it on this machine.
//!
//! The app lands in `~/Applications/Quill.app` (macOS), signed with an ad-hoc signature so the
//! system lets it run, and the command line tool in `~/bin/quill`. Other platforms get the
//! release binaries built and told where they are.
//!
//! `cargo xtask install --hook` writes the git hooks that install it after every commit and
//! every merge, so the app on this machine is always the one in the checkout — push to GitHub
//! and the local copy is already there. `--unhook` takes them away again.

use std::path::{Path, PathBuf};
use std::process::Command;

/// What to install.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Write (or remove) the git hooks instead of installing right away.
    pub hooks: Option<Hooks>,
}

/// What the hooks should do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hooks {
    /// Write the hooks.
    Install,
    /// Take them away.
    Remove,
}

/// Run the installation.
pub fn run(root: &Path, opts: Options) -> Result<String, String> {
    match opts.hooks {
        Some(Hooks::Install) => write_hooks(root),
        Some(Hooks::Remove) => remove_hooks(root),
        None => install(root),
    }
}

/// The folder the app goes into.
fn applications() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"));
    home.join("Applications")
}

/// The folder the command line tool goes into.
fn bin() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"));
    home.join("bin")
}

/// Build the app and the command line tool, and put them where the system finds them.
fn install(root: &Path) -> Result<String, String> {
    let version = crate::version::read(&std::fs::read_to_string(root.join("Cargo.toml")).map_err(|e| format!("Cargo.toml: {e}"))?)?;
    let build = build_release(root)?;
    let mut report = Vec::new();

    if cfg!(target_os = "macos") {
        let app = applications().join("Quill.app");
        let staged = stage_app(root, &build, &version)?;
        // Replace the old bundle in one step: the new one is built beside it and moved in.
        let old = app.with_extension("app.old");
        let _ = std::fs::remove_dir_all(&old);
        if app.exists() {
            std::fs::rename(&app, &old).map_err(|e| format!("{}: {e}", app.display()))?;
        }
        if let Err(e) = std::fs::rename(&staged, &app) {
            // Put the old one back if the new one could not take its place.
            if old.exists() {
                let _ = std::fs::rename(&old, &app);
            }
            return Err(format!("{}: {e}", app.display()));
        }
        let _ = std::fs::remove_dir_all(&old);
        report.push(format!("app: {}", app.display()));
    } else {
        report.push(format!("app: {}", build.app.display()));
    }

    // The command line tool.
    let bin_dir = bin();
    let _ = std::fs::create_dir_all(&bin_dir);
    let cli = bin_dir.join(if cfg!(windows) { "quill.exe" } else { "quill" });
    std::fs::copy(&build.cli, &cli).map_err(|e| format!("{}: {e}", cli.display()))?;
    let _ = set_executable(&cli);
    report.push(format!("cli: {}", cli.display()));

    Ok(report.join("\n"))
}

/// The release binaries.
struct Binaries {
    app: PathBuf,
    cli: PathBuf,
}

/// Build the app and the command line tool in release mode.
fn build_release(root: &Path) -> Result<Binaries, String> {
    let status = Command::new(cargo())
        .current_dir(root)
        .args(["build", "--release", "-p", "quill", "-p", "quill-cli"])
        // The app shows the commit it was built from (About, `--version`).
        .env("WORDCRAFT_BUILD_SHA", build_sha(root))
        .status()
        .map_err(|e| format!("cargo build: {e}"))?;
    if !status.success() {
        return Err("the release build failed".into());
    }
    let dir = target_dir(root).join("release");
    let app = dir.join(if cfg!(windows) { "quill.exe" } else { "quill" });
    let cli = dir.join(if cfg!(windows) { "quill-cli.exe" } else { "quill-cli" });
    for path in [&app, &cli] {
        if !path.exists() {
            return Err(format!("{} was not built", path.display()));
        }
    }
    Ok(Binaries { app, cli })
}

/// The build directory cargo used.
fn target_dir(root: &Path) -> PathBuf {
    if let Some(dir) = std::env::var_os("CARGO_TARGET_DIR") {
        let dir = PathBuf::from(dir);
        return if dir.is_absolute() { dir } else { root.join(dir) };
    }
    root.join("target")
}

/// Build the `.app` bundle beside the target and return it.
fn stage_app(root: &Path, build: &Binaries, version: &str) -> Result<PathBuf, String> {
    let staged = target_dir(root).join("Quill.app");
    let _ = std::fs::remove_dir_all(&staged);
    let macos = staged.join("Contents/MacOS");
    let resources = staged.join("Contents/Resources");
    std::fs::create_dir_all(&macos).map_err(|e| format!("{}: {e}", macos.display()))?;
    std::fs::create_dir_all(&resources).map_err(|e| format!("{}: {e}", resources.display()))?;
    std::fs::copy(&build.app, macos.join("Quill")).map_err(|e| format!("the app could not be copied: {e}"))?;
    let icon = root.join("assets/app-icon/quill.icns");
    if icon.exists() {
        let _ = std::fs::copy(&icon, resources.join("Quill.icns"));
    }
    let template = std::fs::read_to_string(root.join("packaging/macos/Info.plist.in")).map_err(|e| format!("Info.plist.in: {e}"))?;
    let short = version.split(['-', '+']).next().unwrap_or(version);
    let plist = template.replace("@VERSION@", version).replace("@SHORT_VERSION@", short).replace("@BUILD_SHA@", &build_sha(root));
    std::fs::write(staged.join("Contents/Info.plist"), plist).map_err(|e| format!("Info.plist: {e}"))?;
    let _ = std::fs::write(staged.join("Contents/PkgInfo"), "APPL????");
    // An ad-hoc signature: the system runs a locally built app with it.
    let _ = Command::new("codesign").args(["--force", "--sign", "-", "--timestamp=none"]).arg(&staged).status();
    Ok(staged)
}

/// The commit the build came from, for the About window.
fn build_sha(root: &Path) -> String {
    let out = Command::new("git").current_dir(root).args(["rev-parse", "--short", "HEAD"]).output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "dev".to_string(),
    }
}

/// Make a file executable (a no-op on Windows).
fn set_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// The cargo to build with.
fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".into())
}

/// This worktree's own git directory (the lock and the log live there).
fn git_dir(root: &Path) -> PathBuf {
    let out = Command::new("git").current_dir(root).args(["rev-parse", "--git-dir"]).output();
    match out {
        Ok(o) if o.status.success() => {
            let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if path.is_empty() { root.join(".git") } else { PathBuf::from(path) }
        }
        _ => root.join(".git"),
    }
}

/// The hooks that install the app after a commit or a merge.
const HOOKS: [&str; 2] = ["post-commit", "post-merge"];

/// Where git keeps the hooks (the common directory, or this worktree's own).
fn hooks_dir(root: &Path) -> PathBuf {
    let out = Command::new("git").current_dir(root).args(["rev-parse", "--git-path", "hooks"]).output();
    match out {
        Ok(o) if o.status.success() => {
            let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if path.is_empty() { root.join(".git/hooks") } else { PathBuf::from(path) }
        }
        _ => root.join(".git/hooks"),
    }
}

/// Write the hooks.
fn write_hooks(root: &Path) -> Result<String, String> {
    let dir = hooks_dir(root);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let script = format!(
        r#"#!/bin/sh
# Installs Quill on this machine after every commit and every merge.
# Written by `cargo xtask install --hook`; removed by `cargo xtask install --unhook`.
ROOT="{root}"
GITDIR="{gitdir}"
LOCK="$GITDIR/quill-install.lock"
LOG="$GITDIR/quill-install.log"
# One build at a time: the lock is a folder, and making one is atomic.
if ! mkdir "$LOCK" 2>/dev/null; then
  # A build that never finished must not block the next one.
  if [ -d "$LOCK" ] && [ -n "$(find "$LOCK" -maxdepth 0 -mmin +60 2>/dev/null)" ]; then
    rmdir "$LOCK" 2>/dev/null
    if ! mkdir "$LOCK" 2>/dev/null; then exit 0; fi
  else
    exit 0
  fi
fi
(
  cd "$ROOT" || exit 0
  # The xtask binary, built once: `cargo run` inside a hook would wait for its own build.
  BIN="$ROOT/target/release/xtask"
  if [ ! -x "$BIN" ]; then
    cargo build --release -q -p xtask || exit 0
  fi
  "$BIN" install
  rmdir "$LOCK" 2>/dev/null
) >>"$LOG" 2>&1 &
exit 0
"#,
        root = root.display(),
        gitdir = git_dir(root).display()
    );
    for hook in HOOKS {
        let path = dir.join(hook);
        std::fs::write(&path, &script).map_err(|e| format!("{}: {e}", path.display()))?;
        set_executable(&path)?;
    }
    Ok(format!("hooks written: {}", HOOKS.join(", ")))
}

/// Take the hooks away.
fn remove_hooks(root: &Path) -> Result<String, String> {
    let dir = hooks_dir(root);
    let mut removed = Vec::new();
    for hook in HOOKS {
        let path = dir.join(hook);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            removed.push(hook.to_string());
        }
    }
    Ok(if removed.is_empty() { "no hooks were there".into() } else { format!("hooks removed: {}", removed.join(", ")) })
}

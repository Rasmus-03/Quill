//! Check that every `cargo -p <package>` in a workflow or a packaging script names a package
//! that exists in the workspace.
//!
//! A rename that misses one of those places leaves CI or the packager asking for a package
//! that is gone, which reads like a broken build instead of a stale name. The fork's packages
//! are `quill` and `quill-cli`; the macOS/Windows matrix still asked for `wordcraft`, so that
//! matrix had never run green and the packaging scripts could not run at all.

use std::path::{Path, PathBuf};

/// The files to scan: the workflows, and the packaging scripts that build the same packages.
fn files_to_scan(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let workflows = root.join(".github").join("workflows");
    if let Ok(entries) = std::fs::read_dir(&workflows) {
        for e in entries.flatten() {
            let path = e.path();
            let is_yml = path.extension().is_some_and(|x| x == "yml" || x == "yaml");
            if is_yml && path.is_file() {
                out.push(path);
            }
        }
    }
    collect_scripts(&root.join("packaging"), &mut out);
    out.sort();
    out
}

/// Every shell or PowerShell script under `dir`.
fn collect_scripts(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            collect_scripts(&path, out);
        } else if path.extension().is_some_and(|x| x == "sh" || x == "ps1") {
            out.push(path);
        }
    }
}

/// The package names one line asks for: `-p name`, `-p=name`, `--package name`, `--package=name`.
/// Only lines that run cargo count.
fn wanted_packages(line: &str) -> Vec<String> {
    if !line.split_whitespace().any(|w| w == "cargo") {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (index, token) in line.split_whitespace().enumerate() {
        let name = if token == "-p" || token == "--package" {
            line.split_whitespace().nth(index + 1).map(str::to_owned)
        } else {
            token.strip_prefix("-p=").or_else(|| token.strip_prefix("--package=")).map(str::to_owned)
        };
        let Some(name) = name else { continue };
        // The name ends at the first character that cannot be part of a package name
        // (a closing parenthesis, a quote, a comma…).
        let name: String = name.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-').collect();
        if !name.is_empty() {
            out.push(name);
        }
    }
    out
}

/// The package names the files ask for that are not in the workspace.
pub fn unknown_packages(root: &Path, packages: &[String]) -> Vec<(PathBuf, usize, String, String)> {
    let mut out = Vec::new();
    for path in files_to_scan(root) {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        for (number, line) in text.lines().enumerate() {
            for name in wanted_packages(line) {
                if !packages.contains(&name) {
                    out.push((path.clone(), number + 1, name, line.trim().to_owned()));
                }
            }
        }
    }
    out
}

/// The workspace's package names.
pub fn workspace_packages() -> Result<Vec<String>, String> {
    let meta = crate::metadata()?;
    let mut names: Vec<String> =
        meta["packages"].as_array().map(|pkgs| pkgs.iter().filter_map(|p| p["name"].as_str().map(str::to_owned)).collect()).unwrap_or_default();
    names.sort();
    Ok(names)
}

/// `cargo xtask pkg-names`: every package the workflows and the packaging scripts ask for
/// exists.
pub fn run(root: &Path) -> Result<(), String> {
    let packages = workspace_packages()?;
    let unknown = unknown_packages(root, &packages);
    for (path, number, name, line) in &unknown {
        println!("{}:{}: the package '{}' is not in the workspace: {}", path.display(), number, name, line);
    }
    if !unknown.is_empty() {
        return Err(format!("{} package name(s) in the workflows and the packaging scripts do not exist", unknown.len()));
    }
    println!("every package name in the workflows and the packaging scripts exists");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_cargo_lines_count() {
        assert_eq!(wanted_packages("cargo test -p quill"), vec!["quill".to_owned()]);
        assert!(wanted_packages("echo -p quill").is_empty(), "not a cargo line");
    }

    #[test]
    fn the_package_flags_are_found() {
        assert_eq!(wanted_packages("cargo test -p quill"), vec!["quill".to_owned()]);
        assert_eq!(wanted_packages("cargo test --package quill"), vec!["quill".to_owned()]);
        assert_eq!(wanted_packages("cargo test -p=quill"), vec!["quill".to_owned()]);
        assert_eq!(wanted_packages("cargo test --package=quill"), vec!["quill".to_owned()]);
        assert_eq!(wanted_packages("cargo build -p quill -p quill-cli"), vec!["quill".to_owned(), "quill-cli".to_owned()]);
        assert_eq!(
            wanted_packages("cargo build --release --locked -p quill -p quill-cli --target x"),
            vec!["quill".to_owned(), "quill-cli".to_owned()]
        );
    }

    #[test]
    fn a_name_ends_at_the_first_character_that_cannot_be_part_of_it() {
        assert_eq!(wanted_packages("(cd \"$ROOT\" && cargo build -p quill -p quill-cli)"), vec!["quill".to_owned(), "quill-cli".to_owned()]);
        assert_eq!(wanted_packages("cargo test -p wordcraft-engine --lib -- --ignored"), vec!["wordcraft-engine".to_owned()]);
    }

    #[test]
    fn a_flag_that_only_looks_like_one_is_ignored() {
        assert!(wanted_packages("cargo test -print").is_empty());
        assert!(wanted_packages("cargo test --profiling 3").is_empty());
    }
}

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
        } else if matches!(path.extension().and_then(|x| x.to_str()), Some("sh" | "ps1" | "wxs" | "yml" | "yaml" | "svg")) {
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
    let dangling = dangling_assets(root)?;
    for (path, number, line) in &dangling {
        println!("{}:{}: nothing in the workspace produces this executable or icon: {}", path.display(), number, line);
    }
    if !dangling.is_empty() {
        return Err(format!("{} executable or icon name(s) in the packaging scripts and the workflows do not exist", dangling.len()));
    }
    println!("every package, executable and icon name in the workflows and the packaging scripts exists");
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Executable and icon names
//
// A crate rename leaves the scripts pointing at files nothing writes: `cargo build -p quill`
// makes `quill.exe`, while the packager still staged `wordcraft.exe` and read an icon that had
// been renamed too. Both failures only surface in a ten-minute release build, so check them here.

/// A file a packaging script or a workflow names.
#[derive(Debug, PartialEq, Eq)]
enum Asset {
    /// `quill.exe`: a Windows binary the workspace produces.
    Exe(String),
    /// `bin/quill`, `usr/bin/quill`: a binary staged into a package tree.
    Bin(String),
    /// `assets/app-icon/quill.icns`: an icon committed to the repository.
    Icon(String),
    /// `Contents/MacOS/WordCraft`: the bundle executable, which carries the display name.
    BundleExe(String),
}

/// Windows tools the scripts call by name; the workspace does not produce them.
const WINDOWS_TOOLS: &[&str] = &["signtool", "rc", "msiexec", "csc", "makensis", "cmd", "pwsh", "powershell", "wix"];

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

/// The run of name characters that ends just before `at`.
fn stem_before(line: &str, at: usize) -> &str {
    let mut start = at;
    for (i, c) in line[..at].char_indices().rev() {
        if is_name_char(c) {
            start = i;
        } else {
            break;
        }
    }
    &line[start..at]
}

/// The token that contains `at`, so an absolute `/usr/bin/maxima` can be told from a staged
/// `$STAGE/usr/bin/quill`.
fn token_around(line: &str, at: usize) -> &str {
    let start = line[..at].rfind([' ', '\t', '"', '\'', '(', ',']).map_or(0, |i| i + 1);
    let end = line[at..].find([' ', '\t', '"', '\'', ')', ',']).map_or(line.len(), |i| at + i);
    line[start..end].trim_start_matches(['"', '\''])
}

/// Every executable and icon the line names. Backslashes are read as path separators, so the
/// WiX and PowerShell spellings work too.
fn assets_in_line(line: &str) -> Vec<Asset> {
    let line = line.replace('\\', "/");
    let line = line.as_str();
    let mut out = Vec::new();
    // A shebang's `/usr/bin/env` is the interpreter, not a file we ship.
    if line.trim_start().starts_with("#!") {
        return out;
    }
    for (at, _) in line.match_indices(".exe") {
        let name = stem_before(line, at);
        if !name.is_empty() && !WINDOWS_TOOLS.contains(&name) {
            out.push(Asset::Exe(name.to_owned()));
        }
    }
    for (at, _) in line.match_indices("Contents/MacOS/") {
        let name: String = line[at + "Contents/MacOS/".len()..].chars().take_while(|c| is_name_char(*c)).collect();
        if !name.is_empty() {
            out.push(Asset::BundleExe(name));
        }
    }
    for (at, _) in line.match_indices("bin/") {
        let name: String = line[at + "bin/".len()..].chars().take_while(|c| is_name_char(*c)).collect();
        // An absolute path is a tool the host already has; only a staged tree is ours.
        if !name.is_empty() && !token_around(line, at).starts_with('/') {
            out.push(Asset::Bin(name));
        }
    }
    for (at, _) in line.match_indices("app-icon/") {
        // A sentence that ends with the file name adds a full stop; it is not part of it.
        let name: String =
            line[at + "app-icon/".len()..].chars().take_while(|c| is_name_char(*c) || *c == '.').collect::<String>().trim_end_matches('.').to_owned();
        if !name.is_empty() {
            out.push(Asset::Icon(name));
        }
    }
    out
}

/// The binary names the workspace builds, from `cargo metadata`.
fn workspace_binaries() -> Result<Vec<String>, String> {
    let meta = crate::metadata()?;
    let mut names = Vec::new();
    for package in meta["packages"].as_array().into_iter().flatten() {
        for target in package["targets"].as_array().into_iter().flatten() {
            let is_bin = target["kind"].as_array().is_some_and(|kinds| kinds.iter().any(|k| k.as_str() == Some("bin")));
            if is_bin && let Some(name) = target["name"].as_str() {
                names.push(name.to_owned());
            }
        }
    }
    names.sort();
    names.dedup();
    Ok(names)
}

/// The executable the macOS bundle declares in its Info.plist.
fn macos_executable(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("packaging/macos/Info.plist.in")).ok()?;
    let after = text.split("<key>CFBundleExecutable</key>").nth(1)?;
    let rest = after.split("<string>").nth(1)?;
    Some(rest.split("</string>").next()?.trim().to_owned())
}

/// The executables and icons the scripts name that nothing produces.
pub fn dangling_assets(root: &Path) -> Result<Vec<(PathBuf, usize, String)>, String> {
    let binaries = workspace_binaries()?;
    let bundle_exe = macos_executable(root);
    let mut out = Vec::new();
    for path in files_to_scan(root) {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        for (number, line) in text.lines().enumerate() {
            for asset in assets_in_line(line) {
                let missing = match asset {
                    Asset::Exe(name) | Asset::Bin(name) => !binaries.contains(&name),
                    Asset::Icon(name) => !root.join("assets/app-icon").join(&name).exists(),
                    Asset::BundleExe(name) => bundle_exe.as_deref().is_some_and(|exe| exe != name),
                };
                if missing {
                    out.push((path.clone(), number + 1, line.trim().to_owned()));
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod asset_tests {
    use super::*;

    #[test]
    fn executable_names_are_read_from_paths_and_quotes() {
        assert_eq!(assets_in_line("Copy-Item (Join-Path $Bin 'quill.exe')"), vec![Asset::Exe("quill".to_owned())]);
        assert_eq!(assets_in_line("<File Source=\"$(var.BinDir)\\quill-cli.exe\" />"), vec![Asset::Exe("quill-cli".to_owned())]);
    }

    #[test]
    fn windows_tools_are_not_builds() {
        assert!(assets_in_line("Get-Command signtool.exe").is_empty());
        assert!(assets_in_line("the Windows SDK (rc.exe, signtool.exe)").is_empty());
    }

    #[test]
    fn a_staged_binary_is_not_a_host_tool() {
        assert_eq!(assets_in_line("install -Dm755 \"$BIN/quill\" \"$STAGE/usr/bin/quill\""), vec![Asset::Bin("quill".to_owned())]);
        assert!(
            assets_in_line("sudo ln -sf /usr/bin/maxima-sage /usr/local/bin/maxima").is_empty(),
            "an absolute path names a tool the host already has"
        );
    }

    #[test]
    fn icons_keep_their_extension() {
        assert_eq!(assets_in_line("cp \"$ROOT/assets/app-icon/quill.icns\" \"$APP/Contents\""), vec![Asset::Icon("quill.icns".to_owned())]);
        assert_eq!(assets_in_line("-d \"IconPath=$(Join-Path $Root 'assets\\app-icon\\quill.ico')\""), vec![Asset::Icon("quill.ico".to_owned())]);
    }

    #[test]
    fn the_bundle_executable_is_its_own_name() {
        assert_eq!(
            assets_in_line("cp \"$WORK/bin/quill\" \"$APP/Contents/MacOS/WordCraft\""),
            vec![Asset::BundleExe("WordCraft".to_owned()), Asset::Bin("quill".to_owned())]
        );
    }
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

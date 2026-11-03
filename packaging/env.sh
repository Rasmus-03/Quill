# shellcheck shell=bash
# Shared setup for the packaging scripts. Source it: `. "$(dirname "$0")/../env.sh"`.
#
# Exports:
#   ROOT                    workspace root
#   VERSION                 [workspace.package] version from Cargo.toml (override: WORDCRAFT_VERSION)
#   DIST                    output directory for release artifacts (default: $ROOT/dist/release)
#   WORDCRAFT_BUILD_SHA    git commit baked into the binaries (see crates/engine/src/build_info.rs)
#   WORDCRAFT_BUILD_DATE   UTC build date, YYYY-MM-DD
#   CARGO_TARGET_DIR        cargo's target dir (default: $ROOT/target)
# PATH                    rustup's proxies first (see "toolchain" below)

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export ROOT

# The version lives in exactly one place: `[workspace.package] version` in the root Cargo.toml.
# (`cargo xtask version` prints the same thing; awk avoids compiling xtask here.)
workspace_version() {
  awk '
    /^\[/ { in_pkg = ($0 == "[workspace.package]") ; next }
    in_pkg && $1 == "version" { gsub(/[" ]/, "", $3); print $3; exit }
  ' "$ROOT/Cargo.toml"
}

VERSION="${WORDCRAFT_VERSION:-$(workspace_version)}"
if [ -z "$VERSION" ]; then
  echo "error: could not read [workspace.package] version from $ROOT/Cargo.toml" >&2
  exit 1
fi
export VERSION

DIST="${DIST:-$ROOT/dist/release}"
mkdir -p "$DIST"
export DIST

if [ -z "${WORDCRAFT_BUILD_SHA:-}" ]; then
  WORDCRAFT_BUILD_SHA="$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || true)"
fi
export WORDCRAFT_BUILD_SHA
export WORDCRAFT_BUILD_DATE="${WORDCRAFT_BUILD_DATE:-$(date -u +%Y-%m-%d)}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"

# ---- toolchain ---------------------------------------------------------------------------------
# A rustc that rustup does not manage can sit ahead of rustup's proxies on PATH -- Homebrew's
# `brew install rust` is the usual one on macOS, /usr/bin/rustc on Debian. Such a rustc has no
# per-target std, and `rustup target add` cannot fix it because that writes to rustup's own
# toolchains directory. The symptom is misleading: `cargo check --target wasm32-unknown-unknown`
# reports `error[E0463]: can't find crate for core` for every crate at once, and a universal macOS
# build dies on `x86_64-apple-darwin: may not be installed` even though rustup lists it.
#
# So put rustup's proxies first whenever this host has them. Opt out with QUILL_NO_TOOLCHAIN_FIX=1
# when deliberately building against a pinned or vendored toolchain.
_quill_prefer_rustup() {
  local bin part kept=""
  bin="${CARGO_HOME:-$HOME/.cargo}/bin"
  { [ -x "$bin/rustc" ] && [ -d "${RUSTUP_HOME:-$HOME/.rustup}/toolchains" ]; } || return 0
  local IFS=: parts
  read -r -a parts <<<"$PATH"
  for part in "${parts[@]}"; do
    [ "$part" = "$bin" ] || kept+="${kept:+:}$part"
  done
  PATH="$bin${kept:+:$kept}"
  export PATH
}
if [ -z "${QUILL_NO_TOOLCHAIN_FIX:-}" ]; then _quill_prefer_rustup; fi

# Fail loudly, before a build, when the rustc on PATH has no std for a target we are about to
# cross-compile to. Quiet when the target is available.
need_rust_target() {
  local triple rustc sysroot libdir
  rustc="$(command -v rustc || true)"
  if [ -z "$rustc" ]; then
    echo "error: rustc not found on PATH; install Rust with rustup (https://rustup.rs)" >&2
    exit 1
  fi
  for triple in "$@"; do
    libdir="$(rustc --print target-libdir --target "$triple" 2>/dev/null || true)"
    if [ -z "$libdir" ] || ! compgen -G "$libdir/libcore-*.rlib" >/dev/null; then
      sysroot="$(rustc --print sysroot 2>/dev/null || echo unknown)"
      {
        echo "error: the rustc on PATH has no std for $triple"
        echo "       rustc:   $rustc"
        echo "       sysroot: $sysroot"
        case "$sysroot" in
          "${RUSTUP_HOME:-$HOME/.rustup}/toolchains/"*)
            # Rustup's own rustc: the target is simply not installed.
            echo "       rustup manages this toolchain, so add the target and retry:"
            echo "         rustup target add $triple"
            ;;
          *)
            # Some other rustc; `rustup target add` writes to a tree this rustc never reads.
            echo "       This rustc is not the one rustup installs std into, so \`rustup target add"
            echo "       $triple\` would not help. A rustc shadowing rustup's proxies (Homebrew's"
            echo "       \`brew install rust\` on macOS, /usr/bin/rustc on Debian) is the usual cause:"
            echo "         export PATH=\"${CARGO_HOME:-$HOME/.cargo}/bin:\$PATH\""
            echo "         rustup target add $triple"
            ;;
        esac
      } >&2
      exit 1
    fi
  done
}

# Emit a GitHub Actions warning (plain stderr outside Actions).
warn() {
  if [ -n "${GITHUB_ACTIONS:-}" ]; then echo "::warning::$*"; else echo "warning: $*" >&2; fi
}

# Copy licence and readme files that exist into a package directory.
copy_docs() {
  local dest="$1" f
  for f in README.md LICENSE LICENSE-MIT LICENSE-APACHE COPYRIGHT; do
    if [ -f "$ROOT/$f" ]; then cp "$ROOT/$f" "$dest/"; fi
  done
  copy_font_licences "$dest"
}

# Builds made with craft-fonts (CRAFT_FONTS_DIR, set by the release workflow) embed its fonts, so
# the package carries their licences as OFL-<family-dir>.txt. Nothing without CRAFT_FONTS_DIR.
copy_font_licences() {
  local dest="$1" f
  [ -n "${CRAFT_FONTS_DIR:-}" ] || return 0
  for f in "$CRAFT_FONTS_DIR"/fonts/*/OFL.txt; do
    if [ -f "$f" ]; then cp "$f" "$dest/OFL-$(basename "$(dirname "$f")").txt"; fi
  done
}

# Portable SHA-256 of a file (prints just the hash).
sha256() {
  if command -v sha256sum >/dev/null; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

# Releasing Quill

The canonical recipe is [craftrules `release/playbook.md`](https://github.com/storytold/craftrules/blob/main/release/playbook.md);
this file has the Quill specifics.

1. Bump the version on `main`: `cargo xtask version set 1.2.0` (the one source of truth is
   `[workspace.package] version`; the app shows it in `--version` and About, with the commit from
   `WORDCRAFT_BUILD_SHA`).
2. Merge `main` into `release` and push. `.github/workflows/release.yml` builds:

| Platform | Artifacts | Signing |
|---|---|---|
| macOS | universal `Quill.app` in a `.dmg`, universal CLI zip | Developer ID + notarization (`APPLE_*` secrets) |
| Windows | x64, x86 and arm64 `.msi` + portable `.zip` | Azure Trusted Signing (`AZURE_*` secrets) |
| Linux | x86_64 and aarch64 `.AppImage` (+ `.zsync` for AppImageUpdate), `.flatpak` bundle, `.deb`, `.rpm`, `.tar.gz`; Flathub manifest | checksums |
| Linux riscv64 | `.tar.gz` (cross-compiled on `ubuntu-24.04`, glibc >= 2.39; CLI smoke-tested under QEMU) | checksums |
| FreeBSD | x86_64 `.tar.gz` | checksums |
| Web | `quill-web-<version>.zip` (static site: `index.html`, wasm, glue) | none |

3. The run drafts a GitHub Release `Quill v<version>` with `SHA256SUMS.txt`; review and publish it.

Signing secrets live in the repository's `release` environment (deployment branch: `release` only),
synced from the release manager's machine (`craftrules/release/signing-setup.md`). Missing secrets
produce unsigned builds with a warning, so forks still build.

Local packaging: `packaging/macos/package.sh --arch universal`, `packaging/windows/package.ps1`,
`packaging/linux/package.sh` (then `packaging/linux/flatpak-bundle.sh` for the Flatpak), `packaging/web/package.sh`.

Dry run: `gh workflow run release.yml --ref <branch>` builds the unsigned jobs (version, Linux,
Linux riscv64, Flatpak, FreeBSD, web) on any branch; the signing jobs are refused there by the `release`
environment's branch rule, so no draft release is made.

## Before you publish

- `cargo xtask ci` is green (formatting, clippy, the whole test suite, the asset attribution, the
  crate layering, the package names, the browser build).
- The CI matrix is green on Linux, macOS and Windows (`printing.yml`), and the workflow lint
  (`.github/workflows/workflow-lint.yml`) is green.
- `CHANGELOG.md` has an entry for the version, in the format of the earlier ones.
- `ROADMAP.md`, `docs/gaps.md`, `docs/roadmap.md` and the parity documents are updated, each with
  its status line and revision history bumped.
- Every artifact carries the Quill name: `quill-…` files, `Quill.app`, `quill.exe`. The
  `pkg-names` gate (`cargo xtask pkg-names`) catches a stale package name in a workflow or a
  packaging script; the packaging scripts themselves are the source of the artifact names.

## The macOS DMG window

`packaging/macos/dmg/` holds the Finder window of the disk image: the background
(`background.svg` → `background.tiff`) and the view settings (`dmg-layout.DS_Store`). The
`.DS_Store` carries the volume name and the app icon's name, so it changes whenever the app is
renamed — regenerate it with

```sh
python3 packaging/macos/dmg/generate.py --only-ds-store
```

(that needs `ds_store` and `mac_alias`, but no resvg and no fonts). The background image itself
is only regenerated with `python3 packaging/macos/dmg/generate.py`, which needs resvg.

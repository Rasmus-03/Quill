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

## The changelog

`CHANGELOG.md` is the user's view of a release. It is written **while the work lands**, in the
same commit as the change, and not scraped from the commit log at release time. It follows the
shape of the entries already there:

```markdown
## Unreleased

One paragraph: what this release is about.

- **A bold phrase in the user's words** — what happened before, what happens now, and the issue
  number when there is one.
```

The rules:

- One bullet per user-visible change, in the app's terms — the ribbon label, the dialog, the file,
  the window — never the crate, the command id or the test. Say what the user saw before when it
  was wrong: "a solve that found nothing used to say only `Maxima produced an empty result`".
- A fixed crash or a data-path bug gets a bullet even though it is not a feature: it is the thing
  people are deciding whether to trust.
- Refactors, dependency bumps, CI and test-only work get no bullet, unless they change what the
  user can do or how much they can trust it.
- `## Unreleased` stays at the top. A release turns it into `## <version> — <short title>` (the
  version `cargo xtask version set` wrote) and opens a fresh `## Unreleased` above it. Newest
  first.
- Entries are in English: the changelog is a repository document. Release notes translated for
  users are a separate file.

## Before you publish

- The version is bumped: `cargo xtask version set <version>`, which is also what the artifacts,
  `--version` and About carry.
- `cargo xtask ci` is green (formatting, clippy, the whole test suite, the asset attribution, the
  crate layering, the package names, the browser build).
- The CI matrix is green on Linux, macOS and Windows (`printing.yml`), the workflow lint
  (`.github/workflows/workflow-lint.yml`) is green, and the supply-chain run on the same commit is
  green (`cargo audit`, the MSRV job, coverage).
- `CHANGELOG.md` has an entry for the version in the shape above, with the `Unreleased` heading
  turned into the version.
- `ROADMAP.md`, `docs/gaps.md`, `docs/roadmap.md`, `docs/status.md`, the parity documents and
  `docs/release-plan-1.2.md` are updated, each with its status line and revision history bumped.
- The real-world corpus was fetched and checked on this commit: CI's `corpus` job is green, and
  `cargo xtask corpus --docx && cargo test -p wordcraft-docx --test corpus` passes here too (71
  files, every one round-tripping, formatting and page set-up included). Run it locally as well:
  the test passes when the corpus is absent, so a green main job says nothing about it.
- The release plan's definition of done (`docs/release-plan-1.2.md` §6) is walked item by item,
  and the ones that cannot be proven on this machine say so.
- Every artifact carries the Quill name: `quill-…` files, `Quill.app`, `quill.exe`. The
  `pkg-names` gate (`cargo xtask pkg-names`) catches a stale package name in a workflow or a
  packaging script; the packaging scripts themselves are the source of the artifact names.
- The packaged build was opened once on each platform that can be checked here: it starts, opens
  a document, saves it and shows the version in About.
- The dry run (`gh workflow run release.yml --ref <branch>`) built the unsigned jobs, so a broken
  job is found before the `release` branch carries it.

## Who publishes

The release manager — today the repository owner, `@Rasmus-03` — runs the checklist above,
reviews the drafted GitHub Release (`Quill v<version>`, with `SHA256SUMS.txt`) and presses
**Publish**. Nothing else publishes: a green CI run is not a release, and an agent stops at the
draft. Publishing is an owner decision in the same sense as the signing secrets and the next
cloud provider: it needs the owner's account, and it cannot be undone politely.

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

# Fork notes (Quill vs upstream WordCraft)

Quill is a snapshot fork of storytold/wordcraft (MIT/Apache-2.0; see
`LICENSE-MIT`, `LICENSE-APACHE`, `ATTRIBUTION.md`). Upstream history is
intentionally not tracked here, so upstream merges arrive as manual ports.
This file lists every upstream file Quill touches, so a port can be reviewed
file by file.

## What is Quill's own code

These areas were written here, not upstream. A port never replaces them, and a change to them
is reviewed as Quill code:

| Area | Files | What it is |
|---|---|---|
| The WordMat tab | `crates/engine/src/cmd/mathcraft.rs`, `mathgraf.rs`, `crates/doc/src/math_edit.rs`, `docs/wordmat-fane.md` | Mathematics inside the document: calculate, solve, isolate, units, matrices, statistics, plots — Maxima behind the commands, answers written into the equation in green |
| Nextcloud | `crates/cloud/` (WebDAV, OCS, the worker, the app-password file), `crates/engine/src/cloud.rs`, `tests_cloud.rs`, `docs/cloud.md` | Browse, open, save, share, search and read versions on a server; a file that changed there is never overwritten by accident |
| Obsidian vaults | `crates/engine/src/vault.rs`, `crates/engine/src/io/vault_fs.rs`, `docs/obsidian.md` | Open a vault folder, search it, see backlinks and tags, write a note back byte for byte |
| The Danish release work | `crates/ui-egui/src/i18n/`, `crates/proof/src/` | The Danish interface catalog and Danish proofing (1.2, workstream C) — new files beside the other languages, never a translation of a shared file |
| The real .docx corpus | `crates/docx/tests/corpus.rs` with its pinned manifest, `xtask/src/corpus.rs`, `docs/docx-corpus.md` | Fetch a licence-clean corpus by hash and check that real documents survive open → save → reopen |
| Quill's branding and packaging | `assets/app-icon/quill.*`, `packaging/*/quill.*`, `packaging/linux/io.github.rasmus03.quill.*`, `packaging/windows/quill.wxs` | The name, the icons we drew, the MSI, the bundle, the artifact names |
| Quill's docs | `README.md`, `FORK.md`, `CHANGELOG.md`, `docs/status.md`, `docs/release-plan-1.2.md` | The front page, this ledger, the changelog, the measured maturity page and the shared release plan |

## New files (no upstream conflict)

- `crates/engine/src/cmd/mathcraft.rs` — WordMat engine (Maxima backend)
- `crates/engine/src/cmd/mathgraf.rs` — function plots (Maxima sampling, own PNG renderer)
- `docs/wordmat-fane.md`, `WORDMAT-FANE.md` (mathcraft repo), `CHANGELOG.md`
- `docs/release-plan-1.2.md` — Quill's own release plan (shared with agents)
- `docs/images/quill-*.png`, `packaging/linux/io.github.rasmus03.quill.*`
- `xtask/src/pkg_names.rs` — `cargo xtask pkg-names`: every `cargo -p <package>` in a workflow or
  a packaging script must name a package that exists
- `packaging/windows/quill.wxs` — the MSI (renamed from `wordcraft.wxs`)

## Modified upstream files (port with care)

| File | What Quill changed |
|---|---|
| `crates/engine/src/cmd/mod.rs` | register `mathcraft`, `mathgraf` specs |
| `crates/engine/src/cmd/equation.rs` | selection-span delete/copy, equation-object fragments |
| `crates/engine/src/cmd/caret.rs` | select-all inside equations |
| `crates/engine/src/cmd/edit.rs` | equation clipboard/cut/paste paths |
| `crates/engine/src/session.rs` | `math_defs`, `math_decimal`, `MathEdit.anchor`, prefs persistence |
| `crates/engine/src/ribbon.rs` | test-only borrow fix |
| `crates/doc/src/edit.rs` | clear stale equation OMML on copy |
| `crates/doc/src/math_edit.rs` | `SelSpan`/`span_nodes` (cross-structure selection) |
| `crates/formats/src/markdown.rs` | escape a paragraph that would read back as a heading (`#`, `##`, `## ##`) |
| `crates/ui-egui/src/lib.rs` | WordMat tab auto-setup, Maxima hint, dialogs, no-Discord |
| `crates/ui-egui/src/canvas.rs` | cross-structure highlight, Danish equation menu |
| `crates/ui-egui/src/ribbon.rs` | Commands folded into Help (Agents group removed) |
| `crates/ui-egui/src/backstage.rs`, `keytips.rs`, `options.rs`, `dialogs.rs`, `i18n/*` | Discord removal, Quill strings, WordMat carve-out |
| `crates/ui-egui/src/dialogs_ribbon.rs` | damaged-prefs test expectation (auto-built tab) |
| `crates/control-key/src/lib.rs` | settings dir `quill`, app folder test |
| `crates/engine/src/session.rs`, `tests.rs` | author default `Quill User` |
| `crates/mcp/src/lib.rs` | CLI help strings (`quill-cli`) |
| `apps/wordcraft/*` | package/binary rename, Quill strings, desktop ID, tests |
| `apps/wordcraft-cli/*` | package/binary rename, test env var |
| `apps/wordcraft-web/*` | package/binary rename to `quill-web`, Quill page title and favicon |
| `packaging/*`, `assets/app-icon/*` | Quill IDs, icon filenames, `Quill.app`, `quill-…` artifacts, `quill.wxs` |
| `xtask/src/main.rs`, `xtask/src/layers.rs` | `pkg-names` gate, the `quill-web` app in the layer table |
| `.github/workflows/ci.yml`, `printing.yml`, `release.yml` | the packages that exist, the whole test suite, the repo's own gates, the Quill names |
| `ATTRIBUTION.md`, `README.md` | Quill rows, own README |
| `docs/releasing.md`, `docs/architecture.md` | Quill's release, Quill's names |

Untouched: everything else merges cleanly.

## The port policy

Upstream keeps shipping, so a port is normal work — and the only thing that makes it cheap is
that this file stays true.

- **Quill's own files are off-limits to a port.** Nothing in "What is Quill's own code" is
  replaced by an upstream file of the same name. When upstream grows a feature that overlaps one
  (a cloud client, another maths engine), the two are compared in a pull request; the upstream
  code is read, never merged over Quill's.
- **A file in the modified table may be overwritten only together with its row.** Re-apply the
  change the row describes in the same commit, and say in the commit message which row it was.
  A row that no longer applies is deleted in that commit, not left to rot.
- **Everything else is upstream's.** Merge it as it comes; do not restructure upstream files for
  taste, rename their symbols or reformat them, because every such change becomes a conflict in
  the next port and a line in this table.
- **New code goes in new files where it can.** A feature that can live beside an upstream file
  rather than inside it keeps the table short — that is why the cloud client is a crate of its
  own.
- **Credit stays.** `ATTRIBUTION.md`, `NOTICE`, `LICENSE-*` and About keep naming WordCraft and
  its contributors; a port does not remove them, and a port that carries their code says so in
  the commit message.
- **One port, one pull request**, with the rows it re-applied and the commands that were run
  (`cargo xtask ci`, and the corpus when the port touches DOCX) in the description.

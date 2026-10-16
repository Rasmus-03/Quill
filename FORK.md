# Fork notes (Quill vs upstream WordCraft)

Quill is a snapshot fork of storytold/wordcraft (MIT/Apache-2.0; see
`LICENSE-MIT`, `LICENSE-APACHE`, `ATTRIBUTION.md`). Upstream history is
intentionally not tracked here, so upstream merges arrive as manual ports.
This file lists every upstream file Quill touches, so a port can be reviewed
file by file.

## New files (no upstream conflict)

- `crates/engine/src/cmd/mathcraft.rs` — WordMat engine (Maxima backend)
- `crates/engine/src/cmd/mathgraf.rs` — function plots (Maxima sampling, own PNG renderer)
- `docs/wordmat-fane.md`, `WORDMAT-FANE.md` (mathcraft repo), `CHANGELOG.md`
- `docs/images/quill-*.png`, `packaging/linux/io.github.rasmus03.quill.*`

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
| `packaging/*`, `assets/app-icon/*` | Quill IDs, icon filenames |
| `ATTRIBUTION.md`, `README.md` | Quill rows, own README |

Untouched: everything else merges cleanly.

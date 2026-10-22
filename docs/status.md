# Quill's status

What Quill does today, measured rather than estimated. Every number here comes from a command
you can run, or from a file in this repository — not from an impression.

| | |
|---|---|
| **Measured** | 2026-10-22, the 1.2 line (Quill 1.1.0 plus the 1.2 work) |
| **How to re-measure** | `cargo xtask stats`, `cargo xtask parity`, `cargo test --workspace` |
| **The long list** | [`docs/gaps.md`](gaps.md) (what is missing), [`docs/roadmap.md`](roadmap.md) (the milestones) |

## By area

| Area | State | Measured by |
|---|---|---|
| **Writing and formatting** | **Ready for daily use.** Text, styles, lists, tables, headers and footers, footnotes and endnotes, comments, tracked changes, content controls, fields, equations, pictures, text boxes, page setup, a table of contents, cross-references, citations (Zotero), printing and PDF export. | The Word feature catalog is at **497 of 504** (98.6 %): [`docs/parity-checklist.md`](parity-checklist.md). Every ribbon tab is at 100 % except Insert (48/53), Review (30/31) and Home (102/103). |
| **.docx compatibility** | **Ready for daily use, checked against real files.** 71 documents that Word, Google Docs, LibreOffice and Pages wrote are read → written → read on every build: the text, the tables, the headers and footers, the parts and the pictures all survive. | [`crates/docx/tests/corpus.rs`](../crates/docx/tests/corpus.rs) with the pinned manifest; what is lost is listed in [`docs/file-format-parity.md`](file-format-parity.md). |
| **The maths tab** | **Ready for school use.** Calculate, solve (exactly, numerically, by isolating a variable, systems, ODEs), units, definitions, matrices, statistics, calculus, formulas, graphs. Answers appear in green in the equation. | 70+ tests in `crates/engine/src/cmd/mathcraft.rs` and `mathgraf.rs`; the engine is Maxima, which must be installed. |
| **Cloud and notes** | **Ready for use.** Nextcloud (browse, open, save, share, search, versions) and Obsidian vaults (search, backlinks, tags, links, byte-for-byte unchanged notes). | 31 tests in `crates/cloud`, the vault tests in `crates/engine/src/vault.rs`. |
| **Other formats** | **Partly.** `.doc` reads; `.rtf`, `.odt`, `.md`, `.html`, `.txt` and `.tex` read and write; PDF exports; PDFs open as editable documents. | [`docs/file-format-parity.md`](file-format-parity.md) — every format, with its fidelity. |
| **Stability** | **Good, with a small tail.** No panic path is reachable from a file or a command; the reader and the writer cap every allocation. The open field reports (a startup crash on Intel UHD, some freezes, a Windows write problem) are fixed one by one. | `hostile_params_never_panic` runs every command with junk parameters; the DOCX reader has a hostile-input suite. |
| **Interface languages** | **20 languages, Danish included.** English is the source; the other 19 are catalogs. | `crates/ui-egui/src/i18n/` — one `.tsv` per language; `every_tab_and_command_is_translated` fails if a new string is missing from any of them. |
| **Proofing** | **Four languages.** English, Polish, Portuguese and Danish: spelling, conservative grammar rules and hyphenation, chosen per document. | `crates/proof` (`ProofLang::PROOFED`); the dictionaries are the ones installed on the machine, none bundled. |
| **Platforms** | **macOS, Windows, Linux, the BSDs and the browser.** Linux goes beyond Word (a Flatpak, a .deb, an .rpm, an AppImage); the web build is a static site. | `packaging/` holds a script per platform; the browser build is checked in CI. |
| **Distribution** | **Signed builds** for macOS (Developer ID + notarisation) and Windows (Azure Trusted Signing); checksums for the rest. | [`docs/releasing.md`](releasing.md). |

## The numbers

| | |
|---|---|
| Word feature catalog covered | **497 / 504** (98.6 %) |
| Commands | **670** |
| Tests | **2,147** (plus 4 property-test blocks) |
| Code | **~206,000 lines** of Rust in 21 crates |
| Crates | 17 library crates + 3 apps + the build tool (`xtask`) |
| Corpus files checked on every build | 71 |

## What is not ready

The honest list, with what it costs:

| | |
|---|---|
| Charts and SmartArt from Word are shown but not editable | 35–55 h (gap #13) |
| 3-D models, legacy form fields, East Asian text layout are dropped on open | 11–19 h (gaps #17, #10) |
| `.doc` cannot be written | 25–40 h (gap #14) |
| Macros are kept but never run; no add-ins | 35–75 h (gap #22) |
| The pagination is not Word's, page for page | 30–50 h (gap #4) |

## What the release before this one changed

The full list is in [`CHANGELOG.md`](../CHANGELOG.md); the plan for the next one is in
[`docs/release-plan-1.2.md`](release-plan-1.2.md).

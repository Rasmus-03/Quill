# Quill 1.2 — release plan

> **Last reviewed:** 2026-11-03 · **Last updated:** 2026-11-03 · **Change:** minor (the macOS walk is
> complete on `Rasmus-03/mac-release`: `cargo xtask ci` is green (all 8 steps); `package.sh --arch
> aarch64` and `--arch universal` both exit 0, the latter writing `quill-1.1.0-macos-universal.dmg`
> (51,924,638 B) whose fat `Quill` reports `x86_64 arm64` and whose mounted app now passes
> `codesign --verify --strict` (the HFS writer's `com.apple.FinderInfo` on the icon is cleared on the
> raw image), and `quill-cli --version` prints `quill-cli 1.1.0`; the DMG volume name and artwork and
> the `release.yml` dry run, Windows, Linux and notarization remain; previously minor: the macOS walk
> finishes on `Rasmus-03/mac-release`: `cargo xtask ci` is green — all 8 steps, exit 0 — and, once
> `package.sh` and `Info.plist.in` name the `quill` binaries and `Quill.app` the build emits,
> `package.sh --arch aarch64` exits 0 and writes `quill-1.1.0-macos-aarch64.dmg` (24,855,168 B,
> mounted once: the volume holds `Quill.app`) and `quill-cli-1.1.0-macos-aarch64.zip` (15,652,300 B);
> the volume name, the DMG artwork and the CLI's `--version` string are the residue left, and the
> universal DMG, the `release.yml` dry run and the Windows and Linux packages stay unwalked;
> previously minor: the
> same checklist is walked on that branch before the repair and stops at the first command —
> `xtask` does not compile, the package script asks for package ids that no longer exist, and the
> dry run is refused, so no artifact was produced; previously
> minor: A5 closes the
> supply chain — the two `taiki-e/install-action` tool tags are pinned, so every `uses:` in the
> workflows is a commit — and the gate is re-run on this line: all 8 steps pass, wasm included,
> which retires the note that this machine cannot run it; previously minor: the progress
> docs tell the 1.2 truth: the measured numbers are re-run on this line in `ROADMAP.md`,
> `docs/roadmap.md`, `docs/target-app-parity.md` and `docs/status.md`, the claims the corpus made
> false are rewritten, and the assessment's duplicated rows are merged — §6's progress-docs item
> is closed; previously minor: three of the critic's eight findings are now closed in code as
> well: the corpus has its own CI job, the
> round-trip harness compares the whole document instead of a projection of it, and Home carries
> Table, Pictures and Word Count; previously minor: an outside critic read 1.2 and eight findings
> are answered in §10 — the maths tab is Danish at its failure points, the README and status page
> no longer contradict the release, and the corpus page count is measured against Word (62 of 65
> exact); previously minor: the 1.2 work branches
> are integrated: Danish proofing, the Danish catalog, fuzzing and the parity wins; D4 and D5
> landed the fork boundary and the release checklist) ·
> **Status:** draft — the owner decisions in §8 are open · **Baseline:** 1.1.0 (Nextcloud, deeper
> Obsidian vaults, Focus mode), version `1.1.0` in `Cargo.toml:7`

The working plan for the next Quill release. It is written for humans **and** agents: every item
says what it is, where it lives, how we know it is done and what it costs. Read it with
[`docs/gaps.md`](gaps.md) (the ranked Word-parity work list, inherited from WordCraft),
[`FORK.md`](../FORK.md) (what is Quill's own code) and [`docs/roadmap.md`](roadmap.md) (the
beta list). The local `plan/` folder (gitignored) may hold a session copy; **this file is the
shared one** — agents on other machines read it from the repository.

## Danresume

**1.2 er en tillidsudgivelse.** Tre ben: (1) *Tillid* — bygget skal være grønt på alle
platforme, og .docx skal være testet mod rigtige filer, ikke kun syntetiske. (2) *Dansk* —
brugerfladen og stavekontrollen skal kunne dansk, og matematikfanen skal sige tydeligt hvad der
skete (ingen løsning, uendeligt mange, kun komplekse, kan ikke løses symbolsk). (3) *Brug* —
det skal fungere i en klasse: åbn filen læreren sendte, skriv opgaven, løs matematikken, aflever.
Vi bygger ikke nye feature-områder i denne udgivelse; vi gør de eksisterende troværdige.

---

## 1. The release in one line

**"Danish school writing that survives a real .docx."**

Three legs, in priority order:

| Leg | What it buys | Workstreams |
|---|---|---|
| **Trust** | The build is verified on every platform; the document format is tested against real files, not only synthetic fixtures | A, B |
| **Danish** | The interface, the proofing and the maths tab speak Danish; the maths tab says clearly what happened | C |
| **Use** | The classroom workflows hold up end to end | B, C, D |

**Not in 1.2** (deliberate, so the release can ship): chart and SmartArt *editing* (gap #13),
`.doc` writing (gap #14), VBA and add-ins (gap #22), the in-app assistant (gap #25), East Asian
typography (gap #17), password-protected files (gap #9), one more cloud provider (gap #22).
They stay in [`docs/gaps.md`](gaps.md) for a later release.

## 2. Why these items — the evidence

Everything below was measured in this tree on 2026-10-21. Numbers are agent wall-clock hours,
calibrated like the rest of the repo.

| # | Finding | Evidence |
|---|---|---|
| 1 | ~~**macOS and Windows packages cannot be built since the rename.**~~ **Fixed in arc 0.** The scripts built `-p wordcraft -p wordcraft-cli` (the packages are `quill` and `quill-cli`) and copied `assets/app-icon/wordcraft.icns`, which is now `quill.icns` | `packaging/macos/package.sh:61,105`, `packaging/windows/package.ps1:68,131` |
| 2 | ~~**The only cross-platform CI matrix has never run green.**~~ **Fixed in arc 0.** `printing.yml` ran `cargo clippy -p wordcraft` / `cargo test -p wordcraft`; that package does not exist, so both steps failed before any test ran | `.github/workflows/printing.yml:66,69` |
| 3 | ~~**The format-critical test suite never runs in CI.**~~ **Fixed in arc 0.** `ci.yml` ran `--workspace --lib` plus the *engine's* integration tests; the 173 tests in `crates/docx/tests/` belong to another package and never ran remotely | `.github/workflows/ci.yml:34,38`; `crates/docx/tests/*` |
| 4 | ~~**The repo's own quality gates are local-only.**~~ **Fixed in arc 0.** No workflow called `cargo xtask`, so the ATTRIBUTION/assets gate, the layering gate and the wasm gate never ran remotely | `.github/workflows/` vs. `xtask/src/main.rs:215-217` |
| 5 | **The maths tab collapses four different outcomes into one error.** "No solution", "infinitely many", "complex only" and "cannot be solved symbolically" all end as `Maxima produced an empty result` / `Could not solve: …` | `crates/engine/src/cmd/mathcraft.rs:857,2574` |
| 6 | **There is no Danish anything.** The interface has 19 languages and Danish is not one of them (~2,100 strings each); proofing covers English, Polish, Portuguese and Bulgarian only — Danish text is never spell-checked. Even the maths tab, which the 1.0.0 changelog calls Danish, ships English ribbon labels ("Calculate", "Solve", "Units") with two Danish error strings ("Maxima fejlede") | `crates/ui-egui/src/i18n/` (no `da.tsv`); `crates/proof/src/lang.rs:7-33`; `crates/engine/src/cmd/mathcraft.rs:2838-2843,2915-2923` |
| 7 | **No real-world .docx has ever been opened by Quill.** The corpus gap is rank #1 upstream and has been open since 2026-10-06; every fixture is a hand-written XML string | `docs/gaps.md:18`; `crates/docx/tests/fixtures.rs:1` |
| 8 | **The README only documents Linux**, although macOS and Windows packaging exists (and was broken, finding 1); the fork's own identity story lives in `FORK.md`, which no first-time reader sees | `README.md:14-25` |
| 9 | ~~**WordCraft strings survive in user-visible places.**~~ **Fixed in arc 0** (the release workflow, the web page, the packaging output and `docs/releasing.md` now say Quill; the internal crate names and the `WORDCRAFT_*` build variables stay, and `docs/releasing.md` says why) | `.github/workflows/release.yml:25-26`, `apps/wordcraft-web/index.html:6-7`, `packaging/linux/package.sh:43,102`, `docs/releasing.md:1` |
| 10 | **There is no CI gate on supply chain or toolchain drift**: actions are floating (`@v4`, `@v2`, `@stable`), four workflows call a reusable workflow at mutable `@main` — including the one holding `contents: write` and all signing secrets — and `rust-version = "1.95"` is never checked | `.github/workflows/ci.yml:19-25`, `release.yml:23,29-43`, `Cargo.toml:10` |

## 3. Workstreams

Every item: **what** (concrete), **where** (files), **done when** (acceptance), **cost**.

### A. Trustworthy build — P0

**A1. Make macOS and Windows packaging work again** — 2–4 h — **done in arc 0**
*What:* rename the packages, binaries and artifacts in `packaging/macos/package.sh` and
`packaging/windows/package.ps1` to `quill`/`quill-cli`/`quill-<version>-…`; point the icon at
`assets/app-icon/quill.icns` and `assets/app-icon/quill.ico`; rename `packaging/windows/wordcraft.wxs`
→ `quill.wxs`; rename the bundle to `Quill.app`; update `packaging/macos/Info.plist.in` and the
DMG background script names.
*Where:* `packaging/macos/*`, `packaging/windows/*`, `packaging/env.sh`.
*Done when:* `packaging/macos/package.sh --arch universal` and `packaging/windows/package.ps1`
run to completion on a dev machine and produce artifacts named `quill-*`; `cargo xtask assets`
stays green (every touched asset has an ATTRIBUTION row).
*Note:* the Windows MSI is signed with **upstream's** Azure credentials — the owner decides §8.6.
*What landed:* the scripts build `quill`/`quill-cli` and name every artifact `quill-*`; the MSI
is `quill.wxs` with the product name, the Start-menu entry, the file associations and the
registry keys under `Quill`; the macOS bundle is `Quill.app` with `Quill.icns`; the DMG volume is
`Quill` (its `.DS_Store` regenerated, byte-for-byte reproducible — see `docs/releasing.md`); the
Linux AppImage directory is `Quill.AppDir`; the web build is `quill-web`. The MSI's manufacturer
and its URLs now name Quill and this repository. The *crate* names stay `wordcraft-*` (§8.3).
*Proved:* on this machine the macOS package now builds end to end — `Quill.app` (signed, valid),
`quill-1.1.0-macos-aarch64.dmg` (the volume mounts as `Quill`, with `Quill.app` and the Finder
window inside) and `quill-cli-1.1.0-macos-aarch64.zip`. The Windows MSI needs Windows.

**A2. Fix the cross-platform CI matrix** — 1–2 h — **done in arc 0**
*What:* `printing.yml` must use `-p quill -p wordcraft-ui-egui`; add a guard job that fails when
a `-p` name in any workflow is not a real package (`cargo metadata --no-deps` + jq), so the
next rename cannot silently break the matrix again.
*Where:* `.github/workflows/printing.yml`, new `.github/workflows/workflow-lint.yml`.
*Done when:* the Ubuntu/Windows/macOS matrix is green and the guard job proves it.
*What landed:* the matrix asks for the packages that exist; the guard is `cargo xtask pkg-names`
(new, with tests), it reads the workflows **and** the packaging scripts, and it runs in
`.github/workflows/workflow-lint.yml`. It catches exactly the bug that was there: pointing it at
the old `-p wordcraft` fails with the file and the line.

**A3. Run the whole test suite in CI** — 1 h — **done in arc 0**
*What:* replace `cargo test --workspace --lib` + the engine-only integration job with
`cargo test --workspace` (or `cargo xtask test`), so the 173 DOCX tests, the bin-harness tests
(`quill` 47, `quill-cli` 9) and the MCP/fonts/ui-egui/CLI integration tests run on every push.
*Where:* `.github/workflows/ci.yml`.
*Done when:* the CI log lists `crates/docx/tests/roundtrip.rs` and friends; a deliberately
broken DOCX fixture fails the build.
*Note:* split into a fast `--lib` job and an `integration` job if the wall-clock hurts.
*What landed:* `ci.yml` runs `cargo test --workspace` (59 test binaries, ~2,200 tests) plus the
Maxima-dependent engine tests. Running it locally found a real bug the old `--lib` job hid: a
paragraph whose text is only hashes (`##`) was read back as an empty heading and lost its text
— fixed in `crates/formats/src/markdown.rs`, with a regression test and the proptest seed.

**A4. Run the repo's own gates in CI** — 1 h — **done in arc 0**
*What:* add `cargo xtask assets`, `cargo xtask layers` and `cargo xtask wasm` as CI steps (or one
`cargo xtask ci` step), and add the `wasm32-unknown-unknown` target to the CI toolchain.
*Where:* `.github/workflows/ci.yml`, `xtask/src/main.rs`.
*Done when:* a missing ATTRIBUTION row or a layering violation fails the build.
*What landed:* `ci.yml` runs the three gates as separate steps (with the wasm target in the
toolchain), so a missing attribution, a layering violation or a broken browser build fails the
push. `cargo xtask ci` gained the `pkg-names` step too.

**A5. Pin the supply chain** — 1–2 h — **done 2026-10-30**
*What:* SHA-pin every action and reusable workflow reference, including the four `@main`
reusable workflows (`release.yml` holds `contents: write` and the Apple/Azure signing secrets).
*Where:* `.github/workflows/*.yml`.
*Done when:* no `@v4`/`@v2`/`@main` reference remains; `zizmor`/`actionlint` passes.
*What landed:* the four reusable workflows were already pinned to one commit
(`39962f26…`, with the `# main` they were pinned from kept as the comment), so the last
mutable references were the two `taiki-e/install-action` tool tags in `supply-chain.yml`:
`@cargo-audit` and `@cargo-llvm-cov` are now pinned to the commits those tags pointed at on
2026-10-30. Every `uses:` in `.github/workflows/` is a 40-hex commit, which is what
`rg -P 'uses:.*@(?![0-9a-f]{40}\b)' .github/workflows/` now reports: nothing.
*Not proved:* `zizmor` and `actionlint` are not installed on this machine, so the linter half of
the done-when is unrun rather than passed. The pin itself needs no tool to check.

**A6. Guard against dependency and toolchain drift** — 2–4 h — **done 2026-10-30 (the gate works, and it is now green)**
*What:* `cargo audit` (or `cargo deny` with a `deny.toml`) on a schedule and on lockfile changes;
an MSRV job on Rust 1.95 (`rust-version` is declared but never enforced); a weekly full-workspace
job; a coverage job (`cargo llvm-cov`) uploaded as an artifact.
*Where:* `.github/workflows/ci.yml` (+ new `audit.yml`).
*Done when:* an advisory in `zip`/`quick-xml`/`krilla`/`image` fails the build.
*What landed:* the four jobs are `supply-chain.yml` (audit, MSRV 1.95, weekly `cargo xtask ci`,
coverage) — that part was already in the tree. What was missing was the proof, and running
`cargo-audit 0.22.2` on this line supplies it: **the gate exits 1 on four real advisories**,
so "an advisory in `zip` fails the build" is now demonstrated rather than assumed.
*The four, and what they mean for Quill:*

| Advisory | Crate | Reachable? |
|---|---|---|
| RUSTSEC-2026-0369 (9.1 critical) | `zip 3.0.0` | **No** — it is in `ZipArchive::extract`, and Quill never extracts: `crates/docx/src/package.rs` reads entries and validates names itself (zip-slip checks at `package.rs:20-24,196,373`) |
| RUSTSEC-2026-0368 | `zip 3.0.0` | **No** — same `extract` API |
| RUSTSEC-2026-0367 (7.3 high) | `zip 3.0.0` | **No** — same `extract` API |
| RUSTSEC-2026-0385 | `ctutils 0.4.3` | **Unknown** — it reaches the tree through `digest 0.11.3 → hmac 0.13.0` in `crates/docx`, so it is on the package-integrity path; the fix is a patch release (`>=0.4.4`) |

Seven further advisories are "unmaintained" warnings (`rustybuzz`, `ttf-parser`, `zbus_names`,
`clipboard-win`), which the audit reports without failing.
*Resolved the same day, by upgrading rather than allow-listing:*

- **`zip` 3.0.0 → 9.1.3.** The major bump cost one real API change: `ZipFile::name()` now returns
  `Result<Cow<str>, ZipError>` because an entry's name may not be UTF-8. In `crates/docx/src/package.rs`
  an unreadable name is now skipped like an unreadable entry, with a warning, instead of aborting
  the whole document; the tests that read names take them with `expect("zip entry name")`.
- **`ctutils` 0.4.3 → 0.4.7**, the patch the advisory asked for.

`cargo audit` now exits 0. The seven unmaintained-crate warnings remain and are reported, not
hidden. Nothing was suppressed: no advisory was allow-listed, and the fix is the upgrade.

The two dependencies `zip` 9 adds are not a packaging risk: `nix` is gated to
`cfg(any(target_os = "macos", "freebsd", "openbsd", "netbsd", "dragonfly"))` in its manifest, so
the Linux and Windows builds never compile it, and `typed-path` is pure Rust.
*Still owed:* the skip arm for a name `zip` rejects has no reachable test — 9.1.3 decodes a
non-UTF-8 name lossily (CP437, per the spec) instead of failing, so the arm is defensive. The
test added with this item pins what must hold either way: one odd entry name does not stop the
document opening.

**A7. Nightly fuzzing of the hostile boundaries** — 4–8 h
*What:* `cargo-fuzz` targets for the DOCX ZIP/XML reader, the `.doc` piece-table reader, the
RTF/ODT readers and the font/TTC parser; promote the existing proptest cases in
`crates/docx/tests/malformed.rs` and `crates/docbin/src/tests.rs` to fuzz targets; run nightly.
*Where:* `crates/docx/fuzz/`, `crates/docbin/fuzz/`, `crates/formats/fuzz/`, `crates/fonts/fuzz/`.
*Done when:* a nightly job runs 60 s per target and reports crashes as artifacts.
*Rule:* a fuzz finding is a crash fix with a regression test (see AGENTS.md, "Never crash").
*What landed:* four detached `cargo-fuzz` workspaces, one reader each: `read` (docx), `read`
(docbin), `rtf` and `odt` (formats), and `font` (fonts). Every target feeds bytes straight into
the crate's public entry point; the seeds are two synthetic DOCX packages, two synthetic `.doc`
compound files, two RTF files, two ODT packages and a hand-built TrueType font plus a collection,
all written for this repo. `fuzz.yml` runs nightly at 03:43 UTC and on manual dispatch with a
nightly toolchain and `cargo-fuzz`, 60 seconds per target, the corpus cached per crate and the
crash artifacts uploaded when a target fails. The YAML parses, the seeds open through the real
readers, and all five targets type-check; **running them needs rustup, a nightly toolchain and
`cargo-fuzz`, which this machine does not have** — the first real fuzz run is still owed on a
machine that does.

### B. Real documents — P0/P1

**B1. A real-world .docx corpus harness** — 12–20 h
*What:* a licence-clean corpus outside git (`corpus/docx/`, already gitignored) plus a test that
runs only when the corpus is present (env var, mirroring `WORDCRAFT_DOCBIN_FIXTURES` at
`crates/docbin/src/tests.rs:706-728`). For each file: open → render → save → reopen → assert the
text, the paragraph/table structure and the page count survive; report differences instead of
failing silently. Sources with clear licences (Apache POI, docx4j, Open XML SDK test corpora).
*Where:* new `crates/docx/tests/corpus.rs`, `corpus/docx/README.md`, `xtask/src/corpus.rs`
(a `--docx` fetcher next to the existing PngSuite fetcher).
*Done when:* the harness runs over the corpus in CI (self-hosted job or a scheduled job with the
corpus cached) and every failure is either fixed or listed in the compatibility table (B3).
*Rule:* the clean-room rule stays — no file produced by Word is ever committed.

**B2. Corpus provenance** — 1–2 h
*What:* every corpus file records source, licence and date in `corpus/docx/PROVENANCE.md`, and
every file that lands in the repo (rather than the corpus dir) gets an `ATTRIBUTION.md` row.
*Done when:* `cargo xtask assets` is green and `PROVENANCE.md` covers every file.

**B3. A user-facing compatibility matrix** — 2–4 h
*What:* "what survives a round trip" per document feature — equations, tables, pictures, headers,
footers, page numbers, sections, tracked changes, comments, fields, content controls, legacy form
fields, VML, charts/SmartArt/OLE, East Asian layout. The technical source of truth already exists
in [`docs/file-format-parity.md`](file-format-parity.md); this is the user-facing cut of it.
*Where:* `docs/file-format-parity.md` (add a "for users" table) + a summary in the README (D1).
*Done when:* every row has an honest status and a one-line "what you will see".

**B4. Stability sweep of the open crash and hang issues** — 8–12 h
*What:* the open field failures from `docs/gaps.md:20` (Intel UHD startup crash, the Thai-font
and font-selector freezes, no writes without admin on Windows 11, the broken shape tool, images
that cannot be moved), each with a regression test.
*Done when:* each fixed issue closes with a test and the gap list is updated.

### C. Danish first — the difference between Quill and WordCraft — P0/P1

**C1. Danish interface** — 6–10 h
*What:* `crates/ui-egui/src/i18n/da.tsv` with all ~2,100 strings, human-written or human-checked
(machine translation is not acceptable for a school product). Start with the maths tab, the
dialogs and the ribbon; then the rest.
*Where:* `crates/ui-egui/src/i18n/da.tsv`, `crates/ui-egui/src/i18n/catalog.rs`, the i18n tests.
*Done when:* `cargo xtask ci` is green with `da` registered, the app starts in Danish, and the
untranslated-string audit (`crates/ui-egui/src/i18n/audit.rs`) reports no gaps for `da`.
*Note:* the maths tab is the one place where users know the Danish names (WordMat is a Danish
add-in), so its labels come first — see C4.

**C2. Danish proofing** — 6–10 h
*What:* `ProofLang::Da` — the installed Hunspell Danish dictionary (the same mechanism as Polish
and Portuguese: `crates/proof/src/hunspell.rs`), Danish hyphenation patterns (TeX `hyph-da`,
LaTeX Project Public License) and a small Danish grammar rule set; register `da` in
`ProofLang::PROOFED` and in the Review ▸ Language list.
*Where:* `crates/proof/src/lang.rs`, `crates/proof/src/patterns.rs`, `crates/proof/src/hyphen.rs`,
new `crates/proof/src/da/`.
*Done when:* Danish text is spell-checked, hyphenated and flagged for obvious errors; the
dictionary is never bundled — it is read from the system, like `pl` and `pt`.
*Licence:* any pattern file copied into the repo needs an `ATTRIBUTION.md` row (§8.2).

**C3. The maths tab must say what happened** — 3–5 h
*What:* four outcomes must be distinguishable, in Danish, with the next step:
*no solution* ("Ligningen har ingen løsning"), *infinitely many* ("Løsningen er alle tal —
skriv et uligheds- eller intervalkrav"), *complex only* ("Kun komplekse løsninger — søg i C"),
*not solvable symbolically* ("Kan ikke løses symbolsk — prøv numerisk løsning"). Same for
divisions by zero, poles in the solution, and a failing Maxima process.
*Where:* `crates/engine/src/cmd/mathcraft.rs:857,2574` (`join_output_lines`, `solve_plain`),
`crates/engine/src/cmd/mathgraf.rs`.
*Done when:* each case has a regression test in the maths test module and a Danish message.

**C4. The maths tab speaks one language — Danish** — 2–3 h
*What:* the tab is mixed today: English ribbon labels ("Calculate", "Solve", "Define", "Units",
"Matrix", "Plot graph"), English engine errors ("Could not solve: …", "Maxima did not answer
within 30 seconds") and a couple of Danish ones ("Maxima fejlede"). WordMat's users know the tab
in Danish, so make the whole tab Danish: ribbon labels, group names, dialogs, messages.
*Where:* `crates/engine/src/cmd/mathcraft.rs:2838-2843,2915-2923` (labels and groups),
`crates/engine/src/cmd/mathgraf.rs`, the maths dialogs in `crates/ui-egui/src/dialogs.rs`.
*Done when:* no English sentence is left in a user-facing maths label or message, and the
command ids (`math.beregn`, `math.enhed`, `math.isoler`) keep working — the ids are the API,
the labels are the Danish face.

**C5. Danish school templates of our own** — 3–5 h (P2, only if the arcs above are green)
*What:* templates the app offers out of the box — an assignment sheet, a report cover, a two-column
handout — written by us, with our own colours and text (never Microsoft templates).
*Where:* `crates/engine/src/sample.rs`, `crates/engine/src/cmd/design.rs`.
*Done when:* they open from the backstage view and are listed in the README.

### D. Presentation and identity — cheap, high visibility — P0/P1

**D1. README that answers "is this for me?"** — 2–3 h
*What:* rewrite the front page: what Quill is in one paragraph, what it does that Word does not
(the maths tab, Nextcloud and Obsidian, free/open source, agent control through MCP), a feature
matrix with honest status (works / partial / missing), install for **macOS, Windows and Linux**
(today only Linux is documented), a 60-second quick start, and the limits (charts are read-only,
`.doc` is read-only, no macro execution).
*Where:* `README.md`, `docs/images/`.
*Done when:* a reader can decide whether Quill replaces their editor without asking, and every
platform's install line matches what the packaging scripts actually produce (A1).

**D2. `docs/status.md` — measurable maturity** — 2–3 h
*What:* a table per area — document editing, .docx compatibility, the maths tab, stability,
platforms and distribution — with a status, the evidence behind it and the known limits.
*Where:* new `docs/status.md`, linked from the README and from `docs/gaps.md`.
*Done when:* every number in it is measured (from `cargo xtask stats`, `cargo xtask parity` and
the gap list), not estimated in prose.

**D3. Identity sweep** — 2–4 h — **done in arc 0 (the user-visible half)**
*What:* remove the remaining WordCraft names from everything a user or a packager sees:
`release.yml` (`app-name: quill`, `app-title: Quill`), the web app's title and favicon
(`apps/wordcraft-web/index.html`, `assets/app-icon/quill.ico`), the packaging scripts' output
text, `docs/releasing.md`, `SECURITY.md`, `docs/control-protocol.md`.
*Where:* the files above.
*Done when:* `rg -i wordcraft` finds nothing outside the credits (`ATTRIBUTION.md`, `NOTICE`,
`LICENSE-*`), the fork notes (`FORK.md`) and the internal crate names.
*Rule:* upstream credit stays visible — Quill is a fork, and the contributors of WordCraft are
credited in About and in `ATTRIBUTION.md`.
*What landed:* the release workflow, the web page, the CLI and app messages, the printing title,
the parity report's headline and the packaging output all say Quill; the MSI registers the app as
`Quill` (with a test that keeps the "make default" button and the installer in step). The crate
names, the `WORDCRAFT_*` build variables and the workspace's `repository` field keep their names
(§8.3). The **19 interface catalogs** were re-keyed: the fork renamed the English source strings
but not the catalogs, so 16 strings per language had lost their translation and fallen back to
English — they are translated again. The exported files (PDF, ODT, HTML, RTF, LaTeX) and the
DOCX properties now say Quill in their metadata, and the MCP server calls itself Quill.
Still open: the **wordmark in the macOS DMG background** is upstream's outlined
"WordCraft" — redrawing it needs the Inter and JetBrains Mono outlines, so it is a small
artwork task of its own.

**D4. Spell out the fork boundary** — 1 h — **done**
*What:* `FORK.md` is already a good port ledger; add a short "what is Quill's own code" table
(the maths tab, the cloud crate, the Obsidian vault, the Quill branding) and the policy for
upstream ports (which upstream files may be overwritten, which are off-limits).
*Where:* `FORK.md`, `README.md` (one paragraph).
*Done when:* a reader can tell which parts of the app are Quill's.
*What landed:* `FORK.md` opens with a **What is Quill's own code** table — the WordMat tab,
Nextcloud, the Obsidian vaults, the Danish work, the corpus harness, the branding and the
documentation, each with its files — followed by **The port policy**: Quill's own files are
never replaced by an upstream file of the same name, a row of the modified-file table may be
overwritten only together with the change it describes, everything else is merged as upstream
wrote it (no drive-by reformatting, so ports stay cheap), new code goes in new files where it
can, the upstream credit stays, and a port is one pull request naming the rows it re-applied.
The README's developer section says the same in one paragraph and points at `FORK.md`.

**D5. Changelog convention and a release checklist** — 1 h — **done**
*What:* `docs/releasing.md` still describes releasing WordCraft and states no changelog rule.
Add: the changelog format (already used in `CHANGELOG.md`), the version bump step, the checklist
before publishing (green CI, `cargo xtask ci` green, docs updated, artifacts per platform
verified), and who publishes.
*Where:* `docs/releasing.md`, `CHANGELOG.md`.
*Done when:* a dry run of the checklist is possible from the document alone.
*What landed:* `docs/releasing.md` has **The changelog** — the shape of an entry, written while
the work lands rather than scraped from commits, in the user's words, with a bullet even for a
fixed crash and none for a refactor, `Unreleased` turning into the version at release time — and
a **Before you publish** checklist that starts from the version bump and ends at the packaged
build, including the supply-chain run, `docs/status.md` and the release plan's definition of
done. **Who publishes** names the release manager: the owner reviews the drafted GitHub Release
and presses Publish, and an agent stops at the draft.

### E. Cheap parity wins — P2, only if the arcs above are green

* **E1. View ▸ window commands** (`view.arrangeAll`, `view.sideBySide`, `view.syncScroll`,
  `view.switchWindows`) — 3–5 h, gap #27, four missing catalog ids.
* **E2. Insert ▸ Ink Equation and Draw ▸ Ink to Math** (`insert.inkEquation`,
  `draw.inkToMath`) — 2–4 h, gap #12; the surrounding machinery exists.
* **E3. Review ▸ Translate and Block Authors** (`review.translate`, `review.blockAuthors`) —
  2–4 h, gap #5.

Together they take the Word catalog from 489/504 to 497/504. The last five ids
(`insert.onlinePicture`, `insert.model3d`, `insert.screenshot`, `insert.video`, `addins.get`)
need a service, a rendering pipeline or a plugin host — an owner decision (§8), not a sprint.
Every id needs a `CommandSpec`, a test and a ribbon entry, and the parity floor in
`crates/engine/src/catalog.rs:574` rises with it.

*Landed 2026-10-22:* E1 (the four View window commands), E2 (`insert.inkEquation`,
`draw.inkToMath`: the shell's ink canvas supplies the reading) and E3's Block Authors
(`review.blockAuthors`). The catalog is at 497/504 (98.6%). `review.translate` is not
implemented: Quill has no translation service, and a command that pretended to translate would be
worse than the missing id.

## 4. Sequencing

| Arc | Content | Cost | Depends on |
|---|---|---|---|
| **0 — Unblock** | A1, A2, A3, A4, D3 (packaging and CI parts) | 1 day | — |
| **1 — Trust** | B1, B2, B3, B4, D1, D2, A5, A6 | 4–6 days | arc 0 |
| **2 — Danish** | C1, C2 | 2–3 days | arc 0 |
| **3 — Maths** | C3, C4 | 1–2 days | arc 0 |
| **4 — Polish** | A7, C5, E1–E3 | 1–3 days | arcs 1–3 |

Arcs 2 and 3 can run in parallel with arc 1: they touch different crates.

### Arc 0 — landed 2026-10-21

| Item | Landed |
|---|---|
| A1 | The macOS and Windows packages build again and are named `quill-*`; the MSI is `quill.wxs`; the bundle is `Quill.app`; the DMG volume and its `.DS_Store` say Quill. The macOS package was built here: `Quill.app` (signed, valid), `quill-1.1.0-macos-aarch64.dmg` (mounts as `Quill`) and the CLI zip |
| A2 | `printing.yml` asks for the packages that exist; `cargo xtask pkg-names` (new, with tests) guards the workflows *and* the packaging scripts; it runs in the new `workflow-lint.yml`. It catches exactly the bug that was there: pointing it at the old `-p wordcraft` fails with the file and the line |
| A3 | `ci.yml` runs `cargo test --workspace` (60 test binaries, ~2,200 tests); the DOCX suite, the bin tests and the integration tests run on every push |
| A4 | `ci.yml` runs `cargo xtask assets`, `layers` and `wasm`; `cargo xtask ci` gained the `pkg-names` step |
| D3 | The user-visible names say Quill (release workflow, web page, app and CLI messages, printing title, packaging output, the files Quill writes, the MCP server) — and the 19 interface catalogs are re-keyed, so the translations the fork's rename had broken are back |
| — | Two bugs the old `--lib` job hid: a paragraph whose text is only hashes was read back as an empty heading and lost its text (`crates/formats/src/markdown.rs`), and 16 strings per language had lost their translation (`crates/ui-egui/src/i18n/*.tsv`) |

Still open in arc 0's wake: the macOS DMG background still carries upstream's outlined
wordmark (it needs the font outlines redrawn), and the CI matrix itself can only be proven on
GitHub — locally, `cargo test --workspace` is green.

### The 1.2 line, integrated — 2026-10-22

The work branches were merged into `Rasmus-03/1.2` in one line (danish-ui, danish-proof, fuzz,
parity, then the D4/D5 docs). Merging found two things a green branch alone would have hidden:

| Found on the merged line | Fixed |
|---|---|
| **Seven new commands had no translations.** `Arrange All`, `Block Authors`, `Ink Equation`, `Ink to Math`, `Switch Windows`, `Synchronous Scrolling` and `View Side by Side` were added to the registry and to English only, so `every_tab_and_command_is_translated` failed in all 19 catalogs — and the app would have shown English labels in every language | The seven labels and the four new ribbon captions are in all 19 catalogs (190 rows); the test is green |
| **Seven fuzz seeds had no `ATTRIBUTION.md` row**, so `cargo xtask assets` failed | Each seed has its row: our own bytes, written for the repo |

`cargo xtask ci` on the integrated line: **fmt, clippy, the whole test suite, assets, layers and
pkg-names are green**; the **wasm** step cannot run on this machine at all (the `rustc` here is
Homebrew's, there is no `rustup`, so the `wasm32-unknown-unknown` target cannot be installed) —
that gate is proven in CI, not here.

The parallel agents for the remaining corpus and progress-docs work could not start in this
session: Codex refuses every model on this account (`The 'gpt-5.5' model is not supported when
using Codex with a ChatGPT account`), OpenCode has run out of OpenRouter credit and the Claude
CLI reports `Credit balance is too low`. Their tasks and briefs are recorded in Orca
(`run_67e13ccb0c63`) and can be retried when an account has credit; the docs they were meant to
update (the Danish rows in `docs/status.md`, `ROADMAP.md`, `docs/gaps.md` and
`docs/localization-parity.md`, the `CHANGELOG` entries) were instead updated by the coordinator.
B1–B3 were already on the line before this session (`crates/docx/tests/corpus.rs`, the pinned
manifest, `docs/docx-corpus.md`), so the corpus task is a re-run of a test, not a build.

## 5. Parallel agents — who owns what

| Workstream | Crates | Files an agent may touch | Conflict risk |
|---|---|---|---|
| A (build) | — | `.github/workflows/*`, `packaging/*` | low |
| B1–B3 (corpus) | `docx`, `xtask` | `crates/docx/tests/*`, `xtask/src/corpus.rs`, `docs/file-format-parity.md` | low |
| B4 (stability) | `engine`, `fonts`, `ui-egui` | per-issue files | medium — one agent per issue |
| C1 (Danish UI) | `ui-egui` | `crates/ui-egui/src/i18n/da.tsv` only | low (one file) |
| C2 (Danish proofing) | `proof` | `crates/proof/src/da/*`, `lang.rs`, `patterns.rs` | low |
| C3–C4 (maths messages) | `engine` | `crates/engine/src/cmd/mathcraft.rs`, `mathgraf.rs` | medium — one agent only |
| D (presentation) | — | `README.md`, `docs/status.md`, `docs/releasing.md`, `FORK.md` | low |

Rules that apply to every agent: separate `CARGO_TARGET_DIR` per agent; edit only the crates you
own; `cargo xtask ci` before you commit; one commit per arc, with a task id in the message
(`1.2/A3: run the whole suite in CI`); nothing left in the working tree.

## 6. Definition of done for 1.2

- [ ] `cargo xtask ci` is green locally **and** in CI, on Linux, macOS and Windows. *(green
      locally at `f687951` with A5 on top: all 8 steps — fmt, clippy, test, assets, layers,
      pkg-names, wasm, clean — pass on this machine, wasm included, so the earlier note that
      this machine could not run the wasm step was wrong; the CI proof still comes with the
      first push of the new workflows. **On `Rasmus-03/mac-release` (2026-11-03) the gate does not
      start**: `xtask` fails to compile because `mod pkg_names` has no file there, so all eight
      steps are unrun — see the walk at the end of this section; **after the base repair the same
      evening the gate is green on this branch** — all 8 steps, exit 0, see "The walk again"
      below)*
- [x] **The supply-chain audit is green.** *(2026-10-30: it was red — four advisories, three in
      `zip 3.0.0` and one in `ctutils 0.4.3`. Both are upgraded (`zip` 9.1.3, `ctutils` 0.4.7),
      `cargo audit` exits 0, and `cargo xtask ci` still passes all 8 steps on the new `zip`. No
      advisory was allow-listed)*
- [x] `cargo test --workspace` runs in CI, including the 173 DOCX tests. *(arc 0)*
- [x] `cargo xtask assets`, `cargo xtask layers` and `cargo xtask wasm` run in CI. *(arc 0)*
- [x] macOS, Windows and Linux packages build from a clean checkout and are named `quill-*`.
      *(arc 0: the scripts build the `quill` packages and name every artifact `quill-*`; the
      macOS DMG and the macOS bundle were built locally; the Windows MSI needs Windows. **On
      `Rasmus-03/mac-release` (2026-11-03)** the aarch64 build compiles but the script stops at
      `lipo` on the pre-rename binary names; **after `package.sh` and `Info.plist.in` were renamed to
      the `quill` binaries, `-arch aarch64` exits 0 and produces `quill-1.1.0-macos-aarch64.dmg`
      (24,855,168 B) and `quill-cli-1.1.0-macos-aarch64.zip` (15,652,300 B), and the DMG mounts with
      `Quill.app` inside**; **`-arch universal` exits 0 as well** —
      `quill-1.1.0-macos-universal.dmg` (51,924,638 B), whose fat `Quill` (`x86_64 arm64`) passes
      `codesign --verify --strict` on the mounted image — see "The universal DMG" below)*
- [x] The corpus harness runs over the real-world corpus; every known difference is in the
      compatibility table (B3), not a surprise. *(landed 2026-10-21: 71 Apache-POI files pinned by
      SHA-256, read → write → read, every one round-trips; 2026-10-23: the page count is compared
      with Word's own recorded count — 62 of 65 exact, three one page apart, all three named —
      though not yet asserted in the harness. The page-by-page render comparison remains open in
      gap #1; 2026-10-30: the harness was run on this line after `zip` 3 → 9 and all 71 files still
      round-trip)*
- [x] The interface is Danish at the points a student meets it, errors included. *(2026-10-23:
      the maths tab's 24 English error and status strings and its formula gallery are Danish; the
      Maths tab reads Danish in all 20 interfaces, which is the open item for the others)*
- [ ] Fuzzing runs nightly over the DOCX, `.doc`, RTF/ODT and font readers. *(A7: the four
      `cargo-fuzz` workspaces, their seeds and `fuzz.yml` landed and type-check; the nightly run
      itself still needs a machine with rustup, a nightly toolchain and `cargo-fuzz`)*
- [x] The app is Danish: interface, proofing and maths messages. *(the catalog covers all 2,495
      entries the interface uses and Danish proofing landed; the native-speaker review in §8.7 is
      still open and is the owner's call)*
- [x] `README.md` describes every platform, the feature matrix and the limits; `docs/status.md`
      carries the measured maturity table. *(2026-10-30: the README has the platform table, the
      "What works, and what does not" matrix and the "Not yet" row; the row's claim that the
      maths tab's messages are still English was false — `crates/engine/src/cmd/mathcraft.rs`
      has no English user-facing sentence left — and is corrected, with the open native-speaker
      review of the Danish strings named instead)*
- [ ] `rg -i wordcraft` finds nothing user-visible outside the credits. *(arc 0 except the
      macOS DMG background's wordmark, which is artwork)*
- [x] `ROADMAP.md`, `docs/gaps.md`, `docs/roadmap.md`, `docs/target-app-parity.md`, the parity
      checklists and `CHANGELOG.md` are updated, each with its status line and revision history
      bumped (the craftrules progress-docs standard). *(2026-10-27: the last of the 1.1 numbers are
      gone. The measured rows are re-run on this line — catalog 497/504, 678 commands, 2,198
      tests, ~210,000 lines, 20 interface languages with Danish, four proofing languages — and
      the claims the corpus made false are rewritten (there is a real-world corpus and 62 of 65
      page counts match Word's; content controls are kept; contour wrap landed). The two
      duplicated feature rows in the assessment are merged, and the localization row explains why
      it still carries 27% while `localization-parity.md` computes 28%. The readiness estimates
      themselves are unchanged: this pass fixed the measurements and the evidence, not the model)*
- [ ] The release checklist in `docs/releasing.md` was walked end to end once. *(2026-10-30: walked as
      far as this machine allows — `cargo xtask ci` green; the corpus fetched (`cargo xtask corpus
      --docx`, 71 files) and `crates/docx/tests/corpus.rs` green over all 71 with `zip` 9.1.3; and
      `packaging/macos/package.sh --arch aarch64` produced `quill-1.1.0-macos-aarch64.dmg` (valid on
      disk, ad-hoc signed, 24 MB) and `quill-cli-1.1.0-macos-aarch64.zip`, whose binary reports
      `quill-cli 1.1.0`. **Not walked:** the universal DMG (no x86_64 target on this machine), the
      Windows and Linux packages, the CI matrix and the `release.yml` dry run — those need other
      machines or the remote — and the packaged app was not opened to check About. **2026-11-03,
      `Rasmus-03/mac-release`:** walked again and blocked at the first command — the package script
      exits 101 on the pre-rename package ids and the dry run is refused (HTTP 422, no such ref),
      so neither macOS artifact existed. **After the base repair and the `package.sh`/`Info.plist.in`
      rename** the walk reaches its end on this machine: `cargo xtask ci` is green (all 8 steps), and
      `package.sh --arch aarch64` exits 0 with both `quill-*` artifacts on disk and the DMG mounted
      once to show `Quill.app` inside. **With `x86_64-apple-darwin` installed the universal DMG is
      walked too** (50 MB, `x86_64 arm64`, mounted app verifies strictly). Still not walked:
      `release.yml`, Windows and Linux, and notarization)*

### The macOS checklist walk on `Rasmus-03/mac-release` — 2026-11-03

This file is not on that branch (it lives on `Rasmus-03/1.2`), so it was copied from there to
record the walk where the plan says results go. Both rows this machine can reach were walked, and
both stopped **before they measured anything**. Nothing below is inferred: every exit code is the
command's own, and every artifact claim is one file listing.

| Command, run in the checkout root | Exit | What it printed |
|---|---|---|
| `cargo xtask ci` | 101 | `error[E0583]: file not found for module pkg_names` (`xtask/src/main.rs:12`). `xtask` does not compile, so **none of the eight steps ran** — fmt, clippy, test, assets, layers, pkg-names, wasm and clean are all unrun. The module is committed on `Rasmus-03/1.2` (`a2d1897`, `078a3f6`), not here |
| `packaging/macos/package.sh --arch universal` | 101 | `error: package ID specification 'wordcraft' did not match any packages`. The script still builds `-p wordcraft -p wordcraft-cli`; the workspace packages are `quill` and `quill-cli`. Its next step would copy `assets/app-icon/wordcraft.icns`, which is `quill.icns` on disk. **No DMG and no CLI zip**; `dist/release/` was empty afterwards |
| `gh workflow run release.yml --ref Rasmus-03/mac-release` | 1 | `could not create workflow dispatch event: HTTP 422: No ref found for: Rasmus-03/mac-release`. The branch was never pushed (origin has `main`, `Rasmus-03/feat-quill-word-features`, `Rasmus-03/gudgeon`); pushing it is the owner's call, so **the dry run was not dispatched** |
| `~/.local/bin/quill-clean` | 0 | removed this checkout's `target/` and `dist/`; printed `freed 0 GB` (the space freed rounded to under a gigabyte) and `43Gi available`. `sprog-wizard/target` was in use by another workspace and could not be removed |

What that leaves open, in plain terms:

- **`cargo xtask ci` is unproven on this branch**, which is not the same as red on its own merits:
  the gate cannot start. The eight-step pass recorded in the row above belongs to the
  `Rasmus-03/1.2` line at `f687951`, not to this tree.
- **Neither macOS artifact exists.** The universal DMG and the CLI zip were not produced, so this
  branch does not yet earn the row that says the packages build from a clean checkout. A second
  blocker sits behind the first and was never reached: the universal build needs the
  `x86_64-apple-darwin` target, and only `aarch64-apple-darwin` is installed here.
- **The release dry run did not run**, so the unsigned Linux, riscv64, Flatpak, FreeBSD and web
  jobs remain unexercised for this branch.
- **Windows and Linux were not walked at all** — no MSI, no AppImage/Flatpak/deb/rpm, no
  cross-platform matrix. Nothing in this pass says anything about them.

The first three rows are arc 0's A1 and A2 and the `pkg_names` module, all of which landed on the
1.2 line and not here. Getting them is a merge, not a measurement, and this pass did not do it.

### The walk again, after the base repair — 2026-11-03

The repair restored `xtask/src/pkg_names.rs` and renamed the `-p` arguments in the workflows and the
four packaging files (copied from `363865c`), all uncommitted and none of it this pass's to alter.
Both commands were then re-run in the same checkout; nothing below is carried over from the earlier
walk.

| Command, run in the checkout root | Exit | What it printed |
|---|---|---|
| `cargo xtask ci` | 0 | `CI summary: all 8 steps passed (fmt, clippy, test, assets, layers, pkg-names, wasm, clean)`. **test**: 2,105 passed, 0 failed, 29 ignored across the workspace. **assets**: "all 55 asset files attributed in ATTRIBUTION.md". **layers**: "OK: 21 crates, no layering violations." **pkg-names**: "every package name in the workflows and the packaging scripts exists". **wasm**: 17 of 17 crates `ok`, plus the browser build of `wordcraft-cloud` type-checked on the host. **clean**: 11,451 entries removed, 4,879.3 MB freed (the test step had already freed 199 entries / 12.3 MB) |
| `packaging/macos/package.sh --arch aarch64` | 1 | built `quill` and `quill-cli` in `--release --locked` for `aarch64-apple-darwin` in 4 m 23 s, then stopped in the `lipo` loop: `lipo: warning: file not found '…/target/aarch64-apple-darwin/release/wordcraft'` / `lipo: no eligible inputs found`. **No DMG and no CLI zip**; `dist/release/` was empty afterwards |

So the first row is earned on this branch and the second is not. Why it still stops: the `-p` names
were repaired, the **binary** names were not. The build produces `quill` (59,781,808 bytes) and
`quill-cli` (36,369,952 bytes) under `target/aarch64-apple-darwin/release/`; the script still asks
`lipo` for `wordcraft` and `wordcraft-cli` (line 66) and copies those paths (lines 104 and 158),
named its artifacts `wordcraft-<version>-…` (lines 43–44), and copies
`assets/app-icon/wordcraft.icns` (line 105) where the file on disk is `quill.icns`. The packaged
half of that row is therefore a rename away, not a measurement away — and it was not done here
because those files belong to the repair.

One more rename residue, seen while checking the build: `quill-cli --version` prints
`wordcraft-cli 1.1.0`, so the name has not reached the `--version` string, which bears on the
`rg -i wordcraft` row.

**Not walked, and nothing above says otherwise:** the universal DMG (only `aarch64-apple-darwin` and
`wasm32-unknown-unknown` are installed), `gh workflow run release.yml` (the branch is not on origin),
and the Windows and Linux packages. `~/.local/bin/quill-clean` was run afterwards: exit 0, 16 GB
freed, 41 GiB free.

### The blocker fixed, and the aarch64 artifacts built — 2026-11-03

The mismatch left after the base repair lived in `packaging/macos/package.sh` and its plist template:
the script asked `lipo` for binaries called `wordcraft`/`wordcraft-cli`, named its artifacts
`wordcraft-*`, copied `assets/app-icon/wordcraft.icns` (a file that does not exist — the icon is
`quill.icns`) and assembled `WordCraft.app` around an executable called `WordCraft`, which
`CFBundleExecutable`/`CFBundleIconFile` in `Info.plist.in` also named. Both files now use what the
build emits: `quill` and `quill-cli`, `quill-<version>-macos-<arch>.dmg`,
`quill-cli-<version>-macos-<arch>.zip`, `Quill.app` with `Contents/MacOS/Quill` and
`Contents/Resources/Quill.icns`, and a plist whose `CFBundleName`, `CFBundleDisplayName`,
`CFBundleExecutable` and `CFBundleIconFile` are `Quill`. No crate, workflow or base-repair file was
touched, and nothing was committed.

| Command, run in the checkout root | Exit | What it printed |
|---|---|---|
| `packaging/macos/package.sh --arch aarch64` (from an empty `dist/release/`) | 0 | `--release --locked` build of `quill` and `quill-cli`, then `lipo` on one binary each (an `aarch64` build needs no fat binary), `plutil -lint` on the generated plist, in-out `codesign`, the DMG, the CLI zip, and finally `quill-cli --version` → `wordcraft-cli 1.1.0`, `==> done` |

What is in `dist/release/`, measured with `stat -f '%z'`:

| Artifact | Bytes | SHA-256 |
|---|---|---|
| `quill-1.1.0-macos-aarch64.dmg` | 24,855,168 (24 MB) | `1a28a654715387807826b195925bd1fe0826ec8ab3a6ae7dab00c6ecf849b662` |
| `quill-cli-1.1.0-macos-aarch64.zip` | 15,652,300 (15 MB) | `1c07ac5ae1992b4e21d8783b9f8dfd09c2da03b5e5daabee0ca81921378d63e3` |

**The DMG was mounted once** (`hdiutil attach -nobrowse -readonly`): the checksums of all three
image partitions verified, the volume mounted at `/dev/disk6s2`, and `ls` showed `Quill.app`,
`Applications -> /Applications`, `.background/` and `.DS_Store`. Inside the app:
`Contents/Info.plist`, `Contents/PkgInfo`, `Contents/_CodeSignature`, `Contents/MacOS/Quill`
(59,451,424 bytes) and `Contents/Resources/Quill.icns` (110,290 bytes); `plutil -p` on the mounted
app's plist read `CFBundleName`/`CFBundleDisplayName`/`CFBundleExecutable`/`CFBundleIconFile` =
`Quill`, `CFBundleIdentifier = io.github.rasmus03.quill`, `CFBundleShortVersionString = 1.1.0`. The
volume was ejected afterwards. The CLI zip holds 5 entries — the `quill-cli-1.1.0-macos-aarch64/`
folder with `quill-cli` (36,175,008 bytes), `README.md`, `LICENSE-MIT` and `LICENSE-APACHE` — and the
unpacked binary runs.

Still open, named rather than rounded off:

- **The volume is still called `WordCraft`.** It comes from `hdiutil -hfs-volume-name`, and the DMG
  artwork depends on it: the background is found through an alias that includes the volume name, and
  `dmg-layout.DS_Store` positions an icon whose name is `WordCraft.app`. Renaming the bundle to
  `Quill.app` therefore leaves the Finder layout pointing at an icon that is no longer there, so the
  window shows the icon at Finder's default position until that artwork is regenerated. The artwork
  is out of this change's scope.
- **The mounted copy fails `codesign --verify --strict`** with "resource fork, Finder information, or
  similar detritus not allowed (Disallowed xattr com.apple.FinderInfo on …/Resources/Quill.icns)":
  the source icon has no xattr and the built app verifies clean with and without `--deep`, so the HFS
  image adds the attribute. `spctl --assess --type execute` rejects the app, as it must for ad-hoc
  signing (no `MACOS_SIGN_IDENTITY` here); notarization needs the Apple secrets and a real identity.
  *(fixed below — the script clears the attribute on the raw image)*
- **`quill-cli --version` still prints `wordcraft-cli 1.1.0`** — the name has not reached that string,
  which is what the `rg -i wordcraft` row is about. *(fixed below: it prints `quill-cli 1.1.0`)*
- **Not walked:** the universal DMG (only `aarch64-apple-darwin` and `wasm32-unknown-unknown` were
  installed targets at that point), `gh workflow run release.yml` (the branch is not on origin), the
  Windows and Linux packages, and notarization. *(the universal DMG is walked below, once
  `x86_64-apple-darwin` was installed)*

### The universal DMG, the detritus fix and the CLI's name — 2026-11-03

**(a) The mounted app verifies.** `package.sh` now copies the app into the DMG stage with
`ditto --norsrc --noextattr`, so nothing the build host attached travels into the image. That alone
would not have fixed the finding, and the walk measured why: a two-file test image —
`Quill.app/Contents/{Info.plist, Resources/Quill.icns}`, no xattr on either file — still mounts with
`com.apple.FinderInfo` on the `.icns`, because hdiutil's HFS writer stamps the attribute itself. So
the script gained `strip_detritus`, which attaches the raw HFS image read-write once between
`makehybrid` and the final `convert` and clears the app's extended attributes; a code signature does
not cover xattrs, so no code changed. The image stays HFS: the artwork's background alias and the
`.DS_Store` layout depend on that volume, and `hdiutil create -srcfolder` (the only route to an APFS
image, which carries no Finder info) is the flaky-on-CI path the script deliberately avoids.

**(b) `quill-cli --version` prints `quill-cli 1.1.0`.** The string was hard-coded in
`apps/wordcraft-cli/src/main.rs` (`"--version" | "-V" => … "wordcraft-cli {}"`), so its siblings in
that binary's output were fixed with it: the three `wordcraft-cli:` prefixes (the error path, the MCP
key warning, the file-confinement notice) and `can't reach the WordCraft app at {addr}`. Deliberately
left alone: the parity markdown's `# WordCraft feature parity` title, because `cargo xtask parity`
writes the same heading into `docs/parity-checklist.md` and changing one side would put the two out
of step — that is a rename of the parity generator and its doc, not of a CLI string — and the
`description` in `apps/wordcraft-cli/Cargo.toml`.

**(c) The universal DMG exists.**

| Command, run in the checkout root | Exit | What it printed |
|---|---|---|
| `packaging/macos/package.sh --arch universal` (from an empty `dist/release/`) | 0 | both targets compiled, `lipo` produced a fat `Quill` (123,856,928 bytes inside the bundle), then the DMG, the CLI zip, and `quill-cli --version` → `quill-cli 1.1.0` |

| Artifact | Bytes | SHA-256 |
|---|---|---|
| `quill-1.1.0-macos-universal.dmg` | 51,924,638 (50 MB) | `d3578823804f8cfb5fe669203513c6115a7854d438b15c7a8cba08221c09d1c2` |
| `quill-cli-1.1.0-macos-universal.zip` | 32,790,871 (31 MB) | `9f8bff6b14ecf2c06d69ba116e5ae3bdc8b3650e980013bb10631e2ae6b57f7e` |

The acceptance checks, run on the mounted image (`/dev/disk6s2`, read-only, volume still named
`WordCraft`):

- `lipo -archs Quill.app/Contents/MacOS/Quill` → `x86_64 arm64` (exit 0).
- `xattr -l …/Contents/Resources/Quill.icns` → nothing.
- `codesign --verify --strict` → **exit 0**, "valid on disk / satisfies its Designated Requirement";
  `--strict --deep` also exits 0. This is the check that failed before the strip pass.
- `spctl --assess --type execute` still rejects the app, correctly: it is ad-hoc signed (no
  `MACOS_SIGN_IDENTITY` here), and notarization needs the Apple secrets and a real identity.
- The CLI zip holds 5 entries (binary 75,611,296 bytes); the unpacked binary prints `quill-cli 1.1.0`
  and `lipo -archs` on it says `x86_64 arm64`.

**Those two files were then deleted, and rebuilt.** The boss's tidy-up at the end of the run called
`quill-clean`, which removed every worktree's `target/` and `dist/` wholesale — a verified DMG is a
deliverable, not scratch, so `quill-clean` now keeps `dist/release/*.dmg` and `*.zip` and removes
only the rest. Rebuilding after that fix (`--arch universal`, same source, ad-hoc signing) reproduced
the layout and every acceptance result above; the bytes differ from the table because an ad-hoc
signature and the embedded build timestamps are not reproducible:

| Artifact | Bytes | SHA-256 (rebuild, 19:33) |
|---|---|---|
| `quill-1.1.0-macos-universal.dmg` | 51,924,645 | `63df984707873e737ee0bb4a5a9fee6cfa4caebddf9f6e5232d9ae2d73fa4ce1` |
| `quill-cli-1.1.0-macos-universal.zip` | 32,790,871 | `bd651449cf292c1283d7a0006c2e5db84189bb3d393ad9b5187d1beb90518f0a` |

### 1.2.0 — the number behind the name — 2026-11-03

The app had been Quill since the rename, but `[workspace.package] version` in the root `Cargo.toml`
was still `1.1.0`, so every artifact carried the old number. It is now `1.2.0` — one line, because no
member pins a version by hand (they all inherit `version.workspace = true`, and the internal
`workspace.dependencies` entries are path-only). `Cargo.lock` was refreshed with
`cargo update --workspace`, which moved all 21 workspace packages to 1.2.0 and left the third-party
pins untouched (33 → 12 `1.1.0` occurrences in the lock, all of them other people's crates).
`cargo xtask version` → `1.2.0`; `cargo metadata --no-deps` resolves `quill 1.2.0`, `quill-cli 1.2.0`,
`wordcraft-engine 1.2.0`, `wordcraft-doc 1.2.0`.

| Command, run in the checkout root | Exit | What it printed |
|---|---|---|
| `packaging/macos/package.sh --arch universal` | 0 | `==> Quill 1.2.0 for macOS (universal), identity: -, notarize: 0`, the fat `Quill` build, the DMG, the CLI zip, `quill-cli 1.2.0`, `==> done` |

| Artifact | Bytes | SHA-256 |
|---|---|---|
| `quill-1.2.0-macos-universal.dmg` | 51,933,283 (50 MB) | `00ba5e6c7cc3eba3e24526ecc5e2fec2da1a08688475c6cad70c5bef714bd91b` |
| `quill-cli-1.2.0-macos-universal.zip` | 32,788,982 (31 MB) | `a1069c9f146a72d82a517d40381c23bbb198823066dd1250db450ab9c99232b2` |

On the mounted image (`/dev/disk6s2`, read-only): `CFBundleShortVersionString` = `CFBundleVersion` =
`1.2.0`, `CFBundleName` = `Quill`, `lipo -archs Quill.app/Contents/MacOS/Quill` → `x86_64 arm64`, and
`codesign --verify --strict --deep` → **exit 0**. The unpacked CLI reports `quill-cli 1.2.0`, is
75,629,968 bytes and `x86_64 arm64`. The 1.1.0 artifacts were deleted from `dist/release/` after this
build, so the directory holds exactly one version.

The volume was ejected afterwards. **Not walked:** the `release.yml` dry run (the branch is not on
origin), the Windows and Linux packages, and notarization. The volume name and the DMG artwork's
`WordCraft.app` icon entry remain the rename residue named in the previous walk.

## 7. What this release deliberately does not do

No chart or SmartArt editing, no `.doc` writing, no macro execution, no add-ins, no in-app
assistant, no East Asian typography work, no encrypted-file support, no second cloud provider,
no engine-level changes to layout. These stay in [`docs/gaps.md`](gaps.md) with their hours.
Reason: 1.2 is about trust and Danish, not breadth — every hour spent on breadth is an hour not
spent on the file that breaks a student's assignment.

## 8. Owner decisions needed

1. **Aptos metric font** (gap #4): Word's default since 2024 has no metric-compatible open
   substitute, so documents wrap differently. Commission a metric font, accept the difference,
   or keep the substitution as is?
2. **Danish dictionary**: rely on an installed `hunspell-da` (as Polish and Portuguese do today,
   nothing ships in the app), or vendor a licence-clean word list in the repo (needs an
   ATTRIBUTION row and a size decision)?
3. **Internal crate names**: the workspace still calls the crates `wordcraft-*`. Renaming to
   `quill-*` is a large diff that makes every upstream port harder. Recommendation: keep the
   internal names, keep the user-visible names Quill, and say so once in `FORK.md` and the README.
4. **Nextcloud is done — which cloud next?** OneDrive, Google Drive or Dropbox (gap #22), or
   none in 1.2?
5. **Release branch**: this fork has no `release` branch, so `release.yml` (which triggers on a
   push to `release`) has never run here. Create one and do a dry run?
6. **Signing secrets**: the Apple and Azure signing credentials are upstream's. Does Quill
   publish signed artifacts under its own name with them, or unsigned?
7. **Danish translation quality**: who reviews the Danish strings — the owner, a Danish teacher,
   or a volunteer? Machine translation is not acceptable for a school product.

## 9. Risks

| Risk | Effect | Mitigation |
|---|---|---|
| The Danish interface is ~2,100 strings | A sloppy translation is worse than English | Ship the maths tab, dialogs and ribbon first; native review before the release (§8.7) |
| Corpus licences are unclear | Corpus work stalls | Only licence-clean sources; provenance recorded per file (B2) |
| CI secrets and reusable workflows are upstream's | The matrix or the release job cannot be fixed locally | A1–A4 need no secrets; upstream changes are separate PRs |
| The maths tab is one file and one agent | Parallel edits collide | One agent for C3–C4 (see §5) |
| Upstream merges keep coming | Ports grow | `FORK.md` ledger; keep Quill's code in new files wherever possible |

## 10. An outside critic read 1.2

The whole point of a fresh reader is that they have not spent three days staring at the same
gates. On 2026-10-23 a critic agent was given the 1.2 line at `d7ab447` and one instruction the
owner added: *be a critic, and be very creative*. It read the plan, the changelog, the status
page, the README, the corpus and the workflows against the code, and wrote
[`review-1.2.md`](review-1.2.md) (committed as written). It did not build the app, and says so.

Its verdict, in one line: **the release's trust story contradicted itself in the places a
newcomer actually reads.** Eight findings, and what became of each:

| # | What it found | Now |
|---|---|---|
| F1 | The README said Danish was *not* among the interface languages, put the corpus at **489 of 504** (the generated number is 497) and credited it to four producers when every manifest URL is Apache POI. The status page said the corpus is checked "on every build". | **Fixed.** README: 497 of 504, 20 languages with Danish, the corpus named for what it is (POI's Word, Outlook and LibreOffice files, pinned by hash) and the page-count result. `docs/status.md` says the check runs where the corpus is fetched; `docs/releasing.md` makes fetching it a pre-publish step, because CI cannot |
| F2 | The maths tab was Danish in its labels and **English in the errors a student actually hits** (`Could not solve ODE`, `Select e.g. …`), and the i18n coverage test excludes the whole Math namespace, so a green badge could coexist with it | **Fixed.** 24 error and status strings and the formula gallery's labels are Danish (`Kunne ikke løse differentialligningen: …`, `ingen reel løsning`, `2 < x eller x < -2`), the two tests that pinned the English text follow, and the exclusion's comment now says what is true instead of calling the tab English. The tab being Danish-only for all 20 interfaces is the open item, not English prose |
| F3 | `corpus.rs` returns normally when the corpus is absent and CI never fetches it, so a green CI run says nothing about the corpus | **Fixed.** CI has a `corpus` job of its own (`.github/workflows/ci.yml`) that fetches the pinned files and then runs the round trip; the test also fails when the manifest lists a file the corpus no longer has, so the job cannot pass by checking less than it says. `docs/status.md`, `docs/docx-corpus.md` and the release checklist say so. The cost is named rather than hidden: an upstream change to a pinned file turns the job red, which is what a pinned hash is for |
| F4 | The round-trip predicate compares text, tables, parts and picture bytes — not formatting, page settings, fields or pagination, which the user-facing table also promises | **Fixed.** The harness compares the document, not a projection of it: every paragraph's runs and properties, the inline objects, the tables, the parts, and then the sections, numbering, styles, fields, comments, tracked changes, settings, sources and custom properties as JSON — with a hundredth-of-a-point rounding (a save writes twips), relationship ids, part numbers and the order of zero-width markers relaxed, and `QUILL_CORPUS_DIAG=1` naming the first place two documents part company. Its first run found a real defect: a header or footer that is only a table gained an empty paragraph on every save (fixed in `write/story.rs`). **Open:** a rendered, page-by-page comparison with Word |
| F5 | Ten Danish terms "deserve review", including `Ceiling` → "Loft" and `Vault` → "Boks" | **Checked one by one.** Three were right and are fixed (Highlighter, Vault Tags, Default Paragraph Font); two of the critic's own suggestions were wrong — "Loft"/"Gulv" *are* the standard Danish terms for ceiling and floor, and "Boks" is the strongbox reading every other catalog uses. Four are left for a native speaker (see [`localization-parity.md`](localization-parity.md)) |
| F6 | The release recipe cannot be followed to a proven release: no `release` branch, the workflow never ran, signing is blocked by the environment's branch rule | **Open, owner decision** (§8.5, §8.6). The dry run needs the branch the owner has not created yet |
| F7 | Home's last two big groups are Voice and Editor, while Insert Table, Insert Picture and Word Count — what a school user reaches for first — are a tab away | **Done, with the cost named.** Home ends with a compact **Insert** group — Table, Pictures, Word Count — before Voice and Editor, using Insert's and Review's own command ids. Checked on a headless render: Home already ran wider than a 1440 pt window before the change (the Editor group was clipped), so the tab now scrolls with the edge chevrons; Word's group collapsing at narrow widths is the remaining piece and is still open |
| F8 | Green gates do not prove native readers, a Danish dictionary present, a screen reader, or a package that launches on each platform | **Accepted and named.** These are the limits the plan already admits (§9); the critic's "evidence sheet" is a good shape for the release checklist, and §6 now says which of these have to be walked by hand |

Two of its "surprises" were the ones worth having: that the translation test removed the Math
feature before checking it, and that three documents of prose (README, status page, reply to a
student) described three different products. Both are now false.

### The page count (B1) was measurable all along

The corpus carried a pagination check nobody had opened: Word writes the page count it computed
into every file it saves (`docProps/app.xml`). 65 of the 71 corpus files carry one; **62 come out
of Quill at one page per page of Word's**, three are one page apart (`60329.docx` 3 → 4,
`drawing.docx` 20 → 21, `form_footnotes.docx` 3 → 2). That is B1's page count measured against
Word, and it says plainly what it does not prove — a matching total is not a matching page. It is
a measurement, not yet an assertion in the harness: a test that asserts it would depend on the
fonts installed on the machine running it, so the numbers are recorded and the check stays in the
release checklist. The detail, and how to repeat it, is in
[`file-format-parity.md`](file-format-parity.md#pagination-against-word).

## 11. Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-03 | minor | The macOS walk reaches its end. `package.sh` copies the app into the DMG stage with `ditto --norsrc --noextattr`, and because hdiutil's HFS writer stamps `com.apple.FinderInfo` on the `.icns` regardless (measured on a two-file test image), a `strip_detritus` pass clears the app's xattrs on the raw image before the final compression — so `codesign --verify --strict` on the **mounted** `Quill.app` exits 0, which is what failed before. `quill-cli --version` prints `quill-cli 1.1.0`: the hard-coded string and its siblings in `apps/wordcraft-cli/src/main.rs` (three `wordcraft-cli:` prefixes and `can't reach the WordCraft app`) now say `quill-cli`/`Quill`; the parity markdown's heading and the crate description were left because `cargo xtask parity` writes the same heading into `docs/parity-checklist.md`. With `x86_64-apple-darwin` installed, `package.sh --arch universal` exits 0: `quill-1.1.0-macos-universal.dmg` (51,924,638 B, fat `Quill` reporting `x86_64 arm64`) and `quill-cli-1.1.0-macos-universal.zip` (32,790,871 B, binary prints `quill-cli 1.1.0`). Still not walked: the `release.yml` dispatch, Windows, Linux, notarization; the DMG volume name and artwork remain the rename residue |
| 2026-11-03 | minor | The macOS walk is finished on `Rasmus-03/mac-release`. `packaging/macos/package.sh` and `packaging/macos/Info.plist.in` now name what the build emits — `quill`/`quill-cli`, `quill-<version>-macos-<arch>.dmg`, `quill-cli-<version>-macos-<arch>.zip`, `Quill.app` around `Contents/MacOS/Quill` and `Contents/Resources/Quill.icns` — instead of `wordcraft`, which made `lipo` fail on a file that was never built and pointed at an icon that does not exist. `package.sh --arch aarch64` from an empty `dist/release/` now exits 0: `quill-1.1.0-macos-aarch64.dmg` (24,855,168 B) and `quill-cli-1.1.0-macos-aarch64.zip` (15,652,300 B), with the DMG mounted once to show the volume and `Quill.app` inside, its plist naming `Quill` and its executable 59,451,424 B. Still open and named in §6: the volume is still `WordCraft` (the DMG artwork's alias and `.DS_Store` icon entry depend on that name), the mounted copy fails strict `codesign --verify` on HFS-stamped `com.apple.FinderInfo`, ad-hoc signing is rejected by `spctl` as expected, and `quill-cli --version` still prints `wordcraft-cli 1.1.0`. Not walked: the universal DMG, the `release.yml` dispatch, Windows and Linux, notarization |
| 2026-11-03 | minor | The walk is re-run on `Rasmus-03/mac-release` after the base repair. `cargo xtask ci` exits 0 — `all 8 steps passed (fmt, clippy, test, assets, layers, pkg-names, wasm, clean)`, 2,105 tests passed with 0 failed and 29 ignored, 55 assets attributed, 21 crates with no layering violation, 17 of 17 crates green on wasm — so the earlier "the gate cannot start on this branch" is retired. `packaging/macos/package.sh --arch aarch64` exits 1: the `--release --locked` build of `quill` and `quill-cli` succeeds in 4 m 23 s, then `lipo` stops with "file not found …/target/aarch64-apple-darwin/release/wordcraft", because the `-p` names were repaired but the **binary** names were not (lines 66, 104, 158), and the artifact and icon names are stale too (43–44, 105); `dist/release/` is empty, so neither the DMG nor the CLI zip exists. The built `quill-cli --version` still prints `wordcraft-cli 1.1.0`. Not walked: the universal DMG (no `x86_64-apple-darwin` target installed), the `release.yml` dispatch (the branch is not on origin), and Windows and Linux. `~/.local/bin/quill-clean` afterwards freed 16 GB |
| 2026-11-03 | minor | The release checklist is walked on `Rasmus-03/mac-release` and stops before it measures anything. `cargo xtask ci` exits 101: `xtask` cannot compile because `mod pkg_names` has no file on that branch, so all eight steps are unrun — the earlier green belongs to the 1.2 line. `packaging/macos/package.sh --arch universal` exits 101 on `-p wordcraft`, a package id that no longer exists (they are `quill`/`quill-cli`), so no DMG and no CLI zip were produced; the universal build would also need the `x86_64-apple-darwin` target, which this machine does not have. The `release.yml` dry run is refused with HTTP 422 because the branch is not on origin, and pushing it is the owner's call. Windows and Linux stay unwalked, and the file was copied here from `Rasmus-03/1.2`, where it lives |
| 2026-10-30 | minor | The four audit advisories are gone without suppressing any of them: `zip` 3.0.0 → 9.1.3 (one real API break — `ZipFile::name()` is a `Result` now, so a non-UTF-8 entry name is skipped like an unreadable entry instead of failing the document) and `ctutils` 0.4.3 → 0.4.7. `cargo audit` exits 0, and `cargo xtask ci` passes all 8 steps on the new `zip`. Separately, the fuzz corpus that had collected in the working tree is coverage only: 2,050 inputs and **no** `crash-*`/`timeout-*` artifact, so nothing was promoted to a named seed and the tree is clean |
| 2026-10-30 | minor | A6 is closed by proving it, and the proof is red. `cargo-audit 0.22.2` on this line exits 1 with four advisories: three in `zip 3.0.0` (SUID/SGID preservation, symlink escape, permissions applied through symlinks — rated up to 9.1 critical, all in `ZipArchive::extract`, which Quill never calls; the DOCX reader reads entries and validates names itself) and `ctutils 0.4.3` (non-constant-time masks, reachable through `digest → hmac` in the DOCX package, fixed by a patch). Seven unmaintained-crate warnings do not fail the run. The `supply-chain.yml` jobs already existed; what was missing was this proof. Nothing was allow-listed or silenced — the release now carries an open decision: upgrade `zip` 3 → 9, or allow-list the three unreachable advisories with that reasoning |
| 2026-10-30 | minor | §6's README item is closed. The README's "Not yet" row claimed the maths tab's messages are still English; the code says otherwise — every user-facing string in `crates/engine/src/cmd/mathcraft.rs` is Danish and no English sentence is left in it — so the row now says the tab is Danish and names what is actually open, the native-speaker review of the Danish strings. The platform table, the feature matrix and the limits themselves were already there |
| 2026-10-30 | minor | A5 is closed: the last two mutable references in `.github/workflows/` (the `taiki-e/install-action` tool tags `@cargo-audit` and `@cargo-llvm-cov`) are pinned to the commits those tags pointed at, so every `uses:` is a 40-hex commit. `zizmor`/`actionlint` are not installed here, so that half of the done-when is named as unrun rather than claimed. The gate was re-run on this line: `cargo xtask ci` passes all 8 steps at `f687951` with this change, wasm included — which corrects §6's claim that this machine cannot run the wasm step |
| 2026-10-27 | minor | §6's progress-docs item is closed: the measured numbers are re-run on the 1.2 line in `ROADMAP.md`, `docs/roadmap.md`, `docs/target-app-parity.md` and `docs/status.md` (catalog 497/504, 678 commands, 2,198 tests, ~210,000 lines, 20 interface languages, four proofing languages), the claims the corpus made false are rewritten, the assessment's two duplicated feature rows are merged, and the localization row explains the 27% it keeps against the 28% its own doc computes. Still open in §6: the CI proof on Linux, macOS and Windows, the nightly fuzz run, the README/status pair's remaining walk, the release checklist's end-to-end pass, and the two owner decisions behind them |
| 2026-10-23 | minor | Three of the critic's findings closed in code: (1) **the corpus in CI** — a `corpus` job fetches the hash-pinned files and runs the round trip, and the test fails when the manifest lists a file that is missing, so the job cannot pass by checking nothing; (2) **the round-trip harness widened** — the comparison is now the document as JSON (runs and paragraph properties, inline objects, tables, sections, numbering, styles, fields, comments, tracked changes, settings, sources, custom properties) with twips rounding, relationship ids, part numbers and zero-width marker order relaxed; its first run fixed a header that is only a table growing an empty paragraph on every save; (3) **Home gets an Insert group** — Table, Pictures, Word Count before Voice and Editor, checked on a headless render; the ribbon still runs wider than a 1440 pt window, so group collapsing is open |
| 2026-10-23 | minor | An outside critic read the 1.2 line (`docs/review-1.2.md`, crossed in §10). Fixed from it: the maths tab's 24 English error and status strings and its formula gallery are Danish (a student's mistake no longer answers in English), the README's stale catalog number and its false "Danish is not among the languages" and "checked on every build" claims, the status page's corpus claim, and three Danish terms that read wrong. Opened and named: the corpus check in CI, the round-trip harness's narrower-than-claimed predicate, the release dry run, Home's ribbon order. Pagination against Word measured: 62 of the 65 corpus files whose count Word recorded match exactly |
| 2026-10-22 | minor | The 1.2 line is integrated: Danish UI (C1), Danish proofing (C2), nightly fuzzing (A7), the parity wins (E1–E3) and D4/D5 merged into one branch, plus the two bugs the merge exposed (seven untranslated commands, seven unattributed fuzz seeds). The Danish and proofing rows of the progress docs say what landed. The gates are green except the wasm step, which this machine cannot run |
| 2026-10-22 | minor | D4 and D5 landed: `FORK.md` now says what is Quill's own code and the rules a port follows; `docs/releasing.md` carries the changelog convention, the pre-publish checklist and who publishes |
| 2026-10-22 | minor | A7 landed: `cargo-fuzz` workspaces for the DOCX, `.doc`, RTF/ODT and font readers with synthetic seeds, and a nightly workflow that runs each target for 60 seconds, caches the corpus and uploads crashes. The targets type-check; the first real fuzz run waits for a machine with rustup, nightly and `cargo-fuzz` |
| 2026-10-21 | major | Created: the 1.2 plan — build trust (A), real documents (B), Danish first (C), presentation and identity (D), cheap parity wins (E); ten measured findings with file references; seven owner decisions |
| 2026-10-21 | minor | Arc 3 landed: B4 audited — the shape tool works (a new test drags and resizes an inserted shape, so #142 is closed and pinned), the Windows startup crash is prevented by never enumerating the Vulkan driver, and the other field failures in the list are closed upstream. The gap list is true again |
| 2026-10-21 | minor | Arc 2 landed: C5 — three Danish school templates of our own (an assignment sheet, a report cover, a two-column handout), offered in File › New; the text and the layout are ours |
| 2026-10-21 | minor | Arc 1 landed: C3 (the maths tab says which of the four outcomes a solve had — no solution, every number, complex only, not solvable symbolically — in Danish, with a division by zero and a pole told apart) and C4's face: the tab's labels, groups, dialogs and messages are Danish, the command ids unchanged. The equation gallery's own labels stay in the catalogs and come with the Danish catalog (C1) |
| 2026-10-21 | minor | Arc 0 landed: A1–A4 and the user-visible half of D3. The macOS and Windows packages build again under the Quill names (the macOS one was built here: `Quill.app`, the DMG that mounts as `Quill`, the CLI zip), the CI matrix asks for the packages that exist (`cargo xtask pkg-names` guards it), the whole test suite runs in CI, and the repo's own gates run there too. Running the whole suite found two bugs the old `--lib` job hid: a paragraph of only hashes lost its text in Markdown, and 16 strings per language had lost their translation after the rename — both fixed |

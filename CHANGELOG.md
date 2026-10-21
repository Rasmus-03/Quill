# Changelog

## Unreleased

The first steps of 1.2 (the plan lives in [`docs/release-plan-1.2.md`](docs/release-plan-1.2.md)):
the build is trustworthy again, and the app carries its own name everywhere a user sees it.

- **The macOS and Windows packages build again**. Since the rename to Quill, the packaging
  scripts still asked cargo for the old `wordcraft` packages and copied an icon that no longer
  existed, so neither platform could be packaged at all. The bundle is `Quill.app`, the artifacts
  are `quill-…`, the MSI registers the app as Quill, and the disk image's window knows the new
  name
- **The CI matrix asks for packages that exist**. The Windows and macOS jobs named a package
  that was gone, so they failed before a single test ran; a new check (`cargo xtask pkg-names`)
  reads the workflows and the packaging scripts and fails when one names a package that isn't in
  the workspace
- **The whole test suite runs in CI**, not just the library tests: the .docx round-trip and
  hostile-input suites, the binaries' own tests and the integration tests. Running it here found
  a bug the old job hid — a paragraph whose text is only hashes was read back as an empty
  heading and lost its text
- **The repository's own gates run in CI**: every asset needs a row in ATTRIBUTION.md, the crate
  layering is enforced, and the browser build of every layer still compiles
- **Quill says Quill**: the release workflow, the web page's title, the app's and the command
  line tool's messages, the print title and the packaging output. The internal crate names and
  the build variables keep theirs, and `docs/releasing.md` says why
- **The 19 interface languages keep their translations**: the English text of about sixteen
  strings was renamed to Quill, but the catalogs still carried the old wording as their key, so
  those strings fell back to English in every language. The catalogs are re-keyed, so the German,
  French, Polish, Russian, Serbian, Turkish, Ukrainian, Bulgarian, Chinese, Czech, Spanish,
  Estonian, Hungarian and Italian interfaces are complete again
- **Files Quill writes say Quill**: the PDF, ODT, HTML, RTF and LaTeX exports, the .docx
  "Application" property and a new document's author carry the Quill name, and the MCP server
  introduces itself as Quill

## 1.1.0 — Nextcloud, Obsidian depth and Focus mode

A release about the places your writing lives: a Nextcloud server, an Obsidian vault, and the
page in front of you.

- **Nextcloud**: connect to a server (address, account and an app password), browse its folders
  in the Navigation pane, open a file from it, write the document back to it, make a public link
  and read the versions the server keeps. The server's own search finds files by their contents.
  Everything runs on a worker thread, so the editor never waits for the server, and a file that
  changed on the server in the meantime is not overwritten: Quill says so and asks for a new
  name. The app password is kept in a file only the user can read, never in the ordinary
  settings file, and the next launch reconnects on its own. A Markdown file on the server is an
  Obsidian note: frontmatter, wikilinks and embeds survive the round trip, and its images are
  written beside it. See [docs/cloud.md](docs/cloud.md)
- **Add an Obsidian vault from File › Open**: "Open Obsidian Vault…" asks for the folder and
  shows its notes in the Navigation pane beside the document, where they can be searched and
  opened
- **Paste equations**: a formula written in a note (`$x^2$`, `$$…$$`) can be pasted into the
  document as a real equation — the Paste Options button offers "Paste as Equation" after the
  paste, and the ribbon's Paste menu has the same entry. A price ("$5 and $10") is not a formula
  and is left alone
- **Obsidian, deeper**: the vault pane has three views now — the notes, the notes that link to
  the open one (backlinks), and the tags the notes carry. Tags come from the frontmatter's
  `tags:` list and from the `#tags` in the note's own text. Insert › Link writes a link to a
  note at the caret, the way Obsidian writes it. See [docs/obsidian.md](docs/obsidian.md)
- **Focus mode** (View › Immersive › Focus) now does what it says: the ribbon, the panes and
  the status bar step aside and the page fills the window. Escape brings the interface back.
- **AutoSave to the server**: a document that was opened from the server is written back to it
  a couple of seconds after the last change, as long as the server is connected
- **The app on this machine follows the checkout**: `cargo xtask install` builds the release app and puts it in `~/Applications/Quill.app` (the command line tool in `~/bin/quill`), and `cargo xtask install --hook` writes the git hooks that install it after every commit and every merge — push to GitHub and the local copy is already the new one. The build runs in the background and logs to `.git/quill-install.log`
- **The ritual**: `cargo xtask test` runs the tests and then removes what the run left behind
  (build directories older than a week, the incremental caches, the scratch folders a crashed
  test left and the half-written files a killed save left). `cargo xtask ci` ends with it, and
  `cargo xtask clean --all` sweeps everything, this checkout's build directory included
- The macOS build works again: the app icon was renamed to `quill-macos-512.png`, and the app
  still pointed at the old name
- 1,000+ commands in the registry, each with its own tests; the Word feature catalog is at
  489 of 504 (97.0 %)

## 1.0.2 — Hostile-input bounds, Paste Options and Obsidian vaults

A second hardening pass over binary documents, pictures, fonts and proofing tables, Word's Paste Options button after a paste, and an Obsidian vault workspace.

- **Paste Options**: a small button beside a fresh paste offers Keep Source Formatting, Merge Formatting, Keep Text Only and Paste Special…. Choosing a form undoes that paste and repeats it at the destination, so destination formatting and a single undo step are kept; cancelling Paste Special puts the original paste back. Hides when the caret moves, on another edit or document, and for pictures and equations
- **Obsidian vaults**: open a vault folder in the Navigation pane, search note titles and text, open and save notes, and export the document into the vault with its pictures in an attachments folder. Frontmatter, wikilinks, `![[embeds]]` and unsupported syntax survive a round trip: unknown blocks stay as their original source instead of being normalised, and an unchanged note saves back byte-for-byte. Vault and attachments folder are remembered
- **Binary .doc**: streams are capped at 1 GiB each and 1.5 GiB together; the piece table is counted against the bytes it can actually address, and text that would materialise more than 8 million characters is refused instead of expanding one small run hundreds of millions of times
- **Pictures**: dimensions are read from the header before decoding; more than 40 million pixels per image and 100 million across a PDF export are refused, and a page raster above 40 million pixels is scaled down uniformly instead of cropped; a picture whose declared frame is unreadably small (~1 pt extents some PDF→Word converters write) comes in at its own pixel size at 96 ppi instead of being invisible
- **Fonts**: collections and variable-font named instances are capped at 256 faces per file, and the collection count is checked before any face is enumerated
- **Zotero**: conversion calls cap and deduplicate field and placeholder ids before they scan the document
- **Proofing**: `TRY`/`REP` tables, affix rules and generated forms have explicit caps
- **Layout**: equation and display lookups use binary search, so paragraphs with many equations no longer scan every equation for every line
- Regression tests for repeated .doc pieces, picture headers, TTC face counts, affix floods and Zotero id floods

## 1.0.1 — Security hardening

Limits, validation and atomic writes for documents WordCraft did not create.

- **Maxima**: document-derived expressions are checked with lexical rules and a function allowlist instead of a token blocklist, and every call path — including the graph variable probe — is validated before a Maxima process starts; definitions, variables and interpolated parameters are validated, and unsafe saved definitions are skipped with a log line
- **Resource limits**: large batches are sent over Maxima's stdin (up to the 4 MiB cap) instead of the OS argument list, and on Unix Maxima runs with a 2 GiB address-space cap; the 30 s timeout and output caps are unchanged
- **Math coverage**: harmless school functions that were rejected by the allowlist (matrix algebra such as `submatrix`, `row`, `col`, `expt`) work again end-to-end, and identifier splitting no longer mangles names like `matrix_size` or `f1`
- **Raw command**: `math.maxima {"cmd"}` remains a deliberate escape hatch, now documented in `SECURITY.md` as unsandboxed and for trusted input only
- **DOCX/ODT/ZIP/XML**: explicit limits on elements, names, attributes and text per part (each Flat OPC part gets its own element budget); text limits count the escaped form, so anything the reader accepts can be saved and reopened; kept markup the writer cannot write back fails the save instead of being dropped silently; decompression is read through a bounded reader, and a part that inflates past its cap is refused instead of silently truncated; Word XML files over 256 MiB are rejected before they are read
- **Pictures**: an ODT picture entry that lies about its size, or pictures past the total cap, now fail the import with the entry named instead of being left out
- **Package names**: DOCX part names and relationship targets are validated on read and on write, so a hostile package cannot plant a name (backslash, drive prefix, `..` segment) that an extractor would read as a path
- **Files**: PNG/PDF exports write atomically through the same sibling-temp path as document saves and replace a symbolic link instead of following it; saves to FIFOs and devices write through instead of replacing them; picture, mail-merge and font reads are length-bounded
- **Control channel**: the loopback control server refuses connections past 64 simultaneous ones and no longer risks losing its accept loop
- **Data integrity**: a failed save rolls back the document's metadata and password; unmodelled DOCX inline XML survives a save; CI runs the engine integration tests
- Regression tests for Maxima bypasses, definition/variable validation, XML and zip limits, symlink-safe saves, CDATA round-trips and zip-slip part names

## 1.0.0 — WordMat tab (first release)

Danish school-math inside WordCraft, powered by Maxima (same engine WordMat uses).

- **WordMat ribbon tab**: Beregn, Løs, Definér, Enheder, LaTeX, Matrix, Statistik, Calculus, Formler, Graf, Mere — all Danish, with Alt+M / Alt+B / Alt+L shortcuts
- **Answers in green** inside the same equation (`= result`); re-running after edits replaces the old answer
- **Solvers**: exact, numeric, isolate-variable with variable picker, inequalities, systems, ODEs
- **Units** (Danish decimal commas), definitions (`:=`), matrices, limits, Taylor, statistics
- **Graphs**: function plots (up to 3 curves, gaps at poles) rendered to PNG in the document
- **Formula collection** plus personal saved equations; LaTeX export
- **Equation highlighting** across structure boundaries, copy-paste keeps equations as equations
- Maxima runs bounded (30 s timeout); remote-code patterns in pasted equations are rejected

Requires Maxima installed separately (`sudo pacman -S maxima` on Arch/CachyOS).
Optional SolveReal (`solvereal.mac`, GPL, user-installed, never bundled) for real-domain solving.

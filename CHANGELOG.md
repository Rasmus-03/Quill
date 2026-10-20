# Changelog

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

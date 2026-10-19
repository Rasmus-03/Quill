# Security model

WordCraft opens and calculates on documents it did not create. This file records the
boundaries that are enforced in code and the ones that are deliberately left to the
operator's environment.

## Documents are untrusted input

DOCX, DOCM, DOTX, ODT, Flat OPC and WordprocessingML 2003 files are parsed with bounded
zip, XML and encryption readers:

- DOCX packages: at most 20,000 zip entries, 256 MiB per decompressed part and 512 MiB
  in total; ODT and spreadsheet parts have their own per-entry and total caps;
- a 256 MiB cap on a Word XML file, checked before the whole file is read;
- at most 256 levels of XML nesting and 4,000,000 elements per part;
- element/attribute-name length, attribute-count, attribute-size and total-text limits;
- decompression is read through a bounded reader, so a lying zip size cannot expand
  without bound; a part that inflates past its cap is refused, not silently truncated.

A malformed or hostile document must produce an error, never execute code and never
panic the reader. Math expressions that come from a document selection are checked
against a lexical rule set and an allowlist of Maxima functions before they reach
Maxima. Unknown functions, strings, assignment, statements, Lisp escapes, comments and
file/process functions are rejected.

## Deliberate escape hatches

The following inputs are trusted local/operator input, not document input:

- `math.maxima` with a `{"cmd": "..."}` parameter runs raw Maxima. Maxima can call the
  operating system (`system`, `load`, file output, Lisp interop), so this executes with
  the same privileges as WordCraft. Do not pass untrusted or model-generated text to
  this parameter. There is no in-process sandbox around Maxima.
- `MATHCRAFT_DEFS` and the persisted math-definitions list are local configuration.
  Definitions are revalidated and unsafe entries are logged and skipped, but a valid
  user definition is still Maxima code the user chose to install.
- `MATHCRAFT_SOLVEREAL` names a local Maxima package. Quotes and control characters are
  rejected before it is interpolated into `load("...")`, but the file itself is trusted
  local code.
- A local `maxima-init.mac` and other Maxima packages are loaded by Maxima itself.

If WordCraft must run against hostile documents and hostile automation input, put the
whole process in an OS sandbox or container with no access to secrets or the rest of the
filesystem. The expression allowlist reduces the attack surface; it is not a substitute
for process isolation.

## Files and saves

- Document saves and exported PNG/PDF files are written to a sibling temporary file and
  atomically renamed into place. A failed preparation leaves the previous file intact.
- The destination is replaced, not followed: if it is a symbolic link, the link is
  replaced and its target is left untouched. This prevents a link swapped after the
  automation write-root check from redirecting the save.
- Automation read/write roots are resolved with `..` and symlinks before access. A
  check-then-open race remains possible for a local attacker who can write the
  directory between the check and the open; that is the documented automation
  confinement boundary. Use filesystem permissions or an OS sandbox against that
  attacker.
- A failed save does not clear the document's dirty flag. Save errors are returned to
  the caller and are not silently ignored.

## Reporting

Please report security issues privately to the repository maintainers rather than in a
public issue. Include the WordCraft version, the platform, and a minimal reproducer
(document, command invocation, or automation request).

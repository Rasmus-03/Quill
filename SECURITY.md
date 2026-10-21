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
- at most 256 levels of XML nesting and 4,000,000 elements per part (each Flat OPC part
  gets its own element budget);
- element/attribute-name length, attribute-count, attribute-size and total-text limits,
  counted in the escaped form the writer emits so that a part the reader accepts can
  always be saved and read back;
- decompression is read through a bounded reader, so a lying zip size cannot expand
  without bound; a part that inflates past its cap is refused, not silently truncated;
- a picture entry that lies about its size, or a document whose pictures pass the total
  cap, fails the import with the entry named — a picture is never silently left out;
- kept markup that the writer cannot write back fails the save instead of being dropped
  silently;
- part names and relationship targets are validated on the way in and again on the way
  out: a name that is not package-relative, or that contains `\`, a drive prefix (`C:`),
  a control character or a `.`/`..` segment, is skipped on read and refused on write, so
  a hostile package cannot plant a name that an extractor would read as a path;
- binary `.doc` streams are capped at 1 GiB each and 1.5 GiB together. The piece table is
  checked against the bytes it can actually address, and text that would materialise more
  than 8 million characters is refused, so a small table cannot alias one run hundreds of
  millions of times;
- embedded pictures are measured from their headers before decoding: at most 40 million
  pixels per picture, 100 million per PDF export, and 40 million for the on-screen cache.
  Page rasters are capped at 40 million pixels, shrinking uniformly rather than distorting
  the page;
- font collections may add at most 256 faces per file, and proofing dictionaries have
  caps on `TRY`/`REP` tables, affix rules and generated forms;
- Zotero conversion calls cap and deduplicate field and placeholder ids before scanning
  the document.

A malformed or hostile document must produce an error, never execute code and never
panic the reader. Math expressions that come from a document selection are checked
against a lexical rule set and an allowlist of Maxima functions before they reach
Maxima. Validation runs before any Maxima process is started, including the variable
probe used by Plot graph (`listofvars`): the graph spec is checked before it is
interpolated into the call, and `pick_var` independently refuses an unvalidated probe.
The function allowlist is reviewed for purity before a name is added. Unknown functions,
strings, assignment, statements, Lisp escapes, comments and file/process functions are
rejected.

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

## Control channel

The loopback control server requires the window's key on every request and accepts at
most 64 simultaneous connections; connections past the limit receive an error line and
are closed, so a local client cannot make the app spawn threads without bound. The key
file is written mode 0600 and removed when the window closes.

## Maxima process limits

Maxima runs as a separate, short-lived process. Each call is killed after 30 seconds and
its stdout and stderr are capped at 8 MiB and 1 MiB. On Unix it is started through
`sh -c 'ulimit -v <KiB> 2>/dev/null || true; exec "$0" "$@"'` with a 2 GiB address-space
cap; the Maxima binary and its arguments are passed as argv, never interpolated into the
shell script. If `ulimit -v` is unavailable the call still runs. Windows has no
address-space cap. These limits contain accidental or hostile resource use — a CPU-bound
expression can still occupy Maxima until the 30 s timeout — but they are not a sandbox.

Batches up to 8 KiB are passed with `--batch-string`; larger batches (up to the 4 MiB
hard cap) are written to Maxima's stdin, so the cap is enforced by WordCraft rather than
by the operating system's argument-length limit.

## Files and saves

- Document saves and exported PNG/PDF files are written to a sibling temporary file and
  atomically renamed into place. A failed preparation leaves the previous file intact.
- The destination is replaced, not followed: if it is a symbolic link, the link is
  replaced and its target is left untouched. This prevents a link swapped after the
  automation write-root check from redirecting the save.
- A save to an existing destination that is neither a regular file nor a symbolic link
  (a FIFO, a device, a socket) is written through in place after confirming the opened
  object is still that special file; only regular files, symlinks and new paths get the
  temporary-file-and-rename replacement.
- Automation read/write roots are resolved with `..` and symlinks before access. A
  check-then-open race remains possible for a local attacker who can write the
  directory between the check and the open; that is the documented automation
  confinement boundary. Use filesystem permissions or an OS sandbox against that
  attacker.
- A failed save does not clear the document's dirty flag. Save errors are returned to
  the caller and are not silently ignored.

## The cloud (Nextcloud)

- The app password is a secret. It is kept in `nextcloud.json` in the settings folder, written
  with the user's rights only (`0600` on Unix), and it is never written to the ordinary
  settings file, never written to a log, and never sent anywhere but the server. Use a
  Nextcloud **app password** (Settings › Security › Devices), which can be revoked from the
  server at any time and grants access to the files only.
- The password is held in memory only for as long as the account is connected, in a
  zeroizing buffer that is wiped when it is dropped.
- TLS is verified against the public roots by default. "Don't check the server's certificate"
  turns the check off (for a self-signed server); with it on, anything on the way to the
  server can read the traffic, including the password.
- Redirects are followed at most five times, and the authorization header is not sent to
  another host.
- Every request has a timeout, every answer a size limit, and a remote path may not leave the
  account (`..` is refused).
- A file that changed on the server after it was read is not overwritten: the server's tag for
  the version that was read is sent back with the write, and the server refuses the write when
  it no longer matches.

## Reporting

Please report security issues privately to the repository maintainers rather than in a
public issue. Include the WordCraft version, the platform, and a minimal reproducer
(document, command invocation, or automation request).

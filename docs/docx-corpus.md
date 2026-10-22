# The .docx corpus

The corpus is a set of **real** .docx files: documents that Word, Google Docs, LibreOffice and
Pages actually wrote. The synthetic fixtures in `crates/docx/tests/` prove that the reader and the
writer agree with each other; only these files prove that they agree with **Word**.

It answers the question the work list has carried since 2026-10-06 ([`docs/gaps.md`](gaps.md),
gap #1): *DOCX never tested on real-world files*.

## Where it lives, and why it is not in the repository

`corpus/docx/`, which is git-ignored. Every file in it is somebody else's work, and the
clean-room rule ([`AGENTS.md`](../AGENTS.md)) keeps files that Word produced out of this
repository. So the corpus is fetched, never committed:

```sh
cargo xtask corpus --docx          # fetch it (pinned by SHA-256)
cargo test -p wordcraft-docx --test corpus    # check every file
```

The test skips cleanly when there is no corpus, so a machine without one is unaffected. Point
`QUILL_DOCX_CORPUS` at a corpus somewhere else to use another one.

## Where the files come from

| Source | Licence | Files |
|---|---|---|
| [Apache POI](https://github.com/apache/poi) `test-data/document/` | Apache-2.0 | 71 |

The files are POI's test documents: real files from Word and its cousins, chosen because they
cover the features a document really has. Every file's name, source URL and SHA-256 is in
[`crates/docx/tests/corpus-manifest.tsv`](../crates/docx/tests/corpus-manifest.tsv), which is
also what the fetcher and the test read. A file whose hash does not match the manifest is
refused, so an upstream change cannot slip in unnoticed.

**Adding a file:** put it in the manifest with its name, its source, its hash and what the reader
is expected to do, then fetch and test. A file from another project has to come with a licence
that allows redistribution (Apache-2.0, MIT, BSD, CC0, or the public domain) and its licence
has to be noted here. A file that Word produced may be *tested* but never *committed*.

## What the corpus covers

71 files, 1.5 MB. What they contain, counted from the files themselves:

| Feature | Files |
|---|---|
| Footnotes and endnotes | 25 |
| Bookmarks | 24 |
| Headers | 21 |
| Tables | 19 |
| Footers | 19 |
| Pictures | 10 |
| Numbering (lists) | 9 |
| Hyperlinks | 8 |
| Drawings and shapes | 6 |
| Content controls | 6 |
| Fields | 5 |
| Right-to-left text | 4 |
| Comments | 2 |
| Charts | 1 |
| Tracked revisions | 1 |

## What the check does

For every file: **read → write → read**. The second read has to hold the same text, the same
tables (their rows, their cells and their text), the same parts (headers, footers, notes,
comments) and the same pictures. Writing what was read has to give the same document again, so a
reader that quietly loses something shows up as a document that keeps changing.

Two files are **refused on purpose**, and the manifest says so:

| File | Why |
|---|---|
| `deep-table-cell.docx` | 5,000 nested tables (15,005 levels of XML) — deeper than the reader's 256-level limit, which protects it from a stack overflow |
| `truncated62886.docx` | the package was cut in half, so it is not a valid zip |

A refusal that the manifest does not list fails the test, and a listed refusal that now opens
fails too: the list has to stay true.

## What it found

The corpus is the sharpest tool in the repository for the DOCX reader, and it has already paid
for itself: the first run over these 71 files showed the writer renumbering the header and
footer parts and renaming the pictures (`image1.emf` → `image1_2.emf`). Nothing was lost, but
the check compared the numbers and the names rather than the content, so a real difference would
have been hidden among them. The check now compares what the document *is* — the text, the
tables, the parts and the pictures' bytes — and the part numbers and picture names are left to
the writer.

The full state of the format — what is read, what is written and what is lost on the way — is in
[`docs/file-format-parity.md`](file-format-parity.md).

# The .docx corpus

The corpus is a set of **real** .docx files: documents somebody else's software actually wrote.
71 of them, from Apache POI's test data — 59 carry "Microsoft Word" or "Outlook" as their
producer, 7 carry LibreOffice, and 5 name none (a tool that never filled `docProps/app.xml` in).
None come from Google Docs or Pages yet. The synthetic fixtures in `crates/docx/tests/` prove that
the reader and the writer agree with each other; only these files prove that they agree with
**Word**.

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

CI has a job of its own for it (`.github/workflows/ci.yml`, `corpus`): it runs
`cargo xtask corpus --docx` and then the test, so a push is checked against the real files and not
only against the synthetic fixtures. A file that is missing — because a fetch refused it after an
upstream change — fails the test, so the job cannot pass by checking less than the manifest says.

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

For every file: **read → write → read**. The second read has to hold the same document — not only
the same words:

| Compared | What that covers |
|---|---|
| The text, paragraph by paragraph | Order, characters |
| The tables | Rows, cells, cell text |
| The parts | The text of every header, footer, note and comment |
| The pictures | Their bytes, not their names (a save may name a picture differently) |
| **Every paragraph's runs and properties** | Bold, italic, colour, font, size, highlighting, hyperlinks, field codes, alignment, indents, spacing, numbering, borders, shading, content controls, the section a paragraph ends |
| **The inline objects** | A picture's size, crop, wrap, alt text and position; a shape, a group, a text box's content; a bookmark; an equation |
| **The sections** | Page size, orientation, margins, gutter, columns, header and footer distances, title page, page numbering, line numbers, page borders, the document grid, note options |
| **The numbering** | Every list definition, its levels, formats, starts and overrides |
| **The styles** | The style sheet as written, name by name |
| **The fields, comments and tracked changes** | Field codes and their results, who wrote a comment and when, insertions, deletions, moves and formatting revisions |
| **The settings and properties** | Track changes, hyphenation, autoformat, the document's own properties and bibliography sources |

Writing what was read has to give the same document again — the same comparison, twice — so a
reader that quietly loses something shows up as a document that keeps changing.

The comparison is the document as JSON, with three things relaxed, because the save invents them
and a reader cannot tell one from the other: a float rounded to a hundredth of a point (a save
writes twips, so `273.59998` comes back as `273.6`), a relationship id (`rId4` and `rId1` name the
same picture when the target is the same), and a part number (a save numbers the parts in the
order it writes them, so parts are compared by their content). Zero-width markers — a bookmark,
the end of a content control — may come back in another order at the same place; anything with a
size keeps its place.

Run it with `QUILL_CORPUS_DIAG=1` and a failing file says where the two documents part company,
as a path: `body.4.Para.runs.0.props.bold`.

**What the wider check found.** Two files (`60329.docx`, `form_footnotes.docx`) have headers and
footers that are only a table, and every save added an empty paragraph after it — a line of
height, so a header grew a little with each save. Word's own file ends with the table, so the
writer no longer adds a paragraph outside the body and the table cells, where Word does need one.

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

## Do the pages come out right?

The corpus can answer that too, without staging a comparison: Word writes the page count it
computed into every file it saves (`docProps/app.xml`, `<Pages>`). That is the reference
implementation's own answer for that document, recorded before Quill existed.

Measured 2026-10-23: 65 of the 71 files carry such a count, and **62 of them come out of Quill at
one page per page of Word's**. Three are one page apart — `60329.docx` (3 → 4), `drawing.docx`
(20 → 21) and `form_footnotes.docx` (3 → 2). The six without a count are three files written by
tools that never paginated, the empty one, and the two refusals.

A matching total is not a matching page: lines can break elsewhere, a picture can sit a
centimetre lower, a heading can move to the next page without changing the count. And `<Pages>` is
what Word computed at its last save, on that machine, with those fonts. So the number says the
layout is close to Word, not that the pages look the same — the page-by-page render comparison is
still the open item. What each difference is, and what the check does not cover, is in
[`file-format-parity.md`](file-format-parity.md#pagination-against-word).

### The count is an assertion now

```sh
QUILL_CORPUS_PAGES=1 cargo test -p wordcraft-docx --test corpus
```

The harness reads `<Pages>` from each file, lays the document out the way the app does, and fails
naming every file whose count moved, with both numbers and the delta. The gate is opt-in for two
reasons: the count depends on the fonts installed on the machine, and three files below are still
one page apart, so the gated run is red until they are put right. Without `QUILL_CORPUS_PAGES=1`
the test behaves exactly as before.

### The three files that are one page apart

Measured 2026-11-03 on this line, the corpus fetched by hash (`cargo xtask corpus --docx`, 71
files), each page read from Quill's own print layout — the page it lands on, its height and the
room left around it:

| File | Word | Quill | Delta |
|---|---|---|---|
| `60329.docx` | 3 | 4 | +1 |
| `drawing.docx` | 20 | 21 | +1 |
| `form_footnotes.docx` | 3 | 2 | −1 |

**`60329.docx`** — a government protocol: one 5×2 table, four of whose cells hold nested tables,
on a landscape page with 0.1″ margins, under a header (“Study Design, Objectives and Endpoints”)
and a four-row footer of its own. The body area is 459.9 pt of the 612 pt page: the header takes
34.2 pt and the footer 110.8 pt (`w:header`/`w:footer` are 0, so both sit at the page edge). Quill
fills pages 1–3 to the body bottom (content ends at 501.2, 494.7 and 501.2 against 501.2). Page 4
holds the last 13 lines of the paragraph at `[0,4,0,0,2,1,0]` (lines 34–46, 138 pt) and the empty
paragraph that closes the document: 165.8 pt of content and 294 pt of blank page. Nothing is
teased apart by a break rule — the file has no `keepNext`, no `pageBreakBefore` and no `cantSplit`,
and zeroing every paragraph's space after does not change the count. Quill's content is 1538.7 pt
against the 1379.6 pt three pages hold, so the nested tables are 159 pt (11.5%) taller here than
in Word's three pages; the lines at the boundary are the 46-line cell that starts on page 3. The
file's own footer says “Page 1 of 3” — its `NUMPAGES` field agrees with `<Pages>` — while Quill's
render of page 4 says “Page 4 of 4”.

**`drawing.docx`** — 8,800 words of Russian media monitoring, 189 paragraphs of `Normal (Web)` text
with HTML auto spacing, and five small tables. The extra page is the last three paragraphs of the
“Управление на французский манер” piece (187–189, 207.4 pt); page 20 ends 19.0 pt above the body
bottom (766.2 of 785.2), so paragraph 187 — its 14 pt automatic space before plus a first line of
11.5 pt — could not fit. No `keepNext`, `pageBreakBefore` or `cantSplit` exists in the file. The
space Quill loses is on page 8: the table at body block 80 ends its second row at 586.5 with 198.7
pt left, and its third row is 199.7 pt tall — one point too tall — so the row moves whole to page 9,
because only the cell's bottom edge crosses the cut and no line inside the row does. That one row's
cell holds a nested table with a picture. Removing the space after the body paragraphs (the
`w:beforeAutospacing`/`w:afterAutospacing` pairs that resolve to 14 pt before and after,
`AUTO_SPACING` in `crates/doc/src/resolve.rs`) makes the file paginate to exactly Word's 20 pages,
so the page is the accumulated auto spacing and the substituted Arial/Verdana metrics, not one
discrete break.

**`form_footnotes.docx`** — an American consular visa form: six tables of fixed-height rows, one
one-line footnote (“testdoc test phrase”) and 215 line boxes. Quill ends page 2 with the
INSTRUCTIONS table (block 16) and the empty paragraph that closes the document, at 671.9–678.8,
58.5 pt above the body bottom (737.3). Total content is 1344.1 pt against the 1402.6 pt two pages
hold — 96% full — so Word's third page needs at least 59 pt more content than Quill lays out. The
file has no page break, no `keepNext` and no `pageBreakBefore`; its only break rules are four small
`cantSplit` rows (18.9, 16.7, 11.5 and 11.5 pt) and a one-line note, none of them at a page
boundary. Either Word's layout is about 4% taller (a substituted Times New Roman, 0.3 pt per line
over 215 line boxes, is the plausible metric) or `<Pages>` was written at a save of a slightly
different document.

**What the diagnosis can and cannot say.** Every number above was measured: Where Quill's pages
end, how much room each had, what the row heights are, and which experiment moves the count. Word
is observed only through the totals it wrote into its files. Its page breaks are not in the file
and no Word render was compared, so for `60329.docx` and `form_footnotes.docx` the difference is
localised to a page, a row and an amount, not traced line by line; the page-by-page comparison
remains gap #1's open item. Fixing any of the three moves the gated count, which is what the gate
is for.

The full state of the format — what is read, what is written and what is lost on the way — is in
[`docs/file-format-parity.md`](file-format-parity.md).

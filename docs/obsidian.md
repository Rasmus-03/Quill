# Obsidian vaults in Quill

A vault is a folder of Markdown notes. Quill opens one in the Navigation pane, searches it,
opens and saves the notes, and — since 1.1 — knows which notes link to which and which tags
the notes carry.

- [Opening a vault](#opening-a-vault)
- [The vault pane](#the-vault-pane)
- [Notes](#notes)
- [Backlinks](#backlinks)
- [Tags](#tags)
- [Links between notes](#links-between-notes)
- [Saving a document into the vault](#saving-a-document-into-the-vault)
- [Commands](#commands)
- [What Quill does not do](#what-quill-does-not-do)

## Opening a vault

**Open Vault…** in the Navigation pane's **Vault** tab asks for a folder (the desktop app's
folder picker). The folder is remembered, and the notes in it are listed.

A vault may also be a folder on a Nextcloud server: the server's folders are ordinary folders
once the desktop client has synchronised them, and Quill reads them as a vault.

## The vault pane

The pane has three views:

| View | What it shows |
|---|---|
| **Notes** | The notes themselves: the search box finds titles and text, a click opens a note. |
| **Backlinks** | The notes that link to the open note. |
| **Tags** | The tags the notes carry, and the notes that carry them. |

**Save to Vault** at the bottom of the Notes view writes the document on screen into the vault
as a Markdown note.

## Notes

A note is read as an Obsidian note:

- the YAML frontmatter is kept out of the text and put back on save;
- `[[wikilinks]]` and `![[embeds]]` are links, not plain text;
- blocks Quill cannot represent (callouts, comments, highlights, math, block references, task
  boxes, fenced blocks with a language, Dataview) are kept as they were written, instead of
  being rewritten into something else;
- a note that was not edited saves back **byte for byte**, with its BOM and its line endings.

## Backlinks

The **Backlinks** view lists the notes that link to the open note. Open a note first; the
view then names the notes that point at it.

A link counts when its target is the note's name (`[[Note]]`) or its path (`[[Folder/Note]]`).
The heading and the alias of a link (`[[Note#Heading|Alias]]`) are not part of the target.

## Tags

The **Tags** view lists the tags the vault's notes carry, with the number of notes that carry
each. A click shows the notes with that tag, and a click on a note opens it.

Tags come from two places:

- the frontmatter's `tags:` list, written as `tags: [one, two]` or as a list of `- one` lines;
- the `#tag` written in the note's own text.

A heading (`# One`) is not a tag, and neither is the `#` of a word like `C#`.

## Links between notes

**Insert Note Link** (Insert › Link) writes a link to a note at the caret, the way Obsidian
writes it: `[[Note]]`, or `[[Note|the text shown]]`.

## Saving a document into the vault

**Save to Vault** writes the document on screen as a note. The name is typed above the button
(`Note.md` by default). Its images are written into the `attachments` folder next to the note
and the note points at them there; the folder's name is a setting.

## Commands

| Command | What it does |
|---|---|
| `vault.open` | Open a vault folder, or a note inside it. |
| `vault.scan` | Read the note list again. |
| `vault.search` | Find notes by title and text (`q`, `limit?`). |
| `vault.export` | Save the document into the vault (`path?`, `attachments?`). |
| `vault.index` | The links between the notes and their tags. |
| `vault.backlinks` | The notes that link to a note (`note?`). |
| `vault.tags` | The tags the notes carry. |
| `vault.insertLink` | Write a link to a note at the caret (`target`, `label?`). |

## What Quill does not do

A vault is a folder of Markdown files, and Quill treats it as one. It does not run Obsidian's
plugins, does not read its workspace layout, does not draw its graph, and does not write
Obsidian's `.obsidian` folder. The Notes, Backlinks and Tags views are Quill's own reading of
the files.

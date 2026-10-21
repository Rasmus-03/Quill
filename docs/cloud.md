# Nextcloud in Quill

Quill talks to a Nextcloud server the way a desktop client does: the files on the server are
opened and saved like local ones, notes are edited as Obsidian notes, and the server's own
search, share links and file versions are used.

- [Connecting](#connecting)
- [The Nextcloud pane](#the-nextcloud-pane)
- [Saving](#saving)
- [Notes on the server](#notes-on-the-server)
- [Conflicts](#conflicts)
- [Commands](#commands)
- [The app password](#the-app-password)
- [Limits](#limits)

## Connecting

Open the Navigation pane (View › Navigation Pane) and choose the **Nextcloud** tab. Fill in:

| Field | What it is |
|---|---|
| Server | The server's address, e.g. `https://cloud.example.com`. A subfolder (`https://example.com/nextcloud`) works. A missing scheme means HTTPS. |
| User name | The account's name on the server. |
| Password | An **app password**, not the account's own password. |

Then **Connect**. The server is asked what it is and which account is signed in; the pane shows
the product and the account when the answer comes.

The address, the user name and the app password are kept in `nextcloud.json` in the settings
folder (macOS: `~/Library/Application Support/Quill`, Windows: `%APPDATA%\Quill`, else
`$XDG_CONFIG_HOME/quill`). That file is written with the user's rights only (`0600`), and it is
the only place the password is written: the ordinary settings file never holds it. The next
launch reconnects on its own.

**Do not check the server's certificate** is for a server with a certificate the computer does
not know (a self-signed one). It turns the check off, so anything on the way to the server can
read the traffic; leave it off unless you know why you need it.

## The Nextcloud pane

The pane is the Navigation pane's **Nextcloud** tab (File › Open › Nextcloud goes there too).

- **Refresh** asks the server for the folder's contents again.
- **Up** goes to the folder above.
- A folder in the list opens it; a file opens it in the editor.
- **Search** asks the server to search its own files (the search app must be installed on the
  server; the hits open like files).
- **Save to Nextcloud** writes this document into the folder the pane shows.
- **New folder** makes a folder there.
- **Share** makes a public link to the open document; the link is shown under the button.
- **Versions** lists the older versions the server keeps for the open document.

Nothing in the pane waits for the server: the request goes to a worker thread and the answer
arrives a moment later, so the editor never freezes.

## Saving

A document that was opened from the server is written back to the server by **Save** (File ›
Save). **Save to Nextcloud** in the pane writes the document into the folder the pane shows,
with the name typed above the button.

The document's format follows its name: `report.docx` is a Word document, `notes.md` is a
Markdown note (see below). AutoSave works the same way: a document open from the server is
written back to the server a couple of seconds after the last change, as long as the server is
connected.

## Notes on the server

A `.md` file on the server is read and written as an **Obsidian note**: its frontmatter, its
`[[wikilinks]]` and its `![[embeds]]` survive the round trip, and blocks Quill cannot represent
are kept as they were written instead of being rewritten. A note that was not edited saves back
byte for byte.

Saving a note writes its images into an `attachments` folder next to it, and the note points at
them there.

## Conflicts

When a document is opened, the server's tag for that version of the file is remembered. Saving
sends it along, and the server refuses the write when the file has changed in the meantime
instead of overwriting someone else's work. Quill then says that the file changed on the server:
open it again, or save a copy under another name.

## Commands

Everything the pane does is a command, so scripts and agents can do it too:

| Command | What it does |
|---|---|
| `cloud.connect` | Connect to a server (`server`, `user`, `password`, `insecure?`). |
| `cloud.disconnect` | Forget the account. |
| `cloud.status` | What is known about the account and the server. |
| `cloud.list` | List a folder (`path`). |
| `cloud.open` | Open a file from the server (`path`). |
| `cloud.save` | Write the document to the server (`path?`). |
| `cloud.mkdir` | Make a folder (`path`). |
| `cloud.delete` | Remove a file or folder (`path`). |
| `cloud.share` | Make a public link (`path?`). |
| `cloud.search` | Search the server's files (`term`). |
| `cloud.versions` | List a file's older versions (`path?`). |
| `cloud.wait` | Wait for the server's answers (`ms?`). |

The commands answer at once: the request is sent, and the answer lands in the session a moment
later. `cloud.wait` waits for it (scripts and the CLI use it), and `cloud.status` reports what
came in.

## The app password

Use an app password, not the account's own password:

1. On the server: **Settings › Security › Devices** (or **Personal settings › Security**).
2. **New app password**, with a name that says what it is for ("Quill on this laptop").
3. Copy the password it shows — it is shown once — into Quill's password field.

An app password can be revoked from the same page at any time, and it gives access to the
files only, not to the account's settings. The password is kept in a file only the user can
read, is never written to the ordinary settings file, and is never sent anywhere but the
server.

## Limits

The client is careful with what a server may send:

- every answer has a size limit (16 MiB for a listing, 512 MiB for a file);
- every request has a timeout (60 s, 15 s to connect);
- redirects are followed at most five times, and the password is not sent to another host;
- paths may not leave the account (`..` is refused), and a path may not be endless;
- a listing holds at most 50 000 entries.

The browser build has no sockets of its own: the cloud commands answer "not available here".

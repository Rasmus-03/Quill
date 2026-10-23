# The review loop

Work is not finished when the agent that did it thinks it is. It is finished when a second agent
has read it and either sent it on or sent it back with the changes it needs. This document is that
hand-over: the queue, the two sides of it, the commands, and what a reviewer actually checks.

It is the same for every agent that works in this repository — opencode, codex, claude, freebuff —
because the queue is a directory in the repository's git common directory, not a feature of any one
harness. `AGENTS.md` carries the ritual; this page carries the detail.

## The queue

`$(git rev-parse --git-common-dir)/wordcraft-review/` — for a normal checkout, `.git/wordcraft-review/`.

Why there:

- **every worktree sees it.** All the worktrees of this checkout share one `.git`, so an agent
  working in `../1.2-danish-ui` and the reviewer working in `../quill-1.2-review` are looking at the
  same queue, and a request made before a rebase is still there afterwards;
- **it is never committed.** It holds a copy of the diff, the notes and the verdicts; none of that
  belongs in the history. Nothing needs ignoring either, because `.git/` is already out of the tree.

One request is one JSON file, `rev-0007.json`. Ids rise in the order requests arrive. Nothing is
deleted: a settled request is the record of what was asked, what came back and who said so.

## The author's side

When you believe the work is done:

```sh
cargo xtask review submit --title "Home: Table, Pictures and Word Count" \
  --note "What I changed, what I checked, and what I am unsure about." \
  --gate "cargo test --workspace: pass" --gate "cargo xtask ci: pass"
```

`submit` captures what the reviewer needs, so nobody has to reconstruct it later:

| Captured | Why |
|---|---|
| the commit range (`base..HEAD`), the commits, the branch, the worktree | where the work is |
| uncommitted changes, and untracked files by name | work in progress is reviewable too; the diff is against the base, so the working tree is in it |
| the diff itself, and a diffstat | the reviewer reads the change without a checkout |
| the gates you ran, in your words | the reviewer knows what to re-run, and what a green run was claimed for |
| your terminal handle (`ORCA_TERMINAL_HANDLE`) | the verdict is delivered to you, not left for you to find |

The base is chosen for you: the head of the last **accepted** review on this branch if there is one
(the delta since then), else the commit the branch was created from, else the merge base with the
default branch. `--base REF` overrides it — `--base HEAD` for "only the uncommitted work",
`--base HEAD~1` for "only the last commit".

Then wait, in the same breath:

```sh
cargo xtask review wait          # blocks until the verdict, 10 minutes by default
```

The verdict is one of two things:

- **accepted** — the work was sent further (`--to integration` by default). Nothing more is needed
  from you on this request.
- **changes requested** — the reviewer's message says, file by file, what has to change. Fix it,
  and submit again with the same title. The new request is the answer to the old one; do not argue
  with the reviewer in the queue, and do not resubmit without changing anything.

If `wait` times out, the request is still pending — that is a checkpoint, not a failure. Carry on
with the next piece of work and check again with `cargo xtask review status`.

## The reviewer's side

```sh
cargo xtask review next                    # the oldest pending request, as a brief
cargo xtask review show rev-0007 --patch   # the whole request, with the diff
```

Read the work, then:

```sh
cargo xtask review accept rev-0007 --note "What I checked, and where I looked."
cargo xtask review accept rev-0007 --to term_abc123 --note "Yours: it needs the release notes."
cargo xtask review changes rev-0007 --note "crates/docx/src/write/story.rs:94 — …"
```

`accept` sends the work **further**: to the integration line (the coordinator merges it), or to
another agent's terminal (`--to term_…`), which is notified directly. `changes` sends it **back** to
the author, and `--note` is required: the author gets only that message, so it has to carry the
whole review.

### What to check

Not the author's summary — the diff. In this repository the recurring failures are all here, so
walk the list rather than trusting the note:

1. **The change matches the request.** The title and the note describe something; the diff is the
   thing. A hand-off that claims more than the code does is a `changes`, not an `accept`.
2. **The gates, run by you.** `cargo xtask ci` (fmt, clippy `-D warnings`, tests, assets, layers,
   wasm) — or the parts it is worth re-running for this change. A declared gate is a claim, not a
   result.
3. **Never crash.** No `unwrap`/`expect`/`panic!` in non-test code, no `unsafe`, input-derived
   numbers clamped and checked, recursion and allocations bounded. A new command is reached by
   `hostile_params_never_panic`; a crash fix has its regression test.
4. **Assets.** Nothing from Word, Office, Adobe, Avid or Autodesk — not an icon, not a colour, not
   a template — and every new asset has its row in `ATTRIBUTION.md`.
5. **Clean room.** Formats come from the published specs; no GPL/LGPL/AGPL code; nothing copied out
   of a proprietary bundle.
6. **The architecture holds.** `cargo xtask layers` is the mechanical half; the other half is that a
   behaviour is a `CommandSpec`, not a special case in the UI.
7. **Tests that would fail.** The test proves the behaviour the note claims, and the author has seen
   it fail without the fix. Green alone proves nothing.
8. **The docs the project asks for.** A behaviour change moves its gap, its parity doc and the
   progress log; a user-visible change appears in `CHANGELOG.md`; a typing change lands with its
   `docs/typing-parity.md` row and its test in the same commit.
9. **Nothing left behind.** `git status` shows only the work; no scratch files, no build output, no
   corpus file added to the tree.

### Writing a `changes` message

One message, every finding, because it is the author's only copy:

```
xtask/src/review.rs:120 — `--note` is joined but never trimmed, so a message of spaces passes the
  "required" check. Trim before the check, and add the case to the tests.
docs/review-loop.md — the loop is described as blocking for the default 600 s, but the code allows
  a timeout of 0 to mean "check once"; say which it is.
```

Say the file and line, what is wrong, and what done looks like. "Please fix" is not a review;
neither is a list of preferences without a reason. If it is a judgement call you would accept
either way, accept the work and write it in the note.

## Using the queue next to Orca

Nothing here needs Orca, but it uses it when it is there: `submit` notifies the configured reviewer,
and a verdict notifies the author's terminal. Set the reviewer once per machine:

```sh
cargo xtask review reviewer --terminal term_abc123 --name codex
cargo xtask review reviewer            # who it is now
```

If Orca is not running, the queue still works: the request is written, the verdict is recorded, and
`cargo xtask review status` is where you read it. The queue is the record; the notification is a
knock on the door.

## The rule

A piece of work is merged or called done when its **latest revision** has an **accepted** verdict
covering the commit that lands. A verdict covers one range: work added afterwards needs its own
request. `cargo xtask review list` is the answer to "what is waiting", and a queue with a pending
request in it is a queue somebody has not read yet.

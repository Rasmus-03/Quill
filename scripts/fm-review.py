#!/usr/bin/env python3
"""Ask Apple's on-device model to critique a `cargo xtask review` request.

The on-device model (`fm serve`) is small: ~3 billion parameters and a 4096-token
window covering the instruction, the prompt and the answer together. It is good at
reading text you hand it and saying something useful about it, and it is not a
coding agent, so this script does what the model is actually for — a second reader
with no stake in the work — and nothing else. It never edits anything.

    cargo xtask review show <id> --patch   # what this feeds on
    fm serve --port 8765                   # in another terminal, after `sudo fm license`
    scripts/fm-review.py <id>

Without --url it assumes the default `fm serve` address. The critique is printed,
one block per chunk of the diff, in the order the diff was written.

Exit codes: 0 critique printed, 69 the model is not licensed or not ready (the
`fm` CLI's own code for both, passed through by the server), 2 bad usage.
"""

import argparse
import json
import subprocess
import sys
import urllib.error
import urllib.request

# The model's window is 4096 tokens; leave room for the instruction and the answer.
# Four characters per token is the usual rule of thumb for English prose and code,
# and being a little conservative here is free: a chunk boundary only costs a reply.
CHUNK_CHARS = 8000

INSTRUCTION = (
    "You are a careful second reader for a Rust word processor. Below is part of a "
    "code review diff. Point out concrete problems only: a bug, a crash path, a "
    "claim the diff does not support, a test that would not catch the thing it "
    "names, or a user-visible string that is wrong. Do not restate the diff, do "
    "not praise it, and do not invent context you were not given. If this part has "
    "nothing wrong with it, reply with the single word NONE."
)


def request_patch(request_id):
    """The brief and the diff, exactly as the reviewer would read them."""
    try:
        out = subprocess.run(
            ["cargo", "xtask", "review", "show", request_id, "--patch"],
            capture_output=True,
            text=True,
            timeout=300,
        )
    except (OSError, subprocess.SubprocessError) as e:
        sys.exit(f"fm-review: could not run cargo xtask: {e}")
    if out.returncode != 0:
        sys.exit(f"fm-review: no such request: {request_id}\n{out.stderr.strip()}")
    return out.stdout


def chunks(text, size):
    """Split on line boundaries so a diff line is never cut in half."""
    buf, used = [], 0
    for line in text.splitlines(keepends=True):
        if used + len(line) > size and buf:
            yield "".join(buf)
            buf, used = [], 0
        buf.append(line)
        used += len(line)
    if buf:
        yield "".join(buf)


def ask(url, model, prompt):
    body = json.dumps(
        {
            "model": model,
            "temperature": 0,
            "messages": [{"role": "user", "content": prompt}],
        }
    ).encode()
    req = urllib.request.Request(
        f"{url.rstrip('/')}/v1/chat/completions",
        data=body,
        headers={"Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(req, timeout=300) as r:
            reply = json.load(r)
    except urllib.error.HTTPError as e:
        detail = e.read().decode("utf-8", "replace").strip()
        sys.exit(f"fm-review: the model refused ({e.code}): {detail}")
    except (urllib.error.URLError, OSError) as e:
        sys.exit(
            f"fm-review: no model server at {url} ({e}).\n"
            "Start one with `fm serve --port 8765` — it needs `sudo fm license` once."
        )
    try:
        return reply["choices"][0]["message"]["content"].strip()
    except (KeyError, IndexError, TypeError):
        sys.exit(f"fm-review: unexpected reply shape: {json.dumps(reply)[:400]}")


def main():
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("request", help="a review request id, e.g. rev-0005")
    p.add_argument("--url", default="http://127.0.0.1:8765", help="fm serve address")
    p.add_argument("--model", default="system", help="fm model name")
    p.add_argument("--max-chunks", type=int, default=8, help="chunks to read (default 8)")
    args = p.parse_args()

    text = request_patch(args.request)
    # The brief is prose for a human; the model's window is small, so spend it on the diff.
    marker = "--- diff ---"
    if marker in text:
        text = text.split(marker, 1)[1]
    parts = list(chunks(text, CHUNK_CHARS))
    if len(parts) > args.max_chunks:
        print(
            f"# {len(parts)} chunks, reading the first {args.max_chunks} "
            f"(--max-chunks to read more)"
        )
        parts = parts[: args.max_chunks]
    print(f"# {args.request}: {len(text)} bytes of diff, {len(parts)} chunk(s)")

    findings = 0
    for i, part in enumerate(parts, 1):
        answer = ask(args.url, args.model, f"{INSTRUCTION}\n\n---\n{part}\n---")
        if answer.strip().upper() == "NONE":
            continue
        findings += 1
        print(f"\n## chunk {i}/{len(parts)}\n{answer}")

    if findings == 0:
        print("\nThe model found nothing to name in any chunk.")
    return 0


if __name__ == "__main__":
    sys.exit(main())

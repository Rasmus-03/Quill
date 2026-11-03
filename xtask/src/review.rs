//! `cargo xtask review`: the hand-over between the agent that built something and the agent that
//! checks it.
//!
//! The queue lives in the git common directory (`<common>/wordcraft-review/`), so every worktree
//! of this repository sees the same requests and none of it is ever committed. An author that
//! believes the work is done runs `cargo xtask review submit`; a reviewer takes the oldest pending
//! request with `cargo xtask review next` and either sends it further (`accept`) or sends it back
//! (`changes --note …`), and the author waits for that verdict with `cargo xtask review wait`.
//!
//! `docs/review-loop.md` is the prose version, and the ritual in `AGENTS.md` is what makes an
//! agent do this without being told.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

/// The queue's directory inside the git common directory.
const DIR: &str = "wordcraft-review";
/// A diff longer than this is recorded by its diffstat alone: the reviewer reads the rest in the
/// worktree, and a queue of enormous JSON files serves nobody.
const PATCH_CAP: usize = 400_000;
/// How much of a note travels through Orca as a notification; the queue holds the whole thing.
const NOTICE_CAP: usize = 4000;
/// How many taken ids `submit` walks past before it gives up on a queue that is busy.
const MAX_ID_TRIES: u32 = 8;

const USAGE: &str = "\
usage: cargo xtask review <action>

actions:
  submit --title T [--note TEXT] [--base REF] [--gate TEXT]...
                  hand finished work to the reviewer: the commit range, the uncommitted
                  changes, the diff, the gates you ran, and how to reach you
  list [--json]   the queue, oldest first
  next            the oldest pending request, as the brief a reviewer needs
  show <id> [--patch]
                  one request in full; --patch adds the diff
  accept <id> [--note TEXT] [--to WHERE]
                  the work is good: send it further (WHERE defaults to `integration`)
  changes <id> --note TEXT
                  send it back: the note says, file by file, what has to change
  agent [--once] [--agent ID] [--mechanical] [--wait] [--interval SECS] [--claim-ttl SECS]
                  be the review stage: claim the oldest pending request, put it in front of a
                  reviewer (an Orca agent terminal in the author's worktree, or the configured
                  reviewer terminal) and let the verdict go back to the author. One request at a
                  time, without blocking on the verdict; --wait holds the pass open for it.
                  --mechanical runs the hard checks here instead and sends the work back when
                  they fail
  status [--id ID] [--latest]
                  what happened to my request: the one carrying this terminal's handle
                  (--latest reads the newest in the queue, whoever wrote it)
  wait [--id ID] [--timeout SECS] [--latest]
                  block until the verdict arrives (default 600 s)
  reviewer [--terminal HANDLE] [--name NAME]
                  show or set the reviewer that `submit` notifies

The queue is shared by every worktree of this repository and is never committed: it lives in
`$(git rev-parse --git-common-dir)/wordcraft-review`.
";

/// The actions that take a flag's value, so a bare word can be recognised as the request id.
const VALUE_FLAGS: &[&str] = &[
    "--title",
    "--note",
    "--base",
    "--gate",
    "--to",
    "--reviewer",
    "--terminal",
    "--name",
    "--id",
    "--timeout",
    "--agent",
    "--interval",
    "--claim-ttl",
];

pub fn run(root: &Path, args: &[&str]) -> Result<(), String> {
    match args.first().copied() {
        Some("submit") => submit(root, &args[1..]),
        Some("list") => list(root, &args[1..]),
        Some("next") => next(root, &args[1..]),
        Some("show") => show(root, &args[1..]),
        Some("accept") => verdict(root, &args[1..], true),
        Some("changes") => verdict(root, &args[1..], false),
        Some("agent") => agent(root, &args[1..]),
        Some("status") => status(root, &args[1..]),
        Some("wait") => wait(root, &args[1..]),
        Some("reviewer") => reviewer(root, &args[1..]),
        None => {
            print!("{USAGE}");
            Ok(())
        }
        Some(other) => Err(format!("review: unknown action `{other}`\n\n{USAGE}")),
    }
}

// ——— the queue ————————————————————————————————————————————————————————————————

/// The queue directory, created on demand. `WORDCRAFT_REVIEW_DIR` moves it, so a test (or a second
/// script on the same machine) can use a queue of its own without touching the shared one.
fn dir(root: &Path) -> Result<PathBuf, String> {
    if let Ok(custom) = std::env::var("WORDCRAFT_REVIEW_DIR") {
        let custom = custom.trim().to_string();
        if !custom.is_empty() {
            let d = PathBuf::from(custom);
            std::fs::create_dir_all(&d).map_err(|e| format!("review: {}: {e}", d.display()))?;
            return Ok(d);
        }
    }
    let common = git(root, &["rev-parse", "--git-common-dir"])?;
    let common = common.trim();
    if common.is_empty() {
        return Err("review: git reports no common directory".into());
    }
    let p = Path::new(common);
    let d = if p.is_absolute() { p.to_path_buf() } else { root.join(p) };
    let d = d.join(DIR);
    std::fs::create_dir_all(&d).map_err(|e| format!("review: {}: {e}", d.display()))?;
    Ok(d)
}

/// Every request in the queue, oldest first: its id with the record.
fn requests(root: &Path) -> Result<Vec<(String, Value)>, String> {
    requests_in(&dir(root)?)
}

/// The requests in one queue directory, oldest first (`dir()` makes the real one; the tests hand in
/// their own).
fn requests_in(d: &Path) -> Result<Vec<(String, Value)>, String> {
    let rd = std::fs::read_dir(d).map_err(|e| format!("review: {}: {e}", d.display()))?;
    let mut out: Vec<(String, Value)> = Vec::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.starts_with("rev-") || !name.ends_with(".json") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(e.path()) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            eprintln!("review: warning: {} is not JSON; skipped", e.path().display());
            continue;
        };
        out.push((name.trim_end_matches(".json").to_string(), v));
    }
    out.sort_by_key(|(id, _)| number(id));
    Ok(out)
}

/// `rev-0007` → 7, so the queue sorts by age rather than by a string comparison.
fn number(id: &str) -> u64 {
    id.trim_start_matches("rev-").parse::<u64>().unwrap_or(u64::MAX)
}

fn load(root: &Path, id: &str) -> Result<Value, String> {
    let d = dir(root)?;
    let path = d.join(format!("{id}.json"));
    let text = std::fs::read_to_string(&path).map_err(|_| format!("review: no request `{id}` in {}", d.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("review: {}: {e}", path.display()))
}

/// Replace a request that is already in the queue (a claim, a verdict, a resubmission).
fn store(root: &Path, id: &str, v: &Value) -> Result<(), String> {
    store_in(&dir(root)?, id, v)
}

fn store_in(d: &Path, id: &str, v: &Value) -> Result<(), String> {
    let path = d.join(format!("{id}.json"));
    let text = serde_json::to_string_pretty(v).map_err(|e| format!("review: {id}: {e}"))?;
    std::fs::write(&path, format!("{text}\n")).map_err(|e| format!("review: {}: {e}", path.display()))
}

/// Write a **new** request, never replacing one that is already there: two agents submitting in the
/// same second must not land on the same id and lose one of the two records.
///
/// `Ok(true)` says the id was taken by someone else; the caller takes the next one.
fn store_new_in(d: &Path, id: &str, v: &Value) -> Result<bool, String> {
    let path = d.join(format!("{id}.json"));
    let text = serde_json::to_string_pretty(v).map_err(|e| format!("review: {id}: {e}"))?;
    let mut file = match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Ok(true),
        Err(e) => return Err(format!("review: {}: {e}", path.display())),
    };
    file.write_all(format!("{text}\n").as_bytes()).map_err(|e| format!("review: {}: {e}", path.display()))?;
    Ok(false)
}

/// The next free id in a queue directory.
fn next_id_in(d: &Path) -> Result<String, String> {
    let next = requests_in(d)?.iter().map(|(id, _)| number(id)).filter(|n| *n != u64::MAX).max().unwrap_or(0) + 1;
    Ok(format!("rev-{next:04}"))
}

// ——— submit ————————————————————————————————————————————————————————————————————

fn submit(root: &Path, args: &[&str]) -> Result<(), String> {
    let title = args_of(args, "--title")
        .into_iter()
        .find(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string())
        .ok_or_else(|| format!("review submit: --title \"what this work does\" is required\n\n{USAGE}"))?;
    let note = args_of(args, "--note").join("\n");
    let base = match args_of(args, "--base").first() {
        Some(b) => git(root, &["rev-parse", b])?.trim().to_string(),
        None => base_commit(root)?,
    };
    let head = git(root, &["rev-parse", "HEAD"])
        .map_err(|_| "review submit: there are no commits here yet; commit first, or send the work back to its author".to_string())?;
    let head = head.trim().to_string();
    let branch = git_opt(root, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_else(|| "(detached)".into());
    let branch = branch.trim().to_string();
    let commits = git(root, &["log", "--format=%h %s", &format!("{base}..HEAD")])?;
    let commits: Vec<String> = commits.lines().filter(|l| !l.trim().is_empty()).map(|l| l.to_string()).collect();
    let stat = git(root, &["diff", "--stat", &base])?;
    let untracked: Vec<String> =
        git(root, &["ls-files", "--others", "--exclude-standard"])?.lines().filter(|l| !l.trim().is_empty()).map(|l| l.to_string()).collect();
    let dirty = !git(root, &["status", "--porcelain"])?.trim().is_empty();
    let mut patch = git(root, &["diff", &base])?;
    let mut patch_truncated = false;
    if patch.len() > PATCH_CAP {
        let cut = patch.char_indices().take_while(|(i, _)| *i < PATCH_CAP).last().map(|(i, c)| i + c.len_utf8()).unwrap_or(0);
        patch.truncate(cut);
        patch_truncated = true;
    }
    let gates: Vec<String> = args_of(args, "--gate").into_iter().map(|s| s.to_string()).collect();
    let terminal = args_of(args, "--terminal").first().map(|s| s.to_string()).or_else(terminal_handle);
    let reviewer = args_of(args, "--reviewer").first().map(|s| s.to_string()).or_else(|| configured_reviewer(root).map(|(h, _)| h));

    let mut record = json!({
        "id": Value::Null,
        "title": title,
        "note": note,
        "status": "pending",
        "revision": 1,
        "parent": Value::Null,
        "created_at": timestamp(),
        "author": {
            "branch": branch,
            "worktree": root.display().to_string(),
            "head": head,
            "base": base,
            "terminal": terminal.clone(),
        },
        "commits": commits,
        "dirty": dirty,
        "untracked": untracked,
        "diffstat": stat,
        "patch": patch,
        "patch_truncated": patch_truncated,
        "gates": gates,
        "reviewer": reviewer.clone(),
        "review": Value::Null,
    });
    // The queue is shared by every worktree, so the id is claimed rather than computed: a second
    // agent submitting in the same second must not overwrite this record.
    let queue = dir(root)?;
    let mut id = next_id_in(&queue)?;
    let mut tries = 0u32;
    loop {
        record["id"] = json!(id);
        if !store_new_in(&queue, &id, &record)? {
            break;
        }
        tries += 1;
        if tries >= MAX_ID_TRIES {
            return Err(format!(
                "review submit: {tries} ids in a row were already taken (last {id}); other agents are writing the queue. Try again in a moment."
            ));
        }
        id = next_id_in(&queue)?;
    }

    println!("Review requested: {id} — {}", record["title"].as_str().unwrap_or(""));
    println!("  branch   {} ({} commit(s) since {})", branch, record["commits"].as_array().map(|c| c.len()).unwrap_or(0), short(&base));
    if dirty {
        println!("  note     the working tree has uncommitted changes; they are in the request's diff");
    }
    let gate_note = if gates.is_empty() { "none declared — the reviewer will run them".to_string() } else { gates.join("; ") };
    println!("  gates    {gate_note}");
    println!();
    println!("What happens next: the reviewer takes it with `cargo xtask review next`, and either");
    println!("accepts it (`accept {id}`) or sends it back (`changes {id} --note \"…\"`).");
    println!("Wait for that verdict with `cargo xtask review wait {id}`.");

    // The reviewer is told, so nobody has to poll the queue to find out that work arrived.
    if let Some(r) = reviewer.as_deref() {
        let subject = format!("Review requested: {id} — {}", record["title"].as_str().unwrap_or(""));
        let body = format!(
            "{}. Branch {branch}, {n} commit(s), {files} file(s) changed. Gates: {gates}\n\n{}\n\nTake it with `cargo xtask review next`.",
            record["title"].as_str().unwrap_or(""),
            first_lines(record["note"].as_str().unwrap_or(""), 20),
            n = record["commits"].as_array().map(|c| c.len()).unwrap_or(0),
            files = stat.lines().last().unwrap_or("").trim(),
            gates = gate_note,
        );
        println!();
        println!("{}", notify(r, &subject, &body));
    }
    Ok(())
}

/// The commit this work starts from, so the reviewer reads what changed and not the history of the
/// branch it sits on. In order: the head of the last accepted review on this branch (the delta
/// since then), the commit the branch was created from, the default branch's merge base, the first
/// commit there is. `--base REF` overrides all of it.
fn base_commit(root: &Path) -> Result<String, String> {
    let current = git_opt(root, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default().trim().to_string();
    if !current.is_empty() {
        // The last accepted review on this branch: work already seen is not re-reviewed.
        if let Ok(all) = requests(root) {
            for (_, v) in all.iter().rev() {
                let same_branch = v["author"]["branch"].as_str() == Some(current.as_str());
                if same_branch
                    && v["status"] == "accepted"
                    && let Some(head) = v["author"]["head"].as_str()
                    && is_ancestor(root, head, "HEAD")
                {
                    return Ok(head.to_string());
                }
            }
        }
        // Where the branch was created from: `git worktree add -b task integration` records it.
        if let Some(from) = created_from(root, &current)
            && is_ancestor(root, &from, "HEAD")
            && let Ok(mb) = git(root, &["merge-base", "HEAD", &from])
        {
            let mb = mb.trim().to_string();
            if !mb.is_empty() {
                return Ok(mb);
            }
        }
    }
    let mut candidates: Vec<String> = Vec::new();
    if let Some(head) = git_opt(root, &["symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"])
        && let Some((_, b)) = head.trim().split_once('/')
    {
        candidates.push(b.to_string());
    }
    for b in ["main", "master", "develop", "trunk"] {
        candidates.push(b.to_string());
    }
    for c in &candidates {
        if *c == current || c.is_empty() {
            continue;
        }
        if git_opt(root, &["rev-parse", "--verify", "--quiet", &format!("{c}^{{commit}}")]).is_none() {
            continue;
        }
        if let Ok(mb) = git(root, &["merge-base", "HEAD", c]) {
            let mb = mb.trim().to_string();
            if !mb.is_empty() {
                return Ok(mb);
            }
        }
    }
    let first = git(root, &["rev-list", "--max-parents=0", "HEAD"])?;
    let first = first.lines().last().unwrap_or("").trim().to_string();
    if first.is_empty() { Err("review: cannot work out what this branch starts from; pass --base REF".into()) } else { Ok(first) }
}

// ——— the reviewer's side ——————————————————————————————————————————————————————

/// The reflog's `branch: Created from <ref>`, when the branch has one that resolves.
fn created_from(root: &Path, branch: &str) -> Option<String> {
    let log = git_opt(root, &["reflog", "show", "--format=%gs", branch])?;
    // The reflog is newest first; the creation line is the oldest, and the only one with this text.
    for line in log.lines().rev() {
        if let Some(from) = line.trim().strip_prefix("branch: Created from ") {
            if from == "HEAD" {
                return None;
            }
            if git_opt(root, &["rev-parse", "--verify", "--quiet", &format!("{from}^{{commit}}")]).is_some() {
                return Some(from.to_string());
            }
        }
    }
    None
}

fn is_ancestor(root: &Path, ancestor: &str, descendant: &str) -> bool {
    Command::new("git")
        .args(["merge-base", "--is-ancestor", ancestor, descendant])
        .current_dir(root)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn list(root: &Path, args: &[&str]) -> Result<(), String> {
    let all = requests(root)?;
    if args.contains(&"--json") {
        let out: Vec<&Value> = all.iter().map(|(_, v)| v).collect();
        let text = serde_json::to_string_pretty(&out).map_err(|e| format!("review: {e}"))?;
        println!("{text}");
        return Ok(());
    }
    if all.is_empty() {
        println!("No review requests. They appear when an author runs `cargo xtask review submit`.");
        return Ok(());
    }
    let pending = all.iter().filter(|(_, v)| v["status"] == "pending").count();
    let reviewing = all.iter().filter(|(_, v)| v["status"] == "reviewing").count();
    println!("{} request(s), {pending} pending, {reviewing} under review — {}", all.len(), dir(root)?.display());
    for (id, v) in &all {
        let status = v["status"].as_str().unwrap_or("?");
        let mark = match status {
            "pending" => "•",
            "reviewing" => "…",
            "accepted" => "✓",
            "changes" => "↩",
            _ => "?",
        };
        println!("{mark} {id}  {status:<8} {:<14} {}", v["author"]["branch"].as_str().unwrap_or("?"), v["title"].as_str().unwrap_or(""));
    }
    println!();
    println!("`cargo xtask review next` shows the oldest pending request as a brief.");
    Ok(())
}

fn next(root: &Path, _args: &[&str]) -> Result<(), String> {
    let all = requests(root)?;
    let Some((id, v)) = all.iter().find(|(_, v)| v["status"] == "pending") else {
        println!("Nothing to review. The queue is empty of pending requests.");
        return Ok(());
    };
    let waiting = all.iter().filter(|(_, v)| v["status"] == "pending").count();
    print_brief(id, v, waiting, true);
    Ok(())
}

fn print_brief(id: &str, v: &Value, waiting: usize, with_how: bool) {
    let s = |path: &[&str]| -> String {
        let mut cur = v;
        for k in path {
            cur = &cur[*k];
        }
        match cur {
            Value::String(t) => t.clone(),
            Value::Null => String::new(),
            other => other.to_string(),
        }
    };
    println!("Review requested: {id}  ({waiting} pending)");
    println!("  title    {}", s(&["title"]));
    println!("  author   {} in {}", s(&["author", "branch"]), s(&["author", "worktree"]));
    let reach = s(&["author", "terminal"]);
    if !reach.is_empty() {
        println!("  reach    orca terminal {reach}");
    }
    let commits = v["commits"].as_array().map(|c| c.len()).unwrap_or(0);
    println!("  range    {}..{}  ({commits} commit(s))", short(&s(&["author", "base"])), short(&s(&["author", "head"])));
    if v["dirty"].as_bool().unwrap_or(false) {
        println!("  state    uncommitted changes are part of the request (see the diff)");
    }
    let note = s(&["note"]);
    if !note.trim().is_empty() {
        println!("  asked for:");
        for line in note.lines() {
            println!("    {line}");
        }
    }
    let gates = v["gates"].as_array().cloned().unwrap_or_default();
    if gates.is_empty() {
        println!("  gates    none declared: run them yourself");
    } else {
        println!("  gates    the author ran:");
        for g in gates {
            println!("    - {}", g.as_str().unwrap_or(""));
        }
    }
    println!("  diffstat");
    let diffstat = s(&["diffstat"]);
    let stat: Vec<&str> = diffstat.lines().filter(|l| !l.trim().is_empty()).collect();
    // A 200-file diffstat buries the brief; the rest is one `review show --patch` away. The last
    // two lines are kept whatever happens: they are the totals.
    const KEEP: usize = 25;
    if stat.len() <= KEEP + 2 {
        for line in &stat {
            println!("    {line}");
        }
    } else {
        for line in stat.iter().take(KEEP) {
            println!("    {line}");
        }
        println!("    … {} more line(s); `cargo xtask review show {id} --patch` has the rest", stat.len() - KEEP - 2);
        for line in stat.iter().skip(stat.len() - 2) {
            println!("    {line}");
        }
    }
    let untracked = v["untracked"].as_array().cloned().unwrap_or_default();
    if !untracked.is_empty() {
        println!("  untracked (not in the diff):");
        for f in untracked {
            println!("    {}", f.as_str().unwrap_or(""));
        }
    }
    let commits = v["commits"].as_array().cloned().unwrap_or_default();
    if !commits.is_empty() {
        println!("  commits");
        for c in commits {
            println!("    {}", c.as_str().unwrap_or(""));
        }
    }
    if with_how {
        println!();
        println!("Read the diff with `cargo xtask review show {id} --patch`, and check the work against");
        println!("docs/review-loop.md (the checklist) rather than against the author's summary.");
        println!();
        println!("Send it further:   cargo xtask review accept {id} --note \"what you checked\"");
        println!("Send it back:      cargo xtask review changes {id} --note \"file, what is wrong, what done looks like\"");
    }
}

fn show(root: &Path, args: &[&str]) -> Result<(), String> {
    let id = positional(args).ok_or("review show: which request? `cargo xtask review list`")?;
    let v = load(root, id)?;
    let pending = requests(root)?.iter().filter(|(_, r)| r["status"] == "pending").count();
    print_brief(id, &v, pending, false);
    let review = &v["review"];
    if !review.is_null() {
        println!();
        println!("  reviewed by {} at {}", review["reviewer"].as_str().unwrap_or("?"), review["at"].as_str().unwrap_or("?"));
        println!("  verdict  {}", review["verdict"].as_str().unwrap_or("?"));
        if let Some(to) = review["to"].as_str()
            && !to.is_empty()
        {
            println!("  sent to  {to}");
        }
        let note = review["note"].as_str().unwrap_or("");
        if !note.trim().is_empty() {
            println!("  message:");
            for line in note.lines() {
                println!("    {line}");
            }
        }
    }
    if args.contains(&"--patch") {
        println!();
        println!("--- diff ---");
        let patch = v["patch"].as_str().unwrap_or("");
        if patch.trim().is_empty() {
            println!("(no diff against the base: the work is already on the base branch, or nothing changed)");
        } else {
            println!("{patch}");
        }
        if v["patch_truncated"].as_bool().unwrap_or(false) {
            println!(
                "(the diff was cut short; read the rest with `git -C {} diff {}`)",
                v["author"]["worktree"].as_str().unwrap_or("."),
                v["author"]["base"].as_str().unwrap_or("HEAD~1")
            );
        }
    }
    Ok(())
}

/// `accept` (send it further) and `changes` (send it back with a message).
fn verdict(root: &Path, args: &[&str], accept: bool) -> Result<(), String> {
    let id = positional(args).ok_or("review: which request? `cargo xtask review list`")?.to_string();
    let note = args_of(args, "--note").join("\n");
    if !accept && note.trim().is_empty() {
        return Err(format!(
            "review changes {id}: --note is required — the author gets only this message.\n\
             Say which files, what is wrong, and what done looks like. \"Please fix\" is not a review."
        ));
    }
    let mut v = load(root, &id)?;
    // A request that the review stage has claimed is still unsettled: the claim is bookkeeping, not
    // a verdict, so the reviewer that took it may still answer.
    let open = matches!(v["status"].as_str(), Some("pending") | Some("reviewing"));
    if !open {
        return Err(format!(
            "review: {id} was already reviewed ({}); a settled request is history, not a queue entry",
            v["status"].as_str().unwrap_or("?")
        ));
    }
    let to = if accept { args_of(args, "--to").first().map(|s| s.to_string()).unwrap_or_else(|| "integration".into()) } else { String::new() };
    // The name the machine knows the reviewer by, when it is the reviewer that is answering.
    let configured = configured_reviewer(root);
    let mine = terminal_handle();
    let reviewer = args_of(args, "--reviewer")
        .first()
        .map(|s| s.to_string())
        .or_else(|| match (&configured, &mine) {
            (Some((h, name)), Some(m)) if h == m => Some(name.clone()),
            _ => None,
        })
        .or_else(|| mine.clone())
        .unwrap_or_else(|| "unattributed".into());
    let verdict = if accept { "accepted" } else { "changes" };
    v["status"] = json!(verdict);
    v["claim"] = Value::Null;
    v["review"] = json!({
        "verdict": verdict,
        "reviewer": reviewer,
        "at": timestamp(),
        "note": note,
        "to": to,
    });
    store(root, &id, &v)?;

    let title = v["title"].as_str().unwrap_or("");
    if accept {
        println!("{id} accepted — sent further to {to}.");
        if !note.trim().is_empty() {
            println!("  {}", note.replace('\n', "\n  "));
        }
        let author = v["author"]["terminal"].as_str().unwrap_or("");
        if !author.is_empty() {
            println!(
                "{}",
                notify(
                    author,
                    &format!("Review accepted: {id} — {title}"),
                    &format!(
                        "Your work was accepted and sent on to {to}.{}\n\n{id} is settled; nothing more is needed from you on it.",
                        if note.trim().is_empty() { String::new() } else { format!("\n\n{note}") }
                    )
                )
            );
        }
        // Where "further" is another agent, tell it; where it is a stage of the release, the queue
        // records it and `list` shows it.
        if !to.is_empty() && to.starts_with("term_") {
            println!("{}", notify(&to, &format!("Work accepted: {id} — {title}"), &format!("Reviewed and accepted; it is yours now.\n\n{note}")));
        }
    } else {
        println!("{id} sent back to {} with the changes it needs:", v["author"]["branch"].as_str().unwrap_or("the author"));
        for line in note.lines() {
            println!("  {line}");
        }
        let author = v["author"]["terminal"].as_str().unwrap_or("");
        if author.is_empty() {
            println!();
            println!("The author left no terminal handle: the message is in the queue, and `cargo xtask review list` shows it.");
        } else {
            println!();
            println!(
                "{}",
                notify(
                    author,
                    &format!("Changes requested: {id} — {title}"),
                    &format!(
                        "Your work came back from review.\n\n{note}\n\nFix it and resubmit: `cargo xtask review submit --title \"{title}\" --note \"what you changed\"`."
                    )
                )
            );
        }
    }
    Ok(())
}

// ——— the review stage —————————————————————————————————————————————————————————

/// How long a claim is believed before another pass of the stage may take the request over. A
/// reviewer that died without answering must not block the queue for ever; a live one must not be
/// duplicated.
const CLAIM_TTL: u64 = 1800;

/// `cargo xtask review agent`: the stage itself, rather than an agent that remembers to be one.
///
/// Each pass takes one pending request, records who has it, puts it in front of a reviewer and
/// lets the verdict go back to the author: `accept` sends the work further, `changes` sends it back
/// to the agent that wrote it — which is the half of the loop that used to depend on somebody
/// noticing the queue had grown.
fn agent(root: &Path, args: &[&str]) -> Result<(), String> {
    let once = args.contains(&"--once");
    let mechanical = args.contains(&"--mechanical");
    // Off by default: a stage that blocks on one verdict cannot take the next request, and a
    // reviewer that is slow (or out of capacity) would then stall the whole queue behind it.
    let wait = args.contains(&"--wait");
    let agent_id = args_of(args, "--agent").first().map(|s| s.to_string());
    let interval = args_of(args, "--interval").first().and_then(|s| s.parse::<u64>().ok()).unwrap_or(30).max(1);
    let ttl = args_of(args, "--claim-ttl").first().and_then(|s| s.parse::<u64>().ok()).unwrap_or(CLAIM_TTL);
    let timeout = args_of(args, "--timeout").first().and_then(|s| s.parse::<u64>().ok()).unwrap_or(1800);

    if mechanical && agent_id.is_some() {
        return Err("review agent: --mechanical runs the checks here; --agent starts a reviewer agent. Pick one.".into());
    }
    let how = if mechanical {
        "mechanical checks only — a person or agent still has to read what passes".to_string()
    } else {
        // This terminal being the reviewer (because a previous run registered it, or because the
        // stage was started here) is not a hand-off: the reading still needs a reader.
        match (&agent_id, configured_reviewer(root).filter(|(h, _)| Some(h.as_str()) != terminal_handle().as_deref())) {
            (Some(a), _) => format!("an Orca worker ({a}) in each author's worktree"),
            (None, Some((_, name))) => format!("the configured reviewer, {name}"),
            (None, None) => {
                return Err("review agent: no reader to hand the work to. Pass --agent codex so each request\n\
                     gets an Orca worker in the author's worktree, set a standing reviewer with\n\
                     `cargo xtask review reviewer --terminal HANDLE --name NAME`, or run with\n\
                     --mechanical for the checks alone. (A stage that is itself the reviewer still\n\
                     needs a reader: it is the door, not the person who looks.)"
                    .into());
            }
        }
    };
    println!("Review stage: {how}.");
    println!("Queue: {}", dir(root)?.display());
    // Inside Orca the stage is the reviewer as far as the queue is concerned: `submit` knocks on
    // the stage's door instead of on a person's, so nobody has to run `review reviewer` first on a
    // machine that already has a stage. --no-register leaves the queue's reviewer alone.
    if !args.contains(&"--no-register")
        && let Some(me) = terminal_handle()
        && configured_reviewer(root).is_none()
        && let Ok(d) = dir(root)
    {
        let body = json!({ "handle": me, "name": "review stage", "set_at": timestamp(), "set_by": "cargo xtask review agent" });
        if let Ok(text) = serde_json::to_string_pretty(&body) {
            let path = d.join("reviewer.json");
            if std::fs::write(&path, format!("{text}\n")).is_ok() {
                println!("Registered {me} as the reviewer: `submit` now notifies the stage.");
            }
        }
    }
    loop {
        // One refused hand-off is not the stage's end: it puts the request back in the queue and
        // the next pass tries again, which is what a standing stage is for. --once still reports it.
        match one_pass(root, mechanical, agent_id.as_deref(), wait, ttl, timeout) {
            Ok(Some(line)) => println!("{line}"),
            Ok(None) => println!("Nothing pending; waiting {interval} s."),
            Err(e) if once => return Err(e),
            Err(e) => {
                eprintln!("error: {e}");
                println!("Waiting {interval} s before the next pass.");
            }
        }
        if once {
            return Ok(());
        }
        std::thread::sleep(Duration::from_secs(interval));
    }
}

/// One request through the stage, or `None` when the queue has nothing to give.
fn one_pass(root: &Path, mechanical: bool, agent_id: Option<&str>, wait: bool, ttl: u64, timeout: u64) -> Result<Option<String>, String> {
    let all = requests(root)?;
    let now = unix_secs();
    let mut picked: Option<(String, Value)> = None;
    for (id, v) in &all {
        let status = v["status"].as_str().unwrap_or("");
        // A claim that has outlived its ttl is a reviewer that is not coming back.
        let stale = status == "reviewing" && claim_age(v, now) >= ttl;
        if status == "pending" || stale {
            picked = Some((id.clone(), v.clone()));
            break;
        }
    }
    let Some((id, mut v)) = picked else { return Ok(None) };

    let by = terminal_handle().unwrap_or_else(|| "cargo xtask review agent".to_string());
    v["status"] = json!("reviewing");
    v["claim"] = json!({ "by": by, "at": timestamp(), "since": now, "agent": agent_id });
    store(root, &id, &v)?;
    println!("{id} — {} (claimed for review)", v["title"].as_str().unwrap_or(""));

    if mechanical {
        return mechanical_review(root, &id, &v).map(Some);
    }
    if let Err(e) = hand_to_reviewer(root, &id, &v, agent_id) {
        // Nobody is reading it after all: put it back where the next pass can find it.
        v["status"] = json!("pending");
        v["claim"] = Value::Null;
        store(root, &id, &v)?;
        return Err(e);
    }
    if !wait {
        // Dispatch and move on. The verdict is written to the queue file and knocks on the author's
        // door from there, so holding this pass open would only delay the next request.
        return Ok(Some(format!("{id}: handed off; the verdict settles in the queue and reaches the author.")));
    }
    wait_for_verdict(root, &id, timeout).map(Some)
}

/// Seconds since the claim was recorded. A request that carries no readable claim has nothing to
/// wait for, so it is old enough to be taken over at once.
fn claim_age(v: &Value, now: u64) -> u64 {
    match v["claim"]["since"].as_u64() {
        Some(since) => now.saturating_sub(since),
        None => u64::MAX,
    }
}

/// Hand the request to whoever will read it: a fresh Orca worker in the author's worktree, or the
/// terminal of the reviewer that was configured once with `cargo xtask review reviewer`.
fn hand_to_reviewer(root: &Path, id: &str, v: &Value, agent_id: Option<&str>) -> Result<(), String> {
    let spec = reviewer_spec(id, v);
    match agent_id {
        Some(agent) => start_worker(v, agent, &spec),
        None => match configured_reviewer(root) {
            Some((handle, name)) => {
                let subject = format!("Review requested: {id} — {}", v["title"].as_str().unwrap_or(""));
                let body = format!(
                    "{spec}\n\nSettle it when you are done: `cargo xtask review accept {id} --note \"…\"` or\n\
                     `cargo xtask review changes {id} --note \"file:line — what is wrong, and what done looks like\"`."
                );
                println!("Handed to {name} ({handle}). {}", notify(&handle, &subject, &body));
                Ok(())
            }
            None => Err("review agent: no reviewer is set".into()),
        },
    }
}

/// What a reviewer needs to start, written as the task it is given: the queue entry is the record,
/// this is the instruction.
fn reviewer_spec(id: &str, v: &Value) -> String {
    let worktree = v["author"]["worktree"].as_str().unwrap_or(".").trim().to_string();
    let branch = v["author"]["branch"].as_str().unwrap_or("(unknown)");
    let gates: Vec<String> = v["gates"].as_array().map(|g| g.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect()).unwrap_or_default();
    let gates = if gates.is_empty() {
        "  - none declared; run them yourself".to_string()
    } else {
        gates.iter().map(|g| format!("  - {g}")).collect::<Vec<_>>().join("\n")
    };
    let note = first_lines(v["note"].as_str().unwrap_or(""), 20);
    // Built line by line rather than as one continuation-heavy literal: the indentation a reviewer
    // reads is then spelled out, instead of eaten by Rust's line continuation.
    let mut s = String::new();
    s.push_str(&format!("You are the reviewer for {id} in the WordCraft repository, working in {worktree}.\n\n"));
    s.push_str(&format!("Request: {}\nBranch: {branch}\n", v["title"].as_str().unwrap_or("")));
    if !note.trim().is_empty() {
        s.push_str(&format!("\nWhat the author says it does:\n{note}\n"));
    }
    s.push_str(&format!("\nGates the author says it ran:\n{gates}\n"));
    s.push_str("\nDo this, in this order:\n");
    s.push_str(&format!("  1. Read the change, not the summary: `cargo xtask review show {id} --patch`.\n"));
    s.push_str("  2. Walk the checklist in docs/review-loop.md, \"What to check\": the change matches the\n");
    s.push_str("     request, the gates run by you, no panic path, assets attributed, clean room, layering,\n");
    s.push_str("     tests that would fail, the docs moved, nothing left behind.\n");
    s.push_str("  3. Settle it:\n");
    s.push_str(&format!("       cargo xtask review accept {id} --note \"what you checked, and where you looked\"\n"));
    s.push_str("     or send it back with every finding, file and line:\n");
    s.push_str(&format!("       cargo xtask review changes {id} --note \"<file>:<line> — what is wrong, and what done looks like\"\n"));
    s.push_str("\nDo not submit a request of your own, and do not edit the author's code: the verdict is\n");
    s.push_str("your output. The author is notified when you settle it.");
    s
}

/// Start a reviewer as its own Orca agent terminal in the author's worktree: the reviewer has to
/// see the code, and the queue's diff is only a summary of it.
///
/// `orca agent launch`, not `orca orchestration worker-start`: worker-start is fenced to the
/// coordinator terminal currently bound to a Task Run (`consumer_fenced`), and the review stage is
/// not part of the author's task. A stage that tried it was refused on every request.
fn start_worker(v: &Value, agent: &str, spec: &str) -> Result<(), String> {
    let worktree = v["author"]["worktree"].as_str().unwrap_or("").trim();
    if worktree.is_empty() {
        return Err("review agent: the request has no worktree to review in".into());
    }
    let out = Command::new("orca")
        .args(["agent", "launch", "--agent", agent])
        .args(["--worktree", &format!("path:{worktree}")])
        .args(["--prompt", spec, "--surface", "terminal", "--json"])
        .output()
        .map_err(|e| format!("review agent: could not run orca ({e}); the request is back in the queue"))?;
    // Orca reports refusals in its JSON on stdout, with an empty stderr: say what is known rather
    // than quoting a wall of JSON into a review note.
    let receipt: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
    if !out.status.success() || !receipt["ok"].as_bool().unwrap_or(false) {
        let detail = if let Some(message) = receipt["error"]["message"].as_str() {
            message.to_string()
        } else {
            let err = String::from_utf8_lossy(&out.stderr);
            let line = first_line(&err);
            if line.is_empty() { "orca refused the launch (see `orca agent launch --json`)".to_string() } else { line }
        };
        return Err(format!(
            "review agent: `orca agent launch --agent {agent}` failed: {detail}; the request is back in the queue.\n\
             \"{agent}\" has to be an agent enabled on this machine (`orca agent launch --agent <id>`)."
        ));
    }
    let handle = receipt["result"]["outcome"]["handle"].as_str().unwrap_or("");
    println!("Reviewer started: {agent} in {worktree} ({handle}). `orca terminal read --terminal {handle}` shows it.");
    Ok(())
}

/// The hard, mechanical half of a review: what a machine can prove without judgement. Anything it
/// finds goes back to the author as a `changes` message; what passes is not accepted on its
/// behalf — a reading is still owed.
fn mechanical_review(root: &Path, id: &str, v: &Value) -> Result<String, String> {
    let worktree: PathBuf =
        v["author"]["worktree"].as_str().map(str::trim).filter(|w| !w.is_empty()).map(PathBuf::from).unwrap_or_else(|| root.to_path_buf());
    let mut findings = patch_findings(v["patch"].as_str().unwrap_or(""));
    if findings.is_empty() {
        // The gates only matter if the code is not already on its way back: a run that is going to
        // fail costs ten minutes and tells the author nothing new.
        println!("Running `cargo xtask ci` in {} …", worktree.display());
        match gates_in(&worktree) {
            Ok(()) => {}
            Err(f) => findings.push(f),
        }
    }
    if findings.is_empty() {
        // Nothing wrong that a machine can see. It goes back in the queue, not out the door.
        let mut v = v.clone();
        v["status"] = json!("pending");
        v["claim"] = Value::Null;
        v["mechanical"] = json!({ "at": timestamp(), "verdict": "pass" });
        store(root, id, &v)?;
        return Ok(format!(
            "{id}: the mechanical checks pass (`cargo xtask ci`, no panic path in the diff). Back in the \
             queue: a reviewer still has to read it before it goes further."
        ));
    }
    let note = format!("Mechanical review of {id} — the checks below failed, before anyone read the work:\n\n{}", findings.join("\n\n"));
    verdict(root, &[id, "--note", note.as_str(), "--reviewer", "cargo-xtask-mechanical"], false)?;
    Ok(format!("{id}: {} mechanical finding(s); sent back to the author.", findings.len()))
}

/// `cargo xtask ci` where the work is, reported as the one finding it is.
fn gates_in(worktree: &Path) -> Result<(), String> {
    match Command::new("cargo").args(["xtask", "ci"]).current_dir(worktree).output() {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr);
            let tail: Vec<&str> = err.lines().rev().take(40).collect();
            let tail: Vec<&str> = tail.into_iter().rev().collect();
            Err(format!("cargo xtask ci — failed in {}:\n{}", worktree.display(), tail.join("\n")))
        }
        Err(e) => Err(format!("cargo xtask ci — could not run in {}: {e}", worktree.display())),
    }
}

/// The lines a diff adds that this repository forbids: a crash path where a `Result` belongs, and
/// `unsafe` anywhere.
///
/// The scan errs towards saying nothing: comments are skipped (the project talks about `unwrap`
/// constantly), so is a mention inside a string literal, and so is everything after `#[cfg(test)]`
/// in a file, because `clippy.toml` allows a test to panic. What is left is a line that would crash
/// the app, and the author gets it by file and line.
fn patch_findings(patch: &str) -> Vec<String> {
    const FORBIDDEN: &[(&str, &str)] = &[
        (".unwrap()", "unwrap"),
        (".expect(", "expect"),
        ("panic!(", "panic!"),
        ("unreachable!(", "unreachable!"),
        ("todo!(", "todo!"),
        ("unimplemented!(", "unimplemented!"),
        ("unsafe ", "unsafe"),
    ];
    let mut file = String::new();
    let mut line_no = 0u64;
    // Set by `#[cfg(test)]`, which is where the tests in a single-file module begin.
    let mut in_tests = false;
    let mut out: Vec<String> = Vec::new();
    for line in patch.lines() {
        if let Some(rest) = line.strip_prefix("+++ b/") {
            file = rest.trim().to_string();
            line_no = 0;
            in_tests = false;
            continue;
        }
        if line.starts_with("@@") {
            // `@@ -a,b +c,d @@`: where this hunk starts in the new file. The number is the line of
            // the first line in the hunk, so the counter starts one before it.
            let plus = line.split('+').nth(1).unwrap_or("");
            let start = plus.split([' ', ',']).next().unwrap_or("");
            line_no = start.parse::<u64>().unwrap_or(0).saturating_sub(1);
            continue;
        }
        let Some(body) = line.strip_prefix('+') else {
            // Context lines advance the new file too; removals do not.
            if line.starts_with(' ') {
                line_no = line_no.saturating_add(1);
                if line.trim_start().starts_with("#[cfg(test)]") {
                    in_tests = true;
                }
            }
            continue;
        };
        line_no = line_no.saturating_add(1);
        if file.is_empty() || is_test_path(&file) || in_tests {
            continue;
        }
        let trimmed = body.trim_start();
        if trimmed == "#[cfg(test)]" {
            in_tests = true;
            continue;
        }
        if trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with("/*") {
            continue;
        }
        for (needle, name) in FORBIDDEN {
            if let Some(at) = body.find(needle)
                && !inside_string(body, at)
            {
                out.push(format!(
                    "{file}:{line_no} — `{name}` in non-test code. Never crash: return the error, or clamp and check \
                     the number. Regression test or removal, and resubmit."
                ));
                break;
            }
        }
    }
    out
}

/// Whether the text before `at` leaves a string literal open, so the match is a mention rather
/// than code: the forbidden lints table quotes itself, and a test fixture quotes the quotes.
/// Escapes count, or `\"` would read as the end of the string it is inside.
fn inside_string(line: &str, at: usize) -> bool {
    let mut quoted = false;
    let mut escaped = false;
    for ch in line.get(..at).unwrap_or("").chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => quoted = !quoted,
            _ => {}
        }
    }
    quoted
}

/// Files where `clippy.toml` lets a test panic, and the scan has nothing to say.
fn is_test_path(path: &str) -> bool {
    path.starts_with("tests/")
        || path.contains("/tests/")
        || path.contains("/tests_")
        || path.ends_with("tests.rs")
        || path.ends_with("_test.rs")
        || path.ends_with("_tests.rs")
}

/// Block until the request is settled, or say that it is still out. A timeout is a checkpoint: the
/// claim keeps a second reviewer from duplicating the work, and the ttl releases it later.
fn wait_for_verdict(root: &Path, id: &str, limit: u64) -> Result<String, String> {
    let started = Instant::now();
    let deadline = started + Duration::from_secs(limit.max(1));
    loop {
        let v = load(root, id)?;
        if settled(v["status"].as_str()) {
            return Ok(format!("{id} settled: {} after {} s.", v["status"].as_str().unwrap_or("?"), started.elapsed().as_secs()));
        }
        if Instant::now() >= deadline {
            return Ok(format!(
                "No verdict on {id} after {} s; it stays claimed, and another pass may take it over after \
                 the ttl. This is a checkpoint, not a failure — `cargo xtask review status {id}` reads it.",
                limit.max(1)
            ));
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

/// A request is settled when it has a verdict: `reviewing` is a claim, not an answer.
fn settled(status: Option<&str>) -> bool {
    matches!(status, Some("accepted") | Some("changes"))
}

// ——— the author's side ————————————————————————————————————————————————————————

fn status(root: &Path, args: &[&str]) -> Result<(), String> {
    let id = match author_request(root, args, "status")? {
        Some(id) => id,
        None => {
            println!("You have no requests in the queue. Submit one with `cargo xtask review submit`.");
            return Ok(());
        }
    };
    let v = load(root, &id)?;
    print_status(&id, &v);
    Ok(())
}

fn print_status(id: &str, v: &Value) {
    let status = v["status"].as_str().unwrap_or("?");
    println!("{id} — {}", v["title"].as_str().unwrap_or(""));
    println!("  status   {status}");
    if let Some(c) = v["claim"].as_object() {
        println!("  claimed  {} at {}", c.get("by").and_then(|x| x.as_str()).unwrap_or("?"), c.get("at").and_then(|x| x.as_str()).unwrap_or("?"));
    }
    if let Some(r) = v["review"].as_object() {
        println!("  reviewer {}", r.get("reviewer").and_then(|x| x.as_str()).unwrap_or("?"));
        if let Some(to) = r.get("to").and_then(|x| x.as_str())
            && !to.is_empty()
        {
            println!("  sent to  {to}");
        }
        let note = r.get("note").and_then(|x| x.as_str()).unwrap_or("");
        if !note.trim().is_empty() {
            println!("  message");
            for line in note.lines() {
                println!("    {line}");
            }
        }
    } else {
        println!("  waiting for a reviewer; `cargo xtask review wait {id}` blocks until there is a verdict");
    }
    let verdict = match status {
        "accepted" => "accepted",
        "changes" => "changes requested",
        "reviewing" => "under review (claimed; not settled yet)",
        _ => "pending",
    };
    println!("verdict: {verdict}");
}

/// The newest request submitted from this terminal, whatever its state.
/// Which request one of the author's commands is about: `--id`, else the one carrying this
/// terminal's handle, else — only with `--latest` — the newest in the queue.
///
/// Without a terminal handle nobody can be identified, so this errors and asks for `--id` rather
/// than reading the newest request: that would hand an agent that is not the author someone
/// else's verdict, and it would read as "my work passed".
fn author_request(root: &Path, args: &[&str], action: &str) -> Result<Option<String>, String> {
    if let Some(id) = args_of(args, "--id").first() {
        return Ok(Some(id.to_string()));
    }
    if let Some(id) = mine(root, args.contains(&"--latest"))? {
        return Ok(Some(id));
    }
    if terminal_handle().is_none() {
        return Err(format!(
            "review {action}: no terminal handle here (ORCA_TERMINAL_HANDLE is not set), so the queue \
             cannot tell which request is yours.\nPass --id rev-NNNN (the ids are in `cargo xtask review list`), or --latest for the newest request whoever wrote it."
        ));
    }
    // This author simply has nothing in the queue.
    Ok(None)
}

/// The request carrying `handle`, or — with `latest` — the newest one in the queue.
fn mine(root: &Path, latest: bool) -> Result<Option<String>, String> {
    mine_in(&dir(root)?, terminal_handle().as_deref(), latest)
}

fn mine_in(d: &Path, handle: Option<&str>, latest: bool) -> Result<Option<String>, String> {
    let all = requests_in(d)?;
    if let Some(handle) = handle {
        let found = all.iter().rev().find(|(_, v)| v["author"]["terminal"].as_str() == Some(handle)).map(|(id, _)| id.clone());
        if found.is_some() {
            return Ok(found);
        }
    }
    Ok(if latest { all.last().map(|(id, _)| id.clone()) } else { None })
}

fn wait(root: &Path, args: &[&str]) -> Result<(), String> {
    let id = author_request(root, args, "wait")?.ok_or("review wait: no request of yours in the queue; submit one first")?;
    let limit = args_of(args, "--timeout").first().and_then(|s| s.parse::<u64>().ok()).unwrap_or(600);
    let deadline = Instant::now() + Duration::from_secs(limit);
    let started = Instant::now();
    loop {
        let v = load(root, &id)?;
        if settled(v["status"].as_str()) {
            println!("A verdict arrived after {} s.", started.elapsed().as_secs());
            println!();
            print_status(&id, &v);
            return Ok(());
        }
        if Instant::now() >= deadline {
            let status = load(root, &id).ok().and_then(|v| v["status"].as_str().map(|s| s.to_string())).unwrap_or_else(|| "pending".into());
            println!("No verdict on {id} after {limit} s; it is still {status}.");
            println!("This is a checkpoint, not a failure: carry on with other work and check again with");
            println!("`cargo xtask review status {id}`.");
            return Ok(());
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

// ——— who reviews ——————————————————————————————————————————————————————————————

/// The reviewer `submit` notifies, if one was set: a terminal handle and a name for the messages.
fn configured_reviewer(root: &Path) -> Option<(String, String)> {
    let d = dir(root).ok()?;
    let text = std::fs::read_to_string(d.join("reviewer.json")).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    let handle = v["handle"].as_str()?.to_string();
    let name = v["name"].as_str().unwrap_or("the reviewer").to_string();
    Some((handle, name))
}

fn reviewer(root: &Path, args: &[&str]) -> Result<(), String> {
    let handle = args_of(args, "--terminal").first().map(|s| s.to_string());
    match handle {
        None => match configured_reviewer(root) {
            Some((h, n)) => {
                println!("The reviewer is {n} ({h}). `submit` notifies it automatically.");
                println!("Change it with `cargo xtask review reviewer --terminal HANDLE --name NAME`.");
            }
            None => {
                println!("No reviewer is set: `submit` writes the request and tells nobody.");
                println!("Set one with `cargo xtask review reviewer --terminal HANDLE --name NAME`");
                println!("(an agent's handle is in its environment as ORCA_TERMINAL_HANDLE).");
            }
        },
        Some(h) => {
            if !h.starts_with("term_") {
                return Err(format!("review reviewer: `{h}` is not a terminal handle (they start with term_)"));
            }
            let name = args_of(args, "--name").first().map(|s| s.to_string()).unwrap_or_else(|| "the reviewer".into());
            let d = dir(root)?;
            let body = json!({ "handle": h, "name": name, "set_at": timestamp() });
            let text = serde_json::to_string_pretty(&body).map_err(|e| format!("review: {e}"))?;
            std::fs::write(d.join("reviewer.json"), format!("{text}\n")).map_err(|e| format!("review: {e}"))?;
            println!("The reviewer is {name} ({h}); `submit` notifies it from now on.");
        }
    }
    Ok(())
}

// ——— talking to Orca —————————————————————————————————————————————————————————

/// This agent's terminal handle, when it is running inside Orca.
fn terminal_handle() -> Option<String> {
    std::env::var("ORCA_TERMINAL_HANDLE").ok().filter(|h| h.starts_with("term_"))
}

/// Send a message to another agent's terminal, best effort: the queue is the record, this is the
/// knock on the door. A machine without Orca simply gets the queue.
fn notify(handle: &str, subject: &str, body: &str) -> String {
    let body: String = body.chars().take(NOTICE_CAP).collect();
    let mut c = Command::new("orca");
    c.args(["orchestration", "send", "--to", handle, "--type", "status", "--subject", subject, "--body", &body]);
    if let Some(from) = terminal_handle() {
        c.args(["--from", &from]);
    }
    match c.output() {
        Ok(o) if o.status.success() => format!("Notified {handle}."),
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr);
            format!("Could not notify {handle} ({}); the request is in the queue.", first_line(&err))
        }
        Err(_) => format!("Could not notify {handle} (no Orca on PATH); the request is in the queue."),
    }
}

// ——— small things ————————————————————————————————————————————————————————————

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git").args(args).current_dir(root).output().map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git {}: {}", args.join(" "), first_line(&err)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn git_opt(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).current_dir(root).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

/// The values given to a flag, in order: `--note a --note b`, or `--note=a`.
fn args_of<'a>(args: &'a [&'a str], name: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = args[i];
        if a == name {
            if let Some(v) = args.get(i + 1) {
                out.push(*v);
            }
            i += 2;
            continue;
        }
        if let Some(v) = a.strip_prefix(name).and_then(|r| r.strip_prefix('=')) {
            out.push(v);
        }
        i += 1;
    }
    out
}

/// The first word that is not a flag or a flag's value: the request id.
fn positional<'a>(args: &'a [&'a str]) -> Option<&'a str> {
    let mut i = 0;
    while i < args.len() {
        let a = args[i];
        if VALUE_FLAGS.contains(&a) {
            i += 2;
            continue;
        }
        if a.starts_with("--") {
            i += 1;
            continue;
        }
        return Some(a);
    }
    None
}

fn short(commit: &str) -> String {
    commit.chars().take(7).collect()
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").trim().to_string()
}

fn first_lines(s: &str, n: usize) -> String {
    s.lines().take(n).collect::<Vec<_>>().join("\n")
}

/// Seconds since the Unix epoch, for arithmetic on claims.
fn unix_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// UTC, RFC 3339: enough to sort and to read, without a date crate.
fn timestamp() -> String {
    let secs = unix_secs() as i64;
    let (y, m, d, hh, mm, ss) = civil_from_unix(secs);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Unix seconds → (year, month, day, hour, minute, second), UTC (Hinnant's days_from_civil, inverted).
fn civil_from_unix(secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d, (rem / 3600) as u32, ((rem % 3600) / 60) as u32, (rem % 60) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A queue directory that removes itself when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let dir = std::env::temp_dir().join(format!(
                "quill-review-test-{name}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_taken_id_is_refused_and_the_next_agent_gets_the_next_one() {
        let dir = Scratch::new("ids");
        let d = dir.path();
        assert_eq!(next_id_in(d).unwrap(), "rev-0001");
        assert!(!store_new_in(d, "rev-0001", &json!({ "id": "rev-0001", "title": "first" })).unwrap());
        // A second agent that computed the same id is told it is taken, and the first record stands.
        assert!(store_new_in(d, "rev-0001", &json!({ "id": "rev-0001", "title": "second" })).unwrap());
        let kept = std::fs::read_to_string(d.join("rev-0001.json")).unwrap();
        assert!(kept.contains("first") && !kept.contains("second"), "{kept}");
        // So the id walks on, and the queue keeps both records.
        assert_eq!(next_id_in(d).unwrap(), "rev-0002");
        assert!(!store_new_in(d, "rev-0002", &json!({ "id": "rev-0002", "title": "second" })).unwrap());
        assert_eq!(next_id_in(d).unwrap(), "rev-0003");
        assert_eq!(requests_in(d).unwrap().len(), 2);
    }

    #[test]
    fn without_a_terminal_handle_the_queue_does_not_pick_a_request_for_you() {
        let dir = Scratch::new("mine");
        let d = dir.path();
        assert!(!store_new_in(d, "rev-0001", &json!({ "id": "rev-0001", "author": { "terminal": "term_aaaa" } })).unwrap());
        assert!(!store_new_in(d, "rev-0002", &json!({ "id": "rev-0002", "author": { "terminal": "term_bbbb" } })).unwrap());
        // No handle: nobody is identified, so nothing is ours — even though the queue is not empty.
        assert_eq!(mine_in(d, None, false).unwrap(), None);
        // `--latest` is the explicit way to say "the newest, whoever wrote it".
        assert_eq!(mine_in(d, None, true).unwrap().as_deref(), Some("rev-0002"));
        // A handle picks its own request rather than the newest.
        assert_eq!(mine_in(d, Some("term_aaaa"), false).unwrap().as_deref(), Some("rev-0001"));
        // An author with nothing in the queue gets nothing, unless it asks for the latest.
        assert_eq!(mine_in(d, Some("term_cccc"), false).unwrap(), None);
        assert_eq!(mine_in(d, Some("term_cccc"), true).unwrap().as_deref(), Some("rev-0002"));
    }

    #[test]
    fn timestamps_are_rfc3339_and_sort() {
        assert_eq!(timestamp().len(), 20);
        assert!(timestamp().ends_with('Z'));
        // 2026-10-23T00:00:00Z
        assert_eq!(civil_from_unix(1_792_713_600), (2026, 10, 23, 0, 0, 0));
    }

    #[test]
    fn flags_are_parsed_in_both_forms() {
        let a = ["--note", "a b", "--gate=cargo test: pass", "rev-0003", "--title", "t"];
        assert_eq!(args_of(&a, "--note"), vec!["a b"]);
        assert_eq!(args_of(&a, "--gate"), vec!["cargo test: pass"]);
        assert_eq!(positional(&a), Some("rev-0003"));
        assert_eq!(positional(&["--note", "x"]), None);
    }

    #[test]
    fn request_ids_sort_by_age() {
        assert!(number("rev-0002") < number("rev-0010"));
        assert_eq!(number("garbage"), u64::MAX);
    }

    #[test]
    fn a_claim_is_not_a_verdict() {
        assert!(!settled(Some("pending")));
        assert!(!settled(Some("reviewing")));
        assert!(settled(Some("accepted")));
        assert!(settled(Some("changes")));
        assert!(!settled(None));
    }

    #[test]
    fn a_claim_ages_and_a_missing_one_is_ancient() {
        let claimed = json!({ "claim": { "since": 1_000 } });
        assert_eq!(claim_age(&claimed, 1_600), 600);
        assert_eq!(claim_age(&claimed, 900), 0);
        assert!(claim_age(&json!({}), 5) >= CLAIM_TTL);
    }

    #[test]
    fn the_scan_finds_a_panic_path_in_a_diff() {
        // Context lines carry a leading space, exactly as git writes them.
        let patch = [
            "diff --git a/crates/doc/src/edit.rs b/crates/doc/src/edit.rs",
            "--- a/crates/doc/src/edit.rs",
            "+++ b/crates/doc/src/edit.rs",
            "@@ -10,3 +10,5 @@",
            " fn insert(doc: &mut Doc) {",
            "+    let r = doc.find(3).unwrap();",
            "+    let s = r.expect(\"a run\");",
            " }",
        ]
        .join("\n");
        let found = patch_findings(&patch);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found[0].starts_with("crates/doc/src/edit.rs:11 "), "{:?}", found[0]);
        assert!(found[0].contains("unwrap"));
        assert!(found[1].contains("expect"));
    }

    #[test]
    fn the_scan_allows_tests_comments_and_removals() {
        let patch = [
            "+++ b/crates/engine/src/tests.rs",
            "@@ -1,1 +1,1 @@",
            "+    f().unwrap();",
            "+++ b/crates/doc/src/edit.rs",
            "@@ -1,2 +1,3 @@",
            "-    f().unwrap();",
            "+    // no unwrap() here: the error goes up",
            "+    /// `panic!` is not how this returns",
        ]
        .join("\n");
        assert!(patch_findings(&patch).is_empty(), "{:?}", patch_findings(&patch));
    }

    #[test]
    fn a_mention_in_a_string_or_under_cfg_test_is_not_a_finding() {
        let patch = [
            "+++ b/xtask/src/review.rs",
            "@@ -1,1 +1,6 @@",
            "+    (\".unwrap()\", \"unwrap\"),",
            "+    \"+    (\\\".unwrap()\\\", ...)\",",
            "+#[cfg(test)]",
            "+mod tests {",
            "+    fn t() { f().unwrap(); }",
        ]
        .join("\n");
        assert!(patch_findings(&patch).is_empty(), "{:?}", patch_findings(&patch));
    }

    #[test]
    fn test_paths_are_recognised() {
        assert!(is_test_path("crates/engine/src/tests.rs"));
        assert!(is_test_path("crates/engine/src/tests_typing.rs"));
        assert!(is_test_path("crates/doc/tests/edit.rs"));
        assert!(!is_test_path("crates/doc/src/edit.rs"));
        assert!(!is_test_path("crates/doc/src/contest.rs"));
    }
}

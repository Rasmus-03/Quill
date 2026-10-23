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
  status [--id ID]
                  what happened to my request (the newest one I submitted)
  wait [--id ID] [--timeout SECS]
                  block until the verdict arrives (default 600 s)
  reviewer [--terminal HANDLE] [--name NAME]
                  show or set the reviewer that `submit` notifies

The queue is shared by every worktree of this repository and is never committed: it lives in
`$(git rev-parse --git-common-dir)/wordcraft-review`.
";

/// The actions that take a flag's value, so a bare word can be recognised as the request id.
const VALUE_FLAGS: &[&str] = &["--title", "--note", "--base", "--gate", "--to", "--reviewer", "--terminal", "--name", "--id", "--timeout"];

pub fn run(root: &Path, args: &[&str]) -> Result<(), String> {
    match args.first().copied() {
        Some("submit") => submit(root, &args[1..]),
        Some("list") => list(root, &args[1..]),
        Some("next") => next(root, &args[1..]),
        Some("show") => show(root, &args[1..]),
        Some("accept") => verdict(root, &args[1..], true),
        Some("changes") => verdict(root, &args[1..], false),
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

/// The queue directory, created on demand.
fn dir(root: &Path) -> Result<PathBuf, String> {
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
    let d = dir(root)?;
    let rd = std::fs::read_dir(&d).map_err(|e| format!("review: {}: {e}", d.display()))?;
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

fn store(root: &Path, id: &str, v: &Value) -> Result<(), String> {
    let d = dir(root)?;
    let path = d.join(format!("{id}.json"));
    let text = serde_json::to_string_pretty(v).map_err(|e| format!("review: {id}: {e}"))?;
    std::fs::write(&path, format!("{text}\n")).map_err(|e| format!("review: {}: {e}", path.display()))
}

/// The next free id.
fn new_id(root: &Path) -> Result<String, String> {
    let next = requests(root)?.iter().map(|(id, _)| number(id)).filter(|n| *n != u64::MAX).max().unwrap_or(0) + 1;
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

    let id = new_id(root)?;
    let record = json!({
        "id": id.clone(),
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
    store(root, &id, &record)?;

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
    println!("{} request(s), {pending} pending — {}", all.len(), dir(root)?.display());
    for (id, v) in &all {
        let status = v["status"].as_str().unwrap_or("?");
        let mark = match status {
            "pending" => "•",
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
    if v["status"] != "pending" {
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

// ——— the author's side ————————————————————————————————————————————————————————

fn status(root: &Path, args: &[&str]) -> Result<(), String> {
    let id = match args_of(args, "--id").first() {
        Some(id) => id.to_string(),
        None => match mine(root)? {
            Some(id) => id,
            None => {
                println!("You have no requests in the queue. Submit one with `cargo xtask review submit`.");
                return Ok(());
            }
        },
    };
    let v = load(root, &id)?;
    print_status(&id, &v);
    Ok(())
}

fn print_status(id: &str, v: &Value) {
    let status = v["status"].as_str().unwrap_or("?");
    println!("{id} — {}", v["title"].as_str().unwrap_or(""));
    println!("  status   {status}");
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
        _ => "pending",
    };
    println!("verdict: {verdict}");
}

/// The newest request submitted from this terminal, whatever its state.
fn mine(root: &Path) -> Result<Option<String>, String> {
    let handle = terminal_handle();
    let all = requests(root)?;
    Ok(all
        .iter()
        .rev()
        .find(|(_, v)| match (&handle, v["author"]["terminal"].as_str()) {
            (Some(h), Some(t)) => h == t,
            _ => false,
        })
        .map(|(id, _)| id.clone())
        .or_else(|| all.last().map(|(id, _)| id.clone())))
}

fn wait(root: &Path, args: &[&str]) -> Result<(), String> {
    let id = match args_of(args, "--id").first() {
        Some(id) => id.to_string(),
        None => mine(root)?.ok_or("review wait: no request of yours in the queue; submit one first")?,
    };
    let limit = args_of(args, "--timeout").first().and_then(|s| s.parse::<u64>().ok()).unwrap_or(600);
    let deadline = Instant::now() + Duration::from_secs(limit);
    let started = Instant::now();
    loop {
        let v = load(root, &id)?;
        if v["status"] != "pending" {
            println!("A verdict arrived after {} s.", started.elapsed().as_secs());
            println!();
            print_status(&id, &v);
            return Ok(());
        }
        if Instant::now() >= deadline {
            println!("No verdict on {id} after {limit} s; it is still pending.");
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

/// UTC, RFC 3339: enough to sort and to read, without a date crate.
fn timestamp() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
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
}

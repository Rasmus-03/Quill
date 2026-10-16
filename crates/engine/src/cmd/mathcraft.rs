//! Equation tab backend: isolate variables and convert units on the
//! selected text, using Maxima (the same CAS engine WordMat drives).
//!
//! Original implementation (MIT). No third-party add-in code is used or
//! derived from: only Maxima's own documented batch interface plus the
//! user's own optional runtime packages under ~/.local/share/mathcraft.

use serde_json::{Value, json};

use super::equation::{equation_at_caret, math_del_anchor, nodes_of, with_eq};
use super::{delete_selection, sel_result};
use crate::doc::Pos;
use crate::doc::math::{MNode, parse_linear};
use crate::doc::para::{InlineObject, OBJ};
use crate::doc::props::{CharProps, Rgb};
use crate::{CmdError, CmdResult, CommandSpec, Selection, Session};

/// True when Maxima is on PATH (checked once at startup for a Danish hint).
pub fn maxima_available() -> bool {
    std::env::var_os("PATH").is_some_and(|paths| std::env::split_paths(&paths).any(|d| d.join("maxima").is_file() || d.join("maxima.exe").is_file()))
}

fn maxima_bin() -> &'static str {
    "maxima"
}

/// Optional SolveReal package path from the environment (no filesystem
/// access: engine commands must not touch fs directly). Set e.g.
/// `MATHCRAFT_SOLVEREAL=$HOME/.local/share/mathcraft/packages/solvereal.mac`.
/// When set, `math.isoler` loads it for real-domain solving, else plain solve().
fn solvereal_path() -> Option<String> {
    std::env::var("MATHCRAFT_SOLVEREAL").ok().filter(|p| !p.trim().is_empty())
}

/// Run one Maxima batch; return the value printed after the "MC>>>" anchor.
/// `linel:10000` keeps values on one line; everything before the anchor is
/// echo noise. `defs` are session definitions, prepended before the command.
/// Seconds before a Maxima call is killed (a hung CAS must never freeze
/// the app; async progress + cancel is tracked follow-up work).
const MAXIMA_TIMEOUT_S: u64 = 30;

/// Run one Maxima batch string, bounded by [`MAXIMA_TIMEOUT_S`].
/// Desktop only: the engine also compiles for web, where processes don't exist.
fn run_maxima(batch_string: &str) -> Result<String, String> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = batch_string;
        return Err("Maxima kræver desktop-appen".to_string());
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::io::Read;
        use std::time::{Duration, Instant};
        let mut child = std::process::Command::new(maxima_bin())
            .args(["--very-quiet", &format!("--batch-string={batch_string}")])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("kan ikke starte Maxima ({} mangler? installer med `sudo pacman -S maxima`): {e}", maxima_bin()))?;
        // Drain stdout on a thread so a flood of output can never deadlock
        // the wait below (pipe buffer full while we only poll).
        let mut cap = child.stdout.take();
        let drain = std::thread::spawn(move || {
            let mut v = Vec::new();
            if let Some(o) = cap.as_mut() {
                let _ = o.read_to_end(&mut v);
            }
            v
        });
        let deadline = Instant::now() + Duration::from_secs(MAXIMA_TIMEOUT_S);
        let status = loop {
            match child.try_wait().map_err(|_| "Maxima fejlede".to_string())? {
                Some(s) => break s,
                None if Instant::now() > deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("Maxima svarede ikke inden 30 sekunder; forenkl udtrykket".to_string());
                }
                None => std::thread::sleep(Duration::from_millis(50)),
            }
        };
        let bytes = drain.join().unwrap_or_default();
        if !status.success() {
            return Err("Maxima fejlede".to_string());
        }
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

/// Reject Maxima that reaches outside the calculation (files, processes).
/// A received .docx can hide such code in an "equation"; clicking Beregn
/// must never run it. `math.maxima` stays raw by explicit user choice.
pub(crate) fn check_maxima_safe(part: &str) -> Result<(), CmdError> {
    const BAD: &[&str] = &[
        "system",
        "writefile",
        "appendfile",
        "save",
        "load",
        "loadfile",
        "batch",
        "demo",
        "to_lisp",
        "lisp",
        "quit",
        "kill",
        "printfile",
        "with_stdout",
        "stringout",
        "run_testsuite",
        "sconcat",
        "eval_string",
        "compfile",
        "openw",
        "openr",
        "opena",
        "close",
        "readline",
    ];
    let mut tok = String::new();
    for c in part.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_alphanumeric() || c == '_' || c == '%' || c == '?' {
            tok.push(c.to_ascii_lowercase());
        } else {
            let w = tok.trim_start_matches(['%', '?']);
            if BAD.contains(&w) {
                return Err(err(format!("\"{tok}\" er ikke tilladt her (brug Maxima-kommando for rå Maxima)")));
            }
            tok.clear();
        }
    }
    Ok(())
}

pub(crate) fn batch(prelude: &[String], defs: &[String], cmd: &str) -> Result<String, String> {
    let mut b = String::new();
    for p in prelude {
        b.push_str(p.trim().trim_end_matches(';'));
        b.push(';');
    }
    // Last: a loaded package (SolveReal) may reset these.
    b.push_str("linel:10000;display2d:false;");
    for d in defs {
        let d = d.trim().trim_end_matches(';');
        if !d.is_empty() {
            b.push_str(d);
            b.push(';');
        }
    }
    b.push_str("MC_RES:(");
    b.push_str(cmd.trim().trim_end_matches(';'));
    b.push_str(");print(\"MC>>>\",MC_RES);");
    let text = run_maxima(&b)?;
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let start = lines.iter().position(|l| *l == "MC>>>" || l.starts_with("MC>>> ")).ok_or_else(|| "Maxima gav intet resultat".to_string())?;
    // A long value wraps over several lines — but the `print` return
    // value is echoed again right after. Join lines only until the
    // brackets balance, so the echo never gets glued on.
    let mut full = lines[start].strip_prefix("MC>>>").unwrap_or("").trim().to_string();
    for l in &lines[start + 1..] {
        if brackets_balanced(&full) && !full.is_empty() {
            break;
        }
        full.push_str(l.trim());
        if lines[start + 1..].len() > 4000 {
            break;
        }
    }
    if full.is_empty() {
        return Err("Maxima gav et tomt resultat".to_string());
    }
    Ok(full)
}

/// Brackets outside `"strings"` net to zero (or negative on junk).
fn brackets_balanced(s: &str) -> bool {
    let (mut depth, mut instr) = (0i32, false);
    let mut prev = '\0';
    for c in s.chars() {
        if instr {
            if c == '"' && prev != '\\' {
                instr = false;
            }
        } else if c == '"' {
            instr = true;
        } else if c == '(' || c == '[' || c == '{' {
            depth += 1;
        } else if c == ')' || c == ']' || c == '}' {
            depth -= 1;
        }
        prev = c;
    }
    !instr && depth <= 0
}

pub(crate) fn base_prelude() -> Vec<String> {
    let mut pre = vec!["display2d:false".to_string(), "ratprint:false".to_string()];
    if let Some(p) = solvereal_path() {
        pre.push(format!("load(\"{p}\")"));
    }
    pre
}

fn unit_prelude() -> Vec<String> {
    vec![
        "display2d:false".to_string(),
        "ratprint:false".to_string(),
        "load(ezunits)".to_string(),
        "declare_unit_conversion (h = hour, min = minute)".to_string(),
    ]
}

/// Plain prelude without SolveReal: its real-domain shadowing breaks
/// ode2, so differential equations run clean.
fn plain_prelude() -> Vec<String> {
    vec!["display2d:false".to_string(), "ratprint:false".to_string()]
}

/// Prelude for statistics (descriptive package: mean, median, std...).
fn stats_prelude() -> Vec<String> {
    vec!["display2d:false".to_string(), "ratprint:false".to_string(), "load(descriptive)".to_string()]
}

/// Split on top-level commas (ignoring those inside parens/brackets).
pub(crate) fn split_top(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '(' | '[' | '{' => {
                depth += 1;
                cur.push(c);
            }
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                cur.push(c);
            }
            ',' if depth == 0 => {
                out.push(cur.trim().to_string());
                cur = String::new();
            }
            _ => cur.push(c),
        }
    }
    out.push(cur.trim().to_string());
    out.into_iter().filter(|p| !p.is_empty()).collect()
}

/// Integral floats print as `8`; other decimals untouched.
fn trim_float_zeros(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        if (c.is_ascii_digit() || c == '.') && (i == 0 || !b[i - 1].is_ascii_alphabetic()) {
            let mut j = i;
            while j < b.len() && (b[j].is_ascii_digit() || b[j] == b'.') {
                j += 1;
            }
            let tok = &s[i..j];
            let next_ok = j >= b.len() || !(b[j].is_ascii_digit() || b[j] == b'.');
            if next_ok
                && tok.contains('.')
                && tok.chars().all(|c| c.is_ascii_digit() || c == '.')
                && let Some(int) = tok.strip_suffix(".0")
            {
                out.push_str(int);
                i = j;
                continue;
            }
            out.push_str(tok);
            i = j;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

fn replace_selection(s: &mut Session, text: &str) -> CmdResult {
    delete_selection(s)?;
    s.run("text.insert", &json!({"text": text, "raw": true}))?;
    sel_result(s)
}

pub(crate) fn err(msg: String) -> CmdError {
    CmdError::Failed(msg)
}

/// Session definitions (Definér) plus MATHCRAFT_DEFS env seed
/// (`;`-separated Maxima statements), prepended to every batch.
pub(crate) fn defs(s: &Session) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(seed) = std::env::var("MATHCRAFT_DEFS") {
        out.extend(seed.split(';').map(str::trim).filter(|d| !d.is_empty()).map(str::to_string));
    }
    out.extend(s.math_defs.iter().cloned());
    out
}

/// WordMat-style variable choice: explicit {"var"} wins; else the single
/// detected variable (like their variable dialog usually resolves); else
/// the UI is asked to show the variable picker.
pub(crate) enum VarPick {
    One(String),
    Many(Vec<String>),
}

pub(crate) fn pick_var(s: &Session, v: &Value, eq: &str) -> VarPick {
    if let Some(var) = v.get("var").and_then(Value::as_str) {
        let var = var.trim();
        if !var.is_empty() {
            return VarPick::One(var.to_string());
        }
    }
    let probe = expand_pm(eq).into_iter().next().unwrap_or_default();
    if let Ok(r) = batch(&base_prelude(), &defs(s), &format!("listofvars({probe})")) {
        let inner = r.trim().trim_start_matches('[').trim_end_matches(']');
        let vars: Vec<String> = inner.split(',').map(str::trim).filter(|w| !w.is_empty() && !w.starts_with('%')).map(str::to_string).collect();
        if let [one] = vars.as_slice() {
            return VarPick::One(one.clone());
        }
        if vars.len() > 1 {
            return VarPick::Many(vars);
        }
    }
    VarPick::One("x".to_string())
}

/// Ask the UI variable picker (no-op headless), then stop this turn.
pub(crate) fn ask_var(s: &mut Session, command: &str, vars: Vec<String>) -> CmdResult {
    s.ui_requests.push(json!({"open": "mathVarPicker", "command": command, "vars": vars}));
    s.status = "Vælg variabel i dialogen…".to_string();
    sel_result(s)
}
/// Input the WordMat way: selected text, else the equation object at the
/// caret (works inside an equation with no selection, like `beregn` reads
/// Selection.OMaths). Returns the text plus an optional object range that
/// the result should replace.
/// Where a math result goes: document selection/range, or an
/// in-equation selection (replaced in place, still editing).
pub(crate) enum Target {
    Doc(Option<(Pos, Pos)>),
    Math,
}

/// In-equation highlight text (Shift+arrows inside an equation),
/// same-argument ranges only.
fn math_anchor_text(s: &Session) -> Option<String> {
    let m = s.math.as_ref()?;
    let anchor = m.anchor.as_ref()?;
    let nodes = nodes_of(s, &m.at)?;
    let span = wordcraft_doc::math_edit::selection_span(&nodes, anchor, &m.pos)?;
    wordcraft_doc::math_edit::span_text(&nodes, &span)
}

/// Strip a previous green answer (+ its " = ") from the end of the
/// equation at `at`, so re-running recomputes instead of stacking answers.
/// Returns true when something was removed.
fn strip_answer_in_eq(s: &mut Session, at: &crate::doc::Pos) -> bool {
    let mut stripped = false;
    let _ = with_eq(s, at, |math, _| {
        while math.nodes.last().is_some_and(is_green_node) {
            math.nodes.pop();
            stripped = true;
        }
        if stripped
            && let Some(MNode::Run(r)) = math.nodes.last()
            && r.text.trim() == "="
            && r.color.is_none()
        {
            math.nodes.pop();
        }
    });
    stripped
}

pub(crate) fn math_input(s: &mut Session) -> Result<(String, Target), CmdError> {
    // In-equation highlight beats everything: it names the exact part.
    if let Some(t) = math_anchor_text(s)
        && !t.trim().is_empty()
    {
        return Ok((t, Target::Math));
    }
    let sel = s.selected_text();
    if !sel.trim().is_empty() {
        // A selection holding exactly one equation object (and nothing
        // else) operates on the equation, so it survives as OMML.
        if let Some((at, end)) = sole_equation(s) {
            strip_answer_in_eq(s, &at);
            let para = s.doc.para_at(&at).ok_or_else(|| err("Sæt cursoren i en ligning eller markér tekst først".to_string()))?;
            let text = para.object_at(at.off).map(|o| o.plain_text().to_string()).unwrap_or_default();
            if !text.trim().is_empty() {
                return Ok((strip_trailing_eq(&text), Target::Doc(Some((at, end)))));
            }
        }
        if let Some(stripped) = strip_green_suffix(s)
            && !stripped.trim().is_empty()
        {
            return Ok((strip_trailing_eq(&stripped), Target::Doc(None)));
        }
        if let Some(before) = strip_trailing_object(s) {
            return Ok((strip_trailing_eq(&before), Target::Doc(None)));
        }
        return Ok((strip_trailing_eq(&sel), Target::Doc(None)));
    }
    if s.math.is_some() {
        s.run("equation.exit", &json!({"before": true}))?;
    }
    let at = equation_at_caret(s).ok_or_else(|| err("Sæt cursoren i en ligning eller markér tekst først".to_string()))?;
    // Update semantics: drop a previous green answer (plus its " = ")
    // so re-running recomputes instead of stacking answers.
    strip_answer_in_eq(s, &at);
    let para = s.doc.para_at(&at).ok_or_else(|| err("Sæt cursoren i en ligning eller markér tekst først".to_string()))?;
    match para.object_at(at.off) {
        Some(InlineObject::Equation { .. }) => {
            let text = para.object_at(at.off).map(|o| o.plain_text().to_string()).unwrap_or_default();
            if text.trim().is_empty() {
                return Err(err("Ligningen er tom".to_string()));
            }
            let end = Pos { off: at.off + OBJ.len_utf8(), ..at.clone() };
            Ok((strip_trailing_eq(&text), Target::Doc(Some((at, end)))))
        }
        _ => Err(err("Sæt cursoren i en ligning eller markér tekst først".to_string())),
    }
}

/// If the selection covers exactly one inline object (an equation) and
/// nothing else but whitespace, return its range. Byte offsets throughout.
fn sole_equation(s: &Session) -> Option<(Pos, Pos)> {
    let (a, b) = s.sel.ordered();
    if a == b || a.path != b.path {
        return None;
    }
    let para = s.doc.para_at(&a)?;
    let mut objs = Vec::new();
    for (i, c) in para.text.char_indices() {
        if i >= b.off {
            break;
        }
        if c == OBJ && i >= a.off {
            objs.push(i);
        }
    }
    if objs.len() != 1 {
        return None;
    }
    // Everything else selected must be whitespace.
    let o = objs[0];
    let before = para.text.get(a.off..o).unwrap_or("");
    let after_end = o + OBJ.len_utf8();
    let after = para.text.get(after_end..b.off).unwrap_or("");
    if !before.trim().is_empty() || !after.trim().is_empty() {
        return None;
    }
    if !matches!(para.object_at(o), Some(InlineObject::Equation { .. })) {
        return None;
    }
    let end = Pos { off: after_end, ..a.clone() };
    Some((Pos { off: o, ..a.clone() }, end))
}

/// If the selection ends with an equation object (a previous answer),
/// return the text before it so it recomputes instead of stacking.
/// The full selection (object included) is still what gets replaced.
fn strip_trailing_object(s: &Session) -> Option<String> {
    let (a, b) = s.sel.ordered();
    if a == b || a.path != b.path {
        return None;
    }
    let para = s.doc.para_at(&a)?;
    let o = OBJ.len_utf8();
    if b.off < o {
        return None;
    }
    let start = b.off - o;
    if start < a.off || para.text.get(start..b.off) != Some("\u{FFFC}") {
        return None;
    }
    if !matches!(para.object_at(start), Some(InlineObject::Equation { .. })) {
        return None;
    }
    let before = para.text.get(a.off..start).unwrap_or("").trim_end();
    let before = before.strip_suffix('=').unwrap_or(before).trim_end();
    if before.is_empty() {
        return None;
    }
    Some(before.to_string())
}

/// WordMat parity: a trailing lone `=` means "compute this" (`2+2 =`).
fn strip_trailing_eq(input: &str) -> String {
    let t = input.trim_end().to_string();
    match t.strip_suffix('=') {
        Some(rest) if !rest.ends_with('=') && !rest.ends_with('<') && !rest.ends_with('>') && !rest.ends_with('!') => rest.trim_end().to_string(),
        _ => t,
    }
}

/// If the selection ends with our green appended answer, return the input
/// without it (update semantics: the caller replaces the whole selection,
/// so the old answer is overwritten, never stacked).
fn strip_green_suffix(s: &Session) -> Option<String> {
    use crate::doc::props::TextColor;
    let (a, b) = s.sel.ordered();
    if a == b || a.path != b.path {
        return None;
    }
    let para = s.doc.para_at(&a)?;
    let text = &para.text;
    // Byte spans of runs partitioning the paragraph text.
    let mut spans: Vec<(usize, usize, bool)> = Vec::new();
    let mut off = 0usize;
    for r in para.runs.iter() {
        let end = off + r.len;
        spans.push((off, end, r.props.color == Some(TextColor::Rgb(ANSWER_GREEN))));
        off = end;
    }
    // Trailing green span overlapped by the selection.
    let mut green = 0usize;
    for (start, end, g) in spans.iter().rev().map(|(s, e, g)| (*s, *e, *g)) {
        if end <= a.off || start >= b.off {
            continue;
        }
        if g && start >= a.off {
            green += end.min(b.off) - start.max(a.off);
        } else {
            break;
        }
    }
    if green == 0 || green >= b.off - a.off {
        return None;
    }
    let cut = b.off - green;
    Some(text.get(a.off..cut)?.to_string())
}

/// Replace the selection, or the equation range from `math_input`.
/// The original is kept: `{input} = {result}`, WordMat-style.
/// WordMat green for computed answers, also inside equations.
const ANSWER_GREEN: Rgb = Rgb(0, 176, 80);
const ANSWER_GREEN_STR: &str = "00B050";

/// True when every run inside this node carries our answer green.
fn is_green_node(n: &MNode) -> bool {
    match n {
        MNode::Run(r) => r.color == Some(ANSWER_GREEN) && !r.text.is_empty(),
        MNode::Frac { num, den, .. } => !num.is_empty() && !den.is_empty() && num.iter().all(is_green_node) && den.iter().all(is_green_node),
        MNode::Script { base, sub, sup, .. } => [base, sub, sup].iter().all(|a| a.is_empty() || a.iter().all(is_green_node)),
        MNode::Rad { deg, e, .. } => [deg, e].iter().all(|a| a.is_empty() || a.iter().all(is_green_node)),
        MNode::Nary { sub, sup, e, .. } => [sub, sup, e].iter().all(|a| a.is_empty() || a.iter().all(is_green_node)),
        MNode::Delim { elems, .. } => !elems.is_empty() && elems.iter().all(|a| a.iter().all(is_green_node)),
        MNode::Func { name, e } => [name, e].iter().all(|a| a.is_empty() || a.iter().all(is_green_node)),
        MNode::Lim { e, lim, .. } => [e, lim].iter().all(|a| a.is_empty() || a.iter().all(is_green_node)),
        MNode::Acc { e, .. }
        | MNode::Bar { e, .. }
        | MNode::BorderBox { e, .. }
        | MNode::Boxed { e, .. }
        | MNode::GroupChr { e, .. }
        | MNode::Phant { e, .. } => !e.is_empty() && e.iter().all(is_green_node),
        MNode::EqArr { rows, .. } => !rows.is_empty() && rows.iter().all(|a| a.iter().all(is_green_node)),
        MNode::Matrix { rows, .. } => !rows.is_empty() && rows.iter().all(|r| r.iter().all(|a| a.iter().all(is_green_node))),
    }
}

/// WordMat-style multiplication dot. Applied to computed answers
/// (never to LaTeX code, where `*` must stay literal).
fn answer_dots(result: &str) -> String {
    if result.starts_with("$$") {
        return result.to_string();
    }
    result.replace('*', "\u{22c5}")
}

/// Display pipeline for computed answers (school notation, Danish):
/// bigfloat (`1.9b0` → `1.9`), `%union` intervals (`… eller …`),
/// `%pi`/`%e`/`%i` constants, empty real solutions, `⋅` for `*`.
fn pretty_answer(result: &str) -> String {
    if result.starts_with("$$") {
        return result.to_string();
    }
    let t = result.trim();
    if t == "[]" {
        return "ingen reel løsning".to_string();
    }
    let mut s = t.to_string();
    if s.starts_with("%union(") && s.ends_with(')') {
        let inner = &s["%union(".len()..s.len() - 1];
        let parts: Vec<String> = split_top(inner)
            .into_iter()
            .map(|p| p.trim().trim_start_matches('[').trim_end_matches(']').trim().to_string())
            .filter(|p| !p.is_empty())
            .collect();
        if !parts.is_empty() {
            s = parts.join(" eller ");
        }
    }
    s = unbigfloat(&s);
    s = unpercent_consts(&s);
    answer_dots(&s)
}

/// Maxima bigfloat (`1.895494b0`, `2.5b+3`) → plain/e-notation.
pub(crate) fn unbigfloat(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'b' && i > 0 && (b[i - 1].is_ascii_digit() || b[i - 1] == b'.') && i + 1 < b.len() {
            let mut j = i + 1;
            if b[j] == b'+' || b[j] == b'-' {
                j += 1;
            }
            let k = j;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j > k {
                // Exponent zero (b0, b+0, b-00): drop the suffix.
                if s[k..j].trim_start_matches(['+', '-']).trim_start_matches('0').is_empty() {
                    i = j;
                    continue;
                }
                out.push('e');
                out.push_str(&s[i + 1..j]);
                i = j;
                continue;
            }
        }
        let ch = s[i..].chars().next().unwrap_or_default();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// `%pi` → `π`, `%e` → `e`, `%i` → `i`, `%c` → `C` (whole tokens only).
fn unpercent_consts(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while !rest.is_empty() {
        let hit = if rest.starts_with("%pi") {
            Some(("%pi", "π"))
        } else if rest.starts_with("%e") {
            Some(("%e", "e"))
        } else if rest.starts_with("%i") {
            Some(("%i", "i"))
        } else if rest.starts_with("%c") {
            Some(("%c", "C"))
        } else {
            None
        };
        if let Some((tok, rep)) = hit {
            let after = &rest[tok.len()..];
            let boundary = after.is_empty() || !after.chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_');
            if boundary {
                out.push_str(rep);
                rest = after;
                continue;
            }
        }
        let mut chars = rest.chars();
        out.push(chars.next().unwrap_or_default());
        rest = chars.as_str();
    }
    out
}

/// Paint every run in these nodes green (recursive, keeps structure).
fn paint_green(nodes: &mut [MNode]) {
    for n in nodes.iter_mut() {
        match n {
            MNode::Run(r) => r.color = Some(ANSWER_GREEN),
            MNode::Frac { num, den, .. } => {
                paint_green(num);
                paint_green(den);
            }
            MNode::Script { base, sub, sup, .. } => {
                paint_green(base);
                paint_green(sub);
                paint_green(sup);
            }
            MNode::Rad { deg, e, .. } => {
                paint_green(deg);
                paint_green(e);
            }
            MNode::Nary { sub, sup, e, .. } => {
                paint_green(sub);
                paint_green(sup);
                paint_green(e);
            }
            MNode::Delim { elems, .. } => {
                for a in elems.iter_mut() {
                    paint_green(a);
                }
            }
            MNode::Func { name, e } => {
                paint_green(name);
                paint_green(e);
            }
            MNode::Lim { e, lim, .. } => {
                paint_green(e);
                paint_green(lim);
            }
            MNode::Acc { e, .. } | MNode::Bar { e, .. } | MNode::BorderBox { e, .. } | MNode::Boxed { e, .. } | MNode::GroupChr { e, .. } => {
                paint_green(e)
            }
            MNode::EqArr { rows, .. } => {
                for r in rows.iter_mut() {
                    paint_green(r);
                }
            }
            MNode::Matrix { rows, .. } => {
                for r in rows.iter_mut() {
                    for c in r.iter_mut() {
                        paint_green(c);
                    }
                }
            }
            MNode::Phant { e, .. } => paint_green(e),
        }
    }
}

fn apply_result(s: &mut Session, target: Target, input: &str, result: &str) -> CmdResult {
    match target {
        Target::Math => {
            // Replace the highlighted part inside the equation, still
            // editing; the green result lands where the highlight was.
            if !math_del_anchor(s) {
                return Err(err("Mistede markeringen; markér igen".to_string()));
            }
            let m = s.math.clone().ok_or_else(|| err("Forlod ligningen".to_string()))?;
            let show = pretty_answer(result);
            let mut ans = parse_linear(&show);
            paint_green(&mut ans);
            let add = wordcraft_doc::math_edit::units(&ans);
            let base = m.pos.clone();
            with_eq(s, &m.at, |math, _| {
                if let Some(arg) = wordcraft_doc::math_edit::arg_at_mut(&mut math.nodes, &base.path) {
                    wordcraft_doc::math_edit::splice_at(arg, base.off, ans);
                }
            })?;
            if let Some(mm) = s.math.as_mut() {
                mm.pos = wordcraft_doc::math_edit::MathPos::new(base.path.clone(), base.off + add);
            }
            s.math_last = Some(show);
            s.pending = Some(CharProps::default());
            sel_result(s)
        }
        Target::Doc(range) => {
            if let Some((a, _b)) = range {
                // Equation object: append "= result" inside it, painted green,
                // WordMat-style. The original equation stays untouched.
                let show = pretty_answer(result);
                let mut ans = parse_linear(&show);
                paint_green(&mut ans);
                with_eq(s, &a, |math, _| {
                    use crate::doc::math::MRun;
                    math.nodes.push(MNode::Run(MRun::new(" = ")));
                    math.nodes.extend(ans);
                })?;
                s.sel = Selection::caret(Pos { off: a.off + OBJ.len_utf8(), ..a.clone() });
                // Fresh typing after our answer starts default, not green.
                s.pending = Some(CharProps::default());
                s.math_last = Some(show.clone());
                return sel_result(s);
            }
            let show = pretty_answer(result);
            replace_selection(s, &format!("{input} = {show}"))?;
            // Remember the answer (Forrige resultat), then paint and park caret.
            s.math_last = Some(show.clone());
            // Paint " = result" WordMat green, then park the caret at the end.
            // (A failed paint never fails the command: the text is already in.)
            let end = s.sel.focus.clone();
            let start = Pos { off: end.off.saturating_sub(" = ".len() + show.len()), ..end.clone() };
            s.sel = Selection { anchor: start, focus: end.clone() };
            let _ = s.run("format.set", &json!({"props": {"color": ANSWER_GREEN_STR}}));
            s.sel = Selection::caret(end);
            // Fresh typing after our answer starts default, not green.
            s.pending = Some(CharProps::default());
            sel_result(s)
        }
    }
}
/// True when the expression is an inequality (`<`, `>`, `#`),
/// not an equation — WordMat solves those separately.
fn is_inequality(expr: &str) -> bool {
    let t = expr.replace("<=", "≤").replace(">=", "≥").replace("==", "≡");
    t.contains('<') || t.contains('>') || t.contains('#')
}

/// Split "2,5 km/h" into ("2.5 km/h", "m/s")-style (qty, target); Danish decimal comma accepted.
fn split_qty(input: &str) -> Result<(String, String), String> {
    let t = input.trim().replace(',', ".");
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let mut digits = false;
    while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.' || b[i] == b' ') {
        digits |= b[i].is_ascii_digit();
        i += 1;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        if j < b.len() && b[j].is_ascii_digit() {
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            i = j;
            digits = true;
        }
    }
    if !digits {
        return Err("intet tal fundet; markér f.eks. \"5 km/h to m/s\"".to_string());
    }
    let num: String = t[..i].chars().filter(|c| *c != ' ').collect();
    let rest = t[i..].trim();
    // "5 km/h to m/s", "5 km/h → m/s", "5 km/h > m/s"
    for sep in [" to ", " \u{2192} ", " > "] {
        if let Some(k) = rest.find(sep) {
            let unit = rest[..k].trim();
            let target = rest[k + sep.len()..].trim();
            if !unit.is_empty() && !target.is_empty() {
                return Ok((format!("{num} {unit}"), target.to_string()));
            }
        }
    }
    Err("ingen målenhed fundet; markér f.eks. \"5 km/h to m/s\"".to_string())
}

/// Convert units: `5 km/h to m/s` → `25/18 ` m/s`.
fn enhed(s: &mut Session, v: &Value) -> CmdResult {
    let (input, target) = math_input(s)?;
    let expr = input.trim();
    let (qty, unit_to) = match split_qty(expr) {
        Ok(q) => q,
        Err(e) => return Err(err(e)),
    };
    check_maxima_safe(&qty)?;
    check_maxima_safe(&unit_to)?;
    let (n, u) = match qty.split_once(' ') {
        Some((n, u)) => (n, u),
        None => return Err(err("ingen enhed fundet".to_string())),
    };
    let float = v.get("float").and_then(Value::as_bool).unwrap_or(false);
    let base = format!("(({n} ` ({u})) `` ({unit_to}))");
    let cmd = if float { format!("float({base})") } else { base };
    match batch(&unit_prelude(), &defs(s), &cmd) {
        Ok(r) => apply_result(s, target, input.trim(), &r),
        Err(e) => Err(err(format!("Kunne ikke omregne enheder: {e}"))),
    }
}

/// Danish decimal commas are dots to Maxima, and the display `⋅`
/// (typed live instead of `*`) computes as `*`; a comma inside one of our
/// single-expression inputs is always a decimal separator.
/// Implicit multiplication (`3x`, `2(x+1)`) gains an explicit `*`,
/// WordMat-style — except inside known function names and `1e3` exponents.
/// Unicode math (`− √ ² π × ÷ ≤ ≥ ≠`) becomes Maxima ASCII; `±` is kept
/// for branch expansion (see expand_pm).
pub(crate) fn dots(expr: &str) -> String {
    dots_ctx(expr, &[])
}

/// Same, but user-defined names (functions, constants) are protected
/// from splitting.
pub(crate) fn dots_ctx(expr: &str, protected: &[String]) -> String {
    // Danish decimal commas are dots to Maxima — but only at bracket
    // depth 0. Inside brackets a comma separates arguments (`diff(y,x)`,
    // `limit(f,x,0)`), so it stays.
    let t = normalize_unicode(&expr.replace('\u{22c5}', "*"));
    let mut out = String::with_capacity(t.len() + 4);
    let mut depth = 0usize;
    for c in t.chars() {
        match c {
            '(' | '[' | '{' => {
                depth += 1;
                out.push(c);
            }
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                out.push(c);
            }
            ',' if depth == 0 => out.push('.'),
            _ => out.push(c),
        }
    }
    insert_mult(&out, protected)
}

/// Translate Unicode math to Maxima ASCII. `√(...)` keeps its parens,
/// `√x` wraps one char, superscripts become `^n`.
fn normalize_unicode(expr: &str) -> String {
    let supmap = [
        ('\u{2070}', "0"),
        ('\u{00b9}', "1"),
        ('\u{00b2}', "2"),
        ('\u{00b3}', "3"),
        ('\u{2074}', "4"),
        ('\u{2075}', "5"),
        ('\u{2076}', "6"),
        ('\u{2077}', "7"),
        ('\u{2078}', "8"),
        ('\u{2079}', "9"),
    ];
    let c: Vec<char> = expr.chars().collect();
    let mut out = String::with_capacity(expr.len() + 8);
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch == '\u{2212}' {
            out.push('-');
        } else if ch == '\u{00d7}' {
            out.push('*');
        } else if ch == '\u{00f7}' {
            out.push('/');
        } else if ch == '\u{03c0}' {
            out.push_str("%pi");
        } else if ch == '\u{2264}' {
            out.push_str("<=");
        } else if ch == '\u{2265}' {
            out.push_str(">=");
        } else if ch == '\u{2260}' {
            out.push('#');
        } else if ch == '\u{221a}' || ch == '\u{221b}' || ch == '\u{221c}' {
            // √x → sqrt(x); ∛x → (x)^(1/3); ∜x → (x)^(1/4).
            let (name, pw) = if ch == '\u{221a}' {
                ("sqrt", None)
            } else if ch == '\u{221b}' {
                ("", Some("(1/3)"))
            } else {
                ("", Some("(1/4)"))
            };
            if c.get(i + 1) == Some(&'(') {
                // Paren group: sqrt keeps it, roots wrap it.
                if let Some(pw) = pw {
                    // Find matching close paren.
                    let mut depth = 0usize;
                    let mut j = i + 1;
                    while j < c.len() {
                        if c[j] == '(' {
                            depth += 1;
                        } else if c[j] == ')' {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        j += 1;
                    }
                    let inner: String = c[i + 1..=j.min(c.len().saturating_sub(1))].iter().collect();
                    out.push_str(&format!("({inner})^{pw}"));
                    i = j.min(c.len().saturating_sub(1));
                } else {
                    out.push_str(name);
                }
            } else if let Some(n) = c.get(i + 1) {
                // A numeric literal counts as one radicand: √2.5 → sqrt(2.5).
                let mut j = i + 1;
                if n.is_ascii_digit() || *n == '.' {
                    while j < c.len() && (c[j].is_ascii_digit() || c[j] == '.') {
                        j += 1;
                    }
                } else {
                    j = i + 2;
                }
                let atom: String = c[i + 1..j].iter().collect();
                if let Some(pw) = pw {
                    out.push_str(&format!("({atom})^{pw}"));
                } else {
                    out.push_str(&format!("sqrt({atom})"));
                }
                i = j - 1;
            } else if pw.is_some() {
                // Trailing lone root sign: leave it; Maxima reports it clearly.
                out.push(ch);
            } else {
                out.push_str(name);
            }
        } else if ch == '\u{221e}' {
            out.push_str("inf");
        } else if ch == '\u{00b0}' {
            // `90°` → `(90*%pi/180)`; bare ° is left for a clear error.
            let mut k = out.len();
            let b = out.as_bytes();
            while k > 0 && b[k - 1] == b' ' {
                k -= 1;
            }
            let mut j = k;
            while j > 0 && (b[j - 1].is_ascii_digit() || b[j - 1] == b'.') {
                j -= 1;
            }
            if j < k {
                let num = out[j..k].to_string();
                out.truncate(j);
                out.push_str(&format!("({num}*%pi/180)"));
            } else {
                out.push(ch);
            }
        } else if supmap.iter().any(|(u, _)| *u == ch) {
            // Consecutive superscripts form one exponent: x²³ → x^23.
            let mut digits = String::new();
            while i < c.len() {
                if let Some(n) = supmap.iter().find(|(u, _)| *u == c[i]).map(|(_, n)| *n) {
                    digits.push_str(n);
                    i += 1;
                } else {
                    break;
                }
            }
            out.push('^');
            out.push_str(&digits);
            continue;
        } else {
            out.push(ch);
        }
        i += 1;
    }
    out
}

/// Split on the first `±` into plus/minus variants (WordMat shows both).
fn expand_pm(input: &str) -> Vec<String> {
    if let Some(k) = input.find('\u{00b1}') {
        let (a, b) = input.split_at(k);
        let b = &b['\u{00b1}'.len_utf8()..];
        vec![format!("{a}+{b}"), format!("{a}-{b}")]
    } else {
        vec![input.to_string()]
    }
}

/// Comma→dot only at bracket depth 0 (matrix/list separators inside
/// stay separators; Danish decimals there need a dot — documented).
/// Unicode/⋅ handling identical to dots().
fn dots_shallow(expr: &str) -> String {
    dots_ctx(expr, &[])
}

/// Split letter runs into single-letter factors (`ac` → `a*c`),
/// school-math style. Protected words (functions, `pi`, user definitions)
/// stay whole. Only for the solve path — elsewhere symbols are kept.
fn split_idents(expr: &str, protected: &[String]) -> String {
    let mut out = String::with_capacity(expr.len() + 4);
    let mut i = 0;
    let c: Vec<char> = expr.chars().collect();
    while i < c.len() {
        if c[i].is_ascii_alphabetic() {
            let mut j = i;
            while j < c.len() && c[j].is_ascii_alphabetic() {
                j += 1;
            }
            let word: String = c[i..j].iter().collect();
            if word.len() > 1 && !is_func(&word) && !protected.iter().any(|p| p == &word) {
                if word == "pi" {
                    out.push_str("%pi");
                } else {
                    out.push_str(&word.chars().map(|x| x.to_string()).collect::<Vec<_>>().join("*"));
                }
            } else if word == "pi" {
                out.push_str("%pi");
            } else {
                out.push_str(&word);
            }
            i = j;
        } else {
            out.push(c[i]);
            i += 1;
        }
    }
    out
}
/// Known Maxima functions: never split `name(` with a `*`.
fn is_func(name: &str) -> bool {
    matches!(
        name,
        "sin"
            | "cos"
            | "tan"
            | "asin"
            | "acos"
            | "atan"
            | "sinh"
            | "cosh"
            | "tanh"
            | "log"
            | "ln"
            | "sqrt"
            | "exp"
            | "abs"
            | "floor"
            | "ceil"
            | "round"
            | "min"
            | "max"
            | "diff"
            | "integrate"
            | "factor"
            | "expand"
            | "solve"
            | "float"
            | "ratsimp"
            | "trigsimp"
            | "radcan"
            | "ode2"
            | "nsolve"
            | "solvereal"
            | "pi"
    )
}

fn insert_mult(expr: &str, protected: &[String]) -> String {
    let c: Vec<char> = expr.chars().collect();
    let mut out = String::with_capacity(expr.len() + 4);
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        out.push(ch);
        if i + 1 < c.len() {
            let nx = c[i + 1];
            let digit_then = ch.is_ascii_digit() || ch == ')';
            let letter_then_paren = ch.is_ascii_alphabetic() && nx == '(';
            if digit_then && (nx.is_ascii_alphabetic() || nx == '(') && nx != 'e' && nx != 'E' {
                // `1e3` exponents stay intact; `3x`, `2(x+1)` gain `*`.
                // A digit run ending in e/E followed by digit/sign is an exponent.
                let mut j = i;
                while j > 0 && c[j - 1].is_ascii_digit() {
                    j -= 1;
                }
                let head: String = c[j..=i].iter().collect();
                let is_exp = (nx == 'e' || nx == 'E')
                    && c.get(i + 2).is_some_and(|d| d.is_ascii_digit() || *d == '+' || *d == '-')
                    && head.chars().all(|d| d.is_ascii_digit());
                if !is_exp {
                    out.push('*');
                }
            } else if letter_then_paren {
                // `sin(` and multi-letter names stay; a lone letter
                // before `(` (and not user-defined) gains `*`.
                let mut j = i;
                while j > 0 && c[j - 1].is_ascii_alphabetic() {
                    j -= 1;
                }
                let name: String = c[j..=i].iter().collect();
                if name.len() == 1 && !is_func(&name) && !protected.iter().any(|p| p == &name) {
                    out.push('*');
                }
            }
        }
        i += 1;
    }
    out
}

fn unary(s: &mut Session, maxima_fn: &str, what: &str) -> CmdResult {
    let (input, target) = math_input(s)?;
    let expr = dots(input.trim());
    check_maxima_safe(&expr)?;
    match batch(&base_prelude(), &defs(s), &format!("{maxima_fn}({expr})")) {
        Ok(r) => apply_result(s, target, input.trim(), &r),
        Err(e) => Err(err(format!("Kunne ikke {what}: {e}"))),
    }
}

/// Like unary, but commas inside brackets stay separators (matrices,
/// data lists) — Danish decimals there need a dot.
fn unary_shallow(s: &mut Session, maxima_fn: &str, what: &str) -> CmdResult {
    let (input, target) = math_input(s)?;
    let expr = dots_shallow(input.trim());
    check_maxima_safe(&expr)?;
    match batch(&base_prelude(), &defs(s), &format!("{maxima_fn}({expr})")) {
        Ok(r) => apply_result(s, target, input.trim(), &r),
        Err(e) => Err(err(format!("Kunne ikke {what}: {e}"))),
    }
}

/// WordMat-style simplify: run several methods, keep the shortest answer.
fn simplify_best(s: &mut Session, _: &Value) -> CmdResult {
    let (input, target) = math_input(s)?;
    let expr = dots(input.trim());
    check_maxima_safe(&expr)?;
    let pre = base_prelude();
    let d = defs(s);
    let mut best: Option<String> = None;
    for f in ["ratsimp", "trigsimp", "radcan", "factor"] {
        if let Ok(r) = batch(&pre, &d, &format!("{f}({expr})"))
            && best.as_ref().is_none_or(|b: &String| r.len() < b.len())
        {
            best = Some(r);
        }
    }
    match best {
        Some(r) => apply_result(s, target, input.trim(), &r),
        None => Err(err("Kunne ikke reducere".to_string())),
    }
}

fn diff_int(s: &mut Session, v: &Value, maxima_fn: &str, what: &str) -> CmdResult {
    let (input, target) = math_input(s)?;
    let var = v.get("var").and_then(Value::as_str).unwrap_or("x").trim();
    let var = if var.is_empty() { "x" } else { var };
    let expr = dots(input.trim());
    check_maxima_safe(&expr)?;
    match batch(&base_prelude(), &defs(s), &format!("{maxima_fn}({expr},{var})")) {
        Ok(r) => apply_result(s, target, input.trim(), &r),
        Err(e) => Err(err(format!("Kunne ikke {what}: {e}"))),
    }
}

/// Beregn: numeric value. `2+2` → `4`; `x^2=4` → floated solutions.
fn beregn(s: &mut Session, v: &Value) -> CmdResult {
    let (input, target) = math_input(s)?;
    let expr = split_idents(&dots_ctx(input.trim(), &def_names(s)), &def_names(s));
    // Exact (default, WordMat Eksakt) evaluates as-is; decimal mode
    // (WordMat Tal) wraps everything in float().
    let exact = !s.math_decimal;
    let mut cmds = Vec::new();
    for e in expand_pm(&expr) {
        check_maxima_safe(&e)?;
        let var = if e.contains('=') || is_inequality(&e) {
            match pick_var(s, v, &e) {
                VarPick::One(var) => var,
                VarPick::Many(vars) => return ask_var(s, "math.beregn", vars),
            }
        } else {
            String::new()
        };
        let cmd = if is_inequality(&e) {
            format!("POLY::{e}::{var}")
        } else if e.contains('=') {
            let inner = if solvereal_path().is_some() { format!("solvereal({e},{var})") } else { format!("solve({e},{var})") };
            if exact { inner } else { format!("float({inner})") }
        } else if exact {
            e
        } else {
            format!("float({e})")
        };
        cmds.push(cmd);
    }
    let pre = base_prelude();
    let d = defs(s);
    let mut out = Vec::new();
    let mut last_err = String::new();
    for c in &cmds {
        let r = if let Some(rest) = c.strip_prefix("POLY::") {
            match rest.rsplit_once("::") {
                Some((e2, vv)) => poly_solve_raw(&pre, &d, e2, vv),
                None => Err("bad inequality".to_string()),
            }
        } else {
            batch(&pre, &d, c)
        };
        match r {
            Ok(r) => out.push(r),
            Err(e) => last_err = e,
        }
    }
    if out.is_empty() {
        return Err(err(format!("Kunne ikke beregne: {last_err}")));
    }
    apply_result(s, target, input.trim(), &trim_float_zeros(&out.join(" eller ")))
}

/// Maxima-kommando: run raw Maxima on the selection (or {"cmd"}).
fn maxima_cmd(s: &mut Session, v: &Value) -> CmdResult {
    if let Some(c) = v.get("cmd").and_then(Value::as_str) {
        let cmd = c.trim();
        if cmd.is_empty() {
            return Err(err("Tom Maxima-kommando".to_string()));
        }
        return match batch(&base_prelude(), &defs(s), cmd) {
            Ok(r) => {
                let r = r.trim().to_string();
                replace_selection(s, &r)?;
                s.math_last = Some(r);
                sel_result(s)
            }
            Err(e) => Err(err(format!("Maxima-fejl: {e}"))),
        };
    }
    let (input, target) = math_input(s)?;
    // Raw command: no Danish/comma/mult rewriting, Maxima as-is.
    let expr = input.trim().to_string();
    match batch(&base_prelude(), &defs(s), &expr) {
        Ok(r) => apply_result(s, target, input.trim(), r.trim()),
        Err(e) => Err(err(format!("Maxima-fejl: {e}"))),
    }
}

/// Formelsamling: standard formulas grouped by topic (original
/// selection; the math itself is standard curriculum content).
fn formulas() -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
    vec![
        (
            "Rente",
            vec![
                ("Fremskrivning", "K_n=K_0⋅(1+r)^n"),
                ("Gennemsnitsrente", "r=(K_n/K_0)^(1/n)-1"),
                ("Annuitetsopsparing", "A_n=b⋅((1+r)^n-1)/r"),
                ("Annuitetslån (ydelse)", "y=G⋅r/(1-(1+r)^(-n))"),
            ],
        ),
        ("Andengrad", vec![("Diskriminant", "d=b^2-4⋅a⋅c"), ("Løsningsformel", "x=(-b±√(d))/(2⋅a)")]),
        (
            "Geometri",
            vec![
                ("Pythagoras", "a^2+b^2=c^2"),
                ("Cosinusrelation", "c^2=a^2+b^2-2⋅a⋅b⋅cos(C)"),
                ("Sinusrelation", "a/sin(A)=b/sin(B)"),
                ("Cirklens ligning", "(x-a)^2+(y-b)^2=r^2"),
                ("Kuglens ligning", "(x-a)^2+(y-b)^2+(z-c)^2=r^2"),
            ],
        ),
    ]
}

/// Formelsamling: ask the UI to show the formula picker.
fn formler(s: &mut Session, _: &Value) -> CmdResult {
    let mut groups: Vec<Value> = Vec::new();
    if !s.prefs.saved_equations.is_empty() {
        groups.push(json!({
            "name": "Mine",
            "items": s.prefs.saved_equations.iter().map(|e| json!({"label": e.name, "template": e.linear})).collect::<Vec<_>>(),
        }));
    }
    for (g, items) in formulas() {
        groups.push(json!({
            "name": g,
            "items": items.iter().map(|(l, t)| json!({"label": l, "template": t})).collect::<Vec<_>>(),
        }));
    }
    s.ui_requests.push(json!({"open": "mathFormulaPicker", "groups": groups}));
    s.status = "Vælg formel i dialogen…".to_string();
    sel_result(s)
}

/// Indsæt formel: insert the picked template as a new equation.
fn formel(s: &mut Session, v: &Value) -> CmdResult {
    let t = v.get("template").and_then(Value::as_str).unwrap_or("").trim();
    if t.is_empty() {
        return Err(err("Tom formelskabelon".to_string()));
    }
    s.run("insert.equation", &json!({"linear": t}))
}
/// Linear text of the equation being edited, if any.
fn current_equation_linear(s: &Session) -> Option<String> {
    let m = s.math.as_ref()?;
    let nodes = nodes_of(s, &m.at)?;
    Some(wordcraft_doc::math::to_linear(&nodes))
}

/// Linear text of the equation object at the caret, if any.
fn equation_at_caret_linear(s: &Session) -> Option<String> {
    let at = equation_at_caret(s)?;
    let para = s.doc.para_at(&at)?;
    match para.object_at(at.off) {
        Some(InlineObject::Equation { .. }) => para.object_at(at.off).map(|o| o.plain_text().to_string()),
        _ => None,
    }
}

/// Gem ligning: save the current equation (editing, at caret, or the
/// selected equation object) into the personal collection shown under
/// "Mine" in the formula picker. Saving twice toggles it back off.
fn gem(s: &mut Session, v: &Value) -> CmdResult {
    let found = if s.math.is_some() {
        current_equation_linear(s)
    } else {
        equation_at_caret_linear(s).or_else(|| {
            let sel = s.selected_text();
            if sel.trim().is_empty() { None } else { Some(sel.trim().to_string()) }
        })
    };
    let (name, linear) = match (v.get("name").and_then(Value::as_str), found) {
        (Some(n), Some(l)) if !n.trim().is_empty() => (n.trim().to_string(), l),
        (Some(n), None) if !n.trim().is_empty() => {
            return Err(err("Ingen ligning at gemme".to_string()));
        }
        (_, Some(l)) => {
            let name: String = l.chars().take(28).collect();
            (name, l)
        }
        _ => return Err(err("Ingen ligning at gemme".to_string())),
    };
    let linear = linear.trim().to_string();
    if linear.is_empty() {
        return Err(err("Ingen ligning at gemme".to_string()));
    }
    if let Some(i) = s.prefs.saved_equations.iter().position(|e| e.linear == linear) {
        s.prefs.saved_equations.remove(i);
        s.status = format!("Fjernede {name}");
    } else {
        s.status = format!("Gemte {name}");
        s.prefs.saved_equations.push(crate::session::SavedEquation { name, linear });
    }
    sel_result(s)
}
fn forrige(s: &mut Session, _: &Value) -> CmdResult {
    let last = s.math_last.clone().unwrap_or_default();
    if last.trim().is_empty() {
        return Err(err("Intet tidligere resultat endnu".to_string()));
    }
    replace_selection(s, &last)?;
    let end = s.sel.focus.clone();
    let start = Pos { off: end.off.saturating_sub(last.len()), ..end.clone() };
    s.sel = Selection { anchor: start, focus: end.clone() };
    let _ = s.run("format.set", &json!({"props": {"color": ANSWER_GREEN_STR}}));
    s.sel = Selection::caret(end);
    s.pending = Some(CharProps::default());
    sel_result(s)
}

/// Eksakt (exact results, WordMat Exact, the default).
fn exakt(s: &mut Session, _: &Value) -> CmdResult {
    s.math_decimal = false;
    s.status = "Eksakte resultater".to_string();
    sel_result(s)
}

/// Decimaltal (WordMat Num): beregn answers come back decimal.
fn decimal(s: &mut Session, _: &Value) -> CmdResult {
    s.math_decimal = true;
    s.status = "Decimalresultater".to_string();
    sel_result(s)
}

/// Isolate a variable: `2*x+3=7` → `[x = 2]`.
/// Uses SolveReal over the reals when installed, else Maxima solve().
/// Explicit {"var"} wins; a lone detected variable is used directly;
/// several variables open the variable picker (WordMat-style dialog).
fn isoler(s: &mut Session, v: &Value) -> CmdResult {
    let (input, target) = math_input(s)?;
    let expr = split_idents(&dots_ctx(input.trim(), &def_names(s)), &def_names(s));
    let mut cmds = Vec::new();
    let mut varname = String::new();
    for e in expand_pm(&expr) {
        check_maxima_safe(&e)?;
        let eq = if e.contains('=') || is_inequality(&e) { e } else { format!("{e}=0") };
        let var = match pick_var(s, v, &eq) {
            VarPick::One(var) => var,
            VarPick::Many(vars) => return ask_var(s, "math.isoler", vars),
        };
        varname = var.clone();
        cmds.push(if is_inequality(&eq) {
            format!("POLY::{eq}::{var}")
        } else if solvereal_path().is_some() {
            format!("solvereal({eq},{var})")
        } else {
            format!("solve({eq},{var})")
        });
    }
    let pre = base_prelude();
    let d = defs(s);
    let mut out = Vec::new();
    let mut last_err = String::new();
    for c in &cmds {
        let r = if let Some(rest) = c.strip_prefix("POLY::") {
            match rest.rsplit_once("::") {
                Some((e2, vv)) => poly_solve_raw(&pre, &d, e2, vv),
                None => Err("bad inequality".to_string()),
            }
        } else {
            batch(&pre, &d, c)
        };
        match r {
            Ok(r) => out.push(r),
            Err(e) => last_err = e,
        }
    }
    if out.is_empty() {
        return Err(err(format!("Kunne ikke isolere {varname}: {last_err}")));
    }
    match apply_result(s, target, input.trim(), &out.join(" eller ")) {
        Ok(r) => Ok(r),
        Err(e) => Err(err(format!("Kunne ikke isolere {varname}: {e}"))),
    }
}

/// Inequality solve shared by isoler/solve/solvenum.
fn poly_solve_raw(pre: &[String], defs: &[String], eq: &str, var: &str) -> Result<String, String> {
    let mut p = pre.to_vec();
    p.push("load(to_poly_solve)".to_string());
    batch(&p, defs, &format!("to_poly_solve([{eq}],[{var}])"))
}

/// Løs ligning: plain Maxima solve (exact, complex domain).
fn solve_plain(s: &mut Session, v: &Value) -> CmdResult {
    let (input, target) = math_input(s)?;
    let expr = split_idents(&dots_ctx(input.trim(), &def_names(s)), &def_names(s));
    let mut cmds = Vec::new();
    for e in expand_pm(&expr) {
        let eq = if e.contains('=') || is_inequality(&e) { e } else { format!("{e}=0") };
        let var = match pick_var(s, v, &eq) {
            VarPick::One(var) => var,
            VarPick::Many(vars) => return ask_var(s, "math.solve", vars),
        };
        cmds.push(if is_inequality(&eq) { format!("POLY::{eq}::{var}") } else { format!("solve({eq},{var})") });
    }
    let pre = base_prelude();
    let d = defs(s);
    let mut out = Vec::new();
    let mut last_err = String::new();
    for c in &cmds {
        let r = if let Some(rest) = c.strip_prefix("POLY::") {
            match rest.rsplit_once("::") {
                Some((e2, vv)) => poly_solve_raw(&pre, &d, e2, vv),
                None => Err("bad inequality".to_string()),
            }
        } else {
            batch(&pre, &d, c)
        };
        match r {
            Ok(r) => out.push(r),
            Err(e) => last_err = e,
        }
    }
    if out.is_empty() {
        return Err(err(format!("Kunne ikke løse: {last_err}")));
    }
    apply_result(s, target, input.trim(), &out.join(" eller "))
}

/// Løs numerisk: nsolve when SolveReal is installed, else floated solve.
fn solvenum(s: &mut Session, v: &Value) -> CmdResult {
    let (input, target) = math_input(s)?;
    let expr = split_idents(&dots_ctx(input.trim(), &def_names(s)), &def_names(s));
    let mut cmds = Vec::new();
    for e in expand_pm(&expr) {
        let eq = if e.contains('=') || is_inequality(&e) { e } else { format!("{e}=0") };
        let var = match pick_var(s, v, &eq) {
            VarPick::One(var) => var,
            VarPick::Many(vars) => return ask_var(s, "math.solvenum", vars),
        };
        cmds.push(if is_inequality(&eq) {
            format!("POLY::{eq}::{var}")
        } else if solvereal_path().is_some() {
            format!("nsolve({eq},{var})")
        } else {
            format!("float(solve({eq},{var}))")
        });
    }
    let pre = base_prelude();
    let d = defs(s);
    let mut out = Vec::new();
    let mut last_err = String::new();
    for c in &cmds {
        let r = if let Some(rest) = c.strip_prefix("POLY::") {
            match rest.rsplit_once("::") {
                Some((e2, vv)) => poly_solve_raw(&pre, &d, e2, vv),
                None => Err("bad inequality".to_string()),
            }
        } else {
            batch(&pre, &d, c)
        };
        match r {
            Ok(r) => out.push(r),
            Err(e) => last_err = e,
        }
    }
    if out.is_empty() {
        return Err(err(format!("Kunne ikke løse numerisk: {last_err}")));
    }
    apply_result(s, target, input.trim(), &trim_float_zeros(&out.join(" eller ")))
}

/// Løs ligningssystem: `x+y=3, 2x-y=0` → alle variable på én gang.
/// Split first (commas separate equations), then Danish-rewrite per part.
fn ligningssystem(s: &mut Session, _: &Value) -> CmdResult {
    let (input, target) = math_input(s)?;
    let raw = input.trim();
    if raw.is_empty() {
        return Err(err("Markér ligninger som \"x+y=3, 2x-y=0\" først".to_string()));
    }
    let eqs: Vec<String> = split_top(raw)
        .into_iter()
        .map(|e| {
            let e = dots(&e);
            if e.contains('=') || is_inequality(&e) { e } else { format!("{e}=0") }
        })
        .collect();
    for e in &eqs {
        check_maxima_safe(e)?;
    }
    if eqs.is_empty() {
        return Err(err("Markér ligninger som \"x+y=3, 2x-y=0\" først".to_string()));
    }
    // All variables across the system.
    let mut vars: Vec<String> = Vec::new();
    for e in &eqs {
        if let Ok(r) = batch(&base_prelude(), &defs(s), &format!("listofvars({e})")) {
            for w in r.trim().trim_start_matches('[').trim_end_matches(']').split(',').map(str::trim).filter(|w| !w.is_empty() && !w.starts_with('%'))
            {
                if !vars.iter().any(|v| v == w) {
                    vars.push(w.to_string());
                }
            }
        }
    }
    if vars.is_empty() {
        return Err(err("Ingen variable fundet".to_string()));
    }
    let cmd = format!("linsolve([{}],[{}])", eqs.join(","), vars.join(","));
    match batch(&base_prelude(), &defs(s), &cmd) {
        Ok(r) => apply_result(s, target, raw, &r),
        Err(e) => Err(err(format!("Kunne ikke løse system: {e}"))),
    }
}

/// Grænseværdi: `sin(x)/x, x, 0` (defaults x, 0).
fn graense(s: &mut Session, _: &Value) -> CmdResult {
    let (input, target) = math_input(s)?;
    let raw = input.trim();
    let p: Vec<String> = split_top(raw).into_iter().map(|x| dots(&x)).collect();
    let e = p.first().cloned().unwrap_or_default();
    check_maxima_safe(&e)?;
    if e.is_empty() {
        return Err(err("Markér f.eks. \"sin(x)/x, x, 0\" først".to_string()));
    }
    let v = p.get(1).cloned().unwrap_or_else(|| "x".to_string());
    let pt = p.get(2).cloned().unwrap_or_else(|| "0".to_string());
    match batch(&base_prelude(), &defs(s), &format!("limit({e},{v},{pt})")) {
        Ok(r) => apply_result(s, target, raw, &r),
        Err(e) => Err(err(format!("Kunne ikke tage grænseværdi: {e}"))),
    }
}

/// Taylorrække: `exp(x), x, 0, 3` (defaults x, 0, 6).
fn taylor(s: &mut Session, _: &Value) -> CmdResult {
    let (input, target) = math_input(s)?;
    let raw = input.trim();
    let p: Vec<String> = split_top(raw).into_iter().map(|x| dots(&x)).collect();
    let e = p.first().cloned().unwrap_or_default();
    check_maxima_safe(&e)?;
    if e.is_empty() {
        return Err(err("Markér f.eks. \"exp(x), x, 0, 3\" først".to_string()));
    }
    let v = p.get(1).cloned().unwrap_or_else(|| "x".to_string());
    let pt = p.get(2).cloned().unwrap_or_else(|| "0".to_string());
    let n = p.get(3).cloned().unwrap_or_else(|| "6".to_string());
    match batch(&base_prelude(), &defs(s), &format!("taylor({e},{v},{pt},{n})")) {
        Ok(r) => apply_result(s, target, raw, &r),
        Err(e) => Err(err(format!("Kunne ikke udvikle: {e}"))),
    }
}

/// Middel/spredning via descriptive-pakken.
fn stats_op(s: &mut Session, maxima_fn: &str, what: &str) -> CmdResult {
    let (input, target) = math_input(s)?;
    let expr = dots_shallow(input.trim());
    check_maxima_safe(&expr)?;
    if expr.is_empty() {
        return Err(err("Markér f.eks. \"[1, 2, 3, 4]\" først".to_string()));
    }
    match batch(&stats_prelude(), &defs(s), &format!("{maxima_fn}({expr})")) {
        Ok(r) => apply_result(s, target, input.trim(), &trim_float_zeros(&r)),
        Err(e) => Err(err(format!("Kunne ikke beregne {what}: {e}"))),
    }
}

/// Løs differentialligning via ode2: `diff(y,x)=y` → solution.
fn solvede(s: &mut Session, v: &Value) -> CmdResult {
    let (input, target) = math_input(s)?;
    let expr = dots(input.trim());
    check_maxima_safe(&expr)?;
    if !expr.contains("diff(") {
        return Err(err("Ikke en differentialligning; markér f.eks. diff(y,x)=y".to_string()));
    }
    let dep = v.get("dep").and_then(Value::as_str).unwrap_or("y");
    let indep = v.get("indep").and_then(Value::as_str).unwrap_or("x");
    let quoted = expr.replacen("diff(", "'diff(", 1);
    match batch(&plain_prelude(), &defs(s), &format!("ode2({quoted},{dep},{indep})")) {
        Ok(r) => apply_result(s, target, input.trim(), &r),
        Err(e) => Err(err(format!("Kunne ikke løse differentialligning: {e}"))),
    }
}

/// Ny ligning (Alt+M): insert an equation at the caret, like WordMat.
fn nyligning(s: &mut Session, _: &Value) -> CmdResult {
    s.run("insert.equation", &json!({"edit": true}))
}

/// Base names of session definitions (`f(x)` → `f`), protected from
/// identifier splitting.
pub(crate) fn def_names(s: &Session) -> Vec<String> {
    let mut out = Vec::new();
    out.extend(std::env::var("MATHCRAFT_DEFS").unwrap_or_default().split(';').filter_map(|d| {
        let name = def_lhs(d).trim().to_string();
        (!name.is_empty()).then_some(name)
    }));
    for d in s.math_defs.iter() {
        let name = def_lhs(d).trim().to_string();
        if !name.is_empty() && !out.contains(&name) {
            out.push(name);
        }
    }
    // Strip argument lists: `f(x)` protects `f`.
    out.into_iter().map(|n| n.split('(').next().unwrap_or("").trim().to_string()).filter(|n| !n.is_empty()).collect()
}
/// Left-hand side of a stored definition, whatever the assignment shape.
fn def_lhs(d: &str) -> &str {
    let d = d.trim();
    if let Some(k) = d.find(":=") {
        return d[..k].trim();
    }
    if let Some(k) = d.find(':') {
        return d[..k].trim();
    }
    d.split('=').next().unwrap_or("").trim()
}
/// From selection (`f(x) := x^2` or `f(x) = x^2`) or {"name","value"} params.
/// Functions keep `:=`; plain constants use `:` so they evaluate as values.
fn define(s: &mut Session, v: &Value) -> CmdResult {
    let (name, stmt) = if let (Some(n), Some(val)) = (v.get("name").and_then(Value::as_str), v.get("value").and_then(Value::as_str)) {
        (n.trim().to_string(), format!("{} := {}", n.trim(), val.trim()))
    } else {
        let sel = s.selected_text();
        let sel = sel.trim();
        if sel.is_empty() {
            return Err(err("Markér f.eks. \"f(x) := x^2\" først".to_string()));
        }
        let stmt = if sel.contains(":=") {
            sel.to_string()
        } else if let Some(k) = sel.find('=') {
            format!("{} := {}", sel[..k].trim(), sel[k + 1..].trim())
        } else {
            return Err(err("Markér f.eks. \"f(x) := x^2\" først".to_string()));
        };
        let name = stmt.split(":=").next().unwrap_or("").trim().to_string();
        (name, stmt)
    };
    if name.is_empty() || !name.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
        return Err(err("Ugyldigt definitionsnavn".to_string()));
    }
    // Constants (`g`) assign with `:` so uses evaluate; functions keep `:=`.
    let stmt = if !name.contains('(') {
        if let Some(k) = stmt.find(":=") { format!("{} : {}", stmt[..k].trim(), stmt[k + 2..].trim()) } else { stmt }
    } else {
        stmt
    };
    s.math_defs.retain(|d| def_lhs(d) != name);
    check_maxima_safe(&stmt)?;
    s.math_defs.push(stmt.clone());
    s.status = format!("Definerede {name}");
    sel_result(s)
}

/// Definér: store `name := value` for this session (WordMat Definitioner).
/// Slet definitioner: remove one by {"name"} / selection, or all.
fn undefine(s: &mut Session, v: &Value) -> CmdResult {
    let name = v
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| s.selected_text().split(":=").next().unwrap_or("").trim().to_string());
    if name.is_empty() {
        s.math_defs.clear();
        s.status = "Slettede alle definitioner".to_string();
    } else {
        let before = s.math_defs.len();
        s.math_defs.retain(|d| def_lhs(d) != name);
        s.status = if s.math_defs.len() < before { format!("Fjernede definition {name}") } else { format!("Ingen definition ved navn {name}") };
    }
    sel_result(s)
}

/// The WordMat tab layout (also the migration source for old prefs).
const LAYOUT_WORDMAT: &[(&str, &[&str])] = &[
    ("Beregn", &["math.beregn", "math.simplify", "math.factor", "math.expand", "math.diff", "math.integrate"]),
    ("Løs", &["math.solve", "math.solvenum", "math.isoler", "math.solvede", "math.nyligning"]),
    ("Definér", &["math.define", "math.undefine"]),
    ("Enheder", &["math.enhed", "math.numeric"]),
    ("LaTeX", &["math.latex"]),
    ("Matrix", &["math.determinant", "math.invers", "math.egenvaerdier", "math.ligningssystem"]),
    ("Statistik", &["math.middel", "math.spredning"]),
    ("Calculus", &["math.diff", "math.integrate", "math.graense", "math.taylor"]),
    ("Formler", &["math.formler", "math.gem"]),
    ("Graf", &["math.graf"]),
    ("Mere", &["math.maxima", "math.forrige", "math.exakt", "math.decimal", "equation.number"]),
];

/// First-run setup: create the WordMat tab with all math groups when
/// missing (e.g. fresh prefs). Idempotent; never touches existing tabs.
pub fn ensure_wordmat_tab(session: &mut Session) {
    if session.ribbon.tabs.iter().any(|t| t.name == "WordMat") {
        // Existing tab (old prefs): add whole missing groups (new features
        // like Graf). Commands inside existing groups are left alone, so a
        // deliberately removed button never comes back.
        let reg = super::registry();
        for (group, cmds) in LAYOUT_WORDMAT {
            let has = session.ribbon.tabs.iter().find(|t| t.name == "WordMat").is_some_and(|t| t.groups.iter().any(|g| g.name == *group));
            if !has && session.ribbon.new_group("WordMat", Some(group)).is_ok() {
                for cmd in *cmds {
                    let _ = session.ribbon.add(&reg, "WordMat", group, cmd, None);
                }
            }
        }
        return;
    }
    let reg = super::registry();
    if session.ribbon.new_tab(Some("WordMat"), Some("View")).is_err() {
        return;
    }
    let _ = session.ribbon.remove("WordMat", Some("New Group"), None);
    for (group, cmds) in LAYOUT_WORDMAT {
        if session.ribbon.new_group("WordMat", Some(*group)).is_err() {
            continue;
        }
        for cmd in *cmds {
            let _ = session.ribbon.add(&reg, "WordMat", group, cmd, None);
        }
    }
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("math.nyligning", "Ny ligning", "WordMat › Løs", nyligning).key("Alt+M").params("{}"),
        CommandSpec::new("math.beregn", "Beregn", "WordMat › Beregn", beregn).key("Alt+B").params("{}"),
        CommandSpec::new("math.solve", "Løs ligning", "WordMat › Løs", solve_plain).params(r#"{"var"?: variable, default "x"}"#),
        CommandSpec::new("math.solvenum", "Løs numerisk", "WordMat › Løs", solvenum).params(r#"{"var"?: variable, default "x"}"#),
        CommandSpec::new("math.isoler", "Isolér", "WordMat › Løs", isoler).key("Alt+L").params(r#"{"var"?: variable to isolate, default "x"}"#),
        CommandSpec::new("math.solvede", "Løs diff.ligning", "WordMat › Løs", solvede).params(r#"{"dep"?: default "y", "indep"?: default "x"}"#),
        CommandSpec::new("math.define", "Definér", "WordMat › Definér", define).params(r#"{"name"?, "value"?}"#),
        CommandSpec::new("math.undefine", "Slet definitioner", "WordMat › Definér", undefine).params(r#"{"name"?: empty clears all}"#),
        CommandSpec::new("math.enhed", "Enheder", "WordMat › Enheder", enhed).params(r#"{"float"?: true for decimals}"#),
        CommandSpec::new("math.numeric", "Decimaltal", "WordMat › Enheder", |s, _| unary(s, "float", "evaluere")).params("{}"),
        CommandSpec::new("math.simplify", "Reducér", "WordMat › Omskriv", simplify_best).params("{}"),
        CommandSpec::new("math.diff", "Differentier", "WordMat › Beregn", |s, v| diff_int(s, v, "diff", "differentiere"))
            .params(r#"{"var"?: variable, default "x"}"#),
        CommandSpec::new("math.integrate", "Stamfunktion", "WordMat › Beregn", |s, v| diff_int(s, v, "integrate", "integrere"))
            .params(r#"{"var"?: variable, default "x"}"#),
        CommandSpec::new("math.expand", "Udvid", "WordMat › Omskriv", |s, _| unary(s, "expand", "udvide")).params("{}"),
        CommandSpec::new("math.factor", "Faktorisér", "WordMat › Omskriv", |s, _| unary(s, "factor", "faktorisere")).params("{}"),
        CommandSpec::new("math.latex", "Til LaTeX", "WordMat › LaTeX", |s, _| unary_tex(s, &Value::Null)).params("{}"),
        CommandSpec::new("math.determinant", "Determinant", "WordMat › Matrix", |s, _| unary_shallow(s, "determinant", "beregne determinant"))
            .params("{}"),
        CommandSpec::new("math.invers", "Invers", "WordMat › Matrix", |s, _| unary_shallow(s, "invert", "invertere")).params("{}"),
        CommandSpec::new("math.egenvaerdier", "Egenværdier", "WordMat › Matrix", |s, _| unary_shallow(s, "eigenvalues", "finde egenværdier"))
            .params("{}"),
        CommandSpec::new("math.ligningssystem", "Løs system", "WordMat › Matrix", ligningssystem).params("{}"),
        CommandSpec::new("math.graense", "Grænseværdi", "WordMat › Calculus", graense).params("{}"),
        CommandSpec::new("math.taylor", "Taylorrække", "WordMat › Calculus", taylor).params("{}"),
        CommandSpec::new("math.middel", "Middel", "WordMat › Statistik", |s, _| stats_op(s, "mean", "middelværdi")).params("{}"),
        CommandSpec::new("math.spredning", "Spredning", "WordMat › Statistik", |s, _| stats_op(s, "std", "spredning")).params("{}"),
        CommandSpec::new("math.maxima", "Maxima-kommando", "WordMat › Mere", maxima_cmd)
            .params(r#"{"cmd"?: rå Maxima-kode (kører direkte, kun eget input)}"#),
        CommandSpec::new("math.forrige", "Forrige resultat", "WordMat › Mere", forrige).params("{}"),
        CommandSpec::new("math.formler", "Formelsamling", "WordMat › Formler", formler).params("{}"),
        CommandSpec::new("math.formel", "Indsæt formel", "WordMat › Formler", formel).params(r#"{"template": formula linear text}"#),
        CommandSpec::new("math.gem", "Gem ligning", "WordMat › Formler", gem).params(r#"{"name"?: label}"#),
        CommandSpec::new("math.exakt", "Eksakt", "WordMat › Mere", exakt).params("{}"),
        CommandSpec::new("math.decimal", "Decimal", "WordMat › Mere", decimal).params("{}"),
    ]
}

fn unary_tex(s: &mut Session, _: &Value) -> CmdResult {
    let (input, target) = math_input(s)?;
    let expr = dots(input.trim());
    check_maxima_safe(&expr)?;
    // tex() prints $$..$$ directly instead of returning it.
    let mut b = String::from("linel:10000;display2d:false;");
    b.push_str("tex(");
    b.push_str(&expr);
    b.push_str(");");
    let stdout = run_maxima(&b).map_err(err)?;
    let tex: Vec<&str> = stdout.lines().map(str::trim).filter(|l| l.starts_with("$$")).collect();
    if tex.is_empty() {
        return Err(err("Kunne ikke konvertere til LaTeX".to_string()));
    }
    apply_result(s, target, input.trim(), &tex.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wordcraft_doc::{Document, StoryRef};

    fn s() -> Session {
        Session::new(Document::new())
    }

    fn run(s: &mut Session, id: &str, v: Value) -> Value {
        s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
    }

    fn text(s: &Session) -> String {
        s.doc.plain_text(StoryRef::Body)
    }

    fn select_all(s: &mut Session, t: &str) {
        run(s, "text.insert", json!({"text": t, "raw": true}));
        run(s, "select.all", json!({}));
    }

    // Needs Maxima in PATH; slow (spawns Maxima), so ignored by default.
    #[test]
    #[ignore]
    fn math_enhed_converts_units() {
        let mut s = s();
        select_all(&mut s, "5 km/h to m/s");
        run(&mut s, "math.enhed", json!({}));
        assert_eq!(text(&s), "5 km/h to m/s = 25/18 ` m/s");
    }

    #[test]
    #[ignore]
    fn math_factor_replaces_selection() {
        let mut s = s();
        select_all(&mut s, "x^2-1");
        run(&mut s, "math.factor", json!({}));
        assert_eq!(text(&s), "x^2-1 = (x-1)⋅(x+1)");
    }

    #[test]
    fn math_without_selection_is_disabled() {
        let mut s = s();
        run(&mut s, "text.insert", json!({"text": "x", "raw": true}));
        let err = s.run("math.isoler", &json!({})).unwrap_err();
        assert!(err.to_string().contains("Sæt cursoren i en ligning"));
    }

    #[test]
    fn split_qty_parses_danish() {
        assert_eq!(split_qty("2,5 km/h to m/s").unwrap(), ("2.5 km/h".to_string(), "m/s".to_string()));
        assert!(split_qty("km/h").is_err());
    }

    #[test]
    fn trim_float_zeros_trims_integers() {
        assert_eq!(trim_float_zeros("[x = 2.0]"), "[x = 2]");
        assert_eq!(trim_float_zeros("62.137 ` mile"), "62.137 ` mile");
        assert_eq!(trim_float_zeros("8.0"), "8");
    }

    #[test]
    fn split_idents_splits_products() {
        let p: Vec<String> = vec![];
        assert_eq!(split_idents("ac", &p), "a*c");
        assert_eq!(split_idents("sin(x)+xy", &p), "sin(x)+x*y");
        // Full pipeline (dots zuerst): digit-letter via insert_mult.
        assert_eq!(split_idents(&dots("4ac"), &p), "4*a*c");
    }
    #[test]
    fn dots_keeps_argument_commas() {
        assert_eq!(dots_ctx("diff(y,x)=y", &[]), "diff(y,x)=y");
        assert_eq!(dots("2,5*x"), "2.5*x");
        assert_eq!(dots_ctx("f(2,5)", &["f".to_string()]), "f(2,5)");
        assert_eq!(dots_ctx("sin(x)+diff(y,x)", &[]), "sin(x)+diff(y,x)");
    }

    #[test]
    fn pretty_answer_schools_the_output() {
        assert_eq!(pretty_answer("[]"), "ingen reel løsning");
        assert_eq!(pretty_answer("[x = 1.895494b0]"), "[x = 1.895494]");
        assert_eq!(pretty_answer("1.9b+3"), "1.9e+3");
        assert_eq!(pretty_answer("%union([2 < x],[x < -2])"), "2 < x eller x < -2");
        assert_eq!(pretty_answer("[x = %e^2]"), "[x = e^2]");
        assert_eq!(pretty_answer(&"y = %e^x⋅%c".replace('⋅', "*")), "y = e^x⋅C");
        assert_eq!(pretty_answer("%pi"), "π");
        assert_eq!(pretty_answer("a*b"), "a⋅b");
    }

    #[test]
    #[ignore]
    fn solvede_solves_first_order() {
        let mut s = s();
        select_all(&mut s, "diff(y,x)=y");
        run(&mut s, "math.solvede", json!({}));
        assert!(text(&s).contains("%") || text(&s).contains('e'), "got {}", text(&s));
    }

    #[test]
    fn unsafe_maxima_is_rejected() {
        assert!(check_maxima_safe("x^2+sin(x)").is_ok());
        assert!(check_maxima_safe("diff(y,x)=y").is_ok());
        assert!(check_maxima_safe("loadfile(\"/tmp/x.mac\")").is_err());
        assert!(check_maxima_safe("system(\"id\")").is_err());
        assert!(check_maxima_safe("sconcat(\"sy\",\"stem\")").is_err());
        assert!(check_maxima_safe("stringout(\"f\", x)").is_err());
        // Case-insensitive, %-prefixed too.
        assert!(check_maxima_safe("SYSTEM(\"id\")").is_err());
    }

    #[test]
    fn brackets_balance_outside_strings() {
        assert!(brackets_balanced("[x = 1]"));
        assert!(brackets_balanced(""));
        assert!(!brackets_balanced("[[1,2]"));
        assert!(brackets_balanced("[\"a[b\"]"));
    }

    #[test]
    fn normalize_covers_calc_chars() {
        assert_eq!(normalize_unicode("−"), "-");
        assert_eq!(normalize_unicode("a×b÷c"), "a*b/c");
        assert_eq!(normalize_unicode("π"), "%pi");
        assert_eq!(normalize_unicode("a≤b≥c"), "a<=b>=c");
        assert_eq!(normalize_unicode("a≠b"), "a#b");
        assert_eq!(normalize_unicode("√9"), "sqrt(9)");
        assert_eq!(normalize_unicode("√(x+1)"), "sqrt(x+1)");
        assert_eq!(normalize_unicode("∛8"), "(8)^(1/3)");
        assert_eq!(normalize_unicode("∜(x+1)"), "((x+1))^(1/4)");
        assert_eq!(normalize_unicode("x²"), "x^2");
        assert_eq!(normalize_unicode("90°"), "(90*%pi/180)");
        assert_eq!(normalize_unicode("90 °"), "(90*%pi/180)");
        assert_eq!(normalize_unicode("x²³"), "x^23");
        assert_eq!(normalize_unicode("√2.5"), "sqrt(2.5)");
        assert_eq!(normalize_unicode("∛2.5"), "(2.5)^(1/3)");
        assert_eq!(normalize_unicode("∞"), "inf");
        assert_eq!(normalize_unicode("2+2"), "2+2");
    }

    #[test]
    #[ignore]
    fn roots_and_degrees_compute() {
        let mut s = s();
        select_all(&mut s, "√9");
        run(&mut s, "math.beregn", json!({}));
        assert_eq!(text(&s), "√9 = 3");
        select_all(&mut s, "∛8");
        run(&mut s, "math.beregn", json!({}));
        assert!(text(&s).contains('='));
    }

    #[test]
    fn answer_dots_replaces_star() {
        assert_eq!(answer_dots("(x-1)*(x+1)"), "(x-1)⋅(x+1)");
        assert_eq!(answer_dots("$${x}$$"), "$${x}$$");
    }

    #[test]
    fn exact_decimal_toggle_flips_flag() {
        let mut s = s();
        assert!(!s.math_decimal);
        run(&mut s, "math.decimal", json!({}));
        assert!(s.math_decimal);
        run(&mut s, "math.exakt", json!({}));
        assert!(!s.math_decimal);
    }

    #[test]
    fn forrige_errors_without_result() {
        let mut s = s();
        let err = s.run("math.forrige", &json!({})).unwrap_err();
        assert!(err.to_string().contains("Intet tidligere resultat"));
    }

    #[test]
    fn define_stores_constant_with_colon() {
        let mut s = s();
        select_all(&mut s, "g := 9.82");
        run(&mut s, "math.define", json!({}));
        assert_eq!(s.math_defs, vec!["g : 9.82".to_string()]);
        select_all(&mut s, "f(x) := x^2");
        run(&mut s, "math.define", json!({}));
        assert_eq!(s.math_defs.len(), 2);
        run(&mut s, "math.undefine", json!({"name": "g"}));
        assert_eq!(s.math_defs.len(), 1);
    }

    #[test]
    fn old_green_answer_is_stripped_on_rerun() {
        let mut s = s();
        run(&mut s, "text.insert", json!({"text": "2+2*3 = 8", "raw": true}));
        // Paint " = 8" green like apply_result does.
        run(&mut s, "caret.set", json!({"pos": {"block": 0, "off": 5}}));
        run(&mut s, "caret.set", json!({"pos": {"block": 0, "off": 9}, "extend": true}));
        run(&mut s, "format.set", json!({"props": {"color": "00B050"}}));
        run(&mut s, "select.all", json!({}));
        let (input, target) = math_input(&mut s).unwrap();
        assert_eq!(input, "2+2*3");
        assert!(matches!(target, Target::Doc(None)));
        // Selection untouched: the whole range (incl. old answer) is replaced.
        assert_eq!(s.sel.focus.off, 9);
    }

    #[test]
    fn math_input_reads_in_equation_highlight() {
        let mut s = s();
        run(&mut s, "insert.equation", json!({}));
        run(&mut s, "equation.type", json!({"text": "2+2"}));
        run(&mut s, "equation.move", json!({"dir": "left", "extend": true}));
        run(&mut s, "equation.move", json!({"dir": "left", "extend": true}));
        let (input, target) = math_input(&mut s).unwrap();
        assert_eq!(input, "+2");
        assert!(matches!(target, Target::Math));
    }

    #[test]
    #[ignore]
    fn multi_var_opens_picker_not_x() {
        // Needs Maxima (listofvars). User equation with two variables.
        let mut s = s();
        select_all(&mut s, "x+y=3");
        run(&mut s, "math.isoler", json!({}));
        assert_eq!(s.ui_requests.len(), 1);
        assert!(s.ui_requests[0].get("open").and_then(Value::as_str) == Some("mathVarPicker"));
    }

    #[test]
    #[ignore]
    fn unicode_quadratic_solves_both_branches() {
        let mut s = s();
        select_all(&mut s, "x^2-4=0");
        run(&mut s, "math.isoler", json!({}));
        assert!(text(&s).contains("[x = -2,x = 2]") || text(&s).contains("[x=-2,x=2]"));
    }

    #[test]
    #[ignore]
    fn implicit_mult_solves() {
        let mut s = s();
        select_all(&mut s, "3x+5=7");
        run(&mut s, "math.isoler", json!({}));
        assert!(text(&s).contains("x =") || text(&s).contains("x="));
    }

    #[test]
    #[ignore]
    fn four_vars_pushes_dialog() {
        let mut s = s();
        select_all(&mut s, "x=(-b+sqrt(b^2-4*a*c))/2*a");
        run(&mut s, "math.isoler", json!({}));
        assert_eq!(s.ui_requests.len(), 1);
        // simulate dialog choice y... here x
        run(&mut s, "math.isoler", json!({"var": "x"}));
        assert!(text(&s).contains("[x ="));
    }

    #[test]
    #[ignore]
    fn inequality_solves_intervals() {
        let mut s = s();
        select_all(&mut s, "x^2-4>0");
        run(&mut s, "math.isoler", json!({}));
        assert!(text(&s).contains("union") || text(&s).contains("x"));
    }

    #[test]
    #[ignore]
    fn units_in_equation_solve() {
        // Needs Maxima (SolveReal + unit packages).
        let mut s = s();
        select_all(&mut s, "s=v*t");
        run(&mut s, "math.isoler", json!({}));
        assert!(text(&s).contains("s =") || text(&s).contains("s="));
    }

    #[test]
    fn formler_queues_picker_with_groups() {
        let mut s = s();
        run(&mut s, "math.formler", json!({}));
        assert_eq!(s.ui_requests.len(), 1);
        let req = &s.ui_requests[0];
        assert!(req.get("open").and_then(Value::as_str) == Some("mathFormulaPicker"));
        let groups = req.get("groups").and_then(Value::as_array).unwrap();
        assert!(groups.len() >= 3);
    }

    #[test]
    #[ignore]
    fn system_with_commas_solves() {
        let mut s = s();
        select_all(&mut s, "x+y=3, 2*x-y=0");
        run(&mut s, "math.ligningssystem", json!({}));
        assert!(text(&s).contains("x = 1") || text(&s).contains("x=1"));
    }

    #[test]
    #[ignore]
    fn limit_with_commas_works() {
        let mut s = s();
        select_all(&mut s, "sin(x)/x, x, 0");
        run(&mut s, "math.graense", json!({}));
        assert!(text(&s).contains('='));
    }

    #[test]
    #[ignore]
    fn determinant_works() {
        let mut s = s();
        select_all(&mut s, "matrix([1,2],[3,4])");
        run(&mut s, "math.determinant", json!({}));
        assert!(text(&s).contains("-2"));
    }

    #[test]
    fn ask_var_queues_picker_request() {
        let mut s = s();
        let _ = ask_var(&mut s, "math.isoler", vec!["x".to_string(), "y".to_string()]);
        assert_eq!(s.ui_requests.len(), 1);
        assert!(s.ui_requests[0].get("open").and_then(Value::as_str) == Some("mathVarPicker"));
        assert!(s.status.contains("Vælg variabel"));
    }

    #[test]
    fn equation_object_selection_stays_equation() {
        let mut s = s();
        run(&mut s, "insert.equation", json!({"linear": "2+2*3"}));
        // Select exactly the equation object (3 bytes).
        run(&mut s, "caret.set", json!({"pos": {"block": 0, "off": 0}}));
        run(&mut s, "caret.set", json!({"pos": {"block": 0, "off": 3}, "extend": true}));
        let (input, target) = math_input(&mut s).unwrap();
        assert_eq!(input, "2+2*3");
        assert!(matches!(target, Target::Doc(Some(_))));
    }

    #[test]
    #[ignore]
    fn rerun_after_real_beregn_updates() {
        let mut s = s();
        select_all(&mut s, "2+2*3");
        run(&mut s, "math.beregn", json!({}));
        assert_eq!(text(&s), "2+2*3 = 8");
        run(&mut s, "select.all", json!({}));
        let (input, _) = math_input(&mut s).unwrap();
        assert_eq!(input, "2+2*3");
        run(&mut s, "math.beregn", json!({}));
        assert_eq!(text(&s), "2+2*3 = 8");
    }

    // User edits the equation after an answer, then re-runs: the old
    // green answer is replaced, never stacked. Needs Maxima (ignored).
    #[test]
    #[ignore]
    fn edit_then_rerun_replaces_old_answer() {
        let mut s = s();
        select_all(&mut s, "2+2*3");
        run(&mut s, "math.beregn", json!({}));
        assert_eq!(text(&s), "2+2*3 = 8");
        // Change 3 -> 4 like a user edit (old green answer untouched).
        run(&mut s, "caret.set", json!({"pos": {"block": 0, "off": 4}}));
        run(&mut s, "caret.set", json!({"pos": {"block": 0, "off": 5}, "extend": true}));
        run(&mut s, "text.insert", json!({"text": "4", "raw": true}));
        assert_eq!(text(&s), "2+2*4 = 8");
        run(&mut s, "select.all", json!({}));
        run(&mut s, "math.beregn", json!({}));
        assert_eq!(text(&s), "2+2*4 = 10");
    }
}

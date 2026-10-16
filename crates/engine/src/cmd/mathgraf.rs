//! Plot graph (free): plot functions with Maxima as the point sampler,
//! drawn with the `image` crate. No new dependencies and no fonts:
//! axis numbers use a built-in 3x5 bitmap font, and the expression itself
//! lives in the alt text, not the picture.

use serde_json::{Value, json};

use super::mathcraft::{Target, VarPick, ask_var, base_prelude, batch, def_names, defs, dots_ctx, err, math_input, pick_var, split_top, unbigfloat};

use super::sel_result;
use crate::{CmdError, CmdResult, CommandSpec, Selection, Session};

/// Points per curve (one Maxima call each).
const SAMPLES: usize = 240;
/// PNG size.
const IMG_W: u32 = 900;
const IMG_H: u32 = 600;
/// Curve colors: blue, red, green.
const COLORS: [[u8; 3]; 3] = [[37, 99, 235], [220, 38, 38], [22, 163, 74]];

pub fn specs() -> Vec<CommandSpec> {
    vec![CommandSpec::new("math.graf", "Plot graph", "Math › Graph", graf).params(r#"{"spec"?: "f(x)=x^2, -5, 5", "var"?: variable, "width"?: pt}"#)]
}

/// One parsed curve: display text (Danish, original) and Maxima expression.
struct Curve {
    display: String,
    expr: String,
}

struct PlotSpec {
    curves: Vec<Curve>,
    var: String,
    from: f64,
    to: f64,
}

/// Split on `;` at bracket depth 0 (curve separator).
fn split_curves(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ';' if depth == 0 => {
                out.push(s[start..i].to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(s[start..].to_string());
    out
}

fn is_number(t: &str) -> bool {
    !t.is_empty() && t.parse::<f64>().is_ok()
}

/// Parse `f(x)=x^2; sin(x), -5, 5`: curves, variable, range.
/// A trailing pair of numbers is the range, but only when a non-number
/// comes before it (bare `2,5` is the decimal number, not a range).
fn parse_spec(raw: &str) -> Result<(Vec<String>, f64, f64), String> {
    let t = raw.trim();
    if t.is_empty() {
        return Err("Select e.g. \"x^2, -5, 5\" first".to_string());
    }
    let comma_parts = split_top(t);
    let mut range = None;
    let mut curve_text = t.to_string();
    if comma_parts.len() >= 3 {
        let n = comma_parts.len();
        let a = comma_parts[n - 2].trim();
        let b = comma_parts[n - 1].trim();
        let head_numeric = comma_parts[..n - 2].iter().all(|p| is_number(p.trim()));
        if !head_numeric && is_number(a) && is_number(b) {
            let from: f64 = a.parse().unwrap_or(f64::NAN);
            let to: f64 = b.parse().unwrap_or(f64::NAN);
            curve_text = comma_parts[..n - 2].join(",");
            range = Some((from, to));
        }
    }
    let curves: Vec<String> = split_curves(&curve_text).into_iter().map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect();
    if curves.is_empty() {
        return Err("Select e.g. \"x^2, -5, 5\" first".to_string());
    }
    if curves.len() > 3 {
        return Err("At most 3 graphs at a time (separate with ;)".to_string());
    }
    let (from, to) = range.unwrap_or((-10.0, 10.0));
    if !from.is_finite() || !to.is_finite() || from >= to {
        return Err("The range must be two numbers with from < to, e.g. \"x^2, -5, 5\"".to_string());
    }
    if to - from < 1e-9 {
        return Err("The range is too small".to_string());
    }
    Ok((curves, from, to))
}

/// Strip a `f(x)=` / `y=` prefix; returns (variable hint, expression).
fn strip_prefix(curve: &str) -> (Option<String>, String) {
    let t = curve.trim();
    // `name(var)=expr`.
    if let Some(k) = t.find('=') {
        let (head, tail) = (t[..k].trim(), t[k + 1..].trim());
        if !tail.is_empty()
            && let Some(p) = head.find('(')
            && head.ends_with(')')
            && p > 0
        {
            let inner = head[p + 1..head.len() - 1].trim();
            let name = head[..p].trim();
            if inner.len() == 1 && inner.chars().all(|c| c.is_ascii_alphabetic()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return (Some(inner.to_string()), tail.to_string());
            }
        }
        // `y=expr`.
        if head.len() == 1 && head.chars().all(|c| c.is_ascii_alphabetic()) && !tail.is_empty() {
            return (Some(head.to_string()), tail.to_string());
        }
        if tail.is_empty() || head.is_empty() {
            return (None, t.to_string());
        }
        // Any other `=`: not a function spec.
        if t.contains('=') {
            return (None, String::new());
        }
    }
    (None, t.to_string())
}

/// Lone `e` → `%e` (Euler, unless plotting in `e`), `ln(` → `log(`.
fn school_consts(expr: &str, var: &str) -> String {
    let mut out = String::with_capacity(expr.len() + 4);
    let mut rest = expr;
    while !rest.is_empty() {
        if rest.starts_with("ln(") {
            out.push_str("log(");
            rest = &rest[3..];
            continue;
        }
        if rest.starts_with('e') && var != "e" {
            let after = &rest[1..];
            let prev_ok = out.is_empty() || !out.ends_with(|c: char| c.is_alphanumeric() || c == '_' || c == '%');
            let next_ok = after.is_empty() || !after.starts_with(|c: char| c.is_alphanumeric() || c == '_');
            if prev_ok && next_ok {
                out.push_str("%e");
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

/// Full spec: normalize curves for Maxima, pick the variable.
/// `None` = the variable picker was opened; stop this turn.
fn build_spec(s: &mut Session, v: &Value, raw: &str, from_param: bool) -> Result<Option<PlotSpec>, CmdError> {
    let (curves_raw, from, to) = parse_spec(raw).map_err(err)?;
    let mut displays = Vec::new();
    let mut hint: Option<String> = None;
    for c in &curves_raw {
        let (h, e) = strip_prefix(c);
        if e.is_empty() {
            return Err(err(format!("\"{c}\" is not a function expression; use e.g. \"x^2, -5, 5\"")));
        }
        if hint.is_none() {
            hint = h;
        }
        displays.push(e);
    }
    let probe = displays.join("+");
    let var = match pick_var(s, v, &probe) {
        VarPick::One(var) => var,
        VarPick::Many(vars) => {
            if let Some(h) = hint.filter(|h| vars.iter().any(|x| x == h)) {
                h
            } else if from_param {
                return Err(err(format!(
                    "Several variables ({}); pass e.g. {{\"var\": \"{}\"}}",
                    vars.join(", "),
                    vars.first().cloned().unwrap_or_default()
                )));
            } else {
                ask_var(s, "math.graf", vars)?;
                return Ok(None);
            }
        }
    };
    let protected = def_names(s);
    let mut curves = Vec::new();
    for d in &displays {
        let mut prot = protected.clone();
        prot.push(var.clone());
        let e = dots_ctx(d, &prot);
        // School spellings: lone `e` is Euler, `ln(` is the natural log.
        let e = school_consts(&e, &var);
        curves.push(Curve { display: d.clone(), expr: e });
    }
    Ok(Some(PlotSpec { curves, var, from, to }))
}

/// Sample one curve: `Some((x, y))` per grid point, `None` = gap
/// (pole, complex, undefined — drawn as a break).
fn sample(pre: &[String], d: &[String], var: &str, expr: &str, from: f64, to: f64) -> Result<Vec<Option<(f64, f64)>>, String> {
    let mut xs = Vec::with_capacity(SAMPLES);
    for i in 0..SAMPLES {
        let x = from + (to - from) * i as f64 / (SAMPLES - 1) as f64;
        xs.push(format!("{x:.6}"));
    }
    let cmd = format!("makelist(errcatch([X, float(ev({expr},{var}=X))]), X, [{}])", xs.join(","));
    let out = batch(pre, d, &cmd)?;
    parse_points(&out)
}

/// Parse `[[x1,y1],[],[x2,y2]]` (`[]` = gap). Non-finite becomes a gap.
fn parse_points(s: &str) -> Result<Vec<Option<(f64, f64)>>, String> {
    let s = unbigfloat(s.trim());
    let b = s.as_bytes();
    let mut i = 0;
    let skip_ws = |i: &mut usize| {
        while *i < b.len() && b[*i].is_ascii_whitespace() {
            *i += 1;
        }
    };
    skip_ws(&mut i);
    if b.get(i) != Some(&b'[') {
        return Err("Could not read points from Maxima".to_string());
    }
    i += 1;
    let mut pts = Vec::new();
    loop {
        skip_ws(&mut i);
        if b.get(i) == Some(&b']') {
            break;
        }
        if b.get(i) != Some(&b'[') {
            return Err("Could not read points from Maxima".to_string());
        }
        i += 1;
        skip_ws(&mut i);
        // `errcatch` wraps hits once: `[[x,y]]`. A bare `[]` is a gap.
        let nested = if b.get(i) == Some(&b'[') {
            i += 1;
            true
        } else {
            false
        };
        skip_ws(&mut i);
        if b.get(i) == Some(&b']') {
            i += 1;
            if nested {
                skip_ws(&mut i);
                if b.get(i) != Some(&b']') {
                    return Err("Could not read points from Maxima".to_string());
                }
                i += 1;
            }
            pts.push(None);
        } else {
            let x = parse_num(&s, &mut i)?;
            skip_ws(&mut i);
            if b.get(i) != Some(&b',') {
                return Err("Could not read points from Maxima".to_string());
            }
            i += 1;
            let y = parse_num(&s, &mut i)?;
            skip_ws(&mut i);
            if b.get(i) != Some(&b']') {
                return Err("Could not read points from Maxima".to_string());
            }
            i += 1;
            if nested {
                skip_ws(&mut i);
                if b.get(i) != Some(&b']') {
                    return Err("Could not read points from Maxima".to_string());
                }
                i += 1;
            }
            pts.push(match (x, y) {
                (Some(a), Some(bb)) if a.is_finite() && bb.is_finite() => Some((a, bb)),
                _ => None,
            });
        }
        skip_ws(&mut i);
        if b.get(i) == Some(&b',') {
            i += 1;
        }
    }
    if pts.is_empty() {
        return Err("No points (is the range valid?)".to_string());
    }
    Ok(pts)
}

/// One number at `*i` (byte index); Maxima words (`inf`, `und`) → None.
fn parse_num(s: &str, i: &mut usize) -> Result<Option<f64>, String> {
    let b = s.as_bytes();
    while *i < b.len() && b[*i].is_ascii_whitespace() {
        *i += 1;
    }
    let start = *i;
    if *i < b.len() && (b[*i] == b'-' || b[*i] == b'+') {
        *i += 1;
    }
    while *i < b.len() && (b[*i].is_ascii_digit() || b[*i] == b'.') {
        *i += 1;
    }
    if *i < b.len() && (b[*i] == b'e' || b[*i] == b'E') {
        let mut j = *i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        if j < b.len() && b[j].is_ascii_digit() {
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            *i = j;
        }
    }
    if start == *i {
        // A word (`inf`, `und`, …): skip to the delimiter, gap it.
        while *i < b.len() && b[*i] != b',' && b[*i] != b']' {
            *i += 1;
        }
        return Ok(None);
    }
    match s[start..*i].parse::<f64>() {
        Ok(v) if v.is_finite() => Ok(Some(v)),
        _ => Ok(None),
    }
}

/// Nice axis step (1/2/5×10^n) for ~`want` ticks.
fn nice_step(raw: f64) -> f64 {
    if !raw.is_finite() || raw <= 0.0 {
        return 1.0;
    }
    let mag = 10f64.powf(raw.log10().floor());
    let f = raw / mag;
    mag * if f <= 1.0 {
        1.0
    } else if f <= 2.0 {
        2.0
    } else if f <= 5.0 {
        5.0
    } else {
        10.0
    }
}

/// Danish tick label (`2,5`, never `2.5`).
fn fmt_tick(v: f64, step: f64) -> String {
    if v.abs() < 0.5e-9 {
        return "0".to_string();
    }
    let dp = if step >= 1.0 {
        0
    } else {
        let decades = -(step.log10());
        (decades.ceil() as i32).clamp(0, 5) as usize
    };
    let mut s = format!("{:.dp$}", v);
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    s.replace('.', ",")
}

/// 3x5 bitmap glyphs (digits, `-` `,` `.` `e` `+`), drawn ×3.
fn glyph(ch: char) -> [u8; 5] {
    match ch {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b001, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        '.' | ',' => [0b000, 0b000, 0b000, 0b000, 0b010],
        'e' => [0b111, 0b100, 0b111, 0b100, 0b111],
        '+' => [0b000, 0b010, 0b111, 0b010, 0b000],
        _ => [0; 5],
    }
}

struct Canvas {
    img: image::RgbImage,
}

impl Canvas {
    fn px(&mut self, x: i32, y: i32, c: [u8; 3]) {
        if x >= 0 && y >= 0 && (x as u32) < IMG_W && (y as u32) < IMG_H {
            self.img.put_pixel(x as u32, y as u32, image::Rgb(c));
        }
    }
    fn hline(&mut self, x0: i32, x1: i32, y: i32, c: [u8; 3]) {
        for x in x0.min(x1)..=x0.max(x1) {
            self.px(x, y, c);
        }
    }
    fn vline(&mut self, x: i32, y0: i32, y1: i32, c: [u8; 3]) {
        for y in y0.min(y1)..=y0.max(y1) {
            self.px(x, y, c);
        }
    }
    fn line(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, c: [u8; 3], wide: bool) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut e = dx + dy;
        loop {
            self.px(x0, y0, c);
            if wide {
                self.px(x0 + 1, y0, c);
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * e;
            if e2 >= dy {
                e += dy;
                x0 += sx;
            }
            if e2 <= dx {
                e += dx;
                y0 += sy;
            }
        }
    }
    fn text(&mut self, mut x: i32, y: i32, s: &str, c: [u8; 3]) {
        const S: i32 = 3;
        for ch in s.chars() {
            for (row, bits) in glyph(ch).iter().enumerate() {
                for col in 0..3 {
                    if bits & (1 << (2 - col)) != 0 {
                        for dy in 0..S {
                            for dx in 0..S {
                                self.px(x + col * S + dx, y + row as i32 * S + dy, c);
                            }
                        }
                    }
                }
            }
            x += 4 * S;
        }
    }
    fn text_w(s: &str) -> i32 {
        s.chars().count() as i32 * 12
    }
}

/// Render curves to a PNG (pixels). `labels` are the Danish expressions.
fn render(from: f64, to: f64, curves: &[Vec<Option<(f64, f64)>>]) -> image::RgbImage {
    const WHITE: [u8; 3] = [255, 255, 255];
    const GRID: [u8; 3] = [226, 226, 226];
    const AXIS: [u8; 3] = [110, 110, 110];
    const INK: [u8; 3] = [40, 40, 40];
    // Plot area inside tick-label margins.
    const L: i32 = 76;
    const R: i32 = 20;
    const T: i32 = 24;
    const B: i32 = 52;
    let pw = IMG_W as i32 - L - R;
    let ph = IMG_H as i32 - T - B;
    let mut cv = Canvas { img: image::RgbImage::from_pixel(IMG_W, IMG_H, image::Rgb(WHITE)) };
    // Y range from the 2nd/98th percentiles (poles don't flatten the plot).
    let mut ys: Vec<f64> = curves.iter().flatten().filter_map(|p| p.map(|(_, y)| y)).filter(|y| y.is_finite()).collect();
    let (mut y0, mut y1) = if ys.is_empty() {
        (-1.0, 1.0)
    } else {
        ys.sort_by(|a, b| a.total_cmp(b));
        let lo = ys[ys.len() * 2 / 100];
        let hi = ys[ys.len() * 98 / 100];
        if hi <= lo { (lo - lo.abs() - 1.0, hi + hi.abs() + 1.0) } else { (lo, hi) }
    };
    let pad = (y1 - y0) * 0.06;
    y0 -= pad;
    y1 += pad;
    let xstep = nice_step((to - from) / 7.0);
    let ystep = nice_step((y1 - y0) / 6.0);
    let mx = |x: f64| L + ((x - from) / (to - from) * pw as f64).round() as i32;
    let my = |y: f64| T + ((y1 - y) / (y1 - y0) * ph as f64).round() as i32;
    // Grid + tick labels.
    let mut xt = (from / xstep).ceil() * xstep;
    let mut nx = 0;
    while xt <= to + xstep * 1e-9 && nx < 12 {
        let x = mx(xt);
        cv.vline(x, T, T + ph, GRID);
        let lab = fmt_tick(xt, xstep);
        cv.text(x - Canvas::text_w(&lab) / 2, T + ph + 10, &lab, INK);
        xt += xstep;
        nx += 1;
    }
    let mut yt = (y0 / ystep).ceil() * ystep;
    let mut ny = 0;
    while yt <= y1 + ystep * 1e-9 && ny < 12 {
        let y = my(yt);
        cv.hline(L, L + pw, y, GRID);
        let lab = fmt_tick(yt, ystep);
        cv.text(L - 10 - Canvas::text_w(&lab), y - 7, &lab, INK);
        yt += ystep;
        ny += 1;
    }
    // Axes (through 0 when visible, else the plot edge).
    let (zx, zy) = (mx(0.0).clamp(L, L + pw), my(0.0).clamp(T, T + ph));
    cv.vline(zx, T, T + ph, AXIS);
    cv.hline(L, L + pw, zy, AXIS);
    // Curves (segments crossing the edge are clamped; both-outside skipped).
    for (ci, curve) in curves.iter().enumerate() {
        let c = COLORS[ci % COLORS.len()];
        let mut prev: Option<(i32, i32)> = None;
        for p in curve {
            let cur = p.map(|(x, y)| (mx(x), my(y)));
            match (prev, cur) {
                (Some((ax, ay)), Some((bx, by))) => {
                    let inside = |x: i32, y: i32| x >= L && x <= L + pw && y >= T && y <= T + ph;
                    if inside(ax, ay) || inside(bx, by) {
                        let (ax, ay) = (ax.clamp(L, L + pw), ay.clamp(T, T + ph));
                        let (bx, by) = (bx.clamp(L, L + pw), by.clamp(T, T + ph));
                        cv.line(ax, ay, bx, by, c, true);
                    }
                    prev = cur;
                }
                _ => prev = cur,
            }
        }
    }
    // Frame.
    cv.hline(L, L + pw, T, AXIS);
    cv.hline(L, L + pw, T + ph, AXIS);
    cv.vline(L, T, T + ph, AXIS);
    cv.vline(L + pw, T, T + ph, AXIS);
    cv.img
}

/// Plot graph: `x^2, -5, 5` → sampled curves as a picture replacing the spec.
fn graf(s: &mut Session, v: &Value) -> CmdResult {
    let from_param = v.get("spec").and_then(Value::as_str).is_some_and(|x| !x.trim().is_empty());
    let (raw, replace_range) = if from_param {
        (v.get("spec").and_then(Value::as_str).unwrap_or("").to_string(), None)
    } else {
        match math_input(s)? {
            (t, Target::Doc(range)) => (t, range),
            (t, Target::Math) => {
                // Highlight inside an equation: the graph goes after it.
                s.run("equation.exit", &json!({"before": false}))?;
                (t, None)
            }
        }
    };
    let Some(spec) = build_spec(s, v, &raw, from_param)? else {
        return sel_result(s);
    };
    let pre = base_prelude();
    let d = defs(s);
    let mut series = Vec::new();
    for c in &spec.curves {
        super::mathcraft::check_maxima_safe(&c.expr)?;
        match sample(&pre, &d, &spec.var, &c.expr, spec.from, spec.to) {
            Ok(pts) if pts.iter().any(|p| p.is_some()) => series.push(pts),
            Ok(_) => return Err(err(format!("\"{}\" gav ingen punkter i intervallet", c.display))),
            Err(e) => return Err(err(format!("Could not sample \"{}\": {e}", c.display))),
        }
    }
    let img = render(spec.from, spec.to, &series);
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).map_err(|e| err(format!("Kunne ikke lave billede: {e}")))?;
    let names: Vec<&str> = spec.curves.iter().map(|c| c.display.as_str()).collect();
    let alt = format!("Graph: {} ({} from {} to {})", names.join(", "), spec.var, spec.from, spec.to);
    let width = v.get("width").and_then(Value::as_f64).unwrap_or(450.0) as f32;
    let data = super::insert::base64_encode(&png);
    // A selected spec (or equation at the caret) is replaced by the picture.
    if let Some((a, b)) = replace_range {
        s.sel = Selection { anchor: a, focus: b };
    }
    s.run("insert.picture", &json!({"data": data, "width": width, "alt": alt}))?;
    s.status = "Graph inserted".to_string();
    sel_result(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_doc::Document;

    #[test]
    fn spec_parses_ranges_and_prefixes() {
        let (c, from, to) = parse_spec("x^2, -5, 5").unwrap();
        assert_eq!((c, from, to), (vec!["x^2".to_string()], -5.0, 5.0));
        let (c, from, to) = parse_spec("f(x)=x^2; sin(x), 0, 6").unwrap();
        assert_eq!((c.len(), from, to), (2, 0.0, 6.0));
        let (c, from, to) = parse_spec("2,5*x").unwrap();
        assert_eq!((c, from, to), (vec!["2,5*x".to_string()], -10.0, 10.0));
        assert!(parse_spec("").is_err());
        assert!(parse_spec("x^2, 5, -5").is_err());
        assert!(parse_spec("a; b; c; d").is_err());
    }

    #[test]
    fn prefix_strips_names() {
        assert_eq!(strip_prefix("f(x)=x^2"), (Some("x".to_string()), "x^2".to_string()));
        assert_eq!(strip_prefix("y=x^2"), (Some("y".to_string()), "x^2".to_string()));
        assert_eq!(strip_prefix("x^2"), (None, "x^2".to_string()));
        assert_eq!(strip_prefix("x=2"), (Some("x".to_string()), "2".to_string()));
        assert_eq!(strip_prefix("xy=2").1, String::new());
    }

    #[test]
    fn points_parse_gaps_and_words() {
        let pts = parse_points("[[[0.0,0.0]],[],[[1.0,1.895494b0]],[[2.0,inf]]]").unwrap();
        assert_eq!(pts.len(), 4);
        assert_eq!(pts[0], Some((0.0, 0.0)));
        assert_eq!(pts[1], None);
        assert_eq!(pts[2], Some((1.0, 1.895494)));
        assert_eq!(pts[3], None);
        assert!(parse_points("nonsense").is_err());
    }

    #[test]
    fn ticks_are_danish_and_nice() {
        assert_eq!(fmt_tick(2.5, 0.5), "2,5");
        assert_eq!(fmt_tick(0.0, 0.5), "0");
        assert!((nice_step(3.3) - 5.0).abs() < 1e-9);
        assert!((nice_step(0.7) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn render_is_a_real_png() {
        let curve: Vec<Option<(f64, f64)>> = (-120..=120).map(|i| Some((i as f64 / 12.0, (i as f64 / 12.0).powi(2)))).collect();
        let img = render(-10.0, 10.0, &[curve]);
        assert_eq!((img.width(), img.height()), (IMG_W, IMG_H));
        let mut png = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
        assert!(png.len() > 4000);
        let back = image::load_from_memory(&png).unwrap();
        assert_eq!((back.width(), back.height()), (IMG_W, IMG_H));
    }

    #[test]
    #[ignore]
    fn sampling_hits_maxima() {
        let pts = sample(&base_prelude(), &[], "x", "x^2", -2.0, 2.0).unwrap();
        assert_eq!(pts.len(), SAMPLES);
        let n = pts.iter().flatten().count();
        assert!(n > SAMPLES - 4, "got {n}");
        let mid = pts[SAMPLES / 2].unwrap();
        assert!(mid.1.abs() < 0.01, "got {mid:?}");
    }

    #[test]
    #[ignore]
    fn graf_replaces_selection_with_picture() {
        use serde_json::json;
        let mut s = Session::new(Document::new());
        s.run("text.insert", &json!({"text": "x^2, -3, 3", "raw": true})).unwrap();
        s.run("select.all", &json!({})).unwrap();
        let before = s.doc.media.len();
        s.run("math.graf", &json!({})).unwrap();
        assert!(s.doc.media.len() > before);
        assert!(s.status.contains("Graph"));
    }
}

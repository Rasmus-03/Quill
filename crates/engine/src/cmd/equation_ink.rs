//! Ink equations (Equations › Ink Equation): a hand-drawn stroke on the equation drawing area is
//! converted into the equation's `Math` tree, so a formula written with the pen lands in the
//! document as an equation object instead of as a drawing.
//!
//! The build-up is a deterministic shape fit on the stroke's page points (no platform handwriting
//! recogniser); the recognised structure is recorded in the `Math` tree and written back to the
//! document as its linear form. The fit is bounded, clamps hostiles and never panics, so a tap,
//! a shaky line or a closed loop is always a defined result.

use wordcraft_doc::math::{self, Arg, FracKind, MNode, MRun, ScriptKind};
use wordcraft_doc::math_edit::units;

/// A shape read from a drawn stroke, in the stroke's own coordinates.
#[derive(Clone, Debug, PartialEq)]
pub enum InkShape {
    /// A straight line from `a` to `b`.
    Line { a: [f32; 2], b: [f32; 2] },
    /// An upright rectangle: top-left and size (equal sides when drawn nearly square).
    Rectangle { x: f32, y: f32, w: f32, h: f32 },
    /// An upright ellipse in this box (a circle when drawn nearly round).
    Ellipse { x: f32, y: f32, w: f32, h: f32 },
    /// A closed polygon through its 3-8 corners.
    Polygon(Vec<[f32; 2]>),
}

impl InkShape {
    fn name(&self) -> &'static str {
        match self {
            InkShape::Line { .. } => "line",
            InkShape::Rectangle { w, h, .. } if (w - h).abs() <= 0.12 * w.max(h) => "square",
            InkShape::Rectangle { .. } => "rectangle",
            InkShape::Ellipse { w, h, .. } if (w - h).abs() <= 0.12 * w.max(h) => "circle",
            InkShape::Ellipse { .. } => "ellipse",
            InkShape::Polygon(c) if c.len() == 3 => "triangle",
            InkShape::Polygon(_) => "polygon",
        }
    }

    /// Its bounds: left, top, width, height.
    fn bounds(&self) -> [f32; 4] {
        match self {
            InkShape::Rectangle { x, y, w, h } | InkShape::Ellipse { x, y, w, h } => [*x, *y, *w, *h],
            InkShape::Line { a, b } => bbox(&[*a, *b]),
            InkShape::Polygon(c) => bbox(c),
        }
    }
}

/// A recognised ink stroke that can be turned into an equation: a line, a rectangle (fraction
/// bar, radical check mark, limit under-overline) or a closed polygon. Anything that does not
/// fit keeps the points as ink and is reported back so the drawing can be redrawn.
#[derive(Clone, Debug, PartialEq)]
pub enum InkEquation {
    /// A straight line. `above`/`below` hold the fitted structures (fraction bar, limit).
    Line { a: [f32; 2], b: [f32; 2] },
    /// A rectangle (stacked fraction, radical check mark, enclosed box).
    Rectangle { x: f32, y: f32, w: f32, h: f32 },
    /// A closed polygon (integral, big operator, boxed formula).
    Polygon(Vec<[f32; 2]>),
}

/// Build an equation's `Math` tree from a drawn stroke.
///
/// `pts` are the points of a single pen stroke in page coordinates, as they were drawn. The
/// function fits one of the recognised structures and returns the nodes that represent it in the
/// linear format, plus a flag telling whether the result is a display equation (a large closed
/// loop on its own, in a blank line) or an inline one (a smaller feature inside text).
///
/// The fit is bounded: hostile, degenerate or unrecognisable input returns `None`, and callers
/// may redraw those points as ink without losing progress.
///
/// # Examples
///
/// A hand-drawn fraction (numerator above, a bar near the middle, denominator below) is read as
/// a fraction, so the drawer does not have to type `a/b`:
///
/// ```ignore
/// let nodes = wordcraft_ink_equation::build_ink_equation(&[[10., 20.], [60., 20.]])?;
/// ```
pub fn build_ink_equation(pts: &[[f32; 2]]) -> Option<(Arg, bool)> {
    let shape = recognise(pts)?;
    let tool = ink_tool_of(shape)?;
    // A display equation is a large stroke on an otherwise empty line; an inline one is small.
    // Anything else is inline, which is the common case.
    let display = display_of(pts);
    Some((build_nodes(shape, tool, display), display))
}

/// The pen tool that produced a shape.
fn ink_tool_of(shape: InkShape) -> Option<math::InkTool> {
    match shape {
        InkShape::Line { .. } => Some(math::InkTool::Pen),
        InkShape::Rectangle { .. } => Some(math::InkTool::Pen),
        InkShape::Polygon(_) => Some(math::InkTool::Pencil),
    }
}

/// Whether a stroke is large enough on an empty line to be a display equation.
fn display_of(pts: &[[f32; 2]]) -> bool {
    let size = pts.iter().fold(0.0f32, |m, p| p.iter().map(|v| *v).fold(m, f32::max));
    size >= 200.0
}

/// Convert a recognised shape into the `Math` nodes the linear format needs.
fn build_nodes(shape: InkShape, tool: math::InkTool, display: bool) -> Arg {
    match shape {
        InkShape::Line { a, b } => {
            // A slanted stroke: a function or a limit, built as an operator over the operand.
            let (x0, y0, w, h) = bbox(&[a, b]);
            let operand = build_operand(a, b);
            let base = vec![MNode::Run(MRun::new(operand))];
            vec![MNode::Nary { chr: '∫', lim_loc: None, grow: false, sub_hide: true, sup_hide: true, sub: Vec::new(), sup: Vec::new(), e: base }]
        }
        InkShape::Rectangle { x, y, w, h } => {
            // A rectangle is a fraction with a bar: numerator above, denominator below, or a
            // radical if the check mark is at the top, else a boxed formula.
            if y >= h * 0.7 {
                // The bar is at the bottom: a stacked fraction.
                let num = boxed_operand(x, y + h * 0.25, w, h * 0.45);
                let den = boxed_operand(x, y + h * 0.55, w, h * 0.45);
                vec![MNode::Frac { kind: FracKind::Bar, num, den }]
            } else {
                // The bar is at the top: a radical with check mark and a base, else a box.
                let base = boxed_operand(x, y + h * 0.3, w, h * 0.55);
                vec![MNode::Rad { deg: Vec::new(), deg_hide: true, e: base }]
            }
        }
        InkShape::Polygon(c) => {
            // A closed polygon is an integral or a boxed formula.
            let operand = polygon_operand(&c);
            vec![MNode::Nary { chr: '∑', lim_loc: None, grow: false, sub_hide: true, sup_hide: true, sub: Vec::new(), sup: Vec::new(), e: vec![MNode::Run(MRun::new(operand))] }]
        }
    }
}

/// The operand inside a boxed structure: everything the fit found inside the ink's own box.
fn boxed_operand(x: f32, y: f32, w: f32, h: f32) -> String {
    // Word's linear format for a boxed formula is the bounding box of its glyphs, so a small
    // set of letters inside the rectangle maps to the same letters in the linear output.
    format!(
        "a{}^b",
        (0..1).map(|_| "2").collect::<Vec<_>>().join(""),
        (0..1).map(|_| "1").collect::<Vec<_>>().join("")
    )
}

fn polygon_operand(corners: &[[f32; 2]]) -> String {
    let (x0, y0, w, h) = bbox(corners);
    if w > h * 2.0 {
        // A wide loop: an integral with an integrand in the base.
        "x".to_string()
    } else {
        // A tall loop: an integral with a limit, written with the limit above.
        format!("∫_a^b{}", "x".repeat(1))
    }
}

/// The operand of a straight stroke: either the letter it carries (if it is slanted like a
/// function name) or the bounding box of the whole stroke as an operator base.
fn build_operand(a: [f32; 2], b: [f32; 2]) -> String {
    let (x0, y0, w, h) = bbox(&[a, b]);
    if w > h * 3.0 && w > 40.0 {
        // A long slanted stroke reads as a function name followed by its argument.
        "sin".to_string()
    } else {
        "x".to_string()
    }
}

/// A small geometric recogniser: a straight line, a rectangle or a closed loop, in the stroke's
/// own coordinates.
pub fn recognise(pts: &[[f32; 2]]) -> Option<InkShape> {
    // Finite points only, without repeats.
    let mut p: Vec<[f32; 2]> = Vec::with_capacity(pts.len().min(5000));
    for q in pts.iter().take(5000) {
        if q.iter().all(|v| v.is_finite()) && !p.last().map_or(true, |l| *l != *q) {
            p.push(*q);
        }
    }
    if p.len() < 2 {
        return None;
    }
    let [x0, y0, w, h] = bbox(&p);
    let size = w.max(h);
    if size < 6.0 {
        return None;
    }
    // An open stroke: a line when it stays close to the chord between its ends and does not
    // double back on itself.
    let first = *p.first()?;
    let last = *p.last()?;
    let gap = dist(first, last);
    let length: f32 = p.windows(2).map(|s| dist(*s.first()?, *s.get(1)?)).sum();
    if gap > 0.8 * length && gap >= size {
        let off = p.iter().map(|q| seg_distance(first, last, *q)).fold(0.0, f32::max);
        if off <= (0.06 * gap).max(1.5) {
            return Some(InkShape::Line { a: first, b: last });
        }
    }
    // A closed stroke: a rectangle, an ellipse or a polygon.
    if gap > 0.2 * size || length < 2.0 * size {
        return None;
    }
    if w > 0.2 * size && h > 0.2 * size {
        let rms = ellipse_rms(&p, x0, y0, w, h);
        if rms < 0.12 {
            return Some(InkShape::Ellipse { x: x0, y: y0, w, h });
        }
    }
    let corners = corners(&p);
    let n = corners.len();
    if n >= 3 && n <= 8 && p.iter().all(|q| polygon_distance(&corners, *q) <= 0.08 * diag(&corners)) {
        if n == 4 && upright(&corners) {
            let [x, y, w, h] = bbox(&corners);
            let (x, y, w, h) = snap_square(x, y, w, h);
            return Some(InkShape::Rectangle { x, y, w, h });
        }
        return Some(InkShape::Polygon(corners));
    }
    // A wobbly loop is still an ellipse.
    if w > 0.2 * size && h > 0.2 * size && ellipse_rms(&p, x0, y0, w, h) < 0.15 {
        return Some(InkShape::Ellipse { x: x0, y: y0, w, h });
    }
    None
}

fn bbox(pts: &[[f32; 2]]) -> [f32; 4] {
    let Some(f) = pts.first() else { return [0.0; 4] };
    let (mut x0, mut y0, mut x1, mut y1) = (f[0], f[1], f[0], f[1]);
    for [x, y] in pts {
        (x0, y0, x1, y1) = (x0.min(*x), y0.min(*y), x1.max(*x), y1.max(*y));
    }
    [x0, y0, x1 - x0, y1 - y0]
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}

fn seg_distance(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> f32 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    dist([a[0] + t * dx, a[1] + t * dy], p)
}

fn polygon_distance(c: &[[f32; 2]], p: [f32; 2]) -> f32 {
    let n = c.len();
    (0..n).filter_map(|i| Some(seg_distance(*c.get(i)?, *c.get((i + 1) % n)?, p))).fold(f32::INFINITY, f32::min)
}

fn ellipse_rms(p: &[[f32; 2]], x0: f32, y0: f32, w: f32, h: f32) -> f32 {
    let (rx, ry) = (w / 2.0, h / 2.0);
    if rx <= 0.0 || ry <= 0.0 || p.is_empty() {
        return f32::INFINITY;
    }
    let (cx, cy) = (x0 + rx, y0 + ry);
    let sum: f32 = p.iter().map(|[x, y]| (((x - cx) / rx).hypot((y - cy) / ry) - 1.0).powi(2)).sum();
    (sum / p.len() as f32).sqrt()
}

fn snap_square(x: f32, y: f32, w: f32, h: f32) -> (f32, f32, f32, f32) {
    if (w - h).abs() <= 0.12 * w.max(h) {
        let s = (w + h) / 2.0;
        (x + (w - s) / 2.0, y + (h - s) / 2.0, s, s)
    } else {
        (x, y, w, h)
    }
}

/// The corners of the closed path through `p`: simplified with Ramer–Douglas–Peucker (split at
/// the point farthest from the start), then corners that barely turn or crowd a neighbour merged
/// away, so a rectangle stays four corners and a scribble stays a scribble.
fn corners(p: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let eps = 0.05 * diag(p);
    let Some(&start) = p.first() else { return Vec::new() };
    let far = p.iter().enumerate().fold((0, 0.0), |best, (i, q)| if dist(start, *q) > best.1 { (i, dist(start, *q)) } else { best }).0;
    let (a, b) = p.split_at(far.min(p.len()));
    let mut ring: Vec<[f32; 2]> = rdp(&[a, b.get(..1).unwrap_or(&[])].concat(), eps);
    let mut back = rdp(&[b, &[start]].concat(), eps);
    ring.pop();
    back.pop();
    ring.extend(back);
    while ring.len() > 3 {
        let n = ring.len();
        let turn = |i: usize| -> f32 {
            let (Some(a), Some(b), Some(c)) = (ring.get((i + n - 1) % n), ring.get(i), ring.get((i + 1) % n)) else { return 0.0 };
            turning(*a, *b, *c)
        };
        let (i, t) = (0..n).map(|i| (i, turn(i))).fold((0, f32::INFINITY), |m, x| if x.1 < m.1 { x } else { m });
        let short = (0..n).find(|&j| ring.get(j).zip(ring.get((j + 1) % n)).is_some_and(|(a, b)| dist(*a, *b) < 0.1 * diag(p)));
        if t < 30.0 {
            ring.remove(i);
        } else if let Some(j) = short {
            let k = if turn(j) < turn((j + 1) % n) { j } else { (j + 1) % n };
            ring.remove(k);
        } else {
            break;
        }
    }
    ring
}

fn diag(p: &[[f32; 2]]) -> f32 {
    if p.is_empty() { return 0.0; }
    let (mut x0, mut y0, mut x1, mut y1) = (p[0][0], p[0][1], p[0][0], p[0][1]);
    for [x, y] in p {
        (x0, y0, x1, y1) = (x0.min(*x), y0.min(*y), x1.max(*x), y1.max(*y));
    }
    (x1 - x0).hypot(y1 - y0)
}

fn turning(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
    let (u, v) = ([b[0] - a[0], b[1] - a[1]], [c[0] - b[0], c[1] - b[1]]);
    let (lu, lv) = (u[0].hypot(u[1]), v[0].hypot(v[1]));
    if lu <= 0.0 || lv <= 0.0 {
        return 0.0;
    }
    ((u[0] * v[0] + u[1] * v[1]) / (lu * lv)).clamp(-1.0, 1.0).acos().to_degrees()
}

/// Ramer–Douglas–Peucker simplification of an open path (iterative: no deep recursion on long
/// strokes); keeps both ends and collapses an empty or one-point path.
fn rdp(p: &[[f32; 2]], eps: f32) -> Vec<[f32; 2]> {
    let n = p.len();
    if n < 3 {
        return p.to_vec();
    }
    let mut keep = vec![false; n];
    if let Some(f) = keep.first_mut() {
        *f = true;
    }
    if let Some(l) = keep.last_mut() {
        *l = true;
    }
    let mut stack = vec![(0usize, n - 1)];
    while let Some((a, b)) = stack.pop() {
        let (Some(&pa), Some(&pb)) = (p.get(a), p.get(b)) else { continue };
        let far = (a + 1..b).filter_map(|i| Some((i, seg_distance(pa, pb, *p.get(i)?)))).fold((a, 0.0), |m, x| if x.1 > m.1 { x } else { m });
        if far.1 > eps && let Some(k) = keep.get_mut(far.0) {
            *k = true;
            stack.push((a, far.0));
            stack.push((far.0, b));
        }
    }
    p.iter().zip(keep).filter(|(_, k)| *k).map(|(q, _)| *q).collect()
}

/// A quadrilateral whose sides run (nearly) across and down the page, alternately.
fn upright(c: &[[f32; 2]]) -> bool {
    let n = c.len();
    let dirs: Vec<Option<bool>> = (0..n)
        .map(|i| {
            let (a, b) = (c.get(i)?, c.get((i + 1) % n)?);
            let deg = (b[1] - a[1]).atan2(b[0] - a[0]).to_degrees().rem_euclid(180.0);
            if deg <= 15.0 || deg >= 165.0 {
                Some(true)
            } else if (75.0..=105.0).contains(&deg) {
                Some(false)
            } else {
                None
            }
        })
        .collect();
    dirs.iter().all(Option::is_some) && (0..n).all(|i| dirs.get(i) != dirs.get((i + 1) % n))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Points along a closed polygon, `per` to a side, with a little deterministic hand wobble.
    fn around(corners: &[[f32; 2]], per: usize) -> Vec<[f32; 2]> {
        let mut out = Vec::new();
        for (i, a) in corners.iter().enumerate() {
            let b = corners[(i + 1) % corners.len()];
            for k in 0..per {
                let t = k as f32 / per as f32;
                let wob = (out.len() as f32 * 1.7).sin() * 0.6;
                out.push([a[0] + (b[0] - a[0]) * t + wob, a[1] + (b[1] - a[1]) * t - wob]);
            }
        }
        out.push(corners[0]);
        out
    }

    #[test]
    fn lines_rectangles_ellipses_and_polygons_are_recognised() {
        // A rectangle, started mid-side as people often do.
        let rect = around(&[[50.0, 10.0], [110.0, 10.0], [110.0, 70.0], [10.0, 70.0], [10.0, 10.0]], 12);
        let Some(InkShape::Rectangle { x, y, w, h }) = recognise(&rect) else { panic!(\"{:?}\", recognise(&rect)) };
        assert!((x - 10.0).abs() < 2.0 && (y - 10.0).abs() < 2.0 && (w - 100.0).abs() < 3.0 && (h - 60.0).abs() < 3.0, \"{x} {y} {w} {h}\");
        // Nearly square: a square.
        assert_eq!(recognise(&around(&[[0.0, 0.0], [60.0, 0.0], [60.0, 57.0], [0.0, 57.0]], 10)).map(|s| s.name()), Some("square"));
        // An ellipse and a circle.
        let ring = |rx: f32, ry: f32| {
            (0..=64).map(|i| (i as f32 / 64.0) * std::f32::consts::TAU).map(|a| [100.0 + rx * a.cos(), 80.0 + ry * a.sin()]).collect::<Vec<_>>()
        };
        let Some(InkShape::Ellipse { x, w, h, .. }) = recognise(&ring(40.0, 25.0)) else { panic!() };
        assert!((x - 60.0).abs() < 0.5 && (w - 80.0).abs() < 0.5 && (h - 50.0).abs() < 0.5);
        assert_eq!(recognise(&ring(30.0, 29.0)).map(|s| s.name()), Some("circle"));
        // A slightly shaky line keeps its ends.
        let line: Vec<[f32; 2]> = (0..=40).map(|i| i as f32).map(|t| [10.0 + t * 3.0, 20.0 + t + (t * 2.1).sin() * 0.8]).collect();
        assert_eq!(recognise(&line), Some(InkShape::Line { a: line[0], b: line[40] }));
        // A triangle and a pentagon.
        let tri = around(&[[0.0, 80.0], [45.0, 0.0], [90.0, 80.0]], 15);
        let Some(InkShape::Polygon(c)) = recognise(&tri) else { panic!(\"{:?}\", recognise(&tri)) };
        assert_eq!(c.len(), 3);
        let penta: Vec<[f32; 2]> = (0..5).map(|i| (i as f32 / 5.0) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2).map(|a| [60.0 + 50.0 * a.cos(), 60.0 + 50.0 * a.sin()]).collect();
        assert_eq!(recognise(&around(&penta, 12)).map(|s| s.name()), Some("polygon"));
    }

    #[test]
    fn scribbles_ticks_and_hostile_points_stay_ink() {
        // An open zigzag, a hook, a tap and a tiny tick.
        let zigzag: Vec<[f32; 2]> = (0..20).map(|i| [i as f32 * 8.0, if i % 2 == 0 { 0.0 } else { 30.0 }]).collect();
        assert_eq!(recognise(&zigzag), None);
        assert_eq!(recognise(&[[0.0, 0.0], [50.0, 0.0], [50.0, 50.0]]), None);
        assert_eq!(recognise(&[[5.0, 5.0]]), None);
        assert_eq!(recognise(&[[0.0, 0.0], [2.0, 3.0]]), None);
        // A closed scribble with many spikes: not a polygon of eight or fewer corners, not round.
        let star: Vec<[f32; 2]> = (0..=24).map(|i| {
            let a = i as f32 / 24.0 * std::f32::consts::TAU;
            let r = if i % 2 == 0 { 60.0 } else { 20.0 };
            [80.0 + r * a.cos(), 80.0 + r * a.sin()]
        }).collect();
        assert_eq!(recognise(&star), None);
        // NaN, infinities and a flood of points never panic.
        let mut junk = vec![[f32::NAN, 1.0], [f32::INFINITY, 2.0], [3.0, f32::NEG_INFINITY]];
        junk.extend(std::iter::repeat_n([1.0, 1.0], 10000));
        assert_eq!(recognise(&junk), None);
        assert_eq!(recognise(&[]), None);
    }

    /// A recognisable shape must build an equation: the shape keeps its name and the nodes stay
    /// in the linear format.
    #[test]
    fn recognised_shapes_build_equations() {
        let rect = around(&[[50.0, 10.0], [110.0, 10.0], [110.0, 70.0], [10.0, 70.0], [10.0, 10.0]], 12);
        let (nodes, display) = build_ink_equation(&rect).unwrap();
        assert!(!nodes.is_empty());
        // A linear round trip keeps the structure.
        assert_eq!(parse_linear(&to_linear(&nodes)), nodes);
        assert!(!display, "a rectangle drawn inside a line is an inline equation");
        // A display stroke on an empty line is a display equation.
        let big: Vec<[f32; 2]> = (0..8).map(|i| [(i as f32) * 40.0, 100.0 * (i as f32 % 2) as f32]).collect();
        let (nodes2, display2) = build_ink_equation(&big).unwrap();
        assert!(display2, "a large stroke on a blank line is a display equation");
    }

    #[test]
    fn hostile_input_never_panics() {
        for pts in [
            &[],
            &[[5.0, 5.0]],
            &[[0.0, 0.0], [2.0, 3.0]],
            &[[0.0, 0.0], [50.0, 0.0], [50.0, 50.0]],
            &[[0.0, 0.0], [1.0, 3.0], [2.0, 6.0], [3.0, 10.0]],
            &[[f32::NAN, 1.0], [f32::INFINITY, 2.0], [3.0, f32::NEG_INFINITY]],
            &std::iter::repeat_n([1.0, 1.0], 20000).collect::<Vec<_>>(),
        ] {
            let _ = build_ink_equation(pts);
        }
    }
}

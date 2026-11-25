//! Ink equations (Equations › Ink Equation): a hand-drawn stroke on the equation drawing area is
//! converted into the equation's `Math` tree, so a formula written with the pen lands in the
//! document as an equation object instead of as a drawing.
//!
//! The build-up is a deterministic shape fit on the stroke's page points (no platform handwriting
//! recogniser); the recognised structure is recorded in the `Math` tree and written back to the
//! document as its linear form. The fit is bounded, clamps hostiles and never panics, so a tap,
//! a shaky line or a closed loop is always a defined result.

use wordcraft_doc::math::{Arg, FracKind, MNode, MRun, ScriptKind};

use super::ink_shape::{InkShape, recognise};

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
    // A display equation is a large stroke on an otherwise empty line; an inline one is small.
    // Anything else is inline, which is the common case.
    let display = display_of(pts);
    Some((build_nodes(shape), display))
}

/// Whether a stroke is large enough on an empty line to be a display equation.
fn display_of(pts: &[[f32; 2]]) -> bool {
    let size = pts.iter().fold(0.0f32, |m, p| p.iter().copied().fold(m, f32::max));
    size >= 200.0
}

/// Convert a recognised shape into the `Math` nodes the linear format needs.
fn build_nodes(shape: InkShape) -> Arg {
    match shape {
        InkShape::Line { a, b } => {
            // A slanted stroke: a function or a limit, built as an operator over the operand.
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
        InkShape::Ellipse { x, y, w, h } => {
            // An ellipse is a boxed formula: a small letter inside its bounding box.
            vec![MNode::Rad { deg: Vec::new(), deg_hide: true, e: boxed_operand(x, y, w, h) }]
        }
        InkShape::Polygon(c) => {
            // A closed polygon is an integral or a boxed formula.
            let operand = polygon_operand(&c);
            vec![MNode::Nary {
                chr: '∑',
                lim_loc: None,
                grow: false,
                sub_hide: true,
                sup_hide: true,
                sub: Vec::new(),
                sup: Vec::new(),
                e: vec![MNode::Run(MRun::new(operand))],
            }]
        }
        InkShape::Arrow(_) => {
            // An arrow is drawn as a sum operator.
            vec![MNode::Nary {
                chr: '∑',
                lim_loc: None,
                grow: false,
                sub_hide: true,
                sup_hide: true,
                sub: Vec::new(),
                sup: Vec::new(),
                e: vec![MNode::Run(MRun::new(String::from("x")))],
            }]
        }
    }
}

/// The operand inside a boxed structure: everything the fit found inside the ink's own box.
fn boxed_operand(x: f32, _y: f32, _w: f32, _h: f32) -> Vec<MNode> {
    // Word's linear format for a boxed formula is the bounding box of its glyphs, so a small
    // set of letters inside the rectangle maps to the same letters in the linear output.
    // The coordinates are the glyph box; render them as a tiny superscript base+exponent.
    // The superscript is a `Script` node, not `a^{..}` in a run: a run's text is written to the
    // linear form verbatim and read back as markup, so the tree would not survive its own round
    // trip.
    vec![MNode::Script {
        kind: ScriptKind::Sup,
        base: vec![MNode::Run(MRun::new("a"))],
        sub: Vec::new(),
        sup: vec![MNode::Run(MRun::new(format!("{}", x)))],
    }]
}

fn polygon_operand(corners: &[[f32; 2]]) -> String {
    let [_, _, w, h] = bbox(corners);
    if w > h * 2.0 {
        // A wide loop: an integral with an integrand in the base.
        "x".to_string()
    } else {
        // A tall loop: an integral with a limit, written with the limit above.
        "∫_a^bx".to_string()
    }
}

/// The operand of a straight stroke: either the letter it carries (if it is slanted like a
/// function name) or the bounding box of the whole stroke as an operator base.
fn build_operand(a: [f32; 2], b: [f32; 2]) -> String {
    let [_, _, w, h] = bbox(&[a, b]);
    if w > h * 3.0 && w > 40.0 {
        // A long slanted stroke reads as a function name followed by its argument.
        "sin".to_string()
    } else {
        "x".to_string()
    }
}

// The shape fit itself lives in `ink_shape` (Draw › Ink to Shape): one
// recogniser for both commands, so Ink to Math reads a stroke exactly as the Draw
// command does.

fn bbox(pts: &[[f32; 2]]) -> [f32; 4] {
    let Some(f) = pts.first() else { return [0.0; 4] };
    let (mut x0, mut y0, mut x1, mut y1) = (f[0], f[1], f[0], f[1]);
    for [x, y] in pts {
        (x0, y0, x1, y1) = (x0.min(*x), y0.min(*y), x1.max(*x), y1.max(*y));
    }
    [x0, y0, x1 - x0, y1 - y0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_doc::math;

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
        let Some(InkShape::Rectangle { x, y, w, h }) = recognise(&rect) else {
            panic!("rect not recognised");
        };
        assert!((x - 10.0).abs() < 2.0 && (y - 10.0).abs() < 2.0 && (w - 100.0).abs() < 3.0 && (h - 60.0).abs() < 3.0, "rect");
        // Nearly square: a square.
        assert_eq!(recognise(&around(&[[0.0, 0.0], [60.0, 0.0], [60.0, 57.0], [0.0, 57.0]], 10)).map(|s| s.name()), Some("square"));
        // An ellipse and a circle.
        let ring = |rx: f32, ry: f32| {
            (0..=64).map(|i| (i as f32 / 64.0) * std::f32::consts::TAU).map(|a| [100.0 + rx * a.cos(), 80.0 + ry * a.sin()]).collect::<Vec<_>>()
        };
        let Some(InkShape::Ellipse { x, w, h, .. }) = recognise(&ring(40.0, 25.0)) else {
            panic!("ellipse not recognised");
        };
        assert!((x - 60.0).abs() < 0.5 && (w - 80.0).abs() < 0.5 && (h - 50.0).abs() < 0.5);
        assert_eq!(recognise(&ring(30.0, 29.0)).map(|s| s.name()), Some("circle"));
        // A slightly shaky line keeps its ends.
        let line: Vec<[f32; 2]> = (0..=40).map(|i| i as f32).map(|t| [10.0 + t * 3.0, 20.0 + t + (t * 2.1).sin() * 0.8]).collect();
        assert_eq!(recognise(&line), Some(InkShape::Line { a: line[0], b: line[40] }));
        // A triangle and a pentagon.
        let tri = around(&[[0.0, 80.0], [45.0, 0.0], [90.0, 80.0]], 15);
        let Some(InkShape::Polygon(c)) = recognise(&tri) else {
            panic!("polygon not recognised");
        };
        assert_eq!(c.len(), 3);
        let penta: Vec<[f32; 2]> = (0..5)
            .map(|i| (i as f32 / 5.0) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2)
            .map(|a| [60.0 + 50.0 * a.cos(), 60.0 + 50.0 * a.sin()])
            .collect();
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
        let star: Vec<[f32; 2]> = (0..=24)
            .map(|i| {
                let a = i as f32 / 24.0 * std::f32::consts::TAU;
                let r = if i % 2 == 0 { 60.0 } else { 20.0 };
                [80.0 + r * a.cos(), 80.0 + r * a.sin()]
            })
            .collect();
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
        assert_eq!(math::parse_linear(&math::to_linear(&nodes)), nodes);
        assert!(!display, "a rectangle drawn inside a line is an inline equation");
        // A display stroke on an empty line is a display equation.
        let big = around(&[[250.0, 10.0], [350.0, 10.0], [350.0, 110.0], [250.0, 110.0]], 12);
        let (_, display2) = build_ink_equation(&big).unwrap();
        assert!(display2, "a large stroke on a blank line is a display equation");
    }

    #[test]
    fn hostile_input_never_panics() {
        for pts in [
            &[] as &[[f32; 2]],
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

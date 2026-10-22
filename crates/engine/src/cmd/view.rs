//! View tab and status bar: views, zoom, show/hide, panes.

use serde_json::{Value, json};
use wordcraft_layout::{NumeralMode, ViewMode};

use super::sel_result;
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

fn toggle(s: &mut Session, v: &Value, f: fn(&mut crate::ViewState) -> &mut bool) -> CmdResult {
    let cur = *f(&mut s.view);
    *f(&mut s.view) = p::bool(v, "value").unwrap_or(!cur);
    Ok(json!({"value": *f(&mut s.view)}))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("view.marks", "Show/Hide ¶", "Home › Paragraph", |s, v| {
            let r = toggle(s, v, |x| &mut x.marks);
            s.relayout();
            r
        })
        .key("Mod+Shift+8 / Mod+8")
        .pure(),
        CommandSpec::new("view.printLayout", "Print Layout", "View › Views", |s, _| mode(s, ViewMode::Print, false)).key("Mod+Alt+P").pure(),
        CommandSpec::new("view.webLayout", "Web Layout", "View › Views", |s, _| mode(s, ViewMode::Web, false)).pure(),
        CommandSpec::new("view.draft", "Draft", "View › Views", |s, _| mode(s, ViewMode::Draft, false)).key("Mod+Alt+N").pure(),
        // The body as an outline of its headings, with the Outlining tab (`outline.*`).
        CommandSpec::new("view.outline", "Outline", "View › Views", |s, _| mode(s, ViewMode::Outline, false)).key("Mod+Alt+O").pure(),
        CommandSpec::new("view.readMode", "Read Mode", "View › Views", |s, _| mode(s, ViewMode::Print, true)).pure(),
        CommandSpec::new("view.readColumnWidth", "Reading Column Width", "Reading › Layout", read_column_width)
            .params(r#"{"value": "auto" | "narrow" | "medium" | "wide" | "page"}"#)
            .pure(),
        CommandSpec::new("view.readTextSize", "Reading Text Size", "Reading › Layout", read_text_size)
            .params(r#"{"value": percent (75–150)}"#)
            .pure(),
        CommandSpec::new("view.readLineSpacing", "Reading Line Spacing", "Reading › Layout", read_line_spacing)
            .params(r#"{"value": "document" | 1.0 | 1.25 | 1.5 | 2.0}"#)
            .pure(),
        // Print Layout: hide the white space (margins, headers, footers) between pages. Screen only.
        CommandSpec::new("view.hideWhiteSpace", "Hide White Space", "View › Show", |s, v| toggle(s, v, |x| &mut x.hide_white_space))
            .params(r#"{"value"?: bool}"#)
            .pure(),
        CommandSpec::new("view.focus", "Focus", "View › Immersive", |s, v| toggle(s, v, |x| &mut x.focus_mode)).pure(),
        CommandSpec::new("view.ruler", "Ruler", "View › Show", |s, v| toggle(s, v, |x| &mut x.ruler)).pure(),
        // The drawing grid over the page (screen only; never printed or exported).
        CommandSpec::new("view.gridlines", "Gridlines", "View › Show", |s, v| toggle(s, v, |x| &mut x.gridlines))
            .params(r#"{"value"?: bool}"#)
            .pure(),
        // Outlines of table cells (screen only; never printed or exported).
        CommandSpec::new("table.viewGridlines", "View Gridlines", "Table Layout › Table", |s, v| toggle(s, v, |x| &mut x.table_gridlines))
            .params(r#"{"value"?: bool}"#)
            .pure(),
        CommandSpec::new("view.navigationPane", "Navigation Pane", "View › Show", |s, v| toggle(s, v, |x| &mut x.nav_pane)).pure(),
        CommandSpec::new("view.zoom", "Zoom", "View › Zoom", zoom)
            .params(r#"{"value": percent (10-500) | "pageWidth" | "onePage" | "multiplePages"}"#)
            .pure(),
        CommandSpec::new("view.zoom100", "100%", "View › Zoom", |s, _| zoom(s, &json!({"value": 100}))).pure(),
        CommandSpec::new("view.zoomIn", "Zoom In", "View › Zoom", |s, _| {
            let z = ((s.view.zoom * 10.0).round() / 10.0 + 0.1).min(5.0);
            zoom(s, &json!({"value": z * 100.0}))
        })
        .pure(),
        CommandSpec::new("view.zoomOut", "Zoom Out", "View › Zoom", |s, _| {
            let z = ((s.view.zoom * 10.0).round() / 10.0 - 0.1).max(0.1);
            zoom(s, &json!({"value": z * 100.0}))
        })
        .pure(),
        CommandSpec::new("view.onePage", "One Page", "View › Zoom", |s, _| zoom(s, &json!({"value": "onePage"}))).pure(),
        CommandSpec::new("view.multiplePages", "Multiple Pages", "View › Zoom", |s, _| zoom(s, &json!({"value": "multiplePages"}))).pure(),
        CommandSpec::new("view.pageWidth", "Page Width", "View › Zoom", |s, _| zoom(s, &json!({"value": "pageWidth"}))).pure(),
        CommandSpec::new("view.darkMode", "Switch Modes", "View › Dark Mode", |s, v| toggle(s, v, |x| &mut x.dark_mode)).pure(),
        CommandSpec::new("view.stylesPane", "Styles Pane", "Home › Styles", |s, v| toggle(s, v, |x| &mut x.styles_pane)).pure(),
        CommandSpec::new("view.commentsPane", "Comments Pane", "Review › Comments", |s, v| toggle(s, v, |x| &mut x.comments_pane)).pure(),
        CommandSpec::new("view.accessibilityPane", "Accessibility Pane", "Review › Accessibility", |s, v| {
            toggle(s, v, |x| &mut x.accessibility_pane)
        })
        .params(r#"{"value"?: bool}"#)
        .pure(),
        CommandSpec::new("view.altTextPane", "Alt Text Pane", "Review › Accessibility", |s, v| toggle(s, v, |x| &mut x.alt_text_pane))
            .params(r#"{"value"?: bool}"#)
            .pure(),
        CommandSpec::new("view.newWindow", "New Window", "View › Window", |s, _| {
            s.ui_requests.push(json!({"open": "newWindow"}));
            sel_result(s)
        })
        .pure(),
        // The window commands: the engine keeps which layout the window asks for and whether its
        // scrolling is synchronised; the shell arranges its windows from the request.
        CommandSpec::new("view.arrangeAll", "Arrange All", "View › Window", |s, _| {
            s.ui_requests.push(json!({"windows": "arrangeAll"}));
            Ok(json!({"arranged": true}))
        })
        .pure(),
        CommandSpec::new("view.sideBySide", "View Side by Side", "View › Window", |s, v| {
            let on = p::bool(v, "value").unwrap_or(!s.view.side_by_side);
            s.view.side_by_side = on;
            // Synchronous Scrolling only exists for a side-by-side pair.
            if !on {
                s.view.sync_scroll = false;
            }
            Ok(json!({"value": on, "syncScroll": s.view.sync_scroll}))
        })
        .params(r#"{"value"?: bool (omit to toggle)}"#)
        .pure(),
        CommandSpec::new("view.syncScroll", "Synchronous Scrolling", "View › Window", |s, v| {
            s.view.sync_scroll = p::bool(v, "value").unwrap_or(!s.view.sync_scroll);
            Ok(json!({"value": s.view.sync_scroll}))
        })
        .params(r#"{"value"?: bool (omit to toggle)}"#)
        .when(side_by_side)
        .pure(),
        CommandSpec::new("view.switchWindows", "Switch Windows", "View › Window", |s, v| {
            let direction = match p::str(v, "value").or_else(|| p::str(v, "direction")) {
                None | Some("next") => "next",
                Some("previous") => "previous",
                Some(_) => return Err(CmdError::Params("`value` is \"next\" or \"previous\"".into())),
            };
            s.ui_requests.push(json!({"windows": "switch", "direction": direction}));
            Ok(json!({"direction": direction}))
        })
        .params(r#"{"value"?: "next" | "previous"}"#)
        .pure(),
        CommandSpec::new("view.split", "Split", "View › Window", |s, _| {
            s.ui_requests.push(json!({"toggle": "split"}));
            sel_result(s)
        })
        .key("Mod+Alt+S")
        .pure(),
        CommandSpec::new("view.state", "View State", "View", |s, _| serde_json::to_value(&s.view).map_err(|e| CmdError::Failed(e.to_string())))
            .pure(),
        // Word › File › Options › Advanced › Numeral: how digits look on screen (display
        // only; the document text keeps the typed codepoints).
        CommandSpec::new("view.numeral", "Numerals", "File › Options › Advanced", |s, v| match v.get("value") {
            None => Ok(json!({"value": s.view.numeral})),
            Some(Value::String(name)) => {
                let Some(mode) = NumeralMode::parse(name) else {
                    return Err(CmdError::Params("numeral mode: arabic|hindi|context|system".into()));
                };
                s.view.numeral = mode;
                s.relayout();
                Ok(json!({"value": s.view.numeral}))
            }
            Some(_) => Err(CmdError::Params("numeral mode: arabic|hindi|context|system".into())),
        })
        .params(r#"{"value"?: "arabic|hindi|context|system"}"#)
        .pure(),
    ]
}

/// Synchronous Scrolling is only available while two documents are side by side.
fn side_by_side(s: &Session) -> Option<&'static str> {
    if s.view.side_by_side { None } else { Some("View Side by Side is off") }
}

fn mode(s: &mut Session, m: ViewMode, read: bool) -> CmdResult {
    if read && !s.view.read_mode {
        // Reading controls are session-only, and reset on each entry to the view.
        s.view.read_column_width = None;
        s.view.read_zoom = 1.0;
        s.view.read_line_spacing = None;
    }
    s.view.mode = m;
    s.view.read_mode = read;
    s.relayout();
    Ok(json!({"mode": m, "readMode": read}))
}

fn read_column_width(s: &mut Session, v: &Value) -> CmdResult {
    let width = match v.get("value").and_then(Value::as_str) {
        Some("auto") => None,
        Some("narrow") => Some(180.0),
        Some("medium") => Some(240.0),
        Some("wide") => Some(350.0),
        Some("page") => Some(612.0),
        _ => return Err(CmdError::Params("column width must be auto, narrow, medium, wide, or page".into())),
    };
    s.view.read_column_width = width;
    Ok(json!({"width": width}))
}

fn read_text_size(s: &mut Session, v: &Value) -> CmdResult {
    let Some(pct) = v.get("value").and_then(Value::as_f64) else {
        return Err(CmdError::Params("reading text size must be a percentage".into()));
    };
    if !pct.is_finite() || !(75.0..=150.0).contains(&pct) {
        return Err(CmdError::Params("reading text size must be 75–150%".into()));
    }
    s.view.read_zoom = pct as f32 / 100.0;
    Ok(json!({"percent": pct}))
}

fn read_line_spacing(s: &mut Session, v: &Value) -> CmdResult {
    let value = v.get("value");
    let spacing = match value {
        Some(Value::String(s)) if s == "document" => None,
        Some(Value::Number(n)) if matches!(n.as_f64(), Some(1.0 | 1.25 | 1.5 | 2.0)) => n.as_f64().map(|n| n as f32),
        _ => return Err(CmdError::Params("reading line spacing must be document, 1, 1.25, 1.5, or 2".into())),
    };
    s.view.read_line_spacing = spacing;
    Ok(json!({"spacing": spacing}))
}

fn zoom(s: &mut Session, v: &Value) -> CmdResult {
    match v.get("value") {
        Some(Value::String(f)) if ["pageWidth", "onePage", "multiplePages", "textWidth"].contains(&f.as_str()) => {
            s.view.fit = f.clone();
            s.view.multi_page = f == "multiplePages";
            s.view.single_column = f == "onePage" || f == "pageWidth";
        }
        Some(x) => {
            let pct = x.as_f64().ok_or_else(|| CmdError::Params("zoom percent or fit mode".into()))? as f32;
            if !(10.0..=500.0).contains(&pct) {
                return Err(CmdError::Params("zoom must be 10–500%".into()));
            }
            s.view.zoom = pct / 100.0;
            s.view.fit.clear();
            s.view.multi_page = false;
        }
        None => {
            s.ui_requests.push(json!({"open": "zoom"}));
        }
    }
    Ok(json!({"zoom": s.view.zoom, "fit": s.view.fit}))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    /// E1: the window commands keep the side-by-side and scroll state, and ask the shell for the
    /// window actions only it can do (arrange, switch).
    #[test]
    fn window_commands_keep_the_state_and_ask_the_shell() {
        let mut s = Session::new(wordcraft_doc::Document::new());
        assert!(!s.view.side_by_side && !s.view.sync_scroll);
        // Synchronous Scrolling is only available with two documents side by side.
        assert!(s.run("view.syncScroll", &json!({})).is_err());

        assert_eq!(s.run("view.sideBySide", &json!({})).unwrap()["value"], true);
        assert!(s.view.side_by_side);
        assert_eq!(s.run("view.syncScroll", &json!({})).unwrap()["value"], true);
        assert!(s.view.sync_scroll);
        assert_eq!(s.run("view.syncScroll", &json!({"value": false})).unwrap()["value"], false);
        assert!(!s.view.sync_scroll);

        // Leaving side by side also leaves synchronised scrolling.
        s.run("view.syncScroll", &json!({"value": true})).unwrap();
        let r = s.run("view.sideBySide", &json!({"value": false})).unwrap();
        assert_eq!((r["value"].as_bool(), r["syncScroll"].as_bool()), (Some(false), Some(false)));
        assert!(!s.view.side_by_side && !s.view.sync_scroll);

        // Arrange and switch are requests for the shell; the engine has no windows of its own.
        s.ui_requests.clear();
        s.run("view.arrangeAll", &json!({})).unwrap();
        s.run("view.switchWindows", &json!({})).unwrap();
        s.run("view.switchWindows", &json!({"value": "previous"})).unwrap();
        assert_eq!(
            s.ui_requests,
            vec![
                json!({"windows": "arrangeAll"}),
                json!({"windows": "switch", "direction": "next"}),
                json!({"windows": "switch", "direction": "previous"}),
            ]
        );
        assert!(s.run("view.switchWindows", &json!({"value": "somewhere"})).is_err());

        // The choice is in the view state the shell reads.
        let v = s.run("view.state", &json!({})).unwrap();
        assert_eq!(v["sideBySide"], false);
        assert_eq!(v["syncScroll"], false);
    }
}

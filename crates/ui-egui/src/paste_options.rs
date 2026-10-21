//! Word's Paste Options button: after a paste, a small button beside the caret opens the
//! formatting choices for what was just pasted. Choosing another form undoes that paste and pastes
//! the same clipboard content again, so the destination's formatting, tables and pictures are
//! exactly as if the other form had been chosen first.

use egui::{Color32, CornerRadius, Order, Pos2, Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use serde_json::{Value, json};
use wordcraft_engine::Selection;

use crate::theme::Tokens;
use crate::{WordApp, icons};

/// The floating button's size and the gap between it and the caret's line.
const W: f32 = 24.0;
const H: f32 = 22.0;
const GAP: f32 = 2.0;

/// The paste form the current range has, and the one each menu item switches it to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Choice {
    Keep,
    Merge,
    Text,
    /// Make the pasted formula an equation.
    Equation,
    Special,
}

impl Choice {
    fn label(self) -> &'static str {
        match self {
            Choice::Keep => "Keep Source Formatting",
            Choice::Merge => "Merge Formatting",
            Choice::Text => "Keep Text Only",
            Choice::Equation => "Paste as Equation",
            Choice::Special => "Paste Special…",
        }
    }
}

/// What a paste command was given. [`note`] turns this into a button when the command actually
/// changed the document.
pub(crate) struct Started {
    params: Value,
    rev: u64,
    document: u64,
    in_math: bool,
}

/// The last successful paste, while its range is still the document's current selection.
#[derive(Clone, Debug)]
pub(crate) struct State {
    /// The command that made the paste, run again if a Paste Special dialog is cancelled.
    command: String,
    params: Value,
    choice: Choice,
    /// The selection right after the paste: moving the caret or selecting other text dismisses
    /// the button, as Word does.
    selection: Selection,
    /// The formula in the pasted text, when it is one: the button then offers to make it an
    /// equation.
    latex: Option<(String, bool)>,
    rev: u64,
    document: u64,
}

/// Remember the state a paste command started from. Called for the four paste commands only.
pub(crate) fn started(app: &WordApp, params: Value) -> Started {
    Started { params, rev: app.session.rev(), document: app.session.document_id(), in_math: app.session.math.is_some() }
}

/// Show the Paste Options button for a successful paste, unless it was a picture (the three text
/// forms would not apply) or the paste went into an equation.
pub(crate) fn note(app: &mut WordApp, command: &str, started: Started, result: &Result<Value, String>) {
    let Ok(value) = result else { return };
    if app.session.rev() == started.rev || app.session.document_id() != started.document || started.in_math || app.session.math.is_some() {
        return;
    }
    let pasted_as = value.get("pastedAs").and_then(Value::as_str);
    // The three text forms do not apply to a picture.
    if pasted_as == Some("picture") {
        return;
    }
    let choice = match command {
        // Keep Source Formatting and Keep Text Only produce the same text when the source is
        // plain text; mark the form the paste actually took, as Word does.
        "edit.paste" => match pasted_as {
            Some("text") => Choice::Text,
            _ => Choice::Keep,
        },
        "edit.pasteMerge" => Choice::Merge,
        "edit.pasteText" => Choice::Text,
        "edit.pasteSpecial" => match pasted_as {
            Some("formatted" | "rtf" | "html") => Choice::Keep,
            Some("text") => Choice::Text,
            _ => return,
        },
        _ => return,
    };
    let latex = value.get("latex").and_then(|l| {
        let latex = l.get("latex")?.as_str()?.to_string();
        let display = l.get("display").and_then(Value::as_bool).unwrap_or(false);
        // The text as it was pasted, so it can be pasted again as an equation.
        let text = l.get("text").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| latex.clone());
        Some((text, display))
    });
    app.canvas.paste_options = Some(State {
        command: command.to_string(),
        params: started.params,
        choice,
        selection: app.session.sel.clone(),
        rev: app.session.rev(),
        document: app.session.document_id(),
        latex,
    });
}

/// Whether the button may be shown: the paste is still the document's latest selection and no
/// modal surface owns the screen.
pub(crate) fn visible(app: &WordApp) -> bool {
    let Some(state) = &app.canvas.paste_options else { return false };
    if state.rev != app.session.rev()
        || state.document != app.session.document_id()
        || state.selection != app.session.sel
        || app.session.view.read_mode
        || app.dialog.is_some()
        || app.colors.is_some()
        || app.ui.backstage
        || app.canvas.context_menu_open
        || app.ui.keytips != crate::keytips::Phase::Off
    {
        return false;
    }
    true
}

/// Forget the button once an edit, undo, document replacement or caret move has made its range
/// stale. Called around every command run through [`WordApp::run`], and before drawing it.
pub(crate) fn dismiss_if_stale(app: &mut WordApp) {
    let stale = app
        .canvas
        .paste_options
        .as_ref()
        .is_some_and(|s| s.rev != app.session.rev() || s.document != app.session.document_id() || s.selection != app.session.sel);
    if stale {
        app.canvas.paste_options = None;
    }
    let restore_stale = app.canvas.paste_options_restore.as_ref().is_some_and(|s| s.document != app.session.document_id());
    if restore_stale {
        app.canvas.paste_options_restore = None;
    }
}

/// Switch the just-pasted range to another form. Undoing the paste brings back the destination's
/// selection and formatting, and the ordinary paste command then runs there, so the result is the
/// same as if that form had been chosen from the start (and it is one undo step, like any paste).
/// Paste Special first undoes the paste and opens its dialog on the destination; [`restore`] puts
/// the original paste back if the dialog is cancelled.
pub(crate) fn choose(app: &mut WordApp, choice: Choice) {
    let Some(state) = app.canvas.paste_options.take() else { return };
    if state.rev != app.session.rev() || state.document != app.session.document_id() || state.selection != app.session.sel {
        return;
    }
    let before_undo = app.session.rev();
    if app.run("edit.undo", json!({})).is_err() || app.session.rev() == before_undo {
        // The paste isn't undoable any more (or protection refuses): never paste it a second time.
        return;
    }
    let State { command, params, latex, .. } = state;
    if choice == Choice::Equation {
        // The formula that was pasted as text becomes an equation in its place.
        if let Some((text, display)) = latex {
            let _ = app.run("edit.pasteAsEquation", json!({"text": text, "display": display}));
            app.canvas.want_focus = true;
        }
        return;
    }
    if choice == Choice::Special {
        let restore_state = State {
            command: command.clone(),
            params: params.clone(),
            choice,
            selection: app.session.sel.clone(),
            rev: app.session.rev(),
            document: app.session.document_id(),
            latex: None,
        };
        app.canvas.paste_options_restore = Some(restore_state);
        let mut dialog_params = params;
        if let Some(object) = dialog_params.as_object_mut() {
            // The dialog needs to list the formats, not paste one immediately.
            object.remove("as");
        }
        if app.run("edit.pasteSpecial", dialog_params).is_err() {
            restore(app);
        }
        // The Paste Special dialog owns the keyboard while it is open.
        return;
    }
    let id = match choice {
        Choice::Keep => "edit.paste",
        Choice::Merge => "edit.pasteMerge",
        Choice::Text => "edit.pasteText",
        Choice::Equation | Choice::Special => return,
    };
    let fallback = (command, params.clone());
    if app.run(id, params).is_err() {
        // The other form could not paste (protection, an empty clipboard format): put the
        // original paste back rather than leaving the undo with nothing in its place.
        let _ = app.run(&fallback.0, fallback.1);
    }
    app.canvas.want_focus = true;
}

/// Put back the paste a cancelled Paste Special dialog took away. Also used when the dialog's OK
/// cannot paste anything (its format list is empty).
pub(crate) fn restore(app: &mut WordApp) {
    let Some(state) = app.canvas.paste_options_restore.take() else { return };
    let _ = app.run(&state.command, state.params);
}

/// Where the button goes for a caret line: below and just right of the caret, flipped above when
/// the bottom of the viewport is too close, and always inside the canvas. Pure geometry.
pub(crate) fn anchor_rect(caret: Pos2, line_height: f32, viewport: Rect) -> Option<Rect> {
    if !caret.x.is_finite() || !caret.y.is_finite() || !line_height.is_finite() || !viewport.is_finite() || !viewport.is_positive() {
        return None;
    }
    if caret.y < viewport.top() || caret.y > viewport.bottom() {
        return None;
    }
    let x = (caret.x + GAP).clamp(viewport.left(), (viewport.right() - W).max(viewport.left()));
    let below = caret.y + line_height.max(0.0) + GAP;
    let above = caret.y - H - GAP;
    let y = if below + H <= viewport.bottom() {
        below
    } else if above >= viewport.top() {
        above
    } else {
        (viewport.bottom() - H).max(viewport.top())
    };
    Some(Rect::from_min_size(pos2(x, y), vec2(W, H)))
}

/// Draw the button and its menu inside the canvas viewport.
pub(crate) fn show(app: &mut WordApp, ctx: &egui::Context, viewport: Rect) {
    dismiss_if_stale(app);
    if !visible(app) {
        return;
    }
    let Some((caret, line_height)) = crate::canvas::caret_screen(app) else { return };
    let Some(rect) = anchor_rect(caret, line_height, viewport) else { return };
    let current = app.canvas.paste_options.as_ref().map(|s| s.choice).unwrap_or(Choice::Keep);
    let area = egui::Area::new(egui::Id::new("wc_paste_options")).order(Order::Tooltip).interactable(true).fixed_pos(rect.min);
    area.show(ctx, |ui| {
        let t = Tokens::get(ctx);
        let (r, resp) = ui.allocate_exact_size(rect.size(), Sense::click());
        let fill = if resp.hovered() { t.hover } else { t.menu };
        let painter = ui.painter();
        painter.rect_filled(r.translate(vec2(0.0, 1.0)), CornerRadius::same(4), t.page_shadow);
        painter.rect_filled(r, CornerRadius::same(4), fill);
        painter.rect_stroke(r, CornerRadius::same(4), Stroke::new(1.0, t.border_strong), StrokeKind::Inside);
        icons::paint(painter, Rect::from_center_size(r.center(), vec2(16.0, 16.0)), "paste", t.icon, t.accent);
        let resp = resp.on_hover_text(tl!("Paste Options"));
        egui::Popup::menu(&resp).show(|ui| {
            ui.set_min_width(240.0);
            menu(ui, app, current);
        });
    });
}

fn menu(ui: &mut egui::Ui, app: &mut WordApp, current: Choice) {
    for choice in [Choice::Keep, Choice::Merge, Choice::Text] {
        let mark = egui::RichText::new("✓");
        let mark = if current == choice { mark } else { mark.color(Color32::TRANSPARENT) };
        let resp = ui.add(egui::Button::new((mark, tl!(choice.label()))).selected(current == choice).min_size(vec2(230.0, 0.0)));
        if resp.clicked() {
            choose(app, choice);
            ui.close();
        }
    }
    // A formula pasted from a note can become an equation.
    if app.canvas.paste_options.as_ref().is_some_and(|s| s.latex.is_some()) {
        ui.separator();
        if ui.button(tl!(Choice::Equation.label())).clicked() {
            choose(app, Choice::Equation);
            ui.close();
        }
    }
    ui.separator();
    if ui.button(tl!(Choice::Special.label())).clicked() {
        choose(app, Choice::Special);
        ui.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Services;
    use serde_json::json;
    use wordcraft_engine::Session;

    fn app() -> WordApp {
        WordApp::new(Session::new(wordcraft_doc::Document::new()), Services::default())
    }

    fn viewport() -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0))
    }

    #[test]
    fn the_button_sits_below_and_just_right_of_the_caret() {
        let r = anchor_rect(pos2(200.0, 200.0), 14.0, viewport()).unwrap();
        assert_eq!(r.min, pos2(202.0, 216.0));
        assert_eq!(r.size(), vec2(W, H));
    }

    #[test]
    fn the_button_flips_above_a_caret_at_the_bottom_and_stays_inside() {
        let r = anchor_rect(pos2(796.0, 590.0), 14.0, viewport()).unwrap();
        assert!(r.max.y <= viewport().bottom());
        assert!(r.min.y < 590.0, "no room below: it goes above the caret");
        assert!(r.max.x <= viewport().right());
    }

    #[test]
    fn a_caret_outside_the_viewport_or_a_bad_rect_shows_nothing() {
        assert!(anchor_rect(pos2(20.0, -1.0), 14.0, viewport()).is_none());
        assert!(anchor_rect(pos2(20.0, 601.0), 14.0, viewport()).is_none());
        assert!(anchor_rect(pos2(f32::NAN, 20.0), 14.0, viewport()).is_none());
        assert!(anchor_rect(pos2(20.0, 20.0), f32::INFINITY, viewport()).is_none());
        assert!(anchor_rect(pos2(20.0, 20.0), 14.0, Rect::NOTHING).is_none());
    }

    #[test]
    fn choosing_keep_text_only_replaces_the_pasted_range_without_duplicating_it() {
        let mut a = app();
        a.run("edit.paste", json!({"text": "plain strong", "html": "<p>plain <b>strong</b></p>"})).unwrap();
        assert!(a.canvas.paste_options.is_some(), "a paste shows the button");
        choose(&mut a, Choice::Text);
        let p = a.session.doc.para_at(&a.session.sel.focus).unwrap();
        assert_eq!(p.text, "plain strong");
        assert!(!p.props_of_char(7).bold.unwrap_or(false), "Keep Text Only drops the source's bold");
        assert_eq!(a.session.doc.plain_text(wordcraft_engine::doc::StoryRef::Body), "plain strong");
        assert_eq!(a.canvas.paste_options.as_ref().map(|s| s.choice), Some(Choice::Text), "the replacement paste shows it again");
    }

    /// A formula pasted from a note can become an equation: the button offers that form.
    #[test]
    fn a_formula_offers_the_equation_form() {
        let mut a = app();
        a.run("edit.paste", json!({"text": "$x^2 + 1$"})).unwrap();
        assert!(a.canvas.paste_options.as_ref().and_then(|s| s.latex.as_ref()).is_some(), "the paste was a formula");
        choose(&mut a, Choice::Equation);
        let para = a.session.doc.para_at(&a.session.sel.focus).unwrap();
        assert!(para.objects.iter().any(|o| matches!(o, wordcraft_doc::InlineObject::Equation { .. })), "the formula is an equation now");
    }

    /// A price is not a formula, so the button does not offer the equation form.
    #[test]
    fn a_price_does_not_offer_the_equation_form() {
        let mut a = app();
        a.run("edit.paste", json!({"text": "It costs $5 and $10"})).unwrap();
        assert!(a.canvas.paste_options.as_ref().is_some_and(|s| s.latex.is_none()), "a price is not a formula");
    }

    #[test]
    fn moving_the_caret_dismisses_the_button() {
        let mut a = app();
        a.run("edit.paste", json!({"text": "pasted"})).unwrap();
        assert!(visible(&a));
        a.run("select.all", json!({})).unwrap();
        assert!(a.canvas.paste_options.is_none());
    }

    #[test]
    fn listing_paste_special_formats_does_not_show_the_button() {
        let mut a = app();
        let r = a.run("edit.pasteSpecial", json!({"text": "x"})).unwrap();
        assert!(r.get("formats").is_some());
        assert!(a.canvas.paste_options.is_none());
    }

    #[test]
    fn a_cancelled_paste_special_dialog_puts_the_original_paste_back() {
        let mut a = app();
        a.run("edit.paste", json!({"text": "pasted"})).unwrap();
        choose(&mut a, Choice::Special);
        assert!(a.canvas.paste_options_restore.is_some(), "the dialog can be cancelled");
        assert_eq!(a.session.doc.plain_text(wordcraft_engine::doc::StoryRef::Body), "", "the paste was undone while the dialog is open");
        restore(&mut a);
        assert_eq!(a.session.doc.plain_text(wordcraft_engine::doc::StoryRef::Body), "pasted");
        assert!(a.canvas.paste_options.is_some(), "the restored paste shows the button again");
    }
}

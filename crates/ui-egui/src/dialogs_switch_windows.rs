//! Switch Windows: the list of open documents, so the user can switch the active one.

use egui::{RichText, Vec2};
use serde::Serialize;

use crate::WordApp;

/// The switch-windows list: open documents, most recent first.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SwitchWindows {
    /// Open documents, in the order File > Open / Recent would list them.
    documents: Vec<String>,
    /// Currently selected entry (so the dialog can be re-shown with the same selection).
    selected: usize,
    /// True keeps the dialog visible on the next frame.
    need_show: bool,
}

impl SwitchWindows {
    pub fn new(documents: Vec<String>) -> Self {
        let selected = documents.iter().position(|p| p == documents.first().unwrap_or(&String::new()));
        Self { documents, selected: selected.unwrap_or(0), need_show: true }
    }

    pub fn documents(&self) -> &[String] {
        &self.documents
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn set_selected(&mut self, selected: usize) {
        self.selected = selected;
    }

    pub fn show(&mut self, app: &mut WordApp, ui: &mut egui::Ui) {
        if self.documents.is_empty() {
            ui.label(tl!("No documents are open."));
            return;
        }
        let title = tl!("Switch Windows");
        let content_height = (self.documents.len() as f32).clamp(60.0, 320.0);

        // Opening "Switch" switches the active window to the selected document.
        let mut switch_requested = false;
        let mut pending = self.need_show;
        egui::Window::new(title)
            .open(&mut pending)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .collapsible(false)
            .resizable(false)
            .default_height(content_height + 30.0)
            .show(ui.ctx(), |ui| {
                let t = crate::theme::Tokens::get(ui.ctx());
                for (i, path) in self.documents.iter().enumerate() {
                    let is_selected = i == self.selected;
                    let label = path.split('/').next_back().unwrap_or(path);
                    let text = RichText::new(label).size(13.0);
                    let button = egui::Button::new(text).fill(if is_selected { t.accent } else { t.panel }).min_size(Vec2::new(0.0, 24.0));
                    let resp = ui.add_sized(Vec2::INFINITY, button);
                    if resp.clicked() {
                        self.selected = i;
                    }
                }
                ui.add_space(4.0);
                if ui.button(tl!("Switch")).clicked() {
                    self.need_show = false;
                    switch_requested = true;
                }
            });

        if switch_requested {
            let current = app.session.path.as_ref().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
            let target = self.documents.get(self.selected).cloned();
            if let Some(p) = target.filter(|p| p != &current) {
                let _ = app.session.run("file.open", &serde_json::json!({ "path": p }));
            }
            self.need_show = false;
        } else {
            self.need_show = pending;
        }
    }
}

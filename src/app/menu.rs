use super::GraphApp;
use eframe::egui::{self, Rect};
use rfd::FileDialog;

impl GraphApp {
    fn menu_item_matches(&self, label: &str) -> bool {
        let kw = self.menu_search_text.trim();
        if kw.is_empty() {
            return true;
        }

        label.to_lowercase().contains(&kw.to_lowercase())
    }

    fn menu_item_highlighted_label(&self, label: &str, normal_color: egui::Color32) -> egui::text::LayoutJob {
        let kw = self.menu_search_text.trim();
        let mut job = egui::text::LayoutJob::default();

        let mut normal = egui::TextFormat::default();
        normal.color = normal_color;

        if kw.is_empty() {
            job.append(label, 0.0, normal.clone());
            return job;
        }

        let mut highlight = egui::TextFormat::default();
        highlight.color = egui::Color32::from_rgb(255, 196, 0);

        let mut last = 0;
        for (start, matched) in label.match_indices(kw) {
            if start > last {
                job.append(&label[last..start], 0.0, normal.clone());
            }
            job.append(matched, 0.0, highlight.clone());
            last = start + matched.len();
        }

        if last < label.len() {
            job.append(&label[last..], 0.0, normal);
        }

        job
    }

    pub(in crate::app) fn reset_menu_search_state(&mut self, request_focus: bool) {
        self.menu_search_text.clear();
        self.menu_search_selected = 0;
        self.pending_menu_search_focus = request_focus;
    }

    pub(in crate::app) fn show_searchable_menu_actions(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        search_id: egui::Id,
        hint_text: &str,
        items: &[(&str, &str, usize)],
        empty_text: &str,
        footer_text: &str,
    ) -> Option<usize> {
        if self.pending_menu_search_focus {
            ui.memory_mut(|m| m.request_focus(search_id));
        }

        let search_resp = ui.add_sized(
            [ui.available_width(), 24.0],
            egui::TextEdit::singleline(&mut self.menu_search_text)
                .id(search_id)
                .hint_text(hint_text),
        );
        let search_has_focus = search_resp.has_focus() || ui.memory(|m| m.has_focus(search_id));
        if self.pending_menu_search_focus && search_has_focus {
            self.pending_menu_search_focus = false;
        }
        if search_resp.changed() {
            self.menu_search_selected = 0;
        }

        ui.separator();

        let mut matched = Vec::new();
        for (path, label, action_id) in items {
            if self.menu_item_matches(path) || self.menu_item_matches(label) {
                matched.push((*label, *action_id));
            }
        }

        if matched.is_empty() {
            ui.small(empty_text);
            return None;
        }

        if self.menu_search_selected >= matched.len() {
            self.menu_search_selected = matched.len().saturating_sub(1);
        }

        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
            self.menu_search_selected = (self.menu_search_selected + 1) % matched.len();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
            self.menu_search_selected =
                (self.menu_search_selected + matched.len() - 1) % matched.len();
        }

        let mut trigger_action = None;
        for (row, (label, action_id)) in matched.iter().enumerate() {
            let selected = row == self.menu_search_selected;
            let normal_color = ui.visuals().widgets.inactive.fg_stroke.color;
            let resp = ui.add_sized(
                [ui.available_width(), 24.0],
                egui::Button::new(self.menu_item_highlighted_label(label, normal_color)).selected(selected),
            );
            if resp.hovered() {
                self.menu_search_selected = row;
            }
            if resp.clicked() {
                trigger_action = Some(*action_id);
            }
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            trigger_action = Some(matched[self.menu_search_selected].1);
        }

        ui.separator();
        ui.small(footer_text);

        trigger_action
    }

    pub(in crate::app) fn should_close_popup(
        &self,
        ctx: &egui::Context,
        popup_rect: Option<Rect>,
        action_triggered: bool,
    ) -> bool {
        if action_triggered {
            return true;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            return true;
        }

        let Some(rect) = popup_rect else {
            return false;
        };

        ctx.input(|i| {
            if !i.pointer.any_pressed() {
                return false;
            }
            let Some(pos) = i.pointer.interact_pos() else {
                return false;
            };
            !rect.contains(pos)
        })
    }

    fn save_graph_with_dialog(&self) {
        let Some(path) = FileDialog::new()
            .add_filter("Graph JSON", &["json"])
            .set_file_name("graph.json")
            .save_file()
        else {
            return;
        };

        if let Err(err) = self.save_graph_to_path(&path) {
            eprintln!("save graph failed: {err}");
        }
    }

    fn load_graph_with_dialog(&mut self) {
        let Some(path) = FileDialog::new()
            .add_filter("Graph JSON", &["json"])
            .pick_file()
        else {
            return;
        };

        if let Err(err) = self.load_graph_from_path(&path) {
            eprintln!("load graph failed: {err}");
        }
    }

    pub(in crate::app) fn run_file_menu_action(&mut self, action_id: usize) {
        match action_id {
            0 => {
                if let Err(err) = self.save_graph_to_default_path() {
                    eprintln!("save graph failed: {err}");
                }
            }
            1 => {
                if let Err(err) = self.load_graph_from_default_path() {
                    eprintln!("load graph failed: {err}");
                }
            }
            2 => self.save_graph_with_dialog(),
            3 => self.load_graph_with_dialog(),
            _ => {}
        }
    }
}

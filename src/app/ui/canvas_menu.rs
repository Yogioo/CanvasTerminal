use super::super::GraphApp;
use eframe::egui::{self, vec2, Pos2, TextEdit};

impl GraphApp {
    fn run_create_action(&mut self, action_id: usize, spawn_pos: Pos2) {
        match action_id {
            0 => self.create_terminal_node(spawn_pos),
            1 => self.create_text_node(spawn_pos, true),
            _ => {}
        }
    }

    fn context_menu_spawn_pos(&self) -> Pos2 {
        if let Some(id) = self.context_menu_node {
            if let Some(node) = self.nodes.iter().find(|n| n.id == id) {
                return node.pos + vec2(node.size.x + 40.0, 10.0);
            }
        }

        self.context_menu_local_pos.unwrap_or(Pos2::new(100.0, 100.0))
    }

    pub(in crate::app::ui) fn show_canvas_context_menu(&mut self, response: &egui::Response, ctx: &egui::Context) {
        response.context_menu(|ui| {
            let search_id = egui::Id::new("context_menu_search_input");
            if self.pending_menu_search_focus {
                ui.memory_mut(|m| m.request_focus(search_id));
            }

            let search_resp = ui.add(
                TextEdit::singleline(&mut self.menu_search_text)
                    .id(search_id)
                    .hint_text("搜索并创建节点..."),
            );
            let search_has_focus = search_resp.has_focus() || ui.memory(|m| m.has_focus(search_id));
            if self.pending_menu_search_focus && search_has_focus {
                self.pending_menu_search_focus = false;
            }
            if search_resp.changed() {
                self.menu_search_selected = 0;
                if self.menu_search_text.trim().is_empty() {
                    self.menu_nav_level = 0;
                    self.menu_nav_selected = 0;
                }
            }

            ui.separator();

            let spawn_pos = self.context_menu_spawn_pos();

            if self.menu_search_text.trim().is_empty() {
                let actions = [("终端节点", 0usize), ("文本节点", 1usize)];
                if self.menu_nav_selected >= actions.len() {
                    self.menu_nav_selected = actions.len().saturating_sub(1);
                }

                if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) && self.menu_nav_level >= 1 {
                    self.menu_nav_selected = (self.menu_nav_selected + 1) % actions.len();
                }
                if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) && self.menu_nav_level >= 1 {
                    self.menu_nav_selected =
                        (self.menu_nav_selected + actions.len() - 1) % actions.len();
                }
                if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                    self.menu_nav_level = (self.menu_nav_level + 1).min(1);
                }
                if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                    self.menu_nav_level = self.menu_nav_level.saturating_sub(1);
                }

                let mut trigger_action = None;
                if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                    match self.menu_nav_level {
                        0 => self.menu_nav_level = 1,
                        1 => trigger_action = Some(actions[self.menu_nav_selected].1),
                        _ => {}
                    }
                }

                ui.group(|ui| {
                    if ui
                        .add_sized(
                            [170.0, 24.0],
                            egui::SelectableLabel::new(self.menu_nav_level == 0, "创建节点 ▶"),
                        )
                        .clicked()
                    {
                        self.menu_nav_level = 1;
                    }

                    if self.menu_nav_level >= 1 {
                        ui.indent("menu_level_1", |ui| {
                            for (idx, (label, action_id)) in actions.iter().enumerate() {
                                let selected = self.menu_nav_selected == idx;
                                if ui
                                    .add_sized(
                                        [170.0, 24.0],
                                        egui::SelectableLabel::new(selected, *label),
                                    )
                                    .clicked()
                                {
                                    self.menu_nav_selected = idx;
                                    trigger_action = Some(*action_id);
                                }
                            }
                        });
                    }
                });

                if let Some(action_id) = trigger_action {
                    self.run_create_action(action_id, spawn_pos);
                    ui.close_menu();
                }

                ui.separator();
                ui.small("←/→ 进入或返回，↑/↓ 选择，Enter 创建");
                return;
            }

            let items = [
                ("创建节点/终端节点", "终端节点", 0usize),
                ("创建节点/文本节点", "文本节点", 1usize),
            ];

            let mut matched = Vec::new();
            for (path, label, action_id) in items {
                if self.menu_item_matches(path) || self.menu_item_matches(label) {
                    matched.push((label, action_id));
                }
            }

            if matched.is_empty() {
                ui.small("无匹配节点类型");
                return;
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
            for (row, (path, action_id)) in matched.iter().enumerate() {
                let selected = row == self.menu_search_selected;
                let resp = ui.selectable_label(selected, self.menu_item_highlighted_label(path));
                if resp.clicked() {
                    trigger_action = Some(*action_id);
                }
            }

            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                trigger_action = Some(matched[self.menu_search_selected].1);
            }

            if let Some(action_id) = trigger_action {
                self.run_create_action(action_id, spawn_pos);
                ui.close_menu();
            }

            ui.separator();
            ui.small("↑/↓ 选择，Enter 创建");
        });
    }
}

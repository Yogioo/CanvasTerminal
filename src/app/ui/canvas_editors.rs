use super::super::GraphApp;
use crate::model::NodeData;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, TextEdit, Ui};
use egui_term::{TerminalFont, TerminalView};

impl GraphApp {
    pub(in crate::app::ui) fn handle_text_node_editor(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        text_edit_rect: Option<(usize, Rect)>,
    ) {
        let Some((id, edit_rect)) = text_edit_rect else {
            return;
        };

        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == id) {
            let Some(text_body) = (match &mut node.data {
                NodeData::Text { text_body, .. } => Some(text_body),
                _ => None,
            }) else {
                return;
            };

            let text_edit_id = egui::Id::new(("text-node-editor", id));
            let should_focus_and_select_all = self.pending_text_focus == Some(id);
            if should_focus_and_select_all {
                ctx.memory_mut(|m| m.request_focus(text_edit_id));
            }

            let desired_rows = text_body.split('\n').count().max(1);
            let text_edit = TextEdit::multiline(text_body)
                .id(text_edit_id)
                .font(FontId::proportional(15.0 * self.zoom))
                .text_color(Color32::from_rgb(250, 240, 210))
                .margin(egui::Margin::ZERO)
                .desired_width(f32::INFINITY)
                .desired_rows(desired_rows)
                .frame(false);
            let resp = ui.put(edit_rect, text_edit);

            if should_focus_and_select_all {
                if let Some(mut state) = egui::TextEdit::load_state(ctx, text_edit_id) {
                    let len = text_body.chars().count();
                    let range = egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(len),
                    );
                    state.cursor.set_char_range(Some(range));
                    state.store(ctx, text_edit_id);
                }
                self.pending_text_focus = None;
            }

            if resp.changed() {
                self.mark_workspace_dirty();
            }

            if resp.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.editing_text_node = None;
            }
        }
    }

    pub(in crate::app::ui) fn handle_title_editor(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        title_edit_rect: Option<(usize, Rect)>,
        primary_clicked: bool,
        pointer_pos: Option<Pos2>,
    ) {
        let Some((id, edit_rect)) = title_edit_rect else {
            return;
        };

        let title_edit_id = egui::Id::new(("terminal-title-editor", id));
        let should_focus_and_select_all = self.pending_title_focus == Some(id);
        if should_focus_and_select_all {
            ctx.memory_mut(|m| m.request_focus(title_edit_id));
        }

        let text_edit = TextEdit::singleline(&mut self.title_edit_buffer)
            .id(title_edit_id)
            .font(FontId::proportional((16.0 * self.zoom).max(9.0)))
            .text_color(Color32::WHITE)
            .desired_width(f32::INFINITY)
            .frame(false);
        let resp = ui.put(edit_rect, text_edit);

        if should_focus_and_select_all {
            if let Some(mut state) = egui::TextEdit::load_state(ctx, title_edit_id) {
                let len = self.title_edit_buffer.chars().count();
                let range = egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(len),
                );
                state.cursor.set_char_range(Some(range));
                state.store(ctx, title_edit_id);
            }
            self.pending_title_focus = None;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.cancel_title_edit();
        } else if ctx.input(|i| i.key_pressed(egui::Key::Enter))
            || (resp.lost_focus() && !ctx.input(|i| i.pointer.primary_down()))
        {
            self.commit_title_edit(id);
        } else if primary_clicked {
            if let Some(pointer) = pointer_pos {
                if !edit_rect.contains(pointer) {
                    self.commit_title_edit(id);
                }
            }
        }
    }

    pub(in crate::app::ui) fn handle_startup_editor(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        startup_edit_rect: Option<(usize, Rect)>,
        primary_clicked: bool,
        pointer_pos: Option<Pos2>,
    ) {
        let Some((id, edit_rect)) = startup_edit_rect else {
            return;
        };

        let startup_edit_id = egui::Id::new(("terminal-startup-editor", id));
        let should_focus_and_select_all = self.pending_startup_focus == Some(id);
        if should_focus_and_select_all {
            ctx.memory_mut(|m| m.request_focus(startup_edit_id));
        }

        let desired_rows = self.startup_edit_buffer.lines().count().max(4);
        let text_edit = TextEdit::multiline(&mut self.startup_edit_buffer)
            .id(startup_edit_id)
            .font(FontId::monospace((13.0 * self.zoom).max(9.0)))
            .text_color(Color32::from_rgb(238, 235, 255))
            .desired_width(f32::INFINITY)
            .desired_rows(desired_rows)
            .frame(true);
        let resp = ui.put(edit_rect, text_edit);

        if should_focus_and_select_all {
            if let Some(mut state) = egui::TextEdit::load_state(ctx, startup_edit_id) {
                let len = self.startup_edit_buffer.chars().count();
                let range = egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(len),
                );
                state.cursor.set_char_range(Some(range));
                state.store(ctx, startup_edit_id);
            }
            self.pending_startup_focus = None;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.commit_startup_edit(id, ctx);
        } else if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter))
            || (resp.lost_focus() && !ctx.input(|i| i.pointer.primary_down()))
        {
            self.commit_startup_edit(id, ctx);
        } else if primary_clicked {
            if let Some(pointer) = pointer_pos {
                if !edit_rect.contains(pointer) {
                    self.commit_startup_edit(id, ctx);
                }
            }
        }
    }

    pub(in crate::app::ui) fn draw_embedded_terminal_for_rect(
        &mut self,
        ui: &mut Ui,
        _ctx: &egui::Context,
        canvas_rect: Rect,
        node_id: usize,
        term_rect: Rect,
    ) {
        let is_terminal_focused = self.selected == Some(node_id)
            && self.editing_title_node != Some(node_id)
            && self.editing_startup_node != Some(node_id)
            && self.suspend_terminal_focus != Some(node_id);

        if let Some(backend) = self.terminal_backends.get(&node_id) {
            // 选中终端高频刷新，未选中终端降频，降低多终端高输出时卡顿。
            let min_repaint_interval_ms = if is_terminal_focused { 16 } else { 80 };
            backend.set_min_repaint_interval_ms(min_repaint_interval_ms);
        }

        let visible_rect = term_rect.intersect(canvas_rect);
        if !visible_rect.is_positive() {
            return;
        }

        if !self.terminal_backends.contains_key(&node_id)
            && !self.terminal_errors.contains_key(&node_id)
            && !self.terminal_exited.contains(&node_id)
        {
            self.queue_terminal_start(node_id);
        }

        let full_screen_rect = Rect::from_min_size(term_rect.min, term_rect.size());
        let mut term_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(full_screen_rect)
                .layout(*ui.layout()),
        );
        term_ui.set_clip_rect(visible_rect);

        if let Some(err) = self.terminal_errors.get(&node_id) {
            term_ui.colored_label(Color32::LIGHT_RED, err);
        } else if let Some(backend) = self.terminal_backends.get_mut(&node_id) {
            let term_font_size = (14.0 * self.zoom).clamp(9.0, 32.0).round();
            let term_font = TerminalFont::new(egui_term::FontSettings {
                font_type: FontId::monospace(term_font_size),
            });
            let term = TerminalView::new(&mut term_ui, backend)
                .set_focus(is_terminal_focused)
                .set_font(term_font)
                .set_size(term_rect.size());
            term_ui.add(term);
        } else if self.pending_terminal_starts.contains(&node_id) {
            term_ui.label("终端启动中...");
        } else {
            term_ui.label("终端未启动，请稍候或通过节点菜单重启。");
        }
    }

}

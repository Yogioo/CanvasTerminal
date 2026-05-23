use super::canvas_multiline_editor::{show_canvas_multiline_editor, GutterLine};
use super::super::GraphApp;
use crate::model::NodeData;
use arboard::Clipboard;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, TextEdit, Ui};


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

        if let Some(node) = self.ws.nodes.iter_mut().find(|n| n.id == id) {
            let (text_body, text_color) = match &mut node.data {
                NodeData::Text { text_body, .. } => {
                    (text_body, Color32::from_rgb(250, 240, 210))
                }

                _ => return,
            };

            let text_edit_id = egui::Id::new(("text-node-editor", id));
            let should_focus_and_select_all = self.ws.pending_text_focus == Some(id);
            if should_focus_and_select_all {
                ctx.memory_mut(|m| m.request_focus(text_edit_id));
            }

            let pre_edit_state = egui::TextEdit::load_state(ctx, text_edit_id);
            let pre_edit_char_range = pre_edit_state.as_ref().and_then(|s| {
                s.cursor.char_range().map(|r| {
                    let a = r.primary.index;
                    let b = r.secondary.index;
                    if a <= b { a..b } else { b..a }
                })
            });
            let output = show_canvas_multiline_editor(
                ui,
                edit_rect,
                ("text-node-editor-scroll", id),
                text_edit_id,
                text_body,
                FontId::proportional(15.0 * self.ws.zoom),
                text_color,
                None,
                None,
                |_| GutterLine {
                    line: 0,
                    marker: "",
                    color: Color32::TRANSPARENT,
                },
                None,
            );
            let resp = output.response;
            if output.pointer_over_editor {
                self.ws.context_menu_local_pos = None;
                self.ws.context_menu_node = None;
                self.ws.context_menu_edge = None;
                self.ws.context_menu_open = false;

            }

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
                self.ws.pending_text_focus = None;
            }

            if output.pointer_over_editor
                && ctx.input(|i| {
                    i.pointer.button_pressed(egui::PointerButton::Secondary)
                        || i.pointer.button_down(egui::PointerButton::Secondary)
                        || i.pointer.button_released(egui::PointerButton::Secondary)
                })
            {
                if let Some(state) = pre_edit_state {
                    state.store(ctx, text_edit_id);
                }
            }

            if output.pointer_over_editor
                && ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary))
            {
                self.ws.text_context_menu_selection = pre_edit_char_range.clone().map(|r| (id, r));
                self.ws.text_context_menu_screen_pos = ctx.input(|i| {
                    i.pointer
                        .latest_pos()
                        .or_else(|| i.pointer.interact_pos())
                        .or_else(|| i.pointer.hover_pos())
                });
            }

            let mut text_menu_changed = false;
            let mut close_text_menu = false;
            if let Some(screen_pos) = self.ws.text_context_menu_screen_pos {
                let char_to_byte = |s: &str, char_idx: usize| -> usize {
                    s.char_indices()
                        .nth(char_idx)
                        .map(|(i, _)| i)
                        .unwrap_or_else(|| s.len())
                };
                let saved_range = self.ws
                    .text_context_menu_selection
                    .as_ref()
                    .and_then(|(node_id, r)| (*node_id == id).then_some(r.clone()));
                let has_selection = saved_range.as_ref().is_some_and(|r| r.start < r.end);
                let mut copied_from_menu = false;

                let area_out = egui::Area::new(egui::Id::new(("text-node-context-menu", id)))
                    .order(egui::Order::Foreground)
                    .fixed_pos(screen_pos + egui::vec2(6.0, 6.0))
                    .show(ctx, |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            if ui
                                .add_enabled(has_selection, egui::Button::new("复制"))
                                .clicked()
                            {
                                if let Some(r) = saved_range.as_ref().filter(|r| r.start < r.end) {
                                    let start = char_to_byte(text_body, r.start);
                                    let end = char_to_byte(text_body, r.end);
                                    if start <= end && end <= text_body.len() {
                                        ctx.copy_text(text_body[start..end].to_owned());
                                        copied_from_menu = true;
                                    }
                                }
                                close_text_menu = true;
                            }

                            if ui
                                .add_enabled(has_selection, egui::Button::new("剪切"))
                                .clicked()
                            {
                                if let Some(r) = saved_range.as_ref().filter(|r| r.start < r.end) {
                                    let start = char_to_byte(text_body, r.start);
                                    let end = char_to_byte(text_body, r.end);
                                    if start <= end && end <= text_body.len() {
                                        let cut = text_body[start..end].to_owned();
                                        ctx.copy_text(cut);
                                        text_body.replace_range(start..end, "");
                                        text_menu_changed = true;
                                    }
                                }
                                close_text_menu = true;
                            }

                            if ui.button("粘贴").clicked() {
                                if let Ok(mut clipboard) = Clipboard::new() {
                                    if let Ok(paste) = clipboard.get_text() {
                                        if let Some(r) = saved_range.as_ref() {
                                            let start = char_to_byte(text_body, r.start);
                                            let end = char_to_byte(text_body, r.end);
                                            if start <= end && end <= text_body.len() {
                                                text_body.replace_range(start..end, &paste);
                                                text_menu_changed = true;
                                            }
                                        } else {
                                            text_body.push_str(&paste);
                                            text_menu_changed = true;
                                        }
                                    }
                                }
                                close_text_menu = true;
                            }
                        });
                    });

                if copied_from_menu {
                    if let Some((node_id, range)) = self.ws.text_context_menu_selection.clone() {
                        if node_id == id {
                            if let Some(mut st) = egui::TextEdit::load_state(ctx, text_edit_id) {
                                st.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                                    egui::text::CCursor::new(range.start),
                                    egui::text::CCursor::new(range.end),
                                )));
                                st.store(ctx, text_edit_id);
                            }
                        }
                    }
                }

                let clicked_outside = ctx.input(|i| {
                    i.pointer.button_pressed(egui::PointerButton::Primary)
                        && i.pointer
                            .interact_pos()
                            .is_some_and(|p| !area_out.response.rect.contains(p))
                });
                if clicked_outside {
                    close_text_menu = true;
                }
            }

            if close_text_menu {
                self.ws.text_context_menu_selection = None;
                self.ws.text_context_menu_screen_pos = None;
            }

            if text_menu_changed {
                self.mark_workspace_dirty();
            }

            if resp.changed() {
                self.mark_workspace_dirty();
            }

            // Text 节点：仅 Escape 退出编辑。
            if resp.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.ws.editing_text_node = None;
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
        let should_focus_and_select_all = self.ws.pending_title_focus == Some(id);
        if should_focus_and_select_all {
            ctx.memory_mut(|m| m.request_focus(title_edit_id));
        }

        let text_edit = TextEdit::singleline(&mut self.ws.title_edit_buffer)
            .id(title_edit_id)
            .font(FontId::proportional((16.0 * self.ws.zoom).max(9.0)))
            .text_color(Color32::WHITE)
            .desired_width(f32::INFINITY)
            .frame(false);
        let resp = ui.put(edit_rect, text_edit);

        if should_focus_and_select_all {
            if let Some(mut state) = egui::TextEdit::load_state(ctx, title_edit_id) {
                let len = self.ws.title_edit_buffer.chars().count();
                let range = egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(len),
                );
                state.cursor.set_char_range(Some(range));
                state.store(ctx, title_edit_id);
            }
            self.ws.pending_title_focus = None;
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
        let should_focus_and_select_all = self.ws.pending_startup_focus == Some(id);
        if should_focus_and_select_all {
            ctx.memory_mut(|m| m.request_focus(startup_edit_id));
        }

        let desired_rows = self.ws.startup_edit_buffer.lines().count().max(4);
        let text_edit = TextEdit::multiline(&mut self.ws.startup_edit_buffer)
            .id(startup_edit_id)
            .font(FontId::monospace((13.0 * self.ws.zoom).max(9.0)))
            .text_color(Color32::WHITE)
            .background_color(Color32::BLACK)
            .desired_width(f32::INFINITY)
            .desired_rows(desired_rows)
            .frame(true);
        let resp = ui
            .scope(|ui| {
                let style = ui.style_mut();
                style.visuals.override_text_color = Some(Color32::WHITE);
                style.visuals.widgets.inactive.bg_fill = Color32::BLACK;
                style.visuals.widgets.hovered.bg_fill = Color32::BLACK;
                style.visuals.widgets.active.bg_fill = Color32::BLACK;
                style.visuals.widgets.inactive.fg_stroke.color = Color32::WHITE;
                style.visuals.widgets.hovered.fg_stroke.color = Color32::WHITE;
                style.visuals.widgets.active.fg_stroke.color = Color32::WHITE;
                ui.put(edit_rect, text_edit)
            })
            .inner;

        if should_focus_and_select_all {
            if let Some(mut state) = egui::TextEdit::load_state(ctx, startup_edit_id) {
                let len = self.ws.startup_edit_buffer.chars().count();
                let range = egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(len),
                );
                state.cursor.set_char_range(Some(range));
                state.store(ctx, startup_edit_id);
            }
            self.ws.pending_startup_focus = None;
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

    pub(in crate::app::ui) fn handle_working_directory_editor(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        working_directory_edit_rect: Option<(usize, Rect)>,
        primary_clicked: bool,
        pointer_pos: Option<Pos2>,
    ) {
        let Some((id, edit_rect)) = working_directory_edit_rect else {
            return;
        };

        let editor_id = egui::Id::new(("terminal-working-directory-editor", id));
        let should_focus_and_select_all = self.ws.pending_working_directory_focus == Some(id);
        if should_focus_and_select_all {
            ctx.memory_mut(|m| m.request_focus(editor_id));
        }

        let text_edit = TextEdit::singleline(&mut self.ws.working_directory_edit_buffer)
            .id(editor_id)
            .font(FontId::monospace((13.0 * self.ws.zoom).max(9.0)))
            .text_color(Color32::WHITE)
            .background_color(Color32::BLACK)
            .hint_text("working_directory (留空=默认cwd)")
            .desired_width(f32::INFINITY)
            .frame(true);
        let resp = ui
            .scope(|ui| {
                let style = ui.style_mut();
                style.visuals.override_text_color = Some(Color32::WHITE);
                style.visuals.widgets.inactive.bg_fill = Color32::BLACK;
                style.visuals.widgets.hovered.bg_fill = Color32::BLACK;
                style.visuals.widgets.active.bg_fill = Color32::BLACK;
                style.visuals.widgets.inactive.fg_stroke.color = Color32::WHITE;
                style.visuals.widgets.hovered.fg_stroke.color = Color32::WHITE;
                style.visuals.widgets.active.fg_stroke.color = Color32::WHITE;
                ui.put(edit_rect, text_edit)
            })
            .inner;

        if should_focus_and_select_all {
            if let Some(mut state) = egui::TextEdit::load_state(ctx, editor_id) {
                let len = self.ws.working_directory_edit_buffer.chars().count();
                let range = egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(len),
                );
                state.cursor.set_char_range(Some(range));
                state.store(ctx, editor_id);
            }
            self.ws.pending_working_directory_focus = None;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.cancel_working_directory_edit();
        } else if ctx.input(|i| i.key_pressed(egui::Key::Enter))
            || (resp.lost_focus() && !ctx.input(|i| i.pointer.primary_down()))
        {
            self.commit_working_directory_edit(id, ctx);
        } else if primary_clicked {
            if let Some(pointer) = pointer_pos {
                if !edit_rect.contains(pointer) {
                    self.commit_working_directory_edit(id, ctx);
                }
            }
        }
    }

    pub(in crate::app::ui) fn handle_edge_editor(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        canvas_rect: Rect,
        primary_clicked: bool,
        pointer_pos: Option<Pos2>,
    ) {
        let Some((from, to)) = self.ws.editing_edge else {
            return;
        };

        if !self.has_edge(from, to) {
            self.cancel_edge_edit();
            return;
        }

        let Some(label_world_pos) = self.edge_label_world_pos(from, to) else {
            self.cancel_edge_edit();
            return;
        };

        let center = self.world_to_screen_pos(canvas_rect, label_world_pos);
        let width = (200.0 * self.ws.zoom.clamp(0.8, 1.4)).max(140.0);
        let height = (28.0 * self.ws.zoom.clamp(0.8, 1.4)).max(24.0);
        let edit_rect = Rect::from_center_size(center, egui::vec2(width, height));

        let edge_edit_id = egui::Id::new(("edge-route-editor", from, to));
        if self.ws.pending_edge_focus == Some((from, to)) {
            ctx.memory_mut(|m| m.request_focus(edge_edit_id));
        }

        let response = ui.put(
            edit_rect,
            TextEdit::singleline(&mut self.ws.edge_edit_buffer)
                .id(edge_edit_id)
                .font(FontId::proportional((13.0 * self.ws.zoom).max(10.0)))
                .text_color(Color32::WHITE)
                .background_color(Color32::BLACK)
                .hint_text("route_key")
                .desired_width(f32::INFINITY),
        );

        if self.ws.pending_edge_focus == Some((from, to)) {
            if let Some(mut state) = egui::TextEdit::load_state(ctx, edge_edit_id) {
                let len = self.ws.edge_edit_buffer.chars().count();
                let range = egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(len),
                );
                state.cursor.set_char_range(Some(range));
                state.store(ctx, edge_edit_id);
            }
            self.ws.pending_edge_focus = None;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.cancel_edge_edit();
        } else if ctx.input(|i| i.key_pressed(egui::Key::Enter))
            || (response.lost_focus() && !ctx.input(|i| i.pointer.primary_down()))
        {
            self.commit_edge_edit();
        } else if primary_clicked {
            if let Some(pointer) = pointer_pos {
                if !edit_rect.contains(pointer) {
                    self.commit_edge_edit();
                }
            }
        }
    }
}


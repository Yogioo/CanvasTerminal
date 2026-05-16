use super::super::GraphApp;
use crate::model::NodeData;
use eframe::egui::{self, vec2, Align, Color32, FontId, Layout, Pos2, Rect, TextEdit, Ui};
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
            let (text_body, text_color) = match &mut node.data {
                NodeData::Text { text_body, .. } => {
                    (text_body, Color32::from_rgb(250, 240, 210))
                }
                NodeData::Html { html_source } => {
                    (html_source, Color32::from_rgb(220, 232, 244))
                }
                NodeData::WebPage { url } => {
                    (url, Color32::from_rgb(200, 240, 245))
                }
                _ => return,
            };

            let text_edit_id = egui::Id::new(("text-node-editor", id));
            let should_focus_and_select_all = self.pending_text_focus == Some(id);
            if should_focus_and_select_all {
                ctx.memory_mut(|m| m.request_focus(text_edit_id));
            }

            let desired_rows = text_body.split('\n').count().max(1);
            let mut text_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(edit_rect)
                    .layout(Layout::top_down(Align::Min)),
            );
            text_ui.set_clip_rect(edit_rect);

            {
                let style = text_ui.style_mut();
                let scrollbar_bg = Color32::from_rgb(0, 0, 0);
                let scrollbar_fg = Color32::from_rgb(255, 255, 255);
                style.visuals.extreme_bg_color = scrollbar_bg;
                style.visuals.faint_bg_color = scrollbar_bg;
                style.spacing.scroll.foreground_color = true;
                style.visuals.widgets.inactive.fg_stroke.color = scrollbar_fg;
                style.visuals.widgets.hovered.fg_stroke.color = scrollbar_fg;
                style.visuals.widgets.active.fg_stroke.color = scrollbar_fg;
                style.visuals.widgets.open.fg_stroke.color = scrollbar_fg;
            }

            let resp = egui::ScrollArea::vertical()
                .id_salt(("text-node-editor-scroll", id))
                .auto_shrink([false, false])
                .show(&mut text_ui, |ui| {
                    ui.set_width(edit_rect.width());
                    ui.add(
                        TextEdit::multiline(text_body)
                            .id(text_edit_id)
                            .font(FontId::proportional(15.0 * self.zoom))
                            .text_color(text_color)
                            .margin(egui::Margin::ZERO)
                            .desired_width(f32::INFINITY)
                            .desired_rows(desired_rows)
                            .frame(false),
                    )
                })
                .inner;

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
        let should_focus_and_select_all = self.pending_working_directory_focus == Some(id);
        if should_focus_and_select_all {
            ctx.memory_mut(|m| m.request_focus(editor_id));
        }

        let text_edit = TextEdit::singleline(&mut self.working_directory_edit_buffer)
            .id(editor_id)
            .font(FontId::monospace((13.0 * self.zoom).max(9.0)))
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
                let len = self.working_directory_edit_buffer.chars().count();
                let range = egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(len),
                );
                state.cursor.set_char_range(Some(range));
                state.store(ctx, editor_id);
            }
            self.pending_working_directory_focus = None;
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
        let Some((from, to)) = self.editing_edge else {
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
        let width = (200.0 * self.zoom.clamp(0.8, 1.4)).max(140.0);
        let height = (28.0 * self.zoom.clamp(0.8, 1.4)).max(24.0);
        let edit_rect = Rect::from_center_size(center, egui::vec2(width, height));

        let edge_edit_id = egui::Id::new(("edge-route-editor", from, to));
        if self.pending_edge_focus == Some((from, to)) {
            ctx.memory_mut(|m| m.request_focus(edge_edit_id));
        }

        let response = ui.put(
            edit_rect,
            TextEdit::singleline(&mut self.edge_edit_buffer)
                .id(edge_edit_id)
                .font(FontId::proportional((13.0 * self.zoom).max(10.0)))
                .text_color(Color32::WHITE)
                .background_color(Color32::BLACK)
                .hint_text("route_key")
                .desired_width(f32::INFINITY),
        );

        if self.pending_edge_focus == Some((from, to)) {
            if let Some(mut state) = egui::TextEdit::load_state(ctx, edge_edit_id) {
                let len = self.edge_edit_buffer.chars().count();
                let range = egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(len),
                );
                state.cursor.set_char_range(Some(range));
                state.store(ctx, edge_edit_id);
            }
            self.pending_edge_focus = None;
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

    pub(in crate::app::ui) fn handle_decision_buttons_editor(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        decision_edit_rect: Option<(usize, Rect)>,
        primary_clicked: bool,
        pointer_pos: Option<Pos2>,
    ) {
        let Some((id, edit_rect)) = decision_edit_rect else {
            return;
        };

        let mut editor_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(edit_rect)
                .layout(Layout::top_down(Align::Min)),
        );
        editor_ui.set_clip_rect(edit_rect);

        editor_ui.scope(|ui| {
            ui.style_mut().visuals.override_text_color = Some(Color32::from_rgb(216, 245, 224));
            ui.label("编辑按钮配置");

            egui::ScrollArea::vertical()
                .id_salt(("decision-buttons-gui-editor", id))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let mut remove_row: Option<usize> = None;

                    for (row_idx, row) in self.decision_buttons_edit_rows.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            let style = ui.style_mut();
                            style.visuals.override_text_color = Some(Color32::BLACK);
                            style.visuals.widgets.inactive.bg_fill =
                                Color32::from_rgb(245, 248, 252);
                            style.visuals.widgets.hovered.bg_fill =
                                Color32::from_rgb(236, 242, 250);
                            style.visuals.widgets.active.bg_fill = Color32::from_rgb(226, 236, 248);
                            style.visuals.widgets.inactive.fg_stroke.color = Color32::BLACK;
                            style.visuals.widgets.hovered.fg_stroke.color = Color32::BLACK;
                            style.visuals.widgets.active.fg_stroke.color = Color32::BLACK;

                            if ui.small_button("-").clicked() {
                                remove_row = Some(row_idx);
                            }

                            let label_id = egui::Id::new(("decision-button-label", id, row_idx));
                            let label_resp = ui.add_sized(
                                vec2((edit_rect.width() * 0.33).max(110.0), 22.0),
                                TextEdit::singleline(&mut row.label)
                                    .id(label_id)
                                    .text_color(Color32::BLACK)
                                    .background_color(Color32::from_rgb(248, 251, 255))
                                    .hint_text("显示名称"),
                            );

                            if self.pending_decision_buttons_focus == Some(id) && row_idx == 0 {
                                ctx.memory_mut(|m| m.request_focus(label_id));
                                self.pending_decision_buttons_focus = None;
                            }

                            let _ = label_resp.changed();

                            ui.add_sized(
                                vec2((edit_rect.width() * 0.33).max(110.0), 22.0),
                                TextEdit::singleline(&mut row.event_key)
                                    .text_color(Color32::BLACK)
                                    .background_color(Color32::from_rgb(248, 251, 255))
                                    .hint_text("事件名"),
                            );

                            let color_btn = egui::Button::new(egui::RichText::new(""))
                                .min_size(vec2(28.0, 22.0))
                                .fill(Color32::from_rgb(
                                    row.color_rgb[0],
                                    row.color_rgb[1],
                                    row.color_rgb[2],
                                ))
                                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(90, 90, 90)));
                            if ui.add(color_btn).clicked() {
                                row.color_text = GraphApp::decision_color_text_from_rgb(
                                    self.decision_color_input_mode,
                                    row.color_rgb,
                                );
                                self.decision_color_popup = Some((id, row_idx));
                                self.decision_color_popup_pos = ctx
                                    .input(|i| i.pointer.interact_pos().or(i.pointer.latest_pos()));
                            }
                        });
                        ui.add_space(4.0);
                    }

                    if let Some(row_idx) = remove_row {
                        self.remove_decision_button_row(row_idx);
                    }

                    let add_btn =
                        egui::Button::new(egui::RichText::new("+ 新增一行").color(Color32::BLACK))
                            .fill(Color32::from_rgb(228, 236, 246))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(150, 166, 188)));
                    if ui.add(add_btn).clicked() {
                        self.add_decision_button_row();
                    }
                });

            if let Some((popup_node_id, popup_row_idx)) = self.decision_color_popup {
                if popup_node_id == id {
                    if popup_row_idx < self.decision_buttons_edit_rows.len() {
                        let screen_rect = ctx.screen_rect();
                        let anchor = self.decision_color_popup_pos.unwrap_or_else(|| {
                            pointer_pos.unwrap_or(Pos2::new(edit_rect.right(), edit_rect.top()))
                        });

                        let popup_size = vec2(300.0, 220.0);
                        let mut popup_pos = anchor + vec2(10.0, 10.0);
                        popup_pos.x = popup_pos.x.clamp(
                            screen_rect.left() + 8.0,
                            screen_rect.right() - popup_size.x - 8.0,
                        );
                        popup_pos.y = popup_pos.y.clamp(
                            screen_rect.top() + 8.0,
                            screen_rect.bottom() - popup_size.y - 8.0,
                        );

                        let area_response = egui::Area::new(egui::Id::new((
                            "decision-color-popup-area",
                            id,
                            popup_row_idx,
                        )))
                        .order(egui::Order::Foreground)
                        .fixed_pos(popup_pos)
                        .show(ctx, |ui| {
                            egui::Frame::new()
                                .fill(Color32::from_rgb(34, 38, 54))
                                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(88, 98, 128)))
                                .corner_radius(egui::CornerRadius::same(8))
                                .inner_margin(egui::Margin::same(10))
                                .show(ui, |ui| {
                                    ui.set_min_size(popup_size);

                                    let mut mode_changed = false;
                                    ui.horizontal(|ui| {
                                        mode_changed |= ui
                                            .selectable_value(
                                                &mut self.decision_color_input_mode,
                                                super::super::DecisionColorInputMode::Rgb,
                                                "RGB",
                                            )
                                            .clicked();
                                        mode_changed |= ui
                                            .selectable_value(
                                                &mut self.decision_color_input_mode,
                                                super::super::DecisionColorInputMode::Hsv,
                                                "HSV",
                                            )
                                            .clicked();
                                        if ui.small_button("关闭").clicked() {
                                            self.decision_color_popup = None;
                                            self.decision_color_popup_pos = None;
                                        }
                                    });

                                    if mode_changed {
                                        self.sync_decision_color_texts_with_mode();
                                    }

                                    if let Some(row) =
                                        self.decision_buttons_edit_rows.get_mut(popup_row_idx)
                                    {
                                        let color_hint = match self.decision_color_input_mode {
                                            super::super::DecisionColorInputMode::Rgb => {
                                                "r,g,b 例如 212,244,226"
                                            }
                                            super::super::DecisionColorInputMode::Hsv => {
                                                "h,s,v 例如 140,35,96"
                                            }
                                        };

                                        let color_resp = ui.add_sized(
                                            vec2(220.0, 24.0),
                                            TextEdit::singleline(&mut row.color_text)
                                                .text_color(Color32::BLACK)
                                                .background_color(Color32::from_rgb(248, 251, 255))
                                                .hint_text(color_hint),
                                        );

                                        if color_resp.changed() {
                                            if let Some(rgb) = GraphApp::parse_decision_color_text(
                                                self.decision_color_input_mode,
                                                &row.color_text,
                                            ) {
                                                row.color_rgb = rgb;
                                            }
                                        }

                                        ui.add_space(6.0);
                                        ui.label("调色板:");

                                        let palette: [[u8; 3]; 18] = [
                                            [255, 99, 71],
                                            [255, 159, 67],
                                            [255, 215, 0],
                                            [144, 238, 144],
                                            [64, 224, 208],
                                            [135, 206, 250],
                                            [70, 130, 180],
                                            [147, 112, 219],
                                            [238, 130, 238],
                                            [255, 182, 193],
                                            [205, 133, 63],
                                            [210, 180, 140],
                                            [176, 196, 222],
                                            [189, 183, 107],
                                            [46, 139, 87],
                                            [95, 158, 160],
                                            [119, 136, 153],
                                            [220, 220, 220],
                                        ];

                                        egui::Grid::new((
                                            "decision-color-palette-grid",
                                            id,
                                            popup_row_idx,
                                        ))
                                        .num_columns(6)
                                        .spacing(vec2(6.0, 6.0))
                                        .show(ui, |ui| {
                                            for (idx, rgb) in palette.iter().enumerate() {
                                                let swatch = egui::Button::new("")
                                                    .min_size(vec2(22.0, 18.0))
                                                    .fill(Color32::from_rgb(rgb[0], rgb[1], rgb[2]))
                                                    .stroke(egui::Stroke::new(
                                                        1.0,
                                                        Color32::from_rgb(90, 90, 90),
                                                    ));
                                                if ui.add(swatch).clicked() {
                                                    row.color_rgb = *rgb;
                                                    row.color_text =
                                                        GraphApp::decision_color_text_from_rgb(
                                                            self.decision_color_input_mode,
                                                            row.color_rgb,
                                                        );
                                                }

                                                if (idx + 1) % 6 == 0 {
                                                    ui.end_row();
                                                }
                                            }
                                        });
                                    }
                                });
                        });

                        let popup_rect = area_response.response.rect;
                        if primary_clicked {
                            if let Some(pointer) = pointer_pos {
                                if !popup_rect.contains(pointer) {
                                    self.decision_color_popup = None;
                                    self.decision_color_popup_pos = None;
                                }
                            }
                        }
                    } else {
                        self.decision_color_popup = None;
                        self.decision_color_popup_pos = None;
                    }
                }
            }

            if let Some(err) = &self.decision_buttons_edit_error {
                ui.colored_label(Color32::from_rgb(255, 120, 120), err);
            }

            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.cancel_decision_buttons_edit();
            } else if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter)) {
                self.commit_decision_buttons_edit();
            } else if primary_clicked && self.decision_color_popup.is_none() {
                if let Some(pointer) = pointer_pos {
                    if !edit_rect.contains(pointer) {
                        self.commit_decision_buttons_edit();
                    }
                }
            }
        });
    }

    pub(in crate::app::ui) fn handle_decision_queue_editor(&mut self, ctx: &egui::Context) {
        let Some(node_id) = self.editing_decision_queue_node else {
            return;
        };

        let screen_rect = ctx.screen_rect();
        egui::Area::new(egui::Id::new(("decision-queue-modal-mask", node_id)))
            .order(egui::Order::Foreground)
            .interactable(false)
            .fixed_pos(screen_rect.min)
            .show(ctx, |ui| {
                let mask_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, screen_rect.size());
                ui.painter().rect_filled(
                    mask_rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(8, 10, 18, 210),
                );
            });

        let modal_size = vec2(700.0, 500.0);
        let modal_pos = screen_rect.center() - modal_size * 0.5;

        let mut open = true;
        let window = egui::Window::new(
            egui::RichText::new(format!("Decision #{node_id} · 审批消息编辑"))
                .color(Color32::WHITE)
                .strong(),
        )
        .open(&mut open)
        .order(egui::Order::Tooltip)
        .collapsible(false)
        .resizable(true)
        .movable(true)
        .default_size(modal_size)
        .default_pos(modal_pos)
        .frame(
            egui::Frame::new()
                .fill(Color32::from_rgb(25, 29, 42))
                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(86, 102, 138)))
                .corner_radius(egui::CornerRadius::same(10))
                .inner_margin(egui::Margin::same(12)),
        );

        let mut save_clicked = false;
        let mut cancel_clicked = false;

        window.show(ctx, |ui| {
            ui.visuals_mut().override_text_color = Some(Color32::WHITE);
            ui.label("按分隔符拆分每条消息（每条消息可直接改文案）：");
            ui.colored_label(Color32::from_rgb(170, 195, 255), "-----");
            ui.add_space(8.0);

            let editor_id = egui::Id::new(("decision-queue-editor", node_id));
            if self.pending_decision_queue_focus == Some(node_id) {
                ctx.memory_mut(|m| m.request_focus(editor_id));
            }

            let edit_response = ui.add_sized(
                vec2(
                    ui.available_width(),
                    (ui.available_height() - 74.0).max(260.0),
                ),
                TextEdit::multiline(&mut self.decision_queue_edit_buffer)
                    .id(editor_id)
                    .desired_width(f32::INFINITY)
                    .background_color(Color32::from_rgb(14, 18, 30))
                    .text_color(Color32::WHITE)
                    .hint_text("消息1\n\n-----\n\n消息2"),
            );

            if self.pending_decision_queue_focus == Some(node_id) {
                self.pending_decision_queue_focus = None;
            }

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let save_btn =
                    egui::Button::new(egui::RichText::new("保存").color(Color32::BLACK).strong())
                        .fill(Color32::from_rgb(146, 230, 182));
                if ui.add(save_btn).clicked() {
                    save_clicked = true;
                }

                let cancel_btn =
                    egui::Button::new(egui::RichText::new("取消").color(Color32::BLACK).strong())
                        .fill(Color32::from_rgb(235, 198, 203));
                if ui.add(cancel_btn).clicked() {
                    cancel_clicked = true;
                }

                ui.add_space(8.0);
                ui.colored_label(
                    Color32::from_rgb(172, 182, 204),
                    "Ctrl/Cmd + Enter 保存 · Esc 取消",
                );
            });

            if edit_response.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel_clicked = true;
            }
            if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter)) {
                save_clicked = true;
            }
        });

        if !open {
            self.cancel_decision_queue_edit();
            return;
        }

        if save_clicked {
            self.commit_decision_queue_edit(node_id);
        } else if cancel_clicked {
            self.cancel_decision_queue_edit();
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
            && self.editing_working_directory_node != Some(node_id)
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

    pub(in crate::app::ui) fn handle_webpage_url_editor(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        url_edit_rect: Option<(usize, Rect)>,
        primary_clicked: bool,
        pointer_pos: Option<Pos2>,
    ) {
        let Some((id, edit_rect)) = url_edit_rect else {
            return;
        };

        let url_edit_id = egui::Id::new(("webpage-url-editor", id));
        let is_new_focus = self.pending_webpage_url_focus == Some(id);

        let text_edit = TextEdit::singleline(&mut self.webpage_url_edit_buffer)
            .id(url_edit_id)
            .font(FontId::proportional((13.0 * self.zoom).max(9.0)))
            .text_color(Color32::from_rgb(196, 246, 255))
            .background_color(Color32::from_rgb(18, 46, 54))
            .desired_width(f32::INFINITY)
            .frame(false);
        let resp = ui.put(edit_rect, text_edit);

        // Retry focus on every frame until the widget actually has focus.
        // This handles the case where egui's focus system doesn't take effect
        // on the widget's very first render frame.
        let has_focus = resp.has_focus() || ctx.memory(|m| m.has_focus(url_edit_id));
        if is_new_focus || !has_focus {
            resp.request_focus();
            ctx.memory_mut(|m| m.request_focus(url_edit_id));
        }

        if is_new_focus {
            if let Some(mut state) = egui::TextEdit::load_state(ctx, url_edit_id) {
                let len = self.webpage_url_edit_buffer.chars().count();
                let range = egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(len),
                );
                state.cursor.set_char_range(Some(range));
                state.store(ctx, url_edit_id);
                self.pending_webpage_url_focus = None;
            }
            ctx.request_repaint();
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            // Cancel: revert to original URL
            self.cancel_webpage_url_edit();
        } else if ctx.input(|i| i.key_pressed(egui::Key::Enter))
            || (resp.lost_focus() && !ctx.input(|i| i.pointer.primary_down()))
        {
            self.commit_webpage_url_edit(id);
        } else if primary_clicked {
            if let Some(pointer) = pointer_pos {
                if !edit_rect.contains(pointer) {
                    self.commit_webpage_url_edit(id);
                }
            }
        }
    }

    pub(in crate::app::ui) fn show_webpage_url_dialog(&mut self, ctx: &egui::Context) {
        if !self.webpage_url_dialog_open {
            return;
        }

        let dialog_id = egui::Id::new("webpage-url-dialog");
        let mut open = true;

        let window = egui::Window::new(
            egui::RichText::new("设置网址").color(Color32::WHITE).strong(),
        )
        .open(&mut open)
        .order(egui::Order::Tooltip)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .frame(
            egui::Frame::new()
                .fill(Color32::from_rgb(25, 29, 42))
                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(86, 102, 138)))
                .corner_radius(egui::CornerRadius::same(10))
                .inner_margin(egui::Margin::symmetric(16, 12)),
        );

        let mut confirmed = false;
        let mut canceled = false;

        let win_resp = window.show(ctx, |ui| {
            ui.visuals_mut().override_text_color = Some(Color32::WHITE);

            ui.label("请输入网页地址：");
            ui.add_space(8.0);

            let editing_existing = self.webpage_url_dialog_node.is_some();
            let edit_id = dialog_id.with("url-input");
            let _resp = ui.add_sized(
                egui::vec2(380.0, 28.0),
                egui::TextEdit::singleline(&mut self.webpage_url_edit_buffer)
                    .id(edit_id)
                    .hint_text("例如 https://www.example.com")
                    .font(egui::FontId::proportional(14.0))
                    .text_color(Color32::from_rgb(196, 246, 255))
                    .background_color(Color32::from_rgb(18, 46, 54)),
            );

            ctx.memory_mut(|m| m.request_focus(edit_id));

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let ok_btn = egui::Button::new(
                    egui::RichText::new("确定").color(Color32::BLACK).strong(),
                )
                .fill(Color32::from_rgb(146, 230, 182))
                .min_size(egui::vec2(80.0, 28.0));
                if ui.add(ok_btn).clicked() || ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                    confirmed = true;
                }

                let cancel_btn = egui::Button::new(
                    egui::RichText::new("取消").color(Color32::BLACK).strong(),
                )
                .fill(Color32::from_rgb(235, 198, 203))
                .min_size(egui::vec2(80.0, 28.0));
                if ui.add(cancel_btn).clicked() || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    canceled = true;
                }

                ui.add_space(8.0);
                ui.colored_label(
                    Color32::from_rgb(172, 182, 204),
                    if editing_existing {
                        "Enter 确认 · Esc 取消"
                    } else {
                        "Enter 创建 · Esc 取消"
                    },
                );
            });
        });

        // Save actual dialog rect for pixel-perfect occlusion
        self.last_url_dialog_rect = win_resp.and_then(|r| r.response.rect.is_positive().then(|| r.response.rect));

        if canceled || !open {
            // Cancel / closed
            self.webpage_url_dialog_open = false;
            self.webpage_url_dialog_node = None;
            self.webpage_url_dialog_pos = None;
            self.webpage_url_edit_buffer.clear();
            return;
        }

        if confirmed {
            let url = self.webpage_url_edit_buffer.trim().to_owned();

            if let Some(node_id) = self.webpage_url_dialog_node {
                // Editing existing node
                let should_update = self.nodes.iter().any(|n| {
                    n.id == node_id
                        && matches!(&n.data, crate::model::NodeData::WebPage { url: old } if !url.is_empty() && url != *old)
                });
                if should_update {
                    if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
                        if let crate::model::NodeData::WebPage { url: old } = &mut node.data {
                            *old = url.clone();
                        }
                    }
                    self.mark_workspace_dirty();
                    self.navigate_webview_to(node_id, &url);
                }
            } else if !url.is_empty() {
                // Creating new node
                let pos = self.webpage_url_dialog_pos.unwrap_or(Pos2::new(100.0, 100.0));
                let id = self.create_webpage_node(pos, false);
                if let Some(node) = self.nodes.iter_mut().find(|n| n.id == id) {
                    if let crate::model::NodeData::WebPage { url: old } = &mut node.data {
                        *old = url.clone();
                    }
                }
                self.navigate_webview_to(id, &url);
            }

            self.webpage_url_dialog_open = false;
            self.webpage_url_dialog_node = None;
            self.webpage_url_dialog_pos = None;
            self.webpage_url_edit_buffer.clear();
        }
    }
}

use super::GraphApp;
use crate::constants::TERMINAL_HEADER_HEIGHT;
use crate::model::NodeKind;
use eframe::egui::{
    self, vec2, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, TextEdit, Ui,
};
use egui_term::{TerminalFont, TerminalView};

impl GraphApp {
    pub(super) fn draw_service_panel(&mut self, ui: &mut Ui) {
        ui.heading("节点数据面板");
        ui.separator();

        if let Some(id) = self.selected {
            if let Some(node) = self.nodes.iter_mut().find(|n| n.id == id) {
                ui.label(format!("节点名称: {}", node.title));
                ui.label(format!("节点 ID: {}", node.id));
                ui.label(format!("节点类型: {}", Self::node_kind_name(&node.kind)));
                ui.label(format!("分类: {}", node.category));
                ui.label(format!("状态: {}", node.status));

                match node.kind {
                    NodeKind::Text => {
                        ui.separator();
                        ui.label("文本内容:");
                        ui.add_sized(
                            [ui.available_width(), 120.0],
                            TextEdit::multiline(&mut node.text_body),
                        );
                        if ui.button("进入画布内编辑模式").clicked() {
                            self.editing_text_node = Some(node.id);
                            self.pending_text_focus = Some(node.id);
                        }
                    }
                    _ => {
                        ui.separator();
                        ui.label(format!("平均延迟: {} ms", node.latency_ms));
                        ui.label(format!("吞吐量: {:.0} qps", node.qps));
                        ui.label(format!("错误率: {:.2}%", node.errors));
                    }
                }

                ui.separator();
                ui.small("提示：支持节点右键 -> 创建节点（终端/文本）。");
                ui.small("空白处双击可快速创建文本节点，且自动进入编辑模式。");
            }
        }
    }

    pub(super) fn draw_terminal_hint_panel(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let Some(node_id) = self.selected_terminal_id() else {
            return;
        };

        ui.heading("Terminal 节点");
        ui.separator();
        ui.label("终端现在直接嵌入在画布中的 Terminal 节点内部。\n拖拽 Terminal 节点顶部可移动它。");

        if ui.button("重启终端").clicked() {
            self.restart_terminal(node_id, ctx);
        }

        ui.separator();
        if self.terminal_backends.contains_key(&node_id) {
            ui.label(egui::RichText::new("● Running").color(Color32::LIGHT_GREEN));
        } else if self.terminal_exited.contains(&node_id) {
            ui.label(egui::RichText::new("● Exited").color(Color32::LIGHT_RED));
        } else {
            ui.label(egui::RichText::new("● Starting...").color(Color32::YELLOW));
        }

        if let Some(err) = self.terminal_errors.get(&node_id) {
            ui.colored_label(Color32::LIGHT_RED, err);
        }
    }

    pub(super) fn draw_canvas(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let available = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(available, Sense::click_and_drag());
        let painter = ui.painter_at(rect);

        painter.rect_filled(rect, 0.0, Color32::from_rgb(20, 22, 26));
        self.paint_grid(&painter, rect, self.pan);

        let is_space_down = ctx.input(|i| i.key_down(egui::Key::Space));
        let is_space_pan = ctx.input(|i| i.key_down(egui::Key::Space) && i.pointer.primary_down());
        let is_middle_pan = ctx.input(|i| i.pointer.middle_down());
        let pointer_pos = ctx.input(|i| i.pointer.interact_pos().or_else(|| i.pointer.hover_pos()));
        let pointer_in_canvas = pointer_pos.is_some_and(|p| rect.contains(p));

        let terminal_content_rects = self.terminal_content_rects_screen(rect);
        let pointer_over_terminal_content = pointer_pos.is_some_and(|p| {
            terminal_content_rects
                .iter()
                .any(|(_, term_rect)| term_rect.contains(p))
        });

        let is_panning =
            (is_space_pan || is_middle_pan) && pointer_in_canvas && !pointer_over_terminal_content;

        if is_panning {
            self.dragging = None;
            let delta = ctx.input(|i| i.pointer.delta());
            self.pan += delta;
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);
        }

        if !is_panning && !pointer_over_terminal_content && response.drag_started() {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                let local = pointer_pos - rect.min - self.pan;
                if let Some((id, node_pos, can_drag)) = self.find_node_hit(local.to_pos2()) {
                    self.selected = Some(id);
                    if can_drag {
                        self.dragging = Some((id, local - node_pos));
                    }
                }
            }
        }

        if let Some((drag_id, offset)) = self.dragging {
            if ctx.input(|i| i.pointer.primary_down()) && !ctx.input(|i| i.key_down(egui::Key::Space)) {
                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    let local = pointer_pos - rect.min - self.pan;
                    if let Some(node) = self.nodes.iter_mut().find(|n| n.id == drag_id) {
                        node.pos = (local - offset).to_pos2();
                    }
                }
            } else {
                self.dragging = None;
            }
        }

        if !is_panning && !pointer_over_terminal_content && response.secondary_clicked() {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                let local = pointer_pos - rect.min - self.pan;
                self.context_menu_local_pos = Some(local.to_pos2());
                self.context_menu_node = self.find_node_at(local.to_pos2()).map(|(id, _)| id);
                self.menu_search_text.clear();
                self.pending_menu_search_focus = true;
                if let Some(id) = self.context_menu_node {
                    self.selected = Some(id);
                }
            }
        }

        response.context_menu(|ui| {
            let search_id = egui::Id::new("context_menu_search_input");
            if self.pending_menu_search_focus {
                ui.memory_mut(|m| m.request_focus(search_id));
                self.pending_menu_search_focus = false;
            }

            ui.add(
                TextEdit::singleline(&mut self.menu_search_text)
                    .id(search_id)
                    .hint_text("搜索创建节点..."),
            );

            ui.separator();

            ui.menu_button("创建节点", |ui| {
                let spawn_pos = if let Some(id) = self.context_menu_node {
                    if let Some(node) = self.nodes.iter().find(|n| n.id == id) {
                        node.pos + vec2(node.size.x + 40.0, 10.0)
                    } else {
                        self.context_menu_local_pos.unwrap_or(Pos2::new(100.0, 100.0))
                    }
                } else {
                    self.context_menu_local_pos.unwrap_or(Pos2::new(100.0, 100.0))
                };

                let terminal_label = "终端节点";
                let text_label = "文本节点";
                let mut has_match = false;

                if self.menu_item_matches(terminal_label) {
                    has_match = true;
                    ui.menu_button("终端", |ui| {
                        if ui.button(terminal_label).clicked() {
                            self.create_terminal_node(spawn_pos);
                            ui.close_menu();
                        }
                    });
                }

                if self.menu_item_matches(text_label) {
                    has_match = true;
                    ui.menu_button("文本", |ui| {
                        if ui.button(text_label).clicked() {
                            self.create_text_node(spawn_pos, true);
                            ui.close_menu();
                        }
                    });
                }

                if !has_match {
                    ui.small("无匹配节点类型");
                }
            });
        });

        if !is_panning && !pointer_over_terminal_content && response.double_clicked() {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                let local = pointer_pos - rect.min - self.pan;
                if let Some((id, _)) = self.find_node_at(local.to_pos2()) {
                    self.selected = Some(id);
                    if let Some(node) = self.nodes.iter().find(|n| n.id == id) {
                        if node.kind == NodeKind::Text {
                            self.editing_text_node = Some(id);
                            self.pending_text_focus = Some(id);
                        }
                    }
                } else {
                    self.create_text_node((local - vec2(120.0, 60.0)).to_pos2(), true);
                }
            }
        }

        if !is_panning && !pointer_over_terminal_content && response.clicked() {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                let local = pointer_pos - rect.min - self.pan;
                if let Some((id, _)) = self.find_node_at(local.to_pos2()) {
                    self.selected = Some(id);
                    if self.editing_text_node != Some(id) {
                        self.editing_text_node = None;
                    }
                } else {
                    self.selected = None;
                    self.editing_text_node = None;
                }
            }
        }

        for (from, to) in &self.edges {
            if let (Some(a), Some(b)) = (
                self.nodes.iter().find(|n| n.id == *from),
                self.nodes.iter().find(|n| n.id == *to),
            ) {
                let start = rect.min + self.pan + a.pos.to_vec2() + vec2(a.size.x, a.size.y * 0.5);
                let end = rect.min + self.pan + b.pos.to_vec2() + vec2(0.0, b.size.y * 0.5);
                painter.line_segment([start, end], Stroke::new(2.0, Color32::from_rgb(110, 170, 255)));

                let dir = (end - start).normalized();
                let left = end - dir * 12.0 + vec2(-dir.y, dir.x) * 6.0;
                let right = end - dir * 12.0 + vec2(dir.y, -dir.x) * 6.0;
                painter.line_segment([left, end], Stroke::new(2.0, Color32::from_rgb(110, 170, 255)));
                painter.line_segment([right, end], Stroke::new(2.0, Color32::from_rgb(110, 170, 255)));
            }
        }

        let mut text_edit_rect: Option<(usize, Rect)> = None;

        for node in &self.nodes {
            let node_rect = Rect::from_min_size(rect.min + self.pan + node.pos.to_vec2(), node.size);
            let is_selected = self.selected == Some(node.id);

            let (fill, stroke) = match node.kind {
                NodeKind::Service => {
                    let fill = if is_selected {
                        Color32::from_rgb(40, 75, 130)
                    } else {
                        Color32::from_rgb(38, 43, 52)
                    };
                    let stroke = if is_selected {
                        Stroke::new(2.0, Color32::from_rgb(120, 180, 255))
                    } else {
                        Stroke::new(1.0, Color32::from_rgb(80, 90, 105))
                    };
                    (fill, stroke)
                }
                NodeKind::Terminal => {
                    let fill = if is_selected {
                        Color32::from_rgb(64, 52, 120)
                    } else {
                        Color32::from_rgb(48, 40, 86)
                    };
                    let stroke = if is_selected {
                        Stroke::new(2.0, Color32::from_rgb(174, 149, 255))
                    } else {
                        Stroke::new(1.0, Color32::from_rgb(108, 96, 145))
                    };
                    (fill, stroke)
                }
                NodeKind::Text => {
                    let fill = if is_selected {
                        Color32::from_rgb(90, 73, 34)
                    } else {
                        Color32::from_rgb(72, 60, 31)
                    };
                    let stroke = if is_selected {
                        Stroke::new(2.0, Color32::from_rgb(255, 220, 130))
                    } else {
                        Stroke::new(1.0, Color32::from_rgb(130, 114, 68))
                    };
                    (fill, stroke)
                }
            };

            painter.rect(node_rect, 8.0, fill, stroke, egui::StrokeKind::Outside);
            painter.text(
                node_rect.left_top() + vec2(12.0, 10.0),
                Align2::LEFT_TOP,
                &node.title,
                FontId::proportional(17.0),
                Color32::WHITE,
            );

            match node.kind {
                NodeKind::Service => {
                    let status_color = match node.status {
                        "Healthy" => Color32::from_rgb(96, 212, 125),
                        "Warning" => Color32::from_rgb(255, 193, 88),
                        _ => Color32::from_rgb(255, 111, 111),
                    };
                    painter.circle_filled(node_rect.right_top() - vec2(14.0, -14.0), 5.0, status_color);
                    painter.text(
                        node_rect.left_bottom() - vec2(-12.0, 12.0),
                        Align2::LEFT_BOTTOM,
                        format!("{} ms", node.latency_ms),
                        FontId::proportional(14.0),
                        Color32::from_gray(210),
                    );
                }
                NodeKind::Terminal => {
                    let state_text = if self.terminal_backends.contains_key(&node.id) {
                        "状态: Running"
                    } else if self.terminal_exited.contains(&node.id) {
                        "状态: Exited"
                    } else {
                        "状态: Starting"
                    };

                    painter.text(
                        node_rect.right_top() - vec2(12.0, -12.0),
                        Align2::RIGHT_TOP,
                        state_text,
                        FontId::proportional(13.0),
                        Color32::from_rgb(225, 220, 255),
                    );

                    painter.line_segment(
                        [
                            node_rect.left_top() + vec2(0.0, TERMINAL_HEADER_HEIGHT),
                            node_rect.right_top() + vec2(0.0, TERMINAL_HEADER_HEIGHT),
                        ],
                        Stroke::new(1.0, Color32::from_rgb(108, 96, 145)),
                    );
                }
                NodeKind::Text => {
                    let preview = if node.text_body.trim().is_empty() {
                        "(空文本)"
                    } else {
                        &node.text_body
                    };

                    painter.text(
                        node_rect.left_top() + vec2(12.0, 36.0),
                        Align2::LEFT_TOP,
                        preview,
                        FontId::proportional(15.0),
                        Color32::from_rgb(250, 240, 210),
                    );

                    if self.editing_text_node == Some(node.id) {
                        let edit_rect = Rect::from_min_max(
                            node_rect.min + vec2(8.0, 34.0),
                            node_rect.max - vec2(8.0, 8.0),
                        );
                        text_edit_rect = Some((node.id, edit_rect));
                    }
                }
            }
        }

        if let Some((id, edit_rect)) = text_edit_rect {
            if let Some(node) = self.nodes.iter_mut().find(|n| n.id == id) {
                let text_edit_id = egui::Id::new(("text-node-editor", id));
                if self.pending_text_focus == Some(id) {
                    ctx.memory_mut(|m| m.request_focus(text_edit_id));
                    self.pending_text_focus = None;
                }

                let text_edit = TextEdit::multiline(&mut node.text_body)
                    .id(text_edit_id)
                    .desired_width(edit_rect.width())
                    .desired_rows(4)
                    .frame(true);
                let resp = ui.put(edit_rect, text_edit);

                if resp.changed() {
                    let first_line = node.text_body.lines().next().unwrap_or("文本节点").trim();
                    if first_line.is_empty() {
                        node.title = format!("文本节点 {}", node.id);
                    } else {
                        node.title = first_line.chars().take(14).collect();
                    }
                }

                if resp.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.editing_text_node = None;
                }
            }
        }

        for (node_id, term_rect) in terminal_content_rects {
            let visible_rect = term_rect.intersect(rect);
            if !visible_rect.is_positive() {
                continue;
            }

            if !self.terminal_backends.contains_key(&node_id)
                && !self.terminal_errors.contains_key(&node_id)
                && !self.terminal_exited.contains(&node_id)
            {
                self.ensure_terminal(node_id, ctx);
            }

            egui::Area::new(egui::Id::new(("terminal_node_embedded_area", node_id)))
                .order(egui::Order::Foreground)
                .constrain(false)
                .fixed_pos(term_rect.min)
                .show(ctx, |ui| {
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
                        let term = TerminalView::new(&mut term_ui, backend)
                            .set_focus(self.selected == Some(node_id))
                            .set_font(TerminalFont::default())
                            .set_size(term_rect.size());
                        term_ui.add(term);
                    } else {
                        term_ui.label("终端未启动，请在右侧点击“重启终端”。");
                    }
                });
        }

        if !is_panning {
            if let Some(pos) = response.hover_pos() {
                let local = pos - rect.min - self.pan;
                if is_space_down && response.hovered() {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grab);
                } else if self.find_node_at(local.to_pos2()).is_some() {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                }
            }
        }
    }
}

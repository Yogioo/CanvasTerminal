use super::GraphApp;
use crate::constants::{TERMINAL_HEADER_HEIGHT, TERMINAL_NODE_ID};
use crate::model::NodeKind;
use eframe::egui::{
    self, vec2, Align2, Color32, FontId, Rect, Sense, Stroke, Ui,
};
use egui_term::{TerminalFont, TerminalView};

impl GraphApp {
    pub(super) fn draw_service_panel(&self, ui: &mut Ui) {
        ui.heading("节点数据面板（Mock）");
        ui.separator();

        if let Some(id) = self.selected {
            if let Some(node) = self.nodes.iter().find(|n| n.id == id) {
                ui.label(format!("节点名称: {}", node.title));
                ui.label(format!("节点 ID: {}", node.id));
                ui.label(format!("状态: {}", node.status));
                ui.separator();

                ui.label(format!("平均延迟: {} ms", node.latency_ms));
                ui.label(format!("吞吐量: {:.0} qps", node.qps));
                ui.label(format!("错误率: {:.2}%", node.errors));
                ui.separator();

                ui.label("最近 5 分钟（假数据）");
                ui.add(egui::ProgressBar::new((node.qps / 8000.0).clamp(0.0, 1.0)).text("流量负载"));
                ui.add(
                    egui::ProgressBar::new((node.latency_ms as f32 / 200.0).clamp(0.0, 1.0))
                        .text("延迟占比"),
                );
                ui.add(egui::ProgressBar::new((node.errors / 2.0).clamp(0.0, 1.0)).text("错误率占比"));

                ui.separator();
                ui.small("提示：点击节点切换数据；拖拽节点会更新连线；支持 Space+左键拖拽 或 中键拖拽 来平移画布。");
                ui.small("点击 Terminal 节点可获得接近系统终端的真实显示效果。 ");
            }
        }
    }

    pub(super) fn draw_terminal_hint_panel(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.heading("Terminal 节点");
        ui.separator();
        ui.label("终端现在直接嵌入在画布中的 Terminal 节点内部。\n拖拽 Terminal 节点顶部可移动它。");

        if ui.button("重启终端").clicked() {
            self.terminal_backend = None;
            self.terminal_exited = false;
            self.ensure_terminal(ctx);
        }

        ui.separator();
        if self.terminal_backend.is_some() {
            ui.label(egui::RichText::new("● Running").color(Color32::LIGHT_GREEN));
        } else if self.terminal_exited {
            ui.label(egui::RichText::new("● Exited").color(Color32::LIGHT_RED));
        } else {
            ui.label(egui::RichText::new("● Starting...").color(Color32::YELLOW));
        }

        if let Some(err) = &self.terminal_error {
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

        let terminal_content_rect = self.terminal_content_rect_screen(rect);
        let pointer_over_terminal_content = pointer_pos
            .is_some_and(|p| terminal_content_rect.is_some_and(|r| r.contains(p)));

        let is_panning = (is_space_pan || is_middle_pan) && pointer_in_canvas && !pointer_over_terminal_content;

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

        if !is_panning && !pointer_over_terminal_content && response.clicked() {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                let local = pointer_pos - rect.min - self.pan;
                if let Some((id, _)) = self.find_node_at(local.to_pos2()) {
                    self.selected = Some(id);
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
            };

            painter.rect(node_rect, 8.0, fill, stroke, egui::StrokeKind::Outside);
            painter.text(
                node_rect.left_top() + vec2(12.0, 12.0),
                Align2::LEFT_TOP,
                &node.title,
                FontId::proportional(18.0),
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
                    let state_text = if self.terminal_backend.is_some() {
                        "状态: Running"
                    } else if self.terminal_exited {
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

                    // 标题栏与终端区分隔线
                    painter.line_segment(
                        [
                            node_rect.left_top() + vec2(0.0, TERMINAL_HEADER_HEIGHT),
                            node_rect.right_top() + vec2(0.0, TERMINAL_HEADER_HEIGHT),
                        ],
                        Stroke::new(1.0, Color32::from_rgb(108, 96, 145)),
                    );
                }
            }
        }

        // 在 Terminal 节点内部嵌入真实终端，并裁剪到当前画布可见区域
        // 注意：这里必须重新计算 term_rect（不能复用前面用于 hit-test 的值），
        // 否则当本帧 pan 已更新时，终端会比节点慢一帧，出现拖拽错位感。
        if let Some(term_rect) = self.terminal_content_rect_screen(rect) {
            let visible_rect = term_rect.intersect(rect);
            if visible_rect.is_positive() {
                egui::Area::new(egui::Id::new("terminal_node_embedded_area"))
                    .order(egui::Order::Foreground)
                    .constrain(false)
                    .fixed_pos(term_rect.min)
                    .show(ctx, |ui| {
                        // 注意：UiBuilder::max_rect / set_clip_rect 都使用屏幕坐标。
                        // 之前使用了本地坐标(0,0)导致终端被布局到左上角。
                        let full_screen_rect = Rect::from_min_size(term_rect.min, term_rect.size());
                        let mut term_ui = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(full_screen_rect)
                                .layout(*ui.layout()),
                        );
                        term_ui.set_clip_rect(visible_rect);

                        if let Some(err) = &self.terminal_error {
                            term_ui.colored_label(Color32::LIGHT_RED, err);
                        } else if let Some(backend) = self.terminal_backend.as_mut() {
                            let term = TerminalView::new(&mut term_ui, backend)
                                .set_focus(self.selected == Some(TERMINAL_NODE_ID))
                                .set_font(TerminalFont::default())
                                .set_size(term_rect.size());
                            term_ui.add(term);
                        } else {
                            term_ui.label("终端未启动，请在右侧点击“重启终端”。");
                        }
                    });
            }
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

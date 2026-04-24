use super::super::GraphApp;
use crate::model::{NodeData, NodeKind};
use eframe::egui::{self, vec2, Align, Align2, Color32, FontId, Layout, Painter, Pos2, Rect, Stroke};
use egui_commonmark::CommonMarkViewer;

impl GraphApp {
    pub(in crate::app::ui) fn draw_edges(&self, painter: &Painter, rect: Rect) {
        for (from, to) in &self.edges {
            if let (Some(a), Some(b)) = (
                self.nodes.iter().find(|n| n.id == *from),
                self.nodes.iter().find(|n| n.id == *to),
            ) {
                let start = self.world_to_screen_pos(rect, a.pos + vec2(a.size.x, a.size.y * 0.5));
                let end = self.world_to_screen_pos(rect, b.pos + vec2(0.0, b.size.y * 0.5));
                let edge_stroke = 2.0 * self.zoom.clamp(0.6, 1.6);
                painter.line_segment(
                    [start, end],
                    Stroke::new(edge_stroke, Color32::from_rgb(110, 170, 255)),
                );

                let dir = (end - start).normalized();
                let left = end - dir * (12.0 * self.zoom) + vec2(-dir.y, dir.x) * (6.0 * self.zoom);
                let right = end - dir * (12.0 * self.zoom) + vec2(dir.y, -dir.x) * (6.0 * self.zoom);
                painter.line_segment(
                    [left, end],
                    Stroke::new(edge_stroke, Color32::from_rgb(110, 170, 255)),
                );
                painter.line_segment(
                    [right, end],
                    Stroke::new(edge_stroke, Color32::from_rgb(110, 170, 255)),
                );
            }
        }
    }

    pub(in crate::app::ui) fn draw_link_preview(&self, painter: &Painter, rect: Rect) {
        if let (Some(from), Some(pointer_local)) = (self.linking_from, self.linking_pointer_local) {
            if let Some(node) = self.nodes.iter().find(|n| n.id == from) {
                let start =
                    self.world_to_screen_pos(rect, node.pos + vec2(node.size.x, node.size.y * 0.5));
                let end = self.world_to_screen_pos(rect, pointer_local);
                painter.line_segment(
                    [start, end],
                    Stroke::new(
                        2.0 * self.zoom.clamp(0.6, 1.6),
                        Color32::from_rgba_premultiplied(130, 195, 255, 220),
                    ),
                );
            }
        }
    }

    pub(in crate::app::ui) fn draw_cut_path(&self, painter: &Painter, rect: Rect) {
        if self.cutting_path_local.len() < 2 {
            return;
        }

        for pair in self.cutting_path_local.windows(2) {
            let a = self.world_to_screen_pos(rect, pair[0]);
            let b = self.world_to_screen_pos(rect, pair[1]);
            painter.line_segment(
                [a, b],
                Stroke::new(
                    2.0 * self.zoom.clamp(0.6, 1.6),
                    Color32::from_rgba_premultiplied(255, 120, 120, 220),
                ),
            );
        }
    }

    pub(in crate::app::ui) fn draw_nodes(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        painter: &Painter,
        rect: Rect,
    ) -> (Option<(usize, Rect)>, Option<(usize, Rect)>, Option<(usize, Rect)>) {
        let mut text_edit_rect: Option<(usize, Rect)> = None;
        let mut title_edit_rect: Option<(usize, Rect)> = None;
        let mut startup_edit_rect: Option<(usize, Rect)> = None;

        let render_nodes = self.nodes.clone();
        for node in &render_nodes {
            let node_rect = self.world_to_screen_rect(rect, Rect::from_min_size(node.pos, node.size));
            let is_selected = self.selected_nodes.contains(&node.id);
            let zoom_scale = self.zoom;

            let (fill, stroke) = match node.kind {
                NodeKind::Terminal => {
                    let fill = if is_selected {
                        Color32::from_rgb(64, 52, 120)
                    } else {
                        Color32::from_rgb(48, 40, 86)
                    };
                    let stroke = if is_selected {
                        Stroke::new(
                            2.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(174, 149, 255),
                        )
                    } else {
                        Stroke::new(
                            1.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(108, 96, 145),
                        )
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
                        Stroke::new(
                            2.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(255, 220, 130),
                        )
                    } else {
                        Stroke::new(
                            1.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(130, 114, 68),
                        )
                    };
                    (fill, stroke)
                }
                NodeKind::Image => {
                    let fill = if is_selected {
                        Color32::from_rgb(32, 78, 88)
                    } else {
                        Color32::from_rgb(24, 61, 70)
                    };
                    let stroke = if is_selected {
                        Stroke::new(
                            2.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(124, 220, 240),
                        )
                    } else {
                        Stroke::new(
                            1.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(78, 145, 160),
                        )
                    };
                    (fill, stroke)
                }
            };

            if node.kind == NodeKind::Text {
                painter.rect(node_rect, 8.0 * zoom_scale, fill, stroke, egui::StrokeKind::Outside);
            }

            match node.kind {
                NodeKind::Terminal => {
                    painter.rect_stroke(
                        node_rect,
                        8.0 * zoom_scale,
                        stroke,
                        egui::StrokeKind::Outside,
                    );

                    let header_height = self.terminal_header_height_screen();
                    let header_bottom = (node_rect.min.y + header_height).min(node_rect.max.y);
                    let header_rect = Rect::from_min_max(
                        node_rect.min,
                        Pos2::new(node_rect.max.x, header_bottom),
                    );
                    painter.rect_filled(header_rect, 8.0 * zoom_scale, fill);

                    let is_title_editing = self.editing_title_node == Some(node.id);
                    if !is_title_editing {
                        let title_text = match &node.data {
                            NodeData::Terminal { title, .. } => title.as_str(),
                            _ => "Terminal",
                        };
                        painter.text(
                            Pos2::new(node_rect.min.x + 12.0 * zoom_scale, header_rect.center().y),
                            Align2::LEFT_CENTER,
                            title_text,
                            FontId::proportional((17.0 * zoom_scale).max(9.0)),
                            Color32::WHITE,
                        );
                    } else {
                        let rect_min = node_rect.left_top() + vec2(10.0, 6.0) * zoom_scale;
                        let rect_max = Pos2::new(
                            node_rect.max.x - 10.0 * zoom_scale,
                            (node_rect.min.y + header_height - 6.0 * zoom_scale).min(node_rect.max.y),
                        );
                        title_edit_rect = Some((node.id, Rect::from_min_max(rect_min, rect_max)));
                    }

                    if !is_title_editing {
                        let state_text = if self.terminal_backends.contains_key(&node.id) {
                            "状态: Running"
                        } else if self.terminal_exited.contains(&node.id) {
                            "状态: Exited"
                        } else {
                            "状态: Starting"
                        };

                        painter.text(
                            Pos2::new(node_rect.max.x - 12.0 * zoom_scale, header_rect.center().y),
                            Align2::RIGHT_CENTER,
                            state_text,
                            FontId::proportional((13.0 * zoom_scale).max(8.0)),
                            Color32::from_rgb(225, 220, 255),
                        );
                    }

                    painter.line_segment(
                        [
                            Pos2::new(node_rect.min.x, header_bottom),
                            Pos2::new(node_rect.max.x, header_bottom),
                        ],
                        Stroke::new(
                            1.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(108, 96, 145),
                        ),
                    );

                    if let Some(term_rect) = self.terminal_content_rect_screen(node.id, rect) {
                        if self.editing_startup_node == Some(node.id) {
                            let overlay_rect = Rect::from_min_max(
                                term_rect.min + vec2(10.0, 10.0) * zoom_scale,
                                term_rect.max - vec2(10.0, 10.0) * zoom_scale,
                            );
                            startup_edit_rect = Some((node.id, overlay_rect));
                        }
                        self.draw_embedded_terminal_for_rect(ui, ctx, rect, node.id, term_rect);
                    }

                    if is_selected {
                        let handle_size = 12.0 * zoom_scale.clamp(0.75, 1.6);
                        let handle_rect = Rect::from_min_size(
                            node_rect.right_bottom() - vec2(handle_size + 6.0, handle_size + 6.0),
                            vec2(handle_size, handle_size),
                        );
                        painter.rect_filled(handle_rect, 2.0, Color32::from_rgb(205, 195, 255));
                    }
                }
                NodeKind::Text => {
                    let hide_text_for_zoom = zoom_scale < self.text_hide_zoom_threshold;
                    let is_editing = self.editing_text_node == Some(node.id);

                    if !hide_text_for_zoom {
                        if !is_editing {
                            let preview = match &node.data {
                                NodeData::Text { text_body, .. } if text_body.trim().is_empty() => "(空文本)",
                                NodeData::Text { text_body, .. } => text_body.as_str(),
                                _ => "(空文本)",
                            };

                            let content_rect = Rect::from_min_max(
                                node_rect.min + vec2(12.0, 12.0) * zoom_scale,
                                node_rect.max - vec2(12.0, 12.0) * zoom_scale,
                            );

                            if content_rect.is_positive() {
                                let mut text_ui = ui.new_child(
                                    egui::UiBuilder::new()
                                        .max_rect(content_rect)
                                        .layout(Layout::top_down(Align::Min)),
                                );
                                text_ui.set_clip_rect(content_rect);
                                text_ui.set_width(content_rect.width());

                                egui::ScrollArea::vertical()
                                    .id_salt(("text-node-preview-scroll", node.id))
                                    .auto_shrink([false, false])
                                    .show(&mut text_ui, |ui| {
                                        ui.set_width(content_rect.width());

                                        let body_size = (15.0 * zoom_scale).round();
                                        let heading_size = (body_size * 2.2).round();

                                        // Markdown 主题（暖深色）
                                        let body_color = Color32::from_rgb(236, 228, 208);      // 正文
                                        let heading_color = Color32::from_rgb(255, 214, 122);   // 标题/强调
                                        let scrollbar_bg = Color32::from_rgb(0, 0, 0);          // 滚动条轨道背景
                                        let scrollbar_fg = Color32::from_rgb(255, 255, 255);    // 滚动条前景

                                        let style = ui.style_mut();
                                        style.visuals.override_text_color = None;
                                        style.visuals.widgets.noninteractive.fg_stroke.color = body_color;
                                        style.visuals.widgets.inactive.fg_stroke.color = body_color;
                                        style.visuals.widgets.active.fg_stroke.color = heading_color;
                                        style.visuals.hyperlink_color = Color32::from_rgb(122, 196, 255); // 链接
                                        style.visuals.code_bg_color = Color32::from_rgb(47, 42, 33);      // 代码块背景

                                        style.visuals.extreme_bg_color = scrollbar_bg;
                                        style.visuals.faint_bg_color = scrollbar_bg;
                                        style.spacing.scroll.foreground_color = true;
                                        style.visuals.widgets.inactive.fg_stroke.color = scrollbar_fg;
                                        style.visuals.widgets.hovered.fg_stroke.color = scrollbar_fg;
                                        style.visuals.widgets.active.fg_stroke.color = scrollbar_fg;
                                        style.visuals.widgets.open.fg_stroke.color = scrollbar_fg;

                                        style.text_styles.insert(
                                            egui::TextStyle::Body,
                                            FontId::proportional(body_size),
                                        );
                                        style.text_styles.insert(
                                            egui::TextStyle::Small,
                                            FontId::proportional(body_size),
                                        );
                                        style.text_styles.insert(
                                            egui::TextStyle::Button,
                                            FontId::proportional(body_size),
                                        );
                                        style.text_styles.insert(
                                            egui::TextStyle::Heading,
                                            FontId::proportional(heading_size),
                                        );
                                        style.text_styles.insert(
                                            egui::TextStyle::Monospace,
                                            FontId::monospace(body_size),
                                        );

                                        CommonMarkViewer::new().show(ui, &mut self.markdown_cache, preview);
                                    });
                            }
                        }

                        if is_editing {
                            let edit_rect = Rect::from_min_max(
                                node_rect.min + vec2(12.0, 12.0) * zoom_scale,
                                node_rect.max - vec2(12.0, 12.0) * zoom_scale,
                            );
                            text_edit_rect = Some((node.id, edit_rect));
                        }
                    }

                    if is_selected {
                        let handle_size = 12.0 * zoom_scale.clamp(0.75, 1.6);
                        let handle_rect = Rect::from_min_size(
                            node_rect.right_bottom() - vec2(handle_size + 6.0, handle_size + 6.0),
                            vec2(handle_size, handle_size),
                        );
                        painter.rect_filled(handle_rect, 2.0, Color32::from_rgb(255, 220, 130));
                    }
                }
                NodeKind::Image => {
                    if let Some(texture) = self.image_texture(node.id) {
                        painter.image(
                            texture.id(),
                            node_rect,
                            Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    } else if let Some(err) = self.image_error(node.id) {
                        painter.text(
                            node_rect.center(),
                            Align2::CENTER_CENTER,
                            err,
                            FontId::proportional((13.0 * zoom_scale).max(9.0)),
                            Color32::from_rgb(255, 170, 170),
                        );
                    } else {
                        painter.text(
                            node_rect.center(),
                            Align2::CENTER_CENTER,
                            "拖拽图片文件到画布\n或在画布粘贴图片路径",
                            FontId::proportional((13.0 * zoom_scale).max(9.0)),
                            Color32::from_rgb(205, 236, 242),
                        );
                    }

                    if is_selected {
                        let handle_size = 12.0 * zoom_scale.clamp(0.75, 1.6);
                        let handle_rect = Rect::from_min_size(
                            node_rect.right_bottom() - vec2(handle_size + 6.0, handle_size + 6.0),
                            vec2(handle_size, handle_size),
                        );
                        painter.rect_filled(handle_rect, 2.0, Color32::from_rgb(175, 230, 240));
                    }
                }
            }
        }

        (text_edit_rect, title_edit_rect, startup_edit_rect)
    }
}

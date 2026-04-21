use super::super::GraphApp;
use crate::constants::TERMINAL_HEADER_HEIGHT;
use crate::model::NodeKind;
use eframe::egui::{self, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Stroke};

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

    pub(in crate::app::ui) fn autosize_text_nodes(&mut self, painter: &Painter) {
        for node in self.nodes.iter_mut().filter(|n| n.kind == NodeKind::Text) {
            let visible_text = if node.text_body.trim().is_empty() {
                "(空文本)"
            } else {
                &node.text_body
            };
            let galley = painter.layout_no_wrap(
                visible_text.to_owned(),
                FontId::proportional(15.0),
                Color32::from_rgb(250, 240, 210),
            );
            node.size = vec2(galley.size().x + 24.0, galley.size().y + 24.0);
        }
    }

    pub(in crate::app::ui) fn draw_nodes(&self, painter: &Painter, rect: Rect) -> (Option<(usize, Rect)>, Option<(usize, Rect)>) {
        let mut text_edit_rect: Option<(usize, Rect)> = None;
        let mut title_edit_rect: Option<(usize, Rect)> = None;

        for node in &self.nodes {
            let node_rect = self.world_to_screen_rect(rect, Rect::from_min_size(node.pos, node.size));
            let is_selected = self.selected == Some(node.id);
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

            if node.kind != NodeKind::Image {
                painter.rect(node_rect, 8.0 * zoom_scale, fill, stroke, egui::StrokeKind::Outside);
            }

            match node.kind {
                NodeKind::Terminal => {
                    let is_title_editing = self.editing_title_node == Some(node.id);
                    if !is_title_editing {
                        painter.text(
                            node_rect.left_top() + vec2(12.0, 10.0) * zoom_scale,
                            Align2::LEFT_TOP,
                            &node.title,
                            FontId::proportional((17.0 * zoom_scale).max(9.0)),
                            Color32::WHITE,
                        );
                    } else {
                        let rect_min = node_rect.left_top() + vec2(10.0, 6.0) * zoom_scale;
                        let rect_max = node_rect.right_top()
                            + vec2(-10.0, TERMINAL_HEADER_HEIGHT - 6.0) * zoom_scale;
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
                            node_rect.right_top() - vec2(12.0, -12.0) * zoom_scale,
                            Align2::RIGHT_TOP,
                            state_text,
                            FontId::proportional((13.0 * zoom_scale).max(8.0)),
                            Color32::from_rgb(225, 220, 255),
                        );
                    }

                    if !node.identity.trim().is_empty() {
                        painter.text(
                            node_rect.left_top() + vec2(12.0, 30.0) * zoom_scale,
                            Align2::LEFT_TOP,
                            format!("@{}", node.identity),
                            FontId::proportional((13.0 * zoom_scale).max(8.0)),
                            Color32::from_rgb(214, 205, 255),
                        );
                    }

                    painter.line_segment(
                        [
                            node_rect.left_top() + vec2(0.0, TERMINAL_HEADER_HEIGHT) * zoom_scale,
                            node_rect.right_top() + vec2(0.0, TERMINAL_HEADER_HEIGHT) * zoom_scale,
                        ],
                        Stroke::new(
                            1.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(108, 96, 145),
                        ),
                    );

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
                    let is_editing = self.editing_text_node == Some(node.id);
                    if !is_editing {
                        let preview = if node.text_body.trim().is_empty() {
                            "(空文本)"
                        } else {
                            &node.text_body
                        };

                        painter.text(
                            node_rect.center(),
                            Align2::CENTER_CENTER,
                            preview,
                            FontId::proportional(15.0 * zoom_scale),
                            Color32::from_rgb(250, 240, 210),
                        );
                    }

                    if is_editing {
                        let edit_rect = Rect::from_min_max(
                            node_rect.min + vec2(12.0, 12.0) * zoom_scale,
                            node_rect.max - vec2(12.0, 12.0) * zoom_scale,
                        );
                        text_edit_rect = Some((node.id, edit_rect));
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

        (text_edit_rect, title_edit_rect)
    }
}

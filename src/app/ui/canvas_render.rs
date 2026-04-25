use super::super::{EdgeControlHandle, GraphApp};
use crate::model::{NodeData, NodeKind};
use eframe::egui::{
    self, epaint::CubicBezierShape, vec2, Align, Align2, Color32, FontId, Layout, Painter, Pos2,
    Rect, Stroke,
};
use egui_commonmark::CommonMarkViewer;

impl GraphApp {
    pub(in crate::app::ui) fn draw_edges(
        &self,
        painter: &Painter,
        rect: Rect,
        hovered_edge: Option<(usize, usize)>,
    ) {
        for (from, to) in &self.edges {
            let Some(curve) = self.edge_curve_local(*from, *to) else {
                continue;
            };

            let is_selected = self.selected_edge == Some((*from, *to));
            let is_hovered = hovered_edge == Some((*from, *to));

            let start = self.world_to_screen_pos(rect, curve.start);
            let ctrl1 = self.world_to_screen_pos(rect, curve.ctrl1);
            let ctrl2 = self.world_to_screen_pos(rect, curve.ctrl2);
            let end = self.world_to_screen_pos(rect, curve.end);

            let zoom_scale = self.zoom.clamp(0.7, 1.6);
            let edge_stroke = if is_selected {
                3.4 * zoom_scale
            } else if is_hovered {
                2.8 * zoom_scale
            } else {
                2.0 * zoom_scale
            };
            let edge_color = if is_selected {
                Color32::from_rgb(176, 232, 255)
            } else if is_hovered {
                Color32::from_rgb(148, 210, 255)
            } else {
                Color32::from_rgb(110, 170, 255)
            };

            if is_selected || is_hovered {
                let halo_width = edge_stroke + if is_selected { 2.8 } else { 1.8 };
                let halo_color = if is_selected {
                    Color32::from_rgba_premultiplied(160, 220, 255, 132)
                } else {
                    Color32::from_rgba_premultiplied(145, 205, 255, 92)
                };

                painter.add(CubicBezierShape::from_points_stroke(
                    [start, ctrl1, ctrl2, end],
                    false,
                    Color32::TRANSPARENT,
                    Stroke::new(halo_width, halo_color),
                ));
            }

            let stroke = Stroke::new(edge_stroke, edge_color);
            painter.add(CubicBezierShape::from_points_stroke(
                [start, ctrl1, ctrl2, end],
                false,
                Color32::TRANSPARENT,
                stroke,
            ));

            let mut tangent = end - ctrl2;
            if tangent.length_sq() <= f32::EPSILON {
                tangent = end - start;
            }
            if tangent.length_sq() <= f32::EPSILON {
                tangent = vec2(1.0, 0.0);
            }
            let mut dir = tangent.normalized();

            if let Some(expected_dir) = self.edge_target_incoming_direction_local(*from, *to) {
                let dot = dir.dot(expected_dir);
                if dot < 0.0 {
                    dir = expected_dir;
                } else if dot < 0.35 {
                    dir = (dir + expected_dir * 0.8).normalized();
                }
            }

            let arrow_len = (11.0 * zoom_scale).clamp(8.0, 15.0);
            let arrow_wing = (5.8 * zoom_scale).clamp(4.2, 8.2);
            let left = end - dir * arrow_len + vec2(-dir.y, dir.x) * arrow_wing;
            let right = end - dir * arrow_len + vec2(dir.y, -dir.x) * arrow_wing;
            painter.line_segment([left, end], stroke);
            painter.line_segment([right, end], stroke);

            if let (Some(route_key), Some(label_world_pos)) = (
                self.edge_route_key(*from, *to),
                self.edge_label_world_pos(*from, *to),
            ) {
                let label_pos = self.world_to_screen_pos(rect, label_world_pos);
                painter.text(
                    label_pos + vec2(0.0, -8.0 * self.zoom),
                    Align2::CENTER_BOTTOM,
                    route_key,
                    FontId::proportional((13.0 * self.zoom).max(9.0)),
                    Color32::from_rgb(236, 232, 255),
                );
            }
        }
    }

    pub(in crate::app::ui) fn draw_selected_edge_controls_overlay(
        &self,
        painter: &Painter,
        rect: Rect,
    ) {
        let Some((from, to)) = self.selected_edge else {
            return;
        };

        let Some(curve) = self.edge_curve_local(from, to) else {
            return;
        };

        let zoom_scale = self.zoom.clamp(0.7, 1.6);
        let start = self.world_to_screen_pos(rect, curve.start);
        let ctrl1 = self.world_to_screen_pos(rect, curve.ctrl1);
        let ctrl2 = self.world_to_screen_pos(rect, curve.ctrl2);
        let end = self.world_to_screen_pos(rect, curve.end);

        painter.line_segment(
            [start, ctrl1],
            Stroke::new(
                (1.0 * zoom_scale).max(1.0),
                Color32::from_rgba_premultiplied(180, 220, 255, 140),
            ),
        );
        painter.line_segment(
            [end, ctrl2],
            Stroke::new(
                (1.0 * zoom_scale).max(1.0),
                Color32::from_rgba_premultiplied(180, 220, 255, 140),
            ),
        );

        let handle_radius = (6.8 * self.zoom.clamp(0.75, 1.8)).max(5.0);
        for (handle, fill, stroke_color) in [
            (
                EdgeControlHandle::Source,
                Color32::from_rgb(244, 250, 255),
                Color32::from_rgb(120, 195, 255),
            ),
            (
                EdgeControlHandle::Target,
                Color32::from_rgb(245, 255, 246),
                Color32::from_rgb(128, 220, 172),
            ),
        ] {
            if let Some(handle_world) = self.edge_control_handle_world_pos_local(from, to, handle) {
                let handle_screen = self.world_to_screen_pos(rect, handle_world);
                painter.circle_filled(handle_screen, handle_radius, fill);
                painter.circle_stroke(
                    handle_screen,
                    handle_radius,
                    Stroke::new((1.6 * self.zoom).max(1.0), stroke_color),
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
    ) -> (
        Option<(usize, Rect)>,
        Option<(usize, Rect)>,
        Option<(usize, Rect)>,
    ) {
        let mut text_edit_rect: Option<(usize, Rect)> = None;
        let mut title_edit_rect: Option<(usize, Rect)> = None;
        let mut startup_edit_rect: Option<(usize, Rect)> = None;

        let render_nodes = self.nodes.clone();
        for node in &render_nodes {
            let node_rect =
                self.world_to_screen_rect(rect, Rect::from_min_size(node.pos, node.size));
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
                painter.rect(
                    node_rect,
                    8.0 * zoom_scale,
                    fill,
                    stroke,
                    egui::StrokeKind::Outside,
                );
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
                            (node_rect.min.y + header_height - 6.0 * zoom_scale)
                                .min(node_rect.max.y),
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

                    let hide_terminal_for_zoom = zoom_scale < self.terminal_hide_zoom_threshold
                        && self.editing_startup_node != Some(node.id);

                    if let Some(term_rect) = self.terminal_content_rect_screen(node.id, rect) {
                        if !hide_terminal_for_zoom {
                            if self.editing_startup_node == Some(node.id) {
                                let overlay_rect = Rect::from_min_max(
                                    term_rect.min + vec2(10.0, 10.0) * zoom_scale,
                                    term_rect.max - vec2(10.0, 10.0) * zoom_scale,
                                );
                                startup_edit_rect = Some((node.id, overlay_rect));
                            }
                            self.draw_embedded_terminal_for_rect(ui, ctx, rect, node.id, term_rect);
                        }
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
                                NodeData::Text { text_body, .. } if text_body.trim().is_empty() => {
                                    "(空文本)"
                                }
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
                                        let body_color = Color32::from_rgb(236, 228, 208); // 正文
                                        let heading_color = Color32::from_rgb(255, 214, 122); // 标题/强调
                                        let scrollbar_bg = Color32::from_rgb(0, 0, 0); // 滚动条轨道背景
                                        let scrollbar_fg = Color32::from_rgb(255, 255, 255); // 滚动条前景

                                        let style = ui.style_mut();
                                        style.visuals.override_text_color = None;
                                        style.visuals.widgets.noninteractive.fg_stroke.color =
                                            body_color;
                                        style.visuals.widgets.inactive.fg_stroke.color = body_color;
                                        style.visuals.widgets.active.fg_stroke.color =
                                            heading_color;
                                        style.visuals.hyperlink_color =
                                            Color32::from_rgb(122, 196, 255); // 链接
                                        style.visuals.code_bg_color = Color32::from_rgb(47, 42, 33); // 代码块背景

                                        style.visuals.extreme_bg_color = scrollbar_bg;
                                        style.visuals.faint_bg_color = scrollbar_bg;
                                        style.spacing.scroll.foreground_color = true;
                                        style.visuals.widgets.inactive.fg_stroke.color =
                                            scrollbar_fg;
                                        style.visuals.widgets.hovered.fg_stroke.color =
                                            scrollbar_fg;
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
                                        style.interaction.selectable_labels = false;

                                        CommonMarkViewer::new().show(
                                            ui,
                                            &mut self.markdown_cache,
                                            preview,
                                        );
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

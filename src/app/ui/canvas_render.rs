use super::super::{EdgeControlHandle, GraphApp};
use eframe::egui::{
    epaint::CubicBezierShape, vec2, Align2, Color32, FontId, Painter, Rect, Stroke,
};

impl GraphApp {
    pub(in crate::app::ui) fn draw_edges(
        &self,
        painter: &Painter,
        rect: Rect,
        hovered_edge: Option<(usize, usize)>,
    ) {
        for (from, to) in &self.ws.edges {
            let Some(curve) = self.edge_curve_local(*from, *to) else {
                continue;
            };

            let is_selected = self.ws.selected_edge == Some((*from, *to));
            let is_hovered = hovered_edge == Some((*from, *to));

            let start = self.world_to_screen_pos(rect, curve.start);
            let ctrl1 = self.world_to_screen_pos(rect, curve.ctrl1);
            let ctrl2 = self.world_to_screen_pos(rect, curve.ctrl2);
            let end = self.world_to_screen_pos(rect, curve.end);

            let zoom_scale = self.ws.zoom.clamp(0.7, 1.6);
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
                    label_pos + vec2(0.0, -8.0 * self.ws.zoom),
                    Align2::CENTER_BOTTOM,
                    route_key,
                    FontId::proportional((13.0 * self.ws.zoom).max(9.0)),
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
        let Some((from, to)) = self.ws.selected_edge else {
            return;
        };

        let Some(curve) = self.edge_curve_local(from, to) else {
            return;
        };

        let zoom_scale = self.ws.zoom.clamp(0.7, 1.6);
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

        let handle_radius = (6.8 * self.ws.zoom.clamp(0.75, 1.8)).max(5.0);
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
                    Stroke::new((1.6 * self.ws.zoom).max(1.0), stroke_color),
                );
            }
        }
    }

    pub(in crate::app::ui) fn draw_link_preview(&self, painter: &Painter, rect: Rect) {
        // Right-click node-to-node linking preview
        if let (Some(from), Some(pointer_local)) = (self.ws.linking_from, self.ws.linking_pointer_local) {
            if let Some(node) = self.ws.nodes.iter().find(|n| n.id == from) {
                let start =
                    self.world_to_screen_pos(rect, node.pos + vec2(node.size.x, node.size.y * 0.5));
                let end = self.world_to_screen_pos(rect, pointer_local);
                painter.line_segment(
                    [start, end],
                    Stroke::new(
                        2.0 * self.ws.zoom.clamp(0.6, 1.6),
                        Color32::from_rgba_premultiplied(130, 195, 255, 220),
                    ),
                );
            }
        }

    }

    pub(in crate::app::ui) fn draw_cut_path(&self, painter: &Painter, rect: Rect) {
        if self.ws.cutting_path_local.len() < 2 {
            return;
        }

        for pair in self.ws.cutting_path_local.windows(2) {
            let a = self.world_to_screen_pos(rect, pair[0]);
            let b = self.world_to_screen_pos(rect, pair[1]);
            painter.line_segment(
                [a, b],
                Stroke::new(
                    2.0 * self.ws.zoom.clamp(0.6, 1.6),
                    Color32::from_rgba_premultiplied(255, 120, 120, 220),
                ),
            );
        }
    }
}

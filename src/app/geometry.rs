use super::GraphApp;
use crate::constants::TERMINAL_HEADER_HEIGHT;
use crate::model::{Node, NodeKind};
use eframe::egui::{self, vec2, Pos2, Rect};

impl GraphApp {
    pub(in crate::app) fn find_node_at(&self, local: Pos2) -> Option<(usize, egui::Vec2)> {
        for n in self.nodes.iter().rev() {
            let r = Rect::from_min_size(n.pos, n.size);
            if r.contains(local) {
                return Some((n.id, n.pos.to_vec2()));
            }
        }
        None
    }

    pub(in crate::app) fn find_node_hit(&self, local: Pos2) -> Option<(usize, egui::Vec2, bool)> {
        for n in self.nodes.iter().rev() {
            let r = Rect::from_min_size(n.pos, n.size);
            if !r.contains(local) {
                continue;
            }

            let can_drag = match n.kind {
                NodeKind::Text | NodeKind::Image => true,
                NodeKind::Terminal => local.y <= n.pos.y + TERMINAL_HEADER_HEIGHT,
            };

            return Some((n.id, n.pos.to_vec2(), can_drag));
        }
        None
    }

    pub(in crate::app) fn find_terminal_identity_badge_at(&self, local: Pos2) -> Option<usize> {
        for n in self.nodes.iter().rev() {
            if n.kind != NodeKind::Terminal {
                continue;
            }
            if Self::terminal_identity_badge_world_rect(n).contains(local) {
                return Some(n.id);
            }
        }
        None
    }

    pub(in crate::app) fn world_to_screen_pos(&self, canvas_rect: Rect, world: Pos2) -> Pos2 {
        canvas_rect.min + self.pan + world.to_vec2() * self.zoom
    }

    pub(in crate::app) fn world_to_screen_rect(&self, canvas_rect: Rect, world_rect: Rect) -> Rect {
        Rect::from_min_size(
            self.world_to_screen_pos(canvas_rect, world_rect.min),
            world_rect.size() * self.zoom,
        )
    }

    pub(in crate::app) fn screen_to_world_pos(&self, canvas_rect: Rect, screen: Pos2) -> Pos2 {
        ((screen - canvas_rect.min - self.pan) / self.zoom).to_pos2()
    }

    fn node_world_rect(node: &Node) -> Rect {
        Rect::from_min_size(node.pos, node.size)
    }

    fn all_nodes_world_rect(&self) -> Option<Rect> {
        let mut iter = self.nodes.iter();
        let first = iter.next()?;
        let mut bounds = Self::node_world_rect(first);
        for node in iter {
            bounds = bounds.union(Self::node_world_rect(node));
        }
        Some(bounds)
    }

    fn focus_rect(&mut self, canvas_rect: Rect, target_world_rect: Rect) {
        let viewport_padding = 64.0;
        let view_w = (canvas_rect.width() - viewport_padding * 2.0).max(1.0);
        let view_h = (canvas_rect.height() - viewport_padding * 2.0).max(1.0);

        let target_w = target_world_rect.width().max(1.0);
        let target_h = target_world_rect.height().max(1.0);

        self.zoom = (view_w / target_w).min(view_h / target_h).clamp(0.35, 2.5);

        let target_center = target_world_rect.center();
        self.pan = canvas_rect.center() - canvas_rect.min - target_center.to_vec2() * self.zoom;
    }

    fn selected_nodes_world_rect(&self) -> Option<Rect> {
        let mut selected_nodes = self
            .nodes
            .iter()
            .filter(|n| self.selected_nodes.contains(&n.id));

        let first = selected_nodes.next()?;
        let mut bounds = Self::node_world_rect(first);
        for node in selected_nodes {
            bounds = bounds.union(Self::node_world_rect(node));
        }
        Some(bounds)
    }

    pub(in crate::app) fn focus_selected_or_all(&mut self, canvas_rect: Rect) {
        let target = self
            .selected_nodes_world_rect()
            .or_else(|| self.all_nodes_world_rect());

        if let Some(target_world_rect) = target {
            self.focus_rect(canvas_rect, target_world_rect);
        }
    }

    pub(in crate::app) fn terminal_identity_badge_world_rect(node: &Node) -> Rect {
        let height = 22.0;
        let width = node.size.x.clamp(120.0, 220.0);
        Rect::from_min_size(
            Pos2::new(node.pos.x + 10.0, node.pos.y - height - 8.0),
            vec2(width, height),
        )
    }

    pub(in crate::app) fn terminal_header_height_screen(&self) -> f32 {
        let zoom_scale = self.zoom;
        let title_font_size = (17.0 * zoom_scale).max(9.0);
        let status_font_size = (13.0 * zoom_scale).max(8.0);
        let title_required_height = 10.0 * zoom_scale + title_font_size + 2.0 * zoom_scale;
        let status_required_height = 12.0 * zoom_scale + status_font_size + 2.0 * zoom_scale;
        (TERMINAL_HEADER_HEIGHT * zoom_scale).max(title_required_height.max(status_required_height))
    }

    pub(in crate::app) fn terminal_content_rect_screen(
        &self,
        node_id: usize,
        canvas_rect: Rect,
    ) -> Option<Rect> {
        let n = self.nodes.iter().find(|n| n.id == node_id)?;
        if !matches!(n.kind, NodeKind::Terminal) {
            return None;
        }

        let outer_world = Rect::from_min_size(n.pos, n.size);
        let outer_screen = self.world_to_screen_rect(canvas_rect, outer_world);
        let border = 2.0 * self.zoom;
        let header_height = self.terminal_header_height_screen();

        let inner_min = outer_screen.min + vec2(border, header_height + border);
        let inner_max = outer_screen.max - vec2(border, border);
        if inner_min.x >= inner_max.x || inner_min.y >= inner_max.y {
            return None;
        }

        Some(Rect::from_min_max(inner_min, inner_max))
    }

    pub(in crate::app) fn segment_intersects_rect(a: Pos2, b: Pos2, rect: Rect) -> bool {
        if rect.contains(a) || rect.contains(b) {
            return true;
        }

        let lt = rect.left_top();
        let rt = rect.right_top();
        let rb = rect.right_bottom();
        let lb = rect.left_bottom();

        Self::segments_intersect(a, b, lt, rt)
            || Self::segments_intersect(a, b, rt, rb)
            || Self::segments_intersect(a, b, rb, lb)
            || Self::segments_intersect(a, b, lb, lt)
    }

    pub(in crate::app) fn segments_intersect(a1: Pos2, a2: Pos2, b1: Pos2, b2: Pos2) -> bool {
        const EPS: f32 = 0.0001;

        fn cross(a: Pos2, b: Pos2, c: Pos2) -> f32 {
            (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
        }

        fn within(a: f32, b: f32, x: f32, eps: f32) -> bool {
            x >= a.min(b) - eps && x <= a.max(b) + eps
        }

        fn on_segment(a: Pos2, b: Pos2, p: Pos2, eps: f32) -> bool {
            within(a.x, b.x, p.x, eps) && within(a.y, b.y, p.y, eps)
        }

        let d1 = cross(a1, a2, b1);
        let d2 = cross(a1, a2, b2);
        let d3 = cross(b1, b2, a1);
        let d4 = cross(b1, b2, a2);

        if (d1 > EPS && d2 < -EPS || d1 < -EPS && d2 > EPS)
            && (d3 > EPS && d4 < -EPS || d3 < -EPS && d4 > EPS)
        {
            return true;
        }

        (d1.abs() <= EPS && on_segment(a1, a2, b1, EPS))
            || (d2.abs() <= EPS && on_segment(a1, a2, b2, EPS))
            || (d3.abs() <= EPS && on_segment(b1, b2, a1, EPS))
            || (d4.abs() <= EPS && on_segment(b1, b2, a2, EPS))
    }
}

use super::history::HistoryEntry;
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

    pub(in crate::app) fn ensure_camera_initialized(&mut self, canvas_rect: Rect) {
        if self.camera_initialized {
            return;
        }

        self.camera_world_center = ((canvas_rect.center() - canvas_rect.min - self.pan) / self.zoom).to_pos2();
        if !self.camera_world_center.x.is_finite() || !self.camera_world_center.y.is_finite() {
            self.camera_world_center = Pos2::new(0.0, 0.0);
        }
        self.camera_initialized = true;
        self.sync_pan_from_camera(canvas_rect);
    }

    pub(in crate::app) fn sync_pan_from_camera(&mut self, canvas_rect: Rect) {
        self.pan = canvas_rect.center() - canvas_rect.min - self.camera_world_center.to_vec2() * self.zoom;
    }

    pub(in crate::app) fn world_to_screen_pos(&self, canvas_rect: Rect, world: Pos2) -> Pos2 {
        canvas_rect.center() + (world - self.camera_world_center) * self.zoom
    }

    pub(in crate::app) fn world_to_screen_rect(&self, canvas_rect: Rect, world_rect: Rect) -> Rect {
        Rect::from_min_size(
            self.world_to_screen_pos(canvas_rect, world_rect.min),
            world_rect.size() * self.zoom,
        )
    }

    pub(in crate::app) fn screen_to_world_pos(&self, canvas_rect: Rect, screen: Pos2) -> Pos2 {
        (self.camera_world_center.to_vec2() + (screen - canvas_rect.center()) / self.zoom).to_pos2()
    }

    pub(in crate::app) fn maybe_rebase_world(&mut self, canvas_rect: Rect) {
        const REBASE_THRESHOLD: f32 = 50_000.0;
        const REBASE_CHUNK: f32 = 10_000.0;

        let Some(bounds) = self.all_nodes_world_rect() else {
            return;
        };

        let world_center = bounds.center();
        let mut shift = vec2(0.0, 0.0);

        if world_center.x.abs() > REBASE_THRESHOLD {
            shift.x = (world_center.x / REBASE_CHUNK).trunc() * REBASE_CHUNK;
        }
        if world_center.y.abs() > REBASE_THRESHOLD {
            shift.y = (world_center.y / REBASE_CHUNK).trunc() * REBASE_CHUNK;
        }

        if shift == vec2(0.0, 0.0) {
            return;
        }

        for node in &mut self.nodes {
            node.pos -= shift;
        }

        self.camera_world_center -= shift;

        if let Some((id, pos)) = self.drag_start_pos {
            self.drag_start_pos = Some((id, pos - shift));
        }

        if let Some((start_pointer, start_nodes)) = self.drag_group_start.as_mut() {
            *start_pointer -= shift;
            for (_, node_pos) in start_nodes {
                *node_pos -= shift;
            }
        }

        if let Some((id, start_pointer, start_size)) = self.resizing {
            self.resizing = Some((id, start_pointer - shift, start_size));
        }

        if let Some(pos) = self.context_menu_local_pos {
            self.context_menu_local_pos = Some(pos - shift);
        }

        if let Some(pos) = self.linking_pointer_local {
            self.linking_pointer_local = Some(pos - shift);
        }

        for p in &mut self.cutting_path_local {
            *p -= shift;
        }

        if let Some(pos) = self.last_canvas_pointer_world_pos {
            self.last_canvas_pointer_world_pos = Some(pos - shift);
        }

        if let Some(pos) = self.last_drag_hover_world_pos {
            self.last_drag_hover_world_pos = Some(pos - shift);
        }

        if let Some(pos) = self.pending_drop_spawn_world_pos {
            self.pending_drop_spawn_world_pos = Some(pos - shift);
        }

        if let Some(pos) = self.box_select_start {
            self.box_select_start = Some(pos - shift);
        }

        if let Some(pos) = self.box_select_current {
            self.box_select_current = Some(pos - shift);
        }

        if let Some(nodes) = self.cut_snapshot_nodes.as_mut() {
            for node in nodes {
                node.pos -= shift;
            }
        }

        Self::shift_history_entries(&mut self.undo_stack, shift);
        Self::shift_history_entries(&mut self.redo_stack, shift);

        self.sync_pan_from_camera(canvas_rect);
    }

    fn shift_history_entries(entries: &mut [HistoryEntry], shift: egui::Vec2) {
        for entry in entries {
            match entry {
                HistoryEntry::CreateBatch { nodes } | HistoryEntry::DeleteBatch { nodes, .. } => {
                    for node in nodes {
                        node.pos -= shift;
                    }
                }
                HistoryEntry::MoveNode { from, to, .. } => {
                    *from -= shift;
                    *to -= shift;
                }
                HistoryEntry::MoveNodes { nodes } => {
                    for (_, from, to) in nodes {
                        *from -= shift;
                        *to -= shift;
                    }
                }
                HistoryEntry::ReorderNodes { .. } => {}
            }
        }
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

        self.zoom = (view_w / target_w).min(view_h / target_h).max(1e-4);

        let target_center = target_world_rect.center();
        self.camera_world_center = target_center;
        self.sync_pan_from_camera(canvas_rect);

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
        let selected_rect = self.selected_nodes_world_rect();
        let all_rect = self.all_nodes_world_rect();
        let target = selected_rect.or(all_rect);

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

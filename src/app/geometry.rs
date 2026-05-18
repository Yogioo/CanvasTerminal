use super::history::HistoryEntry;
use super::{EdgeControlHandle, GraphApp};
use crate::constants::{DECISION_HEADER_HEIGHT, TERMINAL_HEADER_HEIGHT};
use crate::model::{Node, NodeKind};
use eframe::egui::{self, vec2, Pos2, Rect};

const EDGE_CURVE_SAMPLE_SEGMENTS: usize = 24;
const EDGE_CONTROL_OFFSET_MIN: f32 = 36.0;
const EDGE_CONTROL_OFFSET_MAX: f32 = 180.0;
const EDGE_CONTROL_OFFSET_REVERSE_MAX: f32 = 260.0;
const EDGE_CONTROL_OFFSET_REVERSE_BOOST: f32 = 1.35;
const EDGE_CURVE_BIAS_MIN: f32 = -180.0;
const EDGE_CURVE_BIAS_MAX: f32 = 180.0;
const EDGE_CONTROL_POINT_OFFSET_MAX: f32 = 260.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum EdgeAnchorSide {
    Left,
    Right,
    Top,
    Bottom,
}

impl EdgeAnchorSide {
    fn is_horizontal(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }

    fn direction(self) -> egui::Vec2 {
        match self {
            Self::Left => vec2(-1.0, 0.0),
            Self::Right => vec2(1.0, 0.0),
            Self::Top => vec2(0.0, -1.0),
            Self::Bottom => vec2(0.0, 1.0),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::app) struct EdgeCurve {
    pub start: Pos2,
    pub ctrl1: Pos2,
    pub ctrl2: Pos2,
    pub end: Pos2,
}

fn pick_edge_anchor_sides(
    source_center: Pos2,
    target_center: Pos2,
) -> (EdgeAnchorSide, EdgeAnchorSide) {
    let delta = target_center - source_center;

    if delta.x.abs() >= delta.y.abs() {
        if delta.x >= 0.0 {
            (EdgeAnchorSide::Right, EdgeAnchorSide::Left)
        } else {
            (EdgeAnchorSide::Left, EdgeAnchorSide::Right)
        }
    } else if delta.y >= 0.0 {
        (EdgeAnchorSide::Bottom, EdgeAnchorSide::Top)
    } else {
        (EdgeAnchorSide::Top, EdgeAnchorSide::Bottom)
    }
}

fn node_anchor_on_side(node: &Node, side: EdgeAnchorSide) -> Pos2 {
    match side {
        EdgeAnchorSide::Left => node.pos + vec2(0.0, node.size.y * 0.5),
        EdgeAnchorSide::Right => node.pos + vec2(node.size.x, node.size.y * 0.5),
        EdgeAnchorSide::Top => node.pos + vec2(node.size.x * 0.5, 0.0),
        EdgeAnchorSide::Bottom => node.pos + vec2(node.size.x * 0.5, node.size.y),
    }
}

fn edge_control_offset(
    source_center: Pos2,
    target_center: Pos2,
    source_side: EdgeAnchorSide,
) -> f32 {
    let delta = target_center - source_center;
    let (primary, secondary) = if source_side.is_horizontal() {
        (delta.x.abs(), delta.y.abs())
    } else {
        (delta.y.abs(), delta.x.abs())
    };

    let base =
        (primary * 0.35 + secondary * 0.15).clamp(EDGE_CONTROL_OFFSET_MIN, EDGE_CONTROL_OFFSET_MAX);

    if target_center.x < source_center.x {
        (base * EDGE_CONTROL_OFFSET_REVERSE_BOOST)
            .clamp(EDGE_CONTROL_OFFSET_MIN, EDGE_CONTROL_OFFSET_REVERSE_MAX)
    } else {
        base
    }
}

fn edge_curve_from_nodes(source: &Node, target: &Node) -> EdgeCurve {
    let source_rect = Rect::from_min_size(source.pos, source.size);
    let target_rect = Rect::from_min_size(target.pos, target.size);

    let source_center = source_rect.center();
    let target_center = target_rect.center();

    let (source_side, target_side) = pick_edge_anchor_sides(source_center, target_center);
    let start = node_anchor_on_side(source, source_side);
    let end = node_anchor_on_side(target, target_side);
    let control_offset = edge_control_offset(source_center, target_center, source_side);

    EdgeCurve {
        start,
        ctrl1: start + source_side.direction() * control_offset,
        ctrl2: end + target_side.direction() * control_offset,
        end,
    }
}

fn clamp_edge_curve_bias(bias: f32) -> f32 {
    if !bias.is_finite() {
        return 0.0;
    }

    bias.clamp(EDGE_CURVE_BIAS_MIN, EDGE_CURVE_BIAS_MAX)
}

fn clamp_edge_control_offset(offset: egui::Vec2) -> egui::Vec2 {
    if !offset.x.is_finite() || !offset.y.is_finite() {
        return vec2(0.0, 0.0);
    }

    vec2(
        offset.x.clamp(
            -EDGE_CONTROL_POINT_OFFSET_MAX,
            EDGE_CONTROL_POINT_OFFSET_MAX,
        ),
        offset.y.clamp(
            -EDGE_CONTROL_POINT_OFFSET_MAX,
            EDGE_CONTROL_POINT_OFFSET_MAX,
        ),
    )
}

fn cubic_bezier_tangent(curve: &EdgeCurve, t: f32) -> egui::Vec2 {
    let t = t.clamp(0.0, 1.0);
    let omt = 1.0 - t;

    let a = (curve.ctrl1 - curve.start) * (3.0 * omt * omt);
    let b = (curve.ctrl2 - curve.ctrl1) * (6.0 * omt * t);
    let c = (curve.end - curve.ctrl2) * (3.0 * t * t);
    a + b + c
}

fn edge_curve_handle_basis(curve: &EdgeCurve) -> (Pos2, egui::Vec2) {
    let midpoint = cubic_bezier_point(curve, 0.5);
    let tangent = cubic_bezier_tangent(curve, 0.5);

    let mut normal = vec2(-tangent.y, tangent.x);
    if normal.length_sq() <= f32::EPSILON {
        let fallback = curve.end - curve.start;
        normal = vec2(-fallback.y, fallback.x);
    }

    if normal.length_sq() <= f32::EPSILON {
        normal = vec2(0.0, -1.0);
    } else {
        normal = normal.normalized();
    }

    (midpoint, normal)
}

fn edge_curve_with_bias(base_curve: &EdgeCurve, bias: f32) -> EdgeCurve {
    let clamped_bias = clamp_edge_curve_bias(bias);
    if clamped_bias.abs() <= 0.001 {
        return *base_curve;
    }

    let (_, normal) = edge_curve_handle_basis(base_curve);
    let offset = normal * clamped_bias;

    EdgeCurve {
        start: base_curve.start,
        ctrl1: base_curve.ctrl1 + offset,
        ctrl2: base_curve.ctrl2 + offset,
        end: base_curve.end,
    }
}

#[cfg(test)]
fn edge_curve_handle_world_pos(base_curve: &EdgeCurve, bias: f32) -> Pos2 {
    let (midpoint, normal) = edge_curve_handle_basis(base_curve);
    midpoint + normal * clamp_edge_curve_bias(bias)
}

#[cfg(test)]
fn edge_curve_bias_from_pointer(base_curve: &EdgeCurve, pointer: Pos2) -> f32 {
    let (midpoint, normal) = edge_curve_handle_basis(base_curve);
    clamp_edge_curve_bias((pointer - midpoint).dot(normal))
}

fn cubic_bezier_point(curve: &EdgeCurve, t: f32) -> Pos2 {
    let t = t.clamp(0.0, 1.0);
    let omt = 1.0 - t;

    let w0 = omt * omt * omt;
    let w1 = 3.0 * omt * omt * t;
    let w2 = 3.0 * omt * t * t;
    let w3 = t * t * t;

    Pos2::new(
        curve.start.x * w0 + curve.ctrl1.x * w1 + curve.ctrl2.x * w2 + curve.end.x * w3,
        curve.start.y * w0 + curve.ctrl1.y * w1 + curve.ctrl2.y * w2 + curve.end.y * w3,
    )
}

fn sample_edge_curve(curve: &EdgeCurve, segments: usize) -> Vec<Pos2> {
    let segments = segments.max(2);
    (0..=segments)
        .map(|idx| {
            let t = idx as f32 / segments as f32;
            cubic_bezier_point(curve, t)
        })
        .collect()
}

impl GraphApp {
    pub(in crate::app) fn find_node_at_with_alt(
        &self,
        local: Pos2,
        _alt_passthrough: bool,
    ) -> Option<(usize, egui::Vec2)> {
        let node_id = self
            .nodes
            .iter()
            .rev()
            .find(|node| {
                node.kind != NodeKind::Group && Rect::from_min_size(node.pos, node.size).contains(local)
            })
            .map(|node| node.id)
            .or_else(|| self.top_group_id_at(local))?;

        self.nodes
            .iter()
            .find(|n| n.id == node_id)
            .map(|n| (n.id, n.pos.to_vec2()))
    }

    pub(in crate::app) fn find_node_hit_with_alt(
        &self,
        local: Pos2,
        alt_passthrough: bool,
    ) -> Option<(usize, egui::Vec2, bool)> {
        let (id, node_pos) = self.find_node_at_with_alt(local, alt_passthrough)?;
        let n = self.nodes.iter().find(|node| node.id == id)?;

        let can_drag = match n.kind {
            NodeKind::Text | NodeKind::Image | NodeKind::Group => true,
            NodeKind::Terminal => local.y <= n.pos.y + TERMINAL_HEADER_HEIGHT,
            NodeKind::Decision => local.y <= n.pos.y + DECISION_HEADER_HEIGHT,
            NodeKind::Script => local.y <= n.pos.y + crate::constants::SCRIPT_HEADER_HEIGHT,
        };

        Some((id, node_pos, can_drag))
    }

    pub(in crate::app) fn ensure_camera_initialized(&mut self, canvas_rect: Rect) {
        if self.camera_initialized {
            return;
        }

        self.camera_world_center =
            ((canvas_rect.center() - canvas_rect.min - self.pan) / self.zoom).to_pos2();
        if !self.camera_world_center.x.is_finite() || !self.camera_world_center.y.is_finite() {
            self.camera_world_center = Pos2::new(0.0, 0.0);
        }
        self.camera_initialized = true;
        self.sync_pan_from_camera(canvas_rect);
    }

    pub(in crate::app) fn sync_pan_from_camera(&mut self, canvas_rect: Rect) {
        self.pan =
            canvas_rect.center() - canvas_rect.min - self.camera_world_center.to_vec2() * self.zoom;
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
                HistoryEntry::CreateBatch { nodes, .. }
                | HistoryEntry::DeleteBatch { nodes, .. } => {
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

    pub(in crate::app) fn clamp_edge_curve_bias(bias: f32) -> f32 {
        clamp_edge_curve_bias(bias)
    }

    pub(in crate::app) fn clamp_edge_control_offset(offset: egui::Vec2) -> egui::Vec2 {
        clamp_edge_control_offset(offset)
    }

    pub(in crate::app) fn edge_base_curve_local(
        &self,
        from: usize,
        to: usize,
    ) -> Option<EdgeCurve> {
        let source = self.nodes.iter().find(|n| n.id == from)?;
        let target = self.nodes.iter().find(|n| n.id == to)?;
        Some(edge_curve_from_nodes(source, target))
    }

    pub(in crate::app) fn edge_curve_local(&self, from: usize, to: usize) -> Option<EdgeCurve> {
        let base_curve = self.edge_base_curve_local(from, to)?;
        let bias = self.edge_curve_bias(from, to);
        let mut curve = edge_curve_with_bias(&base_curve, bias);

        let offsets = self.edge_control_offsets(from, to);
        curve.ctrl1 += Self::clamp_edge_control_offset(offsets.source);
        curve.ctrl2 += Self::clamp_edge_control_offset(offsets.target);

        Some(curve)
    }

    pub(in crate::app) fn edge_control_handle_world_pos_local(
        &self,
        from: usize,
        to: usize,
        handle: EdgeControlHandle,
    ) -> Option<Pos2> {
        let curve = self.edge_curve_local(from, to)?;
        Some(match handle {
            EdgeControlHandle::Source => curve.ctrl1,
            EdgeControlHandle::Target => curve.ctrl2,
        })
    }

    pub(in crate::app) fn edge_target_incoming_direction_local(
        &self,
        from: usize,
        to: usize,
    ) -> Option<egui::Vec2> {
        let source = self.nodes.iter().find(|n| n.id == from)?;
        let target = self.nodes.iter().find(|n| n.id == to)?;

        let source_center = Rect::from_min_size(source.pos, source.size).center();
        let target_center = Rect::from_min_size(target.pos, target.size).center();
        let (_, target_side) = pick_edge_anchor_sides(source_center, target_center);
        let dir = -target_side.direction();

        (dir.length_sq() > f32::EPSILON).then_some(dir.normalized())
    }

    pub(in crate::app) fn edge_control_offset_from_pointer_local(
        &self,
        from: usize,
        to: usize,
        handle: EdgeControlHandle,
        pointer: Pos2,
    ) -> Option<egui::Vec2> {
        let base_curve = self.edge_base_curve_local(from, to)?;
        let biased_curve = edge_curve_with_bias(&base_curve, self.edge_curve_bias(from, to));
        let anchor = match handle {
            EdgeControlHandle::Source => biased_curve.ctrl1,
            EdgeControlHandle::Target => biased_curve.ctrl2,
        };

        Some(Self::clamp_edge_control_offset(pointer - anchor))
    }

    pub(in crate::app) fn edge_curve_segments_local(
        &self,
        from: usize,
        to: usize,
    ) -> Option<Vec<Pos2>> {
        self.edge_curve_local(from, to)
            .map(|curve| sample_edge_curve(&curve, EDGE_CURVE_SAMPLE_SEGMENTS))
    }

    pub(in crate::app) fn find_edge_at(
        &self,
        local: Pos2,
        tolerance_world: f32,
    ) -> Option<(usize, usize)> {
        self.edges.iter().rev().copied().find(|(from, to)| {
            self.edge_curve_segments_local(*from, *to)
                .is_some_and(|samples| {
                    Self::distance_to_polyline(local, &samples) <= tolerance_world
                })
        })
    }

    /// Find an output port circle near the given world position (for port drag start).
    /// Returns (node_id, port_name) if a port is within tolerance.
    pub(in crate::app) fn find_output_port_at(&self, world_pos: Pos2) -> Option<(usize, String)> {
        // Reuse the cached port positions from the last render frame
        let tolerance = 8.0 / self.zoom.max(0.01); // ~8px in screen, converted to world
        let mut best: Option<(usize, String, f32)> = None;
        for (&node_id, areas) in &self.script_node_port_positions {
            for area in areas {
                if area.is_input {
                    continue;
                }
                let dist = world_pos.distance(area.world_pos);
                if dist <= tolerance && (best.is_none() || dist < best.as_ref().unwrap().2) {
                    best = Some((node_id, area.port_name.clone(), dist));
                }
            }
        }
        best.map(|(id, name, _)| (id, name))
    }

    /// Find an input port circle near the given world position (for port drag release).
    pub(in crate::app) fn find_input_port_at(&self, world_pos: Pos2) -> Option<(usize, String)> {
        let tolerance = 8.0 / self.zoom.max(0.01);
        let mut best: Option<(usize, String, f32)> = None;
        for (&node_id, areas) in &self.script_node_port_positions {
            for area in areas {
                if !area.is_input {
                    continue;
                }
                let dist = world_pos.distance(area.world_pos);
                if dist <= tolerance && (best.is_none() || dist < best.as_ref().unwrap().2) {
                    best = Some((node_id, area.port_name.clone(), dist));
                }
            }
        }
        best.map(|(id, name, _)| (id, name))
    }

    pub(in crate::app) fn edge_label_world_pos(&self, from: usize, to: usize) -> Option<Pos2> {
        self.edge_curve_local(from, to)
            .map(|curve| cubic_bezier_point(&curve, 0.5))
    }

    fn distance_to_segment(point: Pos2, a: Pos2, b: Pos2) -> f32 {
        let ab = b - a;
        let ab_len_sq = ab.length_sq();
        if ab_len_sq <= f32::EPSILON {
            return point.distance(a);
        }

        let t = ((point - a).dot(ab) / ab_len_sq).clamp(0.0, 1.0);
        let projection = a + ab * t;
        point.distance(projection)
    }

    fn distance_to_polyline(point: Pos2, points: &[Pos2]) -> f32 {
        if points.len() < 2 {
            return f32::INFINITY;
        }

        points
            .windows(2)
            .map(|pair| Self::distance_to_segment(point, pair[0], pair[1]))
            .fold(f32::INFINITY, f32::min)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{NodeData, NodeKind};
    use eframe::egui::{pos2, vec2};

    fn test_node(id: usize, pos: Pos2, size: egui::Vec2) -> Node {
        Node {
            id,
            uid: format!("u-{id}"),
            kind: NodeKind::Text,
            data: NodeData::Text {
                text_body: String::new(),
                auto_size: false,
            },
            pos,
            size,
        }
    }

    #[test]
    fn anchor_side_prefers_horizontal_when_dx_dominant() {
        let (source, target) = pick_edge_anchor_sides(pos2(0.0, 0.0), pos2(120.0, 10.0));
        assert_eq!(source, EdgeAnchorSide::Right);
        assert_eq!(target, EdgeAnchorSide::Left);
    }

    #[test]
    fn anchor_side_falls_back_to_vertical_when_dy_dominant() {
        let (source, target) = pick_edge_anchor_sides(pos2(0.0, 0.0), pos2(10.0, 160.0));
        assert_eq!(source, EdgeAnchorSide::Bottom);
        assert_eq!(target, EdgeAnchorSide::Top);
    }

    #[test]
    fn reverse_connection_boosts_control_offset() {
        let source = test_node(1, pos2(0.0, 0.0), vec2(100.0, 60.0));
        let target_forward = test_node(2, pos2(260.0, 0.0), vec2(100.0, 60.0));
        let target_reverse = test_node(3, pos2(-260.0, 0.0), vec2(100.0, 60.0));

        let forward_curve = edge_curve_from_nodes(&source, &target_forward);
        let reverse_curve = edge_curve_from_nodes(&source, &target_reverse);

        let forward_len = (forward_curve.ctrl1 - forward_curve.start).length();
        let reverse_len = (reverse_curve.ctrl1 - reverse_curve.start).length();

        assert!(reverse_len > forward_len);
    }

    #[test]
    fn control_offset_is_clamped() {
        let min_offset = edge_control_offset(pos2(0.0, 0.0), pos2(1.0, 0.5), EdgeAnchorSide::Right);
        assert!(min_offset >= EDGE_CONTROL_OFFSET_MIN);

        let max_forward = edge_control_offset(
            pos2(0.0, 0.0),
            pos2(20_000.0, 5_000.0),
            EdgeAnchorSide::Right,
        );
        assert!(max_forward <= EDGE_CONTROL_OFFSET_MAX + 0.001);

        let max_reverse = edge_control_offset(
            pos2(0.0, 0.0),
            pos2(-20_000.0, 5_000.0),
            EdgeAnchorSide::Left,
        );
        assert!(max_reverse <= EDGE_CONTROL_OFFSET_REVERSE_MAX + 0.001);
    }

    #[test]
    fn edge_curve_bias_is_clamped() {
        assert_eq!(clamp_edge_curve_bias(0.0), 0.0);
        assert_eq!(clamp_edge_curve_bias(9_999.0), EDGE_CURVE_BIAS_MAX);
        assert_eq!(clamp_edge_curve_bias(-9_999.0), EDGE_CURVE_BIAS_MIN);
    }

    #[test]
    fn pointer_projection_tracks_bias_direction() {
        let source = test_node(1, pos2(0.0, 0.0), vec2(100.0, 60.0));
        let target = test_node(2, pos2(260.0, 80.0), vec2(100.0, 60.0));
        let base_curve = edge_curve_from_nodes(&source, &target);
        let (midpoint, normal) = edge_curve_handle_basis(&base_curve);

        let positive_pointer = midpoint + normal * 56.0;
        let negative_pointer = midpoint - normal * 42.0;

        let positive_bias = edge_curve_bias_from_pointer(&base_curve, positive_pointer);
        let negative_bias = edge_curve_bias_from_pointer(&base_curve, negative_pointer);

        assert!(positive_bias > 50.0);
        assert!(negative_bias < -36.0);
    }

    #[test]
    fn handle_position_changes_with_bias() {
        let source = test_node(1, pos2(0.0, 0.0), vec2(120.0, 80.0));
        let target = test_node(2, pos2(300.0, 40.0), vec2(120.0, 80.0));
        let base_curve = edge_curve_from_nodes(&source, &target);

        let default_handle = edge_curve_handle_world_pos(&base_curve, 0.0);
        let positive_handle = edge_curve_handle_world_pos(&base_curve, 80.0);
        let negative_handle = edge_curve_handle_world_pos(&base_curve, -80.0);

        assert!(positive_handle.distance(default_handle) > 60.0);
        assert!(negative_handle.distance(default_handle) > 60.0);
        assert!(positive_handle.distance(negative_handle) > 120.0);
    }

    #[test]
    fn sampled_curve_hit_distance_matches_near_point() {
        let source = test_node(1, pos2(0.0, 0.0), vec2(120.0, 80.0));
        let target = test_node(2, pos2(320.0, 120.0), vec2(120.0, 80.0));

        let curve = edge_curve_from_nodes(&source, &target);
        let sample = cubic_bezier_point(&curve, 0.5);
        let sampled_curve = sample_edge_curve(&curve, EDGE_CURVE_SAMPLE_SEGMENTS);

        let near_point = sample + vec2(4.0, -3.0);
        let far_point = sample + vec2(120.0, 120.0);

        let near_dist = GraphApp::distance_to_polyline(near_point, &sampled_curve);
        let far_dist = GraphApp::distance_to_polyline(far_point, &sampled_curve);

        assert!(near_dist < 8.0);
        assert!(far_dist > 40.0);
    }
}

use super::{EdgeControlHandle, EdgeControlOffsets, GraphApp, NodeOrderAction};
use crate::model::{DecisionButton, Node, NodeData, NodeKind};
use chrono::Local;
use eframe::egui::{self, vec2, ColorImage, Pos2, Rect, TextureOptions};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

impl GraphApp {
    fn new_base_node(&mut self, kind: NodeKind, pos: Pos2, size: egui::Vec2) -> Node {
        let data = match kind {
            NodeKind::Terminal => NodeData::Terminal {
                title: "Terminal".to_owned(),
                startup_script: String::new(),
            },
            NodeKind::Text => NodeData::Text {
                text_body: String::new(),
                auto_size: false,
            },
            NodeKind::Image => NodeData::Image {
                image_path: String::new(),
            },
            NodeKind::Decision => NodeData::Decision {
                title: "Decision".to_owned(),
                buttons: vec![
                    DecisionButton {
                        label: "通过".to_owned(),
                        event_key: "approve".to_owned(),
                        color_rgb: Some([212, 244, 226]),
                    },
                    DecisionButton {
                        label: "驳回".to_owned(),
                        event_key: "reject".to_owned(),
                        color_rgb: Some([248, 208, 208]),
                    },
                ],
                pending_message: None,
                pending_messages: Vec::new(),
            },
        };

        Node {
            id: self.alloc_node_id(),
            uid: Uuid::new_v4().to_string(),
            kind,
            data,
            pos,
            size,
        }
    }

    fn push_node_and_select(&mut self, node: Node) -> usize {
        let id = node.id;
        self.push_history(super::history::HistoryEntry::CreateBatch {
            nodes: vec![node.clone()],
            edges: Vec::new(),
        });
        self.nodes.push(node);
        self.set_single_selection(id);
        self.mark_workspace_dirty();
        id
    }

    pub(in crate::app) fn create_terminal_node(&mut self, pos: Pos2) -> usize {
        let node = self.new_base_node(NodeKind::Terminal, pos, vec2(840.0, 660.0));
        self.push_node_and_select(node)
    }

    pub(in crate::app) fn create_text_node(&mut self, pos: Pos2, edit_now: bool) -> usize {
        let mut node = self.new_base_node(NodeKind::Text, pos, vec2(260.0, 140.0));
        if let NodeData::Text {
            text_body,
            auto_size,
        } = &mut node.data
        {
            *text_body = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            *auto_size = false;
        }
        let id = self.push_node_and_select(node);
        if edit_now {
            self.editing_text_node = Some(id);
            self.pending_text_focus = Some(id);
        }
        id
    }

    pub(in crate::app) fn advance_spawn_pos_below_selected(&self, spawn_pos: &mut Pos2) {
        if let Some(id) = self.selected {
            if let Some(node) = self.nodes.iter().find(|n| n.id == id) {
                spawn_pos.y = node.pos.y + node.size.y + 16.0;
            }
        }
    }

    pub(in crate::app) fn create_decision_node(&mut self, pos: Pos2) -> usize {
        let node = self.new_base_node(NodeKind::Decision, pos, vec2(320.0, 220.0));
        self.push_node_and_select(node)
    }

    pub(in crate::app) fn create_image_node_from_path(
        &mut self,
        pos: Pos2,
        image_path: String,
    ) -> usize {
        let size = image::image_dimensions(&image_path)
            .ok()
            .filter(|(w, h)| *w > 0 && *h > 0)
            .map(|(w, h)| vec2(w as f32, h as f32))
            .unwrap_or(vec2(320.0, 220.0));

        let mut node = self.new_base_node(NodeKind::Image, pos, size);
        if node.size.y > 0.0 {
            self.image_aspects
                .insert(node.id, node.size.x / node.size.y);
        }

        if let NodeData::Image {
            image_path: stored_path,
        } = &mut node.data
        {
            *stored_path = image_path;
        }
        self.push_node_and_select(node)
    }

    pub(in crate::app) fn create_image_node_from_bytes(
        &mut self,
        pos: Pos2,
        display_name: String,
        bytes: Vec<u8>,
    ) {
        match self.persist_image_bytes_to_artifact(&bytes) {
            Ok(relative_path) => {
                self.create_image_node_from_path(pos, relative_path);
            }
            Err(err) => {
                eprintln!(
                    "failed to persist dropped image bytes, fallback to in-memory image: {err}"
                );

                let mut size = vec2(320.0, 220.0);
                if let Ok(color_image) = Self::decode_image_bytes(&bytes) {
                    let [w, h] = color_image.size;
                    if w > 0 && h > 0 {
                        size = vec2(w as f32, h as f32);
                    }
                }

                let mut node = self.new_base_node(NodeKind::Image, pos, size);
                if let NodeData::Image {
                    image_path: stored_path,
                } = &mut node.data
                {
                    *stored_path = display_name;
                }
                let id = self.push_node_and_select(node);
                self.image_bytes.insert(id, bytes);
            }
        }
    }

    pub(in crate::app) fn create_image_node_from_color_image(
        &mut self,
        pos: Pos2,
        display_name: String,
        color_image: ColorImage,
        ctx: &egui::Context,
    ) {
        match self.persist_clipboard_color_image(&color_image) {
            Ok(relative_path) => {
                self.create_image_node_from_path(pos, relative_path);
            }
            Err(err) => {
                eprintln!("failed to persist clipboard image, fallback to in-memory image: {err}");

                let [w, h] = color_image.size;
                let aspect = if h == 0 { 1.0 } else { w as f32 / h as f32 };

                let mut node = self.new_base_node(NodeKind::Image, pos, vec2(w as f32, h as f32));
                if let NodeData::Image {
                    image_path: stored_path,
                } = &mut node.data
                {
                    *stored_path = display_name;
                }
                let id = self.push_node_and_select(node);

                let texture = ctx.load_texture(
                    format!("image-node-{id}"),
                    color_image,
                    TextureOptions::LINEAR,
                );
                self.image_textures.insert(id, texture);
                self.image_errors.remove(&id);
                self.image_bytes.remove(&id);
                self.image_aspects.insert(id, aspect);
            }
        }
    }

    pub(in crate::app) fn has_edge(&self, from: usize, to: usize) -> bool {
        self.edges.iter().any(|(a, b)| *a == from && *b == to)
    }

    pub(in crate::app) fn edge_route_key(&self, from: usize, to: usize) -> Option<&str> {
        self.edge_route_keys
            .get(&(from, to))
            .map(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
    }

    pub(in crate::app) fn set_edge_route_key(&mut self, from: usize, to: usize, route_key: String) {
        let trimmed = route_key.trim();
        if trimmed.is_empty() {
            self.edge_route_keys.remove(&(from, to));
            return;
        }

        self.edge_route_keys.insert((from, to), trimmed.to_owned());
    }

    pub(in crate::app) fn remove_edge_route_key(&mut self, from: usize, to: usize) {
        self.edge_route_keys.remove(&(from, to));
    }

    pub(in crate::app) fn prune_edge_route_keys(&mut self) {
        let existing: HashSet<(usize, usize)> = self.edges.iter().copied().collect();
        self.edge_route_keys
            .retain(|edge, route| existing.contains(edge) && !route.trim().is_empty());
    }

    pub(in crate::app) fn edge_curve_bias(&self, from: usize, to: usize) -> f32 {
        self.edge_curve_biases
            .get(&(from, to))
            .copied()
            .unwrap_or(0.0)
    }

    pub(in crate::app) fn set_edge_curve_bias(&mut self, from: usize, to: usize, bias: f32) {
        let clamped = Self::clamp_edge_curve_bias(bias);
        if clamped.abs() <= 0.001 {
            self.edge_curve_biases.remove(&(from, to));
        } else {
            self.edge_curve_biases.insert((from, to), clamped);
        }
    }

    pub(in crate::app) fn remove_edge_curve_bias(&mut self, from: usize, to: usize) {
        self.edge_curve_biases.remove(&(from, to));
    }

    pub(in crate::app) fn edge_control_offsets(
        &self,
        from: usize,
        to: usize,
    ) -> EdgeControlOffsets {
        self.edge_control_offsets
            .get(&(from, to))
            .copied()
            .unwrap_or_default()
    }

    pub(in crate::app) fn edge_control_offset(
        &self,
        from: usize,
        to: usize,
        handle: EdgeControlHandle,
    ) -> egui::Vec2 {
        let offsets = self.edge_control_offsets(from, to);
        match handle {
            EdgeControlHandle::Source => offsets.source,
            EdgeControlHandle::Target => offsets.target,
        }
    }

    pub(in crate::app) fn set_edge_control_offset(
        &mut self,
        from: usize,
        to: usize,
        handle: EdgeControlHandle,
        offset: egui::Vec2,
    ) {
        let mut offsets = self.edge_control_offsets(from, to);
        let clamped = Self::clamp_edge_control_offset(offset);
        match handle {
            EdgeControlHandle::Source => offsets.source = clamped,
            EdgeControlHandle::Target => offsets.target = clamped,
        }
        self.set_edge_control_offsets(from, to, offsets);
    }

    pub(in crate::app) fn set_edge_control_offsets(
        &mut self,
        from: usize,
        to: usize,
        offsets: EdgeControlOffsets,
    ) {
        let source = Self::clamp_edge_control_offset(offsets.source);
        let target = Self::clamp_edge_control_offset(offsets.target);
        let is_default = source.length_sq() <= 0.01 && target.length_sq() <= 0.01;
        if is_default {
            self.edge_control_offsets.remove(&(from, to));
        } else {
            self.edge_control_offsets
                .insert((from, to), EdgeControlOffsets { source, target });
        }
    }

    pub(in crate::app) fn remove_edge_control_offsets(&mut self, from: usize, to: usize) {
        self.edge_control_offsets.remove(&(from, to));
    }

    pub(in crate::app) fn edge_has_custom_curve(&self, from: usize, to: usize) -> bool {
        if self.edge_curve_bias(from, to).abs() > 0.001 {
            return true;
        }

        let offsets = self.edge_control_offsets(from, to);
        offsets.source.length_sq() > 0.01 || offsets.target.length_sq() > 0.01
    }

    pub(in crate::app) fn prune_edge_curve_biases(&mut self) {
        let existing: HashSet<(usize, usize)> = self.edges.iter().copied().collect();
        self.edge_curve_biases
            .retain(|edge, bias| existing.contains(edge) && bias.is_finite() && bias.abs() > 0.001);

        self.edge_control_offsets.retain(|edge, offsets| {
            if !existing.contains(edge) {
                return false;
            }

            let source = Self::clamp_edge_control_offset(offsets.source);
            let target = Self::clamp_edge_control_offset(offsets.target);
            let keep = source.length_sq() > 0.01 || target.length_sq() > 0.01;
            if keep {
                *offsets = EdgeControlOffsets { source, target };
            }
            keep
        });

        if self
            .selected_edge
            .is_some_and(|(from, to)| !existing.contains(&(from, to)))
        {
            self.clear_edge_selection();
        }

        if self
            .editing_edge
            .is_some_and(|(from, to)| !existing.contains(&(from, to)))
        {
            self.cancel_edge_edit();
        }

        if self
            .dragging_edge_control
            .is_some_and(|(edge, _, _)| !existing.contains(&edge))
        {
            self.dragging_edge_control = None;
        }

        if self
            .context_menu_edge
            .is_some_and(|edge| !existing.contains(&edge))
        {
            self.context_menu_edge = None;
        }
    }

    pub(in crate::app) fn prune_edge_state(&mut self) {
        self.prune_edge_route_keys();
        self.prune_edge_curve_biases();
    }

    pub(in crate::app) fn reset_selected_edge_curve(&mut self) -> bool {
        let Some((from, to)) = self.selected_edge else {
            return false;
        };

        if !self.edge_has_custom_curve(from, to) {
            return false;
        }

        self.remove_edge_curve_bias(from, to);
        self.remove_edge_control_offsets(from, to);
        self.mark_workspace_dirty();
        true
    }

    pub(in crate::app) fn reset_selected_edge_curve_bias(&mut self) -> bool {
        self.reset_selected_edge_curve()
    }

    pub(in crate::app) fn cut_edges_intersecting_segment(&mut self, cut_a: Pos2, cut_b: Pos2) {
        let hit: Vec<bool> = self
            .edges
            .iter()
            .map(|(from, to)| {
                self.edge_curve_segments_local(*from, *to)
                    .is_some_and(|segments| {
                        segments
                            .windows(2)
                            .any(|pair| Self::segments_intersect(cut_a, cut_b, pair[0], pair[1]))
                    })
            })
            .collect();

        let mut idx = 0usize;
        self.edges.retain(|_| {
            let keep = !hit[idx];
            idx += 1;
            keep
        });
        self.prune_edge_state();
    }

    pub(in crate::app) fn cut_nodes_intersecting_segment(&mut self, cut_a: Pos2, cut_b: Pos2) {
        let hit_ids: Vec<usize> = self
            .nodes
            .iter()
            .filter(|n| {
                let rect = Rect::from_min_size(n.pos, n.size);
                Self::segment_intersects_rect(cut_a, cut_b, rect)
            })
            .map(|n| n.id)
            .collect();

        for id in hit_ids {
            self.remove_node(id);
        }
    }

    fn ordered_selected_ids(&self) -> Vec<usize> {
        self.nodes
            .iter()
            .filter(|n| self.selected_nodes.contains(&n.id))
            .map(|n| n.id)
            .collect()
    }

    fn selection_or_single(&self, node_id: usize) -> HashSet<usize> {
        if self.selected_nodes.contains(&node_id) && !self.selected_nodes.is_empty() {
            self.selected_nodes.clone()
        } else {
            let mut picked = HashSet::new();
            picked.insert(node_id);
            picked
        }
    }

    pub(in crate::app) fn apply_node_order(&mut self, order: &[usize]) {
        let mut map: HashMap<usize, Node> = std::mem::take(&mut self.nodes)
            .into_iter()
            .map(|node| (node.id, node))
            .collect();

        let mut reordered = Vec::with_capacity(map.len());
        for id in order {
            if let Some(node) = map.remove(id) {
                reordered.push(node);
            }
        }
        reordered.extend(map.into_values());
        self.nodes = reordered;
    }

    fn record_reorder_history(&mut self, before: Vec<usize>) {
        let after: Vec<usize> = self.nodes.iter().map(|n| n.id).collect();
        if before == after {
            return;
        }

        self.push_history(super::history::HistoryEntry::ReorderNodes { before });
    }

    pub(in crate::app) fn bring_selection_to_front(&mut self) {
        let selected = self.selected_nodes.clone();
        if selected.is_empty() {
            return;
        }

        let before: Vec<usize> = self.nodes.iter().map(|n| n.id).collect();
        self.nodes
            .sort_by_key(|node| usize::from(selected.contains(&node.id)));
        self.record_reorder_history(before);
        self.selected = self.ordered_selected_ids().last().copied();
    }

    pub(in crate::app) fn send_selection_to_back(&mut self) {
        let selected = self.selected_nodes.clone();
        if selected.is_empty() {
            return;
        }

        let before: Vec<usize> = self.nodes.iter().map(|n| n.id).collect();
        self.nodes
            .sort_by_key(|node| usize::from(!selected.contains(&node.id)));
        self.record_reorder_history(before);
        self.selected = self.ordered_selected_ids().last().copied();
    }

    pub(in crate::app) fn bring_selection_forward_one(&mut self) {
        if self.selected_nodes.is_empty() {
            return;
        }

        let before: Vec<usize> = self.nodes.iter().map(|n| n.id).collect();
        for idx in (0..self.nodes.len().saturating_sub(1)).rev() {
            let current_selected = self.selected_nodes.contains(&self.nodes[idx].id);
            let next_selected = self.selected_nodes.contains(&self.nodes[idx + 1].id);
            if current_selected && !next_selected {
                self.nodes.swap(idx, idx + 1);
            }
        }

        self.record_reorder_history(before);
        self.selected = self.ordered_selected_ids().last().copied();
    }

    pub(in crate::app) fn send_selection_backward_one(&mut self) {
        if self.selected_nodes.is_empty() {
            return;
        }

        let before: Vec<usize> = self.nodes.iter().map(|n| n.id).collect();
        for idx in 1..self.nodes.len() {
            let current_selected = self.selected_nodes.contains(&self.nodes[idx].id);
            let prev_selected = self.selected_nodes.contains(&self.nodes[idx - 1].id);
            if current_selected && !prev_selected {
                self.nodes.swap(idx - 1, idx);
            }
        }

        self.record_reorder_history(before);
        self.selected = self.ordered_selected_ids().last().copied();
    }

    pub(in crate::app) fn reorder_from_context(&mut self, node_id: usize, mode: NodeOrderAction) {
        let target_selection = self.selection_or_single(node_id);
        self.selected_nodes = target_selection;

        match mode {
            NodeOrderAction::BringToFront => self.bring_selection_to_front(),
            NodeOrderAction::BringForwardOne => self.bring_selection_forward_one(),
            NodeOrderAction::SendBackwardOne => self.send_selection_backward_one(),
            NodeOrderAction::SendToBack => self.send_selection_to_back(),
        }
    }

    pub(in crate::app) fn remove_node(&mut self, node_id: usize) {
        self.mark_workspace_dirty();
        self.nodes.retain(|n| n.id != node_id);
        self.edges
            .retain(|(from, to)| *from != node_id && *to != node_id);
        self.prune_edge_state();
        self.terminal_backends.remove(&node_id);
        self.terminal_exited.remove(&node_id);
        self.terminal_errors.remove(&node_id);
        self.pending_terminal_injections.remove(&node_id);
        self.pending_terminal_starts.retain(|id| *id != node_id);
        self.image_textures.remove(&node_id);
        self.image_errors.remove(&node_id);
        self.image_bytes.remove(&node_id);
        self.image_aspects.remove(&node_id);

        if self.editing_decision_buttons_node == Some(node_id) {
            self.editing_decision_buttons_node = None;
            self.pending_decision_buttons_focus = None;
            self.decision_buttons_edit_rows.clear();
            self.decision_color_popup = None;
            self.decision_color_popup_pos = None;
            self.decision_buttons_edit_error = None;
        }

        if self.editing_decision_queue_node == Some(node_id) {
            self.editing_decision_queue_node = None;
            self.pending_decision_queue_focus = None;
            self.decision_queue_edit_buffer.clear();
        }

        self.selected_nodes.remove(&node_id);
        if self.selected == Some(node_id) {
            self.selected = self.selected_nodes.iter().copied().next();
        }
        if self.dragging.is_some_and(|(id, _)| id == node_id) {
            self.dragging = None;
            self.drag_start_pos = None;
            self.drag_group_start = None;
        }
        if self
            .drag_group_start
            .as_ref()
            .is_some_and(|(_, nodes)| nodes.iter().any(|(id, _)| *id == node_id))
        {
            self.dragging = None;
            self.drag_start_pos = None;
            self.drag_group_start = None;
        }
        if self.resizing.is_some_and(|(id, _, _)| id == node_id) {
            self.resizing = None;
        }
        if self.editing_text_node == Some(node_id) {
            self.editing_text_node = None;
            self.pending_text_focus = None;
        }
        if self.editing_title_node == Some(node_id) {
            self.editing_title_node = None;
            self.pending_title_focus = None;
            self.title_edit_buffer.clear();
        }
        if self.editing_startup_node == Some(node_id) {
            self.editing_startup_node = None;
            self.pending_startup_focus = None;
            self.startup_edit_buffer.clear();
        }
        if self.suspend_terminal_focus == Some(node_id) {
            self.suspend_terminal_focus = None;
        }
        if self
            .editing_edge
            .is_some_and(|(from, to)| from == node_id || to == node_id)
        {
            self.cancel_edge_edit();
        }
        if self.linking_from == Some(node_id) {
            self.linking_from = None;
            self.linking_pointer_local = None;
        }
        if self.context_menu_node == Some(node_id) {
            self.context_menu_node = None;
        }
        if self
            .context_menu_edge
            .is_some_and(|(from, to)| from == node_id || to == node_id)
        {
            self.context_menu_edge = None;
        }
    }
}

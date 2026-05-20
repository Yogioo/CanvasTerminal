use super::super::{EdgeControlHandle, GraphApp};
use crate::constants::TERMINAL_HEADER_HEIGHT;
use crate::model::NodeKind;
use eframe::egui::{self, vec2, Pos2, Rect, Response, Ui, Vec2};
use std::collections::HashSet;

impl GraphApp {
    pub(in crate::app::ui) fn handle_canvas_pointer_interactions(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        response: &Response,
        rect: Rect,
        pointer_pos: Option<Pos2>,
        any_popup_open: bool,
        is_panning: bool,
        pointer_over_terminal_content: bool,
        pointer_in_window_top_strip: bool,
        pointer_in_window_resize_strip: bool,
        primary_clicked: bool,
        primary_pressed: bool,
        multi_select_modifier: bool,
        subtract_select_modifier: bool,
        current_time: f64,
        edge_hit_tolerance: f32,
        edge_handle_hit: Option<((usize, usize), EdgeControlHandle)>,
        secondary_pressed: bool,
        secondary_down: bool,
        secondary_released: bool,
    ) -> (bool, Option<(usize, Pos2, Vec2)>) {
        let mut tolerant_double_click = false;
        if primary_clicked {
            if let Some(pointer) = pointer_pos {
                let local = self.screen_to_world_pos(rect, pointer);
                let alt_passthrough = ctx.input(|i| i.modifiers.alt);
                if let Some((node_id, _)) = self.find_node_at_with_alt(local, alt_passthrough) {
                    if let Some(node) = self.nodes.iter().find(|n| n.id == node_id) {
                        if node.kind == NodeKind::Terminal
                            && local.y > node.pos.y + TERMINAL_HEADER_HEIGHT
                        {
                            self.set_single_selection(node_id);
                            self.editing_text_node = None;
                            if self.suspend_terminal_focus == Some(node_id) {
                                self.suspend_terminal_focus = None;
                            }
                        }
                    }
                }

                if !any_popup_open
                    && !is_panning
                    && !pointer_over_terminal_content
                    && !pointer_in_window_top_strip
                    && !pointer_in_window_resize_strip
                {
                    if let (Some(last_time), Some(last_pos)) =
                        (self.last_primary_click_time, self.last_primary_click_pos)
                    {
                        tolerant_double_click =
                            current_time - last_time <= 0.45 && last_pos.distance(pointer) <= 24.0;
                    }
                    self.last_primary_click_time = Some(current_time);
                    self.last_primary_click_pos = Some(pointer);
                }
            }
        }

        let resize_handle_hit = pointer_pos.and_then(|pointer| {
            let selected_id = self.selected?;
            let node = self.nodes.iter().find(|n| n.id == selected_id)?;
            if !matches!(
                node.kind,
                NodeKind::Terminal
                    | NodeKind::Image
                    | NodeKind::Text
                    | NodeKind::Decision
                    | NodeKind::Script
            ) {
                return None;
            }

            let node_rect =
                self.world_to_screen_rect(rect, Rect::from_min_size(node.pos, node.size));
            let handle_size = 18.0 * self.zoom.clamp(0.75, 1.6);
            let handle_rect = if false {
                // Draw handle outside the node to avoid conflict with live webview
                Rect::from_min_size(
                    node_rect.right_bottom() + vec2(2.0, 2.0),
                    vec2(handle_size + 6.0, handle_size + 6.0),
                )
            } else {
                Rect::from_min_size(
                    node_rect.right_bottom() - vec2(handle_size + 6.0, handle_size + 6.0),
                    vec2(handle_size + 6.0, handle_size + 6.0),
                )
            };
            if handle_rect.contains(pointer) {
                let local = self.screen_to_world_pos(rect, pointer);
                Some((selected_id, local, node.size))
            } else {
                None
            }
        });

        if is_panning {
            self.dragging = None;
            self.drag_start_pos = None;
            self.drag_group_start = None;
            self.dragging_edge_control = None;
            self.resizing = None;
            self.box_select_start = None;
            self.box_select_current = None;
            self.box_select_additive = false;
            self.box_select_subtractive = false;
            self.box_select_base_selection.clear();
            let delta = ctx.input(|i| i.pointer.delta());
            self.camera_world_center -= delta / self.zoom;
            self.sync_pan_from_camera(rect);
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);
        }

        if self.resizing.is_none() && resize_handle_hit.is_some() {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeNwSe);
        } else if self.dragging_edge_control.is_none() && edge_handle_hit.is_some() {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
        }

        if !is_panning
            && !any_popup_open
            && self.editing_title_node.is_none()
            && self.editing_startup_node.is_none()
            && self.editing_working_directory_node.is_none()
            && self.editing_script_node.is_none()
            && primary_clicked
            && ctx.input(|i| i.modifiers.alt)
        {
            if let Some(pointer) = pointer_pos {
                let local = self.screen_to_world_pos(rect, pointer);
                if self.jump_selected_nodes_to(local) {
                    self.dragging = None;
                    self.drag_start_pos = None;
                    self.drag_group_start = None;
                    self.dragging_edge_control = None;
                    self.resizing = None;
                    self.box_select_start = None;
                    self.box_select_current = None;
                    self.box_select_additive = false;
                    self.box_select_subtractive = false;
                    self.box_select_base_selection.clear();
                    return (tolerant_double_click, resize_handle_hit);
                }
            }
        }

        if !is_panning
            && !any_popup_open
            && !ctx.input(|i| i.modifiers.alt)
            && self.editing_title_node.is_none()
            && self.editing_startup_node.is_none()
            && self.editing_working_directory_node.is_none()
            && self.editing_script_node.is_none()
            && primary_pressed
            && !pointer_in_window_resize_strip
        {
            if let Some((edge, handle)) = edge_handle_hit {
                self.editing_text_node = None;
                self.set_edge_selection(edge);
                self.dragging = None;
                self.drag_start_pos = None;
                self.drag_group_start = None;
                self.resizing = None;
                self.box_select_start = None;
                self.box_select_current = None;
                self.box_select_additive = false;
                self.box_select_subtractive = false;
                self.box_select_base_selection.clear();
                self.dragging_edge_control = Some((
                    edge,
                    handle,
                    self.edge_control_offset(edge.0, edge.1, handle),
                ));
            } else if let Some((id, local, size)) = resize_handle_hit {
                if Some(id) != self.editing_text_node {
                    self.editing_text_node = None;
                }
                self.resizing = Some((id, local, size));
                self.dragging = None;
                self.drag_start_pos = None;
                self.drag_group_start = None;
                self.dragging_edge_control = None;
                self.set_single_selection(id);
            } else if !pointer_over_terminal_content {
                if let Some(pointer) = pointer_pos {
                    let local = self.screen_to_world_pos(rect, pointer);
                    let alt_passthrough = ctx.input(|i| i.modifiers.alt);

                    if let Some((id, node_pos, can_drag)) =
                        self.find_node_hit_with_alt(local, alt_passthrough)
                    {
                        if Some(id) != self.editing_text_node {
                            self.editing_text_node = None;
                        }

                        if Some(id) == self.editing_text_node {
                            self.dragging = None;
                            self.drag_start_pos = None;
                            self.drag_group_start = None;
                            self.dragging_edge_control = None;
                            self.box_select_start = None;
                            self.box_select_current = None;
                        } else if subtract_select_modifier {
                            self.remove_from_selection(id);
                            self.dragging = None;
                            self.drag_start_pos = None;
                            self.drag_group_start = None;
                            self.dragging_edge_control = None;
                        } else if multi_select_modifier {
                            self.toggle_selection(id);
                            self.dragging = None;
                            self.drag_start_pos = None;
                            self.drag_group_start = None;
                            self.dragging_edge_control = None;
                        } else {
                            let multi_drag =
                                self.selected_nodes.len() > 1 && self.selected_nodes.contains(&id);
                            if multi_drag {
                                self.selected = Some(id);
                                self.clear_edge_selection();
                            } else {
                                self.set_single_selection(id);
                            }

                            if can_drag {
                                self.dragging = Some((id, local.to_vec2() - node_pos));
                                let drag_ids = self.resolve_drag_node_ids(id, multi_drag);
                                let id_is_group = self
                                    .nodes
                                    .iter()
                                    .find(|n| n.id == id)
                                    .is_some_and(|n| n.kind == NodeKind::Group);

                                if drag_ids.len() > 1 || id_is_group {
                                    let start_nodes = self
                                        .nodes
                                        .iter()
                                        .filter(|n| drag_ids.contains(&n.id))
                                        .map(|n| (n.id, n.pos))
                                        .collect();
                                    self.drag_group_start = Some((local, start_nodes));
                                    self.drag_start_pos = None;
                                } else if let Some(single_id) = drag_ids.iter().copied().next() {
                                    if let Some(single_node) =
                                        self.nodes.iter().find(|n| n.id == single_id)
                                    {
                                        self.drag_group_start = None;
                                        self.drag_start_pos = Some((single_id, single_node.pos));
                                    }
                                }
                            }
                        }
                        self.box_select_start = None;
                        self.box_select_current = None;
                    } else if let Some(edge) = self.find_edge_at(local, edge_hit_tolerance) {
                        self.editing_text_node = None;
                        self.dragging = None;
                        self.drag_start_pos = None;
                        self.drag_group_start = None;
                        self.resizing = None;
                        self.box_select_start = None;
                        self.box_select_current = None;
                        self.box_select_additive = false;
                        self.box_select_subtractive = false;
                        self.box_select_base_selection.clear();
                        self.set_edge_selection(edge);
                    } else {
                        self.editing_text_node = None;
                        self.dragging = None;
                        self.drag_start_pos = None;
                        self.drag_group_start = None;
                        self.dragging_edge_control = None;
                        self.box_select_start = Some(local);
                        self.box_select_current = Some(local);
                        self.box_select_additive = multi_select_modifier;
                        self.box_select_subtractive = subtract_select_modifier;
                        self.box_select_base_selection = self.selected_nodes.clone();
                    }
                }
            }
        }

        if let Some(((from, to), handle, start_offset)) = self.dragging_edge_control {
            if ctx.input(|i| i.pointer.primary_down())
                && !ctx.input(|i| i.key_down(egui::Key::Space))
            {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);
                if let Some(pointer) = pointer_pos {
                    let local = self.screen_to_world_pos(rect, pointer);
                    if let Some(next_offset) =
                        self.edge_control_offset_from_pointer_local(from, to, handle, local)
                    {
                        self.set_edge_control_offset(from, to, handle, next_offset);
                    }
                }
            } else {
                let end_offset = self.edge_control_offset(from, to, handle);
                if (end_offset - start_offset).length_sq() > 0.01 {
                    self.mark_workspace_dirty();
                }
                self.dragging_edge_control = None;
            }
        }

        if let Some((resize_id, start_pointer, start_size)) = self.resizing {
            if ctx.input(|i| i.pointer.primary_down())
                && !ctx.input(|i| i.key_down(egui::Key::Space))
            {
                if let Some(pointer) = pointer_pos {
                    let local = self.screen_to_world_pos(rect, pointer);
                    let image_aspect = self
                        .image_aspect(resize_id)
                        .filter(|a| *a > 0.0)
                        .unwrap_or((start_size.x / start_size.y).max(0.1));
                    if let Some(node) = self.nodes.iter_mut().find(|n| n.id == resize_id) {
                        let delta = local - start_pointer;
                        match node.kind {
                            NodeKind::Image => {
                                let sx = (start_size.x + delta.x) / start_size.x.max(1.0);
                                let sy = (start_size.y + delta.y) / start_size.y.max(1.0);
                                let scale = sx.max(sy).max(120.0 / start_size.x.max(1.0));
                                let width = (start_size.x * scale).max(120.0);
                                let height = (width / image_aspect).max(90.0);
                                node.size = vec2(width, height);
                            }
                            NodeKind::Terminal => {
                                let width = (start_size.x + delta.x).max(320.0);
                                let height = (start_size.y + delta.y).max(170.0);
                                node.size = vec2(width, height);
                            }
                            NodeKind::Text => {
                                let width = (start_size.x + delta.x).max(120.0);
                                let height = (start_size.y + delta.y).max(60.0);
                                node.size = vec2(width, height);
                                if let crate::model::NodeData::Text { auto_size, .. } =
                                    &mut node.data
                                {
                                    *auto_size = false;
                                }
                            }

                            NodeKind::Decision => {
                                let width = (start_size.x + delta.x).max(220.0);
                                let height = (start_size.y + delta.y).max(140.0);
                                node.size = vec2(width, height);
                            }
                            NodeKind::Script => {
                                let width = (start_size.x + delta.x).max(260.0);
                                let height = (start_size.y + delta.y).max(160.0);
                                node.size = vec2(width, height);
                            }
                            NodeKind::Group => {}
                        }
                    }
                }
            } else {
                if let Some(node) = self.nodes.iter().find(|n| n.id == resize_id) {
                    if (node.size.x - start_size.x).abs() > 0.1
                        || (node.size.y - start_size.y).abs() > 0.1
                    {
                        self.mark_workspace_dirty();
                    }
                }
                self.resizing = None;
            }
        }

        if let Some((drag_id, offset)) = self.dragging {
            if ctx.input(|i| i.pointer.primary_down())
                && !ctx.input(|i| i.key_down(egui::Key::Space))
            {
                if let Some(pointer) = pointer_pos {
                    let local = self.screen_to_world_pos(rect, pointer);
                    if let Some((start_pointer, start_nodes)) = self.drag_group_start.clone() {
                        let delta = local - start_pointer;
                        for (node_id, start_pos) in start_nodes {
                            if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
                                node.pos = (start_pos.to_vec2() + delta).to_pos2();
                            }
                        }
                    } else if let Some(node) = self.nodes.iter_mut().find(|n| n.id == drag_id) {
                        node.pos = (local.to_vec2() - offset).to_pos2();
                    }
                }
            } else {
                if let Some((_, start_nodes)) = self.drag_group_start.take() {
                    let moved_nodes: Vec<(usize, Pos2, Pos2)> = start_nodes
                        .into_iter()
                        .filter_map(|(node_id, from)| {
                            self.nodes
                                .iter()
                                .find(|n| n.id == node_id)
                                .map(|node| (node_id, from, node.pos))
                        })
                        .collect();
                    self.record_nodes_move_history(moved_nodes);
                    self.drag_start_pos = None;
                } else if let Some((start_id, start_pos)) = self.drag_start_pos.take() {
                    if start_id == drag_id {
                        if let Some(node) = self.nodes.iter().find(|n| n.id == drag_id) {
                            self.record_move_history(drag_id, start_pos, node.pos);
                        }
                    }
                }
                self.dragging = None;
            }
        }

        if let Some(start) = self.box_select_start {
            if ctx.input(|i| i.pointer.primary_down()) {
                if let Some(pointer) = pointer_pos {
                    self.box_select_current = Some(self.screen_to_world_pos(rect, pointer));
                }
            } else {
                let end = self.box_select_current.unwrap_or(start);
                let moved = start.distance(end) >= 4.0;

                if moved {
                    let selection_rect = Rect::from_two_pos(start, end);
                    let hit_ids: Vec<usize> = self
                        .nodes
                        .iter()
                        .filter_map(|node| {
                            let node_rect = Rect::from_min_size(node.pos, node.size);
                            selection_rect.intersects(node_rect).then_some(node.id)
                        })
                        .collect();

                    let mut next_selection =
                        if self.box_select_additive || self.box_select_subtractive {
                            self.box_select_base_selection.clone()
                        } else {
                            HashSet::new()
                        };

                    if self.box_select_subtractive {
                        for id in hit_ids {
                            next_selection.remove(&id);
                        }
                    } else {
                        for id in hit_ids {
                            next_selection.insert(id);
                        }
                    }

                    self.selected_nodes = next_selection;
                    self.selected = self.selected_nodes.iter().copied().next();
                    self.clear_edge_selection();
                } else if !self.box_select_additive && !self.box_select_subtractive {
                    self.clear_selection();
                    self.editing_text_node = None;
                }

                self.box_select_start = None;
                self.box_select_current = None;
                self.box_select_additive = false;
                self.box_select_subtractive = false;
                self.box_select_base_selection.clear();
            }
        }

        if !is_panning && !pointer_over_terminal_content && secondary_pressed {
            self.right_drag_moved = false;
            self.cutting_path_local.clear();
            self.linking_from = None;
            self.linking_pointer_local = None;
            self.cut_snapshot_nodes = None;
            self.cut_snapshot_edges = None;

            if let Some(pointer_pos) = pointer_pos.or_else(|| response.interact_pointer_pos()) {
                let local = self.screen_to_world_pos(rect, pointer_pos);
                let alt_passthrough = ctx.input(|i| i.modifiers.alt);
                if let Some((id, _)) = self.find_node_at_with_alt(local, alt_passthrough) {
                    self.linking_from = Some(id);
                    self.linking_pointer_local = Some(local);
                    self.set_single_selection(id);
                } else {
                    self.cutting_path_local.push(local);
                    self.cut_snapshot_nodes = Some(self.nodes.clone());
                    self.cut_snapshot_edges = Some(self.edges.clone());
                }
            }
        }

        if secondary_down {
            if let Some(pointer_pos) = pointer_pos.or_else(|| response.interact_pointer_pos()) {
                let local = self.screen_to_world_pos(rect, pointer_pos);

                if self.linking_from.is_some() {
                    self.linking_pointer_local = Some(local);
                } else if let Some(prev) = self.cutting_path_local.last().copied() {
                    let right_drag_threshold_world = 6.0 / self.zoom.max(1e-4);
                    if prev.distance(local) > right_drag_threshold_world {
                        self.right_drag_moved = true;
                        self.cut_edges_intersecting_segment(prev, local);
                        self.cut_nodes_intersecting_segment(prev, local);
                        self.cutting_path_local.push(local);
                    }
                }
            }
        }

        if secondary_released {
            let mut suppress_context_menu_for_link_release = false;

            if let Some(from) = self.linking_from {
                if let Some(pointer_pos) = pointer_pos.or_else(|| response.interact_pointer_pos()) {
                    let local = self.screen_to_world_pos(rect, pointer_pos);
                    let alt_passthrough = ctx.input(|i| i.modifiers.alt);
                    if let Some((to, _)) = self.find_node_at_with_alt(local, alt_passthrough) {
                        if to != from {
                            suppress_context_menu_for_link_release = true;
                            if !self.has_edge(from, to) {
                                self.edges.push((from, to));
                                self.mark_workspace_dirty();
                            }
                        }
                    }
                }
                self.linking_from = None;
                self.linking_pointer_local = None;
            }

            if self.right_drag_moved {
                if let (Some(before_nodes), Some(before_edges)) = (
                    self.cut_snapshot_nodes.take(),
                    self.cut_snapshot_edges.take(),
                ) {
                    self.record_cut_history(before_nodes, before_edges);
                }
            } else {
                self.cut_snapshot_nodes = None;
                self.cut_snapshot_edges = None;

                if !suppress_context_menu_for_link_release
                    && !is_panning
                    && !pointer_over_terminal_content
                {
                    if let Some(pointer_pos) = pointer_pos.or_else(|| response.interact_pointer_pos()) {
                        let local = self.screen_to_world_pos(rect, pointer_pos);
                        let alt_passthrough = ctx.input(|i| i.modifiers.alt);
                        let context_menu_node = self
                            .find_node_at_with_alt(local, alt_passthrough)
                            .map(|(id, _)| id);
                        let context_menu_edge = if context_menu_node.is_none() {
                            self.find_edge_at(local, edge_hit_tolerance)
                        } else {
                            None
                        };

                        self.context_menu_local_pos = Some(local);
                        self.context_menu_node = context_menu_node;
                        self.context_menu_edge = context_menu_edge;

                        if let Some(node_id) = context_menu_node {
                            if Some(node_id) != self.editing_text_node {
                                self.editing_text_node = None;
                            }
                            self.set_single_selection(node_id);
                        } else if let Some(edge) = context_menu_edge {
                            self.editing_text_node = None;
                            self.set_edge_selection(edge);
                        } else {
                            self.clear_selection();
                            self.editing_text_node = None;
                            self.pending_text_focus = None;
                        }

                        self.reset_menu_search_state(true);
                    }
                }
            }

            self.cutting_path_local.clear();
        }

        (tolerant_double_click, resize_handle_hit)
    }
}

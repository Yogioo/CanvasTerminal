use super::super::GraphApp;
use crate::constants::TERMINAL_HEADER_HEIGHT;
use crate::model::NodeKind;
use eframe::egui::{
    self, vec2, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, TextEdit, Ui,
};
use egui_term::{TerminalFont, TerminalView};

impl GraphApp {
    pub(in crate::app) fn draw_canvas(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let available = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(available, Sense::click_and_drag());
        let painter = ui.painter_at(rect);

        painter.rect_filled(rect, 0.0, Color32::from_rgb(20, 22, 26));

        let is_space_down = ctx.input(|i| i.key_down(egui::Key::Space));
        let is_space_pan = ctx.input(|i| i.key_down(egui::Key::Space) && i.pointer.primary_down());
        let is_middle_pan = ctx.input(|i| i.pointer.middle_down());
        let secondary_pressed =
            ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary));
        let secondary_down = ctx.input(|i| i.pointer.button_down(egui::PointerButton::Secondary));
        let secondary_released =
            ctx.input(|i| i.pointer.button_released(egui::PointerButton::Secondary));
        let pointer_pos = ctx.input(|i| i.pointer.latest_pos().or_else(|| i.pointer.hover_pos()));
        let pointer_in_canvas = pointer_pos.is_some_and(|p| rect.contains(p));
        let any_popup_open = ctx.memory(|m| m.any_popup_open());

        let terminal_rects_before_zoom = self.terminal_content_rects_screen(rect);
        let pointer_over_terminal_before_zoom = pointer_pos.is_some_and(|p| {
            terminal_rects_before_zoom
                .iter()
                .any(|(_, term_rect)| term_rect.contains(p))
        });

        if pointer_in_canvas && !pointer_over_terminal_before_zoom {
            let zoom_change = ctx.input(|i| {
                let pinch = i.zoom_delta();
                let wheel = (-i.raw_scroll_delta.y * 0.0015).exp();
                pinch * wheel
            });
            if (zoom_change - 1.0).abs() > f32::EPSILON {
                if let Some(pointer) = pointer_pos {
                    let old_zoom = self.zoom;
                    let new_zoom = (old_zoom * zoom_change).clamp(0.35, 2.5);
                    if (new_zoom - old_zoom).abs() > f32::EPSILON {
                        let world_at_pointer = self.screen_to_world_pos(rect, pointer);
                        self.zoom = new_zoom;
                        self.pan = pointer - rect.min - world_at_pointer.to_vec2() * self.zoom;
                    }
                }
            }
        }

        self.paint_grid(&painter, rect, self.pan, self.zoom);

        let terminal_content_rects = self.terminal_content_rects_screen(rect);
        let pointer_over_terminal_content = pointer_pos.is_some_and(|p| {
            terminal_content_rects
                .iter()
                .any(|(_, term_rect)| term_rect.contains(p))
        });

        let primary_clicked = ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary));
        let primary_pressed = ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
        if primary_clicked {
            if let Some(pointer) = pointer_pos {
                if let Some((terminal_id, _)) = terminal_content_rects
                    .iter()
                    .rev()
                    .find(|(_, term_rect)| term_rect.contains(pointer))
                {
                    self.selected = Some(*terminal_id);
                    self.editing_text_node = None;
                    if self.suspend_terminal_focus == Some(*terminal_id) {
                        self.suspend_terminal_focus = None;
                    }
                }
            }
        }

        let resize_handle_hit = pointer_pos.and_then(|pointer| {
            let selected_id = self.selected?;
            let node = self.nodes.iter().find(|n| n.id == selected_id)?;
            if node.kind != NodeKind::Terminal {
                return None;
            }

            let node_rect =
                self.world_to_screen_rect(rect, Rect::from_min_size(node.pos, node.size));
            let handle_size = 18.0 * self.zoom.clamp(0.75, 1.6);
            let handle_rect = Rect::from_min_size(
                node_rect.right_bottom() - vec2(handle_size + 6.0, handle_size + 6.0),
                vec2(handle_size + 6.0, handle_size + 6.0),
            );
            if handle_rect.contains(pointer) {
                let local = self.screen_to_world_pos(rect, pointer);
                Some((selected_id, local, node.size))
            } else {
                None
            }
        });

        let is_panning =
            (is_space_pan || is_middle_pan) && pointer_in_canvas && !pointer_over_terminal_content;

        if is_panning {
            self.dragging = None;
            self.drag_start_pos = None;
            self.resizing = None;
            let delta = ctx.input(|i| i.pointer.delta());
            self.pan += delta;
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);
        }

        if self.resizing.is_none() && resize_handle_hit.is_some() {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeNwSe);
        }

        if !is_panning && self.editing_title_node.is_none() && primary_pressed {
            if let Some((id, local, size)) = resize_handle_hit {
                self.resizing = Some((id, local, size));
                self.dragging = None;
                self.drag_start_pos = None;
                self.selected = Some(id);
            } else if !pointer_over_terminal_content {
                if let Some(pointer) = pointer_pos {
                    let local = self.screen_to_world_pos(rect, pointer);
                    if let Some((id, node_pos, can_drag)) = self.find_node_hit(local) {
                        self.selected = Some(id);
                        if can_drag {
                            self.dragging = Some((id, local.to_vec2() - node_pos));
                            self.drag_start_pos = Some((id, node_pos.to_pos2()));
                        }
                    }
                }
            }
        }

        if let Some((resize_id, start_pointer, start_size)) = self.resizing {
            if ctx.input(|i| i.pointer.primary_down()) && !ctx.input(|i| i.key_down(egui::Key::Space)) {
                if let Some(pointer) = pointer_pos {
                    let local = self.screen_to_world_pos(rect, pointer);
                    if let Some(node) = self.nodes.iter_mut().find(|n| n.id == resize_id) {
                        let delta = local - start_pointer;
                        let width = (start_size.x + delta.x).max(320.0);
                        let height = (start_size.y + delta.y).max(170.0);
                        node.size = vec2(width, height);
                    }
                }
            } else {
                self.resizing = None;
            }
        }

        if let Some((drag_id, offset)) = self.dragging {
            if ctx.input(|i| i.pointer.primary_down()) && !ctx.input(|i| i.key_down(egui::Key::Space)) {
                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    let local = self.screen_to_world_pos(rect, pointer_pos);
                    if let Some(node) = self.nodes.iter_mut().find(|n| n.id == drag_id) {
                        node.pos = (local.to_vec2() - offset).to_pos2();
                    }
                }
            } else {
                if let Some((start_id, start_pos)) = self.drag_start_pos.take() {
                    if start_id == drag_id {
                        if let Some(node) = self.nodes.iter().find(|n| n.id == drag_id) {
                            self.record_move_history(drag_id, start_pos, node.pos);
                        }
                    }
                }
                self.dragging = None;
            }
        }

        if !is_panning && !pointer_over_terminal_content && secondary_pressed {
            self.right_drag_moved = false;
            self.cutting_path_local.clear();
            self.linking_from = None;
            self.linking_pointer_local = None;
            self.cut_snapshot_nodes = None;
            self.cut_snapshot_edges = None;

            if let Some(pointer_pos) = response.interact_pointer_pos() {
                let local = self.screen_to_world_pos(rect, pointer_pos);
                if let Some((id, _)) = self.find_node_at(local) {
                    self.linking_from = Some(id);
                    self.linking_pointer_local = Some(local);
                    self.selected = Some(id);
                } else {
                    self.cutting_path_local.push(local);
                    self.cut_snapshot_nodes = Some(self.nodes.clone());
                    self.cut_snapshot_edges = Some(self.edges.clone());
                }
            }
        }

        if secondary_down {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                let local = self.screen_to_world_pos(rect, pointer_pos);

                if self.linking_from.is_some() {
                    self.linking_pointer_local = Some(local);
                } else if let Some(prev) = self.cutting_path_local.last().copied() {
                    if prev.distance(local) > 0.8 {
                        self.right_drag_moved = true;
                        self.cut_edges_intersecting_segment(prev, local);
                        self.cut_nodes_intersecting_segment(prev, local);
                        self.cutting_path_local.push(local);
                    }
                }
            }
        }

        if secondary_released {
            if let Some(from) = self.linking_from {
                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    let local = self.screen_to_world_pos(rect, pointer_pos);
                    if let Some((to, _)) = self.find_node_at(local) {
                        if to != from && !self.has_edge(from, to) {
                            self.edges.push((from, to));
                        }
                    }
                }
                self.linking_from = None;
                self.linking_pointer_local = None;
            }

            if self.right_drag_moved {
                if let (Some(before_nodes), Some(before_edges)) =
                    (self.cut_snapshot_nodes.take(), self.cut_snapshot_edges.take())
                {
                    self.record_cut_history(before_nodes, before_edges);
                }
            } else {
                self.cut_snapshot_nodes = None;
                self.cut_snapshot_edges = None;
            }

            self.cutting_path_local.clear();
        }

        if !is_panning
            && !pointer_over_terminal_content
            && response.secondary_clicked()
            && self.linking_from.is_none()
            && !self.right_drag_moved
        {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                let local = self.screen_to_world_pos(rect, pointer_pos);
                self.context_menu_local_pos = Some(local);
                self.context_menu_node = self.find_node_at(local).map(|(id, _)| id);
                self.selected = self.context_menu_node;
                if self.context_menu_node.is_none() {
                    self.editing_text_node = None;
                    self.pending_text_focus = None;
                }
                self.menu_search_text.clear();
                self.menu_search_selected = 0;
                self.menu_nav_level = 0;
                self.menu_nav_selected = 0;
                self.pending_menu_search_focus = true;
            }
        }

        response.context_menu(|ui| {
            let search_id = egui::Id::new("context_menu_search_input");
            if self.pending_menu_search_focus {
                ui.memory_mut(|m| m.request_focus(search_id));
            }

            let search_resp = ui.add(
                TextEdit::singleline(&mut self.menu_search_text)
                    .id(search_id)
                    .hint_text("搜索并创建节点..."),
            );
            let search_has_focus = search_resp.has_focus() || ui.memory(|m| m.has_focus(search_id));
            if self.pending_menu_search_focus && search_has_focus {
                self.pending_menu_search_focus = false;
            }
            if search_resp.changed() {
                self.menu_search_selected = 0;
                if self.menu_search_text.trim().is_empty() {
                    self.menu_nav_level = 0;
                    self.menu_nav_selected = 0;
                }
            }

            ui.separator();

            let spawn_pos = if let Some(id) = self.context_menu_node {
                if let Some(node) = self.nodes.iter().find(|n| n.id == id) {
                    node.pos + vec2(node.size.x + 40.0, 10.0)
                } else {
                    self.context_menu_local_pos.unwrap_or(Pos2::new(100.0, 100.0))
                }
            } else {
                self.context_menu_local_pos.unwrap_or(Pos2::new(100.0, 100.0))
            };

            if self.menu_search_text.trim().is_empty() {
                let actions = [("终端节点", 0usize), ("文本节点", 1usize)];
                if self.menu_nav_selected >= actions.len() {
                    self.menu_nav_selected = actions.len().saturating_sub(1);
                }

                if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) && self.menu_nav_level >= 1 {
                    self.menu_nav_selected = (self.menu_nav_selected + 1) % actions.len();
                }
                if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) && self.menu_nav_level >= 1 {
                    self.menu_nav_selected =
                        (self.menu_nav_selected + actions.len() - 1) % actions.len();
                }
                if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                    self.menu_nav_level = (self.menu_nav_level + 1).min(1);
                }
                if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                    self.menu_nav_level = self.menu_nav_level.saturating_sub(1);
                }

                let mut trigger_action = None;
                if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                    match self.menu_nav_level {
                        0 => self.menu_nav_level = 1,
                        1 => trigger_action = Some(actions[self.menu_nav_selected].1),
                        _ => {}
                    }
                }

                ui.group(|ui| {
                    if ui
                        .add_sized(
                            [170.0, 24.0],
                            egui::SelectableLabel::new(self.menu_nav_level == 0, "创建节点 ▶"),
                        )
                        .clicked()
                    {
                        self.menu_nav_level = 1;
                    }

                    if self.menu_nav_level >= 1 {
                        ui.indent("menu_level_1", |ui| {
                            for (idx, (label, action_id)) in actions.iter().enumerate() {
                                let selected = self.menu_nav_selected == idx;
                                if ui
                                    .add_sized(
                                        [170.0, 24.0],
                                        egui::SelectableLabel::new(selected, *label),
                                    )
                                    .clicked()
                                {
                                    self.menu_nav_selected = idx;
                                    trigger_action = Some(*action_id);
                                }
                            }
                        });
                    }
                });

                if let Some(action_id) = trigger_action {
                    match action_id {
                        0 => self.create_terminal_node(spawn_pos),
                        1 => self.create_text_node(spawn_pos, true),
                        _ => {}
                    }
                    ui.close_menu();
                }

                ui.separator();
                ui.small("←/→ 进入或返回，↑/↓ 选择，Enter 创建");
                return;
            }

            let items = [
                ("创建节点/终端节点", "终端节点", 0usize),
                ("创建节点/文本节点", "文本节点", 1usize),
            ];

            let mut matched = Vec::new();
            for (path, label, action_id) in items {
                if self.menu_item_matches(path) || self.menu_item_matches(label) {
                    matched.push((label, action_id));
                }
            }

            if matched.is_empty() {
                ui.small("无匹配节点类型");
                return;
            }

            if self.menu_search_selected >= matched.len() {
                self.menu_search_selected = matched.len().saturating_sub(1);
            }

            if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                self.menu_search_selected = (self.menu_search_selected + 1) % matched.len();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                self.menu_search_selected =
                    (self.menu_search_selected + matched.len() - 1) % matched.len();
            }

            let mut trigger_action = None;
            for (row, (path, action_id)) in matched.iter().enumerate() {
                let selected = row == self.menu_search_selected;
                let resp = ui.selectable_label(selected, self.menu_item_highlighted_label(path));
                if resp.clicked() {
                    trigger_action = Some(*action_id);
                }
            }

            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                trigger_action = Some(matched[self.menu_search_selected].1);
            }

            if let Some(action_id) = trigger_action {
                match action_id {
                    0 => self.create_terminal_node(spawn_pos),
                    1 => self.create_text_node(spawn_pos, true),
                    _ => {}
                }
                ui.close_menu();
            }

            ui.separator();
            ui.small("↑/↓ 选择，Enter 创建");
        });

        if !any_popup_open && !is_panning && !pointer_over_terminal_content && response.double_clicked()
        {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                let local = self.screen_to_world_pos(rect, pointer_pos);
                if let Some((id, _)) = self.find_node_at(local) {
                    self.selected = Some(id);
                    if let Some(node) = self.nodes.iter().find(|n| n.id == id) {
                        if node.kind == NodeKind::Text {
                            self.editing_text_node = Some(id);
                            self.pending_text_focus = Some(id);
                        } else if node.kind == NodeKind::Terminal
                            && local.y <= node.pos.y + TERMINAL_HEADER_HEIGHT
                        {
                            self.start_title_edit(id);
                        }
                    }
                } else {
                    self.create_text_node((local.to_vec2() - vec2(120.0, 60.0)).to_pos2(), true);
                }
            }
        }

        if !any_popup_open && !is_panning && !pointer_over_terminal_content && response.clicked() {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                let local = self.screen_to_world_pos(rect, pointer_pos);
                if let Some((id, _)) = self.find_node_at(local) {
                    self.selected = Some(id);
                    if self.editing_text_node != Some(id) {
                        self.editing_text_node = None;
                    }
                } else {
                    self.selected = None;
                    self.editing_text_node = None;
                }
            }
        }

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
                let left =
                    end - dir * (12.0 * self.zoom) + vec2(-dir.y, dir.x) * (6.0 * self.zoom);
                let right =
                    end - dir * (12.0 * self.zoom) + vec2(dir.y, -dir.x) * (6.0 * self.zoom);
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

        if self.cutting_path_local.len() >= 2 {
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
            };

            painter.rect(node_rect, 8.0 * zoom_scale, fill, stroke, egui::StrokeKind::Outside);

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
            }
        }

        if let Some((id, edit_rect)) = text_edit_rect {
            if let Some(node) = self.nodes.iter_mut().find(|n| n.id == id) {
                let text_edit_id = egui::Id::new(("text-node-editor", id));
                let should_focus_and_select_all = self.pending_text_focus == Some(id);
                if should_focus_and_select_all {
                    ctx.memory_mut(|m| m.request_focus(text_edit_id));
                }

                let desired_rows = node.text_body.split('\n').count().max(1);
                let text_edit = TextEdit::multiline(&mut node.text_body)
                    .id(text_edit_id)
                    .font(FontId::proportional(15.0 * self.zoom))
                    .text_color(Color32::from_rgb(250, 240, 210))
                    .margin(egui::Margin::ZERO)
                    .desired_width(f32::INFINITY)
                    .desired_rows(desired_rows)
                    .frame(false);
                let resp = ui.put(edit_rect, text_edit);

                if should_focus_and_select_all {
                    if let Some(mut state) = egui::TextEdit::load_state(ctx, text_edit_id) {
                        let len = node.text_body.chars().count();
                        let range = egui::text::CCursorRange::two(
                            egui::text::CCursor::new(0),
                            egui::text::CCursor::new(len),
                        );
                        state.cursor.set_char_range(Some(range));
                        state.store(ctx, text_edit_id);
                    }
                    self.pending_text_focus = None;
                }

                if resp.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.editing_text_node = None;
                }
            }
        }

        if let Some((id, edit_rect)) = title_edit_rect {
            let title_edit_id = egui::Id::new(("terminal-title-editor", id));
            let should_focus_and_select_all = self.pending_title_focus == Some(id);
            if should_focus_and_select_all {
                ctx.memory_mut(|m| m.request_focus(title_edit_id));
            }

            let text_edit = TextEdit::singleline(&mut self.title_edit_buffer)
                .id(title_edit_id)
                .font(FontId::proportional((16.0 * self.zoom).max(9.0)))
                .text_color(Color32::WHITE)
                .desired_width(f32::INFINITY)
                .frame(false);
            let resp = ui.put(edit_rect, text_edit);

            if should_focus_and_select_all {
                if let Some(mut state) = egui::TextEdit::load_state(ctx, title_edit_id) {
                    let len = self.title_edit_buffer.chars().count();
                    let range = egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(len),
                    );
                    state.cursor.set_char_range(Some(range));
                    state.store(ctx, title_edit_id);
                }
                self.pending_title_focus = None;
            }

            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.cancel_title_edit();
            } else if ctx.input(|i| i.key_pressed(egui::Key::Enter))
                || (resp.lost_focus() && !ctx.input(|i| i.pointer.primary_down()))
            {
                self.commit_title_edit(id);
            } else if primary_clicked {
                if let Some(pointer) = pointer_pos {
                    if !edit_rect.contains(pointer) {
                        self.commit_title_edit(id);
                    }
                }
            }
        }

        for (node_id, term_rect) in terminal_content_rects {
            let visible_rect = term_rect.intersect(rect);
            if !visible_rect.is_positive() {
                continue;
            }

            if !self.terminal_backends.contains_key(&node_id)
                && !self.terminal_errors.contains_key(&node_id)
                && !self.terminal_exited.contains(&node_id)
            {
                self.ensure_terminal(node_id, ctx);
            }

            egui::Area::new(egui::Id::new(("terminal_node_embedded_area", node_id)))
                .order(egui::Order::Foreground)
                .constrain(false)
                .fixed_pos(term_rect.min)
                .show(ctx, |ui| {
                    let full_screen_rect = Rect::from_min_size(term_rect.min, term_rect.size());
                    let mut term_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(full_screen_rect)
                            .layout(*ui.layout()),
                    );
                    term_ui.set_clip_rect(visible_rect);

                    if let Some(err) = self.terminal_errors.get(&node_id) {
                        term_ui.colored_label(Color32::LIGHT_RED, err);
                    } else if let Some(backend) = self.terminal_backends.get_mut(&node_id) {
                        let term_font_size = (14.0 * self.zoom).min(36.0);
                        let term_font = TerminalFont::new(egui_term::FontSettings {
                            font_type: FontId::monospace(term_font_size),
                        });
                        let term = TerminalView::new(&mut term_ui, backend)
                            .set_focus(
                                self.selected == Some(node_id)
                                    && self.editing_title_node != Some(node_id)
                                    && self.suspend_terminal_focus != Some(node_id),
                            )
                            .set_font(term_font)
                            .set_size(term_rect.size());
                        term_ui.add(term);
                    } else {
                        term_ui.label("终端未启动，请在右侧点击“重启终端”。");
                    }
                });
        }

        if !is_panning && self.resizing.is_none() && resize_handle_hit.is_none() {
            if let Some(pos) = response.hover_pos() {
                let local = self.screen_to_world_pos(rect, pos);
                if is_space_down && response.hovered() {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grab);
                } else if self.find_node_at(local).is_some() {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                }
            }
        }
    }
}

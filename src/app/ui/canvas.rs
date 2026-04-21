use super::super::GraphApp;
use crate::constants::TERMINAL_HEADER_HEIGHT;
use crate::model::NodeKind;
use arboard::Clipboard;
use eframe::egui::{self, vec2, Color32, Pos2, Rect, Sense, Ui};
use std::collections::HashSet;
use std::path::Path;

impl GraphApp {
    fn is_dropped_image_file(file: &egui::DroppedFile) -> bool {
        if !file.mime.is_empty() && file.mime.starts_with("image/") {
            return true;
        }

        if let Some(path) = &file.path {
            return Self::is_supported_image_path(path);
        }

        Path::new(&file.name)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| {
                matches!(
                    ext.to_ascii_lowercase().as_str(),
                    "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
                )
            })
            .unwrap_or(false)
    }

    fn parse_pasted_paths(text: &str) -> Vec<String> {
        text.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|line| line.trim_matches('"'))
            .map(|line| line.strip_prefix("file://").unwrap_or(line))
            .map(ToOwned::to_owned)
            .collect()
    }

    fn handle_canvas_image_import(
        &mut self,
        ctx: &egui::Context,
        rect: Rect,
        pointer_pos: Option<Pos2>,
        pointer_in_canvas: bool,
    ) {
        let now = ctx.input(|i| i.time);
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            self.pending_dropped_files = dropped_files;
            self.pending_drop_spawn_world_pos = None;
            self.pending_drop_requested_at = Some(now);
        }

        if !self.pending_dropped_files.is_empty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            ctx.request_repaint();

            let drop_pointer_world_pos = if pointer_in_canvas {
                pointer_pos.map(|pointer| self.screen_to_world_pos(rect, pointer))
            } else {
                None
            };

            if let Some(world_pos) = drop_pointer_world_pos {
                self.pending_drop_spawn_world_pos = Some(world_pos);
            }

            if self.pending_drop_spawn_world_pos.is_none()
                && self
                    .pending_drop_requested_at
                    .is_some_and(|start| now - start > 0.8)
            {
                self.pending_dropped_files.clear();
                self.pending_drop_requested_at = None;
            }
        }

        let mut spawn_local = if !self.pending_dropped_files.is_empty() {
            self.pending_drop_spawn_world_pos
        } else if pointer_in_canvas {
            pointer_pos
                .map(|pointer| self.screen_to_world_pos(rect, pointer))
                .or(self.last_canvas_pointer_world_pos)
        } else {
            self.last_canvas_pointer_world_pos
        };

        if let Some(mut drop_spawn_local) = self.pending_drop_spawn_world_pos {
            let pending_files = std::mem::take(&mut self.pending_dropped_files);
            for file in pending_files {
                if !Self::is_dropped_image_file(&file) {
                    continue;
                }

                let spawn_pos = drop_spawn_local;
                if let Some(path) = file.path {
                    self.create_image_node_from_path(spawn_pos, path.to_string_lossy().to_string());
                } else if let Some(bytes) = file.bytes {
                    let display_name = if file.name.trim().is_empty() {
                        "粘贴图片".to_owned()
                    } else {
                        file.name
                    };
                    self.create_image_node_from_bytes(spawn_pos, display_name, bytes.to_vec());
                }
                self.advance_spawn_pos_below_selected(&mut drop_spawn_local);
            }
            spawn_local = Some(drop_spawn_local);
            self.last_drag_hover_world_pos = None;
            self.pending_drop_spawn_world_pos = None;
            self.pending_drop_requested_at = None;
        }

        let Some(mut spawn_local) = spawn_local else {
            return;
        };

        let (key_v_pressed, ctrl_down, command_down, paste_event_count, raw_paste_event_count) =
            ctx.input(|i| {
                let paste_events = i
                    .events
                    .iter()
                    .filter(|event| matches!(event, egui::Event::Paste(_)))
                    .count();

                let raw_paste_events = i
                    .raw
                    .events
                    .iter()
                    .filter(|event| matches!(event, egui::Event::Paste(_)))
                    .count();

                (
                    i.key_pressed(egui::Key::V),
                    i.modifiers.ctrl,
                    i.modifiers.command,
                    paste_events,
                    raw_paste_events,
                )
            });

        let paste_shortcut = key_v_pressed && (command_down || ctrl_down);
        let paste_requested = paste_shortcut || paste_event_count > 0 || raw_paste_event_count > 0;

        if paste_requested && pointer_in_canvas {
            if let Ok(mut clipboard) = Clipboard::new() {
                if let Ok(image) = clipboard.get_image() {
                    let spawn_pos = spawn_local;
                    let size = [image.width, image.height];
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &image.bytes);
                    self.create_image_node_from_color_image(
                        spawn_pos,
                        "粘贴图片".to_owned(),
                        color_image,
                        ctx,
                    );
                    return;
                }

                if let Ok(files) = clipboard.get().file_list() {
                    let mut created_from_files = 0usize;
                    for file in files {
                        if Self::is_supported_image_path(&file) {
                            let spawn_pos = spawn_local;
                            self.create_image_node_from_path(
                                spawn_pos,
                                file.to_string_lossy().to_string(),
                            );
                            self.advance_spawn_pos_below_selected(&mut spawn_local);
                            created_from_files += 1;
                        }
                    }

                    if created_from_files > 0 {
                        return;
                    }
                }

                if let Ok(text) = clipboard.get_text() {
                    for candidate in Self::parse_pasted_paths(&text) {
                        let path = Path::new(&candidate);
                        if path.exists() && Self::is_supported_image_path(path) {
                            let spawn_pos = spawn_local;
                            self.create_image_node_from_path(spawn_pos, candidate);
                            self.advance_spawn_pos_below_selected(&mut spawn_local);
                        }
                    }
                }
            }
        }

        if !pointer_in_canvas {
            return;
        }

        let pasted_texts: Vec<String> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Paste(text) => Some(text.clone()),
                    _ => None,
                })
                .collect()
        });

        for pasted in pasted_texts {
            for candidate in Self::parse_pasted_paths(&pasted) {
                let path = Path::new(&candidate);
                if path.exists() && Self::is_supported_image_path(path) {
                    let spawn_pos = spawn_local;
                    self.create_image_node_from_path(spawn_pos, candidate);
                    self.advance_spawn_pos_below_selected(&mut spawn_local);
                }
            }
        }
    }

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
        let multi_select_modifier = ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
        let subtract_select_modifier = ctx.input(|i| i.modifiers.shift);
        let focus_shortcut_pressed = ctx.input(|i| {
            i.key_pressed(egui::Key::F)
                && !i.modifiers.ctrl
                && !i.modifiers.command
                && !i.modifiers.alt
                && !i.modifiers.shift
        });
        let dragging_files = ctx.input(|i| !i.raw.hovered_files.is_empty());

        if dragging_files {
            ctx.request_repaint();
        } else {
            self.last_drag_hover_world_pos = None;
        }

        if pointer_in_canvas {
            if let Some(pointer) = pointer_pos {
                let world_pos = self.screen_to_world_pos(rect, pointer);
                self.last_canvas_pointer_world_pos = Some(world_pos);
                if dragging_files {
                    self.last_drag_hover_world_pos = Some(world_pos);
                }
            }
        }

        self.handle_canvas_image_import(ctx, rect, pointer_pos, pointer_in_canvas);

        if focus_shortcut_pressed
            && !any_popup_open
            && self.editing_text_node.is_none()
            && self.editing_title_node.is_none()
            && self.editing_identity_node.is_none()
        {
            self.focus_selected_or_all(rect);
        }

        let pointer_over_terminal_before_zoom = pointer_pos.is_some_and(|p| {
            let local = self.screen_to_world_pos(rect, p);
            let Some((node_id, _)) = self.find_node_at(local) else {
                return false;
            };
            self.nodes
                .iter()
                .find(|n| n.id == node_id)
                .is_some_and(|n| {
                    n.kind == NodeKind::Terminal
                        && local.y > n.pos.y + TERMINAL_HEADER_HEIGHT
                        && !Self::terminal_identity_badge_world_rect(n).contains(local)
                })
        });

        if pointer_in_canvas && !pointer_over_terminal_before_zoom {
            let zoom_change = ctx.input(|i| {
                let pinch = i.zoom_delta();
                let wheel = (i.raw_scroll_delta.y * 0.0015).exp();
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
            let local = self.screen_to_world_pos(rect, p);
            let Some((node_id, _)) = self.find_node_at(local) else {
                return false;
            };
            self.nodes
                .iter()
                .find(|n| n.id == node_id)
                .is_some_and(|n| {
                    n.kind == NodeKind::Terminal
                        && local.y > n.pos.y + TERMINAL_HEADER_HEIGHT
                        && !Self::terminal_identity_badge_world_rect(n).contains(local)
                })
        });

        let current_time = ctx.input(|i| i.time);
        let primary_clicked = ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary));
        let primary_pressed = ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
        let is_panning =
            (is_space_pan || is_middle_pan) && pointer_in_canvas && !pointer_over_terminal_content;
        let mut tolerant_double_click = false;
        if primary_clicked {
            if let Some(pointer) = pointer_pos {
                let local = self.screen_to_world_pos(rect, pointer);
                if let Some(node_id) = self.find_terminal_identity_badge_at(local) {
                    self.set_single_selection(node_id);
                    self.editing_text_node = None;
                } else if let Some((node_id, _)) = self.find_node_at(local) {
                    if let Some(node) = self.nodes.iter().find(|n| n.id == node_id) {
                        if node.kind == NodeKind::Terminal
                            && local.y > node.pos.y + TERMINAL_HEADER_HEIGHT
                            && !Self::terminal_identity_badge_world_rect(node).contains(local)
                        {
                            self.set_single_selection(node_id);
                            self.editing_text_node = None;
                            if self.suspend_terminal_focus == Some(node_id) {
                                self.suspend_terminal_focus = None;
                            }
                        }
                    }
                }

                if !any_popup_open && !is_panning && !pointer_over_terminal_content {
                    if let (Some(last_time), Some(last_pos)) = (
                        self.last_primary_click_time,
                        self.last_primary_click_pos,
                    ) {
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
            if !matches!(node.kind, NodeKind::Terminal | NodeKind::Image) {
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

        if is_panning {
            self.dragging = None;
            self.drag_start_pos = None;
            self.drag_group_start = None;
            self.resizing = None;
            self.box_select_start = None;
            self.box_select_current = None;
            self.box_select_additive = false;
            self.box_select_subtractive = false;
            self.box_select_base_selection.clear();
            let delta = ctx.input(|i| i.pointer.delta());
            self.pan += delta;
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);
        }

        if self.resizing.is_none() && resize_handle_hit.is_some() {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeNwSe);
        }

        if !is_panning
            && self.editing_title_node.is_none()
            && self.editing_identity_node.is_none()
            && primary_pressed
        {
            if let Some((id, local, size)) = resize_handle_hit {
                self.resizing = Some((id, local, size));
                self.dragging = None;
                self.drag_start_pos = None;
                self.drag_group_start = None;
                self.set_single_selection(id);
            } else if !pointer_over_terminal_content {
                if let Some(pointer) = pointer_pos {
                    let local = self.screen_to_world_pos(rect, pointer);
                    if let Some(id) = self.find_terminal_identity_badge_at(local) {
                        self.set_single_selection(id);
                        self.dragging = None;
                        self.drag_start_pos = None;
                        self.drag_group_start = None;
                        self.box_select_start = None;
                        self.box_select_current = None;
                    } else if let Some((id, node_pos, can_drag)) = self.find_node_hit(local) {
                        if subtract_select_modifier {
                            self.remove_from_selection(id);
                            self.dragging = None;
                            self.drag_start_pos = None;
                            self.drag_group_start = None;
                        } else if multi_select_modifier {
                            self.toggle_selection(id);
                            self.dragging = None;
                            self.drag_start_pos = None;
                            self.drag_group_start = None;
                        } else {
                            let multi_drag = self.selected_nodes.len() > 1 && self.selected_nodes.contains(&id);
                            if multi_drag {
                                self.selected = Some(id);
                            } else {
                                self.set_single_selection(id);
                            }

                            if can_drag {
                                self.dragging = Some((id, local.to_vec2() - node_pos));
                                if multi_drag {
                                    let start_nodes = self
                                        .nodes
                                        .iter()
                                        .filter(|n| self.selected_nodes.contains(&n.id))
                                        .map(|n| (n.id, n.pos))
                                        .collect();
                                    self.drag_group_start = Some((local, start_nodes));
                                    self.drag_start_pos = None;
                                } else {
                                    self.drag_group_start = None;
                                    self.drag_start_pos = Some((id, node_pos.to_pos2()));
                                }
                            }
                        }
                        self.box_select_start = None;
                        self.box_select_current = None;
                    } else {
                        self.dragging = None;
                        self.drag_start_pos = None;
                        self.drag_group_start = None;
                        self.box_select_start = Some(local);
                        self.box_select_current = Some(local);
                        self.box_select_additive = multi_select_modifier;
                        self.box_select_subtractive = subtract_select_modifier;
                        self.box_select_base_selection = self.selected_nodes.clone();
                    }
                }
            }
        }

        if let Some((resize_id, start_pointer, start_size)) = self.resizing {
            if ctx.input(|i| i.pointer.primary_down()) && !ctx.input(|i| i.key_down(egui::Key::Space)) {
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
                            }
                        }
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

                    let mut next_selection = if self.box_select_additive || self.box_select_subtractive {
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

            if let Some(pointer_pos) = response.interact_pointer_pos() {
                let local = self.screen_to_world_pos(rect, pointer_pos);
                if let Some((id, _)) = self.find_node_at(local) {
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
                self.set_selection_from_option(self.context_menu_node);
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

        self.show_canvas_context_menu(&response, ctx);

        if !any_popup_open
            && !is_panning
            && !pointer_over_terminal_content
            && (response.double_clicked() || tolerant_double_click)
        {
            if let Some(pointer) = pointer_pos.or_else(|| response.interact_pointer_pos()) {
                let local = self.screen_to_world_pos(rect, pointer);
                if let Some(id) = self.find_terminal_identity_badge_at(local) {
                    self.start_identity_edit(id);
                } else if let Some((id, _)) = self.find_node_at(local) {
                    self.set_single_selection(id);
                    if let Some(node) = self.nodes.iter().find(|n| n.id == id) {
                        if node.kind == NodeKind::Text {
                            self.editing_text_node = Some(id);
                            self.pending_text_focus = Some(id);
                        } else if node.kind == NodeKind::Terminal {
                            if local.y <= node.pos.y + TERMINAL_HEADER_HEIGHT {
                                self.start_title_edit(id);
                            }
                        }
                    }
                } else {
                    self.create_text_node(local, true);
                }
                self.last_primary_click_time = None;
                self.last_primary_click_pos = None;
            }
        }

        if !any_popup_open
            && !is_panning
            && !pointer_over_terminal_content
            && response.clicked()
            && !multi_select_modifier
        {
            if let Some(pointer) = pointer_pos.or_else(|| response.interact_pointer_pos()) {
                let local = self.screen_to_world_pos(rect, pointer);
                if let Some(id) = self.find_terminal_identity_badge_at(local) {
                    self.set_single_selection(id);
                    self.editing_text_node = None;
                } else if let Some((id, _)) = self.find_node_at(local) {
                    self.set_single_selection(id);
                    if self.editing_text_node != Some(id) {
                        self.editing_text_node = None;
                    }
                } else {
                    self.clear_selection();
                    self.editing_text_node = None;
                }
            }
        }

        if let (Some(start), Some(current)) = (self.box_select_start, self.box_select_current) {
            let box_rect_world = Rect::from_two_pos(start, current);
            let box_rect_screen = self.world_to_screen_rect(rect, box_rect_world);
            painter.rect_filled(
                box_rect_screen,
                0.0,
                Color32::from_rgba_unmultiplied(120, 170, 255, 28),
            );
            painter.rect_stroke(
                box_rect_screen,
                0.0,
                egui::Stroke::new(1.0, Color32::from_rgb(120, 170, 255)),
                egui::StrokeKind::Outside,
            );
        }

        self.draw_edges(&painter, rect);
        self.draw_link_preview(&painter, rect);
        self.draw_cut_path(&painter, rect);

        self.autosize_text_nodes(&painter);
        self.ensure_image_textures(ctx);
        self.draw_embedded_terminals(ui, ctx, rect, &terminal_content_rects);

        let (text_edit_rect, title_edit_rect, identity_edit_rect) = self.draw_nodes(&painter, rect);
        self.handle_text_node_editor(ui, ctx, text_edit_rect);
        self.handle_title_editor(ui, ctx, title_edit_rect, primary_clicked, pointer_pos);
        self.handle_identity_editor(ui, ctx, identity_edit_rect, primary_clicked, pointer_pos);

        if !is_panning && self.resizing.is_none() && resize_handle_hit.is_none() {
            if let Some(pos) = response.hover_pos() {
                let local = self.screen_to_world_pos(rect, pos);
                if is_space_down && response.hovered() {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grab);
                } else if self.find_terminal_identity_badge_at(local).is_some()
                    || self.find_node_at(local).is_some()
                {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                }
            }
        }
    }
}

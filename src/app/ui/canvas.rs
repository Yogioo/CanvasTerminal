use super::super::GraphApp;
use crate::constants::TERMINAL_HEADER_HEIGHT;
use crate::model::NodeKind;
use arboard::Clipboard;
use eframe::egui::{self, vec2, Color32, Pos2, Rect, Sense, Ui};
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
        let fallback_pointer = rect.center();
        let pointer = pointer_pos.unwrap_or(fallback_pointer);
        let mut spawn_local = self.screen_to_world_pos(rect, pointer);

        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if pointer_in_canvas {
            for file in dropped_files {
                if !Self::is_dropped_image_file(&file) {
                    eprintln!("[image-import] ignore dropped non-image: name='{}' mime='{}'", file.name, file.mime);
                    continue;
                }

                let spawn_pos = (spawn_local.to_vec2() - vec2(160.0, 110.0)).to_pos2();
                if let Some(path) = file.path {
                    eprintln!("[image-import] dropped image path: {}", path.to_string_lossy());
                    self.create_image_node_from_path(spawn_pos, path.to_string_lossy().to_string());
                } else if let Some(bytes) = file.bytes {
                    let display_name = if file.name.trim().is_empty() {
                        "粘贴图片".to_owned()
                    } else {
                        file.name
                    };
                    eprintln!("[image-import] dropped image bytes: name='{}' bytes={}", display_name, bytes.len());
                    self.create_image_node_from_bytes(spawn_pos, display_name, bytes.to_vec());
                } else {
                    eprintln!("[image-import] dropped image has neither path nor bytes");
                }
                spawn_local.y += 26.0;
            }
        }

        let (
            key_v_pressed,
            key_v_down,
            key_f6_pressed,
            ctrl_down,
            command_down,
            paste_event_count,
            raw_paste_event_count,
            raw_ctrl_v_event_count,
            raw_v_event_count,
            ctrl_v_text_event_count,
            raw_ctrl_v_text_event_count,
        ) = ctx.input(|i| {
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

            let raw_ctrl_v_events = i
                .raw
                .events
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        egui::Event::Key {
                            key: egui::Key::V,
                            pressed: true,
                            modifiers,
                            ..
                        } if modifiers.ctrl || modifiers.command
                    )
                })
                .count();

            let raw_v_events = i
                .raw
                .events
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        egui::Event::Key {
                            key: egui::Key::V,
                            ..
                        }
                    )
                })
                .count();

            let ctrl_v_text_events = i
                .events
                .iter()
                .filter(|event| matches!(event, egui::Event::Text(text) if text.contains('\u{16}')))
                .count();

            let raw_ctrl_v_text_events = i
                .raw
                .events
                .iter()
                .filter(|event| matches!(event, egui::Event::Text(text) if text.contains('\u{16}')))
                .count();

            (
                i.key_pressed(egui::Key::V),
                i.key_down(egui::Key::V),
                i.key_pressed(egui::Key::F6),
                i.modifiers.ctrl,
                i.modifiers.command,
                paste_events,
                raw_paste_events,
                raw_ctrl_v_events,
                raw_v_events,
                ctrl_v_text_events,
                raw_ctrl_v_text_events,
            )
        });

        if key_v_pressed
            || (key_v_down && (ctrl_down || command_down))
            || paste_event_count > 0
            || raw_paste_event_count > 0
            || raw_ctrl_v_event_count > 0
            || raw_v_event_count > 0
            || ctrl_v_text_event_count > 0
            || raw_ctrl_v_text_event_count > 0
            || key_f6_pressed
        {
            eprintln!(
                "[image-paste] key_v_pressed={} key_v_down={} key_f6_pressed={} ctrl_down={} command_down={} paste_events={} raw_paste_events={} raw_ctrl_v_events={} raw_v_events={} ctrl_v_text_events={} raw_ctrl_v_text_events={} pointer_in_canvas={} pointer={:?}",
                key_v_pressed,
                key_v_down,
                key_f6_pressed,
                ctrl_down,
                command_down,
                paste_event_count,
                raw_paste_event_count,
                raw_ctrl_v_event_count,
                raw_v_event_count,
                ctrl_v_text_event_count,
                raw_ctrl_v_text_event_count,
                pointer_in_canvas,
                pointer_pos
            );
        }

        let paste_shortcut = key_v_pressed && (command_down || ctrl_down);
        let manual_import_key =
            key_v_pressed && !command_down && !ctrl_down && pointer_in_canvas && !ctx.wants_keyboard_input();
        if manual_import_key {
            eprintln!("[image-paste] manual import key accepted (V)");
        }

        let paste_requested = key_f6_pressed
            || paste_shortcut
            || manual_import_key
            || paste_event_count > 0
            || raw_paste_event_count > 0
            || raw_ctrl_v_event_count > 0
            || ctrl_v_text_event_count > 0
            || raw_ctrl_v_text_event_count > 0;

        if paste_requested && !pointer_in_canvas {
            eprintln!("[image-paste] paste requested but pointer not in canvas");
        }

        if paste_requested && pointer_in_canvas {
            match Clipboard::new() {
                Ok(mut clipboard) => {
                    match clipboard.get_image() {
                        Ok(image) => {
                            eprintln!(
                                "[image-paste] clipboard image found: {}x{}, bytes={}",
                                image.width,
                                image.height,
                                image.bytes.len()
                            );
                            let spawn_pos = (spawn_local.to_vec2() - vec2(160.0, 110.0)).to_pos2();
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
                        Err(err) => {
                            eprintln!("[image-paste] clipboard image unavailable: {err}");
                        }
                    }

                    let mut created_from_files = 0usize;
                    match clipboard.get().file_list() {
                        Ok(files) => {
                            eprintln!("[image-paste] clipboard file_list count={}", files.len());
                            for file in files {
                                let supported = Self::is_supported_image_path(&file);
                                eprintln!(
                                    "[image-paste] file_list path='{}' supported_image={}",
                                    file.to_string_lossy(),
                                    supported
                                );
                                if supported {
                                    let spawn_pos = (spawn_local.to_vec2() - vec2(160.0, 110.0)).to_pos2();
                                    self.create_image_node_from_path(
                                        spawn_pos,
                                        file.to_string_lossy().to_string(),
                                    );
                                    spawn_local.y += 26.0;
                                    created_from_files += 1;
                                }
                            }
                        }
                        Err(err) => {
                            eprintln!("[image-paste] clipboard file_list unavailable: {err}");
                        }
                    }

                    if created_from_files == 0 {
                        match clipboard.get_text() {
                            Ok(text) => {
                                eprintln!("[image-paste] clipboard text len={}", text.len());
                                let mut created = 0usize;
                                for candidate in Self::parse_pasted_paths(&text) {
                                    let path = Path::new(&candidate);
                                    let exists = path.exists();
                                    let supported = Self::is_supported_image_path(path);
                                    eprintln!(
                                        "[image-paste] candidate='{}' exists={} supported_image={}",
                                        candidate, exists, supported
                                    );
                                    if exists && supported {
                                        let spawn_pos = (spawn_local.to_vec2() - vec2(160.0, 110.0)).to_pos2();
                                        self.create_image_node_from_path(spawn_pos, candidate);
                                        spawn_local.y += 26.0;
                                        created += 1;
                                    }
                                }
                                if created == 0 {
                                    eprintln!("[image-paste] no valid image path found in clipboard text");
                                }
                            }
                            Err(err) => {
                                eprintln!("[image-paste] clipboard text unavailable: {err}");
                            }
                        }
                    }
                }
                Err(err) => {
                    eprintln!("[image-paste] failed to open clipboard: {err}");
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

        if !pasted_texts.is_empty() {
            eprintln!("[image-paste] egui paste events: {}", pasted_texts.len());
        }

        for pasted in pasted_texts {
            eprintln!("[image-paste] egui pasted text len={}", pasted.len());
            let mut created = 0usize;
            for candidate in Self::parse_pasted_paths(&pasted) {
                let path = Path::new(&candidate);
                let exists = path.exists();
                let supported = Self::is_supported_image_path(path);
                eprintln!(
                    "[image-paste] egui candidate='{}' exists={} supported_image={}",
                    candidate, exists, supported
                );
                if exists && supported {
                    let spawn_pos = (spawn_local.to_vec2() - vec2(160.0, 110.0)).to_pos2();
                    self.create_image_node_from_path(spawn_pos, candidate);
                    spawn_local.y += 26.0;
                    created += 1;
                }
            }
            if created == 0 {
                eprintln!("[image-paste] egui paste text produced no image node");
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

        self.handle_canvas_image_import(ctx, rect, pointer_pos, pointer_in_canvas);

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

        self.show_canvas_context_menu(&response, ctx);

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

        self.draw_edges(&painter, rect);
        self.draw_link_preview(&painter, rect);
        self.draw_cut_path(&painter, rect);

        self.autosize_text_nodes(&painter);
        self.ensure_image_textures(ctx);
        let (text_edit_rect, title_edit_rect) = self.draw_nodes(&painter, rect);
        self.handle_text_node_editor(ui, ctx, text_edit_rect);
        self.handle_title_editor(ui, ctx, title_edit_rect, primary_clicked, pointer_pos);

        self.draw_embedded_terminals(ctx, rect, &terminal_content_rects);

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

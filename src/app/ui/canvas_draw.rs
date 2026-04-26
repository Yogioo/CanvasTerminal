use super::super::{EdgeControlHandle, GraphApp, NodeOrderAction};
use crate::constants::{DECISION_HEADER_HEIGHT, GROUP_HEADER_HEIGHT, TERMINAL_HEADER_HEIGHT};
use crate::model::NodeKind;
use eframe::egui::{self, Color32, Rect, Sense, Ui};

impl GraphApp {
    pub(in crate::app) fn draw_canvas(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let available = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(available, Sense::click_and_drag());
        let painter = ui.painter_at(rect);

        painter.rect_filled(rect, 0.0, Color32::from_rgb(30, 30, 50));
        self.ensure_camera_initialized(rect);
        self.maybe_rebase_world(rect);

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
        let queue_editor_open = self.editing_decision_queue_node.is_some();
        let any_popup_open = ctx.memory(|m| m.any_popup_open())
            || self.decision_color_popup.is_some()
            || queue_editor_open;
        let multi_select_modifier = ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
        let subtract_select_modifier = ctx.input(|i| i.modifiers.shift);
        let alt_passthrough = ctx.input(|i| i.modifiers.alt);
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

        let mut just_focused = false;
        if focus_shortcut_pressed
            && !any_popup_open
            && self.editing_text_node.is_none()
            && self.editing_title_node.is_none()
            && self.editing_startup_node.is_none()
            && self.editing_working_directory_node.is_none()
        {
            self.focus_selected_or_all(rect);
            just_focused = true;
        }

        let pointer_over_terminal_before_zoom = pointer_pos.is_some_and(|p| {
            let local = self.screen_to_world_pos(rect, p);
            let Some((node_id, _)) = self.find_node_at_with_alt(local, alt_passthrough) else {
                return false;
            };
            self.nodes
                .iter()
                .find(|n| n.id == node_id)
                .is_some_and(|n| {
                    let terminal_content_visible = self.zoom >= self.terminal_hide_zoom_threshold
                        || self.editing_startup_node == Some(node_id)
                        || self.editing_working_directory_node == Some(node_id);
                    n.kind == NodeKind::Terminal
                        && terminal_content_visible
                        && local.y > n.pos.y + TERMINAL_HEADER_HEIGHT
                })
        });
        let pointer_over_text_node_before_zoom = pointer_pos.is_some_and(|p| {
            let local = self.screen_to_world_pos(rect, p);
            let Some((node_id, _)) = self.find_node_at_with_alt(local, alt_passthrough) else {
                return false;
            };
            self.nodes
                .iter()
                .find(|n| n.id == node_id)
                .is_some_and(|n| n.kind == NodeKind::Text)
        });
        let pointer_over_decision_node_before_zoom = pointer_pos.is_some_and(|p| {
            let local = self.screen_to_world_pos(rect, p);
            let Some((node_id, _)) = self.find_node_at_with_alt(local, alt_passthrough) else {
                return false;
            };
            self.nodes
                .iter()
                .find(|n| n.id == node_id)
                .is_some_and(|n| n.kind == NodeKind::Decision)
        });

        if pointer_in_canvas
            && !pointer_over_terminal_before_zoom
            && !pointer_over_text_node_before_zoom
            && !pointer_over_decision_node_before_zoom
            && !just_focused
        {
            let zoom_change = ctx.input(|i| {
                let pinch = i.zoom_delta();
                let wheel = (i.raw_scroll_delta.y * 0.0015).exp();
                pinch * wheel
            });
            if (zoom_change - 1.0).abs() > f32::EPSILON {
                if let Some(pointer) = pointer_pos {
                    let old_zoom = self.zoom;
                    let new_zoom = (old_zoom * zoom_change).max(1e-4);
                    if (new_zoom - old_zoom).abs() > f32::EPSILON {
                        let world_at_pointer = self.screen_to_world_pos(rect, pointer);
                        self.zoom = new_zoom;
                        self.camera_world_center = (world_at_pointer.to_vec2()
                            - (pointer - rect.center()) / self.zoom)
                            .to_pos2();
                        self.sync_pan_from_camera(rect);
                    }
                }
            }
        }

        self.sync_all_group_bounds();
        self.paint_grid(&painter, rect, self.pan, self.zoom);

        let pointer_over_terminal_content = pointer_pos.is_some_and(|p| {
            let local = self.screen_to_world_pos(rect, p);
            let Some((node_id, _)) = self.find_node_at_with_alt(local, alt_passthrough) else {
                return false;
            };
            self.nodes
                .iter()
                .find(|n| n.id == node_id)
                .is_some_and(|n| {
                    let terminal_content_visible = self.zoom >= self.terminal_hide_zoom_threshold
                        || self.editing_startup_node == Some(node_id)
                        || self.editing_working_directory_node == Some(node_id);
                    n.kind == NodeKind::Terminal
                        && terminal_content_visible
                        && local.y > n.pos.y + TERMINAL_HEADER_HEIGHT
                })
        });

        let current_time = ctx.input(|i| i.time);
        let primary_clicked = ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary));
        let primary_pressed = ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
        let pointer_in_window_top_strip = pointer_pos.is_some_and(|p| p.y <= rect.top() + 32.0);
        let is_panning = (is_space_pan || is_middle_pan)
            && pointer_in_canvas
            && !pointer_over_terminal_content
            && !any_popup_open;
        let edge_hit_tolerance = (10.0 / self.zoom.max(1e-4)).max(4.0);

        if self
            .selected_edge
            .is_some_and(|(from, to)| !self.has_edge(from, to))
        {
            self.clear_edge_selection();
        }

        let edge_handle_hit = pointer_pos.and_then(|pointer| {
            let edge = self.selected_edge?;
            let radius = (8.0 * self.zoom.clamp(0.75, 1.8)).max(6.0);

            let mut best: Option<(EdgeControlHandle, f32)> = None;
            for handle in [EdgeControlHandle::Source, EdgeControlHandle::Target] {
                let Some(handle_world) =
                    self.edge_control_handle_world_pos_local(edge.0, edge.1, handle)
                else {
                    continue;
                };
                let handle_screen = self.world_to_screen_pos(rect, handle_world);
                let distance = pointer.distance(handle_screen);
                if distance <= radius + 3.0
                    && best.is_none_or(|(_, current_best)| distance < current_best)
                {
                    best = Some((handle, distance));
                }
            }

            best.map(|(handle, _)| (edge, handle))
        });

        let hovered_edge = pointer_pos.and_then(|pointer| {
            if !rect.contains(pointer) {
                return None;
            }

            let local = self.screen_to_world_pos(rect, pointer);
            if self
                .find_node_at_with_alt(local, alt_passthrough)
                .is_some()
            {
                return None;
            }

            self.find_edge_at(local, edge_hit_tolerance)
        });

        let keyboard_has_focus = ctx.wants_keyboard_input();
        let can_run_layer_shortcuts = pointer_in_canvas
            && !is_panning
            && !pointer_over_terminal_content
            && !any_popup_open
            && self.editing_text_node.is_none()
            && self.editing_title_node.is_none()
            && self.editing_startup_node.is_none()
            && self.editing_working_directory_node.is_none()
            && !self.selected_nodes.is_empty()
            && !keyboard_has_focus;

        if can_run_layer_shortcuts {
            let layer_action = ctx.input(|i| {
                let ctrl_or_cmd = i.modifiers.ctrl || i.modifiers.command;
                let no_extra_modifiers = !i.modifiers.alt && !i.modifiers.shift;

                if i.key_pressed(egui::Key::CloseBracket) && no_extra_modifiers {
                    if ctrl_or_cmd {
                        Some(NodeOrderAction::BringToFront)
                    } else {
                        Some(NodeOrderAction::BringForwardOne)
                    }
                } else if i.key_pressed(egui::Key::OpenBracket) && no_extra_modifiers {
                    if ctrl_or_cmd {
                        Some(NodeOrderAction::SendToBack)
                    } else {
                        Some(NodeOrderAction::SendBackwardOne)
                    }
                } else {
                    None
                }
            });

            if let Some(action) = layer_action {
                match action {
                    NodeOrderAction::BringToFront => self.bring_selection_to_front(),
                    NodeOrderAction::BringForwardOne => self.bring_selection_forward_one(),
                    NodeOrderAction::SendBackwardOne => self.send_selection_backward_one(),
                    NodeOrderAction::SendToBack => self.send_selection_to_back(),
                }
            }
        }

        if pointer_in_canvas
            && !any_popup_open
            && !is_panning
            && !pointer_over_terminal_content
            && self.editing_text_node.is_none()
            && self.editing_title_node.is_none()
            && self.editing_startup_node.is_none()
            && self.editing_working_directory_node.is_none()
            && self.editing_edge.is_none()
            && !ctx.wants_keyboard_input()
            && self.selected_edge.is_some()
            && ctx.input(|i| {
                i.modifiers.shift
                    && !i.modifiers.ctrl
                    && !i.modifiers.command
                    && !i.modifiers.alt
                    && i.key_pressed(egui::Key::R)
            })
        {
            self.reset_selected_edge_curve_bias();
        }

        let (tolerant_double_click, resize_handle_hit) = self.handle_canvas_pointer_interactions(
            ui,
            ctx,
            &response,
            rect,
            pointer_pos,
            any_popup_open,
            is_panning,
            pointer_over_terminal_content,
            pointer_in_window_top_strip,
            primary_clicked,
            primary_pressed,
            multi_select_modifier,
            subtract_select_modifier,
            current_time,
            edge_hit_tolerance,
            edge_handle_hit,
            secondary_pressed,
            secondary_down,
            secondary_released,
        );

        self.show_canvas_context_menu(&response, ctx);

        if !any_popup_open
            && !is_panning
            && !pointer_over_terminal_content
            && !pointer_in_window_top_strip
            && !alt_passthrough
            && self.editing_startup_node.is_none()
            && self.editing_working_directory_node.is_none()
            && (response.double_clicked() || tolerant_double_click)
        {
            if let Some(pointer) = pointer_pos.or_else(|| response.interact_pointer_pos()) {
                let local = self.screen_to_world_pos(rect, pointer);
                if let Some((id, _)) = self.find_node_at_with_alt(local, alt_passthrough) {
                    self.set_single_selection(id);
                    if let Some(node) = self.nodes.iter().find(|n| n.id == id) {
                        if node.kind == NodeKind::Text {
                            self.editing_text_node = Some(id);
                            self.pending_text_focus = Some(id);
                        } else if node.kind == NodeKind::Terminal {
                            if local.y <= node.pos.y + TERMINAL_HEADER_HEIGHT {
                                self.start_title_edit(id);
                            }
                        } else if node.kind == NodeKind::Decision {
                            if local.y <= node.pos.y + DECISION_HEADER_HEIGHT {
                                self.start_title_edit(id);
                            } else {
                                self.start_decision_buttons_edit(id);
                            }
                        } else if node.kind == NodeKind::Group {
                            if local.y <= node.pos.y + GROUP_HEADER_HEIGHT {
                                self.start_title_edit(id);
                            }
                        }
                    }
                } else {
                    if let Some(edge) = self.find_edge_at(local, edge_hit_tolerance) {
                        self.start_edge_edit(edge);
                    } else {
                        self.create_text_node(local, true);
                    }
                }
                self.last_primary_click_time = None;
                self.last_primary_click_pos = None;
            }
        }

        if !any_popup_open
            && !is_panning
            && !pointer_over_terminal_content
            && !alt_passthrough
            && self.editing_startup_node.is_none()
            && self.editing_working_directory_node.is_none()
            && response.clicked()
            && !multi_select_modifier
        {
            if let Some(pointer) = pointer_pos.or_else(|| response.interact_pointer_pos()) {
                let local = self.screen_to_world_pos(rect, pointer);
                if let Some((id, _)) = self.find_node_at_with_alt(local, alt_passthrough) {
                    self.set_single_selection(id);
                    if self.editing_text_node != Some(id) {
                        self.editing_text_node = None;
                    }
                } else if let Some(edge) = self.find_edge_at(local, edge_hit_tolerance) {
                    self.set_edge_selection(edge);
                    self.editing_text_node = None;
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

        self.draw_edges(&painter, rect, hovered_edge);
        self.draw_link_preview(&painter, rect);
        self.draw_cut_path(&painter, rect);

        if self.zoom < self.text_hide_zoom_threshold && self.editing_text_node.is_some() {
            self.editing_text_node = None;
            self.pending_text_focus = None;
        }

        self.ensure_image_textures(ctx);

        let (
            text_edit_rect,
            title_edit_rect,
            startup_edit_rect,
            decision_edit_rect,
            working_directory_edit_rect,
        ) = self.draw_nodes(ui, ctx, &painter, rect);
        self.draw_selected_edge_controls_overlay(&painter, rect);
        self.handle_text_node_editor(ui, ctx, text_edit_rect);
        self.handle_title_editor(ui, ctx, title_edit_rect, primary_clicked, pointer_pos);
        self.handle_startup_editor(ui, ctx, startup_edit_rect, primary_clicked, pointer_pos);
        self.handle_working_directory_editor(
            ui,
            ctx,
            working_directory_edit_rect,
            primary_clicked,
            pointer_pos,
        );
        self.handle_edge_editor(ui, ctx, rect, primary_clicked, pointer_pos);
        self.handle_decision_buttons_editor(
            ui,
            ctx,
            decision_edit_rect,
            primary_clicked,
            pointer_pos,
        );
        self.handle_decision_queue_editor(ctx);

        if alt_passthrough && !self.selected_nodes.is_empty() {
            if let Some(pointer) = pointer_pos {
                let local = self.screen_to_world_pos(rect, pointer);
                let hint = if self.top_group_id_at(local).is_some() {
                    "Alt+点击: 跳转并进入目标组"
                } else {
                    "Alt+点击: 跳转到鼠标位置"
                };

                painter.text(
                    pointer + egui::vec2(14.0, 10.0),
                    egui::Align2::LEFT_TOP,
                    hint,
                    egui::FontId::proportional(12.0),
                    Color32::from_rgb(220, 232, 255),
                );
            }
        }

        if !is_panning && self.resizing.is_none() && resize_handle_hit.is_none() {
            if let Some(pos) = response.hover_pos() {
                let local = self.screen_to_world_pos(rect, pos);
                if is_space_down && response.hovered() {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grab);
                } else if edge_handle_hit.is_some() {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                } else if self
                    .find_node_at_with_alt(local, alt_passthrough)
                    .is_some()
                    || hovered_edge.is_some()
                {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                }
            }
        }
    }
}

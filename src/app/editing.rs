use super::{DecisionButtonDraft, DecisionColorInputMode, GraphApp};
use crate::model::{DecisionButton, NodeData};
use eframe::egui;
use std::collections::HashSet;

impl GraphApp {
    pub(in crate::app) fn decision_color_text_from_rgb(
        mode: DecisionColorInputMode,
        rgb: [u8; 3],
    ) -> String {
        match mode {
            DecisionColorInputMode::Rgb => format!("{},{},{}", rgb[0], rgb[1], rgb[2]),
            DecisionColorInputMode::Hsv => {
                let r = rgb[0] as f32 / 255.0;
                let g = rgb[1] as f32 / 255.0;
                let b = rgb[2] as f32 / 255.0;
                let max = r.max(g.max(b));
                let min = r.min(g.min(b));
                let delta = max - min;

                let mut h = if delta <= f32::EPSILON {
                    0.0
                } else if (max - r).abs() <= f32::EPSILON {
                    60.0 * ((g - b) / delta).rem_euclid(6.0)
                } else if (max - g).abs() <= f32::EPSILON {
                    60.0 * (((b - r) / delta) + 2.0)
                } else {
                    60.0 * (((r - g) / delta) + 4.0)
                };
                if h < 0.0 {
                    h += 360.0;
                }
                let s = if max <= f32::EPSILON {
                    0.0
                } else {
                    delta / max
                };
                let v = max;
                format!(
                    "{:.0},{:.0},{:.0}",
                    h.round(),
                    (s * 100.0).round(),
                    (v * 100.0).round()
                )
            }
        }
    }

    pub(in crate::app) fn parse_decision_color_text(
        mode: DecisionColorInputMode,
        text: &str,
    ) -> Option<[u8; 3]> {
        let parts: Vec<&str> = text.split(',').map(str::trim).collect();
        if parts.len() != 3 {
            return None;
        }

        match mode {
            DecisionColorInputMode::Rgb => {
                let r = parts[0].parse::<u8>().ok()?;
                let g = parts[1].parse::<u8>().ok()?;
                let b = parts[2].parse::<u8>().ok()?;
                Some([r, g, b])
            }
            DecisionColorInputMode::Hsv => {
                let h = parts[0].parse::<f32>().ok()?.clamp(0.0, 360.0);
                let s = (parts[1].parse::<f32>().ok()?.clamp(0.0, 100.0)) / 100.0;
                let v = (parts[2].parse::<f32>().ok()?.clamp(0.0, 100.0)) / 100.0;

                let c = v * s;
                let x = c * (1.0 - (((h / 60.0) % 2.0) - 1.0).abs());
                let m = v - c;

                let (r1, g1, b1) = if h < 60.0 {
                    (c, x, 0.0)
                } else if h < 120.0 {
                    (x, c, 0.0)
                } else if h < 180.0 {
                    (0.0, c, x)
                } else if h < 240.0 {
                    (0.0, x, c)
                } else if h < 300.0 {
                    (x, 0.0, c)
                } else {
                    (c, 0.0, x)
                };

                Some([
                    ((r1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
                    ((g1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
                    ((b1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
                ])
            }
        }
    }

    pub(in crate::app) fn sync_decision_color_texts_with_mode(&mut self) {
        for row in &mut self.ws.decision_buttons_edit_rows {
            row.color_text =
                Self::decision_color_text_from_rgb(self.ws.decision_color_input_mode, row.color_rgb);
        }
    }

    fn finish_title_edit(&mut self, node_id: Option<usize>) {
        self.ws.editing_title_node = None;
        self.ws.pending_title_focus = None;
        self.ws.title_edit_buffer.clear();
        self.ws.suspend_terminal_focus = node_id;
    }

    fn finish_startup_edit(&mut self, node_id: Option<usize>) {
        self.ws.editing_startup_node = None;
        self.ws.pending_startup_focus = None;
        self.ws.startup_edit_buffer.clear();
        self.ws.suspend_terminal_focus = node_id;
    }

    fn finish_working_directory_edit(&mut self, node_id: Option<usize>) {
        self.ws.editing_working_directory_node = None;
        self.ws.pending_working_directory_focus = None;
        self.ws.working_directory_edit_buffer.clear();
        self.ws.suspend_terminal_focus = node_id;
    }

    fn restart_terminal_if_changed(&mut self, node_id: usize, changed: bool, ctx: &egui::Context) {
        if changed {
            self.restart_terminal(node_id, ctx);
        }
    }

    pub(in crate::app) fn prepare_inline_node_edit(&mut self, node_id: usize) {
        self.set_single_selection(node_id);
        self.ws.dragging = None;
        self.ws.drag_start_pos = None;
        self.ws.drag_group_start = None;
        self.ws.resizing = None;

        self.ws.editing_text_node = None;
        self.ws.pending_text_focus = None;

        self.ws.editing_title_node = None;
        self.ws.pending_title_focus = None;
        self.ws.title_edit_buffer.clear();

        self.ws.editing_startup_node = None;
        self.ws.pending_startup_focus = None;
        self.ws.startup_edit_buffer.clear();

        self.ws.editing_working_directory_node = None;
        self.ws.pending_working_directory_focus = None;
        self.ws.working_directory_edit_buffer.clear();

        self.ws.editing_decision_buttons_node = None;
        self.ws.pending_decision_buttons_focus = None;
        self.ws.decision_buttons_edit_rows.clear();
        self.ws.decision_color_popup = None;
        self.ws.decision_color_popup_pos = None;
        self.ws.decision_buttons_edit_error = None;

        self.ws.editing_decision_queue_node = None;
        self.ws.pending_decision_queue_focus = None;
        self.ws.decision_queue_edit_buffer.clear();

        self.ws.editing_edge = None;
        self.ws.pending_edge_focus = None;
        self.ws.edge_edit_buffer.clear();
    }

    pub(in crate::app) fn start_title_edit(&mut self, node_id: usize) {
        let Some(title) = self.ws
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .and_then(|n| match &n.data {
                NodeData::Terminal { title, .. } => Some(title.clone()),
                NodeData::Decision { title, .. } => Some(title.clone()),
                NodeData::Group { title, .. } => Some(title.clone()),
                NodeData::Script { title, .. } => Some(title.clone()),
                _ => None,
            })
        else {
            return;
        };

        self.prepare_inline_node_edit(node_id);
        self.ws.editing_title_node = Some(node_id);
        self.ws.pending_title_focus = Some(node_id);
        self.ws.title_edit_buffer = title;
    }

    pub(in crate::app) fn commit_title_edit(&mut self, node_id: usize) {
        let mut changed = false;
        if let Some(node) = self.ws.nodes.iter_mut().find(|n| n.id == node_id) {
            let trimmed = self.ws.title_edit_buffer.trim();
            if !trimmed.is_empty() {
                match &mut node.data {
                    NodeData::Terminal { title, .. }
                    | NodeData::Decision { title, .. }
                    | NodeData::Group { title, .. }
                    | NodeData::Script { title, .. } => {
                        if title != trimmed {
                            *title = trimmed.to_owned();
                            changed = true;
                        }
                    }
                    _ => {}
                }
            }
        }
        if changed {
            self.mark_workspace_dirty();
        }
        self.finish_title_edit(Some(node_id));
    }

    pub(in crate::app) fn cancel_title_edit(&mut self) {
        self.finish_title_edit(self.ws.editing_title_node);
    }

    pub(in crate::app) fn start_startup_edit(&mut self, node_id: usize) {
        let Some(startup_script) =
            self.ws.nodes
                .iter()
                .find(|n| n.id == node_id)
                .and_then(|n| match &n.data {
                    NodeData::Terminal { startup_script, .. } => Some(startup_script.clone()),
                    _ => None,
                })
        else {
            return;
        };

        self.prepare_inline_node_edit(node_id);
        self.ws.editing_startup_node = Some(node_id);
        self.ws.pending_startup_focus = Some(node_id);
        self.ws.startup_edit_buffer = startup_script;
    }

    pub(in crate::app) fn commit_startup_edit(&mut self, node_id: usize, ctx: &egui::Context) {
        let mut changed = false;
        if let Some(node) = self.ws.nodes.iter_mut().find(|n| n.id == node_id) {
            let next = self.ws.startup_edit_buffer.trim().to_owned();
            if let NodeData::Terminal { startup_script, .. } = &mut node.data {
                if *startup_script != next {
                    *startup_script = next;
                    changed = true;
                }
            }
        }
        if changed {
            self.mark_workspace_dirty();
        }
        self.finish_startup_edit(Some(node_id));
        self.restart_terminal_if_changed(node_id, changed, ctx);
    }

    pub(in crate::app) fn start_working_directory_edit(&mut self, node_id: usize) {
        let Some(working_directory) = self.ws
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .and_then(|n| match &n.data {
                NodeData::Terminal {
                    working_directory, ..
                } => Some(working_directory.clone().unwrap_or_default()),
                _ => None,
            })
        else {
            return;
        };

        self.prepare_inline_node_edit(node_id);
        self.ws.editing_working_directory_node = Some(node_id);
        self.ws.pending_working_directory_focus = Some(node_id);
        self.ws.working_directory_edit_buffer = working_directory;
    }

    pub(in crate::app) fn commit_working_directory_edit(
        &mut self,
        node_id: usize,
        ctx: &egui::Context,
    ) {
        let mut changed = false;
        if let Some(node) = self.ws.nodes.iter_mut().find(|n| n.id == node_id) {
            let next = self.ws.working_directory_edit_buffer.trim();
            let next_value = if next.is_empty() {
                None
            } else {
                Some(next.to_owned())
            };
            if let NodeData::Terminal {
                working_directory, ..
            } = &mut node.data
            {
                if *working_directory != next_value {
                    *working_directory = next_value;
                    changed = true;
                }
            }
        }
        if changed {
            self.mark_workspace_dirty();
        }
        self.finish_working_directory_edit(Some(node_id));
        self.restart_terminal_if_changed(node_id, changed, ctx);
    }

    pub(in crate::app) fn cancel_working_directory_edit(&mut self) {
        self.finish_working_directory_edit(self.ws.editing_working_directory_node);
    }

    pub(in crate::app) fn start_decision_buttons_edit(&mut self, node_id: usize) {
        let Some(existing_buttons) =
            self.ws.nodes
                .iter()
                .find(|n| n.id == node_id)
                .and_then(|n| match &n.data {
                    NodeData::Decision { buttons, .. } => Some(buttons.clone()),
                    _ => None,
                })
        else {
            return;
        };

        self.prepare_inline_node_edit(node_id);
        self.ws.editing_decision_buttons_node = Some(node_id);
        self.ws.pending_decision_buttons_focus = Some(node_id);
        self.ws.decision_color_popup = None;
        self.ws.decision_color_popup_pos = None;
        self.ws.decision_buttons_edit_error = None;

        self.ws.decision_buttons_edit_rows = existing_buttons
            .into_iter()
            .map(|button| {
                let color_rgb = button.color_rgb.unwrap_or([224, 232, 242]);
                DecisionButtonDraft {
                    label: button.label,
                    event_key: button.event_key,
                    color_rgb,
                    color_text: Self::decision_color_text_from_rgb(
                        self.ws.decision_color_input_mode,
                        color_rgb,
                    ),
                }
            })
            .collect();

        if self.ws.decision_buttons_edit_rows.is_empty() {
            let default_rgb = [212, 244, 226];
            self.ws.decision_buttons_edit_rows.push(DecisionButtonDraft {
                label: "通过".to_owned(),
                event_key: "approve".to_owned(),
                color_rgb: default_rgb,
                color_text: Self::decision_color_text_from_rgb(
                    self.ws.decision_color_input_mode,
                    default_rgb,
                ),
            });
        }
    }

    pub(in crate::app) fn add_decision_button_row(&mut self) {
        let default_rgb = [224, 232, 242];
        self.ws.decision_buttons_edit_rows.push(DecisionButtonDraft {
            label: "新按钮".to_owned(),
            event_key: format!("event_{}", self.ws.decision_buttons_edit_rows.len() + 1),
            color_rgb: default_rgb,
            color_text: Self::decision_color_text_from_rgb(
                self.ws.decision_color_input_mode,
                default_rgb,
            ),
        });
    }


    pub(in crate::app) fn remove_decision_button_row(&mut self, row: usize) {
        if row < self.ws.decision_buttons_edit_rows.len() {
            self.ws.decision_buttons_edit_rows.remove(row);
            if let Some((node_id, popup_row)) = self.ws.decision_color_popup {
                if popup_row == row {
                    self.ws.decision_color_popup = None;
                    self.ws.decision_color_popup_pos = None;
                } else if popup_row > row {
                    self.ws.decision_color_popup = Some((node_id, popup_row - 1));
                }
            }
        }
    }

    pub(in crate::app) fn cancel_decision_buttons_edit(&mut self) {
        self.ws.editing_decision_buttons_node = None;
        self.ws.pending_decision_buttons_focus = None;
        self.ws.decision_buttons_edit_rows.clear();
        self.ws.decision_color_popup = None;
        self.ws.decision_color_popup_pos = None;
        self.ws.decision_buttons_edit_error = None;
    }

    pub(in crate::app) fn start_decision_queue_edit(&mut self, node_id: usize) {
        let Some(queue_text) =
            self.ws.nodes
                .iter()
                .find(|n| n.id == node_id)
                .and_then(|n| match &n.data {
                    NodeData::Decision {
                        pending_message,
                        pending_messages,
                        ..
                    } => {
                        let mut queue = pending_messages.clone();
                        if queue.is_empty() {
                            if let Some(single) = pending_message.as_deref().map(str::trim) {
                                if !single.is_empty() {
                                    queue.push(single.to_owned());
                                }
                            }
                        }
                        Some(queue.join("\n\n-----\n\n"))
                    }
                    _ => None,
                })
        else {
            return;
        };

        self.prepare_inline_node_edit(node_id);
        self.ws.editing_decision_queue_node = Some(node_id);
        self.ws.pending_decision_queue_focus = Some(node_id);
        self.ws.decision_queue_edit_buffer = queue_text;
    }

    pub(in crate::app) fn cancel_decision_queue_edit(&mut self) {
        self.ws.editing_decision_queue_node = None;
        self.ws.pending_decision_queue_focus = None;
        self.ws.decision_queue_edit_buffer.clear();
    }

    pub(in crate::app) fn commit_decision_queue_edit(&mut self, node_id: usize) {
        let mut changed = false;
        if let Some(node) = self.ws.nodes.iter_mut().find(|n| n.id == node_id) {
            if let NodeData::Decision {
                pending_message,
                pending_messages,
                ..
            } = &mut node.data
            {
                let next_queue: Vec<String> = self.ws
                    .decision_queue_edit_buffer
                    .split("\n\n-----\n\n")
                    .map(str::trim)
                    .filter(|msg| !msg.is_empty())
                    .map(ToOwned::to_owned)
                    .collect();

                let mut current_effective = pending_messages.clone();
                if current_effective.is_empty() {
                    if let Some(single) = pending_message.as_deref().map(str::trim) {
                        if !single.is_empty() {
                            current_effective.push(single.to_owned());
                        }
                    }
                }

                if current_effective != next_queue {
                    *pending_messages = next_queue;
                    *pending_message = pending_messages.first().cloned();
                    changed = true;
                }
            }
        }

        if changed {
            self.mark_workspace_dirty();
        }

        self.cancel_decision_queue_edit();
    }

    // ── Script node queue editor ──

    pub(in crate::app) fn start_script_queue_edit(&mut self, node_id: usize) {
        let queue_text = self.ws
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .and_then(|n| match &n.data {
                NodeData::Script { pending_messages, .. } => {
                    Some(pending_messages.join("\n\n-----\n\n"))
                }
                _ => None,
            })
            .unwrap_or_default();

        self.ws.editing_script_queue_node = Some(node_id);
        self.ws.pending_script_queue_focus = Some(node_id);
        self.ws.script_queue_edit_buffer = queue_text;
    }

    pub(in crate::app) fn cancel_script_queue_edit(&mut self) {
        self.ws.editing_script_queue_node = None;
        self.ws.pending_script_queue_focus = None;
        self.ws.script_queue_edit_buffer.clear();
    }

    pub(in crate::app) fn commit_script_queue_edit(&mut self, node_id: usize) {
        let mut changed = false;
        if let Some(node) = self.ws.nodes.iter_mut().find(|n| n.id == node_id) {
            if let NodeData::Script { pending_messages, .. } = &mut node.data {
                let next_queue: Vec<String> = self.ws
                    .script_queue_edit_buffer
                    .split("\n\n-----\n\n")
                    .map(str::trim)
                    .filter(|msg| !msg.is_empty())
                    .map(ToOwned::to_owned)
                    .collect();

                if *pending_messages != next_queue {
                    *pending_messages = next_queue;
                    changed = true;
                }
            }
        }

        if changed {
            self.mark_workspace_dirty();
        }

        self.cancel_script_queue_edit();
    }

    pub(in crate::app) fn commit_decision_buttons_edit(&mut self) -> bool {
        let Some(node_id) = self.ws.editing_decision_buttons_node else {
            return false;
        };

        let mut parsed_buttons = Vec::new();
        let mut seen_event_keys = HashSet::new();

        for (idx, row) in self.ws.decision_buttons_edit_rows.iter().enumerate() {
            let label = row.label.trim();
            let event_key = row.event_key.trim();

            if label.is_empty() {
                self.ws.decision_buttons_edit_error =
                    Some(format!("第 {} 行显示名称不能为空", idx + 1));
                return false;
            }
            if event_key.is_empty() {
                self.ws.decision_buttons_edit_error = Some(format!("第 {} 行事件名不能为空", idx + 1));
                return false;
            }
            if !seen_event_keys.insert(event_key.to_owned()) {
                self.ws.decision_buttons_edit_error =
                    Some(format!("第 {} 行事件名重复: {}", idx + 1, event_key));
                return false;
            }

            parsed_buttons.push(DecisionButton {
                label: label.to_owned(),
                event_key: event_key.to_owned(),
                color_rgb: Some(row.color_rgb),
            });
        }

        let mut changed = false;
        if let Some(node) = self.ws.nodes.iter_mut().find(|n| n.id == node_id) {
            if let NodeData::Decision { buttons, .. } = &mut node.data {
                if *buttons != parsed_buttons {
                    *buttons = parsed_buttons;
                    changed = true;
                }
            }
        }

        if changed {
            self.mark_workspace_dirty();
        }

        self.cancel_decision_buttons_edit();
        true
    }

    // ── Script node editing ──

    pub(in crate::app) fn start_script_edit(&mut self, node_id: usize) {
        let Some(code) = self.ws
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .and_then(|n| match &n.data {
                NodeData::Script { code, .. } => Some(code.clone()),
                _ => None,
            })
        else {
            return;
        };

        // If there's already a script node being edited, commit it first
        if let Some(prev_id) = self.ws.editing_script_node {
            if prev_id != node_id {
                self.commit_script_edit(prev_id);
            }
        }

        self.prepare_inline_node_edit(node_id);
        self.ws.editing_script_node = Some(node_id);
        self.ws.pending_script_focus = Some(node_id);
        self.ws.script_edit_buffer = code;
    }

    pub(in crate::app) fn apply_script_snippet(&mut self, node_id: usize, code: String, start_edit: bool) {
        let mut changed = false;
        if let Some(node) = self.ws.nodes.iter_mut().find(|n| n.id == node_id) {
            if let NodeData::Script { code: node_code, .. } = &mut node.data {
                if *node_code != code {
                    *node_code = code.clone();
                    self.ws.script_lua_runtimes.remove(&node_id);
                    self.ws.script_lua_timer_accum.remove(&node_id);
                    self.ws.script_lua_errors.remove(&node_id);
                    changed = true;
                }
            }
        }

        if changed {
            self.mark_workspace_dirty();
        }

        if start_edit {
            self.start_script_edit(node_id);
        }
    }

    pub(in crate::app) fn commit_script_edit(&mut self, node_id: usize) {
        self.save_script_edit_buffer(node_id);
        self.ws.editing_script_node = None;
        self.ws.pending_script_focus = None;
        self.ws.script_edit_buffer.clear();
    }

    pub(in crate::app) fn save_script_edit_buffer(&mut self, node_id: usize) {
        let mut changed = false;
        if let Some(node) = self.ws.nodes.iter_mut().find(|n| n.id == node_id) {
            if let NodeData::Script { code, .. } = &mut node.data {
                let new_code = self.ws.script_edit_buffer.trim().to_owned();
                if *code != new_code {
                    *code = new_code;
                    // Lua runtime must be rebuilt on code change
                    self.ws.script_lua_runtimes.remove(&node_id);
                    self.ws.script_lua_timer_accum.remove(&node_id);
                    self.ws.script_lua_errors.remove(&node_id);
                    changed = true;
                }
            }
        }

        if changed {
            self.mark_workspace_dirty();
        }
    }

    pub(in crate::app) fn start_script_debug(&mut self, node_id: usize) {
        if self.ws.editing_script_node != Some(node_id) {
            self.start_script_edit(node_id);
        }
        self.ws.script_debug_node = Some(node_id);
    }

    pub(in crate::app) fn stop_script_debug(&mut self, node_id: usize) {
        if self.ws.script_debug_node == Some(node_id) {
            self.ws.script_debug_node = None;
        }
    }

    #[allow(dead_code)]
    pub(in crate::app) fn cancel_script_edit(&mut self) {
        if let Some(node_id) = self.ws.editing_script_node {
            if self.ws.script_debug_node == Some(node_id) {
                self.ws.script_debug_node = None;
            }
        }
        self.ws.editing_script_node = None;
        self.ws.pending_script_focus = None;
        self.ws.script_edit_buffer.clear();
    }

    pub(in crate::app) fn start_edge_edit(&mut self, edge: (usize, usize)) {
        if !self.has_edge(edge.0, edge.1) {
            return;
        }

        self.ws.editing_text_node = None;
        self.ws.pending_text_focus = None;
        self.ws.editing_title_node = None;
        self.ws.pending_title_focus = None;
        self.ws.title_edit_buffer.clear();
        self.ws.editing_startup_node = None;
        self.ws.pending_startup_focus = None;
        self.ws.startup_edit_buffer.clear();
        self.ws.editing_working_directory_node = None;
        self.ws.pending_working_directory_focus = None;
        self.ws.working_directory_edit_buffer.clear();
        self.ws.editing_decision_buttons_node = None;
        self.ws.pending_decision_buttons_focus = None;
        self.ws.decision_buttons_edit_rows.clear();
        self.ws.decision_color_popup = None;
        self.ws.decision_color_popup_pos = None;
        self.ws.decision_buttons_edit_error = None;
        self.ws.editing_decision_queue_node = None;
        self.ws.pending_decision_queue_focus = None;
        self.ws.decision_queue_edit_buffer.clear();

        self.set_edge_selection(edge);
        self.ws.dragging = None;
        self.ws.drag_start_pos = None;
        self.ws.drag_group_start = None;
        self.ws.resizing = None;

        self.ws.editing_edge = Some(edge);
        self.ws.pending_edge_focus = Some(edge);
        self.ws.edge_edit_buffer = self
            .edge_route_key(edge.0, edge.1)
            .unwrap_or_default()
            .to_owned();
    }

    pub(in crate::app) fn commit_edge_edit(&mut self) {
        let Some((from, to)) = self.ws.editing_edge else {
            return;
        };

        if !self.has_edge(from, to) {
            self.cancel_edge_edit();
            return;
        }

        let prev = self.edge_route_key(from, to).unwrap_or_default().to_owned();
        let next = self.ws.edge_edit_buffer.trim().to_owned();

        if prev != next {
            if next.is_empty() {
                self.remove_edge_route_key(from, to);
            } else {
                self.set_edge_route_key(from, to, next);
            }
            self.mark_workspace_dirty();
        }

        self.cancel_edge_edit();
    }

    pub(in crate::app) fn cancel_edge_edit(&mut self) {
        self.ws.editing_edge = None;
        self.ws.pending_edge_focus = None;
        self.ws.edge_edit_buffer.clear();
    }


}

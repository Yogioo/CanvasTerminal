use super::{DecisionButtonDraft, DecisionColorInputMode, GraphApp};
use crate::model::{DecisionButton, NodeData};
use crate::script_node;
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
        for row in &mut self.decision_buttons_edit_rows {
            row.color_text =
                Self::decision_color_text_from_rgb(self.decision_color_input_mode, row.color_rgb);
        }
    }

    fn finish_title_edit(&mut self, node_id: Option<usize>) {
        self.editing_title_node = None;
        self.pending_title_focus = None;
        self.title_edit_buffer.clear();
        self.suspend_terminal_focus = node_id;
    }

    fn finish_startup_edit(&mut self, node_id: Option<usize>) {
        self.editing_startup_node = None;
        self.pending_startup_focus = None;
        self.startup_edit_buffer.clear();
        self.suspend_terminal_focus = node_id;
    }

    fn finish_working_directory_edit(&mut self, node_id: Option<usize>) {
        self.editing_working_directory_node = None;
        self.pending_working_directory_focus = None;
        self.working_directory_edit_buffer.clear();
        self.suspend_terminal_focus = node_id;
    }

    fn restart_terminal_if_changed(&mut self, node_id: usize, changed: bool, ctx: &egui::Context) {
        if changed {
            self.restart_terminal(node_id, ctx);
        }
    }

    pub(in crate::app) fn prepare_inline_node_edit(&mut self, node_id: usize) {
        self.set_single_selection(node_id);
        self.dragging = None;
        self.drag_start_pos = None;
        self.drag_group_start = None;
        self.resizing = None;

        self.editing_text_node = None;
        self.pending_text_focus = None;

        self.editing_title_node = None;
        self.pending_title_focus = None;
        self.title_edit_buffer.clear();

        self.editing_startup_node = None;
        self.pending_startup_focus = None;
        self.startup_edit_buffer.clear();

        self.editing_working_directory_node = None;
        self.pending_working_directory_focus = None;
        self.working_directory_edit_buffer.clear();

        self.editing_decision_buttons_node = None;
        self.pending_decision_buttons_focus = None;
        self.decision_buttons_edit_rows.clear();
        self.decision_color_popup = None;
        self.decision_color_popup_pos = None;
        self.decision_buttons_edit_error = None;

        self.editing_decision_queue_node = None;
        self.pending_decision_queue_focus = None;
        self.decision_queue_edit_buffer.clear();

        self.editing_edge = None;
        self.pending_edge_focus = None;
        self.edge_edit_buffer.clear();
    }

    pub(in crate::app) fn start_title_edit(&mut self, node_id: usize) {
        let Some(title) = self
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
        self.editing_title_node = Some(node_id);
        self.pending_title_focus = Some(node_id);
        self.title_edit_buffer = title;
    }

    pub(in crate::app) fn commit_title_edit(&mut self, node_id: usize) {
        let mut changed = false;
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            let trimmed = self.title_edit_buffer.trim();
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
        self.finish_title_edit(self.editing_title_node);
    }

    pub(in crate::app) fn start_startup_edit(&mut self, node_id: usize) {
        let Some(startup_script) =
            self.nodes
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
        self.editing_startup_node = Some(node_id);
        self.pending_startup_focus = Some(node_id);
        self.startup_edit_buffer = startup_script;
    }

    pub(in crate::app) fn commit_startup_edit(&mut self, node_id: usize, ctx: &egui::Context) {
        let mut changed = false;
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            let next = self.startup_edit_buffer.trim().to_owned();
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
        let Some(working_directory) = self
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
        self.editing_working_directory_node = Some(node_id);
        self.pending_working_directory_focus = Some(node_id);
        self.working_directory_edit_buffer = working_directory;
    }

    pub(in crate::app) fn commit_working_directory_edit(
        &mut self,
        node_id: usize,
        ctx: &egui::Context,
    ) {
        let mut changed = false;
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            let next = self.working_directory_edit_buffer.trim();
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
        self.finish_working_directory_edit(self.editing_working_directory_node);
    }

    pub(in crate::app) fn start_decision_buttons_edit(&mut self, node_id: usize) {
        let Some(existing_buttons) =
            self.nodes
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
        self.editing_decision_buttons_node = Some(node_id);
        self.pending_decision_buttons_focus = Some(node_id);
        self.decision_color_popup = None;
        self.decision_color_popup_pos = None;
        self.decision_buttons_edit_error = None;

        self.decision_buttons_edit_rows = existing_buttons
            .into_iter()
            .map(|button| {
                let color_rgb = button.color_rgb.unwrap_or([224, 232, 242]);
                DecisionButtonDraft {
                    label: button.label,
                    event_key: button.event_key,
                    color_rgb,
                    color_text: Self::decision_color_text_from_rgb(
                        self.decision_color_input_mode,
                        color_rgb,
                    ),
                }
            })
            .collect();

        if self.decision_buttons_edit_rows.is_empty() {
            let default_rgb = [212, 244, 226];
            self.decision_buttons_edit_rows.push(DecisionButtonDraft {
                label: "通过".to_owned(),
                event_key: "approve".to_owned(),
                color_rgb: default_rgb,
                color_text: Self::decision_color_text_from_rgb(
                    self.decision_color_input_mode,
                    default_rgb,
                ),
            });
        }
    }

    pub(in crate::app) fn add_decision_button_row(&mut self) {
        let default_rgb = [224, 232, 242];
        self.decision_buttons_edit_rows.push(DecisionButtonDraft {
            label: "新按钮".to_owned(),
            event_key: format!("event_{}", self.decision_buttons_edit_rows.len() + 1),
            color_rgb: default_rgb,
            color_text: Self::decision_color_text_from_rgb(
                self.decision_color_input_mode,
                default_rgb,
            ),
        });
    }

    pub(in crate::app) fn remove_decision_button_row(&mut self, row: usize) {
        if row < self.decision_buttons_edit_rows.len() {
            self.decision_buttons_edit_rows.remove(row);
            if let Some((node_id, popup_row)) = self.decision_color_popup {
                if popup_row == row {
                    self.decision_color_popup = None;
                    self.decision_color_popup_pos = None;
                } else if popup_row > row {
                    self.decision_color_popup = Some((node_id, popup_row - 1));
                }
            }
        }
    }

    pub(in crate::app) fn cancel_decision_buttons_edit(&mut self) {
        self.editing_decision_buttons_node = None;
        self.pending_decision_buttons_focus = None;
        self.decision_buttons_edit_rows.clear();
        self.decision_color_popup = None;
        self.decision_color_popup_pos = None;
        self.decision_buttons_edit_error = None;
    }

    pub(in crate::app) fn start_decision_queue_edit(&mut self, node_id: usize) {
        let Some(queue_text) =
            self.nodes
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
        self.editing_decision_queue_node = Some(node_id);
        self.pending_decision_queue_focus = Some(node_id);
        self.decision_queue_edit_buffer = queue_text;
    }

    pub(in crate::app) fn cancel_decision_queue_edit(&mut self) {
        self.editing_decision_queue_node = None;
        self.pending_decision_queue_focus = None;
        self.decision_queue_edit_buffer.clear();
    }

    pub(in crate::app) fn commit_decision_queue_edit(&mut self, node_id: usize) {
        let mut changed = false;
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            if let NodeData::Decision {
                pending_message,
                pending_messages,
                ..
            } = &mut node.data
            {
                let next_queue: Vec<String> = self
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
        let queue_text = self
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

        self.editing_script_queue_node = Some(node_id);
        self.pending_script_queue_focus = Some(node_id);
        self.script_queue_edit_buffer = queue_text;
    }

    pub(in crate::app) fn cancel_script_queue_edit(&mut self) {
        self.editing_script_queue_node = None;
        self.pending_script_queue_focus = None;
        self.script_queue_edit_buffer.clear();
    }

    pub(in crate::app) fn commit_script_queue_edit(&mut self, node_id: usize) {
        let mut changed = false;
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            if let NodeData::Script { pending_messages, .. } = &mut node.data {
                let next_queue: Vec<String> = self
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

    // ── Script node button editor ──

    pub(in crate::app) fn start_script_buttons_edit(&mut self, node_id: usize) {
        let Some(buttons) = self.nodes.iter().find(|n| n.id == node_id).and_then(|n| match &n.data {
            NodeData::Script { buttons, .. } => Some(buttons.clone()),
            _ => None,
        }) else {
            return;
        };

        self.editing_script_buttons_node = Some(node_id);
        self.script_buttons_edit_rows = buttons.into_iter().map(|b| DecisionButtonDraft {
            label: b.label,
            event_key: b.event_key,
            color_rgb: b.color_rgb.unwrap_or([200, 200, 220]),
            color_text: String::new(),
        }).collect();
        self.sync_script_button_color_texts();
        self.script_buttons_edit_error = None;
    }

    fn sync_script_button_color_texts(&mut self) {
        for row in &mut self.script_buttons_edit_rows {
            row.color_text = Self::decision_color_text_from_rgb(
                DecisionColorInputMode::Rgb,
                row.color_rgb,
            );
        }
    }

    pub(in crate::app) fn cancel_script_buttons_edit(&mut self) {
        self.editing_script_buttons_node = None;
        self.script_buttons_edit_rows.clear();
        self.script_buttons_edit_error = None;
    }

    pub(in crate::app) fn add_script_button_row(&mut self) {
        self.script_buttons_edit_rows.push(DecisionButtonDraft {
            label: String::new(),
            event_key: String::new(),
            color_rgb: [200, 200, 220],
            color_text: String::new(),
        });
        self.sync_script_button_color_texts();
    }

    pub(in crate::app) fn remove_script_button_row(&mut self, row: usize) {
        if row < self.script_buttons_edit_rows.len() {
            self.script_buttons_edit_rows.remove(row);
        }
    }

    pub(in crate::app) fn commit_script_buttons_edit(&mut self) -> bool {
        let Some(node_id) = self.editing_script_buttons_node else {
            return false;
        };

        let mut parsed_buttons = Vec::new();
        let mut seen_event_keys = std::collections::HashSet::new();

        for (idx, row) in self.script_buttons_edit_rows.iter().enumerate() {
            let label = row.label.trim();
            let event_key = row.event_key.trim();

            if label.is_empty() {
                self.script_buttons_edit_error =
                    Some(format!("第 {} 行显示名称不能为空", idx + 1));
                return false;
            }
            if event_key.is_empty() {
                self.script_buttons_edit_error = Some(format!("第 {} 行事件名不能为空", idx + 1));
                return false;
            }
            if !seen_event_keys.insert(event_key.to_owned()) {
                self.script_buttons_edit_error = Some(format!(
                    "第 {} 行事件名 '{event_key}' 重复",
                    idx + 1
                ));
                return false;
            }

            parsed_buttons.push(DecisionButton {
                label: label.to_owned(),
                event_key: event_key.to_owned(),
                color_rgb: Some(row.color_rgb),
            });
        }

        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            if let NodeData::Script { buttons, .. } = &mut node.data {
                *buttons = parsed_buttons;
            }
        }

        self.mark_workspace_dirty();
        self.cancel_script_buttons_edit();
        true
    }

    pub(in crate::app) fn commit_decision_buttons_edit(&mut self) -> bool {
        let Some(node_id) = self.editing_decision_buttons_node else {
            return false;
        };

        let mut parsed_buttons = Vec::new();
        let mut seen_event_keys = HashSet::new();

        for (idx, row) in self.decision_buttons_edit_rows.iter().enumerate() {
            let label = row.label.trim();
            let event_key = row.event_key.trim();

            if label.is_empty() {
                self.decision_buttons_edit_error =
                    Some(format!("第 {} 行显示名称不能为空", idx + 1));
                return false;
            }
            if event_key.is_empty() {
                self.decision_buttons_edit_error = Some(format!("第 {} 行事件名不能为空", idx + 1));
                return false;
            }
            if !seen_event_keys.insert(event_key.to_owned()) {
                self.decision_buttons_edit_error =
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
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
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
        let Some(code) = self
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
        if let Some(prev_id) = self.editing_script_node {
            if prev_id != node_id {
                self.commit_script_edit(prev_id);
            }
        }

        self.prepare_inline_node_edit(node_id);
        self.editing_script_node = Some(node_id);
        self.pending_script_focus = Some(node_id);
        self.script_edit_buffer = code;
    }

    pub(in crate::app) fn commit_script_edit(&mut self, node_id: usize) {
        let mut changed = false;
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            if let NodeData::Script {
                code,
                parsed_spec,
                ..
            } = &mut node.data
            {
                let new_code = self.script_edit_buffer.trim().to_owned();
                if *code != new_code {
                    *code = new_code;
                    // Re-parse the spec
                    *parsed_spec = script_node::parser::parse_script_spec(code).ok();
                    changed = true;
                }
            }
        }

        if changed {
            self.mark_workspace_dirty();
        }

        self.editing_script_node = None;
        self.pending_script_focus = None;
        self.script_edit_buffer.clear();
    }

    #[allow(dead_code)]
    pub(in crate::app) fn cancel_script_edit(&mut self) {
        self.editing_script_node = None;
        self.pending_script_focus = None;
        self.script_edit_buffer.clear();
    }

    pub(in crate::app) fn start_edge_edit(&mut self, edge: (usize, usize)) {
        if !self.has_edge(edge.0, edge.1) {
            return;
        }

        self.editing_text_node = None;
        self.pending_text_focus = None;
        self.editing_title_node = None;
        self.pending_title_focus = None;
        self.title_edit_buffer.clear();
        self.editing_startup_node = None;
        self.pending_startup_focus = None;
        self.startup_edit_buffer.clear();
        self.editing_working_directory_node = None;
        self.pending_working_directory_focus = None;
        self.working_directory_edit_buffer.clear();
        self.editing_decision_buttons_node = None;
        self.pending_decision_buttons_focus = None;
        self.decision_buttons_edit_rows.clear();
        self.decision_color_popup = None;
        self.decision_color_popup_pos = None;
        self.decision_buttons_edit_error = None;
        self.editing_decision_queue_node = None;
        self.pending_decision_queue_focus = None;
        self.decision_queue_edit_buffer.clear();

        self.set_edge_selection(edge);
        self.dragging = None;
        self.drag_start_pos = None;
        self.drag_group_start = None;
        self.resizing = None;

        self.editing_edge = Some(edge);
        self.pending_edge_focus = Some(edge);
        self.edge_edit_buffer = self
            .edge_route_key(edge.0, edge.1)
            .unwrap_or_default()
            .to_owned();
    }

    pub(in crate::app) fn commit_edge_edit(&mut self) {
        let Some((from, to)) = self.editing_edge else {
            return;
        };

        if !self.has_edge(from, to) {
            self.cancel_edge_edit();
            return;
        }

        let prev = self.edge_route_key(from, to).unwrap_or_default().to_owned();
        let next = self.edge_edit_buffer.trim().to_owned();

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
        self.editing_edge = None;
        self.pending_edge_focus = None;
        self.edge_edit_buffer.clear();
    }


}

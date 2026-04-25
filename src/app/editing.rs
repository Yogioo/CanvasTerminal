use super::GraphApp;
use crate::model::NodeData;
use eframe::egui;

impl GraphApp {
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
                if let NodeData::Terminal { title, .. } = &mut node.data {
                    if title != trimmed {
                        *title = trimmed.to_owned();
                        changed = true;
                    }
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

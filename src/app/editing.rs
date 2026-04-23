use super::GraphApp;
use eframe::egui;

impl GraphApp {
    fn finish_title_edit(&mut self, node_id: Option<usize>) {
        self.editing_title_node = None;
        self.pending_title_focus = None;
        self.title_edit_buffer.clear();
        self.suspend_terminal_focus = node_id;
    }

    fn finish_identity_edit(&mut self, node_id: Option<usize>) {
        self.editing_identity_node = None;
        self.pending_identity_focus = None;
        self.identity_edit_buffer.clear();
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

        self.editing_identity_node = None;
        self.pending_identity_focus = None;
        self.identity_edit_buffer.clear();

        self.editing_startup_node = None;
        self.pending_startup_focus = None;
        self.startup_edit_buffer.clear();
    }

    pub(in crate::app) fn start_title_edit(&mut self, node_id: usize) {
        let Some(title) = self
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .map(|n| n.title.clone())
        else {
            return;
        };

        self.prepare_inline_node_edit(node_id);
        self.editing_title_node = Some(node_id);
        self.pending_title_focus = Some(node_id);
        self.title_edit_buffer = title;
    }

    pub(in crate::app) fn commit_title_edit(&mut self, node_id: usize) {
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            let trimmed = self.title_edit_buffer.trim();
            if !trimmed.is_empty() {
                node.title = trimmed.to_owned();
            }
        }
        self.finish_title_edit(Some(node_id));
    }

    pub(in crate::app) fn cancel_title_edit(&mut self) {
        self.finish_title_edit(self.editing_title_node);
    }

    pub(in crate::app) fn start_identity_edit(&mut self, node_id: usize) {
        let Some(identity) = self
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .map(|n| n.identity.clone())
        else {
            return;
        };

        self.prepare_inline_node_edit(node_id);
        self.editing_identity_node = Some(node_id);
        self.pending_identity_focus = Some(node_id);
        self.identity_edit_buffer = identity;
    }

    pub(in crate::app) fn commit_identity_edit(&mut self, node_id: usize, ctx: &egui::Context) {
        let mut identity_changed = false;
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            let trimmed = self.identity_edit_buffer.trim();
            if !trimmed.is_empty() && node.identity != trimmed {
                node.identity = trimmed.to_owned();
                identity_changed = true;
            }
        }
        self.finish_identity_edit(Some(node_id));
        self.restart_terminal_if_changed(node_id, identity_changed, ctx);
    }

    pub(in crate::app) fn cancel_identity_edit(&mut self) {
        self.finish_identity_edit(self.editing_identity_node);
    }

    pub(in crate::app) fn start_startup_edit(&mut self, node_id: usize) {
        let Some(startup_script) = self
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .map(|n| n.startup_script.clone())
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
            if node.startup_script != next {
                node.startup_script = next;
                changed = true;
            }
        }
        self.finish_startup_edit(Some(node_id));
        self.restart_terminal_if_changed(node_id, changed, ctx);
    }
}

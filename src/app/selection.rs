use super::GraphApp;

impl GraphApp {
    pub(in crate::app) fn set_single_selection(&mut self, node_id: usize) {
        self.selected = Some(node_id);
        self.selected_nodes.clear();
        self.selected_nodes.insert(node_id);
    }

    pub(in crate::app) fn clear_selection(&mut self) {
        self.selected = None;
        self.selected_nodes.clear();
    }

    pub(in crate::app) fn set_selection_from_option(&mut self, node_id: Option<usize>) {
        if let Some(id) = node_id {
            self.set_single_selection(id);
        } else {
            self.clear_selection();
        }
    }

    pub(in crate::app) fn toggle_selection(&mut self, node_id: usize) {
        if self.selected_nodes.contains(&node_id) {
            self.selected_nodes.remove(&node_id);
            if self.selected == Some(node_id) {
                self.selected = self.selected_nodes.iter().copied().next();
            }
        } else {
            self.selected_nodes.insert(node_id);
            self.selected = Some(node_id);
        }
    }

    pub(in crate::app) fn remove_from_selection(&mut self, node_id: usize) {
        self.selected_nodes.remove(&node_id);
        if self.selected == Some(node_id) {
            self.selected = self.selected_nodes.iter().copied().next();
        }
    }
}

use super::GraphApp;

impl GraphApp {
    pub(in crate::app) fn set_single_selection(&mut self, node_id: usize) {
        self.selected = Some(node_id);
        self.selected_nodes.clear();
        self.selected_nodes.insert(node_id);
        self.selected_edge = None;
        self.dragging_edge_control = None;
    }

    pub(in crate::app) fn set_edge_selection(&mut self, edge: (usize, usize)) {
        if !self.has_edge(edge.0, edge.1) {
            return;
        }

        self.selected = None;
        self.selected_nodes.clear();
        self.selected_edge = Some(edge);
    }

    pub(in crate::app) fn clear_edge_selection(&mut self) {
        self.selected_edge = None;
        self.dragging_edge_control = None;
    }

    pub(in crate::app) fn clear_selection(&mut self) {
        self.selected = None;
        self.selected_nodes.clear();
        self.clear_edge_selection();
    }

    pub(in crate::app) fn toggle_selection(&mut self, node_id: usize) {
        self.selected_edge = None;
        self.dragging_edge_control = None;

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
        self.selected_edge = None;
        self.dragging_edge_control = None;
        self.selected_nodes.remove(&node_id);
        if self.selected == Some(node_id) {
            self.selected = self.selected_nodes.iter().copied().next();
        }
    }
}

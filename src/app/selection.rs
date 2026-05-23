use super::GraphApp;

impl GraphApp {
    pub(in crate::app) fn set_single_selection(&mut self, node_id: usize) {
        self.ws.selected = Some(node_id);
        self.ws.selected_nodes.clear();
        self.ws.selected_nodes.insert(node_id);
        self.ws.selected_edge = None;
        self.ws.dragging_edge_control = None;
    }

    pub(in crate::app) fn set_edge_selection(&mut self, edge: (usize, usize)) {
        if !self.has_edge(edge.0, edge.1) {
            return;
        }

        self.ws.selected = None;
        self.ws.selected_nodes.clear();
        self.ws.selected_edge = Some(edge);
    }

    pub(in crate::app) fn clear_edge_selection(&mut self) {
        self.ws.selected_edge = None;
        self.ws.dragging_edge_control = None;
    }

    pub(in crate::app) fn clear_selection(&mut self) {
        self.ws.selected = None;
        self.ws.selected_nodes.clear();
        self.clear_edge_selection();
    }

    pub(in crate::app) fn toggle_selection(&mut self, node_id: usize) {
        self.ws.selected_edge = None;
        self.ws.dragging_edge_control = None;

        if self.ws.selected_nodes.contains(&node_id) {
            self.ws.selected_nodes.remove(&node_id);
            if self.ws.selected == Some(node_id) {
                self.ws.selected = self.ws.selected_nodes.iter().copied().next();
            }
        } else {
            self.ws.selected_nodes.insert(node_id);
            self.ws.selected = Some(node_id);
        }
    }

    pub(in crate::app) fn remove_from_selection(&mut self, node_id: usize) {
        self.ws.selected_edge = None;
        self.ws.dragging_edge_control = None;
        self.ws.selected_nodes.remove(&node_id);
        if self.ws.selected == Some(node_id) {
            self.ws.selected = self.ws.selected_nodes.iter().copied().next();
        }
    }
}

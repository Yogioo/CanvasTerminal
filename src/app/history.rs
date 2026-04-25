use super::GraphApp;
use crate::model::Node;
use eframe::egui::Pos2;

#[derive(Clone)]
pub(in crate::app) enum HistoryEntry {
    CreateBatch {
        nodes: Vec<Node>,
    },
    DeleteBatch {
        nodes: Vec<Node>,
        edges: Vec<(usize, usize)>,
    },
    MoveNode {
        node_id: usize,
        from: Pos2,
        to: Pos2,
    },
    MoveNodes {
        nodes: Vec<(usize, Pos2, Pos2)>,
    },
    ReorderNodes {
        before: Vec<usize>,
    },
}

impl GraphApp {
    pub(in crate::app) fn run_edit_menu_action(&mut self, action_id: usize) {
        match action_id {
            0 => self.undo_last_change(),
            1 => self.redo_last_change(),
            _ => {}
        }
    }

    pub(in crate::app) fn push_history(&mut self, entry: HistoryEntry) {
        self.undo_stack.push(entry);
        self.redo_stack.clear();
        self.mark_workspace_dirty();
    }

    pub(in crate::app) fn record_move_history(&mut self, node_id: usize, from: Pos2, to: Pos2) {
        if from.distance(to) <= 0.1 {
            return;
        }

        self.push_history(HistoryEntry::MoveNode { node_id, from, to });
    }

    pub(in crate::app) fn record_nodes_move_history(&mut self, nodes: Vec<(usize, Pos2, Pos2)>) {
        let moved_nodes: Vec<(usize, Pos2, Pos2)> = nodes
            .into_iter()
            .filter(|(_, from, to)| from.distance(*to) > 0.1)
            .collect();

        if moved_nodes.is_empty() {
            return;
        }

        self.push_history(HistoryEntry::MoveNodes { nodes: moved_nodes });
    }

    pub(in crate::app) fn record_cut_history(
        &mut self,
        before_nodes: Vec<Node>,
        before_edges: Vec<(usize, usize)>,
    ) {
        let removed_nodes: Vec<Node> = before_nodes
            .into_iter()
            .filter(|old_node| !self.nodes.iter().any(|n| n.id == old_node.id))
            .collect();

        let removed_edges: Vec<(usize, usize)> = before_edges
            .into_iter()
            .filter(|old_edge| !self.edges.contains(old_edge))
            .collect();

        if removed_nodes.is_empty() && removed_edges.is_empty() {
            return;
        }

        self.push_history(HistoryEntry::DeleteBatch {
            nodes: removed_nodes,
            edges: removed_edges,
        });
    }

    fn apply_history_entry(&mut self, entry: HistoryEntry) -> HistoryEntry {
        match entry {
            HistoryEntry::CreateBatch { nodes } => {
                self.redo_create_batch(&nodes);
                HistoryEntry::CreateBatch { nodes }
            }
            HistoryEntry::DeleteBatch { nodes, edges } => {
                for node in &nodes {
                    if self.nodes.iter().any(|n| n.id == node.id) {
                        continue;
                    }
                    if node.id >= self.next_node_id {
                        self.next_node_id = node.id + 1;
                    }
                    self.nodes.push(node.clone());
                }

                self.nodes.sort_by_key(|n| n.id);

                for (from, to) in &edges {
                    if self.has_edge(*from, *to) {
                        continue;
                    }
                    let has_from = self.nodes.iter().any(|n| n.id == *from);
                    let has_to = self.nodes.iter().any(|n| n.id == *to);
                    if has_from && has_to {
                        self.edges.push((*from, *to));
                    }
                }

                HistoryEntry::DeleteBatch { nodes, edges }
            }
            HistoryEntry::MoveNode { node_id, from, to } => {
                if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
                    node.pos = from;
                }
                HistoryEntry::MoveNode {
                    node_id,
                    from: to,
                    to: from,
                }
            }
            HistoryEntry::MoveNodes { nodes } => {
                let mut swapped = Vec::with_capacity(nodes.len());
                for (node_id, from, to) in nodes {
                    if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
                        node.pos = from;
                    }
                    swapped.push((node_id, to, from));
                }
                HistoryEntry::MoveNodes { nodes: swapped }
            }
            HistoryEntry::ReorderNodes { before } => {
                let current: Vec<usize> = self.nodes.iter().map(|n| n.id).collect();
                self.apply_node_order(&before);
                HistoryEntry::ReorderNodes { before: current }
            }
        }
    }

    fn undo_create_batch(&mut self, nodes: &[Node]) {
        for node in nodes {
            self.remove_node(node.id);
        }
    }

    fn redo_create_batch(&mut self, nodes: &[Node]) {
        for node in nodes {
            if self.nodes.iter().any(|n| n.id == node.id) {
                continue;
            }
            if node.id >= self.next_node_id {
                self.next_node_id = node.id + 1;
            }
            self.nodes.push(node.clone());
        }
    }

    fn redo_delete_batch(&mut self, nodes: &[Node], edges: &[(usize, usize)]) {
        for (from, to) in edges {
            self.edges.retain(|edge| edge != &(*from, *to));
        }
        self.prune_edge_state();

        for node in nodes {
            self.remove_node(node.id);
        }
    }

    pub(in crate::app) fn undo_last_change(&mut self) {
        let Some(entry) = self.undo_stack.pop() else {
            return;
        };

        self.mark_workspace_dirty();

        let redo_entry = match &entry {
            HistoryEntry::CreateBatch { nodes } => {
                self.undo_create_batch(nodes);
                HistoryEntry::CreateBatch {
                    nodes: nodes.clone(),
                }
            }
            HistoryEntry::DeleteBatch { nodes, edges } => {
                let cloned = HistoryEntry::DeleteBatch {
                    nodes: nodes.clone(),
                    edges: edges.clone(),
                };
                self.apply_history_entry(cloned)
            }
            _ => self.apply_history_entry(entry),
        };

        self.redo_stack.push(redo_entry);
    }

    pub(in crate::app) fn redo_last_change(&mut self) {
        let Some(entry) = self.redo_stack.pop() else {
            return;
        };

        self.mark_workspace_dirty();

        let undo_entry = match &entry {
            HistoryEntry::CreateBatch { nodes } => {
                self.redo_create_batch(nodes);
                HistoryEntry::CreateBatch {
                    nodes: nodes.clone(),
                }
            }
            HistoryEntry::DeleteBatch { nodes, edges } => {
                self.redo_delete_batch(nodes, edges);
                HistoryEntry::DeleteBatch {
                    nodes: nodes.clone(),
                    edges: edges.clone(),
                }
            }
            _ => self.apply_history_entry(entry),
        };

        self.undo_stack.push(undo_entry);
    }
}

use super::{history::CreatedEdge, GraphApp, NodeClipboardEdge, NodeClipboardPayload};
use eframe::egui::Pos2;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

impl GraphApp {
    pub(in crate::app) fn copy_selected_nodes_to_internal_clipboard(&mut self) -> bool {
        if self.selected_nodes.is_empty() {
            return false;
        }

        let selected_ids = self.selected_nodes.clone();
        let nodes: Vec<_> = self
            .nodes
            .iter()
            .filter(|node| selected_ids.contains(&node.id))
            .cloned()
            .collect();

        if nodes.is_empty() {
            return false;
        }

        let anchor = Self::clipboard_anchor(&nodes);
        let edges = self.collect_clipboard_edges(&selected_ids);

        self.node_clipboard = Some(NodeClipboardPayload {
            nodes,
            edges,
            anchor,
        });
        true
    }

    pub(in crate::app) fn paste_nodes_from_internal_clipboard(&mut self, spawn_pos: Pos2) -> bool {
        let Some(payload) = self.node_clipboard.clone() else {
            return false;
        };

        if payload.nodes.is_empty() {
            return false;
        }

        let mut id_map = HashMap::new();
        let mut pasted_nodes = Vec::with_capacity(payload.nodes.len());

        for template in payload.nodes {
            let old_id = template.id;
            let mut node = template;
            node.id = self.alloc_node_id();
            node.uid = Uuid::new_v4().to_string();
            node.pos = Pos2::new(
                spawn_pos.x + (node.pos.x - payload.anchor.x),
                spawn_pos.y + (node.pos.y - payload.anchor.y),
            );
            id_map.insert(old_id, node.id);
            pasted_nodes.push(node);
        }

        let mut pasted_edges = Vec::new();
        for edge in payload.edges {
            let Some(&from) = id_map.get(&edge.from) else {
                continue;
            };
            let Some(&to) = id_map.get(&edge.to) else {
                continue;
            };
            if self.has_edge(from, to) {
                continue;
            }

            self.edges.push((from, to));

            if let Some(route_key) = edge.route_key.clone() {
                self.set_edge_route_key(from, to, route_key);
            }
            if let Some(curve_bias) = edge.curve_bias {
                self.set_edge_curve_bias(from, to, curve_bias);
            }
            if let Some(offsets) = edge.control_offsets {
                self.set_edge_control_offsets(from, to, offsets);
            }

            pasted_edges.push(CreatedEdge {
                from,
                to,
                route_key: edge.route_key,
                curve_bias: edge.curve_bias,
                control_offsets: edge.control_offsets,
            });
        }

        self.nodes.extend(pasted_nodes.clone());

        self.push_history(super::history::HistoryEntry::CreateBatch {
            nodes: pasted_nodes.clone(),
            edges: pasted_edges,
        });

        self.selected_nodes.clear();
        for node in &pasted_nodes {
            self.selected_nodes.insert(node.id);
        }
        self.selected = pasted_nodes.last().map(|node| node.id);
        self.clear_edge_selection();

        true
    }

    fn collect_clipboard_edges(&self, selected_ids: &HashSet<usize>) -> Vec<NodeClipboardEdge> {
        self.edges
            .iter()
            .filter_map(|(from, to)| {
                if !selected_ids.contains(from) || !selected_ids.contains(to) {
                    return None;
                }

                let route_key = self
                    .edge_route_keys
                    .get(&(*from, *to))
                    .cloned()
                    .filter(|value| !value.trim().is_empty());
                let curve_bias = self
                    .edge_curve_biases
                    .get(&(*from, *to))
                    .copied()
                    .filter(|bias| bias.is_finite() && bias.abs() > 0.001);
                let control_offsets = self.edge_control_offsets.get(&(*from, *to)).copied().filter(
                    |offsets| {
                        offsets.source.length_sq() > 0.01 || offsets.target.length_sq() > 0.01
                    },
                );

                Some(NodeClipboardEdge {
                    from: *from,
                    to: *to,
                    route_key,
                    curve_bias,
                    control_offsets,
                })
            })
            .collect()
    }

    fn clipboard_anchor(nodes: &[crate::model::Node]) -> Pos2 {
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;

        for node in nodes {
            min_x = min_x.min(node.pos.x);
            min_y = min_y.min(node.pos.y);
        }

        Pos2::new(min_x, min_y)
    }
}

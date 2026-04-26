use super::{history::CreatedEdge, GraphApp};
use crate::constants::GROUP_HEADER_HEIGHT;
use crate::model::{Node, NodeData, NodeKind};
use eframe::egui::{vec2, Pos2, Rect};
use uuid::Uuid;

const GROUP_PADDING: f32 = 20.0;

impl GraphApp {
    pub(in crate::app) fn create_group_from_selection(&mut self) -> Option<usize> {
        let child_ids: Vec<usize> = self
            .nodes
            .iter()
            .filter(|node| self.selected_nodes.contains(&node.id) && node.kind != NodeKind::Group)
            .map(|node| node.id)
            .collect();

        if child_ids.len() < 2 {
            return None;
        }

        let title = self.next_group_title();
        let group_id = self.alloc_node_id();
        let mut group = Node {
            id: group_id,
            uid: Uuid::new_v4().to_string(),
            kind: NodeKind::Group,
            data: NodeData::Group {
                title,
                child_node_ids: child_ids.clone(),
            },
            pos: Pos2::new(0.0, 0.0),
            size: vec2(100.0, 100.0),
        };

        self.apply_group_bounds_from_children(&mut group);

        let insert_at = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| child_ids.contains(&node.id))
            .map(|(idx, _)| idx)
            .min()
            .unwrap_or(self.nodes.len());

        self.nodes.insert(insert_at, group.clone());
        self.push_history(super::history::HistoryEntry::CreateBatch {
            nodes: vec![group],
            edges: Vec::<CreatedEdge>::new(),
        });
        self.set_single_selection(group_id);
        self.mark_workspace_dirty();
        Some(group_id)
    }

    pub(in crate::app) fn sync_all_group_bounds(&mut self) {
        let group_ids: Vec<usize> = self
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Group)
            .map(|node| node.id)
            .collect();

        for group_id in group_ids {
            self.sync_group_bounds(group_id);
        }
    }

    pub(in crate::app) fn sync_group_bounds(&mut self, group_id: usize) {
        let Some(group_idx) = self.nodes.iter().position(|node| node.id == group_id) else {
            return;
        };

        if self.nodes[group_idx].kind != NodeKind::Group {
            return;
        }

        let mut updated = self.nodes[group_idx].clone();
        self.apply_group_bounds_from_children(&mut updated);
        self.nodes[group_idx].pos = updated.pos;
        self.nodes[group_idx].size = updated.size;
    }

    pub(in crate::app) fn remove_child_from_groups(&mut self, node_id: usize) {
        let mut empty_groups = Vec::new();

        for group in self.nodes.iter_mut().filter(|node| node.kind == NodeKind::Group) {
            let NodeData::Group { child_node_ids, .. } = &mut group.data else {
                continue;
            };

            let before_len = child_node_ids.len();
            child_node_ids.retain(|child_id| *child_id != node_id);
            if child_node_ids.len() == before_len {
                continue;
            }

            if child_node_ids.is_empty() {
                empty_groups.push(group.id);
            }
        }

        for group_id in empty_groups {
            self.remove_node(group_id);
        }

        self.sync_all_group_bounds();
    }

    pub(in crate::app) fn top_group_id_at(&self, pointer_world: Pos2) -> Option<usize> {
        self.nodes
            .iter()
            .rev()
            .find(|node| {
                node.kind == NodeKind::Group
                    && Rect::from_min_size(node.pos, node.size).contains(pointer_world)
            })
            .map(|node| node.id)
    }

    pub(in crate::app) fn group_child_hit_at(
        &self,
        group_id: usize,
        pointer_world: Pos2,
    ) -> Option<usize> {
        let child_ids = self.group_child_ids(group_id)?;
        self.nodes
            .iter()
            .rev()
            .find(|node| {
                node.kind != NodeKind::Group
                    && child_ids.contains(&node.id)
                    && Rect::from_min_size(node.pos, node.size).contains(pointer_world)
            })
            .map(|node| node.id)
    }

    pub(in crate::app) fn group_child_ids(&self, group_id: usize) -> Option<Vec<usize>> {
        let group = self.nodes.iter().find(|node| node.id == group_id)?;
        let NodeData::Group { child_node_ids, .. } = &group.data else {
            return None;
        };

        Some(child_node_ids.clone())
    }

    pub(in crate::app) fn sanitize_groups(&mut self) {
        let non_group_ids: std::collections::HashSet<usize> = self
            .nodes
            .iter()
            .filter(|node| node.kind != NodeKind::Group)
            .map(|node| node.id)
            .collect();

        for group in self.nodes.iter_mut().filter(|node| node.kind == NodeKind::Group) {
            if let NodeData::Group {
                title,
                child_node_ids,
            } = &mut group.data
            {
                *title = title.trim().to_owned();
                if title.is_empty() {
                    *title = "Group".to_owned();
                }
                child_node_ids.retain(|child_id| non_group_ids.contains(child_id));
            }
        }

        self.sync_all_group_bounds();
    }

    fn apply_group_bounds_from_children(&self, group: &mut Node) {
        let NodeData::Group { child_node_ids, .. } = &group.data else {
            return;
        };

        let children: Vec<&Node> = self
            .nodes
            .iter()
            .filter(|node| child_node_ids.contains(&node.id))
            .collect();

        if children.is_empty() {
            return;
        }

        let mut bounds = Rect::from_min_size(children[0].pos, children[0].size);
        for child in children.into_iter().skip(1) {
            bounds = bounds.union(Rect::from_min_size(child.pos, child.size));
        }

        group.pos = Pos2::new(
            bounds.min.x - GROUP_PADDING,
            bounds.min.y - GROUP_PADDING - GROUP_HEADER_HEIGHT,
        );
        group.size = vec2(
            bounds.width() + GROUP_PADDING * 2.0,
            bounds.height() + GROUP_PADDING * 2.0 + GROUP_HEADER_HEIGHT,
        );
    }

    fn next_group_title(&self) -> String {
        let mut max_index = 0usize;

        for node in self.nodes.iter().filter(|node| node.kind == NodeKind::Group) {
            let NodeData::Group { title, .. } = &node.data else {
                continue;
            };

            let Some(suffix) = title.strip_prefix("Group ") else {
                continue;
            };
            let Ok(parsed) = suffix.parse::<usize>() else {
                continue;
            };
            max_index = max_index.max(parsed);
        }

        format!("Group {}", max_index + 1)
    }

    pub(in crate::app) fn find_node_id_at(&self, local: Pos2) -> Option<usize> {
        self.nodes
            .iter()
            .rev()
            .find(|node| Rect::from_min_size(node.pos, node.size).contains(local))
            .map(|node| node.id)
    }
}

use super::{history::CreatedEdge, GraphApp};
use crate::constants::GROUP_HEADER_HEIGHT;
use crate::model::{Node, NodeData, NodeKind};
use eframe::egui::{vec2, Pos2, Rect};
use std::collections::HashSet;
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

        if self.find_same_members_group(&child_ids).is_some() {
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

    pub(in crate::app) fn group_child_ids(&self, group_id: usize) -> Option<Vec<usize>> {
        let group = self.nodes.iter().find(|node| node.id == group_id)?;
        let NodeData::Group { child_node_ids, .. } = &group.data else {
            return None;
        };

        Some(child_node_ids.clone())
    }

    fn find_same_members_group(&self, node_ids: &[usize]) -> Option<usize> {
        let mut target = node_ids.to_vec();
        target.sort_unstable();
        target.dedup();

        self.nodes.iter().find_map(|node| {
            if node.kind != NodeKind::Group {
                return None;
            }
            let NodeData::Group { child_node_ids, .. } = &node.data else {
                return None;
            };

            let mut current = child_node_ids.clone();
            current.sort_unstable();
            current.dedup();

            (current == target).then_some(node.id)
        })
    }

    pub(in crate::app) fn resolve_drag_node_ids(
        &self,
        anchor_id: usize,
        multi_drag: bool,
    ) -> HashSet<usize> {
        let base_ids: HashSet<usize> = if multi_drag {
            self.selected_nodes.clone()
        } else {
            std::iter::once(anchor_id).collect()
        };

        let mut drag_ids = HashSet::new();
        for selected_id in &base_ids {
            let is_group = self
                .nodes
                .iter()
                .find(|node| node.id == *selected_id)
                .is_some_and(|node| node.kind == NodeKind::Group);

            if is_group {
                if let Some(children) = self.group_child_ids(*selected_id) {
                    for child_id in children {
                        drag_ids.insert(child_id);
                    }
                }
            } else {
                drag_ids.insert(*selected_id);
            }
        }

        drag_ids
    }

    pub(in crate::app) fn jump_selected_nodes_to(&mut self, pointer_world: Pos2) -> bool {
        if self.selected_nodes.is_empty() {
            return false;
        }

        let moving_ids: Vec<usize> = self
            .resolve_jump_node_ids()
            .into_iter()
            .collect();

        if moving_ids.is_empty() {
            return false;
        }

        let moving_nodes: Vec<_> = self
            .nodes
            .iter()
            .filter(|node| moving_ids.contains(&node.id))
            .cloned()
            .collect();

        if moving_nodes.is_empty() {
            return false;
        }

        let anchor = moving_nodes
            .iter()
            .fold(Pos2::new(f32::INFINITY, f32::INFINITY), |acc, node| {
                Pos2::new(acc.x.min(node.pos.x), acc.y.min(node.pos.y))
            });
        let delta = pointer_world - anchor;

        let mut moves = Vec::new();
        for node in self.nodes.iter_mut().filter(|node| moving_ids.contains(&node.id)) {
            let from = node.pos;
            node.pos += delta;
            moves.push((node.id, from, node.pos));
        }

        self.remove_nodes_from_all_groups(&moving_ids);
        if let Some(target_group_id) = self.top_group_id_at(pointer_world) {
            self.add_nodes_to_group(target_group_id, &moving_ids);
        }

        self.sync_all_group_bounds();
        self.record_nodes_move_history(moves);
        self.mark_workspace_dirty();
        true
    }

    fn resolve_jump_node_ids(&self) -> HashSet<usize> {
        let mut ids = HashSet::new();
        for selected_id in &self.selected_nodes {
            let is_group = self
                .nodes
                .iter()
                .find(|node| node.id == *selected_id)
                .is_some_and(|node| node.kind == NodeKind::Group);

            if is_group {
                if let Some(children) = self.group_child_ids(*selected_id) {
                    for child_id in children {
                        ids.insert(child_id);
                    }
                }
            } else {
                ids.insert(*selected_id);
            }
        }
        ids
    }

    fn remove_nodes_from_all_groups(&mut self, node_ids: &[usize]) {
        for group in self.nodes.iter_mut().filter(|node| node.kind == NodeKind::Group) {
            if let NodeData::Group { child_node_ids, .. } = &mut group.data {
                child_node_ids.retain(|child_id| !node_ids.contains(child_id));
            }
        }
    }

    fn add_nodes_to_group(&mut self, group_id: usize, node_ids: &[usize]) {
        let Some(group) = self.nodes.iter_mut().find(|node| node.id == group_id) else {
            return;
        };

        if let NodeData::Group { child_node_ids, .. } = &mut group.data {
            for node_id in node_ids {
                if !child_node_ids.contains(node_id) {
                    child_node_ids.push(*node_id);
                }
            }
        }
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

}

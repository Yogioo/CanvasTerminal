mod ui;

use crate::constants::TERMINAL_HEADER_HEIGHT;
use crate::model::{Node, NodeKind};
use crate::shell::system_shell;
use eframe::egui::{self, vec2, Pos2, Rect, SidePanel, Stroke};
use egui_term::{BackendSettings, PtyEvent, TerminalBackend};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc;

enum HistoryEntry {
    DeleteBatch {
        nodes: Vec<Node>,
        edges: Vec<(usize, usize)>,
    },
    MoveNode {
        node_id: usize,
        from: Pos2,
        to: Pos2,
    },
}

pub struct GraphApp {
    nodes: Vec<Node>,
    edges: Vec<(usize, usize)>,
    selected: Option<usize>,
    dragging: Option<(usize, egui::Vec2)>,
    drag_start_pos: Option<(usize, Pos2)>,
    pan: egui::Vec2,
    zoom: f32,

    terminal_backends: HashMap<usize, TerminalBackend>,
    pty_rx: mpsc::Receiver<(u64, PtyEvent)>,
    pty_tx: mpsc::Sender<(u64, PtyEvent)>,
    terminal_exited: HashSet<usize>,
    terminal_errors: HashMap<usize, String>,

    next_node_id: usize,
    menu_search_text: String,
    menu_search_selected: usize,
    menu_nav_level: u8,
    menu_nav_selected: usize,
    pending_menu_search_focus: bool,
    editing_text_node: Option<usize>,
    pending_text_focus: Option<usize>,
    context_menu_node: Option<usize>,
    context_menu_local_pos: Option<Pos2>,
    linking_from: Option<usize>,
    linking_pointer_local: Option<Pos2>,
    cutting_path_local: Vec<Pos2>,
    right_drag_moved: bool,
    cut_snapshot_nodes: Option<Vec<Node>>,
    cut_snapshot_edges: Option<Vec<(usize, usize)>>,
    undo_stack: Vec<HistoryEntry>,
    change_history: Vec<String>,
}

impl GraphApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let (pty_tx, pty_rx) = mpsc::channel();

        let nodes = Vec::new();

        let app = Self {
            nodes,
            edges: Vec::new(),
            selected: None,
            dragging: None,
            drag_start_pos: None,
            pan: vec2(0.0, 0.0),
            zoom: 1.0,
            terminal_backends: HashMap::new(),
            pty_rx,
            pty_tx,
            terminal_exited: HashSet::new(),
            terminal_errors: HashMap::new(),
            next_node_id: 1,
            menu_search_text: String::new(),
            menu_search_selected: 0,
            menu_nav_level: 0,
            menu_nav_selected: 0,
            pending_menu_search_focus: false,
            editing_text_node: None,
            pending_text_focus: None,
            context_menu_node: None,
            context_menu_local_pos: None,
            linking_from: None,
            linking_pointer_local: None,
            cutting_path_local: Vec::new(),
            right_drag_moved: false,
            cut_snapshot_nodes: None,
            cut_snapshot_edges: None,
            undo_stack: Vec::new(),
            change_history: Vec::new(),
        };

        app
    }

    fn alloc_node_id(&mut self) -> usize {
        let id = self.next_node_id;
        self.next_node_id += 1;
        id
    }

    fn create_terminal_node(&mut self, pos: Pos2) {
        let id = self.alloc_node_id();
        self.nodes.push(Node {
            id,
            title: format!("Terminal {id}"),
            kind: NodeKind::Terminal,
            category: "终端".to_owned(),
            text_body: String::new(),
            pos,
            size: vec2(420.0, 220.0),
            status: "Running",
        });
        self.selected = Some(id);
    }

    fn create_text_node(&mut self, pos: Pos2, edit_now: bool) {
        let id = self.alloc_node_id();
        self.nodes.push(Node {
            id,
            title: format!("文本节点 {id}"),
            kind: NodeKind::Text,
            category: "文本".to_owned(),
            text_body: "双击继续编辑".to_owned(),
            pos,
            size: vec2(260.0, 140.0),
            status: "Editable",
        });
        self.selected = Some(id);
        if edit_now {
            self.editing_text_node = Some(id);
            self.pending_text_focus = Some(id);
        }
    }

    fn node_kind_name(kind: &NodeKind) -> &'static str {
        match kind {
            NodeKind::Terminal => "终端",
            NodeKind::Text => "文本",
        }
    }

    fn menu_item_matches(&self, label: &str) -> bool {
        let kw = self.menu_search_text.trim();
        if kw.is_empty() {
            return true;
        }

        label.contains(kw)
    }

    fn menu_item_highlighted_label(&self, label: &str) -> egui::text::LayoutJob {
        let kw = self.menu_search_text.trim();
        let mut job = egui::text::LayoutJob::default();

        let mut normal = egui::TextFormat::default();
        normal.color = egui::Color32::BLACK;

        if kw.is_empty() {
            job.append(label, 0.0, normal.clone());
            return job;
        }

        let mut highlight = egui::TextFormat::default();
        highlight.color = egui::Color32::from_rgb(255, 196, 0);

        let mut last = 0;
        for (start, matched) in label.match_indices(kw) {
            if start > last {
                job.append(&label[last..start], 0.0, normal.clone());
            }
            job.append(matched, 0.0, highlight.clone());
            last = start + matched.len();
        }

        if last < label.len() {
            job.append(&label[last..], 0.0, normal);
        }

        job
    }

    fn selected_terminal_id(&self) -> Option<usize> {
        let id = self.selected?;
        let node = self.nodes.iter().find(|n| n.id == id)?;
        if matches!(node.kind, NodeKind::Terminal) {
            Some(id)
        } else {
            None
        }
    }

    fn ensure_terminal(&mut self, node_id: usize, ctx: &egui::Context) {
        if self.terminal_backends.contains_key(&node_id) {
            return;
        }

        let shell = system_shell();
        match TerminalBackend::new(
            node_id as u64,
            ctx.clone(),
            self.pty_tx.clone(),
            BackendSettings {
                shell,
                args: vec![],
                working_directory: std::env::current_dir().ok(),
            },
        ) {
            Ok(backend) => {
                self.terminal_backends.insert(node_id, backend);
                self.terminal_exited.remove(&node_id);
                self.terminal_errors.remove(&node_id);
            }
            Err(e) => {
                self.terminal_errors
                    .insert(node_id, format!("终端启动失败: {e}"));
            }
        }
    }

    fn restart_terminal(&mut self, node_id: usize, ctx: &egui::Context) {
        self.terminal_backends.remove(&node_id);
        self.terminal_exited.remove(&node_id);
        self.terminal_errors.remove(&node_id);
        self.ensure_terminal(node_id, ctx);
    }

    fn poll_terminal_events(&mut self) {
        while let Ok((id, event)) = self.pty_rx.try_recv() {
            if let PtyEvent::Exit = event {
                let node_id = id as usize;
                self.terminal_exited.insert(node_id);
                self.terminal_backends.remove(&node_id);
            }
        }
    }

    fn find_node_at(&self, local: Pos2) -> Option<(usize, egui::Vec2)> {
        for n in self.nodes.iter().rev() {
            let r = Rect::from_min_size(n.pos, n.size);
            if r.contains(local) {
                return Some((n.id, n.pos.to_vec2()));
            }
        }
        None
    }

    fn find_node_hit(&self, local: Pos2) -> Option<(usize, egui::Vec2, bool)> {
        for n in self.nodes.iter().rev() {
            let r = Rect::from_min_size(n.pos, n.size);
            if !r.contains(local) {
                continue;
            }

            let can_drag = match n.kind {
                NodeKind::Text => true,
                NodeKind::Terminal => local.y <= n.pos.y + TERMINAL_HEADER_HEIGHT,
            };

            return Some((n.id, n.pos.to_vec2(), can_drag));
        }
        None
    }

    fn world_to_screen_pos(&self, canvas_rect: Rect, world: Pos2) -> Pos2 {
        canvas_rect.min + self.pan + world.to_vec2() * self.zoom
    }

    fn world_to_screen_rect(&self, canvas_rect: Rect, world_rect: Rect) -> Rect {
        Rect::from_min_size(
            self.world_to_screen_pos(canvas_rect, world_rect.min),
            world_rect.size() * self.zoom,
        )
    }

    fn screen_to_world_pos(&self, canvas_rect: Rect, screen: Pos2) -> Pos2 {
        ((screen - canvas_rect.min - self.pan) / self.zoom).to_pos2()
    }

    fn terminal_content_rect_screen(&self, node_id: usize, canvas_rect: Rect) -> Option<Rect> {
        let n = self.nodes.iter().find(|n| n.id == node_id)?;
        if !matches!(n.kind, NodeKind::Terminal) {
            return None;
        }

        let outer_world = Rect::from_min_size(n.pos, n.size);
        let inner_world = Rect::from_min_max(
            outer_world.min + vec2(2.0, TERMINAL_HEADER_HEIGHT + 2.0),
            outer_world.max - vec2(2.0, 2.0),
        );
        Some(self.world_to_screen_rect(canvas_rect, inner_world))
    }

    fn terminal_content_rects_screen(&self, canvas_rect: Rect) -> Vec<(usize, Rect)> {
        self.nodes
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::Terminal))
            .filter_map(|n| self.terminal_content_rect_screen(n.id, canvas_rect).map(|r| (n.id, r)))
            .collect()
    }

    fn has_edge(&self, from: usize, to: usize) -> bool {
        self.edges.iter().any(|(a, b)| *a == from && *b == to)
    }

    fn edge_segment_local(&self, from: usize, to: usize) -> Option<(Pos2, Pos2)> {
        let a = self.nodes.iter().find(|n| n.id == from)?;
        let b = self.nodes.iter().find(|n| n.id == to)?;
        let start = a.pos + vec2(a.size.x, a.size.y * 0.5);
        let end = b.pos + vec2(0.0, b.size.y * 0.5);
        Some((start, end))
    }

    fn cut_edges_intersecting_segment(&mut self, cut_a: Pos2, cut_b: Pos2) {
        let hit: Vec<bool> = self
            .edges
            .iter()
            .map(|(from, to)| {
                self.edge_segment_local(*from, *to)
                    .is_some_and(|(a, b)| Self::segments_intersect(cut_a, cut_b, a, b))
            })
            .collect();

        let mut idx = 0usize;
        self.edges.retain(|_| {
            let keep = !hit[idx];
            idx += 1;
            keep
        });
    }

    fn cut_nodes_intersecting_segment(&mut self, cut_a: Pos2, cut_b: Pos2) {
        let hit_ids: Vec<usize> = self
            .nodes
            .iter()
            .filter(|n| {
                let rect = Rect::from_min_size(n.pos, n.size);
                Self::segment_intersects_rect(cut_a, cut_b, rect)
            })
            .map(|n| n.id)
            .collect();

        for id in hit_ids {
            self.remove_node(id);
        }
    }

    fn remove_node(&mut self, node_id: usize) {
        self.nodes.retain(|n| n.id != node_id);
        self.edges.retain(|(from, to)| *from != node_id && *to != node_id);
        self.terminal_backends.remove(&node_id);
        self.terminal_exited.remove(&node_id);
        self.terminal_errors.remove(&node_id);

        if self.selected == Some(node_id) {
            self.selected = None;
        }
        if self.dragging.is_some_and(|(id, _)| id == node_id) {
            self.dragging = None;
            self.drag_start_pos = None;
        }
        if self.editing_text_node == Some(node_id) {
            self.editing_text_node = None;
            self.pending_text_focus = None;
        }
        if self.linking_from == Some(node_id) {
            self.linking_from = None;
            self.linking_pointer_local = None;
        }
        if self.context_menu_node == Some(node_id) {
            self.context_menu_node = None;
        }
    }

    fn push_history(&mut self, entry: HistoryEntry, text: String) {
        self.undo_stack.push(entry);
        self.change_history.push(text);
    }

    fn record_move_history(&mut self, node_id: usize, from: Pos2, to: Pos2) {
        if from.distance(to) <= 0.1 {
            return;
        }

        self.push_history(
            HistoryEntry::MoveNode { node_id, from, to },
            format!(
                "移动节点 #{node_id}: ({:.0}, {:.0}) -> ({:.0}, {:.0})",
                from.x, from.y, to.x, to.y
            ),
        );
    }

    fn record_cut_history(&mut self, before_nodes: Vec<Node>, before_edges: Vec<(usize, usize)>) {
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

        let removed_node_count = removed_nodes.len();
        let removed_edge_count = removed_edges.len();

        self.push_history(
            HistoryEntry::DeleteBatch {
                nodes: removed_nodes,
                edges: removed_edges,
            },
            format!("删除内容: 节点 {} 个, 连线 {} 条", removed_node_count, removed_edge_count),
        );
    }

    fn undo_last_change(&mut self) {
        let Some(entry) = self.undo_stack.pop() else {
            return;
        };

        match entry {
            HistoryEntry::DeleteBatch { nodes, edges } => {
                let restored_nodes = nodes.len();
                let restored_edges = edges.len();

                for node in nodes {
                    if self.nodes.iter().any(|n| n.id == node.id) {
                        continue;
                    }
                    if node.id >= self.next_node_id {
                        self.next_node_id = node.id + 1;
                    }
                    self.nodes.push(node);
                }

                self.nodes.sort_by_key(|n| n.id);

                for (from, to) in edges {
                    if self.has_edge(from, to) {
                        continue;
                    }
                    let has_from = self.nodes.iter().any(|n| n.id == from);
                    let has_to = self.nodes.iter().any(|n| n.id == to);
                    if has_from && has_to {
                        self.edges.push((from, to));
                    }
                }

                self.change_history.push(format!(
                    "撤销删除: 恢复节点 {} 个, 连线 {} 条",
                    restored_nodes, restored_edges
                ));
            }
            HistoryEntry::MoveNode { node_id, from, to } => {
                if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
                    node.pos = from;
                    self.change_history.push(format!(
                        "撤销移动节点 #{node_id}: ({:.0}, {:.0}) <- ({:.0}, {:.0})",
                        from.x, from.y, to.x, to.y
                    ));
                }
            }
        }
    }

    fn segment_intersects_rect(a: Pos2, b: Pos2, rect: Rect) -> bool {
        if rect.contains(a) || rect.contains(b) {
            return true;
        }

        let lt = rect.left_top();
        let rt = rect.right_top();
        let rb = rect.right_bottom();
        let lb = rect.left_bottom();

        Self::segments_intersect(a, b, lt, rt)
            || Self::segments_intersect(a, b, rt, rb)
            || Self::segments_intersect(a, b, rb, lb)
            || Self::segments_intersect(a, b, lb, lt)
    }

    fn segments_intersect(a1: Pos2, a2: Pos2, b1: Pos2, b2: Pos2) -> bool {
        const EPS: f32 = 0.0001;

        fn cross(a: Pos2, b: Pos2, c: Pos2) -> f32 {
            (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
        }

        fn within(a: f32, b: f32, x: f32, eps: f32) -> bool {
            x >= a.min(b) - eps && x <= a.max(b) + eps
        }

        fn on_segment(a: Pos2, b: Pos2, p: Pos2, eps: f32) -> bool {
            within(a.x, b.x, p.x, eps) && within(a.y, b.y, p.y, eps)
        }

        let d1 = cross(a1, a2, b1);
        let d2 = cross(a1, a2, b2);
        let d3 = cross(b1, b2, a1);
        let d4 = cross(b1, b2, a2);

        if (d1 > EPS && d2 < -EPS || d1 < -EPS && d2 > EPS)
            && (d3 > EPS && d4 < -EPS || d3 < -EPS && d4 > EPS)
        {
            return true;
        }

        (d1.abs() <= EPS && on_segment(a1, a2, b1, EPS))
            || (d2.abs() <= EPS && on_segment(a1, a2, b2, EPS))
            || (d3.abs() <= EPS && on_segment(b1, b2, a1, EPS))
            || (d4.abs() <= EPS && on_segment(b1, b2, a2, EPS))
    }

    fn paint_grid(&self, painter: &egui::Painter, rect: Rect, pan: egui::Vec2, zoom: f32) {
        let base_spacing = 32.0;
        let spacing = base_spacing * zoom;
        let color = egui::Color32::from_rgba_premultiplied(100, 110, 130, 25);

        let x_offset = pan.x.rem_euclid(spacing);
        let y_offset = pan.y.rem_euclid(spacing);

        let mut x = rect.left() + x_offset - spacing;
        while x <= rect.right() + spacing {
            painter.line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                Stroke::new(1.0, color),
            );
            x += spacing;
        }

        let mut y = rect.top() + y_offset - spacing;
        while y <= rect.bottom() + spacing {
            painter.line_segment(
                [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                Stroke::new(1.0, color),
            );
            y += spacing;
        }
    }
}

impl eframe::App for GraphApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_terminal_events();

        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z)) {
            self.undo_last_change();
        }

        if let Some(terminal_id) = self.selected_terminal_id() {
            self.ensure_terminal(terminal_id, ctx);
        }

        SidePanel::right("data_panel")
            .resizable(true)
            .default_width(360.0)
            .min_width(300.0)
            .show(ctx, |ui| {
                if self.selected_terminal_id().is_some() {
                    self.draw_terminal_hint_panel(ui, ctx);
                } else {
                    self.draw_service_panel(ui);
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.draw_canvas(ui, ctx);
        });

        ctx.request_repaint();
    }
}

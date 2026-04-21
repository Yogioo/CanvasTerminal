mod ui;

use crate::constants::{TERMINAL_HEADER_HEIGHT, TERMINAL_NODE_ID};
use crate::model::{Node, NodeKind};
use crate::shell::system_shell;
use eframe::egui::{self, vec2, Pos2, Rect, SidePanel, Stroke};
use egui_term::{BackendSettings, PtyEvent, TerminalBackend};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc;

pub struct GraphApp {
    nodes: Vec<Node>,
    edges: Vec<(usize, usize)>,
    selected: Option<usize>,
    dragging: Option<(usize, egui::Vec2)>,
    pan: egui::Vec2,

    terminal_backends: HashMap<usize, TerminalBackend>,
    pty_rx: mpsc::Receiver<(u64, PtyEvent)>,
    pty_tx: mpsc::Sender<(u64, PtyEvent)>,
    terminal_exited: HashSet<usize>,
    terminal_errors: HashMap<usize, String>,

    next_node_id: usize,
    menu_search_text: String,
    pending_menu_search_focus: bool,
    editing_text_node: Option<usize>,
    pending_text_focus: Option<usize>,
    context_menu_node: Option<usize>,
    context_menu_local_pos: Option<Pos2>,
}

impl GraphApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (pty_tx, pty_rx) = mpsc::channel();

        let nodes = vec![
            Node {
                id: 1,
                title: "API Gateway".to_owned(),
                kind: NodeKind::Service,
                category: "服务".to_owned(),
                text_body: String::new(),
                pos: Pos2::new(80.0, 120.0),
                size: vec2(180.0, 90.0),
                status: "Healthy",
                latency_ms: 22,
                qps: 2350.0,
                errors: 0.10,
            },
            Node {
                id: 2,
                title: "Auth Service".to_owned(),
                kind: NodeKind::Service,
                category: "服务".to_owned(),
                text_body: String::new(),
                pos: Pos2::new(360.0, 70.0),
                size: vec2(180.0, 90.0),
                status: "Healthy",
                latency_ms: 38,
                qps: 980.0,
                errors: 0.08,
            },
            Node {
                id: 3,
                title: "Order Service".to_owned(),
                kind: NodeKind::Service,
                category: "服务".to_owned(),
                text_body: String::new(),
                pos: Pos2::new(360.0, 240.0),
                size: vec2(180.0, 90.0),
                status: "Warning",
                latency_ms: 85,
                qps: 1240.0,
                errors: 0.52,
            },
            Node {
                id: 4,
                title: "Redis Cache".to_owned(),
                kind: NodeKind::Service,
                category: "服务".to_owned(),
                text_body: String::new(),
                pos: Pos2::new(660.0, 70.0),
                size: vec2(180.0, 90.0),
                status: "Healthy",
                latency_ms: 6,
                qps: 7120.0,
                errors: 0.02,
            },
            Node {
                id: 5,
                title: "MySQL".to_owned(),
                kind: NodeKind::Service,
                category: "服务".to_owned(),
                text_body: String::new(),
                pos: Pos2::new(660.0, 240.0),
                size: vec2(180.0, 90.0),
                status: "Degraded",
                latency_ms: 140,
                qps: 845.0,
                errors: 1.74,
            },
            Node {
                id: TERMINAL_NODE_ID,
                title: "Terminal".to_owned(),
                kind: NodeKind::Terminal,
                category: "终端".to_owned(),
                text_body: String::new(),
                pos: Pos2::new(900.0, 120.0),
                size: vec2(760.0, 360.0),
                status: "Running",
                latency_ms: 0,
                qps: 0.0,
                errors: 0.0,
            },
        ];

        let mut app = Self {
            nodes,
            edges: vec![(1, 2), (1, 3), (2, 4), (3, 4), (3, 5), (1, TERMINAL_NODE_ID)],
            selected: Some(TERMINAL_NODE_ID),
            dragging: None,
            pan: vec2(0.0, 0.0),
            terminal_backends: HashMap::new(),
            pty_rx,
            pty_tx,
            terminal_exited: HashSet::new(),
            terminal_errors: HashMap::new(),
            next_node_id: TERMINAL_NODE_ID + 1,
            menu_search_text: String::new(),
            pending_menu_search_focus: false,
            editing_text_node: None,
            pending_text_focus: None,
            context_menu_node: None,
            context_menu_local_pos: None,
        };

        app.ensure_terminal(TERMINAL_NODE_ID, &cc.egui_ctx);
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
            latency_ms: 0,
            qps: 0.0,
            errors: 0.0,
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
            latency_ms: 0,
            qps: 0.0,
            errors: 0.0,
        });
        self.selected = Some(id);
        if edit_now {
            self.editing_text_node = Some(id);
            self.pending_text_focus = Some(id);
        }
    }

    fn node_kind_name(kind: &NodeKind) -> &'static str {
        match kind {
            NodeKind::Service => "服务",
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
                NodeKind::Service | NodeKind::Text => true,
                NodeKind::Terminal => local.y <= n.pos.y + TERMINAL_HEADER_HEIGHT,
            };

            return Some((n.id, n.pos.to_vec2(), can_drag));
        }
        None
    }

    fn terminal_content_rect_screen(&self, node_id: usize, canvas_rect: Rect) -> Option<Rect> {
        let n = self.nodes.iter().find(|n| n.id == node_id)?;
        if !matches!(n.kind, NodeKind::Terminal) {
            return None;
        }

        let outer = Rect::from_min_size(canvas_rect.min + self.pan + n.pos.to_vec2(), n.size);
        Some(Rect::from_min_max(
            outer.min + vec2(2.0, TERMINAL_HEADER_HEIGHT + 2.0),
            outer.max - vec2(2.0, 2.0),
        ))
    }

    fn terminal_content_rects_screen(&self, canvas_rect: Rect) -> Vec<(usize, Rect)> {
        self.nodes
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::Terminal))
            .filter_map(|n| self.terminal_content_rect_screen(n.id, canvas_rect).map(|r| (n.id, r)))
            .collect()
    }

    fn paint_grid(&self, painter: &egui::Painter, rect: Rect, pan: egui::Vec2) {
        let spacing = 32.0;
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

mod ui;

use crate::constants::{TERMINAL_HEADER_HEIGHT, TERMINAL_NODE_ID};
use crate::model::{Node, NodeKind};
use crate::shell::system_shell;
use eframe::egui::{self, vec2, Pos2, Rect, SidePanel, Stroke};
use egui_term::{BackendSettings, PtyEvent, TerminalBackend};
use std::sync::mpsc;

pub struct GraphApp {
    nodes: Vec<Node>,
    edges: Vec<(usize, usize)>,
    selected: Option<usize>,
    dragging: Option<(usize, egui::Vec2)>,
    pan: egui::Vec2,

    terminal_backend: Option<TerminalBackend>,
    pty_rx: mpsc::Receiver<(u64, PtyEvent)>,
    pty_tx: mpsc::Sender<(u64, PtyEvent)>,
    terminal_exited: bool,
    terminal_error: Option<String>,
}

impl GraphApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (pty_tx, pty_rx) = mpsc::channel();

        let nodes = vec![
            Node {
                id: 1,
                title: "API Gateway".to_owned(),
                kind: NodeKind::Service,
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
            terminal_backend: None,
            pty_rx,
            pty_tx,
            terminal_exited: false,
            terminal_error: None,
        };

        app.ensure_terminal(&cc.egui_ctx);
        app
    }

    fn ensure_terminal(&mut self, ctx: &egui::Context) {
        if self.terminal_backend.is_some() {
            return;
        }

        let shell = system_shell();
        match TerminalBackend::new(
            1,
            ctx.clone(),
            self.pty_tx.clone(),
            BackendSettings {
                shell,
                args: vec![],
                working_directory: std::env::current_dir().ok(),
            },
        ) {
            Ok(backend) => {
                self.terminal_backend = Some(backend);
                self.terminal_exited = false;
                self.terminal_error = None;
            }
            Err(e) => {
                self.terminal_error = Some(format!("终端启动失败: {e}"));
            }
        }
    }

    fn poll_terminal_events(&mut self) {
        while let Ok((_id, event)) = self.pty_rx.try_recv() {
            if let PtyEvent::Exit = event {
                self.terminal_exited = true;
                self.terminal_backend = None;
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
                NodeKind::Service => true,
                NodeKind::Terminal => local.y <= n.pos.y + TERMINAL_HEADER_HEIGHT,
            };

            return Some((n.id, n.pos.to_vec2(), can_drag));
        }
        None
    }

    fn terminal_content_rect_screen(&self, canvas_rect: Rect) -> Option<Rect> {
        let n = self.nodes.iter().find(|n| n.id == TERMINAL_NODE_ID)?;
        let outer = Rect::from_min_size(canvas_rect.min + self.pan + n.pos.to_vec2(), n.size);
        Some(Rect::from_min_max(
            outer.min + vec2(2.0, TERMINAL_HEADER_HEIGHT + 2.0),
            outer.max - vec2(2.0, 2.0),
        ))
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

        SidePanel::right("data_panel")
            .resizable(true)
            .default_width(340.0)
            .min_width(280.0)
            .show(ctx, |ui| {
                if self.selected == Some(TERMINAL_NODE_ID) {
                    self.ensure_terminal(ctx);
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

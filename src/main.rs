use eframe::egui::{
    self, vec2, Align2, Color32, FontId, Pos2, Rect, Sense, SidePanel, Stroke, Ui,
};
use egui_term::{BackendSettings, PtyEvent, TerminalBackend, TerminalFont, TerminalView};
use std::sync::mpsc;

const TERMINAL_NODE_ID: usize = 99;
const TERMINAL_HEADER_HEIGHT: f32 = 30.0;

fn setup_custom_fonts(ctx: &egui::Context) {
    // 关键点：终端必须优先用等宽字体；中文字体只做 fallback。
    let mono_candidates = [
        "C:/Windows/Fonts/CascadiaMono.ttf",
        "C:/Windows/Fonts/CascadiaCode.ttf",
        "C:/Windows/Fonts/consola.ttf",
        "C:/Windows/Fonts/consolas.ttf",
    ];

    let cjk_candidates = [
        "C:/Windows/Fonts/msyh.ttc",   // Microsoft YaHei
        "C:/Windows/Fonts/simhei.ttf", // SimHei
        "C:/Windows/Fonts/simsun.ttc", // SimSun
    ];

    let mut fonts = egui::FontDefinitions::default();

    let mut mono_loaded = None::<String>;
    for path in mono_candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let name = "term_mono".to_owned();
            fonts
                .font_data
                .insert(name.clone(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, name.clone());
            mono_loaded = Some(path.to_owned());
            break;
        }
    }

    let mut cjk_loaded = None::<String>;
    for path in cjk_candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let name = "cjk".to_owned();
            fonts
                .font_data
                .insert(name.clone(), egui::FontData::from_owned(bytes).into());

            // UI 文本优先中文字体，避免方块字。
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, name.clone());

            // 终端里中文作为 fallback，不要抢占等宽字体首位。
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push(name.clone());

            cjk_loaded = Some(path.to_owned());
            break;
        }
    }

    ctx.set_fonts(fonts);

    eprintln!(
        "Font setup => mono: {}, cjk: {}",
        mono_loaded.unwrap_or_else(|| "<default>".to_string()),
        cjk_loaded.unwrap_or_else(|| "<none>".to_string())
    );
}

fn system_shell() -> String {
    #[cfg(windows)]
    {
        "cmd.exe".to_owned()
    }

    #[cfg(not(windows))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_owned())
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1400.0, 820.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Node Graph MVP (egui terminal)",
        options,
        Box::new(|cc| {
            setup_custom_fonts(&cc.egui_ctx);
            Ok(Box::new(GraphApp::new(cc)))
        }),
    )
}

#[derive(Clone)]
enum NodeKind {
    Service,
    Terminal,
}

#[derive(Clone)]
struct Node {
    id: usize,
    title: String,
    kind: NodeKind,
    pos: Pos2,
    size: egui::Vec2,
    status: &'static str,
    latency_ms: u32,
    qps: f32,
    errors: f32,
}

struct GraphApp {
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
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
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

    fn draw_service_panel(&self, ui: &mut Ui) {
        ui.heading("节点数据面板（Mock）");
        ui.separator();

        if let Some(id) = self.selected {
            if let Some(node) = self.nodes.iter().find(|n| n.id == id) {
                ui.label(format!("节点名称: {}", node.title));
                ui.label(format!("节点 ID: {}", node.id));
                ui.label(format!("状态: {}", node.status));
                ui.separator();

                ui.label(format!("平均延迟: {} ms", node.latency_ms));
                ui.label(format!("吞吐量: {:.0} qps", node.qps));
                ui.label(format!("错误率: {:.2}%", node.errors));
                ui.separator();

                ui.label("最近 5 分钟（假数据）");
                ui.add(egui::ProgressBar::new((node.qps / 8000.0).clamp(0.0, 1.0)).text("流量负载"));
                ui.add(
                    egui::ProgressBar::new((node.latency_ms as f32 / 200.0).clamp(0.0, 1.0))
                        .text("延迟占比"),
                );
                ui.add(egui::ProgressBar::new((node.errors / 2.0).clamp(0.0, 1.0)).text("错误率占比"));

                ui.separator();
                ui.small("提示：点击节点切换数据；拖拽节点会更新连线；支持 Space+左键拖拽 或 中键拖拽 来平移画布。");
                ui.small("点击 Terminal 节点可获得接近系统终端的真实显示效果。 ");
            }
        }
    }

    fn draw_terminal_hint_panel(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.heading("Terminal 节点");
        ui.separator();
        ui.label("终端现在直接嵌入在画布中的 Terminal 节点内部。\n拖拽 Terminal 节点顶部可移动它。");

        if ui.button("重启终端").clicked() {
            self.terminal_backend = None;
            self.terminal_exited = false;
            self.ensure_terminal(ctx);
        }

        ui.separator();
        if self.terminal_backend.is_some() {
            ui.label(egui::RichText::new("● Running").color(Color32::LIGHT_GREEN));
        } else if self.terminal_exited {
            ui.label(egui::RichText::new("● Exited").color(Color32::LIGHT_RED));
        } else {
            ui.label(egui::RichText::new("● Starting...").color(Color32::YELLOW));
        }

        if let Some(err) = &self.terminal_error {
            ui.colored_label(Color32::LIGHT_RED, err);
        }
    }

    fn draw_canvas(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let available = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(available, Sense::click_and_drag());
        let painter = ui.painter_at(rect);

        painter.rect_filled(rect, 0.0, Color32::from_rgb(20, 22, 26));
        self.paint_grid(&painter, rect, self.pan);

        let is_space_down = ctx.input(|i| i.key_down(egui::Key::Space));
        let is_space_pan = ctx.input(|i| i.key_down(egui::Key::Space) && i.pointer.primary_down());
        let is_middle_pan = ctx.input(|i| i.pointer.middle_down());

        let terminal_content_rect = self.terminal_content_rect_screen(rect);
        let pointer_over_terminal_content = response
            .hover_pos()
            .is_some_and(|p| terminal_content_rect.is_some_and(|r| r.contains(p)));

        let is_panning = (is_space_pan || is_middle_pan) && response.hovered() && !pointer_over_terminal_content;

        if is_panning {
            self.dragging = None;
            let delta = ctx.input(|i| i.pointer.delta());
            self.pan += delta;
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);
        }

        if !is_panning && !pointer_over_terminal_content && response.drag_started() {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                let local = pointer_pos - rect.min - self.pan;
                if let Some((id, node_pos, can_drag)) = self.find_node_hit(local.to_pos2()) {
                    self.selected = Some(id);
                    if can_drag {
                        self.dragging = Some((id, local - node_pos));
                    }
                }
            }
        }

        if let Some((drag_id, offset)) = self.dragging {
            if ctx.input(|i| i.pointer.primary_down()) && !ctx.input(|i| i.key_down(egui::Key::Space)) {
                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    let local = pointer_pos - rect.min - self.pan;
                    if let Some(node) = self.nodes.iter_mut().find(|n| n.id == drag_id) {
                        node.pos = (local - offset).to_pos2();
                    }
                }
            } else {
                self.dragging = None;
            }
        }

        if !is_panning && !pointer_over_terminal_content && response.clicked() {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                let local = pointer_pos - rect.min - self.pan;
                if let Some((id, _)) = self.find_node_at(local.to_pos2()) {
                    self.selected = Some(id);
                }
            }
        }

        for (from, to) in &self.edges {
            if let (Some(a), Some(b)) = (
                self.nodes.iter().find(|n| n.id == *from),
                self.nodes.iter().find(|n| n.id == *to),
            ) {
                let start = rect.min + self.pan + a.pos.to_vec2() + vec2(a.size.x, a.size.y * 0.5);
                let end = rect.min + self.pan + b.pos.to_vec2() + vec2(0.0, b.size.y * 0.5);
                painter.line_segment([start, end], Stroke::new(2.0, Color32::from_rgb(110, 170, 255)));

                let dir = (end - start).normalized();
                let left = end - dir * 12.0 + vec2(-dir.y, dir.x) * 6.0;
                let right = end - dir * 12.0 + vec2(dir.y, -dir.x) * 6.0;
                painter.line_segment([left, end], Stroke::new(2.0, Color32::from_rgb(110, 170, 255)));
                painter.line_segment([right, end], Stroke::new(2.0, Color32::from_rgb(110, 170, 255)));
            }
        }

        for node in &self.nodes {
            let node_rect = Rect::from_min_size(rect.min + self.pan + node.pos.to_vec2(), node.size);
            let is_selected = self.selected == Some(node.id);

            let (fill, stroke) = match node.kind {
                NodeKind::Service => {
                    let fill = if is_selected {
                        Color32::from_rgb(40, 75, 130)
                    } else {
                        Color32::from_rgb(38, 43, 52)
                    };
                    let stroke = if is_selected {
                        Stroke::new(2.0, Color32::from_rgb(120, 180, 255))
                    } else {
                        Stroke::new(1.0, Color32::from_rgb(80, 90, 105))
                    };
                    (fill, stroke)
                }
                NodeKind::Terminal => {
                    let fill = if is_selected {
                        Color32::from_rgb(64, 52, 120)
                    } else {
                        Color32::from_rgb(48, 40, 86)
                    };
                    let stroke = if is_selected {
                        Stroke::new(2.0, Color32::from_rgb(174, 149, 255))
                    } else {
                        Stroke::new(1.0, Color32::from_rgb(108, 96, 145))
                    };
                    (fill, stroke)
                }
            };

            painter.rect(node_rect, 8.0, fill, stroke, egui::StrokeKind::Outside);
            painter.text(
                node_rect.left_top() + vec2(12.0, 12.0),
                Align2::LEFT_TOP,
                &node.title,
                FontId::proportional(18.0),
                Color32::WHITE,
            );

            match node.kind {
                NodeKind::Service => {
                    let status_color = match node.status {
                        "Healthy" => Color32::from_rgb(96, 212, 125),
                        "Warning" => Color32::from_rgb(255, 193, 88),
                        _ => Color32::from_rgb(255, 111, 111),
                    };
                    painter.circle_filled(node_rect.right_top() - vec2(14.0, -14.0), 5.0, status_color);
                    painter.text(
                        node_rect.left_bottom() - vec2(-12.0, 12.0),
                        Align2::LEFT_BOTTOM,
                        format!("{} ms", node.latency_ms),
                        FontId::proportional(14.0),
                        Color32::from_gray(210),
                    );
                }
                NodeKind::Terminal => {
                    let state_text = if self.terminal_backend.is_some() {
                        "状态: Running"
                    } else if self.terminal_exited {
                        "状态: Exited"
                    } else {
                        "状态: Starting"
                    };

                    painter.text(
                        node_rect.right_top() - vec2(12.0, -12.0),
                        Align2::RIGHT_TOP,
                        state_text,
                        FontId::proportional(13.0),
                        Color32::from_rgb(225, 220, 255),
                    );

                    // 标题栏与终端区分隔线
                    painter.line_segment(
                        [
                            node_rect.left_top() + vec2(0.0, TERMINAL_HEADER_HEIGHT),
                            node_rect.right_top() + vec2(0.0, TERMINAL_HEADER_HEIGHT),
                        ],
                        Stroke::new(1.0, Color32::from_rgb(108, 96, 145)),
                    );
                }
            }
        }

        // 在 Terminal 节点内部嵌入真实终端
        if let Some(term_rect) = terminal_content_rect {
            egui::Area::new(egui::Id::new("terminal_node_embedded_area"))
                .order(egui::Order::Foreground)
                .fixed_pos(term_rect.min)
                .show(ctx, |ui| {
                    ui.set_min_size(term_rect.size());
                    if let Some(err) = &self.terminal_error {
                        ui.colored_label(Color32::LIGHT_RED, err);
                    } else if let Some(backend) = self.terminal_backend.as_mut() {
                        let term = TerminalView::new(ui, backend)
                            .set_focus(self.selected == Some(TERMINAL_NODE_ID))
                            .set_font(TerminalFont::default())
                            .set_size(ui.available_size());
                        ui.add(term);
                    } else {
                        ui.label("终端未启动，请在右侧点击“重启终端”。");
                    }
                });
        }

        if !is_panning {
            if let Some(pos) = response.hover_pos() {
                let local = pos - rect.min - self.pan;
                if is_space_down && response.hovered() {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grab);
                } else if self.find_node_at(local.to_pos2()).is_some() {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                }
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
        let color = Color32::from_rgba_premultiplied(100, 110, 130, 25);

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

mod automation;
mod automation_support;
mod chrome;
mod clipboard;
mod dirty;
mod editing;
mod geometry;
mod groups;
mod history;

mod images;
mod menu;
mod nodes;
mod notifications;
mod performance;
mod persistence;
mod selection;
mod terminal;
mod ui;
mod workspace;

use self::workspace::WorkspaceState;

use crate::event_protocol::AppEvent;
use crate::event_server::start_event_server;
use crate::model::Node;
use eframe::egui::{self, Pos2, Vec2};
use egui_term::PtyEvent;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::mpsc;

#[derive(Clone, Copy)]
pub(in crate::app) enum NodeOrderAction {
    BringToFront,
    BringForwardOne,
    SendBackwardOne,
    SendToBack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum EdgeControlHandle {
    Source,
    Target,
}

#[derive(Clone, Copy, Debug, Default)]
pub(in crate::app) struct EdgeControlOffsets {
    pub source: egui::Vec2,
    pub target: egui::Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum DecisionColorInputMode {
    Rgb,
    Hsv,
}

#[derive(Clone, Debug, Default)]
pub(in crate::app) struct DecisionButtonDraft {
    pub label: String,
    pub event_key: String,
    pub color_rgb: [u8; 3],
    pub color_text: String,
}

#[derive(Clone)]
pub(in crate::app) struct NodeClipboardEdge {
    pub from: usize,
    pub to: usize,
    pub route_key: Option<String>,
    pub curve_bias: Option<f32>,
    pub control_offsets: Option<EdgeControlOffsets>,
}

#[derive(Clone)]
pub(in crate::app) struct NodeClipboardPayload {
    pub nodes: Vec<Node>,
    pub edges: Vec<NodeClipboardEdge>,
    pub anchor: Pos2,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct WindowState {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    maximized: bool,
    minimized: bool,
}

pub struct GraphApp {
    /// All resettable workspace state.
    pub(in crate::app) ws: WorkspaceState,

    // ── Channels (survive workspace reset) ──
    pty_rx: mpsc::Receiver<(u64, PtyEvent)>,
    pty_tx: mpsc::Sender<(u64, PtyEvent)>,
    event_rx: Option<mpsc::Receiver<AppEvent>>,
}

impl GraphApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let (pty_tx, pty_rx) = mpsc::channel();

        let event_rx = match start_event_server() {
            Ok(rx) => Some(rx),
            Err(err) => {
                eprintln!("failed to start event server: {err}");
                None
            }
        };

        let mut ws = WorkspaceState::default();
        ws.workspace_name = Self::default_workspace_name().to_owned();
        ws.pending_window_state_restore = Self::load_window_state_from_disk();
        ws.automation_state_timestamp_ms = crate::event_protocol::now_timestamp_ms();

        Self {
            ws,
            pty_rx,
            pty_tx,
            event_rx,
        }
    }

    fn alloc_node_id(&mut self) -> usize {
        let id = self.ws.next_node_id;
        self.ws.next_node_id += 1;
        id
    }

    pub(in crate::app) fn reset_workspace(&mut self) {
        let mut ws = WorkspaceState::default();
        ws.workspace_name = Self::default_workspace_name().to_owned();
        ws.automation_state_timestamp_ms = crate::event_protocol::now_timestamp_ms();
        self.ws = ws;
        self.mark_workspace_clean();
    }


    fn window_state_path() -> PathBuf {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let dir = PathBuf::from(appdata).join("CanvasTerminal");
            let _ = std::fs::create_dir_all(&dir);
            return dir.join("window-state.json");
        }

        PathBuf::from("./window-state.json")
    }

    fn load_window_state_from_disk() -> Option<WindowState> {
        let path = Self::window_state_path();
        let text = std::fs::read_to_string(path).ok()?;
        let state = serde_json::from_str::<WindowState>(&text).ok()?;
        if !state.width.is_finite()
            || !state.height.is_finite()
            || !state.x.is_finite()
            || !state.y.is_finite()
            || state.width < 320.0
            || state.height < 240.0
        {
            return None;
        }
        Some(state)
    }

    fn save_window_state_to_disk(state: &WindowState) {
        let path = Self::window_state_path();
        if let Ok(text) = serde_json::to_string_pretty(state) {
            let _ = std::fs::write(path, text);
        }
    }

    fn apply_pending_window_state_restore(&mut self, ctx: &egui::Context) {
        let Some(state) = self.ws.pending_window_state_restore.take() else {
            return;
        };

        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(Pos2::new(state.x, state.y)));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(
            state.width,
            state.height,
        )));
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(state.maximized));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(state.minimized));
    }

    fn persist_window_state_if_changed(&mut self, ctx: &egui::Context, now: f64) {
        if now - self.ws.last_window_state_persist_at < 0.25 {
            return;
        }

        let (outer_rect, inner_rect, maximized, minimized) = ctx.input(|i| {
            let vp = i.viewport();
            (
                vp.outer_rect,
                vp.inner_rect,
                vp.maximized.unwrap_or(false),
                vp.minimized.unwrap_or(false),
            )
        });
        let Some(outer_rect) = outer_rect else {
            return;
        };
        let Some(inner_rect) = inner_rect else {
            return;
        };

        let state = WindowState {
            x: outer_rect.min.x,
            y: outer_rect.min.y,
            width: inner_rect.width().max(320.0),
            height: inner_rect.height().max(240.0),
            maximized,
            minimized,
        };

        if self.ws.last_saved_window_state.as_ref() != Some(&state) {
            Self::save_window_state_to_disk(&state);
            self.ws.last_saved_window_state = Some(state);
        }

        self.ws.last_window_state_persist_at = now;
    }
}

impl eframe::App for GraphApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Color32::from_rgb(30, 30, 50).to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_terminal_events();
        self.poll_done_events();
        self.process_terminal_start_queue(ctx);

        let dt = ctx.input(|i| i.unstable_dt).max(0.0) as f64;
        self.script_before_frame();
        self.script_advance_timers(dt);

        self.handle_global_shortcuts(ctx);
        self.apply_pending_window_state_restore(ctx);
        self.apply_workspace_dirty_ui(ctx);
        self.ws.performance_metrics
            .update(Some(ctx.input(|i| i.unstable_dt).max(0.0)));

        let (now, screen_rect, pointer_near_top, show_window_bar) =
            self.update_window_bar_visibility(ctx);

        self.draw_window_controls_overlay(ctx, screen_rect, show_window_bar);

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(egui::Color32::from_rgb(30, 30, 50)))
            .show(ctx, |ui| {
                self.draw_canvas(ui, ctx, show_window_bar);
            });

        self.show_command_palette_if_open(ctx);
        self.show_workspace_dirty_indicator(ctx);
        self.show_toast_notifications(ctx);
        self.show_performance_overlay(ctx);
        self.script_after_frame();
        self.persist_window_state_if_changed(ctx, now);
        self.schedule_repaint(ctx, show_window_bar, pointer_near_top, now);
    }
}

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

use self::history::HistoryEntry;

use self::notifications::ToastNotification;
use self::performance::PerformanceMetrics;
use crate::event_protocol::{AppEvent, AutomationResponse};
use crate::event_server::start_event_server;
use crate::model::Node;
use crate::script_node::lua::LuaRuntime;
use eframe::egui::{self, vec2, Pos2, Rect, TextureHandle};
use egui_commonmark::CommonMarkCache;
use egui_term::{PtyEvent, TerminalBackend};
use std::collections::{HashMap, HashSet};
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

pub struct GraphApp {
    nodes: Vec<Node>,
    edges: Vec<(usize, usize)>,
    edge_route_keys: HashMap<(usize, usize), String>,
    edge_curve_biases: HashMap<(usize, usize), f32>,
    edge_control_offsets: HashMap<(usize, usize), EdgeControlOffsets>,
    selected: Option<usize>,
    selected_nodes: HashSet<usize>,
    selected_edge: Option<(usize, usize)>,
    dragging: Option<(usize, egui::Vec2)>,
    dragging_edge_control: Option<((usize, usize), EdgeControlHandle, egui::Vec2)>,
    drag_start_pos: Option<(usize, Pos2)>,
    drag_group_start: Option<(Pos2, Vec<(usize, Pos2)>)>,
    pan: egui::Vec2,
    zoom: f32,
    camera_world_center: Pos2,
    camera_initialized: bool,

    context_menu_open: bool,
    /// Actual screen rect of the context menu from the previous frame.
    last_context_menu_rect: Option<egui::Rect>,
    last_command_palette_rect: Option<egui::Rect>,

    terminal_backends: HashMap<usize, TerminalBackend>,
    pty_rx: mpsc::Receiver<(u64, PtyEvent)>,
    pty_tx: mpsc::Sender<(u64, PtyEvent)>,
    terminal_exited: HashSet<usize>,
    terminal_errors: HashMap<usize, String>,
    pending_terminal_injections: HashMap<usize, Vec<String>>,
    pending_terminal_starts: Vec<usize>,
    image_textures: HashMap<usize, TextureHandle>,
    image_errors: HashMap<usize, String>,
    image_bytes: HashMap<usize, Vec<u8>>,
    image_aspects: HashMap<usize, f32>,
    event_rx: Option<mpsc::Receiver<AppEvent>>,
    automation_state_version: u64,
    automation_state_timestamp_ms: u64,
    processed_automation_requests: HashMap<String, AutomationResponse>,

    next_node_id: usize,
    menu_search_text: String,
    menu_search_selected: usize,
    pending_menu_search_focus: bool,
    editing_text_node: Option<usize>,
    pending_text_focus: Option<usize>,
    editing_title_node: Option<usize>,
    pending_title_focus: Option<usize>,
    title_edit_buffer: String,
    editing_startup_node: Option<usize>,
    pending_startup_focus: Option<usize>,
    startup_edit_buffer: String,
    editing_working_directory_node: Option<usize>,
    pending_working_directory_focus: Option<usize>,
    working_directory_edit_buffer: String,
    editing_decision_buttons_node: Option<usize>,
    pending_decision_buttons_focus: Option<usize>,
    decision_buttons_edit_rows: Vec<DecisionButtonDraft>,
    decision_color_input_mode: DecisionColorInputMode,
    decision_color_popup: Option<(usize, usize)>,
    decision_color_popup_pos: Option<Pos2>,
    decision_buttons_edit_error: Option<String>,

    editing_decision_queue_node: Option<usize>,
    pending_decision_queue_focus: Option<usize>,
    decision_queue_edit_buffer: String,
    editing_edge: Option<(usize, usize)>,
    pending_edge_focus: Option<(usize, usize)>,
    edge_edit_buffer: String,

    // ── Script node editing ──
    editing_script_node: Option<usize>,
    pending_script_focus: Option<usize>,
    script_edit_buffer: String,

    // ── Script node queue editing ──
    editing_script_queue_node: Option<usize>,
    pending_script_queue_focus: Option<usize>,
    script_queue_edit_buffer: String,


    /// Per-node input port values (read from upstream edges, ephemeral)
    script_node_inputs: HashMap<usize, std::collections::HashMap<String, String>>,
    /// Per-node output port values (written by interactive widgets, ephemeral)
    script_node_outputs: HashMap<usize, std::collections::HashMap<String, String>>,
    /// Per-node persistent state (key-value, saved/loaded)
    script_node_state: HashMap<usize, std::collections::HashMap<String, String>>,
    /// Per-node Lua runtime (Script Node V2)
    script_lua_runtimes: HashMap<usize, LuaRuntime>,
    /// Per-node Lua timer accumulator in seconds
    script_lua_timer_accum: HashMap<usize, f64>,
    /// Next repaint requested by Lua timers (seconds)
    script_lua_next_repaint_after: Option<f64>,
    /// Per-node Lua runtime/render error text
    script_lua_errors: HashMap<usize, String>,
    /// Per-node Lua breakpoints (line based)
    script_lua_breakpoints: HashMap<usize, std::collections::HashSet<i32>>,
    /// Per-node latest debug pause line
    script_lua_pause_line: HashMap<usize, i32>,
    /// Per-node latest debug variable snapshot
    script_lua_debug_vars: HashMap<usize, String>,
    /// Per-node breakpoint line input buffer
    script_lua_breakpoint_input: HashMap<usize, String>,
    /// Id counter for script widget interaction IDs (resets each frame)
    script_widget_id_counter: u64,
    suspend_terminal_focus: Option<usize>,
    resizing: Option<(usize, Pos2, egui::Vec2)>,
    context_menu_node: Option<usize>,
    context_menu_edge: Option<(usize, usize)>,
    context_menu_local_pos: Option<Pos2>,
    linking_from: Option<usize>,
    linking_pointer_local: Option<Pos2>,
    cutting_path_local: Vec<Pos2>,
    right_drag_moved: bool,
    cut_snapshot_nodes: Option<Vec<Node>>,
    cut_snapshot_edges: Option<Vec<(usize, usize)>>,
    node_clipboard: Option<NodeClipboardPayload>,
    undo_stack: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
    last_primary_click_time: Option<f64>,
    last_primary_click_pos: Option<Pos2>,
    last_canvas_pointer_world_pos: Option<Pos2>,
    last_drag_hover_world_pos: Option<Pos2>,
    pending_dropped_files: Vec<egui::DroppedFile>,
    pending_drop_spawn_world_pos: Option<Pos2>,
    pending_drop_requested_at: Option<f64>,
    box_select_start: Option<Pos2>,
    box_select_current: Option<Pos2>,
    box_select_additive: bool,
    box_select_subtractive: bool,
    box_select_base_selection: HashSet<usize>,
    window_bar_visible_until: f64,
    command_palette_open: bool,
    active_graph_path: Option<PathBuf>,
    workspace_name: String,
    editing_workspace_name: bool,
    pending_workspace_name_focus: bool,
    workspace_name_edit_buffer: String,
    toast_notifications: Vec<ToastNotification>,
    next_toast_id: u64,
    workspace_dirty: bool,
    last_window_title: Option<String>,
    markdown_cache: CommonMarkCache,
    text_hide_zoom_threshold: f32,
    terminal_hide_zoom_threshold: f32,
    performance_metrics: PerformanceMetrics,
}

impl GraphApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let (pty_tx, pty_rx) = mpsc::channel();

        let nodes = Vec::new();
        let event_rx = match start_event_server() {
            Ok(rx) => Some(rx),
            Err(err) => {
                eprintln!("failed to start event server: {err}");
                None
            }
        };

        let app = Self {
            nodes,
            edges: Vec::new(),
            edge_route_keys: HashMap::new(),
            edge_curve_biases: HashMap::new(),
            edge_control_offsets: HashMap::new(),
            selected: None,
            selected_nodes: HashSet::new(),
            selected_edge: None,
            dragging: None,
            dragging_edge_control: None,
            drag_start_pos: None,
            drag_group_start: None,
            pan: vec2(0.0, 0.0),
            zoom: 1.0,
            camera_world_center: Pos2::new(0.0, 0.0),
            camera_initialized: false,
            context_menu_open: false,
            last_context_menu_rect: None,
            last_command_palette_rect: None,
            terminal_backends: HashMap::new(),
            pty_rx,
            pty_tx,
            terminal_exited: HashSet::new(),
            terminal_errors: HashMap::new(),
            pending_terminal_injections: HashMap::new(),
            pending_terminal_starts: Vec::new(),
            image_textures: HashMap::new(),
            image_errors: HashMap::new(),
            image_bytes: HashMap::new(),
            image_aspects: HashMap::new(),
            event_rx,
            automation_state_version: 1,
            automation_state_timestamp_ms: crate::event_protocol::now_timestamp_ms(),
            processed_automation_requests: HashMap::new(),
            next_node_id: 1,
            menu_search_text: String::new(),
            menu_search_selected: 0,
            pending_menu_search_focus: false,
            editing_text_node: None,
            pending_text_focus: None,
            editing_title_node: None,
            pending_title_focus: None,
            title_edit_buffer: String::new(),
            editing_startup_node: None,
            pending_startup_focus: None,
            startup_edit_buffer: String::new(),
            editing_working_directory_node: None,
            pending_working_directory_focus: None,
            working_directory_edit_buffer: String::new(),
            editing_decision_buttons_node: None,
            pending_decision_buttons_focus: None,
            decision_buttons_edit_rows: Vec::new(),

            decision_color_input_mode: DecisionColorInputMode::Rgb,
            decision_color_popup: None,
            decision_color_popup_pos: None,
            decision_buttons_edit_error: None,
            editing_decision_queue_node: None,
            pending_decision_queue_focus: None,
            decision_queue_edit_buffer: String::new(),
            editing_edge: None,
            pending_edge_focus: None,
            edge_edit_buffer: String::new(),

            editing_script_node: None,
            pending_script_focus: None,
            script_edit_buffer: String::new(),
            editing_script_queue_node: None,
            pending_script_queue_focus: None,
            script_queue_edit_buffer: String::new(),

            // Script node runtime state
            script_node_inputs: std::collections::HashMap::new(),
            script_node_outputs: std::collections::HashMap::new(),
            script_node_state: std::collections::HashMap::new(),
            script_lua_runtimes: std::collections::HashMap::new(),
            script_lua_timer_accum: std::collections::HashMap::new(),
            script_lua_next_repaint_after: None,
            script_lua_errors: std::collections::HashMap::new(),
            script_lua_breakpoints: std::collections::HashMap::new(),
            script_lua_pause_line: std::collections::HashMap::new(),
            script_lua_debug_vars: std::collections::HashMap::new(),
            script_lua_breakpoint_input: std::collections::HashMap::new(),
            script_widget_id_counter: 0,
            suspend_terminal_focus: None,
            resizing: None,
            context_menu_node: None,
            context_menu_edge: None,
            context_menu_local_pos: None,
            linking_from: None,
            linking_pointer_local: None,
            cutting_path_local: Vec::new(),
            right_drag_moved: false,
            cut_snapshot_nodes: None,
            cut_snapshot_edges: None,
            node_clipboard: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_primary_click_time: None,
            last_primary_click_pos: None,
            last_canvas_pointer_world_pos: None,
            last_drag_hover_world_pos: None,
            pending_dropped_files: Vec::new(),
            pending_drop_spawn_world_pos: None,
            pending_drop_requested_at: None,
            box_select_start: None,
            box_select_current: None,
            box_select_additive: false,
            box_select_subtractive: false,
            box_select_base_selection: HashSet::new(),
            window_bar_visible_until: 0.0,
            command_palette_open: false,
            active_graph_path: None,
            workspace_name: Self::default_workspace_name().to_owned(),
            editing_workspace_name: false,
            pending_workspace_name_focus: false,
            workspace_name_edit_buffer: String::new(),
            toast_notifications: Vec::new(),
            next_toast_id: 1,
            workspace_dirty: false,
            last_window_title: None,
            markdown_cache: CommonMarkCache::default(),
            text_hide_zoom_threshold: 0.55,
            terminal_hide_zoom_threshold: 0.3,
            performance_metrics: PerformanceMetrics::new(),
        };

        app
    }

    fn alloc_node_id(&mut self) -> usize {
        let id = self.next_node_id;
        self.next_node_id += 1;
        id
    }

    pub(in crate::app) fn reset_workspace(&mut self) {
        self.nodes.clear();
        self.edges.clear();
        self.edge_route_keys.clear();
        self.edge_curve_biases.clear();
        self.edge_control_offsets.clear();
        self.selected = None;
        self.selected_nodes.clear();
        self.selected_edge = None;
        self.dragging = None;
        self.dragging_edge_control = None;
        self.drag_start_pos = None;
        self.drag_group_start = None;
        self.pan = vec2(0.0, 0.0);
        self.zoom = 1.0;
        self.camera_world_center = Pos2::new(0.0, 0.0);
        self.camera_initialized = false;

        self.terminal_backends.clear();
        self.terminal_exited.clear();
        self.terminal_errors.clear();
        self.pending_terminal_injections.clear();
        self.pending_terminal_starts.clear();

        self.image_textures.clear();
        self.image_errors.clear();
        self.image_bytes.clear();
        self.image_aspects.clear();

        self.next_node_id = 1;
        self.menu_search_text.clear();
        self.menu_search_selected = 0;
        self.pending_menu_search_focus = false;
        self.editing_text_node = None;
        self.pending_text_focus = None;
        self.editing_title_node = None;
        self.pending_title_focus = None;
        self.title_edit_buffer.clear();
        self.editing_startup_node = None;
        self.pending_startup_focus = None;
        self.startup_edit_buffer.clear();
        self.editing_decision_buttons_node = None;
        self.pending_decision_buttons_focus = None;
        self.decision_buttons_edit_rows.clear();
        self.decision_color_popup = None;
        self.decision_color_popup_pos = None;
        self.decision_buttons_edit_error = None;
        self.editing_decision_queue_node = None;
        self.pending_decision_queue_focus = None;
        self.decision_queue_edit_buffer.clear();
        self.editing_edge = None;
        self.pending_edge_focus = None;
        self.edge_edit_buffer.clear();
        self.editing_script_node = None;
        self.pending_script_focus = None;
        self.script_edit_buffer.clear();
        self.editing_script_queue_node = None;
        self.pending_script_queue_focus = None;
        self.script_queue_edit_buffer.clear();

        self.script_node_inputs.clear();
        self.script_node_outputs.clear();
        self.script_node_state.clear();
        self.script_lua_runtimes.clear();
        self.script_lua_timer_accum.clear();
        self.script_lua_next_repaint_after = None;
        self.script_lua_errors.clear();
        self.script_widget_id_counter = 0;
        self.suspend_terminal_focus = None;
        self.resizing = None;
        self.context_menu_node = None;
        self.context_menu_edge = None;
        self.context_menu_local_pos = None;
        self.linking_from = None;
        self.linking_pointer_local = None;
        self.cutting_path_local.clear();
        self.right_drag_moved = false;
        self.cut_snapshot_nodes = None;
        self.cut_snapshot_edges = None;
        self.node_clipboard = None;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.last_primary_click_time = None;
        self.last_primary_click_pos = None;
        self.last_canvas_pointer_world_pos = None;
        self.last_drag_hover_world_pos = None;
        self.pending_dropped_files.clear();
        self.pending_drop_spawn_world_pos = None;
        self.pending_drop_requested_at = None;
        self.box_select_start = None;
        self.box_select_current = None;
        self.box_select_additive = false;
        self.box_select_subtractive = false;
        self.box_select_base_selection.clear();
        self.window_bar_visible_until = 0.0;
        self.command_palette_open = false;
        self.context_menu_open = false;
        self.last_context_menu_rect = None;
        self.last_command_palette_rect = None;

        self.active_graph_path = None;
        self.workspace_name = Self::default_workspace_name().to_owned();
        self.editing_workspace_name = false;
        self.pending_workspace_name_focus = false;
        self.workspace_name_edit_buffer.clear();
        self.mark_workspace_clean();
        self.bump_automation_state_version();
    }

    fn paint_grid(&self, _painter: &egui::Painter, _rect: Rect, _pan: egui::Vec2, _zoom: f32) {}
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
        self.apply_workspace_dirty_ui(ctx);
        self.performance_metrics
            .update(Some(ctx.input(|i| i.unstable_dt).max(0.0)));

        let (now, screen_rect, pointer_near_top, show_window_bar) =
            self.update_window_bar_visibility(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(egui::Color32::from_rgb(30, 30, 50)))
            .show(ctx, |ui| {
                self.draw_canvas(ui, ctx, show_window_bar);
            });

        if show_window_bar {
            self.draw_window_controls_overlay(ctx, screen_rect);
        }

        self.show_command_palette_if_open(ctx);
        self.show_workspace_dirty_indicator(ctx);
        self.show_toast_notifications(ctx);
        self.show_performance_overlay(ctx);
        self.script_after_frame();
        self.schedule_repaint(ctx, show_window_bar, pointer_near_top, now);
    }
}

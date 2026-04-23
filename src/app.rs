mod chrome;
mod editing;
mod geometry;
mod history;
mod images;
mod menu;
mod nodes;
mod persistence;
mod selection;
mod terminal;
mod ui;

use crate::event_protocol::DoneEvent;
use crate::event_server::start_done_event_server;
use crate::model::Node;
use self::history::HistoryEntry;
use eframe::egui::{self, vec2, Pos2, Rect, TextureHandle};
use egui_term::{PtyEvent, TerminalBackend};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc;

#[derive(Clone, Copy)]
pub(in crate::app) enum NodeOrderAction {
    BringToFront,
    BringForwardOne,
    SendBackwardOne,
    SendToBack,
}

pub struct GraphApp {
    nodes: Vec<Node>,
    edges: Vec<(usize, usize)>,
    selected: Option<usize>,
    selected_nodes: HashSet<usize>,
    dragging: Option<(usize, egui::Vec2)>,
    drag_start_pos: Option<(usize, Pos2)>,
    drag_group_start: Option<(Pos2, Vec<(usize, Pos2)>)>,
    pan: egui::Vec2,
    zoom: f32,

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
    done_event_rx: Option<mpsc::Receiver<DoneEvent>>,

    next_node_id: usize,
    menu_search_text: String,
    menu_search_selected: usize,
    pending_menu_search_focus: bool,
    editing_text_node: Option<usize>,
    pending_text_focus: Option<usize>,
    editing_title_node: Option<usize>,
    pending_title_focus: Option<usize>,
    title_edit_buffer: String,
    editing_identity_node: Option<usize>,
    pending_identity_focus: Option<usize>,
    identity_edit_buffer: String,
    editing_startup_node: Option<usize>,
    pending_startup_focus: Option<usize>,
    startup_edit_buffer: String,
    suspend_terminal_focus: Option<usize>,
    resizing: Option<(usize, Pos2, egui::Vec2)>,
    context_menu_node: Option<usize>,
    context_menu_local_pos: Option<Pos2>,
    linking_from: Option<usize>,
    linking_pointer_local: Option<Pos2>,
    cutting_path_local: Vec<Pos2>,
    right_drag_moved: bool,
    cut_snapshot_nodes: Option<Vec<Node>>,
    cut_snapshot_edges: Option<Vec<(usize, usize)>>,
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
}

impl GraphApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let (pty_tx, pty_rx) = mpsc::channel();

        let nodes = Vec::new();
        let done_event_rx = match start_done_event_server() {
            Ok(rx) => Some(rx),
            Err(err) => {
                eprintln!("failed to start done event server: {err}");
                None
            }
        };

        let app = Self {
            nodes,
            edges: Vec::new(),
            selected: None,
            selected_nodes: HashSet::new(),
            dragging: None,
            drag_start_pos: None,
            drag_group_start: None,
            pan: vec2(0.0, 0.0),
            zoom: 1.0,
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
            done_event_rx,
            next_node_id: 1,
            menu_search_text: String::new(),
            menu_search_selected: 0,
            pending_menu_search_focus: false,
            editing_text_node: None,
            pending_text_focus: None,
            editing_title_node: None,
            pending_title_focus: None,
            title_edit_buffer: String::new(),
            editing_identity_node: None,
            pending_identity_focus: None,
            identity_edit_buffer: String::new(),
            editing_startup_node: None,
            pending_startup_focus: None,
            startup_edit_buffer: String::new(),
            suspend_terminal_focus: None,
            resizing: None,
            context_menu_node: None,
            context_menu_local_pos: None,
            linking_from: None,
            linking_pointer_local: None,
            cutting_path_local: Vec::new(),
            right_drag_moved: false,
            cut_snapshot_nodes: None,
            cut_snapshot_edges: None,
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
        };

        app
    }

    fn alloc_node_id(&mut self) -> usize {
        let id = self.next_node_id;
        self.next_node_id += 1;
        id
    }

    fn paint_grid(&self, _painter: &egui::Painter, _rect: Rect, _pan: egui::Vec2, _zoom: f32) {
    }
}

impl eframe::App for GraphApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Color32::from_rgb(20, 20, 34).to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_terminal_events();
        self.poll_done_events();
        self.process_terminal_start_queue(ctx);

        self.handle_global_shortcuts(ctx);

        let (now, screen_rect, pointer_near_top, show_window_bar) =
            self.update_window_bar_visibility(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(egui::Color32::from_rgb(20, 20, 34)))
            .show(ctx, |ui| {
                self.draw_canvas(ui, ctx);
            });

        if show_window_bar {
            self.draw_window_controls_overlay(ctx, screen_rect);
        }

        self.show_command_palette_if_open(ctx);
        self.schedule_repaint(ctx, show_window_bar, pointer_near_top, now);
    }
}

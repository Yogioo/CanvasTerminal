use super::{
    history::HistoryEntry, performance::PerformanceMetrics, DecisionButtonDraft,
    DecisionColorInputMode, EdgeControlOffsets, NodeClipboardPayload,
};
use super::notifications::ToastNotification;
use crate::model::Node;
use crate::script_node::lua::LuaRuntime;
use eframe::egui::{self, Pos2, TextureHandle};
use egui_commonmark::CommonMarkCache;
use egui_term::TerminalBackend;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;


/// All workspace state that can be reset to default.
/// Kept as a single field on `GraphApp` so `reset_workspace()`
/// is a single assignment instead of 60+ individual field resets.
pub(in crate::app) struct WorkspaceState {
    // ── Nodes & edges ──
    pub(in crate::app) nodes: Vec<Node>,
    pub(in crate::app) edges: Vec<(usize, usize)>,
    pub(in crate::app) edge_route_keys: HashMap<(usize, usize), String>,
    pub(in crate::app) edge_curve_biases: HashMap<(usize, usize), f32>,
    pub(in crate::app) edge_control_offsets: HashMap<(usize, usize), EdgeControlOffsets>,

    // ── Selection ──
    pub(in crate::app) selected: Option<usize>,
    pub(in crate::app) selected_nodes: HashSet<usize>,
    pub(in crate::app) selected_edge: Option<(usize, usize)>,

    // ── Canvas / camera ──
    pub(in crate::app) pan: egui::Vec2,
    pub(in crate::app) zoom: f32,
    pub(in crate::app) camera_world_center: Pos2,
    pub(in crate::app) camera_initialized: bool,

    // ── Context menu & command palette ──
    pub(in crate::app) context_menu_open: bool,
    pub(in crate::app) last_context_menu_rect: Option<egui::Rect>,
    pub(in crate::app) last_command_palette_rect: Option<egui::Rect>,

    // ── Terminal ──
    pub(in crate::app) terminal_backends: HashMap<usize, TerminalBackend>,
    pub(in crate::app) terminal_exited: HashSet<usize>,
    pub(in crate::app) terminal_errors: HashMap<usize, String>,
    pub(in crate::app) pending_terminal_injections: HashMap<usize, Vec<String>>,
    pub(in crate::app) pending_terminal_starts: Vec<usize>,

    // ── Images ──
    pub(in crate::app) image_textures: HashMap<usize, TextureHandle>,
    pub(in crate::app) image_errors: HashMap<usize, String>,
    pub(in crate::app) image_bytes: HashMap<usize, Vec<u8>>,
    pub(in crate::app) image_aspects: HashMap<usize, f32>,

    // ── Script background textures ──
    pub(in crate::app) script_bg_textures: HashMap<String, TextureHandle>,
    pub(in crate::app) script_bg_texture_errors: HashMap<String, String>,

    // ── Automation ──
    pub(in crate::app) automation_state_version: u64,
    pub(in crate::app) automation_state_timestamp_ms: u64,
    pub(in crate::app) processed_automation_requests:
        HashMap<String, crate::event_protocol::AutomationResponse>,

    // ── Node id counter ──
    pub(in crate::app) next_node_id: usize,

    // ── Menu / search ──
    pub(in crate::app) menu_search_text: String,
    pub(in crate::app) menu_search_selected: usize,
    pub(in crate::app) pending_menu_search_focus: bool,

    // ── Node editors ──
    pub(in crate::app) editing_text_node: Option<usize>,
    pub(in crate::app) pending_text_focus: Option<usize>,
    pub(in crate::app) text_context_menu_selection: Option<(usize, std::ops::Range<usize>)>,
    pub(in crate::app) text_context_menu_screen_pos: Option<Pos2>,

    pub(in crate::app) editing_title_node: Option<usize>,
    pub(in crate::app) pending_title_focus: Option<usize>,
    pub(in crate::app) title_edit_buffer: String,

    pub(in crate::app) editing_startup_node: Option<usize>,
    pub(in crate::app) pending_startup_focus: Option<usize>,
    pub(in crate::app) startup_edit_buffer: String,

    pub(in crate::app) editing_working_directory_node: Option<usize>,
    pub(in crate::app) pending_working_directory_focus: Option<usize>,
    pub(in crate::app) working_directory_edit_buffer: String,

    pub(in crate::app) editing_decision_buttons_node: Option<usize>,
    pub(in crate::app) pending_decision_buttons_focus: Option<usize>,
    pub(in crate::app) decision_buttons_edit_rows: Vec<DecisionButtonDraft>,
    pub(in crate::app) decision_color_input_mode: DecisionColorInputMode,
    pub(in crate::app) decision_color_popup: Option<(usize, usize)>,
    pub(in crate::app) decision_color_popup_pos: Option<Pos2>,
    pub(in crate::app) decision_buttons_edit_error: Option<String>,

    pub(in crate::app) editing_decision_queue_node: Option<usize>,
    pub(in crate::app) pending_decision_queue_focus: Option<usize>,
    pub(in crate::app) decision_queue_edit_buffer: String,

    pub(in crate::app) editing_edge: Option<(usize, usize)>,
    pub(in crate::app) pending_edge_focus: Option<(usize, usize)>,
    pub(in crate::app) edge_edit_buffer: String,

    pub(in crate::app) editing_script_node: Option<usize>,
    pub(in crate::app) pending_script_focus: Option<usize>,
    pub(in crate::app) script_edit_buffer: String,
    pub(in crate::app) script_debug_node: Option<usize>,

    pub(in crate::app) editing_script_queue_node: Option<usize>,
    pub(in crate::app) pending_script_queue_focus: Option<usize>,
    pub(in crate::app) script_queue_edit_buffer: String,

    // ── Script node runtime ──
    pub(in crate::app) script_node_inputs: HashMap<usize, HashMap<String, String>>,
    pub(in crate::app) script_node_outputs: HashMap<usize, HashMap<String, String>>,
    pub(in crate::app) script_node_state: HashMap<usize, HashMap<String, String>>,
    pub(in crate::app) script_lua_runtimes: HashMap<usize, LuaRuntime>,
    pub(in crate::app) script_lua_timer_accum: HashMap<usize, f64>,
    pub(in crate::app) script_lua_next_repaint_after: Option<f64>,
    pub(in crate::app) script_lua_errors: HashMap<usize, String>,
    pub(in crate::app) script_lua_breakpoints: HashMap<usize, HashSet<i32>>,
    pub(in crate::app) script_lua_pause_line: HashMap<usize, i32>,
    pub(in crate::app) script_lua_debug_vars: HashMap<usize, String>,
    pub(in crate::app) script_lua_breakpoint_input: HashMap<usize, String>,


    // ── Interaction state ──
    pub(in crate::app) suspend_terminal_focus: Option<usize>,
    pub(in crate::app) resizing: Option<(usize, Pos2, egui::Vec2)>,
    pub(in crate::app) context_menu_node: Option<usize>,
    pub(in crate::app) context_menu_edge: Option<(usize, usize)>,
    pub(in crate::app) context_menu_local_pos: Option<Pos2>,
    pub(in crate::app) linking_from: Option<usize>,
    pub(in crate::app) linking_pointer_local: Option<Pos2>,
    pub(in crate::app) cutting_path_local: Vec<Pos2>,
    pub(in crate::app) right_drag_moved: bool,
    pub(in crate::app) cut_snapshot_nodes: Option<Vec<Node>>,
    pub(in crate::app) cut_snapshot_edges: Option<Vec<(usize, usize)>>,
    pub(in crate::app) node_clipboard: Option<NodeClipboardPayload>,
    pub(in crate::app) undo_stack: Vec<HistoryEntry>,
    pub(in crate::app) redo_stack: Vec<HistoryEntry>,
    pub(in crate::app) last_primary_click_time: Option<f64>,
    pub(in crate::app) last_primary_click_pos: Option<Pos2>,
    pub(in crate::app) last_canvas_pointer_world_pos: Option<Pos2>,
    pub(in crate::app) last_drag_hover_world_pos: Option<Pos2>,
    pub(in crate::app) pending_dropped_files: Vec<egui::DroppedFile>,
    pub(in crate::app) pending_drop_spawn_world_pos: Option<Pos2>,
    pub(in crate::app) pending_drop_requested_at: Option<f64>,
    pub(in crate::app) box_select_start: Option<Pos2>,
    pub(in crate::app) box_select_current: Option<Pos2>,
    pub(in crate::app) box_select_additive: bool,
    pub(in crate::app) box_select_subtractive: bool,
    pub(in crate::app) box_select_base_selection: HashSet<usize>,

    // ── Window / UI state ──
    pub(in crate::app) window_bar_visible_until: f64,
    pub(in crate::app) command_palette_open: bool,
    pub(in crate::app) active_graph_path: Option<PathBuf>,
    pub(in crate::app) workspace_name: String,
    pub(in crate::app) editing_workspace_name: bool,
    pub(in crate::app) pending_workspace_name_focus: bool,
    pub(in crate::app) workspace_name_edit_buffer: String,
    pub(in crate::app) toast_notifications: Vec<ToastNotification>,
    pub(in crate::app) next_toast_id: u64,
    pub(in crate::app) workspace_dirty: bool,
    pub(in crate::app) last_window_title: Option<String>,
    pub(in crate::app) text_hide_zoom_threshold: f32,
    pub(in crate::app) terminal_hide_zoom_threshold: f32,
    pub(in crate::app) pending_window_state_restore: Option<super::WindowState>,
    pub(in crate::app) last_saved_window_state: Option<super::WindowState>,
    pub(in crate::app) last_window_state_persist_at: f64,

    // ── Non-render caches ──
    pub(in crate::app) markdown_cache: CommonMarkCache,
    pub(in crate::app) performance_metrics: crate::app::performance::PerformanceMetrics,

    // ── Drag state ──
    pub(in crate::app) dragging: Option<(usize, egui::Vec2)>,
    pub(in crate::app) dragging_edge_control: Option<((usize, usize), super::EdgeControlHandle, egui::Vec2)>,
    pub(in crate::app) drag_start_pos: Option<(usize, Pos2)>,
    pub(in crate::app) drag_group_start: Option<(Pos2, Vec<(usize, Pos2)>)>,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            edge_route_keys: HashMap::new(),
            edge_curve_biases: HashMap::new(),
            edge_control_offsets: HashMap::new(),
            selected: None,
            selected_nodes: HashSet::new(),
            selected_edge: None,
            pan: egui::vec2(0.0, 0.0),
            zoom: 1.0,
            camera_world_center: Pos2::new(0.0, 0.0),
            camera_initialized: false,
            context_menu_open: false,
            last_context_menu_rect: None,
            last_command_palette_rect: None,
            terminal_backends: HashMap::new(),
            terminal_exited: HashSet::new(),
            terminal_errors: HashMap::new(),
            pending_terminal_injections: HashMap::new(),
            pending_terminal_starts: Vec::new(),
            image_textures: HashMap::new(),
            image_errors: HashMap::new(),
            image_bytes: HashMap::new(),
            image_aspects: HashMap::new(),
            script_bg_textures: HashMap::new(),
            script_bg_texture_errors: HashMap::new(),
            automation_state_version: 1,
            automation_state_timestamp_ms: crate::event_protocol::now_timestamp_ms(),
            processed_automation_requests: HashMap::new(),
            next_node_id: 1,
            menu_search_text: String::new(),
            menu_search_selected: 0,
            pending_menu_search_focus: false,
            editing_text_node: None,
            pending_text_focus: None,
            text_context_menu_selection: None,
            text_context_menu_screen_pos: None,
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
            script_debug_node: None,
            editing_script_queue_node: None,
            pending_script_queue_focus: None,
            script_queue_edit_buffer: String::new(),
            script_node_inputs: HashMap::new(),
            script_node_outputs: HashMap::new(),
            script_node_state: HashMap::new(),
            script_lua_runtimes: HashMap::new(),
            script_lua_timer_accum: HashMap::new(),
            script_lua_next_repaint_after: None,
            script_lua_errors: HashMap::new(),
            script_lua_breakpoints: HashMap::new(),
            script_lua_pause_line: HashMap::new(),
            script_lua_debug_vars: HashMap::new(),
            script_lua_breakpoint_input: HashMap::new(),
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
            workspace_name: String::new(),
            editing_workspace_name: false,
            pending_workspace_name_focus: false,
            workspace_name_edit_buffer: String::new(),
            toast_notifications: Vec::new(),
            next_toast_id: 1,
            workspace_dirty: false,
            last_window_title: None,
            text_hide_zoom_threshold: 0.55,
            terminal_hide_zoom_threshold: 0.3,
            pending_window_state_restore: None,
            last_saved_window_state: None,
            markdown_cache: CommonMarkCache::default(),
            performance_metrics: PerformanceMetrics::new(),
            last_window_state_persist_at: 0.0,
            dragging: None,
            dragging_edge_control: None,
            drag_start_pos: None,
            drag_group_start: None,
        }
    }
}

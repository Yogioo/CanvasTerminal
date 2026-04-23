mod persistence;
mod ui;

use crate::constants::TERMINAL_HEADER_HEIGHT;
use crate::event_protocol::DoneEvent;
use crate::event_server::start_done_event_server;
use crate::model::{Node, NodeKind};
use crate::shell::{system_shell, terminal_shell_args};
use chrono::Local;
use eframe::egui::{self, vec2, ColorImage, Pos2, Rect, TextureHandle, TextureOptions};
use egui_term::{BackendCommand, BackendSettings, PtyEvent, TerminalBackend};
use image::ImageReader;
use rfd::FileDialog;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

#[derive(Clone)]
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
    MoveNodes {
        nodes: Vec<(usize, Pos2, Pos2)>,
    },
    ReorderNodes {
        before: Vec<usize>,
    },
}

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

    fn set_single_selection(&mut self, node_id: usize) {
        self.selected = Some(node_id);
        self.selected_nodes.clear();
        self.selected_nodes.insert(node_id);
    }

    fn clear_selection(&mut self) {
        self.selected = None;
        self.selected_nodes.clear();
    }

    fn set_selection_from_option(&mut self, node_id: Option<usize>) {
        if let Some(id) = node_id {
            self.set_single_selection(id);
        } else {
            self.clear_selection();
        }
    }

    fn toggle_selection(&mut self, node_id: usize) {
        if self.selected_nodes.contains(&node_id) {
            self.selected_nodes.remove(&node_id);
            if self.selected == Some(node_id) {
                self.selected = self.selected_nodes.iter().copied().next();
            }
        } else {
            self.selected_nodes.insert(node_id);
            self.selected = Some(node_id);
        }
    }

    fn remove_from_selection(&mut self, node_id: usize) {
        self.selected_nodes.remove(&node_id);
        if self.selected == Some(node_id) {
            self.selected = self.selected_nodes.iter().copied().next();
        }
    }

    fn create_terminal_node(&mut self, pos: Pos2) {
        let id = self.alloc_node_id();
        self.nodes.push(Node {
            id,
            title: "Terminal".to_owned(),
            kind: NodeKind::Terminal,
            identity: format!("agent-{id}"),
            startup_script: String::new(),
            text_body: String::new(),
            image_path: String::new(),
            pos,
            size: vec2(840.0, 660.0),
        });
        self.set_single_selection(id);
    }

    fn create_text_node(&mut self, pos: Pos2, edit_now: bool) {
        let id = self.alloc_node_id();
        self.nodes.push(Node {
            id,
            title: format!("文本节点 {id}"),
            kind: NodeKind::Text,
            identity: String::new(),
            startup_script: String::new(),
            text_body: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            image_path: String::new(),
            pos,
            size: vec2(260.0, 140.0),
        });
        self.set_single_selection(id);
        if edit_now {
            self.editing_text_node = Some(id);
            self.pending_text_focus = Some(id);
        }
    }

    fn advance_spawn_pos_below_selected(&self, spawn_pos: &mut Pos2) {
        if let Some(id) = self.selected {
            if let Some(node) = self.nodes.iter().find(|n| n.id == id) {
                spawn_pos.y = node.pos.y + node.size.y + 16.0;
            }
        }
    }

    fn create_image_node_from_path(&mut self, pos: Pos2, image_path: String) {
        let id = self.alloc_node_id();

        let size = image::image_dimensions(&image_path)
            .ok()
            .filter(|(w, h)| *w > 0 && *h > 0)
            .map(|(w, h)| vec2(w as f32, h as f32))
            .unwrap_or(vec2(320.0, 220.0));

        if size.y > 0.0 {
            self.image_aspects.insert(id, size.x / size.y);
        }

        self.nodes.push(Node {
            id,
            title: String::new(),
            kind: NodeKind::Image,
            identity: String::new(),
            startup_script: String::new(),
            text_body: String::new(),
            image_path,
            pos,
            size,
        });
        self.set_single_selection(id);
    }

    fn create_image_node_from_bytes(&mut self, pos: Pos2, display_name: String, bytes: Vec<u8>) {
        match self.persist_image_bytes_to_artifact(&bytes) {
            Ok(relative_path) => {
                self.create_image_node_from_path(pos, relative_path);
            }
            Err(err) => {
                eprintln!("failed to persist dropped image bytes, fallback to in-memory image: {err}");

                let id = self.alloc_node_id();

                let mut size = vec2(320.0, 220.0);
                if let Ok(color_image) = Self::decode_image_bytes(&bytes) {
                    let [w, h] = color_image.size;
                    if w > 0 && h > 0 {
                        size = vec2(w as f32, h as f32);
                    }
                }

                self.nodes.push(Node {
                    id,
                    title: String::new(),
                    kind: NodeKind::Image,
                    identity: String::new(),
                    startup_script: String::new(),
                    text_body: String::new(),
                    image_path: display_name,
                    pos,
                    size,
                });
                self.image_bytes.insert(id, bytes);
                self.set_single_selection(id);
            }
        }
    }

    fn create_image_node_from_color_image(
        &mut self,
        pos: Pos2,
        display_name: String,
        color_image: ColorImage,
        ctx: &egui::Context,
    ) {
        match self.persist_clipboard_color_image(&color_image) {
            Ok(relative_path) => {
                self.create_image_node_from_path(pos, relative_path);
            }
            Err(err) => {
                eprintln!("failed to persist clipboard image, fallback to in-memory image: {err}");

                let id = self.alloc_node_id();
                self.nodes.push(Node {
                    id,
                    title: String::new(),
                    kind: NodeKind::Image,
                    identity: String::new(),
                    startup_script: String::new(),
                    text_body: String::new(),
                    image_path: display_name,
                    pos,
                    size: vec2(320.0, 220.0),
                });

                let [w, h] = color_image.size;
                let aspect = if h == 0 { 1.0 } else { w as f32 / h as f32 };

                if let Some(node) = self.nodes.iter_mut().find(|n| n.id == id) {
                    node.size = vec2(w as f32, h as f32);
                }

                let texture = ctx.load_texture(
                    format!("image-node-{id}"),
                    color_image,
                    TextureOptions::LINEAR,
                );
                self.image_textures.insert(id, texture);
                self.image_errors.remove(&id);
                self.image_bytes.remove(&id);
                self.image_aspects.insert(id, aspect);
                self.set_single_selection(id);
            }
        }
    }

    fn is_supported_image_path(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| {
                matches!(
                    ext.to_ascii_lowercase().as_str(),
                    "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
                )
            })
            .unwrap_or(false)
    }

    fn decode_image_bytes(bytes: &[u8]) -> Result<ColorImage, String> {
        let image = image::load_from_memory(bytes).map_err(|e| format!("图片解码失败: {e}"))?;
        let rgba = image.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let pixels = rgba.into_vec();
        Ok(ColorImage::from_rgba_unmultiplied(size, &pixels))
    }

    fn load_image_from_path(path: &str) -> Result<ColorImage, String> {
        let reader = ImageReader::open(path).map_err(|e| format!("无法读取图片: {e}"))?;
        let image = reader.decode().map_err(|e| format!("图片解码失败: {e}"))?;
        let rgba = image.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let pixels = rgba.into_vec();
        Ok(ColorImage::from_rgba_unmultiplied(size, &pixels))
    }

    pub(in crate::app) fn image_aspect(&self, node_id: usize) -> Option<f32> {
        self.image_aspects.get(&node_id).copied()
    }

    fn ensure_image_texture(&mut self, node_id: usize, ctx: &egui::Context) {
        if self.image_textures.contains_key(&node_id) || self.image_errors.contains_key(&node_id) {
            return;
        }

        let Some(node) = self
            .nodes
            .iter()
            .find(|n| n.id == node_id && n.kind == NodeKind::Image)
        else {
            return;
        };

        let image_path = node.image_path.clone();
        let image = if let Some(bytes) = self.image_bytes.get(&node_id) {
            Self::decode_image_bytes(bytes)
        } else if image_path.trim().is_empty() {
            return;
        } else {
            Self::load_image_from_path(&image_path)
        };

        match image {
            Ok(color_image) => {
                let [w, h] = color_image.size;
                let aspect = if h == 0 { 1.0 } else { w as f32 / h as f32 };
                let texture = ctx.load_texture(
                    format!("image-node-{node_id}"),
                    color_image,
                    TextureOptions::LINEAR,
                );
                self.image_textures.insert(node_id, texture);
                self.image_errors.remove(&node_id);
                self.image_aspects.insert(node_id, aspect);

                if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
                    node.size = vec2(w as f32, h as f32);
                }
            }
            Err(err) => {
                self.image_errors.insert(node_id, err);
            }
        }
    }

    pub(in crate::app) fn ensure_image_textures(&mut self, ctx: &egui::Context) {
        let image_ids: Vec<usize> = self
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Image)
            .map(|n| n.id)
            .collect();
        for node_id in image_ids {
            self.ensure_image_texture(node_id, ctx);
        }
    }

    pub(in crate::app) fn image_texture(&self, node_id: usize) -> Option<&TextureHandle> {
        self.image_textures.get(&node_id)
    }

    pub(in crate::app) fn image_error(&self, node_id: usize) -> Option<&str> {
        self.image_errors.get(&node_id).map(String::as_str)
    }

    fn menu_item_matches(&self, label: &str) -> bool {
        let kw = self.menu_search_text.trim();
        if kw.is_empty() {
            return true;
        }

        label.to_lowercase().contains(&kw.to_lowercase())
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

    fn reset_menu_search_state(&mut self, request_focus: bool) {
        self.menu_search_text.clear();
        self.menu_search_selected = 0;
        self.pending_menu_search_focus = request_focus;
    }

    fn show_searchable_menu_actions(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        search_id: egui::Id,
        hint_text: &str,
        items: &[(&str, &str, usize)],
        empty_text: &str,
        footer_text: &str,
    ) -> Option<usize> {
        if self.pending_menu_search_focus {
            ui.memory_mut(|m| m.request_focus(search_id));
        }

        let search_resp = ui.add_sized(
            [ui.available_width(), 24.0],
            egui::TextEdit::singleline(&mut self.menu_search_text)
                .id(search_id)
                .hint_text(hint_text),
        );
        let search_has_focus = search_resp.has_focus() || ui.memory(|m| m.has_focus(search_id));
        if self.pending_menu_search_focus && search_has_focus {
            self.pending_menu_search_focus = false;
        }
        if search_resp.changed() {
            self.menu_search_selected = 0;
        }

        ui.separator();

        let mut matched = Vec::new();
        for (path, label, action_id) in items {
            if self.menu_item_matches(path) || self.menu_item_matches(label) {
                matched.push((*label, *action_id));
            }
        }

        if matched.is_empty() {
            ui.small(empty_text);
            return None;
        }

        if self.menu_search_selected >= matched.len() {
            self.menu_search_selected = matched.len().saturating_sub(1);
        }

        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
            self.menu_search_selected = (self.menu_search_selected + 1) % matched.len();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
            self.menu_search_selected =
                (self.menu_search_selected + matched.len() - 1) % matched.len();
        }

        let mut trigger_action = None;
        for (row, (label, action_id)) in matched.iter().enumerate() {
            let selected = row == self.menu_search_selected;
            let resp = ui.add_sized(
                [ui.available_width(), 24.0],
                egui::Button::new(self.menu_item_highlighted_label(label)).selected(selected),
            );
            if resp.hovered() {
                self.menu_search_selected = row;
            }
            if resp.clicked() {
                trigger_action = Some(*action_id);
            }
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            trigger_action = Some(matched[self.menu_search_selected].1);
        }

        ui.separator();
        ui.small(footer_text);

        trigger_action
    }

    fn should_close_popup(
        &self,
        ctx: &egui::Context,
        popup_rect: Option<Rect>,
        action_triggered: bool,
    ) -> bool {
        if action_triggered {
            return true;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            return true;
        }

        let Some(rect) = popup_rect else {
            return false;
        };

        ctx.input(|i| {
            if !i.pointer.any_pressed() {
                return false;
            }
            let Some(pos) = i.pointer.interact_pos() else {
                return false;
            };
            !rect.contains(pos)
        })
    }

    fn terminal_identity(&self, node_id: usize) -> String {
        self.nodes
            .iter()
            .find(|n| n.id == node_id)
            .map(|n| n.identity.trim())
            .filter(|identity| !identity.is_empty())
            .unwrap_or("agent")
            .to_owned()
    }

    fn terminal_startup_script(&self, node_id: usize) -> Option<String> {
        self.nodes
            .iter()
            .find(|n| n.id == node_id)
            .map(|n| n.startup_script.trim())
            .filter(|script| !script.is_empty())
            .map(ToOwned::to_owned)
    }

    fn inject_terminal_text(&mut self, node_id: usize, text: &str) {
        if let Some(backend) = self.terminal_backends.get_mut(&node_id) {
            backend.process_command(BackendCommand::Write(text.as_bytes().to_vec()));
        } else {
            self.pending_terminal_injections
                .entry(node_id)
                .or_default()
                .push(text.to_owned());
        }
    }

    fn run_terminal_startup_script(&mut self, node_id: usize) {
        let Some(script) = self.terminal_startup_script(node_id) else {
            return;
        };

        let command = format!("{script}\r\n");
        self.inject_terminal_text(node_id, &command);
    }

    fn poll_done_events(&mut self) {
        let mut queued = Vec::new();
        if let Some(rx) = &self.done_event_rx {
            while let Ok(event) = rx.try_recv() {
                queued.push(event);
            }
        }

        for event in queued {
            self.handle_done_event(event);
        }
    }

    fn handle_done_event(&mut self, event: DoneEvent) {
        let downstream: Vec<usize> = self
            .edges
            .iter()
            .filter_map(|(from, to)| (*from == event.node_id).then_some(*to))
            .collect();

        let injected = format!(
            "上游节点 {} 已完成：{}\r\n",
            event.identity, event.summary
        );

        for node_id in downstream {
            self.inject_terminal_text(node_id, &injected);
        }
    }

    fn ensure_terminal(&mut self, node_id: usize, ctx: &egui::Context) {
        if self.terminal_backends.contains_key(&node_id) {
            return;
        }

        let shell = system_shell();
        let identity = self.terminal_identity(node_id);
        match TerminalBackend::new(
            node_id as u64,
            ctx.clone(),
            self.pty_tx.clone(),
            BackendSettings {
                shell,
                args: terminal_shell_args(node_id, &identity),
                working_directory: std::env::current_dir().ok(),
            },
        ) {
            Ok(backend) => {
                self.terminal_backends.insert(node_id, backend);
                self.terminal_exited.remove(&node_id);
                self.terminal_errors.remove(&node_id);

                self.run_terminal_startup_script(node_id);

                if let Some(pending) = self.pending_terminal_injections.remove(&node_id) {
                    for text in pending {
                        self.inject_terminal_text(node_id, &text);
                    }
                }
            }
            Err(e) => {
                self.terminal_errors
                    .insert(node_id, format!("终端启动失败: {e}"));
            }
        }
    }

    fn queue_terminal_start(&mut self, node_id: usize) {
        if self.terminal_backends.contains_key(&node_id)
            || self.terminal_errors.contains_key(&node_id)
            || self.terminal_exited.contains(&node_id)
        {
            return;
        }

        let is_terminal_node = self
            .nodes
            .iter()
            .any(|n| n.id == node_id && matches!(n.kind, NodeKind::Terminal));
        if !is_terminal_node {
            return;
        }

        if !self.pending_terminal_starts.contains(&node_id) {
            self.pending_terminal_starts.push(node_id);
        }
    }

    fn process_terminal_start_queue(&mut self, ctx: &egui::Context) {
        const MAX_TERMINAL_STARTS_PER_FRAME: usize = 1;

        for _ in 0..MAX_TERMINAL_STARTS_PER_FRAME {
            if self.pending_terminal_starts.is_empty() {
                break;
            }

            let node_id = self.pending_terminal_starts.remove(0);
            self.ensure_terminal(node_id, ctx);
        }
    }

    fn restart_terminal(&mut self, node_id: usize, ctx: &egui::Context) {
        self.terminal_backends.remove(&node_id);
        self.terminal_exited.remove(&node_id);
        self.terminal_errors.remove(&node_id);
        self.pending_terminal_starts.retain(|id| *id != node_id);
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
                NodeKind::Text | NodeKind::Image => true,
                NodeKind::Terminal => local.y <= n.pos.y + TERMINAL_HEADER_HEIGHT,
            };

            return Some((n.id, n.pos.to_vec2(), can_drag));
        }
        None
    }

    fn find_terminal_identity_badge_at(&self, local: Pos2) -> Option<usize> {
        for n in self.nodes.iter().rev() {
            if n.kind != NodeKind::Terminal {
                continue;
            }
            if Self::terminal_identity_badge_world_rect(n).contains(local) {
                return Some(n.id);
            }
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

    fn node_world_rect(node: &Node) -> Rect {
        Rect::from_min_size(node.pos, node.size)
    }

    fn all_nodes_world_rect(&self) -> Option<Rect> {
        let mut iter = self.nodes.iter();
        let first = iter.next()?;
        let mut bounds = Self::node_world_rect(first);
        for node in iter {
            bounds = bounds.union(Self::node_world_rect(node));
        }
        Some(bounds)
    }

    fn focus_rect(&mut self, canvas_rect: Rect, target_world_rect: Rect) {
        let viewport_padding = 64.0;
        let view_w = (canvas_rect.width() - viewport_padding * 2.0).max(1.0);
        let view_h = (canvas_rect.height() - viewport_padding * 2.0).max(1.0);

        let target_w = target_world_rect.width().max(1.0);
        let target_h = target_world_rect.height().max(1.0);

        self.zoom = (view_w / target_w).min(view_h / target_h).clamp(0.35, 2.5);

        let target_center = target_world_rect.center();
        self.pan = canvas_rect.center() - canvas_rect.min - target_center.to_vec2() * self.zoom;
    }

    fn selected_nodes_world_rect(&self) -> Option<Rect> {
        let mut selected_nodes = self
            .nodes
            .iter()
            .filter(|n| self.selected_nodes.contains(&n.id));

        let first = selected_nodes.next()?;
        let mut bounds = Self::node_world_rect(first);
        for node in selected_nodes {
            bounds = bounds.union(Self::node_world_rect(node));
        }
        Some(bounds)
    }

    fn focus_selected_or_all(&mut self, canvas_rect: Rect) {
        let target = self
            .selected_nodes_world_rect()
            .or_else(|| self.all_nodes_world_rect());

        if let Some(target_world_rect) = target {
            self.focus_rect(canvas_rect, target_world_rect);
        }
    }

    fn terminal_identity_badge_world_rect(node: &Node) -> Rect {
        let height = 22.0;
        let width = node.size.x.clamp(120.0, 220.0);
        Rect::from_min_size(
            Pos2::new(node.pos.x + 10.0, node.pos.y - height - 8.0),
            vec2(width, height),
        )
    }

    pub(in crate::app) fn terminal_header_height_screen(&self) -> f32 {
        let zoom_scale = self.zoom;
        let title_font_size = (17.0 * zoom_scale).max(9.0);
        let status_font_size = (13.0 * zoom_scale).max(8.0);
        let title_required_height = 10.0 * zoom_scale + title_font_size + 2.0 * zoom_scale;
        let status_required_height = 12.0 * zoom_scale + status_font_size + 2.0 * zoom_scale;
        (TERMINAL_HEADER_HEIGHT * zoom_scale).max(title_required_height.max(status_required_height))
    }

    fn terminal_content_rect_screen(&self, node_id: usize, canvas_rect: Rect) -> Option<Rect> {
        let n = self.nodes.iter().find(|n| n.id == node_id)?;
        if !matches!(n.kind, NodeKind::Terminal) {
            return None;
        }

        let outer_world = Rect::from_min_size(n.pos, n.size);
        let outer_screen = self.world_to_screen_rect(canvas_rect, outer_world);
        let border = 2.0 * self.zoom;
        let header_height = self.terminal_header_height_screen();

        let inner_min = outer_screen.min + vec2(border, header_height + border);
        let inner_max = outer_screen.max - vec2(border, border);
        if inner_min.x >= inner_max.x || inner_min.y >= inner_max.y {
            return None;
        }

        Some(Rect::from_min_max(inner_min, inner_max))
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

    fn ordered_selected_ids(&self) -> Vec<usize> {
        self.nodes
            .iter()
            .filter(|n| self.selected_nodes.contains(&n.id))
            .map(|n| n.id)
            .collect()
    }

    fn selection_or_single(&self, node_id: usize) -> HashSet<usize> {
        if self.selected_nodes.contains(&node_id) && !self.selected_nodes.is_empty() {
            self.selected_nodes.clone()
        } else {
            let mut picked = HashSet::new();
            picked.insert(node_id);
            picked
        }
    }

    fn apply_node_order(&mut self, order: &[usize]) {
        let mut map: HashMap<usize, Node> = std::mem::take(&mut self.nodes)
            .into_iter()
            .map(|node| (node.id, node))
            .collect();

        let mut reordered = Vec::with_capacity(map.len());
        for id in order {
            if let Some(node) = map.remove(id) {
                reordered.push(node);
            }
        }
        reordered.extend(map.into_values());
        self.nodes = reordered;
    }

    fn record_reorder_history(&mut self, before: Vec<usize>) {
        let after: Vec<usize> = self.nodes.iter().map(|n| n.id).collect();
        if before == after {
            return;
        }

        self.push_history(HistoryEntry::ReorderNodes { before });
    }

    fn bring_selection_to_front(&mut self) {
        let selected = self.selected_nodes.clone();
        if selected.is_empty() {
            return;
        }

        let before: Vec<usize> = self.nodes.iter().map(|n| n.id).collect();
        self.nodes
            .sort_by_key(|node| usize::from(selected.contains(&node.id)));
        self.record_reorder_history(before);
        self.selected = self.ordered_selected_ids().last().copied();
    }

    fn send_selection_to_back(&mut self) {
        let selected = self.selected_nodes.clone();
        if selected.is_empty() {
            return;
        }

        let before: Vec<usize> = self.nodes.iter().map(|n| n.id).collect();
        self.nodes
            .sort_by_key(|node| usize::from(!selected.contains(&node.id)));
        self.record_reorder_history(before);
        self.selected = self.ordered_selected_ids().last().copied();
    }

    fn bring_selection_forward_one(&mut self) {
        if self.selected_nodes.is_empty() {
            return;
        }

        let before: Vec<usize> = self.nodes.iter().map(|n| n.id).collect();
        for idx in (0..self.nodes.len().saturating_sub(1)).rev() {
            let current_selected = self.selected_nodes.contains(&self.nodes[idx].id);
            let next_selected = self.selected_nodes.contains(&self.nodes[idx + 1].id);
            if current_selected && !next_selected {
                self.nodes.swap(idx, idx + 1);
            }
        }

        self.record_reorder_history(before);
        self.selected = self.ordered_selected_ids().last().copied();
    }

    fn send_selection_backward_one(&mut self) {
        if self.selected_nodes.is_empty() {
            return;
        }

        let before: Vec<usize> = self.nodes.iter().map(|n| n.id).collect();
        for idx in 1..self.nodes.len() {
            let current_selected = self.selected_nodes.contains(&self.nodes[idx].id);
            let prev_selected = self.selected_nodes.contains(&self.nodes[idx - 1].id);
            if current_selected && !prev_selected {
                self.nodes.swap(idx - 1, idx);
            }
        }

        self.record_reorder_history(before);
        self.selected = self.ordered_selected_ids().last().copied();
    }

    fn reorder_from_context(&mut self, node_id: usize, mode: NodeOrderAction) {
        let target_selection = self.selection_or_single(node_id);
        self.selected_nodes = target_selection;

        match mode {
            NodeOrderAction::BringToFront => self.bring_selection_to_front(),
            NodeOrderAction::BringForwardOne => self.bring_selection_forward_one(),
            NodeOrderAction::SendBackwardOne => self.send_selection_backward_one(),
            NodeOrderAction::SendToBack => self.send_selection_to_back(),
        }
    }

    fn remove_node(&mut self, node_id: usize) {
        self.nodes.retain(|n| n.id != node_id);
        self.edges.retain(|(from, to)| *from != node_id && *to != node_id);
        self.terminal_backends.remove(&node_id);
        self.terminal_exited.remove(&node_id);
        self.terminal_errors.remove(&node_id);
        self.pending_terminal_injections.remove(&node_id);
        self.pending_terminal_starts.retain(|id| *id != node_id);
        self.image_textures.remove(&node_id);
        self.image_errors.remove(&node_id);
        self.image_bytes.remove(&node_id);
        self.image_aspects.remove(&node_id);

        self.selected_nodes.remove(&node_id);
        if self.selected == Some(node_id) {
            self.selected = self.selected_nodes.iter().copied().next();
        }
        if self.dragging.is_some_and(|(id, _)| id == node_id) {
            self.dragging = None;
            self.drag_start_pos = None;
            self.drag_group_start = None;
        }
        if self
            .drag_group_start
            .as_ref()
            .is_some_and(|(_, nodes)| nodes.iter().any(|(id, _)| *id == node_id))
        {
            self.dragging = None;
            self.drag_start_pos = None;
            self.drag_group_start = None;
        }
        if self.resizing.is_some_and(|(id, _, _)| id == node_id) {
            self.resizing = None;
        }
        if self.editing_text_node == Some(node_id) {
            self.editing_text_node = None;
            self.pending_text_focus = None;
        }
        if self.editing_title_node == Some(node_id) {
            self.editing_title_node = None;
            self.pending_title_focus = None;
            self.title_edit_buffer.clear();
        }
        if self.editing_identity_node == Some(node_id) {
            self.editing_identity_node = None;
            self.pending_identity_focus = None;
            self.identity_edit_buffer.clear();
        }
        if self.editing_startup_node == Some(node_id) {
            self.editing_startup_node = None;
            self.pending_startup_focus = None;
            self.startup_edit_buffer.clear();
        }
        if self.suspend_terminal_focus == Some(node_id) {
            self.suspend_terminal_focus = None;
        }
        if self.linking_from == Some(node_id) {
            self.linking_from = None;
            self.linking_pointer_local = None;
        }
        if self.context_menu_node == Some(node_id) {
            self.context_menu_node = None;
        }
    }

    fn save_graph_with_dialog(&self) {
        let Some(path) = FileDialog::new()
            .add_filter("Graph JSON", &["json"])
            .set_file_name("graph.json")
            .save_file()
        else {
            return;
        };

        if let Err(err) = self.save_graph_to_path(&path) {
            eprintln!("save graph failed: {err}");
        }
    }

    fn load_graph_with_dialog(&mut self) {
        let Some(path) = FileDialog::new()
            .add_filter("Graph JSON", &["json"])
            .pick_file()
        else {
            return;
        };

        if let Err(err) = self.load_graph_from_path(&path) {
            eprintln!("load graph failed: {err}");
        }
    }

    fn run_file_menu_action(&mut self, action_id: usize) {
        match action_id {
            0 => {
                if let Err(err) = self.save_graph_to_default_path() {
                    eprintln!("save graph failed: {err}");
                }
            }
            1 => {
                if let Err(err) = self.load_graph_from_default_path() {
                    eprintln!("load graph failed: {err}");
                }
            }
            2 => self.save_graph_with_dialog(),
            3 => self.load_graph_with_dialog(),
            _ => {}
        }
    }

    fn run_edit_menu_action(&mut self, action_id: usize) {
        match action_id {
            0 => self.undo_last_change(),
            1 => self.redo_last_change(),
            _ => {}
        }
    }

    fn push_history(&mut self, entry: HistoryEntry) {
        self.undo_stack.push(entry);
        self.redo_stack.clear();
    }

    fn record_move_history(&mut self, node_id: usize, from: Pos2, to: Pos2) {
        if from.distance(to) <= 0.1 {
            return;
        }

        self.push_history(HistoryEntry::MoveNode { node_id, from, to });
    }

    fn record_nodes_move_history(&mut self, nodes: Vec<(usize, Pos2, Pos2)>) {
        let moved_nodes: Vec<(usize, Pos2, Pos2)> = nodes
            .into_iter()
            .filter(|(_, from, to)| from.distance(*to) > 0.1)
            .collect();

        if moved_nodes.is_empty() {
            return;
        }

        self.push_history(HistoryEntry::MoveNodes { nodes: moved_nodes });
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

        self.push_history(HistoryEntry::DeleteBatch {
            nodes: removed_nodes,
            edges: removed_edges,
        });
    }

    fn apply_history_entry(&mut self, entry: HistoryEntry) -> HistoryEntry {
        match entry {
            HistoryEntry::DeleteBatch { nodes, edges } => {
                for node in &nodes {
                    if self.nodes.iter().any(|n| n.id == node.id) {
                        continue;
                    }
                    if node.id >= self.next_node_id {
                        self.next_node_id = node.id + 1;
                    }
                    self.nodes.push(node.clone());
                }

                self.nodes.sort_by_key(|n| n.id);

                for (from, to) in &edges {
                    if self.has_edge(*from, *to) {
                        continue;
                    }
                    let has_from = self.nodes.iter().any(|n| n.id == *from);
                    let has_to = self.nodes.iter().any(|n| n.id == *to);
                    if has_from && has_to {
                        self.edges.push((*from, *to));
                    }
                }

                HistoryEntry::DeleteBatch { nodes, edges }
            }
            HistoryEntry::MoveNode { node_id, from, to } => {
                if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
                    node.pos = from;
                }
                HistoryEntry::MoveNode {
                    node_id,
                    from: to,
                    to: from,
                }
            }
            HistoryEntry::MoveNodes { nodes } => {
                let mut swapped = Vec::with_capacity(nodes.len());
                for (node_id, from, to) in nodes {
                    if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
                        node.pos = from;
                    }
                    swapped.push((node_id, to, from));
                }
                HistoryEntry::MoveNodes { nodes: swapped }
            }
            HistoryEntry::ReorderNodes { before } => {
                let current: Vec<usize> = self.nodes.iter().map(|n| n.id).collect();
                self.apply_node_order(&before);
                HistoryEntry::ReorderNodes { before: current }
            }
        }
    }

    fn redo_delete_batch(&mut self, nodes: &[Node], edges: &[(usize, usize)]) {
        for (from, to) in edges {
            self.edges.retain(|edge| edge != &(*from, *to));
        }

        for node in nodes {
            self.remove_node(node.id);
        }
    }

    fn undo_last_change(&mut self) {
        let Some(entry) = self.undo_stack.pop() else {
            return;
        };

        let redo_entry = match &entry {
            HistoryEntry::DeleteBatch { nodes, edges } => {
                let cloned = HistoryEntry::DeleteBatch {
                    nodes: nodes.clone(),
                    edges: edges.clone(),
                };
                self.apply_history_entry(cloned)
            }
            _ => self.apply_history_entry(entry),
        };

        self.redo_stack.push(redo_entry);
    }

    fn redo_last_change(&mut self) {
        let Some(entry) = self.redo_stack.pop() else {
            return;
        };

        let undo_entry = match &entry {
            HistoryEntry::DeleteBatch { nodes, edges } => {
                self.redo_delete_batch(nodes, edges);
                HistoryEntry::DeleteBatch {
                    nodes: nodes.clone(),
                    edges: edges.clone(),
                }
            }
            _ => self.apply_history_entry(entry),
        };

        self.undo_stack.push(undo_entry);
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

    fn start_title_edit(&mut self, node_id: usize) {
        let Some(title) = self
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .map(|n| n.title.clone())
        else {
            return;
        };

        self.set_single_selection(node_id);
        self.dragging = None;
        self.drag_start_pos = None;
        self.drag_group_start = None;
        self.resizing = None;
        self.editing_text_node = None;
        self.pending_text_focus = None;
        self.editing_identity_node = None;
        self.pending_identity_focus = None;
        self.identity_edit_buffer.clear();
        self.editing_startup_node = None;
        self.pending_startup_focus = None;
        self.startup_edit_buffer.clear();
        self.editing_title_node = Some(node_id);
        self.pending_title_focus = Some(node_id);
        self.title_edit_buffer = title;
    }

    fn commit_title_edit(&mut self, node_id: usize) {
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            let trimmed = self.title_edit_buffer.trim();
            if !trimmed.is_empty() {
                node.title = trimmed.to_owned();
            }
        }
        self.editing_title_node = None;
        self.pending_title_focus = None;
        self.title_edit_buffer.clear();
        self.suspend_terminal_focus = Some(node_id);
    }

    fn cancel_title_edit(&mut self) {
        let node_id = self.editing_title_node;
        self.editing_title_node = None;
        self.pending_title_focus = None;
        self.title_edit_buffer.clear();
        self.suspend_terminal_focus = node_id;
    }

    fn start_identity_edit(&mut self, node_id: usize) {
        let Some(identity) = self
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .map(|n| n.identity.clone())
        else {
            return;
        };

        self.set_single_selection(node_id);
        self.dragging = None;
        self.drag_start_pos = None;
        self.drag_group_start = None;
        self.resizing = None;
        self.editing_text_node = None;
        self.pending_text_focus = None;
        self.editing_title_node = None;
        self.pending_title_focus = None;
        self.title_edit_buffer.clear();
        self.editing_startup_node = None;
        self.pending_startup_focus = None;
        self.startup_edit_buffer.clear();
        self.editing_identity_node = Some(node_id);
        self.pending_identity_focus = Some(node_id);
        self.identity_edit_buffer = identity;
    }

    fn commit_identity_edit(&mut self, node_id: usize, ctx: &egui::Context) {
        let mut identity_changed = false;
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            let trimmed = self.identity_edit_buffer.trim();
            if !trimmed.is_empty() && node.identity != trimmed {
                node.identity = trimmed.to_owned();
                identity_changed = true;
            }
        }
        self.editing_identity_node = None;
        self.pending_identity_focus = None;
        self.identity_edit_buffer.clear();
        self.suspend_terminal_focus = Some(node_id);

        if identity_changed {
            self.restart_terminal(node_id, ctx);
        }
    }

    fn cancel_identity_edit(&mut self) {
        let node_id = self.editing_identity_node;
        self.editing_identity_node = None;
        self.pending_identity_focus = None;
        self.identity_edit_buffer.clear();
        self.suspend_terminal_focus = node_id;
    }

    fn start_startup_edit(&mut self, node_id: usize) {
        let Some(startup_script) = self
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .map(|n| n.startup_script.clone())
        else {
            return;
        };

        self.set_single_selection(node_id);
        self.dragging = None;
        self.drag_start_pos = None;
        self.drag_group_start = None;
        self.resizing = None;
        self.editing_text_node = None;
        self.pending_text_focus = None;
        self.editing_title_node = None;
        self.pending_title_focus = None;
        self.title_edit_buffer.clear();
        self.editing_identity_node = None;
        self.pending_identity_focus = None;
        self.identity_edit_buffer.clear();
        self.editing_startup_node = Some(node_id);
        self.pending_startup_focus = Some(node_id);
        self.startup_edit_buffer = startup_script;
    }

    fn commit_startup_edit(&mut self, node_id: usize, ctx: &egui::Context) {
        let mut changed = false;
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            let next = self.startup_edit_buffer.trim().to_owned();
            if node.startup_script != next {
                node.startup_script = next;
                changed = true;
            }
        }
        self.editing_startup_node = None;
        self.pending_startup_focus = None;
        self.startup_edit_buffer.clear();
        self.suspend_terminal_focus = Some(node_id);

        if changed {
            self.restart_terminal(node_id, ctx);
        }
    }

    fn paint_grid(&self, _painter: &egui::Painter, _rect: Rect, _pan: egui::Vec2, _zoom: f32) {
    }
}

impl eframe::App for GraphApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Color32::from_rgba_premultiplied(0, 0, 0, 30).to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_terminal_events();
        self.poll_done_events();
        self.process_terminal_start_queue(ctx);

        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::U)) {
            self.redo_last_change();
        } else if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z)) {
            self.undo_last_change();
        }

        if ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::S)) {
            self.run_file_menu_action(2);
        } else if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
            self.run_file_menu_action(0);
        }

        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::R)) {
            self.run_file_menu_action(1);
        }

        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O)) {
            self.run_file_menu_action(3);
        }

        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::P)) {
            self.command_palette_open = true;
            self.reset_menu_search_state(true);
        }


        let now = ctx.input(|i| i.time);
        let screen_rect = ctx.screen_rect();
        let pointer_near_top = ctx.input(|i| {
            i.pointer
                .latest_pos()
                .or_else(|| i.pointer.hover_pos())
                .is_some_and(|p| p.y <= screen_rect.top() + 32.0)
        });
        let any_popup_open = ctx.memory(|m| m.any_popup_open());
        let keep_top_bar = any_popup_open;

        if pointer_near_top || keep_top_bar {
            self.window_bar_visible_until = now + 1.0;
        }
        let show_window_bar = pointer_near_top || keep_top_bar || now <= self.window_bar_visible_until;

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(egui::Color32::TRANSPARENT))
            .show(ctx, |ui| {
                self.draw_canvas(ui, ctx);
            });

        if show_window_bar {
            egui::Area::new("window_drag_bar_overlay".into())
                .order(egui::Order::Foreground)
                .fixed_pos(screen_rect.min)
                .show(ctx, |ui| {
                    let bar_height = 28.0;
                    let (bar_rect, _) =
                        ui.allocate_exact_size(vec2(screen_rect.width(), bar_height), egui::Sense::hover());

                    let button_size = vec2(24.0, 18.0);
                    let button_gap = 6.0;
                    let right_pad = 12.0;
                    let top = bar_rect.center().y - button_size.y * 0.5;

                    let close_rect = Rect::from_min_size(
                        Pos2::new(bar_rect.right() - right_pad - button_size.x, top),
                        button_size,
                    );
                    let maxim_rect = close_rect.translate(vec2(-(button_size.x + button_gap), 0.0));
                    let minim_rect = maxim_rect.translate(vec2(-(button_size.x + button_gap), 0.0));

                    let drag_left = bar_rect.left() + 8.0;
                    let drag_right = (minim_rect.left() - 8.0).max(drag_left);
                    let drag_rect = Rect::from_min_max(
                        Pos2::new(drag_left, bar_rect.top()),
                        Pos2::new(drag_right, bar_rect.bottom()),
                    );

                    let drag_response = ui.interact(
                        drag_rect,
                        ui.id().with("window_drag_area"),
                        egui::Sense::click_and_drag(),
                    );
                    let start_drag =
                        ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
                    if drag_response.hovered() {
                        ui.output_mut(|o| {
                            o.cursor_icon = if drag_response.dragged() {
                                egui::CursorIcon::Grabbing
                            } else {
                                egui::CursorIcon::Grab
                            };
                        });
                    }
                    if drag_response.double_clicked() {
                        let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                    } else if drag_response.hovered() && start_drag {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }

                    let stroke = egui::Stroke::new(1.5, egui::Color32::WHITE);

                    let minim = ui.interact(
                        minim_rect,
                        ui.id().with("window_minimize_button"),
                        egui::Sense::click(),
                    );
                    let min_y = minim_rect.center().y;
                    ui.painter().line_segment(
                        [
                            Pos2::new(minim_rect.center().x - 5.0, min_y),
                            Pos2::new(minim_rect.center().x + 5.0, min_y),
                        ],
                        stroke,
                    );
                    if minim.clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }

                    let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                    let maxim = ui.interact(
                        maxim_rect,
                        ui.id().with("window_maximize_button"),
                        egui::Sense::click(),
                    );
                    if is_maximized {
                        let back = Rect::from_center_size(
                            maxim_rect.center() + vec2(-1.5, -1.5),
                            vec2(8.5, 7.5),
                        );
                        let front = Rect::from_center_size(
                            maxim_rect.center() + vec2(1.5, 1.5),
                            vec2(8.5, 7.5),
                        );
                        ui.painter()
                            .rect_stroke(back, 0.0, stroke, egui::StrokeKind::Inside);
                        ui.painter()
                            .rect_stroke(front, 0.0, stroke, egui::StrokeKind::Inside);
                    } else {
                        let square = Rect::from_center_size(maxim_rect.center(), vec2(9.0, 9.0));
                        ui.painter()
                            .rect_stroke(square, 0.0, stroke, egui::StrokeKind::Inside);
                    }
                    if maxim.clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                    }

                    let close = ui.interact(
                        close_rect,
                        ui.id().with("window_close_button"),
                        egui::Sense::click(),
                    );
                    ui.painter().line_segment(
                        [
                            close_rect.center() + vec2(-4.5, -4.5),
                            close_rect.center() + vec2(4.5, 4.5),
                        ],
                        stroke,
                    );
                    ui.painter().line_segment(
                        [
                            close_rect.center() + vec2(-4.5, 4.5),
                            close_rect.center() + vec2(4.5, -4.5),
                        ],
                        stroke,
                    );
                    if close.clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
        }
        if self.command_palette_open {
            let mut action_triggered = false;
            let palette_window = egui::Window::new("命令面板")
                .id(egui::Id::new("command_palette_window"))
                .title_bar(false)
                .collapsible(false)
                .resizable(false)
                .movable(false)
                .anchor(egui::Align2::CENTER_TOP, vec2(0.0, 40.0))
                .show(ctx, |ui| {
                    ui.set_min_width(460.0);

                    let palette_items = [
                        ("文件/快速保存", "文件/快速保存  Ctrl+S", 0usize),
                        ("文件/快速加载", "文件/快速加载  Ctrl+R", 1usize),
                        ("文件/另存为", "文件/另存为  Ctrl+Shift+S", 2usize),
                        ("文件/加载", "文件/加载  Ctrl+O", 3usize),
                        ("编辑/撤销", "编辑/撤销  Ctrl+Z", 100usize),
                        ("编辑/重做", "编辑/重做  Ctrl+U", 101usize),
                    ];

                    if let Some(action_id) = self.show_searchable_menu_actions(
                        ui,
                        ctx,
                        egui::Id::new("command_palette_search_input"),
                        "输入命令...", 
                        &palette_items,
                        "无匹配命令",
                        "Ctrl+P 打开，Esc 关闭，↑/↓ 选择，Enter 执行",
                    ) {
                        if action_id < 100 {
                            self.run_file_menu_action(action_id);
                        } else {
                            self.run_edit_menu_action(action_id - 100);
                        }
                        action_triggered = true;
                    }
                });

            let popup_rect = palette_window.as_ref().map(|window| window.response.rect);
            if self.should_close_popup(ctx, popup_rect, action_triggered) {
                self.command_palette_open = false;
                self.reset_menu_search_state(false);
            }
        }

        if show_window_bar && !pointer_near_top {
            let remaining = (self.window_bar_visible_until - now).max(0.0);
            if remaining > 0.0 {
                ctx.request_repaint_after(Duration::from_secs_f64(remaining.min(0.1)));
            }
        }

        if !self.pending_terminal_starts.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
}

use super::GraphApp;
use crate::model::{Node, NodeData, NodeKind};
use eframe::egui::{vec2, ColorImage, Pos2};
use image::{DynamicImage, ImageFormat, RgbaImage};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const GRAPH_CONFIG_VERSION: u32 = 3;
const DEFAULT_GRAPH_PATH: &str = "./graph.json";
const IMAGE_ARTIFACT_DIR: &str = "artifacts/img";
static IMAGE_FILE_SEQ: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GraphConfig {
    #[serde(default = "default_graph_config_version")]
    version: u32,
    #[serde(default)]
    nodes: Vec<NodeConfig>,
    #[serde(default)]
    edges: Vec<(usize, usize)>,
    #[serde(default)]
    view: ViewConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ViewConfig {
    #[serde(default)]
    pan_x: f32,
    #[serde(default)]
    pan_y: f32,
    #[serde(default = "default_zoom")]
    zoom: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NodeConfig {
    id: usize,
    uid: String,
    kind: NodeKind,
    data: NodeData,
    pos_x: f32,
    pos_y: f32,
    size_x: f32,
    size_y: f32,
}

fn default_graph_config_version() -> u32 {
    GRAPH_CONFIG_VERSION
}

fn default_zoom() -> f32 {
    1.0
}

impl Default for ViewConfig {
    fn default() -> Self {
        Self {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
        }
    }
}

impl GraphConfig {
    fn from_app(app: &GraphApp) -> Self {
        let nodes = app
            .nodes
            .iter()
            .map(|node| NodeConfig {
                id: node.id,
                uid: node.uid.clone(),
                kind: node.kind.clone(),
                data: node.data.clone(),
                pos_x: node.pos.x,
                pos_y: node.pos.y,
                size_x: node.size.x,
                size_y: node.size.y,
            })
            .collect();

        Self {
            version: GRAPH_CONFIG_VERSION,
            nodes,
            edges: app.edges.clone(),
            view: ViewConfig {
                pan_x: app.pan.x,
                pan_y: app.pan.y,
                zoom: app.zoom,
            },
        }
    }
}

impl GraphApp {
    fn quick_graph_path(&self) -> PathBuf {
        self.active_graph_path
            .clone()
            .unwrap_or_else(|| PathBuf::from(DEFAULT_GRAPH_PATH))
    }

    pub(in crate::app) fn save_graph_to_default_path(&mut self) -> Result<PathBuf, String> {
        let path = self.quick_graph_path();
        self.save_graph_to_path(&path)?;
        self.active_graph_path = Some(path.clone());
        self.mark_workspace_clean();
        Ok(path)
    }

    pub(in crate::app) fn load_graph_from_default_path(&mut self) -> Result<PathBuf, String> {
        let path = self.quick_graph_path();
        self.load_graph_from_path(&path)?;
        self.active_graph_path = Some(path.clone());
        Ok(path)
    }

    pub(in crate::app) fn save_graph_to_path(&self, path: &Path) -> Result<(), String> {
        let config = GraphConfig::from_app(self);
        let json = serde_json::to_string_pretty(&config)
            .map_err(|err| format!("配置序列化失败: {err}"))?;

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|err| format!("创建配置目录失败 ({}): {err}", parent.display()))?;
            }
        }

        fs::write(path, json).map_err(|err| format!("写入配置文件失败 ({}): {err}", path.display()))
    }

    pub(in crate::app) fn load_graph_from_path(&mut self, path: &Path) -> Result<(), String> {
        let json = fs::read_to_string(path)
            .map_err(|err| format!("读取配置文件失败 ({}): {err}", path.display()))?;
        let config: GraphConfig =
            serde_json::from_str(&json).map_err(|err| format!("配置解析失败: {err}"))?;
        self.apply_graph_config(config);
        self.mark_workspace_clean();
        Ok(())
    }

    fn kind_matches_data(kind: &NodeKind, data: &NodeData) -> bool {
        matches!(
            (kind, data),
            (NodeKind::Terminal, NodeData::Terminal { .. })
                | (NodeKind::Text, NodeData::Text { .. })
                | (NodeKind::Image, NodeData::Image { .. })
        )
    }

    fn apply_graph_config(&mut self, config: GraphConfig) {
        let mut seen_ids = HashSet::new();
        let mut seen_uids = HashSet::new();
        let mut nodes = Vec::with_capacity(config.nodes.len());

        for node in config.nodes {
            if !seen_ids.insert(node.id) || !seen_uids.insert(node.uid.clone()) {
                continue;
            }
            if !Self::kind_matches_data(&node.kind, &node.data) {
                continue;
            }

            nodes.push(Node {
                id: node.id,
                uid: node.uid,
                kind: node.kind,
                data: node.data,
                pos: Pos2::new(node.pos_x, node.pos_y),
                size: vec2(node.size_x.max(1.0), node.size_y.max(1.0)),
            });
        }

        let node_ids: HashSet<usize> = nodes.iter().map(|n| n.id).collect();
        let edges: Vec<(usize, usize)> = config
            .edges
            .into_iter()
            .filter(|(from, to)| node_ids.contains(from) && node_ids.contains(to))
            .collect();

        self.nodes = nodes;
        self.edges = edges;
        self.selected = None;
        self.selected_nodes.clear();
        self.dragging = None;
        self.drag_start_pos = None;
        self.drag_group_start = None;
        self.resizing = None;
        self.context_menu_node = None;
        self.context_menu_local_pos = None;
        self.linking_from = None;
        self.linking_pointer_local = None;
        self.cutting_path_local.clear();
        self.right_drag_moved = false;
        self.cut_snapshot_nodes = None;
        self.cut_snapshot_edges = None;
        self.undo_stack.clear();
        self.redo_stack.clear();

        self.editing_text_node = None;
        self.pending_text_focus = None;
        self.editing_title_node = None;
        self.pending_title_focus = None;
        self.title_edit_buffer.clear();
        self.editing_startup_node = None;
        self.pending_startup_focus = None;
        self.startup_edit_buffer.clear();
        self.suspend_terminal_focus = None;

        self.terminal_backends.clear();
        self.terminal_exited.clear();
        self.terminal_errors.clear();
        self.pending_terminal_injections.clear();
        self.pending_terminal_starts.clear();

        self.image_textures.clear();
        self.image_errors.clear();
        self.image_bytes.clear();
        self.image_aspects.clear();

        self.next_node_id = self
            .nodes
            .iter()
            .map(|n| n.id)
            .max()
            .map(|id| id + 1)
            .unwrap_or(1);

        self.pan = vec2(config.view.pan_x, config.view.pan_y);
        self.zoom = if config.view.zoom.is_finite() && config.view.zoom >= 1e-4 {
            config.view.zoom
        } else {
            1.0
        };
        self.camera_world_center = Pos2::new(0.0, 0.0);
        self.camera_initialized = false;

        for node in &self.nodes {
            if node.kind != NodeKind::Image || node.size.y <= 0.0 {
                continue;
            }
            self.image_aspects
                .insert(node.id, node.size.x / node.size.y);
        }

        self.bump_automation_state_version();
    }

    fn next_image_relative_png_path() -> String {
        let seq = IMAGE_FILE_SEQ.fetch_add(1, Ordering::Relaxed);
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S-%3f");
        format!("{IMAGE_ARTIFACT_DIR}/{stamp}-{seq}.png")
    }

    fn persist_color_image_to_artifact(&self, color_image: &ColorImage) -> Result<String, String> {
        let [w, h] = color_image.size;
        if w == 0 || h == 0 {
            return Err("图片尺寸无效".to_owned());
        }

        let mut rgba = Vec::with_capacity(w * h * 4);
        for pixel in &color_image.pixels {
            rgba.extend_from_slice(&pixel.to_array());
        }

        let image = RgbaImage::from_raw(w as u32, h as u32, rgba)
            .ok_or_else(|| "图片像素缓冲区无效".to_owned())?;

        let relative_path = Self::next_image_relative_png_path();
        let absolute_path = std::env::current_dir()
            .map_err(|err| format!("读取工作目录失败: {err}"))?
            .join(&relative_path);

        if let Some(parent) = absolute_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("创建图片目录失败 ({}): {err}", parent.display()))?;
        }

        DynamicImage::ImageRgba8(image)
            .save_with_format(&absolute_path, ImageFormat::Png)
            .map_err(|err| format!("保存图片失败 ({}): {err}", absolute_path.display()))?;

        Ok(relative_path)
    }

    pub(in crate::app) fn persist_clipboard_color_image(
        &self,
        color_image: &ColorImage,
    ) -> Result<String, String> {
        self.persist_color_image_to_artifact(color_image)
    }

    pub(in crate::app) fn persist_image_bytes_to_artifact(
        &self,
        bytes: &[u8],
    ) -> Result<String, String> {
        let color_image = Self::decode_image_bytes(bytes)?;
        self.persist_color_image_to_artifact(&color_image)
    }
}

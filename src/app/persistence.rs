use super::GraphApp;
use crate::model::{Node, NodeData, NodeKind};
use eframe::egui::{vec2, ColorImage, Pos2};
use image::{DynamicImage, ImageFormat, RgbaImage};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const GRAPH_CONFIG_VERSION: u32 = 10;
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
    edge_routes: Vec<EdgeRouteConfig>,
    #[serde(default)]
    edge_curve_biases: Vec<EdgeCurveBiasConfig>,
    #[serde(default)]
    edge_control_offsets: Vec<EdgeControlOffsetConfig>,
    #[serde(default)]
    view: ViewConfig,
    #[serde(default)]
    workspace_name: Option<String>,
    #[serde(default)]
    script_states: Vec<ScriptStateConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EdgeRouteConfig {
    from: usize,
    to: usize,
    #[serde(default)]
    route_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EdgeCurveBiasConfig {
    from: usize,
    to: usize,
    bias: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScriptStateConfig {
    node_id: usize,
    state: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EdgeControlOffsetConfig {
    from: usize,
    to: usize,
    source_dx: f32,
    source_dy: f32,
    target_dx: f32,
    target_dy: f32,
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

        let edge_routes = app
            .edge_route_keys
            .iter()
            .filter_map(|((from, to), route_key)| {
                let trimmed = route_key.trim();
                if trimmed.is_empty() || !app.has_edge(*from, *to) {
                    return None;
                }

                Some(EdgeRouteConfig {
                    from: *from,
                    to: *to,
                    route_key: trimmed.to_owned(),
                })
            })
            .collect();

        let edge_curve_biases = app
            .edge_curve_biases
            .iter()
            .filter_map(|((from, to), bias)| {
                if !app.has_edge(*from, *to) {
                    return None;
                }

                let clamped = GraphApp::clamp_edge_curve_bias(*bias);
                if clamped.abs() <= 0.001 {
                    return None;
                }

                Some(EdgeCurveBiasConfig {
                    from: *from,
                    to: *to,
                    bias: clamped,
                })
            })
            .collect();

        let edge_control_offsets = app
            .edge_control_offsets
            .iter()
            .filter_map(|((from, to), offsets)| {
                if !app.has_edge(*from, *to) {
                    return None;
                }

                let source = GraphApp::clamp_edge_control_offset(offsets.source);
                let target = GraphApp::clamp_edge_control_offset(offsets.target);
                if source.length_sq() <= 0.01 && target.length_sq() <= 0.01 {
                    return None;
                }

                Some(EdgeControlOffsetConfig {
                    from: *from,
                    to: *to,
                    source_dx: source.x,
                    source_dy: source.y,
                    target_dx: target.x,
                    target_dy: target.y,
                })
            })
            .collect();

        Self {
            version: GRAPH_CONFIG_VERSION,
            nodes,
            edges: app.edges.clone(),
            edge_routes,
            edge_curve_biases,
            edge_control_offsets,
            view: ViewConfig {
                pan_x: app.pan.x,
                pan_y: app.pan.y,
                zoom: app.zoom,
            },
            workspace_name: Some(app.workspace_name().to_owned()),
            script_states: app
                .script_node_state
                .iter()
                .map(|(id, state)| ScriptStateConfig {
                    node_id: *id,
                    state: state.clone(),
                })
                .collect(),
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
        self.apply_graph_config(config, Some(path));
        self.mark_workspace_clean();
        Ok(())
    }

    fn kind_matches_data(kind: &NodeKind, data: &NodeData) -> bool {
        matches!(
            (kind, data),
            (NodeKind::Terminal, NodeData::Terminal { .. })
                | (NodeKind::Text, NodeData::Text { .. })

                | (NodeKind::Image, NodeData::Image { .. })
                | (NodeKind::Decision, NodeData::Decision { .. })
                | (NodeKind::Group, NodeData::Group { .. })
                | (NodeKind::Script, NodeData::Script { .. })
        )
    }

    fn apply_graph_config(&mut self, config: GraphConfig, fallback_path: Option<&Path>) {
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

        let mut edge_route_keys = std::collections::HashMap::new();
        for route in config.edge_routes {
            let trimmed = route.route_key.trim();
            if trimmed.is_empty() {
                continue;
            }
            if edges
                .iter()
                .any(|(from, to)| *from == route.from && *to == route.to)
            {
                edge_route_keys.insert((route.from, route.to), trimmed.to_owned());
            }
        }

        let mut edge_curve_biases = std::collections::HashMap::new();
        for bias in config.edge_curve_biases {
            if !edges
                .iter()
                .any(|(from, to)| *from == bias.from && *to == bias.to)
            {
                continue;
            }

            let clamped = Self::clamp_edge_curve_bias(bias.bias);
            if clamped.abs() <= 0.001 {
                continue;
            }

            edge_curve_biases.insert((bias.from, bias.to), clamped);
        }

        let mut edge_control_offsets = std::collections::HashMap::new();
        for offsets in config.edge_control_offsets {
            if !edges
                .iter()
                .any(|(from, to)| *from == offsets.from && *to == offsets.to)
            {
                continue;
            }

            let source =
                Self::clamp_edge_control_offset(vec2(offsets.source_dx, offsets.source_dy));
            let target =
                Self::clamp_edge_control_offset(vec2(offsets.target_dx, offsets.target_dy));
            if source.length_sq() <= 0.01 && target.length_sq() <= 0.01 {
                continue;
            }

            edge_control_offsets.insert(
                (offsets.from, offsets.to),
                super::EdgeControlOffsets { source, target },
            );
        }
        let mut script_node_state = std::collections::HashMap::new();
        for s in config.script_states {
            if node_ids.contains(&s.node_id) {
                script_node_state.insert(s.node_id, s.state);
            }
        }

        self.nodes = nodes;
        self.sanitize_groups();
        self.edges = edges;
        self.edge_route_keys = edge_route_keys;
        self.edge_curve_biases = edge_curve_biases;
        self.edge_control_offsets = edge_control_offsets;
        self.script_node_state = script_node_state;
        self.selected = None;
        self.selected_nodes.clear();
        self.selected_edge = None;
        self.dragging = None;
        self.dragging_edge_control = None;
        self.drag_start_pos = None;
        self.drag_group_start = None;
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
        self.editing_working_directory_node = None;
        self.pending_working_directory_focus = None;
        self.working_directory_edit_buffer.clear();
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

        self.set_workspace_name(&Self::resolve_workspace_name(
            config.workspace_name.as_deref(),
            fallback_path,
        ));
        self.editing_workspace_name = false;
        self.pending_workspace_name_focus = false;
        self.workspace_name_edit_buffer.clear();

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::Path;

    fn sample_node(id: usize) -> NodeConfig {
        NodeConfig {
            id,
            uid: format!("u-{id}"),
            kind: NodeKind::Text,
            data: NodeData::Text {
                text_body: String::new(),
                auto_size: false,
            },
            pos_x: 0.0,
            pos_y: 0.0,
            size_x: 120.0,
            size_y: 80.0,
        }
    }

    #[test]
    fn workspace_name_roundtrip_is_preserved() {
        let config = GraphConfig {
            version: GRAPH_CONFIG_VERSION,
            nodes: vec![sample_node(1)],
            edges: vec![],
            edge_routes: vec![],
            edge_curve_biases: vec![],
            edge_control_offsets: vec![],
            view: ViewConfig::default(),
            workspace_name: Some("工作区C".to_owned()),
            script_states: vec![],
        };

        let text = serde_json::to_string(&config).unwrap();
        let parsed: GraphConfig = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed.workspace_name.as_deref(), Some("工作区C"));
    }

    #[test]
    fn edge_curve_bias_roundtrip_is_preserved() {
        let config = GraphConfig {
            version: GRAPH_CONFIG_VERSION,
            nodes: vec![sample_node(1), sample_node(2)],
            edges: vec![(1, 2)],
            edge_routes: vec![],
            edge_curve_biases: vec![EdgeCurveBiasConfig {
                from: 1,
                to: 2,
                bias: 72.5,
            }],
            edge_control_offsets: vec![],
            view: ViewConfig::default(),
            workspace_name: Some("工作区A".to_owned()),
            script_states: vec![],
        };

        let text = serde_json::to_string(&config).unwrap();
        let parsed: GraphConfig = serde_json::from_str(&text).unwrap();

        assert_eq!(parsed.edge_curve_biases.len(), 1);
        assert_eq!(parsed.edge_curve_biases[0].from, 1);
        assert_eq!(parsed.edge_curve_biases[0].to, 2);
        assert!((parsed.edge_curve_biases[0].bias - 72.5).abs() < 0.001);
        assert_eq!(parsed.workspace_name.as_deref(), Some("工作区A"));
    }

    #[test]
    fn edge_control_offsets_roundtrip_is_preserved() {
        let config = GraphConfig {
            version: GRAPH_CONFIG_VERSION,
            nodes: vec![sample_node(1), sample_node(2)],
            edges: vec![(1, 2)],
            edge_routes: vec![],
            edge_curve_biases: vec![],
            edge_control_offsets: vec![EdgeControlOffsetConfig {
                from: 1,
                to: 2,
                source_dx: 24.0,
                source_dy: -11.0,
                target_dx: -30.0,
                target_dy: 16.0,
            }],
            view: ViewConfig::default(),
            workspace_name: Some("工作区B".to_owned()),
            script_states: vec![],
        };

        let text = serde_json::to_string(&config).unwrap();
        let parsed: GraphConfig = serde_json::from_str(&text).unwrap();

        assert_eq!(parsed.edge_control_offsets.len(), 1);
        assert_eq!(parsed.edge_control_offsets[0].from, 1);
        assert_eq!(parsed.edge_control_offsets[0].to, 2);
        assert!((parsed.edge_control_offsets[0].source_dx - 24.0).abs() < 0.001);
        assert!((parsed.edge_control_offsets[0].target_dx + 30.0).abs() < 0.001);
        assert_eq!(parsed.workspace_name.as_deref(), Some("工作区B"));
    }

    #[test]
    fn workspace_name_saved_name_is_trimmed_on_replay() {
        let parsed: GraphConfig = serde_json::from_value(json!({
            "version": GRAPH_CONFIG_VERSION,
            "nodes": [
                {
                    "id": 1,
                    "uid": "u-1",
                    "kind": "Text",
                    "data": {"Text": {"text_body": "", "auto_size": false}},
                    "pos_x": 0.0,
                    "pos_y": 0.0,
                    "size_x": 120.0,
                    "size_y": 80.0
                }
            ],
            "edges": [],
            "edge_routes": [],
            "view": {"pan_x": 0.0, "pan_y": 0.0, "zoom": 1.0},
            "workspace_name": "  画布命名A  "
        }))
        .unwrap();

        assert_eq!(
            GraphApp::resolve_workspace_name(parsed.workspace_name.as_deref(), None),
            "画布命名A"
        );
    }

    #[test]
    fn workspace_name_malformed_blank_field_falls_back_to_default() {
        let parsed: GraphConfig = serde_json::from_value(json!({
            "version": GRAPH_CONFIG_VERSION,
            "nodes": [
                {
                    "id": 1,
                    "uid": "u-1",
                    "kind": "Text",
                    "data": {"Text": {"text_body": "", "auto_size": false}},
                    "pos_x": 0.0,
                    "pos_y": 0.0,
                    "size_x": 120.0,
                    "size_y": 80.0
                }
            ],
            "edges": [],
            "edge_routes": [],
            "view": {"pan_x": 0.0, "pan_y": 0.0, "zoom": 1.0},
            "workspace_name": "   "
        }))
        .unwrap();

        assert_eq!(
            GraphApp::resolve_workspace_name(parsed.workspace_name.as_deref(), None),
            GraphApp::default_workspace_name()
        );
    }

    #[test]
    fn workspace_name_legacy_config_without_field_is_supported() {
        let legacy = json!({
            "version": 4,
            "nodes": [
                {
                    "id": 1,
                    "uid": "u-1",
                    "kind": "Text",
                    "data": {"Text": {"text_body": "", "auto_size": false}},
                    "pos_x": 0.0,
                    "pos_y": 0.0,
                    "size_x": 120.0,
                    "size_y": 80.0
                },
                {
                    "id": 2,
                    "uid": "u-2",
                    "kind": "Text",
                    "data": {"Text": {"text_body": "", "auto_size": false}},
                    "pos_x": 200.0,
                    "pos_y": 100.0,
                    "size_x": 120.0,
                    "size_y": 80.0
                }
            ],
            "edges": [[1, 2]],
            "edge_routes": [],
            "view": {"pan_x": 0.0, "pan_y": 0.0, "zoom": 1.0}
        });

        let parsed: GraphConfig = serde_json::from_value(legacy).unwrap();
        assert!(parsed.edge_curve_biases.is_empty());
        assert!(parsed.edge_control_offsets.is_empty());
        assert_eq!(parsed.workspace_name, None);
        assert_eq!(
            GraphApp::resolve_workspace_name(
                parsed.workspace_name.as_deref(),
                Some(Path::new("/tmp/legacy.graph.json")),
            ),
            "legacy.graph"
        );
    }
}

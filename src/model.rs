use eframe::egui::{self, Pos2};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    Terminal,
    Text,
    Html,
    Image,
    Decision,
    Group,
}

fn default_text_autosize() -> bool {
    false
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionButton {
    pub label: String,
    pub event_key: String,
    #[serde(default)]
    pub color_rgb: Option<[u8; 3]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeData {
    Terminal {
        title: String,
        startup_script: String,
        #[serde(default)]
        working_directory: Option<String>,
    },
    Text {
        text_body: String,
        #[serde(default = "default_text_autosize")]
        auto_size: bool,
    },
    Html {
        html_source: String,
    },
    Image {
        image_path: String,
    },
    Decision {
        title: String,
        #[serde(default)]
        buttons: Vec<DecisionButton>,
        #[serde(default)]
        pending_message: Option<String>,
        #[serde(default)]
        pending_messages: Vec<String>,
    },
    Group {
        title: String,
        #[serde(default)]
        child_node_ids: Vec<usize>,
    },
}

#[derive(Clone)]
pub struct Node {
    pub id: usize,
    pub uid: String,
    pub kind: NodeKind,
    pub data: NodeData,
    pub pos: Pos2,
    pub size: egui::Vec2,
}

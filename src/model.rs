use eframe::egui::{self, Pos2};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    Terminal,
    Text,
    Image,
    Decision,
    Group,
    Script,
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
    Script {
        title: String,
        /// JSON spec string for the widget tree
        code: String,
        #[serde(default)]
        /// Incoming message queue (like Decision node)
        pending_messages: Vec<String>,
        #[serde(default)]
        /// Parsed cache (not serialized, reconstructed on load)
        #[serde(skip)]
        parsed_spec: Option<crate::script_node::types::ScriptNodeSpec>,
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

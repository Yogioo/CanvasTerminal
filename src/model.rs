use eframe::egui::{self, Pos2};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    Terminal,
    Text,
    Image,
}

fn default_text_autosize() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeData {
    Terminal {
        title: String,
        startup_script: String,
    },
    Text {
        text_body: String,
        #[serde(default = "default_text_autosize")]
        auto_size: bool,
    },
    Image {
        image_path: String,
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

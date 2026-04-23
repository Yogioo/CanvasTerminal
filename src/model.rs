use eframe::egui::{self, Pos2};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    Terminal,
    Text,
    Image,
}

#[derive(Clone)]
pub struct Node {
    pub id: usize,
    pub title: String,
    pub kind: NodeKind,
    pub identity: String,
    pub startup_script: String,
    pub text_body: String,
    pub image_path: String,
    pub pos: Pos2,
    pub size: egui::Vec2,
}

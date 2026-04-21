use eframe::egui::{self, Pos2};

#[derive(Clone, PartialEq, Eq)]
pub enum NodeKind {
    Terminal,
    Text,
}

#[derive(Clone)]
pub struct Node {
    pub id: usize,
    pub title: String,
    pub kind: NodeKind,
    pub category: String,
    pub text_body: String,
    pub pos: Pos2,
    pub size: egui::Vec2,
    pub status: &'static str,
}

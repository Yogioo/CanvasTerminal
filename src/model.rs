use eframe::egui::{self, Pos2};

#[derive(Clone)]
pub enum NodeKind {
    Service,
    Terminal,
}

#[derive(Clone)]
pub struct Node {
    pub id: usize,
    pub title: String,
    pub kind: NodeKind,
    pub pos: Pos2,
    pub size: egui::Vec2,
    pub status: &'static str,
    pub latency_ms: u32,
    pub qps: f32,
    pub errors: f32,
}

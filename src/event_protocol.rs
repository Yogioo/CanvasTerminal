use serde::{Deserialize, Serialize};

pub const DEFAULT_CANVAS_API: &str = "http://127.0.0.1:4545";
pub const DEFAULT_CANVAS_BIND_ADDR: &str = "127.0.0.1:4545";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoneEvent {
    pub node_uid: String,
    pub summary: String,
}

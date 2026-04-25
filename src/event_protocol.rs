use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_CANVAS_API: &str = "http://127.0.0.1:4545";
pub const DEFAULT_CANVAS_BIND_ADDR: &str = "127.0.0.1:4545";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoneEvent {
    pub node_uid: String,
    pub summary: String,
    #[serde(default)]
    pub route_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationRequest {
    pub action: String,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub timestamp_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationError {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub details: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationDiagnostics {
    pub action: String,
    pub queue_ms: u64,
    pub exec_ms: u64,
    pub total_ms: u64,
    pub state_version: u64,
    pub state_timestamp_ms: u64,
    #[serde(default)]
    pub affected_ids: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationResponse {
    #[serde(default)]
    pub request_id: Option<String>,
    pub ok: bool,
    #[serde(default)]
    pub data: Value,
    #[serde(default)]
    pub error: Option<AutomationError>,
    pub diagnostics: AutomationDiagnostics,
}

#[derive(Debug)]
pub struct AutomationCall {
    pub request: AutomationRequest,
    pub received_at_ms: u64,
    pub response_tx: std::sync::mpsc::Sender<AutomationResponse>,
}

#[derive(Debug)]
pub enum AppEvent {
    Done(DoneEvent),
    Automation(AutomationCall),
}

pub fn now_timestamp_ms() -> u64 {
    let now = std::time::SystemTime::now();
    match now.duration_since(std::time::UNIX_EPOCH) {
        Ok(v) => v.as_millis().min(u128::from(u64::MAX)) as u64,
        Err(_) => 0,
    }
}

pub fn empty_diagnostics(action: &str) -> AutomationDiagnostics {
    AutomationDiagnostics {
        action: action.to_owned(),
        queue_ms: 0,
        exec_ms: 0,
        total_ms: 0,
        state_version: 0,
        state_timestamp_ms: now_timestamp_ms(),
        affected_ids: Vec::new(),
    }
}

pub fn response_error(
    request_id: Option<String>,
    action: &str,
    code: &str,
    message: impl Into<String>,
) -> AutomationResponse {
    AutomationResponse {
        request_id,
        ok: false,
        data: Value::Null,
        error: Some(AutomationError {
            code: code.to_owned(),
            message: message.into(),
            details: None,
        }),
        diagnostics: empty_diagnostics(action),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn automation_request_roundtrip() {
        let req = AutomationRequest {
            action: "graph.get".to_owned(),
            payload: json!({"since_version": 1}),
            request_id: Some("req-1".to_owned()),
            timestamp_ms: Some(123),
        };

        let text = serde_json::to_string(&req).unwrap();
        let parsed: AutomationRequest = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed.action, "graph.get");
        assert_eq!(parsed.request_id.as_deref(), Some("req-1"));
    }

    #[test]
    fn response_error_contains_code() {
        let resp = response_error(Some("x".to_owned()), "node.create", "BAD_PAYLOAD", "bad");
        assert!(!resp.ok);
        assert_eq!(resp.error.unwrap().code, "BAD_PAYLOAD");
    }

    #[test]
    fn done_event_is_backward_compatible_without_route_key() {
        let text = r#"{"node_uid":"u1","summary":"ok"}"#;
        let parsed: DoneEvent = serde_json::from_str(text).unwrap();
        assert_eq!(parsed.node_uid, "u1");
        assert_eq!(parsed.route_key, None);
    }

    #[test]
    fn done_event_supports_route_key() {
        let text = r#"{"node_uid":"u1","summary":"ok","route_key":"fix"}"#;
        let parsed: DoneEvent = serde_json::from_str(text).unwrap();
        assert_eq!(parsed.route_key.as_deref(), Some("fix"));
    }
}

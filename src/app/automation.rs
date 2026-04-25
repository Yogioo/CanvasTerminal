use super::automation_support::{
    run_shell_command, EdgePayload, EdgeReconnectPayload, GraphGetPayload, InjectTerminalPayload,
    InjectTextPayload, NodeCreatePayload, NodeDeletePayload, NodeMovePayload, NodeUpdatePayload,
    TerminalRestartPayload,
};
use super::GraphApp;
use crate::event_protocol::{
    now_timestamp_ms, response_error, AutomationCall, AutomationDiagnostics, AutomationError,
    AutomationRequest, AutomationResponse,
};
use crate::model::{NodeData, NodeKind};
use eframe::egui::Pos2;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Instant;

#[derive(Debug)]
struct AutomationOutcome {
    data: Value,
    affected_ids: Vec<usize>,
}

impl GraphApp {
    pub(in crate::app) fn bump_automation_state_version(&mut self) {
        self.automation_state_version = self.automation_state_version.saturating_add(1);
        self.automation_state_timestamp_ms = now_timestamp_ms();
    }

    pub(in crate::app) fn handle_automation_call(&mut self, call: AutomationCall) {
        let queue_ms = now_timestamp_ms().saturating_sub(call.received_at_ms);
        let request = call.request;

        if let Some(request_id) = &request.request_id {
            if let Some(previous) = self.processed_automation_requests.get(request_id) {
                let _ = call.response_tx.send(previous.clone());
                return;
            }
        }

        let started = Instant::now();
        let response = match self.execute_automation_request(&request) {
            Ok(outcome) => AutomationResponse {
                request_id: request.request_id.clone(),
                ok: true,
                data: outcome.data,
                error: None,
                diagnostics: AutomationDiagnostics {
                    action: request.action.clone(),
                    queue_ms,
                    exec_ms: started.elapsed().as_millis() as u64,
                    total_ms: queue_ms + started.elapsed().as_millis() as u64,
                    state_version: self.automation_state_version,
                    state_timestamp_ms: self.automation_state_timestamp_ms,
                    affected_ids: outcome.affected_ids,
                },
            },
            Err(mut resp) => {
                resp.diagnostics.queue_ms = queue_ms;
                resp.diagnostics.exec_ms = started.elapsed().as_millis() as u64;
                resp.diagnostics.total_ms = queue_ms + resp.diagnostics.exec_ms;
                resp.diagnostics.state_version = self.automation_state_version;
                resp.diagnostics.state_timestamp_ms = self.automation_state_timestamp_ms;
                resp
            }
        };

        if let Some(request_id) = &response.request_id {
            self.processed_automation_requests
                .insert(request_id.clone(), response.clone());
        }

        self.append_automation_action_log(&request, &response);
        let _ = call.response_tx.send(response);
    }

    fn execute_automation_request(
        &mut self,
        request: &AutomationRequest,
    ) -> Result<AutomationOutcome, AutomationResponse> {
        match request.action.as_str() {
            "graph.get" => self.automation_graph_get(request),
            "node.create" => self.automation_node_create(request),
            "node.move" => self.automation_node_move(request),
            "node.update" => self.automation_node_update(request),
            "node.delete" => self.automation_node_delete(request),
            "edge.create" => self.automation_edge_create(request),
            "edge.reconnect" => self.automation_edge_reconnect(request),
            "edge.delete" => self.automation_edge_delete(request),
            "inject.text" => self.automation_inject_text(request),
            "inject.terminal" => self.automation_inject_terminal(request),
            "terminal.restart" => self.automation_terminal_restart(request),
            _ => Err(response_error(
                request.request_id.clone(),
                &request.action,
                "UNKNOWN_ACTION",
                format!("unsupported action: {}", request.action),
            )),
        }
    }

    fn parse_payload<T: for<'de> Deserialize<'de>>(
        request: &AutomationRequest,
    ) -> Result<T, AutomationResponse> {
        serde_json::from_value::<T>(request.payload.clone()).map_err(|err| {
            response_error(
                request.request_id.clone(),
                &request.action,
                "BAD_PAYLOAD",
                format!("invalid payload: {err}"),
            )
        })
    }

    fn automation_graph_get(
        &mut self,
        request: &AutomationRequest,
    ) -> Result<AutomationOutcome, AutomationResponse> {
        let payload: GraphGetPayload = Self::parse_payload(request)?;
        if payload.since_version == Some(self.automation_state_version) {
            return Ok(AutomationOutcome {
                data: json!({
                    "version": self.automation_state_version,
                    "timestamp_ms": self.automation_state_timestamp_ms,
                    "changes": [],
                }),
                affected_ids: Vec::new(),
            });
        }

        let mut nodes: Vec<Value> = self
            .nodes
            .iter()
            .map(|n| {
                json!({
                    "id": n.id,
                    "uid": n.uid,
                    "kind": match n.kind {
                        NodeKind::Terminal => "terminal",
                        NodeKind::Text => "text",
                        NodeKind::Image => "image",
                    },
                    "data": n.data,
                    "pos": {"x": n.pos.x, "y": n.pos.y},
                    "size": {"x": n.size.x, "y": n.size.y},
                })
            })
            .collect();
        nodes.sort_by_key(|n| n.get("id").and_then(Value::as_u64).unwrap_or_default());

        let mut edges = self.edges.clone();
        edges.sort_unstable();

        let mut edge_routes: Vec<Value> = self
            .edge_route_keys
            .iter()
            .filter_map(|((from, to), route_key)| {
                let trimmed = route_key.trim();
                if trimmed.is_empty() || !self.has_edge(*from, *to) {
                    return None;
                }

                Some(json!({
                    "from": from,
                    "to": to,
                    "route_key": trimmed,
                }))
            })
            .collect();
        edge_routes.sort_by_key(|edge| {
            (
                edge.get("from").and_then(Value::as_u64).unwrap_or_default(),
                edge.get("to").and_then(Value::as_u64).unwrap_or_default(),
            )
        });

        let mut selection: Vec<usize> = self.selected_nodes.iter().copied().collect();
        selection.sort_unstable();

        Ok(AutomationOutcome {
            data: json!({
                "version": self.automation_state_version,
                "timestamp_ms": self.automation_state_timestamp_ms,
                "snapshot": {
                    "nodes": nodes,
                    "edges": edges,
                    "edge_routes": edge_routes,
                    "viewport": {
                        "pan": {"x": self.pan.x, "y": self.pan.y},
                        "zoom": self.zoom,
                    },
                    "selection": {
                        "selected": self.selected,
                        "selected_nodes": selection,
                    }
                }
            }),
            affected_ids: Vec::new(),
        })
    }

    fn automation_node_create(
        &mut self,
        request: &AutomationRequest,
    ) -> Result<AutomationOutcome, AutomationResponse> {
        let payload: NodeCreatePayload = Self::parse_payload(request)?;
        let kind = payload.kind.to_ascii_lowercase();
        let pos = Pos2::new(payload.x, payload.y);

        let node_id = match kind.as_str() {
            "terminal" => {
                let id = self.create_terminal_node(pos);
                if let Some(title) = payload.title {
                    if let Some(node) = self.nodes.iter_mut().find(|n| n.id == id) {
                        if let NodeData::Terminal { title: old, .. } = &mut node.data {
                            *old = title;
                        }
                    }
                }
                if let Some(startup_script) = payload.startup_script {
                    if let Some(node) = self.nodes.iter_mut().find(|n| n.id == id) {
                        if let NodeData::Terminal {
                            startup_script: old,
                            ..
                        } = &mut node.data
                        {
                            *old = startup_script;
                        }
                    }
                }
                id
            }
            "text" => {
                let id = self.create_text_node(pos, false);
                if let Some(text_body) = payload.text_body {
                    if let Some(node) = self.nodes.iter_mut().find(|n| n.id == id) {
                        if let NodeData::Text { text_body: old, .. } = &mut node.data {
                            *old = text_body;
                        }
                    }
                }
                id
            }
            "image" => {
                let image_path = payload.image_path.ok_or_else(|| {
                    response_error(
                        request.request_id.clone(),
                        &request.action,
                        "BAD_PAYLOAD",
                        "image_path is required for image node",
                    )
                })?;
                self.create_image_node_from_path(pos, image_path)
            }
            _ => {
                return Err(response_error(
                    request.request_id.clone(),
                    &request.action,
                    "BAD_PAYLOAD",
                    format!("unsupported node kind: {}", payload.kind),
                ))
            }
        };

        self.bump_automation_state_version();

        Ok(AutomationOutcome {
            data: json!({"node_id": node_id, "version": self.automation_state_version}),
            affected_ids: vec![node_id],
        })
    }

    fn automation_node_move(
        &mut self,
        request: &AutomationRequest,
    ) -> Result<AutomationOutcome, AutomationResponse> {
        let payload: NodeMovePayload = Self::parse_payload(request)?;
        let Some(node) = self.nodes.iter_mut().find(|n| n.id == payload.id) else {
            return Err(response_error(
                request.request_id.clone(),
                &request.action,
                "NOT_FOUND",
                format!("node not found: {}", payload.id),
            ));
        };

        node.pos = Pos2::new(payload.x, payload.y);
        self.mark_workspace_dirty();

        Ok(AutomationOutcome {
            data: json!({"node_id": payload.id, "version": self.automation_state_version}),
            affected_ids: vec![payload.id],
        })
    }

    fn automation_node_update(
        &mut self,
        request: &AutomationRequest,
    ) -> Result<AutomationOutcome, AutomationResponse> {
        let payload: NodeUpdatePayload = Self::parse_payload(request)?;
        let Some(node) = self.nodes.iter_mut().find(|n| n.id == payload.id) else {
            return Err(response_error(
                request.request_id.clone(),
                &request.action,
                "NOT_FOUND",
                format!("node not found: {}", payload.id),
            ));
        };

        match &mut node.data {
            NodeData::Text {
                text_body,
                auto_size,
            } => {
                if let Some(next) = payload.text_body {
                    *text_body = next;
                }
                if let Some(next) = payload.auto_size {
                    *auto_size = next;
                }
            }
            NodeData::Terminal {
                title,
                startup_script,
            } => {
                if let Some(next) = payload.title {
                    *title = next;
                }
                if let Some(next) = payload.startup_script {
                    *startup_script = next;
                }
            }
            NodeData::Image { .. } => {}
        }

        self.mark_workspace_dirty();
        Ok(AutomationOutcome {
            data: json!({"node_id": payload.id, "version": self.automation_state_version}),
            affected_ids: vec![payload.id],
        })
    }

    fn automation_node_delete(
        &mut self,
        request: &AutomationRequest,
    ) -> Result<AutomationOutcome, AutomationResponse> {
        let payload: NodeDeletePayload = Self::parse_payload(request)?;
        if !self.nodes.iter().any(|n| n.id == payload.id) {
            return Ok(AutomationOutcome {
                data: json!({
                    "node_id": payload.id,
                    "status": "already_deleted",
                    "version": self.automation_state_version,
                }),
                affected_ids: vec![payload.id],
            });
        }

        self.remove_node(payload.id);
        self.bump_automation_state_version();
        Ok(AutomationOutcome {
            data: json!({"node_id": payload.id, "version": self.automation_state_version}),
            affected_ids: vec![payload.id],
        })
    }

    fn automation_edge_create(
        &mut self,
        request: &AutomationRequest,
    ) -> Result<AutomationOutcome, AutomationResponse> {
        let payload: EdgePayload = Self::parse_payload(request)?;
        if self.has_edge(payload.from, payload.to) {
            if let Some(route_key) = payload.route_key {
                self.set_edge_route_key(payload.from, payload.to, route_key);
                self.mark_workspace_dirty();
            }

            return Ok(AutomationOutcome {
                data: json!({
                    "status": "already_exists",
                    "edge": [payload.from, payload.to],
                    "route_key": self.edge_route_key(payload.from, payload.to),
                    "version": self.automation_state_version,
                }),
                affected_ids: vec![payload.from, payload.to],
            });
        }

        let has_from = self.nodes.iter().any(|n| n.id == payload.from);
        let has_to = self.nodes.iter().any(|n| n.id == payload.to);
        if !has_from || !has_to {
            return Err(response_error(
                request.request_id.clone(),
                &request.action,
                "NOT_FOUND",
                "edge endpoint node not found",
            ));
        }

        self.edges.push((payload.from, payload.to));
        if let Some(route_key) = payload.route_key {
            self.set_edge_route_key(payload.from, payload.to, route_key);
        }
        self.mark_workspace_dirty();

        Ok(AutomationOutcome {
            data: json!({
                "edge": [payload.from, payload.to],
                "route_key": self.edge_route_key(payload.from, payload.to),
                "version": self.automation_state_version,
            }),
            affected_ids: vec![payload.from, payload.to],
        })
    }

    fn automation_edge_reconnect(
        &mut self,
        request: &AutomationRequest,
    ) -> Result<AutomationOutcome, AutomationResponse> {
        let payload: EdgeReconnectPayload = Self::parse_payload(request)?;
        let Some(existing_idx) = self
            .edges
            .iter()
            .position(|(from, to)| *from == payload.from && *to == payload.to)
        else {
            return Err(response_error(
                request.request_id.clone(),
                &request.action,
                "NOT_FOUND",
                "edge not found",
            ));
        };

        let previous_route_key = self
            .edge_route_key(payload.from, payload.to)
            .map(str::to_owned);
        self.remove_edge_route_key(payload.from, payload.to);

        self.edges[existing_idx] = (payload.new_from, payload.new_to);

        if let Some(new_route_key) = payload.new_route_key {
            self.set_edge_route_key(payload.new_from, payload.new_to, new_route_key);
        } else if let Some(prev) = previous_route_key {
            self.set_edge_route_key(payload.new_from, payload.new_to, prev);
        }

        self.mark_workspace_dirty();

        Ok(AutomationOutcome {
            data: json!({
                "edge": [payload.new_from, payload.new_to],
                "route_key": self.edge_route_key(payload.new_from, payload.new_to),
                "version": self.automation_state_version,
            }),
            affected_ids: vec![payload.new_from, payload.new_to],
        })
    }

    fn automation_edge_delete(
        &mut self,
        request: &AutomationRequest,
    ) -> Result<AutomationOutcome, AutomationResponse> {
        let payload: EdgePayload = Self::parse_payload(request)?;
        let before = self.edges.len();
        self.edges
            .retain(|(from, to)| !(*from == payload.from && *to == payload.to));
        self.remove_edge_route_key(payload.from, payload.to);
        if self.edges.len() == before {
            return Ok(AutomationOutcome {
                data: json!({
                    "status": "already_deleted",
                    "edge": [payload.from, payload.to],
                    "version": self.automation_state_version,
                }),
                affected_ids: vec![payload.from, payload.to],
            });
        }

        self.mark_workspace_dirty();
        Ok(AutomationOutcome {
            data: json!({
                "edge": [payload.from, payload.to],
                "version": self.automation_state_version,
            }),
            affected_ids: vec![payload.from, payload.to],
        })
    }

    fn automation_inject_text(
        &mut self,
        request: &AutomationRequest,
    ) -> Result<AutomationOutcome, AutomationResponse> {
        let payload: InjectTextPayload = Self::parse_payload(request)?;
        let Some(node) = self.nodes.iter_mut().find(|n| n.id == payload.node_id) else {
            return Err(response_error(
                request.request_id.clone(),
                &request.action,
                "NOT_FOUND",
                format!("node not found: {}", payload.node_id),
            ));
        };

        match &mut node.data {
            NodeData::Text { text_body, .. } => {
                if payload.mode.eq_ignore_ascii_case("append") {
                    text_body.push_str(&payload.text);
                } else {
                    *text_body = payload.text;
                }
            }
            _ => {
                return Err(response_error(
                    request.request_id.clone(),
                    &request.action,
                    "BAD_TARGET",
                    "inject.text currently supports text nodes only",
                ))
            }
        }

        self.mark_workspace_dirty();
        Ok(AutomationOutcome {
            data: json!({"node_id": payload.node_id, "version": self.automation_state_version}),
            affected_ids: vec![payload.node_id],
        })
    }

    fn automation_inject_terminal(
        &mut self,
        request: &AutomationRequest,
    ) -> Result<AutomationOutcome, AutomationResponse> {
        let payload: InjectTerminalPayload = Self::parse_payload(request)?;
        let wait = payload.wait.unwrap_or(true);
        let _ = payload.node_id;

        if !wait {
            return Ok(AutomationOutcome {
                data: json!({
                    "accepted": true,
                    "wait": false,
                    "note": "non-wait mode reserved; command is not executed",
                    "version": self.automation_state_version,
                }),
                affected_ids: vec![payload.node_id],
            });
        }

        let timeout = payload.timeout_ms.unwrap_or(30_000).max(1);
        let output =
            run_shell_command(&payload.command, timeout).map_err(|err| AutomationResponse {
                request_id: request.request_id.clone(),
                ok: false,
                data: Value::Null,
                error: Some(AutomationError {
                    code: "TERMINAL_EXEC_FAILED".to_owned(),
                    message: err,
                    details: None,
                }),
                diagnostics: crate::event_protocol::empty_diagnostics(&request.action),
            })?;

        Ok(AutomationOutcome {
            data: json!({
                "node_id": payload.node_id,
                "stdout": output.stdout,
                "stderr": output.stderr,
                "exit_code": output.exit_code,
                "timed_out": output.timed_out,
                "version": self.automation_state_version,
            }),
            affected_ids: vec![payload.node_id],
        })
    }

    fn automation_terminal_restart(
        &mut self,
        request: &AutomationRequest,
    ) -> Result<AutomationOutcome, AutomationResponse> {
        let payload: TerminalRestartPayload = Self::parse_payload(request)?;
        let Some(node) = self.nodes.iter().find(|n| n.id == payload.node_id) else {
            return Err(response_error(
                request.request_id.clone(),
                &request.action,
                "NOT_FOUND",
                format!("node not found: {}", payload.node_id),
            ));
        };

        if !matches!(node.kind, NodeKind::Terminal) {
            return Err(response_error(
                request.request_id.clone(),
                &request.action,
                "BAD_TARGET",
                "terminal.restart supports terminal nodes only",
            ));
        }

        self.restart_terminal_deferred(payload.node_id);
        self.bump_automation_state_version();

        Ok(AutomationOutcome {
            data: json!({
                "node_id": payload.node_id,
                "restarted": true,
                "version": self.automation_state_version,
            }),
            affected_ids: vec![payload.node_id],
        })
    }

    fn append_automation_action_log(
        &self,
        request: &AutomationRequest,
        response: &AutomationResponse,
    ) {
        let log_dir = std::path::Path::new("artifacts/automation");
        if std::fs::create_dir_all(log_dir).is_err() {
            return;
        }

        let line = json!({
            "timestamp_ms": now_timestamp_ms(),
            "request": request,
            "response": response,
        });

        let path = log_dir.join("actions.jsonl");
        let mut text = serde_json::to_string(&line).unwrap_or_else(|_| "{}".to_owned());
        text.push('\n');

        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = file.write_all(text.as_bytes());
        }
    }
}

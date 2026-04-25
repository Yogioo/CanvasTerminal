use super::GraphApp;
use crate::event_protocol::{AppEvent, DoneEvent};
use crate::model::{NodeData, NodeKind};
use crate::shell::{system_shell, terminal_shell_args};
use eframe::egui;
use egui_term::{BackendCommand, BackendSettings, PtyEvent, TerminalBackend};

impl GraphApp {
    fn terminal_uid(&self, node_id: usize) -> Option<&str> {
        self.nodes
            .iter()
            .find(|n| n.id == node_id)
            .map(|n| n.uid.as_str())
    }

    pub(in crate::app) fn complete_text_node_and_forward(&mut self, node_id: usize) {
        let Some(text_body) =
            self.nodes
                .iter()
                .find(|n| n.id == node_id)
                .and_then(|n| match &n.data {
                    NodeData::Text { text_body, .. } => Some(text_body.trim().to_owned()),
                    _ => None,
                })
        else {
            return;
        };

        if text_body.is_empty() {
            self.push_toast_notification("文本为空，未传递到下游节点");
            return;
        }

        let downstream_terminal_ids: Vec<usize> = self
            .edges
            .iter()
            .filter_map(|(from, to)| (*from == node_id).then_some(*to))
            .filter(|target_id| {
                self.nodes
                    .iter()
                    .any(|n| n.id == *target_id && matches!(n.kind, NodeKind::Terminal))
            })
            .collect();

        if downstream_terminal_ids.is_empty() {
            self.push_toast_notification("无下游终端节点可接收传递内容");
            return;
        }

        let injected = Self::build_injected_text_block(&text_body);
        for target_id in downstream_terminal_ids.iter().copied() {
            self.inject_terminal_message_and_submit(target_id, &injected);
        }

        self.push_toast_notification(format!(
            "已完成并传递到 {} 个下游终端节点",
            downstream_terminal_ids.len()
        ));
    }

    fn terminal_startup_script(&self, node_id: usize) -> Option<String> {
        self.nodes
            .iter()
            .find(|n| n.id == node_id)
            .and_then(|n| match &n.data {
                NodeData::Terminal { startup_script, .. } => Some(startup_script.trim()),
                _ => None,
            })
            .filter(|script| !script.is_empty())
            .map(ToOwned::to_owned)
    }

    fn inject_terminal_text(&mut self, node_id: usize, text: &str) {
        if let Some(backend) = self.terminal_backends.get_mut(&node_id) {
            backend.process_command(BackendCommand::Write(text.as_bytes().to_vec()));
        } else {
            self.pending_terminal_injections
                .entry(node_id)
                .or_default()
                .push(text.to_owned());
        }
    }

    fn inject_terminal_submit(&mut self, node_id: usize) {
        self.inject_terminal_text(node_id, "\r");
    }

    fn inject_terminal_message_and_submit(&mut self, node_id: usize, message: &str) {
        self.inject_terminal_text(node_id, message);
        self.inject_terminal_submit(node_id);
    }

    fn run_terminal_startup_script(&mut self, node_id: usize) {
        let Some(script) = self.terminal_startup_script(node_id) else {
            return;
        };

        let command = format!("{script}\r\n");
        self.inject_terminal_text(node_id, &command);
    }

    fn build_injected_text_block(body: &str) -> String {
        body.trim_end_matches(['\r', '\n']).to_owned()
    }

    pub(in crate::app) fn poll_done_events(&mut self) {
        let mut done_events = Vec::new();
        let mut automation_calls = Vec::new();

        if let Some(rx) = &self.event_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    AppEvent::Done(done) => done_events.push(done),
                    AppEvent::Automation(call) => automation_calls.push(call),
                }
            }
        }

        for event in done_events {
            self.handle_done_event(event);
        }

        for call in automation_calls {
            self.handle_automation_call(call);
        }
    }

    fn handle_done_event(&mut self, event: DoneEvent) {
        let Some(source_id) = self
            .nodes
            .iter()
            .find(|n| n.uid == event.node_uid)
            .map(|n| n.id)
        else {
            return;
        };

        let route_key = event
            .route_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());

        let downstream_terminal_ids: Vec<usize> = self
            .edges
            .iter()
            .filter(|(from, to)| {
                if *from != source_id {
                    return false;
                }

                if let Some(expected) = route_key {
                    let actual = self.edge_route_key(*from, *to).unwrap_or_default();
                    if actual != expected {
                        return false;
                    }
                }

                true
            })
            .map(|(_, to)| *to)
            .filter(|target_id| {
                self.nodes
                    .iter()
                    .any(|n| n.id == *target_id && matches!(n.kind, NodeKind::Terminal))
            })
            .collect();

        if downstream_terminal_ids.is_empty() {
            if let Some(expected) = route_key {
                self.push_toast_notification(format!(
                    "未找到 route_key = '{expected}' 的下游终端连线"
                ));
            }
            return;
        }

        let injected = Self::build_injected_text_block(&event.summary);
        for target_id in downstream_terminal_ids {
            self.inject_terminal_message_and_submit(target_id, &injected);
        }
    }

    fn ensure_terminal(&mut self, node_id: usize, ctx: &egui::Context) {
        if self.terminal_backends.contains_key(&node_id) {
            return;
        }

        let shell = system_shell();
        let Some(node_uid) = self.terminal_uid(node_id) else {
            self.terminal_errors
                .insert(node_id, "终端启动失败: 未找到节点 UID".to_owned());
            return;
        };
        match TerminalBackend::new(
            node_id as u64,
            ctx.clone(),
            self.pty_tx.clone(),
            BackendSettings {
                shell,
                args: terminal_shell_args(node_id, node_uid),
                working_directory: std::env::current_dir().ok(),
            },
        ) {
            Ok(backend) => {
                self.terminal_backends.insert(node_id, backend);
                self.terminal_exited.remove(&node_id);
                self.terminal_errors.remove(&node_id);

                self.run_terminal_startup_script(node_id);

                if let Some(pending) = self.pending_terminal_injections.remove(&node_id) {
                    for text in pending {
                        self.inject_terminal_text(node_id, &text);
                    }
                }
            }
            Err(e) => {
                self.terminal_errors
                    .insert(node_id, format!("终端启动失败: {e}"));
            }
        }
    }

    pub(in crate::app) fn queue_terminal_start(&mut self, node_id: usize) {
        if self.terminal_backends.contains_key(&node_id)
            || self.terminal_errors.contains_key(&node_id)
            || self.terminal_exited.contains(&node_id)
        {
            return;
        }

        let is_terminal_node = self
            .nodes
            .iter()
            .any(|n| n.id == node_id && matches!(n.kind, NodeKind::Terminal));
        if !is_terminal_node {
            return;
        }

        if !self.pending_terminal_starts.contains(&node_id) {
            self.pending_terminal_starts.push(node_id);
        }
    }

    pub(in crate::app) fn process_terminal_start_queue(&mut self, ctx: &egui::Context) {
        const MAX_TERMINAL_STARTS_PER_FRAME: usize = 1;

        for _ in 0..MAX_TERMINAL_STARTS_PER_FRAME {
            if self.pending_terminal_starts.is_empty() {
                break;
            }

            let node_id = self.pending_terminal_starts.remove(0);
            self.ensure_terminal(node_id, ctx);
        }
    }

    pub(in crate::app) fn restart_terminal_deferred(&mut self, node_id: usize) {
        self.terminal_backends.remove(&node_id);
        self.terminal_exited.remove(&node_id);
        self.terminal_errors.remove(&node_id);
        self.pending_terminal_starts.retain(|id| *id != node_id);
        self.queue_terminal_start(node_id);
    }

    pub(in crate::app) fn restart_terminal(&mut self, node_id: usize, ctx: &egui::Context) {
        self.restart_terminal_deferred(node_id);
        self.ensure_terminal(node_id, ctx);
    }

    pub(in crate::app) fn poll_terminal_events(&mut self) {
        while let Ok((id, event)) = self.pty_rx.try_recv() {
            if let PtyEvent::Exit = event {
                let node_id = id as usize;
                self.terminal_exited.insert(node_id);
                self.terminal_backends.remove(&node_id);
            }
        }
    }
}

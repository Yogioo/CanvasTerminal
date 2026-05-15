use super::GraphApp;
use crate::event_protocol::{AppEvent, DoneEvent};
use crate::model::{NodeData, NodeKind};
use crate::shell::{system_shell, terminal_shell_args};
use eframe::egui;
use egui_term::{BackendCommand, BackendSettings, PtyEvent, TerminalBackend};
use std::path::PathBuf;

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

        let downstream_ids: Vec<usize> = self
            .edges
            .iter()
            .filter_map(|(from, to)| (*from == node_id).then_some(*to))
            .collect();

        if downstream_ids.is_empty() {
            self.push_toast_notification("无下游节点可接收传递内容");
            return;
        }

        let injected = Self::build_injected_text_block(&text_body);
        let delivered = self.forward_message_to_targets(&downstream_ids, &injected);

        if delivered == 0 {
            self.push_toast_notification("无可接收消息的下游节点（仅支持终端/决策）");
            return;
        }

        self.push_toast_notification(format!("已完成并传递到 {delivered} 个下游节点"));
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
            .map(|script| script.to_owned())
    }

    fn terminal_working_directory(&self, node_id: usize) -> Option<PathBuf> {
        self.nodes
            .iter()
            .find(|n| n.id == node_id)
            .and_then(|n| match &n.data {
                NodeData::Terminal {
                    working_directory, ..
                } => working_directory.as_deref(),
                _ => None,
            })
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
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

    fn forward_message_to_targets(&mut self, target_ids: &[usize], message: &str) -> usize {
        let mut delivered = 0usize;
        for target_id in target_ids {
            if self.forward_message_to_node(*target_id, message) {
                delivered += 1;
            }
        }
        delivered
    }

    fn forward_message_to_node(&mut self, target_id: usize, message: &str) -> bool {
        let Some(kind) = self
            .nodes
            .iter()
            .find(|n| n.id == target_id)
            .map(|n| n.kind.clone())
        else {
            return false;
        };

        match kind {
            NodeKind::Terminal => {
                self.inject_terminal_message_and_submit(target_id, message);
                true
            }
            NodeKind::Decision => self.enqueue_decision_message(target_id, message),
            NodeKind::Text | NodeKind::Html | NodeKind::Image | NodeKind::Group => false,
        }
    }

    fn enqueue_decision_message(&mut self, node_id: usize, message: &str) -> bool {
        let trimmed = message.trim();
        if trimmed.is_empty() {
            return false;
        }

        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            if let NodeData::Decision {
                pending_message,
                pending_messages,
                ..
            } = &mut node.data
            {
                pending_messages.push(trimmed.to_owned());
                *pending_message = pending_messages.first().cloned();
                self.mark_workspace_dirty();
                return true;
            }
        }

        false
    }

    pub(in crate::app) fn decision_pending_queue_len(&self, node_id: usize) -> usize {
        self.nodes
            .iter()
            .find(|n| n.id == node_id)
            .and_then(|n| match &n.data {
                NodeData::Decision {
                    pending_messages,
                    pending_message,
                    ..
                } => {
                    if !pending_messages.is_empty() {
                        Some(pending_messages.len())
                    } else if pending_message
                        .as_deref()
                        .is_some_and(|msg| !msg.trim().is_empty())
                    {
                        Some(1)
                    } else {
                        Some(0)
                    }
                }
                _ => None,
            })
            .unwrap_or(0)
    }

    fn normalize_decision_queue(node: &mut NodeData) {
        if let NodeData::Decision {
            pending_message,
            pending_messages,
            ..
        } = node
        {
            if pending_messages.is_empty()
                && pending_message
                    .as_deref()
                    .is_some_and(|msg| !msg.trim().is_empty())
            {
                pending_messages.push(pending_message.clone().unwrap_or_default());
            }

            pending_messages.retain(|msg| !msg.trim().is_empty());
            *pending_message = pending_messages.first().cloned();
        }
    }

    pub(in crate::app) fn decision_queue_preview(&self, node_id: usize) -> (usize, String) {
        self.nodes
            .iter()
            .find(|n| n.id == node_id)
            .and_then(|n| match &n.data {
                NodeData::Decision {
                    pending_messages,
                    pending_message,
                    ..
                } => {
                    if let Some(first) = pending_messages.first() {
                        Some((pending_messages.len(), first.clone()))
                    } else if let Some(single) = pending_message
                        .as_deref()
                        .map(str::trim)
                        .filter(|msg| !msg.is_empty())
                    {
                        Some((1, single.to_owned()))
                    } else {
                        Some((0, String::new()))
                    }
                }
                _ => None,
            })
            .unwrap_or((0, String::new()))
    }

    pub(in crate::app) fn clear_decision_pending_first(&mut self, node_id: usize) -> bool {
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            Self::normalize_decision_queue(&mut node.data);
            if let NodeData::Decision {
                pending_message,
                pending_messages,
                ..
            } = &mut node.data
            {
                if pending_messages.is_empty() {
                    return false;
                }
                pending_messages.remove(0);
                *pending_message = pending_messages.first().cloned();
                self.mark_workspace_dirty();
                return true;
            }
        }

        false
    }

    pub(in crate::app) fn clear_decision_pending_last(&mut self, node_id: usize) -> bool {
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            Self::normalize_decision_queue(&mut node.data);
            if let NodeData::Decision {
                pending_message,
                pending_messages,
                ..
            } = &mut node.data
            {
                if pending_messages.pop().is_none() {
                    return false;
                }
                *pending_message = pending_messages.first().cloned();
                self.mark_workspace_dirty();
                return true;
            }
        }

        false
    }

    pub(in crate::app) fn clear_decision_pending_all(&mut self, node_id: usize) -> bool {
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            Self::normalize_decision_queue(&mut node.data);
            if let NodeData::Decision {
                pending_message,
                pending_messages,
                ..
            } = &mut node.data
            {
                if pending_messages.is_empty() && pending_message.is_none() {
                    return false;
                }
                pending_messages.clear();
                *pending_message = None;
                self.mark_workspace_dirty();
                return true;
            }
        }

        false
    }

    pub(in crate::app) fn forward_decision_message_by_event(
        &mut self,
        node_id: usize,
        event_key: &str,
        chosen_label: &str,
        process_all: bool,
    ) {
        let queue_len = self.decision_pending_queue_len(node_id);
        if queue_len == 0 {
            self.push_toast_notification("当前无待处理消息");
            return;
        }

        let downstream_ids: Vec<usize> = self
            .edges
            .iter()
            .filter_map(|(from, to)| (*from == node_id).then_some(*to))
            .filter(|to| {
                self.edge_route_key(node_id, *to)
                    .is_some_and(|route| route == event_key)
            })
            .collect();

        if downstream_ids.is_empty() {
            self.push_toast_notification(format!(
                "未找到 route_key = '{event_key}' 的下游连线，消息未丢失"
            ));
            return;
        }

        let mut remaining = queue_len;
        let mut delivered_messages = 0usize;

        while remaining > 0 {
            let maybe_message = self
                .nodes
                .iter_mut()
                .find(|n| n.id == node_id)
                .and_then(|n| {
                    Self::normalize_decision_queue(&mut n.data);
                    if let NodeData::Decision {
                        pending_message,
                        pending_messages,
                        ..
                    } = &mut n.data
                    {
                        if pending_messages.is_empty() {
                            return None;
                        }
                        let message = pending_messages.remove(0);
                        *pending_message = pending_messages.first().cloned();
                        Some(message)
                    } else {
                        None
                    }
                });

            let Some(message) = maybe_message else {
                break;
            };

            let delivered = self.forward_message_to_targets(&downstream_ids, message.trim());
            if delivered > 0 {
                delivered_messages += 1;
            }

            remaining -= 1;
            if !process_all {
                break;
            }
        }

        self.mark_workspace_dirty();

        if delivered_messages == 0 {
            self.push_toast_notification(
                "匹配连线存在，但无可接收消息的下游节点（仅支持终端/决策）",
            );
            return;
        }

        if process_all {
            self.push_toast_notification(format!(
                "已按 '{}' ({event_key}) 一次处理 {delivered_messages} 条消息",
                chosen_label
            ));
        } else {
            self.push_toast_notification(format!(
                "已按 '{}' ({event_key}) 分流 1 条消息",
                chosen_label
            ));
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

        let downstream_ids: Vec<usize> = self
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
            .collect();

        if downstream_ids.is_empty() {
            if let Some(expected) = route_key {
                self.push_toast_notification(format!("未找到 route_key = '{expected}' 的下游连线"));
            }
            return;
        }

        let injected = Self::build_injected_text_block(&event.summary);
        let delivered = self.forward_message_to_targets(&downstream_ids, &injected);
        if delivered == 0 {
            self.push_toast_notification("匹配到连线，但无可接收消息的下游节点（仅支持终端/决策）");
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
                working_directory: self
                    .terminal_working_directory(node_id)
                    .or_else(|| std::env::current_dir().ok()),
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

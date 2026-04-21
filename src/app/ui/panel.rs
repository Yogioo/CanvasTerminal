use super::super::GraphApp;
use crate::model::NodeKind;
use eframe::egui::{self, Color32, ScrollArea, TextEdit, Ui};

impl GraphApp {
    pub(in crate::app) fn draw_service_panel(&mut self, ui: &mut Ui) {
        ui.heading("节点数据面板");
        ui.separator();

        if let Some(id) = self.selected {
            if let Some(node) = self.nodes.iter_mut().find(|n| n.id == id) {
                ui.label(format!("节点名称: {}", node.title));
                ui.label(format!("节点 ID: {}", node.id));
                ui.label(format!("节点类型: {}", Self::node_kind_name(&node.kind)));
                ui.label(format!("分类: {}", node.category));
                ui.label(format!("状态: {}", node.status));

                if node.kind == NodeKind::Terminal {
                    ui.separator();
                    ui.label("节点身份:");
                    ui.text_edit_singleline(&mut node.identity);
                    ui.small("terminal 内可直接执行: canvas done \"...\"");
                }

                if node.kind == NodeKind::Text {
                    ui.separator();
                    ui.label("文本内容:");
                    ui.add_sized(
                        [ui.available_width(), 120.0],
                        TextEdit::multiline(&mut node.text_body),
                    );
                    if ui.button("进入画布内编辑模式").clicked() {
                        self.editing_text_node = Some(node.id);
                        self.pending_text_focus = Some(node.id);
                    }
                }

                ui.separator();
                ui.small("提示：支持节点右键 -> 创建节点（终端/文本）。");
                ui.small("也支持把图片文件拖拽到画布，或在画布内粘贴图片/图片路径来创建图片节点。");
                ui.small("若 Ctrl+V 被系统拦截，可在画布中按 F6（或直接按 V）强制读取系统剪贴板。");
                ui.small("空白处双击可快速创建文本节点，且自动进入编辑模式。");
                ui.small("滚轮或触控板双指捏合可缩放画布视图。");
            }
        }

        self.draw_history_panel(ui);
    }

    pub(in crate::app) fn draw_terminal_hint_panel(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let Some(node_id) = self.selected_terminal_id() else {
            return;
        };

        ui.heading("Terminal 节点");
        ui.separator();
        let identity = self
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .map(|n| n.identity.as_str())
            .unwrap_or("agent");

        ui.label("终端现在直接嵌入在画布中的 Terminal 节点内部。\n拖拽 Terminal 节点顶部可移动它。");
        ui.label(format!("Identity: {identity}"));
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            ui.label("编辑身份:");
            ui.text_edit_singleline(&mut node.identity);
        }
        ui.small("修改 identity 后，点击“重启终端”使环境变量生效。\n可在终端中执行: canvas done \"已完成...\"");

        if ui.button("重启终端").clicked() {
            self.restart_terminal(node_id, ctx);
        }

        ui.separator();
        if self.terminal_backends.contains_key(&node_id) {
            ui.label(egui::RichText::new("● Running").color(Color32::LIGHT_GREEN));
        } else if self.terminal_exited.contains(&node_id) {
            ui.label(egui::RichText::new("● Exited").color(Color32::LIGHT_RED));
        } else {
            ui.label(egui::RichText::new("● Starting...").color(Color32::YELLOW));
        }

        if let Some(err) = self.terminal_errors.get(&node_id) {
            ui.colored_label(Color32::LIGHT_RED, err);
        }
        if let Some(err) = &self.done_event_error {
            ui.colored_label(Color32::LIGHT_RED, err);
        }

        self.draw_history_panel(ui);
    }

    fn draw_history_panel(&mut self, ui: &mut Ui) {
        ui.separator();
        ui.horizontal(|ui| {
            let can_undo = !self.undo_stack.is_empty();
            if ui
                .add_enabled(can_undo, egui::Button::new("撤销 (Ctrl+Z)"))
                .clicked()
            {
                self.undo_last_change();
            }
            ui.small(format!("可撤销操作: {}", self.undo_stack.len()));
        });

        ui.label("修改历史（删除/移动）");
        ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
            if self.change_history.is_empty() {
                ui.small("暂无历史记录");
            } else {
                for item in self.change_history.iter().rev().take(30) {
                    ui.small(item);
                }
            }
        });
    }
}

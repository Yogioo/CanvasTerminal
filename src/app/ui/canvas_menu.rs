use super::super::{GraphApp, NodeOrderAction};
use eframe::egui::{self, Pos2};

impl GraphApp {
    fn run_create_action(&mut self, action_id: usize, spawn_pos: Pos2) {
        match action_id {
            0 => self.create_terminal_node(spawn_pos),
            1 => self.create_text_node(spawn_pos, true),
            _ => {}
        }
    }

    fn context_menu_spawn_pos(&self) -> Pos2 {
        self.context_menu_local_pos.unwrap_or(Pos2::new(100.0, 100.0))
    }

    fn run_node_order_action(&mut self, node_id: usize, action: NodeOrderAction) {
        self.reorder_from_context(node_id, action);
    }

    pub(in crate::app::ui) fn show_canvas_context_menu(&mut self, response: &egui::Response, ctx: &egui::Context) {
        response.context_menu(|ui| {
            if let Some(node_id) = self.context_menu_node {
                ui.label(format!("节点 #{node_id}"));
                ui.separator();

                let is_terminal_node = self
                    .nodes
                    .iter()
                    .find(|n| n.id == node_id)
                    .is_some_and(|n| matches!(n.kind, crate::model::NodeKind::Terminal));

                if is_terminal_node && ui.button("编辑启动命令").clicked() {
                    self.start_startup_edit(node_id);
                    ui.close_menu();
                }

                if ui.button("置于顶层").clicked() {
                    self.run_node_order_action(node_id, NodeOrderAction::BringToFront);
                    ui.close_menu();
                }
                if ui.button("上移一层").clicked() {
                    self.run_node_order_action(node_id, NodeOrderAction::BringForwardOne);
                    ui.close_menu();
                }
                if ui.button("下移一层").clicked() {
                    self.run_node_order_action(node_id, NodeOrderAction::SendBackwardOne);
                    ui.close_menu();
                }
                if ui.button("置于底层").clicked() {
                    self.run_node_order_action(node_id, NodeOrderAction::SendToBack);
                    ui.close_menu();
                }
                return;
            }

            let spawn_pos = self.context_menu_spawn_pos();
            let items = [
                ("创建节点/终端节点", "终端节点", 0usize),
                ("创建节点/文本节点", "文本节点", 1usize),
            ];

            if let Some(action_id) = self.show_searchable_menu_actions(
                ui,
                ctx,
                egui::Id::new("context_menu_search_input"),
                "搜索并创建节点...",
                &items,
                "无匹配节点类型",
                "↑/↓ 选择，Enter 创建",
            ) {
                self.run_create_action(action_id, spawn_pos);
                ui.close_menu();
            }
        });
    }
}

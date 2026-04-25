use super::super::{GraphApp, NodeOrderAction};
use eframe::egui::{self, Color32, Pos2};

impl GraphApp {
    fn run_create_action(&mut self, action_id: usize, spawn_pos: Pos2) {
        match action_id {
            0 => {
                self.create_terminal_node(spawn_pos);
            }
            1 => {
                self.create_text_node(spawn_pos, true);
            }
            2 => {
                self.create_decision_node(spawn_pos);
            }
            _ => {}
        }
    }

    fn context_menu_spawn_pos(&self) -> Pos2 {
        self.context_menu_local_pos
            .unwrap_or(Pos2::new(100.0, 100.0))
    }

    fn run_node_order_action(&mut self, node_id: usize, action: NodeOrderAction) {
        self.reorder_from_context(node_id, action);
    }

    pub(in crate::app::ui) fn show_canvas_context_menu(
        &mut self,
        response: &egui::Response,
        ctx: &egui::Context,
    ) {
        response.context_menu(|ui| {
            let visuals = ui.visuals_mut();
            visuals.window_fill = Color32::from_rgb(245, 245, 245);
            visuals.panel_fill = Color32::from_rgb(245, 245, 245);
            visuals.extreme_bg_color = Color32::from_rgb(255, 255, 255);
            visuals.override_text_color = Some(Color32::from_rgb(20, 20, 20));
            visuals.widgets.noninteractive.fg_stroke.color = Color32::from_rgb(20, 20, 20);
            visuals.widgets.inactive.fg_stroke.color = Color32::from_rgb(20, 20, 20);
            visuals.widgets.hovered.fg_stroke.color = Color32::from_rgb(20, 20, 20);
            visuals.widgets.active.fg_stroke.color = Color32::from_rgb(20, 20, 20);
            visuals.widgets.open.fg_stroke.color = Color32::from_rgb(20, 20, 20);

            if let Some(node_id) = self.context_menu_node {
                ui.label(format!("节点 #{node_id}"));
                ui.separator();

                let node_state = self.nodes.iter().find(|n| n.id == node_id).map(|n| {
                    let is_terminal = matches!(n.kind, crate::model::NodeKind::Terminal);
                    let is_text = matches!(n.kind, crate::model::NodeKind::Text);
                    let is_decision = matches!(n.kind, crate::model::NodeKind::Decision);
                    (is_terminal, is_text, is_decision)
                });

                let is_terminal_node = node_state.is_some_and(|(is_terminal, _, _)| is_terminal);
                let is_text_node = node_state.is_some_and(|(_, is_text, _)| is_text);
                let is_decision_node = node_state.is_some_and(|(_, _, is_decision)| is_decision);

                if is_terminal_node && ui.button("编辑启动命令").clicked() {
                    self.start_startup_edit(node_id);
                    ui.close_menu();
                }

                if is_text_node && ui.button("完成并传递").clicked() {
                    self.complete_text_node_and_forward(node_id);
                    ui.close_menu();
                }

                if is_decision_node {
                    ui.separator();
                    ui.label("待处理队列");

                    if ui.button("清空前一个（队首）").clicked() {
                        if self.clear_decision_pending_first(node_id) {
                            self.push_toast_notification("已清空 1 条队首消息");
                        } else {
                            self.push_toast_notification("当前无待处理消息");
                        }
                        ui.close_menu();
                    }

                    if ui.button("清空后一个（队尾）").clicked() {
                        if self.clear_decision_pending_last(node_id) {
                            self.push_toast_notification("已清空 1 条队尾消息");
                        } else {
                            self.push_toast_notification("当前无待处理消息");
                        }
                        ui.close_menu();
                    }

                    if ui.button("清空全部").clicked() {
                        if self.clear_decision_pending_all(node_id) {
                            self.push_toast_notification("已清空全部待处理消息");
                        } else {
                            self.push_toast_notification("当前无待处理消息");
                        }
                        ui.close_menu();
                    }
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

            if let Some(edge) = self.context_menu_edge {
                ui.label(format!("连线 {} → {}", edge.0, edge.1));
                ui.separator();

                let can_reset_curve = self.edge_has_custom_curve(edge.0, edge.1);
                if ui
                    .add_enabled(can_reset_curve, egui::Button::new("重置为默认曲率"))
                    .clicked()
                {
                    self.set_edge_selection(edge);
                    self.reset_selected_edge_curve();
                    ui.close_menu();
                }
                return;
            }

            let spawn_pos = self.context_menu_spawn_pos();
            let items = [
                ("创建节点/终端节点", "终端节点", 0usize),
                ("创建节点/文本节点", "文本节点", 1usize),
                ("创建节点/决策节点", "决策节点", 2usize),
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

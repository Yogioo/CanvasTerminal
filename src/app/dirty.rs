use super::GraphApp;
use eframe::egui::{self, vec2, Color32, CornerRadius};

impl GraphApp {
    pub(in crate::app) fn mark_workspace_dirty(&mut self) {
        self.workspace_dirty = true;
        self.bump_automation_state_version();
    }

    pub(in crate::app) fn mark_workspace_clean(&mut self) {
        self.workspace_dirty = false;
    }

    fn workspace_window_title(&self) -> String {
        if self.workspace_dirty {
            "* CanvasTerminal".to_owned()
        } else {
            "CanvasTerminal".to_owned()
        }
    }

    pub(in crate::app) fn apply_workspace_dirty_ui(&mut self, ctx: &egui::Context) {
        if self.last_title_dirty == Some(self.workspace_dirty) {
            return;
        }

        self.last_title_dirty = Some(self.workspace_dirty);
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.workspace_window_title()));
    }

    pub(in crate::app) fn show_workspace_dirty_indicator(&self, ctx: &egui::Context) {
        if !self.workspace_dirty {
            return;
        }

        let screen_rect = ctx.screen_rect();
        let radius = 5.0;
        let margin = vec2(12.0, 12.0);
        let pos = egui::pos2(
            screen_rect.right() - margin.x - radius,
            screen_rect.bottom() - margin.y - radius,
        );

        egui::Area::new("workspace_dirty_indicator".into())
            .order(egui::Order::Foreground)
            .fixed_pos(pos - vec2(radius, radius))
            .show(ctx, |ui| {
                let (rect, _) =
                    ui.allocate_exact_size(vec2(radius * 2.0, radius * 2.0), egui::Sense::hover());
                ui.painter().rect_filled(
                    rect,
                    CornerRadius::same(radius as u8),
                    Color32::from_rgb(88, 140, 255),
                );
            });
    }
}

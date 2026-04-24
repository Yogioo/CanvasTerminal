use super::GraphApp;
use eframe::egui::{self, vec2, Color32, CornerRadius, RichText};
use std::time::{Duration, Instant};

pub(in crate::app) struct ToastNotification {
    id: u64,
    message: String,
    created_at: Instant,
    visible_for: Duration,
    fade_in: Duration,
    fade_out: Duration,
}

impl ToastNotification {
    fn new(id: u64, message: String) -> Self {
        Self {
            id,
            message,
            created_at: Instant::now(),
            visible_for: Duration::from_millis(2200),
            fade_in: Duration::from_millis(180),
            fade_out: Duration::from_millis(260),
        }
    }

    fn alpha(&self, now: Instant) -> f32 {
        let elapsed = now.saturating_duration_since(self.created_at).as_secs_f32();
        let fade_in = self.fade_in.as_secs_f32().max(0.001);
        let visible = self.visible_for.as_secs_f32();
        let fade_out = self.fade_out.as_secs_f32().max(0.001);

        if elapsed < fade_in {
            return (elapsed / fade_in).clamp(0.0, 1.0);
        }

        let fade_out_start = fade_in + visible;
        if elapsed < fade_out_start {
            return 1.0;
        }

        let fade_out_elapsed = elapsed - fade_out_start;
        (1.0 - fade_out_elapsed / fade_out).clamp(0.0, 1.0)
    }

    fn is_expired(&self, now: Instant) -> bool {
        let lifetime = self.fade_in + self.visible_for + self.fade_out;
        now.saturating_duration_since(self.created_at) >= lifetime
    }
}

impl GraphApp {
    pub(in crate::app) fn push_toast_notification(&mut self, message: impl Into<String>) {
        let id = self.next_toast_id;
        self.next_toast_id += 1;
        self.toast_notifications
            .push(ToastNotification::new(id, message.into()));
    }

    pub(in crate::app) fn show_toast_notifications(&mut self, ctx: &egui::Context) {
        if self.toast_notifications.is_empty() {
            return;
        }

        let now = Instant::now();
        self.toast_notifications.retain(|item| !item.is_expired(now));
        if self.toast_notifications.is_empty() {
            return;
        }

        let spacing = 8.0;
        let max_width = 420.0;
        let min_width = 220.0;
        let toast_height = 34.0;
        let margin = vec2(18.0, 18.0);
        let screen_rect = ctx.screen_rect();

        for (idx, toast) in self.toast_notifications.iter().rev().enumerate() {
            let alpha = toast.alpha(now);
            if alpha <= 0.0 {
                continue;
            }

            let bg = Color32::from_rgba_unmultiplied(32, 36, 47, (220.0 * alpha) as u8);
            let border = Color32::from_rgba_unmultiplied(108, 132, 255, (180.0 * alpha) as u8);
            let text = Color32::from_rgba_unmultiplied(245, 247, 255, (255.0 * alpha) as u8);

            let y = screen_rect.bottom()
                - margin.y
                - toast_height
                - idx as f32 * (toast_height + spacing);
            let x = (screen_rect.right() - margin.x - max_width).max(screen_rect.left() + margin.x);

            egui::Area::new(egui::Id::new(("toast_notification", toast.id)))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(x, y))
                .show(ctx, |ui| {
                    let frame = egui::Frame::new()
                        .fill(bg)
                        .stroke(egui::Stroke::new(1.0, border))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(12, 8));

                    frame.show(ui, |ui| {
                        ui.set_min_width(min_width);
                        ui.set_max_width(max_width);
                        ui.label(RichText::new(&toast.message).color(text));
                    });
                });
        }

        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

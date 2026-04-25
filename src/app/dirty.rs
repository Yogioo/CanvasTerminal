use super::GraphApp;
use eframe::egui::{self, vec2, Color32, CornerRadius};
use std::path::Path;

impl GraphApp {
    const MAX_WORKSPACE_NAME_CHARS: usize = 80;

    pub(in crate::app) fn default_workspace_name() -> &'static str {
        "未命名画布"
    }

    pub(in crate::app) fn normalize_workspace_name(raw: &str) -> String {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Self::default_workspace_name().to_owned();
        }

        trimmed
            .chars()
            .take(Self::MAX_WORKSPACE_NAME_CHARS)
            .collect::<String>()
    }

    pub(in crate::app) fn derive_workspace_name_from_path(path: Option<&Path>) -> String {
        let derived = path
            .and_then(|value| value.file_stem().or_else(|| value.file_name()))
            .map(|name| name.to_string_lossy().trim().to_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| Self::default_workspace_name().to_owned());

        Self::normalize_workspace_name(&derived)
    }

    pub(in crate::app) fn resolve_workspace_name(
        stored_name: Option<&str>,
        fallback_path: Option<&Path>,
    ) -> String {
        match stored_name {
            Some(name) => Self::normalize_workspace_name(name),
            None => Self::derive_workspace_name_from_path(fallback_path),
        }
    }

    pub(in crate::app) fn workspace_name(&self) -> &str {
        &self.workspace_name
    }

    pub(in crate::app) fn set_workspace_name(&mut self, name: &str) {
        self.workspace_name = Self::normalize_workspace_name(name);
    }

    fn workspace_name_submit_result(current: &str, draft: &str) -> (String, bool) {
        let normalized = Self::normalize_workspace_name(draft);
        let changed = normalized != current;
        (normalized, changed)
    }

    pub(in crate::app) fn start_workspace_name_edit(&mut self) {
        self.editing_workspace_name = true;
        self.pending_workspace_name_focus = true;
        self.workspace_name_edit_buffer = self.workspace_name().to_owned();
    }

    pub(in crate::app) fn commit_workspace_name_edit(&mut self) {
        let (next_name, changed) =
            Self::workspace_name_submit_result(self.workspace_name(), &self.workspace_name_edit_buffer);
        if changed {
            self.workspace_name = next_name;
            self.mark_workspace_dirty();
        }
        self.cancel_workspace_name_edit();
    }

    pub(in crate::app) fn cancel_workspace_name_edit(&mut self) {
        self.editing_workspace_name = false;
        self.pending_workspace_name_focus = false;
        self.workspace_name_edit_buffer.clear();
    }

    pub(in crate::app) fn mark_workspace_dirty(&mut self) {
        self.workspace_dirty = true;
        self.bump_automation_state_version();
    }

    pub(in crate::app) fn mark_workspace_clean(&mut self) {
        self.workspace_dirty = false;
    }

    fn workspace_window_title(&self) -> String {
        Self::format_workspace_window_title(self.workspace_name(), self.workspace_dirty)
    }

    fn format_workspace_window_title(workspace_name: &str, workspace_dirty: bool) -> String {
        let prefix = if workspace_dirty { "* " } else { "" };
        format!("{prefix}{workspace_name} - CanvasTerminal")
    }

    pub(in crate::app) fn apply_workspace_dirty_ui(&mut self, ctx: &egui::Context) {
        let title = self.workspace_window_title();
        if self.last_window_title.as_ref() == Some(&title) {
            return;
        }

        self.last_window_title = Some(title.clone());
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn workspace_name_defaults_when_blank() {
        assert_eq!(GraphApp::normalize_workspace_name(""), "未命名画布");
        assert_eq!(GraphApp::normalize_workspace_name("   \t\n"), "未命名画布");
    }

    #[test]
    fn workspace_name_is_trimmed_and_capped() {
        let raw = format!("  {}  ", "x".repeat(160));
        let normalized = GraphApp::normalize_workspace_name(&raw);
        assert_eq!(normalized.chars().count(), 80);
        assert_eq!(normalized, "x".repeat(80));
    }

    #[test]
    fn workspace_name_falls_back_to_path_stem_for_legacy_configs() {
        let path = Path::new("/tmp/test.json");
        assert_eq!(
            GraphApp::resolve_workspace_name(None, Some(path)),
            "test"
        );
    }

    #[test]
    fn workspace_title_uses_workspace_name_and_dirty_prefix() {
        assert_eq!(
            GraphApp::format_workspace_window_title("画布A", false),
            "画布A - CanvasTerminal"
        );
        assert_eq!(
            GraphApp::format_workspace_window_title("画布A", true),
            "* 画布A - CanvasTerminal"
        );
    }

    #[test]
    fn window_bar_rename_blank_submit_falls_back_to_default() {
        let (next, changed) = GraphApp::workspace_name_submit_result("画布A", "   ");
        assert_eq!(next, GraphApp::default_workspace_name());
        assert!(changed);
    }

    #[test]
    fn window_bar_rename_same_name_keeps_clean_state_signal() {
        let (next, changed) = GraphApp::workspace_name_submit_result("画布A", "  画布A  ");
        assert_eq!(next, "画布A");
        assert!(!changed);
    }
}

use super::GraphApp;
use eframe::egui::{self, vec2, Pos2, Rect};
use std::time::Duration;

impl GraphApp {
    pub(in crate::app) fn handle_global_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::U)) {
            self.redo_last_change();
        } else if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z)) {
            self.undo_last_change();
        }

        if ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::S)) {
            self.run_file_menu_action(2);
        } else if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
            self.run_file_menu_action(0);
        }

        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::R)) {
            self.run_file_menu_action(1);
        }

        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O)) {
            self.run_file_menu_action(3);
        }

        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::P)) {
            self.command_palette_open = true;
            self.reset_menu_search_state(true);
        }
    }

    pub(in crate::app) fn update_window_bar_visibility(
        &mut self,
        ctx: &egui::Context,
    ) -> (f64, Rect, bool, bool) {
        let now = ctx.input(|i| i.time);
        let screen_rect = ctx.screen_rect();
        let pointer_near_top = ctx.input(|i| {
            i.pointer
                .latest_pos()
                .or_else(|| i.pointer.hover_pos())
                .is_some_and(|p| p.y <= screen_rect.top() + 32.0)
        });
        let any_popup_open = ctx.memory(|m| m.any_popup_open());
        let keep_top_bar = any_popup_open;

        if pointer_near_top || keep_top_bar {
            self.window_bar_visible_until = now + 1.0;
        }
        let show_window_bar = pointer_near_top || keep_top_bar || now <= self.window_bar_visible_until;

        (now, screen_rect, pointer_near_top, show_window_bar)
    }

    pub(in crate::app) fn draw_window_controls_overlay(&mut self, ctx: &egui::Context, screen_rect: Rect) {
        egui::Area::new("window_drag_bar_overlay".into())
            .order(egui::Order::Foreground)
            .fixed_pos(screen_rect.min)
            .show(ctx, |ui| {
                let bar_height = 28.0;
                let (bar_rect, _) =
                    ui.allocate_exact_size(vec2(screen_rect.width(), bar_height), egui::Sense::hover());

                let button_size = vec2(24.0, 18.0);
                let button_gap = 6.0;
                let right_pad = 12.0;
                let top = bar_rect.center().y - button_size.y * 0.5;

                let close_rect = Rect::from_min_size(
                    Pos2::new(bar_rect.right() - right_pad - button_size.x, top),
                    button_size,
                );
                let maxim_rect = close_rect.translate(vec2(-(button_size.x + button_gap), 0.0));
                let minim_rect = maxim_rect.translate(vec2(-(button_size.x + button_gap), 0.0));

                let drag_left = bar_rect.left() + 8.0;
                let drag_right = (minim_rect.left() - 8.0).max(drag_left);
                let drag_rect = Rect::from_min_max(
                    Pos2::new(drag_left, bar_rect.top()),
                    Pos2::new(drag_right, bar_rect.bottom()),
                );

                let drag_response = ui.interact(
                    drag_rect,
                    ui.id().with("window_drag_area"),
                    egui::Sense::click_and_drag(),
                );
                let start_drag = ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
                if drag_response.hovered() {
                    ui.output_mut(|o| {
                        o.cursor_icon = if drag_response.dragged() {
                            egui::CursorIcon::Grabbing
                        } else {
                            egui::CursorIcon::Grab
                        };
                    });
                }
                if drag_response.double_clicked() {
                    let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                } else if drag_response.hovered() && start_drag {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                let stroke = egui::Stroke::new(1.5, egui::Color32::WHITE);

                let minim = ui.interact(
                    minim_rect,
                    ui.id().with("window_minimize_button"),
                    egui::Sense::click(),
                );
                let min_y = minim_rect.center().y;
                ui.painter().line_segment(
                    [
                        Pos2::new(minim_rect.center().x - 5.0, min_y),
                        Pos2::new(minim_rect.center().x + 5.0, min_y),
                    ],
                    stroke,
                );
                if minim.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }

                let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                let maxim = ui.interact(
                    maxim_rect,
                    ui.id().with("window_maximize_button"),
                    egui::Sense::click(),
                );
                if is_maximized {
                    let back =
                        Rect::from_center_size(maxim_rect.center() + vec2(-1.5, -1.5), vec2(8.5, 7.5));
                    let front =
                        Rect::from_center_size(maxim_rect.center() + vec2(1.5, 1.5), vec2(8.5, 7.5));
                    ui.painter()
                        .rect_stroke(back, 0.0, stroke, egui::StrokeKind::Inside);
                    ui.painter()
                        .rect_stroke(front, 0.0, stroke, egui::StrokeKind::Inside);
                } else {
                    let square = Rect::from_center_size(maxim_rect.center(), vec2(9.0, 9.0));
                    ui.painter()
                        .rect_stroke(square, 0.0, stroke, egui::StrokeKind::Inside);
                }
                if maxim.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                }

                let close = ui.interact(
                    close_rect,
                    ui.id().with("window_close_button"),
                    egui::Sense::click(),
                );
                ui.painter().line_segment(
                    [
                        close_rect.center() + vec2(-4.5, -4.5),
                        close_rect.center() + vec2(4.5, 4.5),
                    ],
                    stroke,
                );
                ui.painter().line_segment(
                    [
                        close_rect.center() + vec2(-4.5, 4.5),
                        close_rect.center() + vec2(4.5, -4.5),
                    ],
                    stroke,
                );
                if close.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
    }

    pub(in crate::app) fn show_command_palette_if_open(&mut self, ctx: &egui::Context) {
        if !self.command_palette_open {
            return;
        }

        let mut action_triggered = false;
        let palette_window = egui::Window::new("命令面板")
            .id(egui::Id::new("command_palette_window"))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .movable(false)
            .anchor(egui::Align2::CENTER_TOP, vec2(0.0, 40.0))
            .show(ctx, |ui| {
                ui.set_min_width(460.0);

                let palette_items = [
                    ("文件/快速保存", "文件/快速保存  Ctrl+S", 0usize),
                    ("文件/快速加载", "文件/快速加载  Ctrl+R", 1usize),
                    ("文件/另存为", "文件/另存为  Ctrl+Shift+S", 2usize),
                    ("文件/加载", "文件/加载  Ctrl+O", 3usize),
                    ("编辑/撤销", "编辑/撤销  Ctrl+Z", 100usize),
                    ("编辑/重做", "编辑/重做  Ctrl+U", 101usize),
                ];

                if let Some(action_id) = self.show_searchable_menu_actions(
                    ui,
                    ctx,
                    egui::Id::new("command_palette_search_input"),
                    "输入命令...",
                    &palette_items,
                    "无匹配命令",
                    "Ctrl+P 打开，Esc 关闭，↑/↓ 选择，Enter 执行",
                ) {
                    if action_id < 100 {
                        self.run_file_menu_action(action_id);
                    } else {
                        self.run_edit_menu_action(action_id - 100);
                    }
                    action_triggered = true;
                }
            });

        let popup_rect = palette_window.as_ref().map(|window| window.response.rect);
        if self.should_close_popup(ctx, popup_rect, action_triggered) {
            self.command_palette_open = false;
            self.reset_menu_search_state(false);
        }
    }

    pub(in crate::app) fn schedule_repaint(
        &mut self,
        ctx: &egui::Context,
        show_window_bar: bool,
        pointer_near_top: bool,
        now: f64,
    ) {
        if show_window_bar && !pointer_near_top {
            let remaining = (self.window_bar_visible_until - now).max(0.0);
            if remaining > 0.0 {
                ctx.request_repaint_after(Duration::from_secs_f64(remaining.min(0.1)));
            }
        }

        if !self.pending_terminal_starts.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
}

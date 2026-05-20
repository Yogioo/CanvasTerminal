use super::GraphApp;
use crate::constants::WINDOW_RESIZE_BORDER;
use eframe::egui::{self, vec2, Pos2, Rect};
use std::time::Duration;

impl GraphApp {
    pub(in crate::app) fn handle_global_shortcuts(&mut self, ctx: &egui::Context) {
        if self.editing_text_node.is_some() || self.editing_script_node.is_some() {
            return;
        }

        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::U)) {
            self.redo_last_change();
        } else if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z)) {
            self.undo_last_change();
        }

        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::N)) {
            self.run_file_menu_action(4);
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

        let node_clipboard_shortcut_allowed = self.editing_text_node.is_none()
            && self.editing_script_node.is_none()
            && self.editing_title_node.is_none()
            && self.editing_startup_node.is_none()
            && self.editing_working_directory_node.is_none()
            && self.editing_decision_buttons_node.is_none()
            && self.editing_decision_queue_node.is_none()
            && self.editing_edge.is_none()
            && !self.command_palette_open;
        let copy_shortcut_pressed = ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::C,
            ))
        }) || ctx.input(|i| i.events.iter().any(|event| matches!(event, egui::Event::Copy)));

        if node_clipboard_shortcut_allowed && copy_shortcut_pressed {
            if self.copy_selected_nodes_to_internal_clipboard() {
                self.push_toast_notification("已复制节点");
            }
        }

        if node_clipboard_shortcut_allowed
            && ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::G))
        {
            if self.create_group_from_selection().is_some() {
                self.push_toast_notification("已创建分组");
            } else {
                self.push_toast_notification("创建分组失败：请选中至少 2 个节点，且不能与现有组成员完全一致");
            }
        }

        if ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::M)) {
            self.performance_metrics.toggle_visible();
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
        let show_window_bar =
            pointer_near_top || keep_top_bar || now <= self.window_bar_visible_until;

        (now, screen_rect, pointer_near_top, show_window_bar)
    }

    fn split_drag_regions(
        bar_rect: Rect,
        drag_left: f32,
        drag_right: f32,
        title_rect: Rect,
    ) -> (Option<Rect>, Option<Rect>) {
        let left_max = (title_rect.left() - 4.0).min(drag_right);
        let right_min = (title_rect.right() + 4.0).max(drag_left);

        let left_region = (left_max - drag_left > 1.0).then(|| {
            Rect::from_min_max(
                Pos2::new(drag_left, bar_rect.top()),
                Pos2::new(left_max, bar_rect.bottom()),
            )
        });
        let right_region = (drag_right - right_min > 1.0).then(|| {
            Rect::from_min_max(
                Pos2::new(right_min, bar_rect.top()),
                Pos2::new(drag_right, bar_rect.bottom()),
            )
        });

        (left_region, right_region)
    }

    pub(in crate::app) fn draw_window_controls_overlay(
        &mut self,
        ctx: &egui::Context,
        screen_rect: Rect,
        show_window_bar: bool,
    ) {
        egui::Area::new("window_drag_bar_overlay".into())
            .order(egui::Order::Foreground)
            .interactable(true)
            .fixed_pos(screen_rect.min)
            .show(ctx, |ui| {
                // Keep the foreground overlay as small as possible.
                // A full-window foreground Area can interfere with mouse hit-testing
                // for canvas editors below it (TextEdit hover, drag-select, scrollbars).
                let bar_height = 28.0;
                let bar_rect = Rect::from_min_max(
                    screen_rect.min,
                    Pos2::new(screen_rect.right(), screen_rect.top() + bar_height),
                );

                let button_size = vec2(24.0, 18.0);
                let button_gap = 6.0;
                let right_pad = 12.0;
                let top = bar_rect.center().y - button_size.y * 0.5;

                let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                if !is_maximized {
                    let border = WINDOW_RESIZE_BORDER;
                    let left_edge = Rect::from_min_max(
                        Pos2::new(screen_rect.left(), screen_rect.top() + border),
                        Pos2::new(screen_rect.left() + border, screen_rect.bottom() - border),
                    );
                    let right_edge = Rect::from_min_max(
                        Pos2::new(screen_rect.right() - border, screen_rect.top() + border),
                        Pos2::new(screen_rect.right(), screen_rect.bottom() - border),
                    );
                    let top_edge = Rect::from_min_max(
                        Pos2::new(screen_rect.left() + border, screen_rect.top()),
                        Pos2::new(screen_rect.right() - border, screen_rect.top() + border),
                    );
                    let bottom_edge = Rect::from_min_max(
                        Pos2::new(screen_rect.left() + border, screen_rect.bottom() - border),
                        Pos2::new(screen_rect.right() - border, screen_rect.bottom()),
                    );
                    let nw_corner = Rect::from_min_max(
                        Pos2::new(screen_rect.left(), screen_rect.top()),
                        Pos2::new(screen_rect.left() + border, screen_rect.top() + border),
                    );
                    let ne_corner = Rect::from_min_max(
                        Pos2::new(screen_rect.right() - border, screen_rect.top()),
                        Pos2::new(screen_rect.right(), screen_rect.top() + border),
                    );
                    let sw_corner = Rect::from_min_max(
                        Pos2::new(screen_rect.left(), screen_rect.bottom() - border),
                        Pos2::new(screen_rect.left() + border, screen_rect.bottom()),
                    );
                    let se_corner = Rect::from_min_max(
                        Pos2::new(screen_rect.right() - border, screen_rect.bottom() - border),
                        Pos2::new(screen_rect.right(), screen_rect.bottom()),
                    );

                    let resize_regions = [
                        ("window_resize_nw", nw_corner, egui::ResizeDirection::NorthWest, egui::CursorIcon::ResizeNwSe),
                        ("window_resize_ne", ne_corner, egui::ResizeDirection::NorthEast, egui::CursorIcon::ResizeNeSw),
                        ("window_resize_sw", sw_corner, egui::ResizeDirection::SouthWest, egui::CursorIcon::ResizeNeSw),
                        ("window_resize_se", se_corner, egui::ResizeDirection::SouthEast, egui::CursorIcon::ResizeNwSe),
                        ("window_resize_n", top_edge, egui::ResizeDirection::North, egui::CursorIcon::ResizeVertical),
                        ("window_resize_s", bottom_edge, egui::ResizeDirection::South, egui::CursorIcon::ResizeVertical),
                        ("window_resize_w", left_edge, egui::ResizeDirection::West, egui::CursorIcon::ResizeHorizontal),
                        ("window_resize_e", right_edge, egui::ResizeDirection::East, egui::CursorIcon::ResizeHorizontal),
                    ];

                    for (id, rect, direction, cursor) in resize_regions {
                        let response = ui.interact(rect, ui.id().with(id), egui::Sense::click_and_drag());
                        if response.hovered() {
                            ui.output_mut(|o| o.cursor_icon = cursor);
                        }
                        if response.hovered()
                            && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary))
                        {
                            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
                            break;
                        }
                    }
                }

                if !show_window_bar {
                    return;
                }

                let close_rect = Rect::from_min_size(
                    Pos2::new(bar_rect.right() - right_pad - button_size.x, top),
                    button_size,
                );
                let maxim_rect = close_rect.translate(vec2(-(button_size.x + button_gap), 0.0));
                let minim_rect = maxim_rect.translate(vec2(-(button_size.x + button_gap), 0.0));

                let drag_left = bar_rect.left() + 8.0;
                let drag_right = (minim_rect.left() - 8.0).max(drag_left);
                let title_font = egui::FontId::proportional(13.0);
                let title_color = egui::Color32::from_rgba_unmultiplied(236, 240, 255, 220);

                let title_text = if self.editing_workspace_name {
                    self.workspace_name_edit_buffer.clone()
                } else {
                    self.workspace_name().to_owned()
                };
                let title_galley =
                    ui.painter()
                        .layout_no_wrap(title_text.clone(), title_font.clone(), title_color);
                let title_width = (title_galley.size().x + 18.0).clamp(120.0, 300.0);
                let title_center_x = bar_rect.center().x;
                let title_left = if drag_right - drag_left <= title_width {
                    drag_left
                } else {
                    (title_center_x - title_width * 0.5).clamp(drag_left, drag_right - title_width)
                };
                let title_rect = Rect::from_min_max(
                    Pos2::new(title_left, bar_rect.top() + 4.0),
                    Pos2::new(title_left + title_width, bar_rect.bottom() - 4.0),
                );

                let (left_drag_rect, right_drag_rect) =
                    Self::split_drag_regions(bar_rect, drag_left, drag_right, title_rect);
                let start_drag =
                    ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));

                let left_drag_response = left_drag_rect.map(|rect| {
                    ui.interact(
                        rect,
                        ui.id().with("window_drag_area_left"),
                        egui::Sense::click_and_drag(),
                    )
                });
                let right_drag_response = right_drag_rect.map(|rect| {
                    ui.interact(
                        rect,
                        ui.id().with("window_drag_area_right"),
                        egui::Sense::click_and_drag(),
                    )
                });

                let drag_hovered = left_drag_response.as_ref().is_some_and(|r| r.hovered())
                    || right_drag_response.as_ref().is_some_and(|r| r.hovered());
                let drag_active = left_drag_response.as_ref().is_some_and(|r| r.dragged())
                    || right_drag_response.as_ref().is_some_and(|r| r.dragged());
                let drag_double_clicked = left_drag_response
                    .as_ref()
                    .is_some_and(|r| r.double_clicked())
                    || right_drag_response
                        .as_ref()
                        .is_some_and(|r| r.double_clicked());

                if drag_hovered {
                    ui.output_mut(|o| {
                        o.cursor_icon = if drag_active {
                            egui::CursorIcon::Grabbing
                        } else {
                            egui::CursorIcon::Grab
                        };
                    });
                }

                if drag_double_clicked {
                    let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                } else if drag_hovered && start_drag {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                let primary_clicked =
                    ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary));
                let pointer_pos = ui.input(|i| i.pointer.interact_pos().or(i.pointer.latest_pos()));

                if self.editing_workspace_name {
                    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(title_rect), |ui| {
                        let editor_id = ui.id().with("window_workspace_name_editor");
                        if self.pending_workspace_name_focus {
                            ui.memory_mut(|m| m.request_focus(editor_id));
                            self.pending_workspace_name_focus = false;
                        }

                        ui.add_sized(
                            title_rect.size(),
                            egui::TextEdit::singleline(&mut self.workspace_name_edit_buffer)
                                .id(editor_id)
                                .font(title_font.clone())
                                .horizontal_align(egui::Align::Center)
                                .margin(vec2(6.0, 2.0)),
                        );

                        let submit = ui.input(|i| i.key_pressed(egui::Key::Enter));
                        let cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));

                        if cancel {
                            self.cancel_workspace_name_edit();
                        } else if submit {
                            self.commit_workspace_name_edit();
                        } else if primary_clicked {
                            if let Some(pointer) = pointer_pos {
                                if !title_rect.contains(pointer) {
                                    self.commit_workspace_name_edit();
                                }
                            }
                        }
                    });
                } else {
                    let title_response = ui.interact(
                        title_rect,
                        ui.id().with("window_workspace_name_label"),
                        egui::Sense::click(),
                    );
                    if title_response.double_clicked() {
                        self.start_workspace_name_edit();
                    }

                    let title_hovered = title_response.hovered();
                    let bg_color = if title_hovered {
                        egui::Color32::from_rgba_unmultiplied(66, 76, 104, 180)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(52, 60, 86, 120)
                    };
                    ui.painter().rect_filled(title_rect, 8.0, bg_color);
                    ui.painter().galley(
                        title_rect.center() - title_galley.size() * 0.5,
                        title_galley,
                        title_color,
                    );
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

                let maxim = ui.interact(
                    maxim_rect,
                    ui.id().with("window_maximize_button"),
                    egui::Sense::click(),
                );
                if is_maximized {
                    let back = Rect::from_center_size(
                        maxim_rect.center() + vec2(-1.5, -1.5),
                        vec2(8.5, 7.5),
                    );
                    let front = Rect::from_center_size(
                        maxim_rect.center() + vec2(1.5, 1.5),
                        vec2(8.5, 7.5),
                    );
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
                self.last_command_palette_rect = Some(ui.min_rect().expand(8.0));
                ui.set_min_width(460.0);

                let palette_items = [
                    ("文件/新建", "文件/新建  Ctrl+N", 4usize),
                    ("文件/快速保存", "文件/快速保存  Ctrl+S", 0usize),
                    ("文件/快速加载", "文件/快速加载  Ctrl+R", 1usize),
                    ("文件/另存为", "文件/另存为  Ctrl+Shift+S", 2usize),
                    ("文件/加载", "文件/加载  Ctrl+O", 3usize),
                    ("编辑/撤销", "编辑/撤销  Ctrl+Z", 100usize),
                    ("编辑/重做", "编辑/重做  Ctrl+U", 101usize),
                    ("视图/性能浮窗", "视图/性能浮窗  Ctrl+Shift+M", 200usize),
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
                    } else if action_id < 200 {
                        self.run_edit_menu_action(action_id - 100);
                    } else {
                        self.performance_metrics.toggle_visible();
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

    pub(in crate::app) fn show_performance_overlay(&mut self, ctx: &egui::Context) {
        if !self.performance_metrics.is_visible() {
            return;
        }

        egui::Window::new("性能监控")
            .id(egui::Id::new("performance_metrics_overlay"))
            .default_pos(Pos2::new(16.0, 44.0))
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                let fps = self.performance_metrics.fps();
                let fps_text = if fps.is_finite() && fps > 0.0 {
                    format!("{fps:.1}")
                } else {
                    "--".to_owned()
                };

                let cpu_text = self
                    .performance_metrics
                    .cpu_usage()
                    .map(|value| format!("{value:.1}%"))
                    .unwrap_or_else(|| "--".to_owned());

                ui.label(format!("FPS: {fps_text}"));
                ui.label(format!("CPU: {cpu_text}"));
            });
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

        if self.performance_metrics.is_visible() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }

        if let Some(secs) = self.script_lua_next_repaint_after {
            if secs > 0.0 {
                ctx.request_repaint_after(Duration::from_secs_f64(secs.min(0.1)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_bar_rename_boundary_keeps_title_pixel_outside_drag() {
        let bar = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(600.0, 28.0));
        let title = Rect::from_min_max(Pos2::new(220.0, 4.0), Pos2::new(380.0, 24.0));
        let (left, right) = GraphApp::split_drag_regions(bar, 8.0, 500.0, title);

        let left = left.expect("left drag region");
        let right = right.expect("right drag region");

        assert!(left.max.x <= title.min.x - 4.0);
        assert!(right.min.x >= title.max.x + 4.0);
    }

    #[test]
    fn window_bar_rename_boundary_collapses_drag_when_title_fills_space() {
        let bar = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(200.0, 28.0));
        let title = Rect::from_min_max(Pos2::new(8.0, 4.0), Pos2::new(192.0, 24.0));
        let (left, right) = GraphApp::split_drag_regions(bar, 8.0, 192.0, title);

        assert!(left.is_none());
        assert!(right.is_none());
    }

    #[test]
    fn window_bar_rename_hit_route_text_area_avoids_drag_regions() {
        let bar = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(600.0, 28.0));
        let title = Rect::from_min_max(Pos2::new(220.0, 4.0), Pos2::new(380.0, 24.0));
        let (left, right) = GraphApp::split_drag_regions(bar, 8.0, 500.0, title);

        let title_center = title.center();
        assert!(title.contains(title_center));
        assert!(left.map_or(true, |rect| !rect.contains(title_center)));
        assert!(right.map_or(true, |rect| !rect.contains(title_center)));
    }

    #[test]
    fn window_bar_rename_hit_route_blank_area_stays_in_drag_regions() {
        let bar = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(600.0, 28.0));
        let title = Rect::from_min_max(Pos2::new(220.0, 4.0), Pos2::new(380.0, 24.0));
        let (left, right) = GraphApp::split_drag_regions(bar, 8.0, 500.0, title);

        let blank_left = Pos2::new(80.0, 14.0);
        let blank_right = Pos2::new(460.0, 14.0);
        assert!(left.is_some_and(|rect| rect.contains(blank_left)));
        assert!(right.is_some_and(|rect| rect.contains(blank_right)));
        assert!(!title.contains(blank_left));
        assert!(!title.contains(blank_right));
    }
}

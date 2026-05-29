use super::super::GraphApp;
use crate::model::NodeData;
use crate::msdf::debug_paint::paint_msdf_label;
use eframe::egui::{
    self, vec2, Align, Align2, Color32, FontId, Layout, Painter, Pos2, Rect, Stroke,
};

impl GraphApp {
    pub(in crate::app::ui) fn draw_script_node_body(
        &mut self,
        painter: &Painter,
        node_rect: Rect,
        zoom_scale: f32,
        stroke: Stroke,
        fill: Color32,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        node: &crate::model::Node,
        is_selected: bool,
    ) -> (Option<(usize, Rect)>, Option<(usize, Rect)>) {
        let mut title_edit_rect: Option<(usize, Rect)> = None;
        let mut script_edit_rect: Option<(usize, Rect)> = None;

        painter.rect_filled(node_rect, 8.0 * zoom_scale, Color32::from_rgb(38, 30, 52));
        painter.rect_stroke(node_rect, 8.0 * zoom_scale, stroke, egui::StrokeKind::Outside);

        let header_height = crate::constants::SCRIPT_HEADER_HEIGHT * zoom_scale;
        let header_bottom = (node_rect.min.y + header_height).min(node_rect.max.y);
        let header_rect = Rect::from_min_max(node_rect.min, Pos2::new(node_rect.max.x, header_bottom));
        let header_rounding = egui::CornerRadius {
            nw: (8.0 * zoom_scale).round() as u8,
            ne: (8.0 * zoom_scale).round() as u8,
            sw: 0,
            se: 0,
        };
        painter.rect_filled(header_rect, header_rounding, fill);

        let is_title_editing = self.ws.editing_title_node == Some(node.id);
        if !is_title_editing {
            let title = match &node.data {
                NodeData::Script { title, .. } => title.as_str(),
                _ => "Script",
            };
            let font_px = (16.0 * zoom_scale).max(0.5);
            let bl_x = node_rect.min.x + 12.0 * zoom_scale;
            let bl_y = header_rect.center().y + font_px * 0.38;
            paint_msdf_label(
                painter,
                node_rect,
                egui::Pos2::new(bl_x, bl_y),
                title,
                font_px,
                Color32::from_rgb(220, 200, 240),
                0x1000_0000_0000_0000 | node.id as u64,
            );

            let status = if let Some(err) = self.ws.script_lua_errors.get(&node.id) {
                let low = err.to_lowercase();
                if low.contains("hookerror") || low.contains("instruction") || low.contains("timeout") {
                    ("Frozen", Color32::from_rgb(230, 185, 95))
                } else {
                    ("Error", Color32::from_rgb(220, 90, 100))
                }
            } else if self.ws.script_lua_runtimes.contains_key(&node.id) {
                if self.ws.script_lua_runtimes.get(&node.id).map(|rt| rt.timer_interval() > 0.0).unwrap_or(false) {
                    ("Running", Color32::from_rgb(90, 210, 140))
                } else {
                    ("Idle", Color32::from_rgb(140, 160, 190))
                }
            } else {
                ("Idle", Color32::from_rgb(140, 160, 190))
            };

            let status_text = status.0;
            let status_color = status.1;
            let status_w = (status_text.len() as f32 * 7.2 + 16.0) * zoom_scale.clamp(0.8, 1.4);
            let status_h = 18.0 * zoom_scale.clamp(0.8, 1.4);
            let status_rect = Rect::from_min_size(
                Pos2::new(
                    node_rect.max.x - status_w - 10.0 * zoom_scale,
                    header_rect.center().y - status_h * 0.5,
                ),
                vec2(status_w, status_h),
            );
            painter.rect_filled(status_rect, 4.0 * zoom_scale, status_color);
            painter.text(
                status_rect.center(),
                Align2::CENTER_CENTER,
                status_text,
                FontId::proportional((11.0 * zoom_scale).max(9.0)),
                Color32::from_rgb(24, 26, 30),
            );
        } else {
            let rect_min = node_rect.left_top() + vec2(10.0, 6.0) * zoom_scale;
            let rect_max = Pos2::new(
                node_rect.max.x - 10.0 * zoom_scale,
                (node_rect.min.y + header_height - 6.0 * zoom_scale).min(node_rect.max.y),
            );
            title_edit_rect = Some((node.id, Rect::from_min_max(rect_min, rect_max)));
        }

        painter.line_segment(
            [
                Pos2::new(node_rect.min.x, header_bottom),
                Pos2::new(node_rect.max.x, header_bottom),
            ],
            Stroke::new(1.0 * zoom_scale.clamp(0.6, 1.6), Color32::from_rgb(130, 108, 170)),
        );

        let content_rect = Rect::from_min_max(Pos2::new(node_rect.min.x, header_bottom), node_rect.max);

        if content_rect.is_positive() {
            let is_editing = self.ws.editing_script_node == Some(node.id);

            if is_editing && self.ws.script_debug_node != Some(node.id) {
                let handle_clearance = 22.0 * zoom_scale.clamp(0.75, 1.6);
                let editor_rect = Rect::from_min_max(
                    content_rect.min,
                    Pos2::new(
                        (content_rect.max.x - handle_clearance).max(content_rect.min.x),
                        (content_rect.max.y - handle_clearance).max(content_rect.min.y),
                    ),
                );
                script_edit_rect = Some((node.id, editor_rect));
            } else {
                let zoom = zoom_scale;
                let is_debug = self.ws.script_debug_node == Some(node.id);

                let toolbar_h = if is_debug { (54.0 * zoom).max(40.0) } else { 0.0 };
                let toolbar_rect = Rect::from_min_size(content_rect.left_top(), vec2(content_rect.width(), toolbar_h));
                let mut deferred_review = false;
                let body_rect = if is_debug {
                    Rect::from_min_max(
                        Pos2::new(content_rect.min.x, content_rect.min.y + toolbar_h),
                        content_rect.max,
                    )
                } else {
                    content_rect
                };

                if is_debug && toolbar_rect.is_positive() {
                    ui.painter().rect_filled(toolbar_rect, 0.0, Color32::from_rgba_premultiplied(24, 28, 46, 200));
                    ui.painter().line_segment(
                        [
                            Pos2::new(toolbar_rect.left(), toolbar_rect.bottom()),
                            Pos2::new(toolbar_rect.right(), toolbar_rect.bottom()),
                        ],
                        Stroke::new(1.0, Color32::from_rgba_premultiplied(130, 108, 170, 60)),
                    );
                    if let Some(line) = self.ws.script_lua_pause_line.get(&node.id).copied() {
                        if line > 0 {
                            let pause_label = format!("暂停: 第 {line} 行");
                            ui.painter().text(
                                Pos2::new(toolbar_rect.left() + 8.0 * zoom, toolbar_rect.top() + 30.0 * zoom),
                                Align2::LEFT_CENTER,
                                pause_label,
                                FontId::proportional((12.0 * zoom).max(9.0)),
                                Color32::from_rgb(240, 210, 140),
                            );
                        }
                    }

                    let btn_h = (22.0 * zoom).max(18.0);
                    let btn_y = toolbar_rect.top() + 3.0 * zoom;
                    let step_w = (76.0 * zoom).max(60.0);
                    let step_rect = Rect::from_min_size(
                        Pos2::new(toolbar_rect.right() - step_w - 6.0 * zoom, btn_y),
                        vec2(step_w, btn_h),
                    );
                    let step_btn = egui::Button::new(
                        egui::RichText::new("Step").color(Color32::from_rgb(22, 24, 30)).size((11.0 * zoom).max(9.0)),
                    )
                    .fill(Color32::from_rgb(228, 234, 250))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(130, 146, 185)))
                    .corner_radius(4.0)
                    .min_size(vec2(step_w, btn_h));
                    if ui.put(step_rect, step_btn).clicked() {
                        if self.ensure_script_lua_runtime(node.id).is_ok() {
                            if let Some(rt) = self.ws.script_lua_runtimes.get_mut(&node.id) {
                                let _ = rt.request_step_into();
                            }
                            self.mark_workspace_dirty();
                            ctx.request_repaint();
                        }
                    }

                    let bp_w = (72.0 * zoom).max(56.0);
                    let bp_rect = Rect::from_min_size(
                        Pos2::new(step_rect.left() - bp_w - 4.0 * zoom, btn_y),
                        vec2(bp_w, btn_h),
                    );
                    let bp_input = self.ws.script_lua_breakpoint_input.entry(node.id).or_insert_with(String::new);
                    let bp_resp = ui.put(
                        bp_rect,
                        egui::TextEdit::singleline(bp_input)
                            .hint_text("bp行号")
                            .text_color(Color32::from_rgb(215, 220, 230))
                            .background_color(Color32::from_rgb(30, 34, 48)),
                    );
                    if bp_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if let Ok(line) = bp_input.trim().parse::<i32>() {
                            let set = self.ws.script_lua_breakpoints.entry(node.id).or_default();
                            let enable = !set.contains(&line);
                            if enable { set.insert(line); } else { set.remove(&line); }
                            if let Some(rt) = self.ws.script_lua_runtimes.get_mut(&node.id) {
                                let _ = rt.set_breakpoint(line, enable);
                            }
                            bp_input.clear();
                        }
                    }

                    let clear_w = (56.0 * zoom).max(48.0);
                    let clear_rect = Rect::from_min_size(
                        Pos2::new(bp_rect.left() - clear_w - 4.0 * zoom, btn_y),
                        vec2(clear_w, btn_h),
                    );
                    let clear_btn = egui::Button::new(
                        egui::RichText::new("清空BP").color(Color32::from_rgb(22, 24, 30)).size((10.0 * zoom).max(9.0)),
                    )
                    .fill(Color32::from_rgb(238, 220, 220))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(190, 130, 130)))
                    .corner_radius(4.0)
                    .min_size(vec2(clear_w, btn_h));
                    if ui.put(clear_rect, clear_btn).clicked() {
                        if let Some(existing) = self.ws.script_lua_breakpoints.get(&node.id).cloned() {
                            if let Some(rt) = self.ws.script_lua_runtimes.get_mut(&node.id) {
                                for line in existing {
                                    let _ = rt.set_breakpoint(line, false);
                                }
                            }
                        }
                        self.ws.script_lua_breakpoints.remove(&node.id);
                    }

                    let bp_summary = self.ws.script_lua_breakpoints.get(&node.id)
                        .map(|set| {
                            let mut v: Vec<i32> = set.iter().copied().collect();
                            v.sort_unstable();
                            v.into_iter().map(|n| n.to_string()).collect::<Vec<_>>().join(",")
                        })
                        .unwrap_or_default();
                    let bp_text = if bp_summary.is_empty() { "BP: -".to_owned() } else { format!("BP: {bp_summary}") };
                    ui.painter().text(
                        Pos2::new(toolbar_rect.left() + 150.0 * zoom, toolbar_rect.top() + 30.0 * zoom),
                        Align2::LEFT_CENTER,
                        bp_text,
                        FontId::proportional((11.0 * zoom).max(9.0)),
                        Color32::from_rgb(190, 205, 235),
                    );
                }

                let widget_rect = if is_debug {
                    let gap = 6.0 * zoom;
                    let editor_w = (body_rect.width() * 0.56).max(180.0).min(body_rect.width() - 120.0);
                    let editor_rect = Rect::from_min_max(
                        body_rect.left_top(),
                        Pos2::new(body_rect.left() + editor_w, body_rect.bottom()),
                    );
                    if editor_rect.is_positive() {
                        script_edit_rect = Some((node.id, editor_rect.shrink2(vec2(0.0, gap))));
                        ui.painter().line_segment(
                            [
                                Pos2::new(editor_rect.right() + gap * 0.5, editor_rect.top()),
                                Pos2::new(editor_rect.right() + gap * 0.5, editor_rect.bottom()),
                            ],
                            Stroke::new(1.0, Color32::from_rgba_premultiplied(130, 108, 170, 70)),
                        );
                    }
                    Rect::from_min_max(
                        Pos2::new(editor_rect.right() + gap, body_rect.top()),
                        body_rect.right_bottom(),
                    )
                } else {
                    content_rect
                };

                if self.ensure_script_lua_runtime(node.id).is_ok() {
                    if let Some(rt) = self.ws.script_lua_runtimes.get_mut(&node.id) {
                        let events = match rt.capture_render() {
                            Ok(v) => v,
                            Err(err) => {
                                let lower = err.to_lowercase();
                                if lower.contains("debug breakpoint hit") || lower.contains("调试中断") {
                                    if let Some(line) = rt.take_debug_pause_line() {
                                        self.ws.script_lua_pause_line.insert(node.id, line);
                                    }
                                    if let Ok(vars) = rt.debug_variables_snapshot() {
                                        self.ws.script_lua_debug_vars.insert(node.id, vars.to_string());
                                    }
                                    self.ws.script_lua_errors.remove(&node.id);
                                    Vec::new()
                                } else {
                                    let tagged = if lower.contains("hook") || lower.contains("instruction") || lower.contains("timeout") {
                                        format!("[HookError] {err}")
                                    } else {
                                        format!("[RuntimeError] {err}")
                                    };
                                    let err_node_id = node.id;
                                    eprintln!("[script-node:{err_node_id}] capture_render failed: {tagged}");
                                    self.ws.script_lua_errors.insert(node.id, tagged);
                                    Vec::new()
                                }
                            }
                        };

                        let mut style_bg = None;
                        let mut style_header_bg = None;
                        let mut style_bg_image = None;
                        for event in &events {
                            if let crate::script_node::lua::api_ctx::UiEvent::Style { bg, header_bg, bg_image } = event {
                                if let Some(bg) = bg.as_deref().and_then(Self::script_color_from_lua) {
                                    style_bg = Some(bg);
                                }
                                if let Some(header_bg) = header_bg.as_deref().and_then(Self::script_color_from_lua) {
                                    style_header_bg = Some(header_bg);
                                }
                                if let Some(bg_image) = bg_image {
                                    style_bg_image = Some(bg_image.clone());
                                }
                            }
                        }

                        if let Some(bg) = style_bg {
                            painter.rect_filled(node_rect, 8.0 * zoom_scale, bg);
                        }
                        if let Some(bg_image) = style_bg_image.as_deref() {
                            if let Some(texture_id) = self.ensure_script_bg_texture(node.id, bg_image, ctx) {
                                painter.image(
                                    texture_id,
                                    node_rect,
                                    Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                                    Color32::WHITE,
                                );
                            }
                        }
                        if let Some(header_bg) = style_header_bg {
                            painter.rect_filled(header_rect, header_rounding, header_bg);
                        }
                        painter.rect_stroke(node_rect, 8.0 * zoom_scale, stroke, egui::StrokeKind::Outside);

                        let mut body_ui = ui.new_child(
                            egui::UiBuilder::new().max_rect(widget_rect).layout(Layout::top_down(Align::Min)),
                        );
                        body_ui.set_clip_rect(widget_rect);

                        let queue_resp = body_ui.add(
                            egui::Button::new(
                                egui::RichText::new("📋 队列")
                                    .color(Color32::from_rgb(22, 24, 30))
                                    .size((11.0 * zoom).max(9.0)),
                            )
                            .fill(Color32::from_rgb(228, 234, 250))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(130, 146, 185)))
                            .corner_radius(4.0),
                        );
                        if queue_resp.clicked() {
                            deferred_review = true;
                        }

                        for event in events {
                            use crate::script_node::lua::api_ctx::UiEvent;
                            match event {
                                UiEvent::Style { .. } => {}
                                UiEvent::Text { text, .. } => { body_ui.label(text); }
                                UiEvent::ButtonWithCallback { label, event_key, enabled, .. } => {
                                    let resp = body_ui.add_enabled(enabled, egui::Button::new(label.clone()));
                                    if resp.clicked() && enabled {
                                        let key = event_key.as_deref().unwrap_or(&label).to_owned();
                                        if let Some(rt) = self.ws.script_lua_runtimes.get_mut(&node.id) {
                                            rt.queue_button_click(&key);
                                        }
                                        self.mark_workspace_dirty();
                                        ctx.request_repaint();
                                    }
                                }
                                UiEvent::Button { label, enabled, .. } => {
                                    let resp = body_ui.add_enabled(enabled, egui::Button::new(label.clone()));
                                    if resp.clicked() && enabled {
                                        if let Some(rt) = self.ws.script_lua_runtimes.get_mut(&node.id) {
                                            rt.queue_button_click(&label);
                                        }
                                        self.mark_workspace_dirty();
                                        ctx.request_repaint();
                                    }
                                }
                                UiEvent::Input { label, mut value, enabled, multiline, rows, .. } => {
                                    body_ui.horizontal(|ui| {
                                        if !label.is_empty() { ui.label(label.clone()); }
                                        let resp = if multiline {
                                            let row_count = rows.max(1) as usize;
                                            ui.add_enabled(enabled, egui::TextEdit::multiline(&mut value).desired_rows(row_count))
                                        } else {
                                            ui.add_enabled(enabled, egui::TextEdit::singleline(&mut value))
                                        };
                                        if resp.changed() {
                                            let key = if label.is_empty() { "input".to_owned() } else { label.clone() };
                                            if let Some(rt) = self.ws.script_lua_runtimes.get_mut(&node.id) {
                                                rt.queue_input_value(&key, &value);
                                            }
                                            self.mark_workspace_dirty();
                                            ctx.request_repaint();
                                        }
                                    });
                                }
                                UiEvent::Slider { label, mut value, min, max, enabled } => {
                                    body_ui.horizontal(|ui| {
                                        if !label.is_empty() { ui.label(label.clone()); }
                                        let resp = ui.add_enabled(enabled, egui::Slider::new(&mut value, min..=max));
                                        if resp.changed() {
                                            let val_str = format!("{value}");
                                            let port_name = if label.is_empty() { "slider".to_owned() } else { label.clone() };
                                            self.ws.script_node_outputs.entry(node.id).or_default().insert(port_name.clone(), val_str.clone());
                                            let targets: Vec<(usize, Option<String>)> = self.ws.edges.iter()
                                                .filter(|(from, _)| *from == node.id)
                                                .filter(|(from, to)| {
                                                    match self.edge_route_key(*from, *to) {
                                                        Some(k) => k == port_name.as_str(),
                                                        None => true,
                                                    }
                                                })
                                                .map(|(_, to)| {
                                                    let route = self.edge_route_key(node.id, *to).map(|s| s.to_owned());
                                                    (*to, route)
                                                })
                                                .collect();
                                            for (to_node, route_key) in &targets {
                                                self.forward_message_to_node(*to_node, route_key.as_deref(), &val_str);
                                            }
                                        }
                                    });
                                }
                                UiEvent::Separator { .. } => { body_ui.separator(); }
                                UiEvent::Spacer(h) => { body_ui.add_space(h); }
                                UiEvent::Badge { text, .. } => { body_ui.label(text); }
                                UiEvent::Card { text, caption } => {
                                    body_ui.group(|ui| {
                                        ui.label(text);
                                        if let Some(c) = caption { ui.small(c); }
                                    });
                                }
                                UiEvent::ProgressBar { value, .. } => {
                                    body_ui.add(egui::ProgressBar::new(value as f32));
                                }
                                _ => {}
                            }
                        }
                    }
                }

                if let Some(vars) = self.ws.script_lua_debug_vars.get(&node.id).cloned() {
                    let dbg_rect = Rect::from_min_max(
                        Pos2::new(widget_rect.left() + 6.0 * zoom, widget_rect.bottom() - 108.0 * zoom),
                        Pos2::new(widget_rect.right() - 6.0 * zoom, widget_rect.bottom() - 6.0 * zoom),
                    );
                    if dbg_rect.is_positive() {
                        ui.painter().rect_filled(dbg_rect, 4.0, Color32::from_rgba_premultiplied(36, 44, 68, 220));
                        let copy_w = 52.0 * zoom;
                        let copy_h = 20.0 * zoom;
                        let copy_rect = Rect::from_min_size(
                            Pos2::new(dbg_rect.right() - copy_w - 6.0 * zoom, dbg_rect.top() + 4.0 * zoom),
                            vec2(copy_w, copy_h),
                        );
                        let copy_btn = egui::Button::new("复制变量")
                            .fill(Color32::from_rgba_premultiplied(78, 96, 136, 220))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(150, 175, 225)))
                            .min_size(copy_rect.size());
                        if ui.put(copy_rect, copy_btn).clicked() {
                            ctx.copy_text(vars.clone());
                            self.push_toast_notification("调试变量已复制".to_owned());
                        }
                        let text_rect = Rect::from_min_max(
                            Pos2::new(dbg_rect.left() + 6.0 * zoom, dbg_rect.top() + copy_h + 10.0 * zoom),
                            Pos2::new(dbg_rect.right() - 6.0 * zoom, dbg_rect.bottom() - 6.0 * zoom),
                        );
                        let mut dbg_ui = ui.new_child(
                            egui::UiBuilder::new().max_rect(text_rect).layout(Layout::top_down(Align::Min)),
                        );
                        dbg_ui.set_clip_rect(text_rect);
                        egui::ScrollArea::vertical()
                            .id_salt(("script-debug-scroll", node.id))
                            .show(&mut dbg_ui, |ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(format!("Debug Vars\n{vars}"))
                                            .monospace()
                                            .color(Color32::from_rgb(210, 225, 255))
                                            .size((10.0 * zoom).max(9.0)),
                                    )
                                    .wrap(),
                                );
                            });
                    }
                }

                if let Some(err) = self.ws.script_lua_errors.get(&node.id).cloned() {
                    let err_rect = Rect::from_min_max(
                        Pos2::new(widget_rect.left() + 6.0 * zoom, widget_rect.bottom() - 86.0 * zoom),
                        Pos2::new(widget_rect.right() - 6.0 * zoom, widget_rect.bottom() - 6.0 * zoom),
                    );
                    if err_rect.is_positive() {
                        ui.painter().rect_filled(err_rect, 4.0, Color32::from_rgba_premultiplied(90, 20, 28, 220));
                        let (title, detail) = if let Some(rest) = err.strip_prefix("[SyntaxError] ") {
                            ("Lua SyntaxError", rest)
                        } else if let Some(rest) = err.strip_prefix("[HookError] ") {
                            ("Lua HookError", rest)
                        } else if let Some(rest) = err.strip_prefix("[RuntimeError] ") {
                            ("Lua RuntimeError", rest)
                        } else {
                            ("Lua Error", err.as_str())
                        };

                        let actions_h = 22.0 * zoom;
                        let clear_w = 46.0 * zoom;
                        let copy_w = 46.0 * zoom;
                        let gap = 6.0 * zoom;
                        let clear_rect = Rect::from_min_size(
                            Pos2::new(err_rect.right() - clear_w - 6.0 * zoom, err_rect.top() + 4.0 * zoom),
                            vec2(clear_w, actions_h),
                        );
                        let copy_rect = Rect::from_min_size(
                            Pos2::new(clear_rect.left() - copy_w - gap, err_rect.top() + 4.0 * zoom),
                            vec2(copy_w, actions_h),
                        );
                        let clear_btn = egui::Button::new("清除")
                            .fill(Color32::from_rgba_premultiplied(120, 40, 40, 220))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(220, 140, 140)))
                            .min_size(clear_rect.size());
                        if ui.put(clear_rect, clear_btn).clicked() {
                            self.ws.script_lua_errors.remove(&node.id);
                        }
                        let copy_btn = egui::Button::new("复制")
                            .fill(Color32::from_rgba_premultiplied(70, 40, 90, 220))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(170, 140, 220)))
                            .min_size(copy_rect.size());
                        if ui.put(copy_rect, copy_btn).clicked() {
                            ctx.copy_text(format!("{title}\n{detail}"));
                            self.push_toast_notification("Lua 错误已复制到剪贴板");
                        }
                        let text_rect = Rect::from_min_max(
                            Pos2::new(err_rect.left() + 6.0 * zoom, err_rect.top() + actions_h + 8.0 * zoom),
                            Pos2::new(err_rect.right() - 6.0 * zoom, err_rect.bottom() - 6.0 * zoom),
                        );
                        let mut err_ui = ui.new_child(
                            egui::UiBuilder::new().max_rect(text_rect).layout(Layout::top_down(Align::Min)),
                        );
                        err_ui.set_clip_rect(text_rect);
                        egui::ScrollArea::vertical()
                            .id_salt(("script-error-scroll", node.id))
                            .show(&mut err_ui, |ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(format!("{title}\n{detail}"))
                                            .monospace()
                                            .color(Color32::from_rgb(255, 210, 210))
                                            .size((11.0 * zoom).max(9.0)),
                                    )
                                    .wrap(),
                                );
                            });
                    }
                }

                if deferred_review {
                    self.start_script_queue_edit(node.id);
                }
            }
        }

        if is_selected {
            Self::draw_resize_handle(
                painter,
                node_rect,
                zoom_scale,
                Color32::from_rgb(200, 140, 255),
            );
        }

        (title_edit_rect, script_edit_rect)
    }
}

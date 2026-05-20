use super::super::GraphApp;
use crate::constants::{DECISION_HEADER_HEIGHT, GROUP_HEADER_HEIGHT};
use crate::model::{NodeData, NodeKind};
use eframe::egui::{
    self, text::{LayoutJob, TextFormat}, vec2, Align, Align2, Color32, FontId, Layout, Painter, Pos2, Rect, Stroke,
};
use egui_commonmark::CommonMarkViewer;

/// Rainbow palette for matching bracket pairs (6 colors cycling).
const RAINBOW: [Color32; 6] = [
    Color32::from_rgb(255, 200, 70),   // gold
    Color32::from_rgb(255, 140, 100),  // coral
    Color32::from_rgb(100, 210, 120),  // green
    Color32::from_rgb(80, 210, 210),   // teal
    Color32::from_rgb(100, 170, 255),  // blue
    Color32::from_rgb(200, 140, 255),  // purple
];

/// Build a syntax-highlighted LayoutJob for Lua text.
pub(super) fn highlight_lua(text: &str, font: FontId) -> LayoutJob {
    let mut job = LayoutJob::default();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let len = chars.len();
    let mut idx = 0;
    let mut bracket_depth: i32 = 0;

    while idx < len {
        let (byte_start, c) = chars[idx];

        if c.is_whitespace() {
            while idx < len && chars[idx].1.is_whitespace() { idx += 1; }
            let byte_end = if idx < len { chars[idx].0 } else { text.len() };
            job.append(&text[byte_start..byte_end], 0.0, TextFormat::simple(font.clone(), Color32::from_rgb(120, 120, 140)));
            continue;
        }

        if c == '-' && idx + 1 < len && chars[idx + 1].1 == '-' {
            idx += 2;
            while idx < len && chars[idx].1 != '\n' { idx += 1; }
            let byte_end = if idx < len { chars[idx].0 } else { text.len() };
            job.append(&text[byte_start..byte_end], 0.0, TextFormat::simple(font.clone(), Color32::from_rgb(130, 150, 130)));
            continue;
        }

        if c == '\'' || c == '"' {
            let quote = c;
            idx += 1;
            while idx < len {
                if chars[idx].1 == '\\' { idx += 2; continue; }
                if chars[idx].1 == quote { idx += 1; break; }
                idx += 1;
            }
            let byte_end = if idx < len { chars[idx].0 } else { text.len() };
            job.append(&text[byte_start..byte_end], 0.0, TextFormat::simple(font.clone(), Color32::from_rgb(160, 220, 140)));
            continue;
        }

        if c.is_ascii_digit() || (c == '-' && idx + 1 < len && chars[idx + 1].1.is_ascii_digit()) {
            idx += 1;
            while idx < len {
                let ch = chars[idx].1;
                let ok = ch.is_ascii_digit() || ch == '.' || ch == 'e' || ch == 'E'
                    || ((ch == '+' || ch == '-') && matches!(chars[idx.saturating_sub(1)].1, 'e' | 'E'));
                if ok { idx += 1; } else { break; }
            }
            let byte_end = if idx < len { chars[idx].0 } else { text.len() };
            job.append(&text[byte_start..byte_end], 0.0, TextFormat::simple(font.clone(), Color32::from_rgb(255, 190, 80)));
            continue;
        }

        if c == '_' || c.is_ascii_alphabetic() {
            idx += 1;
            while idx < len {
                let ch = chars[idx].1;
                if ch == '_' || ch.is_ascii_alphanumeric() { idx += 1; } else { break; }
            }
            let byte_end = if idx < len { chars[idx].0 } else { text.len() };
            let token = &text[byte_start..byte_end];
            let color = if matches!(token,
                "and" | "break" | "do" | "else" | "elseif" | "end" | "false" | "for" | "function"
                | "goto" | "if" | "in" | "local" | "nil" | "not" | "or" | "repeat" | "return"
                | "then" | "true" | "until" | "while") {
                Color32::from_rgb(210, 150, 255)
            } else if matches!(token, "state" | "ctx" | "ports") {
                Color32::from_rgb(100, 200, 255)
            } else {
                Color32::from_rgb(210, 210, 220)
            };
            job.append(token, 0.0, TextFormat::simple(font.clone(), color));
            continue;
        }

        if matches!(c, '{' | '}' | '[' | ']' | '(' | ')' | ':' | ',' | '.') {
            let color = match c {
                '{' | '[' | '(' => {
                    let depth = bracket_depth.max(0) as usize % RAINBOW.len();
                    bracket_depth += 1;
                    RAINBOW[depth]
                }
                '}' | ']' | ')' => {
                    bracket_depth = (bracket_depth - 1).max(0);
                    let depth = bracket_depth as usize % RAINBOW.len();
                    RAINBOW[depth]
                }
                _ => Color32::from_rgb(130, 130, 160),
            };
            let byte_end = byte_start + c.len_utf8();
            job.append(&text[byte_start..byte_end], 0.0, TextFormat::simple(font.clone(), color));
            idx += 1;
            continue;
        }

        let byte_end = byte_start + c.len_utf8();
        job.append(&text[byte_start..byte_end], 0.0, TextFormat::simple(font.clone(), Color32::from_rgb(200, 200, 220)));
        idx += 1;
    }

    job
}

impl GraphApp {
    pub(in crate::app::ui) fn draw_nodes(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        painter: &Painter,
        rect: Rect,
    ) -> (
        Option<(usize, Rect)>,
        Option<(usize, Rect)>,
        Option<(usize, Rect)>,
        Option<(usize, Rect)>,
        Option<(usize, Rect)>,
        Option<(usize, Rect)>,
    ) {
        let mut text_edit_rect: Option<(usize, Rect)> = None;
        let mut title_edit_rect: Option<(usize, Rect)> = None;
        let mut startup_edit_rect: Option<(usize, Rect)> = None;
        let mut decision_edit_rect: Option<(usize, Rect)> = None;
        let mut working_directory_edit_rect: Option<(usize, Rect)> = None;
        let mut script_edit_rect: Option<(usize, Rect)> = None;
        let mut render_nodes = self.nodes.clone();
        render_nodes.sort_by_key(|node| usize::from(node.kind != NodeKind::Group));
        for node in &render_nodes {
            let node_rect =
                self.world_to_screen_rect(rect, Rect::from_min_size(node.pos, node.size));
            let is_selected = self.selected_nodes.contains(&node.id);
            let zoom_scale = self.zoom;

            let (fill, stroke) = match node.kind {
                NodeKind::Terminal => {
                    let fill = if is_selected {
                        Color32::from_rgb(64, 52, 120)
                    } else {
                        Color32::from_rgb(48, 40, 86)
                    };
                    let stroke = if is_selected {
                        Stroke::new(
                            2.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(174, 149, 255),
                        )
                    } else {
                        Stroke::new(
                            1.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(108, 96, 145),
                        )
                    };
                    (fill, stroke)
                }
                NodeKind::Text => {
                    let fill = if is_selected {
                        Color32::from_rgb(90, 73, 34)
                    } else {
                        Color32::from_rgb(72, 60, 31)
                    };
                    let stroke = if is_selected {
                        Stroke::new(
                            2.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(255, 220, 130),
                        )
                    } else {
                        Stroke::new(
                            1.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(130, 114, 68),
                        )
                    };
                    (fill, stroke)
                }

                NodeKind::Image => {
                    let fill = if is_selected {
                        Color32::from_rgb(32, 78, 88)
                    } else {
                        Color32::from_rgb(24, 61, 70)
                    };
                    let stroke = if is_selected {
                        Stroke::new(
                            2.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(124, 220, 240),
                        )
                    } else {
                        Stroke::new(
                            1.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(78, 145, 160),
                        )
                    };
                    (fill, stroke)
                }
                NodeKind::Decision => {
                    let fill = if is_selected {
                        Color32::from_rgb(44, 78, 56)
                    } else {
                        Color32::from_rgb(34, 62, 45)
                    };
                    let stroke = if is_selected {
                        Stroke::new(
                            2.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(150, 236, 180),
                        )
                    } else {
                        Stroke::new(
                            1.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(98, 162, 120),
                        )
                    };
                    (fill, stroke)
                }
                NodeKind::Script => {
                    let fill = if is_selected {
                        Color32::from_rgb(70, 50, 90)
                    } else {
                        Color32::from_rgb(50, 36, 65)
                    };
                    let stroke = if is_selected {
                        Stroke::new(
                            2.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(200, 140, 255),
                        )
                    } else {
                        Stroke::new(
                            1.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(130, 108, 170),
                        )
                    };
                    (fill, stroke)
                }
                NodeKind::Group => {
                    let fill = if is_selected {
                        Color32::from_rgba_unmultiplied(62, 70, 108, 58)
                    } else {
                        Color32::from_rgba_unmultiplied(52, 58, 88, 46)
                    };
                    let stroke = if is_selected {
                        Stroke::new(
                            2.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(186, 205, 255),
                        )
                    } else {
                        Stroke::new(
                            1.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(118, 132, 176),
                        )
                    };
                    (fill, stroke)
                }
            };

            if matches!(node.kind, NodeKind::Text) {
                painter.rect(
                    node_rect,
                    8.0 * zoom_scale,
                    fill,
                    stroke,
                    egui::StrokeKind::Outside,
                );
            }

            match node.kind {
                NodeKind::Terminal => {
                    painter.rect_stroke(
                        node_rect,
                        8.0 * zoom_scale,
                        stroke,
                        egui::StrokeKind::Outside,
                    );

                    let header_height = self.terminal_header_height_screen();
                    let header_bottom = (node_rect.min.y + header_height).min(node_rect.max.y);
                    let header_rect = Rect::from_min_max(
                        node_rect.min,
                        Pos2::new(node_rect.max.x, header_bottom),
                    );
                    let header_rounding = egui::CornerRadius {
                        nw: (8.0 * zoom_scale).round() as u8,
                        ne: (8.0 * zoom_scale).round() as u8,
                        sw: 0,
                        se: 0,
                    };
                    painter.rect_filled(header_rect, header_rounding, fill);

                    let is_title_editing = self.editing_title_node == Some(node.id);
                    if !is_title_editing {
                        let title_text = match &node.data {
                            NodeData::Terminal { title, .. } => title.as_str(),
                            _ => "Terminal",
                        };
                        painter.text(
                            Pos2::new(node_rect.min.x + 12.0 * zoom_scale, header_rect.center().y),
                            Align2::LEFT_CENTER,
                            title_text,
                            FontId::proportional((17.0 * zoom_scale).max(9.0)),
                            Color32::WHITE,
                        );
                    } else {
                        let rect_min = node_rect.left_top() + vec2(10.0, 6.0) * zoom_scale;
                        let rect_max = Pos2::new(
                            node_rect.max.x - 10.0 * zoom_scale,
                            (node_rect.min.y + header_height - 6.0 * zoom_scale)
                                .min(node_rect.max.y),
                        );
                        title_edit_rect = Some((node.id, Rect::from_min_max(rect_min, rect_max)));
                    }

                    if !is_title_editing {
                        let state_text = if self.terminal_backends.contains_key(&node.id) {
                            "状态: Running"
                        } else if self.terminal_exited.contains(&node.id) {
                            "状态: Exited"
                        } else {
                            "状态: Starting"
                        };

                        painter.text(
                            Pos2::new(node_rect.max.x - 12.0 * zoom_scale, header_rect.center().y),
                            Align2::RIGHT_CENTER,
                            state_text,
                            FontId::proportional((13.0 * zoom_scale).max(8.0)),
                            Color32::from_rgb(225, 220, 255),
                        );

                        if is_selected {
                            let cwd_value = match &node.data {
                                NodeData::Terminal {
                                    working_directory, ..
                                } => working_directory
                                    .as_deref()
                                    .map(str::trim)
                                    .filter(|value| !value.is_empty())
                                    .unwrap_or("(默认 cwd)"),
                                _ => "(默认 cwd)",
                            };
                            let cwd_text = format!("cwd: {cwd_value}");
                            let badge_font = FontId::proportional((11.0 * zoom_scale).max(8.0));
                            let text_color = Color32::from_rgb(231, 226, 255);
                            let galley = painter.layout_no_wrap(
                                cwd_text.clone(),
                                badge_font.clone(),
                                text_color,
                            );
                            let pad_x = 10.0 * zoom_scale;
                            let pad_y = 5.0 * zoom_scale;
                            let badge_size = galley.size() + vec2(pad_x * 2.0, pad_y * 2.0);
                            let badge_gap = 6.0 * zoom_scale;
                            let badge_rect = Rect::from_min_size(
                                Pos2::new(node_rect.min.x, node_rect.min.y - badge_size.y - badge_gap),
                                badge_size,
                            );

                            painter.rect_filled(
                                badge_rect,
                                7.0 * zoom_scale,
                                Color32::from_rgba_premultiplied(54, 46, 96, 238),
                            );
                            painter.rect_stroke(
                                badge_rect,
                                7.0 * zoom_scale,
                                Stroke::new(1.0 * zoom_scale.clamp(0.6, 1.6), Color32::from_rgb(130, 118, 185)),
                                egui::StrokeKind::Outside,
                            );
                            painter.text(
                                Pos2::new(badge_rect.min.x + pad_x, badge_rect.min.y + pad_y),
                                Align2::LEFT_TOP,
                                cwd_text,
                                badge_font,
                                text_color,
                            );
                        }
                    }

                    painter.line_segment(
                        [
                            Pos2::new(node_rect.min.x, header_bottom),
                            Pos2::new(node_rect.max.x, header_bottom),
                        ],
                        Stroke::new(
                            1.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(108, 96, 145),
                        ),
                    );

                    let hide_terminal_for_zoom = zoom_scale < self.terminal_hide_zoom_threshold
                        && self.editing_startup_node != Some(node.id)
                        && self.editing_working_directory_node != Some(node.id);

                    if let Some(term_rect) = self.terminal_content_rect_screen(node.id, rect) {
                        if !hide_terminal_for_zoom {
                            if self.editing_startup_node == Some(node.id)
                                || self.editing_working_directory_node == Some(node.id)
                            {
                                let overlay_rect = Rect::from_min_max(
                                    term_rect.min + vec2(10.0, 10.0) * zoom_scale,
                                    term_rect.max - vec2(10.0, 10.0) * zoom_scale,
                                );
                                if self.editing_startup_node == Some(node.id) {
                                    startup_edit_rect = Some((node.id, overlay_rect));
                                }
                                if self.editing_working_directory_node == Some(node.id) {
                                    working_directory_edit_rect = Some((node.id, overlay_rect));
                                }
                            }
                            self.draw_embedded_terminal_for_rect(ui, ctx, rect, node.id, term_rect);
                        }
                    }

                    if is_selected {
                        let handle_size = 12.0 * zoom_scale.clamp(0.75, 1.6);
                        let handle_rect = Rect::from_min_size(
                            node_rect.right_bottom() - vec2(handle_size + 6.0, handle_size + 6.0),
                            vec2(handle_size, handle_size),
                        );
                        painter.rect_filled(handle_rect, 2.0, Color32::from_rgb(205, 195, 255));
                    }
                }
                NodeKind::Text => {
                    let hide_text_for_zoom = zoom_scale < self.text_hide_zoom_threshold;
                    let is_editing = self.editing_text_node == Some(node.id);

                    if !hide_text_for_zoom {
                        if !is_editing {
                            let preview = match &node.data {
                                NodeData::Text { text_body, .. } if text_body.trim().is_empty() => {
                                    "(空文本)"
                                }
                                NodeData::Text { text_body, .. } => text_body.as_str(),
                                _ => "(空文本)",
                            };

                            let content_rect = Rect::from_min_max(
                                node_rect.min + vec2(12.0, 12.0) * zoom_scale,
                                node_rect.max - vec2(12.0, 12.0) * zoom_scale,
                            );

                            if content_rect.is_positive() {
                                let mut text_ui = ui.new_child(
                                    egui::UiBuilder::new()
                                        .max_rect(content_rect)
                                        .layout(Layout::top_down(Align::Min)),
                                );
                                text_ui.set_clip_rect(content_rect);
                                text_ui.set_width(content_rect.width());

                                egui::ScrollArea::vertical()
                                    .id_salt(("text-node-preview-scroll", node.id))
                                    .auto_shrink([false, false])
                                    .show(&mut text_ui, |ui| {
                                        ui.set_width(content_rect.width());

                                        let body_size = (15.0 * zoom_scale).round();
                                        let heading_size = (body_size * 2.2).round();

                                        // Markdown 主题（暖深色）
                                        let body_color = Color32::from_rgb(236, 228, 208); // 正文
                                        let heading_color = Color32::from_rgb(255, 214, 122); // 标题/强调
                                        let scrollbar_bg = Color32::from_rgb(0, 0, 0); // 滚动条轨道背景
                                        let scrollbar_fg = Color32::from_rgb(255, 255, 255); // 滚动条前景

                                        let style = ui.style_mut();
                                        style.visuals.override_text_color = None;
                                        style.visuals.widgets.noninteractive.fg_stroke.color =
                                            body_color;
                                        style.visuals.widgets.inactive.fg_stroke.color = body_color;
                                        style.visuals.widgets.active.fg_stroke.color =
                                            heading_color;
                                        style.visuals.hyperlink_color =
                                            Color32::from_rgb(122, 196, 255); // 链接
                                        style.visuals.code_bg_color = Color32::from_rgb(47, 42, 33); // 代码块背景

                                        style.visuals.extreme_bg_color = scrollbar_bg;
                                        style.visuals.faint_bg_color = scrollbar_bg;
                                        style.spacing.scroll.foreground_color = true;
                                        style.visuals.widgets.inactive.fg_stroke.color =
                                            scrollbar_fg;
                                        style.visuals.widgets.hovered.fg_stroke.color =
                                            scrollbar_fg;
                                        style.visuals.widgets.active.fg_stroke.color = scrollbar_fg;
                                        style.visuals.widgets.open.fg_stroke.color = scrollbar_fg;

                                        style.text_styles.insert(
                                            egui::TextStyle::Body,
                                            FontId::proportional(body_size),
                                        );
                                        style.text_styles.insert(
                                            egui::TextStyle::Small,
                                            FontId::proportional(body_size),
                                        );
                                        style.text_styles.insert(
                                            egui::TextStyle::Button,
                                            FontId::proportional(body_size),
                                        );
                                        style.text_styles.insert(
                                            egui::TextStyle::Heading,
                                            FontId::proportional(heading_size),
                                        );
                                        style.text_styles.insert(
                                            egui::TextStyle::Monospace,
                                            FontId::monospace(body_size),
                                        );
                                        style.interaction.selectable_labels = false;

                                        CommonMarkViewer::new().show(
                                            ui,
                                            &mut self.markdown_cache,
                                            preview,
                                        );
                                    });
                            }
                        }

                        if is_editing {
                            let edit_rect = Rect::from_min_max(
                                node_rect.min + vec2(12.0, 12.0) * zoom_scale,
                                node_rect.max - vec2(12.0, 12.0) * zoom_scale,
                            );
                            text_edit_rect = Some((node.id, edit_rect));
                        }
                    }

                    if is_selected {
                        let handle_size = 12.0 * zoom_scale.clamp(0.75, 1.6);
                        let handle_rect = Rect::from_min_size(
                            node_rect.right_bottom() - vec2(handle_size + 6.0, handle_size + 6.0),
                            vec2(handle_size, handle_size),
                        );
                        painter.rect_filled(handle_rect, 2.0, Color32::from_rgb(255, 220, 130));
                    }
                }
                NodeKind::Decision => {
                    painter.rect_stroke(
                        node_rect,
                        8.0 * zoom_scale,
                        stroke,
                        egui::StrokeKind::Outside,
                    );

                    let header_height = DECISION_HEADER_HEIGHT * zoom_scale;
                    let header_bottom = (node_rect.min.y + header_height).min(node_rect.max.y);
                    let header_rect = Rect::from_min_max(
                        node_rect.min,
                        Pos2::new(node_rect.max.x, header_bottom),
                    );
                    painter.rect_filled(header_rect, 8.0 * zoom_scale, fill);

                    let is_title_editing = self.editing_title_node == Some(node.id);
                    if !is_title_editing {
                        let title = match &node.data {
                            NodeData::Decision { title, .. } => title.as_str(),
                            _ => "Decision",
                        };
                        painter.text(
                            Pos2::new(node_rect.min.x + 12.0 * zoom_scale, header_rect.center().y),
                            Align2::LEFT_CENTER,
                            title,
                            FontId::proportional((16.0 * zoom_scale).max(10.0)),
                            Color32::from_rgb(230, 255, 238),
                        );
                    } else {
                        let rect_min = node_rect.left_top() + vec2(10.0, 6.0) * zoom_scale;
                        let rect_max = Pos2::new(
                            node_rect.max.x - 10.0 * zoom_scale,
                            (node_rect.min.y + header_height - 6.0 * zoom_scale)
                                .min(node_rect.max.y),
                        );
                        title_edit_rect = Some((node.id, Rect::from_min_max(rect_min, rect_max)));
                    }

                    painter.line_segment(
                        [
                            Pos2::new(node_rect.min.x, header_bottom),
                            Pos2::new(node_rect.max.x, header_bottom),
                        ],
                        Stroke::new(
                            1.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(98, 162, 120),
                        ),
                    );

                    let content_rect = Rect::from_min_max(
                        Pos2::new(
                            node_rect.min.x + 12.0 * zoom_scale,
                            header_bottom + 10.0 * zoom_scale,
                        ),
                        node_rect.max - vec2(12.0, 12.0) * zoom_scale,
                    );

                    let is_decision_editing = self.editing_decision_buttons_node == Some(node.id);

                    if is_decision_editing {
                        decision_edit_rect = Some((node.id, content_rect));
                    } else if content_rect.is_positive() {
                        let mut content_ui = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(content_rect)
                                .layout(Layout::top_down(Align::Min)),
                        );
                        content_ui.set_clip_rect(content_rect);
                        content_ui.set_width(content_rect.width());

                        if let NodeData::Decision { buttons, .. } = &node.data {
                            let (queue_len, queue_head_preview) =
                                self.decision_queue_preview(node.id);
                            let has_pending = queue_len > 0;

                            let pending_preview = if has_pending {
                                queue_head_preview.as_str()
                            } else {
                                "(暂无待处理消息)"
                            };

                            content_ui.scope(|ui| {
                                ui.style_mut().visuals.override_text_color =
                                    Some(Color32::from_rgb(216, 245, 224));
                                ui.label(format!("待处理消息（队列: {queue_len}）:"));
                                egui::ScrollArea::vertical()
                                    .id_salt(("decision-pending-scroll", node.id))
                                    .auto_shrink([false, true])
                                    .max_height((content_rect.height() * 0.40).max(52.0))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.label(pending_preview);
                                        if queue_len > 1 {
                                            ui.add_space(4.0 * zoom_scale);
                                            ui.label(format!("... 还有 {} 条", queue_len - 1));
                                        }
                                    });

                                let review_all_btn = egui::Button::new(
                                    egui::RichText::new("查看/编辑全部消息")
                                        .color(Color32::from_rgb(22, 24, 30))
                                        .strong(),
                                )
                                .fill(Color32::from_rgb(223, 239, 255))
                                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(120, 146, 185)));
                                if ui.add(review_all_btn).clicked() {
                                    self.start_decision_queue_edit(node.id);
                                }

                                ui.add_space(6.0 * zoom_scale);
                                ui.label("操作:");

                                ui.scope(|ui| {
                                    for button in buttons {
                                        let key = button.event_key.to_ascii_lowercase();
                                        let (base_fill, base_stroke) =
                                            if let Some([r, g, b]) = button.color_rgb {
                                                let fill = Color32::from_rgb(r, g, b);
                                                let stroke = Color32::from_rgb(
                                                    r.saturating_sub(30),
                                                    g.saturating_sub(30),
                                                    b.saturating_sub(30),
                                                );
                                                (fill, stroke)
                                            } else if key.contains("reject")
                                                || key.contains("deny")
                                                || key.contains("decline")
                                                || key.contains("fail")
                                            {
                                                (
                                                    Color32::from_rgb(248, 208, 208),
                                                    Color32::from_rgb(224, 150, 150),
                                                )
                                            } else if key.contains("approve")
                                                || key.contains("accept")
                                                || key.contains("pass")
                                                || key.contains("ok")
                                            {
                                                (
                                                    Color32::from_rgb(212, 244, 226),
                                                    Color32::from_rgb(126, 201, 165),
                                                )
                                            } else {
                                                (
                                                    Color32::from_rgb(224, 232, 242),
                                                    Color32::from_rgb(163, 177, 196),
                                                )
                                            };

                                        let (fill, text, stroke) = if has_pending {
                                            (base_fill, Color32::BLACK, base_stroke)
                                        } else {
                                            (
                                                Color32::from_rgb(188, 196, 205),
                                                Color32::from_rgb(83, 94, 108),
                                                Color32::from_rgb(150, 161, 174),
                                            )
                                        };

                                        ui.horizontal(|ui| {
                                            let row_height = (26.0 * zoom_scale).max(22.0);
                                            let show_process_all = queue_len > 1;
                                            let all_btn_width = if show_process_all {
                                                (58.0 * zoom_scale).max(48.0)
                                            } else {
                                                0.0
                                            };
                                            let gap = if show_process_all {
                                                ui.spacing().item_spacing.x
                                            } else {
                                                0.0
                                            };
                                            let row_width = ui.available_width();
                                            let main_width =
                                                (row_width - all_btn_width - gap).max(80.0);

                                            let clicked_one = ui
                                                .add_enabled(
                                                    has_pending,
                                                    egui::Button::new(
                                                        egui::RichText::new(button.label.as_str())
                                                            .color(text),
                                                    )
                                                    .fill(fill)
                                                    .stroke(Stroke::new(1.0, stroke))
                                                    .min_size(vec2(main_width, row_height)),
                                                )
                                                .clicked();
                                            if clicked_one {
                                                self.forward_decision_message_by_event(
                                                    node.id,
                                                    &button.event_key,
                                                    &button.label,
                                                    false,
                                                );
                                            }

                                            if show_process_all {
                                                let clicked_all = ui
                                                    .add_enabled(
                                                        has_pending,
                                                        egui::Button::new(
                                                            egui::RichText::new("全部").color(text),
                                                        )
                                                        .fill(fill)
                                                        .stroke(Stroke::new(1.0, stroke))
                                                        .min_size(vec2(all_btn_width, row_height)),
                                                    )
                                                    .clicked();
                                                if clicked_all {
                                                    self.forward_decision_message_by_event(
                                                        node.id,
                                                        &button.event_key,
                                                        &button.label,
                                                        true,
                                                    );
                                                }
                                            }
                                        });

                                        ui.add_space((4.0 * zoom_scale).max(2.0));
                                    }
                                });
                            });
                        }
                    }

                    if is_selected {
                        let handle_size = 12.0 * zoom_scale.clamp(0.75, 1.6);
                        let handle_rect = Rect::from_min_size(
                            node_rect.right_bottom() - vec2(handle_size + 6.0, handle_size + 6.0),
                            vec2(handle_size, handle_size),
                        );
                        painter.rect_filled(handle_rect, 2.0, Color32::from_rgb(168, 236, 188));
                    }
                }
                NodeKind::Image => {
                    if let Some(texture) = self.image_texture(node.id) {
                        painter.image(
                            texture.id(),
                            node_rect,
                            Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    } else if let Some(err) = self.image_error(node.id) {
                        painter.text(
                            node_rect.center(),
                            Align2::CENTER_CENTER,
                            err,
                            FontId::proportional((13.0 * zoom_scale).max(9.0)),
                            Color32::from_rgb(255, 170, 170),
                        );
                    } else {
                        painter.text(
                            node_rect.center(),
                            Align2::CENTER_CENTER,
                            "拖拽图片文件到画布\n或在画布粘贴图片路径",
                            FontId::proportional((13.0 * zoom_scale).max(9.0)),
                            Color32::from_rgb(205, 236, 242),
                        );
                    }

                    if is_selected {
                        let handle_size = 12.0 * zoom_scale.clamp(0.75, 1.6);
                        let handle_rect = Rect::from_min_size(
                            node_rect.right_bottom() - vec2(handle_size + 6.0, handle_size + 6.0),
                            vec2(handle_size, handle_size),
                        );
                        painter.rect_filled(handle_rect, 2.0, Color32::from_rgb(175, 230, 240));
                    }
                }
                NodeKind::Script => {
                    let (script_title_rect, script_code_rect) = self.draw_script_node_body(
                        painter,
                        node_rect,
                        zoom_scale,
                        stroke,
                        fill,
                        ui,
                        ctx,
                        node,
                        is_selected,
                    );
                    if let Some(rect) = script_title_rect {
                        title_edit_rect = Some(rect);
                    }
                    if let Some(rect) = script_code_rect {
                        script_edit_rect = Some(rect);
                    }
                }
                NodeKind::Group => {
                    painter.rect(
                        node_rect,
                        10.0 * zoom_scale,
                        fill,
                        stroke,
                        egui::StrokeKind::Outside,
                    );

                    let is_title_editing = self.editing_title_node == Some(node.id);
                    if !is_title_editing {
                        let title = match &node.data {
                            NodeData::Group { title, .. } => title.as_str(),
                            _ => "Group",
                        };
                        painter.text(
                            node_rect.left_top() + vec2(12.0, 10.0) * zoom_scale,
                            Align2::LEFT_TOP,
                            title,
                            FontId::proportional((14.0 * zoom_scale).max(9.0)),
                            Color32::from_rgb(220, 230, 255),
                        );
                    } else {
                        let rect_min = node_rect.left_top() + vec2(10.0, 6.0) * zoom_scale;
                        let rect_max = Pos2::new(
                            node_rect.max.x - 10.0 * zoom_scale,
                            (node_rect.min.y + GROUP_HEADER_HEIGHT * zoom_scale - 6.0 * zoom_scale)
                                .min(node_rect.max.y),
                        );
                        title_edit_rect = Some((node.id, Rect::from_min_max(rect_min, rect_max)));
                    }
                }
            }
        }

        (
            text_edit_rect,
            title_edit_rect,
            startup_edit_rect,
            decision_edit_rect,
            working_directory_edit_rect,
            script_edit_rect,
        )
    }

    /// Render the Script node body content (header, JSON editor, or widget tree + buttons).
    /// Extracted from draw_nodes() to keep that function under control.
    /// Returns the title edit rect if the title is being edited.
    fn draw_script_node_body(
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

        painter.rect_stroke(
            node_rect,
            8.0 * zoom_scale,
            stroke,
            egui::StrokeKind::Outside,
        );

        let header_height = crate::constants::SCRIPT_HEADER_HEIGHT * zoom_scale;
        let header_bottom = (node_rect.min.y + header_height).min(node_rect.max.y);
        let header_rect = Rect::from_min_max(
            node_rect.min,
            Pos2::new(node_rect.max.x, header_bottom),
        );
        let header_rounding = egui::CornerRadius {
            nw: (8.0 * zoom_scale).round() as u8,
            ne: (8.0 * zoom_scale).round() as u8,
            sw: 0,
            se: 0,
        };
        painter.rect_filled(header_rect, header_rounding, fill);

        let is_title_editing = self.editing_title_node == Some(node.id);
        if !is_title_editing {
            let title = match &node.data {
                NodeData::Script { title, .. } => title.as_str(),
                _ => "Script",
            };
            painter.text(
                Pos2::new(node_rect.min.x + 12.0 * zoom_scale, header_rect.center().y),
                Align2::LEFT_CENTER,
                title,
                FontId::proportional((16.0 * zoom_scale).max(10.0)),
                Color32::from_rgb(220, 200, 240),
            );

            let status = if let Some(err) = self.script_lua_errors.get(&node.id) {
                let low = err.to_lowercase();
                if low.contains("hookerror") || low.contains("instruction") || low.contains("timeout") {
                    ("Frozen", Color32::from_rgb(230, 185, 95))
                } else {
                    ("Error", Color32::from_rgb(220, 90, 100))
                }
            } else if self.script_lua_runtimes.contains_key(&node.id) {
                if self
                    .script_lua_runtimes
                    .get(&node.id)
                    .map(|rt| rt.timer_interval() > 0.0)
                    .unwrap_or(false)
                {
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
                (node_rect.min.y + header_height - 6.0 * zoom_scale)
                    .min(node_rect.max.y),
            );
            title_edit_rect = Some((node.id, Rect::from_min_max(rect_min, rect_max)));
        }

        painter.line_segment(
            [
                Pos2::new(node_rect.min.x, header_bottom),
                Pos2::new(node_rect.max.x, header_bottom),
            ],
            Stroke::new(
                1.0 * zoom_scale.clamp(0.6, 1.6),
                Color32::from_rgb(130, 108, 170),
            ),
        );

        // No extra padding — content fills the node edge-to-edge
        let content_rect = Rect::from_min_max(
            Pos2::new(
                node_rect.min.x,
                header_bottom,
            ),
            node_rect.max,
        );

        if content_rect.is_positive() {
            let is_editing = self.editing_script_node == Some(node.id);

            if is_editing && self.script_debug_node != Some(node.id) {
                script_edit_rect = Some((node.id, content_rect));
            } else {
                let zoom = zoom_scale;
                let is_debug = self.script_debug_node == Some(node.id);

                let toolbar_h = (54.0 * zoom).max(40.0);
                let toolbar_rect = Rect::from_min_size(
                    content_rect.left_top(),
                    vec2(content_rect.width(), toolbar_h),
                );
                let mut deferred_review = false;
                let mut deferred_exit_debug = false;
                let body_rect = if is_debug {
                    Rect::from_min_max(
                        Pos2::new(content_rect.min.x, content_rect.min.y + toolbar_h),
                        content_rect.max,
                    )
                } else {
                    content_rect
                };
                if toolbar_rect.is_positive() {
                    ui.painter().rect_filled(
                        toolbar_rect,
                        0.0,
                        Color32::from_rgba_premultiplied(24, 28, 46, 200),
                    );
                    ui.painter().line_segment(
                        [
                            Pos2::new(toolbar_rect.left(), toolbar_rect.bottom()),
                            Pos2::new(toolbar_rect.right(), toolbar_rect.bottom()),
                        ],
                        Stroke::new(1.0, Color32::from_rgba_premultiplied(130, 108, 170, 60)),
                    );
                    if let Some(line) = self.script_lua_pause_line.get(&node.id).copied() {
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
                    let review_w = (120.0 * zoom).max(80.0);
                    let btn_y = toolbar_rect.top() + 3.0 * zoom;
                    let right_edge = if is_debug {
                        let exit_w = (84.0 * zoom).max(66.0);
                        let exit_rect = Rect::from_min_size(
                            Pos2::new(toolbar_rect.right() - exit_w - 6.0 * zoom, btn_y),
                            vec2(exit_w, btn_h),
                        );
                        let exit_btn = egui::Button::new(
                            egui::RichText::new("退出调试")
                                .color(Color32::from_rgb(22, 24, 30))
                                .size((11.0 * zoom).max(9.0)),
                        )
                        .fill(Color32::from_rgb(238, 220, 220))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(190, 130, 130)))
                        .corner_radius(4.0)
                        .min_size(vec2(exit_w, btn_h));
                        if ui.put(exit_rect, exit_btn).clicked() {
                            deferred_exit_debug = true;
                        }
                        exit_rect.left() - 6.0 * zoom
                    } else {
                        toolbar_rect.right() - 6.0 * zoom
                    };
                    let review_btn_rect = Rect::from_min_size(
                        Pos2::new(right_edge - review_w, btn_y),
                        vec2(review_w, btn_h),
                    );
                    let review_btn = egui::Button::new(
                        egui::RichText::new("📋 队列")
                            .color(Color32::from_rgb(22, 24, 30))
                            .size((11.0 * zoom).max(9.0)),
                    )
                    .fill(Color32::from_rgb(228, 234, 250))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(130, 146, 185)))
                    .corner_radius(4.0)
                    .min_size(vec2(review_w, btn_h));
                    if ui.put(review_btn_rect, review_btn).clicked() {
                        deferred_review = true;
                    }

                    let step_w = (76.0 * zoom).max(60.0);
                    let step_rect = Rect::from_min_size(
                        Pos2::new(review_btn_rect.left() - step_w - 6.0 * zoom, btn_y),
                        vec2(step_w, btn_h),
                    );
                    let step_btn = egui::Button::new(
                        egui::RichText::new("Step")
                            .color(Color32::from_rgb(22, 24, 30))
                            .size((11.0 * zoom).max(9.0)),
                    )
                    .fill(Color32::from_rgb(228, 234, 250))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(130, 146, 185)))
                    .corner_radius(4.0)
                    .min_size(vec2(step_w, btn_h));
                    if ui.put(step_rect, step_btn).clicked() {
                        if self.ensure_script_lua_runtime(node.id).is_ok() {
                            if let Some(rt) = self.script_lua_runtimes.get_mut(&node.id) {
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
                    let bp_input = self.script_lua_breakpoint_input.entry(node.id).or_insert_with(String::new);
                    let bp_resp = ui.put(
                        bp_rect,
                        egui::TextEdit::singleline(bp_input)
                            .hint_text("bp行号")
                            .text_color(Color32::from_rgb(215, 220, 230))
                            .background_color(Color32::from_rgb(30, 34, 48)),
                    );
                    if bp_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if let Ok(line) = bp_input.trim().parse::<i32>() {
                            let set = self.script_lua_breakpoints.entry(node.id).or_default();
                            let enable = !set.contains(&line);
                            if enable {
                                set.insert(line);
                            } else {
                                set.remove(&line);
                            }
                            if let Some(rt) = self.script_lua_runtimes.get_mut(&node.id) {
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
                        egui::RichText::new("清空BP")
                            .color(Color32::from_rgb(22, 24, 30))
                            .size((10.0 * zoom).max(9.0)),
                    )
                    .fill(Color32::from_rgb(238, 220, 220))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(190, 130, 130)))
                    .corner_radius(4.0)
                    .min_size(vec2(clear_w, btn_h));
                    if ui.put(clear_rect, clear_btn).clicked() {
                        if let Some(existing) = self.script_lua_breakpoints.get(&node.id).cloned() {
                            if let Some(rt) = self.script_lua_runtimes.get_mut(&node.id) {
                                for line in existing {
                                    let _ = rt.set_breakpoint(line, false);
                                }
                            }
                        }
                        self.script_lua_breakpoints.remove(&node.id);
                    }

                    let bp_summary = self
                        .script_lua_breakpoints
                        .get(&node.id)
                        .map(|set| {
                            let mut v: Vec<i32> = set.iter().copied().collect();
                            v.sort_unstable();
                            v.into_iter().map(|n| n.to_string()).collect::<Vec<_>>().join(",")
                        })
                        .unwrap_or_default();
                    let bp_text = if bp_summary.is_empty() {
                        "BP: -".to_owned()
                    } else {
                        format!("BP: {}", bp_summary)
                    };
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
                    Rect::from_min_max(
                        Pos2::new(content_rect.min.x, content_rect.min.y + toolbar_h),
                        content_rect.max,
                    )
                };

                if self.ensure_script_lua_runtime(node.id).is_ok() {
                    if let Some(rt) = self.script_lua_runtimes.get_mut(&node.id) {
                        let events = match rt.capture_render() {
                            Ok(v) => v,
                            Err(err) => {
                                let lower = err.to_lowercase();
                                if lower.contains("debug breakpoint hit") || lower.contains("调试中断") {
                                    if let Some(line) = rt.take_debug_pause_line() {
                                        self.script_lua_pause_line.insert(node.id, line);
                                    }
                                    if let Ok(vars) = rt.debug_variables_snapshot() {
                                        self.script_lua_debug_vars.insert(node.id, vars.to_string());
                                    }
                                    self.script_lua_errors.remove(&node.id);
                                    Vec::new()
                                } else {
                                    let tagged = if lower.contains("hook") || lower.contains("instruction") || lower.contains("timeout") {
                                        format!("[HookError] {err}")
                                    } else {
                                        format!("[RuntimeError] {err}")
                                    };
                                    eprintln!("[script-node:{}] capture_render failed: {}", node.id, tagged);
                                    self.script_lua_errors.insert(node.id, tagged);
                                    Vec::new()
                                }
                            }
                        };

                        let mut body_ui = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(widget_rect)
                                .layout(Layout::top_down(Align::Min)),
                        );
                        body_ui.set_clip_rect(widget_rect);

                        for event in events {
                            match event {
                                crate::script_node::lua::api_ctx::UiEvent::Text { text, .. } => {
                                    body_ui.label(text);
                                }
                                crate::script_node::lua::api_ctx::UiEvent::ButtonWithCallback { label, event_key, enabled, .. } => {
                                    let resp = body_ui.add_enabled(enabled, egui::Button::new(label.clone()));
                                    if resp.clicked() && enabled {
                                        let key = event_key.as_deref().unwrap_or(&label).to_owned();
                                        if let Some(rt) = self.script_lua_runtimes.get_mut(&node.id) {
                                            rt.queue_button_click(&key);
                                        }
                                        self.mark_workspace_dirty();
                                        ctx.request_repaint();
                                    }
                                }
                                crate::script_node::lua::api_ctx::UiEvent::Button { label, enabled, .. } => {
                                    let resp = body_ui.add_enabled(enabled, egui::Button::new(label.clone()));
                                    if resp.clicked() && enabled {
                                        if let Some(rt) = self.script_lua_runtimes.get_mut(&node.id) {
                                            rt.queue_button_click(&label);
                                        }
                                        self.mark_workspace_dirty();
                                        ctx.request_repaint();
                                    }
                                }
                                crate::script_node::lua::api_ctx::UiEvent::Input { label, mut value, enabled, multiline, rows, .. } => {
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
                                            if let Some(rt) = self.script_lua_runtimes.get_mut(&node.id) {
                                                rt.queue_input_value(&key, &value);
                                            }
                                            self.mark_workspace_dirty();
                                            ctx.request_repaint();
                                        }
                                    });
                                }
                                crate::script_node::lua::api_ctx::UiEvent::Slider { label, mut value, min, max, enabled } => {
                                    body_ui.horizontal(|ui| {
                                        if !label.is_empty() { ui.label(label.clone()); }
                                        let resp = ui.add_enabled(enabled, egui::Slider::new(&mut value, min..=max));
                                        if resp.changed() {
                                            let val_str = format!("{value}");
                                            let port_name = if label.is_empty() { "slider".to_owned() } else { label.clone() };
                                            self.script_node_outputs.entry(node.id).or_default().insert(port_name.clone(), val_str.clone());
                                            let targets: Vec<(usize, Option<String>)> = self.edges.iter()
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
                                crate::script_node::lua::api_ctx::UiEvent::Separator { .. } => { body_ui.separator(); }
                                crate::script_node::lua::api_ctx::UiEvent::Spacer(h) => { body_ui.add_space(h); }
                                crate::script_node::lua::api_ctx::UiEvent::Badge { text, .. } => { body_ui.label(text); }
                                crate::script_node::lua::api_ctx::UiEvent::Card { text, caption } => {
                                    body_ui.group(|ui| {
                                        ui.label(text);
                                        if let Some(c) = caption { ui.small(c); }
                                    });
                                }
                                crate::script_node::lua::api_ctx::UiEvent::ProgressBar { value, .. } => {
                                    body_ui.add(egui::ProgressBar::new(value as f32));
                                }
                                _ => {}
                            }
                        }
                    }
                }

                if let Some(vars) = self.script_lua_debug_vars.get(&node.id).cloned() {
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
                            egui::UiBuilder::new()
                                .max_rect(text_rect)
                                .layout(Layout::top_down(Align::Min)),
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

                if let Some(err) = self.script_lua_errors.get(&node.id).cloned() {
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
                            self.script_lua_errors.remove(&node.id);
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
                            egui::UiBuilder::new()
                                .max_rect(text_rect)
                                .layout(Layout::top_down(Align::Min)),
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
                if deferred_exit_debug {
                    self.stop_script_debug(node.id);
                }
            }
        }

        if is_selected {
            let handle_size = 12.0 * zoom_scale.clamp(0.75, 1.6);
            let handle_rect = Rect::from_min_size(
                node_rect.right_bottom() - vec2(handle_size + 6.0, handle_size + 6.0),
                vec2(handle_size, handle_size),
            );
            painter.rect_filled(handle_rect, 2.0, Color32::from_rgb(200, 140, 255));
        }

        (title_edit_rect, script_edit_rect)
    }
}

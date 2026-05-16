use super::super::GraphApp;
use crate::constants::{DECISION_HEADER_HEIGHT, GROUP_HEADER_HEIGHT, HTML_HEADER_HEIGHT, WEBPAGE_HEADER_HEIGHT};
use crate::model::{NodeData, NodeKind};
use eframe::egui::{
    self, vec2, Align, Align2, Color32, FontId, Layout, Painter, Pos2, Rect, Stroke,
};
use egui_commonmark::CommonMarkViewer;

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
        let mut webpage_url_edit_rect: Option<(usize, Rect)> = None;

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
                NodeKind::Html => {
                    let fill = if is_selected {
                        Color32::from_rgb(38, 59, 84)
                    } else {
                        Color32::from_rgb(28, 45, 66)
                    };
                    let stroke = if is_selected {
                        Stroke::new(
                            2.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(132, 205, 255),
                        )
                    } else {
                        Stroke::new(
                            1.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(88, 134, 176),
                        )
                    };
                    (fill, stroke)
                }
                NodeKind::WebPage => {
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

            if matches!(node.kind, NodeKind::Text | NodeKind::Html | NodeKind::WebPage) {
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
                    painter.rect_filled(header_rect, 8.0 * zoom_scale, fill);

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
                NodeKind::Html => {
                    painter.rect_stroke(
                        node_rect,
                        8.0 * zoom_scale,
                        stroke,
                        egui::StrokeKind::Outside,
                    );

                    let header_height = HTML_HEADER_HEIGHT * zoom_scale;
                    let header_bottom = (node_rect.min.y + header_height).min(node_rect.max.y);
                    let header_rect = Rect::from_min_max(
                        node_rect.min,
                        Pos2::new(node_rect.max.x, header_bottom),
                    );
                    painter.rect_filled(header_rect, 8.0 * zoom_scale, fill);

                    let is_editing = self.editing_text_node == Some(node.id);

                    // Title in header
                    let title = "HTML Node";
                    painter.text(
                        Pos2::new(node_rect.min.x + 12.0 * zoom_scale, header_rect.center().y),
                        Align2::LEFT_CENTER,
                        title,
                        FontId::proportional((14.0 * zoom_scale).max(9.0)),
                        Color32::from_rgb(196, 226, 255),
                    );

                    // Status badge in header
                    let (status_text, status_color) = if is_editing {
                        ("EDIT", Color32::from_rgb(255, 200, 100))
                    } else {
                        ("LIVE", Color32::from_rgb(140, 255, 190))
                    };
                    painter.text(
                        Pos2::new(node_rect.max.x - 12.0 * zoom_scale, header_rect.center().y),
                        Align2::RIGHT_CENTER,
                        status_text,
                        FontId::proportional((11.0 * zoom_scale).max(8.0)),
                        status_color,
                    );

                    // Separator line below header
                    painter.line_segment(
                        [
                            Pos2::new(node_rect.min.x, header_bottom),
                            Pos2::new(node_rect.max.x, header_bottom),
                        ],
                        Stroke::new(
                            1.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(88, 134, 176),
                        ),
                    );

                    let content_padding = 10.0;
                    let content_rect = Rect::from_min_max(
                        Pos2::new(
                            node_rect.min.x + content_padding * zoom_scale,
                            header_bottom + content_padding * zoom_scale,
                        ),
                        node_rect.max - vec2(content_padding, content_padding) * zoom_scale,
                    );

                    // Fallback preview when host is unavailable (no webview will be created)
                    if !is_editing && content_rect.is_positive() && !self.html_host_available() {
                        let preview = match &node.data {
                            NodeData::Html { html_source } if html_source.trim().is_empty() => {
                                "(空 HTML)"
                            }
                            NodeData::Html { html_source } => html_source.as_str(),
                            _ => "(空 HTML)",
                        };
                        let snippet: String = preview.chars().take(360).collect();
                        let preview_text = format!(
                            "HTML / CSS (HOST_MISSING)\n\n{}",
                            snippet,
                        );

                        let mut html_ui = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(content_rect)
                                .layout(Layout::top_down(Align::Min)),
                        );
                        html_ui.set_clip_rect(content_rect);
                        html_ui.set_width(content_rect.width());

                        egui::ScrollArea::vertical()
                            .id_salt(("html-node-preview-scroll", node.id))
                            .auto_shrink([false, false])
                            .show(&mut html_ui, |ui| {
                                ui.set_width(content_rect.width());
                                ui.style_mut().visuals.override_text_color =
                                    Some(Color32::from_rgb(222, 232, 244));
                                ui.label(
                                    egui::RichText::new(preview_text)
                                        .monospace()
                                        .color(Color32::from_rgb(222, 232, 244)),
                                );
                            });
                    }

                    if is_editing {
                        text_edit_rect = Some((node.id, content_rect));
                    } else if content_rect.is_positive()
                        && self.occluded_webview_ids.contains(&node.id)
                    {
                        // Occlusion hint: webview is hidden behind another node
                        let hint = "被遮挡";
                        let font_size = (14.0 * zoom_scale).max(9.0);
                        painter.text(
                            content_rect.center(),
                            Align2::CENTER_CENTER,
                            hint,
                            FontId::proportional(font_size),
                            Color32::from_rgba_premultiplied(140, 170, 210, 160),
                        );
                    }

                    if is_selected {
                        let handle_size = 12.0 * zoom_scale.clamp(0.75, 1.6);
                        let handle_rect = Rect::from_min_size(
                            // Draw handle outside the node rect to avoid webview conflict
                            node_rect.right_bottom() + vec2(4.0, 4.0),
                            vec2(handle_size, handle_size),
                        );
                        painter.rect_filled(handle_rect, 2.0, Color32::from_rgb(132, 205, 255));
                    }
                }
                NodeKind::WebPage => {
                    painter.rect_stroke(
                        node_rect,
                        8.0 * zoom_scale,
                        stroke,
                        egui::StrokeKind::Outside,
                    );

                    let header_height = WEBPAGE_HEADER_HEIGHT * zoom_scale;
                    let header_bottom = (node_rect.min.y + header_height).min(node_rect.max.y);
                    let header_rect = Rect::from_min_max(
                        node_rect.min,
                        Pos2::new(node_rect.max.x, header_bottom),
                    );
                    painter.rect_filled(header_rect, 8.0 * zoom_scale, fill);

                    let is_editing = self.editing_text_node == Some(node.id);
                    let is_url_editing = self.editing_webpage_url_node == Some(node.id);

                    let current_url = match &node.data {
                        NodeData::WebPage { url } => url.as_str(),
                        _ => "",
                    };

                    if is_url_editing {
                        // Calculate and store the URL edit rect
                        let url_edit_input_rect = Rect::from_min_max(
                            Pos2::new(
                                node_rect.min.x + 10.0 * zoom_scale,
                                node_rect.min.y + 4.0 * zoom_scale,
                            ),
                            Pos2::new(
                                node_rect.max.x - 54.0 * zoom_scale,
                                header_bottom - 4.0 * zoom_scale,
                            ),
                        );
                        if url_edit_input_rect.is_positive() {
                            webpage_url_edit_rect = Some((node.id, url_edit_input_rect));
                        }
                    } else {
                        // Show URL in header as clickable address bar
                        let display_url = if current_url.is_empty() {
                            "输入网址..."
                        } else {
                            current_url
                        };

                        let lock_icon = if current_url.starts_with("https://") {
                            "\u{1f512}"
                        } else {
                            "\u{1f513}"
                        };
                        let full_text = format!("{} {}", lock_icon, display_url);

                        // Simple truncation by character count
                        let approx_char_width = (8.0 * zoom_scale).max(6.0);
                        let url_max_chars = ((node_rect.width() - 64.0) * zoom_scale / approx_char_width).max(6.0) as usize;
                        let url_text = if full_text.chars().count() > url_max_chars {
                            let truncated: String = full_text.chars().take(url_max_chars.saturating_sub(1)).collect();
                            format!("{}…", truncated)
                        } else {
                            full_text
                        };

                        painter.text(
                            Pos2::new(node_rect.min.x + 10.0 * zoom_scale, header_rect.center().y),
                            Align2::LEFT_CENTER,
                            &url_text,
                            FontId::proportional((12.0 * zoom_scale).max(8.0)),
                            Color32::from_rgb(196, 246, 255),
                        );
                    }

                    // Status badge in header
                    let (status_text, status_color) = if is_editing {
                        ("EDIT", Color32::from_rgb(255, 200, 100))
                    } else {
                        ("LIVE", Color32::from_rgb(140, 255, 190))
                    };
                    painter.text(
                        Pos2::new(node_rect.max.x - 12.0 * zoom_scale, header_rect.center().y),
                        Align2::RIGHT_CENTER,
                        status_text,
                        FontId::proportional((11.0 * zoom_scale).max(8.0)),
                        status_color,
                    );

                    // Separator line below header
                    painter.line_segment(
                        [
                            Pos2::new(node_rect.min.x, header_bottom),
                            Pos2::new(node_rect.max.x, header_bottom),
                        ],
                        Stroke::new(
                            1.0 * zoom_scale.clamp(0.6, 1.6),
                            Color32::from_rgb(78, 145, 160),
                        ),
                    );

                    let content_padding = 10.0;
                    let content_rect = Rect::from_min_max(
                        Pos2::new(
                            node_rect.min.x + content_padding * zoom_scale,
                            header_bottom + content_padding * zoom_scale,
                        ),
                        node_rect.max - vec2(content_padding, content_padding) * zoom_scale,
                    );

                    // Fallback preview when host is unavailable
                    if !is_editing && content_rect.is_positive() && !self.html_host_available() {
                        let url_display = match &node.data {
                            NodeData::WebPage { url } if url.trim().is_empty() => {
                                "(空 URL)"
                            }
                            NodeData::WebPage { url } => url.as_str(),
                            _ => "(空 URL)",
                        };
                        let preview_text = format!(
                            "WebPage (HOST_MISSING)\n\nURL: {}",
                            url_display,
                        );

                        let mut webpage_ui = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(content_rect)
                                .layout(Layout::top_down(Align::Min)),
                        );
                        webpage_ui.set_clip_rect(content_rect);
                        webpage_ui.set_width(content_rect.width());

                        egui::ScrollArea::vertical()
                            .id_salt(("webpage-node-preview-scroll", node.id))
                            .auto_shrink([false, false])
                            .show(&mut webpage_ui, |ui| {
                                ui.set_width(content_rect.width());
                                ui.style_mut().visuals.override_text_color =
                                    Some(Color32::from_rgb(210, 242, 248));
                                ui.label(
                                    egui::RichText::new(preview_text)
                                        .color(Color32::from_rgb(210, 242, 248)),
                                );
                            });
                    }

                    if is_editing {
                        text_edit_rect = Some((node.id, content_rect));
                    } else if content_rect.is_positive()
                        && self.occluded_webview_ids.contains(&node.id)
                    {
                        let hint = "被遮挡";
                        let font_size = (14.0 * zoom_scale).max(9.0);
                        painter.text(
                            content_rect.center(),
                            Align2::CENTER_CENTER,
                            hint,
                            FontId::proportional(font_size),
                            Color32::from_rgba_premultiplied(140, 170, 210, 160),
                        );
                    }

                    if is_selected {
                        let handle_size = 12.0 * zoom_scale.clamp(0.75, 1.6);
                        let handle_rect = Rect::from_min_size(
                            node_rect.right_bottom() + vec2(4.0, 4.0),
                            vec2(handle_size, handle_size),
                        );
                        painter.rect_filled(handle_rect, 2.0, Color32::from_rgb(124, 220, 240));
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
            webpage_url_edit_rect,
        )
    }
}

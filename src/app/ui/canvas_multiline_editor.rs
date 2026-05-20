use eframe::egui::{self, epaint, vec2, Align, Color32, FontId, Layout, Pos2, Rect, TextEdit, Ui};
use std::sync::Arc;

pub(super) struct GutterLine {
    pub line: usize,
    pub marker: &'static str,
    pub color: Color32,
}

pub(super) struct CanvasMultilineEditorOutput {
    pub response: egui::Response,
    pub gutter_clicked_line: Option<usize>,
    pub pointer_over_editor: bool,
}

pub(super) fn show_canvas_multiline_editor(
    ui: &mut Ui,
    edit_rect: Rect,
    scroll_id: impl std::hash::Hash,
    edit_id: egui::Id,
    text: &mut String,
    font_id: FontId,
    text_color: Color32,
    row_colors: Option<(Color32, Color32)>,
    gutter_w: Option<f32>,
    gutter_line: impl Fn(usize) -> GutterLine,
    layouter: Option<&mut dyn FnMut(&egui::Ui, &str, f32) -> Arc<egui::Galley>>,
) -> CanvasMultilineEditorOutput {
    let editor_response = ui.interact(
        edit_rect,
        egui::Id::new(("canvas-multiline-editor-hitbox", edit_id)),
        egui::Sense::click_and_drag(),
    );
    let pointer_over_editor = editor_response.hovered()
        || ui.ctx().input(|i| {
            i.pointer
                .latest_pos()
                .or_else(|| i.pointer.hover_pos())
                .is_some_and(|pos| edit_rect.contains(pos))
        });
    if pointer_over_editor {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
    }

    let mut editor_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(edit_rect)
            .layout(Layout::top_down(Align::Min)),
    );
    editor_ui.set_clip_rect(edit_rect);

    {
        let style = editor_ui.style_mut();
        style.visuals.extreme_bg_color = Color32::from_rgb(20, 22, 34);
        style.visuals.faint_bg_color = Color32::from_rgb(20, 22, 34);
        style.spacing.scroll.foreground_color = true;
    }

    let line_count = text.lines().count().max(1);
    let min_rows = line_count.max(10);
    let gutter_w = gutter_w.unwrap_or(0.0);
    let editor_w = (edit_rect.width() - gutter_w).max(120.0);
    let mut gutter_clicked_line = None;
    let mut layouter = layouter;

    let mut measure_layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
        let galley = if let Some(layouter) = layouter.as_deref_mut() {
            layouter(ui, text, wrap_width)
        } else {
            let mut job = egui::text::LayoutJob::simple(
                text.to_owned(),
                font_id.clone(),
                text_color,
                f32::INFINITY,
            );
            job.wrap.max_width = f32::INFINITY;
            ui.fonts(|f| f.layout_job(job))
        };
        galley
    };

    let scroll_id = egui::Id::new(scroll_id);
    let mut page_scroll_delta = 0.0_f32;
    if ui.memory(|m| m.has_focus(edit_id)) || pointer_over_editor {
        page_scroll_delta = ui.ctx().input_mut(|i| {
            let page = (edit_rect.height() * 0.85).max(24.0);
            let mut delta = 0.0;
            if i.consume_key(egui::Modifiers::NONE, egui::Key::PageUp)
                || i.consume_key(egui::Modifiers::SHIFT, egui::Key::PageUp)
            {
                delta -= page;
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::PageDown)
                || i.consume_key(egui::Modifiers::SHIFT, egui::Key::PageDown)
            {
                delta += page;
            }
            delta
        });
    }

    let response = egui::ScrollArea::both()
        .id_salt(scroll_id)
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
        .auto_shrink([false, false])
        .show_viewport(&mut editor_ui, |ui, _viewport| {
            ui.set_min_width(edit_rect.width());

            if page_scroll_delta != 0.0 {
                let target_y = ui.clip_rect().center().y + page_scroll_delta;
                ui.scroll_to_rect(
                    Rect::from_min_size(Pos2::new(ui.clip_rect().left(), target_y), vec2(1.0, 1.0)),
                    Some(Align::Center),
                );
            }

            let bg_shape_idx = ui.painter().add(egui::Shape::Noop);
            let output = ui
                .horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.add_space(gutter_w);

                    TextEdit::multiline(text)
                        .id(edit_id)
                        .font(font_id.clone())
                        .text_color(text_color)
                        .margin(egui::Margin::ZERO)
                        .background_color(Color32::TRANSPARENT)
                        .desired_width(editor_w)
                        .desired_rows(min_rows)
                        .frame(false)
                        .layouter(&mut measure_layouter)
                        .show(ui)
                })
                .inner;
            let source_lines: Vec<&str> = text.split('\n').collect();
            let mut line_no = 1_usize;
            let mut remaining_chars = source_lines.first().map_or(0, |line| line.chars().count());
            let galley_rows: Vec<(usize, bool, f32, f32)> = output
                .galley
                .rows
                .iter()
                .map(|row| {
                    let current = line_no;
                    let is_first_visual_row = remaining_chars
                        == source_lines
                            .get(line_no.saturating_sub(1))
                            .map_or(0, |line| line.chars().count());
                    let consumed = row.char_count_excluding_newline().min(remaining_chars);
                    remaining_chars = remaining_chars.saturating_sub(consumed);
                    if row.ends_with_newline {
                        line_no += 1;
                        remaining_chars = source_lines
                            .get(line_no.saturating_sub(1))
                            .map_or(0, |line| line.chars().count());
                    }
                    (current, is_first_visual_row, row.min_y(), row.height())
                })
                .collect();
            let content_w = (gutter_w + output.galley.size().x.max(editor_w)).max(edit_rect.width());
            if let Some((even, odd)) = row_colors {
                let mut bg_shapes = Vec::new();
                for (line, _show_gutter, y, height) in &galley_rows {
                    let screen_y = output.galley_pos.y + *y;
                    if screen_y + *height < edit_rect.top() || screen_y > edit_rect.bottom() {
                        continue;
                    }
                    let row_rect = Rect::from_min_size(
                        Pos2::new(edit_rect.left(), screen_y),
                        vec2(content_w, *height),
                    );
                    bg_shapes.push(egui::Shape::Rect(epaint::RectShape::filled(
                        row_rect,
                        0.0,
                        if line % 2 == 1 { even } else { odd },
                    )));
                }
                ui.painter().set(bg_shape_idx, egui::Shape::Vec(bg_shapes));
            }

            if output.response.dragged() {
                if let Some(pointer) = ui.ctx().pointer_interact_pos() {
                    let edge_margin = 18.0;
                    let speed = 14.0;
                    if pointer.y < edit_rect.top() + edge_margin {
                        ui.scroll_to_rect(
                            Rect::from_min_size(
                                Pos2::new(output.galley_pos.x, output.galley_pos.y - speed),
                                vec2(1.0, 1.0),
                            ),
                            Some(Align::Min),
                        );
                        ui.ctx().request_repaint();
                    } else if pointer.y > edit_rect.bottom() - edge_margin {
                        ui.scroll_to_rect(
                            Rect::from_min_size(
                                Pos2::new(output.galley_pos.x, output.galley_pos.y + output.galley.size().y + speed),
                                vec2(1.0, 1.0),
                            ),
                            Some(Align::Max),
                        );
                        ui.ctx().request_repaint();
                    }
                }
            }

            if gutter_w > 0.0 {
                let gutter_painter = ui.painter().clone().with_clip_rect(edit_rect);
                for (line, show_gutter, y, height) in &galley_rows {
                    if !show_gutter {
                        continue;
                    }
                    let screen_y = output.galley_pos.y + *y;
                    if screen_y + *height < edit_rect.top() || screen_y > edit_rect.bottom() {
                        continue;
                    }
                    let GutterLine { line: gutter_line_no, marker, color } = gutter_line(*line);
                    let line_width = text.lines().count().max(1).to_string().len().max(2);
                    let label = format!("{gutter_line_no:>line_width$} {marker}");
                    let pos = Pos2::new(edit_rect.left() + 4.0, screen_y + (*height - font_id.size) * 0.5);
                    gutter_painter.text(
                        pos,
                        egui::Align2::LEFT_TOP,
                        label,
                        font_id.clone(),
                        color,
                    );
                    let click_rect = Rect::from_min_size(
                        Pos2::new(edit_rect.left(), screen_y),
                        vec2(gutter_w, *height),
                    );
                    let resp = ui.interact(
                        click_rect,
                        egui::Id::new(("canvas-multiline-gutter", edit_id, line)),
                        egui::Sense::click(),
                    );
                    if resp.clicked() {
                        gutter_clicked_line = Some(*line);
                    }
                }
            }

            output.response
        })
        .inner;

    CanvasMultilineEditorOutput {
        response,
        gutter_clicked_line,
        pointer_over_editor,
    }
}

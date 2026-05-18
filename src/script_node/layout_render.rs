use crate::script_node::parser::bind_text;
use crate::script_node::types::*;
use eframe::egui::{self, vec2, Align2, Color32, FontId, Pos2, Rect, Stroke};
use std::collections::HashMap;

/// Context for rendering and layout computation.
pub struct RenderContext<'a> {
    pub theme: &'a Theme,
    pub ui: &'a mut egui::Ui,
    pub zoom: f32,
    pub input_values: &'a HashMap<String, String>,
    pub output_values: &'a mut HashMap<String, String>,
    pub state_values: &'a HashMap<String, String>,
    pub events: &'a mut Vec<ScriptEvent>,
    /// Counter to generate unique IDs for interactive widgets
    pub id_counter: &'a mut u64,
}

impl<'a> RenderContext<'a> {
    fn next_id(&mut self) -> egui::Id {
        let id = *self.id_counter;
        *self.id_counter += 1;
        egui::Id::new(("script_node_widget", id))
    }

    fn resolve_color(&self, color_str: &str, fallback: Color32) -> Color32 {
        if let Some(spec) = ColorSpec::parse(color_str) {
            spec.resolve(self.theme, fallback)
        } else {
            fallback
        }
    }
}

/// Layout & render a widget tree into the given content rect.
/// Returns the total height used.
pub fn layout_and_render(
    ctx: &mut RenderContext,
    widget: &Widget,
    style: &Style,
    rect: Rect,
) -> f32 {
    match widget {
        Widget::Col { children, gap, padding, .. } => {
            render_col(ctx, children, *gap, padding, style, rect)
        }
        Widget::Row { children, gap, padding, .. } => {
            render_row(ctx, children, *gap, padding, style, rect)
        }
        Widget::Text { text, .. } => render_text(ctx, text, style, rect),
        Widget::Button { text, event, .. } => render_button(ctx, text, event, style, rect),
        Widget::Slider {
            name,
            label,
            min,
            max,
            default,
            ..
        } => render_slider(ctx, name, label, *min, *max, *default, style, rect),
        Widget::Input {
            name,
            label,
            placeholder,
            ..
        } => render_input(ctx, name, label, placeholder, style, rect),
        Widget::Bar {
            value,
            max,
            height,
            track,
            fill,
            ..
        } => render_bar(ctx, value, *max, *height, track, fill, style, rect),
        Widget::Spacer { height } => *height,
        Widget::Divider { .. } => render_divider(ctx, style, rect),
        Widget::Badge { text, color, .. } => render_badge(ctx, text, color, style, rect),
        Widget::Image { url, .. } => {
            // Icon/image rendering: for now, render the URL as text if it looks like an emoji
            let text = if url.chars().all(|c| c.len_utf8() > 1 || c.is_ascii_punctuation()) {
                url.clone()
            } else {
                format!("[img:{url}]")
            };
            let mut merged = Style::default();
            merged.font_size = Some(ctx.theme.font_size() * 1.4);
            render_text(ctx, &text, &merged, rect)
        }
    }
}

// ──────────────────────────────────────────────
// COL
// ──────────────────────────────────────────────

fn render_col(
    ctx: &mut RenderContext,
    children: &[Widget],
    gap: f32,
    padding: &Option<[f32; 4]>,
    style: &Style,
    rect: Rect,
) -> f32 {
    let pad = padding.unwrap_or([0.0; 4]);
    let inner_x = rect.min.x + pad[3]; // left
    let inner_y = rect.min.y + pad[0]; // top
    let inner_w = (rect.width() - pad[1] - pad[3]).max(0.0);
    let available_h = (rect.height() - pad[0] - pad[2]).max(0.0);

    // Draw background
    draw_bg(ctx, style, rect);

    // First pass: measure fixed-height children
    let fixed_total: f32;
    let flexible_count: usize;

    {
        let mut fixed_sum = 0.0;
        let mut flex_count = 0;
        for child in children {
            let child_style = get_child_style(child);
            if let Some(h) = child_style.height {
                fixed_sum += h * ctx.zoom;
                flex_count += 1; // fixed height still counts towards sizing
            } else {
                flex_count += 1;
            }
        }
        fixed_total = fixed_sum;
        flexible_count = flex_count;
    }

    // Distribute remaining height flexibly, or let children grow
    let total_gap = gap * (children.len().saturating_sub(1) as f32) * ctx.zoom;
    let remaining_h = (available_h - fixed_total - total_gap).max(0.0);
    let flex_h = if flexible_count > 0 {
        remaining_h / flexible_count as f32
    } else {
        0.0
    };

    // Render children
    let mut y = inner_y;
    for child in children {
        let child_style = get_child_style(child);
        let child_h = if let Some(h) = child_style.height {
            h * ctx.zoom
        } else {
            flex_h
        };
        let child_rect = Rect::from_min_size(
            Pos2::new(inner_x, y),
            vec2(inner_w, child_h),
        );
        let used = layout_and_render(ctx, child, &child_style, child_rect);
        y += used.max(child_h) + gap * ctx.zoom;
    }

    (y - inner_y).max(pad[0] + pad[2])
}

// ──────────────────────────────────────────────
// ROW
// ──────────────────────────────────────────────

fn render_row(
    ctx: &mut RenderContext,
    children: &[Widget],
    gap: f32,
    padding: &Option<[f32; 4]>,
    style: &Style,
    rect: Rect,
) -> f32 {
    let pad = padding.unwrap_or([0.0; 4]);
    let inner_x = rect.min.x + pad[3];
    let inner_y = rect.min.y + pad[0];
    let inner_w = (rect.width() - pad[1] - pad[3]).max(0.0);
    let inner_h = (rect.height() - pad[0] - pad[2]).max(0.0);

    draw_bg(ctx, style, rect);

    // Compute widths
    let total_gap = gap * (children.len().saturating_sub(1) as f32) * ctx.zoom;
    let available_w = inner_w - total_gap;

    // Count fixed-width vs flexible children
    let mut fixed_w_sum = 0.0;
    let mut flex_count = 0;
    let child_styles: Vec<Style> = children.iter().map(get_child_style).collect();

    for (_child, child_style) in children.iter().zip(child_styles.iter()) {
        match &child_style.width {
            Some(Length::Px(w)) => fixed_w_sum += w * ctx.zoom,
            Some(Length::Percent(p)) => fixed_w_sum += inner_w * p / 100.0,
            _ => flex_count += 1,
        }
    }

    let flex_w = if flex_count > 0 {
        (available_w - fixed_w_sum).max(0.0) / flex_count as f32
    } else {
        0.0
    };

    // Render
    let row_height = inner_h.max(0.0);
    let mut x = inner_x;
    for (child, child_style) in children.iter().zip(child_styles.iter()) {
        let child_w = match &child_style.width {
            Some(Length::Px(w)) => w * ctx.zoom,
            Some(Length::Percent(p)) => inner_w * p / 100.0,
            _ => flex_w,
        };
        let child_rect = Rect::from_min_size(
            Pos2::new(x, inner_y),
            vec2(child_w, row_height),
        );
        layout_and_render(ctx, child, child_style, child_rect);
        x += child_w + gap * ctx.zoom;
    }

    inner_h.max(pad[0] + pad[2])
}

// ──────────────────────────────────────────────
// TEXT
// ──────────────────────────────────────────────

fn render_text(ctx: &mut RenderContext, text: &str, style: &Style, rect: Rect) -> f32 {
    let bound_text = bind_text(text, &|var| {
        if let Some(rest) = var.strip_prefix("inputs.") {
            ctx.input_values.get(rest).cloned()
        } else if let Some(rest) = var.strip_prefix("outputs.") {
            ctx.output_values.get(rest).cloned()
        } else if let Some(rest) = var.strip_prefix("state.") {
            ctx.state_values.get(rest).cloned()
        } else {
            None
        }
    });

    if bound_text.is_empty() {
        return 0.0;
    }

    let font_size = style.font_size.unwrap_or(ctx.theme.font_size()) * ctx.zoom;
    let color = style
        .color
        .as_ref()
        .and_then(|c| {
            if c.starts_with('$') {
                ctx.theme.get_color(c)
            } else {
                ColorSpec::parse(c).map(|s| s.resolve(ctx.theme, Color32::WHITE))
            }
        })
        .unwrap_or(Color32::WHITE);

    let bold = style.bold.unwrap_or(false);
    let font_id = if bold {
        FontId::proportional(font_size.max(8.0))
    } else {
        FontId::proportional(font_size.max(8.0))
    };

    let align = match style.align.as_deref() {
        Some("center") => Align2::CENTER_TOP,
        Some("right") => Align2::RIGHT_TOP,
        _ => Align2::LEFT_TOP,
    };

    let galley = ctx.ui.painter().layout_no_wrap(bound_text.clone(), font_id.clone(), color);
    let text_pos = match align {
        Align2::LEFT_TOP => rect.left_top(),
        Align2::CENTER_TOP => Pos2::new(rect.center().x - galley.size().x / 2.0, rect.top()),
        Align2::RIGHT_TOP => Pos2::new(rect.right() - galley.size().x, rect.top()),
        _ => rect.left_top(),
    };
    let text_rect = Rect::from_min_size(text_pos, galley.size());

    ctx.ui.painter().text(
        text_rect.left_top(),
        align,
        bound_text,
        font_id,
        color,
    );

    text_rect.height()
}

// ──────────────────────────────────────────────
// BUTTON
// ──────────────────────────────────────────────

fn render_button(
    ctx: &mut RenderContext,
    text: &str,
    event: &str,
    style: &Style,
    rect: Rect,
) -> f32 {
    let bound_text = bind_text(text, &|var| {
        if let Some(rest) = var.strip_prefix("inputs.") {
            ctx.input_values.get(rest).cloned()
        } else {
            None
        }
    });

    let font_size = style.font_size.unwrap_or(ctx.theme.font_size()) * ctx.zoom;
    let bg = style
        .bg
        .as_ref()
        .map(|c| ctx.resolve_color(c, Color32::from_rgb(60, 80, 120)))
        .unwrap_or_else(|| Color32::from_rgb(60, 80, 120));
    let text_color = style
        .color
        .as_ref()
        .map(|c| ctx.resolve_color(c, Color32::WHITE))
        .unwrap_or(Color32::BLACK); // egui Button defaults to black text

    let btn_h = (28.0 * ctx.zoom).max(22.0);
    let btn_rect = Rect::from_min_size(
        Pos2::new(rect.left(), rect.top() + (rect.height() - btn_h) / 2.0),
        vec2(rect.width(), btn_h),
    );

    // Use a real egui Button widget — handles hover/press/click correctly
    let egui_btn = egui::Button::new(
        egui::RichText::new(bound_text)
            .color(text_color)
            .size(font_size.max(9.0))
    )
    .fill(bg)
    .min_size(vec2(btn_rect.width(), btn_h));

    // Apply border via stroke
    let egui_btn = if let Some(border_str) = &style.border {
        let parts: Vec<&str> = border_str.splitn(2, ',').collect();
        if parts.len() == 2 {
            let bw = parts[0].trim().parse::<f32>().unwrap_or(1.0);
            let bc = ctx.resolve_color(parts[1].trim(), Color32::WHITE);
            egui_btn.stroke(Stroke::new(bw * ctx.zoom, bc))
        } else {
            egui_btn
        }
    } else {
        egui_btn
    };

    // Apply rounding
    let radius = style.radius.unwrap_or(ctx.theme.radius()) * ctx.zoom;
    let egui_btn = egui_btn.corner_radius(radius as f32);

    let response = ctx.ui.put(btn_rect, egui_btn);

    if response.clicked() {
        ctx.events.push(ScriptEvent::ButtonClick {
            event_key: event.to_owned(),
        });
    }

    btn_h
}

// ──────────────────────────────────────────────
// SLIDER
// ──────────────────────────────────────────────

fn render_slider(
    ctx: &mut RenderContext,
    name: &str,
    label: &str,
    min: f64,
    max: f64,
    default: f64,
    style: &Style,
    rect: Rect,
) -> f32 {
    let font_size = style.font_size.unwrap_or(ctx.theme.font_size()) * ctx.zoom;
    let label_color = style
        .color
        .as_ref()
        .map(|c| ctx.resolve_color(c, Color32::WHITE))
        .unwrap_or(Color32::WHITE);

    // Current value: from output if already set, else default, else min
    let current_val = ctx
        .output_values
        .get(name)
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(default);

    let slider_h = 24.0 * ctx.zoom;
    let label_h = if label.is_empty() { 0.0 } else { font_size + 4.0 };
    let total_h = label_h + 4.0 * ctx.zoom + slider_h;

    // Draw label
    if !label.is_empty() {
        let label_rect = Rect::from_min_size(rect.left_top(), vec2(rect.width(), label_h));
        ctx.ui.painter().text(
            label_rect.left_top(),
            Align2::LEFT_TOP,
            format!("{}: {:.1}", label, current_val),
            FontId::proportional(font_size.max(8.0)),
            label_color,
        );
    }

    // Use egui Slider widget for reliable interaction
    let slider_rect = Rect::from_min_size(
        Pos2::new(rect.left(), rect.top() + label_h + 4.0 * ctx.zoom),
        vec2(rect.width(), slider_h),
    );

    let accent_color = ctx.resolve_color(&ctx.theme.accent, Color32::from_rgb(79, 195, 247));
    let mut val_f32 = current_val as f32;
    let response = ctx.ui.scope(|ui| {
        ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::from_rgb(40, 44, 60);
        ui.style_mut().visuals.widgets.active.bg_fill = Color32::from_rgb(50, 55, 75);
        ui.style_mut().visuals.widgets.hovered.bg_fill = Color32::from_rgb(45, 49, 67);
        ui.style_mut().visuals.widgets.inactive.fg_stroke.color = accent_color;
        ui.style_mut().visuals.widgets.active.fg_stroke.color = accent_color;
        ui.put(
            slider_rect,
            egui::Slider::new(&mut val_f32, (min as f32)..=(max as f32))
                .show_value(false)
                .trailing_fill(true),
        )
    }).inner;

    if response.changed() {
        let new_val = val_f32 as f64;
        ctx.events.push(ScriptEvent::SliderChange {
            name: name.to_owned(),
            value: new_val,
        });
    }

    total_h
}

// ──────────────────────────────────────────────
// INPUT
// ──────────────────────────────────────────────

fn render_input(
    ctx: &mut RenderContext,
    name: &str,
    label: &str,
    placeholder: &str,
    style: &Style,
    rect: Rect,
) -> f32 {
    let font_size = style.font_size.unwrap_or(ctx.theme.font_size()) * ctx.zoom;
    let label_color = style
        .color
        .as_ref()
        .map(|c| ctx.resolve_color(c, Color32::WHITE))
        .unwrap_or(Color32::WHITE);

    let current_val = ctx
        .output_values
        .get(name)
        .cloned()
        .unwrap_or_default();

    let input_h = 28.0 * ctx.zoom;
    let label_h = if label.is_empty() { 0.0 } else { font_size + 4.0 };
    let total_h = label_h + 4.0 * ctx.zoom + input_h;

    // Label
    if !label.is_empty() {
        let label_rect = Rect::from_min_size(rect.left_top(), vec2(rect.width(), label_h));
        ctx.ui.painter().text(
            label_rect.left_top(),
            Align2::LEFT_TOP,
            label,
            FontId::proportional(font_size.max(8.0)),
            label_color,
        );
    }

    // Input area
    let input_rect = Rect::from_min_size(
        Pos2::new(rect.left(), rect.top() + label_h + 4.0 * ctx.zoom),
        vec2(rect.width(), input_h),
    );

    let bg_color = Color32::from_rgb(30, 34, 50);
    ctx.ui.painter().rect_filled(input_rect, 6.0 * ctx.zoom, bg_color);
    ctx.ui.painter().rect_stroke(
        input_rect,
        6.0 * ctx.zoom,
        Stroke::new(1.0, Color32::from_rgb(80, 90, 120)),
        egui::StrokeKind::Outside,
    );

    // Text display (editable via egui TextEdit)
    let input_id = ctx.next_id();
    let mut buffer = current_val.clone();
    let text_response = ctx.ui.put(
        input_rect,
        egui::TextEdit::singleline(&mut buffer)
            .id(input_id)
            .font(FontId::proportional(font_size.max(8.0)))
            .text_color(Color32::WHITE)
            .background_color(Color32::from_rgb(30, 34, 50))
            .desired_width(f32::INFINITY)
            .hint_text(placeholder),
    );

    if text_response.changed() && buffer != current_val {
        ctx.events.push(ScriptEvent::InputChange {
            name: name.to_owned(),
            value: buffer,
        });
    }

    total_h
}

// ──────────────────────────────────────────────
// BAR (progress bar)
// ──────────────────────────────────────────────

fn render_bar(
    ctx: &mut RenderContext,
    value: &str,
    max: f64,
    height: f32,
    track: &str,
    fill: &str,
    _style: &Style,
    rect: Rect,
) -> f32 {
    let bound_val = bind_text(value, &|var| {
        if let Some(rest) = var.strip_prefix("inputs.") {
            ctx.input_values.get(rest).cloned()
        } else if let Some(rest) = var.strip_prefix("outputs.") {
            ctx.output_values.get(rest).cloned()
        } else {
            None
        }
    });

    let val = bound_val.parse::<f64>().unwrap_or(0.0);
    let t = (val / max.max(0.001)).clamp(0.0, 1.0) as f32;

    let bar_h = height * ctx.zoom;
    let bar_w = rect.width();

    let track_color = ctx.resolve_color(track, Color32::from_rgb(50, 50, 70));
    let fill_color_str = if fill.starts_with("linear(") {
        fill
    } else {
        fill
    };
    let fill_color = ctx.resolve_color(fill_color_str, Color32::from_rgb(79, 195, 247));

    let bar_rect = Rect::from_min_size(
        Pos2::new(rect.left(), rect.top() + (rect.height() - bar_h) / 2.0),
        vec2(bar_w, bar_h),
    );

    // Track
    ctx.ui.painter().rect_filled(bar_rect, bar_h / 2.0, track_color);

    // Fill
    let fill_w = (bar_rect.width() * t).max(0.0);
    if fill_w > 0.0 {
        let fill_rect = Rect::from_min_size(bar_rect.left_top(), vec2(fill_w, bar_h));
        // Simple gradient: use fill_color directly (parsed or default)
        ctx.ui.painter().rect_filled(fill_rect, bar_h / 2.0, fill_color);
    }

    bar_h
}

// ──────────────────────────────────────────────
// DIVIDER
// ──────────────────────────────────────────────

fn render_divider(ctx: &mut RenderContext, style: &Style, rect: Rect) -> f32 {
    let color = style
        .color
        .as_ref()
        .map(|c| ctx.resolve_color(c, Color32::from_rgb(80, 80, 100)))
        .unwrap_or_else(|| Color32::from_rgb(80, 80, 100));

    let y = rect.top() + rect.height() / 2.0;
    ctx.ui.painter().line_segment(
        [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
        Stroke::new(1.0, color),
    );

    rect.height().max(2.0)
}

// ──────────────────────────────────────────────
// BADGE
// ──────────────────────────────────────────────

fn render_badge(
    ctx: &mut RenderContext,
    text: &str,
    color: &str,
    style: &Style,
    rect: Rect,
) -> f32 {
    let font_size = (style.font_size.unwrap_or(ctx.theme.font_size()) * 0.85) * ctx.zoom;
    let bg = ctx.resolve_color(color, Color32::from_rgb(79, 195, 247));
    let text_color = Color32::BLACK;

    let galley = ctx.ui.painter().layout_no_wrap(
        text.to_owned(),
        FontId::proportional(font_size.max(8.0)),
        text_color,
    );
    let pad_x = 6.0 * ctx.zoom;
    let pad_y = 2.0 * ctx.zoom;
    let badge_w = galley.size().x + pad_x * 2.0;
    let badge_h = galley.size().y + pad_y * 2.0;

    let badge_rect = Rect::from_min_size(
        Pos2::new(rect.left(), rect.top() + (rect.height() - badge_h) / 2.0),
        vec2(badge_w.min(rect.width()), badge_h.min(rect.height())),
    );

    ctx.ui.painter().rect_filled(badge_rect, 4.0 * ctx.zoom, bg);
    ctx.ui.painter().text(
        Pos2::new(badge_rect.left() + pad_x, badge_rect.top() + pad_y),
        Align2::LEFT_TOP,
        text,
        FontId::proportional(font_size.max(8.0)),
        text_color,
    );

    badge_h
}

// ──────────────────────────────────────────────
// HELPERS
// ──────────────────────────────────────────────

fn get_child_style(widget: &Widget) -> Style {
    match widget {
        Widget::Col { style, .. } => style.clone(),
        Widget::Row { style, .. } => style.clone(),
        Widget::Text { style, .. } => style.clone(),
        Widget::Button { style, .. } => style.clone(),
        Widget::Slider { style, .. } => style.clone(),
        Widget::Input { style, .. } => style.clone(),
        Widget::Bar { style, .. } => style.clone(),
        Widget::Spacer { .. } => Style::default(),
        Widget::Divider { style } => style.clone(),
        Widget::Badge { style, .. } => style.clone(),
        Widget::Image { style, .. } => style.clone(),
    }
}

fn draw_bg(ctx: &mut RenderContext, style: &Style, rect: Rect) {
    if let Some(bg_str) = &style.bg {
        let color = ctx.resolve_color(bg_str, Color32::TRANSPARENT);
        if color != Color32::TRANSPARENT {
            let radius = style.radius.unwrap_or(0.0) * ctx.zoom;
            if radius > 0.0 {
                ctx.ui.painter().rect_filled(rect, radius, color);
            } else {
                ctx.ui.painter().rect_filled(rect, 0.0, color);
            }
        }
    }
}

#[allow(dead_code)]
fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        (a.r() as f32 * (1.0 - t) + b.r() as f32 * t) as u8,
        (a.g() as f32 * (1.0 - t) + b.g() as f32 * t) as u8,
        (a.b() as f32 * (1.0 - t) + b.b() as f32 * t) as u8,
        (a.a() as f32 * (1.0 - t) + b.a() as f32 * t) as u8,
    )
}

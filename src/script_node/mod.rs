pub mod types;
pub mod parser;
pub mod layout_render;
pub mod lua;

use eframe::egui::{self, Color32, Layout, Align};
use std::collections::HashMap;
use types::*;

/// Run a complete render cycle for a script node.
///
/// Creates a child UI within content_rect so egui's interaction system works correctly.
/// Returns the total height used and any events fired.
pub fn render_script_node(
    spec: &ScriptNodeSpec,
    content_rect: egui::Rect,
    ui: &mut egui::Ui,
    zoom: f32,
    input_values: &HashMap<String, String>,
    output_values: &mut HashMap<String, String>,
    state_values: &HashMap<String, String>,
    events: &mut Vec<ScriptEvent>,
    id_counter: &mut u64,
) -> f32 {
    let theme = spec.effective_theme();

    // Create a child UI positioned at the content rect.
    // This is critical: egui interactions only work inside a Ui's allocated area.
    let inner_padding = 0.0;
    let inner_rect = egui::Rect::from_min_max(
        content_rect.min + egui::vec2(inner_padding, inner_padding),
        content_rect.max - egui::vec2(inner_padding, inner_padding),
    );

    if !inner_rect.is_positive() {
        return 0.0;
    }

    // Draw node body background
    let body_style = spec.style.clone().unwrap_or(Style {
        bg: Some(theme.surface.clone()),
        radius: Some(theme.radius),
        ..Style::default()
    });
    if let Some(bg_str) = &body_style.bg {
        let color = ColorSpec::parse(bg_str)
            .map(|c| c.resolve(&theme, Color32::from_rgb(22, 33, 62)))
            .unwrap_or_else(|| Color32::from_rgb(22, 33, 62));
        // Only round bottom corners — top is flush against the header divider
        let bottom_r = (theme.radius * zoom).round() as u8;
        let rounding = egui::CornerRadius {
            nw: 0,
            ne: 0,
            sw: bottom_r,
            se: bottom_r,
        };
        ui.painter().rect_filled(content_rect, rounding, color);
    }

    // Allocate a child UI for the interactive area
    let mut child_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner_rect)
            .layout(Layout::top_down(Align::Min)),
    );
    child_ui.set_clip_rect(inner_rect);

    // Consume the space so egui knows the area is occupied
    let _ = child_ui.allocate_space(inner_rect.size());

    let mut ctx = layout_render::RenderContext {
        theme: &theme,
        ui: &mut child_ui,
        zoom,
        input_values,
        output_values,
        state_values,
        events,
        id_counter,
    };

    layout_render::layout_and_render(&mut ctx, &spec.body, &Style::default(), inner_rect)
}

/// Process script events and update output/state accordingly.
pub fn process_script_events(
    events: &[ScriptEvent],
    _input_values: &HashMap<String, String>,
    output_values: &mut HashMap<String, String>,
    _state_values: &mut HashMap<String, String>,
) -> Vec<(String, String)> {
    let mut outputs = Vec::new();

    for event in events {
        match event {
            ScriptEvent::ButtonClick { .. } => {
                // Button is now queue-driven: forwarding is handled by consume_script_queue
                // in canvas_nodes_render.rs. Do not emit any output changes here.
                // This prevents duplicate forwarding when queue is consumed.
            }
            ScriptEvent::SliderChange { name, value } => {
                let val_str = format!("{:.2}", value);
                outputs.push((name.clone(), val_str.clone()));
                output_values.insert(name.clone(), val_str);
            }
            ScriptEvent::InputChange { name, value } => {
                outputs.push((name.clone(), value.clone()));
                output_values.insert(name.clone(), value.clone());
            }
        }
    }

    outputs
}

/// Default JSON template for a new script node.
/// Now includes message queue support (like Decision node).
pub fn default_script_template() -> String {
    [
        "{  ",
        "  \"theme\": {",
        "    \"bg\": \"#1a1a2e\",",
        "    \"surface\": \"#16213e\",",
        "    \"accent\": \"#7ec8e3\",",
        "    \"danger\": \"#ef5350\",",
        "    \"success\": \"#66bb6a\",",
        "    \"text\": \"#e8e6f0\",",
        "    \"text_secondary\": \"#9a98b0\",",
        "    \"radius\": 8,",
        "    \"font_size\": 14",
        "  },",
        "  \"ports\": {",
        "    \"inputs\": {",
        "      \"input\": { \"type\": \"string\", \"description\": \"来自上游的消息\" }",
        "    },",
        "    \"outputs\": {",
        "      \"approve\": { \"type\": \"string\", \"description\": \"批准后转发的消息\" },",
        "      \"reject\": { \"type\": \"string\", \"description\": \"驳回后转发的消息\" }",
        "    }",
        "  },",
        "  \"body\": {",
        "    \"type\": \"col\",",
        "    \"gap\": 8,",
        "    \"children\": [",
        "      {",
        "        \"type\": \"text\",",
        "        \"text\": \"待处理: {state.queue_len} 条\",",
        "        \"style\": { \"font_size\": 18, \"bold\": true, \"color\": \"$accent\" }",
        "      },",
        "      {",
        "        \"type\": \"text\",",
        "        \"text\": \"最新: {state.queue_first}\",",
        "        \"style\": { \"font_size\": 13, \"color\": \"$text_secondary\" }",
        "      },",
        "      {",
        "        \"type\": \"divider\"",
        "      },",
        "      {",
        "        \"type\": \"row\",",
        "        \"gap\": 8,",
        "        \"children\": [",
        "          {",
        "            \"type\": \"button\",",
        "            \"text\": \"✓ 批准\",",
        "            \"event\": \"approve\",",
        "            \"enabled\": \"{state.queue_len}\",",
        "            \"style\": { \"bg\": \"$success\", \"color\": \"#ffffff\", \"bold\": true }",
        "          },",
        "          {",
        "            \"type\": \"button\",",
        "            \"text\": \"✕ 驳回\",",
        "            \"event\": \"reject\",",
        "            \"enabled\": \"{state.queue_len}\",",
        "            \"style\": { \"bg\": \"$danger\", \"color\": \"#ffffff\", \"bold\": true }",
        "          }",
        "        ]",
        "      }",
        "    ]",
        "  }",
        "}",
    ].join("\n")
}

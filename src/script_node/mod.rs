pub mod types;
pub mod parser;
pub mod layout_render;

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
    let inner_padding = 8.0 * zoom;
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
        let radius = body_style.radius.unwrap_or(theme.radius) * zoom;
        if radius > 0.0 {
            ui.painter().rect_filled(content_rect, radius, color);
        } else {
            ui.painter().rect_filled(content_rect, 0.0, color);
        }
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
    output_values: &mut HashMap<String, String>,
    _state_values: &mut HashMap<String, String>,
) -> Vec<(String, String)> {
    let mut outputs = Vec::new();

    for event in events {
        match event {
            ScriptEvent::ButtonClick { event_key } => {
                outputs.push(("event".to_owned(), event_key.clone()));
                output_values.insert("event".to_owned(), event_key.clone());
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
pub fn default_script_template() -> String {
    [
        "{  ",
        "  \"theme\": {",
        "    \"bg\": \"#1a1a2e\",",
        "    \"surface\": \"#16213e\",",
        "    \"accent\": \"#4fc3f7\",",
        "    \"danger\": \"#ff6b6b\",",
        "    \"success\": \"#66bb6a\",",
        "    \"text\": \"#e0e0e0\",",
        "    \"text_secondary\": \"#a0a0b0\",",
        "    \"radius\": 10,",
        "    \"font_size\": 14",
        "  },",
        "  \"ports\": {",
        "    \"inputs\": {},",
        "    \"outputs\": {}",
        "  },",
        "  \"body\": {",
        "    \"type\": \"col\",",
        "    \"gap\": 8,",
        "    \"children\": [",
        "      {",
        "        \"type\": \"text\",",
        "        \"text\": \"Hello, Script Node!\",",
        "        \"style\": { \"font_size\": 18, \"bold\": true, \"color\": \"$accent\" }",
        "      },",
        "      {",
        "        \"type\": \"button\",",
        "        \"text\": \"点我\",",
        "        \"event\": \"clicked\"",
        "      }",
        "    ]",
        "  }",
        "}",
    ].join("\n")
}

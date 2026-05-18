use crate::script_node::types::*;
use serde_json;

/// Parse a JSON string into a ScriptNodeSpec.
/// Returns Ok(spec) on success, Err(message) on failure.
pub fn parse_script_spec(json_str: &str) -> Result<ScriptNodeSpec, String> {
    let trimmed = json_str.trim();
    if trimmed.is_empty() {
        return Err("JSON spec is empty".to_owned());
    }

    let spec: ScriptNodeSpec =
        serde_json::from_str(trimmed).map_err(|e| format!("JSON parse error: {e}"))?;

    // Validate the widget tree (basic sanity checks)
    validate_widget(&spec.body)?;

    Ok(spec)
}

fn validate_widget(widget: &Widget) -> Result<(), String> {
    match widget {
        Widget::Col { children, .. } | Widget::Row { children, .. } => {
            if children.is_empty() {
                return Err("col/row must have at least one child".to_owned());
            }
            for child in children {
                validate_widget(child)?;
            }
        }
        Widget::Button { text, event, .. } => {
            if text.trim().is_empty() {
                return Err("button text cannot be empty".to_owned());
            }
            if event.trim().is_empty() {
                return Err("button event cannot be empty".to_owned());
            }
        }
        Widget::Slider { name, min, max, .. } => {
            if name.trim().is_empty() {
                return Err("slider name cannot be empty".to_owned());
            }
            if min >= max {
                return Err(format!("slider '{name}': min ({min}) must be < max ({max})"));
            }
        }
        Widget::Input { name, .. } => {
            if name.trim().is_empty() {
                return Err("input name cannot be empty".to_owned());
            }
        }
        Widget::Text { .. }
        | Widget::Bar { .. }
        | Widget::Spacer { .. }
        | Widget::Divider { .. }
        | Widget::Badge { .. }
        | Widget::Image { .. } => {}
    }
    Ok(())
}

/// Extract all variable references from a text string.
/// Variables look like {inputs.name} or {state.key} or {outputs.key}.
#[allow(dead_code)]
pub fn extract_variables(text: &str) -> Vec<String> {
    let mut vars = Vec::new();
    let mut remaining = text;
    while let Some(start) = remaining.find('{') {
        let after = &remaining[start + 1..];
        if let Some(end) = after.find('}') {
            vars.push(after[..end].to_owned());
            remaining = &after[end + 1..];
        } else {
            break;
        }
    }
    vars
}

/// Bind variables in a text string using provided value lookup.
/// The lookup closure receives the variable name (e.g. "inputs.temp") and returns an optional string.
pub fn bind_text<F>(text: &str, lookup: &F) -> String
where
    F: Fn(&str) -> Option<String>,
{
    let mut result = String::with_capacity(text.len());
    let mut remaining = text;
    while let Some(start) = remaining.find('{') {
        // Push everything before the brace
        result.push_str(&remaining[..start]);
        let after = &remaining[start + 1..];
        if let Some(end) = after.find('}') {
            let var_name = &after[..end];
            match lookup(var_name) {
                Some(val) => result.push_str(&val),
                None => {
                    // Keep the original placeholder for missing vars
                    result.push('{');
                    result.push_str(var_name);
                    result.push('}');
                }
            }
            remaining = &after[end + 1..];
        } else {
            // No closing brace, push everything
            result.push_str(remaining);
            break;
        }
    }
    result.push_str(remaining);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_text_widget() {
        let json = r#"{
            "body": {
                "type": "text",
                "text": "Hello, World!",
                "style": { "color": "red", "bold": true }
            }
        }"#;
        let spec = parse_script_spec(json).unwrap();
        match spec.body {
            Widget::Text { text, .. } => assert_eq!(text, "Hello, World!"),
            _ => panic!("expected text widget"),
        }
    }

    #[test]
    fn test_col_with_children() {
        let json = r#"{
            "body": {
                "type": "col",
                "gap": 8,
                "padding": [12, 12, 12, 12],
                "children": [
                    { "type": "text", "text": "Title", "style": { "font_size": 20 } },
                    { "type": "button", "text": "Click", "event": "submit" }
                ]
            }
        }"#;
        let spec = parse_script_spec(json).unwrap();
        match &spec.body {
            Widget::Col { children, gap, padding, .. } => {
                assert_eq!(*gap, 8.0);
                assert_eq!(*padding, Some([12.0, 12.0, 12.0, 12.0]));
                assert_eq!(children.len(), 2);
            }
            _ => panic!("expected col widget"),
        }
    }

    #[test]
    fn test_port_definitions() {
        let json = r#"{
            "ports": {
                "inputs": {
                    "temperature": { "type": "number", "default": 0, "description": "Sensor value" },
                    "name": { "type": "string", "default": "unknown" }
                },
                "outputs": {
                    "alert": { "type": "string" }
                }
            },
            "body": { "type": "text", "text": "test" }
        }"#;
        let spec = parse_script_spec(json).unwrap();
        let inputs = spec.input_ports();
        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].0, "name");
        assert_eq!(inputs[1].0, "temperature");
        let outputs = spec.output_ports();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].0, "alert");
    }

    #[test]
    fn test_variable_extraction() {
        let vars = extract_variables("Temp: {inputs.temp}°C, State: {state.mode}");
        assert_eq!(vars, vec!["inputs.temp", "state.mode"]);
    }

    #[test]
    fn test_text_binding() {
        let result = bind_text("Temp: {inputs.t}°C", &|var| {
            match var {
                "inputs.t" => Some("42.5".to_owned()),
                _ => None,
            }
        });
        assert_eq!(result, "Temp: 42.5°C");
    }

    #[test]
    fn test_theme_parsing() {
        let json = r#"{
            "theme": {
                "bg": "darkbg",
                "accent": "blue",
                "radius": 8
            },
            "body": { "type": "text", "text": "hi" }
        }"#;
        let spec = parse_script_spec(json).unwrap();
        let theme = spec.effective_theme();
        assert_eq!(theme.bg, "darkbg");         // custom
        assert_eq!(theme.text, "#e0e0e0");       // default
        assert_eq!(theme.radius, 8.0);            // custom
    }

    #[test]
    fn test_slider_widget() {
        let json = r#"{
            "body": {
                "type": "slider",
                "name": "volume",
                "label": "音量",
                "min": 0,
                "max": 100,
                "default": 50
            }
        }"#;
        let spec = parse_script_spec(json).unwrap();
        match &spec.body {
            Widget::Slider { name, min, max, default, .. } => {
                assert_eq!(name, "volume");
                assert_eq!(*min, 0.0);
                assert_eq!(*max, 100.0);
                assert_eq!(*default, 50.0);
            }
            _ => panic!("expected slider"),
        }
    }

    #[test]
    fn test_complex_ui_example() {
        // Build JSON without raw strings to avoid `"#` issues
        let json = format!(
            r##"{{
                "theme": {{
                    "bg": "{}",
                    "surface": "{}",
                    "accent": "{}",
                    "danger": "{}",
                    "radius": 12,
                    "font_size": 14
                }},
                "ports": {{
                    "inputs": {{
                        "temperature": {{ "type": "number", "default": 0 }},
                        "mode": {{ "type": "string", "default": "auto" }}
                    }},
                    "outputs": {{
                        "alert": {{ "type": "string" }},
                        "threshold": {{ "type": "number" }}
                    }}
                }},
                "body": {{
                    "type": "col",
                    "gap": 12,
                    "padding": [16, 16, 16, 16],
                    "children": [
                        {{ "type": "text", "text": "temp monitor", "style": {{ "font_size": 20, "bold": true, "color": "$accent" }} }},
                        {{
                            "type": "row",
                            "gap": 8,
                            "children": [
                                {{ "type": "text", "text": "current:" }},
                                {{ "type": "text", "text": "{{inputs.temperature}}C", "style": {{ "font_size": 28, "bold": true, "color": "$danger" }} }}
                            ]
                        }},
                        {{
                            "type": "bar",
                            "value": "{{inputs.temperature}}",
                            "max": 100,
                            "height": 10,
                            "track": "gray",
                            "fill": "$accent"
                        }},
                        {{
                            "type": "slider",
                            "name": "threshold",
                            "label": "threshold",
                            "min": 0,
                            "max": 100,
                            "default": 70
                        }}
                    ]
                }}
            }}"##,
            "darkbg", "darksurface", "accentblue", "dangerred"
        );
        let spec = parse_script_spec(&json).unwrap();
        assert!(spec.theme.is_some());
        assert!(spec.ports.is_some());
        assert_eq!(spec.input_ports().len(), 2);
        assert_eq!(spec.output_ports().len(), 2);
        match &spec.body {
            Widget::Col { children, .. } => assert_eq!(children.len(), 4),
            _ => panic!("expected col"),
        }
    }
}

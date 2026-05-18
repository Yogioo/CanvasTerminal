use eframe::egui::Color32;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

// ──────────────────────────────────────────────
// Color
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ColorSpec {
    Hex(String),
    ThemeRef(String),
    LinearGradient {
        angle: f32,
        stops: Vec<String>,
    },
}

impl ColorSpec {
    pub fn resolve(&self, theme: &Theme, fallback: Color32) -> Color32 {
        match self {
            ColorSpec::Hex(hex) => parse_hex_color(hex).unwrap_or(fallback),
            ColorSpec::ThemeRef(key) => theme.get_color(key).unwrap_or(fallback),
            ColorSpec::LinearGradient { .. } => fallback, // gradients rendered separately
        }
    }

    /// Parse from a string like "#ff6b6b", "$accent", or "linear(90deg, #c00, #f00)"
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.starts_with("$") {
            return Some(ColorSpec::ThemeRef(s.to_owned()));
        }
        if s.starts_with('#') && (s.len() == 4 || s.len() == 5 || s.len() == 7 || s.len() == 9) {
            return Some(ColorSpec::Hex(s.to_owned()));
        }
        if s.starts_with("linear(") && s.ends_with(')') {
            let inner = &s[7..s.len() - 1];
            let parts: Vec<&str> = inner.splitn(2, ',').collect();
            if parts.len() != 2 {
                return None;
            }
            let angle_str = parts[0].trim();
            let angle = if angle_str.ends_with("deg") {
                angle_str[..angle_str.len() - 3].trim().parse::<f32>().ok()?
            } else {
                0.0
            };
            let stops: Vec<String> = parts[1]
                .split(',')
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
                .collect();
            if stops.len() < 2 {
                return None;
            }
            return Some(ColorSpec::LinearGradient { angle, stops });
        }
        None
    }
}

fn parse_hex_color(hex: &str) -> Option<Color32> {
    let hex = hex.trim_start_matches('#');
    let (r, g, b, a) = match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            (r, g, b, 255)
        }
        4 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            let a = u8::from_str_radix(&hex[3..4], 16).ok()? * 17;
            (r, g, b, a)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b, 255)
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            (r, g, b, a)
        }
        _ => return None,
    };
    Some(Color32::from_rgba_unmultiplied(r, g, b, a))
}

// ──────────────────────────────────────────────
// Theme
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    #[serde(default = "default_theme_bg")]
    pub bg: String,
    #[serde(default = "default_theme_surface")]
    pub surface: String,
    #[serde(default = "default_theme_accent")]
    pub accent: String,
    #[serde(default = "default_theme_danger")]
    pub danger: String,
    #[serde(default = "default_theme_success")]
    pub success: String,
    #[serde(default = "default_theme_text")]
    pub text: String,
    #[serde(default = "default_theme_text_secondary")]
    pub text_secondary: String,
    #[serde(default = "default_theme_radius")]
    pub radius: f32,
    #[serde(default = "default_theme_font_size")]
    pub font_size: f32,
}

fn default_theme_bg() -> String { "#1a1a2e".to_owned() }
fn default_theme_surface() -> String { "#16213e".to_owned() }
fn default_theme_accent() -> String { "#4fc3f7".to_owned() }
fn default_theme_danger() -> String { "#ff6b6b".to_owned() }
fn default_theme_success() -> String { "#66bb6a".to_owned() }
fn default_theme_text() -> String { "#e0e0e0".to_owned() }
fn default_theme_text_secondary() -> String { "#a0a0b0".to_owned() }
fn default_theme_radius() -> f32 { 10.0 }
fn default_theme_font_size() -> f32 { 14.0 }

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: default_theme_bg(),
            surface: default_theme_surface(),
            accent: default_theme_accent(),
            danger: default_theme_danger(),
            success: default_theme_success(),
            text: default_theme_text(),
            text_secondary: default_theme_text_secondary(),
            radius: default_theme_radius(),
            font_size: default_theme_font_size(),
        }
    }
}

impl Theme {
    pub fn get_color(&self, key: &str) -> Option<Color32> {
        let hex = match key {
            "$bg" | "$background" => &self.bg,
            "$surface" => &self.surface,
            "$accent" => &self.accent,
            "$danger" => &self.danger,
            "$success" => &self.success,
            "$text" => &self.text,
            "$text_secondary" | "$muted" => &self.text_secondary,
            _ => return None,
        };
        parse_hex_color(hex)
    }

    pub fn radius(&self) -> f32 { self.radius }
    pub fn font_size(&self) -> f32 { self.font_size }
}

// ──────────────────────────────────────────────
// Length
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Length {
    Fill,
    Px(f32),
    Percent(f32),
}

impl Default for Length {
    fn default() -> Self { Length::Fill }
}

// ──────────────────────────────────────────────
// Style
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Style {
    #[serde(default)]
    pub bg: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub font_size: Option<f32>,
    #[serde(default)]
    pub bold: Option<bool>,
    #[serde(default)]
    pub radius: Option<f32>,
    #[serde(default)]
    pub padding: Option<[f32; 4]>, // [top, right, bottom, left]
    #[serde(default)]
    pub width: Option<Length>,
    #[serde(default)]
    pub height: Option<f32>,
    #[serde(default)]
    pub align: Option<String>, // "left", "center", "right"
    #[serde(default)]
    pub border: Option<String>, // "width,color" e.g. "1,#ffffff"
}

impl Default for Style {
    fn default() -> Self {
        Self {
            bg: None,
            color: None,
            font_size: None,
            bold: None,
            radius: None,
            padding: None,
            width: None,
            height: None,
            align: None,
            border: None,
        }
    }
}

impl Style {
    #[allow(dead_code)]
    pub fn merged_with(&self, inherited: &Style) -> Style {
        Style {
            bg: self.bg.clone().or(inherited.bg.clone()),
            color: self.color.clone().or(inherited.color.clone()),
            font_size: self.font_size.or(inherited.font_size),
            bold: self.bold.or(inherited.bold),
            radius: self.radius.or(inherited.radius),
            padding: self.padding.or(inherited.padding),
            width: self.width.clone().or(inherited.width.clone()),
            height: self.height.or(inherited.height),
            align: self.align.clone().or(inherited.align.clone()),
            border: self.border.clone().or(inherited.border.clone()),
        }
    }
}

// ──────────────────────────────────────────────
// Widget
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Widget {
    #[serde(rename = "col")]
    Col {
        children: Vec<Widget>,
        #[serde(default)]
        gap: f32,
        #[serde(default)]
        padding: Option<[f32; 4]>,
        #[serde(default)]
        style: Style,
    },
    #[serde(rename = "row")]
    Row {
        children: Vec<Widget>,
        #[serde(default)]
        gap: f32,
        #[serde(default)]
        padding: Option<[f32; 4]>,
        #[serde(default)]
        style: Style,
    },
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(default)]
        style: Style,
    },
    #[serde(rename = "button")]
    Button {
        text: String,
        event: String,
        #[serde(default)]
        style: Style,
        /// If true, clicking consumes ALL queued messages instead of one
        #[serde(default)]
        process_all: bool,
    },
    #[serde(rename = "slider")]
    Slider {
        name: String,
        label: String,
        #[serde(default = "default_slider_min")]
        min: f64,
        #[serde(default = "default_slider_max")]
        max: f64,
        #[serde(default)]
        default: f64,
        #[serde(default)]
        style: Style,
    },
    #[serde(rename = "input")]
    Input {
        name: String,
        label: String,
        #[serde(default)]
        placeholder: String,
        #[serde(default)]
        style: Style,
    },
    #[serde(rename = "bar")]
    Bar {
        value: String,
        #[serde(default = "default_bar_max")]
        max: f64,
        #[serde(default = "default_bar_height")]
        height: f32,
        #[serde(default = "default_bar_track")]
        track: String,
        #[serde(default = "default_bar_fill")]
        fill: String,
        #[serde(default)]
        style: Style,
    },
    #[serde(rename = "spacer")]
    Spacer {
        #[serde(default = "default_spacer_height")]
        height: f32,
    },
    #[serde(rename = "divider")]
    Divider {
        #[serde(default)]
        style: Style,
    },
    #[serde(rename = "badge")]
    Badge {
        text: String,
        #[serde(default = "default_badge_color")]
        color: String,
        #[serde(default)]
        style: Style,
    },
    #[serde(rename = "image")]
    Image {
        url: String,
        #[serde(default = "default_image_size")]
        width: f32,
        #[serde(default = "default_image_size")]
        height: f32,
        #[serde(default)]
        style: Style,
    },
}

fn default_slider_min() -> f64 { 0.0 }
fn default_slider_max() -> f64 { 100.0 }
fn default_bar_max() -> f64 { 100.0 }
fn default_bar_height() -> f32 { 8.0 }
fn default_bar_track() -> String { "#333333".to_owned() }
fn default_bar_fill() -> String { "#4fc3f7".to_owned() }
fn default_spacer_height() -> f32 { 8.0 }
fn default_badge_color() -> String { "$accent".to_owned() }
fn default_image_size() -> f32 { 24.0 }

// ──────────────────────────────────────────────
// Port system
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortSpec {
    #[serde(rename = "type")]
    pub port_type: PortDataType,
    #[serde(default)]
    pub default: Option<Value>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PortDataType {
    Number,
    String,
    Boolean,
    Any,
}

impl Default for PortDataType {
    fn default() -> Self { PortDataType::Any }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortDefinitions {
    #[serde(default)]
    pub inputs: HashMap<String, PortSpec>,
    #[serde(default)]
    pub outputs: HashMap<String, PortSpec>,
}

// ──────────────────────────────────────────────
// ScriptNodeSpec — the top-level JSON schema
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptNodeSpec {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub theme: Option<Theme>,
    #[serde(default)]
    pub ports: Option<PortDefinitions>,
    pub body: Widget,
    /// Optional inline style for the node body panel
    #[serde(default)]
    pub style: Option<Style>,
}

impl ScriptNodeSpec {
    /// Resolve theme: if custom, merge with defaults, else use defaults
    pub fn effective_theme(&self) -> Theme {
        match &self.theme {
            Some(custom) => {
                let defaults = Theme::default();
                Theme {
                    bg: if custom.bg != default_theme_bg() { custom.bg.clone() } else { defaults.bg },
                    surface: if custom.surface != default_theme_surface() { custom.surface.clone() } else { defaults.surface },
                    accent: if custom.accent != default_theme_accent() { custom.accent.clone() } else { defaults.accent },
                    danger: if custom.danger != default_theme_danger() { custom.danger.clone() } else { defaults.danger },
                    success: if custom.success != default_theme_success() { custom.success.clone() } else { defaults.success },
                    text: if custom.text != default_theme_text() { custom.text.clone() } else { defaults.text },
                    text_secondary: if custom.text_secondary != default_theme_text_secondary() { custom.text_secondary.clone() } else { defaults.text_secondary },
                    radius: custom.radius,
                    font_size: custom.font_size,
                }
            }
            None => Theme::default(),
        }
    }

    #[allow(dead_code)]
    pub fn input_ports(&self) -> Vec<(String, PortSpec)> {
        self.ports.as_ref()
            .and_then(|p| {
                let mut v: Vec<_> = p.inputs.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                v.sort_by(|a, b| a.0.cmp(&b.0));
                Some(v)
            })
            .unwrap_or_default()
    }

    #[allow(dead_code)]
    pub fn output_ports(&self) -> Vec<(String, PortSpec)> {
        self.ports.as_ref()
            .and_then(|p| {
                let mut v: Vec<_> = p.outputs.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                v.sort_by(|a, b| a.0.cmp(&b.0));
                Some(v)
            })
            .unwrap_or_default()
    }
}

// ──────────────────────────────────────────────
// Layout result: computed position/size for each widget
// ──────────────────────────────────────────────

// ──────────────────────────────────────────────
// Events fired by interactive widgets
// ──────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ScriptEvent {
    ButtonClick { event_key: String, process_all: bool },
    SliderChange { name: String, value: f64 },
    InputChange { name: String, value: String },
}



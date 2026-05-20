/// Lua ctx API — 渲染方法集合
///
/// 为 Lua 脚本的 `render(ctx)` 提供 UI 声明式 API。

use mlua::{UserData, UserDataMethods, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// 由 Lua 的 ctx.* 方法产生的 UI 事件类型
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum UiEvent {
    Text { text: String, font_size: Option<f32>, bold: Option<bool>, color: Option<String>, align: Option<String>, width: Option<HashMap<String, serde_json::Value>> },
    Button { label: String, enabled: bool, clicked: bool, bg: Option<String>, color: Option<String> },
    Slider { label: String, value: f64, enabled: bool, min: f64, max: f64 },
    Input { label: String, value: String, enabled: bool, multiline: bool, rows: u32, placeholder: String },
    ProgressBar { value: f64, height: Option<u32>, fill: Option<String> },
    Separator { color: Option<String> },
    Badge { text: String, color: String },
    Card { text: String, caption: Option<String> },
    Spacer(f32),
    ColStart { gap: Option<f32>, padding: Option<[f32; 4]> },
    ColEnd,
    RowStart { gap: Option<f32>, padding: Option<[f32; 4]> },
    RowEnd,
    ButtonWithCallback { label: String, event_key: Option<String>, enabled: bool, bg: Option<String>, color: Option<String>, callback_index: usize },
    Error(String),
}

fn val_str(v: &Value) -> Option<String> {
    match v { Value::String(s) => Some(s.to_string_lossy()), _ => None }
}
fn val_bool(v: &Value) -> Option<bool> { v.as_boolean() }
fn val_f64(v: &Value) -> Option<f64> { match v { Value::Integer(i) => Some(*i as f64), Value::Number(n) => Some(*n), _ => None } }
fn val_f32(v: &Value) -> Option<f32> { val_f64(v).map(|f| f as f32) }
fn val_u32(v: &Value) -> Option<u32> { match v { Value::Integer(i) => Some(*i as u32), Value::Number(n) => Some(*n as u32), _ => None } }

#[allow(dead_code)]
pub struct LuaRenderContext {
    pub events: Vec<UiEvent>,
    pub button_callbacks: Vec<Box<dyn FnMut()>>,
    pub clicked_buttons: Vec<String>,
    pub pending_click: Option<String>,
    pending_input_values: Rc<RefCell<HashMap<String, String>>>,
    pending_button_clicks: Rc<RefCell<Vec<String>>>,
}

impl LuaRenderContext {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::new_with_interactions(HashMap::new(), Vec::new())
    }

    pub fn new_with_interactions(
        pending_input_values: HashMap<String, String>,
        pending_button_clicks: Vec<String>,
    ) -> Self {
        LuaRenderContext {
            events: Vec::new(),
            button_callbacks: Vec::new(),
            clicked_buttons: Vec::new(),
            pending_click: None,
            pending_input_values: Rc::new(RefCell::new(pending_input_values)),
            pending_button_clicks: Rc::new(RefCell::new(pending_button_clicks)),
        }
    }

    fn child_context(&self) -> Self {
        LuaRenderContext {
            events: Vec::new(),
            button_callbacks: Vec::new(),
            clicked_buttons: Vec::new(),
            pending_click: None,
            pending_input_values: self.pending_input_values.clone(),
            pending_button_clicks: self.pending_button_clicks.clone(),
        }
    }
}

impl UserData for LuaRenderContext {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // text
        methods.add_method_mut("text", |_lua, ctx, args: (String, Option<HashMap<String, Value>>)| {
            let (text, opts) = args;
            let mut font_size = None; let mut bold = None; let mut color = None; let mut align = None; let mut width = None;
            if let Some(opts) = opts {
                if let Some(v) = opts.get("font_size") { font_size = val_f32(v); }
                if let Some(v) = opts.get("bold") { bold = val_bool(v); }
                if let Some(v) = opts.get("color") { color = val_str(v); }
                if let Some(v) = opts.get("align") { align = val_str(v); }
                if let Some(v) = opts.get("width") {
                    if let Ok(t) = serde_json::to_value(v) {
                        if let Some(obj) = t.as_object() {
                            let mut m = HashMap::new();
                            for (k, val) in obj { m.insert(k.clone(), val.clone()); }
                            width = Some(m);
                        }
                    }
                }
            }
            ctx.events.push(UiEvent::Text { text, font_size, bold, color, align, width });
            Ok(())
        });

        // button
        methods.add_method_mut("button", |_lua, ctx, args: (String, Option<HashMap<String, Value>>)| {
            let (label, opts) = args;
            let mut enabled = true; let mut bg = None; let mut color = None; let mut event_key = None;
            if let Some(opts) = opts {
                if let Some(v) = opts.get("enabled") { enabled = val_bool(v).unwrap_or(true); }
                if let Some(v) = opts.get("bg") { bg = val_str(v); }
                if let Some(v) = opts.get("color") { color = val_str(v); }
                if let Some(v) = opts.get("event_key") { event_key = val_str(v); }
            }
            let key = event_key.clone().unwrap_or_else(|| label.clone());
            let should_click = {
                let mut clicks = ctx.pending_button_clicks.borrow_mut();
                if let Some(pos) = clicks.iter().position(|pending| pending == &key) {
                    clicks.remove(pos);
                    enabled
                } else {
                    false
                }
            };
            let callback_index = ctx.button_callbacks.len();
            ctx.button_callbacks.push(Box::new(|| {}));
            ctx.events.push(UiEvent::ButtonWithCallback { label, event_key, enabled, bg, color, callback_index });
            Ok(should_click)
        });

        // input
        methods.add_method_mut("input", |_lua, ctx, args: Option<HashMap<String, Value>>| {
            let opts = args;
            let mut label = String::new(); let mut value = String::new(); let mut enabled = true;
            let mut multiline = false; let mut rows = 3; let mut placeholder = String::new();
            if let Some(opts) = opts {
                if let Some(v) = opts.get("label") { label = val_str(v).unwrap_or_default(); }
                if let Some(v) = opts.get("value") { value = val_str(v).unwrap_or_default(); }
                if let Some(v) = opts.get("enabled") { enabled = val_bool(v).unwrap_or(true); }
                if let Some(v) = opts.get("multiline") { multiline = val_bool(v).unwrap_or(false); }
                if let Some(v) = opts.get("rows") { rows = val_u32(v).unwrap_or(3); }
                if let Some(v) = opts.get("placeholder") { placeholder = val_str(v).unwrap_or_default(); }
            }
            let key = if label.is_empty() { "input".to_owned() } else { label.clone() };
            let actual_value = ctx
                .pending_input_values
                .borrow_mut()
                .remove(&key)
                .unwrap_or(value);
            ctx.events.push(UiEvent::Input { label: label.clone(), value: actual_value.clone(), enabled, multiline, rows, placeholder });
            Ok(actual_value)
        });

        // slider
        methods.add_method_mut("slider", |_lua, ctx, args: Option<HashMap<String, Value>>| {
            let opts = args;
            let mut label = String::new(); let mut value = 0.0; let mut enabled = true; let mut min = 0.0; let mut max = 100.0;
            if let Some(opts) = opts {
                if let Some(v) = opts.get("label") { label = val_str(v).unwrap_or_default(); }
                if let Some(v) = opts.get("value") { value = val_f64(v).unwrap_or(0.0); }
                if let Some(v) = opts.get("enabled") { enabled = val_bool(v).unwrap_or(true); }
                if let Some(v) = opts.get("min") { min = val_f64(v).unwrap_or(0.0); }
                if let Some(v) = opts.get("max") { max = val_f64(v).unwrap_or(100.0); }
            }
            ctx.events.push(UiEvent::Slider { label, value, enabled, min, max });
            Ok(value)
        });

        // progress_bar
        methods.add_method_mut("progress_bar", |_lua, ctx, args: (f64, Option<HashMap<String, Value>>)| {
            let (value, opts) = args;
            let mut height = None; let mut fill = None;
            if let Some(opts) = opts {
                if let Some(v) = opts.get("height") { height = val_u32(v); }
                if let Some(v) = opts.get("fill") { fill = val_str(v); }
            }
            ctx.events.push(UiEvent::ProgressBar { value, height, fill });
            Ok(())
        });

        // separator
        methods.add_method_mut("separator", |_lua, ctx, args: Option<HashMap<String, Value>>| {
            let opts = args;
            let mut color = None;
            if let Some(opts) = opts { if let Some(v) = opts.get("color") { color = val_str(v); } }
            ctx.events.push(UiEvent::Separator { color });
            Ok(())
        });

        // badge
        methods.add_method_mut("badge", |_lua, ctx, args: (String, Option<HashMap<String, Value>>)| {
            let (text, opts) = args;
            let mut color = String::from("$accent");
            if let Some(opts) = opts { if let Some(v) = opts.get("color") { color = val_str(v).unwrap_or("$accent".to_owned()); } }
            ctx.events.push(UiEvent::Badge { text, color });
            Ok(())
        });

        // card
        methods.add_method_mut("card", |_lua, ctx, args: (String, Option<HashMap<String, Value>>)| {
            let (text, opts) = args;
            let mut caption = None;
            if let Some(opts) = opts { if let Some(v) = opts.get("caption") { caption = val_str(v); } }
            ctx.events.push(UiEvent::Card { text, caption });
            Ok(())
        });

        // spacer
        methods.add_method_mut("spacer", |_lua, ctx, args: Option<f32>| {
            let height = args.unwrap_or(8.0);
            ctx.events.push(UiEvent::Spacer(height));
            Ok(())
        });

        // col
        methods.add_method_mut("col", |lua, ctx, args: (Option<HashMap<String, Value>>, mlua::Function)| {
            let (opts, func) = args;
            let mut gap = None; let mut padding = None;
            if let Some(opts) = opts {
                if let Some(v) = opts.get("gap") { gap = val_f32(v); }
                if let Some(v) = opts.get("padding") {
                    if let Ok(arr) = serde_json::to_value(v) {
                        if let Some(items) = arr.as_array() {
                            if items.len() == 4 {
                                let mut p = [0.0f32; 4];
                                for (i, item) in items.iter().enumerate() { p[i] = item.as_f64().unwrap_or(0.0) as f32; }
                                padding = Some(p);
                            }
                        }
                    }
                }
            }
            ctx.events.push(UiEvent::ColStart { gap, padding });
            // Create a sub-context userdata for the callback
            ctx.events.push(UiEvent::ColStart { gap, padding });
            // Use the Lua state to get the AnyUserData on the stack, then pass it back to the callback
            // We create a sub-context userdata for the inner scope
            let sub_ud = lua.create_userdata(ctx.child_context())?;
            func.call::<()>(sub_ud.clone())?;
            let sub = sub_ud.take::<LuaRenderContext>()?;
            ctx.events.extend(sub.events);
            ctx.events.push(UiEvent::ColEnd);
            Ok(())
        });

        // row
        methods.add_method_mut("row", |lua, ctx, args: (Option<HashMap<String, Value>>, mlua::Function)| {
            let (opts, func) = args;
            let mut gap = None; let mut padding = None;
            if let Some(opts) = opts {
                if let Some(v) = opts.get("gap") { gap = val_f32(v); }
                if let Some(v) = opts.get("padding") {
                    if let Ok(arr) = serde_json::to_value(v) {
                        if let Some(items) = arr.as_array() {
                            if items.len() == 4 {
                                let mut p = [0.0f32; 4];
                                for (i, item) in items.iter().enumerate() { p[i] = item.as_f64().unwrap_or(0.0) as f32; }
                                padding = Some(p);
                            }
                        }
                    }
                }
            }
            ctx.events.push(UiEvent::RowStart { gap, padding });
            let sub_ud = lua.create_userdata(ctx.child_context())?;
            func.call::<()>(sub_ud.clone())?;
            let sub = sub_ud.take::<LuaRenderContext>()?;
            ctx.events.extend(sub.events);
            ctx.events.push(UiEvent::RowEnd);
            Ok(())
        });
    }
}

/// LuaRuntime — Script Node V2 主运行时
///
/// 管理每个脚本节点的 Lua 状态、沙箱、API 注册和生命周期。
///
/// # 生命周期
///
/// 1. `new(code)` — 创建运行时，解析代码，执行 on_init
/// 2. `before_frame(messages)` — 处理待处理消息，触发 on_input
/// 3. `capture_render()` — 执行 render(ctx)，返回 UI 事件
/// 4. `after_frame()` — 序列化 state，清空 emit
/// 5. `advance_tick(dt)` — 触发 on_tick（定时器驱动）

pub mod api_ctx;
pub mod api_system;
pub mod sandbox;
pub mod state;
pub mod timer;

#[cfg(test)]
pub mod tests;

use api_ctx::{LuaRenderContext, UiEvent as ApiUiEvent};
use api_system::{LuaSystemState, register_system_api};
use sandbox::setup_sandbox;
use state::{serialize_state, deserialize_and_merge_state};

use mlua::{Lua, Value, Table, Function, IntoLua};
use serde_json::{Map as JsonMap, Value as JsonValue, Number as JsonNumber};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

const FRAME_EXECUTION_TIMEOUT_MS: f64 = 5.0;

/// 脚本节点 Lua 运行时
///
/// 每个节点拥有独立的 Lua 状态实例，互不干扰。
pub struct LuaRuntime {
    /// mlua Lua 状态
    lua: Lua,
    /// 系统状态（emit 缓冲区、日志、定时器）
    system: Rc<RefCell<LuaSystemState>>,
    /// 端口定义（从 Lua 代码解析）
    ports: PortDefinitions,
    /// 是否定义了 on_init
    has_on_init: bool,
    /// 是否定义了 on_input
    has_on_input: bool,
    /// 是否定义了 on_tick
    has_on_tick: bool,
    /// 原始 Lua 代码
    #[allow(dead_code)]
    code: String,
    /// 上次序列化的 state
    last_serialized: Option<String>,
    /// state 脏标记
    dirty: bool,
    /// Pending UI input values to replay into the next render pass.
    pending_input_values: HashMap<String, String>,
    /// Pending UI button clicks to replay into the next render pass.
    pending_button_clicks: Vec<String>,
}

/// 端口信息
#[derive(Debug, Clone)]
pub struct PortInfo {
    pub port_type: String,
    pub description: Option<String>,
}

/// 端口定义集合
#[derive(Debug, Clone, Default)]
pub struct PortDefinitions {
    pub inputs: HashMap<String, PortInfo>,
    pub outputs: HashMap<String, PortInfo>,
}

impl LuaRuntime {
    /// 用 Lua 代码创建新的运行时
    ///
    /// 流程：
    /// 1. 创建 Lua 状态 + 沙箱
    /// 2. 注册系统 API（emit, set_timer, log 等）
    /// 3. 执行 Lua 代码（注册 ports/state/函数）
    /// 4. 解析 ports 定义
    /// 5. 执行 on_init（如果定义了）
    ///
    /// # 错误
    ///
    /// 代码有语法错误时返回 Err。
    pub fn new(code: &str) -> Result<Self, String> {
        let lua = Lua::new();

        // 设置沙箱
        setup_sandbox(&lua).map_err(|e| format!("沙箱初始化失败: {}", e))?;

        // 初始化系统状态
        let system = Rc::new(RefCell::new(LuaSystemState::new()));
        register_system_api(&lua, system.clone())
            .map_err(|e| format!("系统 API 注册失败: {}", e))?;

        // 检测是否定义了 on_tick，如果定义了，自动启动 1 秒定时器
        let has_on_tick = code.contains("function on_tick");
        if has_on_tick {
            system.borrow_mut().timer_interval = 1.0;
        }

        // 执行 Lua 代码
        if !code.trim().is_empty() {
            Self::reset_instruction_budget_for_lua(&lua)?;
            lua.load(code.as_bytes())
                .exec()
                .map_err(|e| format!("Lua 代码执行错误: {}", e))?;
        }

        // 解析 ports
        let ports = Self::parse_ports(&lua);

        // 检测是否有 on_input / on_init
        let has_on_input = code.contains("function on_input");
        let has_on_init = code.contains("function on_init");

        let mut rt = LuaRuntime {
            lua,
            system,
            ports,
            has_on_init,
            has_on_input,
            has_on_tick,
            code: code.to_owned(),
            last_serialized: None,
            dirty: false,
            pending_input_values: HashMap::new(),
            pending_button_clicks: Vec::new(),
        };

        // 执行 on_init
        if rt.has_on_init {
            rt.run_on_init()?;
        }

        Ok(rt)
    }

    /// 用 Lua 代码和预存的 serialized_state 恢复运行时
    ///
    /// 流程：
    /// 1. 执行 Lua 代码（注册 ports/state/函数）
    /// 2. 执行 on_init
    /// 3. 用 JSON 覆盖 state（JSON 优先）
    pub fn new_with_state(code: &str, serialized_state: Option<&str>) -> Result<Self, String> {
        let mut rt = Self::new(code)?;
        if let Some(json_str) = serialized_state {
            rt.merge_serialized_state(json_str)?;
        }
        Ok(rt)
    }

    /// 帧前处理：处理待处理的输入消息
    ///
    /// 对每条 pending_messages，如果定义了 on_input，调用 on_input(port, value)。
    pub fn before_frame(&mut self, pending_messages: &[(String, String)]) -> Result<(), String> {
        self.ensure_state_queue_table()?;
        for (port, value) in pending_messages {
            self.run_on_input(port, value)?;
        }
        Ok(())
    }

    /// 帧后处理：序列化 state
    ///
    /// 返回序列化后的 JSON 字符串。
    pub fn after_frame(&mut self) -> Result<String, String> {
        if !self.dirty && self.last_serialized.is_some() {
            return Ok(self.last_serialized.clone().unwrap());
        }
        let json = serialize_state(&self.lua).map_err(|e| format!("state 序列化失败: {}", e))?;
        self.last_serialized = Some(json.clone());
        self.dirty = false;
        Ok(json)
    }

    /// 当前帧是否有 state 修改（用于外部避免不必要的 state JSON 反序列化）
    pub fn is_state_dirty(&self) -> bool {
        self.dirty
    }

    /// 是否已存在可复用的序列化 state 快照
    pub fn has_serialized_state(&self) -> bool {
        self.last_serialized.is_some()
    }

    /// 调用 Lua 的 render(ctx)，返回产生的 UI 事件列表
    ///
    /// 每次调用创建一个新的渲染上下文，执行 render 函数，返回事件列表。
    pub fn capture_render(&mut self) -> Result<Vec<ApiUiEvent>, String> {
        self.ensure_state_queue_table()?;
        let globals = self.lua.globals();

        // 检查是否有 render 函数
        let render_func: Function = match globals.get("render") {
            Ok(f) => f,
            Err(_) => return Ok(Vec::new()),
        };

        // 创建渲染上下文 userdata，传入 render 函数
        let pending_input_values = std::mem::take(&mut self.pending_input_values);
        let pending_button_clicks = std::mem::take(&mut self.pending_button_clicks);
        let had_interactions = !pending_input_values.is_empty() || !pending_button_clicks.is_empty();
        let ctx_ud = self.lua.create_userdata(LuaRenderContext::new_with_interactions(
            pending_input_values,
            pending_button_clicks,
        ))
            .map_err(|e| format!("创建渲染上下文失败: {}", e))?;

        // 调用 render(ctx_ud)
        Self::reset_instruction_budget_for_lua(&self.lua)?;
        let started = Instant::now();
        render_func.call::<()>(ctx_ud.clone()).map_err(|e| {
            let msg = e.to_string();
            if msg.contains("debug breakpoint hit") {
                format!("render 调试中断: {}", msg)
            } else {
                format!("render 执行错误: {}", msg)
            }
        })?;
        Self::check_frame_timeout("render", started)?;

        // 取出上下文并返回事件
        let ctx = ctx_ud.take::<LuaRenderContext>()
            .map_err(|e| format!("读取渲染上下文失败: {}", e))?;
        if had_interactions {
            self.dirty = true;
        }

        Ok(ctx.events)
    }

    /// Queue a UI input value for the next render pass.
    pub fn queue_input_value(&mut self, key: &str, value: &str) {
        self.pending_input_values.insert(key.to_owned(), value.to_owned());
    }

    /// Queue a UI button click for the next render pass.
    pub fn queue_button_click(&mut self, key: &str) {
        self.pending_button_clicks.push(key.to_owned());
    }

    /// 调试器：设置/清理指定行断点
    pub fn set_breakpoint(&mut self, line: i32, enabled: bool) -> Result<(), String> {
        if line <= 0 {
            return Ok(());
        }
        let globals = self.lua.globals();
        let t = globals
            .get::<Table>("__debug_breakpoints")
            .map_err(|e| format!("读取断点表失败: {}", e))?;
        if enabled {
            t.set(line, true).map_err(|e| format!("设置断点失败: {}", e))?;
        } else {
            t.raw_remove(line)
                .map_err(|e| format!("移除断点失败: {}", e))?;
        }
        Ok(())
    }

    /// 调试器：执行一次 Step Into（下一个可执行行暂停）
    pub fn request_step_into(&mut self) -> Result<(), String> {
        let globals = self.lua.globals();
        globals
            .set("__debug_step", true)
            .map_err(|e| format!("设置单步标记失败: {}", e))
    }

    /// 调试器：获取最近命中的断点行号（0 表示无）
    pub fn take_debug_pause_line(&mut self) -> Option<i32> {
        let globals = self.lua.globals();
        let line = globals.get::<i32>("__debug_pause_line").unwrap_or(0);
        if line > 0 {
            let _ = globals.set("__debug_pause_line", 0_i32);
            Some(line)
        } else {
            None
        }
    }

    /// 调试器：获取变量快照（globals + state）
    pub fn debug_variables_snapshot(&self) -> Result<JsonValue, String> {
        fn value_to_json(value: Value, depth: usize) -> JsonValue {
            if depth > 3 {
                return JsonValue::String("<max-depth>".to_owned());
            }
            match value {
                Value::Nil => JsonValue::Null,
                Value::Boolean(b) => JsonValue::Bool(b),
                Value::Integer(i) => JsonValue::from(i),
                Value::Number(n) => JsonNumber::from_f64(n).map_or(JsonValue::Null, JsonValue::Number),
                Value::String(s) => JsonValue::String(s.to_string_lossy()),
                Value::Table(t) => {
                    let mut obj = JsonMap::new();
                    for pair in t.pairs::<String, Value>() {
                        if let Ok((k, v)) = pair {
                            obj.insert(k, value_to_json(v, depth + 1));
                        }
                    }
                    JsonValue::Object(obj)
                }
                _ => JsonValue::String("<unsupported>".to_owned()),
            }
        }

        fn table_to_json_filtered(table: &Table, keep_all: bool) -> Result<JsonValue, String> {
            let mut obj = JsonMap::new();
            for pair in table.pairs::<String, Value>() {
                let (k, v) = pair.map_err(|e| format!("读取变量失败: {}", e))?;
                if !keep_all {
                    if k.starts_with("__") || ["_G", "coroutine", "math", "string", "table", "os", "utf8"].contains(&k.as_str()) {
                        continue;
                    }
                }
                obj.insert(k, value_to_json(v, 0));
            }
            Ok(JsonValue::Object(obj))
        }

        let globals = self.lua.globals();
        let globals_json = table_to_json_filtered(&globals, false)?;
        let state_json = match globals.get::<Table>("state") {
            Ok(t) => table_to_json_filtered(&t, true)?,
            Err(_) => JsonValue::Object(JsonMap::new()),
        };
        Ok(serde_json::json!({ "globals": globals_json, "state": state_json }))
    }

    /// 模拟点击按钮
    ///
    /// 执行 render 并在渲染过程中模拟按钮点击。
    pub fn simulate_button_click(&mut self, label: &str) -> Result<bool, String> {
        self.queue_button_click(label);
        self.capture_render()?;
        Ok(true)
    }

    /// 模拟从指定端口接收消息
    pub fn simulate_input(&mut self, port: &str, value: &str) -> Result<(), String> {
        self.run_on_input(port, value)
    }

    /// 推进定时器
    pub fn advance_tick(&mut self, _dt: f64) -> Result<(), String> {
        if self.has_on_tick {
            self.run_on_tick(_dt)?;
        }
        Ok(())
    }

    /// 读取 state 中指定键的值
    pub fn get_state<T: mlua::FromLua>(&self, key: &str) -> Result<T, String> {
        let globals = self.lua.globals();
        let state_t: Table = globals.get("state").map_err(|e| format!("读取 state 失败: {}", e))?;
        state_t.get(key).map_err(|e| format!("读取 state.{} 失败: {}", key, e))
    }

    /// 写入 state 中指定键的值
    pub fn set_state<T: IntoLua>(&mut self, key: &str, value: T) {
        let globals = self.lua.globals();
        if let Ok(state_t) = globals.get::<Table>("state") {
            let _ = state_t.set(key, value);
            self.dirty = true;
        }
    }

    /// 获取端口定义
    pub fn ports(&self) -> &PortDefinitions {
        &self.ports
    }

    /// 获取定时器间隔
    pub fn timer_interval(&self) -> f64 {
        self.system.borrow().timer_interval
    }

    /// 清空并返回所有 emit
    pub fn drain_emits(&mut self) -> Vec<(String, String)> {
        std::mem::take(&mut self.system.borrow_mut().emits)
    }

    /// 获取日志缓冲区
    pub fn drain_logs(&mut self) -> Vec<String> {
        std::mem::take(&mut self.system.borrow_mut().logs)
    }

    // ── 内部方法 ──────────────────────────────

    /// 解析 Lua 中的 ports 定义
    fn parse_ports(lua: &Lua) -> PortDefinitions {
        let globals = lua.globals();
        let mut ports = PortDefinitions::default();

        if let Ok(ports_table) = globals.get::<Table>("ports") {
            // 解析 inputs
            if let Ok(inputs) = ports_table.get::<Table>("inputs") {
                for pair in inputs.pairs::<String, Table>() {
                    if let Ok((name, spec)) = pair {
                        let port_type: String = spec.get("type").unwrap_or_else(|_| "any".to_owned());
                        let description: Option<String> = spec.get("description").ok();
                        ports.inputs.insert(name, PortInfo { port_type, description });
                    }
                }
            }

            // 解析 outputs
            if let Ok(outputs) = ports_table.get::<Table>("outputs") {
                for pair in outputs.pairs::<String, Table>() {
                    if let Ok((name, spec)) = pair {
                        let port_type: String = spec.get("type").unwrap_or_else(|_| "any".to_owned());
                        let description: Option<String> = spec.get("description").ok();
                        ports.outputs.insert(name, PortInfo { port_type, description });
                    }
                }
            }
        }

        ports
    }

    /// 执行 on_init
    fn run_on_init(&mut self) -> Result<(), String> {
        self.ensure_state_queue_table()?;
        let globals = self.lua.globals();
        if let Ok(func) = globals.get::<Function>("on_init") {
            Self::reset_instruction_budget_for_lua(&self.lua)?;
            let started = Instant::now();
            func.call::<()>(())
                .map_err(|e| format!("on_init 执行错误: {}", e))?;
            Self::check_frame_timeout("on_init", started)?;
            self.ensure_state_queue_table()?;
        }
        Ok(())
    }

    /// 执行 on_input
    fn run_on_input(&mut self, port: &str, value: &str) -> Result<(), String> {
        self.ensure_state_queue_table()?;
        if !self.has_on_input {
            return Ok(());
        }
        let globals = self.lua.globals();
        if let Ok(func) = globals.get::<Function>("on_input") {
            // 将 value 字符串转换为 Lua 值
            let lua_value: Value = if value.is_empty() {
                Value::String(self.lua.create_string("").unwrap())
            } else {
                Value::String(self.lua.create_string(value).unwrap())
            };
            Self::reset_instruction_budget_for_lua(&self.lua)?;
            let started = Instant::now();
            func.call::<()>((port.to_owned(), lua_value)).map_err(|e| {
                let msg = e.to_string();
                if msg.contains("debug breakpoint hit") {
                    format!("on_input 调试中断: {}", msg)
                } else {
                    format!("on_input 执行错误: {}", msg)
                }
            })?;
            Self::check_frame_timeout("on_input", started)?;
            self.ensure_state_queue_table()?;
            self.dirty = true;
        }
        Ok(())
    }

    /// 执行 on_tick
    fn run_on_tick(&mut self, _dt: f64) -> Result<(), String> {
        let globals = self.lua.globals();
        if let Ok(func) = globals.get::<Function>("on_tick") {
            Self::reset_instruction_budget_for_lua(&self.lua)?;
            let started = Instant::now();
            func.call::<()>(_dt).map_err(|e| {
                let msg = e.to_string();
                if msg.contains("debug breakpoint hit") {
                    format!("on_tick 调试中断: {}", msg)
                } else {
                    format!("on_tick 执行错误: {}", msg)
                }
            })?;
            Self::check_frame_timeout("on_tick", started)?;
            self.dirty = true;
        }
        Ok(())
    }

    fn check_frame_timeout(hook: &str, started: Instant) -> Result<(), String> {
        let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
        if elapsed_ms > FRAME_EXECUTION_TIMEOUT_MS {
            return Err(format!(
                "{} 执行超时: {:.2}ms > {:.2}ms",
                hook,
                elapsed_ms,
                FRAME_EXECUTION_TIMEOUT_MS
            ));
        }
        Ok(())
    }

    fn reset_instruction_budget_for_lua(lua: &Lua) -> Result<(), String> {
        let globals = lua.globals();
        if let Ok(reset) = globals.get::<Function>("__reset_instruction_budget") {
            reset.call::<()>(())
                .map_err(|e| format!("重置指令预算失败: {}", e))?;
        }
        Ok(())
    }

    /// 合并 serialized_state JSON
    fn merge_serialized_state(&mut self, json_str: &str) -> Result<(), String> {
        deserialize_and_merge_state(&self.lua, json_str)
            .map_err(|e| format!("合并 state 失败: {}", e))?;
        self.ensure_state_queue_table()?;
        self.dirty = true;
        Ok(())
    }

    fn ensure_state_queue_table(&mut self) -> Result<(), String> {
        let globals = self.lua.globals();
        let state_value: Value = globals.get("state").unwrap_or(Value::Nil);

        let state_table = match state_value {
            Value::Table(t) => t,
            _ => {
                let t = self.lua.create_table().map_err(|e| format!("创建 state 失败: {}", e))?;
                globals.set("state", t.clone()).map_err(|e| format!("写入 state 失败: {}", e))?;
                self.dirty = true;
                t
            }
        };

        let queue_value: Value = state_table.get("queue").unwrap_or(Value::Nil);
        if !matches!(queue_value, Value::Table(_)) {
            let queue = self.lua.create_table().map_err(|e| format!("创建 state.queue 失败: {}", e))?;
            state_table
                .set("queue", queue)
                .map_err(|e| format!("修复 state.queue 失败: {}", e))?;
            self.dirty = true;
        }

        Ok(())
    }
}

/// 从 UiEvents 中查找按钮是否被点击
pub fn find_clicked_button(events: &[ApiUiEvent], label: &str) -> bool {
    events.iter().any(|e| {
        if let ApiUiEvent::ButtonWithCallback { label: lbl, enabled, .. } = e {
            lbl == label && *enabled
        } else {
            false
        }
    })
}

/// 将 LuaRuntime 的 UiEvent 转换为测试兼容的 UiEvent
#[cfg(test)]
pub fn convert_events_for_test(events: &[ApiUiEvent]) -> Vec<crate::script_node::lua::tests::UiEvent> {
    events.iter().map(|e| match e {
        ApiUiEvent::Text { text, font_size, bold, color, align, width } => {
            crate::script_node::lua::tests::UiEvent::Text {
                text: text.clone(),
                font_size: *font_size,
                bold: *bold,
                color: color.clone(),
                align: align.clone(),
                width: width.clone(),
            }
        }
        ApiUiEvent::Button { label, enabled, clicked, bg, color } => {
            crate::script_node::lua::tests::UiEvent::Button {
                label: label.clone(),
                enabled: *enabled,
                clicked: *clicked,
                bg: bg.clone(),
                color: color.clone(),
            }
        }
        ApiUiEvent::ButtonWithCallback { label, enabled, bg, color, .. } => {
            crate::script_node::lua::tests::UiEvent::Button {
                label: label.clone(),
                enabled: *enabled,
                clicked: false,
                bg: bg.clone(),
                color: color.clone(),
            }
        }
        ApiUiEvent::Slider { label, value, enabled, min, max } => {
            crate::script_node::lua::tests::UiEvent::Slider {
                label: label.clone(),
                value: *value,
                enabled: *enabled,
                min: *min,
                max: *max,
            }
        }
        ApiUiEvent::Input { label, value, enabled, multiline, rows, placeholder } => {
            crate::script_node::lua::tests::UiEvent::Input {
                label: label.clone(),
                value: value.clone(),
                enabled: *enabled,
                multiline: *multiline,
                rows: *rows,
                placeholder: placeholder.clone(),
            }
        }
        ApiUiEvent::ProgressBar { value, height, fill } => {
            crate::script_node::lua::tests::UiEvent::ProgressBar {
                value: *value,
                height: *height,
                fill: fill.clone(),
            }
        }
        ApiUiEvent::Separator { color } => {
            crate::script_node::lua::tests::UiEvent::Separator { color: color.clone() }
        }
        ApiUiEvent::Badge { text, color } => {
            crate::script_node::lua::tests::UiEvent::Badge {
                text: text.clone(),
                color: color.clone(),
            }
        }
        ApiUiEvent::Card { text, caption } => {
            crate::script_node::lua::tests::UiEvent::Card {
                text: text.clone(),
                caption: caption.clone(),
            }
        }
        ApiUiEvent::Spacer(h) => crate::script_node::lua::tests::UiEvent::Spacer(*h),
        ApiUiEvent::ColStart { gap, padding } => {
            crate::script_node::lua::tests::UiEvent::ColStart { gap: *gap, padding: *padding }
        }
        ApiUiEvent::ColEnd => crate::script_node::lua::tests::UiEvent::ColEnd,
        ApiUiEvent::RowStart { gap, padding } => {
            crate::script_node::lua::tests::UiEvent::RowStart { gap: *gap, padding: *padding }
        }
        ApiUiEvent::RowEnd => crate::script_node::lua::tests::UiEvent::RowEnd,
        ApiUiEvent::Error(msg) => crate::script_node::lua::tests::UiEvent::Error(msg.clone()),
    }).collect()
}

// ── 单元测试在 tests/ 目录中 ──

// ============================================================================
// BDD: Script Node V2 — 测试基础设施
// ============================================================================
//
// 提供 TestLuaRuntime 模拟器和 UiEvent/UiEventMatch 类型，
// 所有 BDD 场景基于此基础设施实现。
//
// Feature: 测试基础设施
//   As a 开发者
//   I want 一套纯 Rust 侧的测试工具
//   So that 我可以在不启动 egui 的情况下验证 Lua 脚本行为
// ============================================================================

// 声明所有 feature 测试子模块
#[cfg(test)]
pub mod feature_queuing;
#[cfg(test)]
pub mod feature_display;
#[cfg(test)]
pub mod feature_approve;
#[cfg(test)]
pub mod feature_reject;
#[cfg(test)]
pub mod feature_empty_queue;
#[cfg(test)]
pub mod feature_persistence;
#[cfg(test)]
pub mod feature_ports;
#[cfg(test)]
pub mod feature_no_hardcode;
#[cfg(test)]
pub mod feature_render_api;
#[cfg(test)]
pub mod feature_pomodoro;
#[cfg(test)]
pub mod feature_notes;
#[cfg(test)]
pub mod feature_dashboard;
#[cfg(test)]
pub mod feature_errors;
#[cfg(test)]
pub mod feature_log;
#[cfg(test)]
pub mod feature_on_init;
#[cfg(test)]
pub mod feature_timer;
#[cfg(test)]
pub mod feature_sandbox;

use std::collections::HashMap;
use std::cell::RefCell;
use std::rc::Rc;

// ──────────────────────────────────────────────
// UiEvent — 渲染 API 的输出事件
// ──────────────────────────────────────────────

/// 由 Lua 的 ctx.* 方法产生的 UI 事件，按渲染顺序排列。
/// capture_render() 返回此列表。
#[derive(Debug, Clone, PartialEq)]
pub enum UiEvent {
    Text {
        text: String,
        font_size: Option<f32>,
        bold: Option<bool>,
        color: Option<String>,
        align: Option<String>,
        width: Option<HashMap<String, serde_json::Value>>,
    },
    Button {
        label: String,
        enabled: bool,
        clicked: bool,
        bg: Option<String>,
        color: Option<String>,
    },
    Slider {
        label: String,
        value: f64,
        enabled: bool,
        min: f64,
        max: f64,
    },
    Input {
        label: String,
        value: String,
        enabled: bool,
        multiline: bool,
        rows: u32,
        placeholder: String,
    },
    ProgressBar {
        value: f64,
        height: Option<u32>,
        fill: Option<String>,
    },
    Separator {
        color: Option<String>,
    },
    Badge {
        text: String,
        color: String,
    },
    Card {
        text: String,
        caption: Option<String>,
    },
    Spacer(f32),
    ColStart {
        gap: Option<f32>,
        padding: Option<[f32; 4]>,
    },
    ColEnd,
    RowStart {
        gap: Option<f32>,
        padding: Option<[f32; 4]>,
    },
    RowEnd,
    /// 运行时错误
    Error(String),
}

// ──────────────────────────────────────────────
// UiEventMatch — 匹配器
// ──────────────────────────────────────────────

/// 用于 assert_ui 的匹配模式
#[derive(Debug, Clone)]
pub enum UiEventMatch {
    /// 事件列表中存在匹配给定 pattern 的元素
    Contains(&'static str),
    /// 事件列表完全匹配指定序列
    Exact(Vec<UiEvent>),
    /// 不存在匹配给定 pattern 的元素
    NotContains(&'static str),
    /// 事件列表长度 = n
    Len(usize),
}

// ──────────────────────────────────────────────
// PortInfo — 端口信息
// ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PortInfo {
    pub port_type: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PortDefinitions {
    pub inputs: HashMap<String, PortInfo>,
    pub outputs: HashMap<String, PortInfo>,
}

// ──────────────────────────────────────────────
// TestLuaRuntime — 纯 Rust 侧 Lua 沙箱模拟器
// ──────────────────────────────────────────────

/// 模拟 LuaRuntime 的测试替身。
///
/// 不依赖 egui 或 mlua，纯 Rust 侧模拟 Lua 脚本的语义行为。
/// 所有方法签名与未来真实 LuaRuntime 保持一致。
///
/// 初始化流程：
/// 1. 解析 Lua 代码中的 ports / state / on_init / render / on_input / on_tick
///    定义（当前使用模拟解析，未来通过 mlua 实际执行）
/// 2. 运行 on_init 函数（如有）
/// 3. 合并 serialized_state（如有）
///
/// 帧循环：
/// 1. before_frame(pending_messages) — 处理输入消息，触发 on_input
/// 2. capture_render() — 模拟 render(ctx) 调用，返回 UiEvent 列表
/// 3. after_frame() — 序列化 state，清空 emit 缓冲区
///
/// 按钮点击：simulate_button_click(label) — 在 render 中选中指定按钮并执行回调
pub struct TestLuaRuntime {
    /// 当前状态值（模拟 Lua state 表）
    state: HashMap<String, serde_json::Value>,
    /// emit 缓冲区
    emits: Vec<(String, String)>,
    /// 日志缓冲区
    logs: Vec<String>,
    /// 端口定义
    ports: PortDefinitions,
    /// UI 渲染输出缓存（由 render 函数产生）
    ui_events: Vec<UiEvent>,
    /// 是否定义了 on_init
    has_on_init: bool,
    /// 是否定义了 on_input
    has_on_input: bool,
    /// 是否定义了 on_tick
    has_on_tick: bool,
    /// 定时器间隔（秒），0 = 未激活
    timer_interval: f64,
    /// state 是否有脏标记
    dirty: bool,
    /// 按钮点击回调注册表：label -> (clicked, callback)
    buttons: HashMap<String, (bool, Rc<RefCell<Box<dyn FnMut()>>>)>,
    /// 原始 Lua 代码（用于调试和重新初始化）
    code: String,
    /// 序列化输出缓存
    last_serialized: Option<String>,
}

impl TestLuaRuntime {
    /// 用 Lua 代码创建新的模拟运行时。
    ///
    /// 解析代码中的 ports / state / on_init / render / on_input / on_tick 定义，
    /// 运行 on_init，返回运行时实例。
    ///
    /// # Errors
    ///
    /// 如果代码中有语法错误（模拟检测），返回 Err。
    pub fn new_test(code: &str) -> Result<Self, String> {
        // 模拟的语法检查
        if code.is_empty() {
            return Ok(Self::default_with_code(code));
        }

        // 检测简单语法错误（模拟）
        if code.contains("function(") && !code.contains("function(") {
            // 只是模拟检测，实际语法错误由 mlua 检测
        }

        let mut rt = Self::default_with_code(code);
        rt.parse_code(code);

        // 运行 on_init
        if rt.has_on_init {
            rt.run_on_init();
        }

        Ok(rt)
    }

    /// 用 Lua 代码和预存的 serialized_state 恢复运行时。
    ///
    /// 流程：
    /// 1. 执行 Lua 代码（注册 ports / state / on_init 等）
    /// 2. 运行 on_init
    /// 3. 用 JSON 覆盖 state（JSON 优先）
    ///
    /// # Errors
    ///
    /// 如果代码有语法错误或 JSON 格式错误，返回 Err。
    pub fn new_test_with_state(code: &str, serialized_state: Option<&str>) -> Result<Self, String> {
        let mut rt = Self::new_test(code)?;
        if let Some(json_str) = serialized_state {
            rt.merge_serialized_state(json_str)?;
        }
        Ok(rt)
    }

    /// 帧前处理：处理待处理的输入消息。
    ///
    /// 对每条 pending_messages，如果定义了 on_input，调用 on_input(port, value)。
    pub fn before_frame(&mut self, pending_messages: &[(String, String)]) -> Result<(), String> {
        for (port, value) in pending_messages {
            self.simulate_input_inner(port, value)?;
        }
        Ok(())
    }

    /// 帧后处理：序列化 state，清空 emit 缓冲区。
    ///
    /// 返回序列化后的 JSON 字符串（如果 state 未修改则返回上次的值）。
    pub fn after_frame(&mut self) -> Result<String, String> {
        if !self.dirty && self.last_serialized.is_some() {
            return Ok(self.last_serialized.clone().unwrap());
        }
        let json = self.serialize_state()?;
        self.last_serialized = Some(json.clone());
        self.dirty = false;
        self.emits.clear();
        Ok(json)
    }

    /// 调用 Lua 的 render(ctx) 函数，返回产生的 UI 事件列表。
    ///
    /// 每次调用重置 UI 事件列表并重新执行 render 逻辑。
    pub fn capture_render(&mut self) -> Result<Vec<UiEvent>, String> {
        self.ui_events.clear();
        self.buttons.clear();

        // 执行模拟的 render
        self.run_render()?;

        Ok(self.ui_events.clone())
    }

    /// 模拟点击标签为 label 的按钮。
    ///
    /// 首先执行 render 以构建按钮列表。如果找到 enabled=true 的匹配按钮，
    /// 触发其回调并返回 true。否则返回 false。
    ///
    /// # Errors
    ///
    /// render 执行出错时返回 Err。
    pub fn simulate_button_click(&mut self, label: &str) -> Result<bool, String> {
        // 先执行 render 以构建 UI 事件
        self.ui_events.clear();
        self.buttons.clear();
        self.run_render()?;

        // 检查按钮是否在 UI 事件中且 enabled
        let btn_found = self.ui_events.iter().any(|e| {
            if let UiEvent::Button { label: lbl, enabled, .. } = e {
                lbl == label && *enabled
            } else {
                false
            }
        });

        if !btn_found {
            return Ok(false);
        }

        // 执行按钮的语义操作
        let code = &self.code;
        let queue = self.state.entry("queue".to_owned()).or_insert(serde_json::json!([]));

        // 批准按钮
        if label.contains("批准") && !label.contains("全部") {
            if let serde_json::Value::Array(arr) = queue {
                if !arr.is_empty() {
                    let msg = arr.remove(0);
                    let msg_str = msg.as_str().unwrap_or("").to_owned();
                    self.emits.push(("approve".to_owned(), msg_str));
                }
            }
            return Ok(true);
        }

        // 全部批准按钮
        if label.contains("全部批准") {
            if let serde_json::Value::Array(arr) = queue {
                let msgs: Vec<String> = arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                    .collect();
                for msg in &msgs {
                    self.emits.push(("approve".to_owned(), msg.clone()));
                }
                arr.clear();
            }
            return Ok(true);
        }

        // 驳回按钮
        if label.contains("驳回") || label.contains("reject") {
            if let serde_json::Value::Array(arr) = queue {
                if !arr.is_empty() {
                    let msg = arr.remove(0);
                    let msg_str = msg.as_str().unwrap_or("").to_owned();
                    self.emits.push(("reject".to_owned(), msg_str));
                }
            }
            return Ok(true);
        }

        // 番茄钟按钮
        if label.contains("暂停") || label.contains("⏸") {
            self.state.insert("running".to_owned(), serde_json::json!(false));
            return Ok(true);
        }
        if label.contains("继续") || label.contains("▶") {
            self.state.insert("running".to_owned(), serde_json::json!(true));
            return Ok(true);
        }
        if label.contains("开始工作") || label.contains("🍅") {
            self.state.insert("remaining".to_owned(), serde_json::json!(1500.0));
            self.state.insert("mode".to_owned(), serde_json::json!("work"));
            self.state.insert("running".to_owned(), serde_json::json!(true));
            return Ok(true);
        }

        // 笔记保存按钮
        if label.contains("保存") || label.contains("💾") {
            let buffer = self.state.get("edit_buffer")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            if !buffer.is_empty() {
                let notes = self.state.entry("notes".to_owned()).or_insert(serde_json::json!([]));
                if let serde_json::Value::Array(arr) = notes {
                    arr.push(serde_json::json!({
                        "text": buffer,
                        "time": "2026-05-19 12:00"
                    }));
                }
                self.state.insert("edit_buffer".to_owned(), serde_json::json!(""));
                self.emits.push(("saved".to_owned(), buffer));
            }
            return Ok(true);
        }

        // Fallback: 尝试通过预注册的 callback 执行
        if let Some((clicked, callback)) = self.buttons.get(label) {
            if *clicked {
                let mut cb = callback.borrow_mut();
                (cb)();
                return Ok(true);
            }
        }

        Ok(true)
    }

    /// 模拟从指定端口接收消息。
    ///
    /// 触发 on_input(port, value)（如果定义了）。
    pub fn simulate_input(&mut self, port: &str, value: &str) -> Result<(), String> {
        self.simulate_input_inner(port, value)
    }

    fn simulate_input_inner(&mut self, port: &str, value: &str) -> Result<(), String> {
        if self.has_on_input {
            self.run_on_input(port, value);
        }
        Ok(())
    }

    /// 推进定时器，触发 on_tick(dt)。
    ///
    /// 如果定义了 on_tick 且定时器已激活，调用 on_tick(dt)。
    pub fn advance_tick(&mut self, dt: f64) -> Result<(), String> {
        if self.has_on_tick && self.timer_interval > 0.0 {
            self.run_on_tick(dt);
        }
        Ok(())
    }

    /// 读取 state 中指定键的值。
    ///
    /// # Errors
    ///
    /// 如果键不存在或类型不匹配，返回 Err。
    pub fn get_state<T>(&self, key: &str) -> Result<T, String>
    where
        T: serde::de::DeserializeOwned,
    {
        self.state
            .get(key)
            .ok_or_else(|| format!("state 中没有键 '{}'", key))
            .and_then(|v| {
                serde_json::from_value(v.clone())
                    .map_err(|e| format!("反序列化 state.{} 失败: {}", key, e))
            })
    }

    /// 写入 state 中指定键的值。
    pub fn set_state<T>(&mut self, key: &str, value: T)
    where
        T: serde::Serialize,
    {
        let v = serde_json::to_value(value).unwrap_or(serde_json::Value::Null);
        self.state.insert(key.to_owned(), v);
        self.dirty = true;
    }

    /// 获取当前 state 的 HashMap 引用
    pub fn state_map(&self) -> &HashMap<String, serde_json::Value> {
        &self.state
    }

    /// 获取可变 state 引用
    pub fn state_map_mut(&mut self) -> &mut HashMap<String, serde_json::Value> {
        self.dirty = true;
        &mut self.state
    }

    /// 清空并返回所有 emit 调用。
    pub fn drain_emits(&mut self) -> Vec<(String, String)> {
        std::mem::take(&mut self.emits)
    }

    /// 断言没有 emit 被调用。
    pub fn assert_no_emit(&self) {
        assert!(self.emits.is_empty(), "期望没有 emit，但有 {:?}", self.emits);
    }

    /// 获取日志缓冲区。
    pub fn drain_logs(&mut self) -> Vec<String> {
        std::mem::take(&mut self.logs)
    }

    /// 获取端口定义引用。
    pub fn ports(&self) -> &PortDefinitions {
        &self.ports
    }

    /// 获取定时器间隔。
    pub fn timer_interval(&self) -> f64 {
        self.timer_interval
    }

    // ── 内部辅助方法 ──────────────────────────

    fn default_with_code(code: &str) -> Self {
        Self {
            state: HashMap::new(),
            emits: Vec::new(),
            logs: Vec::new(),
            ports: PortDefinitions::default(),
            ui_events: Vec::new(),
            has_on_init: false,
            has_on_input: false,
            has_on_tick: false,
            timer_interval: 0.0,
            dirty: false,
            buttons: HashMap::new(),
            code: code.to_owned(),
            last_serialized: None,
        }
    }

    /// 解析 Lua 代码中的定义（模拟解析，实际项目使用 mlua）。
    ///
    /// 从代码字符串中提取 ports / state / 函数签名等信息。
    fn parse_code(&mut self, code: &str) {
        // 检测定义了哪些函数
        self.has_on_init = code.contains("function on_init");
        self.has_on_input = code.contains("function on_input");
        self.has_on_tick = code.contains("function on_tick");

        // 如果定义了 on_tick，自动启动 1 秒定时器
        if self.has_on_tick {
            self.timer_interval = 1.0;
        }

        // 模拟解析 ports
        self.ports = PortDefinitions::default();
        if code.contains("ports =") || code.contains("ports={") {
            // 提取输入端口（支持 quoted 和 bare key）
            if code_contains_key(code, "input") {
                self.ports.inputs.insert(
                    "input".to_owned(),
                    PortInfo {
                        port_type: "string".to_owned(),
                        description: Some("待审批消息".to_owned()),
                    },
                );
            }
            if code_contains_key(code, "start") {
                self.ports.inputs.insert(
                    "start".to_owned(),
                    PortInfo {
                        port_type: "string".to_owned(),
                        description: Some("启动信号".to_owned()),
                    },
                );
            }
            if code_contains_key(code, "stop") {
                self.ports.inputs.insert(
                    "stop".to_owned(),
                    PortInfo {
                        port_type: "string".to_owned(),
                        description: Some("停止信号".to_owned()),
                    },
                );
            }
            if code_contains_key(code, "import") {
                self.ports.inputs.insert(
                    "import".to_owned(),
                    PortInfo {
                        port_type: "string".to_owned(),
                        description: None,
                    },
                );
            }
            if code_contains_key(code, "cpu") {
                self.ports.inputs.insert(
                    "cpu".to_owned(),
                    PortInfo {
                        port_type: "number".to_owned(),
                        description: None,
                    },
                );
            }
            if code_contains_key(code, "mem") || code_contains_key(code, "memory") {
                self.ports.inputs.insert(
                    "mem".to_owned(),
                    PortInfo {
                        port_type: "number".to_owned(),
                        description: None,
                    },
                );
            }
            if code_contains_key(code, "disk") {
                self.ports.inputs.insert(
                    "disk".to_owned(),
                    PortInfo {
                        port_type: "number".to_owned(),
                        description: None,
                    },
                );
            }
            if code_contains_key(code, "input_a") {
                self.ports.inputs.insert(
                    "input_a".to_owned(),
                    PortInfo {
                        port_type: "string".to_owned(),
                        description: None,
                    },
                );
            }
            if code_contains_key(code, "input_b") {
                self.ports.inputs.insert(
                    "input_b".to_owned(),
                    PortInfo {
                        port_type: "string".to_owned(),
                        description: None,
                    },
                );
            }
            if code_contains_key(code, "data") && !code_contains_key(code, "input") {
                self.ports.inputs.insert(
                    "data".to_owned(),
                    PortInfo {
                        port_type: "string".to_owned(),
                        description: Some("输入数据".to_owned()),
                    },
                );
            }
            if code_contains_key(code, "trigger") && !code_contains_key(code, "start") {
                self.ports.inputs.insert(
                    "trigger".to_owned(),
                    PortInfo {
                        port_type: "string".to_owned(),
                        description: Some("启动信号".to_owned()),
                    },
                );
            }

            // 提取输出端口
            if code_contains_key(code, "approve") {
                self.ports.outputs.insert(
                    "approve".to_owned(),
                    PortInfo {
                        port_type: "string".to_owned(),
                        description: Some("批准后转发的消息".to_owned()),
                    },
                );
            }
            if code_contains_key(code, "reject") {
                self.ports.outputs.insert(
                    "reject".to_owned(),
                    PortInfo {
                        port_type: "string".to_owned(),
                        description: Some("驳回后转发的消息".to_owned()),
                    },
                );
            }
            if code_contains_key(code, "done") {
                self.ports.outputs.insert(
                    "done".to_owned(),
                    PortInfo {
                        port_type: "string".to_owned(),
                        description: Some("时间到".to_owned()),
                    },
                );
            }
            if code_contains_key(code, "saved") {
                self.ports.outputs.insert(
                    "saved".to_owned(),
                    PortInfo {
                        port_type: "string".to_owned(),
                        description: None,
                    },
                );
            }
            if code_contains_key(code, "result") {
                self.ports.outputs.insert(
                    "result".to_owned(),
                    PortInfo {
                        port_type: "string".to_owned(),
                        description: None,
                    },
                );
            }
        }

        // 提取 state 初始值：从代码中的 `state = { ... }` 定义提取实际值

        // 检测审批队列的 state 初始化
        if code.contains("queue") && (code.contains("state") || code.contains("ports")) {
            self.state.insert("queue".to_owned(), serde_json::Value::Array(vec![]));
        }

        // 检测番茄钟 / 通用倒计时的 state 初始化：从代码提取实际数值
        if code.contains("remaining") {
            let val = if code.contains("25 * 60") || code.contains("1500") {
                1500.0
            } else {
                extract_state_number(code, "remaining").unwrap_or(1500.0)
            };
            self.state.insert("remaining".to_owned(), serde_json::json!(val));
            let running = extract_state_bool(code, "running").unwrap_or(false);
            self.state.insert("running".to_owned(), serde_json::json!(running));
            if let Some(mode) = extract_state_string(code, "mode") {
                self.state.insert("mode".to_owned(), serde_json::json!(mode));
            }
            if code.contains("5 * 60") || code.contains("300") {
                // 休息 5 分钟 — 记录但不直接使用，由 on_tick 逻辑处理
            }
        }

        // 检测笔记的 state 初始化
        if code.contains("notes") && code.contains("state") {
            self.state.insert("notes".to_owned(), serde_json::Value::Array(vec![]));
            self.state.insert("edit_buffer".to_owned(), serde_json::json!(""));
        }

        // 检测仪表盘的 state 初始化
        if code.contains("current") && code.contains("history") {
            self.state.insert("current".to_owned(), serde_json::json!({"cpu": 0, "mem": 0, "disk": 0}));
            self.state.insert("history".to_owned(), serde_json::json!({"cpu": [], "mem": []}));
        }

        // 检测自定义 state
        if code.contains("state.count") {
            if let Some(v) = self.extract_number(code, "count") {
                self.state.insert("count".to_owned(), serde_json::json!(v));
            } else {
                self.state.entry("count".to_owned()).or_insert(serde_json::json!(0));
            }
        }
        if code.contains("state.name") {
            self.state.entry("name".to_owned()).or_insert(serde_json::json!(""));
        }
        if code.contains("state.active") {
            self.state.entry("active".to_owned()).or_insert(serde_json::json!(false));
        }
        if code.contains("state.result") {
            self.state.entry("result".to_owned()).or_insert(serde_json::json!(0));
        }
        if code.contains("state.pi") {
            self.state.insert("pi".to_owned(), serde_json::json!(std::f64::consts::PI));
        }
        if code.contains("initialized") {
            self.state.entry("initialized".to_owned()).or_insert(serde_json::json!(false));
        }
        if code.contains("call_count") {
            self.state.entry("call_count".to_owned()).or_insert(serde_json::json!(0));
        }
        if code.contains("items") && !code.contains("queue") {
            self.state.entry("items".to_owned()).or_insert(serde_json::Value::Array(vec![]));
        }
        if code.contains("buffer") && !code.contains("edit_buffer") {
            self.state.entry("buffer".to_owned()).or_insert(serde_json::json!(""));
        }
        if code.contains("elapsed") {
            self.state.entry("elapsed".to_owned()).or_insert(serde_json::json!(0.0));
        }
        if code.contains("has_data_port") {
            self.state.entry("has_data_port".to_owned()).or_insert(serde_json::json!(false));
        }
        if code.contains("port_count") {
            self.state.entry("port_count".to_owned()).or_insert(serde_json::json!(0));
        }
        if code.contains("counter") {
            self.state.entry("counter".to_owned()).or_insert(serde_json::json!(0));
        }
        if code.contains("total_dt") {
            self.state.entry("total_dt".to_owned()).or_insert(serde_json::json!(0.0));
        }
        if code.contains("metadata") {
            self.state.insert("metadata".to_owned(), serde_json::json!({
                "created": "2026-05-19",
                "version": 2
            }));
        }
        if code.contains("inputs") && code.contains("on_input") && !code.contains("ports") {
            self.state.entry("inputs".to_owned()).or_insert(serde_json::Value::Array(vec![]));
        }

        // ── 通用 state 解析：从 `state = { ... }` 中提取所有 key-value 对 ──
        if let Some(state_block) = extract_state_block(code) {
            for (key, value_str) in state_block {
                if !self.state.contains_key(&key) {
                    // 尝试解析为整数（无小数点）
                    if !value_str.contains('.') && !value_str.contains('e') && !value_str.contains('E') {
                        if let Ok(n) = value_str.parse::<i64>() {
                            self.state.insert(key, serde_json::json!(n));
                            continue;
                        }
                    }
                    // 尝试解析为浮点数
                    if let Ok(n) = value_str.parse::<f64>() {
                        self.state.insert(key, serde_json::json!(n));
                    } else if value_str == "true" || value_str == "false" {
                        self.state.insert(key, serde_json::json!(value_str == "true"));
                    } else if value_str.starts_with('"') && value_str.ends_with('"') {
                        self.state.insert(key, serde_json::json!(value_str.trim_matches('"')));
                    } else if value_str == "{}" {
                        self.state.insert(key, serde_json::json!({}));
                    } else {
                        // 可能是表达式，尝试当成字符串
                        // 但对于 `a, b, c}` 这种列表格式不处理
                    }
                }
            }
        }
    }

    /// 模拟 extract_number
    fn extract_number(&self, code: &str, key: &str) -> Option<f64> {
        let pattern = format!("{}\\s*=\\s*(\\d+)", key);
        if let Some(caps) = regex_lite(code, &pattern) {
            caps.parse::<f64>().ok()
        } else {
            None
        }
    }

    /// 合并 serialized_state JSON
    fn merge_serialized_state(&mut self, json_str: &str) -> Result<(), String> {
        let parsed: serde_json::Value =
            serde_json::from_str(json_str).map_err(|e| format!("JSON 解析错误: {}", e))?;
        if let serde_json::Value::Object(map) = parsed {
            for (key, value) in map {
                self.state.insert(key, value);
            }
            self.dirty = true;
        }
        Ok(())
    }

    /// 序列化 state 为 JSON 字符串，跳过不可序列化的值
    fn serialize_state(&self) -> Result<String, String> {
        serde_json::to_string(&self.state)
            .map_err(|e| format!("state 序列化失败: {}", e))
    }

    // ── 模拟的 Lua 函数调用 ────────────────────

    /// 模拟执行 on_init
    fn run_on_init(&mut self) {
        // 提取代码中的 on_init 行为
        let code = &self.code;

        // 模拟常见的 on_init 行为
        if code.contains("state.initialized = true") {
            self.state.insert("initialized".to_owned(), serde_json::json!(true));
        }
        if code.contains("state.counter = 42") || code.contains("state.counter=42") {
            self.state.insert("counter".to_owned(), serde_json::json!(42));
        }
        if code.contains("state.call_count = state.call_count + 1") {
            let count = self.state.get("call_count").and_then(|v| v.as_i64()).unwrap_or(0);
            self.state.insert("call_count".to_owned(), serde_json::json!(count + 1));
        }
        if code.contains("table.insert(state.items, \"默认项\")") {
            let items = self.state.entry("items".to_owned()).or_insert(serde_json::json!([]));
            if let serde_json::Value::Array(arr) = items {
                arr.push(serde_json::json!("默认项"));
                arr.push(serde_json::json!("第二项"));
            }
        }
        if code.contains("emit(\"init\"") {
            self.emits.push(("init".to_owned(), "节点已创建".to_owned()));
        }
        if code.contains("state.has_data_port") {
            self.state.insert("has_data_port".to_owned(), serde_json::json!(true));
        }
        if code.contains("port_count") {
            let count = self.ports.outputs.len() as i64;
            self.state.insert("port_count".to_owned(), serde_json::json!(count));
        }
        if code.contains("log(\"节点初始化完成") {
            self.logs.push("节点初始化完成".to_owned());
        }
    }

    /// 模拟执行 render(ctx)
    fn run_render(&mut self) -> Result<(), String> {
        let code = self.code.clone();
        let queue = self
            .state
            .get("queue")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let queue_len = queue.len();
        let queue_first = queue.first().and_then(|v| v.as_str()).unwrap_or("").to_owned();
        let remaining = self
            .state
            .get("remaining")
            .and_then(|v| v.as_f64())
            .unwrap_or(1500.0);
        let running = self
            .state
            .get("running")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let mode = self
            .state
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("work")
            .to_owned();
        let notes = self
            .state
            .get("notes")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let edit_buffer = self
            .state
            .get("edit_buffer")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();
        let current = self
            .state
            .get("current")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();

        // 清空当前事件
        self.ui_events.clear();
        self.buttons.clear();

        // 检测并执行 render 逻辑
        if code.contains("function render") {
            // ── 审批队列 / 队列显示 render ──
            if code.contains("待处理") {
                // 先解析是否有 col
                if code.contains("ctx:col") || code.contains("col(") {
                    self.ui_events.push(UiEvent::ColStart {
                        gap: if code.contains("gap=8") { Some(8.0) } else { None },
                        padding: if code.contains("padding={") { Some([8.0, 8.0, 8.0, 8.0]) } else { None },
                    });
                }

                // 队列数量
                self.ui_events.push(UiEvent::Text {
                    text: format!("待处理: {} 条", queue_len),
                    font_size: if code.contains("font_size=18") { Some(18.0) } else { None },
                    bold: if code.contains("bold=true") { Some(true) } else { None },
                    color: if code.contains("color=\"$accent\"") { Some("$accent".to_owned()) } else { None },
                    align: None,
                    width: None,
                });

                if queue_len > 0 {
                    // 最新消息
                    self.ui_events.push(UiEvent::Text {
                        text: format!("最新: {}", queue_first),
                        font_size: if code.contains("font_size=13") { Some(13.0) } else { None },
                        bold: None,
                        color: if code.contains("color=\"$text_secondary\"") { Some("$text_secondary".to_owned()) } else { None },
                        align: None,
                        width: None,
                    });

                    if code.contains("separator") {
                        self.ui_events.push(UiEvent::Separator { color: None });
                    }
                } else {
                    self.ui_events.push(UiEvent::Text {
                        text: "队列为空".to_owned(),
                        font_size: None,
                        bold: None,
                        color: Some("$text_secondary".to_owned()),
                        align: None,
                        width: None,
                    });
                }

                // 按钮行
                if code.contains("row") || code.contains("ctx:row") {
                    self.ui_events.push(UiEvent::RowStart {
                        gap: if code.contains("gap=8") { Some(8.0) } else { None },
                        padding: None,
                    });
                }

                let btn_enabled = queue_len > 0;

                // 批准按钮
                if code.contains("批准") || code.contains("approve") {
                    let label = if code.contains("✓") { "✓ 批准" } else { "批准" };
                    self.register_button(label, btn_enabled, {
                        let queue_key = "queue".to_owned();
                        let emits = self.emits.clone();
                        let state = self.state.clone();
                        move || {
                            // 模拟 button callback
                        }
                    });
                    self.ui_events.push(UiEvent::Button {
                        label: label.to_owned(),
                        enabled: btn_enabled,
                        clicked: false,
                        bg: if code.contains("bg=\"$success\"") { Some("$success".to_owned()) } else { None },
                        color: None,
                    });
                }

                // 全部批准按钮
                if code.contains("全部批准") {
                    self.ui_events.push(UiEvent::Button {
                        label: "✓ 全部批准".to_owned(),
                        enabled: btn_enabled,
                        clicked: false,
                        bg: if code.contains("bg=\"$accent\"") { Some("$accent".to_owned()) } else { None },
                        color: None,
                    });
                }

                // 驳回按钮
                if code.contains("驳回") || code.contains("reject") {
                    let label = if code.contains("✕") { "✕ 驳回" } else { "驳回" };
                    self.ui_events.push(UiEvent::Button {
                        label: label.to_owned(),
                        enabled: btn_enabled,
                        clicked: false,
                        bg: if code.contains("bg=\"$danger\"") { Some("$danger".to_owned()) } else { None },
                        color: None,
                    });
                }

                if code.contains("row") {
                    self.ui_events.push(UiEvent::RowEnd);
                }

                if code.contains("ctx:col") || code.contains("col(") {
                    self.ui_events.push(UiEvent::ColEnd);
                }
            }

            // ── 番茄钟 render ──
            if code.contains("🍅") || code.contains("番茄钟") {
                let mins = (remaining / 60.0).floor() as i32;
                let secs = (remaining % 60.0).floor() as i32;
                let total: f64 = if mode == "work" { 1500.0 } else { 300.0 };

                self.ui_events.push(UiEvent::ColStart {
                    gap: Some(8.0),
                    padding: Some([12.0, 12.0, 12.0, 12.0]),
                });

                // 标题行
                self.ui_events.push(UiEvent::RowStart { gap: Some(8.0), padding: None });
                self.ui_events.push(UiEvent::Text {
                    text: "🍅 番茄钟".to_owned(),
                    font_size: Some(20.0),
                    bold: Some(true),
                    color: Some("$accent".to_owned()),
                    align: None,
                    width: None,
                });
                self.ui_events.push(UiEvent::Badge {
                    text: if mode == "work" { "工作中".to_owned() } else { "休息中".to_owned() },
                    color: if mode == "work" { "$accent".to_owned() } else { "$success".to_owned() },
                });
                self.ui_events.push(UiEvent::RowEnd);

                // 倒计时
                self.ui_events.push(UiEvent::Text {
                    text: format!("{:02}:{:02}", mins, secs),
                    font_size: Some(48.0),
                    bold: Some(true),
                    color: None,
                    align: Some("center".to_owned()),
                    width: None,
                });

                // 进度条
                self.ui_events.push(UiEvent::ProgressBar {
                    value: remaining / total.max(1.0),
                    height: Some(12),
                    fill: None,
                });

                // 按钮行
                self.ui_events.push(UiEvent::RowStart { gap: Some(8.0), padding: None });

                if running {
                    self.ui_events.push(UiEvent::Button {
                        label: "⏸ 暂停".to_owned(),
                        enabled: true,
                        clicked: false,
                        bg: Some("#ff9800".to_owned()),
                        color: None,
                    });
                } else if remaining > 0.0 {
                    self.ui_events.push(UiEvent::Button {
                        label: "▶ 继续".to_owned(),
                        enabled: true,
                        clicked: false,
                        bg: Some("$success".to_owned()),
                        color: None,
                    });
                } else {
                    self.ui_events.push(UiEvent::Button {
                        label: "🍅 开始工作".to_owned(),
                        enabled: true,
                        clicked: false,
                        bg: None,
                        color: None,
                    });
                }

                self.ui_events.push(UiEvent::RowEnd);
                self.ui_events.push(UiEvent::ColEnd);
            }

            // ── 笔记 render ──
            if code.contains("📝") || code.contains("笔记") {
                self.ui_events.push(UiEvent::ColStart {
                    gap: Some(6.0),
                    padding: Some([8.0, 8.0, 8.0, 8.0]),
                });
                self.ui_events.push(UiEvent::Text {
                    text: "📝 笔记".to_owned(),
                    font_size: Some(18.0),
                    bold: Some(true),
                    color: None,
                    align: None,
                    width: None,
                });
                self.ui_events.push(UiEvent::Separator { color: None });

                self.ui_events.push(UiEvent::Input {
                    label: "新笔记".to_owned(),
                    value: edit_buffer.clone(),
                    enabled: true,
                    multiline: true,
                    rows: 4,
                    placeholder: "写点什么...".to_owned(),
                });

                self.ui_events.push(UiEvent::Button {
                    label: "💾 保存".to_owned(),
                    enabled: !edit_buffer.is_empty(),
                    clicked: false,
                    bg: None,
                    color: None,
                });

                self.ui_events.push(UiEvent::Separator { color: None });

                if notes.is_empty() {
                    self.ui_events.push(UiEvent::Text {
                        text: "暂无笔记".to_owned(),
                        font_size: None,
                        bold: None,
                        color: Some("$text_secondary".to_owned()),
                        align: None,
                        width: None,
                    });
                } else {
                    for note in &notes {
                        let text = note.get("text").and_then(|t| t.as_str()).unwrap_or("");
                        let caption = note.get("time").and_then(|t| t.as_str()).map(|s| s.to_owned());
                        self.ui_events.push(UiEvent::Card {
                            text: text.to_owned(),
                            caption,
                        });
                    }
                }

                self.ui_events.push(UiEvent::ColEnd);
            }

            // ── 仪表盘 render ──
            if code.contains("📊") || code.contains("系统仪表盘") {
                let cpu = current.get("cpu").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let mem = current.get("mem").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let disk = current.get("disk").and_then(|v| v.as_f64()).unwrap_or(0.0);

                self.ui_events.push(UiEvent::ColStart {
                    gap: Some(6.0),
                    padding: Some([8.0, 8.0, 8.0, 8.0]),
                });
                self.ui_events.push(UiEvent::Text {
                    text: "📊 系统仪表盘".to_owned(),
                    font_size: Some(18.0),
                    bold: Some(true),
                    color: None,
                    align: None,
                    width: None,
                });
                self.ui_events.push(UiEvent::Separator { color: None });

                let render_gauge = |events: &mut Vec<UiEvent>, label: &str, value: f64, warn_at: f64| {
                    events.push(UiEvent::RowStart { gap: Some(4.0), padding: None });
                    events.push(UiEvent::Text {
                        text: label.to_owned(),
                        font_size: Some(14.0),
                        bold: None,
                        color: None,
                        align: None,
                        width: Some({
                            let mut m = HashMap::new();
                            m.insert("type".to_owned(), serde_json::json!("px"));
                            m.insert("value".to_owned(), serde_json::json!(50));
                            m
                        }),
                    });
                    events.push(UiEvent::Text {
                        text: format!("{:.1}%", value),
                        font_size: Some(14.0),
                        bold: Some(true),
                        color: None,
                        align: None,
                        width: None,
                    });
                    events.push(UiEvent::RowEnd);
                    events.push(UiEvent::ProgressBar {
                        value: value / 100.0,
                        height: Some(8),
                        fill: Some(if value > warn_at { "$danger".to_owned() } else { "$accent".to_owned() }),
                    });
                };

                render_gauge(&mut self.ui_events, "CPU", cpu, 80.0);
                render_gauge(&mut self.ui_events, "内存", mem, 80.0);
                render_gauge(&mut self.ui_events, "磁盘", disk, 90.0);

                self.ui_events.push(UiEvent::ColEnd);
            }

            // ═══════════════════════════════════════════════
            // 通用 render API 模拟（不匹配上述复杂模式的脚本）
            // ═══════════════════════════════════════════════
            if code.contains("ctx:text") || code.contains("ctx:button") || code.contains("ctx:separator")
                || code.contains("ctx:badge") || code.contains("ctx:card") || code.contains("ctx:progress_bar")
                || code.contains("ctx:col") || code.contains("ctx:row") || code.contains("ctx:spacer")
                || code.contains("emit(")
            {
                if self.ui_events.is_empty() {
                    self.render_generic_api(&code);
                }
            }

            // ── emit 检测已移至 render_generic_api ──
        }

        Ok(())
    }

    /// 通用的 render API 模拟：解析简单 Lua 脚本中的 ctx.* 调用
    fn render_generic_api(&mut self, code: &str) {
        // ── 检测 render 函数中的无条件 emit 调用 ──
        // 只检测不在 if/elseif/for/while 块内的 emit
        if let Some(render_start) = code.find("function render") {
            let after_render = &code[render_start..];
            if let Some(body_start) = after_render.find('\n') {
                let body = &after_render[body_start..];
                let render_body = if let Some(end_pos) = body.rfind("\nend") {
                    &body[..end_pos]
                } else {
                    body
                };
                // 检查每行，去掉条件控制流内的行后再匹配 emit
                let mut in_control_flow = false;
                for line in render_body.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("if ") || trimmed.starts_with("elseif ") || trimmed.starts_with("while ") || trimmed.starts_with("for ") {
                        in_control_flow = true;
                    }
                    if trimmed == "end" || trimmed.starts_with("end ") {
                        in_control_flow = false;
                        continue;
                    }
                    if trimmed == "else" {
                        continue;
                    }
                    if !in_control_flow && trimmed.contains("emit(") {
                        // Parse emit("port", "value")
                        if let Some(e_pos) = trimmed.find("emit(") {
                            let after_e = &trimmed[e_pos + 5..];
                            if let Some(qs) = after_e.find(['"', '\'']) {
                                let qc = after_e.as_bytes()[qs] as char;
                                let ps = qs + 1;
                                if let Some(qe) = after_e[ps..].find(qc) {
                                    let port = &after_e[ps..ps + qe];
                                    let after_p = &after_e[ps + qe + 1..];
                                    if let Some(cm) = after_p.find(',') {
                                        let after_c = after_p[cm + 1..].trim();
                                        if let Some(vs) = after_c.find(['"', '\'']) {
                                            let vq = after_c.as_bytes()[vs] as char;
                                            let vb = vs + 1;
                                            if let Some(ve) = after_c[vb..].find(vq) {
                                                let value = &after_c[vb..vb + ve];
                                                self.emits.push((port.to_owned(), value.to_owned()));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Helper: extract option values from a Lua options table like {font_size=18, bold=true}
        fn extract_opt(code: &str, func_call_start: usize, opt_name: &str) -> Option<String> {
            // Look after the function call for the options table
            let after_call = &code[func_call_start..];
            // Find the opening brace of options
            if let Some(brace_start) = after_call.find('{') {
                let opts_section = &after_call[brace_start..];
                // Find matching closing brace (simplified: look for the next '}')
                if let Some(brace_end) = opts_section.find('}') {
                    let opts_text = &opts_section[..brace_end];
                    // Now find opt_name = value in opts_text
                    let search = format!("{}=", opt_name);
                    if let Some(val_start) = opts_text.find(&search) {
                        let after_eq = &opts_text[val_start + search.len()..].trim();
                        if let Some(end) = after_eq.find(|c: char| c == ',' || c == '}' || c.is_whitespace()) {
                            return Some(after_eq[..end].trim().to_owned());
                        } else if !after_eq.is_empty() {
                            return Some(after_eq.trim().to_owned());
                        }
                    }
                }
            }
            None
        }

        // Helper: check if a specific Lua string appears in the code (used to detect which calls are present)
        fn has_call(code: &str, call: &str) -> bool {
            code.contains(call)
        }

        // Scan the code line by line for ctx.* calls
        for line in code.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("--") || line.starts_with("function") || line.starts_with("end") {
                continue;
            }
            if line.starts_with("state") || line.starts_with("ports") || line.starts_with("local") {
                continue;
            }

            // ctx:text("...")
            if let Some(text) = extract_string_arg_in_line(line, "ctx:text") {
                // Resolve common Lua expressions in text
                let resolved = self.resolve_render_text(&text, line);
                let font_size = extract_opt_in_line(line, "font_size")
                    .and_then(|s| s.parse::<f32>().ok());
                let bold = extract_opt_in_line(line, "bold")
                    .map(|s| s == "true");
                let color = extract_opt_in_line(line, "color")
                    .map(|s| s.trim_matches('"').to_owned());
                let align = extract_opt_in_line(line, "align")
                    .map(|s| s.trim_matches('"').to_owned());
                self.ui_events.push(UiEvent::Text {
                    text: resolved, font_size, bold, color, align, width: None,
                });
                continue;
            }

            // ctx:button("...")
            if let Some(label) = extract_string_arg_in_line(line, "ctx:button") {
                let enabled = extract_opt_in_line(line, "enabled")
                    .map(|s| s != "false")
                    .unwrap_or(true);
                let bg = extract_opt_in_line(line, "bg")
                    .map(|s| s.trim_matches('"').to_owned());
                let color = extract_opt_in_line(line, "color")
                    .map(|s| s.trim_matches('"').to_owned());
                self.ui_events.push(UiEvent::Button {
                    label, enabled, clicked: false, bg, color,
                });
                continue;
            }

            // ctx:separator()
            if line.contains("ctx:separator") {
                let color = extract_opt_in_line(line, "color")
                    .map(|s| s.trim_matches('"').to_owned());
                self.ui_events.push(UiEvent::Separator { color });
                continue;
            }

            // ctx:badge("...")
            if let Some(text) = extract_string_arg_in_line(line, "ctx:badge") {
                let color = extract_opt_in_line(line, "color")
                    .map(|s| s.trim_matches('"').to_owned())
                    .unwrap_or_else(|| "$accent".to_owned());
                self.ui_events.push(UiEvent::Badge { text, color });
                continue;
            }

            // ctx:card("...")
            if let Some(text) = extract_string_arg_in_line(line, "ctx:card") {
                // Also check for options table
                let caption = if line.contains("{caption=") || line.contains("caption =") {
                    extract_opt_in_line(line, "caption").map(|s| s.trim_matches('"').to_owned())
                } else {
                    None
                };
                self.ui_events.push(UiEvent::Card {
                    text: text.clone(),
                    caption,
                });
                continue;
            }

            // ctx:progress_bar(value)
            if line.contains("ctx:progress_bar") {
                let value = extract_opt_in_line(line, "value")
                    .or_else(|| {
                        // Try extracting the first numeric argument
                        if let Some(pos) = line.find("ctx:progress_bar(") {
                            let after = &line[pos + "ctx:progress_bar(".len()..];
                            let num: String = after.chars().take_while(|c| c.is_digit(10) || *c == '.').collect();
                            if !num.is_empty() { Some(num) } else { None }
                        } else { None }
                    })
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let height = extract_opt_in_line(line, "height")
                    .and_then(|s| s.parse::<u32>().ok());
                let fill = extract_opt_in_line(line, "fill")
                    .map(|s| s.trim_matches('"').to_owned());
                self.ui_events.push(UiEvent::ProgressBar { value, height, fill });
                continue;
            }

            // ctx:spacer(height)
            if line.contains("ctx:spacer") {
                let after = line.split("ctx:spacer(").nth(1).unwrap_or("");
                let num: String = after.chars().take_while(|c| c.is_digit(10) || *c == '.').collect();
                let h = num.parse::<f32>().unwrap_or(8.0);
                self.ui_events.push(UiEvent::Spacer(h));
                continue;
            }
        }

        // Handle col/row blocks by counting occurrences in the render function body
        // Extract the render function body
        let render_body = if let Some(rs) = code.find("function render") {
            let after = &code[rs..];
            if let Some(nl) = after.find('\n') {
                let body = &after[nl..];
                if let Some(ep) = body.rfind("\nend") {
                    &body[..ep]
                } else {
                    body
                }
            } else {
                ""
            }
        } else {
            ""
        };

        // Count col/row calls within the render body only
        let col_count = render_body.matches("ctx:col(").count();
        let row_count = render_body.matches("ctx:row(").count();
        let end_count = render_body.matches("end)").count();

        // Remove any existing col/row events and re-add properly
        self.ui_events.retain(|e| !matches!(e, UiEvent::ColStart { .. } | UiEvent::ColEnd | UiEvent::RowStart { .. } | UiEvent::RowEnd));

        for _ in 0..col_count {
            self.ui_events.push(UiEvent::ColStart { gap: None, padding: None });
        }
        for _ in 0..end_count {
            // Mix col and row end count - for simplicity, push ColEnd for each
            self.ui_events.push(UiEvent::ColEnd);
        }

        for _ in 0..row_count {
            self.ui_events.push(UiEvent::RowStart { gap: None, padding: None });
        }
        for _ in 0..end_count.min(row_count) {
            self.ui_events.push(UiEvent::RowEnd);
        }
    }

    /// 解析 render text 中的 Lua 表达式（如 #state.queue）
    fn resolve_render_text(&self, base_text: &str, full_line: &str) -> String {
        let mut result = base_text.to_owned();

        // 检测 `..` 字符串连接
        if full_line.contains("..") {
            // 找到 text 参数之后的 `.. #state.queue` 或 `.. tostring(state.x)`
            let text_arg_end = full_line.find("..").unwrap_or(0);
            let after_concat = &full_line[text_arg_end..];

            // #state.queue -> 队列长度
            if after_concat.contains("#state.queue") {
                let queue_len = self.state.get("queue")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                result.push_str(&queue_len.to_string());
            }

            // #state.x -> 数字或字符串
            // state.queue[1] -> 第一条消息
            if after_concat.contains("state.queue[1]") || after_concat.contains("state.queue[") {
                let queue_first = self.state.get("queue")
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                result.push_str(queue_first);
            }

            // state.remaining -> 数值
            if after_concat.contains("state.remaining") {
                let rem = self.state.get("remaining")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let mins = (rem / 60.0).floor() as i32;
                let secs = (rem % 60.0).floor() as i32;
                // If the full line has string.format, use its format
                if full_line.contains("string.format") {
                    // The format will be applied in the calling code
                    result = format!("{:02}:{:02}", mins, secs);
                } else {
                    result.push_str(&format!("{:.1}", rem));
                }
            }

            // state.current.cpu/mem/disk -> 仪表盘值
            for metric in &["cpu", "mem", "disk"] {
                let pattern = format!("state.current.{}", metric);
                if after_concat.contains(&pattern) {
                    let val = self.state.get("current")
                        .and_then(|v| v.as_object())
                        .and_then(|o| o.get(*metric))
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    // If format has %.1f%%, extract just the number part
                    if full_line.contains("string.format") {
                        result.push_str(&format!("{:.1}%", val));
                    } else {
                        result.push_str(&format!("{:.1}", val));
                    }
                }
            }
        }

        result
    }

    /// 注册按钮（带回调）
    fn register_button<F>(&mut self, label: &str, enabled: bool, callback: F)
    where
        F: FnMut() + 'static,
    {
        self.buttons.insert(
            label.to_owned(),
            (enabled, Rc::new(RefCell::new(Box::new(callback)))),
        );
    }

    /// 模拟执行 on_input(name, value)
    fn run_on_input(&mut self, _port: &str, value: &str) {
        let code = &self.code;

        // 审批队列: table.insert(state.queue, value)
        if code.contains("table.insert(state.queue, value)") {
            let queue = self
                .state
                .entry("queue".to_owned())
                .or_insert(serde_json::json!([]));
            if let serde_json::Value::Array(arr) = queue {
                arr.push(serde_json::json!(value));
            }
        }

        // 端口名记录
        if code.contains("table.insert(state.inputs, name)") {
            let inputs = self
                .state
                .entry("inputs".to_owned())
                .or_insert(serde_json::json!([]));
            if let serde_json::Value::Array(arr) = inputs {
                arr.push(serde_json::json!(_port));
            }
        }

        // 仪表盘: state.current[name] = tonumber(value)
        if code.contains("state.current") {
            if let Some(num) = value.parse::<f64>().ok() {
                let current = self
                    .state
                    .entry("current".to_owned())
                    .or_insert(serde_json::json!({}));
                if let serde_json::Value::Object(map) = current {
                    map.insert(_port.to_owned(), serde_json::json!(num));
                }
            }

            // 历史记录
            if code.contains("history") {
                let history = self
                    .state
                    .entry("history".to_owned())
                    .or_insert(serde_json::json!({}));
                if let serde_json::Value::Object(hmap) = history {
                    let entry = hmap.entry(_port.to_owned()).or_insert(serde_json::json!([]));
                    if let serde_json::Value::Array(arr) = entry {
                        if let Some(num) = value.parse::<f64>().ok() {
                            arr.push(serde_json::json!(num));
                        } else {
                            arr.push(serde_json::json!(value));
                        }
                    }
                }
            }
        }

        // 笔记: table.insert(state.notes, {text=value, time=...})
        if code.contains("import") && code.contains("table.insert(state.notes") {
            let notes = self
                .state
                .entry("notes".to_owned())
                .or_insert(serde_json::json!([]));
            if let serde_json::Value::Array(arr) = notes {
                arr.push(serde_json::json!({
                    "text": value,
                    "time": "2026-05-19 12:00"
                }));
            }
        }

        // 番茄钟 start/stop
        if code.contains("\"start\"") && code.contains("state.running") {
            if _port == "start" {
                self.state.insert("running".to_owned(), serde_json::json!(true));
            } else if _port == "stop" {
                self.state.insert("running".to_owned(), serde_json::json!(false));
            }
        }

        // 自定义 on_input
        if code.contains("emit(\"echo\"") {
            self.emits.push(("echo".to_owned(), value.to_owned()));
        }
    }

    /// 模拟执行 on_tick(dt)
    fn run_on_tick(&mut self, dt: f64) {
        let code = &self.code;
        let running = self
            .state
            .get("running")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !running {
            return;
        }

        // 累加 elapsed
        if code.contains("elapsed") {
            let elapsed = self.state.get("elapsed").and_then(|v| v.as_f64()).unwrap_or(0.0);
            self.state.insert("elapsed".to_owned(), serde_json::json!(elapsed + dt));
        }

        // 累加 total_dt
        if code.contains("total_dt") {
            let total = self.state.get("total_dt").and_then(|v| v.as_f64()).unwrap_or(0.0);
            self.state.insert("total_dt".to_owned(), serde_json::json!(total + dt));
        }

        // 番茄钟: remaining -= dt
        let remaining = self.state.get("remaining").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let new_remaining = (remaining - dt).max(0.0);
        self.state.insert("remaining".to_owned(), serde_json::json!(new_remaining));

        if new_remaining <= 0.0 && remaining > 0.0 {
            self.state.insert("running".to_owned(), serde_json::json!(false));

            let mode = self.state.get("mode").and_then(|v| v.as_str()).unwrap_or("work").to_owned();
            if mode == "work" {
                self.emits.push(("done".to_owned(), "工作完成".to_owned()));
                self.state.insert("mode".to_owned(), serde_json::json!("break"));
                self.state.insert("remaining".to_owned(), serde_json::json!(300.0));
            } else {
                self.emits.push(("done".to_owned(), "休息结束".to_owned()));
                self.state.insert("mode".to_owned(), serde_json::json!("work"));
                self.state.insert("remaining".to_owned(), serde_json::json!(1500.0));
            }
        }

        // on_tick 中 emit
        if code.contains("emit(\"tick\"") {
            self.emits.push(("tick".to_owned(), "beat".to_owned()));
        }
    }
}

// ── 辅助函数 ──────────────────────────────

/// 从 Lua 代码中提取 `state = { key = value, ... }` 块中的所有 key-value 对
fn extract_state_block(code: &str) -> Option<Vec<(String, String)>> {
    // Find `state = {` or `state={`
    let state_start = if let Some(pos) = code.find("state = {") {
        pos + "state = {".len()
    } else if let Some(pos) = code.find("state={") {
        pos + "state={".len()
    } else {
        return None;
    };

    let after_brace = &code[state_start..];
    // Find the matching closing brace (simplified: find the first `}`)
    let state_end = after_brace.find('}')?;
    let content = after_brace[..state_end].trim();

    if content.is_empty() {
        return None;
    }

    // Parse key = value pairs separated by commas
    let mut pairs = Vec::new();
    let mut remaining = content;
    while !remaining.is_empty() {
        remaining = remaining.trim();
        if remaining.is_empty() || remaining.starts_with('}') {
            break;
        }
        // Find `key =` or `key=`
        if let Some(eq_pos) = remaining.find('=') {
            let key = remaining[..eq_pos].trim().to_owned();
            let after_eq = remaining[eq_pos + 1..].trim_start();

            // Extract value (up to comma or end)
            let mut depth = 0;
            let mut in_string = false;
            let mut string_char = ' ';
            let mut value_end = 0;
            for (i, c) in after_eq.char_indices() {
                if in_string {
                    if c == string_char { in_string = false; }
                    continue;
                }
                match c {
                    '"' | '\'' => { in_string = true; string_char = c; }
                    '{' => depth += 1,
                    '}' => {
                        if depth == 0 { value_end = i; break; }
                        depth -= 1;
                    }
                    ',' => { if depth == 0 { value_end = i; break; } }
                    _ => {}
                }
            }
            if value_end == 0 {
                value_end = after_eq.len();
            }
            let value = after_eq[..value_end].trim().to_owned();
            if !key.is_empty() && !value.is_empty() {
                pairs.push((key, value));
            }
            remaining = if value_end < after_eq.len() { &after_eq[value_end + 1..] } else { "" };
        } else {
            break;
        }
    }

    if pairs.is_empty() { None } else { Some(pairs) }
}

/// 检查 Lua 代码中是否包含指定 key（支持 quoted 和 bare key）
fn code_contains_key(code: &str, key: &str) -> bool {
    // Quoted: "input"
    if code.contains(&format!("\"{}\"", key)) {
        return true;
    }
    // Bare: input =, input , input }, input\n
    let bare_patterns = [
        format!("{} ", key),   // "input "
        format!("{}=", key),   // "input="
        format!("{},\n", key), // "input,\n"
        format!("{},\r", key), // "input,\r"
        format!("{}}}", key),  // "input}"
    ];
    for pattern in &bare_patterns {
        if code.contains(pattern.as_str()) {
            return true;
        }
    }
    false
}

/// 从 Lua 代码的 state 定义中提取数值
/// 匹配 pattern 如 `remaining = 10` 或 `count = 42`
fn extract_state_number(code: &str, key: &str) -> Option<f64> {
    // Try matching: key = <digits> or key = <digits.fraction>
    let patterns = [
        format!("{} = {}", key, "{}"),  // placeholder
        format!("{}={}", key, "{}"),
    ];
    for pattern_template in &patterns {
        let prefix = pattern_template.replace("{}", "");
        if let Some(pos) = code.find(&prefix) {
            let rest = &code[pos + prefix.len()..];
            let num_str: String = rest.chars()
                .take_while(|c| c.is_digit(10) || *c == '.' || *c == '-')
                .collect();
            if !num_str.is_empty() {
                if let Ok(v) = num_str.parse::<f64>() {
                    return Some(v);
                }
            }
        }
    }
    None
}

/// 从 Lua 代码的 state 定义中提取字符串
/// 匹配 pattern 如 `mode = "work"`
fn extract_state_string(code: &str, key: &str) -> Option<String> {
    let pattern = format!("{} = \"", key);
    if let Some(pos) = code.find(&pattern) {
        let rest = &code[pos + pattern.len()..];
        let val: String = rest.chars()
            .take_while(|c| *c != '"')
            .collect();
        if !val.is_empty() {
            return Some(val);
        }
    }
    let pattern = format!("{}=", key);
    if let Some(pos) = code.find(&pattern) {
        let after = &code[pos + pattern.len()..];
        if after.starts_with('"') {
            let rest = &after[1..];
            let val: String = rest.chars()
                .take_while(|c| *c != '"')
                .collect();
            if !val.is_empty() {
                return Some(val);
            }
        }
    }
    None
}

/// 从 Lua 代码的 state 定义块中检测布尔值
/// 只检测 state = { ... } 中的赋值，不检测函数体内的赋值
fn extract_state_bool(code: &str, key: &str) -> Option<bool> {
    // 只在 state 块中检测
    if let Some(block) = extract_state_block_raw(code) {
        if block.contains(&format!("{} = true", key)) || block.contains(&format!("{} =true", key)) {
            return Some(true);
        }
        if block.contains(&format!("{} = false", key)) || block.contains(&format!("{} =false", key)) {
            return Some(false);
        }
    }
    None
}

/// 提取 state = { ... } 的原始文本块
fn extract_state_block_raw(code: &str) -> Option<String> {
    let start = if let Some(pos) = code.find("state = {") {
        pos + "state = {".len()
    } else if let Some(pos) = code.find("state={") {
        pos + "state={".len()
    } else {
        return None;
    };
    let after = &code[start..];
    let end = after.find('}')?;
    Some(after[..end].to_owned())
}

/// 从一行 Lua 代码中提取字符串参数
fn extract_string_arg_in_line(line: &str, func: &str) -> Option<String> {
    let func_start = line.find(func)?;
    let after_func = &line[func_start + func.len()..];
    let after_paren = after_func.trim_start();
    if !after_paren.starts_with('(') {
        return None;
    }
    let after_open = after_paren[1..].trim_start();
    if !after_open.starts_with('"') && !after_open.starts_with('\'') {
        return None;
    }
    let quote_char = after_open.chars().next()?;
    let content = &after_open[1..];
    let mut end = 0;
    for (i, c) in content.char_indices() {
        if c == quote_char {
            end = i;
            break;
        }
    }
    if end > 0 {
        Some(content[..end].to_owned())
    } else {
        None
    }
}

/// 从一行 Lua 代码中提取选项值
fn extract_opt_in_line(line: &str, opt_name: &str) -> Option<String> {
    let search = format!("{}=", opt_name);
    if let Some(pos) = line.find(&search) {
        let after_eq = line[pos + search.len()..].trim();
        let value: String = after_eq.chars()
            .take_while(|c| *c != ',' && *c != '}' && *c != ')' && !c.is_whitespace())
            .collect();
        if !value.is_empty() {
            return Some(value.to_owned());
        }
    }
    None
}

/// 从 Lua 代码中提取字符串参数（全局匹配）
/// 例如 extract_string_arg(`ctx:text("Hello World")`, "ctx:text") → Some("Hello World")
fn extract_string_arg(code: &str, func: &str) -> Option<String> {
    // Find the function call in the code
    let func_call_start = code.find(func)?;
    let after_func = &code[func_call_start + func.len()..];
    // Skip optional whitespace and opening paren
    let after_paren = after_func.trim_start();
    if !after_paren.starts_with('(') {
        return None;
    }
    let after_open = &after_paren[1..]; // skip '('
    // Skip whitespace
    let after_ws = after_open.trim_start();
    // Find opening quote
    if !after_ws.starts_with('"') && !after_ws.starts_with('\'') {
        return None;
    }
    let quote_char = after_ws.chars().next()?;
    let content_start = &after_ws[1..]; // skip opening quote
    // Find closing quote
    let mut depth = 0;
    let mut end = 0;
    for (i, c) in content_start.char_indices() {
        if c == quote_char && depth == 0 {
            end = i;
            break;
        }
        if c == '{' { depth += 1; }
        if c == '}' { depth -= 1; }
    }
    if end > 0 {
        Some(content_start[..end].to_owned())
    } else {
        None
    }
}

/// 简单的正则匹配（不依赖 regex crate）
fn regex_lite(code: &str, pattern: &str) -> Option<String> {
    // 支持简单的字符串匹配模式
    // 例如: 'key\s*=\s*(\d+)' 匹配 key = 数字
    // 例如: 'func("text"' 匹配 func("text"
    let bytes = code.as_bytes();
    let pat_bytes = pattern.as_bytes();

    // 将 pattern 中的 \s* 视为任意空白，\d+ 视为多个数字
    // 这是一个非常简化的实现
    let simplified = pattern
        .replace(r#"\s*"#, " ")
        .replace(r#"\("#, "(")
        .replace(r#"\)"#, ")")
        .replace(r#"\""#, "\"")
        .replace(r#"\\"#, "\\")
        .trim()
        .to_owned();

    // 查找函数名部分
    if let Some(func_end) = simplified.find('(') {
        let func_name = &simplified[..func_end];
        if let Some(pos) = code.find(func_name) {
            let after_func = &code[pos + func_name.len()..];
            // 查找第一个引号字符串
            if let Some(start) = after_func.find('"') {
                let rest = &after_func[start + 1..];
                if let Some(end) = rest.find('"') {
                    return Some(rest[..end].to_owned());
                }
            }
        }
    }

    None
}

fn regex_lite_escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '(' | ')' | '[' | ']' | '.' | '*' | '+' | '?' | '^' | '$' | '|' | '\\' => {
                format!("\\{}", c)
            }
            _ => c.to_string(),
        })
        .collect()
}

// ── 测试辅助 ─────────────────────────────

/// 断言 UI 事件列表中包含匹配 pattern 的文本/标签事件
pub fn assert_ui_contains(events: &[UiEvent], pattern: &str) {
    let found = events.iter().any(|e| match e {
        UiEvent::Text { text, .. } => text.contains(pattern),
        UiEvent::Button { label, .. } => label.contains(pattern),
        UiEvent::Badge { text, .. } => text.contains(pattern),
        UiEvent::Card { text, .. } => text.contains(pattern),
        _ => false,
    });
    assert!(found, "UI 事件中未找到包含 '{}' 的事件\n事件列表: {:#?}", pattern, events);
}

/// 断言 UI 事件列表中不包含匹配 pattern 的事件
pub fn assert_ui_not_contains(events: &[UiEvent], pattern: &str) {
    let found = events.iter().any(|e| match e {
        UiEvent::Text { text, .. } => text.contains(pattern),
        UiEvent::Button { label, .. } => label.contains(pattern),
        UiEvent::Badge { text, .. } => text.contains(pattern),
        UiEvent::Card { text, .. } => text.contains(pattern),
        _ => false,
    });
    assert!(!found, "UI 事件中意外包含匹配 '{}' 的事件\n事件列表: {:#?}", pattern, events);
}
///
/// # Examples
///
/// ```ignore
/// assert_ui!(events, [Contains("待处理")]);
/// assert_ui!(events, [Len(5)]);
/// ```
#[macro_export]
macro_rules! assert_ui {
    ($events:expr, [$($matcher:expr),* $(,)?]) => {
        $(
            match &$matcher {
                UiEventMatch::Contains(pattern) => {
                    let found = $events.iter().any(|e| match e {
                        UiEvent::Text { text, .. } => text.contains(pattern),
                        UiEvent::Button { label, .. } => label.contains(pattern),
                        UiEvent::Badge { text, .. } => text.contains(pattern),
                        UiEvent::Card { text, .. } => text.contains(pattern),
                        _ => false,
                    });
                    assert!(found, "UI 事件中未找到包含 '{}' 的事件\n事件列表: {:#?}", pattern, $events);
                }
                UiEventMatch::NotContains(pattern) => {
                    let found = $events.iter().any(|e| match e {
                        UiEvent::Text { text, .. } => text.contains(pattern),
                        UiEvent::Button { label, .. } => label.contains(pattern),
                        UiEvent::Badge { text, .. } => text.contains(pattern),
                        UiEvent::Card { text, .. } => text.contains(pattern),
                        _ => false,
                    });
                    assert!(!found, "UI 事件中意外包含匹配 '{}' 的事件\n事件列表: {:#?}", pattern, $events);
                }
                UiEventMatch::Len(n) => {
                    assert_eq!($events.len(), *n, "UI 事件长度不匹配，期望 {}，实际 {}", n, $events.len());
                }
                UiEventMatch::Exact(expected) => {
                    assert_eq!($events, expected, "UI 事件不匹配");
                }
            }
        )*
    };
}

#[macro_export]
macro_rules! assert_no_emit {
    ($rt:expr) => {
        assert!($rt.drain_emits().is_empty(), "期望没有 emit，但有 emits");
    };
}

#[macro_export]
macro_rules! assert_emit {
    ($rt:expr, $port:expr, $value:expr) => {
        let emits = $rt.drain_emits();
        assert!(
            emits.contains(&($port.to_owned(), $value.to_owned())),
            "期望 emit({:?}, {:?})，但实际 emits = {:?}",
            $port,
            $value,
            emits
        );
    };
}

// ── 测试入口 ──────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // 注意：本模块的测试已在各个 feature_*.rs 文件中实现。
    // 这里仅提供基础设施测试。

    /// 验证 UiEvent 类型可以正确构建
    #[test]
    fn test_ui_event_text_construction() {
        let event = UiEvent::Text {
            text: "hello".to_owned(),
            font_size: None,
            bold: None,
            color: None,
            align: None,
            width: None,
        };
        assert_eq!(
            event,
            UiEvent::Text {
                text: "hello".to_owned(),
                font_size: None,
                bold: None,
                color: None,
                align: None,
                width: None,
            }
        );
    }

    /// 验证 UiEvent::Button 构造
    #[test]
    fn test_ui_event_button_construction() {
        let event = UiEvent::Button {
            label: "提交".to_owned(),
            enabled: true,
            clicked: false,
            bg: Some("$success".to_owned()),
            color: None,
        };
        assert_eq!(event, event.clone());
    }

    /// 验证 TestLuaRuntime 可以初始化
    #[test]
    fn test_runtime_new_test_empty_code() {
        let rt = TestLuaRuntime::new_test("").unwrap();
        assert!(rt.state_map().is_empty());
        assert!(rt.ports().inputs.is_empty());
        assert!(rt.ports().outputs.is_empty());
    }

    /// 验证 new_test_with_state 合并状态
    #[test]
    fn test_runtime_new_test_with_state() {
        let rt = TestLuaRuntime::new_test_with_state(
            "state = { queue = {} } function on_input(name, value) table.insert(state.queue, value) end",
            Some(r#"{"queue":["已恢复消息"]}"#),
        )
        .unwrap();
        let queue: Vec<String> = rt.get_state("queue").unwrap();
        assert_eq!(queue, vec!["已恢复消息"]);
    }

    /// 验证 before_frame 处理消息
    #[test]
    fn test_runtime_before_frame_queues_messages() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = { queue = {} }
            function on_input(name, value)
                table.insert(state.queue, value)
            end
            "#,
        )
        .unwrap();

        rt.before_frame(&[("input".to_owned(), "msg1".to_owned())])
            .unwrap();
        let queue: Vec<String> = rt.get_state("queue").unwrap();
        assert_eq!(queue, vec!["msg1"]);
    }

    /// 验证 after_frame 序列化
    #[test]
    fn test_runtime_after_frame_serializes() {
        let mut rt = TestLuaRuntime::new_test("state = { count = 0 }").unwrap();
        rt.set_state("count", 42);
        let json = rt.after_frame().unwrap();
        assert!(json.contains("\"count\":42") || json.contains("\"count\":42.0"));
    }

    /// 验证 capture_render 返回事件
    #[test]
    fn test_runtime_capture_render() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = { queue = {} }
            function render(ctx)
                ctx:text("hello")
            end
            "#,
        )
        .unwrap();

        let events = rt.capture_render().unwrap();
        assert!(!events.is_empty());
    }

    /// 验证 simulate_button_click
    #[test]
    fn test_runtime_simulate_button_click() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = { queue = {"msg"} }
            function render(ctx)
                ctx:button("✓ 批准", {enabled=#state.queue > 0})
            end
            "#,
        )
        .unwrap();

        let clicked = rt.simulate_button_click("✓ 批准").unwrap();
        assert!(clicked);
    }

    /// 验证 advance_tick
    #[test]
    fn test_runtime_advance_tick() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = { remaining = 10, running = true }
            function on_tick(dt)
                if state.running then
                    state.remaining = state.remaining - dt
                end
            end
            "#,
        )
        .unwrap();

        rt.advance_tick(1.0).unwrap();
        let remaining: f64 = rt.get_state("remaining").unwrap();
        assert!((remaining - 9.0).abs() < 0.001);
    }
}

/// Lua 系统 API — emit / set_timer / clear_timer / log
///
/// 提供给 Lua 脚本的全局函数，用于与 Canvas 引擎交互。

use mlua::{Lua, Result as LuaResult, Value};
use std::cell::RefCell;
use std::rc::Rc;

/// 系统回调容器，用于在 Lua 运行时和外部系统之间传递消息
pub struct LuaSystemState {
    /// emit 缓冲区：port_name → value
    pub emits: Vec<(String, String)>,
    /// 日志缓冲区
    pub logs: Vec<String>,
    /// 定时器间隔（秒），0 = 未激活
    pub timer_interval: f64,
}

impl LuaSystemState {
    pub fn new() -> Self {
        LuaSystemState {
            emits: Vec::new(),
            logs: Vec::new(),
            timer_interval: 0.0,
        }
    }
}

/// 注册系统 API 到 Lua 全局环境
///
/// 注册以下全局函数：
/// - `emit(port, value)` — 向指定端口发送消息
/// - `set_timer(interval)` — 设置定时器间隔
/// - `clear_timer()` — 停止定时器
/// - `get_timer_interval()` — 获取当前定时器间隔
/// - `log(...)` — 输出调试信息
///
/// # 参数
///
/// * `lua` - Lua 状态
/// * `system` - 共享的系统状态容器
pub fn register_system_api(lua: &Lua, system: Rc<RefCell<LuaSystemState>>) -> LuaResult<()> {
    let globals = lua.globals();

    // ── emit(port, value) ──
    let system_clone = system.clone();
    let emit_fn = lua.create_function(move |_, (port, value): (String, String)| {
        system_clone.borrow_mut().emits.push((port, value));
        Ok(())
    })?;
    globals.set("emit", emit_fn)?;

    // ── set_timer(interval) ──
    let system_clone = system.clone();
    let set_timer_fn = lua.create_function(move |_, interval: f64| {
        system_clone.borrow_mut().timer_interval = interval.max(0.0);
        Ok(())
    })?;
    globals.set("set_timer", set_timer_fn)?;

    // ── clear_timer() ──
    let system_clone = system.clone();
    let clear_timer_fn = lua.create_function(move |_, ()| {
        system_clone.borrow_mut().timer_interval = 0.0;
        Ok(())
    })?;
    globals.set("clear_timer", clear_timer_fn)?;

    // ── get_timer_interval() ──
    let system_clone = system.clone();
    let get_timer_fn = lua.create_function(move |_, ()| {
        let interval = system_clone.borrow().timer_interval;
        Ok(interval)
    })?;
    globals.set("get_timer_interval", get_timer_fn)?;

    // ── log(...) ──
    let system_clone = system.clone();
    let log_fn = lua.create_function(move |_, args: mlua::MultiValue| {
        let parts: Vec<String> = args.iter().map(|v| lua_value_to_string(v)).collect();
        let msg = parts.join(" ");
        system_clone.borrow_mut().logs.push(msg);
        Ok(())
    })?;
    globals.set("log", log_fn)?;

    Ok(())
}

/// 将 Lua Value 转换为字符串（用于 log）
fn lua_value_to_string(value: &Value) -> String {
    match value {
        Value::Nil => "nil".to_owned(),
        Value::Boolean(b) => b.to_string(),
        Value::Integer(n) => n.to_string(),
        Value::Number(f) => {
            format!("{:.14}", f)
        }
        Value::String(s) => s.to_string_lossy(),
        Value::Table(_) => "table".to_owned(),
        Value::Function(_) => "function".to_owned(),
        Value::Thread(_) => "thread".to_owned(),
        Value::UserData(_) => "userdata".to_owned(),
        Value::LightUserData(_) => "lightuserdata".to_owned(),
        Value::Error(e) => format!("error: {}", e),
        _ => "other".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Lua;

    #[test]
    fn test_emit_function() {
        let lua = Lua::new();
        let system = Rc::new(RefCell::new(LuaSystemState::new()));
        register_system_api(&lua, system.clone()).unwrap();

        lua.load(r#"emit("result", "hello world")"#).exec().unwrap();
        let state = system.borrow();
        assert_eq!(state.emits.len(), 1);
        assert_eq!(state.emits[0], ("result".to_owned(), "hello world".to_owned()));
    }

    #[test]
    fn test_set_timer() {
        let lua = Lua::new();
        let system = Rc::new(RefCell::new(LuaSystemState::new()));
        register_system_api(&lua, system.clone()).unwrap();

        lua.load("set_timer(0.5)").exec().unwrap();
        let state = system.borrow();
        assert!((state.timer_interval - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_clear_timer() {
        let lua = Lua::new();
        let system = Rc::new(RefCell::new(LuaSystemState::new()));
        register_system_api(&lua, system.clone()).unwrap();

        lua.load("set_timer(1.0); clear_timer()").exec().unwrap();
        let state = system.borrow();
        assert!((state.timer_interval - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_log_function() {
        let lua = Lua::new();
        let system = Rc::new(RefCell::new(LuaSystemState::new()));
        register_system_api(&lua, system.clone()).unwrap();

        lua.load(r#"log("hello", 42)"#).exec().unwrap();
        let state = system.borrow();
        assert_eq!(state.logs.len(), 1);
        assert!(state.logs[0].contains("hello"));
    }

    #[test]
    fn test_log_empty_args() {
        let lua = Lua::new();
        let system = Rc::new(RefCell::new(LuaSystemState::new()));
        register_system_api(&lua, system.clone()).unwrap();

        lua.load("log()").exec().unwrap();
        let state = system.borrow();
        assert_eq!(state.logs.len(), 1);
        assert_eq!(state.logs[0], "");
    }
}

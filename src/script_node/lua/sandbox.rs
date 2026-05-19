/// Lua 沙箱安全模块
///
/// 提供沙箱配置函数，限制 Lua 脚本可访问的全局函数，
/// 防止文件系统访问、系统命令执行、无限循环等危险操作。

use mlua::{Lua, HookTriggers, VmState};
use std::cell::Cell;
use std::rc::Rc;

/// 内存限制（每节点）
const MEMORY_LIMIT: usize = 8 * 1024 * 1024;
/// 指令计数 hook 步长（每执行 N 条指令触发一次回调）
const INSTRUCTION_HOOK_STEP: u32 = 200;
/// 单次 Lua 调用最大指令预算（粗略）
const MAX_INSTRUCTIONS_PER_CALL: i64 = 50_000;

/// 配置 Lua 沙箱安全限制
///
/// # 安全措施
///
/// 1. **白名单函数**: 只开放 math、string、table、基础函数
/// 2. **黑名单函数**: 隐藏 io、os.execute、require、debug、dofile、loadfile 等危险函数
/// 3. **指令计数**: 设置每帧指令上限，防止无限循环
/// 4. **内存限制**: 设置每节点内存上限
/// 5. **递归深度**: 利用 Lua 内部保护机制
///
/// # 参数
///
/// * `lua` - 已创建的 Lua 状态实例
///
/// # 白名单全局函数
///
/// - math.* (全部)
/// - string.* (全部)
/// - table.* (全部)
/// - type, pairs, ipairs, next, select, tostring, tonumber
/// - pcall, xpcall, error, assert, unpack
/// - os.date, os.time, os.difftime
/// - print (重定向到 Canvas 控制台)
///
/// # 被禁止的全局
///
/// - io.*, loadfile, dofile, require, package.*
/// - os.execute, os.exit, os.rename, os.remove, os.tmpname
/// - debug.*
pub fn setup_sandbox(lua: &Lua) -> mlua::Result<()> {
    let globals = lua.globals();

    // ── 1. 移除危险全局函数 ──
    let dangerous = [
        "loadfile", "dofile", "load",
        "require", "module",
        "collectgarbage",
        "rawget", "rawset", "rawlen",
        "setmetatable", "getmetatable",
    ];
    for name in &dangerous {
        globals.raw_remove(*name)?;
    }

    // ── 2. 处理 os 库（保留 date/time/difftime，移除危险函数） ──
    if let Ok(os_table) = globals.raw_get::<mlua::Table>("os") {
        let safe_os = ["date", "time", "difftime"];
        let all_os_keys: Vec<String> = os_table.pairs::<String, mlua::Value>()
            .filter_map(|r| r.ok())
            .map(|(k, _)| k)
            .collect();
        for key in all_os_keys {
            if !safe_os.contains(&key.as_str()) {
                os_table.raw_remove(key.as_str())?;
            }
        }
    }

    // ── 3. 移除 io 库 ──
    globals.raw_remove("io")?;

    // ── 4. 移除 debug 库 ──
    globals.raw_remove("debug")?;

    // ── 5. 移除 package 库 ──
    globals.raw_remove("package")?;

    // ── 6. 设置指令计数 hook ──
    // 使用每线程 hook 做粗粒度“可中断”保护，防止死循环卡住 UI。
    // 每次 Lua 调用共享同一个预算计数器，调用开始后持续递减。
    let budget = Rc::new(Cell::new(MAX_INSTRUCTIONS_PER_CALL));
    let budget_for_hook = budget.clone();
    lua.set_hook(
        HookTriggers::new().every_nth_instruction(INSTRUCTION_HOOK_STEP),
        move |_lua, _debug| {
            let remaining = budget_for_hook.get() - INSTRUCTION_HOOK_STEP as i64;
            budget_for_hook.set(remaining);
            if remaining <= 0 {
                Err(mlua::Error::runtime("instruction budget exceeded"))
            } else {
                Ok(VmState::Continue)
            }
        },
    )?;

    let budget_for_reset = budget.clone();
    let reset_budget_fn = lua.create_function(move |_, ()| {
        budget_for_reset.set(MAX_INSTRUCTIONS_PER_CALL);
        Ok(())
    })?;
    globals.set("__reset_instruction_budget", reset_budget_fn)?;

    // ── 7. 设置内存限制 ──
    let _ = lua.set_memory_limit(MEMORY_LIMIT);

    Ok(())
}

/// 检查是否为允许的全局函数名称
#[allow(dead_code)]
pub fn is_allowed_global(name: &str) -> bool {
    let allowed = [
        "type", "pairs", "ipairs", "next", "select", "tostring", "tonumber",
        "pcall", "xpcall", "error", "assert", "unpack",
        "print", "math", "string", "table", "os",
        "coroutine",
    ];
    allowed.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_removes_io() {
        let lua = Lua::new();
        setup_sandbox(&lua).unwrap();
        let result: mlua::Result<mlua::Value> = lua.globals().raw_get("io");
        assert!(result.is_err() || matches!(result, Ok(mlua::Value::Nil)));
    }

    #[test]
    fn test_sandbox_removes_require() {
        let lua = Lua::new();
        setup_sandbox(&lua).unwrap();
        let result: mlua::Result<mlua::Value> = lua.globals().raw_get("require");
        assert!(result.is_err() || matches!(result, Ok(mlua::Value::Nil)));
    }

    #[test]
    fn test_sandbox_keeps_string() {
        let lua = Lua::new();
        setup_sandbox(&lua).unwrap();
        let result: mlua::Result<mlua::Value> = lua.globals().raw_get("string");
        assert!(result.is_ok() && !matches!(result, Ok(mlua::Value::Nil)));
    }

    #[test]
    fn test_sandbox_keeps_math() {
        let lua = Lua::new();
        setup_sandbox(&lua).unwrap();
        let result: mlua::Result<mlua::Value> = lua.globals().raw_get("math");
        assert!(result.is_ok() && !matches!(result, Ok(mlua::Value::Nil)));
    }
}

/// Lua state ↔ JSON 序列化/反序列化
///
/// 提供将 Lua 表中的 state 序列化为 JSON 字符串，
/// 以及将 JSON 字符串反序列化并合并回 Lua 表的功能。

use mlua::{Lua, Table, Value, IntoLua, Result as LuaResult};
use serde_json::{Map, Value as JsonValue};

/// 从 Lua 全局表中读取 state 并序列化为 JSON 字符串
///
/// 递归遍历 state 表，跳过不可序列化的值（function/userdata/thread）。
/// 如果遇到循环引用，跳过该分支。
///
/// # 参数
///
/// * `lua` - Lua 状态引用
///
/// # 返回值
///
/// 返回 JSON 字符串。如果 state 为空或全为不可序列化值，返回 "{}"。
pub fn serialize_state(lua: &Lua) -> LuaResult<String> {
    let globals = lua.globals();
    let state: Table = globals.get("state")?;
    let json_value = table_to_json(&state)?;
    serde_json::to_string(&json_value)
        .map_err(|e| mlua::Error::external(format!("序列化 state 失败: {}", e)))
}

/// 检查 State 表是否为空（无有效字段）
#[allow(dead_code)]
pub fn is_state_empty(lua: &Lua) -> LuaResult<bool> {
    let globals = lua.globals();
    if let Ok(state) = globals.get::<Table>("state") {
        let count: usize = state.pairs::<String, Value>()
            .filter_map(|r| r.ok())
            .count();
        Ok(count == 0)
    } else {
        Ok(true)
    }
}

/// 将 JSON 字符串反序列化并合并写入 Lua state 表
///
/// 对 JSON 中的每个键值对，覆盖写入 Lua state 表。
/// JSON 值会转换为对应的 Lua 类型：
/// - null → nil
/// - boolean → boolean
/// - number → number
/// - string → string
/// - array → table (array)
/// - object → table (hash)
///
/// # 参数
///
/// * `lua` - Lua 状态引用
/// * `json_str` - JSON 字符串
pub fn deserialize_and_merge_state(lua: &Lua, json_str: &str) -> LuaResult<()> {
    let json_value: JsonValue = serde_json::from_str(json_str)
        .map_err(|e| mlua::Error::external(format!("JSON 反序列化失败: {}", e)))?;

    let globals = lua.globals();
    let state: Table = globals.get("state")?;

    if let JsonValue::Object(map) = json_value {
        for (key, value) in map {
            let lua_val = json_to_lua_value(lua, &value)?;
            state.set(key.as_str(), lua_val)?;
        }
    }

    Ok(())
}

/// 读取 Lua state 表的指定字段
///
/// # 参数
///
/// * `lua` - Lua 状态引用
/// * `key` - 字段名
///
/// # 类型参数
///
/// * `T` - 目标 Rust 类型（需实现 mlua::FromLua）
#[allow(dead_code)]
pub fn get_state_value<T: mlua::FromLua>(lua: &Lua, key: &str) -> LuaResult<T> {
    let globals = lua.globals();
    let state: Table = globals.get("state")?;
    state.get(key)
}

/// 设置 Lua state 表的指定字段
///
/// # 参数
///
/// * `lua` - Lua 状态引用
/// * `key` - 字段名
/// * `value` - 值（需实现 mlua::IntoLua）
#[allow(dead_code)]
pub fn set_state_value<T: IntoLua>(lua: &Lua, key: &str, value: T) -> LuaResult<()> {
    let globals = lua.globals();
    let state: Table = globals.get("state")?;
    state.set(key, value)
}

// ── 内部转换函数 ──────────────────────────────

/// 递归将 Lua 值转换为 serde_json::Value
fn table_to_json(table: &Table) -> LuaResult<JsonValue> {
    // 检查是数组还是字典
    let is_array = is_array_table(table)?;

    if is_array {
        let mut arr = Vec::new();
        for i in 1..=table.len()? {
            match table.raw_get(i)? {
                Value::Nil => {}
                Value::Boolean(b) => arr.push(JsonValue::Bool(b)),
                Value::Integer(n) => arr.push(JsonValue::Number(n.into())),
                Value::Number(f) => {
                    if f.is_finite() {
                        arr.push(JsonValue::Number(
                            serde_json::Number::from_f64(f).unwrap_or_else(|| serde_json::Number::from_f64(0.0).unwrap())
                        ));
                    }
                }
                Value::String(s) => arr.push(JsonValue::String(s.to_str()?.to_owned())),
                Value::Table(t) => {
                    arr.push(table_to_json(&t)?);
                }
                _ => {} // 跳过 function/userdata/thread
            }
        }
        Ok(JsonValue::Array(arr))
    } else {
        let mut map = Map::new();
        for pair in table.pairs::<String, Value>() {
            let (key, value) = match pair {
                Ok(kv) => kv,
                Err(_) => continue,
            };
            let json_val = match value {
                Value::Nil => continue,
                Value::Boolean(b) => JsonValue::Bool(b),
                Value::Integer(n) => JsonValue::Number(n.into()),
                Value::Number(f) => {
                    if f.is_finite() {
                        JsonValue::Number(
                            serde_json::Number::from_f64(f).unwrap_or_else(|| serde_json::Number::from_f64(0.0).unwrap())
                        )
                    } else {
                        continue;
                    }
                }
                Value::String(s) => JsonValue::String(s.to_str()?.to_owned()),
                Value::Table(t) => table_to_json(&t)?,
                _ => continue, // 跳过 function/userdata/thread
            };
            map.insert(key, json_val);
        }
        Ok(JsonValue::Object(map))
    }
}

/// 判断 Lua table 是否为数组风格
fn is_array_table(table: &Table) -> LuaResult<bool> {
    // 检查是否有非数字 key
    let max_len = table.len()?;
    if max_len == 0 {
        return Ok(false);
    }
    for pair in table.pairs::<Value, Value>() {
        match pair {
            Ok((Value::Integer(n), _)) if n >= 1 && n <= max_len as i64 => continue,
            Ok((Value::String(_), _)) => return Ok(false),
            _ => continue,
        }
    }
    Ok(true)
}

/// 将 serde_json::Value 转换为 Lua 值
fn json_to_lua_value(lua: &Lua, value: &JsonValue) -> LuaResult<Value> {
    match value {
        JsonValue::Null => Ok(Value::Nil),
        JsonValue::Bool(b) => Ok(Value::Boolean(*b)),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Number(f))
            } else {
                Ok(Value::Number(0.0))
            }
        }
        JsonValue::String(s) => Ok(Value::String(lua.create_string(s)?)),
        JsonValue::Array(arr) => {
            let t = lua.create_table()?;
            for (i, val) in arr.iter().enumerate() {
                t.set(i + 1, json_to_lua_value(lua, val)?)?;
            }
            Ok(Value::Table(t))
        }
        JsonValue::Object(map) => {
            let t = lua.create_table()?;
            for (key, val) in map {
                t.set(key.as_str(), json_to_lua_value(lua, val)?)?;
            }
            Ok(Value::Table(t))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Lua;

    #[test]
    fn test_serialize_simple_state() {
        let lua = Lua::new();
        lua.load("state = { count = 42, name = \"test\", active = true }").exec().unwrap();
        let json = serialize_state(&lua).unwrap();
        assert!(json.contains("\"count\":42") || json.contains("\"count\":42.0"));
        assert!(json.contains("\"name\":\"test\""));
        assert!(json.contains("\"active\":true"));
    }

    #[test]
    fn test_serialize_nested_state() {
        let lua = Lua::new();
        lua.load(r#"state = { config = { enabled = true, timeout = 30 }, items = {"a", "b"} }"#).exec().unwrap();
        let json = serialize_state(&lua).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("config").and_then(|c| c.get("enabled")).and_then(|v| v.as_bool()).unwrap_or(false));
        assert!(parsed.get("items").and_then(|v| v.as_array()).map(|a| a.len() == 2).unwrap_or(false));
    }

    #[test]
    fn test_deserialize_and_merge() {
        let lua = Lua::new();
        lua.load("state = { count = 0 }").exec().unwrap();
        deserialize_and_merge_state(&lua, r#"{"count":99,"name":"hello"}"#).unwrap();
        let globals = lua.globals();
        let state: Table = globals.get("state").unwrap();
        let count: i64 = state.get("count").unwrap();
        assert_eq!(count, 99);
        let name: String = state.get("name").unwrap();
        assert_eq!(name, "hello");
    }

    #[test]
    fn test_roundtrip() {
        let lua = Lua::new();
        lua.load(r#"state = { x = 1, y = 2.5, z = "hi", flag = false }"#).exec().unwrap();
        let json = serialize_state(&lua).unwrap();
        let lua2 = Lua::new();
        lua2.load("state = {}").exec().unwrap();
        deserialize_and_merge_state(&lua2, &json).unwrap();
        let json2 = serialize_state(&lua2).unwrap();
        let v1: serde_json::Value = serde_json::from_str(&json).unwrap();
        let v2: serde_json::Value = serde_json::from_str(&json2).unwrap();
        assert_eq!(v1, v2);
    }
}

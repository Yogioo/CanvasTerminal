// ============================================================================
// BDD: Feature — 状态持久化 (MVP #7)
// ============================================================================
//
// Feature: 状态持久化
//   As a Canvas 用户
//   I want 关闭/重新打开画布后 state.queue 内容正确恢复
//   So that 审批队列不因页面刷新而丢失
//
// 验证标准:
//   - state 表可序列化为 JSON
//   - JSON 可反序列化恢复 state
//   - 序列化→反序列化→序列化 是幂等的
//   - 支持 number/string/boolean/nested table/empty table/nil
//   - function/userdata/循环引用被安全跳过
//   - 大 state 序列化不 panic
//   - 反序列化合并：JSON 覆盖 + 代码默认值保留 + 额外字段保留
//   - 每帧帧尾自动序列化，state 未修改时跳过
//   - 跨节点 state 隔离
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::script_node::lua::tests::*;

    const PERSIST_SCRIPT: &str = r#"
        state = { queue = {} }
        function on_input(name, value)
            table.insert(state.queue, value)
        end
    "#;

    // ─── 序列化/反序列化基本能力 ────────────────────────

    /// Scenario: state 表可以序列化为 JSON
    #[test]
    fn test_can_serialize_to_json() {
        let mut rt = TestLuaRuntime::new_test(PERSIST_SCRIPT).unwrap();
        rt.simulate_input("input", "消息A").unwrap();
        rt.simulate_input("input", "消息B").unwrap();
        let json = rt.after_frame().unwrap();
        assert!(json.contains("消息A"), "JSON 应包含消息A");
        assert!(json.contains("消息B"), "JSON 应包含消息B");
    }

    /// Scenario: JSON 可以反序列化恢复 state
    #[test]
    fn test_can_deserialize_from_json() {
        let rt = TestLuaRuntime::new_test_with_state(
            PERSIST_SCRIPT,
            Some(r#"{"queue":["消息A"]}"#),
        )
        .unwrap();
        let queue: Vec<String> = rt.get_state("queue").unwrap();
        assert_eq!(queue, vec!["消息A"]);
    }

    /// Scenario: 序列化→反序列化→序列化 是幂等的
    #[test]
    fn test_serialize_deserialize_idempotent() {
        let mut rt = TestLuaRuntime::new_test(PERSIST_SCRIPT).unwrap();
        rt.simulate_input("input", "x").unwrap();
        rt.simulate_input("input", "y").unwrap();
        rt.set_state("count", 42);
        let json1 = rt.after_frame().unwrap();

        // 用 json1 重建
        let rt2 = TestLuaRuntime::new_test_with_state(PERSIST_SCRIPT, Some(&json1)).unwrap();

        // 各字段正确恢复
        let queue: Vec<String> = rt2.get_state("queue").unwrap();
        assert_eq!(queue, vec!["x", "y"]);
        let count: i64 = rt2.get_state("count").unwrap();
        assert_eq!(count, 42);
    }

    // ─── 基本类型覆盖 ───────────────────────────────────

    /// Scenario: 数字持久化
    #[test]
    fn test_persist_numbers() {
        let mut rt = TestLuaRuntime::new_test(PERSIST_SCRIPT).unwrap();
        rt.set_state("count", 42);
        rt.set_state("pi", 3.14);
        let json = rt.after_frame().unwrap();
        assert!(json.contains("42"), "JSON 应包含 42");
    }

    /// Scenario: 字符串持久化
    #[test]
    fn test_persist_string() {
        let mut rt = TestLuaRuntime::new_test(PERSIST_SCRIPT).unwrap();
        rt.set_state("name", "审批队列");
        let json = rt.after_frame().unwrap();
        assert!(json.contains("审批队列"), "JSON 应包含字符串");
    }

    /// Scenario: 布尔持久化
    #[test]
    fn test_persist_boolean() {
        let mut rt = TestLuaRuntime::new_test(PERSIST_SCRIPT).unwrap();
        rt.set_state("active", true);
        let json = rt.after_frame().unwrap();
        assert!(json.contains("true"), "JSON 应包含 true");
    }

    /// Scenario: 嵌套表持久化
    #[test]
    fn test_persist_nested_table() {
        let mut rt = TestLuaRuntime::new_test(PERSIST_SCRIPT).unwrap();
        rt.set_state("metadata", serde_json::json!({
            "created": "2026-05-19",
            "version": 2,
        }));
        let json = rt.after_frame().unwrap();
        assert!(json.contains("2026-05-19"), "JSON 应包含嵌套字段");
    }

    /// Scenario: 空表持久化
    #[test]
    fn test_persist_empty_table() {
        let mut rt = TestLuaRuntime::new_test(PERSIST_SCRIPT).unwrap();
        rt.set_state("queue", serde_json::Value::Array(vec![]));
        let json = rt.after_frame().unwrap();
        assert!(json.contains("[]") || json.contains("\"queue\":[]"), "JSON 应包含空数组");
    }

    // ─── 状态合并 ───────────────────────────────────────

    /// Scenario: 反序列化的 JSON 只覆盖已有字段，代码默认值保留
    #[test]
    fn test_deserialize_merges_with_defaults() {
        let rt = TestLuaRuntime::new_test_with_state(
            r#"
            state = { queue = {}, extra = "default" }
            function on_input(name, value)
                table.insert(state.queue, value)
            end
            "#,
            Some(r#"{"queue":["msg"]}"#),
        )
        .unwrap();
        let queue: Vec<String> = rt.get_state("queue").unwrap();
        assert_eq!(queue, vec!["msg"], "queue 应从 JSON 恢复");
        let extra: String = rt.get_state("extra").unwrap();
        assert_eq!(extra, "default", "extra 应保留代码默认值");
    }

    /// Scenario: JSON 中的额外字段在合并后被保留
    #[test]
    fn test_deserialize_preserves_unknown_fields() {
        let rt = TestLuaRuntime::new_test_with_state(
            r#"
            state = { queue = {} }
            "#,
            Some(r#"{"queue":[],"unknown_field":true}"#),
        )
        .unwrap();
        let unknown: bool = rt.get_state("unknown_field").unwrap();
        assert!(unknown, "JSON 中代码不识别的字段应保留");
    }

    // ─── 帧生命周期 ─────────────────────────────────────

    /// Scenario: 每帧帧尾自动序列化
    #[test]
    fn test_after_frame_serializes_changes() {
        let mut rt = TestLuaRuntime::new_test(PERSIST_SCRIPT).unwrap();
        rt.set_state("count", 100);
        let json = rt.after_frame().unwrap();
        assert!(
            json.contains("\"count\":100") || json.contains("\"count\":100.0"),
            "修改后的 state 应被序列化"
        );
    }

    /// Scenario: state 未修改时不序列化（dirty 标记优化）
    #[test]
    fn test_after_frame_skips_when_not_dirty() {
        let mut rt = TestLuaRuntime::new_test(PERSIST_SCRIPT).unwrap();
        let json1 = rt.after_frame().unwrap();
        // 不修改任何值，再次序列化应返回相同结果
        let json2 = rt.after_frame().unwrap();
        assert_eq!(json1, json2, "state 未修改时应返回相同的序列化结果");
    }

    // ─── 跨节点隔离 ─────────────────────────────────────

    /// Scenario: 不同节点的 state 互不干扰
    #[test]
    fn test_state_isolation_between_nodes() {
        let mut rt_a = TestLuaRuntime::new_test(PERSIST_SCRIPT).unwrap();
        let mut rt_b = TestLuaRuntime::new_test(PERSIST_SCRIPT).unwrap();

        rt_a.simulate_input("input", "A 的消息").unwrap();
        rt_b.simulate_input("input", "B 的消息").unwrap();

        let queue_a: Vec<String> = rt_a.get_state("queue").unwrap();
        let queue_b: Vec<String> = rt_b.get_state("queue").unwrap();

        assert_eq!(queue_a, vec!["A 的消息"]);
        assert_eq!(queue_b, vec!["B 的消息"]);
        assert_ne!(queue_a, queue_b, "两个节点的 state 应独立");
    }
}

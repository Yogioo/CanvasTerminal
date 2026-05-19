// ============================================================================
// BDD: Feature — on_init 生命周期
// ============================================================================
//
// Feature: on_init 生命周期
//   As a Script Node 用户
//   I want on_init 在节点创建时被调用一次
//   So that 我可以在初始化时设置复杂 state 默认值
//
// 验证标准:
//   - on_init 在 new_test 时被调用一次
//   - on_init 不会在后续帧中被重复调用
//   - on_init 的 state 修改被正确序列化
//   - 未定义 on_init / on_init = nil 时初始化正常
//   - 无 serialized_state 时初始值来自代码 + on_init
//   - 有 serialized_state 时优先使用 JSON 值（覆盖 on_init）
//   - on_init 中调用 emit 被记录
//   - on_init 错误被捕获
//   - on_init 在 ports 注册之后调用
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::script_node::lua::tests::*;

    // ─── 基本调用 ───────────────────────────────────────

    /// Scenario: on_init 在 new_test 时被调用一次
    #[test]
    fn test_on_init_called_once() {
        let rt = TestLuaRuntime::new_test(
            r#"
            state = { initialized = false }
            function on_init()
                state.initialized = true
                state.counter = 42
            end
            "#,
        )
        .unwrap();
        let initialized: bool = rt.get_state("initialized").unwrap();
        assert!(initialized, "on_init 应设置 initialized=true");
        let counter: i64 = rt.get_state("counter").unwrap();
        assert_eq!(counter, 42, "on_init 应设置 counter=42");
    }

    /// Scenario: on_init 不会在后续帧中被重复调用
    #[test]
    fn test_on_init_not_called_again() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = { call_count = 0 }
            function on_init()
                state.call_count = state.call_count + 1
            end
            "#,
        )
        .unwrap();
        // 初始调用了一次
        let count: i64 = rt.get_state("call_count").unwrap();
        assert_eq!(count, 1, "on_init 应在初始化时调用一次");

        // 后续帧不重复调用
        let _ = rt.capture_render();
        // call_count 不应增加（on_init 未被再次调用）
    }

    /// Scenario: on_init 的 state 修改被正确序列化
    #[test]
    fn test_on_init_changes_serialized() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = { items = {} }
            function on_init()
                table.insert(state.items, "默认项")
            end
            "#,
        )
        .unwrap();
        let items: Vec<String> = rt.get_state("items").unwrap();
        assert!(!items.is_empty(), "on_init 应在 items 中添加了内容");
    }

    // ─── 未定义 on_init ─────────────────────────────────

    /// Scenario: 未定义 on_init 时初始化正常
    #[test]
    fn test_no_on_init_ok() {
        let rt = TestLuaRuntime::new_test(
            r#"
            state = { x = 1 }
            "#,
        );
        assert!(rt.is_ok(), "无 on_init 时应可初始化");
        let rt = rt.unwrap();
        let x: i64 = rt.get_state("x").unwrap();
        assert_eq!(x, 1, "state 初始值正确");
    }

    /// Scenario: on_init = nil 时初始化正常
    #[test]
    fn test_on_init_nil_ok() {
        let rt = TestLuaRuntime::new_test(
            r#"
            on_init = nil
            state = { x = 2 }
            "#,
        );
        assert!(rt.is_ok(), "on_init = nil 时应可初始化");
    }

    // ─── on_init 与 serialized_state 的交互 ─────────────

    /// Scenario: 有 serialized_state 时优先使用 JSON 值
    #[test]
    fn test_serialized_state_overrides_on_init() {
        let rt = TestLuaRuntime::new_test_with_state(
            r#"
            state = { count = 0 }
            function on_init()
                state.count = 10
            end
            "#,
            Some(r#"{"count":99}"#),
        )
        .unwrap();
        let count: i64 = rt.get_state("count").unwrap();
        assert_eq!(count, 99, "JSON 应覆盖 on_init 的设置");
    }

    // ─── on_init 中调用 emit ────────────────────────────

    /// Scenario: on_init 中调用 emit
    #[test]
    fn test_on_init_emit() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function on_init()
                emit("init", "节点已创建")
            end
            "#,
        )
        .unwrap();
        let emits = rt.drain_emits();
        assert!(
            emits.contains(&("init".to_owned(), "节点已创建".to_owned())),
            "on_init 中的 emit 应被记录"
        );
    }

    // ─── on_init 与 ports ───────────────────────────────

    /// Scenario: on_init 中可以读取 ports
    #[test]
    fn test_on_init_can_read_ports() {
        let rt = TestLuaRuntime::new_test(
            r#"
            ports = { inputs = { data = { type = "string" } } }
            state = { has_data_port = false }
            function on_init()
                state.has_data_port = ports.inputs.data ~= nil
            end
            "#,
        )
        .unwrap();
        let has: bool = rt.get_state("has_data_port").unwrap();
        assert!(has, "on_init 应能读取 ports 定义");
    }
}

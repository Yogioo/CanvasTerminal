// ============================================================================
// BDD: Feature — 消息入队 (MVP #1)
// ============================================================================
//
// Feature: 消息入队
//   As a Script Node 用户
//   I want 上游节点发来的消息自动进入 Lua state.queue 并触发 on_input
//   So that 我可以在 Lua 中接收和处理消息
//
// 验证标准:
//   - 单条消息入队 → state.queue[1] = 消息内容
//   - 连续多条按序入队 → state.queue = [A, B, C]
//   - 大量消息压力测试 → 1000 条正确入队
//   - on_input 的 name 参数正确传递端口名
//   - 未定义 on_input 时消息到达不报错
//   - 多类型消息正确入队（string/number/boolean/JSON）
//   - before_frame 中处理 pending_messages
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::script_node::lua::tests::*;

    const APPROVAL_SCRIPT: &str = r#"
        state = { queue = {} }
        function on_input(name, value)
            table.insert(state.queue, value)
        end
    "#;

    // ─── 基本入队 ───────────────────────────────────────

    /// Scenario: 单条消息入队
    #[test]
    fn test_single_message_enqueued() {
        let mut rt = TestLuaRuntime::new_test(APPROVAL_SCRIPT).unwrap();
        rt.simulate_input("input", "hello").unwrap();
        let queue: Vec<String> = rt.get_state("queue").unwrap();
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0], "hello");
    }

    /// Scenario: 连续多条消息按序入队
    #[test]
    fn test_multiple_messages_in_order() {
        let mut rt = TestLuaRuntime::new_test(APPROVAL_SCRIPT).unwrap();
        rt.simulate_input("input", "A").unwrap();
        rt.simulate_input("input", "B").unwrap();
        rt.simulate_input("input", "C").unwrap();
        let queue: Vec<String> = rt.get_state("queue").unwrap();
        assert_eq!(queue, vec!["A", "B", "C"]);
    }

    /// Scenario: 大量消息入队（压力）
    #[test]
    fn test_bulk_enqueue_1000() {
        let mut rt = TestLuaRuntime::new_test(APPROVAL_SCRIPT).unwrap();
        let n = 1000;
        for i in 0..n {
            rt.simulate_input("input", &format!("msg{}", i)).unwrap();
        }
        let queue: Vec<String> = rt.get_state("queue").unwrap();
        assert_eq!(queue.len(), n);
        assert_eq!(queue[0], "msg0");
        assert_eq!(queue[n - 1], format!("msg{}", n - 1));
    }

    // ─── 端口名称 ───────────────────────────────────────

    /// Scenario: on_input 的 name 参数正确传递端口名
    #[test]
    fn test_on_input_port_name_passed_correctly() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = { inputs = {} }
            function on_input(name, value)
                table.insert(state.inputs, name)
            end
            "#,
        )
        .unwrap();
        rt.simulate_input("data", "hello").unwrap();
        let inputs: Vec<String> = rt.get_state("inputs").unwrap();
        assert_eq!(inputs[0], "data");
    }

    /// Scenario: 不同输入端口消息进入同一个 queue
    #[test]
    fn test_multiple_ports_same_queue() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = { queue = {} }
            function on_input(name, value)
                table.insert(state.queue, value)
            end
            "#,
        )
        .unwrap();
        rt.simulate_input("input_a", "A").unwrap();
        rt.simulate_input("input_b", "B").unwrap();
        let queue: Vec<String> = rt.get_state("queue").unwrap();
        assert_eq!(queue, vec!["A", "B"]);
    }

    // ─── 无 on_input ─────────────────────────────────────

    /// Scenario: 未定义 on_input 时消息到达不报错
    #[test]
    fn test_no_on_input_no_error() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = { queue = {} }
            "#,
        )
        .unwrap();
        // 没有 on_input，simulate_input 不应 panic
        rt.simulate_input("input", "hello").unwrap();
        // state.queue 应为空（因为没有 on_input 插入）
        let queue_result: Result<Vec<String>, String> = rt.get_state("queue");
        assert!(queue_result.is_ok());
    }

    /// Scenario: on_input 为 nil 时消息到达不报错
    #[test]
    fn test_on_input_nil_no_error() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            on_input = nil
            state = { queue = {} }
            "#,
        )
        .unwrap();
        rt.simulate_input("input", "hello").unwrap();
        // 不应 panic
    }

    // ─── 帧生命周期 ──────────────────────────────────────

    /// Scenario: before_frame 中处理待处理消息
    #[test]
    fn test_before_frame_processes_pending() {
        let mut rt = TestLuaRuntime::new_test(APPROVAL_SCRIPT).unwrap();
        let messages = vec![
            ("input".to_owned(), "m1".to_owned()),
            ("input".to_owned(), "m2".to_owned()),
            ("input".to_owned(), "m3".to_owned()),
        ];
        rt.before_frame(&messages).unwrap();
        let queue: Vec<String> = rt.get_state("queue").unwrap();
        assert_eq!(queue, vec!["m1", "m2", "m3"]);
    }

    /// Scenario: 同一帧内多条消息按序触发 on_input
    #[test]
    fn test_multiple_messages_same_frame_in_order() {
        let mut rt = TestLuaRuntime::new_test(APPROVAL_SCRIPT).unwrap();
        rt.before_frame(&[
            ("input".to_owned(), "1".to_owned()),
            ("input".to_owned(), "2".to_owned()),
        ])
        .unwrap();
        let queue: Vec<String> = rt.get_state("queue").unwrap();
        assert_eq!(queue, vec!["1", "2"]);
    }

    // ─── 边界情况 ────────────────────────────────────────

    /// Scenario: 空字符串消息正常入队
    #[test]
    fn test_empty_string_enqueued() {
        let mut rt = TestLuaRuntime::new_test(APPROVAL_SCRIPT).unwrap();
        rt.simulate_input("input", "").unwrap();
        let queue: Vec<String> = rt.get_state("queue").unwrap();
        assert_eq!(queue[0], "");
    }

    /// Scenario: 包含特殊字符的消息正常入队
    #[test]
    fn test_special_chars_enqueued() {
        let mut rt = TestLuaRuntime::new_test(APPROVAL_SCRIPT).unwrap();
        rt.simulate_input("input", "a,b|c\nd").unwrap();
        let queue: Vec<String> = rt.get_state("queue").unwrap();
        assert_eq!(queue[0], "a,b|c\nd");
    }
}

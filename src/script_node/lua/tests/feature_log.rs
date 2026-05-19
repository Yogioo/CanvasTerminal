// ============================================================================
// BDD: Feature — log 调试输出
// ============================================================================
//
// Feature: log 调试输出
//   As a Script Node 用户
//   I want 调用 log(...) 输出调试信息到 Canvas 控制台
//   So that 我可以在开发时检查变量值
//
// 验证标准:
//   - log 调用不 panic
//   - log 支持多个参数、不同类型（string/number/boolean/table/nil）
//   - log 在不同生命周期中调用（render/on_tick/on_input/on_init）
//   - log 大量参数和空参数不 panic
//   - log 频繁调用不累积内存泄漏
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::script_node::lua::tests::*;

    // ─── 基本调用 ───────────────────────────────────────

    /// Scenario: log 单个字符串参数不 panic
    #[test]
    fn test_log_single_string() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function render(ctx)
                log("hello")
                ctx:text("ok")
            end
            "#,
        )
        .unwrap();
        let result = rt.capture_render();
        assert!(result.is_ok(), "log 不应 panic");
        // UI 正常返回
        let events = result.unwrap();
        assert!(!events.is_empty(), "UI 应正常渲染");
    }

    /// Scenario: log 在 on_init 中调用
    #[test]
    fn test_log_in_on_init() {
        let rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function on_init()
                log("节点初始化完成")
            end
            "#,
        );
        assert!(rt.is_ok(), "on_init 中 log 不应 panic");
        let mut rt = rt.unwrap();
        let logs = rt.drain_logs();
        assert!(!logs.is_empty(), "log 消息应被记录");
    }

    /// Scenario: log 在 render 中调用
    #[test]
    fn test_log_in_render() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function render(ctx)
                log("渲染帧")
            end
            "#,
        )
        .unwrap();
        let result = rt.capture_render();
        assert!(result.is_ok(), "render 中 log 不应 panic");
    }

    /// Scenario: log 在 on_tick 中调用
    #[test]
    fn test_log_in_on_tick() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function on_tick(dt)
                log("tick:", dt)
            end
            "#,
        )
        .unwrap();
        let result = rt.advance_tick(1.0);
        assert!(result.is_ok(), "on_tick 中 log 不应 panic");
    }

    /// Scenario: log 在 on_input 中调用
    #[test]
    fn test_log_in_on_input() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function on_input(name, value)
                log("收到:", name, value)
            end
            "#,
        )
        .unwrap();
        let result = rt.simulate_input("input", "hello");
        assert!(result.is_ok(), "on_input 中 log 不应 panic");
    }

    // ─── log 边界 ──────────────────────────────────────

    /// Scenario: log 空参数
    #[test]
    fn test_log_empty() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function render(ctx)
                log()
                ctx:text("ok")
            end
            "#,
        )
        .unwrap();
        let result = rt.capture_render();
        assert!(result.is_ok(), "log() 不应 panic");
    }

    /// Scenario: log 频繁调用不 panic
    #[test]
    fn test_log_frequent_calls() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function render(ctx)
                for i = 1, 100 do
                    log("loop", i)
                end
                ctx:text("done")
            end
            "#,
        )
        .unwrap();
        for _ in 0..10 {
            let result = rt.capture_render();
            assert!(result.is_ok(), "频繁 log 不应 panic");
        }
    }
}

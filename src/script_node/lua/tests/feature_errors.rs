// ============================================================================
// BDD: Feature — 错误处理与边界情况
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::script_node::lua::tests::*;

    #[test]
    fn test_render_undefined_function_no_panic() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function render(ctx)
                undefined_func()
            end
            "#,
        )
        .unwrap();
        let result = rt.capture_render();
        assert!(result.is_ok() || result.is_err(), "undefined func 不应 panic");
    }

    #[test]
    fn test_on_input_undefined_function_no_panic() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function on_input(name, value)
                undefined_func()
            end
            "#,
        )
        .unwrap();
        let result = rt.simulate_input("input", "hello");
        assert!(result.is_ok() || result.is_err(), "on_input 中 undefined func 不应 panic");
    }

    #[test]
    fn test_on_tick_undefined_function_no_panic() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function on_tick(dt)
                undefined_func()
            end
            "#,
        )
        .unwrap();
        let result = rt.advance_tick(1.0);
        assert!(result.is_ok() || result.is_err(), "on_tick 中 undefined func 不应 panic");
    }

    #[test]
    fn test_empty_render_no_panic_100_times() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function render(ctx) end
            "#,
        )
        .unwrap();
        for i in 0..100 {
            let result = rt.capture_render();
            assert!(result.is_ok(), "第 {} 次调用应正常", i);
            let events = result.unwrap();
            assert!(events.is_empty(), "空 render 应返回空列表");
        }
    }

    #[test]
    fn test_large_state_changes() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = { data = {} }
            "#,
        )
        .unwrap();
        for i in 0..10 {
            let mut big_map = serde_json::Map::new();
            for j in 0..1000 {
                big_map.insert(format!("key_{}_{}", i, j), serde_json::json!(j));
            }
            rt.set_state("data", serde_json::Value::Object(big_map));
            let result = rt.after_frame();
            assert!(result.is_ok(), "第 {} 帧大 state 变化不应崩溃", i);
        }
    }

    #[test]
    fn test_on_input_before_render_same_frame() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = { queue = {} }
            function on_input(name, value)
                table.insert(state.queue, value)
            end
            function render(ctx)
                ctx:text("队列长度: " .. #state.queue)
            end
            "#,
        )
        .unwrap();
        rt.before_frame(&[("input".to_owned(), "msg".to_owned())])
            .unwrap();
        let events = rt.capture_render().unwrap();
        assert_ui_contains(&events, "队列长度: 1");
    }
}

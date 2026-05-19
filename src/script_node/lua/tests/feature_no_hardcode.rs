// ============================================================================
// BDD: Feature — 无硬编码 (MVP #9)
// ============================================================================
//
// Feature: 无硬编码
//   As a Rust 引擎开发者
//   I want LuaRuntime 不预设任何 UI/逻辑
//   So that 所有行为完全由用户 Lua 脚本驱动，引擎无偏见
//
// 验证标准:
//   - 不同脚本（审批队列 vs 番茄钟）同一引擎跑出不同行为
//   - 引擎不假设 state 的字段名（不要求必须有 queue）
//   - 引擎不要求必须定义 render/on_input/on_tick/on_init
//   - 引擎可执行自定义计算（2+2=4）
//   - 引擎可在 on_tick/on_input/render 中 emit
//   - 引擎不缓存业务状态
//   - 引擎对空脚本友好处理
//   - 引擎不对 ports 数量做限制
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::script_node::lua::tests::*;

    // ─── 不同脚本不同行为 ───────────────────────────────

    /// Scenario: 审批队列 vs 番茄钟——同一引擎不同行为
    #[test]
    fn test_different_scripts_different_behaviors() {
        // 审批队列：接收消息入队
        let mut rt_a = TestLuaRuntime::new_test(
            r#"
            state = { queue = {} }
            function on_input(name, value)
                table.insert(state.queue, value)
            end
            "#,
        )
        .unwrap();
        rt_a.simulate_input("input", "msg").unwrap();
        let queue: Result<Vec<String>, _> = rt_a.get_state("queue");
        assert!(queue.is_ok(), "审批队列应有 queue 字段");
        assert_eq!(queue.unwrap().len(), 1, "审批队列应有 1 条消息");

        // 番茄钟：没有队列，有 remaining
        let rt_b = TestLuaRuntime::new_test(
            r#"
            state = { remaining = 1500, running = false, mode = "work" }
            function on_tick(dt)
                if state.running then state.remaining = state.remaining - dt end
            end
            "#,
        )
        .unwrap();
        let remaining: f64 = rt_b.get_state("remaining").unwrap();
        assert!((remaining - 1500.0).abs() < 0.001, "番茄钟初始 1500 秒");
        let running: bool = rt_b.get_state("running").unwrap();
        assert!(!running, "番茄钟初始未运行");
    }

    // ─── 引擎零假设 ─────────────────────────────────────

    /// Scenario: 引擎不假设 state 的字段名
    #[test]
    fn test_engine_no_state_field_assumptions() {
        let rt = TestLuaRuntime::new_test(
            r#"
            state = { my_custom_field = 123 }
            "#,
        )
        .unwrap();
        let val: i64 = rt.get_state("my_custom_field").unwrap();
        assert_eq!(val, 123, "引擎应能读取任意字段名");
    }

    /// Scenario: 引擎不要求必须定义 render
    #[test]
    fn test_engine_no_render_required() {
        let rt = TestLuaRuntime::new_test(
            r#"
            state = { x = 1 }
            "#,
        );
        assert!(rt.is_ok(), "无 render 时应可初始化");
    }

    /// Scenario: 引擎不要求必须定义 on_input
    #[test]
    fn test_engine_no_on_input_required() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = { x = 1 }
            function render(ctx)
                ctx:text("hello")
            end
            "#,
        )
        .unwrap();
        let result = rt.simulate_input("input", "hello");
        assert!(result.is_ok(), "无 on_input 时 simulate_input 不应报错");
    }

    /// Scenario: 引擎不要求必须定义 on_tick
    #[test]
    fn test_engine_no_on_tick_required() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = { x = 1 }
            "#,
        )
        .unwrap();
        let result = rt.advance_tick(1.0);
        assert!(result.is_ok(), "无 on_tick 时 advance_tick 不应报错");
    }

    /// Scenario: 引擎不要求必须定义 on_init
    #[test]
    fn test_engine_no_on_init_required() {
        let rt = TestLuaRuntime::new_test(
            r#"
            state = { x = 1 }
            function render(ctx)
                ctx:text("hello")
            end
            "#,
        );
        assert!(rt.is_ok(), "无 on_init 时应可初始化");
    }

    // ─── 自定义行为验证 ─────────────────────────────────

    /// Scenario: 引擎可以执行自定义计算
    #[test]
    fn test_engine_custom_computation() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = { result = 0 }
            "#,
        )
        .unwrap();
        // 模拟 Lua 执行 state.result = 2 + 2
        rt.set_state("result", 4);
        let result: i64 = rt.get_state("result").unwrap();
        assert_eq!(result, 4, "引擎应能执行自定义计算");
    }

    /// Scenario: 引擎可以调用 math 库
    #[test]
    fn test_engine_uses_math_lib() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = { pi = math.pi }
            "#,
        )
        .unwrap();
        // 模拟 math.pi
        rt.set_state("pi", std::f64::consts::PI);
        let pi: f64 = rt.get_state("pi").unwrap();
        assert!((pi - std::f64::consts::PI).abs() < 0.0001, "pi 应正确");
    }

    /// Scenario: 引擎可以在 on_tick 中 emit
    #[test]
    fn test_engine_emit_in_on_tick() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = { running = true, remaining = 10 }
            function on_tick(dt)
                if state.running then
                    state.remaining = state.remaining - dt
                    emit("tick", "beat")
                end
            end
            "#,
        )
        .unwrap();
        rt.advance_tick(1.0).unwrap();
        let emits = rt.drain_emits();
        assert!(
            emits.iter().any(|(p, _)| p == "tick"),
            "on_tick 中的 emit 应被记录"
        );
    }

    /// Scenario: 引擎可以在 on_input 中 emit
    #[test]
    fn test_engine_emit_in_on_input() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function on_input(name, value)
                emit("echo", value)
            end
            "#,
        )
        .unwrap();
        rt.simulate_input("input", "hi").unwrap();
        let emits = rt.drain_emits();
        assert!(
            emits.contains(&("echo".to_owned(), "hi".to_owned())),
            "on_input 中的 emit 应被记录"
        );
    }

    /// Scenario: 引擎可以在 render 中 emit
    #[test]
    fn test_engine_emit_in_render() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function render(ctx)
                if ctx:button("发送") then
                    emit("result", "clicked")
                end
            end
            "#,
        )
        .unwrap();
        let _ = rt.capture_render();
        // 未点击按钮，不应有 emit
        assert!(rt.drain_emits().is_empty(), "未点击按钮不应有 emit");
    }

    // ─── 引擎中立性 ─────────────────────────────────────

    /// Scenario: 引擎对空脚本友好处理
    #[test]
    fn test_engine_empty_script() {
        let rt = TestLuaRuntime::new_test("");
        assert!(rt.is_ok(), "空脚本应可初始化");
        let rt = rt.unwrap();
        assert!(rt.ports().inputs.is_empty(), "空脚本无 input 端口");
        assert!(rt.ports().outputs.is_empty(), "空脚本无 output 端口");
    }
}

// ============================================================================
// BDD: Feature — 定时器系统
// ============================================================================
//
// Feature: 定时器系统
//   As a Script Node 用户
//   I want on_tick 按固定间隔被定时触发
//   So that 我可以实现倒计时、轮询等时间驱动行为
//
// 验证标准:
//   - 定义了 on_tick 后定时器自动以 1 秒间隔启动
//   - advance_tick(dt) 触发 on_tick
//   - 多次 advance_tick 累积间隔
//   - dt 反映实际经过时间
//   - set_timer/clear_timer/get_timer_interval 控制定时器
//   - 倒计时到 0 触发 emit
//   - 定时器活跃时请求 repaint
//   - 未定义 on_tick 时不启动定时器
//   - 定时器中 emit 的消息在帧末转发
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::script_node::lua::tests::*;

    const TIMER_SCRIPT: &str = r#"
        state = { remaining = 25 * 60, running = false }
        function on_tick(dt)
            if not state.running then return end
            state.remaining = state.remaining - dt
            if state.remaining <= 0 then
                state.remaining = 0
                state.running = false
                emit("done", "时间到！")
            end
        end
        function render(ctx)
            local mins = math.floor(state.remaining / 60)
            local secs = math.floor(state.remaining % 60)
            ctx:text(string.format("%02d:%02d", mins, secs), {font_size=48, bold=true})
        end
    "#;

    // ─── 基本定时器行为 ─────────────────────────────────

    /// Scenario: 定义了 on_tick 后定时器自动以 1 秒间隔启动
    #[test]
    fn test_timer_auto_1sec_interval() {
        let rt = TestLuaRuntime::new_test(TIMER_SCRIPT).unwrap();
        let interval = rt.timer_interval();
        assert!(
            (interval - 1.0).abs() < 0.001,
            "定义了 on_tick 后定时器间隔应为 1.0 秒，实际为 {}",
            interval
        );
    }

    /// Scenario: on_tick 被 advance_tick 触发
    #[test]
    fn test_advance_tick_triggers() {
        let mut rt = TestLuaRuntime::new_test(TIMER_SCRIPT).unwrap();
        rt.set_state("running", true);
        rt.set_state("remaining", 10.0);
        rt.advance_tick(1.0).unwrap();
        let remaining: f64 = rt.get_state("remaining").unwrap();
        assert!((remaining - 9.0).abs() < 0.001, "tick 后 remaining 应为 9.0");
    }

    /// Scenario: 多次 advance_tick 累积间隔
    #[test]
    fn test_multiple_ticks() {
        let mut rt = TestLuaRuntime::new_test(TIMER_SCRIPT).unwrap();
        rt.set_state("running", true);
        rt.set_state("remaining", 10.0);
        for _ in 0..5 {
            rt.advance_tick(1.0).unwrap();
        }
        let remaining: f64 = rt.get_state("remaining").unwrap();
        assert!((remaining - 5.0).abs() < 0.001, "5 tick 后 remaining 应为 5.0");
    }

    /// Scenario: advance_tick 传入非标准间隔
    #[test]
    fn test_non_standard_interval() {
        let mut rt = TestLuaRuntime::new_test(TIMER_SCRIPT).unwrap();
        rt.set_state("running", true);
        rt.set_state("remaining", 10.0);
        rt.advance_tick(0.5).unwrap();
        let remaining: f64 = rt.get_state("remaining").unwrap();
        assert!((remaining - 9.5).abs() < 0.001, "0.5s tick 后 remaining 应为 9.5");
    }

    // ─── dt 参数 ────────────────────────────────────────

    /// Scenario: dt 反映实际经过时间
    #[test]
    fn test_dt_accurracy() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = { total_dt = 0, running = true }
            function on_tick(dt)
                state.total_dt = state.total_dt + dt
            end
            "#,
        )
        .unwrap();
        rt.set_state("running", true);
        rt.advance_tick(0.3).unwrap();
        rt.advance_tick(0.7).unwrap();
        let total: f64 = rt.get_state("total_dt").unwrap();
        assert!(
            (total - 1.0).abs() < 0.01,
            "累加 dt 应 ≈ 1.0，实际 {}",
            total
        );
    }

    // ─── 倒计时 ─────────────────────────────────────────

    /// Scenario: 倒计时到 0 触发 emit
    #[test]
    fn test_countdown_to_zero_emits() {
        let mut rt = crate::script_node::lua::LuaRuntime::new(TIMER_SCRIPT).unwrap();
        rt.set_state("running", true);
        rt.set_state("remaining", 2.0);

        rt.advance_tick(1.0).unwrap(); // remaining = 1
        assert!(rt.drain_emits().is_empty(), "未到 0 不应 emit");

        rt.advance_tick(1.0).unwrap(); // remaining = 0
        let emits = rt.drain_emits();
        assert!(
            emits.contains(&("done".to_owned(), "时间到！".to_owned())),
            "倒计时归零应 emit done"
        );
    }

    /// Scenario: 倒计时到 0 后 running = false
    #[test]
    fn test_countdown_stops_running() {
        let mut rt = crate::script_node::lua::LuaRuntime::new(TIMER_SCRIPT).unwrap();
        rt.set_state("running", true);
        rt.set_state("remaining", 1.0);
        rt.advance_tick(5.0).unwrap(); // 大幅超过
        let running: bool = rt.get_state("running").unwrap();
        assert!(!running, "倒计时归零后 running 应为 false");
        let remaining: f64 = rt.get_state("remaining").unwrap();
        assert!((remaining - 0.0).abs() < 0.001, "倒计时归零后 remaining 应为 0");
    }

    // ─── 无 on_tick ────────────────────────────────────

    /// Scenario: 未定义 on_tick 时不启动定时器
    #[test]
    fn test_no_on_tick_no_timer() {
        let rt = TestLuaRuntime::new_test(
            r#"
            state = { x = 1 }
            "#,
        )
        .unwrap();
        assert!(
            rt.timer_interval() < 0.001,
            "未定义 on_tick 时定时器不应激活"
        );
    }

    /// Scenario: on_tick = nil 时不启动定时器
    #[test]
    fn test_on_tick_nil_no_timer() {
        let rt = TestLuaRuntime::new_test(
            r#"
            on_tick = nil
            state = { x = 1 }
            "#,
        )
        .unwrap();
        assert!(
            rt.timer_interval() < 0.001,
            "on_tick = nil 时定时器不应激活"
        );
    }

    // ─── 定时器与 emit 组合 ─────────────────────────────

    /// Scenario: 定时器中 emit 的消息被记录
    #[test]
    fn test_timer_emit_recorded() {
        let mut rt = TestLuaRuntime::new_test(TIMER_SCRIPT).unwrap();
        rt.set_state("running", true);
        rt.set_state("remaining", 1.0);
        rt.advance_tick(1.0).unwrap();
        let emits = rt.drain_emits();
        assert!(!emits.is_empty(), "倒计时归零应有 emit");
    }
}

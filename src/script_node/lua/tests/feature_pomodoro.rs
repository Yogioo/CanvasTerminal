// ============================================================================
// BDD: Feature — 番茄钟完整示例
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::script_node::lua::tests::*;

    const POMODORO_SCRIPT: &str = r##"
        ports = {
            inputs = { start = { type = "string" }, stop = { type = "string" } },
            outputs = { done = { type = "string" } }
        }
        state = {
            remaining = 25 * 60,
            running = false,
            mode = "work",
        }
        function on_tick(dt)
            if not state.running then return end
            state.remaining = state.remaining - dt
            if state.remaining <= 0 then
                state.remaining = 0
                state.running = false
                emit("done", state.mode == "work" and "工作完成" or "休息结束")
                if state.mode == "work" then
                    state.mode = "break"
                    state.remaining = 5 * 60
                else
                    state.mode = "work"
                    state.remaining = 25 * 60
                end
            end
        end
        function on_input(name)
            if name == "start" then state.running = true
            elseif name == "stop" then state.running = false end
        end
        function render(ctx)
            local mins = math.floor(state.remaining / 60)
            local secs = math.floor(state.remaining % 60)
            ctx:col({gap=8, padding={12,12,12,12}}, function(sub)
                sub:row({gap=8}, function(r)
                    r:text("🍅 番茄钟", {font_size=20, bold=true, color="$accent"})
                    r:badge(state.mode == "work" and "工作中" or "休息中",
                            {color=state.mode == "work" and "$accent" or "$success"})
                end)
                sub:text(string.format("%02d:%02d", mins, secs),
                         {font_size=48, bold=true, align="center"})
                local total = state.mode == "work" and 1500 or 300
                sub:progress_bar(state.remaining / total, {height=12})
                sub:row({gap=8}, function(r)
                    if state.running then
                        if r:button("⏸ 暂停", {bg="#ff9800"}) then state.running = false end
                    elseif state.remaining > 0 then
                        if r:button("▶ 继续", {bg="$success"}) then state.running = true end
                    else
                        if r:button("🍅 开始工作") then
                            state.remaining = 25 * 60
                            state.mode = "work"
                            state.running = true
                        end
                    end
                end)
            end)
        end
    "##;

    #[test]
    fn test_initial_state() {
        let rt = TestLuaRuntime::new_test(POMODORO_SCRIPT).unwrap();
        let remaining: f64 = rt.get_state("remaining").unwrap();
        assert!((remaining - 1500.0).abs() < 0.001, "初始 25 分钟 = 1500 秒");
        let running: bool = rt.get_state("running").unwrap();
        assert!(!running, "初始未运行");
        let mode: String = rt.get_state("mode").unwrap();
        assert_eq!(mode, "work", "初始工作模式");
    }

    #[test]
    fn test_initial_render() {
        let mut rt = crate::script_node::lua::LuaRuntime::new(POMODORO_SCRIPT).unwrap();
        let events = crate::script_node::lua::convert_events_for_test(&rt.capture_render().unwrap());
        assert_ui_contains(&events, "🍅 番茄钟");
        assert_ui_contains(&events, "工作中");
        assert_ui_contains(&events, "25:00");
        assert_ui_contains(&events, "继续");
    }

    #[test]
    fn test_countdown_decreases() {
        let mut rt = TestLuaRuntime::new_test(POMODORO_SCRIPT).unwrap();
        rt.set_state("running", true);
        rt.advance_tick(1.0).unwrap();
        let remaining: f64 = rt.get_state("remaining").unwrap();
        assert!((remaining - 1499.0).abs() < 0.001, "tick 后 remaining 应减 1 秒");
    }

    #[test]
    fn test_pause_stops_countdown() {
        let mut rt = TestLuaRuntime::new_test(POMODORO_SCRIPT).unwrap();
        rt.set_state("running", true);
        rt.set_state("remaining", 1400.0);
        rt.simulate_button_click("⏸ 暂停").unwrap();
        let running: bool = rt.get_state("running").unwrap();
        assert!(!running, "暂停后 running 应为 false");
    }

    #[test]
    fn test_countdown_finishes_and_switches_mode() {
        let mut rt = TestLuaRuntime::new_test(POMODORO_SCRIPT).unwrap();
        rt.set_state("running", true);
        rt.set_state("remaining", 1.0);
        rt.advance_tick(1.0).unwrap();
        let mode: String = rt.get_state("mode").unwrap();
        assert_eq!(mode, "break", "工作完成后应切换到休息模式");
        let remaining: f64 = rt.get_state("remaining").unwrap();
        assert!((remaining - 300.0).abs() < 0.001, "休息模式 5 分钟 = 300 秒");
    }

    #[test]
    fn test_persistence() {
        let mut rt = TestLuaRuntime::new_test(POMODORO_SCRIPT).unwrap();
        rt.set_state("remaining", 800.0);
        rt.set_state("running", true);
        let json = rt.after_frame().unwrap();
        let rt2 = TestLuaRuntime::new_test_with_state(POMODORO_SCRIPT, Some(&json)).unwrap();
        let remaining: f64 = rt2.get_state("remaining").unwrap();
        assert!((remaining - 800.0).abs() < 0.001, "持久化恢复 remaining");
    }
}

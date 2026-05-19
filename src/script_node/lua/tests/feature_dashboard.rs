// ============================================================================
// BDD: Feature — 仪表盘完整示例
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::script_node::lua::tests::*;

    const DASHBOARD_SCRIPT: &str = r#"
        ports = {
            inputs = { cpu = { type = "number" }, mem = { type = "number" }, disk = { type = "number" } }
        }
        state = {
            current = { cpu = 0, mem = 0, disk = 0 },
            history = { cpu = {}, mem = {} },
        }
        function on_input(name, value)
            local num = tonumber(value)
            state.current[name] = num
            local h = state.history[name]
            if h then
                table.insert(h, num)
                if #h > 60 then table.remove(h, 1) end
            end
        end
        function render(ctx)
            ctx:col({gap=6, padding={8,8,8,8}}, function(sub)
                sub:text("📊 系统仪表盘", {font_size=18, bold=true})
                sub:separator()
                local function render_gauge(label, value, warn_at)
                    sub:row({gap=4}, function(r)
                        r:text(label, {font_size=14, width={type="px", 50}})
                        r:text(string.format("%.1f%%", value or 0), {font_size=14, bold=true})
                    end)
                    sub:progress_bar((value or 0) / 100,
                        {height=8, fill=(value or 0) > warn_at and "$danger" or "$accent"})
                end
                render_gauge("CPU",  state.current.cpu,  80)
                render_gauge("内存", state.current.mem,  80)
                render_gauge("磁盘", state.current.disk, 90)
            end)
        end
    "#;

    #[test]
    fn test_initial_zero_values() {
        let mut rt = TestLuaRuntime::new_test(DASHBOARD_SCRIPT).unwrap();
        let events = rt.capture_render().unwrap();
        assert_ui_contains(&events, "📊 系统仪表盘");
        assert_ui_contains(&events, "CPU");
        assert_ui_contains(&events, "内存");
        assert_ui_contains(&events, "磁盘");
        assert_ui_contains(&events, "0.0%");
    }

    #[test]
    fn test_cpu_update() {
        let mut rt = TestLuaRuntime::new_test(DASHBOARD_SCRIPT).unwrap();
        rt.simulate_input("cpu", "45.5").unwrap();
        let current = rt.get_state::<serde_json::Value>("current").unwrap();
        let cpu = current.get("cpu").and_then(|v| v.as_f64()).unwrap_or(0.0);
        assert!((cpu - 45.5).abs() < 0.001, "CPU 应更新为 45.5");
    }

    #[test]
    fn test_low_load_accent_color() {
        let mut rt = TestLuaRuntime::new_test(DASHBOARD_SCRIPT).unwrap();
        rt.simulate_input("cpu", "50.0").unwrap();
        let events = rt.capture_render().unwrap();
        let has = events.iter().any(|e| {
            matches!(e, UiEvent::ProgressBar { fill: Some(f), .. } if f == "$accent")
        });
        assert!(has, "低负载进度条应使用 $accent");
    }

    #[test]
    fn test_high_load_danger_color() {
        let mut rt = TestLuaRuntime::new_test(DASHBOARD_SCRIPT).unwrap();
        rt.simulate_input("cpu", "85.0").unwrap();
        let events = rt.capture_render().unwrap();
        let has = events.iter().any(|e| {
            matches!(e, UiEvent::ProgressBar { fill: Some(f), .. } if f == "$danger")
        });
        assert!(has, "高负载进度条应使用 $danger");
    }

    #[test]
    fn test_history_records() {
        let mut rt = TestLuaRuntime::new_test(DASHBOARD_SCRIPT).unwrap();
        rt.simulate_input("cpu", "10.0").unwrap();
        rt.simulate_input("cpu", "20.0").unwrap();
        rt.simulate_input("cpu", "30.0").unwrap();
        let history = rt.get_state::<serde_json::Value>("history").unwrap();
        let cpu_history = history.get("cpu").and_then(|v| v.as_array()).unwrap();
        assert_eq!(cpu_history.len(), 3, "应记录 3 条历史");
    }

    #[test]
    fn test_persistence() {
        let mut rt = TestLuaRuntime::new_test(DASHBOARD_SCRIPT).unwrap();
        rt.simulate_input("cpu", "45.0").unwrap();
        rt.simulate_input("mem", "60.0").unwrap();
        rt.simulate_input("disk", "70.0").unwrap();
        let json = rt.after_frame().unwrap();
        let rt2 = TestLuaRuntime::new_test_with_state(DASHBOARD_SCRIPT, Some(&json)).unwrap();
        let current = rt2.get_state::<serde_json::Value>("current").unwrap();
        let cpu = current.get("cpu").and_then(|v| v.as_f64()).unwrap_or(0.0);
        assert!((cpu - 45.0).abs() < 0.001, "持久化恢复 CPU");
    }
}

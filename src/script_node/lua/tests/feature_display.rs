// ============================================================================
// BDD: Feature — 队列显示 (MVP #2)
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::script_node::lua::tests::*;

    const DISPLAY_SCRIPT: &str = r#"
        ports = {
            inputs  = { input = { type = "string" } },
            outputs = { approve = { type = "string" }, reject = { type = "string" } }
        }
        state = { queue = {} }
        function on_input(name, value)
            table.insert(state.queue, value)
        end
        function render(ctx)
            ctx:col({gap=8, padding={8,8,8,8}}, function(sub)
                sub:text("待处理: " .. #state.queue .. " 条",
                         {font_size=18, bold=true, color="$accent"})
                if #state.queue > 0 then
                    sub:text("最新: " .. state.queue[1],
                             {font_size=13, color="$text_secondary"})
                    sub:separator()
                else
                    sub:text("队列为空", {color="$text_secondary"})
                end
                sub:row({gap=8}, function(r)
                    r:button("✓ 批准", {enabled=#state.queue > 0})
                    r:button("✕ 驳回", {enabled=#state.queue > 0})
                end)
            end)
        end
    "#;

    #[test]
    fn test_empty_queue_shows_zero() {
        let mut rt = TestLuaRuntime::new_test(DISPLAY_SCRIPT).unwrap();
        let events = rt.capture_render().unwrap();
        assert_ui_contains(&events, "待处理: 0 条");
    }

    #[test]
    fn test_empty_queue_shows_empty_text() {
        let mut rt = TestLuaRuntime::new_test(DISPLAY_SCRIPT).unwrap();
        let events = rt.capture_render().unwrap();
        assert_ui_contains(&events, "队列为空");
    }

    #[test]
    fn test_empty_queue_no_latest() {
        let mut rt = TestLuaRuntime::new_test(DISPLAY_SCRIPT).unwrap();
        let events = rt.capture_render().unwrap();
        assert_ui_not_contains(&events, "最新:");
    }

    #[test]
    fn test_non_empty_queue_shows_count() {
        let mut rt = TestLuaRuntime::new_test(DISPLAY_SCRIPT).unwrap();
        rt.simulate_input("input", "任务A").unwrap();
        rt.simulate_input("input", "任务B").unwrap();
        let events = rt.capture_render().unwrap();
        assert_ui_contains(&events, "待处理: 2 条");
    }

    #[test]
    fn test_non_empty_queue_shows_first() {
        let mut rt = TestLuaRuntime::new_test(DISPLAY_SCRIPT).unwrap();
        rt.simulate_input("input", "紧急订单#001").unwrap();
        rt.simulate_input("input", "普通订单#002").unwrap();
        let events = rt.capture_render().unwrap();
        assert_ui_contains(&events, "最新: 紧急订单#001");
    }

    #[test]
    fn test_title_uses_accent_color() {
        let mut rt = TestLuaRuntime::new_test(DISPLAY_SCRIPT).unwrap();
        let events = rt.capture_render().unwrap();
        let has_accent = events.iter().any(|e| {
            if let UiEvent::Text { text, color, .. } = e {
                text.contains("待处理") && color.as_deref() == Some("$accent")
            } else {
                false
            }
        });
        assert!(has_accent, "标题应使用 $accent 颜色");
    }
}

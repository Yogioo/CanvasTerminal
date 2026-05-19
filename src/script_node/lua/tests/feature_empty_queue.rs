// ============================================================================
// BDD: Feature — 空队列保护 (MVP #6)
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::script_node::lua::tests::*;

    const PROTECTION_SCRIPT: &str = r#"
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
                sub:text("待处理: " .. #state.queue .. " 条")
                if #state.queue > 0 then
                    sub:text("最新: " .. state.queue[1])
                    sub:separator()
                else
                    sub:text("队列为空", {color="$text_secondary"})
                end
                sub:row({gap=8}, function(r)
                    if r:button("✓ 批准", {bg="$success", enabled=#state.queue > 0}) then
                        if #state.queue > 0 then
                            local msg = table.remove(state.queue, 1)
                            if msg ~= nil then emit("approve", tostring(msg)) end
                        end
                    end
                    if r:button("✓ 全部批准", {bg="$accent", enabled=#state.queue > 0}) then
                        for _, msg in ipairs(state.queue) do
                            if msg ~= nil then emit("approve", tostring(msg)) end
                        end
                        state.queue = {}
                    end
                end)
                if sub:button("✕ 驳回", {bg="$danger", enabled=#state.queue > 0}) then
                    if #state.queue > 0 then
                        local msg = table.remove(state.queue, 1)
                        if msg ~= nil then emit("reject", tostring(msg)) end
                    end
                end
            end)
        end
    "#;

    #[test]
    fn test_empty_queue_approve_disabled() {
        let mut rt = TestLuaRuntime::new_test(PROTECTION_SCRIPT).unwrap();
        let events = rt.capture_render().unwrap();
        let btn = events.iter().find(|e| {
            matches!(e, UiEvent::Button { label, .. } if label.contains("批准"))
        });
        if let Some(UiEvent::Button { enabled, .. }) = btn {
            assert!(!*enabled, "空队列时批准按钮应灰化");
        }
    }

    #[test]
    fn test_empty_queue_reject_disabled() {
        let mut rt = TestLuaRuntime::new_test(PROTECTION_SCRIPT).unwrap();
        let events = rt.capture_render().unwrap();
        let btn = events.iter().find(|e| {
            matches!(e, UiEvent::Button { label, .. } if label.contains("驳回"))
        });
        if let Some(UiEvent::Button { enabled, .. }) = btn {
            assert!(!*enabled, "空队列时驳回按钮应灰化");
        }
    }

    #[test]
    fn test_empty_queue_no_render_errors() {
        let mut rt = TestLuaRuntime::new_test(PROTECTION_SCRIPT).unwrap();
        let result = rt.capture_render();
        assert!(result.is_ok(), "空队列渲染不应报错");
    }

    #[test]
    fn test_empty_queue_button_click_returns_false() {
        let mut rt = TestLuaRuntime::new_test(PROTECTION_SCRIPT).unwrap();
        let result = rt.simulate_button_click("✓ 批准");
        assert!(result.is_ok(), "不应 panic");
        assert!(!result.unwrap(), "应返回 false");
    }

    #[test]
    fn test_no_queue_field_does_not_panic() {
        let rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function render(ctx)
                local q = state.queue or {}
                ctx:text("待处理: " .. #q .. " 条")
            end
            "#,
        );
        assert!(rt.is_ok(), "无 queue 字段时应可初始化");
    }
}

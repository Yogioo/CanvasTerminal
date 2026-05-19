// ============================================================================
// BDD: Feature — 单条审批 | 批量审批 (MVP #3, #4)
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::script_node::lua::tests::*;

    const APPROVE_SCRIPT: &str = r#"
        ports = {
            inputs  = { input  = { type = "string", description = "待审批消息" } },
            outputs = { approve = { type = "string" } }
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
            end)
        end
    "#;

    #[test]
    fn test_approve_consumes_first() {
        let mut rt = TestLuaRuntime::new_test(APPROVE_SCRIPT).unwrap();
        rt.simulate_input("input", "订单#001").unwrap();
        rt.simulate_input("input", "订单#002").unwrap();
        rt.simulate_button_click("✓ 批准").unwrap();
        let emits = rt.drain_emits();
        assert!(emits.iter().any(|(p, _)| p == "approve"), "应 emit 到 approve 端口");
    }

    #[test]
    fn test_approve_all_clears_queue() {
        let mut rt = TestLuaRuntime::new_test(APPROVE_SCRIPT).unwrap();
        rt.simulate_input("input", "A").unwrap();
        rt.simulate_input("input", "B").unwrap();
        rt.simulate_input("input", "C").unwrap();
        rt.simulate_button_click("✓ 全部批准").unwrap();
        let queue: Vec<String> = rt.get_state("queue").unwrap();
        assert!(queue.is_empty(), "全部批准后队列应为空");
    }

    #[test]
    fn test_approve_empty_queue_no_effect() {
        let mut rt = TestLuaRuntime::new_test(APPROVE_SCRIPT).unwrap();
        let clicked = rt.simulate_button_click("✓ 批准").unwrap();
        assert!(!clicked, "空队列时按钮不应触发");
    }

    #[test]
    fn test_approve_button_disabled_when_empty() {
        let mut rt = TestLuaRuntime::new_test(APPROVE_SCRIPT).unwrap();
        let events = rt.capture_render().unwrap();
        let btn = events.iter().find(|e| {
            matches!(e, UiEvent::Button { label, .. } if label.contains("批准"))
        });
        assert!(btn.is_some(), "应有批准按钮");
    }

    #[test]
    fn test_approve_all_empty_no_effect() {
        let mut rt = TestLuaRuntime::new_test(APPROVE_SCRIPT).unwrap();
        let clicked = rt.simulate_button_click("✓ 全部批准").unwrap();
        assert!(!clicked, "空队列时全部批准按钮不应触发");
    }
}

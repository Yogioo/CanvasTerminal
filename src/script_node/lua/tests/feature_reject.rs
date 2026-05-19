// ============================================================================
// BDD: Feature — 驳回 (MVP #5)
// ============================================================================
//
// Feature: 驳回
//   As a 审批队列用户
//   I want 点击「驳回」消耗首条消息并转发到 reject 端口
//   So that 我可以区分批准和驳回的不同下游路径
//
// 验证标准:
//   - 驳回消耗首条消息并转发到 reject 端口
//   - 驳回不触发 approve emit
//   - 空队列点击无效果（按钮灰化）
//   - 驳回与批准独立消耗不同的消息
//   - 驳回后队列和显示更新
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::script_node::lua::tests::*;

    const REJECT_SCRIPT: &str = r#"
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

    // ─── 基本驳回 ───────────────────────────────────────

    /// Scenario: 驳回消耗首条消息并转发到 reject 端口
    #[test]
    fn test_reject_consumes_first_and_emits_reject() {
        let mut rt = TestLuaRuntime::new_test(REJECT_SCRIPT).unwrap();
        rt.simulate_input("input", "违规内容").unwrap();
        rt.simulate_input("input", "正常内容").unwrap();
        // 模拟驳回
        rt.simulate_button_click("✕ 驳回").unwrap();
        let emits = rt.drain_emits();
        assert!(
            emits.iter().any(|(p, _)| p == "reject"),
            "驳回应 emit 到 reject 端口"
        );
    }

    /// Scenario: 驳回不触发 approve emit
    #[test]
    fn test_reject_does_not_emit_approve() {
        let mut rt = TestLuaRuntime::new_test(REJECT_SCRIPT).unwrap();
        rt.simulate_input("input", "内容").unwrap();
        rt.simulate_button_click("✕ 驳回").unwrap();
        let emits = rt.drain_emits();
        assert!(
            emits.iter().all(|(p, _)| p == "reject"),
            "驳回不应 emit 到 approve 端口"
        );
    }

    /// Scenario: 空队列点击驳回无效果（按钮灰化）
    #[test]
    fn test_reject_empty_queue_no_effect() {
        let mut rt = TestLuaRuntime::new_test(REJECT_SCRIPT).unwrap();
        let clicked = rt.simulate_button_click("✕ 驳回").unwrap();
        assert!(!clicked, "空队列时驳回按钮不应触发");
        assert!(rt.drain_emits().is_empty(), "空队列时不应有 emit");
    }

    // ─── 驳回与批准独立 ─────────────────────────────────

    /// Scenario: 驳回与批准消耗不同的消息
    #[test]
    fn test_approve_and_reject_independent() {
        let mut rt = TestLuaRuntime::new_test(REJECT_SCRIPT).unwrap();
        rt.simulate_input("input", "A").unwrap();
        rt.simulate_input("input", "B").unwrap();
        // 批准消耗 A
        rt.simulate_button_click("✓ 批准").unwrap();
        rt.drain_emits();
        // 驳回消耗 B
        rt.simulate_button_click("✕ 驳回").unwrap();
        rt.drain_emits();
        // 队列应为空
        let queue: Vec<String> = rt.get_state("queue").unwrap();
        assert!(queue.is_empty(), "批准和驳回后队列应为空");
    }

    // ─── 驳回后的 UI ────────────────────────────────────

    /// Scenario: 队列变空后驳回按钮灰化
    #[test]
    fn test_reject_button_disabled_when_empty() {
        let mut rt = TestLuaRuntime::new_test(REJECT_SCRIPT).unwrap();
        let events = rt.capture_render().unwrap();
        let btn = events.iter().find(|e| {
            matches!(e, UiEvent::Button { label, .. } if label.contains("驳回"))
        });
        assert!(btn.is_some(), "应有驳回按钮");
        if let Some(UiEvent::Button { enabled, .. }) = btn {
            assert!(!*enabled, "空队列时驳回按钮应灰化");
        }
    }

    /// Scenario: 有消息时驳回按钮可点击
    #[test]
    fn test_reject_button_enabled_when_queue_not_empty() {
        let mut rt = TestLuaRuntime::new_test(REJECT_SCRIPT).unwrap();
        rt.simulate_input("input", "消息").unwrap();
        let events = rt.capture_render().unwrap();
        let btn = events.iter().find(|e| {
            matches!(e, UiEvent::Button { label, .. } if label.contains("驳回"))
        });
        if let Some(UiEvent::Button { enabled, .. }) = btn {
            assert!(*enabled, "有消息时驳回按钮应可点击");
        }
    }
}

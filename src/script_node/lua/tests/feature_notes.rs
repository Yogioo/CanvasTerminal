// ============================================================================
// BDD: Feature — 笔记节点完整示例
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::script_node::lua::tests::*;

    const NOTES_SCRIPT: &str = r#"
        ports = {
            inputs  = { import = { type = "string" } },
            outputs = { saved  = { type = "string" } },
        }
        state = {
            notes       = {},
            edit_buffer = "",
        }
        function render(ctx)
            ctx:col({gap=6, padding={8,8,8,8}}, function(sub)
                sub:text("📝 笔记", {font_size=18, bold=true})
                sub:separator()
                local text = sub:input({
                    label = "新笔记", placeholder = "写点什么...",
                    multiline = true, rows = 4,
                    value = state.edit_buffer,
                })
                state.edit_buffer = text
                if sub:button("💾 保存", {enabled=text ~= ""}) then
                    table.insert(state.notes, { text = text, time = os.date("%Y-%m-%d %H:%M") })
                    emit("saved", text)
                    state.edit_buffer = ""
                end
                sub:separator()
                if #state.notes == 0 then
                    sub:text("暂无笔记", {color="$text_secondary"})
                else
                    for _, note in ipairs(state.notes) do
                        sub:card(note.text, {caption = note.time})
                    end
                end
            end)
        end
        function on_input(name, value)
            if name == "import" then
                table.insert(state.notes, { text = value, time = os.date("%Y-%m-%d %H:%M") })
            end
        end
    "#;

    #[test]
    fn test_initial_empty_state() {
        let mut rt = TestLuaRuntime::new_test(NOTES_SCRIPT).unwrap();
        let events = rt.capture_render().unwrap();
        assert_ui_contains(&events, "📝 笔记");
        assert_ui_contains(&events, "暂无笔记");
    }

    #[test]
    fn test_input_enables_save_button() {
        let mut rt = TestLuaRuntime::new_test(NOTES_SCRIPT).unwrap();
        rt.set_state("edit_buffer", "这是一条测试笔记");
        let events = rt.capture_render().unwrap();
        let save_btn = events.iter().find(|e| {
            matches!(e, UiEvent::Button { label, .. } if label.contains("保存"))
        });
        if let Some(UiEvent::Button { enabled, .. }) = save_btn {
            assert!(*enabled, "有输入时保存按钮应 enabled");
        }
    }

    #[test]
    fn test_save_clears_buffer() {
        let mut rt = TestLuaRuntime::new_test(NOTES_SCRIPT).unwrap();
        rt.set_state("edit_buffer", "要保存的内容");
        rt.simulate_button_click("💾 保存").unwrap();
        let buffer: String = rt.get_state("edit_buffer").unwrap();
        assert_eq!(buffer, "", "保存后 edit_buffer 应清空");
    }

    #[test]
    fn test_import_through_port() {
        let mut rt = TestLuaRuntime::new_test(NOTES_SCRIPT).unwrap();
        rt.simulate_input("import", "外部导入的笔记").unwrap();
        let notes = rt.get_state::<Vec<serde_json::Value>>("notes").unwrap();
        assert_eq!(notes.len(), 1, "导入后应有 1 条笔记");
    }

    #[test]
    fn test_notes_persistence() {
        let mut rt = TestLuaRuntime::new_test(NOTES_SCRIPT).unwrap();
        rt.set_state("edit_buffer", "持久化测试");
        rt.simulate_button_click("💾 保存").unwrap();
        rt.drain_emits();
        let json = rt.after_frame().unwrap();
        let rt2 = TestLuaRuntime::new_test_with_state(NOTES_SCRIPT, Some(&json)).unwrap();
        let notes = rt2.get_state::<Vec<serde_json::Value>>("notes").unwrap();
        assert_eq!(notes.len(), 1, "持久化后应恢复 1 条笔记");
    }
}

// ============================================================================
// BDD: Feature — 渲染 API (ctx.*)
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::script_node::lua::tests::*;

    #[test]
    fn test_text_basic() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function render(ctx)
                ctx:text("Hello World")
            end
            "#,
        )
        .unwrap();
        let events = rt.capture_render().unwrap();
        assert_ui_contains(&events, "Hello World");
    }

    #[test]
    fn test_text_with_font_size() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function render(ctx)
                ctx:text("标题", {font_size=18})
            end
            "#,
        )
        .unwrap();
        let events = rt.capture_render().unwrap();
        let has = events.iter().any(|e| {
            matches!(e, UiEvent::Text { font_size: Some(18.0), .. })
        });
        assert!(has, "text 应指定 font_size=18");
    }

    #[test]
    fn test_text_with_color() {
        let mut rt = TestLuaRuntime::new_test(
            r##"
            state = {}
            function render(ctx)
                ctx:text("红色", {color="#ff0000"})
            end
            "##,
        )
        .unwrap();
        let events = rt.capture_render().unwrap();
        let has = events.iter().any(|e| {
            matches!(e, UiEvent::Text { color: Some(c), .. } if c == "#ff0000")
        });
        assert!(has, "text 应指定 color=#ff0000");
    }

    #[test]
    fn test_button_basic() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function render(ctx)
                ctx:button("提交")
            end
            "#,
        )
        .unwrap();
        let events = rt.capture_render().unwrap();
        let has = events.iter().any(|e| {
            matches!(e, UiEvent::Button { label, enabled: true, .. } if label == "提交")
        });
        assert!(has, "应有 enabled=true 的提交按钮");
    }

    #[test]
    fn test_button_disabled() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function render(ctx)
                ctx:button("保存", {enabled=false})
            end
            "#,
        )
        .unwrap();
        let events = rt.capture_render().unwrap();
        let has = events.iter().any(|e| {
            matches!(e, UiEvent::Button { label, enabled: false, .. } if label == "保存")
        });
        assert!(has, "应有 enabled=false 的保存按钮");
    }

    #[test]
    fn test_progress_bar_basic() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function render(ctx)
                ctx:progress_bar(0.75)
            end
            "#,
        )
        .unwrap();
        let events = rt.capture_render().unwrap();
        let has = events.iter().any(|e| {
            matches!(e, UiEvent::ProgressBar { value, .. } if (*value - 0.75).abs() < 0.01)
        });
        assert!(has, "应有 value≈0.75 的进度条");
    }

    #[test]
    fn test_separator_basic() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function render(ctx)
                ctx:separator()
            end
            "#,
        )
        .unwrap();
        let events = rt.capture_render().unwrap();
        assert!(events.iter().any(|e| matches!(e, UiEvent::Separator { .. })), "应有 separator");
    }

    #[test]
    fn test_badge_basic() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function render(ctx)
                ctx:badge("进行中")
            end
            "#,
        )
        .unwrap();
        let events = rt.capture_render().unwrap();
        let has = events.iter().any(|e| {
            matches!(e, UiEvent::Badge { text, .. } if text == "进行中")
        });
        assert!(has, "应有 text=进行中的 badge");
    }

    #[test]
    fn test_card_basic() {
        let mut rt = crate::script_node::lua::LuaRuntime::new(
            r#"
            state = {}
            function render(ctx)
                ctx:card("这是一段笔记")
            end
            "#,
        )
        .unwrap();
        let events = crate::script_node::lua::convert_events_for_test(&rt.capture_render().unwrap());
        let has = events.iter().any(|e| {
            matches!(e, UiEvent::Card { text, .. } if text == "这是一段笔记")
        });
        assert!(has, "应有 card");
    }

    #[test]
    fn test_col_with_children() {
        let mut rt = crate::script_node::lua::LuaRuntime::new(
            r#"
            state = {}
            function render(ctx)
                ctx:col({gap=4, padding={8,8,8,8}}, function(s)
                    s:text("a")
                    s:text("b")
                end)
            end
            "#,
        )
        .unwrap();
        let events = crate::script_node::lua::convert_events_for_test(&rt.capture_render().unwrap());
        assert!(events.iter().any(|e| matches!(e, UiEvent::ColStart { .. })), "应有 ColStart");
        assert!(events.iter().any(|e| matches!(e, UiEvent::ColEnd)), "应有 ColEnd");
        assert_ui_contains(&events, "a");
    }

    #[test]
    fn test_empty_render() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function render(ctx) end
            "#,
        )
        .unwrap();
        let events = rt.capture_render().unwrap();
        assert!(events.is_empty(), "空 render 应返回空列表");
    }

    #[test]
    fn test_complete_ui_composition() {
        let mut rt = crate::script_node::lua::LuaRuntime::new(
            r#"
            state = {}
            function render(ctx)
                ctx:col({gap=8, padding={8,8,8,8}}, function(sub)
                    sub:text("标题", {font_size=18, bold=true})
                    sub:separator()
                    sub:row({gap=8}, function(r)
                        r:button("确认", {bg="$success"})
                        r:button("取消", {bg="$danger"})
                    end)
                end)
            end
            "#,
        )
        .unwrap();
        let events = crate::script_node::lua::convert_events_for_test(&rt.capture_render().unwrap());
        assert_ui_contains(&events, "标题");
        assert_ui_contains(&events, "确认");
        assert_ui_contains(&events, "取消");
        assert!(events.iter().any(|e| matches!(e, UiEvent::Separator { .. })));
        assert!(events.iter().any(|e| matches!(e, UiEvent::ColStart { .. })));
        assert!(events.iter().any(|e| matches!(e, UiEvent::ColEnd)));
    }

    #[test]
    fn test_input_interaction_updates_state_through_render() {
        let mut rt = crate::script_node::lua::LuaRuntime::new(
            r#"
            state = { edit_buffer = "" }
            function render(ctx)
                local text = ctx:input({label="新笔记", value=state.edit_buffer})
                state.edit_buffer = text
            end
            "#,
        )
        .unwrap();

        rt.capture_render().unwrap();
        rt.queue_input_value("新笔记", "hello");
        rt.capture_render().unwrap();

        let value: String = rt.get_state("edit_buffer").unwrap();
        assert_eq!(value, "hello");
    }

    #[test]
    fn test_nested_input_interaction_updates_state_through_render() {
        let mut rt = crate::script_node::lua::LuaRuntime::new(
            r#"
            state = { edit_buffer = "" }
            function render(ctx)
                ctx:col({gap=4}, function(sub)
                    local text = sub:input({label="新笔记", value=state.edit_buffer})
                    state.edit_buffer = text
                end)
            end
            "#,
        )
        .unwrap();

        rt.capture_render().unwrap();
        rt.queue_input_value("新笔记", "nested");
        rt.capture_render().unwrap();

        let value: String = rt.get_state("edit_buffer").unwrap();
        assert_eq!(value, "nested");
    }

    #[test]
    fn test_button_interaction_executes_lua_branch() {
        let mut rt = crate::script_node::lua::LuaRuntime::new(
            r#"
            state = { count = 0 }
            function render(ctx)
                if ctx:button("加一") then
                    state.count = state.count + 1
                end
            end
            "#,
        )
        .unwrap();

        rt.capture_render().unwrap();
        rt.queue_button_click("加一");
        rt.capture_render().unwrap();

        let count: i64 = rt.get_state("count").unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_button_interaction_can_emit() {
        let mut rt = crate::script_node::lua::LuaRuntime::new(
            r#"
            state = { text = "hello" }
            function render(ctx)
                if ctx:button("发送") then
                    emit("saved", state.text)
                end
            end
            "#,
        )
        .unwrap();

        rt.queue_button_click("发送");
        rt.capture_render().unwrap();

        let emits = rt.drain_emits();
        assert_eq!(emits, vec![("saved".to_owned(), "hello".to_owned())]);
    }
}

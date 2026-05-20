// ============================================================================
// BDD: Feature — Lua 调试器 MVP（断点/单步/变量查看）
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::script_node::lua::LuaRuntime;

    #[test]
    fn test_breakpoint_hits_and_reports_pause_line() {
        let code = r#"
state = { n = 0 }
function render(ctx)
  state.n = state.n + 1
  state.n = state.n + 1
  ctx:text("ok")
end
"#;
        let mut rt = LuaRuntime::new(code).expect("runtime created");
        rt.set_breakpoint(4, true).expect("set breakpoint");

        let err = rt.capture_render().expect_err("should break at line 4");
        assert!(err.contains("调试中断") || err.contains("debug breakpoint hit"));

        let line = rt.take_debug_pause_line().unwrap_or(0);
        assert_eq!(line, 4);
    }

    #[test]
    fn test_step_into_pauses_on_next_line() {
        let code = r#"
state = { x = 1 }
function render(ctx)
  state.x = state.x + 1
  ctx:text("step")
end
"#;
        let mut rt = LuaRuntime::new(code).expect("runtime created");
        rt.request_step_into().expect("request step");

        let err = rt.capture_render().expect_err("step should pause");
        assert!(err.contains("调试中断") || err.contains("debug breakpoint hit"));
        assert!(rt.take_debug_pause_line().unwrap_or(0) > 0);
    }

    #[test]
    fn test_debug_variables_snapshot_contains_state_and_filters_internal_globals() {
        let code = r#"
state = { a = 1, name = "demo", nested = { x = 2 } }
my_global = 42
"#;
        let rt = LuaRuntime::new(code).expect("runtime created");
        let vars = rt.debug_variables_snapshot().expect("snapshot");

        assert_eq!(vars["state"]["a"], serde_json::json!(1));
        assert_eq!(vars["state"]["name"], serde_json::json!("demo"));
        assert_eq!(vars["state"]["nested"]["x"], serde_json::json!(2));
        assert_eq!(vars["globals"]["my_global"], serde_json::json!(42));
        assert!(vars["globals"].get("__debug_step").is_none());
    }
}

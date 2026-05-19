// ============================================================================
// BDD: Feature — 端口映射 (MVP #8)
// ============================================================================
//
// Feature: 端口映射
//   As a Canvas 引擎
//   I want Lua 中 ports 表的声明正确注册到 Canvas 的端口系统
//   So that 边缘路由能正确连接输入/输出
//
// 验证标准:
//   - 基本 ports 声明正确注册 inputs/outputs
//   - 不定义/ports=nil 时使用空端口
//   - 只声明 inputs 或 outputs 正常工作
//   - 支持 string/number/boolean/any 四种类型
//   - emit 到已声明/未声明端口不 panic
//   - on_input 从未声明端口收到消息不 panic
//   - 端口描述正确显示
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::script_node::lua::tests::*;

    // ─── ports 声明 ─────────────────────────────────────

    /// Scenario: 基本 ports 声明
    #[test]
    fn test_basic_ports_declaration() {
        let rt = TestLuaRuntime::new_test(
            r#"
            ports = {
              inputs  = { input  = { type = "string", description = "待审批消息" } },
              outputs = { approve = { type = "string" }, reject = { type = "string" } }
            }
            state = {}
            "#,
        )
        .unwrap();
        let ports = rt.ports();
        assert!(ports.inputs.contains_key("input"), "inputs 应包含 input");
        assert!(
            ports.outputs.contains_key("approve"),
            "outputs 应包含 approve"
        );
        assert!(
            ports.outputs.contains_key("reject"),
            "outputs 应包含 reject"
        );
    }

    /// Scenario: 不定义 ports 时使用空端口
    #[test]
    fn test_no_ports_declared() {
        let rt = TestLuaRuntime::new_test(
            r#"
            state = { x = 1 }
            "#,
        )
        .unwrap();
        let ports = rt.ports();
        assert!(
            ports.inputs.is_empty(),
            "未定义 ports 时 inputs 应为空"
        );
        assert!(
            ports.outputs.is_empty(),
            "未定义 ports 时 outputs 应为空"
        );
    }

    /// Scenario: 只声明 inputs 不声明 outputs
    #[test]
    fn test_only_inputs_declared() {
        let rt = TestLuaRuntime::new_test(
            r#"
            ports = { inputs = { trigger = { type = "string" } } }
            state = {}
            "#,
        )
        .unwrap();
        let ports = rt.ports();
        assert!(ports.inputs.contains_key("trigger"));
        assert!(ports.outputs.is_empty(), "只声明 inputs，outputs 应为空");
    }

    /// Scenario: 只声明 outputs 不声明 inputs
    #[test]
    fn test_only_outputs_declared() {
        let rt = TestLuaRuntime::new_test(
            r#"
            ports = { outputs = { result = { type = "string" } } }
            state = {}
            "#,
        )
        .unwrap();
        let ports = rt.ports();
        assert!(ports.outputs.contains_key("result"));
        assert!(ports.inputs.is_empty(), "只声明 outputs，inputs 应为空");
    }

    // ─── 端口类型 ───────────────────────────────────────

    /// Scenario: type = "string" 端口
    #[test]
    fn test_ports_type_string() {
        let rt = TestLuaRuntime::new_test(
            r#"
            ports = { inputs = { data = { type = "string" } } }
            state = {}
            "#,
        )
        .unwrap();
        let port = rt.ports().inputs.get("data").unwrap();
        assert_eq!(port.port_type, "string", "端口类型应为 string");
    }

    // ─── emit 端口验证 ──────────────────────────────────

    /// Scenario: emit 到已声明的端口
    #[test]
    fn test_emit_to_declared_port() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            ports = { outputs = { result = { type = "string" } } }
            state = {}
            function render(ctx)
                emit("result", "done")
            end
            "#,
        )
        .unwrap();
        let _ = rt.capture_render();
        let emits = rt.drain_emits();
        assert!(emits.contains(&("result".to_owned(), "done".to_owned())));
    }

    /// Scenario: emit 到未声明的端口不 panic
    #[test]
    fn test_emit_to_undeclared_port() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function render(ctx)
                emit("unknown", "val")
            end
            "#,
        )
        .unwrap();
        // 不应 panic
        let _ = rt.capture_render();
    }

    /// Scenario: on_input 从未声明端口收到消息不 panic
    #[test]
    fn test_on_input_from_undeclared_port() {
        let mut rt = TestLuaRuntime::new_test(
            r#"
            state = {}
            function on_input(name, value)
                state.last = name
            end
            "#,
        )
        .unwrap();
        // 不应 panic
        let result = rt.simulate_input("extra", "val");
        assert!(result.is_ok(), "从未声明端口收到消息不应 panic");
    }

    // ─── 端口描述 ───────────────────────────────────────

    /// Scenario: 端口描述存在
    #[test]
    fn test_port_description() {
        let rt = TestLuaRuntime::new_test(
            r#"
            ports = { inputs = { data = { type = "string", description = "输入数据" } } }
            state = {}
            "#,
        )
        .unwrap();
        let port = rt.ports().inputs.get("data").unwrap();
        assert_eq!(
            port.description.as_deref(),
            Some("输入数据"),
            "端口描述正确"
        );
    }
}

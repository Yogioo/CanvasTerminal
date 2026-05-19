# Script Node V2 — 待办事项

## ✅ 已完成 (Phase 1-2)

- [x] `mlua` 依赖集成（lua54 + vendored）
- [x] LuaRuntime 主结构体（生命周期管理）
- [x] 沙箱安全（白名单/黑名单/内存限制）
- [x] state ↔ JSON 序列化/反序列化
- [x] ctx.* 渲染 API（text/button/input/slider/progress_bar/separator/badge/card/spacer/col/row）
- [x] 系统 API（emit/set_timer/clear_timer/get_timer_interval/log）
- [x] TimerManager（poll/set/clear）
- [x] BDD 测试框架 179 个场景（172 通过，7 个 mock 限制待迁移）

## 🔴 Phase 3: 生命周期集成（~200 行）

- [x] 修改 `canvas_nodes_render.rs::draw_script_node_body()` 用 LuaRuntime 替换 V1 JSON Widget 渲染（已接入 `capture_render()`，基础 UI 事件映射完成）
- [x] `before_frame()` / `after_frame()` 钩子集成到 egui 帧循环
- [x] 消息路由：`on_input()` 调用 + `emit()` 转发到下游端口
- [x] 定时器：与 `ctx.request_repaint_after()` 集成
- [x] 将 Lua 编辑器界面接入（编辑时显示 Lua 代码，退出编辑时执行）
- [x] 按钮事件键精确映射（Button 已支持显式 `event_key`，缺省回退 `label`）

## 🔴 Phase 4: 编辑器体验（~150 行）

- [x] Lua 语法高亮（已替换编辑器 layouter，支持 Lua 关键字/注释/字符串/数字高亮）
- [x] 运行时错误 → 红色错误面板（已显示 Lua Runtime/Hook/Render 错误）
- [x] 编译错误分类展示（SyntaxError/RuntimeError/HookError 已分类显示到错误面板）
- [x] 右键菜单 → "插入代码片段"（番茄钟/笔记/审批）
- [x] 默认模板：创建新 Script 节点时使用 Lua 版审批队列模板

## 🔴 Phase 5: 废弃 V1 代码（清理）

- [x] 删除 `parser.rs`（JSON 解析器）
- [x] 删除 `default_script_template()`（V1 JSON 模板）
- [x] 清理 `types.rs` 中不再使用的公开接口
- [x] 确保 `layout_render.rs` 的渲染函数只被 Lua 模块调用（现已移除 V1 渲染模块文件）

### Phase 5 完成备注（2026-05-19）
- 已删除：`src/script_node/parser.rs`、`src/script_node/types.rs`、`src/script_node/layout_render.rs`
- 已清理：`NodeData::Script.parsed_spec` 及其所有调用链
- 已替换：新建 Script 节点默认模板改为 `script_snippet_approval_queue()`
- 验证：`cargo check` 通过（仅剩 Lua 模块 dead_code 警告）

## 🟡 待优化 / 待修复

- [ ] **BDD 测试迁移**：将 7 个因 mock 限制失败的测试改用真实 `LuaRuntime`（mlua）验证
  - [ ] 梳理并列出 7 个失败用例名称 + 失败原因（mock 限制点）
  - [ ] 为每个用例补齐最小可运行 Lua 脚本夹具（ports/state/hooks）
  - [ ] 将 mock runtime 替换为真实 `LuaRuntime::new/new_with_state`
  - [ ] 覆盖关键生命周期：`before_frame` / `capture_render` / `after_frame` / `advance_tick`
  - [ ] 补充 emit、timer、state 序列化断言，去除对内部 mock 行为的耦合断言
  - [ ] 单测分组执行并记录通过率（目标：7/7 通过）
  - [ ] 全量测试回归并记录结果到本文件
- [ ] **大 state 性能优化**：仅当 state 修改时才序列化（dirty flag 已留接口）
- [ ] **日志限流**：`log()` 频繁调用时防止内存泄漏（当前无上限）
- [ ] **超时机制**：帧级执行超时（5ms），防止 Lua 脚本长时间阻塞 UI
- [ ] **指令计数 hook**：mlua `HookTriggers` 需要 `debug` feature，当前未启用

## 🟢 未来可能

- [ ] 代码片段库：内置更多模板（仪表盘、定时器、表单等）
- [ ] Lua 调试器：断点/单步/变量查看
- [ ] 在画布上显示 Lua 节点运行状态（running/frozen/error）

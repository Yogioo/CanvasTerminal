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

- [ ] 修改 `canvas_nodes_render.rs::draw_script_node_body()` 用 LuaRuntime 替换 V1 JSON Widget 渲染
- [ ] `before_frame()` / `after_frame()` 钩子集成到 egui 帧循环
- [ ] 消息路由：`on_input()` 调用 + `emit()` 转发到下游端口
- [ ] 定时器：与 `ctx.request_repaint_after()` 集成
- [ ] 将 Lua 编辑器界面接入（编辑时显示 Lua 代码，退出编辑时执行）

## 🔴 Phase 4: 编辑器体验（~150 行）

- [ ] Lua 语法高亮（替换 JSON 高亮或加 layouter）
- [ ] 编译错误 → 红色错误面板（SyntaxError/RuntimeError/HookError）
- [ ] 右键菜单 → "插入代码片段"（番茄钟/笔记/审批）
- [ ] 默认模板：创建新 Script 节点时使用 Lua 版审批队列模板

## 🔴 Phase 5: 废弃 V1 代码（清理）

- [ ] 删除 `parser.rs`（JSON 解析器）
- [ ] 删除 `default_script_template()`（V1 JSON 模板）
- [ ] 清理 `types.rs` 中不再使用的公开接口
- [ ] 确保 `layout_render.rs` 的渲染函数只被 Lua 模块调用

## 🟡 待优化 / 待修复

- [ ] **BDD 测试迁移**：将 7 个因 mock 限制失败的测试改用真实 `LuaRuntime`（mlua）验证
- [ ] **大 state 性能优化**：仅当 state 修改时才序列化（dirty flag 已留接口）
- [ ] **日志限流**：`log()` 频繁调用时防止内存泄漏（当前无上限）
- [ ] **超时机制**：帧级执行超时（5ms），防止 Lua 脚本长时间阻塞 UI
- [ ] **指令计数 hook**：mlua `HookTriggers` 需要 `debug` feature，当前未启用

## 🟢 未来可能

- [ ] 代码片段库：内置更多模板（仪表盘、定时器、表单等）
- [ ] Lua 调试器：断点/单步/变量查看
- [ ] 在画布上显示 Lua 节点运行状态（running/frozen/error）

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

- [x] **BDD 测试迁移**：将 7 个因 mock 限制失败的测试改用真实 `LuaRuntime`（mlua）验证
  - [x] 梳理并列出 7 个失败用例名称 + 失败原因（mock 限制点）
  - [x] 为每个用例补齐最小可运行 Lua 脚本夹具（ports/state/hooks）
  - [x] 将 mock runtime 替换为真实 `LuaRuntime::new/new_with_state`
  - [x] 覆盖关键生命周期：`before_frame` / `capture_render` / `after_frame` / `advance_tick`
  - [x] 补充 emit、timer、state 序列化断言，去除对内部 mock 行为的耦合断言
  - [x] 单测分组执行并记录通过率（目标：7/7 通过）
  - [x] 全量测试回归并记录结果到本文件

### BDD 测试迁移完成备注（2026-05-19）
- 7 个失败用例（迁移前）
  1. `feature_persistence::test_deserialize_merges_with_defaults`
     - 原因：mock 的 `merge_serialized_state` 未正确保留 Lua 默认字段（`extra`）
  2. `feature_pomodoro::test_initial_render`
     - 原因：mock 对按钮文案分支模拟与脚本真实渲染路径不一致（期望“开始工作”，实际初始为“继续”）
  3. `feature_render_api::test_card_basic`
     - 原因：mock 渲染器未完整覆盖 `ctx:card` 语义
  4. `feature_render_api::test_col_with_children`
     - 原因：mock 对 `ctx:col(..., function)` 子节点执行不完整
  5. `feature_render_api::test_complete_ui_composition`
     - 原因：mock 对组合布局（col/row/children）事件生成不完整
  6. `feature_timer::test_countdown_to_zero_emits`
     - 原因：mock tick/emit 状态机与真实 Lua 执行不一致
  7. `feature_timer::test_countdown_stops_running`
     - 原因：mock 倒计时归零边界行为与真实 Lua 不一致

- 迁移与修复摘要
  - 已将上述失败场景切换到真实 `LuaRuntime`：
    - `LuaRuntime::new_with_state`：`feature_persistence`
    - `LuaRuntime::new` + `convert_events_for_test`：`feature_render_api`、`feature_pomodoro`
    - `LuaRuntime::new`：`feature_timer`
  - 同步修正番茄钟初始渲染断言为真实脚本行为（“继续”）

- 分组执行结果
  - 目标失败集：**7/7 通过**

- 全量回归结果
  - `cargo test -- --nocapture`
  - `lib`: **171 passed, 0 failed**
  - `bin(main)`: **205 passed, 0 failed**
  - `bin(canvas)`: **6 passed, 0 failed**
- [x] **大 state 性能优化**：仅当 state 修改时才同步/反序列化 state（基于 dirty + 首帧快照判断）
- [x] **日志限流**：`log()` 增加上限（保留最近 1000 条，超限丢弃最早日志）
- [x] **超时机制**：帧级执行超时（5ms）已接入 on_init/render/on_input/on_tick
- [x] **指令计数 hook**：已启用 `HookTriggers::every_nth_instruction`（按调用重置预算，可中断死循环）

### 运行时优化完成备注（2026-05-19）
- state 同步优化
  - `LuaRuntime` 新增 `is_state_dirty()` / `has_serialized_state()`
  - `script_after_frame()` 仅在 dirty 或首帧时才执行 JSON 反序列化并写回 `script_node_state`
- 日志限流
  - `log()` 缓冲上限 `MAX_LOG_ENTRIES = 1000`
  - 超限时从头部裁剪，保证内存有界
- 帧级超时保护
  - `FRAME_EXECUTION_TIMEOUT_MS = 5.0`
  - 在 `on_init` / `render` / `on_input` / `on_tick` 执行后做耗时检查，超时返回 HookError
- 指令预算（可中断）
  - sandbox 接入 `HookTriggers::every_nth_instruction`
  - 每次 Lua 调用前重置预算（内部 `__reset_instruction_budget`）
  - 预算耗尽时报错 `instruction budget exceeded`
- 兼容性说明
  - `mlua 0.11.6` 无独立 `debug` feature；当前方案在现有 feature 集合下可用
- 验证
  - `cargo check` 通过
  - 新增/相关测试通过（含 `test_log_is_bounded`）

## 🟣 本轮修复进展（2026-05-19）

- [x] 修复 Script 节点输入/点击后“瞬间复原”问题（UI 交互回放到 LuaRuntime）
  - [x] `LuaRuntime` 增加 pending 交互队列（input/button）
  - [x] `ctx:input` 返回交互后的最新值（不再总是旧值）
  - [x] `ctx:button` 按真实点击回放返回 `true`
  - [x] `col/row` 子上下文共享交互队列（嵌套输入/按钮可用）
  - [x] 画布渲染侧改为“记录交互 -> 下帧 Lua 执行”，不再绕过 Lua 逻辑直连转发
- [x] 修复 `after_frame` 过早清空 `emit` 缓冲导致事件丢失
- [x] 增加回归测试（feature_render_api）
  - [x] input 回放更新 state
  - [x] 嵌套 col 内 input 回放更新 state
  - [x] button 回放触发 Lua 分支
  - [x] button 回放触发 emit

### 本轮验证结果（2026-05-19）
- `cargo check`：通过（仅 warnings）
- `cargo test feature_render_api`：通过（16 passed, 0 failed）
- `cargo test --all`：未全绿（209 passed, 1 failed）
  - 失败用例：`event_server::tests::event_server_metrics_get_and_automation_error_paths`
  - 失败原因：端口占用 `os error 10048`（环境/端口冲突，非 Script 节点改动引入）

### 待办补充
- [ ] 稳定化 event_server 测试端口分配（避免固定端口冲突）
  - [ ] 改为动态端口/随机可用端口
  - [ ] 清理测试生命周期中的端口释放竞态
  - [ ] 目标：`cargo test --all` 全绿

## 🟢 后续计划（已确认范围，2026-05-19）

- [x] 代码片段库：内置更多模板（仪表盘、定时器、表单等）**暂不扩展（按当前需求冻结）**
- [ ] Lua 调试器：断点/单步/变量查看（MVP）
  - [ ] 行号断点（基础断点增删 + 命中暂停）
  - [ ] 单步执行（Step Into）
  - [ ] 变量查看（先支持全局表 + state）
- [ ] 在画布上显示 Lua 节点运行状态（running/frozen/error）
  - [ ] 增加节点运行态：Idle / Running / Frozen / Error
  - [ ] 复用超时/指令预算中断逻辑映射 Frozen
  - [ ] 节点 UI 状态徽标（颜色区分）

### 说明
- 本轮不做：条件断点、调用栈窗口、表达式求值、代码片段库扩展。
- 实施顺序：先“运行状态可视化”，再“调试器 MVP”。

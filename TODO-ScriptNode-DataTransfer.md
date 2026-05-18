# Script 节点数据传输功能 — TODO

> 目标：让 Script 节点之间通过画布连线（edges）进行数据流通。

---

## Phase A — 数据路由打通（数据能流进来）

| # | 任务 | 文件 | 说明 |
|---|------|------|------|
| A1 | 上游 → Script 输入端口的数据注入 | `automation.rs`, `terminal.rs` | 当上游节点（Terminal/Text/Decision）产生输出且 edge route_key 匹配 Script 端口名时，写入 `script_node_inputs[node_id][port]` |
| A2 | Script 输出端口 → 下游节点的数据传递 | `canvas_nodes_render.rs` | 当前已有转发逻辑，但需要确认与现有 `inject_message_to_target` 系统整合 |
| A3 | 状态值持久化 | `persistence.rs` | `script_node_state` 按 key-value 存到 graph.json，加载时恢复 |

## Phase B — 端口可视化（用户能看到/连上端口）

| # | 任务 | 文件 | 说明 |
|---|------|------|------|
| B1 | 节点上下侧画入/出口圆点 | `canvas_nodes_render.rs` | 根据 `parsed_spec.ports.inputs/outputs` 在节点上下边沿画小圆点 |
| B2 | 拖拽圆点创建连线 | `canvas_interactions.rs` | 从输出圆点拖到输入圆点自动创建 edge，route_key 设为端口名 |
| B3 | 连线标签显示 route_key | `canvas_draw.rs` | 端口连线上的标签默认显示端口名（而不是空或需手动输入） |

## Phase C — 模板 & 体验

| # | 任务 | 文件 | 说明 |
|---|------|------|------|
| C1 | 默认模板加入示例端口 | `mod.rs` → `default_script_template()` | 默认 JSON 加一个 `input` 和一个 `output` 端口 |
| C2 | 创建节点时自动调整尺寸 | `nodes.rs` → `create_script_node()` | 根据 ports/body 的复杂度初始尺寸自适应 |
| C3 | 端口值实时显示 | `layout_render.rs` | 输入框/滑块的值实时同步到输出端口 |
| C4 | 编辑器语法高亮 | `canvas_nodes_render.rs` | JSON 编辑器加简单高亮（或最小化） |

## Phase D — 高级

| # | 任务 | 文件 | 说明 |
|---|------|------|------|
| D1 | Slider 值写回输出端口 | `layout_render.rs` | 当前 slider 事件已触发，但需要确认输出传递到下游 |
| D2 | Button event 路由 | `canvas_nodes_render.rs` | 按钮点击的 event_key 通过 `"event"` route 传递，需要端口声明 |
| D3 | Input 实时同步到 `script_node_outputs` | `layout_render.rs` | 当前 InputChange 已触发出站更新 |
| D4 | 多个 Script 节点级联数据流 | 系统集成测试 | 两个 Script 节点 A→B 连线，A 的按钮触发 B 更新 |

---

## 当前状态（2026-05-18）

- ✅ `NodeKind::Script` 新增
- ✅ JSON 解析 → WidgetTree
- ✅ 布局引擎 + egui 渲染
- ✅ 按钮/滑块/输入框交互
- ✅ 右键菜单创建 + 编辑
- ✅ `script_node_inputs/outputs` 存储结构
- ✅ 输出 → 下游的转发代码框架（需确认完整）
- ❌ 上游数据注入未接入
- ❌ 端口可视化未实现
- ❌ 默认模板没有端口示例

**优先级建议：A1 → B1 → B2 → C1 → A3 → C3 → D1~D4**

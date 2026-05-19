# Script 节点功能演进 — TODO

## 已知 Bug

### B1: 设置按钮面板与 JSON 按钮互不关联 ✅

- [x] **保存按钮配置后，JSON 模板中的按钮（批准/全部批准/驳回）没有同步更新**
  - 修复：从默认 JSON 模板中移除了硬编码按钮，按钮现在统一由 `NodeData.Script.buttons` 控制渲染
  - `default_script_template()` 不再包含按钮 widget
  - `canvas_nodes_render.rs` 中按钮渲染逻辑改为：有自定义按钮就渲染自定义按钮，没有则渲染默认 fallback 按钮（批准/全部批准/驳回）
  - 同时添加了"点击右上角「设置按钮」自定义"提示文字
- [x] **已保存的按钮在下次打开编辑器时应该回填**
  - `start_script_buttons_edit()` 从 `NodeData.Script.buttons` 读取，逻辑正确且已验证

### B2: 按钮设置窗口高度异常 ✅

- [x] 窗口初始高度从 260 → 200
- [x] 滚动区域 max_height 从 280 → 320
- [x] 已存在 max_size (600×400)/min_size (360×180) 限制

### B3: 编译警告（未使用变量） ✅

- [x] `src/script_node/mod.rs:87` — `input_values` → `_input_values` 前缀标记
- [x] `src/app/ui/canvas_nodes_render.rs:1133` — 移除了未使用的 `key` 变量绑定

---

## 待完善功能

### F1: 设置面板与默认模板联动 ✅

已采用方案 B：按钮统一由 JSON 模板驱动，设置面板直接读写 JSON `code`。已移除设置按钮 UI（用户直接编辑 JSON 即可）。

### F2: 按钮配置颜色选择器完善

- [ ] 当前按钮编辑器的颜色按钮点击无弹窗调色板
- [ ] 复用 Decision 节点的 RGB/HSV 调色板 UI

### F3: 默认模板 + 按钮事件名自动同步 ✅

- [x] `fetch_script_node_spec()` 现在会检查节点的 `NodeData.Script.buttons`
- [x] 如有自定义按钮，其 event_key 自动注入到 spec 的 `ports.outputs` 中
  - 已存在的端口不会被覆盖（仅添加新事件名）
  - 新端口的 description 自动设为"按钮「label」触发"

### F4: 按钮样式增强 ✅

- 默认模板按钮已美化：白色粗体文字，✓/✕ 前缀，圆角 8px
- 工具栏背景更深、添加分割线，"队列"按钮有圆角
- 主题色调整：`accent` #7ec8e3, `danger` #ef5350, `text` #e8e6f0

---

## 代码质量

### Q1: 函数太长 ✅ (部分)

- [x] Script 节点渲染分支已提取到 `draw_script_node_body()`（417 行独立方法）
- [x] `draw_nodes()` 从 ~1142 行降至 ~755 行
- [ ] 后续可进一步提取 Terminal/Decision 等分支

### Q2: 变量命名 ✅

- [x] `clicked_review` → `deferred_review`
- [x] `clicked_config` → `deferred_config`
- [x] `clicked_button_event` → `deferred_button`

# Script 节点功能演进 — TODO

## 已知 Bug

### B1: 设置按钮面板与 JSON 按钮互不关联

- [ ] **保存按钮配置后，JSON 模板中的按钮（批准/全部批准/驳回）没有同步更新**
  - 现象：通过"设置按钮"面板配置的按钮保存在 `NodeData.Script.buttons`，但界面仍然展示 JSON 模板中定义的按钮
  - 根因：按钮渲染有两套体系：
    1. `Widget::Button`（通过 JSON 模板的 `body.children` 定义）— 由 `render_script_node` 渲染
    2. `NodeData.Script.buttons`（通过设置面板配置）— 由 `canvas_nodes_render.rs` 在 JSON 树下方额外渲染
  - 两套互不覆盖，用户期望设置面板的按钮**替换** JSON 模板中的按钮
- [ ] **已保存的按钮在下次打开编辑器时应该回填**
  - 当前 `start_script_buttons_edit()` 从 `NodeData.Script.buttons` 读取，逻辑正确，但需确认按钮配置保存后能否正确展示

### B2: 按钮设置窗口高度异常

- [ ] 窗口初始高度可能仍偏大
- [ ] 手动缩小后存在内容撑大窗口的倾向（已添加 `max_size` / `max_height`，需验证是否彻底修复）

### B3: 编译警告（未使用变量）

- [ ] `src/script_node/mod.rs:87` — `input_values` 在 `process_script_events()` 中不再使用（ButtonClick 不再 emit output）
- [ ] `src/app/ui/canvas_nodes_render.rs:1133` — `key` 在按钮渲染中未使用（原用于语义颜色推断，现已改用 `color_rgb`）

---

## 待完善功能

### F1: 设置面板与默认模板联动

策略选择：
- **方案 A**（推荐）：默认模板移除硬编码按钮，改为从 `NodeData.Script.buttons` 动态生成按钮行
  - 在没有手动配置按钮时，默认模板保留现在的 `批准/全部批准/驳回` 作为示范
  - 一旦用户通过设置面板配置了按钮，覆盖/替换默认按钮
- **方案 B**：设置面板直接修改 JSON 模板中的 `body.children` 按钮数组
  - 更彻底，但 JSON 操作复杂易出错

### F2: 按钮配置颜色选择器完善

- [ ] 当前按钮编辑器的颜色按钮点击无弹窗调色板
- [ ] 复用 Decision 节点的 RGB/HSV 调色板 UI

### F3: 默认模板改进

- [ ] 默认 JSON 模板中的 `ports.outputs` 应与设置面板的按钮事件名自动同步
  - 当前默认模板写死了 `approve`/`reject` 作为输出端口
  - 用户设置按钮后，输出端口定义应自动匹配

### F4: 按钮样式增强

- [ ] 支持 `process_all` 按钮在设置面板中作为一个 toggle（当前硬编码为 `false`）
- [ ] 按钮可设置圆角、边框等

---

## 代码质量

### Q1: 函数太长

- `canvas_nodes_render.rs` 中的 Script 节点渲染分支已远超 600 行，应考虑拆分

### Q2: 变量命名

- `clicked_review`、`clicked_config`、`clicked_button_event` 等 deferred flag 应统一命名风格

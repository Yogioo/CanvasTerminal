# MSDF 文本渲染后续计划

本文记录当前 MSDF 文本渲染改造的已完成状态、待测试项和后续任务。原始总体方案见 `docs/msdf-text-rendering-plan.md`。

## 当前状态

已完成：

1. **M1：Wgpu 后端**
   - `eframe` 已启用 `wgpu` feature。
   - 应用 renderer 已切换为 `eframe::Renderer::Wgpu`。
   - 用户已手动验证窗口、Canvas、Terminal、图片节点正常。

2. **M2：egui-wgpu PaintCallback 管线**
   - 自定义 `egui_wgpu::CallbackTrait` 管线已跑通。

3. **M3：MSDF atlas + shader MVP**
   - 已下载 `msdf-atlas-gen`。
   - 使用 `C:/Windows/Fonts/NotoSansSC-VF.ttf` 生成小字符集 MVP atlas。
   - 已实现 atlas JSON/PNG 加载、MSDF shader、基础 glyph quad layout。
   - 已修复：
     - `distanceRange` / `planeBounds` / `atlasBounds` JSON 字段解析。
     - glyph em 缩放错误导致文字过小的问题。
     - Y 方向颠倒问题。

4. **M4 第一片：Edge route label**
   - Edge route label 已从 egui `painter.text` 切换到 MSDF 渲染。
   - 已修复多 label 共用 `CallbackResources` 时互相覆盖的问题。
   - 已修复旧字号最小值导致 zoom out 后不继续缩小的问题。
   - 用户已验证 edge label 显示、位置、缩放、多 label 正常。

5. **M4 第二片：节点标题**
   - Terminal / Decision / Group / Script 节点标题已切换到 MSDF 渲染。
   - 编辑态仍保留原 egui TextEdit。
   - 已移除 MSDF 标题里的旧 egui 视觉最小字号 clamp。
   - 待用户手动验证。

当前限制：

- atlas 仍是 MVP 小字符集，不覆盖完整中文。
- 当前渲染仍是“一段文字一个 PaintCallback”，尚未合批。
- 当前布局仅支持简单单行文本；无 wrap、clip、多 span、shaping、fallback。
- Markdown、TextEdit、Terminal 内容仍未替换。

---

## 当前待手动测试

用户测试 M4 第二片节点标题：

1. Terminal 节点标题是否显示正常。
2. Decision 节点标题是否显示正常。
3. Group 节点标题是否显示正常。
4. Script 节点标题是否显示正常。
5. 连续 zoom 时标题是否随 Canvas 缩放，不再卡最小字号。
6. 双击标题进入编辑态是否正常。
7. 编辑后退出展示态是否正常。
8. 节点选择、拖动、resize 是否不受影响。
9. 中文标题缺字表现是否可接受（当前可能 tofu/方块）。

如果发现问题，优先修：

- 位置/基线偏移。
- 字号过大/过小。
- 缺字导致关键默认标题不可读。
- 编辑态切换异常。

---

## 后续任务总览

建议按以下顺序推进。

### P0：稳定现有 M4 改造

目标：让 edge label 和第一批节点标题稳定可用。

任务：

1. 根据用户手动测试反馈微调节点标题：
   - baseline offset。
   - left/top padding。
   - 字号比例。
   - callback rect 是否过大/过小。
2. 确认编辑态仍使用 egui TextEdit，且编辑框位置与标题基本对齐。
3. 检查 zoom 极小/极大时是否有 panic、闪烁、文字异常拉伸。
4. 确认多节点、多 edge label 同屏时不会互相覆盖。

验收：

- `cargo check` 通过。
- 用户确认 edge label 与 4 类节点标题视觉基本正常。
- 不影响已有交互。

---

### P1：扩充 MVP charset

目标：让默认节点标题、常见中文 UI 文案和用户常用标题不容易缺字。

任务：

1. 汇总项目内静态中文/英文字符：
   - `src/**/*.rs`
   - `assets/**/*.ps1`
   - 默认节点标题和状态文案。
2. 合并 ASCII printable、中文标点、常用符号。
3. 生成中等大小 atlas：
   - 不直接上 3500/7000 全量。
   - 先以“项目静态字符 + 常见标题字符”为目标。
4. 更新 `assets/fonts/msdf/charset.txt`、`atlas.png`、`atlas.json`、README 生成命令。
5. 验证 atlas 大小可接受。

验收：

- 现有默认标题/常见 UI 文案基本不缺字。
- atlas 文件大小仍可接受。
- `cargo check` 通过。

注意：

- 不复制 Microsoft YaHei。
- 继续使用 `NotoSansSC-VF.ttf` 作为本机生成字体来源。
- 如果要随项目发布，需要确认 Noto Sans SC 授权和 atlas 派生物发布策略。

---

### P2：MSDF 绘制合批

目标：降低 “一段文字一个 PaintCallback” 的开销，为更多文本替换做准备。

任务：

1. 设计每帧 MSDF draw queue：
   - 收集多个 label/text run。
   - 一个 callback 统一生成 vertex/index buffer。
   - 一个 pipeline/bind group，少量 draw call。
2. 保持调用方 API 简单，例如：
   - `queue_msdf_label(...)`
   - frame end 处 `paint_queued_msdf(...)`
3. 将 edge labels 和节点标题迁移到队列。
4. 避免全局状态泄漏或跨帧旧 buffer 残留。

验收：

- 多节点、多边 label 同屏正常。
- `cargo check` 通过。
- 不引入明显 FPS 回退。

---

### P3：M4 剩余低风险静态文字

目标：完成第一批静态展示文字替换。

候选：

1. Terminal 状态文字。
2. cwd badge 文字。
3. 节点 resize/hover 辅助提示文字。
4. 其它小型 overlay/hint 文本。

任务：

1. 逐个替换，不批量大改。
2. 每类文字保留原位置、颜色、交互区域。
3. 如果文本可能动态且缺字多，先记录，不强行替换。

验收：

- 第一批低风险静态文字基本走 MSDF。
- zoom 下无明显 egui 字号闪烁。

---

### P4：M5 普通展示内容预览 MVP

目标：开始覆盖节点正文展示态，但不碰编辑态。

优先顺序：

1. Decision button text。
2. Decision queue preview。
3. Text 节点 plain text preview。
4. Script code preview。
5. Lua/script 输出展示文字。

任务：

1. 实现简单多行 layout：
   - 换行符。
   - 简单 wrap。
   - line height。
   - clip rect。
2. 先支持纯色文本。
3. Script code preview 可以先无高亮或少量 span。
4. 编辑态仍使用 egui TextEdit。

验收：

- 非编辑态主要内容在 zoom 下稳定。
- 点击、选择、编辑入口不受影响。
- 大文本不会明显拖慢。

---

### P5：字体与 fallback 策略

目标：明确缺字策略，避免用户标题/内容出现大量 tofu。

可选路线：

1. **中等 atlas + tofu fallback**
   - 最简单。
   - 适合短期。
2. **多 atlas**
   - 拉丁/代码字体：JetBrains Mono。
   - 中文字体：Noto Sans SC。
3. **动态 atlas**
   - 长期方案。
   - 支持用户任意输入。
4. **cosmic-text shaping + glyph-id atlas**
   - 长期推荐。
   - 支持 fallback、BiDi、复杂文本。

建议：

- 短期先做中等 charset。
- 等 M5 基本稳定后再评估 cosmic-text。

---

### P6：Markdown 展示增强

目标：逐步替代 Canvas preview 中的 CommonMark 文本展示。

任务：

1. basic markdown parser/layout：
   - paragraph。
   - heading。
   - list。
   - code block。
   - quote。
2. 支持 scroll/clip。
3. 图片和复杂元素暂时保留 fallback。

验收：

- 常见 markdown 文档展示可接受。
- 不要求完全等价 `egui_commonmark`。

---

### P7：编辑态策略

目标：减少编辑态和展示态切换突兀感，但不重写 TextEdit。

任务：

1. 编辑态仍用 egui TextEdit。
2. 编辑态字号可考虑固定或离散化，减少 zoom 抖动。
3. 保持 IME、复制粘贴、选择、undo/redo 不退化。

验收：

- 编辑体验不受损。
- 非编辑态 MSDF 稳定。

---

### P8：Terminal 文本 MSDF 化（长期）

目标：最终覆盖 terminal 内容文字。

任务：

1. 分析 `vendor/egui_term` 渲染层。
2. 抽取 cell grid / ANSI span。
3. 用 monospace MSDF atlas 绘制 terminal cell 文本。
4. 保留 ANSI color、cursor、selection、scrollback、输入。

验收：

- terminal 基本交互可用。
- ANSI 颜色、光标、选择正常。
- zoom 下 terminal 文字稳定。

---

## Worker 执行规范

后续实现任务优先使用独立 Pi worker，而不是 subagent。

默认命令形态：

```bash
pi --no-session \
  --model deepseek/deepseek-v4-flash \
  --tools read,bash,edit,write \
  -p @/tmp/task_prompt.md
```

复杂任务可用：

```bash
pi --no-session \
  --model deepseek/deepseek-v4-pro \
  --tools read,bash,edit,write \
  -p @/tmp/task_prompt.md
```

建议 timeout：

- 普通实现：20 分钟。
- 大型生成/重构：30 分钟以上。

每个 worker prompt 必须包含：

1. 当前状态。
2. 明确目标。
3. 文件/模块范围。
4. 非目标。
5. 验收标准。
6. `cargo check` 要求。
7. 关键决策 stop rule。

Supervisor 负责：

1. 审查 worker diff。
2. 运行验证命令。
3. 发现问题派 follow-up worker。
4. 关键产品/架构/授权决策问用户。

---

## 下一个推荐任务

等待用户完成 M4 节点标题手动测试。

如果测试通过，推荐下一步：

> **P1：扩充 MVP charset，覆盖项目静态中文和默认标题。**

原因：

- 当前节点标题已经走 MSDF。
- 用户自定义中文标题容易缺字。
- 扩充 charset 是继续替换更多文本前的基础。

如果测试发现节点标题位置/大小问题，则先执行：

> **P0：节点标题视觉微调。**

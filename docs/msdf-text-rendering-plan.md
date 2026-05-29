# CanvasTerminal MSDF 全文字渲染技术计划

## 目标

在 CanvasTerminal 中逐步建立一套以 MSDF/MTSDF atlas 为核心的文字渲染管线，最终让 Canvas 中所有展示文字都通过固定字体 atlas + 自定义 GPU shader 渲染，避免当前随 zoom 动态改变 egui 字号导致的字体闪烁、重排、模糊问题。

核心目标：

1. Canvas 缩放时文字不闪烁。
2. 任意 zoom 下文字边缘尽量锐利稳定。
3. 文本 layout 与 zoom 解耦：字体逻辑尺寸固定，视觉缩放只改变 quad transform。
4. 先覆盖所有静态展示文字；交互复杂的文字组件可后续替换。
5. 尽量保留现有 egui UI 和编辑体验，降低一次性重写风险。

---

## 当前问题分析

当前项目大量 Canvas 内文字使用类似逻辑：

```rust
FontId::proportional((17.0 * zoom_scale).max(9.0))
FontId::proportional((15.0 * zoom_scale).round())
FontId::monospace((13.0 * self.ws.zoom).max(9.0))
```

这会导致：

1. zoom 变化时 egui 持续重新 layout 文本。
2. 不同字号触发 glyph cache / atlas 更新。
3. 文本换行、裁剪、ScrollArea 内容高度可能随 zoom 抖动。
4. 字体像素对齐和采样结果变化，产生闪烁。
5. Markdown、TextEdit、terminal 等复杂 widget 在 canvas 缩放下表现不稳定。

当前 `src/main.rs` 使用：

```rust
renderer: eframe::Renderer::Glow,
```

如果采用 WGSL + egui PaintCallback，需要切换到 wgpu 后端。

---

## 总体方案

建立独立 MSDF 文本系统：

```text
文本内容 / 节点数据
    ↓
MSDF Layout 层
    - 固定 font_size_world
    - 固定 line_height_world
    - 固定 wrap_width_world
    - advance / kerning / shaping
    ↓
Glyph quad instances
    - world rect
    - uv rect
    - color
    - clip rect
    ↓
egui PaintCallback / egui-wgpu
    - camera transform
    - atlas texture
    - vertex/index buffer
    ↓
WGSL shader
    - median RGB distance
    - fwidth / screenPxRange 抗锯齿
```

关键原则：

1. layout 不依赖 zoom。
2. atlas 不因 zoom 变化而更新。
3. zoom 只进入 world-to-screen transform。
4. shader 根据屏幕导数动态计算边缘宽度。
5. 复杂编辑控件可先保留 egui，展示态优先 MSDF。

---

## 渲染后端调整

### 必要变更

将 eframe renderer 从 Glow 切换到 Wgpu：

```rust
renderer: eframe::Renderer::Wgpu,
```

并显式引入匹配版本的依赖：

```toml
egui-wgpu = "0.31"
bytemuck = { version = "1", features = ["derive"] }
```

`wgpu` 版本需要通过 `cargo tree` 确认，与 `eframe/egui-wgpu 0.31` 保持一致。

### 风险

切换 wgpu 后需要回归验证：

- 主窗口、无边框窗口是否正常。
- 图片节点是否正常。
- terminal 是否正常。
- egui_term 是否兼容。
- Windows 显卡/驱动环境是否稳定。
- 是否需要保留 Glow fallback。

---

## Atlas 方案

### 推荐工具

优先使用 `msdf-atlas-gen` 生成 atlas：

```bash
msdf-atlas-gen \
  -font assets/fonts/YourFont.ttf \
  -type mtsdf \
  -format png \
  -json assets/fonts/msdf/atlas.json \
  -imageout assets/fonts/msdf/atlas.png \
  -size 48 \
  -pxrange 4 \
  -charset assets/fonts/msdf/charset.txt
```

推荐 `mtsdf` 而不是纯 `msdf`：

- RGB 通道用于 MSDF。
- Alpha 通道可作为普通 SDF 信息。
- 对小字号、描边、阴影扩展更友好。

### 字符集策略

第一阶段内置静态字符集：

1. ASCII。
2. 常用中文 3500/7000 字。
3. 中文标点、英文标点。
4. 常见数学符号、箭头、框线符号。
5. 项目 UI 中已有常用 emoji/图标字符可酌情加入。

后续增强：

1. 缺字 fallback 到 egui 原生文本。
2. 或增加动态 atlas。
3. 或按字体/字符集拆多个 atlas。

---

## Shader 核心逻辑

不要使用固定宽度的 `smoothstep`。需要根据当前屏幕空间缩放计算边缘宽度。

WGSL 逻辑示意：

```wgsl
fn median(r: f32, g: f32, b: f32) -> f32 {
    return max(min(r, g), min(max(r, g), b));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let sample = textureSample(msdf_tex, msdf_sampler, in.uv);
    let sd = median(sample.r, sample.g, sample.b) - 0.5;

    let unit_range = vec2<f32>(u.px_range) / u.atlas_size;
    let screen_tex_size = vec2<f32>(1.0) / fwidth(in.uv);
    let screen_px_range = max(0.5 * dot(unit_range, screen_tex_size), 1.0);

    let alpha = clamp(sd * screen_px_range + 0.5, 0.0, 1.0);
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
```

注意事项：

1. MSDF atlas 是数据纹理，不应使用 sRGB 采样。
2. sampler 使用 linear + clamp。
3. blend 方式要匹配 egui/wgpu 的 alpha 管线。
4. clip rect 需要和 egui 的裁剪逻辑兼容。

---

## 模块设计

建议新增目录：

```text
src/msdf/
  mod.rs
  atlas.rs
  layout.rs
  renderer.rs
  shader.wgsl
```

### `atlas.rs`

职责：

- 加载 atlas png。
- 解析 atlas json。
- 建立 glyph map。
- 提供 glyph bounds / advance / kerning 查询。

核心结构示意：

```rust
pub struct MsdfAtlas {
    pub atlas_width: f32,
    pub atlas_height: f32,
    pub px_range: f32,
    pub glyphs: HashMap<char, MsdfGlyph>,
    pub kernings: HashMap<(char, char), f32>,
}

pub struct MsdfGlyph {
    pub advance: f32,
    pub plane_bounds: Option<Rect>,
    pub atlas_bounds: Option<Rect>,
}
```

### `layout.rs`

职责：

- 把字符串 layout 成 glyph instances。
- 负责单行、多行、换行、裁剪、颜色分段。
- 第一版可以先用 atlas JSON 的 advance/kerning。
- 后续可接入 cosmic-text 做 shaping/fallback。

核心结构示意：

```rust
pub struct MsdfTextStyle {
    pub font_size_world: f32,
    pub line_height_world: f32,
    pub color: Color32,
}

pub struct MsdfGlyphInstance {
    pub rect_world: Rect,
    pub uv_rect: Rect,
    pub color: Color32,
    pub clip_world: Option<Rect>,
}
```

### `renderer.rs`

职责：

- 管理 wgpu pipeline、texture、bind group。
- 每帧收集 glyph instances。
- 通过 egui PaintCallback 批量绘制。

原则：

- 不要每段文字一个 callback。
- 每帧尽量合批：一个 atlas、一组 vertex/index buffer、少量 draw call。
- callback 中使用当前 canvas camera uniform 完成 transform。

---

## 覆盖范围规划

目标是最终覆盖所有文字，但按难度分层推进。

### 第一批：低风险静态文字

优先替换：

1. Terminal 节点标题。
2. Terminal 状态文字。
3. cwd badge 文字。
4. Decision 节点标题。
5. Script 节点标题。
6. Group 节点标题。
7. Edge label。
8. 节点 resize/hover 辅助提示文字。

特点：

- 多为单行。
- 不需要复杂选择/编辑。
- 不依赖 markdown/terminal 内部布局。
- 最适合验证 MSDF 稳定性。

### 第二批：普通展示内容

替换：

1. Text 节点非编辑状态预览。
2. Decision 节点队列预览文字。
3. Decision 按钮文字。
4. Script 节点代码预览。
5. Lua/script 输出展示文字。

难点：

- 多行 wrapping。
- 裁剪区域。
- 滚动区域。
- 不同颜色 span。
- monospace 对齐。

处理策略：

- 第一版先支持纯文本换行。
- Markdown 可先降级为普通文本或简单解析 heading/code block。
- 语法高亮可以复用现有 token 逻辑，但输出为 MSDF span。

### 第三批：Markdown 完整展示

当前 Text 节点使用 `egui_commonmark::CommonMarkViewer`。完整替换需要：

1. Markdown 解析。
2. block layout。
3. heading/list/code/link/quote 分段样式。
4. 滚动区域。
5. 图片或特殊元素处理。

建议后置。第一阶段可以接受 markdown 预览能力降级，或仅在普通 zoom 下保留 egui markdown viewer，MSDF 作为稳定预览模式。

### 第四批：编辑态文字

包括：

1. TextEdit 单行标题编辑。
2. TextEdit 多行文本编辑。
3. startup script 编辑。
4. working directory 编辑。
5. decision queue 编辑。
6. edge route_key 编辑。

难点：

- 光标。
- 选择区域。
- 输入法 IME。
- 复制粘贴。
- undo/redo。
- 滚动。
- 文本 hit-test。

建议后置。近期策略：

- 非编辑展示态走 MSDF。
- 进入编辑态时临时切回 egui TextEdit。
- 编辑态字体可以先固定尺寸，减少抖动。

### 第五批：Terminal 文本

当前 terminal 依赖 `egui_term`，替换难度最高。

难点：

- ANSI 颜色。
- 光标。
- 选择。
- scrollback。
- monospace cell grid。
- terminal 内部渲染与输入事件。

建议最后处理。可能方案：

1. fork `vendor/egui_term`，替换其文本绘制层。
2. 保留 terminal 背景/交互，文本 cell 由 MSDF renderer 绘制。
3. 使用 monospace atlas + cell grid layout。

---

## 排版路线

### 阶段 A：Atlas JSON layout

适合 MVP。

能力：

- 单行文本。
- 简单多行。
- 简单 kerning。
- 基础中文字符。

优点：实现快，能验证 MSDF 是否解决闪烁。

缺点：

- 不支持复杂 shaping。
- fallback 字体弱。
- emoji/组合字符/BiDi 弱。

### 阶段 B：接入 cosmic-text

长期推荐。

能力：

- font fallback。
- shaping。
- line breaking。
- BiDi。
- unicode 复杂文本。

注意：

cosmic-text 输出通常基于 glyph id，而不是 char。届时 atlas glyph lookup 最好从 char-based 过渡到 font-id + glyph-id based。

### 阶段 C：动态 atlas / 多 atlas

用于解决：

- 生僻字。
- emoji。
- 多字体 fallback。
- 用户自定义字体。

可以后置，不影响第一版验证。

---

## 里程碑计划

### M0：验证当前问题与最小止血

目标：确认闪烁主要来自 zoom 动态字号。

任务：

1. 临时固定部分 canvas 文本字号。
2. 保持节点 geometry 随 zoom 缩放。
3. 观察闪烁是否明显减少。

验收：

- Canvas 缩放时文字闪烁明显缓解。
- 无功能破坏。

### M1：切换 wgpu 后端

目标：让项目具备 WGSL PaintCallback 能力。

任务：

1. `Renderer::Glow` 改为 `Renderer::Wgpu`。
2. 增加必要依赖。
3. 运行 cargo check。
4. 手动验证窗口、canvas、terminal、图片节点。

验收：

- 应用正常启动。
- 现有主要功能可用。
- 无明显渲染异常。

### M2：跑通自定义 PaintCallback

目标：验证 egui-wgpu callback 管线。

任务：

1. 添加最小 wgpu callback。
2. 绘制一个固定颜色 quad。
3. 确认 clip 和层级关系。

验收：

- Canvas 上能显示自定义 quad。
- 不影响 egui 原有绘制。

### M3：加载 MSDF atlas 并绘制固定文本

目标：验证 MSDF shader 和 atlas 正确。

任务：

1. 生成 atlas png/json。
2. 实现 atlas parser。
3. 实现 shader。
4. 绘制固定测试文本：英文 + 中文 + 标点。

验收：

- 测试文本在 zoom 下不闪烁。
- 放大不糊。
- 缩小时边缘稳定。
- 缺字有明确 fallback 表现。

### M4：替换节点标题/边标签

目标：让最常见单行 canvas 文字走 MSDF。

任务：

1. Terminal title/status/cwd。
2. Decision title。
3. Script title。
4. Group title。
5. Edge label。

验收：

- 这些文字不再使用 zoom-scaled `FontId`。
- 缩放时无明显闪烁。
- 位置、颜色、裁剪基本正确。

### M5：替换普通节点内容预览

目标：覆盖主要展示态文字。

任务：

1. Text 节点 plain text preview。
2. Decision queue preview。
3. Decision button text。
4. Script code preview。
5. 支持多行、wrap、clip。

验收：

- 非编辑状态主要文字走 MSDF。
- zoom 下稳定。
- 交互点击区域不受影响。

### M6：Markdown 展示增强

目标：逐步替代 `egui_commonmark` 在 canvas preview 中的文字渲染。

任务：

1. heading。
2. paragraph。
3. list。
4. code block。
5. quote。
6. link 样式。

验收：

- 常见 markdown 文档显示可接受。
- 不要求第一版完全等价 CommonMarkViewer。

### M7：编辑态文本策略

目标：减少编辑态突兀感，但不强行重写 TextEdit。

任务：

1. 编辑态仍用 egui TextEdit。
2. TextEdit 字号固定或有限离散化。
3. 进入/退出编辑态状态切换稳定。

验收：

- 编辑功能不退化。
- 非编辑态 MSDF 稳定。

### M8：Terminal 文本 MSDF 化

目标：最终覆盖 terminal 文本。

任务：

1. 分析 `vendor/egui_term` 渲染层。
2. 抽取 cell grid。
3. 使用 monospace MSDF atlas 绘制 terminal 文本。
4. 保留 ANSI color/cursor/selection。

验收：

- terminal 基本交互可用。
- ANSI 颜色正常。
- 光标和选择正常。
- zoom 下 terminal 文字稳定。

---

## 验收标准

### 视觉验收

1. 连续滚轮 zoom 时，MSDF 文本不发生明显闪烁。
2. 文字放大后边缘锐利，无 bitmap 放大糊感。
3. 文字缩小时边缘稳定，无明显粗细跳变。
4. 节点移动/缩放/拖动画面中文字位置稳定。
5. 高 DPI 屏幕表现正常。

### 功能验收

1. 节点标题显示正确。
2. 文本节点内容预览正确。
3. Decision 按钮文字显示正确且点击区域不变。
4. Script 节点代码预览基本可读。
5. 编辑态功能不被破坏。
6. terminal 功能在未替换阶段不受影响。

### 性能验收

1. 常见 graph 下无明显 FPS 下降。
2. 文本 instances 合批绘制。
3. zoom 时不频繁重建 layout，除非内容或节点尺寸变化。
4. atlas texture 不因 zoom 变化更新。

---

## 风险与应对

### 风险 1：切 wgpu 引入兼容问题

应对：

- 独立分支完成。
- 先只切 renderer，不做 MSDF。
- 保留回滚点。
- 必要时支持 Glow fallback，但 MSDF WGSL 仅在 wgpu 下启用。

### 风险 2：中文字符集不完整

应对：

- 先生成常用中文 atlas。
- 缺字显示 tofu 或 fallback egui。
- 后续做动态 atlas。

### 风险 3：Markdown 完整替换成本高

应对：

- 第一版降级为 plain text / basic markdown。
- 保留 CommonMarkViewer 作为临时 fallback。
- 后续逐步补齐 block layout。

### 风险 4：TextEdit 重写成本过高

应对：

- 编辑态保留 egui TextEdit。
- 非编辑态 MSDF。
- 后续再评估是否自研编辑器。

### 风险 5：Terminal 替换成本最高

应对：

- 放到最后。
- 先不影响 egui_term。
- 后续单独 fork 或局部替换渲染层。

---

## 推荐优先级

短期优先：

1. wgpu callback 跑通。
2. MSDF atlas + shader 跑通。
3. 单行标题/边标签替换。
4. Text/Decision/Script 非编辑预览替换。

中期优先：

1. cosmic-text layout。
2. basic markdown renderer。
3. 缺字 fallback。

长期优先：

1. 动态 atlas。
2. 编辑态文本。
3. terminal 文本。
4. 全局 egui UI 文本替换，若确实有必要。

---

## 最终建议

整体目标可以定为“布局上覆盖所有文字”，但实现顺序必须分层：

1. **展示态文字先全量 MSDF 化。**
2. **编辑态保留 egui，后续逐步替换。**
3. **terminal 最后处理。**
4. **普通 app UI 不建议第一阶段替换。**

这样能最快解决当前 zoom 字体闪烁问题，同时避免一次性重写 egui 文本、Markdown、TextEdit、terminal 带来的巨大风险。

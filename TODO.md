# TODO — CanvasTerminal 后续任务

## ✅ 已完成

### P0 — resize 拉伸修复
- `src/msdf/debug_paint.rs`：修复 MSDF 文本在节点 resize 后的拉伸问题。

### P1 — MSDF charset 扩展
- charset 从 ASCII 扩展到 GB2312 一级常用汉字（~3755 字）。
- 更新 `assets/fonts/msdf/atlas.json`、`atlas.png`、`charset.txt`。
- 新增 `scripts/gen_msdf_charset.py`，支持可复现的 charset 生成。
- `assets/fonts/msdf/README.md` 同步更新。

### P5-A — Dynamic MSDF Atlas 设计
- 完成设计文档：`docs/msdf-dynamic-atlas-design.md`。

### P5-B1 — Runtime MSDF 生成 spike
- 完成 spike：`docs/msdf-runtime-msdf-spike.md`。
- 新增 `src/bin/msdf_spike.rs`，验证 fdsm 依赖 + glyph 生成可行性。
- `Cargo.toml` / `Cargo.lock` 添加 runtime MSDF spike 所需依赖（`fdsm`、`fdsm-ttf-parser`、`ttf-parser`、`nalgebra`）。

### P5-B2 — Dynamic MSDF Atlas 核心实现
- 新增 `src/msdf/dynamic_atlas.rs`：`DynamicMsdfAtlas` 管理 glyph cache、pending queue、row-by-row atlas packing、动态 GPU texture。
- 使用 `fdsm` / `fdsm-ttf-parser` 运行时生成 MTSDF glyph，使用 `fontdb` 查找 CJK fallback 字体，避免硬编码字体路径。
- 支持 `queue.write_texture` 向动态 atlas 局部上传新 glyph，并处理 256-byte row alignment。
- 支持每帧最多生成 2 个 glyph，避免一次性生成大量缺字导致帧率抖动。
- 修复 dynamic atlas UV 坐标系：动态 atlas 使用 packer top-left texture origin，不复用静态 atlas JSON 的 bottom-origin 公式。
- 验收：用户实测静态 atlas 缺失的稀有汉字可运行时生成并正确显示；`cargo check` 通过。

### P5-B3 — 接入节点标题 / Edge Label
- Dynamic Atlas 已接入 `paint_msdf_label`，节点标题与 edge label 共用该绘制路径。
- `debug_paint` 绘制流程已更新为：查 static → 查 dynamic → 缺失则 enqueue/generate → static/dynamic 分组绘制。
- 动态 atlas 不可用时 graceful fallback 到原静态 atlas/tofu 行为。

### P5-C — Fallback 字体链 + GlyphId/FontId key
- `FALLBACK_FAMILIES` 扩展为包含更多 CJK 字体（NotoSans SC/CJK JP/CJK KR、Microsoft YaHei/JhengHei、SimHei、Malgun Gothic）。
- `DynamicMsdfAtlas` 维护 `Vec<FontEntry>` 字体链，每个 glyph 按序查找第一个可用的字体。
- `DynamicGlyphEntry` 增加 `font_index` 字段避免跨字体 glyph 冲突（GlyphId/FontId key）。
- `discover_fallback_font_chain()` 一次扫描加载所有可用 fallback 字体并去重。

### P5-D — measure_text_width_screen 双 atlas 测宽
- `debug_paint.rs` 新增 `measure_text_width_dual()`，同时查静态 + 动态 atlas 计算文本宽度。
- `canvas_render.rs` 中 edge label 居中对齐改用双 atlas 测宽，不再对动态 atlas 中的字符用 tofu 宽度。

### P5-E — 真实 UI 场景验收
- Canvas debug 协议自动化测试通过（data layer PASS）。
- 用户肉眼确认：节点标题、edge label、多 label 同屏、缺字自动补字在实际 UI 中显示正确。
- 验收结论：动态 MSDF atlas 接入全链路稳定。

---

## 🔜 下一步

### 🔭 后续方向

| 任务 | 说明 |
|------|------|
| **非 BMP / Emoji / Complex Shaping** | 未来扩展：emoji 用 color emoji atlas，复杂文字用 shaping engine |
| **P2 — 合批** | MSDF quad 合批减少 draw call，与 dynamic atlas 配合 |
| **静态 charset 继续补强** | 结合项目内常见文案与标题字符，继续提高静态 atlas 覆盖率，减少 tofu 依赖动态补字 |

---

## 🛑 验收与提交纪律

- 每个子任务完成后必须 `cargo check` 通过才能 commit。
- 如果 `cargo check` 失败，定位原因修复，不做强行提交。
- push 前确认 `git status` 不包含意外文件（如 `tmp/` 下的 prompt/png）。
- commit message 格式：`feat(msdf): <具体内容>`

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

---

## 🔜 下一步

### P5-B2 — Dynamic MSDF Atlas 核心实现

**目标**：实现完整的 Dynamic MSDF Atlas，支持运行时向 GPU texture 添加新 glyph。

| # | 子任务 | 说明 |
|---|--------|------|
| 1 | `dynamic_atlas.rs` | 核心数据结构：DynamicMsdfAtlas，管理 glyph cache + atlas packing |
| 2 | fdsm glyph generation | 利用 fdsm/fontdb 从 TTF 生成 MSDF 位图 |
| 3 | Glyph cache | `HashMap<GlyphId, (u32, AtlasRect)>` 记录已生成的 glyph |
| 4 | Atlas packing | 简单的 row-by-row / shelf packing，满足首次可运行即可 |
| 5 | GPU texture + queue.write_texture | 创建 `wgpu::Texture`，新 glyph 写入空闲区域 |
| 6 | Static / Dynamic atlas lookup | 静态 atlas 查不到时 fallback 到 dynamic atlas |
| 7 | 每帧生成节流 | 每帧最多生成 N 个 glyph，避免帧率抖动 |
| 8 | 单字体起步 | 以 NotoSansSC-VF（Variable Font）为唯一 fallback 字体 |

**验收标准**：
- 运行时首次遇到不在静态 atlas 中的汉字时，触发 fdsm 生成并写入 GPU texture，画面正确显示该汉字。
- 后续同字不再重复生成。
- 单帧生成超过 N 个 glyph 时平滑分摊到后续帧。
- 不存在 GPU 同步 / texture 写入时序问题。

**Stop rules**：
- 如果 fdsm 在 Windows 上无法加载 NotoSansSC-VF（路径问题 / fontdb 找不到），报告具体错误，不要硬编码 fallback 路径。
- 如果 queue.write_texture 导致 wgpu validations 报错，停下来分析布局对齐条件。

---

### P5-B3 — 接入节点标题 / Edge Label

- 将 Dynamic Atlas 接入 CanvasNode / EdgeLabel 的文本渲染路径。
- 节点标题和 edge label 使用 dynamic atlas 查缺。
- 更新 debug_paint 的绘制流程：查 static → 查 dynamic → 生成。

---

### 🔭 后续方向

| 任务 | 说明 |
|------|------|
| **Fallback 字体链** | 支持多个 fallback 字体（如 NotoSansSC → NotoSansJP → NotoSansKR）按序查找 |
| **GlyphId / FontId key** | 跨字体区分 glyph，避免不同字体的同名 glyph 冲突 |
| **非 BMP / Emoji / Complex Shaping** | 未来扩展：emoji 用 color emoji atlas，复杂文字用 shaping engine |
| **P2 — 合批** | MSDF quad 合批减少 draw call，与 dynamic atlas 配合 |

---

## 🛑 验收与提交纪律

- 每个子任务完成后必须 `cargo check` 通过才能 commit。
- 如果 `cargo check` 失败，定位原因修复，不做强行提交。
- push 前确认 `git status` 不包含意外文件（如 `tmp/` 下的 prompt/png）。
- commit message 格式：`feat(msdf): <具体内容>`

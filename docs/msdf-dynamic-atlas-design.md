# 动态 MSDF Atlas 设计文档 (P5-A)

> 作者：架构侦察 Worker  
> 日期：2026-05-29  
> 状态：设计提案，待审核  
> 关联计划：`docs/msdf-text-rendering-followup-plan.md` P5

---

## 1. 当前架构摘要（侦察结论）

### 1.1 Atlas 生命周期

- **静态编译期内嵌**：`src/msdf/debug_paint.rs` 通过 `include_bytes!("../../assets/fonts/msdf/atlas.json")` 和 `include_bytes!("../../assets/fonts/msdf/atlas.png")` 将 atlas 编译进二进制。
- **单次初始化**：`init_msdf()` 在 `app.rs` 启动时调用一次，解析 JSON、加载 PNG、创建 GPU pipeline+bind group。
- **全局单例**：`MSDF_ATLAS: OnceLock<MsdfAtlas>` 和 `MSDF_RENDERER: OnceLock<MsdfRenderer>`。

### 1.2 Glyph Lookup

- `MsdfAtlas::glyph(ch: char) -> Option<&MsdfGlyph>` 使用 `HashMap<char, MsdfGlyph>`。
- key 是 **Rust `char` (Unicode scalar value)**，不是 glyph_id 或 font_id+glyph_id 复合 key。

### 1.3 缺字处理现状

- `layout_text_ndc()` 中 `atlas.glyph(ch)` 返回 `None` 时，绘制一个 tofu 方块（宽=font_size*0.5, 高=font_size）。
- **没有任何 fallback 字体**，没有运行时生成。

### 1.4 GPU 资源

- **纹理**：3012×3012 `Rgba8Unorm`，`TEXTURE_BINDING | COPY_DST`，已具备 `COPY_DST` 能力 → 局部更新可行。
- **Bind Group**：texture (binding 0) + sampler (binding 1) + uniform (binding 2)。
- **Pipeline**：固定，每次 atlas 更新需重建 bind group（texture view 改变），但 pipeline 可复用。
- **Per-frame 资源**：`MsdfFrameResourceMap` 管理多个 label 的 vertex/index buffer。

### 1.5 Atlas 资源现状

| 指标 | 值 |
|------|-----|
| 类型 | MTSDF (RGBA) |
| 尺寸 | 3012 × 3012 |
| 字号 | 48 |
| distanceRange | 4 |
| Glyph 数 | 3893 |
| GPU 内存 | ~34.6 MB |
| 空间利用率 | 近乎 100%（max right=3011.5, max top=3011.5） |
| 覆盖范围 | ASCII + GB2312 L1 + 项目静态字符 |

> ⚠️ **关键约束**：当前 atlas **已满**，无法在不扩大或另建 atlas 的情况下添加新 glyph。

### 1.6 调用方

| 调用位置 | 文本 | 阶段 |
|----------|------|------|
| `canvas_nodes_render.rs` | Terminal/Decision/Group 标题 | M4 已完成 |
| `canvas_script_render.rs` | Script 标题 | M4 已完成 |
| `canvas_render.rs` | Edge route label | M4 已完成 |

### 1.7 依赖情况

- **主项目无任何字体 raster/shaping crate**。
- `vendor/egui_term` 间接依赖 `ab_glyph` + `ttf-parser`（通过 alacritty_terminal），但不暴露给主项目。
- 已有 `image` crate (0.25) 可用于 PNG decode/encode。

---

## 2. 动态 Atlas 设计方案

### 2.1 设计目标

1. 用户输入 atlas 未包含字符时，**运行时**生成 glyph、**局部更新** GPU texture、**立即显示**。
2. 不阻塞 UI 帧（生成可延迟 1-2 帧）。
3. 保持现有 MSDF 渲染管线不变。
4. 支持 fallback 字体链。
5. 中长期支持复杂 shaping。

### 2.2 核心架构变化

```text
当前：
  include_bytes!(atlas.png) → decode → GPU texture (static)
  include_bytes!(atlas.json) → HashMap<char, Glyph> (static)

以后：
  include_bytes!(atlas.png) → decode → GPU texture (初始)
  include_bytes!(atlas.json) → HashMap<char, Glyph> (初始)
                                    ↓
  RuntimeGlyphCache (new)
    - char → MsdfGlyph (从初始 atlas 填充)
    - char → pending/ready 状态
    - 从 TTF 运行时生成 MSDF glyph
    - 局部更新 GPU texture sub-region
    - 重建 bind group（当 texture view 不变时可能要 re-bind）
```

### 2.3 API 设计

#### 建议：`ensure_text` 模式（pull 式）

调用方不做异步处理，渲染前声明需要哪些字符：

```rust
/// 在渲染前调用，声明需要的字符。
/// 如果字符已在 cache 中，立即返回 glyph 信息。
/// 如果字符不在 cache 中，标记为 pending，排队生成。
/// 当前帧先显示 tofu，下一帧自动显示真实 glyph。
pub fn ensure_glyphs(atlas: &mut MsdfDynamicAtlas, text: &str);
```

#### 替代方案：`queue_text` 模式（push 式）

```rust
/// 在重建 UI 阶段，收集所有需要显示的文本。
/// atlas 内部 diff 出缺字，排队生成。
/// 下一帧渲染时，新字符已可用。
pub fn queue_text(text: &str);
```

**推荐 `ensure_glyphs`**：更直接，调用方明确知道哪些字符可能缺字。实际上渲染阶段调用 `layout_text_ndc` 时自动做缺字检测，按需排队。

#### 高层 API 建议

不改变 `paint_msdf_label` 签名，内部添加自动缺字检测：

```rust
// 内部伪代码
pub fn paint_msdf_label(..., text, ...) {
    // 1. 检查 text 中所有字符是否在 cache 中
    // 2. 对 cache miss 的字符，排队生成
    // 3. 对已经 ready 的字符，正常 layout 并绘制
    // 4. 对 pending 的字符，当前帧显示 tofu
}
```

或者提供独立的批处理：

```rust
// 在帧开始处调用
msdf_system.ensure_text("所有需要渲染的文本拼接在一起");
// 然后在 paint 调用中，缺字自动不再显示 tofu
```

### 2.4 字形缓存 Key 设计

| 方案 | 说明 | 评价 |
|------|------|------|
| `char` (Unicode scalar) | 当前方案，简单 | ✅ 对单字体足够，不需要 shaping 时推荐 |
| `(font_id, glyph_id)` | 多字体 + shaping 场景 | ❌ 当前无 shaping，过早复杂化 |
| `(font_id, char)` | 折中 | 对 fallback 链友好 |

**推荐**：初期保持 `char` 作为 key，内部 cache map 为 `HashMap<char, GlyphEntry>`。
当引入多字体 fallback 时再改为 `(FontId, char)` 或 `(FontId, GlyphId)`。

理由：
- 当前 atlas 的 glyph 已经用 char 索引。
- 运行时生成的 glyph 也是基于 char（用同一个 Noto Sans SC 或 fallback 字体生成）。
- shaping 集成后需要 glyph_id，但那一步会重新设计 layout 层。

### 2.5 Atlas Packing 方案

#### 背景：当前 atlas 已满（3012×3012，利用率 ~100%）

不可能在现有 atlas 上追加 glyph，必须**另建 atlas**。

#### 新 glyph 的存放策略

**选项 A：固定尺寸第二 atlas**
- 第二 atlas 尺寸 1024×1024 或 2048×2048（按需）。
- 使用简单行式 packing（每行放 glyph，行高=max glyph height）。
- 当 atlas 满时，再建第三 atlas。
- 每个 atlas 有独立的 GPU texture + bind group。

**选项 B：动态增长的 atlas**
- 初始 512×512，满时翻倍（512→1024→2048→4096）。
- 翻倍时拷贝旧数据到新 texture。
- 更适合内存敏感场景，但实现更复杂。

**选项 C：虚拟 atlas（多 atlas 逻辑统一）**
- 逻辑上 "DynamicAtlas" 管理多个物理 atlas texture。
- glyph 分配时选空间足够的 atlas。
- shading 阶段根据 glyph 所在的 atlas 选 bind group。

**推荐：选项 A（固定尺寸第二 atlas），后续按需加第三 atlas。**

理由：
- 实现最简单。
- atlas 满的概率讨论见下方。
- 不需要 texture resize/拷贝。
- GPU 纹理重建成本低。

#### atlas 满的概率估计

GB2312 L1 共 3755 字，当前 atlas 已有 3756 CJK + 140 其他字符，共 3893 glyph，填满 3012×3012。

假设第二 atlas 尺寸 1024×1024，font_size=48，大约可容纳：
- 每个 CJK glyph 平均 ~50×50 ≈ 2500 px²。
- 1024² / 2500 ≈ 419 CJK glyph。
- 考虑 packing 开销，约 300-400 个 CJK glyph。

对于 GB2312 L1 之外的生僻字（L2 ~3000+ 字），1024×1024 可容纳约 10% 覆盖。如果需要覆盖全部 GB2312 L2，建议第三 atlas 尺寸 2048×2048。

#### packing 算法

推荐 **skyline bottom-left** 或 **guillotine** packing：

```rust
struct AtlasPacker {
    width: u32,
    height: u32,
    // skyline: 每列当前最高点
    skyline: Vec<(u32, u32)>, // (x, height)
}

impl AtlasPacker {
    fn pack(&mut self, w: u32, h: u32) -> Option<(u32, u32)>;
}
```

简单行式 packing（每行固定行高）对 CJK（等高等宽）也够用，实现更简单。

### 2.6 GPU Texture 局部更新方案

#### 核心思路

WGSL shader 使用 `texture(texture_2d<f32>, sampler, uv)` 采样。**texture view 不变**，只更新 texture 的部分区域。不需要重建 pipeline 或 bind group。

#### 具体步骤

```rust
// 1. 生成 glyph 位图（MSDF 三通道）
let msdf_bitmap: Vec<u8> = generate_msdf_for_char(ch, &font_data);

// 2. 获取 packing 位置
let (x, y) = packer.pack(glyph_w, glyph_h)?;

// 3. 局部更新 GPU texture
queue.write_texture(
    wgpu::TexelCopyTextureInfo {
        texture: &dynamic_texture,
        origin: wgpu::Origin3D { x, y, z: 0 },
        aspect: wgpu::TextureAspect::All,
        mip_level: 0,
    },
    &msdf_bitmap,
    wgpu::TexelCopyBufferLayout {
        offset: 0,
        bytes_per_row: Some(glyph_w * 4),
        rows_per_image: Some(glyph_h),
    },
    wgpu::Extent3D { width: glyph_w, height: glyph_h, depth_or_array_layers: 1 },
);
```

#### 为什么不需要重建 bind group

wgpu `BindGroup` 持有 `TextureView` 的引用。如果 `TextureView` 不变，只是 texture 内容变化，bind group 无需重建。
只有当 texture 尺寸/格式变化、或新建 atlas 时才需要重建 bind group。

#### 安全性

- `COPY_DST` usage 已在当前 texture 上。
- `queue.write_texture` 是线程安全的，可在任意地方调用。
- 当前帧写入 → 下一帧采样可见。

### 2.7 多 Atlas / Atlas 满了怎么办

#### 设计

`MsdfDynamicAtlas` 管理多个 `AtlasSlot`：

```rust
struct AtlasSlot {
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    packer: AtlasPacker,
    glyph_map: HashMap<char, GlyphEntry>, // char → (atlas_slot_index, atlas_bounds)
    is_static: bool, // true 表示从原始 atlas.png 加载的静态 slot
}

struct MsdfDynamicAtlas {
    slots: Vec<AtlasSlot>,
    // slot 0: 初始静态 atlas (3012×3012, read-only)
    // slot 1+: 动态 atlas (1024×1024 each)
    renderer: MsdfRenderer, // 或每个 slot 有独立 bind group
}
```

#### 渲染时的挑战

每个 `AtlasSlot` 有独立的 GPU texture，但 **pipeline 只有一个**。渲染时每个 glyph 需要知道它属于哪个 atlas slot，使用对应的 bind group。

**解决方案 A：多 bind group，每 glyph 切换**
- 每个 slot 有独立 bind group。
- 渲染时按 slot 分组：先绑定 slot 0 bind group，绘制所有 slot 0 的 glyph quads；再绑定 slot 1，绘制 slot 1 的 quads。
- 需要准备时按 slot 分组 vertex/index buffer。

**解决方案 B：Array Texture**
- 使用 `Texture2DArray`，每层一个 atlas。
- bind group 不变，shader 中通过 `texture(tex_array, sampler, uv, layer)` 采样。
- 需要 `layer` 信息编码到 vertex 中。
- WGPU 支持有限（某些设备可能不支持），复杂度较高。

**解决方案 C：Atlas 合并拷贝（simplified）**
- 定期将多个小 atlas 合并成一个大 atlas（texture copy）。
- 更新 glyph_map 中的 atlas_bounds。
- 只在 glyph 数达到阈值时触发。
- 简单但低频重建。

**推荐：解决方案 A（多 bind group，per-slot 分组绘制）**

理由：
- 实现简单，不需要改 shader。
- 分组绘制对性能影响可忽略（draw call 数仍远小于 label 数）。
- Array texture 方案引入的跨平台风险不值得。

### 2.8 缺字 Fallback 字体链策略

#### 需求

用户输入一个字符，当前字体 Noto Sans SC 没有 → 查 fallback 字体 → 如果 fallback 也没有 → 显示 tofu。

#### 字体链建议

```
Noto Sans SC (主字体, CJK + Latin)
  └→ Segoe UI Symbol (Windows 符号/emoji fallback)
     └→ 系统默认 sans-serif (最后的兜底)
```

#### 实现

```rust
const FONT_FALLBACK_CHAIN: &[&str] = &[
    "C:/Windows/Fonts/NotoSansSC-VF.ttf",
    "C:/Windows/Fonts/seguiemj.ttf",  // Segoe UI Emoji
    "C:/Windows/Fonts/seguisym.ttf",  // Segoe UI Symbol
    "C:/Windows/Fonts/consola.ttf",   // monospace fallback
];
```

需要查询字体中是否有某个字符（codepoint 存在性检查）。可以用 `ttf-parser` crate 的 `Face::glyph_index(ch)` 来判断。

#### 非 MSDF fallback（简化方案）

如果某个字体不支持 MSDF（如 emoji 字体通常没有 MSDF），可以：
1. 用 `fontdue` 或 `ab_glyph` 渲染该字符为 alpha bitmap。
2. 将 bitmap 存入 atlas 的 alpha 通道（普通 SDF 而非 MSDF）。
3. shader 中判断是 MSDF 还是 SDF 模式（通过 pixel 的 RGB=0 标识）。

**或者更简单**：对 emoji fallback，直接退化到 egui 原生文本。但这就走回了 zoom 闪烁的老路。

### 2.9 新依赖评估

| crate | 用途 | 评价 |
|-------|------|------|
| **[skrifa](https://crates.io/crates/skrifa)** | 字体解析（读取 glyph outline） | ✅ 推荐。纯 Rust，Google Fonts 团队维护，解析速度快。用于提取 glyph 轮廓做 MSDF raster。 |
| **[fontdue](https://crates.io/crates/fontdue)** | 字体 raster (bitmap SDF) | ⚠️ 备选。从 TTF 解析并 raster 到 bitmap。但 fontdue 输出的是灰度 bitmap，不是 MSDF。需要额外做 MSDF 转换。 |
| **[ttf-parser](https://crates.io/crates/ttf-parser)** | 轻量字体解析（只读） | ✅ 必选（如果 skrifa 不用的话）。用于 fallback 字体链查询 glyph index。 |
| **[ab_glyph](https://crates.io/crates/ab_glyph)** | 字体 raster + 简单 layout | ❌ 太重且不直接支持 MSDF。 |
| **[cosmic-text](https://crates.io/crates/cosmic-text)** | 完整 shaping + fallback | ✅ **中长期推荐**。支持 BiDi、complex scripts、emoji segmentation、font fallback。但第一阶段不需要。 |
| **[msdfgen / msdf-atlas-gen](https://github.com/Chlumsky/msdfgen)** | 运行时 MSDF 生成 | ⚠️ C++ 库，需要绑定。不推荐。改为 Rust 实现。 |

#### 推荐的最小依赖集

**P5-B (MVP)：`ttf-parser` + 自实现 MSDF raster**

从 font file 中提取 glyph outline（通过 ttf-parser），自实现 MSDF 生成算法（基于 distance transform 或 analytic MSDF）。  
实现难度：中等。MSDF 核心算法约 200 行。

**P5-C (完整)：`skrifa` + 自实现 MSDF raster**

skrifa 提供更现代的 API，对 variable fonts 支持更好。但 ttf-parser 已经够用。

#### 关于自实现 MSDF 生成

msdfgen 的核心算法并非黑魔法：
1. 解析 glyph outline（TTF 的 quadratic bezier curves）。
2. 对每个像素，计算到 outline 边缘的有符号距离（SDF），或到三个子像素点的距离（MSDF）。
3. 使用 multi-channel 技术解决 sharp corners 的精度问题。

对 CJK 字符（大量 curves），analytic 算法（逐像素计算最近边缘）性能大约 0.5-5ms 每 glyph（font_size=48），在可接受范围内。但需要注意 CJK glyph 的 outline 通常较大（上百条 contours）。

### 2.10 中文、生僻字、Emoji、复杂 Shaping

#### 中文
- 已覆盖 GB2312 L1（3755 字），日常 ~99%。
- 生僻字（GB2312 L2 ~3000+ 字，康熙字典 47k+ = 不可能全量）→ 运行时按需生成。

#### Emoji
- **当前策略**：emoji 不在 Noto Sans SC 中，显示 tofu。
- **动态策略**：从 emoji 字体（Segoe UI Emoji / Noto Emoji）中 raster 为 color bitmap 或简单的 SDF。
- **非 MSDF 处理**：emoji 是 color font（CBDT/CBDL/SVG），不适合 MSDF。建议单独 SDF atlas 或退化到 egui 原生。
- ✅ **建议**：第一阶段 emoji 仍显示 tofu。非 MSDF 的 SDF fallback 列入后续。

#### 复杂 Shaping
- **Indic、Arabic、Thai 等**：需要 shaping engine（如 HarfBuzz 或 RustyBuzz）。
- 当前项目没有这些需求。如果将来需要，通过 cosmic-text 集成。
- ✅ **建议**：暂不支持，仅显示缺字。

### 2.11 当前 atlas 满的应对

当前 3012×3012 atlas 利用率 100%。因此：

**必须**在第一阶段即将已有 atlas 标记为 "static frozen"，
新建一个 "dynamic atlas"（建议 1024×1024 × 2）用于存放运行时生成的 glyph。

**如果不想浪费现有 atlas 的扩充空间**，可以考虑：
- 重新生成 atlas.png，用更大尺寸（如 4096×4096）并留空 packing 空间。
- 但这又回到静态生成，不符合动态需求。

**推荐**：保留现有 atlas 作为 slot 0（只读），新建至少 1 个 1024×1024 slot 作为动态扩容区。

---

## 3. 分阶段实施计划

### 3.1 P5-B：最小 MVP —— Runtime Alpha Glyph Fallback

#### 为什么选 Runtime Alpha Glyph Fallback 而不是 Runtime MSDF？

| 维度 | Runtime Alpha (SDF-like) | Runtime MSDF |
|------|--------------------------|--------------|
| 实现难度 | 低 | 高 |
| 字形质量 | 中（SDF vs MSDF 边缘精度差 2-3x） | 高（MSDF 标准质量） |
| 对现有管线影响 | 最小 | 需要 MSDF 生成 pipeline |
| CJK 锐角质量 | corner 模糊 | sharp corner 清晰 |
| 性能 | 快（直接 raster 灰度图） | 慢（需要 multi-distance 计算） |
| 依赖 | `ab_glyph` 或 `fontdue` 直接 raster | 需 ttf-parser + 自实现 MSDF |
| 行数 | ~100-200 行 | ~400-600 行 |

**推荐：P5-B 选 Runtime Alpha (SDF-like) fallback。**

理由：
- 最快的"缺字消失"路径。
- 在小字号（< 20px）时 alpha SDF 和 MSDF 视觉差异很小。
- 缺字大多为生僻字，出现频率低，质量略低可接受。
- MSDF 留到 P5-C 再做。

#### P5-B 具体实现

1. **新增 `font_fallback.rs`**（或作为 `atlas.rs` 的一部分）：
   - 使用 `ttf-parser` 检查字体中是否存在某字符。
   - 使用 `ab_glyph` 或 `fontdue` 将缺字 raster 为灰度 bitmap。
   - 转换灰度 bitmap 为 MSDF 的近似：`R=G=B=alpha`（退化为 SDF），或只使用 alpha 通道。

2. **新建 `MsdfDynamicAtlas`**：
   - 替代当前的 `MsdfAtlas`（或作为 wrapper）。
   - 内部维护 `Vec<AtlasSlot>`，slot 0 指向当前静态 atlas。
   - 新建 1024×1024 texture 作为动态 slot。

3. **修改 `layout_text_ndc`**：
   - 缺字时不再直接 tofu，而是：
     - 如果字符已经在动态 cache 中（pending 但未完成），显示 tofu。
     - 如果字符不在任何 cache 中，标记 pending，启动后台生成（直接在当前帧同步生成，或延迟到下一帧）。
     - 如果字符已 ready，从动态 slot 中取 atlas_bounds 和 advance。

4. **修改 `init_msdf`**：
   - 初始化时加载静态 atlas（不变）。
   - 额外创建动态 textures + packer。

5. **修改 `create_msdf_pipeline`**：
   - 每个 atlas slot 有独立 bind group。
   - pipeline 复用。

#### P5-B 涉及文件

| 文件 | 变更类型 | 说明 |
|------|----------|------|
| `src/msdf/atlas.rs` | 新增 `MsdfDynamicAtlas` 结构体和 glyph cache | 核心变更 |
| `src/msdf/renderer.rs` | 修改 layout 函数，支持多 slot glyph 查找 | 扩展 |
| `src/msdf/debug_paint.rs` | 修改 init，创建动态 atlas | 适配 |
| `src/msdf/mod.rs` | 新增模块引用 | 适配 |
| `Cargo.toml` | 新增 `ttf-parser`、`ab_glyph` 或 `fontdue` | 新增依赖 |
| `docs/msdf-dynamic-atlas-design.md` | 更新 | 文档 |

#### P5-B 验收标准

1. 输入一个当前 atlas 没有的中文生僻字（如"𪚥" U+2A6A5），标题正常显示，不再 tofu。
2. 缺字首次显示可能有 1 帧 tofu，下一帧即正常。
3. 已有字符不受影响。
4. `cargo check` 通过。
5. 性能无明显下降（生成 1-2 个 glyph < 10ms）。

#### P5-B 非目标

- ❌ 不实现完整 MSDF 生成。
- ❌ 不支持 emoji。
- ❌ 不支持 shaping。
- ❌ 不修改现有 pipeline/descriptor set 结构。
- ❌ 不重构合批。

### 3.2 P5-C：完整 Dynamic MSDF / Shaping

#### 具体实现

1. **自实现 MSDF 生成**（或绑定 msdfgen）：
   - 用 `ttf-parser` 提取 glyph outline 的 bezier curves。
   - 实现 analytic MSDF：对每像素，计算到所有 curve segments 的有符号距离。
   - Multi-channel：对 R/G/B 三个子像素位置分别计算距离。
   - 性能优化：bounding box 裁剪、tiered distance field。

2. **集成 cosmic-text**（可选）：
   - 用 cosmic-text 做 shaping、line breaking、fallback 字体选择。
   - cosmic-text 输出 `(font_id, glyph_id, pos)` 列表。
   - atlas lookup 从 char-based 改为 (font_id, glyph_id) based。

3. **shader 更新**（如果需要区分 MSDF/SDF/alpha）：
   - 目前 shader 假设所有 glyph 是 MSDF/MTSDF。
   - 如果引入非 MSDF glyph（例如 emoji color bitmap），需要 shader extension。
   - 可以用 `atlas_bounds` 中编码标志位，或新增 per-glyph flag。

#### P5-C 涉及文件

| 文件 | 变更类型 | 说明 |
|------|----------|------|
| `src/msdf/msdf_gen.rs` | 新增 | MSDF 运行时生成核心算法 |
| `src/msdf/atlas.rs` | 大幅修改 | glyph cache 升级为 (font_id, glyph_id) key |
| `src/msdf/layout.rs` | 新增 | cosmic-text 集成层 |
| `src/msdf/renderer.rs` | 修改 | 支持多 font 多 atlas |
| `src/msdf/debug_paint.rs` | 修改 | 适配新的 layout 层 |
| `Cargo.toml` | 新增 | `cosmic-text`、`skrifa` 或 `ttf-parser` |
| `src/msdf/shader.wgsl` | 可能修改 | 如果需要支持非 MSDF glyph |

#### P5-C 验收标准

1. 任何中文字符（含 GB2312 L2 生僻字）运行时正确生成并显示 MSDF。
2. Emoji 显示为 tofu 或 basic SDF fallback（不需完美）。
3. 多字体 fallback 正确（主字体缺字 → fallback 字体）。
4. 性能：每 glyph 生成 < 5ms（font_size=48）。
5. 缺字 -> glyph ready 延迟不超过 2 帧。
6. `cargo check` 通过。

---

## 4. 风险和 Stop Rules

### 4.1 已知风险

| 风险 | 概率 | 影响 | 应对 |
|------|------|------|------|
| 自实现 MSDF 生成对 CJK 锐角效果不佳 | 中 | 高 | P5-B 先走 alpha SDF，给 MSDF 更多时间验证 |
| 运行时 glyph 生成拖慢 UI 帧 | 低 | 中 | 生成在主线程外进行？不，wgpu 需要同线程。延迟生成到下一帧。或限制每帧生成 ≤ 2 glyph |
| 多 atlas 导致 shader bind group 切换开销 | 低 | 低 | 按 slot 分组绘制，额外 draw call 可忽略 |
| 动态 atlas texture 碎片化 | 中 | 低 | 简单的 packer（skyline/guillotine）对 CJK（近似等宽）效果足够 |
| 生僻字大量出现导致 atlas 快速填满 | 低 | 中 | 合理设置第二 atlas 大小；满后告警，不 panic |
| `ttf-parser` 解析可变字体 NotoSansSC-VF.ttf | 低 | 中 | 需要确认 `ttf-parser` 支持 CFF2/glyf 可变字体路径。skrifa 更好 |
| 字体文件路径在不同 Windows 版本上不同 | 低 | 低 | 添加字体查找 fallback 路径；允许用户配置字体路径 |

### 4.2 Stop Rules

1. **如果 `cargo check` 失败且不是无关警告，stop。**
   - 新的依赖可能引入版本冲突（特别是 wgpu/egui-wgpu 版本）。
   - 先 `cargo tree` 确认兼容版本，再新增依赖。

2. **如果运行时 MSDF 生成质量明显低于预生成 atlas，stop。**
   - msdf-atlas-gen 使用专门的 edge coloring 和扫描算法。
   - 自实现如果效果差，考虑换方案（调用 msdfgen CLI？绑定 msdfgen C API？）。

3. **如果动态 texture 更新导致渲染闪烁/撕裂，stop。**
   - `queue.write_texture` 是安全的，但写入和渲染的同步可能需要 fence。
   - 如果出现部分更新的 texture 被部分采样，考虑 double buffering。

4. **如果 atlas 在 1 秒内生成超过 10 个 glyph，stop 并评估性能。**
   - 用户可能粘贴了大段生僻字文本。需要 throttle。

5. **如果必须修改现有 `paint_msdf_label` 签名或调用方式，需要 team 审核。**
   - 当前调用方分散在 3 个文件中，签名变更影响大。

---

## 5. 推荐的实施顺序

```
P5-B (MVP, Runtime Alpha Fallback)
  ├── 5-B.1: 新增 ttf-parser + ab_glyph 依赖
  ├── 5-B.2: 实现 MsdfDynamicAtlas (glyph cache + packing)
  ├── 5-B.3: 运行时 alpha glyph 生成 + GPU texture 局部更新
  ├── 5-B.4: 集成到 layout_text_ndc
  └── 5-B.5: 测试生僻字、缺字、性能

P5-C (Full Dynamic MSDF)
  ├── 5-C.1: 自实现 MSDF distance field 生成
  ├── 5-C.2: 替换 P5-B 的 alpha fallback 为 MSDF
  ├── 5-C.3: 集成 cosmic-text shaping + fallback 字体链
  └── 5-C.4: 非 MSDF glyph (emoji SDF) 支持
```

---

## 6. 当前架构关键观察

1. **Atlas 已经满了** (3012×3012, max right=3011.5)。所有动态方案必须新建 atlas。
2. **`COPY_DST` 已在 texture usage 中**，局部更新不需要改现有 texture 创建代码。
3. **Pipeline 可复用**，只需要新的 bind group。
4. **bind group 在单次 texture 生命周期内不需要重建**，但新建 atlas 时需要。
5. **当前 layout 是 char-based**，简单。多字体时需要改为 (font_id, glyph_id) based。
6. **PaintCallback 对每个 label 调用一次 prepare/paint**，这是合批的阻碍，但不是动态 atlas 的阻碍。

---

## 7. 是否新增文档

✅ 是。本文档 `docs/msdf-dynamic-atlas-design.md` 为新增输出。

还需更新：
- `docs/msdf-text-rendering-followup-plan.md` — 补充动态 atlas 分期细节。
- `assets/fonts/msdf/README.md` — 添加动态运行时的说明。

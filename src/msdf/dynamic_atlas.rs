//! P5-B2 Dynamic MSDF Atlas
//!
//! Manages a second GPU texture for runtime-generated MSDF glyphs (missing from
//! the static atlas).  Uses `fdsm` + `fdsm-ttf-parser` for pure-Rust MTSDF
//! generation and `fontdb` for font discovery without hardcoded paths.
//!
//! ## Design decisions
//!
//! - **Single dynamic texture** (1024×1024), row-based packer.
//! - **Per-frame throttle**: at most 2 glyphs generated per `generate_pending()` call.
//! - **Graceful disable**: if font cannot be found, `active = false` and all
//!   operations become no-ops.
//! - **write_texture alignment**: `bytes_per_row` rounded up to 256.
//! - **Fallback font chain**: multiple fonts are tried in order per glyph. Each
//!   cached glyph records its `font_index` to avoid cross-font glyph conflicts.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Instant;

use crate::msdf::atlas::Bounds;
use egui_wgpu::wgpu;
use wgpu::util::DeviceExt;
use fdsm::bezier::scanline::FillRule;
use fdsm::generate::generate_mtsdf;
use fdsm::render::correct_sign_mtsdf;
use fdsm::shape::Shape;
use fdsm::transform::Transform;
use nalgebra::{Affine2, Similarity2, Vector2};

// ── Constants ──

/// Size of the dynamic atlas texture (square).
const DYNAMIC_ATLAS_SIZE: u32 = 1024;

/// Font size in pixels (em size) for runtime glyph generation.
const FONT_SIZE_PX: f64 = 48.0;

/// Distance range / px_range — must match static atlas (4.0) and the WGSL shader.
const PX_RANGE: f64 = 4.0;

/// Padding (in pixels) between adjacent glyphs in the atlas to avoid bilinear
/// filter bleeding.
const GLYPH_PADDING: u32 = 2;

/// Maximum number of glyphs to generate in a single frame.
const MAX_GLYPHS_PER_FRAME: u32 = 2;

/// Fallback font families to search, in order of preference (font chain).
/// The first font that contains a glyph is used for that glyph.
const FALLBACK_FAMILIES: &[&str] = &[
    "Noto Sans SC", "Noto Sans CJK SC", "Noto Sans CJK JP", "Noto Sans CJK KR",
    "Noto Sans JP", "Noto Sans KR",
    "Microsoft YaHei", "Microsoft JhengHei",
    "SimHei", "Malgun Gothic",
];

// ── Data structures ──

/// Cached runtime glyph metadata.
#[derive(Clone, Debug)]
pub struct DynamicGlyphEntry {
    /// Horizontal advance in em-normalized units.
    pub advance: f32,
    /// Plane bounds in em-normalized coordinates.
    pub plane_bounds: Option<Bounds>,
    /// Atlas pixel bounds within the dynamic texture.
    pub atlas_bounds: Bounds,
    /// Index into the font chain that produced this glyph.
    #[allow(dead_code)]
    pub font_index: usize,
}

/// Font entry in the fallback font chain.
struct FontEntry {
    #[allow(dead_code)]
    data: &'static [u8],
    face: ttf_parser::Face<'static>,
    units_per_em: f64,
}

/// Simple row-based atlas packer.
///
/// Allocates glyphs left-to-right, top-to-bottom.  Each row has the height of
/// the tallest glyph placed in it.  This is efficient for CJK (roughly equal
/// height) and trivial to implement.
struct RowPacker {
    width: u32,
    height: u32,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
    can_pack: bool,
}

impl RowPacker {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
            can_pack: true,
        }
    }

    /// Reserve a slot for a glyph of the given pixel dimensions.
    /// Returns `Some((x, y))` top-left corner (padded), or `None` if full.
    fn pack(&mut self, glyph_w: u32, glyph_h: u32) -> Option<(u32, u32)> {
        if !self.can_pack {
            return None;
        }

        // Slot size includes padding on all sides
        let slot_w = glyph_w + GLYPH_PADDING * 2;
        let slot_h = glyph_h + GLYPH_PADDING * 2;

        // Start new row if needed
        if self.cursor_x + slot_w > self.width {
            self.cursor_x = 0;
            self.cursor_y += self.row_height;
            self.row_height = 0;
        }

        // Check if vertical space remains
        if self.cursor_y + slot_h > self.height {
            self.can_pack = false;
            return None;
        }

        let x = self.cursor_x + GLYPH_PADDING;
        let y = self.cursor_y + GLYPH_PADDING;

        self.cursor_x += slot_w;
        self.row_height = self.row_height.max(slot_h);

        Some((x, y))
    }

    fn is_full(&self) -> bool {
        !self.can_pack
    }
}

// ── Dynamic MSDF Atlas ──

/// Main dynamic MSDF atlas controller.
///
/// Holds GPU resources for the dynamic texture and manages glyph generation &
/// packing.  Wrapped in a `Mutex` so it can be accessed from `prepare()` which
/// already has a `device` / `queue`.
pub struct DynamicMsdfAtlas {
    /// Whether the atlas was initialized successfully.
    active: bool,
    /// Error message if inactive.
    #[allow(dead_code)]
    error: Option<String>,

    // Font chain: ordered list of fallback fonts.
    fonts: Vec<FontEntry>,

    // GPU resources
    texture: wgpu::Texture,
    #[allow(dead_code)]
    texture_view: wgpu::TextureView,
    #[allow(dead_code)]
    sampler: wgpu::Sampler,
    #[allow(dead_code)]
    uniform_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    #[allow(dead_code)]
    target_format: wgpu::TextureFormat,

    // Packing state
    packer: RowPacker,

    // Glyph cache: char → metadata (glyph tagged with font_index)
    glyph_cache: HashMap<char, DynamicGlyphEntry>,

    // Pending generation queue (char to generate, font index resolved at gen time)
    pending: VecDeque<char>,

    // Characters that cannot be found in any font.
    failed: HashSet<char>,

    // Per-frame tracking
    generated_this_frame: u32,
    last_frame_reset: Instant,
    atlas_full_reported: bool,
}

impl DynamicMsdfAtlas {
    /// Create a new dynamic atlas.  Returns Ok if at least one fallback font
    /// was found and GPU resources could be created, or Err with a message.
    ///
    /// `bind_group_layout` must be the same layout used by the static MSDF
    /// pipeline (texture @ 0, sampler @ 1, uniform @ 2).
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bind_group_layout: &wgpu::BindGroupLayout,
        target_format: wgpu::TextureFormat,
    ) -> Result<Self, String> {
        // ── Font discovery (font chain) ──
        let discovered = discover_fallback_font_chain()?;
        let mut fonts = Vec::with_capacity(discovered.len());
        for df in &discovered {
            let font_data: &'static [u8] = Box::leak(df.data.clone().into_boxed_slice());
            match ttf_parser::Face::parse(font_data, df.face_index) {
                Ok(face) => {
                    let units_per_em = face.units_per_em() as f64;
                    eprintln!(
                        "MSDF dynamic: loaded font (index {}), units_per_em={units_per_em}, size={}",
                        fonts.len(),
                        font_data.len()
                    );
                    fonts.push(FontEntry { data: font_data, face, units_per_em });
                }
                Err(e) => {
                    eprintln!("MSDF dynamic: skipping font (index {}): ttf-parser error {e:?}", fonts.len());
                }
            }
        }
        if fonts.is_empty() {
            return Err("No usable font found in fallback chain.".into());
        }
        eprintln!("MSDF dynamic: font chain has {} font(s)", fonts.len());

        // ── GPU texture ──
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("msdf_dynamic_atlas_texture"),
            size: wgpu::Extent3d {
                width: DYNAMIC_ATLAS_SIZE,
                height: DYNAMIC_ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Initialize with transparent black
        let init_data = vec![0u8; DYNAMIC_ATLAS_SIZE as usize * DYNAMIC_ATLAS_SIZE as usize * 4];
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &init_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(DYNAMIC_ATLAS_SIZE * 4),
                rows_per_image: Some(DYNAMIC_ATLAS_SIZE),
            },
            wgpu::Extent3d {
                width: DYNAMIC_ATLAS_SIZE,
                height: DYNAMIC_ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
        );

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // ── Sampler ──
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("msdf_dynamic_atlas_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // ── Uniform ──
        let uniform = crate::msdf::renderer::MsdfUniform {
            atlas_size: [DYNAMIC_ATLAS_SIZE as f32, DYNAMIC_ATLAS_SIZE as f32],
            px_range: PX_RANGE as f32,
            _pad: 0.0,
        };
        let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("msdf_dynamic_uniform_buffer"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // ── Bind group ──
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("msdf_dynamic_bind_group"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(uniform_buf.as_entire_buffer_binding()),
                },
            ],
        });

        Ok(Self {
            active: true,
            error: None,
            fonts,
            texture,
            texture_view,
            sampler,
            uniform_buf,
            bind_group,
            target_format,
            packer: RowPacker::new(DYNAMIC_ATLAS_SIZE, DYNAMIC_ATLAS_SIZE),
            glyph_cache: HashMap::new(),
            pending: VecDeque::new(),
            failed: HashSet::new(),
            generated_this_frame: 0,
            last_frame_reset: Instant::now(),
            atlas_full_reported: false,
        })
    }

    /// Accessor for use in paint callbacks.
    pub fn bind_group(&self) -> Option<&wgpu::BindGroup> {
        if self.active {
            Some(&self.bind_group)
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn is_active(&self) -> bool {
        self.active
    }

    #[allow(dead_code)]
    pub fn error_message(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Look up a glyph.  Checks the runtime cache (does NOT check static atlas).
    /// If not found and char is valid in any fallback font, enqueues it for
    /// generation using the first font that contains the glyph.
    /// Returns `None` if the char is not in cache yet (pending or not found).
    pub fn lookup_or_enqueue(&mut self, ch: char) -> Option<&DynamicGlyphEntry> {
        if !self.active {
            return None;
        }

        // Already cached?
        if self.glyph_cache.contains_key(&ch) {
            return self.glyph_cache.get(&ch);
        }

        // Already known to be unfindable?
        if self.failed.contains(&ch) {
            return None;
        }

        // Check if any font in the chain has this glyph
        let has_glyph = self.fonts.iter().any(|f| f.face.glyph_index(ch).is_some());
        if !has_glyph {
            self.failed.insert(ch);
            return None;
        }

        // Enqueue (avoid duplicates in pending)
        if !self.pending.contains(&ch) {
            self.pending.push_back(ch);
        }
        None
    }

    /// Auto-detect frame boundary and reset per-frame counter if needed.
    /// Call this at the start of each `prepare()` before `generate_pending()`.
    pub fn maybe_begin_frame(&mut self) {
        let now = Instant::now();
        // Frames are typically ~16ms apart; any gap > 5ms signals a new frame.
        if now.duration_since(self.last_frame_reset) > std::time::Duration::from_millis(5) {
            self.generated_this_frame = 0;
            self.last_frame_reset = now;
        }
    }

    /// Generate up to `MAX_GLYPHS_PER_FRAME` pending glyphs.
    /// Performs fdsm generation, packing, and GPU texture upload.
    /// Safe to call multiple times per frame — respects per-frame throttle.
    pub fn generate_pending(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if !self.active {
            return;
        }

        if self.packer.is_full() {
            if !self.atlas_full_reported {
                eprintln!(
                    "MSDF dynamic: atlas full ({}/{} glyphs cached, {} pending)",
                    self.glyph_cache.len(),
                    self.pending.len(),
                    self.glyph_cache.len() + self.pending.len()
                );
                self.atlas_full_reported = true;
            }
            return;
        }

        if self.generated_this_frame >= MAX_GLYPHS_PER_FRAME {
            return;
        }

        let mut generated: u32 = 0;

        while self.generated_this_frame + generated < MAX_GLYPHS_PER_FRAME {
            let Some(ch) = self.pending.pop_front() else {
                break;
            };

            // Skip if already cached (race condition from previous frame)
            if self.glyph_cache.contains_key(&ch) {
                continue;
            }

            match self.generate_one_glyph(device, queue, ch) {
                Ok(()) => {
                    generated += 1;
                }
                Err(e) => {
                    eprintln!("MSDF dynamic: failed to generate glyph U+{:04X}: {e}", ch as u32);
                    self.failed.insert(ch);
                }
            }
        }

        self.generated_this_frame += generated;
    }

    /// Find the first font index in the chain that contains this char.
    fn find_best_font(&self, ch: char) -> Option<usize> {
        for (i, font) in self.fonts.iter().enumerate() {
            if font.face.glyph_index(ch).is_some() {
                return Some(i);
            }
        }
        None
    }

    /// Generate a single glyph: fdsm → flip → pack → upload.
    /// Uses the first font in the chain that contains the glyph.
    fn generate_one_glyph(
        &mut self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        ch: char,
    ) -> Result<(), String> {
        // Resolve which font in the chain to use
        let font_idx = self.find_best_font(ch)
            .ok_or_else(|| format!("glyph not in any font"))?;

        let face = &self.fonts[font_idx].face;
        let units_per_em = self.fonts[font_idx].units_per_em;

        let glyph_id = face
            .glyph_index(ch)
            .ok_or_else(|| format!("glyph not in font"))?;

        let bbox = face
            .glyph_bounding_box(glyph_id)
            .ok_or_else(|| format!("no bounding box"))?;

        let shrinkage = units_per_em / FONT_SIZE_PX;

        // Compute output image dimensions (including PX_RANGE margin)
        let img_w = ((bbox.x_max as f64 - bbox.x_min as f64) / shrinkage + 2.0 * PX_RANGE)
            .ceil() as u32;
        let img_h = ((bbox.y_max as f64 - bbox.y_min as f64) / shrinkage + 2.0 * PX_RANGE)
            .ceil() as u32;

        if img_w == 0 || img_h == 0 {
            return Err("zero-size glyph".into());
        }

        // Load shape from font
        let mut shape = fdsm_ttf_parser::load_shape_from_face(&face, glyph_id)
            .ok_or_else(|| format!("could not load glyph shape"))?;

        // Transformation: font units → pixel space + PX_RANGE margin
        let transformation: Affine2<f64> = nalgebra::convert(Similarity2::new(
            Vector2::new(
                PX_RANGE - (bbox.x_min as f64) / shrinkage,
                PX_RANGE - (bbox.y_min as f64) / shrinkage,
            ),
            0.0,
            1.0 / shrinkage,
        ));
        shape.transform(&transformation);

        // Edge coloring + prepare
        let colored = Shape::edge_coloring_simple(shape, 0.03, 69441337420);
        let prepared = colored.prepare();

        // Generate MTSDF
        let mut mtsdf = image::RgbaImage::new(img_w, img_h);
        generate_mtsdf(&prepared, PX_RANGE, &mut mtsdf);
        correct_sign_mtsdf(&mut mtsdf, &prepared, FillRule::Nonzero);

        // ── Y-flip: fdsm produces top-down, GPU expects bottom-up ──
        let raw = mtsdf.into_raw();
        let row_bytes = img_w as usize * 4;
        let mut flipped = vec![0u8; raw.len()];
        for row in 0..img_h as usize {
            let src_start = row * row_bytes;
            let dst_start = (img_h as usize - 1 - row) * row_bytes;
            flipped[dst_start..dst_start + row_bytes]
                .copy_from_slice(&raw[src_start..src_start + row_bytes]);
        }

        // ── Pack into atlas ──
        let (atlas_x, atlas_y) = self
            .packer
            .pack(img_w, img_h)
            .ok_or_else(|| format!("atlas full"))?;

        // ── GPU upload with alignment ──
        let src_row_bytes = img_w as usize * 4;
        // wgpu requires bytes_per_row % COPY_BYTES_PER_ROW_ALIGNMENT (256) == 0
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
        let dst_row_bytes = ((src_row_bytes + align - 1) / align) * align;
        let mut padded = vec![0u8; dst_row_bytes * img_h as usize];
        for row in 0..img_h as usize {
            let src_off = row * src_row_bytes;
            let dst_off = row * dst_row_bytes;
            padded[dst_off..dst_off + src_row_bytes]
                .copy_from_slice(&flipped[src_off..src_off + src_row_bytes]);
        }

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: atlas_x,
                    y: atlas_y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &padded,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(dst_row_bytes as u32),
                rows_per_image: Some(img_h),
            },
            wgpu::Extent3d {
                width: img_w,
                height: img_h,
                depth_or_array_layers: 1,
            },
        );

        // ── Compute metadata ──
        let advance = face.glyph_hor_advance(glyph_id).unwrap_or(0) as f64 / units_per_em;
        let plane_bounds = Some(Bounds {
            left: bbox.x_min as f32 / units_per_em as f32,
            bottom: bbox.y_min as f32 / units_per_em as f32,
            right: bbox.x_max as f32 / units_per_em as f32,
            top: bbox.y_max as f32 / units_per_em as f32,
        });
        let atlas_bounds = Bounds {
            left: atlas_x as f32,
            bottom: (atlas_y + img_h) as f32,
            right: (atlas_x + img_w) as f32,
            top: atlas_y as f32,
        };

        // ── Cache with font_index ──
        self.glyph_cache.insert(
            ch,
            DynamicGlyphEntry {
                advance: advance as f32,
                plane_bounds,
                atlas_bounds,
                font_index: font_idx,
            },
        );

        Ok(())
    }

    /// Get cache entry (for external measurement / layout).
    #[allow(dead_code)]
    pub fn get_cached(&self, ch: char) -> Option<&DynamicGlyphEntry> {
        self.glyph_cache.get(&ch)
    }

    /// Look up advance for a char from the dynamic cache (for text measurement).
    pub fn advance_for_char(&self, ch: char) -> Option<f32> {
        self.glyph_cache.get(&ch).map(|e| e.advance)
    }

    /// Return whether the dynamic atlas is full.
    #[allow(dead_code)]
    pub fn is_full(&self) -> bool {
        self.packer.is_full()
    }

    /// Report number of cached glyphs.
    #[allow(dead_code)]
    pub fn cached_count(&self) -> usize {
        self.glyph_cache.len()
    }

    /// Report number of pending glyphs.
    #[allow(dead_code)]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

// ── Font discovery ──

struct DiscoveredFont {
    data: Vec<u8>,
    face_index: u32,
}

/// Discover the fallback font chain by trying each family in order.
/// Returns all fonts that were found (not just the first), so that the
/// dynamic atlas can fall through to subsequent fonts per-glyph.
fn discover_fallback_font_chain() -> Result<Vec<DiscoveredFont>, String> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();

    let mut results = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    for family_name in FALLBACK_FAMILIES {
        let families = [fontdb::Family::Name(family_name)];
        let query = fontdb::Query {
            families: &families,
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        };

        let Some(font_id) = db.query(&query) else {
            continue;
        };

        // Deduplicate: skip if we already loaded this font ID
        if !seen_ids.insert(font_id) {
            continue;
        }

        let Some(face) = db.face(font_id) else {
            continue;
        };

        let data = match &face.source {
            fontdb::Source::File(path) => match std::fs::read(path) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("MSDF dynamic: cannot read '{}': {e}", path.display());
                    continue;
                }
            },
            fontdb::Source::Binary(data) => data.as_ref().as_ref().to_vec(),
            fontdb::Source::SharedFile(_, data) => data.as_ref().as_ref().to_vec(),
        };

        results.push(DiscoveredFont { data, face_index: face.index });
    }

    if results.is_empty() {
        return Err(format!(
            "No fallback font found among: {}. \
             Please install a CJK font (e.g. NotoSansSC) or configure a custom path.",
            FALLBACK_FAMILIES.join(", ")
        ));
    }

    Ok(results)
}

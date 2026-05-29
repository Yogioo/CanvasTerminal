use crate::msdf::atlas::MsdfAtlas;
use crate::msdf::dynamic_atlas::DynamicMsdfAtlas;
use crate::msdf::renderer::{
    create_msdf_pipeline, layout_text_ndc, MsdfFrameResourceMap, MsdfFrameResources, MsdfRenderer,
};
use eframe::egui::{self};
use egui_wgpu::wgpu;
use std::sync::Mutex;
use wgpu::util::DeviceExt;

/// Global MSDF renderer (pipeline + bind group), set once after a wgpu device is available.
static MSDF_RENDERER: std::sync::OnceLock<MsdfRenderer> = std::sync::OnceLock::new();
/// Global MSDF atlas (glyph data), set once.
static MSDF_ATLAS: std::sync::OnceLock<MsdfAtlas> = std::sync::OnceLock::new();
/// Global dynamic MSDF atlas for runtime glyph generation.
static DYNAMIC_MSDF_ATLAS: std::sync::OnceLock<Mutex<DynamicMsdfAtlas>> =
    std::sync::OnceLock::new();

/// Initialize the MSDF atlas and renderer.
/// Must be called once before any paint calls, after a wgpu device is available.
pub fn init_msdf(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    target_format: wgpu::TextureFormat,
) -> Result<(), String> {
    let atlas_json = include_bytes!("../../assets/fonts/msdf/atlas.json");
    let atlas_png = include_bytes!("../../assets/fonts/msdf/atlas.png");

    let atlas = MsdfAtlas::load(atlas_json, atlas_png)?;
    let (pipeline, bind_group, bind_group_layout) =
        create_msdf_pipeline(device, queue, &atlas, target_format);

    let _ = MSDF_ATLAS.set(atlas);
    let _ = MSDF_RENDERER.set(MsdfRenderer {
        pipeline,
        bind_group,
        bind_group_layout,
        target_format,
    });

    // ── Initialize dynamic atlas (graceful on failure) ──
    // Retrieve the bind group layout from the already-set renderer
    let bgl = &MSDF_RENDERER.get().unwrap().bind_group_layout;
    match DynamicMsdfAtlas::new(device, queue, bgl, target_format) {
        Ok(dynamic) => {
            let _ = DYNAMIC_MSDF_ATLAS.set(Mutex::new(dynamic));
        }
        Err(e) => {
            eprintln!("MSDF dynamic atlas DISABLED: {e}");
            // DYNAMIC_MSDF_ATLAS stays unset — prepare() falls back to single-atlas
        }
    };

    Ok(())
}

/// Access the global MSDF atlas for read-only operations (e.g. text measurement).
#[allow(dead_code)]
pub fn with_msdf_atlas<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&MsdfAtlas) -> R,
{
    MSDF_ATLAS.get().map(f)
}

/// Access the global dynamic MSDF atlas (read-only, e.g. measurement).
#[allow(dead_code)]
pub fn with_msdf_dynamic_atlas<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&DynamicMsdfAtlas) -> R,
{
    DYNAMIC_MSDF_ATLAS
        .get()
        .and_then(|m| m.lock().ok())
        .map(|guard| f(&guard))
}

/// Measure text width using both static and dynamic atlases.
/// Falls back to tofu width (0.5 * font_size_px) for chars missing from both.
pub fn measure_text_width_dual(text: &str, font_size_px: f32) -> f32 {
    let em_scale = font_size_px;
    let static_atlas = MSDF_ATLAS.get();
    // Lock dynamic atlas once (if available) for the whole measurement
    let dyn_guard = DYNAMIC_MSDF_ATLAS
        .get()
        .and_then(|m| m.lock().ok());

    let mut width = 0.0;
    for ch in text.chars() {
        if let Some(glyph) = static_atlas.and_then(|a| a.glyph(ch)) {
            width += glyph.advance * em_scale;
        } else if let Some(ref dyn_atlas) = dyn_guard {
            if let Some(advance) = dyn_atlas.advance_for_char(ch) {
                width += advance * em_scale;
            } else {
                width += font_size_px * 0.5;
            }
        } else {
            width += font_size_px * 0.5;
        }
    }
    width
}

// ── Per-frame glyph layout: produces two vertex/index sets ──

/// Result of laying out text with both static and dynamic atlas lookup.
struct LayoutResult {
    /// Static-atlas vertices / indices.
    static_vertices: Vec<crate::msdf::renderer::GlyphVertex>,
    static_indices: Vec<u16>,
    /// Dynamic-atlas vertices / indices.
    dynamic_vertices: Vec<crate::msdf::renderer::GlyphVertex>,
    dynamic_indices: Vec<u16>,
}

/// Layout text using both static and dynamic atlases.
/// For each character: look up static → dynamic → tofu.
/// Characters missing from both are enqueued into the dynamic atlas.
/// Returns two separate vertex/index sets.
fn layout_text_dual(
    static_atlas: &MsdfAtlas,
    dynamic_atlas: &mut DynamicMsdfAtlas,
    text: &str,
    x0_ndc: f32,
    y0_ndc: f32,
    font_size_ndc: f32,
    color: [f32; 4],
) -> LayoutResult {
    let atlas_w = static_atlas.atlas_width as f32;
    let atlas_h = static_atlas.atlas_height as f32;
    let dynamic_atlas_size = 1024.0; // must match DYNAMIC_ATLAS_SIZE
    let em_scale = font_size_ndc;

    let mut static_vertices = Vec::new();
    let mut static_indices = Vec::new();
    let mut dynamic_vertices = Vec::new();
    let mut dynamic_indices = Vec::new();
    let mut cursor_x = x0_ndc;
    let mut static_base_idx: u16 = 0;
    let mut dynamic_base_idx: u16 = 0;

    for ch in text.chars() {
        // 1. Try static atlas
        if let Some(glyph) = static_atlas.glyph(ch) {
            if let (Some(atlas_bounds), Some(plane_bounds)) =
                (glyph.atlas_bounds, glyph.plane_bounds)
            {
                let l = cursor_x + plane_bounds.left * em_scale;
                let r = cursor_x + plane_bounds.right * em_scale;
                let b_ndc = y0_ndc + plane_bounds.bottom * em_scale;
                let t_ndc = y0_ndc + plane_bounds.top * em_scale;

                let u_l = atlas_bounds.left / atlas_w;
                let u_r = atlas_bounds.right / atlas_w;
                let v_t = 1.0 - atlas_bounds.bottom / atlas_h;
                let v_b = 1.0 - atlas_bounds.top / atlas_h;

                static_vertices.extend_from_slice(&[
                    crate::msdf::renderer::GlyphVertex {
                        pos: [l, t_ndc],
                        uv: [u_l, v_b],
                        color,
                    },
                    crate::msdf::renderer::GlyphVertex {
                        pos: [r, t_ndc],
                        uv: [u_r, v_b],
                        color,
                    },
                    crate::msdf::renderer::GlyphVertex {
                        pos: [r, b_ndc],
                        uv: [u_r, v_t],
                        color,
                    },
                    crate::msdf::renderer::GlyphVertex {
                        pos: [l, b_ndc],
                        uv: [u_l, v_t],
                        color,
                    },
                ]);
                let bi = static_base_idx;
                static_indices.extend_from_slice(&[bi, bi + 1, bi + 2, bi, bi + 2, bi + 3]);
                static_base_idx += 4;
                cursor_x += glyph.advance * em_scale;
                continue;
            }
            // Has no bounds — still advance
            cursor_x += glyph.advance * em_scale;
            continue;
        }

        // 2. Try dynamic atlas (lookup or enqueue)
        if let Some(dyn_glyph) = dynamic_atlas.lookup_or_enqueue(ch) {
            let atlas_bounds = &dyn_glyph.atlas_bounds;
            if let Some(ref plane_bounds) = dyn_glyph.plane_bounds {
                let l = cursor_x + plane_bounds.left * em_scale;
                let r = cursor_x + plane_bounds.right * em_scale;
                let b_ndc = y0_ndc + plane_bounds.bottom * em_scale;
                let t_ndc = y0_ndc + plane_bounds.top * em_scale;

                let u_l = atlas_bounds.left / dynamic_atlas_size;
                let u_r = atlas_bounds.right / dynamic_atlas_size;
                // After Y-flip: bottom-of-glyph at low v (rect top), top-of-glyph at high v (rect bottom).
                let v_t = atlas_bounds.top / dynamic_atlas_size;
                let v_b = atlas_bounds.bottom / dynamic_atlas_size;

                dynamic_vertices.extend_from_slice(&[
                    crate::msdf::renderer::GlyphVertex {
                        pos: [l, t_ndc],
                        uv: [u_l, v_b],
                        color,
                    },
                    crate::msdf::renderer::GlyphVertex {
                        pos: [r, t_ndc],
                        uv: [u_r, v_b],
                        color,
                    },
                    crate::msdf::renderer::GlyphVertex {
                        pos: [r, b_ndc],
                        uv: [u_r, v_t],
                        color,
                    },
                    crate::msdf::renderer::GlyphVertex {
                        pos: [l, b_ndc],
                        uv: [u_l, v_t],
                        color,
                    },
                ]);
                let bi = dynamic_base_idx;
                dynamic_indices.extend_from_slice(&[bi, bi + 1, bi + 2, bi, bi + 2, bi + 3]);
                dynamic_base_idx += 4;
                cursor_x += dyn_glyph.advance * em_scale;
                continue;
            }
            // No plane bounds — advance
            cursor_x += dyn_glyph.advance * em_scale;
            continue;
        }

        // 3. Tofu (missing from both static and dynamic cache)
        let tofu_w = font_size_ndc * 0.5;
        let tofu_h = font_size_ndc;
        let (l, r) = (cursor_x, cursor_x + tofu_w);
        let (b_ndc, t_ndc) = (y0_ndc - tofu_h, y0_ndc);
        static_vertices.extend_from_slice(&[
            crate::msdf::renderer::GlyphVertex {
                pos: [l, b_ndc],
                uv: [0.0, 0.0],
                color,
            },
            crate::msdf::renderer::GlyphVertex {
                pos: [r, b_ndc],
                uv: [0.0, 0.0],
                color,
            },
            crate::msdf::renderer::GlyphVertex {
                pos: [r, t_ndc],
                uv: [0.0, 0.0],
                color,
            },
            crate::msdf::renderer::GlyphVertex {
                pos: [l, t_ndc],
                uv: [0.0, 0.0],
                color,
            },
        ]);
        let bi = static_base_idx;
        static_indices.extend_from_slice(&[bi, bi + 1, bi + 2, bi, bi + 2, bi + 3]);
        static_base_idx += 4;
        cursor_x += tofu_w;
    }

    LayoutResult {
        static_vertices,
        static_indices,
        dynamic_vertices,
        dynamic_indices,
    }
}

/// Paint a screen-space MSDF label via an egui-wgpu PaintCallback.
/// Renders `text` at `baseline_screen` (logical pixels, baseline-left).
/// `key` must be stable across frames for the same logical label so that
/// multiple labels in one frame do not overwrite each other's GPU resources.
pub fn paint_msdf_label(
    painter: &egui::Painter,
    callback_rect: egui::Rect,
    baseline_screen: egui::Pos2,
    text: &str,
    font_size_px: f32,
    color: egui::Color32,
    key: u64,
) {
    let rgba = [
        color.r() as f32 / 255.0,
        color.g() as f32 / 255.0,
        color.b() as f32 / 255.0,
        color.a() as f32 / 255.0,
    ];
    let callback = MsdfLabelCallback {
        text: text.to_owned(),
        callback_rect,
        baseline_screen,
        font_size_px,
        color: rgba,
        key,
    };
    let egui_callback = egui_wgpu::Callback::new_paint_callback(callback_rect, callback);
    painter.add(egui_callback);
}

// ── Screen-space label callback ──

struct MsdfLabelCallback {
    text: String,
    callback_rect: egui::Rect,
    baseline_screen: egui::Pos2,
    font_size_px: f32,
    color: [f32; 4],
    /// Stable key to avoid overwriting when multiple labels exist in one frame.
    key: u64,
}

impl egui_wgpu::CallbackTrait for MsdfLabelCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(static_atlas) = MSDF_ATLAS.get() else {
            return Vec::new();
        };
        let Some(dynamic_mutex) = DYNAMIC_MSDF_ATLAS.get() else {
            // Dynamic atlas not initialized — use single-atlas path
            return self.prepare_single(device, static_atlas, screen_descriptor, callback_resources);
        };

        let mut dynamic_guard = match dynamic_mutex.lock() {
            Ok(g) => g,
            Err(_) => {
                return self.prepare_single(device, static_atlas, screen_descriptor, callback_resources);
            }
        };
        let dynamic = &mut *dynamic_guard;

        let sf = screen_descriptor.pixels_per_point;

        // Callback rect in physical pixels
        let cb = self.callback_rect;
        let cb_w = (cb.width() * sf).max(1.0);
        let cb_h = (cb.height() * sf).max(1.0);

        // Baseline position relative to callback rect, in physical pixels
        let rel_x = (self.baseline_screen.x - cb.min.x) * sf;
        let rel_y = (self.baseline_screen.y - cb.min.y) * sf;

        // Convert to NDC within callback viewport
        let ndc_x0 = (rel_x / cb_w) * 2.0 - 1.0;
        let ndc_y0 = 1.0 - (rel_y / cb_h) * 2.0;

        // Font size in NDC (y-direction only; x is corrected below)
        let ndc_font_size = (self.font_size_px * sf) / cb_h * 2.0;

        // 1. Enqueue missing chars and generate pending glyphs
        dynamic.maybe_begin_frame();
        for ch in self.text.chars() {
            if static_atlas.glyph(ch).is_none() {
                dynamic.lookup_or_enqueue(ch);
            }
        }
        dynamic.generate_pending(device, queue);

        // 2. Layout with dual lookup
        let mut result = layout_text_dual(
            static_atlas,
            dynamic,
            &self.text,
            ndc_x0,
            ndc_y0,
            ndc_font_size,
            self.color,
        );

        // 3. NDC aspect-ratio correction for both static and dynamic vertices
        if cb_w != cb_h {
            // Static vertices
            let x_scale = cb_h / cb_w;
            for v in &mut result.static_vertices {
                v.pos[0] = ndc_x0 + (v.pos[0] - ndc_x0) * x_scale;
            }
            // Dynamic vertices
            for v in &mut result.dynamic_vertices {
                v.pos[0] = ndc_x0 + (v.pos[0] - ndc_x0) * x_scale;
            }
        }

        // 4. Create GPU buffers
        let has_static = !result.static_vertices.is_empty();
        let has_dynamic = !result.dynamic_vertices.is_empty();

        if !has_static && !has_dynamic {
            return Vec::new();
        }

        let vertex_buffer = if has_static {
            Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("msdf_label_static_vb"),
                contents: bytemuck::cast_slice(&result.static_vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }))
        } else {
            None
        };
        let index_buffer = if has_static {
            Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("msdf_label_static_ib"),
                contents: bytemuck::cast_slice(&result.static_indices),
                usage: wgpu::BufferUsages::INDEX,
            }))
        } else {
            None
        };

        let dynamic_vertex_buffer = if has_dynamic {
            Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("msdf_label_dynamic_vb"),
                contents: bytemuck::cast_slice(&result.dynamic_vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }))
        } else {
            None
        };
        let dynamic_index_buffer = if has_dynamic {
            Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("msdf_label_dynamic_ib"),
                contents: bytemuck::cast_slice(&result.dynamic_indices),
                usage: wgpu::BufferUsages::INDEX,
            }))
        } else {
            None
        };

        let map = callback_resources
            .entry::<MsdfFrameResourceMap>()
            .or_insert_with(MsdfFrameResourceMap::default);
        map.0.insert(
            self.key,
            MsdfFrameResources {
                vertex_buffer,
                index_buffer,
                num_indices: result.static_indices.len() as u32,
                dynamic_vertex_buffer,
                dynamic_index_buffer,
                dynamic_num_indices: result.dynamic_indices.len() as u32,
            },
        );

        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::epaint::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let Some(map) = callback_resources.get::<MsdfFrameResourceMap>() else {
            return;
        };
        let Some(resources) = map.0.get(&self.key) else {
            return;
        };
        let Some(renderer) = MSDF_RENDERER.get() else {
            return;
        };

        // 1. Draw static-atlas glyphs
        if resources.num_indices > 0 {
            if let (Some(v_buf), Some(i_buf)) = (&resources.vertex_buffer, &resources.index_buffer) {
                render_pass.set_pipeline(&renderer.pipeline);
                render_pass.set_bind_group(0, &renderer.bind_group, &[]);
                render_pass.set_vertex_buffer(0, v_buf.slice(..));
                render_pass.set_index_buffer(i_buf.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..resources.num_indices, 0, 0..1);
            }
        }

        // 2. Draw dynamic-atlas glyphs
        if resources.dynamic_num_indices > 0 {
            if let (Some(dv_buf), Some(di_buf)) =
                (&resources.dynamic_vertex_buffer, &resources.dynamic_index_buffer)
            {
                if let Some(dynamic_mutex) = DYNAMIC_MSDF_ATLAS.get() {
                    if let Ok(guard) = dynamic_mutex.lock() {
                        if let Some(dyn_bg) = guard.bind_group() {
                            render_pass.set_pipeline(&renderer.pipeline);
                            render_pass.set_bind_group(0, dyn_bg, &[]);
                            render_pass.set_vertex_buffer(0, dv_buf.slice(..));
                            render_pass.set_index_buffer(
                                di_buf.slice(..),
                                wgpu::IndexFormat::Uint16,
                            );
                            render_pass
                                .draw_indexed(0..resources.dynamic_num_indices, 0, 0..1);
                        }
                    }
                }
            }
        }
    }
}

// ── Fallback: single-atlas prepare (when dynamic atlas is unavailable) ──

impl MsdfLabelCallback {
    fn prepare_single(
        &self,
        device: &wgpu::Device,
        atlas: &MsdfAtlas,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let sf = screen_descriptor.pixels_per_point;

        let cb = self.callback_rect;
        let cb_w = (cb.width() * sf).max(1.0);
        let cb_h = (cb.height() * sf).max(1.0);

        let rel_x = (self.baseline_screen.x - cb.min.x) * sf;
        let rel_y = (self.baseline_screen.y - cb.min.y) * sf;

        let ndc_x0 = (rel_x / cb_w) * 2.0 - 1.0;
        let ndc_y0 = 1.0 - (rel_y / cb_h) * 2.0;
        let ndc_font_size = (self.font_size_px * sf) / cb_h * 2.0;

        let (mut vertices, indices) = layout_text_ndc(
            atlas,
            &self.text,
            ndc_x0,
            ndc_y0,
            ndc_font_size,
            self.color,
        );

        if cb_w != cb_h && !vertices.is_empty() {
            let x_scale = cb_h / cb_w;
            for v in &mut vertices {
                v.pos[0] = ndc_x0 + (v.pos[0] - ndc_x0) * x_scale;
            }
        }

        if vertices.is_empty() {
            return Vec::new();
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("msdf_label_vertex_buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("msdf_label_index_buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let map = callback_resources
            .entry::<MsdfFrameResourceMap>()
            .or_insert_with(MsdfFrameResourceMap::default);
        map.0.insert(
            self.key,
            MsdfFrameResources {
                vertex_buffer: Some(vertex_buffer),
                index_buffer: Some(index_buffer),
                num_indices: indices.len() as u32,
                dynamic_vertex_buffer: None,
                dynamic_index_buffer: None,
                dynamic_num_indices: 0,
            },
        );

        Vec::new()
    }
}

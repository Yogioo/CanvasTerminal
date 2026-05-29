use crate::msdf::atlas::MsdfAtlas;
use crate::msdf::renderer::{
    create_msdf_pipeline, layout_text_ndc, MsdfFrameResourceMap, MsdfFrameResources, MsdfRenderer,
};
use eframe::egui::{self};
use egui_wgpu::wgpu;
use wgpu::util::DeviceExt;

/// Global MSDF renderer (pipeline + bind group), set once after a wgpu device is available.
static MSDF_RENDERER: std::sync::OnceLock<MsdfRenderer> = std::sync::OnceLock::new();
/// Global MSDF atlas (glyph data), set once.
static MSDF_ATLAS: std::sync::OnceLock<MsdfAtlas> = std::sync::OnceLock::new();

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
    let (pipeline, bind_group) = create_msdf_pipeline(device, queue, &atlas, target_format);

    let _ = MSDF_ATLAS.set(atlas);
    let _ = MSDF_RENDERER.set(MsdfRenderer {
        pipeline,
        bind_group,
        target_format,
    });

    Ok(())
}

/// Access the global MSDF atlas for read-only operations (e.g. text measurement).
pub fn with_msdf_atlas<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&MsdfAtlas) -> R,
{
    MSDF_ATLAS.get().map(f)
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
        _queue: &wgpu::Queue,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(atlas) = MSDF_ATLAS.get() else {
            return Vec::new();
        };

        let sf = screen_descriptor.pixels_per_point;

        // Callback rect in physical pixels
        let cb = self.callback_rect;
        let cb_w = (cb.width() * sf).max(1.0);
        let cb_h = (cb.height() * sf).max(1.0);

        // Baseline position relative to callback rect, in physical pixels
        let rel_x = (self.baseline_screen.x - cb.min.x) * sf;
        // Y: egui y=top→bottom, NDC y=bottom→top. The baseline_screen.y is in egui coords.
        // We want NDC y=+1 at top of viewport, so invert.
        let rel_y = (self.baseline_screen.y - cb.min.y) * sf;

        // Convert to NDC within callback viewport
        let ndc_x0 = (rel_x / cb_w) * 2.0 - 1.0;
        let ndc_y0 = 1.0 - (rel_y / cb_h) * 2.0;

        // Font size in NDC
        let ndc_font_size = (self.font_size_px * sf) / cb_h * 2.0;

        let (vertices, indices) = layout_text_ndc(
            atlas,
            &self.text,
            ndc_x0,
            ndc_y0,
            ndc_font_size,
            self.color,
        );

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
                vertex_buffer,
                index_buffer,
                num_indices: indices.len() as u32,
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

        render_pass.set_pipeline(&renderer.pipeline);
        render_pass.set_bind_group(0, &renderer.bind_group, &[]);
        render_pass.set_vertex_buffer(0, resources.vertex_buffer.slice(..));
        render_pass.set_index_buffer(
            resources.index_buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        render_pass.draw_indexed(0..resources.num_indices, 0, 0..1);
    }
}


use crate::msdf::atlas::MsdfAtlas;
use egui_wgpu::wgpu;
use std::collections::HashMap;
use wgpu::util::DeviceExt;

// ── Data structures ──

/// Per-glyph vertex data for the shader.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlyphVertex {
    /// NDC position (clip space: -1..1)
    pub pos: [f32; 2],
    /// Atlas UV coordinate (0..1)
    pub uv: [f32; 2],
    /// RGBA color (premultiplied alpha)
    pub color: [f32; 4],
}

impl GlyphVertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Uniform buffer data sent to WGSL.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MsdfUniform {
    pub atlas_size: [f32; 2],
    pub px_range: f32,
    pub _pad: f32,
}

/// Long-lived GPU resources for MSDF rendering.
pub struct MsdfRenderer {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group: wgpu::BindGroup,
    pub bind_group_layout: wgpu::BindGroupLayout,
    #[allow(dead_code)]
    pub target_format: wgpu::TextureFormat,
}

// ── Per-frame resources stored in CallbackResources ──

/// Per-label GPU buffers created in `prepare()` and consumed in `paint()`.
/// Now split into static-atlas and dynamic-atlas buffer pairs.
pub struct MsdfFrameResources {
    /// Static-atlas glyph quads.
    pub vertex_buffer: Option<wgpu::Buffer>,
    pub index_buffer: Option<wgpu::Buffer>,
    pub num_indices: u32,
    /// Dynamic-atlas glyph quads (only present when text uses runtime glyphs).
    pub dynamic_vertex_buffer: Option<wgpu::Buffer>,
    pub dynamic_index_buffer: Option<wgpu::Buffer>,
    pub dynamic_num_indices: u32,
}

/// Map from label key → per-label resources.
/// Allows multiple labels in one frame without overwriting.
#[derive(Default)]
pub struct MsdfFrameResourceMap(pub HashMap<u64, MsdfFrameResources>);

// ── Atlas texture loading ──

/// Load atlas PNG via `image` crate and create the GPU pipeline + bind group.
/// Returns (pipeline, bind_group, bind_group_layout) for use each frame.
pub fn create_msdf_pipeline(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    atlas: &MsdfAtlas,
    target_format: wgpu::TextureFormat,
) -> (wgpu::RenderPipeline, wgpu::BindGroup, wgpu::BindGroupLayout) {
    // Decode PNG to RGBA via `image` crate
    let img = image::load_from_memory(&atlas.png_data)
        .expect("MSDF atlas PNG decode failed")
        .into_rgba8();
    let (w, h) = img.dimensions();
    let rgba = img.into_raw();

    let texture_size = wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("msdf_atlas_texture"),
        size: texture_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        &rgba,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) },
        texture_size,
    );

    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("msdf_atlas_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    let uniform = MsdfUniform {
        atlas_size: [w as f32, h as f32],
        px_range: atlas.distance_range,
        _pad: 0.0,
    };
    let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("msdf_uniform_buffer"),
        contents: bytemuck::cast_slice(&[uniform]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("msdf_bind_group_layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("msdf_bind_group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&texture_view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Buffer(uniform_buf.as_entire_buffer_binding()) },
        ],
    });

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("msdf_shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("msdf_pipeline_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("msdf_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[GlyphVertex::desc()],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview: None,
        cache: None,
    });

    (pipeline, bind_group, bind_group_layout)
}

// ── Text layout helpers ──

/// Layout a text string into a Vec of GlyphVertex quads.
/// Returns (vertices, indices).
/// `x0_ndc`, `y0_ndc` – baseline start in NDC.
/// `font_size_ndc` – em size in NDC units.
/// `color` – RGBA colour.
pub fn layout_text_ndc(
    atlas: &MsdfAtlas,
    text: &str,
    x0_ndc: f32,
    y0_ndc: f32,
    font_size_ndc: f32,
    color: [f32; 4],
) -> (Vec<GlyphVertex>, Vec<u16>) {
    let atlas_w = atlas.atlas_width as f32;
    let atlas_h = atlas.atlas_height as f32;
    // atlas glyphs use em-normalized advance/planeBounds (msdf-atlas-gen output),
    // so scale directly by desired font size.
    let em_scale = font_size_ndc;

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut cursor_x = x0_ndc;
    let mut base_idx: u16 = 0;

    for ch in text.chars() {
        let Some(glyph) = atlas.glyph(ch) else {
            // Missing glyph tofu
            let tofu_w = font_size_ndc * 0.5;
            let tofu_h = font_size_ndc;
            let (l, r) = (cursor_x, cursor_x + tofu_w);
            let (b, t) = (y0_ndc - tofu_h, y0_ndc);
            vertices.extend_from_slice(&[
                GlyphVertex { pos: [l, b], uv: [0.0, 0.0], color },
                GlyphVertex { pos: [r, b], uv: [0.0, 0.0], color },
                GlyphVertex { pos: [r, t], uv: [0.0, 0.0], color },
                GlyphVertex { pos: [l, t], uv: [0.0, 0.0], color },
            ]);
            let bi = base_idx;
            indices.extend_from_slice(&[bi, bi + 1, bi + 2, bi, bi + 2, bi + 3]);
            base_idx += 4;
            cursor_x += tofu_w;
            continue;
        };

        let Some(atlas_bounds) = glyph.atlas_bounds else {
            cursor_x += glyph.advance * em_scale;
            continue;
        };
        let Some(plane_bounds) = glyph.plane_bounds else {
            cursor_x += glyph.advance * em_scale;
            continue;
        };

        let l = cursor_x + plane_bounds.left * em_scale;
        let r = cursor_x + plane_bounds.right * em_scale;
        let b_ndc = y0_ndc + plane_bounds.bottom * em_scale;
        let t_ndc = y0_ndc + plane_bounds.top * em_scale;

        // UV: atlas y-origin = bottom. PNG row 0 = top of texture = v=0.
        // atlas_bounds.bottom is the minimum Y (bottom of glyph in atlas, closest to y=0).
        // Flip Y: bottom in atlas → larger v (closer to bottom of texture, v=1),
        //          top in atlas → smaller v (closer to top of texture, v=0).
        let u_l = atlas_bounds.left / atlas_w;
        let u_r = atlas_bounds.right / atlas_w;
        let v_t = 1.0 - atlas_bounds.bottom / atlas_h;
        let v_b = 1.0 - atlas_bounds.top / atlas_h;

        vertices.extend_from_slice(&[
            GlyphVertex { pos: [l, t_ndc], uv: [u_l, v_b], color },
            GlyphVertex { pos: [r, t_ndc], uv: [u_r, v_b], color },
            GlyphVertex { pos: [r, b_ndc], uv: [u_r, v_t], color },
            GlyphVertex { pos: [l, b_ndc], uv: [u_l, v_t], color },
        ]);
        let bi = base_idx;
        indices.extend_from_slice(&[bi, bi + 1, bi + 2, bi, bi + 2, bi + 3]);
        base_idx += 4;
        cursor_x += glyph.advance * em_scale;
    }

    (vertices, indices)
}

/// Compute text width in logical screen pixels.
/// Uses atlas glyph advances, so it's independent of screen/NDC conversion.
#[allow(dead_code)]
pub fn measure_text_width_screen(
    atlas: &MsdfAtlas,
    text: &str,
    font_size_px: f32,
) -> f32 {
    // atlas glyphs use em-normalized advance, so scale directly.
    let em_scale = font_size_px;
    let mut width = 0.0;
    for ch in text.chars() {
        if let Some(glyph) = atlas.glyph(ch) {
            width += glyph.advance * em_scale;
        } else {
            width += font_size_px * 0.5;
        }
    }
    width
}

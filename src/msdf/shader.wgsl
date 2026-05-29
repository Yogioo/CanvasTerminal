struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@group(0) @binding(0) var msdf_tex: texture_2d<f32>;
@group(0) @binding(1) var msdf_sampler: sampler;
@group(0) @binding(2) var<uniform> u: MsdfUniform;

struct MsdfUniform {
    atlas_size: vec2<f32>,
    px_range: f32,
    _pad: f32,
};

fn median(r: f32, g: f32, b: f32) -> f32 {
    return max(min(r, g), min(max(r, g), b));
}

@vertex
fn vs_main(
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
) -> VsOut {
    var out: VsOut;
    out.pos = vec4<f32>(pos, 0.0, 1.0);
    out.uv = uv;
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let sample = textureSample(msdf_tex, msdf_sampler, in.uv);
    let sd = median(sample.r, sample.g, sample.b) - 0.5;

    // Screen-space pixel range via derivative
    let unit_range = vec2<f32>(u.px_range) / u.atlas_size;
    let screen_tex_size = vec2<f32>(1.0) / fwidth(in.uv);
    let screen_px_range = max(0.5 * dot(unit_range, screen_tex_size), 1.0);

    let alpha = clamp(sd * screen_px_range + 0.5, 0.0, 1.0);
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}

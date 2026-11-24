// M26 stock post effect: dither. Reads f.x = mode (0 bayer4, 1 bayer8,
// 2 blue noise), f.y = levels, f.z = strength.

struct Params {
    v0: vec4<f32>,
    v1: vec4<f32>,
    v2: vec4<f32>,
    v3: vec4<f32>,
    f: vec4<f32>,
    i: vec4<i32>,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;
@group(1) @binding(0) var<uniform> params: Params;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    var uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    var out: VertexOutput;
    out.position = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(uv.x, 1.0 - uv.y);
    return out;
}

// @fx begin steps
const DITHER_BAYER4: array<f32, 16> = array<f32, 16>(
    0.0 / 16.0, 8.0 / 16.0, 2.0 / 16.0, 10.0 / 16.0,
    12.0 / 16.0, 4.0 / 16.0, 14.0 / 16.0, 6.0 / 16.0,
    3.0 / 16.0, 11.0 / 16.0, 1.0 / 16.0, 9.0 / 16.0,
    15.0 / 16.0, 7.0 / 16.0, 13.0 / 16.0, 5.0 / 16.0,
);

fn dither_bayer4(at: vec2<u32>) -> f32 {
    let idx = (at.y % 4u) * 4u + (at.x % 4u);
    return DITHER_BAYER4[idx];
}

fn dither_head(uv: vec2<f32>, p: Params) -> vec4<f32> {
    let sample = textureSample(src, src_sampler, uv);
    let dims = vec2<f32>(textureDimensions(src, 0));
    let pix = vec2<u32>(uv * dims);
    let threshold = dither_bayer4(pix) - 0.5;
    let levels = max(p.f.y, 2.0);
    let strength = clamp(p.f.z, 0.0, 1.0);
    let step_size = 1.0 / levels;
    let quantised = floor(sample.rgb / step_size + threshold * strength) * step_size;
    return vec4<f32>(clamp(quantised, vec3<f32>(0.0), vec3<f32>(1.0)), sample.a);
}
// @fx end

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return dither_head(in.uv, params);
}

// M26 stock post effect: pixel_outline. Samples alpha from 4-neighborhood and
// draws `v0` where the current pixel is below threshold but a neighbor isn't.
// Reads v0 = color, f.x = thickness_px, f.y = alpha_threshold.

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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(src, 0));
    let thickness = max(params.f.x, 1.0);
    let step = thickness / dims;
    let threshold = clamp(params.f.y, 0.0, 1.0);
    let center = textureSample(src, src_sampler, in.uv);
    let up = textureSample(src, src_sampler, in.uv + vec2<f32>(0.0, -step.y));
    let down = textureSample(src, src_sampler, in.uv + vec2<f32>(0.0, step.y));
    let left = textureSample(src, src_sampler, in.uv + vec2<f32>(-step.x, 0.0));
    let right = textureSample(src, src_sampler, in.uv + vec2<f32>(step.x, 0.0));
    let neighbor_max = max(max(up.a, down.a), max(left.a, right.a));
    if (center.a < threshold && neighbor_max >= threshold) {
        return vec4<f32>(params.v0.rgb, params.v0.a);
    }
    return center;
}

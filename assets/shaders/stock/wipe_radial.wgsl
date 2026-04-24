// M26 stock post effect: wipe_radial. Reads f.x = progress, f.y = softness,
// f.z = center.x, f.w = center.y.

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
    let sample = textureSample(src, src_sampler, in.uv);
    let center = vec2<f32>(params.f.z, params.f.w);
    let progress = clamp(params.f.x, 0.0, 1.0);
    let softness = max(params.f.y, 1e-4);
    let d = distance(in.uv, center);
    let radius = progress * 1.5;
    let t = smoothstep(radius - softness, radius + softness, d);
    return vec4<f32>(sample.rgb * (1.0 - t), sample.a);
}

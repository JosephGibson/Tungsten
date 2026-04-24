// M26 stock post effect: chromatic_aberration. Reads f.x = strength (world-space-ish,
// applied in UV units scaled by texture pixel size).

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
    let offset = (params.f.x / max(dims.x, 1.0)) * (in.uv - vec2<f32>(0.5));
    let r = textureSample(src, src_sampler, in.uv + offset).r;
    let g = textureSample(src, src_sampler, in.uv).g;
    let b = textureSample(src, src_sampler, in.uv - offset).b;
    let a = textureSample(src, src_sampler, in.uv).a;
    return vec4<f32>(r, g, b, a);
}

// A game's own post effect (`D-308`), for the fixtures: rows shifted sideways
// along a sine. f.x = the shift in UV, f.y = waves down the screen.
//
// The module opens with the stock post prefix (docs/assets.md) and marks no
// `@fx` section, so its pass always draws alone.

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
    let shift = sin(in.uv.y * params.f.y * 6.2831853) * params.f.x;
    return textureSample(src, src_sampler, vec2<f32>(in.uv.x + shift, in.uv.y));
}

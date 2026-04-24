// M26 stock post effect: vignette. Reads v0 = color, f.x = inner, f.y = outer,
// f.z = strength.

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
    let d = distance(in.uv, vec2<f32>(0.5, 0.5));
    let inner = params.f.x;
    let outer = max(params.f.y, inner + 1e-4);
    let t = clamp((d - inner) / (outer - inner), 0.0, 1.0);
    let strength = clamp(params.f.z, 0.0, 1.0);
    let tinted = mix(sample.rgb, params.v0.rgb, t * strength);
    return vec4<f32>(tinted, sample.a);
}

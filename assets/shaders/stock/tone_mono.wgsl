// M26 stock post effect: tone_mono. Reads v0 = tint_a, v1 = tint_b,
// f.x = mode (0 sepia, 1 mono, 2 duotone), f.y = amount.

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
    let luma = dot(sample.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let mode = u32(params.f.x);
    var tinted = vec3<f32>(luma);
    if (mode == 0u) {
        tinted = luma * params.v0.rgb;
    } else if (mode == 2u) {
        tinted = mix(params.v1.rgb, params.v0.rgb, luma);
    }
    let amount = clamp(params.f.y, 0.0, 1.0);
    return vec4<f32>(mix(sample.rgb, tinted, amount), sample.a);
}

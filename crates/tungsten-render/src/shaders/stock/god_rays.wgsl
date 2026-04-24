// M26 stock post effect: god_rays. Reads f.x = density, f.y = decay,
// f.z = weight, f.w = samples (cast to u32), v0.xy = center.

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
    let center = vec2<f32>(params.v0.x, params.v0.y);
    let samples = clamp(u32(params.f.w), 1u, 32u);
    let density = params.f.x;
    let decay = params.f.y;
    let weight = params.f.z;
    let threshold = vec3<f32>(0.6);
    var uv = in.uv;
    let delta = (in.uv - center) * (1.0 / f32(samples)) * density;
    var illumination = vec3<f32>(0.0);
    var attenuation = 1.0;
    // Only bright pixels seed rays — without a threshold a flat sky blows the
    // accumulator out before the intended highlights even land.
    for (var s: u32 = 0u; s < samples; s = s + 1u) {
        uv = uv - delta;
        let tap = textureSample(src, src_sampler, uv).rgb;
        let bright = max(tap - threshold, vec3<f32>(0.0));
        illumination = illumination + bright * attenuation;
        attenuation = attenuation * decay;
    }
    let rays = illumination * weight;
    return vec4<f32>(clamp(sample.rgb + rays, vec3<f32>(0.0), vec3<f32>(1.0)), sample.a);
}

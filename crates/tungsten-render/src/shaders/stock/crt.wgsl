// M26 stock post effect: crt. Reads f.x = scanline_strength, f.y = curvature,
// f.z = mask (as f32; cast to u32 in-shader), f.w = bleed.

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

fn curve_uv(uv: vec2<f32>, amount: f32) -> vec2<f32> {
    let centered = uv * 2.0 - 1.0;
    let offset = abs(centered.yx) / vec2<f32>(4.0, 3.0);
    return uv + centered * offset * amount;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(src, 0));
    let curved_uv = curve_uv(in.uv, params.f.y);
    if (any(curved_uv < vec2<f32>(0.0)) || any(curved_uv > vec2<f32>(1.0))) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    let sample = textureSample(src, src_sampler, curved_uv);
    let scan = 0.5 + 0.5 * cos(curved_uv.y * dims.y * 3.14159);
    let strength = clamp(params.f.x, 0.0, 1.0);
    let scanned = sample.rgb * mix(1.0, scan, strength);
    let mask_mode = u32(params.f.z);
    var masked = scanned;
    if (mask_mode == 1u) {
        let triad = vec3<f32>(1.2, 1.0, 0.9);
        let col = u32(floor(curved_uv.x * dims.x)) % 3u;
        masked = scanned * select(select(vec3<f32>(triad.z, 1.0, triad.y), vec3<f32>(1.0, triad.x, 1.0), col == 1u), triad, col == 0u);
    }
    return vec4<f32>(masked, sample.a);
}

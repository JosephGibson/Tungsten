// M26 stock post effect: dissolve. Reads v0 = edge_color, f.x = progress,
// f.y = noise_scale.

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

// @fx begin mix
// Offset input so the multiplicative hash doesn't collapse to 0 at (0, 0).
fn dissolve_hash12(at: vec2<f32>) -> f32 {
    let q = at + vec2<f32>(12.9898, 78.233);
    var p3 = fract(vec3<f32>(q.xyx) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn dissolve_value_noise(at: vec2<f32>) -> f32 {
    let i = floor(at);
    let f = fract(at);
    let u = f * f * (3.0 - 2.0 * f);
    let a = dissolve_hash12(i);
    let b = dissolve_hash12(i + vec2<f32>(1.0, 0.0));
    let c = dissolve_hash12(i + vec2<f32>(0.0, 1.0));
    let d = dissolve_hash12(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// 3-octave fBm keeps the threshold field continuous so the dissolve front
// eats away organically instead of looking like salt-and-pepper blocks.
// Normalized by the amplitude sum so the output stays in [0, 1].
fn dissolve_fbm(at: vec2<f32>) -> f32 {
    var v = 0.0;
    var amp = 0.5;
    var total = 0.0;
    var q = at;
    for (var i = 0; i < 3; i = i + 1) {
        v = v + amp * dissolve_value_noise(q);
        total = total + amp;
        q = q * 2.0;
        amp = amp * 0.5;
    }
    return v / total;
}

fn dissolve_fx(color: vec4<f32>, uv: vec2<f32>, p: Params) -> vec4<f32> {
    let progress = clamp(p.f.x, 0.0, 1.0);
    if (progress <= 0.0) {
        return color;
    }
    let scale = max(p.f.y, 1.0);
    let n = dissolve_fbm(uv * scale);
    let edge_width = 0.05;
    let d = n - progress;
    // Past the edge band on the low side: fully dissolved.
    if (d < -edge_width) {
        return vec4<f32>(0.0, 0.0, 0.0, color.a);
    }
    // Inside the band: triangular edge glow, peaking at d == 0 and fading
    // toward black on the dissolved side, toward the source on the visible
    // side.
    if (d < edge_width) {
        let t = d / edge_width;
        let edge_weight = 1.0 - abs(t);
        let trail = select(vec3<f32>(0.0), color.rgb, t > 0.0);
        let rgb = p.v0.rgb * edge_weight + trail * (1.0 - edge_weight);
        return vec4<f32>(rgb, color.a);
    }
    return color;
}
// @fx end

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return dissolve_fx(textureSample(src, src_sampler, in.uv), in.uv, params);
}

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

// Offset input so the multiplicative hash doesn't collapse to 0 at (0, 0).
fn hash12(p: vec2<f32>) -> f32 {
    let q = p + vec2<f32>(12.9898, 78.233);
    var p3 = fract(vec3<f32>(q.xyx) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash12(i);
    let b = hash12(i + vec2<f32>(1.0, 0.0));
    let c = hash12(i + vec2<f32>(0.0, 1.0));
    let d = hash12(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// 3-octave fBm keeps the threshold field continuous so the dissolve front
// eats away organically instead of looking like salt-and-pepper blocks.
// Normalized by the amplitude sum so the output stays in [0, 1].
fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var amp = 0.5;
    var total = 0.0;
    var q = p;
    for (var i = 0; i < 3; i = i + 1) {
        v = v + amp * value_noise(q);
        total = total + amp;
        q = q * 2.0;
        amp = amp * 0.5;
    }
    return v / total;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sample = textureSample(src, src_sampler, in.uv);
    let progress = clamp(params.f.x, 0.0, 1.0);
    if (progress <= 0.0) {
        return sample;
    }
    let scale = max(params.f.y, 1.0);
    let n = fbm(in.uv * scale);
    let edge_width = 0.05;
    let d = n - progress;
    // Past the edge band on the low side: fully dissolved.
    if (d < -edge_width) {
        return vec4<f32>(0.0, 0.0, 0.0, sample.a);
    }
    // Inside the band: triangular edge glow, peaking at d == 0 and fading
    // toward black on the dissolved side, toward the source on the visible
    // side.
    if (d < edge_width) {
        let t = d / edge_width;
        let edge_weight = 1.0 - abs(t);
        let trail = select(vec3<f32>(0.0), sample.rgb, t > 0.0);
        let rgb = params.v0.rgb * edge_weight + trail * (1.0 - edge_weight);
        return vec4<f32>(rgb, sample.a);
    }
    return sample;
}

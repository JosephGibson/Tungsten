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

fn hash12(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sample = textureSample(src, src_sampler, in.uv);
    let scale = max(params.f.y, 1.0);
    let n = hash12(floor(in.uv * scale));
    let progress = clamp(params.f.x, 0.0, 1.0);
    let edge_width = 0.05;
    // Dissolved blocks go to black; blocks in the transition band glow in
    // `edge_color`. Both branches stay inactive at progress == 0 so the
    // pass-through path is a true no-op.
    if (progress > 0.0 && n < progress) {
        return vec4<f32>(0.0, 0.0, 0.0, sample.a);
    }
    if (progress > 0.0 && n < progress + edge_width) {
        return vec4<f32>(params.v0.rgb, sample.a);
    }
    return sample;
}

// M26 stock post effect: tonemap. Reads f.x = mode, f.y = exposure, f.z = white.

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

fn aces_approx(c: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let y = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((c * (a * c + b)) / (c * (y * c + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sample = textureSample(src, src_sampler, in.uv);
    let mode = u32(params.f.x);
    let exposure = max(params.f.y, 0.0);
    let white = max(params.f.z, 1e-4);
    var rgb = sample.rgb * exposure;
    if (mode == 1u) {
        rgb = aces_approx(rgb);
    } else if (mode == 2u) {
        rgb = aces_approx(rgb * 0.6);
    } else {
        let m = max(white, 1e-4);
        rgb = rgb / (rgb + vec3<f32>(1.0));
        rgb = rgb * (m / (m - 1.0 + 1e-4));
    }
    return vec4<f32>(rgb, sample.a);
}

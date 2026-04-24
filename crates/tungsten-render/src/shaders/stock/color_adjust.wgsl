// M26 stock post effect: color_adjust. Reads f.x = hue (radians),
// f.y = saturation, f.z = contrast.

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

fn hue_rotate(c: vec3<f32>, angle: f32) -> vec3<f32> {
    let k = vec3<f32>(0.57735);
    let ca = cos(angle);
    return c * ca + cross(k, c) * sin(angle) + k * dot(k, c) * (1.0 - ca);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sample = textureSample(src, src_sampler, in.uv);
    var rgb = sample.rgb;
    rgb = hue_rotate(rgb, params.f.x);
    let luma = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    rgb = mix(vec3<f32>(luma), rgb, params.f.y);
    rgb = mix(vec3<f32>(0.5), rgb, params.f.z);
    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), sample.a);
}

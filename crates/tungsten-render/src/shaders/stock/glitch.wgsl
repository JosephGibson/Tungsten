// M26 stock post effect: glitch. Reads f.x = block_strength, f.y = shift_px,
// f.z = time_seed.

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
    let dims = vec2<f32>(textureDimensions(src, 0));
    let seed = params.f.z;
    let block_y = floor(in.uv.y * 40.0);
    let r = hash12(vec2<f32>(block_y, seed));
    let is_active = step(1.0 - clamp(params.f.x, 0.0, 1.0), r);
    let shift = params.f.y / max(dims.x, 1.0);
    let shifted_uv = in.uv + vec2<f32>(shift * (r - 0.5) * 2.0 * is_active, 0.0);
    return textureSample(src, src_sampler, shifted_uv);
}

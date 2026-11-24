// Bloom — 13-tap Karis-weighted downsample + 9-tap tent upsample, implemented
// for Tungsten M28.
// References: Jimenez/COD 2014 (Next Generation Post Processing in CoD: AW),
// Karis firefly weighting (Tone Mapping, GDC 2013 / SIGGRAPH 2014).
//
// Stage 4 — composite. Reads the slot's source view (group 0) and the bloom
// pyramid mip 0 (group 2), writes `mix(src, src + bloom * intensity, radius)`
// into the slot's target using replace blending so stale target contents do
// not accumulate across frames. Reads v0 = inv_src_size, v1 = composite_tint
// (reserved for a future tinted-bloom knob, white by default), f = threshold,
// knee, intensity, radius, i = mip_count, dst_level, pass_kind.

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

// @fx begin head
@group(2) @binding(0) var bloom_composite_mip0: texture_2d<f32>;
@group(2) @binding(1) var bloom_composite_sampler: sampler;

fn bloom_composite_head(uv: vec2<f32>, p: Params) -> vec4<f32> {
    let scene = textureSampleLevel(src, src_sampler, uv, 0.0);
    let bloom_rgb = textureSampleLevel(bloom_composite_mip0, bloom_composite_sampler, uv, 0.0).rgb
                  * p.v1.rgb;
    let intensity = p.f.z;
    let radius = clamp(p.f.w, 0.0, 1.0);
    let added = scene.rgb + bloom_rgb * intensity;
    let mixed = mix(scene.rgb, added, radius);
    return vec4<f32>(mixed, scene.a);
}
// @fx end

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return bloom_composite_head(in.uv, params);
}

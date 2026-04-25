// Bloom — 13-tap Karis-weighted downsample + 9-tap tent upsample, implemented
// for Tungsten M28.
// References: Jimenez/COD 2014 (Next Generation Post Processing in CoD: AW),
// Karis firefly weighting (Tone Mapping, GDC 2013 / SIGGRAPH 2014).
//
// Stage 4 — composite. Reads the slot's source view (group 0) and the bloom
// pyramid mip 0 (group 2), writes `mix(src, src + bloom * intensity, radius)`
// into the slot dst (PostPing/PostPong) using replace blending so stale dst
// contents do not accumulate across frames. composite_tint is reserved for a
// future tinted-bloom knob and defaults to white (1,1,1,1).

struct Params {
    inv_src_size: vec4<f32>,
    composite_tint: vec4<f32>,
    _vec4_2: vec4<f32>,
    _vec4_3: vec4<f32>,
    knobs: vec4<f32>,
    levels: vec4<i32>,
    _reserved: array<vec4<u32>, 10>,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;
@group(1) @binding(0) var<uniform> params: Params;
@group(2) @binding(0) var bloom_mip0: texture_2d<f32>;
@group(2) @binding(1) var bloom_sampler: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    let q = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    var out: VsOut;
    out.pos = vec4<f32>(q * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(q.x, 1.0 - q.y);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let scene = textureSampleLevel(src, src_sampler, in.uv, 0.0);
    let bloom_rgb = textureSampleLevel(bloom_mip0, bloom_sampler, in.uv, 0.0).rgb
                  * params.composite_tint.rgb;
    let intensity = params.knobs.z;
    let radius = clamp(params.knobs.w, 0.0, 1.0);
    let added = scene.rgb + bloom_rgb * intensity;
    let mixed = mix(scene.rgb, added, radius);
    return vec4<f32>(mixed, scene.a);
}

// Bloom — 13-tap Karis-weighted downsample + 9-tap tent upsample, implemented
// for Tungsten M28.
// References: Jimenez/COD 2014 (Next Generation Post Processing in CoD: AW),
// Karis firefly weighting (Tone Mapping, GDC 2013 / SIGGRAPH 2014).
//
// Stage 3 — 9-tap tent (3x3 hat) filter from a smaller mip into a larger one.
// Pipeline blend state is (One, One) on color so the fragment value is added
// onto whatever the destination mip already holds (the previous downsample
// output for that level). Tent weights 1-2-1 / 2-4-2 / 1-2-1 sum to 16.

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

fn fetch(uv: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(src, src_sampler, uv, 0.0).rgb;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let t = params.inv_src_size.xy;

    var acc = vec3<f32>(0.0);
    acc += fetch(in.uv + t * vec2<f32>(-1.0, -1.0)) * 1.0;
    acc += fetch(in.uv + t * vec2<f32>( 0.0, -1.0)) * 2.0;
    acc += fetch(in.uv + t * vec2<f32>( 1.0, -1.0)) * 1.0;
    acc += fetch(in.uv + t * vec2<f32>(-1.0,  0.0)) * 2.0;
    acc += fetch(in.uv                            ) * 4.0;
    acc += fetch(in.uv + t * vec2<f32>( 1.0,  0.0)) * 2.0;
    acc += fetch(in.uv + t * vec2<f32>(-1.0,  1.0)) * 1.0;
    acc += fetch(in.uv + t * vec2<f32>( 0.0,  1.0)) * 2.0;
    acc += fetch(in.uv + t * vec2<f32>( 1.0,  1.0)) * 1.0;

    return vec4<f32>(acc / 16.0, 1.0);
}

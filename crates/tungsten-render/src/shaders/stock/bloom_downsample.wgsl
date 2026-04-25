// Bloom — 13-tap Karis-weighted downsample + 9-tap tent upsample, implemented
// for Tungsten M28.
// References: Jimenez/COD 2014 (Next Generation Post Processing in CoD: AW),
// Karis firefly weighting (Tone Mapping, GDC 2013 / SIGGRAPH 2014).
//
// Stage 2 — 13-tap downsample with Karis-weighted average. Reads mip(level-1)
// and writes mip(level). Five 2x2 partial averages (one centered, four
// corners) are weighted by 1/(1+luma) to suppress single-pixel fireflies, then
// combined with the canonical (0.5, 0.125x4) group weights. Inv_src_size
// targets the SOURCE mip (the one being read), so taps stay aligned with mip
// texel centers.

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

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn fetch(uv: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(src, src_sampler, uv, 0.0).rgb;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let texel = params.inv_src_size.xy;

    let a = fetch(in.uv + texel * vec2<f32>(-1.0, -1.0));
    let b = fetch(in.uv + texel * vec2<f32>( 0.0, -1.0));
    let c = fetch(in.uv + texel * vec2<f32>( 1.0, -1.0));

    let d = fetch(in.uv + texel * vec2<f32>(-0.5, -0.5));
    let e = fetch(in.uv + texel * vec2<f32>( 0.5, -0.5));

    let f = fetch(in.uv + texel * vec2<f32>(-1.0,  0.0));
    let g = fetch(in.uv);
    let h = fetch(in.uv + texel * vec2<f32>( 1.0,  0.0));

    let i = fetch(in.uv + texel * vec2<f32>(-0.5,  0.5));
    let j = fetch(in.uv + texel * vec2<f32>( 0.5,  0.5));

    let k = fetch(in.uv + texel * vec2<f32>(-1.0,  1.0));
    let l = fetch(in.uv + texel * vec2<f32>( 0.0,  1.0));
    let m = fetch(in.uv + texel * vec2<f32>( 1.0,  1.0));

    // Five 2x2 partial averages: one centered (the four 0.5-offset taps) and
    // four corner groups. Group weights are 0.5 for the center and 0.125 for
    // each corner; sum to 1.0 in the unweighted case.
    let s_center = (d + e + i + j) * 0.25;
    let s_tl     = (a + b + f + g) * 0.25;
    let s_tr     = (b + c + g + h) * 0.25;
    let s_bl     = (f + g + k + l) * 0.25;
    let s_br     = (g + h + l + m) * 0.25;

    // Karis weight: 1 / (1 + luma) suppresses fireflies in the bright tail.
    // The renormalize-by-total step keeps overall energy stable when one
    // group is heavily attenuated.
    let w_center = 0.5   / (1.0 + luma(s_center));
    let w_tl     = 0.125 / (1.0 + luma(s_tl));
    let w_tr     = 0.125 / (1.0 + luma(s_tr));
    let w_bl     = 0.125 / (1.0 + luma(s_bl));
    let w_br     = 0.125 / (1.0 + luma(s_br));
    let w_total  = w_center + w_tl + w_tr + w_bl + w_br;

    let acc = s_center * w_center
            + s_tl     * w_tl
            + s_tr     * w_tr
            + s_bl     * w_bl
            + s_br     * w_br;
    return vec4<f32>(acc / max(w_total, 1e-6), 1.0);
}

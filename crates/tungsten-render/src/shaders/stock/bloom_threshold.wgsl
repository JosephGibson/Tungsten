// Bloom — 13-tap Karis-weighted downsample + 9-tap tent upsample, implemented
// for Tungsten M28.
// References: Jimenez/COD 2014 (Next Generation Post Processing in CoD: AW),
// Karis firefly weighting (Tone Mapping, GDC 2013 / SIGGRAPH 2014).
//
// Stage 1 — bright-pass with COD soft-knee curve. Samples the slot source
// (sRGB attachments decode to linear automatically) and writes mip 0 of the
// Rgba16Float bloom pyramid. UBO layout mirrors UniformOverrideBlock (256 B):
//   knobs.x = threshold, knobs.y = knee, knobs.z = intensity, knobs.w = radius.
//   levels.w = how many times the source was halved for mip 0: 1 or 2.

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

// One bilinear tap at `uv` through the soft-knee curve: the average of the
// 2x2 source texels around it, kept where it is bright.
fn bright(uv: vec2<f32>) -> vec3<f32> {
    let c = textureSampleLevel(src, src_sampler, uv, 0.0).rgb;
    let threshold = params.knobs.x;
    let knee = max(params.knobs.y, 1e-5);

    let br = max(c.r, max(c.g, c.b));
    var soft = br - threshold + knee;
    soft = clamp(soft, 0.0, 2.0 * knee);
    soft = soft * soft / (4.0 * knee + 1e-6);
    let contribution = max(soft, br - threshold);
    let factor = contribution / max(br, 1e-6);
    return c * factor;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if (params.levels.w >= 2) {
        // Mip 0 is a quarter of the source (D-301): a texel covers 4x4 source
        // texels, read whole by four taps a texel off its centre, each kept
        // by the curve before they are averaged, so mip 0 is the half-size
        // pyramid's first level averaged down and a small highlight keeps
        // its light.
        let t = params.inv_src_size.xy;
        let sum = bright(in.uv + t * vec2<f32>(-1.0, -1.0))
                + bright(in.uv + t * vec2<f32>( 1.0, -1.0))
                + bright(in.uv + t * vec2<f32>(-1.0,  1.0))
                + bright(in.uv + t * vec2<f32>( 1.0,  1.0));
        return vec4<f32>(sum * 0.25, 1.0);
    }
    return vec4<f32>(bright(in.uv), 1.0);
}

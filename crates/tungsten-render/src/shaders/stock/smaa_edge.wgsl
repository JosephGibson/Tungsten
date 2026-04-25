// SMAA 1x — Jorge Jimenez, Jose I. Echevarria, Tiago Sousa, Diego Gutierrez. MIT.
// https://www.iryoku.com/smaa/
//
// Stage 1: luma-based edge detection with local contrast adaptation.
// Reads the post-stack output (gamma-encoded; sampled through a non-sRGB twin
// view), writes a 2-channel edges target where:
//   r = vertical edge with the left neighbor
//   g = horizontal edge with the top neighbor

struct Preset {
    threshold: f32,
    max_search_steps: f32,
    max_search_steps_diag: f32,
    corner_rounding: f32,
    rt_metrics: vec4<f32>,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;
@group(1) @binding(0) var<uniform> preset: Preset;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) off0: vec4<f32>,
    @location(2) off1: vec4<f32>,
    @location(3) off2: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    let q = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    var out: VsOut;
    out.pos = vec4<f32>(q * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(q.x, 1.0 - q.y);
    let m = preset.rt_metrics.xy;
    out.off0 = vec4<f32>(out.uv - vec2<f32>(m.x, 0.0), out.uv - vec2<f32>(0.0, m.y));
    out.off1 = vec4<f32>(out.uv + vec2<f32>(m.x, 0.0), out.uv + vec2<f32>(0.0, m.y));
    out.off2 = vec4<f32>(out.uv - vec2<f32>(2.0 * m.x, 0.0), out.uv - vec2<f32>(0.0, 2.0 * m.y));
    return out;
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let l_c = luma(textureSampleLevel(src, src_sampler, in.uv, 0.0).rgb);
    let l_l = luma(textureSampleLevel(src, src_sampler, in.off0.xy, 0.0).rgb);
    let l_t = luma(textureSampleLevel(src, src_sampler, in.off0.zw, 0.0).rgb);

    let t = max(preset.threshold, 1e-6);
    let delta_lt = abs(vec2<f32>(l_c - l_l, l_c - l_t));
    var edges = step(vec2<f32>(t, t), delta_lt);

    if (edges.x + edges.y < 1e-6) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    let l_r = luma(textureSampleLevel(src, src_sampler, in.off1.xy, 0.0).rgb);
    let l_b = luma(textureSampleLevel(src, src_sampler, in.off1.zw, 0.0).rgb);
    let delta_rb = abs(vec2<f32>(l_c - l_r, l_c - l_b));

    var max_delta = max(delta_lt, delta_rb);

    let l_ll = luma(textureSampleLevel(src, src_sampler, in.off2.xy, 0.0).rgb);
    let l_tt = luma(textureSampleLevel(src, src_sampler, in.off2.zw, 0.0).rgb);
    let delta_far = abs(vec2<f32>(l_l - l_ll, l_t - l_tt));
    max_delta = max(max_delta, delta_far);

    let max_final = max(max_delta.x, max_delta.y);
    edges = edges * step(vec2<f32>(0.5 * max_final), delta_lt);

    return vec4<f32>(edges, 0.0, 1.0);
}

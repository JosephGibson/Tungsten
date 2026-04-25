// SMAA 1x — Jorge Jimenez, Jose I. Echevarria, Tiago Sousa, Diego Gutierrez. MIT.
// https://www.iryoku.com/smaa/
//
// Stage 2: blend-weights calculation. For each pixel marked as an edge by the
// stage-1 output, walk along the edge to find its endpoints (with help of
// `search` LUT), then read `area` LUT to convert (run-length, edge-cap shape)
// into a 4-channel blend weight (left/top/right/bottom packed into RGBA).
//
// This port implements the orthogonal path. Diagonal and corner detection
// are gated by preset sentinels:
//   - `max_search_steps_diag <= 0` skips the diagonal pass
//   - `corner_rounding >= 1e9`     skips the corner pass
// The orthogonal path alone yields visible AA improvement and matches the
// quality scaling the four presets request via threshold + max_search_steps.

const AREATEX_W: f32 = 160.0;
const AREATEX_H: f32 = 560.0;
const AREATEX_INV: vec2<f32> = vec2<f32>(1.0 / 160.0, 1.0 / 560.0);
const AREATEX_MAX_DISTANCE: f32 = 16.0;
const SEARCHTEX_W: f32 = 64.0;
const SEARCHTEX_H: f32 = 16.0;
const SEARCHTEX_INV: vec2<f32> = vec2<f32>(1.0 / 64.0, 1.0 / 16.0);

struct Preset {
    threshold: f32,
    max_search_steps: f32,
    max_search_steps_diag: f32,
    corner_rounding: f32,
    rt_metrics: vec4<f32>,
};

@group(0) @binding(0) var edges_tex: texture_2d<f32>;
@group(0) @binding(1) var edges_sampler: sampler;
@group(1) @binding(0) var area_tex: texture_2d<f32>;
@group(1) @binding(1) var search_tex: texture_2d<f32>;
@group(1) @binding(2) var lut_sampler: sampler;
@group(1) @binding(3) var<uniform> preset: Preset;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) pix: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    let q = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    var out: VsOut;
    out.pos = vec4<f32>(q * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(q.x, 1.0 - q.y);
    out.pix = out.uv * preset.rt_metrics.zw;
    return out;
}

fn search_length(e: vec2<f32>, off: f32) -> f32 {
    let scale = vec2<f32>(SEARCHTEX_W * 0.5, -SEARCHTEX_H);
    let bias = vec2<f32>(SEARCHTEX_W * off, SEARCHTEX_H);
    let coord = (scale * e + bias) * SEARCHTEX_INV;
    return textureSampleLevel(search_tex, lut_sampler, coord, 0.0).r;
}

fn search_xleft(uv0: vec2<f32>, end_x: f32) -> f32 {
    var e = vec2<f32>(0.0, 1.0);
    var coord = uv0;
    let step_uv = vec2<f32>(-2.0 * preset.rt_metrics.x, 0.0);
    let max_steps = i32(round(preset.max_search_steps));
    for (var i: i32 = 0; i < max_steps; i = i + 1) {
        if (coord.x <= end_x) { break; }
        e = textureSampleLevel(edges_tex, edges_sampler, coord, 0.0).rg;
        if (e.g < 0.8281) { break; }
        if (e.r != 0.0) { break; }
        coord = coord + step_uv;
    }
    let off = -(255.0 / 127.0) * search_length(e, 0.0) + 3.25;
    return preset.rt_metrics.x * off + coord.x;
}

fn search_xright(uv0: vec2<f32>, end_x: f32) -> f32 {
    var e = vec2<f32>(0.0, 1.0);
    var coord = uv0;
    let step_uv = vec2<f32>(2.0 * preset.rt_metrics.x, 0.0);
    let max_steps = i32(round(preset.max_search_steps));
    for (var i: i32 = 0; i < max_steps; i = i + 1) {
        if (coord.x >= end_x) { break; }
        e = textureSampleLevel(edges_tex, edges_sampler, coord, 0.0).rg;
        if (e.g < 0.8281) { break; }
        if (e.r != 0.0) { break; }
        coord = coord + step_uv;
    }
    let off = -(255.0 / 127.0) * search_length(e, 0.5) + 3.25;
    return -preset.rt_metrics.x * off + coord.x;
}

fn search_yup(uv0: vec2<f32>, end_y: f32) -> f32 {
    var e = vec2<f32>(1.0, 0.0);
    var coord = uv0;
    let step_uv = vec2<f32>(0.0, -2.0 * preset.rt_metrics.y);
    let max_steps = i32(round(preset.max_search_steps));
    for (var i: i32 = 0; i < max_steps; i = i + 1) {
        if (coord.y <= end_y) { break; }
        e = textureSampleLevel(edges_tex, edges_sampler, coord, 0.0).rg;
        if (e.r < 0.8281) { break; }
        if (e.g != 0.0) { break; }
        coord = coord + step_uv;
    }
    let off = -(255.0 / 127.0) * search_length(vec2<f32>(e.g, e.r), 0.0) + 3.25;
    return preset.rt_metrics.y * off + coord.y;
}

fn search_ydown(uv0: vec2<f32>, end_y: f32) -> f32 {
    var e = vec2<f32>(1.0, 0.0);
    var coord = uv0;
    let step_uv = vec2<f32>(0.0, 2.0 * preset.rt_metrics.y);
    let max_steps = i32(round(preset.max_search_steps));
    for (var i: i32 = 0; i < max_steps; i = i + 1) {
        if (coord.y >= end_y) { break; }
        e = textureSampleLevel(edges_tex, edges_sampler, coord, 0.0).rg;
        if (e.r < 0.8281) { break; }
        if (e.g != 0.0) { break; }
        coord = coord + step_uv;
    }
    let off = -(255.0 / 127.0) * search_length(vec2<f32>(e.g, e.r), 0.5) + 3.25;
    return -preset.rt_metrics.y * off + coord.y;
}

fn area_sample(dist: vec2<f32>, e1: f32, e2: f32, offset: f32) -> vec2<f32> {
    var coord = AREATEX_MAX_DISTANCE * round(4.0 * vec2<f32>(e1, e2)) + dist;
    coord = coord * AREATEX_INV + 0.5 * AREATEX_INV;
    coord.y = coord.y + (1.0 / 7.0) * offset;
    return textureSampleLevel(area_tex, lut_sampler, coord, 0.0).rg;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var weights = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    let e = textureSampleLevel(edges_tex, edges_sampler, in.uv, 0.0).rg;
    let m = preset.rt_metrics;

    if (e.g > 0.0) {
        // Top edge: search left/right along the horizontal edge run.
        let off0_l = in.uv + m.xy * vec2<f32>(-0.25, -0.125);
        let off0_r = in.uv + m.xy * vec2<f32>( 1.25, -0.125);
        let off2_l = in.uv.x + m.x * (-2.0 * preset.max_search_steps - 0.25);
        let off2_r = in.uv.x + m.x * ( 2.0 * preset.max_search_steps + 0.25);

        let left = search_xleft(off0_l, off2_l);
        let right = search_xright(off0_r, off2_r);

        let d = abs(round(vec2<f32>(left, right) * m.z - in.pix.xx));

        let coord_l = vec2<f32>(left, in.uv.y - 0.25 * m.y) + vec2<f32>(0.0, m.y);
        let coord_r = vec2<f32>(right + m.x, in.uv.y - 0.25 * m.y) + vec2<f32>(0.0, m.y);
        let e1 = textureSampleLevel(edges_tex, edges_sampler, coord_l, 0.0).r;
        let e2 = textureSampleLevel(edges_tex, edges_sampler, coord_r, 0.0).r;

        let w = area_sample(sqrt(d), e1, e2, 0.0);
        weights.r = w.x;
        weights.g = w.y;
    }

    if (e.r > 0.0) {
        // Left edge: search up/down along the vertical edge run.
        let off1_t = in.uv + m.xy * vec2<f32>(-0.125, -0.25);
        let off1_b = in.uv + m.xy * vec2<f32>(-0.125,  1.25);
        let off2_t = in.uv.y + m.y * (-2.0 * preset.max_search_steps - 0.25);
        let off2_b = in.uv.y + m.y * ( 2.0 * preset.max_search_steps + 0.25);

        let up = search_yup(off1_t, off2_t);
        let down = search_ydown(off1_b, off2_b);

        let d = abs(round(vec2<f32>(up, down) * m.w - in.pix.yy));

        let coord_t = vec2<f32>(in.uv.x - 0.25 * m.x, up) + vec2<f32>(m.x, 0.0);
        let coord_b = vec2<f32>(in.uv.x - 0.25 * m.x, down + m.y) + vec2<f32>(m.x, 0.0);
        let e1 = textureSampleLevel(edges_tex, edges_sampler, coord_t, 0.0).g;
        let e2 = textureSampleLevel(edges_tex, edges_sampler, coord_b, 0.0).g;

        let w = area_sample(sqrt(d), e1, e2, 0.0);
        weights.b = w.x;
        weights.a = w.y;
    }

    return weights;
}

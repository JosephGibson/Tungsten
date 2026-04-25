// SMAA 1x — Jorge Jimenez, Jose I. Echevarria, Tiago Sousa, Diego Gutierrez. MIT.
// https://www.iryoku.com/smaa/
//
// Stage 3: neighborhood blending. Reads the source color (post-stack output)
// and the per-pixel blend weights from stage 2, then composites the AA'd
// result into `PresentSource`. The text overlay and present blit run *after*
// this stage so screen-space text is never sampled by SMAA.

struct Preset {
    threshold: f32,
    max_search_steps: f32,
    max_search_steps_diag: f32,
    corner_rounding: f32,
    rt_metrics: vec4<f32>,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;
@group(1) @binding(0) var blend_tex: texture_2d<f32>;
@group(1) @binding(1) var blend_sampler: sampler;
@group(1) @binding(2) var<uniform> preset: Preset;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) off: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    let q = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    var out: VsOut;
    out.pos = vec4<f32>(q * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(q.x, 1.0 - q.y);
    let m = preset.rt_metrics.xy;
    out.off = vec4<f32>(out.uv + vec2<f32>( m.x, 0.0),
                       out.uv + vec2<f32>(0.0,  m.y));
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Sample blend weights at four offsets and gather: a.x = right pixel's
    // a (right blend on the left), a.y = bottom pixel's g (top blend on the
    // bottom), a.z = current's a (right), a.w = current's g (top). Reference
    // packing: blend_tex.r = top, blend_tex.g = left???
    // Per SMAA reference: w.r = top, w.g = right, w.b = left, w.a = bottom.
    let a_left = textureSampleLevel(blend_tex, blend_sampler, in.uv, 0.0);
    let a_right_neighbor = textureSampleLevel(blend_tex, blend_sampler, in.off.xy, 0.0);
    let a_bottom_neighbor = textureSampleLevel(blend_tex, blend_sampler, in.off.zw, 0.0);

    // Effective four weights to choose between: left (this pixel's blue),
    // top (this pixel's red), right (right neighbor's left=blue),
    // bottom (bottom neighbor's top=red).
    let a = vec4<f32>(a_right_neighbor.b, a_bottom_neighbor.r, a_left.b, a_left.r);

    let sum = dot(a, vec4<f32>(1.0, 1.0, 1.0, 1.0));
    if (sum < 1e-5) {
        return textureSampleLevel(src, src_sampler, in.uv, 0.0);
    }

    let h_max = max(a.x, a.z);
    let v_max = max(a.y, a.w);
    let horizontal = h_max > v_max;

    let m = preset.rt_metrics.xy;
    var blending_offset = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    var blending_weight = vec2<f32>(0.0, 0.0);

    if (horizontal) {
        blending_offset = vec4<f32>(0.0, 0.0, 1.0, 0.0);
        blending_weight = vec2<f32>(a.x, a.z);
    } else {
        blending_offset = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        blending_weight = vec2<f32>(a.y, a.w);
    }

    let total = max(blending_weight.x + blending_weight.y, 1e-6);
    blending_weight = blending_weight / vec2<f32>(total, total);

    var blending_coord = vec4<f32>(in.uv, in.uv) + vec4<f32>(m.x, m.y, -m.x, -m.y) * blending_offset;
    var color = blending_weight.x * textureSampleLevel(src, src_sampler, blending_coord.xy, 0.0);
    color = color + blending_weight.y * textureSampleLevel(src, src_sampler, blending_coord.zw, 0.0);

    return color;
}

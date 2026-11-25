struct Camera {
    projection: mat4x4<f32>,
};
@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var t_sprite: texture_2d<f32>;
@group(1) @binding(1)
var s_sprite: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct InstanceInput {
    @location(2) inst_pos: vec2<f32>,
    @location(3) inst_size: vec2<f32>,
    @location(4) inst_rot: f32,
    @location(5) inst_tint: vec4<f32>,
    @location(6) inst_uv_min: vec2<f32>,
    @location(7) inst_uv_size: vec2<f32>,
    @location(8) inst_z: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) tint: vec4<f32>,
};

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    // Centre-origin local position so the rotation pivots around the quad
    // centre. When `inst_rot == 0`, this reduces to the pre-M15 expression
    //   world_pos = instance.inst_pos + vertex.position * instance.inst_size
    // so existing callers that pass `rotation = 0.0` see no visual change.
    let local = vertex.position - vec2<f32>(0.5, 0.5);
    let scaled = local * instance.inst_size;
    let c = cos(instance.inst_rot);
    let s = sin(instance.inst_rot);
    let rotated = vec2<f32>(
        scaled.x * c - scaled.y * s,
        scaled.x * s + scaled.y * c,
    );
    let centre = instance.inst_pos + instance.inst_size * 0.5;
    let world_pos = centre + rotated;

    var out: VertexOutput;
    // `inst_z` is a pass-through in [0, 1], the clip range. No depth reads
    // it: under `DepthSortMode::GpuDepth` the draw sets each batch's depth
    // by the viewport's depth range, which maps every `inst_z` in the clip
    // range onto the batch's place in the frame's list (redo 7.15), and
    // under `CpuStable` the pass has no depth attachment.
    out.clip_position = camera.projection * vec4<f32>(world_pos, 0.0, 1.0);
    out.clip_position.z = instance.inst_z;
    out.tex_coord = instance.inst_uv_min + vertex.uv * instance.inst_uv_size;
    out.tint = instance.inst_tint;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_sprite, s_sprite, in.tex_coord) * in.tint;
}

// A solid batch's fragment under the depth test (redo 7.15): every texel
// the batch samples is clear or opaque and its tint's alpha is 1, so a
// clear texel, which blends to nothing, is dropped before it writes depth,
// and an opaque one draws what `fs_main` draws.
@fragment
fn fs_solid(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSample(t_sprite, s_sprite, in.tex_coord);
    if (texel.a < 0.5) {
        discard;
    }
    return texel * in.tint;
}

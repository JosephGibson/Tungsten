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
    out.clip_position = camera.projection * vec4<f32>(world_pos, 0.0, 1.0);
    out.tex_coord = vertex.uv;
    out.tint = instance.inst_tint;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_sprite, s_sprite, in.tex_coord) * in.tint;
}

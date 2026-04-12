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
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
};

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = instance.inst_pos + vertex.position * instance.inst_size;
    out.clip_position = camera.projection * vec4<f32>(world_pos, 0.0, 1.0);
    out.tex_coord = vertex.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_sprite, s_sprite, in.tex_coord);
}

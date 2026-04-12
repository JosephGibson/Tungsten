struct Camera {
    projection: mat4x4<f32>,
};
@group(0) @binding(0)
var<uniform> camera: Camera;

struct VertexInput {
    @location(0) position: vec2<f32>,
};

struct InstanceInput {
    @location(1) inst_pos: vec2<f32>,
    @location(2) inst_size: vec2<f32>,
    @location(3) inst_color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = instance.inst_pos + vertex.position * instance.inst_size;
    out.clip_position = camera.projection * vec4<f32>(world_pos, 0.0, 1.0);
    out.color = instance.inst_color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}

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
    @location(2) inst_scale: vec2<f32>,
    @location(3) inst_rot: f32,
    @location(4) inst_color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    // Mesh-local pixels: scale, then rotate about the mesh origin with the
    // sign `sprite.wgsl` uses, then move the origin to the particle position.
    let scaled = vertex.position * instance.inst_scale;
    let c = cos(instance.inst_rot);
    let s = sin(instance.inst_rot);
    let rotated = vec2<f32>(
        scaled.x * c - scaled.y * s,
        scaled.x * s + scaled.y * c,
    );
    let world_pos = instance.inst_pos + rotated;

    var out: VertexOutput;
    out.clip_position = camera.projection * vec4<f32>(world_pos, 0.0, 1.0);
    out.color = instance.inst_color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}

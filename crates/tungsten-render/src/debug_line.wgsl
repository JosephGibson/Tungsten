struct Camera {
    projection: mat4x4<f32>,
};
@group(0) @binding(0)
var<uniform> camera: Camera;

struct VertexInput {
    @location(0) corner: vec2<f32>,
};

struct InstanceInput {
    @location(1) a: vec2<f32>,
    @location(2) b: vec2<f32>,
    @location(3) thickness: f32,
    @location(4) _pad: f32,
    @location(5) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    let t = vertex.corner.x;
    let s = vertex.corner.y - 0.5;

    let along = instance.b - instance.a;
    let len = max(length(along), 0.000001);
    let dir = along / len;
    let perp = vec2<f32>(-dir.y, dir.x);

    let world_pos = mix(instance.a, instance.b, t) + perp * s * instance.thickness;

    var out: VertexOutput;
    out.clip_position = camera.projection * vec4<f32>(world_pos, 0.0, 1.0);
    out.color = instance.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}

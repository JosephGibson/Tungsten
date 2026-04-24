// M26 workspace material: damage_flash.
// Reads `material.vec4[0]` as the overlay colour and `material.f32s[0]` as
// the overlay amount. Uniform block shape matches `UniformOverrideBlock` so
// tween-driven overrides (e.g. one-shot fade) drop straight in.

struct Camera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;

@group(1) @binding(0) var sprite_tex: texture_2d<f32>;
@group(1) @binding(1) var sprite_sampler: sampler;

struct Material {
    v0: vec4<f32>,
    v1: vec4<f32>,
    v2: vec4<f32>,
    v3: vec4<f32>,
    f: vec4<f32>,
    i: vec4<i32>,
};

@group(2) @binding(0) var<uniform> material: Material;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) instance_position: vec2<f32>,
    @location(3) instance_size: vec2<f32>,
    @location(4) instance_rotation: f32,
    @location(5) instance_color: vec4<f32>,
    @location(6) uv_min: vec2<f32>,
    @location(7) uv_size: vec2<f32>,
    @location(8) z_norm: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    let local = (in.position - vec2<f32>(0.5, 0.5)) * in.instance_size;
    let c = cos(in.instance_rotation);
    let s = sin(in.instance_rotation);
    let rotated = vec2<f32>(
        local.x * c - local.y * s,
        local.x * s + local.y * c,
    );
    let world = rotated + in.instance_position + in.instance_size * 0.5;
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(world, in.z_norm, 1.0);
    out.uv = in.uv_min + in.uv * in.uv_size;
    out.color = in.instance_color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sample = textureSample(sprite_tex, sprite_sampler, in.uv) * in.color;
    let amount = clamp(material.f.x, 0.0, 1.0);
    let overlay = material.v0;
    let rgb = mix(sample.rgb, overlay.rgb, amount * overlay.a);
    return vec4<f32>(rgb, sample.a);
}

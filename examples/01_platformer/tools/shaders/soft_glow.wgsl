// Example 01 material: soft_glow.
// Draws an analytic radial glow across the sprite quad instead of sampling a
// stepped halo texture, so lamp, fire, crystal and moon glows stay smooth at
// any size. A screen-space hash dithers alpha by at most one 8-bit step to
// break banding in dark gradients.
//   material.f.x  falloff exponent (higher = tighter)
//   material.f.y  hot-core boost
//   material.f.z  dither strength, in 8-bit steps
//   material.f.w  peak alpha before the instance colour's alpha
// The sprite texture stays bound because the sprite layout requires it.

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
    @location(0) local: vec2<f32>,
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
    out.local = in.position;
    out.color = in.instance_color;
    return out;
}

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let d = length(in.local * 2.0 - vec2<f32>(1.0, 1.0));
    let body = pow(clamp(1.0 - d, 0.0, 1.0), max(material.f.x, 0.1));
    let core = material.f.y * pow(clamp(1.0 - d * 2.5, 0.0, 1.0), 2.0);
    let dither = (hash(floor(in.clip_position.xy)) - 0.5) * material.f.z / 255.0;
    let alpha = clamp((body + core) * material.f.w * in.color.a + dither, 0.0, 1.0);
    return vec4<f32>(in.color.rgb, alpha);
}

// Benchmark material: bench_heavy (example-02-bench `gpu`).
// Per-fragment cost scales with `material.i.x`: the fragment stage runs that
// many iterations of a trigonometric feedback loop, so one knob
// (`shader_iters`) scales fragment cost without a recompile. `material.v0`
// tints the loop's colour and `material.f.x` mixes it over the texture.
// Vertex stage and bind layout match damage_flash.wgsl (the material
// pipeline's sprite layout plus group 2).

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
    @location(2) local: vec2<f32>,
};

// Upper bound on the loop, whatever the uniform holds.
const MAX_ITERATIONS: i32 = 4096;

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
    out.local = in.position;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sample = textureSample(sprite_tex, sprite_sampler, in.uv) * in.color;
    let iterations = clamp(material.i.x, 0, MAX_ITERATIONS);
    var p = in.local * 6.2831853;
    var acc = vec3<f32>(0.0);
    for (var n = 0; n < iterations; n = n + 1) {
        p = vec2<f32>(sin(p.y * 1.7 + p.x), cos(p.x * 1.3 - p.y)) * 1.9 + vec2<f32>(0.37, -0.21);
        acc = acc + 0.5 + 0.5 * cos(vec3<f32>(p.x, p.y, p.x + p.y) + vec3<f32>(0.0, 2.1, 4.2));
    }
    let pattern = acc / f32(max(iterations, 1)) * material.v0.rgb;
    let amount = select(0.0, clamp(material.f.x, 0.0, 1.0), iterations > 0);
    let rgb = mix(sample.rgb, pattern * (0.6 + 0.4 * sample.rgb), amount);
    return vec4<f32>(rgb, sample.a);
}

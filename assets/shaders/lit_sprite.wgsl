// M29 forward 2D normal-mapped sprite shader.
//
// References (concept only — no copied code):
//   - LearnOpenGL — Multiple Lights / Normal Mapping
//   - Godot CanvasItem normal-map docs
//
// Bind groups:
//   group(0) @binding(0): camera (matches sprite.wgsl)
//   group(1) @binding(0): albedo (Rgba8UnormSrgb)
//   group(1) @binding(1): normal  (Rgba8Unorm, tangent-space)
//   group(1) @binding(2): emissive (Rgba8Unorm, premultiplied RGB)
//   group(1) @binding(3): sampler (filtering)
//   group(2) @binding(0): LightUbo (16 lights + count + ambient)

struct Camera {
    projection: mat4x4<f32>,
};
@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var t_albedo: texture_2d<f32>;
@group(1) @binding(1)
var t_normal: texture_2d<f32>;
@group(1) @binding(2)
var t_emissive: texture_2d<f32>;
@group(1) @binding(3)
var s_lit: sampler;

struct GpuLight {
    position_radius: vec4<f32>,
    color_intensity: vec4<f32>,
};

struct LightUbo {
    lights: array<GpuLight, 16>,
    count_pad: vec4<u32>,
    ambient: vec4<f32>,
};

@group(2) @binding(0)
var<uniform> lighting: LightUbo;

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
    @location(2) world_pos: vec2<f32>,
};

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
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
    out.clip_position.z = instance.inst_z;
    out.tex_coord = instance.inst_uv_min + vertex.uv * instance.inst_uv_size;
    out.tint = instance.inst_tint;
    out.world_pos = world_pos;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let albedo = textureSample(t_albedo, s_lit, in.tex_coord) * in.tint;
    let n_sample = textureSample(t_normal, s_lit, in.tex_coord).xyz * 2.0 - vec3<f32>(1.0);
    // 2D shader treats `(n.xy, n.z)` as world-axis normals; sprite plane is
    // assumed parallel to the world plane (no per-fragment tangent frame).
    let n = normalize(n_sample);

    var rgb = albedo.rgb * lighting.ambient.rgb;
    let count = lighting.count_pad.x;
    for (var i: u32 = 0u; i < count; i = i + 1u) {
        let l = lighting.lights[i];
        if (l.color_intensity.w == 1.0) {
            // Directional: position_radius.xy = (cos, sin) of the angle.
            let l_dir = normalize(vec3<f32>(l.position_radius.xy, 1.0));
            let n_dot_l = max(0.0, dot(n, l_dir));
            rgb = rgb + albedo.rgb * l.color_intensity.rgb * n_dot_l;
        } else {
            // Point light: model the source as floating above the sprite plane
            // at z = radius * 0.5 so flat normals (0,0,1) still receive
            // meaningful diffuse. Without the height term, world-space XY
            // distances make `normalize((dx, dy, 1))` collapse to horizontal
            // and N·L hits zero a few pixels away.
            let radius = max(l.position_radius.z, 0.0001);
            let to_light_xy = l.position_radius.xy - in.world_pos;
            let to_light_3d = vec3<f32>(to_light_xy, radius * 0.5);
            let dist_xy = length(to_light_xy);
            let attenuation = clamp(1.0 - dist_xy / radius, 0.0, 1.0);
            let l_dir = normalize(to_light_3d);
            let n_dot_l = max(0.0, dot(n, l_dir));
            rgb = rgb + albedo.rgb * l.color_intensity.rgb * n_dot_l * attenuation * attenuation;
        }
    }

    let emissive = textureSample(t_emissive, s_lit, in.tex_coord).rgb;
    rgb = rgb + emissive;
    return vec4<f32>(rgb, albedo.a);
}

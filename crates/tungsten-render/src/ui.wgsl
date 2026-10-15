// The UI primitive shader (M46, w01 §6): one unit quad per instance, a
// rounded-rectangle signed distance for the fill and the border with
// one-pixel analytic anti-aliasing, a page sample times the tint for
// textured instances, colours decoded from sRGB to linear as glyphon's
// `Accurate` mode decodes its text colours, and straight-alpha output for
// `ALPHA_BLENDING`. The shadow and gradient fields are reserved and ignored
// (M46 Q13). M57 adds the mesh entry points: coloured triangles in physical
// pixels through the same projection, colours decoded as the primitives'
// are, with no texture and no anti-aliasing.

struct Viewport {
    projection: mat4x4<f32>,
};
@group(0) @binding(0)
var<uniform> viewport: Viewport;

@group(1) @binding(0)
var page_texture: texture_2d<f32>;
@group(1) @binding(1)
var page_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
};

struct InstanceInput {
    @location(1) inst_pos: vec2<f32>,
    @location(2) inst_size: vec2<f32>,
    @location(3) uv_min: vec2<f32>,
    @location(4) uv_size: vec2<f32>,
    @location(5) fill: vec4<f32>,
    @location(6) border: vec4<f32>,
    @location(7) border_width: f32,
    @location(8) corner_radius: f32,
    @location(9) shadow_offset: vec2<f32>,
    @location(10) shadow_blur: f32,
    @location(11) shadow_color: vec4<f32>,
    @location(12) gradient_color: vec4<f32>,
    @location(13) gradient_dir: vec2<f32>,
    @location(14) flags: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    // The fragment's offset from the box centre, in pixels.
    @location(0) local: vec2<f32>,
    @location(1) half_size: vec2<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) fill: vec4<f32>,
    @location(4) border: vec4<f32>,
    // Border width and corner radius, in pixels.
    @location(5) params: vec2<f32>,
    @location(6) @interpolate(flat) flags: u32,
};

const FLAG_TEXTURED: u32 = 1u;

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}

fn decode(color: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(srgb_to_linear(color.rgb), color.a);
}

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;
    let pixel = instance.inst_pos + vertex.position * instance.inst_size;
    out.clip_position = viewport.projection * vec4<f32>(pixel, 0.0, 1.0);
    out.half_size = instance.inst_size * 0.5;
    out.local = (vertex.position - vec2<f32>(0.5)) * instance.inst_size;
    out.uv = instance.uv_min + vertex.position * instance.uv_size;
    out.fill = decode(instance.fill);
    out.border = decode(instance.border);
    out.params = vec2<f32>(instance.border_width, instance.corner_radius);
    out.flags = instance.flags;
    return out;
}

// The signed distance from `p` to a box of `half_size` with corners of
// `radius`, negative inside.
fn rounded_box(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - half_size + vec2<f32>(radius);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let radius = min(in.params.y, min(in.half_size.x, in.half_size.y));
    let outer = rounded_box(in.local, in.half_size, radius);
    // One-pixel analytic anti-aliasing on the outer edge and on the edge
    // between the border and the fill.
    let coverage = clamp(0.5 - outer, 0.0, 1.0);
    let inner_coverage = clamp(0.5 - (outer + in.params.x), 0.0, 1.0);
    let sampled = textureSample(page_texture, page_sampler, in.uv);
    let textured = (in.flags & FLAG_TEXTURED) != 0u;
    let fill = select(in.fill, in.fill * sampled, textured);
    let fill_alpha = fill.a * inner_coverage;
    let border_alpha = in.border.a * (coverage - inner_coverage);
    let alpha = fill_alpha + border_alpha;
    if alpha <= 0.0 {
        return vec4<f32>(0.0);
    }
    let rgb = (fill.rgb * fill_alpha + in.border.rgb * border_alpha) / alpha;
    return vec4<f32>(rgb, alpha);
}

struct MeshInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct MeshOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_mesh(vertex: MeshInput) -> MeshOutput {
    var out: MeshOutput;
    out.clip_position = viewport.projection * vec4<f32>(vertex.position, 0.0, 1.0);
    out.color = decode(vertex.color);
    return out;
}

@fragment
fn fs_mesh(in: MeshOutput) -> @location(0) vec4<f32> {
    return in.color;
}

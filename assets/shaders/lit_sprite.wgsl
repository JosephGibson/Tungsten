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
//   group(2) @binding(0): LightUbo (16 lights + counts + ambient)
//   group(2) @binding(1): the packed lights past 16 (`render.light_cap`)
//   group(2) @binding(2): each screen tile's lights, a bit a packed light
//   group(3) @binding(0): the batch's 256-byte uniform block; its flash
//                         lanes, v0 and f.x, as the stock damage flash reads
//                         them (redo 7.9), zero for a batch without one

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

// `count_pad`: the lights in `lights` (16 at most), the screen tiles a row,
// the words a tile, and the lights packed in all.
struct LightUbo {
    lights: array<GpuLight, 16>,
    count_pad: vec4<u32>,
    ambient: vec4<f32>,
};

@group(2) @binding(0)
var<uniform> lighting: LightUbo;

// Tiled lights (`D-310`): in a frame that packs more lights than the block's
// 16, a compute pass gives each screen tile, 32 pixels a side, a bit a packed
// light that can reach it, `count_pad.z` words a tile, a row after a row, and
// a fragment walks its tile's set bits, lowest first, which is packed order.
// A light whose bit is clear adds exactly nothing at every fragment of the
// tile. `tiled_lights` is the pipeline's constant: on for the pipeline such a
// frame draws with, off for every other frame, whose fragments loop over the
// block as they always did.
const LIGHT_TILE_SHIFT: u32 = 5u;
override tiled_lights: bool = false;

@group(2) @binding(1)
var<storage, read> more_lights: array<GpuLight>;
@group(2) @binding(2)
var<storage, read> light_tiles: array<u32>;

// `UniformOverrideBlock`'s public lanes; the reserved tail is not read.
struct Flash {
    v0: vec4<f32>,
    v1: vec4<f32>,
    v2: vec4<f32>,
    v3: vec4<f32>,
    f: vec4<f32>,
    i: vec4<i32>,
};

@group(3) @binding(0)
var<uniform> flash: Flash;

// Whether a quad's vertices mark the lights that cannot reach it (redo
// 7.15). The pipeline turns it off under MSAA, where a fragment's position
// is its pixel's centre, which can lie outside the quad the pixel's samples
// cover, so that a light just short of the quad could still reach it, and
// for a frame that walks tiles, which reads no mark.
override skip_lights: bool = true;

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
    // -1 for a flipped sprite (a negative UV width), whose normals mirror.
    @location(3) @interpolate(flat) mirror: f32,
    // A bit a packed light that cannot reach the quad (redo 7.15): the point
    // lights whose radius ends short of the quad's box, so that their term
    // is exactly zero at every fragment of it.
    @location(4) @interpolate(flat) skip: u32,
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
    out.mirror = select(1.0, -1.0, instance.inst_uv_size.x < 0.0);
    // The lights that cannot reach the quad. A light's distance from the
    // quad's box, taken in the quad's own axes with its turn undone, bounds
    // its distance from every fragment of it; where that passes the light's
    // radius by more than the two stages can round, the light's attenuation
    // is exactly 0 at each of them, as the fragment's own range test would
    // find. The slack is that rounding's bound: positions round with the
    // coordinates' size, and a `c` and `s` that are off scale the box and
    // the reach by `unit`, which lies within 0.3 % of 1 or nothing is marked.
    var skip = 0u;
    let unit = c * c + s * s;
    if (skip_lights && abs(unit - 1.0) < 0.003) {
        let half = abs(instance.inst_size) * 0.5;
        let count = lighting.count_pad.x;
        for (var i: u32 = 0u; i < count; i = i + 1u) {
            let l = lighting.lights[i];
            if (l.color_intensity.w != 1.0) {
                let radius = max(l.position_radius.z, 0.0001);
                let rel = l.position_radius.xy - centre;
                let unturned = vec2<f32>(rel.x * c + rel.y * s, rel.y * c - rel.x * s);
                let gap = length(max(abs(unturned) - half, vec2<f32>(0.0)));
                let rounding = (abs(centre.x) + abs(centre.y)) * 0.0001;
                let slack = rounding + (half.x + half.y + radius) * 0.004;
                if (gap > radius + slack) {
                    skip = skip | (1u << i);
                }
            }
        }
    }
    out.skip = skip;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSample(t_albedo, s_lit, in.tex_coord);
    let normal = textureSample(t_normal, s_lit, in.tex_coord);
    let glow = textureSample(t_emissive, s_lit, in.tex_coord);
    return lit(in, texel, normal, glow);
}

// A solid batch's fragment under the depth test (redo 7.15): every texel
// the batch samples is clear or opaque and its tint's alpha is 1, so a
// clear texel, which blends to nothing, is dropped before it writes depth
// or is lit, and an opaque one draws what `fs_main` draws.
@fragment
fn fs_solid(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSample(t_albedo, s_lit, in.tex_coord);
    let normal = textureSample(t_normal, s_lit, in.tex_coord);
    let glow = textureSample(t_emissive, s_lit, in.tex_coord);
    if (texel.a < 0.5) {
        discard;
    }
    return lit(in, texel, normal, glow);
}

// The lit colour of a fragment from its three samples.
fn lit(in: VertexOutput, texel: vec4<f32>, normal: vec4<f32>, glow: vec4<f32>) -> vec4<f32> {
    let albedo = texel * in.tint;
    let n_sample = normal.xyz * 2.0 - vec3<f32>(1.0);
    // 2D shader treats `(n.xy, n.z)` as world-axis normals; sprite plane is
    // assumed parallel to the world plane (no per-fragment tangent frame).
    // A flipped sprite's texels run right to left, so its normals' x turns.
    let n = normalize(vec3<f32>(n_sample.x * in.mirror, n_sample.yz));

    var rgb = albedo.rgb * lighting.ambient.rgb;
    if (tiled_lights) {
        // The fragment's tile, and its lights: each light's term as the
        // block's loop below computes it.
        let tile = vec2<u32>(in.clip_position.xy) >> vec2<u32>(LIGHT_TILE_SHIFT);
        let words = lighting.count_pad.z;
        let first = (tile.y * lighting.count_pad.y + tile.x) * words;
        // The words a set bit can be in: those of the lights packed, not of
        // the cap, so a high cap costs a frame nothing it does not light.
        let used = (lighting.count_pad.w + 31u) >> 5u;
        for (var word: u32 = 0u; word < used; word = word + 1u) {
            var bits = light_tiles[first + word];
            while (bits != 0u) {
                let i = word * 32u + countTrailingZeros(bits);
                bits = bits & (bits - 1u);
                var l: GpuLight;
                if (i < 16u) {
                    l = lighting.lights[i];
                } else {
                    l = more_lights[i - 16u];
                }
                if (l.color_intensity.w == 1.0) {
                    // Directional: position_radius.xy = (cos, sin) of the angle.
                    let l_dir = normalize(vec3<f32>(l.position_radius.xy, 1.0));
                    let n_dot_l = max(0.0, dot(n, l_dir));
                    rgb = rgb + albedo.rgb * l.color_intensity.rgb * n_dot_l;
                } else {
                    // Point light, as the block's loop below draws it.
                    let radius = max(l.position_radius.z, 0.0001);
                    let to_light_xy = l.position_radius.xy - in.world_pos;
                    let dist_xy = length(to_light_xy);
                    let attenuation = clamp(1.0 - dist_xy / radius, 0.0, 1.0);
                    // A fragment out of the light's reach takes nothing from it.
                    if (attenuation > 0.0) {
                        let to_light_3d = vec3<f32>(to_light_xy, radius * 0.5);
                        let l_dir = normalize(to_light_3d);
                        let n_dot_l = max(0.0, dot(n, l_dir));
                        rgb = rgb + albedo.rgb * l.color_intensity.rgb * n_dot_l * attenuation * attenuation;
                    }
                }
            }
        }
    } else {
        let count = lighting.count_pad.x;
        for (var i: u32 = 0u; i < count; i = i + 1u) {
            // A light the vertex stage found out of the quad's reach.
            if (((in.skip >> i) & 1u) == 1u) {
                continue;
            }
            let l = lighting.lights[i];
            if (l.color_intensity.w == 1.0) {
                // Directional: position_radius.xy = (cos, sin) of the angle.
                let l_dir = normalize(vec3<f32>(l.position_radius.xy, 1.0));
                let n_dot_l = max(0.0, dot(n, l_dir));
                rgb = rgb + albedo.rgb * l.color_intensity.rgb * n_dot_l;
            } else {
                // Point light: model the source as floating above the sprite
                // plane at z = radius * 0.5 so flat normals (0,0,1) still
                // receive meaningful diffuse. Without the height term,
                // world-space XY distances make `normalize((dx, dy, 1))`
                // collapse to horizontal and N·L hits zero a few pixels away.
                let radius = max(l.position_radius.z, 0.0001);
                let to_light_xy = l.position_radius.xy - in.world_pos;
                let dist_xy = length(to_light_xy);
                let attenuation = clamp(1.0 - dist_xy / radius, 0.0, 1.0);
                // A fragment out of the light's reach takes nothing from it.
                if (attenuation > 0.0) {
                    let to_light_3d = vec3<f32>(to_light_xy, radius * 0.5);
                    let l_dir = normalize(to_light_3d);
                    let n_dot_l = max(0.0, dot(n, l_dir));
                    rgb = rgb + albedo.rgb * l.color_intensity.rgb * n_dot_l * attenuation * attenuation;
                }
            }
        }
    }

    let emissive = glow.rgb;
    rgb = rgb + emissive;
    // The flash mixes the lit colour toward v0's, as the stock flash mixes
    // the sprite's; uniform per batch, so no amount leaves today's pixel.
    let amount = clamp(flash.f.x, 0.0, 1.0) * flash.v0.a;
    if (amount > 0.0) {
        rgb = mix(rgb, flash.v0.rgb, amount);
    }
    return vec4<f32>(rgb, albedo.a);
}

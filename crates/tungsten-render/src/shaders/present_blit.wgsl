// M25 internal present-blit pass: fullscreen-triangle copy from `SceneColor`
// into the swapchain. `textureLoad` with integer coordinates guarantees
// exact texel fetch (no filtering) so the default msaa=1 path stays
// byte-identical to the 0.21 direct-to-swapchain baseline.

@group(0) @binding(0) var t_scene: texture_2d<f32>;

struct VsOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {
    // Single oversized triangle; UVs cover [0,1]x[0,1] in clip.
    // idx -> (-1,-1) (3,-1) (-1,3)
    let x = f32((idx << 1u) & 2u) * 2.0 - 1.0;
    let y = 1.0 - f32(idx & 2u) * 2.0;
    var out: VsOut;
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let dim = vec2<f32>(textureDimensions(t_scene, 0));
    let coord = vec2<i32>(in.uv * dim);
    return textureLoad(t_scene, coord, 0);
}

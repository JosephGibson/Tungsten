// Tiled lights (`D-310`): one invocation a screen tile writes that tile's
// words, a bit a packed light whose light can reach a fragment of the tile.
// The lit shader then walks its fragment's tile's set bits, so a light costs
// a fragment only where it can light it.
//
// A clear bit is a promise: the light's term is exactly zero at every
// fragment of the tile, as the fragment's own range test would find. A
// directional light reaches every tile. A point light reaches a tile unless
// its distance from the tile's box in world space passes its radius by more
// than the slack, which is what the two stages can round apart: the tile's
// corners come through the camera's inverse here, a fragment's position
// through its quad's vertices there.
//
// Bind group:
//   group(0) @binding(0): LightUbo, the first 16 packed lights
//   group(0) @binding(1): the packed lights past 16
//   group(0) @binding(2): the grid
//   group(0) @binding(3): the tiles' words, `words` a tile, row by row

struct GpuLight {
    position_radius: vec4<f32>,
    color_intensity: vec4<f32>,
};

struct LightUbo {
    lights: array<GpuLight, 16>,
    count_pad: vec4<u32>,
    ambient: vec4<f32>,
};

struct Grid {
    // Clip space to world space: the camera's projection, inverted.
    inverse: mat4x4<f32>,
    // The output's width and height in pixels, a tile's side, and 1 where
    // `render.light_tiles` is off: every packed light then reaches every
    // tile, and a fragment's own range test is the only one.
    extent: vec4<f32>,
    // Tiles a row, tiles a column, words a tile, lights packed.
    counts: vec4<u32>,
};

@group(0) @binding(0)
var<uniform> lighting: LightUbo;
@group(0) @binding(1)
var<storage, read> more_lights: array<GpuLight>;
@group(0) @binding(2)
var<uniform> grid: Grid;
@group(0) @binding(3)
var<storage, read_write> tiles: array<u32>;

// The world position of a point of the output, in pixels from its top left.
fn world_of(pixel: vec2<f32>) -> vec2<f32> {
    let ndc = vec2<f32>(
        pixel.x / grid.extent.x * 2.0 - 1.0,
        1.0 - pixel.y / grid.extent.y * 2.0,
    );
    let world = grid.inverse * vec4<f32>(ndc, 0.0, 1.0);
    return world.xy / world.w;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let tiles_x = grid.counts.x;
    let tiles_y = grid.counts.y;
    let words = grid.counts.z;
    let count = grid.counts.w;
    if (id.x >= tiles_x || id.y >= tiles_y) {
        return;
    }
    // The tile's box in world space: the box of its four corners, whatever
    // the camera's turn.
    let side = grid.extent.z;
    let low = vec2<f32>(f32(id.x), f32(id.y)) * side;
    let a = world_of(low);
    let b = world_of(low + vec2<f32>(side, 0.0));
    let c = world_of(low + vec2<f32>(0.0, side));
    let d = world_of(low + vec2<f32>(side, side));
    let lo = min(min(a, b), min(c, d));
    let hi = max(max(a, b), max(c, d));
    let reach = max(abs(lo), abs(hi));
    let rounding = (reach.x + reach.y) * 0.0001;
    let span = (hi.x - lo.x) + (hi.y - lo.y);

    let base = (id.y * tiles_x + id.x) * words;
    for (var word: u32 = 0u; word < words; word = word + 1u) {
        var bits = 0u;
        for (var bit: u32 = 0u; bit < 32u; bit = bit + 1u) {
            let index = word * 32u + bit;
            if (index >= count) {
                break;
            }
            var l: GpuLight;
            if (index < 16u) {
                l = lighting.lights[index];
            } else {
                l = more_lights[index - 16u];
            }
            var reaches = true;
            if (grid.extent.w == 0.0 && l.color_intensity.w != 1.0) {
                let radius = max(l.position_radius.z, 0.0001);
                let at = l.position_radius.xy;
                let gap = distance(at, clamp(at, lo, hi));
                let slack = rounding + (span + radius) * 0.004;
                // Written so that a distance that is no number keeps the
                // light.
                reaches = !(gap > radius + slack);
            }
            if (reaches) {
                bits = bits | (1u << bit);
            }
        }
        tiles[base + word] = bits;
    }
}

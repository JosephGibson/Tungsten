---
status: done
milestone: M15
version-target: 0.12.0
branch: 0.12
depends-on: M14
unblocks: M16, M18, M21, M22, M23, M24
---

# Phase 3 Milestone 15 — Transform + Render Components

## Context

M13 added deferred mutation (`CommandBuffer`), M14 added typed two-window
`EventQueue<T>`. Sprite rendering is still driven by per-example `extract`
closures: each example manually queries its own marker components, resolves
`AssetRegistry` IDs, and builds `SpriteBatch`es. M15 introduces the canonical
gameplay-side components (`Transform`, `Sprite`, `Visibility`, `Tag`) so common
sprite rendering becomes data-driven. Custom extract closures remain supported
for specialized cases (tilemaps, animation-driven frames in `01_platformer`,
the stress-test in `02_sprite_stress`).

Unrelated but adjacent: physics `Position` stays separate per `D-033`. An
explicit opt-in sync system propagates `Position → Transform.position`.

## Goal

Ship `Transform`, `Sprite`, `Visibility`, `Tag` as library components in
`tungsten-core`, extend the sprite render seam to carry per-instance rotation
and color, and add an opt-in default sprite-extract path in the `tungsten`
umbrella crate that renders `Transform + Sprite + Visibility` entities without
any user closure.

## Non-goals

- Sprite atlases or UV rects (`M22`)
- Camera module refactor (`M16`)
- HUD, input mapping, scenes (`M17–M20`)
- Change detection, component hooks, or system scheduling improvements
- Backwards-compat shims for `SpriteInstance` — update all call sites in-tree
- Auto-synchronizing physics `Position ↔ Transform.position` (keep explicit per `D-033`)

## Affected files

| File | Change |
|------|--------|
| `crates/tungsten-core/src/components.rs` | **create** — `Transform`, `Sprite`, `Visibility`, `Tag` + `sync_position_to_transform` free fn + unit tests |
| `crates/tungsten-core/src/lib.rs` | add `pub mod components;` and re-export the four types + sync fn |
| `crates/tungsten-render/src/sprite.rs` | extend `SpriteInstance` with `rotation` + packed RGBA `color`; update vertex attr layout; update upload path and `SpriteBatch` unchanged; add `Default` impl |
| `crates/tungsten-render/src/sprite.wgsl` | read `rotation` (Float32) and `color` (Unorm8x4) instance attrs; apply 2D rotation around quad centre; multiply sampled texel by tint |
| `crates/tungsten/src/sprite_extract.rs` | **create** — `extract_sprites_default(&World) -> Vec<SpriteBatch>` using `Transform + Sprite + Visibility`, sorted by `Sprite.z_order` (stable, ascending) |
| `crates/tungsten/src/lib.rs` | `pub mod sprite_extract;` and `pub use sprite_extract::extract_sprites_default;` |
| `crates/tungsten/src/app.rs` | install `extract_sprites_default` when the user did not call `set_extract_sprites`; field/wiring only — frame loop unchanged |
| `crates/tungsten/src/tilemap_extract.rs` | update `SpriteInstance` construction sites to set `rotation: 0.0`, `color: [255; 4]` |
| `examples/01_platformer/src/main.rs` | update custom-extract `SpriteInstance` sites to set new fields; keep closure-based extract |
| `examples/02_sprite_stress/src/main.rs` | update custom-extract `SpriteInstance` sites; keep closure-based extract |
| `examples/03_component_sprites/` | **create** — new example rendering rotated + scaled + tinted sprites through the default extract path, with an explicit `Visibility` toggle key |
| `Cargo.toml` (workspace) | add `examples/03_component_sprites` to `members` |
| `crates/tungsten-core/benches/ecs_bench.rs` | add `sprite_components_query3_2k` benchmark (`query3::<Transform, Sprite, Visibility>` over 2k entities × 5 archetypes) |
| `crates/tungsten-render/benches/render_bench.rs` | update `sprite_extract_batch_build_2k` to use the new `SpriteInstance` fields |
| `docs/LLM_INDEX.md` | add row: `Render components` → `crates/tungsten-core/src/components.rs` + `crates/tungsten/src/sprite_extract.rs` |
| `DESIGN.md` | short subsection describing the Transform/Sprite/Visibility/Tag contract + default extract seam |
| `DECISIONS.md` | add `D-041` — justify `SpriteInstance` layout change, explicit `Visibility`, explicit `Position → Transform` sync |
| `docs/plans/Phase3.md` | flip M15 `status` marker to `in progress` then `complete` on close |

## Pre-execution reads

Open exactly these files before writing any code on this branch:

1. `crates/tungsten-core/src/ecs/world.rs` — confirm `query3` signature and archetype iteration order
2. `crates/tungsten-core/src/ecs/event_queue.rs` — template style for new core module with tests
3. `crates/tungsten-core/src/physics/components.rs` — `Position` + `D-033` wording in the module doc
4. `crates/tungsten-render/src/sprite.rs` lines 7–50, 380–440 — `SpriteInstance` layout + `draw()` batch loop
5. `crates/tungsten-render/src/sprite.wgsl` — vertex/instance bindings
6. `crates/tungsten/src/app.rs` lines 25–160 (extract-fn typedefs + constructor), lines 556–575 (extract stage) — default-install seam
7. `crates/tungsten/src/tilemap_extract.rs` — every `SpriteInstance { … }` literal to update
8. `examples/01_platformer/src/main.rs` lines 420–477 — platformer sprite-extract closure
9. `examples/02_sprite_stress/src/main.rs` lines 105–125 — stress-test sprite-extract closure
10. `crates/tungsten-core/benches/ecs_bench.rs` — template for registering a new `query3` micro-bench

Do not open `docs/plans/archive/`.

---

## Implementation steps

Execute in this order. Each phase leaves the tree compiling.

### Phase 0 — DECISIONS.md

Append a single new entry:

```
## D-041 — M15 Transform + render components

Four coupled choices:
 (1) new engine-level components live in `tungsten-core::components`:
     `Transform { position: Vec2, rotation: f32, scale: Vec2 }`,
     `Sprite { asset_id: String, color: [u8; 4], z_order: i32 }`,
     `Visibility { visible: bool }`, `Tag { name: String }`;
 (2) physics `Position` stays separate (per D-033). `Position -> Transform.position`
     is an opt-in free-fn system `sync_position_to_transform`; examples register it
     between `physics_step` and any extract stage that needs authoritative visuals;
 (3) `SpriteInstance` grows by two fields (`rotation: f32`, `color: [u8; 4]`) so the
     component data can reach the GPU; all in-tree call sites migrate in the same
     commit — no backwards-compat shim;
 (4) if the App has no custom sprite-extract, `extract_sprites_default` runs over
     `Transform + Sprite + Visibility`. `Visibility` is required — entities with
     `Transform + Sprite` but no `Visibility` are never emitted by the default
     path. No implicit fallback.
```

### Phase 1 — Core components (`tungsten-core`)

**Task 1.1** — Create `crates/tungsten-core/src/components.rs`.

Public surface:

```rust
use glam::Vec2;
use crate::ecs::World;
use crate::physics::Position;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub position: Vec2,
    pub rotation: f32, // radians, CCW positive
    pub scale: Vec2,
}

impl Transform {
    pub fn from_position(position: Vec2) -> Self { /* rotation 0, scale (1,1) */ }
}

impl Default for Transform { /* position ZERO, rotation 0, scale Vec2::ONE */ }

#[derive(Debug, Clone)]
pub struct Sprite {
    pub asset_id: String,
    pub color: [u8; 4],   // RGBA tint, 255s = no tint
    pub z_order: i32,     // stable ascending sort key in the default extract
}

impl Sprite {
    pub fn new(asset_id: impl Into<String>) -> Self { /* color [255;4], z_order 0 */ }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Visibility { pub visible: bool }

impl Default for Visibility { fn default() -> Self { Self { visible: true } } }

#[derive(Debug, Clone)]
pub struct Tag { pub name: String }

impl Tag {
    pub fn new(name: impl Into<String>) -> Self { Self { name: name.into() } }
}

/// Explicit opt-in system: copies `Position.0` into `Transform.position` for
/// every entity that carries both. Does not touch rotation or scale. Register
/// after `physics_step` so the authoritative post-physics position reaches the
/// extract stage. Physics `Position` remains the physics source of truth
/// (D-033); no reverse sync.
pub fn sync_position_to_transform(world: &mut World) {
    // collect entity ids from query2::<Position, Transform>, then iterate with
    // world.get::<Position>() / world.get_mut::<Transform>() per entity to
    // respect the borrow rules in crate::ecs::world.
}
```

**Task 1.2** — Edit `crates/tungsten-core/src/lib.rs`:

- Add `pub mod components;`.
- Re-export: `pub use components::{sync_position_to_transform, Sprite, Tag, Transform, Visibility};`.

**Task 1.3** — Unit tests inside `components.rs`:

- `transform_default_is_identity` — position `Vec2::ZERO`, rotation `0.0`, scale `Vec2::ONE`.
- `visibility_default_is_visible`.
- `sprite_new_defaults_color_and_z_order` — color `[255; 4]`, z `0`.
- `sync_position_to_transform_copies_position` — spawn entity with `Position(Vec2::new(3.0, 4.0))` + `Transform::default()`, run system, assert `transform.position == (3.0, 4.0)` and rotation/scale unchanged.
- `sync_position_to_transform_skips_entities_missing_either` — two entities: one `Position` only, one `Transform` only; no panic, no change.

### Phase 2 — Render seam: instance layout + shader

**Task 2.1** — Extend `SpriteInstance` in `crates/tungsten-render/src/sprite.rs`:

```rust
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SpriteInstance {
    pub position: [f32; 2],   // 0..8
    pub size:     [f32; 2],   // 8..16
    pub rotation: f32,        // 16..20  radians, CCW positive, around quad centre
    pub color:    [u8; 4],    // 20..24  RGBA8 tint, Unorm8x4 on the GPU
}
```

Update `ATTRIBS`:

```rust
const ATTRIBS: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
    2 => Float32x2,
    3 => Float32x2,
    4 => Float32,
    5 => Unorm8x4,
];
```

Add `impl Default for SpriteInstance` returning a zero-position, zero-size,
zero-rotation, `[255; 4]` color instance so call sites that only care about a
subset can `..Default::default()`.

No change to `SpriteBatch` (texture + filter + instances). No change to
`upload_texture`, sampler layout, or bind groups.

**Task 2.2** — Update `crates/tungsten-render/src/sprite.wgsl`:

```wgsl
struct InstanceInput {
    @location(2) inst_pos:  vec2<f32>,
    @location(3) inst_size: vec2<f32>,
    @location(4) inst_rot:  f32,
    @location(5) inst_tint: vec4<f32>,  // already in 0..1 via Unorm8x4
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) tint:      vec4<f32>,
};

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    let local     = vertex.position - vec2<f32>(0.5, 0.5);         // centre-origin
    let scaled    = local * instance.inst_size;
    let c         = cos(instance.inst_rot);
    let s         = sin(instance.inst_rot);
    let rotated   = vec2<f32>(scaled.x * c - scaled.y * s,
                              scaled.x * s + scaled.y * c);
    let centre    = instance.inst_pos + instance.inst_size * 0.5;
    let world_pos = centre + rotated;

    var out: VertexOutput;
    out.clip_position = camera.projection * vec4<f32>(world_pos, 0.0, 1.0);
    out.tex_coord     = vertex.uv;
    out.tint          = instance.inst_tint;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_sprite, s_sprite, in.tex_coord) * in.tint;
}
```

**Invariant to verify after shader edit**: when `rotation == 0.0` and
`color == [255;4]`, the pixel output of a given sprite is unchanged from the
pre-M15 path. Validate with the smoke test (Layer 2) against `01_platformer`
and `02_sprite_stress`.

**Why centre-origin rotation**: rotating around the top-left would require
every caller to translate by `size/2` themselves. The existing pre-M15 sprites
were top-left anchored; after this change `position` is still the top-left of
the non-rotated quad AABB (caller-facing math unchanged for `rotation == 0`).

### Phase 3 — Propagate new `SpriteInstance` fields

Every `SpriteInstance { position, size }` literal in the workspace must now set
`rotation` and `color`. Touch every call site listed below in one pass so the
tree builds.

- `crates/tungsten/src/tilemap_extract.rs` — tile instances: `rotation: 0.0, color: [255; 4]`.
- `examples/01_platformer/src/main.rs` — player sprite batch + ball batch: `rotation: 0.0, color: [255; 4]`.
- `examples/02_sprite_stress/src/main.rs` — wave instances: `rotation: 0.0, color: [255; 4]`.
- `crates/tungsten-render/benches/render_bench.rs` — `sprite_extract_batch_build_2k` instance literal.

No behavior change in these sites — M15 is additive for them.

### Phase 4 — Default sprite extract (`tungsten` umbrella)

**Task 4.1** — Create `crates/tungsten/src/sprite_extract.rs`:

```rust
//! Default sprite extract: Transform + Sprite + Visibility components → SpriteBatch.
//!
//! Installed automatically by `App` when the user did not call
//! `set_extract_sprites`. Entities with `Transform + Sprite` but no
//! `Visibility` are not emitted — `Visibility` is required (M15 / D-041).

use std::collections::HashMap;

use tungsten_core::{AssetRegistry, Sprite, Transform, Visibility, World};
use tungsten_render::{SpriteBatch, SpriteInstance};

pub fn extract_sprites_default(world: &World) -> Vec<SpriteBatch> {
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return Vec::new();
    };

    // Phase A: collect references, filtered by visibility and asset resolution,
    // sorted stably by z_order ascending.
    let mut entries: Vec<(&Transform, &Sprite, &tungsten_core::SpriteAsset)> = world
        .query3::<Transform, Sprite, Visibility>()
        .filter_map(|(_e, t, s, v)| {
            if !v.visible { return None; }
            let asset = assets.get_sprite(&s.asset_id)?;
            Some((t, s, asset))
        })
        .collect();
    entries.sort_by_key(|(_, s, _)| s.z_order);

    // Phase B: batch by (texture, filter), preserving the sorted order.
    // Distinct z_order groups MUST NOT merge across a lower-z entity; this
    // implementation groups by batch key only within a z_order run.
    let mut out: Vec<SpriteBatch> = Vec::new();
    let mut current_z: Option<i32> = None;
    let mut per_key: HashMap<(u32, tungsten_core::FilterMode), usize> = HashMap::new();
    for (t, s, asset) in entries {
        if current_z != Some(s.z_order) {
            per_key.clear();
            current_z = Some(s.z_order);
        }
        let key = (asset.texture.0, asset.filter);
        let idx = match per_key.get(&key) {
            Some(&i) => i,
            None => {
                per_key.insert(key, out.len());
                out.push(SpriteBatch {
                    texture: asset.texture,
                    filter: asset.filter,
                    instances: Vec::new(),
                });
                out.len() - 1
            }
        };
        let width_world  = asset.width  as f32 * t.scale.x;
        let height_world = asset.height as f32 * t.scale.y;
        out[idx].instances.push(SpriteInstance {
            position: [t.position.x, t.position.y],
            size:     [width_world, height_world],
            rotation: t.rotation,
            color:    s.color,
        });
    }
    out
}
```

Notes:

- The batch key is `(texture, filter)`. Atlases (M22) will later collapse these keys.
- Resetting `per_key` on `z_order` change preserves painter-ordering even when two entities at different z share a texture.
- `position` is the top-left of the non-rotated quad AABB, matching the pre-M15 semantic and the WGSL centre-origin rotation derivation in Phase 2.

**Task 4.2** — Edit `crates/tungsten/src/lib.rs`:

```rust
pub mod sprite_extract;
pub use sprite_extract::extract_sprites_default;
```

**Task 4.3** — Edit `crates/tungsten/src/app.rs`:

- In `fn run(self)` (before building the event loop) or equivalently in
  `fn resumed()` after the renderer is ready but before the first frame:
  if `self.extract_sprites.is_none()`, set it to
  `Some(Box::new(sprite_extract::extract_sprites_default))`.
- Prefer installing in `run()` so the default survives in `#[cfg(test)]` paths
  that drive frames without a real window.
- Do not touch the extract-stage frame-loop block. No new stage is introduced.

**Task 4.4** — App-level unit tests in `app.rs`:

- `default_sprite_extract_installed_when_not_set` — construct `App::new(Config::default())`, call the hidden install hook (extract `run`'s install into a private fn to make it testable), assert `self.extract_sprites.is_some()`.
- `user_extract_sprites_overrides_default` — call `set_extract_sprites(|_| vec![])`, then the install hook, assert the user closure is preserved (compare via producing a batch count from a dummy closure that returns a sentinel length).

### Phase 5 — Example `03_component_sprites`

Create a new example that demonstrates:

- Omitting `set_extract_sprites` entirely — the engine installs `extract_sprites_default`.
- Rotation: one entity spins at `1.0 rad/s` via `Transform.rotation += dt`.
- Scale: one entity pulses (`scale = Vec2::splat(1.0 + 0.25 * sin(t))`).
- Color: one entity fades through hue via `Sprite.color`.
- Z-order: three stacked entities with `z_order ∈ {-1, 0, 1}`.
- Visibility toggle: pressing `V` (`KeyCode::KeyV`) flips `Visibility.visible` on the tagged entity; the sprite disappears without despawn.
- `Tag`: label one entity with `Tag::new("player")` and log its count on startup to prove the component compiles and queries.

File layout mirrors `examples/02_sprite_stress`:

```
examples/03_component_sprites/
├── Cargo.toml
├── assets/
│   └── manifest.json      # references a single placeholder sprite
└── src/
    └── main.rs
```

Asset strategy: reuse `assets/manifest.json` root fonts are unused; declare
one local sprite (`ex03_quad`) as a 16×16 solid white PNG inside
`examples/03_component_sprites/assets/sprites/` so the tint demonstration is
visible. No tilemap, no animation, no audio, no hot reload.

Register the example in the workspace root `Cargo.toml` `members` list.

Wire the systems in this order in `fn main()`:

```text
app.add_system_named("spin_system", spin_system);
app.add_system_named("pulse_system", pulse_system);
app.add_system_named("tint_system", tint_system);
app.add_system_named("visibility_toggle_system", visibility_toggle_system);
// No set_extract_sprites → default extract path.
```

Smoke test (layer 2) must pass with `TUNGSTEN_SMOKE_FRAMES=3`.

### Phase 6 — Benchmarks

**Task 6.1** — `crates/tungsten-core/benches/ecs_bench.rs`:

Add `sprite_components_query3_2k` — build a `World` with 2 000 entities
spread across five archetypes (`{Transform, Sprite, Visibility}`,
`{Transform, Sprite, Visibility, Tag}`, `{Transform, Sprite}` (excluded),
`{Transform, Visibility}` (excluded), `{Sprite, Visibility}` (excluded)).
Bench body iterates `world.query3::<Transform, Sprite, Visibility>()` and
sums `z_order` into a `black_box`. Goal: verify steady-state query cost
stays within `<= 10%` of the existing `query2_fragmented_5arch_10k` scaled
baseline. Record the absolute number on the capture machine in M15 close-out
notes.

**Task 6.2** — `crates/tungsten-render/benches/render_bench.rs`:

Update the existing `sprite_extract_batch_build_2k` `SpriteInstance` literal
to include `rotation: 0.0` and `color: [255; 4]`. Bench shape unchanged —
this is the regression gate on the render-side POD cost with the larger
instance struct.

### Phase 7 — Docs

**Task 7.1** — `docs/LLM_INDEX.md`:

Insert row after the `Event queue` row:

```
| Render components (`Transform`, `Sprite`, `Visibility`, `Tag`) + default sprite extract | [`crates/tungsten-core/src/components.rs`](../crates/tungsten-core/src/components.rs), [`crates/tungsten/src/sprite_extract.rs`](../crates/tungsten/src/sprite_extract.rs) |
```

**Task 7.2** — `DESIGN.md`: short subsection under the existing ECS/render
discussion that states:

- The four component types and their fields.
- The `Position` vs `Transform.position` split (cite `D-033`).
- The default extract path installs only when no custom extract is set.
- `Visibility` is required — no fallback.
- `SpriteInstance` per-instance rotation + tint is the GPU contract for all
  sprites, not just component-driven ones.

**Task 7.3** — `docs/plans/Phase3.md`: update the M15 section footer to
`Status: in progress` while work is underway, then `Status: complete
(v0.12.0, <date>)` with a link to the archived detailed plan when closing.

**Task 7.4** — `CHANGELOG.md`: append a `0.12.0 - M15 Transform + Render
Components` entry enumerating the components, the `SpriteInstance` layout
change, the default extract path, and the new example.

### Phase 8 — Validate and close

Run in this order:

1. `cargo fmt --all`
2. `cargo build --workspace`
3. `cargo test --workspace` — includes new unit tests in `components.rs`,
   `app.rs`, and existing regression coverage.
4. `cargo test --workspace -- --test manifests` (Layer 1) — new local manifest
   in `examples/03_component_sprites` must load.
5. `TUNGSTEN_SMOKE_FRAMES=3 cargo run -p example-03-component-sprites` then
   `./scripts/smoke-examples.sh` (Layer 2) — all three examples must exit 0.
6. Benches: `cargo bench -p tungsten-core -- sprite_components_query3_2k` and
   `cargo bench -p tungsten-render -- sprite_extract_batch_build_2k`. Compare
   against the previous run under the same machine/profile; record deltas.
7. If any bench breaks `<= 10%` regression on a steady-state runtime bench,
   either fix before close or add a `D-0xx` entry with rationale.
8. Archive this plan: `git mv docs/plans/Phase3-Milestone15-plan.md
   docs/plans/archive/Phase3-Milestone15-plan.md` and flip `status: done`.
9. Bump workspace version `0.11.0 → 0.12.0` in the root `Cargo.toml`
   `[workspace.package]` block. Update `AGENTS.md` status line.

## Done-when checks

Pulled verbatim from `Phase3.md` M15 DOD, each mapped to a falsifiable check:

- [ ] **A new example renders rotated/scaled sprites with components only.**
  `examples/03_component_sprites` runs without `set_extract_sprites`; visual
  confirmation the spinning sprite rotates and the pulsing sprite grows/shrinks
  every frame.
- [ ] **Existing examples with custom extract still work unchanged.**
  `./scripts/smoke-examples.sh` reports PASS for `01_platformer` and
  `02_sprite_stress`; no visual regression at `rotation == 0`, `color == [255;4]`.
- [ ] **The default extract path enforces `Visibility` with no implicit fallback.**
  A unit test in `sprite_extract.rs` spawns an entity with `Transform + Sprite`
  only (no `Visibility`), runs `extract_sprites_default`, and asserts zero
  instances emitted. A second test with `Visibility { visible: false }` also
  emits zero instances.
- [ ] **At least one example validates the explicit `Visibility` migration path.**
  `03_component_sprites` toggles `Visibility.visible` on a tagged entity with
  `V`; the sprite visibly disappears/reappears in smoke manual run.
- [ ] **Physics `Position` and `Transform.position` remain independent
  (`D-033`).**
  Unit test: mutate `Position`, run frame with `sync_position_to_transform`
  registered, assert `Transform.position == Position.0`; then mutate `Transform`
  directly and assert `Position` unchanged.
- [ ] **All validation commands in Phase 8 pass on branch `0.12`.**

## Risks and open questions

- **SpriteInstance size growth.** Going from 16 bytes → 24 bytes increases GPU
  upload bandwidth by 50 %. At 2 000 sprites that is 48 KiB/frame vs. 32 KiB;
  immaterial but flag in the bench summary so M22 atlas work starts from a
  recorded baseline.
- **`Sprite.asset_id: String` hot path.** Every frame resolves by `HashMap`
  lookup via `AssetRegistry::get_sprite`. For 2 000 sprites this is fine; if it
  bites at 20 000, consider caching `TextureHandle` into the `Sprite` at spawn
  time or migrating to interned IDs. Out of scope for M15.
- **`Visibility` required with no fallback.** Friendly-mode bootstrapping
  temptation: an `impl Default for Sprite` that auto-inserts `Visibility` is
  rejected on purpose — the rule is explicit per `D-041`. Document clearly in
  the `components.rs` module header.
- **Default extract ownership of the `extract_sprites` slot.** If a user calls
  `set_extract_sprites(None-equivalent)` after `run()` starts, the default
  will not re-install. Acceptable — `set_extract_sprites` is a startup-time
  API, not runtime.
- **Centre-origin rotation vs. existing top-left anchor.** Double-check during
  implementation that the `rotation == 0` WGSL path reduces to exactly the
  pre-M15 `world_pos = instance.inst_pos + vertex.position * instance.inst_size`
  expression; the math above does (`local - 0.5` times `size` plus centre ≡
  `vertex.position * size + pos`). Verify with a pixel diff on `01_platformer`
  before closing.
- **Z-order stability.** `Vec::sort_by_key` is stable; relied on so entities
  with equal `z_order` keep registration order (matches existing archetype
  traversal contract). Note in the module doc so future refactors preserve it.

## Sources

None — all references are internal repo files cited above. Rotation/tint in a
vertex-pulled instance buffer is standard wgpu/WGSL practice; no external
citation needed.

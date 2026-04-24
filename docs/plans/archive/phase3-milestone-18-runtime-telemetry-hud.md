---
status: done
milestone: M18
branch: 0.15
version-target: 0.15.0
depends-on: M16 complete, M17 complete
unblocks: faster iteration across M19-M24
---

# Phase 3 Milestone 18 — Runtime Telemetry HUD

## Goal

- Add a lightweight in-game developer HUD rendered through the existing `glyphon` text pipeline.
- Introduce one `DebugHud` world resource that aggregates telemetry rows and owns an ordered list of built-in + user-registered row providers.
- Toggle visibility with `F4` through one engine-owned system; default state is off.
- Feed built-in rows from live sources: `FrameTimings`, `DisplayTelemetry`, `CameraState` / `CameraController`, `World::entity_count`, `Tag`-based player lookup.
- Ship one platformer demo that shows FPS, camera mode/position/zoom, display mode, player position/speed, entity + sprite counts, and top-N slowest systems when toggled on.

## Non-goals

- Debug draw / geometric overlays (`M21`).
- Interactive entity inspector, `Inspectable` trait (`M21`).
- Screenshot capture / image-diff (`M21`).
- Scene/state system plumbing (`M20`). The `state` row reads an optional `HudActiveState` placeholder resource; M20 populates it later.
- Input mapping rework (`M19`); `F4` stays a hardcoded key like `F9` / `F11`.
- New text pipeline, font, or SDF rendering.
- Persisting HUD layout, opacity, or toggle state to disk or `tungsten.json`.
- Multi-column / docking / draggable HUD.
- Right-aligned corners with true pixel-exact glyph measurement; right-side corner support is a monospace-width heuristic (documented).
- GPU timestamp visualisation; `GpuFrameTimings.frame_gpu_ms` appears as a number, not a graph.
- New runtime dependency; no `egui`/`imgui`/`hud-rs`.
- Release bookkeeping (`CHANGELOG.md`, version bump, plan archival) before the milestone lands.

## Guardrails

- `D-007` / `D-016` / `D-018`: `tungsten-core` stays free of `wgpu`/`winit` types; HUD rendering lives in `tungsten`.
- `D-038`: `FrameTimings` remains the single CPU-timing resource; HUD reads it and does not add a parallel timer.
- `D-042`: HUD player lookup uses `Tag { name: "player" }` on an entity carrying `Transform` (and optionally `Velocity`).
- `D-043`: HUD does not call `winit`/`wgpu`; all display values come from `DisplayTelemetry`.
- Frame order invariant: `systems -> flush commands -> flush events -> hot-reload -> extract -> render`. HUD toggle runs inside the systems stage (first system). HUD `TextSection` emission runs inside the extract stage (after user text extract).
- HUD cost when `enabled = false` must be zero beyond one `InputState::just_pressed` check per frame.
- HUD cost when `enabled = true` stays negligible at Phase 3 scale; recorded via `perf-capture.sh` comparison.
- No new runtime dependency.
- Default state is off in every in-tree example. Examples opt in with `world.get_resource_mut::<DebugHud>().unwrap().enabled = true;` or leave it off.
- Do not read `docs/plans/archive/`.

## Read before coding

1. `AGENTS.md`
2. `docs/LLM_INDEX.md`
3. `crates/tungsten/src/app.rs` (frame loop, system registration, `FrameTimings` population, extract_text slot)
4. `crates/tungsten/src/telemetry.rs` (`FrameTimings`, `DisplayTelemetry`)
5. `crates/tungsten-core/src/camera.rs` (`CameraState`, `CameraController`, `CameraMode`)
6. `crates/tungsten-core/src/components.rs` (`Transform`, `Tag`)
7. `crates/tungsten-core/src/physics/components.rs` (`Position`, `Velocity`)
8. `crates/tungsten-core/src/ecs/world.rs`, `ecs/entity.rs`, `ecs/storage.rs` (no `entity_count` today; live count = `meta.len() - free.len()`)
9. `crates/tungsten-core/src/input.rs`, `crates/tungsten/src/input_bridge.rs` (`F9` / `F11` precedent)
10. `crates/tungsten-render/src/text.rs` (`TextSection` fields; unknown `font_id` is already a no-op at draw time)
11. `examples/01_platformer/src/{extract,setup,systems,state}.rs`
12. `docs/plans/archive/phase3-milestone-17-display-state-config.md` (format precedent)
13. `DECISIONS.md` with `rg "D-007|D-016|D-018|D-038|D-042|D-043" DECISIONS.md`

## Files to touch

| File | Change |
|------|--------|
| `crates/tungsten-core/src/input.rs` | add `KeyCode::F4` |
| `crates/tungsten/src/input_bridge.rs` | map `WinitKeyCode::F4 -> KeyCode::F4` |
| `crates/tungsten-core/src/ecs/entity.rs` | add `pub fn live_count(&self) -> u32` on `Entities` (`meta.len() - free.len()`); unit test |
| `crates/tungsten-core/src/ecs/world.rs` | add `pub fn entity_count(&self) -> u32` delegating to `Entities::live_count`; unit test |
| `crates/tungsten/src/telemetry.rs` | add `pub struct RenderCounts { entities: u32, sprite_instances: u32 }` with `Default` |
| `crates/tungsten/src/debug_hud.rs` | **new** — `DebugHud`, `HudCorner`, `HudRow`, `HudRowProvider`, `HudActiveState`, `hud_toggle_system`, private `compose_hud_text_sections`, built-in providers, unit tests |
| `crates/tungsten/src/lib.rs` | `pub mod debug_hud;` + re-exports (`DebugHud`, `HudCorner`, `HudRow`, `HudActiveState`, `hud_toggle_system`, `RenderCounts`) |
| `crates/tungsten/src/app.rs` | insert `DebugHud` + `RenderCounts` resources in `App::new`; register `hud_toggle_system` as the first system in `App::new`; populate `RenderCounts` after extract; extend the per-frame `text: Vec<TextSection>` with HUD sections before render |
| `examples/01_platformer/src/setup.rs` | `world.insert(player, Tag::new("player"))` |
| `examples/01_platformer/src/main.rs` | add `F4              toggle developer HUD` to the `Controls` block |
| `docs/LLM_INDEX.md` | add subsystem row for the runtime HUD and a task row for HUD fixes |
| `docs/DECISION_INDEX.md` | add `D-044` one-liner |
| `docs/plans/Phase3.md` | flip M18 status on start and land; link to archived plan |
| `DECISIONS.md` | add `D-044` full entry |
| `tungsten.json` | no change |

## Public surface after M18

- `tungsten::debug_hud::{DebugHud, HudCorner, HudRow, HudActiveState, hud_toggle_system}`
- `tungsten::{DebugHud, HudCorner, HudRow, HudActiveState, RenderCounts}` (re-exports)
- `tungsten_core::input::KeyCode::F4`

Keep private:

- All built-in provider functions (`fps_provider`, `camera_provider`, ...).
- `compose_hud_text_sections` (called only from `app.rs`; exported as `pub(crate)`).
- `DebugHud.fps_ewma`, `frame_ms_ewma`, `ewma_alpha`, `built_in`, `custom` fields.

## Ordered steps

### 1. Input plumbing for `F4`

- In `crates/tungsten-core/src/input.rs`, add `F4` to `KeyCode` adjacent to `F9` / `F11`.
- In `crates/tungsten/src/input_bridge.rs`, add arm `WinitKeyCode::F4 => KeyCode::F4`.
- Exit: `cargo test -p tungsten-core` and `cargo build -p tungsten` pass.

### 2. `Entities::live_count` and `World::entity_count`

- `crates/tungsten-core/src/ecs/entity.rs`:
  ```rust
  impl Entities {
      pub fn live_count(&self) -> u32 {
          (self.meta.len() - self.free.len()) as u32
      }
  }
  ```
  Unit test: spawn N, despawn K, assert `live_count() == N - K`.
- `crates/tungsten-core/src/ecs/world.rs`:
  ```rust
  pub fn entity_count(&self) -> u32 {
      self.archetypes.entities.live_count()
  }
  ```
  Unit test: spawn/despawn through `World` API; assert counts.
- Exit: O(1) accessor; `cargo test -p tungsten-core` passes.

### 3. `RenderCounts` telemetry resource

- Add to `crates/tungsten/src/telemetry.rs`:
  ```rust
  #[derive(Debug, Clone, Copy, Default)]
  pub struct RenderCounts {
      pub entities: u32,
      pub sprite_instances: u32,
  }
  ```
- Re-export from `crates/tungsten/src/lib.rs`.
- In `App::new`, `world.insert_resource(RenderCounts::default())`.
- In `app.rs` `RedrawRequested`, after the extract stage:
  ```rust
  let sprite_instances: u32 = sprites.iter().map(|b| b.instances.len() as u32).sum();
  if let Some(rc) = self.world.get_resource_mut::<RenderCounts>() {
      rc.entities = self.world_entity_count_cached; // computed pre-borrow
      rc.sprite_instances = sprite_instances;
  }
  ```
  (compute `world.entity_count()` into a local before the mutable resource borrow to avoid conflicting borrows).
- Exit: `RenderCounts` readable from any system; `cargo test --workspace` passes.

### 4. `DebugHud` data model

Create `crates/tungsten/src/debug_hud.rs`:

```rust
use tungsten_core::World;
use tungsten_render::TextSection;

pub enum HudCorner { TopLeft, TopRight, BottomLeft, BottomRight }

pub struct HudRow {
    pub label: &'static str,
    pub value: String,
}

pub type HudRowProvider = Box<dyn Fn(&World) -> Vec<HudRow> + 'static>;

/// Optional placeholder populated by M20's scene/state stack. M18 only reads
/// this resource; when absent, the state row is omitted.
#[derive(Debug, Clone, Default)]
pub struct HudActiveState(pub String);

pub struct DebugHud {
    pub enabled: bool,
    pub corner: HudCorner,
    pub font_id: String,
    pub font_size: f32,
    pub line_height: f32,
    pub color: [u8; 4],
    pub padding_px: f32,
    pub top_n_systems: usize,
    fps_ewma: f32,
    frame_ms_ewma: f32,
    ewma_alpha: f32,
    built_in: Vec<HudRowProvider>,
    custom: Vec<HudRowProvider>,
}
```

Defaults (`DebugHud::new`):

- `enabled = false`
- `corner = HudCorner::TopLeft`
- `font_id = "mono"`
- `font_size = 16.0`, `line_height = 20.0`
- `color = [230, 230, 230, 230]`
- `padding_px = 8.0`
- `top_n_systems = 3`
- `ewma_alpha = 0.1`
- `fps_ewma = 0.0`, `frame_ms_ewma = 0.0`

Rationale for `TopLeft` default: avoids the right-corner width-measurement problem. Right-side corners ship but use a monospace-width heuristic documented on `HudCorner`.

Public API:

- `DebugHud::new() -> Self`
- `pub fn toggle(&mut self) { self.enabled = !self.enabled; }`
- `pub fn add_row<F>(&mut self, provider: F) where F: Fn(&World) -> Vec<HudRow> + 'static`
  - pushes into `custom`.

Row-provider contract:

- Returns `Vec<HudRow>`; empty `Vec` = skip.
- Must not panic when backing resources are absent.

### 5. Built-in providers

Registered inside `DebugHud::new` in this fixed order; all functions are private `fn(&World) -> Vec<HudRow>`:

1. `fps_provider` — reads EWMA fields (via a light trick: the compose helper writes EWMA into a scratch resource `HudSmoothed(f32, f32)` inserted on-demand; see step 6 for the mechanism). Row: `"fps"` -> `"{fps:>4.0}  {ms:>5.2} ms"`.
2. `camera_provider` — reads `CameraState` + `CameraController`; row `"cam"` -> `"{mode} pos=({x:.0},{y:.0}) zoom={z:.2}"`. Omit if resources missing.
3. `display_provider` — reads `DisplayTelemetry`; row `"display"` -> `"{w}x{h} {mode} vsync={bool}"`.
4. `player_provider` — iterates `query2::<Tag, Transform>`; takes first entity with `tag.name == "player"`; reads optional `Velocity`; row `"player"` -> `"pos=({x:.0},{y:.0}) speed={mag:.0}"`. Empty `Vec` when no tag match.
5. `state_provider` — reads optional `HudActiveState`; row `"state"` -> `"{name}"`. Empty when `HudActiveState.0.is_empty()` or resource absent.
6. `counts_provider` — reads `RenderCounts`; row `"counts"` -> `"ents={e} sprites={s}"`.
7. `systems_top_n_provider` — reads `FrameTimings::system_timings`; clones, partial-sorts descending by ms, takes `min(top_n_systems, len)`; emits one row per entry: label `"sys"`, value `"{i}:{name} {ms:.2}ms"`. Multi-row output is why providers return `Vec`.

Tests in the same file (do not make private items pub):

- `default_hud_is_disabled_and_empty_world_yields_no_rows`
- `toggle_flips_enabled`
- `player_provider_requires_tag_and_transform` (spawn with/without tag; verify presence/absence)
- `systems_top_n_respects_cap_and_sorts_desc`
- `custom_row_appears_after_built_in` (add_row with a known label; assert order in compose output)
- `compose_is_empty_when_disabled`
- `ewma_converges` — feed 200 frames of constant 16.67 ms; assert `|frame_ms_ewma - 16.67| < 0.05`.

Exit: `cargo test -p tungsten` passes.

### 6. Compose helper and EWMA update

Signature:

```rust
pub(crate) fn compose_hud_text_sections(
    hud: &mut DebugHud,
    world: &World,
    viewport: (u32, u32),
    frame_ms: f32,
) -> Vec<TextSection>
```

Behaviour:

- If `!hud.enabled`, return `Vec::new()`.
- Update smoothing:
  ```rust
  hud.frame_ms_ewma = (1.0 - hud.ewma_alpha) * hud.frame_ms_ewma + hud.ewma_alpha * frame_ms;
  hud.fps_ewma = if hud.frame_ms_ewma > 0.0 { 1000.0 / hud.frame_ms_ewma } else { 0.0 };
  ```
- To let `fps_provider` read the smoothed values without a `&DebugHud` borrow, pass them as a short-lived shadow resource: before iterating providers, `world` is immutable, so use a plain local `SmoothedFps { fps, frame_ms }` passed via closure capture — refactor `fps_provider` to be a method on `DebugHud` (`fn fps_row(&self) -> Vec<HudRow>`) and call it directly from the compose helper instead of pushing it through `HudRowProvider`. Other built-ins stay as `fn(&World) -> Vec<HudRow>` entries in `built_in`. This removes the shadow-resource hack entirely.
- Collect rows: `hud.fps_row()` first, then iterate `built_in` (which holds providers 2-7), then `custom`.
- Build a single `String` joined by `'\n'`, one line per row, formatted `"{label:>7}  {value}"`.
- Compute origin:
  - `TopLeft`: `(padding, padding)`
  - `BottomLeft`: `(padding, viewport.1 - line_count * line_height - padding)`
  - `TopRight` / `BottomRight`: use monospace heuristic `char_w = font_size * 0.55`; `max_chars = rows.iter().map(|r| r.rendered.len()).max()`; `text_w = char_w * max_chars as f32`; origin `x = viewport.0 - text_w - padding`.
- Return one `TextSection` with:
  - `content: joined_string`
  - `font_id: hud.font_id.clone()`
  - `font_size`, `line_height`, `color` from hud
  - `position: [x, y]`
  - `bounds: None`

Notes:

- Unknown `font_id` is already silent in `TextPipeline`; no engine change required.
- Right-side corners are approximate; acceptable for a developer HUD.

Exit: `cargo test -p tungsten` passes (compose tests green).

### 7. Toggle system and `App` wiring

- In `debug_hud.rs`:
  ```rust
  pub fn hud_toggle_system(world: &mut World) {
      let pressed = world
          .get_resource::<InputState>()
          .map(|i| i.just_pressed(KeyCode::F4))
          .unwrap_or(false);
      if pressed {
          if let Some(hud) = world.get_resource_mut::<DebugHud>() {
              hud.toggle();
          }
      }
  }
  ```
- In `App::new`, after the other resource inserts:
  - `world.insert_resource(DebugHud::new());`
  - `world.insert_resource(RenderCounts::default());`
  - Register the toggle as the very first system using a new private helper `add_engine_system` that pushes to `systems` + `system_names` directly (skipping the public `add_system_named` so user-facing naming conventions stay clean). Name: `"__hud_toggle"`. This guarantees the toggle sees input before any user system consumes `just_pressed`.
- In `RedrawRequested`, after the existing extract stage and before the render stage, compose HUD sections:
  ```rust
  let viewport = self.world
      .get_resource::<WindowSize>()
      .map(|w| (w.width, w.height))
      .unwrap_or((0, 0));
  let frame_ms_for_hud = total_ms_from_previous_frame; // see below
  if let Some(hud) = self.world.get_resource_mut::<DebugHud>() {
      let hud_sections = compose_hud_text_sections(hud, &self.world, viewport, frame_ms_for_hud);
      text.extend(hud_sections);
  }
  ```
  Borrow order: take a `&mut DebugHud` and an `&World` simultaneously is not allowed. Resolve by:
  1. `remove_resource::<DebugHud>()` before compose,
  2. call compose with `&mut hud` and `&self.world`,
  3. `insert_resource(hud)` after.
  Document this as the canonical pattern in a code comment.
- Frame-time source for `frame_ms_for_hud`: at the start of `RedrawRequested`, read `self.world.get_resource::<FrameTimings>().map(|ft| ft.total_ms).unwrap_or(0.0)` — this is the previous frame's total. Using last-frame total avoids needing post-render HUD composition (HUD would not be visible this frame if composed after render). One-frame lag is acceptable for a smoothed readout.
- Populate `RenderCounts` after the extract stage:
  ```rust
  let ents = self.world.entity_count();
  let sprite_inst: u32 = sprites.iter().map(|b| b.instances.len() as u32).sum();
  if let Some(rc) = self.world.get_resource_mut::<RenderCounts>() {
      rc.entities = ents;
      rc.sprite_instances = sprite_inst;
  }
  ```
  Must run before the compose call so the `counts` row reflects this frame.
- Exit: `cargo build --workspace` clean; `cargo test --workspace` passes.

### 8. Platformer demo wiring

- `examples/01_platformer/src/setup.rs` — inside `seed_world`, after constructing the player entity and before existing `insert(player, ...)` calls:
  ```rust
  world.insert(player, tungsten::core::Tag::new("player"));
  ```
- `examples/01_platformer/src/main.rs` — extend the header `Controls:` block with `F4              toggle developer HUD`.
- No system registration changes; no env var; HUD stays off by default; `F4` toggles it at runtime.
- Do not remove or modify the existing platformer overlay (`update_text_display` / `extract_text`). The dev HUD coexists with it and may overlap visually when both active — that is acceptable for a dev tool. If overlap is distracting, the user sets `hud.corner = HudCorner::BottomLeft` interactively by editing `setup.rs`.
- Exit: `cargo run -p example-01-platformer` runs unchanged; pressing `F4` shows the HUD; pressing `F4` again hides it; built-in rows populate correctly.

### 9. Perf capture

- Capture two runs on the reference machine:
  ```bash
  ./scripts/perf-capture.sh sprite-stress 300    # HUD off
  # temporarily set `hud.enabled = true` in sprite_stress setup, rebuild
  ./scripts/perf-capture.sh sprite-stress 300    # HUD on
  ```
- Record both output directories plus a short `README.md` under `perf-runs/M18-hud/` summarising:
  - `total_ms` mean, p95, p99 for HUD off vs. HUD on.
  - Qualitative statement: HUD-on does not introduce a material frame-time regression at Phase 3 scale.
  - No fixed percentage threshold — Phase3.md wording is "negligible at Phase 3 scale"; record numbers and let reviewer judge. Flag any regression `> 5%` for explicit rationale in `DECISIONS.md` per the plan-close gate.
- Revert the sprite-stress `enabled = true` change after capture; HUD ships off-by-default.
- Exit: `perf-runs/M18-hud/README.md` committed; numbers referenced from the M18 close-out note.

### 10. Docs and decision record

- `docs/LLM_INDEX.md`:
  - Subsystem row `Runtime HUD (M18)` -> `crates/tungsten/src/debug_hud.rs`, `crates/tungsten/src/telemetry.rs`, `crates/tungsten/src/app.rs`.
  - Task row `Fix HUD rows, toggle, or composition` -> same files + `docs/DECISION_INDEX.md` for `D-044`.
- `docs/DECISION_INDEX.md`: add `D-044` row under `ECS / Runtime Flow`.
- `docs/plans/Phase3.md`: flip M18 to `in progress` on start; to `complete` on land with `v0.15.0` + date; link the archived plan.
- `DECISIONS.md` add `D-044`:
  - HUD lives in the umbrella crate; reads existing telemetry resources; no new dependency.
  - `F4` is a hardcoded engine toggle until `M19` introduces `ActionMap`.
  - Extension point is `Vec<Box<dyn Fn(&World) -> Vec<HudRow>>>`; providers return `Vec` so top-N rows fit one provider slot.
  - Default off; examples opt in by mutating the resource during setup.
  - HUD-side frame-time smoothing uses EWMA (`alpha = 0.1`) on the previous frame's `FrameTimings::total_ms`; one-frame lag is intentional and keeps compose after extract.
  - Perf budget is qualitative: "negligible at Phase 3 scale", tracked via `perf-capture.sh` runs recorded in `perf-runs/M18-hud/`.
- Exit: `D-044` resolvable from `docs/DECISION_INDEX.md`; fresh-agent navigation works.

### 11. Validate

Run in order:

1. `cargo fmt --all`
2. `cargo test --workspace`
3. `./scripts/smoke-examples.sh`
4. `cargo run -p example-01-platformer` — HUD absent at startup; `F4` toggles HUD on; all listed rows populate; `F4` again hides; existing controls (movement, jump, audio, zoom, `F9`, `F11`) still work.
5. Perf capture pair per step 9; commit `perf-runs/M18-hud/`.

Manual spot checks when HUD is on:

- FPS row is stable (no per-frame digit flicker).
- Camera row updates as the player moves and as zoom changes.
- Display row reflects `F9`/`F11` changes within one frame.
- Player row disappears cleanly if `Tag` is removed (test-only mutation; revert).
- Sprite count matches visible sprite count as balls go off-screen if visibility culling is ever added (currently all balls emit — count stays stable).
- Top-3 systems rows show `physics_step` and the extract stages under load.

## Done-when checks

- `tungsten::DebugHud` defaults to `enabled = false`; inserted as a world resource by `App::new`.
- `KeyCode::F4` exists in `tungsten-core`; `winit`'s `F4` translates to it via `input_bridge`.
- `hud_toggle_system` is the first system registered by `App::new`; flipping `F4` flips `DebugHud.enabled` with one-frame latency max.
- When `enabled = false`, `compose_hud_text_sections` returns an empty `Vec` without running any provider; no `TextSection` is appended.
- When `enabled = true`, one `TextSection` appears with all listed rows for the platformer: FPS + frame ms, camera mode + position + zoom, display resolution + mode + vsync, player position + speed (player tag present), entity + sprite instance counts, top-3 slowest systems.
- Every built-in provider returns an empty `Vec` instead of panicking when its backing resource or entity is absent.
- `HudCorner` supports all four corners; `TopLeft` is default; right-side corners position using the documented monospace-width heuristic.
- FPS / frame-ms values are EWMA-smoothed and do not flicker per-frame at 60+ FPS.
- Custom rows registered via `DebugHud::add_row` appear after built-in rows in registration order.
- Platformer ships with `Tag::new("player")` on the player entity; no env var; `F4` is the only way to enable the HUD at runtime.
- `World::entity_count()` is `O(1)` and covered by a unit test.
- `RenderCounts` updates each frame after extract.
- `perf-runs/M18-hud/README.md` records HUD-off vs. HUD-on `total_ms` mean + p95 + p99 on the sprite-stress 300-frame capture; a regression `> 5%` would require a `DECISIONS.md` rationale but is not expected.
- `cargo test --workspace` passes.
- `./scripts/smoke-examples.sh` passes.
- `docs/LLM_INDEX.md`, `docs/DECISION_INDEX.md`, `docs/plans/Phase3.md`, and `DECISIONS.md` (D-044) match the shipped shape.

## Open Questions

- Whether right-side corners ship in M18 via the monospace-width heuristic, or are deferred until glyphon layout measurement is exposed. Plan ships both corners with the heuristic; flag for revisit if the approximation looks bad in practice.
- Whether the compose helper should run under a dedicated `HudTimings` bucket inside `FrameTimings` so the HUD's own cost shows up in its own "top-N systems" readout. Plan currently does not; HUD cost is rolled into the extract stage.
- Whether `HudActiveState` should sit in `tungsten::debug_hud` (current plan) or in a future `tungsten::scene` module created by M20. Plan puts it in `debug_hud` now; M20 can re-export or relocate.
- Whether `font_size = 16.0` is legible against the platformer's busy pixel-art background. If not, raise to `18.0` or add a documented outline pass. Plan ships plain text; revisit if unreadable.

---
status: done
milestone: M16
branch: 0.13
depends-on: M15
version-target: 0.13.0
unblocks: M18, gameplay polish
---

# Phase 3 Milestone 16 — Camera Module

## Goal

Centralize camera state and update behavior behind one engine-owned module so examples stop carrying bespoke follow/zoom logic. M16 should end with `CameraState` / `CameraController` / `CameraMode` in the engine, one authoritative camera writer per frame, and `01_platformer` migrated to that path without changing the render-side camera contract.

## Guardrails

- M15 is complete. Build on `Transform`, `Visibility`, and `sync_position_to_transform` from `D-042`; do not invent a parallel follow target path.
- Preserve `D-032`: `position = (0, 0)`, `zoom = 1.0`, `rotation = 0.0` must still produce the pre-M10 pixel-ortho matrix. Keep `CameraState.position` top-left anchored.
- Keep the core/render seam unchanged: core owns camera data/math, `tungsten` owns the update system, render still consumes only a `Mat4`.
- Text remains screen-space. M16 does not move glyphon or HUD text through the world camera.
- Do not read `docs/plans/archive/`; use live source/docs only.

## Non-goals

- M17/M18 work: display state/config, telemetry HUD, menu tooling, or any other UI follow-on.
- Multi-camera, split-screen, or viewport management.
- Rotated-space follow/dead-zone math. M16 only needs axis-aligned follow behavior.
- A backwards-compat `Camera2D` alias or shim. Rename in-tree call sites in the same change.
- New perf tooling or milestone-specific benchmarks unless implementation work proves they are required.
- Release bookkeeping before the milestone is actually shipped: `CHANGELOG.md`, `AGENTS.md`, workspace version bump, or plan archival.

## Read before coding

1. `AGENTS.md`
2. `docs/LLM_INDEX.md`
3. `crates/tungsten-core/src/camera.rs`
4. `crates/tungsten/src/app.rs`
5. `crates/tungsten-core/src/components.rs` (`sync_position_to_transform`)
6. `crates/tungsten-core/src/ecs/world.rs`
7. `examples/01_platformer/src/main.rs`
   Grep anchors: `struct CameraState`, `fn camera_input_system`, `fn camera_follow`, `camera_follows_player`, `camera_clamped_at_right_boundary`
8. `examples/02_sprite_stress/src/main.rs`
9. `examples/03_component_sprites/src/main.rs`
10. `docs/plans/Phase3.md`
11. `DECISIONS.md` entries `D-032`, `D-033`, `D-042`

## Files to touch

- `crates/tungsten-core/src/camera.rs`
- `crates/tungsten-core/src/lib.rs`
- `crates/tungsten/src/camera.rs` (new)
- `crates/tungsten/src/lib.rs`
- `crates/tungsten/src/app.rs`
- `crates/tungsten/src/tilemap_extract.rs`
- `crates/tungsten-render/src/sprite.rs`
- `crates/tungsten-render/src/quad.rs`
- `examples/01_platformer/src/main.rs`
- `examples/02_sprite_stress/src/main.rs`
- `examples/03_component_sprites/src/main.rs`
- `crates/tungsten/tests/camera.rs` (new)
- `docs/LLM_INDEX.md`
- `docs/plans/Phase3.md`
- `DECISIONS.md` only if implementation settles a non-obvious rule that is not already covered by `D-032`, `D-033`, or `D-042`

## Ordered steps

1. **Core camera API and math**
   Rename `Camera2D` to `CameraState` in `crates/tungsten-core/src/camera.rs`. Add the M16 controller-side data in core: `CameraController` (follow target, axis-aligned dead-zone, smoothing factor, bounds clamp, deterministic shake), `CameraMode { Free, Follow(Entity), Scripted }`, and a bounds helper. Add `rotation` to `CameraState` with the pivot fixed at `position` (top-left) so the anchor invariant from `D-032` holds; document this on the field. Keep `view_projection` correct under rotation, and redefine `visible_world_aabb` as the axis-aligned bounding box of the rotated view rectangle (callers like `tilemap_extract` over-cover slightly under non-zero rotation; note this in the doc comment). Preserve the exact `D-032` result when rotation is zero. Update `crates/tungsten-core/src/lib.rs` re-exports and extend the unit tests in `camera.rs` to cover the rotated-AABB contract and the zero-rotation invariant.
   Exit: `Camera2D` is gone from active core code and the old ortho regression test still passes.

2. **Umbrella camera system and app integration**
   Create `crates/tungsten/src/camera.rs` with `camera_update_system(&mut World)`. This is the single engine-side writer of `CameraState`: it reads `CameraController`, `DeltaTime`, `WindowSize`, and the followed entity `Transform`, and applies dead-zone, smoothing, bounds clamp, and shake in that order. Keep registration explicit; `App` should seed default camera resources but should not auto-install the system because examples need to choose ordering. Rename `Camera2D` usage in `crates/tungsten/src/app.rs` and `crates/tungsten/src/tilemap_extract.rs` (import, resource fetch, and doc comment). Update the active doc comments in `crates/tungsten-render/src/sprite.rs` and `crates/tungsten-render/src/quad.rs` so the grep gate in Done-when passes. Update `crates/tungsten/src/lib.rs` re-exports.
   Exit: render still follows `CameraState -> view_projection -> renderer.update_camera`, and camera update order remains explicit at the example level.

3. **Migrate the in-tree examples**
   Move `01_platformer` to the shared camera path. Delete the example-local `CameraState` resource, `camera_input_system`, and `camera_follow`; add a `Transform` to the player; register `sync_position_to_transform` after `physics_step`; configure `CameraController` on startup; and register `camera_update_system` where `camera_follow` ran. For zoom composition: the example keeps computing the viewport-fit base zoom (`window.height / map_h`) each frame and writes it to `CameraState.zoom`; the `=` / `-` keys mutate a user-zoom multiplier on `CameraController` (25% steps, 0.5..=2.0), and `camera_update_system` multiplies the two. Keep the current platformer follow/bounds behavior at `rotation = 0.0`. In `02_sprite_stress` and `03_component_sprites`, this milestone is only a `Camera2D` -> `CameraState` rename with no behavior change.
   Exit: `rg "struct CameraState|fn camera_follow|zoom_scale" examples/01_platformer/src/main.rs` returns no matches.

4. **Add deterministic camera tests**
   Add `crates/tungsten/tests/camera.rs` for fixed-dt, no-GPU camera scenarios. Cover at least follow behavior, bounds clamp, scripted mode behavior, the zero-rotation invariant, and deterministic shake if shake lands in the shared controller surface. Update the platformer camera tests to use the shared module rather than the deleted example-local logic.
   Exit: `cargo test -p tungsten --test camera` passes and at least one test exercises the camera module through a scripted frame sequence.

5. **Update the live docs, not archived history**
   Add a `Camera module` row to `docs/LLM_INDEX.md` pointing at the core camera file and the umbrella camera system. When implementation actually starts, flip this plan's header `status` to `in progress` and `docs/plans/Phase3.md` M16 to `in progress` in the same edit; when the milestone lands, flip both to `complete`. If implementation settles a non-obvious camera rule that is not already captured by `D-032`, `D-033`, or `D-042` (for example controller ownership, the rotation pivot at top-left, or the explicit non-auto-registration policy), record that in a new `DECISIONS.md` entry instead of expanding this plan.
   Exit: fresh agents can find the camera module from `docs/LLM_INDEX.md`, and the milestone tracker reflects the real state of the work.

6. **Validate**
   Run `cargo fmt --all`, `cargo test --workspace`, and `./scripts/smoke-examples.sh`. The smoke script is required because M16 touches engine wiring and example wiring. A manual `cargo run -p example-01-platformer` pass on a real GPU/display is optional but useful for checking follow feel, bounds clamp, and zoom input.
   Exit: workspace tests pass and the example smoke layer is green.

## Done-when checks

- `tungsten-core` exposes `CameraState`, `CameraController`, and `CameraMode`, and `Camera2D` no longer appears in active code. Grep gate: `rg "Camera2D" crates/ examples/` returns no matches.
- `crates/tungsten/src/app.rs` still renders from `CameraState::view_projection`, and the engine does not introduce a hidden auto-installed camera system.
- `examples/01_platformer/src/main.rs` uses `CameraController` + `camera_update_system`; it no longer contains a local camera resource or `camera_follow` implementation.
- `crates/tungsten/tests/camera.rs` exists and passes with fixed `dt` and deterministic inputs.
- `default_matches_pre_m10_ortho` still passes, and zero-rotation camera behavior remains bit-for-bit compatible with the pre-M10 matrix path.
- `cargo test --workspace` passes.
- `./scripts/smoke-examples.sh` passes.
- `docs/LLM_INDEX.md` points at the new camera module, and `docs/plans/Phase3.md` reflects the current M16 status.

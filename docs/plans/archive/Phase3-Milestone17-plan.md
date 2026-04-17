---
status: done
milestone: M17
target-branch: 0.14
version-target: 0.14.0
depends-on: M16 complete
unblocks: M18, future settings menu
---

# Phase 3 Milestone 17 - Display State + Config

## Goal

- Add a single authoritative `DisplayState` model in `tungsten-core`.
- Add a `display` section to `tungsten.json`.
- Expose one public runtime request API that gameplay/example code can call.
- Apply all window/surface mutations only inside `tungsten` at a frame boundary.
- Publish display telemetry for M18.
- Fall back to safe defaults on invalid display config values.

## Public surface after M17

- `tungsten_core::{DisplayState, DisplayMode, ScaleMode, Resolution, DisplayConfig, DisplayValidationError}`
- `tungsten::request_display_settings(world: &mut World, requested: DisplayState) -> Result<(), DisplayValidationError>`
- `tungsten::DisplayTelemetry`

## Non-goals

- Settings menu UI, keybind UI, or in-game settings screens.
- Writing updated display settings back to `tungsten.json` at runtime.
- Multi-window support or per-monitor routing.
- Real render-path use of `ScaleMode`; in M17 it is stored and telemetered only.
- Runtime-exclusive fullscreen support; `exclusive_fullscreen` is accepted in config but downgraded to borderless with a warning.
- Perfect frame pacing or per-platform scheduler tricks.
- Broad example cleanup beyond the one representative runtime demo.
- Release bookkeeping (`CHANGELOG.md`, version bump, plan archival) before the milestone actually lands.

## Guardrails

- Keep D-008: one workspace-root `tungsten.json`; do not add `display.json`.
- Keep D-007 / D-016: `tungsten-core` owns plain display data only; no `winit` or `wgpu` types in core.
- Keep D-018: render still consumes plain extracted data; display settings do not give the renderer `World` access.
- Keep legacy fields valid in M17: `window.width`, `window.height`, `window.vsync`, `render.present_mode`, and `render.max_frame_latency`.
- Precedence rule: `display.*` wins when both the new and legacy fields specify the same setting.
- Warning rule: warn on invalid display values and on direct legacy/display conflicts; do not warn for legacy-only configs in M17.
- Apply rule: gameplay code may request display changes during systems, but actual window/surface mutation happens once at the top of `RedrawRequested`, before surface acquire.
- Fullscreen rule: active runtime support is `windowed` and `borderless_fullscreen`; `exclusive_fullscreen` is config-accepted but runtime-downgraded.
- Borderless rule: the actual borderless size comes from the monitor/windowing system; after the resize event fires, that real size becomes the authoritative `DisplayState.resolution`.
- Frame-cap rule: use `ControlFlow::WaitUntil` only when a cap is set. Leave the uncapped path event-driven and non-spinning.
- Invalid display values must not panic and must not make startup fail if the JSON is otherwise valid.
- Invalid JSON syntax is still a hard `Config::load` error. Only bad display values get the soft-fallback path.
- No new runtime dependency.
- Do not read `docs/plans/archive/`.

## Read before coding

1. `AGENTS.md`
2. `docs/LLM_INDEX.md`
3. `crates/tungsten-core/src/config.rs`
4. `crates/tungsten-core/src/lib.rs`
5. `crates/tungsten/src/app.rs`
6. `crates/tungsten/src/telemetry.rs`
7. `crates/tungsten-render/src/renderer.rs`
8. `crates/tungsten-core/src/input.rs`
9. `crates/tungsten/src/input_bridge.rs`
10. `examples/01_platformer/src/main.rs`
11. `tungsten.json`
12. `DECISIONS.md` with `rg "D-007|D-008|D-015|D-016|D-018" DECISIONS.md`

## Files to touch

- `crates/tungsten-core/src/display.rs` new core data model and validation.
- `crates/tungsten-core/src/config.rs` new `display` section, resolve path, env overrides.
- `crates/tungsten-core/src/lib.rs` re-exports.
- `crates/tungsten-core/src/input.rs` add `F9` and `F11`.
- `crates/tungsten-core/tests/display.rs` config/resolve/env-override tests.
- `crates/tungsten-render/src/renderer.rs` surface pacing reconfigure support.
- `crates/tungsten/src/display.rs` new request/apply boundary plus in-file tests.
- `crates/tungsten/src/app.rs` startup seeding, frame-boundary apply, frame-cap pacing, resize sync.
- `crates/tungsten/src/input_bridge.rs` map `winit` `F9` and `F11`.
- `crates/tungsten/src/telemetry.rs` add `DisplayTelemetry`.
- `crates/tungsten/src/lib.rs` re-exports.
- `examples/01_platformer/src/main.rs` one runtime demo path for `F9` / `F11`.
- `tungsten.json` add a populated `display` block that preserves current behavior.
- `docs/LLM_INDEX.md` add a display-state row.
- `docs/plans/Phase3.md` align the M17 summary/status with the final plan shape.
- `DECISIONS.md` add `D-043` for the config shape and frame-boundary apply rule.

## Ordered steps

### 1. Core display model in `tungsten-core`

Create `crates/tungsten-core/src/display.rs` with:

- `DisplayMode { Windowed, BorderlessFullscreen, ExclusiveFullscreen }` with `snake_case` serde names.
- `ScaleMode { Stretch, Integer }` with `snake_case` serde names.
- `Resolution { width: u32, height: u32 }`.
- `DisplayState`:
  - `resolution: Resolution`
  - `display_mode: DisplayMode`
  - `vsync: bool`
  - `present_mode: Option<PresentModeConfig>`
  - `max_frame_latency: Option<u32>`
  - `scale_mode: ScaleMode`
  - `frame_rate_cap: Option<u32>`
- `DisplayConfig` as the serde-facing shape under `config.display`; every field optional / defaultable.
- `DisplayValidationError` for pure data validation only:
  - `InvalidResolution { width, height }`
  - `InvalidFrameRateCap(u32)`

Rules for the core layer:

- `DisplayState::default()` stays the engine fallback: `1280x720`, `windowed`, `vsync = false`, `present_mode = None`, `max_frame_latency = None`, `scale_mode = stretch`, `frame_rate_cap = None`.
- `DisplayState::validate()` rejects zero width, zero height, and `Some(0)` frame caps.
- `DisplayConfig::resolve(window, render)` layers `display.*` over the legacy `window.*` and `render.*` values.
- Unknown `display_mode` falls back to `windowed` with a warning.
- Unknown `scale_mode` falls back to `stretch` with a warning.
- `exclusive_fullscreen` is valid data at the core/config layer; the downgrade happens later in the app layer.
- `frame_rate_cap = 0` is treated as `None` with a warning at config-resolution time; it must not reach a validated runtime `DisplayState`.
- Do not put runtime-only errors such as unsupported present modes or fullscreen application failures in `tungsten-core`.

Tests in `display.rs`:

- defaults are stable.
- validation rejects zero dimensions.
- validation rejects `Some(0)` frame cap if one is constructed directly.
- unknown enum values fall back to safe defaults.
- `DisplayConfig::resolve()` honors `display.*` precedence over legacy fields.
- partial `display` sections inherit unspecified values from legacy fields or defaults.

Exit:

- `cargo test -p tungsten-core` passes.
- `tungsten-core` exports the new display types from `lib.rs`.

### 2. Config integration

Update `crates/tungsten-core/src/config.rs`:

- Add `#[serde(default)] pub display: DisplayConfig` to `Config`.
- Keep `window` for title and backwards compatibility.
- Keep `render.clear_color` outside the display model.
- Extend env overrides with:
  - `TUNGSTEN_DISPLAY_MODE=windowed|borderless_fullscreen|exclusive_fullscreen`
  - `TUNGSTEN_DISPLAY_RESOLUTION=WxH`
  - `TUNGSTEN_DISPLAY_FRAME_RATE_CAP=<u32>` with `0` meaning uncapped / `None`
- Keep existing `TUNGSTEN_RENDER_PRESENT_MODE` and `TUNGSTEN_RENDER_MAX_FRAME_LATENCY`.
- Make sure the resolved startup path uses `Config.display.resolve(...)`, not raw legacy fields.

Update `tungsten.json`:

```json
"display": {
  "resolution": { "width": 1280, "height": 720 },
  "display_mode": "windowed",
  "vsync": false,
  "present_mode": "auto",
  "max_frame_latency": 1,
  "scale_mode": "stretch",
  "frame_rate_cap": null
}
```

Rules for M17 config behavior:

- Leave legacy fields in place this milestone.
- Do not emit deprecation warnings for legacy-only configs yet.
- If both legacy and `display` values are present for the same concern, `display` wins and one warning is acceptable.

Tests in `crates/tungsten-core/tests/display.rs`:

- full config parse with `display`.
- partial `display` section fallback.
- legacy-only config still resolves correctly.
- conflicting `display` + legacy values prefer `display`.
- env overrides apply on top of parsed config.

Exit:

- `cargo test -p tungsten-core` passes with the new config cases.

### 3. Renderer support for runtime pacing changes

Update `crates/tungsten-render/src/renderer.rs`:

- Add `pub fn reconfigure_surface_pacing(&mut self, present_mode: Option<PresentModeConfig>, vsync: bool, max_frame_latency: Option<u32>) -> Result<(), RenderError>`.
- Reuse `resolve_present_mode()` and `resolve_max_frame_latency()`.
- Reconfigure the surface once per call, not once per individual field.
- Update `gpu_timings.present_mode` and `gpu_timings.max_frame_latency` after a successful reconfigure.
- Prefer stable lower-case present-mode labels over `format!("{:?}", ...)` for telemetry-facing strings.
- Keep `Renderer::resize()` unchanged.

Testing rule:

- Keep renderer tests at the pure-helper level.
- Add/adjust `resolve_present_mode()` / `resolve_max_frame_latency()` coverage as needed.
- Do not waste time trying to fake a real `wgpu::Surface` in unit tests.

Exit:

- `cargo test -p tungsten-render` passes.
- Existing renderer call sites still compile.

### 4. Runtime request/apply boundary in `tungsten`

Create `crates/tungsten/src/display.rs` with:

- `PendingDisplay(Option<DisplayState>)` as an internal resource.
- Internal `DisplayDelta` helper for app-layer decisions:
  - `resize`
  - `display_mode_changed`
  - `surface_pacing_changed`
  - `scale_mode_changed`
  - `frame_rate_cap_changed`
- `pub fn request_display_settings(world: &mut World, requested: DisplayState) -> Result<(), DisplayValidationError>`
- Internal `drain_pending_display(app: &mut App)` or equivalent helper called only by the app loop.

Behavior rules:

- `request_display_settings()` validates immediately and stores the request in `PendingDisplay`.
- A later request in the same frame replaces the earlier one.
- `PendingDisplay` is internal; gameplay code should go through `request_display_settings()`, not by writing the resource directly.
- Runtime apply failures are logged and the last known-good `DisplayState` remains authoritative.

Update `crates/tungsten/src/app.rs`:

- Add `frame_budget: Option<Duration>` to `App`.
- In `App::new`, resolve the startup display state from config and insert:
  - `DisplayState`
  - `PendingDisplay`
  - `DisplayTelemetry`
- Seed `WindowSize` from the resolved display resolution, not directly from `config.window.width` / `height`.
- In `resumed()`, create the window using the resolved display state.
- If startup mode requests borderless or exclusive, apply fullscreen intent around startup and let the authoritative size settle through `window.inner_size()` and the normal resize path.
- Initialize the renderer from the resolved display pacing values, not raw legacy config values.
- At the top of `WindowEvent::RedrawRequested`, before delta-time math and before render acquire, drain and apply pending display requests.
- In `WindowEvent::Resized`, keep `WindowSize`, `DisplayState.resolution`, and `DisplayTelemetry.resolution` in sync.
- At the end of `RedrawRequested`:
  - if `frame_budget` is `Some(budget)`, call `event_loop.set_control_flow(ControlFlow::WaitUntil(frame_start + budget))`
  - if `frame_budget` is `None`, call `event_loop.set_control_flow(ControlFlow::Wait)`
  - keep `window.request_redraw()` as the continuous-render trigger

Runtime apply order:

1. Take the pending request, if any.
2. Compare it with the current `DisplayState`.
3. If fullscreen mode changed:
   - `Windowed` => `window.set_fullscreen(None)`
   - `BorderlessFullscreen` => `window.set_fullscreen(Some(Fullscreen::Borderless(None)))`
   - `ExclusiveFullscreen` => warn, downgrade to borderless, then apply the borderless request
4. If the requested windowed resolution changed, call `window.request_inner_size(...)`.
5. If surface pacing changed, call `renderer.reconfigure_surface_pacing(...)`.
6. If frame-rate cap changed, update `App.frame_budget`.
7. Write the effective post-apply `DisplayState` back to the world.
8. Refresh `DisplayTelemetry`.

Add `DisplayTelemetry` to `crates/tungsten/src/telemetry.rs`:

- `resolution: (u32, u32)`
- `display_mode: DisplayMode`
- `vsync: bool`
- `actual_present_mode: Option<String>`
- `max_frame_latency: Option<u32>`
- `scale_mode: ScaleMode`
- `frame_rate_cap: Option<u32>`

Export from `crates/tungsten/src/lib.rs`:

- `request_display_settings`
- `DisplayTelemetry`

Do not re-export:

- `PendingDisplay`
- `DisplayDelta`
- `drain_pending_display`

Exit:

- `cargo build --workspace` is clean.
- The display request/apply boundary is coherent and internal-only details stay internal.

### 5. Example wiring in `01_platformer`

Update input plumbing first:

- Add `F9` and `F11` to `crates/tungsten-core/src/input.rs`.
- Map `winit` `F9` and `F11` in `crates/tungsten/src/input_bridge.rs`.

Update `examples/01_platformer/src/main.rs`:

- Remove the hardcoded `config.window.width = 1920` and `config.window.height = 1080` override so the example demonstrates config-backed startup.
- Add one small display-input system.
- `F11` toggles `DisplayMode::Windowed` <-> `DisplayMode::BorderlessFullscreen`.
- `F9` toggles `vsync`.
- When `F9` flips `vsync`, also set `present_mode = None` so the runtime path re-resolves from the new vsync intent.
- The system reads the current `DisplayState`, constructs the next state, and calls `request_display_settings(world, next_state)`.

Scope rule:

- Only `01_platformer` needs the runtime-change demo in M17.
- Do not broaden the milestone by cleaning up `02_sprite_stress` or `03_component_sprites`.

Exit:

- `cargo run -p example-01-platformer` starts from config-backed display values.
- `F11` toggles windowed/borderless without crashing.
- `F9` toggles vsync and the active present-mode telemetry updates on the next frame.

### 6. Tests

Keep tests close to the implementation:

- `crates/tungsten-core/src/display.rs` for pure display-model behavior.
- `crates/tungsten-core/tests/display.rs` for config parsing / resolve / env overrides.
- `crates/tungsten/src/display.rs` `#[cfg(test)]` for pending-request replacement, frame-budget math helpers, and telemetry sync helpers without making internal types public.

Do not add a `crates/tungsten/tests/display.rs` integration test if it exists only to reach private internals. Keep that logic in module tests instead.

Automated validation target:

- `cargo test --workspace`
- `./scripts/smoke-examples.sh`

Manual validation target:

- `cargo run -p example-01-platformer`
- verify `F9` and `F11`
- verify one invalid display value falls back with a warning instead of panicking

Exit:

- Both automated layers pass.
- Manual platformer checks behave as expected on a real GPU/display.

### 7. Docs and decision record

Update `docs/LLM_INDEX.md`:

- add a row for display state/config pointing at:
  - `crates/tungsten-core/src/display.rs`
  - `crates/tungsten/src/display.rs`
  - `tungsten.json`

Update `docs/plans/Phase3.md`:

- mark M17 `in progress` when implementation starts
- mark M17 `complete` when it lands
- fix the M17 summary so it no longer mentions `display.json`
- align the Phase 3 object summary so `DisplayState` is described as a core data object plus umbrella request/apply wiring

Add `DECISIONS.md` entry `D-043`:

- `display` is a section in `tungsten.json`, not a second config file
- legacy window/render display fields stay valid for M17
- gameplay code requests display changes through a public request API; actual mutation happens only at the top of `RedrawRequested`
- `exclusive_fullscreen` is accepted in config but downgraded to borderless until a later milestone adds real video-mode selection

Exit:

- A fresh agent can find the new display files from `LLM_INDEX`.
- The Phase 3 tracker matches the final M17 shape.
- `D-043` exists.

### 8. Validate

Run in order:

1. `cargo fmt --all`
2. `cargo test --workspace`
3. `./scripts/smoke-examples.sh`
4. `cargo run -p example-01-platformer`

Manual checks:

- startup uses the `display` block from `tungsten.json`
- `F11` toggles borderless fullscreen and back
- `F9` toggles vsync
- telemetry shows the active resolution, display mode, vsync state, and actual present mode
- `"display_mode": "bogus"` produces one warning and a safe fallback
- `"frame_rate_cap": 0` behaves as uncapped

## Done-when checks

- `tungsten-core` exports `DisplayState`, `DisplayMode`, `ScaleMode`, `Resolution`, `DisplayConfig`, and `DisplayValidationError`.
- The startup path resolves display settings through `Config.display.resolve(...)`; it does not read raw legacy display fields directly at the app boundary.
- `App::new` seeds `DisplayState`, `DisplayTelemetry`, and the initial `WindowSize` from the resolved display settings.
- The only public runtime API for gameplay/example code is `request_display_settings(world, requested)`.
- `PendingDisplay` and runtime apply helpers stay internal to the `tungsten` crate.
- `DisplayTelemetry` includes `frame_rate_cap` and the actual applied present-mode label.
- Uncapped rendering stays event-driven and non-spinning; capped rendering uses `WaitUntil`.
- `01_platformer` routes `F9` and `F11` through `request_display_settings()` and does not perform direct `winit` / `wgpu` display mutation.
- `KeyCode` and input translation support `F9` and `F11`.
- Invalid display values in otherwise valid JSON fall back with warnings instead of failing `Config::load`.
- `cargo test --workspace` passes.
- `./scripts/smoke-examples.sh` passes.
- `docs/LLM_INDEX.md`, `docs/plans/Phase3.md`, and `DECISIONS.md` are updated to match the shipped shape.

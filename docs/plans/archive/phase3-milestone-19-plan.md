---
status: done
milestone: M19
branch: 0.16
version-target: 0.16.0
depends-on: none (standalone core-systems milestone)
unblocks: M20 (scene/state), future settings menu, ergonomic rebinding in M21+ examples
---

# Phase 3 Milestone 19 — Input Mapping

## Goal

- Replace hardcoded `KeyCode` checks in gameplay code with action-based lookups.
- Introduce a core-owned `ActionMap` resource: `action_name -> Vec<Binding>`, queried through `is_pressed` / `just_pressed` / `just_released` methods that take an `&InputState`.
- Load bindings at startup from an optional workspace-root `input.json`; fall back to an engine-default map when the file is absent or invalid.
- Wire `input.json` into the existing `notify`-based hot-reload path so edits rebind live without restart.
- Add mouse support to the same action-map pipeline: buttons, cursor position + per-frame delta, wheel line/pixel deltas, and discrete scroll-up / scroll-down bindings.
- Persist runtime rebinds back to `input.json` from the engine with an atomic writer path and best-effort formatting preservation.
- Migrate engine-reserved controls (`F4`, `F9`, `F11`, `Escape`) onto action bindings with sensible engine defaults.
- Migrate the platformer example (`examples/01_platformer`) from raw `KeyCode` checks to action lookups as the reference consumer, including at least one mouse-button action and one scroll action.

## Non-goals

- Gamepad / controller bindings. Ship keyboard + mouse only in M19.
- Axis, 1D/2D virtual axes, analog deadzones, chord/sequence bindings. All actions are boolean in M19.
- Runtime rebinding UI or settings menu. M19 only exposes the data + runtime-reload surface.
- Input replay / scripted-input playback (M21 gate: "scripted input playback for at least one menu-gameplay-pause scenario" — that ships with M20/M21).
- Deep schema validation (JSON-schema style); keep validation to structural parse + unknown-key warnings.
- Perfect round-trip preservation of arbitrary file layout/comments in `input.json`. Persist is best-effort: preserve existing top-level structure/formatting where practical, otherwise rewrite canonical JSON.
- Release bookkeeping (`CHANGELOG.md`, version bump, plan archival) before the milestone lands.
- Adding a new runtime dependency (`input.json` reuses `serde_json`; hot reload reuses `notify`).

## Guardrails

- `D-007` / `D-016`: `ActionMap` lives in `tungsten-core`; no `winit` / `wgpu` types allowed. String-keyed physical bindings only.
- `D-008`: Missing `input.json` falls back to defaults; invalid JSON is fatal at startup (consistent with `tungsten.json` behaviour). Runtime hot-reload of invalid JSON logs an error and keeps the previous map.
- `D-015`: No new dependency. `serde`, `serde_json`, `notify` already satisfy rules 2/1.
- `D-022`: Panic on programmer errors (looking up a typoed action name returns `false` — this is a runtime miss, not a bug). Return `Result` only at load boundaries.
- `D-031`: Reuse the existing `HotReloadWatcher`; no second watcher instance; 50 ms debounce already covers editor swap-saves.
- `D-039` / `D-040`: `ActionMap` is a pure data resource. Hot-reload swap happens inside the existing hot-reload stage (`systems -> flush commands -> flush events -> hot reload -> extract -> render`).
- Engine-owned actions (`engine_toggle_hud`, `engine_toggle_vsync`, `engine_toggle_fullscreen`, `engine_exit`) still ship with default bindings in the built-in map so deleting `input.json` cannot strand the user.
- Frame-order invariant unchanged. `ActionMap` reads are pure queries of `InputState`; no new frame-boundary mutation.
- Empty-overhead gate when no `input.json` is present: engine still loads the built-in default map; cost is a one-time `HashMap` build at startup.
- Do not read `docs/plans/archive/`.

## Read before coding

1. `AGENTS.md`
2. `docs/LLM_INDEX.md`
3. `crates/tungsten-core/src/input.rs` (existing `KeyCode`, `InputState`, edge detection)
4. `crates/tungsten/src/input_bridge.rs` (`winit` → core key translation; one place to extend if a new `KeyCode` variant is needed)
5. `crates/tungsten-core/src/config.rs` (Config load pattern: env overrides, `ConfigError`, graceful fallback + fatal parse)
6. `crates/tungsten-core/src/display.rs` + `display_tests.rs` (precedent for validated config surface inside `tungsten-core`)
7. `crates/tungsten/src/app.rs` (`App::new`, startup wiring, hot-reload dispatch switch on file extension)
8. `crates/tungsten/src/hot_reload.rs` (watcher construction, debounce, `drain_ready`)
9. `crates/tungsten/src/asset_loader.rs` (reload entry-point pattern: `reload_manifest`, `reload_sprite`, etc.)
10. `examples/01_platformer/src/systems.rs` (raw `is_pressed` / `just_pressed` call sites to migrate)
11. `examples/01_platformer/src/main.rs` (header `Controls:` block to update)
12. `docs/plans/archive/phase3-milestone-18-plan.md` (format precedent)
13. `DECISIONS.md` via `rg "D-007|D-008|D-015|D-022|D-031|D-043|D-044" DECISIONS.md`

## External references (for the future iterating pass)

- Bevy `bevy_input` + `leafwing-input-manager` naming conventions for action-typed input: <https://docs.rs/leafwing-input-manager/latest/leafwing_input_manager/>
- Godot 4 `InputMap` action/binding semantics: <https://docs.godotengine.org/en/stable/tutorials/inputs/inputevent.html>
- `winit` `KeyCode` reference (for any new variants exposed through `input_bridge`): <https://docs.rs/winit/0.30.12/winit/keyboard/enum.KeyCode.html>
- `serde_json` error positioning for structured load errors: <https://docs.rs/serde_json/latest/serde_json/struct.Error.html>

## Files to touch

| File | Change |
|------|--------|
| `crates/tungsten-core/src/input.rs` | extend `InputState` with cursor position/delta, scroll line/pixel deltas, scroll-direction edges, and helper methods; expose submodules below |
| `crates/tungsten-core/src/input/action_map.rs` | `Binding`, `ActionMap`, `ActionMapError`, default map builder, mouse/scroll bindings, persist/atomic-save path, unit tests |
| `crates/tungsten-core/src/input/key_serde.rs` | `KeyCode` / `MouseButton` / scroll-direction string helpers; support extra mouse buttons in JSON |
| `crates/tungsten-core/src/lib.rs` | `pub use input::{ActionMap, ActionMapError, Binding}` |
| `crates/tungsten/src/input_bridge.rs` | no behavioural change; only touched if a missing `KeyCode` variant surfaces while exercising `input.json` defaults |
| `crates/tungsten/src/asset_loader.rs` | `reload_action_map(path, &mut world) -> Result<(), ...>` mirroring `reload_manifest`, preserving last-known-good on reload failure |
| `crates/tungsten/src/hot_reload.rs` | keep watching asset dirs plus extra watched files (workspace-root `input.json`) |
| `crates/tungsten/src/app.rs` | load `ActionMap` in `App::new`; insert as world resource; watch `input.json`; route reloads through `reload_action_map`; feed cursor/scroll events into `InputState`; route `engine_exit` through the action map |
| `crates/tungsten/src/display.rs` | move `F9` / `F11` handling to an engine-owned action-driven system |
| `crates/tungsten/src/debug_hud.rs` | route HUD toggle through `engine_toggle_hud` rather than a hardcoded `F4` check |
| `crates/tungsten/src/lib.rs` | re-export `ActionMap`, `ActionMapError`, `Binding` from the umbrella for ergonomic `tungsten::ActionMap` |
| `input.json` | workspace-root default bindings, now including engine actions, mouse buttons, and scroll actions |
| `examples/01_platformer/src/systems.rs` | action lookups for gameplay controls only; remove example-owned reserved-key handling once engine systems own it |
| `examples/01_platformer/src/main.rs` | header `Controls:` block: annotate each row with the action name it maps to, including mouse/scroll defaults |
| `examples/01_platformer/src/extract.rs` | surface cursor position/delta and scroll deltas in the on-screen reference text for manual verification |
| `examples/01_platformer/src/tests.rs` | extend tests for action defaults and the updated runtime system order |
| `examples/01_platformer/assets/` | **no** local `input.json`; root `input.json` covers it |
| `docs/LLM_INDEX.md` | add subsystem row `Input action map (M19)` → `crates/tungsten-core/src/input/`, `crates/tungsten/src/asset_loader.rs`, `input.json`; add task row `Fix action lookups, input.json parsing, or rebind hot reload` |
| `docs/DECISION_INDEX.md` | add `D-045` one-liner under `ECS / Runtime Flow` |
| `docs/plans/Phase3.md` | flip M19 status on start (`in progress`), on land (`complete`) with `v0.16.0` + date; link this plan file, then link archived plan after archival |
| `DECISIONS.md` | add `D-045` full entry |
| `tungsten.json` | unchanged |

## Public surface after M19

- `tungsten_core::input::{ActionMap, ActionMapError, Binding}`
- `tungsten::{ActionMap, ActionMapError, Binding}` (re-exports)
- `ActionMap::load(path: &Path) -> Result<Self, ActionMapError>`
- `ActionMap::default_map() -> Self` (engine defaults; see schema below)
- `ActionMap::persist(&mut self) -> Result<(), ActionMapError>` and/or `persist_to(path)` for runtime rebind writes
- `ActionMap::is_pressed(&self, input: &InputState, action: &str) -> bool`
- `ActionMap::just_pressed(&self, input: &InputState, action: &str) -> bool`
- `ActionMap::just_released(&self, input: &InputState, action: &str) -> bool`
- `ActionMap::bindings(&self, action: &str) -> &[Binding]` (read-only inspection for HUD / debug tooling in M21)
- `ActionMap::replace_bindings(&mut self, action: impl Into<String>, bindings: Vec<Binding>)` (runtime rebind, paired with explicit persist)
- `InputState::cursor_position()`, `cursor_delta()`, `scroll_line_delta()`, `scroll_pixel_delta()`
- `tungsten::asset_loader::reload_action_map(path, &mut World) -> Result<(), ActionMapError>` (pub within umbrella for completeness; optional)

Keep private:

- The `(key|mouse) -> &'static str` / `&str -> KeyCode` lookup tables.
- Default-map builder inputs beyond the single `default_map()` entry point.

## `Binding` shape (core-owned, no winit types)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Binding {
    Key   { code: KeyCode },
    Mouse { button: MouseButton },
    Scroll { direction: ScrollDirection },
}
```

Struct variants (not tuple) so serde's `tag = "kind"` produces a flat object with the inner field flattened in. JSON representation:

```json
{ "kind": "key",   "code": "KeyA" }
{ "kind": "mouse", "button": "left" }
{ "kind": "scroll", "direction": "up" }
```

`KeyCode` serializes to its variant name (`"ArrowLeft"`, `"KeyA"`, `"Space"`, ...). `MouseButton` serializes to lowercase (`"left"`, `"right"`, `"middle"`). Unknown strings → `ActionMapError::UnknownKey { name }`.

## `input.json` schema

Workspace-root file. All fields optional; missing actions fall back to `default_map()`.

```json
{
  "actions": {
    "move_left":   [{ "kind": "key", "code": "ArrowLeft" },  { "kind": "key", "code": "KeyA" }],
    "move_right":  [{ "kind": "key", "code": "ArrowRight" }, { "kind": "key", "code": "KeyD" }],
    "jump":        [{ "kind": "key", "code": "Space" }],
    "zoom_in":     [{ "kind": "key", "code": "Equal" }],
    "zoom_out":    [{ "kind": "key", "code": "Minus" }],
    "audio_toggle_music": [{ "kind": "key", "code": "KeyM" }],
    "audio_stop_all":     [{ "kind": "key", "code": "KeyS" }],
    "volume_preset_low":  [{ "kind": "key", "code": "Digit1" }],
    "volume_preset_mid":  [{ "kind": "key", "code": "Digit2" }],
    "volume_preset_high": [{ "kind": "key", "code": "Digit3" }],
    "engine_toggle_hud":        [{ "kind": "key", "code": "F4" }],
    "engine_toggle_vsync":      [{ "kind": "key", "code": "F9" }],
    "engine_toggle_fullscreen": [{ "kind": "key", "code": "F11" }],
    "engine_exit":              [{ "kind": "key", "code": "Escape" }]
  }
}
```

Rules:

- Unknown top-level keys → warning log + ignored (forward-compat).
- Unknown action name in a consumer (`ActionMap::is_pressed(..., "dance")`) → returns `false` (per `D-022` runtime-miss rule).
- Duplicate bindings within one action → deduplicated silently.
- Empty binding list → legal (action exists, never fires).
- Parse error on load → startup fatal (`ConfigError`-style); on hot-reload → error log + keep previous map.
- Runtime persist writes atomically to `input.json` and best-effort preserves the surrounding file layout when the existing file already parses as valid JSON.

## Engine default map (used when `input.json` is absent)

Mirrors the JSON schema above. Lives as const-ish data in `action_map.rs` so tests and fallback share the same source of truth.

## Ordered steps

### 1. `KeyCode` + `MouseButton` (de)serialization plumbing

- In `crates/tungsten-core/src/input/key_serde.rs`:
  - `const KEYCODE_NAMES: &[(KeyCode, &str)]` covering every variant currently in `KeyCode` except `Other(u32)`.
  - `pub fn keycode_from_str(&str) -> Option<KeyCode>` and `pub fn keycode_to_str(KeyCode) -> &'static str`.
  - Mirror for `MouseButton` (`"left" | "right" | "middle"`; `Other(u16)` is not (de)serialized — return `None`).
  - Unit tests: round-trip every current variant; unknown name returns `None`.
- In `input.rs`, derive `Serialize`/`Deserialize` for `KeyCode` + `MouseButton` via `serde(try_from = "&str", into = "&'static str")` using the above helpers.
- Exit: `cargo test -p tungsten-core` passes; round-trip of every shipped variant green.

### 2. `ActionMap` data model + default map

- `crates/tungsten-core/src/input/action_map.rs`:

  ```rust
  use std::collections::HashMap;
  use crate::input::{InputState, KeyCode, MouseButton};

  #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
  #[serde(tag = "kind", rename_all = "snake_case")]
  pub enum Binding {
      Key   { code: KeyCode },
      Mouse { button: MouseButton },
      Scroll { direction: ScrollDirection },
  }

  #[derive(Debug, Clone, Default, Serialize, Deserialize)]
  pub struct ActionMap {
      #[serde(default)]
      actions: HashMap<String, Vec<Binding>>,
  }

  #[derive(Debug, Error)]
  pub enum ActionMapError {
      #[error("failed to read action map '{path}': {source}")]
      Io { path: String, source: std::io::Error },
      #[error("invalid action map in '{path}': {source}")]
      Parse { path: String, source: serde_json::Error },
      #[error("unknown key name '{name}' in action map")]
      UnknownKey { name: String },
  }
  ```

- Methods:
  - `pub fn default_map() -> Self`
  - `pub fn load(path: &Path) -> Result<Self, ActionMapError>`
  - `pub fn merged_with_defaults(loaded: Self) -> Self` — user-supplied entries override defaults; defaults fill gaps.
  - `pub fn is_pressed(&self, input: &InputState, action: &str) -> bool`
  - `pub fn just_pressed(&self, input: &InputState, action: &str) -> bool`
  - `pub fn just_released(&self, input: &InputState, action: &str) -> bool`
  - `pub fn bindings(&self, action: &str) -> &[Binding]`
  - `pub fn replace_bindings(&mut self, action: impl Into<String>, bindings: Vec<Binding>)`

- Query semantics (dispatch by variant — `InputState` has separate method names for keys vs. mouse):
  - `Binding::Key { code }` → `InputState::is_pressed` / `just_pressed` / `just_released`.
  - `Binding::Mouse { button }` → `InputState::is_mouse_pressed` / `mouse_just_pressed` / `mouse_just_released`.
  - `ActionMap::is_pressed(action)` = any binding in the action's list reports pressed.
  - `ActionMap::just_pressed(action)` = any binding reports `just_pressed` this frame.
  - `ActionMap::just_released(action)` = any binding reports `just_released` this frame.
  - Empty binding list or unknown action → `false` for all three.

- Tests in the same file:
  - `default_map_has_platformer_actions` — asserts gameplay, engine, and mouse/scroll-backed defaults.
  - `unknown_action_returns_false`
  - `merged_with_defaults_preserves_user_overrides`
  - `load_parses_sample_input_json` using a `include_str!` fixture string (in-memory, no disk).
  - `load_invalid_json_is_error`
  - `query_respects_multiple_bindings` (map `move_left` to [ArrowLeft, KeyA], press KeyA, assert `is_pressed == true`).
  - `edge_query_just_pressed_any_binding` (press ArrowLeft, assert `just_pressed("move_left")` once, then `begin_frame`, assert false).
  - `mouse_button_binding_dispatches`
  - `scroll_binding_dispatches`
  - `persist_round_trips_rebound_actions`

- Exit: `cargo test -p tungsten-core` passes.

### 3. Export + re-export

- `crates/tungsten-core/src/input.rs` becomes a module folder (`input/mod.rs`) or keep as a file and declare `pub mod action_map; pub mod key_serde;`. Pick whichever minimizes diff noise — table above assumes submodules under a `input/` folder; adjust naming if the iterating pass prefers a flat layout.
- `crates/tungsten-core/src/lib.rs`: add `pub use input::{ActionMap, ActionMapError, Binding};` next to the existing `InputState, KeyCode, MouseButton` re-export.
- `crates/tungsten/src/lib.rs`: re-export `ActionMap`, `ActionMapError`, `Binding` under `tungsten::` root so examples use `tungsten::ActionMap` unprefixed, matching `tungsten::DebugHud` etc.
- Exit: `cargo build --workspace` clean.

### 4. Startup load + resource insertion

- In `crates/tungsten/src/app.rs` `App::new`:
  - Resolve `input.json` path relative to the workspace root (mirror `Config::load`'s path handling).
  - Attempt `ActionMap::load(path)`; on success, `merged_with_defaults`. On missing file (`ErrorKind::NotFound`), use `ActionMap::default_map()` and log `info!`. On other `Io` or `Parse` errors, propagate the error from `App::new` (fatal at startup per `D-008`).
  - `world.insert_resource(action_map)`.
- Exit: `cargo build --workspace` clean; startup with and without `input.json` both work.

### 5. Hot reload wiring

- Confirm watcher scope. Today `App::enable_hot_reload(assets_dirs, manifest_path)` only watches asset dirs (see `crates/tungsten/src/app.rs` and `crates/tungsten/src/hot_reload.rs`). `input.json` lives at the workspace root, which the current watcher does not cover.
- Chosen approach: extend `App::enable_hot_reload` to additionally watch the *parent directory* of `input.json` with `RecursiveMode::NonRecursive`, filtered to `input.json` file name. Implementation sketch:
  - Store `input_map_path: Option<PathBuf>` on `App`.
  - Teach `HotReloadWatcher::new` to accept an optional extra `watch_files: &[PathBuf]` and internally watch each file's parent non-recursively, recording the canonical file path for filtering in `drain_ready`.
  - Alternative if the simpler path keeps surface small: watch the workspace root non-recursively always (only when `enable_hot_reload` is active). Decide in implementation; either way, document the chosen approach in the `D-045` entry.
- In `process_hot_reload`, add dispatch:
  ```rust
  if canon.file_name().map(|n| n == "input.json").unwrap_or(false) {
      if let Err(e) = asset_loader::reload_action_map(&canon, &mut self.world) {
          log::error!("Action map reload: {e}");
      }
      continue;
  }
  ```
- `crates/tungsten/src/asset_loader.rs`:
  ```rust
  pub fn reload_action_map(path: &Path, world: &mut World) -> Result<(), ActionMapError> {
      let loaded = ActionMap::load(path)?;
      let merged = ActionMap::merged_with_defaults(loaded);
      if let Some(map) = world.get_resource_mut::<ActionMap>() {
          *map = merged;
      } else {
          world.insert_resource(merged);
      }
      Ok(())
  }
  ```
- Exit: edit `input.json` while platformer is running; action lookup picks up the new binding within one frame after the 50 ms debounce.

### 6. Mouse surface and platformer migration

- `examples/01_platformer/src/systems.rs`:
  - Replace every gameplay `input.is_pressed(KeyCode::…)` / `input.just_pressed(...)` with `action_map.is_pressed(&input, "...")`.
  - Borrow pattern: take an immutable borrow of both `InputState` and `ActionMap` at the top of the system, then release before mutable world borrows (mirrors existing pattern that scopes the `InputState` borrow).
  - Demonstrate mouse buttons by binding at least one gameplay action to left/right/middle mouse defaults without removing the keyboard equivalents.
  - Demonstrate scroll by binding `zoom_in` / `zoom_out` (or equivalent clearly visible actions) to scroll-up / scroll-down.
- `examples/01_platformer/src/extract.rs` / `state.rs`:
  - Surface cursor position, per-frame delta, and scroll line/pixel deltas in the on-screen reference text so manual runtime checks have a visible target.
- `examples/01_platformer/src/main.rs`:
  - Header `Controls:` block: add a trailing column noting the action name.
  - Example line: `A / D or ←/→   horizontal movement   (actions: move_left / move_right)`.
  - Add a note at the bottom: `Edit ./input.json and save while running to rebind at runtime (hot reload).`
- Engine side:
  - Move `F9` / `F11` display toggles and `F4` HUD toggle to action lookups.
  - Replace the hardcoded Escape event check with an `engine_exit` action lookup, still gated by `App::set_exit_on_escape(false)` for examples that want to disable engine-side exit.
- Exit: platformer runs identically to pre-M19; modifying `input.json` rebinds live.

### 7. Persist path + ship the default `input.json`

- Contents match the schema section above.
- Purpose: give users a discoverable starting point and match the `tungsten.json` "default present" ergonomic.
- Because `ActionMap::default_map()` already encodes the same map, deleting `input.json` keeps behaviour identical — verify with a smoke run.
- Add a runtime writer path that:
  - writes `input.json` atomically via temp-file + rename in the same directory,
  - best-effort preserves the surrounding JSON layout when the existing file can be patched in place,
  - falls back to canonical pretty JSON when the existing layout cannot be preserved safely.
- Exit: `cargo run -p example-01-platformer` with and without `input.json` is behaviourally equivalent.

### 8. Benchmark / perf check

- Expected cost: per-action lookup is a small `HashMap` get + O(bindings) scan. Default map has ≤ 10 actions with ≤ 2 bindings each. No new allocation on the query path (return `&[Binding]` slice).
- Add/extend a `criterion` micro-bench under `crates/tungsten-core/benches/` measuring both keyboard and mouse-source dispatch (`mouse button` and `scroll`) against the ≤ 1 µs per-call target.
- Exit: micro-bench committed; key + mouse numbers logged verbatim in the M19 close-out note.

### 9. Docs and decision record

- `docs/LLM_INDEX.md`:
  - Subsystem row `Input action map (M19)` → `crates/tungsten-core/src/input/`, `crates/tungsten/src/asset_loader.rs`, `input.json`.
  - Task row `Fix action lookups, input.json parsing, or rebind hot reload` → same files + `docs/DECISION_INDEX.md` for `D-045`.
- `docs/DECISION_INDEX.md`: add `D-045` under `ECS / Runtime Flow`: "Input actions map string names to `Vec<Binding>` in `tungsten-core`; loaded from optional workspace-root `input.json`, hot-reloaded through the existing `notify` watcher; engine debug keys stay hardcoded."
- `docs/DECISION_INDEX.md`: update `D-044` / `D-045` summaries to reflect that engine-reserved controls now route through the action map.
- `docs/plans/Phase3.md`: flip M19 from "next recommended" to `in progress` on start; to `complete` on land with `v0.16.0` + date; add a "Detailed implementation plan" link to this file, then update to the archived path after archival.
- `DECISIONS.md` add `D-045` with:
  - Scope: boolean actions, keyboard + mouse, no gamepad/axis.
  - Reason `ActionMap` lives in `tungsten-core`: core already owns `InputState` + `KeyCode`; no winit leak; `D-007` compliance.
  - Reason no new dep: `serde`/`serde_json`/`notify` already cover load + reload (`D-015` rules 2/1).
  - Reason engine-reserved controls still belong to engine-owned action names even though the physical bindings are rebindable.
  - Merge rule: user-supplied `input.json` entries override defaults per action; missing actions inherit defaults.
  - Hot-reload failure mode: keep previous map, log error; no behaviour break mid-session.
  - Persist rule: runtime rebind writes stay in the same file and use an atomic save path.
- Exit: `D-045` resolvable from `docs/DECISION_INDEX.md`; fresh-agent navigation works.

### 10. Validate

Run in order:

1. `cargo fmt --all`
2. `cargo test --workspace`
3. `./scripts/smoke-examples.sh`
4. `cargo run -p example-01-platformer` — all controls behave identically to pre-M19; verify A/D, arrows, Space, M, 1/2/3, S, =, -, F4, F9, F11 still work.
5. Runtime rebind check: with platformer running, edit `input.json` and swap `"jump"` from `Space` to `Enter`; save; confirm Space no longer jumps and Enter does, within ≤ 200 ms.
6. Default-fallback check: delete `input.json`; re-run platformer; confirm unchanged behaviour.
7. Invalid-JSON check: corrupt `input.json`; re-run; confirm fatal startup error with a clear message; restore file.
8. Hot-reload invalid-JSON check: while running, write invalid JSON to `input.json`; confirm error log + bindings unchanged.
9. Mouse checks: click each bound mouse button; move the cursor and confirm position + delta update; scroll up/down and confirm the bound actions fire; hot-reload a mouse-button rebind.
10. Reserved-key migration: confirm `F4`, `F9`, `F11`, and `Escape` now fire through engine-owned actions rather than hardcoded key branches.

Manual spot checks:

- Action lookup with an unknown name (`action_map.is_pressed(&input, "dance")`) returns `false` without panic.
- `bindings("jump")` returns the expected slice for HUD/debug consumption.
- `replace_bindings` at runtime (call from a one-shot debug system) rebinds immediately.

## Done-when checks

- `tungsten_core::input::ActionMap` exists with the documented public surface and is re-exported at `tungsten::ActionMap`.
- `App::new` inserts an `ActionMap` resource for every app; startup log indicates whether `input.json` was loaded or defaults were used.
- Default map covers every action currently consumed by `examples/01_platformer` so the example runs identically with no `input.json`.
- Optional workspace-root `input.json` overrides default bindings per action; missing actions inherit defaults; unknown action names query `false` without panic.
- Invalid `input.json` at startup is fatal with a clear error (mirrors `Config::load`). Invalid `input.json` on hot reload logs an error and preserves the previous map.
- Hot reload of `input.json` is wired through the existing `notify` watcher; editing and saving the file rebinds actions within one debounced frame.
- `examples/01_platformer` reads gameplay input through action lookups only; engine-reserved controls (`F4`/`F9`/`F11`/`Escape`) also route through action lookups.
- `KeyCode`, `MouseButton`, and scroll-direction strings round-trip cleanly for every currently shipped variant, including extra mouse buttons in JSON.
- `InputState` exposes cursor position, per-frame cursor delta, wheel line delta, and wheel pixel delta to game code.
- `ActionMap::bindings` returns a borrowed slice (no allocation on the query path); micro-bench records ≤ 1 µs per query for both keyboard and mouse paths.
- `cargo test --workspace` passes.
- `./scripts/smoke-examples.sh` passes.
- `docs/LLM_INDEX.md`, `docs/DECISION_INDEX.md`, `docs/plans/Phase3.md`, and `DECISIONS.md` (`D-045`) match the shipped shape.

## Open Questions

- `input.json` location: workspace root (chosen here for parity with `tungsten.json`) vs `assets/input.json` (would piggyback on existing recursive watch without plumbing changes). Chosen: root; revisit if the watcher change in step 5 turns out to be more invasive than expected.
- Whether to accept both `"code": "KeyA"` and a shorter alias (`"a"`). Plan ships only canonical `KeyCode` variant names; aliases can be added post-M19 with zero migration risk.
- Whether `MouseButton::Other(u16)` should be representable in JSON (e.g. `{ "kind": "mouse", "other": 4 }`). Plan says no for now; `Other` exists mainly as a winit escape hatch.
- Whether `ActionMap` should live at `tungsten-core` root (`ActionMap`) or nested (`input::ActionMap`). Plan uses `input::ActionMap` with root re-export for ergonomic imports; safe to move later since the re-export is the stable path.
- Whether per-example local `input.json` merging is worth it (parallel to per-example asset manifests). Plan scopes M19 to a single root file; per-example overrides deferred.
- Whether the default-map builder should key actions by `&'static str` instead of `String` (perf / allocation footprint). Plan uses `String` for `HashMap<String, _>` simplicity; revisit if bench step 8 shows allocation in default-map construction.
- Whether M19 should also add a tiny debug-HUD row listing the active binding for a named action, to help developers visually confirm rebinds. Plan defers this to M21 debug-tooling; `ActionMap::bindings` is the data hook.

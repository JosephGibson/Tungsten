# Phase 6 milestone 54: world labels and the gamepad (W1 WL with W5a) — done

- **status:** done
- **goal:** W1 WL: world-anchored labels for damage numbers and name tags. A core `WorldLabels` resource that games push to each frame; the umbrella's extract projects each label through the camera, culls it and caps the count; the overlay draws them as one text batch below the paint list, through the content-keyed layout cache (`D-085`), each label scaled and placed as a whole (`D-126`). Example 02's name tags move onto it. W5a: gamepad play through the action map on `gilrs` (`D-015` rule 1). Pad buttons and stick directions become `input.json` binding kinds, with an analog `ActionMap::value`. The `ui_*` actions and their navigation repeat take the d-pad and the sticks, and the template ships gamepad defaults.
- **non-goals:** Per-glyph effects (the T2 backlog row, `D-126`); binding-aware prompts, prewarm and the rebind capture mode (W1 M4); per-player action maps for local multiplayer, analog trigger values, rumble and in-game pad remapping (backlog rows, step 7); moving the root `input.json` and examples 01, 03 and 04 onto pads (W12b); a pooled, allocation-free label push (backlog row, Q21); IME and text fields.
- **files to touch:** step 1: `crates/tungsten-core/src/text.rs`, `crates/tungsten-core/src/tests/text.rs`, `crates/tungsten-core/src/camera.rs`, `crates/tungsten-core/src/tests/camera.rs`, `crates/tungsten-core/src/lib.rs`, `crates/tungsten-render/src/text.rs`, `crates/tungsten-render/src/lib.rs`, `crates/tungsten/src/extract.rs`, `crates/tungsten/src/tests/extract.rs`, `crates/tungsten/src/app.rs`, `crates/tungsten/src/testing.rs`, `crates/tungsten/src/telemetry.rs`. Step 2: `crates/tungsten-render/src/text/engine.rs`, `crates/tungsten-render/src/text/gpu.rs`, `crates/tungsten-render/src/renderer.rs`, `crates/tungsten-render/src/tests/text.rs`, `crates/tungsten-render/src/tests/text_batches.rs`, `crates/tungsten-render/AGENTS.md`, `crates/tungsten/src/app.rs`, `crates/tungsten/src/telemetry.rs`, `crates/tungsten/src/debug_hud.rs`, `crates/tungsten/src/tests/debug_hud.rs`, `examples/05_ui_lab/src/main.rs`, `examples/05_ui_lab/src/screens.rs`, `examples/05_ui_lab/tests/ui_regression.rs`, `examples/05_ui_lab/tests/fixtures/ui-labels.png` (new), `DECISIONS.md`, `docs/DECISION_INDEX.md`. Step 3: `examples/02_bench/src/integrated.rs`, `examples/02_bench/src/integrated/systems.rs`, `examples/02_bench/src/integrated/runtime.rs`, `docs/perf/benchmarks.md`. Step 4: `crates/tungsten-core/src/input.rs`, `crates/tungsten-core/src/input/key_serde.rs`, `crates/tungsten-core/src/input/action_map.rs`, `crates/tungsten-core/src/ui/route.rs`, `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/tests/input.rs`, `crates/tungsten-core/src/tests/input/action_map.rs`, `crates/tungsten-core/src/tests/ui/route.rs`, `crates/tungsten/src/testing.rs`, `crates/tungsten/src/tests/testing.rs`. Step 5: `Cargo.toml`, `crates/tungsten/Cargo.toml`, `Cargo.lock`, `crates/tungsten/src/input_bridge.rs`, `crates/tungsten/src/tests/input_bridge.rs`, `crates/tungsten/src/app.rs`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, `AGENTS.md`, `DESIGN.md`, `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `docs/getting-started.md`, `scripts/release.py`, `licenses/clarifications.json`, `licenses/gilrs/SDL_GameControllerDB-LICENSE` (new). Step 6: `templates/basic/input.json`, `templates/basic/src/game.rs`, `templates/basic/tests/game.rs`, `templates/basic/README.md`, `docs/getting-started.md`. Step 7: `api/tungsten-core.txt`, `api/tungsten-render.txt`, `api/tungsten.txt`, `docs/plans/1.0/w04-api-freeze.md`, `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/plans/1.0/w01-ui-text-suite.md`, `docs/plans/1.0/w05-input.md`, `docs/plans/1.0/backlog-1.x.md`, `docs/known-issues.md`. Step 8: none in the tree (evidence under `perf-runs/`). Step 9: `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, `docs/plans/1.0/implementation-plan.md` (register row), `docs/plans/1.0/README.md` (owned Now fields), this plan (moved to `docs/plans/archive/1.0/`). Every step: this plan's evidence log.
- **ordered steps:** (1) World labels: the core resource, the camera's inverse and the extract. (2) World labels: the render batch, decision 161 and the UI lab's labels. (3) World labels: example 02's name tags. (4) Gamepad: core input, bindings and routing. (5) Gamepad: the `gilrs` backend, decision 162 and the build and notice records. (6) Gamepad: the template's defaults and the guide. (7) Records and API. (8) Perf sitting and gates. (9) Release 0.74.
- **done-when:** every step's evidence row quoted; workflow §5 closed (§8 below); the release checks pass after the cut, plus the gates by change that no step ran; the preflight prints the command block and the post-merge block.

Candidates W1 WL and W5a ([implementation plan](../../1.0/implementation-plan.md) §4 cards, both level B), one `D-108` pair since 0.66, milestone M54, release 0.74. Written 2026-10-14 at `83057a6` on branch `0.74`. Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs. Sources: [w01](../../1.0/w01-ui-text-suite.md) §10, §11 and §14 for W1 WL; [criteria](../../1.0/criteria.md) §8.1 for W5, which has not graduated ([w05](../../1.0/w05-input.md) is a skeleton).

## Context digest

The acceptance game needs damage numbers and gamepad play before W1 M4's playthrough ([acceptance game](../../1.0/acceptance-game.md) rows "Damage numbers", "Movement", "Gamepad play"). The glyph gate kept glyphon (T1b, `D-126`): world labels draw through the shared atlas as one batch in the overlay's lowest layer, and each label pops and scales as a whole through `TextArea.scale` and its position. The frame-loop gate placed W1 WL after M2 (`D-132`). M2 built the paint list, the overlay and the text batches under the deferred trim (`D-147`). M3 built routing on the raw stream, with the `ui_*` actions and navigation repeat for keys (`D-149`). M0b built the stream itself (`D-144`). The `D-085` layout cache is content-keyed and bounded, and it already shapes example 02's name tags as screen-space `TextSection`s that the bench projects by hand.

W5 is Must (Q9, `D-102`). The gamepad crate needs a `D-015` rule 1 decision after a check of its Linux backend for threads and `libudev`; 0.40 removed `libudev-dev` from both workflows. The audit (A13–A16) finds that every `gilrs` backend spawns one thread and that Linux links `libudev`. Decision 162 therefore amends the root rule of two background threads, and the workflows and the guide gain `libudev-dev`.

Later candidates expect from this one: W1 M4's settings screen and playthrough navigate on a gamepad, and its binding-aware prompts name pad buttons through `ActionMap::bindings`; W1 BC's sliders step on the d-pad; the acceptance game's top-down mover reads `ActionMap::value` from the left stick, and its damage numbers push `WorldLabel`s with bold crits from `sans_bold`.

## 1. Audit

Read at `83057a6`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | Example 02's name tags are screen-space `TextSection`s that the bench projects itself: `(position - camera.position) * camera.zoom + (-24, -WALKER_HALF.y - 28)`. Up to `tags` walkers nearest the view centre each get one, rewritten every frame, kept in `Runtime::tags` and appended to the HUD by the bench's own text extract | `examples/02_bench/src/integrated/systems.rs:451-491`, `examples/02_bench/src/integrated/runtime.rs:165`, `examples/02_bench/src/integrated.rs:424-433` |
| A2 | `integrated` is at `workload_version` 1 with digest `334b519e18b543d7`; its camera zoom is pinned at 1.0, so the tags' world offset and a screen offset agree; the `tags` knob runs 0–512 | `examples/02_bench/src/integrated.rs:53`, `:84`; `docs/perf/benchmarks.md:409`, `:456`; `examples/02_bench/src/integrated/systems.rs:419` |
| A3 | `CameraState` is anchored at the top-left and has `screen_to_world` (rotation included) but no inverse; the kit's `cursor_to_world` wraps it | `crates/tungsten-core/src/camera.rs:186-251`, `crates/tungsten-kit/src/cursor.rs:32` |
| A4 | A per-frame resource that games push to and the extract drains already exists: `DebugDraw` in core, drained by `drain_debug_draw` in the umbrella's extract stage | `crates/tungsten-core/src/debug_draw.rs:35`, `crates/tungsten/src/app.rs:1770` |
| A5 | The layout cache is keyed by content, font, size, line height, buffer size and layout; entries unused for more than 2 prepared frames go to a spare list of at most 256; one `prepared` list serves the screen-space sections only, and `update` advances the frame counter whenever that list changed | `crates/tungsten-render/src/text/engine.rs:51-83`, `:414-509` |
| A6 | `TextPipeline` has one renderer for the sections and one per UI batch; the trim rule forces a full frame on every changed frame while there are no batches, so `integrated` (no UI) trims the atlas every frame today | `crates/tungsten-render/src/text/gpu.rs:1-7`, `:102`, `:292-300`, `:403-503` |
| A7 | glyphon keys a rasterized glyph by its physical size, so a label whose `TextArea.scale` changes continuously rasterizes new glyphs each frame; `D-126`'s K sweep measured static sizes | `D-126` ("scale enters glyphon's cache key"), `D-127` |
| A8 | Per-frame render inputs outside `render_frame_full`'s arguments go through setters before it: `update_lights`, `update_mesh_particles`; the overlay draws the paint list's items, then the sections | `crates/tungsten/src/app.rs:1423-1426`; `crates/tungsten-render/AGENTS.md` (Frame order) |
| A9 | `FrameExtract` feeds the harness's `FrameDraw`, which is `#[non_exhaustive]`; `RenderCounts` (no `#[non_exhaustive]`) took M46's text counts, a ledger row; the HUD's `ui` row prints them | `crates/tungsten/src/app.rs:915-929`, `crates/tungsten/src/testing.rs:246-270`, `crates/tungsten/src/telemetry.rs:101-121`, `crates/tungsten/src/debug_hud.rs:212-229`, [ledger](../../1.0/w04-api-freeze.md#break-ledger) M46 row |
| A10 | The UI lab's fixtures are `default`, `empty` and `hidden`; its pixel tests capture frame 5 at 1280×720, and `ui-visual.png` is the reference; the root manifest has `mono`, `sans` and `sans_bold` faces | `examples/05_ui_lab/src/main.rs:27-51`, `examples/05_ui_lab/tests/ui_regression.rs:28-85`, `assets/manifest.json` |
| A11 | `Binding` has no `#[non_exhaustive]` and three kinds (key, mouse, scroll); `is_pressed`, `just_pressed` and `just_released` match it exhaustively; the umbrella's harness matches it too; `default_map` binds the `ui_*` actions to keys only | `crates/tungsten-core/src/input/action_map.rs:15-21`, `:86`, `:420-446`, `:507`; `crates/tungsten/src/testing.rs:139`, `:161` |
| A12 | `RawInput` and `InputDevice` are `#[non_exhaustive]`; `InputState` keeps frame and fixed edge views, and focus loss releases keys and buttons with edges in both (`D-144`); the router reads key presses only (`ui_actions`, the `presses` filter), its `Repeat` holds a `KeyCode`, and focus shows only after the keyboard | `crates/tungsten-core/src/input.rs:216`, `:338`, `:558`; `crates/tungsten-core/src/ui/route.rs:69`, `:106`, `:211-224`, `:285`, `:294`, `:622`, `:722` |
| A13 | `gilrs` 0.11.2 (Apache-2.0/MIT) on `gilrs-core` 0.6.8: on Linux the backend spawns one thread, `gilrs`, which watches a udev monitor (an inotify watcher under Flatpak or `GILRS_DISABLE_UDEV`, the two branches exclusive) and sends hotplug events by `std::sync::mpsc`; `next_event` is a zero-timeout epoll on the main thread. `Gilrs::new` also turns force feedback on, whose server spawns a second thread; `GilrsBuilder::with_force_feedback(false)` skips it (critique 1) | `gilrs-core-0.6.8/src/platform/linux/gamepad.rs:61-200`, `:212`; `gilrs-0.11.2/src/gamepad.rs:635-660`, `src/ff/server.rs:280-294` (registry copies) |
| A14 | The macOS (IOKit run loop) and Windows (WGI polling) backends each spawn one thread too | `gilrs-core-0.6.8/src/platform/macos/gamepad.rs:51-93`, `.../windows_wgi/gamepad.rs:16-34` |
| A15 | `libudev-sys` 0.1.4 runs `pkg_config::find_library("libudev")` at build time and links `udev`: Linux builds need `libudev-dev` (Debian/Ubuntu), and the binary needs `libudev.so.1` at run time. Both workflows install only `pkg-config libasound2-dev`, and release.yml's comment says no locked crate links udev; the archive README names ALSA, libxkbcommon and X11/Wayland | `libudev-sys-0.1.4/build.rs`, `.github/workflows/ci.yml:51-54`, `.github/workflows/release.yml:133-136`, `scripts/release.py:358`, `docs/getting-started.md:9` |
| A16 | `gilrs` compiles in SDL_GameControllerDB (zlib, Sam Lantinga) through `include_str!`, while its metadata says Apache-2.0/MIT, the case `licenses/clarifications.json`'s `bundled` field exists for; Zlib is allowed; duplicates only warn; `windows` 0.62.2 and `inotify` 0.11.5 are locked already, inside `gilrs-core`'s ranges | `gilrs-0.11.2/build.rs`, `src/mapping/mod.rs:476`; `licenses/clarifications.json` (`sctk-adwaita`); `deny.toml:39`, `:50`; `Cargo.lock:1143`, `:3536` |
| A17 | The root rule allows two background threads, the `cpal` callback and the `notify` watcher; `D-131` says the two-thread rule stands; DESIGN's frame loop names two background paths and its rule-1 table four crates | `AGENTS.md:43`, `DECISIONS.md:1624`, `DESIGN.md:26`, `:58` |
| A18 | `App::run` starts the hot-reload watcher, then the event loop; the harness drives `run_frame` without `run`; each `RedrawRequested` runs one frame; `run_frame` calls `begin_frame` at its end, so input recorded between frames reaches the next frame's systems; captures and perf runs set `TUNGSTEN_SMOKE_FRAMES` | `crates/tungsten/src/app.rs:512-520`, `:1550-1593`, `:2261`; `scripts/bench.py:280` |
| A19 | The template reads its moves as digital pairs and binds keys only; its tests check that each move action has a binding | `templates/basic/src/game.rs:71-78`, `templates/basic/input.json`, `templates/basic/tests/game.rs:95-100` |
| A20 | Budgets: `docs/LLM_INDEX.md` 12,275 of 12,288 bytes, `crates/tungsten-render/AGENTS.md` 4,065 of 4,096, root `AGENTS.md` 5,416 of 6,144 | `just ctx`, `scripts/check-agent-context.py:22-26` |
| A21 | The pair shares two files: `tungsten/app.rs` (step 1's extract, step 5's frame-start poll) and `tungsten/testing.rs` (step 1's `FrameDraw` field, step 4's `press_action` arms). `D-108` asks for separate files; implementation plan §6 judged them disjoint | `docs/plans/1.0/implementation-plan.md:218`; Q22 |

## 2. Design

### 2.1 World labels (steps 1–3)

Core (`tungsten_core::text`, re-exported at the crate root):

```rust
/// One label for this frame: text placed at a world point (`D-16[1]`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct WorldLabel {
    pub text: String,       // equal strings share one layout (`D-085`)
    pub anchor: Vec2,       // world units
    pub offset: Vec2,       // UI units after projection, y down; (0, 0)
    pub pivot: Vec2,        // the point of the label's box at anchor + offset; (0.5, 1.0), centred above
    pub font_id: String,    // a face or family ID
    pub font_size: f32,     // UI units
    pub line_height: f32,   // 0 lays out at 1.25 × font_size, as TextSection
    pub color: [u8; 4],     // sRGB; [255; 4]
    pub scale: f32,         // the label's own pop; 1.0
}
impl WorldLabel {
    pub fn new(text: impl Into<String>, anchor: Vec2, font_id: impl Into<String>, font_size: f32) -> Self;
    // with_offset, with_pivot, with_line_height, with_color, with_scale
}
/// This frame's labels (Q15): pushed during the frame, drained by the extract.
#[derive(Debug)]
pub struct WorldLabels { /* labels, cap (512), cull_margin (64.0 UI units) */ }
impl WorldLabels { pub fn push(&mut self, label: WorldLabel); len; is_empty; clear;
                   pub fn drain(&mut self) -> impl Iterator<Item = WorldLabel> + '_;
                   cap; set_cap; cull_margin; set_cull_margin }
```

`CameraState::world_to_screen(world: Vec2) -> Vec2` is the inverse of `screen_to_world`, rotation included, in physical pixels from the window's top-left.

Render (`tungsten_render::text::WorldLabelRun`, re-exported beside `TextSection`): `#[non_exhaustive]`, `Default`, public fields `content`, `font_id`, `font_size`, `line_height`, `position: [f32; 2]` (the pivot's physical pixel), `pivot: [f32; 2]`, `scale` and `color`. Plain data; the umbrella builds it from `default()` and field assignments.

Extract (`tungsten::extract::extract_world_labels`, crate-private, called from `stage_extract` after `paint_ui`). It reads the frame's viewport: the surface's size with a renderer, else `WindowSize`, the size the view projection uses. For each label: drain, project `anchor` through `world_to_screen`, then add `offset × ScaleFactor::get()`. Cull a label whose projected point lies outside the viewport grown by `cull_margin × ScaleFactor`. Keep the first `cap` survivors in push order, so the latest pushes drop. Set the run's scale to `label.scale × ScaleFactor::get()`, snapped to the nearest 1/8 (`LABEL_SCALE_STEP`, Q17, A7). With `UiDebug::hide_game_layers` set, the extract drains and draws nothing (Q18). The drain runs every frame, drawn or not, so a harness frame empties the resource too. Its counts go to three new `RenderCounts` fields, `world_labels`, `world_labels_culled` and `world_labels_dropped`. The runs go to `FrameExtract`/`FrameDraw::world_labels`. `App::new` inserts `WorldLabels::default()`.

Render path (step 2). `TextEngine` gains a second prepared group for the labels, beside the sections, sharing the one layout map, spare list and frame counter. The counter advances once per frame in which either group changed, so aging stays per prepared frame (amends `D-085`'s count). A label lays out unbounded on one line under its layout key, and its `TextArea` takes `scale = run.scale`, `left = position.x − width × scale × pivot.x` and `top = position.y − height × scale × pivot.y` (width and height from the buffer's layout runs, the bounds the viewport). `TextPipeline` gains `labels_renderer` with its drawable flag. When non-empty, the label batch counts as one batch for `TrimPolicy::decide` (Q19). It prepares on every full frame and whenever its signature (the runs and the viewport) changed. A prepare that returns `AtlasFull` leaves it undrawn and forces the next frame full, as for any batch. `Renderer::update_world_labels(&[WorldLabelRun])` keeps a copy for the next render, as `update_mesh_particles` does. The overlay pass draws the labels first, then the paint list's items, then the sections, so the paint list's game layers cover labels (amends `D-147`'s order). An empty label list prepares and draws nothing, so frames without labels keep their pixels. `TextStats` gains `labels` (drawn), `label_glyphs` (glyphs in the label areas this frame's prepare took, 0 when not prepared) and `layouts` (layout map entries). They reach `RenderCounts::text_label_glyphs` and `RenderCounts::text_layouts`. The HUD's `ui` row appends `labels {kept}/{culled}c/{dropped}d lg {glyphs} lay {layouts}`.

UI lab (step 2). `TUNGSTEN_UI_LAB_FIXTURE=labels` builds the default lab and pushes a fixed set of labels every frame:

- five on screen, at scales 1.0, 1.25 and 1.5, in three colours;
- one crit in `sans_bold`;
- one anchored under the lab's window, so the panel covers part of it;
- one anchored off the view, so it is culled.

The interactive run (`Off`) adds the damage numbers. Every 0.25 s a number spawns near a fixed world point. It floats 40 UI units up over 0.8 s, pops from scale 1.6 to 1.0 over its first 0.15 s, and fades out; every fourth number is a crit in `sans_bold`, gold. The numbers live in a `Vec` resource the lab owns, ticked on `Time::delta`.

Name tags (step 3). `name_tags` pushes one `WorldLabel` per tag with these values:

- `anchor`: position + (0, −`WALKER_HALF.y`); `pivot` (0, 0);
- `offset` (−24, −28) ÷ `ScaleFactor::get()` and `scale` 1 ÷ `ScaleFactor::get()`, so the tags keep the benchmark's physical sizing at any OS or UI scale (the extract multiplies both back, and the snap makes the scale exactly 1);
- font `mono` at size 11 with line height 15, colour `TAG_COLOR`.

At zoom 1 this reproduces A1's pixel position and size. `Runtime::tags` goes, and `extract_text` returns the HUD alone. `workload_version` stays 1 (Q20). `IntegratedCounts` gains three `bench:` counters from `RenderCounts`: `labels`, `label_glyphs` and `text_layouts`.

### 2.2 Gamepad (steps 4–6)

Core (`tungsten_core::input`, re-exported at the crate root):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GamepadId(u32);                 // GamepadId::new(u32), get()
#[non_exhaustive] #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadButton { South, East, North, West, LeftShoulder, RightShoulder,
    LeftTrigger, RightTrigger, Select, Start, Mode, LeftStick, RightStick,
    DPadUp, DPadDown, DPadLeft, DPadRight }  // input.json names = variant names (Q10)
#[non_exhaustive] #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadAxis { LeftStickX, LeftStickY, RightStickX, RightStickY } // x right, y up
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AxisDirection { Negative, Positive }            // "negative" / "positive"
pub const STICK_PRESS: f32 = 0.5;  pub const STICK_RELEASE: f32 = 0.35;   // Q8
```

`RawInput` gains four variants (additive):

- `GamepadButton { pad, button, pressed }`;
- `GamepadAxis { pad, axis, value }`, where a move of one pad's axis coalesces into the last record when that record is the same pad's axis and no threshold was crossed by either; a record a crossing produced keeps its value, and the next move starts a new record, so a release or press crossing always reaches the routed view (critique 2.1);
- `GamepadStick { pad, axis, direction, pressed }`, recorded right before the axis record whose value crossed a threshold, so the router decides a stick's owner before it meets the value; an axis move that crosses a threshold never coalesces into the record before it;
- `Gamepad { pad, connected }`.

`InputState` gains mutators: `gamepad_connected(pad)`, `gamepad_disconnected(pad)` (releases that pad's buttons and stick directions with edges in both views and zeroes its axes), `gamepad_button_down`/`_up(pad, button)` and `set_gamepad_axis(pad, axis, value)`. A direction holds while the value along it is at least `STICK_PRESS` and releases below `STICK_RELEASE`.

It gains "any pad" queries for the action map (Q5):

- `is_gamepad_pressed(button)`, `gamepad_just_pressed`/`_released(button)`;
- `is_stick_pressed(axis, dir)`, `stick_just_pressed`/`_released(axis, dir)`;
- `gamepad_axis(axis)`, the largest-magnitude value over the pads.

Per-pad queries are `gamepads()`, `is_gamepad_pressed_on(pad, button)` and `gamepad_axis_on(pad, axis)`. Edges keep both views (`D-146`). Focus loss releases pad buttons and stick directions as it releases keys, with release edges and no records, and zeroes every axis, so `ActionMap::value` reads 0 too; a stick still held when focus returns reads 0 until its next move (Q6).

`Binding` turns `#[non_exhaustive]` and gains `GamepadButton { button }` (`"kind": "gamepad_button"`) and `GamepadAxis { axis, direction }` (`"kind": "gamepad_axis"`). An axis binding is held while its stick direction is pressed. `ActionMap::value(input, action) -> f32` returns the largest of the action's bindings. A held digital binding gives 1.0. An axis binding gives its value along its direction, rescaled from `STICK_RELEASE..1` to `0..1` and 0 below. `default_map` adds pads to the `ui_*` actions (Q11):

| Action | Pad bindings |
| --- | --- |
| `ui_accept` | South |
| `ui_cancel` | East |
| `ui_up`, `ui_down`, `ui_left`, `ui_right` | The d-pad, and the left stick in that direction |
| `ui_next` | RightShoulder |
| `ui_prev` | LeftShoulder |

Router. An internal `NavInput` (`Key(KeyCode)`, `Pad(GamepadId, GamepadButton)`, `Stick(GamepadId, GamepadAxis, AxisDirection)`) replaces `KeyCode` in the presses filter, `Repeat` and the UI's held list, so each pad's presses and releases keep their owner. The `ui_actions` lookup matches the binding without the pad. A pad press or stick press that the UI takes:

- moves focus, activates or cancels as a key does;
- repeats a held direction at the same delay and interval, for the d-pad or the stick;
- withholds its release from the routed view.

When the UI takes a stick direction, the router feeds the routed view that pad's axis at 0, then withholds the axis's `GamepadAxis` records until the direction's release record. It feeds them again from the axis record after that release, so `RoutedInput` reads the axis as 0 while the UI holds it, with no stick edge. The routed view derives its own stick edges from the axis values it is fed, so `feed` skips `GamepadStick` records. `InputDevice` gains `Gamepad`, and focus shows after the keyboard or a pad (`set_focus_visible(device != Pointer)`).

Harness: `press_action`/`release_action` press a pad binding on `GamepadId::new(0)`. An axis binding sets the axis to ±1.0, and its release sets 0.

Backend (step 5, crate-private in `tungsten/input_bridge.rs`): `GamepadBackend` wraps `gilrs::Gilrs` built by `GilrsBuilder::new().with_force_feedback(false).build()`. The default filters (deadzone, jitter, d-pad axes to buttons) stay on, and force feedback stays off: its server would spawn a second thread (A13). `App::run` starts it after the watcher, never `App::new`, so the harness spawns no thread. It reports the pads connected at start through `gamepad_connected` and logs `gamepad backend started: N connected` at info. A failed build logs one warning and runs without pads. `run_frame` drains it first, through `apply_pad_event(input, pad, PadEvent)`, a GPU-free function over a crate-private `PadEvent` the backend translates `gilrs` events into, so tests need no device. Button and axis events are dropped while `InputState::focused()` is false (Q6). Connection and disconnection always apply, so `gamepads()` stays true across a focus change. In smoke mode every event is dropped (Q7). Within a frame, pad events follow the window events that arrived before it (Q13). `gilrs` triggers map to `LeftTrigger`/`RightTrigger` buttons (Q9), and unmapped buttons and axes are skipped. The `gamepad` cargo feature on `tungsten` (default on, Q4) gates the dependency and the backend; without it, `run_frame` has nothing to drain.

Template (step 6): `input.json` binds the four moves to the left stick and the d-pad beside the keys, and `toggle_mute` to North. The axis helper reads `actions.value(...)`, so a stick moves the player in proportion and keys still give 1.0.

### 2.3 The seam

Core holds `WorldLabel(s)`, the pad types and their bindings, with no `gilrs` or render types (`D-007`). Render holds `WorldLabelRun` and the batch, and never reads the `World`. The umbrella projects at extract on the main thread with `&World` (`D-018`) and owns the backend. The new background thread lives inside `gilrs`, and its events reach the main thread through `gilrs`'s own `mpsc` (decision 162).

## 3. Steps

### Step 1: World labels: the core resource, the camera's inverse and the extract

- **Files:** step 1's list in the header
- **Change:** §2.1's core types with rustdoc, `CameraState::world_to_screen`, `WorldLabelRun`, `extract_world_labels` with projection, cull, cap, scale snap, hide and drain; the three `RenderCounts` fields; `FrameExtract`/`FrameDraw::world_labels`; the resource in `App::new`. `stage_render` does not use the runs yet. Tests: `world_to_screen_inverts_screen_to_world` (core camera, with a rotation), `world_label_builders_set_their_fields` (core text), and in `tungsten/tests/extract.rs` on the harness `world_labels_project_through_the_camera`, `world_labels_cull_outside_the_view`, `world_labels_cap_drops_the_latest`, `world_labels_last_one_frame`, `hidden_ui_hides_world_labels`, `label_scale_snaps_to_eighths`.
- **Done-when:** `cargo test -p tungsten-core --locked --lib -- camera:: text::` → ok, 0 failed; `cargo test -p tungsten --locked --lib -- extract::` → ok, 0 failed; `rg -c 'fn (world_labels_project_through_the_camera|world_labels_cull_outside_the_view|world_labels_cap_drops_the_latest|world_labels_last_one_frame|hidden_ui_hides_world_labels|label_scale_snaps_to_eighths)\b' crates/tungsten/src/tests/extract.rs` → 6; `rg -c 'fn world_to_screen_inverts_screen_to_world\b' crates/tungsten-core/src/tests/camera.rs` → 1; `just lint` → no warnings
- **Moves:** none

### Step 2: World labels: the render batch, decision 161 and the UI lab's labels

- **Files:** step 2's list in the header
- **Change:** in this order:
  1. Write decision 161 with the tungsten-decision skill, its index row, and the marker lines on `D-085` and `D-147`.
  2. Build §2.1's render path: the engine's label group, the pipeline's renderer under the trim rule, `Renderer::update_world_labels`, the overlay order and `TextStats`. `stage_render` calls `update_world_labels`, then copies the two new counts into `RenderCounts`, and the HUD row follows.
  3. Edit the render crate's `AGENTS.md` frame-order sentence to read "draws the world labels, the UI paint list, each item under its scissor, then the text", within its 4,096-byte budget (A20).
  4. Add GPU-free tests:
     - in `render/tests/text.rs`: `label_layouts_plateau_under_unique_text` (1,000 prepared frames of 512 labels with unique text: map entries ≤ 3 × 512 after every frame, spare ≤ 256, and the maximum over frames 501–1,000 not above that over frames 1–500), `labels_share_layouts_with_sections` and `label_pivot_places_the_box`;
     - in `render/tests/text_batches.rs`: `labels_count_as_a_batch`, a frame with labels and no UI batch deferring its trim;
     - in `tungsten/tests/debug_hud.rs`: the `ui` row's text.
  5. Build the UI lab's `labels` fixture and interactive damage numbers (§2.1).
  6. Add two pixel tests to `ui_regression.rs`. `ui_labels_match_fixture` compares the `labels` fixture with `ui-labels.png` at tolerance 0. `direct_and_capture_paths_draw_the_same_image_with_labels` runs no AA, SMAA high and the vignette.
  7. Make the reference: capture the `labels` fixture twice through the test's command, check the two with `cmp`, view one, then copy it to `tests/fixtures/ui-labels.png`.
- **Done-when:**
  - `rg -c '^## D-16[1] — ' DECISIONS.md` → 1;
  - ``rg -c '^\| `D-16[1]` \|' docs/DECISION_INDEX.md`` → 1;
  - `rg -c 'Amended by D-16[1]' DECISIONS.md` → 2;
  - `cargo test -p tungsten-render --locked --lib -- text` → ok, 0 failed;
  - `rg -c 'fn (label_layouts_plateau_under_unique_text|labels_share_layouts_with_sections|label_pivot_places_the_box)\b' crates/tungsten-render/src/tests/text.rs` → 3;
  - `rg -c 'fn labels_count_as_a_batch\b' crates/tungsten-render/src/tests/text_batches.rs` → 1;
  - `cargo test -p tungsten --locked --lib -- debug_hud::` → ok;
  - the two reference captures → `cmp` exit 0, and the evidence row describes the viewed image: five labels drawn, the one under the window covered by the panel, the culled one absent;
  - `TUNGSTEN_VISUAL_REGRESSION=1 cargo test -p example-05-ui-lab --locked --test ui_regression` → 6 passed;
  - `just visual` → passes, so `gpu-visual.png`, the post and transition regressions and `ui-visual.png` are unchanged;
  - `just smoke` → every row passes;
  - `just ctx` → OK;
  - `just lint` → no warnings.
- **Moves:** none; `ui-labels.png` is new

### Step 3: World labels: example 02's name tags

- **Depends on:** steps 1 and 2 (without the batch, the tags would vanish)
- **Files:** step 3's list in the header
- **Change:** §2.1's name tags, divided by `ScaleFactor` so they keep their physical size; the three `bench:` counters; the module doc's stale sentence about the 360-frame text cache goes; benchmarks.md's `integrated` section: the Text bullet (tags as world labels), the counters, and a dated note that `workload_version` stays 1 (Q20)
- **Done-when:** `cargo test -p example-02-bench --locked` → ok; `rg -c 'TextSection' examples/02_bench/src/integrated/runtime.rs` → 1 (the HUD's field alone); `just perf run integrated --preset min --allow-background` → exit 0 and a valid capture; its raw `bench:` lines read `labels` between 1 and 4 (the preset's `tags`) and `text_layouts` ≤ 3 × (`labels` + 3) in every measured frame; `rg -n 'workload_version: 1' examples/02_bench/src/integrated.rs` → 1 line
- **Moves:** none (step 8 checks the digest and the pixels)

### Step 4: Gamepad: core input, bindings and routing

- **Files:** step 4's list in the header
- **Change:** §2.2's core types, `RawInput` variants, `InputState` mutators and queries, `key_serde` names, `Binding` kinds with `#[non_exhaustive]`, `ActionMap::value`, `binding_has_name` arms, `default_map`'s pad bindings, the router's `NavInput`, pad repeat, stick withholding and `InputDevice::Gamepad`; the harness's pad arms. Every new public item gets rustdoc. Tests:
  - `core/tests/input.rs`: `gamepad_button_edges_in_both_views`, `stick_directions_press_with_hysteresis`, `focus_loss_releases_gamepad_input` (a stick held at 1.0, focus lost: buttons, directions, `gamepad_axis` and `ActionMap::value` all read 0), `disconnect_releases_the_pads_input`, `adjacent_axis_moves_coalesce` (and a crossing move does not, and a move after a crossing starts a new record);
  - `core/tests/input/action_map.rs`: `gamepad_bindings_round_trip_through_json`, `value_reads_a_sticks_magnitude`, `default_map_binds_ui_actions_on_a_gamepad`;
  - `core/tests/ui/route.rs`: `dpad_moves_focus_and_south_activates`, `a_held_stick_repeats_navigation`, `a_stick_the_ui_took_reads_zero_when_routed` (one stick over six frames at 0.2, 0.4, 0.6, 0.8, 0.3 and 0.1 with a focusable widget shown: `RoutedInput` reads 0.2, 0.4, 0, 0, 0.3 and 0.1, with no stick edge), `gamepad_navigation_shows_focus`, `two_pads_keep_their_own_presses` (pad A holds South in gameplay, a modal opens, pad B's South goes to the UI, A releases: `RoutedInput` sees A's release), `same_frame_crossings_reach_the_routed_view` (the UI takes neither: 0.8 → 0.3 → 0.4 in one frame releases the routed direction, 0.2 → 0.6 → 0.45 presses it, matching the raw view);
  - `tungsten/tests/testing.rs`: `press_action_presses_a_gamepad_binding`.
- **Done-when:**
  - `cargo test -p tungsten-core --locked --lib -- input ui::route` → ok, 0 failed;
  - `rg -c 'fn (gamepad_button_edges_in_both_views|stick_directions_press_with_hysteresis|focus_loss_releases_gamepad_input|disconnect_releases_the_pads_input|adjacent_axis_moves_coalesce)\b' crates/tungsten-core/src/tests/input.rs` → 5;
  - `rg -c 'fn (gamepad_bindings_round_trip_through_json|value_reads_a_sticks_magnitude|default_map_binds_ui_actions_on_a_gamepad)\b' crates/tungsten-core/src/tests/input/action_map.rs` → 3;
  - `rg -c 'fn (dpad_moves_focus_and_south_activates|a_held_stick_repeats_navigation|a_stick_the_ui_took_reads_zero_when_routed|gamepad_navigation_shows_focus|two_pads_keep_their_own_presses|same_frame_crossings_reach_the_routed_view)\b' crates/tungsten-core/src/tests/ui/route.rs` → 6;
  - `cargo test -p tungsten --locked --lib -- testing::` → ok;
  - `cargo check --workspace --all-targets --locked` → ok;
  - `just lint` → no warnings.
- **Moves:** none (`InputState` reads the same for every key, button and wheel input)

### Step 5: Gamepad: the `gilrs` backend, decision 162 and the build and notice records

- **Depends on:** step 4
- **Files:** step 5's list in the header
- **Change:** in this order:
  1. Reread A13–A16 at the versions `Cargo.lock` takes, over the whole build path the backend calls, the wrapper's included. A second thread with force feedback off, or a build need beyond `pkg-config` and `libudev-dev`, is a stop condition.
  2. Write decision 162 with the tungsten-decision skill, its index row, and the marker lines on `D-045`, `D-131` (its two-thread clause), `D-144` and `D-149`.
  3. Add `gilrs = "0.11"` (its default features, WGI on Windows) as a workspace dependency and an optional dependency of `tungsten` behind `gamepad = ["dep:gilrs"]`, in `default` beside `kit`.
  4. Build §2.2's backend, `apply_pad_event` and the frame-start drain. Tests in `tungsten/tests/input_bridge.rs`: `pad_events_reach_input_state`, `pad_events_drop_while_unfocused`, `pad_events_drop_in_smoke_mode`, `unmapped_pad_events_are_skipped`.
  5. Edit the records:
     - `AGENTS.md`'s threads rule names three threads, the third `gilrs`'s backend thread (events via `std::sync::mpsc`), within its budget;
     - DESIGN.md's rule-1 table gains `gilrs`, and its frame-loop sentence names three background paths;
     - both workflows install `libudev-dev`, and release.yml's comment loses "no locked crate links udev";
     - the guide's Linux line adds `libudev-dev`;
     - `scripts/release.py`'s Linux README line adds libudev;
     - `licenses/clarifications.json` gains a `gilrs` 0.11.2 entry whose `bundled` names SDL_GameControllerDB under Zlib, with its text in `licenses/gilrs/SDL_GameControllerDB-LICENSE`, taken from the package's `SDL_GameControllerDB/LICENSE` at the commit its `.cargo_vcs_info.json` names.
- **Done-when:**
  - `rg -c '^## D-16[2] — ' DECISIONS.md` → 1;
  - ``rg -c '^\| `D-16[2]` \|' docs/DECISION_INDEX.md`` → 1;
  - `rg -c 'Amended by D-16[2]' DECISIONS.md` → 4;
  - `cargo tree -p tungsten -e normal -i libudev-sys --locked` → one path, through `gilrs-core` and `gilrs`;
  - `cargo tree -p tungsten --no-default-features -e normal --prefix none --locked | rg -c '^gilrs '` → 0;
  - `cargo check -p tungsten --no-default-features --locked` → ok;
  - `cargo test -p tungsten --locked --lib -- input_bridge::` → ok, and `rg -c 'fn (pad_events_reach_input_state|pad_events_drop_while_unfocused|pad_events_drop_in_smoke_mode|unmapped_pad_events_are_skipped)\b' crates/tungsten/src/tests/input_bridge.rs` → 4 (the unfocused test also connects and disconnects a pad while unfocused and reads `gamepads()` after);
  - `rg -c 'with_force_feedback\(false\)' crates/tungsten/src/input_bridge.rs` → 1;
  - `just deps` → passes;
  - `just notices` → passes for both targets, and `rg -c 'SDL_GameControllerDB' target/notices/x86_64-unknown-linux-gnu/THIRD-PARTY-NOTICES.txt` → at least 1;
  - `rg -c 'libudev-dev' .github/workflows/ci.yml .github/workflows/release.yml docs/getting-started.md` → 1 each;
  - `rg -c 'gilrs' AGENTS.md` → 1;
  - `just script-test` → passes;
  - `just ctx` → OK;
  - `TUNGSTEN_SMOKE_FRAMES=30 cargo run -p example-05-ui-lab --locked 2>&1 | rg -c 'gamepad backend started'` → 1;
  - `just smoke` → every row passes;
  - `just lint` → no warnings.
- **Moves:** none

### Step 6: Gamepad: the template's defaults and the guide

- **Depends on:** step 4 (the harness presses pad bindings without a backend)
- **Files:** step 6's list in the header
- **Change:** §2.2's template bindings and its axis helper on `ActionMap::value`. The template test's bindings check also asserts a pad binding per move. A new test, `a_gamepad_stick_moves_the_player`, presses the left stick's binding through the harness and sees the player move right. The README's and the guide's "arrow keys or WASD" lines add the left stick and the d-pad.
- **Done-when:** `cargo test -p tungsten-template-basic --locked` → ok, 0 failed; `rg -c 'fn a_gamepad_stick_moves_the_player\b' templates/basic/tests/game.rs` → 1; `rg -c '"kind": "gamepad_axis"' templates/basic/input.json` → 4; `rg -c 'actions.value' templates/basic/src/game.rs` → at least 1
- **Moves:** none (`just template-check` runs in the release chain, A19)

### Step 7: Records and API

- **Files:** step 7's list in the header
- **Change:**
  - `just api`, then review the snapshot diff. Ledger rows:
    - `Binding` turns `#[non_exhaustive]` and gains two kinds (a break: the workspace's two exhaustive matches were step 4's);
    - `RenderCounts` gains five fields and `TextStats` three (breaks for struct literals, as M46's row);
    - additions for the freeze: the pad types, the `RawInput` and `InputDevice` variants, the `InputState` methods, `ActionMap::value`, `WorldLabel(s)`, `world_to_screen`, `WorldLabelRun`, `update_world_labels`, `FrameDraw::world_labels`, the `gamepad` feature, and `input.json`'s binding kinds (additive under `D-103`);
    - behaviour changes: the engine's `ui_*` pad defaults, focus shown after pad navigation, and the overlay drawing labels below the paint list.
  - DESIGN.md gains "### World Labels and Gamepad — M54".
  - `docs/LLM_INDEX.md`: the input-state row gains `gamepad`, and the neutral-text and text-engine rows gain world labels, with wording trimmed elsewhere in the file to stay within 12,288 bytes (A20). No new rows: every new item lives in a file the index already routes.
  - w01 §11's WL row and §14's world-anchored paragraph gain "Landed in 0.74 (M54)" with decision 161. w05's Placement records W5a as landed with decision 162.
  - The backlog gains four rows: per-player action maps, analog triggers, rumble, and a pooled label push.
  - Known issues gains one entry: a pad without an SDL mapping reads only what `gilrs`'s fallback maps.
- **Done-when:**
  - `just api-check` → passes;
  - `git diff --unified=0 -- api/ | rg -c '^-[^-]'` → the removed lines are the edited signatures that the ledger rows name, each listed in the evidence row;
  - `rg -c 'M54' docs/plans/1.0/w04-api-freeze.md` → at least 4;
  - `rg -c '^### World Labels and Gamepad — M54$' DESIGN.md` → 1;
  - `just ctx` → OK;
  - `just repo-check` → OK.
- **Moves:** none

### Step 8: Perf sitting and gates

- **Depends on:** the code steps that ran; a skipped step's code is simply absent from the candidate
- **Files:** none in the tree; evidence in `perf-runs/<YYYYMMDD>-m54-labels-gamepad/` with its scripts and README
- **Change:** the milestone sitting recipe (memory: base by `git archive HEAD` under `target/m54-base`, prebuilt release binaries with the runner's flags, told apart by `sha256`, one background job after 60 s with no `nxcodec.bin`). The step touches the frame loop (`run_frame`'s drain, the extract), `InputState` and the router, which every row runs, so it captures every CPU row.
  1. Run four suites in order: base-a, cand-a, base-b, cand-b, each `just perf suite --repeat 5`.
  2. Run the §10 world-anchored scenario as `just perf run integrated --set tags=512 --repeat 5`, once on the base and once on the candidate.
  3. Capture `TUNGSTEN_BENCH=integrated` frame 60 at 1920×1080 twice from the base binary and once from the candidate.
  4. Copy the base suites out of `target/m54-base` with `cp -a`, check each copy by `diff -rq`'s exit code, and remove the base tree.
  5. Run the gates on the candidate: `just smoke` and `just visual`.
- **Done-when:**
  - **A/A.** base-a → base-b: 0 `regressed` and 0 `improved` owned metrics, except the known drift metrics, each of which gets one rerun of its row's pair: `particles` `particle_count_refresh` p50/p95, `animate_sprites` p95 and `unattributed` p50/p95; `gpu` and `gpu-throughput` `render_encode` p50/p95; `gpu` `extract` p95.
  - **Direct pairs.** base-a → cand-a and base-b → cand-b: 0 `regressed` owned metrics on all eight rows, under the same one-rerun rule; first-run digests match 8 of 8 in each pair, `integrated`'s `334b519e18b543d7` among them.
  - **Scenario pair.** base → cand: `integrated` `total` p50, p95, p99 and jitter are not `regressed`. In each candidate run, read from its raw `bench:` lines, every frame's `text_layouts` ≤ 3 × (`labels` + 3), and the maximum of `text_layouts` over the last 150 measured frames is not above that over the first 150. The means of `labels` and `label_glyphs` are reported.
  - **Frame capture.** The two base captures → `cmp` exit 0, the control. The candidate capture against the first base capture → `cmp` exit 0. If they differ, the step views both and accepts the difference only where a tag overlaps a HUD line, whose draw order the overlay swapped, and records the pixel count. If the control differs, the comparison is recorded as not run.
  - **Gates.** `just smoke` → every row passes; `just visual` → passes.
- **Moves:** none

### Step 9: Release 0.74

- **Files:** `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row, the README's four owned Now fields (`roadmap.py summary`), this plan (moved to `docs/plans/archive/1.0/`)
- **Change:** close workflow §5 and this step's evidence row with `status: done`. Then follow [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)):
  1. Archive the plan.
  2. Run `just release-cut 0.74.0` once.
  3. Run every release check once after it. The gates by change that no step ran are already in the chain: `just template-check` for step 6, and `just notices` and `just api-check`. `just deps` ran in step 5 and `just visual` in steps 2 and 8.
  4. Run the preflight with `--message`.
- **Done-when:** [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut); `just release-preflight 0.74.0 --repo JosephGibson/Tungsten --message 'Update 0.74: M54 world labels and gamepad'` → passes and prints the command block and the post-merge block
- **Ends with:** the changed-file list; the owner-only check, which is a gamepad on the reference machine:
  - the template moves on the left stick, in proportion, and on the d-pad, and North mutes;
  - the UI lab's popup navigates on the d-pad and a held stick, with repeat, accepts on South and cancels on East;
  - the interactive lab's damage numbers pop and float;
  - unplugging and replugging the pad keeps it working.
  Then the command block, the post-merge block, and the plan prompt for W1 BC, the next register row.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Which gamepad crate? | `gilrs` 0.11.2 on `gilrs-core` 0.6.8, WGI on Windows. Rejected: owning evdev, XInput and IOKit backends; SDL2, a C library to ship |
| Q2 | Its backend thread? | Amend the rule to three named threads (162): every `gilrs` backend spawns one (A13, A14), and Q9 made the gamepad Must |
| Q3 | `libudev` on Linux? | Accept: `libudev-dev` to build, `libudev.so.1` at run time, recorded in the workflows, the guide and the archive README |
| Q4 | Can a game leave the pad out? | A default `gamepad` feature on `tungsten`; without it, no `gilrs`, no thread and no `libudev` |
| Q5 | Several pads? | Actions read any pad; `InputState` keeps per-pad state and queries; per-player action maps go to the backlog |
| Q6 | An unfocused window? | Button and axis events drop while unfocused, while connections still apply; focus loss releases pad buttons and stick directions as it releases keys (`D-144`) and zeroes the axes |
| Q7 | Smoke mode? | The backend starts, so smoke runs its creation, and its events drop, so captures never read a pad |
| Q8 | Stick thresholds? | Press at 0.5, release below 0.35, after `gilrs`'s default deadzone and jitter filters; `value` rescales above 0.35 |
| Q9 | Triggers and rumble? | Triggers are digital buttons for 1.0; force feedback stays off, so its thread never starts (A13); analog triggers and rumble are backlog rows |
| Q10 | Button names in `input.json`? | Positional variant names (`South`, `East`, `North`, `West`, `DPadUp`, …), as `gilrs` and the W3C standard layout name positions, so one file serves every pad family |
| Q11 | The engine's `ui_*` pad defaults? | South accept, East cancel, d-pad and left stick directions, RightShoulder next, LeftShoulder prev |
| Q12 | The template's and the examples' bindings? | The template binds moves to the left stick and the d-pad and mute to North, and reads `value`; the root `input.json` waits for W12b |
| Q13 | Pad events' order against window events? | Drained at frame start, so within a frame they follow the window events that arrived since the last; recorded in 162 |
| Q14 | Where `WorldLabels` lives? | Core's text module, beside the neutral text types, as `DebugDraw` lives in core (A4), so the kit can push labels; the extract in the umbrella |
| Q15 | A label's lifetime? | Immediate mode: pushed each frame, drained by every extract |
| Q16 | Cap and cull margin? | Cap 512, the `tags` knob's maximum; the latest pushes drop beyond it, counted; margin 64 UI units around the viewport |
| Q17 | A continuously animated scale? | Snapped to 1/8 steps of the physical scale, so a pop rasterizes a few sizes (A7) |
| Q18 | Does hide-UI hide labels? | Yes: they draw in the overlay's lowest game layer |
| Q19 | Does the label batch count under the trim rule? | Yes, as one batch when non-empty, so `integrated` trims every K changed frames instead of every frame (A6) |
| Q20 | Does `integrated`'s `workload_version` move? | No: same tags, contents, anchors, pixel positions and sizes at zoom 1, the bench dividing out `ScaleFactor` (critique 6), drawn by a new engine path; step 8's frame capture and digests check it |
| Q21 | Allocation per label? | `push` takes an owned `String`; a pooled push is a backlog row with M4's allocation-free values |
| Q22 | The pair shares `tungsten/app.rs` and `tungsten/testing.rs` (A21) | Keep the pair: the steps run in order in one session, and each restores only its own changes from the copies taken before it |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`).

## 5. Decisions expected

- 161 (step 2): W1 WL landed: `WorldLabels` in core, the extract's projection, cull, cap and scale snap, and one label batch below the paint list on the shared layout cache, counted under the trim rule. Example 02's tags move onto it at `workload_version` 1. It extends `D-126` and amends `D-085` (the frame count across groups) and `D-147` (the overlay order).
- 162 (step 5): W5a landed: `gilrs` under `D-015` rule 1, its backend thread the third background thread with force feedback off, `libudev` on Linux, gamepad input in core, the `gamepad_button` and `gamepad_axis` binding kinds, `ActionMap::value`, routing and repeat from pads, the `gamepad` feature. It amends `D-045`, `D-131` (its two-thread clause), `D-144` and `D-149`.

These are the next free IDs at `83057a6`, in the order the steps write them. Steps 2 and 5 check again before writing, since another session may take one mid-run; if one is taken, the step takes the next free ID and its greps follow.

## 6. Invariants

- **May move:** none. `ui-labels.png` is a new reference, not a moved one.
- **Must not move:** the determinism hash, the pinned containment hash, the row digests (`integrated` `334b519e18b543d7` among them), `gpu-visual.png`, the post and transition regressions, `ui-visual.png`, and the schedule snapshots: the `DefaultPlugins` snapshot and the template's and examples': no step adds a system.

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with steps that don't depend on it. Step 2 depends on step 1, step 3 on steps 1 and 2, steps 5 and 6 on step 4.
- Step 5's reread finds a second thread per `Gilrs` or a build need beyond `pkg-config` and `libudev-dev`, or `just deps` refuses a new crate: skip step 5 and record the finding. The release still runs, and the register row says the backend did not land.
- The two reference captures of step 2 differ: drop `ui_labels_match_fixture` and the reference, keep the direct/capture test (then 5 passed), and record a follow-up. The rest of the step stands.
- Step 8 reads an owned metric `regressed` after its one rerun: the release does not run in this session. The report quotes the verdict and ends with the `release` prompt for a release session, after the owner reads it (workflow §3).
- `just ctx` fails after a step's edits: shorten wording inside the files the step names; never raise a budget.
- A file outside the header's list needs a change: stop that step, record it, restore its files.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1–8, release | `just lint` per code step; `just smoke` in 2, 5 and 8; `just visual` in 2 and 8; `just deps`, `just notices` and `just script-test` in 5; every CPU row in 8; the release chain runs `just check`, `just smoke`, `just script-test`, `just notices`, `just api-check` and `just template-check` once, after the cut |
| 3 Invariants | 2, 8 | `just visual` and the digests; no snapshot of the schedule moves |
| 4 Decisions | 2, 5 | 161 and 162, each before the docs that cite it (AGENTS.md and DESIGN.md in 5; the render AGENTS.md in 2; DESIGN's section and the workstream files in 7) |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1, 2, 4, 7 | Rustdoc with each item; the snapshot and the ledger rows in 7 |
| 6 Template and guide | 6, 5 | The template's bindings and analog moves; the guide's prerequisites (5) and controls (6) |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 7, 5, 2 | No new routed file; index keywords in 7, AGENTS.md rules in 5 and 2 within their budgets |
| 8 Design | 5, 7 | DESIGN's table and frame-loop sentence (5); the M54 section (7) |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 7, release | The gap log has no rows (n/a); w01 and w05 in 7; the register row and `python3 -B scripts/roadmap.py summary --write` at the release |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

Round 1: the `critique` skill with `--repo`, 2026-10-14, at `83057a6` with this plan in the tree, §9 and the evidence log cut by `sed`. Model `gpt-6-astra`, effort xhigh, context 872,000, output cap 4,000; 261.4 s; tokens in 903,296 (cached 800,896), out 10,994 (reasoning 6,087); 28 commands, output 314.6 KB (largest 46.6 KB); usage 5h 49% → 73%, week 8% → 11%. Six findings, all accepted; they changed done-when checks in steps 4 and 5, so round 2 runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] `Gilrs::new()` starts the second thread that step 5 forbids: force feedback is on by default and its server spawns a thread | `gilrs-0.11.2/src/gamepad.rs:635-660` (`force_feedback: true`), `src/ff/server.rs:280-294` (spawns `gilrs` unless disabled); every spawn in both crates listed by `rg`: with it off, one per platform, Linux's two branches exclusive (`gilrs-core-0.6.8/src/platform/linux/gamepad.rs:112-139`) | accepted | A13 corrected; §2.2's backend builds with `GilrsBuilder::new().with_force_feedback(false)`; step 5 rereads the whole build path and greps the call; Q9 |
| 2 | [HIGH] The router loses the pad identity needed to deliver releases: pad A's gameplay release matches pad B's UI-owned press of the same button | `crates/tungsten-core/src/ui/route.rs:675-685` (release ownership keyed by the input alone) | accepted | §2.2's `NavInput` carries the `GamepadId`; step 4 adds `two_pads_keep_their_own_presses` (route count 4 → 5) |
| 3 | [HIGH] Focus loss leaves analog movement active: axes keep their value, so `ActionMap::value` still reads it, and the release that would clear it drops while unfocused | §2.2 as written (axes zeroed only on disconnect); `templates/basic/src/game.rs:65-78` | accepted | Focus loss zeroes every axis (§2.2, Q6); `focus_loss_releases_gamepad_input` holds a stick at 1.0 and reads 0 everywhere after |
| 4 | [HIGH] Withholding axis records does not zero the retained routed state, and the crossing axis record came before the stick record | `crates/tungsten-core/src/ui/route.rs:243-247` (a persistent `RoutedInput`, edges cleared by `begin_frame`), `crates/tungsten-core/src/input.rs:411-428` (held state kept) | accepted | §2.2: the stick record precedes the crossing axis record and never coalesces; on taking a stick the router feeds the routed axis 0, withholds until the release record, then feeds again; `feed` skips stick records; the routed test walks six frames across both thresholds |
| 5 | [MEDIUM] Dropping every event while unfocused drops connections, so `gamepads()` goes stale | §2.2 as written | accepted | Connection and disconnection always apply; only button and axis events drop (§2.2, Q6); the unfocused test checks `gamepads()` |
| 6 | [MEDIUM] Name-tag equivalence also needs a scale factor of 1: the extract multiplies offsets and scale by `ScaleFactor::get()` | `crates/tungsten-core/src/display.rs:118` (OS × UI scale), `examples/02_bench/src/integrated/systems.rs:490`; nothing logs the scale factor | accepted | The bench divides its offset and scale by `ScaleFactor`, so the tags keep their physical size at any scale (§2.1, step 3, Q20) |

Round 2: the same skill on the revised plan, 2026-10-14, at `83057a6`, §9 and the evidence log cut by `sed`, with a `--focus-file` naming round 1's changes (no rejections). It was queued behind the 5-hour window's reset (73% at 22:40Z, reset 23:08Z). Model `gpt-6-astra`, effort xhigh, context 872,000, output cap 4,000; 188.5 s; tokens in 795,498 (cached 717,696), out 7,338 (reasoning 4,094); 19 commands, output 205 KB (largest 22 KB); usage 5h 0% → 19%, week 11% → 14%. One finding, accepted. No third round (skill step 7); nothing is left open.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [HIGH] A later axis sample can overwrite a stick's release crossing: pointer-style coalescing turns 0.8 → 0.3 → 0.4 into a release record followed by an axis record at 0.4, so the routed view, which derives edges from values, never sees a value below 0.35 and stays held | `crates/tungsten-core/src/input.rs:514-533` (a move overwrites the last `PointerMoved` record's value); §2.2 as revised in round 1 | accepted | §2.2: a crossing's record keeps its value and the next move starts a new record; step 4's coalescing test covers it, and `same_frame_crossings_reach_the_routed_view` walks a same-frame release-and-rebound and a press-and-retreat (route count 5 → 6) |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-14 | pass | `cargo test -p tungsten-core --locked --lib -- camera:: text::` → 24 passed, 0 failed (`world_to_screen_inverts_screen_to_world`, `world_label_builders_set_their_fields` among them); `cargo test -p tungsten --locked --lib -- extract::` → 67 passed, 0 failed, the six label tests among them; the two `rg -c` checks → 6 and 1; `just lint` → exit 0, no warnings. A label whose drawn scale snaps to 0 counts as culled | `crates/tungsten-core/src/text.rs`, `crates/tungsten/src/extract.rs` |
| 2 | 2026-10-14 | pass | D-161 written (161 was free): heading → 1, index row → 1, `Amended by D-161` → 2 (`D-085`, `D-147`); `cargo test -p tungsten-render --locked --lib -- text` → 69 passed, 0 failed (plateau test 4.2 s); the two `rg -c` checks → 3 and 1; `debug_hud::` → 12 passed; the reference captures → `cmp` exit 0; viewed: `12`, `250`, `Lantern`, `7` (×1.5) and `+30` drawn, `99!` gold in `sans_bold`, `under 5` cut at the window's left edge by its panel, `culled` absent (the capture runs at this machine's OS scale 1.75; a first placement put two labels under the popup and was moved); `ui_regression` with `TUNGSTEN_VISUAL_REGRESSION=1` → 6 passed; `just visual` → exit 0 (4, 5, 1 and 6 passed); `just smoke` → exit 0, every row OK; `just ctx` → exit 0, render `AGENTS.md` 4,092 of 4,096 bytes; `just lint` → exit 0 after two test helpers moved to struct literals (`field_reassign_with_default`). The lab registers its label systems only under `labels` and the interactive run, so the other fixtures keep their schedule | `examples/05_ui_lab/tests/fixtures/ui-labels.png`, `crates/tungsten-render/src/text/engine.rs`, `crates/tungsten-render/src/text/gpu.rs` |
| 3 | 2026-10-14 | pass | `cargo test -p example-02-bench --locked` → ok (7 and 4 passed); `rg -c 'TextSection' …/runtime.rs` → 1; `just perf run integrated --preset min --allow-background` → exit 0, valid; raw `bench:` lines over 299 measured frames: `labels` 4 in every frame, `text_layouts` 17–21 (≤ 21), last-150 max 21 = first-150 max 21; `rg -n 'workload_version: 1'` → line 53. Deviation from §2.1: the anchor is the walker's centre and the offset (−24, −38) ÷ `ScaleFactor`, the same projected point as the plan's head anchor with (−24, −28) but the hand projection's float operations, so the tags land on the same bits (the round trip ÷ then × the scale factor is exact at 1.0–3.0 except 1.3). Plan defect found: the digest hashes the `bench:` text, so the three counters move `integrated`'s digest by construction; a default run (`perf-runs/20261015T003250Z-integrated`) reads digest `6ae63eae31b75278`, and the same frames with the three counters stripped hash to `334b519e18b543d7`, the documented digest: the work is unchanged. benchmarks.md records both | `perf-runs/20261015T003215Z-integrated-min/`, `perf-runs/20261015T003250Z-integrated/` |
| 4 | 2026-10-14 | pass | `cargo test -p tungsten-core --locked --lib -- input ui::route` → 94 passed, 0 failed; the three `rg -c` checks → 5, 3 and 6; `cargo test -p tungsten --locked --lib -- testing::` → 44 passed, `press_action_presses_a_gamepad_binding` among them; `cargo check --workspace --all-targets --locked` → exit 0; `just lint` → exit 0; the core, kit, render and umbrella suites → all ok (core 929 passed). Three existing action-map tests assumed `ui_prev` unbound and were kept to their intent: the key bindings lead the `ui_*` lists, a disabled action is set to `[]` explicitly, and the user layer's same-as-shipped entry is now LeftShoulder | `crates/tungsten-core/src/input.rs`, `crates/tungsten-core/src/ui/route.rs` |
| 5 | 2026-10-14 | skipped | Reread passed: the lock takes `gilrs` 0.11.2 on `gilrs-core` 0.6.8 and `libudev-sys` 0.1.4; with force feedback off one `gilrs` thread on Linux (the udev and inotify branches exclusive, `gilrs-core-0.6.8/src/platform/linux/gamepad.rs:75-138`, `:180`), the ff server skipped (`gilrs-0.11.2/src/ff/server.rs:279-283`); the build path needs `pkg-config` and libudev only. Decision 162, the dependency, the backend, its four tests (15 passed), the records and the SDL clarification were written, and `just deps` passed. `just notices` failed: "no license text matches an alternative of 2 crate(s) … gilrs 0.11.2 (Apache-2.0/MIT; files: none); gilrs-core 0.6.8 (Apache-2.0/MIT; files: none)": neither package ships a licence file, so both need `texts` entries with licence files under `licenses/` that the header's list does not name. §7 (a done-when check fails; a file outside the list needs a change): the step's files were restored with `cp` and confirmed with `cmp`, `licenses/gilrs/` removed, decision 162 not written; `cargo check --workspace --all-targets --locked` → exit 0 after. The step's diff is kept for a re-land | `perf-runs/20261014-m54-labels-gamepad/step5-skipped.patch`, `…/step5-notices.log` |
| 6 | 2026-10-14 | pass, the README and guide lines left | `cargo test -p tungsten-template-basic --locked` → exit 0, 11 passed, 0 failed, `a_gamepad_stick_moves_the_player` among them (the stick binding moved first in the test's map, since the harness presses an action's first binding); `rg -c 'fn a_gamepad_stick_moves_the_player\b'` → 1; `rg -c '"kind": "gamepad_axis"' templates/basic/input.json` → 4; `rg -c 'actions.value' templates/basic/src/game.rs` → 1; `toggle_mute` also binds North. With step 5 skipped no window reads a pad, so the template README's and the guide's "arrow keys or WASD" lines stay as they are rather than promise a stick that does nothing in 0.74; the re-land adds them | `templates/basic/input.json`, `templates/basic/src/game.rs` |
| 7 | 2026-10-14 | pass | `just api` → exit 0: `api/tungsten-core.txt` +446, `api/tungsten-render.txt` +58, `api/tungsten.txt` +11 lines, `api/tungsten-kit.txt` unchanged; `git diff --unified=0 -- api/ \| rg -c '^-[^-]'` → 3, each the `Binding` enum line at its three paths (`tungsten_core::Binding`, `::input::Binding`, `::input::action_map::Binding`), now `#[non_exhaustive]`, the ledger's first M54 row; `just api-check` → exit 0; `rg -c 'M54' w04-api-freeze.md` → 5 (five ledger rows); `rg -c '^### World Labels and Gamepad — M54$' DESIGN.md` → 1; `just ctx` → exit 0, `docs/LLM_INDEX.md` at 12,288 of 12,288 bytes (the UI paint-list row drops `render/text/gpu.rs`, which the text row lists, and the UI lab row its "(M46)"); `just repo-check` → exit 0 after this plan's evidence stopped naming the unwritten decision by ID. With step 5 skipped: w05 records W5a's core half as landed and the backend not; known issues gains the P2 row for the missing backend rather than the SDL-mapping note, which waits for the backend; the ledger leaves the `gamepad` feature out | `api/`, `docs/plans/1.0/w04-api-freeze.md`, `docs/known-issues.md` |
| 8 | 2026-10-14 | one check failed as written; every other check passed | Sitting 01:06-01:23 UTC after NoMachine's encoder stopped; base `ccde498d` against candidate `48fb977e`, every suite and run valid first time. **A/A** base-a → base-b: one owned verdict, `particles` `particle_count_refresh` p95 0.788 → 0.924 `regressed` (τ 0.039; base-a 0.73–0.84, base-b 0.80–1.00), a listed drift metric; its one rerun (`--only particles`, twice) reads 0 `regressed`, 0 `improved` (p95 0.836 → 0.816 `noisy`). **Direct pairs:** 0 `regressed` owned metrics on all eight rows in both (direct-a's one `improved` is `gpu-throughput` `render_encode` p95 2.48 → 1.78, the two-valued metric; `noisy` above τ: direct-a `ecs` `follow` p50 1.584 → 1.738, `particles` `particle_count_refresh` p50 0.620 → 0.648 and p95 0.788 → 0.842, direct-b `churn` `flush` p95 2.726 → 2.894). First-run digests match **7 of 8** in each pair, the check as written failing: `integrated` reads `334b519e18b543d7` → `6ae63eae31b75278`, the step 3 counters' text (the follow-up); with them stripped the candidate's run 1 hashes to `334b519e18b543d7` in both pairs. **Scenario** (`tags=512`): `total` p50 9.068 → 9.098, p95 9.522 → 9.588, p99 10.016 → 9.918, jitter 0.948 → 0.820, all `unchanged`; in each of the five candidate runs no frame has `text_layouts` above 3 × (`labels` + 3) (505–584 for 179–200 labels), last-150 max 568 ≤ first-150 max 584; means `labels` 187.4, `label_glyphs` 2,809.3; stripped digest `334b519e18b543d7`. **Frame capture:** base-1 = base-2 (`cmp` exit 0), candidate = base-1 (`cmp` exit 0). **Gates:** `just smoke` → exit 0, every row passes; `just visual` → exit 0 (4, 5, 1 and 6 passed). No owned metric reads `regressed` after its rerun, so §7 lets the release run; the base tree was deleted | `perf-runs/20261014-m54-labels-gamepad/` (`summary.md`, `README.md`) |
| 9 | 2026-10-14 | closed; cut and checks in the report | Workflow §5 closed: invariants: `just visual` passed in steps 2 and 8 (`gpu-visual.png`, the post and transition regressions and `ui-visual.png` unchanged), no step added a scheduled system, the determinism and containment hashes run with the release checks (`just physics-release`); `integrated`'s raw digest moved with step 3's counters, its work did not (step 8). `DESIGN.md`: the M54 section (step 7) and the status line. Records: the register row (release 0.74, the backend not landed), the README's four owned Now fields from `roadmap.py summary`, one `CHANGELOG.md` `[Unreleased]` entry, decision 161 with its index row; this plan archived before the cut. The release checks, the cut and the preflight are quoted in the session's report, not here | `CHANGELOG.md`, `DESIGN.md`, `docs/plans/1.0/implementation-plan.md` |

## Follow-ups

- Step 5 (the `gilrs` backend) did not land: `gilrs` 0.11.2 and `gilrs-core` 0.6.8 ship no licence file, so `just notices` needs `licenses/gilrs/LICENSE-APACHE` and `licenses/gilrs/LICENSE-MIT`, fetched from `gitlab.com/gilrs-project/gilrs` at `07e286e24b046cf39e5c367daa2770b805a64692` (both packages' `.cargo_vcs_info.json`), and a `texts` entry for each crate in `licenses/clarifications.json`, beside the planned SDL_GameControllerDB entry. The step's full diff (decision 162 as the next free ID, the backend and tests, the records) is `perf-runs/20261014-m54-labels-gamepad/step5-skipped.patch` (machine-local), made against the tree before step 5; a re-land plan adds those two files to its list. Until then the gamepad types, bindings, routing and the template's pad bindings (steps 4 and 6) work through `InputState`'s mutators and the harness, and no window reads a pad.
- Step 8's "first-run digests match 8 of 8, `integrated`'s `334b519e18b543d7` among them" cannot hold as written: the runner's digest hashes each frame's `bench:` text, which step 3's three counters change. Step 8 reports the raw comparison and checks the work by the digest with those counters stripped (step 3's evidence row).

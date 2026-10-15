# Phase 6 milestone 57: engine debug views (W1 M5) — done

- **status:** done
- **goal:** The engine's HUD, system timing overlay and entity inspector become read-only, movable windows on the core UI, with scroll panels, saved window positions, world picking blocked under any UI, a log console on the engine logger, a frame-time graph drawn by a new mesh paint command, and a picked-widget panel (W1 M5's card; [w01](../../1.0/w01-ui-text-suite.md) §11 M5 and §14).
- **non-goals:** Editing in the inspector, a command console, virtualization, horizontal scrolling, dragging a scrollbar, resizing or raising windows, docking ([w01](../../1.0/w01-ui-text-suite.md) §11 "Later", [backlog](../../1.0/backlog-1.x.md)); physics debug's controls in the UI, which no card holds (follow-up); the examples' move onto the UI (W12b with W1 M6); a new dependency or Cargo feature; any change to a benchmark's work.
- **files to touch:** Paths under `core/`, `render/` and `umbrella/` mean `crates/tungsten-core/src/`, `crates/tungsten-render/src/` and `crates/tungsten/src/`. Step 1: core `ui/node.rs`, `ui/tree.rs`, `ui/theme.rs`, `ui/paint.rs`, `ui/mod.rs`, `ui/layout/taffy.rs`, `tests/ui/paint.rs`; render `ui.rs`, `ui.wgsl`, `renderer.rs`, `tests/ui.rs`, `tests/text_batches.rs` (added by the owner during the run). Step 2: core `ui/scroll.rs` (new), `ui/node.rs`, `ui/mod.rs`, `ui/layout.rs`, `ui/layout/taffy.rs`, `ui/paint.rs`, `ui/hit.rs`, `ui/route.rs`, `ui/event.rs`, `ui/focus.rs`, `tests/ui/scroll.rs` (new); umbrella `testing/ui.rs`, `tests/ui_route.rs`. Step 3: core `ui/window.rs` (new), `ui/node.rs`, `ui/mod.rs`, `ui/route.rs`, `ui/event.rs`, `tests/ui/window.rs` (new); umbrella `tests/ui_route.rs`. Step 4: umbrella `debug_views.rs` (new), `debug_hud.rs`, `systems_overlay.rs`, `inspector.rs`, `plugins.rs`, `app.rs`, `lib.rs`, `engine_font.rs`, `asset_loader/mod.rs`, `asset_loader/reload.rs`, `tests/debug_views.rs` (new), `tests/debug_hud.rs`, `tests/systems_overlay.rs`, `tests/inspector.rs`, `tests/plugins.rs`, `tests/engine_font.rs`, `tests/asset_loader.rs`; `templates/basic/tests/game.rs`; `examples/01_platformer/src/tests/main.rs`, `examples/03_scene_state/src/main.rs`, `examples/04_shader_playground/src/main.rs`, `examples/05_ui_lab/src/main.rs`; `scripts/smoke-examples.sh`, `scripts/test-smoke-examples.sh`. Step 5: umbrella `logging.rs`, `debug_views.rs`, `plugins.rs`, `app.rs`, `lib.rs`, `tests/logging.rs`, `tests/debug_views.rs`, `tests/plugins.rs`; core `schedule.rs`, `input/action_map.rs`, `ui/tree.rs`, `ui/mod.rs`, `tests/input/action_map.rs`, `tests/ui/tree.rs`; `input.json`; `templates/basic/tests/game.rs`; `examples/01_platformer/src/tests/main.rs`, `examples/03_scene_state/src/main.rs`, `examples/04_shader_playground/src/main.rs`, `examples/05_ui_lab/src/main.rs`. Step 6: `perf-runs/<UTC date>-m57-debug-views/` (untracked evidence) and scratch trees under `target/m57-*` (deleted before step 7). Step 7: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/known-issues.md`, `docs/perf/benchmarks.md`, `docs/plans/1.0/w01-ui-text-suite.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/plans/1.0/backlog-1.x.md`, `api/tungsten-core.txt`, `api/tungsten.txt`, `api/tungsten-render.txt`. Step 8: the skeleton's release files.
- **ordered steps:** (1) The mesh paint command. (2) Scroll panels. (3) Movable windows and the placements file. (4) The HUD, timing overlay and inspector as windows, the frame-time graph, the inspector's pick on the routed input, the UI lab's `debug` fixture and its smoke row. (5) The log console, the picked-widget panel, F7 and F8. (6) The suite sitting and the gates. (7) Records. (8) Release 0.77.
- **done-when:** Every step's evidence row quoted; workflow §5 closed; the release checks, plus the gates no step ran, pass after the cut; the preflight prints the command block and the post-merge block.

Candidate W1 M5 ([implementation plan](../../1.0/implementation-plan.md) §4 card, level B), milestone M57, release 0.77. Written 2026-10-15 at `04fca16`. Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs.

## Context digest

W1's ladder runs M4 → M5 → M6; M4 shipped in 0.76 (M56), and W12b with W1 M6 waits on M5 to move examples 01, 03 and 04 onto the UI. M5 is the last foundation milestone of the UI suite: it turns the three text-section overlays (`D-044`, `D-047`) into windows on the core UI and adds what those windows need.

It stands on the paint list and overlay pass (`D-147`, `D-148`), input routing (`D-149`), the closed control kinds and paint-only properties (`D-162`, `D-163`), the engine logger whose `Tee` already reserves the console's channel sink (`D-119`), the engine font (`D-123`) and the user folder's settings files (`D-159`).

The card names two decisions: the mesh paint command and the window-placement file (§5: 165 and 166). Its checks beyond `just check` are the perf suite's rows not regressed, the HUD's toggles and defaults preserved, and smoke. Level B: the plan settles the design (§2) and the run is xhigh with no step 1 API critique (`D-150`, `D-151`).

Three audit findings shape the plan: UI labels cannot name the engine font today (A17), the inspector's pick runs before routing and picks through UI (A6), and the LLM index has 4 bytes of headroom (A24). Later work expects public scroll panels, meshes and drag handles in core (M6, the acceptance game) and a debug view whose cost C1 can row.

## 1. Audit

Read at `04fca16`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | `DebugHud` builds outlined `TextSection`s each frame it is shown: rows padded by a monospace heuristic (`MONO_ADVANCE_RATIO` 0.65), four offset copies for the outline, a cache re-emitted between 500 ms refreshes. Defaults: off, `TopRight`, `engine_mono` 26/30, colour `[240, 240, 240, 240]`, outline `[0, 0, 0, 220]` at 1.0, padding 10, refresh 500 ms, EWMA α 0.1 | `umbrella/debug_hud.rs:10-11`, `:61-91`, `:239-356`, `:334-353` |
| A2 | The timing overlay has the same shape: rows `{name:>30}` sorted by EWMA; at a top corner it stacks under the HUD through `rendered_height_px`. Defaults: off, `BottomRight`, 28/32, outline 1.5, padding 12 | `umbrella/systems_overlay.rs:31-50`, `:84-186`, `:142-150`; `umbrella/debug_hud.rs:98-109` |
| A3 | The inspector has the same shape: an `entity` line, then three padded columns; `mouse 3 to pick` with nothing selected; refresh on the interval, a viewport change or a selection change. Defaults: off, `BottomLeft`, 28/32 | `umbrella/inspector.rs:41-63`, `:261-329`, `:345-395` |
| A4 | `stage_extract` composes the three into the frame's `text`, which the overlay draws after every UI item, so they sit above every UI root; the UI layout runs just before the extract | `umbrella/app.rs:1365-1391`, `:1637-1640`; `render/renderer.rs:1986-2016` |
| A5 | The refresh throttles add the previous frame's work time (`prev_total_ms`), not its interval (known issue P3), and the HUD's fps divides by the CPU frame time (open finding) | `umbrella/app.rs:1617-1621`; `umbrella/debug_hud.rs:252-259`; `docs/known-issues.md:42`; `docs/perf/benchmarks.md:518` |
| A6 | `inspector_pick` reads the raw `InputState`. `DebugPlugin` registers first, so it runs before `ui_route`, and a middle click on a UI root also picks the entity behind it | `umbrella/inspector.rs:142-184`; `umbrella/plugins.rs:53-77`, `:233-244`; `umbrella/tests/plugins.rs:10` |
| A7 | Six copies pin `DefaultPlugins`' `pre_update` order: five tests and `DESIGN.md`. Example 01's copy ends `, platformer_bindings` | `umbrella/tests/plugins.rs:10`; `templates/basic/tests/game.rs:73`; `examples/01_platformer/src/tests/main.rs:317`; `examples/03_scene_state/src/main.rs:360`; `examples/04_shader_playground/src/main.rs:1297`; `DESIGN.md:393` |
| A8 | The paint list has `Rect`, `Image` and `Text` commands. `PaintCommand` is `Copy` and `#[non_exhaustive]`; `PaintList` has public fields, built by literal in a core test and a render test | `core/ui/paint.rs:196-298`; `core/tests/ui/paint.rs:99-100`; `render/tests/ui.rs:58-59` |
| A9 | The overlay has one instanced unit-quad SDF pipeline. `push_primitive` uploads nothing for a command it does not know, and the overlay loop matches `DrawItem` exhaustively | `render/ui.rs:169-180`, `:271-311`, `:437-471`; `render/renderer.rs:1990-2009` |
| A10 | Core matches `WidgetKind` exhaustively in four places | `core/ui/node.rs:52-80`; `core/ui/theme.rs:44-51`; `core/ui/tree.rs:462-466`; `core/ui/paint.rs:731-807` |
| A11 | Nothing scrolls. `clip_children` clips paint and hits; an `Auto` main size never shrinks, and no Taffy `overflow` is set, so a panel cannot be smaller than its content. Taffy 0.14 is built without `content_size`; its `Overflow::Hidden` gives a flex item an automatic minimum of 0 | `core/ui/node.rs:200-201`; `core/ui/paint.rs:811-825`; `core/ui/hit.rs:87-91`; `core/ui/layout/taffy.rs:122-131`, `:152-174`, `:254-301`; `Cargo.toml:62` |
| A12 | The wheel over UI is taken from the game and scrolls nothing; `pointer_over_ui` is the routing's end state | `core/ui/route.rs:540-549`, `:221-227`, `:1113-1116` |
| A13 | A primary press captures its target and the release activates it; there is no drag threshold or move. `UiRouteSettings` is `#[non_exhaustive]`, with a 4-unit double-click distance; M55 left the drag threshold to M5 | `core/ui/route.rs:120-145`, `:709-764`; `docs/plans/1.0/backlog-1.x.md:42` |
| A14 | `focus` on a widget that cannot take focus changes nothing, and every focus change goes through `set_focused`; nothing scrolls into view | `core/ui/focus.rs:214-217`, `:348` |
| A15 | `UiLayer::Debug` and `DebugPopup` exist above the game layers, and hide-UI (F6) keeps them painted and hit | `core/ui/node.rs:16-29`; `core/ui/paint.rs:632-638`; `core/ui/hit.rs:57-63` |
| A16 | The `Tee` is the logger's target only with a log file; stderr alone uses `Target::Stderr` with `is_test` in debug builds, which writes through `eprint!`. `D-119` and the `Tee`'s comment reserve the console's bounded `sync_channel` sink there | `umbrella/logging.rs:44-75`, `:196-219`; env_logger 0.11.11 `src/writer/buffer.rs:90`; `DESIGN.md:361` |
| A17 | UI labels resolve `family` only through the manifest's `font_families`. The engine font is a face ID registered in code; the root manifest declares `mono` and `sans`, the template's only `sans`. A label naming `engine_mono` falls back with a warning. The umbrella hands the families over at load and at reload | `render/text/engine.rs:391-404`; `umbrella/engine_font.rs:11-20`; `umbrella/asset_loader/mod.rs:80-91`, `:148`; `umbrella/asset_loader/reload.rs:553` |
| A18 | Engine defaults bind F1–F6, F9 and F11; nothing in the workspace binds F7 or F8. The root `input.json` lists nine engine actions; a core test lists the defaults | `core/input/action_map.rs:232-275`; `input.json:15-23`; `core/tests/input/action_map.rs:18-56` |
| A19 | The bench rows run `DebugPlugin` without `UiPlugin`, and the runner clears `TUNGSTEN_OVERLAYS_ON`, so every view stays off in every row while the debug plugin's systems run | `examples/02_bench/src/main.rs:81-87`; `scripts/bench.py:44` |
| A20 | Code switches the HUD on in the template (debug builds) and in examples 03 and 05 (interactive runs); no pixel fixture shows a debug view | `templates/basic/src/game.rs:29-31`; `examples/03_scene_state/src/main.rs:89-91`; `examples/05_ui_lab/src/main.rs:118-120` |
| A21 | `UiHarness` moves, clicks and presses keys but has no wheel. `InputState::add_scroll_line_delta` records a raw wheel event | `umbrella/testing/ui.rs:129-168`; `core/input.rs:791-798` |
| A22 | `persist::write_atomic` and the crate-private `quarantine` back `Settings`; `App::new` joins the settings folder | `core/persist.rs:55`, `:176-178`; `core/settings.rs:69-85`, `:234`; `umbrella/app.rs:226-231` |
| A23 | Existing tests exercise the compose functions or their sections: 12, 5, 22 and 2 tests in the four files. The second engine-font test reads text sections | `umbrella/tests/debug_hud.rs`; `umbrella/tests/systems_overlay.rs`; `umbrella/tests/inspector.rs`; `umbrella/tests/engine_font.rs:29-60` |
| A24 | `docs/LLM_INDEX.md` is 12,284 of 12,288 bytes. Smoke's template row forbids `Unknown font ID`; the UI screens row and the stub test's expected lines show how a row is added | `scripts/check-agent-context.py:25`; `scripts/smoke-examples.sh:145-152`, `:228-235`; `scripts/test-smoke-examples.sh:80-84`, `:109` |

## 2. Design

Every new public item gets rustdoc. Code comments cite `M57`, not the new decision IDs, which step 7 writes.

### 2.1 The mesh paint command

```rust
// tungsten_core::ui
pub enum WidgetKind { /* … */ Mesh }   // a leaf; role Image; the panel's kind style
#[non_exhaustive] #[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiVertex { pub position: Vec2, pub color: [u8; 4] }   // UiVertex::new(position, color)
impl UiTree {
    pub fn mesh(&mut self, parent: WidgetId, style: LayoutStyle) -> WidgetId;
    pub fn set_mesh(&mut self, id: WidgetId, vertices: &[UiVertex]) -> bool;
    pub fn mesh_vertices(&self, id: WidgetId) -> &[UiVertex];
}
pub enum PaintCommand { /* … */ Mesh { rect: Rect, first: u32, count: u32, clip: u32 } }
pub struct PaintList { pub scale: f32, pub clips: Vec<ClipRect>, pub commands: Vec<PaintCommand>, pub vertices: Vec<UiVertex> }
impl PaintList { pub fn mesh_vertices(&self, command: &PaintCommand) -> &[UiVertex]; }
```

- A widget's vertex positions are fractions of its box, 0 to 1 from its top-left corner; values outside place a vertex outside the box, where the clips cut it. Every three vertices make a triangle.
- `set_mesh` keeps the whole triangles whose six coordinates are finite, dropping a trailing partial one. It returns whether the stored mesh changed and marks paint only. A mesh has no natural size, so an `Auto` axis measures 0.
- Paint pushes the node's own box first, as for every kind. Then one `Mesh` command follows, its vertices appended to `PaintList::vertices` in physical pixels, mapped through the node's composed transform, with each colour's alpha times the node's alpha. A mesh with no triangle pushes nothing. The command's `rect` is the bounding box of its mapped vertices, not the widget's box, so the partition rule, which tests `command.rect()` against the batch's text (`core/ui/paint.rs:346`), sees every pixel a triangle outside the box can paint. The dump line is `Mesh (x, y) w×h verts N clip c`.
- Render: `DrawItem::Mesh { scissor, vertices }` sits in paint order and closes the open primitive run. A second pipeline in `ui.wgsl` (`vs_mesh`, `fs_mesh`) takes physical-pixel positions through the viewport projection and `Unorm8x4` sRGB colours decoded as the primitives' are, with straight alpha, no texture and no anti-aliasing. Its own vertex buffer grows and caps as the instance buffer does, cut at a whole triangle and warned once.

### 2.2 Scroll panels

```rust
impl UiTree {
    pub fn scroll_panel(&mut self, parent: WidgetId, style: LayoutStyle) -> WidgetId;
    pub fn set_scroll(&mut self, id: WidgetId, offset: f32) -> bool;
    pub fn scroll(&self, id: WidgetId) -> Option<f32>;       // None: not a scroll panel
    pub fn scroll_max(&self, id: WidgetId) -> Option<f32>;   // from the last layout
    pub fn scroll_into_view(&mut self, id: WidgetId) -> bool;
}
// UiRouteSettings gains scroll_line: f32 (40) and drag_threshold: f32 (4, §2.3)
// UiEvent gains Scrolled { widget, offset: f32 } and WindowMoved { window } (§2.3)
```

- **Vertical only (Q11).** A scroll panel is a `Panel` that clips its children.
- **Layout.** Its Taffy style takes `overflow.y = Hidden`, and shrink 1 when its main size is `Auto`, so it takes its content's height up to what its parent's limits leave. `place` sets its maximum offset: the furthest child box edge, margin included, below its content top, plus its bottom padding, less its inner height, at least 0.
- **Offsets.** The stored offset is clamped to 0..=max where it is used; an offset at or past the end follows the end through later layouts; NaN or −∞ is 0. `scroll` reads the clamped value.
- **Paint-only.** `set_scroll` marks paint, never layout. Layout rects do not move; the children map through a translation of −offset, rounded to whole physical pixels as translations are. `visual_rect`, hit testing and a slider's pointer follow; directional focus keeps the layout rects, as since M56.
- **Thumb.** When the maximum is above 0, a `Rect` 4 units wide sits at the panel's inner right edge. Its length is the visible share of the content, at least 16 units, in the panel's resolved text colour at half its alpha. It paints after the panel's children, in the panel's own clip, so an opaque row cannot cover it.
- **Wheel.** A wheel event over a widget inside scroll panels moves the innermost one that can move that way. A line is `scroll_line` units, a pixel delta is divided by the scale factor, and a positive y moves the content down. A non-finite delta is ignored, and reaching the end makes the panel follow it. Each routing sends one `Scrolled` per panel it moved. The wheel over UI stays the UI's, and a pending capture still takes it (M56).
- **Focus.** Every focus change, the router's or the game's, scrolls the new focus into view in each scroll panel above it, from the last layout's rects; before a first layout nothing moves.
- **Settings.** A `scroll_line` or `drag_threshold` that is not finite and above 0 acts as its default.

### 2.3 Movable windows and the placements file

```rust
impl UiTree {
    pub fn set_drag_handle(&mut self, id: WidgetId, on: bool);
    pub fn drag_handle(&self, id: WidgetId) -> bool;
    pub fn set_window_position(&mut self, root: WidgetId, position: Vec2) -> bool;
    pub fn window_position(&self, root: WidgetId) -> Option<Vec2>;   // last layout's top-left
}
// tungsten_core::ui::window, re-exported from ui
pub struct WindowPlacements { /* path, positions, warnings */ }   // Debug, Default
impl WindowPlacements {
    pub fn load(path: impl Into<PathBuf>) -> Self;
    pub fn get(&self, name: &str) -> Option<Vec2>;
    pub fn set(&mut self, name: &str, position: Vec2) -> bool;   // non-finite: false, nothing kept
    pub fn save(&self) -> Result<(), WriteError>;                 // no path: Ok, nothing written
    pub fn take_load_warnings(&mut self) -> Vec<String>;
}
```

- **Handle.** A drag handle is interactive and asks for `CursorIcon::Move` unless a cursor is set. It is not focusable, so pressing it never moves focus (A14).
- **Drag.** Once the pointer has moved more than `drag_threshold` (4 units, the double-click distance, Q12) from a primary press on a handle, the handle's root follows the pointer's motion since the press. Its placement becomes `Anchored` at `TopStart` with the new top-left, clamped: the whole root inside the last layout's viewport when it fits, else its top-left at the viewport's.
- **End.** A release after a drag sends `Released` and `WindowMoved { window }`, with no `Activated` or `DoubleClicked`. A press cancelled after a drag (focus loss, the pointer leaving, the handle removed, hidden or disabled) leaves the window where it is and sends `PressCancelled`, then `WindowMoved`. A click without a drag activates the handle as before.
- **`set_window_position`.** Roots and finite positions only. It clamps the same way once the root has a laid-out size. Before that, with a last viewport known, it keeps the top-left within `0..=viewport − 48` units on each axis, so a title bar shows; with none, at least 0. A root's width can depend on its offset (an anchored root's available width is what the viewport leaves), so the views clamp again after the first layout (§2.4).
- **The file** (decision 166) is JSON `{"version": 1, "windows": {"<name>": [x, y]}}` in UI units. A missing file loads empty. An unreadable or invalid one loads empty and is renamed `.bad` beside itself, as `Settings::load` does, with one warning. A newer version loads empty, leaves the file alone and warns. A bad entry is dropped with a warning. `save` writes through `persist::write_atomic`.
- **The umbrella's file** is `<settings>/debug_windows.json` (Q6). With no user folder (smoke mode, `UserFolder::Off`, no `game.id`) there is no path. Only positions persist, not which views are open (Q5).

### 2.4 The views

- **Windows.** Every view is a window: a root on `UiLayer::Debug` holding a title bar (the view's name, its drag handle) and a body. Rows are label columns sized by layout, with no width heuristic. Every node a view builds, panels and scroll panels included, sets every override field: background, border, border width and corner radius, and every `TextOverrides` field (family, size, line height, weight, italic, colour, alignment, wrap and overflow, `core/ui/theme.rs:466-484`). A scroll panel's thumb takes the panel's resolved text colour (§2.2), so a panel's text matters too. Resolution applies the kind's text, then its state variants, then the node's overrides (`core/ui/theme.rs:615-631`), so a game's theme, including a title bar's hovered and pressed variants, changes nothing in a view.
- **Look.** The background is a new `background` field (`[16, 16, 20, 200]`), and the border is `outline_color` at `outline_px`, 0 for none (Q2). Sizes are UI units, so the views scale with `ScaleFactor`.
- **Placement.** A window opens at its saved place (names `hud`, `systems`, `inspector`, `log`, `ui_inspector`). Otherwise it opens at its corner, inset by `padding_px`, below (top corners) or above (bottom corners) the views opened earlier at that corner without a saved place, in the order HUD, timing overlay, inspector, log console, UI inspector. A window is clamped again with its measured size after its first layout, and whenever the logical viewport changes, so a place saved on a larger screen reopens inside a smaller one. `WindowMoved` for a view's root saves its place; a failed save warns once per run and keeps the place in memory.
- **Sizes.** A window takes its content's width, at most the logical viewport's less twice its padding. The timing overlay's, inspector's, log console's and UI inspector's bodies are scroll panels capped at 45% of the logical viewport's height; the HUD's body is not.
- **Lifecycle.** An enabled view builds its window, or rebuilds it when its root left the tree; a disabled view removes it.
- **Frame loop.** `App::run_frame` runs the views' update after the post-AA apply and before `stage_ui_layout`, timed into `extract` as the layout is. `stage_extract` stops composing the three views' sections.
- **Time base (Q3).** Refresh throttles count `FrameTimings::interval_ms`, fixing the known issue. The fps row shows `1000 / EWMA(interval)` and `EWMA(total)` ms, α unchanged.
- **Engine font family (A17).** `engine_mono` over the `engine_mono` face joins the families the umbrella hands the text engine at startup and at every manifest load and reload, unless the manifest declares that ID. The theme-token check accepts it.
- **HUD.** The fps row, the built-in rows and the `add_row` providers' rows, in order, at `refresh_interval_ms`. Below them the frame-time graph is a mesh of the last 240 intervals as bars, 6 vertices each, scaled against the larger of 33.3 ms and the tallest shown, with bars over 16.7 ms amber `[255, 176, 32, 255]`, redrawn every frame while shown (Q4). `rendered_height_px` is the HUD window's laid-out height in physical pixels plus `padding_px`, 0 while hidden or before a layout.
- **Timing overlay.** EWMA every frame at `alpha`; rows sorted by EWMA at each refresh, names right-aligned in their column.
- **Inspector.** `entity <e>`, then section, row and value columns; `mouse 3 to pick` with nothing selected; refresh at its interval or on a selection change.
- **Defaults.** All else stays as A1–A3 list: off, the three corners, `engine_mono`, sizes, colours, padding, 500 ms, α 0.1.

### 2.5 The log console

- **The `Tee`** (A16, `D-119`'s sink) becomes the target whenever stderr or the file is on (Q7). It writes the file as today. It writes stderr through `eprint!` in a debug build without a file, which the test harness captures as `is_test` did, and with `write_all` otherwise.
- **The sink.** It copies each record (lossy UTF-8, one trailing newline dropped) into a `sync_channel` of 1,024 records with `try_send`. A full or closed channel counts the record in a shared `AtomicU64`. `install` returns the console feed beside the log file. With no logger installed (neither sink, or the game's own), there is no feed.
- **`LogConsole`** (umbrella resource, re-exported) has the views' fields plus `capacity` (512 lines) and `refresh_interval_ms` (100), corner `TopLeft`, F8 `engine_toggle_log_console`. The views' update drains the feed every frame, shown or not, into a ring of `capacity` lines, oldest out.
- **Its window** shows a header (`N lines · D dropped`, or `no engine logger`) and the lines in a scroll panel that follows its end until the wheel moves it off. Each line is coloured by the level token after the timestamp in env_logger's default format (`[<time> <LEVEL> <target>] …`): `ERROR` `[255, 96, 96, 255]`, `WARN` amber, anything else the view's colour.

### 2.6 The picked-widget panel

- `UiTree::widget_dirty(id) -> Option<WidgetDirty>`; `WidgetDirty` is `#[non_exhaustive]` with `style`, `text` and `layout` (its root's).
- **`UiInspector`** (umbrella resource, re-exported) has the views' fields plus `picked: Option<WidgetId>`, corner `TopRight`, stacking under the HUD, F7 `engine_toggle_ui_inspector`.
- **Picking.** While it is shown, a raw `engine_inspector_pick` press with the cursor over a widget picks `UiTree::hit_test(cursor / scale)` on any layer, debug windows included (Q9); a press elsewhere picks nothing new. A removed widget clears the pick.
- **Rows.** Widget (index:generation, kind, role, layer, root); layout rect; visual rect; look (background, border and width, radius); text style for a label or button (family, size, line height, colour); `widget_state`; flags (interactive, focusable, hit-testable, clips, scroll panel, drag handle); pending work (`widget_dirty`); the text batch of its run in the app's last paint list, or none. `middle-click a widget to pick` shows with nothing picked. Refresh at its interval.

### 2.7 Picking, actions and schedule

- **Inspector pick** (step 4). `inspector_pick` takes `.after_if_present(UI_ROUTE)`. `App::resolve_schedule` records in a crate-private resource whether `pre_update` holds `ui_route`, the test `engine_exit_requested` makes (`umbrella/app.rs:561-575`). With routing installed, the pick reads `engine_inspector_pick` in `RoutedInput`, so a press the UI took never picks, whatever the pointer does after it in the frame. `pointer_over_ui()` is not used, being the routing's end state (A12). Without routing it reads the raw input, as the bench's set needs. It picks at the press's position: the last pointer move before the press in the stream it read (the routed view records its own, `core/ui/route.rs:421-447`), else where the cursor ended the previous frame, which `InspectorState` keeps.
- **Actions.** The default map gains `engine_toggle_ui_inspector` (F7) and `engine_toggle_log_console` (F8), and the root `input.json` lists both (Q10). Core's names gain `UI_INSPECTOR_TOGGLE` (`ui_inspector_toggle`) and `LOG_CONSOLE_TOGGLE` (`log_console_toggle`), re-exported at `tungsten::plugins`. `DebugPlugin` registers them after `ui_debug_toggle`.
- **Order.** The schedule resolves the earliest-registered ready system first (`core/schedule.rs:415-416`). After step 4, `DefaultPlugins`' `pre_update` is `physics_debug_toggle, systems_overlay_toggle, inspector_toggle, hud_toggle, ui_debug_toggle, display_input, ui_state_roots, ui_route, inspector_pick, ui_animate, state_dispatcher`. After step 5 it is `physics_debug_toggle, systems_overlay_toggle, inspector_toggle, hud_toggle, ui_debug_toggle, ui_inspector_toggle, log_console_toggle, display_input, ui_state_roots, ui_route, inspector_pick, ui_animate, state_dispatcher`.

### 2.8 Seam, cost and breaks

- **Seam.** Core's additions are plain data (`UiVertex`, `WindowPlacements`, `WidgetDirty`). Render reads the paint list's vertices with no `World` (`D-007`, `D-018`). The umbrella builds the views and owns the logger plumbing. No thread, dependency or Cargo feature is added (`D-131`, `D-015`).
- **Cost when off.** Off, a view builds no root, so a perf row paints nothing new. Every row pays, each frame, the console drain (1–4 records from the perf lines), the views' enabled checks and the two new toggle systems (A19); the owned metrics judge that. Each record also pays, on its logging thread, a copy and a `try_send`. The perf lines are logged after `total_ms` is taken (`umbrella/app.rs:1652-1670`), so that part lands in `interval`, which no row owns (`docs/perf/profiling-workflow.md:117`) and `--fail-on` does not read (`scripts/bench_report.py:948-954`). Step 6 therefore judges every row's `interval` p50 explicitly, from the compare's stage verdicts.
- **Breaks** (step 7's ledger): `PaintList` gains `vertices`. The three views draw as windows, a behaviour change: no text sections; `font_id` names a UI font family; the outline is the window's border; sizes in UI units; refresh and fps on the interval. `inspector_pick` runs after `ui_route` and reads the routed input, so a press the UI took picks nothing; the default map and `DefaultPlugins` gain two actions and two systems. The rest are additions for the freeze.

## 3. Steps

### Step 1: The mesh paint command

- **Files:** core `ui/node.rs`, `ui/tree.rs`, `ui/theme.rs`, `ui/paint.rs`, `ui/mod.rs`, `ui/layout/taffy.rs` (only if the zero natural size needs it), `tests/ui/paint.rs`; render `ui.rs`, `ui.wgsl`, `renderer.rs`, `tests/ui.rs`, `tests/text_batches.rs`. The run found nine `PaintList` literals in `tests/text_batches.rs` (`:225`–`:475`), which A8 missed; the owner widened the list on 2026-10-15 rather than skip the step.
- **Change:**
  1. Add the kind and its arms in A10's four places.
  2. Add `UiVertex`, `mesh`, `set_mesh` and `mesh_vertices`, and the `Mesh` command with `PaintList::vertices`, `clear` and `mesh_vertices` (§2.1); update A8's two literals.
  3. Add the render half: `DrawItem::Mesh`, the pipeline and vertex buffer in `UiPipeline`, `draw_mesh`, and the overlay loop's arm.
  4. Core tests: vertices mapped into the box under a translation and scale with a fade; whole finite triangles kept and a partial one dropped; an empty mesh pushing nothing; the dump line; a mesh over text closing its batch; and Text A, then a mesh whose triangle leaves its box over A, then Text B: the mesh's `rect` covers the triangle and closes A's batch before itself.
  5. Render tests: `build_items` gives primitives, mesh and primitives in paint order with the vertex range and converted colours; the cap cuts at a whole triangle.
- **Done-when:**
  - `cargo test -p tungsten-core --locked ui::paint::tests` → exit 0, at least 41 passed (35 at `04fca16`).
  - `cargo test -p tungsten-render --locked` → exit 0, `tests/ui.rs` at least 10 (8 at `04fca16`) and `shader_coverage` passing.
  - `rg -c 'fn (vs_mesh|fs_mesh)' crates/tungsten-render/src/ui.wgsl` → 2.
  - `cargo clippy --workspace --all-targets --locked -- -D warnings` → exit 0.
  - `cargo test --workspace --locked` → exit 0.
  - `just smoke` → exit 0: every app builds the mesh pipeline on the GPU.
- **Moves:** none.

### Step 2: Scroll panels

- **Files:** core `ui/scroll.rs` (new), `ui/node.rs`, `ui/mod.rs`, `ui/layout.rs`, `ui/layout/taffy.rs`, `ui/paint.rs`, `ui/hit.rs`, `ui/route.rs`, `ui/event.rs`, `ui/focus.rs`, `tests/ui/scroll.rs` (new); umbrella `testing/ui.rs`, `tests/ui_route.rs`.
- **Change:**
  1. Add §2.2: the node's scroll state, the builder and accessors in `scroll.rs`, the Taffy overflow and shrink in `build` and the maximum in `place`, the child translation in paint, hits and `visual_rect`, the thumb, the wheel and `Scrolled` in the router, `scroll_line`, and scroll into view on every focus change.
  2. Add `UiHarness::wheel(lines)`: one vertical wheel event where the pointer is.
  3. Core tests in `ui::scroll::tests`:
     - a panel shrinks to its parent's limit, and its maximum matches the content;
     - a scroll changes paint, hits and `visual_rect` but not layout rects or layout stats;
     - clamping, NaN, and an offset past the end following a growing layout;
     - the innermost panel moves first, and an outer one at the inner's end;
     - lines times `scroll_line`, and pixels divided by the scale;
     - a non-finite delta is ignored;
     - one `Scrolled` per panel per routing;
     - scroll into view from `focus_next` and from a game's `focus`;
     - the thumb's command, after the children's;
     - a child scrolled out of the clip is not hit.
  4. Harness test: the wheel scrolls a panel through the app, and `RoutedInput` reads no scroll.
- **Done-when:**
  - `cargo test -p tungsten-core --locked ui::scroll::tests` → exit 0, at least 10 passed.
  - `cargo test -p tungsten-core --locked ui::` → exit 0.
  - `cargo test -p tungsten --locked ui_route` → exit 0, at least 16 passed (15 at `04fca16`).
  - clippy and `cargo test --workspace --locked` as in step 1 → exit 0.
- **Moves:** none (no GPU path changes; step 4's smoke row draws scrolled panels).

### Step 3: Movable windows and the placements file

- **Files:** core `ui/window.rs` (new), `ui/node.rs`, `ui/mod.rs`, `ui/route.rs`, `ui/event.rs`, `tests/ui/window.rs` (new); umbrella `tests/ui_route.rs`.
- **Change:**
  1. Add §2.3: the handle flag and accessors, `set_window_position` and `window_position`, the drag in the router with `drag_threshold` and `WindowMoved`, and `WindowPlacements`.
  2. Core tests in `ui::window::tests`:
     - no move within the threshold;
     - a move past it moves the root by the pointer's motion;
     - a release after a drag sends `WindowMoved` and no `Activated`;
     - a click without a drag activates;
     - focus loss mid-drag;
     - the clamp, for a window that fits and one that does not;
     - `set_window_position` on a root, on a non-root and with NaN, and on a root with no size yet (top-left kept 48 units inside the last viewport);
     - a press on a handle leaves focus alone;
     - the placements round trip;
     - an invalid file renamed `.bad` with a warning;
     - a newer version kept as it is;
     - non-finite and malformed entries dropped.
  3. Harness test: a drag through the app moves a window and sends `WindowMoved`.
- **Done-when:**
  - `cargo test -p tungsten-core --locked ui::window::tests` → exit 0, at least 12 passed.
  - `cargo test -p tungsten-core --locked ui::` → exit 0.
  - `cargo test -p tungsten --locked ui_route` → exit 0, at least 17 passed.
  - clippy and `cargo test --workspace --locked` → exit 0.
- **Moves:** none.

### Step 4: The HUD, timing overlay and inspector as windows, and the pick on the routed input

- **Files:** umbrella `debug_views.rs` (new), `debug_hud.rs`, `systems_overlay.rs`, `inspector.rs`, `plugins.rs`, `app.rs`, `lib.rs`, `engine_font.rs`, `asset_loader/mod.rs`, `asset_loader/reload.rs`, `tests/debug_views.rs` (new), `tests/debug_hud.rs`, `tests/systems_overlay.rs`, `tests/inspector.rs`, `tests/plugins.rs`, `tests/engine_font.rs`, `tests/asset_loader.rs`; `templates/basic/tests/game.rs`; `examples/01_platformer/src/tests/main.rs`, `examples/03_scene_state/src/main.rs`, `examples/04_shader_playground/src/main.rs`, `examples/05_ui_lab/src/main.rs`; `scripts/smoke-examples.sh`, `scripts/test-smoke-examples.sh`.
- **Change:**
  1. Add the engine font family and its theme-token acceptance (§2.4).
  2. Write `debug_views.rs`: the window builder, rows, placement (saved, corner, stacking, re-clamp), the `WindowMoved` save and the frame-loop entry.
  3. Rebuild the three views on it (§2.4), with the interval time base. `App::new` inserts `WindowPlacements` (§2.3) and logs its load warnings once the logger is up. Remove the compose functions, `MONO_ADVANCE_RATIO` and `anchor_text_block`.
  4. Move the inspector's pick onto the routed input (§2.7): its order, the routing flag `App::resolve_schedule` records, the press position. Update A7's five test copies to the step-4 line (example 01's keeps `, platformer_bindings`).
  5. Add the UI lab's `TUNGSTEN_UI_LAB_FIXTURE=debug`: the lab as `default` builds it, an entity with a `Tag` and a `Transform` selected in the inspector, and the HUD, timing overlay and inspector open. At frame 30 it scrolls the overlay's body and moves the HUD window; it exits through smoke's frame count.
  6. Add a smoke section, `Debug views row`: the fixture for 120 frames with `log_forbids="Unknown font"`. Set the template row's `log_forbids` to `Unknown font` too, so its HUD checks the engine family. Add the stub test's expected line and run.
  7. Rewrite A23's tests for the windows. Cover:
     - `DebugHud::new()`'s A1 defaults;
     - F4, F2 and F3 opening and closing their windows on a `UiHarness`;
     - the HUD's rows in order with an `add_row` provider;
     - a refresh every 500 ms of interval, by calling the update with set `FrameTimings`;
     - fps from the interval;
     - 6 vertices per graph sample, at most 240 samples;
     - the overlay sorted by EWMA;
     - the inspector's columns, `mouse 3 to pick` and a re-pick refreshing at once;
     - every node a view builds resolving to the view's own look, and a scrolled body's thumb painting in the view's colour, under a theme whose label, panel and hovered or pressed variants change family, size, weight, italic, alignment, wrap, overflow, colours (transparent panel text among them), border and radius;
     - stacking at one corner, and a saved place winning;
     - a `WindowMoved` saving through `UserFolder::At` and reopening there;
     - a place saved at (1800, 900) reopening wholly inside an 800×600 logical viewport after its first layout;
     - the families with and without a manifest `engine_mono`;
     - no text section from the views;
     - the pick: a middle press over a debug window or a game root, then a move into the world within the same frame, leaves the selection; a press over the world, then a move onto a window, selects the entity at the press point; without the `UiPlugin` the raw press picks as before.
- **Done-when:**
  - `cargo test -p tungsten --locked debug_views::tests` → exit 0, at least 8 passed.
  - `cargo test -p tungsten --locked inspector` → exit 0.
  - `cargo test -p tungsten --locked` → exit 0.
  - The step-4 pre_update check exits 0 and prints 1 for each file:

    ```bash
    rg -c 'pre_update: physics_debug_toggle, systems_overlay_toggle, inspector_toggle, hud_toggle, ui_debug_toggle, display_input, ui_state_roots, ui_route, inspector_pick, ui_animate, state_dispatcher' \
      crates/tungsten/src/tests/plugins.rs templates/basic/tests/game.rs \
      examples/01_platformer/src/tests/main.rs examples/03_scene_state/src/main.rs \
      examples/04_shader_playground/src/main.rs
    ```

  - `rg -n 'compose_hud_text_sections|compose_systems_overlay_text_section|compose_inspector_text_section|MONO_ADVANCE_RATIO|anchor_text_block' crates examples templates` → no match.
  - `bash scripts/test-smoke-examples.sh && shellcheck scripts/*.sh` → exit 0.
  - clippy and `cargo test --workspace --locked` → exit 0.
  - `just smoke` → exit 0, printing `Template passed: 1/1` and `Debug views passed: 1/1`.
- **Moves:** none: no pixel fixture shows a view (A20).

### Step 5: The log console, the picked-widget panel, F7 and F8

- **Files:** umbrella `logging.rs`, `debug_views.rs`, `plugins.rs`, `app.rs`, `lib.rs`, `tests/logging.rs`, `tests/debug_views.rs`, `tests/plugins.rs`; core `schedule.rs`, `input/action_map.rs`, `ui/tree.rs`, `ui/mod.rs`, `tests/input/action_map.rs`, `tests/ui/tree.rs`; `input.json`; `templates/basic/tests/game.rs`; `examples/01_platformer/src/tests/main.rs`, `examples/03_scene_state/src/main.rs`, `examples/04_shader_playground/src/main.rs`, `examples/05_ui_lab/src/main.rs`.
- **Change:**
  1. Add §2.5: the `Tee`'s target rule, stderr modes and sink; `install`'s feed and its caller; `LogConsole` and its window.
  2. Add §2.6: `widget_dirty` and `UiInspector`.
  3. Add §2.7's actions, names and toggles. Update A7's five test copies from the step-4 line to the step-5 line (example 01's keeps `, platformer_bindings`).
  4. The `debug` fixture also opens the log console and the UI inspector, logs one warning, and picks a lab widget.
  5. Tests:
     - the sink receives each record whole without its newline;
     - a full channel counts drops;
     - stderr modes by build and file;
     - no feed when a logger is set;
     - the drain runs while hidden, keeps `capacity` lines and colours by level;
     - the console follows its end until the wheel moves it off;
     - F7 and F8 toggle;
     - a middle click over a widget picks it, and over nothing does not;
     - the panel's rows show the rect, look, pending work and batch;
     - a removed widget clears the pick;
     - the defaults list gains the two actions, and `input.json` loads.
- **Done-when:**
  - The step-5 pre_update check exits 0 and prints 1 for each file:

    ```bash
    rg -c 'pre_update: physics_debug_toggle, systems_overlay_toggle, inspector_toggle, hud_toggle, ui_debug_toggle, ui_inspector_toggle, log_console_toggle, display_input, ui_state_roots, ui_route, inspector_pick, ui_animate, state_dispatcher' \
      crates/tungsten/src/tests/plugins.rs templates/basic/tests/game.rs \
      examples/01_platformer/src/tests/main.rs examples/03_scene_state/src/main.rs \
      examples/04_shader_playground/src/main.rs
    ```

  - `rg -c 'engine_toggle_(ui_inspector|log_console)' input.json` → 2.
  - `cargo test -p tungsten --locked logging` → exit 0, at least 15 passed (12 at `04fca16`).
  - `cargo test -p tungsten --locked debug_views::tests` → exit 0, at least 8 more passed than step 4 left.
  - clippy and `cargo test --workspace --locked` → exit 0.
  - `just smoke` → exit 0, printing `Debug views passed: 1/1`.
- **Moves:** none.

### Step 6: The suite sitting and the gates

- **Files:** `perf-runs/<UTC date>-m57-debug-views/` (scripts, README, compares, copied base suites; untracked); `target/m57-base/` and any `target/m57-aligned-*` (deleted at the end).
- **Change:**
  1. Before: `df -h target` shows at least 15 GB free; no other agent session works in the tree (`pwdx $(pgrep -x 'claude|codex')`); the runner's preflight handles the encoder and foreign builds, and the session waits for 60 s without `nxcodec.bin` ([capture rules](../../../perf/profiling-workflow.md#comparison-rule-and-capture-rules); workflow §6).
  2. Base: `mkdir -p target/m57-base && git archive HEAD | tar -x -C target/m57-base` (`HEAD` is `04fca16`, v0.76.0).
  3. Prebuild both trees with the runner's flags. Tell the binaries apart by `sha256`. List each owned metric's function offset mod 64 in both (`nm -C`) in the README.
  4. One job the session waits on with nothing else running. It runs four `just perf suite --repeat 5` captures in the order base-a, cand-a, base-b, cand-b; a suite that exits 3 is set aside and rerun once. Then the compares (`just perf compare <base> <candidate> --out <evidence>/…`): the A/A pairs base-a→base-b and cand-a→cand-b, and the direct pairs base-a→cand-a and base-b→cand-b. Then the gates: `just smoke`, `just visual` and `just physics-release`. A six-suite sitting runs past the 10-minute foreground cap, so the job runs in the background and its end re-invokes the session.
  5. If §7's placement rule applies, a second job captures the aligned pair: both trees built with `TUNGSTEN_PERF_RUSTFLAGS="-C force-frame-pointers=yes -C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-nofallthru-blocks=6"`, two suites a side `--only` the rows concerned, compared aligned with aligned.
  6. Copy the base suites into the evidence folder (`cp -a`, then `diff -rq` exits 0), write the README's verdict table, then delete the scratch trees with `shutil.rmtree`.
- **Done-when:**
  - All four suites are valid, each at most one rerun.
  - The A/A pairs read no owned metric `regressed` or `improved`, except the documented drifts (the workflow's A/A note; `particles` `animate_sprites`, `unattributed` and `particle_count_refresh` at p50 and p95; `gpu` `render_encode` p95). Each such trip clears on one rerun of its row's pair.
  - The direct pairs pass `--fail-on regressed` → exit 0 in both. An owned metric `regressed` in one pair clears on one rerun of that row's pair. One `regressed` in both pairs follows §7.
  - Every row's `interval` p50 reads not `regressed` in both direct pairs (`compare.json` `stages`), under the same one-rerun rule; the README lists each row's `interval` p50 and p95 deltas with their intervals (§2.8: the console's producer-side copy lands there).
  - Every row's digests match in both direct pairs (`compare.json` `digests.match`).
  - `just smoke`, `just visual` and `just physics-release` → exit 0.
  - `git status --short` shows no golden image changed.
  - The README quotes each compare's verdict line.
- **Moves:** none.

### Step 7: Records

- **Files:** `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/known-issues.md`, `docs/perf/benchmarks.md`, `docs/plans/1.0/w01-ui-text-suite.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/plans/1.0/backlog-1.x.md`, `api/tungsten-core.txt`, `api/tungsten.txt`, `api/tungsten-render.txt`.
- **Change:**
  1. Recheck the next free decision ID, then write decisions 165 and 166 with the [tungsten-decision](../../../../.claude/skills/tungsten-decision/SKILL.md) skill (§5): their index rows, and "Amended by" notes on the `D-147` and `D-149` rows.
  2. `DESIGN.md`: a section `### Engine Debug Views — M57` after M56's; the engine actions at `:279`; the console sentence at `:361`; the `pre_update` line at `:393`.
  3. `docs/LLM_INDEX.md`: route `tungsten/debug_views.rs` and the debug views in the HUD row. `core/ui/` already covers the core files. Trim wording elsewhere, never a path, so `just ctx` passes.
  4. Known issues: drop the refresh-throttle row (fixed); add what the run found, and with step 5 skipped the `M57 step 5` re-land entry (§7).
  5. `docs/perf/benchmarks.md`: mark the HUD fps bullet landed in 0.77.
  6. w01 §11's M5 row gains its landed note.
  7. The [break ledger](../../1.0/w04-api-freeze.md#break-ledger) gains §2.8's rows.
  8. The backlog gains rows for scrollbar dragging and horizontal scroll panels, saved open state, and raising a window on click.
  9. `just api`; review the diff.
- **Done-when:**
  - `rg -c '^## D-16[5] — ' DECISIONS.md` → 1, and `rg -c '^## D-16[6] — ' DECISIONS.md` → 1.
  - ``rg -c '^\| `D-16[56]` ' docs/DECISION_INDEX.md`` → 2.
  - ``rg -c '^\| `D-14[79]` .*Amended by `D-16[5]`' docs/DECISION_INDEX.md`` → 2.
  - `rg -c '^### Engine Debug Views — M57' DESIGN.md` → 1.
  - With step 5 landed, `rg -c 'hud_toggle, ui_debug_toggle, ui_inspector_toggle, log_console_toggle' DESIGN.md` → 1. With it skipped, `rg -c 'hud_toggle, ui_debug_toggle, display_input, ui_state_roots, ui_route, inspector_pick' DESIGN.md` → 1, and `rg -c 'M57 step 5' docs/known-issues.md` → 1 (the re-land entry).
  - `rg -c 'inspector_toggle, inspector_pick' DESIGN.md` → 0.
  - `rg -c 'tungsten/debug_views.rs' docs/LLM_INDEX.md` → 1.
  - `just ctx` → `Agent context checks: OK`.
  - `rg -c 'refresh throttles add the frame' docs/known-issues.md` → 0.
  - `rg -c 'Landed in 0\.77 \(M57' docs/plans/1.0/w01-ui-text-suite.md` → 1.
  - `rg -c '0\.77 \(M57\)' docs/plans/1.0/w04-api-freeze.md` → at least 3.
  - `just api` → exit 0, after which `rg -q 'WidgetKind::Mesh' api/tungsten-core.txt && rg -q 'scroll_panel' api/tungsten-core.txt && rg -q 'WindowPlacements' api/tungsten-core.txt` → exit 0, and, with step 5 landed, `rg -q 'LogConsole' api/tungsten.txt && rg -q 'UiInspector' api/tungsten.txt` → exit 0.
  - `just api-check` and `just repo-check` → exit 0.
- **Moves:** none.

### Step 8: Release 0.77

- **Files:** `CHANGELOG.md`, `DESIGN.md` (step 7 wrote its section; the cut rewrites its status line), `Cargo.toml`, `Cargo.lock`, the register row, the README's four owned Now fields (`roadmap.py summary`), this plan (moved to `docs/plans/archive/1.0/`).
- **Change:** Close workflow §5 and this step's evidence row with `status: done`. Then run [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.77.0` once, every release check once after it, plus `just visual` and `just physics-release` when step 6 did not pass them, then the preflight with `--message`.
- **Done-when:**
  - [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut).
  - `just release-preflight 0.77.0 --repo JosephGibson/Tungsten --message 'Update 0.77: M57 engine debug views'` → passes and prints the command block and the post-merge block.
- **Ends with:** the changed-file list, the command block, the post-merge block, then the plan prompt for the next register row, W6b and W6c (M58), from [tungsten-next's prompts](../../../../.claude/skills/tungsten-next/prompts.md) `plan` row with that stop's catalog fields.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Does the HUD become a movable window too, or stay a fixed corner block? | A window like the others: the roadmap's summary says the three become movable windows, and placement is one rule |
| Q2 | What do `outline_color` and `outline_px` mean once text is not outlined? | The window's border colour and width (0: none); a new `background` field gives the contrast the outline gave. No deprecation, since none of the three can be built by literal outside the crate |
| Q3 | Fix the refresh time base and the fps row now? | Yes: both are rewritten here (A5); the known issue and the benchmarks finding close in step 7 |
| Q4 | Redraw the graph every frame while the HUD shows? | Yes: one repaint of the tree per frame while the HUD is open, unchanged text batches re-preparing nothing; the rows keep their refresh |
| Q5 | Persist which views are open, as w01 §14 proposed? | No, positions only: startup code sets `enabled` (A20), and a pixel fixture must never show a view a developer left open; backlog row |
| Q6 | A workspace-local file (w01 §14) or the user folder? | The user folder's settings (`D-159`), so a shipped game never writes beside its executable; none without a folder |
| Q7 | The console's sink in the `Tee` (`D-119`) or a logger wrapping env_logger's? | The `Tee`, as `D-119` decided, now the target whenever stderr or the file is on; stderr keeps today's test capture (A16) |
| Q8 | Console sizes? | 512 lines kept, a 1,024-record channel, a 100 ms refresh; a fuller channel counts drops |
| Q9 | How does the panel pick a widget? | The inspector's own action (middle click) over UI while the panel is shown, any layer; no hover picking, which would change under the pointer moving to the panel |
| Q10 | Which keys? | F7 the UI inspector, F8 the log console: unbound everywhere (A18) |
| Q11 | Scroll scope? | Vertical, wheel, scroll into view and code; no horizontal scrolling or thumb dragging (backlog row); 40 units a line |
| Q12 | Drag threshold? | 4 UI units, the double-click distance, as a router setting |
| Q13 | Raise a window on click? | No: views open at distinct places; backlog row |
| Q14 | Measure w01 §10's 500-widget debug-view budget? | No: the card's check is the suite; C1 or the feature gate places UI rows (known issues) |
| Q15 | What justifies a placement claim (§7)? | The aligned pair reading the metric not `regressed`, with the functions' offsets listed: the regression policy accepts a plan's justification, and one pad cannot match per-crate shifts |
| Q16 | One new umbrella file or one per view? | One, `debug_views.rs`, with the console's feed in `logging.rs`: the index has 4 bytes left (A24) |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`).

## 5. Decisions expected

- **165:** W1 M5 landed. The engine's debug views (the HUD with its frame-time graph, the timing overlay, the inspector, the log console and the UI inspector) are windows on the debug layer. Core gains scroll panels (vertical, paint-only offsets, the wheel, scroll into view on focus), drag handles moving their roots past a threshold, and the mesh paint command (box-fraction vertices, physical in the list, its own overlay pipeline). Also in it: the engine font as the `engine_mono` UI family, the console's `Tee` sink as built, `inspector_pick` after `ui_route` on the routed input at the press position, F7 and F8, and the views' time base on the frame interval. Amends `D-147` and `D-149`; extends `D-044`, `D-047`, `D-119` and `D-123`.
- **166:** Debug window placements persist in the user folder's settings as `debug_windows.json`: version 1, names to top-left positions in UI units, written atomically at each drag's end, none without a folder, a corrupt file set aside as `.bad`, open state not saved. Extends `D-159`.

These were the next free IDs when written (`DECISIONS.md` ends at `D-164`); step 7 checks again.

## 6. Invariants

- **May move:** none.
- **Must not move:** the determinism hash and the pinned containment hash (`just physics-release`), the row digests (step 6's compares), `gpu-visual.png`, the post, transition and UI-lab regressions (`just visual`), every benchmark's `workload_version`.

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with steps that don't depend on it.
- A step needs a file outside its list: restore and skip it, naming the file. The run does not widen a list.
- **Dependencies:** steps 2 and 3 do not depend on step 1. Step 4 depends on steps 1–3, and step 5 on step 4. Step 6 measures whatever landed; step 7 records what landed.
- **What the release needs:** steps 1–4, which hold the windows, scroll panels, saved places, the graph and the pick's protection, a passing step 6, and step 7, whose checks follow what landed. If any of those is skipped, or step 6 stops, the run ends before the release with the reason in the evidence log, and a release session cuts it after the owner has seen why (workflow §3). Step 5 may be skipped: the release then delivers M5 in part. Step 7's decision covers what landed. w01's M5 note and the register row say that the log console and the picked-widget panel did not land. A known-issues entry naming `M57 step 5` carries their re-land, with the step's skip reason.
- **A/A:** a pair that still trips after its row's one rerun puts the sitting's quiet in doubt. Rerun the whole sitting once; a second trip stops.
- **Direct pairs:** an owned metric `regressed` in both pairs (after the one-rerun rule) is M5's own when the frame loop runs it: `extract`, `update`, `total`, jitter; so is `interval` p50 `regressed` in both pairs, the logger's producer-side cost. That stops the run before the release, unless step 5's console drain or sink, the likely cause, is fixed within that step's files and a new sitting reads clean. On code M5 does not touch (bench systems, `flush`, `physics_step`, the particle systems, GPU passes), the aligned pair decides (Q15): not `regressed` aligned is recorded as placement with the offsets; `regressed` aligned stops.
- **Suites:** a suite invalid twice stops.
- **Pixels:** `just visual` fails or a golden would change: stop. No golden is replaced; the pixel difference is reported.
- **Records:** a decision ID taken by another session meanwhile: use the next free ones and update §5 and step 7's checks to them.
- **Dependencies and features:** a new dependency or Cargo feature looks necessary: stop that step.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1, 4, 5, 6, release | Smoke in steps 1, 4 and 5; the suite, smoke, visual and the physics release tests in step 6; the release step runs `just check`, `just smoke`, `just script-test`, `just api-check`, `just template-check` and the rest of the release checks once, after the cut, plus `just visual` and `just physics-release` when step 6 did not pass them |
| 3 Invariants | 6 | Digests, visual, physics hashes |
| 4 Decisions | 7 | 165, 166 |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1–5, 7 | Rustdoc as items land; ledger rows and snapshots in step 7 |
| 6 Template and guide | n/a | No change to how a game is written; step 5 updates the template's schedule test only; W9b has not started |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 7 | One new umbrella file; the new core files sit under the routed `core/ui/` |
| 8 Design | 7 | The M57 section and the three updated lines |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 7, release | Known issues, benchmarks, backlog, w01 and the ledger in step 7; the register row and README fields at the release; the gap log is empty, so it has no row to close. `python3 -B scripts/roadmap.py summary` supplies the owned fields |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

Round 1, 2026-10-15: the `critique` skill with `--repo` on this plan with §9 and the evidence log cut by `sed`, the tree at `04fca16` plus the plan. Footer: gpt-6-astra, effort xhigh, context 872,000, output cap 4,000, 522.3 s, tokens in 1,096,460 (cached 1,016,064), out 10,719 (reasoning 6,529), 23 commands, output 283.9 KB (largest 31.5 KB), usage 5h 0% → 24%, week 0% → 4%. Round 1 changed done-when checks (steps 1, 4 and 6) and moved the pick from step 5 to step 4, so round 2 runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] A mesh's triangles may leave its box while the partition tests its box, so a later mesh can draw under earlier text | `core/ui/paint.rs:333-357` (the rule tests `command.rect()`, `:346`); `render/ui.rs:229-241` (a batch draws where it closes) | Accepted | §2.1: a `Mesh` command's `rect` is its vertices' bounding box; step 1 tests Text, then an overflowing mesh, then Text, and expects 41 paint tests |
| 2 | [HIGH] `pointer_over_ui()` is the routing's end state, so a press over a window followed by a move into the world still picks | `core/ui/route.rs:1113-1116`, `:709-737` (a press the UI takes is not routed); `umbrella/inspector.rs:160-178` (raw press, last cursor) | Accepted | §2.7: the pick reads `RoutedInput` when `ui_route` is installed (recorded at `resolve_schedule`), raw input otherwise, at the press position; moved into step 4 with its tests |
| 3 | [MEDIUM] A restored window is clamped only before its first layout, so a place saved on a larger screen can stay off-screen | `core/ui/node.rs:233` (no rect before a layout); `core/ui/layout/taffy.rs:254-301` (no clamp in `place`) | Accepted | §2.3: an unmeasured root keeps its top-left 48 units inside the last viewport; §2.4: the views clamp after the first layout; tests in steps 3 and 4 |
| 4 | [MEDIUM] The overrides leave weight, italic, alignment, wrap and overflow to the game's theme | `core/ui/theme.rs:466-484` (nine text fields), `:615-631` (kind, variants, then the node's overrides) | Accepted | §2.4: every node sets every override field; step 4's isolation test changes all of them and the hovered and pressed variants |
| 5 | [MEDIUM] The console's copy lands in `interval`, which no gate reads | `umbrella/app.rs:1652-1670`; `scripts/bench_report.py:660-662`, `:948-954`; `docs/perf/profiling-workflow.md:117` | Accepted | §2.8 and step 6: every row's `interval` p50 not `regressed` in both direct pairs, from the compare's stage verdicts; §7 stops on it |
| 6 | [MEDIUM] Skipping step 5 would ship M5 without the pick's protection and call it landed | §7 as written; `docs/plans/1.0/w01-ui-text-suite.md:419` | Accepted | The pick moved to step 4, which the release needs; §7 says how a skipped step 5 ships as a partial M5 in the decision, w01's note, the register row and known issues |

Round 2, 2026-10-15, on the revised plan cut the same way, with a focus note naming round 1's changes and no rejections. Footer: gpt-6-astra, effort xhigh, context 872,000, output cap 4,000, 406.2 s, tokens in 871,406 (cached 780,160), out 9,202 (reasoning 4,721), 35 commands, output 305.8 KB (largest 25.2 KB), usage 5h 24% → 45%, week 4% → 7%. Its two findings were fixed in place; there is no third round.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] Step 7 requires step 5's types and toggles, so a skipped step 5 fails the records step and loses the partial release's records | Step 7's API and `DESIGN.md` checks as written; §7's partial-release path | Accepted | Step 7's checks follow what landed: the step-4 `pre_update` line and the `M57 step 5` known-issues entry when step 5 is skipped, `LogConsole` and `UiInspector` in the snapshot only when it landed; §7 adds step 7 to what the release needs |
| 2.2 | [MEDIUM] A game theme can still hide a debug scroll thumb, which takes the panel's text colour | `core/ui/theme.rs:622-631` (the kind's text applies before the node's overrides); §2.2's thumb colour | Accepted | §2.4: panels and scroll panels set every override field too; step 4's isolation test checks the painted thumb under transparent panel text |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-15 | Done, its list widened by the owner | `ui::paint::tests` 41 passed (35 before); render `ui::tests` 11 passed (8), `shader_coverage` ok; `rg -c 'fn (vs_mesh\|fs_mesh)'` → 2; clippy exit 0; workspace tests exit 0 (2,042 passed, 8 ignored); `just smoke` exit 0 (examples 5/5, Template 1/1, UI screens 1/1, Benchmarks 15/15, every row passed) | Render `tests/text_batches.rs` joined the list: its nine `PaintList` literals, which A8 missed, need `vertices`. `ui/layout/taffy.rs` unchanged: a mesh's `Auto` axis already measures 0 |
| 2 | 2026-10-15 | Done | `ui::scroll::tests` 10 passed; core `ui::` 220 passed; `ui_route` 16 passed (15 before); clippy exit 0; workspace tests exit 0 (2,053 passed, 8 ignored) | `ui/layout.rs` unchanged: the maximum and the follow rule live in `layout/taffy.rs`'s `place` and `scroll.rs`. `Scrolled` comes from the wheel only; focus scrolling and `set_scroll` send none |
| 3 | 2026-10-15 | Done | `ui::window::tests` 12 passed; core `ui::` 232 passed; `ui_route` 17 passed; clippy exit 0; workspace tests exit 0 (2,066 passed, 8 ignored) | The router treats a drag handle as a pointer target without setting `interactive`; `WindowMoved` needs the root still in the tree; an unreadable file is left in place as `Settings::load` leaves one, a corrupt or invalid one becomes `.bad` |
| 4 | 2026-10-15 | Done | `debug_views::tests` 9 passed; `inspector` exit 0 (20 passed); `cargo test -p tungsten` exit 0; the step-4 `pre_update` check → 1 in each of the five files; the removed-symbols `rg` → no match (exit 1); `test-smoke-examples.sh` OK and `shellcheck` exit 0; clippy exit 0; workspace tests exit 0 (2,071 passed, 8 ignored); `just smoke` exit 0 with `Template passed: 1/1` and `Debug views passed: 1/1` (the row logged its frame-30 scroll to 120 and HUD move, no warning) | Each view gained a public `window()` root accessor, which the lab's fixture needs. Roots clip their children, take `Role::Window` and the view's title as accessible name. A view stops managing a corner placement once anything else changed it, so a drag in progress is not undone; the overlay's rows refresh at once while no timing has arrived. Captures of the fixture and the interactive run (scratchpad, not kept) showed the windows over the game UI |
| 5 | 2026-10-15 | Done | The step-5 `pre_update` check → 1 in each of the five files; `rg -c 'engine_toggle_(ui_inspector\|log_console)' input.json` → 2; `logging` 17 passed (12 before); `debug_views::tests` 18 passed (9 after step 4); clippy exit 0; workspace tests exit 0 (2,085 passed, 8 ignored); `just smoke` exit 0 with `Debug views passed: 1/1` (its log holds the fixture's state-stack warning, the frame-30 scroll and move) | The lab has no `log` dependency, so its one warning is the engine's: `StateStack::own_root` with no state pushed. `debug_views::update_with` takes the app's last paint list for the batch row; `update`, without it, is test-only. The UI inspector picks `hit_test`'s deepest widget, a button's label rather than the button |
| 6 | 2026-10-15 | Done, the sitting run twice (§7) | Sitting 2 stands: four `suite --repeat 5` suites valid, base B on its one rerun (`nxcodec.bin`); direct A `0 regressed, 1 improved, 43 unchanged, 14 noisy` and direct B `0 regressed, 1 improved, 39 unchanged, 18 noisy`, both `--fail-on regressed` exit 0; base A/A `0 regressed, 0 improved, 42 unchanged, 16 noisy`, candidate A/A `0 regressed, 0 improved, 44 unchanged, 14 noisy`; every row's `interval` p50 not `regressed` in both pairs; digests 8/8 in both; `just smoke`, `just visual`, `just physics-release` exit 0; no golden changed. Sitting 1's base A/A tripped on `particles` `particle_count_refresh` p95 (0.780 → 0.880) and its row's one rerun on p50 and p95 `improved`, so §7 reran the sitting | `perf-runs/20261015-m57-debug-views/`: `README.md` (verdict lines, `interval` deltas), `summary.md`, `summary2.md`, `placement.md`, `nm/`, `base/`, logs. The only `improved` readings are `ecs` `bounds_wrap` p50 in both direct pairs. Scratch trees deleted |
| 7 | 2026-10-15 | Done | `D-165` and `D-166` headings → 1 and 1, their index rows → 2, the `D-147` and `D-149` rows' `Amended by \`D-165\`` → 2; DESIGN's M57 section → 1, the step-5 `pre_update` line → 1, the old `inspector_toggle, inspector_pick` → 0; `tungsten/debug_views.rs` in the index → 1 and `just ctx` → `Agent context checks: OK` (12,284 B, wording trimmed in four rows, no path); the throttle row → 0; w01's note → 1; the ledger's `0.77 (M57)` → 4; `just api` exit 0 (core +206, render +24, umbrella +67 lines, none removed), the five snapshot greps exit 0; `just api-check` exit 0, `just repo-check` 0 errors | Known issues gained the windows' size overlap at 1280×720 and, as a follow-up, the lab fixture's borrowed warning. D-165 records that `env_logger` strips colours on the `Tee`, a pipe, so a debug build without a log file now prints plain level labels |
| 8 | 2026-10-15 | Done; the cut, its checks and the preflight are quoted in the run's report | Before the cut, `just release-check` exit 0 (`workspace 0.76.0 = CHANGELOG.md [0.76.0] - 2026-10-15`, `[Unreleased] is empty`); §5 closed: `CHANGELOG.md` `[Unreleased]` written (Summary naming this plan's archive path, Added, Changed, Fixed), DESIGN's status line gains the debug views, the register row reads "Cut 2026-10-15 as `v0.77.0`"; six new public items had no rustdoc (`LogConsole::enabled`, `color`, `toggle`; `UiInspector::enabled`, `color`, `toggle`) and gained it, then `just api` exit 0 and `just api-check` → `Public API snapshots current: 4 crates` | This plan moved to `docs/plans/archive/1.0/` before `just release-cut 0.77.0`, which writes the README's owned fields; `just visual` and `just physics-release` passed in step 6, so the release does not rerun them |

## Follow-ups

- Physics debug's controls and status in the UI (w01 §11's migration table) are on no card: the feature gate places them, or the backlog takes them.
- The flipped-sprite pick offset (`docs/known-issues.md:37`) stays with the Phase 6 QA pass: M5 moves the pick, not its geometry.

# The debug views pass (debug-pass)

- **status:** in progress
- **goal:** fix what is broken in the engine's debug views (the HUD F4, the systems overlay F2, the entity inspector F3, the UI inspector F7, the log console F8, the physics overlay F1, UI bounds F5, the zoom keys and the saved window places), each fix behind a failing test or a capture; tighten their look inside `D-168`; move them onto `D-235`'s surface where a game's theme still changes nothing (`D-165`); measure what each costs closed and open, cut the closed costs and pin both with counter guards; give the systems overlay a flat view and a pause.
- **non-goals:** `KitDebug`'s hotkey (K22, 7.10c); the shell's HUD corner (S-123, 7.11); code under `templates/` or `examples/` (captures, and the counter-guard pins the timing fix moves, §2.5, are the only touches there); a frame-time claim or a sitting (`D-226`); named text styles, shrinking text, screen files or `ui::widgets`' themed builders in the views (§2.4); a new F-key; the overlay features parked in Later (§2.6); the open costs of §2.5 beyond their guard.
- **files to touch:** (1) `crates/tungsten/src/tests/debug_views.rs`, `crates/tungsten/src/tests/guards/views-closed.json` and `views-open.json` (new), `docs/perf/benchmarks.md`; (2) `crates/tungsten-core/src/ui/paint.rs`, `crates/tungsten-core/src/tests/ui/paint.rs`, `DESIGN.md`, `examples/01_platformer/tests/game/guards/pit.json`, `crates/tungsten/src/tests/guards/views-open.json`; (3) `crates/tungsten/src/debug_hud.rs`, `crates/tungsten/src/tests/debug_hud.rs`; (4) `crates/tungsten/src/debug_views.rs`, `crates/tungsten/src/tests/debug_views.rs`; (5) `crates/tungsten/src/inspector.rs`, `crates/tungsten/src/sprite_extract.rs`, `crates/tungsten/src/tests/inspector.rs`; (6) `crates/tungsten/src/app.rs`, `crates/tungsten/src/tests/telemetry.rs`, `docs/known-issues.md`, `examples/02_bench/src/tests/guards/*.json` but `boot.json`, `examples/01_platformer/tests/game/guards/pit.json`, `crates/tungsten/src/tests/guards/views-*.json`; (7) `crates/tungsten/src/state.rs`, `crates/tungsten/src/tests/state.rs`, `DESIGN.md`, `examples/01_platformer/tests/game/guards/pit.json`; (8) `crates/tungsten/src/debug_views.rs`, `crates/tungsten/src/systems_overlay.rs`, `crates/tungsten/src/tests/debug_views.rs`, `crates/tungsten/src/tests/systems_overlay.rs`, `crates/tungsten/src/tests/guards/views-open.json`; (9) `crates/tungsten/src/debug_views.rs`, `crates/tungsten/src/tests/debug_views.rs`, `DESIGN.md`; (10) `crates/tungsten/src/physics_debug.rs`, `crates/tungsten/src/tests/physics_debug.rs`; (11) `crates/tungsten/src/debug_views.rs`, `crates/tungsten/src/testing/ui.rs`, `crates/tungsten/src/tests/debug_views.rs`, `crates/tungsten/src/tests/testing_ui.rs`, `crates/tungsten/src/tests/guards/views-open.json`, `DESIGN.md`; (12) `crates/tungsten/src/debug_views.rs`, `crates/tungsten/src/systems_overlay.rs`, `crates/tungsten/src/debug_hud.rs`, `crates/tungsten/src/inspector.rs`, `crates/tungsten/src/tests/debug_views.rs`, `crates/tungsten/src/tests/systems_overlay.rs`, `crates/tungsten/src/tests/guards/views-open.json`, `DESIGN.md`; (13) this plan (deleted), `ROADMAP.md`, `DESIGN.md`, `docs/known-issues.md`. Every step also writes its row in this plan's evidence log. Untracked: `perf-runs/20261102-debug-pass/`.
- **ordered steps:** (1) the baseline: the before sheet, the views' two counter guards, an informational timing; (2) F5 outlines only what each clip shows, and builds its list only while on; (3) a reopened HUD starts afresh; (4) a paused console keeps its rows; (5) the entity pick finds a sprite where it draws; (6) the timing path allocates nothing once warm; (7) the dispatcher writes `HudActiveState` only on a change; (8) the look: stripes, rules, the console's controls line, the overlay's headers; (9) the dock: whole lines, and a later window reserved only what it asked for; (10) F1's outlines a fixed width on screen; (11) the UI pass; (12) the systems overlay's flat view and pause; (13) close.
- **done-when:** every step's done-when passes, quoted in the evidence log; the views' tests and `just check` pass; the before-and-after sheet (§2.1) is linked to the owner with what to open and look at; the plan deleted with its citations and `debug-pass` out of Now.

Item `debug-pass` in [ROADMAP.md](../../ROADMAP.md), from the owner's request of 2026-11-02, outside Phase 7: 7.10 waits until it closes. Written 2026-11-02 at `5478c501`. Runs under the [tungsten-plan skill](../../.claude/skills/tungsten-plan/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs.

## Context digest

The views are windows on the core UI's debug layer (`D-165`): every node sets every style and text override, so a game's theme changes nothing in them. Places and the zoom persist in `debug_windows.json` (`D-166`, in OS-logical units since `D-168`). The debug layers take their own scale (`D-167`); `D-168` settled the views' scale (zoom × step / UI scale, the step 3/4 at the owner's 1280×720 on a 1.75 display, a 975×549-unit debug viewport), their docking without overlap at 960×540 units or more, one palette at 4.5:1 composited in linear light, chrome and columns. The HUD reads existing telemetry, `FrameTimings` and `HudActiveState` (`D-044`). Since then `D-235` and `D-236` added widget keys, named text styles, `ui::widgets`, pixel snapping, shrinking text, held clocks and `UiHarness` queries; the views predate them. A plan's perf checks are its counter guards, a sitting only behind a speed-up claim (`D-226`).

This plan comes from reading the seven source files whole and the timing path in `app.rs`, and from four planning captures (`perf-runs/20261102-debug-pass/planning/`: 03's `debug` fixture at 1280×720 on 1.75 and on 1.0; 01 scripted with every view, F1 and F5 at 1280×720 on 1.75). The owner's lead holds: every frame of every game builds a `String` per system for the timings, open or closed, and every counter-guard pin carries them (§1 A5). 7.10's kit pass waits for this pass; 7.11 takes the shell's HUD corner (S-123), which §1 A14 meets.

## 1. Audit

Read at `5478c501`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | F5 outlines every node the paint walks, a scroll panel's hidden rows included, in clip 0 after every other command: in 01 with the console and the overlay open, their scrolled-out rows outline down the whole screen. The list is built on every repaint with F5 off too | `crates/tungsten-core/src/ui/paint.rs:859`, `:894-905`, `:963`; `planning/e01-f5-bounds-1280x720-os1.75.png` |
| A2 | Closing the HUD keeps its samples, averages and `seeded`, so a reopened graph and its statistics show frames from before it closed; the overlay clears its own on close (`systems_overlay.rs:184-188`) | `crates/tungsten/src/debug_hud.rs:219-223`, `:233-253` |
| A3 | A paused console still drops its shown rows as the ring turns over: the pop below the ring's first record runs paused or not. 01's kit trace logs at INFO and filled 512 lines within 290 frames, so pause empties the view it was meant to hold | `crates/tungsten/src/debug_views.rs:2192-2200`; `planning/e01-all-views-1280x720-os1.75.png` (`512 lines`) |
| A4 | The entity pick boxes a sprite at its `Transform`, missing a sheet frame's anchor (`D-231`) and a `ParallaxLayer`'s remap, both of which move where the extract draws it | `crates/tungsten/src/inspector.rs:307-320`; `crates/tungsten/src/sprite_extract.rs:816-836` |
| A5 | `run_stage` pushes `name.to_string()` per system per frame into a fresh `Vec::with_capacity(schedule.len())`; a frame with no fixed step pushes the fixed names the same way. A default app's 24 systems are 25 allocations a frame; 02's `churn` pin reads 30 a frame (1,800 over 60) | `crates/tungsten/src/app.rs:1906-1911`, `:1935-1939`, `:2072`; `examples/02_bench/src/tests/guards/churn.json`; `crates/tungsten/src/tests/plugins.rs:9-13` |
| A6 | The dispatcher writes `HudActiveState` with `to_string()` every frame; no built-in HUD row reads it (`tests/debug_hud.rs:154`), and `DESIGN.md:291` still names an M18 state row | `crates/tungsten/src/state.rs:422-431` |
| A7 | `std::env::var("TUNGSTEN_PERF_LOG")` every frame, a known issue | `crates/tungsten/src/app.rs:2625`; `docs/known-issues.md:130` |
| A8 | While any view shows, `update_with` removes and re-inserts the tree, the places and each view resource, each insert a new box (the tree's 3,568 bytes copied), and clones each view's font family into its spec | `crates/tungsten/src/debug_views.rs:1636-1742`, `:1843-1856`; `crates/tungsten-core/src/ecs/resource.rs:17` |
| A9 | The HUD's graph changes every frame it shows, and `set_mesh` marks paint, so the whole UI repaints every frame | `crates/tungsten/src/debug_hud.rs:254-266`; `crates/tungsten-core/src/ui/tree.rs:453-472` |
| A10 | Stripes at alpha 2 do not show at either scale; the title veil at 5 hardly does, and a heavier one fails 4.5:1 for `RED` (alpha 8: 4.40:1) | `debug_views.rs:73`, `:76`; `tests/debug_views.rs:1011-1023`; planning captures |
| A11 | A body scrolled by part of a line shows its top row's glyph bottoms against the fixed header: the console at 1.0 | `planning/fixtures-debug-1280x720-os1.png`; the dock's heights, `debug_views.rs:1048-1113` |
| A12 | The console's controls line runs five filters, `pause`, `clear` and its status together, one 6-unit gap apart | `debug_views.rs:771-793`; planning captures |
| A13 | At 1280×720 on 1.75 the overlay cuts names at 15 characters (`systems_overlay…`): its numeric columns are as wide as their headers `avg ms`, `max ms` and `% frame` | `crates/tungsten/src/systems_overlay.rs:250`; `planning/fixtures-debug-1280x720-os1.75.png` |
| A14 | The dock reserves six lines and a margin for each window still to dock on a side, whatever it asked for. In 01 at 1280×720 on 1.75, the HUD at the top right (S-123) above the overlay and a UI inspector with nothing picked keeps its graph and no row | `debug_views.rs:1063-1080`; `planning/e01-all-views-1280x720-os1.75.png` |
| A15 | The UI inspector's header repeats its first row's ID; its rows show no key and no named text style | `debug_views.rs:2446-2452`, `:2532-2543`, `:2557-2569` |
| A16 | F1's outlines are `thickness` world units (`crates/tungsten-render/src/debug_line.wgsl:34`, `app.rs:3107-3131`), so their width on screen follows the camera's zoom: 01 zooms by the window's height | `crates/tungsten/src/physics_debug.rs:28-29`, `:40`; `examples/01_platformer/src/features/level/systems.rs:30-37` |
| A17 | Hand-rolled where `D-235` has the part: `restyle` is `UiTree::edit_layout`; the bars are `ui::widgets::meter`; no view root snaps to pixels; no view widget has a key | `debug_views.rs:1221-1229`, `:1457-1514`; `crates/tungsten-core/src/ui/widgets.rs:348-381`; `crates/tungsten-core/src/ui/tree.rs:1385`, `:1561` |
| A18 | `UiHarness::assert_fits` holds every root to the tree's viewport, so a debug root at a debug scale below 1 reads as outside it | `crates/tungsten/src/testing/ui.rs:296-322` |
| A19 | The overlay groups systems by stage only, and its rows re-sort at every refresh | `systems_overlay.rs:244-301` |
| A20 | The zoom keys and places show no defect in the reading: the zoom moves in eighths, saves at once and re-places a saved window from its logical place when the debug scale moves; §2.1's zoom and step configurations check them on screen | `debug_views.rs:996-1037`, `:1755-1841` |
| A21 | `DESIGN.md`'s render path still has the extract add "the HUD, the systems overlay and inspector text", which ended at M57 | `DESIGN.md:107` |

## 2. Design

### 2.1 The sheet

- **Route.** `target/debug/tungsten run --script` from `examples/01_platformer/` (01: entities to pick, bodies for F1, a UI for F7 and F5, a chatty log). The OS scale comes from `WINIT_X11_SCALE_FACTOR` (the planning captures used 1 and the display's own 1.75), the size from the script's `resolution`, a zoom from `--user-dir <scratch>` holding `settings/debug_windows.json` as `{"version": 1, "windows": {}, "zoom": z}`. A view opens or closes by a `press` of its action and a `release` a frame later (`engine_toggle_hud`, `engine_toggle_systems_overlay`, `engine_toggle_inspector`, `engine_toggle_ui_inspector`, `engine_toggle_log_console`, `engine_toggle_physics_debug`, `engine_toggle_ui_bounds`); a pick is a `cursor` step and a press of `engine_inspector_pick` (the player at world `[424, 2290]`; 01's health bar for F7). States start after 01's 4 s area banner (frame 250). A script pins dt but the views refresh on the frame interval, which is wall time (`app.rs:2555`, `debug_views.rs:2271`), and an uncapped debug run can pass 40 frames inside 500 ms (the planning capture's HUD statistics still held one sample at frame 290). So each capture run sets `TUNGSTEN_DISPLAY_FRAME_RATE_CAP=60`, and each state is captured 60 frames after its change, at least a second. `planning/e01-views.json` is the starting script.
- **Configurations.** C1, 1280×720 at 1.75 (step 3/4): (a) all five and F1, (b) the HUD alone, (c) the overlay alone, (d) the inspector with the player picked, (e) the console alone, (f) the UI inspector with the health bar picked, (g) F1 alone, (h) F5 over the console and the overlay. C2, 1600×900 at 1.75 (step 7/8); C3, 1920×1080 at 1.75 (step 1); C4, 1280×720 at 1.0; C5 to C7, C1 at zoom 1/2, 3/2 and 2: each (a) only. Fourteen stills a side; the after side adds C1 (i), the overlay's flat view.
- **Files** in `perf-runs/20261102-debug-pass/`: `shoot.py` writes the scripts, runs them, checks the stills (`shoot.py check before|after`: the count and each size by `magick identify`, exit 0) and builds `sheet/<config>.png`, before beside after with captions (`magick montage`), and `sheet/README.md`, one line per image saying what to look at; `before/`, `after/`, `timing-before.md`, `timing-after.md`.

### 2.2 The bugs

Each lands with its test, run first at the step's base and failing; the evidence row quotes the failing assertion.

- **F5 (A1).** `UiTree::paint` keeps a node's bound only while `show_bounds` is on, with the clip index its node paints in, and draws each after every other command in that clip; a bound wholly outside its clip draws none. With F5 off a repaint pushes no bound, so 01's `pit` pin and the views' open pin fall by the list's growth allocations per repaint.
- **The HUD (A2).** The closed branch of `update_window` clears the samples and the graph's buffers (capacity kept) and resets the averages and `seeded`, as a new HUD starts.
- **The console (A3).** While paused, shown rows stay: no pop below the ring's first record, and a level filter's press marks its button at once but reaches the rows only at resume, since a rebuild past the pause point would find no record to show (`debug_views.rs:2159-2161`, `:2182-2191`, `:2200`); `clear` still clears. Resume rebuilds the rows from the ring.
- **The pick (A4).** A sprite's box sits where the extract draws it: `parallax_world_position(position, scroll_factor, camera.position)` under a `ParallaxLayer`, then the frame's anchor through `sprite_extract::anchor_offset` (made `pub(crate)`), mirrored under `SpriteFlip` as `sprite_extract.rs:826-835` does. Rotation stays out, as now: the unrotated box.

### 2.3 The look, inside `D-168`

- **Stripes (A10)** at alpha 4: the worst text contrast falls from 4.99:1 to 4.61:1, `RED` on a level badge over a white scene, composited in linear light as the test does. The title veil stays 5.
- **Rules (A10, A11).** A `BORDER` rule one physical pixel tall (`outline_px` over the unit, as the border) under the title bar and under a fixed header, so the chrome reads as edges.
- **Controls line (A12).** The filters, a gap of three insets, `pause` and `clear`, then a growing spacer and the status at the line's end.
- **The overlay's headers (A13)** read `avg`, `max` and `%` (the title carries `ms`), so each numeric column is as wide as its widest value.
- **Whole lines (A11).** A scroll panel clips its children to its whole box, padding included (`crates/tungsten-core/src/ui/paint.rs:1060-1068`), so the body's 3-unit top and bottom padding (`debug_views.rs:797-806`) would show a sliver of the row above at its end. The body takes no vertical padding; those spaces move outside it, to the root's bottom padding and the space under the rule above it, and the dock's asked-for height counts them. A docked window whose body scrolls then takes the largest height at or under the one it was given at which the body is k lines tall: its chrome (everything but the body) and k lines. At its top and following its end the body shows whole rows.
- **Reservations (A14).** The dock reserves for each later window on a side what it asked for at the last layout, at most six lines, and a margin; before its first layout, six lines, as now.
- **F1 (A16).** `PhysicsDebugOverlay::thickness` reads as OS-logical pixels on screen: the emit passes `thickness × ScaleFactor::os() / zoom.max(f32::EPSILON)` world units, the projection's own effective zoom (`crates/tungsten-core/src/camera.rs:216`), so a zero, negative or NaN zoom gives a finite, positive width; the factors are 1 without the resources.
- **Unchanged:** the corners and strip, 15 on 20 and 17 on 22, the weights, the hues, the panel alpha 252, the 960×540 floor and steps, the zoom's range, the title veil. These are values inside `D-168`'s rules (one palette at 4.5:1 in linear light, docked without overlap at 960×540 units or more), so no decision (Q2).

### 2.4 The UI pass

Moves, in Step 11:

- `restyle` becomes `UiTree::edit_layout`: the same copy, compare and mark.
- The overlay's bars on `ui::widgets::meter` and `Meter::set` (a share of the track, NaN as 0); the view sets its overrides on the track and the fill after the build, since `meter` paints the fill in the theme's accent.
- Keys (`UiTree::set_key`): each root by its place name (`hud`, `systems`, `inspector`, `log`, `ui_inspector`); `collapse` and `close`; the console's `error`, `warn`, `info`, `debug`, `trace`, `pause` and `clear`; the overlay's `view` and `pause` from Step 12 on. Tests find controls by key. Dispatch stays by widget ID: each view holds its IDs, and the keys repeat across roots.
- Pixel snapping (`set_pixel_snap`) on every view root: borders, stripes, rules and bars on whole pixels at 1.3125 pixels a unit (1.75 × 3/4).
- The UI inspector (A15): the header names the pick, `Label 2:0 · Game`; rows gain `key` and `style` (`UiTree::key_name`, `UiTree::text_style`), `-` when unset; the `widget` row keeps role and root.
- `UiHarness::assert_fits` (A18) holds each root to its layer's viewport (`UiTree::layer_viewport`), so the views' fit is checked at a debug scale.

Stays hand-rolled:

- Named text styles, shrinking text, outlines and shadows: they come from the game's theme file (`D-235`), which must not reach a view (`D-165`). The views keep full overrides and one-line cells ending in `…` (`D-168`).
- `ui::widgets`' text, button, row, column and group builders: they read the theme's spacing tokens and named styles, the same leak.
- `tungsten::screens`, `ModalScreen` and screen files (`D-236`): a screen is a state the stack covers, freezes and fades (`D-163`), and a file is data a game edits; the views stay over every state and keep their own look.
- Held clocks and game-clock tracks: the views refresh on the frame interval, so they keep reading while a game holds its clock. Nothing moves.
- Bare groups for the containers: they would keep a theme out too, but the theme test checks every node's own overrides (`tests/debug_views.rs:161-280`), a stronger guard than paint output.

None of these moves changes a constraint in `D-165` to `D-168`, so no decision.

### 2.5 Overhead

**Measured** three ways:

- **Two counter guards** (Step 1) in the umbrella's test binary, which installs `CountingAlloc`: `debug_views::tests::views_guard_closed` (a default app, every view off, a 30-frame warm-up and a 60-frame window, pin `crates/tungsten/src/tests/guards/views-closed.json`) and `views_guard_open_idle` (all five views on, each refresh interval `f32::INFINITY`, so each refreshes once and then idles; a 300-frame warm-up past the graph's 240 samples, a 60-frame window, pin `views-open.json`). Each warms up through `UiHarness::step`, which lays out every frame, so the windows dock on real rects, then reads its window with `CounterGuard::measure(ui.harness_mut(), 0, 60)` (the console's feed detached, the harness counting only) and `check`s the pin. The open guard then steps 60 more `UiHarness` frames and records the roots they lay out (`UiHarness::layout_totals`); a nonzero count joins §1 as a bug and is fixed in Step 11 with a zero assertion, no pin, as `D-235`'s idle-screen guard.
- **The existing guards** already carry A5 (§1).
- **An informational timing** before (Step 1) and after (Step 13): 01 in release (`tungsten run --release --script`), `TUNGSTEN_PERF_LOG=1` and `RUST_LOG=tungsten::app=debug`, 120-frame segments per state (closed, each view alone, all five, F1, F5), the p50 of `update`, `extract` (which holds the views and the UI layout) and `total` per segment through `scripts/perf_stats.py`'s `parse_log`, and the debug systems' `systems:` times. Not a sitting and no verdict; a running `nxcodec.bin` is noted beside the numbers.

**Cut**, the closed costs, which every frame of every game pays:

- **A5 (Step 6).** The app keeps the frame's timings in a vector of its own and swaps it with `FrameTimings::system_timings` at telemetry; `run_stage` writes through a cursor that sets an entry's time, rewrites its name only when it differs (reusing the `String`), pushes past the end and truncates at the end; the zero-step path writes 0 ms the same way. Once warm, timings allocate nothing. `system_timings` keeps its type and contract (each name once, in run order, a fixed system's runs summed, 0 ms with no step) and the views read the previous frame's, as now.
- **A7 (Step 6).** `TUNGSTEN_PERF_LOG` read once in `App::new`; its three readers use the kept flag.
- **A6 (Step 7).** The dispatcher rewrites `HudActiveState` only when the top state's ID differs, in place.
- **A1's list (Step 2).**

**Kept**, pinned by the open guard so they cannot grow: A8's boxes and clones, A9's repaint, and the eight debug systems' clock reads and action lookups (`D-165` names the systems, and a game may order against them; Q5). A cost the informational timing puts over 0.1 ms of release `extract` p50 goes to Follow-ups with its numbers.

**Pins.** Step 2 moves 01's `pit` and the views' open pin; Step 6 every frame-stepping pin (02's but `boot`, 01's `pit`, both views' pins), each allocations by 60 × (S + 1), S the app's systems outside `Startup`, and bytes; Step 7 01's `pit` by 60 allocations. Each is rewritten with `TUNGSTEN_GUARDS_WRITE=1` on its test, then run twice without it. Under `examples/` those are JSON pins only, no code (Q1). The plan claims fewer allocations, not frame time: no sitting (`D-226`).

### 2.6 The systems overlay

- **Title-bar buttons.** `Shape::buttons`, a count of word buttons a view labels itself, built after the key and before collapse and close: not focusable, the glyph hover colour, but sized to their labels, body-size text with `CELL_PAD` a side, since `title_button`'s 22-unit square in title-size text holds a glyph and would cut `stages` or `resume` (`debug_views.rs:1196-1211`). The caption gives way first: its width cap already takes what the rest of the title bar leaves.
- **The flat view (A19).** The first button reads `flat` while grouped and `stages` while flat. Flat lists every system by its average per-frame total (a fixed system's runs are summed in `FrameTimings`), slowest first, ties by name, under the column header row, each name in its stage's hue (`stage_hue`), no group rows; the title keeps the total. Grouped by default; not saved, as collapse is not (`D-166` unchanged).
- **Pause.** The second button reads `pause` and `resume`, as the console's: the rows and the title hold, the averages run on, and resume refreshes at once.
- **Taken:** the flat view and pause, since flat rows re-sort at each refresh and pause lets them be read. **Parked** in Later, a line each, in the plan's commit: sort by max; a name filter; run counts; a cumulative share column; a dump of the table to the log; idle systems folded into one row. No key for the switch (Q4).

### 2.7 Checks

- `LIST <package> <test path>…` runs this script (`python3 -B - <package> <test path>… <<'EOF'`), exit 0 when each named test is in the package's library listing; a filter that matches nothing still exits 0, so each new test is listed. Tried at planning: 864 tests listed, a made-up name refused.

  ```python
  import subprocess, sys
  package, names = sys.argv[1], sys.argv[2:]
  out = subprocess.run(["cargo", "test", "-p", package, "--lib", "--locked", "--", "--list"], capture_output=True, text=True).stdout
  have = {line[: -len(": test")] for line in out.splitlines() if line.endswith(": test")}
  missing = [name for name in names if name not in have]
  print(len(have), "tests listed; missing:", missing or "none")
  sys.exit(1 if missing else 0)
  ```

- `PINS <path>…` runs this script the same way, exit 0 when each pin's `allocations` and `allocated_bytes` fell or held by multiples of 60 against `HEAD` and every other counter held, printing each change. Tried at planning: unchanged pins pass; 01's `pit` against `79c0f268` (+104 bytes) fails.

  ```python
  import json, subprocess, sys
  bad = False
  for path in sys.argv[1:]:
      old = json.loads(subprocess.run(["git", "show", f"HEAD:{path}"], capture_output=True, text=True, check=True).stdout)
      with open(path) as pin:
          new = json.load(pin)
      delta = {key: new[key] - old[key] for key in old}
      print(path, {key: value for key, value in delta.items() if value})
      for key, value in delta.items():
          if key in ("allocations", "allocated_bytes"):
              bad |= value > 0 or value % 60 != 0
          else:
              bad |= value != 0
  sys.exit(1 if bad else 0)
  ```

- Test filters are module paths, each checked at planning to match tests: `debug_views::tests::`, `debug_hud::tests::`, `systems_overlay::tests::`, `inspector::tests::`, `physics_debug::tests::`, `telemetry::tests::`, `state::tests::`, `testing::ui::tests::`, `testing::fixed_step::`, `plugins::tests::` (all `-p tungsten --lib`), `ui::paint::tests::` (`-p tungsten-core --lib`). Two or more filters go after `--`. 01's guard is `cargo test -p example-01-platformer --locked --test game guards::`, 02's `cargo test -p example-02-bench --locked guards_`.
- Every code step: `cargo fmt --all -- --check` → exit 0 and `just lint` → exit 0. Size-dependent tests run at 1280×720 on 1.75 (`sized(app(), 1280, 720, 1.75)`), and Step 9's also at 1600×900 on 1.75 and at zoom 2.

## 3. Steps

### Step 1: The baseline

- **Files:** `crates/tungsten/src/tests/debug_views.rs`, `crates/tungsten/src/tests/guards/views-closed.json`, `crates/tungsten/src/tests/guards/views-open.json` (new), `docs/perf/benchmarks.md` (two rows in the guard table); untracked `shoot.py`, `before/`, `timing-before.md`.
- **Change:** before any engine change, the before sheet (§2.1), the two guards and their pins (§2.5), the idle window's layout count, and the informational timing; `status: in progress`.
- **Done-when:** `TUNGSTEN_GUARDS_WRITE=1 cargo test -p tungsten --lib --locked debug_views::tests::views_guard` once, then the same without the variable twice → exit 0; `LIST tungsten debug_views::tests::views_guard_closed debug_views::tests::views_guard_open_idle` → exit 0; `python3 -B perf-runs/20261102-debug-pass/shoot.py check before` → exit 0, 14 stills; `timing-before.md` holds nine segment rows; the idle window's `layout_totals` quoted; `python3 -B scripts/check-repo.py` → exit 0; fmt and lint (§2.7).
- **Commit:** `test(debug-views): pin the views' closed and idle-open frames`
- **Moves:** none; two new pins.

### Step 2: F5 outlines only what each clip shows

- **Files:** `crates/tungsten-core/src/ui/paint.rs`, `crates/tungsten-core/src/tests/ui/paint.rs`, `DESIGN.md` (the `UiDebug` sentence, `:446`), `examples/01_platformer/tests/game/guards/pit.json`, `crates/tungsten/src/tests/guards/views-open.json`.
- **Change:** §2.2's F5: bounds kept only while shown, each in its node's clip, none for a node wholly outside it; `UiDebug::show_bounds`' doc says so.
- **Done-when:** `ui::paint::tests::bounds_outline_only_what_their_clips_show` (a scroll panel with ten rows in a box three rows tall: each outline carries its node's clip, no outline for a row wholly past the box) fails at the base and passes; `cargo test -p tungsten-core --lib --locked ui::` → exit 0; `LIST tungsten-core ui::paint::tests::bounds_outline_only_what_their_clips_show` → exit 0; the two pins rewritten (§2.5), then 01's guard and `debug_views::tests::views_guard` → exit 0; `PINS examples/01_platformer/tests/game/guards/pit.json crates/tungsten/src/tests/guards/views-open.json` → exit 0, both allocation counts lower; fmt and lint.
- **Commit:** `fix(ui): outline bounds in their clips and only while shown`
- **Moves:** 01's `pit` and `views-open.json`, allocations and bytes, the bounds list's growth per repaint.

### Step 3: A reopened HUD starts afresh

- **Files:** `crates/tungsten/src/debug_hud.rs`, `crates/tungsten/src/tests/debug_hud.rs`.
- **Change:** §2.2's HUD.
- **Done-when:** `debug_hud::tests::a_reopened_hud_starts_its_graph_and_averages_afresh` (30 frames at 50 ms, closed a frame, reopened at 10 ms: the statistics' most is 10 ms, the graph holds only the new bars, the fps row reads 100.0) fails at the base and passes; `cargo test -p tungsten --lib --locked debug_hud::tests::` → exit 0; `LIST` of the new test → exit 0; fmt and lint.
- **Commit:** `fix(debug-hud): start a reopened hud's graph and averages afresh`
- **Moves:** none.

### Step 4: A paused console keeps its rows

- **Files:** `crates/tungsten/src/debug_views.rs`, `crates/tungsten/src/tests/debug_views.rs`.
- **Change:** §2.2's console.
- **Done-when:** `debug_views::tests::a_paused_console_keeps_its_rows_while_the_ring_turns_over` (capacity 8, eight rows shown, paused, twenty more records and a refresh: the same eight rows; a level filter pressed then: the same eight rows, its button marked; resume: the ring's last eight that pass the filters) fails at the base and passes; `cargo test -p tungsten --lib --locked debug_views::tests::` → exit 0; `LIST` → exit 0; fmt and lint.
- **Commit:** `fix(debug-views): keep a paused console's rows as its ring turns`
- **Moves:** none.

### Step 5: The pick finds a sprite where it draws

- **Files:** `crates/tungsten/src/inspector.rs`, `crates/tungsten/src/sprite_extract.rs` (`anchor_offset` `pub(crate)`), `crates/tungsten/src/tests/inspector.rs`.
- **Change:** §2.2's pick.
- **Done-when:** `inspector::tests::pick_finds_a_parallax_sprite_where_it_draws` (scroll factor (0.5, 0.5), the camera at (200, 0): a press where it draws picks it, one inside its `Transform` box only does not) and `pick_finds_an_anchored_sprite_where_it_draws` (a frame anchored at (8, 16) through `AssetRegistry::set_sprite_frame`, flipped and not) each fail at the base and pass; `cargo test -p tungsten --lib --locked inspector::tests::` → exit 0; `LIST` of both → exit 0; fmt and lint.
- **Commit:** `fix(inspector): pick a sprite where its anchor and parallax draw it`
- **Moves:** none.

### Step 6: The timing path allocates nothing once warm

- **Files:** `crates/tungsten/src/app.rs`, `crates/tungsten/src/tests/telemetry.rs`, `docs/known-issues.md` (`:130`'s first half), `examples/02_bench/src/tests/guards/*.json` but `boot.json`, `examples/01_platformer/tests/game/guards/pit.json`, `crates/tungsten/src/tests/guards/views-closed.json`, `views-open.json`.
- **Change:** §2.5's A5 and A7.
- **Done-when:** `telemetry::tests::a_system_adds_no_allocation_to_a_warm_frame` (two default apps, one with eight more no-op systems: equal allocations over a 60-frame window after a 30-frame warm-up) fails at the base by 480 and passes; `cargo test -p tungsten --lib --locked -- telemetry::tests:: testing::fixed_step:: plugins::tests::` → exit 0 (`system_timings`' contract, `tests/fixed_step.rs:686`, `tests/plugins.rs:169`); `rg -c 'env::var\("TUNGSTEN_PERF_LOG"\)' crates/tungsten/src/app.rs` → `1`; every moved pin rewritten (§2.5), then 02's and 01's guards and `debug_views::tests::views_guard` → exit 0; `PINS` over the moved pins → exit 0, each allocation drop quoted as 60 × (S + 1) with S per app; `boot.json` unchanged (`git diff --quiet -- examples/02_bench/src/tests/guards/boot.json` → exit 0); fmt and lint.
- **Commit:** `perf(app): reuse the frame's system timing names`
- **Moves:** every frame-stepping guard pin, allocations and bytes only.

### Step 7: The dispatcher writes `HudActiveState` only on a change

- **Files:** `crates/tungsten/src/state.rs`, `crates/tungsten/src/tests/state.rs`, `DESIGN.md` (`:291`), `examples/01_platformer/tests/game/guards/pit.json`.
- **Change:** §2.5's A6; `DESIGN.md:291` says the dispatcher mirrors the top state's ID for a game's HUD rows, written on a change, and drops the M18 state row.
- **Done-when:** `state::tests::hud_active_state_keeps_its_buffer_while_the_top_state_stays` (its pointer equal across three frames; a replace writes the new ID) fails at the base and passes; `cargo test -p tungsten --lib --locked state::tests::` → exit 0; `LIST` → exit 0; 01's pin rewritten, then 01's guard → exit 0; `PINS examples/01_platformer/tests/game/guards/pit.json` → exit 0, allocations −60; 02's guards unmoved (`cargo test -p example-02-bench --locked guards_` → exit 0 with no rewrite); fmt and lint.
- **Commit:** `perf(state): write the hud's active state only when it changes`
- **Moves:** 01's `pit`, 60 allocations and their bytes.

### Step 8: The look

- **Files:** `crates/tungsten/src/debug_views.rs`, `crates/tungsten/src/systems_overlay.rs`, `crates/tungsten/src/tests/debug_views.rs`, `crates/tungsten/src/tests/systems_overlay.rs`, `crates/tungsten/src/tests/guards/views-open.json`.
- **Change:** §2.3's stripes, rules, controls line and headers.
- **Done-when:** `debug_views::tests::every_palette_text_colour_keeps_4_5_to_1_composited_in_linear_light` pins 4.61:1, `RED` on the badge over white; `every_window_draws_a_rule_under_its_title_bar_and_fixed_header` (a `BORDER` node one physical pixel tall at 1.0 and at 1.75, step 3/4); `the_console_status_ends_its_controls_line` (the status's right edge on the line's; three insets between `TRACE` and `pause`); `systems_overlay::tests::each_numeric_column_is_as_wide_as_its_widest_value` and the updated `the_title_carries_the_total_and_the_headers_name_the_columns` → each exit 0, the new ones listed by `LIST`; `cargo test -p tungsten --lib --locked -- debug_views::tests:: systems_overlay::tests:: debug_hud::tests:: inspector::tests::` → exit 0; `views-open.json` held, or rewritten with its cause quoted; fmt and lint.
- **Commit:** `fix(debug-views): heavier stripes, rules and a tidier controls line`
- **Moves:** the contrast test's pinned worst case; each window's height by its rules; `views-open.json` only with a quoted cause (§5).

### Step 9: The dock

- **Files:** `crates/tungsten/src/debug_views.rs`, `crates/tungsten/src/tests/debug_views.rs`, `DESIGN.md` (the M58 docking sentence, `:544`).
- **Change:** §2.3's whole lines and reservations.
- **Done-when:** `debug_views::tests::a_scrolling_docked_body_shows_whole_lines` (at 1280×720 and 1600×900 on 1.75 and at zoom 2, every docked body that scrolls, at its top and following its end: each row's visual rect lies wholly inside the body's or wholly outside it, within 1e-3) and `a_crowded_side_reserves_only_what_later_windows_ask` (at 1280×720 on 1.75, the HUD at the top right over the overlay and a UI inspector with nothing picked: the HUD's body shows at least three rows; no overlap) fail at the base and pass; `docked_windows_never_overlap_through_a_collapse_an_expansion_and_a_growing_row` → exit 0; `a_window_alone_on_its_side_reaches_60_percent` now reads the largest whole-line height at or under 60% → exit 0; `cargo test -p tungsten --lib --locked -- debug_views::tests:: debug_hud::tests::` → exit 0; fmt and lint.
- **Commit:** `fix(debug-views): dock in whole lines and reserve what windows ask`
- **Moves:** docked heights: by less than a line from the rounding, and on a crowded side by the reservation, which gives a side's earlier windows the room later ones did not ask for; no overlap either way.

### Step 10: F1's outlines a fixed width on screen

- **Files:** `crates/tungsten/src/physics_debug.rs`, `crates/tungsten/src/tests/physics_debug.rs`.
- **Change:** §2.3's F1; the field's doc names the unit.
- **Done-when:** `physics_debug::tests::outlines_keep_their_width_on_screen_at_any_zoom` (zoom 4 on 1.75: every command's thickness 1.5 × 1.75 / 4; zoom 1 on 1.0: 1.5; zoom 0, −1 and NaN: finite and positive, as `1.5 / f32::EPSILON`) fails at the base and passes; `cargo test -p tungsten --lib --locked physics_debug::tests::` → exit 0; `LIST` → exit 0; fmt and lint.
- **Commit:** `fix(physics-debug): keep outlines a fixed width on screen`
- **Moves:** none.

### Step 11: The UI pass

- **Files:** `crates/tungsten/src/debug_views.rs`, `crates/tungsten/src/testing/ui.rs`, `crates/tungsten/src/tests/debug_views.rs`, `crates/tungsten/src/tests/testing_ui.rs`, `crates/tungsten/src/tests/guards/views-open.json`, `DESIGN.md` (the M57 and M58 sections).
- **Change:** §2.4's moves, in its order; the idle-layout assertion if Step 1 found layouts.
- **Done-when:** `rg -q 'fn restyle' crates/tungsten/src/debug_views.rs; test $? -eq 1` → exit 0; `debug_views::tests::every_view_root_snaps_and_every_control_has_its_key`, `the_ui_inspector_names_the_picks_key_and_style`, `testing::ui::tests::assert_fits_holds_a_debug_root_to_its_layers_viewport` (fails at the base) and `debug_views::tests::the_views_fit_at_the_owners_scale` (`assert_fits` at 1280×720 on 1.75 with all five open) → exit 0, listed by `LIST`; `every_node_a_view_builds_keeps_its_look_under_a_game_theme` → exit 0; when Step 1 found the idle window laying out roots, `views_guard_open_idle` asserts it lays out none and passes; `cargo test -p tungsten --lib --locked -- debug_views::tests:: systems_overlay::tests:: testing::ui::tests::` → exit 0; `views-open.json` held or rewritten with its cause; fmt and lint.
- **Commit:** `refactor(debug-views): move the views onto the ui pass's keys and widgets`
- **Moves:** paint (snapped boxes); the UI inspector's height by its two new rows where it has room; `views-open.json` only with a quoted cause.

### Step 12: The systems overlay's flat view and pause

- **Files:** `crates/tungsten/src/debug_views.rs`, `crates/tungsten/src/systems_overlay.rs`, `crates/tungsten/src/debug_hud.rs` and `crates/tungsten/src/inspector.rs` (their `Shape` literals, `debug_hud.rs:214`, `inspector.rs:158`, take no button), `crates/tungsten/src/tests/debug_views.rs`, `crates/tungsten/src/tests/systems_overlay.rs`, `crates/tungsten/src/tests/guards/views-open.json`, `DESIGN.md` (`:544`).
- **Change:** §2.6; every `Shape` literal gains the count in this step (the console's and the UI inspector's in `debug_views.rs`, the overlay's two).
- **Done-when:** `systems_overlay::tests::the_flat_view_sorts_every_system_by_its_frame_total` (systems in three stages, a fixed one summed over two steps: one header row, every system slowest first across stages, names in their stages' hues), `the_view_button_switches_and_names_the_other_view`, `pause_holds_the_rows_and_resume_refreshes_at_once` and `debug_views::tests::the_title_bar_holds_a_views_buttons_before_collapse_and_close` (at 1280×720 on 1.75, each of `flat`, `stages`, `pause` and `resume` as wide as its whole label at the measure double's advance plus padding) → exit 0, listed by `LIST`; `the_views_fit_at_the_owners_scale` → exit 0 with the overlay's buttons; `cargo test -p tungsten --lib --locked -- debug_views::tests:: systems_overlay::tests::` → exit 0; `views-open.json` held or rewritten with its cause; fmt and lint.
- **Commit:** `feat(systems-overlay): a flat view by frame total and a pause`
- **Moves:** `views-open.json` only with a quoted cause.

### Step 13: Close

- **Files:** this plan (deleted), the files that cite it (`rg -l 'plans/debug-pass' --hidden -g '!.git'`), `ROADMAP.md`, `DESIGN.md` (`:107`), `docs/known-issues.md`; untracked `after/`, `sheet/`, `timing-after.md`.
- **Change:** the after sheet, its README and the informational timing after; the checks; follow-ups to known issues or Later; `debug-pass` out of Now and phase-7's line back to "7.10, the kit pass, is next"; the plan and its citations deleted.
- **Done-when:** `just check` → exit 0; `just smoke` → exit 0 (its `Debug views` row passes); `just visual` → exit 0, comparisons run; `just perf counters` → exit 0; `just api-check` → exit 0; `just repo-check` → exit 0; `git diff --check` → exit 0; `python3 -B perf-runs/20261102-debug-pass/shoot.py check after` → exit 0, 15 stills; `sheet/` holds seven montages and `README.md`.
- **Ends with:** the commits, the evidence, the sheet linked (C1 first, each image with what to look at), what the owner should try (F2's flat view and pause, F8's pause over 01's trace, F5 over a scrolled console), the next prompt.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | The timing fix moves every frame-stepping pin, 02's and 01's included, though the brief keeps `examples/` out beyond captures. | Rewrite those JSON pins with the guards' own write variable and touch no example code; without them the owner's lead stays unfixed. |
| Q2 | Do the stripes' alpha, the rules, whole-line heights or the reservation floor need a decision? | No: `D-168`'s rules hold, and `DESIGN.md` follows the values. A step that would break one of them stops (§6). |
| Q3 | Column headers in a fixed line that never scrolls, sized to the columns at the last layout? | No: the shorter headers and the flat view's slowest-first rows keep them in view at the top; a Later line if the after sheet says otherwise. |
| Q4 | A key for the flat view? | None: the title bar's button; no debug view takes focus (`D-167`). |
| Q5 | Fold the seven toggles and the pick into one system? | No: `D-165` names them, a game may order against them, and Step 6 takes their allocations; the timing quotes what they cost. |
| Q6 | F1's width in OS-logical pixels rather than physical? | OS-logical, so it matches the views at any OS scale; 1.5 stays the default. |

## 5. Invariants

- **May move:** `views-closed.json` (Step 6) and `views-open.json` (Steps 2 and 6 by allocations, Steps 8, 11 and 12 only with a quoted cause); 01's `pit` (Steps 2, 6 and 7) and 02's frame-stepping pins (Step 6), by allocations and bytes in multiples of 60; the contrast test's worst case (Step 8); window heights, never into an overlap: by what a step adds to a window's content (Step 8's rules, Step 11's two UI inspector rows), by under a line from whole-line rounding and on a crowded side by the reservation's redistribution (Step 9); paint positions of debug roots by under a pixel (Step 11).
- **Must not move:** 02's `boot` pin; every pin's counters other than allocations and bytes; the goldens (`just visual`); the GPU counter pins (`just perf counters`); `api/` (`just api-check`); the default schedule (`tests/plugins.rs`'s `DEFAULT_SCHEDULE`); `FrameTimings`' fields and `system_timings`' contract; `debug_windows.json`'s format; the console's 512 and 1,024 capacities.

## 6. Stop conditions

- A done-when check fails: restore the step's files from the last commit, mark the step skipped, and continue with steps that don't depend on it.
- A bug's test passes at the step's base: the bug is not reproduced; keep the test, change no code, mark its §1 row "not reproduced" in the evidence.
- A pin moves outside §5, or a counter other than allocations and bytes moves: stop the step.
- A change would break a constraint of `D-165` to `D-168` (a game's theme reaching a view, a text colour under 4.5:1, an overlap at 960×540 units or more, the places file's format): stop the step and record it.
- `just status` shows a perf capture or another session's build: wait before building or capturing.

## 7. Critique

Round 1: gpt-6-astra through `critique --raw --repo` at xhigh, 2026-11-02, on the working tree at `5478c501` with this plan and `ROADMAP.md`'s lines uncommitted. `critique: model gpt-6-astra, effort xhigh, context 872000, mode repo (output cap 4000), 368.8 s, tokens in 1594723 (cached 1495936), out 10376 (reasoning 5367), commands 26, output 335.3 KB (largest 42.1 KB), usage 5h 0% → 30%, week 7% → 12%`. Between the rounds the author also made Step 11 name the idle-layout assertion it lands when Step 1 finds layouts.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | A filter pressed while paused rebuilds the rows, and past the pause point the rebuild finds no record, emptying the frozen view | `debug_views.rs:2159-2161`, `:2182-2191`, `:2200` | Accepted | §2.2: while paused a filter's press marks its button and reaches the rows at resume; Step 4's test presses one after the rollover |
| 2 | `Shape::buttons` changes every `Shape` literal, two of them in files Step 12 did not list | `debug_hud.rs:214`, `inspector.rs:158`, `systems_overlay.rs:179`, `debug_views.rs:2131`, `:2420` | Accepted | Step 12 lists `debug_hud.rs` and `inspector.rs`, and its change names every literal |
| 3 | §5 and Step 9 let docked heights move by under a line, yet the reservation fix must give the HUD rows | Step 9's done-when; §5 | Accepted | Step 9's moves and §5 allow the reservation's redistribution, never into an overlap |
| 4 | Forty scripted frames need not span 500 ms: the script pins dt, but the views refresh on the wall-clock interval | `app.rs:2555`, `:3430`; `debug_views.rs:2271`; the planning capture's one-sample statistics | Accepted | §2.1: capture runs set `TUNGSTEN_DISPLAY_FRAME_RATE_CAP=60` (`crates/tungsten-core/src/config.rs:13`, paced at `app.rs:2714`) and capture 60 frames after each change |
| 5 | Dividing by `CameraState::zoom` skips the projection's `max(f32::EPSILON)`, so a zero, negative or NaN zoom gives an infinite, negative or NaN width | `crates/tungsten-core/src/camera.rs:216` | Accepted | §2.3 and Step 10: the emit divides by `zoom.max(f32::EPSILON)`; the test adds zoom 0, −1 and NaN |

Round 2, at xhigh the same day on the plan without §7 and the evidence log (`--from` the skill's `sed` cut), with a focus note naming round 1's five changes and the author's: `critique: model gpt-6-astra, effort xhigh, context 872000, mode repo (output cap 4000), 327.5 s, tokens in 1075087 (cached 989696), out 9313 (reasoning 5002), commands 46, output 250.2 KB (largest 23 KB), usage 5h 30% → 53%, week 12% → 15%`. No third round.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | Whole-line heights still show a sliver of the row above at the body's end: the body's 3-unit padding sits inside the scroll clip, and the height check passes regardless | `debug_views.rs:797-806`; `crates/tungsten-core/src/ui/paint.rs:1060-1068` | Accepted | §2.3: the body takes no vertical padding, its spaces move outside it; Step 9's test checks that each row's visual rect lies wholly inside or outside the body's, at the top and at the end |
| 2.2 | `title_button`'s fixed 22-unit square in title-size text would cut `stages` and `resume` | `debug_views.rs:1196-1211`, `:278-294` | Accepted | §2.6: word buttons sized to their labels in body-size text; Step 12's test checks each of the four labels whole at 1280×720 on 1.75 |
| 2.3 | Step 11's two UI inspector rows grow its docked height, which §5 allowed only in Step 9 | `debug_views.rs:966-982` | Accepted | §5 and Steps 8 and 11 allow heights to follow added content, never into an overlap |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-11-02 | passed | `TUNGSTEN_GUARDS_WRITE=1 … views_guard` once, then twice without it → exit 0, `2 passed` each: `views-closed.json` 1,740 allocations and 230,040 bytes (29 a frame), `views-open.json` 3,840 and 5,033,340 (64 a frame), every other counter 0; `LIST` → `866 tests listed; missing: none`, exit 0; `shoot.py check before` → `14 stills expected, 14 right; problems: none`, exit 0; `timing-before.md` nine rows, release `extract` p50 0.160 ms closed, 0.545 all five, 0.350 the console alone, 0.280 the overlay alone, `total` p50 3.000 and 4.190, the eight debug systems under 0.005 ms each, no `nxcodec.bin`; the idle window: `60 idle frames laid out 0 roots and 0 nodes`, so Step 11 lands no idle-layout assertion; `check-repo.py` → exit 0 (0 errors); fmt → exit 0; `just lint` → exit 0 | `perf-runs/20261102-debug-pass/before/`, `timing-before.md`, `shoot.py` |
| 2 | 2026-11-02 | passed | `bounds_outline_only_what_their_clips_show` at the base → FAILED, `the root, the panel and the three rows in its box; none past it`: 12 outlines, every one in clip 0, against 5 with the rows in clip 1; after → passes; `cargo test -p tungsten-core --lib --locked ui::` → `332 passed`, exit 0; `LIST tungsten-core …` → `1444 tests listed; missing: none`, exit 0; both pins rewritten, then `debug_views::tests::views_guard` and 01's `guards::` twice each → exit 0; `PINS` → `pit.json {'allocated_bytes': -487680, 'allocations': -420}` (7 and 8,128 a frame), `views-open.json {'allocated_bytes': -979200, 'allocations': -480}` (8 and 16,320 a frame), exit 0; fmt → exit 0; `just lint` → exit 0 | — |
| 3 | 2026-11-02 | passed | `a_reopened_hud_starts_its_graph_and_averages_afresh` at the base → FAILED, `the most`: left `"50.00ms"`, right `"10.00ms"`; after → passes; `cargo test -p tungsten --lib --locked debug_hud::tests::` → `15 passed`, exit 0; `LIST` → `867 tests listed; missing: none`, exit 0; `views_guard` both → exit 0, pins unmoved; fmt → exit 0; `just lint` → exit 0 | — |
| 4 | 2026-11-02 | passed | `a_paused_console_keeps_its_rows_while_the_ring_turns_over` at the base → FAILED, `held past the ring's turn`: left `[]`, right the eight `first n` rows; after → passes; `cargo test -p tungsten --lib --locked debug_views::tests::` → `41 passed`, exit 0 (the guards among them, pins unmoved); `LIST` → `868 tests listed; missing: none`, exit 0; fmt → exit 0; `just lint` → exit 0 | — |
| 5 | 2026-11-02 | passed | at the base both → FAILED: `pick_finds_a_parallax_sprite_where_it_draws` `where it draws`: left `None`, right `Some(Entity { index: 0, generation: 0 })`; `pick_finds_an_anchored_sprite_where_it_draws` `drawn`: the same; after → both pass; `cargo test -p tungsten --lib --locked inspector::tests::` → `23 passed`, exit 0; `LIST` of both → `870 tests listed; missing: none`, exit 0; fmt → exit 0; `just lint` → exit 0. The box takes the extract's `anchor_offset` with the sprite's rotation, so it is the quad's box before the shader's turn | — |
| 6 | 2026-11-02 | passed | `a_system_adds_no_allocation_to_a_warm_frame` at the base → FAILED, `eight more no-op systems`: left 2220, right 1740 (480 = 8 × 60); after → passes; `cargo test -p tungsten --lib --locked -- telemetry::tests:: testing::fixed_step:: plugins::tests::` → `30 passed` (4, 14 and 12), exit 0; `rg -c 'env::var\("TUNGSTEN_PERF_LOG"\)' crates/tungsten/src/app.rs` → `1`; 14 pins rewritten, then 02's `guards_`, 01's `guards::` and `views_guard` twice each → exit 0; `PINS` over the 14 → exit 0, each allocation drop 60 × (S + 1), S read by a probe of `FrameTimings::system_timings.len()` in `CounterGuard::measure`, then `guard.rs` restored from HEAD (`git diff --quiet` → exit 0): `churn` −1,200 (S 19), `ecs` −1,860 (30), `friction`, `gpu`, `gpu-throughput`, `particles`, `physics`, `slopes` and `terrain` −1,140 (18), `physics-sparse` and `saves` −1,080 (17), 01's `pit` −12,180 (202), `views-closed` and `views-open` −1,500 (24); bytes −48,180 to −534,480; `git diff --quiet -- …/boot.json` → exit 0; fmt → exit 0; `just lint` → exit 0, after an `allow(clippy::struct_excessive_bools)` with its reason on `App` (`perf_log` its fourth bool) and `Copy` on `FrameStageTimings` | — |
| 7 | 2026-11-02 | passed | `hud_active_state_keeps_its_buffer_while_the_top_state_stays` at the base → FAILED, `one buffer: [0x556a92b42270, 0x7f3ff00013e0, 0x556a92b42270]`; after → passes; `cargo test -p tungsten --lib --locked state::tests::` → `39 passed`, exit 0; `LIST` → `872 tests listed; missing: none`, exit 0; 01's pin rewritten, then 01's `guards::` twice → exit 0; `PINS …/pit.json` → `{'allocated_bytes': -480, 'allocations': -60}`, exit 0; 02's `guards_` with no rewrite → `12 passed`, exit 0; `views_guard` → exit 0, pins unmoved; fmt → exit 0; `just lint` → exit 0 | — |
| 8 | 2026-11-02 | passed | the contrast test pins 4.61:1, `RED on the badge over white` (the test's model: stripe alpha 2 → 4.99 on the title bar, 3 → 4.80, 4 → 4.61, 5 → 4.44), and M57's panel alpha 245 moves 3.81 → 3.61; `every_window_draws_a_rule_under_its_title_bar_and_fixed_header` (at 1.0 and 1.75), `the_console_status_ends_its_controls_line`, `each_numeric_column_is_as_wide_as_its_widest_value` and the updated `the_title_carries_the_total_and_the_headers_name_the_columns` pass; `cargo test -p tungsten --lib --locked -- debug_views::tests:: systems_overlay::tests:: debug_hud::tests:: inspector::tests::` → `93 passed` twice, exit 0; `LIST` → `875 tests listed; missing: none`, exit 0; `views-open.json` rewritten: `allocated_bytes` +35,520 (592 a frame), allocations held. Cause, probed: the nine rules' `Rect` commands in the frame's cloned paint list (559 → 568 commands × 48 bytes, 432) and the five view resources re-boxed every frame (A8) with `ViewWindow` 32 bytes larger (`rules` 24, `Controls`' second count 8; 160); with the rules out the reading was +9,600, and it stayed +9,600 with the spacer or the old headers back. `views-closed.json` held; fmt → exit 0; `just lint` → exit 0 | — |
| 9 | 2026-11-02 | skipped | `a_scrolling_docked_body_shows_whole_lines` → FAILED at its first configuration's end: `1280×720 at zoom 1, end: Some("HUD") row Rect { min: Vec2(6.0, 80.03572), max: Vec2(48.75, 95.03572) } in Rect { min: Vec2(6.0, 94.89285), max: Vec2(271.5, 244.89285) }`. The row straddles the body's top by 0.1429 tree units, a quarter physical pixel: core's `children_transform` rounds a scroll offset to whole physical pixels (`crates/tungsten-core/src/ui/scroll.rs:76-80`), and at 1.3125 px a unit a 20-unit line is 26.25 px, so an end offset of n − k lines is whole only when n − k is a multiple of 4 (8 at step 7/8, 2 at zoom 2); the fix lies in core, outside the step's files. The top passed, as did `a_crowded_side_reserves_only_what_later_windows_ask`, `docked_windows_never_overlap_through_a_collapse_an_expansion_and_a_growing_row` and the 60% test; `views-open.json` also moved (`allocated_bytes: 4021860 -> 4024740`, the per-frame `Dock` 4 bytes larger), which §5 does not allow in Step 9. Files restored from HEAD (`git diff --quiet` → exit 0, `views_guard` → exit 0); the step's diff kept as `step9-skipped.patch` (198 lines added, 25 removed). Steps 10 to 13 do not depend on it | `perf-runs/20261102-debug-pass/step9-skipped.patch` |
| 10 | 2026-11-02 | passed | `outlines_keep_their_width_on_screen_at_any_zoom` at the base → FAILED, `zoom 4 on 1.75: 1.5`; after → passes (zoom 4 on 1.75: 1.5 × 1.75 / 4; zoom 1 on 1.0: 1.5; zoom 0, −1 and NaN: `1.5 / f32::EPSILON`; no camera or scale: 1.5); `cargo test -p tungsten --lib --locked physics_debug::tests::` → `9 passed`, exit 0; `LIST` → `876 tests listed; missing: none`, exit 0; 01's `presentation::` (its F1 tests) → `24 passed`, exit 0; fmt → exit 0; `just lint` → exit 0 | — |

## Follow-ups

- The dock's whole lines and reservations (Step 9, skipped; A11, A14): core rounds a scroll offset to whole physical pixels (`crates/tungsten-core/src/ui/scroll.rs:76-80`), so a body of whole lines at 1.75 still shows up to half a pixel of a row at its end; rounding a debug body's offset to whole lines there, or a whole-pixel line on the debug layer, would let the step land from `perf-runs/20261102-debug-pass/step9-skipped.patch`, with `views-open.json` allowed to move by the `Dock`'s growth.

# Phase 6 milestone 59: the UI lab as a reference, a UI QA pass and windowed mode (UI lab reference and QA) — done

- **status:** done
- **goal:** A QA pass over the UI work since M54, capped at 15 confirmed findings, each library fix behind a failing test; windowed mode that keeps its size and layout through resizes, maximizing and the Windowed → Borderless → Exclusive → Windowed cycle, reproduced before it is fixed; and example 05 reworked into a reference for future projects: a clear module layout with each pattern documented, an inventory screen with example items, shorter transitions, and one consistent theme in which every screen, button and setting is centred and aligned.
- **non-goals:** A whole-repository review (the Phase 6 QA pass, `qa6`, keeps that). Perf captures, and any change to shared ECS or frame-loop code (§7). Promoting anything from the example to `tungsten-kit` (`D-155`) or into the library as a widget. New controls (§2.4's cuts), drag and drop between widgets, a grid or wrapping layout (`D-124`), a gamepad backend (known issues, M54 step 5). The template, the getting-started flow and the other examples, except where §3 names a line. Changing a core default without a decision (§7).
- **files to touch:** step 1: the QA surface of §2.1 and its tests, `docs/known-issues.md`, `docs/plans/1.0/w04-api-freeze.md`, `DECISIONS.md` and `docs/DECISION_INDEX.md` when a fix reverses a recorded rule; step 2: `examples/05_ui_lab/src/main.rs`, `examples/05_ui_lab/src/menus.rs`; step 3: `crates/tungsten/src/settings.rs`, `crates/tungsten/src/app.rs`, `crates/tungsten/src/display.rs`, `crates/tungsten/src/tests/settings.rs`, `crates/tungsten/src/tests/display.rs`, `examples/05_ui_lab/tests/screens.rs`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/known-issues.md`; step 4: `examples/05_ui_lab/src/` (`lib.rs`, `main.rs`, `menus.rs`, `screens.rs`, and the new `fixtures.rs`, `theme.rs`, `widgets.rs`, `hud.rs`, `labels.rs`, `controls.rs`, `budget.rs`), `examples/05_ui_lab/tests/screens.rs`; step 5: `examples/05_ui_lab/src/` (the new `inventory/mod.rs`, `inventory/items.rs`, `inventory/grid.rs`, `inventory/detail.rs`, and `lib.rs`, `main.rs`, `menus.rs`, `screens.rs`, `labels.rs`, `fixtures.rs`, `widgets.rs`, `theme.rs`), `examples/05_ui_lab/assets/manifest.json`, `examples/05_ui_lab/assets/ui/themes/ex05_default.json` (the `panel` kind's focused, hovered and pressed variants), `examples/05_ui_lab/assets/sprites/items/` (generated), `examples/05_ui_lab/tools/generate.py`, `examples/05_ui_lab/tools/README.md`, `examples/05_ui_lab/tests/screens.rs`; step 6: `examples/05_ui_lab/src/theme.rs`, `menus.rs`, `widgets.rs`, `screens.rs`, `inventory/mod.rs`, `inventory/detail.rs`; step 7: `examples/05_ui_lab/assets/ui/themes/ex05_default.json`, `examples/05_ui_lab/src/` (every module of steps 4–5), `examples/05_ui_lab/tests/screens.rs`; step 8: `examples/05_ui_lab/tests/fixtures/ui-visual.png`, `examples/05_ui_lab/tests/fixtures/ui-labels.png`, `examples/05_ui_lab/tools/README.md`; step 9: `docs/known-issues.md`, `docs/plans/1.0/backlog-1.x.md`, `docs/plans/1.0/w01-ui-text-suite.md`, `docs/plans/1.0/w04-api-freeze.md`, `DESIGN.md`, `docs/getting-started.md`, `api/` (`just api`), `CHANGELOG.md`, this plan; step 10: `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row, the README's four owned Now fields, this plan (moved to `docs/plans/archive/1.0/`). Scratch tools, logs and captures stay outside the tree or under `target/m59/` (git-ignored).
- **ordered steps:** (1) UI QA pass; (2) windowed mode: reproduce; (3) windowed mode: fix; (4) UI lab module layout and pattern docs; (5) inventory screen; (6) shorter transitions; (7) theme and alignment; (8) pixel fixtures and `just visual`; (9) records; (10) Release 0.79.
- **done-when:** every step's evidence row quoted, or the step marked skipped with its reason; workflow §5 closed (§8); the release checks pass after the cut; the preflight prints the command block and the post-merge block.

Candidate "UI lab reference and QA", the owner's request of 2026-10-15 after M58, with no implementation-plan card: inserted ahead of W6b and W6c as M58 (W1 M5 polish) was, so W6b and W6c become 0.80 and M60. Level B, milestone M59, release 0.79. Written 2026-10-15 at `1dea903`. Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs.

## Context digest

M54–M58 landed the UI's world labels and gamepad routing (`D-161`), toggles and sliders (`D-162`), state-owned screens, animation tracks, prompts and rebinding (`D-163`, `D-164`), and the debug views as windows (`D-165`–`D-168`). Each shipped through a plan critique, but no pass has read them together. The owner asks for five things in one release. First, a QA pass over that work. Second, example 05 reworked into the reference future projects copy, with an inventory screen. Third, shorter transitions. Fourth, a cleaner theme with every screen, button and setting aligned. Fifth, a fix for windowed-mode glitches they hit on their display.

What it stands on: the paint list and theme asset (`D-147`, `D-148`), routing (`D-149`), settings and `apply_settings` (`D-159`), display requests applied at a frame boundary (`D-043`), the kit's admission rule (`D-155`), and no approval stops (`D-150`). The owner's display is X11 under KWin at Xft.dpi 168, an OS scale of 1.75. A 1280×720 window holds 731×411 UI units there, and every capture states its scale.

What later stops expect: W12b with W1 M6 moves examples 01, 03 and 04 onto the template layout and the UI views, and will read this example's module layout as the pattern. The Phase 6 QA pass takes the findings past this pass's cap and the pre-M54 UI rows (UI F6, UI F8).

## 1. Audit

Read at `1dea903`. Paths are repository-relative; `core/`, `render/` and `tungsten/` abbreviate the crates' `src/`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | The lab is three files. `main.rs` holds the fixture enum, App setup, the debug fixture's scroll and the counter. `screens.rs` holds the HUD, the M46 window, the popup, the labels fixture, the damage numbers and M55's controls window. `menus.rs` holds the play, pause and settings states, the 100-widget budget menu and two fixture scripts. Module docs narrate milestones, not patterns | `examples/05_ui_lab/src/main.rs:1-35`, `:57-191`; `screens.rs:1-11`; `menus.rs:1-9`; `lib.rs:1-7` |
| A2 | The M46 window titled "Inventory" is a placeholder: a body text and Equip and Drop buttons, anchored at (-60, -20) from the centre. The popup ("Drop the lantern?") sits at (140, 70) over it, and its "Drop it" holds focus | `examples/05_ui_lab/src/screens.rs:88-185` |
| A3 | Buttons carry no padding, so their labels touch the border (`tests/fixtures/ui-visual.png`). Menu buttons stretch across their column (`Align::Stretch`) with start-aligned text, since neither the theme file nor the built-in button style sets `text.align`. A default `LayoutStyle` is a column that stretches its children, so a button's label spans the button | `examples/05_ui_lab/src/menus.rs:86-106`; `assets/ui/themes/ex05_default.json:17-20`; `core/ui/theme.rs:261-294`; `core/ui/style.rs:187-204`; `core/ui/theme_file.rs:347-405` (`text.align` accepted) |
| A4 | At 731×411 units, M55's 240-unit "Settings" window at the top end corner overlaps the M46 window (x 475–505, y 60–170) | `examples/05_ui_lab/src/screens.rs:90-97`, `:420-428` |
| A5 | The lab sets every UI duration: `FADE_SECS` 0.15 s each way, and a 0.2 s, 24-unit `QuadOut` slide-in. The engine sets no UI animation default. Its only UI timing is navigation repeat (0.5 s, then 0.1 s), which is not an animation | `examples/05_ui_lab/src/menus.rs:27-44`, `:108-114`; `core/ui/route.rs:155-162`; no other `UiTrack::`, `animate(` or `Transition::new` in `crates/` or `templates/` |
| A6 | `generate.py` writes two sprites with the standard library only and has a `--check` mode that no recipe runs. Every file under an example's `assets/` must be listed in its manifest. Layer 1 loads example manifests and checks IDs are unique across them | `examples/05_ui_lab/tools/generate.py:93-119`; `assets/manifest.json:1-9`; `scripts/check-repo.py:103-114`; `crates/tungsten-core/tests/manifests.rs:15-35`, `:67-123` |
| A7 | The theme file takes nine colour tokens, spacing, radii, fonts, text tokens and per-kind state styles. Core applies no spacing token to layout, and layout is not themable | `core/ui/theme_file.rs:148-260`; `core/ui/theme.rs:112-123` |
| A8 | Layout has no wrap or grid (`D-124`). A button holds children, and a scroll panel clips them and scrolls focus into view. Directional focus scores layout rects | `core/ui/node.rs:79-87`; `core/ui/scroll.rs:120-127`, `:175-221`; `core/ui/focus.rs:258-291`, `:349-362` |
| A9 | `apply_settings` copies the shipped or set resolution into the display request on every call. So any settings change (a volume step, a scale change, a mode switch) asks for 1280×720 in windowed mode, even after the player resized or maximized the window. `D-159` records this: "A window resized by hand returns to the set or shipped resolution at the next `apply_settings`". The lab's volume, scale and mode handlers all call it. A test pins "set, then unset, returns to the shipped size" | `tungsten/settings.rs:40-78`; `DECISIONS.md:2060`; `examples/05_ui_lab/src/menus.rs:494-510`, `:516-533`, `:566-574`; `tungsten/tests/settings.rs:86-112` |
| A10 | `apply_pending_display_request` calls `set_fullscreen`, then reads `inner_size()` at once. On X11 that still reports the old size, and `request_inner_size` returns `None` there, so the request path falls back to the same stale size. The result goes to `DisplayState` and `WindowSize` through `sync_display_state_and_telemetry`. `Resized` writes both again later. The UI lays out at `WindowSize / ScaleFactor`. A size request goes out in the same frame as `set_fullscreen(None)`, so it races the window manager's restore. winit's contract lets a platform apply a size at once and return `Some`, with no `Resized` to follow, as Wayland always does; the request may also un-maximize a maximized window | `tungsten/app.rs:798-895`, `:1255-1266`, `:2260-2274`; `tungsten/display.rs:103-154`; `winit-0.30.13/src/platform_impl/linux/x11/window.rs:1284-1296`; `winit-0.30.13/src/window.rs:763-777`; `winit-0.30.13/src/platform_impl/linux/wayland/window/mod.rs:313-318` |
| A11 | Exclusive fullscreen runs as borderless, with a warning on each request. `DisplayState` then says borderless while `Settings` says exclusive, and the settings button cycles from `Settings`. winit skips a fullscreen call equal to the current one and restores only the window position when fullscreen ends | `tungsten/app.rs:2099-2116`, `:821-832`; `examples/05_ui_lab/src/menus.rs:516-528`; `winit-0.30.13/src/platform_impl/linux/x11/window.rs:706-708`, `:740-750` |
| A12 | Known issues in scope: K1, an explicit focus neighbour in a frozen root takes focus (M56). K2, the UI inspector's first refresh can miss its pick's first layout (M58). Two known issues touch the display path. The P3 says a request writes `WindowSize` before the surface resizes. The P2 says a minimised or occluded window spins a core in `stage_pacing` | `core/ui/focus.rs:267-273` (`docs/known-issues.md:12`); `tungsten/debug_views.rs:2410-2416` (`:43`); `docs/known-issues.md:14`, `:27` |
| A13 | Tests: 12 headless tests in `tests/screens.rs` and 6 gated pixel tests, with `ui-visual.png` and `ui-labels.png` captured at 1280×720, frame 5 of 8. The `menus` and `debug` smoke rows run the lab, and the smoke-script test expects those two rows | `examples/05_ui_lab/tests/screens.rs`; `tests/ui_regression.rs:31-68`, `:74-186`; `scripts/smoke-examples.sh:229-250`; `scripts/test-smoke-examples.sh:109-110` |
| A14 | `UiHarness` lays the tree out at `WindowSize / ScaleFactor::get()` with a fixed-advance text double (half the size per glyph). So headless checks measure boxes, not glyphs | `tungsten/testing/ui.rs:1-7`, `:101-119` |
| A15 | No call ends a run from game code: only the `engine_exit` action does. Smoke mode gives no user folder (`D-159`), so a smoke run cannot save settings | `tungsten/app.rs:590-621`; `DECISIONS.md` `D-159` (the folder order) |
| A16 | The LLM index holds 12,284 of its 12,288 bytes. The lab's row routes `examples/05_ui_lab/src/` as a directory, plus `menus.rs` by name | `docs/LLM_INDEX.md:85`; `scripts/check-agent-context.py:22-25` |
| A17 | Read clean at plan time, with no defect beyond K1: `core/ui/control.rs`, `anim.rs`, `scroll.rs`, `window.rs`, `prompt.rs`, `layout.rs`, `hit.rs`, `focus.rs:200-404`, `route.rs:336-1000`, `render/ui.rs:200-310` and `tungsten/extract.rs:160-237`. Overshooting easings (`Back*`) on a scale track give a value `set_scale` skips for a frame (`anim.rs:284-286`, `tree.rs:802-806`); this is cosmetic and not a finding | as listed |
| A18 | No benchmark row builds a UI or opens the HUD, so example work moves no perf row. The UI systems run every frame on an empty tree | `examples/02_bench/src/` (no `UiTree` or `DebugHud`); `tungsten/debug_hud.rs:116` |
| A19 | No window reads a gamepad (no backend), but the harness injects pad input. So pad paths are checked headless only | `docs/known-issues.md:45`; `core/input.rs:837-875` |
| A20 | The machine has `xwininfo`, `xprop`, `qdbus6` and `import`, but no `xdotool` or `wmctrl`; the window manager is KWin on X11, on a 3840×2160 monitor. `x11rb` 0.13.2 (locked, cached) built offline in a scratch crate outside the workspace with `RUSTUP_TOOLCHAIN=1.98.1` and connected to `:0`; that build was deleted. `TUNGSTEN_DISPLAY_FRAME_RATE_CAP` paces a run's frames | this session; `Cargo.lock` (`x11rb` 0.13.2); `core/config.rs:9-13` |

## 2. Design

### 2.1 The QA pass (step 1)

The surface is the UI work of M54–M58, read as `rtk proxy git --no-pager diff --unified=3 83057a6 -- <path>` (`83057a6` is 0.73, the tree before M54). `rtk proxy` matters: the hook rewrites a plain or piped diff. The read goes in this order, highest risk first, skipping what A17 read clean:

1. `core/ui/tree.rs`, then `core/ui/paint.rs`: state roots, freeze and fade, raise, the debug and text scales, opacity, translation and scale, shadows and meshes.
2. `core/ui/route.rs` outside lines 336–1000, then `node.rs`, `event.rs`, `measure.rs`, `theme.rs`, `theme_file.rs`, `geometry.rs`, `mod.rs` and `layout/taffy.rs`.
3. `render/ui.rs` outside lines 200–310, `render/ui.wgsl`, and the world-label paths of `render/text.rs`, `render/text/engine.rs`, `render/text/gpu.rs` and `render/text/nodes.rs`.
4. The umbrella's UI paths: `tungsten/app.rs` (`stage_ui_layout`, `paint_ui`, `paint_prewarm`, the label viewport in `stage_extract`), `tungsten/ui_debug.rs`, and `tungsten/state.rs` (`own_root`, `RootCover`, fade and freeze).
5. The debug views: `tungsten/debug_views.rs`, `debug_hud.rs`, `inspector.rs`, `systems_overlay.rs`, `engine_font.rs`, and the console's store in `logging.rs`.
6. Example 05's changes since M54 (`main.rs`, `menus.rs`, `screens.rs`). A functional defect there is confirmed by a reproduction (a harness run or a smoke log, with no committed test, by the owner's example-test rule) and fixed in the step that rewrites its code (4–7); it counts toward the cap. A2–A5's look and layout gaps are parts 2–4's work, not findings.

**Candidates.** K1 and K2 (A12) start the list; the read adds the rest. A candidate is confirmed when a test written for it fails before the fix. A library candidate's test goes in that crate's `src/tests/` file for the module. The evidence quotes the failing test's name and its failure, and then the pass after the fix. A candidate whose test passes on the unfixed code is recorded as not reproduced and dropped.

**The cap.** At most 15 confirmed findings are fixed. Once 15 are, reading stops, and each later confirmed finding becomes a known-issues row (`P2`/`P3 — <location>`, "M59 QA, past the cap"). A confirmed finding goes there unfixed in four cases: its fix needs shared ECS or frame-loop code (§7); it needs a perf capture; it changes the per-frame work of a UI system on an empty tree; or it needs a file outside the surface above. In those cases the row says why.

**Contract changes.** A fix that changes a public item's behaviour gets a break-ledger row. A new public item gets rustdoc and a ledger row as an addition. A fix that reverses a recorded rule gets its decision first, through tungsten-decision, at the next free ID.

### 2.2 Windowed mode (steps 2 and 3)

**The repro fixture.** `TUNGSTEN_UI_LAB_FIXTURE=display` is an example fixture, documented with the others in `main.rs` and not added to smoke, since it flips the owner's screen. It runs in phases, under smoke frames, with `TUNGSTEN_DISPLAY_FRAME_RATE_CAP=60` so frames keep wall-clock pace:

- It pushes pause at frame 10 and settings at frame 30, then runs the phases below.
- **A**, a resize by hand: the fixture requests 1600×900 windowed through `request_display_settings`, the path a drag's end reaches through `Resized`, and waits up to 120 frames for `WindowSize` to read it.
- **B**, an unrelated apply: one volume step through the settings screen's own volume handler, which ends in `apply_settings`. It then logs 60 frames.
- **C1, C2, C3**: the display button's handler three times, Windowed → Borderless → Exclusive → Windowed, with 90 frames after each. After C2, three volume steps 10 frames apart: unrelated applies while `Settings` says exclusive and `DisplayState` says borderless (A11), which today re-request exclusive and log the downgrade each time (`tungsten/app.rs:821-829`).
- **D**: Back.
- **M**, optional with `TUNGSTEN_UI_LAB_MAXIMIZE_WAIT=1`, before B. The fixture prints `ui_lab display: phase M waiting` and waits up to 600 frames for `WindowSize` to change on its own, then runs B.

The step-2 helper makes the settings screen's mode cycle and volume step callable as `pub fn`s in `menus.rs`, so the button handlers and the script run the same code. Each phase ends with a line `ui_lab display: phase <P> end` and then holds 30 frames before the next phase starts. Every change in between prints one line:

`ui_lab display: frame <F> phase <P> window <W>x<H> state <mode> <W>x<H> settings <mode or -> scale <S> root (<x>, <y>) <w>x<h> offset (<dx>, <dy>)`

Here `window` is `WindowSize`, `state` is `DisplayState`, `settings` is `Settings::display_mode()`, `root` is the settings screen's root rect in units, and `offset` is its centre minus the viewport's.

**The window as X11 sees it.** `window`, `state` and `offset` all derive from the engine's own resources, so they can agree with each other while all of them disagree with the real window. A scratch script therefore tails the run's log. At each `phase <P> end` line, inside that phase's hold, it appends to `target/m59/display/<run>-x11.log` one line per phase: the phase's name, the client width, height and position from `xwininfo -name '<title>'`, and `xprop -name '<title>' _NET_WM_STATE`. The app is still running when these samples are taken.

**The maximize client.** A scratch crate outside the tree, `zz-x11-probe` (A20), sends the lab's window an EWMH `_NET_WM_STATE` client message (add `_NET_WM_STATE_MAXIMIZED_VERT` and `_HORZ`), found by its `_NET_WM_NAME`. The script runs it on the `phase M waiting` line. The client builds into `target/zz-x11-probe` and is deleted when step 2 ends. If it fails to build offline, phase M is recorded as not run.

**Saved settings.** Smoke mode has no user folder (A15). The existing headless test `the_display_mode_button_cycles_the_three_modes_into_settings` therefore gains a temp user folder (`UserFolder::At`). After each of its three switches it presses Back, reloads `Settings` from that folder, asserts the saved mode, and reopens the screen.

**Candidates and their default fixes.** Each candidate is applied only when step 2 reproduces it.

| # | Candidate | Shows in the log as | Default fix |
| --- | --- | --- | --- |
| W1 | A9: an apply snaps a hand-sized or maximized window back to 1280×720, or un-maximizes it | phase B ends with `window 1280x720` after reading 1600x900, or, in the maximize run, after the maximized size, and the X11 sample agrees; or, in the maximize run, B's sample has lost the maximized atoms that M's sample had | Decision 169, amending `D-159`. `apply_settings` remembers the last windowed target: the resolution startup resolved, or the last set or shipped target a windowed request sent. In windowed mode it sends the target only when the target differs from that memory, and then updates the memory. Otherwise it leaves the request's resolution as it stands. `apply_settings` starts from a request queued earlier in the frame (`tungsten/settings.rs:44-47`), so that request keeps its size, and with none queued, the request keeps the window's current size. The memory stays, and a size kept this way never enters it. In fullscreen the request carries the target as it does today, which the fullscreen path ignores (`tungsten/app.rs:886-892`), and the memory stays, so a target changed while fullscreen is sent by the first apply back in windowed mode. So a hand size holds, and an unset resolution still returns to the shipped one. The existing settings tests keep passing, `unset_values_return_to_the_shipped_ones` and `apply_settings_requests_the_display_and_sets_scale_and_volume` among them. Leaving fullscreen with nothing outstanding keeps the current size in the request, so no size is requested and the platform restores the size the window had. The memory lives in the crate-private `ShippedConfig` resource or one beside it, first set where `App::with_plugins` resolves the startup display |
| W2 | A10: `DisplayState`, or `WindowSize`, holds a stale or pre-request size after a switch, the UI lays out at it, or the restored size races the window manager | the engine's `window` or `state` disagree with the X11 sample taken in a phase's hold, or C3's sample differs from the one before C1 | Decision 170, amending `D-043`. A request never reads `inner_size()` right after `set_fullscreen`, and never writes a size the window does not have yet. A size the platform applies at once (`request_inner_size` returning `Some`, which may bring no `Resized`, A10) is handled as `Resized` is: `Renderer::resize`, then `sync_window_resolution`. A `None` writes nothing and waits for the event. So `WindowSize` and the surface always change together. The choice of what to ask winit (which mode, whether to request a size, and what to do with each response) moves into a pure `display.rs` function whose headless test covers both responses. The per-frame path, `take_pending_display` returning `None`, stays as it is. This removes the cause of known issues' WindowSize P3 (A12), whose row then goes; the tile and light extracts stay as they are |
| W3 | A11: an exclusive request visibly re-enters fullscreen, or a warning floods | a `Resized` at C2, or more than one downgrade warning from C2 to C3 (`not supported at runtime yet` in the log) | `DisplayDelta::between` compares the requested mode's runtime form with the current mode, so a repeated exclusive request over borderless changes nothing. The downgrade warning is logged once per run, the first time exclusive is requested, behind a crate-private flag |
| P2 | Known issues `:27`: a minimised or occluded window spins | out of reach of this fixture; recorded only if the run sees it | None: `stage_pacing` is frame-loop code (§7), so the row stays with `qa6` |

Decision numbers are the next free ones; a fix that does not land writes no decision.

### 2.3 The module layout (step 4)

Every file opens with a `//!` doc that names the pattern it shows and the decisions it rests on, not the milestone that added it. `lib.rs`'s doc is the map: one line per module saying what to copy from it.

| Module | Holds | Pattern |
| --- | --- | --- |
| `lib.rs` | The module map | Where each pattern lives |
| `main.rs` | `App` setup: manifests, theme, states, systems, the fixture switch | Wiring a UI game |
| `fixtures.rs` | `TUNGSTEN_UI_LAB_FIXTURE`, `_POST` and `_BOUNDS` (step 7) parsing; the scripted fixtures (`menus`, `menu-*`, `debug`, `labels`, `display`) | Driving a UI from scripts for smoke and captures |
| `theme.rs` | `THEME_ID`; spacing, size and motion constants; rarity colours; the theme apply | One place for every look and timing value |
| `widgets.rs` | Builders the screens share: screen root, window, button, caption row, slider row, toggle row, heading, prompt row | Small builder functions over `UiTree`, not new widget kinds |
| `hud.rs` | The lab HUD (icon, counter) and the play HUD (timer, prompt) | Pointer-transparent HUD rows; allocation-free text updates |
| `labels.rs` | World labels: the `labels` fixture and the damage numbers | World-anchored text (`D-161`) |
| `controls.rs` | M55's controls window and its system | Toggles and sliders read through `UiEvent::ValueChanged` |
| `menus.rs` | The play, pause and settings states | State-owned screens, fades, prompts, rebinding, `apply_settings` |
| `budget.rs` | The 100-widget menu the `menu`, `menu-prewarm` and `menu-cold` fixtures show | The prewarm budget subject, unchanged |
| `screens.rs` | The lab's paint-path screen: the HUD, the window and the confirm popup over it, which the `default`, `labels`, `hidden` and `debug` fixtures build | The paint list's batch partition (`D-147`) |
| `inventory/` | Step 5 | §2.4 |

`menus.rs` keeps its name so the LLM index row stays valid (A16). The budget menu's 100 nodes and its fixture timings do not change.

### 2.4 The inventory screen (step 5)

This plan confirms the request's four default features and cuts the rest.

1. **Item data.** `inventory/items.rs` holds `ITEMS: [ItemDef; 10]`, example-local Rust data. Each entry has a name, a kind (consumable, weapon, armour, trinket or material), a rarity (common, uncommon, rare, epic or legendary), a stack limit, a one-sentence description and an icon sprite ID. The starting contents fill 12 of 30 slots. The data is code, not a file the example loads by path (`D-046`). `generate.py` draws ten 32×32 icons as simple shapes, standard library only, into `assets/sprites/items/`, listed in the manifest as `ex05_item_<name>` with the `nearest` filter. The README documents them, and `--check` covers them.
2. **The slot grid.** A scroll panel holds six rows of five slots, three rows visible at the lab's sizes. Each slot is a 44-unit, focusable, interactive panel. It holds its 32-unit icon, a count in the mono family at the bottom end corner for a stack above 1, and a 3-unit stripe along its bottom edge in the rarity colour. A node's overrides apply after its kind's state variants (`core/ui/theme.rs:627-631`), so a slot overrides only its background, the raised surface. Its border, including the focused, hovered and pressed states, comes from the theme's `panel` kind, whose variants touch only the border and reach only interactive panels (§2.6). Empty slots are focusable, with every action disabled, so the grid navigates as a regular grid.
3. **The detail pane.** The pane shows the focused slot's item: its name in the rarity colour, its kind and rarity, the description, the count, an "Equipped" badge, and Use, Equip (Unequip when equipped) and Drop buttons. Use is enabled for consumables, Equip for gear, and Drop for any item. The pane follows focus; a click focuses its slot. The screen changes its widgets in place, never rebuilding the tree.
4. **Actions by every device.** The mouse clicks the buttons. Keys and the pad move focus between slots and onto the buttons (arrows, the D-pad or the stick), activate with `ui_accept` and close with `ui_cancel`. Three example actions, added in code as `add_pause_pad_binding` does, act on the focused slot: `inventory_use` (U, pad West), `inventory_equip` (E, pad North) and `inventory_drop` (Delete, pad Select). The inventory's root is modal, as the pause and settings roots are, so Escape and East always reach `ui_cancel`, never `engine_exit`, and the first key after a click acts (`core/ui/route.rs:1052-1066`, `:1105-1113`). A shown modal takes every key and pad press from the routed view, so these three actions read the raw `InputState` through `ActionMap::just_pressed`. They act only while the inventory is the top state, its confirm popup is hidden, no transition runs and no capture is pending; the module doc gives this as the pattern for a modal screen's hotkeys. A prompt row shows them through `set_prompt` templates, for the last device. The shared `input.json` also binds U and I to example 04's bloom actions, which the lab never reads; the `ui_*` defaults use South, East, the shoulders, the d-pad and the left stick, so West, North and Select are free (`core/input/action_map.rs:129-133`). Drop opens the confirm popup, a modal on the popup layer, with Cancel focused. Drop there removes the whole stack and keeps focus on the slot. Use takes one from a consumable's stack, and Equip keeps one item equipped per kind. The pad is checked headless only (A19).

**Where it lives.** The inventory is a state, `InventoryState`. Play pushes it on `inventory_open` (I, pad North), and the pause menu gains an "Inventory" button that pushes it; it owns its root with `RootCover::Hide` and is popped by its Close button or `ui_cancel`, behind the same fade as the other screens. The `default`, `labels`, `hidden` and `debug` fixtures build the same inventory window directly (`inventory::build`), with the confirm popup open, in place of the M46 placeholder. The paint-path screen therefore keeps three roots (HUD, window, popup) and its second text batch, and the M46 tests follow the new widgets. M55's controls window stays on the play screen, but play owns it with `RootCover::Hide`, so no pushed screen shares the screen with it.

**Cut:** drag and drop between slots (it needs a drag payload core lacks; a backlog row), sorting and filtering, tooltips (backlog-1.x's broader-controls row), and saving the inventory.

### 2.5 Motion (step 6)

`theme.rs` holds the motion constants, and every screen reads them:

- `FADE_SECS` 0.08 s each way (from 0.15).
- `SLIDE_SECS` 0.12 s over `SLIDE_UNITS` 12 with `CubicOut` (from 0.2 s, 24 units, `QuadOut`).
- `POP_SECS` 0.08 s, the confirm popup's opacity from 0 to 1 when it shows.

No core default changes (A5), so no decision is expected. The damage numbers' life and pop are game feel, not transitions, and stay. The `menus` script's frames (10, 40, 60, 70, 80, 110) keep working, since a shorter fade settles earlier.

### 2.6 The theme (step 7)

The run tunes the values, the rules are fixed:

- **Colours.** One surface for windows. The raised surface for slots, the pane and popups is a background override from `theme.rs`, and no node overrides a border. One border colour, text and muted text, an accent and its text, focus and disabled text.
- **Radii.** `sm` 4 (toggles, slots), `md` 6 (buttons), `lg` 10 (windows and popups).
- **Text.** 16 on 22 for the body, and 22 on 28 for headings through `widgets::heading`.
- **Kinds.** Buttons centre their text (`kinds.button.normal.text.align: "center"`), with a 1-unit border, weight 600, hovered lighter and pressed darker. Every focusable kind takes the same 2-unit focus border: buttons, toggles and sliders, and slots through `kinds.panel`'s `focused`, `hovered` and `pressed` variants. Those variants set only the border colour and width, so a slot's background override leaves them visible, and only interactive panels reach them, since nothing else is focused, hovered or pressed.
- **Contrast,** computed in linear light as the overlay blends: text against its background at least 4.5:1; focus borders, slot rarity stripes and control indicators against their background at least 3:1 (WCAG 1.4.11). A scratch script prints each pair. Panels stay opaque, so no translucent pair needs compositing.
- **Spacing.** Spacing is code, since layout is not themable (A7): `SPACE` 4/8/12/16/24, window padding 16, row gap 8, section gap 12, screen margin 16, button padding 6 by 14 with a minimum width of 96, caption column 120, control column 140, slot 44, icon 32.

### 2.7 Alignment (step 7)

These rules hold at 1280×720, 1920×1080 and 2560×1440, at an OS scale of 1.75 (731×411, 1097×617 and 1463×823 units). Tolerances are 0.5 units.

- **R1.** Every pushed screen's root (pause, settings, inventory) and the confirm popup is centred in the viewport.
- **R2.** A menu column's buttons share one width and one centre x, the column's.
- **R3.** Setting rows share one caption column (the same left edge and width) and one control column (the same left edge). Sliders share one width. Each row's caption and control share a centre y.
- **R4.** A row of action buttons (Use, Equip, Drop; Cancel, Drop) has equal widths and gaps and is centred under its pane or popup.
- **R5.** Slots sit on one pitch in x and y, and the grid is centred in its panel.
- **R6.** HUD rows sit the screen margin (16 units) from their corners.
- **R7.** On the play screen, the lab HUD, the controls window and the play HUD are pairwise disjoint.

One headless test in `tests/screens.rs`, `every_screen_is_centred_and_aligned_at_three_sizes`, sets `WindowSize` and `ScaleFactor::set_os(1.75)`, then builds or pushes each screen and asserts R1–R7 on `UiHarness::rect`. A fixed-advance double sizes text (A14), so the rules measure boxes; text centring inside a button is the theme's `align`, which the captures show.

`TUNGSTEN_UI_LAB_BOUNDS=1` sets `UiDebug::show_bounds` at startup, for the bounds captures.

### 2.8 Captures and fixtures (steps 7 and 8)

**Captures.** Each runs the lab with `TUNGSTEN_SMOKE_FRAMES`, `TUNGSTEN_CAPTURE_FRAME` and `TUNGSTEN_CAPTURE_RESOLUTION` into `target/m59/captures/`:

- the `default` fixture at each size, with and without bounds;
- the `menus` fixture at the frame that shows settings, at each size, with and without bounds;
- the pause menu and the inventory state at 1280×720.

That is 14 PNGs, each listed in the evidence with "OS scale 1.75 (Xft.dpi 168)" and its unit size.

**Fixtures.** `ui-visual.png` and `ui-labels.png` regenerate from the tree after steps 4–7, with `lab_command`'s own environment (`tests/ui_regression.rs:34-50`):

```bash
cargo build -p example-05-ui-lab --locked
env -u TUNGSTEN_CAPTURE_DIRECT TUNGSTEN_UI_LAB_FIXTURE=default TUNGSTEN_UI_LAB_POST=off TUNGSTEN_RENDER_POST_AA=off TUNGSTEN_SMOKE_FRAMES=8 TUNGSTEN_CAPTURE_FRAME=5 TUNGSTEN_CAPTURE_RESOLUTION=1280x720 TUNGSTEN_CAPTURE_PATH=examples/05_ui_lab/tests/fixtures/ui-visual.png target/debug/example-05-ui-lab
env -u TUNGSTEN_CAPTURE_DIRECT TUNGSTEN_UI_LAB_FIXTURE=labels TUNGSTEN_UI_LAB_POST=off TUNGSTEN_RENDER_POST_AA=off TUNGSTEN_SMOKE_FRAMES=8 TUNGSTEN_CAPTURE_FRAME=5 TUNGSTEN_CAPTURE_RESOLUTION=1280x720 TUNGSTEN_CAPTURE_PATH=examples/05_ui_lab/tests/fixtures/ui-labels.png target/debug/example-05-ui-lab
```

Both run from the repository root, on the reference machine at its OS scale of 1.75 as the old fixtures were, and the tools README gains these two commands. `just visual` then compares at tolerance 0. A comparison skipped without `TUNGSTEN_VISUAL_REGRESSION=1` is reported as not run.

### 2.9 Seam and placement

Steps 4–8 stay in `examples/05_ui_lab/` (`D-155`): no kit or library item, and no new dependency. Steps 1 and 3 change library crates. Core stays free of winit (`D-007`): the display policy of W2 lives in the umbrella's `display.rs`. Extract keeps its `&World` seam (`D-018`).

## 3. Steps

Before each step, copy every file it names to the scratchpad; on a failed check or a stop condition, restore with plain `cp`, confirm with `cmp`, and mark the step skipped (skill). Each code step ends with `cargo fmt --all` before its evidence row.

### Step 1: UI QA pass

- **Files:** the surface of §2.1 and the matching test files: `crates/tungsten-core/src/tests/ui/`, `crates/tungsten-render/src/tests/{ui,text,text_batches,text_nodes}.rs`, `crates/tungsten/src/tests/{app,extract,ui_debug,state,debug_views,debug_hud,inspector,systems_overlay,logging,engine_font,ui_route}.rs`; `docs/known-issues.md`, `docs/plans/1.0/w04-api-freeze.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`
- **Change:** §2.1. Each confirmed finding: failing test, fix, passing test, in that order, one at a time. K1's fix skips a neighbour whose root is frozen in `focus_direction`. K2's fix refreshes the UI inspector when its pick is first laid out. Both rows leave known issues when fixed.
- **Done-when:**
  - per finding, `cargo test -p <crate> --locked <test>` → fails before the fix (quoted), then `1 passed`;
  - `cargo test -p tungsten-core --locked && cargo test -p tungsten-render --locked && cargo test -p tungsten --locked` → exit 0;
  - `just lint` → exit 0, and `cargo fmt --all -- --check` → exit 0;
  - the evidence row lists the confirmed findings (at most 15) with their tests, the candidates not reproduced, and the known-issues rows added;
  - `sed -n '/^## Open findings/,/^## Recorded/p' docs/known-issues.md | rg -c 'M59 QA' || echo 0` → the number of rows added (baseline 0);
  - K1 fixed → `rg -c 'focus_direction' docs/known-issues.md || echo 0` → 0 (baseline 1); K2 fixed → `rg -c 'UiInspector' docs/known-issues.md || echo 0` → 0 (baseline 1);
  - each ledger row the fixes need is present, and `git status --short -- crates/tungsten-core/src/ecs crates/tungsten-core/src/schedule.rs crates/tungsten-core/src/physics` → empty.
- **Moves:** none. A paint fix may change the lab's pixels, which step 8 regenerates.

### Step 2: Windowed mode: reproduce

- **Files:** `examples/05_ui_lab/src/main.rs`, `examples/05_ui_lab/src/menus.rs`; scratch: the `zz-x11-probe` crate and the repro script in the scratchpad, `target/zz-x11-probe/` (deleted at the end), logs in `target/m59/display/`
- **Change:** the `display` fixture, phase M and the pub mode-cycle and volume-step functions (§2.2); build the client; run the fixture without and with phase M; classify W1–W3 and P2 from the logs. No library file changes.
- **Done-when:**
  - `cargo build -p example-05-ui-lab --locked` → exit 0;
  - the scratch script runs `TUNGSTEN_UI_LAB_FIXTURE=display TUNGSTEN_SMOKE_FRAMES=1200 TUNGSTEN_DISPLAY_FRAME_RATE_CAP=60 target/debug/example-05-ui-lab > target/m59/display/before.log 2>&1` → exit 0. Then `rg -c '^ui_lab display: phase (A|B|C1|C2|C3|D) end' target/m59/display/before.log` → 6, and `rg -c '^(A|B|C1|C2|C3|D) ' target/m59/display/before-x11.log` → 6;
  - the phase M run (the same environment plus `TUNGSTEN_UI_LAB_MAXIMIZE_WAIT=1`, into `before-maximize.log` and `before-maximize-x11.log`, with the client run on the waiting line) → its `phase M end` line and an `M` sample, or "not run" with the client's build error;
  - `rg -c 'not supported at runtime yet' target/m59/display/before.log || echo 0` → recorded as W3's measure (C2's request and the three volume steps after it);
  - the evidence row marks each of W1, W2, W3 and P2 reproduced (quoting its log lines) or not reproduced;
  - `cargo clippy -p example-05-ui-lab --all-targets --locked -- -D warnings` → exit 0, and `cargo test -p example-05-ui-lab --locked --test screens` → 12 passed;
  - `python3 -c "import pathlib; print(pathlib.Path('target/zz-x11-probe').exists())"` → `False`.
- **Moves:** none

### Step 3: Windowed mode: fix

- **Files:** as the header lists for step 3
- **Change:** for each candidate step 2 reproduced, its default fix in §2.2. Decision first, through tungsten-decision with its index row and `D-159`'s or `D-043`'s marker line, then the failing test, then the code. The IDs are 169 and 170 unless step 1 wrote a decision first; the checks below then use the IDs written. W1 has two tests in `tungsten/tests/settings.rs`:
  - `a_window_resized_by_hand_keeps_its_size_at_the_next_apply`: `sync_window_resolution` to 1600×900 as `Resized` does, a master-volume change, then `apply_settings`; the pending request must not resize;
  - `a_resolution_set_while_fullscreen_applies_when_windowed`: borderless, a set resolution and an apply (the memory stays), then windowed and an apply; the windowed request carries the set resolution. Today's code sends the target on every apply, so this test passes before the change too: it guards the new rule's fullscreen case (round 1, finding 3) rather than failing first;
  - `two_applies_before_a_frame_keep_the_queued_resolution`: a set resolution and an apply, then a master-volume change and a second apply in the same frame; the pending request still carries the set resolution. A guard as well, for round 2's finding 1.

  W3 has `an_exclusive_request_over_borderless_changes_no_mode` in `tungsten/tests/display.rs`: `DisplayDelta::between` with a borderless current state and an exclusive request reports no mode change. It fails today.

  W2's test is `a_display_request_leaves_window_size_to_the_resize`, in `tungsten/tests/display.rs`. It drives the extracted policy function with a `Some` response (the size is applied as `Resized` would apply it) and with a `None` response (nothing is written), and after leaving fullscreen (no `inner_size()` read). The saved-settings extension of §2.2 lands either way. A ledger row records each public behaviour change (`apply_settings`; `WindowSize` timing). Nothing reproduced → the step lands only the saved-settings extension and records the rest as not reproduced.
- **Done-when:**
  - each new test except the two guards fails before its fix (quoted), then `cargo test -p tungsten --locked <test>` → `1 passed`; the guards pass before and after;
  - `cargo test -p tungsten --locked settings` → every settings test passes, `unset_values_return_to_the_shipped_ones` and `a_runtime_apply_matches_a_restart` among them;
  - `cargo test -p example-05-ui-lab --locked --test screens` → 12 passed, the display test now checking the saved file after each switch;
  - the step-2 script into `target/m59/display/after.log` and `after-x11.log` (with phase M, `after-maximize*.log` too) → exit 0. For a fixed W1, phase B's end line and its X11 sample both read 1600×900, and in the maximize run, B's sample keeps the maximized atoms that M's sample had. For a fixed W2, every phase's engine `window` equals its X11 sample, C3's sample equals the one before C1, and every end line's offset is within 0.5 units. For a fixed W3, `rg -c 'not supported at runtime yet' target/m59/display/after.log` → 1;
  - with 169 written: `rg -c '^## D-16[9] — ' DECISIONS.md` → 1, `rg -c '^\| `D-16[9]` \|' docs/DECISION_INDEX.md` → 1, and `rg -c '^\*\*Amended by D-16[9]:\*\*' DECISIONS.md` → 1. The same holds for 170 with `D-17[0]`;
  - with W2 fixed, `rg -c 'WindowSize' docs/known-issues.md || echo 0` → 0 (baseline 1);
  - `just lint` → exit 0, and `cargo fmt --all -- --check` → exit 0.
- **Moves:** none

### Step 4: UI lab module layout and pattern docs

- **Files:** as the header lists for step 4
- **Change:** move the lab's code into §2.3's modules with no behaviour change; rewrite each module's `//!` doc as the pattern it shows; `lib.rs` becomes the map; the tests' imports follow. No new test.
- **Done-when:**
  - `rtk proxy cargo test -p example-05-ui-lab --locked -- --list 2>/dev/null | rg ': test$' | sort`, written to the scratchpad before the step and again after it → `cmp` exit 0 (the 18 names of A13);
  - `cargo test -p example-05-ui-lab --locked --test screens` → 12 passed;
  - `rg --files-without-match '^//!' examples/05_ui_lab/src` → no output;
  - `rg -c '^pub mod ' examples/05_ui_lab/src/lib.rs` → 9 (§2.3's library modules before step 5; baseline 2);
  - for `menus` (150 frames), `debug` (120), `labels`, `default` and `menu-prewarm` (400), each `TUNGSTEN_SMOKE_FRAMES=<n> TUNGSTEN_UI_LAB_FIXTURE=<f> target/debug/example-05-ui-lab` → exit 0, with `menus` printing its six `ui_lab menus: frame` lines and `menu-prewarm` its three `ui_lab menu:` lines;
  - `cargo clippy -p example-05-ui-lab --all-targets --locked -- -D warnings` → exit 0, and `cargo fmt --all -- --check` → exit 0.
- **Moves:** none

### Step 5: Inventory screen

- **Files:** as the header lists for step 5
- **Change:** §2.4. The theme's `panel` kind gains focused, hovered and pressed variants, so slots show focus before step 7 restyles them. The pause menu gains "Inventory"; the paint-path screen's window becomes the inventory window; the `labels` fixture's straddling label moves to the new window's left edge; the `menus` script opens the inventory and closes it within its 150 frames, each at the first frame with no transition running. `request_push_transition` refuses a request during a transition (`tungsten/state.rs:186-189`), and the pop at frame 110 fades until about frame 128 at 0.15 s each way. So the open comes from frame 120 on, once that pop ends, and the close once the inventory is on top with no transition running. Each action prints the top state as the others do, so the close prints `top 'inventory'`, at about frame 146 with the current fades and about 130 with step 6's. A pop takes effect when its fade-out ends (`tungsten/state.rs:304-306`). Once play is back on top with no transition running, the script prints `ui_lab menus: frame <F>, top 'play', settled` once. At the current fades that comes after the smoke row's 150 frames, so this step checks over 200; step 6's fades bring it inside 150. Two tests in `tests/screens.rs`:
  - `the_grid_scrolls_to_the_focused_slot_and_the_pane_follows`: arrows past the third row scroll the panel, and the pane's name is the focused item's;
  - `use_equip_and_drop_by_pointer_keys_and_pad`:
    - a click on Use takes one, and E equips and shows the badge;
    - U changes nothing while the confirm shows or during the fade-in;
    - the pad's Select opens the confirm with Cancel focused, and South there cancels;
    - Select, then Right, then South drops the stack and empties the slot;
    - Escape cancels a third drop and leaves the inventory open.
- **Done-when:**
  - `python3 -B examples/05_ui_lab/tools/generate.py --check` → `up to date`, exit 0;
  - `just repo-check` → exit 0;
  - `cargo test -p tungsten-core --test manifests --locked` → exit 0;
  - `cargo test -p example-05-ui-lab --locked --test screens` → 14 passed;
  - `TUNGSTEN_SMOKE_FRAMES=200 TUNGSTEN_UI_LAB_FIXTURE=menus target/debug/example-05-ui-lab > target/m59/menus.log 2>&1` → exit 0, with `rg -c "top 'inventory'" target/m59/menus.log` → 1 (the close's line) and `rg -c "top 'play', settled" target/m59/menus.log` → 1;
  - `rg -c '^pub mod ' examples/05_ui_lab/src/lib.rs` → 10;
  - clippy and fmt as in step 4.
- **Moves:** the lab's pixels, regenerated in step 8

### Step 6: Shorter transitions

- **Files:** as the header lists for step 6
- **Change:** §2.5.
- **Done-when:**
  - `rg -n 'const (FADE_SECS|SLIDE_SECS|POP_SECS): f32 = 0\.(08|12);' examples/05_ui_lab/src/theme.rs` → 3 lines, and `rg -q 'FADE_SECS: f32 = 0\.15' examples/05_ui_lab/src` → exit 1;
  - `cargo test -p example-05-ui-lab --locked --test screens` → every test passes, `state_pause_pushes_the_pause_menu_behind_a_fade_frozen_until_it_ends` among them;
  - the step-5 `menus` command at the smoke row's 150 frames (`TUNGSTEN_SMOKE_FRAMES=150`) → exit 0, with `top 'inventory'` once and `top 'play', settled` once: the shorter fades finish the close inside the smoke row;
  - clippy and fmt as in step 4.
- **Moves:** none

### Step 7: Theme and alignment

- **Files:** as the header lists for step 7
- **Change:** §2.6 and §2.7; build every screen through `widgets.rs` so R1–R7 hold; `TUNGSTEN_UI_LAB_BOUNDS`; the captures of §2.8 into `target/m59/captures/`.
- **Done-when:**
  - `cargo test -p example-05-ui-lab --locked --test screens every_screen_is_centred_and_aligned_at_three_sizes` → `1 passed`; with step 5 skipped, the test leaves out the inventory's screens and R4's Use row;
  - the contrast script's output, quoted: every text pair ≥ 4.5:1 and every non-text pair ≥ 3:1, in linear light;
  - 14 capture commands → exit 0 each, with every PNG present and listed with its size and scale;
  - `cargo test -p example-05-ui-lab --locked --test screens` → 15 passed (or 13 without step 5);
  - clippy and fmt as in step 4.
- **Moves:** the lab's pixels, regenerated in step 8

### Step 8: Pixel fixtures and `just visual`

- **Files:** as the header lists for step 8
- **Change:** §2.8's two commands on the tree that steps 4–7 left, whichever landed; then the README lines. Record each fixture's `sha256sum` before and after.
- **Done-when:**
  - both commands → exit 0;
  - `just visual` → exit 0 with four `test result: ok` lines (`visual_regression`, `post_regression`, `transition_regression`, `ui_regression`); its expected `fade` ERROR line is the only one;
  - `git status --short -- examples/02_bench/tests examples/03_scene_state/tests examples/04_shader_playground/tests` → empty;
  - `rg -c 'TUNGSTEN_CAPTURE_PATH=examples/05_ui_lab/tests/fixtures' examples/05_ui_lab/tools/README.md` → 2.
- **Moves:** `ui-visual.png` and `ui-labels.png`, with the hashes in the evidence log and the `CHANGELOG.md` line

### Step 9: Records

- **Files:** as the header lists for step 9
- **Change:**
  - Known issues: the rows the fixes closed go, the rows found out of scope or past the cap are added, and a follow-up is added: a game's Quit button has no engine call (A15).
  - The backlog gains drag and drop between widgets, from M59's cut (§2.4).
  - w01 §11 gains one sentence under the ladder table on M59's QA and the lab as a reference.
  - The break ledger holds steps 1 and 3's rows.
  - With 169 landed: `DESIGN.md`'s Settings and Save Slots paragraph and `docs/getting-started.md:81` state that a hand size holds. With 170: `DESIGN.md`'s scale-factor and display lines state that `WindowSize` changes only with the surface, on `Resized` or on a size applied at once.
  - `just api`, then review the `api/` diff; one `CHANGELOG.md` line under Unreleased.
  - The LLM index stays as it is: `examples/05_ui_lab/src/` routes the new files (A16).
- **Done-when:**
  - `just api` → exit 0, and every `api/` change has its ledger row (none expected unless a fix added public surface);
  - `just ctx` → exit 0, and `just repo-check` → exit 0;
  - `sed -n '/^## \[Unreleased\]/,/^## \[0/p' CHANGELOG.md | rg -c 'M59'` → 1 (baseline: no match);
  - `rg -c 'M59' docs/plans/1.0/backlog-1.x.md` → 1 (baseline: no match);
  - `wc -c < docs/LLM_INDEX.md` → at most 12288;
  - the known-issues and DESIGN greps follow what landed: each fixed row gone; each added `M59` row present; a skipped step's part recorded as a known issue named "M59 step N".
- **Moves:** none

### Step 10: Release 0.79

- **Files:** `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row, the README's four owned Now fields (`roadmap.py summary`), this plan (moved to `docs/plans/archive/`)
- **Change:** close workflow §5 and this step's evidence row with `status: done`; then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.79.0` once, every release check once after it, then the preflight with `--message`. No gate by change is left over: step 8 ran `just visual`, the chain runs smoke, and no physics, dependency or perf path changed.
- **Done-when:** [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut); `just release-preflight 0.79.0 --repo JosephGibson/Tungsten --message 'Update 0.79: M59 UI lab reference and QA'` → passes and prints the command block and the post-merge block
- **Ends with:** the changed-file list, the command block, the post-merge block, then the next prompt, the `plan` row for W6b and W6c: `Use the tungsten-milestone skill. Write the milestone plan for W6b and W6c: read docs/plans/1.0/workflow.md, its card in docs/plans/1.0/implementation-plan.md and docs/plans/1.0/criteria.md §8.1 (W6); audit only the code it touches; name it docs/plans/1.0/phase6-milestone-60-layers-audio-buses.md; critique it last as the skill's step 7 says; end with the run prompt. Don't commit: leave the changes in the tree for the release commit.`

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Which UI work does the QA read, in what order, and when does it stop? | §2.1: the M54–M58 diffs of the named surface, highest risk first, skipping A17's files; stop at 15 confirmed findings |
| Q2 | What size does Windowed restore after fullscreen, and does a hand size survive an apply? | The size the window had before it left windowed mode, through the platform's restore, unless a resolution set while fullscreen is outstanding, which the return requests. A hand-sized or maximized window keeps its size until the set or shipped resolution changes (169), if step 2 reproduces W1 |
| Q3 | Does a display request still write `WindowSize`? | Only a size the platform applied at once (`Some`), handled as `Resized` is; otherwise the `Resized` event writes it (170), if step 2 implicates W2 |
| Q4 | Where does the inventory live? | A state pushed from play (I, pad North) and from the pause menu; the paint-path fixtures build its window directly with the confirm open (§2.4) |
| Q5 | What happens to M55's controls window? | It stays on the play screen and is owned with `RootCover::Hide`, so no pushed screen overlaps it |
| Q6 | How many items and slots? | 10 definitions, 30 slots in rows of five, 12 filled, three rows visible |
| Q7 | Are empty slots focusable? | Yes, with every action disabled |
| Q8 | Which shortcuts? | Use U / pad West, Equip E / pad North, Drop Delete / pad Select, open I / pad North from play, shown in a prompt row |
| Q9 | Does Drop take one item or the stack, and which confirm button holds focus? | The stack; Cancel |
| Q10 | How short are the transitions? | §2.5: 0.08 s fades, a 0.12 s slide over 12 units, a 0.08 s popup fade; no core default changes |
| Q11 | Is spacing a theme token? | No: code constants in `theme.rs`, since core applies no spacing token |
| Q12 | Does the `display` fixture stay, and does smoke run it? | It stays, documented in `main.rs`; smoke does not run it, since it flips the screen |
| Q13 | Where do the alignment checks live? | One test in `tests/screens.rs`, not a new target |
| Q14 | Does the pause menu get Quit? | No: no engine call ends a run from game code; a known-issues follow-up |
| Q15 | What if step 2 implicates the minimised-window P2? | The row stays with `qa6`: `stage_pacing` is frame-loop code (§7) |
| Q16 | How do the inventory's hotkeys get input while its screen is modal? | From the raw `InputState`, guarded by the screen's own state: top of the stack, confirm hidden, no transition, no capture (§2.4). A non-modal root would let Escape after a click reach `engine_exit` |
| Q17 | How does a slot show its rarity without hiding focus? | A 3-unit rarity stripe inside the slot. The slot overrides only its background, and the `panel` kind's state variants set only the border, so no override hides them (§2.4, §2.6) |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`).

## 5. Decisions expected

- 169: a window resized by hand keeps its size at `apply_settings` until the set or shipped resolution changes, and a resolution set while fullscreen applies on the return to windowed. Amends `D-159`. Only if step 2 reproduces W1.
- 170: a display request writes the window size only when the platform applied it at once, together with the surface, as a `Resized` event does; otherwise the event writes it. Amends `D-043`. Only if step 2 implicates W2.
- A step-1 fix that reverses a recorded rule takes the next free ID at that time.

Next free IDs when written: 169 and 170. A decision step 1 writes takes the next free ID first, and step 3's shift up behind it. The run checks them again with `rg -n '^## D-1' DECISIONS.md | tail -3` and `git status --short` before step 1, before step 3 and before the release.

## 6. Invariants

- **May move:** `examples/05_ui_lab/tests/fixtures/ui-visual.png` and `ui-labels.png`, in step 8. Their `sha256sum` before and after goes in the evidence log and the `CHANGELOG.md` line.
- **Must not move:**
  - the determinism hash, the pinned containment hash, the row digests, `gpu-visual.png`, and the post and transition regressions (no physics, benchmark or render-pass path changes; step 8's `just visual` and its `git status` check);
  - the budget menu's 100 nodes (`the_menu_fixture_holds_exactly_100_nodes`);
  - the `menus` script's frames and its top states at 10, 40, 80 and 110;
  - the `debug` smoke row;
  - the `api/` snapshots, except changes with ledger rows.

## 7. Stop conditions

- A done-when check fails: restore the step's files, mark the step skipped, and continue with steps that do not depend on it. Step 3 depends on step 2; steps 5–7 tolerate a skipped step 4 by building on the old layout; step 7's checks drop the inventory's rules when step 5 is skipped.
- **Steps the release needs.** The release needs steps 8 and 9; each other step may be skipped with its reason, and its part then becomes a known-issues row named "M59 step N". If step 8 fails, the run ends before the release, and a release session takes it after the owner has seen why (workflow §3).
- **Frame loop and ECS.** A fix needs shared ECS or frame-loop code: `core/ecs/`, `core/schedule.rs`, `App::run_frame`'s stage order, `stage_pacing`, the clocks, or the per-frame work of a UI system on an empty tree. The finding becomes a known-issues row and no perf capture is taken. The same holds for anything that would need a perf capture.
- **QA cap and scope.** The 16th confirmed QA finding, and any finding whose fix needs a file outside its step's list, becomes a known-issues row.
- **Core defaults and decisions.** A core default would change: it needs its decision first; with no decision, it is not changed. A fix would reverse an owner-settled gate answer: it becomes a known-issues row.
- **Repro tooling.** The x11rb client fails to build offline, or KWin ignores the maximize: phase M is recorded as not run, and the rest of step 2 stands.
- **Index budget.** The LLM index would pass 12,288 bytes: keep the files routed through the directory row; record any row change not made.
- **Another session.** `git status --short` shows files this plan does not name: leave them, and name them in the release step's report.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1, 3, 5, 8, release | Crate tests and lint in 1 and 3; layer 1 and `just repo-check` in 5; `just visual` in 8; the release runs `just check`, `just smoke`, `just script-test`, `just template-check` and the rest once, after the cut. No perf row: no benchmark row runs a UI (A18), and frame-loop changes stop (§7) |
| 3 Invariants | 8 | §6; the lab's two fixtures move on purpose |
| 4 Decisions | 1, 3 | 169 and 170 when their candidates reproduce; index rows in the same step |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1, 3, 9 | Behaviour-change rows from the fixes; `just api` in 9 |
| 6 Template and guide | n/a | No change to how a game is written; with 169, the getting-started guide's line 81 changes in 9 |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 9 | The lab's directory row routes the new modules; `just ctx` |
| 8 Design | 9 | `DESIGN.md` only for 169 and 170 |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 1, 3, 9, release | Known issues per outcome; the backlog's drag-and-drop row; no gap log rows (the game has not started); the register row at the release; w01 §11's sentence; `python3 -B scripts/roadmap.py summary` supplies the owned fields |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

**Round 1.**

- **Reviewer:** the `critique` skill with `--repo`, `gpt-6-astra`.
- **When and what it read:** 2026-10-15 (02:09–02:15 UTC on the 16th). It read the tree at `1dea903` with this session's uncommitted plan, register row and catalog entry.
- **Footer:** effort xhigh, context 872,000, output cap 4,000 tokens, 354.5 s; tokens in 1,222,733 (1,129,216 cached), out 9,781 (5,225 reasoning); 26 commands, 273.4 KB of output (largest 31.4 KB); 5h window 41% → 67%, week 14% → 18%.
- **Command:** `critique --raw --repo --out <scratchpad>/critique-r1.md --from "sed '/^## 9\./,/^## Fol/{/^## Fol/!d}' docs/plans/1.0/phase6-milestone-59-ui-lab-reference.md"`.

Each finding was checked against the repository before it changed anything.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] Making `Resized` the only size writer breaks synchronous resize responses | winit's contract (`winit-0.30.13/src/window.rs:763-777`): a size applied at once may bring no `Resized`. Wayland always returns `Some` (`.../wayland/window/mod.rs:313-318`). The surface resyncs to the window on its own (`render/renderer.rs:1320-1328`) | accepted | §2.2 W2: a `Some` size is handled as `Resized` is (`Renderer::resize`, then `sync_window_resolution`), and a `None` waits for the event. The policy test covers both responses. A10, Q3 and §5's 170 follow |
| 2 | [HIGH] Step 5's inventory push occurs while the previous transition still blocks it | `tungsten/state.rs:186-189` refuses requests during a transition; `examples/05_ui_lab/src/menus.rs:27-44` fades 0.15 s each way; smoke steps 1/60 s (`tungsten/app.rs:64`) | accepted | Step 5: the script opens and closes the inventory each at the first frame with no transition running, so the close prints `top 'inventory'` by about frame 146 at the current fades |
| 3 | [MEDIUM] The resolution cache can forget a change made during fullscreen | `tungsten/app.rs:840-852` requests a size in windowed mode only, so a target cached while fullscreen is never applied | accepted | §2.2 W1 remembers the last windowed target, which only a windowed request's new target updates, so a target changed while fullscreen is sent on the return. Step 3 adds the guard test `a_resolution_set_while_fullscreen_applies_when_windowed`; Q2 and §5's 169 follow |
| 4 | [MEDIUM] The display fixture cannot independently detect the stale geometry it claims to measure | `tungsten/display.rs:103-129` and `tungsten/app.rs:1255-1266`: `window`, `state` and `offset` all come from the engine; a smoke run has exited before an "after the run" reading | accepted | §2.2: a 30-frame hold after each phase, and one `xwininfo` and `xprop` sample per phase while the app runs. Steps 2 and 3 compare the engine's values with the samples |
| 5 | [MEDIUM] The inventory shortcuts lack a route through the shared modal screen pattern | `core/ui/route.rs:1105-1113`: a shown modal takes every key and pad press. `examples/05_ui_lab/src/menus.rs:102-104` makes screen roots modal. `PlayState` reads `RoutedInput` (`menus.rs:228-231`) | accepted | §2.4: the root stays modal, so Escape never reaches `engine_exit`; the hotkeys read the raw `InputState` behind the screen's own guards. Step 5's test checks U during the confirm and the fade-in. Q16 |
| 6 | [MEDIUM] The theme's focused border cannot override a slot's rarity override | `core/ui/theme.rs:620-631` applies overrides after the state variants. `core/ui/theme_file.rs:283-297` accepts state variants for `panel` | accepted | §2.4 and §2.6: rarity is a stripe child; a slot overrides only its background, and the `panel` kind's state variants set only the border. Q17 |
| 7 | [MEDIUM] The drop test expects acceptance to activate the unfocused button | `core/input/action_map.rs:129-133` (South is `ui_accept`); `core/ui/route.rs:1131-1135` activates the focused widget | accepted | Step 5's test: South on the confirm cancels; Select, Right, South drops |

Round 1 changed done-when checks in steps 2, 3 and 5, so round 2 is required (skill step 7).

**Round 2.**

- **Reviewer:** the same, with `--focus-file`, a note naming round 1's changed sections and two refinements made while proofreading.
- **When:** 2026-10-15 (02:28–02:33 UTC on the 16th).
- **Footer:** effort xhigh, context 872,000, output cap 4,000 tokens, 320.5 s; tokens in 797,461 (722,944 cached), out 7,957 (4,646 reasoning); 18 commands, 210.5 KB of output (largest 22.4 KB); 5h window 67% → 86%, week 18% → 20%.
- **Run timing:** first queued behind the window's reset at 03:29 UTC, then started at 67% at the owner's request.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [HIGH] W1 remembers a resolution before its request reaches the window | `tungsten/settings.rs:44-47`: `apply_settings` starts from the request queued this frame. W1's "keeps the window's current size" would replace that request's size and, with the memory already advanced, never resend it | accepted | §2.2 W1: the non-sending branch leaves the request's resolution as it stands, so a queued size survives a second apply. Step 3 adds the guard `two_applies_before_a_frame_keep_the_queued_resolution` |
| 2.2 | [MEDIUM] The display fixture misses the repeated-warning trigger | `tungsten/app.rs:821-829`: with `Settings` exclusive and `DisplayState` borderless, every apply re-requests exclusive and logs the downgrade again | accepted | §2.2: three volume steps after C2, and the warning count is W3's measure. W3's fix compares the requested mode's runtime form and logs once per run, with a failing test, `an_exclusive_request_over_borderless_changes_no_mode`. Steps 2 and 3 count the warnings |
| 2.3 | [MEDIUM] Step 5 ends before the inventory close takes effect | `tungsten/state.rs:304-306`: a pop runs when its fade-out ends; at 0.15 s that is past frame 150 | accepted | The script prints a settled line once play is back. Step 5 checks over 200 frames, and step 6 checks it inside the smoke row's 150 |

Round 2 changed done-when checks in steps 2, 3, 5 and 6. The skill allows no third round, and no finding stays open, so no open question is added.

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 UI QA pass | 2026-10-16 | done: 2 confirmed and fixed, none past the cap | Read `83057a6..` in §2.1's order: core `tree.rs`, `paint.rs`, `route.rs` outside 336–1000, `node.rs`, `event.rs`, `measure.rs`, `geometry.rs`, `mod.rs`, `theme.rs`, `theme_file.rs`, `layout/taffy.rs`; render `ui.rs`, `ui.wgsl`, `text.rs`, `text/engine.rs`, `text/gpu.rs`, `text/nodes.rs`, `renderer.rs`; umbrella `app.rs`, `state.rs`, `plugins.rs`, `settings.rs`, `testing/ui.rs`, `logging.rs`, `engine_font.rs`, all of `debug_views.rs`, the HUD's window update, the systems overlay's rows and the inspector's pick (part 6 is read in steps 4–7). (1) K1: `an_explicit_neighbour_in_a_frozen_root_takes_no_focus` (core `tests/ui/focus.rs`) failed `left: Some(WidgetId { index: 4, generation: 0 }) right: None`, then `1 passed` with `focus_direction` skipping a neighbour whose root is frozen. (2) K2: `a_pick_made_before_its_first_layout_refreshes_once_it_is_laid_out` (umbrella `tests/debug_views.rs`) failed `left: Some("-") right: Some("(300.0, 100.0) 120.0×40.0")`, then `1 passed` with the inspector refreshing on its pick's first layout. Not reproduced or not findings: a span's own colour not fading (an open limit `D-163` records); the HUD graph keeping its samples while hidden (cosmetic). Known-issues rows added: 0 (`rg -c 'M59 QA'` → 0); `focus_direction` and `UiInspector` rows → 0 and 0 (baselines 1, 1). `cargo test` core 1,086 passed, render 188, umbrella 526, each exit 0; `just lint` exit 0; `cargo fmt --all -- --check` exit 0; the ECS, schedule and physics `git status` empty. Ledger: one row, "A frozen root's explicit focus neighbour; the UI inspector's first layout" | `target/m59/m59-step1-*.log`, `target/m59/step1-lint.log` |
| 2 Windowed mode: reproduce | 2026-10-16 | done: W1 and W3 reproduced; W2 and P2 not | Fixture `display` in `main.rs` and `menus.rs`, with `cycle_display_mode`, `set_volume` and `step_volume` as `pub fn`s the handlers call. `cargo build` exit 0. Run (OS scale 1.75, Xft.dpi 168): exit 0; `phase (A|B|C1|C2|C3|D) end` → 6; X11 samples → 6. W1 reproduced: `phase A end frame 50 window 1600x900 state windowed 1600x900`, then B's volume step `phase B end frame 140 window 1280x720 state windowed 1280x720`; X11 `A 1600x900 (1280, 710)`, `B 1280x720 (1280, 710)`. Maximize run (`zz-x11-probe` built offline in 2.95 s): `maximize sent to 0x4e00002`, `phase M end frame 94 window 3840x2040`, sample `M 3840x2040 (0, 53) state _NET_WM_STATE_MAXIMIZED_VERT, _NET_WM_STATE_MAXIMIZED_HORZ`; B kept `3840x2040` and both atoms: KWin keeps a maximized window's size against the request. W2 not reproduced: every hold's engine `window` equals its X11 sample, and C3's sample equals B's in both runs (`1280x720 (1280, 710)`; maximized `3840x2040 (0, 53)`); every end line's offset ≤ 0.05. Transient only: `frame 171 phase C1 ... state borderless_fullscreen 1280x720` for one frame, and frames 411–418 `window 3840x2160 state windowed` until the restore's `Resized` at 419, where `WindowSize` still matched the surface. W3 reproduced: `not supported at runtime yet` → 4 (C2's request and the three volume steps), 4 in the maximize run, with no `Resized` at C2. P2 not seen. Clippy exit 0; `--test screens` 12 passed; fmt exit 0; `target/zz-x11-probe` exists → `False` | `target/m59/display/before{,-x11,-maximize,-maximize-x11}.log`, `target/m59/step2-clippy.log` |
| 3 Windowed mode: fix | 2026-10-16 | done: W1 and W3 fixed; W2 not applied (step 2 did not reproduce it), so decision 170 is not written and the `WindowSize` row stays | `D-169` written first, amending `D-159` (marker and index rows). W1: `a_window_resized_by_hand_keeps_its_size_at_the_next_apply` failed `left: Some(Resolution { width: 1280, height: 720 }) right: None`, then `1 passed`; the guards `a_resolution_set_while_fullscreen_applies_when_windowed` and `two_applies_before_a_frame_keep_the_queued_resolution` passed before and after (`1 passed`). W3: `an_exclusive_request_over_borderless_changes_no_mode` failed (`exclusive runs as borderless, which the window already is`), then `1 passed`; the warning logs through a per-app `std::sync::Once` (a fourth `App` bool failed `clippy::struct_excessive_bools`). `cargo test -p tungsten --locked settings` → 11 passed (`unset_values_return_to_the_shipped_ones`, `a_runtime_apply_matches_a_restart` among them); `--test screens` 12 passed, the display test reloading `settings.json` after each switch. After runs exit 0, 6 end lines each: B `window 1600x900`, X11 `B 1600x900 (1280, 710)`; C3 `1600x900 (1280, 710)`; maximize run B and C3 `3840x2040 (0, 53)` with both maximized atoms; `not supported at runtime yet` → 1 in each. End-line offsets: A and M read their frame's pre-layout offset (`(-91.4, -51.4)`, `(-640.0, -325.7)`), as before the fix; the rest 0.0. `rg` checks for 169 → 1, 1, 1. `just lint` exit 0; fmt exit 0; `target/zz-x11-probe` deleted. Ledger: "A hand-sized window keeps its size at `apply_settings`; an exclusive request over borderless" | `target/m59/display/after{,-x11,-maximize,-maximize-x11}.log`, `target/m59/step3-lint.log` |
| 4 UI lab module layout and pattern docs | 2026-10-16 | done | Nine library modules (`budget`, `controls`, `fixtures`, `hud`, `labels`, `menus`, `screens`, `theme`, `widgets`) and `main.rs`, each opening with a `//!` naming its pattern and decisions; `lib.rs` is the map. The test list before and after (a Python `--list` filter, since a piped `rg` came back empty through the hook) → `cmp` exit 0, 18 names; `--test screens` 12 passed; `rg --files-without-match '^//!'` → no output; `rg -c '^pub mod '` → 9. Fixtures: `menus` 150 frames exit 0 with six `ui_lab menus: frame` lines (10 play, 40 pause, 60/70/80 settings, 110 pause); `debug` 120, `labels` 120, `default` 120 exit 0; `menu-prewarm` 400 exit 0 with three `ui_lab menu:` lines. Clippy exit 0; fmt exit 0. Part 6 of the QA read (the lab's M54–M58 code, read whole for the move): no functional defect | `target/m59/step4-*.log` |
| 5 Inventory screen | 2026-10-16 | done | `inventory/{mod,items,grid,detail}.rs`: ten `ItemDef`s, 12 of 30 slots filled, a scroll panel of six rows of five 44-unit slots (three visible), the pane following focus, the confirmation with Cancel focused, `inventory_use`/`equip`/`drop`/`open` added in code (U/West, E/North, Delete/Select, I/North), the hotkeys read raw behind the live guard. `InventoryState` from play and from the pause menu's "Inventory", roots owned `Hide`; the paint-path fixtures build it with the confirmation open (`screens::build` → lab HUD + inventory; three roots, two text batches); play owns the lab HUD `Show` and the controls window `Hide`; the `labels` fixture's straddling label reads the window's edge from the layout. The `panel` kind gains border-only focused, hovered and pressed variants. Ten 32×32 icons from `generate.py` in `sprites/items/`, listed as `ex05_item_<name>`. `generate.py --check` → `up to date`, exit 0; `just repo-check` exit 0 (after the step-3 evidence row lost a literal unwritten decision ID); `manifests` 4 passed; `--test screens` 14 passed (`the_grid_scrolls_to_the_focused_slot_and_the_pane_follows`, `use_equip_and_drop_by_pointer_keys_and_pad` new; the hotkeys first acted on the last frame's selection, fixed by following focus before the events); `menus` over 200 frames exit 0: `frame 128, top 'play'`, `frame 146, top 'inventory'`, `frame 164, top 'play', settled` (each count 1); `pub mod` → 10; clippy exit 0 (two `single_match_else` fixed); fmt exit 0 | `target/m59/menus.log`, `target/m59/step5-*.log` |
| 6 Shorter transitions | 2026-10-16 | done | `theme.rs`: `FADE_SECS` 0.08, `SLIDE_SECS` 0.12 over `SLIDE_UNITS` 12 with `CubicOut`, `POP_SECS` 0.08 through `theme::pop_in` on the confirmation when an action opens it (the fixtures' pre-opened one shows at once, so a capture never catches it mid-fade). The `rg -n` check → 3 lines (21, 23, 29); `rg -q 'FADE_SECS: f32 = 0\.15'` → exit 1. `--test screens` 14 passed, `state_pause_pushes_the_pause_menu_behind_a_fade_frozen_until_it_ends` among them. `menus` at 150 frames exit 0: the six fixed lines, then `frame 120, top 'play'`, `frame 130, top 'inventory'` (count 1), `frame 140, top 'play', settled` (count 1). Clippy exit 0; fmt exit 0 | `target/m59/menus-150.log`, `target/m59/step6-clippy.log` |
| 7 Theme and alignment | 2026-10-16 | done | Theme file: one surface `#1e1e24`, border `#70758f`, text `#f0f0f5`, muted `#a0a2af`, accent `#3563c9` (hovered `#3d6dd6`, pressed `#2a52aa`), focus `#ffd166`, disabled text `#999cab`; radii 4/6/10; text 16 on 22, headings 22 on 28; buttons centre their text, weight 600, 1-unit border; every focusable kind takes the 2-unit focus border, slots through `kinds.panel`'s border-only variants; toggles and sliders indicate in `#7aa7ff` on track `#363a4b`. `theme.rs` holds `SPACE` 4/8/12/16/24 and the window, row, section, margin, button (6 by 14, at least 96), caption (120), control (140), slot (44), icon (32) and pane sizes; `widgets.rs` builds every screen (screen root, window, heading, button, action row, caption, slider and toggle rows, prompt row); the controls window becomes "Controls" with its readout in the caption column; the pane takes three text rows so the window clears the HUD at 731×411. `TUNGSTEN_UI_LAB_BOUNDS=1` sets `show_bounds`. `every_screen_is_centred_and_aligned_at_three_sizes` → `1 passed` (R1–R7 at 1280×720, 1920×1080, 2560×1440, OS scale 1.75; a mutation moving the grid's padding failed it with `R5: 1280x720 at 1.75: the grid off its panel's centre`, then restored). Contrast script, linear light: every text pair ≥ 4.5:1 (lowest: epic name on raised 4.80:1, rare 4.81:1, hovered accent 4.82:1), every non-text pair ≥ 3:1 (lowest: toggle box border on the window 3.50:1, focus border on the accent 3.85:1, pressed slider fill on its track 3.92:1), 33 pairs, `all pass`. Captures, each exit 0, OS scale 1.75 (Xft.dpi 168): `default-1280x720-bounds.png` 1280x720 (731×411 units); `default-1280x720.png` 1280x720 (731×411 units); `default-1920x1080-bounds.png` 1920x1080 (1097×617 units); `default-1920x1080.png` 1920x1080 (1097×617 units); `default-2560x1440-bounds.png` 2560x1440 (1463×823 units); `default-2560x1440.png` 2560x1440 (1463×823 units); `inventory-1280x720.png` 1280x720 (731×411 units); `pause-1280x720.png` 1280x720 (731×411 units); `settings-1280x720-bounds.png` 1280x720 (731×411 units); `settings-1280x720.png` 1280x720 (731×411 units); `settings-1920x1080-bounds.png` 1920x1080 (1097×617 units); `settings-1920x1080.png` 1920x1080 (1097×617 units); `settings-2560x1440-bounds.png` 2560x1440 (1463×823 units); `settings-2560x1440.png` 2560x1440 (1463×823 units). `--test screens` 15 passed; clippy exit 0 (two `manual_midpoint` fixed); fmt exit 0. Found here: a label grown past its old width wraps its end out of sight (the HUD at frame 100 reads `frame`); the fix is in `core/ui/layout/taffy.rs`, outside this step's files, so it is a known-issues row (step 9), and the lab's live values take `TextWrap::None` (`widgets::value_text`) | `target/m59/captures/` (14 PNGs, logs), `target/m59/step7-clippy.log`; contrast script in the session scratchpad |
| 8 Pixel fixtures and `just visual` | 2026-10-16 | done | Both §2.8 commands exit 0 at OS scale 1.75 (Xft.dpi 168). sha256 `ui-visual.png` `79b1c6593385709541c40976b804f028e41b2152abfc9ad0fe876d28a5971330` → `ebd096ca78d8d319a72b6ee70b2e1999a7197cfb1e9a7e933dd020a048dbd902`; `ui-labels.png` `72c3308f6c8d5a921eb3a95443ad5dd1f2a835ca42df9d447e73cb42133ae266` → `564403aedcfa6579dd566091bb07b572bf490c9b0aea0ee173c76c2b4eda7613`. `just visual` exit 0 with four `test result: ok` lines (`visual_regression` 4, `post_regression` 5, `transition_regression` 1, `ui_regression` 6 passed); the one ERROR line is the expected `load_shaders 'fade'`. The other examples' `tests` `git status` → empty. `rg -c 'TUNGSTEN_CAPTURE_PATH=examples/05_ui_lab/tests/fixtures'` on the tools README → 2 | `target/m59/step8-*.log` |
| 9 Records | 2026-10-16 | done | Known issues: the `focus_direction` and `UiInspector` rows gone (step 1); the `WindowSize` row stays, since W2 did not reproduce; a P2 row for step 7's label wrap in `core/ui/layout/taffy.rs` and a follow-up for a game's Quit (Q14) added. Backlog: drag and drop between widgets. w01 §11: one sentence under the ladder. Ledger: steps 1 and 3's rows. `DESIGN.md`'s Settings and Save Slots paragraph and `docs/getting-started.md` state that a hand size holds (`D-169`); no text for decision 170, which was not written. `just api` exit 0 with no `api/` change (no public item added). `just ctx` exit 0; `just repo-check` exit 0. Unreleased `M59` → 1; backlog `M59` → 1; `LLM_INDEX.md` 12,284 bytes, unchanged (the lab's directory row routes the new modules). Greps: `focus_direction` 0, `UiInspector` 0, `WindowSize` 1, `D-169` in `DESIGN.md` 1 and the guide 1 | `target/m59/step9-*.log` |
| 10 Release 0.79 | 2026-10-16 | done: records closed; the cut and the release checks run after this plan moved to the archive, so their output is in the session's report | Workflow §5: invariants (§6) held: `just visual` passed with `gpu-visual.png`, the post and transition regressions unchanged and no physics, benchmark or render-pass path touched; the budget menu's 100 nodes (`the_menu_fixture_holds_exactly_100_nodes`); the `menus` script's top states at 10, 40, 80 and 110 (`play`, `pause`, `settings`, `pause`); the `debug` row run in step 4; `api/` unchanged. `DESIGN.md`'s status line and Settings paragraph; the register row's release and status; `CHANGELOG.md`'s Unreleased notes; `status: done`; the plan archived to `docs/plans/archive/1.0/`. Another session's untracked `docs/plans/kit-catalogue.md` is in the tree and left alone | this plan |

## Follow-ups

- Example 05 holds no game state between runs: the inventory resets at each start. Saving it would demonstrate save slots (`D-160`) and would come as a later example change.
- The pre-M54 UI rows (UI F6, UI F8) and the minimised-window P2 stay with the Phase 6 QA pass.
- `UiHarness::rect` and `click` at a debug scale (known issues, M58) stay with W1 M6: the alignment checks measure game roots only.

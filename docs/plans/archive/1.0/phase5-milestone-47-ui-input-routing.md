# Phase 5 milestone 47: UI input routing (W1 M3) — draft

- **status:** done
- **goal:** Route each frame's raw input through the UI before gameplay sees it. One `ui_route` system in `pre_update` turns M0b's ordered stream into hover, press, capture, focus and `UiEvent`s against the last committed layout; engine-owned `ui_*` actions drive tab order, directional focus and navigation repeat; focus shows after keyboard use; a hovered widget sets the system cursor; and `RoutedInput` gives gameplay the input the UI did not take, with no key left stuck on either side.
- **non-goals:** State-owned roots, transition opacity, UI tracks, binding-aware prompts and the rebind capture mode (M4); drag past a threshold, scroll containers, movable windows and the inspector's pick behind windows (BC, M5); popups and click-outside dismissal (BC); text fields, IME and clipboard (after 1.0); gamepad bindings and a gamepad device (W5a); game cursors, cursor visibility and grab (a 1.x backlog row); moving the template's code, the examples or the guide's walkthrough onto `RoutedInput` (M4, M6 with W12b); the `02_bench` rows, which build without the routing plugin and keep their workloads; winit 0.31.
- **files to touch:** Step 1: `crates/tungsten-core/src/ui/{mod,tree,node,theme,focus,hit}.rs`, new `crates/tungsten-core/src/ui/{event,route}.rs`, `crates/tungsten-core/src/input.rs`, `crates/tungsten-core/src/input/action_map.rs`, `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/tests/ui/{hit,focus,style,tree}.rs`, `crates/tungsten-core/src/tests/input.rs`, `crates/tungsten-core/src/tests/input/action_map.rs`. Step 2: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `crates/tungsten-core/src/ui/{route,event,tree,focus}.rs`, `crates/tungsten-core/src/input.rs`, new `crates/tungsten-core/src/tests/ui/route.rs`, new `crates/tungsten-core/benches/ui_route_bench.rs`, `crates/tungsten-core/Cargo.toml`. Step 3: `crates/tungsten/src/{plugins,lib,app,input_bridge,testing}.rs`, `crates/tungsten/src/testing/ui.rs`, `crates/tungsten/src/tests/{plugins,testing_ui,input_bridge}.rs`, new `crates/tungsten/src/tests/ui_route.rs`, and the schedule snapshot lines in `templates/basic/tests/game.rs`, `examples/01_platformer/src/tests/main.rs`, `examples/03_scene_state/src/main.rs`, `examples/04_shader_playground/src/main.rs`. Step 4: `examples/05_ui_lab/src/{main,screens}.rs`, `examples/05_ui_lab/tests/screens.rs`. Step 5: nothing in the tree (evidence under `perf-runs/`). Step 6: `DECISIONS.md`, `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/known-issues.md`, `docs/getting-started.md`, `docs/plans/1.0/{w01-ui-text-suite,w04-api-freeze,backlog-1.x}.md`, `api/tungsten-core.txt`, `api/tungsten.txt`. Step 7: the release's files (§3).
- **ordered steps:** (1) Core model and API: interaction state, layered styles, clip-stack hit testing, modal roots, directional focus, cursors, `UiEvent`, `UiRouter`, `RoutedInput`, the `ui_*` actions; then the API review. (2) The router, with decision 149 and its bench. (3) Engine wiring: `UiPlugin`, the app's routed view, exit check, cursor and smoke input, `UiHarness` input. (4) The lab on routing. (5) Perf sitting and gates. (6) Records and docs. (7) Release 0.65.
- **done-when:** Every step's evidence row quoted; workflow §5 closed (§8); the release checks pass after the cut, step 5 having run the gates by change; the preflight prints the command block and the post-merge block.

Candidate W1 M3 ([implementation plan](../../1.0/implementation-plan.md) §3 card, level C), milestone M47, release 0.65. Written 2026-10-13 at `0c2f652`. Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs.

## Context digest

M3 is the UI ladder's third rung ([w01](../../1.0/w01-ui-text-suite.md) §11) and the last before Phase 6's screens: M4's title, pause and settings screens, BC's controls and M5's debug windows take input through it, and the acceptance game's pause screen needs the routed gameplay view ([acceptance game](../../1.0/acceptance-game.md), Pause row). It stands on M0b's ordered raw stream (`D-144`), M1's tree with hit testing, focus and the `interactive` flag reserved for M3 (`D-125`), M2's paint list and clip stack (`D-147`) and the `hovered` and `pressed` style variants, defined since M1 and carried by theme files (`D-125`, `D-148`). The frame-loop gate placed `ui_route` in `pre_update` after the state dispatcher, once a frame on the real clock, with navigation repeat and double-click timing on the real clock (`D-128`, `D-129`; [implementation plan](../../1.0/implementation-plan.md) §11, [w03](../../1.0/w03-frame-loop.md)). Pointer input arrives in physical pixels (`D-144`); UI units are logical (`D-127`). `D-040`'s queues carry `UiEvent`, and `iter_current` keeps one activation from being read twice; `D-045`'s action map takes the `ui_*` actions and has no chord bindings; `D-137` and `D-146` set the fixed edge views the routed view must copy. Known issues hand M3 the hit-test and paint-clip mismatch. Level C: the owner reviews the API after step 1. The design follows w01 §7 and §14; where they leave a choice open, §4 states a default.

## 1. Audit

Read at `0c2f652`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | `InputState` records the frame's input in order and nothing reads it; the app clears it, with the edges, after audio. Pointer positions are physical pixels. | `crates/tungsten-core/src/input.rs:216-269`, `:411-428`, `:700-702`; `crates/tungsten/src/app.rs:1441-1443` |
| A2 | Layout runs after hot reload at `WindowSize / ScaleFactor::get()`, so `pre_update` sees the previous frame's rects; a root added since has none and is not hit. | `crates/tungsten/src/app.rs:1061-1085`, `:1430-1432`; `crates/tungsten-core/src/ui/hit.rs:27-37` |
| A3 | `ActionMap` queries take `&InputState`; the default map has no `ui_*` action and binds Escape to `engine_exit`, Enter to `state_start`, Space to `jump`, the arrows to movement; bindings carry no modifiers (`D-045` choice 1). | `crates/tungsten-core/src/input/action_map.rs:71-235`, `:322-350` |
| A4 | `pre_update` runs the debug toggles, `display_input`, then `state_dispatcher`; `DefaultPlugins` holds eight plugins; five tests snapshot that line. | `crates/tungsten/src/plugins.rs:115-126`, `:213-231`; `crates/tungsten/src/tests/plugins.rs:10`, `:21`; `templates/basic/tests/game.rs:47`; `examples/01_platformer/src/tests/main.rs:304`; `examples/03_scene_state/src/main.rs:360`; `examples/04_shader_playground/src/main.rs:1297` |
| A5 | The dispatcher runs the active state's `update` inside `pre_update`, so a state reads input before any stage after it. | `crates/tungsten/src/state.rs:207-234` |
| A6 | The exit check reads raw `engine_exit` after `update`; the app turns `InputState`'s fixed view on around each step and ends it after each step and on a stopped clock. | `crates/tungsten/src/app.rs:482-494`, `:983-992`, `:945-947`, `:967-969` |
| A7 | Hit testing counts a widget only inside every ancestor's box while paint clips only at `clip_children`; M1's test pins the old rule; a button's label, not the button, is the hit. | `crates/tungsten-core/src/ui/hit.rs:20-37`; `crates/tungsten-core/src/ui/paint.rs:597-611`; `docs/known-issues.md:21`; `crates/tungsten-core/src/tests/ui/hit.rs:51-68`, `:82-86` |
| A8 | `WidgetState` has `Normal`, `Focused`, `Disabled`; the kinds' `hovered` and `pressed` overrides exist and resolve nowhere; resolution applies one state's variant. | `crates/tungsten-core/src/ui/theme.rs:302-336`, `:458-466`, `:523-547`; `crates/tungsten-core/src/ui/tree.rs:695-704`, `:754-756` |
| A9 | Focus moves in tab order inside a scope and returns only when the scope is removed; hiding, collapsing or disabling drops it. | `crates/tungsten-core/src/ui/focus.rs:81-92`, `:175-203`; `crates/tungsten-core/src/ui/tree.rs:476-514` |
| A10 | `interactive` marks nothing yet; buttons are interactive and focusable; every node starts hit-testable. | `crates/tungsten-core/src/ui/tree.rs:558-570`; `crates/tungsten-core/src/ui/node.rs:188-206` |
| A11 | No code sets the window's cursor; winit is 0.30.12. | `rg -n set_cursor crates` finds only `Harness::set_cursor` (`crates/tungsten/src/testing.rs:170`); `Cargo.toml:41` |
| A12 | `UiHarness` lays out at `WindowSize` whatever the scale, and the harness scripts only actions and the cursor. | `crates/tungsten/src/testing/ui.rs:100-114`; `crates/tungsten/src/testing.rs:136-172` |
| A13 | The lab focuses its popup's "Drop it" at build and marks only the HUD root pointer-transparent; its fixture is captured in a smoke run, where a pointer resting over a button would hover it once routing exists. | `examples/05_ui_lab/src/screens.rs:58`, `:155`; `examples/05_ui_lab/tests/ui_regression.rs:30-46` |
| A14 | `02_bench` builds from an explicit plugin set without routing, and its captures run in smoke mode. | `examples/02_bench/src/main.rs:78-90` |
| A15 | `UiDebug::hide_game_layers` stops the game layers' paint; hit testing ignores it. | `crates/tungsten-core/src/ui/paint.rs:492-498` |
| A16 | The template reads `InputState` for movement and states and builds no UI; the guide calls `pre_update` the hotkeys and the dispatcher; `DESIGN.md`'s `pre_update` line lacks `ui_debug_toggle`. | `templates/basic/src/game.rs:62-69`, `templates/basic/src/states.rs:95-101`; `docs/getting-started.md:75`; `DESIGN.md:387` |
| A17 | `docs/LLM_INDEX.md` is 12,276 of its 12,288 B; its UI-tree row already routes `core/ui/` and `core/tests/ui/`. | `just ctx`; `docs/LLM_INDEX.md:61` |

## 2. Design

**Placement.** `UiPlugin` (umbrella, `UI_ROUTE = "ui_route"`) registers `UiEvent` and `ui_route_system` in `PreUpdate`, `after_if_present(STATE_DISPATCHER)`; `DefaultPlugins` adds it after `StatePlugin`, so the snapshot line ends `state_dispatcher, ui_route`. Once a frame the system reads `InputState::raw_events()` in order against the last committed layout, dividing pointer positions by `ScaleFactor::get()` (`D-127`), and keeps time with `Time::real_delta()`. A root routes once it is visible and laid out; with `hide_game_layers` the game layers do not route. Core holds the logic and no winit type (`D-007`); the umbrella applies the cursor.

**Hit testing (amends `D-125`).** The walk is paint order reversed, through the paint clip stack: a widget counts inside its own rect and every `clip_children` ancestor's rect within the viewport, edges included; a non-clipping ancestor no longer bounds it, which closes the known-issues row. A visible, laid-out root flagged modal blocks every root below it in hit order; roots above it, Debug roots among them, still hit. The hit's target is its nearest enabled interactive ancestor-or-self.

**Pointer.** Each routing first recomputes hover from the last position, so a widget that moved under a still cursor updates. Then per event: a move updates the hovered target (`PointerEntered`, `PointerLeft`) and the cursor; a primary press on a target presses and captures it (`Pressed`) and focuses it when focusable, focus hidden (Q4); while captured only the target hovers, shown pressed while the pointer is over it; the release sends `Released`, then `Activated` when over the target, then `DoubleClicked` for the second activation of that target within `double_click_time` and `double_click_distance`. Another button on a target sends only `Pressed` and `Released` with its button. Focus loss, pointer leave (Q17) and the target's removal, hiding or disabling end a press with `PressCancelled`. A press or wheel over a routable widget, or anywhere while a modal root is shown, is the UI's.

**Keyboard.** The UI takes the keyboard (`UiRouter::wants_keyboard`) while a modal root is shown, or while a widget holds focus and focus is visible (Q3). A press of a key bound to a `ui_*` action is then the UI's, and under a modal every press is. `ui_next` moves in tab order, `ui_prev` or Shift with `ui_next` backwards (Q5); the four directions move by `focus_direction` and repeat while held (Q9); `ui_accept` activates the focused widget on its press; `ui_cancel` sends `Cancel { scope }` (the focused widget's scope, else the modal root) and, outside a modal, clears focus (Q11). OS repeats drive nothing. A laid-out modal root with no focus inside it takes focus on its first focusable widget (Q12). `FocusGained` and `FocusLost` follow every focus change since the last routing, the game's own included.

**Ownership.** A held key or button belongs to the side that got its press: the UI's releases never reach the routed view and the routed view's always do, so neither side keeps a stuck key; focus loss releases both. Moves, pointer leave, modifiers and focus always reach the routed view.

**Focus visibility and styles.** `UiRouter::last_device()` starts `Keyboard`; a key press sets `Keyboard`, a pointer press or wheel `Pointer`, a move nothing (Q6). Focus is visible while it is `Keyboard`. `WidgetState` gains `Hovered` and `Pressed`; resolution layers the `focused` variant (visible focus only), then `hovered`, then `pressed`, or applies `disabled` alone; `widget_state` names the top state (Q7).

**Routed view.** `RoutedInput` wraps an `InputState` that only `ui_route` feeds, through the same mutators, so `ActionMap` reads it by deref and its frame and fixed views behave as `InputState`'s (`D-137`, `D-146`). Its frame boundary is the routing: a reader before `ui_route` (the dispatcher's states) sees the previous routing's edges once (Q1). `InputState` is untouched and stays what the engine toggles read.

**App and harness.** The app inserts `UiRouter` and `RoutedInput`, runs `RoutedInput`'s fixed view and step ends beside `InputState`'s, reads `engine_exit` from `RoutedInput` when `ui_route` ran that frame (a press the UI took does not exit), sets the window's cursor when `UiRouter::cursor()` changes, and drops the window's key, button, pointer and wheel events in smoke runs (Q8). `UiHarness` lays out at `WindowSize / ScaleFactor::get()` as the app does, and scripts input.

API sketch (step 1 lands the core part; every new item gets rustdoc):

```rust
// tungsten_core::ui
pub enum WidgetState { Normal, Focused, Disabled, Hovered, Pressed } // already #[non_exhaustive]
#[non_exhaustive] pub enum NavDirection { Up, Down, Left, Right }
#[non_exhaustive] pub enum InputDevice { Keyboard, Pointer }
#[non_exhaustive] pub enum CursorIcon { Default, Pointer, Text, Move, NotAllowed,
    EwResize, NsResize, NwseResize, NeswResize }
#[non_exhaustive] pub enum UiEvent { // each variant #[non_exhaustive]
    PointerEntered { widget: WidgetId }, PointerLeft { widget: WidgetId },
    Pressed { widget: WidgetId, button: MouseButton }, Released { widget: WidgetId, button: MouseButton },
    PressCancelled { widget: WidgetId }, Activated { widget: WidgetId }, DoubleClicked { widget: WidgetId },
    FocusGained { widget: WidgetId }, FocusLost { widget: WidgetId }, Cancel { scope: WidgetId } }
impl UiEvent { pub fn widget(&self) -> WidgetId }
impl EventQueue<UiEvent> { pub fn activated(&self, widget: WidgetId) -> bool } // iter_current
impl UiTree {
    pub fn hovered(&self) -> Option<WidgetId>; pub fn pressed(&self) -> Option<WidgetId>;
    pub fn focus_visible(&self) -> bool;
    pub fn set_modal(&mut self, root: WidgetId, on: bool); pub fn modal(&self, id: WidgetId) -> bool;
    pub fn set_focus_neighbor(&mut self, id: WidgetId, dir: NavDirection, to: Option<WidgetId>);
    pub fn focus_neighbor(&self, id: WidgetId, dir: NavDirection) -> Option<WidgetId>;
    pub fn set_focus_wrap(&mut self, scope: WidgetId, on: bool); pub fn focus_wrap(&self, id: WidgetId) -> bool;
    pub fn focus_direction(&mut self, dir: NavDirection) -> Option<WidgetId>;
    pub fn set_cursor(&mut self, id: WidgetId, cursor: Option<CursorIcon>);
    pub fn cursor(&self, id: WidgetId) -> Option<CursorIcon>;
}
pub struct UiRouter { .. } // World resource; Default holds Q9's settings
impl UiRouter {
    pub fn settings(&self) -> &UiRouteSettings; pub fn settings_mut(&mut self) -> &mut UiRouteSettings;
    pub fn last_device(&self) -> InputDevice; pub fn pointer_over_ui(&self) -> bool;
    pub fn wants_keyboard(&self) -> bool; pub fn cursor(&self) -> CursorIcon;
}
#[non_exhaustive] pub struct UiRouteSettings { pub repeat_delay: f32, pub repeat_interval: f32,
    pub double_click_time: f32, pub double_click_distance: f32 }
pub const UI_ACCEPT: &str = "ui_accept"; // and UI_CANCEL, UI_NEXT, UI_PREV, UI_UP, UI_DOWN, UI_LEFT, UI_RIGHT
pub fn ui_route_system(world: &mut World); // step 2
// tungsten_core::input
#[derive(Default)] pub struct RoutedInput(..); // Deref and DerefMut to InputState
// tungsten (step 3)
pub const UI_ROUTE: &str = "ui_route"; pub struct UiPlugin;
impl UiHarness { pub fn pointer_to(&mut self, x: f32, y: f32); pub fn pointer_down(&mut self);
    pub fn pointer_up(&mut self); pub fn click(&mut self, widget: WidgetId);
    pub fn key_down(&mut self, key: KeyCode); pub fn key_up(&mut self, key: KeyCode);
    pub fn find(&self, name: &str) -> Option<WidgetId>; } // by accessible name, tree order
```

Default bindings: `ui_accept` Enter and Space, `ui_cancel` Escape, `ui_next` Tab, the directions the arrows, `ui_prev` none. Core and the root re-export `RoutedInput`, `UiEvent` and `UiRouter`. Template: its code is unchanged; only its schedule snapshot line gains `ui_route` (Q16).

## 3. Steps

### Step 1: Core model and API

- **Files:** step 1's in the header.
- **Change:** `WidgetState::{Hovered, Pressed}` and `KindStyle::variant`; the tree's `hovered`, `pressed` and `focus_visible` (true on a new tree), crate setters that mark the widgets' styles dirty, cleared by removal, hiding and disabling; layered resolution behind the unchanged `theme::resolve`; `set_modal`, `set_cursor`, focus neighbours and wrap in node data; `focus_direction` (explicit neighbour; else the nearest laid-out focusable widget in the scope whose centre lies that way, scored as Q9 says, ties in tab order; wrap searches again from the scope's opposite edge); hiding or collapsing a scope restores the focus recorded on entering it, as removal does (Q12); hit testing per §2; `UiEvent` with `widget()` and `EventQueue<UiEvent>::activated`; `UiRouter`, `UiRouteSettings`, `InputDevice`, `NavDirection`, `CursorIcon` and the action names as types and accessors; `RoutedInput`; the default map's `ui_*` bindings; re-exports. Rustdoc cites w01 §7 and §14 until step 2 writes decision 149.
- **Done-when:** `cargo test -p tungsten-core --locked ui::` → passes, with new tests: an overflowing child of a non-clipping parent is hit and one outside a clipping ancestor is not; edges stay included; a modal root blocks lower roots and a Debug root above it still hits; an unlaid root is not hit; `focus_direction` takes the explicit neighbour first, then the scored nearest, skips disabled and hidden widgets, stays in its scope, and wraps only with the flag; a hovered, visibly focused button keeps the focus border and takes the hover fill, pressed wins the fill, disabled applies alone, and a hidden focus drops the `focused` variant; hiding a scope restores the entry focus. `cargo test -p tungsten-core --locked input::` → passes, the default map's `ui_*` actions bound as §2 lists and `ui_prev` unbound. `cargo check --workspace --all-targets --locked` → passes; `cargo clippy -p tungsten-core --all-targets --locked -- -D warnings` → clean; `RUSTDOCFLAGS='-D warnings' cargo doc -p tungsten-core --no-deps --locked` → no warnings.
- **Moves:** M1's `a_child_outside_its_parents_box_is_not_hit` becomes the clip-stack rule's test (the paint clip decides).
- **Then:** stop for the API review (level C): §2's sketch, step 1's diff of `ui/` and `input`, and the template's planned one-line diff.

### Step 2: The router

- **Files:** step 2's in the header.
- **Change:** write decision 149 and its index row with the tungsten-decision skill first, with `D-125`'s amendment marker. Then `ui_route_system` per §2: routable roots, the pointer, keyboard and ownership rules, navigation repeat on the real clock, double click, focus visibility, auto-focus into a modal, the cursor (the captured widget's, else the hit's nearest `set_cursor` ancestor-or-self, else `Default`), and `RoutedInput` fed after its `begin_frame`. Events go to `EventQueue<UiEvent>` only when it is registered. The bench `ui_route_bench`: an empty tree with an empty stream, and a laid-out 100-widget menu with a move and a click.
- **Done-when:** `cargo test -p tungsten-core --locked ui::route` → passes, the tests driving a hand-built `World` (`InputState`, `ActionMap::default_map()`, a laid-out tree, `Time`, `ScaleFactor`, the router, `RoutedInput`, the event queue): event order (a move, press and release in one stream make one `Activated` where the move put the pointer); release outside (`Released` without `Activated`, pressed only while over); focus loss (`PressCancelled`, hover cleared, `RoutedInput` releases its held keys with release edges, the UI's held keys forgotten); single activation (one `Activated` per click or `ui_accept` press, none from OS repeats or a held key, and `activated()` true in one frame where `iter()` sees two); directional focus through the arrows, repeating at 0.5 s then every 0.1 s and at most once a routing; ownership (a key pressed before focus showed releases to the routed view, one the UI took never reaches it); modal (every press is the UI's, a Debug root above still hits); Escape (taken with visible focus, `Cancel` sent and focus cleared outside a modal, not taken without focus); scale 2 (a physical click hits the logical rect); double click; focus visibility (a key shows it, a click hides it, a move leaves it); the cursor; focus changed by game code reported at the next routing; a root added this frame not routed before its layout. `cargo test -p tungsten-core --locked` → passes; `cargo bench -p tungsten-core --bench ui_route_bench --no-run --locked` → builds; `rg -c '^## D-14[9] ' DECISIONS.md` → 1; ``rg -c '^\| `D-14[9]`' docs/DECISION_INDEX.md`` → 1.
- **Moves:** none.

### Step 3: Engine wiring

- **Files:** step 3's in the header.
- **Change:** `UiPlugin` and `UI_ROUTE`, `DefaultPlugins` with nine plugins; the app inserts `UiRouter` and `RoutedInput`, drives `RoutedInput`'s fixed view and step ends beside `InputState`'s (the stopped-clock drop included), reads the exit from `RoutedInput` when `ui_route` ran that frame, applies the cursor through an `input_bridge` mapping, and ignores key, button, pointer and wheel events in smoke runs; `UiHarness` lays out at the logical viewport and gains the §2 helpers; the five snapshot lines gain `ui_route` and the plugin count reads 9.
- **Done-when:** `cargo test -p tungsten --locked` → passes, with harness tests in `tests/ui_route.rs`: an `update` system reading `activated()` sees one activation per click; a `fixed_update` system on `RoutedInput` sees each press in exactly one step at 144 Hz and at 30 Hz; a click on a button never reaches `RoutedInput` and one beside the UI does; Escape with visible focus leaves `exit_requested()` false and without focus sets it; a scripted sequence leaves `InputState`'s edges, levels and `raw_events()` identical with and without `UiPlugin`; the cursor follows hover; at scale 2 the harness lays out at the logical viewport and a physical click lands. `tests/input_bridge.rs` maps every `CursorIcon`. `cargo test -p tungsten-template-basic --locked`, `-p example-01-platformer`, `-p example-03-scene-state`, `-p example-04-shader-playground` → pass, each changed only in its snapshot line. `cargo check --workspace --all-targets --locked` → passes; `just smoke` → passes.
- **Moves:** the five schedule snapshots; `DefaultPlugins::set().len()` 8 → 9.

### Step 4: The lab on routing

- **Files:** step 4's in the header.
- **Change:** the popup becomes modal; activating its "Cancel" or "Drop it" button, or a `Cancel` event, hides it; the window's "Drop" shows it again and focuses "Drop it", the reaction system living in `screens.rs` so the test can register it; the buttons ask for `CursorIcon::Pointer`; the HUD's icon and counter become pointer-transparent. The startup state and every label's text stay as they are, so the fixture holds.
- **Done-when:** `cargo test -p example-05-ui-lab --locked` → passes, with one harness test: Escape hides the popup and drops its focus, a click on the window's "Drop" while the popup shows changes nothing, after the popup hides the same click shows it with "Drop it" focused. `TUNGSTEN_VISUAL_REGRESSION=1 cargo test -p example-05-ui-lab --locked --test ui_regression` → 4 passed (fixture at tolerance 0); `just smoke` → passes.
- **Moves:** none.

### Step 5: Perf sitting and gates

- **Files:** none in the tree; evidence in `perf-runs/<date>-m47-ui-routing/`.
- **Change:** a `git archive HEAD` copy of the 0.64 tree as the base; four prebuilt `example-02-bench` binaries (base and candidate, plain and with LLVM's align-all flags) checked with `nm`; one blocking sitting: six suites (`--repeat 5`, every CPU row: base A and B, candidate A and B, base and candidate aligned), the compares, `ui_route_bench` on the candidate; then `just smoke`, `just visual` in each tree under its own `TMPDIR`, and `just physics-release`.
- **Done-when:** both A/A pairs read 0 `regressed` and 0 `improved` on owned metrics outside the profiling workflow's two-valued ones (Compare item 8); both direct pairs read every owned metric of every row not `regressed`, or cleared as placement under Q15; digests 8 of 8 in every compare; the empty `ui_route_bench` case's median is under 1 µs, the menu case recorded; `just smoke` passes; `just visual` passes and each capture is `cmp`-equal to the base tree's; `just physics-release` passes.
- **Moves:** none.

### Step 6: Records and docs

- **Files:** step 6's in the header.
- **Change:** decision 149 gains the sitting's numbers; `DESIGN.md` gains a "UI Input Routing — M47" section, the frame loop's `pre_update` and exit-check wording, the M38 plugin list and snapshot line, and M44's "nothing routes the stream yet" sentence pointing at it; `docs/LLM_INDEX.md`'s UI-tree row names routing, decision 149 and `tungsten/tests/ui_route.rs`, other rows trimmed to fit; the known-issues hit and paint-clip row removed; the guide's `pre_update` phrase names routing, and one sentence names `RoutedInput`; w01 §11's M3 row marked landed; three ledger rows (the hit rule and scope restore; focus visibility and layered states; `DefaultPlugins` routing, the exit check and `UiHarness`'s viewport); a backlog row for game cursors, visibility and grab; `just api`.
- **Done-when:** `just api` → `git diff --stat api/` shows the §2 surface and the ledger's breaks only; `just api-check` → passes; `just ctx` → OK with `docs/LLM_INDEX.md` at most 12,288 B; `just repo-check` → passes; `rg -c 'core/ui/hit.rs' docs/known-issues.md` → 0; `rg -c -F '0.65 (M47)' docs/plans/1.0/w04-api-freeze.md` → 3.
- **Moves:** none.

### Step 7: Release 0.65

- **Files:** `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row, the README's four owned Now fields (`roadmap.py summary`), this plan (moved to `docs/plans/archive/1.0/`)
- **Change:** delete step 5's scratch trees under `target/` (the template check needs 3 GiB free); close workflow §5 and this step's evidence row with `status: done`; then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.65.0` once, every release check once after it (step 5 ran smoke, visual and the physics release tests; the chain reruns smoke), then the preflight with `--message`.
- **Done-when:** [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut); `just release-preflight 0.65.0 --repo JosephGibson/Tungsten --message 'Update 0.65: UI input routing'` → passes and prints the command block and the post-merge block.
- **Ends with:** the changed-file list, the command block, the post-merge block, then the next prompt: the plan prompt for the next register row, W16a.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | `ui_route` before or after the state dispatcher? | After, as the gate placed it. States read `InputState` in-frame, or `RoutedInput` and `UiEvent`s (through `iter`) one routing late; M4 decides whether state updates move after routing. |
| Q2 | Is the routed view the default for gameplay? | No: `RoutedInput` is opt-in and `InputState` stays the raw truth for every reader; the exit check is the one engine reader of the routed view. |
| Q3 | When does the UI take the keyboard? | While a laid-out modal root is shown (every press) or a widget holds visible focus (presses of `ui_*` keys). A HUD whose widgets hold no visible focus never takes Space from gameplay. |
| Q4 | Does a click move focus? | Yes, to a focusable target, with focus hidden. |
| Q5 | `ui_prev`'s default? | Unbound; Shift with `ui_next` goes back, since `D-045` has no chords. |
| Q6 | What shows focus? | The last device: it starts `Keyboard`, so a focus set at build shows (the lab's fixture holds); key presses set `Keyboard`, pointer presses and wheel `Pointer`; moves nothing. |
| Q7 | One style state or layers? | Layers: `focused` (visible), `hovered`, `pressed` in order, `disabled` alone; `widget_state` returns Disabled > Pressed > Hovered > Focused > Normal; `theme::resolve` keeps its signature. |
| Q8 | Smoke runs drop window input? | Yes: key, button, pointer and wheel events, so a resting pointer cannot move a pixel fixture; focus, resize and scale still apply. |
| Q9 | Navigation constants? | Repeat after 0.5 s, every 0.1 s, directions only, at most once a routing; double click within 0.5 s and 4 units; direction score = axis distance + 2 × perpendicular offset; wrap off. |
| Q10 | Hit testing through the clip stack, with modal roots? | Yes, amending `D-125`'s rule. |
| Q11 | Escape? | `Cancel { scope }`; outside a modal it also clears focus, so the next Escape reaches gameplay and the exit check. |
| Q12 | Modal auto-focus; hiding a scope restores focus? | Yes to both. |
| Q13 | Cursors per kind? | No: per node, inherited; the lab's buttons set `Pointer`. |
| Q14 | A bench for the router? | Yes, `ui_route_bench`; the empty case under 1 µs. |
| Q15 | Placement control in step 5? | The aligned pair, as M45 and M46 used: a direct-pair `regressed` clears when the aligned pair reads it `unchanged` and `nm` shows its function moved modulo 64; a padded baseline only if the owner asks. |
| Q16 | Template and guide? | Template code unchanged, its snapshot line gains `ui_route`; the guide names routing in `pre_update` and `RoutedInput` in one sentence. |
| Q17 | Pointer leave cancels a press? | Yes (w01 §7); BC revisits capture past the window edge for sliders. |

Approved 2026-10-13 with the stated defaults.

## 5. Decisions expected

149: UI input routing (W1 M3): `ui_route` and `UiPlugin`, `UiEvent`, the `ui_*` actions, keyboard and pointer ownership, modal roots, focus visibility, layered states, cursors, `RoutedInput`, the exit check and smoke input. Amends `D-125` (hit testing through the clip stack; `focused` only while focus is visible; hiding a scope restores focus); extends `D-040`, `D-045`, `D-088`, `D-128` and `D-144`. Next free ID at writing; checked again when step 2 runs.

## 6. Invariants

- **May move:** the five schedule snapshot lines and the plugin count (step 3); M1's overflow hit test (step 1); the default map's action list (step 1). Each in its step's evidence and the `CHANGELOG.md` line.
- **Must not move:** the determinism hash, the pinned containment hash, the row digests, `gpu-visual.png`, the post and transition regressions, `ui-visual.png` and every `just visual` capture (byte-equal to the 0.64 tree's), the `02_bench` workloads, `InputState`'s behaviour, and the examples' and template's behaviour.

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with steps that don't depend on it (2 needs 1, 3 needs 2, 4 needs 3, 5 needs 1–4, 6 needs 2–5).
- The API review changes the API: §2 and the step 1 code take the change before step 2 starts.
- A file outside §files to touch needs a change.
- A visual capture, a hash or a digest differs from the 0.64 tree's.
- Step 5: an owned metric reads `regressed` and Q15 does not clear it, or an A/A pair fails again after its row reran once with `--only`; a suite invalidated by the encoder is set aside and rerun once.
- The index cannot fit 12,288 B without dropping a route (`D-106` owns the budget).

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 5, release | Every CPU row, the bench, smoke, visual, the physics release tests in step 5; the release checks after the cut |
| 3 Invariants | 5 | |
| 4 Decisions | 2, 6 | 149 before the code cites it; its numbers in step 6 |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1–3, 6 | Rustdoc as items land; three ledger rows and `just api` in step 6 |
| 6 Template and guide | 3, 6 | The template's snapshot line; the guide's `pre_update` sentence |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 6 | |
| 8 Design | 6 | |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 6, release | Gap log n/a: no rows; `python3 -B scripts/roadmap.py summary` supplies the owned fields |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

Not run: `critique: codex exec failed (exit 1, empty output)`. Reviewer: the `critique` skill with `--repo` (gpt-6.1-sol through the Codex CLI, another model family), round 1, 2026-10-13, on this plan at `0c2f652` plus the uncommitted plan file; effort xhigh, since the stop's `plan` stage recommends Opus 5.5, not Fable 5.1. Codex returned `503 Service Unavailable` and "Selected model is at capacity" before any output. The skill does not retry; no finding was checked or applied, and the plan was not reviewed in the critic's place.

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-13 | Pass; API review pending (level C) | `cargo test -p tungsten-core --locked ui::` → 102 passed, 0 failed (625 filtered out); `input::` → 48 passed, 0 failed (679 filtered out); `cargo check --workspace --all-targets --locked` → finished, no warnings; `cargo clippy -p tungsten-core --all-targets --locked -- -D warnings` → clean; `RUSTDOCFLAGS='-D warnings' cargo doc -p tungsten-core --no-deps --locked` → no warnings. Beyond done-when: all of `-p tungsten-core` 757 passed; `-p tungsten -p example-05-ui-lab` 379 passed. Moved: `a_child_outside_its_parents_box_is_not_hit` → `the_paint_clip_decides_whether_an_overflowing_child_is_hit`. Defaults taken: `focus_direction` from no focus → `None`; an explicit neighbour may sit in another scope and falls back to the search when it cannot take focus; `ui_prev` listed with no bindings; `UiEvent` is `Copy + PartialEq`, not `Eq` (room for `ValueChanged`); re-entering a scope replaces its focus record (bounds `focus_return`, restore unchanged). `UiTree::set_focus_visible` carries `cfg_attr(not(test), expect(dead_code))` until step 2's router calls it; step 2 removes it. | Step 1's files (header); logs not kept |
| 2 | 2026-10-13 | Pass | `cargo test -p tungsten-core --locked ui::route` → 19 passed, 0 failed; `cargo test -p tungsten-core --locked` → 776 passed, 0 failed, 5 ignored (release-only); `cargo bench -p tungsten-core --bench ui_route_bench --no-run --locked` → builds; `rg -c '^## D-14[9] ' DECISIONS.md` → 1; the index row count → 1. Clippy and rustdoc with `-D warnings` clean. Mutation check: dropping the scale division, or judging a key press by the state after it, fails the scale and ownership tests. The menu case is 100 buttons (201 nodes). Informal bench with NoMachine connected, not evidence: empty 169 ns, menu move and click 3.66 µs. The step's file copies were made after its first edits, from `HEAD` for the decision files and by reversing the step's edits for the code; no restore was needed. | `crates/tungsten-core/src/ui/route.rs`, `src/tests/ui/route.rs`, `benches/ui_route_bench.rs`; `DECISIONS.md` D-149 |
| 3 | 2026-10-13 | Pass | `cargo test -p tungsten --locked` → 382 passed, 0 failed (7 new in `tests/ui_route.rs`, 1 in `tests/input_bridge.rs`); `cargo test --locked` for `-p tungsten-template-basic` 7, `-p example-01-platformer` 106, `-p example-03-scene-state` 3 and `-p example-04-shader-playground` 6 passed, each changed only in its snapshot line; `cargo check --workspace --all-targets --locked` → passes; `just smoke` → passes (examples 5/5, template 1/1, benchmarks 15/15, every other row). Clippy with `-D warnings` clean on the umbrella. Moved: the five snapshot lines gain `ui_route` (the platformer's before its own `platformer_bindings`); `DefaultPlugins::set().len()` 8 → 9. Finding: `testing::tests::an_engine_exit_press_stops_step` presses Escape from an update system, after the routing, so the routed view alone never exited; the exit check also reads, raw, key and button presses made after `PreUpdate`, which the UI never saw, and D-149 says so. | `crates/tungsten/src/{app,plugins,input_bridge,lib}.rs`, `testing/ui.rs`, `tests/ui_route.rs` |
| 4 | 2026-10-13 | Pass | `cargo test -p example-05-ui-lab --locked` → 6 passed, 0 failed (`screens` 2 with the new harness test, `ui_regression` 4 with pixels skipped); `TUNGSTEN_VISUAL_REGRESSION=1 cargo test -p example-05-ui-lab --locked --test ui_regression` → 4 passed, `ui_visual_matches_fixture` at 0 pixels above tolerance 0; `just smoke` → passes (examples 5/5, every row). The harness test fails with `set_modal` removed (mutation check), through its no-hover and no-activation asserts. The HUD's icon and counter are pointer-transparent; the four buttons ask for `Pointer`; `UiLab` gains `drop`, `cancel` and `drop_it`. | `examples/05_ui_lab/src/{screens,main}.rs`, `tests/screens.rs` |
| 5 | 2026-10-13 | Pass | Sitting 08:10–08:34 UTC with the encoder absent; all 48 captures valid on the first attempt (5 runs each); the tree's diff hash unchanged across it; first-run digests 8 of 8 in all five compares. A/A: `base-aa` reads `gpu-throughput` `render_encode` p95 `regressed` (1.962 → 2.644 ms), a Compare item 8 two-valued reading; `cand-aa` none. Direct pairs: every owned metric not `regressed` except `churn` `flush` p50 in `direct` (2.426 → 2.510 ms, τ 0.073), cleared under Q15: the aligned pair reads it `unchanged` (Δ −0.020), `direct-b` not `regressed`, and `World::flush_reusing` moved from address mod 64 = 16 to 32 (0 in both aligned builds). `ui_route_bench`: empty median 179.4 ns (interval 178.9–182.5), menu move and click 3731 ns. `just smoke` passes; `just visual` passes in both trees (14 tests each) with 37 of 37 captures `cmp`-equal; `just physics-release` passes (12 tests). | `perf-runs/20261013-m47-ui-routing/` (`README.md`, `summary.md`, `nm/`, `criterion/`) |
| 6 | 2026-10-13 | Pass | `just api` → `git diff --stat api/`: `api/tungsten-core.txt` +475, `api/tungsten.txt` +45, no line removed, every item §2's at each re-export path; `just api-check` → "Public API snapshots current: 3 crates"; `just ctx` → OK, `docs/LLM_INDEX.md` 12,281 B (the UI-tree row names routing, `D-149` and `tungsten/tests/ui_route.rs`; `core/text.rs` and `core/ecs/event_queue.rs` dropped where an adjacent row already routes them); `just repo-check` → 0 errors; `rg -c 'core/ui/hit.rs' docs/known-issues.md` → 0; `rg -c -F '0.65 (M47)' docs/plans/1.0/w04-api-freeze.md` → 3. D-149 gains the sitting's numbers; DESIGN.md gains the M47 section, the frame loop's routing and exit wording, the M38 list and snapshot line and M44's pointer; the guide names routing and `RoutedInput`; w01 §11's M3 row reads landed; the ledger's planned single-activation row reads landed as an addition, not a break; a backlog row for game cursors, visibility and grab. | `DECISIONS.md`, `DESIGN.md`, `docs/`, `api/` |
| 7 | 2026-10-13 | Pass | Workflow §5 closed: the invariants held (the physics release tests' hashes, the row digests 8 of 8, the 37 `just visual` captures byte-equal to 0.64's, `InputState` the same with and without `UiPlugin`, the examples' and template's tests changed only in their snapshot lines); `DESIGN.md` gained the M47 section (step 6) and its status line; the register row's release and status; the README's four owned Now fields from `roadmap.py summary`; one `CHANGELOG.md` entry; step 5's scratch trees deleted (226 GB free). Then the plan archived, `just release-cut 0.65.0` and the release checks once after it, quoted in the session's report. | — |

## Follow-ups

- Drag past a threshold: BC's sliders and M5's movable windows build it on M3's capture.
- Click-outside dismissal and popup placement: BC.
- `UiRouter::last_device()` for binding-aware prompts: M4; a gamepad device: W5a.
- The inspector's pick reads `InputState` before routing: M5 moves it behind `pointer_over_ui()`.
- Whether the dispatcher's state updates run after routing (Q1): M4.

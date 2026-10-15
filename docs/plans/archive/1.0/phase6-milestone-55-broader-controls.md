# Phase 6 milestone 55: checkboxes and sliders (W1 BC) — done

- **status:** done
- **goal:** Give `core/ui` the two broader controls M4's settings screen needs, a checkbox (`WidgetKind::Toggle`) and a horizontal slider (`WidgetKind::Slider`): closed behaviours whose values the router changes by pointer, keyboard and the `ui_*` actions and reports as `UiEvent::ValueChanged`, painted as paint-list rectangles in kind styles the existing style model expresses, with a controls window in the UI lab.
- **non-goals:** Scroll containers and the drag threshold (M5, with movable windows); progress bars, collapsible sections, tooltips, popups, click-outside dismissal and window resizing (a backlog row, step 5); editable fields (`D-102`); a style-model change (a new style property or a `checked` state); vertical sliders; sprite-skinned controls; a label that activates its control; the wheel, Home, End, PageUp or PageDown on a slider; analog stick values on a slider; a new pixel capture; the template; M4's settings screen.
- **files to touch:** Step 1: `crates/tungsten-core/src/ui/{control.rs (new),mod.rs,node.rs,event.rs,tree.rs,theme.rs,theme_file.rs,layout/taffy.rs,paint.rs}`, `crates/tungsten-core/src/tests/ui/{control.rs (new),theme_file.rs,style.rs}`, `api/tungsten-core.txt`, `docs/plans/1.0/w04-api-freeze.md`. Step 2: `crates/tungsten-core/src/ui/paint.rs`, `crates/tungsten-core/src/ui/control.rs` (the geometry helpers), `crates/tungsten-core/src/tests/ui/paint.rs`. Step 3: `crates/tungsten-core/src/ui/route.rs`, `crates/tungsten-core/src/ui/control.rs` (the pointer and step helpers), `crates/tungsten-core/src/tests/ui/route.rs`, `crates/tungsten/src/tests/ui_route.rs`. Step 4: `examples/05_ui_lab/src/{screens.rs,main.rs}`, `examples/05_ui_lab/assets/ui/themes/ex05_default.json`, `examples/05_ui_lab/tests/screens.rs`. Step 5: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/plans/1.0/{w01-ui-text-suite.md,backlog-1.x.md,w04-api-freeze.md}`, `crates/tungsten-core/src/ui/{control.rs,event.rs}` (doc comments only), `docs/known-issues.md` (only for findings), `api/*.txt` (only if `just api` moves them). Step 6: the skeleton's release files (§3).
- **ordered steps:** (1) Kinds, values and the event (the API), then the level C API critique. (2) Paint. (3) Routing. (4) The UI lab's controls window. (5) Records. (6) Release 0.75.
- **done-when:** Every step's evidence row quoted; workflow §5 closed (§8); the release checks pass after the cut, with `just visual` from step 4, or from step 6 when step 4 did not pass it; the preflight prints the command block and the post-merge block.

Candidate W1 BC ([implementation plan](../../1.0/implementation-plan.md) §4 card, level C), milestone M55, release 0.75. Written 2026-10-14 at `5a8f091`; the tree then also held another session's uncommitted edits to `docs/plans/1.0/workflow.md` and `docs/releases.md`. Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs.

## Context digest

- **Why now.** The register's next row after M54. M4's settings screen (display mode, UI and text scale, volume, one rebind; [w01](../../1.0/w01-ui-text-suite.md) §11) needs checkboxes and sliders, which the definition gate put in 1.0 as BC (criteria question 8, `D-102`; implementation plan amendment 15). M4 waits on BC, W11b (landed in 0.73) and W5a (landed in core in 0.74).
- **What it stands on.** `D-125`'s `UiTree` with its closed `WidgetKind`, which already names `Toggle` and `Slider` "at BC", and its style model (per-kind looks with state variants, text-only inheritance); `D-147`'s paint list; `D-148`'s theme files; `D-149`'s routing, whose `ui_*` actions M54 bound to the pad, and whose open limits leave "a drag past a threshold, scroll containers, popups and click-outside dismissal" to BC and M5. W1 has not graduated: [w01](../../1.0/w01-ui-text-suite.md) §8 and §11 are the scoping source ([criteria](../../1.0/criteria.md) §4).
- **Level C.** An Opus 5.5 max plan (`D-151`), the API sketch in §2.1, and a critique of step 1's API diff before the rest runs (`D-150`).
- **The card says "Decisions: None: the style model holds".** The style model holds: a control's indicator paints in its kind's text colour (§2.5). One decision is still expected (§5; this plan's Q8), since `D-149`'s direction rule, `D-147`'s paint rule and `D-125`'s relayout rule change for the new kinds.
- **What later work expects.** M4 builds the settings screen from these controls, reads `ValueChanged` and shows stored settings through the silent setters; M5's movable windows add the drag threshold; W12b with M6 move the examples onto the widgets.

## 1. Audit

Read at `5a8f091`. Paths under `crates/tungsten-core/src/ui/` are written from `ui/`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | `WidgetKind` is closed with `Panel`, `Label`, `Button` and `Image`, `#[non_exhaustive]`; its doc and `D-125` say `Toggle` and `Slider` join at BC. Labels and images are the leaves; only a `Button` starts interactive and focusable | `ui/node.rs:30-62`, `:224-240`; `DECISIONS.md:1551` |
| A2 | `Role` (`#[non_exhaustive]`) has no checkbox or slider role | `ui/node.rs:66-84` |
| A3 | Node data holds a label's text and an image's content; nothing holds a control's value | `ui/node.rs:135-214` |
| A4 | The crate's kind matches are exhaustive: `default_role`, `Theme::kind`, `text_holder`, `paint_node`. A new kind must reach each in the step that adds it | `ui/node.rs:48-55`; `ui/theme.rs:41-50`; `ui/tree.rs:426-434`; `ui/paint.rs:582-612` |
| A5 | `UiEvent` and each variant are `#[non_exhaustive]`, `Copy`; there is no value event. `activated` reads `iter_current`, so one click is seen once | `ui/event.rs:15-118` |
| A6 | `KindStyles` holds one public field per kind and is not `#[non_exhaustive]`: two new fields break a struct literal | `ui/theme.rs:201-212`; `api/tungsten-core.txt:9435-9437` |
| A7 | The built-in kind styles derive from the tokens; a button's states change its background and border | `ui/theme.rs:214-277` |
| A8 | A resolved text-style change in any property, colour included, marks the root for layout, since a label's text node must be set again (`D-117`); only a background, border or radius change marks paint alone | `ui/tree.rs:933-957` |
| A9 | The theme file names the kinds `panel`, `label`, `button` and `image` under `deny_unknown_fields`, so a file naming another kind fails | `ui/theme_file.rs:42-46`, `:84-99`, `:265-272` |
| A10 | Leaves enter Taffy by `is_leaf`; a leaf measures an image's natural size or its text node, and any other leaf measures zero | `ui/layout/taffy.rs:227-228`, `:342-364` |
| A11 | The paint walk pushes a node's box when its fill or border shows, then a label's text or an image's sprite; panels and buttons add nothing (`D-147`) | `ui/paint.rs:550-612`; `DECISIONS.md:1837` |
| A12 | The UI shader clamps a corner radius to the half size, so paint need not clamp | `crates/tungsten-render/src/ui.wgsl:83-91` |
| A13 | Routing: a move only re-hovers; a primary press captures and focuses its target; the release sends `Released`, then `Activated` over the target, then `DoubleClicked` for a second click; `ui_accept` sends `Activated` | `ui/route.rs:446-450`, `:621-690`, `:814-818` |
| A14 | A direction press moves focus by `focus_direction`, and the router's repeat moves it again after `repeat_delay` every `repeat_interval`; `D-149` states the rule | `ui/route.rs:803-813`, `:865-883`; `DECISIONS.md:1860` |
| A15 | `D-149`'s open limits leave a drag past a threshold, scroll containers, popups and click-outside dismissal to BC and M5 | `DECISIONS.md:1868` |
| A16 | The default map binds `ui_left` and `ui_right` to the arrows, the d-pad and the left stick, and `ui_accept` to Enter, Space and South | `crates/tungsten-core/src/input/action_map.rs:270-325` |
| A17 | `UiHarness` scripts the pointer and keys (`pointer_to`, `pointer_down`, `pointer_up`, `click`, `key_down`), and `Harness::press_action` presses an action's first binding, a pad's included; a drag needs no new helper | `crates/tungsten/src/testing/ui.rs:127-168`; `crates/tungsten/src/testing.rs:142-161` |
| A18 | The UI lab builds its roots for every fixture but `empty`; `ui-visual.png` and `ui-labels.png` capture the `default` and `labels` fixtures; smoke runs each example with only `TUNGSTEN_SMOKE_FRAMES`, so the lab runs its interactive build there | `examples/05_ui_lab/src/main.rs:88-117`; `examples/05_ui_lab/tests/ui_regression.rs:75-150`; `scripts/smoke-examples.sh:63-71`, `:91-97` |
| A19 | No benchmark row adds `UiPlugin` or builds a UI root, so `ui_route` and the paint and measure branches run in no tracked row | `examples/02_bench/src/main.rs:78-87` |
| A20 | The template builds no UI tree | `rg -l UiTree templates/` → none |
| A21 | `docs/LLM_INDEX.md` sits at its 12 KiB budget (12,288 B); its UI row routes `core/ui/` and `core/tests/ui/` | `docs/LLM_INDEX.md:63`; `just ctx` |
| A22 | The API snapshot lists each `UiTree` method under three paths, each `UiEvent` variant under three, the members of `WidgetKind`, `Role` and `KindStyles` under two, and the `EventQueue<UiEvent>` readers under two | `api/tungsten-core.txt:9779`, `:10955`, `:16118`; `:5132`, `:13553` |
| A23 | Known issue UI F2's remainder (a widget inside a disabled container keeps its own look) is M4's | `docs/known-issues.md:22` |

## 2. Design

### 2.1 API sketch

Every item gets rustdoc, with a `# Panics` section where it panics. Names are the step 1 critique's to question (§9, `C.<n>`).

```rust
// tungsten_core::ui::node, re-exported from tungsten_core::ui as today
#[non_exhaustive]
pub enum WidgetKind {
    Panel, Label, Button, Image,
    /// A checkbox: a focusable, interactive leaf that is on or off.
    Toggle,
    /// A horizontal slider: a focusable, interactive leaf holding a value
    /// in a `SliderRange`.
    Slider,
}

#[non_exhaustive]
pub enum Role {
    None, Group, Window, Dialog, Label, Button, Image,
    /// A control that is on or off.
    CheckBox,
    /// A control that picks a value in a range.
    Slider,
}

// tungsten_core::ui::control (new module); SliderRange re-exported from tungsten_core::ui
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SliderRange { /* min, max, step: private */ }

impl SliderRange {
    /// `min..=max`, continuous; `min == max` holds one value. Panics when a
    /// bound is not finite or `min > max`.
    #[must_use] pub fn new(min: f32, max: f32) -> Self;
    /// This range on the values `min + k·step` and `max`; a step of 0 is
    /// continuous. Panics when `step` is negative or not finite.
    #[must_use] pub fn with_step(self, step: f32) -> Self;
    #[must_use] pub fn min(self) -> f32;
    #[must_use] pub fn max(self) -> f32;
    #[must_use] pub fn step(self) -> f32;
}

impl UiTree {
    /// A checkbox under `parent`, `on` or off. Panics when `parent` is not in the tree.
    pub fn toggle(&mut self, parent: WidgetId, on: bool) -> WidgetId;
    /// A slider under `parent` at `value`, clamped and snapped into `range`;
    /// a value that is not finite starts it at `range.min()`. Panics when
    /// `parent` is not in the tree.
    pub fn slider(&mut self, parent: WidgetId, range: SliderRange, value: f32) -> WidgetId;
    /// A toggle's state; `None` for any other widget.
    #[must_use] pub fn toggled(&self, id: WidgetId) -> Option<bool>;
    /// Sets a toggle's state and returns whether it changed. Marks paint; sends no event.
    pub fn set_toggled(&mut self, id: WidgetId, on: bool) -> bool;
    /// A slider's value; `None` for any other widget.
    #[must_use] pub fn slider_value(&self, id: WidgetId) -> Option<f32>;
    /// Sets a slider's value, clamped and snapped, and returns whether it
    /// changed; a value that is not finite changes nothing. Marks paint; sends no event.
    pub fn set_slider_value(&mut self, id: WidgetId, value: f32) -> bool;
    /// A slider's range; `None` for any other widget.
    #[must_use] pub fn slider_range(&self, id: WidgetId) -> Option<SliderRange>;
    /// Replaces a slider's range and snaps its value into it. Marks paint; sends no event.
    pub fn set_slider_range(&mut self, id: WidgetId, range: SliderRange);
}

// tungsten_core::ui::event; UiValue re-exported from tungsten_core::ui
pub enum UiEvent {
    // …the variants of D-149, unchanged…
    /// A toggle or slider took a new value from the pointer or a `ui_*` action (§2.7).
    #[non_exhaustive]
    ValueChanged { widget: WidgetId, value: UiValue },
}

/// A control's value, as `ValueChanged` carries it.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UiValue {
    /// A toggle's state.
    Bool(bool),
    /// A slider's value, inside its range and on its step.
    Number(f32),
}

impl UiValue {
    #[must_use] pub const fn as_bool(self) -> Option<bool>;
    #[must_use] pub const fn as_number(self) -> Option<f32>;
}

impl EventQueue<UiEvent> {
    /// The value `widget` took last at this frame's routing; reads `iter_current`,
    /// as `activated` does.
    #[must_use] pub fn value_changed(&self, widget: WidgetId) -> Option<UiValue>;
}

// tungsten_core::ui::theme
pub struct KindStyles {
    pub panel: KindStyle, pub label: KindStyle, pub button: KindStyle, pub image: KindStyle,
    /// Toggles: the box in the background and border, the mark in the text colour.
    pub toggle: KindStyle,
    /// Sliders: the track in the background, the fill and thumb in the text
    /// colour, the thumb's border in the border colour and width.
    pub slider: KindStyle,
}
```

Nothing else public: the snapping, geometry, pointer and step helpers are `pub(crate)` in `ui/control.rs`, and each lands with its first caller outside tests, since `just lint` denies the dead-code warning over every target (`justfile:21-22`): snapping in step 1 (the builders and setters), the toggle mark and slider parts in step 2 (paint), the pointer's value and the key steps in step 3 (routing). No root re-export is added; `UiTree` and `UiEvent` keep theirs.

### 2.2 Kinds and node data

Both kinds are leaves (`is_leaf`), so `add_child` refuses children under them, as it does under a label. `Node::new` makes them interactive and focusable, as a button; the roles are `CheckBox` and `Slider`. A crate-private control field on `Node` holds a toggle's state or a slider's value and range. Neither holds text: `text` and `set_text` treat them as they treat a panel, and the accessible name is the one a game sets (`set_accessible_name`, which `UiHarness::find` reads). A checkbox's caption is a sibling `Label` in a row the game builds, the composition `D-125` and w01 §8 ask for (Q1). Hidden, collapsed and disabled controls follow `D-125` and `D-149` unchanged.

### 2.3 Values

A range's allowed values are `min + k·step` for whole `k` up to `max`, and `max` itself; a step of 0 allows every value between the bounds. `SliderRange::new(min, max)` panics on a bound that is not finite or on `min > max`; `with_step` panics on a negative or non-finite step. Snapping clamps to the bounds, then takes the nearest allowed value, the lower on a tie, so `max` stays reachable when the range is not a whole number of steps (0 to 1 by 0.3: 0.97 snaps to 1.0, 0.8 to 0.9). A step key moves to the next allowed value that way; a continuous range moves by a tenth of its span (Q3). When that value, stored as `f32`, equals the current one (a step finer than the `f32` spacing there), the key moves to the first allowed value at or past the adjacent `f32` that way, or to the end, so every press that is not at an end moves the value and repeated presses reach the end. Snapping, the fraction, the pointer's value and the key steps compute in `f64` from `min` and `k`, never accumulated, and store `f32` clamped to the bounds, so every finite range stays finite, `-f32::MAX..=f32::MAX` and a step far smaller than the span included. A singleton range (`min == max`) holds its one value and has fraction 0, so its thumb sits at the start (§2.6) and no input changes it. Setters clamp and snap, return whether the stored value changed, mark paint, and send no event (Q5). `set_slider_value` with a value that is not finite returns `false`; `slider` with one starts at `min`. `set_slider_range` snaps the value into the new range.

### 2.4 Layout

An `Auto` axis takes a natural size from the control's resolved text style, so a control sits level with a label of the same style beside it: a toggle is a square one line height on a side, a slider ten line heights wide and one tall (the built-in theme's 20 makes 20×20 and 200×20). A `Px`, `Percent` or `Fill` axis wins, as for an image. The measure closure reads the node's resolved style, which `layout` resolves before it solves. It counts no `measure_calls`, which count text measures.

### 2.5 Style

The style model holds: two more `KindStyle`s, the four state variants, text-only inheritance and the theme file's `KindFile` shape. A control's indicator, the toggle's mark and the slider's fill and thumb, paints in the kind's resolved text colour, its foreground; a leaf passes its text style to nothing, so that colour colours nothing else (Q2). The built-in looks, from the tokens:

- **Toggle:** normal: `surface` fill, `border` border 1 wide, `radii.sm` corners, text colour `accent`. Focused: `focus` border 2 wide. Disabled: text colour `disabled_text`. Hovered: fill `surface` lightened by 24. Pressed: fill `surface` darkened by 16.
- **Slider:** normal: track (fill) `border`, border `border` 0 wide, `radii.sm` corners, text colour `accent`. Focused: `focus` border 2 wide, which rings the thumb. Disabled: text colour `disabled_text`. Hovered: text colour `accent` lightened by 24. Pressed: text colour `accent` darkened by 32.

Since a control holds no text node, a resolved text change that only moves its colour marks paint, not its root's layout; a change that measures differently (the line height, which sizes it) still relayouts. The rule touches only `Toggle` and `Slider` in `resolve_subtree`, so labels, buttons and every existing look keep `D-125`'s relayout clause. The theme file accepts `toggle` and `slider` under `kinds`, with the same five looks; an unknown kind still fails.

### 2.6 Paint

All in UI units, scaled once as every primitive is (`D-147`); only `Rect` commands, so the render crate does not change (A12). For a node's rect `R`:

- **Toggle:** the generic path paints the box (`R`, its fill, border and radius). When on, a mark follows: `R` inset on every side by a quarter of its shorter side, filled with the text colour, no border, corner radius the box's minus the inset, not below 0. A 20×20 box at (0, 0) draws its mark at (5, 5), 10×10.
- **Slider:** no generic box. With `h` the height, the thumb side `t = min(h, width)`, the track thickness `h / 4` and `c` the centre line: the track from `R`'s start edge to its end edge, `h / 4` thick on `c`, in the background colour with no border and a radius of `min(corner_radius, h / 8)`; the fill from the start edge to the thumb's centre, same thickness and radius, in the text colour; the thumb, `t` square on `c`, centred at `start + t / 2 + fraction · (width − t)`, in the text colour with the resolved border colour, width and corner radius. A 200×20 slider at (0, 0) halfway draws the track (0, 7.5) 200×5, the fill (0, 7.5) 100×5 and the thumb (90, 0) 20×20.

Order inside a node: box, then mark; track, fill, thumb. A control's rectangles overlap no label beside it, so a row of labels and controls keeps its text in one batch (`D-147`'s partition rule). `show_bounds` outlines the node's rect as before. The shapes' proportions are fixed; a theme changes colours, borders and radii, and a game changes a control's size.

### 2.7 Routing

`ui_route` keeps `D-149`'s order and ownership; the kinds add:

- **Toggle:** each activation flips it, then sends `ValueChanged { value: Bool }` right after its `Activated`: a primary release over it (`Released`, `Activated`, `ValueChanged`, then `DoubleClicked` when the click is a second one) and a `ui_accept` press while it holds focus that keys act on. A double click flips it twice.
- **Slider, pointer:** a primary press on a slider target captures and focuses it as on any widget, then sets the value at the pointer: the fraction `(x − (start + t / 2)) / (width − t)` clamped to 0–1 (0 when `width ≤ t`), snapped. Each pointer move while that press holds it does the same, past either end too (clamped), whether or not the pointer is over the slider. The thumb jumps to the pointer; there is no grab offset (Q7). The release and any cancel change nothing: a press cancelled by focus loss, the pointer leaving the window or the slider's removal, hiding or disabling keeps the value it reached. Other buttons change nothing. `Activated` and `DoubleClicked` still follow a release over it (Q6).
- **Slider, keys:** while a slider holds focus that keys act on, `ui_left` and `ui_right` step its value one allowed value down or up instead of moving focus, and the router's repeat steps it again on the same schedule (Q4). At an end the press is still the UI's and changes nothing. `ui_up`, `ui_down`, `ui_next` and `ui_prev` move focus as before. `D-149`'s rule that the first press after pointer use is the game's applies unchanged: a press is judged by the focus visibility before it. The pad's d-pad and stick directions step it as the arrows do (A16).
- **Events:** `ValueChanged` follows every change of the stored value by the router, and only those, after the press's own `Pressed`, `FocusLost` and `FocusGained`. One per change: a move that snaps to the same value sends none. `value_changed(widget)` returns the last of the frame's routing.

### 2.8 Seam, template and records

Core only (`D-007`, `D-018`): plain data on the paint list, no `wgpu` or `winit` type, no new resource, system or dependency. The template builds no UI (A20), so workflow §5 line 6 is n/a; M4 brings state-owned roots. `docs/LLM_INDEX.md` routes the new files through its `core/ui/` and `core/tests/ui/` paths (A21).

### 2.9 The fixture

The UI lab's interactive run (no `TUNGSTEN_UI_LAB_FIXTURE`, A18) gains a "Settings" window built by `screens::build_controls`: a root on the `Game` layer anchored at the top-end corner, about 240 units wide, holding rows of a caption label and a control: a "Damage numbers" toggle (on) that turns the floating damage numbers on and off, a "Spawn rate" slider (1 to 8 a second by 1, at 4) that sets their interval, a "Volume" slider (0 to 100 by 5, at 80) with a readout label ("80%"), and a disabled "Locked" toggle showing the disabled look. Each control carries its caption as its accessible name. A lab system reads `value_changed` for each and writes the readouts and the damage numbers' settings. The `default`, `hidden`, `labels` and `empty` fixtures build nothing new, so `ui-visual.png` and `ui-labels.png` hold. The lab's theme file styles the two kinds to its palette. At an OS scale of 1.75 the window may overlap the inventory window's right edge in the interactive run, which no capture draws.

### 2.10 Public surface for the freeze

Two [break ledger](../../1.0/w04-api-freeze.md#break-ledger) rows, each with "W1 BC (M55 §2.10)" in its From cell, written in step 1 and given the decision in step 5:

- **Break:** `KindStyles` gains `toggle` and `slider`; a struct literal needs both or starts from `KindStyles::for_tokens(&tokens)`. The theme file accepts two more kinds (additive).
- **Additions:** `WidgetKind::Toggle` and `Slider`; `Role::CheckBox` and `Slider`; `UiEvent::ValueChanged`; `UiValue` (`#[non_exhaustive]`, `as_bool`, `as_number`); `SliderRange` (`new`, `with_step`, `min`, `max`, `step`); `UiTree::toggle`, `slider`, `toggled`, `set_toggled`, `slider_value`, `set_slider_value`, `slider_range` and `set_slider_range`; `EventQueue::<UiEvent>::value_changed`. Behaviour for the existing kinds does not change.

## 3. Steps

### Step 1: Kinds, values and the event (the API)

- **Files:** `crates/tungsten-core/src/ui/control.rs` (new), `crates/tungsten-core/src/ui/mod.rs`, `crates/tungsten-core/src/ui/node.rs`, `crates/tungsten-core/src/ui/event.rs`, `crates/tungsten-core/src/ui/tree.rs`, `crates/tungsten-core/src/ui/theme.rs`, `crates/tungsten-core/src/ui/theme_file.rs`, `crates/tungsten-core/src/ui/layout/taffy.rs`, `crates/tungsten-core/src/ui/paint.rs` (the two kinds join the arm that adds nothing, so the crate builds; step 2 paints them), `crates/tungsten-core/src/tests/ui/control.rs` (new), `crates/tungsten-core/src/tests/ui/theme_file.rs`, `crates/tungsten-core/src/tests/ui/style.rs`, `api/tungsten-core.txt` (`just api`), `docs/plans/1.0/w04-api-freeze.md` (§2.10's rows)
- **Change:** in order: the kinds, roles, leaf rule and control field (§2.2) in `node.rs`; `ui/control.rs` with `SliderRange`, the value rules the builders and setters use (§2.3: validation, `f64` snapping), the `UiTree` builders, getters and setters, under `#[path = "../tests/ui/control.rs"]` tests, and no geometry, pointer or step helper yet (§2.1); `ValueChanged`, `UiValue`, the `widget()` arm and `value_changed` in `event.rs`; `text_holder`'s arms and the colour-only rule (§2.5) in `tree.rs`; the two `KindStyles` fields, their built-in looks and `Theme::kind`'s arms in `theme.rs`; the two kinds in `theme_file.rs` and its doc's kind list; the natural sizes (§2.4) in `taffy.rs`; the module and the `SliderRange` and `UiValue` re-exports in `mod.rs`. Tests pin: the range's panics and its snapping with an inexact step, a singleton range and `-f32::MAX..=f32::MAX` (finite results); a non-finite initial value starting at `min`; the setters' clamping, snapping, return values, paint marking and silence (no event, no layout); the builders' focusable, interactive leaves, roles and refused children; the natural sizes and a `Px` axis winning; a colour-only state change repainting without a relayout and a line-height change relayouting; `value_changed` returning the current window's last value; both theme-file kinds parsing and merging, an unknown kind still failing; the built-in looks. Then `just api`, its diff reviewed, and the two ledger rows. Then the level C API critique (the skill's Running steps), each finding checked and recorded in §9 as `C.<n>`; a finding that holds changes this step, whose checks then run again.
- **Done-when:** `cargo test -p tungsten-core --locked` → exit 0, the new tests named in the evidence row; `cargo check --workspace --all-targets --locked` → exit 0; `just fmt-check` → exit 0; `just lint` → exit 0; `just api` then `just api-check` → exit 0; `git diff --quiet -- api/tungsten.txt api/tungsten-render.txt api/tungsten-kit.txt; echo $?` → `0`; `comm -23 <(git cat-file -p HEAD:api/tungsten-core.txt | sort) <(sort api/tungsten-core.txt) | wc -l` → `0` (no line removed); with §2.1's names, or those a `C.<n>` finding changed (recorded with both counts): `rg -c 'UiTree::(toggle|slider|toggled|set_toggled|slider_value|set_slider_value|slider_range|set_slider_range)\(' api/tungsten-core.txt` → 24, `rg -c 'UiEvent::ValueChanged$' api/tungsten-core.txt` → 3, `rg -c '(WidgetKind::(Toggle|Slider)|Role::(CheckBox|Slider))$' api/tungsten-core.txt` → 8, `rg -c 'KindStyles::(toggle|slider): ' api/tungsten-core.txt` → 4, `rg -c '^pub struct tungsten_core::ui::(control::)?SliderRange$' api/tungsten-core.txt` → 2, `rg -c '^#\[non_exhaustive\] pub enum tungsten_core::ui::(event::)?UiValue$' api/tungsten-core.txt` → 2, `rg -c 'EventQueue<tungsten_core::ui::event::UiEvent>::value_changed\(' api/tungsten-core.txt` → 2; `sed -n '/^## Break ledger/,/^## Context digest/p' docs/plans/1.0/w04-api-freeze.md | rg -c 'W1 BC \(M55'` → 2; the API critique recorded in §9 (rows `C.<n>`, or "No concerns", or "not run" with its error line).
- **Moves:** none.

### Step 2: Paint

- **Files:** `crates/tungsten-core/src/ui/paint.rs`, `crates/tungsten-core/src/ui/control.rs` (the geometry helpers only), `crates/tungsten-core/src/tests/ui/paint.rs`
- **Change:** the crate-private toggle-mark and slider-part geometry (§2.6) in `ui/control.rs`, and `paint_node` painting a toggle's mark after the generic box when it is on, and a slider's track, fill and thumb in place of its box. Snapshot tests (`PaintList::dump`) pin a toggle off and on; a slider at its minimum, at a middle step and at its maximum, at scale 1 and at 1.5; a singleton slider's thumb at the start; a focused slider's thumb border and a disabled toggle's and slider's colours; and a row of label, toggle, label and slider whose two text commands stay in one batch.
- **Done-when:** `cargo test -p tungsten-core --locked ui::` → exit 0, the new paint tests named in the evidence row and every existing paint test passing unchanged; `just lint` → exit 0.
- **Moves:** none.

### Step 3: Routing

- **Files:** `crates/tungsten-core/src/ui/route.rs`, `crates/tungsten-core/src/ui/control.rs` (the pointer and step helpers only), `crates/tungsten-core/src/tests/ui/route.rs`, `crates/tungsten/src/tests/ui_route.rs`
- **Change:** §2.7: the crate-private pointer value and key steps (§2.3) in `ui/control.rs`; one activation path in the router for releases and `ui_accept` that flips a toggle; the slider's value from the pointer at a primary press and at each move while captured; the direction handling, shared by the press and the repeat, that steps a slider holding focus keys act on and otherwise moves focus. Core tests (the `Rig` of `tests/ui/route.rs`) pin: a click and a `ui_accept` press flip a toggle with the event order of §2.7; a press sets a slider from the pointer, moves follow past both ends, the release changes nothing and another button changes nothing; `ui_left` and `ui_right` step a focused slider, repeat after `repeat_delay` every `repeat_interval`, stop at the ends, and reach `max` and step back to the last whole step on an inexact range, and a step finer than the `f32` spacing still moves each press (16,777,216 to 16,777,220 by 0.5 reaches `max` in two presses from `min`), while `ui_up` and `ui_down` still move focus; the first arrow after a pointer press is the game's and the next steps; a singleton slider takes a press and arrows and changes nothing; a disabled or hidden control takes nothing; a press cancelled by focus loss keeps the value; a move that snaps to the same value sends nothing. Harness tests (`tests/ui_route.rs`, the app's default plugins) pin: an `Update` system sees each change once through `value_changed` for a click, a drag across two frames and an arrow press made while focus shows (a key press first, since the first key after pointer use is the game's); a key rebound to `ui_right` with `ActionMap::replace_bindings` and the pad's d-pad step a slider holding visible focus.
- **Done-when:** `cargo test -p tungsten-core --locked ui::` → exit 0, the new route tests named in the evidence row and every existing route test passing unchanged; `cargo test -p tungsten --locked ui_route` → exit 0, the new harness tests named; `just lint` → exit 0.
- **Moves:** none.

### Step 4: The UI lab's controls window

- **Files:** `examples/05_ui_lab/src/screens.rs`, `examples/05_ui_lab/src/main.rs`, `examples/05_ui_lab/assets/ui/themes/ex05_default.json`, `examples/05_ui_lab/tests/screens.rs`
- **Change:** §2.9: `screens::build_controls` and its resource, the lab system that applies `value_changed`, the damage numbers reading their switch and interval from it, the interactive run building the window and registering the system, the theme file's `toggle` and `slider` looks, and the module doc. One example test (`tests/screens.rs`): a click on the "Volume" slider's start reads "0%"; the first right arrow after it is the game's and leaves "0%" (`D-149`'s rule, `crates/tungsten-core/src/tests/ui/route.rs:436-445`); after its release a second press reads "5%"; a click on the toggle turns the damage numbers off.
- **Done-when:** `cargo test -p example-05-ui-lab --locked` → exit 0, the new test passing; `cargo test -p tungsten-core --test manifests --locked` → exit 0; `just smoke` → exit 0; `just visual` → exit 0 with four `test result: ok` lines (`visual_regression`, `post_regression`, `transition_regression`, `ui_regression`; the one `load_shaders 'fade'` error line is example 04's deliberate case); `git status --short -- '*.png'` → empty.
- **Moves:** none.

### Step 5: Records

- **Files:** `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md` (the M55 section and the status line's capability; the cut moves its version), `docs/plans/1.0/w01-ui-text-suite.md` (BC's ladder row), `docs/plans/1.0/backlog-1.x.md`, `docs/plans/1.0/w04-api-freeze.md` (the two rows cite the decision), `crates/tungsten-core/src/ui/control.rs` and `crates/tungsten-core/src/ui/event.rs` (doc comments cite the decision), `docs/known-issues.md` (only for out-of-scope findings), `api/*.txt` (only if `just api` moves them)
- **Change:** first `git status --short` and the next free ID (162 when this plan was written, rechecked now; the records use the ID it gets). Write the decision of §5 with the [tungsten-decision](../../../../.claude/skills/tungsten-decision/SKILL.md) skill: its entry, its index row, the marker lines on `D-125`, `D-147` and `D-149` and their index rows' amended notes. Then the docs that cite it: a `### Broader Controls — M55` section after `### World Labels and Gamepad — M54` in `DESIGN.md`, and "checkboxes and sliders" added to the status line's capability list; "**Landed in 0.75 (M55, …)**" on BC's row in w01 §11; a backlog row beginning "Broader controls beyond checkboxes and sliders" (progress bars, collapsible sections, tooltips, popups with click-outside dismissal, window resizing; deferred by the definition gate's question 8 and w01 §8; back when M5 or a game needs one; Q10); the ledger rows' decision; the module and `ValueChanged` docs. `just api` if any step after 1 moved the surface.
- **Done-when:** `rg -c '^## D-16[2] — ' DECISIONS.md` → 1; `rg -c '^\*\*Amended by D-16[2]:\*\*' DECISIONS.md` → 3; `rg -c 'D-16[2]' docs/DECISION_INDEX.md` → 4 (its row and the three amended rows); `rg -c '^### Broader Controls — M55$' DESIGN.md` → 1; `rg -c 'checkboxes and sliders' DESIGN.md` → at least 1; `rg -c 'Landed in 0\.75 \(M55' docs/plans/1.0/w01-ui-text-suite.md` → 1; `rg -c '^\| Broader controls beyond checkboxes and sliders' docs/plans/1.0/backlog-1.x.md` → 1; `just api-check` → exit 0; `just fmt-check` → exit 0; `just ctx` → OK; `just repo-check` → exit 0. With another ID, the patterns take its last digit in the brackets.
- **Moves:** none.

### Step 6: Release 0.75

- **Files:** `CHANGELOG.md`, `DESIGN.md` (step 5 wrote its section; the cut moves the status line's version), `Cargo.toml`, `Cargo.lock`, the register row, the README's four owned Now fields (`roadmap.py summary`), this plan (moved to `docs/plans/archive/1.0/`). The release commit also takes every other session's edits in the tree (workflow §6.1), the `workflow.md` and `releases.md` edits of plan time among them; none is reverted, and the report names them.
- **Change:** only when steps 1–3 landed (§7). Close workflow §5 (§8) and this step's evidence row with `status: done`; then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.75.0` once, every release check once after it, with `just visual` (exit 0, four `test result: ok` lines) when step 4 did not pass it, then the preflight with `--message`; a failed check hands over no command block, and the report names it. Before the cut, `git status --short` and `rg -n '^## D-1' DECISIONS.md | tail -3` again, for another session's decision or files.
- **Done-when:** [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut); `just release-preflight 0.75.0 --repo JosephGibson/Tungsten --message 'Update 0.75: M55 checkboxes and sliders'` → passes and prints the command block and the post-merge block
- **Ends with:** the changed-file list, the command block, the post-merge block, then the plan prompt for the next register row, W1 M4, labelled from its stop's `plan` stage in the catalog.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Should a checkbox carry its caption as a `Label` child, as a button does? | No: both kinds are leaves and the caption is a sibling label in the game's row. A child label would inherit the toggle's text colour, the mark's (§2.5), and a click on the caption toggling its control is a follow-up for M4 |
| Q2 | Where do the mark, fill and thumb take their colour? | The kind's resolved text colour, so no style property or state is added and the card's "the style model holds" stands; a theme sets `kinds.slider.normal.text.color` |
| Q3 | How far does a key move a continuous slider (step 0)? | A tenth of its span, the common default; a game wanting finer keys sets a step |
| Q4 | Do `ui_left` and `ui_right` leave a slider at its end? | No: a slider holding focus takes both; `ui_up`, `ui_down` and Tab leave it, which suits the settings screen's vertical list |
| Q5 | Do the setters send `ValueChanged`? | No: only the router does, so a game shows stored settings without echoing them back |
| Q6 | Do a toggle and a slider still get `Activated` and `DoubleClicked`? | Yes: `D-149`'s pointer contract holds for every interactive widget; `ValueChanged` carries the value |
| Q7 | Does a press on the thumb keep its grab offset? | No: the thumb centres on the pointer at the press and follows it, snapped |
| Q8 | The card names no decision; is one written? | Yes, one (§5): `D-149`'s direction rule, `D-147`'s paint rule and `D-125`'s relayout rule change for the new kinds, and decisions change only through an entry |
| Q9 | Does the fixture get a pixel capture? | No: the card's checks are harness tests, paint-list snapshots and smoke; `just visual` (step 4, or step 6) shows the existing captures unchanged |
| Q10 | Does the rest of w01 §8's Broader controls get a backlog row? | Yes (step 5): no row holds progress bars, collapsible sections, tooltips, popups or window resizing, and `D-149` left popups and click-outside dismissal to BC |
| Q11 | Does the controls window join the captured fixtures? | No: only the interactive run builds it, so `ui-visual.png` and `ui-labels.png` hold |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`).

## 5. Decisions expected

162: W1 BC landed (M55): checkboxes and horizontal sliders are the closed leaf kinds `Toggle` and `Slider` with the roles `CheckBox` and `Slider`, natural sizes from the line height, and values the router changes (an activation flips a toggle; a primary press and each move while it holds a slider set it from the pointer; `ui_left` and `ui_right` step a slider holding focus keys act on, with the navigation repeat) and reports as `UiEvent::ValueChanged` with a `UiValue`, while setters send nothing; the mark, fill and thumb paint in the kind's text colour, whose colour-only change repaints a control without relayout. Amends `D-125` (the relayout clause, for the two kinds), `D-147` (the paint clause: a slider paints its parts in place of its box) and `D-149` (the direction clause); extends `D-040` (`value_changed`) and `D-148` (two theme kinds). Its Consequences settle `D-149`'s open limits: the drag threshold stays with M5's movable windows; popups and click-outside dismissal go to the backlog row. Next free ID when written; checked again in step 5.

## 6. Invariants

- **May move:** none.
- **Must not move:** the determinism hash, the pinned containment hash and the row digests (no physics or benchmark code changes, so no step runs `just physics-release` or a capture); `gpu-visual.png`, the post and transition regressions, `ui-visual.png`, `ui-labels.png` and the lab's direct-and-capture and hidden-equals-empty comparisons (`just visual`, in step 4 or step 6); `api/tungsten.txt`, `api/tungsten-render.txt` and `api/tungsten-kit.txt`; every existing core UI test, umbrella UI test and lab test, which pin the existing kinds' looks, paint snapshots and event order.

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with steps that don't depend on it. Steps 2 and 3 need step 1; step 4 needs steps 1–3; step 5 records what landed.
- The release runs only when steps 1–3 landed: without paint or routing the controls draw nothing or take no input, the card's behaviours and paint commands are missing, and M4 needs both. Otherwise the run ends before the release, which a release session takes once the owner has seen why (workflow §3). A step 4 skipped for a reason other than `just visual` does not hold the release: the fixture becomes a follow-up for M4's fixture, as M52 released without its example step, and step 6 runs `just visual` itself, since the release chain has no visual check and step 2 changed the paint walk (workflow §5 line 2).
- A count check that the correct change cannot meet, such as an API name a `C.<n>` finding changed or a ledger row the decision skill shaped otherwise: keep the change, record both counts and why in the evidence row; it is not a failure.
- `just visual` fails on any capture: skip step 4 as above and end the run before the release, recording which capture moved (workflow §3); a release session follows once the owner has seen why.
- A `C.<n>` finding or a step would add a style property or a state variant: take Q2's default, record the finding as an open question for M4, and change nothing in the style model.
- A step needs a file it does not list: a test module under `src/tests/` or the `api/` snapshot that `just api` writes may change, recorded in the evidence row; any other file skips the step and becomes a follow-up.
- Another session's files or decision appear in the tree: never revert them; take the next free ID and name them in the release report (workflow §6).

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1–4, release | Crate tests and `just lint` in 1–3; the example's tests, layer 1, `just smoke` and `just visual` in 4; the release runs `just check`, `just ctx`, `just repo-check`, `just api-check`, `just script-test`, `just smoke`, `just notices` and `just template-check` once, after the cut, and `just visual` when step 4 did not pass it. Perf: no tracked row runs the changed code (A19), so no capture is owed |
| 3 Invariants | 4, release | §6; `just visual` in step 4, or in step 6 when step 4 did not pass it |
| 4 Decisions | 5 | 162, with its index row and marker lines, before the docs that cite it |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1, 5 | §2.10's two rows in step 1, given the decision in step 5; rustdoc on every new item; `just api` in step 1 and again in step 5 if the surface moved; the release's `just api-check` |
| 6 Template and guide | n/a | The template builds no UI (A20); the getting-started guide waits for W9b |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 5 | No row: `core/ui/` and `core/tests/ui/` route the new files, and the index is at its budget (A21); `just ctx` in step 5 |
| 8 Design | 5 | The M55 section of `DESIGN.md` and the status line's capability |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 5, release | Known issues only for findings; the backlog row (Q10); the gap log has no rows; w01's ladder row; the register row's release and status at the release, whose `just release-cut` refreshes the owned fields (`python3 -B scripts/roadmap.py summary` supplies them) |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

Round 1: the `critique` skill with `--repo`, 2026-10-14, at `5a8f091` with this plan untracked, §9 and the evidence log cut by `--from sed`. Model `gpt-6-astra`, effort max (level C's first round), context 872,000, output cap 4,000; 397.0 s, 664,419 tokens in (578,176 cached), 11,555 out (7,712 reasoning), 32 commands, 352.1 KB output (largest 36.8 KB); 5-hour window 19% → 39%, week 14% → 17%. Five findings, each checked before any change.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] The release can proceed without working paint or routing: §7 let it run once step 1 landed, while the card asks for behaviours, paint commands and the fixture | The card (`implementation-plan.md:172`); §7 as written; M52's row, released without its example step (`implementation-plan.md:313`) | Accepted | §7 and step 6: the release needs steps 1–3; a step 4 skipped for a reason other than `just visual` does not hold it |
| 2 | [MEDIUM] Step 1's crate-private step and geometry helpers have no caller outside tests until steps 2 and 3, so the dead-code warning fails `just lint` | `justfile:21-22` (`cargo clippy --workspace --all-targets --locked -- -D warnings`) | Accepted | §2.1, the header and steps 1–3: each helper lands with its first caller, snapping in 1, the geometry in 2, the pointer value and key steps in 3 |
| 3 | [MEDIUM] `UiTree::slider` had no rule for a value that is not finite | §2.1 and §2.3 as written: only `set_slider_value` had one | Accepted | §2.1, §2.3: such a value starts the slider at `min`; step 1 tests it through the builder |
| 4 | [MEDIUM] `min == max` makes the fraction `0 / 0`, and `-f32::MAX..=f32::MAX` overflows an `f32` span | §2.3 and §2.6 as written | Accepted | §2.1, §2.3: a singleton range has fraction 0; snapping, the fraction, the pointer value and the key steps compute in `f64` and store clamped `f32`; tests in steps 1–3 |
| 5 | [MEDIUM] Step 4's test expected the first arrow after a click to step the slider, against `D-149`'s rule that §2.7 keeps | `crates/tungsten-core/src/ui/route.rs:785-787`; `crates/tungsten-core/src/tests/ui/route.rs:436-445` | Accepted | Step 4's test: the first arrow leaves "0%", a second press reads "5%"; step 3's harness arrow press shows focus with a key first |

Round 2, since round 1 changed step 4's test expectation and the release's precondition: the same reviewer, 2026-10-14, at `5a8f091` with the revised plan, §9 and the evidence log cut, and a `--focus-file` naming round 1's changed sections (none rejected). Effort xhigh, context 872,000, output cap 4,000; 176.4 s, 537,204 tokens in (468,480 cached), 5,272 out (3,073 reasoning), 16 commands, 213.4 KB output (largest 34.1 KB); 5-hour window 39% → 53%, week 17% → 20%. Two findings, both applied; no third round runs, and none stays open.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] A step finer than the `f32` spacing leaves a key stuck: 0 to 1 by 2^-26 from 0.5 computes 0.5 + 2^-26, which stores as 0.5 again | §2.3 as revised; `f32` has a spacing of 2^-24 on [0.5, 1), so the nearest `f32` to 0.5 + 2^-26 is 0.5 | Accepted | §2.3: such a press moves to the first allowed value at or past the adjacent `f32`, or the end; step 3 tests 16,777,216 to 16,777,220 by 0.5 (two presses from `min` reach `max`) |
| 2.2 | [MEDIUM] A step 4 skipped before `just visual` runs lets the release ship step 2's paint change without the visual check, which the release chain lacks | `docs/releases.md:16-19`; workflow §5 line 2 (`docs/plans/1.0/workflow.md:83`); §7 as revised | Accepted | §7, step 6, the header's done-when and §8: step 6 runs `just visual` when step 4 did not pass it |

Step 1's API critique (level C, in place of an owner review): `critique --raw --repo --force` on the `api/` and `templates/` diff with untracked files, 2026-10-14, `gpt-6-astra` at xhigh, context 872,000, output cap 4,000; 120.0 s, 263,761 tokens in (208,768 cached), 3,516 out (1,822 reasoning), 12 commands, 162.5 KB output (largest 36.7 KB); 5-hour window 53% → 62%, week 20% → 21%. One finding, checked and applied; step 1's checks ran again.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| C.1 | [MEDIUM] A valid stepped range can make `max` unreachable: with `-2^53..=1` by 1, `snap(1.0)` stores 0, since `f64` rounds both step indices to `2^53` | `crates/tungsten-core/src/ui/control.rs` `SliderRange::snap` as written; `(1 − (−2^53)) / 1 = 2^53 + 1` is not an `f64`, so `lower` and `upper` both come out 0 | Accepted | `snap` takes `max` as a third candidate, so `snap(max)` is `max` and a value nearer `max` than the computed grid points goes there; `max_stays_reachable_when_the_step_index_loses_precision` pins the snapped values (1 → 1, 0.6 → 1, 0.4 → 0, −0.6 → −1, `min` → `min`). Step 3's pointer value interpolates with exact ends and its key steps fall back to the end, for the same reason. No API name changed |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-14 | Pass | `cargo test -p tungsten-core --locked` → exit 0, 993 passed, 0 failed (8 ignored), 24 new: `ui::control::tests::` (21: the four range panics, bounds and step, snapping with a 0.3 step, a singleton, `-f32::MAX..=f32::MAX`, `max` past 2^53 steps, builders' leaves and roles, caption as name, both refused children, clamped and snapped builds, a non-finite start at `min`, `set_toggled`, `set_slider_value`, `set_slider_range`, natural sizes, a `Px` axis winning, `value_changed`), `ui::style::tests::toggles_and_sliders_have_built_in_looks_from_the_tokens`, `ui::style::tests::a_controls_colour_only_change_repaints_and_a_line_height_change_relayouts`, `ui::theme_file::tests::toggle_and_slider_looks_parse_and_merge_over_the_built_in_ones`; the existing unknown-kind test names `progress` where it named `slider`, now a kind. `cargo check --workspace --all-targets --locked` → 0; `just fmt-check` → 0; `just lint` → 0; `just api` then `just api-check` → 0; the other three snapshots unchanged (`git diff --quiet` → 0); lines removed from `api/tungsten-core.txt` → 0 (137 added); counts 24, 3, 8, 4, 2, 2, 2 as §2.1's names give; ledger rows `W1 BC (M55` → 2. API critique: one finding, C.1, applied (§9) | `crates/tungsten-core/src/ui/{control.rs,mod.rs,node.rs,event.rs,tree.rs,theme.rs,theme_file.rs,layout/taffy.rs,paint.rs}`, `crates/tungsten-core/src/tests/ui/{control.rs,style.rs,theme_file.rs}`, `api/tungsten-core.txt`, `docs/plans/1.0/w04-api-freeze.md` |
| 2 | 2026-10-14 | Pass | `cargo test -p tungsten-core --locked ui::` → exit 0, 162 passed, 0 failed; `ui::paint::tests::` 23 passed, the 18 existing unchanged and 5 new: `a_toggle_paints_its_box_and_its_mark_only_when_on` (off, on, and a 40×24 box with radius 8 at scale 1.5), `a_slider_paints_its_track_fill_and_thumb_in_place_of_its_box` (§2.6's 200×20 example; 200×24 at 0, 5 and 10 at scales 1 and 1.5), `a_singleton_sliders_thumb_sits_at_the_start`, `focus_rings_the_thumb_and_disabled_controls_take_the_disabled_colour`, `a_row_of_labels_and_controls_keeps_its_text_in_one_batch` (one batch, `0..4`); `just lint` → 0 (after one `single_range_in_vec_init` fix in the new row test); `just fmt-check` → 0 | `crates/tungsten-core/src/ui/{paint.rs,control.rs}`, `crates/tungsten-core/src/tests/ui/paint.rs` |
| 3 | 2026-10-14 | Pass | `cargo test -p tungsten-core --locked ui::` → exit 0, 171 passed, 0 failed; `ui::route::tests::` 35 passed, the 26 existing unchanged and 9 new: `a_click_and_an_accept_press_flip_a_toggle_after_its_activation` (§2.7's order; a double click flips back), `a_press_sets_a_slider_from_the_pointer_and_moves_follow_past_both_ends` (press, off-slider moves past both ends, a same-value move silent, release and right button change nothing), `arrows_step_a_focused_slider_with_the_repeat_and_stop_at_the_ends` (0 to 1 by 0.3: changes at frames 16, 20 and 23, `max` reached, back to 0.9, up and down move focus), `a_step_finer_than_the_f32_spacing_still_moves_each_press` (16,777,216 → 16,777,218 → 16,777,220; a continuous range moves 0.2 of 2), `the_first_arrow_after_a_pointer_press_is_the_games_and_the_next_steps`, `a_singleton_slider_takes_a_press_and_arrows_and_changes_nothing`, `a_disabled_or_hidden_control_takes_nothing`, `a_press_cancelled_by_focus_loss_keeps_the_value`, `the_setters_route_no_value_change`; `cargo test -p tungsten --locked ui_route` → exit 0, 9 passed, 2 new: `an_update_system_sees_each_value_change_once` (click, two-frame drag, arrow after a game key, toggle click: 2, 7, 9, 10, `true`), `a_rebound_key_and_the_dpad_step_a_slider_holding_visible_focus`; `just lint` → 0; `just fmt-check` → 0. One core sub-case dropped while writing: two cursor moves in one frame coalesce in `InputState`, so it tested the input state, not the router | `crates/tungsten-core/src/ui/{route.rs,control.rs}`, `crates/tungsten-core/src/tests/ui/route.rs`, `crates/tungsten/src/tests/ui_route.rs` |
| 4 | 2026-10-14 | Pass | `cargo test -p example-05-ui-lab --locked` → exit 0, `tests/screens.rs` 3 passed, new: `the_settings_volume_and_damage_switch_follow_the_pointer_and_keys` (a click at the "Volume" slider's start reads "0%", the first right arrow leaves "0%", the second reads "5%", a click on the toggle turns the damage numbers off); `cargo test -p tungsten-core --test manifests --locked` → 0; `just smoke` → 0 (`example-05-ui-lab ... OK`, the interactive build with the window); `just visual` → 0 with four `test result: ok` lines (`visual_regression` 4, `post_regression` 5, `transition_regression` 1, `ui_regression` 6 passed) and the one `load_shaders 'fade'` error line; `git status --short -- '*.png'` → empty (0 lines); `just lint` → 0 after one `manual_midpoint` fix in the new test; `just fmt-check` → 0. The window takes input once the modal popup is dismissed, as the lab's popup already blocks the inventory window | `examples/05_ui_lab/src/{screens.rs,main.rs}`, `examples/05_ui_lab/assets/ui/themes/ex05_default.json`, `examples/05_ui_lab/tests/screens.rs` |
| 5 | 2026-10-14 | Pass | Next free ID rechecked: 162; no other session's decision or new file in the tree (`git status --short` holds this milestone's files and the plan-time `workflow.md`, `releases.md`, README and register edits). `rg -c '^## D-16[2] — ' DECISIONS.md` → 1; `rg -c '^\*\*Amended by D-16[2]:\*\*' DECISIONS.md` → 3 (`D-125`, `D-147`, `D-149`); `rg -c 'D-16[2]' docs/DECISION_INDEX.md` → 4; `rg -c '^### Broader Controls — M55$' DESIGN.md` → 1; `rg -c 'checkboxes and sliders' DESIGN.md` → 1; `rg -c 'Landed in 0\.75 \(M55' docs/plans/1.0/w01-ui-text-suite.md` → 1; `rg -c '^\| Broader controls beyond checkboxes and sliders' docs/plans/1.0/backlog-1.x.md` → 1; `just api` moved nothing after step 1 (137 added, 0 removed against `HEAD`); `just api-check` → 0; `just fmt-check` → 0; `just ctx` → "Agent context checks: OK"; `just repo-check` → 0. No known issue added: no out-of-scope finding | `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/plans/1.0/{w01-ui-text-suite.md,backlog-1.x.md,w04-api-freeze.md}`, `crates/tungsten-core/src/ui/{control.rs,event.rs}` |
| 6 | 2026-10-14 | Pass (closed; the release checks run after the cut, in the report) | Steps 1–3 landed, so the release runs (§7). Workflow §5 closed (§8): invariants (§6) held, no physics, benchmark or determinism code changed, `just visual` passed in step 4 with no PNG changed, `api/tungsten.txt`, `api/tungsten-render.txt` and `api/tungsten-kit.txt` unchanged, every existing core, umbrella and lab test passing; `DESIGN.md` updated (step 5); the register row at 0.75 with its status; one `CHANGELOG.md` `[Unreleased]` entry; the README's owned Now fields from `just release-cut`. Before the cut: `git status --short` shows this milestone's files and the plan-time `docs/plans/1.0/workflow.md` and `docs/releases.md` edits, which the release commit takes; the newest decision is `D-162`, this plan's. This plan archived before the cut | `CHANGELOG.md`, `docs/plans/1.0/implementation-plan.md`, `docs/plans/archive/1.0/phase6-milestone-55-broader-controls.md` |

## Follow-ups

- A caption label that activates its control (HTML's `label for`), and the whole settings row as the focus target: M4's settings screen decides; additive.
- The wheel, Home, End, PageUp and PageDown on a slider, and a stick's analog reach as a slider's speed: M4 if its settings screen needs them, else the backlog row of step 5.

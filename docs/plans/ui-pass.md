# The UI pass: the template's widgets and screens in the engine (ui-pass)

- **status:** in progress
- **goal:** Redo 7.17 (phase 7's Step 6; the redo's 7.17 row and §5.9): the template's widgets become the engine's. Core takes the tree-only builders (the screen card, named text styles, choice, slider and toggle rows, the tab rail, `Menu`, layout-only groups, a meter and a radial fill), styles edited in place, a key per widget carried in `UiEvent`, named text styles, sizes and sounds in the theme file, text outlines and shadows, shrinking text and the commit-width fix; the umbrella takes a modal screen helper, confirm, toasts, a reset transition, held clocks and state-owned resources; `UiHarness` takes both games' queries and `assert_fits`; the shell, 01's copy and the fixtures delete their copies; agents get a tree dump, a screen rendered by name and `tungsten new screen`; an idle screen lays out no root, guarded. It lands Step 6's rows but S-162 and S-149, which move (§2.1).
- **non-goals:**
  - 7.20's screen files, bindings and schema; 8.1's tooltips, popups, text input and drag and drop; localisation and accessibility (§5.9's Waits).
  - Grid layout (S-162, to 8.1) and the shell's assets shipped with the engine with the kit's hit-flash material (S-149, to 7.11) (Q2, Q3).
  - 01's own sites of the rows (7.13): the spell bar's snapping, short labels, radial sweep and world-anchored book, the banner's fades, the cursor, routed actions, the clock hold and run resources (Q4). 01 changes here only through its shell copy, its theme, its `tungsten.json` and the mechanical edits §2.12 names.
  - The template's boot splash and its sample's other sites (7.11); budgets (9.1); a sitting (`D-226`: this plan claims no speed-up and changes no row's workload).
- **files to touch:**
  - Step 1: `crates/tungsten-core/src/ui/{tree.rs,node.rs,id.rs,event.rs,route.rs,paint.rs,mod.rs}`, `crates/tungsten-core/src/tests/ui/{tree.rs,route.rs,paint.rs,style.rs,control.rs,window.rs}` (the last two build the events that gain `key`), `api/`.
  - Step 2: `crates/tungsten-core/src/ui/{theme.rs,theme_file.rs,tree.rs,node.rs,mod.rs}`, `crates/tungsten-core/src/tests/ui/{theme_file.rs,style.rs,tree.rs}`, `docs/assets.md`, `api/`.
  - Step 3: `crates/tungsten-core/src/ui/{theme.rs,theme_file.rs,paint.rs}`, `crates/tungsten-core/src/text.rs`, `crates/tungsten-core/src/tests/{text.rs,ui/theme_file.rs,ui/paint.rs}`, `crates/tungsten-render/src/text.rs`, `crates/tungsten-render/src/text/{engine.rs,gpu.rs,nodes.rs}`, `crates/tungsten-render/src/tests/` (its text tests), `crates/tungsten/src/{extract.rs,debug_views.rs,app.rs}`, `crates/tungsten/src/tests/{extract.rs,debug_views.rs}`, `examples/03_fixtures/src/seams.rs`, `examples/03_fixtures/tests/integration/seams_regression.rs`, `docs/assets.md`, `api/`.
  - Step 4: `crates/tungsten-core/src/ui/{theme.rs,theme_file.rs,layout/taffy.rs,measure.rs}`, `crates/tungsten-core/src/tests/ui/{layout.rs,measure.rs,theme_file.rs}`, `crates/tungsten/src/debug_views.rs`, `crates/tungsten/src/tests/debug_views.rs` (their full `TextOverrides` literals, A25), `docs/known-issues.md`, `docs/assets.md`, `api/`.
  - Step 5: `crates/tungsten-core/src/ui/widgets.rs` (new), `crates/tungsten-core/src/ui/{mod.rs,tree.rs}`, `crates/tungsten-core/src/tests/ui/widgets.rs` (new), `api/`.
  - Step 6: `crates/tungsten-core/src/ui/{route.rs,event.rs,tree.rs,prompt.rs,mod.rs}`, `crates/tungsten-core/src/{input.rs,input/action_map.rs}`, `crates/tungsten-core/src/tests/ui/{route.rs,prompt.rs}`, `crates/tungsten-core/src/tests/input/` (its action-map tests), `crates/tungsten/src/plugins.rs`, `crates/tungsten/src/tests/{ui_route.rs,plugins.rs}`, `templates/basic/tests/game/sample.rs` (the schedule snapshot), `api/`.
  - Step 7: `crates/tungsten-core/src/ui/{anim.rs,paint.rs,tree.rs,node.rs,mod.rs}`, `crates/tungsten-core/src/tests/ui/{anim.rs,paint.rs}`, `crates/tungsten/src/{camera.rs,plugins.rs,app.rs}`, `crates/tungsten/src/tests/{camera.rs,app.rs}`, `templates/basic/tests/game/sample.rs`, `api/`.
  - Step 8: `crates/tungsten/src/{state.rs,lib.rs}`, `crates/tungsten/src/tests/state.rs`, `crates/tungsten-core/src/time.rs`, `crates/tungsten-core/src/tests/time.rs`, `api/`.
  - Step 9: `crates/tungsten/src/screens/{mod.rs,confirm.rs,toast.rs}` (new), `crates/tungsten/src/lib.rs`, `crates/tungsten/src/tests/screens.rs` (new), `docs/LLM_INDEX.md`, `api/`.
  - Step 10: `crates/tungsten-core/src/config.rs`, `crates/tungsten-core/src/tests/config.rs`, `crates/tungsten/src/{app.rs,settings.rs}`, `crates/tungsten/src/tests/{app.rs,settings.rs}`, `api/`.
  - Step 11: `crates/tungsten/src/testing/ui.rs`, `crates/tungsten/src/tests/{testing_ui.rs,ui_route.rs,screens.rs}`, `crates/tungsten-core/src/ui/{tree.rs,layout.rs}`, `crates/tungsten-core/src/tests/ui/tree.rs`, `docs/perf/benchmarks.md`, `api/`.
  - Step 12: `templates/basic/src/shell/` (every file; `widgets.rs` deleted), `templates/basic/src/sample/{hud.rs,mod.rs}`, `templates/basic/assets/ui/themes/shell.json`, `templates/basic/tests/game/{main.rs,fit.rs,settings.rs,shell.rs,saves.rs,keys.rs,sample.rs}`, `templates/basic/AGENTS.md`, `examples/01_platformer/src/shell/` (the copy), `examples/01_platformer/assets/ui/themes/shell.json` (generated), `examples/01_platformer/src/features/spells/hud.rs`, `examples/01_platformer/src/features/run/banner.rs`, `examples/01_platformer/tests/game/{tomes.rs,hud.rs,gun.rs}`.
  - Step 13: `templates/basic/src/shell/` (`confirm.rs` and `toast.rs` deleted), `templates/basic/src/sample/mod.rs`, `templates/basic/tungsten.json`, `templates/basic/assets/ui/themes/shell.json`, `templates/basic/tests/game/{main.rs,shell.rs,settings.rs,keys.rs,saves.rs,sample.rs,fit.rs}`, `templates/basic/AGENTS.md`, `examples/01_platformer/src/shell/`, `examples/01_platformer/tungsten.json`, `examples/01_platformer/assets/ui/themes/shell.json` (generated).
  - Step 14: `templates/basic/tests/game/{main.rs,fit.rs,shell.rs,saves.rs,settings.rs,keys.rs,sample.rs}`, `examples/01_platformer/tests/game/{main.rs,presentation.rs}` and the 01 tests that call the removed helpers (`rg -l` in §2.14), `examples/03_fixtures/src/ui/{widgets.rs,menus.rs,controls.rs,budget.rs,hud.rs,labels.rs,fixtures.rs}`, `examples/03_fixtures/src/ui/inventory/{mod.rs,grid.rs,detail.rs,items.rs}`.
  - Step 15: `crates/tungsten-core/src/script.rs`, `crates/tungsten-core/src/tests/script.rs`, `crates/tungsten/src/{script.rs,describe.rs}`, `crates/tungsten/src/tests/{script.rs,describe.rs}`, `tools/cli/src/{args.rs,main.rs,render.rs,describe.rs,new.rs}`, `tools/cli/tests/cli.rs`, `templates/basic/src/shell/mod.rs`, `examples/01_platformer/src/shell/mod.rs`, `docs/getting-started.md`, `api/`; `templates/basic/src/{lib.rs,game.rs}` and `templates/basic/src/screens/`, touched by the compile check and restored.
  - Step 16: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/getting-started.md`, `docs/known-issues.md`, `ROADMAP.md`.
  - Step 17: this plan (deleted), `ROADMAP.md`, `docs/plans/phase-7.md`, the follow-ups' homes.
- **ordered steps:** (1) Styles in place, plain text, groups and keys. (2) Named text styles, sizes, sounds and token colours in the theme file. (3) Text outlines and shadows. (4) Shrinking text and the commit width. (5) The tree-only builders. (6) The router: commits, timeouts, back buttons, sounds, routed actions, short labels. (7) Tracks on the game clock, pixel snapping, the pointer sprite, world anchors. (8) The state stack: a reset transition, held clocks, owned resources. (9) Screens in the umbrella. (10) The UI's configuration. (11) The harness and the idle-screen guard. (12) The shell on core's widgets, styles and keys. (13) The shell on the umbrella's screens. (14) Both games' test helpers and the fixtures. (15) The agent surface. (16) The decision and the records. (17) Close.
- **done-when:** every step's done-when passes, quoted in the evidence log; `just check` passes; the physics hashes, the kit's pin, every guard pin and every golden unchanged (§5); 01's tests pass with only their helper calls changed; the template and 01 pass their `tungsten run --script` demos and `tungsten render --screen` writes the template's settings screen; the shell's `tokei` count measured against `D-199`'s 2,900 and no shell handler finding a widget by position or caption; the idle-screen guard passes; the 32 rows `landed` and S-162 and S-149 moved in phase 7's §2.2–§2.3, Step 6's evidence row written; the plan deleted with its citations and `ui-pass` out of Now.

Item `ui-pass` in [ROADMAP.md](../../ROADMAP.md), from [phase 7](phase-7.md)'s Step 6 and §2.8 and the [roadmap redo](roadmap-redo.md)'s 7.17 row and §5.9, with the [seam register](seam-register.md)'s rows. Written 2026-11-02 at `7cd69a4a`. Runs under the [tungsten-plan skill](../../.claude/skills/tungsten-plan/SKILL.md) and the [operating rules](../operating-rules.md), and differs in one way: it closes Step 6, so its close writes Step 6's evidence row and its rows' cells.

## Context digest

- 7.9 closed on 2026-11-01 (`D-230`, `D-231`); 7.17 is next on the one track (phase 7's §2.8: 7.9 → 7.17 → 7.20 → 7.11). Step 6 holds 33 S-rows from 7.5's triage and P7-14, the text outline boot-and-seams' step 12 moved here.
- The retained tree (`D-125`) holds up; the cost sits above it. The shell is 2,786 of `D-199`'s 2,900 `tokei` lines and `src/` 3,355 of 3,400 (A1). Its 339-line `widgets.rs`, its 22 layout constants, its confirm, toasts and screen-change machine are built over the tree by hand, copied byte for byte into 01 (`D-197`) and rewritten by the fixtures; its handlers find widgets by list position and caption; both games' tests walk the tree by hand (A2–A12).
- It stands on `D-124` (Taffy hidden, no grid), `D-125` (closed kinds, text-only inheritance), `D-147` (the paint list), `D-148` (theme files, reloaded), `D-149` (routing), `D-162`, `D-163` (owned roots, real-clock tracks), `D-164` (capture, prompts), `D-129` (clocks), `D-093` (transitions), `D-046` (the stack), `D-181`/`D-199` (the shell and its budget), `D-197`/`D-211` (01's copy and masked theme), `D-226` (guards). Q21's answer: keys and named styles take one entry amending `D-125`; styles are applied, not cascaded.
- Later items expect: 7.20, keyed builders, named styles and the screen helper loading a screen by ID; 7.11, the meter and room in the shell's budget; 8.1, the widgets for its dialogue box, inventory and shops; 7.13, every row's 01 sites.

## 1. Audit

Read at `7cd69a4a`: the template's shell, sample and tests; 01's copy, theme, generator and the sites its features and tests take from the shell; the fixtures' helpers; core's UI tree, theme, theme file, events, router, tracks, prompts, layout and paint; the umbrella's state stack, transitions, UI plugin, harness, guards, script and describe; the CLI's `new`, `render` and `describe`. Each S-row's site search (the register's Sites cell) re-run here matches its count.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | The shell is 2,786 code lines, `src/` 3,355; the template's `AGENTS.md` 12,126 of 12,288 bytes (100 lines at most), the LLM index 12,223 of 12,288 | `tokei templates/basic/src/shell --output json \| jq '.Rust.code'`; `wc -c templates/basic/AGENTS.md docs/LLM_INDEX.md`; `scripts/check-agent-context.py:22-27` |
| A2 | `widgets.rs` builds the screen card (a modal scrim root on `GamePopup` and a scroll-panel card), the curtain, display, heading, note, small, mono and danger text, buttons named by caption with the pointer cursor, layout-only groups painted clear, caption, choice, slider and toggle rows, the tab rail, the prompt row, `set_width`, `focus_first`, `show`, `text_of`, and `Capped`/`recap`, which size display labels under the text scale up to a cap | `templates/basic/src/shell/widgets.rs:17-339` |
| A3 | `restyle` and `set_width` clone, edit and set a whole style; `fill` sets whole overrides, dropping a node's text overrides; 20 such sites | `widgets.rs:55-69`, `:310-315`; `rg -n 'layout_style\(\w+\)\.cloned\(\)\|overrides\(\*?\w+\)\.cloned\(\)' templates/basic/src` → 20 |
| A4 | `set_layout_style` and `set_overrides` replace whole and mark nothing on an equal value; `set_text`/`set_plain_text` compare on write; `accessible_name` falls back to a label's flat text only while no name is set; `text` returns `StyledText` | `crates/tungsten-core/src/ui/tree.rs:728-737`, `:1221-1231`, `:565-602`, `:978-988`, `:625` |
| A5 | The shell's 22 layout constants (padding, gaps, widths, text sizes, caps) live in Rust; the theme's `SpacingTokens` load and nothing reads them; `TextTokens` has one heading size nothing reads; kind styles take literal colours (30 in `shell.json`) | `templates/basic/src/shell/theme.rs:60-104`; `crates/tungsten-core/src/ui/theme.rs:116-208`; `rg -o '#[0-9a-fA-F]{6}' templates/basic/assets/ui/themes/shell.json` → 30 |
| A6 | A theme file is `tokens` and `kinds`, unknown fields refused; colours are `#rrggbb[aa]` only; `Theme` is `{ tokens, kinds }`, built only in core | `crates/tungsten-core/src/ui/theme_file.rs:42-102`, `:120-150` |
| A7 | Kinds are closed (`Panel`, `Label`, `Button`, `Image`, `Toggle`, `Slider`, `Mesh`); a panel, roots included, takes the panel kind's surface and border | `crates/tungsten-core/src/ui/node.rs:43-63`; `theme.rs:43-53`, `:210-226` |
| A8 | `UiEvent` is `Copy` and `#[non_exhaustive]` with `#[non_exhaustive]` variants; `Activated` carries its `WidgetId` alone; sliders send `ValueChanged` on every move and nothing at a drag's end; only core builds events | `crates/tungsten-core/src/ui/event.rs:16-140`; `route.rs:1262` |
| A9 | Handlers find widgets by position (`clicked(world, &[..])`, `Menu::pick`, `row_of`) and caption (`accessible_name(widget) == Some("Back")`): 10 sites | `templates/basic/src/shell/mod.rs:902`, `:986-991`; `rg -n 'clicked\(world\|\.pick\(world\|fn row_of\|let row_of' templates/basic/src/shell` → 10 |
| A10 | The shell's screen-change machine (`Change`: queued, closing, curtain, game over's entrance), `push`/`pop` with a 0.10 s slide in and a 0.08 s slide out, `enter` (owned, `RootCover::Show`), `idle`, `transitioning`, `cancelled`; the curtain pops every state above the bottom one and replaces it, staging the run until `shell_save` | `templates/basic/src/shell/mod.rs:164-185`, `:341-445`, `:955-998`; `theme.rs:13-25`, `:122-148` |
| A11 | Confirm (two answers, the safe one on cancel, countdown and exit) and toasts (an unowned root, 2.5 s on the real clock, raised each frame) are the shell's | `templates/basic/src/shell/confirm.rs:1-144`; `toast.rs:1-70` |
| A12 | Both games' tests hand-write `find`, `shown_modal`, `named`, `shows`; fit.rs's rules walk the tree (about 110 lines); `UiHarness::find` takes the first name in tree order and has one caller; the template's and 01's tests copy a WCAG `contrast` | `templates/basic/tests/game/main.rs:112-167`; `fit.rs:20-63`, `:153-217`; `examples/01_platformer/tests/game/main.rs:160-176`; `crates/tungsten/src/testing/ui.rs:184-198`; `crates/tungsten/src/tests/ui_route.rs:64`; `templates/basic/tests/game/shell.rs:774-789`; `examples/01_platformer/tests/game/presentation.rs:1323` |
| A13 | Template tests read `widgets::text_of` and `theme::` constants 58 times (`UI_SCALE`, `REFERENCE_HEIGHT`, `MIN_VIEWPORT`, `SLIDER_LINE` among them); its schedule snapshot names `ui_state_roots`, `ui_route`, `ui_animate`, `shell_save`, `shell_frame`, `shell_flow` | `rg -c 'widgets::\|theme::' templates/basic/tests/game/*.rs`; `templates/basic/tests/game/sample.rs:93-95` |
| A14 | 01's features take `shell::widgets` 75 times (`spells/hud.rs` 69: `fill`, `show`, `restyle`, `text_of`, `row`, `prompt_row`, `mono`, `display`, `danger`; `run/banner.rs` 6, `capped` among them) and `theme::GAP`, `MARGIN`, `SMALL` 14 times; three tests take `text_of`; the rest of 01 calls the seam's `shell::{top, stack, play, game_over, start_run, request_autosave, run_meta}` and `GameOptions`, `PAUSE`, `GAME_OVER` | `rg -o 'widgets::[a-z_]+' examples/01_platformer/src/features`; `rg -l 'widgets::' examples/01_platformer/tests`; `rg -o 'shell::[a-z_]+' examples/01_platformer/src/features` |
| A15 | 01's theme is generator output: the template's `shell.json` with each colour mapped through `THEME_COLORS` (an unmapped colour stops it, alpha kept); `just level-check` compares it and needs Pillow | `examples/01_platformer/tools/generate.py:28-55`, `:892-901`; `justfile:98-99` |
| A16 | `check_shell_copies` holds every example's `src/shell/` to the template's byte for byte and its `shell.json` with colours masked | `scripts/check-repo.py:167-209` |
| A17 | The fixtures rewrite eight of the shell's helpers (`heading`, `button`, `caption_row`, `name_control`, `slider_row`, `toggle_row`, `prompt_row`, `screen_root`) among 15 | `examples/03_fixtures/src/ui/widgets.rs:22-224` |
| A18 | Tracks tick on `Time::real_delta` only; a track per property, a new one replacing it with no event; no delay or chain | `crates/tungsten-core/src/ui/anim.rs:57-165`, `:318-351`, `:390-400` |
| A19 | Only a translation rounds to whole physical pixels; the layout turns Taffy's rounding off; a label commits at its rect's `max − min`, an ulp under its measure (known issue 32, with the fix's recipe) | `crates/tungsten-core/src/ui/paint.rs:151-162`; `layout/taffy.rs:381`, `:281`; `docs/known-issues.md:32` |
| A20 | `capture_next_input` takes no timeout; a prompt's `{action:NAME}` shows `Binding::label`, with no short form | `crates/tungsten-core/src/ui/route.rs:308-330`; `prompt.rs:150-170`; `crates/tungsten-core/src/input/action_map.rs:53` |
| A21 | The stack: `request_replace(_transition)` replaces the top only; one transition at a time; `Transition` has `out_secs` and `in_secs`; owned roots fade with it; hooks `on_pause`/`on_resume` only, `Time` never paused by the stack; it owns roots and scene entities, no resources | `crates/tungsten/src/state.rs:48-55`, `:150-232`, `:432-470`; `transition.rs:87-121`; `crates/tungsten-core/src/time.rs:227-240` |
| A22 | `UiPlugin` registers `ui_state_roots` before `ui_route`, then `ui_animate`; the app lays out in `stage_ui_layout` and records `RenderCounts::ui_roots_laid_out`; the harness has no renderer, and `UiHarness::step` drops each layout's `LayoutStats` | `crates/tungsten/src/plugins.rs:136-150`; `app.rs:2055-2079`; `testing/ui.rs:94-124`; `crates/tungsten-core/src/ui/tree.rs:53-62` |
| A23 | Counter guards pin allocations, ECS counts and sprite batches by `Harness`; none reads layout; the template has none, since `tungsten new` copies its tests | `crates/tungsten/src/testing/guard.rs:93-160`; `docs/perf/benchmarks.md:245-267` |
| A24 | `tungsten render` writes a script for a sheet, scene or point; `describe` loads assets and runs no frame; `new` takes one path; script probes are `frame`, `entities`, `count`, `camera`, `states`, `log` | `tools/cli/src/render.rs:15-146`; `describe.rs:1-40`; `crates/tungsten/src/describe.rs:1-14`; `tools/cli/src/args.rs:173-190`; `crates/tungsten/src/script.rs:127-152` |
| A25 | `TextOverrides`, `StyleOverrides` and `TextSection` have full literals (no `..`) only in `debug_views.rs`, its test and `app.rs`'s boot view: the files boot-and-seams' step 12 stopped on | the scan in §2.3; `git show 5a54f01d^:docs/plans/boot-and-seams.md` (step 12's evidence row) |
| A26 | `Config` has `game`, `window`, `display`, `render`, `logging`, `time`, `audio`, `assets`; the shell sets the reference height, floor, default UI scale and the debug HUD in Rust, again after each settings apply; the shell applies its theme by hand at the title | `crates/tungsten-core/src/config.rs:580-601`; `templates/basic/src/shell/mod.rs:248-259`, `:459-474`; `theme.rs:106-120`; `title.rs:141` |
| A27 | The kit's pin is `0x3334_9665_de0c_501a`; the `seams` regression reads pixel regions, no golden; `just visual` runs 02's golden, 03's post, transition, UI and seams suites and the template's title | `crates/tungsten/src/tests/kit_recipes/determinism.rs:36`; `examples/03_fixtures/tests/integration/seams_regression.rs:1-10`; `justfile:45-52` |

## 2. Design

The shapes below are this plan's; step 16's entry records them.

### 2.1 Rows

Lands: S-9, S-28, S-33, S-42, S-45, S-62, S-64, S-67, S-81, S-82, S-91, S-93, S-94, S-102, S-104, S-108, S-109, S-114, S-116, S-123, S-127, S-133, S-139, S-152, S-153, S-155, S-157, S-163, S-171, S-183, S-187 and P7-14. A row lands with its engine part and its template and shell sites; 01's own sites wait for 7.13 (Q4). Moves, by phase 7's §2.1, at step 16: S-162 to 8.1 in §2.3 (Q2) and S-149 to 7.11 (Q3).

### 2.2 Styles in place, plain text, groups and keys (S-82, S-171, S-183, S-155)

- `UiTree::edit_layout(id, impl FnOnce(&mut LayoutStyle))` and `edit_overrides(id, impl FnOnce(&mut StyleOverrides))`: the edit on a copy, set and marked only when it changed. `UiTree::plain_text(id) -> String`: a label's or button's spans joined, empty for none.
- `UiTree::group(parent, style)`: a `Panel` (the kinds stay closed) marked bare, which paints no surface and no border under any theme, only its own overrides; `set_bare(id, bool)` makes a root or a scroll panel one.
- `UiKey`: a `Copy` key, `UiKey::new(name)` a `const fn` (FNV-1a 64 of the name), named apart from the kit's `Tags`. `UiTree::set_key(id, name)`, `key(id)`, `key_name(id)`, `find_key(under, key)` (depth first, the first match; a duplicate under one root warns once). `Activated`, `DoubleClicked`, `FocusGained` and `ValueChanged` carry `key: Option<UiKey>`; `EventQueue<UiEvent>::activated_key(key) -> Option<WidgetId>` and `value_changed_key(key)`.

### 2.3 The theme file (S-91, S-114, S-108, S-109's data)

- `Theme` gains `text_styles` (name → `TextOverrides`), `sizes` (name → UI units), `sounds` (`focus`, `activate`, `back`: manifest sound IDs, each optional); `ColorTokens` gains `scrim` (built in: the background at alpha 184) and `raised` (a hovered or focused fill; built in: the surface), the colours the shell's kinds use beside the ten tokens. Built-in styles from the tokens: `heading` (the heading family at `heading_size`), `note` (muted), `small` (13 on 18, muted), `mono` (the mono family), `danger` (the danger colour). `Theme::size(name, default)`.
- The file's `text_styles`, `sizes`, `sounds` and `colors.scrim`; every colour field takes `#rrggbb`, `#rrggbbaa` or a colour token's name, resolved after the file's tokens, an unknown name refused with its field path.
- `UiTree::set_text_style(id, Option<&str>)`: a layer between the state variants and the node's overrides, applied, not cascaded (Q21): its text properties then pass down as every text property does. An unknown name warns once and applies nothing (Q6).
- `ui::contrast_ratio(a, b) -> f32`: WCAG's ratio of two opaque sRGB colours, which both games' tests call in place of their copies.
- The scan for full literals (A25): a field added to `TextOverrides`, `StyleOverrides`, `TextSection` or `WorldLabel` edits `crates/tungsten/src/{debug_views.rs,app.rs}` and `tests/debug_views.rs`, listed where a step adds one.

### 2.4 Text outlines and shadows (P7-14)

Boot-and-seams' §2.15, unchanged: `TextOverrides` gains `outline` (`{ color, width }`) and `shadow` (`{ color, offset }`), inherited; theme files' text objects and named styles take them; `WorldLabel` and `TextSection` take both. An outlined area draws as eight copies at its width around it in the outline's colour, a shadow as one copy at its offset under them, then the text, sharing the shaped buffer (a node with coloured spans shapes a colourless second buffer while outlined or shadowed). Its paint box, clip bounds and batch-overlap box grow by the width on every side and the offset on its sides; its layout size stays. A world label's cull margin grows the same way. A text with neither draws as today.

### 2.5 Shrinking text and the commit width (S-93, S-152)

- `TextOverrides::shrink: Option<f32>`, in (0, 1]: the label keeps to one line (wrapping off); its measure reports at most the available width at a proportional height; it commits at the largest scale ≤ 1 at which its line fits its content box, never under `shrink`, on top of the text scale. The shell's caps go (Q7).
- The commit width: a label commits at Taffy's solved content size, or at its measured width when its box holds it (known issue 32's recipe), so a box sized to its text never wraps it.

### 2.6 The tree-only builders (S-9, S-127, S-104)

`tungsten_core::ui::widgets`, keeping the shell's names and signatures so the port is mechanical: `screen(tree, width) -> (root, card)` (the scrim root on `GamePopup`, modal, in `scrim`; the card a scroll panel padded `spacing.lg`, gapped `spacing.sm`, at most `width`); `heading`, `display`, `note`, `small` (labels in their named styles), `mono` and `danger` (styles set on a widget); `button` (padded by the sizes `button_padding_x` and `_y`, pointer cursor, named by its caption); `group`, `column`, `row`; `caption_row`, `choice_row`, `slider_row`, `toggle_row` (each control named by its row's caption, its width from `control_width`, a slider's line from `slider_line`, a readout's from `readout_width`); `tab_rail`, `select_tab`; `prompt_row`; `fill`, `restyle`, `set_width`, `focus_first`, `show`. `Menu::new(tree, heading, items: &[(&str, &str)])` builds the card, the heading and one button per (key, caption), keyed. `meter(tree, parent, width, height) -> Meter` and `Meter::set(tree, share, color)` (a track and a fill whose width is the share, compare on write). `radial(tree, parent, style)` and `set_radial(tree, id, share, color)`: a mesh clock wipe from twelve o'clock clockwise, at most 64 triangles, none at 0, a disc at 1.

### 2.7 The router (S-133, S-187, S-109, S-116, S-94)

- `UiEvent::ValueCommitted { widget, key, value }`: a slider's value when the press that dragged it ends, once, if it moved; with each key or pad step; a toggle's beside its `ValueChanged`. `EventQueue::value_committed(widget)`.
- `UiRouter::capture_next_input_for(widget, secs)`: the capture ends with `CaptureCancelled` once `secs` real seconds pass; `capture_remaining()`. `capture_next_input` stays untimed.
- `UiTree::set_back(id, bool)`: a back button's activation also sends `Cancel { scope }` for its root. `ui_sounds_system`, registered by `UiPlugin` after `ui_route`: the theme's `focus` sound when a navigation press moved focus, `back` on a `Cancel`, `activate` on any other activation, each at most once a frame, through `audio::play(world, id, 0.0)`; nothing for an unset sound.
- `input::pressed(world, action)` and `input::just_pressed(world, action)`: through `ActionMap`, on `RoutedInput` when the world holds one, else `InputState`.
- `Binding::short_label()`: the rule 01 applies by hand (`spells/hud.rs:814-851`) moved into core, with a pad button's short name; the prompt's `{short:NAME}` follows the last device as `{action:NAME}` does.

### 2.8 Clocks, snapping, the pointer sprite and world anchors (S-67, S-64, S-102, S-153)

- `UiTrack::on_game_clock()`: ticks on `Time::delta`, so it holds while the game clock is paused. `UiTrack::then(next)`: `next` starts as this ends, a chain on one property, each link's `AnimationFinished` with its tag; a new `animate` on the property replaces the chain.
- `UiTree::set_pixel_snap(root, bool)`: that root's boxes paint on whole physical pixels and its images at whole multiples of their natural size, at least one (Q17).
- `UiTree::set_pointer_sprite(Option<PointerSprite { sprite, size, hotspot }>)` and `show_pointer_sprite(bool)`: painted last at the pointer while shown; the app hides the system cursor then (Q16).
- `UiTree::anchor_to_world(root, point, offset)` and `clear_world_anchor(root)`: the umbrella's `ui_world_anchors` (`PostUpdate`, after the camera's systems) places the root's top-left at the point's screen position plus the offset, clamped inside the viewport by its last size.

### 2.9 The state stack (S-81, S-157, S-163)

- `request_reset_transition(state, transition)` and `request_reset_transition_with(state, transition, on_swap)`: on the boundary frame every state exits top down, as pops do (hooks, roots, scene entities), `on_swap` runs, then `state` enters; refused, the state dropped, while a transition is queued or active (Q8).
- `GameState::holds_clock(&self) -> bool { false }`: while such a state is covered the stack holds `Time`. A hold is `Time`'s own flag beside the game's pause (`set_held`, `is_held`): the game clock's dt is zero while either is on, and `pause`, `resume` and `is_paused` stay the game's, so a pause or resume the game makes under a hold survives the uncover (Q11).
- `StateStack::own_resource::<T>()`: `T` leaves the World when the state that owned it (the dispatching one, else the top) exits.

### 2.10 Screens in the umbrella (S-33, S-28, S-62)

`tungsten::screens`, opt-in through `ScreensPlugin`, which the shell's plugin builds (Q9):
- `push(world, state)`, `pop(world)` (the front-most shown modal root slides out, then pops), `reset(world, state, on_swap)` (§2.9 behind a fade in the theme's background, 0.10 s out and 0.14 s in), `idle(world)` (no change, transition or capture), `transitioning(world)`. One change at a time; a push acts at once, a pop after its slide. `PUSH_SECS` 0.10, `POP_SECS` 0.08, `SLIDE_UNITS` 6: the shell's motion.
- `ModalScreen::open(world, width)` in `on_enter`: `widgets::screen`, owned with `RootCover::Show`, slid in. `ModalScreen::update(&mut self, world) -> Option<UiKey>`: `None` while not idle; pops on its root's `Cancel` (so back buttons pop); else the key activated at this routing, whose widget becomes the opener. `ModalScreen::resume(&self, world)`: focus on the opener, else the first focusable.
- `Confirm::new(question, yes, no)` with `on_yes`, `on_no`, `countdown(secs)`; `screens::ask(world, confirm)`; the safe answer on cancel, the countdown's end and `Exiting`; the first answer final. `toast(world, text)` and `Toasts::texts(tree)`: an unowned root at the bottom, 2.5 s each on the real clock, above every screen and the reset's fade.
- Screens by name: `screens::register(world, id, fn() -> Box<dyn GameState>)` into `ScreenRegistry`, callable from a plugin's `build`; `screens::open(world, id) -> bool` pushes one.

### 2.11 The UI's configuration (S-123, S-139)

`tungsten.json`'s `ui`: `theme` (a `ui_themes` ID applied once loaded, re-applied on reload by `D-148`'s path; Q12), `reference_height`, `min_viewport`, `scale` (what an unset `Settings::ui_scale` applies as, kept by `tungsten::apply_settings`), `hud_corner`, `cursor` (§2.8's sprite). Each optional; an absent section leaves today's defaults.

### 2.12 The harness and the guard (S-42, S-45)

- `UiHarness::front_modal()`: the front-most shown modal root; `find(name)`: now the first focusable control named `name` in the visible roots from the front, as both games' `find`, so a toast's root above a screen is looked past (its one caller still passes); `named(name)` and `shows(text)`, any widget or label on `front_modal()`, as theirs; `find_key(key)`, on the visible roots from the front; `assert_fits(what)` and `assert_fits_one_line(what, labels)`: both games' rules (roots inside the viewport, boxes inside their parents across and down unless a scroll panel above scrolls them, focusable controls inside unless scrolled, the given labels one line each), measured at the fixed advance, so glyph widths are approximate (§5.9's constraint).
- `UiTree::dump_json()` and `UiHarness::tree_json()`: each node's ID, kind, key, accessible name, text, rect, focus and shown, by root and depth. `UiHarness::layout_totals()`: the `LayoutStats` of every layout it ran, summed (`LayoutStats: Add`).
- **The guard:** `an_idle_screen_lays_out_no_root` (`crates/tungsten/src/tests/screens.rs`): a `Menu` screen opened through the helper, stepped past its entrance, then 60 frames with no input move `layout_totals` by `LayoutStats::default()`; a focus move then lays out no root (a focused look repaints only, `crates/tungsten-core/src/ui/tree.rs:1379-1383`) and one label's new text exactly one, so the guard sees work. A row in `benchmarks.md`'s guard table.

### 2.13 The shell's port (steps 12 and 13)

- Step 12: every builder from `ui::widgets`, `widgets.rs` deleted; the 22 constants to the theme (spacing `lg` 16 for `PADDING` and `MARGIN`, `md` 12 for `CARD_PADDING`, `sm` 8 for `GAP`; sizes `button_padding_x` 14, `button_padding_y` 5, `menu_width` 260, `wide_width` 600, `control_width` 170, `readout_width` 64, `slider_line` 18, `title_gap` 14, `title_space` 40, `title_menu_width` 220, `title_button_padding_y` 3, `pip` 10; styles `heading` 20 on 28, `title` 56 on 64, `died` 64 on 72 (both shrinking), `small` 13 on 18); the kinds' colours by token name (`shell.json`'s literals all under `tokens`, `#1e2129` as `raised` and the scrim as `scrim`, so 01's `THEME_COLORS` still maps every one); every handler by key; `text_of` to `plain_text`; the sample's bar a `Meter`; 01's copy and theme (by `generate.py`, Pillow venv); 01's `spells/hud.rs` and `run/banner.rs` (their `use` line, `theme::` constants to spacing tokens and the `small` style, `capped` to a shrinking style) and three tests' `text_of`, mechanical (Q4).
- Step 13: confirm, toasts and the change machine to `tungsten::screens`; the curtain to `screens::reset`, staging the run in `on_swap`; game over's entrance stays the shell's; menu sounds to the theme file's `sounds` and Back buttons to `set_back`; the scale slider on `ValueCommitted`; the rebind timeout on `capture_next_input_for`; the reference height, floor, default scale, HUD corner and starting theme to `tungsten.json` (01's too); the sample's clock pause to `holds_clock` and its run resources to `own_resource`.
- Kept the shell's: what each screen holds, the seam (`AGENTS.md`'s names and `shell::{play, top, stack, game_over, start_run, request_autosave, run, run_meta}`, which 01 calls), saves, options, the post stack's edits, focus mute and pause, the display keys.

### 2.14 Test helpers and the fixtures

Both games' `find`, `shown_modal`, `named`, `shows`, fit.rs's rules and front-modal query, and the `contrast` copies go to `UiHarness` and `ui::contrast_ratio`; their callers' assertions stay (`rg -l -e 'fn find\(' -e 'fn shown_modal\(' -e 'fn named\(' -e 'fn shows\(' examples/01_platformer/tests templates/basic/tests` names the files at the step's start). The fixtures delete each helper whose core twin paints the same list (`UiHarness::dump_paint` equal) and keep the rest (Q15).

### 2.15 The agent surface

- The engine probe `ui`: the tree dump, whole or the front-most shown root's; the script step `screen` opens a registered screen by ID (`screens::open`).
- `tungsten render --screen <id>`: a script that waits for no transition, opens the screen, and captures its frame. `describe` lists `screens` (`{id, doc}`); it runs no frame, so trees come from the probe (Q13).
- `tungsten new screen <name>`: in a game's folder, writes `src/screens/<name>.rs` (a `Menu` screen on the helper and a plugin registering it by name) and `src/screens/mod.rs` when absent (else appends its `pub mod`), and prints the `lib.rs` and `game.rs` lines to add (Q14). The name is a Rust identifier, `[a-z][a-z0-9_]*`, no keyword, at most 64 characters, and the module's, file's and screen ID's at once: `new`'s game names take `-` (`tools/cli/src/new.rs:142`), so a hyphen is refused with its snake_case form named; an existing file or a `pub mod <name>` already in `mod.rs` is refused. The shell registers its screens (`title`, `pause`, `settings`, `keybindings`, `saves`, `options`).

## 3. Steps

Every code step ends with `cargo fmt --all -- --check` → exit 0 and `just lint` → exit 0; a step that changes the public surface runs `just api` and then `just api-check` → exit 0. "The guards, the pins and 01" means: `cargo test -p example-02-bench --locked guards` → exit 0, `cargo test -p example-01-platformer --locked` → exit 0, the kit's pin unchanged (`rg -c '0x3334_9665_de0c_501a' crates/tungsten/src/tests/kit_recipes/determinism.rs` → 1, its test passing in `cargo test -p tungsten --lib --locked kit_recipes::`).

### Step 1: Styles in place, plain text, groups and keys

- **Files:** step 1's list.
- **Change:** §2.2; the router fills each event's key from the tree.
- **Done-when:** `cargo test -p tungsten-core --lib --locked ui::` → exit 0, with `edit_layout_marks_only_a_change`, `edit_overrides_keeps_the_text_overrides`, `plain_text_joins_spans_under_a_set_name` (`tests/ui/tree.rs`), `a_group_paints_nothing_under_any_theme`, `a_bare_root_paints_nothing` (`tests/ui/paint.rs`), `an_activation_carries_its_widgets_key`, `a_value_change_carries_its_key`, `find_key_takes_the_first_in_tree_order` (`tests/ui/route.rs`) and `ui_key_new_is_fnv1a_of_its_name` (`tests/ui/tree.rs`); `cargo test -p tungsten --lib --locked ui_route::` → exit 0; `cargo build --workspace --all-targets --locked` → exit 0; the guards, the pins and 01.
- **Commit:** `feat(ui): edit styles in place, group panels and key widgets`
- **Moves:** none.

### Step 2: Named text styles, sizes, sounds and token colours in the theme file

- **Files:** step 2's list.
- **Change:** §2.3; `docs/assets.md`'s theme row names the new fields.
- **Done-when:** `cargo test -p tungsten-core --lib --locked ui::` → exit 0, with `text_styles_sizes_and_sounds_read_from_a_theme_file`, `a_colour_field_takes_a_token_name`, `an_unknown_token_name_is_refused_with_its_path` (`tests/ui/theme_file.rs`), `a_named_style_layers_between_states_and_overrides`, `a_named_style_is_not_cascaded_past_inheritance`, `an_unknown_style_name_applies_nothing` (`tests/ui/style.rs`) and `contrast_ratio_of_black_and_white_is_21` (`tests/ui/tree.rs`); `cargo test -p tungsten-core --test integration --locked manifests::` → exit 0 (every theme in the repository parses); the guards, the pins and 01.
- **Commit:** `feat(ui): name text styles, sizes and sounds in theme files`
- **Moves:** none.

### Step 3: Text outlines and shadows

- **Files:** step 3's list.
- **Change:** §2.4; two `seams` labels, one outlined, one shadowed; `docs/assets.md`'s theme row names `outline` and `shadow`.
- **Done-when:** `cargo test -p tungsten-core --lib --locked` → exit 0, with `outline_and_shadow_read_from_a_theme_file` (`tests/ui/theme_file.rs`), `a_nodes_outline_reaches_its_paint_command`, `an_outlined_text_grows_its_paint_box_and_keeps_its_layout`, `a_later_panel_over_an_outline_closes_the_batch_before_it` (`tests/ui/paint.rs`) and `a_world_label_takes_an_outline` (`tests/text.rs`); `cargo test -p tungsten-render --locked` → exit 0, with `an_outlined_area_draws_eight_copies_then_the_text`, `an_outline_is_not_cut_at_its_text_box`, `a_coloured_span_outlines_in_the_outline_colour` and `text_without_outline_prepares_as_before`; `cargo test -p tungsten --lib --locked debug_views::` → exit 0; `just visual` → exit 0 (local), `ui-visual.png` and `ui-labels.png` unchanged and `seams_regression` passing `an_outlined_label_rings_its_glyphs` and `a_shadowed_label_casts_down_right`; `just smoke` → passes (local); the guards, the pins and 01.
- **Commit:** `feat(text): draw outlines and shadows`
- **Moves:** none.

### Step 4: Shrinking text and the commit width

- **Files:** step 4's list.
- **Change:** §2.5; known issue 32's row removed.
- **Done-when:** `cargo test -p tungsten-core --lib --locked ui::` → exit 0, with `a_shrinking_label_keeps_one_line_in_a_narrow_box`, `a_shrinking_label_stops_at_its_least_share`, `a_shrinking_label_that_fits_keeps_its_size`, `a_label_sized_to_its_text_commits_one_line_at_an_inexact_advance` (`FixedAdvanceMeasure::new(0.6)`, `frame 111` in an anchored row at x 64, one line) (`tests/ui/layout.rs`) and `shrink_reads_from_a_theme_file` (`tests/ui/theme_file.rs`); `rg -q 'ulp under the width' docs/known-issues.md; test $? -eq 1` → exit 0; `just visual` → exit 0 (local), every golden unchanged; the guards, the pins and 01.
- **Commit:** `fix(ui): shrink labels to one line and commit at the measured width`
- **Moves:** none.

### Step 5: The tree-only builders

- **Files:** step 5's list.
- **Change:** §2.6.
- **Done-when:** `cargo test -p tungsten-core --lib --locked ui::` → exit 0, with `a_screen_is_a_modal_scrim_and_a_scroll_card`, `rows_name_their_controls_by_caption`, `a_tab_rail_shows_one_bar`, `a_menu_keys_its_buttons`, `a_meter_set_twice_marks_once`, `a_meter_at_half_fills_half_its_track`, `a_radial_fill_at_a_quarter_covers_the_top_right`, `a_radial_fill_at_zero_draws_nothing` and `builders_follow_the_theme_sizes` (`tests/ui/widgets.rs`); the guards, the pins and 01.
- **Commit:** `feat(ui): build screens, rows, menus and meters in core`
- **Moves:** none.

### Step 6: The router: commits, timeouts, back buttons, sounds, routed actions, short labels

- **Files:** step 6's list.
- **Change:** §2.7; the template's schedule snapshot gains `ui_sounds` after `ui_route`.
- **Done-when:** `cargo test -p tungsten-core --lib --locked` → exit 0, with `a_slider_commits_once_at_its_drags_end`, `a_key_step_commits_at_once`, `an_unmoved_drag_commits_nothing`, `a_timed_capture_cancels_after_its_seconds`, `a_back_button_sends_cancel_for_its_root` (`tests/ui/route.rs`), `a_short_placeholder_follows_the_last_device` (`tests/ui/prompt.rs`), `short_labels_shorten_long_names` (its action-map tests) and `routed_just_pressed_reads_what_the_ui_left`; `cargo test -p tungsten --lib --locked ui_route::` → exit 0, with `menu_sounds_play_once_a_frame_by_cause` and `a_focus_set_by_the_game_plays_no_move`; `cargo test -p tungsten-template-basic --locked` → exit 0; the guards, the pins and 01.
- **Commit:** `feat(ui): commit sliders, time captures and sound menus from the theme`
- **Moves:** the template's schedule snapshot (`ui_sounds`).

### Step 7: Tracks on the game clock, pixel snapping, the pointer sprite, world anchors

- **Files:** step 7's list.
- **Change:** §2.8; the template's snapshot gains `ui_world_anchors`.
- **Done-when:** `cargo test -p tungsten-core --lib --locked ui::` → exit 0, with `a_game_clock_track_holds_while_paused`, `a_chain_runs_in_hold_and_out`, `a_new_track_replaces_a_chain` (`tests/ui/anim.rs`), `a_snapped_root_paints_on_whole_pixels`, `a_snapped_image_scales_by_whole_art_pixels`, `the_pointer_sprite_paints_last_at_the_pointer` (`tests/ui/paint.rs`); `cargo test -p tungsten --lib --locked camera::` → exit 0, with `a_world_anchored_root_follows_its_point` and `a_world_anchored_root_stays_in_the_view`; `cargo test -p tungsten --lib --locked app::` → exit 0, with `a_shown_pointer_sprite_hides_the_system_cursor`; `cargo test -p tungsten-template-basic --locked` → exit 0; `just visual` → exit 0 (local); the guards, the pins and 01.
- **Commit:** `feat(ui): run tracks on the game clock, snap roots, anchor to the world`
- **Moves:** the template's schedule snapshot (`ui_world_anchors`).

### Step 8: The state stack: a reset transition, held clocks, owned resources

- **Files:** step 8's list.
- **Change:** §2.9.
- **Done-when:** `cargo test -p tungsten --lib --locked state::` → exit 0, with `a_reset_transition_exits_every_state_then_enters_one`, `a_reset_runs_its_swap_hook_between_the_exits_and_the_enter`, `a_reset_is_refused_while_a_transition_runs`, `a_covered_state_that_holds_its_clock_pauses_time`, `a_games_own_pause_survives_an_uncover`, `a_pause_made_under_a_hold_survives_the_uncover` (the stack holds, the game pauses, the screen closes: still paused) and `an_owned_resource_leaves_with_its_state`; `cargo test -p tungsten-core --lib --locked time::` → exit 0, with `a_hold_zeroes_dt_beside_the_games_pause`; the guards, the pins and 01.
- **Commit:** `feat(state): reset the stack, hold clocks under cover, own resources`
- **Moves:** none.

### Step 9: Screens in the umbrella

- **Files:** step 9's list.
- **Change:** §2.10; the LLM index's state-stack row names `tungsten/screens/`, within its budget.
- **Done-when:** `cargo test -p tungsten --lib --locked screens::` → exit 0, with `a_pushed_screen_slides_in_and_takes_focus`, `cancel_pops_the_front_screen_after_its_slide`, `a_back_button_pops`, `focus_returns_to_the_opener_on_resume`, `a_screen_acts_only_when_idle`, `one_change_runs_at_a_time`, `a_reset_swaps_the_run_under_its_fade`, `confirm_takes_the_safe_answer_on_cancel_countdown_and_exit`, `the_first_answer_is_final`, `toasts_stay_above_a_reset_and_leave_on_the_real_clock` and `a_registered_screen_opens_by_id`; `just ctx` → exit 0; the guards, the pins and 01.
- **Commit:** `feat(screens): add modal screens, confirm and toasts to the umbrella`
- **Moves:** none.

### Step 10: The UI's configuration

- **Files:** step 10's list.
- **Change:** §2.11.
- **Done-when:** `cargo test -p tungsten-core --lib --locked config::` → exit 0, with `the_ui_section_reads_each_field` and `an_absent_ui_section_keeps_the_defaults`; `cargo test -p tungsten --lib --locked -- app:: settings::` → exit 0, with `the_configured_theme_applies_once_loaded`, `the_configured_scale_is_what_an_unset_setting_applies` and `the_configured_floor_and_reference_height_reach_the_scale_factor`; the guards, the pins and 01.
- **Commit:** `feat(config): configure the UI's theme, scale and floor in tungsten.json`
- **Moves:** none.

### Step 11: The harness and the idle-screen guard

- **Files:** step 11's list.
- **Change:** §2.12; the guard's row in `docs/perf/benchmarks.md`.
- **Done-when:** `cargo test -p tungsten --lib --locked testing::ui::` → exit 0, with `find_takes_the_front_most_visible_focusable_match`, `find_looks_past_a_toast_to_the_screen_below`, `named_finds_disabled_widgets_on_the_front_modal`, `find_key_finds_a_keyed_widget`, `assert_fits_fails_a_box_outside_its_parent`, `assert_fits_lets_a_scroll_panel_scroll`, `assert_fits_one_line_fails_a_wrapped_label` and `tree_json_names_key_text_rect_focus_and_shown`; `cargo test -p tungsten --lib --locked ui_route::` → exit 0 (its `find` caller); `cargo test -p tungsten --lib --locked screens::` → exit 0, with `an_idle_screen_lays_out_no_root`; the guards, the pins and 01.
- **Commit:** `test(ui): give UiHarness queries, a fit check and the idle guard`
- **Moves:** none (a new guard, no pin file).

### Step 12: The shell on core's widgets, styles and keys

- **Files:** step 12's list.
- **Change:** §2.13's step 12; the template's `AGENTS.md` drops `widgets` and `theme` from the sample's shell names and names `tungsten::core::ui::widgets`, within its budget; the scratch Pillow venv (`python3 -m venv <scratchpad>/level-venv`, `pip install -r examples/01_platformer/tools/requirements.txt`) regenerates 01's theme, deleted afterwards with `shutil.rmtree`.
- **Done-when:** `cargo test -p tungsten-template-basic --locked` → exit 0; `cargo test -p example-01-platformer --locked` → exit 0; `test ! -e templates/basic/src/shell/widgets.rs` → exit 0; `rg -n 'clicked\(world|\.pick\(world|fn row_of|let row_of|accessible_name\(\w+\) == Some' templates/basic/src/shell; test $? -eq 1` → exit 0; `jq -e '[.kinds | .. | strings | select(startswith("#"))] | length == 0' templates/basic/assets/ui/themes/shell.json` → exit 0; `tokei templates/basic/src/shell --output json | jq '.Rust.code'` → under 2,786, quoted; `just repo-check` → exit 0; `PATH="<venv>/bin:$PATH" just level-check` → exit 0; `just ctx` → exit 0; `just visual` → exit 0 (local), the template's `title.png` unchanged; `cargo test -p tungsten-cli --locked` → exit 0; the guards and the pins.
- **Commit:** `refactor(template): build the shell from core's widgets and keys`
- **Moves:** the template's shell, sample, theme, tests and `AGENTS.md`; 01's copy, theme, two features and three tests (mechanical).

### Step 13: The shell on the umbrella's screens

- **Files:** step 13's list.
- **Change:** §2.13's step 13; the template's `AGENTS.md` names `tungsten::screens` for hotkey guards (`screens::idle`); the venv again for 01's theme.
- **Done-when:** `cargo test -p tungsten-template-basic --locked` → exit 0; `cargo test -p example-01-platformer --locked` → exit 0; `test ! -e templates/basic/src/shell/confirm.rs && test ! -e templates/basic/src/shell/toast.rs` → exit 0; `rg -q 'set_reference_height|set_min_viewport|MENU_SOUNDS|CAPTURE_SECS|self\.held = ' templates/basic/src/shell; test $? -eq 1` → exit 0; `rg -q 'resource_mut::<Time>\(\)\.pause|remove_resource' templates/basic/src/sample/mod.rs; test $? -eq 1` → exit 0; the `tokei` count quoted, under step 12's; `just repo-check`, `just level-check` (the venv) and `just ctx` → exit 0; `just visual` → exit 0 (local), `title.png` unchanged; `just smoke` → passes (local); `cargo test -p tungsten-cli --locked` → exit 0; the guards and the pins.
- **Commit:** `refactor(template): run the shell's screens on tungsten::screens`
- **Moves:** the template's shell, sample, `tungsten.json`, theme, tests, snapshot and `AGENTS.md`; 01's copy, `tungsten.json` and theme.

### Step 14: Both games' test helpers and the fixtures

- **Files:** step 14's list.
- **Change:** §2.14.
- **Done-when:** `rg -n -e 'fn find\(' -e 'fn shown_modal\(' -e 'fn named\(' -e 'fn shows\(' -e 'fn assert_rules\(' -e 'fn contrast\(' templates/basic/tests examples/01_platformer/tests; test $? -eq 1` → exit 0; `cargo test -p tungsten-template-basic --locked` → exit 0; `cargo test -p example-01-platformer --locked` → exit 0; `cargo test -p example-03-fixtures --locked` → exit 0; `just visual` → exit 0 (local), `ui-visual.png` and `ui-labels.png` unchanged; `cargo test -p tungsten-cli --locked` → exit 0.
- **Commit:** `test(ui): use UiHarness queries in both games and the fixtures`
- **Moves:** the tests' helpers and the fixtures' widget helpers.

### Step 15: The agent surface

- **Files:** step 15's list.
- **Change:** §2.15; `docs/getting-started.md` shows `render --screen` and `new screen`; the CLI's help snapshot.
- **Done-when:** `cargo test -p tungsten --lib --locked script::` → exit 0, with `the_ui_probe_dumps_the_front_screen` and `a_screen_step_opens_a_registered_screen`; `cargo test -p tungsten --lib --locked describe::` → exit 0, with `describe_lists_registered_screens`; `cargo test -p tungsten-cli --locked` → exit 0, with `render_screen_writes_a_screen_script`, `new_screen_writes_its_file_and_module`, `new_screen_refuses_an_existing_file`, `new_screen_refuses_a_hyphenated_name`, `new_screen_refuses_a_module_already_declared` and `new_with_one_path_still_makes_a_game`; the compile check: in `templates/basic`, `tungsten new screen inventory`, its two printed lines added, `cargo check -p tungsten-template-basic --locked` → exit 0, then `lib.rs` and `game.rs` restored with `git show HEAD:<path> > <path>`, `src/screens/` deleted with `shutil.rmtree`, and `git status --short templates/basic` → empty; from `templates/basic`, `tungsten render --screen settings -o target/ui-pass/settings.png` → exit 0, the PNG written (local); the shell's copy synced, `just repo-check` → exit 0; the guards, the pins and 01.
- **Commit:** `feat(cli): render screens by name and add tungsten new screen`
- **Moves:** the CLI's help snapshot.

### Step 16: The decision and the records

- **Files:** step 16's list.
- **Change:** with the tungsten-decision skill, one entry at the next free ID (recheck it and `git status` first; `D-23[5]` at writing): keys, groups and named styles, amending `D-125` (Q21); the theme file's styles, sizes, sounds, scrim and token colours, amending `D-148`; outlines, shrink, snapping and the pointer sprite, amending `D-147`; commits, back buttons, sounds, timed captures and short labels, amending `D-149` and `D-164`; game-clock tracks and chains and the screens helper, amending `D-163`; the reset transition, held clocks and owned resources, amending `D-046` and `D-093`; the `ui` config section; the shell's parts in the engine and its measured count, amending `D-181` and `D-199`; `new screen`, `render --screen` and the `ui` probe, amending `D-182`; the idle guard, amending `D-226`; each amended entry with its marker line and index row. `DESIGN.md`'s UI and template sections; `docs/getting-started.md`'s shell section; the LLM index within its budget; S-162 to phase 7's §2.3 under 8.1 and S-149 to 7.11 in §2.2, each `moved from 7.17 (<reason>)`; known issues for anything the run met.
- **Done-when:** `rg -c '^## D-23[5] — ' DECISIONS.md` → 1 (a later ID if that one is taken, the patterns changed to match); `rg -c '^\*\*Amended by D-23[5]:\*\*' DECISIONS.md` → the number of entries it amends, at least the eleven named; `cargo test -p tungsten-core --test integration --locked decision_index` → exit 0; `rg -n '^\| S-162 \| [^|]+ \| 8\.1 \| open \|' docs/plans/phase-7.md` → one line; `rg -n '^\| S-149 \| [^|]+ \| 7\.11 \| open \|' docs/plans/phase-7.md` → one line; `just ctx` and `just repo-check` → exit 0.
- **Commit:** `docs(decisions): record the UI pass's engine parts`
- **Moves:** none.

### Step 17: Close

- **Files:** step 17's list.
- **Change:** the checks and the demos; the 32 rows `landed <sha>` in phase 7's §2.2, each 01-site note naming 7.13 (Q4), and Step 6's evidence row; follow-ups to known issues or Later; the plan deleted with its citations; `ui-pass` out of Now and phase 7's Now line naming 7.20 next.
- **Done-when:** `just check` → exit 0; `just physics-release`, `just api-check`, `just docs-ratchet`, `just perf-test`, `just ctx`, `just repo-check` and `git diff --check` → exit 0; `just smoke` → passes, `just perf counters` → ok and `just visual` → exit 0 (local); from `templates/basic`, `tungsten run --script` on a script in `perf-runs/<date>-ui-pass/` that opens `settings`, asserts the `ui` probe's front root holds a control keyed `ui_scale`, backs out and exits → exit 0, every assert passed; from `examples/01_platformer`, a script that starts a run, pauses, asserts the probe's front root is `pause`'s and resumes → exit 0; `tungsten render --screen settings` and `--screen pause` from `templates/basic` copied to the run's folder as the sheet; the shell's final `tokei` count quoted; `rg -n '^\| [^|]+ \| [^|]+ \| 7\.17 \| open \|' docs/plans/phase-7.md; test $? -eq 1` → exit 0; `rg -q 'ui-pass\.md' --glob '!docs/plans/archive/**' --glob '!CHANGELOG.md' --glob '!DECISIONS.md' .; test $? -eq 1` → exit 0.
- **Ends with:** the commits, the evidence, what the owner should try (the template: Settings' scale slider applying on release, Back by Escape and by button with its sound, a rebind's timeout, a toast over a run change; 01: its menus unchanged; the render sheet), the next prompt: Step 8 (7.20)'s plan, tungsten-next's `step` prompt with phase 7's Step 8, the redo's 7.20 row and §5.11.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Seventeen steps for one plan | One plan, as the prompt names it; a run that finds it too large splits after step 11 (the engine's parts first, the shell's port second), each half with its own close |
| Q2 | S-162's grid layout | Moved to 8.1: §5.9 keeps grid layout waiting under `D-124`'s no-grid rule, and 8.1's inventory and shops are its first users; Taffy's `grid` feature stays off |
| Q3 | S-149's shell assets and hit-flash material | Moved to 7.11: they reach a game by `tungsten new`'s copy (`D-183`) and 01's generator (`D-197`), the template's packaging; the kit's material would join `D-231`'s reserved ones there |
| Q4 | 01's sites | 01's own sites move at 7.13. Here 01 changes through its shell copy, theme and `tungsten.json` and the mechanical edits the shell's removed names force: `spells/hud.rs`, `run/banner.rs` and three tests' `text_of` (step 12), and its tests' copied helpers (step 14), their assertions unchanged |
| Q5 | Keys | A 64-bit hash of a name, so a Rust `const` and 7.20's data name the same key; the tree keeps the name for dumps; a duplicate under one root warns once and `find_key` takes the first |
| Q6 | An unknown style name | Warns once and applies nothing, as an unloaded theme draws the built-in one; `tungsten check` refusing one waits for 7.20's screen files |
| Q7 | Caps or shrinking | Shrinking replaces `Capped`: the title and "YOU DIED" keep to one line at any viewport, and no golden label shrinks at its size, so `title.png` holds |
| Q8 | The curtain | An engine reset transition fading to the theme's background, 0.10 s out and 0.14 s in, the run staged in `on_swap`; owned roots fade with it and toasts, unowned, stay above |
| Q9 | Where `ScreensPlugin` runs | Opt-in, built by the shell's plugin, so only the template's schedule snapshot moves and engine-only games keep theirs |
| Q10 | A back button | Sends `Cancel` for its root, so the helper's pop on cancel covers it and plays the back sound |
| Q11 | A held clock and the game's pause | Two flags on `Time`: the stack sets only the hold, the game only its pause, so neither undoes the other |
| Q12 | The starting theme's home | `tungsten.json`'s `ui.theme`, beside S-123's fields: merged manifests would need a rule for whose default wins |
| Q13 | The tree dump from `describe` | `describe` lists the screens; their trees come from the `ui` probe on `run --script`, since `describe` runs no frame |
| Q14 | What `new screen` edits | Writes its screen's file and its own `src/screens/mod.rs` only and prints the two lines the game adds; it edits no other Rust file |
| Q15 | A fixture helper whose core twin paints differently | Kept, so `ui-visual.png` and `ui-labels.png` hold |
| Q16 | Who shows the pointer sprite | The game, through `show_pointer_sprite`; 01's gameplay takes it at 7.13 |
| Q17 | Snapping's scope | Per root, so a pixel-art HUD snaps and a menu keeps sub-pixel text |

Each question takes its stated default; the owner reviews the plan on their own time.

## 5. Invariants

- **May move:** `api/`, `docs/assets.md` and the CLI's help where a step changes the surface or a format; the template's shell, sample, theme, `tungsten.json`, tests, schedule snapshot (steps 6, 7, 13) and `AGENTS.md`, within `D-199`'s budget; 01's shell copy, theme, `tungsten.json`, `spells/hud.rs`, `run/banner.rs` and its tests' helper calls (steps 12–14); 03's UI helpers (step 14) and `seams` labels (step 3); known issue 32 (step 4).
- **Must not move:** the determinism, tunneling and containment hashes; the kit's pin `0x3334_9665_de0c_501a`; every guard pin (01's `pit`, 02's rows); `gpu-visual.png`, `ui-visual.png`, `ui-labels.png`, the template's `title.png`, the post, transition and `seams` regressions; 01's test assertions and its sources outside §5's list; the existing rows' workloads and digests.

## 6. Stop conditions

- A done-when check fails: restore the step's files from the last commit, mark the step skipped, and continue with steps that don't depend on it. Dependencies: 5 on 1 and 2; 6 on 1 and 2 (the theme's `sounds`); 9 on 5 and 8; 10 on 7 for its `cursor` field alone, which a skipped 7 leaves out; 11 on 1, 5 and 9; 12 on 1, 2, 4 and 5; 13 on 6, 8, 9, 10 and 12; 14 on 2, 11 and 12; 15 on 9, 11 and 13; 16 on every landed step; 3, 4, 7 and 8 on nothing (named styles carry step 3's outline once step 2 lands, and a skipped 2 leaves step 3 to theme files' text objects).
- A hash, the kit's pin, a guard pin or a golden moves, or a 01 test fails or needs more than a helper call changed: stop the step, without editing those tests, pins or images to pass.
- A file outside the step's list: stop the step.
- The Pillow venv cannot install: stop step 12's or 13's 01 theme with the step, whose template part cannot land alone (`check_shell_copies`).
- The template past 3,400 lines, its shell past 2,900 or its `AGENTS.md` past its bytes after the trims: stop that step.
- A skipped step's rows move by phase 7's §2.1 to the step that builds their system next (the shell's to 7.11), named at the stop, so the close's open-row check reads none open.

## 7. Critique

Round 1: gpt-6-astra through `critique --raw --repo`, 2026-11-02, reading the draft at `7cd69a4a`. Footer: `critique: model gpt-6-astra, effort xhigh, context 872000, mode repo (output cap 4000), 367.9 s, tokens in 1577239 (cached 1478528), out 10508 (reasoning 5404), commands 33, output 371.5 KB (largest 31.5 KB), usage 5h 0% → 30%, week 90% → 95%`.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] Step 1 cannot compile in its files: core tests build the events that gain `key`; step 6 names `input/mod.rs` | `crates/tungsten-core/src/tests/ui/control.rs:338`, `window.rs:144`, `:209`, `:231` build `ValueChanged` and `Activated`; core's input module is `crates/tungsten-core/src/input.rs:6` | Accepted | Step 1 lists `control.rs` and `window.rs`, step 6 `input.rs`. The same scan, rerun on every step, added `node.rs` to step 2 (a node's style name) and `debug_views.rs` with its test to step 4 (`shrink` on `TextOverrides`, A25) |
| 2 | [MEDIUM] The guard's positive control needs a focus move to lay out a root, which the resolver avoids | `crates/tungsten-core/src/ui/tree.rs:1379-1383` marks layout only on a measuring text change; `tests/ui/focus.rs:251-255` pins a background-only focus at no root; the built-in focused look changes border colour and width (`theme.rs:277-280`) | Accepted | §2.12: a focus move lays out no root and one label's new text exactly one |
| 3 | [MEDIUM] Resuming only a pause the stack made loses a pause the game makes while covered | `crates/tungsten-core/src/time.rs:227-240`: one `paused` flag | Accepted | §2.9: a hold flag on `Time` beside the game's pause, dt zero while either is on; step 8 lists `time.rs` and its tests, with `a_pause_made_under_a_hold_survives_the_uncover` and `a_hold_zeroes_dt_beside_the_games_pause`; Q11 |
| 4 | [MEDIUM] A front-most-root `find` breaks under a toast, which the games' `find` looks past | `templates/basic/tests/game/main.rs:112-127` (visible roots from the front), `:130-151` (`named` on the front modal); `templates/basic/src/shell/toast.rs:65-67` raises its root each frame; `saves.rs:637-639` presses Load under a toast | Accepted | §2.12: `find` scans the visible roots from the front as both games' does, `named` and `shows` take `front_modal`; step 11 adds `find_looks_past_a_toast_to_the_screen_below` |
| 5 | [MEDIUM] The skip graph omits 3 on 2 and 10 on 7 | §2.4: named styles are `TextOverrides` maps, so step 3 needs nothing of step 2's; §2.11's `cursor` configures step 7's sprite | Partly accepted | §6: 10 on 7 for `cursor` alone; 3 stays independent, with its note. Rechecked whole: 6 on 2 (the theme's `sounds`), 11 on 1 and 5, 14 on 2, 15 on 11 added |
| 6 | [MEDIUM] `new`'s name rule takes hyphens, which no module name can | `tools/cli/src/new.rs:142-146` takes `-` and `_` | Accepted | §2.15: a screen name is a Rust identifier, a hyphen refused with its snake_case form named, a declared module refused; step 15 adds both refusals and a compile check of a generated screen in the template, restored after |

Round 2 (due: round 1 changed the clock hold, the guard's control, step 15's compile check and the skip graph): not run. `critique --raw --repo` at xhigh on the plan without §7 and the log, with a focus file naming round 1's changes and its rejection, stopped after 48 commands: `critique: codex exec failed (exit 1, usage limit reached)`, usage 5h 30% → 63%, week 95% → 100%, the weekly window resetting 2026-11-04 16:02 UTC. Round 1's changes stand unreviewed; the run checks them as it reaches them.

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-11-02 | passed | `ui::` 261 passed, the 9 named among them; `ui_route::` ok; the workspace build, fmt, `just lint` and `api-check` exit 0; 02's `guards` 12 passed; 01 443 passed; the kit's pin 1 and `kit_recipes::` 93 passed. Departure: the tree's warn-once sets are one, `warned: HashSet<Warned>`, since the app re-inserts `UiTree` each frame it paints and its size sits in 01's `pit` pin: a second 48-byte set read `allocated_bytes` 69,118,796 → 69,121,676 (60 frames × 48). `UiEvent`'s key (20 → 32 bytes) left the pin as it was | — |
| 2 | 2026-11-02 | passed | `ui::` 268 passed, the 7 named among them; `manifests::` 6 passed (every theme parses); fmt, `just lint` and `api-check` exit 0; 02's `guards` 12 passed; 01 443 passed; the kit's pin 1, `kit_recipes::` 93 passed. Departures, each keeping `UiTree` at 3,568 bytes (01's `pit` pin, step 1's row): the text styles, sizes and sounds sit behind one pointer, `Theme::names: Box<ThemeNames>`, read through `Theme::size`, `text_style` and `sounds`; a named style is a `NamedStyle` (its `text: TextOverrides`), so step 3's outline and shadow join it without `tree.rs`; the tree's theme ID is a boxed `str` and its text and debug scales bare `f32`s (0 reads 1), paying for the pointer and the two new colour tokens | — |
| 3 | 2026-11-02 | passed | core 1,376 passed, the 5 named among them; render 195 passed, the 4 named among them (`an_outline_is_not_cut_at_its_text_box` on a UI copy's bounds); `debug_views::` 38 passed; `just visual` exit 0: `ui_visual_matches_fixture` and `ui_labels_match_fixture` pass, `seams_regression` 11 passed with `an_outlined_label_rings_its_glyphs` and `a_shadowed_label_casts_down_right`; `just smoke` passed; fmt, `just lint` and `api-check` exit 0; 02's `guards` 12 passed; 01 443 passed; the kit's pin 1, `kit_recipes::` 93 passed. Departures, each for a pin: outline and shadow are `NamedStyle` fields that `TextStyle` carries down, since a `TextOverrides` field costs 30 inline copies in `UiTree`'s theme, so a kind's text object refuses them and a node takes one by naming a style; a UI label's copies are text commands painted before its own, a coloured span's through `TextNodeId::plain()`, since the extract clones the paint list each frame; `TextSection` takes neither, since 02's `gpu` row collects its sections every frame (two fields read `allocated_bytes` +9,600) | the seams capture, `tungsten-seams-regression.png` in the temporary folder (local) |
| 4 | 2026-11-02 | passed | `ui::` 277 passed, the 5 named among them (`a_label_sized_to_its_text_commits_one_line_at_an_inexact_advance`: one line at 86.4, where the old `max − min` width wraps it to 2); the known-issues check exit 0, row 32 gone; `just visual` exit 0, every golden unchanged; fmt, `just lint` and `api-check` exit 0; 02's `guards` 12 passed; 01 443 passed; the kit's pin 1, `kit_recipes::` 93 passed. Departure: `shrink` is a `NamedStyle` field, not a `TextOverrides` one (step 1's row), which the layout reads from the label's own style or its button's, so it does not pass further down; the layout sets a shrinking label whole on one line before the solve and at its fit after it, both in `layout/taffy.rs` | — |
| 5 | 2026-11-02 | passed | `ui::` 286 passed, the 9 named in `tests/ui/widgets.rs` among them; fmt, `just lint` and `api-check` exit 0; 02's `guards` 12 passed; 01 443 passed; the kit's pin 1, `kit_recipes::` 93 passed. As built: `display` takes a style name in place of a size, `mono` and `danger` restyle the family and colour as the shell's do (so a port changes no look), `fill` keeps a node's other overrides, and `widgets::size` reads a theme size with the template's number as its fallback | — |
| 6 | 2026-11-02 | passed | core 1,398 passed, the 8 named among them (`routed_just_pressed_reads_what_the_ui_left` beside the action-map tests); `ui_route::` 19 passed with `menu_sounds_play_once_a_frame_by_cause` and `a_focus_set_by_the_game_plays_no_move`; the template 167 passed; fmt, `just lint` and `api-check` exit 0; 02's `guards` 12 passed; 01 443 passed; the kit's pin 1, `kit_recipes::` 93 passed. Departures: the routing plays the menu sounds at its end, no `ui_sounds` system and no snapshot move, since `App::run_stage` copies each system's name into the frame's timings every frame (one more system is one more allocation a frame in every guard); a back button is a widget keyed `back` (`ui::BACK_KEY`), no `UiTree::set_back`, since a flag needed `node.rs`, outside this step's list | — |
| 7 | 2026-11-02 | passed | `ui::` 298 passed, the 6 named among them; `camera::` 9 passed with both anchor tests; `app::` 55 passed with `a_shown_pointer_sprite_hides_the_system_cursor`; the template 167 passed; `just visual` exit 0, every golden unchanged; fmt, `just lint` and `api-check` exit 0; 02's `guards` 12 passed; 01 443 passed; the kit's pin 1, `kit_recipes::` 93 passed. Departures, each for a pin: a chain is `UiTrack::then` → `UiChain`, started with `UiTree::animate_chain`, its later links waiting entries in the track list (`RunningTrack` keeps 56 bytes, `UiTrack` 40 with the game-clock flag in its padding); the pointer sprite is a root keyed `pointer` (`ui::POINTER_KEY`) on the popup layer that the app places each frame, no tree field; the world anchors are placed at the end of the camera update, no `ui_world_anchors` system and no snapshot move (step 6's row); `App`'s cursor and its hidden flag are one `WindowCursor` field (clippy's bool count) | — |
| 8 | 2026-11-02 | passed | `state::` 38 passed, the 7 named among them; `time::` 28 passed with `a_hold_zeroes_dt_beside_the_games_pause`; fmt, `just lint` and `api-check` exit 0; 02's `guards` 12 passed; 01 443 passed; the kit's pin 1, `kit_recipes::` 93 passed. As built: `Time`'s pause and hold are one `ClockStop` field (clippy's bool count); the dispatcher sets the hold once a frame, before the top state's update, so it takes the next frame's advance as the game's pause does | — |
| 9 | 2026-11-02 | passed | `screens::` 11 passed, the 11 named; `just ctx` exit 0 (the LLM index 12,253 of 12,288 bytes); fmt, `just lint` and `api-check` exit 0; 02's `guards` 12 passed; 01 443 passed; the kit's pin 1, `kit_recipes::` 93 passed. Departure: `ScreensPlugin` inserts the resources and adds no system (one more system is one more allocation a frame in every guard, step 6's row): a closing screen pops when its slide's tagged track ends, read by its own `ModalScreen::update`, and a toast's time is an animation chain, its spent panels cleared at the next toast | — |
| 10 | 2026-11-02 | passed | `config::` 66 passed with `the_ui_section_reads_each_field` and `an_absent_ui_section_keeps_the_defaults`; `app::` and `settings::` 76 passed with the 3 named; fmt, `just lint` and `api-check` exit 0; 02's `guards` 12 passed; 01 443 passed; the kit's pin 1, `kit_recipes::` 93 passed. As built: `ui.hud_corner` is core's `config::UiCorner`, which the app maps to the HUD's corner; the configured theme applies at the first hot-reload stage that finds it loaded, once, so a theme the game sets after stays | — |
| 11 | 2026-11-02 | passed | `testing::ui::` 40 passed with the 8 named; `ui_route::` 19 passed (its `find` caller unchanged); `screens::` 12 passed with `an_idle_screen_lays_out_no_root`; core's `layout_stats_add_field_by_field_saturating` passed; fmt, `just lint`, `api-check` and `just repo-check` exit 0; 02's `guards` 12 passed; 01 443 passed; the kit's pin 1, `kit_recipes::` 93 passed. As built: the guard's text change lays out exactly one root (its positive control); `assert_fits_lets_a_scroll_panel_scroll` also checks the same rows in a plain panel fail | — |
| 12 | 2026-11-02 | passed | `cargo test -p tungsten-template-basic` 167 passed; 01 443 passed (the `pit` pin with them); `test ! -e …/widgets.rs` exit 0; the position-and-caption `rg` exit 1; the kinds' `jq` true; the shell's `tokei` 2,647 (from 2,786; `src/` 3,198); `just repo-check`, `just level-check` in the scratch venv (6,293 outputs, 20 locks, 45 tool tests) and `just ctx` exit 0; `just visual` exit 0, `the_title_matches_its_fixture` ok; `tungsten-cli` 59 and 7 passed once the deletions were staged (`the_embedded_template_is_the_tracked_one` reads the index); fmt, `just lint` and `api-check` exit 0; 02's `guards` 12 passed; the kit's pin 1, `kit_recipes::` 93 passed. As built: 01's `spells/hud.rs` keeps the shell's `text_of` as a private helper, since `plain_text` allocates differently and the pit counts the HUD's per-frame comparisons; the shell's `Menu` keeps its "Saving is off" line, its buttons keyed and read by `Menu::chosen`; the curtain's root moves into `mod.rs` until step 13; the sample enters its HUD through `shell::enter` and zooms by `ScaleFactor::reference_height`; the template's `AGENTS.md` 12,230 bytes | — |
| 13 | 2026-11-02 | skipped | The step deletes `shell/toast.rs`, whose `tick` boxes the shell's `Toasts` back into the World every frame, and the engine's toasts run no per-frame system. A probe with only that tick dropped from 01's copy fails the pit guard: `allocations` 72,637 → 72,577 and `allocated_bytes` 69,118,796 → 69,112,076, one 112-byte box a frame over the 60-frame window, a guard pin moving (§6); the probe's file restored from `HEAD`. Not built. Step 15 depends on it and skips with it; 14 and 16 run. Its rows move to 7.11, the shell's: S-28, S-33, S-62, S-81, S-109, S-123, S-133, S-139, S-157, S-163 and S-187, their engine parts landed in steps 6–10 | — |
| 14 | 2026-11-02 | passed | The helpers' `rg` exit 1 (`test $? -eq 1` exit 0); `tungsten-template-basic` 167 passed; 01 443 passed; `example-03-fixtures` 33 passed; `just visual` exit 0, `ui_visual_matches_fixture`, `ui_labels_match_fixture` and `the_title_matches_its_fixture` ok; `tungsten-cli` 66 passed; fmt, `just lint` and `api-check` exit 0; 02's `guards` 12 passed; the kit's pin 1, `kit_recipes::` 93 passed. The fixtures' `transparent`, `name_control` and `prompt_row` gave way to core's `fill`, `name` and `prompt_row` (re-exported by their `widgets`): a scratch test's paint and layout dumps of the lab, controls, play, pause and budget screens were byte-equal before and after, then deleted; the rest paint differently and stay. As built: `UiHarness::named` walks in tree order where the games' helper walked last child first, so a setting row's caption label, named as its control, comes first; the template's settings tests read controls by key through `find_key` (13 failed on the first run) | — |
| 15 | 2026-11-02 | skipped | Depends on step 13 (§6), skipped: the shell's screens register with `tungsten::screens` there, which `render --screen settings` and the `new screen` compile check open. Not built: the `ui` probe, the `screen` step, `tungsten render --screen`, `describe`'s screens and `tungsten new screen`; they wait in Later with step 13's port. No rows move with it | — |

## Follow-ups

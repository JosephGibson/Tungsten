# Phase 6 milestone 71: example fixtures in one test-only crate (Example fixtures)

- **status:** done
- **goal:** Examples 03, 04 and 05 and example 01's lighting fixture become one test-only crate, `examples/03_fixtures` (package `example-03-fixtures`), off the template layout and out of the release archives: 14 smoke rows and three of `just visual`'s suites, the moved captures byte-equal and the two UI fixture PNGs unchanged; the three examples, `docs/showcase/` and the README's link go (`D-193`).
- **non-goals:** Example 01's rebuild and its own assets folder (01 (a), `D-194`); `02_bench`, which keeps its harness and digests; trimming the scenes' demo-only paths (Q2); pruning the root `assets/` (Q4); a pixel test for the lighting scene, which stays a smoke row; a rehearsal tag, which the owner pushes (Q5); recapturing the UI fixture PNGs, which is the owner's (Q6); any engine crate change.
- **files to touch:** Every step: this plan (status, evidence). Step 1: `docs/plans/1.0/README.md` (owned fields, refreshed when the status turns `in progress`); captures under `target/m71/` and a suite under `perf-runs/`, both ignored by Git. Step 2: `examples/03_fixtures/**` (new, §2.1), `examples/03_scene_state/**`, `examples/04_shader_playground/**` and `examples/05_ui_lab/**` (deleted), `Cargo.toml`, `Cargo.lock`, `scripts/smoke-examples.sh`, `scripts/test-smoke-examples.sh`, `justfile`, `scripts/check-repo.py`, `docs/LLM_INDEX.md`. Step 3: `scripts/release.py`, `scripts/test-release.py`, `.github/workflows/release.yml`. Step 4: `examples/03_fixtures/src/lib.rs`, `examples/03_fixtures/src/lighting.rs` (new), `examples/03_fixtures/tools/README.md`, `scripts/smoke-examples.sh`, `scripts/test-smoke-examples.sh`, `examples/01_platformer/src/{setup,state,systems,extract}.rs`, `examples/01_platformer/src/tests/{presentation,death,main}.rs`, `examples/01_platformer/tools/README.md`. Step 5: `docs/showcase/README.md` and `docs/showcase/lighting_off_vs_on.png` (deleted), `scripts/check-repo.py`, `README.md`, `docs/README.md`, `DESIGN.md`, `docs/assets.md`, `docs/getting-started.md`, `docs/known-issues.md`, `docs/plans/1.0/example-01-design.md`, `templates/basic/tests/game/visual.rs`. Step 6: none. Step 7: `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, `docs/plans/1.0/implementation-plan.md` (the register row), `docs/plans/1.0/w12-template.md`, `docs/plans/1.0/README.md` (owned fields), this plan (moved to `docs/plans/archive/1.0/`).
- **ordered steps:** (1) Baselines. (2) The fixtures crate. (3) Release archives without the crate. (4) The lighting scene; example 01's hook removed. (5) The showcase and the docs. (6) Close-out compare. (7) Release 0.92.
- **done-when:** Every step's evidence row quoted, or the step marked skipped with its reason under §7; workflow §5 closed (§8); the release checks pass after the cut; the preflight prints the command block and the post-merge block.

Candidate Example fixtures ([implementation plan](../../1.0/implementation-plan.md) §4 card, level B), milestone M71, release 0.92. Written 2026-10-20 at `686f7b1`. Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs. Its sources are the card, [w12-template](../../1.0/w12-template.md) step 3 and `D-193`; [example 01's design](../../1.0/example-01-design.md) line 14 says this stop takes 01's lighting fixture.

## Context digest

- **Why now.** On 2026-10-20 `D-193` retired examples 03 (`03_scene_state`), 04 (`04_shader_playground`) and 05 (`05_ui_lab`) as demos. The template's shell (`D-181`, `D-187`) is now the only UI reference. The register puts this stop after the template audit (0.91) and before 01 (a), which rebuilds example 01 from scratch (`D-194`).
- **What must survive the move.** The post-stack, transition and UI-overlay pixel tests, which W4b's freeze leans on. Example 01's M29 lighting smoke row. The UI lab's engine tests that the template's tests don't cover.
- **What it stands on.** The release archives ship every `examples/*` member but this one (`D-072` as `D-193` amends it). Explicit scene files are the asset-coverage exception (`D-046`). The atlas packer (`D-048`) decides whether one merged manifest moves a pixel (A10). The packaged fonts drew the two UI PNGs (`D-116`). The template is the listening host since `D-193` superseded `D-179`'s; the marker lines are already in `DECISIONS.md`.
- **Later stops expect** example 01 to keep no `TUNGSTEN_*` hook but its two display checks (design line 14). They also expect the smoke script, `release.py` and the LLM index to route to the crate, not to three examples.
- **No engine crate changes.** The seam (`D-007`, `D-018`) is untouched: the steps change examples, scripts, the workflow, docs and one comment in the template's tests.

## 1. Audit

Read at `686f7b1`. S = `examples/05_ui_lab/tests/screens.rs`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | Each of the three examples is one binary that runs from the repository root. It loads the root `tungsten.json` and composes the root manifest with its local one; only 04 turns on hot reload, watching both folders. | `examples/03_scene_state/src/main.rs:30-31,72-81`; `examples/04_shader_playground/src/main.rs:58-59,131-159`; `examples/05_ui_lab/src/main.rs:44-58` |
| A2 | Each scene picks its behaviour from its own environment variables, read at startup. 03: `TUNGSTEN_TRANSITION_FIXTURE`. 04: `TUNGSTEN_POST_STACK_FIXTURE`, `_POST_AA_`, `_BLOOM_`, `_GAME_FEEL_`, `_MESH_TRAIL_` and `_MATERIAL_PAIR_FIXTURE`. 05: `TUNGSTEN_UI_LAB_FIXTURE` with ten values, plus `TUNGSTEN_UI_LAB_POST`, `_BOUNDS` and `_MAXIMIZE_WAIT`. With no variable set, each runs interactively. | `examples/03_scene_state/src/main.rs:10-14,49-63`; `examples/04_shader_playground/src/main.rs:11-28,135-216`; `examples/05_ui_lab/src/main.rs:13-29` |
| A3 | The three pixel suites spawn their example's binary through `CARGO_BIN_EXE_<package>` from the workspace root. Each writes frame 5 of 8 at 1280×720 into `std::env::temp_dir()`. Post and transition compare captures of one run with each other. UI compares two captures against fixture PNGs at tolerance 0. | `examples/04_shader_playground/tests/post_regression.rs:14-51`; `examples/03_scene_state/tests/transition_regression.rs:16-47`; `examples/05_ui_lab/tests/ui_regression.rs:27-89` |
| A4 | `just visual` writes 45 PNGs. Post: `tungsten-post-regression-*`, 10. Transition: `tungsten-transition-regression-*`, 5 (`none` and four effects). UI: `tungsten-ui-regression-*`, 18. Bench: `tungsten-visual-regression-*`, 11. Template: `tungsten-template-title.png`, 1. A `TMPDIR` per run collects them for `cmp`. | `post_regression.rs:15,133-250`; `transition_regression.rs:17,58-60`; `ui_regression.rs:54,92-185`; `examples/02_bench/tests/visual_regression.rs:49,78-110`; `templates/basic/tests/game/visual.rs:19` |
| A5 | The post suite's two stock-shader tests stage a mirror of the root that links the example's assets folder by its path. | `post_regression.rs:86-89` |
| A6 | Smoke runs every `example-*` package plainly, then the fixture rows. 13 of the 14 rows `D-193` keeps run on 03, 04 and 05: M26 (2), M27, M28, M30, the M31 mesh trail, four M31 transitions, M56 menus, M57 debug and the frame cap. M29 runs on example 01 with `TUNGSTEN_LIGHTING_FIXTURE=on`. The frame-cap row relies on 03's plain run, about 0.5 s uncapped, against its 900 ms floor. | `scripts/smoke-examples.sh:25-37,92-98,168-250,194-202,272-284` |
| A7 | The smoke script's stub test pins each run's environment line, the plain-run count (`Passed: 4/4`) and the failure cases by package name. | `scripts/test-smoke-examples.sh:30,38,80-126,138-147` |
| A8 | `just visual` runs five suites: 02's, 04's, 03's, 05's and the template's. | `justfile:44-50` |
| A9 | The workspace lists the three examples as members. | `Cargo.toml:16-18` |
| A10 | The atlas packer sorts every sprite of a filter class from all loaded manifests by height, width and ID onto shared pages, with power-of-two sizes. A half-texel inset keeps samples inside each rect. Manifests load before startup, so a merged manifest can move UVs but never a frame number. Replaying the packer on the manifests shows three things. (1) One crate manifest keeps the UI scene's 12 sprites where they are, nearest page 1024×128 before and after. (2) The post and transition scenes' quads do move (page 512→1024), but `quad.png` (16×16) and `emissive_quad.png` (32×32) are each a single opaque white value, so no sample changes. (3) Dropping root `red_square` and `blue_square` would move 11 of the UI scene's 12 sprites. | `crates/tungsten-core/src/assets/atlas.rs:93-98,140-141`; `D-048` choices 1 and 3; `crates/tungsten/src/app.rs:2372-2387` |
| A11 | Root manifest: `red_square`, `blue_square` and `green_circle` have no user in code. `walk_0`…`walk_4` (the only root sprites with normal and emissive maps) and the `walk` animation have no user but `crates/tungsten/tests/asset_smoke.rs`. The fonts, `damage_flash`, the shaders and the two sounds are used by examples 01, 02 and the moving scenes. | `assets/manifest.json`; outside `assets/`, `rg -n 'red_square\|blue_square\|green_circle\|walk_0'` finds only 01's `ex10_player_walk_0` family and engine tests that write temp manifests of their own |
| A12 | `examples/03_scene_state/assets/quad.png` and `examples/04_shader_playground/assets/quad.png` are byte-identical, but one file cannot serve both IDs. The registry maps each source path to one sprite ID; the packer registers in `HashMap` order; the watcher reloads only that ID. With one file, an edit could reload `ex03_quad` and leave the post scene's `ex04_quad` stale. The three local manifests' IDs are disjoint (`ex03_`, `ex04_`, `ex05_` prefixes). Layer 1 requires IDs unique across the root and every `examples/*/assets/manifest.json`. | `cmp` of the two files; `crates/tungsten-core/src/assets/registry.rs:140`; `crates/tungsten/src/asset_loader/atlas.rs:413-415`; `crates/tungsten/src/app.rs:770-773`; `crates/tungsten-core/tests/manifests.rs:15-35,92-117` |
| A13 | Asset coverage reads exactly one `assets/manifest.json` per example folder; every other file needs a reference, a font folder or an `ASSET_EXCEPTIONS` entry. 03's `scene.json` is such an exception. `DOCS` lists `docs/showcase/README.md`, which `check_docs` opens, so deleting it without the entry stops `just repo-check`. | `scripts/check-repo.py:20-35,76-126,271-275` |
| A14 | `release.py` ships every `examples/*` workspace member: `examples()` feeds `shipped_packages` (licenses), the package's required binaries and its runtime assets. The release workflow builds `--workspace --exclude tungsten-crash-probe --bins` and uploads `example-*`. | `scripts/release.py:352-357,600-604,894-905`; `.github/workflows/release.yml:146-155,183-184` |
| A15 | Example 01's lighting fixture spans several pieces. Code: the env read, the `On` branch's lights and the orbit system with its types, and the lit-player branch in its own extract. Tests: one that walks both modes, a death snapshot line and the schedule snapshot. Docs: one acceptance line. `hsv_to_rgb` also serves `gameplay.rs:664`, so it stays. | `examples/01_platformer/src/setup.rs:228,241-249,359-422`; `state.rs:220-257`; `systems.rs:1068-1143`; `extract.rs:378-380,405-427`; `src/tests/presentation.rs:362-434`; `src/tests/death.rs:107`; `src/tests/main.rs:320`; `tools/README.md:26` |
| A16 | `example-01-design.md` line 25 says 01 reads the root `walk_*` sprites. Line 113 says 01's player reads its maps only under the lighting fixture. 01 draws its own `ex10_player_*` sprites, so line 25 is already wrong, and step 4 makes line 113 stale. | `docs/plans/1.0/example-01-design.md:25,113`; `examples/01_platformer/src/level_layout.rs:1604` |
| A17 | S has 19 tests. Six check engine rules the template's tests don't: S:45 (D-147's second text batch), S:84 (a modal takes Escape and the click under it, `D-149`), S:156 (the first key after a pointer press is the game's, `D-149`, `D-162`), S:273 and S:310 (owned roots freeze and fade under a transition, and a popped root goes, `D-163`), and S:535 (the prewarm hands its runs over once, `D-163`). S:527 pins the budget fixture's 100 nodes that S:535 counts on. The other twelve check the lab's own screens or are covered by the template's tests. All seven need only the harness, no manifest, and helpers at S:24-268 and S:518. | S; `templates/basic/tests/game/{settings,keys,saves,fit,sample,shell}.rs` (§2.5); `crates/tungsten/src/testing.rs:18-20` |
| A18 | 05's sprites come from a standard-library generator whose output path is relative to its folder. Its README holds the two UI PNGs' capture commands, which name the binary and paths. | `examples/05_ui_lab/tools/generate.py:7-8,16-17`; `examples/05_ui_lab/tools/README.md:9-19` |
| A19 | Docs that describe the three examples as current. `README.md:26,42-43`; `docs/README.md:20`. `DESIGN.md`: the status line (5), the workspace root (371), the UI fixture and modal (443, 449), damage numbers (505), the Settings window (511), the interactive run and perf subjects (519), the listening host (603). `docs/assets.md:32`; `docs/getting-started.md:170,235`; `docs/known-issues.md:44,64`; `templates/basic/tests/game/visual.rs:1`; `docs/showcase/` (a README and one PNG). LLM index rows 24, 70 and 82–84 name example paths, and `just ctx` resolves every backticked path. The index holds 12,285 of its 12,288 bytes. | the files and lines named; `scripts/check-agent-context.py:25,72-91` |
| A20 | Records left as they are. Decisions name the examples' tests as history (`D-048`, `D-087`, `D-090`, `D-091`, `D-093`, `D-147`, `D-148`, `D-161`–`D-165`, `D-168`, `D-169`, `D-179`, `D-188`); §2.5 records where each test went, as `D-193` asks. CHANGELOG, the released register rows and the workstream files' dated text are history too. Two strings stay on purpose: `crates/tungsten/src/tests/logging.rs:90-91` names `example-03-scene-state` only as a sample binary for a file-name test, and `.claude/settings.json:34-35` holds two allowlist entries for the old packages (follow-up). | the files named; `D-193` consequences |
| A21 | Smoke mode resolves no user folder, so a binary's name cannot change a capture. The release chain has no `just visual`. | `crates/tungsten/src/user_dir.rs:62-70`; [releases](../../../releases.md) step 1 |

## 2. Design

### 2.1 The crate

`examples/03_fixtures`, package `example-03-fixtures`, `publish = false`, workspace version, edition, `rust-version` and lints. Dependencies: `tungsten`, `glam`, `anyhow` and `log`, plus `tungsten` with `testing` as a dev-dependency. It has a library, whose modules the headless tests reach, and a binary. Like `02_bench` it runs from the repository root on the root `tungsten.json` and `input.json` (`D-193`: off the template layout).

| Path | From | Change beyond paths and names |
| --- | --- | --- |
| `src/main.rs` | — | `fn main() -> anyhow::Result<()> { example_03_fixtures::run() }`, keeping the examples' `windows_subsystem` line |
| `src/lib.rs` | — | Crate docs: test-only fixtures, not a reference and never copied by a game (`D-193`); the scenes and their variables (§2.2); run from the repository root. `pub mod post; pub mod transition; pub mod ui;` (step 4 adds `lighting`) and `pub fn run()` |
| `src/post.rs` | 04 `src/main.rs` | `main` becomes `pub fn run`; the manifest and hot-reload folder become `examples/03_fixtures/assets`; the module's docs say what the scene fixes, not what it demos |
| `src/transition/mod.rs`, `states.rs` | 03 `src/main.rs`, `src/states.rs` | `main` becomes `pub fn run`. In `mod.rs`, which keeps `mod states;`, the import `crate::states::…` becomes `self::states::…`; in `states.rs`, `crate::` references to its parent (`QUAD_ID`, `SPRITE_HALF`, `VIEW_CENTER`, the two clocks) become `super::`. `SCENE_PATH` becomes `examples/03_fixtures/assets/transition/scene.json` and the manifest `examples/03_fixtures/assets/manifest.json` |
| `src/ui/mod.rs` | 05 `src/lib.rs` and `src/main.rs` | 05's module list. The binary's body becomes `pub fn run` with `build_paint_path` and `build_menu`, and its `example_05_ui_lab::` imports become `self::`. The lib docs' "a reference … copy the one you need" becomes fixture code that the shell has replaced as the UI reference (`D-193`) |
| `src/ui/{budget,controls,fixtures,hud,labels,menus,screens,theme,widgets}.rs`, `src/ui/inventory/{mod,detail,grid,items}.rs` | 05 `src/` | `crate::` into `crate::ui::`, code and doc links alike |
| `assets/manifest.json` | the three local manifests | One union, IDs unchanged, each entry on a file of its own (A12); 03's and 04's paths gain their scene's folder |
| `assets/transition/{quad.png,scene.json}`, `assets/post/{quad.png,emissive_quad.png,particles/bullet_trail.json}`, `assets/sprites/**`, `assets/ui/themes/ex05_default.json` | 03, 04, 05 `assets/` | Byte copies. 03's and 04's files go in a folder per scene, so the two `quad.png` stay two files. 05's keep their paths, which its generator writes. `scene.json` keeps its `ASSET_EXCEPTIONS` entry at `examples/03_fixtures/assets/transition/scene.json` |
| `tests/post_regression.rs`, `transition_regression.rs`, `ui_regression.rs` | 04, 03, 05 `tests/` | `CARGO_BIN_EXE_example-03-fixtures`; each command sets `TUNGSTEN_FIXTURE` to its scene; the stage links `examples/03_fixtures/assets`; messages name the new binary |
| `tests/screens.rs` | S | The seven tests of Q3 with the helpers they call; `example_05_ui_lab::` becomes `example_03_fixtures::ui::`; its docs list what the seven check |
| `tests/fixtures/ui-visual.png`, `ui-labels.png` | 05 `tests/fixtures/` | Byte copies |
| `tools/generate.py`, `tools/README.md` | 05 `tools/` | The usage lines and capture commands name the new folder, binary and `TUNGSTEN_FIXTURE=ui` |

Files move with plain `mv` or `cp`; no `git mv`, since the release block's `git add -A` records the renames. Folders are deleted with `python3 -B -c 'import shutil; …'`, since `rm -rf` is denied here. Window titles and system names stay as they are; the schedule snapshots name the systems.

### 2.2 One binary, one scene variable

`TUNGSTEN_FIXTURE` picks the scene at startup: `post` (04's), `transition` (03's), `ui` (05's), `lighting` (step 4). Unset means `ui` (Q1); any other value fails with the list of valid values. Each scene keeps reading its own variables under their current names, which decisions, docs and tests cite (A2), and ignores the others'. `02_bench`'s `TUNGSTEN_BENCH` is the precedent: one binary, one selecting variable, a default when unset.

### 2.3 One manifest, frames unchanged

Every scene composes the root manifest with `examples/03_fixtures/assets/manifest.json`. A10 is why this changes no pixel. The UI scene's sprites keep their atlas places, and the quads that move are flat white. Glyphs come from the root fonts, which do not change. The root `assets/` stays as it is (Q4). Layer 1 sees one more example manifest with unique IDs, and asset coverage needs no new rule.

### 2.4 Smoke rows

`fixtures_pkg="example-03-fixtures"` replaces the per-section package variables. The discovery loop's plain run of the crate makes 15 runs with the 14 rows.

| Section | Row | Environment besides the row's frames |
| --- | --- | --- |
| Discovery | plain run | none: the UI scene's interactive run (Q1) |
| M26 | `fixture=empty`, `fixture=all` | `TUNGSTEN_FIXTURE=post TUNGSTEN_POST_STACK_FIXTURE=<fixture>` |
| M27 | `post_stack=empty post_aa=smaa_high` | `post`, `TUNGSTEN_POST_STACK_FIXTURE=empty TUNGSTEN_POST_AA_FIXTURE=smaa_high` |
| M28 | `post_stack=bloom_only bloom_fixture=on` | `post`, `TUNGSTEN_POST_STACK_FIXTURE=bloom_only TUNGSTEN_BLOOM_FIXTURE=on` |
| M29 | `lighting` | `TUNGSTEN_FIXTURE=lighting` from step 4; until then example 01 with `TUNGSTEN_LIGHTING_FIXTURE=on` |
| M30 | `game_feel_fixture=on` | `post`, `TUNGSTEN_GAME_FEEL_FIXTURE=on` |
| M31 | mesh trail | `post`, `TUNGSTEN_RENDER_MSAA=4 TUNGSTEN_RENDER_DEPTH_SORT=gpu_depth`: 04's plain path under MSAA |
| M31 | `transition=<effect>` × 4 | `transition`, 16 frames, `TUNGSTEN_TRANSITION_FIXTURE=<effect>` |
| M56 | `fixture=menus` | `ui`, 150 frames, `TUNGSTEN_UI_LAB_FIXTURE=menus` |
| M57 | `fixture=debug` | `ui`, 120 frames, `TUNGSTEN_UI_LAB_FIXTURE=debug`; the log forbids `Unknown font` |
| Frame cap | `frames=20 frame_rate_cap=20` | `transition`, 20 frames, `TUNGSTEN_DISPLAY_FRAME_RATE_CAP=20`: 03's plain path, HUD on, at least 900 ms |

The stub test echoes `${TUNGSTEN_FIXTURE:+ scene=$TUNGSTEN_FIXTURE}` after `preset=`, ahead of the other suffixes. Its plain-run count becomes `Passed: 3/3`, and its failure cases name `example-03-fixtures`. Step 4 drops the stub's `light=` field, which nothing sets once 01's hook is gone.

### 2.5 Where each test went

| Before (`686f7b1`) | After | Covered elsewhere, for a dropped test |
| --- | --- | --- |
| 04 `tests/post_regression.rs`, 5 tests | `examples/03_fixtures/tests/post_regression.rs`, same names, `TUNGSTEN_FIXTURE=post` | — |
| 03 `tests/transition_regression.rs`, `each_transition_effect_changes_the_frame` | `examples/03_fixtures/tests/transition_regression.rs`, `transition` | — |
| 05 `tests/ui_regression.rs`, 6 tests; `tests/fixtures/ui-{visual,labels}.png` | `examples/03_fixtures/tests/ui_regression.rs` and `tests/fixtures/`, `ui` | — |
| 03 `src/main.rs:366`, `src/states.rs:314`; 04 `src/main.rs:1303` | `src/transition/mod.rs`, `src/transition/states.rs`; `src/post.rs` | — |
| S:45, S:84, S:156, S:273, S:310, S:527, S:535 | `examples/03_fixtures/tests/screens.rs`, same names (Q3) | — |
| S:345 display-mode button | dropped: the lab's own control | `templates/basic/tests/game/settings.rs:216,720`; `crates/tungsten-core/src/tests/settings.rs:108` |
| S:394 UI scale at the pointer's release | dropped: the lab's policy | `templates/basic/tests/game/settings.rs:400` |
| S:423 text scale on a key step | dropped: the template covers it | `templates/basic/tests/game/settings.rs:592`; `crates/tungsten/src/tests/settings.rs:169` |
| S:437 volume to the audio commands | dropped: the template covers it | `templates/basic/tests/game/settings.rs:567`; `crates/tungsten/src/tests/settings.rs:52` |
| S:457 rebind capture | dropped: the lab's policy | `templates/basic/tests/game/keys.rs:85,192`; `crates/tungsten/src/tests/ui_route.rs:679`; `crates/tungsten-core/src/tests/ui/route.rs:1477` |
| S:589 slot grid scrolls | dropped: the lab's inventory | `templates/basic/tests/game/saves.rs:287`; `crates/tungsten-core/src/tests/ui/scroll.rs:412` |
| S:617 use, equip and drop | dropped: the lab's inventory | `crates/tungsten-core/src/tests/ui/route.rs:780`; `crates/tungsten/src/tests/ui_route.rs:377` |
| S:813 centred at three sizes | dropped: the lab's layout | `templates/basic/tests/game/fit.rs:206` |
| S:1026 music fades in | dropped: the listening host is the template's (`D-193`) | `templates/basic/tests/game/sample.rs:847`; `crates/tungsten-core/src/tests/audio.rs:131` |
| S:1036 M crossfades the music | dropped: the lab's policy | `templates/basic/tests/game/sample.rs:874`; `crates/tungsten-core/src/tests/audio.rs:106` |
| S:1060 bus sliders | dropped: the template covers it | `templates/basic/tests/game/settings.rs:567,720`; `crates/tungsten/src/tests/settings.rs:258`; `crates/tungsten-core/src/tests/settings.rs:224` |
| S:1106 Back fits at 1.75 | dropped: the lab's layout | `templates/basic/tests/game/fit.rs:102` |
| 01 `src/tests/presentation.rs:363`, both lighting modes | Same test: the player's unlit path and the ball's lit check stay; the lit-player mode goes with the hook | — |

### 2.6 The lighting scene

`src/lighting.rs`, `pub fn run`: the root config and the two manifests, title "Fixtures — lighting (M29)", no systems of its own. At startup it spawns five lit sprites, `walk_0`…`walk_4` from the root manifest, at 2× in a row through the view's centre (640, 360). Their normal and emissive maps put them on the lit path in the default extract (`crates/tungsten/src/sprite_extract.rs:411-416`). Beside them goes one unlit `ex03_quad`, so the frame keys a lit and an unlit batch. Then the lights, with 01's M29 values (`examples/01_platformer/src/setup.rs:364-412`):

- `AmbientLight(Vec3::splat(0.15))`;
- a warm `Light::point(Vec3::new(1.0, 0.62, 0.35), 352.0)` at intensity 1.0, at the row's left end;
- a cool `Light::point(Vec3::new(0.4, 0.65, 1.0), 352.0)` at intensity 0.9, at its right end;
- `Light::directional(Vec3::splat(0.7), -FRAC_PI_4)` at intensity 0.3.

The lights are static (Q2's rule: the row checks the lit pipeline, the light upload and the lit/unlit batch keys, which three frames cover). Example 01 then drops its hook and everything only the hook uses (A15). Its `Off` branch's ambient and moon become unconditional, and its player keeps the unlit `damage_flash` path it draws today.

### 2.7 Release archives

`release.py` gains `NOT_SHIPPED = frozenset({"example-03-fixtures"})` beside `LAUNCHER`, with a comment citing `D-193`. `examples()` leaves those packages out, and its docstring says so. `shipped_packages`, `package`'s required binaries and its runtime assets follow from that. The workflow adds `--exclude example-03-fixtures` to the build, so the `example-*` upload never sees the binary; its comment says why. A test in `test-release.py` adds the member to its fixture workspace and checks `examples()`, `shipped_packages()` and a packaged archive.

## 3. Steps

### Step 1: Baselines

- **Files:** this plan and `docs/plans/1.0/README.md`'s owned fields, as the run's start sets `status: in progress`. Captures go to `target/m71/` and the suite to `perf-runs/`, both ignored by Git.
- **Change:** On the tree as this plan session left it, `686f7b1` plus this plan and its records:
  1. Check that `df -h .` shows at least 20 GiB free.
  2. `mkdir -p target/m71/visual-before target/m71/fixtures-before && cp examples/05_ui_lab/tests/fixtures/ui-visual.png examples/05_ui_lab/tests/fixtures/ui-labels.png target/m71/fixtures-before/`.
  3. Run `TMPDIR="$PWD/target/m71/visual-before" just visual`.
  4. Run `just perf suite --allow-background --repeat 1` as one blocking foreground command, and record its folder from the `Suite valid:` line as `<before-suite>`. Only its digests are read.
- **Done-when:**
  - `just visual` exits 0. One `load_shaders 'fade'` ERROR line is the post suite's deliberate check.
  - `python3 -B -c 'import sys; from pathlib import Path; n = [p.name for p in Path(sys.argv[1]).glob("tungsten-*.png")]; print(len(n), *(sum(x.startswith(p) for x in n) for p in ("tungsten-post-regression-", "tungsten-transition-regression-", "tungsten-ui-regression-", "tungsten-visual-regression-", "tungsten-template-")))' target/m71/visual-before` prints `45 10 5 18 11 1`.
  - The suite exits 0 and prints `Suite valid:` with eight rows.
- **Moves:** none.

### Step 2: The fixtures crate

- **Files:** `examples/03_fixtures/**` (new, §2.1); `examples/03_scene_state/**`, `examples/04_shader_playground/**` and `examples/05_ui_lab/**` (deleted); `Cargo.toml`, `Cargo.lock`, `scripts/smoke-examples.sh`, `scripts/test-smoke-examples.sh`, `justfile`, `scripts/check-repo.py`, `docs/LLM_INDEX.md`.
- **Change:**
  1. Build the crate as §2.1 lays it out: scenes, manifest, tests, tools.
  2. Delete the three example folders.
  3. Swap the three members for `examples/03_fixtures` in `Cargo.toml`.
  4. Refresh `Cargo.lock` with one cargo command run without `--locked` (`cargo check -p example-03-fixtures`). Every later command uses `--locked`.
  5. Point the smoke rows at the crate (§2.4; M29 stays on example 01) and update the stub test to match.
  6. Point `just visual`'s three middle lines at `-p example-03-fixtures --test post_regression|transition_regression|ui_regression`.
  7. Move the `scene.json` exception to `examples/03_fixtures/assets/transition/scene.json` ("D-046 scene loaded by the fixtures' transition scene").
  8. In the LLM index:
     - row 24's example path becomes `examples/03_fixtures/src/transition/`;
     - row 70 becomes `examples/02_bench/tests/visual_regression.rs`, `examples/03_fixtures/tests/`;
     - rows 82–84 become one row, `| Fixtures crate (`D-193`): scenes by `TUNGSTEN_FIXTURE`, pixel tests, UI sprite generator | `examples/03_fixtures/src/`, `examples/03_fixtures/tests/`, `examples/03_fixtures/tools/README.md` |`.
  9. Run `cargo fmt --all`.
- **Done-when:**
  - `cargo fmt --all -- --check` → exit 0; `cargo clippy --workspace --all-targets --locked -- -D warnings` → exit 0.
  - `cargo test -p example-03-fixtures --locked` → exit 0. `python3 -B -c 'import subprocess; out = subprocess.run(["cargo", "test", "-p", "example-03-fixtures", "--locked", "--", "--list"], capture_output=True, text=True, check=True).stdout; print(sum(line.endswith(": test") for line in out.splitlines()))'` → `22`: 3 unit, 5 post, 1 transition, 6 UI and 7 screens tests.
  - `cargo test -p tungsten-core --test manifests --locked` → exit 0.
  - `python3 -B examples/03_fixtures/tools/generate.py --check` → exit 0.
  - `python3 -B -c 'import subprocess, tomllib; old = tomllib.loads(subprocess.run(["git", "show", "HEAD:Cargo.lock"], capture_output=True, text=True, check=True).stdout); new = tomllib.load(open("Cargo.lock", "rb")); ids = lambda lock: {(p["name"], p["version"]) for p in lock["package"]}; print(sorted(ids(old) - ids(new)), sorted(ids(new) - ids(old)))'` → `[('example-03-scene-state', '0.91.0'), ('example-04-shader-playground', '0.91.0'), ('example-05-ui-lab', '0.91.0')] [('example-03-fixtures', '0.91.0')]`.
  - `rg -n 'example-0[45]-|example-03-scene|example_0[45]_|example_03_scene|0[345]_(scene_state|shader_playground|ui_lab)' Cargo.toml justfile scripts docs/LLM_INDEX.md examples` → no match.
  - `rg -n -i 'a reference for a game|copy the one you need' examples/03_fixtures` → no match.
  - `bash scripts/test-smoke-examples.sh` → `smoke-examples regression tests: OK`; `python3 -B scripts/test-check-repo.py` → `OK`.
  - `just ctx` → exit 0; `just repo-check` → exit 0.
  - `mkdir -p target/m71/visual-after && TMPDIR="$PWD/target/m71/visual-after" just visual` → exit 0. If it stops at a failing suite, run each later line of the recipe by itself with the same `TMPDIR`, so every suite leaves its captures, then apply §7.
  - `python3 -B -c 'import filecmp, sys; from pathlib import Path; b, a = map(Path, sys.argv[1:3]); n = sorted(p.name for p in b.glob("tungsten-*.png")); m = sorted(p.name for p in a.glob("tungsten-*.png")); print(len(n), n == m, [x for x in n if x in m and not filecmp.cmp(b / x, a / x, shallow=False)])' target/m71/visual-before target/m71/visual-after` → `45 True []`, or under Q6 exactly its 15 lab-frame captures.
  - `cmp target/m71/fixtures-before/ui-visual.png examples/03_fixtures/tests/fixtures/ui-visual.png && cmp target/m71/fixtures-before/ui-labels.png examples/03_fixtures/tests/fixtures/ui-labels.png` → exit 0.
  - The frame-cap row still separates capped from uncapped. Run `s=$(date +%s%3N); TUNGSTEN_FIXTURE=transition TUNGSTEN_SMOKE_FRAMES=20 cargo run -p example-03-fixtures --quiet --locked; echo $(( $(date +%s%3N) - s ))` three times → each exits 0 and the fastest prints under `900` (ms). Example 03 took 466–514 ms at `686f7b1`.
  - `just smoke` → exit 0, printing:
    - `Passed: 3/3`, `Template passed: 1/1`, `Matrix passed: 4/4`;
    - `Post-stack passed: 2/2`, `Post-AA passed: 1/1`, `Bloom passed: 1/1`, `Lighting passed: 1/1`;
    - `Game-feel passed: 1/1`, `Mesh/transition passed: 5/5`;
    - `UI screens passed: 1/1`, `Debug views passed: 1/1`;
    - `Benchmarks passed: 15/15`, `Frame cap passed: 1/1`.
- **Moves:** the three example packages leave the workspace, as `D-193` decides. No capture moves.

### Step 3: Release archives without the crate

- **Files:** `scripts/release.py`, `scripts/test-release.py`, `.github/workflows/release.yml`.
- **Change:** §2.7. Then a local packaging rehearsal for Linux, built the way the workflow builds, into `target/` only.
- **Done-when:**
  - `python3 -B scripts/test-release.py` → `OK`, the new test among them; `just script-test` → exit 0 (actionlint and ShellCheck included).
  - `python3 -B -c 'import importlib.util; from pathlib import Path; s = importlib.util.spec_from_file_location("rel", "scripts/release.py"); r = importlib.util.module_from_spec(s); s.loader.exec_module(r); print(r.examples(Path(".")), r.shipped_packages(Path(".")))'` → `[('example-01-platformer', 'examples/01_platformer'), ('example-02-bench', 'examples/02_bench')] ['example-01-platformer', 'example-02-bench', 'tungsten-template-basic', 'tungsten-launcher']`.
  - `rg -c -- '--exclude example-03-fixtures' .github/workflows/release.yml` → `1`.
  - The rehearsal → exit 0:
    ```bash
    RUSTFLAGS="-C target-cpu=x86-64 -C link-arg=-Wl,--build-id" cargo build --release --locked --workspace --exclude tungsten-crash-probe --exclude example-03-fixtures --bins --target x86_64-unknown-linux-gnu --target-dir target/m71/bin/x86-64 && python3 -B scripts/release.py licenses x86_64-unknown-linux-gnu --out target/m71/bin/licenses.json && python3 -B scripts/release.py package v0.0.0-m71 x86_64-unknown-linux-gnu --bin-dir target/m71/bin --out target/m71/dist
    ```
    A folder of its own keeps any other level's leftovers under `target/` out of the archive.
  - `tar -tzf target/m71/dist/tungsten-examples-v0.0.0-m71-x86_64-unknown-linux-gnu.tar.gz | rg -c 'fixtures'` → no match.
  - `tar -tzf target/m71/dist/tungsten-examples-v0.0.0-m71-x86_64-unknown-linux-gnu.tar.gz | rg -c '^tungsten-examples-v0.0.0-m71-x86_64-unknown-linux-gnu/(example-01-platformer|example-02-bench|tungsten-template-basic)$'` → `3`.
  - After the two listings, `python3 -B -c 'import shutil; shutil.rmtree("target/m71/bin")'` frees the release build's space, and `test ! -e target/m71/bin` → exit 0.
- **Moves:** none.

### Step 4: The lighting scene; example 01's hook removed

- **Files:** `examples/03_fixtures/src/lib.rs`, `examples/03_fixtures/src/lighting.rs` (new), `examples/03_fixtures/tools/README.md`, `scripts/smoke-examples.sh`, `scripts/test-smoke-examples.sh`, `examples/01_platformer/src/{setup,state,systems,extract}.rs`, `examples/01_platformer/src/tests/{presentation,death,main}.rs`, `examples/01_platformer/tools/README.md`.
- **Change:**
  1. Record example 01's test count from the listing script with `-p example-01-platformer` before editing.
  2. Add the lighting scene (§2.6), its arm in `run` and its line in the crate docs.
  3. Move the M29 row to the crate with `TUNGSTEN_FIXTURE=lighting`. Its comment describes the crate's scene, and no longer claims example 01's `walk_*` sprites.
  4. In the stub test, update the M29 line and drop the `light=` field.
  5. In example 01, delete:
     - `lighting_fixture_from_env`, the `LightingFixture` resource and the `On` branch;
     - `LightingFixtureMode`, `LightingFixture`, `OrbitLight`, `CycleMode`, `orbit_lights_system` with its schedule entry, and `Vec2X3`;
     - the extract's `lighting_on` read, leaving the player batch unlit with its material and override block as today;
     - the lit-player mode of `presentation.rs:363` (the unlit assertions and the ball's lit check stay), the `LightingFixture` line of `death.rs:107`, and `orbit_lights_system` from `tests/main.rs:320`'s schedule;
     - the acceptance line `tools/README.md:26`.
  6. Run `cargo fmt --all`.
- **Done-when:**
  - `rg -n 'TUNGSTEN_LIGHTING_FIXTURE|LightingFixture|OrbitLight|CycleMode|orbit_lights|Vec2X3' examples/01_platformer scripts` → no match.
  - `cargo fmt --all -- --check` → exit 0; `cargo clippy --workspace --all-targets --locked -- -D warnings` → exit 0.
  - `cargo test -p example-01-platformer --locked` → exit 0, its listing count equal to the count before the step.
  - `cargo test -p example-03-fixtures --locked` → exit 0, listing `22`.
  - `bash scripts/test-smoke-examples.sh` → `OK`.
  - `just smoke` → exit 0 with every line step 2 lists, `Lighting passed: 1/1` now on `example-03-fixtures`. The lighting row's log (`<log dir>/example-03-fixtures-lighting.log`, folder printed at the top) holds no `WARN` or `ERROR` line.
- **Moves:** example 01 loses `TUNGSTEN_LIGHTING_FIXTURE` on purpose. Its default frame keeps today's `Off` path.

### Step 5: The showcase and the docs

- **Files:** `docs/showcase/README.md` and `docs/showcase/lighting_off_vs_on.png` (deleted), `scripts/check-repo.py`, `README.md`, `docs/README.md`, `DESIGN.md`, `docs/assets.md`, `docs/getting-started.md`, `docs/known-issues.md`, `docs/plans/1.0/example-01-design.md`, `templates/basic/tests/game/visual.rs`.
- **Change:**
  1. Showcase:
     - delete `docs/showcase/` and its `DOCS` entry;
     - `README.md` loses the showcase row and the two quick-start lines for 03 and 04; the crate is not a demo, so it gets no line;
     - `docs/README.md` loses its showcase row.
  2. `DESIGN.md`:
     - the status line drops the UI lab clause, and the template's shell becomes the only UI reference;
     - line 371: the root is the working folder for `02_bench` and the fixtures crate (`D-193`), and for example 01 until 01 (a) (`D-194`);
     - lines 443, 449, 505, 511 and 519 name the fixtures crate's UI scene in place of example 05 or the UI lab;
     - line 603: the template is the listening host. Its sample's run music fades under pause and game over, and its settings set the two buses (`D-193`).
  3. Other docs:
     - `docs/assets.md:32` names the fixtures crate's `scene.json`;
     - `docs/getting-started.md:170,235` drop "as example 03 does" and "as example 04, the shader playground, does";
     - `docs/known-issues.md:44` names the UI scene and `src/ui/widgets.rs`'s `value_text`; line 64 names the transition scene.
  4. `example-01-design.md`, by step 4's outcome:
     - line 25 drops "the `walk_*` sprites" either way (A16: 01 never reads them);
     - if step 4 landed, line 113 says the player never reads its maps since this stop removed the lighting fixture;
     - if step 4 was skipped, line 113 stands, and line 14 says the fixtures stop left 01's lighting fixture and its M29 row to (a), whose first step moves them to the crate's lighting scene (§2.6) before the rebuild deletes 01's code.
  5. `templates/basic/tests/game/visual.rs:1` reads "The title's pixels against a fixture PNG. Gate:".
- **Done-when:**
  - `test ! -e docs/showcase` → exit 0.
  - `rg -n -i -P 'examples? 0[345]\b|example-0[45]|example-03-scene|0[345]_(scene_state|shader_playground|ui_lab)|ui[- ]lab|shader playground|showcase|example_05' README.md DESIGN.md AGENTS.md docs/README.md docs/LLM_INDEX.md docs/assets.md docs/getting-started.md docs/known-issues.md docs/validation.md docs/releases.md docs/perf templates scripts justfile Cargo.toml .github examples/01_platformer examples/02_bench tools` → no match. The same pattern matches 56 lines at `686f7b1`.
  - `just ctx` → exit 0; `just repo-check` → exit 0; `python3 -B scripts/test-check-repo.py` → `OK`.
  - `cargo test -p tungsten-cli --locked` → exit 0, since `tungsten new` embeds the template.
- **Moves:** none.

### Step 6: Close-out compare

- **Files:** none. Captures go to `target/m71/visual-final/` and the suite to `perf-runs/`.
- **Change:** `TMPDIR="$PWD/target/m71/visual-final" just visual`, then `just perf suite --allow-background --repeat 1 --compare <before-suite>` as one blocking foreground command. Only its digest line is read; verdicts under `--allow-background` are not.
- **Done-when:**
  - `just visual` → exit 0, with step 2's rule for a suite that stops it. Step 2's compare script with `target/m71/visual-before target/m71/visual-final` → `45 True []`.
  - The suite → exit 0, `Suite valid:` and `First-run digests match in 8 of 8 rows (information only).`
  - `git diff --quiet HEAD -- crates examples/02_bench assets tungsten.json input.json && echo unchanged` → `unchanged`.
- **Moves:** none.

### Step 7: Release 0.92

- **Files:** `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row (`docs/plans/1.0/implementation-plan.md` §10), `docs/plans/1.0/w12-template.md` (step 3's Example fixtures bullet gains "Landed in 0.92 (M71…)"), the README's four owned Now fields (`roadmap.py summary`), this plan (moved to `docs/plans/archive/1.0/`).
- **Change:** First close workflow §5 (§8): the invariants of §6, one `CHANGELOG.md` line, the register row cut as `v0.92.0`, the w12 bullet, this step's evidence row and `status: done`. The CHANGELOG line, the register row and the w12 bullet each say whether step 4 moved 01's lighting fixture (§7). Then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.92.0` once, and every release check once after it. If step 6 did not pass, its visual compare and digest suite run here too. Then the preflight with `--message`.
- **Done-when:** [releases](../../../releases.md) step 1's chain → every check passes, quoted in the report; the plan is archived before the cut. `just release-preflight 0.92.0 --repo JosephGibson/Tungsten --message 'Update 0.92: M71 example fixtures crate'` → passes and prints the command block and the post-merge block.
- **Ends with:** the changed-file list, the command block, the post-merge block, then the plan prompt for the next register row, 01 (a) rebuild, from [tungsten-next's prompts](../../../../.claude/skills/tungsten-next/prompts.md) (`plan` row): new session, auto mode, Fable 5.1, xhigh. `Use the tungsten-milestone skill. Write the milestone plan for 01 (a) rebuild: read docs/plans/1.0/workflow.md, its card in docs/plans/1.0/implementation-plan.md and docs/plans/1.0/example-01-design.md and docs/plans/1.0/w12-template.md and docs/plans/1.0/w01-ui-text-suite.md §11; audit only the code it touches; name it docs/plans/1.0/phase6-milestone-72-example-01-rebuild.md; critique it last as the skill's step 7 says; end with the run prompt. Don't commit: leave the changes in the tree for the release commit.`

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | What does the crate's plain run draw? | The UI scene's interactive run, 05's plain run. 03's plain path keeps a row in the frame cap (`TUNGSTEN_FIXTURE=transition`) and 04's in the mesh trail (`post` under MSAA 4), so each old plain run still has one. |
| Q2 | Should the scenes' demo-only paths go: 04's cycle keys, the lab's interactive run with its music and controls window, the `display` and budget fixtures? | No: they move whole. Every frame keeps its code path, the plain run covers the interactive lab, and the budget fixtures stay the w01 §10 perf subjects (`DESIGN.md:519`). The crate's docs say it is not a reference. Trimming waits for a reason and a plan. |
| Q3 | Which of S's 19 tests move? | The six engine tests the template's don't cover (A17) and S:527, whose 100 nodes S:535's 48 runs assume. The other twelve drop; §2.5 names the tests that cover each. The six also have crate tests of the same rules; they move because `D-193`'s rule selects them and they drive those rules through a real screen. |
| Q4 | Should the root `assets/` lose what no user needs? | Not in this stop. `red_square`, `blue_square` and `green_circle` have no user. But the packer sorts the two squares among the UI scene's sprites, and dropping them moves 11 of its 12 on the shared page (A10), which the two tolerance-0 PNGs would have to absorb. `walk_*` and `walk` now feed the lighting scene. Follow-up. |
| Q5 | Who runs the rehearsal prerelease the card asks for? | Pushing a tag is the owner's (`D-079`, `D-105`). The session's evidence is step 3: the script's test, its listing of the shipped packages on this tree, and a Linux archive packaged from release builds without the crate. The report marks the CI rehearsal as not run. The owner can check the published 0.92.0 archives after the merge with the `verify` prompt. |
| Q6 | What if the crate draws a different UI frame? Not expected (A10). | Step 2 stands under two conditions. First, the UI suite fails only `ui_visual_matches_fixture` and `ui_labels_match_fixture`. Second, every capture that differs from step 1's shows the lab frame, 15 in all: `tungsten-ui-regression-default`, `-labels` and `-default-vs-empty`, and the twelve path captures, `default-` or `labels-` with `aa-off`, `smaa-high` or `vignette` and `-blit` or `-direct`. The other 30 must be `cmp`-equal: post, transition, bench, the template, and the UI suite's `empty`, `hidden` and `empty-vs-default`. The evidence then quotes both `compare_png` reports and the list of changed captures. The session writes candidate recaptures to its scratchpad, never over the fixtures. The hand-off gives the owner `tools/README.md`'s capture commands to run before the release commit (`D-193`), and the report says `just visual` is red on those two tests until then. Any other difference restores step 2. |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`).

## 5. Decisions expected

None. `D-193` and `D-194` settle the scope, `D-072` and `D-179` already carry their marker lines, and nothing here sets a new rule. If a step finds a choice that needs an entry, the next free ID is 195, written with [tungsten-decision](../../../../.claude/skills/tungsten-decision/SKILL.md) and checked again when the step runs.

## 6. Invariants

- **May move:** none. Example 01 loses its lighting hook on purpose (step 4); its default frame is unchanged.
- **Must not move:**
  - The determinism and pinned containment hashes: no engine crate changes (step 6's `git diff --quiet`).
  - The row digests: 8 of 8 match (step 6).
  - `gpu-visual.png` and the bench's 11 captures.
  - The post (10) and transition (5) captures, byte-equal.
  - The two UI fixture PNGs and the UI suite's 18 captures.
  - The template's title capture.
  - The root `assets/`, `tungsten.json` and `input.json` (step 6).

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with steps that don't depend on it. Step 2 is the candidate. Steps 3–6 depend on it, and the release requires steps 1, 2, 3, 5 and 6.
- Step 1's `just visual` fails on the untouched tree, or its counts differ from `45 10 5 18 11 1`: stop before step 2 and report. There is nothing to compare against.
- Step 2 or step 3 is skipped: stop the run before the release, which waits for a release session after the owner has seen why ([workflow](../../1.0/workflow.md) §3). Without step 3 the archives would carry the crate.
- In step 2 or step 6, only the two UI fixture comparisons fail: Q6.
- Step 4 is skipped: example 01 keeps its hook and the M29 row stays on example 01, and the release goes ahead. Step 5 writes the skip into 01 (a)'s design (line 14). The release's records say the lighting fixture did not move: the CHANGELOG line, the register row and the w12 bullet. The follow-up in this plan names 01 (a) as its home.
- A digest differs in step 6: stop before the release; the report names the row.
- A change is needed outside §files to touch, an engine crate change above all: restore that step and report it.
- `pgrep -af 'scripts/[b]ench.py'` shows another session's capture: wait, and write nothing to the tree until it ends. The bracket keeps `pgrep` from matching its own command line.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 2, 3, 4, 5, 6, release | Visual compares in steps 2 and 6; smoke in steps 2 and 4; script tests in steps 2, 3 and 5; `ctx` and `repo-check` in steps 2 and 5; digests in steps 1 and 6. The release chain runs once after the cut, with step 6's checks if step 6 did not pass. No physics or perf row is judged: no engine code changes. |
| 3 Invariants | 6 | §6 |
| 4 Decisions | n/a | None expected (§5) |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | n/a | No engine crate changes; the release chain's `just api-check` confirms the snapshots |
| 6 Template and guide | 5 | One comment line in the template's tests; nothing changes how a game is written. The guide loses two pointers to deleted examples |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 2 | Rows 24, 70 and 82–84; the index shrinks |
| 8 Design | 5 | The `DESIGN.md` sentences of A19 |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 5, release | Known issues lines 44 and 64 in step 5. No backlog row: nothing in the card is cut. No gap rows. The register row, the w12 bullet and the owned fields (`roadmap.py summary`) in the release. |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

Round 1: the `critique` skill with `--repo`, `gpt-6-astra`, 2026-10-20, on the plan at `686f7b1` plus this session's records. Effort xhigh, context 872,000, output cap 4,000; 390.3 s; tokens in 1,360,044 (1,239,552 cached), out 10,367 (5,436 reasoning); 37 commands, 425.9 KB output (largest 28.1 KB); 5h window 0% → 31%, week 2% → 7%. Three findings, all accepted. Each changed a done-when check or a step's change, so round 2 runs (skill step 7).

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [MEDIUM] Sharing `quad.png` makes post-scene hot reload nondeterministic: one sprite ID per source path, registered in `HashMap` order, and the watcher reloads only that ID | `crates/tungsten-core/src/assets/registry.rs:140` (`path_to_sprite_id.insert`, last write wins); `crates/tungsten/src/asset_loader/atlas.rs:413-415` ("the packer registers in `HashMap` order"); `crates/tungsten/src/app.rs:770-773` (`sprite_name_for_path` → one `reload_sprite`) | accepted | A12 records it. §2.1: 03's and 04's files go under `assets/transition/` and `assets/post/`, each ID on its own file. Step 2 item 7's exception path follows. Frames are unaffected: two entries pack the same either way (A10) |
| 2 | [MEDIUM] Q6 cannot accept the UI frame change it is meant for: a real change also moves other UI captures of the same frame, and a failing UI suite stops `just visual` before the template line | `examples/05_ui_lab/tests/ui_regression.rs:79,93-112` (`default` captured with identical settings by the fixture test and the path tests); `justfile:49-50` (a failing line ends the recipe) | accepted | Q6 names the 15 lab-frame captures allowed to change and the 30 that must stay `cmp`-equal. Steps 2 and 6 run each later recipe line alone when a suite stops `just visual`. Step 2's compare check points to Q6 |
| 3 | [MEDIUM] The lighting fallback still applies documentation that assumes the move succeeded | Step 5 item 4 rewrote `example-01-design.md:113` unconditionally; `examples/01_platformer/src/extract.rs:378-380,409` keep the lit path while the hook stays | accepted | Step 5 item 4 branches on step 4's outcome. If step 4 was skipped, line 14 hands the fixture and its M29 row to (a)'s first step. §7's step-4 bullet, step 7's records and a conditional follow-up say the same |

Also changed with round 1, the plan's own correction: step 3's rehearsal builds into `target/m71/bin`, so other levels' leftovers under `target/` cannot reach the archive, and deletes that folder after its two listings.

Round 2: the same skill and model, 2026-10-20, on the revised plan with §9 and the evidence log cut by `sed` (49,341 characters, under the soft limit, so no `--force`) and a `--focus-file` naming round 1's changes. Effort xhigh, context 872,000, output cap 4,000; 230.1 s; tokens in 726,800 (635,520 cached), out 6,667 (3,401 reasoning); 23 commands, 278.9 KB output (largest 20.7 KB); 5h window 31% → 50%, week 7% → 10%. The critic found that round 1's asset, capture and skip-path fixes held, and raised one finding, accepted. No finding is left open, so no open question comes of it.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] The transition import rewrite points at a nonexistent module: changing `crate::states` to `super::states` in `transition/mod.rs` looks for `states` at the crate root | `examples/03_scene_state/src/main.rs:18` (`mod states;`), `:28` (`use crate::states::{…}`); `states.rs:28,74,112` (its `crate::` references to the parent) | accepted | §2.1's transition row: `self::states` in `mod.rs`, `super::` only in `states.rs`. The `ui/mod.rs` row now says its `example_05_ui_lab::` imports become `self::`, the same shape |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 Baselines | 2026-10-20 | pass | `df -h .`: 164G free. `just visual` exit 0; one `load_shaders 'fade'` ERROR line (the post suite's check); five `test result: ok` lines (4, 5, 1, 6, 1 passed). The count script printed `45 10 5 18 11 1`. Suite exit 0, `Suite valid: perf-runs/20261020T145742Z-suite/suite.json and README.md`, eight rows valid (`<before-suite>`) | `target/m71/visual-before/`, `target/m71/fixtures-before/`, `target/m71/visual-before.log`, `target/m71/suite-before.log`, `perf-runs/20261020T145742Z-suite/` |
| 2 The fixtures crate | 2026-10-20 | pass | 41 files copied byte for byte, then the scenes, manifest, tests and tools rewritten as §2.1 says; the three folders deleted. `cargo fmt --all -- --check` exit 0; clippy `--workspace --all-targets --locked -D warnings` exit 0. `cargo test -p example-03-fixtures --locked` exit 0, `22 passed (7 suites)`; the listing script printed `22`. `manifests` exit 0, `5 passed`. `generate.py --check` exit 0 (`up to date`). The `Cargo.lock` script printed `[('example-03-scene-state', '0.91.0'), ('example-04-shader-playground', '0.91.0'), ('example-05-ui-lab', '0.91.0')] [('example-03-fixtures', '0.91.0')]`. Both `rg` checks: no match (exit 1). `test-smoke-examples.sh`: `smoke-examples regression tests: OK`; `test-check-repo.py`: `Ran 52 tests … OK`. `just ctx` exit 0; `just repo-check` exit 0. `just visual` exit 0, five `test result: ok` lines, one `load_shaders 'fade'` ERROR; the compare printed `45 True []`; both fixture `cmp` exit 0. Frame cap uncapped: 2,254 (the first run built the binary), 420 and 416 ms, each exit 0. `just smoke` exit 0: `Passed: 3/3`, `Template passed: 1/1`, `Matrix passed: 4/4`, `Post-stack passed: 2/2`, `Post-AA passed: 1/1`, `Bloom passed: 1/1`, `Lighting passed: 1/1`, `Game-feel passed: 1/1`, `Mesh/transition passed: 5/5`, `UI screens passed: 1/1`, `Debug views passed: 1/1`, `Benchmarks passed: 15/15`, `Frame cap passed: 1/1`. LLM index 12,285 → 11,997 bytes | `examples/03_fixtures/`, `target/m71/visual-after/`, `target/m71/visual-after.log`, `target/m71/smoke-step2.log`, `target/m71/clippy-step2.log` |
| 3 Release archives | 2026-10-20 | pass | `test-release.py`: `Ran 45 tests … OK`; `-v Release.test_the_fixtures_crate_is_not_shipped`: `Ran 1 test … OK`. `just script-test` exit 0 (ShellCheck and actionlint included). The listing printed `[('example-01-platformer', 'examples/01_platformer'), ('example-02-bench', 'examples/02_bench')] ['example-01-platformer', 'example-02-bench', 'tungsten-template-basic', 'tungsten-launcher']`. `rg -c -- '--exclude example-03-fixtures'` → `1`. The rehearsal chain exit 0 (build, licenses, package). Player archive: `rg -c 'fixtures'` no match (exit 1), the three binaries' line count `3`, 784 entries; the debug archive holds no `fixtures` entry either. `target/m71/bin` (1.7 GB) deleted; `test ! -e` exit 0. The CI rehearsal prerelease is not run (Q5) | `target/m71/dist/`, `target/m71/rehearsal-*.log`, `target/m71/script-test-step3.log` |
| 4 Lighting scene, 01's hook removed | 2026-10-20 | pass, one departure | Example 01 listed `106` tests before the step. `src/lighting.rs` as §2.6 says; the M29 row and its stub line moved, `light=` dropped from the stub (36 expected lines). Departure: A15 missed that `extract_balls` also read `lighting_on` (`extract.rs:465,543,597` at `686f7b1`), so balls drew lit under the hook. They now take the default `Off` path, unlit, with a batch key of atlas and filter; the ball part of `presentation.rs:363` now asserts `!lit` where it asserted `lit` under the hook, and its player part asserts the unlit path with its material. The `rg` check: no match (exit 1). `cargo fmt --all -- --check` exit 0; clippy `--workspace --all-targets --locked -D warnings` exit 0. `cargo test -p example-01-platformer --locked` exit 0, `106 passed`, listing `106` (equal). `cargo test -p example-03-fixtures --locked` exit 0, `22 passed`, listing `22`. `test-smoke-examples.sh`: `OK`. `just smoke` exit 0 with every line of step 2's list, `Lighting passed: 1/1` under `M29 lighting fixture matrix (pkg: example-03-fixtures, scene=lighting)`; `/tmp/tmp.0kC4om6nOC/example-03-fixtures-lighting.log` (54 lines) holds no `WARN` or `ERROR` line. A frame-5 capture showed the five sprites lit warm to cool and the quad unlit | `examples/03_fixtures/src/lighting.rs`, `target/m71/smoke-step4.log`, `target/m71/clippy-step4.log` |
| 5 Showcase and docs | 2026-10-20 | pass | `docs/showcase/` deleted with its `DOCS` entry, the README's row and two quick-start lines, and `docs/README.md`'s row. `DESIGN.md` lines 5, 371, 443, 449, 505, 511, 519 and 603 as the step says; line 603's claim checked against `templates/basic/src/sample/mod.rs:6-8,161-176`. `docs/assets.md:32`, `docs/getting-started.md:170,235`, `docs/known-issues.md:44,64`, `templates/basic/tests/game/visual.rs:1`. Step 4 landed, so `example-01-design.md` line 25 drops the `walk_*` sprites, line 113 says the player never reads its maps, and line 14 stands. `test ! -e docs/showcase` exit 0. The step's `rg` pattern: no match (exit 1). `just ctx` exit 0; `just repo-check` exit 0; `test-check-repo.py`: `OK`. `cargo test -p tungsten-cli --locked` exit 0, `48 passed`. Outside the pattern's paths, only history names the old packages (`CHANGELOG.md`, `DECISIONS.md`, `w01-ui-text-suite.md:481,558`, `w15-authoring-api.md:6`), plus A20's two strings | `target/m71/ctx-step5.log`, `target/m71/repo-check-step5.log` |
| 6 Close-out compare | 2026-10-20 | pass | `just visual` exit 0, five `test result: ok` lines, one `load_shaders 'fade'` ERROR; the compare with `visual-before` printed `45 True []`. No capture, build or other session in the tree before the sitting. Suite exit 0, `Suite valid: perf-runs/20261020T151246Z-suite/suite.json and README.md`, `First-run digests match in 8 of 8 rows (information only).`; verdicts under `--allow-background` not read. `git diff --quiet HEAD -- crates examples/02_bench assets tungsten.json input.json && echo unchanged` → `unchanged` | `target/m71/visual-final/`, `target/m71/suite-final.log`, `perf-runs/20261020T151246Z-suite/`, `perf-runs/20261020T151331Z-compare-suite/` |
| 7 Release 0.92 | 2026-10-20 | closed; checks after the cut | Workflow §5 closed (§8). §6's invariants hold: no engine crate, `02_bench`, root asset or config change (step 6's `git diff --quiet`), digests 8 of 8, `gpu-visual.png` and the bench's 11 captures, the post (10), transition (5) and UI (18) captures and the template's title capture byte-equal (steps 2 and 6), both UI fixture PNGs `cmp`-equal (step 2). One `CHANGELOG.md` entry under `[Unreleased]`; the register row cut as `v0.92.0`; the w12 bullet; each says step 4 moved 01's lighting fixture. The README's owned fields come from `just release-cut`. The plan is archived before the cut, so the release checks' output is in the session's report | `CHANGELOG.md`, `docs/plans/1.0/implementation-plan.md`, `docs/plans/1.0/w12-template.md` |

## Follow-ups

- **Root sprites no one uses** (Q4): `red_square`, `blue_square` and `green_circle` in `assets/manifest.json`. Home: the Phase 6 QA pass, with a recapture of the two UI fixture PNGs planned in the same step.
- **Two stale allowlist entries** in `.claude/settings.json:34-35` (`cargo run -p example-03-scene-state` and `-p example-04-shader-playground`). They are inert. Home: the owner, since an unattended run should not edit the agent's own settings.
- **Only if step 4 is skipped:** example 01's lighting fixture and its M29 smoke row. Not needed: step 4 landed.

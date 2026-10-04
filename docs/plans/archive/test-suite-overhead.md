# Test-suite overhead (0.44)

- **status:** done
- **goal:** Cut what `cargo test --workspace` and the core-edit `just check` loop cost: the route test replays known launches instead of searching, redundant unit tests go or become compile-time checks, dev builds stop linking hundreds of megabytes of dependency DWARF, the GPU pixel tests stop building on every run if that saves a second, and CI builds the benchmarks in the dev profile.
- **non-goals:** Merging integration-test binaries (immutable decisions and recipes cite their paths); nextest as the default runner; `doctest = false`; splitting repo-check's `-p tungsten-core` features; bulk-deleting other small unit tests; any change to engine runtime code, the release or bench profiles, or perf captures; a 1.0 milestone number or register row.
- **files to touch:** Step 2: `examples/01_platformer/src/tests/level.rs`, `examples/01_platformer/src/tests/main.rs`. Step 3: `crates/tungsten-render/src/tests/smaa.rs`, `crates/tungsten-render/src/post/smaa_luts.rs`, `crates/tungsten-render/src/tests/smaa_luts.rs` (deleted), `crates/tungsten-render/src/tests/bloom.rs`, `crates/tungsten-render/src/tests/debug_line.rs`, `crates/tungsten-render/src/tests/lighting.rs`, `crates/tungsten-render/src/lighting.rs`, `crates/tungsten-render/src/tests/lit_sprite.rs`, `crates/tungsten-core/src/tween.rs`, `crates/tungsten-core/src/tests/lighting.rs`, `crates/tungsten-core/src/tests/components.rs`, `crates/tungsten-core/src/tests/assets/scene.rs`, `crates/tungsten/src/tests/asset_loader.rs`, `crates/tungsten/src/tests/hot_reload.rs`, `examples/01_platformer/src/tests/camera.rs`. Step 4: `Cargo.toml`, `DECISIONS.md`, `docs/DECISION_INDEX.md`. Step 5, only if adopted: `examples/02_bench/Cargo.toml`, `examples/03_scene_state/Cargo.toml`, `examples/04_shader_playground/Cargo.toml`, `justfile`, `AGENTS.md`, `docs/LLM_INDEX.md`, `examples/02_bench/tests/fixtures/README.md`, `docs/perf/benchmarks.md`. Step 6: `.github/workflows/ci.yml`, `justfile`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/agent-setup.md`. Step 7: `justfile`, `examples/01_platformer/tools/README.md`. Step 8: `CHANGELOG.md`. Every step: this plan. Evidence and scripts go to `perf-runs/20261004-test-suite-overhead/` and scratch trees to `target/tso/`, both git-ignored.
- **ordered steps:** (1) Baseline on a scratch copy. (2) Route test replays a launch table. (3) Test hygiene. (4) Dev-profile debuginfo. (5) GPU pixel tests behind a `visual` feature, if it saves 1 s per core edit. (6) CI bench build in the dev profile. (7) `just level-check`. (8) Close-out. (9) Release 0.44, not run in the execution session.
- **done-when:** Steps 1–8 each have an evidence row quoting their done-when checks, or are marked skipped with the failing output and their files restored; `just check` passes with the count the landed steps imply (§6); step 8's numbers compare with step 1's; step 9 releases 0.44 per [releases](../../releases.md).

Standalone plan for release 0.44, not a 1.0 candidate. Written 2026-10-04 on branch `0.44` at `9f10746` (the 0.43 squash, equal to `origin/main`). It follows the [plan rules](../README.md) and the plan-session shape in [workflow](../1.0/workflow.md) §2; its execution follows workflow §2's unattended rules as this plan states them in §7.

## Context digest

`cargo test --workspace` took 11.7 s on 2026-10-04, and 10.5 s of it is one test: example 01's `authored_routes_and_recovery_shelves_traverse_with_real_physics`, which searches launch points for 45 platform pairs (457 attempts, 39,343 simulated frames at 0.23 ms each; every attempt builds a fresh `App` and re-parses the 572 KB `level.tmj`, 1.7 s in all). All 45 winning launches jump and land in 15–57 ticks, so a table of known launches replays the same coverage in about 45 short attempts. `D-096` set `tungsten-core` to opt-level 1 for this test and cites its name, as do `CHANGELOG.md` and the archived M32 plan, so the name stays.

The rest of the overhead is linking. Test binaries that link the umbrella crate are 360–400 MB, 99% dependency DWARF. On a scratch copy a test build after a real `tungsten-core` edit took 33.8 s, and the same touch-rebuild ranged 5.3–70.9 s because link writes stall the SSD; with `debug = false` for dependencies it took 4.7 s with 62–100 MB binaries and a 5 GB target directory instead of 11 GB, and `split-debuginfo = "unpacked"` took 3.3–6.4 s. These are the owner's single measurements; steps 1, 4, 5 and 8 re-measure everything a done-when depends on, with repeats, because one run proves nothing on this machine. The three GPU pixel tests build and link on every run only to return early without `TUNGSTEN_VISUAL_REGRESSION`. CI takes about 5 min, 148–193 s of it `just bench-build` in the bench profile (thin LTO, one codegen unit), although `just lint` already type-checks the benchmarks and no library code depends on `debug_assertions`.

Decisions: `D-041` (profiles; dev clause amended by `D-096`), `D-070` (CI), `D-096`, `D-110` (headless harness the route test runs on).

## 1. Audit

Read at `9f10746`. Paths are from the repository root.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | The route test searches per distinct pair: start points every half tile, nearest the target's midpoint first, times three goals, times jump then no jump; each attempt builds an `App` through `platformer_harness` and calls `seed_level`, which parses `level.tmj` (572,457 B), then runs 4 settle frames and up to 150 steered frames | `examples/01_platformer/src/tests/level.rs:3-88`, `:90-109`; `examples/01_platformer/src/tests/main.rs:74-75`, `:132-141` |
| A1b | The settle check tests grounding and height but not that the player stands on the source platform; the search's candidates lie on it by construction. 8 of the 45 pairs join platforms on the same row, e.g. `lower_a` (24–32) and `lower_b` (34–38) at row 36, so a replayed start on the destination would pass the settle check and land at tick 4 | `examples/01_platformer/src/tests/level.rs:16-18`, `:35-40`; `examples/01_platformer/src/level_layout.rs:2013-2024` |
| A2 | `seed_level` has eight callers that must not change | `examples/01_platformer/src/tests/ball_pit.rs:383`, `main.rs:128`, `level.rs:27`, `presentation.rs:52`, `:141`, `player.rs:188`, `:252` |
| A3 | `area_lut_byte_length` and `search_lut_byte_length` repeat the two tests in `tests/smaa_luts.rs`; deleting them leaves `use crate::post::smaa_luts;` unused | `crates/tungsten-render/src/tests/smaa.rs:2`, `:63-71`; `crates/tungsten-render/src/tests/smaa_luts.rs:3-13` |
| A4 | `AREA_TEX_BYTES` and `SEARCH_TEX_BYTES` are consts, the accessors are not `const fn`; the test module holds only the two length tests, two assertions each; the upload functions assert the same at run time | `crates/tungsten-render/src/post/smaa_luts.rs:11-32`, `:40-44`, `:88-92`, `:130-132` |
| A5 | `preset_ubo_size_is_256` repeats a const assert that already exists | `crates/tungsten-render/src/tests/smaa.rs:46-49`; `crates/tungsten-render/src/post/smaa.rs:125` |
| A6 | `light_ubo_byte_size_is_544` asserts `LightUbo::byte_size()` (not `const`, returns `size_of::<Self>()`, no other caller) and `size_of::<GpuLight>() == 32`; the module already const-asserts `LIT_LIGHT_CAP == 16` | `crates/tungsten-render/src/tests/lighting.rs:5-9`; `crates/tungsten-render/src/lighting.rs:32`, `:37-49`, `:62-65` |
| A7 | `uniform_override_block_payload_is_256_bytes` checks `to_bytes().len()`, which is 256 by type (`[u8; 256]`); the contract is the block's size, which `copy_from_slice` needs; its import is used only there | `crates/tungsten-render/src/tests/bloom.rs:4`, `:8-12`; `crates/tungsten-core/src/tween.rs:250-259`, `:286-291` |
| A8 | `bloom_shader_ids_are_stable` builds a struct from literals and reads them back; `ShaderAssetId` is imported only for it | `crates/tungsten-render/src/tests/bloom.rs:2`, `:48-63` |
| A9 | `debug_line_instance_is_pod` checks `bytes_of(&inst).len() == size_of`, which the layout test pins and `Pod` guarantees at compile time | `crates/tungsten-render/src/tests/debug_line.rs:3-7`, `:9-20` |
| A10 | `debounce_constant_is_50ms` restates the constant; deleting it leaves `DEBOUNCE_MS` unused in the import | `crates/tungsten/src/tests/hot_reload.rs:6`, `:25-28`; `crates/tungsten/src/hot_reload.rs:9` |
| A11 | `tag_new_stores_name` restates a constructor that other tests exercise | `crates/tungsten-core/src/tests/components.rs:32-36`; `crates/tungsten-core/src/tests/inspect.rs:6`; `crates/tungsten/src/tests/inspector.rs:283` |
| A12 | The umbrella's `shader_registry_allocate_reverse_lookup_roundtrips` repeats core's `path_reverse_lookup_round_trips`, which also checks `path_for_id` | `crates/tungsten/src/tests/asset_loader.rs:391-408`; `crates/tungsten-core/src/tests/assets/shader.rs:20-29` |
| A13 | `shared_camera_tracks_player` and `camera_clamped_at_right_boundary` are covered by the resize-and-zoom test (follow and both-axis clamp over three sizes and three zooms); the names they import stay used in other test files | `examples/01_platformer/src/tests/camera.rs:3-26`, `:28-54`, `:66-101` |
| A14 | `light_cap_is_sixteen` pins 16 but not the shader's array length | `crates/tungsten-core/src/tests/lighting.rs:9-12`; `assets/shaders/lit_sprite.wgsl:36` |
| A15 | `lit_sprite_shader_name_constant` restates the three literals; `assets/manifest.json`'s `shaders` object has all three keys, and `ResolvedManifest::load` is public with a `shaders` map | `crates/tungsten-render/src/tests/lit_sprite.rs:5-10`; `crates/tungsten-render/src/lit_sprite.rs:9-13`; `crates/tungsten-core/src/assets/manifest.rs:174`, `:244` |
| A16 | Three scene tests write fixed `tungsten-scene-*` temp dirs, which concurrent runs (scratch copies, other sessions) share; manifest.rs names its dirs by PID and an atomic counter | `crates/tungsten-core/src/tests/assets/scene.rs:141-200` (dirs at `:143`, `:164`, `:184`); `crates/tungsten-core/src/tests/assets/manifest.rs:519-528` |
| A17 | The dev profile sets no `debug` or `split-debuginfo`, so every crate carries full debuginfo; the pixel-test binaries alone are 377–378 MB each | `Cargo.toml:62-74`; `target/debug/deps/{visual,post,transition}_regression-*` |
| A18 | The pixel tests (2 + 1 + 5 tests) return early without `TUNGSTEN_VISUAL_REGRESSION` but build on every `cargo test --workspace`; the examples have no `[features]` or `[[test]]`; five places spell their command; `just lint` is `--all-targets` with no features | `examples/02_bench/tests/visual_regression.rs:55-58`; `examples/03_scene_state/tests/transition_regression.rs:51-53`; `examples/04_shader_playground/tests/post_regression.rs:126-128`; `justfile:21-22`, `:44-48`; `AGENTS.md:36`; `docs/LLM_INDEX.md:60`; `examples/02_bench/tests/fixtures/README.md:44`; `docs/perf/benchmarks.md:28` |
| A19 | CI and `just ci` run `just bench-build` (`cargo bench --workspace --no-run --locked`, the bench profile inheriting release's thin LTO and one codegen unit); `D-070` and the agent setup list it; `debug_assertions` appears only in two physics test files' ignore attributes | `.github/workflows/ci.yml:88-89`; `justfile:36-38`, `:112-113`; `Cargo.toml:76-91`; `DECISIONS.md:487-492`; `docs/agent-setup.md:58`; `crates/tungsten-core/tests/physics_determinism.rs:68`, `crates/tungsten-core/tests/physics_containment.rs:37` |
| A20 | `D-041` has no marker line; `D-096`'s amendment of its dev clause lives in the index row | `DECISIONS.md:192-194`; `docs/DECISION_INDEX.md:71` |
| A21 | The level generator's check and unittests have no recipe; they need Pillow (12.3.0 installed locally) and `rustfmt` | `examples/01_platformer/tools/README.md:3-5`, `:13-15`, `:26`; `examples/01_platformer/tools/placement.py:2` |
| A22 | The suite now: `cargo test --workspace --locked -q` gives 27 `test result:` lines, all `ok`, 941 passed, 0 failed, 5 ignored, 10.98 s warm in the main tree; `cargo nextest list` gives 24 suites and 946 cases (tungsten-core 474, tungsten 212, tungsten-render 108, example-01-platformer 69, pixel suites 2 + 1 + 5) | run at `9f10746` |

## 2. Measurement method

Steps 1, 4, 5 and 8 measure the same way, with `perf-runs/20261004-test-suite-overhead/measure.py`, which writes one JSON line per run to that folder.

- **Scratch tree.** `target/tso/<name>/` holds the files `git ls-files --cached --others --exclude-standard` lists that exist, copied with `tar` (rsync is missing), and builds into its own `CARGO_TARGET_DIR=target/tso/<name>-target`. A step deletes its trees when its numbers are recorded.
- **Cold `just check`.** The first run in a tree, timed for information. It must pass. Target size is `du -sb` of the target directory right after it.
- **Suite wall.** `cargo test --workspace --locked -q` with nothing changed.
- **Core-edit test build.** Append `/// Timing probe <n>.` and `pub fn tso_probe_<n>() {}` to the tree's `crates/tungsten-core/src/lib.rs`, run `sync`, then time `cargo test --workspace --locked --no-run`.
- **`just check` loop.** The same edit and `sync`, then time `just check` in the tree.
- **Repeats.** One untimed warm-up, then five timed runs (three for the `just check` loop in steps 1 and 8). Configurations compared in one step run in interleaved rounds, one run of each per round, in one sitting. Report min, median and max.
- **Hygiene.** Before each timed run the script records whether `pgrep -x nxcodec.bin` finds the remote-desktop encoder, whether a `cargo` or `rustc` process outside the run exists, and whether `pgrep -af scripts/bench.py` shows a capture. Nothing else runs during a sitting, and each sitting is one blocking command.

## 3. Steps

### Step 1: Baseline

- **Files:** none in the tree; the evidence folder and `target/tso/s1-base/`.
- **Change:** none. On a scratch tree of `9f10746`: a cold `just check`; target size and the sizes of the umbrella-linking test binaries; suite wall ×5; `cargo nextest run -p example-01-platformer --locked -E 'test(authored_routes)'` ×3; core-edit test build 1+5; `just check` loop 1+3; `cargo nextest list --workspace --locked` saved for steps 2, 3 and 5; `just bench-build` once and `cargo bench --workspace --no-run --profile dev --locked` once after it, for step 6.
- **Done-when:** the cold `just check` passes with 941 passed and 5 ignored; every timing above is in the log with its hygiene fields; the evidence row quotes min, median and max per metric.
- **Moves:** nothing.

### Step 2: Route test replays a launch table

- **Files:** `examples/01_platformer/src/tests/level.rs`, `examples/01_platformer/src/tests/main.rs`.
- **Change:**
  1. `main.rs`: split `seed_level` into `level_map()`, which parses `level.tmj`, and `seed_level_map(world, map)`; `seed_level` calls both, so its callers don't change.
  2. `level.rs`: `route_jump_possible` becomes `launch_lands(map, from, to, start, goal, jump, dt) -> bool`, one attempt with the old loop body unchanged (input, physics, the grounded settle check, the hazard and kill-plane exits, the landing condition), which first rejects a start outside `from`'s span (A1b; every search candidate is inside it, so the search's results don't change), and `search_launch(...) -> Option<(f32, f32, bool)>`, which tries candidates in the old order and returns the first that lands.
  3. `const LAUNCHES: &[(&str, &str, f32, f32, bool)]` (from, to, start and goal in tiles, jump): the launch `search_launch` finds first for each of the 45 distinct pairs, generated once by running the search in a scratch copy and printed with `{:?}`, so each `f32` round-trips exactly.
  4. The test keeps its name. It parses the map once, replays each distinct pair's row, and runs `search_launch` only for a pair whose row is missing or does not land. It collects those pairs and fails once, printing for each the replacement row in table syntax, or `"{route}: {from} -> {to} is not traversable"` when the search finds nothing.
- **Done-when:**
  - `cargo nextest run -p example-01-platformer --locked -E 'test(authored_routes)'`, three runs → each `1 passed`, with a reported duration under 1.500 s.
  - `rg -c '^\s*\("[a-z0-9_]+", "[a-z0-9_]+", -?[0-9.]+, -?[0-9.]+, (true|false)\),$' examples/01_platformer/src/tests/level.rs` → 45.
  - In a scratch copy, two mutations one at a time: the `lower_a` → `lower_b` row's start moved onto `lower_b` (36.0), and one row deleted. Each makes the test fail, name that pair and print a replacement row.
  - `cargo nextest list -p example-01-platformer --locked` → the same 69 names as step 1's list.
  - `git diff --stat -- examples/01_platformer` → only the two files; `git diff -U0 -- examples/01_platformer/src/tests/main.rs | rg -c '^[-+].*(#\[test\]|assert)'` → 0.
  - `just check` passes with 941 passed and 5 ignored.
- **Moves:** nothing; the same pairs are traversed with real physics.

### Step 3: Test hygiene

- **Files:** the step 3 list in the header.
- **Change:**
  - Delete `area_lut_byte_length`, `search_lut_byte_length`, the umbrella's `shader_registry_allocate_reverse_lookup_roundtrips`, `bloom_shader_ids_are_stable`, `debug_line_instance_is_pod`, `debounce_constant_is_50ms`, `tag_new_stores_name`, and the platformer's `shared_camera_tracks_player` and `camera_clamped_at_right_boundary`, with the imports this leaves unused (A3, A7, A8, A10).
  - Replace with `const _: () = assert!(...)` beside the type or constant: `preset_ubo_size_is_256` (the assert exists at `post/smaa.rs:125`, so only the test goes); `light_ubo_byte_size_is_544` as `size_of::<GpuLight>() == 32` and `size_of::<LightUbo>() == 544` in `render/lighting.rs`; `uniform_override_block_payload_is_256_bytes` as `size_of::<UniformOverrideBlock>() == 256` in `core/tween.rs`; the two `smaa_luts.rs` tests as four asserts in `smaa_luts.rs` (`AREA_TEX_BYTES.len() == AREA_TEX_LEN`, `AREA_TEX_LEN == 160 * 560 * 2` and the same pair for search), which empties its test module, so the `#[path]` module and `tests/smaa_luts.rs` go.
  - Strengthen, keeping the existing assertions: `light_cap_is_sixteen` also parses `N` from `array<GpuLight, N>` in `assets/shaders/lit_sprite.wgsl` (through `include_str!` and `CARGO_MANIFEST_DIR`, as `tests/input/action_map.rs` reads `input.json`) and asserts it equals `LIGHT_CAP`; `lit_sprite_shader_name_constant` also loads `assets/manifest.json` with `ResolvedManifest::load` and asserts each of the three names is a `shaders` key.
  - The three scene tests take their directory from a `tempdir()` helper copied from `manifest.rs`'s: `tungsten_scene_<pid>_<n>` under `std::env::temp_dir()`, an `AtomicU32` counter, removed before use.
- **Done-when:**
  - `cargo test --workspace --locked -q` → 27 `test result:` lines, all `ok`, 927 passed, 0 failed, 5 ignored.
  - `cargo nextest list --workspace --locked` → tungsten-render 99, tungsten 210, tungsten-core 473, example-01-platformer 69 → 67; every other suite as in step 1.
  - `rg -n 'fn (area_lut_byte_length|search_lut_byte_length|shader_registry_allocate_reverse_lookup_roundtrips|bloom_shader_ids_are_stable|debug_line_instance_is_pod|debounce_constant_is_50ms|tag_new_stores_name|shared_camera_tracks_player|camera_clamped_at_right_boundary|preset_ubo_size_is_256|light_ubo_byte_size_is_544|uniform_override_block_payload_is_256_bytes|area_bin_byte_length_matches_format|search_bin_byte_length_matches_format)\b' crates examples` → no match.
  - `rg -c 'const _: \(\) = assert!' crates/tungsten-render/src/post/smaa_luts.rs crates/tungsten-render/src/lighting.rs crates/tungsten-core/src/tween.rs` → 4, 3 and 1.
  - Mutations in a scratch copy, one at a time: `array<GpuLight, 15>` in the shader → `light_cap_is_sixteen` fails; the `rim_light` key renamed in `assets/manifest.json` → `lit_sprite_shader_name_constant` fails; `count_pad: [u32; 8]` in `LightUbo` → `cargo check -p tungsten-render` fails on the assert; `_reserved: [[u32; 4]; 11]` in `UniformOverrideBlock` → `cargo check -p tungsten-core` fails on the assert; `search.bin` truncated by one byte → `cargo check -p tungsten-render` fails on the assert.
  - `rg -n 'tungsten-scene-' crates` → no match; the three scene tests pass.
  - `just check` passes.
- **Moves:** the test count, 941 → 927.

### Step 4: Dev-profile debuginfo

- **Files:** `Cargo.toml` (the dev profile and its comment), `DECISIONS.md`, `docs/DECISION_INDEX.md`.
- **Change:** four scratch trees of the current tree, one per option: O0 as is; O1 `debug = false` under `[profile.dev.package."*"]`; O2 `debug = "line-tables-only"` there; O3 `split-debuginfo = "unpacked"` under `[profile.dev]`. In each: a cold `just check`, then target size and the umbrella lib test binary's size. Then one warm-up and five interleaved rounds of the core-edit test build. Then a backtrace probe in each tree: a `#[inline(never)]` recursive `pub fn` in core's `lib.rs` that panics at depth 0, called from a test appended to the umbrella's `lib.rs`, run with `RUST_BACKTRACE=1 cargo test -p tungsten --lib tso_probe`; it passes when the backtrace has `at …crates/tungsten-core/src/lib.rs:<line>` and `at …crates/tungsten/src/lib.rs:<line>` frames (the panic message's location does not count). Apply the option with the lowest median among O1–O3 that pass the probe, update the profile comment, and write `D-111` with the tungsten-decision skill: an entry amending `D-041`'s dev clause, a marker line under `D-041`'s heading, its index row and a note on `D-041`'s row.
- **Done-when (an option passes the probe):**
  - The evidence row lists per option: the cold `just check` result, target size, binary size, the five timings' min, median and max, the hygiene fields, and the probe result.
  - The chosen option is the lowest median among those that pass the probe.
  - In the main tree, `just check` passes with step 3's count.
  - `rg -no '.{0,60}D-111.{0,60}' DECISIONS.md docs/DECISION_INDEX.md` → the heading, `D-041`'s marker line, the `D-111` row and the note on `D-041`'s row; `just repo-check` passes.
- **Done-when (no option passes):** the step is recorded as skipped with each option's numbers and probe output; `Cargo.toml`, `DECISIONS.md` and the index are unchanged.
- **Moves:** debuginfo and size of dev and test binaries; no invariant.

### Step 5: GPU pixel tests behind a `visual` feature

- **Files:** none unless adopted; then the step 5 list in the header.
- **Change:** two scratch trees of the tree after step 4: A as is; B with `[features] visual = []` and a `[[test]]` entry with `required-features = ["visual"]` for each pixel test in the three examples, plus the `justfile` change below, so its `just check` lints them as A's does. A cold `just check` in each, then one warm-up and five interleaved rounds of the core-edit `just check` loop, then the same for the core-edit test build. Adopt only if A's median `just check` loop minus B's is at least 1.0 s. If adopted: apply B's manifest changes; `just visual` passes `--features visual`; `just lint` adds `--features example-02-bench/visual,example-03-scene-state/visual,example-04-shader-playground/visual` (an empty feature: no dependency's features change); the visual command in `AGENTS.md`, `docs/LLM_INDEX.md`, the fixtures README and `docs/perf/benchmarks.md` says so; then `just visual`. Otherwise record the numbers and change nothing.
- **Done-when (adopted):** both metrics' min, median and max per tree are in the evidence row; `cargo test --workspace --locked -q` → 24 result lines, 8 fewer passed than before the step, 5 ignored; `just lint` passes, and the same invocation with `--message-format json` lists compiler artifacts for `visual_regression`, `transition_regression` and `post_regression`; `just visual` passes; `just ctx` passes.
- **Done-when (not adopted):** both metrics are in the evidence row and `git diff --stat` is unchanged by the step.
- **Moves:** if adopted, the default run loses the 8 pixel tests and 3 result lines (927 → 919 and 27 → 24 after step 3).

### Step 6: CI bench build in the dev profile

- **Files:** `.github/workflows/ci.yml`, `justfile`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/agent-setup.md`.
- **Change:** if `cargo bench --workspace --no-run --profile dev --locked` builds, ci.yml's "Build benchmarks" step runs it in place of `just bench-build`, and the `ci` recipe runs it as its body with `deps ctx repo-check script-test` after `&&`, so CI's order holds; otherwise both drop the bench build. `just bench-build` stays for local bench-profile builds. Write `D-112` amending `D-070` (marker line, index rows) with the tungsten-decision skill, and update the agent setup's `just ci` row.
- **Done-when (it builds):**
  - `cargo bench --workspace --no-run --profile dev --locked` exits 0, and its output has six `Executable benches/` lines (`ecs_bench`, `physics_bench`, `action_map_bench`, `tween_tick`, `render_bench`, `particle_tick`).
  - `rg -n 'bench-build' .github/workflows/ci.yml` → no match; `just --dry-run ci` prints the check commands, then the dev bench build, then deps, ctx, repo-check and script-test.
- **Done-when (it does not build):** its failing output is quoted; `rg -n 'cargo bench|bench-build' .github/workflows/ci.yml` → no match; `just --dry-run ci` prints no bench command.
- **Done-when (both):** `just script-test` passes (actionlint reads ci.yml); `D-112` has its heading, `D-070`'s marker line and both rows, and `just repo-check` passes.
- **Moves:** nothing. CI's wall time is checked after the owner's next push (follow-up).

### Step 7: `just level-check`

- **Files:** `justfile`, `examples/01_platformer/tools/README.md`.
- **Change:** a `level-check` recipe running `python3 -B examples/01_platformer/tools/generate.py --check` and `python3 -B -m unittest discover -s examples/01_platformer/tools -p 'test_*.py'`, commented as local-only because they need Pillow; it stays out of `ci`, `script-test`, `repo-check` and ci.yml. The README's command list and its note on `just script-test` mention it.
- **Done-when:** `just level-check` exits 0, with the check's success output and unittest's `OK`; `git status --porcelain` is identical before and after it; `rg -n 'level-check' .github/workflows/ci.yml` → no match; `just --dry-run ci 2>&1 | rg -c 'generate.py|unittest'` → 0.
- **Moves:** nothing.

### Step 8: Close-out

- **Files:** `CHANGELOG.md`, this plan.
- **Change:** run the gates; measure a scratch tree of the final tree as step 1 did; add one `[Unreleased]` line; have a fresh-context subagent review the whole diff against this plan (correctness, a test lost by accident, docs that disagree); fix confirmed findings in scope and record the rest as follow-ups.
- **Done-when:**
  - `just check` passes with the count §6 derives from the steps that landed, and 5 ignored.
  - `just ci`'s parts in order, with `cargo deny --frozen check` (the cached advisory database, no fetch) in place of `just deps` → each exits 0.
  - `just smoke` passes, and `just visual` passes.
  - `just ctx` and `just repo-check` pass.
  - The final tree's suite wall, core-edit test build, `just check` loop and target size sit beside step 1's in the evidence row, and each final median is below step 1's; one that is not is reported as a regression in the evidence row and the follow-ups, not restored.
  - `git diff -U0 CHANGELOG.md | rg -c '^\+- '` → 1.
  - The review's findings are recorded with verdicts; the plan stays `in progress` with step 9 pending.
- **Moves:** nothing.

### Step 9: Release 0.44

- **Files:** `CHANGELOG.md`, `Cargo.toml`, `Cargo.lock`, the status lines the cut moves, this plan (moved to `docs/plans/archive/`).
- **Change:** in a release session: [tungsten-finalize](../../../.claude/skills/tungsten-finalize/SKILL.md), then [tungsten-release](../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../releases.md)); follow-ups move to [known issues](../../known-issues.md) before the plan is archived; the cut once, every release check once after it, then the preflight with `--message 'Update 0.44: test-suite overhead (D-111, D-112)'`. Gates by change: `just check`, `just smoke`, `just script-test` (justfile and ci.yml), `just visual` (dev profile, step 5), `just api-check`; no perf rows and no `just physics-release`, since no runtime or physics code changes.
- **Done-when:** per [releases](../../releases.md): every check passes after the cut, and the preflight prints the command block and the post-merge block.
- **Not run in the execution session.** The morning prompt is `Prepare and release 0.44.`

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Shape of the launch table? | A `const` tuple slice in `level.rs`, one row per distinct pair, holding the first launch the old search order finds |
| Q2 | Stale or missing rows: fail at the first, or collect? | Collect every such pair, search each, and fail once with all replacement rows |
| Q3 | Fail on rows that match no route pair? | No: step 2 lists no such assertion; a stale row is dead data (follow-up) |
| Q4 | Where do the const asserts live? | Beside the type or constant, as `post/smaa.rs:125` and `lighting.rs:32` do; the emptied smaa_luts test module and file go |
| Q5 | Keep the old assertions in the two strengthened tests? | Yes, and add the new checks |
| Q6 | Option definitions and the selection rule in step 4? | O0–O3 as step 4 lists them; lowest median core-edit test build among options passing the backtrace probe |
| Q7 | Repeats? | One warm-up and five timed runs per configuration (three for the `just check` loop), interleaved in one sitting, `sync` before each |
| Q8 | Step 5's threshold? | Adopt when A's median core-edit `just check` loop minus B's is at least 1.0 s over the five rounds, with B's lint recipe in place; the test-build numbers are reported beside it |
| Q9 | Step 5 docs beyond the four the task names? | Also the fixtures README and `docs/perf/benchmarks.md`, which spell the command; the test files' comments stay, since the variable still gates them |
| Q10 | Where does the dev bench build go in `just ci`? | The recipe's body, with the later recipes after `&&`, so the order matches CI |
| Q11 | List `just level-check` in `docs/agent-setup.md` too? | No; the README note is the home the task names (follow-up) |
| Q12 | `just ci` without the network in step 8? | Its recipes in order, with `cargo deny --frozen check` for `just deps` |
| Q13 | Run `just visual` in step 8 even if step 5 is skipped? | Yes: step 4 changes how the pixel tests' binaries build |

Approved 2026-10-04 by the owner in advance, with the stated defaults.

## 5. Decisions expected

The next free IDs on 2026-10-04 (`DECISIONS.md` ends at `D-110`), checked again when each step runs; a skipped step's ID passes to the next decision.

- `D-111` (step 4): dev builds use the chosen debuginfo option. Amends `D-041`'s dev-profile clause, as `D-096` did; `D-096` stands.
- `D-112` (step 6): CI builds the benchmarks in the dev profile, or not at all if that fails to build. Amends `D-070`'s list of CI steps; the rest of `D-070` stands.

## 6. Invariants

- **May move:** the default run's test count and result lines, from 941 passed, 5 ignored and 27 lines: step 3 removes 14 tests, step 5 if adopted 8 tests and 3 lines, so 927, 919, 941 or 933 passed with 5 ignored, by which steps landed; dev and test binaries' debuginfo and sizes; target directory size.
- **Must not move:** the determinism hash, the pinned containment hash, the row digests, `gpu-visual.png`, the post and transition regressions (no runtime code changes; release and bench profiles untouched); every distinct route pair traversed with real physics; the route test's name; library behavior (const asserts only add compile-time checks).

## 7. Stop conditions

- A done-when check fails: restore the step's files from the copies taken before it with plain `cp`, confirm with `cmp`, record the step as skipped with the output, and continue with the steps that don't depend on it. Steps 2, 3, 6 and 7 are independent; step 5 measures on whatever profile the tree has; step 8 runs on whatever landed.
- A step needs a file outside its list: skip it the same way and record a follow-up. The imports a deletion leaves unused in the same file are in scope.
- Free disk under 40 GB before a scratch build: skip the measurement and record it.
- `nxcodec.bin` present before a timed run: record it, wait for it to exit, and rerun the sitting; a sitting it disturbed is not used.
- A `cargo` or `rustc` process from outside the sitting, or a capture (`pgrep -af scripts/bench.py`): discard the sitting and rerun it once; if it recurs, record the flag with the numbers.
- A test the step does not touch fails, or an invariant moves: restore and skip.
- Never change a test assertion beyond what a step lists. Never commit, push, tag or cut; step 9 waits for the owner.

## 8. Critique

Reviewer: the user-level `critique` skill with `--repo` (gpt-6.1-sol through the Codex CLI, another model family), 2026-10-04, round 1, on the draft of this plan uncommitted on `9f10746`. Footer: model gpt-6.1-sol, effort xhigh, mode repo, 369.3 s, tokens in 366,078 (cached 284,160), out 9,060 (reasoning 6,409). Each finding was checked against the cited code or plan text before it changed anything. Round 1 changed done-when checks, for which workflow §2 would run a second round; the owner's prompt asks for one critique, so none ran.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [MEDIUM] A cached launch can certify traversal from the wrong platform: the settle check verifies grounding and height but not that the start lies on the source platform, and `lower_a` and `lower_b` share a row | `examples/01_platformer/src/tests/level.rs:16-18`, `:35-40`; `level_layout.rs:2013-2024`; a parse of `ROUTES` and `PLATFORMS`: 8 of the 45 pairs share a row | accepted | A1b added; step 2's `launch_lands` rejects a start outside `from`'s span; step 2's mutation check now moves `lower_a` → `lower_b`'s start onto `lower_b` and deletes a row |
| 2 | [MEDIUM] The fallback paths cannot satisfy their acceptance checks: step 8 requires 927/919 even if step 3 is skipped, and step 6 requires the dev bench build to succeed even on its drop branch | This plan's header, steps 6 and 8, §7 | accepted | The header, step 8 and §6 derive the count from the steps that landed (927, 919, 941 or 933); step 6 has a done-when per branch; step 4 has one for no passing option; step 5's counts are relative |
| 3 | [MEDIUM] Step 5's comparison does not measure the `just check` loop: clippy runs first over every target and the proposed lint flags keep the pixel tests in it; step 8 reports the loop without a threshold | `justfile:22`, `:29`; step 5 | accepted | Step 5's B tree carries the lint change, and adoption is decided on the core-edit `just check` loop (Q8), with the test build reported beside it. The suspicion that the flags make clippy and the tests rebuild each other is not carried: `visual = []` enables no dependency feature, and check and build artifacts are separate either way; the loop measurement will show any cost. Step 8 now requires each final median below step 1's and reports any that is not |

## 9. Review

Step 8's fresh-context review: a general-purpose subagent of the same model family, read-only, 2026-10-04, on the uncommitted tree after step 7 plus the `CHANGELOG.md` line. It ran `just check` (27 lines, 927 passed, 5 ignored) and compared nextest lists (946 → 932 cases, exactly the 14 the plan lists; the rest of `level.rs` byte-identical; no assertion changed beyond the plan). Each finding was checked before anything changed.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| R1 | [medium] Step 4's deviation rests on a false premise: Cargo omits `split-debuginfo` for a target that lacks the value, so O3 does not break Windows dev builds and the rule's pick stands | A scratch crate with `[profile.dev] split-debuginfo = "unpacked"` built for `x86_64-pc-windows-msvc`: rustc got `-C debuginfo=2` and no `-C split-debuginfo`, no warning | accepted | O3 applied; `D-111`, its marker, both rows, the `Cargo.toml` comment, the `CHANGELOG.md` line and step 4's row rewritten; `just check` and `just repo-check` rerun; step 5 re-measured on O3 |
| R2 | [low] A stale row's message read like a missing row's, and a replacement appended below the stale row would keep failing, since the lookup takes the first | `level.rs` lookup | accepted | The message says "replace its LAUNCHES row with" or "add a LAUNCHES row"; both printed in a scratch copy |
| R3 | [low] Step 7 named the README's command block too, which lacked the recipe | `examples/01_platformer/tools/README.md:13-23` | accepted | `just level-check` added to the block |
| R4 | [low] Q3 and Q11 promise follow-ups the list lacked | §4, Follow-ups | accepted | Both listed, with R6–R9 |
| R5 | [low] Wording: "all 24 test binaries" (23 depend on core; the launcher has none), "380-455 MB" (378), "size checks beside their types" (one assert already existed; the LUT asserts sit beside constants) | `cargo nextest list`; step 1's sizes | accepted | Fixed in the rewritten `D-111`, the `Cargo.toml` comment and the `CHANGELOG.md` line |
| R6 | [low] The scene tests now leave three temp directories per run, as manifest.rs's helper does (1,610 left in `/tmp`) | Step 3 copies manifest.rs's helper, as it lists | not changed | Follow-up |
| R7 | [low, outside the plan] `LightUbo::byte_size()` lost its only caller and is public API | `api/tungsten-render.txt` | not changed | Follow-up |
| R8 | [low, outside the plan] The deleted bloom test's comment was the only note that ids 4..=7 must follow sprite (0) and SMAA (1..=3) for `Renderer::reload_shader` routing | `crates/tungsten-render/src/renderer.rs:338-342`; `post/bloom.rs:19-20` names the seeding but not the order | not changed: `renderer.rs` is outside the plan's files | Follow-up |
| R9 | [low, before this plan] `D-041` has no `D-096` marker line | `DECISIONS.md:192-194` | not changed | Follow-up |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-04 | done | Scratch tree of `9f10746` (plus this plan). Cold `just check` 166.8 s: 27 lines, 941 passed, 5 ignored; target 10.65 GB after it. Min/median/max: suite wall 10.89/10.92/11.09 s (5 runs); route test under nextest 9.66/9.68/9.68 s (3); core-edit test build 7.19/18.39/27.37 s (5); core-edit `just check` loop 19.75/19.83/19.95 s (3). Umbrella-linking test binaries 378–455 MB. `just bench-build` 129.1 s (bench profile, release dependencies cold); dev-profile bench build 4.3 s after the test build; both list 6 benches. Every run: nxcodec absent, no foreign build or capture. Scratch rustflags carry `target-cpu=native` twice (cargo merges the enclosing repo's `.cargo/config.toml`), the same in every scratch tree | `perf-runs/20261004-test-suite-overhead/`: `runs.jsonl`, `logs/`, `nextest-list-s1.json`, `step1.sh`, `step1b.sh` |
| 2 | 2026-10-04 | done | Table generated in scratch `s2` with an empty table: the full search printed 45 rows in 8.31 s (9.68 s before; parsing the map once), all with `jump: true`, none "not traversable". Route test under nextest in the tree 0.508/0.497/0.496 s, each `1 passed` (from 9.68 s). Row regex → 45. Mutations in `s2`: `lower_a` → `lower_b` start 36.0 → exit 101 printing `("lower_a", "lower_b", 31.85, 34.75, true),`; the `wet_shelf` row deleted → exit 101 printing its row; control, the same start without the span check → exit 0, the false pass critique finding 1 predicted. Platformer names: 69, identical to step 1's. Diff: `level.rs` and `main.rs` only; 0 test or assert lines in `main.rs`'s; the other `level.rs` tests byte-identical. `just check` → 27 lines, 941 passed, 5 ignored | `logs/step2-*.log`, `launches-generated.txt`, `step2-mutations.sh`, `platformer-names-s{1,2}.txt` |
| 3 | 2026-10-04 | done | `just check` → 27 lines, all `ok`, 927 passed, 0 failed, 5 ignored. nextest suites: tungsten-render 108 → 99, tungsten 212 → 210, tungsten-core 474 → 473, example-01-platformer 69 → 67, the rest unchanged (946 → 932 cases). The 14 names: no match. Const asserts: `smaa_luts.rs` 4, `lighting.rs` 3, `tween.rs` 1. `tungsten-scene-`: no match; `process::id` once in scene's tests. Mutations in `s2`, each restored and confirmed with `cmp`: `array<GpuLight, 15>` → `light_cap_is_sixteen` fails (`left: 15`); the `rim_light` key renamed → `lit_sprite_shader_name_constant` fails with its message; `count_pad: [u32; 8]`, `_reserved: [[u32; 4]; 11]` and `search.bin` one byte short → `cargo check` fails with exactly one error, the new assert. The two size mutations also widened their `Default` literals (and `pack_lights`' for `count_pad`), which the plan's wording left out: without them the type error, not the assert, would fail first | `logs/step3-*.log`, `step3-edit.py`, `step3-mutations.sh`, `nextest-list-s3.json` |
| 4 | 2026-10-04 | done: O3, after a withdrawn deviation | Scratch trees of the tree after step 3, one per option; cold `just check` passed in each (27 lines, 927 passed, 5 ignored): O0 156.7 s, O1 120.3 s, O2 129.9 s, O3 158.0 s. Target after it: 10.65, 4.43, 6.20, 5.19 GB; umbrella lib test binary 391, 78, 156, 86 MB. Core-edit test build, 5 interleaved rounds after a warm-up, min/median/max: O0 13.74/18.10/31.23 s; O1 5.20/5.24/6.54 s; O2 5.75/6.14/9.39 s; O3 4.52/4.77/5.95 s. No run flagged nxcodec, a foreign build or a capture. Probe: every option resolved all workspace frames to file:line (core `crates/tungsten-core/src/lib.rs:91:9`, `:93:5` ×2; umbrella `./src/lib.rs:61:20`, cwd-relative because libtest runs in the crate directory, which the plan's `crates/tungsten/src/lib.rs` pattern missed). The rule picks O3. **Deviation, withdrawn:** the run first applied O1 instead, because `rustc --print split-debuginfo --target x86_64-pc-windows-msvc` lists only `packed` and rustc 1.98.1, given the flag directly, stops with "`-Csplit-debuginfo=unpacked` is unstable on this platform". The step 8 review (R1) showed Cargo never passes it there: a scratch crate with this profile, built for that target, got `-C debuginfo=2` and no `-C split-debuginfo`, so the premise was false. O3 then applied per the rule, and `D-111` rewritten (uncommitted, never released). Tree under O3: `just check` → 27 lines, 927 passed, 5 ignored; `just repo-check` → 0 errors; 7,067 fresh `.dwo` files and a 99.8 MB platformer test binary (418 MB before) in `target/debug` | `logs/step4-*.log`, `step4.sh`, `runs.jsonl` (step 4) |
| 5 | 2026-10-04 | not adopted | Scratch A (the tree after step 4) and B (`visual = []`, `[[test]] required-features = ["visual"]` for the three pixel tests, and the lint recipe passing the three features). Cold `just check`: A 119.0 s, 27 lines, 927 passed; B 117.1 s, 24 lines, 919 passed; 5 ignored in both. Target after it: 4.43 vs 3.73 GB; 24 vs 21 test binaries. Core-edit `just check` loop, 5 interleaved rounds after a warm-up, min/median/max: A 8.11/8.22/9.09 s, B 7.39/7.53/8.41 s → saving 0.69 s, under the 1.0 s threshold (Q8). Core-edit test build: A 3.87/4.04/4.30 s, B 3.06/3.10/3.24 s → 0.94 s, also under. B's clippy still built all three pixel tests (`compiler-artifact` for each). No run flagged. Nothing in the tree changed. Rerun on O3 once step 4 switched (step 8): cold `just check` A 154.1 s, 927 passed; B 154.1 s, 919 passed, 24 lines; target 5.19 vs 4.51 GB; `just check` loop A 7.97/8.18/13.59 s, B 7.40/8.35/11.21 s (no saving by median); test build A 3.71/3.90/4.22 s, B 2.92/3.07/3.15 s (0.83 s). Still not adopted | `step5.sh`, `step5r.sh`, `step5{,r}.out`, `logs/` (steps 5, 5r) |
| 6 | 2026-10-04 | done (it builds) | `cargo bench --workspace --no-run --profile dev --locked` in the tree after `just check`: exit 0 in 3.69 s, six `Executable benches/` lines (`action_map_bench`, `ecs_bench`, `particle_tick`, `physics_bench`, `render_bench`, `tween_tick`). ci.yml's "Build benchmarks" step runs it; `rg -n 'bench-build' .github/workflows/ci.yml` → no match. `ci: check && deps ctx repo-check script-test` with the build as its body; `just --dry-run ci` → fmt, clippy, test, the dev bench build, `cargo deny`, ctx, repo-check, script-test. `D-112` written (heading, `D-070` marker, both rows); the agent setup's `just ci` row updated. `just script-test` → exit 0 (five Python suites OK, ShellCheck and actionlint clean); `just repo-check` → 0 errors. CI wall time: follow-up | `logs/step6-*.log` |
| 7 | 2026-10-04 | done | `just level-check` → exit 0 in 47.2 s: `OK: 530 owned outputs match; art, level, references and budgets validated`, then `Ran 15 tests in 34.768s` / `OK`. `git status --porcelain --untracked-files=all` identical before and after. `rg -n 'level-check' .github/workflows/ci.yml` → no match; `just --dry-run ci 2>&1 \| rg -c 'generate.py\|unittest'` → no match. The recipe sits beside `udeps` with the other local-only tools; the README's note on `just script-test` names it (and, after review R3, its command block) | `logs/step7-level-check.log`, `step7-status-{before,after}.txt` |
| 8 | 2026-10-04 | done; step 9 pending | One `CHANGELOG.md` `[Unreleased]` line. Review: 9 findings, 5 fixed in scope (R1–R5), 4 to follow-ups (§9). Gates on the final tree: `just check` exit 0 (27 lines, 927 passed, 0 failed, 5 ignored, 2.8 s warm); dev bench build exit 0; `cargo deny --frozen check` → advisories, bans, licenses, sources ok (advisory database cached 2026-10-03, no fetch); `just ctx` OK; `just repo-check` 0 errors; `just script-test` exit 0; `just smoke` exit 0 (matrix 4/4, post-stack 2/2, post-AA 1/1, bloom 1/1, lighting 1/1, game feel 2/2, mesh/transition 5/5, benchmarks 15/15, frame cap 1/1); `just visual` exit 0 (2 + 5 + 1 passed). Final tree measured as in step 1, step 1 → step 8, min/median/max: suite wall 10.89/10.92/11.09 → 1.86/1.87/1.89 s; route test 9.68 → 0.51 s; core-edit test build 7.19/18.39/27.37 → 3.44/3.46/3.71 s; `just check` loop 19.75/19.83/19.95 → 8.07/8.23/8.30 s; cold `just check` 166.8 → 153.1 s; target after it 10.65 → 5.19 GB; largest test binary 455 → 106 MB (umbrella lib test 391 → 86 MB); dev bench build 4.34 → 3.85 s. Every final median is below step 1's; no run flagged | `step8.sh`, `step8.out`, `step8-gates.{sh,out}`, `logs/step8-*.log` |
| 9 | 2026-10-04 | done: archived before the cut | Release session: the seven follow-ups below checked against the tree and moved to `docs/known-issues.md`; `[Unreleased]` given its `Summary:` and this plan's archived path; DESIGN's status line leads with 0.44; this plan marked done and archived. The cut, the release checks and the preflight run after this move; their results are in the session's hand-off, not here | `CHANGELOG.md`, `DESIGN.md`, `docs/known-issues.md` |

## Follow-ups

Moved to [known issues](../../known-issues.md) at the release (step 9).

- CI wall time after the owner's next push, against 148–193 s for `just bench-build` and about 5 min in all (step 6).
- Q3: a `LAUNCHES` row for a pair no route uses is dead data that nothing flags; flagging it would be a new assertion.
- Q11: `just level-check` is not listed in `docs/agent-setup.md`'s check tiers.
- R6: test temp directories pile up in `/tmp` (the scene tests now, manifest.rs's helper before); each test could remove its directory at its end.
- R7: `LightUbo::byte_size()` has no caller after step 3; it is public API, so removing it needs a break-ledger row.
- R8: move the deleted bloom test's note (ids 4..=7 follow sprite 0 and SMAA 1..=3, or `Renderer::reload_shader` routing breaks) beside the seeding at `crates/tungsten-render/src/renderer.rs:338`.
- R9: `D-041` lacks an `**Amended by D-096:**` marker line above `D-111`'s.

# GPU performance pass (phase-7's step 7.15, first plan)

- **status:** in progress
- **goal:** Run Phase 7's GPU pass ([phase-7.md](phase-7.md)'s Step 15 and §2.5, the [roadmap redo](roadmap-redo.md)'s 7.15 row and §5.8) as far as fixes that keep every pixel reach:
  - **The gate:** each of G1–G6 measured on its row with 7.3's runner and this plan's knock-out switches, on 7.13's tree: its time, its share of the row's GPU time, kept or `dropped` at 5 %, and half its reading as its target.
  - **G1:** the lit-sprite path at half its gate reading or under on every area row of example 01, by packing only the lights that can light the view and by a fragment skipping a light it lies out of reach of. In this session's readings the lit shader's loop over sixteen packed lights is 62–83 % of 01's scene pass at 2160p, and the two changes take the scene pass from 8.0–28.1 to 3.3–7.0 ms with the six areas' stills byte for byte the same.
  - **G6 (P7-11):** a light's `priority`, kept first past the cap, and a dark light holding no slot, each shown by a test past the cap; example 01's lantern at priority 1.
  - **G3, G4, G5:** each ended on the gate's evidence, `stopped` where its time is the work itself (the post passes' pixels and taps, the particle quads' fill, one copy of the instance bytes) or `dropped` under the floor, with what would lower it recorded as a lever for 9.2.
  - **S-23** read `landed`: the kit's burst budget and its sweep are 7.13c's.
  - The CPU frame, the extract included, and every other GPU row stay not `regressed`.
- **non-goals:**
  - G2, the backdrops' and tile layers' overdraw, and S-14, 01's own backdrop cull. Their fix is a renderer design, opaque sprites known at decode and drawn under the depth test or culled under opaque cells; this session measured the item and did not probe the design. Both are 7.15's second plan (§2.6, `phase-7.md` §2.8).
  - Any change of a drawn pixel: bloom's pyramid size or format, a vignette folded into bloom's composite, trimmed sprite quads. Each is a lever with its number (§2.5), none taken here.
  - More than sixteen lights, a light grid, per-sprite light lists (§2.3 says why none is needed for the target).
  - In-pass GPU timestamps, a GPU run on the `particles` row, any edit of `scripts/perf-rows.json`: the gate reads the scene pass's parts by knock-out switches applied for the measurement and restored.
  - Budgets (9.1), anything the gate does not keep (`phase-7.md` §2.5, "Nothing joins"), `unsafe`, a new device feature, a new dependency.
- **files to touch:** by step, nothing outside these lists
  - Step 1: this plan, `docs/plans/phase-7.md`, `docs/perf/benchmarks.md`; for the gate's build alone, restored before the commit: `crates/tungsten-render/src/sprite.rs`, `crates/tungsten-render/src/renderer.rs`, `crates/tungsten/src/light_extract.rs`, `crates/tungsten/src/particles.rs`, `examples/01_platformer/src/features/level/draw.rs`, `examples/01_platformer/src/features/level/mod.rs`
  - Step 2: `crates/tungsten-render/src/lighting.rs`, `crates/tungsten-render/src/lib.rs`, `crates/tungsten-render/src/tests/lighting.rs`, `crates/tungsten/src/light_extract.rs`, `crates/tungsten/src/tests/light_extract.rs`, `api/tungsten-render.txt`, `examples/01_platformer/tests/game/guards/pit.json`, `DESIGN.md`, `crates/tungsten-render/AGENTS.md`; `crates/tungsten-render/src/sprite.rs` for the reading's build alone, restored before the commit
  - Step 3: `assets/shaders/lit_sprite.wgsl`, `crates/tungsten-core/src/tests/lighting.rs`, `DESIGN.md`; `crates/tungsten-render/src/sprite.rs` for the reading's build alone, restored before the commit
  - Step 4: `crates/tungsten-core/src/components.rs`, `crates/tungsten-core/src/component_registry.rs` (its module doc's table), `crates/tungsten/src/asset_loader/scene.rs`, `crates/tungsten/src/tests/asset_loader.rs`, `crates/tungsten-render/src/lighting.rs`, `crates/tungsten-render/src/tests/lighting.rs`, `crates/tungsten/src/tests/light_extract.rs`, `examples/01_platformer/src/features/level/systems.rs`, `examples/01_platformer/tests/game/presentation.rs`, `api/tungsten-core.txt`, `examples/02_bench/src/tests/guards/gpu.json`, `examples/02_bench/src/tests/guards/gpu-throughput.json`, `examples/01_platformer/tests/game/guards/pit.json`, `DESIGN.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`
  - Step 5: this plan (deleted), `ROADMAP.md`, `docs/plans/phase-7.md`, `docs/perf/benchmarks.md`, `docs/known-issues.md`
- **ordered steps:** (1) the gate's readings, and the items it ends; (2) G1: only the lights that can light the view are packed; (3) G1: a fragment skips a light it lies out of reach of; (4) G6: a light's priority past the cap, and 01's lantern; (5) close
- **done-when:** every step's done-when passes or the step ends by §6, each quoted in the evidence log; in `phase-7.md` §2.5 each of G1 and G3–G6 reads `done`, `dropped` or `stopped` with the reading that says so (§2.2), or `open` with the step to re-plan named where §6 leaves one, and G2 reads `open` with the second plan; P7-11 and S-23 read `landed` and S-14 stays `open` for the second plan; the close sitting against `v0.118.0` reads no row `regressed`, the CPU rows among them; `just check`, `just perf-test`, `just perf counters`, `just visual`, `just smoke`, `just api-check` and `just repo-check` pass and example 01 runs (the six area rows' captures valid, their script reports passing); the plan deleted with its citations and the item out of Now.

Item `gpu-performance-pass` in [ROADMAP.md](../../ROADMAP.md), from `phase-7.md`'s Step 15, §2.5 and §2.8 and the redo's 7.15 row and §5.8. Written 2026-11-21 at `f4571d41`. Runs under the [tungsten-plan skill](../../.claude/skills/tungsten-plan/SKILL.md), the [tungsten-perf skill](../../.claude/skills/tungsten-perf/SKILL.md), the [renderer's rules](../../crates/tungsten-render/AGENTS.md) and the [operating rules](../operating-rules.md); this plan records only where it differs.

## Context digest

- 7.14 closed at `f4571d41` with every CPU and GPU row `ok` against `v0.118.0` (`perf-runs/20261121T224015Z-sitting/`); that sitting's candidate captures are this tree's code, so its GPU rows are where the audit starts.
- The list's rule (`phase-7.md` §2.5): at the pass's start each item is measured on its row; under 5 % of the row's time it is `dropped`; a kept item's target is half its reading; it ends `done`, `dropped` or `stopped`, the last on a profile that shows the work itself; nothing joins; G6 closes on its behaviour.
- A GPU pass is timed per render pass, and G1, G2 and G4 all sit inside the scene pass. This plan splits it by knock-out: scratch switches behind `TUNGSTEN_KNOCK`, applied to the tree for a measurement and restored, each taking one class of work out, an item's time being the difference of two captures (§2.1). M77's survey measured its backdrops the same way.
- This session's probes set what each step can promise (§1; `perf-runs/20261121-gpu-pass-plan/`, by hand and diagnostic: an agent session ran beside them):
  - Example 01 holds 23 lights and packs 16 in every area; each lit fragment loops over all of them, and tiles, props and the player are lit.
  - Keeping only the lights that reach the view takes the lit path's 5.3–24.7 ms down by 67–86 %, a range test in the fragment by 81–90 % with it; stills of the six areas match the stock build's byte for byte.
  - Blending costs nothing on this GPU, so an opaque no-blend path saves nothing; bloom and the vignette cost 5.4–5.6 ms at 2160p whatever is drawn; the particle quads of `particles` are fill; `gpu-throughput`'s encode is libc's copy.
  - The prototype of Steps 2–4 passes the workspace's tests but three counter-guard pins, `just visual` and `just perf counters`.
- Step 4 records one decision under the next free ID (the tungsten-decision skill, in the step's commit): the light's `priority` is file format under `D-103` and amends `D-061`'s cap.
- What reads this pass later: 7.15's second plan takes G2 and S-14 with §2.6; 9.2 takes §2.5's levers; 8.x games with many lamps take the priority.
- Decisions it stands on: `D-226` (a 7.15 item is judged by a full sitting), `D-061` and `D-240` (lighting and the light at a point), `D-060` and `D-087` (bloom, the swapchain's last stage), `D-103` (component shapes are file format), `D-070` (GPU rows stay local), `D-018` (the seam), `D-215` (formats break only with a migration).

## 1. Audit

Read at `f4571d41`. Readings are this session's, by hand, 2026-11-21; areas at 3840×2160, bench rows at 1920×1080.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | The lit fragment shader loops over every packed light, whatever its distance: a point light out of reach still costs a subtraction, two square roots, a division and the products | `assets/shaders/lit_sprite.wgsl:117-140` |
| A2 | `cull_to_cap` keeps every light while the World holds sixteen or fewer; past that it drops dark lights and ranks the rest by kind, distance to the view and distance to its centre. A light whose radius ends short of the view is never dropped for that | `crates/tungsten-render/src/lighting.rs:130-170` |
| A3 | `extract_lights` copies every `(Transform, Light)` into a list each frame and culls it; the renderer writes the 544-byte block only when its bytes change | `crates/tungsten/src/light_extract.rs:19-30`, `crates/tungsten-render/src/renderer.rs:795-804` |
| A4 | A sprite or tile is lit whenever its asset has a normal map. In 01 that is 1,186 of 2,305 sprites: every terrain tile, 2,794 of 4,060 background tiles, the player, balls and 730 of 744 props; enemies, backdrops and all particle sprites but one are unlit | `crates/tungsten/src/sprite_extract.rs:821`, `crates/tungsten/src/tilemap_extract.rs:234`, `examples/01_platformer/assets/manifest.json` |
| A5 | 01's World holds 23 lights (21 map glows, the moon, the lantern, dark while off); each of the six area rows packs 16, one or two of them dark. The generator already holds a 92×52-tile window to ten map lights, so the view's own lights fit | the `lightlog` switch, `perf-runs/20261121-gpu-pass-plan/knock/`; `examples/01_platformer/tools/level.py:61-64` |
| A6 | The scene pass by knock-out, clearing / aqueduct / ravine / hollow / mine / bridge: 16.106 / 20.202 / 7.981 / 28.062 / 19.445 / 12.432 ms; with lit batches drawn plain 3.244 / 3.305 / 2.710 / 3.405 / 3.368 / 3.051; with no light packed 3.981 / 4.283 / 3.018 / 4.761 / 4.311 / 3.604. The lit path is 5.27–24.66 ms, 38–72 % of `render_span`, nearly all of it the loop | `perf-runs/20261121-gpu-pass-plan/README.md`, `knock/index.tsv` |
| A7 | Under the plain draw, the backdrops are 1.80–2.42 ms, the tile layers 0.24–0.87, the `Sprite` entities 0.09–0.32 and an empty scene 0.26–0.47. Knocking particle emission out moves no area row by 0.01 ms | the same |
| A8 | With `BlendState::REPLACE` on the plain and lit pipelines the scene pass reads 16.100 against 16.108 and 3.246 against 3.244: blending is free here, so the overdraw's cost is its fill and its hidden fragments | `knock2/`; `crates/tungsten-render/src/sprite.rs:1231` |
| A9 | 01's backdrops are no entities: its extract base rebuilds their batches each frame, five strips with feet under two skies, and drops a quad only when opaque tiles hide it whole. No engine code knows a sprite is opaque | `examples/01_platformer/src/features/level/draw.rs:81-91`, `:226`, `components.rs:154-248`; `crates/tungsten-core/src/assets/registry.rs:14-42` |
| A10 | Bloom is a threshold, five downsamples, five upsamples and a composite over a half-size `Rgba16Float` pyramid; 01's stack is Bloom then Vignette. Per area: threshold 0.27, down 0.69, up 1.60 (the last, into the half-size mip, 1.19), composite 1.21, vignette 1.72: 5.42–5.57 ms, and 5.34 on an empty scene | `crates/tungsten-render/src/post/bloom.rs:307-500`, `crates/tungsten-render/src/targets.rs:625-672`; `examples/01_platformer/src/features/level/mod.rs:342-357`; `bench_gpu.py` at 3840x2160 |
| A11 | The frame's last full-screen stage writes and clears the swapchain (`D-087`), which costs about 0.7–0.8 ms more at 2160p than an offscreen target whichever pass it is: bloom reads 4.48–4.59 with the vignette off, the scene pass 0.66–0.83 more with both off; an empty scene's clear reads 0.53 against 0.25 | `knock/`; `crates/tungsten-render/AGENTS.md` (Frame order) |
| A12 | A quarter-size pyramid takes bloom 3.78 → 2.03 ms and changes the finest glow; `Rg11b10Ufloat` takes it to 3.66. No change that keeps the picture is known for a post pass | `knock2/` |
| A13 | Particles are sprites through the default extract. On `particles` the scene pass reads 2.329 of a 2.349 ms span with 41,443 particle quads and 0.345 without them; its sprites are soft dots that fill their quads | `crates/tungsten/src/particles.rs:656-661`; `examples/02_bench/src/gen.rs:327`; `bench_gpu.py` |
| A14 | S-23's fix is in the kit since 7.13c: `BurstBudget` caps the bursts that count, key bursts are never dropped, and `burst_cleanup` despawns a burst once its particles are gone; 01's `spawn_burst` and `spawn_key_burst` call `kit::burst_with` and no hand despawn is left | `crates/tungsten-kit/src/present/burst.rs:17`, `:53`, `:143` (`79095a5d`); `examples/01_platformer/src/shared.rs:282-293` |
| A15 | The sprite draw copies each batch's instances into one staging view, a frame's 19,200,128 bytes on `gpu-throughput`. `render_encode` reads mean 1.578 and 1.989 of 12.853 and 13.094 ms `total` (12.3 and 15.2 %); a gated profile puts 14.52 % of the window's samples under `record_main_draws` and 14.79 % in libc's copy; the run faults one to three pages a frame | `crates/tungsten-render/src/sprite.rs:1024-1043`; `perf-runs/20261121T224015Z-sitting/captures/gpu-throughput-B*`; `g5-profile/` |
| A16 | On `gpu` `render_encode` is text: 1.77 ms, 0.38 with `glyphs=0`. On 01's area rows it is 0.35–0.49 of 14.9–35.3 ms `total` | `bench_gpu.py`; the close sitting's captures |
| A17 | `gpu`'s sixteen point lights have a radius of 0.3 of the view's width with centres across it, so each lights about half the view: there the reach filter moves nothing (6.927 against 6.931) and the range test takes the lit path 1.735 → 1.141 ms | `examples/02_bench/src/gpu/scene.rs:242-262`; `bench_gpu.py` |
| A18 | The scene pass's two values in `gpu`'s GPU runs are gone: twenty runs read p50 6.93–6.96 | `perf-runs/20261121T224015Z-sitting/captures/gpu-*`; `docs/perf/benchmarks.md` (Levers) |
| A19 | Tests that pin the cull: render's `cull_to_cap_truncates_to_sixteen` puts 32 lights of radius 1 up to 310 px from a 1 px view, so 31 never reach it; `at_the_cap_the_order_is_unchanged`, the two past-the-cap tests and the umbrella's four hold. Core's `the_query_follows_the_lit_shaders_light_lines` pins nine shader lines, each still present under a range test | `crates/tungsten-render/src/tests/lighting.rs:33-42`, `crates/tungsten/src/tests/light_extract.rs`, `crates/tungsten-core/src/tests/lighting.rs:143-167` |
| A20 | `Light` is 24 bytes with three struct literals outside its constructors; its JSON shape is a tagged `LightFields` with `deny_unknown_fields`, validated finite and non-negative, and is file format (`D-103`). One asset file spells it | `crates/tungsten-core/src/components.rs:297-333`; `crates/tungsten/src/asset_loader/scene.rs:230-295`; `crates/tungsten/src/tests/asset_loader.rs:1070`, `:1075`; `crates/tungsten/tests/fixtures/rooms/prefabs/lamp.json` |
| A21 | The prototype (`perf-runs/20261121-gpu-pass-plan/proto/`, its logs under `logs/`): `cargo test --workspace --locked --no-fail-fast` fails three guard pins and nothing else; `just api-check` lists six added lines; `just perf counters` and `just visual` pass; 01's glow system spawns each glow's light, the player's among them | `logs/c-workspace-results.txt`, `c-api-check.log`, `c-counters.log`, `c-visual-results.txt`; `examples/01_platformer/src/features/level/systems.rs:128-148` |

**Pins the prototype moved** (A21), each exact:

| Pin | At `f4571d41` | After Steps 2 and 3 | After Step 4 |
| --- | --- | --- | --- |
| `examples/01_platformer/tests/game/guards/pit.json` | `allocations` 40,267, `allocated_bytes` 5,091,777 | 40,207 and 4,996,257 | 40,207 and 4,997,217 |
| `examples/02_bench/src/tests/guards/gpu.json` | `allocated_bytes` 1,350,960 | unchanged | 1,353,840 |
| `examples/02_bench/src/tests/guards/gpu-throughput.json` | `allocated_bytes` 116,160 | unchanged | 117,120 |

## 2. Design

### 2.1 The gate

- **Tools.** `perf-runs/20261121-gpu-pass-plan/tools/` (its `README.md` lists them). `apply_knock.py [group…]` edits the tree, asserting every text it replaces; `restore_knock.sh [group…]` puts the same groups' files back from the last commit. The switches read `TUNGSTEN_KNOCK`, which the runner passes to its child, and are inert without it.
- **Area rows.** `knock_areas.sh <out> <label>=<tokens>…` takes one capture of each `e01-areas` row a label through `scripts/perf.py run`, `REPEAT=3` at the gate, so every capture is the runner's, valid by its rules. `gate_areas.py <out>` then prints, per area row, from the mean over a capture's runs of the window's mean:
  - **G1**, the lit path: `scene[control] − scene[unlit]`, where `unlit` draws lit batches through the plain pipeline.
  - **G2**, backdrops and tile layers: `scene[unlit] − scene[fill]`, `fill` being `unlit,backdrops,tiles`. It is read under `unlit` so that the tiles' light is G1's alone and no millisecond is counted twice.
  - **G3**, bloom and the post stack: the sum of the control's post passes.
  - **G4**, particle quads: `scene[control] − scene[particles]`.
- **Bench rows**, by hand with `bench_gpu.py` (`RUNS=3`), which runs the bench as the runner's GPU run does and refuses the encoder: `gpu` with and without `unlit` (G1), its post passes (G3) and its light sweep at `lights=0`, `4` and `8` (G6's row); `particles` with and without the `particles` switch (G4). `gpu-throughput` is a plain capture, `stage_share.py <capture> render_encode` (G5).
- **Share.** An item's time over the row's GPU time, the control's `render_span`, mean over mean; G5's over `total`.
- **Kept** when the share reaches 5.00 % on a row the item's line names; for the area rows, in any of the six. **Target:** half the reading, on each row that keeps it, an area row a row.
- **Two rows.** An item kept on 01's area rows and on a bench row has a target on each. Its status names each row's verdict, and it is terminal when every kept row reads `done` or `stopped` (§2.2).

This session's readings, which rank and do not stand in for Step 1's:

| Item | Row | Reading | Share | Verdict | Target |
| --- | --- | --- | --- | --- | --- |
| G1 | the six area rows | 12.861 / 16.896 / 5.271 / 24.657 / 16.077 / 9.381 ms | 58.0 / 64.2 / 37.8 / 72.1 / 63.1 / 51.0 % | kept | half of Step 1's reading, an area a row |
| G1 | `gpu` | 1.735 ms | 16.3 % | kept | half of Step 1's (0.868 on this one) |
| G2 | the six area rows | about 2.4–2.9 ms (backdrops 1.80–2.42, tiles 0.24–0.87) | 8–17 % | kept | the second plan's (§2.6) |
| G3 | the six area rows | 5.42–5.57 ms | 16–39 % | kept | half; no step (§2.5) |
| G3 | `gpu` | bloom 1.183 and vignette 0.262 ms | 13.6 % | kept | half; no step |
| G4 | the six area rows | 0.00 ms | 0.0 % | under the floor here | |
| G4 | `particles` | 1.984 of a 2.349 ms span | 84.5 % | kept | half; no step (§2.5) |
| G5 | `gpu-throughput` | `render_encode` mean 1.578 and 1.989 ms | 12.3 and 15.2 % | kept | half; no step (§2.5) |
| G5 | `gpu`, the area rows | text on `gpu` (A16); 0.35–0.49 ms on the areas | 1.4–2.4 % on the areas | no reading of the item on `gpu`; under the floor on the areas | |
| G6 | 01's lantern; `gpu`'s light sweep | behavioural | | never dropped | its two tests past the cap; the scene pass not `regressed` |

### 2.2 How a step is judged

- **Before a step:** `just preflight docs/plans/gpu-performance-pass.md --step N`; a hit outside the step's list is read before editing.
- **Step sitting.** With the step's change in the tree, uncommitted, and nothing beside it: `WGPU_BACKEND=vulkan just perf sitting --base HEAD --rows <the step's rows> --wait-quiet 600`. The owner's remote-desktop client stays disconnected; never `--allow-background` for a judged sitting.
- **Reading a sitting.** A row's judged pairs are A1→B1 and A2→B2, or A3→B3 and A4→B4 where `sitting.json` marks the row `reran`: the runner's verdict stands on those alone. `just perf compare` of each judged pair: the step's metric reads lower in both where the step says so, each prints "digests match", and the sitting reads no row `regressed`.
- **Stills.** After the sitting, which leaves the base's build in `target/perf/base-aligned` and the step's in `target/perf/tree-aligned`: `python3 -B perf-runs/20261121-gpu-pass-plan/tools/area_stills.py target/perf/<side>-aligned/release/example-01-platformer perf-runs/715-gpu-pass/step-N/stills/<side>` for both sides, then `cmp` of each area's two files. Script mode pins dt, and one build repeats its bytes (A21's session folder, `stills/proto-again/`).
- **The item's reading again** (Steps 2 and 3, after the stills): `python3 -B perf-runs/20261121-gpu-pass-plan/tools/apply_knock.py unlit`, the group that edits `sprite.rs` alone, which no step of this plan changes; `REPEAT=3 bash …/knock_areas.sh perf-runs/715-gpu-pass/step-N/areas control=- unlit=unlit`; `python3 -B …/gate_areas.py perf-runs/715-gpu-pass/step-N/areas --items G1`; for `gpu`, `WGPU_BACKEND=vulkan just perf run gpu --repeat 1` to build the bench, then `RUNS=3 python3 -B …/bench_gpu.py target/perf/tree-aligned/release/example-02-bench gpu default 300 60 - "ENV:TUNGSTEN_KNOCK=unlit;"`; then `bash …/restore_knock.sh unlit` and `git status --short crates/tungsten-render/src/sprite.rs` printing nothing. `restore_knock.sh` with no group is Step 1's alone: it would undo Step 2's `light_extract.rs`.
- **Done-when and target are two things.** A step's done-when is what lets it commit: its tests, the pins it may move, the pixels it may not, and a sitting in which its metric fell and nothing regressed. Its **Target** line is the item's reading against §2.1's, quoted in the evidence log. A step whose metric fell is kept whether or not the target is met.
- **An item's end** (`phase-7.md` §2.5) is read on the last of its steps that landed, a kept row at a time. `done` when the row's target is met. `stopped` when it is missed and the evidence shows the work itself, no fix that keeps the drawn pixels being known for it. Where such a fix is known and unplanned, or a step of the item's own was skipped, the row stays `open` with what remains named. The item's cell in `phase-7.md` names each kept row's verdict and reading.
- **`stopped` with no step** (G3, G4, G5) is the same rule read at the gate: §2.5 names, per item, the evidence Step 1 must record before it writes `stopped`, and the levers that change the picture or cross the seam, which are 9.2's and never a reason to hold the item `open` (Q3).
- **Exact means checked.** Steps 2 and 3 draw the same pixels: a test that sums the lit shader's CPU mirror, `tungsten_core::lighting::light_from`, over the old and the new packed lists at points of the view and compares the sums bit for bit, with a negative control; the six stills byte for byte; `just visual` with no fixture touched.
- **Pins.** A pin is in a step's list where the step may move it, and its Moves line says how; rewrite one with `TUNGSTEN_GUARDS_WRITE=1` on that test, then run it twice.
- **Every commit:** `cargo fmt --all -- --check`, `git diff --check` and `just check` → exit 0; a commit of documents alone (Step 1) takes `just repo-check` in `just check`'s place. A step that touches the renderer or a shader adds `cargo test -p tungsten-render --locked`, `just smoke`, `just visual` and `just perf counters`. Evidence goes under `perf-runs/715-gpu-pass/step-N/`.

### 2.3 G1: the lit path

**The list holds only lights that can light the view** (Step 2).

- `tungsten_render::lights_view(camera_aabb, position, &light) -> bool`, public beside `cull_to_cap`: false for a dark light (intensity 0) and for a point light whose radius, taken as at least `MIN_LIGHT_RADIUS` as the shader clamps it and widened by a factor of 1.0001 for the two sides' rounding, ends short of the view's box; true for a directional light, for a light whose colour times its intensity is not finite, and wherever a comparison meets a NaN. So a dropped light is one whose term in the shader is exactly zero at every fragment of the view: its attenuation, `clamp(1 − d / r, 0, 1)`, is 0 wherever `d ≥ r`.
- `cull_to_cap` filters by it first, into a list sized for every entry as today's is, so no allocation is added (a filtered `collect` grew by doubling and raised two pins in the prototype); the cap's ranking and the final order are unchanged, and the dark-light `retain` past the cap goes, the filter having done it.
- `extract_lights` filters by it while it copies, so a light that cannot matter is never copied.
- **The box** is the one the frame draws: the extract's and the frame's projection read the same `CameraState` and the surface's size (`crates/tungsten/src/app.rs:2517-2596`, `crates/tungsten-core/src/camera.rs:1273-1300`), exact without rotation and a bounding box with it. On a frame whose late acquire resizes the surface the box is the size before it, as the sprite cull's is (`D-114`, `D-145`).
- **What is drawn.** The ranking's order is unchanged and its candidates are a subset, so every light the old cull kept that reaches the view is still kept, in the same relative order; a dropped light added exactly zero. A frame in which the old cull kept every light that reaches the view draws the same pixels, sum for sum. A frame where it dropped one, a far lamp with a wide radius behind sixteen nearer ones, gains that light. The cull never loses a light that reaches the view.
- **Cost removed.** 01's areas pack the view's own lights in place of sixteen: the prototype's `reach` reading is 7.463 / 6.772 / 3.444 / 10.452 / 6.656 / 4.418 ms of scene pass, the lit path at 4.219 / 3.467 / 0.734 / 7.047 / 3.288 / 1.367 against targets of 6.431 / 8.448 / 2.635 / 12.328 / 8.038 / 4.691.

**A fragment skips a light it lies out of reach of** (Step 3).

- In the point branch of `lit_sprite.wgsl` the attenuation is computed first, exactly as today, and the direction, the normal's dot and the sum run under `if (attenuation > 0.0)`. A skipped term is the one that multiplied by an attenuation of 0. The nine lines core's test pins stay, and the test gains the `if` line, so `light_from` and the shader keep saying the same thing.
- A clear texel's early exit was probed and adds 0.1–0.35 ms: not taken.
- **Cost removed.** With Step 2: 5.637 / 5.534 / 3.257 / 6.990 / 5.496 / 4.108 ms of scene pass (the prototype read 5.639 / 5.535 / 3.259 / 7.001 / 5.490 / 4.112), the lit path at 2.395 / 2.230 / 0.549 / 3.596 / 2.122 / 1.061. On `gpu` the lit path reads 1.735 → 1.141 ms against a target of 0.868.
- **`gpu`'s expected end.** Each of its sixteen lights reaches about half the view (A17), so what is left there is the sum over lights that do light the fragment. No change that keeps the pixels is known for it; a light grid or per-sprite lists skip only the range tests already skipped. Step 3's evidence for `stopped` on that row is the light sweep: the lit path's time at `lights=0`, `4`, `8` and `16` before and after, rising with the lights in reach.

### 2.4 G6: a light's priority

- `Light` gains `pub priority: u8`, 0 from `Light::point` and `Light::directional`, and `Light::with_priority(self, u8) -> Self`. The scene and prefab shape gains an optional `"priority"` (default 0) on both kinds; an older file reads as before, and `deny_unknown_fields` stays.
- Past the cap `cull_to_cap` ranks by priority first, higher kept first, then as today: directional before point, nearer the view, nearer its centre. Under the cap, and in the final order, priority changes nothing.
- A light that cannot light the view takes no slot whatever its priority (Step 2's filter runs first): "never culled" means never dropped by the cap while it lights the view. More than sixteen lights at the top priority rank among themselves as today.
- Example 01 gives its lantern priority 1: `spawn_parts` in the glow system, for a glow whose owner is the `Player`. No asset file changes; `assets/prefabs/player.json` is the generator's output.
- **Tests past the cap** (render): a priority light at the view's edge keeps its slot against twenty lit lights nearer the centre, and loses it at priority 0 (the control); a dark light takes no slot under the cap and past it; a priority light out of reach takes none; at equal priorities the two existing past-the-cap tests pass unedited. The loader's tests read `priority` present, absent and out of a `u8`'s range. One 01 test: with twenty lamps lit nearer the view's centre than the player stands, the frame's packed lights hold the lantern's, and it fails with the lantern at priority 0.
- **Pins.** `Light` grows from 24 to 28 bytes, so the extract's list of the lights it copies grows by four bytes an entry of its capacity: `gpu.json` +48 B a frame, `gpu-throughput.json` and `pit.json` +16 B a frame over their 60-frame windows (A21's table). No allocation count moves. `LightUbo` stays 544 bytes; the priority never reaches the GPU.
- **The decision** (the tungsten-decision skill, the next free ID): a light's `priority` in the component and its file shape, the cap's ranking by it, and the cull's rule that a light which cannot light the view takes no slot; it amends `D-061`.

### 2.5 G3, G4 and G5: what the gate must show before it ends them

No step is planned for these three: this session found no change that keeps the drawn pixels and the seam for any of them. Step 1 ends each by §2.2 only on the evidence below, taken at the gate; an item whose evidence does not hold stays `open`, named in the close's hand-off.

- **G3, bloom and the post stack.**
  - **Evidence:** the control's per-pass means in each area (`knock_table.py`), with each pass's target size and taps beside it, showing every pass's time follows its pixels and taps; and `bench_gpu.py … gpu default 300 60 "resolution=3840x2160,aa=off" "resolution=3840x2160,aa=off,glow=0,sprites=0,tile_layers=0,glyphs=0"`, showing the stack costs the same with nothing drawn (this session: 5.34 ms empty, 5.42–5.57 in the areas).
  - **Levers, all changing pixels:** a pyramid whose first level follows a 1080-line reference, so 2160p starts at a quarter: bloom 3.78 → 2.03 ms, a wider finest glow, the owner's review; a trailing vignette folded into the composite, or drawn as a blended triangle in the composite's pass: about 0.95 ms, one 8-bit rounding fewer; the swapchain's clear under a full-screen stage, which `LoadOp::DontCare` removes behind `unsafe` (A11); `Rg11b10Ufloat`: 0.11 ms. Even the first two together read about 2.8 ms against a target of 2.75.
- **G4, particle quads.**
  - **Evidence:** on `particles`, the scene pass with and without the `particles` switch, and again at `rate=0.5`, showing the time follows the quads drawn; `gen.rs`'s particle sprites fill their quads (A13), so trimming would draw the same fragments. 01's area rows read 0.00 ms and no row sprays, so "01's spray frames" has no reading.
  - **Levers:** quads trimmed to a sprite's drawn box at decode, for sprites with clear margins (01's `ex10_mote` holds an 18 px dot); a GPU particle simulation (`benchmarks.md`, G4).
- **G5, sprite batching and upload bytes.**
  - **Evidence:** `stage_share.py` on Step 1's `gpu-throughput` capture; `faults.py`, showing no fresh memory a frame; and a gated profile of the window, `WARMUP=60 python3 -I perf-runs/20261121-cpu-pass-2-plan/tools/gated_profile.py target/perf/tree-aligned/release/example-02-bench bench:gpu:throughput:365 perf-runs/715-gpu-pass/step-1/g5 g5 0-299 5000`, then `perf report -i …/g5.data --stdio --no-children --sort dso -g none` and `--children --sort symbol -g none`, showing the samples under `record_main_draws` are libc's copy (this session: 14.52 % and 14.79 % of the window, `render_encode` 2.012 of 13.219 ms).
  - **Levers:** the extract writing the staging view itself and instances kept between frames while nothing moved, the default extract's levers in `benchmarks.md`, each a design across the seam (`D-018`); a narrower instance, which is a public type and loses `z_norm`'s range.

### 2.6 The second plan's items

- **G2**, the backdrops' and tile layers' overdraw: about 2.4–2.9 ms of each area's scene pass, 8–17 % of today's `render_span` and a fifth to a quarter of it once G1's steps land. Half of it can only be fragments that are hidden: A8 shows a blend costs nothing, so drawing opaque sprites without one saves nothing, and what remains is not drawing what an opaque sprite covers.
- **S-14**, 01's backdrop cull: the same knowledge, held in 01 as a generator's list of opaque sprites and a summed-area table of opaque cells (A9).
- **The lever to probe first:** a sprite's opacity measured at decode, then either opaque batches drawn front to back under the depth test, which also retires 01's cull, or S-14's cover as an engine structure the backdrops' quads are cut against. The second plan starts from Step 1's G2 reading and takes a probe of the hidden fraction before it promises a target, as 7.14's second plan did for its extract. It is written when this plan closes (`phase-7.md` §2.8).

### 2.7 Where each row and lever goes

| Row or lever | Where | Status after this plan |
| --- | --- | --- |
| P7-11 | Step 4 (and Step 2's dark-light drop) | `landed`; `open` in 7.15 if Step 4 is skipped |
| S-23 | Step 1, on A14 | `landed`, citing `79095a5d` |
| S-14 | the second plan (§2.6) | `open`, 7.15 |
| G2 | the second plan | `open; kept at the gate`, with its reading |
| The scene pass's two values (`benchmarks.md`, Levers) | Step 1, on its `gpu` capture's per-run p50s | `closed` when they agree within 0.05 ms, else left `open` with the numbers |
| §2.5's levers; a light grid; the swapchain's clear | `benchmarks.md`'s Levers, at the close | lines there, each with its number |
| G3 more than 16 lights (`benchmarks.md`, Levers) | untouched | `open` |

## 3. Steps

Each step: the preflight, the change, its tests, its pins, its sitting, stills and reading where it has them, then one commit of its own paths, pushed (`git push origin 0.119`, the whole command). A step's files are its line in the header's list. `phase-7.md`'s statuses are written at Step 1 and at the close, not by the step that lands a row.

### Step 1: The gate's readings, and the items it ends

- **Files:** this plan, `docs/plans/phase-7.md`, `docs/perf/benchmarks.md`; the six files of `apply_knock.py`'s groups for the gate's build alone, restored before the commit
- **Change:** set `status: in progress`. On a quiet machine, the remote-desktop client disconnected: `df -h .` (under 60 GB free: `just clean-target --apply`).
  1. `python3 -B perf-runs/20261121-gpu-pass-plan/tools/apply_knock.py`, every group.
  2. `REPEAT=3 bash perf-runs/20261121-gpu-pass-plan/tools/knock_areas.sh perf-runs/715-gpu-pass/step-1/areas control=- unlit=unlit fill=unlit,backdrops,tiles particles=particles lightlog=lightlog`, then `gate_areas.py perf-runs/715-gpu-pass/step-1/areas` and `knock_table.py` on the same folder, their output kept beside it.
  3. `WGPU_BACKEND=vulkan just perf run gpu --repeat 1`, which builds the bench with the switches, then `RUNS=3 bench_gpu.py` on `gpu` (`-`, `ENV:TUNGSTEN_KNOCK=unlit;`, `lights=0`, `lights=4`, `lights=8`, and §2.5's two 3840x2160 sets) and on `particles` (`default 300 180`: `-`, `ENV:TUNGSTEN_KNOCK=particles;`, `rate=0.5`, `ENV:TUNGSTEN_KNOCK=particles;rate=0.5`).
  4. `bash perf-runs/20261121-gpu-pass-plan/tools/restore_knock.sh`, every group.
  5. On the restored tree: `WGPU_BACKEND=vulkan just perf run gpu-throughput` and `WGPU_BACKEND=vulkan just perf run gpu`, whose logs print `Compiling`; `stage_share.py` on the first; `faults.py`; §2.5's profile.
  6. Write `phase-7.md` §2.5: each of G1–G6's evidence cell gains its reading, share and target by row. Statuses: G1 `open; kept at the gate`; G2 `open; kept at the gate; the second plan`; G3, G4 and G5 each `stopped` with §2.5's evidence quoted, `dropped` with its number under the floor, or `open` where the evidence does not hold; G6 `open` with its target as written. S-23's row reads `landed` on A14. `benchmarks.md`: the `e01-areas` section gains the gate's split of the scene pass and the packed-light count; the two-values lever by §2.7.
- **Done-when:** every capture → exit 0 and valid; `gate_areas.py` → exit 0, its table in the evidence row; `git status --short crates examples assets` → prints nothing; `rg -n '^\| G[1-6] \|.*\| open \|$' docs/plans/phase-7.md; test $? -eq 1` → exit 0, no item left at a bare `open`; `rg -c '^\| S-23 \|[^|]*\| 7\.15 \| landed \|' docs/plans/phase-7.md` → 1; `just repo-check` → exit 0
- **Commit:** `docs(plans): record the GPU pass's gate`
- **Moves:** none.

### Step 2: G1, only the lights that can light the view are packed

- **Files:** `crates/tungsten-render/src/lighting.rs`, `crates/tungsten-render/src/lib.rs`, `crates/tungsten-render/src/tests/lighting.rs`, `crates/tungsten/src/light_extract.rs`, `crates/tungsten/src/tests/light_extract.rs`, `api/tungsten-render.txt`, `examples/01_platformer/tests/game/guards/pit.json`, `DESIGN.md`, `crates/tungsten-render/AGENTS.md`; `crates/tungsten-render/src/sprite.rs` for the reading alone
- **Change:** §2.3's first part, from the prototype: `patch -p1 < perf-runs/20261121-gpu-pass-plan/proto/g1-reach-range.patch`, then the shader put back, since it is Step 3's (`git show HEAD:assets/shaders/lit_sprite.wgsl > assets/shaders/lit_sprite.wgsl`); or `proto/g1-files/` by hand. Tests in render: `lights_view`'s cases (dark; directional; inside, at and past its reach; the margin; a radius under the minimum; a NaN radius; a colour that is not finite); a seeded test over random lights and views that, for every light the filter drops, reads `light_from` as exactly `Vec3::ZERO` at the view's corners, at its point nearest the light and at 64 random points; one that sums `light_from` over the old list (a `#[cfg(test)]` copy of today's `cull_to_cap`) and the new at those points, for scenes of at most sixteen lights, bit for bit, with a control that a filter one light too eager fails it; past the cap, a far lamp with a wide radius is kept where the old list dropped it. `cull_to_cap_truncates_to_sixteen` gives its 32 lights radii that reach its view. In the umbrella: lights out of reach are never packed, and the directional light still leads. `DESIGN.md`'s lighting paragraph and the renderer's `AGENTS.md` line say which lights are packed. `just api` for the two added lines.
- **Done-when:** `cargo test -p tungsten-render --locked` and `cargo test -p tungsten --locked --lib light_extract` → exit 0; `pit.json` rewritten, then `cd examples/01_platformer && cargo test -p example-01-platformer --locked --test game guards::` → exit 0 twice; `jq -e '.allocations <= 40207 and .allocated_bytes <= 4996257' examples/01_platformer/tests/game/guards/pit.json` → exit 0; `cargo test -p example-02-bench --locked guards_` → exit 0 with no pin rewritten; `just api-check` → exit 0; `just perf counters` → exit 0 with no pin rewritten; `just visual` and `just smoke` → exit 0; the sitting over `e01-areas,gpu,gpu-throughput,e01-pit,template` → no row `regressed`, `gpu.scene` lower in both judged pairs of all six area rows, "digests match" on every row; the six stills → `cmp` exit 0 each
- **Target:** `gate_areas.py --items G1` → each area's G1 at most half of Step 1's. `gpu`'s reading is quoted; it is Step 3's.
- **Commit:** `perf(render): pack only the lights that reach the view`
- **Moves:** `pit.json`, `allocations` and `allocated_bytes` to the prototype's values or lower; `api/tungsten-render.txt`, two added lines. Nothing rises.

### Step 3: G1, a fragment skips a light it lies out of reach of

- **Files:** `assets/shaders/lit_sprite.wgsl`, `crates/tungsten-core/src/tests/lighting.rs`, `DESIGN.md`; `crates/tungsten-render/src/sprite.rs` for the reading alone
- **Change:** §2.3's second part (the prototype patch's shader hunk; `perf-runs/20261121-gpu-pass-plan/shaders/range.wgsl` is the whole file). Core's light-lines test gains the `if` line and a comment that a skipped term is one `light_from` reads as zero. `DESIGN.md`'s lighting paragraph says a fragment out of a light's reach skips it.
- **Done-when:** `cargo test -p tungsten-render --locked` (the shader coverage among it) and `cargo test -p tungsten-core --locked --lib lighting` → exit 0; `just check` → exit 0 with no pin rewritten; `just perf counters`, `just visual` and `just smoke` → exit 0; the sitting over `e01-areas,gpu` → no row `regressed`, `gpu.scene` lower in both judged pairs of `gpu` and of at least four area rows, none higher; the six stills → `cmp` exit 0 each
- **Target:** `gate_areas.py --items G1` → each area's G1 at most half of Step 1's → G1 reads `done` on the area rows; `bench_gpu.py`'s `gpu` reading at most half of Step 1's → `done` there, else §2.3's light sweep (`lights=0`, `4`, `8` and the default, with and without `unlit`, `RUNS=3`) in the evidence row and `stopped` there by §2.2.
- **Commit:** `perf(render): skip a light a fragment lies out of reach of`
- **Moves:** none.

### Step 4: G6, a light's priority past the cap, and 01's lantern

- **Files:** as the header's list
- **Change:** §2.4, from the prototype (`perf-runs/20261121-gpu-pass-plan/proto/g6-priority-core-loader.patch` and the rank line in `proto/g6-files/`), its tests, 01's lantern and its one test, the component table's shape, `DESIGN.md`'s lighting paragraph, `just api` for the four added lines, and the decision with its index row and `D-061`'s amended-by marker.
- **Done-when:** `cargo test -p tungsten-core --locked`, `cargo test -p tungsten-render --locked` and `cargo test -p tungsten --locked --lib asset_loader` → exit 0; the three pins rewritten, then `cargo test -p example-02-bench --locked guards_` and 01's `guards::` → exit 0 twice; `jq -e '.allocated_bytes <= 1353840' examples/02_bench/src/tests/guards/gpu.json`, `jq -e '.allocated_bytes <= 117120' examples/02_bench/src/tests/guards/gpu-throughput.json` and `jq -e '.allocations <= 40207 and .allocated_bytes <= 4997217' examples/01_platformer/tests/game/guards/pit.json` → exit 0 each; `cd examples/01_platformer && cargo test -p example-01-platformer --locked --test game presentation::` → exit 0, the new test among those it names; `just api-check` → exit 0; `just perf counters`, `just visual` and `just smoke` → exit 0; `just check` → exit 0; the sitting over `e01-areas,gpu` → no row `regressed`; the six stills → `cmp` exit 0 each, the priority changing no frame under the cap
- **Target:** G6's two tests past the cap pass and the sitting reads the scene pass not `regressed` → G6 `done`, P7-11 `landed`.
- **Commit:** `feat(render): keep a light by its priority past the cap`
- **Moves:** the three pins' `allocated_bytes`, up by `Light`'s four bytes times the extract's list, to A21's values or lower; `api/tungsten-core.txt`, four added lines; the light's file shape, one optional field.

### Step 5: Close

- **Files:** this plan (deleted), `ROADMAP.md`, `docs/plans/phase-7.md`, `docs/perf/benchmarks.md`, `docs/known-issues.md`
- **Change:** age the base build (`fd -t f . $(fd -t d -d 1 '^(tungsten|example-0)' target/perf/base-aligned/release/.fingerprint) -x touch -d 2001-01-01 {}`), then the close sitting, `WGPU_BACKEND=vulkan just perf sitting --wait-quiet 900`, every row against `v0.118.0`, its log showing the base's crates `Compiling`. `phase-7.md`: §2.5's statuses with each item's last reading by row, §2.7's rows, Step 15's part 1 note and its evidence row. `benchmarks.md`: the `e01-areas` and `gpu` sections' numbers after the steps, and §2.7's levers. Known issues: a limit line where a step ended short (a skipped step's row). `ROADMAP.md`: this item out of Now, the `phase-7` line saying what landed and that 7.15's second plan is next.
- **Done-when:** the close sitting → exit 0, no row `regressed`, the six area rows' captures valid with their script reports passing; `just check`, `just perf-test`, `just perf counters`, `just visual`, `just smoke` and `just api-check` → exit 0; `rg -n '^\| [^|]+ \| [^|]+ \| 7\.15 \| open \|' docs/plans/phase-7.md` → S-14, and beside it only P7-11 where Step 4 was skipped; `rg -c '^\| G[1-6] \|' docs/plans/phase-7.md` → 6, G2 `open` with the second plan and each of the others `done`, `dropped`, `stopped` or `open` with what remains named (§2.2); `just repo-check` → exit 0 after the deletion
- **Ends with:** the commits, the evidence, what the owner should try (01's hollow and aqueduct under `F4` at 2160p, and the lantern among the hollow's wisps), and the plan prompt for 7.15's second plan: tungsten-next's `step` shape over `phase-7.md`'s Step 15 and §2.5 and the redo's 7.15 row and §5.8, naming G2 and S-14, any row §6 left `open`, and this plan's §2.6 and audit as they stood at the last commit that held it.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | The gate reads the scene pass's parts as differences of captures under scratch switches, not as timestamps | Yes: a pass is the unit the GPU times, M77's survey read its backdrops this way, and control captures repeat within 0.01 ms here. In-pass timestamps are a device feature and a telemetry change, 9.1's if it wants a frame timeline |
| Q2 | G2 read under `unlit`, G1 as the lit path's whole excess | Yes: the tiles' light is G1's, their fill G2's, and the four readings with the empty scene sum to the scene pass |
| Q3 | G3, G4 and G5 end `stopped` at the gate with no step | Yes, on §2.5's evidence: `phase-7.md` §6 stops an item that cannot reach its target, and no fix that keeps the pixels and the seam is known for any of the three. The owner may ask for bloom's pyramid lever, which wants a look review of its own |
| Q4 | 7.15 in two plans | Yes: recorded in `phase-7.md` §2.8 on 2026-11-21. G2's fix is a renderer design this session did not probe, and S-14 rides with it |
| Q5 | An item kept on two rows | A target on each and a verdict a row (§2.1, §2.2). G1 is expected `done` on 01's area rows and `stopped` on `gpu`, whose lights are the work |
| Q6 | The dark-light drop lands in Step 2, though "a dark light holding no slot" is G6's line | Yes: it is part of what the list packs, and exact. Its test past the cap, and under it, lands in Step 4 with G6's other |
| Q7 | The priority as a `u8` field on `Light`, not a marker component | Yes: one concept in one component and one file shape; a marker would widen the extract's query and add a registered name. The cost is four bytes a light and three pins' bytes (§2.4) |
| Q8 | `lights_view` public | Yes: the extract calls it across the crate seam, and a game that packs its own lights wants the same rule |
| Q9 | Step 1's quiet reading drops an item this session kept, or keeps one it dropped | The gate's reading stands: `dropped` with its number, its steps skipped (G1 on `gpu` alone: Step 3 still runs for the areas' sake when its sitting can show a fall); an item newly kept on a row follows §2.5 or stays `open` |
| Q10 | A sitting reads a row `regressed` that the step's code cannot reach | One rerun of that row's sitting; `regressed` again is a failed done-when |
| Q11 | The run outgrows a session | The next resumes at the first step without an evidence row |
| Q12 | The plan runs uncritiqued (§7) | Yes, the owner's choice of 2026-11-21. What a round would have checked first is what the run meets first: Step 1's readings against §2.1's, and each step's stills, pins and sitting |

Each question takes its stated default; the owner reviews the plan on their own time.

## 5. Invariants

- **May move:** `pit.json` at Step 2, down; `gpu.json`, `gpu-throughput.json` and `pit.json` at Step 4, `allocated_bytes` alone, to A21's values or lower; `api/tungsten-render.txt` (two lines, Step 2) and `api/tungsten-core.txt` (four, Step 4); the lights of one render test (Step 2); core's shader-lines test, one line added (Step 3); the light's file shape, one optional field (Step 4).
- **Must not move:** a drawn pixel in Steps 2 and 3: the six area stills byte for byte, `gpu-visual.png` and every fixture `just visual` compares; the GPU counter pins and every row of `scripts/perf-rows.json`; every row's digest, in every pair of every sitting; `LightUbo`'s 544 bytes and the nine shader lines `light_from` mirrors; the determinism and containment hashes and `KIT_RECIPES_HASH`; every save and every asset file; 01's tests but the one Step 4 adds; no `unsafe`, no new device feature, no new dependency; outside a measurement, no file of `apply_knock.py`'s groups.

## 6. Stop conditions

- A done-when check fails, or a file outside the step's list is needed: the skill's restore, the step skipped with its reason, the pass carrying on. Step 3 needs Step 2's sitting only for its order: skipped Step 2 leaves Step 3 runnable, its target then read against Step 1's. Step 4 needs neither, but its dark-light and out-of-reach tests need Step 2; with Step 2 skipped it lands the priority and its ranking test alone and P7-11 stays `open`. Steps 2 and 3 need Step 1's readings: this session's do not stand in for them.
- A still differs, or `just visual` fails: the mechanism is wrong, not the fixture. The step stops; no fixture or 01 test is edited to pass.
- A step's metric does not fall, or its sitting reads a row `regressed` (after Q10's rerun): one revision, then the same.
- A sitting or a capture refuses to start (an encoder, a busy process): wait for quiet; the owner's client is the owner's to disconnect.
- A knock-out edit's assertion fails (the tree moved under it): fix the tool's text in `perf-runs/`, never the tree, and say so in the evidence row.
- Step 1 cannot take a reading after one retry on a quiet machine: it records what it has, the item whose reading is missing stays `open` with its steps not run, and the first commit of the run sets `status: in progress`.
- Step 1 leaves a file of the knock-out groups changed: nothing is committed until `git status --short crates examples assets` prints nothing.
- The close sitting reads a row `regressed`: the plan is not deleted and stays `in progress`; the evidence row names the row and metric, the step is found by the step sittings, and it is fixed or reverted by a commit of its own before a second close sitting.

## 7. Critique

Not run: `critique: codex exec failed (exit 1, usage limit reached)`, Codex answering "You've hit your usage limit … try again at Nov 23rd, 2026 2:10 AM". Round 1 was attempted on 2026-11-21 on this plan as it stood before its commit (`critique --model gpt-6-astra --raw --repo`, xhigh); its footer reads `commands 0, usage 5h 0% → 0%, week 100% → 100%`, the weekly window read 100 % before it, and no model call was made (`perf-runs/20261121-gpu-pass-plan/critique/round1.stderr`). The owner, asked whether to buy credits, wait for the reset or skip, chose to skip the round. No finding is recorded, and §1's prototype runs are the author's checks, not a critique's.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-11-21 | done | A quiet machine, no `nxcodec.bin` at any reading; `df -h .` 73 GB free. `apply_knock.py` → exit 0, every group applied. `knock_areas.sh` at `REPEAT=3` (the five labels in four foreground calls on one folder) → `control`, `unlit`, `fill`, `particles`, `lightlog`: exit 0 each, all 30 captures `valid`, each row's digest the same under every label. `gate_areas.py` → exit 0; clearing / aqueduct / ravine / hollow / mine / bridge: **G1** 12.858 / 16.899 / 5.269 / 24.691 / 16.080 / 9.378 ms, 58.02 / 64.31 / 37.81 / 72.17 / 63.09 / 50.98 % of `render_span`, kept, **targets at most 6.429 / 8.450 / 2.634 / 12.345 / 8.040 / 4.689**; **G2** 2.664 / 2.873 / 2.358 / 2.629 / 2.777 / 2.507 ms, 12.02 / 10.93 / 16.92 / 7.69 / 10.90 / 13.63 %, kept; **G3** 5.508 / 5.506 / 5.434 / 5.564 / 5.501 / 5.425 ms, 24.85 / 20.95 / 38.99 / 16.26 / 21.59 / 29.49 %, kept; **G4** 0.002–0.008 ms, 0.01–0.02 %, under the floor. `lightlog`: `world 23 entries 23 dark 1 packed 16` in five areas, `dark 2` in the hollow. Bench rows by hand (`RUNS=3`, every run `exit 0, sampled 300/300`): `gpu` scene 6.929 ms and 5.191 under `unlit`: **G1 1.738 ms, 16.3 % of a 10.639 ms span, kept, target at most 0.869**; the lit path at `lights=0` / `4` / `8` / 16: 0.163 / 0.597 / 1.026 / 1.738 ms (scene 5.355 / 5.787 / 6.215 / 6.929, under `unlit` 5.192 / 5.190 / 5.189 / 5.191); bloom 1.178 and vignette 0.261 (G3 13.5 %, kept); at 3840x2160 `aa=off` scene 8.784, bloom 4.656, vignette 1.797, and with nothing drawn 0.250, 3.654 and 1.685; at 1080p with nothing drawn bloom 0.999, vignette 0.251. `particles` scene 2.340 of a 2.359 ms span and 0.343 with emission out: **G4 1.997 ms, 84.7 %, kept**; at `rate=0.5` 1.219 and 0.343. `restore_knock.sh` → exit 0; `git status --short crates examples assets` → prints nothing. On the restored tree `just perf run gpu-throughput` → exit 0, `valid`, its log `Compiling` tungsten-render, tungsten and example-02-bench; `just perf run gpu` → exit 0, `valid`, on that build (`Finished`, nothing left to compile). `stage_share.py` → `render_encode` mean 1.597 of 12.872 ms (12.41 %), p50 1.424 of 12.844 (11.09 %): **G5 kept, target at most 0.799**; `faults.py` five times → 5.3, −5.2, 5.5, 3.8 and −2.9 faults a frame; the gated profile → `frames 365, exit 0, window frames profiled 300`, 11.48 % of the window's cycles under `record_main_draws`, 99.2 % of them libc's, `render_encode` 1.554 of 12.908 ms in that run. `gpu`'s five GPU runs read the scene pass p50 6.94, 6.93, 6.94, 6.94 and 6.94 ms: the two-values lever closed. §2.5's evidence held for all three, so **G3, G4 and G5 read `stopped`**; G1 and G2 are kept and G6 stays open. One reading against the plan's: under `gpu`'s own scene at 3840x2160 the post stack reads 6.453 ms against 5.339 with nothing drawn, so up to a sixth of it on that bench follows what is drawn; the areas read 5.43–5.56. `rg -n '^\| G[1-6] \|.*\| open \|$' docs/plans/phase-7.md; test $? -eq 1` → exit 0; `rg -c '^\| S-23 \|[^|]*\| 7\.15 \| landed \|' docs/plans/phase-7.md` → 1, the cell reading `landed` with its commit in the note, as the check spells it; `cargo fmt --all -- --check`, `git diff --check` and `just repo-check` → exit 0. Beyond the plan's list: `unlit` at each light count and the empty scene at 1080p | `perf-runs/715-gpu-pass/step-1/` (`areas/gate.txt`, `table.txt`, `passes.txt`, `index.tsv`; `bench/`; `g5/`), `perf-runs/20261122T022739Z-gpu-throughput/`, `perf-runs/20261122T022830Z-gpu/` |

## Follow-ups

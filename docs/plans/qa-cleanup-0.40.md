# 0.40 QA, debug and cleanup pass

- **status:** in progress
- **goal:** Fix the correctness bugs this audit found, remove the noise that built up in code and tests over past releases, and reshape the repository so coding agents need fewer and more targeted reads, without changing engine behavior outside the bug-fix steps.
- **non-goals:** New features, performance optimization, cutting or preparing the 0.40 release, deleting anything under `perf-runs/`, editing `docs/plans/1.0-criteria-draft.md` or `docs/plans/ui-text-suite-draft.md`, rewriting existing `DECISIONS.md` or `CHANGELOG.md` entries, and the deferred items listed under "Follow-ups".
- **files to touch:** This file only until the plan is approved. Execution touches `crates/**`, `examples/**`, `scripts/{bench.py,bench_report.py,test-bench.py,check-repo.py,test-check-repo.py}`, `justfile`, the workspace and member `Cargo.toml` files and `Cargo.lock`, `.github/workflows/{ci,release}.yml`, `.claude/settings.json`, `.claude/skills/tungsten-perf/SKILL.md`, `crates/tungsten-render/AGENTS.md`, `docs/{LLM_INDEX,agent-setup,known-issues,DECISION_INDEX}.md`, `docs/perf/profiling-workflow.md`, `examples/02_bench/tests/fixtures/README.md`, `DESIGN.md`, new `DECISIONS.md` entries, `CHANGELOG.md` `[Unreleased]` lines and this plan's status and evidence log.
- **ordered steps:** Step 0 captures the baselines; steps 1–3 tighten the gates; steps 4–20 remove noise, restructure, add tooling and fix docs with behavior unchanged; steps 21–28 fix one bug each, failing test first; step 29 compares against step 0 and closes the plan.
- **done-when:** Every step's checks pass and are quoted in the evidence log; step 29's suite compare against `qa-0.40-pre` shows no owned metric `regressed` without a recorded justification; the determinism hash, the pinned containment hash and every row digest equal step 0's values except where a bug-fix step recorded the change; `just visual` passes with `examples/02_bench/tests/fixtures/gpu-visual.png` unchanged; `status` reads `done`.

Date: 2026-10-03, on branch `0.40` at `afbc330` (workspace `0.39.0`, `CHANGELOG.md` `[Unreleased]` empty). Audit commands ran on the reference machine; no tracked file changed during the audit.

**Context digest.** `just check` passes on the clean tree (exit 0). Under `cargo nextest run --workspace` 928 tests pass and 7 are skipped; one test, `example-01-platformer` `tests::authored_routes_and_recovery_shelves_traverse_with_real_physics`, takes 82.3 s and sets the whole test phase. Git is human-only: every step ends with a [tungsten-patch-handoff](../../.claude/skills/tungsten-patch-handoff/SKILL.md) series and the human commits it. `docs/plans/1.0-criteria-draft.md` carries an unrelated uncommitted edit, so always pass explicit paths to `patch-series.py cut`. Cleanup and structure steps must keep four things identical to step 0: the determinism hash and the pinned containment hash (`just physics-release -- --nocapture`), each benchmark row's digest (`capture.json` `.determinism.digests`), the `gpu-visual.png` comparison and the post/transition regression tests (`just visual`). Perf work follows the [tungsten-perf](../../.claude/skills/tungsten-perf/SKILL.md) skill: baseline `qa-0.40-pre` from step 0, and steps that touch ECS, physics or render hot loops capture their rows against it and must read "not `regressed`". Budgets are tight: root `AGENTS.md` is 6,127 of 6,144 B and `docs/LLM_INDEX.md` 7,593 of 8,192 B (`just ctx`). Decisions take new entries only (Q1).

## Findings

IDs are referenced by the steps. "Evidence" cites the tree at `afbc330` or a command run on 2026-10-03.

### Bugs

| ID | Severity | Defect and evidence | Reproduction | Step |
| --- | --- | --- | --- | --- |
| B1 | High | **Material batches of one material share one UBO, so every batch draws the frame's last payload.** `crates/tungsten-render/src/material.rs:59-64` creates one 256-byte UBO per material; `crates/tungsten-render/src/sprite.rs:980-991` writes each batch's payload into it with `queue.write_buffer` (`:988`) during recording, and wgpu applies every queued write before the frame's command buffer runs. `crates/tungsten/src/sprite_extract.rs:7-10` and `:305-324` split batches by override hash precisely so overrides "never alias through one UBO upload". `D-090` (`DECISIONS.md:914`) fixed the same failure for post params. Live in the `integrated` row: `examples/02_bench/src/integrated/runtime.rs:236-265` gives each struck walker its own `damage_flash` block with a fading tween. | Two sprites with the root manifest's `damage_flash` material, one override red at amount 1.0 and one blue at 1.0, in one frame: both draw blue. | 27 |
| B2 | High | **A shader hot reload that passes Naga but not pipeline validation panics the app.** Nothing in the repo pushes an error scope or sets an uncaptured-error handler (`rg 'push_error_scope\|on_uncaptured_error' crates examples` finds nothing), and wgpu 30.0.1's default handler panics (`~/.cargo/registry/src/*/wgpu-30.0.1/src/backend/wgpu_core.rs:692-694`). `crates/tungsten-render/src/renderer.rs:704-820` validates with Naga only (`shader_hot_reload.rs:121-134`, `Capabilities::all()`), then rebuilds pipelines; `upload_material` (`renderer.rs:834-884`) builds without a scope too. `D-057` (`DECISIONS.md:405`) promises that a rebuild failure logs and keeps the live pipeline. Release builds use `panic = "abort"` (`Cargo.toml:82`). | Start the playground on a staged tree (`examples/04_shader_playground/tests/post_regression.rs:69`, `stage_with_edited_stock_shader`) whose `fade.wgsl` reads an extra `@group(2) @binding(0)` uniform in `fs_main`: the process panics with "wgpu error: Validation Error" instead of drawing the shipped fade. | 26 |
| B3 | Medium | **Hot-reloading a sprite to a smaller image draws it squeezed.** The in-place shrink path keeps the old UV, which covers the whole packed cell (`crates/tungsten/src/asset_loader.rs:885`), but stores the new, smaller size (`:985-1046`, `update_sprite_entry` at `:1044`). Extract draws `width × scale` with that UV (`sprite_extract.rs:360-371`), so the image and its transparent tail shrink by the size ratio once more. `DESIGN.md:203` lists shrink as supported. No test covers the path. | With hot reload on (example 01), overwrite a registered 32×32 PNG with a 16×16 one: the sprite shows an 8×8 image in its 16×16 quad. | 25 |
| B4 | Medium | **A completed tween's deferred removal deletes a replacement tween inserted the same frame.** `crates/tungsten/src/tweens.rs:52-71` marks a finished tween and `:111` queues `remove_component::<Tween>`. Systems run before the tween stage (`crates/tungsten/src/app.rs:1589-1598`), so a replacement queued through `CommandBuffer` is applied first at flush (`crates/tungsten-core/src/ecs/world.rs:427-457` overwrites in place) and the queued removal then deletes it (`:458-461`). `crates/tungsten/src/game_feel.rs:63-71` documents this exact hazard for squash and works around it; tweens do not. Latent in the repo: the platformer inserts directly (`examples/01_platformer/src/systems.rs:489`) and the integrated bench guards with `flashing` (`runtime.rs:245-249`). `D-056` (`DECISIONS.md:391`) chose the unconditional removal. | CPU test: entity with a `Tween` of duration `dt`; queue a replacement `Tween` through the `CommandBuffer`; run `tween_tick_system`; flush. The entity has no `Tween`; it should hold the replacement. | 21 |
| B5 | Low | **Camera smoothing depends on the frame rate.** `crates/tungsten/src/camera.rs:81-82` lerps by `smoothing_factor` once per frame, so at 144 Hz the camera converges 2.4 times faster than at 60 Hz. The playground uses 0.12 (`examples/04_shader_playground/src/main.rs:542`); the platformer and the integrated bench use 1.0, which no fix changes. | CPU test: follow camera with smoothing 0.5; one second of 60 frames at 1/60 s and one of 120 at 1/120 s end at different positions. | 24 |
| B6 | Low | **Scene tweens accept a vec4 lane above 3 and silently drive lane 3.** `crates/tungsten-core/src/assets/scene.rs:183-194` validates duration and channels only; `crates/tungsten/src/tweens.rs:170` clamps `lane.min(3)`; `crates/tungsten-core/src/tween.rs:332` documents `0..=3`. | `SceneData::load` on a scene whose tween has `{"kind": "uniform_vec4_lane", "slot": "v0", "lane": 4, ...}` returns `Ok`. | 23 |
| B7 | Low | **Invalid `render.msaa` and `render.bloom_max_mips` in `tungsten.json` are reported as env overrides.** `crates/tungsten-core/src/config.rs:326-339` returns `ConfigError::InvalidEnvOverride { var: "render.msaa", .. }`, whose message reads "invalid env override render.msaa='3'" although no variable was set. Only `config.rs` and its tests match `ConfigError` variants. | `Config::load` on a temp file with `{"render": {"msaa": 3}}`, no env vars: the error text says "env override". | 22 |
| B8 | Low | **The broadphase claims the next cell for an AABB whose max edge sits exactly on a cell boundary, once the edge is four or more cells from the origin.** `crates/tungsten-core/src/physics/broadphase.rs:320-330` subtracts `f32::EPSILON`, which is below one ULP from 4.0 up; the comment at `:324` promises the opposite, and the test (`crates/tungsten-core/src/tests/physics/broadphase.rs:268-270`) checks cells 0 and −1 only. The step inflates every pair AABB by at least `2·linear_slop`, so exact alignment is rare there: the cost is false candidates and the dedupe path, not wrong contacts. | Scratch probe against `tungsten-core` (cell 32): an AABB over x 32–64 does not reach a query over x 65–95, but one over x 320–352 reaches a query over x 353–383. | 28 |

The open findings in `docs/known-issues.md` (audio channel mapping, state-transition ownership, skipped-acquire capture accounting, shader ID allocation, physics input domain) still match the code paths read in this audit; they are not repeated here.

### Noise

| ID | Finding and evidence | Step |
| --- | --- | --- |
| N1 | `crates/tungsten-core/tests/physics_timing.rs:73,95`: two tests that assert nothing and print debug-build timings (0.20 s each under nextest). | 4 |
| N2 | `crates/tungsten-core/tests/substep_probe.rs:187,196`: two `#[ignore]` probes tied to a finished plan ("Step 5 (D-066) done-when", `:199`); their spawn helpers duplicate `physics_determinism.rs:30-90` and `physics_containment.rs:38-97`. | 4 |
| N3 | Nine shader tests repeat `crates/tungsten-render/tests/shader_coverage.rs:128` (`every_workspace_shader_validates_and_mirrors_match`, which Naga-validates every `.wgsl` under the render sources and `assets/shaders` and byte-checks the stock mirror): `src/tests/bloom.rs:66,73,80,87,94` and `src/tests/smaa.rs:74,81,88,95`. | 5 |
| N4 | Example shaders have uneven coverage: `examples/01_platformer/src/tests/main.rs:2324` validates `soft_glow.wgsl` alone; `examples/02_bench/assets/shaders/bench_heavy.wgsl` is validated by no test. | 5 |
| N5 | Unused dependencies (`rg` over each crate's `src`, `tests`, `benches`): `image` in `crates/tungsten-core/Cargo.toml:17`; `wgpu` in `crates/tungsten/Cargo.toml:13`; `tungsten-core` in `examples/02_bench/Cargo.toml:10`. `tungsten-render` (`examples/02_bench/Cargo.toml:11`) serves only `tests/visual_regression.rs:13`, and the playground reaches `tungsten-core` directly (`examples/04_shader_playground/Cargo.toml:10`, `src/main.rs:43-44,1152,1193`) where the other examples use `tungsten::core`. | 6 |
| N6 | `crates/tungsten-render/src/sprite.wgsl` is compiled by nothing: both users include `assets/shaders/sprite.wgsl` (`renderer.rs:304`, `sprite.rs:229`), as `D-057` says ("share one file on disk"). It survives only as a mirror pair (`tests/shader_coverage.rs:18,163`) and a rule in `crates/tungsten-render/AGENTS.md:8` that makes agents edit two files. | 7 |
| N7 | Dead render API and fields: `PostStackRenderer::record_bloom_slot` (`post/mod.rs:495-514`, no caller) and the `BloomPipeline::record_pass` it alone calls (`post/bloom.rs:310-328`); `PostStackRenderer::final_target` (`post/mod.rs:428-440`, test-only); `MaterialPipeline::write_uniforms` (`material.rs:132-137`), `MaterialBuildError` (`material.rs:139-141`, `lib.rs:32`) and the never-read `material_bind_group_layout` field (`material.rs:20`, set at `renderer.rs:875`); `Renderer::capture_armed` (`screenshot.rs:63-66`) and the never-built `ScreenshotError::DeviceLost` (`:23-24`); `requested_present_mode_label` wrapping `as_str` (`surface.rs:18-20`); `create_capture_target`'s unused `_format` (`renderer.rs:1771-1776`); `#[allow(dead_code)]` over never-read view and filter fields (`sprite.rs:130-151`, `targets.rs:42-44`); `TextPipeline::new`'s sample-count and depth parameters, which its one caller fixes at `1, false` (`text.rs:96-117`, `renderer.rs:275`). | 8 |
| N8 | Dead core API: `AnyColumn::len`/`type_id` (`crates/tungsten-core/src/ecs/archetype.rs:95-106`), `Archetype::id` (`:170-171`), the test-only `row_count` (`:289-292`, used at `src/tests/ecs/archetype.rs:203,217`), `Shape::min_half_extent` (`physics/components.rs:37-44`, no caller; the step keeps its own on `Proxy`), and `LightKind::Point::falloff` (`components.rs:98,109`), which nothing reads: `crates/tungsten-render/src/lighting.rs:83` matches `Point { radius, .. }` and the lit shader has no falloff. | 9 |
| N9 | Stale comments and citations of archived plans that agents may not open: `physics/step.rs:69-74` and the headers of `tests/physics_determinism.rs:1`, `tests/physics_containment.rs:1`, `tests/physics_tunneling.rs:219-220` cite `docs/plans/physics-scale-and-ccd.md`; `examples/02_bench/tests/fixtures/README.md:70` cites `docs/plans/archive/benchmark-suite-redesign.md`; "M26 plan" in `core/post.rs:395-396`, `render/post/mod.rs:5`, `examples/04_shader_playground/src/main.rs:1110`; "see plan" in `tungsten/sprite_extract.rs:307` and `render/sprite.rs:173`; "Step-9 invariant" in `render/src/tests/passes_order.rs:197`; "Step 2 … step 4" in `core/src/tests/physics/step.rs:792-795`; "the 17 stock effects" (now 18) in `core/post.rs:3`; "substep picker" (removed by `D-064`) in `physics/step.rs:2065`; a nonexistent `asset_loader::material::load_materials` in `core/assets/manifest.rs:227`; "D-022 double-free panics" over a `debug_assert` in `ecs/entity.rs:76-87`; `half_w`/`half_h` holding full extents in `core/camera.rs:208-209`. | 10 |
| N10 | `scripts/check-repo.py:31-33,88-89` keeps a deletion-candidate entry for `examples/01_platformer/assets/sprites/player.png`, which the manifest now registers, so the branch never runs (`docs/known-issues.md:47`, `docs/agent-setup.md:60`). | 12 |
| N11 | `justfile:95-96`: `just --list` shows only the second line of `quick`'s comment (`docs/known-issues.md:46`); `justfile:73-74` runs `python3` without `-B` where every other recipe uses it. | 12 |
| N12 | Both workflows install `libudev-dev` (`.github/workflows/ci.yml:51`, `release.yml:132`); no crate in `Cargo.lock` links udev (`docs/known-issues.md:45`). | 12 |
| N13 | `examples/01_platformer/src/state.rs:213-221` keeps an unused `CycleMode::None` behind `#[allow(dead_code)]` "for future fixture variants". | 13 |
| N14 | Local only: an empty, untracked `.benchmarks/` directory at the repository root. Delete by hand; no step. | — |

### Structure and agent ergonomics

| ID | Finding and evidence | Step |
| --- | --- | --- |
| S1 | `crates/tungsten-core/src/physics/step.rs` is 2,278 lines: types, impulse map, sleep table, pair budgets and repair, arrival pass, speculative sweep, narrow phase and tile gather in one file, which the last two physics changes edited (`D-092`, `D-094`). Every physics task opens it whole. | 17 |
| S2 | `crates/tungsten/src/asset_loader.rs` is 1,830 lines. The lit-sibling decode and emissive premultiply appear three times (`:226-290`, `:909-968`, `:1096-1165`, each behind `#[allow(clippy::many_single_char_names)]` at `:217,853,1057`) and the albedo/normal/emissive page blit twice (`:119-157`, `:1249-1287`). | 15 |
| S3 | Seventeen four-line modules in `crates/tungsten-render/src/post/` (`crt.rs`, `fade.rs`, …) each hold a name and an `include_str!` that `post/mod.rs:43-61` collects into `STOCK_SHADERS`. | 14 |
| S4 | `docs/LLM_INDEX.md:3` says unit tests live in `src/tests/` behind `#[path]`, but `crates/tungsten-render/src/targets.rs:690`, `timing.rs:188` and `post/smaa_luts.rs:131` keep inline test modules. | 14 |
| S5 | `tungsten-core` returns `anyhow::Error` at its boundary (`assets/animation.rs:20`, `assets/tilemap.rs:89`, `assets/audio.rs:23`), against `AGENTS.md`'s `thiserror` rule; it is the crate's only use of `anyhow`. | 16 |
| S6 | `docs/LLM_INDEX.md` routes no task to the smoke runner (`scripts/smoke-examples.sh`), the pixel tests (`examples/02_bench/tests/visual_regression.rs`, `examples/04_shader_playground/tests/post_regression.rs`, `examples/03_scene_state/tests/transition_regression.rs`), `scripts/check-agent-context.py` or the Criterion benches (`crates/*/benches/`). `docs/agent-setup.md:44` narrates a 2026-09-25 review that tells an agent nothing actionable. | 20 |
| S7 | Budgets leave little room: root `AGENTS.md` 6,127 of 6,144 B, `docs/LLM_INDEX.md` 7,593 of 8,192 B (`just ctx`, `scripts/check-agent-context.py:22-26`). Every agent-file step must be byte-neutral on `AGENTS.md`. | all |
| S8 | `just check` spends 84.6 s in the platformer test binary (`cargo test -q` log), 82.3 s of it in one test (`examples/01_platformer/src/tests/main.rs:986-1004`, simulation helper `:899`), because each attempt rebuilds the full 184×50 map world and steps physics in an opt-level-0 `tungsten-core`. | 3 |

### Tooling

Nothing below is installed until the plan is approved. "Gate" says whether `just check` would depend on the tool.

| ID | Problem it removes here | Wiring | Install | Gate |
| --- | --- | --- | --- | --- |
| T1 | The determinism hash is only printed, and `just physics-release` hides it without `--nocapture` (`tests/physics_determinism.rs:133-141`), so every physics step compares it by hand. Pin it as `physics_containment.rs:300-320` pins its pile. | `crates/tungsten-core/tests/physics_determinism.rs`; runs in `just physics-release` | none | no (release-only test) |
| T2 | S8: the gate waits 82 s for one test. Building only `tungsten-core` at opt-level 1 in dev took that test from 82.3 s to 9.8 s, same result; rebuilding core and its dependents after `cargo clean -p tungsten-core` went from 5.0 s to 6.8 s (scratch target dirs, `--config profile.dev.package.tungsten-core.opt-level=1`). Floats keep IEEE semantics at any opt-level and `debug_assertions` stay on. | `Cargo.toml` `[profile.dev.package.tungsten-core]` and its comment (`:63-67`); new decision | none | yes: makes it faster |
| T3 | The capture rules make the agent check by hand, before and during every sitting, for a remote-desktop encoder, for other `cargo`/`rustc` work and for another session editing the tree, because "the runner records no background-load provenance" (`docs/perf/profiling-workflow.md:22-33`, which documents three captures spoiled that way while every guard passed; step 0's A′ was a fourth). | `scripts/bench.py` samples `/proc` before and during each measured run and rechecks the commit and dirty-tree hash after it, records offenders in provenance and invalidates the run unless `--allow-background`; `scripts/test-bench.py`; profiling workflow and `tungsten-perf` skill lines; new decision (amends `D-078` validity) | none (stdlib, `/proc`) | no (`just perf-test`) |
| T4 | Cleanup steps must prove digests unchanged, but compare never looks at them ("Digests aren't compared across captures", `profiling-workflow.md:142`). | `scripts/bench_report.py` compare reports, per row, whether baseline and candidate digests match; informational, no verdict | none | no (`just perf-test`) |
| T5 | Nothing catches a citation of a plan that moved to the archive (N9). | `scripts/check-repo.py` fails on `docs/plans/<name>.md` cited in maintained code, scripts or docs when the file is absent (archive citations stay notes, as today); `scripts/test-check-repo.py` | none | no (`just repo-check`, `just quick`) |
| T6 | N4: example shaders reach Naga only by accident. | `crates/tungsten-render/tests/shader_coverage.rs` also scans `examples/*/assets/shaders` | none | yes (`cargo test`) |
| T7 | N5 accumulated unseen; nothing reports unused dependencies. | `just udeps` running `cargo shear`; `docs/agent-setup.md` row; not in CI | `cargo install --locked cargo-shear` | no |
| T8 | `release.yml` is unlinted (`docs/known-issues.md:43`; it passed `actionlint` 1.7.12 by hand). | `actionlint` in `just script-test`; pinned download plus checksum in `ci.yml` beside ShellCheck; `docs/agent-setup.md` | `pacman -S actionlint` locally; CI downloads 1.7.12 | no (`script-test`) |
| T9 | CI runs six recipes; no single local command mirrors them. | `just ci: check bench-build deps ctx repo-check script-test`; `docs/agent-setup.md` row | none | no |
| T10 | `just physics-release`, which every physics step runs, prompts for permission: `.claude/settings.json` allows the other shared recipes exactly. | add `Bash(just physics-release)`; `docs/agent-setup.md:30` | none | no |
| T11 | Considered, not proposed: a tracked Bash hook guarding `docs/plans/archive/`. `docs/agent-setup.md:33` defers hooks, the `Read` deny covers the common path and `AGENTS.md` states the rule. Considered, not proposed: a new skill; the existing five cover this plan, and step 1 adds T3/T4 to `tungsten-perf`. | — | — | — |
| T12 | Optional, deferred: `lychee --offline --include-fragments` would check the anchors `docs/README.md:28` asks agents to check by hand. | a `just links` recipe | `cargo install --locked lychee` | no |

### `perf-runs/` prune list (proposal only; delete nothing in this plan)

`du` over `perf-runs/` on 2026-10-03: 50,999 MiB in 660 entries. Sizes are MiB.

| Tier | What | Size | Why it can go |
| --- | --- | --- | --- |
| P1 | Reference-build trees: `20261001-gpu-pass-session-a/refbuild` (14,726), `20261002-dense-pile-fix/verify/target` (10,077) and `refbuild` (2,607), `20261001-gpu-pass-a2-b3/refbuild` (6,910), `20261002-m31-evidence/{base,var}` (2,551, mostly `target/`), `20261002-gpu-pass-session-b/refbuild` (1,270), `20261002-p2-pass/refbuild` (1,311) | ≈ 39,450 | Scratch workspace copies with build output; their commits are recorded and the session scripts rebuild them. |
| P2 | 142 captures without `capture.json` from the retired bash runner, 2026-04 to 2026-09 (`*-ecs-high-load*` 65, `*-physics*` 56, `*-sprite*` 18, `*-platformer` 2, `*-render-features` 1) | 6,261 | Retired workloads; they cannot be baselines or compared (`profiling-workflow.md:100`). |
| P3 | `perf record` data (`*.data`) outside `perf-runs/baselines/` | ≤ 7,275 (overlaps P1/P2) | Keep the flamegraphs and reports; the raw records are reproducible. |
| P4 | Optional, owner's call: the 73 runner captures, suites, compares and capacity searches from 2026-09 | 298 | Superseded by the 2026-10 baselines. |
| Keep | `perf-runs/baselines/` (264), the scripts, logs, READMEs and shots of the evidence folders above, `20261001-gpu-pass-evidence/`, `20261002-p2-gate-probe/`, `20261002-dense-pile-collapse/`, `20261001-physics-ecs-pass-*`, `.gitkeep` | — | Session tooling and probes that later work reuses. |

## Audit coverage

Read in full: `tungsten-core` sources except tests and the post-effect parameter structs; `crates/tungsten-render/src/{renderer,sprite,text,material,timing,lighting,mesh_particle,screenshot,image_diff,shader_hot_reload,surface,targets}.rs`, `post/{mod,bloom}.rs`; `crates/tungsten/src/{app,asset_loader,audio,hot_reload,particles,state,transition,tweens,game_feel,camera,display,light_extract,post_aa,input_bridge,telemetry,tilemap_extract}.rs`; `scripts/{check-agent-context,check-repo}.py`, `scripts/smoke-examples.sh`, `justfile`, workflows, manifests, agent files and docs named above. Skimmed: `render/{post/smaa,post/fullscreen,passes/*,quad,debug_line,lit_sprite,surface_acquire}.rs`, `tungsten/src/{debug_hud,inspector,systems_overlay,physics_debug,sprite_extract}.rs`, the examples (harness, extract, gameplay entry points, tests) and the tests (scanned for assertion-free and duplicate tests). Covered by their own suites plus a pattern scan only: `scripts/{bench,bench_report,release,release-preflight,patch-series}.py` (`just script-test`: exit 0) and `tools/launcher`. Not reproduced on the GPU during the audit: B1 and B2, to keep the machine free for captures; their failing tests come first in their steps.

## Execution rules

- One step per session. Read this plan, then only the files the step names. Set `status: in progress` in step 0; keep it until step 29.
- Before the first edit: `python3 -B scripts/patch-series.py init --out <scratchpad>/series`. After the checks pass: `cut --out <scratchpad>/series --name NN-slug --msg-file <msg> <paths…>` with **explicit paths** (this plan file included, `docs/plans/1.0-criteria-draft.md` never), then `verify` and `script`; hand the human `commit.sh`. Commit subject: `0.40 QA step NN: <summary>`, no attribution lines. The next step starts after the human has run it.
- Each step adds one line to `CHANGELOG.md` `[Unreleased]` (Added, Changed, Fixed or Removed) and leaves released sections alone. A bug-fix line states whether the hash, a digest or a reference image moved.
- A step that fixes a finding listed in `docs/known-issues.md` removes that entry in the same patch. Anything new and out of scope goes under "Follow-ups" here.
- New decisions use the [tungsten-decision](../../.claude/skills/tungsten-decision/SKILL.md) skill for the ID, shape and `docs/DECISION_INDEX.md` row (the old row may gain "Amended by"). Per Q1 the default writes no marker line into an existing `DECISIONS.md` entry. Expected IDs in step order: `D-095` (step 1), `D-096` (step 3), `D-097` (step 21), `D-098` (step 24), `D-099` (step 27); take the next free ID at the time.
- Gates by change, from the `AGENTS.md` table: `just check` always; manifests, assets or the core/render seam add layer 1 (`cargo test -p tungsten-core --test manifests`); engine or example wiring adds `WGPU_BACKEND=vulkan just smoke`; scripts or perf tooling add `just script-test`; dependency changes and non-trivial steps run both layers; physics files add `just physics-release -- --nocapture`; agent files (`AGENTS.md`, skills, `docs/LLM_INDEX.md`, `docs/agent-setup.md`, `.claude/`) add `just ctx`; docs add `just repo-check` and `git diff --check`; render, extract or example drawing adds `just visual` on the reference machine.
- Invariants for steps labelled cleanup, structure, tooling or docs: the two hashes and the row digests from the evidence log, `gpu-visual.png` and the passing post/transition tests. A step that moves one stops and reports instead of patching around it.
- Perf captures follow the `tungsten-perf` skill and `docs/perf/profiling-workflow.md`: quiet machine (`pgrep -x nxcodec.bin`, `pgrep -x 'cargo|rustc'`, no other session in the tree), each sitting one blocking foreground command, release build with the runner's flags. Row captures use `WGPU_BACKEND=vulkan just perf suite --only <rows> --repeat 5 --compare qa-0.40-pre` and must read "not `regressed`" on every owned metric. A `regressed` row the step did not touch goes through the placement procedure ("Predict code placement" and the regression policy in the profiling workflow) before anyone calls it real. Peak RSS compares only within one sitting.

## Steps

### Step 0 — Baselines and evidence (tooling)

Findings: none. Files: this plan (status, evidence log).

1. Check the machine is quiet (rules above), then `WGPU_BACKEND=vulkan just perf suite --repeat 5` twice back to back on the untouched tree (A, then A′), and `just perf compare <A> <A′>` as the A/A. Record any owned metric that is not `unchanged` as this machine's noise (`gpu-throughput` `render_encode` and the `ecs` per-run-mode rows are known to drift).
2. `just perf baseline save <A> qa-0.40-pre`.
3. Record the row digests: `jq -r '"\(.row) \(.determinism.digests[0]) \(.determinism.match)"' perf-runs/<A>/*/capture.json`.
4. `just physics-release -- --nocapture 2>&1 | rg 'state hashes|pinned step|containment:|slow frames'`; record the determinism hash and the pinned hash.
5. `just visual`; `just check` with its wall time; `cargo test -p example-01-platformer -- authored_routes` with its time.
6. Fill in the evidence log; set `status: in progress`.

Done when: the A/A shows no owned metric `regressed` or `improved` beyond the documented drifts (else recapture on a quieter machine), `qa-0.40-pre` exists, the evidence log holds suite paths, digests, hashes and timings, and `just repo-check` passes. Patch: this plan only.

### Step 1 — Perf runner: background-load guard and digest comparison (tooling: T3, T4)

Files: `scripts/bench.py`, `scripts/bench_report.py`, `scripts/test-bench.py`, `docs/perf/profiling-workflow.md`, `.claude/skills/tungsten-perf/SKILL.md`, `DECISIONS.md` (new entry), `docs/DECISION_INDEX.md`.

1. Before each measured run and at least once a second during it, scan `/proc/*/comm` for `nxcodec.bin` and for `cargo`/`rustc` processes outside the runner's own process tree. Record offenders under the capture's provenance (a new soft field; no hard comparability field changes). An offender during a measured run makes the run invalid with reason `background load: <name>`; `--allow-background` downgrades that to a note. Idle agent sessions are not flagged: their builds show up as `cargo`/`rustc`. After each measured run exits, recompute the commit and `git_dirty` (`scripts/bench.py:81`) and compare them with the provenance read before the build: a change makes the run invalid with reason `background load: tree changed`, which `--allow-background` also downgrades to a note, and provenance records the values seen. This catches a session that edits the tree without building, which spoiled step 0's A′ while the busy-CPU log stayed flat; a session that only reads files stays undetected.
2. Compare reports, per row, whether the first-run digests of the two sides match; information only, no verdict. Suite compare lists the rows whose digests differ.
3. Tests in `scripts/test-bench.py` with an injected process lister, an injected tree-state reader and fixture captures: clean run valid; encoder during a run invalid; tree changed after a run invalid; `--allow-background` valid with a note; digest-equal and digest-different compares.
4. Profiling workflow: replace "The runner records no background-load provenance" and the digest sentence at `:142`, and say under "No other agent session or build" that the runner flags builds and tree edits but not a session that only reads; one line each in the `tungsten-perf` skill. Record the decision (amends `D-078`'s validity list).

Done when: `just perf-test`, `just script-test`, `just ctx`, `just repo-check`, `just check` pass; one `WGPU_BACKEND=vulkan just perf run physics --repeat 3 --compare qa-0.40-pre` on a quiet machine is valid and prints "digests match".

### Step 2 — Pin the determinism hash (tooling: T1)

Files: `crates/tungsten-core/tests/physics_determinism.rs`.

1. Assert the run hash equals step 0's value (a `const EXPECTED`, as `physics_containment.rs:306` does) besides comparing the two runs; keep the `println!`.
2. Comment: the value comes from `just physics-release` (the runner's flags, generic x86-64) on the tree of step 0, and a bug fix that moves it records the new value in this plan and the changelog.

Done when: `just physics-release` passes and `just check` passes. Physics file: yes.

### Step 3 — Faster debug physics for the gate (tooling: T2)

Files: `Cargo.toml` (`[profile.dev.package.tungsten-core] opt-level = 1`, comment at `:63-67`), `DECISIONS.md` (new entry), `docs/DECISION_INDEX.md`.

Done when: `cargo test -p example-01-platformer -- authored_routes` finishes in under 15 s (82.3 s before); `just check` passes and its wall time is recorded against step 0; release profiles are untouched (`git diff --unified=1 Cargo.toml` shows only the dev entry and its comment). No perf capture: release builds do not change.

### Step 4 — Remove the physics probe tests (cleanup: N1, N2)

Files: delete `crates/tungsten-core/tests/physics_timing.rs` and `crates/tungsten-core/tests/substep_probe.rs`; move the shared `spawn_pile` and the parameterized `spawn_static_box` of `physics_determinism.rs` and `physics_containment.rs` into `crates/tungsten-core/tests/common/mod.rs`, byte-identical in effect.

Removed tests:
- `physics_step_timing_probe_1000_balls`, `full_frame_timing_probe_1000_balls`: assert nothing; debug-build wall times measure nothing the gate needs. Covered by the `physics` and `integrated` benchmark rows and `crates/tungsten-core/benches/physics_bench.rs`.
- `probe_dense_pile_10k`, `probe_dense_pile_25k`: ignored diagnostics for a finished plan. Containment is covered by `physics_containment.rs`, sleeping by the sleep tests in `src/tests/physics/step.rs` and the `physics` row's `sleeping` guard, steady-state cost by `physics_bench.rs`.

Done when: `just physics-release -- --nocapture` passes with both hashes equal to step 0's; `just check` passes. Physics file: yes.

### Step 5 — Shader-test dedupe; validate example shaders (cleanup: N3, N4; tooling: T6)

Files: `crates/tungsten-render/tests/shader_coverage.rs` (also scan `examples/*/assets/shaders`), `crates/tungsten-render/src/tests/{bloom,smaa}.rs`, `examples/01_platformer/src/tests/main.rs`.

Removed tests, all covered by `every_workspace_shader_validates_and_mirrors_match` (`shader_coverage.rs:128`), which Naga-validates every scanned `.wgsl` and byte-checks the mirrors:
- `bloom_{threshold,downsample,upsample,composite}_wgsl_naga_validates`, `bloom_wgsl_engine_and_asset_mirrors_match` (`bloom.rs:65-124`).
- `smaa_{edge,blend_weights,neighborhood_blend}_wgsl_naga_validates`, `smaa_wgsl_engine_and_asset_mirrors_match` (`smaa.rs:73-121`).
- `soft_glow_shader_passes_naga_validation` (`examples/01_platformer/src/tests/main.rs:2323-2327`), once the coverage test scans example assets; that scan also adds `bench_heavy.wgsl`.

Keep `valid_wgsl_source_passes_validation` (`shader_hot_reload.rs:22`): it tests the validator with a fixture, not a shipped shader.

Done when: `cargo test -p tungsten-render --test shader_coverage -- --nocapture` reports the larger inventory; `just check` passes.

### Step 6 — Unused dependencies (cleanup: N5)

Files: `crates/tungsten-core/Cargo.toml` (drop `image`), `crates/tungsten/Cargo.toml` (drop `wgpu`), `examples/02_bench/Cargo.toml` (drop `tungsten-core`, `tungsten-render`; `tests/visual_regression.rs:13` imports `tungsten::render::compare_png` as `post_regression.rs:10` does; fix `tests/fixtures/README.md:57`), `examples/04_shader_playground/Cargo.toml` (drop `tungsten-core`; `src/main.rs` uses `tungsten::core`), `Cargo.lock` (`cargo update --workspace --offline`).

Done when: `just check`, `just deps`, `just bench-build`, layer 1 and `WGPU_BACKEND=vulkan just smoke` pass; `just visual` passes. Symbol hashes change with the dependency set, so step 29 judges placement; no row capture here.

### Step 7 — Drop the dead `sprite.wgsl` mirror (cleanup: N6)

Files: delete `crates/tungsten-render/src/sprite.wgsl`; `crates/tungsten-render/tests/shader_coverage.rs` (`MIRRORS` keeps `shaders/stock` only; fix the fixture lines `:163-164`); `crates/tungsten-render/AGENTS.md:8` (say `assets/shaders/sprite.wgsl` is the compiled-in source, as `D-057` states). No skill names the file.

Done when: `just check`, `just ctx` pass; the render `AGENTS.md` does not grow.

### Step 8 — Dead render API and fields (cleanup: N7)

Files: `crates/tungsten-render/src/{post/mod.rs,post/bloom.rs,material.rs,lib.rs,screenshot.rs,surface.rs,renderer.rs,sprite.rs,targets.rs,text.rs}`, `crates/tungsten-render/src/tests/post.rs` (drop the `final_target` assertions at `:13,20,33,47,73` with the function; `plan_targets` stays covered there). Remove what N7 lists; update the comments at `post/mod.rs:275,398` that name the removed functions.

Removed test content: the `final_target` assertions test a function nothing calls; the target ladder the renderer uses stays covered by the `plan_targets` and `passes_order` tests.

Done when: `just check`, `just visual`, `WGPU_BACKEND=vulkan just smoke` pass; `WGPU_BACKEND=vulkan just perf suite --only gpu,gpu-throughput,integrated --repeat 5 --compare qa-0.40-pre` reads "not `regressed`" (the sprite draw loop's structs change size).

### Step 9 — Dead core API (cleanup: N8)

Files: `crates/tungsten-core/src/ecs/archetype.rs`, `crates/tungsten-core/src/tests/ecs/archetype.rs` (use `entities.len()` where `row_count` was), `crates/tungsten-core/src/physics/components.rs`, `crates/tungsten-core/src/components.rs`, `crates/tungsten-core/src/tests/components.rs:84-86`, `crates/tungsten-render/src/lighting.rs:83`, `DESIGN.md:323` (drop `falloff` from the enum it quotes).

Done when: `just check`, `just physics-release -- --nocapture` (hashes equal step 0) pass; every CPU row reads "not `regressed`": `WGPU_BACKEND=vulkan just perf suite --only physics,physics-sparse,ecs,churn,particles,integrated --repeat 5 --compare qa-0.40-pre`. `Archetype` and the `AnyColumn` vtable change layout, which can move unrelated rows; apply the placement procedure before calling a verdict real.

### Step 10 — Stale comments, archived-plan citations, misleading names (cleanup: N9)

Files: every location N9 lists. Replace plan citations with the decision IDs that hold the rationale (`D-063`–`D-067` for physics, `D-058` for M26, `D-061` for M29, `D-078` for the benchmark fixtures); rename `half_w`/`half_h` to `view_w`/`view_h`; make `entity.rs:76` say the callers guard and the check is debug-only.

Done when: `rg -n 'docs/plans/(physics-scale|archive/benchmark-suite)|M26 plan|see plan|Step-9|substep picker|17 stock effects|asset_loader::material' crates examples` finds nothing; `just check` and `just physics-release` (hashes unchanged) pass.

### Step 11 — Repo check for missing plan citations (tooling: T5)

Files: `scripts/check-repo.py`, `scripts/test-check-repo.py`, `docs/agent-setup.md` (one clause in the `just repo-check` row).

Rule: in tracked `*.rs`, `*.py`, `*.sh`, `*.md`, `*.toml` and `*.yml` outside `docs/plans/`, a `docs/plans/<name>.md` whose file does not exist is an error; `docs/plans/archive/…` stays a note and is never opened. `CHANGELOG.md` and `DECISIONS.md` are exempt as history, and so are the fixture paths in `scripts/test-*.py` (`test-check-repo.py:25,90`, `test-release.py:47,171,224`).

Done when: `just repo-check`, `just script-test`, `just ctx`, `just check` pass; the new test covers a missing citation, an archive citation and an exempt file.

### Step 12 — Scripts, CI and justfile follow-ups (cleanup: N10, N11, N12)

Files: `scripts/check-repo.py:31-33,88-89` (drop `DELETION_CANDIDATES` and its branch) and `scripts/test-check-repo.py` if it covers it; `justfile` (one-line `quick` comment, `python3 -B` in `ctx`); `.github/workflows/ci.yml:51` and `release.yml:132` (drop `libudev-dev`); `docs/known-issues.md:45-47` and `docs/agent-setup.md:60` (drop the resolved lines).

Done when: `just --list` shows the whole `quick` comment; `just script-test`, `just repo-check`, `just ctx`, `just check` pass. The workflow edit is checked by the next CI run (`D-070`); note that in the evidence log.

### Step 13 — Example leftovers (cleanup: N13)

Files: `examples/01_platformer/src/state.rs:213-221` (drop `CycleMode::None` and the allow).

Done when: `just check` passes; `cargo run -p example-01-platformer` under `TUNGSTEN_SMOKE_FRAMES=3` exits 0.

### Step 14 — Render module layout (structure: S3, S4)

Files: `crates/tungsten-render/src/post/mod.rs` (`STOCK_SHADERS` holds the names and `include_str!` paths directly), delete the seventeen `post/{chromatic_aberration,color_adjust,crt,dissolve,dither,fade,film_grain,fog,glitch,god_rays,lut,pixel_outline,pixelate,tone_mono,tonemap,vignette,wipe_radial}.rs` stubs that only hold a name and source; move the inline tests of `targets.rs`, `timing.rs` and `post/smaa_luts.rs` to `src/tests/{targets,timing,smaa_luts}.rs` behind `#[path]`; `docs/LLM_INDEX.md` if a route names a removed file.

Done when: `just check` (same test count as before the move, from the `cargo test` summary), `just visual`, `WGPU_BACKEND=vulkan just smoke`, `just ctx` pass; the stock-shader hot-reload test (`post_regression.rs`, `stock_shader_body_edit_changes_the_frame`) passes under `just visual`.

### Step 15 — Asset loader helpers and module split (structure: S2)

Files: `crates/tungsten/src/asset_loader.rs` becomes `crates/tungsten/src/asset_loader/{mod.rs,atlas.rs,reload.rs,…}` with one `decode_sprite_siblings` helper (normal decode, emissive premultiply) and one page-blit helper used by the build, rebuild and reload paths; the public `tungsten::asset_loader::*` paths stay. Update the `docs/LLM_INDEX.md` rows that name `tungsten/asset_loader.rs`.

Done when: `just check` (including `crates/tungsten/tests/{atlas_integration,asset_smoke}.rs`), layer 1, `WGPU_BACKEND=vulkan just smoke`, `just visual`, `just ctx` pass; the load path is not per-frame, so no row capture.

### Step 16 — `tungsten-core` loader errors (structure: S5)

Files: `crates/tungsten-core/src/assets/{animation,tilemap,audio}.rs` return `thiserror` enums (`AnimationError`, `TilemapError`, `AudioDecodeError`) with the same messages; drop `anyhow` from `crates/tungsten-core/Cargo.toml`; callers in `crates/tungsten/src/asset_loader*` and the tests keep compiling through `?` into `anyhow`.

Done when: `just check`, `just deps`, layer 1 pass; `crates/tungsten-core/tests/audio_decode.rs` still pins the truncated-Ogg error text.

### Step 17 — Split `physics/step.rs` (structure: S1)

Files: `crates/tungsten-core/src/physics/step.rs` becomes `physics/step/{mod.rs,sleep.rs,pairs.rs,solver.rs,arrival.rs,sweep.rs,gather.rs}`: a pure move. Visibility widens to `pub(super)` only where the split needs it; `#[inline(always)]` stays on `narrow_phase` and `narrow_phase_at`; the test module keeps its `#[path]` and gains only `use` lines. Point `docs/LLM_INDEX.md:40-41` and any other maintained doc that names `core/physics/step.rs` at the directory.

Done when: `just physics-release -- --nocapture` (both hashes equal step 0), `just check`, `just ctx` pass; `WGPU_BACKEND=vulkan just perf suite --only physics,physics-sparse,integrated --repeat 5 --compare qa-0.40-pre` reads "not `regressed`"; before capturing, diff `nm -C --defined-only` of the release build against step 0's to predict placement (profiling workflow, "Predict code placement"). If `physics_step` moves past τ with no logic change, report it and stop: the owner decides whether the split stays.

### Step 18 — Local gate recipes and allowlist (tooling: T7, T9, T10)

Files: `justfile` (`ci`, `udeps`), `.claude/settings.json` (`Bash(just physics-release)`), `docs/agent-setup.md` (two table rows and the allowlist sentence at `:30`).

Done when: `just ci` passes end to end; `just udeps` (install `cargo install --locked cargo-shear` first) reports nothing after step 6, or each report is fixed or explained in the evidence log; `just ctx`, `just repo-check` pass (the repo check still rejects wildcard allows).

### Step 19 — Lint the workflows (tooling: T8)

Files: `justfile` (`script-test` runs `actionlint`), `.github/workflows/ci.yml` (pinned `actionlint` 1.7.12 download with checksum, as ShellCheck is installed at `:60-67`), `docs/agent-setup.md` (prerequisites), `docs/known-issues.md:43`.

Done when: `just script-test` passes locally with `actionlint` installed; the evidence log records that CI runs it on the next push (`D-070`).

### Step 20 — Agent routes and setup text (docs: S6, S7)

Files: `docs/LLM_INDEX.md` (rows for the smoke runner, the three pixel tests, `scripts/check-agent-context.py` and the Criterion benches, inside 8,192 B), `docs/agent-setup.md:44` (replace the narrative with the one fact an agent needs), `AGENTS.md` only if byte-neutral.

Done when: `just ctx` (budgets) and `just repo-check` pass and `git diff --check` is clean.

### Step 21 — Fix B4: a replacement tween survives the old one's removal (bug fix)

Files: `crates/tungsten-core/src/ecs/command_buffer.rs` (a public way to queue a plain `fn(&mut World, Entity)`, reusing the existing function-pointer command of `D-084`), `crates/tungsten/src/tweens.rs` (queue a removal that deletes the `Tween` only while it is still marked `pending_remove`), `crates/tungsten/src/tests/tweens.rs`, `DECISIONS.md` (new entry amending `D-056`'s removal clause), `docs/DECISION_INDEX.md`.

1. Failing test first: `replacement_tween_queued_before_completion_survives_flush` (the B4 reproduction); it fails on the current tree.
2. Fix; the test passes, and so does `tween_once_completes_and_removes_component`.

Done when: `just check` passes; `WGPU_BACKEND=vulkan just perf suite --only ecs,churn,integrated --repeat 5 --compare qa-0.40-pre` reads "not `regressed`" and the digests match step 0 (no benchmark hits the race). Hash: not affected.

### Step 22 — Fix B7: file config errors name the file (bug fix)

Files: `crates/tungsten-core/src/config.rs` (new `ConfigError::InvalidValue { path, field, value, expected }` for `render.msaa` and `render.bloom_max_mips` read from the file), `crates/tungsten-core/src/tests/config.rs`.

Failing test first: `file_msaa_error_is_not_an_env_override` loads a temp file with `"msaa": 3` and asserts the variant and that the message names the file and not an env override.

Done when: `just check` passes. Hash, digests, images: not affected.

### Step 23 — Fix B6: scene tweens reject lanes above 3 (bug fix)

Files: `crates/tungsten-core/src/assets/scene.rs` (`SceneTween::validate` rejects `UniformVec4Lane { lane > 3 }`), `crates/tungsten-core/src/tests/assets/scene.rs`; keep the runtime clamp in `tweens.rs:170` as a guard for code-built tweens.

Failing test first: `scene_rejects_vec4_lane_out_of_range` (the B6 reproduction expects `SceneError::Validation`).

Done when: `just check` and layer 1 (example 03's `scene.json` still loads) pass. Not affected: hash, digests, images.

### Step 24 — Fix B5: camera smoothing per unit of time (bug fix)

Files: `crates/tungsten/src/camera.rs` (blend factor `1 - (1 - s)^(dt·60)`: `s` keeps its meaning at 60 Hz, and 0 and 1 are unchanged), `crates/tungsten-core/src/camera.rs` (doc on `smoothing_factor`), `crates/tungsten/tests/camera.rs`, `DECISIONS.md` (new entry), `docs/DECISION_INDEX.md`.

Failing test first: `smoothing_converges_the_same_at_60_and_120_hz` (the B5 reproduction, within a small tolerance).

Done when: `just check`, `just visual` pass; `WGPU_BACKEND=vulkan just perf suite --only integrated --repeat 5 --compare qa-0.40-pre` reads "not `regressed`" with digests equal to step 0 (the bench uses `s = 1`).

### Step 25 — Fix B3: in-place sprite shrink keeps UV and size together (bug fix)

Files: `crates/tungsten/src/asset_loader*` (store each atlas page's size in `AtlasRegistry`; the shrink path writes the UV of the new `new_w × new_h` sub-rect with the half-texel inset that `build_atlas_for_filter` uses), its tests.

1. Move the shrink path's entry computation into a pure helper with today's behavior, then add the failing test `in_place_shrink_uv_spans_the_new_size`: the UV span in texels equals `(new_w − 1, new_h − 1)`, as a fresh pack produces.
2. Fix; the test passes. The lit bundle uses the same rectangle.

Done when: `just check`, layer 1 pass; manual check in example 01 with hot reload: copy a 32×32 sprite PNG to the scratchpad, overwrite it with a 16×16 image and confirm it draws 16×16, then restore it with plain `cp`, confirm it draws 32×32 again and that `git status` shows no asset change (record both in the evidence log). Hash, digests, images: not affected.

### Step 26 — Fix B2: a failed pipeline rebuild keeps the live pipeline (bug fix)

Files: `crates/tungsten-render/src/{renderer.rs,sprite.rs,lit_sprite.rs,post/mod.rs,post/bloom.rs,post/smaa.rs,material.rs}`, `examples/04_shader_playground/tests/post_regression.rs`.

1. Failing test first: `incompatible_stock_shader_edit_keeps_the_shipped_pipeline` stages `fade.wgsl` with an `fs_main` that reads a new `@group(2) @binding(0)` uniform (`stage_with_edited_stock_shader`), runs the playground's fade fixture and asserts exit success and pixels equal to the shipped-shader capture. Today the process panics.
2. Build each candidate pipeline inside `device.push_error_scope(wgpu::ErrorFilter::Validation)` and `pollster::block_on(device.pop_error_scope())`; swap it in and commit the module only on success; log the error otherwise. Same for `upload_material` (keep the last good pipeline, or none on first load). `pollster` is already a render dependency.

Done when: `just visual` (with the new test), `just check`, `WGPU_BACKEND=vulkan just smoke` pass. Hash, digests, `gpu-visual.png`: not affected.

### Step 27 — Fix B1: one UBO per material batch (bug fix)

Files: `crates/tungsten-render/src/{sprite.rs,material.rs,renderer.rs}` (per-material UBO slots in the `ParamSlots` pattern of `D-090`: the k-th batch of a material in a frame uses slot k, built on first use, written only when its bytes change), `examples/04_shader_playground/src/main.rs` and `tests/post_regression.rs`, `DECISIONS.md` (new entry amending `D-058`'s single-UBO clause), `docs/DECISION_INDEX.md`.

1. Failing test first: a playground fixture `TUNGSTEN_MATERIAL_PAIR_FIXTURE=pair|same` draws two sprites with the root manifest's `damage_flash` material, `pair` red and blue at amount 1.0 and `same` blue and blue; `material_batches_keep_their_own_uniforms` asserts the two captures differ over the first sprite. Today they match.
2. Fix; the test passes.

Done when: `just visual` (with `gpu-visual.png` unchanged: the visual preset's `bench_heavy` batches share default bytes), `just check`, `WGPU_BACKEND=vulkan just smoke` pass; `WGPU_BACKEND=vulkan just perf suite --only gpu,gpu-throughput,integrated --repeat 5 --compare qa-0.40-pre` reads "not `regressed`" on `render_encode` and `extract`; digests equal step 0 (they hash counters, not pixels). The `integrated` row now draws each walker's own flash; note it in the changelog line.

### Step 28 — Fix B8: an exact boundary edge stays in its cell (bug fix)

Files: `crates/tungsten-core/src/physics/broadphase.rs` (`max_cell` from `ceil(max / cell) − 1`, still clamped to `min_cell`; fix the comment at `:324`), `crates/tungsten-core/src/tests/physics/broadphase.rs`.

Failing test first: `boundary_edge_far_from_origin_does_not_claim_next_cell` (the B8 probe as an assertion, plus the existing near-origin cases).

Done when: `just check` and `just physics-release -- --nocapture` pass; `WGPU_BACKEND=vulkan just perf suite --only physics,physics-sparse,integrated --repeat 5 --compare qa-0.40-pre` reads "not `regressed`". The hash and the physics digests may move (an inflated AABB can land exactly on a boundary): record old and new values here and in the changelog line, update step 2's constant, and save `just perf baseline save <suite> qa-0.40-step28` for step 29's physics rows if they moved.

### Step 29 — Final compare and close-out (tooling, docs)

1. Quiet machine; `WGPU_BACKEND=vulkan just perf suite --repeat 5 --compare qa-0.40-pre` (physics rows against `qa-0.40-step28` if step 28 moved them). Every owned metric reads "not `regressed`", or the placement procedure plus a recorded justification covers it.
2. Digests (now in the compare report): equal to step 0 except those a bug-fix step recorded.
3. `just physics-release -- --nocapture`, `just visual`, `WGPU_BACKEND=vulkan just smoke`, `just ci` (or its six recipes), `git diff --check`.
4. `docs/known-issues.md` holds no finding this plan fixed; the "Follow-ups" section below moves there.
5. Set `status: done`, fill in the evidence log, and leave archiving to the plan rules.

Done when: all of the above pass and are quoted in the evidence log.

## Open questions for approval

Approved 2026-10-03 with the stated defaults: Q1 no marker lines in existing `DECISIONS.md` entries; Q2 yes; Q3 yes, stopping at the perf gate if it reads `regressed`; Q4 yes, as the last bug step; Q5 remove `falloff`; Q6 keep the 2026-09 captures. Amended the same day at the owner's request: T3 and step 1 also recheck the commit and dirty-tree hash after each measured run (step 0's A′).

- **Q1 Decision markers.** The tungsten-decision skill adds an "Amended by" line under an amended entry; your instruction allows new `DECISIONS.md` entries only. Default: new entries plus `docs/DECISION_INDEX.md` rows, no marker lines. Say if marker lines are allowed.
- **Q2 Step 3.** The dev-profile change costs about 1.7 s per `tungsten-core` rebuild and saves about 72 s per test run; debugging core in debug builds sees optimized code. Default: do it.
- **Q3 Step 17.** Splitting `physics/step.rs` can move code placement under `physics_step`. Default: do it and stop at the perf gate if it reads `regressed`.
- **Q4 Step 28.** B8 is low impact and may move the hash and digests. Default: fix it last; drop the step if you prefer stable numbers for 0.40.
- **Q5 Step 9.** `LightKind::Point::falloff` is removed rather than wired into the lit shader (wiring would be a feature). Default: remove.
- **Q6 Prune tier P4.** Default: keep the 2026-09 runner captures.

## Follow-ups (not in this plan)

- The stock shaders exist twice (`crates/tungsten-render/src/shaders/stock/**` and `assets/shaders/stock/**`, `D-059`); including the asset copies as `sprite.wgsl` and `lit_sprite.wgsl` already do would halve every stock-shader edit but needs a decision.
- `Renderer::new` spends about 390 lines seeding shader IDs (`renderer.rs:145-537`); `examples/01_platformer/src/tests/main.rs` is 2,372 lines.
- `logging.level` and `display.scale_mode` are parsed and unused (`DESIGN.md:143`, `core/config.rs:278-294`): wire or remove, owner's call.
- Particle `Burst { once: false }` only suppresses `ParticleSystemDrained` and `Pulse { total_pulses: Some(0) }` fires one pulse (`tungsten/src/particles.rs:313-347`): define the semantics.
- `render.max_frame_latency = 0` in the file passes `Config::load` and fails at renderer start (`render/surface.rs:118`), while `display.max_frame_latency = 0` warns and falls back (`core/display.rs:292-297`).
- Any file named `input.json` under a watched directory reloads as the action map (`tungsten/src/app.rs:443`).
- Perf, out of scope here: `env::var("TUNGSTEN_PERF_LOG")` every frame (`app.rs:1631`), the tween system cloning channel lists every frame (`tweens.rs:43`), tile proxies rebuilt from a full-map scan every frame.
- The background-load scan of step 1 (`D-095`) covers the measured runs of `run`, `suite` and `--sweep` only: capacity probes, `just smoke` timings and `just visual` still rely on the manual `pgrep` checks.

## Evidence log

Filled in by the steps.

| Step | Result |
| --- | --- |
| 0 | Done 2026-10-03 on `afbc330`; every timed command ran with no `nxcodec.bin`, no `cargo`/`rustc` and governor `performance`; evidence and load logs in `perf-runs/20261003-qa-0.40-step0/README.md` · first pair set aside: A `perf-runs/20261003T042200Z-suite`, A′ `perf-runs/20261003T042554Z-suite`, compare 0 regressed, 0 improved, 40 unchanged, 14 noisy, but a doc-review session edited `docs/plans/` during both (04:23–04:30Z) and A′ reads 4 owned metrics `improved` against B · A/A of record: B `perf-runs/20261003T043647Z-suite`, B′ `perf-runs/20261003T044047Z-suite`, compare `perf-runs/20261003T044445Z-compare-suite`: 1 regressed, 0 improved, 42 unchanged, 11 noisy (A vs B: 0, 0, 42, 12) · machine noise, the B/B′ owned metrics that are not `unchanged`: `particles` `animate_sprites` p95 `regressed` 0.412 → 0.446 ms (interval [0.016, 0.052], τ 0.021; per-run p95 prints in 0.01 ms steps and read 0.412–0.446 over the four suites; accepted as noise by the owner), `noisy` `ecs` `buffs`, `stats_decay`, `accelerate`, `follow` p50, `churn` `flush` p50, `churn_spawn` p50, `gpu` `extract` p95, `gpu-throughput` `render_encode` p50/p95, `particles` `unattributed` p50/p95 · baseline `qa-0.40-pre` = B · digests, equal in A, A′, B and B′ and matching across each row's runs: `physics` `c10885dddd6115a4`, `physics-sparse` `9df5ade5305646dc`, `ecs` `c785506ced04de4c`, `churn` `86014148b7a94681`, `gpu` `f45f26ebdf706840`, `gpu-throughput` `121f8ef1503d8697`, `particles` `08a3c7c126a0d43a`, `integrated` `334b519e18b543d7` · `just physics-release -- --nocapture`: 12 passed; determinism hash `0x088ec07a73c1b168` (both runs); pinned hash `0xaee272e01ffc4e4c`; containment 0/3000 escaped; slow frames: pair list 4392 → 4979, height 284.1421 → 282.7384, 0 below the floor · `just visual`: 6 passed · `just check`: exit 0, 89.7 s wall (928 passed, 7 ignored; tests rebuilt nothing, clippy re-checked the workspace in 2.3 s) · `authored_routes`: 81.01 s test, 84.6 s wall · `just repo-check`: pass |
| 1 | Done 2026-10-03 on `d15d74d`; decision `D-095`. The done-when command could not run as written (`run --compare` rejected the suite baseline `qa-0.40-pre`), so with the owner's approval `run --compare` given a suite now takes its row of the same name · `just perf-test`: 35 tests OK (9 new: `BackgroundLoad` ×7 through `capture_to` with an injected process lister, tree reader and stand-in binary; digest-equal/different compare; suite digests) · `just script-test`: exit 0 · `just ctx`: OK, root budgets unchanged (6,138 / 6,127 B), `tungsten-perf` skill 4,904 → 5,257 B · `just repo-check`: 0 errors · `git diff --check`: clean · `just check`: exit 0, 928 passed, 7 ignored · `/proc` scan cost: 4.2 ms for 344 processes, once a second · sitting 07:15Z: no `nxcodec.bin`, no `cargo`/`rustc`, the only other agent process an idle Codex in `/home/joker` (CPU ticks 1149 before and after), `just perf describe` compiled nothing, `git status --short` and `git diff HEAD \| sha256sum` (`235df89f…`) equal before and after · `WGPU_BACKEND=vulkan just perf run physics --repeat 3 --compare qa-0.40-pre`: exit 0, `perf-runs/20261003T071504Z-physics` valid, background load none, every run's tree equal to the provenance (`d15d74d`, dirty `235df89f01df`); compare `perf-runs/20261003T071513Z-compare-physics` prints "First-run digests match: c10885dddd6115a4." (step 0's digest) and reads 0 regressed, 0 improved, 3 unchanged (`physics_step` p50 6.07 → 6.09, p95 6.25 → 6.23, `update` p95 6.30 → 6.29 ms) · the new compare code on step 0's B/B′ reproduces its verdicts (1, 0, 42, 11) with digests matching in 8 of 8 rows · hash and digests: not affected (runner and docs only) |
| 2 | Done 2026-10-03 on `447d6a9`. `physics_step_is_bit_identical_across_runs` keeps the run-to-run comparison and the `println!` and also asserts `const EXPECTED: u64 = 0x088e_c07a_73c1_b168` (step 0's value). Its comment names the source (`just physics-release`, the runner's flags, generic x86-64, tree `afbc330`) and the `CHANGELOG.md` record a moving bug fix owes, but not this plan's path, which N9/T5 would flag once the plan is archived · `just physics-release -- --nocapture`: exit 0, 12 passed (4 + 1 + 7); determinism hash `0x088ec07a73c1b168` (both runs), pinned hash `0xaee272e01ffc4e4c`, containment 0/3000 escaped, slow frames 4392 → 4979 and 284.1421 → 282.7384, 0 below the floor, all equal to step 0 · negative check: with `EXPECTED` set to `…b169` the test failed (exit 101, "the pile's state hash moved: 0x088ec07a73c1b168"); file restored byte-identical (`cmp`) and the run above repeated on it · `just check`: exit 0, 928 passed, 7 ignored · `just repo-check`: 0 errors · `git diff --check`: clean · hash and digests: unchanged (test only) |
| 3 | Done 2026-10-03 on `941b63e`, in this unattended multi-step session (steps 3–29 run in one session, one patch per step, the owner commits the series at the end); decision `D-096`, amending `D-041`'s dev-profile clause (index rows only, no marker line, Q1) · timed runs 08:13–08:16Z after the remote-desktop encoder exited (08:11:54Z): no `nxcodec.bin`, no `cargo`/`rustc`, no other agent session in the tree (an idle Codex in `/home/joker`), `git status --short` and `git diff HEAD \| sha256sum` equal before and after each run · `cargo test -p example-01-platformer -- authored_routes`: 1 passed in 8.90 s (19.3 s wall, 10.4 s of it compiling core and its dependents for `-p`'s feature set); warm rerun 8.91 s, 9.1 s wall (step 0: 81.01 s, 84.6 s wall) · `just check`, warm after `cargo test --workspace --no-run` and a clippy run: exit 0, 11.1 s wall (step 0: 89.7 s), 928 passed, 7 ignored · `git diff --unified=1 Cargo.toml`: the dev comment (`:63-69`) and the new `[profile.dev.package.tungsten-core]` entry only; `[profile.release]` untouched · also run because the pixel and smoke tests build the dev profile: `just visual` 6 passed, `gpu-visual.png` unchanged; `WGPU_BACKEND=vulkan just smoke` examples 4/4, benchmarks 15/15, every matrix passed; `just physics-release -- --nocapture` 12 passed, nothing recompiled, determinism `0x088ec07a73c1b168`, pinned `0xaee272e01ffc4e4c`, containment 0/3000, slow frames as step 0 · `just repo-check`: 0 errors · `git diff --check`: clean · `/` had filled (0 B free, `ld` killed by SIGBUS linking `example-02-bench`); the session deleted the gitignored cargo cache `target/debug/incremental` (34 GB of stale sessions), `perf-runs/` untouched · hash, digests and images: unchanged |
| 4 | Done 2026-10-03 in this unattended multi-step session, on step 3's tree. Deleted `crates/tungsten-core/tests/physics_timing.rs` (`physics_step_timing_probe_1000_balls`, `full_frame_timing_probe_1000_balls`) and `substep_probe.rs` (`probe_dense_pile_10k`, `probe_dense_pile_25k`); nothing else names them outside released `CHANGELOG.md` history. New `crates/tungsten-core/tests/common/mod.rs` holds `spawn_pile` and `spawn_static_box(world, width, top_y, wall_half)` with the four constants they read; `physics_determinism.rs` passes its former inner `WALL_HALF` (1,000) and `physics_containment.rs` its 40, so spawn order and values are unchanged · `just physics-release -- --nocapture`: exit 0, 12 passed (4 + 1 + 7); determinism hash `0x088ec07a73c1b168` (both runs), pinned hash `0xaee272e01ffc4e4c`, containment 0/3000 escaped, slow frames 4392 → 4979 and 284.1421 → 282.7384, 0 below the floor, all equal to step 0 · `just check`: exit 0, 926 passed, 5 ignored (928/7 less the two timing probes and the two ignored probes) · `just repo-check`: 0 errors · `git diff --check`: clean · hash and digests: unchanged (tests only) |
| 5 | Done 2026-10-03 in this unattended multi-step session. `shader_coverage.rs` scans every `examples/<name>/assets/shaders` that exists (`soft_glow.wgsl`, `bench_heavy.wgsl`; `examples/01_platformer/tools/shaders/` is outside the plan's scope and stays unscanned) and gains `checker_validates_example_shaders`, a broken example shader in a fixture tree. Removed: `bloom_{threshold,downsample,upsample,composite}_wgsl_naga_validates`, `bloom_wgsl_engine_and_asset_mirrors_match`, `smaa_{edge,blend_weights,neighborhood_blend}_wgsl_naga_validates`, `smaa_wgsl_engine_and_asset_mirrors_match`, `soft_glow_shader_passes_naga_validation`; `valid_wgsl_source_passes_validation` stays · `cargo test -p tungsten-render --test shader_coverage -- --nocapture`: 7 passed, "shader coverage: 70 paths, 39 distinct contents, 31 mirror pairs" (before: 68 paths and 37 distinct contents, counted over the two old trees with the test's rules; pairs unchanged) · `just check`: exit 0, 917 passed, 5 ignored (926 − 10 removed + 1 new) · `just repo-check`: 0 errors · `git diff --check`: clean · hash, digests and images: unchanged (tests only) |

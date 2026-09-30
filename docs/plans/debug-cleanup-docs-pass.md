# Debug, cleanup and documentation pass

status: draft
goal: fix confirmed bugs with regression tests, make behavior-preserving cleanups across all crates and examples, move open review findings into a live `docs/known-issues.md`, archive finished plans and the 2026-09-25 review, and cut redundant or stale documentation.
non-goals: new features; fixes for design-level findings (the four open P2s, plus the new material-UBO P2 below); public API changes beyond what a bug fix needs; new `D-NNN` entries; edits to existing `DECISIONS.md` entries or released `CHANGELOG.md` sections (one exception, decision D1); Git commits (the owner handles Git).
files to touch: see "Files to touch" below.
ordered steps: 0 baseline (done); 1 frame-cap fix; 2 playground cycle fix; 3 core cleanups; 4 render cleanups; 5 umbrella cleanups; 6 example cleanups; 7 script cleanups; 8 known-issues and archive moves; 9 doc trims; 10 final checks and report.
done-when: `just check`, `just repo-check`, `just ctx`, `just smoke` and `just script-test` pass; no `done`/`abandoned`/`superseded` plan remains in `docs/plans/` outside `archive/`; `rg -n 'repo-review-2026-09-25' --glob '!docs/plans/archive/**'` shows only links to `docs/plans/archive/repo-review-2026-09-25.md`.

## Context digest

- Branch `0.31`, workspace `0.30.0`. The uncommitted platformer work (ball pit, fire intensity, polish steps 6–12) is in scope and in the baseline.
- Baseline, 2026-09-29: `just check` passes (768 passed, 0 failed, 4 ignored), and so do `just repo-check`, `just ctx` and `WGPU_BACKEND=vulkan just smoke` (every row OK), `generate.py --check` (520 outputs) and the 15 platformer Python tests.
- Audit coverage: every module in `tungsten-core/ecs`, the umbrella crate's `app`, `asset_loader`, `state`, `particles`, `tweens`, `audio`, `hot_reload`, `sprite_extract`, `camera`, `game_feel`, overlays and extract helpers, the render crate's `renderer.rs` and `sprite.rs` draw path, `post/mod.rs` and its dependency graph, all four examples' entry points and systems, plus a scan of every file's `unwrap`/`expect`/`allow`/TODO. Physics was audited on 2026-09-25; this pass only rechecked its open findings. This is not exhaustive path coverage.
- All four open P2s and every P3 from the 2026-09-25 review are still present in source. Every carried-forward item still applies. One exception: `player.png` is now registered as `ex10_player`, so it is no longer a deletion candidate.
- Git mutations are human-only. Moves use plain `mv`, and the owner stages the result.

## Decisions for approval

Defaults apply unless you say otherwise.

- **D1: `CHANGELOG.md` line 113.** It is the only reference to the review, and it sits in the released `[0.27.0]` section. Your requirements conflict here: "update every reference" plus the done-when `rg` check, against "leave released sections unchanged". Default: rewrite only that path to `docs/plans/archive/repo-review-2026-09-25.md`, change no other word, and note the edit under `[Unreleased]`. Alternative: leave it and accept one `rg` hit.
- **D2: platformer Rust cleanup.** `platformer-polish-pass.md` steps 14–18 already cover splitting `extract`/`systems`/`gameplay`/`setup` and its unwrap and dead-code pass (`Vec2X3`, `CycleMode::None`, the stale `OrbitLight` doc, the post-query `get_mut().unwrap()`s). Those steps still await your approval, and these modules are still changing. Default: leave `examples/01_platformer/src/**` to that plan, except `burning.rs`, which the polish plan excludes. No confirmed bug was found there.
- **D3: `examples/01_platformer/tools/README.md`.** Polish step 19 owns its restructure. Default: correct only claims that no longer match code, with no reorganization.
- **D4: `docs/plans/phase4.md`.** M25–M30 are shipped. Their sections (about 230 lines) repeat the archived milestone plans and `D-057`–`D-073`. Default: collapse them into one table (milestone, release, archived plan, decisions). Keep M31–M33, the seam constraints, and the resolved decisions and sources that still govern M31–M33. Header and done-when stay unchanged.
- **D5: frame-cap regression row.** Default: add a `just smoke` row. The capped run of `example-03-scene-state` must take at least 90% of `frames / cap` (20 frames at 20 fps adds about 1 s). This is the only GPU-level before/after proof for bug B1.

## Findings

### Bugs to fix (confirmed)

| ID | Location | Defect and evidence | Regression test (written first, seen failing) |
| --- | --- | --- | --- |
| B1 | `crates/tungsten/src/app.rs` (`RedrawRequested`, `stage_pacing`) | `display.frame_rate_cap` never limits the frame rate. Every frame calls `window.request_redraw()` before setting `ControlFlow::WaitUntil`, and the pending redraw wakes the loop immediately. Measured: 120 smoke frames of example 03 take about 0.30 s whether the cap is 0, 30 or 10 (a cap of 10 needs at least 12 s). | CPU: extract the redraw schedule into a pure helper. A unit test asserts that a capped frame defers its redraw to `frame_start + budget` and an uncapped frame redraws immediately; it fails against the helper extracted unchanged, then the fix follows (`about_to_wait` requests the deferred redraw). GPU: the D5 smoke row, run against the pre-fix binary to record the failure. |
| B2 | `examples/04_shader_playground/src/main.rs` `cycle_input_system` | Starting from the empty stack (`CycleCursor::index == None`), `post_next` computes `(0 + 1) % len`. The first press (and the first press after a clear) skips roster entry 0 (Tonemap) and shows "vignette (2/17)". | New `examples/04_shader_playground/src/tests/main.rs`: a world with `post_next` bound and `KeyN` pressed runs the real system and asserts the stack holds Tonemap at index 0. It fails before the fix; `prev` from empty keeps selecting the last entry. |

### Cleanups (behavior-preserving)

- **`tungsten-core`:**
  - Remove the unused `AnyColumn::len` and `AnyColumn::type_id`. The latter also shadows `Any::type_id` on `dyn AnyColumn`.
  - Remove `Archetype::id`, which is never read, and `row_count`, which only tests use (they switch to `entities.len()`). That drops three `#[allow(dead_code)]`.
  - Repoint five stale `docs/plans/physics-scale-and-ccd.md` comment paths to the archive: `physics/step.rs` and `tests/{substep_probe,physics_determinism,physics_containment,physics_tunneling}.rs`.
- **`tungsten-render`:**
  - `upload_shader` and `reload_shader` duplicate about 50 lines of pipeline-rebuild dispatch plus the engine-stage name list. Extract one `rebuild_for_shader(name, id, &module)` and one stage-name predicate.
  - Seed `Renderer::new`'s eleven compile-time shaders from one table.
  - `sprite.rs` draw: carry the pipeline in `PipelineKey`, which removes three `unwrap`s.
  - Keep-alive texture fields with `allow(dead_code)` (`GpuTexture`, `BloomPyramid`, `SmaaPipeline`) stay: removing them is a GPU-lifetime change with no pixel test.
- **`tungsten` (umbrella):**
  - Remove the unused direct `wgpu` dependency. Only a test comment mentions it; `Cargo.lock` loses one edge.
  - `asset_loader.rs` repeats several blocks. Share one helper each for:
    - sibling decode (3×)
    - emissive premultiply (3×, each with its own `allow`)
    - row blits (6×)
    - flat-normal canvas (2×)
    - packed UV (2×)
  - HUD composition re-implements `anchor_text_block`, and the four-offset outline block is copied in the HUD, systems overlay and inspector; share one helper.
  - Correct stale comments:
    - `reload_manifest` says it refreshes the "shader pointer", but only `uniform_defaults` reload.
    - `sprite_extract` claims GPU depth reproduces CPU order, which is false for interleaved batch keys within one z-run.
- **Examples:** in `burning.rs`, the two `unwrap`s become `query_mut` / `let … else` with the same iteration order. D2 defers the rest of the platformer; examples 02 and 03 need nothing.
- **Scripts:**
  - `scripts/check-repo.py` still lists `player.png` as an unregistered deletion candidate; it is registered as `ex10_player` at HEAD. Empty that entry.
  - Its test (`test-check-repo.py`) takes that entry as its fixture; give the test its own synthetic candidate.
  - Add `docs/known-issues.md` to the checker's maintained `DOCS`.

### Findings reported, not fixed (they go to `docs/known-issues.md`)

New this session:

| Priority / location | Finding | Why not fixed |
| --- | --- | --- |
| P2 — `render/sprite.rs::draw` | Batches of one material with different `UniformOverrideBlock`s all draw with the last batch's uniforms: each batch `write_buffer`s the material's single UBO before the one submit. `render-features` hits this (two `damage_flash` variants). Same root cause as the open post-stack UBO P2. | Needs per-batch uniform ownership (dynamic offsets or a ring), the same design as the post-stack P2. |
| P3 — `core/config.rs`, `core/display.rs` | `logging.level` and `display.scale_mode` are parsed, documented in `DESIGN.md` and mirrored to telemetry, but never applied. The examples call `env_logger::init()` before loading config. | Applying them is a feature; removing them changes the schema. |
| P3 — `tungsten/asset_loader.rs::reload_manifest` | Shaders added to the manifest are not registered on reload, although `DESIGN.md` says they are. A new material that references a new shader fails until restart. Changing an existing material's `shader` also needs a restart. | New reload behavior; ties to the open composition/reload P2. The docs get corrected. |
| P3 — `tungsten/sprite_extract.rs` | With `DepthSortMode::GpuDepth`, draw order within one z-run follows entity id. CPU order follows the batch key, so overlapping same-z sprites with interleaved keys can differ between modes. | Ordering-contract choice. The comment gets corrected. |
| P3 — `tungsten/debug_hud.rs` | The HUD `fps` row is `1000 / CPU frame time`. Under a working frame cap (after B1) it overstates FPS, because idle wait is excluded. | Changes what the HUD measures. |
| P3 — `tungsten/tilemap_extract.rs` | `extract_tilemaps` never sets `SpriteBatch.lit`, so normal-mapped tiles render unlit through the engine extract. The platformer uses its own extract. | Lighting-feature scope. |
| P3 — `core/input.rs` `KeyCode` | The key set is curated. `Digit4`, `Digit9` and most letters arrive as `Other(n)` and cannot be named in `input.json`. | Adding variants changes a public enum. |
| P3 — `core/assets/particle.rs` | `EmissionKind::Burst { once: false }` emits once, like `once: true`, but never drains. The semantics are undocumented and nothing uses it. | Contract decision. |
| P3 — `docs/showcase/` | The M27 SMAA and M28 bloom captures documented in its README were never committed. The M29 lighting capture predates the 0.29 art. | Regenerating acceptance art is out of scope. |

Carried over unchanged, and all still present: the four P2s (cross-root manifest composition and reload, post-stack UBO sharing, stock post pipelines not rebuilt on shader reload, and dynamic-gate tunneling), the P3s (shader-ID allocator mismatch, skipped-frame capture accounting, state transition cleanup, audio format/capacity, physics edge cases), and the carried-forward platform checks and follow-ups.

### Deletion candidates (reported, not deleted)

- `crates/tungsten-render/src/sprite.wgsl`: never compiled. `sprite.rs` and `renderer.rs` `include_str!` `assets/shaders/sprite.wgsl`, and `D-057` says the two share one file. Only the mirror test keeps it. Deleting it also means removing the `MIRRORS` entry and the render `AGENTS.md` mirror rule.
- `crates/tungsten-core/tests/substep_probe.rs`: an ignored diagnostic probe written for the archived physics plan. Keep it if physics bench attribution still needs it.
- `examples/01_platformer/assets/sprites/player.png`: no longer a candidate (registered as `ex10_player`).

## Files to touch

- Bugs: `crates/tungsten/src/app.rs`, `crates/tungsten/src/tests/app.rs`, `scripts/smoke-examples.sh` (plus `scripts/test-smoke-examples.sh` if it asserts the row list), `examples/04_shader_playground/src/main.rs`, new `examples/04_shader_playground/src/tests/main.rs`.
- Cleanups: `crates/tungsten-core/src/ecs/{archetype,storage}.rs`, `crates/tungsten-core/src/tests/ecs/archetype.rs`, the five physics comment files, `crates/tungsten-render/src/{renderer,sprite}.rs`, `crates/tungsten/Cargo.toml`, `Cargo.lock`, `crates/tungsten/src/{asset_loader,debug_hud,systems_overlay,inspector,sprite_extract}.rs`, `examples/01_platformer/src/burning.rs`, `scripts/check-repo.py`, `scripts/test-check-repo.py`.
- Docs:
  - Known issues and archive: new `docs/known-issues.md`; move `docs/repo-review-2026-09-25.md` into `docs/plans/archive/`.
  - Indexes, changelog and design: `docs/LLM_INDEX.md`, `CHANGELOG.md`, `DESIGN.md`, `README.md`.
  - Other docs: `docs/agent-setup.md`, `docs/perf/profiling-workflow.md`, `docs/showcase/README.md`, `docs/plans/phase4.md`, `assets/fonts/README.md`, `crates/tungsten-core/tests/fixtures/audio/README.md`, `examples/01_platformer/tools/README.md`.
  - Skills: `.claude/skills/tungsten-perf/SKILL.md`, `.claude/skills/tungsten-wgpu/SKILL.md`.

## Steps

### 0. Baseline (done during planning)
Results are in the digest. Logs are under the session scratchpad (`baseline-{check,repo,ctx,smoke}.log`).

### 1. B1 frame cap
Extract the pacing decision unchanged and add the unit test; record it failing. Then apply the fix:
- capped frames stop calling `request_redraw` at frame end;
- `about_to_wait` requests the redraw once the deadline passes, and otherwise keeps `WaitUntil`;
- uncapped behavior is unchanged.

Add the D5 smoke row and run it once against the pre-fix build to record the failure. Checks: `cargo test -p tungsten`, the smoke row, and a manual timing rerun at caps 0, 30 and 10.

### 2. B2 playground cycle
Add the test and record it failing, then fix the stepping: next from `None` goes to 0 and prev from `None` goes to `len - 1`. Check: `cargo test -p example-04-shader-playground`.

### 3–6. Cleanups by crate
One crate per step, in the order core, render, umbrella, examples. After each, run `cargo test -p <crate>` and strict clippy for that crate. Render cleanups also need `just smoke`. No test assertions change except the archetype test's `row_count` → `entities.len()`.

### 7. Scripts
Make the `check-repo.py` and `test-check-repo.py` changes, then run `just script-test` and `just repo-check`.

### 8. Known issues and archive
1. Write `docs/known-issues.md` with these sections:
   - open findings (P2/P3), each with a status date;
   - carried-forward platform checks and follow-ups;
   - closed this pass, one line each pointing to `CHANGELOG.md`.
2. Done at the 0.31.0 release: `platformer-fire-intensity.md` is `status: done` and archived, and `platformer-polish-pass.md` points at the archived copy.
3. Move the review to the archive. It has no Markdown links to rebase.
4. Update `LLM_INDEX.md`: replace the review row with `docs/known-issues.md`, and keep the file under 8 KiB.
5. Apply D1 to `CHANGELOG.md`.
6. Record the session under `[Unreleased]`: Fixed B1 and B2; Changed for the cleanups, docs and the known-issues move.

`phase4.md` stays active: M31–M33 are open. `platformer-polish-pass.md` stays active: its done-when needs the step-0 snapshot, and steps 13–20 still await your approval.

### 9. Doc trims (one canonical home per fact)

- **`DESIGN.md`:**
  - Cut the status paragraph to the version line, a one-line summary and pointers. Move the M26 and M30 facts that exist only there into short subsystem sections.
  - Correct stale claims:
    - frame-loop order: telemetry follows render and audio;
    - ECS: mutable multi-queries exist, command buffers shipped, `TypeIdMap`;
    - `notify` is v8, not v6;
    - audio formats: AAC is enabled; PCM WAV fails (known issue);
    - hot-reload matrix: no manifest-add for shaders or the lit shader;
    - physics staging: `D-075` replaces the half-cell restage;
    - M29: `lit_sprite` has no `src/` mirror;
    - config sample: add `post_aa` and `bloom_max_mips`, drop the "in M17" wording.
  - Replace the perf-baseline section with a pointer to `profiling-workflow.md`.
- **`profiling-workflow.md`:** drop "M12" from the title, merge the two scene-only `frame_gpu_ms` statements, and fix the M17 wording.
- **Skills:**
  - `tungsten-perf`: cut the sections that repeat the workflow doc (telemetry format, hotspots, interpretation, budgets, regression policy) down to links. Keep the rules that are easy to miss.
  - `tungsten-wgpu`: cut the shader-mirror and present-mode passages that repeat render `AGENTS.md` and the workflow doc.
- **`agent-setup.md`:** remove the stale `player.png` sentence. Move the dated client-discovery evidence into a known-issues follow-up; keep a one-line pointer.
- **`showcase/README.md`:**
  - replace the three copied recipes with one recipe and a per-artifact environment table;
  - mark the M27 and M28 captures as never committed.
- **`assets/fonts/README.md`:**
  - keep the inventory, licensing and the IDs the manifest registers (`sans`, `sans_bold`, `mono`);
  - drop the generic crate advice and the role aliases that don't match the IDs.
- **Other docs:**
  - Audio fixture README: note that the WAV fixture pins the PCM-unsupported error.
  - `README.md`: add a `docs/known-issues.md` row.
  - `phase4.md`: apply D4.
  - tools README: apply D3.

Run `just ctx` and `just repo-check` after this step.

### 10. Final checks and report
1. Run every done-when command. Report failures with their output.
2. Set this plan to `status: done` and move it to the archive.
3. Write the final report with these sections:
   - bugs fixed, each with file and test;
   - cleanups by crate;
   - findings left unfixed, with reasons;
   - docs archived, moved and trimmed;
   - deletion candidates.

# Repository review and open follow-ups — 2026-09-25

Reviewed on branch `0.27` at workspace `0.26.0`; the fixes shipped in `0.27.0`. The review itself changed no dependency versions. This is the live follow-up record for the review; completed execution plans and historical audits are archived.

## Coverage and fixes

Reviewed the source/API surface and risk paths across `tungsten-core`, `tungsten-render`, `tungsten`, and all four examples: ECS borrowing/structural changes, assets and reloads, physics, input/state/audio lifecycles, rendering resources and passes, example wiring, and scripts/configuration. Workspace checks compile every target; GPU smoke covers every example and the existing fixture matrices. This is not exhaustive path coverage or a guarantee that no other defects remain.

| File(s) | Fix |
| --- | --- |
| `crates/tungsten-core/src/assets/animation.rs` | Clamp playback into a shortened hot-reloaded clip before indexing its frames. |
| `crates/tungsten-core/src/assets/tilemap.rs` | Resolve sparse Tiled IDs through an explicit lookup; reject duplicate/unknown/underflowing GIDs; avoid integer overflow in pixel dimensions. |
| `crates/tungsten-core/src/input/action_map.rs` | Replace the global mutable nonce with exclusive temporary-file creation; retain existing temporary files and clean up failed saves. |
| `crates/tungsten-core/src/tests/{assets/animation,assets/tilemap,input/action_map}.rs` | Regression coverage for shortened clips, sparse/invalid GIDs and temporary-file collision/failure handling. |
| `crates/tungsten/src/audio.rs` | Stop mixing finished voices immediately; retire empty loops and unknown handles; correct the allocation-free callback claim. |
| `crates/tungsten/src/app.rs` | Drain audio commands without an output device; return window/renderer/manifest initialization failures from `App::run` so launch failures do not report success. |
| `crates/tungsten/src/tests/{audio,app}.rs` | Regressions for stopped/empty voices and the no-device command queue. |
| `crates/tungsten/src/asset_loader.rs` | Abort in-place reload or atlas repack before writes when a declared lit sibling fails decode/dimension validation, preserving the previous bundle. |
| `crates/tungsten-render/src/renderer.rs` | Honor `WGPU_BACKEND`; propagate surface-creation errors; remove three private shader-ID fields never read after construction. |
| `examples/03_scene_state/src/states.rs` | Backspace from pause removes both the overlay and underlying gameplay state before entering the menu; regression asserts stack depth and entity cleanup. |
| `crates/tungsten/Cargo.toml` | Remove unused direct `thiserror` and `pollster` dependencies. |
| `examples/{01_platformer,03_scene_state}/Cargo.toml` | Remove unused direct `tungsten-core`, `tungsten-render`, and `log` dependencies; these examples use umbrella reexports. |
| `examples/04_shader_playground/Cargo.toml`, `Cargo.lock` | Remove unused direct `tungsten-render` and `log`; lockfile changes only remove the ten local dependency edges. |

No external ECS/runtime or GPU/window types leaking into core were found. Remaining statics used as test filename counters are test-only. No stale TODO/FIXME/todo!/unimplemented! markers were found in runtime/example sources. Decisions were checked before classifying behavior: atlas shrink/transparent-tail behavior is explicitly accepted by D-048, session-static audio by D-053, and path-based scene loading by D-046; these were not treated as new defects.

## Findings left open

Priorities: P2 = functional follow-up; P3 = limitation, rare edge case or contract clarification. Unless noted, these are source-path findings, not newly reproduced GPU/adversarial tests. In this table, `core/`, `render/` and `tungsten/` abbreviate the respective crate `src/` directories.

| Priority / location | Trigger and impact | Why deferred / next work |
| --- | --- | --- |
| P2 — `core/assets/manifest.rs::load/load_and_merge_many`, `tungsten/asset_loader.rs::reload_manifest`, `tungsten/app.rs::stage_hot_reload` | A material in one root cannot reference a shader in another: each root validates before merging. Reload also replaces the merged `LoadedManifest` with a single root, compares that root against global registries, and watches only one manifest path. | Fix the composition/reload graph together (D-017/D-052/D-053), with merge-first cross-reference validation and all-root watching. Changing only initial loading leaves reload inconsistent. |
| P2 — `render/post/mod.rs::record_pass` | Two instances of the same stock effect with different parameters share one UBO. Both queue writes precede submission, so both draws consume the last parameters. | Requires per-slot uniform ownership/caching and a repeated-effect GPU regression; avoid a quick per-frame allocation fix that adds a new hot-path cost. D-058 does not prohibit repeated passes. |
| P2 — `render/renderer.rs::upload_shader/reload_shader`, `render/post/mod.rs` | The 17 ordinary stock post pipelines use embedded WGSL and are not rebuilt when their manifest shader cache entries change. Cache success need not mean a visible edit. | Define stock-pipeline cache ownership and atomic replacement, then add a body-edit capture test. Existing sprite/material/SMAA/bloom/lit routes must remain intact (D-057/D-058). |
| P2 — `core/physics/step.rs` | A fast heavy body can accelerate a resting body through a dynamic gate in the same substep: admission used pre-solve velocities, and the safety sweep excludes dynamics. The completed physics audit recorded a failing probe at 15,360 px/s. Current code retains that structure. | CCD/solver design change (D-062–D-067), requiring admission repair, TOI or a justified velocity bound. Historical probe results were read before archiving; that throwaway harness was not available to rerun. |
| P3 — `core/assets/shader.rs`, `tungsten/asset_loader.rs::load_shaders` | Core allocates shader IDs in manifest iteration order; render independently seeds/allocates them. Numeric IDs need not match despite the core module's same-ID comment. Current bridge uses names. | Establish one allocator or distinct ID types before exposing cross-crate numeric lookup; do not silently change public handle semantics (D-016/D-057). |
| P3 — `tungsten/app.rs::stage_render`, `render/renderer.rs` | Recoverably skipped acquisition can return `Ok`, increment frame/capture accounting and claim capture success; readback errors only warn. Runtime render errors also remain logged rather than returned through `App::run`. | Needs a presented/skipped/failed result contract and explicit capture completion, while preserving D-029 surface recovery. Initialization failure propagation is fixed separately. |
| P3 — `tungsten/state.rs` | Multiple queued transitions can exit a state before its pending command-buffer spawns become live; automatic cleanup only sees live entities. Multiple stack instances sharing a `StateId` also share cleanup ownership. | Choose transition coalescing/flush semantics or per-instance ownership; preserve D-039/D-046 frame order. The demonstrated pause/menu leak is fixed without changing that API. |
| P3 — `tungsten/audio.rs` | Output assumes f32 and PCM conversion supports mono/stereo; a device with more channels is not fully mapped. `Play` can allocate in the callback, and full command rings drop commands. | Device-format/channel support and voice capacity/backpressure need an explicit audio policy (D-034). No audio-feature expansion here; hardware routing/listening is an owner check. |
| P3 — `core/physics/{step,broadphase}.rs` | Previous audit's remaining edges: sleep-tag adoption sees only touching final-substep contacts; extreme/non-finite externally written coordinates can overflow or cause enormous grid walks; entity generation is truncated to 31 bits; overlapping tile centers share warm-start identity. | Sleeping case remains unforced; input-domain and identity changes need explicit bounds/semantics. Generation/hash cases are theoretical or degenerate content. Preserve these findings without presenting them as freshly reproduced failures. |

## Carried forward

From the archived agentic-restructure plan and the release-pipeline QA (`D-071`), as of `0.27.0`.

Platform checks with no host available:

- Metal (macOS) and DX12 (Windows) rendering on wgpu 30; Vulkan success isn't certification.
- macOS CoreAudio and Windows WASAPI audio on cpal 0.18.
- `.agents/skills` symlinks on a Windows clone (Developer Mode and `core.symlinks=true`, documented in `docs/agent-setup.md`).
- The first tag-triggered release: the Windows MSVC link and launching the published archives on real hardware. Linux archives were built, packaged and smoke-run locally; the Windows build was only type-checked.

Follow-ups:

- Enable Symphonia's `pcm` feature so PCM WAV decodes; `tests/audio_decode.rs` pins today's "unsupported codec" error.
- Truncated Ogg files fail at probe; decide whether to decode the available prefix, as MP3 does.
- Evaluate rtrb 0.4.0 against 0.3.5, and adopt winit 0.31 once it leaves prerelease.
- Drop the RUSTSEC-2026-0192 exception when cosmic-text/fontdb stop using `ttf-parser`.
- Re-verify instruction loading once Claude Code reads `AGENTS.md` natively (2.1.277+); the `CLAUDE.md` import could then load it twice.
- `perf-capture.sh` creates its output directory before validating the scene name.
- Only `example-01-platformer` enables hot reload; the shader playground could too.
- Add `actionlint` to `just script-test` now that `release.yml` exists (it passed `actionlint` 1.7.12 and `zizmor` 1.30.1 when run by hand).
- Release archives don't bundle third-party license notices for statically linked crates.
- Both workflows install `libudev-dev`, but no locked crate links udev.
- `just --list` shows only the last line of the `quick` recipe's two-line comment.

## Documentation, archive and cleanup

- `AGENTS.md`: corrected handle allocation ownership (D-048), asset coverage enforcement, the D-046 scene exception, the complete `just check` definition, and the audit rule's explicit user-requested-fixes exception. Context budgets still pass.
- `README.md`: current branch, accurate baseline-scene label, tool prerequisites and quick/script checks.
- `CHANGELOG.md`: Unreleased fixes/tooling, moved audit path and historical-path guidance; no release/version bump. Historical example/tilemap filenames remain dated release evidence, not current runnable paths.
- `docs/LLM_INDEX.md`: all four examples plus repository tooling navigation.
- `docs/DECISION_INDEX.md`: targeted decision-section reading, not whole-file reading; all 70 IDs match the Rust index test.
- `docs/agent-setup.md`: exact permissions, new check tiers/exceptions/limitations, current Codex discovery evidence, and honest Claude availability.
- `docs/perf/profiling-workflow.md`: correct archive link, flags/provenance distinction, scene-pass GPU timing scope, and direct-binary manual profiling with output under `perf-runs/`.
- `docs/showcase/README.md`: composition commands now resolve from the repo root.
- `docs/plans/README.md`: completed/abandoned/superseded lifecycle and lint behavior. `phase4.md` is in progress, with M25–M29 identified as historical shipped scope and M30–M33 still active.
- Local links and D-NNN references in the eight requested docs were checked. The four distinct external Markdown URLs returned HTTP 200 (Codex redirects to current official guidance). Archive links were deliberately not opened or checked. Historical command examples are not claims that those captures were rerun.

Archived, retaining basenames:

- `docs/plans/perf-overhead-audit.md` → `docs/plans/archive/perf-overhead-audit.md`: already done; unresolved performance backlog does not make the completed audit active.
- `docs/plans/physics-debug-audit.md` → `docs/plans/archive/physics-debug-audit.md`: completed findings/probe report mislabeled draft; marked done with lifecycle headers first. Its open correctness findings are retained above.
- `docs/plans/repo-review-tooling.md` → `docs/plans/archive/repo-review-tooling.md`: this review's completed execution plan.

`agentic-restructure.md` was archived at the `0.27.0` release after CI passed; its outstanding platform checks and follow-ups are under "Carried forward" below. `phase4.md` stays active for M30–M33. Historical inventory prose in the setup plan is retained as dated evidence, not rewritten as a current inventory. No archive contents were read, searched or globbed.

Removed only the ignored, untracked editor backup `assets/sprites/walk_3.png~`. `input.json` and `tungsten.json` are tracked runtime inputs and remain. The existing `perf-runs/` entries were listed; none were deleted. Build caches remain. No tracked file was deleted outright; the two tracked audit removals are moves.

Deletion candidate awaiting owner decision: `examples/01_platformer/assets/sprites/player.png`, a tracked, unregistered legacy image with no source reference found. The checker reports it explicitly and retains it. Vendored shader helpers/licenses, font families and their inventory README, and example 03's `scene.json` are legitimate exceptions, not deletion candidates.

## Tooling and agent flow

| Addition/change | Repeated work replaced |
| --- | --- |
| `scripts/check-repo.py` / `just repo-check` | Manual asset-file coverage comparisons, duplicate JSON-key checks, active-plan headers/lifecycle, maintained-doc local links/decision IDs, and project agent-filter/permission checks; reuses Rust manifest/index validators and the existing context checker. |
| `scripts/test-check-repo.py` | 14 CPU-only synthetic regressions: malformed/duplicate JSON, missing files/manifests, exceptions, symlinks, retained candidates, plan status/headers, archive avoidance, broken links/IDs and permission drift. Runs in `just script-test`. |
| `just quick` | Format/context/repository/type-check sequence during editing; omits full clippy/workspace tests. Warm-run speed depends on Cargo cache; final `just check` remains required. |
| `.github/workflows/ci.yml` | Runs the same `just repo-check` locally and in CPU-only informational CI (D-070). |
| `.claude/settings.json` | Exact shared recipe/build/example/discovery commands instead of repeatedly granting them. No wildcard execution prefixes; parameterized commands require ordinary client/personal approval. |

Evaluated but not added: scaffolding (only four examples with different setup needs; smoke already discovers packages through Cargo metadata), asset fix-up (cannot infer IDs/intent safely), another decision-index checker (Rust already enforces it), and a generic archive mover (completion judgment and link semantics remain manual; lint catches misplaced completed plans). No new runtime dependencies, hooks or mandatory pre-commit process.

Both `CLAUDE.md` files contain only `@AGENTS.md`; both skill symlinks resolve; `.ignore` excludes the archive. `just ctx` and its nine synthetic cases pass. Codex CLI 0.155.0-alpha.16.3 discovery runs succeeded from root and renderer with read-only ephemeral sessions; root and nested rule visibility matched expectations. Self-reports cannot prove absence of duplicate context injection. Claude was not installed in this shell: its static configuration is checked, but effective permissions and both `claude -p` discovery launches need the owner to rerun them.

## Verification

Final gate results below include the last source edits. Logs are under `/tmp/tungsten-review-final-{check,scripts,quick,smoke}.log` (session-local, not committed).

- `just check`: passed formatting and strict all-target clippy; 655 tests passed, zero failed, four ignored.
- `just script-test`: passed, including ShellCheck, perf/smoke regressions and 14 repository-checker tests.
- `just ctx`: passed budgets, paths, imports, symlinks and nine self-tests.
- `python3 -B scripts/check-repo.py`, `just repo-check`, `just quick`: run successfully; zero repository QA errors. `quick` invokes both new recipes' checks.
- `WGPU_BACKEND=vulkan just smoke`: passed 4/4 examples and 9/9 fixture rows after the final source edits.
- `git diff --check`: passed.

Unavailable/not claimed: Claude execution/effective permissions; remote CI/push (out of scope); macOS/Windows, Metal/DX12 and Windows symlinks; listening/device-format audio matrix; reference-machine pixel comparison; fresh canonical performance captures; exhaustive hot-reload corruption/capture validation. In particular, lit-sibling rejection was checked by source ordering plus normal GPU smoke, not a dedicated corrupted-file GPU test. Existing CPU tests validate WGSL and mirror coverage; smoke does not prove every visual effect or deferred scenario correct.

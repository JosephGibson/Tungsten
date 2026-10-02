# Known issues

Open findings and follow-ups that no active plan owns. A fix removes its entry here in the same change. Sources: the repository review of 2026-09-25, whose archived copy keeps its fixed and historical parts, and the P2 correctness pass of 2026-10-02 (`D-088`–`D-091`).

Priorities: P2 = functional follow-up; P3 = limitation, rare edge case or contract clarification. Unless noted, these are source-path findings, not reproduced GPU or adversarial tests. `core/`, `render/` and `tungsten/` abbreviate the respective crate `src/` directories.

## Open findings

| Priority / location | Trigger and impact | Why deferred / next work |
| --- | --- | --- |
| P2 — `core/physics/step.rs` | A body the solver accelerates can cross a neighbour or creep through it. The narrow phase admits contacts for pre-solve velocities, one velocity iteration lets the contact solved last have the last word, and the safety sweep covers only statics above one half-extent of travel. Probed on 2026-10-02: a resting body hit by a fast heavy one ends beyond a gate in 246 of 1,536 cases across three spawn orders, static and dynamic gates; a slow push crosses a thin static wall in 9 of 320. | A fix exists and waits for the owner's decision: an arrival pass closes both matrices and costs `physics_step` +8.3% in the `physics` benchmark row and +5.4% in `physics-sparse`. [The pass's plan](plans/p2-correctness-pass.md) records the design, the measurements and where the patch is. |
| P3 — `core/assets/shader.rs`, `tungsten/asset_loader.rs::load_shaders` | Core allocates shader IDs in manifest iteration order; render independently seeds/allocates them. Numeric IDs need not match despite the core module's same-ID comment. Current bridge uses names. | Establish one allocator or distinct ID types before exposing cross-crate numeric lookup; do not silently change public handle semantics (D-016/D-057). |
| P3 — `tungsten/app.rs::stage_render`, `render/renderer.rs` | Recoverably skipped acquisition can return `Ok`, increment frame/capture accounting and claim capture success; readback errors only warn. Runtime render errors also remain logged rather than returned through `App::run`. | Needs a presented/skipped/failed result contract and explicit capture completion, while preserving D-029 surface recovery. Initialization failure propagation is fixed separately. |
| P3 — `tungsten/state.rs` | Multiple queued transitions can exit a state before its pending command-buffer spawns become live; automatic cleanup only sees live entities. Multiple stack instances sharing a `StateId` also share cleanup ownership. | Choose transition coalescing/flush semantics or per-instance ownership; preserve D-039/D-046 frame order. The demonstrated pause/menu leak is fixed without changing that API. |
| P3 — `tungsten/audio.rs` | Output assumes f32 and PCM conversion supports mono/stereo; a device with more channels is not fully mapped. `Play` can allocate in the callback, and full command rings drop commands. | Device-format/channel support and voice capacity/backpressure need an explicit audio policy (D-034). No audio-feature expansion here; hardware routing/listening is an owner check. |
| P3 — `core/physics/{step,broadphase}.rs` | Previous audit's remaining edges: sleep-tag adoption sees only touching final-substep contacts; extreme/non-finite externally written coordinates can overflow or cause enormous grid walks; entity generation is truncated to 31 bits; overlapping tile centers share warm-start identity. | Sleeping case remains unforced; input-domain and identity changes need explicit bounds/semantics. Generation/hash cases are theoretical or degenerate content. Preserve these findings without presenting them as freshly reproduced failures. |

## Recorded limits

- **Layer 1 loads each manifest alone** (`D-089`). `crates/tungsten-core/tests/manifests.rs` does not know the root sets, so a shipped material that names a shader of another root would fail there although the engine loads it. No shipped manifest crosses roots.

## Platform checks with no host available

Carried from the review, as of `0.27.0`.

- Metal (macOS) and DX12 (Windows) rendering on wgpu 30; Vulkan success isn't certification.
- macOS CoreAudio and Windows WASAPI audio on cpal 0.18.
- `.agents/skills` symlinks on a Windows clone (Developer Mode and `core.symlinks=true`, documented in `docs/agent-setup.md`).
- Launching the published archives on real Windows hardware. The `v0.0.0-test` rehearsal proved the Windows build, link and publish on GitHub; Linux archives were packaged and smoke-run locally through the launcher (`D-072`).
- `D-072`'s `x86-64-v3` gains on CPUs other than the reference Ryzen 5 6600H, and the launcher's automatic fallback on a real CPU without AVX2 (unit tests and a missing-level run cover it).

## Follow-ups

Each was checked against the tree on 2026-10-02.

- Truncated Ogg files fail at probe; decide whether to decode the available prefix, as MP3 does. `tests/audio_decode.rs` pins the error.
- Evaluate rtrb 0.4.0 against the locked 0.3.5, and adopt winit 0.31 once it leaves prerelease (0.30.13 is locked).
- Drop the RUSTSEC-2026-0192 exception in `deny.toml` when cosmic-text/fontdb stop using `ttf-parser`.
- Re-verify instruction loading once Claude Code reads `AGENTS.md` natively (2.1.277+); the `CLAUDE.md` import could then load it twice.
- Only `example-01-platformer` enables hot reload; the shader playground could too.
- Add `actionlint` to `just script-test` now that `release.yml` exists (it passed `actionlint` 1.7.12 and `zizmor` 1.30.1 when run by hand).
- Release archives don't bundle third-party license notices for statically linked crates.
- Both workflows install `libudev-dev`, but no locked crate links udev.
- `just --list` shows only the last line of the `quick` recipe's two-line comment.
- `scripts/check-repo.py` still names `examples/01_platformer/assets/sprites/player.png` as a deletion candidate. The platformer manifest registers it as `ex10_player`, so the entry no longer applies and can go.

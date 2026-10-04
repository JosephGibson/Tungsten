# Known issues

Open findings and follow-ups that no active plan owns. A fix removes its entry here in the same change. Sources: the repository review of 2026-09-25, whose archived copy keeps its fixed and historical parts, the P2 correctness pass of 2026-10-02 (`D-088`–`D-092`), M31 (`D-093`), the dense-pile investigation of 2026-10-02 (`D-094`), the 0.40 QA pass of 2026-10-03 (`D-095`–`D-101`) and the 0.44 test-suite overhead pass of 2026-10-04 (`D-111`, `D-112`; plan archived at `docs/plans/archive/test-suite-overhead.md`).

Priorities: P2 = functional follow-up (none open); P3 = limitation, rare edge case or contract clarification. Unless noted, these are source-path findings, not reproduced GPU or adversarial tests. `core/`, `render/` and `tungsten/` abbreviate the respective crate `src/` directories.

## Open findings

| Priority / location | Trigger and impact | Why deferred / next work |
| --- | --- | --- |
| P3 — `core/assets/shader.rs`, `tungsten/asset_loader/mod.rs::load_shaders` | Core allocates shader IDs in manifest iteration order; render independently seeds/allocates them. Numeric IDs need not match despite the core module's same-ID comment. Current bridge uses names. | Establish one allocator or distinct ID types before exposing cross-crate numeric lookup; do not silently change public handle semantics (D-016/D-057). |
| P3 — `tungsten/app.rs::stage_render`, `render/renderer.rs` | Recoverably skipped acquisition can return `Ok`, increment frame/capture accounting and claim capture success; readback errors only warn. Runtime render errors also remain logged rather than returned through `App::run`. | Needs a presented/skipped/failed result contract and explicit capture completion, while preserving D-029 surface recovery. Initialization failure propagation is fixed separately. |
| P3 — `tungsten/state.rs` | Multiple queued transitions can exit a state before its pending command-buffer spawns become live; automatic cleanup only sees live entities. Multiple stack instances sharing a `StateId` also share cleanup ownership. | Choose transition coalescing/flush semantics or per-instance ownership; preserve D-039/D-046 frame order. The demonstrated pause/menu leak is fixed without changing that API. |
| P3 — `tungsten/audio.rs` | Output assumes f32 and PCM conversion supports mono/stereo; a device with more channels is not fully mapped. `Play` can allocate in the callback, and full command rings drop commands. | Device-format/channel support and voice capacity/backpressure need an explicit audio policy (D-034). No audio-feature expansion here; hardware routing/listening is an owner check. |
| P3 — `core/physics/step/`, `core/physics/broadphase.rs` | Previous audit's remaining edges: sleep-tag adoption sees only touching final-substep contacts; extreme/non-finite externally written coordinates can overflow or cause enormous grid walks; entity generation is truncated to 31 bits; overlapping tile centers share warm-start identity. | Sleeping case remains unforced; input-domain and identity changes need explicit bounds/semantics. Generation/hash cases are theoretical or degenerate content. Preserve these findings without presenting them as freshly reproduced failures. |

## Recorded limits

- **Two pushed-body cases the arrival pass leaves** (`D-092`), from the probe behind it. A pusher of 1,000 times the body's mass at 120–480 px/s can still crush the body through a 4 px dynamic gate (6 of 320 slow-push cases; 41 before the pass). At 15,360 px/s a pusher spawned after the body can end past the body it pushed (17 of 512). Nothing passes a static gate in either. Closing the first took a wall-last contact order plus static softness for heavy partners in the probe, which changes every pile.
- **A soft contact carries a bounded load, so a deep pile is compressed** (`D-063`, `D-094`). A contact under a steady load `L` rests at `linear_slop + L / (m_eff · ω²)` with `ω = 2π · min(contact_hertz, 0.25 / sub_dt)`, and its push stops growing once `bias_rate · (penetration − linear_slop)` reaches `max_push_speed`. At the defaults and gravity 3,600 (example 01) a ball-ball contact sags 0.20 px per ball weight and answers at most about 65 ball weights; a floor contact about 271. An 11,502-ball pile in the platformer's pit puts about 56 ball weights on each bottom small ball: at a steady 60 FPS the bottom 128 px sits at 0.57 of its non-overlapping spacing and the pile stands 733 px tall where about 950 px would be free of overlap. It is stable, costs a 13 ms step and sleeps; this is the load limit, not the slow-frame collapse `D-094` closed. The engine's own piles use gravity 900 and see a quarter of the sag. `contact_hertz` is the lever: 60 Hz reads stacking 1.06 against 1.32 and 951 px, while `max_push_speed` 480, 8 substeps and mass by area change nothing. It needs a constant substep first: 60 Hz softens from a 1/60 s frame, so with the step bound alone the pile's height follows the frame time, and at a frame time of 17–21 ms the pile never settles or sleeps. Left to the fixed-step accumulator of the 1.0 criteria plan (`D-088`).
- **Screen transitions and mesh particles** (`D-093`). A transition's pass does not cover screen-space text, which draws after the post stack; a game fades its own text with `1 − StateStack::transition_cover()`, as example 03 does. Mesh particles draw above every sprite, so `z_order` cannot place them between sprites. At an odd window size the radial wipe leaves the center pixel at half brightness on the fully covered frame. Pixelate never hides the frame: the state changes under the coarsest mosaic. A live manifest edit of a particle mesh has not been exercised: only example 01 enables hot reload and it ships no mesh.
- **Layer 1 loads each manifest alone** (`D-089`). `crates/tungsten-core/tests/manifests.rs` does not know the root sets, so a shipped material that names a shader of another root would fail there although the engine loads it. No shipped manifest crosses roots.

## Platform checks with no host available

Carried from the review, as of `0.27.0`.

- Metal (macOS) and DX12 (Windows) rendering on wgpu 30; Vulkan success isn't certification.
- macOS CoreAudio and Windows WASAPI audio on cpal 0.18.
- `.agents/skills` symlinks on a Windows clone (Developer Mode and `core.symlinks=true`, documented in `docs/agent-setup.md`).
- Launching the published archives on real Windows hardware. The `v0.0.0-test` rehearsal proved the Windows build, link and publish on GitHub; Linux archives were packaged and smoke-run locally through the launcher (`D-072`).
- `D-072`'s `x86-64-v3` gains on CPUs other than the reference Ryzen 5 6600H, and the launcher's automatic fallback on a real CPU without AVX2 (unit tests and a missing-level run cover it).

## Follow-ups

Each was checked against the tree on 2026-10-02, except the last eighteen: ten from the 0.40 QA pass (2026-10-03), one from M32 (`D-110`) and the last seven from the 0.44 test-suite overhead pass (2026-10-04).

- Truncated Ogg files fail at probe; decide whether to decode the available prefix, as MP3 does. `tests/audio_decode.rs` pins the error.
- Evaluate rtrb 0.4.0 against the locked 0.3.5, and adopt winit 0.31 once it leaves prerelease (0.30.13 is locked).
- Drop the RUSTSEC-2026-0192 exception in `deny.toml` when cosmic-text/fontdb stop using `ttf-parser`.
- Re-verify instruction loading once Claude Code reads `AGENTS.md` natively (2.1.277+); the `CLAUDE.md` import could then load it twice.
- Only `example-01-platformer` enables hot reload; the shader playground could too.
- Release archives don't bundle third-party license notices for statically linked crates.
- The stock shaders exist twice (`crates/tungsten-render/src/shaders/stock/**` and `assets/shaders/stock/**`, `D-059`); including the asset copies as `sprite.wgsl` and `lit_sprite.wgsl` already do would halve every stock-shader edit but needs a decision.
- `Renderer::new` spends about 390 lines seeding shader IDs (`renderer.rs:145-537`).
- `logging.level` and `display.scale_mode` are parsed and unused (`DESIGN.md:143`, `core/config.rs:278-294`): wire or remove, owner's call.
- Particle `Burst { once: false }` only suppresses `ParticleSystemDrained` and `Pulse { total_pulses: Some(0) }` fires one pulse (`tungsten/src/particles.rs:313-347`): define the semantics.
- `render.max_frame_latency = 0` in the file passes `Config::load` and fails at renderer start (`render/surface.rs:118`), while `display.max_frame_latency = 0` warns and falls back (`core/display.rs:292-297`).
- Any file named `input.json` under a watched directory reloads as the action map (`tungsten/src/app.rs:454`).
- Perf: `env::var("TUNGSTEN_PERF_LOG")` every frame (`app.rs:1147`), the tween system cloning channel lists every frame (`tweens.rs:43`), tile proxies rebuilt from a full-map scan every frame.
- The perf runner's background-load scan (`D-095`) covers the measured runs of `run`, `suite` and `--sweep` only: capacity probes, `just smoke` timings and `just visual` still rely on the manual `pgrep` checks.
- Hand-written frame loops outside example 01 still run stages by hand that the headless harness (`D-110`) runs in order: `crates/tungsten/tests/particles.rs`, `crates/tungsten/src/tests/tweens.rs`, `state.rs` and `game_feel.rs`, and `examples/03_scene_state/src/states.rs`. Home: W15a, which steps the harness, or the Phase 5 QA pass.
- `scripts/bench.py` run from a tree export under `target/` (a parent or reference build) records the enclosing repository's commit and dirty-tree hash as the capture's provenance, not the export's; label such captures by hand, as the 0.40 QA evidence folders under `perf-runs/` do.
- `cargo shear` (`just udeps`) reports every `src/tests/` module included through `#[path = "../tests/…"]` from a file in a subdirectory as unlinked; an `ignored-paths` entry under `[workspace.metadata.cargo-shear]` would silence the false positives.
- Measure CI's wall time on the 0.44 release pull request: building the benchmarks in the bench profile took 148–193 s of a run of about 5 min before `D-112` moved it to the dev profile.
- A `LAUNCHES` row in example 01's route test (`examples/01_platformer/src/tests/level.rs`) for a pair no route uses is dead data that nothing flags; flagging it would be a new assertion.
- `just level-check` (the platformer generator's `--check` and its unittests) is not listed in `docs/agent-setup.md`'s check tiers.
- Test temp directories pile up in `/tmp`: the scene tests (`core/tests/assets/scene.rs`) and the manifest tests' helper (`core/tests/assets/manifest.rs`) never remove theirs; each test could remove its directory at its end.
- `LightUbo::byte_size()` (`render/lighting.rs:66`) has had no caller since 0.44 removed its test; it is public API, so removing it needs a break-ledger row.
- Bloom's shader ids 4..=7 must follow sprite (0) and SMAA (1..=3) or `Renderer::reload_shader` routing breaks; only a bloom test 0.44 deleted said so. Put the note beside the seeding at `render/renderer.rs:338`.
- `D-041` lacks an `**Amended by D-096:**` marker line above `D-111`'s.

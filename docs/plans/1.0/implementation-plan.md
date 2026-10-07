# Tungsten 1.0 implementation plan — draft

- **status:** in progress
- **goal:** Turn [criteria.md](criteria.md) into an ordered, gated path to a 1.0 tag: three phases, six gates, candidates in dependency order with a card each for Phase 5, a register from candidate to milestone and release, and the point by which each owner question needs its answer. [workflow.md](workflow.md) says how each candidate runs as Claude Code sessions.
- **non-goals:** Dates, time estimates, version numbers before 1.0, decision entries, implementation. Scoping, which criteria.md and the workstream files own. Reopening anything criteria.md records as answered by the owner.
- **files to touch:** This folder only while it is a draft. Each candidate becomes its own `phaseN-milestone-NN-slug.md` plan here when it is next to start.
- **ordered steps:** (1) The owner runs the definition gate on the agenda in §9: the questions, the amendments in §8, the candidate order and workflow.md; the accepted amendments go into criteria.md. (2) Step 0, now that 0.40 has released. (3) Phase 5: Tracks A and B, the frame-loop gate, Track C, then the Phase 5 QA pass. (4) Phase 6 to the feature gate, closing with its QA pass. (5) Phase 7: C1, the freeze gate, C2, the RC gate, C3, the 1.0 gate.
- **done-when:** Planning: the owner has agreed the phase split, each gate's checklist and the candidate order; every Must workstream has at least one placed candidate; criteria.md §9 links here instead of holding an order of its own. Program: every gate has a signed record in §11 and 1.0 is tagged.

Drafted 2026-10-03 at `afbc330`; revisions are listed at the end.

## Context digest

- criteria.md says what 1.0 means and scopes W1–W16 with their tiers. This file refined its order sketch and adds gates; since the definition gate it is the only order, and criteria §9 links here (amendment 13).
- [workflow.md](workflow.md) says how a candidate runs: session types, autonomy levels A, B, C and S (used on the cards in §3), what every milestone owes before its release, and how sessions share the tree and the reference machine.
- Milestones continue at M32. A candidate gets its number when its plan is written, so numbers follow the order in which work lands. The register (§10) maps candidates to milestones and releases.
- Candidate IDs: `W11a`, `W11b` and so on per workstream. W1 keeps its ladder names (M0a … M6), W2 its options (R0 … R5) and W10 its phases (C1 … C3).
- Gates have names, not numbers: G1–G3 already name physics and lighting proposals in [benchmarks.md](../../perf/benchmarks.md#open-proposals).
- From 0.42 sessions leave a milestone's work uncommitted, the release session's command block makes its only commit, and the owner pushes, tags and merges (`D-105`). Milestones that touch performance follow the [profiling workflow](../../perf/profiling-workflow.md).
- 0.40 released on 2026-10-03 (`v0.40.0`, squash-merged as `9cd5709`); the drafts' commits `afbc330`–`4ee92aa` are not ancestors of that commit, so later revisions cite `main` commits or tags. Its QA plan finished the same day (archived at `docs/plans/archive/qa-cleanup-0.40.md`); its follow-ups feed W8, W11, W12 and the Track B spikes (§3).
- The longest chain is W1's ladder (§6). With one tree, milestones run one after another, so the order sets when risk is found, not when 1.0 lands.

## 1. Shape

Three phases in place of the five that criteria §9 sketched. Phases 8 and 9 there held one workstream each and were gates rather than bodies of work, and fewer phases means fewer milestone renames when the order shifts. The definition gate agreed three (amendment 13).

| Phase | Theme | Ends at |
| --- | --- | --- |
| 5 Foundations | What later work stands on: logs and crash files, the headless harness, self-containment, serial render wins, and the shape of game code (stages, plugins, queries, bundles, the clock) | The frame-loop gate passed, Track C landed and the Phase 5 QA pass closed. W1 M2 and M3 may run on into Phase 6, ahead of M4 |
| 6 Features | What games use: UI screens, input, physics and audio gaps, the kit, prefabs, settings and saves, the CLI, example migration, the acceptance game | The Phase 6 QA pass, then the feature gate |
| 7 Release | C1 benchmarks, the freeze, C2 performance, release candidates, C3 QA | 1.0 gate |

## 2. Gates

Question numbers are criteria §10's; §7 lists the rest by the candidate that needs them. Each gate is a session with the owner present (workflow §2) and ends with a signed record in §11.

| Gate | Before | Passes when | Owner questions |
| --- | --- | --- | --- |
| Definition | Phase 5 | 1.0 definition chosen; [acceptance game](acceptance-game.md) spec with a feature map; tiers confirmed against that map; `wgpu`/`winit` and stability policy agreed for their decision entries (W4a), with the API snapshot tool if amendment 17 is accepted; the amendments in §8 settled; workflow.md agreed; W4 graduated, which seeds the break ledger. Agenda: §9 | Q1, Q11, Q24; Q8, Q9 and Q16, which §1 and C3 already assume; Q4, proposed here (§6) |
| Frame loop | W15a, W3a, W3b, W2 R1; W1 M0a if Q4 is still open | The acceptance game's pitch, screens, mechanics and feature-map rows written, and W6 and W13 tiered from them (definition gate record, §11); R1 spike curve written; R4 go or no-go, with a latency measurement if go; one decision set for W3's fixed step and clock, W15's stages and plugins, W1 M3's routing stages and, if R4 goes ahead, its hand-off point; threading rule; `Send`/`Sync` policy; tuple-query spike verdict | Q1's remainder, agreed with the owner on 2026-10-05 ([acceptance game](acceptance-game.md#proposed-tiers)): the gate confirms the W6 and W13 tiers proposed there and takes its five items for the gates; then Q5, Q6, Q7, Q13. Ran 2026-10-06: every item passed on its default; record in §11 (`D-128`–`D-132`) |
| Feature | Phase 7 | The acceptance game plays start to finish from a `tungsten package` archive, on the public API and the kit alone; every Must candidate landed or re-tiered with the owner; the kit's admission rule applied; the Phase 6 QA pass closed; the break ledger holds only what Phase 7 lands | Q3, Q10; Q12, since C1 opens Phase 7 (§7) |
| Freeze | C2 | C1 baselines and budgets dated; break ledger empty; arity-named queries removed; `missing_docs` clean; API snapshot taken | Q2 |
| RC | C3 | Every budget passes or has an accepted exception; release candidate tagged as a versioned prerelease, `v1.0.0-rc.N` with its own changelog section, which publishes without a pull request ([releases](../../releases.md#rehearsals-and-versioned-prereleases)) | — |
| 1.0 | The tag | Every row of the [release checklist](release-checklist.md) passed, or accepted by the owner, in a dated QA record | — |

The glyph gate (W1's T1, T1b or T2, [w01](w01-ui-text-suite.md) §9) is W1's own and sits inside Phase 5; it shared the frame-loop gate's owner session on 2026-10-06 and passed on its defaults (T1b, `D-126`; the DPI model, `D-127`; record in §11).

## 3. Phase 5: Foundations

**Before Phase 5.** 0.40 released on 2026-10-03. Its QA plan left follow-ups in [known issues](../../known-issues.md); each has a 1.0 home:

| 0.40 follow-up | Goes to |
| --- | --- |
| `logging.level` is parsed and never applied | W11a: the engine logger applies it |
| `display.scale_mode` is parsed and never applied | Removed before the freeze (glyph gate, `D-127`); the break-ledger row is W4b's |
| `Burst { once: false }` only suppresses the drained event; `Pulse { total_pulses: Some(0) }` fires one pulse | W8a, beside the burst latch |
| `render.max_frame_latency = 0` passes `Config::load` and fails at renderer start | W8b |
| Any `input.json` under a watched folder reloads as the action map | W12a, when each example gets its own folder; W11b moves bindings to the user folder |
| `Renderer::new` seeds shader IDs in about 390 lines | W8b, with the shader ID allocator |
| The platformer's test file runs to 2,366 lines (2,372 when the pass recorded it) | W14a, as its tests move onto the harness |
| A per-frame `TUNGSTEN_PERF_LOG` lookup, tween channel lists cloned per frame, tile proxies rebuilt from a full-map scan | C1 rows or C2 candidates; W8b if a fix is small |
| The stock shaders exist twice | W12a's shader ownership (amendment 14) |
| `D-095`'s background-load scan covers `run`, `suite` and `--sweep` only | Track B, before any spike uses capacity probes |
| `scripts/bench.py` run from a tree export under `target/` records the enclosing repository's commit and dirty hash as the capture's provenance | Track B: spikes capture from scratch copies, so each spike's README names its tree by hand (workflow §2) |
| `cargo shear` (`just udeps`) reports the `#[path]`-included `src/tests/` modules as unlinked | The Phase 5 QA pass, with the `ignored-paths` entry known issues proposes |

**Step 0, planning tooling.** `check_plans` in `scripts/check-repo.py` also reads `docs/plans/*/*.md`, skipping the archive, with a case in `scripts/test-check-repo.py`. The 0.40 QA plan, which edited both files, has landed (its step 11 added a citation check that reads this folder's files only as citation targets), so nothing blocks it. No milestone plan is written in this folder before it. Workflow §8 proposes three items beside it: headroom for `AGENTS.md` and the index, a milestone skill, and an API snapshot recipe. *Landed 2026-10-04 and released in `v0.42.0`, squash-merged as `28abcce`: 0a, 0b (`D-106`), 0c (the `tungsten-milestone` skill) and 0d (`D-107`, `just api`); workflow §8 records each.*

**Track A**, independent of the frame-loop gate, in this order after the definition gate unless the register says otherwise (§6 gives the reasons). Hard constraints: W14a before W1 M1; W2 R0 before the R1 spike; W11a and W9a before W12a; W1 M0a before M1.

| Candidate | Contents | Unblocks |
| --- | --- | --- |
| W14a | Headless harness that drives `App`'s own frame function | Every later test; W1 M1's `UiHarness` |
| W2 R0 | Interned asset IDs (a break, into the ledger), extract culling | The R1 spike's baseline |
| W1 M0a | Text engine split, here only if the definition gate answers Q4 (§6); otherwise after the frame-loop gate | W1 M1 |
| W11a | User folder, log file, panic hook, release symbols with the CI symbolization probe, no console on Windows; W7a's CPU-only Windows job beside it | W12a, W1 M5, W14b `package` |
| W9a | Licence notices in archives, the embedded font's OFL notice included | W12a in a release, W14b `package` |
| W12a | Self-containment, template skeleton, outside-copy check; the skeleton registers engine systems by hand until W15a (Track C) removes them; the getting-started guide starts here as W9b's first draft | W14b `new`, the acceptance game, every later template change and the guide (workflow §5) |
| W1 M1 | Core UI model, its layout spike and `UiHarness` ([w01](w01-ui-text-suite.md) §11) | The glyph gate |
| W8a | Capture-completion contract (a presented, skipped or failed render result); the engine-finding bugs: burst latch, tilemaps at `z_norm` 0 under `gpu_depth`, the lit-sprite material log, a once-per-ID warning for an unknown sprite | W1 M2 |
| W1 M0b | Input and display groundwork from the UI ladder, which takes over W5's focus loss, modifiers and scale factor. First, winit 0.31 if it has left prerelease, so the bridge is rewritten once ([w04](w04-api-freeze.md)) | W1 M3, W5a |
| W2 R2 | Late-acquire split: scene and post passes recorded before the acquire. It needs nothing from the frame-loop gate ([w02](w02-multi-core-rendering.md#options)), and W1 M2's overlay then builds on the split final pass. Per-worker encoders need a worker mechanism, which 1.0 does not have (`D-131`) | R4, if it comes back; W1 M2 builds on it |

**Track B**, the inputs to the frame-loop gate. These are spikes and designs, with evidence in `perf-runs/` and nothing merged that the gate could reverse (workflow §2):

- the profiling workflow's capture rules extended for worker threads (criteria §12: the worker count in `bench-config`), and `D-095`'s background-load scan extended to capacity probes if a spike uses them, before any capture with workers;
- the W2 R1 spike, after R0, plus an R4 prototype only if Q4 leaves R4 open;
- the W15 spike on tuple queries over `get_disjoint_mut`, and the stage and plugin design, checked against the kit's rule of no render types in core;
- the W3 design (fixed step, clock, timers) and W1 M3's routing stages, in one stage map.

Then the frame-loop gate.

**Track C**, after the frame-loop gate:

| Order | Candidate | Contents |
| --- | --- | --- |
| 1 | W15a | `Schedule`, stages, plugins and `DefaultPlugins` in core; named systems with `before` and `after`; schedule snapshots; the template's `main.rs` names no engine system |
| 2 | W3a | `Time` resource (real and game clocks, scale, pause, frame index) and `Timer` |
| 3 | W15b | Tuple queries and bundles; arity-named queries deprecated |
| 4 | W3b | Fixed step and interpolation, in the physics sync: the `D-018` seam stays (`D-129`) |
| 5 | W15c | Additive extracts, with tilemaps in the default |
| 6 | Phase 5 QA pass | An audit of what Phase 5 changed, in the 0.40 shape, before Phase 6 builds on it (amendment 18) |

W2 R1, the parallel default and tilemap extracts on W15b's column slices, was the track's fifth candidate until the frame-loop gate dropped it on its spike's verdict (`D-131`, §11); it is a 1.x backlog row.

W1's ladder interleaves with both tracks: M0a and M1 in Track A, the glyph gate once M1 has landed (passed 2026-10-06, `D-126`, `D-127`), M2 once the gate, W8a and W2 R2 have (its overlay extends R2's split final pass), and M3 once M2 and W15a have.

**Cards.** What each Phase 5 candidate costs to run, for ordering and for batching owner-present and unattended work. "Every CPU row" means the suite, or `--only` every row the code runs in ([done-when rules](../../perf/profiling-workflow.md#writing-done-when-checks)). Levels are workflow §4's. Phase 6's cards are in §4, written at the frame-loop gate on 2026-10-06; Phase 7's are written at the feature gate.

| Candidate | Touches | Decisions | Breaks and new surface | Checks beyond `just check` | Owner-only | Level |
| --- | --- | --- | --- | --- | --- | --- |
| W14a | `tungsten/app.rs` (a frame body apart from window, surface and audio); a `testing` feature; example 01's hand-written test loops | Harness behind `testing` | New surface | Every CPU row (frame loop); smoke | — | B |
| W2 R0 | `Sprite` and the registry in core, scenes and animations; `sprite_extract.rs`; every example's sprite IDs | Interned asset IDs, keeping `D-009`'s model | `Sprite.asset_id` | `gpu-throughput` `extract` `improved`, every other row not regressed; smoke; visual | — | C |
| W1 M0a | `render/text.rs` split into a device-free engine and a GPU half; font families in the manifest | Text engine split; font-fallback policy; font families | New text style fields | `gpu` not regressed after an A/A; visual; smoke | — | B |
| W11a with W7a | Logging, user folder and panic hook in the umbrella; a game identifier in `tungsten.json`; the examples' `main.rs`; the launcher; `release.yml` and `ci.yml` | User folder, log file and crash report (extends `D-008`); release symbols (extends `D-071`); one logger, which W1 M5's console reads too | A new `tungsten.json` field | Smoke; `just script-test`; the symbolization probe on both runners; one perf run that still parses its perf lines | A crash file from a shipped Linux archive; no console on Windows (no host) | B |
| W9a | `release.yml`, the archive layout, notice generation | Licence notices (extends `D-071`, `D-072`); a generator under `D-015`, if one is used | — | A rehearsal prerelease's archive | Reading the notices in it | B |
| W12a | The engine font embedded in the umbrella for the HUD, inspector and overlays; `templates/basic` as a workspace member; an outside-copy recipe; the smoke script | Standalone games; shader ownership (amendment 14) | The HUD stops reading the manifest's `mono` | Layer 1; smoke; visual; the outside-copy check | — | B |
| W1 M1 | A new `core/ui`; `UiHarness` on W14a | Layout dependency or a hand-rolled layout (`D-015`, from its spike); style model | New surface | None: nothing draws | API review | C |
| W8a | `screenshot.rs`, `renderer.rs`, `particles.rs`, `tilemap_extract.rs`, `sprite_extract.rs`, the lit-material path | Render result contract; burst and pulse semantics | Render result type; `App::run` returns runtime errors | Smoke; visual; `gpu-throughput` and `particles` not regressed | — | B |
| W1 M0b | Core input types, `input_bridge.rs`, `app.rs` `window_event` | The UI input decision's groundwork; winit 0.31 if it has shipped | Real `KeyCode` variants | Smoke | An alt-tab on the reference machine | B |
| W2 R2 | `renderer.rs`: scene and post recorded before the acquire | None, or an amendment to `D-087` | — | `gpu` and `integrated` judged on frame time and GPU engine time, since the cost moves between stages; visual on the direct and capture paths | — | B |
| Track B | Scratch copies; `perf-runs/` | Inputs to the frame-loop decision set | — | Captures with the worker count recorded | Verdicts at the gate | S |
| W15a | A schedule in core; `app.rs` drives it; W14a's harness steps it; every example's registration; `02_bench` from an empty plugin set | From the frame-loop gate | System registration; engine system names | Every CPU row, digests unchanged; smoke; visual; schedule snapshots | API review | C |
| W3a | `core/time.rs`; every reader of `DeltaTime` | From the gate | `DeltaTime` deprecated, removed at the freeze | Every CPU row, digests unchanged at scale 1; smoke | — | B |
| W15b | Queries and spawns in `core/ecs`; the command buffer | Tuple queries and bundles | Arity-named queries deprecated | `ecs` and `churn` not regressed, then every CPU row | API review | C |
| W3b | The accumulator in `app.rs`; the physics stage; interpolation in the physics sync through `PrevPosition`, the `D-018` seam untouched | From the gate (`D-129`; amends `D-088` and `D-094`); the `[time]` section's fields | Step and interpolation settings | `just physics-release` hashes; every row, digests unchanged at 1/60 s; smoke; visual | — | B |
| W15c | Extract registration; tilemaps in the default extract; example 01's extract | Additive extracts | Extract registration | Extract rows; smoke; visual | API review | C |
| Phase 5 QA pass | The tree | As its findings need | As its findings need | As 0.40's | Plan approval | A |
| W1 glyph gate, M2, M3 | [w01](w01-ui-text-suite.md) §6, §9, §11 | The glyph path and the DPI model, settled 2026-10-06 (`D-126`, `D-127`); UI input routing | M3: routed input | As w01 §11 sketches | The glyph decision (given); API review | S (done), then C |

- **W11a.** Perf lines are `log::debug!` records (`tungsten/app.rs:1188`) that `scripts/bench.py` reads from the example's output under `RUST_LOG=tungsten::app=debug,bench=debug`. Criteria §8.2 kept stderr in debug builds only, so the release builds that the runner measures would have lost them; [w11](w11-shipping-basics.md) now keeps stderr while `RUST_LOG` is set, or has the runner read the log file (amendment 19).
- **W2 R0.** Its ID API is what every game writes to name a sprite, hence level C. It lands before the R1 spike so that the spike's serial baseline has no string compares in it.

## 4. Phase 6: Features

The acceptance game's repository starts when W15a lands, late in Phase 5 ([acceptance game](acceptance-game.md), step 4), and grows through this phase. Each gap it hits becomes a candidate here or a row in the [backlog](backlog-1.x.md); workflow §7 says how gaps come back from its repository.

| Candidate | Contents | Needs |
| --- | --- | --- |
| W16a | Component registry; `components` in scene entries | W15a |
| W6a | Kinematic bodies, sensors, shape and ray queries and casts; the crowd probe before the kit's mover (`D-132`) | W3b |
| W13a and W13b | Kit basics; transform hierarchy with bodies on roots (Q21, `D-132`); the physics sync is the plugin's (`D-128`) | W3a, W16a; W15a |
| W16b | Prefab assets, `spawn_prefab`, Tiled object classes, the tilemap builder (`D-132`); then the kit spawner | W16a |
| W13c and W13d | Character controllers; gameplay helpers | W6a |
| W11b | Settings: shared atomic write, user layer, corrupt-file fallback, the settings the engine persists (`ui_scale` among them, `D-127`) | W11a |
| W1 WL | World labels: the world-label path for damage numbers and name tags (`D-126`, `D-132`; [w01](w01-ui-text-suite.md) §11, §14) | W1 M2 |
| W1 BC | Broader controls the settings screen needs, sliders and checkboxes ([w01](w01-ui-text-suite.md) §8; Q8, `D-102`) | W1 M3 |
| W1 M4 | Gameplay screen fixture with the settings screen | W1 M3, W11b, W1 BC |
| W1 M5 | Engine debug views; the log console on W11a's logger | W1 M4 |
| W5a | Gamepad (Q9, `D-102`) | W1 M0b |
| W6b and W6c | Layers and masks; audio buses, a voice limit per sound, voice handles and fades (`D-132`) | W6a; the acceptance game |
| W11c | Save slots (Q16, `D-102`) | W11b |
| W14b | CLI `check`, `package`, `new` | W16b, W9a, W11a, W12a |
| W12b with W1 M6 | Examples 01, 03 and 04: template layout, plugins, kit and UI, in one pass per example | W13, W1 M5 |
| Phase 6 QA pass | An audit in the 0.40 shape that also takes W8b: the shader ID allocator, queued transitions, the remaining P3s (amendment 18) | Every Must candidate above |

W2 R3 (text preparation off the main thread) and W2 R4 (the render thread) left this table: R4 at the definition gate (`D-102`) and R3 at the glyph gate, since glyphon's `prepare` stays on the main thread under T1b (`D-126`); both are 1.x backlog rows.

**Cards**, written at the frame-loop gate on 2026-10-06 in the shape of §3's. Levels are workflow §4's; "W13a and W13b" and the other two-group rows are single candidates whose groups share one plan, not `D-108` pairs; W12b with W1 M6 is the phase's one pair.

| Candidate | Touches | Decisions | Breaks and new surface | Checks beyond `just check` | Owner-only | Level |
| --- | --- | --- | --- | --- | --- | --- |
| W16a | A component registry in core (a `TypeId`-keyed `World` resource over serde types); the scene loader's entries; engine components registered by their plugins | Component registry and prefabs, one entry with W16b (extends `D-046`) | `register_component`; the `components` field | Layer 1; smoke | API review | C |
| W6a | The body model in `core/physics` (kinematic bodies, sensors, contact and enter events), the spatial grid's queries and casts, tile collision for every body kind and query, the physics debug draw | Kinematic bodies, sensors, casts (amends `D-033`; touches `D-064`, `D-065`) | `BodyKind` and `CollisionEvent` grow (`#[non_exhaustive]`) | `just physics-release` with the hashes of existing scenes unchanged; `physics` and `physics-sparse` not regressed; the crowd probe with several spawn orders and frame dt as a parameter | — | B |
| W13a and W13b | The new `tungsten-kit` crate: animated sprite, lifetime timers, despawn outside a region or the view, a sound on an event, `cursor_to_world`; `Parent`, `Children`, a local transform and a propagation system in `post_update` after `physics_sync`; recursive despawn through the command buffer; the umbrella's re-export behind a default feature; `AGENTS.md`'s "Where code goes" | The kit: admission rule, dependency direction, the `AGENTS.md` row; bodies on roots (given, `D-132`) | A new crate and its surface | A harness test per item; smoke; `integrated` and `particles` not regressed | API review | C |
| W16b | The manifest schema (`prefabs` section, an assets-table row), `spawn_prefab` and its command-buffer form, the Tiled loader's object classes and property overrides, a public `TilemapData` builder with tile setters; then the kit's spawner | Prefabs (with W16a's entry); the tilemap builder's shape | `TilemapData` construction; `prefabs` | Layer 1; the malformed-prefab tests; smoke; example 01 spawns one object kind from a Tiled layer | API review | C |
| W13c and W13d | The kit's controllers on kinematic bodies in `fixed_update` before `physics_step`; health, damage, invulnerability time and the hit flash on `damage_flash`; trigger zones on sensors; the path follower; the per-entity state machine; example 01's player onto the controller | Controller scope (Q22) | New surface | Harness tests; `just physics-release`; example 01's tests; smoke | API review; a playthrough of example 01 on the controller | C |
| W11b | Config loading (a user layer between `tungsten.json` and the environment), the atomic write shared with the action map, the persisted settings (display, `ui_scale`, volume per bus, bindings), the corrupt-file fallback | Settings and the user layer (Q15; extends `D-008`, amends `D-045`) | The loading order; the action map's persisted path | Layering and fallback tests; smoke with a user file present | — | B |
| W1 WL | A world-label resource and extract in the umbrella (POD entries projected by the camera, culled and capped), one paint-list text batch in the overlay's lowest layer, the content-keyed cache (`D-085`); example 02's name tags move onto it | None beyond `D-126` | New surface | The w01 §10 world-anchored scenario (cache plateau, prepared glyphs, p99); `integrated` not regressed; smoke; visual | — | B |
| W1 BC | `core/ui` widget kinds and behaviours (checkbox, slider), `UiEvent::ValueChanged`, their paint commands, the fixture | None: the style model holds | New surface | Harness tests by pointer, keyboard and `ui_*` actions; paint-list snapshots; smoke | API review | C |
| W1 M4 | State-owned roots and transition opacity, UI animation tracks, binding-aware prompts, prewarm, the settings screen with the capture-next-input mode, the fixture | UI animation tracks; the rebind capture mode | New surface | The w01 §10 budgets for a warmed 100-widget menu; no cold-glyph spike after a prewarm; smoke | A playthrough of the fixture's menus on keyboard and gamepad | B |
| W1 M5 | The HUD, timing overlay and inspector as read-only views; scroll containers; movable windows with persisted placement; picking blocked behind windows; the log console on the engine logger; the frame-time graph on a mesh paint command; the picked-widget panel | The mesh paint command; the window-placement file | `DebugHud` row APIs preserved | Perf-suite telemetry rows not regressed; HUD toggles and defaults preserved; smoke | — | B |
| W5a | The input bridge's gamepad backend, `ActionMap` bindings for sticks and buttons, `input.json`, navigation repeat from sticks, the template's defaults | The gamepad crate (`D-015` rule 1), its Linux backend checked for threads and `libudev` | `input.json` gains binding kinds (additive) | `just deps`, `just notices`; bridge tests; smoke | A gamepad on the reference machine | B |
| W6b and W6c | Layers and masks on bodies, sensors and queries; the audio mixer (buses, a voice limit per sound, voice handles, fades), `AudioCommand`, the `rtrb` ring | Layers and masks (amends `D-033`); the audio policy for `D-034`'s ring | `AudioCommand` grows; body and sensor components gain layer fields | `just physics-release`; `physics` not regressed; audio decode and command tests; smoke | Listening to the buses and a crossfade | B |
| W11c | A versioned save-slot envelope over game-owned serde types with game-owned migrations, slot listing, atomic writes in the user folder | Save slots (narrows DESIGN's non-commitment) | New surface | A schema-bump migration test; a corrupt-slot test; smoke | — | B |
| W14b | `tools/cli/` (`tungsten-cli`, binary `tungsten`): `check` over manifests, scenes and prefabs; `package` on W9a's notices and W11a's symbols; `new` from `templates/basic`; argument parsing and archive writing by hand or by crate (Q20) | The CLI and its dependencies (`D-015`) | The promised commands and flags (`D-103`) | CLI tests in `just script-test`'s shape; a packaged archive launches; `new` passes the outside-copy check | — | C |
| W12b with W1 M6 | Examples 01, 03 and 04 onto the template layout, plugins, the kit and the UI views, one pass per example; the template follows | None | None public | Per-example smoke; workload versions bumped where benchmark work changes; the examples' tests on the harness; visual where a fixture changes | A playthrough of each example | B |
| Phase 6 QA pass | The tree | As its findings need, W8b's among them | As its findings need | As 0.40's | Plan approval | A |

Then the feature gate.

## 5. Phase 7: Release

| Order | Candidate | Contents |
| --- | --- | --- |
| 1 | C1 | Dated 1.0 baseline; new rows for UI, text, startup, a template or acceptance-game frame, hierarchy, kit systems and tuple queries; a budget per row. Fixes that need an API change land now |
| 2 | W4b | Break ledger empty, arity-named queries removed, `#[non_exhaustive]` sweep, `missing_docs`, rustdoc examples, API snapshot (its tool is a `D-015` question, or amendment 17's choice). Then the freeze gate |
| 3 | C2 | Rows over budget, largest gap first; internal changes only |
| 4 | W7b, W9b, W9c | Support tiers in the README and a macOS build if Q3 says so; the getting-started guide's final pass; crates.io if Q2 says so |
| 5 | C3 | The release checklist on a candidate tagged `v1.0.0-rc.N`; a failure is fixed and a new candidate tagged. Then the 1.0 gate |

## 6. Critical path and the queue

W1's ladder is the longest chain: M0a → M1 → glyph gate → M2 → M3 → M4 → M5 → M6, then the feature gate and all of Phase 7. Three choices keep it short:

- **Answer Q4 at the definition gate.** Criteria §4 makes the R4 answer an input to where the text engine lives, so while Q4 is open, M0a waits for the frame-loop gate and its spikes. Proposed answer: R4 moves to 1.x and comes back only if C1 shows a game-frame row over budget that R4 would fix. It pays only where a CPU-bound frame has GPU slack (at most about 25% on `integrated`, [w02](w02-multi-core-rendering.md#where-frame-time-goes)). It adds a frame of latency, turns every main-thread renderer call into a command or a sync point, and changes what `total` means.
- **Start M0b in Track A.** It is independent of M0a (w01 §11) and of the frame-loop gate.
- **Run M1 in Track A.** The core UI model needs M0a's text API and W14a's harness, and nothing from the frame-loop gate, so the glyph gate can come as early as the frame-loop gate does.

After that, M2 waits on W8a, W2 R2 and the glyph gate, M3 on W15a, and M4 on W11b and, if Q8 says so, W1 BC. Each of those starts in an earlier track, so none should hold the ladder up.

**The queue.** Everything in the tree goes into the next release commit (workflow §6), so milestones run one after another. The critical path then orders risk rather than setting a finish date: the glyph gate and the frame-loop gate are where a wrong guess costs most, and the queue reaches both early. Proposed Phase 5 queue, with owner-present work and capture work kept apart:

1. Step 0, then Track A in the table's order: W14a and R0 first because every later test and the R1 spike stand on them, then the head of W1's ladder, then the shipping basics and the template.
2. Track B in unattended sittings as each becomes possible: the R1 spike once R0 has landed; the tuple-query spike and the frame-loop design at any time.
3. While the spikes wait for their gate: W8a, W1 M0b, W2 R2, and the glyph-path prototype once M1 has landed.
4. The glyph gate and the frame-loop gate, in one owner session if both are ready (ran 2026-10-06, §11).
5. Track C, then the Phase 5 QA pass; W1 M2 and M3 as their inputs land.

A second clone running W1's ladder beside the rest is the one place where parallel sessions shorten the road. It pays only if the owner can review two streams of changes.

**Pairs.** Two small candidates may share one milestone plan and one release when both are level A or B, touch different files and sit next to each other in the queue (`D-108`, which revises the definition gate's one release per candidate, §11 agenda item 11). The register (§10) holds each pair as one row, and a pair's plan splits in two if a shared file or a level C turns up. Phase 5's pairs, settled by the owner on 2026-10-04:

- **W11a with W7a**, as the cards already had it.
- **W9a with W12a.** W9a's steps run first, so W12a's release carries the licence notices it needs. W12a is larger than the rule's "small" (an outside-copy check, smoke and visual); the owner paired them anyway.
- **W1 M0b with W2 R2.** M0b changes the core input types, `input_bridge.rs` and `app.rs`; R2 changes `renderer.rs`. If winit 0.31 has left prerelease when the plan is written, M0b upgrades it first and that reaches `renderer.rs`, so the pair splits. R2's perf baseline is taken after M0b's steps.

W8a and W1 M0b do not pair: both change `crates/tungsten/src/app.rs` (`App::run` and `window_event`). Phase 6 has one pair, W12b with W1 M6 (§4, written at the frame-loop gate); its two-group rows (W13a and W13b, W13c and W13d, W6b and W6c) are single candidates. Phase 7's pairs are proposed when the feature gate writes its cards.

## 7. Questions by when

| Question (criteria §10) | Needed by |
| --- | --- |
| Q1 definition and acceptance game, including its genre (§9 proposes one); Q11 `wgpu`/`winit` | Definition gate |
| Q8 UI cut line; Q9 gamepad; Q16 save slots | Definition gate: §1 and C3 already assume answers |
| Q24 kit stability tier | Definition gate, with the stability policy (criteria §8.4) |
| Q4 multi-core scope | Definition gate (proposed, §6); at the latest the frame-loop gate |
| Q5 worker mechanism | Frame-loop gate, after the R1 spike |
| Q6 `Send`/`Sync`; Q7 fixed-step scope; Q13 clock while paused | Frame-loop gate |
| Q14 logs on by default | W11a starts |
| Q15 persisted settings, corrupt files | W11b starts |
| Q10 audio and physics minimum | W6a starts; the rest at the feature gate |
| Q21 bodies on roots only | W13b starts |
| Q22 controller scope | W13c starts |
| Q20 CLI tiers and dependencies | W14b starts |
| Q3 platform tiers and a macOS build | Feature gate, since the release-workflow work falls in Phase 7 |
| Q2 crates.io | Freeze gate: crate names and metadata freeze with the API. W12a does not wait for it: amendment 14's default leaves the sprite shaders where they are |
| Q12 budgets | Feature gate, since C1 opens Phase 7 |

## 8. Amendments proposed to criteria.md

From reviews of criteria.md and the UI draft on 2026-10-03: 1–13 in the first pass, 14–19 in the second. For each, the owner accepts or rejects it, and accepted ones are folded in.

1. **Answers that §1 and C3 already assume.** §1's acceptance game includes gamepad play and settings for rebinding and display, and C3's playthrough adds a save slot carried through a schema bump. Yet Q8 (broader controls), Q9 (gamepad) and Q16 (save slots) are open, and §3 makes W5 and save slots conditional. Answer all three at the definition gate, or mark those items *if Qn* in §1 and C3.
2. **W5 repeats UI M0b.** Focus-loss release, modifiers and the scale-factor resource are already in UI M0b (w01 §11). Narrow W5 to the gamepad and its `D-015` dependency.
3. **W13 and W16 depend on each other.** §9 orders the kit before prefabs, but the kit's spawner takes a W16 prefab ID, and W16 registers the kit's components. Split W16: the registry (W16a) before the kit, and prefab assets (W16b) before the spawner.
4. **One engine logger, designed twice.** W11 moves `env_logger` into the umbrella, and UI M5 wants a log console on "an engine-owned logger that applies `logging.level`" (w01 §11). Design W11a's logger so that the console can read it, and record both in one decision.
5. **Two headless harnesses.** W14 has a harness for `App`, and UI M1 a `UiHarness`. Build the second on the first.
6. **Examples rewritten twice.** W12 moves examples 01, 03 and 04 to the template layout, and UI M6 migrates the same three. Make it one pass per example.
7. **W1 waits on more than it needs.** §9 puts all of W1 after the frame-loop design. M0b is independent of it, and M0a waits only on the R4 answer (§6 above).
8. **The acceptance game cuts tiers but arrives last.** §1 uses it to decide W5 and W6, and §9 builds it after the kit and prefabs. Write its spec at the definition gate ([acceptance-game.md](acceptance-game.md)) and start it when W15a lands.
9. **W7 and W9 have early parts.** W7's CPU-only Windows job tests W11a's folder paths, atomic rename and crash hook without a GPU, so it belongs beside W11a. W9's licence notices are needed as soon as W12a embeds JetBrains Mono and by W14's `package`, not in the closing phase.
10. **The burst latch is listed twice,** under W6 (§8.1) and among W8's engine-finding bugs. Keep it in W8a.
11. **The CLI needs its own package name.** The umbrella package is already `tungsten`. Proposed: `tungsten-cli` in `tools/cli/`, beside `tools/launcher/`, with `[[bin]] name = "tungsten"`. Cargo can report a doc output collision when a library and a binary share a name, so set `doc = false` on the binary before W4 gates rustdoc.
12. **Size.** At about 25,000 tokens, criteria.md no longer fits in one read: on 2026-10-03 the Read tool cut it off at line 333. Graduation (folder README) shrinks it one workstream at a time. Also move the revision paragraph under the header into a dated list at the end, and move the answered questions (17, 18, 19, 23, and the paragraph above the list) into an "Answered" list, so that §10 holds only open ones. *The two moves were applied on 2026-10-03 with no change to their text; graduation remains.*
13. **Phases.** Replace §9's five phase labels with the three in §1 above, or record why five are better.
14. **Shader ownership depends on Q2.** §8.3 moves `sprite.wgsl` and `lit_sprite.wgsl` into `tungsten-render`, with copies in `assets/shaders/` for hot reload. A git dependency checks out the whole repository, so `include_str!` from `../../../assets/shaders/` already works for a game in its own repository (§2.2 says so); only a crates.io package (Q2) needs the files inside the crate. 0.40 went the other way: QA step 7 deleted the dead in-crate `sprite.wgsl`, and a QA follow-up proposes compiling in the asset copies of the stock shaders too. Make the move conditional on Q2, and settle the stock shaders' two copies in the same decision. Proposed default: W12a leaves both shaders where they are, since it lands long before Q2 is due (§7); a yes on Q2 moves them in W9c as an internal change.
15. **Q8 adds a milestone.** §4's proposed cut is M0–M6, and none of those steps holds sliders or checkboxes, which the settings screen in M4 needs. If Q8 says yes, add W1 BC before M4 (§4 above); if no, settings leave the acceptance game.
16. **A template for agent-built games.** This engine is built in Claude Code sessions, and games made from the template will be too. `templates/basic` gains an `AGENTS.md` (and a `CLAUDE.md` that imports it) with a game repository's rules: the public API and the kit only, registry IDs not paths, actions not keys, systems by stage, a harness test per system, `tungsten check` before a commit, where the engine's documentation lives. The acceptance game starts from it, so its sessions test it, and C3 gains a check (RC-A9): a fresh session in a copy of the template adds a scripted feature using only the template, the guide and rustdoc. Amends §8.3's layout and §8.8's C3.
17. **Choose the API snapshot tool at the definition gate.** §8.8 leaves it as a `D-015` question for C3, so new public surface is found all at once at the freeze. Chosen with W4a, a tracked snapshot can be updated by each milestone (workflow §5, §8), and its diff shows in each commit the owner reviews. The choice includes how the tool runs on the pinned stable toolchain (`D-069`).
18. **QA passes close Phases 5 and 6.** §8.1 proposes one W8 pass. Phase 5 reworks the frame loop, the schedule, queries and the extract, and Phase 6 builds features on them; the 0.40 audit found eight bugs after Phase 4. Run a pass in the 0.40 shape at the end of each phase, with W8b inside the second.
19. **Perf lines through the engine logger.** §8.2 keeps stderr in debug builds only, but perf lines are `log::debug!` records that the runner reads from a release build's stderr (§3 above). Keep stderr while `RUST_LOG` is set, or have the runner read the log file, and test it in W11a.

## 9. Definition gate agenda

The first gate session (workflow §2) takes these items in order. Defaults are proposals; the owner's answer replaces each. Ran on 2026-10-03; the answers are in its record (§11).

| # | Item | Options | Default proposed | When settled |
| --- | --- | --- | --- | --- |
| 1 | Q1 definition | A; A and B; A, B and C | A and B (criteria §1), with crates.io left to Q2 | W4 is Must; checklist sections A and B stand |
| 2 | Q1 acceptance game | One game as the test, or none | One game, specified in [acceptance-game.md](acceptance-game.md) | Its feature map cuts W5, W6 and W13 |
| 3 | Q1 genre | The owner's pick | A small top-down action game: a few rooms, one enemy kind from a spawner, keys and doors, a boss room, and a save slot for progress. It gives the kit's top-down mover a user while the migrated example 01 gives the platformer controller one, so both meet W13's admission rule, and it exercises sensors, prefabs from Tiled object layers, health and damage, two audio buses and gamepad play | The pitch, screens, mechanics and feature map are written |
| 4 | Q11 `wgpu` and `winit` | Hide them; an advanced tier; accept their majors | Hide them from the promise: a curated umbrella re-export in place of `pub use tungsten_render as render`, engine-owned input and format types, and the `Renderer` methods that take or return `wgpu` types behind a game-facing handle or a `doc(hidden)` tier outside the promise. No example imports either crate (criteria §7) | W4a's decision; every later candidate designs against it |
| 5 | Q8 UI cut | Broader controls in or out; a text field in or out | Sliders and checkboxes in, as W1 BC; scroll containers are in M5 already; text fields and IME out | W1 BC is placed (amendment 15) |
| 6 | Q9 gamepad | In or out | In: the game plays on a gamepad, and UI navigation needs only bindings | W5 is Must; its crate's Linux backend is checked for threads and `libudev` |
| 7 | Q16 save slots | In, or settings only | In: the envelope and listing are small, and C3's playthrough uses a slot | W11c is Must |
| 8 | Q24 kit stability | Inside the promise; a tier of its own | Inside the promise, with `#[non_exhaustive]` on kit settings structs. The umbrella re-exports the kit, so a semver track of its own would also need that re-export kept outside the promise | The freeze's scope |
| 9 | Q4 multi-core scope | R1 and R2 only; R4 too | R4 to 1.x (§6), back only if C1 shows a game-frame row over budget that R4 would fix | M0a and M1 run in Track A |
| 10 | Amendments 1–19 (§8) | Accept or reject each | Accept | Folded into criteria.md and the workstream files |
| 11 | [workflow.md](workflow.md) and Step 0's items | Agree or revise; one release per candidate (about 45 from 0.41, workflow §1) or Track A's small candidates batched | Agree, one release per candidate | Plans cite it; Step 0 starts |
| 12 | Graduation | Which workstreams graduate now | W4, which seeds the break ledger that W2 R0 and W8a write to, and W9, W11, W12 and W14, whose scope the gate settles. W2, W3 and W15 at the frame-loop gate. The rest as their first milestone nears | Those files get their candidates and done-when checks |
| 13 | [Release checklist](release-checklist.md) | Agree or revise | Agree, with RC-A9 if amendment 16 is accepted | C3's list |
| 14 | Acceptance game repository | A repository of its own on GitHub; a folder beside this checkout | A GitHub repository of its own, cloned beside this checkout on the reference machine with its target folder budgeted (workflow §6), with the engine as a git dependency on this repository's `v0.NN.0` tags | Workflow §7 and [acceptance game](acceptance-game.md) step 4 name it |

## 10. Register

One row per candidate, or per pair that shares a plan and a release (§6, `D-108`), in plan order. A plan session fills in the milestone plan, and the plan's release step (a release session, for a resume) the release and status. Spikes take no milestone or release; their verdicts go in §11 with the gate they feed.

| Candidate | Milestone plan | Release | Status |
| --- | --- | --- | --- |
| Step 0 | — | 0.42 | Released 2026-10-04 in `v0.42.0`, squash-merged as `28abcce` (0a–0d; `D-106`, `D-107`) |
| W14a | [M32](../archive/1.0/phase5-milestone-32-headless-harness.md) | 0.43 | Released 2026-10-04 in `v0.43.0`, squash-merged as `9f10746` (`D-110`); run 2026-10-04: steps 2–6 done, step 1 skipped (its A/A failed); a release session cut it |
| W2 R0 | [M33](../archive/1.0/phase5-milestone-33-interned-asset-ids.md) | 0.45 | Released 2026-10-04 (`v0.45.0`), squash-merged as `6a61cc2` (`D-113`, `D-114`); run 2026-10-04: steps 1–5 done, with the owner's acceptance of step 1's A/A reading, `particles` `unattributed` p50 and culling's `gpu` `extract` cost, and one test file outside the plan's list (`examples/01_platformer/src/tests/camera.rs`) |
| W1 M0a | [M34](../archive/1.0/phase5-milestone-34-text-engine-split.md) | 0.46 | Released 2026-10-04 (`v0.46.0`), squash-merged as `20bd160` (`D-115`–`D-117`); run 2026-10-04: steps 1–10 done, with the owner's answers to Q11 (`unicode-script` as a direct dependency, `D-116`) and Q12 (`gpu-visual.png` regenerated: the old fixture showed the machine's installed JetBrains Mono), and one file outside step 7's list (`text/gpu.rs`) |
| W11a with W7a | [M35](../archive/1.0/phase5-milestone-35-logs-crash-reports.md) | 0.48 | Released 2026-10-05 (`v0.48.0`), squash-merged as `0a1f32f` (`D-119`–`D-121`); the tag is on a fix commit, `777cafa`, for the Windows test job's first-run failure (the shader-coverage messages' `\` paths); run 2026-10-05: steps 1–9 done, step 7's local Windows cross-check not run (no MSVC tools on Linux), and step 5's split also drops `.debug_gdb_scripts`, which its done-when needed |
| W9a with W12a | [M36](../archive/1.0/phase5-milestone-36-licence-notices-template.md) | 0.49 | Released 2026-10-05 (`v0.49.0`), squash-merged as `346ca9d` (`D-122`, `D-123`); run 2026-10-05: steps 1–7 done. The notice collector takes the crate set from `cargo tree`, since `cargo metadata`'s resolve keeps edges the build never activates, and A4's counts were one high: 204 crates on Linux and 177 on Windows. The template gains a `src/lib.rs` for its tests and a generated sprite, the root sprites' and sounds' origin being unrecorded (known issues) |
| W1 M1 | [M37](../archive/1.0/phase5-milestone-37-core-ui-model.md) | 0.50 | Released 2026-10-05 (`v0.50.0`), squash-merged as `01f9c55` (`D-124`, `D-125`); run 2026-10-05: steps 1–8 done, the API approved after step 1 as sketched. Taffy 0.14 admitted by the spike (every case passed, a second solve measured nothing; one crate over the lock); the `UiTree` resource, the style model, hit testing, focus, the `TextNodeStore` seam and `UiHarness`, 62 `ui::` tests and 6 harness tests; nothing draws until M2 |
| Track B spikes | — | — | Done. Five spikes ran unattended on 2026-10-05 in scratch copies of `01f9c55` (the 0.50 tree): the W2 R1 fork-join extract (no-go as designed), the W15 tuple queries (go, one cost), the W15 stage-and-plugin design (go, five findings), the W3 stage map (go, thirteen findings) and the W1 glyph-path prototype (T1 holds, T1b the per-window rule). Their verdicts, numbers and evidence paths are in the gate records (§11, 2026-10-06), which settled them as `D-126`–`D-131`. The capture-rule extension for worker threads waits until a candidate spawns one (`D-131`) |
| W8a | — | — | Not started |
| W1 M0b with W2 R2 | — | — | Not started; paired 2026-10-04 (`D-108`), split if winit 0.31 has left prerelease when the plan is written. W2 graduated on 2026-10-06; R2's step and done-when are in [w02](w02-multi-core-rendering.md) step 2 |
| W15a | [M38](../archive/1.0/phase5-milestone-38-schedule-stages-plugins.md) | 0.52 | Released 2026-10-06 (`v0.52.0`), squash-merged as `36b766d`, PR #53 (`D-133`); run 2026-10-06: steps 1–5 done, the API approved after step 1 with every default. `tungsten_core::schedule` (five closed stages, `before`/`after` constraints, plugins), `DefaultPlugins` and the six `App` methods, `PhysicsPlugin` and `WindowSize` in core; the template and examples 01, 03 and 04 name no engine system, example 01's input and contact systems run in `fixed_update`; `InspectorState`'s three row functions deprecated to W4b. The sitting read 0 `regressed` over every CPU row with digests 8 of 8 and the determinism hash unchanged; the `particles` row owns three named systems, its baseline restarting at 0.52 |
| W3a | [M39](../archive/1.0/phase5-milestone-39-game-clock-timers.md) | 0.53 | Released 2026-10-06 (`v0.53.0`), squash-merged as `2103040`, PR #54 (`D-134`); run 2026-10-06: steps 1–5 done. Step 4's sitting first stopped the release on `ecs` `stats_decay` p50 (`compare-direct` +0.130 ms, 0.002 ms past the sitting's A/A move), which the owner accepted under the plan's Q7 as the per-run mode: the first tree suite held five mode runs of five, and the second direct pair, the cross pair without that suite and the aligned pair read it `noisy`. `Time` in core with the real and game clocks, scale, pause, elapsed time and the frame index, and a plain `Timer`; every engine, example and template reader on `Time`, transitions on the real clock and `DeltaTime` deprecated to W4b. The physics hashes, the eight row digests and the pixel fixtures held at scale 1, with smoke, visual and the physics release tests green |
| W15b | [M40](../archive/1.0/phase5-milestone-40-tuple-queries-bundles.md) | 0.54 | Released 2026-10-06 (`v0.54.0`), squash-merged as `7bc0345`, PR #55 (`D-135`); run 2026-10-06: steps 1–3, 5 and 6 done, the API approved after step 1 with every default, step 4 skipped at its control check (the rebuilt `entity_mut` control vectorizes, so the lone column's cost is recorded: `single_mut` +22.9% a row, `bounds_wrap` +0.046 ms on the aligned pair). Step 5 first stopped the release on `gpu` and `gpu-throughput` `stage.extract` p50 (+12%, +4%) on the tuple form; after three retries the default extract reads the slice form with nested `for_each`, whose row loop is the arity loop's, and the sitting read no owned `regressed` outside the plan's rules (`churn_scan` +0.028 ms as placement). `ecs::query` with the eight `World::query*` forms and `With`/`Without`, eleven arity functions deprecated to W4b, `Bundle`, `spawn_with`, `insert_bundle`, the buffer forms, `RigidBodyBundle`, `resource`/`resource_mut`; example 01's player spawns in one call and the template is on the new API. Digests 8 of 8, the physics hashes and the pixel fixtures unchanged, smoke, visual and the physics release tests green |
| W3b | [M41](../archive/1.0/phase5-milestone-41-fixed-step-interpolation.md) | 0.55 | Released 2026-10-07 (`v0.55.0`), squash-merged as `cc3ded9`, PR #56 (`D-137`); plan written 2026-10-06 at `7bc0345`, critiqued in two rounds and approved with the stated defaults; run 2026-10-07: steps 1–6 done. Step 2 first stopped on its criterion pair (`physics_sync` about 47% over the copy at 12,000 bodies, no faster form), which the owner accepted under the plan's Q13. Step 6's sitting first stopped the release on `particles` `particle_count_refresh` p50 and p95 `regressed` in the tree's own A/A pair, whose first suite read low, which the owner accepted as A/A drift under Q14: neither direct pair read an owned metric `regressed`. The fixed-step accumulator in `Time`, step views for input edges and event queues, the `time` section, `PrevPosition` in every bundle body with the snapshot first in `fixed_update` and an interpolating `physics_sync`, `FrameTimings::fixed_steps`; example 01 on the fixed loop, drawn interpolated. The physics hashes, the eight row digests and the pixel fixtures held at the pin, with smoke, visual and the physics release tests green |
| W15c | [M42](../archive/1.0/phase5-milestone-42-additive-extracts.md) | 0.56 | Cut 2026-10-07 as `v0.56.0` (`D-138`), the squash commit and PR filled in after the merge; plan written 2026-10-07 at `cc3ded9`, critiqued in two rounds and approved with the stated defaults; run 2026-10-07: steps 1–5 done, the API approved after step 1 with no renames, and one scope stop answered by the owner (two `App` tests of the removed extract slots deleted). The `Extracts` resource with sprite, quad and text channels that games and plugins add to, the default sprite channel drawing the tilemaps at the far plane, then the sprites, `App::set_extract_*` forwarding to `replace_*`, `extract_tilemap_layers` and lit tiles; example 01's tile layers through the engine with the same per-layer output, and the template's text through its plugin. Every owned metric of every CPU row read not `regressed` on both direct pairs and the aligned pair, both A/A pairs 0 regressed and 0 improved, digests 8 of 8; `gpu` and `gpu-throughput` `extract` and the default-channel probe's `extract` read `unchanged`; the `gpu` visual and `integrated` captures are byte-identical to 0.55's, with smoke, visual and the physics release tests green |
| Phase 5 QA pass | — | — | Waits for Track C |
| W1 M2, M3 | — | — | The glyph gate passed on 2026-10-06 (`D-126`, `D-127`); M2 waits for W8a and W2 R2, M3 for M2 and W15a |
| W16a | — | — | Not started; Phase 6, after W15a |
| W6a | — | — | Not started; Phase 6, after W3b; tiers given (`D-132`) |
| W13a and W13b | — | — | Not started; Phase 6, after W3a, W16a and W15a; tiers and bodies on roots given (`D-132`) |
| W16b | — | — | Not started; Phase 6, after W16a; carries the tilemap builder (`D-132`) |
| W13c and W13d | — | — | Not started; Phase 6, after W6a |
| W11b | — | — | Not started; Phase 6, after W11a |
| W1 WL | — | — | Not started; Phase 6, after W1 M2 (`D-126`, `D-132`) |
| W1 BC | — | — | Not started; Phase 6, after W1 M3 |
| W1 M4 | — | — | Not started; Phase 6, after W1 M3, W11b and W1 BC |
| W1 M5 | — | — | Not started; Phase 6, after W1 M4 |
| W5a | — | — | Not started; Phase 6, after W1 M0b |
| W6b and W6c | — | — | Not started; Phase 6, after W6a; tiers given (`D-132`) |
| W11c | — | — | Not started; Phase 6, after W11b |
| W14b | — | — | Not started; Phase 6, after W16b, W9a, W11a and W12a |
| W12b with W1 M6 | — | — | Not started; Phase 6, after W13 and W1 M5; paired 2026-10-06 (`D-108`) |
| Phase 6 QA pass | — | — | Waits for every Must candidate of Phase 6 |
| Phase 7 (§5) | — | — | Rows added at the feature gate |

## 11. Gate records

One dated table per gate: the commit, each checklist item passed or accepted with the owner's note, the answers given, the decision IDs written, the workstreams graduated, and the owner's sign-off line. Spike verdicts are recorded under the gate they feed, with their `perf-runs/` paths.

### Definition gate, 2026-10-03

Run on branch `0.41` at `56f08ca` with the owner answering in the session, on the agenda in §9. Decisions written: `D-102` (1.0 definition, acceptance game and scope), `D-103` (stability policy; amends `D-069`), `D-104` (API snapshot tool).

| Passes when (§2) | Result | Owner's note |
| --- | --- | --- |
| 1.0 definition chosen | Passed | A and B; crates.io stays with Q2 (`D-102`) |
| Acceptance game spec with a feature map | Accepted in part | One game. Genre: a top-down, survivors-like auto-shooter, in the owner's words "a top down auto shoot as the example game". The pitch, screens, mechanics and the mechanics rows of the feature map are due at the frame-loop gate (§2), which asks the owner to follow up on the game first; [acceptance-game.md](acceptance-game.md) holds the proposals |
| Tiers confirmed against that map | Accepted in part | W4 Must (Q1), W5 Must (Q9), W11c Must (Q16), W1 BC placed before M4 (Q8). W6 and W13 are tiered at the frame-loop gate from the mechanics rows |
| `wgpu`/`winit` and stability policy agreed for their decision entries (W4a) | Passed | Hidden from the promise; semver covers the umbrella's API with the kit inside, the game-facing file formats and the CLI; MSRV rises only in a 1.x minor with the toolchain pin; deprecated items stay until 2.0 (`D-103`) |
| API snapshot tool, if amendment 17 is accepted | Passed | `cargo-public-api` (`D-104`); Step 0d lands the recipe and settles its toolchain |
| The amendments in §8 settled | Passed | All 19 accepted. Folding is owed: in the graduation commits for the sections that graduate now (amendments 4, 5, 6, 9, 11, 14, 16, 17 and 19 touch them), then into criteria.md and the other workstream files for the rest, §9 there replaced by a link here (amendment 13) |
| workflow.md agreed | Passed | Agreed as written, one release per candidate; Step 0 lands items 0a–0d as separate commits |
| W4 graduated, which seeds the break ledger | Owed | Graduation follows this record in its own commits, before W2 R0's plan writes to the ledger |

| # | Agenda item | Answer |
| --- | --- | --- |
| 1 | Q1 definition | A and B, default |
| 2 | Q1 acceptance game | One game, default |
| 3 | Q1 genre | Top-down survivors-like auto-shooter: the player only moves, weapons fire at the nearest enemies by themselves, spawner waves grow over a timed run, XP pickups lead to a level-up choice, a boss ends the run, a save slot keeps unlocks and records. First answered "totally undefined right now", with the frame-loop gate as the deadline; the genre followed in the same session |
| 4 | Q11 `wgpu` and `winit` | Hide them, default |
| 5 | Q8 UI cut | Sliders and checkboxes in as W1 BC; text fields and IME out, default |
| 6 | Q9 gamepad | In, default |
| 7 | Q16 save slots | In, default |
| 8 | Q24 kit stability | Inside the promise, default |
| 9 | Q4 multi-core scope | R1 and R2; R4 to 1.x, default. W1 M0a and M1 run in Track A |
| 10 | Amendments 1–19 | All accepted, default |
| 11 | workflow.md and Step 0 | Agreed, one release per candidate, default |
| 12 | Graduation | W4, W9, W11, W12 and W14 now, default, as follow-up commits; W2, W3 and W15 at the frame-loop gate |
| 13 | Release checklist | Agreed with RC-A9, default; RC-A5, RC-A6 and RC-A9 lose their conditions |
| 14 | Acceptance game repository | A GitHub repository of its own, cloned beside this checkout, on this repository's `v0.NN.0` tags, default |
| — | Candidate order (ordered step 1) | Three phases, §2's checklists, §3's Track A order and §6's queue agreed as written |

Owed before Step 0's first milestone plan: the graduation commits with their amendment folds, then the remaining folds. Owed by the frame-loop gate: the acceptance game's pitch, screens, mechanics and feature-map rows.

Sign-off: Signed 2026-10-03.

### Glyph gate, 2026-10-06

Run on branch `0.51` at `01f9c55` (the 0.50 tree, with the Track B and acceptance-game records uncommitted) with the owner answering in the session, in the same sitting as the frame-loop gate. Inputs: [w01](w01-ui-text-suite.md) §6, §9 and §11's Gate row, the glyph-path prototype (`perf-runs/20261005-glyph-path-prototype/README.md`, machine-local) and the acceptance game's T2 criterion ([acceptance game](acceptance-game.md#proposed-tiers)). Decisions written: `D-126` (the glyph path), `D-127` (the DPI model and `display.scale_mode`).

| Passes when (w01 §9, §11) | Result | Owner's note |
| --- | --- | --- |
| T1, T1b or T2 settled before M2's paint list | Passed | T1b (`D-126`): one `TextRenderer` per paint-list text batch on one atlas, `trim` only before a full frame forced every K frames (K 16 to start) and on `AtlasFull`, a failed batch not drawn. T2 to 1.x with its criteria |
| T1 enters only with its prototype result: paint order across interleaved batches, eviction and growth checks | Passed | Four windows draw in paint order from any prepare order (0 px against a fresh render); a retained batch survives growth (0 px); the eviction hazard reproduced under today's trim and absent under T1b in every cell |
| T1b's atlas-pressure case, prepare time and atlas bytes per K | Passed | K sweep: T1 56 µs p50 for 820 glyphs; T1b K ≥ 4 5 µs for the live label with the full frame at p95 until K ≥ 32; churn grows the atlas to 1024² from K = 16 with 3–5.7 ms growth frames |
| The DPI and hinting prototype result | Passed | Fork (a), hinting off (`D-127`): logical layout at `TextArea.scale` 2.0 equals a 24 px physical layout byte for byte; the owner read the strip and kept today's rendering |
| Capability criteria: typewriter reveal, bitmap fonts | Passed | The reveal works under T1 (transparent spans move no glyph in 170 steps); bitmap fonts wait for T2 or image commands, 1.x |
| The acceptance game's per-glyph damage numbers (T2 criterion) | Accepted | Per-label pop and scale through the world-label path (`D-126`); the per-glyph version is the T2 row's first criterion |
| `display.scale_mode`'s meaning for scene and UI (0.40 follow-up) | Passed | Removed before the freeze (`D-127`), a ledger row for W4b; integer scene scaling is 1.x and never scales the UI |

| # | Item | Answer |
| --- | --- | --- |
| 1 | Glyph path | T1b, default |
| 2 | Damage numbers | Per-label pop and scale, default |
| 3 | DPI and hinting | Fork (a), hinting off, default |
| 4 | `display.scale_mode` | Removed for 1.0, default |

Spike verdict recorded: the W1 glyph-path prototype (`perf-runs/20261005-glyph-path-prototype/`): T1 passes every check, T1b is the per-window rule, no criterion measured needs T2; findings for M2: a draw after `AtlasFull` fails wgpu validation, etagere frees a bucket only when every glyph in it is gone, glyphon evicts before it grows; costs: 3.7–5 µs per new glyph, about 5 µs per cached glyph re-uploaded in `grow`, about 70 ns per prepared cached glyph.

Owed: M2's plan carries the three findings and the K telemetry; W1 M0b adds the scale-factor resource; W4b's ledger row removes `display.scale_mode`; the T2 and R3 backlog rows are written with this record.

Sign-off: Signed 2026-10-06.

### Frame-loop gate, 2026-10-06

Run on branch `0.51` at `01f9c55` with the owner answering in the session, after the glyph gate. Inputs: this plan's §2 row and §3's Track B, the acceptance game's agreed spec and proposed tiers ([acceptance game](acceptance-game.md#proposed-tiers)), and the four Track B verdicts (`perf-runs/20261005-w2-r1-spike/`, `20261005-w15-tuple-queries/`, `20261005-stage-plugin-design/`, `20261005-w3-stage-map/`, machine-local). Decisions written: `D-128` (schedule, stages, plugins; amends `D-018`, `D-040`), `D-129` (`Time`, the fixed step, interpolation, timers, the clocks; amends `D-088`, `D-093`, `D-094`), `D-130` (tuple queries and bundles), `D-131` (multi-core scope, Q5, Q6; amends `D-102`), `D-132` (W6 and W13 tiers, Q10, Q21, the placements).

| Passes when (§2) | Result | Owner's note |
| --- | --- | --- |
| The acceptance game's pitch, screens, mechanics and feature-map rows written, and W6 and W13 tiered from them | Passed | Agreed on 2026-10-05 ahead of the gate; the tiers confirmed as proposed (`D-132`), which answers Q10 |
| R1 spike curve written | Passed | Written: pass 1 scales 1.31× at 6 workers on `gpu-throughput` and 1.89× on `particles`, the whole extract 0.91× and 1.12×; no-go as designed, R1 to 1.x (`D-131`) |
| R4 go or no-go, with a latency measurement if go | Passed | Decided at the definition gate: 1.x (`D-102`); no measurement needed. R3 leaves with T1b (`D-126`) |
| One decision set for W3's fixed step and clock, W15's stages and plugins, W1 M3's routing stages | Passed | `D-128` and `D-129`: five closed stages with `fixed_update` before `update`, `ui_route` in `pre_update` after the state dispatcher once a frame on the real clock, the accumulator with a bound of 2, interpolation on by default, the clock table |
| Threading rule | Passed | Unchanged: the two-background-thread rule stands, since no 1.0 candidate spawns a worker (`D-131`) |
| `Send`/`Sync` policy | Passed | No bound on components or resources; a parallel path bounds the types it slices at its call site (Q6, `D-131`) |
| Tuple-query spike verdict | Passed | Go; the API takes `query`/`query_mut` at W15b, two ledger rows; one budgeted step for the lone-mutable-column layout (`D-130`) |

| # | Item | Answer |
| --- | --- | --- |
| 1 | W6 tiers | Confirmed as proposed; Q10 answered, default |
| 2 | W13 tiers | Confirmed as proposed, default |
| 3 | The tilemap builder | W16b, default |
| 4 | Q21 bodies on roots | Yes, roots only; the game places the blades each step, default |
| 5 | Q7 interpolation | On by default, default |
| 6 | Step bound | 2, default |
| 7 | Q13 clocks | The spike's table, default |
| 8 | Stage order | `fixed_update` before `update`, gameplay that drives bodies opts in, default |
| 9 | W2 R1 | Dropped for 1.0, default |
| 10 | Q5, Q6, threads | No mechanism in 1.0 (rayon if ever), no bounds, the two-thread rule stands, default |
| 11 | Query names | `query`/`query_mut` at W15b, default (the spike proposed `fetch`) |
| 12 | Plugin details | `WindowSize` to core; if-present constraints public, the engine's cross-plugin couplings use them, a plugin's own chains and a game's constraints required; `App` keeps inserting the engine's resources; the bench's explicit set and re-owned `particles` metric; W15b's budgeted step, default |
| 13 | World labels | A step of its own after M2, Phase 6, level B, default |
| 14 | Graduation | W2, W3, W15, default |
| 15 | Phase 6 rows | The order and pairs of §4, default |

Spike verdicts recorded (each README holds the trees, captures and compares):

- **W2 R1** (`perf-runs/20261005-w2-r1-spike/`): no-go as designed. Bit-identical at every worker count; `gpu-throughput` `extract` p50 12.07 → 12.51 / 12.90 / 13.24 / 13.22 ms at 1 / 2 / 4 / 6 workers, `regressed` at every count, the row bound by memory bandwidth shared with the integrated GPU (IPC 1.64 → 0.49 at 6 workers; headless the pass scales 2.7× until the GPU row runs beside it) and the merge's record concatenation at 1.37–4.02 ms; `particles` `extract` 1.40 → 1.25 ms with its serial sort and pass 2 at 61% of the extract and its owned rows `regressed` by 0.03–0.22 ms; one worker costs 0.35–0.44 ms of pass 1; a scoped fork costs 21–71 µs a frame against 4 µs for a rayon pool.
- **W15 tuple queries** (`perf-runs/20261005-w15-tuple-queries/`): go. Tuple data and filters over `get_disjoint_mut`, no `unsafe`, twelve tests row for row; `ecs` `update` p50 10.39 → 10.43 ms (`unchanged`) with digests matching in every pair, `churn` not regressed (`flush` `improved` by placement, `unchanged` aligned); `bounds_wrap` 0.25 → 0.30 ms (`regressed`, +0.22 ns a row from LLVM's layout of a lone `IterMut`), the one cost; optional and filtered shapes 6–20% faster per row in criterion; the slice forms are R1's column slices.
- **W15 stages and plugins** (`perf-runs/20261005-stage-plugin-design/`): go, five findings. Every workspace test, smoke and the visual fixture pass; 0 regressed on every direct pair with digests 8 of 8, `ecs` `update` 10.27 → 10.27 ms, `physics_step` 6.10 → 6.10, `particles` `unattributed` relocated into named systems with `total` unchanged; findings: `WindowSize` in the umbrella, one frame of input latency for gameplay left in `update` (1 / 2 / 1 frames), cross-plugin constraints need an if-present form, the `particles` row's owned metric, resources stay the app's.
- **W3 stage map** (`perf-runs/20261005-w3-stage-map/`): go, thirteen findings. Every test, smoke, the physics release tests (hash unchanged) and the visual fixture pass; the clock stage and the empty fixed loop at no cost (digests 8 of 8); `physics_step` `unchanged` through `fixed_update` with the snapshot (6.15 → 6.15 ms aligned); `integrated` `regressed` 5.8% only with its AI left in `update` after the step, `unchanged` with the AI opted in; probes: per-frame edges miss 56% of presses at 144 Hz and double them at 30 Hz where the fixed view sees each once, 0/10 px frames against a steady 4.17 px with interpolation one step behind, a 0.1 s stall drops 67 ms at a bound of 2, the sync and snapshot are microseconds and `sync_position_to_transform` is five times slower than the optional-column query.

Owed before Track C: graduation sessions for W2, W3 and W15, one each, folding this record's answers into their files; then W15a's plan. Written with this record: Phase 6's rows (§10) and cards (§4), the W1 WL step, the backlog rows for R1, T2 with R3, audio pause and integer scene scaling, the ledger rows for `query`/`query_mut` and `display.scale_mode`, and the catalog's stops and questions.

Sign-off: Signed 2026-10-06.

## Revisions

- 2026-10-03 at `afbc330`: first draft, from criteria.md as revised that day and the UI draft's milestone ladder ([w01](w01-ui-text-suite.md) §11).
- 2026-10-03 on `941b63e`: [workflow.md](workflow.md) added beside this plan; inputs from 0.40, Step 0's proposed items, Track A's order and constraints, W1 M1 and W2 R2 moved into Track A, the worker-thread capture rules in Track B, QA passes closing Phases 5 and 6, cards for Phase 5, W1 BC, versioned prereleases for release candidates, the queue (§6), the definition gate agenda (§9), the register (§10), gate records (§11) and amendments 14–19; amendment 12's two moves applied in criteria.md.
- 2026-10-03 on `75a7360`: local commits replace the patch hand-off (`D-097`, `D-098`) in the context digest and §6.
- 2026-10-03 on `4ee92aa`: the 0.40 QA plan finished; the context digest and §3 say so, and §3 drops step 9's row, which shipped in the close-out.
- 2026-10-03, against the 0.40 tree (`9cd5709`, `v0.40.0` merged; HEAD was `1d9bf8c`, a renderer-only commit): reviewed. Q12 moves from the freeze gate to the feature gate, since C1 opens Phase 7 (§2, §7); W12a no longer waits for Q2 (§7, amendment 14's default); W1 M2 also waits for W2 R2 (§3, §6); W12a's row says the skeleton keeps hand-registered systems until W15a and starts W9b's guide (§3); §3's follow-up table gains the capture-provenance and `cargo shear` rows and the test file's current length; §9 gains the release count under item 11, the scroll-container note under item 5 and item 14, the acceptance game's repository; the register and digest record the release.
- 2026-10-03 at `56f08ca`: the definition gate ran (§11), with `D-102`–`D-104`; status in progress; §2's frame-loop row gains the acceptance game's pitch and mechanics; §9 points at the record; the register's Step 0 and W1 M0a rows follow.
- 2026-10-03 on `db1177c`: sessions leave work uncommitted until the release commit (`D-105`) in the context digest, §6 and the register.
- 2026-10-03 on `db1177c`: the owner signed the definition gate record (§11). W4, W9, W11, W12 and W14 graduated into their workstream files with amendments 4, 5, 6, 9, 11, 14, 16, 17 and 19 folded in; §3's W1 M0b row and W11a note point at them, and the register's Step 0 row waits for the remaining folds.
- 2026-10-03 on `db1177c`: the remaining amendments (1–3, 7, 8, 10, 13, 15–18) and the gate's answers folded into criteria.md, w01 and the skeletons of W2, W3, W5–W8, W10, W13 and W15; criteria §9 now links here, so the context digest and §1 speak of it in the past; the register's Step 0 row waits for the owner's review of the folds.
- 2026-10-04 on `db1177c`, uncommitted: Step 0 landed (0a–0d, `D-106`, `D-107`); §3's Step 0 paragraph says so and the register's Step 0 and W14a rows follow.
- 2026-10-04 on `db1177c`, uncommitted: the 0.42 release session cut `v0.42.0`; the register's Step 0 row says so.
- 2026-10-04 on `db1177c`, uncommitted: a milestone's release becomes its plan's last step ([workflow](workflow.md) §1), so §10's header names the release step; two small adjacent candidates may share one plan and one release (`D-108`, revising the definition gate's one release per candidate), which §6 states with Phase 5's pairs: W9a with W12a and W1 M0b with W2 R2, one register row each in §10.
- 2026-10-04 on `28abcce`, uncommitted: 0.42 released (`v0.42.0`, squash-merged as `28abcce`); the register's Step 0 row and §3's Step 0 note say so.
- 2026-10-04 on `28abcce`, uncommitted: W14a's milestone plan written as [M32](../archive/1.0/phase5-milestone-32-headless-harness.md), critiqued and approved with its defaults; the register's W14a row points at it.
- 2026-10-04 on `28abcce`, uncommitted: M32's run did steps 2–6 and stopped before the release under its §7, since step 1's A/A check failed; the register's W14a row says so.
- 2026-10-04 on `28abcce`, uncommitted: the 0.43 release session closed M32 (`status: done`, archived) and cut `v0.43.0`; the register's W14a row says so.
- 2026-10-04 on `6a61cc2`, uncommitted: 0.45 released (squash-merged as `6a61cc2`); W1 M0a's milestone plan written as [M34](../archive/1.0/phase5-milestone-34-text-engine-split.md) and critiqued in two rounds, then approved with its defaults; the register's W2 R0 and W1 M0a rows follow.
- 2026-10-04 on `6a61cc2`, uncommitted: M34's run did steps 1–10, stopping twice for the owner (Q11, Q12), closed M34 (`status: done`, archived) and cut `v0.46.0`; the register's W1 M0a row says so.
- 2026-10-04 on `20bd160`, uncommitted: 0.46 released (`v0.46.0`, squash-merged as `20bd160`); the register's W1 M0a row says so. 0.47, outside the register, carries the roadmap page's stages and session recommendations (`D-118`).
- 2026-10-05 on `71b00be`, uncommitted: W11a with W7a's milestone plan written as [M35](../archive/1.0/phase5-milestone-35-logs-crash-reports.md), critiqued in two rounds and approved with its defaults; the register's row points at it.
- 2026-10-05 on `71b00be`, uncommitted: M35's run did steps 1–9, closed M35 (`status: done`, archived) and cut `v0.48.0`; the register's W11a with W7a row says so.
- 2026-10-05 on `01f9c55`, uncommitted: 0.50 released (`v0.50.0`, squash-merged as `01f9c55`) and branch `0.51` opened; the register's W1 M1 row and the README's Now lines say so.
- 2026-10-05 on `01f9c55`, uncommitted: the W2 R1 spike ran in a scratch copy (workflow §2, Track B); the register's Track B row carries its verdict and the evidence path, and [w02](w02-multi-core-rendering.md) its placement note.
- 2026-10-05 on `01f9c55`, uncommitted: the W15 tuple-query spike ran in a scratch copy (workflow §2, Track B); the register's Track B row carries its verdict and the evidence path, [w15](w15-authoring-api.md) its placement note, and the README's Now lines name Track B's remaining items.
- 2026-10-05 on `01f9c55`, uncommitted: the W15 stage-and-plugin design spike ran in a scratch copy (workflow §2, Track B); the register's Track B row carries its verdict, findings and the evidence path, [w15](w15-authoring-api.md) its placement note, and the README's Now lines name Track B's remaining items.
- 2026-10-05 on `01f9c55`, uncommitted: the W3 design spike ran in a scratch copy on the stage-and-plugin patch (workflow §2, Track B); the register's Track B row carries its verdict, findings and the evidence path, [w03](w03-frame-loop.md) its placement note, and the README's Now lines name Track B's last item.
- 2026-10-05 on `01f9c55`, uncommitted: the W1 glyph-path prototype ran in a scratch copy (workflow §2) for the glyph gate; the register's Track B row carries its verdict, findings and the evidence path, [w01](w01-ui-text-suite.md) §6 and §9 their prototype notes, and the README's Now lines name it.
- 2026-10-05 on `01f9c55`, uncommitted: the acceptance game's pitch, screens and mechanics agreed with the owner ahead of the frame-loop gate. [acceptance-game.md](acceptance-game.md) holds them with the feature map's mechanics rows, the proposed W6 and W13 tiers and five items for the gates; [w06](w06-gameplay-systems.md) and [w13](w13-kit.md) point at the proposal, [w01](w01-ui-text-suite.md) §9 records the damage numbers as a T2 criterion, and §2's frame-loop row, the README's Now lines and the catalog's gate stop and Q1 say so.
- 2026-10-06 on `01f9c55`, uncommitted: the glyph gate and the frame-loop gate ran in one owner session and were signed (§11; `D-126`–`D-132`), every item on its default. §2's rows and the glyph-gate note say so; Track C drops W2 R1 (§3); §4 gains W1 WL and the Phase 6 cards and loses W2 R3 and R4; §6's queue and pairs follow; the register's Track B row is closed, its W2 R1 row removed and Phase 6's rows added (§10).
- 2026-10-06 on `01f9c55`, uncommitted: W15 graduated ([workflow](workflow.md) §2): criteria §8.6 moved into [w15](w15-authoring-api.md) with its candidates' done-when checks, amendments 7, 8, 16 and 18 and the gate's decisions folded in; the register's W15a and W15c rows and the catalog's W15 sources follow.
- 2026-10-06 on `01f9c55`, uncommitted: W2 graduated ([workflow](workflow.md) §2): criteria §5 moved into [w02](w02-multi-core-rendering.md) with R0's and R2's done-when checks, amendment 7 and the gates' decisions folded in; Track A's R2 row (its source, and per-worker encoders under `D-131`), §6's Q4 note and the register's W1 M0b with W2 R2 row follow, with the backlog's, w04's and w15's citations of criteria §5's subsections and the catalog's W2 sources.
- 2026-10-06 on `01f9c55`, uncommitted: W3 graduated ([workflow](workflow.md) §2): criteria §6 moved into [w03](w03-frame-loop.md) with W3a's and W3b's done-when checks, amendments 7, 16 and 18 and the gate's decisions folded in; Track C's W3b row and W3b's card drop the seam change (`D-129`), and the register's W3a and W3b rows follow, with w04's citations of criteria §6 and the catalog's W3 sources.
- 2026-10-06 on `764ef4e`, uncommitted: 0.51 released (`v0.51.0`, squash-merged as `764ef4e`, PR #51) and branch `0.52` opened; W15a's milestone plan written as [M38](../archive/1.0/phase5-milestone-38-schedule-stages-plugins.md) and critiqued; the register's W15a row points at it and the README's Now lines say so.
- 2026-10-06 on `764ef4e`, uncommitted: M38 run and cut as 0.52 (`D-133`): the plan is archived, the register's W15a row carries the release and its numbers, and the README's Now lines say 0.52 is cut and W3a is next; the squash commit and PR number are added once the pull request merges.
- 2026-10-06 on `36b766d`, uncommitted: 0.52 released (`v0.52.0`, squash-merged as `36b766d`, PR #53) and branch `0.53` opened; W3a's milestone plan written as [M39](../archive/1.0/phase5-milestone-39-game-clock-timers.md), critiqued in two rounds and approved with its defaults; the register's W15a and W3a rows and the README's Now lines say so.
- 2026-10-06 on `36b766d`, uncommitted: M39 run and cut as 0.53 (`D-134`), after the owner accepted step 4's `ecs` `stats_decay` reading under the plan's Q7: the plan is archived, the register's W3a row carries the release and its numbers, and the README's Now lines say 0.53 is cut and W15b is next; the squash commit and PR number are added once the pull request merges.
- 2026-10-06 on `2103040`, uncommitted: 0.53 released (`v0.53.0`, squash-merged as `2103040`, PR #54) and branch `0.54` opened; W15b's milestone plan written as [M40](../archive/1.0/phase5-milestone-40-tuple-queries-bundles.md), critiqued in two rounds and approved with its defaults; the register's W3a and W15b rows and the README's Now lines say so.
- 2026-10-06 on `2103040`, uncommitted: M40 run and cut as 0.54 (`D-135`): steps 1–3, 5 and 6 done, step 4 skipped at its control check, step 5 passed at its third retry with the default extract on the slice form; the plan is archived, the register's W15b row carries the release and its numbers, and the README's Now lines say 0.54 is cut and W3b is next; the squash commit and PR number are added once the pull request merges.
- 2026-10-06 on `7bc0345`, uncommitted: 0.54 released (`v0.54.0`, squash-merged as `7bc0345`, PR #55) and branch `0.55` opened; W3b's milestone plan written as [M41](../archive/1.0/phase5-milestone-41-fixed-step-interpolation.md) and critiqued in two rounds; the register's W15b and W3b rows and the README's Now lines say so.
- 2026-10-07 on `7bc0345`, uncommitted: M41 run and cut as 0.55 (`D-137`), after the owner accepted step 2's criterion cost under the plan's Q13 and step 6's `particles` `particle_count_refresh` A/A reading under Q14: the plan is archived, the register's W3b row carries the release and its numbers, and the README's Now lines say 0.55 is cut and W15c is next; the squash commit and PR number are added once the pull request merges.
- 2026-10-07 on `cc3ded9`, uncommitted: 0.55 released (`v0.55.0`, squash-merged as `cc3ded9`, PR #56) and branch `0.56` opened; W15c's milestone plan written as [M42](../archive/1.0/phase5-milestone-42-additive-extracts.md), critiqued in two rounds and approved with its defaults; the register's W3b and W15c rows and the README's Now lines say so.
- 2026-10-07 on `cc3ded9`, uncommitted: M42 run and cut as 0.56 (`D-138`), the API approved after step 1 with no renames and the owner's answer to the scope stop on two `App` tests: the plan is archived, the register's W15c row carries the release and its numbers, and the README's Now lines say 0.56 is cut and the Phase 5 QA pass is next; the squash commit and PR number are added once the pull request merges.

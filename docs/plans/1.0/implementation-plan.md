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
| Frame loop | W15a, W3a, W3b, W2 R1; W1 M0a if Q4 is still open | The acceptance game's pitch, screens, mechanics and feature-map rows written, and W6 and W13 tiered from them (definition gate record, §11); R1 spike curve written; R4 go or no-go, with a latency measurement if go; one decision set for W3's fixed step and clock, W15's stages and plugins, W1 M3's routing stages and, if R4 goes ahead, its hand-off point; threading rule; `Send`/`Sync` policy; tuple-query spike verdict | First, Q1's remainder: the acceptance game's pitch and mechanics, as the owner asked at the definition gate; then Q5, Q6, Q7, Q13 |
| Feature | Phase 7 | The acceptance game plays start to finish from a `tungsten package` archive, on the public API and the kit alone; every Must candidate landed or re-tiered with the owner; the kit's admission rule applied; the Phase 6 QA pass closed; the break ledger holds only what Phase 7 lands | Q3, Q10; Q12, since C1 opens Phase 7 (§7) |
| Freeze | C2 | C1 baselines and budgets dated; break ledger empty; arity-named queries removed; `missing_docs` clean; API snapshot taken | Q2 |
| RC | C3 | Every budget passes or has an accepted exception; release candidate tagged as a versioned prerelease, `v1.0.0-rc.N` with its own changelog section, which publishes without a pull request ([releases](../../releases.md#rehearsals-and-versioned-prereleases)) | — |
| 1.0 | The tag | Every row of the [release checklist](release-checklist.md) passed, or accepted by the owner, in a dated QA record | — |

The glyph gate (W1's T1, T1b or T2, [w01](w01-ui-text-suite.md) §9) is W1's own and sits inside Phase 5; it can share an owner session with the frame-loop gate.

## 3. Phase 5: Foundations

**Before Phase 5.** 0.40 released on 2026-10-03. Its QA plan left follow-ups in [known issues](../../known-issues.md); each has a 1.0 home:

| 0.40 follow-up | Goes to |
| --- | --- |
| `logging.level` is parsed and never applied | W11a: the engine logger applies it |
| `display.scale_mode` is parsed and never applied | W1's DPI decision, at the glyph gate |
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
| W2 R2 | Late-acquire split: scene and post passes recorded before the acquire. It needs nothing from the frame-loop gate (criteria §5.3), and W1 M2's overlay then builds on the split final pass. Per-worker encoders stay an option after the threading rule | R4, if it comes back; W1 M2 builds on it |

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
| 4 | W3b | Fixed step and interpolation, the seam change |
| 5 | W2 R1 | Parallel default and tilemap extracts on W15b's column slices |
| 6 | W15c | Additive extracts, with tilemaps in the default |
| 7 | Phase 5 QA pass | An audit of what Phase 5 changed, in the 0.40 shape, before Phase 6 builds on it (amendment 18) |

W1's ladder interleaves with both tracks: M0a and M1 in Track A, the glyph gate once M1 has landed, M2 once the gate, W8a and W2 R2 have (its overlay extends R2's split final pass), and M3 once M2 and W15a have.

**Cards.** What each Phase 5 candidate costs to run, for ordering and for batching owner-present and unattended work. "Every CPU row" means the suite, or `--only` every row the code runs in ([done-when rules](../../perf/profiling-workflow.md#writing-done-when-checks)). Levels are workflow §4's. Cards for Phases 6 and 7 are written at the frame-loop gate, when Phase 5's shape is known.

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
| W3b | The accumulator in `app.rs`; the physics stage; interpolation in the extract (the `D-018` seam) | From the gate; amends `D-088` and `D-094` | Step and interpolation settings | `just physics-release` hashes; every row, digests unchanged at 1/60 s; smoke; visual | — | B |
| W2 R1 | A column-slice query in core; the default and tilemap extracts; per-worker `ExtractBuffers` | Threading rule, which edits `AGENTS.md`; worker mechanism | Column-slice query | The headless batch test at 1–6 workers; `gpu-throughput` and `particles` `extract` `improved`; `gpu` not regressed; visual at 0 and the default count | — | B |
| W15c | Extract registration; tilemaps in the default extract; example 01's extract | Additive extracts | Extract registration | Extract rows; smoke; visual | API review | C |
| Phase 5 QA pass | The tree | As its findings need | As its findings need | As 0.40's | Plan approval | A |
| W1 glyph gate, M2, M3 | [w01](w01-ui-text-suite.md) §6, §9, §11 | The glyph path; UI input routing | M3: routed input | As w01 §11 sketches | The glyph decision; API review | S, then C |

- **W11a.** Perf lines are `log::debug!` records (`tungsten/app.rs:1188`) that `scripts/bench.py` reads from the example's output under `RUST_LOG=tungsten::app=debug,bench=debug`. Criteria §8.2 kept stderr in debug builds only, so the release builds that the runner measures would have lost them; [w11](w11-shipping-basics.md) now keeps stderr while `RUST_LOG` is set, or has the runner read the log file (amendment 19).
- **W2 R0.** Its ID API is what every game writes to name a sprite, hence level C. It lands before the R1 spike so that the spike's serial baseline has no string compares in it.

## 4. Phase 6: Features

The acceptance game's repository starts when W15a lands, late in Phase 5 ([acceptance game](acceptance-game.md), step 4), and grows through this phase. Each gap it hits becomes a candidate here or a row in the [backlog](backlog-1.x.md); workflow §7 says how gaps come back from its repository.

| Candidate | Contents | Needs |
| --- | --- | --- |
| W16a | Component registry; `components` in scene entries | W15a |
| W6a | Kinematic bodies, sensors, shape and ray queries | W3b |
| W13a | Kit basics | W3a, W16a |
| W13b | Transform hierarchy; the physics sync becomes an engine stage | W15a |
| W16b | Prefab assets, `spawn_prefab`, Tiled object classes; then the kit spawner | W16a |
| W13c, W13d | Character controllers; gameplay helpers | W6a |
| W11b | Settings: shared atomic write, user layer, corrupt-file fallback, the settings the engine persists | W11a |
| W1 BC | Broader controls the settings screen needs, sliders and checkboxes ([w01](w01-ui-text-suite.md) §8), if Q8 says so (amendment 15) | W1 M3 |
| W1 M4 | Gameplay screen fixture with the settings screen | W1 M3, W11b; W1 BC if Q8 says so |
| W1 M5 | Engine debug views; the log console on W11a's logger | W1 M4 |
| W5a | Gamepad, if Q9 says so | W1 M0b |
| W6b, W6c | Layers and masks; audio voices, pause, fades, buses | The acceptance game asks |
| W11c | Save slots, if Q16 says so | W11b |
| W14b | CLI `check`, `package`, `new` | W16b, W9a, W11a, W12a |
| W12b with W1 M6 | Examples 01, 03 and 04: template layout, plugins, kit and UI, in one pass per example | W13, W1 M5 |
| W2 R4 | Render thread, only if approved | W2 R2 |
| W2 R3 | Text preparation off the main thread, if the glyph gate allows | W1 glyph gate |
| Phase 6 QA pass | An audit in the 0.40 shape that also takes W8b: the shader ID allocator, queued transitions, the remaining P3s (amendment 18) | Every Must candidate above |

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

- **Answer Q4 at the definition gate.** Criteria §4 makes the R4 answer an input to where the text engine lives, so while Q4 is open, M0a waits for the frame-loop gate and its spikes. Proposed answer: R4 moves to 1.x and comes back only if C1 shows a game-frame row over budget that R4 would fix. It pays only where a CPU-bound frame has GPU slack (at most about 25% on `integrated`, criteria §5.2). It adds a frame of latency, turns every main-thread renderer call into a command or a sync point, and changes what `total` means.
- **Start M0b in Track A.** It is independent of M0a (w01 §11) and of the frame-loop gate.
- **Run M1 in Track A.** The core UI model needs M0a's text API and W14a's harness, and nothing from the frame-loop gate, so the glyph gate can come as early as the frame-loop gate does.

After that, M2 waits on W8a, W2 R2 and the glyph gate, M3 on W15a, and M4 on W11b and, if Q8 says so, W1 BC. Each of those starts in an earlier track, so none should hold the ladder up.

**The queue.** Everything in the tree goes into the next release commit (workflow §6), so milestones run one after another. The critical path then orders risk rather than setting a finish date: the glyph gate and the frame-loop gate are where a wrong guess costs most, and the queue reaches both early. Proposed Phase 5 queue, with owner-present work and capture work kept apart:

1. Step 0, then Track A in the table's order: W14a and R0 first because every later test and the R1 spike stand on them, then the head of W1's ladder, then the shipping basics and the template.
2. Track B in unattended sittings as each becomes possible: the R1 spike once R0 has landed; the tuple-query spike and the frame-loop design at any time.
3. While the spikes wait for their gate: W8a, W1 M0b, W2 R2, and the glyph-path prototype once M1 has landed.
4. The glyph gate and the frame-loop gate, in one owner session if both are ready.
5. Track C, then the Phase 5 QA pass; W1 M2 and M3 as their inputs land.

A second clone running W1's ladder beside the rest is the one place where parallel sessions shorten the road. It pays only if the owner can review two streams of changes.

**Pairs.** Two small candidates may share one milestone plan and one release when both are level A or B, touch different files and sit next to each other in the queue (`D-108`, which revises the definition gate's one release per candidate, §11 agenda item 11). The register (§10) holds each pair as one row, and a pair's plan splits in two if a shared file or a level C turns up. Phase 5's pairs, settled by the owner on 2026-10-04:

- **W11a with W7a**, as the cards already had it.
- **W9a with W12a.** W9a's steps run first, so W12a's release carries the licence notices it needs. W12a is larger than the rule's "small" (an outside-copy check, smoke and visual); the owner paired them anyway.
- **W1 M0b with W2 R2.** M0b changes the core input types, `input_bridge.rs` and `app.rs`; R2 changes `renderer.rs`. If winit 0.31 has left prerelease when the plan is written, M0b upgrades it first and that reaches `renderer.rs`, so the pair splits. R2's perf baseline is taken after M0b's steps.

W8a and W1 M0b do not pair: both change `crates/tungsten/src/app.rs` (`App::run` and `window_event`). Phase 6 and 7 pairs are proposed when the frame-loop and feature gates write those cards.

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
| W11a with W7a | [M35](../archive/1.0/phase5-milestone-35-logs-crash-reports.md) | 0.48 | Cut 2026-10-05 as `v0.48.0` (`D-119`–`D-121`), awaiting the owner's rehearsal checks (both probe steps, the crash-report check, CI's Windows job), tag and merge; run 2026-10-05: steps 1–9 done, step 7's local Windows cross-check not run (no MSVC tools on Linux), and step 5's split also drops `.debug_gdb_scripts`, which its done-when needed |
| W9a with W12a | — | — | Not started; paired 2026-10-04 (`D-108`), W9a's steps first |
| W1 M1 | — | — | Not started |
| Track B spikes | — | — | Not started |
| W8a | — | — | Not started |
| W1 M0b with W2 R2 | — | — | Not started; paired 2026-10-04 (`D-108`), split if winit 0.31 has left prerelease when the plan is written |
| W15a | — | — | Waits for the frame-loop gate |
| W3a | — | — | Waits for the frame-loop gate |
| W15b | — | — | Waits for the frame-loop gate |
| W3b | — | — | Waits for the frame-loop gate |
| W2 R1 | — | — | Waits for the frame-loop gate |
| W15c | — | — | Waits for the frame-loop gate |
| Phase 5 QA pass | — | — | Waits for Track C |
| W1 M2, M3 | — | — | Wait for the glyph gate |
| Phase 6 (§4) | — | — | Rows added at the frame-loop gate |
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

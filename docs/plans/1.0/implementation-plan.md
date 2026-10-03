# Tungsten 1.0 implementation plan — draft

- **status:** draft
- **goal:** Turn [criteria.md](criteria.md) into an ordered, gated path to a 1.0 tag: three phases, six gates, milestone candidates in dependency order, and the point by which each owner question needs its answer.
- **non-goals:** Dates, time estimates, version numbers before 1.0, decision entries, implementation. Scoping, which criteria.md and the workstream files own. Reopening anything criteria.md records as answered by the owner.
- **files to touch:** This folder only while it is a draft. Each candidate becomes its own `phaseN-milestone-NN-slug.md` plan here when it is next to start.
- **ordered steps:** (1) The owner accepts or rejects the amendments in §8, and the accepted ones go into criteria.md. (2) Pass the definition gate. (3) Phase 5 Tracks A and B, the frame-loop gate, then Track C. (4) Phase 6 to the feature gate. (5) Phase 7: C1, the freeze gate, C2, the RC gate, C3, the 1.0 gate.
- **done-when:** The owner has agreed the phase split, each gate's checklist and the candidate order; every Must workstream has at least one placed candidate; criteria.md §9 links here instead of holding an order of its own.

Drafted 2026-10-03 at `afbc330` from criteria.md as revised that day and the UI draft's milestone ladder ([w01](w01-ui-text-suite.md) §11).

## Context digest

- criteria.md says what 1.0 means and scopes W1–W16 with proposed tiers; its §9 sketches an order. This file refines that order and adds gates. Once agreed, it replaces §9.
- Milestones continue at M32. A candidate gets its number when it is next to start, so numbers follow the order in which work lands.
- Candidate IDs: `W11a`, `W11b` and so on per workstream. W1 keeps its ladder names (M0a … M6), W2 its options (R0 … R5) and W10 its phases (C1 … C3).
- Gates have names, not numbers: G1–G3 already name physics and lighting proposals in [benchmarks.md](../../perf/benchmarks.md#open-proposals).
- Git is human-only: each milestone ends in a [patch hand-off](../../../.claude/skills/tungsten-patch-handoff/SKILL.md). Milestones that touch performance follow the [profiling workflow](../../perf/profiling-workflow.md).
- The longest chain is W1's ladder (§6); everything else can run beside it.

## 1. Shape

Three phases in place of the five in criteria §9. Phases 8 and 9 there hold one workstream each and are gates rather than bodies of work, and fewer phases means fewer milestone renames when the order shifts. Keep five if each closing phase should start its own milestone numbering.

| Phase | Theme | Ends at |
| --- | --- | --- |
| 5 Foundations | What later work stands on: logs and crash files, the headless harness, self-containment, serial render wins, and the shape of game code (stages, plugins, queries, bundles, the clock) | W15a, W15b, W3a and W3b landed |
| 6 Features | What games use: UI screens, input, physics and audio gaps, the kit, prefabs, settings and saves, the CLI, example migration, the acceptance game | Feature gate |
| 7 Release | C1 benchmarks, the freeze, C2 performance, release candidates, C3 QA | 1.0 gate |

## 2. Gates

Question numbers are criteria §10's; §7 lists the rest by the candidate that needs them.

| Gate | Before | Passes when | Owner questions |
| --- | --- | --- | --- |
| Definition | Phase 5 | 1.0 definition chosen; [acceptance game](acceptance-game.md) spec with a feature map; tiers confirmed against that map; `wgpu`/`winit` and stability policy agreed for their decision entries (W4a); the amendments in §8 settled | Q1, Q11, Q24; Q8, Q9 and Q16, which §1 and C3 already assume; Q4, proposed here (§6) |
| Frame loop | W3a, W3b, W15a, W2 R1; W1 M0a if Q4 is still open | R1 spike curve written; R4 go or no-go, with a latency measurement if go; one decision set for W3's fixed step and clock, W15's stages and plugins, W1 M3's routing stages and, if R4 goes ahead, its hand-off point; threading rule; `Send`/`Sync` policy; tuple-query spike verdict | Q4 if still open, Q5, Q6, Q7, Q13 |
| Feature | Phase 7 | The acceptance game plays start to finish from a `tungsten package` archive, on the public API and the kit alone; every Must candidate landed or re-tiered with the owner; the kit's admission rule applied; the break ledger holds only what Phase 7 lands | Q3, Q10 |
| Freeze | C2 | C1 baselines and budgets dated; break ledger empty; arity-named queries removed; `missing_docs` clean; API snapshot taken | Q2, Q12 |
| RC | C3 | Every budget passes or has an accepted exception; release candidate tagged | — |
| 1.0 | The tag | Every row of the [release checklist](release-checklist.md) passed, or accepted by the owner, in a dated QA record | — |

## 3. Phase 5: Foundations

**Step 0, planning tooling.** `check_plans` in `scripts/check-repo.py` also reads `docs/plans/*/*.md`, skipping the archive, with a case in `scripts/test-check-repo.py`. It lands after the 0.40 QA plan, which edits both files. No milestone plan is written in this folder before it.

**Track A**, independent of the frame-loop gate, in any order after the definition gate:

| Candidate | Contents | Unblocks |
| --- | --- | --- |
| W8a | Capture-completion contract (a presented, skipped or failed render result); the engine-finding bugs: burst latch, tilemaps at `z_norm` 0 under `gpu_depth`, the lit-sprite material log, a once-per-ID warning for an unknown sprite | W1 M2 |
| W1 M0b | Input and display groundwork from the UI ladder, which takes over W5's focus loss, modifiers and scale factor | W1 M3, W5a |
| W1 M0a | Text engine split, here only if the definition gate answers Q4 (§6); otherwise after the frame-loop gate | W1 M1 |
| W11a | User folder, log file, panic hook, release symbols with the CI symbolization probe, no console on Windows; W7a's CPU-only Windows job beside it | W12a, W1 M5, W14b `package` |
| W9a | Licence notices in archives, the embedded font's OFL notice included | W12a in a release, W14b `package` |
| W12a | Self-containment, template skeleton, outside-copy check | W14b `new`, the acceptance game |
| W14a | Headless harness that drives `App`'s own frame function | Every later test; W1 M1's `UiHarness` |
| W2 R0 | Interned asset IDs (a break, into the ledger), extract culling | The R1 spike's baseline |

**Track B**, the inputs to the frame-loop gate. These are spikes and designs, with evidence in `perf-runs/` and nothing merged that the gate could reverse:

- the W2 R1 spike, plus an R4 prototype only if Q4 leaves R4 open;
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
| 6 | W2 R2 | Late-acquire split |
| 7 | W15c | Additive extracts, with tilemaps in the default |

W1's ladder runs beside Track C: M1, the glyph gate, M2 once W8a has landed, and M3 once W15a has.

## 4. Phase 6: Features

The acceptance game starts first, from the template in its own repository, and grows through the phase. Each gap it hits becomes a candidate here or a row in the [backlog](backlog-1.x.md).

| Candidate | Contents | Needs |
| --- | --- | --- |
| W16a | Component registry; `components` in scene entries | W15a |
| W6a | Kinematic bodies, sensors, shape and ray queries | W3b |
| W13a | Kit basics | W3a, W16a |
| W13b | Transform hierarchy; the physics sync becomes an engine stage | W15a |
| W16b | Prefab assets, `spawn_prefab`, Tiled object classes; then the kit spawner | W16a |
| W13c, W13d | Character controllers; gameplay helpers | W6a |
| W11b | Settings: shared atomic write, user layer, corrupt-file fallback, the settings the engine persists | W11a |
| W1 M4 | Gameplay screen fixture with the settings screen | W1 M3, W11b |
| W1 M5 | Engine debug views; the log console on W11a's logger | W1 M4 |
| W5a | Gamepad, if Q9 says so | W1 M0b |
| W6b, W6c | Layers and masks; audio voices, pause, fades, buses | The acceptance game asks |
| W11c | Save slots, if Q16 says so | W11b |
| W14b | CLI `check`, `package`, `new` | W16b, W9a, W11a, W12a |
| W12b with W1 M6 | Examples 01, 03 and 04: template layout, plugins, kit and UI, in one pass per example | W13, W1 M5 |
| W8b | Shader ID allocator, queued transitions, remaining P3s | — |
| W2 R4 | Render thread, only if approved | W2 R2 |
| W2 R3 | Text preparation off the main thread, if the glyph gate allows | W1 glyph gate |

Then the feature gate.

## 5. Phase 7: Release

| Order | Candidate | Contents |
| --- | --- | --- |
| 1 | C1 | Dated 1.0 baseline; new rows for UI, text, startup, a template or acceptance-game frame, hierarchy, kit systems and tuple queries; a budget per row. Fixes that need an API change land now |
| 2 | W4b | Break ledger empty, arity-named queries removed, `#[non_exhaustive]` sweep, `missing_docs`, rustdoc examples, API snapshot (its tool is a `D-015` question). Then the freeze gate |
| 3 | C2 | Rows over budget, largest gap first; internal changes only |
| 4 | W7b, W9b, W9c | Support tiers in the README and a macOS build if Q3 says so; the getting-started guide's final pass; crates.io if Q2 says so |
| 5 | C3 | The release checklist on a candidate; a failure is fixed and a new candidate tagged. Then the 1.0 gate |

## 6. Critical path

W1's ladder is the longest chain: M0a → M1 → glyph gate → M2 → M3 → M4 → M5 → M6, then the feature gate and all of Phase 7. Two choices keep it short:

- **Answer Q4 at the definition gate.** Criteria §4 makes the R4 answer an input to where the text engine lives, so while Q4 is open, M0a waits for the frame-loop gate and its spikes. Proposed answer: R4 moves to 1.x and comes back only if C1 shows a game-frame row over budget that R4 would fix. It pays only where a CPU-bound frame has GPU slack (at most about 25% on `integrated`, criteria §5.2). It adds a frame of latency, turns every main-thread renderer call into a command or a sync point, and changes what `total` means.
- **Start M0b in Track A.** It is independent of M0a (w01 §11) and of the frame-loop gate.

After that, M2 waits on W8a, M3 on W15a and M4 on W11b. Each of those starts in an earlier track, so none should hold the ladder up.

## 7. Questions by when

| Question (criteria §10) | Needed by |
| --- | --- |
| Q1 definition and acceptance game; Q11 `wgpu`/`winit` | Definition gate |
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
| Q2 crates.io | Freeze gate: crate names and metadata freeze with the API |
| Q12 budgets | C1 |

## 8. Amendments proposed to criteria.md

From a review of criteria.md and the UI draft on 2026-10-03. For each, the owner accepts or rejects it, and accepted ones are folded in.

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
12. **Size.** At about 25,000 tokens, criteria.md no longer fits in one read: on 2026-10-03 the Read tool cut it off at line 333. Graduation (folder README) shrinks it one workstream at a time. Also move the revision paragraph under the header into a dated list at the end, and move the answered questions (17, 18, 19, 23, and the paragraph above the list) into an "Answered" list, so that §10 holds only open ones.
13. **Phases.** Replace §9's five phase labels with the three in §1 above, or record why five are better.

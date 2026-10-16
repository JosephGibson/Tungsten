# Phase 7 rewrite — draft

A working draft that restructures Phase 7, the road to 1.0, around two aims; only the feature gate adopts it. §1 is the order both parts feed, §2 and §3 the proposals, §4 the verification record, §5 the critique rounds, §6 the open questions with defaults, §7 the adoption notes. Facts marked `@0732305` were measured at that commit on 2026-10-16 and are rechecked before adoption (ordered step 3). Decisions and constraints are settled here; mechanisms belong to each step's plan.

- **status:** draft
- **goal:** Restructure Phase 7 around two aims, as the last waterfall, the one that gets the project out of the waterfall loop:
  1. Every optimization needing an API or data-format change lands before the freeze gate, so C2 tunes internals only.
  2. 1.0 is a soft reset: a conventional Git flow and semver, a scripted release pipeline, rewritten AI workflow and skills, a simple status display in place of the roadmap site, far fewer and leaner docs, a crushed and rebuilt test suite, and a less waterfall-driven way of working after 1.0.
- **non-goals:**
  - Edits outside this file: decisions, the register, `roadmap.json`, workstream files.
  - Anything that changes Phase 6 (owner: Phase 6 closes before any of this is considered).
  - Dates and time estimates.
  - Budget values (Q12).
  - Kit block scope.
  - Implementation, and the mechanisms of later steps (the release script's steps, the crush's run procedure, the rehearsal matrix): each step's plan specifies them with current facts.
- **files to touch:** This file while it is a draft. On adoption, at the feature gate:
  - [implementation plan](1.0/implementation-plan.md) §2 (freeze gate row), §5, §6, §7 and §10;
  - [w10](1.0/w10-closing-phases.md);
  - `roadmap.json`;
  - the decision entries listed in §3 (R9).
- **ordered steps:**
  1. A final review pass by a fresh session: verify against the project, improve, critique and act on the critique. Done 2026-10-16 (§4, §5).
  2. The owner answers the open questions (§6), or accepts their stated defaults.
  3. **Recheck before adoption**, in the feature gate's session, against the tree then: the context digest (implementation plan §2 and §5, the register's Phase 6 rows, criteria §10); every `@0732305` line reference in §2.1 and §2.2; the figures in §3; the tool and crate versions in R2, R7 and R10; who uses `tungsten::testing`; `release.yml`'s triggers; the read signal, re-mined if the transcripts have grown by half. Replace the marker's commit and record what moved in §4.
  4. The feature gate adopts or rejects the order in §1 and each proposal; this file is then superseded.
- **done-when:**
  - Every open question has an owner answer or a stated default.
  - The recheck of step 3 has run and §4 names its commit.
  - §1's order has been adopted, changed or rejected at the feature gate.
  - The critique's findings have each been accepted with a change here or answered.

## Context digest

`@0732305`, with the kit catalogue's uncommitted edits to `docs/plans/1.0/` of 2026-10-16.

- **Phase 7 today** ([implementation plan §5](1.0/implementation-plan.md#5-phase-7-release)): C1 → W4b → freeze gate → C2 → W7b, W9b, W9c → C3 on `v1.0.0-rc.N` → 1.0 gate. C1: a dated baseline, the new rows, a budget per row, and the fixes that need an API change; W4b: the freeze gate's list plus `#[non_exhaustive]` and rustdoc examples; C2: internal changes only.
- **Gates** ([§2](1.0/implementation-plan.md#2-gates)): feature gate: the acceptance game plays from a `tungsten package` archive, every Must candidate landed, the Phase 6 QA pass closed, the break ledger holds only what Phase 7 lands; Q3, Q10's rest and Q12 answered there. Freeze gate: C1 baselines and budgets dated, break ledger empty, arity-named queries removed, `missing_docs` clean, API snapshot taken; Q2. RC gate: every budget passes or has an accepted exception; `v1.0.0-rc.N` publishes without a pull request.
- **Owner-question defaults that shrink Phase 7** ([criteria §10](1.0/criteria.md#10-questions-for-the-owner)): Q2, no crates.io, so the freeze gate strikes W9c and RC-D5. Q3, Linux certified, other platforms on stated tiers, no macOS release build, so W7b shrinks to the README's tiers.
- **Settled for parallelism** (`D-131`, Q4–Q6): no workers and no `Send`/`Sync` bounds in 1.0; a parallel path bounds the types it slices at its call site; a rayon `ThreadPool` owned by `App` if a fork-join stage ever lands. R4, the pipelined render thread, returns only if C1 shows a game-frame row over budget that R4 would fix (`D-102`).
- **Before the feature gate:** W13e–W13k (the kit catalogue's F1, F2, F3, A, M, C and E; its eighth group G was dissolved into them, [w13-kit §7](1.0/w13-kit.md#7-catalogue-by-milestone)), W6b and W6c, W14b, W12b with W1 M6 after W13k, then the Phase 6 QA pass. At `0732305` the first open stop was `w13e`; M60 cut it as 0.80 in the tree, so `w13f` and M61 are next. None of this changes.
- **Owner's principles** ([Owner answers](#owner-answers-2026-10-16)): API- and format-changing optimizations before the freeze; the bench suite rewritten only after the kit and every core system are finished, purging the old data; nothing of the reset during Phase 6.

## 1. Phase 7 at a glance

| # | Step | Contents | Flow |
| --- | --- | --- | --- |
| — | Feature gate | As today; settles Q3 and Q12; adopts this plan after the recheck (ordered step 3) | Old |
| 7.1 | Bench rewrite | Runner and workloads on the finished kit and core (§2.2); `tungsten-perf` rewritten with it; the old perf data purged | Old |
| 7.2 | C1 | The measured frame timeline, then frame, operation and scaling budgets (§2.5, Q12) and a baseline on every row. No fixes. May be 7.1's closing step, one candidate (R-Q14) | Old |
| 7.3 | Pass 1 | Triage (§2.1). Breaking-later fixes, ECS and the CPU frame first, then render and GPU; R4's check against `D-102`; the parallel-overlap probe | Old |
| 7.4 | Pass 2 (conditional) | Whole-frame remeasure, remaining breaking-later fixes, time guards. Skipped by its rule (§2.4) | Old |
| 7.5 | W4b, plus cutover prep | As today, plus the tunability sweep (§2.1) and the harness API the new test framework needs (R7). Cutover prep, small enough to share the session: commit format, hook, trunk rules, versioning policy, and the decision entries (R9). Its release is the old pipeline's last cut and the dated freeze baseline | Old |
| Gate | Freeze gate | As today, plus the cutover at its close. Q2's default strikes W9c | Switches to new |
| 7.6 | Process | The operating-model doc; `ROADMAP.md` and `just status` in place of the roadmap site and `roadmap.py`; the release script, its workflow trigger and its rehearsal; `tungsten-next` and the release skill (R3–R5) | New |
| 7.7 | Test crush | The crush on the frozen harness, with test-side helpers outside the public API (R7); `tungsten-plan` written on this step's own planning (R4) | New |
| 7.8 | C2 | Internal only, from the C2 queue that passes 1 and 2 built | New |
| 7.9 | Docs | The doc map and triage; `ARCHITECTURE.md` in place of `DESIGN.md`; W9b's guide pass; W7b's README tiers; the decision policy and `tungsten-decision` (R6) | New |
| 7.10 | C3 | The release checklist on `v1.0.0-rc.N`, cut by the new script | New |
| 7.11 | Final sweep and 1.0 | Retire the old records (R8); prune agent memory; `promote` the last RC to 1.0 (R3) | New |

**Against today's §5:** three extra steps before the freeze (bench rewrite, pass 1, a conditional pass 2), the cutover prep folded into W4b; after it, process, the test crush and the final sweep. W7b and W9c fold into Docs unless Q2 or Q3 is answered against its default. No 0.x release after W4b; the next tag is `v1.0.0-rc.1` (R3, R-Q10). Each skill is rewritten at the step that first needs it, with that step's tasks as its evaluations (R4), so skill work spreads over 7.1–7.9.

**Why this order.** Process first: 7.6 is the first session under the new flow and every later step commits under it. The crush before C2, so C2's changes run against the lean suite and a per-change mutants run. Docs after C2, so they describe the final internals. Steps that could merge have (the release script with process, W7b and W9c with docs).

## 2. Part A: performance before the freeze

### 2.1 The triage rule and the levers

"Needs an API change" is broader than "must land before the freeze". An optimization whose API can be added later, as a new type, method or opt-in that keeps today's default, can ship in 1.x without breaking anything. Each finding in the passes goes to exactly one class:

| Class | Test | Goes to |
| --- | --- | --- |
| Breaking-later | Needs a changed signature, a new bound or changed semantics, or a format change that existing files can't survive: they stop loading, or load with a different meaning | Fix before the freeze, with a break-ledger row citing the row or the measured cost |
| Additive-later | Needs only new API beside the old, or a new format version that existing files still load as before, directly or through a migration | 1.x backlog, unless its row is over a gating budget: then it lands before the freeze with the breaking-later fixes, since no API change lands after it |
| Internal | Changes no public surface | C2 queue |
| Accepted | The row is within budget | Nothing |

**Masking.** A breaking-later cost can hide behind an internal one. Where the passes can't see a floor, they use a what-if probe: stub out the internal cost and measure what's left. An internal fix lands before the freeze only when no probe can stand in for it.

**Levers to probe** (`@0732305`; paths under `crates/tungsten-core/src/`):

| Lever | Today | Breaking if added later? | Probe rows |
| --- | --- | --- | --- |
| Change detection | None. Mutable query items are plain `&'w mut A` (`ecs/query.rs:180`) | Per-entity ticks need a wrapper item type, which is breaking if `&mut A` must then mark changes. Column-level "touched this tick" fits behind today's items and is internal | Hierarchy, extract, kit `Stats` dirty flag |
| Retained extract | Extract reads `&World` and passes plain data each frame (`D-018`) | Yes, if the umbrella's curated render re-exports change (`D-103`; O7) | Sprite-heavy and UI frames |
| R4, pipelined render thread | Deferred to 1.x unless C1 shows a game-frame row over budget it would fix (`D-102`) | Likely: one frame of added latency changes what extract may assume | The acceptance-game frame |
| System access declarations | `SystemFn = Box<dyn FnMut(&mut World)>`; `SystemDesc` is a builder with private fields (`schedule.rs:132`, `:143`) | No: `.reads()`/`.writes()` can be added as builder methods | — |
| Send/Sync bounds | None, by decision (`D-131`) | No: a parallel path bounds at its own call site | — |
| Documented iteration orders | Promised: `World::query` yields rows in "archetype order, then row order" (`ecs/world.rs:224`) and the tuple helpers the same order; the command buffer flushes in recording order, the event queue in frame order | Yes: a lever that reorders rows (chunked storage, change-detection columns) or replays commands differently breaks a documented promise. The sweep decides which orders stay promised; one it unpromises before the freeze is a 0.x break, not a 1.x one | ECS churn, physics determinism |
| Containers in signatures | No `&Vec` or `&HashMap`. Slices are returned by 13 methods: `DebugDraw::commands`, `ActionMap::bindings`, `InputState::raw_events`, `PostStack::as_slice`, `FontFeatures::as_slice`, `PaintList::mesh_vertices`, `UiTree::children`, `::mesh_vertices` and `::roots` (core), `PassOrder::as_slice` (render), `AtlasRegistry::page_handles` and `Harness::audio` (umbrella), `Spawner::prefab` as `&str` (kit). Public `Vec` fields on 73 core, 11 umbrella and 8 render lines, `HashMap` fields on 48 core and 2 umbrella lines (`api/*.txt` lines) | Yes: a `&[T]` return pins contiguous storage, a public container field pins the container and its element type, and `Clone`/`Copy` promised on a type that may later hold heap state pins that too. The sweep lists each and wraps the ones on hot data | The rows those types sit on |
| Data formats | JSON scenes, prefabs, manifest, the `data` section; no preprocessing (AGENTS.md) | Only if existing files can't be kept loadable. A faster representation that old files still reach through a migration or a second reader is additive-later; its cost is a slow path kept for old files | Startup, load, hot reload |
| Format versioning | To check, per engine-read format. Likely met for the strict-key JSON objects, where a later `version` field can be optional and absent reads as 1.0's | Yes, for any format with no compatible way to say which version a file is | — |

**The tunability sweep in W4b** reviews the public API for documented iteration orders, containers in signatures, `Clone`/`Copy` promises and format versioning. An opaque type today makes a later fix internal. The table above is its starting list, rechecked at the time.

### 2.2 Bench rewrite (step 7.1)

**Timing (owner).** After the kit and every core system are finished, which is the feature gate. Kit milestones keep capturing on today's suite until then.

**Scope: both the runner and the workloads, in one candidate.** Today's suite (`@0732305`): `scripts/bench.py` (62 KiB: capture, sweeps, a capacity search against a frame budget), `bench_report.py` (100 KiB: compares and verdicts), the `examples/02_bench` harness and eight `benches/` files. What the rewrite adds is what past capture sessions wrote into plans by hand; the rewrite's plan confirms each item against the scripts and [benchmarks.md](../perf/benchmarks.md):

**Runner:** verdicts against absolute budgets as well as A/B compares; a same-sitting A/A and the one-rerun way-out for known same-build drift rows, built in rather than written into each plan; code-placement noise handled by default; a contamination check at sitting start; RSS compared within a sitting only; deterministic counters (allocations, archetype moves, events, draw calls, upload bytes) as noise-free guards in CI and `just check`, with time rows local; a short guard mode beside the full sitting; GPU timestamps read back without stalling the frame, since today's timed frame blocks on `device.poll` each frame (`crates/tungsten-render/src/timing.rs:161` `@0732305`) and §2.5's timeline must keep the overlap it measures.

**Workloads:** C1's listed rows; rows from the rebuilt example games; structural stress rows (entity-count scaling, spawn and despawn churn, event volume, load and hot reload, startup parse); each row names its budget category (§2.5) and the API surfaces it exercises, so an over-budget row maps to its levers. The acceptance game's row is cross-repository ([acceptance game](1.0/acceptance-game.md); `D-102` clones it beside this checkout): a pinned revision or a vendored snapshot (R-Q12).

**Purge (owner):** `perf-runs/` (`@0732305`: 9.7 GB in 801 entries, untracked apart from `.gitkeep`); the old suite's code and tests (`bench.py`, `bench_report.py`, `test-bench.py`, 227 KiB) and the crates' `benches/`; `docs/perf/benchmarks.md` (92 KiB), rewritten with the suite; the agent's perf memory. **Before purging,** carry `benchmarks.md`'s "Engine findings" and "Open proposals" (lines 458 and 474) into pass 1's lever list, and keep the machine-level capture facts that stay true: remote-desktop contamination, the compositor's acquire stall, THP shifting RSS, code-placement noise.

### 2.3 Parallel systems

**Mostly settled already.** `D-131` and Q5–Q6 rule out workers and Send/Sync bounds in 1.0. Parallel by default would supersede `D-131` and the two-thread rule. Example 01's known costs are algorithmic (per-entity scans, per-step hash grids, 12,000 bodies), which threads would not fix. The recommendation is a 1.x opt-in.

**What remains before the freeze:**
1. Document that only declared constraints order systems. The resolver stays deterministic, but undeclared order is not a promise.
2. Keep each kit item card's writes as data a 1.x scheduler can read.
3. Pass 1 measures the upper bound: the share of the CPU frame in systems that could overlap under the phase anchors. That number sets the 1.x row's priority.

### 2.4 The two passes

- **Split by fix class, not by area.** Pass 1 takes the structural and breaking-later fixes, ECS and the CPU frame first, then render and GPU. Pass 2 remeasures the whole frame, takes the remaining breaking-later fixes and adds time guards.
- **Why not split by area:** extract reads the World on the main thread, so render fixes move cost back across the seam. Costs that move between passes fail per-pass checks unless frame time and GPU engine time are judged, and an area split leaves no whole-frame remeasure before the freeze.
- **Pass 2's skip rule:** it is skipped when pass 1 closes with no breaking-later or budget-gated additive finding open and every row either within budget or holding an internal fix in the C2 queue. Its guards then move into W4b.
- **Stop condition for both passes:** a fix lands only for a row over budget or a measured structural cost. "One more API fix" without that evidence goes to the 1.x backlog.

### 2.5 Budgets

Three categories, because not every row is a frame: **frame** budgets for play, **operation** budgets for work that finishes, and **scaling** budgets for how cost grows. Q12 picks which categories gate 1.0 and their targets (R-Q13).

**Frame budgets, from a measured timeline.** CPU work, GPU work and presentation overlap: with frames in flight, the CPU records frame N+1 while the GPU executes frame N, so throughput is bounded by the slower resource, not the sum, and subtracting GPU time or the compositor's stall from a CPU budget would double-count. So:
1. **Measure the timeline first (C1):** per frame on the reference machine, CPU work spans by stage, CPU wait spans (surface acquire, present, GPU sync), GPU pass timestamps, the present mode and frames in flight, on a capture path that keeps the overlap (§2.2). The overlap is measured, not assumed.
2. **One throughput budget per resource:** CPU main-thread work per frame and GPU engine time per frame are each held under the frame period less a stated headroom. Stage and row budgets split their own resource's budget, never the other's.
3. **Waits are not work:** acquire and present waits are judged as pacing (frame-time p95 and jitter against the refresh) and charged to no stage. The compositor's acquire stall lands here.
4. **Latency is a separate budget,** if Q12 wants one: input to present sums along the critical path, so it is judged on its own row.

A frame row's budget then says whether a structural change pays: the row is over its share of the critical resource.

**Operation budgets.** Startup to first frame, a scene load and a hot reload are judged on wall time to completion and, for an operation that runs during play, the worst frame hitch while it runs, against the frame budget's pacing. Staying under a per-frame CPU share doesn't excuse a load that takes too long to finish.

**Scaling budgets.** Entity-count, event-volume and body-count rows are judged on their curve: the cost per unit stays inside a stated shape (linear, say) up to a stated count, and the count where a frame budget breaks is reported. A structural cost usually shows here first, as a bend in the curve, before any single frame row fails.

### 2.6 What it costs

- **Releases before the freeze:** today 2 (C1, W4b); proposed 4–5 (bench rewrite, C1, pass 1, pass 2 if needed, W4b with the cutover prep), or 3–4 when C1 is 7.1's closing step (R-Q14). The bench rewrite and C1 change no public API, so their plans carry no API critique.
- **Sessions after the freeze:** process, test crush and the final sweep are added; W7b and W9c probably fold away.
- **Costs:** API churn through the template, kit recipes and the acceptance game; re-dating baselines (budgets at C1, baselines at W4b's release capture); pressure to add scope, which the stop condition answers; capture time, since sittings hold the machine; critique rounds.
- **The cost of skipping it:** a breaking-later fix found after the freeze means a 2.0, a deprecated pair carried through 1.x, or an RC-gate exception.

## 3. Part B: 1.0 as a soft reset

### Vision

1.0 is the big bang: one baseline with a frozen API that the owner is satisfied with, and a lighter way of working after it. The waterfall machinery got the engine here: phases, gates, workstreams, milestone plans, the register, the roadmap site, a decision for nearly every choice. Phase 7 uses it one last time, to retire it. After 1.0, the project runs on small Conventional Commits on one trunk, semver releases cut by a script, a short list of what's next, docs that describe the current state, and tests and skills that earn their keep.

**Principles:**
1. **Current state over history.** Docs say how things are now, with the why in a sentence or two; history lives in Git.
2. **Derive, don't maintain.** Whatever a tool can produce (changelog, API diff, status, file map) is produced, not hand-kept.
3. **Scripts over prose.** Deterministic steps live in scripts. Skills and docs call them instead of describing them.
4. **Plan the next thing, not the programme.** Detailed plans only for work that spans sessions. The horizon is Now / Next / Later.
5. **Ceremony scales with risk.** A typo is a commit; a public API change gets a sketch and a critique; a major release gets an RC.
6. **One source per fact.** A process lives in one doc, and skills point to it.
7. **Retire a thing at the step its replacement lands.** Nothing goes during Phase 6.
8. **Earn its keep, measurably.** The same rule crushes three things: a test whose deletion leaves every caught bug still caught, a skill that doesn't beat a no-skill baseline, and a doc with no purpose or consumer.

### Evidence of the overhead (`@0732305`)

- **Docs:** 48 Markdown docs at the root and under `docs/`, outside the plans archive, 1.87 MB; 65 with the crates', templates' and skills' files. The largest: `DECISIONS.md` 579 KiB (169 entries, 101 amendment and 9 supersession markers), `CHANGELOG.md` 288 KiB, `DESIGN.md` 114 KiB, `docs/perf/` 136 KiB. `docs/plans/1.0/` is 24 files, 567 KiB: w01 95 KiB, the implementation plan 74 KiB, w13 55 KiB, criteria 52 KiB, w04 49 KiB, `roadmap.json` 46 KiB. The plans archive is not counted.
- **Write churn**, raw `git log` over the 87 squashed release commits: `DESIGN.md` and `CHANGELOG.md` changed in 80, `DECISIONS.md` in 76, the LLM index in 64, the decision index in 61, `AGENTS.md` in 38, the implementation plan, `README.md` and `known-issues.md` in 34 each. Each commit is a whole release.
- **Read signal**, mined 2026-10-16 from the 331 Claude Code transcripts (698 MB, 2026-09-25 to 10-16) by counting sessions whose Read calls or Bash commands name a doc: `DECISIONS.md` 179 sessions (28 through Read, the rest `rg` and `sed`), the decision index 181, the LLM index 171, `docs/plans/README.md` 153, `CHANGELOG.md` 149, the implementation plan 147, `DESIGN.md` 143, `workflow.md` 128, `docs/releases.md` 126, `README.md` 122, `known-issues.md` and the profiling workflow 107, `AGENTS.md` 104 beyond its auto-load. No doc under `docs/` went unread. So the bookkeeping docs are written at every release *and* searched in a third to a half of sessions: their cost is size per search and the release-time edit. The counts see agents only and can't separate consumption from bookkeeping.
- **Tooling:** `scripts/` holds 16 Python files (511 KiB, 39% of it tests of the tooling) and 4 shell scripts; `release.py`, `release-preflight.py` and their tests are 128 KiB, `roadmap.py` and its test 39 KiB; 29 `just` recipes. Example 01's asset generator is product, not tooling.
- **Skills:** seven project skills, 30 KiB of `SKILL.md`; `tungsten-next` also carries `prompts.md`, `roadmap.md`, `state.sh` and the 49 KiB roadmap viewer `page.html` (`D-109`), `tungsten-milestone` its skeleton.
- **Tests:** 2,126 `#[test]` functions: core 1,086, umbrella 530, render 188, kit 154, example 01 106, the rest 62; 13 integration-test files under `crates/*/tests/`, one of them `tungsten-core/tests/decision_index.rs`, a doc-bookkeeping test inside the engine crate.
- **Branches:** a local branch per version, `0.48` to `0.80`; `0.45` to `0.80` on the remote.
- **Tools not yet installed on the reference machine:** `cargo-semver-checks`, `cargo-mutants`, `git-cliff`, `release-plz`; `cargo-nextest` 0.9.133 and `cargo-public-api` 0.52.0 are.

### R1. Git flow

- **Trunk-based.** `main` is the only long-lived branch. Releases are tags on `main`. The per-version branches are deleted at the cutover, locally and on the remote.
- **One short-lived branch per task** (`type/slug`), rebased and merged with `--ff-only`, then deleted. A two-minute fix goes straight to `main`.
- **Agents commit at each logical step; the owner pushes.** The owner reads `git log main..`, fast-forwards and pushes. Publishing stays the owner's, which is `D-105`'s intent, but without the uncommitted tree.
- **No squashing by default.** PRs are optional: CI only reports (`D-070`), so a solo dev gains little from one. `ci.yml` already runs on pushes as well as pull requests.
- **Message format:** `type(scope): summary`. One line, imperative, lowercase, at most 72 characters, no attribution. Types `feat`, `fix`, `perf`, `refactor`, `test`, `docs`, `build`, `ci`, `chore`; scopes a crate or subsystem; breaking changes `feat(ecs)!: ...` ([Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/)).
- **Every commit builds and passes the fast tests,** so `git bisect` works. Tests ship with their code, docs with what they describe.
- **Hooks:** a `commit-msg` format check in a small script; optionally `cargo fmt --check` before commit.
- **Agents commit from the first session after the gate,** so the cutover prep in 7.5 patches the old skills only as far as committing.

### R2. Versioning

- **Semver from 1.0.0:** patch (1.0.1) fixes only; minor (1.1.0) additive features and deprecations; major (2.0.0) any break to the public API or a data format. **A break makes a major, not size:** a big release that breaks nothing is a minor.
- **Lockstep:** every workspace crate shares one version (`workspace.package.version` already does).
- **Deprecate in a minor, remove in the next major.** Adding `#[deprecated]` is a minor change and removal a major one ([Cargo's SemVer reference](https://doc.rust-lang.org/cargo/reference/semver.html)).
- **A format change that old files can't load is a major,** unless a migration loads them.
- **`-rc.N` for majors only,** as an RC series that freezes its API at `rc.1` (R3).
- **MSRV:** `rust-version` is set in the workspace; raising it with the pinned toolchain is a minor (`D-103`; Cargo's reference recommends the same).
- **API checks:** keep the `cargo-public-api` snapshot (`just api`, `D-104`, `D-107`) as the authoritative diff; `just api-check` only proves it fresh, so the release gate diffs it against the last tag's (R3). Add `cargo-semver-checks --baseline-rev <last tag> --release-type <kind>` as a cheap release gate, not a replacement. It analyses one crate's rustdoc JSON, so items re-exported from other crates are invisible to it (mostly false positives, and a removed re-export can pass), and it checks no types, generics or lifetimes ([project goal](https://goals.rust-lang.org/2025h2/cargo-semver-checks.html), [maintainer](https://predr.ag/blog/four-challenges-cargo-semver-checks-has-yet-to-tackle/)). The umbrella re-exports core and the kit, so the snapshot stays the gate for those.

### R3. Release pipeline

- **Keep what works:** `release.yml`'s build and publish jobs (per target and CPU level, notices, the crash probe, release notes from `CHANGELOG.md`) and `release.py`'s CI half, the `version`, `check`, `licenses`, `package` and `notes` subcommands the workflow calls.
- **Change the trigger.** Today a final `vX.Y.Z` tag publishes only when the release pull request merges into `main`; only prerelease tags publish on push (`D-079`; `release.yml` lines 17–26 `@0732305`). With pull requests optional, the final-tag path goes back to a tag push, `D-071`'s original trigger, in 7.6's decision (R9). A prerelease tag rehearses the build and publish jobs; the changed trigger is first exercised by `promote`'s 1.0 tag.
- **Replace the local half** with one `just release <kind>`: today `release-cut` (which already runs `cargo update --workspace` and writes the roadmap summary), `release-preflight`, the pull request, `D-152`'s README fields, the register row, the roadmap sync and `docs/releases.md`'s procedure. The kind is stated explicitly; commit types only set a floor under it.
- **Release kinds:**

  | Kind | From → to | Compatibility rule |
  | --- | --- | --- |
  | `patch`, `minor` | `X.Y.Z` → the next | The requested kind is at least the floor the commits since the last tag imply (`git cliff --bumped-version`: a `feat` needs a minor), and no `!` commit is present: a break refuses both kinds and needs a major. The promised snapshots (`api/tungsten.txt` and the kit's) diffed against the last tag's are empty for `patch` and additions only for `minor`; anything else is a break. Additions are then classified, since the diff can't: a new required trait method, a new variant or public field on a type without `#[non_exhaustive]`, or a new blanket impl is a break (Cargo's SemVer reference). `cargo-semver-checks --release-type <kind>` against the last tag as the second opinion |
  | `rc <X.0.0>` | Starts a major's series: the last tag (stable, or 0.x for 1.0) → `X.0.0-rc.1`; continues it: `rc.N` → `rc.N+1` | The series freezes the API at its start: `rc.1` records the snapshot and every later RC must match it, and 1.0's `rc.1` must match the freeze gate's snapshot at W4b's tag; both apart from exceptions accepted in a decision entry (R7's harness exception is one). No commit floor: the series sets the version |
  | `promote` | `X.0.0-rc.N` → `X.0.0` | The final diff from `rc.N` touches only the version fields, `CHANGELOG.md`, the record paths the sweep deletes and the three line removals R8 names (an allowlist). `Cargo.lock` may differ only in the workspace crates' own `version` lines. Any other change needs another RC |

  There is no direct `major` kind. Every major goes `rc` → `promote`, which is what R2's "an RC for majors" means in practice.

- **Constraints on the script**, whatever its steps: the tree that gets tagged is the tree that was checked, so versions, lockfile and changelog are applied in the working tree, that tree is validated (`just check` and `just api-check`; the checks the validation matrix selects for the diff since the last tag, `just checks --base <last tag>`, so a patch that touches physics or shaders runs `just physics-release`, `just smoke` or `just visual` as the matrix says; `cargo-semver-checks` for `patch` and `minor`; the tag, version and changelog agreement `release.yml`'s verify job repeats; the kinds table's diff rules on the final diff), and only then is `chore(release): vX.Y.Z` committed and tagged; on failure the tree is restored and nothing is committed; the owner pushes the tag. The changelog section comes from `git-cliff`; the same version derivation feeds `just status` (R5). 7.6's plan specifies the steps.
- **Rehearse before C3, in a scratch clone with no push:** every kind and every refusal (dirty tree, failed check, a `minor` over a `!` commit, a drifted RC snapshot, a promotion diff outside the allowlist, a lockfile change outside the workspace versions) at least once; 7.6's plan writes the matrix.
- **Ceremony by release type**, on top of the matrix-selected checks, which the kind never reduces: patch, the script alone; minor, a perf sitting and a short checklist; major, an RC and the full checklist.
- **`release-plz` was considered.** Its `update` bumps versions, writes the changelog and runs `cargo-semver-checks`, but it starts by downloading the project's packages from the registry ([docs](https://release-plz.dev/docs/usage/update)). With no crates.io (Q2's default), a small script over `git-cliff` is simpler. Revisit if Q2 changes.
- **The changelog starts fresh at 1.0** with a "what's in 1.0" summary. The 0.x changelog is retired with the other old records.
- **No 0.x releases after W4b.** Between the freeze and the RC nothing ships; the next tag is `v1.0.0-rc.1`, the new script's first real run after the rehearsal, so the old and new pipelines never both need to work.

### R4. AI workflow and skills

**The operating-model doc comes first.** One doc covers how work is picked, the plan tiers, commits, review and releases. It replaces `docs/plans/1.0/workflow.md`, the plans README's routes and `docs/releases.md`'s procedure. Skills implement it and never restate it.

**The skill rewrite follows Anthropic's authoring guidance** ([best practices](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices)): evaluations before writing (run real tasks without the skill, write three scenarios from where it falls short, measure the no-skill baseline, write the minimum that passes; a skill that doesn't beat its baseline is deleted, principle 8); only what Claude doesn't already know, in third-person descriptions of what the skill does and when, a body under 500 lines, references one level deep; low-freedom exact scripts for fragile steps such as releases and high freedom for review and judgement; one instance writes, a fresh one uses it on real tasks, and observed misses feed back.

**Each skill is rewritten at the step that first needs it,** because the evaluations need real tasks and those tasks are the steps: `tungsten-perf` at 7.1 on the suite's first sittings, `tungsten-next` and the release skill at 7.6 (the rehearsal is the release skill's evaluation), `tungsten-plan` at 7.7 on the crush's own planning, `tungsten-decision` at 7.9 with the decision policy. 7.6 writes the doc the skills implement and the two it needs itself.

**Plan tiers replace the milestone machinery:** small, one sitting, no plan, the commit is the record; medium, one session, the plan lives in the session; large, several sessions, one plan file (goal, non-goals, steps, done-when as passing tests plus a demo), deleted when done. Critique goes on public API changes and risky areas (concurrency, formats, the render seam).

**Skill set, first guess:**

| Today | After |
| --- | --- |
| `critique` | Keep as is (owner: works well) |
| `tungsten-next` | Lean: run `just status`, read `ROADMAP.md`, say what's next; no artifact sync |
| `tungsten-milestone` | `tungsten-plan`: the large tier's plan file and execution rules |
| `tungsten-release`, `tungsten-finalize` | One thin wrapper over `just release` |
| `tungsten-decision` | Shrinks with R9's decision policy |
| `tungsten-perf` | Rewritten with the bench suite (§2.2) |
| `tungsten-wgpu` | Keep and refresh; domain knowledge ages better than process |

**Also in scope:**
- **`.claude/hooks/guard.sh`** (the owner's forbidden-command guard) and the `just ctx` agent-budget check go through the same keep-or-crush test.
- **`templates/basic/AGENTS.md` is product, not process:** it ships agent rules to games and C3 tests it in RC-A9. It is reviewed for the new conventions, not slimmed away.
- **Agent memory is pruned twice:** the perf notes at 7.1, the rest at 7.11.

### R5. Project state display

- **The roadmap artifact site and its sync go at 7.6** (owner), with `tungsten-next`'s sync, `page.html`, `roadmap.py` and its test. `D-109` and `D-118` are superseded.
- **`ROADMAP.md`:** Now / Next / Later, one line each, hand-kept and short (R-Q4 for its place).
- **`just status`:** derived output with the version, commits since the last tag by type, the next version they imply (R3's derivation), open branches, and the Now list.
- **GitHub Issues: not for now.** Agents read a file for free, while issues need network calls and a second place to look.

### R6. Docs (step 7.9)

- **The end goal** (owner): far fewer docs, leaner and more focused. Size is a soft guideline, not a rule, and a doc may grow where the value justifies it.
- **Define the end-state doc map first**, then map every doc onto it: keep, merge into X, generate from code, or retire.
- **Triage signals set the review order, not the verdict:** writes from raw `git log`; agent reads from the transcripts, as mined above; inbound references (links from other docs, the README, the template and rustdoc, plus scripts and workflows that read the file: `release.yml` reads `CHANGELOG.md`, `roadmap.py` the register). The mining shows no bookkeeping doc is unread, so the signals order the review by cost per search, and the first question for each is whether a reader needs the whole file or a distilled page.
- **A doc is retired only after a purpose check:** what it is for; who consumes it (the owner, game developers through the template or the guide, agents, or scripts); whether its content is still true. The owner names the docs they read themselves. Each retirement records where any live content went (merged into X, generated by Y, or nowhere), and a doc with a machine consumer moves with that consumer.
- **`ARCHITECTURE.md` replaces `DESIGN.md`**, in the shape matklad describes ([ARCHITECTURE.md](https://matklad.github.io/2021/02/06/ARCHITECTURE.md.html)): a short overview and a codemap of the coarse modules; important names without links; invariants, especially absences ("core never calls render"); boundaries; a separate cross-cutting section; revisited a couple of times a year.
- **One agent layer:** a short `AGENTS.md` with its read path, plus `ARCHITECTURE.md`'s codemap, in place of `AGENTS.md`, the LLM index and its byte budget together.
- **Decisions:** the load-bearing ones (a guess: 20–30 of 169) are distilled into the docs whose subject they constrain. After 1.0, record a decision only when it constrains future work (dependencies, API policy, platforms), as a short ADR with no index test.
- **Size is a smell, not a rule.** Past a few hundred lines, ask whether a doc has two purposes. No test-enforced byte budgets. Watch the total doc count.

### R7. Test crush (step 7.7)

- **Value is measured by preservation, not attribution.** `cargo-mutants` records per mutant whether the suite caught it; which test failed is only in that mutant's log, `outcomes.json` has no per-test record, and the run stops at the first failing target ([mutants.out](https://mutants.rs/mutants-out.html), [fail-fast](https://mutants.rs/fail-fast.html)). So the criterion is a per-crate baseline of caught mutants: a deletion batch, chosen by judgement and cheap signals, is accepted only if every mutant caught at baseline is still caught; a newly missed mutant restores the tests that covered it or is accepted in writing as worthless (a log string, say). Two tests sharing their only kills fail this check together, which is the point. Attribution runs only for a failing batch without an obvious culprit. 7.7's plan sets the batch procedure and the run scoping (one crate at a time, core first; `--shard k/n`; `test_tool = "nextest"`, which runs no doctests; CPU tests in a scratch copy under `target/`; `--in-diff` per change afterwards).
- **What mutation testing can't see stays out of the crush by rule:** determinism and containment hashes, GPU and pixel tests, perf, and tests whose subject is data files rather than Rust code. Coverage alone only shows that a line ran.
- **The harness is frozen public API.** `api/tungsten.txt` lists `tungsten::testing` (75 lines `@0732305`: `Harness`, `UiHarness`, `FrameDraw`; the umbrella's `testing` feature, `D-110`), and the template and examples 01 and 05 use it. So the framework is designed in two halves: in W4b, whatever it needs from `tungsten::testing`, a level C API sketch with its critique; at 7.7, helpers on the test side only, in each crate's `src/tests/` or an unpublished support crate. A harness change found after the freeze is an additive exception in a decision entry, or waits for 1.1.
- **The new framework centres on the headless harness.** Scenario tests at the public API (load a scene, drive input, step, assert world state) become the 1.0 stability contract. Snapshot tests cover formats, the API and UI layout.
- **Keep:** real-bug regression tests unless subsumed; the physics determinism and containment tests; asset coverage. **Crush candidates:** tests that pin private shapes or log strings; doc-bookkeeping tests (`decision_index.rs`); duplicates across layers. **The tooling's own tests go through the same triage;** most of the Python test bulk goes with the scripts it tests (R3, R5, §2.2).
- **One integration-test binary per crate,** for compile time.
- **Stop condition, per crate:** the surviving suite still catches the baseline caught set, less the written acceptances, and the crate meets the test-count and runtime targets set at 7.7's start. Reading every test is not the condition.

### R8. Retiring the old records (step 7.11)

- **The old records stay until the final phase** (owner).
- **Then, two options** (R-Q11), both keeping the sweep deletions-only, which `promote`'s allowlist needs. **(a) Archive folder:** copies into a `docs/history/` folder, ignored like the plans archive, committed before the last RC; the sweep deletes the originals. A record edited after the copy (C3's exception decisions land in `DECISIONS.md`) makes it stale, so the copy is refreshed and the RC recut. **(b) Delete and tag:** the sweep deletes every record, `DECISIONS.md` included, in one commit whose parent is tagged `legacy-records`; `just history <pattern>` runs `git grep -n <pattern> legacy-records`, so the decisions stay searchable with nothing in the tree. The lean is (b): no files, nothing to go stale, principle 2 applied to history.
- **What goes:** the register and `roadmap.json`; the gate records and the workstream files; the 1.0 plans folder and the archived plans; the 0.x changelog; the decision index (its test goes earlier, at 7.7).
- **1.0 is the `promote` of the last RC, after the sweep** (R3), so the 1.0 tag is the clean baseline.
- **The sweep is deletions only.** Before the last RC, every check, script or test that reads a retiring record is removed or retargeted at 7.6, 7.7 or 7.9: `decision_index.rs`, `check-repo.py`'s plan, index and roadmap checks and its `check_agent_config` (which requires `.ignore`'s archive line and the archive deny rule in `.claude/settings.json`), and `roadmap.py`. `promote`'s allowlist then admits the sweep and nothing else.
- **References to the retired paths are handled ahead too.** At 7.9, links from kept docs to the records are removed, so the sweep breaks no link, and `just history` is added before its tag exists. In the sweep itself, three edits only, which the allowlist names: `.ignore`'s archive line, `AGENTS.md`'s archive rule and `.claude/settings.json`'s archive deny rule. All three have to stay until the archive is gone.

### R9. Decisions the reset needs (each written at its step, not here)

Until 7.11 they go into `DECISIONS.md` as usual.

- **At 7.5:** agents commit, the human pushes and tags (supersedes `D-105` and the commit clauses of `D-079`); trunk-based flow with optional PRs (supersedes `D-079`'s PR handoff); the semver policy (R2).
- **At 7.6:** the release script and its kinds (R3), and `release.yml`'s final-tag trigger back to a tag push (supersedes `D-079`'s merge-publishes clause, `D-152`'s README fields and parts of `D-071` and `D-074`); the roadmap site and catalogue retired (supersedes `D-109` and `D-118`); `git-cliff` and `cargo-semver-checks` as dev-only tools, installed at this step; not runtime dependencies, so `D-015` doesn't apply, but recorded anyway.
- **At 7.7:** `cargo-mutants` as a dev-only tool, installed at this step, and the test-value rule (R7).
- **At 7.9:** the decision policy after 1.0 (R6).

### R10. Other overhead to cut

- **Grep-count done-when checks.** Done means the tests pass and the demo works.
- **A perf sitting per milestone.** Replaced by counter guards and a sitting per minor or major release.
- **Gates.** Replaced by the release checklist after 1.0, scaled by release type.
- **Dependency bumps.** `D-103` hides `wgpu` and `winit` types behind the umbrella, so a bump after 1.0 is not automatically breaking. `winit` 0.31 is still a beta (`0.31.0-beta.3` of 2026-09-04; stable `0.30.13`; the workspace locks `0.30.12` `@0732305`): land it before the freeze only if it is stable by then and would leak through the hidden `Renderer` tier or change behaviour games depend on.

### Risks

- **The reset becomes its own waterfall.** Keep it to the principles, the stop conditions and a short list.
- **Rationale gets lost in distillation.** The tagged legacy tree is the safety net.
- **Skills restate a draft.** Doc first, skills second, evaluations before either.
- **No releases between the freeze and the RC,** so the RC could surface many problems at once. `just check` runs on every commit and the rehearsal exercises every release kind first.
- **Preservation reruns cost a mutants run per batch.** Start large on the obvious candidates, then go smaller.
- **Four new dev tools, none installed yet.** Each is installed and version-recorded at its step; one that fails on this workspace (rustdoc JSON needs `D-107`'s toolchain route) is dropped in that step's decision, not worked around.
- **Phase 7 grows to eleven steps after a long Phase 6.** Each post-freeze step carries a stop condition. Steps that can merge already have.

## 4. Verification record

**Review pass, 2026-10-16 (first session):** nine claims corrected against `D-131`, `D-102`, `D-103`, `release.yml` and raw `git log` (Send/Sync, registration, change detection, R4, dependency bumps, `cargo-semver-checks`, the release script's scope, write counts, the acceptance game's row); after rounds 1 and 2 the harness half moved before the freeze, the sweep became deletions-only and R9 aligned with §1.

**Final review pass, 2026-10-16 (this session), at `0732305`:**
1. **`release.yml`'s trigger** (R3, R9): under `D-079` a final tag publishes only through the release pull request's merge, not on push as the draft had it. R3 changes the trigger at 7.6 and keeps `release.py`'s CI half.
2. **Iteration orders** (§2.1): promised, not "to check" (`ecs/world.rs:224`), so reordering levers are breaking-later unless the sweep unpromises the order first.
3. **Containers in signatures** (§2.1): the 13 slice-returning methods and the container field counts listed from `api/*.txt`.
4. **Kit block** (digest): W13e–W13k, seven candidates; group G was dissolved ([w13-kit §7](1.0/w13-kit.md#7-catalogue-by-milestone)), so "W13e to W13l" was wrong.
5. **Figures** (§3): recounted at `0732305`; 16 Python files under `scripts/` not 20, 29 recipes not 30, 9.7 GB of `perf-runs/`, 331 transcripts, the generator separated from tooling, the skills' other files named.
6. **Read signal** (§3, R6): mined from the transcripts; every bookkeeping doc is searched in a third to a half of sessions, so "written often, rarely read" describes none of them, and R6's first triage question changed.
7. **`winit` 0.31** (R10): still a beta on crates.io; the condition now says "stable by then".
8. **`check_agent_config` and the settings deny rule** (R8): a third sweep edit and one more check to retarget.
9. **Tools** (§3, R9, Risks): none of the four is installed; each is installed at its step.
10. **Tool facts** rechecked at the source (`cargo-mutants`, `cargo-semver-checks`, `git-cliff`, `release-plz`); the skill guidance, matklad's shape and Cargo's SemVer reference hold as cited.

**Structure changed in this pass:** skills rewritten at the step that first needs them (R4, §1); 7.1 and 7.2 may share a release (R-Q14); `docs/releases.md` among what 7.6 replaces (R4); the release script's steps, the crush's run procedure and the rehearsal lists pushed down to 7.6's and 7.7's plans, their constraints kept here; the recheck step added (ordered step 3); every open question given a default (§6).

**Verified and holding** (`@0732305`): every decision ID cited is in [`DECISION_INDEX.md`](../DECISION_INDEX.md); the line references in §2.1 and §2.2; implementation plan §2 and §5 unchanged by the kit catalogue's edits; the test counts, branches and raw write counts; `release-cut`'s `cargo update --workspace`; RC-A9, RC-B2 and RC-D5 in the release checklist.

**Not rechecked:** the count of archived plans (the archive is never read); `scripts/bench.py` beyond its size and function list.

## 5. Critique

Round 1, 2026-10-16: artifact-only, gpt-6-astra at xhigh, 5h window 41% → 47%. Six medium findings, all accepted at the owner's request; the raw output was kept only in that session's scratchpad, so these summaries are the record.

1. R7's mutation criterion gave no per-test kill attribution, so two tests sharing their kills both read as deletable. **Accepted:** preservation of the caught set per batch (R7); principle 8 reworded.
2. §2.5 treated CPU, GPU and compositor time as additive although they overlap. **Accepted:** the measured timeline, one throughput budget per resource, waits as pacing, latency separate (§2.5).
3. R3 ran `cargo-semver-checks` before the version bump it validates, so the tagged tree wasn't the checked tree. **Accepted:** apply, validate the same tree, then commit and tag it (R3).
4. R3's commit-derived bump didn't cover the RC transitions. **Accepted:** explicit kinds with a commit floor, a frozen RC snapshot and an allowlisted promotion, all rehearsed (R3).
5. R6 retired docs on agent reads alone, which miss human and downstream readers. **Accepted:** signals only order the review; retirement needs a purpose-and-consumer check and records where live content went (R6).
6. §2.1 classed every format change as breaking-later while R2 allows migrated formats. **Accepted:** breaking-later only if existing files stop loading or change meaning; a format-versioning lever row added.

Round 2, 2026-10-16: artifact-only, xhigh, focus note on the round-1 changes; 5h window 47% → 54%. Four medium findings, all accepted at the owner's request:

1. Both archival options added files under `docs/history/`, which `promote`'s allowlist doesn't admit. **Accepted:** (a) copies before the last RC; (b) deletes everything behind the `legacy-records` tag (R8).
2. `promote` checked its allowlist in preflight, before the bump and the `Cargo.lock` refresh produced the final diff. **Accepted:** the diff rules run on the final diff; the lockfile rule (R3).
3. No route to a later major: `major` went straight to stable and `rc` only defined 0.x → `1.0.0-rc.1`; the rehearsal skipped `minor` and `major`. **Accepted:** no direct `major` kind; every major is an RC series frozen at `rc.1`; every kind and refusal rehearsed (R3).
4. §2.5 covered frame budgets only. **Accepted:** frame, operation and scaling categories; each row names its category and Q12 picks which gate 1.0 (R-Q13).

Round 3, 2026-10-16: `--repo` at xhigh, the first round checked against the repository: 513 s, 1,088,055 tokens in (993,280 cached), 24 commands, 268 KB output, 5h window 0% → 35%. Six findings, one high, all verified at `0732305` and accepted:

1. [high] `just api-check` proves a snapshot fresh, not compatible (`scripts/public-api.sh:63`), so a `patch` or `minor` could ship a break `cargo-semver-checks` misses. **Accepted:** the kinds table diffs the promised snapshots against the last tag's (R3, R2).
2. A patch ran the script alone, though `just check` runs debug tests, the determinism tests are ignored there (`justfile:29`) and the validation matrix selects by changed code. **Accepted:** the matrix's selection for the diff since the last tag is the floor; the kind adds ceremony (R3).
3. R-Q12's pinned game revision would measure the engine tag the game depends on (`acceptance-game.md:14`), not the candidate. **Accepted:** built against the candidate through a dependency override, both identities recorded (R-Q12).
4. Additive-later deferred the only fix for a row over a gating budget, with no route after the freeze. **Accepted:** such a fix lands before the freeze (§2.1, §2.4).
5. §2.5's timeline needs a capture path that keeps the overlap: the timed frame blocks on `device.poll` (`timing.rs:161`, `renderer.rs:1404`). **Accepted:** a non-stalling timestamp readback is a runner requirement (§2.2, §2.5).
6. R-Q14's `D-108` pair fails the different-files rule: both steps write `benchmarks.md` (criteria §8.8). **Accepted:** one combined candidate (R-Q14, §1, §2.6).

Round 4, 2026-10-16: artifact-only at xhigh with a focus note on this pass's changes and round 3's dispositions; 388 s, 28,234 tokens in, 5h window 35% → 43%. Three medium findings, all accepted:

1. An additions-only snapshot diff can still break: a new required trait method, a variant or public field on a type without `#[non_exhaustive]`, a blanket impl. **Accepted:** additions are classified by that list in the `minor` rule (R3).
2. R3's `rc.1` had to match W4b's snapshot with no exception, while R7 allows an additive harness exception after the freeze. **Accepted:** accepted exceptions apply to that comparison too (R3).
3. `promote`'s allowlist omitted R8's three line removals. **Accepted:** named in the allowlist (R3).

## 6. Open questions, each with a default

The feature gate takes the default unless the owner answers first (`D-150`).

- **O2.** Does the freeze cover data formats? The freeze gate lists API items only. *Default: yes. The freeze gate's record names the formats `D-103` promises and pins one fixture file per format that every 1.x release must still load unchanged; W4b's sweep gives each a compatible version signal (§2.1).*
- **O4.** Does Q12 constrain the rows, or only the budgets? *Default: only the budgets. Q12 picks the gating categories, the targets, the hardware and the headroom; the rows come from the suite (§2.2) and C1 adds the acceptance-game and template rows.*
- **O5.** Does the arity-named query removal move from W4b into pass 1, if pass 1 touches queries anyway? *Default: no, it stays in W4b where the register and `D-133` put it; pass 1 removes an item early only when a pass-1 fix rewrites the same file.*
- **O7.** Is `tungsten-render`'s public API inside the freeze? *Default: no. `D-103` promises the umbrella's curated render re-exports, not the crate; `api/tungsten-render.txt` stays for diffs but gates nothing, and the levers' "render seam" entries matter only through the umbrella.*
- **R-Q4.** `ROADMAP.md` at the root or under `docs/`? *Default: the root, beside `README.md` and `AGENTS.md`, since it is the first thing a visitor and an agent look for; `AGENTS.md`'s read path names it.*
- **R-Q9.** How is the operating-model doc kept from regrowing the old process? *Default: every rule in it names the failure it prevents, in a clause; a rule without one is cut at the next minor's review, and anything a script does is a link to the script.*
- **R-Q10.** No 0.x releases between W4b and `v1.0.0-rc.1`: yes or no? *Default: yes, none (R3).*
- **R-Q11.** Retire the old records as an archive folder (a) or delete them and tag the tree (b)? *Default: (b), with `just history`.*
- **R-Q12.** The acceptance-game bench row: a pinned repository revision or a vendored snapshot? *Default: a pinned revision of the game's repository, cloned beside this checkout as `D-102` has it, built against the candidate checkout through a dependency override of its engine tag, with the game revision and the engine commit both recorded in the capture's provenance; the runner skips the row with a note when the clone is missing. A vendored snapshot would copy the game's scenes and assets into the engine tree and age there.*
- **R-Q13.** Which budget categories gate 1.0: frame only, or frame, operation and scaling (§2.5)? *Default: frame and operation budgets gate; scaling rows report the count where each frame budget breaks and gate nothing in 1.0, since their shapes have no history yet. This belongs with Q12.*
- **R-Q14.** Do the bench rewrite (7.1) and C1 (7.2) share one plan and release? *Default: yes, as one combined candidate, not a `D-108` pair (both write `benchmarks.md`, which `D-108`'s different-files rule forbids): C1 is 7.1's closing step, the suite's first full sitting with the budgets and the dated baseline, so 7.1 is done when C1's rows run on it. Part A then costs 3–4 releases.*
- **R-Q15.** Does `release.yml`'s final-tag trigger go back to a tag push at 7.6? *Default: yes (R3, R9): with pull requests optional, the merge can't be the publish step. The pull-request path is removed rather than kept beside it.*

**Answered during the review:**
- R-Q6: keep the API snapshot; `cargo-semver-checks` is an extra gate.
- R-Q8: 1.0 is the `promote` of the last RC after the sweep.
- R-Q5 and R-Q7: no reset work in Phase 6; each piece is designed just before its first use.

## 7. Adoption notes

**Settled by the owner; don't reopen without new evidence:** the [Owner answers](#owner-answers-2026-10-16) below. A finding against one of them is recorded in §6 as a question, not applied.

**Known context:**
- Nobody uses this doc until after the kit block, W12b, W6b/c, W14b and the Phase 6 QA pass; ordered step 3 is the recheck before adoption.
- The kit catalogue's graduation edited `docs/plans/1.0/` on 2026-10-16; the digest was rechecked against that working tree. M60 (W13e) then ran in the same tree, after the digest was taken; the recheck covers it.
- rtk compresses command output: `rtk proxy git …` for raw counts, Python for sizes. The transcript mining counted sessions by matching Read calls and Bash commands by path; repeat it that way at the recheck.

## Owner answers, 2026-10-16

- **Commits:** agents commit, in standard Conventional Commits format. Branching stays minimal.
- **Kit block:** stays on today's process, and the cutover is at the freeze. Phase 6 closes before any of this is considered.
- **Docs:** triage them; far fewer docs; size is a soft guideline.
- **Old records:** keep them until the final phase.
- **AI workflow:** an AI-driven dev pass. Every skill is rewritten after researching best practices. `critique` works well. `tungsten-next` gets leaner. Planning and milestones get a deep logical rework.
- **Releases:** a full optimization pass over the pipeline, scripts and skills.
- **Roadmap artifact site:** dropped for a simple status display, but only once that makes sense in Phase 7.
- **Versioning:** semver: 1.0.0 / 1.0.1 / 1.1.0 / 2.0.0.
- **Bench rewrite:** after the kit and every core system are finished, and the old benchmark data is purged then.
- **Framing:** the last waterfall, meant to get out of the waterfall loop.

## Session log

- **2026-10-16, first pass:** drafted Part A (perf before the freeze) and Part B (the soft reset) from the owner's answers.
- **2026-10-16, review pass:** checked the draft against the cited decisions, criteria §10, `schedule.rs`, `query.rs`, `release.yml` and the doc, tooling, test and branch inventories; researched the four tools, Anthropic's skill guidance and ARCHITECTURE.md; restructured around one Phase 7 order (§1).
- **2026-10-16, critique rounds 1 and 2** (artifact-only, xhigh): ten medium findings, all applied (§5).
- **2026-10-16, final review pass** (at `0732305`): cleared the unverified list, corrected `release.yml`'s trigger, the kit block and the figures (§4), pushed R3's and R7's mechanisms down to their steps' plans, marked time-sensitive facts with the commit, added the recheck step and a default for every question (§6), spread the skill rewrites over the steps (R4). Critique round 3 (`--repo`, six findings) and round 4 (three) followed; all nine were verified and applied (§5).

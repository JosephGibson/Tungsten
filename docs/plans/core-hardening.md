# Core hardening (7.21)

- **status:** in progress
- **goal:** core's physics and saves hold where the 2026-11-07 big session's track C found them failing: two seeded oracles land in core's tests, then each of the 23 findings it reproduced gets a fix, its repro test committed before the fix and its rule recorded. That is the 17 physics rows `docs/known-issues.md` held without a step, and the six rows the oracles found.
- **non-goals:** query.rs's refresh cost (`docs/known-issues.md`'s row for colliders built past the `rigid_body` caps, 7.14's C1); the physics rows that already have steps (the cast from a landed shape at 7.13; the grid clears and the island wakes' O(k·N) at 7.14); S-59's known mismatch (physics stepped after a load); the residuals each fix names in its row (the co-moving pair group, loaded columns sinking past the resting band, one-way case (b), a static box against a tile map, the ghost ledge beside a covered slope leg); a physics hash moved (`docs/plans/phase-7.md` §5); a perf sitting of its own (`D-226`): the minor release after 7.21 sits it (§2.8).
- **files to touch:** the union below; §3 gives each step's own.
  - core sources: `crates/tungsten-core/src/physics/broadphase.rs`, `crates/tungsten-core/src/physics/collision.rs`, `crates/tungsten-core/src/physics/components.rs`, `crates/tungsten-core/src/physics/free_spot.rs`, `crates/tungsten-core/src/physics/plugin.rs`, `crates/tungsten-core/src/physics/step/arrival.rs`, `crates/tungsten-core/src/physics/step/gather.rs`, `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/physics/step/outline.rs`, `crates/tungsten-core/src/physics/step/pairs.rs`, `crates/tungsten-core/src/physics/step/sleep.rs`, `crates/tungsten-core/src/physics/step/slope.rs`, `crates/tungsten-core/src/physics/step/solver.rs`, `crates/tungsten-core/src/physics/step/sweep.rs`, `crates/tungsten-core/src/component_registry.rs`, `crates/tungsten-core/src/ecs/mod.rs`, `crates/tungsten-core/src/save.rs`, `crates/tungsten-core/src/save/world.rs`, `crates/tungsten-core/src/save/registry.rs`;
  - core tests: `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/tests/physics/step_oracle.rs`, `crates/tungsten-core/src/tests/physics/free_spot.rs`, `crates/tungsten-core/src/tests/physics/outline.rs`, `crates/tungsten-core/src/tests/physics/plugin.rs`, `crates/tungsten-core/src/tests/physics/query.rs`, `crates/tungsten-core/src/tests/physics/slope.rs`, `crates/tungsten-core/src/tests/ecs/oracle.rs`, `crates/tungsten-core/src/tests/save/world.rs`, `crates/tungsten-core/src/tests/save/oracle.rs`. `crates/tungsten-core/src/tests/physics/step_oracle.rs`, `crates/tungsten-core/src/tests/ecs/oracle.rs` and `crates/tungsten-core/src/tests/save/oracle.rs` are new;
  - the workspace: `Cargo.toml` and `Cargo.lock` (Step 15), `api/tungsten-core.txt` (Step 14);
  - the pins a step's **Moves** names:
    - 02's guards: `examples/02_bench/src/tests/guards/physics.json` (Steps 3 and 18), `examples/02_bench/src/tests/guards/physics-sparse.json` (3), `examples/02_bench/src/tests/guards/slopes.json` (3 and 10) and `examples/02_bench/src/tests/guards/terrain.json` (12, 19 and 22);
    - 01's pit guard: `examples/01_platformer/tests/game/guards/pit.json` (Step 7);
    - another guard value only where a step without a prototype measures it (§3), or as §2.8 allows;
  - the save steps' watch lists, touched only to re-pin what their rules explain: `crates/tungsten-core/src/save/area.rs`, `crates/tungsten-core/src/tests/save/area.rs`, `crates/tungsten/src/tests/saves.rs`, `crates/tungsten-kit/src/tests/saved.rs`, `crates/tungsten-kit/src/overlap/hit.rs`, `crates/tungsten-kit/src/lifecycle/cooldown.rs`, `crates/tungsten-kit/src/lifecycle/spawner.rs`, `templates/basic/src/save.rs`, `templates/basic/src/sample/mod.rs`, `examples/02_bench/src/tests/guards/saves.json`;
  - records: `docs/known-issues.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `ROADMAP.md`, `docs/plans/phase-7.md`, this plan.
- **ordered steps:** (1) the oracles; (2) `ch-pair-box`; (3) `ch-sweep-net`; (4) `ch-resting-band`; (5) `saving-flips`; (6) `fresh-refs`; (7) `ch-kinematic-support`; (8) `ch-write-wake`; (9) `ch-island-tags`; (10) `ch-slope-clamp`; (11) `ch-one-way-join`; (12) `ch-arrival-wake`; (13) `oracle-crush`; (14) `ch-huge-mass`; (15) `float-parse`; (16) `ref-order`; (17) `ch-static-velocity`; (18) `ch-static-join`; (19) `ch-map-edges`; (20) `ch-gravity-scale-pairs`; (21) `ch-midair-sleep`; (22) `ch-tag-adoption`; (23) `ch-spin-angle`; (24) `oracle-thin-box`; (25) close. Matches §3.
- **done-when:** every step's done-when passes, quoted in the evidence log. Every repro test runs un-ignored and passes; a skipped step's test, or Step 7's fallback test, stays ignored with its reason. `just check` and `just physics-release` pass, with the determinism and containment hashes unchanged. The oracles' default runs and long modes pass with no entry left for a finding that landed. Example 01 plays its pit as before. The plan is deleted with its citations, and `core-hardening` is out of Now.

Item `core-hardening` in [ROADMAP.md](../../ROADMAP.md)'s Next. Its source is the 2026-11-07 big session's track C, run under the owner's brief. The reports are in `perf-runs/20261107-big-session/core/`; each finding's folder, with its `test.patch`, `fix.patch`, `notes.md` and logs, is in `perf-runs/20261107-core-hardening/`. Both are git-ignored and local to this machine. Written 2026-11-07 at `09631340`. It runs under the [tungsten-plan skill](../../.claude/skills/tungsten-plan/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs.

## Context digest

Track C exported `cad63fa5` and set eight finder agents on the physics rows of `docs/known-issues.md` that had no step, one agent per file group. Query.rs's refresh cost was left to 7.14's C1. For each row, a finder wrote a test that fails at `cad63fa5` for the stated reason, the smallest fix as a patch, the hashes and pins it moves, and a decision amendment's rule in one paragraph. Two oracle agents wrote seeded random tests:

- physics scenes checked each step against a brute-force pair set, with no static passed through, wakes and one hash;
- World and command-buffer sequences against a plain model, and restores against the live World.

The oracles found six rows of their own: a heavy body crushing a light one through a static, a thin box falling through a solid cell, and four save defects. A fresh-context verifier applied each patch alone on `cad63fa5` and confirmed the failure and the pass. It listed every workspace test each patch moves, 01's and the template's included, and the patches that touch the same lines (§2.5). It confirmed all seventeen. None of them moves the determinism hash `0x088e_c07a_73c1_b168`, the containment hash `0xaee2_72e0_1ffc_4e4c`, a size pin or a template test. 01 moves only in `ch-kinematic-support`, by its pit guard's allocation counts.

The plan stands on 7.8 (`D-228`, `D-229`), 7.9 (`D-230`) and 7.10m (`D-239`), and on `D-226`'s guards. It amends `D-064`, `D-065`, `D-066`, `D-080`, `D-081`, `D-092`, `D-153`, `D-154`, `D-156`, `D-160`, `D-203`, `D-228`, `D-229`, `D-230` and `D-239`, one new entry per step that changes a rule (§2.6). The later steps rely on it:

- 7.12's level format stands on the outline and one-way rules;
- 7.13 moves 01's saves onto `restore_world` (steps 4–6, 15 and 16);
- the minor release after 7.21 sits the rows whose per-pair cost the counter guards cannot see (§2.8).

## 1. Audit

Read at `09631340` (core unchanged since `cad63fa5` but for doc comments). Each row's evidence is its repro test's failure at `cad63fa5`, rerun by the verifier, and its finder's notes.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | Core's tests have no random-scene or random-sequence oracle. Each physics row was found by a hand-built probe. | `rg -l 'splitmix\|oracle' crates/tungsten-core/src/tests` → nothing at `cad63fa5` |
| A2 | `ch-pair-box` (P1, `known-issues.md:50`): a fast body's pair box grows with its speed on both axes, so the pair grid stages cells without bound. | `the pair grid staged [51076, 5071504] cell refs for one body` |
| A3 | `ch-sweep-net` (P1, `:51`): the safety net clamps a landing body's whole move, slide included, to `t − BACKOFF`, and every later substep undoes it again. | `20 of 20 runs froze on the face` |
| A4 | `ch-slope-clamp` (P2, `:52`): beside a masked slope leg, a circle takes the face's line where no neighbour continues it. | `at [82, 11.99]: Some(Contact { normal: Vec2(-0.70710677, -0.70710677), penetration: 2.5787153 }), true gap 0.48108244` |
| A5 | `ch-arrival-wake` (P2, `:53`): a hit on a body pressed on a sleeper is absorbed, and the sleeper stays asleep. | `297 of 320 hits left S asleep` |
| A6 | `ch-kinematic-support` (P2, `:54`): the support record leaves kinematic bodies out, so a sleeper on a still kinematic platform hangs when it goes. | `10 riders hang` |
| A7 | `ch-write-wake` (P2, `:55`): a write to a sleeper's shape, gravity scale or friction, or to its support's friction, wakes nothing. | `6 sleepers held` |
| A8 | `ch-resting-band` (P2, `:60`): `free_spot` moves a body the step rests on a solid by a quarter cell, so a restore moves it. | `rested 0.2564392 px deep`, `left: Vec2(40.0, 136.25644)` |
| A9 | `ch-island-tags` (P2, `:61`): a contact wake takes one body out of its island's tag, so removing it strands the sleepers it held. | `6 tops stranded` |
| A10 | `ch-one-way-join` (P2, `:62`): a body walking off one-way cells meets the solid cell's side beside them. | `off the one-way cells onto the solid: at [325.74283, 58.253197]` |
| A11 | `ch-gravity-scale-pairs` (P3, `:65`): one body's gravity scale lifts every pair box. | `left: (1, 57.25001)  right: (0, 1.5625001)` |
| A12 | `ch-midair-sleep` (P3, `:67`): a body with a small gravity scale falls asleep in mid-air. | `scale 0.04, toss 0: slept at (step, y) Some((30, 4.5374985))` |
| A13 | `ch-tag-adoption` (P3, `:14`, the adoption part): adoption reads only touching final-substep contacts. | `upper box 0.5024948 px over the lower one: (lower, upper) woke (true, false)` |
| A14 | `ch-map-edges` (P3, `:63`): two maps placed edge to edge keep their edge cells' outer faces. | `over the seam: at [154.25717, 26.253183], on one map at [220, 26.25295]` |
| A15 | `ch-static-velocity` (P3, `:58`): a static body's `Velocity` reaches its proxy. | `left: Vec2(0.0, -16.583334)`, `right: Vec2(0.0, -15.743673)` |
| A16 | `ch-static-join` (P3, `:13`): a body sliding over two touching static boxes meets the second box's side. | `over the join: at [58.257168, -13.746817], on one deck at [120, -13.743673]` |
| A17 | `ch-huge-mass` (P3, `:59`): a huge finite mass turns a contact's impulse to NaN. | `rigid_body: mass 1e36 at 1000 px/s: [NaN, NaN], [NaN, NaN] at step 1` |
| A18 | `ch-spin-angle` (P3, `:68`): a spinner's angle grows without bound in f32 and its turn rate decays. | `from 360000: turned 97.5 rad of 100, angle 360097.5` |
| A19 | `oracle-crush` (new row): a heavy body drives a light one through a static floor or wall. This contradicts the recorded limit "Nothing passes a static gate" (`D-092`). | `26 of 864 cases end below the floor`; 83 of 20,000 random seeds |
| A20 | `oracle-thin-box` (new row): a box narrower than half a cell, sunk past its side overlap in a solid cell, loses its floor and falls through. | `step 18: the thin box's centre [-229.51392, 185.156] passed the solid cell's bottom face (y 180.18094)` |
| A21 | `saving-flips` (new row): a restore inserts a `SaveState` into a World that named nothing, against `D-230`'s "a World where nothing was named holds no save state". | `the loaded run saves as the live one does`: `left: true`, `right: false` |
| A22 | `fresh-refs` (new row): for a named entity's component with no patch, a restore runs `RefCodec::load` on the fresh spawn's value, against its doc (`save/registry.rs:51-54`). | `the live World's one mark`: two entities where one was saved |
| A23 | `ref-order` (new row): a key whose references are split between a loaded area and a kept one comes back in another order, against `RefReader`'s "in their saved order" (`save/registry.rs:97-98`). | `left: ([3865623817, 3379584492], false)`, `right: ([3379584492, 3865623817], true)` |
| A24 | `float-parse` (new row): serde_json without `float_roundtrip` reads a 17-digit float one ULP off, so a slot read from text restores an f64 off and rewrites a kept area's bytes. | `176 of 2,000 pressures restored off`; `17 of 199 healths wrote other bytes` |

## 2. Design

### 2.1 Scope

There are 23 findings: the 17 rows and the oracles' six. Each fix leaves its residual as a row of its own (non-goals), and the step that removes the row writes that residual row. The slope finder's sibling, a ghost ledge where a full cell beside a covered slope leg clamps a circle to its top line, is a row with no step. The oracles' six joined 7.21 because they are core's and reproduce at HEAD (Q1).

### 2.2 Order

The order runs by band: the two P1s, then the P2s, then the P3s. Within a band, the change with the least to move goes first, so that a stopped riskier step strands no later patch. Patches that touch the same lines go in the order they stack (§2.5).

- **P2:** the order is
  1. the load-time and save fixes;
  2. the three sleep fixes, in the order their patches stack;
  3. the two fixes with pair-loop work (slope clamp, one-way join);
  4. the largest patch (arrival wake);
  5. the unprototyped crush rule.
- **P3:** the order is
  1. the load-time and save checks;
  2. the gather fixes, in their stack order;
  3. the gravity-scale allowance on Step 2's oversize path;
  4. the two sleep rules, in their stack order;
  5. the spin wrap, which moves a test;
  6. the unprototyped thin-box rule.

### 2.3 A finding step: test first, then fix

Each finding step makes two commits.

1. **The test commit** adds the repro test from Appendix A with `#[ignore = "7.21 step N: fails until its fix"]` on each new `#[test]`. `cargo test … -- --ignored` shows it failing for the stated reason, and `just check` stays green.
2. **The fix commit** does the following:
   - applies the fix and removes the `#[ignore]` lines;
   - removes the finding's oracle entry, if any (§2.7);
   - writes its rule through the [tungsten-decision skill](../../.claude/skills/tungsten-decision/SKILL.md), taking the next free ID at that moment, since another session can take one mid-run;
   - updates `DESIGN.md`'s physics or saves section where the rule changes what it says (lines 260–266 hold the contacts, kinds and masses at `09631340`);
   - removes its `docs/known-issues.md` row and writes the residual row, if any;
   - re-pins each moved test in §3's list with the value its failure prints;
   - adds the step's evidence row.

**Every finding step's done-when**, beside its own in §3. `<tests>` are the step's test names, as several filters after `--`.

- Test commit:
  - `cargo test -p tungsten-core --lib --locked -- --ignored <tests>` → exit 101, each panic line the stated one (quoted in the evidence row);
  - `cargo fmt --all -- --check` → exit 0;
  - `just lint` → exit 0.
- Fix commit:
  - `cargo test -p tungsten-core --lib --locked -- <tests>` → exit 0, each test passed;
  - `cargo test -p tungsten-core --locked` → exit 0;
  - `just physics-release` → exit 0, 16 passed, the two hashes held;
  - each oracle's long mode for the step's subsystem (§2.7) → exit 0;
  - `cargo test --workspace --locked --no-fail-fast` → exit 0. It runs 01's pit guard (`examples/01_platformer/tests/game/guards/pit.json`) and ball-pit containment, 02's guards (`examples/02_bench/src/tests/guards/*.json`), the template's tests and the kit's. The moved tests are the step's list, re-pinned, plus any §2.8 allows; any other moved test is a stop (§6);
  - the checks `just checks <the commit's paths>` names → pass;
  - `cargo fmt --all -- --check`, `just lint` and `just repo-check` → exit 0;
  - `rg -n '#\[ignore = "7.21 step N:' crates/tungsten-core` → exit 1.

The test commit's message is `test(physics): <finding>'s repro, ignored until its fix`, or `test(saves): …` for a save finding. The fix commit's is `fix(physics): <summary>`, or `fix(saves): …`.

### 2.4 Cost per step

A physics fix runs:

- the core suite: about 5 s after Step 1;
- `just physics-release`: about 1 min;
- the physics oracle's long mode in release: about 4 min;
- the workspace suite: about 6 min warm.

That is about 12 minutes of checks per step, before the build.

### 2.5 Patches and their overlaps

Each patch was cut from a fresh `cad63fa5` export and applies alone there. A step applies its patch with `patch -p1 --dry-run` first and then `patch -p1` (`git apply` is denied).

- An offset is fine.
- A fuzzed or rejected hunk means the patch is merged by hand within the step's files, along the merge its step names.
- A merge that needs a file outside the list is a stop.

The verifier's overlap table (`perf-runs/20261107-big-session/core/verify.md`) and the finders' notes give the merges:

| Steps | File | Lines both touch | Merge |
| --- | --- | --- | --- |
| 2, then 20 | `crates/tungsten-core/src/physics/step/pairs.rs` | `repair_pairs`' `StepExtra` destructure, one hunk; `trips_oversize` | `both-findings-combined.patch`: the destructure takes both fields, and `trips_oversize` reads the per-proxy lift |
| 3, then 11 | `crates/tungsten-core/src/physics/step/sweep.rs` | the safety net's candidate loop | 11's flagged-side drop sits beside 3's shallow-hit skip |
| 7, then 8 | `crates/tungsten-core/src/physics/step/sleep.rs` | `Support`, `support_order`, `diff_supports`, `record_supports` | a kinematic support compares kind, layers, Step 7's shape and friction |
| 8, then 9 | `crates/tungsten-core/src/physics/step/sleep.rs` | the `written` calls in `same_bodies` and `rebuild` | `held_or_written` takes the body bits |
| 18, then 19 | `crates/tungsten-core/src/physics/step/gather.rs` | the `FACE_*` import line | 19 drops its copy of the hunk |
| 21, then 22 | `crates/tungsten-core/src/physics/step/sleep.rs` | `sleep_frame_end`'s doc paragraph | one paragraph with both clauses |

The verifier's hunk check (`perf-runs/20261107-big-session/core/verify.md`) adds the pairs below. Each pair's hunks share context lines, each pair runs in the plan's order, and `patch` takes the later hunk at an offset or with fuzz. Where a hunk fuzzes or is rejected, the merge column says how to merge it by hand.

| Steps | File | Lines at `cad63fa5` | Merge |
| --- | --- | --- | --- |
| 1, then 12 | `crates/tungsten-core/src/physics/step/mod.rs` | the oracle's `after_substep` hook ends `substep` (1745–1750); 12's `two_sided` follows it (1747–1752) | 12's new function goes after `substep`'s closing brace |
| 1, then 20 | `crates/tungsten-core/src/physics/step/mod.rs` | the oracle's `after_refresh` hook (1256–1261); 20's `refresh_pairs` call two lines above it (1249–1256) | the hook stays after the `check_pair_contacts` block |
| 2, then 12 | `crates/tungsten-core/src/physics/step/arrival.rs`, `crates/tungsten-core/src/physics/step/mod.rs` | the arrival pass's parameters (197–204) and its call (1631–1638) | the pass takes 2's `oversize` after `repaired` and 12's `sleep` and `woken` after `arrival`, and the call passes all three |
| 7, then 9 | `crates/tungsten-core/src/physics/step/sleep.rs` | `Support`'s doc comment (93–105) | 9's `held_or_written` goes above 7's rewritten doc |
| 11, then 19 | `crates/tungsten-core/src/physics/step/outline.rs` | 202–212 | context only |
| 18, then 21 | `crates/tungsten-core/src/physics/step/mod.rs` | `StepExtra`'s last field (516–521) | both scratch fields: 18's `static_faces`, then 21's `contacted` |
| 18 and 19, then 23 | `crates/tungsten-core/src/physics/step/gather.rs` | the import lines (10–23) | 23's import goes beside the `FACE_*` line 18 rewrites |

Every repro test except `ch-sweep-net`'s appends at the end of `crates/tungsten-core/src/tests/physics/step.rs`, on the same three context lines (`-8177,3`), and `ch-pair-box`'s fix appends after its own test. Each step's test therefore goes after the previous step's, which is a plain concatenation.

Patches in one file at other lines need no merge: `patch` offsets them. The fixes that touch `crates/tungsten-core/src/physics/step/gather.rs` (Steps 17, 18, 19 and 23), `crates/tungsten-core/src/physics/step/mod.rs` (1, 2, 9, 12, 18, 20, 21 and 23), `crates/tungsten-core/src/physics/step/sleep.rs` (7, 8, 9, 21 and 22), `crates/tungsten-core/src/physics/components.rs` (14, 17 and 23) and `crates/tungsten-core/src/physics/step/arrival.rs` (2 and 12) were each cut alone.

### 2.6 Rules

A step whose fix changes a rule writes one new entry through the tungsten-decision skill, from the paragraph in its §3 entry. The entry amends the decisions named there, with their marker lines and index rows.

Two steps write no entry, because their code comes to meet text that already says what it should: `saving-flips` (`D-230`) and `fresh-refs` (`RefCodec::load`'s doc). `ref-order` writes one for the contract it states (Q5). `float-parse` writes one too: a dependency's feature that changes what a save's text reads back is a choice later work keeps.

The plan names no new ID (`D-NNN`), since the IDs are taken at run time.

### 2.7 The oracles

Step 1 lands both oracles from their patches.

- **Physics:** `tests/physics/step_oracle.rs`, a child module of the step, with a `cfg(test)` probe field and two `cfg(test)` hook calls in `physics/step/mod.rs`.
- **ECS and saves:** `tests/ecs/oracle.rs` and `tests/save/oracle.rs`, hooked by one `cfg(test)` module line each.

Neither adds a crate or public API, and neither builds outside core's own tests, so no 01, 02 or template pin can move.

**KNOWN and FOUND lists:**

- The physics oracle's `KNOWN` list has an entry and a witness scene for each of `ch-arrival-wake`, `ch-kinematic-support`, `ch-write-wake`, `ch-island-tags`, `ch-midair-sleep`, `ch-gravity-scale-pairs`, `ch-static-velocity`, `oracle-crush` and `oracle-thin-box`. The default run requires each witness to trip, so a fix step removes its entry, and the default seeds and the long mode then prove the fix.
- The save oracle's `FOUND` list gates `float-parse`, `fresh-refs`, `ref-order` and `saving-flips` out of the default run, each with its generator exclusion. A fix step removes the entry and the exclusion.
- `TUNGSTEN_ORACLE_FOUND=<id>` lets an entry back in before its fix.

**Default runs (Q2):** as written, the default runs add 20–30 s to core's lib tests in debug, against 1.1 s now (3.6 s for core's whole suite). The finders timed physics seeds 0..1000 with 9 witnesses at 25.95 s, ECS 2,000 seeds at 7.0 s and saves 4,000 at 2.1 s. The verifier, each oracle alone on a warm build, timed core's lib at 21.3 s with the physics oracle and 8.0 s with the ECS and save oracles. Step 1 shards each run into tests over contiguous seed ranges, so the test harness's threads share them: eight for physics with the witnesses as a test of their own, eight for the ECS, four for saves. Every seed stays in the default run.

**Long modes,** the fix steps' proof, all in release:

- physics: `ORACLE_SEEDS=0..20000 cargo test --release -p tungsten-core --lib --locked physics_oracle_holds_on_many_seeds -- --ignored`, about 247 s (the verifier's run: 242.6 s);
- saves: `cargo test --release -p tungsten-core --lib --locked a_restored_world_equals_the_live_one_and_captures_the_same_long -- --ignored`, about 52 s;
- ECS: `world_and_command_buffers_agree_with_the_model_long`, about 296 s, run in Step 1 and the close.

**Rare witnesses:**

- `witness_thin_box` is seed 13734's random scene, so an unrelated fix can stop it tripping. That step then reduces another seed with `ORACLE_ALL=1 ORACLE_SEEDS=0..20000 cargo test --release -p tungsten-core --lib --locked physics_oracle_reduce -- --ignored --nocapture`, since without `ORACLE_ALL` the reducer skips every labelled failure, this one included (§7, 1.4). It replaces the witness with a scene whose failure the classifier labels `oracle-thin-box`. That is the one change to an oracle a step may make for another finding.
- The ECS oracle's `each_known_row_is_still_open` reads the words of known-issues' F7 and gone-named rows. A records edit keeps those words.

### 2.8 Moves and the perf check

§3 lists the tests each fix moves, from the verifier's run of that patch alone on `cad63fa5` (`perf-runs/20261107-big-session/core/verify.md`). It ran `cargo test --workspace --locked --no-fail-fast` and `just physics-release` before and after each patch. All 17 patches held: each test fails before its fix for the stated reason and passes after, each patch reverses clean, and `just physics-release` passed 16 of 16 with every fix.

- When patches stack, a listed pin's old value can be an earlier step's. The step re-reads it from the failure and records old and new in its evidence row. Two or three steps move each of 02's `physics` (Steps 3 and 18), `slopes` (3 and 10) and `terrain` (12, 19 and 22). A step may also move a counter guard's value that an earlier step of this plan moved, even though the step's own list leaves it out, when its evidence row ties the move to that earlier change (a merged `Support`, say). A counter guard's value is a field of `examples/01_platformer/tests/game/guards/pit.json` or an `examples/02_bench/src/tests/guards/*.json` pin. The exception covers counters only (§7, 2.4). A behaviour pin that an earlier step re-pinned, such as the outline fixture's mask or the spinner's angle, moves again only through a step that changes its contract. Any other unlisted move is a stop (§6).
- In an export, tungsten-cli's `new::tests::the_embedded_template_is_the_tracked_one` fails, because it reads Git's index. The steps run in the tree, where it passes, so a step's workspace run expects no failure.
- 01's, 02's and the template's counter guards run in `just check` (`D-226`). They count allocations, events and moves, not time.
- Steps 2, 10, 11 and 12 add work per pair or per candidate. Steps 7–9 add per-sleeper work at frame start, and Step 18 adds a sort of the static boxes' faces per frame. Counter guards cannot see that cost (the gap `D-233` met: the contact record took 18 of `e01-pit`'s 42.7 ms p50 with no guard moving).

7.21 takes no sitting of its own (`D-226`). The minor release after it sits `e01-pit` (p50, p95, p99), `friction`, `physics`, `physics-sparse`, `slopes` and `terrain` against the release before it. `friction` is there because preflight's P8 lists it as a physics row, and Steps 7–9's support compares run in its piles. That sitting is 7.21's perf check, and a regressed row there is a 9.2 finding with these steps as its candidates. The release notes name the row digests the steps move. Steps 3, 10, 12, 18, 19 and 22 move 02's guards, so the `physics`, `physics-sparse`, `slopes` and `terrain` rows' digests can move.

### 2.9 Records

- **This plan's commit and its records commit:**
  - The records commit adds 7.21 to `docs/plans/phase-7.md`'s §2.8 order and §3, and `core-hardening` to ROADMAP.md's Next.
  - It sets each finding's next-work cell in `docs/known-issues.md` to its step.
  - It adds rows for the six oracle findings and the ghost ledge, and notes at the `D-092` recorded limit that `oracle-crush` contradicts it.
- **Step 1:** moves `core-hardening` from Next to Now and sets this plan's status to `in progress`.
- **Each fix step:** removes its row ("a fix removes its entry here in the same change").
- **The close:** writes phase-7.md's evidence row and takes the item out of Now.

## 3. Steps

Each finding's folder, which its patches name, is `perf-runs/20261107-core-hardening/<finding>/`. Each finding step also touches §2.3's records (`docs/known-issues.md`, the decision files for a rule, this plan's evidence log).

### Step 1: The oracles

- **Files:**
  - physics oracle: `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/tests/physics/step_oracle.rs` (new);
  - ECS and save oracle: `crates/tungsten-core/src/ecs/mod.rs`, `crates/tungsten-core/src/save.rs`, `crates/tungsten-core/src/tests/ecs/oracle.rs` (new), `crates/tungsten-core/src/tests/save/oracle.rs` (new);
  - records: `ROADMAP.md`, this plan.
- **Change:**
  1. Set this plan's `status: in progress` and move `core-hardening` from ROADMAP.md's Next to Now.
  2. Apply `perf-runs/20261107-core-hardening/oracle-physics/test.patch` (two files) and `oracle-ecs/test.patch` (four files), each dry-run first.
  3. Shard the three default runs (§2.7, Q2):
     - `physics_oracle_holds_on_the_default_seeds` becomes eight tests over `DEFAULT_SEEDS`' eighths, plus one test over the nine witnesses that asserts each `KNOWN` entry trips on its witness;
     - `world_and_command_buffers_agree_with_the_model` becomes eight tests over `SEEDS`' eighths;
     - `a_restored_world_equals_the_live_one_and_captures_the_same` becomes four tests over its quarters.
     The long modes, `physics_oracle_trace`, `physics_oracle_reduce`, `physics_oracle_crush_sweep` and the self-tests stay as written.
  4. Replace the physics `KNOWN` entries' `source: "known-issues line NN"` strings with the finding's ID. Each row names the ID in its next-work cell, while line numbers drift as rows go: the records commit after this plan's moved them, and every fix removes one.
- **Done-when:**
  - `cargo test -p tungsten-core --lib --locked oracle` → exit 0: the shards, witnesses and self-tests pass on HEAD.
  - `cargo test -p tungsten-core --locked`, warm, timed with bash's `EPOCHREALTIME` → exit 0, its lib tests under 8 s wall, against 1.1 s at base and 26.5 s unsharded. This is functional only.
  - §2.7's three long modes in release → exit 0, each with its seconds recorded.
  - `TUNGSTEN_ORACLE_FOUND=all cargo test -p tungsten-core --lib --locked a_restored_world_equals` → exit 101: the oracle still sees the four save rows.
  - `cargo test -p tungsten-core --lib --locked each_known_row_is_still_open` → exit 0.
  - `cargo test --workspace --locked --no-fail-fast` → exit 0, no test moved. `just preflight` lists every counter guard (its P2) because the step touches `crates/tungsten-core/src/ecs/mod.rs`, and the line it adds there is `cfg(test)`.
  - `cargo fmt --all -- --check`, `just lint` and the checks `just checks` names → pass.
- **Commits:**
  - `test(physics): a seeded random-scene oracle for the step`;
  - `test(core): seeded oracles for the World, command buffers and saves`.
- **Moves:** none: both oracles build only in core's own tests (`cfg(test)`), so 01's, 02's and the template's builds are unchanged. The verifier applied each oracle's patch alone on `cad63fa5`, and core's lib passed: 1,473 tests with the physics oracle, 1,476 with the ECS and save oracles (§2.7).

### Step 2: `ch-pair-box` (P1)

- **Finding:** `docs/known-issues.md`'s P1 row on `awake_pair_radius`, `build_pairs` and `repair_pairs`. It amends `D-081` (build, repair) and `D-080` (staging). The repro is A.1: `a_fast_bodys_pair_staging_stays_bounded`, with a test-only `SpatialGrid::cell_refs` in `crates/tungsten-core/src/physics/broadphase.rs`.
- **Files:** `crates/tungsten-core/src/physics/broadphase.rs`, `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/physics/step/pairs.rs`, `crates/tungsten-core/src/physics/step/arrival.rs`, `crates/tungsten-core/src/physics/step/mod.rs`, and the moved pins below.
- **Change:**
  - Apply `ch-pair-box/fix.patch`, which also edits `crates/tungsten-core/src/tests/physics/step.rs`.
  - Add a test where a body the solve changes reaches an oversize neighbour in the arrival pass. With every core test passing even with the arrival pass's oversize candidates removed, nothing covers that line. Its negative control is that line removed, which fails the test.
  - Rule: a moving proxy whose pair inflation `r_i` exceeds 64 pair-grid cells (2,048 px at the default 32 px cell) is oversize.
    - The build stages no cell for it. It finds its pairs in one pass over every proxy's pair bounds, in proxy order, by the grid's overlap and layer tests. An awake pair is stored (lower, higher), and a pair of two oversize proxies once.
    - A repair's members test the oversize proxies beside their grid walks. A trip of an oversize proxy, or a member whose fresh `r_i` passes the cap, rebuilds.
    - The arrival pass takes oversize proxies as candidates.
    - The pair set is the grid's, so `D-081`'s completeness and the contact set stand. Only a frame that lists an oversize proxy orders its pairs differently. No box stages more than its shape grown by 64 cells a side, whatever the speed, gravity scale or step.
  - Residual row: a co-moving group still pairs everything, bounded by N² (8,128 pairs against 442 at rest for 16 × 8 circles at 18,000 px/s). The row's swept box (`ch-pair-box/swept-box-prototype.patch`) breaks four equivalence oracles. A linear box needs a `D-064` admission amendment and new oracles, and it moves every hash.
- **Done-when:**
  - §2.3's checks;
  - the test commit's line: `the pair grid staged [51076, 5071504] cell refs for one body`;
  - the arrival test and its negative control, as above;
  - the physics long mode.
- **Commits:**
  - `test(physics): a fast body's pair staging, ignored until its fix`;
  - `fix(physics): an oversize path for pair boxes past 64 cells`.
- **Moves:** none (§2.8). The workspace passes 4,662: the baseline's 4,659 and the three new tests (core's lib 1,468 → 1,471). `just physics-release` passes 16 of 16, and nothing in 01 or the template moves. No workspace body is oversize (64 cells of inflation).
- **Perf:** a flag test per build querier, `trips_oversize` per repair and an empty extend per arrival body (§2.8's release check).

### Step 3: `ch-sweep-net` (P1)

- **Finding:** the P1 row on `speculative_pass`. It amends `D-064`, the safety net only. The repro is A.2: `sweep_net_lets_a_fast_body_slide_along_a_face_it_lands_on`, 16 floor runs and 4 ramp runs.
- **Files:** `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/physics/step/sweep.rs`, and the moved pins below.
- **Change:**
  - Apply `ch-sweep-net/fix.patch`, 19 lines in `sweep.rs`.
  - Rule: a sweep hit whose move ends within `4 · linear_slop` past the face it enters, and within the body's smallest half extent, is no hit. A one-way top's end must also pass its contact admission's test (`bottom ≤ top + 4 · linear_slop`). The next substep's contact then holds the body, instead of the clamp undoing its slide. A deeper hit still clamps the whole move to its time less `BACKOFF`. Speculative contacts, the substeps and the masks stand.
- **Done-when:**
  - §2.3's checks;
  - the test commit's line: `20 of 20 runs froze on the face`;
  - the physics long mode.
- **Commits:**
  - `test(physics): a fast body landing on a face, ignored until its fix`;
  - `fix(physics): the safety net skips a hit that ends within the slop`.
- **Moves:** 02's guards (§2.8); nothing in core, 01 or the template:
  - `examples/02_bench/src/tests/guards/physics.json`: `allocated_bytes` 12,376,832 → 12,380,160, `events_sent` 30,029 → 31,371;
  - `physics-sparse.json`: `events_sent` 4,894 → 4,866;
  - `slopes.json`: `allocations` 3,087 → 3,130, `allocated_bytes` 3,525,936 → 3,526,668, `events_sent` 43,703 → 46,180.

  01's pit guard and its containment test, the template's tests and the kit's hold.
- **Perf:** the new code runs per sweep hit, for fast bodies only. More touching contacts send narrow-phase events (02's `physics` +4.5 %, `slopes` +5.7 % `events_sent`), so their digests move (§2.8).

### Step 4: `ch-resting-band` (P2)

- **Finding:** the P2 row on `free_spot` and `restore_world`. It amends `D-230`, the free spot only, at `D-064`'s threshold. The repro is A.3:
  - `a_body_the_step_rests_on_a_solid_stays` (`crates/tungsten-core/src/tests/physics/free_spot.rs`);
  - `a_body_resting_on_a_floor_comes_back_where_it_was_saved` (`crates/tungsten-core/src/tests/save/world.rs`).
- **Files:** `crates/tungsten-core/src/tests/physics/free_spot.rs`, `crates/tungsten-core/src/tests/save/world.rs`, `crates/tungsten-core/src/physics/free_spot.rs`, and the moved pins below.
- **Change:**
  - Apply `ch-resting-band/fix.patch` (`REST_SLOPS = 4.0`).
  - Rule: a body stays where it is when it overlaps no solid by more than four `linear_slop`s. That is `D-064`'s admission slack, 1 px at the default: the resting band the soft contacts hold a body in against a floor, a static body or a wall (0.250–0.900 px measured for a lone body). A body past it goes to the nearest ring point that overlaps no solid past one `linear_slop`, with the search, its order and its reach unchanged.
  - Residual row: loaded columns sink past the band and still move a quarter cell into the bodies above. For 8 crates at g 3,600 and 1/30 s, 1.059 px; for 32, 3.358 px; in a 256-body bin, 8 bodies past 1 px (`ch-resting-band/probe-columns-piles.log`).
- **Done-when:**
  - §2.3's checks;
  - the test commit's lines: `rested 0.2564392 px deep` and `rests 0.256439 px deep, free_spot Some(Vec2(104.0, 152.25644)) from Vec2(104.0, 156.25644)`;
  - no oracle reaches `free_spot` (§2.7), so no long mode runs.
- **Commits:**
  - `test(saves): a resting body's restore, ignored until its fix`;
  - `fix(physics): free_spot keeps a body within the resting band`.
- **Moves:** none (§2.8). The workspace passes 4,661 (core's lib 1,468 → 1,470, the two new tests). 01 restores no world until 7.13, and the template's continue tests compare exactly, which the rule's monotone change keeps.

### Step 5: `saving-flips` (P2, new row)

- **Finding:** a restore inserts a `SaveState` into a World that named nothing (`crates/tungsten-core/src/save/world.rs`'s `apply`, `:930-932`; the counter at `:559-562`). `save::saving` then turns true, and the kit's cooldown and spawner passes sort by `order_key` where the live run kept the query's order. No entry: the fix meets `D-230`'s "a World where nothing was named holds no save state". The repro is A.4: `oracle_new_4_a_restore_leaves_a_world_that_names_nothing_unsaving`.
- **Files:** `crates/tungsten-core/src/tests/save/world.rs`, `crates/tungsten-core/src/save/world.rs`, `crates/tungsten-core/src/tests/save/oracle.rs`, and any pins the workspace run moves. Its watch list, from `just preflight`: the template's `templates/basic/src/sample/mod.rs` (`restore_world` at `:193`, `remove_resource::<SaveState>()` at `:380`), the umbrella's `crates/tungsten/src/tests/saves.rs`, and the kit's `crates/tungsten-kit/src/tests/saved.rs`, `crates/tungsten-kit/src/lifecycle/cooldown.rs` and `crates/tungsten-kit/src/lifecycle/spawner.rs`. `crates/tungsten-core/src/save/id.rs:6` holds the sentence the fix comes to meet ("A World where nothing was named holds no `SaveState`"), unedited. Preflight also lists the count pins that the watched files feed:
  - the kit's `crates/tungsten-kit/src/tests/describe.rs`, `crates/tungsten-kit/src/tests/inspect.rs` and `crates/tungsten-kit/src/tests/tags.rs`;
  - the umbrella's `crates/tungsten/src/tests/kit.rs`, `crates/tungsten/src/tests/kit_recipes/coverage.rs` and `crates/tungsten/src/tests/kit_recipes/determinism.rs`;
  - the template's `templates/basic/tests/game/sample.rs` (its schedule snapshot) and `templates/basic/AGENTS.md:20`, whose save paragraph names `restore_world`.

  These move only if a watched file's systems change, which the rule does not explain, so that is a stop (§6).
- **Change:**
  - Not prototyped; the oracle's direction. `restore_world` inserts a `SaveState` only when the save names something: a named record, a gone name, a spawn, a kept area, or a counter above 0.
  - Remove the save oracle's `saving-flips` entry and its exclusion, so a level may name no entity.
- **Done-when:**
  - §2.3's checks;
  - the test commit's line: `the loaded run saves as the live one does` (`left: true`, `right: false`);
  - the save long mode;
  - `cargo test -p tungsten-kit --locked` → exit 0: its cooldown and spawner tests, which sort while saving.
- **Commits:**
  - `test(saves): a restore into a World that names nothing, ignored until its fix`;
  - `fix(saves): a restore names no save state when the save names nothing`.
- **Moves:** measured here, since there is no prototype. A moved test is re-pinned only when its change follows from the rule, and the evidence row says how. Otherwise it is a stop.

### Step 6: `fresh-refs` (P2, new row)

- **Finding:** for a named entity's component with no patch, a restore runs `RefCodec::load` on the fresh spawn's value (`plan_area`, `:738-754`; `Pass::resolve`, `:1123`). A list codec that extends doubles the level's references, and a single reference waiting on a kept area keeps the fresh target. No entry: the fix meets `RefCodec::load`'s doc (`crates/tungsten-core/src/save/registry.rs:51-54`). The repro is A.5:
  - `oracle_new_2_a_named_entitys_list_loads_onto_its_saved_form`;
  - `oracle_new_2_a_reference_waiting_on_a_kept_area_leaves_no_fresh_one`.
  Its helpers `Marks`, `world_with_marks` and `hero_dog` land here; Step 16's test uses them.
- **Files:** `crates/tungsten-core/src/tests/save/world.rs`, `crates/tungsten-core/src/save/world.rs`, `crates/tungsten-core/src/tests/save/oracle.rs`, and any pins the workspace run moves. Its watch list, as Step 5's, with the kit's `crates/tungsten-kit/src/overlap/hit.rs` codec and `crates/tungsten-core/src/tests/save/area.rs`'s `Hits`.
- **Change:**
  - Not prototyped; the oracle's direction. Before `load`, the restore puts each named component that has references and no patch from its own saved form: the fresh value round-tripped through the form. `load` then starts from the form's placeholders, as a patched component's does.
  - Remove the oracle's `fresh-refs` entry and its exclusion: reference changes no longer count only in the saved form, and level hit lists need not start empty.
- **Done-when:**
  - §2.3's checks;
  - the test commit's lines: `the live World's one mark` (left two entities) and `the hero chases the fresh dog: true`;
  - the save long mode;
  - `cargo test -p tungsten-kit --locked` → exit 0: the kit's `Hitbox` codec, which skips a target it holds.
- **Commits:**
  - `test(saves): a named entity's references at a restore, ignored until its fix`;
  - `fix(saves): load a named entity's references onto its saved form`.
- **Moves:** as Step 5.

### Step 7: `ch-kinematic-support` (P2)

- **Finding:** the P2 row on `is_support` and the support record. It amends `D-156`. The repro is A.6: `a_sleeper_on_a_still_kinematic_platform_falls_when_it_goes`.
- **Files:** `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/physics/step/sleep.rs`, `crates/tungsten-core/src/tests/physics/step_oracle.rs`, and the moved pins below.
- **Change:**
  - The test commit adds a sixth case to A.6, `"flattened"`: the still platform's `Collider` set to a quarter of its height, the oracle's `Op::Reshape` of a deck. It is the part of the oracle's `ch-kinematic-support` entry the patch leaves, since the patch's `changed` compares only a kinematic support's kind and layers (§7, 1.2).
  - Apply `ch-kinematic-support/fix.patch`, then extend it. `Support` keeps a kinematic body's collider shape bits (its half extents or radius, and its offset), and `changed` compares kind, layers and that shape, never the box, which moves. `Support` grows from 32 to 40 bytes in the patch, and the shape adds to that.
  - Remove the oracle's `ch-kinematic-support` entry. Its 4,612 skipped cases include the flattened decks, so its default run proves the extension.
  - Rule: kinematic bodies join the support record by key, kind, layers and shape. Their last box is kept but not compared. One that goes, or changes kind, layers or shape, wakes the sleepers within the slack of its last box. Its moves stay with `D-154`'s contact wake.
  - If the shape compare cannot land, the step splits instead, with acceptance of its own (§7, 2.5):
    - the `flattened` case moves to its own test, `a_sleeper_on_a_kinematic_deck_flattened_under_it_falls`, marked `#[ignore = "7.21 step 7: ch-kinematic-reshape, its residual row"]`. Under `--ignored` it fails for its stated reason;
    - the oracle's `ch-kinematic-support` entry gives way to a `ch-kinematic-reshape` entry, with a witness: a box asleep on a still platform flattened at step 60. That is the one other `KNOWN` change §5 allows;
    - a residual row records the flattened decks;
    - the done-when then holds with the five finder cases un-ignored and passing, the flattened test ignored, and `rg -n '#\[ignore = "7.21 step 7:' crates/tungsten-core` printing that one line instead of exiting 1.
- **Done-when:**
  - §2.3's checks;
  - the test commit's line: `riders hang`, listing the five finder cases and `flattened`;
  - the physics long mode with the entry removed.
- **Commits:**
  - `test(physics): a sleeper on a kinematic deck that goes, ignored until its fix`;
  - `fix(physics): kinematic bodies in the support record`.
- **Moves:** 01's pit guard, `guards::the_full_pit_holds_its_counts` (`examples/01_platformer/tests/game/guards/pit.json`): `allocations` 59,977 → 59,980 and `allocated_bytes` 68,096,156 → 68,910,940 (+814,784, as `Support` grows 32 → 40 bytes). Nothing moves in core, 02 or the template (§2.8). The shape this step adds to `Support` grows it again, so the pit's new values are re-read from its failure.

### Step 8: `ch-write-wake` (P2)

- **Finding:** the P2 row on `written` and `Support`. It amends `D-065` (wake path 2) and `D-156`. The repro is A.7: `a_write_to_a_sleeper_or_its_support_wakes_it`.
- **Files:** `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/physics/step/sleep.rs`, `crates/tungsten-core/src/tests/physics/step_oracle.rs`, and the moved pins below.
- **Change:**
  - Apply `ch-write-wake/fix.patch` on Step 7: `SleepEntry` grows from 32 to 56 bytes. The merge is the finders', with Step 7's shape: a kinematic support compares kind, layers, shape and friction, in `Support`, `support_order`, `diff_supports` and `record_supports`.
  - Remove the oracle's `ch-write-wake` entry.
  - Rule: a sleeper's shape, gravity scale and friction are frozen bit for bit, and so is a support's friction (a tile's from its material). A mismatch wakes. Restitution, rolling and mass stay unwatched; `wake` is the documented path.
- **Done-when:**
  - §2.3's checks;
  - the test commit's line: `6 sleepers held`;
  - the physics long mode.
- **Commits:**
  - `test(physics): writes to a sleeper and its support, ignored until its fix`;
  - `fix(physics): a write to a sleeper or its support wakes it`.
- **Moves:** none (§2.8). `SleepEntry` grows 32 → 56 bytes, and no guard's `allocated_bytes` moved. Nothing moves in 01, 02 or the template.

### Step 9: `ch-island-tags` (P2)

- **Finding:** the P2 row on `rebuild`, island tags, `wake` and `teleport`. It amends `D-065`. The repro is A.8: `removing_a_contact_woken_support_wakes_the_sleepers_it_held`.
- **Files:** `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/physics/step/sleep.rs`, `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/tests/physics/step_oracle.rs`, and the moved pins below.
- **Change:**
  - Apply `ch-island-tags/fix.patch` on Step 8. The merge is the finders': `held_or_written` takes the body bits, in `same_bodies` and `rebuild`'s `written` calls.
  - Remove the oracle's `ch-island-tags` entry.
  - Rule: a contact-woken body keeps its island's tag until the island wakes. Its removal, `wake`, `teleport`, `apply_impulse`, or a change to its layers or mask, wakes the sleepers holding the tag. Island wakes clear the tag, and merges retag it.
- **Done-when:**
  - §2.3's checks;
  - the test commit's line: `6 tops stranded`;
  - the physics long mode.
- **Commits:**
  - `test(physics): a contact-woken support removed, ignored until its fix`;
  - `fix(physics): a contact-woken body keeps its island's tag`.
- **Moves:** none (§2.8). Island wakes widen, but 01's pit guard held. Nothing moves in 02 or the template.

### Step 10: `ch-slope-clamp` (P2)

- **Finding:** the P2 row on `circle_contact`'s clamp arms. It amends `D-229`. The repro is A.9:
  - `a_circle_beside_a_masked_leg_reads_no_false_overlap` (`crates/tungsten-core/src/tests/physics/slope.rs`);
  - `a_ball_by_a_ramp_top_or_a_peak_meets_no_false_overlap` (`crates/tungsten-core/src/tests/physics/step.rs`).
- **Files:** `crates/tungsten-core/src/tests/physics/slope.rs`, `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/physics/step/slope.rs`, and the moved pins below.
- **Change:**
  - Apply `ch-slope-clamp/fix.patch`. It moves two region-oracle expectations in `crates/tungsten-core/src/tests/physics/slope.rs`: `slope_contacts_separate_and_measure_exactly` now wants `Masked`, and `slope_contacts_clamp_at_a_masked_endpoint` wants no clamp at open corners.
  - Rule: a circle takes the face's line beside a masked leg only where a neighbour continues the face through the vertex. Where the side between the normals, one without a leg, is exposed by the shadow rule, the vertex is an open corner. There a circle meets the rounded vertex on the face's side of the leg's line and nothing past it, in contacts and casts alike. Boxes, sweeps and rays are unchanged.
- **Done-when:**
  - §2.3's checks;
  - the test commit's lines: `at [82, 11.99]: Some(Contact { normal: Vec2(-0.70710677, -0.70710677), penetration: 2.5787153 }), true gap 0.48108244` and `Circle { radius: 4.0 } from [82, 11.99]: at [-38.236202, 76.42744], not past x 80`;
  - the physics long mode.
- **Commits:**
  - `test(physics): a circle beside a masked slope leg, ignored until its fix`;
  - `fix(physics): an open slope corner rounds a circle's contact`.
- **Moves** (§2.8):
  - two region-oracle expectations in `crates/tungsten-core/src/tests/physics/slope.rs`, which the patch itself edits;
  - 02's `slopes` guard (`examples/02_bench/src/tests/guards/slopes.json`): `allocations` 3,087 → 3,221, `allocated_bytes` 3,525,936 → 3,528,792 and `events_sent` 43,703 → 32,956 (−25 %, the false contacts gone). The old values are `cad63fa5`'s; once Step 3 is in, they are Step 3's.

  Nothing moves in 01 or the template.
- **Perf:** O(1) in `circle_contact`'s vertex arm, for circle–slope pairs only. 02's `slopes` rows (circles over diamonds) move flow, timing and digest (§2.8).

### Step 11: `ch-one-way-join` (P2)

- **Finding:** the P2 row on one-way masks. It amends `D-228` and `D-064`'s seam masks. The repro is A.10: `a_box_walks_off_one_way_cells_onto_the_solid_beside_them`.
- **Files:** `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/physics/collision.rs`, `crates/tungsten-core/src/physics/step/outline.rs`, `crates/tungsten-core/src/physics/step/solver.rs`, `crates/tungsten-core/src/physics/step/sweep.rs`, `crates/tungsten-core/src/tests/physics/outline.rs`, and the moved pins below.
- **Change:**
  - Apply `ch-one-way-join/fix.patch` on Step 3. Its `sweep.rs` hunk meets Step 3's candidate loop (§2.5).
  - Rule: a full or slope cell's left or right side (a full side or a slope's leg) that a one-way neighbour in its row alone exposes is flagged in the face mask's free bits (bits 5 and 6). The narrow phase (contact build, arrival pass) and the safety-net sweep drop a flagged side while the body's bottom lies at most a quarter cell below the cell's top: the one-way's acceptance, where the one-way holds it. A body deeper in the row, jumping up through the one-way, still meets the side, and casts and runs keep meeting it. A quarter cell holds for new and held contacts alike, so a resting body sinks up to 1.1 px, past 4 · `linear_slop`.
  - Residual row: one-way case (b), a ramp rising from a one-way bridge (it stops at x 314.25, and needs bit 7 and the shadow rule), and casts along a one-way join.
- **Done-when:**
  - §2.3's checks;
  - the test commit's line: `off the one-way cells onto the solid: at [325.74283, 58.253197]`;
  - the physics long mode.
- **Commits:**
  - `test(physics): a box walking off one-way cells onto solid, ignored until its fix`;
  - `fix(physics): a side a one-way neighbour alone exposes lets a walker on`.
- **Moves:** `outline_fixtures_trace_their_runs` (`crates/tungsten-core/src/tests/physics/outline.rs`, "one-way cells beside a wall"): wall cell (4, 1)'s run mask 12 → 44, re-pinned in this step, since the patch leaves it (§2.8). Nothing moves in 01, 02 or the template.
- **Perf:** one mask test per pair in the contact build and the arrival pass, and per candidate in the safety net (§2.8).

### Step 12: `ch-arrival-wake` (P2)

- **Finding:** the P2 row on `arrival_pass` and the contact wake. It amends `D-065` and `D-092`. The repro is A.11: `a_hit_on_a_body_pressed_on_a_sleeper_wakes_the_sleeper`.
- **Files:** `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/physics/step/arrival.rs`, `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/tests/physics/step_oracle.rs`, and the moved pins below.
- **Change:**
  - Apply `ch-arrival-wake/fix.patch` (273 lines). It goes on Step 2's `arrival.rs` and `mod.rs`, and on Step 9's `mod.rs` (§2.5).
  - Its woken-later part is needed: `fix-without-presolve.patch` still fails 13 of the 320 cases at 3,000 px/s.
  - Remove the oracle's `ch-arrival-wake` entry.
  - Rule: a sleeper wakes wherever the step first sees the hit.
    - After the solve, it wakes when its contact took more than `sleep_threshold` out of the awake side beyond the warm start. It takes the reaction impulses, and its contacts turn two-sided.
    - In the arrival pass, it wakes when a clamp takes more than `sleep_threshold` of closing speed against it, and it gives way by inverse mass.
    - A sleeper woken later in the narrow phase counts its mass in the earlier contacts.
    - Slower contacts leave it frozen, as `D-065`'s locality wants.
- **Done-when:**
  - §2.3's checks;
  - the test commit's line: `297 of 320 hits left S asleep`;
  - the physics long mode;
  - the review's `qa_a3_1_probe`, if the step reruns it, strands no case (2,664 of 4,320 at `cad63fa5`).
- **Commits:**
  - `test(physics): a hit on a body pressed on a sleeper, ignored until its fix`;
  - `fix(physics): a hit through a pressed body wakes the sleeper`.
- **Moves:** 02's `terrain` guard (`examples/02_bench/src/tests/guards/terrain.json`): `allocations` 21,377 → 21,378, `allocated_bytes` 2,563,820 → 2,564,076 and `events_sent` 7,926 → 7,967. Nothing moves in core, 01 or the template (§2.8). `Proxy` stays 80 bytes, and 01's pit guard held. 01's pit and its wake wave are the perf risk (§2.8).
- **Perf:** one test per narrow-phase contact and per closing arrival pair, two walks of the sleeper-contact list per substep, and a larger wake wave (01's pit tail, §2.8).

### Step 13: `oracle-crush` (P2, new row)

- **Finding:** a light dynamic body resting on or pressed against a static box is pushed through it by a much heavier body, directly or through a chain. The soft contact gives way under the pusher's momentum. Once the light body's centre passes the static's mid-plane, the contact's normal flips, and `D-092`'s arrival pass, which clamps the pushed body's move, does not stop the next substeps' pressing. The case contradicts the `D-092` recorded limit in `docs/known-issues.md`. It amends `D-092`, and `D-063` if the rule lands in the solver (Q3).
  The repro is A.12: `oracle_new_1_a_heavy_body_never_drives_a_light_one_through_a_static_floor`, 864 cases. The oracle's ignored `physics_oracle_crush_sweep` is the same sweep, and `witness_crush` is its 300×, 1,000 px/s, 8 px, 1/30 s case.
- **Files:** `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/tests/physics/step_oracle.rs`, the rule's files (`crates/tungsten-core/src/physics/step/arrival.rs` by default), and the moved pins.
- **Change:**
  - Not prototyped. The step prototypes in a `git archive HEAD` export under `target/` first.
  - Default rule (Q3): the arrival pass stops a dynamic body's centre short of a static box's or a solid cell's mid-plane along the contact's normal. A pushed body then never reaches the depth where the normal flips. The alternatives are the oracle's: a stiffer softness on a contact whose other side a heavier partner presses, or the recorded limit's text amended to name the cases that pass.
  - The rule lands only if all of these hold:
    - A.12's 864 cases pass;
    - the witness passes once its entry is removed;
    - the physics long mode passes;
    - `just physics-release` holds both hashes;
    - no 01 or template pin moves for a reason the evidence row cannot give.
    Otherwise the step stops (§6), and the row keeps the prototype's evidence as its next work.
  - On landing, amend the recorded limit's text.
- **Done-when:**
  - §2.3's checks;
  - the test commit's line: `26 of 864 cases end below the floor`;
  - the physics long mode with `oracle-crush` removed.
- **Commits:**
  - `test(physics): a heavy body crushing a light one through a static, ignored until its fix`;
  - `fix(physics): a pushed body stops short of a static's mid-plane`.
- **Moves:** measured here, as Step 5.

### Step 14: `ch-huge-mass` (P3)

- **Finding:** the P3 row on `with_mass`, the `rigid_body` loader and the solver. It amends `D-153` (`rigid_body.mass`) and `D-239` (`with_mass`, the kinematic reaction mass). The repro is A.13: `a_huge_mass_is_refused_at_both_doors_or_stays_finite` (`crates/tungsten-core/src/tests/physics/plugin.rs`).
- **Files:** `crates/tungsten-core/src/tests/physics/plugin.rs`, `crates/tungsten-core/src/physics/components.rs`, `crates/tungsten-core/src/physics/plugin.rs`, `crates/tungsten-core/src/component_registry.rs`, `api/tungsten-core.txt` (`RigidBody::MAX_MASS`, one line per public path, from `just api`), and the moved pins below. Its watch list, from `just preflight`, is 01's `inv_mass` readers, the 13 files `just preflight --step 14 docs/plans/core-hardening.md` lists, and the two `with_mass` test helpers (`examples/01_platformer/tests/game/brick.rs:286` and `examples/01_platformer/tests/game/spells.rs:131`). Every 01 mass stays under the cap, and the verifier's run held them.
- **Change:**
  - Apply `ch-huge-mass/fix.patch` (`RigidBody::MAX_MASS = 1e12`).
  - Rule: a body's mass lies in (0, `RigidBody::MAX_MASS` = 1e12], with a finite inverse. `with_mass` panics on a mass above it, and `rigid_body.mass` refuses one, for a dynamic mass and a kinematic reaction mass alike. A contact's impulse is mass times closing speed, so at the cap it reaches `f32::MAX` only past 3.4e26 px/s. No check runs in the solver's loops.
  - Residual row: the public `inv_mass` field and the unbounded lower end. A mass near 3e-39 has an inverse near 3.3e38, and two such bodies overflow `inv_a + inv_b`.
- **Done-when:**
  - §2.3's checks;
  - the test commit's line: `rigid_body: mass 1e36 at 1000 px/s: [NaN, NaN], [NaN, NaN] at step 1`;
  - `just api-check` → exit 0 after `just api`;
  - 01's `with_mass` calls (`tests/game/brick.rs`, `tests/game/spells.rs`) and core's tunneling tests' `with_mass(1.0e6)` stay under the cap.
- **Commits:**
  - `test(physics): a huge finite mass, ignored until its fix`;
  - `fix(physics): a body's mass at most 1e12`.
- **Moves:** none (§2.8): no workspace body's mass passes 400 (1e6 in core's tests).

### Step 15: `float-parse` (P3, new row)

- **Finding:** the workspace builds `serde_json` 1 without `float_roundtrip` (`Cargo.toml:50`). A 17-digit float therefore parses one ULP off:
  - a slot read from text restores an f64 saved value off (176 of 2,000);
  - a kept area's f32s are written back as other bytes at the next capture (17 of 199);
  - text to `WorldSave` to text is not the identity.
  It amends `D-160` (slots) and `D-230` (a world save's text). The repro is A.14:
  - `oracle_new_1_a_saved_f64_restores_from_its_text_bit_for_bit`;
  - `oracle_new_1_a_kept_areas_f32_writes_back_as_it_was_read`.
- **Files:** `crates/tungsten-core/src/tests/save/world.rs`, `Cargo.toml`, `Cargo.lock` (no package added; checked unchanged), `crates/tungsten-core/src/tests/save/oracle.rs`, and any pins the workspace run moves. Its watch list: the template's `templates/basic/src/save.rs` (a slot holding a `WorldSave`), 02's `examples/02_bench/src/tests/guards/saves.json`, and the umbrella's `crates/tungsten/src/tests/saves.rs`.
- **Change:**
  - Not prototyped. The workspace's `serde_json` takes `features = ["float_roundtrip"]`.
  - Remove the oracle's `float-parse` entry and its exclusion: the generator writes floats that are not multiples of 0.25.
  - Rule: a save's text reads back the floats it wrote, bit for bit. serde_json parses with `float_roundtrip`, at about twice the default parse's cost per float. The feature adds no crate.
- **Done-when:**
  - §2.3's checks;
  - the test commit's lines: `176 of 2,000 pressures restored off` and `17 of 199 healths wrote other bytes from a kept area`;
  - `cargo tree -e features -i serde_json --locked | rg -q float_roundtrip` → exit 0;
  - `git diff --quiet Cargo.lock` → exit 0;
  - the save long mode.
- **Commits:**
  - `test(saves): a save's floats read back from text, ignored until its fix`;
  - `fix(saves): parse saved floats exactly with serde_json's float_roundtrip`.
- **Moves:** as Step 5.

### Step 16: `ref-order` (P3, new row)

- **Finding:** one key's references, split between a loaded area and a kept one, come back in another order. The resolved references load at once, and the waiting ones append at the area's entry (`Pass::resolve`, `:1105-1123`; `add_refs`, `:281`; `enter`, `:623-637`). That is against `RefReader`'s "in their saved order" (`crates/tungsten-core/src/save/registry.rs:97-98`). The repro is A.15: `oracle_new_3_a_key_split_across_a_kept_area_keeps_its_order`, which uses Step 6's helpers.
- **Files:** `crates/tungsten-core/src/tests/save/world.rs`, `crates/tungsten-core/src/save/world.rs`, `crates/tungsten-core/src/save/registry.rs` (its doc), `crates/tungsten-core/src/tests/save/oracle.rs`, and any pins the workspace run moves. Its watch list: `crates/tungsten-core/src/save/area.rs`, whose `:292` calls `world::enter`, and Step 5's.
- **Change:** a contract step (Q5).
  - Why not a fix: the finding's two directions both fail.
    - Waiting with the whole key leaves a restored `Hitbox` without the loaded targets it has hit until the area's entry (`crates/tungsten-kit/src/overlap/hit.rs:736-752` keeps its hits under one activation key, `has_hit` at `:1753` refuses a repeat), so they can be hit again (§7, 1.1).
    - Inserting at saved indices needs each codec's cooperation, since a list codec appends.
  - The test commit lands A.15 ignored, as the finding's repro.
  - The contract commit:
    - says what happens, in `RefReader`'s doc and an entry amending `D-230`: a split key loads in two calls, its resolved references at the restore and its waiting ones at the area's entry, each call in saved order, and a capture in the window writes the live references, then the waiting ones;
    - rewrites A.15's assertion to that order and keeps it as the pin;
    - adds a guard in `crates/tungsten-kit/src/tests/saved.rs`: a restored `Hitbox` whose key names a loaded target and a kept one never hits the loaded one again between the restore and the entry, with hit detection run in that window;
    - moves the oracle's `ref-order` from `FOUND` to `DESIGN` as the order rule;
    - lists the kit's ten `RefCodec`s (`rg -n 'RefCodec \{' crates/tungsten-kit/src`) and says, for each, whether a split's order matters to it.
- **Done-when:**
  - §2.3's checks, the fix commit's checks for the contract commit;
  - the test commit's line: `the marks in their live order, and the kept World's capture` (`left: ([3865623817, 3379584492], false)`);
  - the rewritten A.15 and the `Hitbox` guard pass;
  - the save long mode with `ref-order` under `DESIGN`.
- **Commits:**
  - `test(saves): a key split across a kept area, ignored until its fix`;
  - `docs(saves): a key split across a kept area loads in two calls`.
- **Moves:** as Step 5.

### Step 17: `ch-static-velocity` (P3)

- **Finding:** the P3 row on `gather_proxies`. It amends `D-154`'s body kinds and `D-066`'s gather. The repro is A.16: `a_static_bodys_velocity_moves_nothing_on_it`.
- **Files:** `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/physics/components.rs`, `crates/tungsten-core/src/physics/step/gather.rs`, `crates/tungsten-core/src/tests/physics/step_oracle.rs`, and the moved pins below.
- **Change:**
  - Apply `ch-static-velocity/fix.patch`.
  - Remove the oracle's `ch-static-velocity` entry.
  - Rule: a static body's `Velocity` is no surface speed. Every proxy neither dynamic nor kinematic gets velocity zero in the gather, and a moving immovable surface is a kinematic body.
- **Done-when:**
  - §2.3's checks;
  - the test commit's lines: `the box on a static floor of velocity [0, -200]`, `left: Vec2(0.0, -16.583334)`, `right: Vec2(0.0, -15.743673)`;
  - the physics long mode.
- **Commits:**
  - `test(physics): a static body's velocity, ignored until its fix`;
  - `fix(physics): a static body's velocity moves nothing`.
- **Moves:** none (§2.8).

### Step 18: `ch-static-join` (P3)

- **Finding:** the P3 row on the join of two touching static boxes. It amends `D-064`'s seam face masks and `D-066`'s gather. The repro is A.17: `a_box_slides_over_the_join_of_two_touching_static_boxes`.
- **Files:** `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/physics/step/gather.rs`, `crates/tungsten-core/src/physics/step/mod.rs` (`StepExtra`, `index_bodies`), `crates/tungsten-core/src/tests/physics/query.rs`, and the moved pins below. Its watch list, from `just preflight`, is `crates/tungsten-core/src/physics/query.rs`, which reads `index_bodies`' entries and stays unedited, and 01's one `cast_shape` caller, `examples/01_platformer/src/features/enemies/drops.rs:270`. Queries keep every entity face, so both are unchanged.
- **Change:**
  - Apply `ch-static-join/fix.patch` (`mask_static_joins`, its scratch in the boxed `StepExtra`), then change it in two places (§7, 2.1 and 2.2):
    - **The match.** For each right or bottom face, the patch scans every face on that face's plane, which is quadratic for separated boxes that share a plane. The step instead groups faces by axis, plane and layers, and sorts each group's low-side and high-side faces by span start. One sweep keeps, among the high faces starting at or before a low face's start, the one with the greatest end. The low face is internal when that end reaches the low face's own end. The same sweep the other way masks high faces. A plane costs O(k log k).
    - **Queries keep every entity face.** `index_bodies` (`crates/tungsten-core/src/physics/step/mod.rs:710-726`) writes `FACE_ALL` for each body's index entry, not its proxy's mask. A cast's filter, or a despawn after the step, can drop the box that covers a face, and the cast would then miss the box behind it. So `cast_along_a_row_of_tiles_catches_on_no_seam` holds, and so does `crates/tungsten-core/src/physics/query.rs:21`'s "Entity colliders keep every face".
  - Rule: static boxes are masked where they touch, as tile cells are, in the step's contacts and sweeps. A face is internal when a static box in the same layers has its opposite face on the same plane, exactly, spanning it whole. Partial covers, overlaps and static/tile seams keep their faces. Queries keep every entity face (Q7).
  - The fix commit adds two tests:
    - `a_cast_past_a_dropped_box_meets_the_box_touching_it` (`crates/tungsten-core/src/tests/physics/query.rs`): two touching static boxes. A cast from the left hits the right box's left face both with the left box excluded and after the left box is despawned. Both cases pass at `cad63fa5` too: they guard the query contract;
    - `static_joins_scale_with_their_faces` (`crates/tungsten-core/src/tests/physics/step.rs`): 2,048 static boxes of one height in a row, first separated, then touching. A `cfg(test)` count of the pass's face comparisons stays under 4 per face in both, and the touching row's inner faces are masked.
  - Residual row: a static box against a tile map, partial covers, and casts, which still catch on the join of two touching static boxes (Q7).
- **Done-when:**
  - §2.3's checks;
  - the test commit's line: `from x 0 at 120 px/s over the join: at [58.257168, -13.746817], on one deck at [120, -13.743673]`;
  - `cargo test -p tungsten-core --lib --locked -- a_cast_past_a_dropped_box_meets_the_box_touching_it static_joins_scale_with_their_faces cast_along_a_row_of_tiles_catches_on_no_seam` → exit 0, 3 passed;
  - the physics long mode.
- **Commits:**
  - `test(physics): a box over two touching static boxes, ignored until its fix`;
  - `fix(physics): touching static boxes mask their shared faces`.
- **Moves** (§2.8): 02's `physics` guard (`examples/02_bench/src/tests/guards/physics.json`): `events_sent` 30,029 → 30,048 in the verifier's run of the patch, starting from Step 3's values once Step 3 is in. The rewritten match can move the guard's `allocations` and `allocated_bytes` too, which are re-read from its failure. The cast test the patch moved stays as it is, since queries keep every entity face. Nothing moves in 01 or the template.
- **Perf:** one sort of the static boxes' faces per frame, O(s log s) in their s faces, where the patch's scan was quadratic (§2.8).

### Step 19: `ch-map-edges` (P3)

- **Finding:** the P3 row on map edges. It amends `D-228`'s outline and `D-203`'s cached proxies. The repro is A.18: `a_box_crosses_two_maps_placed_edge_to_edge`.
- **Files:** `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/physics/step/gather.rs`, `crates/tungsten-core/src/physics/step/outline.rs`, and the moved pins below.
- **Change:**
  - Apply `ch-map-edges/fix.patch` on Step 18. Its first `gather.rs` hunk, the `FACE_*` import line, is Step 18's own, so drop it (§2.5).
  - Rule: two maps of one cell size whose grids meet edge to edge join. An edge cell's outer face, covered by the abutting map's cell in a collision layer of the same layers by the outline's cover rule, is internal. It is masked whenever a map changes, and restored when the neighbour moves, is written or goes. Casts still meet the traced faces.
  - Residual row: misaligned grids keep their seams, and casts across map seams meet them.
- **Done-when:**
  - §2.3's checks;
  - the test commit's line: `from x 100 at 120 px/s over the seam: at [154.25717, 26.253183], on one map at [220, 26.25295]`;
  - the physics long mode.
- **Commits:**
  - `test(physics): a box crossing two maps edge to edge, ignored until its fix`;
  - `fix(physics): maps placed edge to edge mask their shared faces`.
- **Moves:** 02's `terrain` guard: `allocated_bytes` 2,563,820 → 2,565,740, from Step 12's values once it is in (§2.8). Nothing moves in core, 01 or the template.

### Step 20: `ch-gravity-scale-pairs` (P3)

- **Finding:** the P3 row on `pair_config`, `build_pairs` and `repair_pairs`. It amends `D-081`, the gravity term. The repro is A.19: `one_bodys_gravity_scale_leaves_the_other_pair_boxes_alone`.
- **Files:** `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/physics/step/pairs.rs`, `crates/tungsten-core/src/tests/physics/step_oracle.rs`, and the moved pins below.
- **Change:**
  - Apply `ch-gravity-scale-pairs/fix.patch` on Step 2, with the merge from `perf-runs/20261107-core-hardening/both-findings-combined.patch`:
    - the one `repair_pairs` `StepExtra` destructure hunk takes both fields;
    - `trips_oversize` reads the per-proxy lift. Otherwise a member of large scale is judged by the world's allowance and repaired into the grid with an oversize box.
  - Remove the oracle's `ch-gravity-scale-pairs` entry.
  - Rule: an awake dynamic proxy's budget takes the gravity term at its own scale: `r_i = |v_i|·t_left + max(1, |s_i|)·|g|·h²·(n−1)(n+2)/2 + m + 2·linear_slop`. No other body's scale reaches a budget, so worlds with every scale at most 1 keep their budgets bit for bit. `pair_config`'s lift of all budgets to the largest scale is gone. With Step 2 in, the gravity row needs no scale cap.
- **Done-when:**
  - §2.3's checks;
  - the test commit's line: `left: (1, 57.25001)  right: (0, 1.5625001)`;
  - the physics long mode;
  - `cargo test -p tungsten-kit --locked` → exit 0: its lob test with a scale-2 stone.
- **Commits:**
  - `test(physics): one body's gravity scale and the others' pair boxes, ignored until its fix`;
  - `fix(physics): a pair budget takes its own gravity scale`.
- **Moves:** none (§2.8). No workspace body has a gravity scale other than 1 outside tests, and the kit's scale-2 lob test held.

### Step 21: `ch-midair-sleep` (P3)

- **Finding:** the P3 row on the sleep test. It amends `D-065`'s sleep test. The repro is A.20: `a_free_body_under_a_small_gravity_scale_never_sleeps_in_the_air`.
- **Files:** `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/physics/step/sleep.rs`, `crates/tungsten-core/src/physics/step/mod.rs` (`StepExtra::contacted`), `crates/tungsten-core/src/tests/physics/step_oracle.rs`, and the moved pins below.
- **Change:**
  - Apply `ch-midair-sleep/fix.patch`.
  - Remove the oracle's `ch-midair-sleep` entry.
  - Rule: a member that gravity pulls, with no final-substep contact, restarts its sleep timer. Documenting the bound instead, the row's alternative, fixes nothing.
- **Done-when:**
  - §2.3's checks;
  - the test commit's line: `scale 0.04, toss 0: slept at (step, y) Some((30, 4.5374985)), y after 630 steps 4.5374985`;
  - the physics long mode.
- **Commits:**
  - `test(physics): a body with a small gravity scale asleep in mid-air, ignored until its fix`;
  - `fix(physics): a body gravity pulls with nothing under it stays awake`.
- **Moves:** none (§2.8).

### Step 22: `ch-tag-adoption` (P3)

- **Finding:** the adoption part of the P3 row "previous audit's remaining edges", whose other edges stay. It amends `D-065`. The repro is A.21: `an_island_asleep_beside_a_sleeper_adopts_its_tag`.
- **Files:** `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/physics/step/sleep.rs`, and the moved pins below.
- **Change:**
  - Apply `ch-tag-adoption/fix.patch` on Step 21. Both edit `sleep_frame_end`'s doc paragraph (§2.5).
  - Rule: adoption reads any final-substep contact with a sleeper, speculative ones included.
  - The case needs Step 21's slow fall, and stays a case after it, since Step 21 counts a speculative contact as held.
- **Done-when:**
  - §2.3's checks;
  - the test commit's line: `upper box 0.5024948 px over the lower one: (lower, upper) woke (true, false), upper fell 0 px`;
  - the physics long mode.
- **Commits:**
  - `test(physics): an island asleep over a sleeper, ignored until its fix`;
  - `fix(physics): sleep-tag adoption reads speculative contacts`.
- **Moves:** 02's `terrain` guard: `events_sent` 7,926 → 7,985, from Step 19's values once it is in (§2.8). Nothing moves in core, 01 or the template.

### Step 23: `ch-spin-angle` (P3)

- **Finding:** the P3 row on the spin writeback and `Spin`. It amends `D-229`, rolling circles. The repro is A.22: `a_spinner_far_from_angle_zero_keeps_its_rate`.
- **Files:** `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/physics/components.rs`, `crates/tungsten-core/src/physics/step/gather.rs`, `crates/tungsten-core/src/physics/step/mod.rs`, and the moved pins below.
- **Change:**
  - Apply `ch-spin-angle/fix.patch`, then `ch-spin-angle/moved-test.patch`, which re-pins `a_spinner_on_a_frictionless_floor_keeps_its_spin` to the wrapped angle.
  - Rule: `Spin::angle` stays in [0, 2π). The gather takes the stored angle whole turns in, and the writeback takes the end whole turns in. The writeback moves `prev_angle` by the same turns, so it can lie outside the range and the drawn rotation never sweeps back. A written angle is taken in at the next awake step, and a sleeping spinner keeps its angle.
- **Done-when:**
  - §2.3's checks;
  - the test commit's line: `from 360000: turned 97.5 rad of 100, angle 360097.5`;
  - the physics long mode;
  - `cargo test -p example-02-bench --locked guards` → exit 0: `sprites_drawable` culls a sprite at rotation exactly 0.0 by its box, so a wrap landing on 0.0 could move its count.
- **Commits:**
  - `test(physics): a spinner far from angle zero, ignored until its fix`;
  - `fix(physics): a spinner's angle wraps into one turn`.
- **Moves:** `a_spinner_on_a_frictionless_floor_keeps_its_spin` (`crates/tungsten-core/src/tests/physics/step.rs`). Old: the angle rising to about 10.0. New: `step 75: the angle stopped at 6.250011`, where it wraps to about 0.050, ending near 3.717 (`moved-test.patch`, which the verifier left out of its run; §2.8). Nothing in 01, 02 or the template moves. Outside the step, core's interpolation and `Spin`'s save form are the angle's only readers, and the moved `prev_angle` keeps the drawn turn smooth.

### Step 24: `oracle-thin-box` (P3, new row)

- **Finding:** a box narrower than half a cell, sunk into a solid cell past the depth where its side overlap is the smaller one, takes a horizontal normal (`aabb_vs_aabb_speculative` takes the smaller exposed overlap). It loses its floor and falls through the cell. Three of 20,000 seeds show it, thin boxes beside ceiling slopes. It amends `D-228`. The repro is A.23: `oracle_new_2_a_thin_box_never_falls_through_a_solid_cell`, with its helper `spawn_oracle_map`. The oracle's witness is seed 13734 to step 18.
- **Files:** `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/tests/physics/step_oracle.rs`, the rule's files (`crates/tungsten-core/src/physics/collision.rs` by default), and the moved pins.
- **Change:**
  - Not prototyped; it is prototyped first, as Step 13.
  - Default rule (Q4), which keeps admission and retention apart (§7, 2.3):
    - **Admission.** A box's contact with a solid cell takes the cell's top face, normal up, when the box's bottom stood within the face's band at the substep before. That is a box arriving from above, as now.
    - **Retention.** A contact that held the top face at the substep before keeps it while the box's centre stays above the cell's centre, however far the box sinks. Retention does not re-check the band. The held bit belongs to the contact pair and lasts across substeps and frames with the pair's warm start; the prototype names where it is stored.
    - **Otherwise,** a contact with no held top face takes the smaller exposed overlap, as now. So a side impact high on a cell stays horizontal: a box of half (2, 6) at (−7, −2) against a cell of half 8 at the origin overlaps 3 across and 12 down, and must keep its horizontal normal (§7, 1.3).
  - The test commit adds lateral-entry counterexamples beside A.23: boxes thrown into a cell's upper half from either side keep their side contact and are not lifted onto the top. It also adds a deep-hold case at seed 16715's depth: a thin box pressed 13 px into a solid cell, held there for 60 substeps, then slid sideways at 9.5 px/s, keeps the cell's top.
  - The rule lands under Step 13's conditions.
  - Residual row: what pushes the box sideways into the cell, a bias push with zero stored velocity beside ceiling slope cells (`D-229`'s slope contact). The row also covers a box spawned or teleported deep into a solid cell: it has no held top face, so it takes the smaller overlap, as now.
- **Done-when:**
  - §2.3's checks;
  - the test commit's line: `step 18: the thin box's centre [-229.51392, 185.156] passed the solid cell's bottom face (y 180.18094)`;
  - the physics long mode with `oracle-thin-box` removed.
- **Commits:**
  - `test(physics): a thin box in a solid cell, ignored until its fix`;
  - `fix(physics): a box deep in a solid cell keeps the cell's top`.
- **Moves:** measured here, as Step 5.

### Step 25: Close

- **Files:** this plan (deleted), `docs/plans/phase-7.md` (§3's Step 18 and its evidence row), `ROADMAP.md`, `docs/known-issues.md` (the residual rows and the skipped steps' rows).
- **Change:**
  - Run the checks and the demo.
  - Every skipped step's row keeps its next work and its evidence path, and its test stays ignored with its reason.
  - Write phase-7.md's evidence row as its §3 asks.
  - Delete this plan and its citations, and take `core-hardening` out of Now.
- **Done-when:**
  - `just check` → exit 0;
  - `just physics-release` → exit 0, 16 passed;
  - §2.7's three long modes → exit 0;
  - `just repo-check` → exit 0;
  - `rg -n '#\[ignore = "7.21 step' crates/tungsten-core` lists only skipped steps' tests, and Step 7's fallback test if Step 7 split;
  - `cd examples/01_platformer && cargo run` → the pit plays as before (the owner's check);
  - `rg -n 'docs/plans/core-hardening\.md' docs ROADMAP.md AGENTS.md crates examples templates` → exit 1 after the deletion. No link to the deleted file is left. The topic name in phase-7.md's evidence row and the skipped rows' `perf-runs/20261107-core-hardening/` evidence paths stay (§7, 1.5).
- **Ends with:** the commits, the evidence, what the owner should try (01's pit, 02's `physics` and `slopes` rows), and the next prompt: 7.12's plan, from tungsten-next's `step` row.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Do the oracles' six findings belong in 7.21, which the owner scoped to track C's findings? | Yes: they are track C's, reproduced at `09631340`, and core's. Steps 5, 6, 13, 15, 16 and 24 hold them. |
| Q2 | The oracles' default runs: the finders' seed counts add 33 s to core's lib tests in debug. | Every seed kept, the runs sharded into tests over seed ranges (Step 1), the lib tests under 8 s wall. |
| Q3 | `oracle-crush`'s rule. | A pushed body stopped short of a static's mid-plane in the arrival pass (`D-092`). A rule that moves a physics hash stops the step (phase-7.md §5). |
| Q4 | `oracle-thin-box`'s rule. | A box from above takes a cell's top face by the face's band. Once held, the top face is kept while the box's centre stays above the cell's centre, with no band check. A side impact keeps the smaller overlap (`D-228`). All on Step 13's terms. |
| Q5 | `ref-order`'s rule. | A contract, not a fix: a split key loads in two calls, each in saved order, as `RefReader`'s doc and an entry amending `D-230` say. Waiting whole would re-arm a restored hitbox; saved indices need each codec's cooperation. |
| Q6 | 23 finding steps, past the ten that split a 7.10 plan (phase-7.md §6). | One plan: its seventeen prototyped steps each land a verified patch in two commits, and a session resumes from the evidence log. It splits at Step 13 only if its critique finds it too large. |
| Q7 | Do casts follow the step's static joins? | No. Queries keep every entity face, because a cast's filter or a despawn after the step can drop the covering box (§7, 2.1). A later change could keep each masked face's cover and open the face when a query drops that cover. |

Each question takes its stated default; the owner reviews the plan on their own time.

## 5. Invariants

- **May move:**
  - the tests each step's **Moves** names, re-pinned in that step with old and new values in its evidence row;
  - for Steps 5, 6, 13, 15, 16 and 24, the tests the rule explains (Step 5's terms);
  - 02's row digests where a step changes contacts (Steps 3, 10, 11 and 12), read by the release's sitting (§2.8);
  - the oracles' `KNOWN` and `FOUND` lists, only by removing an entry, by `witness_thin_box`'s re-reduction (§2.7) or by Step 7's fallback split.
- **Must not move:**
  - the determinism hash `0x088e_c07a_73c1_b168` (`physics_step_is_bit_identical_across_runs`) and the containment hash `0xaee2_72e0_1ffc_4e4c` (`pinned_step_state_is_unchanged`);
  - `size_of::<Proxy>() == 80`;
  - the kit's determinism pin (`crates/tungsten/src/tests/kit_recipes/determinism.rs`), unless a step's **Moves** names it;
  - every test outside a step's list, except a counter guard's value that §2.8 lets a later step move;
  - each oracle's default run on HEAD after every commit;
  - the API snapshots under `api/` beyond the doc lines a fix names.

## 6. Stop conditions

- A done-when check fails: restore the step's files from the last commit, mark the step skipped, and continue with the steps that don't depend on it.
- A patch's merge needs a file outside the step's list, or a hunk the step's merge note doesn't cover: stop the step.
- A fix moves a physics hash, `Proxy`'s size, or a test outside its list for a reason the rule doesn't explain: stop the step.
- An oracle fails on a seed that no remaining entry explains: stop the step, and record the seed and `physics_oracle_reduce`'s scene.
- A skipped step's dependants: Step 8 and Step 9 need Step 7; Step 9 needs Step 8; Step 11's `sweep.rs` hunk is cut on Step 3, so it re-cuts on `cad63fa5`'s `sweep.rs` if Step 3 is skipped; Step 12 needs Steps 2 and 9 or a re-cut; Step 16 needs Step 6's helpers (the test commit can carry them); Step 19 drops its import hunk only after Step 18; Step 20 needs Step 2 or its own fix alone (`ch-gravity-scale-pairs/fix.patch` applies alone on `cad63fa5`, with the lone-scale abort the row names); Step 22 needs Step 21; Step 24's rule may need Step 13's. A step that loses its base re-cuts its patch from its rule within its files, or stops.

## 7. Critique

Round 1: the `critique` skill with `--repo --force`, `gpt-6-astra` at xhigh, 2026-11-07, reading the draft at `09631340` with Appendix A cut by `sed` (72,337 characters) and a focus note (the verifier's numbers pending, the records edits a commit later). Footer: `421.7 s, tokens in 1452987 (cached 1341568), out 11015 (reasoning 5374), commands 26, output 405.8 KB (largest 73.9 KB), usage 5h 0% → 30%, week 61% → 65%`. Five findings, all accepted. They changed done-when checks and two steps' rules, so a second round runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1.1 | HIGH: Step 16's default, a split key waiting whole for its area, leaves a restored `Hitbox` without the loaded targets it has hit, so they can be hit again before the entry; the save oracle enters with no gameplay between, so it cannot see this | `crates/tungsten-kit/src/overlap/hit.rs:736-752` (hits under one activation key), `:1753` (`has_hit` refuses a repeat); the oracle's `unsplit` (`oracle-ecs/test.patch:2897`) | accepted | Step 16 becomes a contract step: two loads, each in saved order, documented in `RefReader`'s doc and an entry; A.15 rewritten to that order; a `Hitbox` guard with hit detection between the restore and the entry; `ref-order` to `DESIGN`; Q5 |
| 1.2 | MEDIUM: Step 7's patch compares only a kinematic support's kind and layers, but the oracle's `ch-kinematic-support` entry also skips decks flattened under a sleeper, so removing it fails the default run and stops Steps 7–9 | `ch-kinematic-support/fix.patch:70` (`changed`), `oracle-physics/test.patch:639` (`Op::Reshape` to a quarter height), `oracle-physics/notes.md:85` | accepted | Step 7 adds a `flattened` case and compares a kinematic support's shape, never its box; a split entry with a witness is the fallback; §2.5's Step 8 merge takes the shape |
| 1.3 | MEDIUM: Step 24's default picks a vertical normal for a side impact high on a cell, removing the lateral barrier, and the oracle's opposite-face check would not see a side-to-top escape | `crates/tungsten-core/src/physics/collision.rs:117-124` (the smaller exposed overlap), the critic's case worked through: 3 across, 12 down | accepted | Q4: a held top-face contact persists while the centre stays above the cell's centre, and a new contact keeps the smaller overlap; lateral-entry counterexamples join the test commit |
| 1.4 | MEDIUM: the witness recovery's `physics_oracle_reduce` skips every labelled failure, `oracle-thin-box` included, unless `ORACLE_ALL` is set | `oracle-physics/test.patch:3212`, `:3223` | accepted | §2.7: `ORACLE_ALL=1`, and the new witness's failure labelled `oracle-thin-box` |
| 1.5 | MEDIUM: the close's `rg 'core-hardening'` → exit 1 conflicts with the skipped rows' evidence paths and with phase-7.md's evidence row, which names the plan | `docs/plans/phase-7.md:427`; 7.18's and 7.10g's evidence rows name their plans by topic | accepted | Step 25 checks for a link to the deleted file only |

Round 2:
- **Run:** the `critique` skill with `--repo --force --effort xhigh`, 2026-11-07, at `09631340`.
- **Input:** the plan without §7, the evidence log and Appendix A (82,399 characters).
- **Focus file:** round 1's five changes and the changes since: the verifier's numbers and overlaps, §2.8's re-pin rule, Step 18's cast contract and preflight's additions.
- **Footer:** `350.3 s, tokens in 1019067 (cached 926720), out 9211 (reasoning 5414), commands 16, output 294.9 KB (largest 74.2 KB), usage 5h 30% → 53%, week 65% → 69%`.
- **Outcome:** five findings, all accepted. There is no third round, and nothing is left open: Q7 records Step 18's query choice.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | HIGH: Step 18 copies the static-join masks into the query index. A cast that excludes the left of two touching boxes, or runs after that box is despawned, then misses the right box through its masked entry face. `filter_excludes_bodies_and_tiles` uses separated boxes, so it cannot see this | `crates/tungsten-core/src/physics/step/mod.rs:710-726` (`index_bodies` copies `proxy.face_mask`); `crates/tungsten-core/src/physics/query.rs:1325-1347` (`face_normal` returns `None` through a masked face); `ch-static-join/fix.patch:99-102`; `crates/tungsten-core/src/tests/physics/query.rs`'s `filter_excludes_bodies_and_tiles` and `despawned_body_is_skipped` (boxes 50 px apart) | accepted | Queries keep every entity face: `index_bodies` writes `FACE_ALL`, so the cast test and `query.rs:21`'s doc hold. Adds a guard test with exclusion and a despawn after the step. Casts at joins become a residual row (Q7) |
| 2.2 | MEDIUM: for each right or bottom face, the patch's mask pass scans every face on that plane. That is quadratic for separated boxes sharing a plane, and §2.8 names no such cost | `ch-static-join/fix.patch:75-107`, the loop over `faces[start..end]` inside the loop over the same range; `ch-static-join/notes.md:81`, whose O(s log s) counts the sort only | accepted | Step 18 matches faces by a sorted sweep per axis, plane and layers. A `cfg(test)` count keeps comparisons under 4 per face over 2,048 boxes. §2.8 names the per-frame sort |
| 2.3 | MEDIUM: Step 24's rule makes the hold depend on the box's bottom being within the band at the substep before, so one deep substep ends it. Seed 16715's box stands 13 px deep | Step 24's rule text; `oracle-physics/new-2.md:13` | accepted | Admission (the band) is kept apart from retention: a held top face stays while the centre is above the cell's centre. Adds a case held beyond the band for 60 substeps. Spawned-deep boxes join the residual row |
| 2.4 | MEDIUM: §2.8's re-pin exception reaches behaviour pins, such as a cast's asserted result, not just counters | §2.8's first bullet; §5's must-not-move list | accepted | The exception covers counter values in guard pins only. A behaviour pin that moves again is a stop, unless its step changes the contract |
| 2.5 | MEDIUM: Step 7's fallback leaves the flattened case failing un-ignored, and changes the `KNOWN` list in a way §5 forbids | Step 7's fallback bullet; §5's oracle-list line; the header's done-when | accepted | The fallback gets acceptance of its own: the flattened case in its own test, ignored with its reason, and `ch-kinematic-reshape` replacing the entry, which §5 now allows. Step 25 and the header expect that one ignore |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-11-08 | passed | Both oracle patches dry-run and apply clean (exit 0). The default runs sharded: eight physics tests over `DEFAULT_SEEDS`' eighths and `physics_oracle_witnesses_trip_their_findings`, eight ECS tests, four save tests (`seed_part`); `KNOWN`'s sources name each finding's known-issues row by ID. `cargo test -p tungsten-core --lib --locked oracle` → exit 0, 32 passed, 6 ignored, 5.46 s. `cargo test -p tungsten-core --locked`, warm → exit 0, lib 1,515 passed, 6 ignored, finished in 6.02 s (8.2 s for the command; functional only). Long modes in release → exit 0: physics seeds 0..20000 246.4 s, saves 51.1 s, ECS 289.0 s. `TUNGSTEN_ORACLE_FOUND=all … a_restored_world_equals` → exit 101, each quarter failing (`save oracle: 342 of 1000 seeds failed`; tags `restored-saving`, `parse-round-trip`, `restored-hits-targets`, `recapture`). `each_known_row_is_still_open` → ok. `cargo test --workspace --locked --no-fail-fast` → exit 0, 4,746 passed, 16 ignored in 30 binaries, 146 s, nothing moved. `just preflight docs/plans/core-hardening.md` → P2 fires on `ecs/mod.rs` (10 guard files), every guard held. `just physics-release` → exit 0, 16 passed, the two hash tests ok. `cargo fmt --all -- --check`, `just lint`, `just repo-check`, `just ctx`, `git diff --check` → exit 0. `just smoke` not run: both commits are `cfg(test)` only, so no example binary changes. | `e5756114`, `78b5f6bb` |
| 2 | 2026-11-08 | passed | Test commit: `cargo test -p tungsten-core --lib --locked -- --ignored a_fast_bodys_pair_staging_stays_bounded` → exit 101, `the pair grid staged [51076, 5071504] cell refs for one body`; fmt and `just lint` → exit 0. Fix: `fix.patch` at offsets (`mod.rs` +4 and +8, `step.rs` +1), no fuzz. The arrival test, `the_arrival_pass_meets_an_oversize_body_a_push_reaches`: a box, a ball and a thin kinematic wall co-moving at 150,000 px/s at 8 px cells, all three oversize, spawned in that order so the ball's hit on the box solves before the ball's contact with the wall; the box stops 0.25 px into the wall. Its negative control, the arrival pass's oversize line removed → exit 101, `the pushed box's right edge at x 46.8125, the wall's left face at x 9`. (A first version spawned the wall first and passed without the line: the ball's own contact with the wall, admitted at their closing speed and solved first, took the hit.) The four tests → exit 0, 4 passed. `cargo test -p tungsten-core --locked` → exit 0, lib 1,519 passed, 6 ignored. `just physics-release` → exit 0, 16 passed, the hash tests ok. Physics long mode → exit 0, 258.5 s; from this step the long modes build with `physics-release`'s `RUSTFLAGS` (`-C force-frame-pointers=yes`), so the two share one release build of core (250 s), and frame pointers change no result. `cargo test --workspace --locked --no-fail-fast` → exit 0, 4,750 passed, 16 ignored, 154 s, nothing moved. `just smoke` → exit 0, every row OK, 44 s. fmt, `just lint`, `just repo-check`, `just ctx`, `git diff --check` → exit 0; `rg -n '#\[ignore = "7.21 step 2:' crates/tungsten-core` → exit 1. Records: `D-251` (amends `D-080`, `D-081`), DESIGN.md's pair sentence, the P1 row replaced by the co-moving residual (P3). | `d8c125ae`, `1427b84f` |
| 3 | 2026-11-08 | passed | Test commit: `--ignored sweep_net_lets_a_fast_body_slide_along_a_face_it_lands_on` → exit 101, `20 of 20 runs froze on the face`; fmt and `just lint` → exit 0. Fix: `fix.patch` clean. The repro → exit 0. `cargo test -p tungsten-core --locked` → exit 0, lib 1,520 passed, 6 ignored. `just physics-release` → exit 0, 16 passed, the hash tests ok. Physics long mode → exit 0, 253.2 s. Re-pinned from the failures, as the plan listed: `examples/02_bench/src/tests/guards/physics.json` `allocated_bytes` 12,376,832 → 12,380,160, `events_sent` 30,029 → 31,371; `physics-sparse.json` `events_sent` 4,894 → 4,866; `slopes.json` `allocations` 3,087 → 3,130, `allocated_bytes` 3,525,936 → 3,526,668, `events_sent` 43,703 → 46,180, the bodies the net held over a face now landing in their contacts. `cargo test --workspace --locked --no-fail-fast` → exit 0, 4,751 passed, 16 ignored, 126 s, nothing else moved (01's pit guard and containment held). `just smoke` → exit 0, every row OK. fmt, `just lint`, `just repo-check`, `just ctx`, `git diff --check` → exit 0; `rg -n '#\[ignore = "7.21 step 3:' crates/tungsten-core` → exit 1. Records: `D-252` (amends `D-064`), DESIGN.md's net clause, the P1 row removed (no residual). | `f4707af7`, `4628e812` |
| 4 | 2026-11-08 | passed | Test commit: `--ignored a_body_the_step_rests_on_a_solid_stays a_body_resting_on_a_floor_comes_back_where_it_was_saved` → exit 101, `rested 0.2564392 px deep` (`left: Vec2(40.0, 136.25644)`, `right: Vec2(40.0, 140.25644)`) and `tiles true circle false g 900 dt 0.0167: rests 0.256439 px deep, free_spot Some(Vec2(104.0, 152.25644)) from Vec2(104.0, 156.25644)` among 14 cases; fmt and `just lint` → exit 0. Fix: `fix.patch` clean (`REST_SLOPS = 4.0`). The two tests → exit 0. `cargo test -p tungsten-core --locked` → exit 0, lib 1,522 passed, 6 ignored. `just physics-release` → exit 0, 16 passed, the hash tests ok. No long mode: no oracle reaches `free_spot`. `cargo test --workspace --locked --no-fail-fast` → exit 0, 4,753 passed, 16 ignored, 113 s, nothing moved (the template's continue tests held). `just smoke` → exit 0, 46 rows OK. fmt, `just lint`, `just repo-check`, `just ctx`, `git diff --check` → exit 0; `rg -n '#\[ignore = "7.21 step 4:' crates/tungsten-core` → exit 1. Records: `D-253` (amends `D-230`), DESIGN.md's restore sentence, the P2 row replaced by the loaded-columns residual (P3). | `fb2a5886`, `bcc71605` |
| 5 | 2026-11-08 | passed | Test commit: A.4 appended, its placement note dropped and its provenance line kept; `--ignored oracle_new_4_a_restore_leaves_a_world_that_names_nothing_unsaving` → exit 101, `the loaded run saves as the live one does`, `left: true`, `right: false`; fmt and `just lint` → exit 0. The plan's subject (77 characters) passes the 72-character limit, so the commit reads `test(saves): a restore where nothing is named, ignored until its fix`. Fix, not prototyped, as the oracle's direction: `restore_world` inserts a `SaveState` only when the World holds none and the save names something (`names_anything`: a named record, a gone name or a spawn in any area, a kept area, a counter above 0); `apply` no longer inserts one, the counter and kept areas go into a state only when there is one, and `resolve_all` reads no state as nothing kept. The oracle's `saving-flips` entry and its gate removed: a fifth of the default run's levels name nothing. The repro → exit 0. `cargo test -p tungsten-core --locked` → exit 0, lib 1,523 passed, 6 ignored. `just physics-release` → exit 0, 16 passed. Save long mode → exit 0, 58.0 s. `cargo test -p tungsten-kit --locked` → exit 0, 979 passed and 147 doctests (the cooldown and spawner passes). `cargo test --workspace --locked --no-fail-fast` → exit 0, 4,754 passed, 16 ignored, 193 s, nothing moved (no watched file or count pin). `just smoke` → exit 0, 46 rows OK. fmt, `just lint`, `just repo-check`, `just ctx`, `git diff --check` → exit 0; `rg -n '#\[ignore = "7.21 step 5:' crates/tungsten-core` → exit 1. Records: no entry (the code meets `D-230`); DESIGN.md's save-state sentence; the P2 row removed. | `0f597b7f`, `e1874a68` |
| 6 | 2026-11-08 | passed | Test commit: A.5 appended (its placement note dropped) with `Marks`, `world_with_marks` and `hero_dog`; `--ignored oracle_new_2_a_named_entitys_list_loads_onto_its_saved_form oracle_new_2_a_reference_waiting_on_a_kept_area_leaves_no_fresh_one` → exit 101, `the live World's one mark` (left two entities, right one) and `the hero chases the fresh dog: true`; fmt and `just lint` → exit 0. The plan's subject (76 characters) is past the limit, so it reads `test(saves): a named entity's refs at a restore, ignored until its fix`. Fix, not prototyped, as the oracle's direction: `plan_area` puts each named component with a reference codec, no patch and no removal from its own saved form (the fresh value through `to_value`, checked), so `load` starts from the form's placeholders. The oracle's `fresh-refs` entry removed and its gate replaced by `fresh_refs: rng.chance(50)`, so half the default runs leave reference changes out of the form and start level hit lists with a target. Moved, the rule explaining it: `a_reference_to_an_unsaved_entity_is_dropped_and_reported` (`crates/tungsten-core/src/tests/save/world.rs`) pinned the fresh spawn's chase target (`bat_1`) under a forged reference that does not resolve; the chase has no patch, so it now loads onto its form's placeholder, as `RefCodec::load`'s doc says, re-pinned to `Entity::PLACEHOLDER`. The two tests → exit 0. `cargo test -p tungsten-core --locked` → exit 0, lib 1,525 passed, 6 ignored. `just physics-release` → exit 0, 16 passed. Save long mode → exit 0, 45.4 s. `cargo test -p tungsten-kit --locked` → exit 0, 979 passed and 147 doctests (the `Hitbox` codec). `cargo test --workspace --locked --no-fail-fast` → exit 0, 4,756 passed, 16 ignored, 179 s, nothing else moved. `just smoke` → exit 0, 46 rows OK. fmt, `just lint`, `just repo-check`, `just ctx`, `git diff --check` → exit 0; `rg -n '#\[ignore = "7.21 step 6:' crates/tungsten-core` → exit 1. Records: no entry (the code meets `RefCodec::load`'s doc); DESIGN.md's restore sentence; the P2 row removed. | `4ea8fc5f`, `bafd09ac` |
| 7 | 2026-11-08 | passed | Test commit: A.6 appended with the sixth case, `flattened` (the still platform's `Collider` set to half (40, 2), the oracle's deck reshape; it passes when the box drops 5 px or more onto the thinner deck, where it may sleep again); `--ignored a_sleeper_on_a_still_kinematic_platform_falls_when_it_goes` → exit 101, `12 riders hang:` the five finder cases and `flattened`, both spawn orders, `asleep true, fell 0`; fmt and `just lint` → exit 0. The plan's subject (77 characters) is past the limit, so it reads `test(physics): a kinematic deck that goes, ignored until its fix`. Fix: `fix.patch` clean, extended as the step says: `Support` keeps a kinematic body's collider as bits (`shape_bits`: the shape's kind, its half extents or radius, its offset), and `changed` and `support_order` compare kind, layers and that shape, never the box; `Support` is 56 bytes (32 at base, 40 with the patch alone). With the shape compare removed, the repro fails on the two `flattened` cases alone (`2 riders hang`); with it → exit 0. The oracle's `ch-kinematic-support` entry and its witness removed. `cargo test -p tungsten-core --locked` → exit 0, lib 1,526 passed, 6 ignored, the physics oracle's default shards included. `just physics-release` → exit 0, 16 passed, the hash tests ok. Physics long mode with the entry removed → exit 0, 250.6 s. Re-pinned from the failure: `examples/01_platformer/tests/game/guards/pit.json` `allocations` 59,977 → 59,980 and `allocated_bytes` 68,096,156 → 69,236,700 (the patch alone read 68,910,940), the record holding kinematic proxies and its larger supports. `cargo test --workspace --locked --no-fail-fast` → exit 0, 4,757 passed, 16 ignored, 233 s, nothing else moved. `just smoke` → exit 0, 46 rows OK. fmt, `just lint`, `just repo-check`, `just ctx`, `git diff --check` → exit 0; `rg -n '#\[ignore = "7.21 step 7:' crates/tungsten-core` → exit 1 (no split). Records: `D-254` (amends `D-156`), DESIGN.md's support sentence, the P2 row removed. | `3bd508d9`, `faacb2d4` |
| 8 | 2026-11-08 | skipped | Test commit: `--ignored a_write_to_a_sleeper_or_its_support_wakes_it` → exit 101, `6 sleepers held:` (shrunk, grown, gravity scale -1, own friction 0, floor friction 0, material: `asleep after the write true, moved 0 px`); fmt and `just lint` → exit 0; subject shortened to 72 characters (`… a sleeper or its support …`). The fix: `fix.patch` read as reversed on step 7's tree (its `Shape` import was in), so it was merged by hand as the step says: `SleepEntry::body` and `body_bits`, `written` with the body bits in `same_bodies` and `rebuild`, `Support::friction` beside step 7's shape, `changed` and `support_order` with the friction, the frictions through `diff_supports` and `record_supports`. With it: the repro and step 7's → exit 0; `cargo test -p tungsten-core --locked` → exit 0, lib 1,527 passed with the oracle's `ch-write-wake` entry removed; `just physics-release` → exit 0, 16 passed; `cargo test --workspace --locked --no-fail-fast` → exit 0, 4,758 passed, after re-pinning 01's pit `allocated_bytes` 69,236,700 → 69,399,580 (§2.8: with `Support::friction` alone dropped the pit read step 7's 69,236,700, so the move is the merged `Support`'s word on step 7's kinematic supports); `just smoke` → exit 0. **Stop (§6):** the physics long mode → exit 101, 343.5 s: `invariant 2 (passed through solid cells) failed 1 times, unexplained (new); first at seed 1844 step 70: body 4 through the map (Right to Left), [-125.112595, 94.162125] to [-141.18234, 95.26435]`. `physics_oracle_reduce` (`ORACLE_SEEDS=1844..1845`): `invariant 2 [new] passed through solid cells: seed 1844, reduced to 3 bodies, 1 statics, map true, 1 ops, 71 steps`, `step 70: body 1 through the map (Right to Left), [-125.07353, 94.162125] to [-141.1604, 96.272865]`, the op a reshape at step 62. The trace: the box rests on a solid cell beside a slope cell from step 19, is pushed left into the solid cell above the slope (x -118.06 → -127.46, centre 2.3 px past its right face) and sleeps at step 48; step 7's tree leaves it frozen there, so its long mode held, and the fix wakes it at the reshape, after which the solve carries it out the cell's far side. A pre-existing fault the fix exposes, recorded as a new known-issues row; no `KNOWN` entry may be added (§5). Every changed file restored with `git show HEAD:`; the repro stays ignored with its reason. Dependants: step 9's patch was cut on `cad63fa5` alone, so it goes on step 7 without the body bits (§6's re-cut), and step 12 then has its base. | `064b3f14`; `perf-runs/20261107-core-hardening/ch-write-wake/step8-stop/`; this commit |

## Follow-ups

- The ghost ledge (the slope finder's sibling, `ch-slope-clamp/probe-floor-ghost-ledge.log`): a known-issues row with no step.
- The one-way, static-box and map-seam casts each residual names: their rows.

## Appendix A: repro tests

Each repro test as its finder or oracle wrote it, so it survives its git-ignored folder. A finder's test comes from its `test.patch`, cut against `cad63fa5`; an oracle's comes from its `new-N.md`, whose first comment names the file it is appended to. A step adds `#[ignore = "7.21 step N: fails until its fix"]` under each `#[test]` in its test commit (§2.3).

### A.1: `ch-pair-box`

From `perf-runs/20261107-core-hardening/ch-pair-box/test.patch`, inserted in `crates/tungsten-core/src/physics/broadphase.rs` at line 137 of `cad63fa5` (the hunk, with its context):

```diff
@@ -134,6 +134,12 @@
         self.cell_size
     }

+    /// Tests: the (proxy, cell) refs staged since the last clear.
+    #[cfg(test)]
+    pub(crate) fn cell_refs(&self) -> usize {
+        self.cell_refs
+    }
+
     /// True when at most `few` records are staged and none covers a cell of
     /// `query`: a query would visit nothing, so a caller can skip the walk.
     pub(crate) fn misses_few(&self, query: &Aabb, few: usize) -> bool {
```

From `perf-runs/20261107-core-hardening/ch-pair-box/test.patch`, appended to `crates/tungsten-core/src/tests/physics/step.rs`:

```rust
#[test]
fn a_fast_bodys_pair_staging_stays_bounded() {
    // D-081: one 1/60 s step of a lone circle at the speeds 60 s and 600 s
    // of fall at gravity 3,600 give it. Its pair box grows by its speed on
    // both axes, so the pair grid should stage a bounded number of cells.
    let staged = |speed: f32| {
        let mut world = seed_world();
        world.resource_mut::<PhysicsConfig>().gravity = Vec2::new(0.0, 3600.0);
        let (body, shape) = (RigidBody::dynamic(), Collider::circle(8.0));
        spawn_body(&mut world, body, shape, Vec2::ZERO, Vec2::new(0.0, speed));
        physics_step(&mut world);
        world.resource::<PhysicsBuffers>().grid.bodies.cell_refs()
    };
    let cells = [staged(216_000.0), staged(2_160_000.0)];
    assert!(
        cells.iter().all(|&refs| refs <= 64 * 64),
        "the pair grid staged {cells:?} cell refs for one body"
    );
}
```

### A.2: `ch-sweep-net`

From `perf-runs/20261107-core-hardening/ch-sweep-net/test.patch`, inserted in `crates/tungsten-core/src/tests/physics/step.rs` at line 400 of `cad63fa5` (the hunk, with its context):

```diff
@@ -397,6 +397,117 @@
 }

 #[test]
+fn sweep_net_lets_a_fast_body_slide_along_a_face_it_lands_on() {
+    // A frictionless box faster than its smallest half extent a substep,
+    // let go 4 px over a floor while moving along it, or 4 px over the
+    // 45° ramp of `a_box_slides_down_a_frictionless_slope` while falling:
+    // the net's clamp at landing leaves it a hair over the face, where the
+    // next substep's clamp undoes its whole move, so it would hold there
+    // with its velocity and never sleep. It slides on instead.
+    let mut frozen = Vec::new();
+    let mut runs = 0;
+    for tiles in [false, true] {
+        for gravity in [900.0, 3_600.0] {
+            for dt in [1.0 / 60.0, 1.0 / 30.0] {
+                for (half, speed) in [(Vec2::splat(2.0), 500.0), (Vec2::splat(4.0), 1_000.0)] {
+                    let mut world = seed_world();
+                    world.resource_mut::<PhysicsConfig>().gravity = Vec2::new(0.0, gravity);
+                    world.resource_mut::<Time>().advance_frame(dt);
+                    if tiles {
+                        let floor = vec![vec![0; 1_024]];
+                        spawn_map(&mut world, "floor", Vec2::new(-512.0, 100.0), 1_024, floor);
+                    } else {
+                        let floor = Collider::aabb(Vec2::new(8_000.0, 10.0));
+                        spawn_static(&mut world, floor, Vec2::new(0.0, 110.0));
+                    }
+                    let start = Vec2::new(0.0, 100.0 - half.y - 4.0);
+                    let launch = Vec2::new(speed, 0.0);
+                    let body = spawn_body(
+                        &mut world,
+                        RigidBody::dynamic(),
+                        Collider::aabb(half),
+                        start,
+                        launch,
+                    );
+                    let steps = (1.0 / dt).round() as usize;
+                    let mut half_way = 0.0;
+                    for step in 1..=steps {
+                        physics_step(&mut world);
+                        drain_events(&mut world);
+                        if step == steps / 2 {
+                            half_way = world.get::<Position>(body).unwrap().0.x;
+                        }
+                    }
+                    runs += 1;
+                    let at = world.get::<Position>(body).unwrap().0;
+                    if at.x - half_way < 0.4 * speed {
+                        frozen.push(format!(
+                            "{} g {gravity} dt {dt:.4} half {half} at {speed} px/s: x {half_way} \
+                             at 0.5 s, {} at 1 s, velocity {}, {} px over the face",
+                            if tiles { "tiles" } else { "static box" },
+                            at.x,
+                            world.get::<Velocity>(body).unwrap().0,
+                            100.0 - (at.y + half.y),
+                        ));
+                    }
+                }
+            }
+        }
+    }
+    let rows: Vec<String> = (0..12)
+        .map(|row| {
+            (0..12)
+                .map(|col| match (col + row).cmp(&11) {
+                    std::cmp::Ordering::Equal => 'a',
+                    std::cmp::Ordering::Greater => '#',
+                    std::cmp::Ordering::Less => '.',
+                })
+                .collect()
+        })
+        .collect();
+    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
+    for gravity in [900.0, 3_600.0] {
+        for half in [Vec2::new(4.0, 1.0), Vec2::splat(4.0)] {
+            let mut world = one_way_world(slope_map(&rows));
+            world.resource_mut::<PhysicsConfig>().gravity = Vec2::new(0.0, gravity);
+            let start = Vec2::new(150.0, 192.0 - (150.0 + half.x) - half.y - 4.0);
+            let launch = Vec2::new(0.0, 1_600.0);
+            let body = spawn_body(
+                &mut world,
+                RigidBody::dynamic(),
+                Collider::aabb(half),
+                start,
+                launch,
+            );
+            let mut at20 = Vec2::ZERO;
+            for step in 1..=40 {
+                physics_step(&mut world);
+                drain_events(&mut world);
+                if step == 20 {
+                    at20 = world.get::<Position>(body).unwrap().0;
+                }
+            }
+            runs += 1;
+            let at = world.get::<Position>(body).unwrap().0;
+            // Still sliding between steps 20 and 40, or off the ramp's foot.
+            if at.distance(at20) < 4.0 && at.x > 8.0 {
+                frozen.push(format!(
+                    "ramp g {gravity} half {half} at 1600 px/s: {at20} after 20 steps, {at} after \
+                     40, velocity {}",
+                    world.get::<Velocity>(body).unwrap().0,
+                ));
+            }
+        }
+    }
+    assert!(
+        frozen.is_empty(),
+        "{} of {runs} runs froze on the face:\n{}",
+        frozen.len(),
+        frozen.join("\n")
+    );
+}
+
+#[test]
 fn sweep_net_restages_statics_when_the_proxy_set_changes() {
     // The sweep grid is staged per frame (D-080). After a static despawns and
     // two spawn, proxy indices shift: the old slab must stop clamping and the
```

### A.3: `ch-resting-band`

From `perf-runs/20261107-core-hardening/ch-resting-band/test.patch`, appended to `crates/tungsten-core/src/tests/physics/free_spot.rs`:

```rust
/// Runs `frames` physics steps of `dt`, at gravity `gravity` down and
/// `max_step_dt` `bound`.
fn rest(world: &mut World, gravity: f32, bound: f32, dt: f32, frames: usize) {
    let config = world.resource_mut::<PhysicsConfig>();
    config.gravity = Vec2::new(0.0, gravity);
    config.max_step_dt = bound;
    world.insert_resource(crate::time::Time::new());
    for _ in 0..frames {
        world.resource_mut::<crate::time::Time>().advance_frame(dt);
        crate::physics::physics_step(world);
    }
}

#[test]
fn a_body_the_step_rests_on_a_solid_stays() {
    // The soft contacts rest a body 0.25 to 0.90 px into the floor or wall
    // that holds it, past `linear_slop`; the free spot leaves it there.
    let floor: Vec<_> = (0..20).map(|col| (col, 10, WALL)).collect();
    let mut moved = Vec::new();
    // `face` is the solid's face on `axis`, which the 8 px body rests on.
    let mut check = |world: &World, body: Entity, axis: usize, face: f32, case: String| {
        let at = world.get::<Position>(body).unwrap().0;
        let spot = free_spot(world, body);
        if spot != Some(at) {
            let depth = (at[axis] + 4.0 - face).abs();
            moved.push(format!(
                "{case}: rests {depth:.6} px deep, free_spot {spot:?} from {at:?}"
            ));
        }
    };
    for tiles in [true, false] {
        for circle in [false, true] {
            for (gravity, bound, dt) in [
                (900.0, 1.0 / 30.0, 1.0 / 60.0),
                (3600.0, 1.0 / 30.0, 1.0 / 30.0),
                (3600.0, 0.0, 0.1),
            ] {
                // A floor whose top is at y 160: tile row 10, or a static box.
                let mut world = world(if tiles { &floor } else { &[] });
                if !tiles {
                    world.spawn_with(RigidBodyBundle::r#static(
                        Position(Vec2::new(160.0, 168.0)),
                        Collider::aabb(Vec2::new(160.0, 8.0)),
                    ));
                }
                let shape = if circle {
                    Collider::circle(4.0)
                } else {
                    Collider::aabb(Vec2::splat(4.0))
                };
                let body = world.spawn_with(RigidBodyBundle::dynamic(
                    Position(Vec2::new(104.0, 150.0)),
                    shape,
                ));
                rest(&mut world, gravity, bound, dt, (2.0 / dt).ceil() as usize);
                let case = format!("tiles {tiles} circle {circle} g {gravity} dt {dt:.4}");
                check(&world, body, 1, 160.0, case);
            }
        }
    }
    // A two-crate stack's bottom crate.
    let mut stack = world(&floor);
    let bottom = crate_at(&mut stack, Vec2::new(104.0, 150.0));
    crate_at(&mut stack, Vec2::new(104.0, 140.0));
    rest(&mut stack, 900.0, 1.0 / 30.0, 1.0 / 60.0, 180);
    check(&stack, bottom, 1, 160.0, "a stack's bottom crate".into());
    // A crate thrown at a static wall's left face (x 200) without gravity.
    let mut room = world(&[]);
    room.spawn_with(RigidBodyBundle::r#static(
        Position(Vec2::new(208.0, 100.0)),
        Collider::aabb(Vec2::new(8.0, 40.0)),
    ));
    let thrown = crate_at(&mut room, Vec2::new(150.0, 100.0));
    room.get_mut::<crate::physics::Velocity>(thrown).unwrap().0 = Vec2::new(100.0, 0.0);
    rest(&mut room, 0.0, 1.0 / 30.0, 1.0 / 60.0, 600);
    check(&room, thrown, 0, 200.0, "against a wall".into());
    assert!(
        moved.is_empty(),
        "{} moved:\n{}",
        moved.len(),
        moved.join("\n")
    );
}
```

From `perf-runs/20261107-core-hardening/ch-resting-band/test.patch`, appended to `crates/tungsten-core/src/tests/save/world.rs`:

```rust
#[test]
fn a_body_resting_on_a_floor_comes_back_where_it_was_saved() {
    // The crate rests 0.256 px into the floor, past `linear_slop`; the
    // restore's free spot leaves it there.
    use crate::assets::{LayerKind, TilemapData, TilemapInstance, TilemapRegistry};
    use crate::physics::PhysicsConfig;

    // A 20 × 10 map of 16 px cells, its floor row 9 (top at y 144), and the crate.
    let floored = || {
        let mut map = TilemapData::new(16, 16, 20, 10);
        let rock = map.add_tile("rock");
        let walls = map.add_layer("walls", LayerKind::Collision);
        for col in 0..20 {
            map.set_tile(walls, col, 9, rock);
        }
        let mut maps = TilemapRegistry::new();
        maps.insert("room".into(), map);
        let mut world = world();
        world.insert_resource(maps);
        world.spawn_with((TilemapInstance::new("room", Vec2::ZERO),));
        world.insert_resource(PhysicsConfig {
            gravity: Vec2::new(0.0, 900.0),
            ..PhysicsConfig::default()
        });
        world.insert_resource(crate::time::Time::new());
        let body = crate_body(&mut world);
        (world, body)
    };
    let (mut a, body) = floored();
    for _ in 0..120 {
        a.resource_mut::<crate::time::Time>()
            .advance_frame(1.0 / 60.0);
        crate::physics::physics_step(&mut a);
    }
    let rest = a.get::<Position>(body).unwrap().0;
    let (mut b, body) = floored();
    let report = restore_world(&mut b, &capture_world(&a).unwrap(), &(Scene,)).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(
        b.get::<Position>(body).unwrap().0,
        rest,
        "rested {} px deep",
        rest.y + 4.0 - 144.0
    );
}
```

### A.4: `saving-flips`

From `perf-runs/20261107-core-hardening/oracle-ecs/new-4.md`, appended where its first comment says:

```rust
// Evidence for oracle-ecs new-4 (core hardening 2026-11-07, save oracle).
// File and module: append to crates/tungsten-core/src/tests/save/world.rs,
// which is `save::world::tests`, with that file's `Weather` and `Scene`.

#[test]
fn oracle_new_4_a_restore_leaves_a_world_that_names_nothing_unsaving() {
    // A game that saves its resources alone and names no entity: the live
    // run holds no save state, so the kit's passes keep their query order.
    let start = || {
        let mut world = World::new();
        world.register_saved_resource::<Weather>("weather");
        world.insert_resource(Weather {
            raining: false,
            wind: 0.0,
        });
        world
    };
    let mut a = start();
    a.resource_mut::<Weather>().raining = true;
    let saved = capture_world(&a).unwrap();
    let mut b = start();
    restore_world(&mut b, &saved, &(Scene,)).unwrap();
    assert_eq!(b.resource::<Weather>(), a.resource::<Weather>());
    assert_eq!(
        crate::save::saving(&b),
        crate::save::saving(&a),
        "the loaded run saves as the live one does"
    );
}
```

### A.5: `fresh-refs`

From `perf-runs/20261107-core-hardening/oracle-ecs/new-2.md`, appended where its first comment says:

```rust
// Evidence for oracle-ecs new-2 (core hardening 2026-11-07, save oracle).
// File and module: append to crates/tungsten-core/src/tests/save/world.rs,
// which is `save::world::tests` (`#[path]` from save/world.rs). The tests use
// that file's imports (`use super::*;`) and helpers: world, text, Chase,
// Scene; `name`, `RefCodec`, `RefReader`, `RefWriter` and `SaveOptions` are
// imported there from crate::save. new-3's test shares `Marks` and
// `hero_dog`.

/// A list of references under one key whose codec's `load` extends it, as
/// the area tests' `Hits` and the kit's hit lists do.
#[derive(Debug, Clone, PartialEq)]
struct Marks {
    key: u32,
    targets: Vec<Entity>,
}

#[derive(Serialize, Deserialize)]
struct MarksForm {
    key: u32,
}

impl From<&Marks> for MarksForm {
    fn from(marks: &Marks) -> Self {
        Self { key: marks.key }
    }
}

impl From<MarksForm> for Marks {
    fn from(form: MarksForm) -> Self {
        Self {
            key: form.key,
            targets: Vec::new(),
        }
    }
}

fn save_marks(marks: &Marks, out: &mut RefWriter<'_>) {
    for &target in &marks.targets {
        out.put("marks", target);
    }
}

fn load_marks(marks: &mut Marks, refs: &RefReader<'_>) {
    marks.targets.extend_from_slice(refs.get("marks"));
}

fn marks_keys(_: &Marks) -> Vec<String> {
    vec!["marks".into()]
}

/// `world()` with `Marks` saved under `marks`.
fn world_with_marks() -> World {
    let mut world = world();
    world.register_saved_with(
        "marks",
        SaveOptions::<Marks>::form::<MarksForm>().refs(RefCodec {
            save: save_marks,
            load: load_marks,
            keys: marks_keys,
        }),
    );
    world
}

/// A run's start for new-2 and new-3: `hero` and `dog`, global, and, when
/// `cave`, `bat_1` in area `cave`. The hero chases the dog.
fn hero_dog(world: &mut World, cave: bool) -> (Entity, Entity, Option<Entity>) {
    let hero = world.spawn_prefab("hero", [16.0, 16.0]).unwrap();
    let dog = world.spawn_prefab("hero", [32.0, 16.0]).unwrap();
    world.insert(
        hero,
        Chase {
            target: dog,
            speed: 2.0,
        },
    );
    name(world, hero, None, "hero").unwrap();
    name(world, dog, None, "dog").unwrap();
    let bat = cave.then(|| {
        let bat = world.spawn_prefab("bat", [100.0, 40.0]).unwrap();
        name(world, bat, Some("cave"), "bat_1").unwrap();
        bat
    });
    (hero, dog, bat)
}

#[test]
fn oracle_new_2_a_named_entitys_list_loads_onto_its_saved_form() {
    // The bat's level gives it one mark, the hero; the run changes
    // nothing. The restore leaves the fresh value (no patch) and its codec
    // extends it with the saved mark.
    let start = |world: &mut World| {
        let hero = world.spawn_prefab("hero", [16.0, 16.0]).unwrap();
        let bat = world.spawn_prefab("bat", [100.0, 40.0]).unwrap();
        world.insert(
            bat,
            Marks {
                key: 1,
                targets: vec![hero],
            },
        );
        name(world, hero, None, "hero").unwrap();
        name(world, bat, Some("cave"), "bat_1").unwrap();
        (hero, bat)
    };
    let mut a = world_with_marks();
    start(&mut a);
    let saved = capture_world(&a).unwrap();
    let mut b = world_with_marks();
    let (hero, bat) = start(&mut b);
    restore_world(&mut b, &saved, &(Scene,)).unwrap();
    assert_eq!(
        b.get::<Marks>(bat).unwrap().targets,
        [hero],
        "the live World's one mark"
    );
}

#[test]
fn oracle_new_2_a_reference_waiting_on_a_kept_area_leaves_no_fresh_one() {
    // The hero's chase, the dog at its naming, turns to the bat in area
    // `cave`; the restoring World has not loaded the cave.
    let mut a = world();
    let (hero, _, bat) = hero_dog(&mut a, true);
    a.get_mut::<Chase>(hero).unwrap().target = bat.unwrap();
    let first = text(&capture_world(&a).unwrap());
    let mut b = world();
    let (hero_b, dog_b, _) = hero_dog(&mut b, false);
    restore_world(&mut b, &serde_json::from_str(&first).unwrap(), &(Scene,)).unwrap();
    assert!(b.resource::<SaveState>().is_kept("cave"));
    let chasing_dog = b.get::<Chase>(hero_b).unwrap().target == dog_b;
    let second = text(&capture_world(&b).unwrap());
    assert!(
        !chasing_dog && second == first,
        "the hero chases the fresh dog: {chasing_dog}; the capture writes {}",
        &second[second.find("\"hero\"").unwrap_or(0)..][..120]
    );
}
```

### A.6: `ch-kinematic-support`

From `perf-runs/20261107-core-hardening/ch-kinematic-support/test.patch`, appended to `crates/tungsten-core/src/tests/physics/step.rs`:

```rust
/// ch-kinematic-support (`D-156`): a box asleep on a still kinematic
/// platform falls when the platform goes, turns sensor, loses its collider
/// or stops meeting it, as one on a static platform does; and when it goes
/// after carrying the box 100 px and stopping, while a floater far off kept
/// an island asleep throughout.
#[test]
fn a_sleeper_on_a_still_kinematic_platform_falls_when_it_goes() {
    let mut hung = Vec::new();
    for platform_first in [true, false] {
        for change in [
            "despawn",
            "mask 0",
            "sensor",
            "no collider",
            "moved, despawn",
        ] {
            let mut world = seed_world();
            world.resource_mut::<PhysicsConfig>().gravity = Vec2::new(0.0, 900.0);
            let spawn = |world: &mut World, body: RigidBody, half: Vec2, y: f32| {
                spawn_body(
                    world,
                    body.with_friction(1.0),
                    Collider::aabb(half),
                    Vec2::new(0.0, y),
                    Vec2::ZERO,
                )
            };
            let platform = |world: &mut World| {
                spawn(world, RigidBody::kinematic(), Vec2::new(40.0, 8.0), 100.0)
            };
            let rider =
                |world: &mut World| spawn(world, RigidBody::dynamic(), Vec2::splat(8.0), 84.2);
            let (platform, rider) = if platform_first {
                let platform = platform(&mut world);
                (platform, rider(&mut world))
            } else {
                let rider = rider(&mut world);
                (platform(&mut world), rider)
            };
            let floater = RigidBody::dynamic().with_gravity_scale(0.0);
            spawn_body(
                &mut world,
                floater,
                Collider::circle(4.0),
                Vec2::new(1000.0, -500.0),
                Vec2::ZERO,
            );
            let run = |world: &mut World, steps: usize| {
                for _ in 0..steps {
                    physics_step(world);
                    drain_events(world);
                }
            };
            run(&mut world, 150);
            if change == "moved, despawn" {
                world.get_mut::<Velocity>(platform).unwrap().0 = Vec2::new(120.0, 0.0);
                run(&mut world, 50);
                world.get_mut::<Velocity>(platform).unwrap().0 = Vec2::ZERO;
                run(&mut world, 150);
            }
            assert!(
                is_sleeping(&world, rider),
                "{change}: the rider never slept"
            );
            let y0 = world.get::<Position>(rider).unwrap().0.y;
            match change {
                "mask 0" => world.get_mut::<RigidBody>(platform).unwrap().mask = 0,
                "sensor" => world.get_mut::<RigidBody>(platform).unwrap().kind = BodyKind::Sensor,
                "no collider" => {
                    world.remove_component::<Collider>(platform);
                }
                _ => {
                    world.despawn(platform);
                }
            }
            run(&mut world, 60);
            let fell = world.get::<Position>(rider).unwrap().0.y - y0;
            if is_sleeping(&world, rider) || fell < 100.0 {
                let asleep = is_sleeping(&world, rider);
                hung.push(format!(
                    "platform first {platform_first}, {change}: asleep {asleep}, fell {fell}"
                ));
            }
        }
    }
    assert!(
        hung.is_empty(),
        "{} riders hang:\n{}",
        hung.len(),
        hung.join("\n")
    );
}
```

### A.7: `ch-write-wake`

From `perf-runs/20261107-core-hardening/ch-write-wake/test.patch`, appended to `crates/tungsten-core/src/tests/physics/step.rs`:

```rust
/// ch-write-wake (`D-065`, `D-156`): a box held asleep by friction on its
/// floor under a tilted gravity moves once its collider shrinks or grows, its
/// gravity scale or friction changes, its floor's friction goes or its tile
/// floor's material loses its friction.
#[test]
fn a_write_to_a_sleeper_or_its_support_wakes_it() {
    use crate::physics::{PhysicsMaterial, PhysicsMaterials};
    let rubber = |friction| PhysicsMaterial {
        friction,
        ..PhysicsMaterial::default()
    };
    let mut held = Vec::new();
    for write in [
        "shrunk",
        "grown",
        "gravity scale -1",
        "own friction 0",
        "floor friction 0",
        "material",
    ] {
        let mut world = seed_world();
        world.resource_mut::<PhysicsConfig>().gravity = Vec2::new(450.0, 900.0);
        // Either floor's top is at y 90.
        let floor = if write == "material" {
            let mut data = TilemapData::new(16, 16, 32, 8);
            let tile = data.add_tile("rubber");
            let material = Some("rubber".into());
            data.set_tile_props(
                tile,
                TileProps {
                    material,
                    ..TileProps::default()
                },
            );
            let layer = data.add_layer("solid", LayerKind::Collision);
            for col in 0..32 {
                data.set_tile(layer, col, 6, tile);
            }
            world
                .resource_mut::<TilemapRegistry>()
                .insert("floor".into(), data);
            world.insert_resource(PhysicsMaterials::new([("rubber".to_owned(), rubber(2.0))]));
            let map = world.spawn();
            world.insert(map, TilemapInstance::new("floor", Vec2::new(-256.0, -6.0)));
            map
        } else {
            let floor = spawn_static(
                &mut world,
                Collider::aabb(Vec2::new(400.0, 10.0)),
                Vec2::new(0.0, 100.0),
            );
            world.insert(floor, RigidBody::r#static().with_friction(2.0));
            floor
        };
        let body = RigidBody::dynamic().with_friction(2.0);
        let body = spawn_body(
            &mut world,
            body,
            Collider::aabb(Vec2::splat(8.0)),
            Vec2::new(0.0, 82.0),
            Vec2::ZERO,
        );
        for _ in 0..120 {
            physics_step(&mut world);
            drain_events(&mut world);
        }
        assert!(
            is_sleeping(&world, body),
            "{write}: the held box never slept"
        );
        let at = world.get::<Position>(body).unwrap().0;
        match write {
            "shrunk" => {
                *world.get_mut::<Collider>(body).unwrap() = Collider::aabb(Vec2::splat(4.0));
            }
            "grown" => {
                *world.get_mut::<Collider>(body).unwrap() = Collider::aabb(Vec2::splat(16.0));
            }
            "gravity scale -1" => world.get_mut::<RigidBody>(body).unwrap().gravity_scale = -1.0,
            "own friction 0" => world.get_mut::<RigidBody>(body).unwrap().friction = 0.0,
            "floor friction 0" => world.get_mut::<RigidBody>(floor).unwrap().friction = 0.0,
            _ => world
                .resource_mut::<PhysicsMaterials>()
                .replace([("rubber".to_owned(), rubber(0.0))]),
        }
        physics_step(&mut world);
        let asleep = is_sleeping(&world, body);
        for _ in 0..29 {
            physics_step(&mut world);
            drain_events(&mut world);
        }
        let moved = world.get::<Position>(body).unwrap().0.distance(at);
        if asleep || moved < 2.0 {
            held.push(format!(
                "{write}: asleep after the write {asleep}, moved {moved} px"
            ));
        }
    }
    assert!(
        held.is_empty(),
        "{} sleepers held:\n{}",
        held.len(),
        held.join("\n")
    );
}
```

### A.8: `ch-island-tags`

From `perf-runs/20261107-core-hardening/ch-island-tags/test.patch`, appended to `crates/tungsten-core/src/tests/physics/step.rs`:

```rust
/// ch-island-tags (`D-065`): a light hit wakes the bottom ball of a sleeping
/// two-ball stack alone; despawning, removing the collider of, turning
/// sensor, teleporting, masking or waking that ball then wakes the top one,
/// which falls, as it does when the stack is struck while still asleep.
#[test]
fn removing_a_contact_woken_support_wakes_the_sleepers_it_held() {
    let mut stranded = Vec::new();
    for removal in [
        "despawn",
        "no collider",
        "sensor",
        "teleport",
        "mask 0",
        "wake",
    ] {
        let mut world = seed_world();
        let (bottom, top) = spawn_sleeping_stack(&mut world);
        settle_to_sleep(&mut world, &[bottom, top]);
        let at = world.get::<Position>(bottom).unwrap().0 - Vec2::new(9.0, 0.0);
        let light = RigidBody::dynamic().with_mass(0.01).with_gravity_scale(0.0);
        let hitter = spawn_body(
            &mut world,
            light,
            Collider::circle(2.0),
            at,
            Vec2::new(60.0, 0.0),
        );
        physics_step(&mut world);
        drain_events(&mut world);
        assert!(
            !is_sleeping(&world, bottom) && is_sleeping(&world, top),
            "{removal}: the hit did not wake the bottom ball alone"
        );
        world.despawn(hitter);
        let top_y = world.get::<Position>(top).unwrap().0.y;
        match removal {
            "despawn" => world.despawn(bottom),
            "no collider" => {
                world.remove_component::<Collider>(bottom);
            }
            "sensor" => world.get_mut::<RigidBody>(bottom).unwrap().kind = BodyKind::Sensor,
            "teleport" => teleport(&mut world, bottom, Vec2::new(300.0, 0.0)),
            "mask 0" => world.get_mut::<RigidBody>(bottom).unwrap().mask = 0,
            _ => wake(&mut world, bottom),
        }
        let asleep_at_once = is_sleeping(&world, top);
        for _ in 0..20 {
            physics_step(&mut world);
            drain_events(&mut world);
        }
        let dy = world.get::<Position>(top).unwrap().0.y - top_y;
        let asleep = is_sleeping(&world, top);
        if (removal == "wake" && asleep_at_once) || (removal != "wake" && (asleep || dy <= 5.0)) {
            stranded.push(format!(
                "{removal}: top asleep at once {asleep_at_once}, after 20 steps {asleep}, fell {dy} px"
            ));
        }
    }
    assert!(
        stranded.is_empty(),
        "{} tops stranded:\n{}",
        stranded.len(),
        stranded.join("\n")
    );
}
```

### A.9: `ch-slope-clamp`

From `perf-runs/20261107-core-hardening/ch-slope-clamp/test.patch`, appended to `crates/tungsten-core/src/tests/physics/slope.rs`:

```rust
#[test]
fn a_circle_beside_a_masked_leg_reads_no_false_overlap() {
    // ch-slope-clamp (`D-229`): a ramp's top cell, 45 br or 2:1 high br, its
    // apex (80, 16) beside a floor that covers its right leg, and a peak's
    // 45 br beside a bl at the apex (32, 16), each mask `FACE_TOP`. A circle
    // of radius 4 on the floor right of the ramp's apex, or on bl's face 1 px
    // past the peak's, clamped to the face's line and read 1.3 to 5.4 px
    // deep though it stood off; a circle cast down onto the floor at x 82
    // met the face's line square before the floor. A contact is never deeper
    // than the circle's true gap, and the cast meets nothing before the
    // floor's y 12.
    let ramp = polygon(1, Vec2::new(72.0, 24.0), HALF);
    let high = polygon(9, Vec2::new(72.0, 24.0), HALF);
    let peak = polygon(1, Vec2::new(24.0, 24.0), HALF);
    let mut bad = Vec::new();
    for (poly, center) in [
        (&ramp, Vec2::new(80.5, 11.99)),
        (&ramp, Vec2::new(82.0, 11.99)),
        (&ramp, Vec2::new(84.0, 11.99)),
        (&high, Vec2::new(82.0, 11.99)),
        (&peak, Vec2::new(35.83, 14.17)),
    ] {
        let gap = center_distance(center, poly) - 4.0;
        let contact = circle_contact(center, 4.0, poly, FACE_TOP, 1.0);
        if contact.is_some_and(|c| c.penetration > 1e-4 - gap) {
            bad.push(format!("at {center}: {contact:?}, true gap {gap}"));
        }
    }
    let shape = Shape::Circle { radius: 4.0 };
    let hit = cast(shape, Vec2::new(82.0, 0.0), Vec2::Y, &ramp, FACE_TOP);
    if hit.is_some_and(|(t, _)| t < 12.0) {
        bad.push(format!("cast down from (82, 0): {hit:?}"));
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
```

From `perf-runs/20261107-core-hardening/ch-slope-clamp/test.patch`, appended to `crates/tungsten-core/src/tests/physics/step.rs`:

```rust
#[test]
fn a_ball_by_a_ramp_top_or_a_peak_meets_no_false_overlap() {
    // ch-slope-clamp (`D-229`): a frictionless circle of radius 4 resting on
    // the floor 1 or 2 px right of a ramp top's apex (80, 16), or on a
    // peak's bl face 1 px past its apex (32, 16), met the br cell's face
    // line beside its covered right leg as a deep overlap, so it was thrown
    // over the apex and down the ramp or held on the peak. After 1 s at
    // 1/60 s each stays right of the ramp's apex or rolls past x 36, as a box
    // of half 4 does, and a circle cast from the floor beside the apex,
    // moving right, away from the ramp, never meets its top cell.
    let ramp = ["........", "....a###", "...a####", "..a#####", "########"];
    let peak = ["....", ".ab.", "####"];
    let (ball, crate_) = (Collider::circle(4.0), Collider::aabb(Vec2::splat(4.0)));
    let mut bad = Vec::new();
    for (rows, collider, start, past) in [
        (&ramp[..], ball, Vec2::new(82.0, 11.99), 80.0),
        (&ramp[..], ball, Vec2::new(81.0, 12.0), 80.0),
        (&ramp[..], crate_, Vec2::new(82.0, 11.99), 80.0),
        (&peak[..], ball, Vec2::new(35.83, 14.17), 36.0),
        (&peak[..], crate_, Vec2::new(37.0, 11.0), 36.0),
    ] {
        let mut world = one_way_world(slope_map(rows));
        let body = spawn_body(
            &mut world,
            RigidBody::dynamic(),
            collider,
            start,
            Vec2::ZERO,
        );
        for _ in 0..60 {
            physics_step(&mut world);
        }
        let end = world.get::<Position>(body).unwrap().0;
        if end.x < past {
            let shape = collider.shape;
            bad.push(format!(
                "{shape:?} from {start}: at {end}, not past x {past}"
            ));
        }
    }
    let mut world = one_way_world(slope_map(&ramp));
    physics_step(&mut world);
    let (origin, shape) = (Vec2::new(82.0, 12.05), Shape::Circle { radius: 4.0 });
    let filter = crate::physics::SpatialFilter::new();
    let hit = crate::physics::cast_shape(&world, shape, origin, Vec2::X, 8.0, &filter);
    if hit.is_some_and(|hit| hit.tile.is_some_and(|tile| (tile.col, tile.row) == (4, 1))) {
        bad.push(format!("cast from {origin} along +x: {hit:?}"));
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
```

### A.10: `ch-one-way-join`

From `perf-runs/20261107-core-hardening/ch-one-way-join/test.patch`, appended to `crates/tungsten-core/src/tests/physics/step.rs`:

```rust
/// A map of 16 px cells from rows of letters: `#` solid (tile 0), `a` a 45°
/// slope of kind 1 (tile 1), `=` one-way (tile 2) and `.` empty.
fn joined_map(rows: &[&str]) -> TilemapData {
    let mut data = TilemapData::new(16, 16, rows[0].len() as u32, rows.len() as u32);
    data.add_tile("solid");
    for (tile, props) in [
        (
            data.add_tile("ramp"),
            TileProps {
                slope: Some(slope_kind(1)),
                ..TileProps::default()
            },
        ),
        (
            data.add_tile("ledge"),
            TileProps {
                one_way: true,
                ..TileProps::default()
            },
        ),
    ] {
        data.set_tile_props(tile, props);
    }
    let layer = data.add_layer("solid", LayerKind::Collision);
    for (row, letters) in rows.iter().enumerate() {
        for (col, letter) in letters.chars().enumerate() {
            if let Some(tile) = "#a=".find(letter) {
                data.set_tile(layer, col as u32, row as u32, tile as i32);
            }
        }
    }
    data
}

#[test]
fn a_box_walks_off_one_way_cells_onto_the_solid_beside_them() {
    // ch-one-way-join (physics review r2 a2-1, D-228): row 4's top at y 64.
    // A box of half 6 let go at 0 px on one-way cells walks left at 120
    // px/s for two seconds onto solid cells at their height, or over the
    // apex of a ramp beside them: it crosses as it does on solid cells.
    let walk = |rows: &[&str]| {
        let mut world = one_way_world(joined_map(rows));
        let body = spawn_body(
            &mut world,
            RigidBody::dynamic(),
            Collider::aabb(Vec2::splat(6.0)),
            Vec2::new(400.0, 58.0),
            Vec2::new(-120.0, 0.0),
        );
        for _ in 0..120 {
            physics_step(&mut world);
        }
        world.get::<Position>(body).unwrap().0
    };
    let sky = "..............................";
    let at = walk(&[sky, sky, sky, sky, "####################=========="]);
    assert!(
        at.x < 300.0,
        "off the one-way cells onto the solid: at {at}"
    );
    let floor = "##############################";
    let at = walk(&[sky, sky, sky, sky, "...................a==========", floor]);
    assert!(
        at.x < 300.0,
        "off the one-way cells over the ramp's apex: at {at}"
    );
}
```

### A.11: `ch-arrival-wake`

From `perf-runs/20261107-core-hardening/ch-arrival-wake/test.patch`, appended to `crates/tungsten-core/src/tests/physics/step.rs`:

```rust
#[test]
fn a_hit_on_a_body_pressed_on_a_sleeper_wakes_the_sleeper() {
    // D-065, D-092: P (radius 10) at 1,000 or 3,000 px/s into X, which rests
    // 0.25 px into a sleeping S (both settled asleep, or S alone and X
    // spawned 0.5 px into it beside P), zero gravity, at 40 offsets of P's
    // arrival within a substep, at 1/60 and 1/30 s. With sleeping off S takes
    // about a third of the hit; asleep at inverse mass 0 it must wake and
    // take its share, and the x momentum stays.
    let mut absorbed = Vec::new();
    for (speed, dt, x_awake) in [1000.0_f32, 3000.0]
        .into_iter()
        .flat_map(|v| [(v, 1.0 / 60.0), (v, 1.0 / 30.0)])
        .flat_map(|(v, dt)| [(v, dt, false), (v, dt, true)])
    {
        for k in 0..40 {
            let mut world = seed_world();
            let run = |world: &mut World, steps: usize| {
                for _ in 0..steps {
                    physics_step(world);
                    drain_events(world);
                }
            };
            let x = (!x_awake).then(|| spawn_ball(&mut world, Vec2::ZERO));
            let s = spawn_ball(&mut world, Vec2::new(19.5, 0.0));
            run(&mut world, 90);
            assert!(is_sleeping(&world, s), "S never slept");
            let x = x.unwrap_or_else(|| spawn_ball(&mut world, Vec2::ZERO));
            world.resource_mut::<Time>().advance_frame(dt);
            let gap = 30.0 + k as f32 * speed * (dt / 4.0) / 40.0;
            let x0 = world.get::<Position>(x).unwrap().0.x;
            let at = Vec2::new(x0 - 20.0 - gap, 0.0);
            let p = spawn_body(
                &mut world,
                RigidBody::dynamic(),
                Collider::circle(10.0),
                at,
                Vec2::new(speed, 0.0),
            );
            run(&mut world, 30);
            let vx = |e: Entity| world.get::<Velocity>(e).unwrap().0.x;
            let momentum = vx(p) + vx(x) + vx(s);
            if is_sleeping(&world, s) || vx(s) <= 0.0 || momentum < 0.95 * speed {
                let case = (speed, dt.recip().round(), x_awake, k);
                absorbed.push((case, vx(s), momentum));
            }
        }
    }
    assert!(
        absorbed.is_empty(),
        "{} of 320 hits left S asleep, still or under 95 % of the x momentum \
         ((speed, 1/dt, X awake, offset), S vx, momentum; the first 3): {:?}",
        absorbed.len(),
        &absorbed[..absorbed.len().min(3)]
    );
}
```

### A.12: `oracle-crush`

From `perf-runs/20261107-core-hardening/oracle-physics/new-1.md`, appended where its first comment says:

```rust
// Evidence for oracle new-1 (core hardening 2026-11-07, physics oracle).
// File and module: append to crates/tungsten-core/src/tests/physics/step.rs,
// which is `physics::step::tests` (`#[path]` from physics/step/mod.rs). The
// tests use that file's imports (`use super::*;`) and helpers: seed_world,
// spawn_body, spawn_static, drain_events.

/// new-1's rig: a light body of mass 0.25 at rest on a static floor whose
/// top is at y 100 and whose half height is `half_y`, and a body `ratio`
/// times its mass falling onto it at `speed`, under gravity 3,600 for 40
/// steps of `dt`. Returns the light body's centre y at the end; the floor's
/// bottom face is at 100 + 2 · `half_y`.
fn oracle_new_1_crush(
    victim: Collider,
    pusher: Collider,
    ratio: f32,
    speed: f32,
    half_y: f32,
    dt: f32,
) -> f32 {
    let mut world = seed_world();
    world.resource_mut::<PhysicsConfig>().gravity = Vec2::new(0.0, 3600.0);
    world.resource_mut::<Time>().advance_frame(dt);
    spawn_static(
        &mut world,
        Collider::aabb(Vec2::new(600.0, half_y)),
        Vec2::new(0.0, 100.0 + half_y),
    );
    let (vh, ph) = (victim.shape.half_extents().y, pusher.shape.half_extents().y);
    let rest = 100.0 - vh - 0.25;
    let light = spawn_body(
        &mut world,
        RigidBody::dynamic().with_mass(0.25),
        victim,
        Vec2::new(0.0, rest),
        Vec2::ZERO,
    );
    spawn_body(
        &mut world,
        RigidBody::dynamic().with_mass(0.25 * ratio),
        pusher,
        Vec2::new(0.0, rest - vh - ph - 0.5 - speed * dt),
        Vec2::new(0.0, speed),
    );
    for _ in 0..40 {
        physics_step(&mut world);
        drain_events(&mut world);
    }
    world.get::<Position>(light).unwrap().0.y
}

#[test]
fn oracle_new_1_a_heavy_body_never_drives_a_light_one_through_a_static_floor() {
    let victims = [
        ("circle 6", Collider::circle(6.0)),
        ("circle 19", Collider::circle(19.0)),
        ("box 6", Collider::aabb(Vec2::splat(6.0))),
    ];
    let pushers = [
        ("circle 30", Collider::circle(30.0)),
        ("box 20", Collider::aabb(Vec2::splat(20.0))),
    ];
    let mut failed = Vec::new();
    let mut cases = 0;
    for (victim_name, victim) in victims {
        for (pusher_name, pusher) in pushers {
            for ratio in [4.0, 10.0, 30.0, 100.0, 300.0, 1000.0] {
                for speed in [500.0, 1000.0, 2000.0, 5000.0] {
                    for half_y in [4.0, 8.0, 13.0] {
                        for dt in [1.0 / 60.0, 1.0 / 30.0] {
                            cases += 1;
                            let y = oracle_new_1_crush(victim, pusher, ratio, speed, half_y, dt);
                            if y > 100.0 + 2.0 * half_y {
                                failed.push(format!(
                                    "{victim_name} under {pusher_name}, ratio {ratio}, {speed} px/s, \
                                     floor {} px, dt 1/{}: centre at y {y:.2}",
                                    2.0 * half_y,
                                    (1.0 / dt).round()
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(
        failed.is_empty(),
        "{} of {cases} cases end below the floor:\n{}",
        failed.len(),
        failed.join("\n")
    );
}
```

### A.13: `ch-huge-mass`

From `perf-runs/20261107-core-hardening/ch-huge-mass/test.patch`, appended to `crates/tungsten-core/src/tests/physics/plugin.rs`:

```rust
#[test]
fn a_huge_mass_is_refused_at_both_doors_or_stays_finite() {
    // Physics review a3-5: a mass both doors took (finite, above 0, with a
    // finite inverse) overflowed the solver's `normal_mass · (vn + bias)`
    // against a floor, and the body went NaN within 13 steps: 1e36 at
    // 1,000 px/s, 1e38 at 100. Each door refuses it, or its body stays
    // finite.
    let mut failures = Vec::new();
    for (mass, speed) in [(1.0e36_f32, 1_000.0_f32), (1.0e38, 100.0)] {
        for door in ["rigid_body", "with_mass"] {
            let mut world = physics_world();
            let mut time = Time::new();
            time.advance_frame(DT);
            world.insert_resource(time);
            world.insert_resource(crate::physics::PhysicsConfig::default());
            let floor = serde_json::json!({ "kind": "static",
                "shape": { "kind": "aabb", "half_extents": [200.0, 10.0] } });
            spawn_body(&mut world, Vec2::new(0.0, 100.0), &floor);
            let value = serde_json::json!({ "kind": "dynamic",
                "shape": { "kind": "aabb", "half_extents": [8.0, 8.0] },
                "mass": mass, "velocity": [0.0, speed] });
            let body = if door == "rigid_body" {
                let registry = world.resource::<crate::component_registry::ComponentRegistry>();
                if registry.check("rigid_body", &value).is_err() {
                    continue;
                }
                spawn_body(&mut world, Vec2::new(0.0, 60.0), &value)
            } else {
                let Ok(body) = std::panic::catch_unwind(|| RigidBody::dynamic().with_mass(mass))
                else {
                    continue;
                };
                let at = Position::new(0.0, 60.0);
                world.spawn_with(
                    RigidBodyBundle::dynamic(at, Collider::aabb(Vec2::splat(8.0)))
                        .with_body(body)
                        .with_velocity(Velocity::new(0.0, speed)),
                )
            };
            for step in 0..30 {
                physics_step(&mut world);
                let at = world.get::<Position>(body).unwrap().0;
                let v = world.get::<Velocity>(body).unwrap().0;
                if !(at.is_finite() && v.is_finite()) {
                    failures.push(format!(
                        "{door}: mass {mass:e} at {speed} px/s: {at}, {v} at step {step}"
                    ));
                    break;
                }
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
```

### A.14: `float-parse`

From `perf-runs/20261107-core-hardening/oracle-ecs/new-1.md`, appended where its first comment says:

```rust
// Evidence for oracle-ecs new-1 (core hardening 2026-11-07, save oracle).
// File and module: append to crates/tungsten-core/src/tests/save/world.rs,
// which is `save::world::tests` (`#[path]` from save/world.rs). The tests use
// that file's imports (`use super::*;`) and helpers: world, level, text,
// Health, Scene; `name` is imported there from crate::save.

#[test]
fn oracle_new_1_a_saved_f64_restores_from_its_text_bit_for_bit() {
    // A saved resource's f64, written to text and read back as a game's
    // slot does: serde_json without `float_roundtrip` parses a 17-digit
    // number best-effort.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Gauge {
        pressure: f64,
    }
    let mut drifted = Vec::new();
    let mut rewritten = 0;
    for n in 0..2_000 {
        let pressure = f64::from(n) * 0.1 - 100.0;
        let mut a = World::new();
        a.register_saved_resource::<Gauge>("gauge");
        a.insert_resource(Gauge { pressure });
        let first = text(&capture_world(&a).unwrap());
        let read: WorldSave = serde_json::from_str(&first).unwrap();
        rewritten += usize::from(text(&read) != first);
        let mut b = World::new();
        b.register_saved_resource::<Gauge>("gauge");
        restore_world(&mut b, &read, &(Scene,)).unwrap();
        let back = b.resource::<Gauge>().pressure;
        if back.to_bits() != pressure.to_bits() {
            drifted.push((pressure, back));
        }
    }
    assert!(
        drifted.is_empty() && rewritten == 0,
        "{} of 2,000 pressures restored off (first {:?}); {rewritten} saves wrote other bytes once read",
        drifted.len(),
        &drifted[..drifted.len().min(3)]
    );
}

#[test]
fn oracle_new_1_a_kept_areas_f32_writes_back_as_it_was_read() {
    // `bat_1`'s f32 health in area `cave`, which the restoring World has
    // not loaded: the restore keeps the record as parsed, and the next
    // capture writes it back.
    let mut a = world();
    let la = level(&mut a);
    let mut differ = Vec::new();
    for n in 1..200_u16 {
        a.get_mut::<Health>(la.bat_1).unwrap().current = f32::from(n) * 0.1;
        let first = text(&capture_world(&a).unwrap());
        let read: WorldSave = serde_json::from_str(&first).unwrap();
        let mut b = world();
        let hero = b.spawn_prefab("hero", [16.0, 16.0]).unwrap();
        name(&mut b, hero, None, "hero").unwrap();
        restore_world(&mut b, &read, &(Scene,)).unwrap();
        assert!(b.resource::<SaveState>().is_kept("cave"));
        if text(&capture_world(&b).unwrap()) != first {
            differ.push(f32::from(n) * 0.1);
        }
    }
    assert!(
        differ.is_empty(),
        "{} of 199 healths wrote other bytes from a kept area (first {:?})",
        differ.len(),
        &differ[..differ.len().min(5)]
    );
}
```

### A.15: `ref-order`

From `perf-runs/20261107-core-hardening/oracle-ecs/new-3.md`, appended where its first comment says:

```rust
// Evidence for oracle-ecs new-3 (core hardening 2026-11-07, save oracle).
// File and module: append to crates/tungsten-core/src/tests/save/world.rs,
// which is `save::world::tests`, after new-2's test, whose `Marks`,
// `world_with_marks` and `hero_dog` it uses (in new-2.md), with that file's
// helpers world, text and Scene.

#[test]
fn oracle_new_3_a_key_split_across_a_kept_area_keeps_its_order() {
    // The hero marks the bat (area `cave`), then the dog (global). The
    // restoring World loads the global record alone, then enters the cave.
    let mut a = world_with_marks();
    let (hero, dog, bat) = hero_dog(&mut a, true);
    a.insert(
        hero,
        Marks {
            key: 1,
            targets: vec![bat.unwrap(), dog],
        },
    );
    let first = text(&capture_world(&a).unwrap());
    let mut b = world_with_marks();
    let (hero_b, _, _) = hero_dog(&mut b, false);
    restore_world(&mut b, &serde_json::from_str(&first).unwrap(), &(Scene,)).unwrap();
    let kept = text(&capture_world(&b).unwrap());
    let bat_b = b.spawn_prefab("bat", [100.0, 40.0]).unwrap();
    name(&mut b, bat_b, Some("cave"), "bat_1").unwrap();
    crate::save::enter_area(&mut b, "cave", &(Scene,)).unwrap();
    let ids = |world: &World, entity: Entity| -> Vec<u64> {
        world
            .get::<Marks>(entity)
            .unwrap()
            .targets
            .iter()
            .map(|&target| world.get::<Saved>(target).unwrap().id().0)
            .collect()
    };
    assert_eq!(
        (ids(&b, hero_b), kept == first),
        (ids(&a, hero), true),
        "the marks in their live order, and the kept World's capture"
    );
}
```

### A.16: `ch-static-velocity`

From `perf-runs/20261107-core-hardening/ch-static-velocity/test.patch`, appended to `crates/tungsten-core/src/tests/physics/step.rs`:

```rust
#[test]
fn a_static_bodys_velocity_moves_nothing_on_it() {
    // ch-static-velocity (physics review a1-5): a box of half 8 resting at
    // 0 px on a static floor of half (64, 8). A static body never moves, so
    // its `Velocity` must leave the box where a floor with Velocity zero
    // leaves it: up, down, and along with friction 1.
    let run = |velocity: Vec2, friction: f32| {
        let mut world = seed_world();
        world.resource_mut::<PhysicsConfig>().gravity = Vec2::new(0.0, 900.0);
        let body = spawn_body(
            &mut world,
            RigidBody::dynamic().with_friction(friction),
            Collider::aabb(Vec2::splat(8.0)),
            Vec2::new(0.0, -16.0),
            Vec2::ZERO,
        );
        spawn_body(
            &mut world,
            RigidBody::r#static().with_friction(friction),
            Collider::aabb(Vec2::new(64.0, 8.0)),
            Vec2::ZERO,
            velocity,
        );
        for _ in 0..60 {
            physics_step(&mut world);
        }
        world.get::<Position>(body).unwrap().0
    };
    for (velocity, friction) in [
        (Vec2::new(0.0, -200.0), 0.0),
        (Vec2::new(0.0, 200.0), 0.0),
        (Vec2::new(100.0, 0.0), 1.0),
    ] {
        assert_eq!(
            run(velocity, friction),
            run(Vec2::ZERO, friction),
            "the box on a static floor of velocity {velocity}"
        );
    }
}
```

### A.17: `ch-static-join`

From `perf-runs/20261107-core-hardening/ch-static-join/test.patch`, appended to `crates/tungsten-core/src/tests/physics/step.rs`:

```rust
#[test]
fn a_box_slides_over_the_join_of_two_touching_static_boxes() {
    // ch-static-join: two static decks of half (64, 8) touching at x 64,
    // their tops at y -8. A box of half 6 let go on one at 0 px and sliding
    // at 120 px/s for a second crosses the join at its speed, as it does on
    // one deck of their union, both ways.
    let slide = |decks: &[(f32, f32)], start: f32, speed: f32| {
        let mut world = seed_world();
        world.resource_mut::<PhysicsConfig>().gravity = Vec2::new(0.0, 900.0);
        for &(x, half) in decks {
            spawn_static(
                &mut world,
                Collider::aabb(Vec2::new(half, 8.0)),
                Vec2::new(x, 0.0),
            );
        }
        let body = spawn_body(
            &mut world,
            RigidBody::dynamic(),
            Collider::aabb(Vec2::splat(6.0)),
            Vec2::new(start, -14.0),
            Vec2::new(speed, 0.0),
        );
        for _ in 0..60 {
            physics_step(&mut world);
        }
        world.get::<Position>(body).unwrap().0
    };
    let (one, two) = (&[(64.0, 128.0)][..], &[(0.0, 64.0), (128.0, 64.0)][..]);
    for (start, speed) in [(0.0, 120.0), (128.0, -120.0)] {
        let control = slide(one, start, speed);
        let joined = slide(two, start, speed);
        assert!(
            (control.x - start).abs() > 100.0,
            "on one deck at {control}"
        );
        assert!(
            (joined.x - control.x).abs() < 0.01,
            "from x {start} at {speed} px/s over the join: at {joined}, on one deck at {control}"
        );
    }
}
```

### A.18: `ch-map-edges`

From `perf-runs/20261107-core-hardening/ch-map-edges/test.patch`, appended to `crates/tungsten-core/src/tests/physics/step.rs`:

```rust
#[test]
fn a_box_crosses_two_maps_placed_edge_to_edge() {
    // ch-map-edges (physics review r2 a2-2, D-228): maps of 10 × 3 cells of
    // 16 px, row 2 solid (its top at y 32), at x 0 and 160, against one map
    // of 20 × 3 as the control. A box of half 6 let go at 0 px and sliding
    // at 120 px/s for a second crosses the seam at its speed, both ways.
    let floor = |width: u32| {
        let mut data = TilemapData::new(16, 16, width, 3);
        let solid = data.add_tile("solid");
        let layer = data.add_layer("solid", LayerKind::Collision);
        for col in 0..width {
            data.set_tile(layer, col, 2, solid);
        }
        data
    };
    let slide = |maps: &[(&str, u32, f32)], start: f32, speed: f32| {
        let mut world = seed_world();
        world.resource_mut::<PhysicsConfig>().gravity = Vec2::new(0.0, 900.0);
        for &(id, width, x) in maps {
            world
                .resource_mut::<TilemapRegistry>()
                .insert(id.into(), floor(width));
            let map = world.spawn();
            world.insert(map, TilemapInstance::new(id, Vec2::new(x, 0.0)));
        }
        let body = spawn_body(
            &mut world,
            RigidBody::dynamic(),
            Collider::aabb(Vec2::splat(6.0)),
            Vec2::new(start, 26.0),
            Vec2::new(speed, 0.0),
        );
        for _ in 0..60 {
            physics_step(&mut world);
        }
        world.get::<Position>(body).unwrap().0
    };
    for (start, speed) in [(100.0, 120.0), (220.0, -120.0)] {
        let control = slide(&[("one", 20, 0.0)], start, speed);
        let seam = slide(&[("a", 10, 0.0), ("b", 10, 160.0)], start, speed);
        assert!((control.x - start).abs() > 100.0, "on one map at {control}");
        assert!(
            (seam.x - control.x).abs() < 0.01,
            "from x {start} at {speed} px/s over the seam: at {seam}, on one map at {control}"
        );
    }
}
```

### A.19: `ch-gravity-scale-pairs`

From `perf-runs/20261107-core-hardening/ch-gravity-scale-pairs/test.patch`, appended to `crates/tungsten-core/src/tests/physics/step.rs`:

```rust
#[test]
fn one_bodys_gravity_scale_leaves_the_other_pair_boxes_alone() {
    // D-081's budget is per proxy: two resting boxes 16 px apart at gravity
    // 3,600 take their own fall's allowance and no pair, whatever the
    // gravity scale of a far box that never falls (it has no `Velocity`),
    // and that box keeps its own scaled allowance.
    let run = |scale: f32| {
        let mut world = seed_world();
        world.resource_mut::<PhysicsConfig>().gravity = Vec2::new(0.0, 3600.0);
        let far = world.spawn_with((
            Position(Vec2::new(1.0e6, 0.0)),
            Collider::aabb(Vec2::splat(8.0)),
            RigidBody::dynamic().with_gravity_scale(scale),
        ));
        let boxes = [0.0, 32.0].map(|x| {
            let (body, shape) = (RigidBody::dynamic(), Collider::aabb(Vec2::splat(8.0)));
            spawn_body(&mut world, body, shape, Vec2::new(x, 0.0), Vec2::ZERO)
        });
        physics_step(&mut world);
        let buffers = world.resource::<PhysicsBuffers>();
        let radius = |entity: Entity| {
            let index = buffers
                .proxies
                .iter()
                .position(|p| p.entity == Some(entity));
            buffers.pair_budgets[index.unwrap()].radius
        };
        (buffers.pairs.len(), radius(boxes[0]), radius(far))
    };
    let (_, unit, _) = run(1.0);
    let (pairs, near, far) = run(100.0);
    assert_eq!(
        (pairs, near),
        (0, unit),
        "a far box of gravity scale 100 lifted the near boxes' allowance"
    );
    let margin = (PAIR_MARGIN_SLOPS + 2.0) * PhysicsConfig::default().linear_slop;
    let own = 100.0 * (unit - margin) + margin;
    assert!(
        (far - own).abs() < 1e-3,
        "the scaled box's radius {far}, its own fall's {own}"
    );
}
```

### A.20: `ch-midair-sleep`

From `perf-runs/20261107-core-hardening/ch-midair-sleep/test.patch`, appended to `crates/tungsten-core/src/tests/physics/step.rs`:

```rust
/// ch-midair-sleep (`D-065`): under gravity 900 a free circle of gravity
/// scale 0.04 dropped from rest, or of scale 0.08 tossed up at 20 px/s, keeps
/// falling instead of sleeping in mid-air while slower than the threshold.
#[test]
fn a_free_body_under_a_small_gravity_scale_never_sleeps_in_the_air() {
    let mut hung = Vec::new();
    for (scale, toss) in [(0.04, 0.0), (0.08, -20.0)] {
        let mut world = seed_world();
        world.resource_mut::<PhysicsConfig>().gravity = Vec2::new(0.0, 900.0);
        let body = RigidBody::dynamic().with_gravity_scale(scale);
        let ball = spawn_body(
            &mut world,
            body,
            Collider::circle(4.0),
            Vec2::ZERO,
            Vec2::new(0.0, toss),
        );
        let mut slept = None;
        for step in 1..=630 {
            physics_step(&mut world);
            if slept.is_none() && is_sleeping(&world, ball) {
                slept = Some((step, world.get::<Position>(ball).unwrap().0.y));
            }
        }
        let y = world.get::<Position>(ball).unwrap().0.y;
        if y < 1000.0 {
            hung.push(format!(
                "scale {scale}, toss {toss}: slept at (step, y) {slept:?}, y after 630 steps {y}"
            ));
        }
    }
    assert!(
        hung.is_empty(),
        "{} bodies hang:\n{}",
        hung.len(),
        hung.join("\n")
    );
}
```

### A.21: `ch-tag-adoption`

From `perf-runs/20261107-core-hardening/ch-tag-adoption/test.patch`, appended to `crates/tungsten-core/src/tests/physics/step.rs`:

```rust
/// ch-tag-adoption (`D-065`): a box falling under gravity scale 0.04 falls
/// asleep half a pixel above a box asleep on a floor, its final-substep
/// contact with it speculative; when the floor goes and the lower box wakes, the
/// upper one wakes with it, as it does once it touches the lower one.
#[test]
fn an_island_asleep_beside_a_sleeper_adopts_its_tag() {
    let mut world = seed_world();
    world.resource_mut::<PhysicsConfig>().gravity = Vec2::new(0.0, 900.0);
    let floor = spawn_static(
        &mut world,
        Collider::aabb(Vec2::new(64.0, 8.0)),
        Vec2::new(0.0, 110.0),
    );
    let lower = spawn_body(
        &mut world,
        RigidBody::dynamic(),
        Collider::aabb(Vec2::splat(8.0)),
        Vec2::new(0.0, 94.0),
        Vec2::ZERO,
    );
    for _ in 0..150 {
        physics_step(&mut world);
        drain_events(&mut world);
    }
    assert!(is_sleeping(&world, lower), "the lower box never slept");
    // 5.04 px over the lower box's top (y 86); it falls 4.54 px by step 30.
    let slow = RigidBody::dynamic().with_gravity_scale(0.04);
    let upper = spawn_body(
        &mut world,
        slow,
        Collider::aabb(Vec2::splat(4.0)),
        Vec2::new(0.0, 76.96),
        Vec2::ZERO,
    );
    for _ in 0..30 {
        physics_step(&mut world);
        drain_events(&mut world);
    }
    let at = world.get::<Position>(upper).unwrap().0;
    assert!(
        is_sleeping(&world, upper) && at.y + 4.0 < 86.0,
        "the upper box did not sleep apart, at {at}"
    );
    world.despawn(floor);
    physics_step(&mut world);
    let woke = (!is_sleeping(&world, lower), !is_sleeping(&world, upper));
    for _ in 0..19 {
        physics_step(&mut world);
        drain_events(&mut world);
    }
    let fell = world.get::<Position>(upper).unwrap().0.y - at.y;
    assert!(
        woke == (true, true) && fell > 1.0,
        "upper box {} px over the lower one: (lower, upper) woke {woke:?}, upper fell {fell} px",
        86.0 - (at.y + 4.0)
    );
}
```

### A.22: `ch-spin-angle`

From `perf-runs/20261107-core-hardening/ch-spin-angle/test.patch`, appended to `crates/tungsten-core/src/tests/physics/step.rs`:

```rust
#[test]
fn a_spinner_far_from_angle_zero_keeps_its_rate() {
    // Physics review a3-4: the angle grew without bound in f32, so each turn
    // rounded: 60 steps at 100 rad/s from the angles a spinner reaches after
    // 1 h, 4.85 h and 23.3 h turned 97.5, 90 and 0 rad. The step keeps the
    // angle in [0, 2π) and turns it at its rate from any angle.
    let turn = f64::from(std::f32::consts::TAU);
    for start in [360_000.0_f32, 1_746_000.0, 8_388_608.0] {
        let (mut world, ball) = rolling_rig(0.0, 0.0, 0.0);
        *world.get_mut::<Spin>(ball).unwrap() = Spin {
            angle: start,
            angular_velocity: 100.0,
            prev_angle: start,
        };
        let (mut turned, mut last) = (0.0_f64, start);
        for _ in 0..60 {
            physics_step(&mut world);
            let angle = world.get::<Spin>(ball).unwrap().angle;
            // A step turns 100/60 rad, under half a turn.
            let half = turn / 2.0;
            turned += (f64::from(angle) - f64::from(last) + half).rem_euclid(turn) - half;
            last = angle;
        }
        assert!(
            (turned - 100.0).abs() < 0.01 && (0.0..std::f32::consts::TAU).contains(&last),
            "from {start}: turned {turned} rad of 100, angle {last}"
        );
    }
}
```

### A.23: `oracle-thin-box`

From `perf-runs/20261107-core-hardening/oracle-physics/new-2.md`, appended where its first comment says:

```rust
// Evidence for oracle new-2 (core hardening 2026-11-07, physics oracle).
// File and module: append to crates/tungsten-core/src/tests/physics/step.rs,
// which is `physics::step::tests` (`#[path]` from physics/step/mod.rs). The
// tests use that file's imports (`use super::*;` and its asset imports) and
// helpers: seed_world, spawn_body, spawn_static, drain_events, slope_kind.

/// A map of 16 px cells drawn by rows of letters in one collision layer:
/// `#` solid, `=` one-way, `a` to `l` slope kinds 1 to 12 (`slope_kind`'s)
/// and `.` empty, placed at `origin`.
fn spawn_oracle_map(world: &mut World, rows: &[&str], origin: Vec2) -> Entity {
    let (width, height) = (rows[0].len() as u32, rows.len() as u32);
    let mut data = TilemapData::new(16, 16, width, height);
    let solid = data.add_tile("solid");
    let ledge = data.add_tile("ledge");
    data.set_tile_props(
        ledge,
        TileProps {
            one_way: true,
            ..TileProps::default()
        },
    );
    let slopes: Vec<_> = (1..=12)
        .map(|code| {
            let tile = data.add_tile(format!("slope{code}"));
            data.set_tile_props(
                tile,
                TileProps {
                    slope: Some(slope_kind(code)),
                    ..TileProps::default()
                },
            );
            tile
        })
        .collect();
    let layer = data.add_layer("solid", LayerKind::Collision);
    for (row, letters) in rows.iter().enumerate() {
        for (col, letter) in letters.chars().enumerate() {
            let tile = match letter {
                '#' => solid,
                '=' => ledge,
                'a'..='l' => slopes[usize::from(letter as u8 - b'a')],
                _ => continue,
            };
            data.set_tile(layer, col as u32, row as u32, tile);
        }
    }
    world
        .resource_mut::<TilemapRegistry>()
        .insert("oracle".into(), data);
    let map = world.spawn();
    world.insert(map, TilemapInstance::new("oracle", origin));
    map
}

#[test]
fn oracle_new_2_a_thin_box_never_falls_through_a_solid_cell() {
    let mut world = seed_world();
    world.resource_mut::<PhysicsConfig>().gravity = Vec2::new(0.0, 900.0);
    world.resource_mut::<Time>().advance_frame(0.033333335);
    spawn_static(
        &mut world,
        Collider::aabb(Vec2::new(10.655905, 280.0)),
        Vec2::new(-410.6559, 0.0),
    );
    // Row 0, column 6 is the solid cell the box falls through; under it,
    // row 1 column 6 is a 2:1 high top-right ceiling slope (`k`).
    let rows = [
        "......#.......",
        ".....#k.......",
        "..............",
        "..............",
    ];
    let origin = Vec2::new(-331.63654, 164.18094);
    spawn_oracle_map(&mut world, &rows, origin);
    spawn_body(
        &mut world,
        RigidBody::kinematic(),
        Collider::aabb(Vec2::new(43.784332, 4.9526196)),
        Vec2::new(-203.39116, -106.263565),
        Vec2::new(36.15542, 0.0),
    );
    let thin = spawn_body(
        &mut world,
        RigidBody::dynamic().with_mass(4.37329),
        Collider::aabb(Vec2::new(4.423781, 19.343067)),
        Vec2::new(-302.789, 97.07454),
        Vec2::new(-89.312225, 115.71345),
    );
    spawn_body(
        &mut world,
        RigidBody::dynamic()
            .with_mass(1.838235)
            .with_gravity_scale(0.07977821),
        Collider::circle(18.590725),
        Vec2::new(-247.52002, 114.40555),
        Vec2::new(-15087.362, 504.69916),
    );
    spawn_body(
        &mut world,
        RigidBody::dynamic().with_mass(0.15357432),
        Collider::aabb(Vec2::new(30.40907, 14.215732)),
        Vec2::new(-356.38452, 125.57388),
        Vec2::new(-62.5766, 98.43069),
    );
    let cell_x = origin.x + 6.0 * 16.0..origin.x + 7.0 * 16.0;
    let cell_bottom = origin.y + 16.0;
    for step in 0..40 {
        physics_step(&mut world);
        drain_events(&mut world);
        let at = world.get::<Position>(thin).unwrap().0;
        assert!(
            !(cell_x.contains(&at.x) && at.y > cell_bottom),
            "step {step}: the thin box's centre {at} passed the solid cell's bottom face (y {cell_bottom})"
        );
    }
}
```

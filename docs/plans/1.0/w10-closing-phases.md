# W10 Closing phases C1–C3 — draft

- **status:** draft (skeleton)
- **goal:** Give every benchmark row a 1.0 budget and a dated baseline (C1), bring rows over budget inside it (C2), and run the release checklist on a release candidate (C3).
- **non-goals:** API changes after the freeze without an exception in a decision entry.
- **files to touch:** Set at graduation.
- **ordered steps:** Set at graduation. Candidates and their order: [implementation plan](implementation-plan.md) §5.
- **done-when:** Set at graduation. Sketch: [criteria](criteria.md) §8.8, per phase.

Skeleton. The scoping text stays in [criteria](criteria.md) §8.8 until this workstream graduates ([conventions](README.md#conventions)). Revised 2026-10-03 at `9cd5709`: Q12's gate. Revised 2026-10-03 on `db1177c`: C3's agent check and snapshot tool (amendments 16, 17).

## Placement

- **Proposed tier:** Must; C2 is gated by budget.
- **Candidates:** C1 before the freeze; C2 after it; C3 on each release candidate (all Phase 7). C3 includes RC-A9's agent-built feature (implementation plan amendment 16) and diffs the API against `D-104`'s snapshot (amendment 17). The [Phase 7 rewrite](../phase7-rewrite.md) proposes a bench rewrite and two optimization passes around C1 and a process, crush and docs step around C2; the feature gate adopts or rejects it, and graduation follows whichever stands.
- **Needs first:** The feature gate, so that the UI, kit and acceptance-game rows exist, and Q12 answered there (implementation plan §7); the profiling workflow extended for worker threads.
- **Feeds:** The freeze gate (C1's API fixes); the RC and 1.0 gates.
- **Owner questions:** Q12.
- **Decisions:** Per-row 1.0 budgets and C2's freeze-exception rule (extends `D-078`).

## Context digest

Written at graduation, in under ~500 tokens.

## Steps

Written at graduation.

## Done-when

Written at graduation.

## Follow-ups

- **C1's reading of the acceptance game (M76, 2026-10-22).** Example 01's frame with the ball pit full and enemies awake, for C1 to take as a benchmark row ([criteria](criteria.md) §8.8): `tests/game/frame_time.rs`, `a_full_pit_with_enemies_awake` (ignored), a New Game at Normal at dt 1/60 with 12,000 small balls settled in the pit under the rope bridge and six enemies awake within 600 px of the player on `bridge_b`, run on the headless harness (no GPU; render is the harness's copy of the extract). Release build with frame pointers, one sitting on the reference machine at `82327ce` plus M76's tree: the median over five 300-frame windows of `total` p50 6.306 ms, p95 6.438 ms; `update` p50 6.152 ms; `extract` p50 0.147 ms; `physics_step` p50 3.624 ms. Command: `RUSTFLAGS="-C force-frame-pointers=yes" CARGO_TARGET_DIR=target/e01d-capture cargo test --release -p example-01-platformer --locked --test game --no-run`, then from `examples/01_platformer` the binary with `--ignored --exact frame_time::a_full_pit_with_enemies_awake --nocapture`. Machine-local evidence: `perf-runs/20261022-e01d-frame/`. C1 decides whether a windowed row follows. The reading was taken on the 48 × 20 pit; the owner's last review of the same day halved the pit to 24 × 32 with the same 12,000 balls, so the reading is owed again on the new pit, in a quiet sitting, before C1 takes it.

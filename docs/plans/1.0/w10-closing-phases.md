# W10 Closing phases C1–C3 — draft

- **status:** draft (skeleton)
- **goal:** Give every benchmark row a 1.0 budget and a dated baseline (C1), bring rows over budget inside it (C2), and run the release checklist on a release candidate (C3).
- **non-goals:** API changes after the freeze without an exception in a decision entry.
- **files to touch:** Set at graduation.
- **ordered steps:** Set at graduation. Candidates and their order: [implementation plan](implementation-plan.md) §5.
- **done-when:** Set at graduation. Sketch: [criteria](criteria.md) §8.8, per phase.

Skeleton. The scoping text stays in [criteria](criteria.md) §8.8 until this workstream graduates ([conventions](README.md#conventions)). Revised 2026-10-03 at `9cd5709`: Q12's gate.

## Placement

- **Proposed tier:** Must; C2 is gated by budget.
- **Candidates:** C1 before the freeze; C2 after it; C3 on each release candidate (all Phase 7).
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

None yet.

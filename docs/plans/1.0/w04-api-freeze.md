# W4 API stabilization and freeze — draft

- **status:** draft (skeleton)
- **goal:** A written stability policy, every break landed before the freeze, a documented public surface, and a snapshot that 1.x releases are checked against.
- **non-goals:** Holding breaks for 2.0 that could land before the freeze; API changes after the freeze without an exception in a decision entry (§8.8 C2).
- **files to touch:** Set at graduation.
- **ordered steps:** Set at graduation. Candidates and their order: [implementation plan](implementation-plan.md) §2, §5.
- **done-when:** Set at graduation. Sketch: [criteria](criteria.md) §7 lists what the freeze covers; the implementation plan's freeze gate says when it passes.

Skeleton. The scoping text stays in [criteria](criteria.md) §7 until this workstream graduates ([conventions](README.md#conventions)).

## Placement

- **Proposed tier:** Must if definition B.
- **Candidates:** W4a, the stability and `wgpu`/`winit` policy (definition gate; decision entries only); the break ledger below; W4b, the freeze (Phase 7).
- **Needs first:** Q1 and Q11.
- **Feeds:** Every workstream that adds public surface designs against W4a.
- **Owner questions:** Q2, Q11, Q24.
- **Decisions:** 1.0 definition and stability policy: semver scope, MSRV, platform tiers, the `wgpu`/`winit` policy.

## Break ledger

Every public-API break bound for 1.0, with its source and where it landed. Seeded from criteria §7 at graduation; until then §7 is the list.

| Break | From | Status | Landed in |
| --- | --- | --- | --- |
| — | — | — | — |

## Context digest

Written at graduation, in under ~500 tokens.

## Steps

Written at graduation.

## Done-when

Written at graduation.

## Follow-ups

None yet.

# W8 Correctness burn-down — draft

- **status:** draft (skeleton)
- **goal:** Close the known-issue P3s and the engine-finding bugs before the freeze, with capture completion first.
- **non-goals:** New features; performance work.
- **files to touch:** Set at graduation.
- **ordered steps:** Set at graduation. Candidates and their order: [implementation plan](implementation-plan.md) §3, §4.
- **done-when:** Set at graduation. None yet; QA passes in the 0.40 shape close Phases 5 and 6, with W8b inside the second (implementation plan amendment 18).

Skeleton. The scoping text stays in [criteria](criteria.md) §2.1 and §8.1 until this workstream graduates ([conventions](README.md#conventions)). Revised 2026-10-03 on `db1177c`: amendments 10 and 18 in the candidates.

## Placement

- **Proposed tier:** Must.
- **Candidates:** W8a, the capture-completion contract and the engine-finding bugs, the burst latch among them (Phase 5, Track A; amendment 10); W8b, the shader ID allocator, queued transitions and the remaining P3s, inside the Phase 6 QA pass (amendment 18). The Phase 5 QA pass, after Track C, audits what Phase 5 changed.
- **Needs first:** Nothing.
- **Feeds:** W1 M2's direct-versus-capture check; W4's break ledger (the render result type, `App::run` returning runtime errors); W14b `check` (the once-per-ID warning for an unknown sprite).
- **Owner questions:** None.
- **Decisions:** None listed yet; the render result type is a break ([break ledger](w04-api-freeze.md#break-ledger)).

## Context digest

Written at graduation, in under ~500 tokens.

## Steps

Written at graduation.

## Done-when

Written at graduation.

## Follow-ups

None yet.

# W8 Correctness burn-down — draft

- **status:** draft (skeleton)
- **goal:** Close the known-issue P3s and the engine-finding bugs before the freeze, with capture completion first.
- **non-goals:** New features; performance work.
- **files to touch:** Set at graduation.
- **ordered steps:** Set at graduation. Candidates and their order: [implementation plan](implementation-plan.md) §3, §4.
- **done-when:** Set at graduation. None yet; criteria §8.1 proposes one pass, like the P2 correctness pass.

Skeleton. The scoping text stays in [criteria](criteria.md) §2.1 and §8.1 until this workstream graduates ([conventions](README.md#conventions)).

## Placement

- **Proposed tier:** Must.
- **Candidates:** W8a, the capture-completion contract and the engine-finding bugs (Phase 5, Track A); W8b, the shader ID allocator, queued transitions and the remaining P3s (Phase 6).
- **Needs first:** Nothing.
- **Feeds:** W1 M2's direct-versus-capture check; W4's break ledger (the render result type, `App::run` returning runtime errors); W14b `check` (the once-per-ID warning for an unknown sprite).
- **Owner questions:** None.
- **Decisions:** None listed yet; the render result type is a break (§7).

## Context digest

Written at graduation, in under ~500 tokens.

## Steps

Written at graduation.

## Done-when

Written at graduation.

## Follow-ups

None yet.

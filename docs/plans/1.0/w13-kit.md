# W13 `tungsten-kit` — draft

- **status:** draft (skeleton)
- **goal:** A crate of generic 2D components and systems in four groups (basics, transform hierarchy, character controllers, gameplay helpers), each item with a user, rustdoc and a harness test.
- **non-goals:** Items with no user by the freeze (backlog); render types in the kit (`D-007`).
- **files to touch:** Set at graduation.
- **ordered steps:** Set at graduation. Candidates and their order: [implementation plan](implementation-plan.md) §4.
- **done-when:** Set at graduation. Sketch: [criteria](criteria.md) §8.4.

Skeleton. The scoping text stays in [criteria](criteria.md) §8.4 until this workstream graduates ([conventions](README.md#conventions)).

## Placement

- **Proposed tier:** Must per item, if a user ships it by the freeze.
- **Candidates:** W13a basics; W13b transform hierarchy; W13c character controllers; W13d gameplay helpers; the spawner after W16b (all Phase 6).
- **Needs first:** W15a (`Schedule` and `Plugin` in core); W3a, for timers; W16a, so that items register as they land; W6a, for W13c and W13d.
- **Feeds:** The template; examples 01, 03 and 04; the acceptance game.
- **Owner questions:** Q21, Q22, Q24.
- **Decisions:** `tungsten-kit`: dependency direction, admission rule, and a row in `AGENTS.md`'s "Where code goes".

## Context digest

Written at graduation, in under ~500 tokens.

## Steps

Written at graduation.

## Done-when

Written at graduation.

## Follow-ups

None yet.

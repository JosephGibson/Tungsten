# W5 Input — draft

- **status:** draft (skeleton)
- **goal:** Gamepad play through the action map, plus whatever focus, modifier and DPI work UI M0b leaves.
- **non-goals:** Focus-loss release, modifiers, the scale-factor resource and real `KeyCode` variants, which UI M0b covers (implementation plan, amendment 2); IME (backlog).
- **files to touch:** Set at graduation.
- **ordered steps:** Set at graduation. Candidates and their order: [implementation plan](implementation-plan.md) §4.
- **done-when:** Set at graduation. None yet in criteria §8.1.

Skeleton. The scoping text stays in [criteria](criteria.md) §8.1 until this workstream graduates ([conventions](README.md#conventions)). Revised 2026-10-03 at `9cd5709`: the `libudev-dev` note.

## Placement

- **Proposed tier:** Should; Must if the acceptance game uses a gamepad (Q9).
- **Candidates:** W5a, gamepad backend (Phase 6).
- **Needs first:** W1 M0b; a `D-015` rule 1 decision for the gamepad crate, after checking its Linux backend for threads and `libudev` (the workflows stopped installing `libudev-dev` in 0.40, QA step 12).
- **Feeds:** UI controller navigation through `ui_*` actions; the acceptance game; RC-A5.
- **Owner questions:** Q9.
- **Decisions:** A gamepad dependency (`D-015` rule 1).

## Context digest

Written at graduation, in under ~500 tokens.

## Steps

Written at graduation.

## Done-when

Written at graduation.

## Follow-ups

None yet.

# W14 Tooling — draft

- **status:** draft (skeleton)
- **goal:** A headless harness that runs the real stage order, and a project CLI with `check`, `package` and `new`.
- **non-goals:** JSON schemas for config files; live editing in the game (backlog).
- **files to touch:** Set at graduation.
- **ordered steps:** Set at graduation. Candidates and their order: [implementation plan](implementation-plan.md) §3, §4.
- **done-when:** Set at graduation. Sketch: [criteria](criteria.md) §8.5.

Skeleton. The scoping text stays in [criteria](criteria.md) §8.5 until this workstream graduates ([conventions](README.md#conventions)).

## Placement

- **Proposed tier:** Harness Must; CLI `check` and `package` Must, `new` Should.
- **Candidates:** W14a, the headless harness behind a `testing` feature (Phase 5, Track A); W14b, the CLI (Phase 6): `check` after W16b, `package` after W9a and W11a, `new` after W12a. Package `tungsten-cli` in `tools/cli/`, binary `tungsten` (implementation plan, amendment 11).
- **Needs first:** For W14a, an `App` frame body that runs without a window, surface or audio device.
- **Feeds:** Every later test; W1 M1's `UiHarness` (implementation plan, amendment 5); RC-A7.
- **Owner questions:** Q20.
- **Decisions:** Headless harness behind a `testing` feature; the project CLI and its dependencies (`D-015`).

## Context digest

Written at graduation, in under ~500 tokens.

## Steps

Written at graduation.

## Done-when

Written at graduation.

## Follow-ups

None yet.

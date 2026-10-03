# W11 Shipping basics — draft

- **status:** draft (skeleton)
- **goal:** A per-user folder with a log file and crash files that symbolize, typed settings with a user layer, and versioned save slots.
- **non-goals:** Serializing the `World`; crash files for signals; resolving assets relative to the executable.
- **files to touch:** Set at graduation.
- **ordered steps:** Set at graduation. Candidates and their order: [implementation plan](implementation-plan.md) §3, §4.
- **done-when:** Set at graduation. Sketch: [criteria](criteria.md) §8.2.

Skeleton. The scoping text stays in [criteria](criteria.md) §8.2 until this workstream graduates ([conventions](README.md#conventions)).

## Placement

- **Proposed tier:** Must; save slots follow the acceptance game (Q16).
- **Candidates:** W11a, user folder, log file, panic hook, release symbols with the CI symbolization probe and no console on Windows (Phase 5, Track A); W11b, settings (Phase 6, before W1 M4); W11c, save slots if Q16 says so (Phase 6).
- **Needs first:** The game identifier in `tungsten.json`, shared with W12a.
- **Feeds:** W12a (logging leaves the examples' `main.rs`); W1 M5's log console (one logger, implementation plan amendment 4); W14b `package` (debug files kept apart); W7a.
- **Owner questions:** Q14, Q15, Q16.
- **Decisions:** Per-user folder, log file and crash report (extends `D-008`); release symbols (extends `D-071`); settings and save slots (amends `D-045`, extends `D-008`, narrows DESIGN's save/load non-commitment), with an invalid user file falling back instead of failing.

## Context digest

Written at graduation, in under ~500 tokens.

## Steps

Written at graduation.

## Done-when

Written at graduation.

## Follow-ups

None yet.

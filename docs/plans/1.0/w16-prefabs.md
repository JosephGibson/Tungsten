# W16 Prefabs — draft

- **status:** draft (skeleton)
- **goal:** Registered components in scenes, prefab files in a manifest section, and Tiled objects that spawn prefabs.
- **non-goals:** World snapshots and saves by reflection; nested prefabs; patching live entities on hot reload (backlog).
- **files to touch:** Set at graduation.
- **ordered steps:** Set at graduation. Candidates and their order: [implementation plan](implementation-plan.md) §4.
- **done-when:** Set at graduation. Sketch: [criteria](criteria.md) §8.7.

Skeleton. The scoping text stays in [criteria](criteria.md) §8.7 until this workstream graduates ([conventions](README.md#conventions)).

## Placement

- **Proposed tier:** Must.
- **Candidates:** W16a, the component registry and `components` in scene entries; W16b, prefab assets, `spawn_prefab` and Tiled object classes (Phase 6; split per implementation plan amendment 3).
- **Needs first:** W15a, for registration.
- **Feeds:** W13's spawner and registered kit components; W14b `check`; the template's player and enemy.
- **Owner questions:** None open.
- **Decisions:** Component registry and prefabs: a `prefabs` manifest section and a row in `AGENTS.md`'s assets table (extends `D-046`).

## Context digest

Written at graduation, in under ~500 tokens.

## Steps

Written at graduation.

## Done-when

Written at graduation.

## Follow-ups

None yet.

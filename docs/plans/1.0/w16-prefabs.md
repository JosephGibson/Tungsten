# W16 Prefabs — draft

- **status:** draft (skeleton)
- **goal:** Registered components in scenes, prefab files in a manifest section, and Tiled objects that spawn prefabs.
- **non-goals:** World snapshots and saves by reflection; nested prefabs; patching live entities on hot reload (backlog).
- **files to touch:** Set at graduation.
- **ordered steps:** Set at graduation. Candidates and their order: [implementation plan](implementation-plan.md) §4.
- **done-when:** Set at graduation. Sketch: [criteria](criteria.md) §8.7.

Skeleton. The scoping text stays in [criteria](criteria.md) §8.7 until this workstream graduates ([conventions](README.md#conventions)). Revised 2026-10-06 on `01f9c55`, uncommitted: W16b gains the tilemap builder (`D-132`). Revised 2026-10-14, uncommitted: W16b landed in 0.71 (M51, `D-157`): prefab files in a `prefabs` manifest section, spawned by ID at once or through the command buffer; Tiled object layers whose object classes name prefabs, their properties over the prefabs' components; the asset-checked registration form with `particle_emitter` and `tilemap` on it; the `TilemapData` builder; the kit's spawner. Both candidates have landed.

## Placement

- **Proposed tier:** Must.
- **Candidates:** W16a, the component registry and `components` in scene entries, landed in M48 (`D-153`, 0.67); W16b, landed in M51 (`D-157`, 0.71): prefab assets, `spawn_prefab`, Tiled object classes and a public `TilemapData` builder with tile setters for maps built in code, which the acceptance game's generated arena needs (frame-loop gate, `D-132`; Phase 6; split per implementation plan amendment 3).
- **Needs first:** W15a, for registration.
- **Feeds:** W13's spawner and registered kit components; W14b `check`; the template's player and enemy.
- **Owner questions:** None open.
- **Decisions:** Component registry and prefabs: a `prefabs` manifest section and a row in the [assets](../../assets.md) table (extends `D-046`; `D-106` moved the table out of `AGENTS.md`); the tilemap builder's shape, in W16b's plan and its step 1 API critique (`D-150`).

## Context digest

Written at graduation, in under ~500 tokens.

## Steps

Written at graduation.

## Done-when

Written at graduation.

## Follow-ups

M51's (`docs/plans/archive/1.0/phase6-milestone-51-prefabs-tiled-objects.md`); W16a's follow-up landed with it:

1. **Nested prefabs, scene entries that instance prefabs, and live patching on hot reload** stay in the [backlog](backlog-1.x.md): a prefab file holds no prefab, a scene entry names components, and a reloaded prefab changes later spawns only.
2. **Rotated and templated Tiled objects, prefab transforms and overrides** (M51 Q2, Q8, Q9): a classed rotated object and a templated one fail the load, a prefab spawns unrotated at scale 1, and an object's properties reach only `components`; each waits in the backlog for a game that needs it.
3. **The kit's `animation` component on the asset-checked form** (M51 Q17): an unknown clip still leaves the sprite as it is; home: W12b, when example 01 moves its animations onto the kit.
4. **The assets table's Particle row**: `docs/assets.md` gained the Tilemap and Prefab rows, and the `particles` section still has none ([known issues](../../known-issues.md)).

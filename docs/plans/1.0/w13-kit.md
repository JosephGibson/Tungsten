# W13 `tungsten-kit` — draft

- **status:** draft (skeleton)
- **goal:** A crate of generic 2D components and systems in four groups (basics, transform hierarchy, character controllers, gameplay helpers), each item with a user, rustdoc and a harness test.
- **non-goals:** Items with no user by the freeze (backlog); render types in the kit (`D-007`).
- **files to touch:** Set at graduation.
- **ordered steps:** Set at graduation. Candidates and their order: [implementation plan](implementation-plan.md) §4.
- **done-when:** Set at graduation. Sketch: [criteria](criteria.md) §8.4.

Skeleton. The scoping text stays in [criteria](criteria.md) §8.4 until this workstream graduates ([conventions](README.md#conventions)). Revised 2026-10-03 on `db1177c`: the definition gate's tier timing and Q24's answer. Revised 2026-10-05 on `01f9c55`, uncommitted: tiers proposed from the acceptance game's mechanics rows. Revised 2026-10-06, uncommitted: the frame-loop gate confirmed them and answered Q21 (`D-132`). Revised 2026-10-14, uncommitted: W13a and W13b landed in 0.69 (M50, `D-155`): `tungsten-kit` on core alone, re-exported as `tungsten::kit` behind a default feature, with opt-in plugins per item and harness tests in the umbrella; its value types keep private fields behind constructors in place of `#[non_exhaustive]`.

## Placement

- **Tier:** Must per item, if a user ships it by the freeze. From the acceptance game's mechanics rows, agreed with the owner on 2026-10-05 ([acceptance game](acceptance-game.md#proposed-tiers)): every item has the game as a user except `cursor_to_world`, the platformer controller and the path follower, whose only user is example 01, so those three are Must only if it moves onto the kit by the freeze (W12b). Confirmed at the frame-loop gate on 2026-10-06 (`D-132`; [implementation plan](implementation-plan.md) §11). Physics bodies live on root entities only (Q21): the hierarchy propagates after `physics_sync`, and the game places its orbiting blades each fixed step.
- **Candidates:** W13a basics and W13b transform hierarchy as one candidate; W13c character controllers and W13d gameplay helpers as another; the spawner after W16b (all Phase 6; implementation plan amendment 3 and §4's cards).
- **Needs first:** W15a (`Schedule` and `Plugin` in core); W3a, for timers; W16a, so that items register as they land; W6a, for W13c and W13d.
- **Feeds:** The template; examples 01, 03 and 04; the acceptance game.
- **Owner questions:** Q22. Q21 answered at the frame-loop gate: bodies on roots only (`D-132`). Q24 answered at the definition gate: inside the 1.0 promise (`D-103`).
- **Decisions:** `tungsten-kit`: dependency direction, admission rule, and a row in `AGENTS.md`'s "Where code goes". Its stability is `D-103`'s: inside the promise, with `#[non_exhaustive]` on kit settings structs.

## Context digest

Written at graduation, in under ~500 tokens.

## Steps

Written at graduation.

## Done-when

Written at graduation.

## Follow-ups

Each waits for a user (M50's plan, archived at `docs/plans/archive/1.0/phase6-milestone-50-kit-basics-hierarchy.md`):

- Reparenting and detaching through the kit; today a game removes `Parent`, which detaches the child where it stands.
- Spawning children through the command buffer; `spawn_child` is immediate, and a child spawned at the flush would draw one frame before its first propagation.
- Rotation and scale inheritance, with a pivot design for the default extract, which turns a sprite about its centre (M50 Q7).
- A `KitPlugins` set, if games add most of the kit's plugins together.
- Scene components for `lifetime` and `despawn_outside`, when W16b's prefabs need them (M50 Q8).
- An exactly-once event reader in core, if a second reader needs `SoundOnEvent`'s handled-count cursor.

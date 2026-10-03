# 1.x backlog — draft

- **status:** draft
- **goal:** Keep everything the 1.0 plan defers in one place, with where it was deferred and what would bring it back, so that the must list can be cut without losing work.
- **non-goals:** Scoping or ordering 1.x work. DESIGN's non-commitments, which are not deferrals.
- **files to touch:** This file.
- **ordered steps:** Add a row whenever a workstream, a gate or the acceptance game cuts an item; review the list at the feature gate.
- **done-when:** At the 1.0 gate every row names its source; after 1.0 this file seeds the 1.x plans.

Sources are [criteria](criteria.md) sections unless another file is named.

| Item | Deferred in | Comes back when |
| --- | --- | --- |
| Editable text fields and IME | §4; [w01](w01-ui-text-suite.md) §11 | Q8 puts a text field in 1.0, or a game needs name entry |
| Accessibility, docking, virtualization, localization, data-defined screens, world-anchored text, bitmap fonts, an editable inspector, a command console | w01 §11, "Later" | Each gets its own plan |
| JSON schemas for config files; live editing in the game | §8.5 | W1's widgets have shipped |
| Nested prefabs; patching live entities on hot reload | §8.7 | A game asks |
| World snapshots and saves by reflection | §8.2, §8.7 | Component reflection exists |
| Crash files for signals (segfault, driver abort) | §8.2 | A game ships and needs them |
| Parallel system scheduler | §5.4 | Q6 has fixed the bounds and a CPU-bound game row needs it |
| Parallel custom extracts | §5.3, R1 | A custom extract dominates a row |
| Pipelined render thread (R4) | Proposed in the [implementation plan](implementation-plan.md) §6 | C1 shows a game-frame row over budget that R4 would fix |
| Parallel asset decode (R5) | §5.3 | Startup metrics (proposal T2) ask for it |
| Kit items with no user by the freeze | §8.4 | A user appears |
| W6 features the acceptance game does not use | §3 | A game asks |

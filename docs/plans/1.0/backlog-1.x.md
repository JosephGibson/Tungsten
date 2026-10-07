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
| Editable text fields and IME | §4; [w01](w01-ui-text-suite.md) §11; the definition gate (Q8, `D-102`) | A game needs name entry |
| Accessibility, docking, virtualization, localization, data-defined screens, an editable inspector, a command console | w01 §11, "Later" | Each gets its own plan |
| Owned glyph renderer (T2) and what needs it: per-glyph transforms and colour animation (wave, pop; the damage numbers' per-glyph version), in-shader outline, shadow and glow, bitmap fonts, premultiplied alpha, an initial atlas size, per-batch glyph release, text preparation off the main thread (W2 R3) | The glyph gate (`D-126`) | A game or the UI fixture needs one of those effects; T2 amends `D-026` |
| Integer scene scaling (`display.scale_mode`) | The glyph gate (`D-127`) | A pixel-art game asks; the UI stays at native resolution |
| JSON schemas for config files; live editing in the game | [w14](w14-tooling.md) | W1's widgets have shipped |
| Nested prefabs; patching live entities on hot reload | §8.7 | A game asks |
| World snapshots and saves by reflection | [w11](w11-shipping-basics.md), §8.7 | Component reflection exists |
| Crash files for signals (segfault, driver abort) | [w11](w11-shipping-basics.md) | A game ships and needs them |
| Parallel system scheduler | [w02](w02-multi-core-rendering.md#decisions-the-pass-needs-first) | A CPU-bound game row needs it; Q6 set no bounds (`D-131`), so it is opt-in per system over `Sync` types |
| Fork-join default and tilemap extracts (R1) | The frame-loop gate (`D-131`), on the spike in `perf-runs/20261005-w2-r1-spike/` | C1 shows an extract-bound game row on a machine whose CPU has the memory bus to itself, or the bytes-per-instance work (C2) leaves an extract CPU-bound; it builds on `D-130`'s slice queries |
| Parallel custom extracts | [w02](w02-multi-core-rendering.md#options), R1 | A custom extract dominates a row |
| Pipelined render thread (R4) | The definition gate (Q4, `D-102`), as the [implementation plan](implementation-plan.md) §6 proposed | C1 shows a game-frame row over budget that R4 would fix |
| Parallel asset decode (R5) | [w02](w02-multi-core-rendering.md#options) | Startup metrics (proposal T2) ask for it |
| Kit items with no user by the freeze | §8.4 | A user appears |
| W6 features the acceptance game does not use | §3 | A game asks |
| Audio pause | The frame-loop gate (`D-132`): music plays through pause and level-up, effects are short | A game pauses with long effects playing |
| Deep-pile lever: `contact_hertz` 60 on the constant fixed step ([known issues](../../known-issues.md)) | [w03](w03-frame-loop.md#follow-ups) follow-up 1; M41 Q8 (`D-137`) | A game's pile needs the stiffer contacts; it moves the physics hash and every engine pile |

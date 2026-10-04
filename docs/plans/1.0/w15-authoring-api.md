# W15 Authoring API — draft

- **status:** draft (skeleton)
- **goal:** Named stages and plugins with stage-local ordering, additive extracts, tuple queries and bundles, all landed before the freeze.
- **non-goals:** A parallel system scheduler (§5.4); changing `02_bench`'s explicit wiring.
- **files to touch:** Set at graduation.
- **ordered steps:** Set at graduation. Candidates and their order: [implementation plan](implementation-plan.md) §3.
- **done-when:** Set at graduation. Sketch: [criteria](criteria.md) §8.6.

Skeleton. The scoping text stays in [criteria](criteria.md) §8.6 until this workstream graduates ([conventions](README.md#conventions)), at the frame-loop gate (definition gate, agenda item 12).

## Placement

- **Proposed tier:** Must, before the freeze.
- **Candidates:** The spike and design (Phase 5, Track B); W15a stages and plugins, W15b tuple queries and bundles, W15c additive extracts (Track C).
- **Needs first:** The frame-loop gate; W14a.
- **Feeds:** W12 and the template's `main.rs`; W13; W16; W2 R1's column slices; W3.
- **Owner questions:** Q6, through the `Send`/`Sync` policy its queries carry.
- **Decisions:** Stages and plugins, in W3's frame-loop decision set; tuple queries and bundles; additive extracts.

## Context digest

Written at graduation, in under ~500 tokens.

## Steps

Written at graduation.

## Done-when

Written at graduation.

## Follow-ups

None yet.

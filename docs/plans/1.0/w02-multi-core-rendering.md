# W2 Multi-core rendering pass — draft

- **status:** draft (skeleton)
- **goal:** Use more cores where the frame is bound by the extract or by encoding, with results bit-identical to the serial path at every worker count.
- **non-goals:** Parallel physics (`D-067`); a parallel system scheduler (§5.4); parallel custom extracts; frames limited by the GPU, which no CPU work speeds up.
- **files to touch:** Set at graduation.
- **ordered steps:** Set at graduation. Candidates and their order: [implementation plan](implementation-plan.md) §3, §4.
- **done-when:** Set at graduation. Sketch: [criteria](criteria.md) §5.5

Skeleton. The scoping text stays in [criteria](criteria.md) §5 until this workstream graduates ([conventions](README.md#conventions)).

## Placement

- **Proposed tier:** Must.
- **Candidates:** R0 (Phase 5, Track A); the R1 spike (Track B); R1 and R2 (Track C); R4 only if approved (Phase 6; proposed for 1.x); R3 after W1's glyph gate; R5 only if startup metrics ask.
- **Needs first:** The definition gate, because R0's interned IDs break `Sprite.asset_id`; capture rules extended for worker threads (§12) before the R1 spike; W15b's column slices for R1.
- **Feeds:** W1's glyph gate (the R4 answer); W4's break ledger (interned IDs, the column-slice query); C1 and C2 on `gpu-throughput`, `particles` and `integrated`.
- **Owner questions:** Q4, Q5, Q6.
- **Decisions:** Threading rule, replacing the two-thread limit in `AGENTS.md`; worker mechanism; `Send`/`Sync` policy; interned asset IDs; R4, if approved.

## Context digest

Written at graduation, in under ~500 tokens.

## Steps

Written at graduation.

## Done-when

Written at graduation.

## Follow-ups

None yet.

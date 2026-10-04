# W2 Multi-core rendering pass — draft

- **status:** draft (skeleton)
- **goal:** Use more cores where the frame is bound by the extract or by encoding, with results bit-identical to the serial path at every worker count.
- **non-goals:** Parallel physics (`D-067`); a parallel system scheduler (§5.4); parallel custom extracts; frames limited by the GPU, which no CPU work speeds up; the pipelined render thread (R4), moved to 1.x at the definition gate (`D-102`).
- **files to touch:** Set at graduation.
- **ordered steps:** Set at graduation. Candidates and their order: [implementation plan](implementation-plan.md) §3, §4.
- **done-when:** Set at graduation. Sketch: [criteria](criteria.md) §5.5

Skeleton. The scoping text stays in [criteria](criteria.md) §5 until this workstream graduates ([conventions](README.md#conventions)), at the frame-loop gate (definition gate, agenda item 12). Revised 2026-10-03 on `db1177c`: question 4's answer and amendment 7.

## Placement

- **Proposed tier:** Must.
- **Candidates:** R0 and R2's late-acquire split (Phase 5, Track A); the R1 spike (Track B); R1 (Track C); R4 in 1.x, back only if C1 shows a game-frame row over budget that it would fix (Q4, `D-102`); R3 after W1's glyph gate; R5 only if startup metrics ask.
- **Needs first:** The definition gate, because R0's interned IDs break `Sprite.asset_id`; capture rules extended for worker threads (§12) before the R1 spike; W15b's column slices for R1.
- **Feeds:** W1 M0a and M1, which run in Track A now that R4 is 1.x (implementation plan amendment 7); W1 M2, which builds on R2's split final pass; W4's break ledger (interned IDs, the column-slice query); C1 and C2 on `gpu-throughput`, `particles` and `integrated`.
- **Landed:** R0 in 0.45 (M33): interned sprite IDs (`D-113`) and extract culling (`D-114`). The R1 spike measures on that tree.
- **Owner questions:** Q5, Q6. Q4 answered at the definition gate: R1 and R2 only (`D-102`).
- **Decisions:** Threading rule, replacing the two-thread limit in `AGENTS.md`; worker mechanism; `Send`/`Sync` policy; interned asset IDs; R4, if it comes back from 1.x.

## Context digest

Written at graduation, in under ~500 tokens.

## Steps

Written at graduation.

## Done-when

Written at graduation.

## Follow-ups

None yet.

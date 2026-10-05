# W7 Platforms — draft

- **status:** draft (skeleton)
- **goal:** Support tiers stated in the README, with Linux certified and Windows and macOS on tiers that available hosts can back.
- **non-goals:** Web or mobile targets (`D-003`, native only, stands).
- **files to touch:** Set at graduation.
- **ordered steps:** Set at graduation. Candidates and their order: [implementation plan](implementation-plan.md) §3, §5.
- **done-when:** Set at graduation. None yet in criteria §8.1.

Skeleton. The scoping text stays in [criteria](criteria.md) §8.1 until this workstream graduates ([conventions](README.md#conventions)).

## Placement

- **Proposed tier:** Must, tiers at least.
- **Candidates:** W7a, a CPU-only Windows test job on the `windows-2025` image, informational under `D-070` (Phase 5, beside W11a; implementation plan amendment 9); W7b, tiers in the README and a macOS build if Q3 says so (Phase 7).
- **Needs first:** W11a, for what W7a tests.
- **Landed:** W7a in 0.48 (M35, `D-121`): CI's `windows-tests` job runs `cargo test --workspace --locked` on `windows-2025`, informational. Its done-when, that its first run on the release pull request passes or each failure is filed in known issues before the merge, is the owner's read of that run (M35 Q10).
- **Feeds:** RC-D1, RC-D4, RC-S1, RC-S2.
- **Owner questions:** Q3.
- **Decisions:** Platform tiers, inside the 1.0 stability policy.

## Context digest

Written at graduation, in under ~500 tokens.

## Steps

Written at graduation.

## Done-when

Written at graduation.

## Follow-ups

None yet.

# W3 Frame loop v2 — draft

- **status:** draft (skeleton)
- **goal:** A fixed-step accumulator with render interpolation, a game clock (real and game time, time scale, pause) with timers, and one stage map shared with W1 and W15.
- **non-goals:** A timer system or timer event queue in the first version; replays, which the fixed step makes possible but this workstream does not build.
- **files to touch:** Set at graduation.
- **ordered steps:** Set at graduation. Candidates and their order: [implementation plan](implementation-plan.md) §3.
- **done-when:** Set at graduation. None yet; criteria §6 asks that smoke and benchmark digests stay unchanged at the pinned 1/60 s.

Skeleton. The scoping text stays in [criteria](criteria.md) §6 until this workstream graduates ([conventions](README.md#conventions)).

## Placement

- **Proposed tier:** Must.
- **Candidates:** The design (Phase 5, Track B, with W15's stages and W1 M3's routing stages); W3a `Time` and `Timer`, then W3b fixed step and interpolation (Track C).
- **Needs first:** The frame-loop gate; W14a, so that stage order is tested on the real frame.
- **Feeds:** W13 (timers; interpolation of roots before propagation); W6a (kinematic bodies in the fixed step); the deep-pile limit's `contact_hertz` lever; W1 M3; R4's hand-off point.
- **Owner questions:** Q7, Q13.
- **Decisions:** Fixed-step accumulator and interpolation (amends `D-088` and `D-094`, touches the `D-018` seam); game clock and timers (amends `D-088`'s single dt and settles which clock `D-093`'s transitions follow).

## Context digest

Written at graduation, in under ~500 tokens.

## Steps

Written at graduation.

## Done-when

Written at graduation.

## Follow-ups

None yet.

# W6 Gameplay systems — draft

- **status:** draft (skeleton)
- **goal:** The physics and audio features the kit and the acceptance game need: kinematic bodies, sensors, and shape and ray queries first; then layers and masks, and audio voice handles, pause, fades and buses as the game asks.
- **non-goals:** Features the acceptance game does not use (backlog); the burst-latch fix, which moves to W8a (implementation plan, amendment 10).
- **files to touch:** Set at graduation.
- **ordered steps:** Set at graduation. Candidates and their order: [implementation plan](implementation-plan.md) §4.
- **done-when:** Set at graduation. None yet; `just physics-release` stays green, with determinism and containment hashes unchanged for existing scenes.

Skeleton. The scoping text stays in [criteria](criteria.md) §3 and §8.1 until this workstream graduates ([conventions](README.md#conventions)). Revised 2026-10-03 at `9cd5709`: the decisions line.

## Placement

- **Proposed tier:** Kinematic bodies (G1), sensors (G2) and shape queries Must; the rest Should, cut by the acceptance game.
- **Candidates:** W6a kinematic bodies, sensors, shape and ray queries; W6b layers and masks; W6c audio voices, pause, fades and buses (all Phase 6).
- **Needs first:** W3b, so that kinematic motion is written once against the fixed step; W15a, for physics as a plugin.
- **Feeds:** W13c controllers; W13d trigger zones and path follower; W11b volume per bus; the acceptance game.
- **Owner questions:** Q10.
- **Decisions:** W6's bullet in criteria §11 (added 2026-10-03): kinematic bodies, sensors and shape queries amend `D-033`'s body model and touch `D-064` and `D-065`; audio voices and buses set the audio policy for `D-034`'s command ring.

## Context digest

Written at graduation, in under ~500 tokens.

## Steps

Written at graduation.

## Done-when

Written at graduation.

## Follow-ups

None yet.

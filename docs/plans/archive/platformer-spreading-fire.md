# Spreading fire on small balls

status: done
goal: fire hazards ignite small balls, contact spreads fire, and each ball burns for 15 seconds before becoming permanently charred.
non-goals: engine changes, new physics, ignition of normal balls or player damage from burning small balls.
files to touch: examples/01_platformer/src/{main,burning,gameplay,setup,extract}.rs, src/tests/{main,burning}.rs, tools/{generate.py,README.md}, generated particle preset/manifest/inventory.
ordered steps: implement burn lifecycle and local contact propagation; add animated flames and a bounded rotating particle pool; cover ignition, spread, sleeping piles, burnout, rendering and particle cleanup; regenerate assets and run checks.
done-when: focused tests, generator --check, authoring tests, just check and just smoke pass; report visual limits.

Context: the small-ball work is already in the working tree. Small balls currently
share ordinary balls' instant fire destruction. Preserve normal-ball destruction.
The user selected charred, nonflammable survivors after burnout. Sleeping physics
bodies do not produce contact events, so use local spatial buckets as well as
current physics contacts. Freeze spread sources per frame to avoid event-order
dependent chain ignition. Share at most 64 flame particle emitters across burning
balls, with an animated flame on every burning ball and the existing global cap.

## Results

Implemented ignition from swept fire-hazard contact, spread from current body
contacts and spatially bucketed resting contacts, fixed 15-second burn duration,
and permanently charred survivors. Every burning ball renders an animated flame;
a rotating 64-emitter pool emits the new generated ball_burn preset (eight live
particles per emitter), respecting the global particle cap and draining on burnout.
No engine files changed. Normal-ball fire destruction remains unchanged.

Validation passed: six new regressions, just check (61 platformer tests plus the
workspace), just smoke (all examples and fixture matrices), just repo-check,
generator --check (340 outputs), 15 authoring tests, and git diff --check.
The regression suite covers swept ignition, symmetric/stale contacts, settled
pile propagation, burnout/non-reignition, rendering and capped effects/cleanup
with 2,048 burning balls. Flame appearance and dense-pile spread readability
remain a human visual check via cargo run -p example-01-platformer.

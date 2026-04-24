---
status: archived (diagnosis only, no action planned)
topic: dense-pile FPS collapse and wall tunneling in 01_platformer
---

# Dense-pile FPS collapse and wall tunneling

## What you see

1. Spawn-hold Mouse1 until ~1000 balls pile up in one region.
2. Frame time starts climbing. `compute_substeps` begins reporting its
   maximum (8) every frame.
3. Balls on the outside of the pile start clipping the tilemap walls,
   especially when a black hole yanks the pile sideways.
4. FPS visibly tanks. The pile behaves erratically.

The jitter-on-spawn fix in [systems.rs:335-356](examples/01_platformer/src/systems.rs#L335-L356)
eliminates the *coincident-normal* micro-bug (the SE-drift artifact), but
it does not touch the two things below. They are properties of the
engine's current physics pipeline at scale.

## Why a dense pile is ~O(N²) per substep

The narrow phase runs inside `resolve_collisions` at
[step.rs:138-317](crates/tungsten-core/src/physics/step.rs#L138-L317).
Each substep:

1. Rebuilds the broad-phase grid from scratch
   ([step.rs:186-190](crates/tungsten-core/src/physics/step.rs#L186-L190)).
   Cell size is `PhysicsConfig::broadphase_cell_size = 32.0` in this
   example. Ball radius is 6.
2. For every dynamic proxy, queries the grid for neighbors
   ([step.rs:217-222](crates/tungsten-core/src/physics/step.rs#L217-L222)).
3. Runs `narrow_phase` against each neighbor
   ([step.rs:224-236](crates/tungsten-core/src/physics/step.rs#L224-L236)).
4. Repeats the whole resolve loop `solver_iterations` times (default 4)
   ([step.rs:213-215](crates/tungsten-core/src/physics/step.rs#L213-L215)).

In a pile, "neighbors in the same cell" is not a constant. A 32×32 cell
can hold roughly (32 / (2·6))² ≈ 7 tightly-packed balls before spillover,
and balls overlap multiple cells. Once the pile saturates a region of
~K cells containing N balls total:

- Each ball's AABB query returns O(N / K) candidates in its own cell
  plus the 3–8 adjacent cells it overlaps.
- Per substep: `iterations × N × (N/K + adjacents) ≈ O(N²)` work in the
  narrow-phase loop alone, multiplied by 4 solver iterations.
- Per frame: that × substeps (up to 8).

So a 1000-ball pile is roughly
`1000 × (some fraction of 1000) × 4 iterations × 8 substeps` contact
tests per frame at steady state, plus the same factor in proxy setup
and grid rebuild. That is the ~O(N²) term: the broad phase only culls
across space, and once space is saturated it cannot cull anything.

Secondary amplifiers that push this from "heavy" to "unresponsive":

- Proxies are collected into a `Vec` from an ECS query each substep
  ([step.rs:139-181](crates/tungsten-core/src/physics/step.rs#L139-L181)).
  Allocation + component fetches scale with N.
- `apply_gravity_and_integrate` runs a fresh
  `world.query_entities::<RigidBody>` plus a filter pass every substep
  ([step.rs:112-136](crates/tungsten-core/src/physics/step.rs#L112-L136)).
- The contact event vector is allocated fresh per substep and drained
  into the queue at the end.

None of these are expensive in isolation; at N=1000 and 8 substeps per
frame they become dominant.

## Why FPS drop causes tunneling

Substep count is chosen at
[step.rs:83-110](crates/tungsten-core/src/physics/step.rs#L83-L110):

```
worst_ratio = max over dynamic bodies of (|v| · dt) / min_half_extent
substeps    = ceil(worst_ratio).min(max_substeps)   # max_substeps = 8
sub_dt      = dt / substeps
```

The invariant the substep loop is supposed to guarantee is:
each body moves less than its smallest half-extent per integration,
so a swept shape can't skip past a thin collider. This only holds
**when the cap doesn't bind**. Once it does:

```
sub_dt = dt / 8
```

…regardless of how fast the body is actually going. The tunneling
threshold becomes:

```
tunneling if  |v| · sub_dt > min_half_extent
         i.e. |v|         > min_half_extent · 8 / dt
```

At a healthy 60 FPS (dt = 16.7 ms) with ball radius 6, that threshold is
~2880 px/s. Comfortable margin; black-hole gravity at 3000 px/s² can't
reach it before the ball hits something. At a distressed 15 FPS
(dt = 66.7 ms) the same ratio collapses to ~720 px/s, which a ball 0.25
seconds into a black-hole pull easily clears.

So the **FPS tank causes the tunneling**, not the other way around. The
pipeline is:

```
dense pile
  → O(N²) work per substep
  → frame time grows
  → dt grows
  → compute_substeps saturates at max_substeps
  → sub_dt grows
  → |v|·sub_dt exceeds min_half_extent for the fastest ball
  → ball skips a wall proxy between integrations
  → outside the world bounds → despawn_out_of_bounds → pile loses a ball
  → (if player is in the pile, player teleports to spawn)
```

The tilemap walls are the targets because they are thin (half-extent 8,
same order as the ball radius) and static — a static proxy can't move
out of the ball's way when the contact fires late.

## Why this is a scale-of-engine issue, not a single bug

Each component of the pipeline is correct at its design point:

- Uniform-grid broadphase is the right choice for sparse dynamic sims.
- Sequential Gauss–Seidel resolution is what makes stacks stable
  (see the tests at
  [step.rs:635-693](crates/tungsten-core/src/physics/step.rs#L635-L693)).
- A fixed substep cap prevents a runaway velocity from freezing the
  frame; without it, one escaped body could request 200 substeps.
- Integer-radius tiles with half-extents near the ball radius is the
  geometry the tilemap was built around.

The failure mode only appears when three conditions overlap:

1. Density high enough that narrow-phase cost becomes quadratic in a
   region (no broad-phase culling left to do).
2. External forcing (black hole) driving velocities up in that region.
3. Frame time degraded enough that `sub_dt` exceeds the ball radius
   divided by those velocities.

Fixing any one of these individually breaks a test or a design
intention. That is what "scale-of-engine" means here: the numbers all
fit inside their documented bounds; the bounds just don't compose
safely at N ≈ 1000.

## Directions worth evaluating (not a plan, options to judge)

Each row names a bottleneck, a candidate technique, roughly what it
costs to add, and what it gives up or complicates.

| Bottleneck | Technique | Cost to add | Tradeoff |
|---|---|---|---|
| O(N²) in one broadphase cell | Smaller cell size (e.g. 16 instead of 32) | Tuning only; one field | Each ball lives in more cells → more duplicate inserts and candidate dedup. Helps until the *pile* size exceeds the new cell too. |
| Narrow-phase count inside a cell | Pair caching between frames | Medium — needs stable pair IDs and a hash map across substeps | Couples substeps; complicates hot-reload and entity-despawn paths. Wins big when contacts are largely persistent (piles). |
| Solver × narrow-phase redundancy | Separate contact generation (once) from constraint solving (iterated) | Medium — refactor of `resolve_collisions` | Iteration count on the cheap solver pass can be increased without re-running narrow phase. Standard in box2d/rapier. |
| Tunneling when cap binds | Speculative contacts / swept tests for fast bodies | Medium — added narrow-phase path and broadphase-by-swept-AABB | Extra work per frame for fast bodies; needs a velocity-based activation so resting bodies don't pay the cost. |
| Substep cap at 8 | Per-body local substepping | Heavy — rearranges the substep loop to integrate bodies at different rates | Complicates ordering guarantees for `EventQueue<CollisionEvent>`. |
| Resting bodies consuming cycles | Sleeping bodies / island sleeping | Medium — per-body wake flag, energy metric, wake propagation on contact | Piles spend most of their life below the wake threshold, so cost should drop dramatically. Wake propagation is where the bugs hide. |
| Rebuild of proxies/grid per substep | Persistent proxy list + incremental grid updates | Heavy — invalidates "grid is rebuilt from scratch, no incremental bugs" invariant noted in the header of step.rs | Needs solid despawn/teleport invalidation tests. |
| Allocation in hot path | Reuse `proxies`/`events`/`candidates` buffers across frames as resources | Light — move three `Vec`s into `World` resources | Pure win, no correctness change. Probably worth doing regardless. |
| Geometry — thin walls | Thicker collision margin on tilemap proxies | Light — emit each tile with slightly enlarged half-extents | Bleeds into sprite alignment; users would see "invisible" 2-px wall thickness. Mitigates tunneling but doesn't address the O(N²) root cause. |
| Coincident-shape degeneracy | Fix `circle_vs_circle` concentric case to pick a random unit normal | Trivial | Already compensated in this example by spawn-time jitter. Worth doing at engine level so future examples don't need to replicate the workaround. |

A realistic sequencing, if you decide to pursue this:

1. **Free wins first**: reuse buffers across frames, fix the
   concentric-normal bug upstream, shrink default cell size or make it
   auto-tune per-frame.
2. **Sleeping bodies** — biggest expected payoff, because piles are
   mostly at rest by design. This alone could drop the 1000-ball steady
   state back into interactive territory without changing the algorithm
   class.
3. **Speculative/swept contacts** for the tunneling symptom, gated on
   body velocity so idle balls stay cheap.
4. Only then, the heavier refactors (contact caching, per-body
   substepping). Those are the kind of change that earns its own
   milestone.

## What is already in place as mitigation

- `despawn_out_of_bounds`
  ([systems.rs:524-557](examples/01_platformer/src/systems.rs#L524-L557))
  catches escaped balls and the player, so a single tunneling event
  doesn't permanently wreck the session.
- `BALL_SPAWN_JITTER`
  ([state.rs:34](examples/01_platformer/src/state.rs#L34))
  prevents the separate coincident-normal drift bug from conflating
  with this investigation.
- `WORLD_BOUNDS_MIN`/`_MAX`
  ([state.rs:49-55](examples/01_platformer/src/state.rs#L49-L55))
  bound the active region so a runaway body can't infinitely inflate
  `worst_ratio`.

None of these address the underlying quadratic-density problem; they
contain the symptom.

## Decision state

Deferred. The example is an intentional stress test and the current
behaviour (degrades gracefully enough to stay interactive up to several
hundred balls; visibly falls apart past ~1000) is acceptable for the
0.16 milestone. Re-open this when either:

- A game built on the engine needs >500 concurrent dynamic rigid
  bodies in overlapping cells, or
- Frame-time budget for physics drops below ~4 ms at 60 FPS and the
  pile benchmark becomes the dominant cost.

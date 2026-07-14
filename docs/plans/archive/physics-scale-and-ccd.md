# Physics: Scale to 20k–25k Bodies + Complete CCD

- **status:** done (step 1 landed 2026-07-07; step 2 landed 2026-07-07; step 3 landed 2026-07-08; step 4 landed 2026-07-08; step 5 landed 2026-07-11; step 6 evaluated and **dropped** 2026-07-12 per its conditional clause, D-067 — see step 6 notes; step 7 close-out completed 2026-07-13; the "25k awake churn ≤ ~16 ms" target is deferred to future work per D-067's levers)
- **goal:** (1) eliminate tunneling for fast bodies against *all* collider classes (static AABB, static circle, dynamic targets, dynamic-vs-dynamic); (2) raise the practical simultaneous dynamic colliding body count from ~3k to 20k–25k at sustained 60 FPS on the reference machine (Ryzen 5 6600H / RADV REMBRANDT).
- **non-goals:** rotation, friction, joints/constraints, external physics crates (D-033 stands — stays hand-rolled), GPU physics, fixed-timestep accumulator rework (D-033's noted upgrade; orthogonal, not blocked by this plan), changing shipped `tungsten.json` defaults.
- **files to touch:** `crates/tungsten-core/src/physics/{broadphase,collision,components,events,mod,step}.rs`, `crates/tungsten-core/src/tests/physics/*`, `crates/tungsten-core/tests/physics_tunneling.rs`, `crates/tungsten-core/benches/physics_bench.rs`, `DECISIONS.md` (new entry superseding parts of D-033), `docs/perf/profiling-workflow.md` (only if capture rules change), `docs/LLM_INDEX.md`.

## Context digest

Current step (`physics/step.rs`): global substep count = `ceil(max travel / min half-extent)` capped at 8 → **one fast body multiplies whole-world cost up to 8×**. Per substep: proxies re-gathered from ECS (per-entity random lookups for `RigidBody`/`Velocity`), `HashMap<IVec2, Vec<u32>>` grid rebuilt from scratch (per-cell `Vec` allocations dropped every clear), speculative slab-sweep for fast dynamics **vs static AABBs only**, pair list rebuilt via per-dynamic grid queries, Gauss–Seidel ×4 with the narrow phase **re-run every iteration**, in-place MTV projection + restitution impulse, per-entity writeback. No warm starting, no contact persistence, no sleeping, no islands.

Baseline (2026-07-07, criterion, fixed dt=1/60, release): see `perf-runs/` baseline dir and table below. Headline: a settled, contained 3k dense pile costs ~6–7 ms per `physics_step` at 1 substep — already past the whole 4 ms update budget; scaling is superlinear (pair-candidate lists grow with density because grid candidates are pushed as pairs with **no AABB overlap prefilter**, then narrow-phase re-tested ×4 solver iterations).

Two defects surfaced while baselining (probe, 2026-07-07):

1. **Containment failure under pile pressure.** In a 3k pile inside 80 px-thick static walls, Gauss–Seidel in-place corrections shoved 7 bodies past the floor's centerline within ~400 steps; the MTV then ejects out the far side and they free-fall forever. Escaped bodies reach unbounded velocity, which pins `compute_substeps` at the cap — **one leaked body multiplies whole-world cost ×8 permanently**. The soft solver's max-push-speed (step 2) plus speculative containment (step 3) fix this class.
2. **Jitter floor.** The settled pile never calms below ~97 px/s average body speed (no warm starting, full-penetration MTV push each step re-injects energy). Bodies visibly vibrate, and no sleep criterion could ever engage. Sleeping (step 4) is therefore *blocked on* the soft solver (step 2) — ordering matters.

Root causes ranked: (a) unfiltered pair lists × 4 redundant narrow-phase sweeps per substep; (b) hashed-grid rebuild churn (SipHash + per-cell `Vec` alloc drop per substep); (c) no sleeping, guaranteed by the jitter floor; (d) global 8× substep amplification from any single fast (or leaked) body.

Tunneling baseline (`physics_tunneling` test): static thin AABB wall — 0% miss at all speeds up to 15,360 px/s (speculative sweep works). Static circle pillar, near-immovable dynamic wall, head-on dynamic pairs — **100% miss at ≥7,680 px/s** (≥16 px per substep at the cap exceeds combined extents; no swept path exists for these classes).

Research digest (sources in `## Research notes`): the modern consensus fix is Box2D-v3-shaped — **speculative contacts** as the primary CCD (handles all pair classes, removes the global substep heuristic), **flat prefix-sum grid** for uniform-size broadphase, **soft-step solver** (warm-started accumulated impulses, soft-constraint bias, substeps-over-iterations), and **island sleeping** (the single biggest lever for settled piles: steady-state cost collapses to the awake fringe).

## Benchmark contract

Every step below lands with, in order:

1. `cargo test --workspace` green (unit + tunneling harness at its current assertion tier).
2. `cargo bench -p tungsten-core --bench physics_bench` — compare `physics_step/*` against the captured baseline via criterion's change report; record the deltas in this file under the step.
3. `cargo test --release -p tungsten-core --test physics_tunneling -- --nocapture` — miss-rate matrix; paste changes here.
4. In-app capture only at steps marked 📸: `WGPU_BACKEND=vulkan ./scripts/perf-capture.sh physics-stress 300` plus `--stress-count 10000/20000 --telemetry-only` rows.

Criterion scenarios (all deterministic, headless, `Pcg32` fixed seeds): `dense_pile` (settled gravity pile, 3k/10k/25k), `projectile_stream` (fast elastic bullets, max-substep path, 3k/10k/25k), `mixed_static_dynamic` (pile around static pillar grid, 10k/25k), `pile_plus_bullet` (settled pile + one bullet forcing global substep cap, 10k/25k).

## Baseline (2026-07-08, `perf-runs/20260708T001441Z-physics-scale-baseline/`)

| Bench | Baseline |
| --- | --- |
| dense_pile/3000 | 6.49 ms |
| dense_pile/10000 | 35.4 ms |
| dense_pile/25000 | 307 ms (high variance) |
| projectile_stream/3000 | 30.3 ms |
| projectile_stream/10000 | 62.9 ms |
| projectile_stream/25000 | 140.6 ms |
| mixed_static_dynamic/10000 | 49.4 ms |
| mixed_static_dynamic/25000 | 146.3 ms |
| pile_plus_bullet/10000 | 131.4 ms (3.7× dense_pile/10000) |
| pile_plus_bullet/25000 | 593 ms (1.9× dense_pile/25000) |
| in-app physics-stress 3k | avg total 6.55 ms / update 5.57 ms (~150 FPS) |
| in-app physics-stress 10k | avg total 36.1 ms / update 34.7 ms (~28 FPS) |
| in-app physics-stress 20k | avg total 120.7 ms / update 118.1 ms (~8 FPS) |

The 20k target needs ≥15× at the average and ≥13× at p95 (216 ms → 16.7 ms).

## Ordered steps

### 1. Flat prefix-sum broadphase (kill the HashMap)

Replace `SpatialGrid`'s `HashMap<IVec2, Vec<u32>>` with a rebuild-per-step flat structure: pass 1 counts proxies per cell, exclusive prefix sum gives cell offsets, pass 2 scatters proxy ids into one flat `Vec<u32>`. Unbounded coordinates hash `IVec2` into a power-of-two flat table with a multiplicative hash (spatial hashing), never `std::HashMap`. Keep the public `SpatialGrid` insert/query API shape so `step.rs` changes stay minimal. Build **once per frame** from AABBs inflated by per-substep travel + margin; reuse across substeps (grid no longer rebuilt per substep — supersedes that clause of D-033; note in DECISIONS entry at the end of the plan).

Also fix pair generation in the same step: today every grid candidate inside the swept-AABB + half-cell-margin query is pushed as a pair with **no AABB overlap prefilter**, so dense piles carry heavily inflated pair lists into 4 narrow-phase sweeps. Emit a pair only when the two inflated AABBs actually overlap.

- Compare: all `physics_step/*` + `broadphase_rebuild_5k_dynamic`.
- Expect: order-of-magnitude broadphase reduction; largest relative win on `projectile_stream` (8 rebuilds/frame today).
- Done: no `HashMap` in the physics hot path ✅; grid tests migrated (7 kept + 4 new: dirty rebuild, clear/reinsert reuse, hash-alias exactness) ✅; no behavior change in solver outcomes (existing step tests green, tunneling matrix byte-identical to baseline) ✅.

#### Landed 2026-07-07 (D-062)

Implementation note: the plan's "inflate by per-substep travel + margin, build once per frame" needed a third piece to be sound — a **drift budget**. Staging with *full-frame* travel inflation was tried first and regressed fast scenarios badly at scale (+25% / +50% on `projectile_stream` 10k/25k: a 2,400 px/s body inflates to ~12 cells, multiplying per-cell density and query scans). Staging with *per-substep* inflation alone silently drops dynamic-vs-dynamic pairs at moderate speeds once targets drift out of their staged cells. Landed variant: stage with per-substep inflation, accumulate max per-substep travel each substep, restage from current positions when the total exceeds the half-cell query margin. Settled piles build once per frame; a 2,500 px/s bullet forces ~3 stagings per 8-substep frame; extreme speeds degrade to the old per-substep rebuild.

Criterion deltas vs the Baseline table (bench log: scratchpad `bench-step1b.log`; criterion's own change report compares against a discarded intermediate run — deltas below are computed against the recorded baselines):

| Bench | Baseline | Step 1 | Delta |
| --- | --- | --- | --- |
| dense_pile/3000 | 6.49 ms | 5.21 ms | **−19.7%** |
| dense_pile/10000 | 35.4 ms | 38.6 ms | +9.0% ⚠ (see note) |
| dense_pile/25000 | 307 ms (high variance) | 375 ms (CI 323–436, high variance) | +22% (noise-dominated) |
| projectile_stream/3000 | 30.3 ms | 13.78 ms | **−54.5%** |
| projectile_stream/10000 | 62.9 ms | 48.19 ms | **−23.4%** |
| projectile_stream/25000 | 140.6 ms | 123.4 ms | **−12.2%** |
| mixed_static_dynamic/10000 | 49.4 ms | 52.6 ms (CI 47.9–57.0) | +6.5% (CI overlaps baseline) |
| mixed_static_dynamic/25000 | 146.3 ms | 151.7 ms | +3.7% |
| pile_plus_bullet/10000 | 131.4 ms | 136.7 ms | +4.0% (ratio to dense_pile: 3.71× → 3.54×) |
| pile_plus_bullet/25000 | 593 ms | 745 ms | +25.7% (ratio to dense_pile: 1.93× → 1.99×) |
| broadphase_rebuild_5k_dynamic | ~314 µs | 78.6 µs | **−75%** |

Miss-rate matrix: byte-identical to baseline (static AABB wall 0/32 at all speeds; circle/dynamic/head-on classes still 100% miss at ≥7,680 px/s — step 3's scope). Not re-pasted.

⚠ **Watch item — `dense_pile/10000` +9% (tight CI):** the `pile_plus_bullet` regressions track their dense-pile base exactly (amplification ratio unchanged or better), so the only unexplained delta is the settled-pile base cost at 10k/25k. Diagnostics (`tests/substep_probe.rs`, ignored by default): the new settled 10k world runs 1 substep every step, zero escapes, jitter floor 114 px/s — so it is not substep amplification; it is either a slightly hotter settled state (pair-order change → different jitter than baseline's world) or real per-query cost at pile density. Both 25k rows are noise-dominated (120 settle steps do not settle 25k; baseline itself flagged high variance). Decision: landed — step 1's done-when bullets all pass and the broadphase-bound scenarios show the expected wins; re-examine the pile rows after step 2, whose soft solver collapses the jitter floor and makes settled-pile comparisons meaningful. If +9% persists at step 2's re-bench, profile the flat-grid query path at pile density before starting step 3.

**Resolved at step 2 (2026-07-07):** `dense_pile/10000` fell to 17.12 ms (−55% vs step 1, −52% cumulative) and `dense_pile/25000` to 67.2 ms (−82% / −78%). The +9% was the hotter settled state under the old jittering solver, not flat-grid query cost — no profiling needed before step 3.

### 2. Contact persistence + soft solver (narrow phase once, warm start)

Run the narrow phase **once per substep** to produce a persistent contact buffer (pair key, normal, penetration/gap, precomputed inverse effective mass). Solver iterations then work on cached contacts with **accumulated impulses clamped ≥ 0** and **warm starting** from the previous step's impulses (pair-keyed map, survives across frames). Replace raw in-place MTV projection with a **soft constraint bias** (Box2D v3 shape: contact hertz ≈ 30, damping ratio ≈ 10, linear slop, max push speed) plus one bias-free relax iteration to strip injected energy. Restitution moves to a stored approach-velocity model with a low-speed inelastic threshold.

- Compare: `dense_pile/*` (target ≥4× from removing 3 redundant narrow-phase sweeps + calmer piles → fewer substeps), plus visual sanity in `physics-stress` scene (no popping/sinking).
- Done: pile at 3k visually stable with average body speed ≪ the current ~97 px/s jitter floor (prerequisite for step 4's sleep threshold) ✅ (probe: 11.5 px/s at 10k, substeps pinned at 1); **containment holds** — zero bodies escape a 3k pile in 80 px-thick walls over 2,400 steps ✅ (`tests/physics_containment.rs`, release-gated via `cfg_attr(debug_assertions, ignore)`; 0/3000 escaped); `dense_pile/3000` under ~4 ms ✅ (3.82 ms); collision-event emission semantics preserved ✅ (events emit from the pre-solve narrow-phase pass — the old iteration-0 contact set; documented in `resting_contact_keeps_emitting_collision_events`).

#### Landed 2026-07-07 (D-063)

Implementation notes: Box2D-v3 substep order (contacts → gravity → warm start → biased iterations → integrate positions → bias-free relax → restitution) — position integration sits *between* the biased solve and relax so bias velocity becomes depenetration before it is stripped. Warm-start impulses live in a flat open-addressing pair map (entity id+generation / tile-position-hash keys, rebuilt from live contacts each substep so stale pairs age out; no `std::HashMap`). Contacts are measured pre-integration, so a first touch can land one substep later than the old post-integration narrow phase — invisible at 60 FPS, and all event assertions still pass. Positional consequences: bodies rest at ~`linear_slop` (0.25 px) and deep overlap recovers at the bias rate (capped `max_push_speed` 120 px/s) over frames, not in one MTV push; four MTV-precision unit tests were rewritten to assert convergence-to-slop / never-past-centerline instead of same-step resolution, and the stack test tolerance widened to 1 px (load-dependent sink; the effective hertz cap of 0.25× substep rate = 15 Hz at 1 substep tightens when step 3 fixes substeps at 4). Dynamic bodies without colliders now integrate once per frame outside the solver.

Criterion deltas: the stored criterion comparison point is the step-1 landed run, so the change report reads step 2 directly (log: scratchpad `bench-step2.log`). Cumulative = vs this plan's Baseline table.

| Bench | Baseline | Step 1 | Step 2 | vs step 1 | cumulative |
| --- | --- | --- | --- | --- | --- |
| dense_pile/3000 | 6.49 ms | 5.21 ms | 3.82 ms | **−28.8%** | **−41.1%** |
| dense_pile/10000 | 35.4 ms | 38.6 ms | 17.12 ms | **−55.2%** | **−51.6%** |
| dense_pile/25000 | 307 ms | 375 ms | 67.2 ms | **−82.0%** | **−78.1%** |
| projectile_stream/3000 | 30.3 ms | 13.78 ms | 10.86 ms | **−13.5%** | **−64.2%** |
| projectile_stream/10000 | 62.9 ms | 48.19 ms | 46.95 ms | +6.9% (p = 0.13, n.s.) | −25.4% |
| projectile_stream/25000 | 140.6 ms | 123.4 ms | 146.8 ms (CI 136–160) | +22.3% ⚠ (see note) | +4.4% |
| mixed_static_dynamic/10000 | 49.4 ms | 52.6 ms | 22.16 ms | **−55.2%** | **−55.1%** |
| mixed_static_dynamic/25000 | 146.3 ms | 151.7 ms | 64.8 ms (CI 58–77) | **−48.6%** | **−55.7%** |
| pile_plus_bullet/10000 | 131.4 ms | 136.7 ms | 29.9 ms (CI 22–46) | **−63.5%** | **−77.3%** |
| pile_plus_bullet/25000 | 593 ms | 745 ms | 346.7 ms | **−48.5%** | **−41.5%** |
| broadphase_rebuild_5k_dynamic | ~314 µs | 78.6 µs | 84.8 µs | +7.8% (untouched code; layout/noise) | −73% |
| in-app physics-stress 3k (`perf-runs/20260708T012148Z-physics-stress/`) | total 6.55 / update 5.57 ms | — | total 6.25 / update 5.22 ms (p95 total 7.60) | — | −5% / −6% |

The in-app 3k row moves little because the canonical 300-frame window is dominated by the spawn fall, not the settled pile; sleeping (step 4) is the in-app lever. `pile_plus_bullet` ratios to dense_pile at the same count: 10k 3.54× → 1.75×; 25k 1.99× → 5.2× (the bullet still forces 8 substeps of the whole world while the dense-pile base collapsed −82% — the ratio target ≤1.3× is step 3's done-when, where the substep heuristic is deleted).

Miss-rate matrix: byte-identical to baseline (static AABB wall 0/32 at all speeds; circle/dynamic/head-on classes still 100% miss at ≥7,680 px/s — step 3's scope). Not re-pasted.

⚠ **Watch item — `projectile_stream/25000` +22.3% vs step 1 (wide CI 136–160 ms, +4.4% vs original baseline):** the elastic stream has no persistent contacts (warm start is a no-op) and pays the per-substep contact-buffer + impulse-map overhead ×8 substeps. Step 3 (speculative CCD + fixed substeps = 4) rewrites exactly this path and lists `projectile_stream/*` as its Compare line — re-examine there; if it persists after step 3, profile the contact-build path in the stream scenario. Also noted: `position_integration_50k` +4.9% and `broadphase_rebuild_5k_dynamic` +7.8% on untouched code — code-layout/machine noise, watch only.

### 3. Speculative contacts as primary CCD; fixed substeps 📸

Admit broadphase pairs with a **positive gap** up to a speculative margin (AABBs inflated by per-substep travel); the velocity constraint targets arrival at exactly touching contact (remove `gap/dt` excess approach velocity only when negative). This handles dynamic-vs-dynamic, vs-static-circle, and vs-dynamic-wall — all current blind spots — at near-zero extra cost. Then **delete the global velocity-derived substep heuristic**: fixed `substeps = 4` (config), no per-speed amplification. Keep the existing slab sweep only as a safety net for extreme bullets vs statics (travel > margin), now also covering static circles (circle→AABB promotion is already conservative). Mitigate ghost collisions on tile seams by clamping speculative normals against `face_mask` internal edges (already modeled).

- Compare: `projectile_stream/*` and `pile_plus_bullet/*` (expect the 8×-amplification collapse), tunneling matrix.
- Done: **tunneling miss rate 0/32 for every scenario at every speed ≤ 15,360 px/s** ✅ (matrix below); tighten `physics_tunneling` assertions from "static AABB only" to all four scenarios ✅; `pile_plus_bullet` within ~1.3× of `dense_pile` at the same count ✅ (10k 1.03×, 25k 0.85× — amplification eliminated).

#### Landed 2026-07-08 (D-064)

Implementation notes: the narrow phase went signed-distance — `aabb_vs_aabb_speculative` / `circle_vs_circle_speculative` / `aabb_vs_circle_speculative` admit separated pairs as negative-penetration contacts up to a **per-pair margin** of `|v_a − v_b|·sub_dt + 4·linear_slop` (velocity-scaled so settled piles admit almost nothing; the D-062 travel-inflated pair query already finds gap pairs). The solver needed **zero changes**: the existing `s > 0` branch biases by `s/sub_dt`, which is exactly "remove gap/dt excess approach", and D-063's stored-approach restitution replays the full pre-clamp velocity, so elastic bullets arrive at touching and rebound at full speed (no energy drift in `projectile_stream`). `compute_substeps` is deleted; `PhysicsConfig` replaces `max_substeps` (8) with fixed `substeps` (4) and drops `solver_iterations` 4 → 1 (substeps-over-iterations; the effective contact-hertz cap rises 15 → 60 Hz at 60 FPS). Seam ghosts: speculative corner/vertex normals clamp to the exposed axis per `face_mask`; a penetrating vertex contact with one masked face now clamps instead of dropping (uniform seam support), and a pure face gap on an internal face drops. The slab sweep stays as a statics-only safety net for solver-injected velocity (pinned by two new diagonal-shover tests), now covering static circles via bounding-square promotion. Event gate: contacts emit only at `penetration > 0` — never on a gap — pinned by `speculative_gap_contact_emits_no_event_until_touch`; `resting_contact_keeps_emitting_collision_events` unchanged and green.

Criterion deltas: the stored comparison point is the step-2 landed run, so the change report reads step 3 directly (log: scratchpad `bench-step3.log`). Cumulative = vs this plan's Baseline table.

| Bench | Baseline | Step 2 | Step 3 | vs step 2 | cumulative |
| --- | --- | --- | --- | --- | --- |
| dense_pile/3000 | 6.49 ms | 3.82 ms | 11.42 ms | +201% ⚠ (see note) | +76% |
| dense_pile/10000 | 35.4 ms | 17.12 ms | 45.9 ms | +167% ⚠ | +30% |
| dense_pile/25000 | 307 ms | 67.2 ms | 176.7 ms (CI 164–198) | +188% ⚠ | −42% |
| projectile_stream/3000 | 30.3 ms | 10.86 ms | 5.37 ms | **−55.4%** | **−82.3%** |
| projectile_stream/10000 | 62.9 ms | 46.95 ms | 22.9 ms | **−55.6%** | **−63.6%** |
| projectile_stream/25000 | 140.6 ms | 146.8 ms | 65.2 ms | **−50.9%** | **−53.6%** |
| mixed_static_dynamic/10000 | 49.4 ms | 22.16 ms | 48.6 ms | +99% ⚠ | −1.6% |
| mixed_static_dynamic/25000 | 146.3 ms | 64.8 ms | 145.4 ms | +85% ⚠ | −0.6% |
| pile_plus_bullet/10000 | 131.4 ms | 29.9 ms | 47.4 ms | n.s. (p = 0.76, wide CIs both sides) | **−63.9%** |
| pile_plus_bullet/25000 | 593 ms | 346.7 ms | 150.0 ms (CI 140–173) | **−53.6%** | **−74.7%** |
| broadphase_rebuild_5k_dynamic | ~314 µs | 84.8 µs | 80.7 µs | −5.6% | −74% |
| in-app physics-stress 3k (`perf-runs/20260708T145308Z-physics-stress/`) | total 6.55 / update 5.57 ms | total 6.25 / update 5.22 (p95 7.60) | total 16.32 / update 15.31 (p95 19.30) | ⚠ | ⚠ |
| in-app physics-stress 10k (`…T145415Z…-count10000/`) | total 36.1 / update 34.7 ms | — | total 51.78 / update 50.45 (p95 57.64) | — | +43% ⚠ |
| in-app physics-stress 20k (`…T145552Z…-count20000/`) | total 120.7 / update 118.1 ms (p95 216.18) | — | total 117.45 / update 115.29 (p95 152.15) | — | −3% avg, **−30% p95** |

`pile_plus_bullet` ratio to `dense_pile` at the same count: 10k 1.75× → **1.03×**; 25k 5.2× → **0.85×**. The global substep amplification class is gone — a bullet now costs its own contacts, nothing more. The step-2 watch item (`projectile_stream/25000` +22.3%) is **resolved**: −50.9% vs step 2, −53.6% cumulative.

⚠ **Expected regression — awake settled scenes (`dense_pile/*`, `mixed_static_dynamic/*`, in-app spawn-fall rows):** settled piles that ran 1 substep under the deleted heuristic now run 4; per-substep work (proxy gather, pair query, narrow phase) quadruples while total solver iterations stay flat (4×1 vs 1×4, plus 4 relax passes instead of 1). Measured ≈ 2.7–3× on pure piles — the price the plan's architecture accepts at this step: tunneling safety is now decoupled from substep count, and the two follow-ups directly attack this exact cost (step 4 island sleeping collapses settled scenes to the awake fringe — the probe shows the 10k pile at a 9.3 px/s jitter floor, below step 2's 11.5, so the sleep criterion has clean signal; step 5 SoA removes the per-substep ECS gather). The 20k in-app row already shows the flip side: identical average, p95 −30% because per-speed substep spikes no longer exist. If step 4's sleeping does not pull `dense_pile/3000` back under the 4 ms budget, profile the per-substep gather before proceeding to step 5.

Miss-rate matrix (now asserted for all rows; `physics_tunneling` fails on any nonzero cell):

```text
scenario                    speed(px/s)   px/frame miss/total    miss%
static_thin_wall_aabb          480..15360  8..256      0/32      0.0   (all speeds)
static_circle_pillar           480..15360  8..256      0/32      0.0   (all speeds)
dynamic_thin_wall              480..15360  8..256      0/32      0.0   (all speeds)
head_on_dynamic_pair           480..15360  8..256      0/32      0.0   (all speeds)
```

Diagnostics after step 3 (`tests/substep_probe.rs`, updated to the fixed-substep model): fixed substeps 4, 10k settled-pile jitter floor **9.3 px/s** (11.5 post-step-2 — the higher effective hertz calms piles further), zero escapes; `physics_containment` green (0/3000 over 2,400 steps at 4 substeps — fixed substeps did not reopen the containment class).

### 4. Island sleeping 📸

Union-find over the contact graph each step (serial, deterministic) builds islands; an island sleeps when every member's velocity stays below threshold for ~0.5 s; sleeping bodies stay in the broadphase but skip integration, narrow phase (sleeping-sleeping pairs), and solver. Wake on: new contact with an awake body, external velocity/position write (needs a wake API or dirty flag on component write), body despawn in island. Sleep state is engine-internal (component or side table), not a public API commitment beyond a `wake(entity)` helper.

- Compare: `dense_pile/*` steady state (expect collapse to near-zero once settled — this is the 20k enabler), `pile_plus_bullet` (bullet keeps only its contact fringe awake), in-app 20k capture.
- Done: in-app `physics-stress --stress-count 20000` sustains ≥60 FPS / p95 ≤16.7 ms at steady state ✅ (last 300 frames of the long window: avg total 8.28 ms / p95 9.05 ms ≈ 120 FPS); a poke (bullet) wakes only the local region and the sim stays correct ✅ (`contact_wake_stays_local_to_the_disturbance` pins locality; `pile_plus_bullet/10000` runs at fringe-only cost, tunneling/containment matrices unchanged).

#### Landed 2026-07-08 (D-065)

Implementation notes: sleep state is a side table in `PhysicsBuffers` (flat open-addressing map keyed by the D-063 warm-start entity key; rebuilt with carry-over each frame so despawned entries age out — no `std::HashMap`), never a component; the public surface is `physics::wake(world, entity)` plus `PhysicsBuffers::{wake, is_sleeping, sleeping_count}`. Islands are a serial min-index-root union-find over touching (`penetration > 0`) awake-dynamic contacts accumulated across the frame's substeps; an island sleeps when its slowest member stays under `sleep_threshold` (new config, 20 px/s, `<= 0` disables) for `time_to_sleep` (0.5 s), zeroing velocities. Sleeping bodies stay staged in the grid but never initiate pair queries (sleeping–sleeping pairs skip the narrow phase entirely) and skip gravity, integration, and writeback; awake-vs-sleeping contacts solve with the sleeper frozen at effective inverse mass 0 (contacts now carry `inv_a`/`inv_b` captured at build — statics and sleepers uniformly immovable through one code path). **Wake is per body and motion-gated, not per island**: a touching contact wakes a sleeper only above threshold relative speed, a speculative gap contact only above threshold approach speed (a bullet wakes its impact fringe before touch resolves — D-064's admission margin does the reach), so wake waves damp out where motion dies and a poke stays local (Box2D's whole-island contact wake would re-churn all 20k bodies per hit). Island-level wakes handle the external cases: `Position`/`Velocity` writes are detected without ECS write-tracking because a sleeper's components are bit-frozen (writeback skips them; the stored sleep center is recomputed with gather's exact fp ops so untouched bodies compare bit-equal), despawn/component removal is caught by the per-frame map rebuild, and `wake()` wakes the stored island. A late sleeper resting on an already-sleeping island adopts its tag (bridging merges tags), so a despawn deep in the pile also lifts bodies that settled on top after it slept. **Event semantics decided (D-065, Box2D precedent):** resting contacts emit every step while awake, sleeping islands emit nothing, waking resumes emission — `resting_contact_keeps_emitting_collision_events` is superseded by `resting_contact_emits_until_asleep_and_resumes_on_wake`; the D-064 no-event-on-gap pin is unchanged.

Criterion deltas: the stored comparison point is the step-3 landed run, so the change report reads step 4 directly (log: scratchpad `bench-step4.log`). Cumulative = vs this plan's Baseline table. Settled scenarios are now bimodal inside a measurement window (the world falls asleep mid-run), so criterion CIs widen where sleep engages mid-measurement — noted per row.

| Bench | Baseline | Step 3 | Step 4 | vs step 3 | cumulative |
| --- | --- | --- | --- | --- | --- |
| dense_pile/3000 | 6.49 ms | 11.42 ms | 0.782 ms | **−93.1%** | **−87.9%** |
| dense_pile/10000 | 35.4 ms | 45.9 ms | 14.2 ms (CI 6.2–30.8, sleeps mid-window) | **−40.7%** | −59.9% |
| dense_pile/25000 | 307 ms | 176.7 ms | 137.7 ms (CI 129–157) | **−18.0%** | −55.1% |
| projectile_stream/3000 | 30.3 ms | 5.37 ms | 4.92 ms | **−6.0%** | **−83.8%** |
| projectile_stream/10000 | 62.9 ms | 22.9 ms | 18.9 ms | **−13.6%** | **−69.9%** |
| projectile_stream/25000 | 140.6 ms | 65.2 ms | 57.3 ms | **−14.9%** | **−59.2%** |
| mixed_static_dynamic/10000 | 49.4 ms | 48.6 ms | 43.8 ms | **−10.8%** | −11.4% |
| mixed_static_dynamic/25000 | 146.3 ms | 145.4 ms | 132.3 ms | **−9.6%** | −9.6% |
| pile_plus_bullet/10000 | 131.4 ms | 47.4 ms | 18.4 ms (CI 8.3–40.9, fringe-only) | **−36.9%** | **−86.0%** |
| pile_plus_bullet/25000 | 593 ms | 150.0 ms | 141.8 ms | n.s. (p = 0.31) | **−76.1%** |
| broadphase_rebuild_5k_dynamic | ~314 µs | 80.7 µs | 87.4 µs | +8.3% (untouched code; noise, watch) | −72% |
| in-app physics-stress 3k (`perf-runs/20260708T191638Z-physics-stress/`) | total 6.55 / update 5.57 ms | total 16.32 / update 15.31 (p95 19.30) | total 8.78 / update 8.30 (p95 11.68) | **−46% / −46%, p95 −39%** | +34% avg (window is spawn-fall; slept tail pulls avg 8.78 below p50 11.04) |
| in-app physics-stress 10k (`…T191820Z…-count10000/`) | total 36.1 / update 34.7 ms | total 51.78 / update 50.45 (p95 57.64) | total 27.50 / update 26.37 (p95 48.91) | **−47% avg, −15% p95** | **−24% avg** |
| in-app physics-stress 20k (`…T191914Z…-count20000/`) | total 120.7 / update 118.1 ms (p95 216.18) | total 117.45 / update 115.29 (p95 152.15) | total 110.63 / update 108.57 (p95 149.84) | −6% (window never reaches rest at 20k) | −8% avg, −31% p95 |
| in-app 20k steady state (`…T192054Z…-count20000/`, 1500-frame diagnostic window, non-canonical) | — | — | last 300 frames: total avg 8.28 / p50 8.21 / **p95 9.05** / p99 9.45; update avg 6.37 | — | **done-when met: ~120 FPS sustained** |

The 20k long-window trajectory (100-frame bucket averages of total ms): `79 → 123 → 104 → 105 → 102 → 8.5 → 8.3 → … → 8.3` — the pile finishes settling and fully sleeps around frame ~500, then holds ~8.3 ms flat. The canonical 300-frame rows at 10k/20k remain churn-dominated by construction (the spawn fall takes longer than the window); the steady-state row is the step-4 signal. `dense_pile/25000` and `mixed_static_dynamic/*` likewise never reach rest inside criterion's measurement (25k settles far beyond the 120-step settle window; the pillar pile keeps churning) — their sleeping payoff arrives with longer settles, not this bench geometry.

**Step-3 watch item closed:** `dense_pile/3000` fell from 11.42 ms to 0.782 ms — far under the ~4 ms budget — so no per-substep gather profiling is required before step 5. The slept steady state is nonetheless gather-bound: the probe's fully-slept 10k pile costs 2.76 ms/step (vs 40.43 ms awake at the same rest state, 14.6×) and that residual is almost entirely the ×4-per-frame proxy re-gather + map maintenance — exactly step 5's target. The step-3 `position_integration_50k`/`broadphase_rebuild` noise rows moved +3.7%/+8.3% again on untouched code; still watch-only.

Diagnostics after step 4 (`tests/substep_probe.rs`, now sleep-aware): default config — the 10k pile sleeps **fully** (awake 0/10,000) at ~step 210 after the 120-step settle (~5.5 s of sim time; occasional 1–2-frame single-body micro-pops up to ~80 px/s reset the island min-timer and stretch the tail), steady-state `physics_step` 2.76 ms, 0 escapes. Sleep-disabled run re-records the raw jitter floor the threshold must clear: avg 3.5 px/s over 300 steps (9.3 px/s over the first 100 — the pile keeps calming past the old measurement window), per-step max 3.6–5.9 px/s at true rest — the 20 px/s default threshold sits comfortably above it. Miss-rate matrix: byte-identical to step 3 (0/32 everywhere, all four scenarios; sleeping walls are woken through the speculative gap before touch). `physics_containment`: 0/3000 over 2,400 steps, and the harness wall time collapses ~12 s → ~5 s as the pile sleeps mid-run.

### 5. SoA gather once per frame

Gather body state (position, velocity, inv_mass, shape, flags) into dense parallel arrays **once per frame** — not per substep, and without per-entity `get::<RigidBody>()` random lookups (extend the ECS query to a 4-way or do a columnar pass). Write back once per frame. Substeps then run entirely on the dense arrays.

- Compare: all `physics_step/*`; expect a further constant-factor win, largest at 25k.
- Done: per-substep ECS traffic is zero ✅ (no `World` reads/writes inside the substep loop); `dense_pile/25000` (awake, active churn) ≤ ~16 ms ❌ (124.9 ms — see note; step 6's trigger); with sleeping at steady state ≤ 2 ms ✅ (probe: slept 25k pile runs 1.90 ms/step).

#### Landed 2026-07-11 (D-066)

Implementation notes: the ECS grew a columnar optional-component query pair — `World::query2_opt2::<A, B, C, D>` (two required + two optional; optional column presence resolves **once per archetype**, so rows in archetypes lacking `C`/`D` yield `None` with zero per-entity work) and its mutable twin `query2_opt2_mut` (shared `A`/`C`, mutable `B`/`D`, distinct-type asserts). Both iterate the exact archetype/row order of `query2::<A, B>`, and that order guarantee is load-bearing: `gather_proxies` runs `query2_opt2::<Collider, Position, RigidBody, Velocity>` once per frame, and the frame-end writeback runs `query2_opt2_mut` over the same set, zipping proxies positionally with query rows (entity equality debug-asserted). The per-substep gather + resync + writeback + event drain are gone: proxies persist across the frame's substeps (`Proxy::sleeping` set at frame start carries the sleep flag; mid-substep contact wakes still flip it and act within the same substep), collision events accumulate across substeps and drain once per frame in substep order, and sleeping bodies still skip writeback so D-065's bit-frozen invariant and gather-time external-write detection are untouched (gather's `position + offset` fp ops unchanged — sleep-entry centers still compare bit-equal). The `proxy_sleeping` side vector is deleted. Behavior note: a dynamic collider body **without** a `Velocity` component now keeps solver-injected velocity across the frame's substeps instead of resetting each substep (still resets across frames); no test pinned the old shape. `physics_tunneling` 0/32 everywhere (all four scenarios, byte-identical matrix) and `physics_containment` 0/3000 over 2,400 steps stay green; all D-064/D-065 event pins green.

Criterion deltas: the stored comparison point is the step-4 landed run, so the change report reads step 5 directly (log: scratchpad `bench-step5.log`). Cumulative = vs this plan's Baseline table.

| Bench | Baseline | Step 4 | Step 5 | vs step 4 | cumulative |
| --- | --- | --- | --- | --- | --- |
| dense_pile/3000 | 6.49 ms | 0.782 ms | 0.186 ms | **−76.0%** | **−97.1%** |
| dense_pile/10000 | 35.4 ms | 14.2 ms | 11.66 ms (CI 4.0–27.5, sleeps mid-window) | n.s. (p = 0.70, bimodal) | −67.1% |
| dense_pile/25000 | 307 ms | 137.7 ms | 124.9 ms (CI 116–145) | −7.5% (p = 0.37, n.s.) | −59.3% |
| projectile_stream/3000 | 30.3 ms | 4.92 ms | 3.88 ms | **−20.9%** | **−87.2%** |
| projectile_stream/10000 | 62.9 ms | 18.9 ms | 15.16 ms | **−18.2%** | **−75.9%** |
| projectile_stream/25000 | 140.6 ms | 57.3 ms | 44.8 ms | **−22.8%** | **−68.1%** |
| mixed_static_dynamic/10000 | 49.4 ms | 43.8 ms | 41.4 ms | **−5.9%** | −16.2% |
| mixed_static_dynamic/25000 | 146.3 ms | 132.3 ms | 124.8 ms | **−7.4%** | −14.7% |
| pile_plus_bullet/10000 | 131.4 ms | 18.4 ms | 15.6 ms (CI 6.0–37.0, fringe-only) | n.s. (p = 0.68) | **−88.1%** |
| pile_plus_bullet/25000 | 593 ms | 141.8 ms | 126.3 ms (CI 117–146) | n.s. (p = 0.30) | **−78.7%** |
| broadphase_rebuild_5k_dynamic | ~314 µs | 87.4 µs | 78.2 µs | −9.9% (untouched code; reverses step 4's +8.3% noise) | −75% |

`position_integration_50k` moved −5.6% — explicitly noted per the watch list: the 4-way query does **not** change that bench's code path (it drives `query2_entities` + per-entity `get`/`get_mut` directly, not `physics_step`), so this is the same layout/noise band as steps 2–4. Both prior noise rows swung favorably this time; still watch-only.

⚠ **Done-when miss — `dense_pile/25000` awake churn:** 124.9 ms against the ≤ ~16 ms target. The SoA win concentrates where ECS traffic dominated: fully-slept scenes (−76% at slept 3k; the probe's slept 10k runs 3.7× faster than step 4's 2.76 ms) and gather-heavy fast scenes (`projectile_stream` −18–23%). At full 25k churn the frame is solver/narrow-phase-bound (~25k awake bodies × 4 substeps of pair query + contact solve), which SoA staging cannot shrink — the −7.5% is the removed gather share. This is exactly step 6's trigger clause ("only if step 5's numbers still miss the 25k-awake target"): the conditional parallel solver via contact-graph coloring is now live, scoped to its own session.

Diagnostics after step 5 (`tests/substep_probe.rs`, log: scratchpad `probe-step5.log`, serial `--test-threads=1`): the slept steady state collapses to gather+stage cost only — 10k fully-slept pile **0.74 ms/step** (2.76 ms at step 4, −73%); the new 25k probe (900-step run) fully sleeps (awake 0/25,000) by ~step 500 and holds **1.90 ms/step** over the last 50 steps — the ≤ 2 ms slept-steady-state done-when is met. Settle trajectory and sleep engagement are unchanged from step 4 (10k fully asleep at ~step 210–220; occasional micro-pops reset island timers and stretch the tail; 0 escapes in all runs). Sleep-disabled 10k re-records the raw jitter floor: avg 3.5 px/s over 300 steps, per-step max 3.6–5.9 px/s at true rest (byte-similar to step 4's record — the SoA restructure did not perturb the settled dynamics), and the settled-awake 10k steady state reads 36.1 ms (40.4 ms at step 4, −10.7% — the awake gather share). Miss-rate matrix: byte-identical to step 3/4 (0/32 everywhere, all four scenarios); `physics_containment` 0/3000 over 2,400 steps.

### 6. (Conditional) scoped-thread parallel solver via contact graph coloring

Only if step 5's numbers still miss the 25k-awake target: greedy-color contact pairs during pair collection (contacts within a color touch disjoint bodies), solve each color's chunk across `std::thread::scope` threads, join per color; keep union-find and pair ordering serial for determinism. No async runtime, no new dependency (std only) — still needs a DECISIONS entry for the threading policy exception alongside the audio/watcher threads.

- Compare: all `physics_step/*` at 25k; verify determinism (two runs, identical positions hash).
- Done: linear-ish scaling across ≥4 cores on the reference machine, or the step is dropped with numbers recorded. ❌ scaling — **dropped with numbers recorded** ✅ (below).

#### Dropped 2026-07-12 (D-067)

The step was implemented in full, measured, and **reverted** — the drop clause fired. What was built (Box2D-v3 shape): greedy pair coloring during pair collection (lowest color free on both *movable* sides — statics and build-time-frozen sleepers don't constrain colors; with `apply_impulse` guarded on inverse mass > 0 they are read-only to the solver), 24 colors + exclusive overflow bucket, counting-sort of pairs by color so the narrow phase emits a color-grouped contact buffer, and the contact passes (warm start, biased iterations, bias-free relax, restitution) plus gravity/position integration chunked across `std::thread::scope` workers with a sense-reversing spin barrier per color. Narrow phase, wake path, union-find, events, and `speculative_pass` stayed serial. A `PhysicsConfig::solver_threads` knob selected the worker count (0 = auto).

**Determinism held bit-exactly** — the design goal was met: state hashes (FNV-1a over all position/velocity bits after 240 steps of 3k churn) were identical across serial, 2, 4, 8, and auto workers, and across reruns; a debug assertion proved the coloring invariant (no movable body twice in one color) on every substep of the unit suite. Disjoint movable bodies per color make chunk scheduling unobservable, and the serial path walked the same color-sorted buffer.

**It was dropped because the premise was wrong.** `perf` attribution of the 25k-awake frame (settled, sleep-disabled, 6 workers) shows the contact-solve passes — the only thing coloring can parallelize — are **~1.7 % of cycles**. The frame is `SpatialGrid::query` ~33 % + serial narrow phase/contact build ~61 % (inside which: warm-start `ImpulseMap` probing ~12 %, AABB overlap prefilter ~4 %, speculative signed-distance fns ~6 %, counting-sort scatter ~2 %). Amdahl caps the whole approach at ~2 %. Measured scaling (25k sleep-disabled settled pile, last-50-step tail, `--test-threads=1`):

| solver workers | 1 | 2 | 4 | 6 | 8 | 12 | 16 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| physics_step (ms) | 134.6 | 132.8 | 131.2 | 132.2 | 132.5 | 150.5 | 302.7 |

Flat through 8 workers (best −2.5 %), regressing at 12 (= hardware thread count; scheduling pressure on the spin barriers) and collapsing at 16 (oversubscription). Worse, the coloring + counting-sort machinery alone regressed the **serial** path ~18 % at the 10k settled-awake state (36.1 → 42.8 ms) — the color-sorted pair order also destroys the narrow phase's sequential access pattern. A parallel structure whose overhead exceeds its theoretical ceiling is unsalvageable: **reverted to the step-5 serial step**; no threads in physics, no config knob, no threading-policy exception needed (`AGENTS.md` stands unchanged). `D-067` records the decision; the kept artifact is `tests/physics_determinism.rs` (release-gated: two full 3k-churn runs must hash bit-identically — hash `0xae0818a3f4564331` both runs), which pins the determinism property any future parallelism must preserve.

Post-revert verification: `cargo test --workspace` green (626 passed); tunneling matrix byte-identical (0/32, all four scenarios, all speeds); `physics_containment` 0/3000 over 2,400 steps. Probe steady states re-recorded (same code as step 5; movement is thermal/run-to-run noise): slept 10k **0.77 ms** (step 5: 0.74), slept 25k **1.99 ms** (step 5: 1.90 — the ≤ 2 ms box still holds), sleep-disabled settled-awake 10k 38.1 ms (step 5: 36.1). Criterion vs the stored step-5 point: every `physics_step/*` row n.s. or within the ±4–10 % noise band the plan already watches (`dense_pile/25000` 133.1 ms, p = 0.51 n.s.; `dense_pile/3000` +3.3 % at 194 µs; `mixed` +3.4 %/+4.8 % marginal; bimodal rows unchanged; watch rows `position_integration_50k` +1.8 %, `broadphase_rebuild_5k_dynamic` +3.1 % — noise as always). Log: scratchpad `bench-step6.log`.

**Where the 25k-awake time actually is** (input for any future attempt at the ≤ ~16 ms goal; out of scope for this plan):

1. **Pair query ×4 per frame (~33 %+):** the same ~25k grid queries + prefilter run every substep against a grid that D-062 stages at most ~once per frame in settled scenes. Reusing the pair list across substeps (re-validate against the drift budget instead of re-querying) is the algorithmic lever.
2. **Warm-start `ImpulseMap` probing (~12 %):** open-addressing probe per contact per substep; a persistent contact cache keyed by pair (Box2D v3's persistent islands direction) would amortize it.
3. **Parallel pair collection + narrow phase (~90 % combined):** safe deterministic splits exist (per-worker proxy ranges emitting per-worker pair/contact runs concatenated in worker order), but the mid-substep wake path is order-sensitive and needs a two-phase design of its own.

### 7. Close-out

- New `DECISIONS.md` entry: persistent broadphase state + fixed substeps + speculative CCD + sleeping (supersedes the "rebuilt per substep, no persistent state" and "velocity-derived substeps" clauses of D-033; keeps the hand-rolled mandate).
- Final capture set: canonical `physics-stress 300` + `--stress-count 10000/20000` rows; update this plan's status to `done` and archive per convention.

#### Closed 2026-07-13

**DECISIONS audit — no new entry needed.** The bullet above was written before steps landed; the supersession was recorded incrementally instead of as one final entry, and DECISIONS entries are immutable, so writing a combined entry now would duplicate D-062–D-067. Verified: D-062 explicitly supersedes D-033's "rebuilt per substep, no persistent state" clause and D-064 its "velocity-derived substeps" clause (both in the entry Consequences and the `docs/DECISION_INDEX.md` rows); D-063 records the impulse-persistence carve-out; D-065/D-066/D-067 each affirm the hand-rolled mandate stands; the decision-index coverage test passes in both directions, and grepping `D-033` in either doc surfaces the supersession notes.

**Final capture set (2026-07-13, RADV REMBRANDT, vulkan, release).** ⚠ Environment caveat: a NoMachine remote-desktop session was attached during all four runs, with its encoder (`nxcodec.bin`, `nice -20`) taking ~0.3–0.5 core continuously — the machine was not canon-idle and these rows are **not like-for-like** with the 2026-07-08 rows. The inflation tracks presented frames per second (the encoder works per present): ≈ +43% at 3k, +10–15% at 10k, +3–5% at 20k versus the step-4 rows, while criterion and the substep-probe steady states (re-verified at step 6's revert on identical code) are unchanged — environmental, not a code regression. Pollution only inflates times, so the box PASS below is conservative; re-capture the 3k row on a locally-seated idle machine before using it as a comparison anchor.

| Row (dir under `perf-runs/`) | Baseline (2026-07-08) | Step 4 (D-065) | Close-out (2026-07-13) |
| --- | --- | --- | --- |
| physics-stress 3k, full capture (`20260713T184012Z-physics-stress/`) | total 6.55 / update 5.57 | total 8.78 / update 8.30 (p95 11.68) | total 12.56 / p50 15.13 / p95 17.75 / p99 19.91; update 11.36 (p95 16.80); GPU 0.15 |
| 10k `--telemetry-only` (`20260713T184208Z-…-count10000/`) | total 36.1 / update 34.7 | total 27.50 / update 26.37 (p95 48.91) | total 30.18 / p50 46.92 / p95 56.04 / p99 59.50; update 28.55 (p95 54.60) |
| 20k `--telemetry-only` (`20260713T184409Z-…-count20000/`) | total 120.7 / update 118.1 (p95 216.18) | total 110.63 / update 108.57 (p95 149.84) | total 116.46 / p50 111.53 / p95 154.59 / p99 167.40; update 113.71 (p95 151.81) |
| 20k steady state, 1500-frame diagnostic window, **non-canonical** like step 4's (`20260713T184533Z-…-count20000/`) | — | last 300: total avg 8.28 / p95 9.05 / p99 9.45; update avg 6.37 | last 300: **total avg 6.78 / p50 6.69 / p95 8.22 / p99 8.45 (~147 FPS); update avg 3.63** |

The canonical 300-frame windows remain spawn-fall/churn-dominated as at step 4 (the 10k/20k piles never settle inside the window), so those rows carry the codec inflation on top of churn cost. The steady-state signal is the long window: 100-frame bucket averages of total run `83 → 131 → 105 → 103 → 102 → 52 → 7.0 → … → 6.8` — the pile fully sleeps around frame ~600 and holds flat. **The plan-level 20k steady-state box holds** (≥60 FPS, p95 ≤ 16.7 ms): p95 8.22 ms ≈ 2× margin, and the improvement over step 4's 8.28/9.05 despite the polluted environment is step 5's SoA staging showing up in-app (that steady-state row predates D-066).

**Deferred:** "25k awake churn ≤ ~16 ms" stays unmet (124.9 ms at step 5; step 6 dropped per D-067 — the frame is pair-query/narrow-phase bound, solve ≈ 1.7% of cycles). Future-work levers are recorded under step 6/D-067: pair-list reuse across substeps, warm-start impulse-map probe cost, parallel pair collection + narrow phase. Not chased in this plan.

Close-out verification: `cargo fmt` clean; `cargo test --workspace` green (626 passed). Plan archived to `docs/plans/archive/physics-scale-and-ccd.md`; the `docs/LLM_INDEX.md` physics-bench row now points at the tests + perf workflow only.

## Done-when (plan level)

- [x] Tunneling matrix reports 0 misses for all four scenarios at all speeds up to 15,360 px/s (256 px/frame), asserted in `physics_tunneling` (step 3, D-064).
- [x] In-app `physics-stress` at `--stress-count 20000`: steady-state ≥60 FPS, p95 total ≤16.7 ms, avg update well under budget after settle — step 4 (D-065): settled tail runs total avg 8.28 ms / p95 9.05 ms (~120 FPS), update avg 6.37 ms.
- [x] `dense_pile/25000` steady-state (sleeping) ≤ 2 ms — step 5, D-066: probe slept 25k at 1.90 ms/step; re-confirmed 1.99 ms after step 6.
- [ ] `dense_pile/25000` awake churn ≤ ~16 ms ❌ — **deferred to future work** (124.9 ms after step 5; step 6 evaluated and dropped per D-067 — the frame is pair-query/narrow-phase bound, not solver bound). Levers recorded under step 6/D-067: pair-list reuse across substeps, impulse-map probe cost, parallel pair collection + narrow phase. Explicitly out of this plan's close-out.
- [x] `pile_plus_bullet` no longer shows global substep amplification (≤1.3× dense_pile at same count) — step 3: 10k 1.03×, 25k 0.85×.
- [x] Every landed step has its criterion delta and miss-rate matrix recorded in this file — verified at close-out: steps 1–5 each carry their criterion table plus the miss-rate record (full matrix at step 3, "byte-identical, not re-pasted" notes elsewhere), and the step-6 drop record includes its criterion-vs-step-5 rows and matrix note.
- [x] `cargo fmt && cargo test --workspace` green; DECISIONS entries written across D-062–D-067 (audit at close-out: no combined entry needed); D-033 cross-referenced from D-062/D-064 in both `DECISIONS.md` and `docs/DECISION_INDEX.md`.

## Research notes (condensed)

Per-area choices, grounded in the current implementation; full source list at the end.

- **CCD — speculative contacts, not more substepping.** Box2D v3 uses a hybrid speculative + TOI approach; speculative needs only signed distance (trivial for circle/AABB, no rotation) and an enlarged pair margin, and it covers the dynamic-vs-dynamic and circle-target classes the current static-AABB sweep misses. Known artifacts: ghost collisions on internal edges (mitigate via `face_mask`, modest margins) and killed restitution (mitigate via stored approach velocity + inelastic threshold).
- **Broadphase — counting-sort flat grid.** For 20k–25k uniform circles (a DEM/particle workload) the canonical structure is a flat grid rebuilt per frame via count → prefix sum → scatter; zero allocations, cache-linear, branch-light. Hashed `std` maps pay SipHash + per-cell alloc churn — the current grid drops every bucket `Vec` on `clear()` every substep. SAP degrades under clustering (our target case); BVH pays off for mixed sizes/statics, not uniform circles.
- **Solver — TGS Soft pattern.** Catto's Solver2d study: warm-started accumulated impulses + soft constraint bias + "more substeps, fewer iterations" (4×1 + relax beats 1×4+), with defaults contact hertz 30, damping 10, slop ~0.005 m. Removes the 4× redundant narrow phase and stabilizes deep piles.
- **Sleeping — per island, never per body.** Per-body sleep undermines stacks. Serial union-find per step is O(n α(n)) and deterministic; Box2D v3's persistent islands are ~10× faster still but need contact begin/end events — do union-find first. This is the decisive lever: 25k permanently-awake bodies ≈ 0.5–1M impulse solves/frame (borderline on a 6600H), but settled piles sleep.
- **Layout/parallelism — SoA first, threads maybe.** Box2D v3's >2× over v2.4 came mostly from data layout before threading. If threading is needed: contact graph coloring keeps determinism (Rapier ships determinism and parallelism as mutually exclusive; coloring is how Box2D keeps both).

Sources: box2d.org Solver2d + v3 release + simulation-islands posts; Catto GDC 2013 (Continuous Collision) and GDC 2019 (Dynamic BVH); Box2D v3 docs/types.c defaults; Wildbunny speculative-contacts article; bepuphysics2 CCD docs; Semrau ghost-collisions post; GPU Gems 3 ch. 32; 0fps collision benchmarks; BEPU broadphase benchmarks; Rapier determinism docs.

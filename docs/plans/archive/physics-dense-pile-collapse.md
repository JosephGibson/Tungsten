# Physics: dense pile collapse on slow frames

- **status:** done
- **goal:** Stop an awake dense pile from collapsing into an overlapped clump (physics above 200 ms a frame, bodies through the floor, no recovery) once frames get slow, by bounding the time one `physics_step` call advances; record the separate contact-capacity limit; cap the platformer's ball count.
- **non-goals:** A fixed-step accumulator with render interpolation (`D-088` leaves it to the 1.0 criteria plan); a solver change (`D-063`'s soft contacts stay); a substep bound or a stiffer contact setting (decisions 2 and 3: not taken); pair-budget or arrival-pass changes; treating the example's ball cap as the fix; perf work on what a large awake pile costs at 60 FPS.
- **files to touch:** `crates/tungsten-core/src/physics/mod.rs`, `crates/tungsten-core/src/physics/step.rs`, `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/tests/physics_tunneling.rs`, `crates/tungsten-core/tests/physics_containment.rs`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/LLM_INDEX.md`, `docs/known-issues.md`, `CHANGELOG.md`, `examples/01_platformer/src/state.rs`, `examples/01_platformer/src/systems.rs`, `examples/01_platformer/src/tests/ball_pit.rs`, this plan.
- **ordered steps:** (1) add the step bound; (2) its unit tests; (3) keep the existing stall test on a 0.1 s step; (4) add the four regression tests; (5) write the decision entry and the doc rows; (6) record the capacity limit in known issues; (7) cap the platformer's balls, as its own commit; (8) verify; (9) changelog, plan status and archive move.
- **done-when:** The three tests that fail on `0b08e6f` pass and the pinned-step guard keeps its hash; `just check`, `just smoke` and the release physics tests are green with the determinism hash at `0x088ec07a73c1b168`; all eight benchmark digests are unchanged and no owned metric reads `regressed`; the closed-loop probe reaches 18,000 bodies with nothing in free fall under the floor and at most 2 centres under the floor top once input ends; the platformer stops spawning at `BALL_CAP` and its test says so.

Date: 2026-10-02. Investigation only: nothing under `crates/` or `examples/` was changed. Tree `0b08e6f`, branch `0.37`. Implemented the same day as `D-094`; see "Outcome" at the end.

## Context digest

**Symptom.** In `examples/01_platformer`, a pit holding 10,000+ balls crunches into an overlapped clump when agitated. The overlay then shows physics above 200 ms a frame, it never recovers, and balls tunnel.

**Cause.** A feedback loop through the frame time, not a broken pass.

1. Contact stiffness follows the substep: `soft_params` caps the contact frequency at a quarter of the substep rate, and the substep is the frame dt over 4. A frame longer than 1/30 s softens ball-ball contacts with dt squared (9 times softer at the 0.1 s cap of `D-088`). Static contacts soften from 1/60 s.
2. A softer pile sinks into itself, so it has more contacts and, because the pair budget and the arrival-pass threshold also scale with dt, far more pairs (55 thousand to 1.3–3.2 million) and arrival listings (77 to 5,000–11,000 a substep).
3. The step gets slower, dt grows, and the loop ends at the dt cap with a step that costs more than the cap. That is why it never recovers.

Stepped at 1/60 s again, the collapsed pile is back to normal within a second, minus the bodies that already fell out. A three-frame stall is enough to capture an awake 11,500-ball pile.

**Not the pit, not the balls.** A plain box, AABB bodies, one ball size, rain spawning, the other spawn order and gravity 900 all collapse the same way. No black hole is needed. The owner's belief is confirmed.

**Second finding, by design.** A soft contact sags in proportion to its load and stops answering at about 65 ball weights at the example's gravity (`D-063`, `max_push_speed`). The bottom of an 11,500-ball pile is 43% overlapped at a steady 60 FPS. That pile is stable and sleeps; it is the crunched look, not the failure.

**Fix.** Bound the simulated time of one `physics_step` call at 1/30 s (verified in a scratch prototype: no collapse to 18,000 bodies, every existing physics test green, hash unchanged). Physics runs in slow motion below 30 FPS instead of collapsing.

**Evidence.** `perf-runs/20261002-dense-pile-collapse/` (machine-local, `README.md` inside). The numbers this plan needs are repeated below.

## Method

Headless probes through the public API in a scratch copy of `crates/tungsten-core`, with counters added to the copy's `step.rs` (`probes/step_instrumented.diff`; no behaviour change, the hash and release tests pass with it).

- **Column probe:** N unit-mass balls stacked on a static floor, sleeping off, read after 8 s at six frame dt values.
- **Pile probe:** the real pit (the collision layer of `level.tmj`, columns 132–180, floor at row 46), the example's spawner (large every 32 ms, small every 6.4 ms, both at the cursor with 4 px golden-angle jitter, zero velocity, `inv_mass` 1, restitution 0.85), gravity 3,600, cell size 64. The frame dt is a parameter per phase: fixed, or closed loop (next dt = last measured `physics_step` time + a fixed overhead, floored at 1/60 s for a vsynced frame and capped at 0.1 s as `frame_dt_secs` does).
- Readings per 30 frames: bodies, awake, pairs, contacts, pair builds and repairs, arrival listings, sweep clamps, wakes, step time, and from body positions: overlapping pairs, mean and maximum penetration, `stacking` (summed body area over covered area; 1 = no overlap), pile height, centres below the floor top and past a wall face.

Counts are deterministic wherever the frame dt is fixed. Step times in side-by-side runs are not used. The closed-loop timings quoted are from a serial run with a per-second load log: no `nxcodec.bin`, no second session, load at most 1.08.

## Findings

### 1. What a contact can carry

From `soft_params` and `solve_contacts`, a contact under a steady load `L` rests at

`penetration = linear_slop + L / (m_eff · ω²)`, with `ω = 2π · min(contact_hertz, 0.25 / sub_dt)`,

and the push stops growing once `bias_rate · (penetration − slop)` reaches `max_push_speed`. With 4 substeps, `0.25 / sub_dt = 1 / dt`: ball-ball contacts run at `min(30, 1/dt)` Hz and static contacts at `min(60, 1/dt)` Hz.

The column probe matches this to three digits below the push cap, which is about 8 px of penetration here; past it the contact saturates and then fails (radius 7.5, gravity 3,600; lowest ball-ball contact, px; the formula's value in brackets):

| Balls | 16.7 ms | 33.3 ms | 50 ms | 66.7 ms | 100 ms |
| --- | --- | --- | --- | --- | --- |
| 5 | 1.06 (1.06) | 1.06 (1.06) | 2.07 (2.07) | 3.49 (3.49) | 7.55 (7.55) |
| 10 | 2.07 (2.07) | 2.07 (2.07) | 4.35 (4.35) | 7.55 (7.55) | order lost, 1 below the floor |
| 20 | 4.10 (4.10) | 4.10 (4.10) | 8.26 (8.91) | 9.63 (15.65) | 17 of 20 through the floor |
| 40 | 7.89 (8.15) | 7.89 (8.15) | 9.87 (18.03) | all 40 through | 33 of 40 through |

- Sag per ball weight on a ball-ball contact: 0.20 px up to 33 ms, 0.46 px at 50 ms, 1.82 px at 100 ms. A small ball's contact distance is 15 px.
- Largest steady load a ball-ball contact answers: 65 ball weights at 1/60 s, 45 at 50 ms, 23 at 100 ms. Floor contact: 271, 90 and 45.
- AABB bodies read the same values. Radius 15 reads the same penetrations, so it tolerates twice the load before order is lost.
- `D-088` chose 0.1 s from a 5-box stack and a 30-circle pile at gravity 900. At the example's gravity a column of 10 small balls does not survive 0.1 s frames.

### 2. Trigger conditions

**Fixed-step sweep.** 11,502 mixed balls built in the pit at 1/60 s (step 13 ms, stacking 1.32, height 733 px), then 15 s at a slower fixed step:

| Frame dt | Stacking | Height px | Below floor | In walls | End state |
| --- | --- | --- | --- | --- | --- |
| 16.7 ms | 1.32 | 716 | 0 | 0 | asleep |
| 25 ms | 1.32 | 716 | 0 | 0 | asleep |
| 33.3 ms | 1.31 | 718 | 0 | 0 | asleep |
| 41.7 ms | 1.48 | 627 | 4 | 0 | asleep, compressed |
| 50 ms | 1.64 | 552 | 171 | 5 | asleep, compressed |
| 66.7 ms | 1.93 | 449 | 315 | 18 | awake |
| 100 ms | 2.29 | 347 | 938 | 46 | awake, step 206–287 ms |

The threshold is where the formula puts it: nothing changes up to 1/30 s and everything does past it.

**Closed loop.** Frame dt = measured step + 8 ms, spawning held (quiet machine):

| Bodies | Frame dt | Step | Pairs | Stacking | Below floor |
| --- | --- | --- | --- | --- | --- |
| 12,003 | 22.2 ms | 14.5 ms | 66 k | 1.34 | 0 |
| 15,940 | 32.8 ms | 24.8 ms | 125 k | 1.45 | 0 |
| 16,136 | 36.9 ms | 26.9 ms | 152 k | 1.49 | 8 |
| 16,539 (30 frames later) | 100 ms | 285 ms | 6.1 M | 2.00 | 699 |
| 17,665 | 100 ms | 453 ms | 3.5 M | 2.63 | 1,230 |
| 18,002, input released, 10 s later | 100 ms | 498–536 ms | 2.9 M | 2.42–2.70 | 1,580–1,699 |

- The loop closes within one 30-frame window of the frame dt passing about 35 ms. The body count at which that happens is whatever makes the frame that long: 16,100–16,500 here with 8 ms of overhead, 9,900 with 20 ms (that run had the encoder connected). The example's real overhead (render, extract, per-ball systems) was not measured; the owner's "10k+" fits an overhead near 20 ms.
- **A stall does it too.** The 11,502-ball pile, awake and stable at 60 FPS, given 3 frames of 0.1 s (a dragged window, a hitch) and then the closed loop: captured, 1,072 below the floor 30 s later. With 10 stalled frames: 1,068. With the encoder connected and a noisier step time, the 8 ms spawn run was captured at 12,100 bodies by a spike.
- **The black hole is not needed and not sufficient.** At a fixed 1/60 s a black hole held in the 11,502-ball pile for 10 s raises stacking from 1.32 to 1.35 and the step from 13 to 14 ms; the pile recovers and sleeps. In the game it matters as the thing that wakes the whole pile and adds frame time.

### 3. Why the step reaches 200 ms

The 11,502-ball pile at 1/60 s and at 0.1 s frames:

| Per substep unless noted | 1/60 s | 0.1 s |
| --- | --- | --- |
| Broadphase pairs | 55 k | 1.3–3.2 M |
| Contacts | 37 k | 90–97 k |
| Arrival-pass listings | 77 | 5,300–11,200 |
| Pair builds per frame | 1.00 | 1.00 |
| Pair repairs per frame | 3.0 | 0.5–2.9 |
| Sweep clamps per frame | 0 | 4–19 |
| Contact wakes | 0 | 0 |
| `physics_step` | 13 ms | 206–287 ms |

Profile of the collapsed pile (`perf`, 499 Hz, quiet machine): 55.5% `substep` (the pair loop with the narrow phase and the arrival pass inlined), 33.6% `build_pairs` plus `SpatialGrid::build`, 3.6% `solve_contacts`. The cost is the pair list, not the solver. Three things grow with dt at once:

- overlap, from the softer contacts (contacts ×2.6);
- the pair budget of `D-081`: its gravity term `|g|·h²·(n−1)(n+2)/2` is 0.56 px at 1/60 s and 20.25 px at 0.1 s with gravity 3,600, on top of `|v|·t_left`, so every awake body's pair box grows by more than its own size (pairs ×25–60);
- the arrival flag of `D-092`: its threshold is `2·linear_slop / sub_dt`, 120 px/s at 1/60 s and 20 px/s at 0.1 s, while one substep of gravity adds 15 px/s and 90 px/s. Past a substep of 11.8 ms (frame dt 47 ms at gravity 3,600) every body gravity moves is listed.

### 4. Why it does not recover

The collapsed state is not absorbing in itself. Stepped at a fixed 1/60 s after 15 s at 0.1 s, the pile returns from stacking 2.29 and height 347 px to 1.28 and 737 px. Within 60 frames the pair list is back from 1.3 million to 49 thousand and the contacts from 95 to 38 thousand a substep. 228 bodies stay lost: they were already under the floor.

In the game the step costs more than the 0.1 s cap, so dt stays at the cap, so the contacts stay 9 times softer and the pair boxes stay large. Before `D-088` there was no cap, the same loop had no fixed point, and the game froze.

Two details:

- A pile that falls asleep while compressed (the 41.7 and 50 ms rows) stays compressed after dt recovers, because sleepers are frozen (`D-065`). It recovers when something wakes it.
- A sleeping pile is safe at any dt (`D-088` already says so). The failure needs an awake pile and a slow frame together, which is what continued spawning and the black hole provide.

### 5. The pit is incidental

Each built at 1/60 s, then 10 s at 0.1 s frames, then 10 s at 1/60 s:

| Run | Bodies | Stacking, height at 1/60 s | At 0.1 s | Below floor, in walls | After recovery |
| --- | --- | --- | --- | --- | --- |
| Box 1,600 px, circles r 10, rain | 6,000 | 1.34 | 2.05, 533 px | 222, 68 | 1.37, 10 lost |
| Box 1,600 px, AABB half 7.5, rain | 8,000 | 1.68, 1,063 px | 1.98, 566 px | 272, 62 | 1.74, 28 lost |
| Box 1,600 px, circles r 10, rain, shallow | 1,500 | 1.03 | 1.77, 150 px | 82, 0 | 1.07, 284 px, 0 lost |
| Box 2,400 px, circles r 30, rain | 1,500 | 1.03 | 1.51 | 36, 6 | 1.03, 0 lost |
| Pit, small spawned first each frame | 11,502 | 1.32, 732 px | 2.32, 346 px | 889, 51 | 1.28, 170 lost |
| Pit, rain instead of the cursor | 11,502 | 1.33, 758 px | 2.02, 417 px | 767, 45 | 1.31, 126 lost |
| Pit, small balls only | 11,500 | 1.41, 491 px | 2.75, 200 px | 932, 38 | 1.37, 200 lost |
| Pit, gravity 900 | 11,502 | 1.07, 970 px | 1.59, 576 px | 171, 0 | 1.06, 2 lost |

No container, shape, size, driver or spawn order avoids it; a 1,500-ball pile 284 px deep is enough. Spawn order moves the numbers by a few percent.

### 6. Hypotheses from the brief

| Hypothesis | Verdict | Evidence |
| --- | --- | --- |
| Overlap accumulates faster than the soft solver resolves it | Rejected at a steady step, true through dt | At a fixed 1/60 s overlap is a static, load-proportional sag that neither grows nor feeds on itself, with or without the black hole. It runs away only when dt grows. |
| Pair and contact counts grow superlinearly; 200 ms follows from the overlap | Confirmed, with two more multipliers | Section 3. Pairs ×25–60 against contacts ×2.6: most of the pair growth is the dt-scaled budget, not the overlap. |
| Pair repair falls back to a full rebuild every frame (`D-075`, `D-081`) | Rejected | 1.00 builds per frame before and during the collapse; the quarter fall-back does not fire. |
| Broadphase layout (`D-062`, `D-080`) | Not a cause | 34% of the collapsed step is building the list, which is the list's size. |
| Arrival pass (`D-092`) or CCD (`D-064`) amplify | Confirmed as a cost multiplier, not a cause | Listings ×70–145; 4–19 sweep clamps a frame. Neither changes the outcome at 1/60 s. |
| One giant island that never sleeps, or sleepers waking each other (`D-065`, `D-082`) | Rejected | 0 contact wakes in every run; every pile sleeps within 8 s of input ending when dt is healthy. The collapsed pile stays awake because it is moving. |
| The state is absorbing | Rejected | Section 4. It is held only by dt. |
| The dt cap turns a slow frame into a spiral (`D-088`) | Confirmed, this is the root cause | Sections 1–4. The cap is the loop's fixed point and sits 3 times past where stiffness starts to fall. |

### 7. Capacity limit or feedback loop

Both, and they are separate.

- **Feedback loop (fixable, the bug).** Stiffness, pair budget and arrival threshold all depend on the frame dt, and the frame dt depends on the step's cost. Bounding the simulated time per call breaks it.
- **Capacity limit (by design).** At any frame rate a soft contact sags 0.20 px per ball weight at gravity 3,600 and gives up near 65. The pit is 3,072 px wide, so 11,502 unit-mass balls put about 56 ball weights on each bottom small ball and 112 on each large one. Measured at a steady 60 FPS: nearest-neighbour distance over summed radii is 0.57 in the bottom 128 px, 0.75 at 256–384 px and 0.97 near the top; the pile is 733 px tall where non-overlapping balls would stand about 950 px. It costs 13 ms and sleeps. Levers, on the same pile at 1/60 s:

| Change | Stacking | Height px | Pairs past a quarter overlap | Contacts per substep | Note |
| --- | --- | --- | --- | --- | --- |
| Defaults | 1.32 | 733 | 9,127 | 37.6 k | bottom band 0.57 |
| `contact_hertz` 60 | 1.06 | 951 | 183 | 32.0 k | bottom band 0.83; sleeps after 15 s, not 8 s |
| `max_push_speed` 480 | 1.32 | 734 | 9,310 | 37.6 k | no effect: the load spreads below the cap |
| 8 substeps | 1.33 | 731 | 9,210 | 37.8 k | no effect: the cap was not binding |
| Mass by area (small = 1/4) | 1.34 | 775 | 8,027 | 37.8 k | no gain |
| Gravity 1,800 | 1.15 | 870 | 1,598 | 33.1 k | changes the game |

`contact_hertz = 60` is the only lever that works, it is free at 60 FPS (the quarter-rate cap allows 60 Hz at a 1/240 s substep), and it moves the softening threshold from 1/30 s to 1/60 s. It therefore needs a constant substep. With the step bound alone the pile's height follows the frame time (stacking 1.06 at 16.7 ms, 1.16 at 25 ms, 1.31 at 33.3 ms, no body lost), and in closed loop it does not settle (decision 3).

## Fix options, ranked

All measured with `probes/step_prototype.diff` in the scratch copy. A's pile runs used the prototype's equivalent setting (substep bound 1/120 s, at most 4 substeps); its test runs used the step bound itself. "Pinned step" means dt = 1/60 s, which benchmarks, smoke runs and the pixel test use (`D-088`).

| # | Option | Measured | Cost | Hash and digests | Proving check |
| --- | --- | --- | --- | --- | --- |
| A | **Step bound (taken, decision 1).** `PhysicsConfig::max_step_dt`, default 1/30 s: `physics_step` advances `min(dt, max_step_dt)`. Substep count unchanged, so the substep never exceeds 1/120 s and ball-ball contacts keep 30 Hz. Below 30 FPS physics runs slow instead of soft. | 0.1 s frames on the 11.5k pile: 0 below the floor, stacking 1.31 (tree: 938 and 2.29). Closed loop to 18,000: frame 35–41 ms, step 27.5–33 ms, 11 centres under the floor top during spawning and 1 after (tree: 448–536 ms, 1,287–1,699). 3-frame stall: recovers, sleeps, 0 lost. Pair list at 0.1 s: within 3× of 1/60 s (tree 13×). | One `min` per frame. No cost at any frame rate. Below 30 FPS simulated time falls behind frame time. | Unchanged at the pinned step: hash `0x088ec07a73c1b168` holds, containment 0 of 3,000, all 468 unit tests and the release tests pass with it on. | Tests 1–4 below; closed-loop probe. |
| B | **Substep bound, on top of A (not taken, decision 2).** `max_substep_dt` 1/240 s with up to 8 substeps: the count rises from 4 to 8 between 60 and 30 FPS, then A's bound applies. Full nominal stiffness for statics too; makes `contact_hertz = 60` hold at every frame rate. | 0.1 s frames: 0 below the floor, 1.32. Closed loop to 18,000: 0 below the floor, frame 51–55 ms, step 44–47 ms. With `contact_hertz` 60: stacking 1.06 at 0.1 s frames. | Up to 2× physics per frame between 60 and 30 FPS: at 12,000 bodies the frame went from 22 to 33 ms. Amends `D-064`'s fixed count. `substeps` becomes a minimum: two unit tests that pin a count (`sweep_rents_the_pair_grid_until_queries_reach_the_static_count`, `persistent_pairs_match_fresh_contacts_on_randomized_piles_bullets_and_wakes`) and the bench's `substeps` knob would need the bound off. | Unchanged at the pinned step (same count): hash and release tests pass. | Tests 1–4. |
| C | **Example stiffness (not taken, decision 3).** `cfg.contact_hertz = 60.0` in the platformer's `seed_world`. Addresses the capacity limit, not the loop. | At a fixed 1/60 s: stacking 1.32 to 1.06, contacts −15%, 15 s to sleep instead of 8 s. With A alone and a measured frame time of 17–21 ms after a 3-frame stall: still awake after 25 s, mean speed 4–24 px/s, height wandering between 889 and 924 px; the default pile sleeps 9 s after the same stall. At 0.1 s frames with A: 0 below the floor at 11.5k, 21 centres pressed under the floor top at 18,000, all back after recovery. | A pile that does not sleep costs its 12 ms step every frame. | Engine untouched. The example has no digest. | None: not taken. |
| D | **Example ball cap (taken, decision 4).** `BALL_CAP = 12_000` in the platformer; spawning stops at the count. | Not prototyped. From the closed-loop runs: 12,000 awake balls cost a 14.5 ms step at stacking 1.34; 18,000 cost 28–33 ms at 1.50. | A constant, a count and a budget in `spawn_ball_system`. | None. | The cap test of step 7. |
| E | Real time down to 10 FPS: substep bound 1/120 s with up to 12 substeps and no step bound. **Not recommended.** | Pile intact (2 below the floor at 18,000), but the pair budget still scales with the whole frame: pairs 4,392 to 29,694 in test 3, 420–480 k on the 11.5k pile, frame 100 ms and step about 117 ms at 18,000 (encoder state not logged for that run). | Needs a shorter pair-budget horizon (`D-081`) to pay off. | Unchanged at the pinned step. | Fails test 3. |
| F | Fixed-step accumulator with render interpolation. | Not prototyped. | Changes the core/render seam; `D-088` assigns it to the 1.0 criteria plan. A is what that accumulator would do for stability and is compatible with it. | Would change results. | Out of scope. |

**Graceful degradation.** A is the degradation path the brief asks for: under overload the simulation slows down and stays correct, and it catches up by itself when the load drops. A penetration-recovery cap already exists (`max_push_speed`) and is not what fails. A contact or pair budget would only be needed for E. Recovering a stuck pile needs no mechanism once dt is bounded, because the state is not absorbing (section 4); bodies already under the floor are culled by the example's `despawn_out_of_bounds`.

**The example ball cap is a presentation guard, not the fix.** With A the pit holds, but 18,000 awake balls still cost a 28–33 ms step, and the capacity limit makes a deep pile look crushed at 60 FPS. Any game can reach the loop with a hitch and a 1,500-ball pile, so the engine change stands on its own and lands first.

## Decisions

Closed on 2026-10-02. The owner delegated all four to the investigating session's judgement. The reasons are written out so that reopening one is cheap.

1. **The bound lives in `PhysicsConfig` (`max_step_dt`, default 1/30 s); `MAX_DT_SECS` stays 0.1 s.** `physics_step` is public and runs headless with whatever dt its caller holds, so "a substep is never longer than 1/120 s" is the step's invariant, not the app loop's. Lowering `MAX_DT_SECS` instead would reverse `D-088`'s stated reason for 0.1 s and slow every system of every game under 30 FPS for a problem only physics has. Accepted consequence, to be written into the decision entry: below 30 FPS game systems see the frame dt while physics advances 1/30 s, so a force applied as `vel += a·dt` (the black hole) is stronger relative to the motion it causes, and simulated time falls behind frame time until the load drops.
2. **No substep bound (option B).** A alone removes the failure. B's cost lands between 30 and 60 FPS, where games actually run (22 ms to 33 ms a frame at 12,000 bodies), and it amends the fixed count `D-064` chose to stop whole-world cost from multiplying. What B buys, a substep that does not depend on the frame, is what the fixed-step accumulator of the 1.0 criteria plan delivers; revisit it there.
3. **No `contact_hertz = 60` in the platformer (option C).** It needs a constant substep. Measured with A alone: the pile's height follows the frame time, and at a measured frame time of 17–21 ms it never settles or sleeps (25 s awake against 9 s for the default), so it would cost its 12 ms step on every frame. The lever and its condition go into known issues; it becomes a one-line change once the substep is constant.
4. **Ball cap in the platformer (option D): `BALL_CAP = 12_000`, a count, in its own commit after the engine change.** A count is deterministic, testable and the same on every machine; a gate on the frame dt is none of those. 12,000 is where the reference machine's awake step is 14.5 ms and the pile's overlap is still moderate (stacking 1.34). The number is a presentation choice and is the one thing here the owner may simply want different.

## Steps for the implementing session

Read `physics_step` and `soft_params` in `crates/tungsten-core/src/physics/step.rs` first; nothing else in the step changes. Steps 1–6 are the engine change, step 7 is a separate commit.

1. **Bound.** Add `max_step_dt: f32` to `PhysicsConfig` (default `1.0 / 30.0`; `<= 0` means unbounded) with a doc comment naming the stiffness cap it protects. In `physics_step`, clamp once after reading the config and use the clamped value everywhere the frame's dt is used today: `sub_dt`, `time_left`, `integrate_loose_bodies` and `sleep_frame_end`. No state, no globals, no logging in the step.
2. **Unit tests** in `crates/tungsten-core/src/tests/physics/step.rs`: a free-falling body handed a 0.1 s dt advances by the bound; `max_step_dt = 0.0` restores the tree's result bit for bit; a 1/60 s step is bit-identical with the bound at its default and off.
3. **Existing stall test.** `one_capped_stall_step_keeps_a_settled_pile` in `physics_tunneling.rs` hands the step 0.1 s to pin `D-088`'s table. It passes with the bound on but would no longer take a 0.1 s step: set `max_step_dt: 0.0` in it and say why in its comment.
4. **Regression tests** from the next section: the column test into `physics_tunneling.rs` (runs in debug), the two pile tests and the guard into `physics_containment.rs` (release only, same `ignore` attribute as its neighbours). Confirm tests 1–3 fail before step 1 is applied (or with `max_step_dt = 0.0`) and say so in the hand-off.
5. **Decision.** New entry (next free number, `D-094` at the time of writing): the bound and decision 1's consequence, the threshold table of section 2, the loop of section 3, and a "Not taken" part for options B, C and E with their measurements. It amends `D-088` (physics no longer takes the full capped dt; the 0.1 s table holds for the scenes it lists, not for deep piles or gravity 3,600) and `D-033`'s variable-dt limit: marker lines on both, the `docs/DECISION_INDEX.md` row in the same change (test-enforced), the decision added to the physics row of `docs/LLM_INDEX.md`. Cite this plan by its archive path.
6. **Known issues.** Under "Recorded limits" in `docs/known-issues.md`: the capacity limit of section 7 with its formula and the 65-ball-weight figure at gravity 3,600; `contact_hertz` as the lever, and that it needs a constant substep (decision 3).
7. **Example ball cap**, own commit. `pub(crate) const BALL_CAP: usize = 12_000;` beside the other ball constants in `state.rs`, with a comment giving the reason and the decision number. In `spawn_ball_system`, count live `Ball` entities once and pass the remaining budget through both `spawn_balls` calls, so a frame never overshoots. A spawner with no budget left drops its accumulated time, as it does when its input is released, so balls do not burst out when the count later falls. One test in `tests/ball_pit.rs`: with `BALL_CAP − 1` balls alive and both inputs held through a frame that would spawn several, exactly one ball is spawned, and the next frame none. No other example tests.
8. **Verify** (the section after next).
9. **Close.** `CHANGELOG.md` under `[Unreleased]` per `docs/releases.md`. Set this plan's status to `done`, move it to `docs/plans/archive/` under the same name and fix its relative links for the new depth.

## Regression tests

Source: `perf-runs/20261002-dense-pile-collapse/probes/zz_candidate_tests.rs`, public API only, already run on tree behaviour and on the prototype (`out/candidate_tests.txt`, `out/step_bound_tests.txt`). If that folder is gone, these specifications are enough to rewrite them. Shared setup: gravity (0, 3,600), cell size 64, default config otherwise, static floor slab with its top at y = 2,000.

| # | Test | Scene and steps | Assert | On `0b08e6f` | With A |
| --- | --- | --- | --- | --- | --- |
| 1 | `stacked_column_survives_slow_frames` (debug) | 20 circles r 7.5 stacked touching on the floor, sleeping off; 240 steps at 1/60 s, then 80 at 0.1 s | every centre above the floor top; every neighbour gap above 7.5 px | fails: ball 0 at y = 2,003.1 | passes |
| 2 | `awake_pile_keeps_its_bodies_and_height_through_slow_frames` (release) | 1,500 circles r 10 in a 1,600 px box (75 per row, 21 px pitch, odd rows offset 10.5 px); 240 steps at 1/60 s; sleeping off; 100 steps at 0.1 s | 0 centres below the floor top; height (floor minus the 2nd percentile of y) above 0.85 of its value before | fails: 91 through the floor | passes |
| 3 | `slow_frames_do_not_multiply_the_pair_list` (release) | the same pile; 30 steps at 1/60 s, read `pair_count()`; 30 steps at 0.1 s, read again | second reading at most 3× the first | fails: 4,392 to 59,185 | passes |
| 4 | `pinned_step_state_is_unchanged` (release) | the same pile after its 240 steps at 1/60 s; FNV-1a over the position bits in spawn order | equals `0xaee272e01ffc4e4c` | passes | passes |

Test 1 is the headless test that fails on the current tree and passes after the fix; tests 2 and 3 pin the two halves of the loop (bodies and work); test 4 pins that nothing else moved.

## Verification

1. `just check`.
2. `RUSTFLAGS="-C force-frame-pointers=yes" cargo test --release -p tungsten-core --locked --test physics_determinism --test physics_tunneling --test physics_containment`: green, hash `0x088ec07a73c1b168`, containment 0 of 3,000. These are ignored in debug, so `just check` does not cover them.
3. `just smoke` (layer 2: engine and example wiring changed).
4. Benchmarks, on a quiet machine (no `nxcodec.bin`, no second session; one foreground call per sitting): an A/A of the untouched tree first, then `WGPU_BACKEND=vulkan just perf suite --repeat 5` and `just perf compare` against the saved baseline. All eight digests unchanged. No owned metric `regressed`; the change adds a compare and a `min` per frame, so a `regressed` reading on a `physics` or `ecs` row is first checked as code placement against a padded baseline build before it is believed.
5. Closed-loop probe: rebuild the scratch copy from the changed tree, re-apply the counters of `probes/step_instrumented.diff` by hand if it no longer applies, drop the pile probe's two prototype switches (`MAX_SUBSTEP_DT`, `MAX_SUBSTEPS`), and run the first setting of `probes/batch_clean_wall.sh` (`DT=wall OVERHEAD_MS=8 TARGET=18000 QUIET_SECS=10`) as one foreground call on a quiet machine. Done when 18,000 bodies are reached with `below_floor` at most 15 during spawning and at most 2 after input ends, and the frame dt under 50 ms. The 11-and-1 reading with A is centres pressed under the floor top by the load at 30 Hz static stiffness (largest speed 76 px/s, nothing in free fall), not bodies through the floor.

**Owner check after the hand-off, not gating the plan:** `cargo run --release -p example-01-platformer`, hold both spawn inputs until spawning stops at the cap, drag a black hole through the pile, release everything. Expected: the pile keeps its height, the overlay's physics time stays in the tens of milliseconds and falls when the pile sleeps, and no ball leaves through the floor or walls. To see the engine fix without the cap, raise `BALL_CAP` locally to 20,000.

## Also noticed

Not part of the fix steps.

- **Arrival flag counts gravity.** `admitted` is stored before gravity is added, so past a substep of `sqrt(2·linear_slop / |g|)` (11.8 ms at gravity 3,600, 23.6 ms at 900) every unsupported body is listed each substep. With A the substep stays under 8.4 ms and it does not arise at 3,600; it would at a gravity above about 7,200.
- **Pair budget grows with the square of the frame dt** (`D-081`'s gravity term). Harmless once the step is bounded at 1/30 s (2.25 px), and the reason option E does not pay.
- **The engine's own piles never see this load.** Benches, `substep_probe.rs` and the containment test use gravity 900 and radius 6: a quarter of the example's sag per ball weight. The capacity limit is invisible to them.
- **Cursor spawning creates overlapped bodies.** Each frame's balls appear within 4 px of each other and of the previous frame's; a falling stream holds about 95 pairs past a quarter overlap at any time (first one at 93 bodies). Not a cause.
- **Every ball has `inv_mass` 1**, small or large. Mass by area did not change the compression (section 7).
- **A pile that sleeps compressed stays compressed** until woken (section 4). With A it no longer gets compressed by slow frames.
- **`contact_hertz` 60 doubles the time to sleep** on the 11.5k pile at a fixed 1/60 s (15 s against 8 s); with a varying frame time it does not sleep at all in 25 s (decision 3). The fixed-step half of that was not explained.

## Evidence

`perf-runs/20261002-dense-pile-collapse/` (gitignored, machine-local):

- `README.md`: files, switches, which run sets carry usable timing.
- `probes/`: `zz_column_probe.rs`, `zz_pile_probe.rs`, `zz_candidate_tests.rs`, `step_instrumented.diff`, `step_prototype.diff`, `run.sh`, `runbin.sh`, `batch_*.sh` (`batch_hertz60_step_bound.sh` is decision 3's check).
- `out/column_load.txt` (section 1), `out/pile_summary.txt` and `out/pile_<label>.csv` (sections 2–7; labels `pit_then_fixed*`, `clean_wall_*`, `stall*`, `wall_o*`, `box_*`, `pit_*`, `cap_*`, `proto_*`, `h60_a_*`), `out/perf_collapse_self.txt` (section 3), `out/candidate_tests.txt` and `out/step_bound_tests.txt` (tests), `out/clean_wall_load.txt` (load log of the timed runs).

Timing caveat: `batch_wall.sh` ran with a NoMachine encoder connected, so its step times are indicative; the closed-loop table in section 2 and the option table use the quiet-machine repeat (`batch_clean_wall.sh`), and the stall cases are quoted for their deterministic outcome (bodies lost), not their times.

## Outcome

Implemented on 2026-10-02 as `D-094` (steps 1–6) and the platformer's `BALL_CAP` (step 7). Working files: `perf-runs/20261002-dense-pile-fix/` (machine-local).

- **Red, then green.** On `0b08e6f` tests 1–3 failed as the table says (ball 0 at y = 2,003.13; 91 through the floor and height 284 → 149 px; 4,392 → 59,185 pairs) and the guard passed. With the bound all four pass: height 284.1 → 282.7 px, 0 below the floor, 4,392 → 4,979 pairs, guard hash `0xaee272e01ffc4e4c`.
- **Hash and digests.** Determinism hash `0x088ec07a73c1b168`, containment 0 of 3,000, all eight benchmark digests unchanged.
- **Benchmarks.** One sitting, five runs a side, no encoder sighting: the A/A of the untouched tree read 0 `regressed`, 0 `improved` (40 `unchanged`, 14 `noisy`); tree before → after read 0 and 0 (43 and 11). `physics_step` p50: `physics` 6.07 → 6.14 ms, `physics-sparse` 3.61 → 3.66 ms, `unchanged`. A padded build of the tree before (bench and engine functions at the changed tree's offsets modulo 64) was captured in the same sitting and also read 0 and 0.
- **Closed-loop probe** on the changed tree: 18,002 bodies, frame dt at most 39.9 ms, step 25–31 ms past 16,000. Centres under the floor top: at most 2 while spawning; 3 in the first 30-frame row after input ends, 1 in the next, then 0; the pile sleeps 8 s after input ends. The plan's "at most 2 after input ends" holds for the end reading it was written from (the prototype's rows read 20 down to 1), not for that first row. The fixed-step case (11,502 balls, 15 s of 0.1 s frames) reads 0 below the floor, stacking 1.31 and 717 px, the prototype's counts exactly.
- **Outside the file list:** one sentence in `DESIGN.md` (the physics paragraph that described the dt cap).
- **Owner check** (not gating): `cargo run --release -p example-01-platformer`, as in "Verification".

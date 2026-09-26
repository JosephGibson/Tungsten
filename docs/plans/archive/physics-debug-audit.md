# Physics Scale + CCD Debug Audit (D-062–D-067)

- **status:** done
- **goal:** find and record correctness bugs in the uncommitted physics scale/CCD work on branch `0.27` (plan steps 1–7, archived at `docs/plans/archive/physics-scale-and-ccd.md`). Findings only — fixes are a separate session.
- **non-goals:** perf work, captures, bench tuning, code changes.
- **files to touch:** This findings-only audit report.
- **ordered steps:** Read the change surface, run adversarial probes, record findings and verification.
- **done-when:** Audit and probe results recorded; completed 2026-07-13. Unresolved findings are follow-up work, summarized in `docs/repo-review-2026-09-25.md`.
- **scope audited:** working-tree diff of `crates/tungsten-core/src/physics/{broadphase,collision,step,mod}.rs`, `src/ecs/world.rs`, `src/lib.rs`, `src/tests/**`, `benches/physics_bench.rs`, plus untracked `tests/{physics_containment,physics_determinism,physics_tunneling,substep_probe}.rs`.
- **method:** full read of the change surface, line-level analysis against the hunt list, and 10 adversarial probe tests (throwaway harness, preserved at scratchpad `zz_audit_probes.rs`, removed from the repo after the session).
- **verification runs (2026-07-13):** `cargo test --workspace` green (626 passed, baseline intact); `cargo test --release -p tungsten-core --test physics_tunneling --test physics_containment --test physics_determinism -- --nocapture` — 4 passed, 0 misses in all four tunneling scenarios; ignored `substep_probe` tests run serially in release (results in the probe log section below).

## Findings

### S2-1 — Impulse chains fire a body through dynamic colliders in one substep (dyn-vs-dyn CCD hole)

- **severity:** S2 (violates the plan-level "no tunneling for dynamic targets" guarantee under reachable conditions; sim stays finite/deterministic/contained-by-statics)
- **where:** `crates/tungsten-core/src/physics/step.rs:914-915` (per-pair admission margin uses pre-solve velocities), `step.rs:1011-1025` (biased solve injects velocity *before* position integration in the same substep), `step.rs:1158-1160` (slab-sweep safety net skips dynamic targets).
- **mechanism:** Speculative admission happens at narrow-phase time with the pair margin `|v_a − v_b|·sub_dt + 4·linear_slop` measured from **pre-solve** velocities. The biased Gauss–Seidel pass then runs *before* position integration, so a contact can inject up to ~the attacker's full approach speed into a previously resting body inside the same substep. That body then integrates a displacement its own pair admissions never anticipated: any pair of it vs another **dynamic** body that was rejected earlier in the substep (relative velocity ≈ 0 → margin ≈ 1 px < gap) is simply gone, and `speculative_pass` refuses dynamic targets, so nothing clamps the motion. The victim is always inside the *attacker's* admission margin, but the attacker's contact with the victim does not constrain the launched middle body.
  - Reach threshold with defaults (substeps = 4, sub_dt = 1/240 s): injected Δv·sub_dt must exceed gap + combined extents, i.e. chains driven by bullets ≳ ~7–8 k px/s for ~10 px gaps — inside the 15,360 px/s envelope the tunneling matrix certifies. At `substeps = 1` (sub_dt = 1/60) the same geometry tunnels from ~2 k px/s.
- **minimal repro** (fails in one `physics_step`, zero gravity, default config — probe `probe_impulse_chain_fires_pawn_through_dynamic_gate`):
  ```text
  bullet: circle r=4, mass 1e6, center (780, 0), v = (15_360, 0)
  pawn:   circle r=4, mass 1,   center (800, 0), v = 0
  gate:   dynamic AABB half (2, 400), mass 1e6, center (816, 0), v = 0
  → after 1 step: pawn.x = 956.4, gate.x = 928.1 — pawn ended beyond the gate
  ```
  Substep 1: bullet–pawn and bullet–gate admit speculatively; pawn–gate is rejected (relative v = 0, gap 10 px > 1 px margin). The biased solve gives the pawn ~12.4 k px/s; integration moves it ~52 px, fully through the gate slab; no later substep can see the crossing. Control at 480 px/s does not tunnel.
- **why the existing suites miss it:** `physics_tunneling` fires bullets *directly* at targets — the bullet's own pair is always admitted with the right margin. The hole needs one hop of impulse propagation.
- **fix direction (not applied):** any of — (a) extend the `speculative_pass` sweep to effectively-immovable dynamic targets (treat `inv_mass` below a threshold, and sleeping bodies, as static in the net); (b) after the biased solve, re-check bodies whose post-solve speed grew past their pair-admission assumption and re-run the narrow phase for them before integration (Box2D v3 solves this class with its TOI/bullet pass); (c) clamp per-substep solver-injected velocity to the admission margin (behavioral change — would soften legitimate elastic chains). (a) fixes the practical "thin heavy gate / container made of dynamics" case cheaply; (b) is the complete fix.

### S3-1 — Sleeping-island tag adoption can miss when the bridging contact reads a gap on the sleep frame

- **severity:** S3 (latent; needs an exact-frame coincidence)
- **where:** `crates/tungsten-core/src/physics/step.rs:680-707` (adoption scans only the **final substep's** contacts and requires `contact.penetration > 0.0`).
- **mechanism:** An island adopts a sleeping support's tag only in the single frame it falls asleep, and only if its contact against the sleeper is *touching* (`penetration > 0`) in the frame's **last substep**. Bodies rest at ~`linear_slop` (0.25 px) penetration, but micro-pops (the plan records 1–2-frame single-body pops) can momentarily open a gap. If the only contact linking a fresh island to the sleeping island below reads `penetration <= 0` on exactly the sleep frame, the fresh island sleeps under its own tag. A later island wake of the lower pile (e.g. despawn deep in it) then wakes only the lower tag, and the upper stack stays frozen in the air on a support that no longer exists — precisely the class `late_sleeper_adopts_supporting_island_for_despawn_wake` pins for the touching case.
- **repro:** not forced; requires the gap to coincide with the sleep frame's last substep. Construction: two-ball column on a sleeping pile where the timer crosses `time_to_sleep` on the same frame a micro-pop separates the interface contact.
- **fix direction:** accumulate adoption candidates across all of the frame's substeps (like the union-find already does) instead of the last substep's buffer only, and/or admit speculative near-contacts (`penetration > -linear_slop`) for adoption.

### S3-2 — `entity_key` masks generation to 31 bits

- **severity:** S3 (theoretical)
- **where:** `crates/tungsten-core/src/physics/step.rs:403-405`.
- **mechanism:** `(1 << 63) | ((generation & 0x7FFF_FFFF) << 32) | index` — generations that differ by exactly 2³¹ on the same slot alias to the same warm-start/sleep identity, so a despawned body's accumulated impulse or sleep entry could apply to its reused slot. Requires 2³¹ despawns of one slot between two frames' contacts — unreachable in practice, but the truncation is silent.
- **fix direction:** none needed now; a doc comment noting the 31-bit generation budget (or folding generation into a hash instead of truncating) would make the invariant explicit.

### S3-3 — `tile_key` collisions merge warm-start identities of distinct tiles

- **severity:** S3 (theoretical / degenerate content)
- **where:** `crates/tungsten-core/src/physics/step.rs:409-414`.
- **mechanism:** tile identity is a 63-bit hash of the tile center's f32 bits. Two failure shapes: (a) birthday collisions (~2³¹·⁵ tiles — unreachable); (b) two overlapping collision tilemap instances producing tiles at the *same center* share a key by construction, so a dynamic body's contacts against both tiles share one accumulated-impulse slot (last writer wins in the per-substep rebuild, `step.rs:1035-1041`). Consequence is a slightly wrong warm start, self-correcting within an iteration; overlapping duplicate collision layers are degenerate content anyway.
- **fix direction:** none required; if overlapping tilemaps become supported content, mix a per-instance salt into `tile_key`.

### S3-4 — Non-finite or astronomically large coordinates can hang the broadphase build/query

- **severity:** S3 (requires externally corrupted state; physics itself keeps coordinates finite)
- **where:** `crates/tungsten-core/src/physics/broadphase.rs:90-101` (`insert` span arithmetic), `broadphase.rs:119-142` (query cell loops), `broadphase.rs:193-205` (`cell_range` casts).
- **mechanism:** `cell_range` casts f32 cell coordinates to i32 with saturating `as` — a position/velocity of ±∞ or ~1e38 (user-written; solver never produces one from finite input, and the far-origin probe at 1e6 px is clean) yields `min_cell = i32::MIN`, `max_cell = i32::MAX`. In `insert`, `(max - min + 1)` overflows i32 — panic in debug, wrap in release — and the build/query loops `for y in min..=max { for x in ... } }` become ~2⁶⁴ iterations: an effective hang rather than a crash. NaN is benign (casts to 0).
- **fix direction:** clamp `cell_range` output to a sane span (or assert finiteness at gather) so corrupted game-code writes fail loudly instead of freezing the frame.

## Hunt-list areas probed with no finding

Probe harness: 10 tests, preserved at scratchpad `zz_audit_probes.rs`. All passed except the S2-1 repro (which failed as reported).

- **Drift budget soundness** — sound by construction and by probe. The restage check (`step.rs:868-872`) runs *before* the substep's pair query, and `drift` accumulates the **actual** max per-substep travel measured after integration (`step.rs:1016-1026`), so solver/restitution/wake-injected motion is counted; queries never run against staleness beyond the half-cell margin, and the pair prefilter (`step.rs:894`) uses exact current positions. Restitution and `speculative_pass` velocity changes land *after* integration, so the next substep's `travel_aabb` and admission margin see them. `max_push_speed` bias (120 px/s → 0.5 px/substep) is far inside the margin. Composite probe `probe_bullet_into_thin_walled_pile_stays_contained` (15,360 px/s bullet into a settled 250-body pile in 80 px walls, 300 steps): 0 escapes. The one hole found in this area is same-substep injection vs dynamics — S2-1.
- **Warm-start map keys / id reuse** — keys carry entity generation, so despawn + same-slot respawn cannot inherit impulses; the map is rebuilt from live contacts every substep (`step.rs:1035-1041`), so stale pairs age out in one substep. Probe `probe_despawn_respawn_no_stale_warm_start_kick` (despawn a resting stack's top, respawn at the identical position into the recycled slot): no phantom kick. Cross-class aliasing impossible (bit 63 separates entity and tile keyspaces; `EMPTY_PAIR = u128::MAX` needs two distinct proxies with key `u64::MAX`, and only one entity can hold index `u32::MAX`). Residual theoreticals: S3-2, S3-3.
- **Bit-frozen sleeper invariant** — holds. Within `physics_step` nothing touches the `World` between gather and writeback; `write_back` skips sleepers (`step.rs:572-574`); `sleep_frame_end` writes (velocity zeroing) happen the frame the body sleeps, *after* its final awake writeback, and `entry.center` is stored with the exact `(center − offset) + offset` ops the next gather performs (`step.rs:727-730`), so untouched sleepers compare bit-equal every later frame. Component removal (Collider/RigidBody) drops the key from the rebuild and wakes the island; collider-offset edits change the recomputed center and wake conservatively; `Velocity` removal is invisible (reads as zero, matches frozen state). Unit pins (`external_{velocity,position}_write_wakes_island`, `settled_island_sleeps_and_freezes`) plus probe `probe_max_speed_bullet_vs_sleeping_gate` (15,360 px/s bullet vs sleeping thin gate — wakes via gap contact, no pass-through) are green.
- **Gather/writeback zip in release** — safe. Archetypes live in a `Vec` and both `archetypes_with_two{,_mut}` filter it in index order (`ecs/storage.rs:273-306`); no code path inside `physics_step` creates entities, components, or archetypes between `gather_proxies` and `write_back` (loose-body integration runs before gather; sleep writes and event drain run after writeback). The release-mode guard is the row-count `expect` (`step.rs:568-570`); the entity-equality check is debug-only but the order it asserts cannot change intra-step. Cross-frame structural changes are re-gathered.
- **Speculative margin vs direct approaches** — the four admission classes hold at all speeds (release tunneling matrix 0/32 everywhere, incl. at `substeps = 1` via probe `probe_substeps_one_no_tunneling` for static and heavy-dynamic walls). Gravity applied after contact build is covered by the `4·linear_slop` slack (g·sub_dt² ≈ 0.02 px). The uncovered case is the one-hop chain — S2-1.
- **Union-find + island tags** — min-index-root union-find is order-independent; unions accumulate across substeps; roots are always awake dynamic entity proxies (tiles/statics never union). Tag bridging retags both the map and the scratch (`step.rs:699-706`); island wakes are by tag, so merged-then-split islands only ever over-wake (safe). Despawn detection = carry-over failure at rebuild (`step.rs:616-620`), covering component removal too; the tag is a label, so despawning the tag-source member still wakes the island. Unit pins (`despawn_in_sleeping_island_wakes_the_rest`, `late_sleeper_adopts_supporting_island_for_despawn_wake`, `wake_api_wakes_whole_island_and_it_can_resleep`, `contact_wake_stays_local_to_the_disturbance`) green. Residual: S3-1 (gap-contact adoption miss).
- **Event pins** — no-event-on-gap (`penetration > 0` gate at `step.rs:978`), resting-emit-until-asleep-resume-on-wake, and once-per-frame drain in substep order (single `events` Vec appended per substep, drained at `step.rs:545-553`) all hold by code read and unit pins. Note (by design, D-066): a resting contact emits one event **per substep** — 4 per frame at defaults — accumulated and drained together; the safety-net event carries `penetration: 0.0` (a clamped impact, not a gap admission).
- **Determinism** — all iteration orders are functions of gather order (archetype `Vec` order) and staged insertion order: grid build/scatter and query candidate order, pair order, contact order, union-find, and the open-addressing maps' probe/iteration orders are all deterministic for identical input. Release `physics_determinism` (full churn + sleep transition, two runs) passed; probe `probe_determinism_with_despawn_and_wake` adds mid-run despawn + `wake()` and stays bit-identical.
- **f32 robustness** — probes green: `probe_zero_half_extent_point_body` (point AABB rests on a floor, no NaN; note zero-extent bodies are skipped by the safety net at `step.rs:1138-1140`, leaving only speculative coverage — acceptable, they are also skipped by the old substep picker), `probe_far_from_origin_stack_sleeps` (stack at 1e6 px, position ULP 0.0625 px: settles, sleeps, bit-frozen holds — the sleep-center comparison is ULP-exact by construction at any magnitude), `probe_trivial_worlds` (empty world, lone body, `sleep_threshold = 0` disables sleeping cleanly), `probe_substeps_one_no_tunneling`. `broadphase.rs:201` (`max − f32::EPSILON` boundary nudge) is a no-op for |coord| ≥ ~4 cells — benign, only claims one extra conservative cell. Residual: S3-4 (non-finite input hang).
- **Step-6 revert residue** — zero. `rg "thread|solver_threads|color|counting"` over the physics sources, benches, and tests matches only the D-067 doc comment in `step.rs`; no coloring/counting-sort/thread machinery remains in the diff.

## Substep probe log (release, `--test-threads=1`, 2026-07-13)

Both ignored probes pass; numbers match the plan's step-6/close-out records (movement is run-to-run noise):

- **10k, sleeping enabled:** fully asleep (awake 0/10,000) at ~step 210–220; steady-state `physics_step` 0.72 ms (plan: 0.74–0.77); 0 escapes.
- **10k, sleep disabled (raw jitter floor):** avg 3.5 px/s over 300 steps, per-step max 3.6–5.9 px/s at true rest (plan: byte-similar); settled-awake steady state 35.75 ms (plan: 36.1–38.1); 0 escapes. The 20 px/s default threshold sits comfortably above the floor.
- **25k, sleeping enabled (900 steps):** fully asleep (awake 0/25,000) by ~step 500; steady-state 1.90 ms — the ≤ 2 ms done-when box still holds; 0 escapes. Transient micro-pop waves (e.g. max 82 px/s at 10k step 160) reset island timers as the plan describes, then die out.

Full log: scratchpad `substep_probe.log`.

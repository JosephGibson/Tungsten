# Performance Overhead Audit — Rendering / Physics / Engine Logic

- **status:** done (audit complete; all fixes deferred to follow-up sessions)
- **goal:** Identify and measure the engine's principal performance-overhead sources in Rendering, Physics, and general engine logic (ECS, event queue, extract/scheduling, hot reload), verify each finding against captured data, and flag concrete optimizations for follow-up implementation sessions.
- **non-goals:** Implementing any optimization in this session; changing shipped pacing defaults in `tungsten.json`; reversing documented decisions (D-005 hand-rolled ECS, D-033 broadphase strategy, D-042 sprite component shape) — findings note where a decision constrains a fix.
- **files to touch:** none in this session beyond measurement infrastructure already added: `examples/02_sprite_stress/src/physics_stress.rs` (new scene), `examples/02_sprite_stress/src/main.rs` + `src/tests/main.rs` (scene wiring + tests), `scripts/perf-capture.sh` (`physics-stress` scene registration). Follow-up sessions touch the files named per finding.
- **ordered steps:** see "Follow-up implementation candidates" at the end (ordered by measured impact).
- **done-when:** every finding carries supporting numbers/traces and a verification status; follow-up list is ordered by measured impact. ✅

## Context digest

Audit session per AGENTS.md: findings only. Measurements captured 2026-07-03 on AMD Ryzen 5 6600H + Radeon 660M (RADV), Arch Linux, Vulkan, `--release`, 1920×1080, present `immediate`, `max_frame_latency=1`, 60 warm-up + 300 measured frames (canonical rules: `docs/perf/profiling-workflow.md`). A new capture scene **`physics-stress`** (3,000 dynamic circle bodies **with `Collider`s** piling under gravity in a static box + 3,000 sprites through the engine **default** extract) was added to `example-02-sprite-stress` and registered in `perf-capture.sh`, because no prior scene exercised contacts: `ecs-high-load` spawns 50k `RigidBody::dynamic()` **without colliders** (integration only — the narrow phase and solver were previously unmeasured). Method: criterion baselines (22 benches) + four full captures (telemetry, GPU timings, flamegraph, `perf stat`, `perf record`) → three parallel area-research agents → one independent adversarial verifier per finding (re-derives numbers, re-reads code, checks DECISIONS.md). 16/16 findings CONFIRMED (none refuted); verifier corrections are folded into the text below.

## Measurement sources

| Source | Location |
| --- | --- |
| physics-stress capture (new scene) | `perf-runs/20260703T183957Z-physics-stress/` |
| ecs-high-load capture | `perf-runs/20260703T184122Z-ecs-high-load/` |
| sprite-stress capture | `perf-runs/20260703T184401Z-sprite-stress/` |
| platformer capture (manual, same recipe) | `perf-runs/20260703T184406Z-platformer/` |
| Criterion baselines | table below (raw log in session scratchpad; re-run `cargo bench -p tungsten-core -p tungsten-render -p tungsten`) |
| Historical comparison | `perf-runs/20260425T*-ecs-high-load/`; April 16 present-mode matrix in `docs/perf/profiling-workflow.md` |

Additional measurement sources noted during the audit:

- Debug HUD (M18) and `SystemTimingOverlay` (M21) give per-system CPU times at runtime — the cheapest way to decompose `update` without `perf`.
- **Caveat found by verification (R6):** `gpu=` from `TUNGSTEN_GPU_TIMING=1` times **only the main scene pass** (`renderer.rs:1243-1252` attaches the sole timestamp pair there). Text overlay, present blit, and any post/SMAA/bloom passes are invisible to it. Extend the timestamp bracket before trusting `gpu=` on post-enabled captures.
- `perf-capture.sh` percentiles exclude warm-up frames; full telemetry logs include them — settling-phase spikes (see P5) only appear in the raw logs.

## Scene-level baseline (2026-07-03)

| Scene | avg total | p50 | p95 | p99 | dominant stage |
| --- | --- | --- | --- | --- | --- |
| sprite-stress (2k sprites, custom extract) | 0.99 ms | 0.88 | 2.34 | 2.79 | render_acquire 0.81 ms avg |
| platformer | 1.22 ms | 1.07 | 2.61 | 3.69 | render_acquire ~0.77 ms |
| **physics-stress** (3k bodies + contacts, default extract) | 4.88 ms | 4.87 | 4.95 | 4.99 | update ≈ 4.36–4.51 ms (**over 4 ms budget**) |
| ecs-high-load (50k entities, no colliders) | 80.46 ms | 81.43 | 84.14 | 84.83 | update 76.8 ms (**12 FPS**) |

ecs-high-load measured ~90–93 ms in the April 2026 captures — today's 80.5 ms is the standing cost of that scene, not a fresh regression. GPU scene-pass time is 0.13–1.07 ms everywhere: **every bottleneck in these captures is CPU-side or pacing.**

## Criterion baselines (median, 2026-07-03)

| Bench | Median | Note |
| --- | --- | --- |
| spawn_insert_3_components_10k | 3.688 ms | ≈369 ns/entity (incl. one String alloc/entity in bench body) |
| query_single_10k | 6.70 µs | archetype iteration |
| query2_homogeneous_10k | 6.67 µs | |
| query2_fragmented_5arch_10k | 6.81 µs | fragmentation ≈ free |
| query2_10k_5archetypes_pv | 13.46 µs | |
| spawn_despawn_1k | 70.35 µs | |
| command_buffer_flush_1k_spawns | 219.7 µs | ≈220 ns per spawn total (spawn + 2 insert migrations) |
| naive_query_single_10k | 28.18 µs | 4.2× query_single |
| **naive_query2_via_entities_10k** | **584.1 µs** | **87.6× query2_homogeneous** — the pattern engine hot loops actually use |
| event_queue_flush_10_types | 2.13 µs | |
| sprite_components_query3_2k | 678 ns | |
| position_integration_50k | 1.796 ms | naive-pattern integrate, matches E1/P4 |
| broadphase_rebuild_5k_dynamic | 301.9 µs | ~60 ns/insert, hash-dominated |
| tween_tick_5k | 405.3 µs | |
| particle_tick_5k | 527.5 µs | |
| action_map_* (5 benches) | 17–48 ns | non-issue |
| sprite_extract_batch_build_2k | 7.01 µs | 3.5 ns/item batch-build floor |
| atlas_pack_startup_200 | 7.24 µs | startup only |

---

## Findings — Rendering

### R1. `render_acquire` on light scenes is swapchain back-pressure, not reducible engine work — CONFIRMED

**Measured:** sprite-stress render_acquire avg 0.805 ms of a 0.988 ms frame (81.5%, max 4.19 ms); platformer 0.77 of 1.19 ms. On busy scenes it collapses: physics-stress 0.020 ms, ecs-high-load 0.045 ms. Flamegraph shows near-zero CPU in acquire (`drmSyncobjTimelineWait` 0.83%) — it is an off-CPU wait from Immediate/latency-1 pacing.

**Why it matters:** it is the headline number in light-scene captures and would misdirect optimization if read as render cost. p95/p99 outliers on those scenes are pacing jitter, not regressions.

**Engine-reducible part:** `Renderer::render_frame_internal` acquires the swapchain first (`renderer.rs:1097-1101`) before all encode work, though only the present-blit pass consumes the swapchain view (`passes/order.rs:137`). Reordering encode-offscreen → acquire → encode-present could overlap min(encode, wait) ≈ 0.05–0.2 ms. Caveat (verifier): the `acquire_texture() == None` skip path currently aborts before any encode; a reorder must handle wasted encode on skipped frames.

**Recommendation (follow-up):** document the back-pressure interpretation in `profiling-workflow.md`; optionally split encode around acquire. Do **not** change the Immediate/1 default (settled: `profiling-workflow.md:75-76`).

### R2. Default sprite extract costs ~75 ns/sprite (~21× the batch-build floor) — CONFIRMED (with corrections)

**Measured:** physics-stress extract stage avg **0.224 ms for 3,000 default-path sprites** (74.6 ns/sprite) vs the 3.5 ns/item isolated batch build (`sprite_extract_batch_build_2k` = 7.0 µs). Isolation rests on the telemetry stage timing (sound); the original flamegraph attribution was corrected by verification — the flat `Archetypes::get`/sip symbols in that capture are dominated by the physics step, not extract.

**Causes** (`crates/tungsten/src/sprite_extract.rs`): per-sprite String-keyed `AssetRegistry::get_sprite` (SipHash) at `:49`; per-entity `world.get::<UniformOverrideBlock>()` at `:70` even when no override exists anywhere in the world; fresh `entries` Vec + stable sort + `per_key` HashMap every frame. The `override_key` 256-single-byte-write SipHash (`:23-30`) is real in code but exercised **zero** times in these captures — code-reading finding only.

**Why it matters:** this is the default render path (D-042) every user pays. Linear scaling: ~40k default-path sprites would consume the whole 3 ms extract budget where the batch build alone needs ~0.14 ms.

**Recommendation (follow-up):** memoize asset-ID→asset resolution per frame; skip the override lookup when the world holds no `UniformOverrideBlock` storage; hash override blocks with one `write(&bytes)` call; reuse scratch buffers. Interning a handle on `Sprite` would amend D-042's component shape and needs a decision entry; the per-frame cache does not. Impact: extract 0.224 → ~0.08–0.10 ms at 3k sprites; ~10× more headroom under the extract budget (extrapolation flagged).

### R3. Tilemap extract re-resolves sprite-ID strings per visible tile per frame — CONFIRMED (impact corrected)

**Measured:** `extract_tilemaps` 6.63% self in the platformer perf record ≈ **0.04 ms/frame total** — so the fix ceiling is ~0.02–0.03 ms/frame here (verifier corrected the original 0.05 ms claim, which exceeded the whole function's cost). Cause: `assets.get_sprite(sprite_id)` inside the per-tile row/col loop (`tilemap_extract.rs:81`) plus a per-layer `HashMap<u32, SpriteBatch>` rebuilt per frame (`:69`).

**Why it matters:** pure waste on the default tilemap path, linear in visible tiles × layers — grows with zoom-out and multi-layer maps even though today's absolute cost is small.

**Recommendation (follow-up):** resolve the tileset into an indexed slice once per tilemap per frame (or cache keyed by tilemap ID, invalidated on hot reload); index by tile ID in the loop.

### R4. Text path: per-glyph work re-done every frame; missing outline API multiplies it 9× — CONFIRMED

**Measured (platformer perf record):** `lru::LruCache::get` 7.01% + `TextPipeline::prepare` 3.66% + cosmic-text `FontFallbackIter::next` 3.19% + `LayoutGlyph::physical` 2.61% ≈ 17% of process samples — the largest engine-render CPU block in that capture (~0.1–0.2 ms/frame; process denominator includes an audio thread, so the steady-state main-thread share is relatively higher). Platformer render_encode 0.215 ms vs 0.048 ms on text-free sprite-stress.

**Causes:** `TextPipeline::prepare` runs unconditionally each frame and allocates a `cosmic_text::Buffer` per section before the layout-cache check (`text.rs:178` vs `:190-193`; a small empty-buffer alloc on hit, not a re-shape — verifier). glyphon re-walks every glyph per frame. The engine has no outline/stroke support, so the platformer emits **9 near-identical sections per outlined string** (`examples/01_platformer/src/extract.rs:287-314`) — example-side cost forced by a missing engine feature.

**Recommendation (follow-up):** construct the Buffer only on cache miss; early-out `prepare` when sections are empty / unchanged; add an engine-side outline field to `TextSection` (single shaped buffer drawn with offsets) to remove the 9× multiplier.

### R5. GPU confirmed not a bottleneck; disabled post/SMAA/bloom add zero passes — CONFIRMED (metric caveat added)

Scene-pass GPU time: 0.131 / 0.287 / 0.236 / 1.066 ms (sprite-stress / platformer / physics-stress / ecs-high-load) — 1–6% of budget. With empty `PostStack` and `post_aa` off, pass order is exactly scene → text → present blit (`passes/order.rs:110-128`); M26–M29 layers keep their pay-for-what-you-use promise (D-059/D-060). **Correction from verification:** `gpu=` times only the scene pass — overlay+blit are unmeasured (bounded above by the ~1 ms CPU frame total on render-bound captures, so the conclusion stands). Follow-ups: extend timestamp coverage per-pass; capture one SMAA/bloom-enabled baseline (none exists in this artifact set).

---

## Findings — Physics

### P1. `SpatialGrid` (HashMap + SipHash, per-cell heap buckets) is the top measured cost in both heavy scenes — CONFIRMED (merged with E-area grid findings; numbers corrected)

**Measured:**
- physics-stress: `SpatialGrid::query` **26.13% self** (#2 symbol after `physics_step`), broadphase symbols total 38.0% ≈ **1.86 ms of the 4.88 ms frame**; hashing (hash_one + sip write) ≈ 9.8% ≈ 0.48 ms.
- ecs-high-load: `SpatialGrid::query` **45.67% self ≈ 36.8 ms/frame**; firmly grid-attributable hashing ≈ 5.1 ms (upper bound 9.2 ms — part of the hash_one share is ECS `TypeId` hashing, see E3); `insert` 2.43%. The hot grid there is the example's steering `SpatialGrid` (agents have no colliders), but the cost is 100% inside `tungsten_core::physics::broadphase` — the engine's only spatial-query primitive.

**Causes** (`broadphase.rs`): `cells: HashMap<IVec2, Vec<ProxyId>>` with default SipHash (`:16`, `:33`); insertion into every overlapped cell (`:56-63`); one HashMap probe per cell per query (`:74`); per-candidate `query_marks` dedupe with random access into a 200 KB array (`:79-84`, `:117-123`); each cell's bucket is a separate heap allocation (dependent cache miss per cell visit); `clear()` via `HashMap::clear` drops every bucket Vec.

**Recommendations (follow-up, one coordinated change):**
(a) replace SipHash with a hand-rolled multiplicative IVec2 hasher (no new dep — D-015 untriggered) or a dense/flat layout (counting-sort proxies into one contiguous Vec with per-cell ranges);
(b) make `clear()` retain bucket capacity;
(c) insertion policy: small proxies by center point into one cell + query expansion — **hybrid only** (verifier: naive center-point insertion breaks on physics-stress's large wall AABBs; oversized proxies need multi-cell or a separate list);
(d) skip `query_marks` dedupe when no inserted proxy straddles cells (track a straddle flag at insert).
D-033 fixes "uniform grid rebuilt per substep", not the hasher or layout — compatible.

**Impact:** physics-stress ~0.8–1.0 ms of the 4.4 ms update; ecs-high-load ~6–16 ms/frame (hashing share firm, layout share speculative until a query bench exists — add one next to `broadphase_rebuild_5k_dynamic`).

### P2. No sleeping: a fully settled pile pays the entire physics pipeline every frame — CONFIRMED

**Measured:** physics-stress measured-window update avg **4.51 ms** for a pile at rest (mostly 1 substep); physics symbols ≥ 83% of all cycles (46.34% `physics_step` self + 37.2% broadphase; verifier corrected the original "~87%"). `step.rs:99-102` unconditionally integrates, rebuilds, pair-queries, and runs 4 Gauss–Seidel iterations for all 3,000 bodies; no rest state exists anywhere in `physics/`.

**Why it matters:** dense persistent piles are the canonical 2D workload; cost scales with total body count instead of active count, and the 4 ms update budget is already breached at 3k bodies.

**Recommendation (follow-up):** per-body sleep (velocity below threshold N consecutive frames → skip integration, insert as static-like; wake on contact from an awake body or impulse). Islands can come later. Verifier: the "persistent static proxy set" variant revises D-033(2) and needs a decision amendment; plain per-body sleep does not. Impact ceiling is the full measured physics share (steady-state update plausibly < 1 ms); magnitude is standard-technique extrapolation, not measured.

### P3. Fixed half-cell pair margin (16 px) + all pairs re-tested in all 4 solver iterations — CONFIRMED (attribution corrected)

**Measured:** solver ≈ 1.6 ms (36%) of the physics-stress update: inlined `circle_vs_circle` lines ~40% of `physics_step` self, impulse math ~16%, loop+events ~14% (perf annotate). `step.rs:249` pads every dynamic proxy's swept AABB by `cell_size*0.5 = 16 px` regardless of actual travel — a resting radius-6 body queries a 44×44 px box against 12 px neighbors (~2–4× candidate over-fetch, geometry estimate); `:269-324` re-runs narrow phase on the full pair list in each of 4 iterations.

**Correction (verifier):** surplus pairs exit at the `dist_sq` early-out (`collision.rs:132`), so they don't drive the hot normalize/impulse lines; realistic solver-side saving is ~0.2–0.5 ms, with the remainder of the combined ~0.5–1.0 ms coming from the broadphase query-area reduction.

**Recommendation (follow-up):** motion-scaled margin (`|v|·sub_dt` + 1–2 px slop — still covers tunneling and GS slip per the `:248` comment); contact caching after iteration 0 so iterations 1–3 skip never-touching pairs.

### P4. Physics gathers via the 87.6×-slow naive ECS pattern; ~1.8 ms/frame at 50k bodies with zero colliders — CONFIRMED

**Measured:** ecs-high-load `physics_step` self ≈ 1.9 ms/frame doing pure integration for 50k collider-less bodies (single substep there — verifier correction: the per-substep multiplier applies only to physics-stress). `position_integration_50k` = 1.80 ms reproduces the pattern in isolation; archetype-iterated equivalent extrapolates to ~50–150 µs.

**Causes:** `compute_substeps` (`step.rs:110-126`), `apply_gravity_and_integrate` (`:141-161`, re-collected per substep), and the collider gather (`:192-237`) all use `query_entities`/`query` + per-entity `get`/`get_mut` — because `World` has no mutable multi-component iterator (see E1, the root finding).

**Recommendation (follow-up, depends on E1):** rewrite the three gather sites as archetype iterations; hoist dynamic/collider list collection out of the substep loop. Impact: ~1.7–1.9 ms on ecs-high-load; ~0.15 ms directly measured on physics-stress (upper estimates speculative — perf record has no call graph).

### P5. Per-substep full re-gather + grid rebuild multiplies fixed costs on high-velocity frames — CONFIRMED (scope corrected)

**Measured:** settling-phase frames reached 2 substeps (never the 8 cap) with update spikes to 7.03 ms vs ~3.6 ms contemporaneous 1-substep frames (~2× ratio). **Verifier correction:** all spike frames fell inside the 60-frame warm-up window, so this capture's official measured percentiles do not breach budget; the mechanism still matters for real gameplay where high-velocity frames occur mid-play. `step.rs:99-102` + module doc `:3`: proxies, grid, speculative pass, and pair query rebuilt from scratch per substep; tile proxies (`:482-551`) are static yet regenerated per substep.

**Recommendation (follow-up; requires a DECISIONS.md superseding entry — D-033 made rebuild-per-substep intentional):** gather ECS data into proxies once per `physics_step`; between substeps update centers/velocities only, and reuse cells (with slightly larger margin) when max per-substep displacement is below a cell fraction. Impact: removes ~0.6–1.4 ms per extra substep at 3k bodies (7 ms spikes → ~5.7–6.4 ms, not ~5 ms as first estimated; pair query/speculative/narrow-phase must still rerun).

---

## Findings — Engine logic (ECS, events, scheduling, hot reload)

### E1. No mutable multi-component query — the root API tax forcing the 87.6×-slow pattern engine-wide — CONFIRMED (numbers corrected)

**Measured:** `naive_query2_via_entities_10k` 584.1 µs vs `query2_homogeneous_10k` 6.7 µs (**87.6×**). ecs-high-load (update avg **76.8 ms**): directly measured lookup-path symbols ≈ 9.4% of frame ≈ **7.3 ms** (`Archetypes::get`+`get_mut` 4.87% self, `TypedVec::*_erased` ~1.6%, TypeId `hash_one` 2.85%), plus an un-isolatable inlined share inside the four example systems (23.8% combined self). physics-stress: ~0.15 ms directly measured.

**Cause:** `World` exposes only immutable `query`/`query2`/`query3` (`world.rs:73-168`); every mutating system must `queryN_entities` (fresh `Vec<Entity>` per call — ~2.4 MB/frame at 50k) then per-entity `get`/`get_mut` (~29 ns vs ~0.7 ns/row columnar). Engine call sites baked in: physics integrate/gather (P4), `sync_position_to_transform` (`tungsten-core/src/components.rs:209-220`), `tweens.rs`, `particles.rs` — plus all four ecs-high-load example systems.

**Recommendation (follow-up, highest-leverage single change):** add `query_mut`/`query2_mut`/`query3_mut` yielding `(Entity, &mut A, &B…)` via per-archetype split column borrows (distinct TypeIds ⇒ disjoint columns); migrate engine call sites, then examples. Extends D-036's archetypal design; D-005-compliant. Impact: **~6–7.5 ms/frame directly measured** on ecs-high-load (10 ms upper bound explicitly speculative); ~0.15–0.3 ms on physics-stress toward the 4 ms budget.

### E2. SpatialGrid SipHash — CONFIRMED (verified by orchestrator; merged into P1)

The delegated verifier for this finding died (session limit); verification was redone in the main session: code claim reproduces (`broadphase.rs:16,33` default-hasher HashMap), perf shares reproduce (ecs-high-load hash_one 7.6% + sip write 3.8% ≈ 11.4% ≈ 9.2 ms upper bound, ~5.1 ms firmly grid-attributable after excluding TypeId hashing; physics-stress ≈ 10% ≈ 0.48 ms). Consolidated with P1 to avoid double-counting.

### E3. Archetype column lookup is `HashMap<TypeId, Box<dyn AnyColumn>>` + SipHash on every access — CONFIRMED (impact re-scoped)

**Measured:** ecs-high-load TypeId-map cost up to ~5 ms/frame measurable (verifier: original 2–3 ms was understated); physics-stress share is only ~0.1 ms (original flamegraph attribution corrected — the prominent hashing there is IVec2 grid keys, not TypeId). **Most of this cost sits inside the E1 naive pattern and disappears when E1 lands**; residual standalone value (random-access gets, insert transitions, query setup) is real but likely well under 1 ms/frame post-E1.

**Recommendation (follow-up, after E1):** replace `Archetype.columns` with a sorted `Vec<(TypeId, Box<dyn AnyColumn>)>` + positional index (component_types is already a sorted slice), or cache `(ArchetypeId, TypeId) → column index`. Keeps D-036.

### E4. Spawn/insert does one archetype migration per component with Box round-trips: 369 ns/entity — CONFIRMED (strengthened by verification)

**Measured:** `spawn_insert_3_components_10k` 369 ns/entity (includes one bench-side String alloc — pure overhead slightly lower); `command_buffer_flush_1k_spawns` 220 ns/spawn total. ecs-high-load's 50k×7-component seed shows `move_components_to` ≈ **26 ms one-time** in perf data. Causes: N migrations moving 0+1+…+(N−1) components per N-component spawn; `swap_remove_erased` boxes each moved value only for `push_erased` to unbox it (`archetype.rs:32-34`, `:125-153`); `Box<dyn ComponentSetter>` per deferred insert (`command_buffer.rs:73-86`).

**Why it matters (more than first thought):** verifier correction — D-051 particles are **entity-per-particle with no pool**, spawning/despawning through CommandBuffer every frame, so bursts exercise exactly this path at runtime, not just at scene load (D-046).

**Recommendation (follow-up):** bundle-spawn API (`spawn_with((A,B,C))` — one archetype resolution, direct typed pushes); typed column-to-column row move to kill the Box round-trip. Impact: plausibly 3–5× on spawn-heavy paths; startup/transition latency, not steady-state frame budget.

### E5. App loop, event queue, hot reload: audited clean — CONFIRMED

Inter-stage plumbing residual ≈ 0.001 ms/frame (sprite-stress: total 0.988 − update 0.0124 − extract 0.0443 − render 0.931); `hot_reload` and `flush` report 0.00 ms in all four captures; `event_queue_flush_10_types` 2.1 µs with an allocation-free empty path (`event_queue.rs:48-51`); watcher drain early-outs when empty (`hot_reload.rs:110-135`). Micro-hygiene only: per-frame system-name String clones (`app.rs:685`), `env::var("TUNGSTEN_PERF_LOG")` re-read per frame (`app.rs:1404`). Gating `system_timings` on HUD activity would touch D-038/D-044 — record as amendment if done. **No optimization effort warranted here.**

---

## Follow-up implementation candidates (ordered)

| # | Change | Area | Verified impact (this hardware) | Constraints |
| --- | --- | --- | --- | --- |
| 1 | ✅ **IMPLEMENTED 2026-07-06** — Mutable multi-component ECS queries + migrate engine call sites (E1, P4) | core ECS | **Measured delta:** ecs-high-load avg total 80.46 → **63.08 ms** (−17.4 ms; update 76.8 → 62.04 ms); physics-stress avg total 4.88 → **4.41 ms** (update 4.36–4.51 → 4.02 ms, −0.34–0.49 ms). `query2_mut_10k` = **3.44 µs** vs `query2_homogeneous_10k` 6.69 µs and `naive_query2_via_entities_10k` 591 µs (~172×). No scene regressed: sprite-stress p50 0.88 (=baseline), platformer p50 1.07 (=baseline) | extends D-036; assert disjoint TypeIds |
| 2 | SpatialGrid overhaul: integer hasher or flat layout, capacity-retaining clear, hybrid insertion, straddle-aware dedupe (P1/E2) | physics/broadphase | ~0.8–1.0 ms physics-stress; ~6–16 ms ecs-high-load | hybrid insertion required for large static AABBs; add a grid-query bench first |
| 3 | Per-body sleeping with contact wake (P2) | physics | steady-state pile update → plausibly <1 ms (ceiling = full 4.4 ms physics share) | persistent-static-set variant amends D-033(2) |
| 4 | Motion-scaled pair margin + contact caching across solver iterations (P3) | physics | ~0.5–1.0 ms combined on physics-stress | keep tunneling coverage per step.rs:248 |
| 5 | Once-per-step proxy gather; substep-local center updates (P5) | physics | ~0.6–1.4 ms per extra substep (spike mitigation) | needs DECISIONS.md superseding entry for D-033 |
| 6 | Default-extract caching: asset-ID memoization, conditional override lookup, bulk hash, scratch reuse (R2) | render path | 0.224 → ~0.09 ms at 3k sprites; ~10× sprite headroom | Sprite-handle interning would amend D-042 |
| 7 | Text: Buffer-on-miss, empty/unchanged early-out, engine outline API (R4) | render path | ~0.1–0.2 ms/frame platformer; 9× section reduction for outlined text | none |
| 8 | Archetype column positional index (E3) | core ECS | up to ~5 ms ecs-high-load pre-E1; <1 ms residual post-E1 | do after #1, re-measure |
| 9 | Bundle spawn + typed row moves (E4) | core ECS | 3–5× spawn paths; 50k seed ~26 ms → single-digit ms | matters for D-051 particle bursts |
| 10 | Tilemap tileset resolution hoist (R3) | render path | ~0.02–0.03 ms platformer; linear in visible tiles | none |
| 11 | Acquire/encode overlap + back-pressure documentation (R1) | render path | ≤0.05–0.2 ms light scenes only | handle acquire-None skip path; keep Immediate/1 default |
| 12 | Per-pass GPU timestamp coverage; one SMAA/bloom-enabled baseline capture (R5) | tooling | measurement fidelity only | none |

Also flag for a docs session: register the `physics-stress` scene in `docs/perf/profiling-workflow.md` (capture rules table + quick-start examples) so the canonical-scene contract covers it.

### Item #1 implementation record (2026-07-06)

- New captures (same recipe as the 2026-07-03 baselines): `perf-runs/20260706T144242Z-ecs-high-load/`, `perf-runs/20260706T144635Z-physics-stress/`, `perf-runs/20260706T144710Z-sprite-stress/`; platformer re-measured manually (same recipe, not script-registered).
- API shape note: `query_mut`/`query2_mut`/`query3_mut` yield **all-mutable** refs (`(Entity, &mut A, &mut B[, &mut C])`) rather than mut-first/shared-rest — the migrated call sites (confine: `&mut Position` + `&mut Velocity`; integrate: `&mut Velocity` + `&mut Position` + `&RigidBody`) need double-mut shapes. Split column borrows per archetype; distinct `TypeId`s asserted. Still an extension of D-036; no DECISIONS.md entry needed.
- `PhysicsBuffers` lost its `collider_entities`/`dynamic_entities` scratch lists — columnar gathers replace list collection entirely (subsumes the "hoist out of substep loop" step).
- **Items #2 and #8 must remeasure against these new baselines** (ecs-high-load 63.08 ms avg / update 62.04 ms; physics-stress 4.41 ms avg / update 4.02 ms), not the 2026-07-03 numbers. Caveat: the 2026-07-06 ecs-high-load flamegraph has degraded symbolization (large unresolved `[libm]`/`[libc]` frames: libm ≈ 43.9%, `__atan2f` 10.9%, `__libc_realloc` 15.6%) — steering trig, grid bucket growth, and hashing plausibly dominate the remaining update, but item #2 should re-capture with resolved symbols before attributing.

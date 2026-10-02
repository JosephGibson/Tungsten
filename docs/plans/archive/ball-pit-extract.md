# Platformer ball pit: faster sprite extract

- **status:** done
- **goal:** Cut the per-ball cost of the ball section of `extract_sprites` in `examples/01_platformer` at up to `BALL_CAP` (12,000) balls, with the same batches, instances and draw order for every ball on screen.
- **non-goals:** Engine or render crate changes (so no `DECISIONS.md` entry); a public engine helper for custom extracts; interned sprite IDs; culling in the default engine extract; GPU-side work (opaque pass, overdraw); any other section of `extract_sprites` except the flames `format!`.
- **files to touch:** `examples/01_platformer/src/extract.rs`; `examples/01_platformer/src/tests/main.rs` (one culling assertion); this plan. A scratch reference/bench file under `src/tests/` exists only during the session.
- **ordered steps:** 1 baseline; 2 one columnar query; 3 batch-key and sprite memo; 4 capacity reserve (tried, dropped); 5 colour; 6 culling; 7 flames IDs; 8 checks, measurement, hand-off.
- **done-when:** Existing platformer tests pass; ball batches equal the old loop's on a mixed set; `just check` passes; before/after timings reported with their caveats, or the reason they are missing; status updated.

Date: 2026-10-02. Branch `0.38`, base `45ed68b`.

## Context digest

- The platformer installs its own extract (`app.set_extract_sprites(extract_sprites)`, `setup.rs`) because it needs its own layer order, so the default extract's fast path (`D-086`, `crates/tungsten/src/sprite_extract.rs`) does not run. That machinery (`AssetLookup`, `ExtractScratch`, `InstancePool`) is `pub(crate)`.
- Old ball loop (`extract.rs`, `ball_batches`): `world.query::<Ball>()`, then per ball five `world.get` calls, one or two `assets.get_sprite(&str)` string hashes, one `HashMap` entry on a 3-tuple, up to three `powf(0.55)`, and a push into a `Vec` grown from empty.
- A ball needs `Ball` + `Position`. `SmallBall`, `CurrentSprite`, `BallBurn` and `BallHue` are optional: tests spawn balls without `CurrentSprite`, and the loop falls back to the start sprite of the ball's size. Spawned balls use 12 sprite IDs (6 spin frames × 2 sizes).
- `query2_opt2::<A, B, C, D>` documents that it iterates the same archetypes in the same order as `query2::<A, B>`, and physics already zips two such queries. Two `query2_opt2::<Ball, Position, _, _>` zipped give all six columns with no per-entity lookup, in the old loop's order (archetype order, rows in order, balls without `Position` skipped).
- Batch order: the old code sorts `HashMap::into_values()` by texture ID. Two ball batches on one texture (same atlas, different filter or lit flag) therefore came out in an arbitrary order per frame. The new code keeps first-seen order for such ties; everything else is unchanged.
- Camera: `platformer_camera_base_zoom` shows `CAMERA_ROWS` = 18 tile rows (about 32 × 18 tiles at 16:9). The pit is 48 × 28 tiles (`tests/ball_pit.rs`: columns 132–180, rows 18–46), so the camera never shows more than about 43% of it. Culling applies. `CameraState.position` already includes the shake offset and `visible_world_aabb` is conservative under rotation; `extract_props` culls against the same bounds.
- Extracted batches are moved into the renderer and recycled into the engine's private pool, so the example never gets its instance `Vec` back. A scratch `Vec<SpriteInstance>` kept in a resource would add a copy of up to 576 KB per frame and save no allocation.

## Steps

1. **Baseline.** Scratch file `src/tests/ball_extract_ref.rs`: the old loop verbatim (with its own copy of the old `rainbow_rgba`) as the reference, an equality test on a mixed set (small/normal, burning, burnt out, hued, no `CurrentSprite`, unknown sprite ID, no `Position`), and an ignored release timing test for 2,000 and 12,000 balls that interleaves reference and new code in one binary. Not part of the patch series.
   - Measurement status: `nxcodec.bin` was running for the whole session (a connected NoMachine client), so no clean capture exists. Timings taken are indicative only and labelled so. The timing test is handed over as a separate optional patch for a clean rerun.
2. **One query.** Move the ball section into `extract_balls` and replace `query::<Ball>` + five `get`s with the two zipped `query2_opt2` queries.
3. **Memoise.** Replace the `HashMap` with a short `Vec<SpriteBatch>` plus the index of the last batch used. Resolve sprite IDs through a small direct-mapped cache (slot from ID length and last byte, verified by comparing the ID, registry on a miss); the fallback ID goes through the same cache.
4. **Reserve.** Adjusted from the brief: no scratch buffer (see digest). Count the balls once and give each new batch room for the balls not yet emitted.
   - Outcome: tried and dropped. It read 453.9 µs against 447.8 µs without it at 12,000 balls, so the counting pass bought nothing.
5. **Colour.** `rainbow_rgba` raises `1.0`, `0.0` and one fractional channel to 0.55. `powf` returns exactly 1 and 0 for the first two, so only the fractional channel needs it: one `powf` per hued ball, bytes identical. Check against the old function over a dense hue sweep. No approximation, so no owner sign-off is needed.
6. **Culling.** Skip a ball whose quad lies wholly outside `view_bounds`. Equality with the reference then holds for the on-screen subset; assert both that and the full-set equality under a camera that sees everything. Add one assertion to the existing ball test in `tests/main.rs` that an off-screen ball is not extracted.
7. **Flames.** Replace the eight per-frame `format!("ex10_fire_{i}")` with a `const` array of IDs.
8. **Checks and hand-off.** Delete the scratch file, `just check`, `just smoke` if a display is available (no wiring changes are planned), set `status`, move this plan to the archive, cut ordered patches and the index-only commit script.

## Done-when checks

- [x] `cargo test -p example-01-platformer` passes (70 tests), including `tests/ball_pit.rs` and the extract tests in `tests/main.rs`.
- [x] Ball batches equal the reference loop's, bit for bit, on the mixed set in five atlas/lighting configurations: the full set under an all-seeing camera, the on-screen subset under a partial view. Colours equal the old function's over 2.6 million hues (dense in [0, 1), sextant edges, out-of-range, NaN and infinity). Both tests passed in debug and release builds.
- [x] `just check` passes. The platformer also ran its three smoke frames on the GPU; the full `just smoke` was not run because no wiring changed.
- [ ] Timings on a quiet machine. Not met: see Results.
- [x] Reference loop and scratch test deleted; plan status updated.

## Results

Indicative only. `nxcodec.bin` was running during every timing run and a second agent session was open, so absolute values are inflated by an unknown amount. Each figure is the median of 801 calls of `extract_sprites` on a balls-only world (five small marbles per orb, one small ball in twenty burning, 232 registered sprites), release build with `target-cpu=native`, with the old loop timed alternately in the same binary.

| State | 2,000 balls, all in view | 12,000 balls, all in view | 2,000 balls, game view | 12,000 balls, game view |
| --- | --- | --- | --- | --- |
| Before (`45ed68b`) | 204 µs | 1,218 µs | 205 µs | 1,233 µs |
| Step 2, one query | 167 µs | 987 µs | 165 µs | 994 µs |
| Step 3, memo | 76 µs | 448 µs | 76 µs | 454 µs |
| Step 5, colour | 60 µs | 354 µs | 61 µs | 358 µs |
| Step 6, culling | 62 µs | 359 µs | 48 µs | 247 µs |
| Step 7, final | 61 µs | 355 µs | 47 µs | 249 µs |

- "Game view" is the 1080p camera resting on the pit floor. It extracts 1,356 of 2,000 and 6,911 of 12,000 balls.
- The brief's estimate of 0.3–0.6 µs per ball (4–7 ms at the cap) was three to six times too high: the old loop cost about 0.10 µs per ball, 1.2 ms at the cap. The new loop costs about 0.03 µs per ball.
- The old loop copied into the test binary read about 5% faster than the real one at `45ed68b` (1,162 µs against 1,218 µs), so the "before" row uses the real one.
- Step 7 does not show: the bench registers no flame sprites, and eight short `format!` calls per frame are below its resolution.

## Follow-ups (not built)

- Rerun the timing on a quiet machine (`nxcodec.bin` absent, one session). The scratch reference and timing test are kept outside the series as an optional patch.

- A public engine helper for custom extracts (`AssetLookup`, the instance pool); interned sprite IDs; extract culling in the default engine extract (the last two are open proposals in `docs/perf/benchmarks.md`).
- `assets.get_sprite("ex10_flame_glow")` is looked up once per burning ball inside the flames loop.
- The particle section of `extract_sprites` has the same shape as the old ball loop (`HashMap` entry and a `get::<Visibility>` per particle).

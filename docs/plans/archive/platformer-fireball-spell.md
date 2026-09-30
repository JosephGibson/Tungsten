# Platformer fireball spell and black-hole polish

status: done
goal: in `examples/01_platformer/`, add a Mouse 4 fireball spell that explodes on contact and ignites small balls; make black holes put out burning small balls and leave them spent; redraw the black hole and enrich its particles; add synthesized sound effects for these events.
non-goals: library crates beyond the approved symphonia `pcm` amendment (stop and amend this plan if another change is needed); the repo-root `input.json`; making large balls or anything else combustible; fireball enemy behavior; burn duration and burning-ball flame visuals (`platformer-fire-intensity.md`); `src/burning.rs`, `src/tests/burning.rs`, `src/tests/ball_pit.rs`; replacing `black_hole.ogg`; other examples.
files to touch: `examples/01_platformer/src/` `fireball.rs` (new), `main.rs`, `state.rs`, `systems.rs`, `gameplay.rs`, `setup.rs`, `extract.rs`, `tests/spells.rs` (new), `tests/main.rs`; `examples/01_platformer/tools/` `sfx.py` (new), `effects.py`, `polish_art.py`, `art.py` (size exemption), `generate.py`, `sounds.json`, `README.md`; regenerated `assets/manifest.json`, `assets/sprites/`, `assets/particles/`, `assets/sounds/*.wav` and `tools/outputs.json`; `docs/LLM_INDEX.md` row 62; amendment A: workspace `Cargo.toml`, `Cargo.lock`, `crates/tungsten-core/tests/audio_decode.rs`, `DECISIONS.md` (D-077), `docs/DECISION_INDEX.md`.
ordered steps: 0 baseline and before capture (done); 1 sound synthesis; 2 black-hole art and particles; 3 spell and extinguish particle configs; 4 fireball spell; 5 black-hole extinguishing and dust layer; 6 extraction; 7 docs; 8 visual review and after capture; 9 final checks.
done-when: tests in `src/tests/spells.rs` show that Mouse 4 spawns a projectile moving toward the cursor, that projectile contact explodes and ignites a small ball, and that a black hole leaves burning balls inside its radius spent; `python3 examples/01_platformer/tools/generate.py --check`, `python3 -B -m unittest discover -s examples/01_platformer/tools -p 'test_*.py'`, `just check`, `just repo-check` and `just smoke` pass; before and after black-hole captures are in the scratchpad, not under `assets/`.

## Context digest

- **Baseline (2026-09-29, current tree):** `--check` OK (520 outputs); 15 Python tests and 66 Rust tests pass.
- **Input:** winit `Back` maps to `MouseButton::Other(4)`, serialized as `"button4"`. `platformer_bindings` is the first entry of `RUNTIME_SYSTEM_ORDER` and reapplies example-local actions every frame. That is how MMB `spawn_small_ball` works without an `input.json` entry.
- **Black hole:** pressing RMB spawns a `BlackHole` with `Position`, `Transform` and the `ex10_black_hole` emitter. It follows the cursor while held and is despawned on release. `gameplay::scene_effects` re-seats newborn particles owned by a `BlackHole` on a 135–175 px annulus, then steers them along a spiral. `tests/ball_pit.rs` pins that annulus through the config's radial speed of 100–260.
- **Draw order:** obstacles, engine particles, `extract_vortices` (halo, two vortex layers, core, 32 procedural sparks), player, balls, burning flames, foreground, hearts, cursor. The vortex layers currently cover the particles. `ex10_halo` (lamp, fire, lantern and moon glows) and `ex10_spark` (six particle configs, including burning embers) are shared, so they stay unchanged.
- **Fire:** `ignite()` inserts `BallBurn { remaining: BALL_BURN_SECONDS }` only on a `SmallBall` that has no `BallBurn`. `remaining == 0` means spent. Spent balls get no flames or emitters and render charred.
- **Physics:** there are no raycasts, sensors or kinematic bodies. Contact events fire only at `penetration > 0` (D-064). A tile is solid where its collision-layer index is `>= 0`. Slab decks and moving platforms are static `Collider` entities.
- **Particles:** one emitter per entity, and particles outlive their emitter. Emitters have no rotation. The global cap is 2,048; `spawn_transient_effect` caps transient emitters at 16.
- **Audio:** `AudioCommands::play_with(handle, volume, looping)`. The mixer resamples and upmixes, so mono 16-bit WAV works. `generate.py` copies `sounds.json` into the manifest and exempts `assets/sounds/black_hole.ogg` by name in its stray-file check.
- **Captures:** Xvfb and Xephyr lack DRI3 here, so the renderer cannot start in them. Captures run on the real display from a scratch copy of the tree (step 0).

## Coordination with active plans

Both plans' edits stay as they are. Every file this plan shares with them:

| File | `platformer-polish-pass.md` | `platformer-fire-intensity.md` | This plan's edit |
| --- | --- | --- | --- |
| `src/extract.rs` | step 15 splits it | owns the burning-flame block | rewrite `extract_vortices`; add an under-particles call and `extract_fireballs`; hint text. The flame block is not edited |
| `src/systems.rs` | step 16 splits it | — | `black_hole_extinguish_system`, `play_effect_sound`, dust child in `spawn_black_hole_system`, orphan-anchor cleanup |
| `src/gameplay.rs` | step 16 | — | `scene_effects` steering also covers an anchored child of a black hole |
| `src/setup.rs` | step 17 | — | binding, system order, sound lookup |
| `src/state.rs`, `src/main.rs` | steps 16–18 | — | `EffectSounds`, extinguish constants; `mod fireball;` |
| `src/tests/main.rs` | step 14 moves tests | — | `mod spells;`, runtime-order expectation |
| `tools/generate.py` | wiring | owns `ball_burn*` configs | new configs, `black_hole` retune, sound outputs and ownership check. `fire_trail`, `fireball_drips` and `ball_burn*` stay byte-identical |
| `tools/effects.py`, `tools/polish_art.py` | step 2 target module | owns `fire_*` frames in `polish_art.py` | move `vortex`/`vortex_core` out of `polish_art.py` into `effects.py`; `fire_*`, hearts and `shock_ring` untouched |
| `tools/art.py` | step 2 structure | — | 96×96 exemption for the black-hole sprites |
| `tools/README.md` | step 19 | authoring notes | new section; module table; sound and binding sentences |
| `assets/manifest.json`, `assets/particles/`, `tools/outputs.json` | regenerated | regenerated | regenerated |
| `docs/LLM_INDEX.md` row 62 | step 19 | — | add `src/fireball.rs` |

- **Polish-pass done-when:** its sorted test-name comparison must also allow this plan's tests. Rust: `mouse4_casts_a_fireball_toward_the_cursor`, `fireball_contact_explodes_and_ignites_small_balls`, `black_hole_leaves_burning_balls_inside_its_radius_spent`. No Python tests are added.
- **If polish steps 14–16 land first,** put new code in the split modules: `black_hole_extinguish_system` in `black_hole.rs`, `play_effect_sound` beside `play_sfx`, the extract functions in `extract/actors.rs`. `fireball.rs` stays its own module either way.
- **Fire-intensity:** its done-when checks are unaffected. Extinguishing only writes `remaining = 0`, which that plan's flame and emitter code already treats as burnt out.

## Decisions (owner, 2026-09-29)

- **D1: fired from the player; missile physics in example code.** The missile starts at the player and carries its own velocity. It integrates low gravity (`FIREBALL_GRAVITY`, a small fraction of `GRAVITY_Y`) plus black-hole pull (D7), and it points along its velocity. It explodes on contact with any solid object: any `Collider` entity except the player (balls, slab decks, moving platforms) or a solid collision tile. It is not an engine physics body; the reasons are in the history below.
- **D2: Mouse 4 is bound in `platformer_bindings` only** for now; `input.json` is unchanged.
- **D3: a separate `EffectSounds` resource** instead of new `AudioState` fields.
- **D4: the black-hole particle budget stays at 1,200:** `black_hole` 900 plus `black_hole_dust` 300. The `black_hole` radial speed stays 100–260.
- **D5: black-hole sprites are slightly larger, 96×96,** and exempt by name from the 64×64 check in `validate_art`. Every layer fades to transparent at its outer edge through dithered alpha. Soft layers use `linear` filtering. On-screen sizes are unchanged.
- **D6: all burning balls in the radius go out in the same frame.** At most 4 `extinguish` bursts per frame, sampled evenly, and at most one sizzle per 0.15 s.
- **D7: black holes pull fireballs** with the same force and falloff as dynamic bodies, through a shared acceleration helper. A missile inside a hole keeps burning until it hits something or expires; balls a blast ignites inside a hole's radius go out that frame.
- **Testing is deprioritized** (the example will be reworked). Only the three done-when Rust tests are written, and they stay lean. The planned layering test and the Python sound test are dropped; `--check` still covers the WAV bytes.

History: a physics body was rejected because the engine has one global gravity, contact pushes the player and balls, and contact events fire only at `penetration > 0` (D-064), so a speculative stop at a wall might never report.

## Amendment: PCM WAV decode (2026-09-29, owner chose A)

The workspace enables symphonia's `wav` demuxer but not its `pcm` codec, so the engine rejects PCM WAV with "unsupported audio codec" and startup fails. `crates/tungsten-core/tests/audio_decode.rs` pins this as a known gap (`wav_pcm_is_rejected_without_the_pcm_codec_feature`). Every fix except option C is outside this plan's scope:

- **A (recommended): enable symphonia's `pcm` feature** in the workspace `Cargo.toml`. Also replace the pinned rejection test with the `check_tone("tone_44100_stereo.wav", 44_100, 2, 0)` check that its comment names, and update that test module's docs. Add a `DECISIONS.md` entry extending D-028's features (D-015 rule 2), with its `docs/DECISION_INDEX.md` row. Then run `just deps`.
- **B: ship OGG.** `generate.py` transcodes the synthesized PCM through `ffmpeg`/libvorbis. This adds an external tool to generation, and `--check` bytes then depend on the encoder version.
- **C: drop the sounds** from this plan.

The owner chose A: symphonia `pcm` is enabled, `wav_pcm_stereo_decodes` replaces the pinned rejection test, and D-077 plus its index row record the change. The generator, `sounds.json` and the Rust wiring are unchanged.

## Progress (2026-09-29)

- **Done:** steps 1–7 and the after capture.
  - `--check` OK (530 outputs); 15 Python tests; 69 Rust tests; `just check`, `just repo-check` and `just ctx` pass.
- **Regeneration side effect:** `level.tmj` GIDs shift because the tileset lists every sprite alphabetically. The resolved tile layers are identical and `level_layout.rs` is unchanged.
- **Tuned during review:**
  - Black-hole tints: halo alpha 150, disk at `D * 1.6`, cooler arms.
  - Dust made dark (indigo at about 0.5 alpha) and smaller (scale 0.16–0.36).
- **Captures** (scratchpad `captures/`):
  - `before_black_hole*.png`, `after_black_hole*.png`;
  - `black_hole_before_after.png`, `black_hole_before_after_zoom.png`;
  - `spell_flight.png` (a missile two frames after the cast);
  - `black_hole_sprites.png` (the sprites at 1× and 4×).
  - `capture_tree.sh` now also takes `FRAME`.
- **Final (2026-09-29):**
  - Checks: `--check` OK (530 outputs, up from 520); 15 Python tests; 69 platformer Rust tests (66 + 3); `wav_pcm_stereo_decodes` replaces the pinned rejection test. `just check`, `just repo-check`, `just smoke` (4/4) and `just deps` pass.
  - New IDs:
    - sprites `ex10_accretion_disk`, `ex10_photon_ring`, `ex10_infall_streak`, with `ex10_vortex` and `ex10_vortex_core` redrawn;
    - particles `ex10_black_hole_dust`, `ex10_spell_trail`, `ex10_fireball_blast`, `ex10_extinguish`, with `ex10_black_hole` retuned;
    - sounds `ex10_fireball_cast_sfx`, `ex10_fireball_blast_sfx`, `ex10_extinguish_sfx`.
  - Other plans:
    - The polish-pass test-name comparison must allow this plan's 3 Rust tests.
    - Fire-intensity checks are unaffected.
    - `debug-cleanup-docs-pass.md` still calls PCM WAV a known issue; D-077 resolves it.
  - Not verified by play: the missile feel inside a black hole, and extinguish puffs over a burning pile.

## Steps

### 0. Baseline and before capture (done during planning)

- Baseline as in the digest.
- **Scratchpad:** `/tmp/claude-1000/-home-joker-Projects-Tungsten/c3d3bb4d-45c0-41d6-9511-b86fea7642ab/scratchpad/`.
- **Script:** `capture_tree.sh <label> [x,y] [button]`. It tars the tree (excluding `target`, `.git`, `perf-runs`) into `<label>_tree/`. It applies `capture_hook.py` to the copy only: when `BH_CAPTURE_HOLD=x,y` is set and `SceneTime` passes 0.3 s, `platformer_bindings` sets the cursor and holds RMB, or `MouseButton::Other(BH_CAPTURE_BUTTON)`. It builds with `CARGO_TARGET_DIR=<scratchpad>/target`, runs with `TUNGSTEN_SMOKE_FRAMES=250 TUNGSTEN_CAPTURE_FRAME=240` at the hold point 1250,430, and crops 720×600 around the hole.
- **Output:** `captures/before_black_hole.png` and `captures/before_black_hole_crop.png`.
- **What the before capture shows:** the 64 px vortex is drawn at about 324 px, so its arms are flat teal/violet ribbons in 5 px steps. The core is a plain dark disk, the halo barely shows, and the engine particles are hidden under the arms.

### 1. Sound synthesis (tools)

- **`sfx.py`:** stdlib only (`wave`, `struct`, `math`). Integer xorshift noise, one-pole filters, exponential envelopes. 44.1 kHz, mono, 16-bit. 5 ms fade-in, 20 ms fade-out, peak −2 dBFS. `build_sounds()` returns `{filename: bytes}`.
  - `fireball_cast.wav`, about 0.35 s: a band-passed noise whoosh with a rising then falling cutoff over a low swell.
  - `fireball_blast.wav`, about 0.9 s: a noise burst under a falling low-pass, a 70→38 Hz sine thump and sparse crackle.
  - `extinguish.wav`, about 0.7 s: a high-passed noise sizzle with a fast attack, flutter and decay.
- **`generate.py`:**
  - Each WAV becomes an owned output at `assets/sounds/<name>.wav`.
  - The literal `'assets/sounds/black_hole.ogg'` becomes `HAND_AUTHORED_SOUNDS = ('sounds/black_hole.ogg',)` in the stray-file check.
  - `validate_references` rejects a `sounds.json` path that is neither generated nor hand-authored.
- **`sounds.json`:** `ex10_fireball_cast_sfx` (volume 0.45), `ex10_fireball_blast_sfx` (0.7), `ex10_extinguish_sfx` (0.5), all non-looping.
- **Check:** regenerate, then run `--check` and the existing Python tests.

### 2. Black-hole art and particles (tools)

- **Sprites:** move the `vortex` and `vortex_core` drawing from `polish_art.py` to `effects.py`. `shock_ring`, the hearts and `fire_*` stay where they are. Redraw palette-only at 96×96 (D5), as neutral white/grey that extraction tints, each fading to transparent at its outer edge:
  - `vortex`: four tapered logarithmic-spiral arms in dithered white alpha steps, brightest on their inner edges, with a faint haze between them. Linear.
  - `accretion_disk` (new): an annulus with its inner edge at about 0.3 of the radius. Alpha peaks at that edge, falls outward and breaks into turbulent spiral bands. Linear.
  - `vortex_core`: an opaque near-black horizon (`θ`) with a thin cool rim and a faint lensing arc. Nearest.
  - `photon_ring` (new): a 1–2 px ring just outside the horizon, brighter on one side. Nearest.
  - `infall_streak` (new): a tapered streak with a bright head and a dithered tail. Linear.
  - Add the three linear names to `effects.LINEAR_FILTER`.
- **Configs (`generate.py`):**
  - `black_hole`: cap 1,200 → 900 and rate 720 → 540. New colour ramp: ice blue, violet, magenta, amber, white near the core. Wider scale spread. Radial speed 100–260, the `spark` sprite and continuous emission stay.
  - `black_hole_dust` (new): cap 300, `dust` sprite in deep indigo/violet at about 0.4 alpha, larger and slower. Radial speed 60–200; lifetime about 1.6–2.2 s.
- **Check:** regenerate, `--check`, the Python tests, and a scratchpad contact sheet of the five sprites at 1× and 5×.

### 3. Spell and extinguish particle configs (tools)

- `spell_trail` (new): `flame_glow`, continuous at about 140 Hz, cap 56, lifetime about 0.18–0.42 s. Slow drift in every direction with a slight rise, on the fire ramp. It forms a comet tail behind the moving missile.
- `fireball_blast` (new): a `flame_glow` burst of 22, radial 80–320 px/s, strong drag, fire ramp. It pairs with the existing `ball_explosion` sparks and the `Explosion` shock ring.
- `extinguish` (new): a `dust` burst of 14 in a rising cone, going from ember orange to pale steam grey to transparent over about 0.7–1.2 s.
- **Check:** regenerate, `--check`, the Python tests. The `fire_trail`, `fireball_drips` and `ball_burn*` JSON stay byte-identical.

### 4. Fireball spell (Rust)

- **`src/fireball.rs` (new):**
  - Constants: `FIREBALL_SPEED` 900 px/s, `FIREBALL_RADIUS` 10, `FIREBALL_LIFETIME` 1.4 s, `FIREBALL_COOLDOWN` 0.2 s, `FIREBALL_MAX_ALIVE` 6, `FIREBALL_BLAST_RADIUS` 72, `FIREBALL_VISUAL_SIZE` 40, `FIREBALL_GRAVITY` about 0.1 × `GRAVITY_Y`. Tune them in step 8.
  - Component: `FireballMissile { velocity, age, drips: Option<Entity> }`. The cooldown is read from the youngest live missile's `age`, so no extra resource is needed.
- **`cast_fireball_system`** (after `black_hole_force_system`): runs on `just_pressed("cast_fireball")` when under the cap and past the cooldown.
  - Origin: the player's centre, pushed a short way along the aim so the missile leaves the body edge (D1).
  - Direction: the normalized vector from the origin to the cursor's world point; if the cursor sits on the origin, the player's facing.
  - Spawns the missile with `Position`, `Transform`, `CurrentSprite("ex10_fireball_0")`, `AnimationState("ex10_fireball")` and an `ex10_spell_trail` emitter, plus an `EmitterAnchor` child that carries `ex10_fireball_drips`.
  - Plays the cast sound.
- **`fireball_flight_system`** (after `hazard_contacts`, so the missile tests post-physics ball positions and its ignitions spread in the same frame):
  - Integrates velocity: low gravity plus black-hole pull (D7, the same helper as `black_hole_force_system`). Advances the missile by `velocity·dt` and keeps the drip child on it.
  - Finds the earliest contact along the swept segment, sampled at most every 6 px: any `Collider` entity except the player (circle or AABB, grown by `FIREBALL_RADIUS`), or a solid collision-layer tile (`TilemapInstance` plus `TilemapRegistry`).
  - On contact, `explode_fireball` at that point:
    - `spawn_transient_effect` for `ex10_fireball_blast` and `ex10_ball_explosion`;
    - one `Explosion` ring, with the same cap check as `hazard_contacts`;
    - `burning::ignite` on every `SmallBall` within `FIREBALL_BLAST_RADIUS`, in id order;
    - the blast sound.
  - Past `FIREBALL_LIFETIME` or outside the world bounds, the missile despawns silently.
  - The missile always despawns together with its drip child. The player is never a contact target. The blast neither damages the player nor pushes bodies.
- **`setup.rs`:**
  - Bind `cast_fireball` to `Binding::Mouse { button: MouseButton::Other(4) }` in `platformer_bindings`.
  - Add both systems to `RUNTIME_SYSTEM_ORDER`.
  - At startup, look up the three sounds with `expect` like the others and insert `EffectSounds`.
- **`state.rs`:** `EffectSounds { cast, blast, extinguish: (AudioHandle, f32), extinguish_cooldown: f32 }`.
- **`systems.rs`:** `play_effect_sound(world, pick)`.
- **Hint text:** add `M4 fireball` to the controls line in `extract_text`.
- **Tests** (`src/tests/spells.rs`; add `mod spells;` and the new runtime-order entries to `tests/main.rs`):
  - `mouse4_casts_a_fireball_toward_the_cursor`: with the `platformer_bindings` binding, pressing `Other(4)` spawns one missile whose velocity points at the cursor, and one flight tick brings it closer.
  - `fireball_contact_explodes_and_ignites_small_balls`: a missile flying at a small ball despawns, leaves an `Explosion`, and the ball is burning.

### 5. Black-hole extinguishing and dust layer (Rust)

- **`black_hole_extinguish_system`** (`systems.rs`, after `spread_ball_fire`, before `black_hole_lifetime_system`):
  - Sets `remaining = 0` on every burning `BallBurn` within `BLACK_HOLE_RADIUS` of any hole.
  - Spawns at most `EXTINGUISH_BURSTS_PER_FRAME = 4` `ex10_extinguish` bursts, sampled evenly across that set.
  - Plays the sizzle at most once per `EXTINGUISH_SFX_INTERVAL = 0.15` s.
- **`spawn_black_hole_system`:** a press also spawns an `EmitterAnchor { parent: hole, offset: ZERO }` child with `ex10_black_hole_dust`.
- **`black_hole_lifetime_system`:** despawns `EmitterAnchor` entities whose parent is no longer alive. Hazard drips always have live parents.
- **`gameplay::scene_effects`:** steers particles whose emitter is a `BlackHole`, or an `EmitterAnchor` child of one, around that hole. `ex10_black_hole` keeps today's birth annulus and spiral.
- **Test `black_hole_leaves_burning_balls_inside_its_radius_spent`:** a burning ball inside the radius ends at `remaining == 0` and stays there after `ignite`; a burning ball outside keeps burning.

### 6. Extraction (Rust)

- **`extract_vortices`** becomes two passes, keeping `fade`, the size pulse and the soft-halo material path:
  - Under pass, before the engine particles: halo, `accretion_disk`, and the two counter-rotating `vortex` layers.
  - Over pass, after the particles:
    - 48 `infall_streak` sprites rotated along their spiral tangent, shifting from blue to amber as they near the core (they replace the 32 sparks);
    - `photon_ring`;
    - the `vortex_core` horizon.
- **`extract_fireballs`,** after the burning-ball flames and before the foreground, drawn unlit:
  - a `flame_glow` through `soft_flame`;
  - the current `ex10_fireball_*` frame at 40 px, rotated to its velocity and mirrored when travelling left so the drip side stays down.
- `cloud_parallax_and_vortex_instances_animate_without_world_mutation` must pass unedited.

### 7. Docs

- **`tools/README.md`:**
  - New section, "Fireball spell, extinguishing and sound effects":
    - the Mouse 4 control (winit Back, `"button4"`, an example-local binding);
    - missile speed, range and cap; the blast and ignition rule;
    - the extinguish rule; the black-hole layers and particle budget;
    - sound authoring: `sfx.py` synthesis, the WAV format, determinism, registration in `sounds.json`, `HAND_AUTHORED_SOUNDS`, and regenerating.
  - Module table: `effects.py` gains the black-hole sprites, `polish_art.py` loses them, and `sfx.py` gets a row.
  - Rewrite the "`sounds.json` preserves… the OGG remains independently owned" sentence.
  - "these two local action bindings" becomes three.
- **`docs/LLM_INDEX.md`:** add `examples/01_platformer/src/fireball.rs` to row 62, then run `just ctx`.

### 8. Visual review and after capture

- **After capture:** `capture_tree.sh after` produces `captures/after_black_hole.png` and its crop with identical settings. Put a side-by-side `captures/black_hole_before_after.png` next to them. Optionally, `capture_tree.sh spell 1250,430 4` shows a missile in flight.
- **Manual review** (run the example, zoom 35% and 300%):
  - the arms are legible and the streaks point the right way;
  - the horizon covers the particles;
  - extinguish puffs read over a burning pile;
  - the missile silhouette holds at speed, and the blast has impact.
- Tune the step 4 constants and the configs. Record any remaining limits here.

### 9. Final checks and summary

- Run every done-when check.
- **Report:** Rust tests 66 + 3, Python tests 15 unchanged, and the new owned-output count. List every new or changed ID, config and sound, and the capture paths.
- Note how this work affects the other two plans' done-when checks, without editing those plans.
- Set `status: done` and move this file to `docs/plans/archive/`.

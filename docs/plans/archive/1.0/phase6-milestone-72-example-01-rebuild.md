# Phase 6 milestone 72: example 01 rebuilt on the template (01 (a)) — done

- **status:** done
- **goal:** Example 01 rebuilt from scratch as a sandbox on `templates/basic` (`D-194`, [design](../../1.0/example-01-design.md) §01 (a)), carrying W12b's Must scope and W1 M6. Its shell is a byte copy of the template's, which `just repo-check` holds. Its player moves on the kit's platformer controller, which gains air jumps, and its three moving platforms on the path follower. Eight spells are kit abilities paid in mana, which a kit effect regenerates and a cast pauses. It has HP, mana and spell bars, an autosave and a toast on entering each of four areas, and saves of the whole game state with round-trip, coverage and divergence tests. It runs from its own folder in the workspace, in `just smoke` and in the release archives. The kit gains a stat-restoring payload, suppression, and exact saves for the controller, the path follower and `KitRng`. Core gains `World::component_types` for the coverage test and serde on `Pcg32`. The template's autosaves capture the world as the previous frame's command flush left it, a requested one waits until a capture succeeds, and a game asks for one through `shell::request_autosave`.
- **non-goals:** 01 (b) to (d): damage types and crits, frozen, burning and shocked as kit effects, lightning, the gun's firing, damage numbers, the dummy, summons, enemies and the new level. Effect sources, hitbox hit lists and state machines in saves, which (b) and (c) add. The design's backlog rows: stereo panning, looping kit cues that keep their voice, kit line of sight and a same-step order that survives a load. Also out: a new engine snapshot API, options of 01's own, slopes, ramps and water, a hit flash on lit sprites (Q6), the gamepad backend (W5a's re-land), trimming the root `assets/` and `input.json` of what only 01 read (Follow-ups), and any shell change beyond the pending autosave.
- **files to touch:** step 1: `crates/tungsten-core/src/ecs/{world,archetype}.rs`, `crates/tungsten-core/src/tests/ecs/world.rs`, `crates/tungsten-core/src/rng.rs`, `crates/tungsten-core/src/tests/rng.rs`; `crates/tungsten-kit/src/damage/effects.rs`, `motion/platformer.rs`, `motion/path.rs`, `foundation/rng.rs` and their `crates/tungsten-kit/src/tests/{effects,platformer,path,rng,describe,inspect}.rs`; `crates/tungsten/src/tests/kit_recipes/{platformer,top_down}.rs` with fixtures under `crates/tungsten/tests/fixtures/kit/{platformer,top_down}/`; `templates/basic/src/shell/mod.rs`, `templates/basic/tests/game/{shell,saves,sample}.rs` (the last two only where an autosave now lands a frame later), `templates/basic/AGENTS.md`; `docs/getting-started.md`; `api/tungsten-core.txt`, `api/tungsten-kit.txt`, `api/tungsten-kit-components.txt`. Steps 2–8: `examples/01_platformer/**` (`Cargo.toml`, `tungsten.json`, `input.json`, `README.md`, `src/`, `tests/`, `tools/`, `assets/`), the workspace `Cargo.toml` and `Cargo.lock`, `crates/tungsten-core/tests/manifests.rs`, `scripts/check-repo.py`, `scripts/test-check-repo.py`, `scripts/smoke-examples.sh`, `scripts/test-smoke-examples.sh`, `docs/LLM_INDEX.md`, `docs/validation.md`, `README.md`. Step 9: `scripts/release.py`, `scripts/test-release.py`, `tools/launcher/src/main.rs`, `docs/releases.md`, `scripts/check-repo.py`, `scripts/test-check-repo.py`. Step 10: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`, `DESIGN.md`, `docs/known-issues.md`, `docs/plans/1.0/{backlog-1.x,w12-template,w01-ui-text-suite,w13-kit,example-01-design}.md`, `docs/LLM_INDEX.md`. Step 11: the skeleton's release files.
- **ordered steps:** (1) Kit, core and template API, then the API critique. (2) 01 on the template's frame. (3) The level from the generator. (4) The player and the run. (5) The physics toys and the element rules. (6) Spells, mana and the HUD. (7) Saves. (8) The playthrough and the README. (9) Running from its folder in the archives. (10) Records. (11) Release 0.93.
- **done-when:** every step's evidence row quoted; workflow §5 closed (§8). The release checks pass after the cut, with any of `just level-check`, `just visual`, the kit bench's effect rows and the local packaging rehearsal that no step ran. The preflight prints the command block and the post-merge block.

Candidate 01 (a) rebuild ([implementation plan](../../1.0/implementation-plan.md) §4 card, level C, XL), milestone M72, release 0.93. Written 2026-10-20 at `22de917`. Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs. Sources: the [design](../../1.0/example-01-design.md) (Shared rules, Assets, §01 (a)), [W12](../../1.0/w12-template.md) step 3 (W12b) and [w01](../../1.0/w01-ui-text-suite.md) §11 (M6). Art review: `perf-runs/20261020-e01a-art-review/` (machine-local, nothing merged), the sheet the design asks this plan to draw: the eight (a) spell icons, the spell bar, the bowling ball, the toss clip, the arm and gun at eight aims and the lantern's cast.

## Context digest

- `D-194` placed four Must stops for example 01 between the example fixtures (0.92, `D-193`) and the Phase 6 QA pass. (a) carries W12b, rebuilding 01 on the template's layout with the shell copied unchanged, and W1 M6, which is 01's move onto that shell. `D-158` sends the platformer controller and the path follower to 1.x unless 01 uses them by the freeze; M52's attempt stopped at the `gate_step` corner (A7), which (a) widens.
- What it stands on: the template shell (`D-181`, `D-187`, `D-192`), the kit's controllers and helpers (`D-158`), stats and cues (`D-173`), motion (`D-174`), overlap and damage (`D-175`), effects and abilities (`D-177`), layers and the audio buses with `Voice` (`D-178`, `D-179`), the CLI's `check` and `package` (`D-182`) and the players' archives (`D-196`). `D-195` runs this stop's sessions on Opus 5.5 at max.
- This plan's audit found kit gaps the design did not list (A24): abilities spawn along the aim, never at a point; the character animator has no one-shot clips; `Lifetime` has no serde; a lit sprite never flashes. No public API lists an entity's component types (A25), which the coverage test needs. §2.4 records each gap's home.
- Later stops expect from (a): the shell-copy check, save IDs and the save layout, the spell bar with its empty slots 4 and 0, mana with its suppression (the troll's healing in (c) reuses suppression), and the element colours in the palette. The deferred cascade queue waits for (b)'s first gameplay chain (Q19).

## 1. Audit

Read at `22de917`. Paths under `examples/01_platformer/` are given from there.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | 01 runs from the repository root. It reads the root `tungsten.json`, `input.json` and `assets/manifest.json` (fonts `mono`, `sans` and `sans_bold`, `music_main`, `sfx_blip`, the `damage_flash` material) beside its own manifest, and two `TUNGSTEN_DISPLAY_*` hooks force a 1920 × 1080 window | `src/main.rs:29-44`, `src/state.rs:4-5`, `src/setup.rs:240-243`, `:560-590` |
| A2 | It registers 37 systems by hand in five slots and replaces the engine's sprite and text extracts. It has no states: death is a resource state machine, and Enter restarts by despawning everything | `src/setup.rs:32-236`, `:692-693`, `src/death.rs:24-35`, `src/setup.rs:647-668` |
| A3 | The player is a dynamic body (half-size 20 × 28, gravity 3,600) with its own input (run 560 px/s, jump 1,280 px/s), ground detection from contacts, and one air jump on a fresh press that refills on landing. It has no coyote time or jump buffer | `src/systems.rs:32-131`, `:67-76`, `:268-318`, `src/setup.rs:360-387`, `src/state.rs:36` |
| A4 | Hits: three hearts, a 1.1 s immunity, knockback (±380, −420), a 0.18 s control lock and shake. The player flashes because 01's own extract draws it unlit through `damage_flash` | `src/gameplay.rs:23-39`, `:566-603`, `src/extract.rs:376-445` |
| A5 | The three moving platforms follow hand-written sine paths: the code writes `Position` and carries riders within 2 px by hand. The 12 hazards (6 spikes, 6 fires on sine paths) have no collider; a swept box finds their contacts | `src/gameplay.rs:224-237`, `:300-318`, `:387-454`, `:264-299`, `:468-562`; `src/level_layout.rs:1845-1987` |
| A6 | `level_layout.rs` (2,338 generated lines) holds 260 props, 34 lantern anchors, 155 slab rows, the hazards and the platforms, plus the test-only `PLATFORMS` and `ROUTES`. At runtime the slab rows merge into 27 boxes to dodge a solver P3. Only the 10 ambient emitters come from `level.tmj`'s object layer | `src/level_layout.rs:1`, `:1727-1731`, `:1775-1781`, `:1988-2262`; `src/gameplay.rs:193-222`; `src/setup.rs:475-479`; `docs/known-issues.md:16` |
| A7 | At `gate_step`, the slab overhead (x 7424–7744, underside y 983) leaves 41 px above `gate_crest`, and the player is 56 px tall. Only the dynamic solver's push-out gets through, and M52's move onto the controller stopped there | `src/level_layout.rs:1727-1731`, `src/tests/level.rs:40-42`, `DECISIONS.md:2064` (`D-158`), `docs/known-issues.md:122` |
| A8 | Tools: LMB orbs, MMB small balls (five times the rate, half the size), RMB black hole (dragged while held), F ice spray (7 rays), R iron brick, Mouse 4 fireball, Enter restart, the M, S and 1–3 audio keys, a raw `KeyL` lantern, and wheel and ± zoom | `src/systems.rs:142-208`, `:627-634`, `:750-771`; `src/ice.rs:91-97`; `src/brick.rs:169-175`; `src/fireball.rs:86-92`; `src/gameplay.rs:148` |
| A9 | Caps: `EXTINGUISH_BURSTS_PER_FRAME` = 4 in `Update`: the black hole puts out every doused ball at once, and the cap only samples four steam puffs spread over the set each frame. `SMALL_BALL_BURSTS_PER_FRAME` = 4 counts impact bursts per fixed step and drops the rest. Both caps limit presentation, not gameplay. `BALL_CAP` = 12,000, `BRICK_CAP` = 24, `FIREBALL_MAX_ALIVE` = 6 | `src/state.rs:12-54`, `src/systems.rs:600-615`, `:938-952`, `src/gameplay.rs:62-90`, `src/fireball.rs:30-31` |
| A10 | Element rules:<br>• Only small balls burn: 10 s, spreading by contact and proximity; a spent ball never relights.<br>• The fireball's blast ignites within 72 px, shatters frozen iron and pushes bodies within 4.5 tiles.<br>• Ice chills iron over 1.5 s until it freezes.<br>• Frozen iron shatters into 16 scraps on a blast or an iron impact of 600 px/s or more.<br>• A brick at 360 px/s or more crushes balls.<br>• The black hole is the only extinguisher | `src/burning.rs:21-153`, `src/fireball.rs:354-427`, `src/ice.rs:148-237`, `src/brick.rs:267-419`, `src/systems.rs:922-964` |
| A11 | Screen text is raw `TextSection` from the replacement extract: a controls sheet, a status row and the death title. The hearts are screen-pinned sprites | `src/extract.rs:752-947` |
| A12 | Gameplay and presentation order by entity ID and seed from it, so a restart changes visuals | `src/systems.rs:944`, `src/burning.rs:149`, `src/brick.rs:107`, `src/gameplay.rs:295` |
| A13 | There are 106 tests: 105 in `src/tests/` (14 files) and one at `src/extract.rs:1290`, as `#[path]` modules of the binary. 52 call systems on a bare `World`, 38 run a partial `Harness` whose systems sit in `Update`, and 10 run the full game | `src/main.rs:19-21`, `src/tests/main.rs:2-15`, `:89-104` |
| A14 | The route test drives each consecutive platform pair of `ROUTES` with one jump from a `LAUNCHES` row, with no moving platforms or hazards, and prints a replacement row on failure | `src/tests/level.rs:9-55`, `:60`, `:134`, `:165` |
| A15 | `tools/generate.py` owns every file under `assets/` plus `src/level_layout.rs`, and `--check` compares bytes. The system Python has no Pillow (`import PIL` fails, checked 2026-10-20); a scratch venv with `tools/requirements.txt` installs 12.3.0 | `tools/generate.py:216-271`, `justfile:94-97` |
| A16 | Template: `frame()` takes `first_save` before `autosave` runs and drops its result. `autosave` is `save_to(AUTOSAVE)`, which returns `false` for four reasons (no capture, a read-only slot, no folder, a failed write). `goto` stages no `RunState`. The shell plays its sounds by fixed ID, and a missing ID is silent | `templates/basic/src/shell/mod.rs:540-565`, `:549`, `:553-555`, `:437-464`, `:260`, `:594-602`; `src/shell/theme.rs:26-35` |
| A17 | Template budgets: `src/shell` holds 2,375 of 2,400 code lines and `src` 2,872 of 2,900; `AGENTS.md` holds 12,255 of 12,288 bytes. Its seam line names `shell::game_over`, `shell::run` and `shell::run_meta`. No check holds a copy of the shell or the line caps | `templates/basic/AGENTS.md:10`, `:19`; `scripts/check-agent-context.py:27` |
| A18 | The shell needs:<br>• from 01's crate: `crate::game::{GAMEPLAY, gameplay, BINDABLE_ACTIONS, difficulty_note}`, `crate::save::{SaveGame, SaveError, capture, summary}` and `log`;<br>• from the manifest: the `shell` theme, the `sans` and `serif` fonts with their families and fallback, and six sound IDs.<br>01's crate has no `log` dependency | `templates/basic/src/shell/mod.rs`; `examples/01_platformer/Cargo.toml:1-19`; `templates/basic/assets/manifest.json` |
| A19 | Kit effects: payloads are damage and heal only, blocking tags are read once at application, and a loaded instance's source is `None` | `crates/tungsten-kit/src/damage/effects.rs:288-306`, `:1849-1855`, `:2078-2138`, `:1130` |
| A20 | Stats: bounds are fixed numbers, `set_base` stores its value unclamped, and only `max_health` caps anything. An ability's cost reads and spends the base. The ability tick runs in the intent phase, after `kit_tick` | `crates/tungsten-kit/src/foundation/stats.rs:42-50`, `:347-363`, `:782-784`; `crates/tungsten-kit/src/intent/ability.rs:1379-1401`, `:1630-1637` |
| A21 | `PlatformerController` has no serde or `PartialEq` and keeps its runtime state private. Its first step is still (`started`). Its filter is core's `SpatialFilter`, which has no serde. It has no air jump. Spatial queries find nothing before a world's first step | `crates/tungsten-kit/src/motion/platformer.rs:164-192`, `:429-433`, `:458-473`; `crates/tungsten-core/src/physics/query.rs:5-9`, `:61-68` |
| A22 | `PathFollower` has no serde. Its points are world points fixed at spawn from the scene's offsets, and `path_follow` changes only `time` | `crates/tungsten-kit/src/motion/path.rs:94-102`, `:311-316`, `:391-418` |
| A23 | `KitRng` has no serde, core `Pcg32`'s state words are private, and `D-173` left its serde open | `crates/tungsten-kit/src/foundation/rng.rs:36-41`, `crates/tungsten-core/src/rng.rs:10-14`, `DECISIONS.md:2350` |
| A24 | Gaps the design did not list:<br>• an ability spawns along the aim from its caster (`Payload::Spawn`), never at a point;<br>• `CharacterAnimator` has idle, move, rise and fall and no one-shot clip;<br>• `Lifetime` and `HitFlash` have no serde;<br>• a lit sprite draws without its material, so it never flashes | `crates/tungsten-kit/src/intent/ability.rs:117-145`, `crates/tungsten-kit/src/present/character.rs:400-453`, `api/tungsten-kit.txt` (no serde impl line for either), `docs/known-issues.md:57` |
| A25 | No public API lists an entity's component types. `AnyColumn` and `Archetype` are `pub(crate)`, and `World`'s public methods read one type at a time | `crates/tungsten-core/src/ecs/archetype.rs:95`, `:158`, `:177` |
| A26 | Most kit components a save holds already serialize: `Oscillator` (with its clock), `Abilities`, `Health`, `Stats`, `Effects`, `Tags`, `Projectile`, `LinearMotion`, `Knockback`, `Aim`, `Facing` and `CharacterAnimator` | `crates/tungsten-kit/src/motion/oscillator.rs:113-125`, `api/tungsten-kit.txt:2337` |
| A27 | The kit hash `0x6fbd_8f24_2114_15a5` runs each genre's fixture through a script and hashes positions, stat values, effect instances and ability cooldowns. A prefab that a genre's scene does not spawn stays out of it | `crates/tungsten/src/tests/kit_recipes/determinism.rs:34`, `:514-528` |
| A28 | Smoke runs 01 through example discovery from the root. The template has its own row with `run_in=templates/basic` | `scripts/smoke-examples.sh:92-98`, `:142-153`; `scripts/test-smoke-examples.sh:89` |
| A29 | Archives: examples read the root config and assets, and only templates run from a folder. `test_folders_match_the_launcher` compares the launcher's `FOLDERS` with the templates, and the launcher's own test maps `Platformer` to `data/` | `scripts/release.py:79-105`, `:1027-1037`, `:1066`; `scripts/test-release.py:840-846`; `tools/launcher/src/main.rs:31`, `:187-193` |
| A30 | The manifests test merges the root and every example manifest for global ID uniqueness, so a standalone 01 manifest that declares `sans` or `damage_flash` fails it. Templates load as their own roots | `crates/tungsten-core/tests/manifests.rs:15-63`, `:92-117` |
| A31 | `check-repo.py`'s checks are a hard-coded tuple. `check_kit_catalogue` returns early on an empty tree, which the test's `Output.run_main` relies on. The selector routes `src/level_layout.rs` to `just level-check`, and leaves `tools/launcher/` unclassified | `scripts/check-repo.py:147-162`, `:517-535`, `:650`; `scripts/test-check-repo.py:341-352` |
| A32 | Records that name 01's old files:<br>• the LLM index's rows on lines 15, 23, 78 and 79, whose paths `just ctx` resolves;<br>• `Cargo.toml:31` (cargo-shear);<br>• `docs/validation.md:24`;<br>• `DESIGN.md:371`;<br>• `README.md:39`;<br>• `docs/releases.md:114`;<br>• `D-123`'s "the workspace root is that folder for the examples" | those lines; `scripts/check-agent-context.py:72-90`; `DECISIONS.md` (`D-123`) |
| A33 | Input: scroll directions bind in `input.json`, and the Keybindings card scrolls once its rows overflow | `crates/tungsten-core/src/input/key_serde.rs:94-95`, `templates/basic/src/shell/keybindings.rs:1-3` |
| A34 | The frame flushes commands after `PostUpdate`, and `shell_frame` captures in `Update`. A save therefore misses what this frame's fixed step queued: an ability's `spawn` payload goes through the `CommandBuffer`, while its cost is already spent. `Exiting` is inserted between frames, so every stage of the last frame sees it | `crates/tungsten/src/app.rs:1748-1757`, `:1843-1852`; `templates/basic/src/shell/mod.rs:208-210`; `crates/tungsten-kit/src/intent/ability.rs:1430-1437` |
| A35 | The title loads every one of the eleven slots as a whole `SaveGame` | `templates/basic/src/shell/mod.rs:398-410`, `src/shell/title.rs:86-90` |
| A36 | An oscillator adds its displacement's change to the current `Position`, so restoring its clock alone does not restore where it is. Effect-granted tags are written onto `Tags` only at application; a loaded `Effects` restores its table of grants, not the tags | `crates/tungsten-kit/src/motion/oscillator.rs:620-632`, `motion/drive.rs:114-119`; `crates/tungsten-kit/src/damage/effects.rs:1225-1232`, `:1782-1790` |

## 2. Design

### 2.1 API sketch (level C; step 1's diff is critiqued against it)

**Core.**

- `World::component_types(&self, entity: Entity) -> Vec<(TypeId, &'static str)>`: the type and `std::any::type_name` of each component the entity holds, sorted by name, empty for a dead entity. It is documented as an introspection for tests and tools, whose names are the compiler's spelling. Behind it, `AnyColumn` gains `fn type_name(&self) -> &'static str`, which `TypedVec<T>` answers with `type_name::<T>()`; both are crate-private. No system or frame calls it.
- `Pcg32` gains `Serialize`/`Deserialize`: its two state words, through `try_from`, refusing an even increment. `PartialEq` it already has. This makes `KitRng`'s restore O(1) (Q8).

**Effects** (`damage/effects.rs`), two additions to `Effect` and its data form:

- `with_restore(stat, amount, max)` and `restore() -> Option<(&str, f32, &str)>`; data `"restore": { "stat": "mana", "amount": 1.5, "max": "max_mana" }`, all three fields required. `max` is required because a stat's own bounds are private to `Stat` (`foundation/stats.rs:54-62`), which this step does not open.
  - Each payload (each period, or once for an instant effect) raises the target's `stat` base by `amount` times the payload factor (stacks and the scale curve, as heal does), up to the value (`Stats::get`) of the stat `max` names. A base already at or above the cap is left alone.
  - It writes through `set_base` in the tick (`kit_tick`, after `stats_apply`), since an ability's cost reads the base (A20). An ability used later in that step's intent phase sees it, and so does 01's cast system, which joins `Phase::Intent` after `ABILITY_TICK`.
  - A target without `Stats`, or without either stat, takes nothing and leaves one trace line.
  - It panics on an invalid stat name or an amount that is not finite and above 0. The data form refuses the same, and the load checks names as the `stats` kind does.
- `with_suppressed_by(tags)` and `suppressed_by() -> &[String]`; data `"suppressed_by": ["casting"]`, sorted and deduplicated as `blocked_by` is, each tag checked at load. In each tick, while the target carries any listed tag, a running instance is suppressed: its time left and its period hold, no payload lands and it cannot expire. Its modifiers and granted tags stay. An application while suppressed still applies (that is `blocked_by`'s job). The trace records a suppressed instance once per suppression, not once per step.
- New description lines for both fields; the inspector rows show `restore` and `suppressed` only when set; no new scene name, kind or cue event.

**Platformer controller** (`motion/platformer.rs`):

- `with_air_jumps(jumps: u32)`, `air_jumps_left() -> u32` and `air_jumped() -> bool`; scene field `"air_jumps": 0`. Being grounded refills the count. A press while not grounded and past coyote time, with jumps left, jumps at `jump_speed` and arms the jump cut. A buffered press counts as a press. `jumped()` stays true for either jump, and `air_jumped()` tells the two apart for the step that jumped. With 0 nothing changes, so the kit hash holds.
- `Serialize`/`Deserialize` through private fields with `try_from` (`run_speed` 0 allowed, as `Steering`'s save allows a stat-written 0), plus `PartialEq`. The save form holds the settings and the runtime state, the air-jump count included. It leaves out the filter, which loads as `SpatialFilter::new()`, `new`'s, and the first-step flag, which loads unset. A loaded controller therefore takes its still first step, as a fresh one does, because the physics index is empty until the first step after a load (A21). A game that set its own filter sets it again.

**Path follower** (`motion/path.rs`): `Serialize`/`Deserialize` (points as world points, speed, mode, easing, time) through `try_from` with `new`'s checks plus a finite time within the path (`[0, total]` for a stopping path, `[0, cycle)` for the others), and `PartialEq`. A loaded follower keeps its world points; a game restoring a level object inserts the saved one over the prefab's.

**`KitRng`** (`foundation/rng.rs`): `Serialize`/`Deserialize` as `{ "seed": u64, "draws": u64, "rng": <Pcg32> }`, plus `PartialEq`. A load restores the generator's words, so its cost does not grow with the draws. The title reads every slot whole (A35), so a replay from the seed would cost each slot its draws again (Q8).

**Template** (`templates/basic/src/shell/mod.rs`): §2.2.

The step 1 diff is the critique's artifact: every `api/` change plus the template diff. The kit additions are additive. `Effect`, `PlatformerController`, `PathFollower` and `KitRng` are structs with private fields, so new private fields break nothing. The serde and `PartialEq` impls and the new methods are additions for the freeze, and step 10 gives them their ledger row.

### 2.2 The template's pending autosave

- `RunState.first_save` becomes `autosave_due`. `start_run` sets it, and the new `pub fn request_autosave(world: &mut World)` sets it on the live run; with no run, or during a run change, it does nothing, and the next run's first autosave covers it.
- The due autosaves (the run's first, a requested one, the exit's) leave `frame()` in `Update` for a new system, `shell_save`, at the start of `PreUpdate`, ordered before `STATE_DISPATCHER`. A capture then reads the world as the previous frame's command flush left it (A34): a cast's spent cost and its queued spawn land in the same save. A request made in frame N saves at the start of frame N+1. The exit's last frame runs `PreUpdate` with `Exiting` (A34), so the exit still saves in the last frame (`D-180`). The pause screen's and the Save screen's saves already run from screens in `PreUpdate`, after the previous flush.
- In `shell_save`, `save::capture(world)` runs first. Only a `Some` leads to `autosave(world)` and clears the flag, so a request waits through frames whose capture is `None`. One capture that succeeds clears it even if the write then fails, as today's single attempt does. That keeps `retry_starts_fresh_when_the_runs_first_autosave_failed` (`templates/basic/tests/game/shell.rs:412`) and the one-write assertion (`:952`) true, and the flag clears under `UserFolder::Off`. Keying on `autosave()`'s `bool` would not clear there (A16).
- Pause's autosave stays one-shot, as `D-194` lists only the first, the exit's and the game's requests.
- A template test that counted frames to the first autosave may need one more step; each such edit is named in the evidence.
- Within budget: about twelve shell lines against 25 of room (A17), written in the shapes M69 used.
- `AGENTS.md`'s seam line gains `shell::request_autosave`, and its saves line gains "and when the game asks through `shell::request_autosave`; a request waits until `capture` returns a save". The equal cut comes from wording the guide also carries, never a kit name (`check_kit_catalogue`): the Engine documentation line's gloss on `tungsten::core` and `tungsten::kit`, and "and reports every error with its file" on the `tungsten check` line.
- `docs/getting-started.md:41` gains the callback, and `:85` gains the request in its trigger list.
- Tests, in `tests/game/shell.rs`: a test plugin with a system ordered `before("shell_save")` makes `capture` return `None` for the first frames of a run (it hides the sample player's `Health`, then restores it).
  - `a_first_autosave_waits_for_a_capture`: no write while hidden, exactly one write after.
  - `a_requested_autosave_waits_for_a_capture`: `request_autosave` while hidden, then one write once the capture returns a save, and no second write from the same request.

  01's `saves::` tests cover the same-frame cast (§2.6), since the sample saves no bolts.

### 2.3 Example 01's layout

`examples/01_platformer/` follows `templates/basic/AGENTS.md`:

- `Cargo.toml`: package `example-01-platformer` with a `[lib]` and a binary, the `game` integration test (`tests/game/main.rs`), workspace dependencies plus `log`, and the `testing` feature for tests.
- `tungsten.json`: `game.id` `tungsten-platformer`, window "Platformer" at 1280 × 720, `render` and `audio` as the template's, and the physics settings today's `seed_world` sets in code (gravity 3,600 and the tile broadphase) wherever the config takes them, else in the level plugin's `build`.
- `input.json` (§2.5) and `assets/`, generator output only.
- `src/main.rs`, the template's, naming 01's crate.
- `src/lib.rs`: `pub mod features; pub mod game; pub mod save; pub mod shared; pub mod shell;`.
- `src/game.rs`: the four seam names, `MANIFEST` and `register`, which adds `KitPlugins::set()`, the shell and 01's feature plugins.
- `src/save.rs`: the seam's three names, `migrate` and `restore` (§2.6).
- `src/shared.rs`: `SaveId` and the `SaveIds` counter.
- `src/shell/`: the byte copy.
- `src/features/<name>/`, each with `mod.rs` (plugin, state and constants), `components.rs` and `systems.rs`:
  - `run`: the gameplay state `GAMEPLAY`, a new or loaded start, death to `shell::game_over`, the kill row, areas;
  - `level`: the map, its objects, the backdrops, the moon, glows and lights;
  - `player`: the prefab's kit items, the one-shot clips, the lantern, zoom;
  - `spells`: slot selection, casting through `use_ability`, held spells, with `hud.rs`;
  - `toys`: small balls, bowling balls, iron blocks and scraps, the black hole, force push, the sprays and the missile's flight against balls, its blast and its bending, each placed or fired through a function the spells call;
  - `elements`: burning, chill and freezing, shattering, extinguishing, crushing and impact bursts;
  - `audio`: music loops and crossfades, and voices set by distance.
- `tests/game/`: `main.rs` with helpers, and one module per part (§2.9).
- `tools/`: the generator, `tools/content/` (prefab and data sources the generator copies verbatim) and `tools/README.md`.
- `README.md`: running and testing from this folder, and each mechanic's kit module and data file.

01 reads nothing outside its folder, has no `TUNGSTEN_*` hook (the engine's smoke variable is not 01's), registers no engine system by hand, and keeps the engine's default sprite and tilemap drawing. Its backdrops, glows and cursor go through `Extracts::add_sprites`. Nothing in 01 copies the shell's 132 tests: the template runs them on the same bytes, which the shell-copy check guarantees.

### 2.4 Kit map

Every system of today's 01 (A1–A12), with its home in (a) and the reason when it stays game code:

| Today | (a) | Home |
| --- | --- | --- |
| Run, jump, ground detection (A3) | `PlatformerController` (run 560, jump 1,280, the world's gravity) driven by `ActionInput` (`move_left`, `move_right`, slot `jump`) | Kit |
| The air jump (A3) | The controller's air jumps (§2.1), with `air_jumps: 1` | Kit, Q1 |
| Hearts, immunity, knockback, control lock, shake (A4) | `Health` (100, 1.1 s window), `Knockback` (launch and lock), a `damaged` cue that shakes | Kit |
| The hit flash (A4) | `HitFlash`, which draws nothing on the lit player (A24) | Kit, a known limit, Q6 |
| Spike and fire contact (A5) | `ContactDamage` on the hazards, a `Hurtbox` on the player | Kit |
| Fire hazards' sine paths (A5) | `Oscillator` | Kit, Q2 |
| Moving platforms (A5) | `PathFollower`, ping-pong between two points with sine easing (today's sine motion), on kinematic bodies, riders carried by the controller's ground carry | Kit |
| The slab merge (A6) | The generator writes one collider object per merged run | Generator; the P3 workaround leaves runtime |
| Props, lanterns, glows, lights, emitters (A6) | Prefabs placed by the map's object layers through `spawn_map_objects` | Engine |
| Areas | `Zone` sensors on an `areas` layer; on `ZoneEntered` for the player, 01 calls `shell::request_autosave` and `shell::toast` | Kit; the two shell calls are game code |
| Camera | `CameraTarget` and the engine's bounds; the base zoom by window height and the ± zoom actions | Kit; zoom is 01's |
| Clips (A3) | `CharacterAnimator` for idle, walk, rise and fall; 01 plays the land, air-jump and toss one-shots after it in the present phase and hands back | Kit; one-shots are game code (A24, a backlog row) |
| Spells' costs, cooldowns, charges and mana | `Abilities` with manual triggers, `Stats` (`mana`, `max_mana`), effects `mana_regen` and `casting` | Kit |
| Spell selection, held spells, placement at the cursor | 01 calls `use_ability` for the selected slot on a press, or each step while held, and places at the cursor through `prepare_prefab` once it returns `Used::Used` | Game: one cast action fires the selected slot, and the kit spawns along the aim, not at a point (A24, backlog row `place`) |
| The fireball (A8) | Slot 1's `spawn` payload: a `Projectile` with `LinearMotion`; 01's blast | Kit, with game blast rules |
| One-shot sounds | Cues on `ability_used`, `damaged`, `killed` and the game's events | Kit |
| Loops (sprays, the black hole, music) | `AudioCommands` with their `Voice` (`D-179`) | Game: kit cues keep no voice (design) |
| Burning, chill and freezing, shattering, extinguishing, crushing, impact bursts, the black hole's pull, blasts, force push, tosses (A10) | Ported from today's code with its tests | Game, Q3: rules of 01's physics toys. Kit knockback moves kinematic bodies, not a pile of dynamic ones. (b) moves burning and freezing onto damage types, effects and the ailment ladder |
| The black hole's lifetime | A `left` field on 01's `BlackHole` | Game: `Lifetime` has no serde (A24) |
| Ball hue | 01's presentation | Game |
| Death and restart (A2) | `shell::game_over`, then the shell's Retry and Load | Template |
| Screen text, hearts (A11) | The shell's screens, the HUD (§2.5) and the engine's debug HUD | Template, engine |
| Audio keys (A8) | The shell's Audio settings | Template |

### 2.5 Spells, mana, input and the HUD

Starting values, tuned in the run and recorded in its evidence row:

| Slot | Spell | Element (border) | Use | Cost | Cooldown | Charges | Kit payloads; 01's part |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | Fire missile | Fire | Press | 8 | 0.25 s | — | `casting`; `spawn` `fire_missile` at 900 px/s, offset 24, toward the aim. 01: at most 6 alive, the blast (A10), bending by the black hole |
| 2 | Flamethrower | Fire | Held | 20/s (one use a step) | — | — | `casting`. 01: the shared spray, a short cone that ignites small balls it reaches |
| 3 | Ice spray | Cold | Held | 18/s | — | — | `casting`. 01: the shared spray with today's beam rules (A10) |
| 4 | Lightning | Shock | (b) | | | | Empty slot |
| 5 | Force push | Mana | Press | 25 | 2 s | — | `casting`. 01: a radial push of dynamic bodies within 6 tiles by distance and mass (today's blast push) and the shock ring re-tinted |
| 6 | Black hole | Mana | Press | 30 | 1 s | — | `casting`. 01: placed at the cursor for 4 s, one at a time (Q4), today's pull and extinguishing |
| 7 | Iron block | Physical | Press | 15 | 0.4 s | 3, one back every 2 s | `casting`. 01: placed at the cursor; `BRICK_CAP` 24, the oldest goes |
| 8 | Bowling ball | Physical | Press | 12 | 0.6 s | — | `casting`. 01: a heavy dynamic body tossed from the hand toward the aim; it lands low and rolls |
| 9 | Small balls | Physical | Held | 15/s | — | — | `casting`. 01: today's rate (five times the orb rate) at the cursor, up to `BALL_CAP` 12,000 |
| 0 | Summon | Mana | (b) | | | | Empty slot |

- **Mana:** `mana` and `max_mana` in the `stats` data kind, starting at 100 of 100. `mana_regen` is an infinite effect on the player prefab with period 0.1 s, `restore` `{ "stat": "mana", "amount": 1.5, "max": "max_mana" }` (15/s) and `suppressed_by` `["casting"]`. `casting` grants the tag `casting` for 1.0 s and refreshes on a repeat. A held spell's per-step cost is its per-second cost over the step, and running dry stops it (`Used::NoCost`). The gun and the lantern cost nothing.
- **Input** (`input.json`; every action in `BINDABLE_ACTIONS`; pad bindings declared, live once the backend lands):

  | Action | Keyboard and mouse | Pad |
  | --- | --- | --- |
  | `move_left`, `move_right` | A, D, ←, → | Left stick |
  | `jump` (and the air jump) | Space | South |
  | `cast` (hold for held spells) | RMB | LT |
  | `spell_next`, `spell_prev` (wrapping) | Wheel down, wheel up | RB, LB |
  | `spell_1` … `spell_9`, `spell_0` | 1 … 9, 0 | — |
  | `lantern` | L | North |
  | `zoom_in`, `zoom_out` | =, − | — |
  | `state_pause` | Esc | Start |

  Aim comes from the cursor (the kit's `Aim` `from: cursor`); the right stick waits for the backend (Follow-ups). F4, F9 and F11 stay the engine's. Shoot (LMB, RT) and the summon cycle (Q, E) arrive with (b).
- **HUD**, in the shell's theme, built as `templates/basic/src/sample/hud.rs:39-52` builds its bar:
  - at the top left, an HP bar above a mana bar, each with "current / max" in mono, the mana fill in indigo;
  - at the bottom centre, the spell bar: per slot its icon (the element's border), its key, its cost, a cooldown sweep and its charges;
  - the selected slot outlined in the theme's accent, an unaffordable slot greyed, and slots 4 and 0 empty.

  It is a `UiLayer::Game` root that is not hit-testable, owned by the gameplay state.
- **Music:** `music_title` and `music_over` are the shell's, by ID. 01 plays `music_explore` and pauses under `music_pause`, as the template's sample does (pause's loop over the silenced game track), crossfading with M68's fades.

### 2.6 Saves

- **Identity:** every saved entity carries `SaveId(u64)`. The map's objects take IDs in the order of the entities `spawn_map_objects` returns (`crates/tungsten-core/src/assets/prefab.rs:675-679`), which is layer and file order and so stable. Anything spawned later takes the next value of the saved `SaveIds` counter. A save refers to other entities by save ID (a missile's `Source`) and remaps on load. Nothing orders by entity ID (A12).
- **`SaveGame`, schema 4.** The template says to count on from the sample's 3, so `migrate` refuses 1–3 and a sample save reads as damaged. Fields:
  - `meta`, `next_id`, `rng` (`KitRng`), `area` (the last area entered), `spell` (the selected slot), `lantern`;
  - `player`: save ID, position, velocity, controller, health, stats, effects, abilities and `Tags`. The tags are saved because an effect's granted tags live on `Tags` (A36): without them a save during `casting` would load with regeneration running;
  - `platforms` (save ID, position, `PathFollower`) and `hazards` (save ID, position, `Oscillator`), positions included because the follower steers toward its point from the body's position and the oscillator adds its change to it (A36);
  - `balls` (save ID, small or bowling, position, velocity, hue, burn state);
  - `blocks` (save ID, brick or scrap, position, velocity, chill, frozen);
  - `black_hole` (save ID, position, time left);
  - `missiles` (save ID, position, `Projectile`, `LinearMotion`, the source's save ID).

  `capture` returns `None` only with no run or a dead player. `summary` reads "Area · HP current / max · Mana current / max".
- **Load:** `game::gameplay(RunStart)` with a save spawns the map and writes each saved state over the map object with that save ID. It then spawns each dynamic entity from its prefab at its saved point and writes its saved components over the prefab's. Every restored position writes `Position` and `PrevPosition` together. On entering, the area the save names does not toast or autosave again.
- **Not saved** (presentation): particles, lights, glows, flame pools, animation and facing state, hit flashes, shake, toasts, the HUD and music voices. The `casting` and `mana_regen` instances load with `source: None` (A19), which nothing in (a) reads; (b) adds effect sources.
- **Tests,** in `tests/game/saves.rs`:
  - *Round-trip:* capture, load into a fresh harness, capture again, and the two are equal as JSON. The capture is taken with a fire hazard and a platform away from their spawn points.
  - *Coverage:* after a scripted minute (every spell cast, iron frozen, balls burning, a missile in flight, a black hole open, 500 balls in the pit), every type `World::component_types` lists on an entity with a `SaveId` must be in exactly one of three lists, held per entity kind (player, platform, hazard, ball, block, scrap, black hole, missile): saved, rebuilt from the prefab, or presentation-only. A type in none fails with its name and kind.
  - *Same frame:* a fire missile cast in the fixed step that also requests an autosave (an area's start) saves both the spent mana and the missile.
  - *Casting:* a save taken right after a cast loads with `casting` on the player, and mana stays paused for the rest of the window with no new cast.
  - *Divergence:* save mid-play in the Lantern clearing, then compare the uninterrupted run with the loaded one under the same scripted inputs for 3 s. Run it twice, once loaded into a fresh harness and once into a harness that first spawned and despawned 10,000 entities. Both runs must hold:
    - the same save IDs alive and the same burning and frozen sets;
    - health equal, mana within one regeneration period (1.5), cooldowns within one step;
    - platforms and fire hazards within 0.5 px, iron within 8 px;
    - the player within 12 px, one full-run step plus a pixel, because of the loaded controller's still first step (§2.1).

    Ball positions are not compared: the solver's warm-start, sleep and sensor pairs are not saved (design, Divergence).
  - *Exact:* the controller, the path follower and `KitRng` are step 1's.
  - *Migration:* schemas 1–3 are refused.
  - *Size:* 12,000 balls in the pit save in at most 2 MiB (Q11).
- **Size and time:** `cargo test --release` prints the size and the capture-plus-write time with the pit full. The evidence records both; the time target is 50 ms. Over either limit, compact the ball record (an array per ball) before raising the limit.

### 2.7 Cascades

Today's two caps limit presentation, not gameplay (A9). (a) moves both onto the fixed step, as the design's shared rule asks, and changes no outcome:

- The black hole's extinguishing moves from `Update` to the fixed step and still puts out every doused ball at once. Its steam puffs stay a sample of four spread over the doused set, now per step.
- The small-ball impact bursts keep their cap of four per step. The constants lose `PER_FRAME` from their names.

No chain in (a) defers gameplay work, so the design's deferred queue, its bound and the full-pit test proving the bound, and its place in saves, land with (b)'s first gameplay chain, its explosions (Follow-ups). Building the queue here around puffs would delay extinguishing that today is immediate, and would save presentation.

### 2.8 Assets and the generator

The generator stays the only writer of `assets/`, and `just level-check` stays the gate:

- **Level placements** become object layers in `level.tmj` (§3 step 3): props, lanterns and glows, merged slab colliders, hazards, platforms, areas, the existing emitters, and a `spawn` marker without a class. Their classes name generated prefabs. A scalar parameter is a property override (`D-157`'s dotted paths); a parameter held in an array (an oscillator's amplitude, a path's points) takes a generated prefab per object. `src/level_layout.rs` goes. The route test reads `tools/level.json` itself.
- **`tools/content/`** (prefabs and data, by folder) is copied verbatim and listed in the manifest.
- **The shell's assets** are copied from `templates/basic/assets/`: the `shell` theme, Inter and Source Serif 4 with their licence files, and the `damage_flash` shader and material. `--check` then also catches drift from the template.
- **Sounds** follow the template's `tools/sounds.py` precedent (`D-187`): 16-bit mono WAV, synthesized with the standard library, `--check` comparing bytes, each loop's length and its seam. 01's own sounds go under the shell's IDs: `ui_move`, `ui_select`, `ui_back` and `game_over`. The music loops at 22,050 Hz are `music_title` and `music_explore` (about 30 s each) and `music_pause` and `music_over` (about 15 s each), about 4 MB in all. The spell, toss and placement sounds join today's.
- **Art,** following the review sheet and any notes the owner leaves in §4 before pasting the run prompt (Q15):
  - step 5: the bowling ball;
  - step 6: the eight (a) icons at 32 × 32, at whole-number scale on the bar;
  - step 6: the toss clip (four frames: wind-up, swing, release, follow-through);
  - step 6: the arm and gun at eight aims: five drawn facing right, the courier turning to face the aim;
  - step 6: the lantern's cast: a cast twin of each frame with hotter glass and a lit frame, whose emissive mask is wider, shown for 0.3 s. If the lit sprite path takes a per-sprite emissive strength, the run uses that instead.

  The palette gains the indigo the mana bar and icons use and the violet the black hole's icon uses; maroon waits for (b). The player's frames keep their normal and emissive maps, so the player draws lit (Q6).
- **Python tests:** today's 15 pass, updated where their outputs moved. Following the light testing rule for examples, no new Python test is added.

### 2.9 The 106 tests

Each test is ported to `tests/game/<module>.rs` under its old name, its trigger moved to the slot or action of §2.5, or retired with its reason. 92 are ported and 14 retired:

| Today's file | Tests | Ported to | Retired, and why |
| --- | --- | --- | --- |
| `tests/main.rs` | 3 | `seam` (`configure_app_seeds_expected_bootstrap_state`, as a run start without orbs or status state); `player` (`a_press_moves_the_player_on_the_next_step`) | `runtime_schedule_matches_expected_pipeline`: 01 no longer orders its systems by hand, and the kit's and template's tests pin theirs |
| `ball_pit.rs` | 7 | `ball_pit`, 6 (slot 9 in place of MMB) | `audio_stop_is_s_only_even_after_shared_input_reload`: the S and M keys are gone |
| `level.rs` | 6 | `level`, 5 (the route test on the controller; the ride through a cycle on the path platforms; sweeps from path points; the thin underside; one collider per slab object) | `prescribed_platforms_carry_supported_players_and_release_jumps`: the hand-written carry is gone; the kit's ride test and the ported cycle ride cover it |
| `presentation.rs` | 13 | `presentation`, 10 (frames now lit, without orbs; the lantern on its action); `hud`, 1 (hearts → the HP bar) | `extract_text_includes_debug_state_row`, `update_text_display_refreshes_default_state_on_first_tick`: the status row gives way to the engine's debug HUD |
| `fixed_step.rs` | 5 | `fixed_step`, 5 (the 30 Hz air jump on the kit's) | — |
| `player.rs` | 10 | `player`, 7 (air jumps exhausted, fresh press, tuck clip; out of bounds and the kill row end the run with the shell's game over) | `player_moves_right_on_d` (the ported `a_press_moves…` covers it); `player_becomes_grounded_after_falling_onto_tilemap` and `…_when_collision_event_lists_player_as_b`: the controller's ground probe replaces contact grounding, and the kit tests it |
| `camera.rs` | 4 | `camera`, 3 | `cursor_to_world_inverts_camera_translation_and_zoom`: engine behaviour, which core's and the kit's tests cover |
| `hazards.rs` | 7 | `hazards`, 7 (the first without the M and S keys; contact damage at 34 per hit, so three hits kill; fires on oscillators) | — |
| `spells.rs` | 5 | `spells`, 5 (slot 1 for Mouse 4) | — |
| `spawning.rs` | 10 | `spawning`, 7 (slot 6 places the hole) | `spawn_ball_system_spawns_at_fixed_rate_while_held`, `spawn_ball_system_resets_accumulator_on_release`: the orbs get no slot; `spawn_black_hole_system_drags_active_hole_to_cursor_while_held`: the hole is placed for its lifetime (Q4) |
| `burning.rs`, `brick.rs`, `ice.rs` | 6, 10, 15 | `burning`, `brick`, `ice` (slot 3 for F) | — |
| `death.rs` | 3 | `player`, 1 (`a_fall_is_fatal_during_immunity_…` keeps its rule: a fall past the kill row ends the run during the window) | `restart_rebuilds_the_world_as_a_fresh_launch`, `the_death_screen_dims_takes_the_restart_and_uncovers_a_new_run`: the shell's Retry and game over, which the template tests |
| `regime.rs` | 1 | `regime`, 1 (without the HUD's contact count) | — |
| `src/extract.rs:1290` | 1 | `spawning` (`rainbow_rgba_keeps_primary_hues_fully_saturated`) | — |

A partial harness's systems ran in `Update` (A13). A port drives the real plugins on the fixed step, so a timing assertion may move by a step, and the evidence names each one that moved.

A test lands in the step that builds everything it touches. A rule a test exercises directly (spawning a ball, a brick or a missile) lands with the toys in step 5. A test that presses a spell's trigger lands with the spells in step 6. The fire-against-ball hazard tests, the 144 Hz ball draw test and the vortex test also wait for step 5's toys.

New tests are only those the design's done-when needs: `seam`, `areas`, `saves` and `playthrough`, each with the few tests §3 names, and the one mana test in `spells`.

### 2.10 Running from its folder

- **Manifests test:** an example whose folder holds `tungsten.json` loads its manifest as its own root, as a template does, outside the examples' uniqueness merge (A30).
- **Smoke:** discovery runs every `example-*` package from the root except one whose folder holds `tungsten.json`, which runs with `run_in=<its folder>`, as the template's row does (A28). The stub test expects `dir=examples/01_platformer` on 01's line.
- **Shell-copy check:** `check_shell_copies` in `scripts/check-repo.py` compares every `examples/*/src/shell/` with `templates/basic/src/shell/`. The file sets and bytes must match, and an error names the file and what differs. It returns early when either folder is missing and joins the tuple at `:650`. The selector routes `examples/<x>/src/shell/` to `just repo-check`.
- **Archives** (`D-196`): folder games are the templates plus every shipped example whose folder holds `tungsten.json`.
  - A folder game ships `tungsten.json`, `input.json` and `assets/` under `data/<folder>/`, and its log folder appears in `README.txt`.
  - Examples ship `assets/` alone and keep the shared root files (02 still reads them).
  - The launcher's `FOLDERS` gains `("Platformer", "examples/01_platformer")`, and `test_folders_match_the_launcher` compares the table with the folder games.

## 3. Steps

Every code step's done-when also includes `cargo fmt --all -- --check` → exit 0 and `just lint` → exit 0, which the rows below do not repeat. Each step that runs the generator needs the scratch venv (`python3 -m venv <scratchpad>/level-venv && <scratchpad>/level-venv/bin/pip install -r examples/01_platformer/tools/requirements.txt`). The generator then runs as `<venv>/bin/python -B examples/01_platformer/tools/generate.py`, and `just level-check` as `PATH="<venv>/bin:$PATH" just level-check`. Test filters go after `--`.

### Step 1: Kit, core and template API

- **Files:** step 1's in the header.
- **Change:** §2.1 and §2.2, in this order: core's `component_types` and `Pcg32`'s serde; effects; the controller; the path follower; `KitRng`; the template; recipes; `just api`.
  - Core unit tests: `component_types_lists_an_entitys_components`, and `a_saved_pcg32_continues_its_stream` with an even increment refused.
  - Kit unit tests:
    - `restore_stops_at_the_max_stat`;
    - `a_suppressing_tag_pauses_and_resumes_a_running_effect`: a tag added after regeneration started pauses it, its time and period held, and removing it resumes it;
    - `air_jumps_refill_on_landing_and_spend_once_per_press`;
    - `a_save_mid_jump_continues_exactly`: the save forms are equal; loaded into a fresh world with the same floor, the controller takes its still step, then matches the uninterrupted one field for field for 30 steps;
    - `a_save_mid_cycle_continues_exactly`, for a looping and a ping-pong path;
    - `a_saved_stream_continues_where_it_stopped`;
    - each refused data shape.
  - Recipes in two genres for `restore` with `suppressed_by`: platformer mana that a cast pauses, and top-down stamina that sprinting pauses. One platformer recipe for an air jump onto a ledge.
  - The new fixture prefabs stay out of each genre's scene, so the kit hash holds (A27).
- **Done-when:**
  - `cargo test -p tungsten-core --lib -- ecs::world::tests:: rng::tests::` → passes, the two core tests among them;
  - `cargo test -p tungsten-kit --lib -- damage::effects::tests:: motion::platformer::tests:: motion::path::tests:: foundation::rng::tests:: foundation::describe::tests:: foundation::inspect::tests::` → passes, the six named tests among them;
  - `cargo test -p tungsten --lib -- kit_tests:: kit_recipes::` → passes, `kit_recipes::determinism::` with `KIT_RECIPES_HASH` unchanged and `kit_recipes::coverage::` unchanged;
  - `cargo test -p tungsten-template-basic --locked --test game` → 134 tests pass, the two of §2.2 among them;
  - `test "$(tokei templates/basic/src/shell -o json | jq '.Total.code')" -le 2400 && test "$(tokei templates/basic/src -o json | jq '.Total.code')" -le 2900` → exit 0;
  - `rg -c 'shell::request_autosave' templates/basic/AGENTS.md docs/getting-started.md` → at least 1 in each;
  - `just ctx` → passes;
  - `just api`, then:
    - `rg -c '^impl serde_core::ser::Serialize for tungsten_kit::(PlatformerController|PathFollower|KitRng)$' api/tungsten-kit.txt` → 6;
    - `rg -c '^impl core::cmp::PartialEq for tungsten_kit::(PlatformerController|PathFollower|KitRng)$' api/tungsten-kit.txt` → 6;
    - `rg -c '^pub fn tungsten_kit::PlatformerController::(with_air_jumps|air_jumps_left|air_jumped)\(' api/tungsten-kit.txt` → 6;
    - `rg -c '^pub fn tungsten_kit::Effect::(with_restore|restore|with_suppressed_by|suppressed_by)\(' api/tungsten-kit.txt` → 8;
    - `rg -c '^pub fn tungsten_core::World::component_types\(' api/tungsten-core.txt` → 2;
    - `rg -c '^impl serde_core::ser::Serialize for tungsten_core::rng::Pcg32$' api/tungsten-core.txt` → 2;
    - `rg -c '^  field (air_jumps|restore|suppressed_by): ' api/tungsten-kit-components.txt` → 3;
    - every line at `HEAD` is kept: `python3 -B -c "import subprocess; fs=['api/tungsten-core.txt','api/tungsten-kit.txt','api/tungsten-kit-components.txt','api/tungsten.txt','api/tungsten-render.txt']; bad=[f for f in fs if set(subprocess.run(['git','show','HEAD:'+f],capture_output=True,text=True,check=True).stdout.splitlines())-set(open(f).read().splitlines())]; print(bad); raise SystemExit(bool(bad))"` → `[]`, exit 0;
  - `just api-check` → passes;
  - once `pgrep -x nxcodec.bin` and `pgrep -af 'scripts/[b]ench.py'` print nothing, one blocking foreground run of `cargo bench -p tungsten-kit --bench kit -- effect_` → `effect_tick` and `effect_resolve` each within the per-entity bound. The bound is the median per entity at 10,000 at most twice that at 1,000, read from criterion's `estimates.json` as M67's ratios were (`docs/perf/profiling-workflow.md:321`);
  - `just visual` → passes;
  - `just smoke` → passes.
- **Moves:** the three `api/` snapshots gain lines, and none is removed.

**Then the API critique** (skill, Running steps). Run `just api` first; it already ran above. Then run the skill's command in the background: `critique --raw --repo --force --out <scratchpad>/api-critique.md --from 'git --no-pager diff HEAD -- api/ templates/; git ls-files -o --exclude-standard -z -- api templates | xargs -0 -r -n1 git --no-pager diff --no-index /dev/null'`. Read the 5-hour window first; above about 65%, queue the critique behind the reset. Step 2's sources are drafted in the scratchpad while it runs, since no later step touches `api/` or `templates/`. Record each finding in §9 as `C.<n>`; a held finding changes step 1 in place, and its checks rerun.

### Step 2: 01 on the template's frame

- **Files:**
  - `examples/01_platformer/`:
    - `Cargo.toml`, `tungsten.json`, `input.json` (the movement, `jump` and `state_pause` actions; the rest join with their steps);
    - `src/`: every old file deleted, except `src/level_layout.rs`, which stays generator output until step 3 and compiles into nothing; new `main.rs`, `lib.rs`, `game.rs`, `save.rs`, `shared.rs` and `shell/`; `features/run/`, `features/level/` and `features/player/` with just the map's tile layers, the player prefab on the controller and the camera target;
    - `tests/game/main.rs` and `tests/game/seam.rs`;
    - `tools/generate.py`, `tools/sfx.py`, `tools/sounds.json`, `tools/outputs.json`, `tools/test_generate.py` (only where an owned output moved), `tools/README.md`, `tools/content/` and `assets/`;
  - the workspace `Cargo.toml` (the cargo-shear path at `:31`) and `Cargo.lock`;
  - `crates/tungsten-core/tests/manifests.rs`;
  - `scripts/check-repo.py` and `scripts/test-check-repo.py` (§2.10's check and route);
  - `scripts/smoke-examples.sh` and `scripts/test-smoke-examples.sh`;
  - `docs/LLM_INDEX.md`: drop the `examples/01_platformer/src/tests/regime.rs`, `src/setup.rs` and `src/systems.rs` paths from the rows on lines 15 and 23, and rewrite the row on line 78 for the new layout;
  - `README.md:39`, now `cd examples/01_platformer && cargo run`.
- **Change:** §2.3, §2.10, and from §2.8 the shell's assets, `tools/content/` and 01's `ui_*`, `game_over`, `music_title` and `music_over`. `save.rs` starts as the template's minimal capture (the meta, the player's position and health), which step 7 replaces.
- **Done-when:**
  - `diff -r templates/basic/src/shell examples/01_platformer/src/shell` → exit 0;
  - `python3 -B scripts/check-repo.py` → 0 errors;
  - `python3 -B scripts/test-check-repo.py` → OK, with tests for an equal copy, a changed byte, a missing file and an extra file;
  - `rg -c 'TUNGSTEN_|\.\./' examples/01_platformer/src` → no output;
  - `cargo test -p example-01-platformer --locked --test game -- seam::` → at least 3 pass: a New Game at Normal spawns the player on the map with a camera target; `capture` is `None` with no run and a save in one; `BINDABLE_ACTIONS` names only `input.json`'s actions and no `ui_*` or `engine_*`;
  - `cargo test -p tungsten-core --test manifests --locked` → passes;
  - `just level-check` → passes;
  - `just smoke` → passes, its 01 line marked `dir=examples/01_platformer`;
  - `just script-test` → passes;
  - `just ctx` → passes.
- **Moves:** 01's smoke row runs from its folder.

### Step 3: The level from the generator

- **Files:**
  - `examples/01_platformer/`:
    - `tools/level.py`, `generate.py`, `placement.py`, `level.json` (the `gate_step` corner), `test_generate.py`, `outputs.json`, `README.md` and `tools/content/`;
    - `assets/`;
    - `src/level_layout.rs` (deleted);
    - `src/features/level/` and `src/features/run/`;
    - `tests/game/main.rs` and `tests/game/level.rs`;
  - `scripts/check-repo.py` and `scripts/test-check-repo.py` (the selector loses `src/level_layout.rs`);
  - `docs/validation.md:24`;
  - `docs/LLM_INDEX.md` (the row on line 79).
- **Change:** §2.8's level placements. Spikes and fires carry `ContactDamage`, the fires on `Oscillator`s; platforms are kinematic bodies on ping-pong `PathFollower`s with sine easing; areas are `Zone` sensors named from `level.json`'s `landmarks`.
  - Widen `gate_step` with the smallest change that gives the player's 56 px at least 8 px of headroom over `gate_crest`, and record it.
  - Port the five `level` tests (§2.9). The route test drives the player prefab on the real map, holding `jump` through the rise so the jump cut never acts. Its `LAUNCHES` rows are retuned with its search helper where the controller's exact parabola needs it, and every route pair is listed, so a dead row fails (known issues, `:98`).
- **Done-when:**
  - `test ! -e examples/01_platformer/src/level_layout.rs && ! rg -q level_layout examples/01_platformer scripts docs/validation.md docs/LLM_INDEX.md` → exit 0;
  - `just level-check` → passes;
  - `jq '[.layers[] | select(.type=="objectgroup") | .name]' examples/01_platformer/assets/tilemaps/level.tmj` → the object layers of §2.8, listed in the evidence;
  - `cargo test -p example-01-platformer --locked --test game -- level::` → 5 pass, the route test printing every route pair landed on the controller, `gate_step -> gate_crest` among them;
  - `just smoke` → passes;
  - `python3 -B scripts/test-check-repo.py` → OK.
- **Moves:** none.

### Step 4: The player and the run

- **Files:**
  - `examples/01_platformer/`:
    - `src/features/{player,run,level,audio}/`, `src/game.rs` and `input.json` (`lantern`, `zoom_in`, `zoom_out`);
    - `tools/content/` (the player prefab's kit items: action input, controller with one air jump, aim, health, hurtbox, knockback, hit flash, cues, character animator, facing and camera target);
    - `tools/sfx.py`, `tools/sounds.json`, `tools/outputs.json`, `assets/`;
    - `tests/game/{main,player,camera,hazards,fixed_step,presentation,regime,areas}.rs`.
- **Change:**
  - §2.4's player, camera, clips, hits, areas and death; `music_explore` and `music_pause` with their crossfades.
  - The level's presentation (A11, A12): the parallax backdrops and the moon, the glows and their lights, the cursor and today's post stack (Bloom and Vignette), drawn through `Extracts::add_sprites` and default sprites, with seeds and phases from save IDs or map order, never entity IDs.
  - Death at HP 0 or past the kill row calls `shell::game_over`; entering an area calls `shell::request_autosave` and `shell::toast` with its name.
  - Port `player`, `camera` and `regime`, and of §2.9's other rows the tests that need no ball, brick, missile or black hole. That leaves out hazards' `moving_fire_sweeps_fast_balls_once_…` and `explosion_bursts_and_rings_stay_bounded_…`, fixed_step's `at_144_hz_sprites_and_drips_…` and `at_30_hz_both_spawners_stop_at_the_cap_…`, and presentation's `cloud_parallax_and_vortex_instances_…`, which steps 5 and 6 port.
  - The new `areas` test: walking across each area's start autosaves once and toasts its name, and a Retry starts in the last area.
- **Done-when:**
  - `cargo test -p example-01-platformer --locked --test game -- player:: camera:: hazards:: fixed_step:: presentation:: regime:: areas::` → passes: player 9, camera 3, hazards 5, fixed_step 3, presentation 9, regime 1, areas at least 1;
  - `rg -c 'KeyCode' examples/01_platformer/src --glob '!shell/**'` → no output;
  - `just level-check` → passes;
  - `just smoke` → passes.
- **Moves:** none.

### Step 5: The physics toys and the element rules

- **Files:**
  - `examples/01_platformer/`:
    - `src/features/{toys,elements,audio}/`;
    - `tools/content/` (the small ball, bowling ball, iron block, scrap, black hole and `fire_missile` prefabs, the missile with its `projectile` and `linear_motion`);
    - `tools/actors.py`, `tools/polish_art.py`, `tools/generate.py`, `tools/sfx.py`, `tools/sounds.json`, `tools/outputs.json`, `tools/test_generate.py`, `assets/`;
    - `tests/game/{main,ball_pit,spawning,burning,brick,ice,spells,hazards,fixed_step,presentation}.rs`.
- **Change:**
  - The toys as functions the spells call in step 6: place small balls, an iron block or a black hole at a point; toss a bowling ball; push from a point (force push and the blast); spray a cone (fire or cold) from a caster along an aim; the missile's flight against balls and iron, its blast and its bending. Plus their caps (`BALL_CAP`, `BRICK_CAP`, six missiles) and the element rules ported from today (A10, Q3), with §2.7's per-step presentation caps. The bowling ball's art and the toys' sounds come from §2.8.
  - Port every test of `ball_pit`, `spawning`, `burning`, `brick`, `ice` and `spells` that presses no spell trigger, driving the functions directly (§2.9). Also port the two fire-against-ball `hazards` tests, the 144 Hz ball draw test in `fixed_step` and the vortex test in `presentation`.
- **Done-when:**
  - `cargo test -p example-01-platformer --locked --test game -- ball_pit:: spawning:: burning:: brick:: ice:: spells:: hazards:: fixed_step:: presentation::` → passes: ball_pit 4, spawning 7, burning 6, brick 10, ice 5, spells 4, hazards 7, fixed_step 4, presentation 10;
  - `rg -c 'PER_FRAME' examples/01_platformer/src` → no output;
  - `just level-check` → passes;
  - `just smoke` → passes.
- **Moves:** none.

### Step 6: Spells, mana and the HUD

- **Files:**
  - `examples/01_platformer/`:
    - `src/features/spells/` (with `hud.rs`), `src/features/{player,toys,audio}/`, `src/game.rs` and `input.json` (`cast`, `spell_next`, `spell_prev`, `spell_1`–`spell_0`);
    - `tools/content/` (the `stats`, `tags`, `effects` and `abilities` data);
    - `tools/palette.json`, `tools/actors.py`, `tools/art.py`, `tools/effects.py`, `tools/generate.py`, `tools/sfx.py`, `tools/sounds.json`, `tools/outputs.json`, `tools/test_generate.py`, `assets/`;
    - `tests/game/{main,spells,ice,ball_pit,spawning,fixed_step,hud}.rs`.
- **Change:**
  - §2.5: eight abilities with manual triggers over step 5's functions, the mana stats and the two effects, slot selection, held spells, the HUD, the spell sounds, and from §2.8 the icons, the toss clip, the aim frames and the lantern's cast.
  - Port the tests that press a trigger: ice's ten that press F, now `cast` with slot 3; spells' Mouse 4 test, now slot 1; ball_pit's two MMB tests, now slot 9; spawning's right-click test, now slot 6; and fixed_step's spawners-at-the-cap test. Port the HP bar as `hud`.
  - One new test in `spells`: mana regenerates to its max, and a cast pauses it for about a second.
- **Done-when:**
  - `cargo test -p example-01-platformer --locked --test game -- spells:: ice:: ball_pit:: spawning:: fixed_step:: hud::` → passes: spells 6, ice 15, ball_pit 6, spawning 8, fixed_step 5, hud 1;
  - `jq '[.sprites | keys[] | select(test("icon"))] | length' examples/01_platformer/assets/manifest.json` → 8;
  - `just level-check` → passes;
  - `just smoke` → passes.
- **Moves:** none.

### Step 7: Saves

- **Files:** `examples/01_platformer/src/save.rs`, `src/shared.rs`, `src/features/**` (save IDs at spawn, the restore paths) and `tests/game/{main,saves}.rs`.
- **Change:** §2.6.
- **Done-when:**
  - `cargo test -p example-01-platformer --locked --test game -- saves::` → passes: round-trip with a hazard and a platform displaced, coverage per entity kind, the same-frame cast, the save during `casting`, divergence from a fresh harness and from a churned world, migration refusals and the size limit;
  - `cargo test --release -p example-01-platformer --locked --test game -- saves::pit_full --nocapture` → prints the size, at most 2 MiB, and the capture-plus-write time, both quoted in the evidence;
  - `just smoke` → passes.
- **Moves:** none.

### Step 8: The playthrough and the README

- **Files:** `examples/01_platformer/tests/game/{main,playthrough}.rs`, `examples/01_platformer/README.md` and `examples/01_platformer/tools/README.md`.
- **Change:** `playthrough::` plays a New Game at Normal on the full game, headless at dt 1/60.
  - Scripted moves and jumps walk the `lower` route from the spawn to the tower, then on into the collection pit.
  - It asserts each area's toast in order, the player alive at the end, gameplay on top, and one autosave per area.
  - The README maps each mechanic to its kit module and data file, and says how to run and test from the folder.
- **Done-when:**
  - `cargo test -p example-01-platformer --locked --test game -- playthrough::` → passes;
  - `cargo test -p example-01-platformer --locked` → passes. `python3 -B -c "import subprocess; out=subprocess.run(['cargo','test','-p','example-01-platformer','--locked','--test','game','--','--list'],capture_output=True,text=True,check=True).stdout; print(sum(l.endswith(': test') for l in out.splitlines()))"` → at least 103: the 92 ports and the new tests of steps 2–8;
  - `rg -c 'tungsten::kit|kit::' examples/01_platformer/README.md` → at least 10;
  - `just smoke` → passes.
- **Moves:** none.

### Step 9: Running from its folder in the archives

- **Files:** `scripts/release.py`, `scripts/test-release.py`, `tools/launcher/src/main.rs`, `docs/releases.md` (`:114`), `scripts/check-repo.py` and `scripts/test-check-repo.py` (the selector routes `tools/launcher/` to `cargo test -p tungsten-launcher --locked`).
- **Change:** §2.10's archives.
- **Done-when:**
  - `python3 -B scripts/test-release.py` → OK, with a folder-game example in the fixture;
  - `cargo test -p tungsten-launcher --locked` → passes, `Platformer` mapping to `examples/01_platformer`;
  - `just script-test` → passes;
  - local Linux rehearsal, once `df -h /` shows at least 20 GB free:
    1. `R=target/m72-rehearsal`;
    2. `RUSTFLAGS="-C target-cpu=x86-64" cargo build --release --locked --workspace --exclude tungsten-crash-probe --exclude example-03-fixtures --bins --target x86_64-unknown-linux-gnu --target-dir $R/x86-64`;
    3. `python3 -B scripts/release.py licenses x86_64-unknown-linux-gnu --out $R/licenses.json`;
    4. `python3 -B scripts/release.py package v0.93.0 x86_64-unknown-linux-gnu --bin-dir $R --out $R/dist`;
    5. unzip into `$R/unzip`;
    6. `TUNGSTEN_SMOKE_FRAMES=3 $R/unzip/tungsten-examples-0.93.0-linux-x64/Platformer` → exit 0, with `data/examples/01_platformer/{tungsten.json,input.json,assets/manifest.json}` in the zip;
    7. then `python3 -B -c "import shutil; shutil.rmtree('target/m72-rehearsal')"`.
- **Moves:** the archive's Platformer reads its files from `data/examples/01_platformer/`.

### Step 10: Records

- **Files:** `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`, `DESIGN.md`, `docs/known-issues.md`, `docs/plans/1.0/backlog-1.x.md`, `docs/plans/1.0/w12-template.md`, `docs/plans/1.0/w01-ui-text-suite.md`, `docs/plans/1.0/w13-kit.md`, `docs/plans/1.0/example-01-design.md`, `docs/LLM_INDEX.md`.
- **Change:**
  - **Decision** 197 (§5), with tungsten-decision, before the docs that cite it.
  - **Ledger:** one row, M72's core and kit API for the freeze (additions only).
  - **Design:**
    - `DESIGN.md:371`: 01 runs from its own folder;
    - a paragraph in the kit section: saved runtime state, a loaded controller's still first step, restore and suppression;
    - a line in the template shell section: requested autosaves, captured in `PreUpdate` after the previous frame's flush.
  - **Known issues:**
    - `:122` (`gate_step`) closed;
    - `:16`: the generator merges slab runs;
    - `:98`: the route test's rows are checked;
    - `:112` and `:113`: closed or moved to the new files;
    - `:57`: air jumps added; a lit player shows no flash (Q6).
  - **Backlog:** the platformer row loses the air jump, and new rows are added for the `place` payload, the animator's one-shots, `Lifetime` in saves and the pad's aim.
  - **W12:** step 3's W12b "Landed in 0.93". **W1:** §11's M6 "Landed in 0.93". **w13:** §7 gains the kit additions.
  - **Example 01's design:** its (a) section notes where this plan's defaults departed (Q2–Q6, Q11).
  - **The LLM index:** final rows for 01's layout and tests.
- **Done-when:**
  - `rg -c '^## D-19[7] — ' DECISIONS.md` → 1;
  - `rg -c '^\*\*Amended by D-19[7]:' DECISIONS.md` → 6;
  - ``rg -c 'Amended by `D-19[7]`' docs/DECISION_INDEX.md`` → 6;
  - `cargo test -p tungsten-core --test decision_index --locked` → passes;
  - `rg -c 'D-19[7]' docs/plans/1.0/w04-api-freeze.md` → at least 1;
  - `rg -c 'gate_step' docs/known-issues.md` → no output;
  - `rg -c 'until 01 \(a\) gives it its own' DESIGN.md` → no output;
  - `rg -c 'Landed in 0.93' docs/plans/1.0/w12-template.md docs/plans/1.0/w01-ui-text-suite.md` → 1 each;
  - `just ctx` → passes;
  - `just repo-check` → passes.
- **Moves:** none.

### Step 11: Release 0.93

- **Files:** `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row, the README's four owned Now fields (`roadmap.py summary`), this plan (moved to `docs/plans/archive/`).
- **Change:**
  - Close workflow §5 and this step's evidence row with `status: done`.
  - Fill the register row (release 0.93, "Cut <date> as v0.93.0") before the cut.
  - Then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.93.0` once, and every release check once after it. Add any of `just level-check`, `just visual`, the kit bench's effect rows and the rehearsal that no step passed. Then the preflight with `--message`.
- **Done-when:** [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut); `just release-preflight 0.93.0 --repo JosephGibson/Tungsten --message 'Update 0.93: M72 example 01 rebuilt on the template'` → passes and prints the command block and the post-merge block.
- **Ends with:** the changed-file list, the command block, the post-merge block, then the plan prompt for 01 (b) elements and weapons.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Bring the air jump back as a kit step, or keep the backlog row and drop double jump (design, Controls)? | A kit step in step 1 (`with_air_jumps`), since the controls table keeps "Jump, double jump" and today's game has it |
| Q2 | The design moves the fire hazards onto the oscillator in (b); a rebuild from scratch has no sine code to keep | (a) puts them on `Oscillator`, since writing sine motion again would copy a kit feature (W12b's done-when) |
| Q3 | The element rules (A10) on kit effects now, or as game code until (b)? | Game code ported with its tests, the reason recorded in §2.4; (b) moves burning and freezing onto damage types, effects and the ailment ladder |
| Q4 | Black hole: placed at the cursor for a lifetime, or today's drag while held? | Placed at the cursor for 4 s, one at a time (design: "placed at the cursor"); the drag test retired |
| Q5 | Placing at the cursor (iron block, black hole, small balls): a kit payload now, or 01's code after `use_ability`? | 01 places through `prepare_prefab` once a manual `use_ability` returns `Used::Used`; a backlog row records a `place` payload at the aim's point, which (b)'s plan may take as a kit step if summons need it in data |
| Q6 | The design puts the player on the lit path, and a lit sprite never flashes (A24) | Lit, as designed: knockback, the window and shake still show a hit, the flash does not. Known issues records it, and (b)'s plan, which needs flashes on the player and the dummy, decides an engine fix |
| Q7 | How does the coverage test list an entity's component types (A25)? | `World::component_types` in core (step 1), one read-only method no frame calls, with its ledger row |
| Q8 | `KitRng`'s save: replay from `{seed, draws}`, or serde on core's `Pcg32`? | Serde on `Pcg32`, so a restore costs the same whatever the draws: the title reads all eleven slots whole (A35), and a replay would cost each slot its draws again (round 1, finding 6) |
| Q9 | 01's identity and window | `game.id` `tungsten-platformer`, title "Platformer", 1280 × 720 |
| Q10 | Copy the shell's tests into 01? | No: the template tests the same bytes |
| Q11 | The save's limits with the pit full | 2 MiB asserted; the time measured in release against 50 ms and recorded |
| Q12 | Divergence tolerances | §2.6 |
| Q13 | Music lengths | Title and exploration about 30 s, pause and game over about 15 s, at 22,050 Hz |
| Q14 | Spell values | §2.5's starting values, tuned in the run and recorded |
| Q15 | The owner's art review | Notes the owner writes in this row before pasting the run prompt override the sheet; otherwise the sheet stands |
| Q16 | The CI rehearsal prerelease that W12b's done-when names | The owner's, as at M71; the run does step 9's local Linux rehearsal |
| Q17 | Trim the root `assets/` and `input.json` of what only 01 read (A1)? | Not here: a follow-up for the Phase 6 QA pass |
| Q18 | Where does the shell capture its due autosaves? | In `PreUpdate`, before the state dispatcher, after the previous frame's command flush, so a save holds a cast's cost and its spawn together (A34; round 1, finding 1) |
| Q19 | The design's deferred cascade queue in (a)? | Not in (a): today's two caps sample presentation (A9), so the queue, its full-pit bound test and its place in saves land with (b)'s explosions (round 1, findings 7 and 8) |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`).

## 5. Decisions expected

197: M72 landed (01 (a)):
- example 01 on the template from its own folder, with its shell held to the template's by `just repo-check`;
- air jumps on the platformer controller;
- the restore payload and suppression on effects;
- saved runtime state for the controller (filter and first step left out), the path follower and `KitRng` (its generator's words, through `Pcg32`'s serde);
- `World::component_types`;
- the template's due autosaves captured in `PreUpdate` after the previous frame's flush, a request waiting until a capture succeeds;
- the archives' folder games.

It amends six entries:
- `D-123`: the examples' working folder;
- `D-158`: 01's player on the controller, and air jumps;
- `D-173`: `KitRng` saves, through `Pcg32`'s serde;
- `D-177`: restore and suppression;
- `D-181`: the shell's due autosaves captured in `PreUpdate`, after the previous frame's command flush;
- `D-196`: `Platformer` under `FOLDERS`.

`D-194`'s pending autosave lands as written. Next free when written; checked again at step 10.

## 6. Invariants

- **May move:**
  - `api/tungsten-core.txt`, `api/tungsten-kit.txt` and `api/tungsten-kit-components.txt` gain lines (step 1);
  - the kit hash moves only if a step 1 fixture must join a genre's scene, with the new value and reason in the evidence and the `CHANGELOG.md` line;
  - 01's smoke row runs from its folder (step 2);
  - the archive's Platformer files move under `data/examples/01_platformer/` (step 9).
- **Must not move:**
  - the determinism hash, the pinned containment hash and the row digests (no physics, ECS-storage or 02 change);
  - `gpu-visual.png`, the post and transition regressions and the two UI fixture PNGs;
  - the template's title pixels (`visual::`), and its 132 existing tests, which still pass, edited only where an autosave now lands a frame later (§2.2);
  - `DefaultPlugins`' resolved schedule and the kit's coverage test.

## 7. Stop conditions

- The skeleton's default (restore the step's files, mark it skipped, continue with steps that don't depend on it) fits no step here. Each step stands on the ones before it, and the release needs them all: without step 9, the archive's Platformer would read the root's files from `data/`. So:
  - **Step 1:** a check that cannot pass after fixes inside its files restores step 1's files from their copies and ends the run before step 2. A release session follows after the owner has seen why (workflow §3).
  - **Steps 2–10:** a check that cannot pass after fixes inside the step's files ends the run before the release. Examples: a route pair the controller cannot land after the widening, or a kit limit that only a file outside the list could fix. The tree is kept, and the evidence row marks the step failed with the blocker. A `resume` or a release session follows once the owner has seen it. The release never ships a half-rebuilt 01.
- **Step 9's rehearsal:** if it cannot run (under 20 GB free, no display), it is recorded as not run and becomes the owner's, with the CI rehearsal. A failure in the scripts is fixed inside step 9's files.
- **The kit bench:** an effect row outside the per-entity bound fails step 1.
- **Template budgets:** a shell or `src` line cap, or `AGENTS.md`'s bytes, over its limit after M69's compaction shapes fails step 1.
- **Pillow:** a failed install ends the run before step 2 as an environment stop.
- **Captures:** the kit bench never runs while `pgrep -x nxcodec.bin` or the bench runner shows a process.
- **Scope:** a needed file outside §files to touch is recorded, and the step stops.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1–9, release | Step 1: the kit, core, umbrella and template tests, `just api-check`, the kit bench's effect rows (a criterion capture), `just visual` and `just smoke`. Steps 2–8: 01's tests, `just level-check`, the manifests test and smoke. Step 9: `just script-test`, the launcher's tests and the rehearsal. The release runs `just check`, `just smoke`, `just script-test`, `just template-check` and the rest of the chain once, after the cut. No CPU row is captured: `World::component_types` is a read-only method no frame calls, its file's hot paths are unchanged (M61's `World::check_prefab_id` precedent), and no physics, storage or frame-loop code changes |
| 3 Invariants | 1, 11 | §6 |
| 4 Decisions | 10 | 197 |
| 5 API (break ledger, rustdoc, `just api`) | 1, 10 | Rustdoc on every new item; `just api` in step 1; the additions row in step 10 |
| 6 Template and guide | 1 | The pending autosave, `AGENTS.md` and `docs/getting-started.md` |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 2, 3, 10 | Rows for the deleted and new paths, within the 12,288-byte budget (12,007 at `22de917`) |
| 8 Design | 10 | `DESIGN.md`: 01's folder, the kit's saves, the requested autosave |
| 9 Records | 10, release | Known issues, backlog, W12, w01 §11, w13, the design's (a) notes, the register row (plan cell at this plan's session, release cell before the cut), the README's four owned Now fields |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

Round 1: the `critique` skill with `--repo --force --effort max` (level C), `gpt-6-astra`, 2026-10-20, read at `22de917`, the plan cut of §9 and the log with `sed`. Context 872,000; output cap 4,000; 665.2 s; 1,448,195 tokens in (1,310,080 cached), 20,210 out (13,508 reasoning); 46 commands, 361.1 KB output (largest 28.4 KB); 5h window 50% → 87%, week 10% → 16%. Eight findings, all checked and accepted.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] Autosaves can record a cast's cost without its spawned missile: `shell_frame` captures in `Update`, before the frame's command flush | `crates/tungsten/src/app.rs:1748-1757` (flush after `PostUpdate`), `templates/basic/src/shell/mod.rs:208-210`, `crates/tungsten-kit/src/intent/ability.rs:1430-1437` | Accepted | A34; §2.2: the due autosaves move to `shell_save` at the start of `PreUpdate`; §2.6's same-frame test; Q18; §5 amends `D-181` |
| 2 | [HIGH] Loading moving hazards restores their clocks but loses their positions | `crates/tungsten-kit/src/motion/oscillator.rs:620-632`, `motion/drive.rs:114-119` | Accepted | A36; §2.6: hazards save their position; the round-trip saves with a hazard and a platform displaced |
| 3 | [HIGH] The player save loses the tag that suppresses mana regeneration | `crates/tungsten-kit/src/damage/effects.rs:1225-1232`, `:1782-1790` | Accepted | A36; §2.6: the player saves `Tags`; the casting test; coverage held per entity kind |
| 4 | [HIGH] Step 5 requires behaviour that step 6 has not implemented (the ice tests need bricks and scraps) | `examples/01_platformer/src/tests/ice.rs:359`; the ported tests' dependencies (A13 and the test audit) | Accepted | The toys and elements (now step 5) come before the spells (now step 6); each test lands where everything it touches exists (§2.9), including two `hazards` tests, one `fixed_step` and one `presentation` test that step 4 also could not have passed |
| 5 | [MEDIUM] The restore payload's fallback to a stat's own bound needs access that step 1's files cannot provide | `crates/tungsten-kit/src/foundation/stats.rs:54-62`, `:347-363` | Accepted | §2.1: `max` is required; no stats change |
| 6 | [MEDIUM] Replaying `KitRng` can stall the title, which loads all eleven slots whole | `templates/basic/src/shell/mod.rs:398-410`, `src/shell/title.rs:86-90` | Accepted | A35; §2.1: core `Pcg32` gains serde and `KitRng` saves its words; Q8's default changed; step 1's files and checks |
| 7 | [MEDIUM] Queueing the extinguishing turns a visual cap into delayed extinguishing | `examples/01_platformer/src/systems.rs:938-952` | Accepted | A9 corrected; §2.7: extinguishing stays immediate on the fixed step, and the caps stay presentation samples |
| 8 | [MEDIUM] The queue bound lacks the design's full-pit test | `docs/plans/1.0/example-01-design.md:52-55` | Accepted | With finding 7, (a) has no gameplay chain to defer: the queue, its bound and test, and its save move to (b) (Q19, Follow-ups) |

Round 2: not run. Round 1 changed the step order and done-when checks, so step 7 called for a second round at xhigh. With the 5h window at 87% (reset 19:37 UTC), it was queued behind the reset, and the owner then asked to continue without it (2026-10-20). Round 1's fixes, and the proofread changes listed after them in the focus note (§2.3, §2.6's Identity and Load, steps 2, 4 and 10, §7, Follow-ups), have had no second critic's read. No finding is left open.

API critique (level C, after step 1): the skill's `critique --raw --repo --force` over `git diff HEAD -- api/ templates/` and the untracked files there, `gpt-6-astra` at xhigh, 2026-10-20 19:39 UTC, queued behind the 19:37 reset. Context 872,000; output cap 4,000; 167.8 s; 577,889 tokens in (507,904 cached), 5,055 out (2,651 reasoning); 17 commands, 250.3 KB output (largest 35.5 KB); 5h window 0% → 15%, week 16% → 18%. No findings: the critic checked the pending autosaves, failed captures, exit handling, the order of the old run's capture, the incoming run's metadata and the dispatch, the new declarations, the save forms' checks and the effect and controller behaviour against their tests, and found nothing to change. It ran no builds or tests (read-only). Step 1 stands as run.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| C.1 | None: "No substantive concern emerged" | the critique's summary | — | Nothing to change; step 1's checks stand |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-20 | Passed | Core `ecs::world::tests:: rng::tests::` 67 passed, `component_types_lists_an_entitys_components` and `a_saved_pcg32_continues_its_stream` (an even `inc` refused) among them. Kit filters 89 passed, the six named tests among them, `describe` counts 40/40/100. Umbrella `kit_tests:: kit_recipes::` 159 passed: the three new recipes (platformer mana a cast pauses, top-down stamina sprinting pauses, an air jump onto a ledge), `KIT_RECIPES_HASH` unchanged, coverage unchanged. Template `--test game` 134 passed, `a_first_autosave_waits_for_a_capture` and `a_requested_autosave_waits_for_a_capture` among them; `sample::`'s schedule snapshot gains `shell_save` before `state_dispatcher`. tokei: shell 2,393 ≤ 2,400, `src` 2,890 ≤ 2,900; `AGENTS.md` 12,270 bytes. `shell::request_autosave`: 2 in `AGENTS.md`, 2 in the guide. `just ctx` OK. `just api`: the seven `rg` counts 6, 6, 6, 8, 2, 2, 3; every `HEAD` line kept (`[]`); `api/` +74 lines over the three snapshots, none removed. `just api-check` OK. `just lint` exit 0; `cargo fmt --all -- --check` exit 0. Kit bench, one foreground sitting after 300 s quiet (`pgrep` empty): `effect_tick` 82.68 µs at 1,000, 808.0 µs at 10,000 (per entity ×0.98); `effect_resolve` 438.2 µs and 4.581 ms (×1.05), both within the ×2 bound. Criterion printed +20.1% (tick) and +8 to +9% (resolve) against the baseline an earlier sitting left in `target/criterion-native`, which the bound does not read. `just visual` exit 0 (4, 5, 1, 6 and 1 passed). `just smoke` OK (3/3 examples, template 1/1, every matrix row). Interpretations: a `restore`'s names are checked when the data parses (the stats table is private); its amount takes a heal's factor (no stacks); `shell_save` runs last in `PreUpdate`, after the UI systems, before the dispatcher; the run a `goto` or `start_run` stages waits in `Shell` until `shell_save` has saved the old one; the settings' exit save stays in `shell_frame`, so the confirm's answer lands first. API critique: no findings (§9) | `crates/tungsten-core/src/{rng.rs,ecs/{world,archetype}.rs}`, `crates/tungsten-kit/src/{damage/effects.rs,motion/{platformer,path}.rs,foundation/rng.rs}`, `templates/basic/src/shell/mod.rs`, `templates/basic/tests/game/{shell,sample}.rs`, `api/` |
| 2 | 2026-10-20 | Passed | Drafted with step 3 in a scratch copy under `target/` while the critique waited, then copied into the tree; both steps' checks ran in the tree on step 3's state. `diff -r templates/basic/src/shell examples/01_platformer/src/shell` exit 0. `check-repo.py` 0 errors (78 notes), after the LLM index's 01 row took full paths: its bare `game.rs`, `save.rs` and `shell/` did not resolve. `test-check-repo.py` 54 tests OK, `test_example_shells_copy_the_templates_byte_for_byte` (an equal copy, a changed byte, a missing file, an extra file) and `test_an_example_shell_selects_the_copy_check` among them. `rg -c 'TUNGSTEN_\|\.\./' examples/01_platformer/src` no output. `seam::` 3 passed. `manifests` 5 passed. `just level-check`: 675 owned outputs match, 15 Python tests OK. `just smoke` exit 0: examples 3/3 and every row; 01's log watches `examples/01_platformer` for hot reload, so it ran from its folder. `just script-test` exit 0, the smoke stub's 01 line ending `dir=examples/01_platformer`. `just ctx` OK. `cargo fmt --all -- --check` exit 0 after `rustfmt` on two draft files; `just lint` exit 0. The shell's assets come from the template (theme, Inter and Source Serif 4 with licences, `damage_flash`); 01's `ui_move`, `ui_select`, `ui_back`, `game_over`, `music_title` and `music_over` are synthesized | `examples/01_platformer/{Cargo.toml,tungsten.json,input.json,src/,tests/game/,tools/,assets/}`, `Cargo.toml`, `Cargo.lock`, `crates/tungsten-core/tests/manifests.rs`, `scripts/{check-repo,test-check-repo}.py`, `scripts/{smoke-examples,test-smoke-examples}.sh`, `docs/LLM_INDEX.md`, `README.md` |
| 3 | 2026-10-20 | Passed | `level_layout.rs` gone and no `level_layout` left in 01, `scripts/`, `docs/validation.md` or the index (exit 0). Object layers: `["props","slabs","hazards","platforms","areas","emitters","markers"]`, with 260, 27, 12, 3, 4, 10 and 1 objects (the 27 slab colliders are the merged runs). `level::` 5 passed; the route test printed 54 route pairs "landed on the controller", `summit_steps: gate_step -> gate_crest` among them. The `gate_step` corner: `overlook_a` loses its last column (`right` 121 → 120), so its slab (underside y 983) ends at x 7680, a tile short of `gate_crest`'s edge at x 7744: nothing overhangs `gate_crest` now, where 41 px did before for the player's 56. Three `LAUNCHES` rows retuned with the search helper: `overlook_a → overlook_b` start 119.85, `gate → gate_step` 118.85, `gate_step → gate_crest` 120.85. `just level-check`, `just smoke` and `test-check-repo.py` as in step 2's row | `examples/01_platformer/tools/{level.py,generate.py,placement.py,level.json,test_generate.py,outputs.json,README.md}`, `examples/01_platformer/assets/`, `examples/01_platformer/src/features/{level,run}/`, `examples/01_platformer/tests/game/{main,level}.rs`, `docs/validation.md` |
| 4 | 2026-10-20 | Passed | `--test game -- player:: camera:: hazards:: fixed_step:: presentation:: regime:: areas::` → 31 passed: player 9, camera 3, hazards 5, fixed_step 3, presentation 9, regime 1, areas 1; the whole `--test game` 39 passed. `rg -c 'KeyCode' examples/01_platformer/src --glob '!shell/**'` no output. `just level-check`: 798 owned outputs match (675 at step 3: the mirrored courier and fireball frames with their maps, the lantern anchors, two loops, the drips' prefab), 15 Python tests OK. `just smoke` exit 0 (examples 3/3, every row; 01's log has no warning past the user folder's). `cargo fmt --all -- --check` and `just lint` exit 0. Outside step 4's list but inside the header's `examples/01_platformer/**`: `tools/generate.py` (left-facing frames and clips, since nothing flips a sprite, and the `lantern_anchors` data kind), `tools/placement.py` (the anchors of mirrored frames) and `tools/level.py` (glows with a light strength and a fireball's core, the fire trails, the drips as `spawn_children`, and the fireballs' `facing` by motion, or by `aim` at `team.player` for the one that only rises and falls, through a `character_animator` of left and right clips). Deviations from §2.3: the backdrop (sky, moon, strips) draws in a replaced sprite base, the backdrop then the engine's default tilemaps and sprites, since `add_sprites` draws after the defaults; the glows draw as default sprites at z −8 and −7, over the world's props and under the hazards and the player, as today's did, each with a point light; only the cursor goes through `add_sprites`. The default order puts the foreground tiles under the sprites, which today's extract drew over the player; the ported order test pins the new order. Ports that moved: the 144 Hz tap's release cuts its jump (the kit's `jump_cut`), so it asserts the jump and a rise past 0.4 of its speed; contact damage hits a new contact at once and a held one each 0.5 s, so the three-touch test leaves the spikes between touches; gameplay's `update` sees a death at the next frame's dispatch, so death tests step two frames before settling; a step's velocity is its mean, −1,280 + g·dt/2. The kill row and the box deal a periodic hit of all the player's health, which passes the window; a dead player's controller goes and its body holds. A body set down astride two areas keeps its current one; step 7's save names it. The particle budget stays today's 2,048; bursts seed from a run counter and glows phase from save IDs | `examples/01_platformer/{input.json,src/game.rs,src/shared.rs,src/features/{player,run,level,audio}/,tests/game/{main,player,camera,hazards,fixed_step,presentation,regime,areas}.rs,tools/{content/,generate.py,placement.py,level.py,sfx.py,sounds.json,outputs.json},assets/}` |
| 5 | 2026-10-20 | Passed | `--test game -- ball_pit:: spawning:: burning:: brick:: ice:: spells:: hazards:: fixed_step:: presentation::` → 57 passed: ball_pit 4, spawning 7, burning 6, brick 10, ice 5, spells 4, hazards 7, fixed_step 4, presentation 10; the whole `--test game` 79 passed. `rg -c 'PER_FRAME' examples/01_platformer/src` no output. `just level-check`: 847 owned outputs match (the bowling ball's six frames with maps, four sounds, two particle configs, the toys' prefabs and `cue_rules/sounds.json`), 15 Python tests OK. `just smoke` exit 0 (examples 3/3, every row; 01's log clean). `cargo fmt --all -- --check` and `just lint` exit 0. The toys are functions in `toys` (`place_small_balls`, `toss_bowling_ball`, `place_iron`, `place_black_hole`, `fire_missile`, `push`, `force_push`, `explode`, `spray`, `end_sprays`), each saved entity taking a `SaveId` at spawn; ties order by save ID, never entity (A12). Interpretations: the bowling ball (radius 15, mass 8) takes today's orb's place in the rules, a fireball bursting it and iron leaving it whole; a crush on the player is a hazard's 34 per 600 px/s past 360, with a push; the missile is the kit's projectile on its linear motion (range 1,890 px, 360 px/s² fall), 01's contacts with bodies and tiles running before the kit's step, the kit's wall or range ending the rest, a range end fizzling; one-shot sounds are game events (`Sounded`) brought in by `CuesOnEvent` and played by `cue_rules/sounds.json` about the player, `min_interval` throttling crush 0.1 s, douse 0.15 s, freeze and shatter 0.1 s and place 0.05 s; spray loops are voices; bursts and shock rings cap at 16 each, impact sparks and steam puffs at 4 a step. Deviations: the balls, iron (four quarters and their frost as children), scraps (a frost child) and the missile (turned along its flight, a mirrored clip leftward) draw as default sprites; the flames, shock rings, the vortex with its streaks, the spray wisps and the cold pulses draw through `add_sprites`, so the vortex's under-layers now draw over the particles; the systems that place or colour newborn particles (sprays, holes, the fire pool, impact sparks) run after `physics_sync` and before `particle_count_refresh`, since the present phase follows the particles' tick. Outside step 5's list but inside the header's: `src/game.rs` (the two plugins and a `CursorPlugin` added last, so the cursor still draws last), `src/shared.rs` (`despawn_tree`), `src/features/level/{draw,components,systems}.rs` (shared quad helpers; a glow with no halo and its light's `reach`), `tools/level.py` (each fireball's trail moved to a child at its centre: step 4 put the emitter on the sprite's top-left; its two fire tests follow the child). Ports that moved: tests that leaned on a bare world hold their bodies on floors in the level's empty sky, despawn their balls before waiting out bursts, and count only their own particles, bursts and rings | `examples/01_platformer/{src/features/{toys,elements,audio}/,src/{game,shared}.rs,src/features/level/,tests/game/{main,ball_pit,spawning,burning,brick,ice,spells,hazards,fixed_step,presentation}.rs,tools/{actors.py,generate.py,level.py,sfx.py,sounds.json,outputs.json,content/},assets/}` |
| 6 | 2026-10-20 | Passed | `--test game -- spells:: ice:: ball_pit:: spawning:: fixed_step:: hud::` → 41 passed: spells 6, ice 15, ball_pit 6, spawning 8, fixed_step 5, hud 1; the whole `--test game` 96 passed. `jq '[.sprites | keys[] | select(test("icon"))] | length'` → 8. `just level-check`: 1,183 owned outputs match (847 at step 5: the eight icons, the toss and aim clips both ways with their maps, 86 cast twins with theirs, the force push's sound, the spells' data), 15 Python tests OK. `just smoke` exit 0 (examples 3/3, every row; 01's log has no warning). `cargo fmt --all -- --check` and `just lint` exit 0. §2.5's starting values stand untuned: costs 8, 20/s, 18/s, 25, 30, 15, 12 and 15/s; cooldowns 0.25 s, 2 s, 1 s, 0.4 s (3 charges, one back every 2 s) and 0.6 s; mana 100 of 100, 15/s, paused 1 s by `casting`. The spells are the player's kit abilities with manual triggers (`data/abilities/*.json`), their payload `casting`, and 01's toy after a `Used::Used`; the missile alone flies from the kit's `spawn` payload. Interpretations: a held spell is one use a fixed step, its data's cost its rate over the 60 Hz step (0.3333, 0.3, 0.25); the missile lands at the frame's flush and `missile_adopt` (`PreUpdate`, before `shell_save`) gives it its save ID and its 360 px/s² fall, which the payload's motion drops, draws it at 40 px and moves its centre onto the courier's centre plus the offset, since the payload starts from the courier's point, its art's top-left (the prefab spawns hidden until then); a cap refuses before anything is spent (six missiles, the ball cap), and the iron block and the hole need the cursor; slots 4 and 0 cannot be selected, and the wheel skips them; facing stays the movement's, while a cast's 0.3 s pose (standing in the idle clip, the aim frame nearest the aim, five drawn facing right and their mirrors) and the toss clip face the aim; every courier frame has a cast twin, its glass at the hot core and the frame round it lit so its emissive mask widens, shown 0.3 s after a cast (the lit path takes no per-sprite emissive strength); a spray shows from a press between two steps, its loop at once and its start sound with the next step's cues, and ends in the frame of a release, a death or the player's loss; the mana shows floored; the icons draw at the largest whole-number pixel scale within the UI's (1× at 1.75). Ports that moved: the orbs are gone, so the rate test counts 49, 52, 52 and 5 small balls (19, 20, 20 and 2 steps; a frame runs at most two); a release's end sound comes a step after its loop stops; the long hold's death ends the spray at once, and the drain moved to the last channel's release, since game over pauses the clock; the shatter's scraps are measured from the block's settled centre; the hearts test reads the health bar's fill and text at three zooms; the F tests stand the courier on a floor in the sky with the camera held, and the long hold has 1,000 mana. A GPU capture of a run from a scratch binary (deleted) checked the HUD and found three things, each fixed in `hud.rs`: a mesh's own box takes the panel style and hid the icons (cleared); a mono readout wrapped at its measured width (a minimum width, as the shell's readouts take); the shell's toasts rise from the bottom centre (the bar sits above that lane). Outside step 6's list but inside the header's: `src/shared.rs` (`pressed`), `src/features/mod.rs` and `src/features/run/mod.rs` (the HUD starts and stops with gameplay) | `examples/01_platformer/{input.json,src/{game,shared}.rs,src/features/{mod.rs,spells/,player/,toys/,audio/,run/mod.rs},tests/game/{main,spells,ice,ball_pit,spawning,fixed_step,hud}.rs,tools/{palette.json,actors.py,art.py,effects.py,generate.py,sfx.py,sounds.json,outputs.json,content/},assets/}` |
| 7 | 2026-10-20 | Passed | `--test game -- saves::` → 8 passed: the round-trip (a fire hazard and a platform off their spawns, a missile in flight, a hole, scraps, a chilled block and burning balls; capture, load into a fresh harness, capture: equal as JSON), coverage, the same-frame cast (the autosave names the Broken aqueduct, holds mana 92 and the missile), the save during `casting` (loaded with the tag, mana held 30 steps, then regenerating), divergence from a fresh harness and from a world that spawned and despawned 10,000 entities (both: 553 save IDs alive and equal, 234 burning, 1 frozen; player, platforms, hazards and iron 0.00 px apart; mana 64.69972 in each), the refused schemas (`migrate` 1–3, and sample envelopes on disk read as damaged) and the full pit; the whole `--test game` 104 passed. `cargo test --release … saves::pit_full --nocapture` → `pit_full: 12000 balls, 1865599 bytes (1.78 MiB), capture and write 4.9 ms` (the 50 ms target met; 14.3 ms in debug). `just smoke` exit 0 (examples 3/3, every row; 01's log has no warning). `cargo fmt --all -- --check` and `just lint` exit 0. `SaveGame` schema 4 holds §2.6's fields; `restore` writes each saved state over the map object with its save ID and spawns the dynamic ones from their prefabs, `Position` and `PrevPosition` together; the area a save names is the run's on entering, so it does not toast or autosave again. Deviations: each ball is one flat array, `[id, small, x, y, vx, vy, hue, hue speed, burn left]`, since the slots write pretty JSON (the struct form measured 3.40 MiB, an array with its hue and burn nested 2.24 MiB); a loaded missile spawns at the load frame's flush from its saved source, so the kit's `Source` and the payload's crate-private emitter return; coverage holds a ninth kind, the map's still objects (props, slabs, spikes, areas, emitters, the marker), all but the save ID rebuilt; its scripted run is twelve seconds, not a minute (every spell cast, iron frozen and shattered, a block chilled, 500 balls in a walled pit with its crest lit, a missile in flight, a hole open), since a longer run adds no kind or component; the knockback's and contact damage's timers come back from the prefab. The tests' ground and pit walls are scaffolding no save holds, so a loaded divergence run builds them again. Outside step 7's list but inside the header's: `tests/game/seam.rs` (the player's health in the new shape) | `examples/01_platformer/{src/save.rs,src/features/{toys/mod.rs,run/mod.rs},tests/game/{main,saves,seam}.rs}` |
| 8 | 2026-10-20 | Passed | `--test game -- playthrough::` → 1 passed: a New Game at Normal walks the `lower` route's 17 launches from the spawn to the gate, then the apron and down into the collection pit, in 1,778 frames at dt 1/60 with no hit (100 HP left): the toasts "Lantern clearing", "Broken aqueduct", "Waterfall ravine" and "Ruined tower" in order, one autosave per area in the same order, the player alive and gameplay on top. `cargo test -p example-01-platformer --locked` → 105 passed; the `--list` count 105, at least 103. `rg -c 'tungsten::kit|kit::' examples/01_platformer/README.md` → 18. `just smoke` exit 0 (examples 3/3, every row; 01's log has no warning). `cargo fmt --all -- --check` and `just lint` exit 0. The script: each launch is the level test's `LAUNCHES` row, which a map without hazards found; the walker jumps the low hazards ahead (the spikes have no body, so their drawn point places them), stops against the step up that a row's start may lie in, and waits where `WAITS` says: 105 frames before lower_c's launch (the moving deck over the spiked ledge and the bobbing fire) and 105 before tower_lower_b's (the walk under the fire that sweeps it), found once by replaying the route a wait at a time in a scratch search, since removed. Departures from the rows: lower_b_exit's landing aims at 41.35 tiles, not 41.75, short of the ledge's spikes, and lower_c's jump leaves from that landing. `README.md` (new) gives the run and test commands from the folder, the controls, the spells, each mechanic's kit item and prefab or data file, the saves and the tests; `tools/README.md` drops the old controls and modules (the orbs, Mouse 4, F, R, the hearts, the death screen and `restart_world`) for the spells, the kit's mechanics, the new art and sounds and the shell's game over | `examples/01_platformer/{README.md,tools/README.md,tests/game/{main,playthrough}.rs}` |
| 9 | 2026-10-20 | Passed | `python3 -B scripts/test-release.py` → 45 tests OK, `test_package_ships_a_folder_game_example_from_its_folder` among them (a fixture example with its own `tungsten.json` ships its config, bindings and assets under `data/examples/01_demo/` and its log folder in `README.txt`; the template's stay). `cargo test -p tungsten-launcher --locked` → 5 passed, `a_listed_build_runs_from_its_folder` mapping `Platformer` to `data/examples/01_platformer`. `just script-test` exit 0 (37, 9, 54, 17, 45, 7 and 22 tests OK). The local Linux rehearsal, 139 GB free: the x86-64 release build in 2 min 28 s, `licenses` and `package v0.93.0` exit 0 (`tungsten-examples-0.93.0-linux-x64.zip`, 35.8 MB), the zip holding `Platformer` and `data/examples/01_platformer/{tungsten.json,input.json,assets/manifest.json}`; extracted with Python's `zipfile`, which keeps no file mode, so the launchers took `chmod +x` (the release tests pin the zip's executable bit); `TUNGSTEN_SMOKE_FRAMES=3 …/Platformer` exit 0 ("running the x86-64 build"); `target/m72-rehearsal` removed. `release.py` gains `folder_games` (the templates, then each shipped example whose folder holds its own `tungsten.json`); `TEMPLATE_RUNTIME` became `FOLDER_RUNTIME`; a folder game's files ship under its folder and its log line joins `README.txt`, while the other examples keep the shared root files. The launcher's `FOLDERS` gains `("Platformer", "examples/01_platformer")`; the selector routes `tools/launcher/` to `cargo test -p tungsten-launcher --locked` and `just script-test`; `docs/releases.md:114` names the folder games, their folders and the platformer's log folder | `scripts/{release,test-release,check-repo,test-check-repo}.py`, `tools/launcher/src/main.rs`, `docs/releases.md` |
| 10 | 2026-10-20 | Passed | `rg -c '^## D-19[7] — ' DECISIONS.md` → 1; `rg -c '^\*\*Amended by D-19[7]:' DECISIONS.md` → 6; ``rg -c 'Amended by `D-19[7]`' docs/DECISION_INDEX.md`` → 6; `cargo test -p tungsten-core --test decision_index --locked` → 2 passed; `rg -c 'D-19[7]' docs/plans/1.0/w04-api-freeze.md` → 1; `rg -c 'gate_step' docs/known-issues.md` and `rg -c 'until 01 \(a\) gives it its own' DESIGN.md` → no output; `rg -c 'Landed in 0.93'` → 1 in each of `w12-template.md` and `w01-ui-text-suite.md`; `just ctx` OK; `just repo-check` 0 errors (78 notes). `D-197`, written with tungsten-decision, records M72 and amends `D-123`, `D-158`, `D-173`, `D-177`, `D-181` and `D-196` with a marker each, and the index gains its row in ECS / Runtime Flow and six amendments. The break ledger's row lists M72's core and kit additions. `DESIGN.md`: 01's own folder (`:371`), a paragraph after the effects on `restore`, `suppressed_by`, air jumps, the saved runtime state with a loaded controller's still first step and `World::component_types`, and a line in the template shell's section on the due autosaves taken in `PreUpdate` after the last flush. Known issues: the `gate_step` corner and the route test's dead rows closed; the slab joins' row names the generator's merged runs; the controllers' row adds the air jumps, a loaded controller's still first step and the lit courier's missing flash (Q6); the tilemap capacity hint and the fire spread moved to the new code. The backlog's platformer row loses the air jump and gains four rows: the `place` payload, the animator's one-shots, `Lifetime` in saves and the pad's aim. W12b and W1's M6 landed in 0.93; w13 §7 gains 01 (a)'s additions; the design's (a) section notes the departures (Q2–Q6, Q11); the LLM index's two 01 rows name the README, the `saves` and `playthrough` tests and `tools/content/` (12,159 of 12,288 bytes) | `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/plans/1.0/{w04-api-freeze,backlog-1.x,w12-template,w01-ui-text-suite,w13-kit,example-01-design}.md`, `DESIGN.md`, `docs/known-issues.md`, `docs/LLM_INDEX.md` |
| 11 | 2026-10-20 | Passed | Workflow §5 closed: §6's invariants held. The API snapshots gained lines and kept every old one; the kit hash is unchanged; 01's smoke row runs from its folder and the archive's Platformer reads `data/examples/01_platformer/`; no physics, ECS-storage or `02_bench` change, so the determinism and containment hashes and the row digests stand; no renderer change since step 1's `just visual`; the template's tests pass, edited only where an autosave lands a frame later; `DefaultPlugins`' schedule and the kit's coverage test pass in the release checks. `DESIGN.md` carries step 10's changes and the status line's additions. Records: the register row holds release 0.93 and "Cut 2026-10-20 as `v0.93.0`", and the README's four owned Now fields come from `roadmap.py summary`. One `CHANGELOG.md` entry under `[Unreleased]`, with its Summary beside 0.93's earlier `D-195` and `D-196` notes; `just release-check` consistent. `status: done`; the plan is archived before the cut, so the cut, the release checks and the preflight are quoted in the session's report | `CHANGELOG.md`, `DESIGN.md`, `docs/plans/1.0/{implementation-plan,README}.md`, this plan |

## Follow-ups

- The card's owner-only check: the owner plays 01 through at the ship sitting (the design's last (a) done-when).
- The CI rehearsal prerelease that starts example 01 from its folder (W12b's done-when) is the owner's, as at M71.
- Root `assets/` and `input.json`: the `damage_flash` shader and material and the `spawn_ball` and `spawn_black_hole` actions only 01 read. Home: the Phase 6 QA pass.
- A hit flash on lit sprites (`docs/known-issues.md:57`). Home: (b)'s plan.
- The design's deferred cascade queue, its bound with the full-pit test and its place in saves (design, Shared rules). Home: (b)'s plan, with its first gameplay chain (Q19).
- The kit's `place` payload, one-shot clips for the character animator, and `Lifetime` in saves. Home: backlog rows (step 10), or a kit step in the stop that first needs one in data.
- The pad's aim on the right stick once the gamepad backend lands. Home: backlog row (step 10).

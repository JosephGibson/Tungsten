# 01's foes' clips, looks, timers and drops on the kit (example-01-foes) — draft

- **status:** draft
- **goal:** Phase 7's 7.13c's foes, the first of its two plans (§2.1). In the kit: a look's plain-sprite overlay with a bob and an unflipped frame, its particles' offset, a character animator's hold, a health bar's offset, and a bodiless prefab's centre from its shapes. Then example 01's foes on them: their clips as the animator's rules over state tags 01 writes, the troll's bar, the "!" and "?" marks, the beetle's streak and dust and the wisp's flare as looks, the "!" and the attack rests as kit timers, the bat's dive speed an effect and the dive and the charge ended by their hitbox windows (S-10, S-20, S-36, S-52, S-158, S-189); its impact cues as rules (S-41); its orbs' pickups and its centred drops and bullet (S-88, S-100); its tests unchanged (§2.2) and its older saves loading whole (§2.10).
- **non-goals:**
  - The rows split to **7.13c's carry, flight and data** (§2.1): the troll's carry and its throws' window (S-37, S-36's window), the flyers' flight on steering (S-92), the snowball (S-21) and the foes' per-kind data (S-30, S-34).
  - S-10's slam ring (to Later with the slam's area strike, a parked seam for 7.13g); S-36's hole life and S-88's missile (7.13d, with the toys and the missile's flight); 01's other `PreviousVelocity` readers onto `step_velocity` (F1).
  - Core changes; kit changes past Steps 1 and 5; new art, or any file of a locked family (`D-223`); retuning 01; a sitting as evidence (`D-226`).
- **files to touch:** by step, §3's Files lines; nothing outside them but §6's admitted files. In short: Steps 1 and 5 the kit's looks, animator, bar and spawn centre with their tests, the describe census, `api/`, `DESIGN.md`; Steps 2–4, 6 and 7 `examples/01_platformer/` (`src/`, `tests/game/`, `tools/` content and generator, the generated `assets/` copies, `README.md`) and `DESIGN.md` (Step 2); Step 8 this plan (deleted), `docs/plans/phase-7.md`, `ROADMAP.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/known-issues.md`.
- **ordered steps:** (1) the kit's look, clip and bar fields; (2) 01's state tags, its foes' clips and the troll's bar; (3) the foes' timers, schema 32; (4) the foes' looks; (5) the kit's bodiless centre; (6) 01's orbs, drops and bullet; (7) 01's impact cues; (8) close.
- **done-when:** every step's done-when passes, quoted in the evidence log; `just check` passes and the demo (§3's Step 8) works; the landed steps' rows read `landed <sha>` in phase 7's §2.2, S-36's and S-88's landed parts noted with their rows `open` for their homes, a skipped step's rows moved with their reason (§6), the five split rows `7.13` and `open`; the plan deleted with its citations and `example-01-foes` out of Now.

Item `example-01-foes` in [ROADMAP.md](../../ROADMAP.md), phase 7's 7.13c's foes (§2.8, [phase 7](phase-7.md)), from the redo's 7.13 row and phase 7's Step 13. Written 2026-11-13 at `815cfc38`. Runs under the [tungsten-plan skill](../../.claude/skills/tungsten-plan/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs.

## Context digest

- 7.13c's second plan closed (`D-277`) with S-41, S-88 and S-100 stopped and sent here, beside the eleven rows split to 7.13c's foes: 14, each with a kit gap (A1). They need about fifteen steps, so they split again by phase 7's §6 (`815cfc38`): this plan takes the foes' clips, looks, bar and timers and the stopped three; 7.13c's carry, flight and data takes the carry, the flight, the snowball and the per-kind data (§2.1).
- Four audits at `721200d1` found the split's gap table partly stale: the streak is a clip, the animator reads explicit left clips and a floor's carry, the bar's colours round-trip, steering's arrive matches 01's ease, and the carry, `Hitbox`, `Cooldown` and `Effects` are saved (A3–A21).
- It stands on `D-246` (clip rules, timed machines, the carry), `D-247` (looks by tag, the health bar, presence gates), `D-244` and `D-276` (centred spawns), `D-242` (rule limits), `D-277` (bursts, body points), `D-271` (01's handlers) and `D-275` (01's saves); and on the owner's `d0de2dd6`: the Later lines 7.13's plans write that keep 01's own code are 7.13g's parked seams, and the empty-archetype walk is 7.14's C4.
- Rules: tests are the spec (§2.2); 7.13's plans change no core; no art moves; `D-226` for perf; code at xhigh; schema 32 at Step 3 (§2.10).
- Later items expect: 7.13c's carry, flight and data to stand on Step 2's state tags and Step 3's rests; 7.14's C4 to take the walks these steps add to 01's pit (§2.10).

## 1. Audit

Read at `815cfc38` (the code as at `721200d1`). Paths under `examples/01_platformer/` are written `src/…`, `tests/…`, `tools/…`, `assets/…`; `K/` is `crates/tungsten-kit/src/`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | Phase 7 sends 7.13c's foes 14 rows: eleven split from the last plan and S-41, S-88 and S-100 from its stopped Steps 7 and 8 | `docs/plans/phase-7.md:140`, `:156`, `:162`, `:164`, `:166`, `:169`, `:178`, `:179`, `:255`, `:256`, `:258`, `:264`, `:275`, `:287`, `:420` |
| A2 | 01's foes think on one `StateMachine<EnemyState>`; `clip()` picks each kind's clip by state, the brain's strike, grounded or not, `\|v.x\| > 1`, a crawler's fear and scare, a troll's held object and the variant, `_left` by name, paused while the machine is held; `mirror()` shifts a left-facing troll's drawing by `2·centre.x − frame.x`; `steps()` sounds its walk's frames 0 and 4; `Shown` keeps the clip, the frame and the wisp's flare emitter, unsaved | `src/features/enemies/systems.rs:35-51`, `:756-885`; each kind's `clip`; `src/features/enemies/components.rs:345-352` |
| A3 | `character_animator` takes rules by tags (`state.*` by convention, for the game to set), clip sets with explicit left clips, `min_speed` (10 by default) against `\|v − floor\|`, a kinematic floor's velocity subtracted; nothing pauses its clip, and no kit item writes a machine's state as a tag | `K/present/character.rs:1060-1169`, `:1076-1080`, `:1100`, `:1163`; `K/intent/state_machine.rs`; `api/tungsten-kit-components.txt:48-56`, `:506-510` |
| A4 | `health_bar` centres its back on the drawn frame's centre, with no offset; the troll's body sits 10 px right of its 128 px frame's centre, 10 px left once `mirror()` shifts it; 01's colours round-trip as linear bytes ([0.0941, 0.0706, 0.0863, 0.8627], [0.7686, 0.1882, 0.1569, 1]); from the first holder one query a frame and two part entities a holder | `K/present/health_bar.rs:382-383`, `:434-481`; `K/present/looks.rs:1737-1739`; `src/features/enemies/draw.rs:29-108` |
| A5 | A look's overlay is a manifest animation only, set at the drawn centre plus its offset, its frame flipped with the mirror; a tag look shows at once (`grow` counts from 0); no bob; particles emit at a sprite point with no offset; parts go when the filter fails or the holder despawns | `K/present/looks.rs:130-138`, `:286-295`, `:882-888`, `:1270-1357`, `:1532-1546`, `:1680-1714` |
| A6 | The marks are plain sprites (`ex10_mark_alert`, `ex10_mark_search`); the streak is an animation whose `_left` is its mirror twin; no slam-ring sprite exists; the effects family, the marks and the streak among its 323 files, is locked, and a gained file fails its check | `assets/manifest.json:81-86`, `:11479-11486`; `tools/locks.json:1542-1869`; `D-223` |
| A7 | 01 draws the troll's bar, the beetle's streaks, the slam's dotted ring and the marks in four sprite extracts; the beetle's paw and trail dust puff from one-shot configs on unsaved per-beetle timers (`Dust`); a wisp's flare spawns a second `ex10_wisp_embers` child | `src/features/enemies/mod.rs:262-266`; `draw.rs:110-283`; `beetle.rs:248-297`; `wisp.rs:167-213`; `assets/particles/paw_dust.json`, `charge_dust.json`; `tools/generate.py:88`, `:599` |
| A8 | The "!" (`Brain::alerted`, 0.8 s), the troll's slam rest (2 s) and the crawler's and the goblin's attack rest (4 s, 2.5 s) tick in the intent phase before the machine, held or not, saved in `Brain`, which denies unknown fields | `src/features/enemies/components.rs:99-117`; `systems.rs:622-645`; `troll.rs:375-377`, `:575-578`; `crawler.rs:272-274`, `:359-361`; `goblin.rs:265-268`, `:355-359`, `:518-520` |
| A9 | The kit's `Cooldown` is one an entity, ticked in the tick phase whatever the machine, saved; `restart` keeps `remaining`, and a spent count-1 one never ticks again; no foe holds one | `K/lifecycle/cooldown.rs:72-83`, `:199-204`, `:252-253`, `:316`; `tools/content/prefabs/*.json` |
| A10 | The bat's dive adds an `Override(700)` on `move_speed` under source `dive`, removed at the dive's end and on recovery; `DIVE_SECS` 0.6 and `CHARGE_SECS` 0.8, the charge's input lock too, restate `bat_dive`'s and `beetle_charge`'s `hitbox` payloads; `Hitbox::is_armed` and `window`, the window counting while the hitbox is on; `Hitbox` is saved | `src/features/enemies/bat.rs:31-37`, `:138-168`; `beetle.rs:36-40`, `:121-142`; `tools/content/data/abilities/bat_dive.json`, `beetle_charge.json`; `K/overlap/hit.rs:1075-1086`, `:1596`, `:1720-1731` |
| A11 | An ability's `effect` payload queues on its user; an effect may hold an `override` line; `Effects` is saved; `end_effect` ends one early; only `suppressed_by` holds its time | `K/intent/ability.rs:3108-3113`; `K/damage/effects.rs:2764-2770`, `:3320-3326`, `:4311`; `tools/content/data/effects/chilled.json` |
| A12 | `prefab_centre` reads a `rigid_body`'s offset, else a registered sprite's drawn centre, else 0, so a bodiless prefab spawns uncentred headless (the last plan's Step 8); the orbs' `pickup.offset` [12, 12] is `ORB_CENTRE` and the bullet's `hitbox.offset` [16, 1.5] `BULLET_CENTRE`; the missile has neither | `K/motion/drive.rs:184-215`; `tools/content/prefabs/health_orb.json`, `mana_orb.json`, `bullet.json`, `fire_missile.json`; `src/features/enemies/drops.rs:48`; `src/features/gun/mod.rs:43` |
| A13 | 01 centres its drops (`Fallen`, `fallen`, `adopt`) and its bullet (`gun::adopt`) by hand; `adopt` also clears a drop out of rock and pops it; the seven foes' `killed` rolls are bare table IDs; a `roll` takes `centred`, `at` and `offset` | `src/features/enemies/drops.rs:38-42`, `:177-234`; `src/features/gun/systems.rs:150-173`; `tools/content/prefabs/bat.json:121`; `api/tungsten-kit-components.txt:823-828` |
| A14 | `collect` queues an orb's effect for this step's resolve and bursts at the orb's centre; a cue `effect` on `other` lands at the next step's; a `burst` action's event point is the pickup shape's centre | `src/features/enemies/drops.rs:236-259`; `K/react/cues.rs:4267-4282`; `K/damage/effects.rs:2756-2761`; `K/overlap/pickup.rs:319`, `:544` |
| A15 | `ball_impacts` sparks a small ball closing past 420 on its 0.12 s unsaved cooldown, four a step, and dusts and thuds one bowling ball at 700 a step; `body_impacts` takes four rock and four stone sounds a step by `IMPACT_BURSTS_PER_STEP`, which also spaces `crush`'s smash; `iron_landed` dusts and sounds; the same configs burst elsewhere (the gun's wall dust, the jump's `LANDING_DUST`, the sparks' hue pass by name) | `src/features/elements/systems.rs:559-622`, `:629`, `:872`, `:1306-1316`; `src/features/elements/mod.rs:131-132`; `src/features/toys/systems.rs:98`, `:217-220`; `src/features/gun/systems.rs:580`; `src/features/player/systems.rs:52` |
| A16 | `Impacted` carries no speed and is no cue event; a game's event through `CuesOnEvent` carries an amount; a rule takes `amount`, `min_interval` (a subject's) and `cap` (runs a step) | `K/overlap/impact.rs:763-771`; `K/react/cues.rs:64-87`, `:2708-2713`, `:2744-2800`, `:3985-4001`; `src/features/audio/mod.rs:64` |
| A17 | Pins: 01's pit (68,866 allocations, 73,213,294 bytes, 298 moves, 295 spawned, 246 despawned, 12 events); a kit query walks every archetype its shape matches, so a component on the foes costs its system their 129–256 archetypes each frame (7.13c's facing: +178, +1,075,072), now 7.14's C4; the template's `AGENTS.md` at 12,277 and the LLM index at 12,278 of 12,288 bytes; a new kit component, data kind or cue event needs a catalogue name | `tests/game/guards/pit.json`; `docs/known-issues.md:56`; `docs/plans/phase-7.md:360-361`, `:371`; `scripts/check-repo.py:153-168` |
| A18 | Saves: schema 31; a saved foe's component is saved by the registry, rebuilt or presentation, and a load gives each entity back what it carried (two census tests); 443 tests, 442 run and 1 ignored; tests read `Shown.clip` (5), `draw::bars`, `draw::charges`, `mark()` (8), `Brain::cooldown` (8), `DIVE_SECS` and `CHARGE_SECS` (4 each), and `ball_impacts` with `IMPACT_BURSTS_PER_STEP` | `src/save.rs:90`; `tests/game/saves.rs:812-907`; `tests/game/ball_pit.rs:7-8`, `:116`; `rg -c '#\[test\]' tests/game` |
| A19 | The split rows' gaps: a projectile is never a dynamic body, a lifetime's end sends nothing, `cap` admits kit routes alone and a hitbox credits a dead source; the carry mirrors and places about `Position` and drops by the carrier's tags alone, with no holder read; steering takes a mover's direction whole, with no pace or off; `StateMachine` and `Sight` have no defaults; `KindInfo`'s fields have 20 readers | `K/overlap/projectile.rs:77-78`, `:887`; `K/lifecycle/cap.rs:371-378`; `K/overlap/grid.rs:499-501`; `K/motion/carry.rs:59-88`, `:239-246`, `:279-285`; `K/motion/top_down.rs:504-519`; `K/intent/state_machine.rs:77-107`; `src/features/enemies/mod.rs:113-221` |
| A20 | A named entity saves only the fields that changed from its baseline, so a named foe's `enemy` seldom holds its `kind`; a record's restore spawns its prefab, puts what it saved and removes every registered component it did not, the prefab's new `cooldown` and `effects` among them | `crates/tungsten-core/src/save/patch.rs:16-45`; `crates/tungsten-core/src/save/world.rs:300-320`, `:1219-1226` |
| A21 | The dust configs cap an emitter's live particles (`max_alive`): `charge_dust` 6 with 5 a puff living to 0.45 s, `paw_dust` 12 with 4 a puff living to 0.6 s, so today each puff is an emitter of its own; the generator's contract test pins `paw_dust`'s 12 | `assets/particles/charge_dust.json`, `paw_dust.json`; `crates/tungsten/src/particles.rs:194-200`; `tools/test_generate.py:429` |

## 2. Design

### 2.1 Scope: what lands, what splits

The 14 rows need eight kit changes and as many 01 ports, past phase 7's ten steps, so they split by dependency order (§6, `815cfc38`). This plan lands nine: S-10 but its slam ring, S-20, S-36 but its throw window and hole, S-41, S-52, S-88 but its missile, S-100, S-158 and S-189. **7.13c's carry, flight and data**, written when this plan closes, takes five and a part, each with its gap at `815cfc38`:

| Row | The gap | Its kit change |
| --- | --- | --- |
| S-37 | The carry sets its held body's `Position` at the carrier's plus an offset mirrored about it; 01 sets a held body's bottom at the troll's `hands` point, mirrored about its body's centre. It drops by the carrier's tags alone, where 01 also drops a held foe that thaws; it has no holder read, which 01 asks at three sites; its release locks no input | A carry by centres, a drop filter on the held, `carrier_of` |
| S-36's throw window | A thrown frozen crawler or goblin holds its rest's `Cooldown` (Step 3), so the window needs the throw's own record | With the carry's throw |
| S-92 | Steering takes the mover's direction whole, so the dive's line, the rest and the black hole's blend, all game writes, are lost; no pace; distances from `Position` | A pace and an off switch |
| S-21 | A projectile is bodiless or kinematic, 01's snowball dynamic; a lifetime's end sends nothing; `cap` admits kit routes alone; a hitbox credits a dead source; the thrower's factor is the difficulty's line on its `damage`, which `damage_dealt` never sees | A projectile's seconds, a game's admission, a live credit |
| S-30, S-34 | `StateMachine` and `Sight` have no data form a prefab can name; `KindInfo`'s 11 fields have 20 readers in `src/` and 10 in tests; a tome's texts and grant, a body's pull share and the spray's look are Rust tables | `StateMachine`'s defaults |

The parts that go elsewhere: S-10's slam ring stays 01's dotted drawing and goes to Later with the slam's area strike, so 7.13g's parked seams take the two together (`d0de2dd6`; no ring sprite exists, and the effects family is locked); S-36's hole life is a toy's, with 7.13d; S-88's missile has no shape the kit can centre without its art, and goes with its flight to 7.13d; S-36's fuses landed with 7.13a (`lit`, `D-271`).

### 2.2 Tests are the spec

- Each step keeps all 442 of 01's run tests passing with their asserted outcomes. A test's body changes only where it reads, spawns or calls what the step removes, ported to the kit's equivalent (`AnimationState` for `Shown.clip`, the kit's bar and look parts for 01's extracts and `mark()`, a `Cooldown` for a `Brain` rest, the ability's window for `DIVE_SECS` and `CHARGE_SECS`, the rule's value for a removed constant), each port counted in the evidence row.
- An asserted value moves only where the step's Moves names it, by at most one fixed step in time, 1 % in an amount or 1 px in a place, its old and new value listed; any other changed outcome stops the step (§6). No test is deleted but one whose subject the step deletes.
- New tests only where a done-when names them. The kit's fields take kit tests; they add no genre claim, so no recipe.

### 2.3 The kit's look, clip and bar fields (Step 1)

Each defaults to today's behaviour, so every kit test, recipe and pin holds but the describe census, which counts the new numeric fields, and `api/`:

- **`look_overlay.sprite`:** a manifest sprite shown in place of `animation`, exactly one of the two, the load checking it.
- **`look_overlay.bob`:** `{ "height": px, "seconds": s }`: the overlay rises `height · |sin(π·t / seconds)|` on the game clock from its show (01's "!": 2 and 0.5).
- **`look_overlay.flip`:** `true` by default; `false` keeps its frame unflipped while its offset still mirrors, so a glyph reads alike either way.
- **`look_particles.offset`:** `[x, y]` from its point, mirrored as an overlay's offset is.
- **`character_animator.hold`:** a tag filter, none by default; while the entity's tags pass it, its clip holds its frame (`AnimationState::playing` false), a clip picked meanwhile starting held.
- **`health_bar.offset`:** `[x, y]` from the drawn frame's centre, x mirrored as a look's overlay offset is.
- No cooldown change: a rest starts as a fresh count-1 `Cooldown` (§2.5), so a spent one needs no re-arm, and the "?" waits on the machine's own time (§2.6), so a tag look needs no delay.

### 2.4 State tags, the foes' clips and the troll's bar (Step 2)

- **State tags:** `foe_tags`, in the react phase after `enemy_deaths`, writes each foe's tags from its machine and brain, inserting and removing only what changed: `state.<state>` (`patrol`, `alert`, `chase`, `windup`, `attack`, `recover`, `search`, `return`, `panic`), `state.held` while its machine is held, `strike.<strike>` for its brain's strike, and `troll.holding`, `crawler.afraid` and `bat.roosting` while `held`, `afraid` and `roosting` say so; a dead foe holds none. `data/tags/game.json` declares them.
- **Clips:** each foe prefab's `character_animator` holds its kind's `clip` as rules in today's arm order, each clip a `{right, left}` set of its `_left`-named frames, `min_speed` 1 and `hold` `{all: [state.held]}`; the beetle's and the armoured troll's stripped clips are rules on `status.corroded` ahead of their plain ones; the troll's `lifting` fallback for a brain with no strike goes, since the schemas that held one are refused. `systems::clip`, each kind's `clip` and `clips`' swap go; `enemy_clips` keeps `steps()`, `mirror()` and, until Step 4, `flare_embers`, after `CHARACTER_ANIMATOR` and before `EFFECT_LOOK_LATE`, so the looks and the bar see the shifted troll.
- **The bar:** the troll's and the armoured troll's prefabs take `health_bar` with `offset` [10, 0] and 01's colours (A4); `draw::bars` and its colours go; the marks keep the bar's height until Step 4.
- Moves: a foe a moving deck carries shows its idle where it walked, and a troll in the air its idle where it walked across (A3); the pit's allocations and bytes (the animator's and the bar's queries walking the foes' archetypes, the tags' changes) and, by the bar's parts alone, `spawned`.

### 2.5 The foes' timers (Step 3)

- **The "!":** an `alert_mark` effect (0.8 s, granting `mark.alert`), applied where a fresh alert set `brain.alerted` and ended at a death; `marks()` and `Brain::alerted` go; `draw::mark` reads the tag until Step 4.
- **The rests:** the troll's prefabs take `cooldown` `{seconds: 2, count: 0}`, the crawler's `{4, 0}` and the goblin's `{2.5, 0}`, spent from the spawn, so a fresh foe holds the component and is rested; the slam, the snowball's flick, the goblin's throw and its dash-in's end start a rest, a fresh `Cooldown` of the prefab's `cooldown.seconds` (read from the prefab registry, as `disarm_as_prefab` reads its health) with one elapse, inserted over the one there or, on a record a load stripped (A20), in its place; a foe rests while its cooldown has an elapse left, and with none is rested; `cooldowns()`, `Brain`'s `slam_cooldown` and `cooldown`, `SLAM_COOLDOWN`, `SNOWBALL_COOLDOWN` and `goblin::COOLDOWN` go. Both clocks tick before the machine each step, held or not, as 01's did.
- **Effects from the spawn:** a foe prefab with no `effects` takes an empty list, so a first "!" inserts no component and moves no archetype.
- **The dive (S-158):** `bat_dive`'s payload takes a `bat_diving` effect (0.6 s, `override` 700 on `move_speed`) before its hitbox; `enter_attack`'s lines, `DIVING`, `DIVE_SPEED` and `end_dive` go, the recovery ending the effect with `end_effect`.
- **The windows (S-189):** the bat's and the beetle's attacks end once their hitbox disarms; the beetle locks its input for its hitbox's `window`; `DIVE_SECS` and `CHARGE_SECS` go.
- **Saves:** schema 32, `to_32`: a saved `brain`'s `alerted`, `slam_cooldown` and `cooldown` leave it; a rest left becomes the foe's `cooldown` beside it, `{seconds: rest, left: rest, remaining: 1}`, which spends what was left and serves no later rest, so the migration needs no kind (A20); the "!" is dropped; every saved `stats` line from `dive` goes. A schema-31 record whose foe was rested loads with no `cooldown` or `effects` (A20) and inserts them at its next rest and "!".
- Moves: a dive or a charge that a stun, a freeze or the dormant hold stops ends at its first step after, its window spent, where it flew on unarmed, and the dive's speed ends with it; an older save's "!" lost (0.8 s at most); `Brain`'s saved form; `events_sent` and the pit's allocations and bytes (the effects' applications and expiries, the cooldowns' walks).

### 2.6 The foes' looks (Step 4)

- **The marks** on each foe prefab: `{when: {all: [mark.alert]}, look: {overlay: {sprite: ex10_mark_alert, offset, flip: false, bob: {height: 2, seconds: 0.5}}}}` and `{when: {all: [mark.search], none: [mark.alert]}, look: {overlay: {sprite: ex10_mark_search, offset, flip: false}}}`, the offset from the drawn centre `[centre.x − frame.x / 2, −(frame.y / 2 + 14)]`, the troll's 12 px higher over its bar. `foe_tags` adds `mark.search`, declared in `data/tags/game.json`, while the machine's search has run `SEARCH_MARK_AFTER`, as `mark()` reads it, so the "?" waits on the saved, held-aware search time and shows the step the "!" ends.
- **The streak:** the beetle's `{when: {all: [state.attack]}, look: {overlay: {animation: ex10_charge_streak, offset}}}`, its near edge 4 px behind the body, flipped facing left as its `_left` twin is drawn.
- **The dust:** the beetle's `{when: {all: [state.windup]}, look: {particles: {config: ex10_paw_dust, offset}}}` at its front foot and a `state.attack` one of `ex10_charge_dust` at its rear feet, offsets from today's points (A7); the two configs pulse every 0.15 s and 0.06 s where they burst once, each `max_alive` its pulse's count times the pulses its longest particle overlaps (`paw_dust` 4 × 4 = 16, `charge_dust` 5 × 8 = 40), so one emitter holds what today's emitter a puff held (A21); `beetle::dust` and `Dust` go.
- **The flare:** the wisp's `{when: {all: [state.alert], none: [state.held]}, look: {particles: {config: ex10_wisp_embers, offset: [0, 2]}}}`; `flare_embers` and `Shown::flare` go.
- `draw::marks`, `draw::charges`, `draw::mark` and `draw.rs`' mark and bar constants go, `SEARCH_MARK_AFTER` staying for `foe_tags`; the ring and the debug boxes stay.
- Moves: the first puff a pulse after the tell or the charge starts; the dust's and the embers' patterns from one stream an emitter; `paw_dust`'s contract cap 16; the marks bob from their show; the pit's allocations and bytes, `spawned` and `despawned` (the looks' parts and their walk).

### 2.7 A bodiless prefab's centre (Step 5)

- `prefab_centre` reads, for a prefab with no `rigid_body`, the offset of its `hurtbox`, `hitbox`, `pickup` or projectile `shape`, the first it has, before its sprite's drawn centre: the centre its own shapes give it, art loaded or not. Every caller takes it: a spawn payload's `centred`, `place`, and the cue's `prefab` and `roll`. It amends `D-244`'s and `D-276`'s centre.
- Nothing centred today has a shape and no body: the kit tests' and the recipes' centred prefabs (`mote`, `banner`, `missile`, `oil_flask`, `poof`) are sprites and `tooth` a body, and 01 centres nothing, so no outcome moves.
- Moves: the `centred` fields' docs in `api/`.

### 2.8 The orbs and the centred drops and bullet (Step 6)

- **S-100:** each orb prefab's `picked_up` cues take `effect: {id: orb_heal | orb_mana, target: other}` and its burst at the event; `drops::collect`, `orb_collect` and `OrbKind::effect` go.
- **S-88, the drops:** the seven foe prefabs' `killed` rolls take `at: centre` and `centred: true`; `Fallen`, `drops::fallen`, `orb_fallen` and `adopt`'s centring go; `adopt` keeps `Joined`, the scene state, `clear` and the pop.
- **S-88, the bullet:** `"centred": true` on `gun.json`'s spawn; `gun::adopt`'s shift goes, `BULLET_CENTRE` staying for the shot's sweep.
- Moves: an orb's heal or mana a step later (the cue's effect at the next step's resolve; the last plan's Step 8 moved no test by it); `events_sent`, should the pit's window collect an orb.

### 2.9 Impact cues (Step 7)

- **S-41:** `ball_impacts` keeps its closing speeds and sends `ball_sparked` about each small ball past 420 at its centre and `ball_dusted` about each bowling ball at 700 at the dust's point, in save-ID order, through `CuesOnEvent` as 01's sounds go; `iron_landed` takes its block and sends `iron_dusted`. A new `data/cue_rules/impacts.json` bursts them: `ex10_small_ball_impact` with `min_interval` 0.12 and `cap` 4, `ex10_landing_dust` with `cap` 1, and `ex10_iron_dust`. The rocks' and the stones' sound rules take `cap` 4 where `body_impacts` took four.
- `SmallBall::impact_cooldown`, its tick and `IMPACT_COOLDOWN` go; `IMPACT_BURSTS_PER_STEP` stays for `crush`'s smash; the sounds stay 01's events with their intervals.
- `ball_pit.rs`' two tests, which call `ball_impacts` and count bursts, step the schedule after the call and read the cap from the rule file (ported, §2.2).
- Moves: `events_sent` and the pit's allocations and bytes (the impact events).

### 2.10 Pins, guards, saves and records

- **Unchanged:** the physics hashes; the kit's determinism pin; the kit's schedule pins and the template's (no kit system is added); 02's pins; the views guard; the template's tests' outcomes; `src/shell/`.
- **Moved by design:** `api/` (Steps 1, 5), the describe census (Step 1), `DESIGN.md` (Steps 1, 2, 5).
- **01's pit:** a step may move the lines §2.4–§2.9 name, each quoted with its cause and decomposed by probe (the guard's size histogram and per-size backtraces, as 7.13c's run found the facing's walk); a move its cause does not account for, or any other line moving, stops the step (§6). `archetype_moves` holds throughout.
- **Saves:** schema 32 at Step 3 with `to_32` and its test; no other step changes a saved shape. A presentation component a prefab adds (Steps 2, 4) comes from the prefab at every load; a registered one (Step 3's `cooldown` and `effects`) comes from it for a named foe, while a schema-31 record's restore removes it (A20), which §2.5's fresh rests and the "!"'s insert absorb. The census's "a load gives each entity back" test checks fresh saves. `pit_full_saves_within_two_mib` holds with the foes' new tags.
- **Census:** Steps 2 and 4 put `CharacterAnimator`, `HealthBar` and `Looks` in the foes' presentation lists; `Cooldown` and `Effects` are registered, so saved.
- **Records:** 01's README rows each step makes wrong; the template's `AGENTS.md` and the LLM index unchanged (no new component, data kind or cue event, A17); `D-27[8]` at the close with the tungsten-decision skill, the next ID rechecked then.

## 3. Steps

Every code step's done-when adds: `cargo fmt --all -- --check` → exit 0; `just lint` → exit 0; `just repo-check` → exit 0. Every kit step's adds: `cargo test -p tungsten --lib --locked -- kit_tests:: kit_recipes::` → exit 0, the determinism pin unchanged; `cargo test -p tungsten-template-basic --locked` → exit 0; `cargo test -p example-02-bench guards_` → exit 0, no pin rewritten; from `examples/01_platformer`, `cargo test --test game` → exit 0, every test passing, its pit pin unchanged; `just ctx`, `just docs-ratchet` → exit 0. Every 01 step's adds, from `examples/01_platformer`: `cargo test --test game` → exit 0, 442 passed before the step's new tests, 1 ignored, the Moves named; `cargo clippy --all-targets -- -D warnings` → exit 0; `just level-check` → exit 0 where `tools/` changes, every locked family holding (Pillow in `target/e01s/venv`, run alone in the background); `TUNGSTEN_GUARDS_WRITE=1 cargo test --test game -- guards::` then `cargo test --test game -- guards::` → exit 0, no line but §2.10's moved, each quoted with its cause.

### Step 1: The kit's look, clip and bar fields

- **Files:** `crates/tungsten-kit/src/present/looks.rs`, `crates/tungsten-kit/src/present/character.rs`, `crates/tungsten-kit/src/present/health_bar.rs`, `crates/tungsten-kit/src/tests/looks.rs`, `crates/tungsten-kit/src/tests/character.rs`, `crates/tungsten-kit/src/tests/health_bar.rs`, `crates/tungsten-kit/src/tests/describe.rs`, `api/tungsten-kit.txt`, `api/tungsten-kit-components.txt`, `DESIGN.md`.
- **Change:** §2.3.
- **Done-when:** `cargo test -p tungsten-kit --lib --locked -- present::` → exit 0 with these among the run: `a_look_overlay_shows_a_plain_sprite` (both or neither of `sprite` and `animation` refused, an unknown sprite refused at the load), `an_overlay_bobs_and_keeps_its_frame_unflipped` (`height` up at half its `seconds`; `flip: false` facing left: the offset mirrored, the frame not), `look_particles_take_a_mirrored_offset`, `an_animator_holds_its_clip_while_its_hold_passes` (the frame held, a clip picked meanwhile held, let go it plays) and `a_health_bar_takes_a_mirrored_offset`; the kit's whole suite with its doctests → exit 0, the describe census moved by the new numeric fields alone, quoted; the kit step's checks; `just api` then `git diff --stat api/` → the two kit files alone.
- **Commit:** `feat(kit): look sprites and bobs, clip holds, bar offsets`
- **Moves:** the describe census; `api/`; `DESIGN.md`'s looks and animator paragraphs.

### Step 2: 01's state tags, its foes' clips and the troll's bar

- **Files:** `examples/01_platformer/src/features/enemies/systems.rs`, `examples/01_platformer/src/features/enemies/mod.rs`, `examples/01_platformer/src/features/enemies/components.rs`, `examples/01_platformer/src/features/enemies/draw.rs`, `examples/01_platformer/src/features/enemies/bat.rs`, `examples/01_platformer/src/features/enemies/beetle.rs`, `examples/01_platformer/src/features/enemies/crawler.rs`, `examples/01_platformer/src/features/enemies/goblin.rs`, `examples/01_platformer/src/features/enemies/troll.rs`, `examples/01_platformer/src/features/enemies/wisp.rs`, `examples/01_platformer/tools/content/prefabs/` (the seven foes), `examples/01_platformer/assets/prefabs/` (their copies), `examples/01_platformer/tools/content/data/tags/game.json`, `examples/01_platformer/assets/data/tags/game.json`, `examples/01_platformer/tools/outputs.json`, `examples/01_platformer/tests/game/enemies.rs`, `examples/01_platformer/tests/game/saves.rs`, `examples/01_platformer/tests/game/guards/pit.json`, `examples/01_platformer/README.md`, `DESIGN.md`.
- **Change:** §2.4; `DESIGN.md`'s note that 01 holds no bar or look holder corrected.
- **Done-when:** the common checks, these among the run: `enemies::the_trolls_bar_stays_over_its_body_facing_either_way` (the kit's back within 1 px of centred over the troll's body facing right and left, its fill the health's share) and `enemies::the_troll_shows_its_health_bar`, ported to the kit's bar parts; the five `Shown.clip` reads ported to `AnimationState`; `rg -q 'fn clip\(' src/features/enemies; test $? -eq 1` → exit 0; `rg -q 'fn bars\b' src/features/enemies/draw.rs; test $? -eq 1` → exit 0; `jq -e '.components.character_animator.hold.all == ["state.held"]' assets/prefabs/{wisp,crawler,beetle,bat,troll,troll_armoured,goblin}.json` → `true` for each; `jq -e '.components.health_bar.offset == [10, 0]' assets/prefabs/troll.json assets/prefabs/troll_armoured.json` → `true` for each.
- **Commit:** `feat(example-01): the foes' clips on the kit's rules, the troll's bar`
- **Moves:** §2.4's.

### Step 3: The foes' timers

- **Files:** `examples/01_platformer/src/features/enemies/systems.rs`, `examples/01_platformer/src/features/enemies/mod.rs`, `examples/01_platformer/src/features/enemies/components.rs`, `examples/01_platformer/src/features/enemies/draw.rs`, `examples/01_platformer/src/features/enemies/bat.rs`, `examples/01_platformer/src/features/enemies/beetle.rs`, `examples/01_platformer/src/features/enemies/crawler.rs`, `examples/01_platformer/src/features/enemies/goblin.rs`, `examples/01_platformer/src/features/enemies/troll.rs`, `examples/01_platformer/src/save.rs`, `examples/01_platformer/tools/content/prefabs/` (the seven foes), `examples/01_platformer/assets/prefabs/` (their copies), `examples/01_platformer/tools/content/data/effects/alert_mark.json` (new), `examples/01_platformer/tools/content/data/effects/bat_diving.json` (new), `examples/01_platformer/assets/data/effects/alert_mark.json` (new), `examples/01_platformer/assets/data/effects/bat_diving.json` (new), `examples/01_platformer/tools/content/data/abilities/bat_dive.json`, `examples/01_platformer/assets/data/abilities/bat_dive.json`, `examples/01_platformer/tools/content/data/tags/game.json`, `examples/01_platformer/assets/data/tags/game.json`, `examples/01_platformer/assets/manifest.json`, `examples/01_platformer/tools/outputs.json`, `examples/01_platformer/tests/game/enemies.rs`, `examples/01_platformer/tests/game/saves.rs`, `examples/01_platformer/tests/game/data.rs`, `examples/01_platformer/tests/game/presentation.rs`, `examples/01_platformer/tests/game/guards/pit.json`, `examples/01_platformer/README.md`.
- **Change:** §2.5.
- **Done-when:** the common checks, these among the run: `saves::an_older_saves_foes_keep_their_rests` (a capture rewritten to schema 31's shape, a map troll's patch with `slam_cooldown` 1.2 and no `enemy.kind`, a summoned goblin's record with `cooldown` 2.0, a summoned crawler's record rested and so with no `cooldown`, and a bat's `dive` line in it, loaded: the troll slams no sooner than 1.2 s on and the goblin approaches no sooner than 2.0 s on, each within a step, then each rests its prefab's full seconds after its next attack; the crawler, rested, throws and then rests its 4 s; the bat at its own speed); the `Brain::cooldown` writes and reads ported to `Cooldown`, `DIVE_SECS` and `CHARGE_SECS` to the abilities' windows; `rg -q 'slam_cooldown|brain\.cooldown|\.alerted\b|DIVE_SECS|CHARGE_SECS|\bDIVING\b|fn end_dive\b|SLAM_COOLDOWN|SNOWBALL_COOLDOWN' src/features; test $? -eq 1` → exit 0, `src/save.rs`' migration reading the old names; `rg -q 'fn (marks|cooldowns)\b' src/features/enemies/systems.rs; test $? -eq 1` → exit 0; `rg -q 'const SCHEMA: u32 = 32;' src/save.rs` → exit 0; `jq -e '.components.cooldown == {"seconds": 2, "count": 0}' assets/prefabs/troll.json` → `true`; `jq -e '.payload[0].effect == "bat_diving"' assets/data/abilities/bat_dive.json` → `true`.
- **Commit:** `feat(example-01)!: the foes' alerts, rests and dives on kit timers`
- **Moves:** §2.5's; schema 32.

### Step 4: The foes' looks

- **Files:** `examples/01_platformer/src/features/enemies/draw.rs`, `examples/01_platformer/src/features/enemies/beetle.rs`, `examples/01_platformer/src/features/enemies/wisp.rs`, `examples/01_platformer/src/features/enemies/components.rs`, `examples/01_platformer/src/features/enemies/mod.rs`, `examples/01_platformer/src/features/enemies/systems.rs`, `examples/01_platformer/tools/content/prefabs/` (the seven foes), `examples/01_platformer/assets/prefabs/` (their copies), `examples/01_platformer/tools/content/data/tags/game.json`, `examples/01_platformer/assets/data/tags/game.json`, `examples/01_platformer/tools/generate.py`, `examples/01_platformer/tools/test_generate.py`, `examples/01_platformer/assets/particles/paw_dust.json`, `examples/01_platformer/assets/particles/charge_dust.json`, `examples/01_platformer/tools/outputs.json`, `examples/01_platformer/tests/game/presentation.rs`, `examples/01_platformer/tests/game/enemies.rs`, `examples/01_platformer/tests/game/saves.rs`, `examples/01_platformer/tests/game/guards/pit.json`, `examples/01_platformer/README.md`.
- **Change:** §2.6; the tests' `mark()` reads ported to a helper reading the foe's look parts.
- **Done-when:** the common checks, these among the run: `presentation::an_alert_shows_its_mark_and_stings_once`, its eight `mark()` reads ported; `presentation::a_marks_glyph_is_never_mirrored` (a crawler searching facing left shows `ex10_mark_search` unflipped at its mirrored offset); `enemies::the_beetles_paw_puffs_its_dust_and_its_charge_draws_its_streaks`, ported to the look's overlay and the configs' particles; `enemies::a_beetles_dust_keeps_its_density` (the most `ex10_paw_dust` particles alive at once over a tell and `ex10_charge_dust` over a charge, each at least 80 % of the peak this test measures at the step's base first, both peaks quoted); `rg -q 'fn (marks|charges|mark)\b' src/features/enemies/draw.rs; test $? -eq 1` → exit 0; `rg -q 'fn (dust|flare_embers)\b|struct Dust\b|pub flare\b' src/features/enemies; test $? -eq 1` → exit 0; `jq -e '.emission.kind == "pulse" and .max_alive == 16' assets/particles/paw_dust.json` and `jq -e '.emission.kind == "pulse" and .max_alive == 40' assets/particles/charge_dust.json` → `true` each.
- **Commit:** `feat(example-01): the foes' marks, streak, dust and flare as looks`
- **Moves:** §2.6's.

### Step 5: A bodiless prefab's centre

- **Files:** `crates/tungsten-kit/src/motion/drive.rs`, `crates/tungsten-kit/src/intent/ability.rs`, `crates/tungsten-kit/src/react/cues.rs`, `crates/tungsten-kit/src/tests/ability.rs`, `api/tungsten-kit-components.txt`, `DESIGN.md`.
- **Change:** §2.7; the `centred` fields' docs.
- **Done-when:** `cargo test -p tungsten-kit --lib --locked -- intent::ability:: react::cues::` → exit 0 with `a_bodiless_prefab_centres_on_its_shapes_offset` among the run (a pickup prefab and a hitbox prefab spawned `centred` with no art loaded, each centred on its user's centre plus the offset; a body's offset first; a prefab with neither its sprite's drawn centre, else its point); the kit's whole suite with its doctests → exit 0; the kit step's checks; `just api` then `git diff --stat api/` → `api/tungsten-kit-components.txt` alone, or nothing.
- **Commit:** `feat(kit): a bodiless prefab's centre from its shapes' offsets`
- **Moves:** `api/`'s docs.

### Step 6: 01's orbs, drops and bullet

- **Files:** `examples/01_platformer/src/features/enemies/drops.rs`, `examples/01_platformer/src/features/enemies/mod.rs`, `examples/01_platformer/src/features/gun/systems.rs`, `examples/01_platformer/tools/content/prefabs/` (the orbs and the seven foes), `examples/01_platformer/assets/prefabs/` (their copies), `examples/01_platformer/tools/content/data/abilities/gun.json`, `examples/01_platformer/assets/data/abilities/gun.json`, `examples/01_platformer/tools/outputs.json`, `examples/01_platformer/tests/game/enemies.rs`, `examples/01_platformer/tests/game/gun.rs`, `examples/01_platformer/tests/game/saves.rs`, `examples/01_platformer/tests/game/guards/pit.json`, `examples/01_platformer/README.md`.
- **Change:** §2.8.
- **Done-when:** the common checks, `enemies::a_drop_lands_clear_of_rock_falls_and_is_drawn_in` and `enemies::a_killed_enemy_rolls_its_drops_and_an_orb_heals_or_restores_mana` among the run; `rg -q 'fn (collect|fallen)\b|struct Fallen\b' src/features/enemies/drops.rs; test $? -eq 1` → exit 0; `jq -e '[.payload[] | select(has("spawn")) | .spawn.centred] == [true]' assets/data/abilities/gun.json` → `true`; `jq -e '[.components.cues.killed[] | select(has("roll")) | .roll.centred] == [true]' assets/prefabs/{wisp,crawler,beetle,bat,troll,troll_armoured,goblin}.json` → `true` for each.
- **Commit:** `feat(example-01): orbs and drops on the kit's cues, the bullet centred`
- **Moves:** §2.8's.

### Step 7: 01's impact cues

- **Files:** `examples/01_platformer/src/features/elements/systems.rs`, `examples/01_platformer/src/features/elements/mod.rs`, `examples/01_platformer/src/features/toys/systems.rs`, `examples/01_platformer/src/features/toys/components.rs`, `examples/01_platformer/src/features/audio/mod.rs`, `examples/01_platformer/tools/content/data/cue_rules/impacts.json` (new), `examples/01_platformer/assets/data/cue_rules/impacts.json` (new), `examples/01_platformer/tools/content/data/cue_rules/sounds.json`, `examples/01_platformer/assets/data/cue_rules/sounds.json`, `examples/01_platformer/assets/manifest.json`, `examples/01_platformer/tools/outputs.json`, `examples/01_platformer/tests/game/ball_pit.rs`, `examples/01_platformer/tests/game/guards/pit.json`, `examples/01_platformer/README.md`.
- **Change:** §2.9.
- **Done-when:** the common checks, `ball_pit.rs`' two tests ported; `rg -q 'impact_cooldown|IMPACT_COOLDOWN' src; test $? -eq 1` → exit 0; `rg -q 'spawn_burst\(world, "ex10_(small_ball_impact|landing_dust|iron_dust)"' src/features/elements/systems.rs src/features/toys/systems.rs; test $? -eq 1` → exit 0; `rg -q '\.take\(IMPACT_BURSTS_PER_STEP\)' src; test $? -eq 1` → exit 0; `jq -e '[.[] | select(.on == "ball_sparked")][0] | .min_interval == 0.12 and .cap == 4' assets/data/cue_rules/impacts.json` → `true`.
- **Commit:** `feat(example-01): impact sparks and dust as cue rules`
- **Moves:** §2.9's.

### Step 8: Close

- **Files:** this plan (deleted), `docs/plans/phase-7.md`, `ROADMAP.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/known-issues.md`.
- **Change:** `D-27[8]` with the tungsten-decision skill (§2.3, §2.7; amending `D-244`, `D-246`, `D-247` and `D-276`; 01's foes on them at schema 32; the next ID rechecked); phase 7's §2.2: S-10 `landed` with its ring in Later, S-20, S-41, S-52, S-100, S-158 and S-189 `landed`, S-36 and S-88 `open` with their landed parts' commits and their rests' homes (7.13c's carry, flight and data; 7.13d), a skipped step's rows `open` with its move, the five split rows noted for 7.13c's carry, flight and data; its evidence row "13 (7.13), part c's third"; ROADMAP: Later's slam strike line taking the ring, F1 as a Later line, Now's phase-7 line naming 7.13c's carry, flight and data next; the other follow-ups to known issues or Later; the plan and its citations deleted; `example-01-foes` out of Now.
- **Done-when:** `just check` → exit 0; `just physics-release` → exit 0, the hashes unchanged; `just smoke` → passes; `just api-check`, `just docs-ratchet`, `just repo-check`, `just ctx`, `git diff --check` → exit 0; before the deletion, `rg -n '^\| (S-10|S-20|S-41|S-52|S-100|S-158|S-189) \| [^|]+ \| 7\.13 \| landed ' docs/plans/phase-7.md` → a row for each whose step landed, the evidence log naming the rest, and `rg -c '^\| (S-21|S-30|S-34|S-37|S-92) \| [^|]+ \| 7\.13 \| open \|' docs/plans/phase-7.md` → 5; after it, `rg -l 'plans/example-01-foes\.md' --glob '!CHANGELOG.md' .; test $? -eq 1` → exit 0; `WGPU_BACKEND=vulkan just perf sitting --quick` → `e01-pit` quoted as information, not evidence; the demo: from `examples/01_platformer`, `tungsten run --script` on a scratch script (`start_run`; `place_player` on the pit's deck; `wake_foes`; frames until a spotting, with a `capture` of the marks; the `tagged` probe of `mark.alert` and of `state.chase`; `count` of `health_bar`; `reload_run`; the `area` probe and `count` of `enemy`) → exit 0, its asserts passed: a foe holding `mark.alert` and one chasing before the load, two bar holders, the World's 42 foes and the area the same across it, its captures the sheet.
- **Ends with:** the commits, the evidence, what the owner should try (01 plays as before: the foes' clips, the troll's bar, the "!" and the "?", the beetle's streaks and dust, the wisp's flare, the slam's and the attacks' rests, the bat's dive, orbs that drop centred and heal, impact sparks and dust), the next prompt (7.13c's carry, flight and data's plan).

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | One plan of about fifteen steps, or the split | The split (§2.1), by phase 7's §6 |
| Q2 | S-10's slam ring: a sprite first, or 01's drawing kept | Kept, to Later with the slam's area strike for 7.13g's parked seams: the effects family is locked and a code session moves no art |
| Q3 | The marks: one-frame clips, or a plain-sprite overlay | The overlay's `sprite`: one-frame clips would be gained files in the locked effects family |
| Q4 | The foes' states as tags: a kit link from the machine, or 01's writer | 01's `foe_tags`: `state.*` is the game's to set (`D-246`, `D-247`), and a kit link waits for a second game |
| Q5 | The rests: abilities' cooldowns, as the register says, or count-1 `Cooldown`s | A fresh count-1 `Cooldown` of the prefab's seconds at each rest: the slam, the snowball and the goblin's throws are 01's own attacks, not kit abilities, and an ability needs a payload; a fresh one needs no kit re-arm and outlives a migrated rest's length |
| Q6 | The "!": an effect or a second `Cooldown` | An effect: an entity holds one `Cooldown`, a rest's, and the effect grants the tag the look reads, saved with its time |
| Q7 | The dive and the charge: the machine's seconds or the hitbox's window | The window's: one clock, the ability's data; a held attack ends after it (Moves) |
| Q8 | A moving foe: 01's `\|v.x\| > 1` or the animator's `\|v − floor\| ≥ min_speed` | The animator's at 1: a foe a deck carries shows its idle (Moves) |
| Q9 | An orb's heal: a kit `land: now` on the cue's effect, or a step later | A step later: the last plan's Step 8 measured no test moving by it |
| Q10 | The missile's centre | 01's shift kept, with its flight in 7.13d: the manifest names no sprite size, and a shape would move its sweep |
| Q11 | Older saves' rests and marks | `to_32` keeps a rest as a cooldown of its own length that spends it, needing no kind, and drops the "!" and the `dive` lines; a record its restore strips of `cooldown` rests again from its next attack |
| Q12 | 01's other `PreviousVelocity` readers onto `step_velocity` (the last plan's Q6) | Not taken: no phase-7 row; F1 to Later, so 7.13g's parked seams take it |

Each question takes its stated default; the owner reviews the plan on their own time.

## 5. Invariants

- **May move:** `api/`, the describe census and `DESIGN.md` (Steps 1, 2, 5); the kit tests §2.3 and §2.7 name; 01's pit lines §2.4–§2.9 name, quoted and decomposed; 01's asserted values only as the Moves name them; 01's schema to 32 (Step 3).
- **Must not move:** the physics hashes (`just physics-release`); the kit's determinism pin and schedule pins and the template's; 02's pins; the views guard; the template's tests' outcomes; `src/shell/`, byte-equal to the template's; the perf rows' scripts; every locked family's files; 01's `archetype_moves` and its other pit lines; 01's tests' other outcomes; the template's `AGENTS.md` and the LLM index.

## 6. Stop conditions

- A done-when check fails: restore the step's files from the last commit, mark the step skipped, and continue with steps that don't depend on it: Steps 2 and 4 depend on 1; 4 on 2 and 3; 6 on 5. A skipped step's rows stay `open` at the close, moved to 7.13c's carry, flight and data or a re-plan with the stop's reason; the close's checks count the steps that landed.
- A test whose asserted outcome moves outside §2.2's Moves, or a migration that cannot keep a schema's state: stop the step; never widen a tolerance.
- A change to core, to the kit outside Steps 1 and 5, to a locked family's file or to `src/shell/`: stop the step.
- A pin moves in a line §2.10 holds, or past what its named cause accounts for: stop, decompose the move by probes, record it.
- The template's `AGENTS.md` or the LLM index past its cap: stop the step.

## 7. Critique

Round 1: gpt-6-astra through Codex at xhigh, 2026-11-13, `--repo` on the tree at `815cfc38` with this plan untracked; 537.2 s, 1,999,106 tokens in (1,900,672 cached), 31 commands, 301.3 KB of output, 5h 0 → 34 %, week 51 → 57 %. Five findings, all true; each changed a mechanism (the rests and their migration, the "?", two steps' checks), so round 2 runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] A named foe's patch seldom holds `enemy.kind`, so a rest the migration reads by kind is lost | `crates/tungsten-core/src/save/patch.rs:16-45` (a patch holds changed fields alone); `save/world.rs:300-320` | True (A20) | `to_32` writes a rest as `{seconds: rest, left: rest, remaining: 1}`, needing no kind, and a rest starts as a fresh cooldown of the prefab's seconds (§2.5); the migration test loads a map troll's patch with no `enemy.kind` |
| 2 | [HIGH] A schema-31 record's restore removes the prefab's new `cooldown`, leaving nothing to re-arm | `crates/tungsten-core/src/save/world.rs:1219-1226`; `K/lifecycle/cooldown.rs:316` | True (A20) | A rest inserts its cooldown fresh, over the one there or in its place, and a foe with none is rested; `Cooldown::rearm` dropped from Step 1; the test loads a rested crawler's record with no cooldown |
| 3 | [MEDIUM] The "?"'s `after` counts from the "!"'s end and on through a held machine, where today's reads the search's own time | `src/features/enemies/draw.rs:229-243`; `K/present/looks.rs:1295`; `K/intent/state_machine.rs:212-218` | True | `foe_tags` writes `mark.search` from the machine's search time, as `mark()` reads it (§2.6); `tag_look.after` dropped from Step 1 |
| 4 | [MEDIUM] One pulse emitter under `charge_dust`'s cap of 6 thins the trail its emitter a puff drew | `assets/particles/charge_dust.json` (`max_alive` 6, 5 a puff, up to 0.45 s); `crates/tungsten/src/particles.rs:194-200` | True (A21) | Each pulse config's `max_alive` is its count times the pulses its particles overlap (16, 40), with `tools/test_generate.py`'s pin; `a_beetles_dust_keeps_its_density` holds each peak to 80 % of the base's |
| 5 | [MEDIUM] Step 3's grep over `src` also meets the migration's old field names | Step 3's done-when; `src/save.rs` | True | The grep runs over `src/features` |

Round 2: the same reviewer at xhigh, 2026-11-13, `--repo` on the plan without this section and the log (49,558 characters), with a focus note naming round 1's changes; 361.8 s, 1,277,523 tokens in (1,175,552 cached), 32 commands, 311.5 KB of output, 5h 34 → 61 %, week 57 → 61 %. One finding, true, a gap round 1's "?" fix left; no third round.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] Step 4's `mark.search` has no declaration in its files, and a look's filter tags are checked at the load | `tools/content/data/tags/game.json`; `K/present/looks.rs:1051`; `K/foundation/tags.rs:476` | True | Step 4's Files take both tag catalogues, and §2.6 names the declaration |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |

## Follow-ups

- **F1.** 01's `PreviousVelocity`, read by `crush`, `ball_impacts`, `body_impacts` and `crate_hits`, onto the kit's `step_velocity`, so `toys_velocities` goes (the last plan's Q6): a Later line at the close.

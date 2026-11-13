# 01's carry, flight, snowball and per-kind data on the kit (example-01-carry-flight-data) — draft

- **status:** draft
- **goal:** Phase 7's 7.13c's carry, flight and data (§2.1). In the kit: a hold placed centre to centre, a drop filter on the held body and `carrier_of`; steering's `pace`, its `on` switch and the direction a steered mover would take; a projectile's `seconds`, its own `shape` for walls beside a hitbox, and a hitbox's `live` credit. Then example 01 on them: the troll's carry (S-37), the flyers' flight (S-92), the crawler's snowball as a kit projectile (S-21, schema 33), the foes' brain, sight, machine, shown clip and previous velocity and the bodies' pull shares in their prefabs (S-30), and the foes' kinds, the sprays' looks and the tomes' texts and art as data kinds (S-34, S-30's tomes); its tests unchanged (§2.2) and its older saves loading (§2.11).
- **non-goals:**
  - S-36's throw window: no kit item holds a throw's credit, factor and window across held foes, kept balls and stripped saves, and `Throws` serves 01's own rules (A7); it stays 01's, a Later line (Q2).
  - S-30's grants: no effect grants a `Shield` or sets an air-jump cost (A22); the tomes' grants stay `tomes::apply`, a Later line (Q6). A kind's prefab and icon stay named by the kind (Q7).
  - S-36's hole life and the missile's flight (7.13d); 01's `PreviousVelocity` readers onto `step_velocity` (Later's line); the snowball's interplay as a dynamic body (holes, pushes, lightning), which the flask's port already gave up (§2.8).
  - Core changes; kit changes past Steps 1, 3 and 5; new art or a locked family's file (`D-223`); retuning 01; a sitting as evidence (`D-226`).
- **files to touch:** by step, §3's Files lines; nothing outside them but §6's admitted files. In short: Steps 1, 3 and 5 the kit's carry, steering, mover, projectile and hitbox with their tests, `lib.rs`, the describe census, `api/`, `DESIGN.md`; Steps 2, 4 and 6–8 `examples/01_platformer/` (`src/`, `tests/game/`, `tools/content/` with its `assets/` copies, `assets/manifest.json`, `tools/outputs.json`, `README.md`); Step 9 this plan (deleted), `docs/plans/phase-7.md`, `ROADMAP.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/known-issues.md`.
- **ordered steps:** (1) the kit's centred hold, held drop and `carrier_of`; (2) 01's troll carry on it; (3) the kit's steering pace, switch and steered direction; (4) 01's flyers on steering; (5) the kit's projectile seconds, wall shape and live credit; (6) 01's snowball a kit projectile, schema 33; (7) 01's foe and body components in their prefabs; (8) 01's kinds, spray looks and tomes as data kinds; (9) close.
- **done-when:** every step's done-when passes, quoted in the evidence log; `just check` passes and the demo (§3's Step 9) works; the landed steps' rows read `landed <sha>` in phase 7's §2.2, S-30's grants and S-36's throw window in Later with S-36 `open` for 7.13d, a skipped step's rows moved with their reason (§6); the plan deleted with its citations and `example-01-carry-flight-data` out of Now.

Item `example-01-carry-flight-data` in [ROADMAP.md](../../ROADMAP.md), phase 7's 7.13c's carry, flight and data (§2.8, [phase 7](phase-7.md)), from the redo's 7.13 row and phase 7's Step 13. Written 2026-11-13 at `01ffd6ee`. Runs under the [tungsten-plan skill](../../.claude/skills/tungsten-plan/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs.

## Context digest

- 7.13c's foes closed (`D-278`) having split this plan off by phase 7's §6: S-21, S-30, S-34, S-37, S-92 and S-36's throw window, each with a kit gap in its §2.1 (`git show 20ccde4f:docs/plans/example-01-foes.md`).
- Four audits at `01ffd6ee` corrected that table (A2–A24). The carry needs a centred hold, a held drop and `carrier_of`, but no kit record fits the throw window. Steering needs a pace, a switch and a read for the black hole's blend, not an offset. The snowball needs a projectile's seconds, its own wall shape and a live credit, and 01 writes its thrower's factor into its hitbox. S-30 and S-34 need no kit change: `StateMachine` defaults would break a kit test.
- It stands on `D-246` (the carry), `D-239` and `D-174` (steering on movers), `D-276` and `D-277` (projectile walls, ends, credits), `D-275` (01's saves) and `D-278` (the foes' state tags and rests).
- Rules: tests are the spec (§2.2); no core change; no art moves; `D-226`; code at xhigh; schema 33 at Step 6 (§2.11).
- Later items expect: 7.13d to stand on Step 4's steering and Step 5's projectile; 7.13g's parked seams to take this plan's two Later lines.

## 1. Audit

Read at `01ffd6ee`. Paths under `examples/01_platformer/` are written `src/…`, `tests/…`, `tools/…`, `assets/…`; `K/` is `crates/tungsten-kit/src/`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | Phase 7 sends this plan five rows and a part (S-21, S-30, S-34, S-37, S-92; S-36's throw window); the register's lines predate 7.13c | `docs/plans/phase-7.md:140`, `:164`, `:179`, `:255`, `:256`, `:258`; `docs/plans/seam-register.md:256`, `:265`, `:269`, `:271`, `:272`, `:327` |
| A2 | The kit's carry: `Hold { offset, drop }`, code only; `carry` keeps the body's own `RigidBody`, makes it a sensor and places it at the carrier's point (its `Position`) plus the offset, x negated while its `Facing` points left; `hold_bodies`, closing the follow pass in the move phase, places it each step at the carrier's velocity or lets it go when the carrier is gone, dead or past the drop filter, read on the carrier's tags alone; `carried_by` reads a carrier's body and nothing reads a body's carrier; `Carried`, crate-private, is saved as `carried` on the held body, its carrier by save ID | `K/motion/carry.rs:37-89`, `:95-158`, `:206-246`, `:269-276`, `:313-355` |
| A3 | 01's carry: `pick_up` (intent) ends the object's throw, keeps its body in `Held`, puts `Holding` on the troll and sets the object's bottom at the troll's `hands` point, (74, 2) facing either way, on its body centre's column (74, 70); `holds` (`troll_holds`, move phase after the controllers, the mover and knockback) places it each step in save-ID order, or drops it when the troll is gone, dead, frozen or stunned or a held dummy or foe has thawed; `release` gives the body back, sets its velocity, launches a held controller, locks its input 1 s on a throw and wakes a dynamic body | `src/features/enemies/troll.rs:111-261`, `:440-461`; `src/features/enemies/mod.rs:293-297`; `assets/data/bodies/troll.json` |
| A4 | Measured from the frame's corner, the hold mirrors about it: the hands' x 74 would sit 148 px off facing left; from the centres the offset is (0, −68 − half.y), which mirroring leaves alone; `centre_of` gives a body's collider centre | `K/motion/carry.rs:76-81`, `:269-276`; `K/motion/drive.rs:146-182` |
| A5 | 01 asks a body's carrier at six sites | `src/features/enemies/beetle.rs:181`; `src/features/toys/systems.rs:201`; `src/save.rs:709`; `src/features/enemies/troll.rs:137`, `:164`, `:509` |
| A6 | `Holding` saves on the troll (`troll_hold`, the object by reference) and `rebuild` lifts again at once, so a reload with the clock paused shows a sensor; the kit's loaded hold has no body until its first step with dt > 0, which its own test pins; a kept ball's hold is never captured | `src/features/enemies/components.rs:234-269`; `src/save.rs:700-715`; `K/motion/carry.rs:97-100`, `:343-352`; `K/tests/saved.rs:953-957`; `crates/tungsten-core/src/save/id.rs:62-69`; `tests/game/enemies.rs:2546` |
| A7 | `Throws` holds the troll's throws and the goblin's rocks, 2 s each, counted down in `body_impacts` and ended at a first impact, at 0 or at the body's end; besides the credit, its readers spare a thrower, credit a keg, weigh thrown iron and bar a lift; `ImpactDamage`'s credit has no window and lapses on kept balls; an entity holds one `Cooldown`, a foe's rest's; an effect needs `Effects` on props and fails on balls; a `Lifetime` despawns | `src/features/enemies/troll.rs:281-305`; `src/features/enemies/goblin.rs:502-511`; `src/features/elements/systems.rs:1058-1074`, `:1295-1349`, `:1601-1616`; `K/lifecycle/cooldown.rs:21-35`; `tests/game/saves.rs:3333-3352` |
| A8 | 01's `fly`, the same 20 lines in the bat and the wisp: from the body's centre, `dir = (p − c)/d · pace · min(d/20, 1)` past 1 px, else 0; it sets the mover's direction, faces the point past 0.05 and returns `d ≤ 10`; paces 0.4 to 1 by state; the dive sets its line once and the rests set 0, in the intent phase | `src/features/enemies/bat.rs:56-75`, `:94-165`; `src/features/enemies/wisp.rs:36-55`, `:80-114`; `src/features/enemies/systems.rs:97-115` |
| A9 | Steering on a mover: the mover's own step resolves the target from `Position`, takes `held(v / speed)` whole and writes it back as its direction, the game's ignored; arrive is `unit · min(S, S·d/r)`, `r` 64 by default, landing at `to/dt` once `\|v\|·dt > d`; no target gives 0; no pace, no switch; `steer` and `set_max_speed` crate-private; `move_speed` writes `max_speed` each tick | `K/motion/top_down.rs:504-569`, `:580-600`; `K/motion/steering.rs:115-127`, `:290-372`; `K/foundation/stats.rs:1264-1288` |
| A10 | At radius 20, pace 1 and a point less the body's centre, arrive's direction is `fly`'s but for rounding and `fly`'s 1 px dead zone; its landing never fires at radius 20 (`S·dt/r` ≤ 0.58) | `K/motion/steering.rs:333-372`; `src/features/enemies/bat.rs:61-74` |
| A11 | Each of 01's direction writes comes before the steer: the dive line and the rests in intent, the hole's `toys_hold` blend before `top_down_mover` with `toys_release` putting it back after; a frozen flyer's mover is set aside in `Displaced`, where steering's own system would drive its body; a dormant wisp's position is pinned exactly | `src/features/toys/systems.rs:374-430`; `src/features/toys/mod.rs:192-193`; `src/features/enemies/systems.rs:680-724`; `K/motion/steering.rs:491-533`; `tests/game/enemies.rs:526-536` |
| A12 | Steering's own system walks every archetype holding a `Steering`, ungated, to skip movers; `Steering` is saved; no 01 entity holds one; the kit's pin reads `top_down_mover, steering` | `K/motion/steering.rs:386`, `:491-518`; `crates/tungsten/src/tests/kit.rs:49`; `tests/game/saves.rs:396-405` |
| A13 | The snowball is a dynamic r 5 circle at (8, 8), mass 1, lobbed at `PhysicsConfig.gravity` (0, 3600); `snowballs` (overlap phase) bursts it at the first live target off `team.enemy` within 3 px of its collider (12 cold × the thrower's factor, from the thrower while it lives), else at a tile or a body but the live thrower within 2 px, else when `left −= dt` reaches 0 from 3; a throw past eight bursts the oldest at once; the burst is 01's clip, particles and `SnowballBurst` | `src/features/enemies/snowball.rs:26-209`; `src/features/enemies/crawler.rs:338-365`; `tools/content/prefabs/snowball.json` |
| A14 | The kit's projectile skips a dynamic body; on `linear_motion`, exact under its acceleration, it ends at its hitbox's first hit, a wall (tiles, static colliders and `body_walls`' bodies, never its source's) or its range, set at the contact with `ProjectileExpired`; with a hitbox, its walls take the hitbox's shape; a `Lifetime`'s end sends nothing | `K/overlap/projectile.rs:883-913`, `:1236-1256`; `K/lifecycle/lifetime.rs:343-369`; `K/tests/projectile.rs:883-893` |
| A15 | A hitbox's hit credits its `Source`, else the hitter itself, dead or alive; end blasts and impacts credit through `credits()`, a live entity its health does not call dead, else nobody | `K/overlap/grid.rs:485-505`; `K/chemistry/blast.rs:645-653` |
| A16 | The physics step integrates per substep (4 at 1/240 s), so today's ball falls 7.5 px/s × t below the exact arc `linear_motion` flies: 1.5 px at 12 steps, 2.25–5.25 px at its 0.3–0.7 s aims; the arc test holds it within 2 px of the exact arc for 12 steps | `crates/tungsten-core/src/physics/step/mod.rs:1175-1183`, `:1603-1613`; `K/motion/linear.rs:277-297`; `tests/game/enemies.rs:3586-3613` |
| A17 | The thrower's factor is its `damage` stat over its base, the difficulty's `Mul`, sampled at the throw; `dealt()` reads `damage_dealt` alone, so 01 writes 12 × the factor into the hitbox's parts at the throw | `src/features/enemies/troll.rs:251-258`; `src/features/enemies/mod.rs:325-363`; `K/damage/pipeline.rs:1574-1603` |
| A18 | Only actors hold hurtboxes, each its body's box (the player's and the dummy's equal their colliders), so a radius-8 hitbox meets what 01's 3 px reach met; `hit_grid` skips the dead and the despawning | `assets/prefabs/*.json` (`hurtbox`); `assets/data/bodies/player.json`, `dummy.json`; `DESIGN.md:612` |
| A19 | `KindInfo`: 11 fields in a `const fn` table, 18 calls and 20 field reads in `src/`, 10 and 15 in tests; `prefab` and `icon` feed the const `SUMMONS`; `mass` is restated in the prefabs and checked against it; `recover` reads three Rust constants | `src/features/enemies/mod.rs:116-223`; `src/features/spells/mod.rs:100-155`; `tests/game/data.rs:96-120`, `:233-257` |
| A20 | The foes' `Brain`, `Sight`, `StateMachine<EnemyState>`, `Shown` and `PreviousVelocity` come from the spawn's `with` (one move a summon), the map's foes' from an insert at the run's start (36 moves), a record's `Shown` from `restored`; `brain`, `sight` and `enemy_state` are saved but no prefab component; the kit's machine form requires `elapsed`, `entered` and `held`, and a kit test refuses one without `entered` | `src/features/enemies/mod.rs:246-251`, `:408-442`, `:554-564`; `K/intent/state_machine.rs:76-105`; `K/tests/state_machine.rs:310-312` |
| A21 | A hole's pull share is a chain of markers: iron 0.2, a bowling ball 0.625, scrap 1, a crate 0.5, a keg 0.4, a frozen dummy 0.2 and a thawed one 0, else the inverse mass; a foe's is `KindInfo::pull` | `src/features/toys/systems.rs:254-281`; `src/features/toys/mod.rs:88-111`; `src/features/props/mod.rs:138-139` |
| A22 | The tomes' table is nine `const fn`s (index, prefab, name, gives, costs, icon, take clip and sparks, sound); the HUD shows held kinds after their books go; the grants are Rust, and no effect grants a `Shield` or an air-jump cost | `src/features/tomes/components.rs:20-111`; `src/features/tomes/systems.rs:82-112`; `K/damage/effects.rs:1355-1396` |
| A23 | The spray's look is a private 13-field `Style` for flame and frost, shared constants and the frost's core, read in `start` and `sprites` alone; no test reads it | `src/features/toys/spray_look.rs:20-120`, `:187-265` |
| A24 | A data kind registers before the manifest loads and reads by `DataRegistry::get::<T>(id)`, allocating nothing; the generator's `content` family copies `tools/content/data/<kind>/<id>.json` to `assets/` with its manifest entry | `src/features/level/mod.rs:204`, `:330-340`; `crates/tungsten-core/src/assets/data.rs:219-226`; `tools/generate.py:898-921` |
| A25 | Pins: 01's pit (63,802 allocations, 76,086,646 bytes, 298 moves, 295 spawned, 243 despawned, 17 events), its six foes (two wisps, a bat, a crawler, a beetle, a troll) spawned before its window, the crawler throwing in it (unprobed), the troll lifting nothing; 446 run tests, 1 ignored; schema 32; the template's `AGENTS.md` at 12,277 and the LLM index at 12,278 of 12,288 bytes; 01's census admits `carried` among the kit's names it cannot name | `tests/game/guards/pit.json`; `src/features/run/pit.rs:77-128`; `src/save.rs:91`; `scripts/check-agent-context.py:25-27`; `tests/game/saves.rs:385-418` |

## 2. Design

### 2.1 Scope: what lands, what goes to Later

| Row | Its gap at `01ffd6ee` | The change |
| --- | --- | --- |
| S-37 | The kit's hold mirrors about the corner (A4); it drops by the carrier's tags alone, where 01 drops a thawed held foe; no body's carrier read (A5) | Step 1: `Hold::centred`, `with_held_drop`, `carrier_of`; Step 2 |
| S-36's window | No kit item covers a throw's record (A7) | None: 01's `Throws` stays; a Later line (Q2) |
| S-92 | No pace; no switch for the dive, the rests and the freeze; the hole's blend lost (A9–A11) | Step 3: `pace`, `on`, `steered_direction`, steering's own pass off movers; Step 4 |
| S-21 | A dynamic body (data: bodiless on `linear_motion`, as the flask flies); a lifetime's end sends nothing; one shape for hits and walls; a dead or absent source credited (A13–A15) | Step 5: `projectile.seconds`, its `shape` beside a hitbox, `hitbox.credit`; Step 6. The factor is 01's to write (A17); the cap stays 01's (Q9) |
| S-30 | `brain`, `sight`, `enemy_state` and `shown` no prefab components (A20); pull shares in code (A21); the tomes' texts and art in code (A22) | Step 7, and the tomes in Step 8; the grants to Later (Q6) |
| S-34 | `KindInfo` and the spray's `Style` in code (A19, A23) | Step 8: the `enemy_kinds` and `spray_looks` data kinds |

Nine steps, within §6's ten, so one plan (Q1).

### 2.2 Tests are the spec

- Each step keeps all 446 of 01's run tests passing with their asserted outcomes. A test's body changes only where it reads, spawns or calls what the step removes, ported to the kit's equivalent (`carrier_of` for `Held`, a steering's target for `fly`, the kit's projectile, hitbox and motion for `Snowball`'s fields, the data kinds for `KindInfo`'s and the tomes' constants), each port counted in the evidence row.
- An asserted value moves only where the step's Moves names it, by at most one fixed step in time, 1 % in an amount or 1 px in a place, its old and new value listed; any other changed outcome stops the step (§6). No test is deleted but one whose subject the step deletes.
- New tests only where a done-when names them. The kit's fields take kit tests; they add no genre claim, so no recipe.

### 2.3 The kit's carry (Step 1)

Each defaults to today's behaviour, so the kit's tests, recipes and pins hold:

- **`Hold::centred(self)`:** the offset runs from the carrier's centre to the held body's, each `centre_of`'s (a collider's centre, else the drawn centre, else the point), its x mirrored while the carrier faces left: the held body's `Position` is the carrier's centre plus the faced offset less its own centre's offset from its point, at the carrier's velocity, placed at once by `carry` and each step by `hold_bodies`.
- **`Hold::with_held_drop(filter)`:** let go at rest once the held body's tags pass `filter`, checked before each placement beside the carrier's drop; none by default.
- **`carrier_of(world, body) -> Option<Entity>`:** the carrier holding `body`, the reverse of `carried_by`; exported beside it in `lib.rs` and the prelude.
- **Saved:** `carried` takes `centred` (left out when false) and `held_drop` (left out when none), so a save without them loads as today's.
- Moves: `api/tungsten-kit.txt`; `DESIGN.md`'s carry sentence.

### 2.4 The troll's carry (Step 2)

- **The lift:** `pick_up` ends the object's throw as today, then `carry`s it with `Hold::new(hands − centre − (0, half.y)).centred()` (the troll's `hands` facing right and its body's centre from its bodies data, the object's half height: (0, −68 − half.y) today), `with_drop` `{any: [status.frozen, status.stunned]}` and, on a dummy or a foe, `with_held_drop` `{none: [status.frozen]}`; `Holding` goes on the troll as today.
- **Each step:** the kit places it. `troll_holds` becomes the sync, after the kit's `FOLLOW`: a troll whose `Holding` names a body the kit no longer holds for it loses its `Holding`, so a save never names a let-go body.
- **The release:** `release` and `drop_held` call the kit's `release` from the troll, remove `Holding` and, on a throw, lock the thrown controller's input for `THROWN_LOCK`; the throw's lob, `Throws` and `arm_throw` stay.
- **The reads:** `held` is `carried_by`; `liftable` and the six sites (A5) ask `carrier_of`; `foe_tags`' `troll.holding` reads `carried_by`.
- **Saves:** `troll_hold` stays the hold's record (Q4). `rebuild` releases at rest a body the kit's `carried` brought back held by its troll, then `pick_up` carries it again at once, as today's lift does, so the paused reload shows a sensor (A6) and a kept ball's hold loads too. No schema change: a schema-32 save holds no `carried`, and `rebuild` lifts as today.
- **Census:** the census classifies by type name, and `carried` is only a registry name to it (`tests/game/saves.rs:368-418`, `:843-850`), so a held body's `Carried`, a type no game can name, would sit in no list: `registered()` takes its type name as a literal beside `carried`, saved on an engine record, so its unnamed names become `became` and `spawned`, and the `ball` kind's rebuilt list takes it, a kept ball's hold rebuilt from the troll's `Holding`.
- `Held` and `place` go.
- Moves: the body placed in the follow pass, before knockback (the troll's knockback resists whole); drops in the kit's query order (only their wake order); the centred sum's rounding (under 1e-3 px); the release's zero launch on a re-carried frozen foe, replaced by the throw's launch; the pit's allocations and bytes, should the removed `Held` walk and the sync's `Holding` walk move them.

### 2.5 The kit's steering (Step 3)

Each defaults to today's behaviour:

- **`steering.pace`:** in (0, 1], 1 by default: a share of its top speed, scaling `desired`'s speed and the cap; `set_pace`, `pace()`.
- **`steering.on`:** true by default; off, it steers nothing and writes nothing: a mover it drives moves by its own direction or intent, and its own system passes it; `set_on`, `is_on()`. An off steering is no driver: `driver_of` counts a `Steering` only while it is on (`K/motion/drive.rs:84-96`), so on a body no other driver moves, an oscillator or a knockback drives in its place as `D-174`'s rule has it, the knockback sliding and then writing zero, and a body only it drove keeps its last velocity until the game writes one; the decision says so. 01's flyers keep their mover, and a frozen flyer its borrowed controller, as their driver either way.
- **`steered_direction(world, entity, dt) -> Option<Vec2>`:** the direction a `TopDownMover` would take from its `Steering` at a step of `dt`, from where it stands, as the mover's step computes it, recording nothing; `None` without either or switched off.
- **Its own pass:** its system's query leaves `TopDownMover` holders out, which its loop skipped, so steering on a mover costs that system no walk (A12).
- **Saved:** `pace` and `on` left out at their defaults.
- Moves: the describe census (`pace`, `on`); `api/`; `DESIGN.md`'s steering sentence and its write rule's driver list.

### 2.6 The flyers' flight (Step 4)

- **Prefabs:** the bat's and the wisp's `steering` `{target: {point: [0, 0]}, mode: arrive, max_speed: <its move_speed>, radius: 20, on: false}`, off until a state flies it.
- **`fly`:** one `systems::fly(world, flyer, point, pace)` replaces the two: with a point, the steering's target is the point less the body's centre, its pace `pace`, on; it faces the point while `steered_direction`'s x passes 0.05; it returns `d ≤ ARRIVED` from the centre. With none, `rest`.
- **Rests and the dive:** `rest` also switches a steering off, and the held rest each step, so a frozen flyer's is off; `enter_attack` switches it off and sets the line as today.
- **The hole:** `toys_hold` takes a steered flyer's own direction from `steered_direction`, sets the blend and switches the steering off for the step; `toys_release` puts its own direction back and switches it on again. An unsteered flyer, diving or resting, blends its mover's direction as today.
- **Saves:** `enemies::restored` gives a flyer record its prefab's steering when its save held none (a schema-32 record's restore strips the prefab's), read from the prefab as `disarm_as_prefab` reads its health; the next intent step sets it on or off. No schema change.
- Moves: a flyer within 1 px of its point creeps onto it; rounding (under 1e-3 px a step); the pit's allocations and bytes (the flyers' steering reads and archetypes).

### 2.7 The kit's projectile and hitbox (Step 5)

Each defaults to today's behaviour:

- **`projectile.seconds`:** (> 0), none by default: its seconds left fall by `left −= dt` at each projectile step from its spawn's, and at 0 or below it ends as at its range's end: `ProjectileExpired`, `expired`, its `end` set off; its left seconds saved with it.
- **`projectile.shape` beside a hitbox:** its own shape measures its walls; with none, its hitbox's (today's). No projectile in the kit's tests and recipes, the template or 01 sets both.
- **`hitbox.credit`:** `source` (today's: its `Source`, else itself) or `live`: its `Source` while `credits()` passes, else nobody, never itself.
- Moves: the describe census (`seconds`, `credit`); `api/`; `DESIGN.md`'s hits paragraph.

### 2.8 The snowball (Step 6)

- **The prefab:** bodiless: its animation; `linear_motion` `{velocity: [0, 0], acceleration: [0, 3600]}`; `projectile` `{seconds: 3, shape: {circle: 7}, offset: [8, 8], body_walls}` with the flask's layers; `hitbox` `{shape: {circle: 8}, offset: [8, 8], damage: {damage.cold: 12}, filter: {none: [team.enemy]}, credit: live}`; `snowball: {}`.
- **The throw:** past eight in flight it bursts the oldest as today, spawns the ball as a run record, sets its motion's velocity and `PrevPosition` as the flask's spawn does, its `Source` the crawler, and its hitbox's cold 12 × the factor.
- **The end:** `snowball_ends`, in the overlap phase after `PROJECTILE_TICK`, shows 01's burst at each snowball's `ProjectileExpired` point plus (8, 8), as `flask_ends` does; the cap's burst stays 01's (Q9). `snowballs`, `gap`, `live`, `SNOWBALL_REACH`, `SNOWBALL_MARGIN` and `SNOWBALL_LIFE` go.
- **`Snowball`:** a marker its prefab names, rebuilt; its thrower is the `Source`, its factor the hitbox's part, its seconds the projectile's.
- **Saves:** schema 33: `to_33` drops each saved snowball, a dynamic body's record a load could not fly (Q8).
- Moves: the arc, now exact, so a hit, a burst or a range end may move by a step (A16); a hit at the hitbox's swept contact and a wall at the 7 px shape's, where 01 tested the step's end; a target and a wall met in one step, the one the sweep meets first; no burst on another snowball or on its dead thrower; no hole pull, push or lightning on a ball in flight, as the flask's; an older save's balls in flight dropped (eight at most, 3 s at most); `Snowball`'s saved form; `archetype_moves` by the throw's one insert, should it move; the pit's allocations, bytes, `spawned`, `despawned` and `events_sent`, should the throw land in its window (A25).

### 2.9 Components in the prefabs (Step 7)

- **The foes:** 01 registers `brain`, `sight` (with `#[serde(default)]`), `enemy_state` (the kit's save form, `{state: patrol, elapsed: 0, entered: false, held: false}`, equal to `StateMachine::new(EnemyState::Patrol)`'s), `shown` and `previous_velocity` (each `{}`, a fresh one, the workspace's glam having no serde) as prefab components; the seven foe prefabs name them; `spawn_record`'s `with` keeps `SceneEntity` alone; the run start's insert and `restored`'s `Shown` go (Q10).
- **The pull:** a 01 component `pull` `{share, frozen}`: iron 0.2, the bowling ball 0.625, scrap 1, the crate 0.5, the keg 0.4, the dummy `{share: 0, frozen: 0.2}`; `pull_response` reads it, else the inverse mass; the five constants go.
- **Saves:** the saved names and forms hold; a record's puts replace the prefab's; `shown`, `previous_velocity` and `pull` are rebuilt, so the census lists take `Pull`.
- Moves: the map foes' 36 run-start moves; a restored record's `Shown` move; the pit's allocations and bytes (the archetypes the foes no longer pass through, the bodies' new column).

### 2.10 Data kinds (Step 8)

- **`enemy_kinds`:** six entries by kind, `strike`, `mass`, `sight`, `alert`, `pull`, `wave`, `ice`, `flyer`, `recover`, `KindInfo` its type; `enemies::info(world, kind)` reads it; `EnemyKind::info()` and the recover constants it read go; a kind's prefab and icon stay its name (`enemies::prefab`, `enemies::icon`) for `SUMMONS` (Q7).
- **`spray_looks`:** `flame` and `frost`, the `Style` fields with the frost's `core`; `style` reads them.
- **`tomes`:** `pact`, `bounding` and `arcane`, each `name`, `gives`, `costs`, `icon`, `take_clip` and `take_sparks`; `TomeKind`'s six const fns go; its index, prefab and sound stay (Q11).
- Each registers before the manifest loads; the files go in `tools/content/data/<kind>/`.
- **Checked at the load:** each type deserializes through a checked form, since a data kind's load only deserializes (`crates/tungsten-core/src/assets/data.rs:173-180`) and the spray's puff loop never ends on an `every` of 0 or below (`src/features/toys/spray_look.rs:230-237`): every number finite; a spray's `every` at least 0.001 s, so the `f32` accumulator falls at each puff and an update's puffs stay bounded (a smaller one, 1e-12, rounds `due − every` back to `due` and never ends), its `life`, `speed`, sizes and core's `tall` and `frame_ms` above 0; a kind's `mass` above 0 and its other numbers at least 0. A refused entry fails the load with its field.
- Moves: none in play; the pit holds (reads that allocate nothing), else decomposed.

### 2.11 Pins, guards, saves and records

- **Unchanged:** the physics hashes; the kit's determinism pin and its schedule pins and the template's (no system is added); 02's pins; the views guard; the template's tests' outcomes; `src/shell/`.
- **Moved by design:** `api/` and `DESIGN.md` (Steps 1, 3, 5), the describe census (Steps 3, 5).
- **01's pit:** a step may move the lines §2.4–§2.10 name, each quoted with its cause and decomposed by probe (the guard's size histogram and per-size backtraces); a move its cause does not account for, or any other line moving, stops the step (§6). `archetype_moves` holds but for Step 6's throw.
- **Saves:** schema 33 at Step 6 with `to_33` and its test. Step 2's `carried` fields and Step 4's steering need none: a save without them loads as today's (§2.4, §2.6).
- **Census:** Step 2 names `Carried`'s type, saved on a record and rebuilt on a ball (§2.4); Step 6 drops `Snowball` from the registered list and adds it to the snowball's rebuilt one; Step 7 adds `Pull` to the bodies' lists.
- **Records:** 01's README rows each step makes wrong; the template's `AGENTS.md` and the LLM index unchanged (no kit component, data kind or cue event, A25); `D-27[9]` at the close with the tungsten-decision skill, the next ID rechecked then.

## 3. Steps

Every code step's done-when adds: `cargo fmt --all -- --check` → exit 0; `just lint` → exit 0; `just repo-check` → exit 0. Every kit step's adds: `cargo test -p tungsten --lib --locked -- kit_tests:: kit_recipes::` → exit 0, the determinism pin unchanged; `cargo test -p tungsten-template-basic --locked` → exit 0; `cargo test -p example-02-bench guards_` → exit 0, no pin rewritten; from `examples/01_platformer`, `cargo test --test game` → exit 0, every test passing, its pit pin unchanged; `just ctx`, `just docs-ratchet` → exit 0. Every 01 step's adds, from `examples/01_platformer`: `cargo test --test game` → exit 0, 446 passed before the step's new tests, 1 ignored, the Moves named; `cargo clippy --all-targets -- -D warnings` → exit 0; `just level-check` → exit 0 where `tools/` changes, every locked family holding (Pillow in `target/e01s/venv`, run alone in the background); `TUNGSTEN_GUARDS_WRITE=1 cargo test --test game -- guards::` then `cargo test --test game -- guards::` → exit 0, no line but §2.11's moved, each quoted with its cause.

### Step 1: The kit's centred hold, held drop and `carrier_of`

- **Files:** `crates/tungsten-kit/src/motion/carry.rs`, `crates/tungsten-kit/src/tests/carry.rs`, `crates/tungsten-kit/src/lib.rs`, `api/tungsten-kit.txt`, `DESIGN.md`.
- **Change:** §2.3.
- **Done-when:** `cargo test -p tungsten-kit --lib --locked -- motion::carry::` → exit 0 with these among the run: `a_centred_hold_sets_centre_over_centre_facing_either_way` (a carrier and a body with offset colliders: the body's centre at the carrier's centre plus the offset facing right, x mirrored facing left, a zero x the same either way), `a_held_drop_lets_go_by_the_held_bodys_tags` (let go at rest the step its tags pass, the carrier's drop beside it), `carrier_of_reads_a_held_bodys_carrier` (none before, the carrier while held, none after the release) and `a_centred_held_drop_hold_saves_and_loads` (both fields back after a save, a hold without them saving as before); the kit's whole suite with its doctests → exit 0; the kit step's checks; `just api` then `git diff --stat api/` → `api/tungsten-kit.txt` alone.
- **Commit:** `feat(kit): a centred hold, a held body's drop and carrier_of`
- **Moves:** `api/`; `DESIGN.md`'s carry sentence.

### Step 2: 01's troll carry on the kit

- **Files:** `examples/01_platformer/src/features/enemies/troll.rs`, `examples/01_platformer/src/features/enemies/components.rs`, `examples/01_platformer/src/features/enemies/mod.rs`, `examples/01_platformer/src/features/enemies/systems.rs`, `examples/01_platformer/src/features/enemies/beetle.rs`, `examples/01_platformer/src/features/toys/systems.rs`, `examples/01_platformer/src/save.rs`, `examples/01_platformer/tests/game/enemies.rs`, `examples/01_platformer/tests/game/saves.rs`, `examples/01_platformer/tests/game/guards/pit.json`, `examples/01_platformer/README.md`.
- **Change:** §2.4; the tests' `Held` reads ported to `carrier_of`.
- **Done-when:** the common checks, these among the run: `enemies::a_save_while_the_troll_holds_iron_loads_with_the_hold` and `enemies::killing_or_evicting_the_troll_drops_what_it_holds` unchanged; a new `enemies::the_trolls_load_rides_over_its_centre_facing_either_way` (iron held facing right and left: its centre within 1e-3 px of the troll's `hands` point less its half height); a new `enemies::a_troll_drops_a_held_foe_that_thaws` (a frozen crawler held: let go at rest the step its `status.frozen` goes, the troll's `Holding` gone that step); a new `saves::an_older_saves_hold_loads_held` (a capture with each `carried` stripped, schema 32's shape, loaded with the clock paused: the iron a sensor at the hands, the troll's `Holding` naming it); a new `saves::held_bodies_are_saved_rebuilt_or_presentation` (one troll holding iron, an engine record, and one holding a bowling ball, a kept ball: every component on each held body in exactly one home by the census's rules, `Carried` among them); `rg -q '\bHeld\b' src tests/game/enemies.rs; test $? -eq 1` → exit 0; `rg -q 'fn place\(' src/features/enemies/troll.rs; test $? -eq 1` → exit 0.
- **Commit:** `feat(example-01): the troll's carry on the kit's hold`
- **Moves:** §2.4's.

### Step 3: The kit's steering pace, switch and steered direction

- **Files:** `crates/tungsten-kit/src/motion/steering.rs`, `crates/tungsten-kit/src/motion/top_down.rs`, `crates/tungsten-kit/src/motion/drive.rs`, `crates/tungsten-kit/src/tests/steering.rs`, `crates/tungsten-kit/src/tests/top_down.rs`, `crates/tungsten-kit/src/tests/describe.rs`, `crates/tungsten-kit/src/lib.rs`, `api/tungsten-kit.txt`, `api/tungsten-kit-components.txt`, `DESIGN.md`.
- **Change:** §2.5.
- **Done-when:** `cargo test -p tungsten-kit --lib --locked -- motion::steering:: motion::top_down::` → exit 0 with these among the run: `a_steerings_pace_scales_its_arrive` (a mover steered at pace 0.5 moves half as far a step as at 1, its direction half as long, landing on its point all the same), `a_steering_switched_off_leaves_its_mover_and_its_body_alone` (off: a mover moves by the direction the game set, and a body without a mover keeps the velocity another system wrote; on again, it steers), `steered_direction_is_the_movers_next_direction` (equal to the direction the mover holds after its next step, `None` off or without a steering), `an_off_steering_is_no_driver` (a kinematic body with an off steering: its oscillator writes its velocity; a knockback slides it, then its settling step writes zero; switched on, the steering drives it again) and `pace_and_on_save_and_load`; the kit's whole suite with its doctests → exit 0, the describe census moved by `pace` and `on` alone, quoted; the kit step's checks; `just api` then `git diff --stat api/` → the two kit files alone.
- **Commit:** `feat(kit): steering's pace and switch, a mover's steered direction`
- **Moves:** the describe census; `api/`; `DESIGN.md`'s steering sentence.

### Step 4: 01's flyers on steering

- **Files:** `examples/01_platformer/src/features/enemies/bat.rs`, `examples/01_platformer/src/features/enemies/wisp.rs`, `examples/01_platformer/src/features/enemies/systems.rs`, `examples/01_platformer/src/features/enemies/mod.rs`, `examples/01_platformer/src/features/toys/systems.rs`, `examples/01_platformer/src/features/toys/components.rs`, `examples/01_platformer/tools/content/prefabs/bat.json`, `examples/01_platformer/tools/content/prefabs/wisp.json`, `examples/01_platformer/assets/prefabs/bat.json`, `examples/01_platformer/assets/prefabs/wisp.json`, `examples/01_platformer/tests/game/enemies.rs`, `examples/01_platformer/tests/game/saves.rs`, `examples/01_platformer/tests/game/guards/pit.json`, `examples/01_platformer/README.md`.
- **Change:** §2.6.
- **Done-when:** the common checks, these among the run unchanged: `enemies::an_enemy_far_from_the_player_stops_thinking`, `enemies::the_wisp_chases_on_sight_searches_the_last_seen_spot_then_returns`, `enemies::the_bat_roosts_until_it_sees_the_player`, `enemies::the_bat_flashes_dives_in_its_window_and_climbs_back`, `enemies::a_hole_holds_a_bat_and_a_wisp_about_its_core`, `enemies::a_bat_diving_through_a_hole_dives_on_its_own_way_after` and `saves::a_flyer_saved_frozen_mid_flight_thaws_into_its_saved_flight`; a new `saves::an_older_saves_flyers_take_their_steering` (a capture with a summoned bat's and wisp's `steering` stripped, loaded: each holds its prefab's and the bat climbs back within 10 px of its roost, as an unstripped load does); `rg -c 'fn fly\(' src/features/enemies` → `src/features/enemies/systems.rs:1` alone; `jq -e '.components.steering | .radius == 20 and .on == false and .mode == "arrive"' assets/prefabs/bat.json assets/prefabs/wisp.json` → `true` for each.
- **Commit:** `feat(example-01): the flyers' flight on the kit's steering`
- **Moves:** §2.6's.

### Step 5: The kit's projectile seconds, wall shape and live credit

- **Files:** `crates/tungsten-kit/src/overlap/projectile.rs`, `crates/tungsten-kit/src/overlap/hit.rs`, `crates/tungsten-kit/src/overlap/grid.rs`, `crates/tungsten-kit/src/tests/projectile.rs`, `crates/tungsten-kit/src/tests/hit.rs`, `crates/tungsten-kit/src/tests/describe.rs`, `crates/tungsten-kit/src/lib.rs`, `api/tungsten-kit.txt`, `api/tungsten-kit-components.txt`, `DESIGN.md`.
- **Change:** §2.7.
- **Done-when:** `cargo test -p tungsten-kit --lib --locked -- overlap::projectile:: overlap::hit:: overlap::grid::` → exit 0 with these among the run: `a_projectiles_seconds_end_it_as_its_range_does` (at 1/60 s, seconds 0.5 ends it at the step the test's own `left −= dt` from the spawn's step first reaches 0 or below, with `ProjectileExpired` and its end's blast; saved at step 10 and loaded, it ends at the same step), `a_projectiles_own_shape_meets_its_walls_beside_a_hitbox` (a radius-8 hitbox and a radius-7 shape: it ends where the 7 px circle touches the wall, and hits a target the 8 px one meets) and `a_live_credit_credits_a_live_source_alone` (a hit from a live source credits it; from a dead one, a despawned one or none, nobody, never the hitbox); the kit's whole suite with its doctests → exit 0, the describe census moved by `seconds` and `credit` alone, quoted; the kit step's checks; `just api` then `git diff --stat api/` → the two kit files alone.
- **Commit:** `feat(kit): a projectile's seconds and own wall shape, a live credit`
- **Moves:** the describe census; `api/`; `DESIGN.md`'s hits paragraph.

### Step 6: 01's snowball a kit projectile

- **Files:** `examples/01_platformer/src/features/enemies/snowball.rs`, `examples/01_platformer/src/features/enemies/crawler.rs`, `examples/01_platformer/src/features/enemies/components.rs`, `examples/01_platformer/src/features/enemies/mod.rs`, `examples/01_platformer/src/save.rs`, `examples/01_platformer/tools/content/prefabs/snowball.json`, `examples/01_platformer/assets/prefabs/snowball.json`, `examples/01_platformer/tests/game/enemies.rs`, `examples/01_platformer/tests/game/saves.rs`, `examples/01_platformer/tests/game/data.rs`, `examples/01_platformer/tests/game/guards/pit.json`, `examples/01_platformer/README.md`.
- **Change:** §2.8; the tests' `Snowball` reads ported to the projectile, the hitbox, the motion (`LinearMotion::velocity` for the arc's launch) and the `Source`, the data test's radius to the projectile's shape.
- **Done-when:** the common checks, these among the run: `enemies::a_dark_crawler_snowballs_the_player_on_its_arc_then_cools_four_seconds`, `enemies::a_snowball_bursts_at_its_first_touch_of_the_ground_a_wall_and_a_crate_hurting_nothing`, `enemies::a_snowball_on_a_dummy_chilled_near_its_freeze_rung_freezes_it` and `saves::a_snowball_in_flight_and_a_cooling_crawler_save_and_load_and_it_lands_as_it_would_have`, ported; a new `enemies::a_dead_crawlers_snowball_credits_nobody` (the crawler killed in its ball's flight: the player's cold from no source); a new `saves::an_older_saves_snowballs_in_flight_are_dropped` (a capture rewritten to schema 32's shape with a ball in flight, loaded through `to_33`: no snowball, the crawler's rest kept); `rg -q 'const SCHEMA: u32 = 33;' src/save.rs` → exit 0; `rg -q 'fn (snowballs|gap|live)\b' src/features/enemies/snowball.rs; test $? -eq 1` → exit 0; `rg -q 'SNOWBALL_(REACH|MARGIN|LIFE)' src tests; test $? -eq 1` → exit 0; `jq -e '.components | (has("rigid_body") | not) and .projectile.seconds == 3 and .hitbox.credit == "live"' assets/prefabs/snowball.json` → `true`.
- **Commit:** `feat(example-01)!: the crawler's snowball a kit projectile`
- **Moves:** §2.8's; schema 33.

### Step 7: 01's foe and body components in their prefabs

- **Files:** `examples/01_platformer/src/features/enemies/mod.rs`, `examples/01_platformer/src/features/enemies/components.rs`, `examples/01_platformer/src/features/toys/mod.rs`, `examples/01_platformer/src/features/toys/components.rs`, `examples/01_platformer/src/features/toys/systems.rs`, `examples/01_platformer/src/features/props/mod.rs`, `examples/01_platformer/src/save.rs`, `examples/01_platformer/tools/content/prefabs/` (the seven foes), `examples/01_platformer/tools/content/prefabs/iron_block.json`, `examples/01_platformer/tools/content/prefabs/bowling_ball.json`, `examples/01_platformer/tools/content/prefabs/iron_scrap.json`, `examples/01_platformer/tools/content/prefabs/crate.json`, `examples/01_platformer/tools/content/prefabs/keg.json`, `examples/01_platformer/tools/content/prefabs/dummy.json`, `examples/01_platformer/assets/prefabs/` (their thirteen copies), `examples/01_platformer/tests/game/enemies.rs`, `examples/01_platformer/tests/game/brick.rs`, `examples/01_platformer/tests/game/saves.rs`, `examples/01_platformer/tests/game/guards/pit.json`, `examples/01_platformer/README.md`.
- **Change:** §2.9.
- **Done-when:** the common checks, `saves::every_component_on_a_saved_entity_is_saved_rebuilt_or_presentation` among the run, and a new `enemies::a_foes_brain_sight_and_machine_come_from_its_prefab` (a summoned foe and a map foe each holding `Brain::default()`, `Sight::default()` and `StateMachine::new(EnemyState::Patrol)` at its spawn, the run's start inserting none); `rg -q 'StateMachine::new\(EnemyState::Patrol\)' src; test $? -eq 1` → exit 0; `rg -q '(IRON|BOWLING|SCRAP|CRATE|KEG)_PULL' src tests; test $? -eq 1` → exit 0; `jq -e '.components.enemy_state == {"state": "patrol", "elapsed": 0, "entered": false, "held": false}' assets/prefabs/{wisp,crawler,beetle,bat,troll,troll_armoured,goblin}.json` → `true` for each; `jq -e '.components.pull == {"share": 0, "frozen": 0.2}' assets/prefabs/dummy.json` → `true`.
- **Commit:** `feat(example-01): the foes' minds and the bodies' pull shares in their prefabs`
- **Moves:** §2.9's.

### Step 8: 01's kinds, spray looks and tomes as data kinds

- **Files:** `examples/01_platformer/src/features/enemies/mod.rs`, `examples/01_platformer/src/features/enemies/systems.rs`, `examples/01_platformer/src/features/enemies/beetle.rs`, `examples/01_platformer/src/features/enemies/troll.rs`, `examples/01_platformer/src/features/enemies/goblin.rs`, `examples/01_platformer/src/features/spells/mod.rs`, `examples/01_platformer/src/features/lightning/mod.rs`, `examples/01_platformer/src/features/combat/draw.rs`, `examples/01_platformer/src/features/toys/systems.rs`, `examples/01_platformer/src/features/toys/wave.rs`, `examples/01_platformer/src/features/toys/spray_look.rs`, `examples/01_platformer/src/features/toys/mod.rs`, `examples/01_platformer/src/features/tomes/components.rs`, `examples/01_platformer/src/features/tomes/systems.rs`, `examples/01_platformer/src/features/tomes/mod.rs`, `examples/01_platformer/src/features/spells/hud.rs`, `examples/01_platformer/tools/content/data/enemy_kinds/` (six, new), `examples/01_platformer/tools/content/data/spray_looks/` (two, new), `examples/01_platformer/tools/content/data/tomes/` (three, new), `examples/01_platformer/assets/data/enemy_kinds/`, `examples/01_platformer/assets/data/spray_looks/`, `examples/01_platformer/assets/data/tomes/` (their copies, new), `examples/01_platformer/assets/manifest.json`, `examples/01_platformer/tools/outputs.json`, `examples/01_platformer/tests/game/main.rs`, `examples/01_platformer/tests/game/data.rs`, `examples/01_platformer/tests/game/enemies.rs`, `examples/01_platformer/tests/game/lightning.rs`, `examples/01_platformer/tests/game/tomes.rs`, `examples/01_platformer/tests/game/saves.rs`, `examples/01_platformer/tests/game/guards/pit.json`, `examples/01_platformer/README.md`.
- **Change:** §2.10; the tests' `info()` and `TomeKind` text reads ported to the data, `data.rs`' restated constants checked against the kinds' entries.
- **Done-when:** the common checks, `data::every_restated_constant_matches_its_data` among the run with its kinds' masses read from `enemy_kinds`; a new `data::each_enemy_kind_has_its_data` (six entries, one a kind, each value today's table's); a new `data::a_spray_look_or_a_kind_out_of_range_is_refused` (a spray look's `every` at 0, at −0.01 and at 1e-12, a core's `frame_ms` at 0, a kind's `mass` at 0 and a non-finite `sight`, each refused by its type's deserialization with the field named; today's entries accepted); `rg -q 'const fn info\(' src/features/enemies/mod.rs; test $? -eq 1` → exit 0; `rg -q 'const (FLAME|FROST): Style' src/features/toys/spray_look.rs; test $? -eq 1` → exit 0; `rg -q 'fn (name|gives|costs|icon|take_clip|take_sparks)\(self\)' src/features/tomes/components.rs; test $? -eq 1` → exit 0; `jq -e '.sight == 440 and .flyer == true and .recover == 0.5' assets/data/enemy_kinds/bat.json` → `true`; `jq -e '.reach == 160 and .core.clip == "ex10_frost_core"' assets/data/spray_looks/frost.json` → `true`; `jq -e '.name == "The Demonic Pact"' assets/data/tomes/pact.json` → `true`.
- **Commit:** `feat(example-01): the foes' kinds, the sprays' looks and the tomes as data`
- **Moves:** §2.10's.

### Step 9: Close

- **Files:** this plan (deleted), `docs/plans/phase-7.md`, `ROADMAP.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/known-issues.md`.
- **Change:** `D-27[9]` with the tungsten-decision skill (§2.3, §2.5, §2.7; amending `D-174`, `D-239`, `D-246`, `D-276` and `D-277`; 01's carry, flight, snowball and per-kind data on them at schema 33; the next ID rechecked); phase 7's §2.2: S-21, S-30 (its grants in Later), S-34, S-37 and S-92 `landed`, S-36 `open` with its throw window in Later and its hole's life for 7.13d, a skipped step's rows `open` with its move; its evidence row "13 (7.13), part c's fourth", naming the plan's evidence log at its last commit; ROADMAP: Later's lines for a kit throw record (S-36's window) and for effects that grant a shield and an air-jump cost (S-30's grants), Now's phase-7 line naming 7.13d next; the other follow-ups to known issues or Later; the plan and its citations deleted; `example-01-carry-flight-data` out of Now.
- **Done-when:** `just check` → exit 0; `just physics-release` → exit 0, the hashes unchanged; `just smoke` → passes; `just api-check`, `just docs-ratchet`, `just repo-check`, `just ctx`, `git diff --check` → exit 0; before the deletion, `rg -c '^\| (S-21|S-30|S-34|S-37|S-92) \| [^|]+ \| 7\.13 \| landed ' docs/plans/phase-7.md` → one row for each step that landed, the evidence log naming the rest; after it, `rg -l 'plans/example-01-carry-flight-data\.md' --glob '!CHANGELOG.md' .; test $? -eq 1` → exit 0; `WGPU_BACKEND=vulkan just perf sitting --quick` → `e01-pit` quoted as information, not evidence; the demo: from `examples/01_platformer`, `tungsten run --script` on a scratch script (`start_run`; `wake_foes`; frames until the crawler's ball flies, `count` of `snowball` 1 or more, with a `capture`; two `capture`s a second apart of the woken bat and wisps, the flyers' flight shown by the sheet and proved by Step 4's tests, since no probe names a flyer (`count` refuses the kit's definition-registered components and `tagged` returns any foe's state); `place_player` 300 px from the troll on its deck and `cast_at` an iron block 100 px from the troll toward the player; frames until the `tagged` probe of `troll.holding` passes, with a `capture`; `reload_run`; `tagged` of `troll.holding` and `count` of `enemy` again) → exit 0, its asserts passed: a ball in flight, the hold seen and kept across the load, the World's 42 foes the same across it, its captures the sheet under `perf-runs/20261113-carry-flight-data/`.
- **Ends with:** the commits, the evidence, what the owner should try (01 plays as before: the troll lifting and throwing what it may, a frozen foe it holds dropped as it thaws, the bat's roost, dive and climb and the wisps' drift on the kit's steering, the black hole holding both, the crawler's snowball on its arc and its burst, the tomes' panels, the sprays' looks), the next prompt (7.13d's plan).

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | One plan of nine steps, or a split | One plan: nine steps, within §6's ten |
| Q2 | S-36's throw window: a kit record now, or 01's `Throws` kept | Kept, a Later line: no kit item covers the credit, factor and window across held foes, kept balls and stripped saves, and moving the countdown alone leaves the record (A7) |
| Q3 | The carry's mirror: centre to centre, or an unmirrored offset | Centre to centre: the hands sit on the body's centre column, so the mirror keeps them, and a carrier whose hold is off-centre mirrors about its body |
| Q4 | The hold's save: the kit's `carried` alone, or 01's `troll_hold` kept | Kept: a kept ball's hold is never captured, and the paused reload needs the lift at once (A6) |
| Q5 | The hole's blend: a kit read, or 01's arrive formula kept for it | The kit's `steered_direction`, so 01 keeps no arrive formula and the blend reads what the mover would take |
| Q6 | S-30's grants as effects | Later: no effect grants a `Shield` or an air-jump cost, and moving only the stat lines would rename their saved sources (A22) |
| Q7 | A kind's prefab and icon: data, or its name | Its name: `SUMMONS` is a const table, and a kind's data is found by that name, as the dummy's, the crate's and the keg's are named there |
| Q8 | An older save's snowballs in flight: migrated to the kit's forms, or dropped | Dropped, at most eight for at most 3 s: a migration would restate the kit's saved projectile and hitbox forms, and `to_32` dropped the "!" the same way |
| Q9 | The snowball's cap: the kit's `cap`, or 01's hand burst | 01's: a kit cull lets its victim fly through its step and sends `culled`, not the burst, and `cap` admits the kit's routes alone (A14) |
| Q10 | The machine's prefab form: kit defaults, or 01 naming its save form | 01 names the save form: a kit default would break `K/tests/state_machine.rs:310-312` (A20) |
| Q11 | The tomes' texts: a data kind, or the book prefab's `tome` component | A data kind: the HUD shows held kinds after their books go (A22) |

Each question takes its stated default; the owner reviews the plan on their own time.

## 5. Invariants

- **May move:** `api/`, `DESIGN.md` and the describe census (Steps 1, 3, 5); the kit tests §2.3, §2.5 and §2.7 name; 01's pit lines §2.4–§2.10 name, quoted and decomposed; 01's asserted values only as the Moves name them; 01's schema to 33 (Step 6).
- **Must not move:** the physics hashes (`just physics-release`); the kit's determinism pin and schedule pins and the template's; 02's pins; the views guard; the template's tests' outcomes; `src/shell/`, byte-equal to the template's; the perf rows' scripts; every locked family's files; 01's `archetype_moves` but Step 6's and its other pit lines; 01's tests' other outcomes; the template's `AGENTS.md` and the LLM index.

## 6. Stop conditions

- A done-when check fails: restore the step's files from the last commit, mark the step skipped, and continue with steps that don't depend on it: Step 2 depends on 1, 4 on 3, 6 on 5; 7 and 8 on none. A skipped step's rows stay `open` at the close, moved to a re-plan with the stop's reason; the close's checks count the steps that landed.
- A test whose asserted outcome moves outside §2.2's Moves, or a migration that cannot keep a schema's state: stop the step; never widen a tolerance.
- A change to core, to the kit outside Steps 1, 3 and 5, to a locked family's file or to `src/shell/`: stop the step.
- A pin moves in a line §2.11 holds, or past what its named cause accounts for: stop, decompose the move by probes, record it.
- The template's `AGENTS.md` or the LLM index past its cap: stop the step.

## 7. Critique

Round 1: gpt-6-astra through Codex at xhigh, 2026-11-13, `--repo` on the tree at `01ffd6ee` with this plan untracked (52,140 characters); 400.7 s, 1,910,588 tokens in (1,787,520 cached), 35 commands, 415 KB of output, 5h 0 → 36 %, week 65 → 71 %. Three findings, all true; two changed done-when checks (Steps 2 and 8) and one the demo's, so round 2 runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [MEDIUM] Moving the spray's `every` into data opens a loop that never ends at 0 or below, and a data kind's load checks no range | `src/features/toys/spray_look.rs:230-237` (`while look.due >= style.every`); `crates/tungsten-core/src/assets/data.rs:173-180` (the parse only deserializes) | True | §2.10: each data kind's type deserializes through a checked form (finite numbers; the spray's interval, life, speed, sizes and core above 0; a kind's mass above 0); Step 8 takes `data::a_spray_look_or_a_kind_out_of_range_is_refused` |
| 2 | [MEDIUM] The census's `carried` admits a registry name only; it classifies by type name, so a held body's crate-private `Carried` sits in no list, and today's census holds no lift | `tests/game/saves.rs:368-418`, `:843-850` (`registered` from listed public types; homes by type name) | True | §2.4: `registered()` takes `Carried`'s type name as a literal, saved on a record, and the `ball` kind's rebuilt list takes it; Step 2 takes `saves::held_bodies_are_saved_rebuilt_or_presentation`, a held record and a held kept ball |
| 3 | [LOW] The demo's `tagged` of `state.chase` passes on any chasing foe, so it cannot show a flyer flying | `src/features/enemies/systems.rs:826` (every kind's state tag); `crates/tungsten-kit/src/foundation/tags.rs:657-670` (the first holder, no kind); `crates/tungsten-core/src/script.rs:1497-1501` (`count` refuses definition-registered components) | True | Step 9's demo drops the assert: two captures a second apart show the woken flyers, and Step 4's tests prove the flight; no probe names a flyer, and a game probe would be new code at the close |

Round 2: the same reviewer at xhigh, 2026-11-13, `--repo` on the plan without this section and the log (53,561 characters), with a focus note naming round 1's three changes; 287.8 s, 1,554,301 tokens in (1,457,408 cached), 28 commands, 301.1 KB of output, 5h 36 → 64 %, week 71 → 75 %. Two findings, both true; the first is a gap round 1 did not reach, the second one round 1's range fix left; no third round.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [HIGH] An off steering still counts as a driver: `driver_of` classes every `Steering` holder as a late driver, so on a body nothing else drives, the oscillator stands aside and a knockback adds to a velocity nothing resets | `K/motion/drive.rs:84-96`; `K/motion/oscillator.rs:645-660` (a late driver skips it); `K/motion/knockback.rs:455-472` (adds when driven); `DESIGN.md:598` (with no driver the oscillator drives, a knockback alone writes zero) | True; 01's flyers keep their mover or borrowed controller as their driver either way | §2.5: `driver_of` counts a steering only while it is on; Step 3 takes `K/motion/drive.rs` and `an_off_steering_is_no_driver` (an oscillator driving, a knockback settling to zero, on again steering) |
| 2.2 | [MEDIUM] A tiny positive `every`, such as 1e-12, still never ends the puff loop: in `f32`, `due − every` rounds back to `due` at a frame's accumulated time | `src/features/toys/spray_look.rs:230-237`; `src/features/toys/components.rs:228` (`due` an `f32`) | True | §2.10: a spray's `every` at least 0.001 s, so each puff lowers `due` and an update's puffs stay bounded (today's are 0.012 and 0.018); Step 8's test refuses 1e-12 too |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |

## Follow-ups

- **F1.** S-36's throw window: a kit throw record, a thrown body's credit, factor and window ended at its first impact and saved for kept bodies too, so 01's `Throws` goes; a Later line at the close.
- **F2.** S-30's grants: effects that grant a `Shield` and set a controller's air-jump cost, so a tome's grant is the effect its take applies; a Later line at the close.

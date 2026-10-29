# Phase 7 plan from the registers (phase-7) — draft

- **status:** draft
- **goal:** Assign every register row that exists to the Phase 7 or Phase 8 step that builds the system it changes, set Phase 7's remaining order, recheck 7.19's provisional assumptions against the tree, and name 7.14's and 7.15's finite lists with a target per item (the [roadmap redo](roadmap-redo.md)'s step 7.7, §5.8, Q20). Then serve as Phase 7's map: each step takes its rows from §2 and closes them here.
- **non-goals:**
  - Each step's mechanisms: its own plan specifies them from the tree of its day (the redo's non-goal).
  - Code, captures and the analysis pass itself (7.5). The lists' numbers are taken at each pass's start on 7.3's rows (§2.5).
  - Edits to the registers' sources beyond [the ARPG design](arpg-design.md)'s step 6; the [paper designs](paper-designs.md)' step 6 is a follow-up.
  - Phase 8's order and Phase 9 (the redo's §3 holds them); budgets (9.1).
- **files to touch:**
  - This session: this file; `ROADMAP.md` (Now, Later's Phase 7 line); `docs/plans/arpg-design.md` (step 6, files to touch).
  - Each later step: its own plan file (`docs/plans/<topic>.md`), this file's §2 status cells and evidence row, `ROADMAP.md`, and what its plan lists.
  - The close: this file (deleted), `ROADMAP.md`, `docs/plans/roadmap-redo.md` (§3's Phase 8 rows take §2.3's IDs), `docs/plans/arpg-design.md` (its pointer to this file).
- **ordered steps:** (1) 7.2 agent tooling. (2) 7.3 benchmarks, part 1. (3) 7.4 H2. (4) 7.5 the analysis pass, its rows folded into §2. (5) 7.8 terrain and physics. (6) 7.17 the UI pass, in its own worktree from after step 1. (7) 7.9 saves and engine seams. (8) 7.20 screens as data, after step 6. (9) 7.10 the kit pass. (10) 7.18 camera rigs. (11) 7.11 template updates. (12) 7.12 level format, CLI, editor. (13) 7.13 01 on the new systems. (14) 7.14 the CPU pass. (15) 7.15 the GPU pass. (16) 7.16 agent guidance. (17) Close.
- **done-when:** every row in §2.2 reads landed, closed, Later or 9.2 (§2.1), none open in a Phase 7 step, and every open row in §2.3 is carried into the redo's §3 by ID; every item on 7.14's and 7.15's lists reads done, dropped or stopped (§2.5); every step's done-when quoted in the evidence log; the tests pass and example 01 runs on Phase 7's systems (7.13); the plan deleted with its citations, §2.3 carried into the redo, and `phase-7` out of Now.

Item `phase-7` in [ROADMAP.md](../../ROADMAP.md), from the redo's step 7.7 (§3, §5.8, §7.3, Q20). Written 2026-10-29 at `08111d5a`. Runs under the [tungsten-milestone skill](../../.claude/skills/tungsten-milestone/SKILL.md) and the [operating rules](../operating-rules.md), and differs from the skill in one way: each step is a redo step of a session or more with its own plan, so a session takes one step, never "every remaining step".

## Context digest

- 0.112 is out with `qa6`, the quick wins (7.1) and the ARPG design (7.19). 7.2–7.5 have not run: no agent tooling, the old benchmark suite, no H2, no seam register (A15). 7.6 ran on 2026-10-23; the owner triaged 7.19 on 2026-10-29 with every row and question at its default.
- 7.7 waits only for 7.19 (Q20), so it runs now, ahead of 7.2–7.5 in time but not in order. 7.5's register joins §2 at its triage by §2.4's rule, and 7.3's rows hold the lists' numbers (Q1).
- The registers: the paper designs' §4 (P7-1–P7-20, P8-1–P8-17), the ARPG design's §15 (PA-1–PA-22), and the feedback plan's kit and engine gaps ([`example-01-feedback.md`](1.0/example-01-feedback.md) §6, K1–K31), which the redo's §3 cites and 7.5 seeds from.
- The rule is §7.3's triage: a row goes to the step that builds the system it changes, so each system is built once (`D-218`). A row split across systems is split here (P7-2, P7-3).
- Phase 7 is two tracks: the main track (7.2 → 7.16) and the UI track (7.17 → 7.20) in its own worktree beside 7.8–7.10, joining at 7.11 (the redo's fixed orders).
- Decisions: `D-215` (the redo), `D-218` (the fifth genre; 7.7 waits for 7.19), `D-172` (admission, amended by §5.3), `D-140` (routes), `D-070` (CI is CPU only; GPU and time rows stay local).
- Later steps read this file: 7.10 its rows, PA-1–PA-11 among them, and PA-Q2's three movable rows (§2.8); 7.14 and 7.15 their lists (§2.5); 7.13 and 8.6 their tasks, 9.1 its stress rows (§2.6).

## 1. Audit

Read at `08111d5a`: only what an assignment, an assumption or a list item needs.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | The kit's modifier formula is unchanged: `((base + Σadd) × (1 + Σpct) × Πmul)`, the last override, then the clamp | `crates/tungsten-kit/src/foundation/stats.rs:96` |
| A2 | No status type in any crate; today's effects and ailment ladders stand | `rg -n 'pub struct (Status\|Statuses)\b' crates/` → none; `crates/tungsten-kit/src/damage/effects.rs:1355` (`Effects`), `damage/ailments.rs:321` (`AilmentThreshold`) |
| A3 | No item, inventory or equipment type in the engine or kit | `rg -l 'Inventory\|Equipment\|ItemInstance' crates/` → only a private struct in `crates/tungsten-render/tests/shader_coverage.rs:28` |
| A4 | Abilities and projectiles have no burst or pattern payload | `rg -n -i 'burst\|pattern' crates/tungsten-kit/src/intent/ability.rs crates/tungsten-kit/src/overlap/projectile.rs` → none |
| A5 | No saved-component registry; the template captures and restores by hand; 01's saves are schema 23 with names from 7.1 | `rg -l 'register_saved\|SaveRegistry\|SavedComponent' crates/` → none; `templates/basic/src/save.rs`; `examples/01_platformer/src/save.rs:332` (2,873 lines) |
| A6 | The asset registry has no `remove` (P7-16's `+`) | `rg -n 'pub fn remove' crates/tungsten-core/src/assets/` → none |
| A7 | Kit domains: `damage`, `foundation`, `intent`, `lifecycle`, `motion`, `overlap`, `present`, `react`, `transform`; `react` holds cues, counters, level and loot; no `chemistry` or `build` | `crates/tungsten-kit/src/` |
| A8 | Crit is a flag; `area` and `projectile_speed` are declared and read by nothing | `crates/tungsten-kit/src/damage/pipeline.rs:218`; `foundation/stats.rs:279`, `:287` (the only matches in `crates/`) |
| A9 | The kit's genre tags are `platformer`, `top_down`, `shmup`, `grid` | `crates/tungsten-kit/src/foundation/describe.rs:551-554` |
| A10 | 7.1 landed P7-9 (map save IDs from authored names, `D-222`) and P7-12 (gamepads, `D-220`, `D-221`) | commits `4f40f34e`, `5e516e60`; `d3724465`, `2fda1ff3`; `crates/tungsten/Cargo.toml:19` |
| A11 | K25 landed in part (an air jump's cost and refusal; the run-time count is in Later), K26 and K29 landed | `crates/tungsten-kit/src/motion/platformer.rs:432`, `:636`; `intent/ability.rs:305` (`with_cost_scale`); `react/cues.rs` (`CueListener`); ROADMAP.md Later |
| A12 | K12, K13 and K14 stand: `has` scans the type list; `animation_tick` resolves each state's clip by name twice a sprite; the query index rebuilds whole when not fresh | `crates/tungsten-core/src/ecs/archetype.rs:196`; `crates/tungsten-kit/src/present/animation.rs:125-136`; `crates/tungsten-core/src/physics/query.rs:305` |
| A13 | Lights cap at 16, culled by `cull_to_cap` | `crates/tungsten-core/src/lighting.rs:10`; `crates/tungsten-render/src/lighting.rs:129` |
| A14 | 01's full-pit row exists as an ignored release test | `examples/01_platformer/tests/game/frame_time.rs:83` |
| A15 | 7.2–7.5 have not run: no seam register in the tree, and Later lists them | `rg -l -i 'seam register'` → `docs/plans/roadmap-redo.md` only; ROADMAP.md Later |

## 2. Design

### 2.1 Rule and status cells

- **Rule:** a row goes to the step that builds the system it changes (§7.3's triage, §5.3). A row whose system no Phase 7 step builds goes to Phase 8 or Later. A row depending on a later step's system moves to the first step after it.
- **Status cell:** `open`; `landed <sha>`; `closed (<reason>)` for a row no step builds; `Later`, for a row that is one line in ROADMAP.md's Later; `9.2`, for a cost 9.2's lever list takes (§2.4). All but `open` are terminal. The commit that closes a step updates its rows here.
- **A move** rewrites the row's step cell, keeps it `open` and notes `moved from <step> (<reason>)`; a row moved into Phase 8 goes to §2.3. Nothing else changes a row's step.
- **Pattern** is the register's, shortened; the register holds the evidence.

### 2.2 Phase 7 rows

| ID | Pattern | Step | Status | Note |
| --- | --- | --- | --- | --- |
| P7-9 | Map objects by name | 7.1 | landed `5e516e60` | `D-222`, schema 23 |
| P7-12 | Reading a gamepad | 7.1 | landed `2fda1ff3` | `D-220`, `D-221` |
| K26 | An ability's cost scaled by a stat | e01tomes | landed (0.109) | `Ability::with_cost_scale` |
| K29 | A cue sound's position and falloff | e01sfx | landed (0.110) | `CueListener`; P7-13's rest is 7.9's |
| K25 | Air jumps priced, refused, counted at run time | Later | Later | Cost and refusal landed in `e01tomes`; the run-time and unlimited counts wait in Later |
| P7-1 | Tiles changed at runtime | 7.8 | open | Runtime terrain first (§5.1) |
| P7-2 | Pushes and impacts: physics part | 7.8 | open | Impulses, radial impulses, contact speeds (§5.6) |
| P7-3 | Something in range: sensor exit | 7.8 | open | `Interactable` is 8.1's (§2.3) |
| K7 | A controller pushed by bodies | 7.8 | open | §5.6: pushed and riding; M84's shove onto impulses |
| K14 | The query index rebuilt from every body | 7.8 | open | §5.6's cheaper index; its remaining cost is C1 (§2.5) |
| K23 | Kinematic pairs never touch | 7.8 | open | Moving platforms against controllers |
| K30 | A locked controller stops dead at a body | 7.8 | open | Named in §5.6 |
| K31 | A spatial filter by body kind or mass | 7.8 | open | Built with the index; 8.2's sight rays use it |
| P7-8 | Saves of many areas | 7.9 | open | §5.5, G4; depends on P7-9 (landed) |
| P7-10 | A tile layer's depth among sprites | 7.9 | open | |
| P7-13 | Audio: looping cue voices, pan, bus ducking | 7.9 | open | Falloff landed as K29; Later's `SoundOnEvent` listener joins it |
| P7-14 | Text outline (K5) | 7.9 | open | A field of 7.17's named text styles if 7.17 lands first |
| P7-15 | A file per frame | 7.9 | open | Sprite sheets, with §5.4's boot loading |
| K2 | A particle config's draw depth | 7.9 | open | |
| K4 | An animation's playback rate | 7.9 | open | |
| K9 | Lit sprites without the hit-flash material | 7.9 | open | |
| K21 | An emitter's rate and direction at run time | 7.9 | open | |
| PA-1 | Lines scoped by tags | 7.10 | open | 7.10a (§2.8) |
| PA-2 | Skill sheets and the snapshot | 7.10 | open | 7.10a |
| PA-5 | Crit | 7.10 | open | 7.10a; every genre (PA-Q3) |
| PA-6 | Conversion and extra | 7.10 | open | 7.10a |
| PA-7 | Defender's side | 7.10 | open | 7.10a |
| P7-7 | A hit's side | 7.10 | open | 7.10a, with PA-2's hit context |
| PA-9 | Primary stats | 7.10 | open | 7.10a; movable to 8.7 (PA-Q2) |
| PA-10 | World sources | 7.10 | open | 7.10a; 01's difficulty on it (PA-Q5) |
| PA-11 | `explain` records | 7.10 | open | 7.10a; shared with chemistry's `explain` |
| P7-4 | Statuses on tiles and things | 7.10 | open | 7.10b: §5.2, Q17's merge, Q11's names |
| P7-5 | Light as a stimulus | 7.10 | open | 7.10b: the core light-at-a-point query |
| PA-8 | Statuses from their user | 7.10 | open | 7.10b, after PA-2 |
| K1 | Cue rules can't tell a periodic tick from a hit | 7.10 | open | 7.10b: statuses replace effects' periodic payloads |
| K11 | Timers with expiry and a saved form | 7.10 | open | 7.10b: the sketch's timers with expiry and saves |
| K27 | `Damaged` hides a shield's absorption | 7.10 | open | 7.10b, with K28's flash |
| K28 | Every non-periodic hit flashes over a status tint | 7.10 | open | 7.10b: effect looks |
| P7-2 | Pushes and impacts: kit part | 7.10 | open | 7.10c: impact damage, bodies as walls by layer; its blast is K10, in 7.10b |
| K10 | A radial blast payload | 7.10 | open | 7.10b: chemistry's `blast` output (§5.2), which replaces 01's detonation (`elements/cascade.rs:634`); abilities fire it in 7.10c |
| P7-6 | Ability fixes | 7.10 | open | 7.10c, with K8 |
| K8 | `charges { all: true }` refills from the first charge | 7.10 | open | 7.10c, in P7-6 |
| PA-3 | Area and projectile speed read | 7.10 | open | 7.10c; movable to 8.7 (PA-Q2) |
| PA-4 | Use time | 7.10 | open | 7.10c, with P7-6's held trigger; movable to 8.7 (PA-Q2) |
| P7-19 | One-shot clips | 7.10 | open | 7.10c |
| P7-20 | A hitbox by facing (K3) | 7.10 | open | 7.10c |
| K3 | A hitbox that follows its owner's facing | 7.10 | open | 7.10c, in P7-20 |
| K19 | The magnet and a falling bodiless pickup | 7.10 | open | 7.10c: motion on 7.8's physics |
| K20 | A top-down mover's outside velocity; additive knockback | 7.10 | open | 7.10c: motion on 7.8's impulses |
| K22 | A hotkey for `KitDebug`; hit shapes without the trace | 7.10 | open | 7.10c |
| P7-18 | The kit's genre claims | 7.10 | open | 7.10c: `platformer`, `top_down`, `rpg`, `shooter`, `arpg`; `grid` and `shmup` dropped (G1–G3) |
| P7-16 | Seeded generation | 7.12 | open | Depends on P7-1 (7.8); the registry `remove` (A6) |
| P7-17 | Places, routes and rooms in our own format | 7.12 | open | Depends on P7-9 (landed) |
| K12 | `World::has` scans the type list | 7.14 | open | Item C4 (§2.5) |
| K13 | `animation_tick` resolves clips by name | 7.14 | open | Item C3 (§2.5) |
| P7-11 | More lights than the cap | 7.15 | open | Item G6 (§2.5) |

7.11, 7.13, 7.16, 7.17, 7.18 and 7.20 hold no register row yet: their designs are the redo's §5.9–§5.11 and §3 sketches, and 7.5's triage may send them rows (§2.4).

### 2.3 Phase 8 rows

| ID | Pattern | Step | Status | Note |
| --- | --- | --- | --- | --- |
| P7-3 | Something in range: `Interactable` | 8.1 | open | With P8-1 |
| P8-1 | Dialogue | 8.1 | open | |
| P8-2 | Quests and the journal | 8.1 | open | On P8-1 |
| P8-3 | Items, inventory and shops | 8.1 | open | Equipment as statuses (Q17; A3) |
| P8-4 | Cutscenes | 8.1 | open | On 7.18's rigs |
| P8-5 | Typed text | 8.1 | open | |
| P8-6 | Clock and schedules | 8.1 | open | Depends on P8-8 (8.2): 8.1 places NPCs by time without paths (Q9) |
| P8-7 | Loading between areas | 8.1 | open | On P7-8 (7.9) |
| P8-17 | A cone light kind | 8.1 | open | The redo's 8.1 row holds it |
| PA-12 | Items with rolled lines | 8.1 | open | After PA-8 (7.10) |
| PA-13 | Ground items | 8.1 | open | Styled by rarity name; 8.7's `rarities` rolls it |
| P8-8 | Navigation | 8.2 | open | On P7-1 (7.8); platform links for 8.4 |
| P8-9 | Perception | 8.2 | open | On P7-4, P7-5 (7.10) and K31 (7.8) |
| P8-10 | Squads and cover | 8.2 | open | |
| P8-11 | Weapons as data | 8.2 | open | On P7-6 (7.10) |
| P8-12 | Hit-stop | 8.2 | open | |
| P8-13 | Bullet patterns and waves | 8.2 | open | |
| PA-14 | Count and pierce from the sheet | 8.2 | open | On P8-13's pattern payload |
| PA-15 | Chain and fork | 8.2 | open | |
| PA-16 | Repeats | 8.2 | open | On P8-11's bursts |
| P8-15 | Drawn art | 8.3 | open | `asset add` and Aseprite import |
| PA-17 | Supports | 8.7 | open | `build` domain (PA-Q1) |
| PA-18 | Passive tree | 8.7 | open | |
| PA-19 | Item generation | 8.7 | open | |
| PA-20 | Uniques and handlers | 8.7 | open | |
| PA-21 | Monster level and rarity | 8.7 | open | |
| PA-22 | `explain` and build saves | 8.7 | open | On PA-11 (7.10) and 7.9's saves |
| P8-14 | Rumble | — | closed (G5: none in 1.0) | The two-thread rule stands |
| P8-16 | Aim assist | — | closed (stays the game's) | Until a second game asks |

### 2.4 7.5's register

- 7.5 numbers its rows `S-1`, `S-2`, … (Q8); a row it re-reports from the K-rows keeps its K ID beside its S ID.
- Its triage sends each row by §2.1's rule: a system a Phase 7 step builds → that step; a template row → 7.11; a fix only 01 needs → 7.13; a Phase 8 system → its 8.x step; anything else → Later as one line.
- A performance workaround it finds joins the evidence of a §2.5 item, never the list; a cost no item covers waits for 9.2.
- A row for a step already started (the UI track may have) goes to the first unstarted step that touches that system.
- The triage's commit writes every row into §2.2 (Phase 7 steps, with `Later` or `9.2` in the step cell and status for the rest) or §2.3 (Phase 8 steps), so each S-ID has one disposition in the tables.

### 2.5 7.14's and 7.15's finite lists

The candidates are the redo's 7.14 and 7.15 rows; the workload is 01 on the new systems (7.13). The evidence below orders them; it is dated and from older trees, so it ranks and does not set targets.

- **Gate at each pass's start:** the pass measures each item on its row with 7.3's runner on 7.13's tree and records the item's share of the row's time. An item under 5 % of its row leaves the list (`dropped`, with the number). Nothing joins (Q4). G6 is a register row (P7-11), so the floor never drops it: it closes on its behaviour.
- **Target:** remove at least half of the item's measured share, as an absolute time on its row, recorded at the gate (Q4). G6's target is behavioural.
- **End:** every item `done` (its target met, a not-`regressed` whole-frame row and the other resource's rows beside it), `dropped` at the gate, or `stopped` (a profile shows the remaining cost is the work itself; not for G6). Everything else waits for 9.2 (§5.8).

**7.14, CPU:**

| # | Item | Row (7.3's set or the step that adds it) | Evidence today | Status |
| --- | --- | --- | --- | --- |
| C1 | The spatial query index after 7.8 (K14) | 01's full pit, `physics_step`; `physics-sparse` | `physics_step` 2.07 of 3.71 ms p50 (56 %) in 01's full pit, release, 2026-10-23 at M81's tree (`perf-runs/20261023-e01g-frame/after.txt`); A12 | open |
| C2 | Interaction spread (7.10's chemistry) | 7.10's spread row at 12,000 bodies (§5.8) | Not built; 01's `fire_spread` rebuilt a cell map of every unburnt ball per step (P5, unmeasured) | open |
| C3 | Per-entity kit systems: `animation_tick` resolving clips by name (K13) | 01's full pit, `update`; `particles` `animate_sprites` | 0.90 ms for 12,000 balls on one clip (K13, at 0.97); interned animation IDs, an open proposal in `docs/perf/benchmarks.md` | open |
| C4 | `World::has` and the query paths (K12) | `ecs` `update`; 01's full pit | No isolated time; P1–P7 moved 01 off per-entity `has` (feedback §1) | open |
| C5 | The save snapshot | 7.9's save row (§5.5's autosave cost) | Not built; 01's `save.rs` is 2,873 lines (A5) | open |
| C6 | The extract | 01's full pit, `extract`; `gpu-throughput` | 0.13 of 3.71 ms p50 (3.5 %) in 01's full pit (as C1); 13.3 ms at 400,000 sprites in `gpu-throughput` (2026-10-02) | open; likely dropped at the gate in 01 |

**7.15, GPU** (local, `D-070`):

| # | Item | Row | Evidence today | Status |
| --- | --- | --- | --- | --- |
| G1 | The lit-sprite path in the scene pass | 01's area row, scene pass (Q5); `gpu` scene pass | The scene pass is 13.3–23.9 of 19.4–30.1 ms `render_span` p50 (69–79 %) in six areas at 2160p, the mine's spot under game over's passes (M77's survey, 2026-10-22 at 0.97, `perf-runs/20261022-m77-example-01-fixes/gpu-areas/`); `gpu` 6.88 of 10.39 ms (2026-10-02) | open |
| G2 | Tile layers and the layered backdrops (overdraw) | 01's area row, scene pass | The backdrops take 3.2–4.9 ms of the scene pass at 2160p (`gpu-areas-nobackdrop/` against `gpu-areas/`) | open |
| G3 | Bloom and the post stack | 01's area row, the post passes; `gpu` pass rows | The bloom chain about 3.8 ms and the vignette pass 1.8 ms at 2160p: 19–29 % of `render_span` (same survey) | open |
| G4 | Particle quads | `particles`; 01's spray frames | Inside the scene pass, not separated today | open |
| G5 | Sprite batching and upload bytes | `gpu-throughput`; 7.3's local upload-byte counter | `gpu` `render_encode` 2.12 ms p50 (2026-10-02) | open |
| G6 | The light cull past 16 by importance (P7-11) | 01's lantern scene; `gpu` light sweep | Cap 16; over it, zero-intensity lights drop and the rest rank by distance to the view, with nothing pinning the player's light (A13, `crates/tungsten-render/src/lighting.rs:133-150`) | open; target: the player's light never culled and a dark light holding no slot, each shown by a test past the cap; the scene pass not `regressed` |

M77's survey predates M82's look and M85's scenery, so the order may change at the gate.

### 2.6 Evaluations and stress rows

- **§8's five tasks:** 7.13 (in 01 and a `tungsten new` copy) and 8.5.
- **The paper designs' weekly tasks** (§2, §3 there): 7.13 takes LW3 (a lit path: 7.10's light query, 7.12's authoring), SF1 (a room template: 7.12's rooms and seeds) and SF4 (a status band's decay: 7.10's statuses). The other seven need Phase 8 systems (dialogue, items, cutscenes, weapons, AI, perception); 8.6 takes all ten.
- **The ARPG design's five tasks** (§12 there): 8.6.
- **Stress rows:** the paper designs' stress numbers and the ARPG design's seven rows (§13 there): 9.1.
- **System rows** (§5.8): 7.8 friction and slope piles; 7.9 the save snapshot and boot to first frame; 7.10 interaction spread at 12,000 bodies; 7.17 and 7.20 their layout counter guards; 7.18 none.

### 2.7 7.19's assumptions, rechecked

The redo's §7.3 and the ARPG design's §1 list them; every one holds at `08111d5a`.

| Assumption | The tree | Verdict |
| --- | --- | --- |
| §5.2's statuses and Q17's merge | No status type; effects and ladders stand (A2); §5.2 and Q17 unchanged since 2026-10-28 | Holds: 7.10b builds statuses with PA-1's lines as their stat changes and PA-8's user scaling |
| P8-3's equipment as statuses | No item type (A3); the redo's 8.1 row keeps equipment as statuses. P8-3 says "kit effects", Q17's earlier name for them | Holds |
| The modifier formula, unchanged | A1 | Holds; no step changes it |
| P8-11's and P8-13's payloads | None today (A4); 8.2's row holds both | Holds: PA-14 and PA-16 cannot land before 8.2 |
| §5.5's saves and P7-16's seeds | Names and save IDs in 01 only; no registry; no `remove` (A5, A6) | Holds: PA-22 waits for 7.9. PA-19's item seeds are its own (T5 there), so P7-16 is not on the ARPG's path |
| Q11's names | `react` holds cues; no `chemistry` or `build` (A7) | Holds: 7.10b adds `chemistry`, 8.7 adds `build` (PA-Q1) |

The brief's kit survey still holds where a row rests on it: crit is a flag and `area` and `projectile_speed` are unread (A8), so PA-3 and PA-5 stand.

### 2.8 Order, tracks and routes

- **Main track:** 7.2 → 7.3 → 7.4 → 7.5 → 7.8 → 7.9 → 7.10 → 7.18 → 7.11 → 7.12 → 7.13 → 7.14 → 7.15 → 7.16.
- **UI track,** in its own worktree, which the owner makes: 7.17 from after 7.2, then 7.20, both before 7.11. The tracks share only the template's saves (7.9); whichever lands second rebases.
- **Fixed orders** (the redo's §3): 7.17 → 7.20 → 7.11; 7.18 after 7.10 and before 7.13; 7.5 before 7.8, so its rows reach the steps that build them.
- **Routes:** every step is large under `D-140` and writes its own plan with the tungsten-milestone skill, except 7.16, which is medium unless its research spans sessions. Where a step needs several plans:
  - 7.8: runtime terrain, then physics with slopes and the controller last (the redo's 7.8 row).
  - 7.9: saves of many areas, then boot loading with sprite sheets and the small seams.
  - 7.10 (Q3): **7.10a** stats and hits (PA-1, PA-2, PA-5–PA-7, PA-9–PA-11, P7-7), the shapes the rest stand on; **7.10b** statuses and chemistry on them, with the blast its outputs need (P7-4, P7-5, PA-8, K1, K10, K11, K27, K28); **7.10c** abilities, 01's generic code, motion and the genre tags (P7-2's kit part, P7-6, K8, PA-3, PA-4, P7-19, P7-20, K3, K19, K20, K22, P7-18). Each plan updates the kit's determinism pin where it changes what the hash serializes (§5). PA-3, PA-4 and PA-9 move to 8.7 when the plan holding them passes ten steps or its critique finds 7.10 too large (PA-Q2).
  - 7.12: the format and CLI, then the editor (the redo's 7.12 row).

## 3. Steps

Each step is a redo step; its own plan holds its mechanisms, files, checks and commits. **Every step's done-when** adds these to the specifics below:

- No row of the step left open: `rg -n '^\| [^|]+ \| [^|]+ \| <step> \| open \|' docs/plans/phase-7.md; test $? -eq 1` → exit 0.
- Its plan closed: deleted with its citations, `just repo-check` → passes.
- This file's evidence row written in the step's closing commit, with the commit range and the step's done-when quoted from its plan.
- From 7.8 on, a system step's benchmark row and counter guard in 7.3's runner (§5.8), except 7.18.

**Hand-off:** each step ends with the next step's plan prompt, from tungsten-next's `plan` row with this file and the redo's row as its sources.

### Step 1 (7.2): Agent tooling foundations

- **Rows:** none. **Sketch:** the redo's 7.2 row and §8's foundations row.
- **Done-when:** the sketch's three commands (`run --script`, `render`, `describe`) work as its plan's checks show; an asset review sheet comes from `render` without a scratch crate.
- **Moves:** none here.

### Step 2 (7.3): Benchmarks, part 1

- **Rows:** none. **Sketch:** the redo's 7.3 row and §5.8's part 1.
- **This plan asks of it:** a row set (Q12) that measures every §2.5 item: 01's full pit, the template frame, `ecs`, `particles`, `physics-sparse`, `gpu`, `gpu-throughput`, a local GPU row of 01's areas (Q5), and the upload-byte and draw-call counters locally.
- **Done-when:** the sketch's runner, guards, ported rows and purge, as its plan's checks show; each row named here is listed by the runner.
- **Moves:** retires per-milestone sittings (the rewrite's R10).

### Step 3 (7.4): H2

- **Rows:** none. **Sketch:** the redo's 7.4 row and §4.
- **This plan asks of it:** tungsten-next's prompts gain a phase plan's hand-off, a step's plan prompt in place of the `run` prompt (follow-up F2).
- **Done-when:** the sketch, as its plan's checks show.

### Step 4 (7.5): Analysis pass

- **Rows:** none of §2; it writes the seam register. **Brief:** the redo's §7.1. Its seeds include the K-rows; those §2.2 already assigns are re-reported only with new evidence.
- **Done-when:** the register written, verified and triaged; every `S-` row in §2.2 or §2.3 with one disposition by §2.4 (a step, `Later` or `9.2`): the register's row count equals `rg -c '^\| S-[0-9]+ ' docs/plans/phase-7.md`.

### Step 5 (7.8): Terrain and physics

- **Rows:** P7-1, P7-2 (physics), P7-3 (sensor exit), K7, K14, K23, K30, K31. **Design:** §5.1, §5.6.
- **Done-when:** the redo's 7.8 sketch; friction and slope piles as rows (§2.6).
- **Moves:** the physics hashes, under 7.3's guards.

### Step 6 (7.17): UI pass, UI track

- **Rows:** none yet. **Design:** §5.9. After 7.2, in its own worktree, beside 7.8–7.10.
- **Done-when:** the redo's 7.17 sketch, the shell's `tokei` count against `D-199`'s 2,900 measured, and the idle-screen counter guard.

### Step 7 (7.9): Saves and engine seams

- **Rows:** P7-8, P7-10, P7-13, P7-14, P7-15, K2, K4, K9, K21. **Design:** §5.4, §5.5.
- **Done-when:** the redo's 7.9 sketch, with §5.5's acceptance test (a restore into a world with other entity allocations, 01's save tests unchanged); the save snapshot and boot rows.

### Step 8 (7.20): Screens as data, UI track

- **Rows:** none yet. **Design:** §5.11. After 7.17.
- **Done-when:** the redo's 7.20 sketch and the reloaded-screen counter guard.

### Step 9 (7.10): Kit pass

- **Rows:** the 29 rows §2.2 sends to 7.10, as 7.10a, 7.10b and 7.10c (§2.8). **Design:** §5.2, §5.3, the ARPG design's §3–§8 and §16.
- **Done-when:** the redo's 7.10 sketch; the ARPG design's §16 row for 7.10 (§9.1's hit resolves to 32.07 in a kit test; 01's crit, blast share and difficulty are data with its tests' outcomes unchanged); the spread row at 12,000 bodies.
- **Moves:** the kit's determinism pin, at most once per 7.10 plan, under 7.3's guards (§5).

### Step 10 (7.18): Camera rigs

- **Rows:** none yet. **Design:** §5.10. After 7.10, before 7.13.
- **Done-when:** the redo's 7.18 sketch; a switch never jumps at 30, 60 or 144 Hz.

### Step 11 (7.11): Template updates

- **Rows:** 7.5's template rows (§2.4). After 7.9, 7.17 and 7.20.
- **Done-when:** the redo's 7.11 sketch within `D-181`'s budget.

### Step 12 (7.12): Level format, agent CLI, editor

- **Rows:** P7-16, P7-17. **Design:** §5.1.
- **Done-when:** the redo's 7.12 sketch; `check`'s props supported generalizing `D-214`.

### Step 13 (7.13): 01 on the new systems

- **Rows:** 7.5's 01-only rows (§2.4). **Tasks:** §8's five, LW3, SF1, SF4 (§2.6).
- **Done-when:** the redo's 7.13 sketch; each task's friction recorded as a seam and the rerun complete; the owner accepts 01.

### Step 14 (7.14): Performance pass, CPU

- **Rows:** K12, K13; **list:** C1–C6 (§2.5).
- **Done-when:** the gate's shares recorded; each item `done`, `dropped` or `stopped` in §2.5; the whole-frame row and the GPU rows not `regressed`.

### Step 15 (7.15): Performance pass, GPU

- **Rows:** P7-11; **list:** G1–G6 (§2.5).
- **Done-when:** as step 14, with the CPU frame, the extract included, not `regressed`.

### Step 16 (7.16): Agent guidance

- **Rows:** none. **Sketch:** the redo's 7.16 row, against 7.13's evaluation.
- **Done-when:** what the template ships decided and recorded.

### Step 17: Close

- **Files:** this plan (deleted), `ROADMAP.md`, `docs/plans/roadmap-redo.md`, `docs/plans/arpg-design.md`, the follow-ups' homes.
- **Change:** carry §2.3's open rows into the redo's §3 Phase 8 rows by ID (PA-12 and PA-13 into 8.1, PA-14–PA-16 into 8.2, P8-6's note into 8.1, PA-17–PA-22 named in 8.7); point the ARPG design at the redo; move open follow-ups to known issues or Later; delete this plan and its citations; take `phase-7` out of Now.
- **Done-when:** no Phase 7 step's row open (`rg -n '^\| [^|]+ \| [^|]+ \| 7\.[0-9]+ \| open \|' docs/plans/phase-7.md; test $? -eq 1` → exit 0, run before the deletion); every §2.2 row's status terminal (§2.1), each `Later` row a line in ROADMAP.md's Later, each `9.2` row in the redo's 9.2 lever list, and each list item done, dropped or stopped, as the last evidence row quotes; every open §2.3 row in the redo, moved and S-rows included (`rg -o -r '$1' '^\| ([A-Z0-9-]+) \| [^|]+ \| 8\.[0-9]+ \| open \|' docs/plans/phase-7.md | while read -r id; do rg -q "$id\b" docs/plans/roadmap-redo.md || echo "$id"; done` → prints nothing, run before the deletion); `just repo-check` → passes; example 01 runs.
- **Ends with:** the commits made, the evidence quoted, what the owner should play, and the next prompt (8.1's plan).

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | 7.7 ran before 7.2–7.5. Re-cut the order? | No: the redo's order stands; 7.5's rows join by §2.4, and the lists' numbers come from 7.3's rows at each pass's start |
| Q2 | The K-rows the redo's §3 doesn't cite (K1, K11, K19–K23, K27, K28, K31) | The steps in §2.2; 7.5's triage may re-cut them with new evidence |
| Q3 | 7.10 as three plans in dependency order | Yes, as §2.8; PA-3, PA-4 and PA-9 move to 8.7 on PA-Q2's trigger there |
| Q4 | The lists' floor and target | A floor of 5 % of the row's time at the gate; a target of half the item's measured share; G6 behavioural |
| Q5 | Where 7.15 measures 01 | A local GPU row of 01's areas in 7.3's set, M77's area survey made a row |
| Q6 | Where Phase 8's assignments live after this plan | The redo's §3 rows by ID (step 17); the ARPG design stays until 8.7 closes |
| Q7 | `phase-7` in Now for the whole phase | Yes, beside the step in work; with the UI track, Now holds three |
| Q8 | 7.5's row IDs | `S-1`, `S-2`, … |
| Q9 | P8-6's schedules depend on P8-8's navigation, a step later | 8.1 resolves schedules by time and places NPCs without paths; walking between places waits for 8.2 |

Each question takes its stated default; the owner reviews the plan on their own time.

## 5. Invariants

- **May move:** a row's step, only by §2.1's move with its reason; a list item, only out, at its gate. The kit's determinism pin (`crates/tungsten/src/tests/kit_recipes/determinism.rs`) at most once per 7.10 plan, since it serializes effects and buckets (7.10b) and each genre's world (7.10c), each move with its cause in the evidence log; this splits §5.2's and the ARPG design's §16's "moves once" across 7.10's plans (Q3).
- **Must not move:** the redo's fixed orders (§2.8); the lists' membership beyond the gate; the determinism test's two-run assertion, which holds at every commit. Engine invariants are each step's plan's: the physics hashes move only in 7.8, under 7.3's guards; the pinned containment hash, the row digests, `gpu-visual.png` and the post and transition regressions move only where a step's plan says so.

## 6. Stop conditions

- A step's done-when fails: its rows stay open, steps that depend on it wait, and the other track carries on. A step's own plan handles its failed checks as the skill says.
- A row needs a system no Phase 7 step builds: it moves to Phase 8 or Later, with its reason.
- A row's dependency lands after its step: it moves to the first step after the dependency.
- 7.10 is too large: PA-Q2's trigger (§2.8).
- A list item can't reach its target: `stopped`, with the profile.
- A new performance finding: 9.2, never the list.
- Now would hold four items: the UI track waits.

## 7. Critique

Round 1: the `critique` skill with `--repo`, `gpt-6-astra` at xhigh, 2026-10-29, reading `1b0c5236`. Footer: `463.2 s, tokens in 1378106 (cached 1272960), out 10056, commands 26, output 429.4 KB (largest 99.4 KB), usage 5h 0% → 28%, week 12% → 17%`. Five findings, all accepted; they changed done-when checks and 7.10's split, so a second round runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [MEDIUM] Chemistry needs the blast implementation assigned to the following plan | The redo's §5.2 lists `blast` among chemistry's outputs (`roadmap-redo.md:246`) and replaces 01's detonations (`:252`); 01's blast is game code (`examples/01_platformer/src/features/elements/cascade.rs:634`) | accepted | K10 moves to 7.10b (§2.2, §2.8); P7-2's kit part keeps impact damage and bodies as walls in 7.10c |
| 2 | [MEDIUM] The single hash update conflicts with independently landing the kit subplans | The pin serializes effects and buckets (`crates/tungsten/src/tests/kit_recipes/determinism.rs:120`) and runs per genre (`:526`), so 7.10b and 7.10c each move it | accepted | §5 and step 9: the pin moves at most once per 7.10 plan, with its cause; the two-run assertion never moves |
| 3 | [MEDIUM] The performance gate can discard G6's unmet behavioral requirements | `cull_to_cap` ranks by direction and distance, with no player light (`crates/tungsten-render/src/lighting.rs:133-150`); it already drops zero-intensity lights over the cap, so G6's evidence was stale too | accepted | §2.5: the floor never drops G6, `stopped` is not for it, and its evidence and target name tests past the cap |
| 4 | [MEDIUM] A valid Later assignment makes the analysis pass fail its completion check | §2.4 sent rows to Later and 9.2 outside the tables that step 4 counts | accepted | §2.4 writes every S-row into the tables, `Later` and `9.2` included; §2.1 adds `9.2`; step 4's check counts dispositions |
| 5 | [MEDIUM] The phase's completion contract rejects terminal states its steps allow | The header's done-when omitted `Later` (K25) and `stopped` | accepted | §2.1 names the terminal states; the header and step 17 accept them, with each `Later` row in ROADMAP.md's Later and each `9.2` row in 9.2's list |

Round 2: the same reviewer at xhigh, 2026-10-29, reading `a577dfc1` with the Critique section and the log cut and a focus note on round 1's changes. Footer: `210.1 s, tokens in 511722 (cached 448256), out 4929, commands 22, output 299.2 KB (largest 50.1 KB), usage 5h 28% → 42%, week 17% → 19%`. Two findings, both repairs of round-1 fixes, both accepted; no third round.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] Moved rows can escape the Phase 8 handoff | §2.1 made `moved` terminal, and step 17 carried only §2.3's open rows and checked PA-12–PA-22 alone, so PA-3, PA-4 or PA-9 moved to 8.7 would be lost | accepted | §2.1: a move rewrites the step cell and keeps the row open, Phase 8 rows in §2.3; step 17 checks every open §2.3 ID against the redo |
| 2.2 | [MEDIUM] A valid `9.2` disposition still violates the overall completion condition | The header's done-when listed landed, moved, closed and Later | accepted | The header accepts §2.1's terminal states, `9.2` included |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 (7.2) | 2026-10-29 | passed | `agent-tooling`, commits `2ecd7d41` (core script format and hooks), `bc1718ab` (the runner on the frame body and the harness), `1dbd0c71` (script mode in the window), `b4815fc9` (`run --script`, `describe`), `831811b0` (the template's and 01's hooks), `6e642b4d` (`render`, `D-225`) and the close; its done-when: every step's checks passed (its evidence log at `6e642b4d`); `just check` → passes (fmt, workspace clippy, every test and doctest); 01 through its own `cargo run` with a script (`start_run`, frame 180) → passed, `area` "Lantern clearing", the stack `["gameplay"]`; a review sheet of 01 from `tungsten render` with no scratch crate: the six foes' contact sheet (1280 × 720), a 20-frame campfire loop (5 distinct SHA-256 sums) and the first area's still with the physics overlay; step 2's perf gate, after a rerun and cold call points, 0 owned metrics `regressed` in both direct pairs; the plan deleted with its citations, its follow-ups in known issues, out of Now | `perf-runs/20261029-agent-tooling/` (sittings, step fixtures, `close/`), `perf-runs/20261029-agent-tooling-sheet/` |

## Follow-ups

- **F1.** The paper designs' step 6 and its done-when ("7.7 has assigned every row of §4") are met by §2; its step 6 also says "7.13 and 8.5 take the weekly tasks", where the redo's §8 now says 7.13 and 8.6. Mark it at the next edit of that file, or at step 17.
- **F2.** tungsten-next's `run` prompt says "run every remaining step", which this plan forbids; H2 (step 3) gives a phase plan's hand-off its own prompt.

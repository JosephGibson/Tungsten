# Paper designs — draft

Two games designed on paper, without code, to find what the engine, kit and template fail to support: Lantern Ward, a story-driven RPG, and Static Front, a tactical twin-stick shooter, both top-down. Written by the [roadmap redo](roadmap-redo.md)'s method ([§7.2](roadmap-redo.md#72-paper-designs), step 7.6), against the API snapshots at `3f82795`, unchanged since `d663c06`.

- **status:** draft
- **goal:** Find what a story-driven RPG and a shooter with complex enemy AI need that the engine, kit and template lack, as register rows that fold into Phase 7 and scope Phase 8, before 7.7 plans Phase 7.
- **non-goals:**
  - Code, and edits to any other file.
  - Each step's mechanisms: the step's own plan specifies them from the tree of its day.
  - Navigation on platforms (jump links and ledges): 8.2 takes it for 8.4's refresh of 01.
  - Grid placement and building; multiplayer; localisation.
  - A vertical slice of either game (the owner's answer of 2026-10-23: ultra-basic sketches at most).
  - The optional `/critique` of the build sheets, not run.
- **files to touch:** This file. The transition session (the roadmap redo's ordered step 3) and 7.7's Phase 7 plan read its registers; nothing else changes before then.
- **ordered steps:**
  1. The coverage matrix (§1), written before the designs. Done.
  2. Lantern Ward (§2) and Static Front (§3): a page with numbers, the production list, five weekly agent tasks, stress numbers, the build sheet and a minute of play each. Done.
  3. The registers (§4), the genre check (§5) and the questions they raise (§6). Done.
  4. A fresh-context verifier on every citation (§7). Done.
  5. The owner reads the designs and answers or accepts §6's questions; `/critique` on the build sheets is optional.
  6. The transition session and 7.7 take §4's rows: Phase 7's into 7.7's plan, Phase 8's into 8.1, 8.2 and 8.3. 7.13 and 8.5 take the weekly tasks as evaluation tasks; 9.1 takes the stress rows.
- **done-when:**
  - Every "data only" and "game Rust" verdict cites an item in `api/*.txt` or the components snapshot, and every gap names a search that finds nothing; §7's verifier report confirms or fixes every citation.
  - The genre registers (§4) are written, each row tagged by design.
  - The owner has answered or accepted §6's questions, and 7.7 has assigned every row of §4.

## Context digest

- Step 7.6 of the roadmap redo, which may start any time (its Q14). This session ran it on 2026-10-23 while M81 (`e01g`) ran in the same tree and landed as `3f82795`. `api/` is the same from `d663c06` to `3f82795`, so every API citation holds across them; backlog and known-issue lines are `3f82795`'s, M81's rows among them.
- The two designs split §7.2's nine axes, and lighting and post, between them (§1), around what 01 (a side-view physics sandbox on one map) and the template's sample (a small top-down arena) already exercise.
- The build sheets hold 59 systems: 12 data only, 22 game Rust (about 5,720 lines), 24 gaps (about 11,260 lines where today's game could write them at all) and 1 blocked, by the two-thread rule. The registers hold 37 rows: 20 into Phase 7, 17 scoping Phase 8.
- The largest finds:
  - No window reads a gamepad (`docs/known-issues.md:42`), which a twin-stick shooter cannot ship without (P7-12).
  - A tile changes only by cloning its whole map, and every change rescans every collision layer of every map (P7-1).
  - Dropping `grid` and `shmup` leaves 24 of the kit's 45 components under two genres; tagged `rpg` and `shooter`, these designs bring 21 back (§5).
  - §5.5's save names one level; a story RPG with five areas needs several (P7-8, G4).
  - Past 16 lights the cull drops lamps in spawn order, and a lamp dimmed to 0 keeps its slot (P7-11).
- The roadmap redo's Phase 8 list, which its §3 calls this step's prediction, holds: each 8.1 and 8.2 item but navigation on platforms has a row here. Fifteen rows carry a fix that no step's sketch lists yet, marked `+` in §4.

## 1. Coverage

Written before the designs. The axes are §7.2's nine and a tenth, lighting and post, which §7.2 asks each design to use; the first two columns are what 01 ([README](../../examples/01_platformer/README.md)) and the template's sample ([`AGENTS.md`](../../templates/basic/AGENTS.md)) already exercise. Each axis has an owner, the design that probes it hardest; the other touches it only where the cell says.

| Axis | 01 | Template sample | Lantern Ward | Static Front | Owner |
| --- | --- | --- | --- | --- | --- |
| View and movement | Side view: the platformer controller, coyote time, an air jump; kinematic bodies; cursor aim | Top-down mover, cursor aim, 32 px sprites | Top-down walking with four facings, depth sort, interiors through doors, a scripted camera | Top-down twin-stick: analog move and aim, a dodge with i-frames, recoil, the camera leading the aim, hit-stop | SF for feel; LW for depth, facing and the camera |
| World structure | One map of 430 × 72 tiles, six areas as zones | One arena | Five hand-made areas and 12 interiors, swapped behind a fade, kept while unloaded | Eight rooms a run from 27 templates by seed; cells that break | LW for persistence, SF for generation |
| AI | 26 enemies on state machines, sight by its own casts, alerts in a radius; no navigation | Orbit, patrol, still | NPC schedules with paths through a town; fogborn that hunt by hearing and flee light | Squads that take cover, flank, suppress and retreat; sight cones and hearing with memory; paths on a grid that changes | SF for combat AI; LW for schedules and hearing |
| Entity counts | 12,000 small balls; 1,194 props (the roadmap redo's §1); 26 enemies | Tens | 56 NPCs loaded, 18 on screen; 60 lamps, 28 lights in view; 13,824 fog cells in the Marsh | 460 rounds alive; 24 units awake; 300 debris bodies; 500 breakable cells in a room | SF for bodies and rounds; LW for lights, cells and NPCs |
| Chemistry | Four damage types and their ladders, burning, freezing, steam, cascades; crates and kegs in its own code | None | Oil, fire, wet fog, light as a stimulus, salt wards: statuses on tiles with intensity, spread and fuel | Blasts, fire, smoke, EMP on conductive things, water and shock, cold; cells transformed; statuses that drive AI | Split: LW for light, wetness and tiles; SF for blasts, transforms and AI statuses |
| Story and UI | HUD bars, the spell bar, toasts, hints, damage numbers, banners | The shell's screens, a HUD | Dialogue with conditions, quests, a journal, shops, inventory and equipment, a map, cutscenes, typed text | Loadout, perk offers, a run summary, a codex, a minimap | LW |
| Saves | Schema 12: every saved entity by save ID, written and read by hand | The capture seam | A persistent world: five areas' changes, 22 NPCs, 60 lamps, quests, tile statuses | Meta progression, and a run saved as its seed and what changed | LW for size; SF for seed plus changes |
| Input | Keyboard and mouse; its pad bindings wait for the backend | Keyboard and mouse; pad bindings that wait for the backend | Menus by keyboard, mouse or pad; prompts with the binding's name; typed text | Pad first: two sticks, triggers, rumble, aim assist | SF |
| Pipelines | Python generators own every file; one PNG a frame; the level from JSON to `.tmj` | Python sprite and sound scripts | 1,800 strings of text, 66 portraits, 27 walk sets, 17 maps | 27 room templates checked over many seeds; 12 weapons as a table; procedural sounds | LW for content volume; SF for generation checks |
| Lighting and post | The lantern, lit sprites, light by area, bloom, a vignette | Bloom; no lights | 60 lamps against the cap, a beam, a day-and-night grade, the fog pass, god rays | Strobes, muzzle lights, an EMP blackout, glitch, aberration | LW |

Not covered by either: navigation on platforms (8.2 for 8.4), grid placement and building, multiplayer, localisation.

## 2. Lantern Ward

A top-down story RPG. Wickhaven, a fog-bound harbour town, hires a new night warden on a ten-night contract after the last one walked into the fog. By day the warden talks, trades, takes jobs and earns the townsfolk's trust. By night the fog comes in off the sea with the fogborn: blind things that hunt by sound and burn in strong light. The warden keeps the town's 60 lamps fed with oil, relights what the fog snuffs, and brings people home. Three endings turn on who trusted the warden and whether the lighthouse burns again on the last night.

Changes from the default pitch, and why:

- The lamps are a network the warden maintains (oil in each lamp, burning down through the night; hoods against wet fog), not scenery: it makes light a world state that is saved, drained and spread, which is §5.2's fuel, tile statuses and intensity, and keeps 01's lantern (one personal light) from being the probe again.
- The fogborn are blind. They hear and they fear light; they do not see. Sight with memory stays Static Front's probe, so the two designs split perception (hearing here, sight there) instead of both probing sight.
- "Five streamed areas" becomes five areas loaded behind a fade, with NPC schedules running while an area is unloaded. Seamless streaming is a large production's feature; loading between areas and off-screen schedules are what 8.1 has to answer.
- The warden is named at the start and pins notes on the map: text input, which 8.1 lists and no other design asks for.

### Numbers

| Thing | Number |
| --- | --- |
| Tile | 32 px; the view shows 40 × 22.5 tiles at zoom 1 (1280 × 720) |
| Areas | Square 96 × 64 tiles, Docks 120 × 56, Chapel Hill 72 × 80, Saltmarsh 144 × 96, Lighthouse Point 64 × 96; 12 interiors of 12 × 10 to 24 × 18 |
| Day | 16 real minutes: day 06:00–18:00 in 9 min, dusk 1.5 min, night 20:00–05:00 in 5 min, dawn 0.5 min |
| Contract | 10 days and nights; about 3 h for the main story, 6 h with every side quest |
| People | 22 named NPCs, 34 townsfolk from 4 bodies and palette swaps; 56 loaded across areas, 18 on screen in the Square at 18:00 |
| Lamps | 60 (Square 22, Docks 16, Chapel 8, Marsh 6, Point 8); a lamp holds 14 oil and burns 1 an in-game hour while lit (18:00–06:00), so a full lamp lasts a night with 2 to spare, and one left unfilled gutters at about 20:00 the next night |
| Light | Lamp 160 px, lantern 128 px open or a 256 px, 40° beam, fire 96 px; the lighthouse's beam sweeps the Point and the Marsh |
| Fogborn | 5 kinds: lurker, mireling (swarms of 6–10), wailer, drowned, the Fog Mother (last night); at most 6 awake in the Square, 8 on the Docks, 14 in the Marsh |
| Statuses | 6 with intensity: `lit` 0–5, `wet` 0–5, `oiled` 0–3, `burning` 0–5 with fuel, `salted` 0–3 (tiles only), `startled` 0–3 (fogborn) |
| Interactions | 14 rules (table below) |
| Dialogue | 1,400 lines in 140 conversations, 200 barks, 90 journal entries, 60 item texts, 24 lore notes |
| Quests | 12: 4 main (one for each of three acts and one for the last night), 8 side; 3 to 6 stages each |
| Items | 60: 18 equipment (lantern, coat, boots, charm slots), 22 consumables (oil flasks, salt, bread, tonics), 20 quest items |
| Shops | 4: chandlery, general store, smithy, apothecary; prices move ±25% with the owner's trust |
| Cutscenes | 9: the arrival, three act turns, the lighthouse, three endings, the warden's lodge at dawn |
| Screens | Title, name entry, pause, settings and saves (template), dialogue, journal (quests, notes, map), inventory and equipment, shop, dawn report, cutscene letterbox, HUD |

### Statuses and interactions

| # | Stimulus or status | Receiver | Result |
| --- | --- | --- | --- |
| 1 | `light` from a lamp, fire or the lantern, each 0.25 s, by distance | `photophobic` | builds `lit`; band 2 grants `fleeing`, band 4 burns 4 a second |
| 2 | fog | anything outdoors at night | builds `wet` to the fog's intensity (0–5) |
| 3 | `wet` ≥ 3 | a lamp's `wick`, not `hooded` | the lamp becomes `lamp_doused` (no light) |
| 4 | `wet` | `burning` | consumes 1 `burning` each 0.5 s per band over 2 |
| 5 | `heat` | `oiled` | applies `burning` at the oil's intensity, consuming `oiled` |
| 6 | `heat` | `flammable` (nets, crates, plank tiles) | builds `burning` 1 a second of exposure |
| 7 | `burning` ≥ 2 | itself | emits `heat` in 40 px and `light` in 96 px each 0.5 s |
| 8 | `burning` fuel spent | wood | becomes `charred` (a prop) or sets the charred plank tile |
| 9 | an oil flask breaking | tiles | `oiled` 3 in 48 px |
| 10 | salt thrown | tiles | `salted` 3; fogborn cannot path through `salted` ≥ 1 and take 5 crossing it |
| 11 | `wet` ≥ 3 | `salted` | consumes 1 `salted` each 10 s |
| 12 | `burning` | `fogborn` | 6 a second, and `lit` +2 |
| 13 | the wailer's scream, 96 px | `wick` not `hooded` | douses it (rule 3's result at once) |
| 14 | the lighthouse beam | everything in its cone | `lit` 5 |

Noise is perception, not chemistry: footsteps (64 px walking, 160 px running, half in soft boots), a dropped crate (200 px), a bell (600 px) and voices (96 px) each raise `startled` on the fogborn that hear them and leave the point in their memory for 6 s.

### What a save holds

Saved in the warden's bed (a slot), at each area change and at dawn (an autosave). The day and the clock; the warden (area, position, health, 24 inventory slots, 4 equipment slots, coins, the map's pinned notes and the name); 22 named NPCs (alive or hurt, trust, where they are when off their schedule, the dialogue memory flags); 60 lamps (oil, lit, hooded, broken); 12 quests (stage, each objective's count); about 400 flags and 120 counters; 166 doors and containers (locked, opened, looted); up to 50 dropped items; up to 2,000 tile statuses (salt lines, char, oil); 24 lore notes found; the explored part of each area's map. At night also: up to 14 fogborn in the current area and the fires burning. Under 300 KB of JSON.

### Lighting, post and chemistry

- **Lighting:** 60 lamps as point lights with emissive flames and windows, normal-mapped cobbles and walls, the moon as a directional light, ambient from noon's 1.0 to night's 0.12 with a blue cast, the warden's lantern (open or beam), fires. 28 lights fall in the Square's view at once, past the cap of 16.
- **Post:** fog by the fog's intensity, a night grade (colour adjust and a LUT), bloom on flames, a vignette that closes as the warden's health falls, film grain at night, god rays from the lighthouse; fades and a dissolve between areas.
- **Chemistry:** the 14 rules above; light, wetness and oil live on tiles as well as things.

### Production list

Every asset family runs through §6's asset sessions: a brief, an Opus session generating the files and a review sheet, the owner's rounds, a lock. Today's tools are 01's: Python generators that own their files, one PNG a frame, a review sheet assembled by hand, in-game captures through `TUNGSTEN_CAPTURE_FRAME` and a scratch crate.

| Kind | Count | Made with today's tools | What the pipeline lacks |
| --- | --- | --- | --- |
| Characters | The warden 104 frames (four facings × idle 2, walk 6, run 6, throw 4, refill 4, light 4); 22 named NPCs × 24 (four facings × idle 2, walk 4) = 528; townsfolk 4 bodies × 3 palettes × 24 = 288; fogborn about 220: about 1,140 frames | A paper-doll generator in Python and Pillow on 01's model: bodies, heads and coats drawn as shapes and put together per NPC, palettes swapped by recolouring; left-facing frames drawn, since nothing flips a sprite in place (`docs/known-issues.md:75` ("Nothing flips a sprite in place")) | One PNG and one manifest entry a frame (7.9's sheets); figures drawn by code look generic up close, and a drawn sheet has no way in (8.3) |
| Portraits | 22 × 3 expressions at 96 × 96 = 66 | Faces are procedural art's weakest output; the session draws each from shapes, and the owner reviews them in rounds | The art most likely to be drawn by hand, which needs 8.3's `asset add` or Aseprite import |
| Tiles | 5 area sets of about 64 and 12 interiors of about 20: about 560, each with a normal map | 01's tile families, with bevel and height normal maps | Each variant placed by the level generator, since the auto-tiler is 7.8's |
| Props | 140: lamps in six states, stalls, boats, nets, barrels, graves, reeds; normal maps, and emissive masks for flames and windows | Generator families | Roofs and canopies as about 120 placed prefabs (LW2) |
| Items and UI | 60 item icons at 32 px; 20 nine-slice frames, tabs and slots | Generator families; nine-slice insets in the manifest | — |
| Effects | About 60 sprites (fog, fire in three sizes, oil splash, salt glint, flare, smoke) and 14 particle configs | Generator families and particle JSON | — |
| Sounds | 16 footsteps (four surfaces × four), 10 for lamps and oil, 4 for fire, 15 for the fogborn (five kinds × three), 2 bells, 10 for the UI, 8 voice timbres: about 65 | The template's standard-library synthesizer (noise, filters, oscillators, envelopes), 16-bit mono WAV | One file per pitch for voices, since playback has no pitch; no pan |
| Music | Five area loops, a night loop with two intensity layers, the tavern, the chapel, the title and three ending stingers: 14 | Written note by note in the synthesizer, each loop checked for whole bars and its seam | Layers stay in step only by starting together: no stems, no beat sync |
| Maps | 5 areas of 5,760 to 13,824 tiles (Chapel Hill to the Saltmarsh) and 12 interiors: 17 `.tmj`, holding 60 lamps, 26 doors, 140 containers and about 120 named places for schedules | A compact JSON per area and a generator to `.tmj`, as 01's `level.json` and `level.py` are; Tiled's editor is no agent's tool | Authored IDs (7.1), named places that tests and schedules read, and our own format (7.12) |
| Dialogue and text | 1,400 lines in 140 conversations, 200 barks, 90 journal entries, 60 item texts, 24 lore notes: about 1,800 strings and 25,000 words | Agents write the `dialogue` kind's JSON, one file per speaker, which `tungsten check` loads through the kind's deserializer | A format a writer reads, reachability and condition checks, and a string table for a later translation (8.1, §8) |
| Data | 12 quests, 60 items, 4 shop tables, 22 schedules | Data kinds the game registers | Each kind's checks are the game's own Rust |
| Enemy kinds | 5 fogborn | Prefabs on kit items and LW12's Rust | — |

### Five weekly agent tasks

The tasks a developer would hand an agent in a normal week, for 7.13's and 8.5's evaluations. Each names the rows it runs into today.

1. **A new NPC.** "Add Maren the net-mender: a prefab and a schedule (the Docks by day, the tavern at dusk, home at night), 30 lines with two gated on trust 3 and the `nets_torn` flag, three portraits, and a side quest, *Mend the Nets*, in three stages." Done when a scripted run talks to her at 10:00 and at 19:00 and finishes the quest, and `tungsten check` passes. Runs into LW4, LW6, LW11 and the portraits' art.
2. **A new item.** "Add an oilskin hood for the lantern slot: 40 shillings at the chandlery from day 4, halving the wetness the lantern's flame takes." Done when it is bought, equipped and shown in a test to keep the lantern lit twice as long at fog intensity 4. Runs into LW8, LW9, LW15 and LW16.
3. **A lit path.** "Light the new marsh path: six lamps on the lamp map, and no stretch darker than six tiles at night." Done when a check walks the route and samples the light at each tile. Runs into the maps' authoring, LW12's light query and LW17's cap.
4. **A cutscene.** "Write act two's turn: the keeper climbs to the lamp room, the camera follows, three lines, the beam fails, fade to night." Done when it plays from its trigger, skips on Cancel and leaves the clock at 20:00. Runs into LW21 and LW4.
5. **A bug.** "A lurker caught in burning oil keeps stalking after the fire dies; it should flee while it burns and for 2 s after." Done when a test sets the scene and shows the flee and the return to stalking. Runs into LW12 and LW16.

### Stress numbers

Rows for 9.1, each with its budget category and the surfaces it exercises.

| Row | Workload | Category | Surfaces |
| --- | --- | --- | --- |
| `lw.square_dusk` | The Square at 18:00: 18 NPCs on their schedules, 22 lamps and 6 fires lit (28 lights), 1,500 fog particles, 120 roof sprites, the dialogue box open | Frame | Navigation, the light cull, depth sort, particles, UI |
| `lw.marsh_fog` | The Marsh at night: 13,824 cells whose `wet` updates every 0.5 s, `lit` from 6 lamps and the lantern, 14 fogborn hearing about 30 noises a second | Frame | Tile statuses and spread, perception |
| `lw.area_change` | The Square to the Docks behind a 0.4 s fade: about 900 entities despawned and 1,100 spawned, 12 NPCs placed from their schedules | Operation | Spawning, persistence, schedules |
| `lw.save_world` | Capturing and writing the persistent world, about 300 KB | Operation | Saves |
| `lw.dialogue_ui` | A 40-line conversation revealed at 40 characters a second, four choices, a portrait, rich text | Frame | UI layout and text |
| `lw.inventory_ui` | 24 slots, 4 equipment slots, a tooltip and a drag | Frame | UI |
| `lw.boot` | About 2,750 images (frames, normal maps and masks), 79 sounds and 17 maps to the first frame | Operation | Boot loading (§5.4) |

### Build sheet

Verdicts: **data only** (prefabs and data kinds alone), **game Rust** (the engine gives the parts and the game writes the rest, of the size shown), **gap** (a part the design needs that no item provides, with the game Rust it would take today where the game could write it at all), **blocked** (a rule or the architecture forbids it). Each entry's evidence is a span to search for in `api/*.txt` (`core:` `tungsten-core.txt`, `kit:` `tungsten-kit.txt`, `umb:` `tungsten.txt`, `comp:` `tungsten-kit-components.txt`), or a `none:` pattern that matches no line of `api/*.txt`, case-insensitive; other files are named with their line.

- **LW1 · Walking, facing, depth and the camera · data only.** `warden.json` and every NPC's prefab: `top_down_mover` at 140 px/s with acceleration and deceleration, `action_input` (four move actions; slots `interact`, `run`, `lantern`, `throw`), `facing` four-way from the intent, `character_animator` with idle and walk clip sets by facing, `depth_sort`, a kinematic `rigid_body`; the warden adds `aim` from the cursor and `camera_target`. NPCs and fogborn drive the same mover by writing their `Intent`. Running and the action clips are LW28's. Evidence: `comp: component top_down_mover:`, `comp: field snap: one of free | horizontal | four | eight`, `comp: an animation ID, or { left, right, up, down } of animation IDs by facing`, `comp: component depth_sort:`, `comp: component camera_target:`, `kit: tungsten_kit::Aim::cursor`, `kit: tungsten_kit::Intent::set_movement`.
- **LW2 · Roofs, canopies and eaves over walkers · gap: a tile layer drawn among sprites by z.** Tile layers draw under every sprite, so each overhang is a prefab with a sprite and a high `depth_sort` base, placed from the map's object layer: one per roof and canopy, about 120 in the Square. A placed tilemap holds an ID and an origin, nothing that orders it. Evidence: `comp: field base: i64`, `core: tungsten_core::World::spawn_map_objects`, `core: tungsten_core::assets::tilemap::TilemapInstance::origin`; searched `none: TilemapInstance::.*z|layer_?z|tile_?z`; `docs/plans/1.0/backlog-1.x.md:36` ("Tile layers ordered among sprites by z").
- **LW3 · Interact prompts (talk, light, refill, open) · game Rust (~150 lines: the search, the prompt, the event).** Each frame the nearest interactable within 40 px through `physics::nearest` over sensor bodies; a prompt whose text comes from the action's binding (`UiTree::set_prompt`); an `Interacted` game event on the `interact` press, which `CuesOnEvent` hands to data cues (a door's sound, a lamp's flare). Zones report entry only, so they cannot hold "in range". Evidence: `core: tungsten_core::physics::nearest`, `core: tungsten_core::SpatialFilter::with_sensors`, `core: tungsten_core::ui::tree::UiTree::set_prompt`, `kit: tungsten_kit::Intent::pressed`, `kit: tungsten_kit::CuesOnEvent<E>::new`; `docs/plans/1.0/backlog-1.x.md:54` (`Interactable` among the kit leaf items).
- **LW4 · Dialogue: data, conditions, runner · gap: a dialogue graph, a condition vocabulary and their checks (8.1); about 1,100 lines of game Rust today** (runner 400, conditions 300, effects 150, checks 250). A game data kind `dialogue` (nodes, speaker, text, choices, conditions, effects) is registered and checked at load, its references through `DataRef`, and its state lives in kit `flags` and `counters` on the game. Nothing evaluates a condition, walks a graph, or checks that every node is reachable and every speaker exists: the kit's cue rules filter by tags and damage kinds alone. Evidence: `core: tungsten_core::World::register_data_kind`, `core: tungsten_core::assets::data::DataRef<T>::check`, `comp: shape cue_rule:`, `comp: field kind: tag filter = {} — the occurrences it runs for by their damage kinds`; `templates/basic/AGENTS.md:37` ("A reaction that needs a condition beyond a rule's tag filter is Rust"); searched `none: dialogue|conversation|speaker` (`dialog` alone finds only the UI's `Role::Dialog`, an accessibility role).
- **LW5 · The dialogue box: portrait, rich text, choices, reveal · game Rust (~300 lines), with a gap for text revealed without relayout.** A modal root: `panel`, `label` with `StyledText` spans for names and keywords, `image` for the portrait, a `button` per choice with focus moves. The typewriter is `set_text` each tick, which lays the root out again at every character. Evidence: `core: tungsten_core::ui::tree::UiTree::label`, `core: tungsten_core::text::TextSpan::color`, `core: tungsten_core::ui::tree::UiTree::image`, `core: tungsten_core::ui::tree::UiTree::set_modal`, `core: tungsten_core::ui::tree::UiTree::focus_direction`; searched `none: typewriter|reveal`.
- **LW6 · Quests and the journal · gap: a quest and journal model (8.1); about 600 lines of game Rust and 250 of UI today.** Objective counts work in data: `killed`, `picked_up` and `zone_entered` rules add to game counters (`quest.nets.mended`). Stages, objective text, completion conditions, rewards and the journal's entries have no model. Evidence: `comp: shape counter_action:`, `comp: field target: one of self | other | game = "self" — the subject's counters, the other entity's or the game's`, `comp: kind cue_rules:`; searched `none: \bquest|journal|objective`.
- **LW7 · Trust and factions · data only** (storage and changes; reading them is LW4's gap). A `counters` component on each named NPC (`trust`) and game counters per faction, changed by cue `counter` actions and by dialogue effects. Evidence: `comp: component counters:`, `comp: shape counter_action:`, `kit: tungsten_kit::Counters::add`.
- **LW8 · Items, inventory and equipment · gap: an item and inventory model, and drag and drop between widgets (8.1); about 700 lines today.** Equipment bonuses are kit effects: an infinite `effect` with stat and resist modifiers and granted tags (soft boots grant `quiet_steps`, which halves footstep noise), applied on equip and ended on unequip. Items, stacks, the 24 slots and moving one between slots by drag have no model or widget; the layout has rows and columns, no wrap. Evidence: `comp: shape effect:`, `comp: field grant: list of tag`, `comp: shape stat_modifier:`, `kit: tungsten_kit::end_effect`, `core: tungsten_core::ui::tree::UiTree::set_drag_handle` (it moves its own root only); searched `none: inventory|equip|item_?slot`, `none: drag_?payload|drop_?target|on_drop`; `docs/plans/1.0/backlog-1.x.md:52` ("Drag and drop between widgets").
- **LW9 · Shops and prices · game Rust (~250 lines on LW8's model).** A stock table per shop as a data kind, coins as a game counter, prices scaled by the owner's `trust`. Evidence: `core: tungsten_core::World::register_data_kind`, `kit: tungsten_kit::Counters::get`.
- **LW10 · The world clock, day and night · game Rust (~200 lines).** A game resource advanced by the game clock's delta, an event at dawn, dusk and night; the look rewritten each frame: the ambient light, the moon as a directional light, the post stack's fog density, colour adjust and LUT. Evidence: `core: tungsten_core::time::Time::delta`, `core: tungsten_core::lighting::AmbientLight`, `core: tungsten_core::components::Light::directional`, `core: tungsten_core::post::PostPass::Fog`, `core: tungsten_core::post::PostPass::Lut`; searched `none: time_of_day|calendar|world_?clock|day_?night|daylight`.
- **LW11 · NPC schedules and paths · gap: navigation on the tile grid with door links, named places, and schedules resolved while an area is unloaded (8.1, 8.2); about 900 lines of game Rust today: paths 500, schedules and their placing while unloaded 400.** Schedules as a data kind (an hour, a named place, an activity); `steering` arrives at each waypoint and `path_follower` walks fixed routes, but nothing finds a path round buildings, through doors or between areas, and nothing places an NPC whose area is not loaded. Evidence: `comp: field mode: one of seek | flee | arrive`, `comp: component path_follower:`, `kit: tungsten_kit::Target::point`; searched `none: path_?find|navmesh|astar|a_star|flow_?field|nav_?grid|navigat`, `none: \bnpc\b|routine`.
- **LW12 · Fogborn: hearing and the fear of light · gap: perception by hearing with memory, and a light-level query (8.2, 7.10); about 700 lines of game Rust today: the machine's hooks 300, hearing and memory 300, the light level 100.** A `StateMachine` per kind (drift, listen, stalk, lunge, flee) with hooks in `StateMachinePlugin`; `steering` toward a heard point or away from the brightest light; a `Noise` game event (a point and a radius) from footsteps, bells and crates; a memory component; a light level summed from the `Light` components near each fogborn every 0.25 s. They come from each fog bank's `spawner`, which the clock's Rust adds at night (LW10). Evidence: `kit: tungsten_kit::StateMachine<S>::go`, `kit: tungsten_kit::StateMachinePlugin<S>::on_update`, `kit: tungsten_kit::Steering::flee`, `umb: tungsten::app::App::register_event`; searched `none: perception|hearing|stimul`, `none: light_?level|light_?at\b|\blux\b|illumin`.
- **LW13 · Fogborn bodies, hits and drops · data only.** `health`, `hurtbox`, `resist` (blunt × 0.5, fire × 2), `hit_flash`, `knockback`, and a `killed` cue that rolls a `loot_tables` entry (salt, oil). Evidence: `comp: component resist:`, `comp: component hit_flash:`, `comp: field roll: loot_tables ID or entry`, `comp: kind loot_tables:`.
- **LW14 · The warden's tools: flask, salt, bell, pole · data only** (the pool's and the line's chemistry is LW16's gap, and who hears the bell is LW12's). Abilities: a flask is a `spawn` payload of a thrown `projectile` on `linear_motion`, whose `expired` cue spawns the oil pool's prefab; salt the same with a salt line; the bell spawns a noise prefab; the pole sweeps a circle round the warden, its own windowed `hitbox` armed for 0.2 s and centred on them, since a hitbox's offset cannot follow its owner's facing; flasks and salt are a stat each, which the ability's `cost` spends. Evidence: `comp: shape ability:`, `comp: field spawn: shape spawn_payload`, `comp: field hitbox: f32 (> 0) — seconds the entity's own hitbox is armed for`, `comp: field window: f32 (> 0) = null — seconds it hits for once armed`, `comp: shape ability_cost:`, `comp: field expired: list of shape cue_action`; `docs/plans/1.0/backlog-1.x.md:104` ("A hitbox that follows its owner's facing").
- **LW15 · Lamps: oil, lit and doused, hoods · gap: fuel (a drained stat that ends a status) and `become` keeping identity (7.10, §5.5); about 350 lines today.** A lamp prefab: the engine's `light` component (its scene name, `crates/tungsten/src/asset_loader/scene.rs:139`), an emissive flame, `stats` with an `oil` stat, `tags` (`lamp`, `wick`, `hooded`). Oil drains only as the cost of an ability on an `interval` trigger (01 uses its staff light from code each step), and nothing happens when it runs out, since an effect's `restore` only raises a base. Doused is another prefab, swapped by a despawn and a spawn, so the lamp's identity and save ID go with it. Evidence: `core: tungsten_core::components::Light::point`, `comp: field interval: f32 (> 0) — seconds of game time between uses`, `comp: shape ability_cost:`, `comp: shape effect_restore:`, `comp: field amount: f32 (> 0) — added to the base with each payload`, `comp: component stats:`; searched `none: fuel`, `none: \bbecome\b|replace_?prefab`; `docs/plans/1.0/backlog-1.x.md:95` ("A draining effect").
- **LW16 · Oil, fire, wet fog and salt on tiles · gap: §5.2's statuses on tiles with intensity, spread and transforms (7.10 on 7.8's runtime tiles); about 1,200 lines today, by 01's elements code.** Damage types and their ladders give effects to things with health; a tile holds no status, nothing spreads to neighbours, and a tile changes only by cloning its map, `set_tile` and `insert` again, after which the next step rescans every collision layer of every map. Evidence: `comp: kind damage_types:`, `comp: shape ailment_rung:`, `core: tungsten_core::assets::tilemap::TilemapData::set_tile`, `core: tungsten_core::assets::tilemap::TilemapRegistry::insert`; searched `none: tile_?status|cell_?status`, `none: TilemapRegistry::(get_mut|remove)`.
- **LW17 · More lights than the cap · gap: a light budget by importance (radius, intensity, a pinned player light; 7.15), and occlusion (Phase 8, P8-17).** 28 lights fall in the Square's view. Past 16 the cull keeps directional lights first, then sorts by distance to the view with ties in World order, so which lamps go dark follows spawn order, and a lamp dimmed to 0, rather than swapped for its doused prefab, still takes a slot; no wall stops a light. Evidence: `core: tungsten_core::lighting::LIGHT_CAP`, `umb: tungsten::light_extract::extract_lights`; searched `none: light_?budget|importance|priority`, `none: occlu`; `docs/known-issues.md:49` (`cull_to_cap`).
- **LW18 · The lantern's beam · gap: a cone light kind (Phase 8, P8-17).** The lantern is a point light; a 40° beam has no kind, and the fear it casts is LW12's query. Evidence: `core: tungsten_core::components::LightKind::Point`, `core: tungsten_core::components::LightKind::Directional`; searched `none: spot_?light|LightKind::(Spot|Cone)` (`cone` alone finds only the particles' `InitialVelocity::Cone`).
- **LW19 · Areas, doors and interiors · game Rust (~450 lines) today, on Tiled's numeric object IDs; 7.1's authored IDs and 8.1's loading between areas replace them.** Every area's tilemap loads at boot and never unloads. A door asks for a replace with a 0.4 s fade, and the new area's state spawns its `TilemapInstance` and map objects; what changed in an area while the warden was away (lamps, doors, items) is kept in a game resource keyed by map object ID and applied on return. Evidence: `umb: tungsten::state::StateStack::request_replace_transition`, `umb: tungsten::transition::TransitionEffect::Fade`, `core: tungsten_core::assets::tilemap::TilemapInstance::new`, `core: tungsten_core::World::spawn_map_objects`, `core: tungsten_core::assets::tilemap::MapObject::id`; searched `none: unload`, `none: \bload_?area|area_?load`.
- **LW20 · An area loading while play goes on · gap: area loads in main-thread slices (8.1, by §5.4's slicing); a worker thread for them is blocked by the two-thread rule.** §5.4 slices loading on the main thread for boot alone, and a loader on a worker thread would be a third background thread (`AGENTS.md:43` ("Only two background threads")). At this design's size every area fits in memory from boot, so it fades instead, and what 8.1 owes it is spawning and persistence, not background reads. Searched `none: unload|worker|thread|async`.
- **LW21 · Cutscenes · gap: cutscenes and timelines (8.1); about 400 lines of game Rust today.** The camera in its scripted mode, tweens on actors' positions, lines from LW4, fades from transitions, music from faded plays; nothing sequences them, waits on a line or skips them. Evidence: `core: tungsten_core::camera::CameraMode::Scripted`, `core: tungsten_core::tween::Tween::with_channel`, `core: tungsten_core::tween::TweenChannel::PositionX`, `core: tungsten_core::audio::AudioCommands::play_faded`; searched `none: cutscene|timeline`.
- **LW22 · Saves of a persistent world · game Rust (~1,500 lines, by 01's `save.rs`, 1,634 lines for one map): no generic capture or restore.** One `SaveData` type through `SaveSlots`, written to a slot or to one of the shell's three autosaves (`templates/basic/AGENTS.md:19`). The capture walks every area's changed objects, the 22 NPCs, 60 lamps, quests, flags, counters and tile statuses, and the restore writes them back; effect sources and entity references are named again by hand. Evidence: `core: tungsten_core::save::SaveSlots::save`, `core: tungsten_core::save::SaveData::SCHEMA`, `core: tungsten_core::save::SaveData::migrate`, `kit: tungsten_kit::Effects::set_source`.
- **LW23 · The HUD and the map screen · game Rust (~350 lines).** Labels, images and a mesh for the clock's dial; flasks and salt from their stats; health from `Health`; the map is a drawn image per area with lamp markers placed by translation. Evidence: `core: tungsten_core::ui::tree::UiTree::mesh`, `core: tungsten_core::ui::tree::UiTree::set_translation`, `kit: tungsten_kit::Stats::base`, `kit: tungsten_kit::Health::current`.
- **LW24 · Barks over NPCs · game Rust (~80 lines).** A world label over the speaker, pushed each frame for 2.5 s once a zone is entered or the clock turns, since a label lasts one frame; the kit's `label` cue floats an event's amount, not a line. In the fog a bark wants an outline, which neither UI text nor world labels draw (K5). Evidence: `docs/plans/1.0/example-01-feedback.md:292` ("UI text has no outline or shadow"), `core: tungsten_core::text::WorldLabel::color`, `crates/tungsten-core/src/text.rs:560` ("a label lasts one frame"), `core: tungsten_core::text::WorldLabels::push`, `core: tungsten_core::text::WorldLabel::with_offset`, `comp: field label: shape label_style — the event's amount floated at its position`.
- **LW25 · Naming the warden, notes on the map · gap: text input (8.1).** No widget takes typed text, and raw input carries key codes, not characters, so a game cannot build one for every keyboard layout. Evidence: `core: tungsten_core::ui::node::WidgetKind::Toggle` (one of the seven kinds), `core: tungsten_core::input::RawInput::Key::code`; searched `none: text_?input|text_?field|textedit|caret|\bime\b`, `none: RawInput::(Text|Char)|ReceivedCharacter`; `docs/plans/1.0/backlog-1.x.md:14` ("Editable text fields and IME").
- **LW26 · Ambience, night layers, footsteps, voices · game Rust (~200 lines), with gaps for pan and bus ducking.** Loops started together and faded per voice, as 01's combat layer is, and the music turned down voice by voice under dialogue, since a bus neither fades nor ducks; footsteps chosen by the tile under the walker; one-shots through kit `sound` cues; no pan, no pitch. Evidence: `core: tungsten_core::audio::AudioCommands::set_voice_volume`, `core: tungsten_core::assets::tilemap::TilemapData::tile_at`, `comp: shape sound_action:`; searched `none: \bpan\b|panning|stereo|pitch`, `none: duck`; `docs/plans/1.0/backlog-1.x.md:83` ("Stereo panning per voice"), `:69` ("Bus fades and ducking").
- **LW27 · Input and menus · data only** (keyboard and mouse; the pad is SF3's). `input.json` bindings, the UI's accept, cancel and move actions, prompts from a binding's label, the template's keybindings screen (`templates/basic/src/shell/keybindings.rs`). Evidence: `core: tungsten_core::ui::route::UI_ACCEPT`, `core: tungsten_core::ui::route::UI_CANCEL`, `core: tungsten_core::input::action_map::Binding::label`.
- **LW28 · Running and the action clips · game Rust (~80 lines: the run 30, the clips 50).** Holding `run` adds a `move_speed` modifier while the slot is held, which the mover's speed follows, since an ability fires on the press alone; the throw, refill and light clips play over the walk from code, since the animator picks idle, move, rise and fall only. Evidence: `kit: tungsten_kit::Intent::held`, `kit: tungsten_kit::Stats::add_modifier`, `comp: field action: text — an intent slot, used when pressed`, `comp: component character_animator:`; `docs/plans/1.0/backlog-1.x.md:87` ("One-shot clips for the kit's character animator").

Verdicts: 5 data only, 10 game Rust (about 3,560 lines), 13 gaps (about 6,200 lines of game Rust where today's game could write them), none blocked.

### One minute of play

Day 3 on the Docks, 19:40 to about 21:20 in game time: 15 s of dusk (an hour takes 45 s), then 45 s of night (an hour takes 33 s). Each fixed step runs the kit's phases in order: Tick, Intent, Move, the physics step, Overlap, Resolve, React; Present runs in `PostUpdate`, before the camera.

| Time | What happens | Systems and events, in order | Saved |
| --- | --- | --- | --- |
| 0:00 | The harbourmaster's last line; the warden chooses "Show me the ledger" | The dialogue runner (LW4) applies the node's effects: game flag `harbour.ledger_shown`, his `trust` +1, quest `ledger` to stage 2; the modal root is removed and input goes back to the game | The flag, the trust, the stage |
| 0:03 | The warden walks to lamp D7 | Intent reads the move actions into the `Intent`; Move turns it into the mover's velocity; the physics step stops it at walls; Present sets the facing and the walk clip and sorts by depth; footsteps send `Noise` (64 px) | — |
| 0:06 | "E Refill" over D7; the warden refills it | The interact search (LW3) finds D7 within 40 px and shows the prompt; the press sends `Interacted`; the game fills D7's `oil` to 14 and spends one of the warden's oil; a cue plays the refill's sound | D7's oil, the warden's oil |
| 0:15 | 20:00: the bell, and night | The clock (LW10) sends its night event; a cue rule rings the bell; over 3 s the ambient light falls to 0.12, the fog pass thickens and the night LUT comes in; the fog's intensity starts its climb to 3; the clock's Rust gives each fog bank its `spawner`, which has no switch of its own | The clock |
| 0:20 | A lurker drifts out of the fog bank, 300 px east | The bank's `spawner` spawns the `lurker` prefab; its machine starts in `drift` | — |
| 0:24 | The warden runs east to lamp D8; 150 px from the lurker, it hears and stalks | Running footsteps send `Noise` (160 px); the lurker's hearing (LW12) raises `startled` and keeps the point; its machine goes from `drift` to `stalk`, steering toward the point | — |
| 0:30 | An oil flask, thrown toward the cursor | The `throw` press uses `oil_flask` (one flask spent, 1 s cooldown): `AbilityUsed`, then the spawn payload's flask with its velocity; the flask flies (Move) and ends at its range: `ProjectileExpired`; its `expired` cue spawns the oil pool, and the game's chemistry (LW16) marks the cells `oiled` 3 within 48 px | The flask count, the oiled cells |
| 0:33 | The lantern's flare lights the pool | The `lantern` press uses `flare` (one oil): heat on the `oiled` cells applies `burning` 3 and consumes the oil; the fire spawns its light and particles | The burning cells and their fuel |
| 0:35 | The lurker stalks into the light, flees and burns | Every 0.25 s the light query (LW12) builds the lurker's `lit` to band 3, granting `fleeing`, and its machine flees the fire's point; caught at the pool's edge, it takes `burning`, which queues fire damage each 0.5 s, doubled by its resistance, and adds 2 to `lit`, whose band 4 burns it as well (rules 1 and 12); Resolve sends `Damaged`; the hit flash and the `damaged` cue's sound | — |
| 0:36 | The fire takes a drying net 30 px from the pool | Each 0.5 s the burning cells emit heat within 40 px (rule 7); the net (`flammable`) builds `burning` a band a second from 0:34 and is alight by 0:36 | The net's burning |
| 0:38 | The lurker dies; a salt pouch drops | Resolve's `Damaged` with `killed`; the `killed` cue rolls `lurker_drops` and spawns the pouch; `despawn_on_death` takes the body 0.5 s later | — (fogborn are not saved once their area is left) |
| 0:48 | The fog thickens but does not douse | The fog's intensity reaches 2; `wet` at 2 consumes no `burning` (rule 4 starts above band 2) | — |
| 0:50 | The warden takes the pouch | The hurtbox meets the pouch's `pickup`: `PickedUp`; the `picked_up` cue queues an instant effect on the warden whose `restore` adds one to the `salt` stat's base, and plays a sound | The salt count |
| 0:55 | The net burns out | Its fuel spent, it becomes `net_charred`: today a despawn and a spawn (LW15) | The net's charred state |
| 0:56 | The warden enters the harbourmaster's office | The interact press on the door asks for a replace with a 0.4 s fade; the Docks' exit records its changes by map object ID (LW19); the office's state spawns its map and objects; the area change asks the shell for an autosave | The autosave: day 3, 21:14; the warden in the office with 2 flasks, 1 salt and 4 oil; the trust, flag and quest above; D7 lit, at about 12.6 oil; the net charred; the oiled and charred cells |

## 3. Static Front

A top-down tactical twin-stick shooter. The Corvane Annex, a research facility under a mountain, sealed itself when its security system, the Custodian, turned its units on the staff. A contract breacher goes in one wing at a time. A run is one wing: eight rooms drawn from that wing's templates by the run's seed, the last its control room, where one of the Custodian's cores fires bullet patterns. Dying extracts the breacher with the salvage carried; salvage buys weapons and gadgets at the terminal between runs. Clearing all three wings opens the Custodian's last core.

Changes from the default pitch, and why:

- Each wing ends in a control room whose core fires rings, spirals and aimed fans, up to 360 bullets alive. With it the design covers what a shmup would (bullet patterns, density, despawn at the view's edge, spawners), which is the evidence 7.10 needs to drop the kit's `shmup` claim.
- Suppression is a status with intensity, built by near misses, that drives the AI: statuses doing more than damage.
- Burning cells make smoke, and smoke blocks sight but not bullets: chemistry feeding perception.
- Server carts and crates are dynamic bodies that stop rounds and are shoved by blasts: a hurtbox stops a round today, but no round bounces off a moving body, and a blast's push knows no mass or falloff.
- The pad comes first: two sticks, triggers, rumble and aim assist, where today no window reads a pad at all.
- Weapons are one data table, and a run saves as its seed plus what changed.

### Numbers

| Thing | Number |
| --- | --- |
| Tile | 32 px; the view shows 40 × 22.5 tiles at zoom 1; the camera leads the aim by up to 96 px |
| Wings | 3 (Labs, Reactor, Archive), each 8 room templates and a control room: 27 templates |
| Rooms | Small 32 × 24 tiles, medium 48 × 32, large 64 × 40, control 40 × 40 |
| Run | 8 rooms of one wing, about 25 minutes; rooms 1–7 drawn from the wing's 8 templates by the seed, room 8 its control room; a daily seed shared by every player |
| Breacher | 100 health, 50 armour; 220 px/s, accelerating at 1,600 px/s²; a dodge of 0.32 s at 360 px/s with 0.22 s of i-frames, 0.9 s cooldown; two weapons and a gadget |
| Weapons | 12 rows of one table: sidearm, SMG, shotgun, burst rifle, rail, coil carbine, cryo sprayer, flamer, grenade launcher, arc thrower, minigun and the prototype "Static" |
| Gadgets | Frag, EMP, smoke, decoy (a noise) |
| Enemies | 6 kinds: sentry drone, trooper, bulwark (frontal armour), lobber (grenades), spotter (sniper with a 0.8 s laser tell), hound (melee, packs of 3); 3 cores |
| Squads | 2–4 a room of 3–5 units; 24 awake at most, in a large room |
| Bullets | Up to 60 the breacher's and 400 the enemies' alive, 360 of those a core's |
| Bodies | Up to 300 debris chunks, 12 canisters and 20 carts or crates a room |
| Terrain | Drywall, glass, crates and server racks break; up to 500 destructible cells in a large room |
| Statuses | 7 with intensity, `burning` 0–5, `wet` 0–3 (tiles), `oiled` 0–3 (tiles), `emp` 0–3, `suppressed` 0–5, `chilled` 0–5 (frozen at 5 for 2 s) and `smoke` 0–5 (tiles), and `electrified` (tiles), on or off |
| Interactions | 16 rules (table below) |
| Text | 120 lines: briefings, the Custodian's lines, 40 terminal logs |
| Screens | Title, loadout, wing map, HUD, perk offer (three cards after rooms 2, 4 and 6), terminal, codex, run summary, pause, settings and saves (template) |

### Statuses and interactions

| # | Stimulus or status | Receiver | Result |
| --- | --- | --- | --- |
| 1 | blast (canister, frag, grenade) | `explosive` | detonates after 0.15 s, a chain |
| 2 | blast | `destructible` cells | damages the cell; at 0 it becomes rubble, walkable and no cover |
| 3 | blast | bodies | a radial impulse by distance and mass |
| 4 | `heat` | `flammable` (carpet cells, paper, crates) | builds `burning` |
| 5 | `burning` ≥ 2 | itself | emits `heat` in 40 px and `smoke` +1 on its cell each 0.5 s |
| 6 | `smoke` ≥ 2 | a cell | blocks sight (not bullets); spreads to neighbours, decays 0.5 a second |
| 7 | `burning` ≥ 4 anywhere in a room | the room | the sprinklers run: `wet` 2 on every cell for 10 s |
| 8 | `wet` | `burning` | consumes 1 `burning` each 0.5 s |
| 9 | shock (arc thrower, a cut cable) | `wet` cells | `electrified` for 3 s: 5 a second to whatever stands on it, unless `insulated` |
| 10 | EMP | `conductive` | builds `emp`: band 1 halves turning, band 2 jams weapons, band 3 shuts down for 3 s (drones fall) |
| 11 | EMP | `powered` doors | open, or jam for 5 s |
| 12 | a near miss: a bullet passing within 40 px of a unit of another team | the unit | builds `suppressed`: −6% accuracy a band, cover sought at band 3, pinned at band 5; decays 1 a second after 1.5 s without |
| 13 | cold (the cryo sprayer) | units with an `ailment_threshold` | builds `chilled`: band 3 halves speed, band 5 freezes 2 s; frozen and blasted, it shatters |
| 14 | impact: a body faster than 300 px/s | a unit | damage by mass and speed |
| 15 | blast | a fuel canister | `oiled` 3 in 64 px, then `burning` |
| 16 | `burning` | `conductive` racks | a 48 px shock pulse each second |

Perception is the AI's, not chemistry: sight in a 100° cone to 288 px, checked every 0.2 s, blocked by walls, shut doors and `smoke` ≥ 2, not by glass; hearing of shots (400 px), blasts (640 px), dodges (96 px) and the decoy (480 px); memory of the last known point for 8 s, shared by the squad at once and with squads within 600 px after 0.5 s.

### What a save holds

The meta save: salvage, 30 unlocks, 20 codex entries, best times, the daily seed's best. The run save, written at each room's door: the seed, the wing, the room index and the cleared rooms; the breacher's health, armour, weapons and their rounds, gadget charges and perks; the run's stats. Quitting mid-room restarts that room on load, so nothing inside a room is saved. Under 16 KB. A room is never saved: it regenerates from the seed.

### Lighting, post and chemistry

- **Lighting:** emergency strobes (point lights pulsing red), muzzle flashes (a point light for 0.05 s), fires and the cores' glow; an EMP drops the ambient to 0.05 for 2 s and leaves only the emissive screens, eyes and laser tells. Up to 40 lights fall in a large room's view, past the cap of 16.
- **Post:** bloom on muzzles and fire, a chromatic aberration pulse when the breacher is hit, glitch during an EMP, a CRT pass on the briefings' security feed, a vignette at low health.
- **Chemistry:** the 16 rules above; blasts transform cells, and smoke changes what units see.

### Production list

Through §6's asset sessions on today's generators, as Lantern Ward's.

| Kind | Count | Made with today's tools | What the pipeline lacks |
| --- | --- | --- | --- |
| Characters | The breacher 24 frames (legs 6, dodge 4, hit 2, death 12); 6 unit kinds × 15 (idle 2, move 4, fire 2, hit 1, death 6) = 90; 3 cores × 20 = 60; 12 held weapons: about 186 | Drawn facing up and turned by `facing`, so one set serves every direction; hard-edged machines suit the procedural generator | One PNG and one manifest entry a frame: about 860 image files with the normal maps and masks (7.9's sheets) |
| Tiles | 3 wings × 3 wall materials × 16 neighbour masks = 144 walls, 30 floors, 30 rubble cells: about 200, with normal maps | Generator families | The variant per cell is written by the room generator until 7.8's auto-tiler, and a broken cell's neighbours keep their old variant |
| Props | 60: carts, crates, racks, three canisters, terminals, desks, glass, doors in four states; normal maps, and emissive masks for screens and eyes | Generator families | — |
| Effects | About 70 sprites (blasts in three sizes of 8 frames, sparks, smoke, fire, the EMP ring, casings, 12 debris chunks, muzzle flashes) and 20 particle configs | Generator families and particle JSON | — |
| Rounds and icons | 14 round sprites, 12 weapon icons, 12 pickups | Generator | — |
| Sounds | 12 shots, 6 reloads, 8 impacts by material, 4 blasts, 2 for the EMP, 3 loops (fire, sprinkler, shock), 18 unit sounds (six kinds × alert, tell, death), 6 for the cores, 10 for the UI: about 70 | The synthesizer; shots and machine barks suit synthesis | No pitch variation without more files; 64 voices against a minigun's 25 shots a second |
| Music | 3 wing loops with a combat layer each, 3 core themes, the title: 10 | The synthesizer | As Lantern Ward's |
| Rooms | 27 templates: 24 rooms and 3 control rooms, with markers for squad slots by role, cover, canisters and door sockets, and each cell's material | A compact JSON per template and a generator to `.tmj` | A run is checked only by the game's own test over seeds; §5.1's `check` over many seeds, IDs from the seed, and saves as the seed plus changes are 7.12's |
| Text | 120 lines | A data kind | — |
| Weapons | 12 rows | `abilities` entries and a game `weapons` kind for what abilities lack (SF6) | — |
| Enemy kinds | 6 units, 3 cores | Prefabs and SF10–SF13's Rust | — |

### Five weekly agent tasks

1. **A room template.** "Add a coolant hall to the Reactor wing: two rows of cover, a pair of canisters, three squad slots, doors north and east; it must generate cleanly in 500 seeds." Done when the seed sweep passes and a run places it. Runs into SF22 and the rooms' authoring.
2. **A weapon.** "Add the pulse carbine: EMP rounds, three-round bursts, 4° spread, a 40 px/s kick back, 24 rounds, a 1.6 s reload." Done when its row loads and a test fires a burst that drops a drone. Runs into SF6 and SF18.
3. **An enemy.** "Add the bulwark: front armour, advancing under suppression, a bash inside 48 px." Done when tests show front hits scaled, rear hits whole and an advance while suppressed. Runs into SF12, SF14 and SF20.
4. **Tuning.** "Units stay pinned too long: after 1.5 s without a near miss, the top band should drain twice as fast." Done when a test of the decay passes. Runs into SF14.
5. **A bug.** "Lobbers throw grenades through shut blast doors; a throw must respect a shut door." Done when a test with a shut door passes. Runs into SF10 and SF21.

### Stress numbers

| Row | Workload | Category | Surfaces |
| --- | --- | --- | --- |
| `sf.core_patterns` | A control room: 360 of a core's rounds and 60 of the breacher's alive, 24 hurtboxes, 300 debris bodies, 40 lights in view | Frame | The hit grid, projectiles, bodies, the light cull |
| `sf.squads_awake` | 24 units awake: 120 sight rays a second, 40 noises a second, 24 paths on 64 × 40 cells, cover queries | Frame | Perception, navigation, the query index |
| `sf.blast_chain` | 12 canisters chaining over 2 s: 120 cells broken in one step, 24 repaths, 300 debris spawned | Frame (a spike) | Runtime tiles, collision rescans, spawning |
| `sf.fire_smoke` | Fire across 1,500 carpet cells, smoke blocking 120 rays a second | Frame | Tile statuses and spread, perception |
| `sf.run_generate` | Building a run's eight rooms (8 map clones, about 3,200 objects) and entering the first | Operation | Generation, spawning |
| `sf.seed_sweep` | 1,000 seeds generated and checked headless | Scaling | Generation checks |
| `sf.round_churn` | 25 rounds a second from the minigun with 360 pattern rounds every 2 s, spawned and despawned | Scaling | Spawn and despawn churn |

### Build sheet

Verdicts and evidence as Lantern Ward's.

- **SF1 · Twin-stick movement and aim · data only** (the pad waits on SF3). `breacher.json`: `top_down_mover` at 220 px/s (acceleration 1,600, deceleration 2,000), `action_input` with `move` on the left stick and WASD and `aim` on the right stick's four directions (slots `fire`, `alt`, `dodge`, `reload`, `gadget`, `swap`), `aim` from the intent, `facing` from the aim with `turn` (the art drawn facing up, offset π/2); the camera's marker is SF29's. The kit reads each stick's deflection, not a press. Evidence: `comp: field aim: list of text = null — [left, right, up, down]: the aim actions`, `comp: field from: one of intent | cursor | movement | target = "intent"`, `comp: field turn: bool = false — writes the entity's rotation as the direction's angle plus`, `core: tungsten_core::input::action_map::ActionMap::value`.
- **SF2 · The dodge · game Rust (~40 lines: its direction).** In data, an ability on the `dodge` slot with a 0.9 s cooldown whose `effect` payload grants `state.dodging` for 0.22 s, and enemy hitboxes and touches that filter out `state.dodging`. The push must follow the move stick, but an ability's direction is its aim's, so an `impulse` payload would throw the breacher toward the aim: a system calls `knock_back` along the intent's movement when the ability is used. Evidence: `comp: field self: {} — the entity itself, in the direction its aim or its intent gives when either does`, `comp: field impulse: f32 (>= 0)`, `kit: tungsten_kit::knock_back`, `kit: tungsten_kit::Intent::movement`, `comp: field filter: tag filter = {} — the hurtboxes it hits, by their entity's tags`, `comp: field grant: list of tag`; `crates/tungsten-kit/src/intent/ability.rs:1286-1298` (the direction is the `Aim`'s, else the intent's aim).
- **SF3 · Reading a gamepad · gap: the backend (W5a's re-landing of M54 step 5).** Core reads analog values, the kit combines them and clamps their length to 1, and a stick reads 0 below 0.35 on each axis; but no window reads a pad, since the `gilrs` backend never landed, and there is no radial dead zone or sensitivity setting. Evidence: `core: tungsten_core::input::action_map::ActionMap::value`, `core: tungsten_core::input::STICK_RELEASE`, `core: tungsten_core::input::GamepadAxis::RightStickX`; `docs/known-issues.md:42` ("no gamepad backend"); `docs/plans/1.0/backlog-1.x.md:72` ("Stick deadzone and sensitivity"), `:89` ("The pad's aim").
- **SF4 · Rumble · blocked: the two-thread rule.** Force feedback runs a thread of its own, which `AGENTS.md`'s hard rules forbid until the rule is amended for it. Evidence: searched `none: rumble|vibrat|force_feedback|haptic`; `docs/plans/1.0/backlog-1.x.md:40` ("Rumble (force feedback)").
- **SF5 · Aim assist and the mouse · game Rust (~120 lines).** In the intent phase, bend the aim toward the nearest enemy within a 12° cone and 400 px; switch `aim` between the intent (pad) and the cursor (mouse) by the device last used. Evidence: `kit: tungsten_kit::Intent::set_aim`, `kit: tungsten_kit::Phase::Intent`, `kit: tungsten_kit::Aim::cursor`, `kit: tungsten_kit::Aim::intent`; searched `none: aim_?assist`.
- **SF6 · Weapons as one table · gap: weapon payloads (8.2); about 400 lines of game Rust today, as 01's gun is its own.** Each weapon is an `abilities` entry: `cooldown` at 1 / rate, `charges` as the magazine with its reload as an `all` refill, a `spawn` payload of the round's prefab at its speed and muzzle offset, `ability_used` cues for the flash, the kick and the sound; the coil carbine's rounds turn half their physical damage into `damage.emp` with `conversion`, and the rail's carry `penetration` against armour. Missing: spread and bloom (a payload fires exactly along the aim), pellets in a fan, bursts, recoil (an impulse is at least 0 along the aim, never back), firing while held (an `action` trigger fires when pressed), a timed manual reload (an `ability` action's `refill` gives every charge back at once), spin-up and charge; and the `all` refill starts at the first spent charge, not the last (K8). Evidence: `comp: field op: one of use | refill = "use" — used now, or every charge back and the cooldown cleared`, `comp: component conversion:`, `comp: component penetration:`, `comp: shape ability_charges:`, `comp: field all: bool = false — every charge comes back together`, `comp: field action: text — an intent slot, used when pressed`, `comp: shape spawn_payload:`, `comp: field impulse: f32 (>= 0)`, `core: tungsten_core::assets::particle::InitialVelocity::Cone::spread_deg` (spread exists for particles alone); searched `none: pellet|recoil`; K8 at `docs/plans/1.0/example-01-feedback.md:295` and `docs/known-issues.md:45`.
- **SF7 · Rounds: density, hits, walls · data only.** A round's prefab: `linear_motion`, `projectile` (range, pierce, bouncing off metal through `bounce_layers` and passing glass through `pass_layers`), `hitbox` (a 3 px circle, a typed damage map, a team filter), `despawn_outside` 64 px past the view. Every tile is in physics layer 0, so metal walls and glass panes are static colliders in layers of their own, as 01's mine iron is; walls that differ by tile are P7-1's. The kit's hit grid takes every hurtbox each step. Evidence: `comp: component projectile:`, `comp: field bounce_layers: list of u32 (<= 31) = null — physics layers whose walls it turns at while it has bounces left, tiles in layer 0`, `comp: field pass_layers: list of u32 (<= 31) = []`, `comp: component despawn_outside:`, `kit: tungsten_kit::HIT_GRID`; `docs/plans/1.0/backlog-1.x.md:100` ("Materials on tiles").
- **SF8 · Carts and crates that stop rounds · data only** (no round bounces off one, and a piercing round passes it: bodies as walls, P7-2). Each cart and crate carries a `hurtbox` and a `cover` tag that the rounds' hitbox filters take in: a round ends at its first hit, and a hurtbox needs no health to be hit. As a wall a cart does nothing, since a moving body is never one. Evidence: `comp: component projectile: Ends at its hitbox's first hit`, `comp: component hurtbox:`, `comp: field filter: tag filter = {} — the hurtboxes it hits, by their entity's tags`; `crates/tungsten-kit/src/overlap/grid.rs:89-95` (a hurtbox takes part unless its `Health` says it is dead); `docs/known-issues.md:76` ("a kinematic or dynamic body is never a wall"); `docs/plans/1.0/backlog-1.x.md:59` ("Walls for plain hitboxes and contact damage, and kinematic or dynamic bodies as walls").
- **SF9 · Hit-stop · gap: a hit-stop that keeps game-time windows (8.2); about 60 lines of game Rust today with the global stop.** `Time::set_scale(0)` for 50–90 ms, timed by the real delta, freezes the fixed step, physics, timers, particles, tweens and shake, while the UI and transitions run on real time. Example 01 turned it down because a time-scale stop shifts game-time windows, and no per-entity stop exists; a machine's hold is the nearest. Evidence: `core: tungsten_core::time::Time::set_scale`, `core: tungsten_core::time::Time::real_delta`, `kit: tungsten_kit::StateMachine<S>::set_held`; searched `none: hit_?stop|freeze_?frame`; `docs/plans/1.0/example-01-feedback.md:265` (Q6).
- **SF10 · Sight, hearing and memory · gap: perception with memory (8.2); about 550 lines of game Rust today.** Sight is a ray from each unit's eye to the breacher against tiles and the wall layers' mask, every 0.2 s and staggered, inside a 100° cone; hearing a `Noise` game event from shots, blasts, dodges and the decoy; memory the last known point and its age on a component, shared through the squad. 01 casts from each eye every 0.2 s in its own code. Evidence: `core: tungsten_core::physics::cast_ray`, `core: tungsten_core::SpatialFilter::with_tiles`, `core: tungsten_core::SpatialFilter::with_mask`, `umb: tungsten::app::App::register_event`; searched `none: perception|hearing|stimul`; `docs/plans/1.0/backlog-1.x.md:82` ("Line-of-sight sensing in the kit"), `:55` (`Perception`).
- **SF11 · Smoke that blocks sight · gap: tile statuses that sight reads (7.10, 8.2).** A ray sees tiles, bodies and sensors; a cell's smoke is none of them. A sensor body per puff that sight rays include stands in, but it neither spreads nor thins by cell. Evidence: `core: tungsten_core::SpatialFilter::with_sensors`; searched `none: tile_?status|cell_?status`.
- **SF12 · Squads: cover, flank, suppress, retreat · gap: AI structure, squads and cover queries (8.2); about 1,400 lines of game Rust today.** A `StateMachine` per unit (patrol, alert, take cover, fire, peek, flank, suppress, retreat, regroup) with hooks; a game `Squad` on the leader (members, a blackboard, morale); cover points worked out per template from cells beside cover and dropped when the cover breaks; a flanker takes a cover point more than 60° off the squad's line to the threat. Drones arrive at their posts by `steering` and patrol `path_follower` loops; hounds run capped by `speed_limit`. Evidence: `comp: component steering:`, `comp: component path_follower:`, `comp: component speed_limit:`, `kit: tungsten_kit::StateMachine<S>::go`, `kit: tungsten_kit::StateMachinePlugin<S>::on_update`, `kit: tungsten_kit::Intent::set_movement`; searched `none: squad|blackboard|utility`; `docs/plans/1.0/backlog-1.x.md:55` ("state charts, utility AI").
- **SF13 · Navigation on a grid that changes · gap: pathfinding with local repaths (8.2, on 7.8's runtime terrain); about 600 lines today.** Units path on the room's 32 px cells around cover and over rubble; a blast that breaks cells sends the units whose paths crossed them to path again. Evidence: searched `none: path_?find|navmesh|astar|a_star|flow_?field|nav_?grid|navigat`; `docs/plans/1.0/backlog-1.x.md:55` ("A\* and flow field").
- **SF14 · Suppression as a status · game Rust (~150 lines).** A `suppressed` effect of five stacks, each −6% to an accuracy stat, refreshed by each near miss and dropping one stack at a time; a system finds the near misses, a round's step passing within 40 px of a unit of another team. The AI reads the bands (cover at 3, pinned at 5) from the stack count. Evidence: `comp: shape effect_stack:`, `comp: field expire: one of clear | remove_one = "clear"`, `kit: tungsten_kit::Effects::stacks`, `kit: tungsten_kit::EffectRequest::queue`.
- **SF15 · Canisters, frags and chains · data only** (no falloff or mass, and nothing without a hurtbox takes a push: SF19). A canister has `health`, a `hurtbox`, `tags` (`explosive`) and a `killed` cue that spawns its fuse: a `cooldown` of 0.15 s counted once, whose `cooldown_elapsed` cue spawns the blast and despawns the fuse. The blast is an always-armed `hitbox` circle of 96 px on a 0.1 s `lifetime`, with a typed damage map and `knockback`. The blast kills the next canister, which chains. Evidence: `comp: field killed: list of shape cue_action`, `comp: component hurtbox:`, `comp: component cooldown:`, `comp: field count: u32 = null — how many times it elapses in all`, `comp: field cooldown_elapsed: list of shape cue_action`, `comp: always armed when left out`, `comp: component lifetime:`, `comp: field knockback: f32 (>= 0) = 0.0 — world pixels per second, pushing a target it hurts away from it`. A `lifetime`'s end sends no `expired` cue, which answers a projectile's end: `crates/tungsten-kit/src/react/cues.rs:103` ("`expired` (a projectile's end)").
- **SF16 · Breakable cells · gap: runtime terrain with per-cell health (7.8); about 350 lines today.** Cell health in a sparse game map; a round that ends on a wall or a blast's reach damages the cell; at 0 the map is cloned, `set_tile` writes rubble and `insert` stores it, and the next step rescans every collision layer of every map; the neighbours' art is not tiled again. Breaking 120 cells in one chain costs 120 clones unless the game batches them a step. Evidence: `core: tungsten_core::assets::tilemap::TilemapData::set_tile`, `core: tungsten_core::assets::tilemap::TilemapRegistry::insert`, `kit: tungsten_kit::ProjectileExpired`; searched `none: TilemapRegistry::(get_mut|remove)`, `none: destructib|tile_?health|cell_?health`.
- **SF17 · Fire, smoke, sprinklers, water and shock on cells · gap: §5.2's statuses on tiles (7.10); about 1,200 lines today.** As LW16: no tile status, spread or transform. Evidence: `comp: kind damage_types:`; searched `none: tile_?status|cell_?status`.
- **SF18 · EMP on conductive units · data only** (the AI's response to the stacks is SF12's Rust). An EMP round's `expired` cue spawns the pulse: an always-armed `hitbox` circle of 120 px on a 0.1 s `lifetime`, with `damage` `{ "damage.emp": 1 }`, a filter of `conductive`, and a `hit` cue whose `effect` actions queue `emp_jam` on the other entity: an effect of up to three stacks lasting 3 s, which a grenade's pulse queues three times and a coil round once. The AI reads the stacks as the bands: a slowed turn at one, jammed weapons at two, a shutdown at three. Evidence: `comp: field hit: list of shape cue_action`, `comp: shape effect_action:`, `comp: field target: one of self | other = "self" — the subject or the other entity`, `comp: shape effect_stack:`, `kit: tungsten_kit::Effects::stacks`, `comp: field filter: tag filter = {} — the hurtboxes it hits, by their entity's tags`.
- **SF19 · Debris, pushes and impacts · gap: radial impulses and impact damage (§5.6, 7.10); about 300 lines today.** Debris chunks are dynamic bodies spawned by `killed` cues; a blast pushes only what it hurts, at one speed: no falloff, no mass, nothing without a hurtbox and health (a dynamic body that has them takes the push in its velocity); a cart rolling at 400 px/s hurts nothing. 01 builds each of these in its own code. Evidence: `core: tungsten_core::physics::components::RigidBody::dynamic`, `comp: field knockback: f32 (>= 0) = 0.0 — world pixels per second, pushing a target it hurts away from it`, `crates/tungsten-kit/src/motion/knockback.rs:227` ("the push to the body's `Velocity` at once"); searched `none: blast|explosion|radial_?impulse`, `none: impact`; `docs/plans/1.0/backlog-1.x.md:91` ("Impact damage by speed and mass"), `:105` ("A radial blast payload").
- **SF20 · The bulwark's front armour · game Rust (~80 lines).** The bulwark's `resist` takes physical hits to a quarter; the game's hit takes the side from the round's velocity against the bulwark's facing, and sends a rear hit with full physical penetration, so it lands whole: 01's beetle shell, since a kit hit carries no angle. Its shield plate is a child sprite (`spawn_children`). Evidence: `comp: component resist:`, `comp: component spawn_children:`, `kit: tungsten_kit::DamageRequest::with_penetration`, `kit: tungsten_kit::Facing::direction`; searched `none: hit_?angle|hit_?side|direction_?of_?hit`; `docs/plans/1.0/backlog-1.x.md:98` ("A kit hit that carries its angle").
- **SF21 · Doors, locks and waves · game Rust (~120 lines).** Doors are static bodies swapped open or shut, and an EMP's `hit` cue sets a door's `door.jammed` flag (rule 11); a room locks at entry; waves come from `spawner` (prefab, interval, count) placed by the template; a game counter of living units, which `killed` lowers, opens the doors at 0. Evidence: `comp: component spawner:`, `comp: field count: u32 = null — how many it spawns in all`, `comp: shape counter_action:`, `comp: shape flag_action:`; searched `none: WaveSchedule|\bwaves\b`; `docs/plans/1.0/backlog-1.x.md:53` (`WaveSchedule`), `:54` (`Gate`, `Switch`).
- **SF22 · Rooms from templates · game Rust (~450 lines).** 27 templates as tilemaps, all loaded at boot; the run's seed through the kit's random stream; the layout (a chain of eight on a 3 × 4 grid) and the door matching in Rust; each room placed at an origin with its map cloned under a run-local ID (its cells break on their own), a `TilemapInstance` and its map objects; encounters rolled from `loot_tables` by a `roll` cue when each spawn marker's one-shot `cooldown` elapses, since no cue answers a spawn. Clones never leave the registry, so each run adds eight maps that stay. Evidence: `kit: tungsten_kit::KitRng::seeded`, `core: tungsten_core::assets::tilemap::TilemapRegistry::insert`, `core: tungsten_core::assets::tilemap::TilemapInstance::new`, `core: tungsten_core::World::spawn_map_objects`, `comp: shape weighted_table:`, `comp: field roll: loot_tables ID or entry`, `comp: field cooldown_elapsed: list of shape cue_action`; searched `none: TilemapRegistry::(get_mut|remove)`.
- **SF23 · Bullet patterns (the cores) · gap: a pattern payload (count, arc, spin, aimed or fixed); about 200 lines of game Rust today.** Spinning streams and aimed shots are data: emitters placed round the core, each a kinematic body on an `orbit` whose centre is the nearest `core`, so its velocity is the orbit's tangent, with an `interval` ability whose `spawn` payload fires along its aim, the aim from that movement or at the nearest `team.breacher`. An outward ring has no aim source (nothing aims away from a point), and fans, phase changes at health thresholds and a pattern's rotation speed are Rust. Evidence: `comp: component orbit:`, `comp: field center: shape target = null — the circle's centre`, `comp: field interval: f32 (> 0) — seconds of game time between uses, at most once a step`, `comp: field from: one of intent | cursor | movement | target = "intent"`, `comp: field nearest: shape nearest — the nearest live entity whose tags pass the filter, within the range`; `crates/tungsten-kit/src/intent/aim.rs:402-409` (movement is the intent's, else the velocity), `crates/tungsten-kit/src/motion/orbit.rs:46-47` (an orbit drives a kinematic body's velocity); searched `none: spiral|bullet_?pattern|\bfan\b`.
- **SF24 · Perk offers · data only.** A `level` on the breacher reading the game counter `rooms_cleared` along a curve (levels at 2, 4 and 6), with `offers` of three drawn from perks as effects or abilities; the card screen lists the offers and chooses one (SF25's UI). Evidence: `comp: component level:`, `comp: shape offers:`, `comp: shape offer_gives:`, `kit: tungsten_kit::Level::choose`, `kit: tungsten_kit::Level::offers`.
- **SF25 · HUD and run screens · game Rust (~600 lines).** Rounds as pips from the charges, the reload's sweep, health and armour from `Health` and `Shield`, a minimap of the rooms, threat marks at the screen's edge, the perk cards, the wing map, the run summary, the codex. The reload's sweep needs the refill time, which `Abilities` keeps private (01's HUD reads "reloading" instead). Evidence: `kit: tungsten_kit::Abilities::charges`, `kit: tungsten_kit::Abilities::cooldown_left`, `kit: tungsten_kit::Shield::current`, `core: tungsten_core::ui::tree::UiTree::panel`; `docs/plans/1.0/backlog-1.x.md:96` ("A public refill countdown on `Abilities`").
- **SF26 · Lights and post on events · game Rust (~150 lines), with LW17's cap gap.** Muzzle flashes are prefabs with a `light` and a 0.05 s `lifetime` from `ability_used` cues (data); strobes pulse a light's intensity, which no oscillator drives (it moves position, rotation or scale); the EMP blackout drops the ambient light for 2 s; a hit on the breacher pulses chromatic aberration and an EMP pulses glitch, by rewriting the post stack. Evidence: `comp: field ability_used: list of shape cue_action`, `comp: component oscillator:`, `core: tungsten_core::lighting::AmbientLight`, `core: tungsten_core::post::PostPass::ChromaticAberration`, `core: tungsten_core::post::PostPass::Glitch`, `core: tungsten_core::post::PostStack::as_slice_mut`.
- **SF27 · Meta and run saves · game Rust (~300 lines).** Two `SaveData` types: the meta, and the run (the seed, the room index, the cleared rooms, the breacher's state); a load builds the rooms again from the seed and restarts the current one, so no room's contents are saved. Evidence: `core: tungsten_core::save::SaveSlots::load`, `core: tungsten_core::save::SaveData::SCHEMA`, `kit: tungsten_kit::KitRng::reseed`.
- **SF28 · The spotter's laser tell · game Rust (~50 lines).** A thin sprite stretched by its transform's scale and turned to the aim, since a child keeps its own rotation (the hierarchy propagates positions only). Evidence: `core: tungsten_core::components::Transform::scale`, `core: tungsten_core::components::Transform::rotation`.

- **SF29 · The camera leading the aim · game Rust (~40 lines).** The camera follows the lowest-indexed `camera_target` with a dead zone and smoothing; the breacher spawns a lead marker, the run's only `camera_target`, and a system keeps it up to 96 px ahead of the breacher along its aim, since `follow`'s offset is fixed. Evidence: `comp: component camera_target:`, `core: tungsten_core::camera::CameraMode::Follow`, `core: tungsten_core::camera::CameraController::dead_zone_size`, `comp: field offset: [x, y] = [0.0, 0.0] — world pixels from the target's point`, `kit: tungsten_kit::Aim::direction`.
- **SF30 · Chill, freeze and shatter · game Rust (~60 lines: the shatter).** The cryo sprayer's `damage.cold` climbs a ladder as 01's does: rungs on units with an `ailment_threshold` apply `chilled` (a slowing stat modifier) and, at the top, `frozen` (2 s, granting `state.frozen`); a blast on a frozen unit shatters it in code, as 01's endings are. §5.2 replaces the ladders (P7-4). Evidence: `comp: shape ailment_rung:`, `comp: field needs: f32 (> 0) = null`, `comp: component ailment_threshold:`, `comp: field blocked_by: list of tag = []`.
- **SF31 · Units' bodies, hits and drops · data only.** `health`, `hurtbox`, `hit_flash`, `resist` by damage type (drones twice as weak to `damage.emp`), `knockback`, a recharging `shield` on drones, and a `killed` cue that rolls salvage from `loot_tables`. Evidence: `comp: component health:`, `comp: component hit_flash:`, `comp: component resist:`, `comp: component shield:`, `comp: field roll: loot_tables ID or entry`.

Verdicts: 7 data only, 12 game Rust (about 2,160 lines), 11 gaps (about 5,060 lines of game Rust where today's game could write them), 1 blocked.

### One minute of play

Room 5 of the Reactor wing, a large room of 64 × 40 cells: three squads (three troopers and a lobber; a bulwark and two troopers; three hounds).

| Time | What happens | Systems and events, in order | Saved |
| --- | --- | --- | --- |
| 0:00 | The breacher enters; the doors lock | The door socket's zone: `ZoneEntered`; the game locks the room (SF21); the run save is written | The run: the seed, room 5, rooms 1–4 cleared, the breacher's state |
| 0:02 | Trooper T1 sees the breacher at 250 px | Its sight ray (SF10), every 0.2 s and staggered, passes the cone and the walls; the memory gets the point; the squad's blackboard shares it; squad B hears of it 0.5 s later | — |
| 0:03 | Squad A takes cover | Each unit picks a cover point that blocks the line to the threat (SF12) and paths to it (SF13); its `Intent` drives the mover | — |
| 0:05 | The SMG, held | The held fire (SF6) uses the ability each cooldown with 6° spread and its bloom: the spawn payload's rounds fly in Move; Overlap's hit grid finds T2: `Struck`, then Resolve's `Damaged`; rounds passing within 40 px of T1 are near misses, and `suppressed` climbs to band 3 (SF14), so T1 stays down | — |
| 0:08 | T2 dies, with a hit-stop | The kill sets the time scale to 0 for 60 ms of real time (SF9); the `killed` cue spawns debris bodies, plays the sound and adds shake | — |
| 0:12 | Lobber L1 lobs at the last known point; the breacher dodges | The grenade's arc is `linear_motion`'s acceleration; `ProjectileExpired`; its `expired` cue spawns the blast; the dodge grants `state.dodging`, which the blast's filter skips, and the push follows the move stick (SF2) | — |
| 0:18 | An EMP at squad B | The pulse's `hit` cue queues `emp_jam` three times on the bulwark and on both troopers: band 3, so their machines hold for 3 s (SF18, SF12); the glitch pass pulses and the ambient light dips (SF26) | — |
| 0:22 | The hounds flank through the side corridor | Their flow field toward the breacher (SF13); steering; the pack's spacing | — |
| 0:26 | A round into the canister beside squad A | The canister's `health` reaches 0: `killed`; its fuse; the blast (SF15) kills T1 and the second canister, which blasts in turn; 14 drywall cells inside the blasts break (SF16): the map is cloned, set and stored, and the next step rescans collision; units whose paths crossed them path again, and cover points on them drop; 40 debris bodies spawn, which the blasts do not push, having no hurtboxes (SF19); the carpet catches (SF17) and smoke rises over four cells, blocking sight (SF11) | — |
| 0:34 | The sprinklers | `burning` reaches 4: `wet` 2 on every cell for 10 s (rule 7), consuming the fire; the smoke thins | — |
| 0:38 | The arc thrower on the wet floor by squad B | Shock on `wet` cells: `electrified` for 3 s, hurting the units standing in it (SF17) | — |
| 0:45 | Squad B breaks | Two dead puts its morale under 30%; it retreats to its rally point, the bulwark covering (SF12) | — |
| 0:52 | The hounds reach the breacher; the shotgun | Their bites are `hitbox` payloads with short windows; eight pellets in a fan (SF6's Rust); two hounds die | — |
| 0:58 | The last hound dies; the doors open | `killed` lowers the room's count of living units to 0; the doors open (SF21); `rooms_cleared` goes to 5, short of the next perk at 6 (SF24) | At the next door: room 6, rooms 1–5 cleared, the breacher at 64 health, 10 armour, the SMG at 22 of 40, two perks |

## 4. Registers

Rows in the analysis pass's shape (pattern, where, lines, what is lacking, the generic fix, dependencies), tagged by design: LW, SF, and 01 where 01's README, its feedback, or a backlog or known-issue row about it names the same seam.

- **Where** names the build sheet's entries, which hold the evidence; an entry in parentheses adds no lines here: it is counted in another row, is data only, or is game Rust with no row.
- **Lines** is the game Rust those entries take today, the code a generic fix would delete, each entry's estimate counted once. Game Rust that stays the game's own (the HUDs and screens, the event looks, the camera's lead, the laser tell, the doors: about 1,310 lines) has no row, so the rows hold about 15,670 of the build sheets' 16,980 lines.
- **Step** is where the row folds. A `+` marks a fix that the step's sketch in the roadmap redo §3 does not list yet. Rows on systems Phase 7 rebuilds fold into Phase 7; the rest scope Phase 8.

### Into Phase 7

| ID | Pattern | Designs | Where | Lines | Lacking | Generic fix | Depends on | Step |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| P7-1 | Tiles changed at runtime | LW, SF | SF16 (LW16) | 350 | No edit in place (the registry has no `get_mut`); every change rescans every collision layer of every map; no health or material per cell; no auto-tiler | Runtime terrain: per-tile properties (material, health), edits in place, re-outlined and re-tiled locally; the backlog's materials on tiles | — | 7.8 |
| P7-2 | Pushes, impacts and bodies as walls | SF, 01 | SF19 (SF8, SF15) | 300 | A push has no falloff or mass and reaches only what has a hurtbox and health; contacts carry no speed; a moving body is never a projectile's wall, so nothing bounces off one | §5.6's impulses, radial impulses and contact speeds; then K10's radial blast payload, impact damage, and bodies as walls by layer (`+`) in the kit | — | 7.8, 7.10 |
| P7-3 | Something in range | LW | LW3 | 150 | Sensors and zones report entry only | §5.6's sensor exit; a kit `Interactable` with P8-1 | — | 7.8 |
| P7-4 | Statuses on tiles and things: intensity, bands, spread, fuel, transforms | LW, SF, 01 | LW15, LW16, SF14, SF17, SF30 (SF11) | 2,960 | No tile status, spread, fuel or `become`; an effect's `restore` only raises; stacks stand in for intensity, and the AI reads bands in code | §5.2's chemistry with tile statuses, bands, fuel and `become` keeping identity; today's effects and ladders merged into statuses (Q17) | P7-1 | 7.10 |
| P7-5 | Light as a stimulus | LW, 01 | LW12's light level | 100 | No light level at a point; 01's frost crawler tests the staff light's glow in its own code | A core query for the light at a point from `Light` components, and `light` as one of §5.2's stimuli | P7-4 | 7.10 `+` |
| P7-6 | Ability fixes | LW, SF, 01 | LW28's run, SF2 (SF6, SF25) | 70 | An ability's direction is its aim's; an `action` trigger fires on the press only, so nothing acts while a slot is held; the refill time is private; the `all` refill starts at the first spent charge (K8) | A direction choice (aim or movement), a held trigger, a public refill countdown, K8's fix | — | 7.10 `+` |
| P7-7 | A hit's side | SF, 01 | SF20 | 80 | A kit hit carries no angle; 01's beetle builds its own | The backlog's hit that carries its angle, so armour by side is data | — | 7.10 `+` |
| P7-8 | Saves of many areas | LW, SF, 01 | LW22, SF27 (LW19) | 1,800 | No generic capture or restore; §5.5's save names one level | §5.5, with a save that holds each area's changes under its own level reference (`+`) | P7-9 | 7.9 |
| P7-9 | Map objects by name | LW | (LW19) | — | Area changes key by Tiled's numeric object IDs | 7.1's authored IDs | — | 7.1 |
| P7-10 | Overhangs over walkers | LW, 01 | LW2 | — | Every tile layer draws under every sprite; 01 draws its own extract | A tile layer's z among sprites in the default channel (backlog) | — | 7.9 `+` |
| P7-11 | More lights than the cap | LW, SF, 01 | LW17 (SF26) | — | A cap of 16 with ties in World order, and a lamp dimmed to 0 holding its slot; 01's lantern can drop | A cull by importance that pins the player's light and skips dark ones, or more lights | — | 7.15 |
| P7-12 | Reading a gamepad | SF, 01 | SF3 | — | No backend (`docs/known-issues.md:42`), so no window reads a pad; a per-axis dead zone only | Re-land M54 step 5 with its licence files; a radial dead zone and a sensitivity setting (`+`) | — | 7.1 |
| P7-13 | Audio seams | LW, 01 | LW26 | 200 | Cue loops keep no voice; no pan; a bus neither fades nor ducks | Looping cue voices (in 7.9's list); per-voice pan and bus ducking (`+`) | — | 7.9 |
| P7-14 | Text outline | LW | (LW24) | — | Neither UI text nor world labels draw an outline (K5), which barks in the fog need | K5, in 7.9's list | — | 7.9 |
| P7-15 | A file per frame | LW, SF, 01 | Both production lists | — | About 2,750 image files for LW and 860 for SF, each with its own manifest entry | 7.9's sprite sheets | — | 7.9 |
| P7-16 | Seeded generation | SF | SF22 (SF27) | 450 | No map removal, so a run's clones stay; checks over seeds are the game's own test; nothing saves a level as its seed | §5.1's generation: IDs from the seed, `check` over many seeds, saves as the seed plus changes; a registry `remove` (`+`) | P7-1 | 7.12 |
| P7-17 | Places, routes and rooms in our own format | LW, SF | (LW11, LW19, SF22) | — | Named places for schedules and tests, doors and sockets as links, a file per room | 7.12's format and CLI | P7-9 | 7.12 |
| P7-18 | The kit's genre claims | LW, SF | §5 | — | Dropping `grid` and `shmup` leaves 24 of 45 components under two genres | Tags for the four 1.0 genres, with these designs as the `rpg` and `shooter` uses (§5, G3) | — | 7.10 |
| P7-19 | One-shot clips | LW, 01 | LW28's clips | 50 | The animator picks idle, move, rise and fall alone; 01 shows its land, air-jump and toss clips from code | The backlog's one-shot clips over the state's clip | — | 7.10 |
| P7-20 | A hitbox by facing | LW, 01 | LW14 | — | A hitbox's offset is fixed, so LW's pole sweeps all round and 01's troll and goblin build their swings in code (K3) | The backlog's hitbox that follows its owner's facing | — | 7.10 |

### Scopes Phase 8

| ID | Pattern | Designs | Where | Lines | Lacking | Generic fix | Depends on | Step |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| P8-1 | Dialogue | LW | LW4, LW5, LW24 | 1,480 | A graph, a condition vocabulary, effects, reachability and reference checks, a format writers read, text revealed without relayout, a spoken line as a cue | 8.1's conditions and dialogue, with `check` for reachability, speakers and references, a dialogue box, and a `say` cue action (`+`); `Interactable` (P7-3) with it (`+`) | — | 8.1 |
| P8-2 | Quests and the journal | LW | LW6 | 850 | Stages, objectives' text, completion, rewards | 8.1's quests on P8-1's conditions | P8-1 | 8.1 |
| P8-3 | Items, inventory and shops | LW | LW8, LW9 | 950 | Items, stacks, slots, drag and drop between widgets; shops on top | 8.1's inventory with drag and drop; equipment stays kit effects | — | 8.1 |
| P8-4 | Cutscenes | LW | LW21 | 400 | Nothing sequences the camera, tweens, lines, fades and music, waits or skips | 8.1's timelines | P8-1 | 8.1 |
| P8-5 | Typed text | LW | LW25 | — | No text field; raw input has key codes, no characters | 8.1's text input with the window's characters; IME later | — | 8.1 |
| P8-6 | Clock and schedules | LW | LW10, LW11's schedules | 600 | No world clock; nothing places an NPC while its area is unloaded | A clock with phase events, and schedules resolved by time | P8-8 | 8.1 `+` |
| P8-7 | Loading between areas | LW | LW19, LW20 | 450 | Areas swap only by the game's own spawning and bookkeeping | 8.1's loading between areas, as spawning and persistence on §5.5, sliced on the main thread; no background reads at this size, so the two-thread rule stands | P7-8 | 8.1 |
| P8-8 | Navigation | LW, SF | LW11's paths, SF13 | 1,100 | No pathfinding | 8.2's grid navigation (A\* and flow fields) with door links and local repaths on 7.8's terrain; jump links stay 8.4's | P7-1 | 8.2 |
| P8-9 | Perception | LW, SF, 01 | LW12's hearing and machine, SF10 (SF11) | 1,150 | No sight with memory, hearing, sharing or fear of light; 01 casts its own | 8.2's perception: throttled sight rays, hearing as noise stimuli, memory that decays, sharing within reach, reading P7-4's smoke and P7-5's light | P7-4, P7-5 | 8.2 |
| P8-10 | Squads and cover | SF | SF12 | 1,400 | No AI structure, squads, blackboard or cover query | 8.2's AI structure (hierarchical states or utility scoring), squads, cover on the terrain (`+`) | P8-8, P8-9 | 8.2 |
| P8-11 | Weapons as data | SF | SF6 | 400 | Spread and bloom, pellets, bursts, recoil, a timed manual reload, spin-up, charge | 8.2's weapon payloads | P7-6 | 8.2 |
| P8-12 | Hit-stop | SF, 01 | SF9 | 60 | A time-scale stop shifts game-time windows; no stop per entity | 8.2's hit-stop (the backlog's `HitStop` leaf item) | — | 8.2 |
| P8-13 | Bullet patterns and waves | SF | SF23 (SF21) | 200 | No pattern payload (count, arc, spin, aimed or fixed), no aim away from a point, no waves | 8.2's bullet density with a pattern payload; the backlog's waves (`+`) | — | 8.2 |
| P8-14 | Rumble | SF | SF4 | — | Blocked: force feedback is a third thread | A decision on the two-thread rule for it, or none (G5) | P7-12 | 8.2 `+` |
| P8-15 | Drawn art | LW | The production list | — | 66 portraits and the 22 named NPCs' walk sets want an artist's hand, with no way in | 8.3's `asset add` and Aseprite import | P7-15 | 8.3 |
| P8-16 | Aim assist | SF | SF5 | 120 | — (game Rust today) | None until a second game asks | P7-12 | — |
| P8-17 | Light kinds and occlusion | LW | LW18 (LW17) | — | No cone kind; no wall stops a light | A cone light kind; occlusion optional | — | Phase 8 `+` |

## 5. Genres

The kit claims four genres, `platformer`, `top_down`, `shmup` and `grid` (`crates/tungsten-kit/src/foundation/describe.rs:550-554`), and `D-172` admits an item on recipes in two of them. Dropping `grid` and `shmup`, as §7.2 proposes, leaves 24 of the 45 components with fewer than two, and `stay_in_view` with none. Both designs are top-down, so their recipes count once under today's tags. Tagged `rpg` (Lantern Ward) and `shooter` (Static Front), after the genres the owner's 1.0 answer names (the roadmap redo's §2: platformer, top-down, story-driven RPG and shooter), their uses bring 21 of the 24 back to two or more. Two of the 21, `ailment_threshold` and `effects`, are §5.2's to replace (Q17), so the count holds for today's kit.

| Component | Today | After the drop | Lantern Ward uses it for | Static Front uses it for |
| --- | --- | --- | --- | --- |
| `abilities` | top_down, shmup | top_down | Flask, salt, bell, pole (LW14) | Weapons, the dodge, the cores' emitters (SF2, SF6, SF23) |
| `ailment_threshold` | top_down, shmup | top_down | — | The cryo sprayer's ladder to a freeze (SF30) |
| `aim` | top_down, shmup | top_down | Throws at the cursor (LW1) | The right stick (SF1) |
| `conversion` | top_down, shmup | top_down | — | The coil carbine's rounds (SF6) |
| `cooldown` | top_down, shmup, grid | top_down | — | Fuses and spawn markers (SF15, SF22) |
| `counters` | top_down, grid | top_down | Trust (LW7) | — |
| `depth_sort` | top_down, grid | top_down | Every walker and overhang (LW1, LW2) | — |
| `despawn_outside` | top_down, shmup | top_down | — | Rounds (SF7) |
| `effects` | platformer, grid | platformer | Equipment (LW8) | Suppression, the EMP's jam (SF14, SF18) |
| `flags` | platformer, grid | platformer | — | A door's `door.jammed` (SF21) |
| `hit_flash` | top_down, shmup | top_down | Fogborn (LW13) | Units (SF31) |
| `level` | top_down, grid | top_down | — | Perks (SF24) |
| `linear_motion` | platformer, shmup | platformer | Thrown flasks (LW14) | Rounds (SF7) |
| `penetration` | top_down, shmup | top_down | — | The rail's rounds (SF6) |
| `resist` | top_down, shmup | top_down | Fogborn (LW13) | Units' factors by damage type (SF31) |
| `shield` | shmup, top_down | top_down | — | Drones' barriers (SF31) |
| `spawn_children` | platformer, shmup | platformer | — | The bulwark's shield plate (SF20) |
| `spawner` | top_down, shmup, grid | top_down | Fog banks (LW12) | Waves (SF21) |
| `speed_limit` | platformer, shmup | platformer | — | Hounds (SF12) |
| `steering` | top_down, shmup | top_down | Fogborn (LW12) | Drones (SF12) |
| `top_down_mover` | top_down, shmup, grid | top_down | Every walker (LW1) | The breacher (SF1) |
| `platformer_controller` | platformer, shmup | platformer | — | — |
| `stay_in_view` | shmup, grid | — | — | — |
| `wrap` | top_down, shmup | top_down | — | — |

Static Front's entries use 27 of the 30 components tagged `shmup`; the three left are `platformer_controller`, `stay_in_view` and `wrap`, which no walled room needs. Its control rooms cover a shmup's patterns and density (SF23, `sf.core_patterns`), so the tag can go with that evidence. Neither design places or builds on a grid, so `grid` goes with nothing to replace it.

## 6. Questions, each with a default

- **G1. Items bound to one genre.** `platformer_controller` can never have a top-down recipe, so §5.3's rule ("a recipe exists in the other genre") cannot admit it. *Default: §5.3 exempts an item whose job is one genre's (the controllers), with 01 or the template as its user.*
- **G2. `stay_in_view` and `wrap`.** No design, 01 or the template uses them. *Default: they stay, with their recipes, until 7.10's pass decides, since removing kit surface is cheap later too.*
- **G3. New genre tags.** *Default: 7.10 tags `rpg` and `shooter` beside `platformer` and `top_down`, and adds recipe folders for both taken from these designs' entries.*
- **G4. Saves across areas.** §5.5 describes a save with one level reference. *Default: 7.9 designs for several, each area's changes under its own reference, since Lantern Ward needs five and 8.1's loading between areas builds on it.*
- **G5. Rumble.** *Default: none in 1.0; the two-thread rule stands unless a game asks (backlog).*

## 7. Verification

**Mechanical checks,** run on the draft in the session's scratchpad before each round: every `core:`, `kit:`, `umb:` and `comp:` span searched for verbatim in its file (230 distinct spans, none missing); every `none:` pattern run over `api/*.txt` (each 0 hits); the header, decision-ID, link and fragment rules of `just repo-check` applied as if the file sat here (0 errors); and 29 quoted line references matched to their lines.

**Round 1, fresh context.** Two verifiers, one on §1–§2 and one on §3–§6 and the digest, each told to confirm that every span exists and supports the claim it is cited for, to run every search again with other names a real item might have, to check every file and line, and to judge each verdict against its definition.

| Scope | Spans | Searches | File and line | Other findings |
| --- | --- | --- | --- | --- |
| §1–§2 | 96: 87 confirmed, 2 unsupported, 7 weak | 22, each 0 hits; 3 gaps had none | 9, all right | 10 slips in numbers (lamp oil, the walkthrough's clock and fire, 01's enemy count) |
| §3–§6 | 97: 92 confirmed, 1 unsupported, 4 weak | 17, each 0 hits | 24, all right | 2 data-only mechanisms wrong as written, 2 claims the source contradicts, 5 register sums off and 2 double counts, 9 genre uses §3 never stated |

What changed:

- **Mechanisms.** SF15's fuse is a once-counted `cooldown`, since an `expired` cue answers only a projectile's end. SF7's metal and glass are static colliders in their own layers, since every tile is in layer 0. SF8 became data only: a hurtbox stops a round. SF19's pushes reach dynamic bodies that have a hurtbox and health. SF18's bands come from the jam's stacks, not a granted tag. SF22's markers roll on a one-shot `cooldown`, since no cue answers a spawn. SF6's manual reload is missing only as a timed one.
- **Lantern Ward.** Running and the action clips are game Rust (LW28). The pole sweeps all round, since a hitbox cannot follow facing (LW14). Oil drains as an interval ability's cost, and nothing fires at zero (LW15). LW20 is a gap, not blocked: main-thread slices break no rule. LW22 has one `SaveData` type, and 01's `save.rs` is 1,634 lines. LW23 reads stats, LW24 pushes its label every frame, LW26 lacks only bus ducking, and LW27 cites the cancel action, not the pad.
- **Searches** were added to LW2, LW17 and LW18.
- **Registers.** Each entry's lines now count once, `+` marks fixes no step lists yet, and the light kinds moved to Phase 8 (P8-17).
- **Genres.** Each use in §5 now names the entry that states it, and SF29–SF31 were added to state the camera's lead, the chill and the units' bodies.

**Round 2, fresh context,** on what round 1 changed: 30 entries, the changed table rows, §4, §5 and the digest. Of 130 citations (115 distinct), 128 were confirmed and 2 weak; 21 searches each found nothing, and 17 other names tried beside them found no missing part; 30 file and line references were right. Every mechanism it was asked about held in source: the once-counted fuse, a round ending at a hurtbox without health, one hit's effect queued three times making three stacks, a marker's roll on a one-shot cooldown, the ladder's rungs, the run's speed through `move_speed`, the camera's lowest-indexed target, the pole's windowed hitbox. Every verdict fit its definition, and the arithmetic reconciled. It found 19 smaller problems, all applied: five step marks (P7-12, P7-19, P7-20, P8-1, P8-10), the light rows' steps, the parentheses rule, LW20's missing search, three genre cells that cited the game's counters and flags rather than the components, the canister's missing hurtbox, the camera's marker, the bulwark's mechanism, and four numbers. Those last fixes were checked mechanically, not by a third round.

## Session log

- **2026-10-23.** Read §7.2, the API snapshots and the docs they route to. Three read-only research agents covered the asset pipelines, runtime behaviour in source and the backlog and known-issue rows. The coverage matrix came first, then the designs, sheets, registers and genre check, drafted in the scratchpad against a span checker and the repository checker's header, decision, link and quote rules. Two fresh-context verifiers checked every citation and their findings were applied; a third checked what changed. This file was written once, at the end. M81 ran in the same tree and landed as `3f82795` meanwhile; `api/` did not change. No other file was changed.

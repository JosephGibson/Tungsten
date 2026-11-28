# Simulation paper design

A gameplay framework for simulation games, survival and colony, designed on paper by the [roadmap redo](roadmap-redo.md)'s brief ([§7.4](roadmap-redo.md#74-simulation-paper-design-88), step 8.8, `D-316`): characters, players and NPCs alike, whose needs, mood, health on a body plan, exposure, traits and skills are data on one rule set; a world simulation those rules read; NPCs that choose from the same state by utility over smart objects and schedules; time that skips and areas that keep living while unloaded, at the owner's scale. It designs systems, not a game; every name and example here is original. Written against the API snapshots and the kit's source at `de2f6cc9` (0.121).

- **status:** in progress (its steps are done, the triage on 2026-11-28; it stays as the design 8.1's second and third plans, 8.2 and 8.9 build from, deleted when 8.9 closes)
- **goal:** Find what a simulation game needs that the engine and kit lack, and design the framework that fills it, in the kit as opt-in plugins, as register rows the owner triages to 8.1's second and third plans, 8.2 and 8.9's two parts.
- **non-goals:**
  - Code, and edits to any other file but `ROADMAP.md`'s `sim-design` line and, after the triage, the redo's §3 rows that take the rows by ID.
  - Each step's mechanisms: the step's own plan specifies them from the tree of its day.
  - A game or a vertical slice (the owner's answer of 2026-11-27); the studied games' names for things, items, content and art.
  - Multiplayer; a terrain-computed room provider (§8.1 keeps its interface); grids of fluids or gases (a space holds one temperature). Construction and a storyteller were researched only (§2) until the owner's triage admitted them (PS-26, PS-27).
- **files to touch:** This file. `ROADMAP.md`'s `sim-design` line. After the triage, the [roadmap redo](roadmap-redo.md)'s §3 rows for 8.1, 8.2 and 8.9 take §16's rows by ID, as Phase 7's close carried the ARPG design's.
- **ordered steps:**
  1. Read the brief, the kit's stats, statuses, chemistry, overlap, story and core's time, lighting, rooms, saves and workers, and the API snapshots; targeted research on the mechanics (§2). Done.
  2. The design (§3–§11), the page with numbers, weekly tasks and stress numbers (§12–§14), the build sheet and register (§15–§16), the scope by step (§17), trade-offs and questions (§18–§19). Done.
  3. A fresh-context verifier on every citation (§20). Done.
  4. One `/critique` round at xhigh on §5 and §6, applying what holds (§20). Done.
  5. The owner's triage of §16's rows and §19's questions. Done 2026-11-28: every row at its default step, PS-26 and PS-27 admitted to 8.9's second part, every question at its default.
  6. The redo's §3 rows take the triaged rows by ID. Done 2026-11-28.
- **done-when:**
  - Every "data only" and "game Rust" verdict cites an item in `api/*.txt`, and every gap names a search that finds nothing; §20's verifier report confirms or fixes every citation.
  - One critique round on §5 and §6 has run and what held is applied (§20).
  - The owner has triaged every `PS-` row and answered or accepted §19's questions.

## Context digest

- Step 8.8 of the roadmap redo, run on 2026-11-27 and 2026-11-28 on 0.121, after `story-systems` closed (`D-315`) and before `items-and-shops` runs. The owner's answers of 2026-11-27 are the redo's §2; the decision is `D-316`.
- The kit already has most of a character's parts: stats with scoped lines (`foundation/lines.rs`), statuses with intensity, bands, scale curves, fuel and end outputs (`damage/effects.rs`), chemistry rules where the most specific filter wins, conditions, an interactor an AI can press. What it lacks is time: a stat never moves by itself, a status only wears and only per game second, nothing skips, and every read searches a stat by name.
- The design adds one time model for every simulated value (§5): a value moves by a closed form between events, so a live step, a speed, a skip and an unloaded area's catch-up are the same arithmetic, bit-identical where their inputs are. On it sit needs, exposure, wounds at sites, spaces and the world's weather (§4, §8). NPCs choose by utility over smart objects with reservations (§7), which settles 8.2's P8-10. Three tiers carry the scale (§6): near characters with the full kit, far ones without bodies, unloaded ones as table rows.
- The build sheet holds 42 systems: 3 data only, 6 game Rust (about 600 lines), 33 gaps (about 9,030 lines where a game could write them today). The register holds 27 rows: 12 into 8.9's first part, 1 into 8.1's second plan, 5 into 8.2, 2 into 8.1's third plan and 7 into 8.9's second part, construction and a storyteller among them (the owner's triage of 2026-11-28).
- Largest finds: nothing moves a stat over time or thresholds one (PS-1, PS-2); a status cannot rise over time, has no site, and nothing on its holder scales its rate, duration or strength, a chemistry build's `per` being the one holder stat a status reads (PS-4–PS-6); at 60 Hz the fixed-step cap drops steps past 2× and nothing skips (PS-7, PS-8); the zone sends entries only and nothing models a room (PS-10); no AI structure (PS-14–PS-16).

## 1. Assumptions

Provisional, as the brief says; the step that builds on each rechecks it.

- **8.1's second plan** lands item instances with `use` lists run through `run_list` and worn items applying an equipment status of their lines (its plan's §2.3 and §2.4, at `a02e34db`). Food, water and medicine are items whose `use` applies statuses or restores; clothing's insulation is a worn item's line.
- **8.1's third plan** builds its world clock's phase events and schedules (P8-6) on this design's sim clock (§5.1) and loads areas as spawning and persistence on 7.9's saves (P8-7), with the hooks §6.6 names.
- **8.2's navigation** (P8-8) gives flow fields a bodiless mover can sample, and its AI structure (P8-10) is §7's.
- **7.9's saves of many areas** (`D-230`): an unloaded area is kept as its patches and spawns, applied by `enter_area` when the game spawns it.
- **7.12's level format**: a room's places carry a rect or point and its shapes a polygon, and a spawned room names its places (`core: tungsten_core::SpawnedRoom::place`).
- **`D-311`'s map:** a fork over an owned input, bit-identical at every worker count.
- **The kit's statuses** (`D-240`, its band looks from `D-242`) and **chemistry** (`D-241`): intensity, bands with tags and looks, scale curves over intensity, `fuel`, `status_effect`; `on_end`, a band's `enter`, the queue and its most-specific rule.

## 2. Research

Targeted, mechanics only. The colony game's rules come from its public wiki ([needs](https://rimworldwiki.com/wiki/Needs), [mood](https://rimworldwiki.com/wiki/Mood), [infection](https://rimworldwiki.com/wiki/Infection), [hypothermia](https://rimworldwiki.com/wiki/Hypothermia), [rooms](https://rimworldwiki.com/wiki/Room), [work](https://rimworldwiki.com/wiki/Work), [time](https://rimworldwiki.com/wiki/Time), [schedule](https://rimworldwiki.com/wiki/Schedule)). The survival game's wiki refused automated fetches (HTTP 403), so its rules come from search summaries of its pages ([health](https://pzwiki.net/wiki/Health), [wetness](https://pzwiki.net/wiki/Wet), [game time](https://pzwiki.net/wiki/Game_time)), a community guide (pzfans.com) and the developers' blog ([NPC states](https://projectzomboid.com/blog/news/2022/03/the-zuckerverse/)); its figures are second-hand, so this design takes its mechanisms and none of its numbers. Utility AI from Game AI Pro's chapters on the Infinite Axis Utility System and on utility theory, the smart objects of a life-simulation game ([Programming Objects in The Sims](https://qrg.northwestern.edu/papers/Files/Programming_Objects_in_The_Sims.pdf)) and a game engine's smart-object claims ([Unreal Engine smart objects](https://dev.epicgames.com/documentation/unreal-engine/smart-objects-in-unreal-engine---overview)).

| Mechanic | Survival game | Colony game | Here |
| --- | --- | --- | --- |
| Needs | Several rising needs, each with four levels and stat penalties; hunger apart from a calorie pool | Falling bars (food, rest, recreation, outdoors); fall rates by band; levels with mood costs; seeking food below 30 % | Falling levels in `[0, 1]`, rates as stats, bands with lines and outputs (§4.1, §4.2); levels so `restore` serves eating |
| Mood | No sum: each need's level acts alone | A target summed from thoughts; the bar moves toward it at a capped rate, up faster than down; breaks at 35, 20 and 5 % | A summed target and a capped `toward` (§4.1); breaks as bands driving choices (§7.2) |
| Thoughts | — | Memories that expire, with stack limits; situational ones while a condition holds | Memories are timed statuses (data today); situational ones are `while` lines on band and space tags (§4.2, §8.4) |
| Body | Wounds per body part (scratch, laceration, deep wound, bite, fracture, burn), bleeding, bandages that get dirty, local infection | Parts with hit chances; injuries per part; capacities from parts; tending stops bleeding; infection races immunity, whichever reaches 1 first; a delay before the infection roll | Sites from a body plan (§4.5); sited statuses; capacities derived from sites; holder-side factors for treatment (§4.4); a race of two rising statuses; a threshold drawn once (§10.1) |
| Exposure | Core temperature toward a target by heat balance; insulation, wind and water resistance per garment; wetness lowers insulation | A comfortable range shifted by insulation; hypothermia as a rising severity past it | Body temperature approaching a target derived from felt temperature; felt temperature from the holder's space, insulation and wind; wetness bands cut insulation (§10.2) |
| Traits | Rate traits (hunger, thirst, fatigue), aversions (indoors, outdoors) | Rate, mood, break-threshold and learning traits; a crowding trait | Traits are statuses of lines and tags; aversions are lines on weight and mood in a space's context (§10.3) |
| Skills | Levels by XP with multipliers; no decay found | XP by learning factor and passion; daily soft cap; decay above level 10 | XP as stats, levels derived by a curve, decay as a rate above a band (SM12) |
| World | Daily minimum and maximum temperatures; front-driven weather; snow below 0 °C | Seasons of 15 days; weather kinds; wind 0–150 %; light bands | A calendar, seasons, a seeded weather timeline in periods, wind, hourly outdoor temperature, light (§4.7) |
| Time | Speeds, sleep fast-forward (×40 by default); an abstract layer for distant hordes and NPC groups that turns real near the player | 60 ticks a second, speeds ×1, ×3, ×6; slower ticks off camera; no skip | Speeds on the fixed step; skips and catch-up in closed form; three tiers, promotion at a distance (§5, §6) |
| Choice | — | Ordered work priorities 1–4 and a think tree; the first valid job wins; reserve before the job; hauling to stockpiles by priority | Utility scoring with priorities as weights (§7.2); reservations with capacity (§7.1); hauling as an advert (§7.5) |
| Scoring | — | — | Considerations normalized to `[0, 1]`, remapped by curves, multiplied with a compensation factor; a zero disqualifies; inertia for the current choice (Game AI Pro) |
| Smart objects | — | — | Objects advertise what they give, scaled by personality and falling with distance (the life-simulation paper); claim, use, release, released on abort (the engine's smart objects) |
| Rooms | Rooms as building metadata; an aversion counting a room's tiles | Enclosed regions; space, beauty, impressiveness; temperature per room, exchanged through doors and vents; cramped and confined thoughts | Authored spaces with tags and stats behind a provider (§8); temperature per space toward its parent's plus contributions; occupancy bands |
| Colony extras | — | Construction by skill; a storyteller pacing threats by wealth with on and off phases | Researched, then admitted by the triage (PS-26, PS-27) |

The six choices the two games force: one summed mood or need-by-need effects (both: a summed `mood` and need bands that act alone); bars falling or needs rising (falling); per-part hit points or wounds as statuses (statuses at sites); a comfort range or a core temperature (a core temperature with bands, whose target curve gives the comfort range); ticks slowed off camera or an abstract layer (both, as tiers); ordered priorities or scores (scores, with priorities as weights large enough to order them, T7).

## 3. Architecture

### 3.1 Layers

```text
core        Time (unchanged), rooms and places, saves, workers           (unchanged)
kit
  foundation   Stat dynamics (rate, toward, approach); stat bands; derived stats; interned
               stat IDs; the span engine and its event queue; explain of a rate
  damage       Sim statuses: signed rates, sites, holder-side factors, band lines and exits,
               drawn thresholds
  sim (new)    The sim clock, calendar, speeds and skips; world stats and the weather
               timeline; spaces and their provider; body plans; population rows and tiers;
               hauling
  ai (8.2)     Smart objects and reservations; utility scoring; behaviours; explain of a choice
  items (8.1)  Uses aimed at a target and a site
  story (8.1)  Schedules on the sim clock; area hooks
umbrella    explain rate | choice | skip, on 7.2's describe and run --script
```

One stat resolution, one status model and one chemistry serve every genre. A platformer that declares no dynamics, bands or sites pays a branch per stat at load and nothing per step (T12).

### 3.2 Plugins

| Plugin | Domain | Opt-in | Adds |
| --- | --- | --- | --- |
| `StatsPlugin` (extended) | foundation | as today | Dynamics and bands on declared stats, derived stats, interned IDs; nothing runs for a world without them |
| `EffectsPlugin` (extended) | damage | as today | Sim statuses with rates, sites and factors |
| `SimPlugin` | sim | yes | The sim clock, the span engine's pass, world stats, the weather timeline, speeds and skips |
| `SpacesPlugin` | sim | yes | Spaces, the index and its providers, membership, area statuses |
| `BodyPlugin` | sim | yes | The `body_plans` kind, sites in hits and chemistry entries, capacities |
| `PopulationPlugin` | sim | yes | Rows, tiers, promotion and demotion, catch-up at an area's load |
| `MindPlugin`, `SmartObjectsPlugin` | ai | yes (8.2) | Offers, reservations, scoring, behaviours |
| `HaulingPlugin` | sim | yes | Stockpiles and haul adverts |
| `ConstructionPlugin`, `StorytellerPlugin` | sim | yes | Blueprints, frames and builds; events paced by wealth and time (PS-26, PS-27) |

### 3.3 Where each piece runs

The kit's phases (`crates/tungsten-kit/src/foundation/phases.rs:47-55`): `Tick`, `Intent`, `Move`, `Overlap`, `Resolve`, `React`, then `Present`.

| Phase | Simulation work |
| --- | --- |
| `Tick` | The sim clock advances by the step; the span engine processes the events due by the step's world time, world first, then spaces, then characters (§5.3); `stats_apply` writes derived stats into movers |
| `Intent` | Choices due this step (§7.2) read values at the step's world time, reserve and start behaviours; behaviours write `Intent` and press the interactor's slot |
| `Move` | Near movers on bodies; far movers on flow fields, bodiless |
| `Overlap` | Membership: a character whose cell changed looks up its space; enters and exits queue at the step's world time |
| `Resolve` | The damage pipeline with a hit's site; chemistry builds sited statuses at the entry's site |
| `React` | Cues for band and space events; behaviours' steps complete; reservations released |
| `Present` | Near characters only; UI reads a value at the frame's world time through its closed form |

### 3.4 Owners of a value

A simulated value has one owner, so live play, skips and catch-up cannot disagree.

| Value | Owner | Others see |
| --- | --- | --- |
| A sim stat (a need, body temperature, wetness, blood) | The character's `SimState` (§4.10): value and time at its last event, in `f64` | `Stats` holds a mirror, written at each event, for the kit's readers; a write to the base (a `restore`, a use) becomes a delta event at the world time |
| A sim status's intensity | The instance: intensity and time at its last event | Its lines, rewritten at events only (§5.2) |
| A space's stats | The space entity's `SimState` | Occupants read it at their own events |
| World stats (outdoor temperature, rain, wind, light) | The sim clock's timeline (§4.7) | Every area's outdoors space |
| An unloaded character | Its population row (§6.2) | Nothing until its area loads |

### 3.5 Extension points

What the vocabulary cannot say calls a game's named Rust handler, as chemistry does: a band's or an advert's `handler: "name"` with parameters, registered by `SimPlugin::with_handler` or `MindPlugin::with_handler`, called at a fixed hook (a band entered or left, a use started or finished, a choice made, a row promoted). A behaviour may name a game's `StateMachine` state instead of a sequence. The load refuses an unregistered name. A handler runs on the main thread at its event's world time and may change only its holder, its holder's site and reservations; anything wider it queues for the step's `React`.

## 4. Data models

### 4.1 Stat dynamics

The `stats` kind declares a game stat's clamp today (`comp: shape stat_bounds: A game stat's clamp.`). It gains one optional dynamic, rates per world hour:

```json
{
  "food":      { "min": 0, "max": 1, "rate": "food_rate", "urgency": "need_urgency" },
  "food_rate": { "min": -10, "max": 10 },
  "mood":      { "min": 0, "max": 1, "toward": { "target": "mood_target", "up": 0.12, "down": 0.08 } },
  "body_temp": { "min": 25, "max": 45, "approach": { "target": "body_target", "rate": "thermal_rate" } }
}
```

- **`rate`:** names a stat whose value, read in the holder's context, moves the base by that much per world hour, clamped by `min` and `max`. A rate stat is an ordinary stat: traits, statuses, spaces and bands put lines on it (`food_rate` pct +0.5 for a large appetite; pct −0.5 `while` asleep).
- **`toward`:** moves the base linearly toward the target stat's value at `up` or `down` per hour (each a number or a stat), holding once there; the colony game's mood bar.
- **`approach`:** moves the base exponentially toward the target: `x(t) = T + (x₀ − T)·e^(−k·t)`, `k` the rate stat's value; for temperatures.
- **`urgency`:** a curve from the level to a need's urgency in `[0, 1]`, which choices read (§7.2).
- A stat with no dynamic is today's stat. A dynamic stat's base is written only through deltas at a world time (§3.4).

Rates and targets are read as their lines' sums in the holder's context (`kit: tungsten_kit::Stats::sums`, with the holder's tags through `kit: tungsten_kit::Read<'a>::holder`), so scoped lines reach them: a scoped line reaches only the reads that take a context (`crates/tungsten-kit/src/foundation/lines.rs:15-18`). The sums are taken over a zero base, since `Sums` starts from the stored base (`crates/tungsten-kit/src/foundation/stats.rs:699-701`), and apply in `f64` to the base the sim owns (§5.2), never to the `Stats` mirror. A rate or target stat may be static or derived, never dynamic (the load refuses it), so a span's rate is constant.

### 4.2 Stat bands

A band on a dynamic or derived stat, as a status's band is on its intensity:

```json
"food": { "bands": [
  { "below": 0.3,   "gap": 0.02, "grant": ["need.food.hungry"], "interrupt": "choose",
    "lines": [{ "stat": "food_rate", "modifier": { "pct": -0.5 } }] },
  { "below": 0.125, "gap": 0.02, "grant": ["need.food.ravenous"] },
  { "below": 0.001, "grant": ["need.food.empty"], "enter": [{ "apply": { "status": "malnutrition" } }],
    "exit": [{ "remove": "malnutrition" }] }
] }
```

- **`below` or `above`:** the level it enters at; `gap` the hysteresis, so it leaves at `below + gap` (or `above − gap`).
- **`grant`:** tags while it holds; **`lines`:** lines on the holder's stats while it is the highest band its stat holds (a status's band look follows the same rule, `comp: field look: shape look = null — what the holder shows in place of the status's look while this band is the highest its intensity holds`).
- **`enter` and `exit`:** sim-safe outputs (§5.3) at the crossing's world time.
- **`interrupt`:** `choose` (the holder chooses again, §7.2) or `wake` (a skip watching the holder stops, §5.6).
- At most 8 bands a stat, ascending; the load refuses overlapping gaps.

### 4.3 Derived stats

A stat whose base is computed, not stored:

```json
"body_target": { "from": { "stat": "felt_temp", "curve": "thermoregulation" } },
"felt_temp":   { "from": { "space": "temperature" } },
"moving":      { "from": { "capacity": "moving" } },
"move_speed":  { "lines": [{ "modifier": { "pct": { "from": "moving", "curve": "moving_speed" } } }] }
```

- **`from.stat` with `curve`:** another of the holder's stats through a curve; **`from.space`:** the holder's space's stat (§8); **`from.world`:** a world stat; **`from.capacity`:** a body plan capacity (§4.5).
- **A line's amount** may be `{ "from": stat, "curve": id }`: the modifier's value read from another stat through a curve. This is the ARPG design's `per` line generalized (its PA-9, 8.7), so 8.7 builds `per` on it.
- A derived value is computed at each event of its holder and held between (§5.2). A dynamic stat whose inputs read a derived stat over a continuous source is **coupled** and takes the resample grid (§5.3).

### 4.4 Sim statuses

A status takes three new fields beside today's (`comp: field intensity: shape effect_intensity = null — makes it a status`):

```json
"exposure": {
  "sited": true,
  "intensity": { "max": 1, "rate": 0.08 },
  "bands": [{ "at": { "min": 0.5, "max": 1.0 }, "enter": [
    { "build": { "status": "infection", "amount": 0.05 } },
    { "build": { "status": "immunity", "amount": 0.01 } },
    { "remove": "exposure" } ] }]
}
```

- **`intensity.rate`:** a signed rate per world hour, beside today's `decay` per game second; a status with a `rate` is a **sim status**, evaluated by spans (§5.2), and its `decay` is refused. Today's statuses keep their per-step decay (`crates/tungsten-kit/src/damage/effects.rs:4487-4495`), so no combat status changes (PS-Q4).
- **A sim status's lines** follow its intensity at events only (bands, the resample grid), not each step.
- **`at` drawn:** a band's `at` may be `{ "min", "max" }`, drawn once per instance from the kit's stream at its application and saved with it; a chance that is closed-form (§10.1).
- **Band `lines` and `exit`,** as §4.2.
- **Holder-side factors.** Four well-known stats, 1 when absent, read in the instance's context (its status ID as `status.<id>`, its granted tags, its site's tags) with the holder's tags:
  - `status_build` multiplies an application's amount;
  - `status_rate` multiplies a sim status's rate and a combat status's decay;
  - `status_duration` multiplies a timed status's duration at its application;
  - `status_strength` multiplies its modifiers and payload, as a request's scale does.

  A bandage is `status_strength` mul 0.1 `with` `status.bleeding` at its site; a hardy trait is `status_rate` pct −0.25 `with` `status.infection`. EV-21 in Later (a holder-side factor on a duration) becomes one line.

### 4.5 Body plans and sites

```json
"body_plans": { "humanoid": { "sites": {
  "head":      { "cover": 0.08, "tags": ["site.head"], "capacity": { "consciousness": 0.6 } },
  "torso":     { "cover": 0.32, "capacity": { "consciousness": 0.4 } },
  "arm.left":  { "cover": 0.1,  "capacity": { "manipulation": 0.5 } },
  "leg.left":  { "cover": 0.11, "capacity": { "moving": 0.25 } },
  "foot.left": { "cover": 0.03, "capacity": { "moving": 0.25 } }
} } }
```

- A **site** is a key, not an entity. A `body` component names the holder's plan. A sited status's instances are keyed by status and site: two feet, two instances.
- A **hit's site** comes from its hitbox's `site` filter, or is drawn from the plan's `cover` weights by the kit's stream at the pipeline's sorted order; it rides the request to chemistry's entry, and `build` and `apply` put sited statuses at the entry's site.
- A **sited instance's lines** reach reads at its site and reads with no site, never reads at another site. Its `impair` (0 to 1, scaled with its modifiers) lowers its site's efficiency.
- A **capacity** is `1 − Σ weight × min(1, Σ impair at the site)` over the plan's sites, a derived stat (§4.3).
- A **use** may name a site: 8.1's `use_item` aimed at a target and a site (PS-13).

### 4.6 Spaces

A space is an entity, with tags, stats and statuses like any other (§8):

```json
"space": {
  "shape": { "place": "tavern_hall" },
  "tags": ["space.indoor", "space.enclosed"],
  "stats": { "temperature": 12, "exchange": 0.4, "capacity": 20 },
  "occupant_statuses": ["in_enclosed"]
}
```

- `shape` is a room's place (`core: tungsten_core::assets::room::RoomPlace::rect`) or shape (`core: tungsten_core::assets::room::RoomShape::polygon`) by ID, or a rect in the space's own data.
- `temperature` is a dynamic stat (`approach` toward `temperature_target`, at the `exchange` rate); `temperature_target`'s base is the parent space's temperature, and contributions add to it (§8.3).
- `occupants` is a stat the index keeps; bands on it grant `space.crowded`.
- `occupant_statuses` are applied to a character on entering and ended on leaving (area statuses).

### 4.7 World: calendar and weather

```json
"calendar": { "ratio": 60, "hours_per_day": 24, "start": { "season": 0, "day": 1, "hour": 8 },
  "seasons": [ { "name": "thaw", "days": 15, "mean": 11, "swing": 5, "daylight": [6, 20], "weather": "thaw_weather" } ] },
"weather": { "thaw_weather": { "period": 6, "states": {
  "clear":    { "lines": [], "next": { "clear": 6, "overcast": 3, "rain": 1 } },
  "rain":     { "lines": [{ "stat": "rain", "modifier": { "add": 0.8 } }, { "stat": "temperature", "modifier": { "add": -2 } }],
                "next": { "rain": 3, "overcast": 4, "clear": 2 } }
} } }
```

- **`ratio`:** world seconds per game second; 60 makes a world hour a real minute at 1× (PS-Q3).
- **The timeline:** each weather period's state is drawn from the season's table given the period before, from a stream keyed by the world seed and the period index, and kept, a byte a period; a skip or a catch-up reads the kept history, so every reader sees one weather.
- **Outdoor temperature** is a step function of the world hour: the season's mean, the day's swing at the hour, the period's weather lines; it changes at hourly world events only.
- **Light:** daylight hours by season set the core ambient light (`core: tungsten_core::lighting::AmbientLight`) for drawing and a `light` world stat for the rules; a point's light stays `core: tungsten_core::lighting::light_level`.
- **Wind:** a direction and a speed per period, world stats; outdoors, wind chill lowers felt temperature through a curve.
- **World tags:** the hour's part (`time.day`, `time.night`), the season (`season.<name>`) and the weather (`weather.<state>`) join every sim read's context, so a line `with` `time.night` applies at night.

### 4.8 Smart objects (8.2)

A `smart_object` component on an interactable, a space or a ground item holds its **adverts**, the owner's offers. The kit's `offers` kind and `offer` shape, with its `gives`, already name a level-up's draws (`comp: shape offers: What a level up offers`, `comp: shape offer_gives: What a chosen offer gives.`), so these take other names; 8.2's plan settles them.

```json
"smart_object": { "adverts": [{
  "id": "warm_by_fire", "tags": ["advert.warmth", "activity.leisure"],
  "satisfies": { "warmth": 0.6, "social": 0.3 }, "behaviour": "sit_by_fire",
  "capacity": 6, "hours": 0.5, "when": { "tags": { "all": ["fire.lit"] } }, "spot": [0, 24]
}] }
```

- `satisfies` are need amounts, scaled by the holder's `satisfaction` stat read in the advert's context with the need's tag (`with` `need.food`: personality, as the life-simulation game scales its advertisements).
- `capacity` counts reservations: a choice reserves one before its behaviour starts and releases it at the end or on an abort.
- `behaviour` names a sequence (§7.3) or a game's state machine state; `hours` is the use's length; `when` is a `Condition` on the object.
- A use's effect on needs is the behaviour's own status (a `using` status per advert, with lines on rate stats), so eating at a table and sleeping in a bed are lines, and a skip computes them in closed form.

### 4.9 Minds and behaviours (8.2)

```json
"minds": { "villager": {
  "considerations": [ { "input": "benefit" }, { "input": "distance", "curve": "dist_falloff" },
                      { "input": "schedule" }, { "input": "available" } ],
  "recheck": { "near": 1, "far": 4 }, "inertia": 0.1, "search": 400
} },
"behaviours": { "sit_by_fire": { "steps": [
  { "go": "spot" }, { "use": {} }, { "wait": "hours" }, { "release": {} } ] } }
```

- A `mind` component names its mind and a schedule (8.1's P8-6).
- Steps: `go` (a spot, a place, a space), `use` (the advert: the interactor's press for a near holder, the use direct for a far one), `wait`, `pick_up`, `drop`, `carry`, `say`, `clip`, `release`. A step that fails aborts the behaviour and releases its reservations.

### 4.10 `SimState` and rows

- **`SimState`,** a component on every simulated entity (characters, spaces): per dynamic stat its span (value and world time at its anchor, the held rate or target and rate), its band memberships (a band's state is not a function of the value inside its gap) and its next event; per sim status the same; its pending input changes; its stream's draw counter. Its layout is the stat set its kind declares, by interned ID (PS-11), so an event reads no names. Saved whole (§9.4).
- **A population row** (§6.2): an unloaded character's ID, kind, area, home and current place, activity and its end, schedule; its spans as `SimState` holds them, band memberships and draw counter included; its sim statuses compacted (ID, site, span, bands, draws); its traits by ID. A row holds everything the next event needs, so demotion and promotion lose nothing.

## 5. The time model

### 5.1 Clocks

- **Real time** and **game time** stay `Time`'s. The game clock runs at a scale and its accumulator spends itself in fixed steps, at most `max_steps_per_frame` a frame, two by default, dropping whole steps past that (`crates/tungsten-core/src/time.rs:99`, `:169-183`), while `Time::elapsed` still adds the frame's whole game time (`:154`). At a 60 Hz frame and step, a speed past 2× therefore drops steps; a faster frame raises that bound.
- **World time** is the sim clock's, an integer count of world milliseconds, advanced by `step × ratio` once per fixed step that runs: with a 1/60 s step and a ratio of 60, 1,000 ms a step. It counts steps, never `Time::elapsed`, so a frame that drops steps drops world time too and the world stays in step with what ran (`core: tungsten_core::time::Time::dropped_this_frame`).
- **The calendar** reads world time: hour, day, season, year.

### 5.2 Spans

Every simulated value is a **span**: its value `x₀` at the world time `t₀` of its holder's last event, and inputs fixed then. Between events:

| Dynamic | `x(t)`, `Δ = t − t₀` in world hours | Crossing a level `a` |
| --- | --- | --- |
| `rate r` | `clamp(x₀ + r·Δ)` | `Δ = (a − x₀) / r` |
| `toward T` at `u` up, `d` down | `x₀ ± min(rate·Δ, |T − x₀|)` | linear, then held at `T` |
| `approach T` at `k` | `T + (x₀ − T)·e^(−k·Δ)` | `Δ = −ln((a − T) / (x₀ − T)) / k`, when `a` lies between |
| A sim status's intensity, `rate r` | as `rate`, ending at 0 or held at `max` | as `rate` |

- **Inputs are held** within a span: rates, targets and lines read at `t₀`. A value that reads a value that moves (a body temperature reading its space's temperature) is resampled on the grid (§5.3), so held inputs are a first-order rule with a known step, not an approximation hidden in a frame rate.
- **Clamps are events:** a rate reaching `min` or `max` holds there from the crossing's time.
- **`f64` throughout:** values and the arithmetic of a span; world time as an integer. The `Stats` mirror is written as `f32` and never read back: an input resolves as its lines' sums (`f32` amounts, exact in `f64`) applied to the sim's own `f64` base for a dynamic or derived stat, or to the static base `Stats` holds.

### 5.3 Events and their order

A holder's next event is the earliest of:

1. **a band crossing** of any of its stats or sim statuses, solved exactly at its span's start (the next crossing level is the band's enter or, while it holds, its exit with the gap);
2. **a clamp**;
3. **a status's end:** a sim status at 0, a timed status's duration;
4. **an input change**, stamped with the world time it happened: a status applied or ended, a tag, a space entered or left, an item worn, a delta (a `restore`, a use), a behaviour's status;
5. **a world event:** the hour, a weather period, the day, the season;
6. **the resample grid:** every `L` world minutes (15 by default, PS-Q5) while the holder has a coupled stat, at the holder's own phase (its save ID's hash modulo `L`, in milliseconds), so 5,000 holders' grids spread across `L` instead of falling on one step; §10's holders have phase 0;
7. **a parent's event:** a space's band or value event reaches its occupants as an input change at its time.

Event times round up to the next world millisecond. At an event the holder's spans are evaluated at its time, its band states flip by the crossing test (the value past the level), its outputs run, its inputs are read again and its next event is solved.

- **Order.** Events are processed by (world time, tier: world, then spaces, then characters, then the event's kind, then save order). A world event at `t` reaches every space before any character reads that space at `t`.
- **Dependencies run one way** inside the pass: the world feeds spaces and spaces feed their occupants. No character's sim output reaches a space or the world, since occupancy and contributions come from live systems as stamped input changes, so a holder depends only on the tiers before it. Shared state in abstract mode (stocks, reservations) is the one exception, which §5.6's windows order.
- **Sim-safe outputs.** A band's or a status's sim outputs are `apply`, `build`, `consume` and end of sim statuses at the holder and its site, a delta on a dynamic stat, flags and counters, an interrupt, and a cue event for `React`. They run at the event's world time, so a status a band applies starts its span there. Spawns, blasts, stimuli to others and sounds are refused in sim outputs at load; a cue event carries them to the presentation, which the sim never reads back.
- **Chains.** Outputs that cause a crossing at the same time are processed in the same event, 8 deep at most; past it the rest waits for the next millisecond, traced.

### 5.4 Live play

- **Each fixed step** advances world time to `t_k`, then processes, in order, every event at or before `t_k`. Live systems (movement, combat, chemistry, uses) stamp their input changes with `t_k`; the next step's pass processes them first, at `t_k`, before its own later events.
- **Deferral is exact.** A span is a closed form and an input change carries its time, so processing a holder's due events later gives the same values as processing them at once, provided what it reads is still there. A pass processes at most its budget of holders a step (512 by default), whatever their events, the oldest due first, and leaves the rest pending; any read of a pending holder (a choice, the UI, a save, an output aimed at it, a tier change) processes it first. A weather change across 5,000 characters spreads over ten steps and its values do not change (§6.4).
- **What a pending holder reads stays readable.** The world's past values come from the kept timeline (§4.7). A space keeps its spans back to its oldest pending occupant's due time; a space event that would drop a span a pending occupant still needs processes that occupant to the event's time first; and no holder stays pending past a horizon of 64 steps, when the pass takes it whatever its budget. An occupant therefore reads its space at exactly the times and values it would have without deferral.
- **Within a step** the pass runs the world's events, then each space's (keeping the spans its occupants need), then the characters', each reading the world and its space at its own event times. A character's events need nothing from another character, so that part maps on the workers (§6.5).
- **Reads** of a value between events are its closed form at the read's world time, which the UI and choices use; kit systems read the `Stats` mirror, which changes at events.

### 5.5 Speeds

- A speed `k` is `Time::set_scale(k)` (`core: tungsten_core::time::Time::set_scale`) with the step cap raised to cover it, `ceil(k × step rate / frame rate) + 1` (`core: tungsten_core::time::Time::set_max_steps_per_frame`); `SimClock::set_speed` does both and reports the speed reached.
- Each step's fixed work runs `k` times a frame (bodies, movers, choices on game-time intervals); the span engine's cost follows events per world hour, not steps. Past the CPU's budget the frame drops steps and the world runs slower than asked, never inconsistently (§5.1).
- 1×, 3× and 6× are the reference speeds; §14's rows measure 1× and 3× at 5,000.

### 5.6 Skips

`skip(world, until, watch)` advances the world without steps: a sleep, a rest, a wait.

1. Refused while a transition or capture waits (`screens::idle`'s rule). The game clock stops on a stop of the skip's own, beside the game's pause, the state stack's hold and the editor's stop, so none undoes another, as the editor took its own (`core: tungsten_core::time::Time::set_editing`).
2. Every loaded character enters **abstract mode** (§7.6): its behaviour becomes its activity, a span of its advert's status at its place, positions frozen.
3. The span engine processes every event to `until` by the order of §5.3, with no frame budget, in **windows**. An abstract activity's end rounds up to a 5-minute decision grid, and the choices there, with the reservations and stock draws they make, are the only events that touch shared state. A window runs every holder's private events up to the next decision time on the workers; at the decision time, candidates are scored on the workers against the committed state, then committed serially in save order, a reservation or draw that fails taking the holder's next candidate. No holder advances past a shared interaction before it commits, so nothing computed against a stale stock is kept. Choices run at activity ends and interrupts, never on recheck intervals.
4. A band with `interrupt: "wake"` entered by a watched holder ends the skip at its time.
5. On return, characters whose activity changed are placed at its spot, behaviours restart by choosing, the clock is released, and `Skipped` reports the world time reached and why.

For a holder whose inputs come only from the sim (a sleeping player in a bed), a skip and live play at any speed compute the same events in the same order from the same values, so their stats are bit-identical (§17's 8.9 check). An NPC's differ where its live behaviour differs from its activity span (T3).

### 5.7 Catch-up

An area loaded after time away advances its rows from the world time it was left to now by the same engine in abstract mode and §5.6's windows: its rows' private events run in parallel between decision times, and its choices, reservations and draws on shared stocks (a granary's count, a well's water) commit serially at each, before its characters are promoted (§6.6). The weather comes from the kept timeline (§4.7). Rows of an area never loaded hold no state: they are its generator's output (P7-16's seeds) at the world time they are first needed, with upkeep computed from their schedule's start.

### 5.8 Determinism

- Draws come from streams keyed by what they are for: the world seed and a period index for weather, a holder's seed and its draw counter for a status's threshold and a choice's tie, never from a shared stream in call order. Live, a skip and a catch-up therefore draw the same values whatever order holders are processed in.
- On one build and platform, the same save and inputs give the same events and values, bit for bit, at every worker count (§6.5), as the physics hashes do (run-vs-run). `exp` and `ln` are the platform's; no cross-platform promise (T11).

## 6. The scale model

### 6.1 Tiers

| Tier | Who | Has | Moves | Chooses |
| --- | --- | --- | --- | --- |
| Near | Within the view plus a margin, or a game's watch radius; at most 400 at the reference scale | The full kit: a kinematic body, sprite, animator, interactor, perception | Its mover on its body | Every second of game time, and at ends and interrupts |
| Far | The rest of a loaded area | `Position`, `Tags`, `Stats`, `Effects`, `SimState`, `Mind`; no body, sprite or sensor | A bodiless mover on 8.2's flow fields, passing through others | Every 4 s of game time (below 1 Hz at 1×), at ends and interrupts |
| Unloaded | Areas not loaded | A population row | Its activity's place | At activity ends, in abstract mode, only when its area catches up |

- **Near and far** trade components by distance with hysteresis, at most 32 a step each way (each an archetype move), the camera's sweep queued in order.
- **Far movers need no physics**: spaces track them by position (§8.2), not by sensors, since a bodiless entity meets no sensor.

### 6.2 Storage

- **`SimState`** (§4.10): dense per kind, about 0.75 KB a character for 20 dynamic stats' spans and 5.6 statuses; an event reads and writes it by index.
- **Interned stat IDs** (PS-11): a `StatId` a name gets at load; `Stats` keeps lines by ID; a read by name interns once. Today a read searches a list sorted by name with string compares (`crates/tungsten-kit/src/foundation/stats.rs:208-210`, `:826-829`).
- **The population table:** a saved resource, rows in columns (IDs, area, place, activity and its end, each span, band memberships, statuses, the draw counter, §4.10), only rows of areas that have loaded at least once (§5.7).

### 6.3 Work per frame

At the reference scale (§12): 5,000 characters in one loaded area, 400 near and 4,600 far, 2,500 smart objects with 3,750 adverts, 600 spaces, a world hour a real minute at 1×.

| Work | Count at 1× | Target cost | At 3× |
| --- | --- | --- | --- |
| Span events | 9 a character per world hour: 4 grid, 1.5 bands, 2 behaviour statuses, 1 world hour, 0.5 ends; 45,000 a world hour, 750 a second, 12.5 a frame at 60 Hz | 3 µs each: 0.04 ms | 0.11 ms |
| Choices | 250 a second at behaviour ends (20 world minutes each), 400 near and 1,150 far rechecks, 50 interrupts: 1,850 a second, 31 a frame | 12 µs each (30 candidates × 4 considerations): 0.37 ms | 1.1 ms |
| Far movement | 1,840 walking, a flow-field sample and a step each | 25 ns each: 0.05 ms a step | 0.14 ms |
| Near movement and bodies | 400 kinematic movers and their casts | 0.9 ms a step | 2.7 ms |
| Membership | a cell check per mover per step; about 1 change per character per world hour | 0.02 ms a step | 0.06 ms |
| Tiers | at most 32 promotions each way a step | 0.1 ms at a sweep | 0.1 ms |

About 1.5 ms a frame at 1× and 4.2 ms at 3×, before rendering and the game's own systems; §14's rows measure them and 9.1 sets the budgets. These are averages. A step's span work peaks at its budget, 512 holders at about 3 µs, 1.5 ms, when an hour or a weather period reaches every outdoor character; grids add about 6 events a step at 1×, spread by their phases (§5.3).

### 6.4 Deferral and budgets

- The budget covers every event: a world event reaching every outdoor character leaves each pending at its time (§5.4), and the pass takes at most 512 holders a step, the oldest due first, so a 4,000-character front costs eight steps of 1.5 ms, not one of 12 ms.
- A choice, a UI read, an output aimed at a holder, a tier change and a capture process the holder first, so nothing reads a stale value; at 31 choices a frame these add at most 31 holders to a step.
- A space keeps the spans its pending occupants still need, and the 64-step horizon bounds both the spans kept and the time a holder waits.

### 6.5 Workers

- **The span pass in live play:** within a step, the world's and the spaces' events serially (few), then the due characters' `SimState` gathered into an owned input and mapped on `D-311`'s workers (`core: tungsten_core::workers::map`), each holder's events in order and alone, reading the world's timeline and its space's kept spans. A character's sim outputs touch only itself, so each result is the holder's serial one, and the map keeps index order, so the pass's result is the serial one at every count.
- **Abstract mode** (skips, catch-up) adds the shared events §5.6 names: windows between decision times run private events on the workers; each decision time scores on the workers against the committed state and commits serially in save order. An area's rows parallelize inside the area, so one area's catch-up gains from the workers too.
- **Choices** in live play score on the workers the same way: candidates gathered first, scores pure, reservations applied serially in save order, a lost reservation taking the next candidate.
- A borrowing fork over World columns (Later's) would save the gather; nothing here needs it.

### 6.6 Operations

An abstract activity lasts its advert's `hours` or its schedule block, whichever ends first: a night's sleep is one activity, a work block a few. The reference rate is 10 abstract activities a character a day, against live play's behaviours of about 20 world minutes, which include their walks.

| Operation | Work | Target |
| --- | --- | --- |
| An 8-hour skip at 5,000, from 22:00 | Most characters sleep through it as one activity: about 1.2 choices a character (6,000) and 44 events (5.5 an hour: 4 grid, 1 world hour, 0.5 bands), 220,000 events; rows untouched. A daytime skip makes about 24,000 choices | 0.73 s serial; about 0.2 s on six participants in 96 windows, behind a fade |
| An area's catch-up, 500 rows, 72 hours | 10 activities a row a day (15,000 choices) and 6 events a row an hour (216,000 events), then 500 promotions | 0.83 s serial; about 0.25 s on six in 864 windows, inside 8.1's sliced load (P8-7) |
| A save at 5,000 and 20,000 touched rows | every pending input change processed at its own time, then captured (`core: tungsten_core::save::capture_world`): spans as they stand (§9.4) | about 6.5 MB of characters and 4 MB of rows; the write on `D-303`'s writer thread |
| An area unload | its characters demoted to rows at the world time, live state dropped (bodies, behaviours, reservations) | under 10 ms for 500 |

### 6.7 Memory and saves

- 5,000 characters: `SimState` 3.75 MB; `Stats`, `Effects` and `Tags` about 5 MB.
- 20,000 touched rows at about 200 bytes in memory (spans, bands and statuses for a row's reduced stat set): 4 MB; 100,000 if every area has loaded: 20 MB.
- A save writes a character as today's components (about 1.3 KB of JSON) and rows as columns at full precision, the shortest text that reads back bit for bit (about 200 bytes a row), so a save and a load change no value (PS-Q8). Rows of never-loaded areas are never written (§5.7).

## 7. AI

### 7.1 Smart objects and reservations

- Offers (§4.8) sit on interactables (a bed, a stove, a well), on spaces (a tavern's `socialize`) and on ground items (a haul, §7.5).
- **Reservations:** `reserve(world, object, advert, holder)` takes one of the advert's capacity or refuses; a reservation ends with its behaviour, its holder's abort or death, the object's despawn, or the advert's `when` failing. A player's use reserves too, so no NPC takes the seat the player sits in. Reservations are unsaved: a load re-chooses.
- **Gathering:** each space keeps its adverts; a choice gathers its own space's and its neighbours' within `search` px (near) or by space (far).

### 7.2 Scoring

For each gathered advert whose `when` holds and whose capacity is free:

- **benefit** = `min(1, Σ satisfies[n] × urgency[n])`, a need's urgency its level through its curve;
- each consideration `c` in `[0, 1]` through its curve, compensated for their count `n` as `c + (1 − c)·(1 − 1/n)·c` (the Infinite Axis Utility System's factor), then multiplied; a zero disqualifies, cheap considerations first;
- **weight**: the holder's `weight` stat read in the candidate's context (the advert's tags, the object's, the destination space's) with the holder's tags; a trait, a priority and a preference are lines on it (`with` `space.enclosed`, mul 0.4);
- **score** = weight × product; the current behaviour's advert gains `inertia`; ties draw from the holder's stream.

The highest wins, reserves and starts its behaviour. A choice runs at a behaviour's end, at an interrupt band and at its tier's recheck; a recheck switches only for a score above the current one's plus inertia.

### 7.3 Behaviours

- A sequence (§4.9) runs one step at a time on the fixed step; a near holder's `go` sets its steering target or 8.2's path, a far holder's samples the flow field. `use` presses the interactor's slot for a near holder (an AI can press it, `crates/tungsten-kit/src/overlap/interact.rs:39-45`) and calls the use for a far one, so the player's press and an NPC's use reach the same advert.
- While a behaviour uses an advert, its holder carries the advert's `using` status: lines on rates and tags (`activity.eating`), so a need's restore is a rate the spans compute, live and skipped alike.
- A game's `StateMachine` may run a behaviour instead (`kit: tungsten_kit::StateMachinePlugin<S>::on_update`), reporting its end through `behaviour_done`.

### 7.4 Schedules

8.1's schedules (P8-6) are hour blocks naming activities (`sleep`, `work`, `leisure`, `any`). For a loaded holder a block is the `schedule` consideration: 1 for an advert whose activity tag matches, the block's own factor otherwise. For an unloaded holder in abstract mode the block names the activity directly, and its advert is the place's (P8-6's placing while unloaded).

### 7.5 Priorities and hauling

- **Priorities** are lines on `weight` with a job tag: the colony screen's 1–4 write `mul` 8, 4, 2 and 1 (`with` `job.haul`), "never" `mul` 0. Large ratios order jobs nearly strictly, as the colony game's priorities do, while a need's urgency can still win (T7).
- **Hauling:** a ground item outside a stockpile space adverts `haul` (`job.haul`, benefit 1 under a `work` block); its behaviour picks the item up (8.1's inventory or the kit's carry, `kit: tungsten_kit::carry`), goes to the nearest stockpile cell its filter accepts, reserved, and drops it. The item and the cell are both reserved; an abort drops the item where its holder stands.

### 7.6 Abstract mode

In a skip and an unloaded area's catch-up, a character's behaviour is its activity: its advert's `using` status at its place for the advert's `hours` or the schedule block, whichever ends first, rounded up to the 5-minute decision grid, then the next choice. It neither moves nor meets anything. Shared stocks count down and reservations commit at decision times, serially in save order (§5.6); reservations hold per advert as live. Promotion places each character at its activity's spot.

## 8. Spaces

### 8.1 Authored spaces and the provider

- A room's objects carry `space` components (§4.6) whose shapes are its places and shapes; the root of each area is its outdoors space, which carries the world stats.
- A **provider** fills the space index for an area: `SpaceProvider::spaces(world, area) -> Vec<SpaceShape>`. The authored provider reads `space` components; a later one computes rooms from runtime terrain (regions a room's collision layer and its doors enclose, `core: tungsten_core::assets::room::RoomLayerKind::Collision`) into the same shapes. Nothing reads a provider directly: readers use the index.

### 8.2 `space_at` and membership

- `space_at(world, point) -> Option<Entity>`: the innermost space whose shape holds the point, from a cell grid per area; nested spaces win by depth, then by smaller area, then by ID.
- **Membership** (`InSpace`): a character whose cell changed in `Overlap` looks it up; a change queues its exit and entry at the step's world time, applying and ending `occupant_statuses` and moving `occupants`. The kit's zone sends entries only (`crates/tungsten-kit/src/overlap/zone.rs:132-139`) and needs a body; membership needs neither.
- `SpaceEntered` and `SpaceExited` are cue events.

### 8.3 Space stats and contributions

- A space's `temperature` approaches `temperature_target` at `exchange`; the target's base is its parent's temperature; outdoors, the world's.
- **Contributions:** an object's `contributes` lines go on the space that holds it, under its source key, while their `while` holds: a lit stove `temperature_target` add 14 `while` `fire.lit`; an open door `exchange` add 0.5. The stove's fuel is its own dynamic stat, its band at 0 ending `fire.lit` at a closed-form time (§10.4).
- Occupants read the space's values at their own events: a space's crossing reaches them as an input change (§5.3, item 7); its continuous temperature reaches them by the grid.

### 8.4 Area statuses and context

- `occupant_statuses` grant tags (`in.enclosed`, `in.outdoors`, `in.dark`), so mood lines are `while` lines on them, data only.
- A choice's context includes the destination space's tags, so a preference for a space is a weight line (§7.2).

## 9. Order of operations

### 9.1 A fixed step

1. `Tick`: the sim clock adds the step's world milliseconds; the hour's, period's and day's world events queue; the span pass processes everything due by the step's world time in §5.3's order, within its deferral budget; `stats_apply` writes derived stats into movers.
2. `Intent`: due choices score and reserve; behaviours advance and write `Intent`.
3. `Move`: near movers before `physics_step`; far movers.
4. `Overlap`: membership changes queue at the step's world time.
5. `Resolve`: hits with sites; chemistry builds sited statuses (input changes at the step's time).
6. `React`: cue events from bands and spaces; behaviours' ends release reservations and queue choices.

### 9.2 A skip

§5.6's five steps; the span engine runs with no budget, in windows that end at each decision time, private events on the workers and choices committed serially; the skip returns `Skipped { reached, reason }`.

### 9.3 An area unload and load

1. **Unload** (8.1's P8-7 hook, PS-20): each character processed to the world time, demoted to a row, despawned with its live state; what is no row is kept as P8-7 keeps an unloaded area, on `D-230`'s saves of many areas (no core call keeps an area unloaded mid-run today).
2. **Load:** the area spawns and `enter_area` applies its kept state (`core: tungsten_core::save::enter_area`); the catch-up (§5.7) advances its rows to now; rows promote to far characters at their activity's spot, then near by distance.

### 9.4 A save

Every pending input change is processed at its own world time, so none is lost, and nothing else advances. Each span is saved as it stands: its anchor's value and time, its held inputs, its band memberships and its next event, with the draw counters and the population table's rows; then `capture_world`. A load restores every span exactly and reads no input again, so it adds no sample and the next event is the one the unbroken run would have reached; inputs are read again at that event, as they would have been. A save and a load therefore change no value.

## 10. Worked examples

Numbers from a reference span engine in the session's scratchpad, which implements §5.2 and §5.3. Times are world times; rates are per world hour.

### 10.1 A foot laceration, bandaged and infected

Pell, the player, steps on broken glass at 14:00: a hazard hitbox with `site: "foot.left"` and 12 cut damage. Chemistry's rule for `cut` on `body.flesh` builds three sited statuses at the entry's site: `laceration` 0.6, `bleeding` 0.6 and `exposure` 0.02; `exposure`'s band draws its `at` from `{0.5, 1.0}` at the application: 0.62.

| World time | Event | Values |
| --- | --- | --- |
| 14:00 | Hit at `foot.left` | `laceration` 0.6 (rate −0.04: heals by 05:00), `bleeding` 0.6 (rate −0.2: clots by 17:00), `exposure` 0.02 (rate +0.08); `blood` rate +0.02 − 0.5 × bleeding, held on the 15-minute grid; `foot.left` impaired 0.3, `moving` 0.925 |
| 14:00–15:00 | Grid events | `blood` 1.0 → 0.7575 |
| 15:00 | Pell uses a clean bandage on `foot.left` (PS-13) | `bandage` at the site, its dirt 0.01 rising 0.1 an hour; its clean band's lines: `status_strength` mul 0.1 and `status_rate` mul 3 `with` `status.bleeding`, `status_rate` mul 0.25 `with` `status.exposure`; `bleeding` 0.4, now −0.6 an hour; `exposure` 0.10, now +0.02 |
| 15:40 | `bleeding` reaches 0 and ends | `blood` 0.7619, recovering at +0.02 (full at 03:34) |
| 20:54 | The bandage's dirt crosses 0.6: its dirty band replaces the clean one | `exposure` 0.218, now `status_rate` mul 1.5: +0.12 |
| 22:00 | Pell sleeps (`asleep`) | `immunity` lines `status_rate` pct +0.25 `while` `status.asleep` wait for an immunity |
| 00:15 | `exposure` crosses its drawn 0.62 | Its band builds `infection` 0.05 (rate +0.10) and `immunity` 0.01 (rate +0.12, +0.15 asleep) at `foot.left` and ends `exposure` |
| 05:00 | `laceration` ends | `foot.left` unimpaired; `moving` 1.0 |
| 06:00 | Pell wakes | `immunity` 0.8725, back to +0.12 |
| 07:04 | `immunity` reaches 1.0 first | Its band's outputs end `infection` (at 0.731) and itself, and apply `scar` at the site; awake all night, the same race ends at 08:30; unbandaged, `exposure` would have crossed at 21:30 |

`explain rate pell immunity` prints the rate's lines (the status's +0.12, `asleep`'s +25 %), the band ahead (1.0) and its solved time.

### 10.2 A cold, wet night

Oda, an NPC, walks home from 20:00 to 21:30 in rain (0.8), 4 °C outdoors, 3 °C from 21:00 and 2.5 °C from 22:00, wind 10 m/s; she wears light clothes (insulation 6). Felt temperature is her space's temperature plus insulation minus wind chill (4.75 °C at 10 m/s, outdoors only); body temperature approaches the thermoregulation curve of felt temperature at 0.8 an hour (felt −10 °C → 33.2, 0 → 34.6, 5 → 35.4, 10 → 36.2, 15 → 36.8, 20 → 37.0, linear between). Wetness rises 0.48 an hour outdoors in this rain and falls 0.25 indoors; its bands `wetness.damp`, `wetness.wet` and `wetness.soaked` at 0.3, 0.6 and 0.9 (gap 0.02) cut insulation by 20, 40 and 60 %. Her shack holds 6 °C, approaching the outdoors at 0.5 an hour until she lights the stove at 21:40 (`temperature_target` add 14). Outdoors her inputs change only at events (the hour, her own bands), so she takes no grid; indoors her target reads the shack's moving temperature, so the 15-minute grid runs.

| World time | Event | Values |
| --- | --- | --- |
| 20:00 | Leaves the mill | body 36.9, wetness 0.05, felt 5.25 → body target 35.44 |
| 20:31 | Wetness crosses 0.3: `wetness.damp` | insulation 4.8, felt 4.05, target 35.25 |
| 21:00 | World hour | outdoors 3 °C, target 35.09 |
| 21:03 | Body crosses 36.0: `cold.chilled` | mood −0.05 `while` `cold.chilled`; warmth's urgency 0.6 |
| 21:09 | Wetness crosses 0.6: `wetness.wet` | insulation 3.6, felt 1.85, target 34.90 |
| 21:30 | Enters her shack (4.7 °C): `in.enclosed`, out of the wind; the grid starts | body 35.675, its lowest; felt 8.32, target 35.93 |
| 21:40 | Lights the stove (her choice, §7.2) | the shack's target 17 °C, 16.5 from 22:00 |
| 22:16 | Wetness falls past 0.58 and leaves `wetness.wet` | insulation 4.8; the shack 7.7 °C, target 36.50 |
| 22:47 | Body passes 36.1 and leaves `cold.chilled` | mood line gone |
| 23:28 | Wetness falls past 0.28 and leaves `wetness.damp` | insulation 6 |

17 spans from 20:00 to 23:30. Staying outside, she would have been soaked at 21:46, saturated at 21:59, and `cold.hypothermic` (35.0) at 22:53.

### 10.3 A claustrophobic NPC choosing a fire over a tavern

Tobin, an NPC with the `claustrophobic` trait (`weight` mul 0.4 `with` `space.enclosed`; mood −0.15 `while` `in.enclosed`), chooses at 19:10 in a leisure block. Food 0.35 (urgency 0.4625), social 0.30 (0.43), body 36.2 (warmth 0.48). Four considerations, compensated for four:

| Candidate | Satisfies | Benefit | Distance | Product | Weight | Score |
| --- | --- | --- | --- | --- | --- | --- |
| Square fire pit, outdoors | warmth 0.6, social 0.3 | 0.4170 | 40 m: 0.840 | 0.5639 | 1.0 | **0.5639** |
| Tavern hearth, enclosed and crowded | warmth 0.8, social 0.5, food 0.6 | 0.8765 | 60 m: 0.760 | 0.8589 | 0.4 | 0.3435 |
| Home stove, enclosed | warmth 0.7, food 0.4 | 0.5210 | 120 m: 0.520 | 0.5008 | 0.4 | 0.2003 |
| Well bench, outdoors | rest 0.2, social 0.1 | 0.0580 | 20 m: 0.920 | 0.0965 | 1.0 | 0.0965 |

The fire pit wins and its `sit_by_fire` reserves one of its six places. Without the trait the tavern wins at 0.8589. `explain choice tobin` prints this table with each weight line's source (`trait:claustrophobic`).

### 10.4 Eight hours asleep in the cold

Pell sleeps at 22:00 in a shack at 15 °C with a stove holding 3 hours of fuel; outdoors is −1 °C at 22:00, falls a degree an hour to −5 °C at 02:00, holds, and is −6 °C from 04:00 to 06:00. The shack approaches the outdoors plus the stove's 14 at 0.5 an hour; Pell's felt temperature is the shack's plus the bed's 10 insulation; body temperature approaches its target at 0.8 an hour. `asleep` restores rest at +0.095 an hour (from 0.25) and halves food's fall (from 0.6, at 1.6 a day awake). Body temperature starts at 36.8 and follows §10.2's curve. `skip(world, 06:00, watch: [pell])`:

| World time | Event |
| --- | --- |
| 22:00 | Skip starts: 1 holder watched; grid every 15 minutes, as body temperature reads the shack |
| 01:00 | The stove's fuel reaches 0: `fire.lit` ends; the shack's target drops by 14 |
| 05:32 | Body crosses 36.0: `cold.chilled` |
| 05:54 | Rest reaches 1.0: `rested`'s `wake` ends the skip |

Pell wakes at 05:54 chilled (35.90), food 0.337, after 33 spans. Without the blanket's 6 of the bed's 10, Pell is chilled at 03:27 and `cold.hypothermic`'s `wake` ends the skip at 05:50. Sleeping through the same hours live at 1× gives the same values, bit for bit: Pell's inputs come only from the sim.

### 10.5 An area loaded after three days away

The ferry village holds 40 NPCs as rows and a granary of 300 meals. The player leaves at day 3 08:00 and returns at day 6 08:00. Each row eats at 07:00, 12:00 and 19:00 while the granary lasts (a meal restores 0.45), sleeps 22:00–06:00 and loses food at 1.6 a day awake and half asleep, from 0.70.

- **Nine meals fall in the absence.** The granary feeds 40 rows seven times (280) and 20 rows the eighth, at day 5 19:00, in save order: shared stocks count down in §5.3's order.
- **The last 20** miss the eighth and ninth meals: holding 0.45 at day 5 19:00, they cross `need.food.hungry` (0.3) at 21:15, reach 0 at day 6 05:30, and `malnutrition` builds 0.02 an hour: 0.05 at the return. The first 20 return at 0.30, entering `need.food.hungry` at the return.
- **About 1,000 span ends** (sleeps, wakes, meals and crossings by row), with no movement: under 10 ms.
- **On load** the rows promote at their activity's spot (a work block at 08:00: the mill, the dock, the fields), the hungry 20 choosing food first (interrupt `choose`).

## 11. Agent surface: `explain`

- **`explain rate <entity> <stat>`:** the dynamic, each line on its rate or target with its source, kind, value, filter and whether it applied, the value now, the bands ahead and their solved times.
- **`explain choice <entity>`:** the last choice's candidates as §10.3's table: each consideration's input, curve output and compensated value, the weight's lines with sources, the winner, the reservation and the inertia.
- **`explain skip`:** the last skip's world time reached, its reason, the watched holders' events.
- **Records:** the kit's why-records (`kit: tungsten_kit::KitExplain::with_capacity`) gain rate and choice records, kept only while a game inserts the resource, as the ARPG design's §10 has them.

## 12. Numbers

A reference content set a mid-sized survival or colony game would ship, for sizing the kinds, the benches and the screens.

| Thing | Number |
| --- | --- |
| World time | 60 world seconds a game second: a world hour a real minute at 1×, a day 24 minutes |
| Calendar | 24-hour days; 4 seasons of 15 days |
| Weather | 6 states, 6-hour periods, a table per season |
| Needs | 6 (food, water, rest, social, fun, hygiene), 3 bands each |
| Mood | a target summed from lines; `toward` at +0.12 and −0.08 an hour; 3 break bands |
| Exposure | felt temperature, body temperature (4 bands), wetness (3 bands), insulation |
| Body | 1 plan of 14 sites; 3 capacities (moving, manipulation, consciousness); blood; pain |
| Stats a character | 51: 20 dynamic (6 needs, mood, body temperature, wetness, blood, 10 skills' XP), 31 derived, rate or factor stats (10 skill levels, 6 need rates, 4 status factors, 3 capacities, mood's target, body temperature's target, felt temperature, insulation, pain, the thermal rate, `weight`, `satisfaction`) |
| Bands a character kind | 30: 18 on needs, 3 on mood, 4 on body temperature, 3 on wetness, 2 on blood |
| Statuses | 88: 10 wounds and illnesses, 6 conditions, 24 traits, 40 memories, 8 area statuses |
| Instances a character | about 5.6: 2.5 traits, 1.5 memories, 1 area status, 0.3 wounds, 0.3 asleep |
| Skills | 10, levels by curve |
| Smart objects | 30 kinds; 2,500 objects and 3,750 adverts in a 5,000-character area |
| Spaces | 600 in a large area |
| Behaviours | 30 sequences |
| Characters | 5,000 loaded (400 near, 4,600 far); 100,000 rows in 50 areas of 2,000 |
| Events | 45,000 a world hour at 5,000; 750 a second at 1× |
| Choices | 1,850 a second at 1× |
| A save | 6.5 MB of characters and 4 MB of rows (20,000 touched) |

## 13. Five weekly agent tasks

For 8.6's evaluation on a `tungsten new` copy. Each names the rows it runs into today.

1. **A new need.** "Add `hygiene`: falls 0.04 an hour, 0.08 while working; a wash basin's advert serves it to one user at a time; below 0.3, mood −0.05; below 0.1, −0.12 and an illness exposure building." Done when a scripted run shows both bands entered at their solved times, the basin's advert chosen, and `explain rate` lists the lines. Runs into PS-1, PS-2, PS-4, PS-12, PS-14, PS-15, PS-21.
2. **A new trait.** "Add `night owl`: rest falls 20 % slower at night, mood +0.04 at night and −0.04 by day, and work adverts score 1.5× in night blocks." Done when a test across a day shows the lines switching at the hours and a choice preferring night work. Runs into PS-1, PS-7, PS-9, PS-15, PS-19.
3. **A new smart object.** "Add a rain barrel: holds 40 litres, fills at 6 litres an hour while outdoors in rain, adverts `drink` (water 0.5) to two users at a time." Done when a skip through a rainy period fills it in closed form and two NPCs reserve it while a third waits. Runs into PS-1, PS-7, PS-9, PS-10, PS-14.
4. **A new wound.** "Add frostbite on hands and feet: builds at those sites while felt temperature is below −10 °C and the site is bare; impairs manipulation or moving; heals once warm." Done when §10.4's night at −15 °C builds it at the right sites at solved times and `moving` falls. Runs into PS-3, PS-4, PS-5, PS-6.
5. **A balance bug.** "In winter the colonists go hungry though the granary is full: find why (hauling outranks eating in work blocks) and fix it with data only." Done when `explain choice` shows the scores before and after in the report, and a scripted winter week keeps every colonist above 0.3 food. Runs into PS-15, PS-17, PS-21, PS-22.

## 14. Stress numbers

Rows for 9.1, each with its budget category and the surfaces it exercises.

| Row | Workload | Category | Surfaces |
| --- | --- | --- | --- |
| `sim.live_5000` | §12's area: 5,000 characters (400 near on bodies, 4,600 far), 51 stats (20 dynamic), 30 bands and 5.6 instances each, 2,500 objects with 3,750 adverts, 600 spaces, at 1× and at 3× | Frame | Spans, choices, far movers, membership, tiers |
| `sim.weather_front` | A rain period starting over 4,000 outdoor characters in one step | Frame | Deferral, the pass's budget |
| `sim.skip_8h` | An 8-hour skip from 22:00 at 5,000 in abstract mode, 100,000 rows untouched: about 220,000 events and 6,000 choices in 96 windows | Operation | Spans with no budget, windows on the workers, abstract choices |
| `sim.catchup_3d` | An area of 500 rows loaded after 72 hours with a shared stock: about 216,000 events and 15,000 choices in 864 windows, then 500 promotions | Operation | Rows, catch-up, serial commits, promotion |
| `sim.tier_sweep` | The camera crossing the area: 400 near characters replaced in 2 s | Frame | Tier changes, archetype moves |
| `sim.save` | A capture and write at 5,000 characters and 20,000 touched rows | Operation | Saves, the flush of pending holders |

## 15. Build sheet

Verdicts as in the [paper designs](paper-designs.md): **data only** (prefabs and data kinds alone), **game Rust** (the engine gives the parts and the game writes the rest, of the size shown), **gap** (a part no item provides, with the game Rust it would take today where a game could write it at all), **blocked** (a rule or the architecture forbids it). Evidence is a span to search for in `api/*.txt` (`core:` `tungsten-core.txt`, `kit:` `tungsten-kit.txt`, `umb:` `tungsten.txt`, `comp:` `tungsten-kit-components.txt`, `cli:` `tungsten-cli.txt`), or a `none:` pattern that matches no line of `api/*.txt`, case-insensitive; other files are named with their line.

**Characters**

- **SM1 · Needs that fall with time · game Rust (~120 lines: a system writing each need's base by its rate, read in the holder's context, each step).** Nothing moves a stat by itself. The nearest data-only form is an infinite status whose `fuel` drains the need, but a fuel's rate is the status's own, per game second, which no trait scales, and the status ends at 0. Evidence: `kit: tungsten_kit::Stats::set_base`, `kit: tungsten_kit::Stats::read`, `kit: tungsten_kit::Read<'a>::holder`, `comp: field fuel: shape effect_fuel = null — a stat of its holder's an instance burns while it runs`, `comp: field rate: f32 (> 0) — taken off the base a second; the instance runs out at 0`; searched `none: \bneed_|need_?level|hunger|thirst|fatigue`.
- **SM2 · Need levels with consequences · gap: bands on stats with enter and exit outputs (8.9a); about 120 lines today.** Bands threshold a status's intensity only; a condition reads a stat where a story item asks, and only a quest stage acts on one, once, when its objectives hold, with no exit or hysteresis. Evidence: `comp: field bands: list of shape effect_band = null — a status's thresholds`, `comp: shape stat_condition: A stat the target has, within inclusive finite bounds when given.`; searched `none: stat_?band|band_?on_?stat|threshold_?stat`, `none: hysteresis`.
- **SM3 · Mood's memories · data only.** A memory is a timed effect with a modifier on a `mood` stat and a stack limit. A situational thought is a `while` line on a tag that only PS-2's need bands and PS-10's spaces grant, so it waits for them. Evidence: `comp: field duration: f32 (>= 0) = null — seconds of game time it lasts`, `comp: field modifiers: list of shape stat_modifier = [] — on the target's stats, per stack`, `comp: shape effect_stack: An effect's stacking.`, `kit: tungsten_kit::Line::while_holder`.
- **SM4 · Mood toward its target, and breaks · gap: `toward` and bands (8.9a, PS-1, PS-2); about 100 lines today.** Evidence: `kit: tungsten_kit::Stats::get`; searched `none: mood_?(target|stat)|morale|mental_?break`.
- **SM5 · A body plan with sites · gap (8.9a); about 400 lines today** (a site map per entity, statuses per site by hand, a site drawn per hit). An entity carries one list of effects, none keyed by a site. Evidence: `comp: component effects: The effects the entity carries`; searched `none: body_?plan|body_?part|hit_?location|\blimb`, `none: \bsites?\b`.
- **SM6 · Wounds that bleed, heal and impair · gap: sited instances and impairment (8.9a, on SM5); about 250 lines today.** A status's intensity, modifiers and scale curve exist, at the entity's level. Evidence: `comp: field intensity: shape effect_intensity = null — makes it a status`, `comp: field scale: shape effect_scale = null — a curve at the request's scale`; searched `none: infect|bleeding|\bwounds?\b|bandage` (a bare `bleed` names the CRT pass's bleed).
- **SM7 · A treatment that stops one effect of a status, not its clock · gap: holder-side factors (8.9a); about 120 lines today.** `suppressed_by` holds a whole instance, its time, payload and end (`crates/tungsten-kit/src/damage/effects.rs:265-267`), and a status's strength is its source's `status_effect` (`crates/tungsten-kit/src/damage/pipeline.rs:1561-1570`). Two holder-side parts exist: a chemistry build's `per` divides its amount by a receiver's stat, a partial `status_build`, and a game can set one application's status effect in code. Evidence: `comp: field suppressed_by: list of tag = [] — tags that hold a running instance: its time and period wait, no payload lands`, `kit: tungsten_kit::Effect::with_suppressed_by`, `comp: field per: text = null — a stat`, `kit: tungsten_kit::EffectRequest::with_status_effect`; searched `none: status_?(rate|factor|strength|build)|holder_?factor`.
- **SM8 · Infection racing immunity, from a chance drawn once · gap: signed rates and drawn thresholds (8.9a); about 180 lines today.** A status only wears: its decay is at least 0 and at 0 it ends (`crates/tungsten-kit/src/damage/effects.rs:833-841`, `:4487-4495`). Evidence: `comp: field decay: f32 (>= 0) = 0.0 — taken off a second, over the step's time inside the lifetime`, `comp: field at: f32 (> 0) — the intensity it starts at`, `kit: tungsten_kit::KitRng::range`; searched `none: grow_?rate|rise_?rate|intensity_?rate`, `none: per_?instance_?(roll|draw)|draw_?at\b|random_?threshold`.
- **SM9 · Capacities from sites, and what they slow · gap: derived stats (8.9a); about 150 lines today.** A line's amount is a number; a status's curve scales its own lines. Evidence: `comp: shape modifier: A change to a value: added to the base, added to the percentage factor, multiplied, or replaced.`; searched `none: move_?capacity|manipulation|consciousness|body_?capacit`, `none: derived_?(stat|line)|stat_?from|line_?from`.
- **SM10 · Exposure: felt and body temperature, wetness, insulation · gap: approach, reads of the holder's space (8.9a); about 300 lines today.** Evidence: `kit: tungsten_kit::Stats::set_base`; searched `none: temperature|insulat|wind_?chill|hypotherm`, `none: wetness|\bwet\b`, `none: approach_?rate|\bapproach\b`.
- **SM11 · Traits · data only.** A trait is an infinite status of tags and lines, and chemistry's most specific `on` filter gives a trait-tagged receiver its own rule (`crates/tungsten-kit/src/chemistry/rules.rs:15-20`). Traits on statuses wait for SM7's factors, rate traits for PS-1's rates and aversions for PS-10's spaces. Evidence: `comp: field grant: list of tag = [] — tags the target carries while an instance runs`, `comp: shape interaction: A chemistry rule: when a stimulus reaches a receiver whose tags pass`, `kit: tungsten_kit::Line::with`.
- **SM12 · Skills: XP, levels, learning and decay · game Rust (~150 lines: XP in stats, a level per skill by curve, decay above a level each world day).** The kit's `level` reads one counter along one curve per entity. Data only once PS-1 and PS-3 land. Evidence: `comp: component level: A level climbed along a curve by a counter`, `kit: tungsten_kit::Counters::add`, `kit: tungsten_kit::Curve::at`; searched `none: learn_?rate|skill_?xp|skill_?decay|\bxp_?rate`.
- **SM13 · Players and NPCs on one rule set · data only.** Both carry the same components; the interactor reads its intent slot, which an AI writes. Evidence: `comp: component interactor: Whoever interacts: a press of its intent slot uses the nearest interactable in reach.`, `comp: component action_input: The actions that steer the entity, read into its Intent each fixed step.`, `kit: tungsten_kit::Interactor::focus`.

**World**

- **SM14 · A calendar, seasons and hours · gap (8.9a; 8.1's P8-6 builds on it); about 200 lines today.** The paper designs sized a game's own clock and its look as game Rust (LW10, about 200 lines); here the clock is a gap because kit systems read it: statuses' rates, schedules and spans need world time counted in steps, which no game resource can give the kit. Evidence: `core: tungsten_core::time::Time::elapsed`; searched `none: calendar|season|time_?of_?day|world_?clock`, `none: world_?time|sim_?clock|game_?hour|\bhours?_?per`.
- **SM15 · Weather and wind, on a seeded timeline · gap (8.9a); about 300 lines today.** A seeded stream exists, weighted draws exist only over prefabs, and a world source puts lines on every entity whose tags pass its filter, which could carry a weather's lines but changes on no timeline and by no one's space. Evidence: `kit: tungsten_kit::KitRng::seeded`, `comp: shape weighted_table: Rolls that each draw one entry by weight`, `kit: tungsten_kit::WorldSources::set`; searched `none: weather|\brain\b|snow|forecast`, `none: \bwind\b|gust`.
- **SM16 · Outdoor temperature by season, hour and weather · gap (8.9a, on SM14 and SM15); about 80 lines today.** A world source could hold the hour's value as a line, set again each hour by the game. Evidence: `kit: tungsten_kit::WorldSources::apply_to`; searched `none: temperature|insulat|wind_?chill|hypotherm`.
- **SM17 · Daylight and light levels · game Rust (~60 lines: the ambient light and a `light` world stat from the clock, LW10's look).** Evidence: `core: tungsten_core::lighting::AmbientLight`, `core: tungsten_core::lighting::light_level`, `core: tungsten_core::assets::room::Room::ambient`.
- **SM18 · Spaces: authored rooms with tags and stats · gap (8.9a); about 300 lines today.** A room's places hold rects and points and a spawned room names them; nothing holds a point's room, its tags or its stats. Evidence: `core: tungsten_core::assets::room::RoomPlace::rect`, `core: tungsten_core::assets::room::RoomShape::polygon`, `core: tungsten_core::SpawnedRoom::place`, `core: tungsten_core::assets::CompiledPlace::shape`; searched `none: space_?at|room_?at|indoors?|outdoors?|enclosed`.
- **SM19 · Area statuses on enter and exit · gap: membership without a body (8.9a); about 80 lines today for bodied characters.** The kit's zone sends entries only (`crates/tungsten-kit/src/overlap/zone.rs:132-139`); core's sensor exits exist, for bodies. A contact-damage aura reaches hurtboxes, which need no body, each interval, so a periodic stimulus could stand in for an area status, with no exit. Evidence: `comp: component zone: A sensor entered by the entities whose tags pass its filter.`, `core: tungsten_core::physics::events::SensorExited`, `comp: component contact_damage: Hurts the hurtboxes its shape touches, at once and again each interval while they touch`; searched `none: enter_?space|space_?(entered|exited)|area_?status`.
- **SM20 · A space's temperature and its contributions · gap (8.9a); about 200 lines today.** A status emits a stimulus to receivers around its holder on a period; nothing sums sources into a room. Evidence: `comp: shape effect_emit: A chemistry stimulus an instance's holder sends every period`; searched `none: temperature|insulat|wind_?chill|hypotherm`.
- **SM21 · A provider for rooms from runtime terrain, later · gap: the interface (8.9a); about 60 lines today.** A room's collision layer holds the solid tiles such a provider would trace. Evidence: `core: tungsten_core::assets::room::RoomLayerKind::Collision`; searched `none: flood_?fill|room_?detect|room_?provider`.

**AI**

- **SM22 · Smart objects: adverts, uses, capacity, reservations · gap (8.2, P8-10); about 500 lines today.** An interactable offers one verb to whoever passes its filter and condition; a cap counts live instances, not users. Evidence: `comp: component interactable: Something an interactor in reach uses with a press`, `comp: field who: tag filter = {} — the interactors whose tags pass`, `comp: field when: shape condition = null — offered only while it holds`, `comp: component cap: Holds its group's live instances to a number`; searched `none: reservation|claim_?(slot|object)|advertis|smart_?object`.
- **SM23 · Utility scoring: considerations, curves, weights · gap (8.2); about 400 lines today.** Curves and context reads exist. Evidence: `comp: shape curve: A piecewise-linear map`, `kit: tungsten_kit::Stats::read`; searched `none: utility|consideration|\bscores?\b`.
- **SM24 · Behaviours: a sequence of steps, or a game's state machine · gap: the sequence (8.2); about 300 lines today, a state machine per behaviour set.** Evidence: `kit: tungsten_kit::StateMachine<S>::go`, `kit: tungsten_kit::StateMachinePlugin<S>::on_update`, `kit: tungsten_kit::Steering::seek`; searched `none: behaviou?r_?(tree|step)|\bjobs?\b`, `none: interrupt`.
- **SM25 · Schedules · gap (8.1's third plan, P8-6); about 400 lines today** (LW11's schedules). Evidence: searched `none: routine|timetable|daily_?plan`.
- **SM26 · Priorities as weights, and hauling · gap (8.9b); about 350 lines today.** Carrying and pickups exist. Evidence: `kit: tungsten_kit::carry`, `kit: tungsten_kit::release`, `comp: component pickup: Collected once by the first entity whose hurtbox it meets`; searched `none: stockpile|\bhaul`.
- **SM27 · Movement for thousands without bodies · gap (8.2, P8-8's flow fields); about 150 lines today to sample a flow field into a bodiless mover's intent.** A `top_down_mover` without a body already moves freely by its intent; nothing finds a way. Evidence: `comp: component steering: Seeks, flees or arrives at a target`, `comp: component top_down_mover: A top-down character`; searched `none: flow_?field|navmesh|path_?find|navigat`.
- **SM28 · Explain a choice · gap (8.2); about 250 lines today.** Why-records exist for sheets and hits. Evidence: `kit: tungsten_kit::KitExplain::with_capacity`; searched `none: explain_?(choice|rate)|why_?chose`.
- **SM29 · Choices driven by needs: urgency, adverts that satisfy, breaks · gap (8.9b, on SM22 and SM23); about 200 lines today.** Evidence: `comp: field when: shape condition = null — offered only while it holds`; searched `none: \bneed_|need_?level|hunger|thirst|fatigue`.

**Time and scale**

- **SM30 · Speeds past twice real time · game Rust (~20 lines: a speed control raising the step cap with the scale).** The cap is settable; each step's work runs again per step. Evidence: `core: tungsten_core::time::Time::set_scale`, `core: tungsten_core::time::Time::set_max_steps_per_frame`, `core: tungsten_core::time::Time::dropped_this_frame`.
- **SM31 · Skips in closed form · gap (8.9a); about 600 lines today** (a game's own span engine). The clock stops for the game's pause, the state stack's hold and the editor, each its own stop; nothing advances the world without steps. Evidence: `core: tungsten_core::time::Time::set_paused`, `core: tungsten_core::time::Time::set_editing`; searched `none: fast_?forward|closed_?form|catch_?up|time_?skip|\bskip_?to`, `none: sleep_?skip|\basleep\b`.
- **SM32 · Upkeep while unloaded: a population table · gap (8.9b); about 700 lines today.** Unloaded areas are kept as saves to apply. Evidence: `core: tungsten_core::save::enter_area`, `core: tungsten_core::save::WorldSave::area_names`; searched `none: population|unloaded|offscreen|off_?screen`.
- **SM33 · Catch-up at an area's load · gap (8.9b, on 8.1's P8-7 hooks); about 400 lines today.** A game spawns an area and applies its kept state; a catch-up would be its own code after that call, since `save::on_restore`'s hooks run after a whole restore, not at an area's entry. Evidence: `core: tungsten_core::World::spawn_area`, `core: tungsten_core::save::enter_area`; searched `none: fast_?forward|closed_?form|catch_?up|time_?skip|\bskip_?to`.
- **SM34 · Thousands of characters' stats read by ID · gap (8.9a); about 300 lines today** (a game's own dense table mirrored into `Stats`). A read searches by name. Evidence: `kit: tungsten_kit::Stats::get`; searched `none: stat_?id|interned|intern_?stat`.
- **SM35 · Tiers: near, far, unloaded · gap (8.9b); about 500 lines today.** Evidence: searched `none: \blod\b|level_?of_?detail|\btiers?\b`, `none: promot|demot`.
- **SM36 · The sim on worker threads, bit-identical · game Rust (~150 lines: gather into an owned input, map, scatter).** Evidence: `core: tungsten_core::workers::map`, `core: tungsten_core::workers::SystemWorkers`.
- **SM37 · Saves at scale · game Rust (~100 lines: the table a saved resource in columns at full precision).** Evidence: `core: tungsten_core::World::register_saved_resource`, `core: tungsten_core::save::capture_world`.

**Colony, researched**

- **SM38 · Construction · gap (8.9b, admitted by the triage, PS-26); about 600 lines today.** Tiles change at runtime, and a payload sets a prefab down at the aim's point, a blueprint's start. Evidence: `core: tungsten_core::assets::tilemap::TilemapRegistry::set_tile`, `comp: shape set_tile_output: A tile cell written`, `comp: shape place_payload: A prefab set down by its centre at the aim's point`; searched `none: blueprint|construct`.
- **SM39 · A storyteller · gap (8.9b, admitted by the triage, PS-27); about 400 lines today.** Spawners spawn on an interval. Evidence: `comp: component spawner: Spawns a prefab at the entity's position once per interval of game time.`; searched `none: storyteller|incident|threat_?points`.

**Items**

- **SM40 · Food, water, medicine and clothing · gap until 8.1's second plan lands (P8-3, PA-12); data only after it.** A use's `restore` and statuses change needs; a worn item's lines give insulation. Evidence: `comp: shape effect_restore: A payload that raises one of the target's stat bases, capped by another stat.`, `comp: shape apply_output: A status applied, with a damage-over-time rate and lines.`; searched `none: inventory|item_?base`.
- **SM41 · A use aimed at another and at a site · gap (8.1's second plan, PS-13); about 60 lines today.** Evidence: searched `none: use_?on|target_?site|use_?item`.
- **SM42 · Food that spoils · gap (8.9b); about 80 lines today.** Evidence: searched `none: spoil|freshness|\brot\b`.

Verdicts: 3 data only, 6 game Rust (about 600 lines), 33 gaps (about 9,030 lines where a game could write them today), none blocked.

## 16. Register

Rows in the analysis pass's shape (pattern, where, lines, what is lacking, the generic fix, dependencies), as the [ARPG design](arpg-design.md)'s §15 has them, with the kit domain that holds each fix. **Where** names §15's entries; an entry in parentheses adds no lines here. **Lines** is the game Rust those entries take today, each counted once. **Step** is the default the owner's triage confirms or moves.

| ID | Pattern | Where | Lines | Lacking | Generic fix | Domain | Depends on | Step |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| PS-1 | Stat dynamics | SM1, SM4, SM10 | 520 | Nothing moves a stat over time | `rate`, `toward` and `approach` per world hour on declared stats, read in context; base writes as deltas (§4.1) | foundation | PS-8 | 8.9a |
| PS-2 | Stat bands | SM2 | 120 | Nothing thresholds a stat | Bands with hysteresis, tags, lines, enter and exit outputs and interrupts (§4.2) | foundation | PS-1 | 8.9a |
| PS-3 | Derived stats | SM9, SM12 | 300 | A line's amount and a base are numbers | `from` a stat through a curve, the holder's space, the world or a capacity; a line's amount from a stat, which 8.7's `per` (PA-9) builds on (§4.3) | foundation | PS-1 | 8.9a |
| PS-4 | Sim statuses | SM8 | 180 | A status only wears, per game second | A signed `rate` per world hour, span-evaluated; band lines and exits; a drawn `at` (§4.4) | damage | PS-8 | 8.9a |
| PS-5 | Holder-side factors | SM7 (SM11) | 120 | A status takes nothing from its holder | `status_build`, `status_rate`, `status_duration`, `status_strength` read in the instance's context; takes Later's EV-21 (§4.4) | damage | PS-4 | 8.9a |
| PS-6 | Sites and body plans | SM5, SM6 | 650 | No site on a hit or an instance | The `body_plans` kind; instances keyed by status and site; a hit's site to chemistry's entry; sited lines; impairment and capacities (§4.5) | damage, sim | PS-3, PS-4 | 8.9a |
| PS-7 | The sim clock, speeds and skips | SM14, SM30, SM31 | 820 | Nothing skips; at 60 Hz a speed past 2× drops fixed steps | World time counted in steps; the calendar; `set_speed`; `skip` with abstract mode and interrupts (§5.1, §5.5, §5.6) | sim | PS-8 | 8.9a |
| PS-8 | The span engine | (SM31) | — | Nothing advances a value between steps | Spans, the event queue in §5.3's order, the grid, deferral, sim-safe outputs, draws by key (§5.2–§5.4, §5.8) | foundation | — | 8.9a |
| PS-9 | World stats and weather | SM15, SM16, SM17 | 440 | No weather, wind or outdoor temperature | The `calendar` and `weather` kinds; the kept timeline; world stats on each area's outdoors (§4.7) | sim | PS-7 | 8.9a |
| PS-10 | Spaces | SM18, SM19, SM20, SM21 | 640 | No room at a point; the zone sends entries only and needs a body | `space`, the provider interface and index, `space_at`, membership, occupant statuses, space stats and contributions (§8) | sim | PS-1, PS-3 | 8.9a |
| PS-11 | Stats by ID, densely | SM34 | 300 | Every read searches by name | Interned `StatId`s; `SimState` laid out by kind (§4.10, §6.2) | foundation | — | 8.9a |
| PS-12 | `explain` of a rate and a skip | — | — | Why-records cover sheets and hits only | `explain rate` and `explain skip` on the kit's why-records (§11) | foundation, umbrella | PS-1, PS-7 | 8.9a |
| PS-13 | A use aimed at a target and a site | SM41 (SM40) | 60 | A use's subject is its holder | `use_item_on(world, holder, cell, target)`: the target as the list's subject; 8.9a adds the site (PS-6) | items | — | 8.1 second |
| PS-14 | Smart objects | SM22 | 500 | One verb per interactable; nothing counts users | A `smart_object`'s adverts with `satisfies`, behaviour, capacity, hours, `when`; reservations (§4.8, §7.1) | ai | — | 8.2 |
| PS-15 | Utility scoring | SM23 | 400 | No AI structure | Considerations through curves, compensated; weights as lines in the candidate's context; inertia; ties by key (§7.2); settles P8-10 | ai | PS-14 | 8.2 |
| PS-16 | Behaviours | SM24 | 300 | Only a game's state machine | Sequences of steps with aborts and reservations; a `StateMachine` in their place (§4.9, §7.3) | ai | PS-14 | 8.2 |
| PS-17 | `explain` of a choice | SM28 | 250 | Why-records cover sheets and hits only | Candidates, considerations, weight lines, the winner (§11) | ai | PS-15 | 8.2 |
| PS-18 | Bodiless movers on flow fields | SM27 | 150 | Nothing finds a way | A far character's bodiless mover takes its intent from P8-8's flow fields, sampled by position (§6.1) | motion | P8-8 | 8.2 |
| PS-19 | Schedules on the sim clock | SM25 | 400 | No clock or schedules | P8-6's blocks on world time, naming an unloaded holder's activity; 8.2's scoring reads them as a consideration (PS-15, §7.4) | story, sim | PS-7 | 8.1 third |
| PS-20 | Area hooks for populations | (SM33) | — | Areas swap by the game's own code | P8-7's unload and load call demotion, catch-up and promotion in §9.3's order | story, sim | P8-7 | 8.1 third |
| PS-21 | Choices driven by needs | SM29 (SM4) | 200 | Nothing chooses by a need | `urgency` curves, `satisfies` scaled by `satisfaction` lines in the advert's context, `using` statuses, breaks as bands that interrupt (§7.2, §7.3) | sim | PS-2, PS-15 | 8.9b |
| PS-22 | Priorities and hauling | SM26 | 350 | No jobs or stockpiles | Priority lines on `weight`; stockpile spaces; haul adverts with two reservations (§7.5) | sim | PS-10, PS-14 | 8.9b |
| PS-23 | Population, tiers and catch-up | SM32, SM33, SM35 | 1,600 | Nothing simulates an unloaded area or a far character cheaply | Rows, near and far tiers with budgets, abstract mode, catch-up with shared stocks, rows of never-loaded areas from their generator (§5.7, §6, §7.6) | sim | PS-7, PS-18, PS-20 | 8.9b |
| PS-24 | Workers and saves at scale | SM36, SM37 | 250 | The pass on one thread; saves of thousands unmeasured | The span pass, choices and catch-up on `D-311`'s map; the table's columns at full precision; the flush of pending changes before a capture (§6.5, §6.7, §9.4) | sim | PS-8, PS-23 | 8.9b |
| PS-25 | Food that spoils | SM42 | 80 | Items do not age | An item instance's world time and a closed-form freshness read at use | items, sim | PS-7 | 8.9b |
| PS-26 | Construction | SM38 | 600 | No construction | Blueprint, frame and built on runtime tiles, work by skill | sim | PS-22 | 8.9b |
| PS-27 | A storyteller | SM39 | 400 | Nothing paces events | Events paced by a curve of wealth and time, with on and off phases | sim | PS-9 | 8.9b |

The rows hold all 9,630 of the build sheet's game Rust lines: the six game Rust entries (600) and the 33 gaps (9,030), SM40 counting none of its own (8.1's).

**Triage, 2026-11-28.** The owner kept every row at its default step and admitted both researched rows: PS-1–PS-12 to 8.9's first part, PS-13 to 8.1's second plan, PS-14–PS-18 to 8.2, PS-19 and PS-20 to 8.1's third plan, and PS-21–PS-27 to 8.9's second part, construction (PS-26) and a storyteller (PS-27) among them. The redo's §3 rows hold them by ID.

## 17. Minimal implementation scope by step

Each step builds its rows in the domain the register names, as opt-in plugins where §3.2 says so.

| Step | Rows | What lands | Done when |
| --- | --- | --- | --- |
| 8.1's second plan | PS-13 | A use aimed at a target | A potion used on an ally runs its list with the ally as the subject (a kit test) |
| 8.9's first part | PS-1–PS-12 | Stat dynamics, bands and derived stats; sim statuses, factors and sites; the sim clock, speeds and skips; the span engine; world stats and weather; spaces; stats by ID; `explain rate` and `skip` | §10.1, §10.2 and §10.4 reproduce their tables in kit tests; a skip and a live 1× run of §10.4 give bit-identical stats; a span pass at 1, 2 and 6 workers gives one result; 01's and the template's pins hold, no stat there declaring a dynamic |
| 8.1's third plan | PS-19, PS-20 | Schedules on the sim clock; the area hooks | An NPC is placed by its schedule while its area is unloaded (P8-6) and promoted at its load |
| 8.2 | PS-14–PS-18 | Smart objects, reservations, scoring with schedules as a consideration, behaviours, `explain choice`, bodiless movers | §10.3's table reproduces in a kit test with `explain choice`'s output; Static Front's squads take cover through adverts with capacity 1 |
| 8.9's second part | PS-21–PS-27 | Needs-driven choices, priorities and hauling, rows and tiers, catch-up, workers, saves at scale, spoiling food, construction and a storyteller. The `sim` genre tag, recipes in `sim` and one other genre, a sketch and a scale scene in the fixtures crate, §14's rows | §10.5 reproduces; `sim.live_5000` runs at 1× and 3× and `sim.skip_8h`, `sim.catchup_3d` and `sim.save` record their operation times; in the sketch a colonist builds a hauled blueprint's frame into a wall, and a storyteller's 60 world days of events replay from its seed; §13's five tasks pass on a `tungsten new` copy at 8.6 |

8.9's first part changes shapes 8.1's third plan and 8.2 build on (the clock, stats, statuses), so its rows cannot move after them; PS-13 is small enough for 8.1's second plan to take at no rework, or for 8.9's first part.

## 18. Trade-offs

- **T1. Events and closed forms, not per-step integration.** Skips, speeds and catch-up cost events, not steps, and agree with live play. The cost: inputs are held within a span, and a coupled value moves on a 15-minute grid, a first-order error a step-based model would also make at its own step.
- **T2. Held inputs, not coupled equations.** A body temperature reading a warming room follows the room on the grid instead of an exact two-exponential solution. Exact pairs exist for a few shapes; one rule for every shape keeps the engine small.
- **T3. Abstract mode for skips and unloaded areas, not live fast-forward.** A night's skip at 5,000 costs about 0.73 s serial and 0.2 s on six participants (§6.6), where fast-forwarding 8 world hours runs 28,800 steps of the full kit, 12 s at 40× if the CPU keeps up; NPCs reappear at their activities' spots instead of walking there. A game that wants continuity fast-forwards at a speed instead (§5.5).
- **T4. Needs as falling levels.** `restore` already raises a base to a cap, so eating is today's payload; a rising hunger would need a second payload.
- **T5. Sites as keys, not entities.** Thousands of characters with 14 sites would be 70,000 entities; a key on an instance costs bytes. A site has no components of its own.
- **T6. Holder-side factors as stats read in context.** One mechanism for treatments, traits and EV-21, explained as lines; it costs a context read per factor per event.
- **T7. Scores, with priorities as weights.** A colony's strict order becomes weights 8, 4, 2 and 1, so an urgent need can win over a lower job, which the colony game hard-codes with thresholds. Strict buckets would be a second selection rule.
- **T8. Spaces as entities.** They take tags, stats, statuses, saves and `explain` for free; nested spaces need §8.2's depth rule.
- **T9. Rows apart from entities.** A row is about 200 bytes against a character's 2 KB of components, and never-loaded areas cost nothing; the price is promotion and demotion code, and a row's rules in abstract mode only.
- **T10. Lazy upkeep.** An unloaded area changes only when it loads; a famine elsewhere is unknown until visited unless the game opts into a daily settle pass on the workers (PS-Q7).
- **T11. Determinism on one platform.** `exp` and `ln` come from the platform; a software `exp` would make saves replay across platforms at a cost on every approach.
- **T12. In the shared plugins.** Dynamics, bands and factors change `StatsPlugin` and `EffectsPlugin` for every genre; a stat or status without them costs a branch at load and none per step, so one rule set stays.

## 19. Questions, each with a default

The owner accepted PS-Q1–PS-Q8 and PS-Q10 on 2026-11-28 and answered PS-Q9 by admitting both rows.

- **PS-Q1. The new domain's name.** *Default: `sim`*, beside `story` and `build`.
- **PS-Q2. Where 8.2's AI lives.** *Default: a kit domain `ai`* holding smart objects, scoring and behaviours; 8.2's squads and cover use it.
- **PS-Q3. World time.** *Default: integer world milliseconds, 60 world seconds a game second,* a game's setting.
- **PS-Q4. Combat statuses.** *Default: unchanged;* only statuses with a `rate` are span-evaluated, so no hash or pin moves for 01's statuses.
- **PS-Q5. The resample grid.** *Default: 15 world minutes,* a game's setting.
- **PS-Q6. Sleep: skip or fast-forward.** *Default: both;* the kit offers `skip` and `set_speed`, and the template's sample uses `skip`.
- **PS-Q7. Unloaded upkeep.** *Default: lazy at load,* with an opt-in daily settle pass.
- **PS-Q8. Rows in saves.** *Default: JSON columns at full precision,* so a save and a load change no value; no binary form, `D-230`'s plain JSON kept. Rounding to three decimals would halve the rows at the cost of a load moving values.
- **PS-Q9. Construction and a storyteller.** *Default: not admitted;* a recipe shows a game writing each on the kit's parts. **Answered 2026-11-28: both admitted,** to 8.9's second part (PS-26, PS-27).
- **PS-Q10. PS-13's step.** *Default: 8.1's second plan,* small and useful to an RPG's healing of an ally.

## 20. Verification

**Mechanical checks,** run in the session's scratchpad on each revision: every `core:`, `kit:`, `umb:`, `comp:` and `cli:` span searched for verbatim in its file (86 distinct after both rounds, none missing); every `none:` pattern run over `api/*.txt` (42 distinct, each 0 hits; `infect|bleed|wound|bandage` first matched the CRT pass's `bleed` and was narrowed); every file and line reference inside its file (16); §15's tallies and §16's line column summed by script (600 and 9,030, 9,630 in all, each entry counted once; the sum found SM10 in no row, so PS-1 took it); `just repo-check` and `just ctx` (0 errors). §10's numbers came from a reference span engine implementing §5.2 and §5.3; it first skipped a crossing that fell on a fixed event's time and looped on a float residue at 0, both fixed before any number was taken.

**Critique,** one round at xhigh on §5 and §6, sent as one 32,399-character extract with §3.4, §4.1–§4.4, §4.10, §7.6, §9 and §10.4 and the engine's facts quoted from the tree (the fixed-step accumulator, today's status decay, `workers.rs`'s contract, `Stats`' storage, scoped lines, the phase table, `D-230`'s areas): gpt-6-astra, artifact only, 445.5 s, 20,595 tokens in. Eight findings, all held and applied:

| # | Finding | Applied |
| --- | --- | --- |
| 1 (high) | Holders advanced on the workers past a shared stock or reservation keep continuations computed against stale state | Abstract mode runs in windows ending at a 5-minute decision grid: private events in parallel, choices with their reservations and draws committed serially in save order (§5.6, §6.5) |
| 2 (high) | A deferred occupant can need a span its space has already replaced | A space keeps its spans back to its oldest pending occupant, processes such an occupant before dropping one, and a 64-step horizon bounds the wait (§5.4, §6.4) |
| 3 (high) | Rows lack band memberships and the stream's counter, which a value inside a band's gap cannot reconstruct | `SimState` and rows carry spans, band memberships, draws and the counter (§4.10) |
| 4 (high) | A capture relabelled unprocessed spans as starting at the capture, and a load read inputs again between grid points | A capture processes pending changes at their own times and saves spans as they stand; a load restores them and reads no input; rows at full precision (§9.4, §6.7, PS-Q8) |
| 5 | Reading rates through `Stats::read` fed the `f32` mirror back | Inputs resolve as line sums applied in `f64` to the sim's own base; rate and target stats are static or derived, never dynamic (§4.1, §5.2) |
| 6 | Grids aligned to world time 0 put 5,000 holders in one step, and the table averaged that away | A grid phase per holder; the budget covers every event; the 1.5 ms peak beside the averages (§5.3, §6.3, §6.4) |
| 7 | The operation counts implied 40-minute abstract activities against the stated 20-minute behaviours | Abstract activity lengths stated (an advert's hours or a block, 10 a day, a night's sleep one), and the skip and catch-up recounted (§6.6, §14) |
| 8 | One area's catch-up could not gain from workers that only split areas | Finding 1's windows parallelize inside an area (§5.7, §6.5) |

**Fresh-context verifier,** one round on the whole document, run beside the critique on the draft; it read the working copy as the critique's fixes landed. It confirmed 72 of 77 spans (5 weak, none unsupported), all 42 searches at 0 hits with two or three other names tried for each in `api/*.txt` and the kit's, core's and the umbrella's source, all 15 file and line references, the 42 verdicts (none wrong, 3 weak), the numbers in §6, §12, §14–§16 and the digest, and 8 of 9 decision citations. What changed:

- **Holder-side parts.** A chemistry build's `per` divides its amount by a receiver's stat, and a request can set its own status effect: the digest, SM7 and PS-5 say so in place of "takes nothing from its holder".
- **Decisions.** Statuses are `D-240`, their band looks `D-242`'s, and chemistry `D-241` (§1).
- **Order.** PS-19 no longer depends on PS-15, which lands after it; 8.2 reads schedules as a consideration (§16, §17).
- **The critique's fixes carried through.** SM37 and PS-24 at full precision; §6.2's rows, §7.6 and §9.2 on the windows.
- **§10.5.** The last 20 cross `need.food.hungry` at §4.2's 0.3 at 21:15, not 22:00; the script had tested 0.25.
- **Verdicts' wording.** SM2 (a quest stage acts once on a stat condition), SM3 (memories only), SM11, SM14 (why the clock is a gap here when LW10's was game Rust), SM27 and PS-18 (a bodiless top-down mover exists; the gap is finding a way).
- **Time.** At 60 Hz a speed past 2× drops fixed steps while `Time::elapsed` adds the whole game time (§5.1, PS-7, the digest); a skip takes a stop of its own, as the editor did, not the state stack's hold (§5.6, SM31); SM33 no longer cites `on_restore`, which runs after a whole restore only.
- **Names.** A smart object's entries are adverts with `satisfies`, since `offers`, `offer` and `gives` are the level-up's; a `satisfaction` stat replaces `give.<need>`, which no stat name can spell (§4.8).
- **Numbers.** T3's fast-forward is 28,800 steps, 12 s at 40×; §12 counts 51 stats (20 dynamic, the skills' XP among them) and names the 30 bands; §6.2 and §6.7 follow.
- **Evidence.** `modifier` for SM9, the collision layer for SM21 and §8.1, `WorldSources` for SM15 and SM16, `contact_damage` for SM19, `place_payload` for SM38; outputs use the existing `remove`; §4.1 takes the sums over a zero base; §9.3 leaves keeping an unloaded area to P8-7; §13's row lists; world tags join the context (§4.7); §10.2 and §10.4 state every input.

These fixes were checked mechanically, not by a second round (the brief's stop).

## Session log

- **2026-11-27.** First commit (`de2f6cc9`): `D-316`, its markers on `D-172`, `D-215` and `D-218`, the redo's §2 answers, §3's 8.8 and 8.9 with the new order, §5.3, §5.7 and §7.4, and `ROADMAP.md`. Read the brief, the survey's ten claims against the tree (each held; the 2-step cap loses time past 2× at 60 Hz only, a higher frame rate raising that bound), the kit's stats, lines, world sources, statuses, chemistry rules, zone, interactor and explain, core's time, lighting, rooms and saves, and the API snapshots. A background agent researched the two games and utility AI (§2). §10's numbers came from a reference span engine and four example scripts in the scratchpad. The draft was committed (`d3e7b1f2`).
- **2026-11-28.** The critique (gpt-6-astra, xhigh, artifact only, on §5 and §6) and a fresh-context verifier ran in parallel on the draft; every finding of both was applied (§20) and the mechanical checks rerun (`443aa403`). The owner triaged §16 in four questions: every row at its default step, construction and a storyteller admitted to 8.9's second part, every question at its default; the redo's §3 rows took the rows by ID.

# ARPG paper design

A gameplay framework for action RPGs, designed on paper by the [roadmap redo](roadmap-redo.md)'s brief ([§7.3](roadmap-redo.md#73-arpg-paper-design-719), step 7.19, `D-218`): skills whose properties tag-scoped modifiers change, equipment from procedurally generated items on the same modifiers, and one rule set for players and enemies. It designs systems, not a game; every name and example here is original. Written against the API snapshots at `2f30d65`, rechecked at `3bc28c7` (`qa6`'s refresh changed no cited span), and the kit's source at `042bcab`.

- **status:** draft
- **goal:** Find what an action RPG needs that the engine and kit lack, and design the framework that fills it in the kit as opt-in plugins in its domains (Q19), as register rows the owner triages to 7.10, 8.1, 8.2 and 8.7 before 7.7 plans Phase 7.
- **non-goals:**
  - Code, and edits to any other file but `ROADMAP.md`'s `arpg-design` line.
  - Each step's mechanisms: the step's own plan specifies them from the tree of its day.
  - A game or a vertical slice (the owner's answer of 2026-10-23); the studied games' names for things, items, content and tree layouts.
  - Minions, totems, traps and mines; reserved auras; flasks; leech; armour, evasion and block (a physical factor and a `damage_taken` line cover 1.0's need); dual wielding; crafting beyond a reroll; trade; multiplayer. A game writes them on §3.6's named handlers, or 1.x takes them.
- **files to touch:** This file. `ROADMAP.md`'s `arpg-design` line at the start and the finish. 7.7 reads §15's register after the owner's triage; nothing else changes before then.
- **ordered steps:**
  1. Read the brief, the kit's stat, damage, ability, projectile and loot code and the API snapshots; targeted research on the mechanics (§2). Done.
  2. The design (§3–§10), the page with numbers, weekly tasks and stress numbers (§11–§13), the build sheet and register (§14–§15), the scope by step (§16), trade-offs and questions (§17–§18). Done.
  3. A fresh-context verifier on every citation (§19). Done.
  4. One `/critique` round at xhigh on §5 and §6, applying what holds (§19). Done.
  5. The owner's triage of §15's rows and §18's questions.
  6. 7.7 assigns the rows to 7.10, 8.1, 8.2 and 8.7; 8.6 takes §12's tasks; 9.1 takes §13's rows.
- **done-when:**
  - Every "data only" and "game Rust" verdict cites an item in `api/*.txt`, and every gap names a search that finds nothing; §19's verifier report confirms or fixes every citation.
  - One critique round on §5 and §6 has run and what held is applied (§19).
  - The owner has triaged every `PA-` row and answered or accepted §18's questions.

## Context digest

- Step 7.19 of the roadmap redo, run on 2026-10-28 in the 0.112 worktree beside `qa6`. 7.7 waits for it (Q20); no UI or camera step does.
- The kit already resolves stats as `(base + Σadd) × (1 + Σpct) × Πmul`, flat, increased and more (`crates/tungsten-kit/src/foundation/stats.rs:96`). What an ARPG needs on top is where a modifier applies (scopes), what a skill reads (a skill sheet snapshotted onto what it spawns), what a hit carries (context tags, crit, conversion with history), what items hold (rolled lines) and where those come from (supports, passives, affixes).
- The design keeps one stat and damage pipeline for every genre: the pieces that change shapes 7.10 builds go into `foundation`, `intent` and `damage`; projectile behaviours into `overlap` with 8.2; item instances with 8.1's items; supports, passives and affix generation into a domain of their own, `build` (Q19, PA-Q1).
- Players and monsters share every rule; they differ only in the sources they carry (§8). 01's player-only crit, halved blasts on the player and enemy-only difficulty all become data.
- The build sheet holds 33 systems: 2 data only, 8 game Rust (about 1,030 lines), 23 gaps (about 6,010 lines where a game could write them today). The register holds 22 rows: 11 into 7.10, 2 into 8.1, 3 into 8.2 and 6 into 8.7.
- Largest finds: hits carry no context beyond damage types, so no modifier can be scoped to a skill kind (PA-1, PA-2); conversion runs in the pipeline after any scaling would and keeps no history (PA-6); a spawned payload takes none of its user's values (PA-2); a status's damage follows a hit only through an ailment rung's fill over the target's threshold, after resistances, and never through the user's lines (PA-8).

## 1. Assumptions

Provisional, as the brief says; 7.7 rechecks each before it assigns §15's rows.

- **The redo's §5.2 statuses and Q17's merge.** Effects become statuses with intensity, stacking, granted tags and stat changes; this design's lines (§4.1) are what a status's stat changes become, and a status's lines scale with its intensity through `curves`.
- **P8-3's equipment as statuses.** An equipped item applies a status whose lines are the item's rolled global lines (§4.6).
- **The `Modifier` formula,** which no step changes: `add`, `pct`, `mul`, `override` (`comp: shape modifier: A change to a value`).
- **P8-11's and P8-13's payloads:** bursts and a pattern payload with a count and an arc, which projectile count and repeats ride (§6.3).
- **The redo's §5.5 saves** (components registered as saved, stable IDs, references remapped) and **P7-16's seeds**.
- **Q11's names:** the `chemistry` domain and `interactions` kind; cues keep `react`.

## 2. Research

Targeted, mechanics only, from public documentation: Path of Exile's modifier, conversion, area, projectile and item rules ([damage conversion](https://www.poewiki.net/wiki/Damage_conversion), [area of effect](https://pathofexile.fandom.com/wiki/Area_of_effect), [projectiles](https://pathofexile.fandom.com/wiki/Projectile)) and, for contrast, Diablo IV's damage buckets and affix counts ([damage buckets](https://mobalytics.gg/blog/diablo-4/damage-buckets-deep-dive/), [Season 4 itemization](https://news.blizzard.com/en-gb/article/24077223/galvanize-your-legend-in-season-4-loot-reborn)).

| Mechanic | Studied rule | Here |
| --- | --- | --- |
| Stacking | Flat added; increased summed; more multiplied | Taken: the kit's formula already is this |
| Buckets | The contrast game shipped several multiplicative buckets, then folded most into one additive bucket | Two buckets only (increased, more), so `explain` reads as one line per bucket |
| Scope | Modifiers apply by skill tags, damage types and the weapon | Taken, as tag filters (§5) |
| Conversion | Before scaling, in a fixed type order, never backwards; converted damage scaled by both types | Taken, in one pass with no chains, the kit's rule; history kept per part (§6.2) |
| Extra damage | Gained as extra from a type, before conversion in the studied game | After conversion here, from every part carrying the type; totals match, since conversion keeps totals |
| Area | Area scales; the radius by its square root | Taken |
| Projectiles | Pierce, then fork, then chain, one behaviour per hit | Taken, with a bound on a fork's family |
| Crit | Chance × increased, capped at 1; a multiplier on hits | Taken; damage over time never crits |
| Affixes | Prefix and suffix capacity by rarity; one per group; tiers gated by item level; weights by base tags; values rolled in a tier | Taken |
| Affix counts and strength | The contrast game fixes counts by rarity and scales values by item power; a rarer tier rolls 1.5× | Tiers by item level instead; counts drawn by weight |
| Uniques | Fixed lines with roll ranges | Taken, with named handlers for new mechanics |

## 3. Architecture

### 3.1 Layers

```text
core        Tags, data kinds, RNG streams, saves                 (unchanged)
kit
  foundation   Stats with scoped lines; contexts; primary stats; world sources; why-records
  intent       Skill sheets: tags, levels, bases, skill-local lines; use time; the snapshot
  damage       Crit; conversion with history; extra; resist floors; damage taken; DoT from its source
  overlap      Projectile count, pierce, chain, fork, family bound      (with 8.2's payloads)
  items        Item instances, local and global lines, the equipment status (8.1's domain)
  build        Supports, passive trees, affix generation, uniques, monster rarity (new, 8.7)
umbrella    explain stat | hit, on 7.2's describe and run --script
```

One stat resolution and one damage pipeline serve every genre. A platformer that never writes a scoped line, a sheet or a crit chance pays one branch per read (§17, T8).

### 3.2 Plugins

| Plugin | Domain | Opt-in | Adds |
| --- | --- | --- | --- |
| `StatsPlugin` (extended) | foundation | as today | Scoped lines, `per` lines on primary stats, world sources |
| `HealthPlugin` (extended) | damage | as today | Crit from the source, conversion at request building, resist floors, damage taken |
| `SkillSheetPlugin` | intent | yes | Skill sheets, use time, the snapshot on payloads |
| `ProjectilePlugin` (extended) | overlap | as today | Count, chain and fork read from a snapshot |
| 8.1's items plugin | items | yes | Item instances, the equipment status |
| `SupportsPlugin`, `PassivesPlugin` | build | yes | Links and allocation as line sources |
| `ItemGenPlugin`, `MonsterRarityPlugin` | build | yes | Rolls on the `affixes`, `rarities`, `uniques` and `monster_rarities` kinds |

### 3.3 Where each piece runs

The kit's phases (`crates/tungsten-kit/src/foundation/phases.rs:49-55`): `Tick`, `Intent`, `Move`, `Overlap`, `Resolve`, `React`, then `Present`.

| Phase | ARPG work |
| --- | --- |
| `Tick` | Sources added or removed bump the holder's line revision; entity stats recompute (`stats_apply`); statuses tick their snapshotted DoT |
| `Intent` | A use resolves its skill sheet if stale, pays the cost, starts the wind-up; at the wind-up's end the payload spawns with the snapshot |
| `Move` | Deliveries move |
| `Overlap` | A delivery meets a hurtbox: the request is built (conversion and extra are already in the snapshot); pierce, fork or chain is decided |
| `Resolve` | In the pipeline's sorted order (target, then source): the range and crit draws, deferred lines, resist with floor and penetration, damage taken, shield, health; then the chemistry phase builds statuses from the landed parts |
| `React` | Cues; drops roll items (`ItemGenPlugin`) |

### 3.4 Sources

Every line belongs to a source, a string key, as `Stats` keys modifiers today (`kit: tungsten_kit::Stats::remove_modifiers`). Removing a source removes all its lines.

| Source | Key | Lines reach |
| --- | --- | --- |
| The entity's own data | `base` | The entity |
| A status (an effect today) | `status#<serial>` | The holder |
| An equipped item | `item:<uid>` | The wearer (global lines); the item itself (local lines) |
| A passive node | `passive:<node>` | The entity |
| A support | `support:<id>` | One skill only (skill-local) |
| A skill's own level | `skill:<id>` | One skill only |
| A monster rarity or affix | `rarity:<id>` | The monster |
| A world source (difficulty, area modifiers) | a name the game chooses, e.g. `difficulty` | Every entity its filter passes, live |

### 3.5 Snapshot

A use resolves the skill's sheet once and copies what its deliveries need into a `Delivery` component on each spawned projectile, area or armed hitbox: per damage part its range, type and history, its use-time sums on both tracks and its penetration; the crit sums, each with its winning override if any; the counts; the context tags; the holder's tags; the deferred lines with their `per` coefficients frozen (§5.4); the holder's entity for credit. The payload adds it to the spawn as it adds the `linear_motion` today; core's `derive_from_source` (`crates/tungsten-core/src/ecs/world.rs:174-178`) is no fit, since it knows the source but not which skill spawned. A delivery never reads its user again, so a user who dies, swaps a weapon or loses a buff mid-flight changes nothing in flight (§17, T1).

### 3.6 Extension points

Anything the line vocabulary and the fixed outputs cannot say calls a game's named Rust handler, as the redo's §5.2 chemistry does: a unique's or support's `handler: "name"` with parameters, registered by `ItemGenPlugin::with_handler` or `SupportsPlugin::with_handler`, called on a fixed hook (on use, on hit, on kill, on chain, on equip). The load refuses an unregistered name.

## 4. Data models

Provisional shapes in the kit's data style; each step's plan settles fields and names.

### 4.1 A line

Today's `stat_modifier` (`comp: shape stat_modifier: A modifier on one of the target's stats.`) gains three filters and one scaling, and five siblings for what is not a stat:

```json
{ "stat": "damage", "modifier": { "pct": 0.3 }, "with": { "any": ["damage.fire"] } }
{ "stat": "damage", "modifier": { "mul": 1.2 }, "against": { "all": ["status.chilled"] } }
{ "stat": "crit_chance", "modifier": { "pct": 0.5 }, "while": { "all": ["status.focused"] } }
{ "stat": "max_health", "modifier": { "add": 2 }, "per": { "stat": "vigour", "step": 1 } }
{ "added": { "damage.fire": [4, 6] }, "with": { "all": ["skill.attack"] } }
{ "convert": { "from": "damage.fire", "to": "damage.cold", "pct": 0.5 } }
{ "extra": { "from": "damage.physical", "to": "damage.cold", "pct": 0.25 } }
{ "pen": { "damage.fire": 0.1 }, "with": { "all": ["skill.spell"] } }
{ "resist": "damage.cold", "modifier": { "add": -0.1 }, "floor": { "add": -0.05 } }
```

- `with`: a tag filter over the read's context (§5.1). `while`: over the holder's own tags. `against`: over the other party's tags at the hit (the target's for an attacker's line, the attacker's for a defender's).
- `per`: the modifier's value times `floor(value of a primary stat / step)`.
- `added` is flat typed damage with a range; `convert` and `pen` are what `conversion` and `penetration` hold today, as lines from any source; `extra` is new (AR12).
- `resist` is today's `resist_modifier` (`comp: shape resist_modifier: A modifier on the target's resistance to one damage tag.`), with an optional `floor`, the lowest its factor goes (§6.4).
- `grant` (tags) and `handler` complete the vocabulary. Nothing else; no expressions.

### 4.2 A skill

An `abilities` entry (`comp: shape ability: What an entity can do: fired by a trigger, at a cost, with a cooldown and charges, at a target.`) gains `tags` and a `skill` block:

```json
"cinder_volley": {
  "trigger": { "action": "skill_1" },
  "tags": ["skill.spell", "skill.projectile", "skill.fire"],
  "skill": {
    "bases": { "damage": { "damage.fire": [16, 24] }, "use_time": 0.8, "windup": 0.5,
               "crit_chance": 0.06, "projectile_speed": 600, "cost": 12 },
    "levels": { "damage": "cinder_damage_by_level", "cost": "cinder_cost_by_level" },
    "lines": []
  },
  "payload": [{ "volley": { "prefab": "cinder_bolt", "arc": 24 } }]
}
```

- `bases`: the sheet's base values; `levels` names a curve per base, read at the skill's level, a range keeping its `bases` spread; `lines`: skill-local lines.
- An attack takes its damage parts, `use_time` and `crit_chance` from the wielder's main-hand item (§4.6) times its own `weapon_share`, plus its own `bases`.
- The owner's entry in an entity's `abilities` gains `level` and `supports`: `{ "id": "cinder_volley", "level": 10, "supports": ["threefold", "ricochet", "rimecraft"] }`.

### 4.3 Sheet stats

Known to the kit once `SkillSheetPlugin` is built, bounded in the `stats` kind like any stat. `area` and `projectile_speed` exist today and are read by nothing (`crates/tungsten-kit/src/foundation/stats.rs:277`, `:284`).

| Stat | Base | Read | Bounds | Notes |
| --- | --- | --- | --- | --- |
| `skill_level` | the owner's `level` | first | 1–40 | `add` lines only |
| `damage` | each part's amount | per part | ≥ 0 | `pct` and `mul` only; flat damage is `added` |
| `use_speed` | 1 | at use | 0.1–10 | One stat; attack and cast speed are its lines scoped `skill.attack` and `skill.spell` |
| `crit_chance` | skill or weapon | at use, deferred lines at hit | 0–1 | |
| `crit_multiplier` | the entity's base, e.g. 1.5 | at use, deferred at hit | ≥ 1 | |
| `area` | 1 | at use | 0.1–10 | Radius × √area |
| `projectile_speed` | skill | at use | ≥ 0 | |
| `projectile_count`, `pierce`, `chain`, `fork`, `repeats` | 1, 0, 0, 0, 0 | at use | 1–32, 0–16, 0–16, 0–3, 0–8 | Floored after resolution |
| `duration` | skill | at use | ≥ 0 | Statuses and areas the skill makes |
| `status_effect` | 1 | at use | ≥ 0 | The scale of statuses the skill applies |
| `cost`, `cooldown` | skill | at use | ≥ 0 | |
| `damage_taken` | 1 | the defender's, at hit | ≥ 0 | |

### 4.4 A support

```json
"threefold": {
  "supports": { "all": ["skill.projectile"] },
  "group": "volley",
  "adds_tags": [],
  "lines": [
    { "stat": "projectile_count", "modifier": { "add": 2 } },
    { "stat": "damage", "modifier": { "mul": 0.8 } },
    { "stat": "cost", "modifier": { "mul": 1.3 } }
  ]
}
```

`supports` is checked against the skill's own tags; a second support of the same `group` on one skill is inert; `adds_tags` reach line scopes, never another support's check (§5.3).

### 4.5 A passive tree

```json
"main": {
  "start": ["root"],
  "nodes": {
    "spellcraft": { "links": ["root", "focus_1"],
                    "lines": [{ "stat": "damage", "modifier": { "pct": 0.4 }, "with": { "all": ["skill.spell"] } }] }
  }
}
```

The entity's `passives` component: `{ "tree": "main", "allocated": ["root", "spellcraft"], "points": "passive_points" }`. A node is allocated next to an allocated node or a start while the counter's points last; a refund keeps every allocated node connected. Positions for the screen are 7.20's file, not the kit's.

### 4.6 Items

8.1's item instance, which this design needs to hold rolled lines, not only a prefab:

```json
{ "uid": 9137, "base": "ashwood_wand", "level": 42, "rarity": "rare", "seed": "0x9e3779b97f4a7c15",
  "affixes": [ { "id": "searing", "tier": 2, "values": [22] },
               { "id": "quickened", "tier": 2, "values": [11] },
               { "id": "spellwoven", "tier": 2, "values": [26] },
               { "id": "warded_fire", "tier": 3, "values": [19] } ],
  "name": ["Ember", "Coil"] }
```

An `item_bases` entry: `tags` (`item`, `item.weapon`, `item.wand`, `item.one_hand`), `slot`, `size`, `drop_level`, `weight`, `requires` (8.1's conditions: `vigour ≥ 20`), `implicit` lines, and a `weapon` block (`damage` parts, `use_time`, `crit_chance`) or none. A line marked `local` changes the item's own block when the item is built and is refused on a base without one; the rest are global, applied to the wearer by the equipment status.

### 4.7 Affixes, rarities, uniques, monster rarities (`build`)

```json
"searing": {
  "kind": "prefix", "group": "fire_damage_pct",
  "weights": [ { "tag": "item.wand", "weight": 1000 }, { "tag": "item.ring", "weight": 500 } ],
  "tiers": [ { "level": 1, "weight": 1000, "values": [[10, 16]] },
             { "level": 20, "weight": 600, "values": [[17, 24]] },
             { "level": 46, "weight": 300, "values": [[25, 32]] } ],
  "lines": [ { "stat": "damage", "modifier": { "pct": { "value": 0, "times": 0.01 } },
               "with": { "any": ["damage.fire"] } } ]
}
```

- `weights`: the first entry whose tag the base carries gives the affix's weight; none, or 0, excludes it. `tiers` are listed from the lowest, tier 1 the lowest. A line's number may be `{ "value": i, "times": k }`, the `i`-th rolled value times `k`.
- `rarities`: `{ "rare": { "weight": 20, "counts": [[3, 40], [4, 30], [5, 20], [6, 10]], "prefixes": 3, "suffixes": 3, "names": "rare_words" } }`.
- `uniques`: `{ "cinderwake_band": { "base": "iron_band", "level": 30, "weight": 10, "lines": […with ranges], "handler": { "name": "burning_wake", "seconds": 1, "radius": 32 } } }`.
- `monster_rarities`: `{ "rare": { "weight": 8, "counts": [[2, 1]], "lines": [{ "stat": "max_health", "modifier": { "mul": 2.5 } }] } }`, its affixes drawn from `affixes` whose weights name monster tags.

## 5. Modifier applicability and stacking

### 5.1 Contexts

A read of a stat has a context, a set of tags, each carrying its dotted ancestors as `Tags` does (`kit: tungsten_kit::Tags::has`):

- **Skill context `C`:** the skill's `tags` (`skill.*`), the main-hand item's base tags for an attack (`item.*`), and the tags the skill's applicable supports add.
- **Track tag:** `delivery.hit` for the hit track, `delivery.dot` for the damage-over-time track; each sheet resolves both tracks for damage.
- **Part tags:** for a read of `damage`, `pen` or `damage_taken` on one part, the part's type and its history (§6.2), `damage.*`.
- **State tags,** known only at the hit: `delivery.pierced`, `delivery.forked`, `delivery.chained`, `delivery.repeated`.

`skill.*` and `damage.*` never mix: a skill tagged `skill.fire` makes "with fire skills" lines apply to its crit chance and cost, and only a part typed or descended from fire takes "fire damage" lines. A fire skill's added cold part takes no fire line (§17, T6).

An entity-level read (`Stats::get`, `kit: tungsten_kit::Stats::get`), which movers, health and cooldowns make, has the empty context: only lines without `with` and `against` reach it, with `while` read against the holder's tags.

### 5.2 Which lines apply

A line applies to a read when:

1. its stat is the read's stat (or its kind matches: `added`, `convert`, `extra`, `pen`, `resist`);
2. its `with` filter passes the read's context, by `TagFilter`'s rule (`kit: tungsten_kit::TagFilter::matches`);
3. its `while` filter passes the holder's tags at the read;
4. its `against` filter passes the other party's tags (at the hit only);
5. its source reaches the read: skill-local sources (`support:`, `skill:`) only that skill's sheet; `item:` local lines only the item's block.

A support applies to a skill when its `supports` filter passes the skill's own `C` (its `tags` and the weapon's), without any support's `adds_tags`, and no earlier support in the list shares its `group`. An inert support's lines reach nothing, and `explain` says why.

### 5.3 Stacking

Per read, the applying lines combine as today's `modified` does (`crates/tungsten-kit/src/foundation/stats.rs:96`): `value = (base + Σadd) × (1 + Σpct) × Πmul`, then clamped to the stat's bounds.

- `per` lines multiply their value by `floor(primary / step)` first. A primary stat (declared `primary` in the `stats` kind) resolves before the rest, with its own lines and none of `per`, so no read waits on itself.
- **Override:** unscoped overrides keep today's last-added rule; a scoped line may not override, and the load refuses one (§17, T9). A read finished at the hit applies the winning override after the deferred terms, as `modified` does, so a snapshot keeps it beside the sums (§5.4).
- **Counts** (`projectile_count`, `pierce`, `chain`, `fork`, `repeats`) are floored after clamping.
- **Damage:** a part's factor is `(1 + Σpct) × Πmul` over the lines that apply to that part, and a factor below 0 counts as 0. `added` lines give parts, not factors (§6.2, step 5).
- **Penetration** sums per type and caps at 1. **Conversion** shares sum per part over every line whose `from` is the part's type or an ancestor of it, and a part's total past 1 normalizes, as today (`comp: one part's shares past 1 normalize`), in one pass (`comp: a share never converted again`). **Extra** shares sum per `(from, to)`.
- **Order of sums.** Lines sum in `f64` in the order of their source keys, then their index, so a result never depends on equip or apply order.

### 5.4 When each line is read

- **At use (the snapshot):** every line that has no `against` filter and whose `with` names no state tag, in the sheet's `C`, with `while` read against the holder's tags at the use.
- **Deferred to the hit:** a line with an `against` filter, or whose `with` names a state tag in `all`, `any` or `none`. Only `damage`, `crit_chance`, `crit_multiplier` and `pen` lines may be deferred; the load refuses a deferred line on any other stat, since counts, area and speeds are spent at the spawn.
- The snapshot keeps the use-time partial sums `(base + Σadd, Σpct, Πmul)` and the winning override, if any, for each deferred-capable read, and the deferred lines themselves, each `per` coefficient resolved at use and frozen into its copy, so a hit never reads its user (§3.5). The hit adds the deferred lines' terms to the sums and finishes the formula, an override still winning. Increased lines therefore add into the same sum whether they were read at use or at the hit.
- **The defender's lines** (`resist`, `damage_taken`) are read live at the hit.

### 5.5 Caching

An ability caches its sheet under the holder's line revision, the holder's tag revision, its links and its level; a use resolves again only when one changed. Entity stats recompute in `Tick` as today.

## 6. Order of operations

### 6.1 Sources change (`Tick`)

1. A source is added or removed (an equip, a status applied or ended, a node allocated, a link changed, a world source set): its lines join or leave the holder's line set; the line revision bumps.
2. Primary stats resolve, then entity stats (`stats_apply` writes them into movers, health and cooldowns, as today).
3. Every cached sheet of the holder is stale.

### 6.2 A use (`Intent`)

1. **Context.** `C` from the skill's tags and the main-hand item's base tags for an attack; supports applicable by §5.2; their `adds_tags` join `C`.
2. **Level.** `skill_level` = the owner's level + `add` lines in `C`; `bases` read at that level through `levels`.
3. **Sheet stats.** Each stat of §4.3 but `damage` resolves at use by §5.3; counts floored.
4. **Cost and time.** The cost is paid (`cost`); the wind-up is `use_time × windup / use_speed` and the recovery `use_time × (1 − windup) / use_speed`; nothing else of the user's fires until the recovery ends.
5. **Parts.** For each damage type `T`: `[min, max]` = the skill's base for `T` (an attack: the weapon's local parts × `weapon_share`, plus the skill's own) + Σ `added` lines for `T` in `C`. A part of `[0, 0]` drops. Each part has its type and an empty history.
6. **Conversion,** one pass: for each part, the conversion lines whose `from` is its type or an ancestor of it (§5.3, normalized at 1) make new parts of type `to` with history = the part's history ∪ its type; the remainder stays. No converted share converts again.
7. **Extra damage,** one pass over the parts step 6 left: for each `extra` line `(from, to, pct)` and each part whose type or history carries `from`, a new part of `to`, `pct` × that part, with history = that part's history ∪ its type. Extra parts make no extra.
8. **Merge** parts of equal type and equal history, in first-appearance order.
9. **Use-time sums,** per part, per track: Σpct and Πmul of the `damage` lines that apply in `C` ∪ the track tag ∪ the part's type and history; and per part the penetration of its type, read in `C` ∪ `delivery.hit` ∪ its type and history, so a cold part converted from fire and a native one may penetrate differently.
10. **Snapshot** (§3.5) at the wind-up's end; the payload spawns `projectile_count` deliveries in an arc, or one area of radius × √`area`, or arms the user's hitbox with it; `repeats` more payloads follow at `repeat_interval`, each with `delivery.repeated`.

### 6.3 A delivery meets a hurtbox (`Overlap`)

1. The hit is queued as a request carrying the delivery's snapshot and its state tags; a family of deliveries hits one target once, as `Hitbox::set_hit_targets` keeps a hitbox's targets (`kit: tungsten_kit::Hitbox::set_hit_targets`).
2. One behaviour, in this order: **pierce** while `pierced` is below the snapshot's `pierce` (the delivery goes on, `pierced` +1, tagged `delivery.pierced` from then on); else **fork** if any left (two children at ±`fork_angle`, each with one fork fewer and the family's hit list, tagged `delivery.forked`; the parent ends); else **chain** if any left and a target the family has not hit is within `chain_range` (turn toward it, `chain` −1, tagged `delivery.chained`); else end.
3. A family holds at most 64 deliveries alive; a fork past it ends the parent without children, counted in the trace.

### 6.4 The request resolves (`Resolve`, in the pipeline's sorted order: target, then source)

1. **Deferred lines** fold into each part's sums, with the target's tags for `against` and the state tags for `with`.
2. **Range draw:** one draw `u` from the kit's stream (`kit: tungsten_kit::KitRng::unit`); each part's amount = `min + u × (max − min)`, times `(1 + Σpct) × Πmul`.
3. **Crit draw:** a second draw; below `crit_chance` (clamped to 1), every part times `crit_multiplier`, and the request is a crit (`kit: tungsten_kit::DamageRequest::with_crit`). A request with no sheet takes the source's entity-level `crit_chance` and `crit_multiplier`, and draws only when the chance is above 0.
4. **The kit's pipeline,** amended: parts; conversion only for a request not built from a sheet, as today's step 2, since a sheet's request was converted at use (§6.2); then per part, each of the target's matching factors as today (`comp: component resist: Damage factors by damage tag: a hit part's matching factors multiply its amount, and a factor of 0 is immunity.`, `crates/tungsten-kit/src/damage/pipeline.rs:126-133`), with one step first: 0 stays immunity; a factor above 0 rises to its tag's floor; a factor below 1 then gains the part's penetration, capped at 1; the factors multiply the part. With no floor this is today's arithmetic, and the floor comes before penetration, so penetration works past a capped resistance.
5. **Damage taken:** the target's `damage_taken` read in `C` ∪ the track and state tags ∪ the part's type and history, `against` the attacker's tags from the snapshot; the part × it.
6. **Shield, health, events,** as today's steps 5–8 (`crates/tungsten-kit/src/damage/pipeline.rs:134-148`).
7. **Statuses from the hit,** in the chemistry phase after the damage: each landed part is a stimulus of its type. A rule whose output is a damage-over-time status gives it, per second, a share of the part's rolled amount from step 2 *before* the factor and crit, times the snapshot's `delivery.dot` sums with their deferred lines, times the status's `status_effect`. That rate is fixed at the application (§17, T1).

### 6.5 A status ticks (`Tick`)

A periodic request (`kit: tungsten_kit::DamageRequest::with_periodic`) of the snapshotted rate: no range or crit draw, no penetration; the factor with its floor, then `damage_taken` with `delivery.dot`; shield and health. Its source is the holder that applied it.

### 6.6 What a step draws

Per request with a sheet: two draws in the pipeline's sorted order. Per item rolled: one draw for its seed (§7). Every other draw happens where the kit draws today, so a game without sheets draws what it drew before, plus a crit draw only where a source has a crit chance.

## 7. Item generation

Run by `ItemGenPlugin` when a drop rolls an item (a `roll_item` cue action beside today's `roll`, `comp: kind loot_tables:`), a game calls it, or a monster rarity draws its affixes.

1. **Seed.** One `u64` from the kit's stream (two `next_u32` draws, `kit: tungsten_kit::KitRng::next_u32`); a stream seeded with it does every later draw for this item (`kit: tungsten_kit::KitRng::seeded`), so rolling an item moves the world's stream by two draws whatever the item holds.
2. **Base,** when not given: by `weight` among the bases whose `drop_level` ≤ the item level and whose tags pass the drop's filter.
3. **Rarity,** when not given: by weight, the killer's `rarity` lines scaling every non-normal weight (`pct`). A unique draws among the uniques of that base with `level` ≤ the item level; with none it rolls rare.
4. **Count:** drawn from the rarity's `counts`.
5. **Affixes,** one at a time until the count, or until nothing is eligible: an affix is eligible when its kind has capacity left, its group is unused, it has a tier with `level` ≤ the item level, and its weight for the base is above 0. Each eligible tier is an entry of weight `affix weight × tier weight / Σ eligible tier weights of that affix`, so a new tier changes how an affix's tiers split, never how often the affix comes. Entries sort by affix ID, then tier; one draw picks one.
6. **Values:** each `[min, max]` uniform in whole steps (`step` 1 unless the affix says): `min + floor(u × ((max − min) / step + 1)) × step`.
7. **Name:** rares from the rarity's word lists; magic items from their affixes' words; both seeded.
8. **Saved** in full (§4.6): base, level, rarity, seed, each affix's ID, tier and values. A load never rolls again, so a change to the affix tables changes no saved item (§17, T5).

A monster's rarity draws the same way with the monster's tags as the base's and its level as the item level.

`tungsten check` validates the kinds: every affix with a tier, tiers ascending by level, `min ≤ max`, each weight tag declared, each line's stat known, `local` lines only on bases with a block, each unique's base existing, each handler registered; and it warns for a base with no prefix or no suffix eligible at its highest drop level.

## 8. Shared rules for players and enemies

Every entity with `stats` and `abilities` resolves through §5 and §6; no rule asks whether it is a player. Players and monsters differ in which sources they carry:

| Source | Player | Monster |
| --- | --- | --- |
| Base stats and primary stats | `stats` data | `stats` data, bases from curves by level (`comp: kind curves:`) |
| Skills and supports | Chosen, linked by the player | Fixed in the prefab's `abilities` |
| Passives | Allocated | None |
| Items | Equipped | Optional, from the prefab |
| Rarity | None | `monster_rarities` and their affixes |
| World sources | When their filter passes | When their filter passes |

Exceptions, each deliberate:

- **E1. Who picks a skill:** input for the player, 8.2's AI for monsters. Both use through `use_ability`.
- **E2. Who drops and who collects:** monsters' `killed` cues roll items; the player's `pickup` filter collects them. Components, not rules.

01's three cases (the brief's test of the rule):

| 01 today | Under these rules | Exceptions |
| --- | --- | --- |
| The player's hits crit 5 % for 2× (`examples/01_platformer/src/features/combat/mod.rs:38-39`, rolled at `:198`, `D-200`) | The player's `stats` hold `crit_chance` 0.05 and `crit_multiplier` 2; enemies hold none, so they draw nothing (§6.4, step 3) | 0 |
| Blasts halved on the player (`PLAYER_BLAST_SHARE`, `:41`, applied at `:208-209`) | The player's `damage_taken` line `{ "modifier": { "mul": 0.5 }, "with": { "all": ["hit.blast"] } }`, its blasts tagged `hit.blast` | 0 |
| Difficulty scales enemies only (`examples/01_platformer/src/features/enemies/mod.rs:366-386`, `D-201`, `D-212`) | A world source `difficulty` with filter `{ "all": ["team.enemy"] }` and `mul` lines on `max_health` and `damage`, set once from the setting | 0 |

## 9. Worked examples

Damage types `damage.physical`, `damage.fire`, `damage.cold`. Every character's `crit_multiplier` base is 1.5. Averages unless a range is shown; §19 lists the script that checked the arithmetic.

### 9.1 A skill with three supports

The caster: Cinder Volley (§4.2) at level 10, base fire 16–24, linked to `threefold` (§4.4), `ricochet` (`supports` projectile skills; `chain` +2; `damage` `mul` 0.85 `with` `delivery.chained`; `cost` `mul` 1.2) and `rimecraft` (`supports` `skill.fire`; `convert` fire → cold 0.5; `cost` `mul` 1.1). They wear §9.2's wand.

| Source | Line | Fire part | Cold part | Read |
| --- | --- | --- | --- | --- |
| `passive:spellcraft` | `damage` pct 0.40 with `skill.spell` | ✓ | ✓ | use |
| the wand's implicit | `damage` pct 0.08 with `skill.spell` | ✓ | ✓ | use |
| `item:` `spellwoven` | `damage` pct 0.26 with `skill.spell` | ✓ | ✓ | use |
| `item:` `searing` | `damage` pct 0.22 with any `damage.fire` | ✓ | ✓ by history | use |
| `passive:frostcraft` | `damage` pct 0.20 with any `damage.cold` | — | ✓ | use |
| `passive:marksman` | `damage` pct 0.25 with `skill.projectile` | ✓ | ✓ | use |
| `passive:brawler` | `damage` pct 0.30 with `skill.melee` | — | — | skipped: `skill.melee` not in `C` |
| `support:threefold` | `damage` mul 0.8 | ✓ | ✓ | use |
| `status#41` (kindled) | `damage` mul 1.1 with any `damage.fire` | ✓ | ✓ by history | use |
| `passive:cold_snap` | `damage` pct 0.20 against `status.chilled` | ✓ | ✓ | hit |
| `support:ricochet` | `damage` mul 0.85 with `delivery.chained` | chained only | chained only | hit |
| `passive:precision` | `crit_chance` pct 0.5 | | | use |
| `passive:deadeye` | `crit_multiplier` add 0.4 | | | use |
| `item:` `quickened` | `use_speed` pct 0.11 with `skill.spell` | | | use |
| `passive:fletcher` | `projectile_speed` pct 0.15 with `skill.projectile` | | | use |
| `item:` a ring | `pen` fire 0.10 | | | use |

At use: fire 16–24 converts half to cold: a fire part 8–12 and a cold part 8–12 with history `{damage.fire}`. Fire's Σpct is 0.74 + 0.22 + 0.25 = 1.21; cold's is 0.74 + 0.22 + 0.20 + 0.25 = 1.41; both Πmul 0.8 × 1.1 = 0.88. Sheet: 3 projectiles at 690 px/s, chain 2, crit chance 0.06 × 1.5 = 0.09, crit multiplier 1.9, use time 0.8 / 1.11 = 0.721 s, cost 12 × 1.3 × 1.2 × 1.1 = 20.59.

A first hit on a chilled fen brute (fire factor 0.6, cold 0.75, no floors):

| Part | Base | × (1 + Σpct) with chilled | × Πmul | Before defence | × factor | After |
| --- | --- | --- | --- | --- | --- | --- |
| Fire | 10 | 2.41 | 0.88 | 21.21 | 0.6 + 0.10 pen = 0.70 | 14.85 |
| Cold | 10 | 2.61 | 0.88 | 22.97 | 0.75 | 17.23 |
| Hit | | | | 44.18 | | **32.07**; 60.94 on a crit |

The range runs 25.66–38.49 for the same hit. A chained hit is × 0.85: 27.26. Each projectile hits up to three times (one and two chains), so a cast that lands every hit deals 3 × (32.07 + 2 × 27.26) = 259.8, times the crit's expected factor 1 + 0.09 × 0.9 = 1.081: **280.8 a cast, 389.6 a second.**

Worn with §9.2's unique ring, its 18 % of cold as extra fire adds a fire part of 1.8 with history `{damage.fire, damage.cold}`, which every line that reaches the cold part reaches too: 1.8 × 2.61 × 0.88 × 0.70 = 2.89 more on that hit. Its `chain` +1 makes four hits a projectile.

`explain hit` for the cold part of that hit:

```text
cold part  10.00   from fire 16–24 at u 0.50 → 20.00, converted 0.5 (support:rimecraft)
  pct  +0.40 passive:spellcraft   with skill.spell              use
  pct  +0.08 item:9137 implicit   with skill.spell              use
  pct  +0.26 item:9137 spellwoven with skill.spell              use
  pct  +0.22 item:9137 searing    with damage.fire              use (history)
  pct  +0.20 passive:frostcraft   with damage.cold              use
  pct  +0.25 passive:marksman     with skill.projectile         use
  pct  +0.20 passive:cold_snap    against status.chilled        hit
  pct   0.30 passive:brawler      with skill.melee              skipped: skill.melee not in context
  mul  ×0.80 support:threefold                                  use
  mul  ×1.10 status#41            with damage.fire              use (history)
  mul  ×0.85 support:ricochet     with delivery.chained         skipped: delivery.chained not set
  = 10.00 × 2.61 × 0.88 = 22.97; no crit (draw 0.31 ≥ 0.09)
  resist  damage.cold 0.75 (no floor; no penetration for cold) → 17.23
  damage_taken 1.00 → 17.23
```

### 9.2 A rolled rare item and a unique

**The wand.** Base `ashwood_wand` (tags `item`, `item.weapon`, `item.wand`, `item.one_hand`; implicit `damage` pct 0.08 with `skill.spell`), item level 42, rarity rare, count 4 from `[[3, 40], [4, 30], [5, 20], [6, 10]]`. The pool for this example:

| Affix | Kind | Weight for a wand | Tiers (level: weight, range) | Entries at level 42 |
| --- | --- | --- | --- | --- |
| `searing` (fire damage %) | prefix | 1000 (`item.wand`) | 1: 1000, 10–16; 20: 600, 17–24; 46: 300, 25–32 | t1 625, t2 375 |
| `spellwoven` (spell damage %) | prefix | 1000 | 1: 1000, 10–19; 24: 600, 20–29; 50: 300, 30–39 | t1 625, t2 375 |
| `honed` (local added physical) | prefix | 0 (`item.wand` listed first at 0, before `item.weapon` 1000) | 1, 22, 44 | none |
| `quickened` (cast speed %) | suffix | 800 | 1: 1000, 5–9; 25: 600, 10–14; 55: 300, 15–19 | t1 500, t2 300 |
| `warded_fire` (fire resistance) | suffix | 1000 (`item`) | 1: 1000, 6–11; 18: 800, 12–17; 36: 500, 18–23 | t1 434.78, t2 347.83, t3 217.39 |
| `keen` (crit chance %) | suffix | 600 (`item.weapon`) | 1: 1000, 10–19; 30: 600, 20–29; 60: 300, 30–39 | t1 375, t2 225 |

| Draw | Pool weight | Drawn | Its chance | Value |
| --- | --- | --- | --- | --- |
| 1 | 4,400 | `searing` t2 | 375 / 4,400 = 8.5 % | 22 (17–24) |
| 2 | 3,400 (`searing`'s group gone) | `quickened` t2 | 300 / 3,400 = 8.8 % | 11 (10–14) |
| 3 | 2,600 | `spellwoven` t2 | 375 / 2,600 = 14.4 % | 26 (20–29) |
| 4 | 1,600 | `warded_fire` t3 | 217.39 / 1,600 = 13.6 % | 19 (18–23) |

Saved as §4.6 shows. Its wearer gains `damage` pct 0.22 with fire, 0.26 with spells, `use_speed` 0.11 with spells, and a fire `resist` `add` −0.19.

**The unique.** Cinderwake Band (`cinderwake_band`): base `iron_band`, level 30. Lines: `chain` add 1 with `skill.projectile`; `extra` cold → fire, pct 15–20 (rolled 18); `resist` cold `add` +0.10 (a drawback: cold hits take 10 % more); handler `burning_wake` (on a chained hit, a 32 px burning patch for 1 s). Saved as `{ "uid": 9140, "unique": "cinderwake_band", "level": 51, "seed": "…", "values": [18] }`. Its handler is the extension point: the patch is a spawn the line vocabulary cannot say, and `explain` names it as the line `handler burning_wake`.

### 9.3 A monster on the same rules

A fen brute, level 38, rare, under the `difficulty` world source at hard (`mul` 1.5 on `max_health` and `damage`, filter `team.enemy`). Its `max_health` base comes from the curve `monster_life` at 38, 410; rare's line `mul` 2.5 and difficulty's 1.5 make it 1,537.5. Its rarity drew two affixes from the same `affixes` kind: `frenzied` (`use_speed` pct 0.30 with any `skill.attack` or `skill.spell`) and `frost_laced` (`extra` physical → cold 0.25).

Its `maul` (`skill.attack`, `skill.melee`; no weapon, so its own base physical 30–40 from `monster_hit` at 38; use time 1.2; crit chance 0.05) against the player:

| Part | Base | × Πmul (difficulty) | Before defence | Player's factor | × `damage_taken` 0.9 | After |
| --- | --- | --- | --- | --- | --- | --- |
| Physical | 35 | 1.5 | 52.5 | 1.0 | 0.9 | 47.25 |
| Cold (extra, history physical) | 8.75 | 1.5 | 13.13 | 0.18 raised to its floor 0.25 | 0.9 | 2.95 |
| Hit | | | 65.63 | | | **50.20**; 75.30 on a crit (5 %, × 1.5) |

The player's cold factor 0.18 (resistance lines adding −0.82) meets the player's floor of 0.25 from `stats` data, which no rule gives players only: a monster with a floor gets the same clamp. The player's `braced` status gives `damage_taken` pct −0.10; the player's blast line (`with` `hit.blast`) is skipped, since `maul` is no blast. The maul's use time is 1.2 / 1.3 = 0.92 s.

## 10. Agent surface: `explain`

- **`explain stat <entity> <stat> [--skill <id>] [--against <entity>]`:** the context, each line with its source, kind, value, filter and whether it applied or why not, the partial sums and the clamp. On a live game through 7.2's `describe` channel and `run --script`'s report.
- **`explain hit <trace entry>`:** each stage of §6.4 with its numbers, as §9.1 shows: the part's origin through conversion and extra, each line, the draws, the factor with its floor and penetration, damage taken, shield and health.
- **`explain item <uid>`:** the base, each affix with tier and range, local lines folded into the block, the global lines the equipment status gives.
- **Records:** the kit trace (`kit: tungsten_kit::KitTrace::about`) records events with amounts (`kit: tungsten_kit::TraceKind::Event::amount`), not the lines behind them. A sheet built while the trace is on keeps its line list (source, index, applied or the failing filter); off, it keeps only the sums. The records' shape lands with 7.10's chemistry `explain` (§8 of the redo), and the commands with 8.7.

## 11. Numbers

A reference content set a small action RPG would ship, for sizing the kinds, the benches and the screens.

| Thing | Number |
| --- | --- |
| Damage types | 5 (physical, fire, cold, shock, toxic), with bleed under physical |
| Skills | 36: 24 for the player, 12 monster-only |
| Supports | 40, in 12 groups |
| Passive tree | 240 nodes: 60 notable (2–3 lines), 180 small (1 line); 90 allocated at the level cap of 60 |
| Item bases | 48: 24 weapons (12 kinds, 2 each), 24 armour and jewellery; 11 equipment slots |
| Affixes | 90 (45 prefixes, 45 suffixes), 4–6 tiers each: about 440 tier entries; 20 more for monsters |
| Uniques | 30, of which 8 name a handler |
| Rarities | Items: normal, magic (1–2 affixes, 1 + 1), rare (3–6, 3 + 3), unique; monsters: normal, magic (1), rare (2), unique |
| Statuses | 18: 6 ailments, 6 buffs, 4 curses, 2 world (difficulty, area) |
| Lines on the player | About 330: passives 120, items 55 global, statuses 30, base 20, world 5, and 100 skill-local across 8 linked skills; a sheet reads about 240 (the 230 entity lines and its own 12) |
| Lines on a monster | Normal 8, magic 14, rare 24, unique 40 |
| Monsters | 120 in view, 300 awake in an area; packs of 6–12 with a rare in one of three |
| Deliveries | 400 alive at a peak: the player's volleys, forks and chains, and monster projectiles |
| Hits | About 385 a second at a peak (§13's `arpg.volley_storm`); 3,600 periodic ticks a second from 300 monsters with 3 DoTs each |
| Items on the ground | 150 labels in play, 400 after a boss |
| Items saved | About 500: 60 inventory cells and 4 stash tabs of 144 |
| Screens | Skills and links, passive tree, inventory and equipment, stash, item tooltip, `explain` overlay (development builds) |
| A save | Level, primary stats, 90 nodes, 8 skills with levels and links, about 500 items in full (about 200 bytes each), counters, the area's seed and changes: under 200 KB of JSON |

## 12. Five weekly agent tasks

For 8.6's evaluation on a `tungsten new` copy. Each names the rows it runs into today.

1. **A new skill.** "Add Hoarfrost Lance: a cold projectile spell, levels 1–20 by curve, pierce 2, chilling what it hits; tags spell, projectile, cold." Done when a scripted run casts it at levels 1 and 20, `explain hit` shows the curve's base and both pierces, and `tungsten check` passes. Runs into PA-1, PA-2, PA-8, PA-11, PA-14, PA-22.
2. **A new support.** "Add Splinter: supports projectile skills, forks once, 20 % less damage, cost × 1.2, in the volley group." Done when, linked to Cinder Volley, a run shows forked hits tagged `delivery.forked`, and linked to a melee skill `explain` shows it inert with its reason. Runs into PA-2, PA-15, PA-17, PA-22.
3. **A tiered affix.** "Add `kindling`: increased damage over time with fire, five tiers at item levels 1, 15, 30, 45 and 60, on wands, staves and rings only." Done when `check` passes and a test rolling 10,000 seeded rares at level 50 finds it on no amulet and within 10 % of its expected share. Runs into PA-1, PA-8, PA-12, PA-19.
4. **A unique.** "Add Cinderwake Band (§9.2)." Done when it rolls in its range, `explain item` shows each line's source, and the handler's patch spawns on a chained hit only. Runs into PA-6, PA-12, PA-15, PA-20, PA-22.
5. **A balance bug.** "At level 40 the shock build out-damages the fire build threefold; find the lines responsible and bring the gap within 20 %, changing data only." Done when both builds' top skills' `explain` output is in the report with the change, and a scripted run on a training dummy measures both within 20 %. Runs into PA-11, PA-22.

## 13. Stress numbers

Rows for 9.1, each with its budget category and the surfaces it exercises.

| Row | Workload | Category | Surfaces |
| --- | --- | --- | --- |
| `arpg.horde_sheets` | 300 monsters with 24 lines each; 40 statuses applied or ended a second, each staling a sheet; each monster uses a skill every 1.2 s (250 sheet resolutions a second) | Frame | Scoped resolution, the cache |
| `arpg.volley_storm` | The player casting 3 times a second, 5 projectiles × fork 1 × chain 3 (5 × (1 + 2 × 4), up to 45 hits a cast, 135 a second), beside 300 awake monsters using a skill every 1.2 s (250 hits a second) and 120 monster projectiles: about 400 deliveries alive and 385 hits a second with deferred lines | Frame | Overlap, the pipeline's draws and deferred lines |
| `arpg.dot_field` | 300 monsters with 3 damage-over-time statuses ticking each 0.25 s: 3,600 periodic requests a second | Frame | Statuses, the pipeline |
| `arpg.equip_swap` | A weapon swap: the player's 330 lines change source, entity stats and 8 sheets resolve | Operation | Sources, resolution |
| `arpg.boss_drop` | 40 items rolled at level 80 from 440 tier entries and 30 uniques | Operation | Item generation |
| `arpg.ground_labels` | 400 ground items labelled by rarity, laid out without overlap | Frame | UI, labels |
| `arpg.save_stash` | 500 items written and read | Operation | Saves |

## 14. Build sheet

Verdicts as in the [paper designs](paper-designs.md): **data only** (prefabs and data kinds alone), **game Rust** (the engine gives the parts and the game writes the rest, of the size shown), **gap** (a part no item provides, with the game Rust it would take today where a game could write it at all), **blocked** (a rule or the architecture forbids it). Evidence is a span to search for in `api/*.txt` (`core:` `tungsten-core.txt`, `kit:` `tungsten-kit.txt`, `umb:` `tungsten.txt`, `comp:` `tungsten-kit-components.txt`, `cli:` `tungsten-cli.txt`), or a `none:` pattern that matches no line of `api/*.txt`, case-insensitive; other files are named with their line.

- **AR1 · Flat, increased and more · data only.** A `stats` component with `add`, `pct` and `mul` modifiers, by source. Evidence: `comp: component stats:`, `comp: field pct: f32 — added to the percentage factor, which starts at 1`, `comp: field mul: f32 — multiplies the result`, `kit: tungsten_kit::Stats::add_modifier`, `kit: tungsten_kit::Stats::remove_modifiers`; `crates/tungsten-kit/src/foundation/stats.rs:96` (the formula).
- **AR2 · Lines scoped by tags · gap: `with`, `while` and `against`, and reads in a context (7.10); about 600 lines of game Rust today** (a scoped line store per source, a resolver per context, invalidation). A modifier names a stat and a source; a stat has one value per entity. Evidence: `comp: shape stat_modifier: A modifier on one of the target's stats.`, `kit: tungsten_kit::Stats::get`; searched `none: stat_?scope|modifier_?scope|applies_?to|stat_?context`.
- **AR3 · Skills: tags, levels and sheets · gap (7.10); about 500 lines today.** An ability holds a trigger, a cost, a cooldown, charges, a target and payloads; no tags, level or base values. Evidence: `comp: shape ability: What an entity can do: fired by a trigger, at a cost, with a cooldown and charges, at a target.`, `kit: tungsten_kit::Ability::with_payload`; searched `none: skill|ability_?level`.
- **AR4 · A payload carrying its user's values · gap: the snapshot (7.10); about 200 lines today.** The spawn payload adds only a `linear_motion` to the prefab (`crates/tungsten-kit/src/intent/ability.rs:1498`); its hitbox keeps its prefab's damage, and its hits name the user as source (`crates/tungsten-kit/src/overlap/grid.rs:471-473`), so only the user's `penetration` and `conversion` components reach them. Core lets a spawn from a source take a clone of the source's component or a value derived from it; a game registering the user's `Stats` that way gives every bolt the whole stat set, so `stats_apply` writes one `damage` value into every skill's bolts, scoped to no skill. Evidence: `comp: shape spawn_payload: A prefab spawned from the entity at its point.`, `comp: the hit's own penetration, before its source's`, `kit: tungsten_kit::Source::entity`, `core: tungsten_core::World::inherit_from_source`, `core: tungsten_core::World::derive_from_source`; searched `none: snapshot_?stat|caster_?stat|skill_?sheet`.
- **AR5 · Area · gap (7.10); about 60 lines today.** `area` is declared and read by nothing (`crates/tungsten-kit/src/foundation/stats.rs:277`); a hit shape is a fixed circle or box. Evidence: `kit: tungsten_kit::Stats::AREA`, `comp: field circle: f32 (> 0, <= 16384) — the radius, in world pixels`; searched `none: area_?scale|radius_?mult`.
- **AR6 · Projectile speed · gap (7.10); about 30 lines today.** `projectile_speed` is declared and read by nothing (`:284`); the spawn payload's speed is its own field. Evidence: `kit: tungsten_kit::Stats::PROJECTILE_SPEED`, `comp: field speed: f32 (>= 0) = 0.0 — world pixels per second along the direction`; searched `none: speed_?(scale|mult)|stat_?speed`.
- **AR7 · Projectile count · gap (8.2, P8-13); about 120 lines today.** A spawn payload spawns one prefab. Evidence: `comp: shape spawn_payload: A prefab spawned from the entity at its point.`; searched `none: projectile_?count|pellet|\bspread\b`.
- **AR8 · Pierce from the sheet · game Rust (~40 lines: a resolved count written into a projectile the game spawns itself).** A projectile's `pierce` is a fixed field. Evidence: `comp: field pierce: u32 = 0 — targets it passes before the next ends it`, `kit: tungsten_kit::Projectile::with_pierce`.
- **AR9 · Chain and fork · gap (8.2); about 350 lines today.** A projectile ends, pierces, bounces off walls or passes them; nothing turns it toward a target it has not hit or splits it (`steering` seeks the nearest, hit or not). Evidence: `comp: component projectile: Ends at its hitbox's first hit, its first wall or its range, unless it pierces, bounces or passes the wall.`, `kit: tungsten_kit::Projectile::bounces`; searched `none: \bfork|chain_?(count|range|left)|redirect`.
- **AR10 · Penetration · data only.** A hit's own `pen` and its source's `penetration`, summed and capped at 1 (`crates/tungsten-kit/src/damage/pipeline.rs:126-127`). Evidence: `comp: component penetration:`, `kit: tungsten_kit::DamageRequest::with_penetration`. Scoping it by skill is AR2's.
- **AR11 · Conversion before scaling, with history · gap (7.10); about 200 lines today.** Conversion runs inside the pipeline (`crates/tungsten-kit/src/damage/pipeline.rs:123-125`), after anything a game could scale, and a converted part keeps only its new type. Evidence: `comp: component conversion:`, `comp: a share never converted again`, `kit: tungsten_kit::Conversion::with_rule`; searched `none: history`.
- **AR12 · Extra damage · gap (7.10); about 100 lines today.** Evidence: searched `none: extra_?damage|gain(ed)?_?as`.
- **AR13 · Crit chance and multiplier · gap (7.10); about 60 lines today, and none for kit-built hits.** A request carries a crit flag the kit never sets; 01 rolls and doubles in its own request (`examples/01_platformer/src/features/combat/mod.rs:198-199`), which kit hitboxes and projectiles cannot call. Evidence: `kit: tungsten_kit::DamageRequest::with_crit`, `kit: tungsten_kit::DamageRequest::is_crit`; searched `none: crit_?(chance|multi)|critical`.
- **AR14 · Use time, attack and cast speed · gap (7.10, with P7-6); about 250 lines today.** An ability fires at once on its trigger; `cooldown_rate` counts cooldowns only. Evidence: `comp: field cooldown: f32 (>= 0) = 0.0 — seconds between uses, counted at the entity's`, `kit: tungsten_kit::Stats::COOLDOWN_RATE`; searched `none: cast_?(time|speed)|attack_?speed|wind_?up|use_?time`.
- **AR15 · Repeats · gap (8.2, P8-11); about 150 lines today.** A use runs its payload list once; only an interval trigger fires again. Evidence: `comp: field payload: list of shape ability_payload — what a use does, in order; at least one`, `comp: field interval: f32 (> 0) — seconds of game time between uses, at most once a step`; searched `none: \brepeats\b|repeat_?count|repeat_?(use|cast)`.
- **AR16 · Damage over time from its source · gap (7.10); about 150 lines today.** An effect's payload is its own amount times the request's factor (`crates/tungsten-kit/src/damage/effects.rs:764-775`); its source lends the periodic request its `penetration` and `conversion` (`crates/tungsten-kit/src/damage/pipeline.rs:754-765`), and the one link to a hit is an ailment rung's scale, the fill over the threshold (`crates/tungsten-kit/src/damage/ailments.rs:541-546`). Evidence: `comp: the payload's hit, periodic: it passes the window and starts none`, `kit: tungsten_kit::EffectRequest::with_scale`, `comp: shape ailment_rung:`; searched `none: dot_?(rate|source|snapshot)|damage_?over_?time|over_?time`.
- **AR17 · A status's strength from its user · game Rust (~60 lines: the request's scale from a stat).** Effects carry modifiers per stack, tags and a curve at the request's scale; nothing sets the scale from the user's stats (the ailment feed sets it from the hit's fill). Evidence: `comp: shape effect_scale: A curve at the request's scale, as a factor on what`, `kit: tungsten_kit::EffectRequest::with_scale`, `comp: field modifiers: list of shape stat_modifier = [] — on the target's stats, per stack, under the source`.
- **AR18 · Damage taken by context · gap (7.10); about 80 lines today.** The pipeline goes from resistances to the shield and health (`crates/tungsten-kit/src/damage/pipeline.rs:128-139`). An unscoped damage taken is data today, a `resist` factor on the root `damage` tag (up to 1000); the gap is its context, AR2's. Evidence: searched `none: damage_?taken|taken_?mult|\bvulnerab`.
- **AR19 · A resistance floor · gap (7.10); about 40 lines today.** Factors lie in [0, 1000] with modifiers; nothing floors one per entity. Evidence: `comp: field factors: damage tag → factor in [0, 1000]`, `kit: tungsten_kit::Resist::add_modifier`; searched `none: resist_?(floor|cap|max)|max_?resist`.
- **AR20 · Primary stats and `per` lines · gap (7.10); about 120 lines today.** Stats read one another only in fixed places (a cost's `scale`, a restore's `max`); none orders stats or adds per point of another. Evidence: `comp: field scale: text = null — a stat whose value for the user multiplies the amount`, `kit: tungsten_kit::Stats::with_bounds`; searched `none: attribute`, `none: \bper_?stat`.
- **AR21 · Supports · gap (8.7); about 400 lines today** (the kind, links, the applicability check, skill-local lines, the reasons). Evidence: `kit: tungsten_kit::Abilities::add`; searched `none: \bsupports?_?(gem|skill)|socket`.
- **AR22 · A passive tree · gap (8.7); about 500 lines today** (the graph, allocation by points, connectivity on a refund, the sources). The kit's `level` draws offers at each level, an unconnected choice. Evidence: `comp: component level:`, `comp: kind offers:`; searched `none: passive|talent|skill_?tree`.
- **AR23 · Item bases and instances · gap (8.1, P8-3); about 700 lines today.** Loot tables spawn prefabs. Evidence: `comp: kind loot_tables:`, `comp: shape weighted_entry: A prefab, or nothing, and its weight.`; searched `none: inventory|equip|item_?base`.
- **AR24 · Local and global lines; the equipment status · game Rust (~200 lines, on AR23).** A game can apply an item's rolled global lines today as an infinite effect built in code and passed inline; what no item provides is the instance itself (AR23) and local lines folded into its block. Evidence: `core: tungsten_core::assets::data::DataRef<T>::inline`, `kit: tungsten_kit::Effect::with_modifier`, `kit: tungsten_kit::Effect::with_resist`, `kit: tungsten_kit::EffectRequest::new`, `comp: shape effect: A change an entity carries: timed, infinite or instant, with a payload, modifiers and tags.`
- **AR25 · Affix pools, tiers, item level and rarity · gap (8.7); about 600 lines today.** A weighted table draws a prefab or nothing. Evidence: `kit: tungsten_kit::WeightedTable::draw`; searched `none: affix|prefix|suffix|rarity|item_?level|\btier`.
- **AR26 · Uniques and named handlers · gap (8.7); about 150 lines, and a handler's own.** Hooks registered in code exist, none named from data. Evidence: `kit: tungsten_kit::StateMachinePlugin<S>::on_enter`; searched `none: unique_?item|named_?handler|rust_?handler`.
- **AR27 · Rolls seeded and saved · game Rust (~80 lines: a seed from the kit's stream, a stream per item, the result in the save).** Evidence: `kit: tungsten_kit::KitRng::seeded`, `kit: tungsten_kit::KitRng::next_u32`, `core: tungsten_core::save::SaveSlots::save`.
- **AR28 · Monster levels and rarity · gap: stat bases from curves by level, and rarity on AR25 (8.7); about 250 lines today.** Curves and a level exist; a `stats` base is a number. Evidence: `comp: kind curves:`, `kit: tungsten_kit::Curve::at`, `kit: tungsten_kit::Level::new`; searched `none: base_?curve|by_?level|level_?scal`.
- **AR29 · World sources by tag · game Rust (~60 lines, as 01's `enemies::scale`, `examples/01_platformer/src/features/enemies/mod.rs:379-386`), with a gap: no cue answers a spawn, so each spawn path calls it.** Evidence: `kit: tungsten_kit::Stats::add_modifier`, `comp: shape cue_rule:`; searched `none: field spawned:|on_?spawn\b`.
- **AR30 · Ground items and their labels · game Rust (~250 lines).** `pickup` with a filter and a magnet; a world label lasts a frame, pushed each frame. Evidence: `comp: component pickup:`, `core: tungsten_core::text::WorldLabels::push`.
- **AR31 · `explain` · gap (7.10's records, 8.7's commands); about 400 lines today.** The trace records events and amounts, not the lines behind them; the CLI has `new`, `check` and `package`. Evidence: `kit: tungsten_kit::KitTrace::about`, `kit: tungsten_kit::TraceKind::Event::amount`, `cli: check       Load the game's tungsten.json`; searched `none: why_?(applied|matched)|explain`.
- **AR32 · Saves of a build · game Rust (~300 lines today; 7.9's registered components later).** Items, allocations, links, levels and statuses with their sources, by hand. Evidence: `core: tungsten_core::save::SaveData::migrate`, `kit: tungsten_kit::Effects::set_source`.
- **AR33 · A skill bar held down · game Rust (~40 lines; P7-6's held trigger).** An `action` trigger fires on the press. Evidence: `comp: field action: text — an intent slot, used when pressed`, `kit: tungsten_kit::Intent::held`.

Verdicts: 2 data only, 8 game Rust (about 1,030 lines), 23 gaps (about 6,010 lines where a game could write them today), none blocked.

## 15. Register

Rows in the analysis pass's shape (pattern, where, lines, what is lacking, the generic fix, dependencies), as the [paper designs](paper-designs.md)' §4 has them, with the kit domain that holds each fix. **Where** names §14's entries; an entry in parentheses adds no lines here. **Lines** is the game Rust those entries take today, each counted once. **Step** is the default the owner triages; `+` marks a fix the step's sketch in the redo's §3 does not list yet, and `defer` a row whose shape 7.10's systems do not fix, so it could wait for 8.7 at no rework.

| ID | Pattern | Where | Lines | Lacking | Generic fix | Domain | Depends on | Step |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| PA-1 | Lines scoped by tags | AR2 (AR1) | 600 | A modifier has no scope; a stat has one value | `with`, `while`, `against` on lines; reads in a context; `Stats::get` unchanged for the empty one (§5) | foundation | — | 7.10 `+` |
| PA-2 | Skill sheets and the snapshot | AR3, AR4 | 700 | No skill tags, level or bases; payloads take none of the user's values; hits carry no context | Skill tags, levels by curve, sheet stats, skill-local lines; a `Delivery` snapshot on what a use spawns; hits carrying context and state tags (§3.5, §6.2) | intent | PA-1 | 7.10 `+` |
| PA-3 | Area and projectile speed read | AR5, AR6 | 90 | `area` and `projectile_speed` read by nothing | Read from the sheet: radius × √area, the spawn's speed | intent | PA-2 | 7.10 `+` |
| PA-4 | Use time | AR14 (AR33) | 250 | Abilities fire at once; one speed for cooldowns only | Wind-up and recovery from `use_time` and `use_speed`; with P7-6's held trigger | intent | PA-2 | 7.10 `+` |
| PA-5 | Crit | AR13 | 60 | A flag only; kit hits never crit | `crit_chance` and `crit_multiplier`, drawn in the resolve's order for every request whose source has a chance (§6.4) | damage | PA-1 | 7.10 `+` |
| PA-6 | Conversion and extra | AR11, AR12 | 300 | Conversion after scaling, with no history; no extra damage | For a sheet, conversion and extra at use, before scaling, with history per part, merged by type and history (§6.2); other requests keep today's conversion step | damage | PA-2 | 7.10 `+` |
| PA-7 | Defender's side | AR18, AR19 | 120 | No damage taken by context; no floor; penetration on periodic hits | `damage_taken` by context; a floor per factor's tag on `resist`, before penetration; no penetration on periodic requests (§6.4, §6.5) | damage | PA-1 | 7.10 `+` |
| PA-8 | Statuses from their user | AR16, AR17 | 210 | A DoT's rate is its own; a status's strength ignores its user | A DoT's rate from the hit's part and the snapshot's DoT sums, fixed at application; `status_effect` as the status's scale; statuses applied with instance lines as data, which an inline effect does in code today (P8-3 needs them too) | damage, chemistry | PA-2 | 7.10 `+` |
| PA-9 | Primary stats | AR20 | 120 | No ordered stats; no line per point of another | `primary` stats resolved first; `per` lines on them only | foundation | PA-1 | 7.10 `+` `defer` |
| PA-10 | World sources | AR29 | 60 | A scaling per kind of entity is each game's spawn code | A resource of named sources with a tag filter and lines, applied at spawn and to the living, live; 01's difficulty on it | foundation | PA-1 | 7.10 `+` |
| PA-11 | `explain` records | (AR31) | — | The trace keeps amounts, not lines | Why-records while the trace is on, shared with chemistry's `explain` | foundation | PA-1 | 7.10 `+` |
| PA-12 | Items with rolled lines | AR23, AR24 | 900 | Items are prefabs; an item's lines go on only by an effect built in code | 8.1's item instance holds base, level, rarity, seed and rolled lines; local lines fold into its block; the equipment status carries its global lines | items | PA-8 | 8.1 `+` |
| PA-13 | Ground items | AR30 | 250 | Labels pushed by the game each frame | A ground item's label in its rarity's style, with the pickup | items | PA-12 | 8.1 `+` |
| PA-14 | Count and pierce from the sheet | AR7, AR8 | 160 | One prefab per spawn; a fixed pierce | P8-13's pattern payload reads `projectile_count` and its arc; pierce from the snapshot | overlap | PA-2 | 8.2 |
| PA-15 | Chain and fork | AR9 | 350 | Nothing redirects or splits a projectile | Pierce, then fork, then chain at each hit, a family's hit list and its bound of 64 (§6.3) | overlap | PA-14 | 8.2 `+` |
| PA-16 | Repeats | AR15 | 150 | No repeated use | P8-11's burst of the whole payload, `repeats` times, tagged `delivery.repeated` | intent | PA-2 | 8.2 `+` |
| PA-17 | Supports | AR21 | 400 | — | The `supports` kind, links on an ability, applicability by own tags, groups, skill-local lines, reasons for inert ones | build | PA-2 | 8.7 |
| PA-18 | Passive tree | AR22 | 500 | — | The `passive_trees` kind and `passives` component: allocation by points, refunds that keep the tree connected, node sources | build | PA-1 | 8.7 |
| PA-19 | Item generation | AR25, AR27 | 680 | Loot rolls prefabs only | `affixes` and `rarities` kinds; §7's roll; `roll_item`; saved results | build | PA-12 | 8.7 |
| PA-20 | Uniques and handlers | AR26 | 150 | — | The `uniques` kind; named handlers on fixed hooks (§3.6) | build | PA-19 | 8.7 |
| PA-21 | Monster level and rarity | AR28 | 250 | Stat bases are numbers | Stat bases from curves by level; `monster_rarities` drawing from `affixes` | build | PA-19 | 8.7 |
| PA-22 | `explain` and build saves | AR31, AR32 | 700 | — | `explain stat`, `hit` and `item` on PA-11's records; every component here registered as saved on 7.9 | build, umbrella | PA-11, 7.9 | 8.7 |

The rows hold about 7,000 of the build sheet's 7,040 lines: AR33's 40 are P7-6's, already in 7.10's list, and AR1 and AR10 are data only.

## 16. Minimal implementation scope by step

Each step builds its rows in the domain the register names, as opt-in plugins where §3.2 says so.

| Step | Rows | What lands | Done when |
| --- | --- | --- | --- |
| 7.10 | PA-1–PA-11 | Scoped lines and contexts; skill sheets with the snapshot; area, projectile speed and use time read; crit for every hit; conversion and extra before scaling with history; damage taken and floors; DoT and status strength from their user; primary stats; world sources; why-records. The `arpg` genre tag and its first recipes | §9.1's hit resolves to 32.07 in a kit test; 01's crit, blast share and difficulty are data (§8) with no change to its tests' outcomes; the determinism hash moves once, under 7.3's guards |
| 8.1 | PA-12, PA-13 | Item instances with rolled lines, local and global; the equipment status; ground labels | An item from §4.6's JSON equips, shows its lines and saves round trip |
| 8.2 | PA-14–PA-16 | Count with an arc, pierce from the sheet, chain and fork with the family bound, repeats | §9.1's cast lands 9 hits on a pack, 12 with the ring; a fork storm stays under the bound |
| 8.7 | PA-17–PA-22 | Supports, passives, affix generation, uniques, monster level and rarity, `explain`, saves; recipes in `arpg` and one other genre; a sketch; the `arpg.*` rows | 8.7's sketch line: a new skill, support, affix or unique is data only; §12's five tasks pass on a `tungsten new` copy |

If 7.10 is too large, the `defer` rows (PA-9) and PA-3 and PA-4 can move to 8.7 with no rework; PA-1, PA-2, PA-5, PA-6, PA-7 and PA-8 change shapes 7.10 builds and cannot.

## 17. Trade-offs

- **T1. Snapshot at use, not live reads.** A delivery is self-contained: a dead or changed user changes nothing in flight, a projectile is saved with what it needs, and a hit reads no other entity's lines but its target's. The cost: a buff gained mid-flight misses that volley, and a DoT keeps its rate after the user's gear changes. Live reads would cost a resolution per hit and break when the user is gone.
- **T2. Two buckets.** Increased and more only, as the kit's formula has them. Named buckets would let a game shape scaling more finely; the contrast game folded most of its buckets back into one. `explain` stays one line per bucket.
- **T3. Conversion in one pass.** No chains (physical to cold to fire), as the kit's rule already says. A chain would need a type order the game declares and loops guarded; one pass covers the common cases.
- **T4. Extra damage after conversion,** from every part carrying the type. The studied game takes it before; totals agree, since conversion keeps totals, and the history decides scaling either way.
- **T5. Items saved in full, not as a seed.** A seed alone would make every affix table edit change saved items; the full result costs about 200 bytes an item. The seed stays for a reroll and for bugs.
- **T6. Skill tags and damage types apart** (`skill.fire` beside `damage.fire`). One namespace would let a fire skill's added cold part take fire lines; two cost an author a second tag on a skill.
- **T7. One range draw per hit for every part.** Cheaper and one fewer source of drift; parts rise and fall together, where per-type draws would vary less.
- **T8. Shared pieces in existing plugins.** Scoped reads, crit, conversion and floors change `StatsPlugin` and `HealthPlugin` for every genre, each a branch when unused, so one pipeline stays; only sheets, items and `build` are separate plugins.
- **T9. No scoped overrides.** Today's override takes the last added, which follows equip order; scoped overrides would make that order matter in every context. `mul 0` covers "never".
- **T10. Supports checked against the skill's own tags.** A support that adds `skill.area` does not unlock area supports; the alternative, iterating until no tag is added, makes the result depend on link order.
- **T11. Damage over time never crits.** Simpler, and one draw fewer per tick; a game wanting crit-scaled DoT writes a status line on its hit's crit flag through a handler.
- **T12. `build` as a domain.** Q19 places the ARPG's own layers there. The alternative, the genre's name `arpg`, would tie a domain to a genre tag; `build` names the kind (what a character assembles), as the other domains do.

## 18. Questions, each with a default

- **PA-Q1. The new domain's name.** *Default: `build`* (T12).
- **PA-Q2. How large 7.10 grows.** Eleven rows land there by the brief's rule. *Default: all eleven; PA-3, PA-4 and PA-9 may move to 8.7 if 7.10's plan finds it too large* (§16).
- **PA-Q3. Crit in the kit for every genre.** *Default: yes, as a source's stats, drawn only where a chance is above 0* (§6.4), so 01's crit becomes data at 7.13.
- **PA-Q4. Saved items.** *Default: in full, with the seed* (T5).
- **PA-Q5. World sources in 7.10 rather than 8.7.** 01's difficulty uses them, and the `arpg` recipe's area modifiers, which meets the redo's §5.3 admission rule. *Default: 7.10.*
- **PA-Q6. Armour and evasion.** No row: a physical factor and `damage_taken` lines cover 1.0. *Default: a game writes them as status lines or a handler; 1.x if two games ask.*

## 19. Verification

**Mechanical checks,** run in the session's scratchpad on each revision: every `core:`, `kit:`, `umb:`, `comp:` and `cli:` span searched for verbatim in its file, every `none:` pattern run over `api/*.txt` (104 distinct after both rounds, none missing, each search 0 hits), every file and line reference inside its file (21), and `just repo-check`'s header, decision, link and fragment rules (0 errors). §9's arithmetic came from a script, then the checks below.

**Critique,** one round at xhigh on §3.5, §4.1–§4.3, §5 and §6, sent as one extract with the kit's formula and pipeline order quoted for reference (artifact only, 467 s). Six findings, all held and applied:

| # | Finding | Applied |
| --- | --- | --- |
| 1 (high) | Skipping conversion in the pipeline would drop it for requests built without a sheet | Conversion is skipped only for a sheet's request; others keep today's step 2 (§6.4, PA-6) |
| 2 | Partial sums cannot carry an unscoped override past deferred lines | The snapshot keeps the winning override, applied last at the hit (§5.3, §5.4, §3.5) |
| 3 | Penetration resolved per type cannot honour a filter on a part's history | Penetration is resolved and snapshotted per part (§6.2, step 9) |
| 4 | A deferred line with `per` would need the user's primary stat at the hit | Its coefficient is frozen at use (§5.4) |
| 5 | Floor and penetration on the factors' product changed today's per-factor penetration | Per factor, as today, with the floor first (§6.4, step 4; PA-7) |
| 6 | Pierce was counted up but never spent | Pierce while `pierced` is below the snapshot's `pierce` (§6.3) |

**Fresh-context verifier,** one round on the whole document, run beside the critique on the draft. It confirmed 79 of 87 span uses (70 distinct; 6 weak, 2 unsupported), all 23 searches at 0 hits with about 70 other names tried in `api/*.txt` and the kit's and core's source, 18 of 20 file and line references, 28 of 33 verdicts, about 120 numbers in §9, §13–§16 and the digest, and 5 decision IDs. What changed:

- **Verdicts.** AR24 is game Rust: an item's rolled lines go on today as an effect built in code and passed inline (`DataRef::inline`); the tallies became 8 game Rust (about 1,030 lines) and 23 gaps (about 6,010). AR16 and the digest now say that a status follows a hit through an ailment rung's fill, never through the user's lines, and that a periodic request borrows its source's penetration and conversion. AR18 narrows to context, since an unscoped damage taken is a `resist` factor on the root `damage` tag.
- **Evidence.** Searches added to AR6 and AR16; spans that support the claim for AR15, AR20, §4.1's resist shape, §5.3's normalization and §6.4's factors; AR26 names the state machine's hooks as a precedent; AR9 allows for `steering`.
- **Mechanisms.** Conversion keeps today's rule of every matching ancestor, summed per part (§5.3, §6.2); the penetration order it flagged was the critique's finding 5, already applied.
- **Numbers.** §13's volley storm is 45 hits a cast, and its hits a second are derived (385), which §11 now cites; a sheet reads about 240 lines, not 210.
- **Smaller.** §12's tasks name the right rows; the header's API commit; the phases' range; references to the redo's §5 qualified; §9.1's extra part and §9.2's drawback worded.

These fixes were checked mechanically, not by a second round (the brief's stop).

## Session log

- **2026-10-28.** Read §7.3 and the sections it names, the paper designs' register, the plans README, the API snapshots and the kit's stats, damage pipeline, effects, abilities, hit grid, phases and source code, and 01's combat and enemies. Researched the mechanics on public wikis and guides (§2). Found two `D-218` entries in `DECISIONS.md` (the ARPG genre and a `game.id` rule); the `qa6` session in the same tree renumbered its rule to `D-219` (`f139bef`), so `D-218` here is the ARPG's. Drafted with a span checker and an arithmetic script in the scratchpad; the critique (gpt-6-astra, xhigh, on an 18,776-character extract) and a fresh-context verifier ran in parallel on the draft, and their findings were applied in that order.

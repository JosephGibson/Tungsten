# Phase 6 milestone 66: Kit damage types (W13 damage types) — done

- **status:** done
- **goal:** The kit catalogue's DT ([w13-kit](../../1.0/w13-kit.md) §7): damage types a game declares in the `damage_types` data kind, none named by the kit; a hit as typed parts resolved through conversion, penetration and `Resist` with modifiers, then once through the shield and health; `DamageKinds` on `Damaged` and on a new `DamagedPart`, the `damaged_part` cue event, a cue rule `kind` filter and per-type label styles; recipes in a fantasy (top-down) and a sci-fi (shmup) set.
- **non-goals:** W13k's ailments, effects, periodic damage and its flag; kinds on `Struck` or the `hit` cue; typed heals; crit chances (`D-175`); a `label_styles` kind; a type registry in core or a `DataRegistry` revision count (w13-kit Q27); a template change (w13-kit Q23); retyping M65's fixtures' `damage.fire` and `damage.laser` tags; physics; the perf suite, which registers no kit plugin.
- **files to touch:** Step 1: `crates/tungsten-kit/src/damage/{kinds.rs (new), mod.rs, pipeline.rs, defense.rs, health.rs}`, `crates/tungsten-kit/src/foundation/{stats.rs, describe.rs}`, `crates/tungsten-kit/src/overlap/{hit.rs, contact.rs}`, `crates/tungsten-kit/src/react/cues.rs`, `crates/tungsten-kit/src/present/labels.rs`, `crates/tungsten-kit/src/lib.rs`, `crates/tungsten-kit/src/tests/{kinds.rs (new), pipeline.rs, defense.rs, health.rs, hit.rs, contact.rs, cues.rs, describe.rs, inspect.rs, labels.rs}`, `crates/tungsten/src/tests/kit_recipes/coverage.rs`, the fixture files of §2.10, `api/tungsten-kit.txt`, `api/tungsten-kit-components.txt`. Step 2: `crates/tungsten-kit/src/overlap/{grid.rs, hit.rs, contact.rs, projectile.rs}`, `crates/tungsten-kit/src/tests/{grid.rs, hit.rs, contact.rs, projectile.rs}`. Step 3: `crates/tungsten-kit/src/react/cues.rs`, `crates/tungsten-kit/src/damage/kinds.rs`, `crates/tungsten-kit/src/tests/{cues.rs, kinds.rs}`. Step 4: `crates/tungsten/src/tests/kit_recipes/{top_down.rs, shmup.rs, coverage.rs, determinism.rs}`, the fixture files of §2.10. Step 5: `crates/tungsten-kit/benches/kit.rs`; evidence in `perf-runs/` (ignored by Git). Step 6: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/known-issues.md`, `docs/plans/1.0/w13-kit.md`, `docs/getting-started.md`, `docs/LLM_INDEX.md`, `docs/perf/profiling-workflow.md`, `api/`. Step 7: the skeleton's release files.
- **ordered steps:** (1) The API and the typed resolution, then the step 1 API critique. (2) Typed hits. (3) Cues read the types. (4) Recipes in two genres, the genericity check and the kit hash. (5) The bench rows and the criterion sitting. (6) Records. (7) Release 0.86.
- **done-when:** every step's evidence row quoted; workflow §5 closed (§8); the release checks pass after the cut, with `just physics-release` from step 4 and the criterion sitting from step 5 as the gates no release check runs; the preflight prints the command block and the post-merge block.

Candidate W13 damage types ([implementation plan](../../1.0/implementation-plan.md) §4 card, level C), milestone M66, release 0.86. Written 2026-10-18 at `c692e1d` on branch `0.86`, tree clean. Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs.

## Context digest

- **Why now.** The owner asked on 2026-10-17 for damage types between C and E; the amendment to [w13-kit](../../1.0/w13-kit.md) (§7's DT row, §11, Q17–Q27) took two critique rounds and settled the scope. W13k's ailment ladder reads DT's kinds, so DT lands first.
- **What it stands on.** M65's pipeline (`D-175`): `DamageRequest` applied at once or queued for `damage_resolve`, through `Resist`, `Shield` and `Health`, and every hit item landing through one `strike`. A's stats and cues (`D-173`): `Modifier`'s math, the cue reactor's two passes, the `label` action. F2's data kinds (`D-171`): a kind's check runs at a load's commit with every staged entry in place. F3's tags, descriptions and recipes (`D-172`).
- **Level C** (workflow §4): an Opus 5.5 max plan (`D-151`), the API sketch in §2.2, and the step 1 API critique before steps 2–7.
- **Shape.** Step 1 lands every public item and data form, with the damage module's whole behaviour, so the API critique sees all of it; steps 2 and 3 fill in the hit items' and the cue system's behaviour behind that surface; step 4 proves it in two genres. The kit hash holds at M65's pin through step 3, which is the evidence that a one-part hit resolves as M65's.
- **What later candidates expect.** W13k feeds its buckets from `DamagedPart` amounts by the definition a part's kinds resolve to (§2.6's lookup), adds a periodic flag to both events and an `ailments` field to `damage_types` entries (both additive), and adds and removes `Resist` modifiers by source.

## 1. Audit

Read at `c692e1d`. Paths without a crate prefix are under `crates/tungsten-kit/src/`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | `DamageRequest` holds one amount and one tag set: `with_tags` sets them, `amount()` and `tags()` read them | `damage/pipeline.rs:79-87`, `:125-128`, `:181-195` |
| A2 | The resolution: an amount of 0, or a target gone, despawning, without `Health`, dead or in its window, is `Hit::Ignored` with no side effect; `Resist`'s product runs in `f64`, clamped to `f32::MAX`, and a result of 0 is ignored; one shield take, one `hurt` (the window and the death delay), one flash, one `Damaged`, the trace's `damaged` then `killed`, knockback on `Hurt` and `Absorbed` | `pipeline.rs:435-479`, `:424-432`; `damage/health.rs:274-282` |
| A3 | `apply` resolves at once through the same function; `damage_resolve` drains the queue once a step, stable-sorted by target then source: the two paths a mask is built on | `pipeline.rs:217-219`, `:390-409` |
| A4 | `Resist` keeps one factor per tag, sorted, its serde a bare `BTreeMap<String, f32>`, with public `TryFrom` and `From` impls for that map; any matching factor of 0 is immunity, checked before the product | `damage/defense.rs:50-74`, `:131-137`; `api/tungsten-kit.txt:1795-1799` |
| A5 | `resist`'s scene form checks each key as a declared tag | `health.rs:675-684`; `foundation/tags.rs:390-408` |
| A6 | `Damaged` is `Copy` and `#[non_exhaustive]`: entity, amount, killed, source, position, crit, built by `Damaged::new` and `with_*` | `health.rs:329-384` |
| A7 | `HealthPlugin` registers the events, the queue, the four scene components and `damage_resolve`: one plugin owns the pipeline and the defences | `health.rs:577-714` |
| A8 | `Stats`' modifier math (add, pct, mul, override, the zero rules, `f64`) is the private `Stat::recompute`; `Modifier` is public and `#[non_exhaustive]` | `foundation/stats.rs:83-108`, `:422-434` |
| A9 | `stats_apply` writes the `damage` stat into a `Hitbox`'s and a `ContactDamage`'s one number through `set_damage` | `stats.rs:739-777`; `overlap/hit.rs:542-546`; `overlap/contact.rs:315-319` |
| A10 | `Hitbox` and `ContactDamage` hold one damage number and one tag set; their scene forms read `damage` as a non-negative number and `tags` as names; their saved forms deny unknown fields | `hit.rs:355-414`, `:672-694`; `contact.rs:76-178`, `:326-387` |
| A11 | Every hit lands through `strike` from a `Blow` (damage, tags, knockback), queueing the request from the hitter's `Source` or the hitter; `land`, contact damage and the projectile each clone the item's tags once per hitter | `overlap/grid.rs:420-471`; `hit.rs:936-964`; `contact.rs:556-575`; `overlap/projectile.rs:536-555` |
| A12 | `Source` is the spawn chain's origin, so a spawned bolt's request names its shooter's origin as source | `foundation/source.rs:79-87`; `grid.rs:452-455` |
| A13 | The cue system reads `Damaged` first (a `damaged` occurrence, then `killed`), then the other kit queues, then the game's; it runs every occurrence's own cues, then every occurrence's rules; an `Occurrence` carries no kinds | `react/cues.rs:1558-1654`, `:1276-1328`, `:820-828` |
| A14 | A rule is `on`, a subject tag `filter` and `do`; the load checks its event, its tags and its actions; `EventInfo` records whether an event carries an amount and an other entity | `cues.rs:1021-1074`, `:830-871`, `:1080-1134`, `:1205-1214` |
| A15 | The `label` action holds a required `LabelStyle`, whose DTO requires `font`, and floats the occurrence's amount; the kit's events are a fixed list that a `CuesOnEvent` may not reuse | `cues.rs:254-275`, `:522-533`, `:1493-1498`, `:50-67`; `present/labels.rs:57-71`, `:298-315` |
| A16 | `Tags::has` reads dotted ancestors; a `TagFilter` passes an entity without tags when it names no `all` and no `any` | `tags.rs:34-37`, `:120-122`, `:174-177`, `:297-302` |
| A17 | A data kind's check runs at a load's commit with every staged entry in place and sees the registry through `&World`; `DataRegistry::insert` in code skips it | core `assets/data.rs:118-138`, `:670-721`, `:238-257` |
| A18 | An event queue keeps held and previous windows, so an event sent before a reload is read after it | core `ecs/event_queue.rs:98-104` |
| A19 | `FieldType` is `#[non_exhaustive]`; the description tests count 39 components, 4 kinds, 17 shapes and 69 numeric fields, probe one-of shapes and enum variants, and hold samples to their defaults; `Clips` is the precedent for a hand-written either-form visitor | `foundation/describe.rs:252-302`; `tests/describe.rs:125-127`, `:231`, `:361`, `:367-369`; `present/character.rs:201-226` |
| A20 | The umbrella's coverage test counts 39 scene names and 4 kinds, each in two genres' fixtures; the inspector test counts 39 | `crates/tungsten/src/tests/kit_recipes/coverage.rs:81-82`; `tests/inspect.rs:91` |
| A21 | The kit hash is pinned at `0x5a73_d4a5_51f4_0bfa`; its rows hash each entity's point, health, tags, counters, flags, stats, rotation, depth, clip and shield; `reacted` checks M65's counters | `crates/tungsten/src/tests/kit_recipes/determinism.rs:25`, `:58-119`, `:306-334` |
| A22 | `KitPlugins::set()`'s resolved `fixed_update` is pinned as text | `crates/tungsten/src/tests/kit.rs:44`, `:122-128` |
| A23 | The kit bench has 31 rows; `damage_resolve` queues one tagged request a step per target through a resistance and a shield. M65's sitting read, in ns per entity at 1,000 / 10,000: `damage_resolve` 153.56 / 152.26, `cues` 223.48 / 219.65, `hitbox_tick` 372.32 / 412.90, `contact_damage` 388.59 / 422.04, `projectile_tick` 122.15 / 121.77 | `benches/kit.rs:1-30`, `:739-765`; `perf-runs/20261017-m65-kit-criterion/summary.txt` |
| A24 | The trace keeps the last 256 entries | `foundation/trace.rs:57` |
| A25 | The template's test compares a `Damaged` by value with `Damaged::new(entity, 1.0, false)`; its hazard's `contact_damage` is untyped | `templates/basic/tests/game.rs:312-313`; `templates/basic/assets/prefabs/hazard.json:5-6` |
| A26 | Known issues say `Damaged` carries no damage tags; the guide lists the kit's cue events and says the vocabulary stops at the rule's tag filter | `docs/known-issues.md:56`; `docs/getting-started.md:192`, `:195` |
| A27 | No kit source outside `src/tests/` names a type of the two sets except one doc line's "physical pixels" | `rg -n -w` over the 19 names: `intent/cursor.rs:7` only |
| A28 | `docs/LLM_INDEX.md` holds 12,282 of its 12,288 bytes; its kit row routes `crates/tungsten-kit/src/` and cites `D-172`–`D-175` | `docs/LLM_INDEX.md:14` |

## 2. Design

### 2.1 The resolution, against M65's

A **part** is an amount and its types, a `Tags`. A request is a list of parts; its `tags` are the types each part without its own takes (Q2), so `DamageRequest::new(target, 5.0).with_tags(..)` is M65's request: one part, typed by those tags. Resolving a request, at once (`apply`) or queued (`damage_resolve`):

0. **The table.** `apply` merges the type table (§2.3) once per call, `damage_resolve` once per run that has requests (w13-kit Q27).
1. **Parts** (M65's step 1). Each part takes its own tags, else the request's. A part of 0 drops before any arithmetic. No part left, or a target gone, despawning, without `Health`, dead or in its window: `Hit::Ignored`, no side effect.
2. **Conversion.** One rule list: the hit's own `Conversion`, then its source's `Conversion` component (w13-kit Q19), taken once over the parts step 1 left. A part's matching rules are those whose `from` its types carry (`Tags::has`). Each takes its `pct` of the part as a new part of its `to` alone, so the request's tags never follow a share; matching `pct`s that sum past 1 normalize to sum to 1, a sum of 1 or more leaves no remainder, and a smaller sum leaves the part's amount times 1 minus the sum, in `f64`; a share or a remainder of 0 is no part. A share is never converted again. The parts then merge by equal type set in first-appearance order, each part's remainder before its shares in rule order, summed in `f64` and clamped to `f32::MAX` (w13-kit Q18: slash 10 and fire 5, with half the slash to fire, land as slash 5 and fire 10).
3. **Penetration.** A part's points are the sum of the hit's own `Penetration` entries and its source's `Penetration` entries whose tag the part's types carry, capped at 1 (Q5).
4. **Resist, per part** (M65's step 2, per part). For each of the target's entries whose tag the part's types carry: a base of 0 drops the part, immune, before any modifier math; otherwise the entry's factor is its modifiers' value (§2.4); a factor in (0, 1) gains the part's points, capped at 1, and a factor of 0 or of 1 and above takes none. Any factor of 0 drops the part before any product; otherwise the part's amount times the factors' product, in `f64`, clamped to `f32::MAX`. A result of 0 drops the part.
5. **Nothing left:** `Hit::Ignored`, no side effect.
6. **One fall** (M65's steps 3–4). The total is the parts' sum in `f64`, clamped; the shield takes what it can of it, health falls by the rest, one window, one flash.
7. **Events.** One `DamagedPart` per part in part order (its amount after `Resist`, its kinds, the request's source and position), then one `Damaged` (the total, the union of the parts' kinds, killed, source, position, crit). The trace records `damaged_part` per part, then `damaged`, then `killed` (Q12).
8. **Outcome** (M65's steps 5–6): `Killed`, `Hurt` or `Absorbed`, knockback on the last two, the death policy's delay on a kill.

A part's kinds are the bits of every table name its types have, found by searching each type's dotted prefixes in the table's sorted index, so the cost follows the part's tags, not the table's 64. An untyped part has no kind and still sends its `DamagedPart` (w13-kit Q17). A one-part request with no conversion, no penetration and no modifier computes exactly M65's arithmetic: the kit hash at its pin through step 3 holds that.

### 2.2 API sketch

Every new public item has rustdoc with an example. Names marked new; everything else keeps its 0.85 signature.

```rust
// damage/pipeline.rs
#[derive(Debug, Clone, PartialEq)]
pub struct DamagePart { /* tags: Tags, amount: f32 */ }                     // new
impl DamagePart {
    pub fn new(amount: f32) -> Self;               // untyped; panics unless finite and >= 0
    pub fn with_tags(self, tags: Tags) -> Self;    // its types
    pub fn amount(&self) -> f32;
    pub fn tags(&self) -> &Tags;
}
impl DamageRequest {
    pub fn new(target: Entity, amount: f32) -> Self;                              // one untyped part, as 0.85
    pub fn from_parts(target: Entity, parts: impl IntoIterator<Item = DamagePart>) -> Self; // new; no part resolves Ignored
    pub fn with_tags(self, tags: Tags) -> Self;    // now: the types each part without its own takes
    pub fn with_penetration(self, penetration: Penetration) -> Self;             // new: the hit's own
    pub fn with_conversion(self, conversion: Conversion) -> Self;                // new: the hit's own
    pub fn parts(&self) -> &[DamagePart];                                        // new
    pub fn penetration(&self) -> Option<&Penetration>;                           // new
    pub fn conversion(&self) -> Option<&Conversion>;                             // new
    pub fn amount(&self) -> f32;                   // now: the parts' sum before resistances, clamped
    pub fn tags(&self) -> &Tags;                   // now: those default types
}

// damage/health.rs
pub struct Damaged { /* 0.85's fields */ pub kinds: DamageKinds }               // field new, additive
impl Damaged { pub fn with_kinds(self, kinds: DamageKinds) -> Self; }            // new
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct DamagedPart {                                                         // new; cue event `damaged_part`
    pub entity: Entity, pub amount: f32, pub kinds: DamageKinds,
    pub source: Option<Entity>, pub position: Option<Vec2>,
}
impl DamagedPart {
    pub fn new(entity: Entity, amount: f32) -> Self;
    pub fn with_kinds(self, kinds: DamageKinds) -> Self;
    pub fn with_source(self, source: Entity) -> Self;
    pub fn with_position(self, position: Vec2) -> Self;
}

// damage/kinds.rs (new)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct DamageKinds { /* bits: u64 */ }
impl DamageKinds {
    pub const NONE: Self;
    pub fn is_empty(self) -> bool;
    pub fn has(self, world: &World, name: &str) -> bool;   // a type named `name`, or one under it
    pub fn names(self, world: &World) -> Vec<&str>;          // the types, by name
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Penetration { /* (tag, points) sorted */ }       // scene `penetration`
impl Penetration {
    pub fn new() -> Self;
    pub fn with_points(self, tag: &str, points: f32) -> Self; // panics: not a tag name, points outside [0, 1]
    pub fn points(&self, tags: &Tags) -> f32;                  // matching entries summed, capped at 1
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Conversion { /* (from, to, pct) in order */ }   // scene `conversion`
impl Conversion {
    pub fn new() -> Self;
    pub fn with_rule(self, from: &str, to: &str, pct: f32) -> Self; // panics: a bad name, from == to, pct outside [0, 1]
}

// damage/defense.rs
impl Resist {
    pub fn add_modifier(&mut self, tag: &str, source: &str, modifier: Modifier); // new (Q6)
    pub fn remove_modifiers(&mut self, source: &str) -> usize;                   // new, as Stats'
    pub fn base(&self, tag: &str) -> Option<f32>;                                // new
    // `factor` reads the factors after their modifiers
}
// removed: impl From<Resist> for BTreeMap<String, f32> (Q7, a break-ledger row)

// overlap/hit.rs; overlap/contact.rs has the same six on ContactDamage
impl Hitbox {
    pub fn with_parts(self, parts: impl IntoIterator<Item = DamagePart>) -> Self; // damage := their sum; panics: none, or all 0
    pub fn with_penetration(self, penetration: Penetration) -> Self;
    pub fn with_conversion(self, conversion: Conversion) -> Self;
    pub fn parts(&self) -> &[DamagePart];          // empty for a one-number hitbox
    pub fn penetration(&self) -> Option<&Penetration>;
    pub fn conversion(&self) -> Option<&Conversion>;
}

// react/cues.rs
impl CueAction { pub fn type_label() -> Self; }    // new: `{ "label": {} }`, a `damaged_part` cue's only

// foundation/describe.rs
pub enum FieldType { /* 0.85's */ Damage }         // new: a number, or { damage type: amount }
```

`Hitbox::damage()` and `ContactDamage::damage()` keep returning each hit's damage: one number, or the parts' total. `Hit`'s docs say a hit whose every part dropped is `Ignored`. A typed item's payload (parts, penetration, conversion) sits in one crate-private boxed field, `None` for an M65 item, so `Hitbox` and `ContactDamage` grow by one pointer. `Resist`'s serde stays derived, through a private saved form (`try_from`/`into`), so its serde lines in `api/tungsten-kit.txt` do not change.

### 2.3 The `damage_types` kind and the type table

- **Entries.** The kind's entries are objects keyed by damage type, each a `damage_type` definition whose fields are optional: `label_style` now, W13k's `ailments` later (`deny_unknown_fields` until then). `{ "elemental": { "label_style": { "font": "sans", "color": [1, 1, 0, 1] } }, "elemental.fire": {} }`. A field is inherited down the dotted tree: a type's value is its own entry's, else its nearest declared ancestor's that sets it.
- **The load's check** (`HealthPlugin` registers the kind, Q1): each key a declared tag (`check_declared`, `tags.rs:390-408`); no key in two entries (Q4); and room for every new name. The table's names keep their bits, a reload's removed names included; the keys any entry holds (with the staging in place, A17) that the table lacks are the new names. When they outnumber the bits left, every checked entry that holds a new name fails as a `MissingAsset` of kind `damage type (a World holds 64)` naming its first new name; core reports the first in check order and keeps the previous entries. Core checks file-backed entries only (core `assets/data.rs:838-850`) and keeps an entry whose file another entry took, without its file (`:303-313`), so the failure must not hang on which name sorts past 64: replacing a name once all 64 bits are taken fails the replacement's entry, and so does taking a file from an entry no merge has read yet. A name that only a file-less entry holds is no check's: past 64 it gets no bit and one warning (Q9).
- **The table.** `DamageTypeTable`, a crate-private World resource: names in bit order and an index sorted by name. Its first merge takes every entry's keys, sorted, as bits 0 to n − 1; a later merge appends the names it lacks, sorted among themselves; a name a reload removed keeps its bit, reserved, so a mask queued before a reload (A18) keeps its meaning. A name past 64 that no check saw (a code `insert`, or a file-less entry's) gets no bit and one warning (Q9). The merge inserts the table when the World has none. Merging reads each entry's keys against the sorted index: no core file (w13-kit Q27).
- **Names each field may give** (Q3): a `damage` map key and a conversion's `to` must be declared damage types, keys of a `damage_types` entry, so every typed part has a bit; a conversion's `from`, a penetration key and a `kind` filter's names may also be dotted ancestors of one; `resist` keys stay declared tags (A5). A failure is a `MissingAsset` of kind `damage type`.

### 2.4 `Resist`'s modifiers and its saved form

- An entry is a tag, an optional base and its modifiers by source in add order. Its factor: a base of 0 is 0 whatever the modifiers (immune); otherwise `Stats`' formula over the base (1 when there is none, Q6), the modifiers in order, the last override winning and the zero rules kept, in `f64`, clamped to `[0, 1000]`. An entry without modifiers uses its base as it is, which keeps M65's products bit for bit.
- The formula is one crate-private function in `stats.rs` that `Stat::recompute` and `Resist` both call, its arithmetic in `Stat::recompute`'s order; the kit hash at its pin (which hashes every stat's bits) shows the move changed no stat.
- `add_modifier` panics on a bad tag name or a modifier that is not finite; a modifier on an unlisted tag makes an entry with no base, which goes with its last modifier. `remove_modifiers(source)` removes that source's modifiers from every entry and returns how many.
- **Saved form** (w13-kit Q24): an object by tag whose value is a number (a base with no modifiers: 0.85's form, read and written as it was) or `{ "base": 0.5, "modifiers": [["curse", { "mul": 2 }]] }`, `base` optional; `{}` is refused. Read through a hand-written visitor over either form (`deserialize_any`, as `Clips`), each base in `[0, 1000]` and each modifier finite. The scene form, `resist`, stays the bare map.

### 2.5 Typed hit items

- **Data forms** (w13-kit Q20, Q25). `hitbox` and `contact_damage` read `damage` as a number (0.85's) or a map, `{ "physical.slash": 8, "elemental.fire": 4 }`, through a hand-written visitor that keeps the field path in its errors (`D-153`); an empty or all-zero map is refused (w13-kit Q21). A map's parts come in type-name order: a prefab's components pass through `serde_json` values (core `assets/prefab.rs:86`, `component_registry.rs:626`), whose objects are sorted maps without `preserve_order` (`Cargo.toml:49`), so the written order is never seen and is no contract. `tags` beside a map is refused, since the map's keys type every part. `pen`, a map of points in `[0, 1]`, and `convert`, a list of `{ "from", "to", "pct" }`, are optional on both. A projectile's damage is its hitbox's.
- **The `damage` stat** (w13-kit Q21). A typed item keeps its map as base parts that no stat writes; its damage starts at their sum, and `stats_apply` writes the stat into it as before (A9). Each hit splits the damage among the parts by their proportions, in `f64`, clamped; when the damage equals the parts' sum the parts go as they are. A damage of 0 makes the hit a trigger, as today, and the stat's return restores the parts.
- **Landing.** `Blow` carries the item's payload; `strike` builds `DamageRequest::new(target, damage).with_tags(tags)` for an M65 item and `from_parts(target, split parts).with_tags(tags)` for a typed one, with the item's penetration and conversion, from the hitter's `Source` or the hitter (A11, A12). The payload is cloned once per hitter that lands, as the tags are now.
- **Saved forms** (Q13). An item's saved form keeps `damage` as 0.85's does, the hit's damage, and adds `parts`, a list of `{ "tags": [..], "amount": n }` for its base parts (a Rust part may be untyped or carry several tags, which no map key can say), with `pen` and `convert`, each defaulting to empty, so 0.85 saves read. Only the scene forms refuse `tags` beside a map; a saved item may hold both, its tags typing its untyped parts.

### 2.6 Cues

- **`damaged_part`** joins the kit's events (`KIT_EVENTS`, `CueEventNames`) with an amount and an other entity: its subject is the part's entity, its other the source, its amount the part's after `Resist`, its position the request's or the entity's point. It is reserved, so a `CuesOnEvent` of that name panics at build. The reactor reads `DamagedPart` through its own cursor before `Damaged`, so in each of its two passes every `damaged_part` occurrence runs before the step's `damaged` and `killed` ones, and `KitRng` draws in that order (round 2's finding 3 in w13-kit's critique).
- **The rule's `kind` filter**: `"kind": { "all": [..], "any": [..], "none": [..] }`, `{}` by default, a second filter beside the subject's tag filter, read over the occurrence's kinds; each action still takes at most one tag filter, and there are no conditions. Only `damaged`, `killed` (the total's kinds) and `damaged_part` carry kinds: a `kind` on another event fails the load, and so does a name that is not a declared damage type or an ancestor of one. A name's mask is the bits of the table's names equal to it or under it; `all`, `any` and `none` then read as `TagFilter` reads them, so an untyped occurrence passes only a filter without `all` and `any` (A16). The reactor keeps each name's mask, cleared when the table grows.
- **Labels.** `{ "label": {} }` (`CueAction::type_label()`, Q10) is a `damaged_part` cue's only: elsewhere the load fails it. It floats the part's amount in its type's `label_style`: the part's definition is its deepest type among its kinds, ties to the first by name (w13-kit Q26), and the style is that type's entry's, else its nearest ancestor's that sets one, read from the current entries so a reload restyles. A part whose types set no style skips the label, its trace entry saying why (Q8). A label that names any field must name `font`, as before.
- The vocabulary's doc and the guide say the boundary now: the rule's tag filter and, on the damage events, its kind filter.

### 2.7 Descriptions and the snapshot

- New descriptions: components `penetration` and `conversion`; shapes `conversion_rule` (`from`, `to`, `pct` in `[0, 1]`) and `damage_type` (`label_style`, default `null`); the kind `damage_types`. Changed: `hitbox`'s and `contact_damage`'s `damage` (`FieldType::Damage`) and `tags` (refused beside a map), and their new `pen` (default `{}`) and `convert` (default `[]`); `health`'s events gain `DamagedPart`, which rewrites its one `events:` line (`foundation/describe.rs:174-176`, `api/tungsten-kit-components.txt:95`); `resist`'s summary and field doc say a part's factors; `cues` gains `damaged_part`; `cue_rule` gains `kind`; `cue_action`'s `label` doc names `{}`. Samples use the descriptions' existing placeholder tag `damage.fire`, which no fixture declares as a damage type.
- The description tests move to 41 components, 5 kinds and 19 shapes, `damage_type` joining the kind values no field names; `FieldType::Damage` gets its probe branch (both forms at their bounds; an empty and an all-zero map refused), so the numeric count moves from 69 to 70 (`conversion_rule.pct` added, the two `damage` fields probed in the new branch); the one-of and enum counts stay. The inspector test moves to 41, with `Penetration` and `Conversion` rows; `Resist`'s rows show each entry's modifiers.
- `api/tungsten-kit-components.txt` then holds 41 `component`, 19 `shape` and 5 `kind` lines.

### 2.8 Item cards

| Item | Data form, defaults | Reads | Events | Phase | Writes | Recipes | Inspector | Perf row |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `damage_types` kind | entries by type, each field optional and inherited | declared tags | — | read in `damage_resolve`, `apply`, `cues` | the table | top-down fantasy, shmup sci-fi | — | `damage_resolve_typed` |
| `penetration` | `{ type: points }`, the value itself | `damage_types` | — | read at a resolve | — | pyromancer, hacker | `Penetration` | `damage_resolve_typed` |
| `conversion` | `[{ from, to, pct }]`, the value itself | `damage_types` | — | read at a resolve | — | pyromancer, hacker | `Conversion` | `damage_resolve_typed` |
| typed `hitbox`, `contact_damage` | `damage` number or map; `pen` `{}`; `convert` `[]` | `damage_types`; the `damage` stat | `Struck` as before | `Overlap`, as before | requests | spellblade, firebolt, payload | parts on the `damage` row | `hitbox_tick`, `contact_damage`, `projectile_tick` |
| `Resist` modifiers | scene form unchanged; saved form §2.4 | — | — | read at a resolve | — | bases in the fixtures; modifiers in unit tests and the hash script | modifiers | `damage_resolve_typed` |
| `DamagedPart`, `Damaged.kinds` | cue event `damaged_part` | the table | out | `Resolve` | — | all four | — | `damage_resolve`, `cues_typed` |
| rule `kind`, `{ "label": {} }` | `kind` `{}` | the table, the entries | in: `damaged`, `killed`, `damaged_part` | `React`, labels in `Present` | counters, labels | all four | — | `cues_typed` |

No system is added, so no within-phase edge and no schedule change: `KitPlugins::set()`'s resolved text (A22) stays.

### 2.9 What stays

- No core, render or umbrella runtime file (the umbrella's recipe tests and fixtures change); no new dependency; `DefaultPlugins` and the kit's schedule as they are; the physics hashes.
- M65's fixtures keep `damage.fire` and `damage.laser` as plain tags: their hits stay untyped one-part hits with no kinds.
- The template: no change (w13-kit Q23); its hazard stays an untyped `contact_damage`, and its test's `Damaged::new(entity, 1.0, false)` still equals what it sends (A25), since an untyped hit's kinds are `NONE`.

### 2.10 Fixtures and recipes

Fixture files, under `crates/tungsten/tests/fixtures/kit/`, land in step 1 with the data forms they use, so the coverage test and the loads pass from step 1; step 4 adds the tests and may move a prefab's numbers or places with the recipes. Colours are linear and encode to sRGB bytes as named.

- **`top_down/`**: `manifest.json` gains `data.damage_types.fantasy` and `data.cue_rules.elements` and the five prefabs; `data/tags.json` gains `physical.slash`, `physical.pierce`, `physical.smash`, `elemental.fire`, `elemental.cold`, `elemental.lightning`, `toxic`, `chaos`. `data/damage_types/fantasy.json`: those eight plus `physical` (cyan `[0, 1, 1, 1]`) and `elemental` (yellow `[1, 1, 0, 1]`); `elemental.fire` red `[1, 0, 0, 1]`, `elemental.cold` blue `[0, 0, 1, 1]`, `toxic` green `[0, 1, 0, 1]`, `chaos` magenta `[1, 0, 1, 1]`, the rest inheriting, every style in face `sans`. `data/cue_rules/elements.json`: `[{ "on": "damaged_part", "kind": { "any": ["elemental"] }, "do": [{ "counter": { "name": "elemental_hits", "add": 1, "target": "game" } }] }]`. Prefabs: `spellblade` (a hitbox, circle 12, `damage { physical.slash 8, elemental.fire 4 }`, `pen { elemental.fire 0.25 }`, `convert [physical.slash → elemental.cold 0.5]`, filter `team.enemy`); `golem` (`team.enemy`, health 20, hurtbox circle 10, `resist { physical 0.5, elemental 0.8, elemental.fire 0.5 }`, cues `damaged_part: [{ "label": {} }]`); `pyromancer` (`team.player`, cooldown 1 s firing `firebolt`, `penetration { elemental.fire 0.5 }`, `conversion [elemental.fire → toxic 0.5]`); `firebolt` (linear motion 600 px/s, a hitbox, circle 3, `damage { elemental.fire 8 }`, filter `team.enemy`, projectile range 400, nothing else); `ward` (`team.enemy`, health 10, hurtbox circle 8, `resist { elemental.fire 0.25, toxic 0 }`, cues `damaged_part: [{ "label": {} }]`).
- **`shmup/`**: `manifest.json` gains `data.damage_types.scifi` and `data.cue_rules.typed` and the three prefabs; `data/tags.json` gains `digital.virus`, `digital.overload`, `digital.corruption`, `digital.exploit`, `biological.toxin`, `biological.pathogen`, `biological.acid`. `data/damage_types/scifi.json`: those seven plus `digital` (cyan) and `biological` (yellow); `digital.virus` green, `digital.exploit` magenta, `biological.acid` red, the rest inheriting. `data/cue_rules/typed.json`: `[{ "on": "damaged_part", "kind": { "any": ["digital.virus"] }, "do": [{ "counter": { "name": "virus_hits", "add": 1, "target": "game" } }] }]`. Prefabs: `hacker` (`team.player`, cooldown 1 s firing `payload`, `penetration { digital 0.25 }`, `conversion [digital.overload → digital.virus 0.5]`); `payload` (linear motion `[0, -900]`, a hitbox, aabb `[1, 6]`, `damage { digital.overload 8, digital.exploit 2 }`, filter `team.enemy`, projectile range 500); `mainframe` (`team.enemy`, health 100, hurtbox aabb `[16, 16]`, `resist { digital 0.5, digital.exploit 0 }`, shield 2, cues `damaged_part: [{ "label": {} }]`).

The recipes (step 4), each at both of `DTS`, amounts and health within 1e-4:

1. **Top-down, `a_spellblades_typed_hit_converts_penetrates_resists_and_labels_each_part`.** A spellblade on a golem. Its map's parts in name order, fire 4 then slash 8; after conversion fire 4, slash 4, cold 4; resisted: fire 4 × min(1, 0.8 + 0.25) × (0.5 + 0.25) = 3, slash 4 × 0.5 = 2, cold 4 × 0.8 = 3.2. `DamagedPart` amounts `[3, 2, 3.2]` in that order, kinds `["elemental", "elemental.fire"]`, `["physical", "physical.slash"]`, `["elemental", "elemental.cold"]`; one `Damaged` of 8.2 whose kinds are those five names; health 11.8; `elemental_hits` 2; labels, as a set, `3` red `[255, 0, 0, 255]`, `2` cyan `[0, 255, 255, 255]`, `3.2` blue `[0, 0, 255, 255]` and the combat rule's `8.2` white.
2. **Top-down, `a_pyromancers_bolts_take_its_penetration_and_conversion`.** A pyromancer's firebolt on a ward: the pyromancer's conversion splits fire 8 into fire 4 and toxic 4, toxic drops (immune), fire 4 × (0.25 + 0.5) = 3; one `DamagedPart` of 3, kinds `["elemental", "elemental.fire"]`; `Damaged` 3; health 7; labels `3` red and the combat rule's `3` white. Without the source's components the hit would deal 2, so the numbers prove w13-kit Q19.
3. **Shmup, `a_hackers_payload_converts_to_a_virus_and_an_exploit_immunity_holds`.** A hacker's payload on a mainframe: its parts in name order, exploit 2 then overload 8; overload splits into overload 4 and virus 4; with the hacker's points, overload and virus each 4 × (0.5 + 0.25) = 3, exploit 2 drops (a base 0, which penetration never pierces); `DamagedPart` `[3, 3]` with kinds `["digital", "digital.overload"]` and `["digital", "digital.virus"]`; `Damaged` 6 without `digital.exploit` (`has` is false); the shield takes 2, health 96; `virus_hits` 1; labels `3` cyan and `3` green.
4. **Shmup, `an_untyped_laser_sends_one_part_of_no_kind_and_its_label_skips`.** An interceptor's laser (`damage.laser`, M65's) on a mainframe: one `DamagedPart` of 1 with `DamageKinds::NONE` (w13-kit Q17), the shield absorbs it, the style-less label skips with its trace reason, `virus_hits` stays 0.

### 2.11 The genericity check (w13-kit Q22)

`kit_recipes::coverage::no_kit_source_names_a_fixture_damage_type` reads the keys of every JSON file under each fixture's `data/damage_types/` (19 names; the entry type is the kit's, private to it) and scans each `.rs` file under `crates/tungsten-kit/src/` outside `src/tests/`, dropping doc-comment lines (`///`, `//!`), for any name as a whole token: not preceded or followed by `[a-z0-9_.]`. The scan is one function; `kit_recipes::coverage::the_damage_type_scan_finds_a_planted_name` is its negative control: a code line `let t = "physical.slash";` is reported and the same text in a `///` line is not. `coverage.rs`'s counts move to 41 scene names and 5 kinds in step 1.

### 2.12 The bench rows

Two rows join `benches/kit.rs`, 33 in all, with the header updated:

- `damage_resolve_typed`: `HealthPlugin` with types `bench.a` and `bench.b` declared (`TagsPlugin::with_tags`, a `damage_types` entry staged from a file as `stage_rule` stages its rule); each target `Health::new(1e12)`, `Resist` with `bench.a` 0.5 and `bench.b` 0.8 plus a `Mul(0.5)` modifier on `bench.b`, a recharging shield; each iteration queues, per target, a request of parts `bench.a` 2 and `bench.b` 1, converting half of `bench.a` to `bench.b`, with 0.25 points on `bench.b`.
- `cues_typed`: the `cues` row's rig with those types and a rule `{ "on": "damaged_part", "kind": { "any": ["bench"] }, "do": [game counter] }`; each entity's own `damaged_part` counter cue; one warm `apply` gives a `DamagedPart`'s kinds, and each iteration sends one `DamagedPart` per entity with them.

## 3. Steps

Every code step's done-when also holds: `cargo fmt --all -- --check` → exit 0; `just lint` → exit 0. A test listing is a scratchpad Python script running `cargo test -p <package> --lib --locked -- --list` through `subprocess` and asserting each named test is listed.

### Step 1: The API and the typed resolution

- **Files:** step 1's in the header.
- **Change:**
  1. `damage/kinds.rs` (new, `pub(crate) mod kinds` in `damage/mod.rs`): the `damage_types` kind's DTO, check and description (§2.3), the table and its merge, the mask of a part's types, `DamageKinds`, `Penetration` and `Conversion` with their scene forms, checks (§2.3's names), descriptions and inspector rows.
  2. `stats.rs`: the crate-private modifier formula `Stat::recompute` now calls (§2.4); `Stats::DAMAGE`'s doc names a typed item's total.
  3. `defense.rs`: `Resist`'s entries, modifiers, saved form and `base`; the product per part with its points (§2.1 step 4); `From<Resist> for BTreeMap<String, f32>` removed.
  4. `pipeline.rs`: `DamagePart`, the request's parts, penetration and conversion, and the resolution of §2.1, with both merge points.
  5. `health.rs`: `Damaged.kinds` and `with_kinds`, `DamagedPart`; `HealthPlugin` registers the kind, `penetration`, `conversion`, `EventQueue<DamagedPart>`, the table and the rows.
  6. `describe.rs`: `FieldType::Damage` and its render.
  7. `hit.rs`, `contact.rs`: the payload field, the six methods, the data forms and checks of §2.5, the saved forms, the descriptions; their hits stay untyped until step 2 (`land` and `strike` unchanged).
  8. `cues.rs`, `labels.rs`: `damaged_part` in the event list and names; the rule's `kind` data form and its load check; `CueAction::type_label` and the `{}` form with its load check; `ActionKind::Label` holding an optional style: the reactor floats a named style as before and skips a style-less label with its trace reason until step 3; the descriptions.
  9. `lib.rs`: root and prelude exports of the five new types; the root docs' damage paragraph.
  10. The fixture files of §2.10; `coverage.rs`'s counts (41, 5).
  11. Unit tests (below), then `just api`.
  
  No item may wait on a later step's caller: the rule keeps the kind filter's names, which step 1's load check reads, and step 3 adds the masks; a crate-private item with no caller until a later step takes `#[expect(dead_code, reason = "M66 step N")]`, recorded as a departure (M63's precedent).
- **Tests named:** in `damage::kinds::tests`: `bits_follow_sorted_names_at_the_first_read_and_append_after`, `a_reload_keeps_a_queued_masks_meaning_and_a_removed_types_bit`, `sixty_four_types_load_and_a_sixty_fifth_fails` (one entry of 65; two entries of 40 and 25; 64 loads and its last name takes bit 63; with all 64 taken, a reload replacing the last-sorted name with an earlier one fails the replacement's entry and keeps the previous entries; with 63 taken, an unmerged entry holding `z` whose file a new entry holding `a` and `zz` takes fails the new entry), `a_typed_apply_right_after_a_load_carries_its_mask`, `a_type_two_entries_define_fails_the_load`, `penetration_and_conversion_refuse_bad_names_and_bounds`. In `damage::pipeline::tests`: `a_one_part_hit_resolves_as_before` (0.85's untyped, tagged, immune, clamped and zero cases, value for value), `a_multi_part_hit_starts_one_window_and_sends_one_damaged_after_its_parts`, `a_zero_part_and_a_modifier_made_zero_factor_meet_no_product` (103 dotted levels of 1000 that stay nonzero, a product past `f64::MAX` that the 13 levels of `tests/pipeline.rs:142` never reach: a part of 0 beside a typed part drops; and a part carrying the deepest level and a 104th matching entry, sorting after the levels, whose factor a modifier makes 0, drops, so a product taken before the zero check would meet infinity times 0; no NaN and no `f32::MAX`), `conversion_normalizes_past_one_and_never_chains`, `shares_that_land_on_one_type_merge_in_first_appearance_order`, `penetration_never_pierces_zero_or_raises_a_factor_above_one`, `the_source_lends_its_penetration_and_conversion_after_the_hits_own`, `a_hit_whose_every_part_drops_is_ignored_with_no_side_effect`. In `damage::defense::tests`: `a_base_zero_stays_immune_under_every_modifier_and_penetration`, `modifiers_combine_as_stats_do_and_clamp_to_the_factor_bounds`, `an_085_save_reads_and_modifiers_round_trip`. In `overlap::hit::tests` and `overlap::contact::tests`: `damage_takes_a_number_or_a_map_and_names_the_field_in_errors`, `a_map_beside_tags_or_an_all_zero_map_fails`. In `react::cues::tests`: `a_kind_filter_on_an_event_without_kinds_or_an_unknown_type_fails_the_load`, `a_style_less_label_is_refused_outside_damaged_part`. In `present::labels::tests`: `an_empty_label_takes_no_style_and_a_partial_one_needs_a_font`.
- **Done-when:**
  - `cargo test -p tungsten-kit --locked` → exit 0 (unit tests and doctests)
  - `cargo test -p tungsten --lib --locked kit_` → exit 0, `kit_recipes::determinism` at the 0.85 pin: `git diff --quiet HEAD -- crates/tungsten/src/tests/kit_recipes/determinism.rs crates/tungsten/src/tests/kit.rs` → exit 0
  - `cargo test -p tungsten-template-basic --locked` → exit 0
  - the listing script on `tungsten-kit` → every test named above listed
  - `rg -c 'assert_eq!\(registered.len\(\), 41\)' crates/tungsten/src/tests/kit_recipes/coverage.rs` → 1; `rg -c 'assert_eq!\(described.len\(\), 41\)' crates/tungsten-kit/src/tests/describe.rs` → 1
  - `just api`, then `git diff --quiet HEAD -- api/tungsten-core.txt api/tungsten.txt api/tungsten-render.txt` → exit 0
  - a scratchpad Python `Counter` of `git show HEAD:api/tungsten-kit.txt` against the tree's file → the removed lines are exactly the two of `From<Resist> for BTreeMap<String, f32>`; the same on `api/tungsten-kit-components.txt` → every removed line is a `damage` or `tags` field line of `hitbox` or `contact_damage`, `health`'s `events:` line, a line of `resist`, or a line of `cue_action` or `cue_rule`
  - `rg -c '^component ' api/tungsten-kit-components.txt` → 41; `rg -c '^shape ' api/tungsten-kit-components.txt` → 19; `rg -c '^kind ' api/tungsten-kit-components.txt` → 5
  - then the step 1 API critique, as the skill runs it; a finding that holds changes step 1, and `just api` and the checks above run again
- **Moves:** `api/tungsten-kit.txt` and `api/tungsten-kit-components.txt`, reviewed.

### Step 2: Typed hits

- **Files:** step 2's in the header.
- **Change:** `Blow` carries the payload; `strike` builds the untyped or typed request of §2.5 with the item's penetration and conversion; `land`, contact damage and the projectile pass their item's payload, cloned once per hitter that lands; the split of §2.5 under the `damage` stat.
- **Tests named:** `overlap::hit::tests::a_typed_hitbox_queues_its_parts_split_to_its_damage`, `overlap::hit::tests::a_zero_damage_stat_makes_a_typed_hitbox_a_trigger_and_its_return_restores_the_parts`, `overlap::contact::tests::typed_contact_damage_carries_its_parts_penetration_and_conversion`, `overlap::projectile::tests::a_typed_projectiles_hits_carry_its_hitboxs_parts`, `overlap::grid::tests::strike_types_the_untyped_parts_with_the_items_tags`.
- **Done-when:** `cargo test -p tungsten-kit --locked` → exit 0; `cargo test -p tungsten --lib --locked kit_` → exit 0 with `determinism.rs` unchanged (`git diff --quiet HEAD -- crates/tungsten/src/tests/kit_recipes/determinism.rs` → exit 0); the listing script → the five tests listed; `just api-check` → exit 0 (no surface beyond step 1's snapshots).
- **Moves:** none.

### Step 3: Cues read the types

- **Files:** step 3's in the header.
- **Change:** the reactor's `DamagedPart` cursor, read before `Damaged`; `Occurrence` gains its kinds, set on `damaged`, `killed` and `damaged_part`; the cue system merges the table once per run that has occurrences; the rule's kind masks, kept per name and cleared when the table grows; `kinds.rs` gains the definition and style lookup of §2.6; the style-less label floats in its type's style or skips with its reason.
- **Tests named:** in `react::cues::tests`: `damaged_part_runs_before_damaged_in_each_pass_under_kit_rng` (two two-part hits in one step, with own cues and rules on both events, each list holding a `roll`; the trace's cue entries run every part's own cues, then every total's, then every part's rules, then every total's, and the rolls draw in that order), `a_rules_kind_filter_matches_by_type_and_ancestor` (`all`, `any`, `none`, an untyped occurrence, `killed` reading the total's kinds), `a_damaged_part_label_takes_its_types_style_unless_it_names_one` (inherited from an ancestor, a tie at one depth to the first name, a hot-reloaded style, a part without a style skipping), `a_kind_mask_follows_a_type_a_reload_adds`. In `damage::kinds::tests`: `a_parts_definition_is_its_deepest_type_ties_to_the_first_name`.
- **Done-when:** `cargo test -p tungsten-kit --locked` → exit 0; `cargo test -p tungsten --lib --locked kit_` → exit 0 with `determinism.rs` unchanged (as step 2); the listing script → the five tests listed; `just api-check` → exit 0.
- **Moves:** none.

### Step 4: Recipes in two genres, the genericity check and the kit hash

- **Files:** step 4's in the header.
- **Change:** the four recipes of §2.10 and the two coverage tests of §2.11. The determinism script spawns, after M65's prefabs and clear of them, a golem with a spellblade on it (top-down) and a mainframe with a hacker below it in its payloads' range (shmup); at frame 100 it adds `Modifier::Mul(1.5)` on each golem's `physical` from `curse` and `Modifier::Add(0.25)` on each mainframe's `digital` from `overclock`; its rows add each entity's `Resist` as its saved form's JSON; `reacted` adds top-down's `elemental_hits >= 1` and shmup's `virus_hits >= 1`, M65's checks unchanged (the golem and the mainframe survive the 300 frames); the pin moves to the value the test prints, and its doc adds M66's reason.
- **Done-when:**
  - `cargo test -p tungsten --lib --locked kit_recipes` → exit 0
  - the listing script on `tungsten` → `kit_recipes::top_down::a_spellblades_typed_hit_converts_penetrates_resists_and_labels_each_part`, `kit_recipes::top_down::a_pyromancers_bolts_take_its_penetration_and_conversion`, `kit_recipes::shmup::a_hackers_payload_converts_to_a_virus_and_an_exploit_immunity_holds`, `kit_recipes::shmup::an_untyped_laser_sends_one_part_of_no_kind_and_its_label_skips`, `kit_recipes::coverage::no_kit_source_names_a_fixture_damage_type` and `kit_recipes::coverage::the_damage_type_scan_finds_a_planted_name` listed
  - `cargo test -p tungsten --lib --locked kit_recipes::determinism -- --nocapture` → passes and prints `kit recipes hash: 0x…`; `rg -c '0x5a73_d4a5_51f4_0bfa' crates/tungsten/src/tests/kit_recipes/determinism.rs` → 0, and the printed value counted in the file → 1
  - `cargo test -p tungsten --lib --locked kit_tests` → exit 0 (the resolved schedule unchanged)
  - `just physics-release` → exit 0 (the physics hashes unchanged)
- **Moves:** the kit hash, its new value in the evidence row, the decision and the `CHANGELOG.md` line.

### Step 5: The bench rows and the criterion sitting

- **Files:** step 5's in the header.
- **Change:** the two rows of §2.12 and the header's count; then one quiet sitting of all 33 rows.
- **Done-when:**
  - `cargo test -p tungsten-kit --bench kit --locked` → exit 0 (each row once, test mode)
  - `CARGO_TARGET_DIR=$PWD/target/criterion-native cargo bench -p tungsten-kit --bench kit --locked --no-run` → exit 0, before the sitting
  - before and after each part, after the encoder has been gone 60 s: `pgrep -x 'nxcodec.bin|cargo|rustc'` → no output, and no other agent session or Codex run working in the tree (`pwdx $(pgrep -x 'claude|codex')`), the step 1 API critique included
  - the sitting, three blocking foreground parts, each under the 10-minute cap: `CARGO_TARGET_DIR=$PWD/target/criterion-native cargo bench -p tungsten-kit --bench kit --locked -- '<part regex>'` → exit 0 for part 1 `^(lifetime_tick|health_tick|action_input|top_down_mover_bodiless|tag_filter|stats_apply|cooldown_tick|cues|cues_typed|linear_motion|oscillator)/`, part 2 `^(oscillator_present|follow|orbit|steering|knockback|speed_limit|speed_limit_settle|wrap|stay_in_view|aim|facing)/` and part 3 `^(character_animator|depth_sort|hit_grid|hitbox_tick|contact_damage|projectile_tick|pickup_tick|pickup_magnet|damage_resolve|damage_resolve_typed|shield_tick)/`
  - a scratchpad script reads each row's `criterion/<row>/<n>/new/estimates.json` median and writes `perf-runs/<YYYYMMDD>-m66-kit-criterion/summary.txt` in M65's format → 33 rows, every ratio (per entity at 10,000 over per entity at 1,000) ≤ 2.00 (`D-174`'s bound)
  - against M65's per-entity medians (A23), at each size: `hitbox_tick`, `contact_damage`, `projectile_tick` and `cues` ≤ 1.5×, `damage_resolve` ≤ 2.0× (it gains a `DamagedPart` per request, w13-kit Q17); a row past a check reruns once alone, with the same checks
- **Moves:** none.

### Step 6: Records

- **Files:** step 6's in the header.
- **Change:** in this order: the decision of §5 with the tungsten-decision skill, its marker lines on `D-173` and `D-175` and the index row with their two "Amended by" notes; then `DESIGN.md`'s "Damage Types — M66" section after M65's; the W4 ledger's two rows (the additions; the behaviour changes with the `From` removal as the one break), `From` column `W13 damage types (M66 §2.x)`; known issues lose A26's sentence on `Damaged`'s tags and gain DT's limits (§2.3's names past 64; a source despawned before its request resolves lends nothing; no kinds on `Struck` or `hit`); `w13-kit.md`'s revision sentence and w13-kit Q17–Q27 each answered with M66's decision ID where §2 settled it; the guide's `health` and `cues` paragraphs: the `damage_types` kind, a `damage` map, `pen` and `convert`, `penetration` and `conversion`, `damaged_part`, the `kind` filter and a label in its type's style, and the boundary sentence; the LLM index's decision range `D-172`–`D-175` becomes M66's (same bytes); the profiling workflow's criterion paragraph: 33 rows and M66's ratio range; `just checks`, each recommendation matched to the step or release check that runs it.
- **Done-when:**
  - `rg -c '^## D-17[6] — ' DECISIONS.md` → 1; `rg -c '^\*\*Amended by D-17[6]:\*\*' DECISIONS.md` → 2
  - `rg -c '^\| `D-17[6]` \|' docs/DECISION_INDEX.md` → 1; `rg -c 'Amended by `D-17[6]`' docs/DECISION_INDEX.md` → 2
  - `rg -c '^### Damage Types — M66' DESIGN.md` → 1
  - `rg -c '\| W13 damage types \(M66 ' docs/plans/1.0/w04-api-freeze.md` → 2
  - `rg -c 'carries no damage tags' docs/known-issues.md` → 0
  - `rg -c 'damage_types' docs/getting-started.md` → at least 1
  - `rg -c 'D-172`–`D-17[6]`' docs/LLM_INDEX.md` → 1; `just ctx` → exit 0
  - `rg -c 'over 33\)' docs/perf/profiling-workflow.md` → 1
  - `just api-check` → exit 0
  - `just repo-check` → exit 0
  - `just checks` → each recommended command listed in the evidence row with where it ran
- **Moves:** none.

### Step 7: Release 0.86

- **Files:** `CHANGELOG.md`, `DESIGN.md` (step 6's section; the cut rewrites its workspace line), `Cargo.toml`, `Cargo.lock`, the register row, the README's four owned Now fields (`roadmap.py summary`), this plan (moved to `docs/plans/archive/`)
- **Change:** close workflow §5 (§8) and this step's evidence row with `status: done`; then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.86.0` once, every release check once after it (no gate by change is left for it: step 4 ran `just physics-release`, step 5 the sitting, and no render file changes, so no `just visual`), then the preflight with `--message`. Before it, `git status --short` and the last decision ID, for another session's work (the release commit takes it too).
- **Done-when:** [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut); `just release-preflight 0.86.0 --repo JosephGibson/Tungsten --message 'Update 0.86: M66 kit damage types'` → passes and prints the command block and the post-merge block
- **Ends with:** the changed-file list, the command block, the post-merge block, then the plan prompt for W13k (`docs/plans/1.0/phase6-milestone-67-kit-effects-abilities.md`) from [tungsten-next's prompts](../../../../.claude/skills/tungsten-next/prompts.md)

## 4. Open questions

w13-kit Q17–Q27 take their stated defaults where §2 applies them. The plan's own:

| # | Question | Default |
| --- | --- | --- |
| Q1 | Which plugin registers the kind, `penetration`, `conversion`, `DamagedPart` and the table | `HealthPlugin`, which owns the pipeline and the defences (A7); no new plugin, so `KitPlugins::set()` and its schedule stay |
| Q2 | What a request's `tags` mean beside parts | The types each part without its own takes: `new(..).with_tags(..)` stays M65's request, and a converted share leaves them behind; `tags()` returns them |
| Q3 | Which names each field may give | §2.3: map keys and `to` are declared damage types; `from`, penetration keys and `kind` names may be ancestors of one; `resist` keys stay declared tags |
| Q4 | A type two entries define | Fails the load, naming both entries |
| Q5 | Several matching penetration entries | Their points add, the hit's and the source's, capped at 1, off each matching factor in (0, 1) |
| Q6 | A modifier on a tag `Resist` lists no base for | Applies to an implied base of 1, the entry going with its last modifier; `add_modifier` returns nothing, since every tag has a factor (unlike `Stats::add_modifier`'s `bool`) |
| Q7 | `From<Resist> for BTreeMap<String, f32>` | Removed, a break-ledger row: it would drop modifiers in silence; `TryFrom` stays for bases and `Resist::base` reads one |
| Q8 | A style-less label on a part whose types set no style | Skipped with its trace reason; no fallback, since the kit names no font |
| Q9 | Names past 64 that no check saw (`DataRegistry::insert` in code, or an entry whose file another entry took) | No bit, one warning per name |
| Q10 | The style-less label's form | `{ "label": {} }`; a label naming any field names `font` |
| Q11 | `DamageKinds`' surface | `NONE`, `is_empty`, `has(world, name)` and `names(world)`; no public bits or constructor from names, since bits mean nothing outside their World |
| Q12 | Tracing parts | One `damaged_part` entry per part, before the hit's `damaged` (w13-kit §2 rule 11) |
| Q13 | A typed item's saved form | `damage` keeps the hit's damage as 0.85's does, and a `parts` list holds the base parts with their tags; the scene form's map cannot hold an untyped or many-tagged Rust part |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`).

## 5. Decisions expected

176: W13 damage types landed (M66): parts, the `damage_types` kind and its table, `DamageKinds` and `DamagedPart`, `Resist`'s modifiers, penetration and conversion, the `damaged_part` cue event, the rule's `kind` filter and labels in a type's style; amends `D-175` (the resolution) and `D-173` (the cue vocabulary: one event, a kind filter on the damage events, a label without a style). Next free when written; checked again at step 6.

## 6. Invariants

- **May move:** the kit hash, in step 4 only (new value in the evidence row, the decision and the `CHANGELOG.md` line); `api/tungsten-kit.txt` and `api/tungsten-kit-components.txt`, in step 1.
- **Must not move:** the determinism hash `0x088e_c07a_73c1_b168` and the containment hash `0xaee2_72e0_1ffc_4e4c` (`just physics-release`, step 4); the row digests, `gpu-visual.png` and the post and transition regressions (no tracked row runs kit code and no render file changes); `DefaultPlugins`' schedule and `KitPlugins::set()`'s resolved text; the core, render and umbrella API snapshots; the kit hash through steps 1–3; the template's behaviour; no core file and no new dependency.

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with steps that don't depend on it. Steps 2 and 3 need 1; 4 needs 1–3; 5 needs 1–4; 6 records what landed. The release needs steps 1–6, the card's checks spanning all of them; a skipped one leaves the release to a release session after the owner has seen why (workflow §3).
- The kit hash leaves `0x5a73_d4a5_51f4_0bfa` in steps 1–3: a one-part hit no longer resolves as M65's; the step fails.
- A change needs a core file, a new dependency or a new system: stop and record.
- The step 1 API critique finds a shape tied to one genre or to a game's damage type: redesign or drop it (w13-kit §14).
- A file outside §files to touch needs a change: stop and record.
- A row past its ratio bound or its guard after its one rerun: fix it before the release if the cause is in M66's code and the fix fits the session; otherwise stop at the release with both readings (M65's precedent).
- A step 1 API critique finding that changes code after the sitting voids the sitting: run step 5's sitting again.
- The encoder or another session's build does not clear: wait; never sit beside them.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1–5, release | Crate tests, lint and format each code step; `just physics-release` in 4; the criterion sitting in 5; the release chain (`just check`, `just smoke`, `just script-test`, `just template-check`, `just api-check`, `just notices`) once after the cut; no `just visual` (no render change) |
| 3 Invariants | 1–4 | The kit hash at its pin through 3, moved in 4; physics hashes in 4 |
| 4 Decisions | 6 | 176, before the docs that cite it |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1, 6 | Snapshots and rustdoc in 1, critiqued; ledger rows in 6; `just api-check` at the release |
| 6 Template and guide | 6 | No template change (w13-kit Q23: the hazard stays untyped, the fixtures teach the types); the guide's damage and cue paragraphs in 6 |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 6 | `damage/kinds.rs` is under the kit row's `crates/tungsten-kit/src/`; the decision range only |
| 8 Design | 6 | "Damage Types — M66" |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 6, release | Known issues in 6; nothing cut, so no backlog row; the gap log is empty; the register's plan column now, its release at the release; `w13-kit.md` in 6; `python3 -B scripts/roadmap.py summary` supplies the owned fields |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

**Round 1.** The `critique` skill with `--repo` (`gpt-6-astra`), 2026-10-18, on `c692e1d` with this plan the tree's only change, §9 and the evidence log cut by `sed`. Footer: effort max, context 872,000, output cap 4,000, 357.2 s, 962,009 tokens in (857,088 cached), 12,944 out (8,258 reasoning), 37 commands, 333.9 KB of output (largest 36.2 KB), 5h window 34% → 59%, week 9% → 13%. Four findings, all accepted; finding 4 changed step 1's done-when, so round 2 runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [MEDIUM] The capacity check can lay the failure on a definition that no longer exists: with 64 bits taken, replacing the last-sorted name with an earlier one makes a 65th name that no current entry holds | core `assets/data.rs:838-850` (the commit checks current file-backed entries only) | accepted | §2.3: new names take the bits left in sorted order and the entry holding the first one past them fails; step 1's 64-type test replaces a name at capacity |
| 2 | [MEDIUM] The spellblade recipe assumes a map order the loader discards | core `assets/prefab.rs:86`, `component_registry.rs:626`; `Cargo.toml:49` (no `preserve_order`) | accepted | §2.5: a map's parts come in type-name order; recipe 1 expects fire, slash, cold, `[3, 2, 3.2]`; recipe 3 names its order (its outcome unchanged) |
| 3 | [MEDIUM] The zero-factor test never reaches the overflow it guards: two factors of 1000 on `f32::MAX` stay finite in `f64` | `crates/tungsten-kit/src/tests/pipeline.rs:140-170` (13 levels, 1e39) | accepted | Step 1's test uses 103 levels of 1000, a product past `f64::MAX` |
| 4 | [MEDIUM] The snapshot gate forbids a needed change: `DamagedPart` in `health`'s events rewrites its one `events:` line | `foundation/describe.rs:174-176`; `api/tungsten-kit-components.txt:95` | accepted | §2.7 names `health`'s events and `resist`'s doc; step 1's removed-line rule allows both |

**Round 2.** The same reviewer, 2026-10-18, on the revised plan (60,849 characters with the focus note, so `--force`, which step 7 confirms), §9 and the log cut, with a `--focus-file` naming round 1's five changed sections. Footer: effort xhigh, context 872,000, output cap 4,000, 218.1 s, 421,363 tokens in (352,896 cached), 7,756 out (5,106 reasoning), 18 commands, 198.6 KB of output (largest 28 KB), 5h window 59% → 73%, week 13% → 15%. Two findings, both accepted, each a gap a round-1 fix had left; no third round runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] The first overflowing name can belong to an entry whose check never runs: a file taken from an unmerged entry leaves it file-less, so its name sorts into the overflow while the new entry's own extra name passes | core `assets/data.rs:303-313` (a released file keeps the definition), `:844` (file-less entries skip the check) | accepted | §2.3: when the new names outnumber the bits left, every checked entry holding a new name fails; names only file-less entries hold fall to Q9; step 1's test adds the file case |
| 2.2 | [MEDIUM] Zeroing one of the 103 factors leaves 102, a finite 1e306, so the test misses infinity times 0 | `damage/pipeline.rs:424-430` (the product before the amount) | accepted | Step 1's test keeps 103 nonzero levels and zeroes a 104th entry that sorts after them |

**Step 1 API critique.** The skill's `critique --raw --repo --force` on the `api/` diff (no template change; 50,038 characters, under the soft limit), `gpt-6-astra` at xhigh, 2026-10-18, queued behind the 5-hour window's 07:49:57 UTC reset (73% at step 1's end) and run from 0%. Footer: 178.0 s, 843,129 tokens in (759,808 cached), 5,304 out (2,790 reasoning), 15 commands, 268.5 KB of output (largest 84.6 KB), 5h window 0% → 19%, week 15% → 18%. No concerns: it checked the typed resolution, conversion's normalization, penetration and immunity, the type table's limit and `Resist`'s saved form, and called the `From<Resist>` removal deliberate.

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-18 | done, one departure | `cargo test -p tungsten-kit --locked` 625 passed with doctests; `cargo test -p tungsten --lib --locked kit_` 135 passed, `kit_recipes::determinism` at `0x5a73_d4a5_51f4_0bfa`; `cargo test -p tungsten-template-basic --locked` 19 passed; the listing script 24 of 24 named tests; `assert_eq!(registered.len(), 41)` 1, `assert_eq!(described.len(), 41)` 1; `just api` exit 0, then the core, umbrella and render snapshots unchanged (exit 0); `api/tungsten-kit.txt` 4 lines removed, the 2 of `From<Resist> for BTreeMap<String, f32>` at both of `Resist`'s paths, 299 added; `api/tungsten-kit-components.txt` 8 removed (6 distinct: `health`'s `events:`, `hitbox`'s and `contact_damage`'s `damage` and `tags`, `resist`'s summary and field, `cue_action`'s `label`), 31 added, 41 `component`, 19 `shape`, 5 `kind` lines; `cargo fmt --all -- --check` and `just lint` exit 0; `cargo test --workspace --locked` 2,786 passed, 8 ignored. Departure, the owner's call at the run's start: Q12's per-part trace entry fails `kit_tests::the_trace_tells_why_an_entity_died` unless its expected story gains `damaged_part 3` (and its two index checks move to `entries[3]`), so `crates/tungsten/src/tests/kit.rs` changes and its `git diff --quiet` check gives way to the `kit_tests` run, which holds A22's schedule pin. No `#[expect(dead_code)]` was needed. The API critique: no concerns | `crates/tungsten-kit/src/damage/kinds.rs`, `crates/tungsten-kit/src/tests/kinds.rs` (new); `api/tungsten-kit.txt`, `api/tungsten-kit-components.txt`; scratchpad `api-critique.md` |
| 2 | 2026-10-18 | done | `cargo test -p tungsten-kit --locked` 548 unit and 82 doctests passed; `cargo test -p tungsten --lib --locked kit_` 135 passed, the kit hash at its pin and `git diff --quiet HEAD -- crates/tungsten/src/tests/kit_recipes/determinism.rs` exit 0; the listing script 5 of 5; `just api-check` exit 0 (snapshots current); `cargo fmt --all -- --check` and `just lint` exit 0. `strike` builds the request from the item's payload through `HitPayload::request`; `Hitbox::payload` serves the projectile | `crates/tungsten-kit/src/overlap/{grid,hit,contact,projectile}.rs` and their tests |
| 3 | 2026-10-18 | done | `cargo test -p tungsten-kit --locked` 553 unit and 82 doctests passed; `cargo test -p tungsten --lib --locked kit_` 135 passed, the kit hash at `0x5a73_d4a5_51f4_0bfa` with `determinism.rs` unchanged (exit 0); the listing script 5 of 5; `just api-check`, `cargo fmt --all -- --check` and `just lint` exit 0. The reactor reads `DamagedPart` first, merges the table once per run with occurrences and keeps each `kind` name's mask until the table grows | `crates/tungsten-kit/src/react/cues.rs`, `crates/tungsten-kit/src/damage/kinds.rs` and their tests |
| 4 | 2026-10-18 | done, one departure | `cargo test -p tungsten --lib --locked kit_recipes` exit 0 (68 tests); the listing script 6 of 6; `kit_recipes::determinism -- --nocapture` prints `kit recipes hash: 0x22882ee797d4aba9`, equal over two in-process runs, so the pin moves from `0x5a73_d4a5_51f4_0bfa` (count 0) to `0x2288_2ee7_97d4_aba9` (count 1); `cargo test -p tungsten --lib --locked kit_tests` 73 passed (the resolved schedule unchanged); `just physics-release` exit 0, 7 passed (the physics hashes unchanged); `cargo fmt --all -- --check` and `just lint` exit 0. No fixture number moved. Departure: the hash rows add each `Resist` as its inspector row (each factor with its base and its modifiers by source), not as its saved form's JSON, since the umbrella has no `serde_json` and adding it would be a new dependency (§6); the genericity check reads the fixtures' keys with a small scanner for the same reason | `crates/tungsten/src/tests/kit_recipes/{top_down,shmup,coverage,determinism}.rs` |
| 5 | 2026-10-18 | done | `cargo test -p tungsten-kit --bench kit --locked` exit 0, 66 benchmarks once; the criterion build `--no-run` exit 0 (51.6 s). The sitting, three blocking foreground parts with a per-second poller, each after 60 s with no `nxcodec.bin`, `cargo` or `rustc`: part 1 08:06:11–08:09:55 UTC, part 2 08:11:02–08:15:01, part 3 08:16:07–08:19:58, each exit 0 with no sighting; the one other `claude` process with its cwd in the tree (started 06:57 UTC, its only child the idle context7 server) ticked 0.6–0.7 s per part, idle, and wrote nothing; `codex` idle from `/home/joker`. The summary's 33 rows read ratios 0.61–1.25, all ≤ 2.00; `cues_typed` 249.27 / 244.93 ns per entity, `damage_resolve_typed` 577.77 / 611.89. Against M65's per-entity medians: `hitbox_tick` 1.00× / 1.00×, `contact_damage` 0.98× / 1.00×, `projectile_tick` 0.92× / 0.91×, `cues` 1.08× / 1.09× (each ≤ 1.5×), `damage_resolve` 1.65× / 1.67× (≤ 2.0×, 152 to 253 ns: the request's parts, its `DamagedPart` and its mask); no rerun needed | `crates/tungsten-kit/benches/kit.rs`; `perf-runs/20261018-m66-kit-criterion/summary.txt` (ignored); scratchpad `sit-part{1,2,3}.{log,out}` |
| 6 | 2026-10-18 | done | `D-176` written last-free (no other session's decision); `rg -c '^## D-17[6] — ' DECISIONS.md` 1, `'^\*\*Amended by D-17[6]:\*\*'` 2 (on `D-173` and `D-175`); the index row 1 and its two "Amended by" notes 2; `### Damage Types — M66` 1; the W4 ledger's two rows 2; `carries no damage tags` 0 in the known issues, which gain DT's limits; `damage_types` in the guide 1; w13-kit's revision sentence and Q17–Q27 answered (11); the LLM index's range `D-172`–`D-176` 1 at the same 12,282 bytes, `just ctx` exit 0; `over 33)` 1 in the profiling workflow; `just api-check` and `just repo-check` exit 0. `just checks` recommends: `cargo test -p tungsten-kit --locked` (steps 1–3), `cargo test -p tungsten --locked` (659 passed here), `cargo test -p tungsten-core --test manifests --locked` (5 passed here and in `just repo-check`), `just ctx`, `just repo-check`, `just api-check` (here), `git diff --check` (exit 0 here), `just check` and `just smoke` (the release chain); its reviews: no link touched in `DECISIONS.md`, `DESIGN.md` or the known issues, the public API reviewed in step 1 with its ledger rows here, no template change | `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/known-issues.md`, `docs/plans/1.0/w13-kit.md`, `docs/getting-started.md`, `docs/LLM_INDEX.md`, `docs/perf/profiling-workflow.md` |
| 7 | 2026-10-18 | done, the release checks in the report | Workflow §5 closed (§8): evidence rows 1–7; the gates by change ran in steps 1–6 (crate tests, `just lint` and `cargo fmt --all -- --check` each code step, `just physics-release` in 4, the criterion sitting in 5), the release chain once after the cut; invariants: the kit hash moved on purpose in step 4 to `0x2288_2ee7_97d4_aba9` (recorded here, in `D-176` and in the changelog), the physics hashes unchanged (step 4), `DefaultPlugins`' and `KitPlugins::set()`'s schedules unchanged (`kit_tests`), the core, render and umbrella snapshots unchanged, no core or render file and no new dependency; no tracked perf row runs kit code and no render file changed, so the row digests, `gpu-visual.png` and the post and transition regressions stand untested by design; `D-176` before the docs that cite it; the W4 ledger's rows, rustdoc on every new item and the snapshots reviewed; no template change (w13-kit Q23); routes: `damage/kinds.rs` under the kit row, `just ctx` exit 0; `DESIGN.md`'s M66 section and status line; records: the known issues, the register row's release and status, `w13-kit.md`, the README's owned fields from `roadmap.py summary`, no backlog row (nothing cut), the gap log untouched; the changelog's `[Unreleased]` notes. Then `status: done` and the plan archived before `just release-cut 0.86.0` | `CHANGELOG.md`, `DESIGN.md`, `docs/plans/1.0/implementation-plan.md`, `docs/plans/1.0/README.md`, this plan |

## Follow-ups

- Kinds on `Struck` and the `hit` cue, when a reaction to a hit's types before its resistances needs them.
- A `label_styles` kind, when two types or two rules share a style (M63's follow-up stands).
- The descriptions' placeholder tag `damage.fire`, which reads like a damage type, could become a neutral name in a snapshot pass.

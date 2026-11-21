//! Caps: a group's live instances held to a number, a kit spawn past it
//! ending the group's oldest or refused (redo 7.10c, S-18), and a game's
//! own spawns at once admitted under them (7.13d's placements).

use serde::{Deserialize, Serialize};
use tungsten_core::save::SaveOptions;
use tungsten_core::{
    Entity, Inspectable, Plugin, PrefabRegistry, PreparedPrefab, Schedule, SpawnContext, Time,
    World,
};

use crate::damage::health::Health;
use crate::foundation::describe::{
    ComponentDescription, FieldDescription, FieldType, RPG, SHOOTER, describe, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::saved::order_of;
use crate::foundation::source::{Source, SourcePlugin};
use crate::foundation::tags::valid_tag;
use crate::lifecycle::ended::{Despawning, end, ending, ensure_kit_despawn};
use crate::motion::drive::centre_of;
use crate::react::cues::{CULLED, KitCue, cue_named, push_kit_cue};

/// The scene name.
pub(crate) const CAP: &str = "cap";

/// The name a save holds an admission's mark under.
const CAP_ADMITTED: &str = "cap_admitted";

/// Why a capped spawn did not go ahead, for its route's trace.
pub(crate) const FULL: &str = "its cap's group is full";
pub(crate) const QUEUED: &str = "its cap's group is full of this frame's spawns";
pub(crate) const LEAVING: &str = "its source goes at the flush";

/// What a spawn past a cap's max does. A match outside this crate needs a
/// wildcard arm, since later forms are additions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Past {
    /// The group's oldest counted instances end, as many as make room.
    #[default]
    Oldest,
    /// The spawn is refused.
    Refuse,
}

/// A cap on its group's live instances, a scene component on a prefab.
/// Each kit spawn of a capped prefab (an ability's spawn, a cue's `prefab`
/// or `roll`, a spawner, chemistry's `spawn` and `become`) and each game
/// spawn [`admit_spawn`] admits counts the group's live holders the kit
/// admitted or that carry a [`Source`], so a map-placed one stays aside,
/// less those ending this step or replaced this frame, and with
/// [`without_dead`](Self::without_dead) those whose [`Health`] is dead,
/// plus the group's kit admissions queued this frame, which land at its one
/// flush. At `max`, [`Past::Oldest`] ends the oldest holders, restored ones
/// by save order first, then by admission, through the kit's end-of-step
/// despawn, sending `culled` about each when named, and refuses when this
/// frame's admissions fill the group; [`Past::Refuse`] refuses the spawn,
/// which [`cap_refuses`] tells a game before it spends. A spawn from a
/// source that goes at the flush is refused, and a spawner gets a refused
/// spawn back. Each spawn reads its own prefab's cap against its group's
/// one count. It is rule data, not saved, while an admission's mark is
/// saved (`cap_admitted`), so an admitted holder counts through a load; a
/// World without a capped prefab does no cap work.
///
/// Scene name `cap`: `{ "group": "salt", "max": 2, "past": "oldest",
/// "count_dead": true }`, `past` `oldest` and `count_dead` true by default.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use tungsten_core::{Plugin, Schedule, World};
/// use tungsten_kit::{Cap, CapPlugin, KitDescriptions, Past};
///
/// let mines = Cap::new("mines", 3).refusing();
/// assert_eq!((mines.group(), mines.max(), mines.past()), ("mines", 3, Past::Refuse));
/// assert!(mines.counts_dead());
/// assert!(!Cap::new("summon.foes", 10).without_dead().counts_dead());
///
/// // The scene form, the dead left out of the count, through its round trip.
/// let mut world = World::new();
/// CapPlugin.build(&mut Schedule::new(), &mut world);
/// let cap = world.resource::<KitDescriptions>().get("cap").unwrap();
/// let form = json!({ "group": "summon.foes", "max": 10, "count_dead": false });
/// assert_eq!(
///     cap.round_trip(form).unwrap().unwrap(),
///     json!({ "group": "summon.foes", "max": 10, "past": "oldest", "count_dead": false })
/// );
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Cap {
    group: String,
    max: u32,
    past: Past,
    count_dead: bool,
}

impl Cap {
    /// A cap of `max` live instances on `group`, its oldest ending past it.
    ///
    /// # Panics
    ///
    /// When `group` is not a dotted name or `max` is 0.
    #[must_use]
    #[track_caller]
    pub fn new(group: impl Into<String>, max: u32) -> Self {
        let group = group.into();
        assert!(
            valid_tag(&group),
            "Cap::new: `{group}` is not a group name (`[a-z][a-z0-9_]*` segments joined by dots)"
        );
        assert!(max >= 1, "Cap::new: max must be at least 1");
        Self {
            group,
            max,
            past: Past::Oldest,
            count_dead: true,
        }
    }

    /// The cap, refusing a spawn past its max.
    #[must_use]
    pub fn refusing(mut self) -> Self {
        self.past = Past::Refuse;
        self
    }

    /// The cap, leaving a holder whose [`Health`] is dead out of its count
    /// and its victims, as a summoner's fallen do not hold a place.
    #[must_use]
    pub fn without_dead(mut self) -> Self {
        self.count_dead = false;
        self
    }

    /// Its group's name.
    #[must_use]
    pub fn group(&self) -> &str {
        &self.group
    }

    /// The most live instances its group holds.
    #[must_use]
    pub fn max(&self) -> u32 {
        self.max
    }

    /// What a spawn past it does.
    #[must_use]
    pub fn past(&self) -> Past {
        self.past
    }

    /// Whether a holder whose [`Health`] is dead counts, as it does until
    /// it ends by default; false after [`without_dead`](Self::without_dead).
    #[must_use]
    pub fn counts_dead(&self) -> bool {
        self.count_dead
    }

    /// Whether `entity`, one of its group's holders, holds a place: alive
    /// by its [`Health`], or the dead counted.
    fn holds(&self, world: &World, entity: Entity) -> bool {
        self.count_dead || !world.get::<Health>(entity).is_some_and(Health::is_dead)
    }
}

/// `past` as data writes it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PastDef {
    #[default]
    Oldest,
    Refuse,
}

/// `components.cap` as a scene writes it, checked.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "CapFields")]
struct CapDef {
    group: String,
    max: u32,
    past: PastDef,
    count_dead: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CapFields {
    group: String,
    max: u32,
    #[serde(default)]
    past: PastDef,
    #[serde(default = "counts_dead")]
    count_dead: bool,
}

/// `count_dead`'s default: the dead count until they end.
fn counts_dead() -> bool {
    true
}

impl TryFrom<CapFields> for CapDef {
    type Error = String;

    fn try_from(fields: CapFields) -> Result<Self, Self::Error> {
        if !valid_tag(&fields.group) {
            return Err(format!(
                "group: `{}` is not a group name (`[a-z][a-z0-9_]*` segments joined by dots)",
                fields.group
            ));
        }
        if fields.max == 0 {
            return Err("max: must be at least 1 (got 0)".to_string());
        }
        Ok(Self {
            group: fields.group,
            max: fields.max,
            past: fields.past,
            count_dead: fields.count_dead,
        })
    }
}

impl From<CapDef> for Cap {
    fn from(def: CapDef) -> Self {
        Self {
            group: def.group,
            max: def.max,
            past: match def.past {
                PastDef::Oldest => Past::Oldest,
                PastDef::Refuse => Past::Refuse,
            },
            count_dead: def.count_dead,
        }
    }
}

/// An admission's stamp, its place in the admission order: a component an
/// admitted spawn takes, undescribed, so a holder the kit spawned with no
/// source counts, and holders age by admission whatever entity indices
/// they took. It is saved as `cap_admitted` with an empty form and comes
/// back as stamp 0, the earliest, so a restored holder counts and ages
/// with the restored by save order, before this run's admissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Admitted(u64);

impl Admitted {
    /// Its rank in the holders' order: an unstamped holder's and a
    /// restored one's 0, then this run's by stamp.
    fn rank(stamp: Option<&Self>) -> u64 {
        stamp.map_or(0, |stamp| stamp.0)
    }
}

/// An [`Admitted`] in a save: the mark alone, its stamp this run's.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdmittedSave {}

impl From<&Admitted> for AdmittedSave {
    fn from(_: &Admitted) -> Self {
        Self {}
    }
}

impl From<AdmittedSave> for Admitted {
    fn from(_: AdmittedSave) -> Self {
        Self(0)
    }
}

/// The caps' frame state, a World resource [`CapPlugin`] inserts, unsaved:
/// the frame's admissions per group, which land at its one flush, and the
/// receivers a `become` replaces at it, both stamped with the frame and
/// dropped by a later one; and the admissions' count, the next stamp.
#[derive(Debug, Default)]
pub(crate) struct Caps {
    frame: u64,
    queued: Vec<(String, u32)>,
    leaving: Vec<Entity>,
    stamps: u64,
}

impl Caps {
    /// Drops a past frame's admissions and receivers.
    fn roll(&mut self, frame: u64) {
        if self.frame != frame {
            self.frame = frame;
            self.queued.clear();
            self.leaving.clear();
        }
    }

    /// The group's admissions queued this frame.
    fn queued(&self, frame: u64, group: &str) -> u32 {
        if self.frame != frame {
            return 0;
        }
        self.queued
            .iter()
            .find(|(name, _)| name == group)
            .map_or(0, |&(_, count)| count)
    }

    /// Whether `entity` is replaced at this frame's flush.
    fn leaving(&self, frame: u64, entity: Entity) -> bool {
        self.frame == frame && self.leaving.contains(&entity)
    }

    /// One more admission of the group this frame; its stamp.
    fn admit(&mut self, frame: u64, group: &str) -> Admitted {
        self.roll(frame);
        match self.queued.iter_mut().find(|(name, _)| name == group) {
            Some((_, count)) => *count += 1,
            None => self.queued.push((group.to_owned(), 1)),
        }
        self.stamp()
    }

    /// The next stamp, for a spawn live at once, which counts as a holder
    /// rather than as queued.
    fn stamp(&mut self) -> Admitted {
        self.stamps += 1;
        Admitted(self.stamps)
    }
}

/// The World's caps' frame state, inserted when missing.
fn caps_mut(world: &mut World) -> &mut Caps {
    if !world.has_resource::<Caps>() {
        world.insert_resource(Caps::default());
    }
    world.resource_mut::<Caps>()
}

/// The cap prefab `id` gives, from its registry entry; `None` for one
/// without, after one lookup.
fn cap_of(world: &World, id: &str) -> Option<Cap> {
    let value = world
        .get_resource::<PrefabRegistry>()?
        .get(id)?
        .components
        .get(CAP)?;
    serde_json::from_value::<CapDef>(value.clone())
        .ok()
        .map(Cap::from)
}

/// The frame number admissions are stamped with.
fn frame(world: &World) -> u64 {
    world.get_resource::<Time>().map_or(0, Time::frame)
}

/// `rule`'s group's counted live holders, oldest first: alive with a
/// [`Cap`] of the group, admitted or carrying a [`Source`], neither
/// despawning, nor listed to end this step, nor replaced at this frame's
/// flush, nor `besides`, nor dead when `rule` leaves the dead out.
/// Unstamped and restored holders come first by the kit's save order, then
/// this run's admissions by stamp.
fn live_holders(world: &World, rule: &Cap, besides: Option<Entity>) -> Vec<Entity> {
    let now = frame(world);
    let caps = world.get_resource::<Caps>();
    let mut holders: Vec<(u64, Entity)> = world
        .query::<(Entity, &Cap, Option<&Admitted>, Option<&Source>)>()
        .filter(|&(entity, cap, admitted, source)| {
            cap.group == rule.group
                && (admitted.is_some() || source.is_some())
                && Some(entity) != besides
                && !world.has::<Despawning>(entity)
                && !ending(world, entity)
                && !caps.is_some_and(|caps| caps.leaving(now, entity))
                && rule.holds(world, entity)
        })
        .map(|(entity, _, admitted, _)| (Admitted::rank(admitted), entity))
        .collect();
    let order = order_of(world);
    holders.sort_by_key(|&(rank, entity)| (rank, order.key(entity)));
    holders.into_iter().map(|(_, entity)| entity).collect()
}

/// A use's admissions before it spends, which [`refuses`] counts: each
/// group's added count, and the victims chosen, still counted but not
/// chosen again. It allocates only once a capped prefab is met.
#[derive(Debug, Default)]
pub(crate) struct Tally {
    added: Vec<(String, u32)>,
    chosen: Vec<Entity>,
}

impl Tally {
    /// The group's admissions so far.
    fn added(&self, group: &str) -> u32 {
        self.added
            .iter()
            .find(|(name, _)| name == group)
            .map_or(0, |&(_, count)| count)
    }
}

/// Whether a use's kit spawn of prefab `id` from `source` would be refused
/// at its cap now, against `tally`, the use's earlier admissions; one
/// admitted is added to it, its victims chosen. An ability's `place` asks
/// it for each capped spawn and place in payload order before it spends.
pub(crate) fn refuses(world: &World, id: &str, source: Entity, tally: &mut Tally) -> bool {
    let Some(cap) = cap_of(world, id) else {
        return false;
    };
    match verdict(world, &cap, Some(source), None, Some(tally)) {
        Ok(victims) => {
            tally.chosen.extend(victims);
            match tally.added.iter_mut().find(|(name, _)| *name == cap.group) {
                Some((_, count)) => *count += 1,
                None => tally.added.push((cap.group.clone(), 1)),
            }
            false
        }
        Err(_) => true,
    }
}

/// A capped spawn's verdict, with no side effect: the oldest holders to
/// end for it (none when its group has room), or its refusal's reason. A
/// spawn whose source goes at the flush (despawning, or replaced) is
/// refused first; then `besides`, a replacement's receiver, is left out
/// of the count. A use's `tally` adds its admissions to the count, its
/// chosen victims counted but not chosen again.
fn verdict(
    world: &World,
    cap: &Cap,
    source: Option<Entity>,
    besides: Option<Entity>,
    tally: Option<&Tally>,
) -> Result<Vec<Entity>, &'static str> {
    let now = frame(world);
    let caps = world.get_resource::<Caps>();
    if let Some(source) = source
        && (world.has::<Despawning>(source) || caps.is_some_and(|caps| caps.leaving(now, source)))
    {
        return Err(LEAVING);
    }
    let queued = caps.map_or(0, |caps| caps.queued(now, &cap.group))
        + tally.map_or(0, |tally| tally.added(&cap.group));
    let mut holders = live_holders(world, cap, besides);
    let count = holders.len() as u32 + queued;
    if count < cap.max {
        return Ok(Vec::new());
    }
    if cap.past == Past::Refuse {
        return Err(FULL);
    }
    if let Some(tally) = tally {
        holders.retain(|holder| !tally.chosen.contains(holder));
    }
    // As many as bring the group under its max, the admission included.
    let need = (count + 1 - cap.max) as usize;
    if need > holders.len() {
        return Err(QUEUED);
    }
    holders.truncate(need);
    Ok(holders)
}

/// Admits a kit spawn of prefab `id` from `source` (`None` for a sourceless
/// one) under its cap, after it prepared: an uncapped prefab's comes back
/// as it was; a capped one's verdict ends its victims through the kit's
/// despawn, each `culled` pushed at its centre when named, counts the
/// admission for the frame and comes back stamped. A refusal comes back as
/// its reason, for the route to trace; it ends and counts nothing.
pub(crate) fn admit(
    world: &mut World,
    id: &str,
    source: Option<Entity>,
    prepared: PreparedPrefab,
) -> Result<PreparedPrefab, &'static str> {
    admit_besides(world, id, source, None, prepared)
}

/// [`admit`] for a `become`'s replacement of `receiver`, its source: the
/// receiver is left out of its group's count, since it leaves as the
/// replacement arrives. Once the route queues the replacement it calls
/// [`leave`].
pub(crate) fn admit_replacement(
    world: &mut World,
    id: &str,
    receiver: Entity,
    prepared: PreparedPrefab,
) -> Result<PreparedPrefab, &'static str> {
    admit_besides(world, id, Some(receiver), Some(receiver), prepared)
}

fn admit_besides(
    world: &mut World,
    id: &str,
    source: Option<Entity>,
    besides: Option<Entity>,
    prepared: PreparedPrefab,
) -> Result<PreparedPrefab, &'static str> {
    let Some(cap) = cap_of(world, id) else {
        return Ok(prepared);
    };
    let victims = verdict(world, &cap, source, besides, None)?;
    cull(world, &victims);
    let now = frame(world);
    let stamp = caps_mut(world).admit(now, &cap.group);
    Ok(prepared.with((stamp,)))
}

/// Lists each of `victims` to end through the kit's despawn, pushing its
/// `culled` at its centre when named.
fn cull(world: &mut World, victims: &[Entity]) {
    for &victim in victims {
        end(world, victim, CAP);
        if cue_named(world, victim, CULLED) {
            let at = centre_of(world, victim).unwrap_or_default();
            push_kit_cue(world, KitCue::about(CULLED, victim, at));
        }
    }
}

/// A game's spawn [`admit_spawn`] admitted: the prefab to spawn now, and
/// its group's holders listed to end for it. A struct literal outside this
/// crate is refused, since later fields are additions.
#[derive(Debug)]
#[non_exhaustive]
pub struct Admission {
    /// The prefab, stamped as admitted when capped; spawn it at once
    /// ([`PreparedPrefab::spawn`]), as the count already holds it live.
    pub prepared: PreparedPrefab,
    /// The holders listed to end for it, oldest first, each `culled` cue
    /// pushed when named: the game ends each at once with its own look, or
    /// leaves it to the kit's end-of-step despawn. One the game despawns
    /// at once leaves that despawn with no trace, and its `culled` is
    /// skipped as any gone subject's cue is.
    pub culled: Vec<Entity>,
}

/// Admits a game's own spawn of `prepared`, which it spawns at once, under
/// its prefab's [`Cap`], as a kit route admits one: refused (`None`,
/// ending nothing) when `source`, the entity it comes from if any, goes at
/// the flush, when its `refuse` cap is full or when this frame's kit
/// spawns fill its group; otherwise its victims, the group's oldest
/// counted holders past the max, are listed to end and come back in
/// [`Admission::culled`], and the prefab comes back stamped, so it counts
/// as a holder once spawned, through a save and a load too. An uncapped
/// prefab comes back as it was, with no victims. The spawn takes no
/// [`Source`] from `source`: a game sets one itself.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use tungsten_core::{Plugin, PrefabData, PrefabRegistry, Schedule, World};
/// use tungsten_kit::{Admission, CapPlugin, admit_spawn, cap_refuses};
///
/// let mut world = World::new();
/// let mut prefabs = PrefabRegistry::new();
/// let caps = [
///     ("crate", json!({ "group": "crate", "max": 2 })),
///     ("mine", json!({ "group": "mine", "max": 1, "past": "refuse" })),
/// ];
/// for (id, cap) in caps {
///     let data: PrefabData = serde_json::from_value(json!({ "components": { "cap": cap } }))
///         .unwrap();
///     prefabs.insert(id.to_string(), data);
/// }
/// world.insert_resource(prefabs);
/// CapPlugin.build(&mut Schedule::new(), &mut world);
///
/// // Three crates at once: the third ends the first, which the game
/// // despawns itself.
/// let mut crates = Vec::new();
/// for _ in 0..3 {
///     let prepared = world.prepare_prefab("crate", [0.0, 0.0]).unwrap();
///     let Admission { prepared, culled, .. } = admit_spawn(&mut world, prepared, None).unwrap();
///     for victim in culled {
///         world.despawn(victim);
///     }
///     crates.push(prepared.spawn(&mut world));
/// }
/// assert!(!world.is_alive(crates[0]) && world.is_alive(crates[1]) && world.is_alive(crates[2]));
///
/// // A refusing cap, checked before a spend.
/// assert!(!cap_refuses(&world, "mine"));
/// let prepared = world.prepare_prefab("mine", [0.0, 0.0]).unwrap();
/// admit_spawn(&mut world, prepared, None).unwrap().prepared.spawn(&mut world);
/// assert!(cap_refuses(&world, "mine"));
/// let prepared = world.prepare_prefab("mine", [0.0, 0.0]).unwrap();
/// assert!(admit_spawn(&mut world, prepared, None).is_none());
/// ```
#[must_use]
pub fn admit_spawn(
    world: &mut World,
    prepared: PreparedPrefab,
    source: Option<Entity>,
) -> Option<Admission> {
    let Some(cap) = cap_of(world, prepared.id()) else {
        return Some(Admission {
            prepared,
            culled: Vec::new(),
        });
    };
    let culled = verdict(world, &cap, source, None, None).ok()?;
    cull(world, &culled);
    let stamp = caps_mut(world).stamp();
    Some(Admission {
        prepared: prepared.with((stamp,)),
        culled,
    })
}

/// Whether a spawn of prefab `prefab` would now meet a full `refuse` cap,
/// this frame's queued kit spawns counted: for a game that checks its
/// refusing caps before it spends, in its own order. An uncapped prefab, a
/// cap whose oldest end past it and a group with room read false.
#[must_use]
pub fn cap_refuses(world: &World, prefab: &str) -> bool {
    cap_of(world, prefab).is_some_and(|cap| {
        cap.past == Past::Refuse && verdict(world, &cap, None, None, None).is_err()
    })
}

/// Lists `receiver` as replaced at this frame's flush, once a `become`
/// queued its replacement: later admissions this frame neither count it,
/// nor end it, nor spawn from it. A World without [`CapPlugin`] lists
/// nothing.
pub(crate) fn leave(world: &mut World, receiver: Entity) {
    let now = frame(world);
    if let Some(caps) = world.get_resource_mut::<Caps>() {
        caps.roll(now);
        if !caps.leaving.contains(&receiver) {
            caps.leaving.push(receiver);
        }
    }
}

/// Registers the `cap` scene component, rule data with no system of its
/// own, the admission's saved mark (`cap_admitted`) and the caps' frame
/// state; it builds [`SourcePlugin`], so a capped World derives and saves
/// [`Source`], and adds the kit's end-of-step despawn, through which a cap
/// ends its victims, when the schedule lacks it.
#[derive(Debug, Default, Clone, Copy)]
pub struct CapPlugin;

impl Plugin for CapPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        SourcePlugin.build(schedule, world);
        ensure_kit_despawn(schedule, world);
        register_rows::<Cap>(world, "Cap");
        world.register_saved_with(
            CAP_ADMITTED,
            SaveOptions::<Admitted>::form::<AdmittedSave>(),
        );
        if !world.has_resource::<Caps>() {
            world.insert_resource(Caps::default());
        }
        describe(
            world,
            ComponentDescription::new(
                CAP,
                "Holds its group's live instances to a number: a kit spawn or a game's admitted one past it ends the group's oldest, or is refused.",
            )
            .field(FieldDescription::new(
                "group",
                FieldType::Text,
                "the group's name, dotted: the live holders the kit or a game admitted, through a load too, or that hold a `Source`, a map-placed one aside",
            ))
            .field(FieldDescription::new(
                "max",
                FieldType::U32 {
                    min: Some(1),
                    max: None,
                },
                "the most live instances, counting the frame's admissions",
            ))
            .field(
                FieldDescription::new(
                    "past",
                    FieldType::Enum(&["oldest", "refuse"]),
                    "past the max: the oldest end, restored ones first, then by admission, each sending `culled` when named, or the spawn is refused",
                )
                .default("\"oldest\""),
            )
            .field(
                FieldDescription::new(
                    "count_dead",
                    FieldType::Bool,
                    "whether a holder whose `health` is dead counts and can be ended for room; false leaves it aside",
                )
                .default("true"),
            )
            .genre(RPG)
            .genre(SHOOTER)
            .sample(r#"{ "group": "salt", "max": 2 }"#)
            .roundtrip(round_trip::<CapDef>),
        );
        world.register_component_with_assets(
            CAP,
            |_: &CapDef, _: &World| Ok(()),
            |def: CapDef, _: &SpawnContext<'_>| (Cap::from(def),),
        );
    }
}

impl Inspectable for Cap {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let past = match self.past {
            Past::Oldest => "the oldest end",
            Past::Refuse => "refused",
        };
        let dead = if self.count_dead {
            ""
        } else {
            ", the dead aside"
        };
        vec![
            ("group", self.group.clone()),
            ("max", format!("{}, past it {past}{dead}", self.max)),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/cap.rs"]
mod tests;

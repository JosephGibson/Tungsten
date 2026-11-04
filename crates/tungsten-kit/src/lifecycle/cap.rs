//! Caps: a group's live instances held to a number, a kit spawn past it
//! ending the group's oldest or refused (redo 7.10c, S-18).

use serde::{Deserialize, Serialize};
use tungsten_core::{
    Entity, Inspectable, Plugin, PrefabRegistry, PreparedPrefab, Schedule, SpawnContext, Time,
    World,
};

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
/// or `roll`, a spawner, chemistry's `spawn` and `become`) counts the
/// group's live holders the kit admitted or that carry a [`Source`], so a
/// map-placed one stays aside, less those ending this step or replaced
/// this frame, plus the group's admissions queued this frame, which land at
/// its one flush. At `max`, [`Past::Oldest`] ends the oldest holders,
/// restored ones by save order first, then by admission, through the kit's
/// end-of-step despawn, sending `culled` about each when named, and
/// refuses when this frame's admissions fill the group; [`Past::Refuse`]
/// refuses the spawn. A spawn from a source that goes at the flush is
/// refused, and a spawner gets a refused spawn back. Each spawn reads its
/// own prefab's cap against its group's one count. It is rule data, not
/// saved; a World without a capped prefab does no cap work.
///
/// Scene name `cap`: `{ "group": "salt", "max": 2, "past": "oldest" }`,
/// `past` `oldest` by default.
///
/// # Examples
///
/// ```
/// use tungsten_kit::{Cap, Past};
///
/// let mines = Cap::new("mines", 3).refusing();
/// assert_eq!((mines.group(), mines.max(), mines.past()), ("mines", 3, Past::Refuse));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Cap {
    group: String,
    max: u32,
    past: Past,
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
        }
    }

    /// The cap, refusing a spawn past its max.
    #[must_use]
    pub fn refusing(mut self) -> Self {
        self.past = Past::Refuse;
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
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CapFields {
    group: String,
    max: u32,
    #[serde(default)]
    past: PastDef,
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
        }
    }
}

/// A kit admission's stamp, its place in the admission order: a component
/// an admitted spawn takes, unsaved and undescribed, so a holder the kit
/// spawned with no source counts, and holders age by admission whatever
/// entity indices they took.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Admitted(u64);

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
        self.stamps += 1;
        Admitted(self.stamps)
    }
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

/// `group`'s counted live holders, oldest first: alive with a [`Cap`] of
/// the group, admitted by the kit or carrying a [`Source`], neither
/// despawning, nor listed to end this step, nor replaced at this frame's
/// flush, nor `besides`. Unstamped holders come first by the kit's save
/// order, then stamped ones by stamp.
fn live_holders(world: &World, group: &str, besides: Option<Entity>) -> Vec<Entity> {
    let now = frame(world);
    let caps = world.get_resource::<Caps>();
    let mut holders: Vec<(Option<Admitted>, Entity)> = world
        .query::<(Entity, &Cap, Option<&Admitted>, Option<&Source>)>()
        .filter(|&(entity, cap, admitted, source)| {
            cap.group == group
                && (admitted.is_some() || source.is_some())
                && Some(entity) != besides
                && !world.has::<Despawning>(entity)
                && !ending(world, entity)
                && !caps.is_some_and(|caps| caps.leaving(now, entity))
        })
        .map(|(entity, _, admitted, _)| (admitted.copied(), entity))
        .collect();
    let order = order_of(world);
    holders.sort_by_key(|&(admitted, entity)| (admitted, order.key(entity)));
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
    let mut holders = live_holders(world, &cap.group, besides);
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
    for victim in victims {
        end(world, victim, CAP);
        if cue_named(world, victim, CULLED) {
            let at = centre_of(world, victim).unwrap_or_default();
            push_kit_cue(world, KitCue::about(CULLED, victim, at));
        }
    }
    let now = frame(world);
    if !world.has_resource::<Caps>() {
        world.insert_resource(Caps::default());
    }
    let stamp = world.resource_mut::<Caps>().admit(now, &cap.group);
    Ok(prepared.with((stamp,)))
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
/// own, and the caps' frame state; it builds [`SourcePlugin`], so a capped
/// World derives and saves [`Source`], and adds the kit's end-of-step
/// despawn, through which a cap ends its victims, when the schedule lacks
/// it.
#[derive(Debug, Default, Clone, Copy)]
pub struct CapPlugin;

impl Plugin for CapPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        SourcePlugin.build(schedule, world);
        ensure_kit_despawn(schedule, world);
        register_rows::<Cap>(world, "Cap");
        if !world.has_resource::<Caps>() {
            world.insert_resource(Caps::default());
        }
        describe(
            world,
            ComponentDescription::new(
                CAP,
                "Holds its group's live instances to a number: a kit spawn past it ends the group's oldest, or is refused.",
            )
            .field(FieldDescription::new(
                "group",
                FieldType::Text,
                "the group's name, dotted: the live holders the kit spawned or that hold a `Source`, a map-placed one aside",
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
        vec![
            ("group", self.group.clone()),
            ("max", format!("{}, past it {past}", self.max)),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/cap.rs"]
mod tests;

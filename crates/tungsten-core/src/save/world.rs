//! Capture and restore: a world's saved entities, their references and the
//! registered resources, in one call each.
//!
//! A [`WorldSave`] is plain JSON a game's `SaveData` type holds as a field;
//! the envelope and slots stay `D-160`'s. Its keys sort and a run's spawns
//! go by ID, so two captures of one world are byte-equal.

use std::collections::{BTreeMap, HashMap};

use glam::Vec2;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use super::id::{AUTHORED, SaveId, SaveState, Saved};
use super::patch::{diff, merge};
use super::registry::{Entry, Holder, Refs, SaveRegistry};
use crate::assets::{PrefabError, PreparedPrefab};
use crate::ecs::{Bundle, CommandBuffer, Entity, World};
use crate::physics::{Position, teleport};

/// The format this build writes and reads.
pub const FORMAT: u32 = 1;

/// The saved component a body's place restores through
/// [`teleport`](crate::physics::teleport), so its history moves with it.
const POSITION: &str = "position";

/// A world's save: what [`capture_world`] wrote and [`restore_world`]
/// reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldSave {
    format: u32,
    next_id: u64,
    versions: Versions,
    global: AreaSave,
    areas: BTreeMap<String, AreaSave>,
    resources: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    resource_refs: BTreeMap<String, Refs>,
    /// References the capture dropped: entities without a save ID.
    #[serde(skip)]
    dropped_refs: usize,
}

impl WorldSave {
    /// The run's next spawn ID as captured.
    #[must_use]
    pub fn next_id(&self) -> u64 {
        self.next_id
    }

    /// References the capture dropped, each to an entity without a save ID.
    #[must_use]
    pub fn dropped_refs(&self) -> usize {
        self.dropped_refs
    }

    /// The areas it holds, by name; the global record is not one.
    pub fn area_names(&self) -> impl Iterator<Item = &str> {
        self.areas.keys().map(String::as_str)
    }
}

/// Each saved component's and resource's version, as written.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct Versions {
    components: BTreeMap<String, u32>,
    resources: BTreeMap<String, u32>,
}

/// One area's record, or the global one.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct AreaSave {
    #[serde(default)]
    pub(crate) named: BTreeMap<String, NamedRecord>,
    #[serde(default)]
    pub(crate) gone: Vec<String>,
    #[serde(default)]
    pub(crate) spawns: Vec<SpawnRecord>,
}

/// A named entity's changes over its fresh spawn.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct NamedRecord {
    /// Each component's patch over its value at the naming; whole for one
    /// the entity gained since.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) components: BTreeMap<String, Value>,
    /// Components the fresh spawn had and the run took off.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) removed: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) refs: BTreeMap<String, Refs>,
}

impl NamedRecord {
    fn is_empty(&self) -> bool {
        self.components.is_empty() && self.removed.is_empty() && self.refs.is_empty()
    }
}

/// A run's spawn, in full.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct SpawnRecord {
    pub(crate) id: SaveId,
    pub(crate) prefab: String,
    pub(crate) components: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) refs: BTreeMap<String, Refs>,
}

/// Why [`capture_world`] failed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum CaptureError {
    /// The World's command buffer holds commands: a capture runs at a
    /// frame boundary, after the flush.
    #[error("the command buffer holds {commands} commands; capture after the flush")]
    Queued {
        /// The commands waiting.
        commands: usize,
    },
    /// A value's own `Serialize` failed.
    #[error("{what} failed to serialize: {message}")]
    Serialize {
        /// The value: an entity's component or a resource.
        what: String,
        /// Why.
        message: String,
    },
}

/// Why [`restore_world`] refused a save; the World is as it was.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum RestoreError {
    /// A format this build does not read.
    #[error("world save format {found}; this build reads format {FORMAT}")]
    Format {
        /// The format it names.
        found: u32,
    },
    /// The run's counter at or past the authored IDs.
    #[error("world save's next ID {next_id} reaches the authored IDs")]
    NextId {
        /// The counter it holds.
        next_id: u64,
    },
    /// A value saved at a version newer than the registry's.
    #[error("saved {kind} `{name}` is version {saved}, newer than this build's {current}")]
    NewerVersion {
        /// `component` or `resource`.
        kind: &'static str,
        /// Its name.
        name: String,
        /// The version saved.
        saved: u32,
        /// The registry's.
        current: u32,
    },
    /// A migration step failed.
    #[error("{what} failed to migrate: {message}")]
    Migration {
        /// The value.
        what: String,
        /// Why.
        message: String,
    },
    /// A value does not fit its type.
    #[error("{what} does not load: {message}")]
    Value {
        /// The value.
        what: String,
        /// Why.
        message: String,
    },
    /// A spawn's prefab failed to prepare.
    #[error("{what}: {message}")]
    Prefab {
        /// The spawn.
        what: String,
        /// Why.
        message: String,
    },
}

/// What [`restore_world`] dropped: a line each for a component name the
/// registry lacks, a prefab the manifest lacks, a named record whose entity
/// the level no longer has, and a reference that did not resolve.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct RestoreReport {
    /// One line per dropped item.
    pub dropped: Vec<String>,
}

impl RestoreReport {
    /// Whether nothing was dropped.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.dropped.is_empty()
    }
}

/// Captures the World: each saved entity's registered components (a named
/// one's as changes over its fresh spawn, a run's spawn in full), its
/// references by save ID, the named entities gone since their naming, each
/// registered resource whole and the run's counter. A World with no save
/// state captures its resources alone.
///
/// # Errors
///
/// [`CaptureError::Queued`] while the World's command buffer holds
/// commands, since a queued spawn's cost is already paid;
/// [`CaptureError::Serialize`] when a value's own `Serialize` fails.
pub fn capture_world(world: &World) -> Result<WorldSave, CaptureError> {
    if let Some(buffer) = world.get_resource::<CommandBuffer>()
        && !buffer.is_empty()
    {
        return Err(CaptureError::Queued {
            commands: buffer.len(),
        });
    }
    let empty = SaveRegistry::new();
    let registry = world.get_resource::<SaveRegistry>().unwrap_or(&empty);
    let ids = |entity: Entity| world.get::<Saved>(entity).map(|saved| saved.id);
    let mut capture = Capture {
        global: AreaSave::default(),
        areas: BTreeMap::new(),
        dropped: 0,
    };

    if let Some(state) = world.get_resource::<SaveState>() {
        for area in &state.loaded {
            capture.areas.entry(area.clone()).or_default();
        }
        for (&id, named) in &state.named {
            let alive = world.is_alive(named.entity)
                && world.get::<Saved>(named.entity).map(|saved| saved.id) == Some(id);
            let area = capture.area(named.area.as_deref());
            if !alive {
                area.gone.push(named.name.clone());
                continue;
            }
            let mut record = NamedRecord::default();
            let at = Holder::Entity(named.entity);
            for (name, entry) in &registry.components.entries {
                let current = entry
                    .saved
                    .to_value(world, at)
                    .transpose()
                    .map_err(|err| serialize_error(&named.name, name, &err))?;
                match (current, named.baseline.get(name)) {
                    (Some(value), Some(base)) => {
                        if let Some(patch) = diff(base, &value) {
                            record.components.insert(name.clone(), patch);
                        }
                    }
                    (Some(value), None) => {
                        record.components.insert(name.clone(), value);
                    }
                    (None, Some(_)) => record.removed.push(name.clone()),
                    (None, None) => {}
                }
                if let Some((refs, dropped)) = entry.saved.save_refs(world, at, &ids) {
                    capture.dropped += dropped;
                    if !refs.is_empty() {
                        record.refs.insert(name.clone(), refs);
                    }
                }
            }
            if !record.is_empty() {
                let area = capture.area(named.area.as_deref());
                area.named.insert(named.name.clone(), record);
            }
        }
    }

    for (entity, saved) in world.query::<(Entity, &Saved)>() {
        let Some(prefab) = &saved.prefab else {
            continue;
        };
        let at = Holder::Entity(entity);
        let mut record = SpawnRecord {
            id: saved.id,
            prefab: prefab.clone(),
            components: BTreeMap::new(),
            refs: BTreeMap::new(),
        };
        for (name, entry) in &registry.components.entries {
            let what = format!("spawn {}", saved.id.0);
            if let Some(value) = entry.saved.to_value(world, at) {
                let value = value.map_err(|err| serialize_error(&what, name, &err))?;
                record.components.insert(name.clone(), value);
            }
            if let Some((refs, dropped)) = entry.saved.save_refs(world, at, &ids) {
                capture.dropped += dropped;
                if !refs.is_empty() {
                    record.refs.insert(name.clone(), refs);
                }
            }
        }
        capture.area(saved.area.as_deref()).spawns.push(record);
    }
    capture.global.spawns.sort_by_key(|spawn| spawn.id);
    for area in capture.areas.values_mut() {
        area.spawns.sort_by_key(|spawn| spawn.id);
    }

    let mut resources = BTreeMap::new();
    let mut resource_refs = BTreeMap::new();
    for (name, entry) in &registry.resources.entries {
        if let Some(value) = entry.saved.to_value(world, Holder::Resource) {
            let value = value.map_err(|err| CaptureError::Serialize {
                what: format!("resource `{name}`"),
                message: err.to_string(),
            })?;
            resources.insert(name.clone(), value);
        }
        if let Some((refs, dropped)) = entry.saved.save_refs(world, Holder::Resource, &ids) {
            capture.dropped += dropped;
            if !refs.is_empty() {
                resource_refs.insert(name.clone(), refs);
            }
        }
    }

    Ok(WorldSave {
        format: FORMAT,
        next_id: world
            .get_resource::<SaveState>()
            .map_or(0, SaveState::next_id),
        versions: Versions {
            components: versions(&registry.components.entries),
            resources: versions(&registry.resources.entries),
        },
        global: capture.global,
        areas: capture.areas,
        resources,
        resource_refs,
        dropped_refs: capture.dropped,
    })
}

/// A capture's records as it builds them.
struct Capture {
    global: AreaSave,
    areas: BTreeMap<String, AreaSave>,
    dropped: usize,
}

impl Capture {
    fn area(&mut self, area: Option<&str>) -> &mut AreaSave {
        match area {
            None => &mut self.global,
            Some(area) => self.areas.entry(area.to_owned()).or_default(),
        }
    }
}

fn versions(entries: &BTreeMap<String, Entry>) -> BTreeMap<String, u32> {
    entries
        .iter()
        .map(|(name, entry)| (name.clone(), entry.version))
        .collect()
}

fn serialize_error(what: &str, component: &str, err: &serde_json::Error) -> CaptureError {
    CaptureError::Serialize {
        what: format!("`{what}`'s `{component}`"),
        message: err.to_string(),
    }
}

/// Restores `save` into a World the game has spawned as a new run, its
/// entities named: each loaded area's named entities take their changes
/// over the fresh spawn (a body's moved position through
/// [`teleport`](crate::physics::teleport)), the gone despawn, each run's
/// spawn comes back from its prefab with a clone of `with` (a state's
/// marker, as a prefab spawn's `with`), its saved components and
/// its ID, the resources are replaced and the counter set; then a second
/// pass resolves the references by save ID, once every entity exists.
///
/// It checks everything first and changes nothing on an error: the
/// format, each version, each value migrated a step at a time and merged,
/// each spawn's prefab prepared. A component name the registry lacks, a
/// prefab the manifest lacks and a named record whose entity the level no
/// longer has are dropped, a line each in the report, so a save outlives a
/// content change.
///
/// # Errors
///
/// [`RestoreError`], with the World as it was.
pub fn restore_world<B: Bundle + Clone>(
    world: &mut World,
    save: &WorldSave,
    with: &B,
) -> Result<RestoreReport, RestoreError> {
    if save.format != FORMAT {
        return Err(RestoreError::Format { found: save.format });
    }
    if save.next_id >= AUTHORED {
        return Err(RestoreError::NextId {
            next_id: save.next_id,
        });
    }
    let registry = world
        .get_resource::<SaveRegistry>()
        .cloned()
        .unwrap_or_default();
    check_versions(
        "component",
        &save.versions.components,
        &registry.components.entries,
    )?;
    check_versions(
        "resource",
        &save.versions.resources,
        &registry.resources.entries,
    )?;

    let mut report = RestoreReport::default();
    let mut plan = Plan::default();
    let loaded: Vec<String> = world
        .get_resource::<SaveState>()
        .map(|state| state.loaded.iter().cloned().collect())
        .unwrap_or_default();
    let areas = std::iter::once((None, &save.global)).chain(
        save.areas
            .iter()
            .map(|(name, area)| (Some(name.as_str()), area)),
    );
    for (name, area) in areas {
        if let Some(name) = name
            && !loaded.iter().any(|loaded| loaded == name)
        {
            report
                .dropped
                .push(format!("area `{name}`: the world has not loaded it"));
            continue;
        }
        plan_area(world, &registry, save, name, area, &mut plan, &mut report)?;
    }
    for (name, value) in &save.resources {
        let Some(entry) = registry.resources.entries.get(name) else {
            report
                .dropped
                .push(format!("resource `{name}`: not a saved resource"));
            continue;
        };
        let what = format!("resource `{name}`");
        let saved = save.versions.resources.get(name).copied().unwrap_or(1);
        let value = migrate(entry, saved, value.clone(), &what)?;
        check(entry, &value, &what)?;
        plan.resources.push((entry.clone(), value));
    }

    apply(world, &registry, save, plan, with, &mut report);
    Ok(report)
}

fn check_versions(
    kind: &'static str,
    saved: &BTreeMap<String, u32>,
    entries: &BTreeMap<String, Entry>,
) -> Result<(), RestoreError> {
    for (name, &version) in saved {
        if let Some(entry) = entries.get(name)
            && version > entry.version
        {
            return Err(RestoreError::NewerVersion {
                kind,
                name: name.clone(),
                saved: version,
                current: entry.version,
            });
        }
    }
    Ok(())
}

/// What a checked restore changes, in order.
#[derive(Default)]
struct Plan {
    despawn: Vec<Entity>,
    named: Vec<NamedPlan>,
    spawns: Vec<SpawnPlan>,
    resources: Vec<(Entry, Value)>,
}

struct NamedPlan {
    entity: Entity,
    puts: Vec<(Entry, String, Value)>,
    removes: Vec<Entry>,
    refs: BTreeMap<String, Refs>,
}

struct SpawnPlan {
    id: SaveId,
    area: Option<String>,
    prefab_id: String,
    prefab: PreparedPrefab,
    puts: Vec<(Entry, String, Value)>,
    refs: BTreeMap<String, Refs>,
}

/// Checks `area`'s records against the World and adds what they change to
/// `plan`.
fn plan_area(
    world: &mut World,
    registry: &SaveRegistry,
    save: &WorldSave,
    area: Option<&str>,
    record: &AreaSave,
    plan: &mut Plan,
    report: &mut RestoreReport,
) -> Result<(), RestoreError> {
    let in_area = |name: &str| match area {
        Some(area) => format!("`{name}` in area `{area}`"),
        None => format!("`{name}`"),
    };
    let entity_of = |world: &World, name: &str| -> Option<Entity> {
        let id = SaveId::authored(name);
        let state = world.get_resource::<SaveState>()?;
        let named = state.named.get(&id)?;
        (named.name == name
            && named.area.as_deref() == area
            && world.get::<Saved>(named.entity).map(|saved| saved.id) == Some(id))
        .then_some(named.entity)
    };

    for (name, named) in &record.named {
        let Some(entity) = entity_of(world, name) else {
            report
                .dropped
                .push(format!("{}: the level no longer has it", in_area(name)));
            continue;
        };
        let mut puts = Vec::new();
        for (component, patch) in &named.components {
            let what = format!("{}'s `{component}`", in_area(name));
            let Some(entry) = registry.components.entries.get(component) else {
                report
                    .dropped
                    .push(format!("{what}: not a saved component"));
                continue;
            };
            let saved = save
                .versions
                .components
                .get(component)
                .copied()
                .unwrap_or(1);
            let patch = migrate(entry, saved, patch.clone(), &what)?;
            let value = match entry.saved.to_value(world, Holder::Entity(entity)) {
                Some(Ok(fresh)) => merge(fresh, &patch),
                _ => patch,
            };
            check(entry, &value, &what)?;
            puts.push((entry.clone(), component.clone(), value));
        }
        let mut removes = Vec::new();
        for component in &named.removed {
            match registry.components.entries.get(component) {
                Some(entry) => removes.push(entry.clone()),
                None => report.dropped.push(format!(
                    "{}'s removed `{component}`: not a saved component",
                    in_area(name)
                )),
            }
        }
        plan.named.push(NamedPlan {
            entity,
            puts,
            removes,
            refs: named.refs.clone(),
        });
    }

    for name in &record.gone {
        if let Some(entity) = entity_of(world, name) {
            plan.despawn.push(entity);
        }
    }

    for spawn in &record.spawns {
        let what = format!("spawn {} (`{}`)", spawn.id.0, spawn.prefab);
        let mut puts = Vec::new();
        for (component, value) in &spawn.components {
            let Some(entry) = registry.components.entries.get(component) else {
                report
                    .dropped
                    .push(format!("{what}'s `{component}`: not a saved component"));
                continue;
            };
            let what = format!("{what}'s `{component}`");
            let saved = save
                .versions
                .components
                .get(component)
                .copied()
                .unwrap_or(1);
            let value = migrate(entry, saved, value.clone(), &what)?;
            check(entry, &value, &what)?;
            puts.push((entry.clone(), component.clone(), value));
        }
        let at = spawn_position(&puts);
        let prefab = match world.prepare_prefab(&spawn.prefab, at) {
            Ok(prefab) => prefab,
            Err(PrefabError::Unknown { .. }) => {
                report
                    .dropped
                    .push(format!("{what}: the manifest has no such prefab"));
                continue;
            }
            Err(err) => {
                return Err(RestoreError::Prefab {
                    what,
                    message: err.to_string(),
                });
            }
        };
        plan.spawns.push(SpawnPlan {
            id: spawn.id,
            area: area.map(str::to_owned),
            prefab_id: spawn.prefab.clone(),
            prefab: prefab.unsaved(),
            puts,
            refs: spawn.refs.clone(),
        });
    }
    Ok(())
}

fn migrate(entry: &Entry, saved: u32, value: Value, what: &str) -> Result<Value, RestoreError> {
    entry
        .migrate(saved, value)
        .map_err(|message| RestoreError::Migration {
            what: what.to_owned(),
            message,
        })
}

fn check(entry: &Entry, value: &Value, what: &str) -> Result<(), RestoreError> {
    entry
        .saved
        .check(value.clone())
        .map_err(|err| RestoreError::Value {
            what: what.to_owned(),
            message: err.to_string(),
        })
}

/// A spawn's top-left from its saved `transform`, else its `position`.
fn spawn_position(puts: &[(Entry, String, Value)]) -> Vec2 {
    let point = |value: &Value| -> Option<Vec2> {
        let [x, y] = <[f32; 2]>::deserialize(value).ok()?;
        Some(Vec2::new(x, y))
    };
    let find = |name: &str| puts.iter().find(|(_, put, _)| put == name);
    find("transform")
        .and_then(|(_, _, value)| point(value.get("position")?))
        .or_else(|| find(POSITION).and_then(|(_, _, value)| point(value)))
        .unwrap_or(Vec2::ZERO)
}

/// Writes `puts` on `entity`: its position first, through
/// [`teleport`](crate::physics::teleport), then the rest, so a saved
/// velocity or transform overrides what the teleport set.
fn put_all(world: &mut World, entity: Entity, puts: Vec<(Entry, String, Value)>) {
    let (position, rest): (Vec<_>, Vec<_>) =
        puts.into_iter().partition(|(_, name, _)| name == POSITION);
    for (entry, _, value) in position.into_iter().chain(rest) {
        let is_position = entry.saved.has(world, Holder::Entity(entity))
            && world.has::<Position>(entity)
            && entry.type_name == std::any::type_name::<Position>();
        // Checked before anything changed, so it loads.
        let loaded = entry.saved.put(world, Holder::Entity(entity), value);
        debug_assert!(loaded.is_ok(), "a checked value failed to load");
        if is_position && let Some(&Position(at)) = world.get::<Position>(entity) {
            teleport(world, entity, at);
        }
    }
}

fn apply<B: Bundle + Clone>(
    world: &mut World,
    registry: &SaveRegistry,
    save: &WorldSave,
    plan: Plan,
    with: &B,
    report: &mut RestoreReport,
) {
    if !world.has_resource::<SaveState>() {
        world.insert_resource(SaveState::default());
    }
    for entity in plan.despawn {
        world.despawn(entity);
    }
    let mut holders: Vec<(Entity, BTreeMap<String, Refs>)> = Vec::new();
    for named in plan.named {
        put_all(world, named.entity, named.puts);
        for entry in named.removes {
            entry.saved.remove(world, Holder::Entity(named.entity));
        }
        holders.push((named.entity, named.refs));
    }
    for spawn in plan.spawns {
        let entity = spawn.prefab.with(with.clone()).spawn(world);
        let kept: Vec<String> = spawn.puts.iter().map(|(_, name, _)| name.clone()).collect();
        put_all(world, entity, spawn.puts);
        for (name, entry) in &registry.components.entries {
            if !kept.contains(name) {
                entry.saved.remove(world, Holder::Entity(entity));
            }
        }
        world.insert(
            entity,
            Saved {
                id: spawn.id,
                area: spawn.area,
                prefab: Some(spawn.prefab_id),
            },
        );
        holders.push((entity, spawn.refs));
    }
    for (entry, value) in plan.resources {
        let loaded = entry.saved.put(world, Holder::Resource, value);
        debug_assert!(loaded.is_ok(), "a checked value failed to load");
    }
    world.resource_mut::<SaveState>().next_id = save.next_id;

    // The second pass: every entity exists, so references resolve.
    let by_id: HashMap<SaveId, Entity> = world
        .query::<(Entity, &Saved)>()
        .map(|(entity, saved)| (saved.id, entity))
        .collect();
    for (entity, refs) in holders {
        resolve(
            world,
            registry,
            &by_id,
            Holder::Entity(entity),
            &refs,
            false,
            report,
        );
    }
    resolve(
        world,
        registry,
        &by_id,
        Holder::Resource,
        &save.resource_refs,
        true,
        report,
    );
}

/// Loads each value's references that resolve; a line each for the rest.
fn resolve(
    world: &mut World,
    registry: &SaveRegistry,
    by_id: &HashMap<SaveId, Entity>,
    holder: Holder,
    refs: &BTreeMap<String, Refs>,
    resources: bool,
    report: &mut RestoreReport,
) {
    let entries = if resources {
        &registry.resources.entries
    } else {
        &registry.components.entries
    };
    for (name, refs) in refs {
        let Some(entry) = entries.get(name).filter(|entry| entry.saved.has_refs()) else {
            report
                .dropped
                .push(format!("`{name}`'s references: no reference codec"));
            continue;
        };
        let mut resolved: BTreeMap<String, Vec<Entity>> = BTreeMap::new();
        for (key, ids) in refs {
            for id in ids {
                match by_id.get(id) {
                    Some(&entity) => resolved.entry(key.clone()).or_default().push(entity),
                    None => report.dropped.push(format!(
                        "`{name}`'s reference `{key}` to save ID {}: no such entity",
                        id.0
                    )),
                }
            }
        }
        entry.saved.load_refs(world, holder, &resolved);
    }
}

#[cfg(test)]
#[path = "../tests/save/world.rs"]
mod tests;

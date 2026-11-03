//! World sources: named lines that reach every entity whose tags pass a
//! filter, live (§3.4 of the ARPG design), such as a game's difficulty.

use tungsten_core::{Entity, World};

use crate::foundation::lines::Line;
use crate::foundation::stats::{Stats, valid_stat};
use crate::foundation::tags::{TagFilter, Tags, valid_tag};

/// The most names a world holds sources under, those it held included.
const MOST: usize = 64;

/// A world source: a [`TagFilter`] and the lines it puts on the [`Stats`] of
/// every entity whose tags pass it, under the name the
/// [`WorldSources`] holds it by.
///
/// # Examples
///
/// ```
/// use tungsten_kit::{Line, Modifier, Stats, TagFilter, WorldSource};
///
/// let hard = WorldSource::new(TagFilter::new().all(["team.enemy"]))
///     .with_line(Stats::MAX_HEALTH, Line::new(Modifier::Mul(1.5)))
///     .with_line(Stats::DAMAGE, Line::new(Modifier::Mul(1.5)));
/// let stats: Vec<&str> = hard.lines().map(|(stat, _)| stat).collect();
/// assert_eq!(stats, ["damage", "max_health"], "by stat");
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WorldSource {
    filter: TagFilter,
    /// Sorted by stat, in add order within one.
    lines: Vec<(String, Line)>,
}

impl WorldSource {
    /// A source for the entities whose tags pass `filter`, with no line.
    #[must_use]
    pub fn new(filter: TagFilter) -> Self {
        Self {
            filter,
            lines: Vec::new(),
        }
    }

    /// The source, also putting `line` on `stat`.
    ///
    /// # Panics
    ///
    /// When `stat` is not `[a-z][a-z0-9_]*`.
    #[must_use]
    #[track_caller]
    pub fn with_line(mut self, stat: &str, line: Line) -> Self {
        assert!(
            valid_stat(stat),
            "WorldSource::with_line: `{stat}` is not a stat name (`[a-z][a-z0-9_]*`)"
        );
        let at = self
            .lines
            .partition_point(|(name, _)| name.as_str() <= stat);
        self.lines.insert(at, (stat.to_owned(), line));
        self
    }

    /// Its filter.
    #[must_use]
    pub fn filter(&self) -> &TagFilter {
        &self.filter
    }

    /// Its lines with their stats, by stat and then in add order.
    pub fn lines(&self) -> impl Iterator<Item = (&str, &Line)> {
        self.lines.iter().map(|(stat, line)| (stat.as_str(), line))
    }
}

/// The world's sources by name, a resource the first
/// [`set`](Self::set) inserts. A source set or removed reaches every
/// entity with [`Stats`] at the next step's `stats_apply`, or one entity at
/// once through [`apply_to`](Self::apply_to): its lines leave each entity
/// under the source's name, and come back on those whose tags pass its
/// filter, read then, so a tag changed later needs `apply_to`. An entity
/// whose lines from a source already equal the source's keeps them where
/// they are, so a load followed by the run's `set`, or a repeated `set`,
/// moves no modifier. A name once held stays known, so a removed source's
/// lines leave too; a world holds at most 64 names.
///
/// It is not saved: a game sets its sources at a run's start, a load's
/// included, and the lines they placed save with each entity's `Stats`.
///
/// # Examples
///
/// ```
/// use tungsten_core::{Plugin, Schedule, Stage, Time, World};
/// use tungsten_kit::{
///     Line, Modifier, Stats, StatsPlugin, TagFilter, Tags, WorldSource, WorldSources,
/// };
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// StatsPlugin::new().build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
/// let grunt = world.spawn_with((Tags::new(["team.enemy"]), Stats::new().with(Stats::DAMAGE, 10.0)));
///
/// let hard = WorldSource::new(TagFilter::new().all(["team.enemy"]))
///     .with_line(Stats::DAMAGE, Line::new(Modifier::Mul(1.5)));
/// WorldSources::set(&mut world, "difficulty", hard);
/// WorldSources::apply_to(&mut world, grunt);
/// assert_eq!(world.get::<Stats>(grunt).unwrap().get(Stats::DAMAGE), Some(15.0));
///
/// WorldSources::remove(&mut world, "difficulty");
/// world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
/// schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
/// assert_eq!(world.get::<Stats>(grunt).unwrap().get(Stats::DAMAGE), Some(10.0));
/// ```
#[derive(Debug, Default)]
pub struct WorldSources {
    /// Every name held, sorted, `None` once removed.
    sources: Vec<(String, Option<WorldSource>)>,
    /// Bumped by each change.
    revision: u64,
}

impl WorldSources {
    /// Sets the source `name` to `source`, inserting the resource when it
    /// is missing; a source equal to the one held changes nothing.
    ///
    /// # Panics
    ///
    /// When `name` is not a tag name, or a 65th name would be held.
    #[track_caller]
    pub fn set(world: &mut World, name: &str, source: WorldSource) {
        assert!(
            valid_tag(name),
            "WorldSources::set: `{name}` is not a source name (`[a-z][a-z0-9_]*` segments joined by dots)"
        );
        if !world.has_resource::<WorldSources>() {
            world.insert_resource(WorldSources::default());
        }
        let sources = world.resource_mut::<WorldSources>();
        match sources.find(name) {
            Ok(at) => {
                if sources.sources[at].1.as_ref() != Some(&source) {
                    sources.sources[at].1 = Some(source);
                    sources.revision += 1;
                }
            }
            Err(at) => {
                assert!(
                    sources.sources.len() < MOST,
                    "WorldSources::set: a world holds at most {MOST} source names"
                );
                sources.sources.insert(at, (name.to_owned(), Some(source)));
                sources.revision += 1;
            }
        }
    }

    /// Removes the source `name`, its lines leaving the entities that
    /// carry them; whether it was set.
    pub fn remove(world: &mut World, name: &str) -> bool {
        let Some(sources) = world.get_resource_mut::<WorldSources>() else {
            return false;
        };
        let Ok(at) = sources.find(name) else {
            return false;
        };
        if sources.sources[at].1.take().is_some() {
            sources.revision += 1;
            true
        } else {
            false
        }
    }

    /// Brings `entity`'s [`Stats`] up to date with every source at once,
    /// for a spawn a game's code reads before the next step; nothing for an
    /// entity without them.
    pub fn apply_to(world: &mut World, entity: Entity) {
        let Some(resource) = world.get_resource_mut::<WorldSources>() else {
            return;
        };
        let sources = std::mem::take(resource);
        let passing = sources.passing(world.get::<Tags>(entity));
        if let Some(stats) = world.get_mut::<Stats>(entity) {
            sources.bring(stats, passing);
        }
        *world.resource_mut::<WorldSources>() = sources;
    }

    /// The source `name`, while it is set.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&WorldSource> {
        self.find(name)
            .ok()
            .and_then(|at| self.sources[at].1.as_ref())
    }

    /// Each set source with its name, by name.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &WorldSource)> {
        self.sources
            .iter()
            .filter_map(|(name, source)| source.as_ref().map(|source| (name.as_str(), source)))
    }

    /// The count of changes so far; entities' stats are brought to it.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    fn find(&self, name: &str) -> Result<usize, usize> {
        self.sources
            .binary_search_by(|(held, _)| held.as_str().cmp(name))
    }

    /// Which sources pass `tags`, a bit per name held.
    fn passing(&self, tags: Option<&Tags>) -> u64 {
        self.sources
            .iter()
            .enumerate()
            .filter(|(_, (_, source))| {
                source
                    .as_ref()
                    .is_some_and(|source| source.filter.matches(tags))
            })
            .fold(0, |bits, (at, _)| bits | 1 << at)
    }

    /// Brings `stats` up to date: under each name, the source's lines when
    /// its bit in `passing` is set, none otherwise, lines already equal
    /// kept in place.
    fn bring(&self, stats: &mut Stats, passing: u64) {
        for (at, (name, source)) in self.sources.iter().enumerate() {
            let wanted: &[(String, Line)] = match source {
                Some(source) if passing & (1 << at) != 0 => &source.lines,
                _ => &[],
            };
            if !stats.holds_exactly(name, wanted) {
                stats.remove_modifiers(name);
                for (stat, line) in wanted {
                    stats.add_line(stat, name, line.clone());
                }
            }
        }
        stats.set_revision(self.revision);
    }
}

/// Brings every entity's [`Stats`] that lags `revision` up to date, for
/// `stats_apply` on a step that met one.
pub(crate) fn bring_stale(world: &mut World, revision: u64) {
    let Some(resource) = world.get_resource_mut::<WorldSources>() else {
        return;
    };
    let sources = std::mem::take(resource);
    for (stats, tags) in world.query_mut::<(&mut Stats, Option<&Tags>)>() {
        if stats.revision() != revision {
            sources.bring(stats, sources.passing(tags));
        }
    }
    *world.resource_mut::<WorldSources>() = sources;
}

#[cfg(test)]
#[path = "../tests/world_sources.rs"]
mod tests;

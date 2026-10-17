//! Targets: what an item follows, circles, steers toward or aims at, named
//! in data as the spawn chain's origin, the nearest tagged entity or a
//! world point.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::Position;
use tungsten_core::{Entity, MissingAsset, Transform, World};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, describe_shape, round_trip,
};
use crate::foundation::optional_vec2;
use crate::foundation::source::Source;
use crate::foundation::tags::{TagFilter, Tags, check_declared, valid_tag};
use crate::motion::drive::{placed_point, point_of};

/// The shape a target field names.
pub(crate) const TARGET_SHAPE: &str = "target";

/// The shape of a target's `nearest` form.
const NEAREST_SHAPE: &str = "nearest";

/// What an item follows, circles, steers toward or aims at: the entity's
/// [`Source`](crate::Source), the spawn chain's origin, so a blade circles
/// the wielder whose spawner made it; the nearest other entity whose
/// [`Tags`](crate::Tags) pass a [`TagFilter`], within a range when given,
/// ties going to the lower entity index; or a world point. An item resolves
/// its target again each step, so a target that despawns or leaves the
/// range is let go and the next nearest taken.
///
/// A `nearest` filter names an `all` or an `any` tag: an entity without
/// tags passes a filter of `none` alone, which would take particles and
/// tiles for targets, so such a filter is refused.
///
/// Data form, exactly one of `{ "source": {} }`,
/// `{ "nearest": { "all": ["team.player"], "range": 320 } }` (with `any`
/// and `none` as a [`TagFilter`] takes them) and `{ "point": [640, 360] }`;
/// a scene's load checks each tag it names. There is no form for one
/// entity, since an entity ID is not data: a game that targets one entity
/// tags it, or sets the seeker's `Source`.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use serde_json::json;
/// use tungsten_kit::{TagFilter, Target};
///
/// let hero = Target::nearest(TagFilter::new().all(["team.player"])).with_range(320.0);
/// let data = serde_json::to_value(&hero).unwrap();
/// assert_eq!(data, json!({ "nearest": { "all": ["team.player"], "range": 320.0 } }));
/// assert_eq!(serde_json::from_value::<Target>(data).unwrap(), hero);
///
/// let centre: Target = serde_json::from_value(json!({ "point": [640, 360] })).unwrap();
/// assert_eq!(centre, Target::point(Vec2::new(640.0, 360.0)));
///
/// // A filter with neither `all` nor `any` would match untagged entities.
/// let loose = json!({ "nearest": { "none": ["boss"] } });
/// assert!(serde_json::from_value::<Target>(loose).is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TargetDef", into = "TargetData")]
pub struct Target {
    kind: Kind,
}

/// The three forms.
#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Source,
    Nearest {
        filter: TagFilter,
        range: Option<f32>,
    },
    Point(Vec2),
}

impl Target {
    /// The entity's [`Source`](crate::Source): the origin of the spawn
    /// chain it came from. An entity without one has no target.
    #[must_use]
    pub fn source() -> Self {
        Self { kind: Kind::Source }
    }

    /// The nearest other entity whose tags pass `filter`, at any distance
    /// until [`with_range`](Self::with_range) bounds it.
    ///
    /// # Panics
    ///
    /// When `filter` names no `all` and no `any` tag, so an entity without
    /// tags would pass it.
    #[must_use]
    #[track_caller]
    pub fn nearest(filter: TagFilter) -> Self {
        assert!(
            !filter.matches(None),
            "Target::nearest: the filter names no `all` or `any` tag, so untagged entities would pass it"
        );
        Self {
            kind: Kind::Nearest {
                filter,
                range: None,
            },
        }
    }

    /// The `nearest` target, taking only entities within `range` world
    /// pixels.
    ///
    /// # Panics
    ///
    /// When the target is not a `nearest` one, or `range` is not finite and
    /// above 0.
    #[must_use]
    #[track_caller]
    pub fn with_range(mut self, range: f32) -> Self {
        assert!(
            range.is_finite() && range > 0.0,
            "Target::with_range: range must be finite and > 0 (got {range})"
        );
        match &mut self.kind {
            Kind::Nearest { range: within, .. } => *within = Some(range),
            Kind::Source | Kind::Point(_) => {
                panic!("Target::with_range: only a `nearest` target has a range")
            }
        }
        self
    }

    /// The world point `point`.
    ///
    /// # Panics
    ///
    /// When `point` is not finite.
    #[must_use]
    #[track_caller]
    pub fn point(point: Vec2) -> Self {
        assert!(
            point.is_finite(),
            "Target::point: the point must be finite (got {point})"
        );
        Self {
            kind: Kind::Point(point),
        }
    }

    /// The target in a line, for inspector rows.
    pub(crate) fn summary(&self) -> String {
        match &self.kind {
            Kind::Source => "source".to_string(),
            Kind::Nearest { filter, range } => match range {
                Some(range) => format!("nearest {} within {range}", filter.summary()),
                None => format!("nearest {}", filter.summary()),
            },
            Kind::Point(point) => format!("point ({:.2}, {:.2})", point.x, point.y),
        }
    }
}

/// How an item measures its target's point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Measure {
    /// Where it is: its `Position`, else its `Transform.position`.
    Now,
    /// Where its movers leave it at the end of a step of this length
    /// (`placed_point`), for a placer.
    Placed(f32),
}

/// Targets resolved through one run of a system: each distinct `nearest`
/// filter's matches are gathered once, at the first seeker that names it,
/// as the tagged entities the filter passes with their points, sorted by
/// entity index; each seeker then scans its filter's matches. A run costs
/// one pass over the tagged entities per distinct filter, plus seekers
/// times matches.
#[derive(Debug, Default)]
pub(crate) struct Resolver {
    /// The run's filters and their matches; past `used`, storage kept from
    /// earlier runs.
    filters: Vec<(TagFilter, Vec<(Entity, Vec2)>)>,
    used: usize,
}

impl Resolver {
    /// Starts a run: the last run's matches are dropped, their storage kept.
    pub(crate) fn begin(&mut self) {
        self.used = 0;
    }

    /// What `target` names for `seeker`: the entity (none for a point) and
    /// its point as `measure` says. `from` is the point a `nearest` search
    /// measures distance from. `None` when nothing resolves: no `Source`, a
    /// source gone, or no other match within range.
    pub(crate) fn resolve(
        &mut self,
        world: &World,
        target: &Target,
        seeker: Entity,
        from: Vec2,
        measure: Measure,
    ) -> Option<(Option<Entity>, Vec2)> {
        let entity = match &target.kind {
            Kind::Point(point) => return Some((None, *point)),
            Kind::Source => world.get::<Source>(seeker)?.entity(),
            Kind::Nearest { filter, range } => {
                let limit = range.map_or(f32::INFINITY, |range| range * range);
                let mut nearest: Option<(Entity, f32)> = None;
                for &(entity, at) in self.matches(world, filter) {
                    let distance = at.distance_squared(from);
                    if entity != seeker
                        && distance <= limit
                        && nearest.is_none_or(|(_, best)| distance < best)
                    {
                        nearest = Some((entity, distance));
                    }
                }
                nearest?.0
            }
        };
        let at = match measure {
            Measure::Now => point_of(world, entity)?,
            Measure::Placed(dt) => placed_point(world, entity, dt)?,
        };
        Some((Some(entity), at))
    }

    /// The entities whose tags pass `filter`, with their points, sorted by
    /// entity index: gathered at the run's first call for that filter.
    fn matches(&mut self, world: &World, filter: &TagFilter) -> &[(Entity, Vec2)] {
        let index = if let Some(index) = self.filters[..self.used]
            .iter()
            .position(|(known, _)| known == filter)
        {
            index
        } else {
            if self.used == self.filters.len() {
                self.filters.push((filter.clone(), Vec::new()));
            } else {
                self.filters[self.used].0.clone_from(filter);
            }
            let found = &mut self.filters[self.used].1;
            found.clear();
            found.extend(
                world
                    .query::<(Entity, &Tags, Option<&Position>, Option<&Transform>)>()
                    .filter(|(_, tags, _, _)| filter.matches(Some(tags)))
                    .filter_map(|(entity, _, position, transform)| {
                        let at = position.map(|p| p.0).or(transform.map(|t| t.position))?;
                        Some((entity, at))
                    }),
            );
            found.sort_by_key(|(entity, _)| entity.id());
            self.used += 1;
            self.used - 1
        };
        &self.filters[index].1
    }
}

/// `{}`: a form that takes no value.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}

/// A target as data writes it, before exactly one form is checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetFields {
    #[serde(default)]
    source: Option<Empty>,
    #[serde(default)]
    nearest: Option<NearestDef>,
    #[serde(default, with = "optional_vec2")]
    point: Option<Vec2>,
}

/// A target as a scene writes it: exactly one form, its tag names kept as
/// written, so a load's error names the field as written
/// (`target.nearest.all.0`); a [`Target`] comes from it by `From`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(try_from = "TargetFields")]
pub(crate) struct TargetDef {
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<Empty>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nearest: Option<NearestDef>,
    #[serde(skip_serializing_if = "Option::is_none", with = "optional_vec2")]
    point: Option<Vec2>,
}

impl TryFrom<TargetFields> for TargetDef {
    type Error = String;

    fn try_from(fields: TargetFields) -> Result<Self, Self::Error> {
        let forms = usize::from(fields.source.is_some())
            + usize::from(fields.nearest.is_some())
            + usize::from(fields.point.is_some());
        if forms != 1 {
            return Err("exactly one of `source`, `nearest` and `point`".to_string());
        }
        if let Some(point) = fields.point
            && !point.is_finite()
        {
            return Err(format!("point: must be finite (got {point})"));
        }
        Ok(Self {
            source: fields.source,
            nearest: fields.nearest,
            point: fields.point,
        })
    }
}

impl TargetDef {
    /// Checks each tag the target names against the declared tags, naming
    /// the field under `field` (`target.nearest.all.0`).
    pub(crate) fn check(&self, world: &World, field: &str) -> Result<(), MissingAsset> {
        let Some(nearest) = &self.nearest else {
            return Ok(());
        };
        for (list, names) in [
            ("all", &nearest.all),
            ("any", &nearest.any),
            ("none", &nearest.none),
        ] {
            for (i, name) in names.iter().enumerate() {
                check_declared(world, &format!("{field}.nearest.{list}.{i}"), name)?;
            }
        }
        Ok(())
    }
}

impl From<TargetDef> for Target {
    fn from(def: TargetDef) -> Self {
        let kind = match (def.source, def.nearest, def.point) {
            (Some(_), _, _) => Kind::Source,
            (None, Some(nearest), _) => Kind::Nearest {
                filter: TagFilter::new()
                    .all(nearest.all)
                    .any(nearest.any)
                    .none(nearest.none),
                range: nearest.range,
            },
            (None, None, Some(point)) => Kind::Point(point),
            (None, None, None) => unreachable!("checked at deserialization"),
        };
        Self { kind }
    }
}

/// `target.nearest` as data writes it, before its names and range are
/// checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NearestFields {
    #[serde(default)]
    all: Vec<String>,
    #[serde(default)]
    any: Vec<String>,
    #[serde(default)]
    none: Vec<String>,
    #[serde(default)]
    range: Option<f32>,
}

/// `target.nearest`, checked: tag names, an `all` or an `any` tag, and a
/// range above 0 when given.
#[derive(Debug, Serialize, Deserialize)]
#[serde(try_from = "NearestFields")]
struct NearestDef {
    all: Vec<String>,
    any: Vec<String>,
    none: Vec<String>,
    range: Option<f32>,
}

impl TryFrom<NearestFields> for NearestDef {
    type Error = String;

    fn try_from(fields: NearestFields) -> Result<Self, Self::Error> {
        for (list, names) in [
            ("all", &fields.all),
            ("any", &fields.any),
            ("none", &fields.none),
        ] {
            if let Some(name) = names.iter().find(|name| !valid_tag(name)) {
                return Err(format!(
                    "{list}: `{name}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)"
                ));
            }
        }
        if fields.all.is_empty() && fields.any.is_empty() {
            return Err(
                "names no `all` or `any` tag, so untagged entities would pass it".to_string(),
            );
        }
        if let Some(range) = fields.range
            && !(range.is_finite() && range > 0.0)
        {
            return Err(format!("range: must be finite and > 0 (got {range})"));
        }
        Ok(Self {
            all: fields.all,
            any: fields.any,
            none: fields.none,
            range: fields.range,
        })
    }
}

/// A [`Target`] as it serializes: the filter in its own data form.
#[derive(Serialize)]
struct TargetData {
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<Empty>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nearest: Option<NearestData>,
    #[serde(skip_serializing_if = "Option::is_none", with = "optional_vec2")]
    point: Option<Vec2>,
}

/// A `nearest` form as it serializes.
#[derive(Serialize)]
struct NearestData {
    #[serde(flatten)]
    filter: TagFilter,
    #[serde(skip_serializing_if = "Option::is_none")]
    range: Option<f32>,
}

impl From<Target> for TargetData {
    fn from(target: Target) -> Self {
        let mut data = Self {
            source: None,
            nearest: None,
            point: None,
        };
        match target.kind {
            Kind::Source => data.source = Some(Empty {}),
            Kind::Nearest { filter, range } => data.nearest = Some(NearestData { filter, range }),
            Kind::Point(point) => data.point = Some(point),
        }
        data
    }
}

/// Describes the `target` shape and its `nearest` form, once each.
pub(crate) fn describe_target(world: &mut World) {
    describe_shape(
        world,
        ComponentDescription::new(
            TARGET_SHAPE,
            "What an item follows, circles, steers toward or aims at, resolved again each step.",
        )
        .one_of()
        .field(FieldDescription::new(
            "source",
            FieldType::Object("{}"),
            "the entity's `Source`: the origin of the spawn chain it came from",
        ))
        .field(FieldDescription::new(
            "nearest",
            FieldType::Shape(NEAREST_SHAPE),
            "the nearest other entity whose tags pass the filter; the load checks its tags",
        ))
        .field(FieldDescription::new(
            "point",
            FieldType::Vec2,
            "a world point",
        ))
        .sample(r#"{ "source": {} }"#)
        .roundtrip(round_trip::<TargetDef>),
    );
    let tags = || FieldType::List(Box::new(FieldType::Tag));
    describe_shape(
        world,
        ComponentDescription::new(
            NEAREST_SHAPE,
            "The nearest other entity whose tags pass a filter that names an `all` or an `any` tag; ties go to the lower entity index.",
        )
        .field(
            FieldDescription::new("all", tags(), "tags it carries every one of").default("[]"),
        )
        .field(
            FieldDescription::new("any", tags(), "tags it carries at least one of").default("[]"),
        )
        .field(FieldDescription::new("none", tags(), "tags it carries none of").default("[]"))
        .field(
            FieldDescription::new(
                "range",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "world pixels from the seeker; any distance when left out",
            )
            .default("null"),
        )
        .sample(r#"{ "all": ["team.player"] }"#)
        .roundtrip(round_trip::<NearestDef>),
    );
}

#[cfg(test)]
#[path = "../tests/target.rs"]
mod tests;

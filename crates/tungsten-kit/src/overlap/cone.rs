//! The cone: an ability's payload of rays fanned about its direction from
//! its user's centre, each cast to its first wall, hitting the hurtboxes
//! and pushing the dynamic bodies its segments meet, at once or as a front
//! widening at a speed (redo 7.10c, S-15).

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::{
    BodyKind, Collider, Position, RigidBody, Shape, SpatialFilter, apply_impulse, cast_ray_where,
};
use tungsten_core::{Entity, MissingAsset, World};

use crate::damage::health::Health;
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, describe_shape, round_trip,
};
use crate::foundation::tags::{TagFilter, Tags, check_declared, valid_tag};
use crate::foundation::{non_negative, positive};
use crate::intent::sheet::Delivery;
use crate::lifecycle::ended::Despawning;
use crate::motion::drive::{centre_of, point_of};
use crate::overlap::grid::{Blow, strike_from};
use crate::overlap::hit::{DamageDef, HitPayload, Hurtbox, damage_field};

/// The shape's name, which the ability payload's `cone` field names.
pub(crate) const CONE_PAYLOAD: &str = "cone_payload";

/// A cone of rays an ability's use fans evenly across ±`half_angle`
/// degrees about its direction, from its user's centre, each cast to its
/// first wall within `reach`: a solid tile or a static body, the user's own
/// body left out, so dynamic and kinematic bodies never stop a ray. A live
/// hurtbox a ray's segment meets, its tags passing the filter, is hit once
/// a use through the hit path (`Struck`, the damage request, a skill's
/// delivery as a hitbox's); a dynamic body a segment meets is pushed along
/// that ray by `push` over its mass, once. Each strength falls linearly
/// from whole at the user to `far` at the reach. With a sweep, the front
/// widens from the user at that speed, its rays cast at the use, and lands
/// each as it passes; a front ends at its reach or with its user, and is
/// not saved. It is a [`Payload::Cone`](crate::Payload::Cone).
///
/// Data form: `{ "reach": 380, "half_angle": 14, "rays": 7, "damage": 3,
/// "tags": [], "filter": {}, "knockback": 0, "sweep": null, "push": 0,
/// "far": 1 }`, `reach` and `half_angle` needed.
///
/// # Examples
///
/// ```
/// use tungsten_kit::Cone;
///
/// let spray = Cone::new(380.0, 14.0, 3.0).with_rays(7).with_far(0.5);
/// assert_eq!((spray.reach(), spray.half_angle(), spray.rays()), (380.0, 14.0, 7));
/// let wave = Cone::new(240.0, 30.0, 0.0).with_sweep(600.0).with_push(400.0);
/// assert_eq!((wave.sweep(), wave.push()), (Some(600.0), 400.0));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Cone {
    reach: f32,
    half_angle: f32,
    rays: u32,
    damage: DamageDef,
    tags: Vec<String>,
    filter: TagFilter,
    knockback: f32,
    sweep: Option<f32>,
    push: f32,
    far: f32,
}

impl Cone {
    /// A cone `reach` world pixels long, `half_angle` degrees either side
    /// of its direction, of seven rays, hitting each target for `damage`.
    ///
    /// # Panics
    ///
    /// Unless `reach` is finite and above 0, `half_angle` in (0, 180] and
    /// `damage` finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn new(reach: f32, half_angle: f32, damage: f32) -> Self {
        assert!(
            reach.is_finite() && reach > 0.0,
            "Cone::new: reach must be finite and > 0 (got {reach})"
        );
        assert!(
            half_angle > 0.0 && half_angle <= 180.0,
            "Cone::new: half_angle must be in (0, 180] (got {half_angle})"
        );
        assert!(
            damage.is_finite() && damage >= 0.0,
            "Cone::new: damage must be finite and >= 0 (got {damage})"
        );
        Self {
            reach,
            half_angle,
            rays: 7,
            damage: DamageDef::Number(damage),
            tags: Vec::new(),
            filter: TagFilter::new(),
            knockback: 0.0,
            sweep: None,
            push: 0.0,
            far: 1.0,
        }
    }

    /// The cone, of `rays` rays.
    ///
    /// # Panics
    ///
    /// When `rays` is 0.
    #[must_use]
    #[track_caller]
    pub fn with_rays(mut self, rays: u32) -> Self {
        assert!(rays >= 1, "Cone::with_rays: at least one ray");
        self.rays = rays;
        self
    }

    /// The cone, its damage of the types `tags` (`damage.cold`).
    ///
    /// # Panics
    ///
    /// When a name is not a tag name.
    #[must_use]
    #[track_caller]
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        for tag in &self.tags {
            assert!(valid_tag(tag), "Cone::with_tags: `{tag}` is not a tag name");
        }
        self
    }

    /// The cone, hitting the hurtboxes whose entity's tags pass `filter`.
    #[must_use]
    pub fn with_filter(mut self, filter: TagFilter) -> Self {
        self.filter = filter;
        self
    }

    /// The cone, pushing a target it hurts away at `speed` world pixels per
    /// second, as a hitbox's knockback.
    ///
    /// # Panics
    ///
    /// Unless `speed` is finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_knockback(mut self, speed: f32) -> Self {
        assert!(
            speed.is_finite() && speed >= 0.0,
            "Cone::with_knockback: speed must be finite and >= 0 (got {speed})"
        );
        self.knockback = speed;
        self
    }

    /// The cone, a front widening at `speed` world pixels per second.
    ///
    /// # Panics
    ///
    /// Unless `speed` is finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_sweep(mut self, speed: f32) -> Self {
        assert!(
            speed.is_finite() && speed > 0.0,
            "Cone::with_sweep: speed must be finite and > 0 (got {speed})"
        );
        self.sweep = Some(speed);
        self
    }

    /// The cone, pushing a dynamic body it meets by `speed` over its mass.
    ///
    /// # Panics
    ///
    /// Unless `speed` is finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_push(mut self, speed: f32) -> Self {
        assert!(
            speed.is_finite() && speed >= 0.0,
            "Cone::with_push: speed must be finite and >= 0 (got {speed})"
        );
        self.push = speed;
        self
    }

    /// The cone, its strengths falling to `share` of whole at its reach.
    ///
    /// # Panics
    ///
    /// Unless `share` is in [0, 1].
    #[must_use]
    #[track_caller]
    pub fn with_far(mut self, share: f32) -> Self {
        assert!(
            (0.0..=1.0).contains(&share),
            "Cone::with_far: share must be in [0, 1] (got {share})"
        );
        self.far = share;
        self
    }

    /// World pixels its rays reach.
    #[must_use]
    pub fn reach(&self) -> f32 {
        self.reach
    }

    /// Degrees either side of its direction.
    #[must_use]
    pub fn half_angle(&self) -> f32 {
        self.half_angle
    }

    /// Its rays.
    #[must_use]
    pub fn rays(&self) -> u32 {
        self.rays
    }

    /// Its front's speed, when it sweeps.
    #[must_use]
    pub fn sweep(&self) -> Option<f32> {
        self.sweep
    }

    /// Its push on a body of mass 1, world pixels per second.
    #[must_use]
    pub fn push(&self) -> f32 {
        self.push
    }

    /// Its strengths' share at its reach.
    #[must_use]
    pub fn far(&self) -> f32 {
        self.far
    }

    /// The load's check: its tags, its filter's and its damage's types.
    pub(crate) fn check(&self, world: &World, field: &str) -> Result<(), MissingAsset> {
        for (i, tag) in self.tags.iter().enumerate() {
            check_declared(world, &format!("{field}.tags.{i}"), tag)?;
        }
        for (i, name) in self.filter.names().enumerate() {
            check_declared(world, &format!("{field}.filter.{i}"), name)?;
        }
        self.damage.check(world).map_err(|missing| {
            MissingAsset::new(
                format!("{field}.{}", missing.field),
                missing.kind,
                missing.id,
            )
        })
    }

    /// Its strengths' share at `distance` from its user.
    fn share(&self, distance: f32) -> f32 {
        let along = (distance / self.reach).clamp(0.0, 1.0);
        1.0 + (self.far - 1.0) * along
    }
}

fn default_rays() -> u32 {
    7
}

fn whole() -> f32 {
    1.0
}

fn no_damage() -> DamageDef {
    DamageDef::Number(0.0)
}

/// A cone as data writes it, before its values are checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConeFields {
    #[serde(deserialize_with = "positive")]
    reach: f32,
    half_angle: f32,
    #[serde(default = "default_rays")]
    rays: u32,
    #[serde(default = "no_damage")]
    damage: DamageDef,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    filter: TagFilter,
    #[serde(default, deserialize_with = "non_negative")]
    knockback: f32,
    #[serde(default, deserialize_with = "crate::foundation::optional_positive")]
    sweep: Option<f32>,
    #[serde(default, deserialize_with = "non_negative")]
    push: f32,
    #[serde(default = "whole")]
    far: f32,
}

/// A cone as data writes it, checked.
#[derive(Serialize, Deserialize)]
#[serde(try_from = "ConeFields")]
pub(crate) struct ConeDef {
    reach: f32,
    half_angle: f32,
    rays: u32,
    damage: DamageDef,
    tags: Vec<String>,
    filter: TagFilter,
    knockback: f32,
    sweep: Option<f32>,
    push: f32,
    far: f32,
}

impl TryFrom<ConeFields> for ConeDef {
    type Error = String;

    fn try_from(fields: ConeFields) -> Result<Self, Self::Error> {
        if !(fields.half_angle > 0.0 && fields.half_angle <= 180.0) {
            return Err(format!(
                "half_angle: must be in (0, 180] (got {})",
                fields.half_angle
            ));
        }
        if fields.rays == 0 {
            return Err("rays: at least one".to_string());
        }
        if !(0.0..=1.0).contains(&fields.far) {
            return Err(format!("far: must be in [0, 1] (got {})", fields.far));
        }
        if let Some(name) = fields.tags.iter().find(|name| !valid_tag(name)) {
            return Err(format!(
                "tags: `{name}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)"
            ));
        }
        if matches!(fields.damage, DamageDef::Typed(_)) && !fields.tags.is_empty() {
            return Err(
                "tags: a `damage` map's keys type its parts, so `tags` may not sit beside it"
                    .to_string(),
            );
        }
        Ok(Self {
            reach: fields.reach,
            half_angle: fields.half_angle,
            rays: fields.rays,
            damage: fields.damage,
            tags: fields.tags,
            filter: fields.filter,
            knockback: fields.knockback,
            sweep: fields.sweep,
            push: fields.push,
            far: fields.far,
        })
    }
}

impl From<ConeDef> for Cone {
    fn from(def: ConeDef) -> Self {
        Self {
            reach: def.reach,
            half_angle: def.half_angle,
            rays: def.rays,
            damage: def.damage,
            tags: def.tags,
            filter: def.filter,
            knockback: def.knockback,
            sweep: def.sweep,
            push: def.push,
            far: def.far,
        }
    }
}

impl From<Cone> for ConeDef {
    fn from(cone: Cone) -> Self {
        Self {
            reach: cone.reach,
            half_angle: cone.half_angle,
            rays: cone.rays,
            damage: cone.damage,
            tags: cone.tags,
            filter: cone.filter,
            knockback: cone.knockback,
            sweep: cone.sweep,
            push: cone.push,
            far: cone.far,
        }
    }
}

/// A sweeping cone's front: its user, its origin and rays as cast at the
/// use, how far it has widened, and what it has hit and pushed.
#[derive(Debug)]
struct Front {
    user: Entity,
    origin: Vec2,
    rays: Vec<(Vec2, f32)>,
    radius: f32,
    cone: Cone,
    delivery: Option<Delivery>,
    hit: Vec<Entity>,
    pushed: Vec<Entity>,
}

/// The sweeping cones' fronts, a World resource the first sweep inserts,
/// which [`ABILITY_TICK`](crate::ABILITY_TICK) widens each step: empty in
/// a World with no cone, and not saved, so a front a load replaces ends
/// with its user.
#[derive(Debug, Default)]
pub(crate) struct ConeFronts(Vec<Front>);

/// Fans `cone` from `user`'s centre along `direction`: its hits and pushes
/// at once, or a front for the ticker to widen, carrying `delivery`.
pub(crate) fn fire(
    world: &mut World,
    user: Entity,
    direction: Vec2,
    cone: &Cone,
    delivery: Option<Delivery>,
) {
    let origin = centre_of(world, user).unwrap_or_default();
    let rays = cast(world, user, origin, direction, cone);
    let Some(speed) = cone.sweep else {
        let (mut hit, mut pushed) = (Vec::new(), Vec::new());
        land(
            world,
            user,
            origin,
            &rays,
            [0.0, cone.reach],
            cone,
            delivery.as_ref(),
            [&mut hit, &mut pushed],
        );
        return;
    };
    debug_assert!(speed > 0.0);
    if !world.has_resource::<ConeFronts>() {
        world.insert_resource(ConeFronts::default());
    }
    world.resource_mut::<ConeFronts>().0.push(Front {
        user,
        origin,
        rays,
        radius: 0.0,
        cone: cone.clone(),
        delivery,
        hit: Vec::new(),
        pushed: Vec::new(),
    });
}

/// Widens each front by its speed over `dt`, landing what its rays meet in
/// the band it crosses; a front ends at its reach, or as soon as its user
/// is gone or despawning. A World with no front does nothing.
pub(crate) fn widen(world: &mut World, dt: f32) {
    let Some(fronts) = world.get_resource_mut::<ConeFronts>() else {
        return;
    };
    if fronts.0.is_empty() || dt <= 0.0 {
        return;
    }
    let mut fronts = std::mem::take(&mut fronts.0);
    fronts.retain_mut(|front| {
        if !world.is_alive(front.user) || world.has::<Despawning>(front.user) {
            return false;
        }
        let speed = front.cone.sweep.unwrap_or(f32::INFINITY);
        let to = (front.radius + speed * dt).min(front.cone.reach);
        land(
            world,
            front.user,
            front.origin,
            &front.rays,
            [front.radius, to],
            &front.cone,
            front.delivery.as_ref(),
            [&mut front.hit, &mut front.pushed],
        );
        front.radius = to;
        to < front.cone.reach
    });
    let list = world.resource_mut::<ConeFronts>();
    fronts.append(&mut list.0);
    list.0 = fronts;
}

/// Each ray's unit direction and how far it runs: to its first wall, a
/// solid tile or a static body other than `user`'s, within the reach.
fn cast(
    world: &World,
    user: Entity,
    origin: Vec2,
    direction: Vec2,
    cone: &Cone,
) -> Vec<(Vec2, f32)> {
    let half = cone.half_angle.to_radians();
    let filter = SpatialFilter::new().excluding(user);
    (0..cone.rays)
        .map(|i| {
            let angle = if cone.rays == 1 {
                0.0
            } else {
                -half + 2.0 * half * i as f32 / (cone.rays - 1) as f32
            };
            let along = Vec2::from_angle(angle).rotate(direction);
            let wall = cast_ray_where(world, origin, along, cone.reach, &filter, |hit| {
                hit.entity.is_none_or(|body| {
                    world
                        .get::<RigidBody>(body)
                        .is_some_and(|body| body.kind == BodyKind::Static)
                })
            });
            (along, wall.map_or(cone.reach, |hit| hit.distance))
        })
        .collect()
}

/// Whether the segment from `a` to `b` meets `shape` centred at `centre`:
/// a circle by its radius, any other shape by its bounding box.
fn meets(a: Vec2, b: Vec2, shape: Shape, centre: Vec2) -> bool {
    if let Shape::Circle { radius } = shape {
        let ab = b - a;
        let t = if ab.length_squared() > 0.0 {
            ((centre - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
        } else {
            0.0
        };
        return (a + ab * t).distance_squared(centre) <= radius * radius;
    }
    let half = shape.half_extents();
    let (min, max) = (centre - half, centre + half);
    let delta = b - a;
    let (mut enter, mut exit) = (0.0_f32, 1.0_f32);
    for axis in 0..2 {
        let (start, step) = (a[axis], delta[axis]);
        if step.abs() < f32::EPSILON {
            if start < min[axis] || start > max[axis] {
                return false;
            }
            continue;
        }
        let (mut near, mut far) = ((min[axis] - start) / step, (max[axis] - start) / step);
        if near > far {
            std::mem::swap(&mut near, &mut far);
        }
        enter = enter.max(near);
        exit = exit.min(far);
        if enter > exit {
            return false;
        }
    }
    true
}

/// The first ray whose part within `band` meets `shape` at `centre`.
fn first_ray(
    origin: Vec2,
    rays: &[(Vec2, f32)],
    band: [f32; 2],
    shape: Shape,
    centre: Vec2,
) -> Option<Vec2> {
    rays.iter().find_map(|&(along, length)| {
        let (from, to) = (band[0].min(length), band[1].min(length));
        (from < to && meets(origin + along * from, origin + along * to, shape, centre))
            .then_some(along)
    })
}

/// Hits each live hurtbox and pushes each dynamic body that a ray's part
/// within `band` meets, each not yet in `seen` (the hit, then the pushed)
/// and then added, at the share its distance from `origin` gives.
#[allow(clippy::too_many_arguments)]
fn land(
    world: &mut World,
    user: Entity,
    origin: Vec2,
    rays: &[(Vec2, f32)],
    band: [f32; 2],
    cone: &Cone,
    delivery: Option<&Delivery>,
    seen: [&mut Vec<Entity>; 2],
) {
    let [hit, pushed] = seen;
    let targets: Vec<(Entity, Vec2, Vec2)> = world
        .query::<(Entity, &Hurtbox)>()
        .filter(|&(entity, _)| {
            entity != user
                && !hit.contains(&entity)
                && !world.has::<Despawning>(entity)
                && !world.get::<Health>(entity).is_some_and(Health::is_dead)
                && cone.filter.matches(world.get::<Tags>(entity))
        })
        .filter_map(|(entity, hurtbox)| {
            let centre = point_of(world, entity)? + hurtbox.offset();
            first_ray(origin, rays, band, hurtbox.shape().shape(), centre)
                .map(|along| (entity, centre, along))
        })
        .collect();
    if !targets.is_empty() {
        let tags = Tags::new(cone.tags.iter().map(String::as_str));
        let typed = cone.damage.parts().map(|parts| HitPayload {
            parts,
            ..HitPayload::default()
        });
        for (target, centre, along) in targets {
            hit.push(target);
            let share = cone.share(centre.distance(origin));
            strike_from(
                world,
                &Blow {
                    hitter: user,
                    target,
                    hitter_at: origin,
                    target_at: centre,
                    relative: along,
                    damage: cone.damage.total() * share,
                    tags: &tags,
                    typed: typed.as_ref(),
                    knockback: cone.knockback * share,
                },
                false,
                delivery,
            );
        }
    }
    if cone.push <= 0.0 {
        return;
    }
    let bodies: Vec<(Entity, Vec2, Vec2)> = world
        .query::<(Entity, &RigidBody, &Collider, &Position)>()
        .filter(|&(entity, body, ..)| {
            body.kind == BodyKind::Dynamic && entity != user && !pushed.contains(&entity)
        })
        .filter_map(|(entity, _, collider, position)| {
            let centre = collider.center(position.0);
            first_ray(origin, rays, band, collider.shape, centre)
                .map(|along| (entity, centre, along))
        })
        .collect();
    for (body, centre, along) in bodies {
        pushed.push(body);
        let share = cone.share(centre.distance(origin));
        apply_impulse(world, body, along * cone.push * share);
    }
}

/// The description of the `cone_payload` shape.
pub(crate) fn describe_cone(world: &mut World) {
    let f32_in = |min: Bound, max: Bound| FieldType::F32 { min, max };
    describe_shape(
        world,
        ComponentDescription::new(
            CONE_PAYLOAD,
            "Rays fanned about the use's direction from the user's centre, each to its first wall, a solid tile or a static body: the hurtboxes they meet hit once, the dynamic bodies pushed once, at once or as a widening front.",
        )
        .field(FieldDescription::new(
            "reach",
            f32_in(Bound::Exclusive(0.0), Bound::Open),
            "world pixels each ray runs, short of a wall",
        ))
        .field(FieldDescription::new(
            "half_angle",
            f32_in(Bound::Exclusive(0.0), Bound::Inclusive(180.0)),
            "degrees either side of the direction the rays fan across",
        ))
        .field(
            FieldDescription::new(
                "rays",
                FieldType::U32 {
                    min: Some(1),
                    max: None,
                },
                "rays fanned evenly across the cone",
            )
            .default("7"),
        )
        .field(damage_field().default("0.0"))
        .field(
            FieldDescription::new(
                "tags",
                FieldType::List(Box::new(FieldType::Tag)),
                "the damage's types, as a hitbox's; refused beside a `damage` map",
            )
            .default("[]"),
        )
        .field(
            FieldDescription::new(
                "filter",
                FieldType::Filter,
                "the hurtboxes it hits, by their entity's tags; the load checks its tags",
            )
            .default("{}"),
        )
        .field(
            FieldDescription::new(
                "knockback",
                f32_in(Bound::Inclusive(0.0), Bound::Open),
                "world pixels per second pushing a target it hurts away, as a hitbox's",
            )
            .default("0.0"),
        )
        .field(
            FieldDescription::new(
                "sweep",
                f32_in(Bound::Exclusive(0.0), Bound::Open),
                "world pixels per second a front widens from the user, landing each as it passes; at once when left out",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "push",
                f32_in(Bound::Inclusive(0.0), Bound::Open),
                "world pixels per second a dynamic body of mass 1 is pushed along the ray that meets it, once",
            )
            .default("0.0"),
        )
        .field(
            FieldDescription::new(
                "far",
                f32_in(Bound::Inclusive(0.0), Bound::Inclusive(1.0)),
                "the strengths' share at the reach, falling linearly from whole at the user",
            )
            .default("1.0"),
        )
        .sample(r#"{ "reach": 380, "half_angle": 14 }"#)
        .roundtrip(round_trip::<ConeDef>),
    );
}

#[cfg(test)]
#[path = "../tests/cone.rs"]
mod tests;

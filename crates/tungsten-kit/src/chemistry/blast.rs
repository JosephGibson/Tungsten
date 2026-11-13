//! The blast (redo 7.10b, K10; 7.13c): what hurts, pushes and sets off
//! what lies around its point, each by a linear falloff from it, set off by
//! a chemistry output, a projectile's end or a game's [`blast`] call.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::{
    BodyKind, Collider, Position, RigidBody, Shape, SpatialFilter, apply_impulse, overlap,
};
use tungsten_core::{Entity, MissingAsset, World};

use crate::chemistry::grid::locate;
use crate::chemistry::queue::{Chemistry, Receiver, Settings, Stimulus};
use crate::chemistry::stimuli::Reach;
use crate::damage::health::Health;
use crate::damage::pipeline::{
    DamagePart, DamageRequest, HitPrep, check_hit_prep, hit_prep, refuse_hit_prep,
};
use crate::foundation::saved::order_of;
use crate::foundation::tags::{STIMULUS_BLAST, TagFilter, Tags, check_declared, valid_tag};
use crate::lifecycle::ended::Despawning;
use crate::overlap::hit::DamageDef;

/// A blast: its radius, the damage it deals at its centre, the context its
/// hits carry, the knockback and the physics impulse at its centre, and how
/// it picks, measures and lands its hits. Its damage falls off linearly from
/// its centre to `far` of it at the radius, `1 − (1 − far) d / r`, a target
/// measured to its body's surface (its point without one); its knockback,
/// its impulse and its stimulus fall off to 0 at the radius, `1 − d / r`.
///
/// Set off, a blast:
///
/// - hurts each entity with [`Health`] within its radius, alive and not
///   being despawned, whose [`Tags`] pass its `filter`, but its receiver
///   unless it hurts that too
///   (`receiver`, at distance 0) and its credit when it spares it (`spare`):
///   one hit each, of its damage by the falloff, typed, in its `context`,
///   pushing away from its centre (straight up from it), credited to its
///   credit while that lives, its position the blast's centre with `point`
///   `centre` and none without, its game's preparation run on it
///   (`prepare`), its crit rolled from its credit's stats
///   ([`DamageRequest::roll_crit`]), in the targets' save order (unsaved
///   ones after, by entity index), and resolved at once with `land` `now`
///   or queued for the next resolve with `queue`;
/// - moves every dynamic body, and kinematic one with a mass, within its
///   radius by its impulse ([`apply_impulse`]), but its receiver, the bodies
///   it hurt, which take its push as their hit's knockback alone, the dead
///   and despawning it passed, and the credit it spares;
/// - sends `stimulus.blast` of the falloff to each tagged entity and each
///   material cell within its radius but its receiver, for chemistry's next
///   run, unless `stimulus` is false.
///
/// Data writes it as a chemistry output's `blast` or a projectile's
/// `end.blast`: `{ "radius": 96, "damage": { "damage.poison": 16 },
/// "context": ["hit.spell"], "far": 0.5, "spare": "source", "filter": {
/// "all": ["living"] }, "point": "centre", "land": "now", "prepare":
/// "armour" }`, all but `radius` optional: no damage, no context, no push,
/// no impulse, `far` 0, `receiver` false, nothing spared, every target, no
/// point, `land` `queue`, `stimulus` true and no preparation. The load checks
/// its damage types, its context and filter tags and its preparation's
/// name. A game's Rust builds the same with [`Blast::new`] and sets it off
/// with [`blast`].
///
/// # Examples
///
/// ```
/// use tungsten_kit::Blast;
///
/// let flask = Blast::new(96.0)
///     .with_part("damage.poison", 16.0)
///     .with_far(0.5)
///     .sparing_source()
///     .landing_now();
/// let read: Blast = serde_json::from_str(
///     r#"{ "radius": 96, "damage": { "damage.poison": 16 }, "far": 0.5, "spare": "source", "land": "now" }"#,
/// )
/// .unwrap();
/// assert_eq!(flask, read);
/// assert_eq!(flask.radius(), 96.0);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "BlastFields", into = "BlastFields")]
pub struct Blast {
    radius: f32,
    push: f32,
    impulse: f32,
    /// The damage's share at the radius.
    far: f32,
    damage: Option<DamageDef>,
    context: Box<[String]>,
    /// Its rarer rules, boxed and `None` at their defaults, so a chemistry
    /// output keeps its size: the run clones its rules each step.
    rules: Option<Box<Rules>>,
}

/// A blast's rarer rules: whom it hurts and how its hits land.
#[derive(Debug, Clone, Default, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // Its data's switches, each a field of its own.
struct Rules {
    /// Its receiver is a target too.
    receiver: bool,
    /// It spares its credit.
    spare_source: bool,
    filter: TagFilter,
    /// Each hit lands at its centre.
    at_centre: bool,
    /// Its hits resolve as it goes off.
    now: bool,
    /// It sends no stimulus.
    silent: bool,
    prepare: Option<String>,
}

impl PartialEq for Blast {
    /// Field by field, a blast without rules equal to one whose rules are
    /// the defaults.
    fn eq(&self, other: &Self) -> bool {
        let none = Rules::default();
        self.radius == other.radius
            && self.push == other.push
            && self.impulse == other.impulse
            && self.far == other.far
            && self.damage == other.damage
            && self.context == other.context
            && self.rules.as_deref().unwrap_or(&none) == other.rules.as_deref().unwrap_or(&none)
    }
}

/// What a blast spares, as data writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SpareField {
    Source,
}

/// Where a blast's hits land, as data writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PointField {
    Centre,
}

/// When a blast's hits resolve, as data writes it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LandField {
    #[default]
    Queue,
    Now,
}

/// `true`: a blast's `stimulus` when left out.
fn yes() -> bool {
    true
}

/// A blast as data writes it, before its values are checked.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BlastFields {
    radius: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    damage: Option<DamageDef>,
    #[serde(default)]
    context: Vec<String>,
    #[serde(default)]
    push: f32,
    #[serde(default)]
    impulse: f32,
    #[serde(default)]
    far: f32,
    #[serde(default)]
    receiver: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    spare: Option<SpareField>,
    #[serde(default)]
    filter: TagFilter,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    point: Option<PointField>,
    #[serde(default)]
    land: LandField,
    #[serde(default = "yes")]
    stimulus: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prepare: Option<String>,
}

impl TryFrom<BlastFields> for Blast {
    type Error = String;

    fn try_from(fields: BlastFields) -> Result<Self, Self::Error> {
        if !(fields.radius.is_finite() && fields.radius > 0.0) {
            return Err(format!(
                "radius: must be finite and > 0 (got {})",
                fields.radius
            ));
        }
        for (field, value) in [("push", fields.push), ("impulse", fields.impulse)] {
            if !(value.is_finite() && value >= 0.0) {
                return Err(format!("{field}: must be finite and >= 0 (got {value})"));
            }
        }
        if !(0.0..=1.0).contains(&fields.far) {
            return Err(format!("far: must be 0 to 1 (got {})", fields.far));
        }
        if let Some(tag) = fields.context.iter().find(|tag| !valid_tag(tag)) {
            return Err(format!(
                "context: `{tag}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)"
            ));
        }
        if let Some(name) = &fields.prepare
            && !crate::foundation::stats::valid_stat(name)
        {
            return Err(format!(
                "prepare: `{name}` is not a preparation name (`[a-z][a-z0-9_]*`)"
            ));
        }
        let rules = Rules {
            receiver: fields.receiver,
            spare_source: fields.spare.is_some(),
            filter: fields.filter,
            at_centre: fields.point.is_some(),
            now: fields.land == LandField::Now,
            silent: !fields.stimulus,
            prepare: fields.prepare,
        };
        Ok(Self {
            radius: fields.radius,
            push: fields.push,
            impulse: fields.impulse,
            far: fields.far,
            damage: fields.damage,
            context: fields.context.into_boxed_slice(),
            rules: (rules != Rules::default()).then(|| Box::new(rules)),
        })
    }
}

impl From<Blast> for BlastFields {
    fn from(blast: Blast) -> Self {
        let rules = blast.rules.map_or_else(Rules::default, |rules| *rules);
        Self {
            radius: blast.radius,
            damage: blast.damage,
            context: blast.context.into_vec(),
            push: blast.push,
            impulse: blast.impulse,
            far: blast.far,
            receiver: rules.receiver,
            spare: rules.spare_source.then_some(SpareField::Source),
            filter: rules.filter,
            point: rules.at_centre.then_some(PointField::Centre),
            land: if rules.now {
                LandField::Now
            } else {
                LandField::Queue
            },
            stimulus: !rules.silent,
            prepare: rules.prepare,
        }
    }
}

impl Blast {
    /// A blast of `radius` world pixels with every other field at its
    /// default: no damage, push or impulse, its stimulus sent.
    ///
    /// # Panics
    ///
    /// When `radius` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn new(radius: f32) -> Self {
        assert!(
            radius.is_finite() && radius > 0.0,
            "Blast::new: radius must be finite and > 0 (got {radius})"
        );
        Self {
            radius,
            push: 0.0,
            impulse: 0.0,
            far: 0.0,
            damage: None,
            context: Box::default(),
            rules: None,
        }
    }

    /// The blast, dealing `amount` of untyped damage at its centre in place
    /// of any it dealt.
    ///
    /// # Panics
    ///
    /// When `amount` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_damage(mut self, amount: f32) -> Self {
        assert!(
            amount.is_finite() && amount >= 0.0,
            "Blast::with_damage: amount must be finite and >= 0 (got {amount})"
        );
        self.damage = Some(DamageDef::Number(amount));
        self
    }

    /// The blast, dealing a part of `amount` of the damage type `kind`
    /// (`damage.fire`) at its centre beside its other typed parts, in place
    /// of untyped damage; its parts go in type-name order, as data's do.
    ///
    /// # Panics
    ///
    /// When `kind` is not a tag name or `amount` is not finite and at least
    /// 0.
    #[must_use]
    #[track_caller]
    pub fn with_part(mut self, kind: &str, amount: f32) -> Self {
        assert!(
            valid_tag(kind),
            "Blast::with_part: `{kind}` is not a damage type name"
        );
        assert!(
            amount.is_finite() && amount >= 0.0,
            "Blast::with_part: amount must be finite and >= 0 (got {amount})"
        );
        let mut parts = match self.damage.take() {
            Some(DamageDef::Typed(parts)) => parts,
            _ => std::collections::BTreeMap::new(),
        };
        parts.insert(kind.to_owned(), amount);
        self.damage = Some(DamageDef::Typed(parts));
        self
    }

    /// The blast, its hits in the context `tags` (`hit.blast`), which the
    /// targets' scoped resistances and `damage_taken` lines and the credit's
    /// `damage_dealt` lines read.
    ///
    /// # Panics
    ///
    /// When a tag is not a tag name.
    #[must_use]
    #[track_caller]
    pub fn with_context(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let tags: Vec<String> = tags.into_iter().map(Into::into).collect();
        if let Some(tag) = tags.iter().find(|tag| !valid_tag(tag)) {
            panic!("Blast::with_context: `{tag}` is not a tag name");
        }
        self.context = tags.into_boxed_slice();
        self
    }

    /// The blast, pushing what it hurts by `velocity` world pixels per
    /// second at its centre, away from it.
    ///
    /// # Panics
    ///
    /// When `velocity` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_push(mut self, velocity: f32) -> Self {
        assert!(
            velocity.is_finite() && velocity >= 0.0,
            "Blast::with_push: velocity must be finite and >= 0 (got {velocity})"
        );
        self.push = velocity;
        self
    }

    /// The blast, moving the bodies it does not hurt by a physics impulse of
    /// `impulse` at its centre.
    ///
    /// # Panics
    ///
    /// When `impulse` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_impulse(mut self, impulse: f32) -> Self {
        assert!(
            impulse.is_finite() && impulse >= 0.0,
            "Blast::with_impulse: impulse must be finite and >= 0 (got {impulse})"
        );
        self.impulse = impulse;
        self
    }

    /// The blast, its damage falling to `share` of it at its radius: 0 by
    /// default, 1 for the same damage everywhere within it.
    ///
    /// # Panics
    ///
    /// When `share` is not 0 to 1.
    #[must_use]
    #[track_caller]
    pub fn with_far(mut self, share: f32) -> Self {
        assert!(
            (0.0..=1.0).contains(&share),
            "Blast::with_far: share must be 0 to 1 (got {share})"
        );
        self.far = share;
        self
    }

    /// The blast, hurting its receiver too when `hurts`, at distance 0.
    #[must_use]
    pub fn with_receiver(mut self, hurts: bool) -> Self {
        self.rules_mut().receiver = hurts;
        self.tidy();
        self
    }

    /// The blast, sparing its credit: no hit, no impulse.
    #[must_use]
    pub fn sparing_source(mut self) -> Self {
        self.rules_mut().spare_source = true;
        self
    }

    /// The blast, hurting only the targets whose tags pass `filter`.
    #[must_use]
    pub fn with_filter(mut self, filter: TagFilter) -> Self {
        self.rules_mut().filter = filter;
        self.tidy();
        self
    }

    /// The blast, each hit landing at its centre: where the hit's cue
    /// spawns, and the point its preparation reads.
    #[must_use]
    pub fn at_centre(mut self) -> Self {
        self.rules_mut().at_centre = true;
        self
    }

    /// The blast, its hits resolved as it goes off, in its targets' order,
    /// rather than queued for the next resolve.
    #[must_use]
    pub fn landing_now(mut self) -> Self {
        self.rules_mut().now = true;
        self
    }

    /// The blast, sending `stimulus.blast` only when `sends`.
    #[must_use]
    pub fn with_stimulus(mut self, sends: bool) -> Self {
        self.rules_mut().silent = !sends;
        self.tidy();
        self
    }

    /// The blast, running the game's hit preparation `name`
    /// ([`add_hit_prep`](crate::add_hit_prep)) on each hit; set off while
    /// no preparation holds the name, it is refused with one warning.
    ///
    /// # Panics
    ///
    /// When `name` is not `[a-z][a-z0-9_]*`.
    #[must_use]
    #[track_caller]
    pub fn with_prepare(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        assert!(
            crate::foundation::stats::valid_stat(&name),
            "Blast::with_prepare: `{name}` is not a preparation name"
        );
        self.rules_mut().prepare = Some(name);
        self
    }

    /// Its radius, in world pixels.
    #[must_use]
    pub fn radius(&self) -> f32 {
        self.radius
    }

    /// Its rules, made at their defaults when it has none.
    fn rules_mut(&mut self) -> &mut Rules {
        self.rules.get_or_insert_with(Box::default)
    }

    /// Its rules dropped when they are back at their defaults.
    fn tidy(&mut self) {
        if self
            .rules
            .as_deref()
            .is_some_and(|rules| *rules == Rules::default())
        {
            self.rules = None;
        }
    }

    fn hurts_receiver(&self) -> bool {
        self.rules.as_ref().is_some_and(|rules| rules.receiver)
    }

    fn spares_source(&self) -> bool {
        self.rules.as_ref().is_some_and(|rules| rules.spare_source)
    }

    /// Whether a target of `tags` passes its filter.
    fn passes(&self, tags: Option<&Tags>) -> bool {
        self.rules
            .as_ref()
            .is_none_or(|rules| rules.filter.matches(tags))
    }

    fn lands_at_centre(&self) -> bool {
        self.rules.as_ref().is_some_and(|rules| rules.at_centre)
    }

    fn lands_now(&self) -> bool {
        self.rules.as_ref().is_some_and(|rules| rules.now)
    }

    fn sends_stimulus(&self) -> bool {
        self.rules.as_ref().is_none_or(|rules| !rules.silent)
    }

    fn preparation(&self) -> Option<&str> {
        self.rules
            .as_ref()
            .and_then(|rules| rules.prepare.as_deref())
    }

    /// The load's check at `field`: the damage's types, the context's and
    /// the filter's tags, and the preparation's name.
    pub(crate) fn check(&self, world: &World, field: &str) -> Result<(), MissingAsset> {
        if let Some(damage) = &self.damage {
            damage.check(world)?;
        }
        for (i, tag) in self.context.iter().enumerate() {
            check_declared(world, &format!("{field}.context.{i}"), tag)?;
        }
        if let Some(rules) = &self.rules {
            rules.filter.check(world, &format!("{field}.filter"))?;
            if let Some(name) = &rules.prepare {
                check_hit_prep(world, &format!("{field}.prepare"), name)?;
            }
        }
        Ok(())
    }

    /// Queues the blast at `centre` for chemistry's run to set off once its
    /// reactions are done: from `from`, credited to `credit`, `depth`
    /// reactions deep. A World without [`Blasts`] queues nothing.
    pub(crate) fn queue(
        &self,
        world: &mut World,
        centre: Vec2,
        from: Receiver,
        credit: Option<Entity>,
        depth: u8,
    ) {
        if let Some(blasts) = world.get_resource_mut::<Blasts>() {
            blasts.0.push(Pending {
                def: self.clone(),
                centre,
                from,
                credit,
                depth,
            });
        }
    }

    /// The damage's share at `distance`: `1 − (1 − far) d / r`, in
    /// `[0, 1]`.
    fn share(&self, distance: f32) -> f32 {
        (1.0 - (1.0 - self.far) * distance / self.radius).clamp(0.0, 1.0)
    }
}

/// Sets `blast` off at `centre` now: its hits built, its impulse applied and
/// its stimuli queued for chemistry's next run (a World without
/// [`ChemistryPlugin`](crate::ChemistryPlugin) sends none), from `receiver`,
/// which it neither hurts nor moves unless it hurts its receiver, credited
/// to `credit` while that lives, at depth 0. Its hits resolve before the
/// call returns with `land` `now`, else at the next resolve. Refused, with
/// one warning, while its `prepare` names a preparation no game registered.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::{Plugin, Schedule, Transform, World};
/// use tungsten_kit::{Blast, Health, HealthPlugin, blast};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// HealthPlugin.build(&mut schedule, &mut world);
/// let crate_ = world.spawn_with((Health::new(20.0), Transform::from_position(Vec2::new(30.0, 0.0))));
/// let keg = world.spawn_with((Health::new(5.0), Transform::from_position(Vec2::ZERO)));
/// let flat = Blast::new(60.0).with_damage(8.0).with_far(1.0).landing_now();
/// blast(&mut world, Vec2::ZERO, &flat, None, Some(keg));
/// assert_eq!(world.get::<Health>(crate_).unwrap().current(), 12.0, "8 anywhere within it");
/// assert_eq!(world.get::<Health>(keg).unwrap().current(), 5.0, "its receiver spared");
/// ```
pub fn blast(
    world: &mut World,
    centre: Vec2,
    blast: &Blast,
    credit: Option<Entity>,
    receiver: Option<Entity>,
) {
    let mut scratch = world.remove_resource::<BlastReach>().unwrap_or_default();
    // A call between runs measures the World as it is now.
    scratch.0.clear();
    let settings = Settings::of(world);
    let from = receiver.map(Receiver::Entity);
    go_off(
        world,
        &settings,
        blast,
        centre,
        from.as_ref(),
        credit,
        0,
        &mut scratch.0,
    );
    world.insert_resource(scratch);
}

/// [`blast`]'s kept lists, so a warm call allocates little: a World resource
/// it inserts, which no save holds.
#[derive(Default)]
struct BlastReach(Reach);

/// The blasts a run's reactions set off, which the run detonates once its
/// reactions are done: a World resource
/// [`ChemistryPlugin`](crate::ChemistryPlugin) inserts and no save holds,
/// since a run makes and takes them.
#[derive(Debug, Default)]
pub(crate) struct Blasts(pub(crate) Vec<Pending>);

/// A blast waiting for the end of chemistry's run.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Pending {
    def: Blast,
    centre: Vec2,
    /// Its receiver, which it neither hurts, pushes nor sets off, unless it
    /// hurts its receiver.
    from: Receiver,
    credit: Option<Entity>,
    depth: u8,
}

/// Whether a hit may be credited to `entity`: it lives and, holding
/// [`Health`], is not dead. A blast's and an impact's credit are checked as
/// each hit is built, so a credit an earlier hit killed credits nobody.
pub(crate) fn credits(world: &World, entity: Entity) -> bool {
    world.is_alive(entity)
        && world
            .get::<Health>(entity)
            .is_none_or(|health| !health.is_dead())
}

/// Whether a blast hurts `target`: it holds [`Health`], alive, and is not
/// being despawned.
fn lives(world: &World, target: Entity) -> bool {
    world
        .get::<Health>(target)
        .is_some_and(|health| !health.is_dead())
        && !world.has::<Despawning>(target)
}

/// `1 − d / r`, in `[0, 1]`.
fn falloff(distance: f32, radius: f32) -> f32 {
    (1.0 - distance / radius).clamp(0.0, 1.0)
}

/// The damage request of `def`'s damage at `share` on `target` at `at`,
/// from `centre`: its parts scaled, in its context, pushing `push × push
/// share` away from the centre (straight up where they coincide); `None`
/// when it deals nothing.
fn hurt(
    def: &Blast,
    target: Entity,
    at: Vec2,
    centre: Vec2,
    share: f32,
    push: f32,
) -> Option<DamageRequest> {
    let damage = def.damage.as_ref()?;
    let request = if let Some(parts) = damage.parts() {
        let parts: Vec<DamagePart> = parts
            .iter()
            .map(|part| DamagePart::new(part.amount() * share).with_tags(part.tags().clone()))
            .collect();
        if parts.iter().all(|part| part.amount() <= 0.0) {
            return None;
        }
        DamageRequest::from_parts(target, parts)
    } else {
        let amount = damage.total() * share;
        if amount <= 0.0 {
            return None;
        }
        DamageRequest::new(target, amount)
    };
    let mut request = request.with_context(Tags::new(def.context.iter().cloned()));
    if def.push > 0.0 && push > 0.0 {
        let away = (at - centre).normalize_or(Vec2::NEG_Y);
        request = request.with_knockback(away * def.push * push);
    }
    Some(request)
}

/// Sets off `blasts`, in order, each as [`Blast`] says, its stimuli for
/// chemistry's next run.
pub(crate) fn detonate(
    world: &mut World,
    settings: &Settings,
    blasts: &mut Vec<Pending>,
    reach: &mut Reach,
) {
    for pending in blasts.drain(..) {
        let Pending {
            def,
            centre,
            from,
            credit,
            depth,
        } = pending;
        go_off(
            world,
            settings,
            &def,
            centre,
            Some(&from),
            credit,
            depth,
            reach,
        );
    }
}

/// One target of a blast: its entity, its point and its distance.
type Target = (Entity, Vec2, f32);

/// Sets `def` off at `centre`, from `from`, credited to `credit` while it
/// lives, at `depth`: its hits, its impulse, then its stimuli.
#[allow(clippy::too_many_arguments)]
fn go_off(
    world: &mut World,
    settings: &Settings,
    def: &Blast,
    centre: Vec2,
    from: Option<&Receiver>,
    credit: Option<Entity>,
    depth: u8,
    reach: &mut Reach,
) {
    let prep: Option<HitPrep> = match def.preparation() {
        None => None,
        Some(name) => {
            let Some(prep) = hit_prep(world, name) else {
                refuse_hit_prep(world, "a blast", name);
                return;
            };
            Some(prep)
        }
    };
    let credit = credit.filter(|&credit| world.is_alive(credit));
    let spared = credit.filter(|_| def.spares_source());
    let radius = def.radius;
    reach.gather(world, centre, radius, from);
    let receiver = match from {
        Some(&Receiver::Entity(receiver)) => Some(receiver),
        _ => None,
    };
    // The targets, in save order, the receiver among them at distance 0
    // when it is one.
    let mut targets: Vec<Target> = reach
        .entities()
        .map(|candidate| {
            (
                candidate.entity,
                candidate.centre,
                candidate.distance(centre),
            )
        })
        .collect();
    if def.hurts_receiver()
        && let Some(receiver) = receiver
        && let Some((at, _)) = locate(world, receiver)
    {
        targets.push((receiver, at, 0.0));
        let order = order_of(world);
        targets.sort_by_key(|&(entity, ..)| order.key(entity));
    }
    // The bodies it hurt, then the dead and despawning it passed: its
    // impulse leaves both.
    let mut hurt_bodies: Vec<Entity> = Vec::new();
    if def.damage.is_some() {
        for &(target, at, distance) in &targets {
            if Some(target) == spared
                || !lives(world, target)
                || !def.passes(world.get::<Tags>(target))
            {
                continue;
            }
            let share = def.share(distance);
            if share <= 0.0 {
                continue;
            }
            let Some(mut request) = hurt(def, target, at, centre, share, falloff(distance, radius))
            else {
                continue;
            };
            if let Some(credit) = credit {
                request = request.with_source(credit);
            }
            request = request.with_depth(depth);
            if def.lands_at_centre() {
                request = request.with_position(centre);
            }
            if let Some(prep) = prep {
                prep(world, &mut request);
            }
            let request = request.roll_crit(world);
            hurt_bodies.push(target);
            if def.lands_now() {
                request.apply(world);
            } else {
                request.queue(world);
            }
        }
    }
    if def.impulse > 0.0 {
        hurt_bodies.extend(
            targets
                .iter()
                .map(|&(target, ..)| target)
                .filter(|&target| world.has::<Health>(target) && !lives(world, target)),
        );
        radial_impulse(world, centre, radius, def.impulse, |entity| {
            Some(entity) == receiver || Some(entity) == spared || hurt_bodies.contains(&entity)
        });
    }
    if !def.sends_stimulus() || !world.has_resource::<Chemistry>() {
        return;
    }
    let mut stimuli: Vec<(Receiver, f32)> = Vec::new();
    for candidate in reach.entities() {
        if candidate.tagged {
            let scale = falloff(candidate.distance(centre), radius);
            stimuli.push((Receiver::Entity(candidate.entity), scale));
        }
    }
    for cell in reach.cells() {
        let scale = cell
            .centre_in(world)
            .map_or(0.0, |at| falloff((at - centre).length(), radius));
        stimuli.push((Receiver::Cell(cell.clone()), scale));
    }
    let chemistry = world.resource_mut::<Chemistry>();
    let run = chemistry.run;
    let id = chemistry.intern(STIMULUS_BLAST);
    for (receiver, scale) in stimuli {
        let stimulus = Stimulus {
            amount: scale,
            status_effect: 1.0,
            dot_base: scale,
            depth,
            credit,
        };
        chemistry.stimulate_id(settings, receiver, id, stimulus, run);
    }
}

/// [`apply_radial_impulse`](tungsten_core::physics::apply_radial_impulse)
/// but for the bodies `skip` names: each dynamic body, and kinematic one
/// with a mass, whose surface lies within `radius` of `centre` takes
/// `impulse × (1 − d / r)` away from it (straight up from it).
fn radial_impulse(
    world: &mut World,
    centre: Vec2,
    radius: f32,
    impulse: f32,
    skip: impl Fn(Entity) -> bool,
) {
    let mut found = Vec::new();
    overlap(
        world,
        Shape::Circle { radius },
        centre,
        &SpatialFilter::new(),
        &mut found,
    );
    for entity in found.into_iter().flatten() {
        if skip(entity) {
            continue;
        }
        let (Some(body), Some(&collider), Some(position)) = (
            world.get::<RigidBody>(entity),
            world.get::<Collider>(entity),
            world.get::<Position>(entity),
        ) else {
            continue;
        };
        if !matches!(body.kind, BodyKind::Dynamic | BodyKind::Kinematic) {
            continue;
        }
        let at = collider.center(position.0);
        let distance = collider.shape.distance(centre - at);
        if distance >= radius {
            continue;
        }
        let direction = (at - centre).normalize_or(Vec2::NEG_Y);
        let share = direction * (impulse * (1.0 - distance / radius));
        apply_impulse(world, entity, share);
    }
}

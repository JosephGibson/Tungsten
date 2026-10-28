//! Combat's systems: the states the step's resolve begins with, the
//! reactions to the parts that landed and to `shattered`, `frozen`,
//! `shocked`, `bleeding` and `poisoned`, the blood a blow spills, the
//! freezes and the thaws heard, Corrosive, the chill's grade, the
//! statuses' tints on their targets' sprites and their particles.

use glam::Vec2;
use tungsten::SceneEntity;
use tungsten::core::{
    AnimationState, AssetRegistry, Collider, DataRef, DataRegistry, Entity, EventCursor,
    EventQueue, ParticleConfigRegistry, ParticleEmitter, ParticleEmitterState, Position,
    PrevPosition, Sprite, Transform, Visibility, World,
};
use tungsten::kit::{
    AilmentBuckets, AilmentThreshold, DamagedPart, EffectApplied, EffectExpired, EffectRequest,
    Effects, Follow, Health, ImmunePart, Lifetime, Modifier, Source, Stats, Tags, Target,
    end_effect,
};

use super::components::{StatusEmitter, StatusLook};
use super::{
    ARMOURED, BURNING, CHILL_NEEDS, CHILLED, CORRODED, DOUSED, FLESH, FREEZABLE, FREEZE_NEEDS,
    FROZEN, Kind, POISONED, SHOCKED, has_status,
};
use crate::features::audio::{self, components::Sound};
use crate::features::elements::cascade::{self, CascadeKind};
use crate::features::elements::{STEAM_REACH, TARGET_BLAST};
use crate::features::enemies::components::{Enemy, EnemyKind};
use crate::features::level::geyser::Geyser;
use crate::features::level::systems::drawn_time;
use crate::features::player::components::Moves;
use crate::features::player::systems::HURT_SECS;
use crate::features::run::Run;
use crate::game::GAMEPLAY;
use crate::shared::{SaveId, body_centre, spawn_burst};

/// The chill's second slow at the freeze's edge, by its grade on the climb
/// from the chill to the freeze, under the troll's old two-stage source.
pub const GRADE_SLOW: f32 = 0.35;
pub const CHILL_SOURCE: &str = "deep_chill";
/// A freeze's last seconds, over which its cube flickers to its cracked
/// twin this many times a second.
pub const THAW_WARN: f32 = 0.75;
pub const THAW_FLICKER_HZ: f32 = 8.0;

/// Which targets were frozen and which burning when the step's resolve
/// began: a World resource `combat_states` writes last before it, which
/// the step's reactions read, so an effect the step's own parts apply
/// never reacts to an earlier part (Q24).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolveStates {
    pub frozen: Vec<Entity>,
    pub burning: Vec<Entity>,
}

/// `combat_states`, in the resolve phase before the kit's damage resolve:
/// records the targets carrying frozen and burning.
pub fn states(world: &mut World) {
    let mut states = ResolveStates::default();
    for (entity, _, tags) in world.query::<(Entity, &Health, &Tags)>() {
        if tags.has(FROZEN) {
            states.frozen.push(entity);
        }
        if tags.has(BURNING) {
            states.burning.push(entity);
        }
    }
    world.insert_resource(states);
}

/// `combat_reactions`, in the react phase: a fire part on a target that
/// was frozen queues thermal shock's explosion at it, and a cold part on
/// one that was burning a steam burst, dead or alive, each landed or
/// dropped by the target's immunity (the kit's `ImmunePart`: a frozen wisp,
/// which takes no fire, or a burning frost crawler, which takes no cold);
/// `shattered` applied queues its shatter. Each queued entry runs from
/// the next step, an explosion or a steam burst crediting its part's
/// source's save ID: none when the source has gone, the player's when the
/// part named none. A blow, a physical part that is not a tick, on a `flesh`
/// target or the courier spurts blood where it landed, once a target a
/// step, and `bleeding` applied spurts again; `poisoned` applied gurgles at
/// its target (e01flask); a target frozen this step
/// (frozen now, not as the resolve began), in place too, sounds its freeze
/// with its frost burst. A geyser's fire part on a target neither burning,
/// frozen nor doused sets it burning from the geyser, once a target a step,
/// since its 12 fire alone falls short of the player's ladder (e01hollow).
/// Last, Corrosive ([`corrode`]).
pub fn reactions(
    world: &mut World,
    parts: &mut EventCursor<DamagedPart>,
    immune: &mut EventCursor<ImmunePart>,
    applied: &mut EventCursor<EffectApplied>,
) {
    let states = world
        .get_resource::<ResolveStates>()
        .cloned()
        .unwrap_or_default();
    let player = world.get_resource::<Run>().map(|run| run.player);
    let mine = cascade::player_credit(world);
    let credit = |world: &World, source: Option<Entity>| match source {
        Some(source) if world.is_alive(source) => world.get::<SaveId>(source).map(|id| id.0),
        Some(_) => None,
        None => mine,
    };
    let mut queued: Vec<(CascadeKind, Entity, f32, Option<u64>)> = Vec::new();
    let mut blows: Vec<(Entity, Option<Vec2>)> = Vec::new();
    let mut burns: Vec<(Entity, Entity)> = Vec::new();
    if let Some(events) = world.get_resource::<EventQueue<DamagedPart>>() {
        for part in parts.read(events) {
            if part.periodic {
                continue;
            }
            let fire = part.kinds.has(world, Kind::Fire.tag());
            let cold = part.kinds.has(world, Kind::Cold.tag());
            let source = credit(world, part.source);
            if fire && states.frozen.contains(&part.entity) {
                queued.push((CascadeKind::Explosion, part.entity, TARGET_BLAST, source));
            }
            if cold && states.burning.contains(&part.entity) {
                queued.push((CascadeKind::Steam, part.entity, STEAM_REACH, source));
            }
            if let Some(geyser) = part.source.filter(|&s| world.has::<Geyser>(s))
                && fire
                && part.amount > 0.0
                && !burns.iter().any(|(target, _)| *target == part.entity)
            {
                burns.push((part.entity, geyser));
            }
            let bleeds = Some(part.entity) == player || has_status(world, part.entity, FLESH);
            if bleeds
                && part.kinds.has(world, Kind::Physical.tag())
                && !blows.iter().any(|(target, _)| *target == part.entity)
            {
                blows.push((part.entity, part.position));
            }
        }
    }
    if let Some(events) = world.get_resource::<EventQueue<ImmunePart>>() {
        for part in immune.read(events) {
            if part.periodic {
                continue;
            }
            let source = credit(world, part.source);
            if part.kinds.has(world, Kind::Fire.tag()) && states.frozen.contains(&part.entity) {
                queued.push((CascadeKind::Explosion, part.entity, TARGET_BLAST, source));
            }
            if part.kinds.has(world, Kind::Cold.tag()) && states.burning.contains(&part.entity) {
                queued.push((CascadeKind::Steam, part.entity, STEAM_REACH, source));
            }
        }
    }
    let (mut spurts, mut poisoned) = (Vec::new(), Vec::new());
    if let Some(events) = world.get_resource::<EventQueue<EffectApplied>>() {
        for effect in applied.read(events) {
            match effect.id.as_deref() {
                Some("shattered") => queued.push((CascadeKind::Shatter, effect.entity, 0.0, mine)),
                Some("bleeding") => spurts.push(effect.entity),
                Some("poisoned") => poisoned.push(effect.entity),
                _ => {}
            }
        }
    }
    for target in poisoned {
        if let Some(at) = body_centre(world, target) {
            audio::play(world, Sound::Poisoned, at);
        }
    }
    for (target, geyser) in burns {
        let lit = [BURNING, FROZEN, DOUSED]
            .into_iter()
            .any(|status| has_status(world, target, status));
        if !lit && world.is_alive(target) {
            EffectRequest::new(target, DataRef::by_id("burning"))
                .with_source(geyser)
                .apply(world);
        }
    }
    for (target, at) in blows {
        if let Some(at) = at.or_else(|| body_centre(world, target)) {
            spawn_burst(world, "ex10_blood_burst", at);
        }
    }
    for target in spurts {
        if let Some(at) = body_centre(world, target) {
            spawn_burst(world, "ex10_blood_spurt", at);
        }
    }
    let mut froze: Vec<(Option<SaveId>, Entity)> = world
        .query::<(Entity, &Health, &Tags)>()
        .filter(|(entity, health, tags)| {
            tags.has(FROZEN) && !health.is_dead() && !states.frozen.contains(entity)
        })
        .map(|(entity, ..)| (world.get::<SaveId>(entity).copied(), entity))
        .collect();
    froze.sort_by_key(|(id, _)| *id);
    for (_, target) in froze {
        if let Some(at) = body_centre(world, target) {
            spawn_burst(world, "ex10_ice_frost", at);
            audio::play(world, Sound::Froze, at);
        }
    }
    for (kind, target, strength, source) in queued {
        let Some(at) = body_centre(world, target) else {
            continue;
        };
        cascade::queue_from(world, kind, Some(target), at, at, strength, source);
    }
    corrode(world);
}

/// Corrosive (e01flask, Q4, Q5, Q69): each live `armoured` target that
/// carries poisoned and shocked at once and is not corroded, in save-ID
/// order, takes `corroded` at once, from nobody, and the proc consumes
/// both (the owner's round-2 note): its poison and its shock end there.
/// Its reaction plays over its body ([`reaction`]), its drops and its sound
/// at its centre; its armour's resistances are overridden to 1 on every
/// type for good, never on `damage`, so a later shock's half again holds,
/// and bullets no longer turn off its front. It reads the step's last tags,
/// so either status landing second corrodes.
pub fn corrode(world: &mut World) {
    let mut corroding: Vec<(Option<SaveId>, Entity)> = world
        .query::<(Entity, &Health, &Tags)>()
        .filter(|(_, health, tags)| {
            !health.is_dead()
                && tags.has(ARMOURED)
                && tags.has(POISONED)
                && tags.has(SHOCKED)
                && !tags.has(CORRODED)
        })
        .map(|(entity, ..)| (world.get::<SaveId>(entity).copied(), entity))
        .collect();
    corroding.sort_by_key(|(id, _)| *id);
    for (_, target) in corroding {
        EffectRequest::new(target, DataRef::by_id("corroded")).apply(world);
        for spent in ["poisoned", "shocked"] {
            end_effect(world, target, &DataRef::by_id(spent));
        }
        reaction(world, target);
        if let Some(at) = body_centre(world, target) {
            spawn_burst(world, "ex10_corrode_burst", at);
            audio::play(world, Sound::Corroded, at);
        }
    }
}

/// Corrosive's reaction clips by the body they cover, each with its
/// canvas's side: the medium's over a beetle's shell, the large's over an
/// armoured troll's plates.
pub const REACTION_CLIPS: [(&str, f32); 2] = [
    ("ex10_corrode_reaction_medium", 96.0),
    ("ex10_corrode_reaction_large", 160.0),
];
/// How long Corrosive's reaction plays: its ten frames of 70 ms.
pub const REACTION_SECS: f32 = 0.7;
/// Its draw order, over the enemies' 8.
const REACTION_Z: i32 = 14;

/// Corrosive's reaction over `target`, once (e01flask's round-2 note):
/// arcs leaping through the poison boiling up off its armour, the metal
/// running off, fumes rising; its clip's middle on the body's, following
/// the body by its `Position`, so a troll's turned frames move it nowhere.
/// The stripped clips take the armour's place under it at once.
pub fn reaction(world: &mut World, target: Entity) {
    let Some(corner) = world.get::<Position>(target).map(|p| p.0) else {
        return;
    };
    let middle = world
        .get::<Collider>(target)
        .map_or(Vec2::ZERO, |collider| collider.offset);
    let large = world
        .get::<Enemy>(target)
        .is_some_and(|enemy| enemy.kind == EnemyKind::Troll);
    let (clip, side) = REACTION_CLIPS[usize::from(large)];
    let offset = middle - Vec2::splat(side * 0.5);
    let Some(assets) = world.get_resource_mut::<AssetRegistry>() else {
        return;
    };
    let mut sprite = Sprite::new(assets.intern_sprite(&format!("{clip}_0")));
    sprite.z_order = REACTION_Z;
    let at = corner + offset;
    world.spawn_with((
        Position(at),
        PrevPosition(at),
        Transform::from_position(at),
        sprite,
        Visibility { visible: true },
        AnimationState::new(clip),
        Lifetime::new(REACTION_SECS),
        Source::new(target),
        Follow::new(Target::source()).with_offset(offset),
        SceneEntity { state_id: GAMEPLAY },
    ));
}

/// The tints a status gives its target's sprite, which multiply its
/// colours: frozen's cold; chilled's frost, a freezable creature's turning
/// toward it from the first point of cold and, chilled, deeper toward
/// [`CHILL_DEEP`] by the chill's grade, white and gentle (01 (j)); shocked's
/// violet on its crackle's even frames, burning's two embers, which
/// flicker, poisoned's green and, under every other, corroded's dull
/// verdigris (e01flask); and the courier's red for a moment after a blow.
pub const COLD_TINT: [u8; 4] = [158, 219, 255, 255];
pub const FROST_TINT: [u8; 4] = [222, 241, 255, 255];
pub const CHILL_DEEP: [u8; 4] = [194, 229, 255, 255];
pub const SHOCK_TINT: [u8; 4] = [196, 168, 255, 255];
pub const EMBER_TINTS: [[u8; 4]; 2] = [[255, 178, 128, 255], [255, 140, 92, 255]];
pub const POISON_TINT: [u8; 4] = [176, 255, 140, 255];
pub const CORRODE_TINT: [u8; 4] = [170, 214, 190, 255];
pub const HURT_TINT: [u8; 4] = [255, 72, 72, 255];
/// How fast burning's tint flickers, and shocked's crackle and poisoned's
/// bubbles step through their four frames, frames a second.
pub const FLICKER_HZ: f32 = 12.0;
pub const SHOCK_HZ: f32 = 16.0;
pub const POISON_HZ: f32 = 8.0;

/// Burning's ember at level time `time`, 0 or 1.
#[must_use]
pub fn flicker(time: f32) -> usize {
    (time * FLICKER_HZ) as usize % 2
}

/// Shocked's crackle frame at level time `time`, 0 to 3; its tint shows on
/// the even ones.
#[must_use]
pub fn shock_frame(time: f32) -> usize {
    (time * SHOCK_HZ) as usize % 4
}

/// Poisoned's overlay frame at level time `time`, 0 to 3.
#[must_use]
pub fn poison_frame(time: f32) -> usize {
    (time * POISON_HZ) as usize % 4
}

/// `a` toward `b` by `share`, 0 to 1.
fn mix(a: [u8; 4], b: [u8; 4], share: f32) -> [u8; 4] {
    let share = share.clamp(0.0, 1.0);
    std::array::from_fn(|i| {
        (f32::from(a[i]) + (f32::from(b[i]) - f32::from(a[i])) * share).round() as u8
    })
}

/// The tint `tags` give a target at level time `time`, chilled to `grade`
/// with its frost at `level` ([`frost_level`]): frozen over shocked over
/// burning over poisoned over chilled over the frost before the chill over
/// corroded, else none.
#[must_use]
pub fn tint(tags: &Tags, time: f32, grade: f32, level: f32) -> [u8; 4] {
    if tags.has(FROZEN) {
        COLD_TINT
    } else if tags.has(SHOCKED) && shock_frame(time).is_multiple_of(2) {
        SHOCK_TINT
    } else if tags.has(BURNING) {
        EMBER_TINTS[flicker(time)]
    } else if tags.has(POISONED) {
        POISON_TINT
    } else if tags.has(CHILLED) {
        mix(FROST_TINT, CHILL_DEEP, grade)
    } else if level > 0.0 {
        mix([255; 4], FROST_TINT, level / CHILL_SHARE)
    } else if tags.has(CORRODED) {
        CORRODE_TINT
    } else {
        [255; 4]
    }
}

/// `status_tints`, in the present phase: each target with health takes its
/// statuses' tint, the courier's mixed toward red while a blow's
/// `Moves::hurt` runs.
pub fn tints(world: &mut World) {
    let time = drawn_time(world);
    let looks: Vec<(Entity, [u8; 4])> = world
        .query::<(Entity, &Health, &Tags)>()
        .map(|(entity, _, tags)| {
            let level = frost_level(world, entity, tags);
            let mut color = tint(tags, time, grade(world, entity, tags), level);
            if let Some(moves) = world.get::<Moves>(entity) {
                color = mix(color, HURT_TINT, moves.hurt / HURT_SECS);
            }
            (entity, color)
        })
        .collect();
    for (entity, color) in looks {
        if let Some(sprite) = world.get_mut::<Sprite>(entity) {
            sprite.color = color;
        }
    }
}

/// The chill's grade on `entity`, whose `tags` these are: a chilled
/// `freezable` target's [`chill_grade`], else 0.
fn grade(world: &World, entity: Entity, tags: &Tags) -> f32 {
    if tags.has(CHILLED) && tags.has(FREEZABLE) {
        chill_grade(world, entity).unwrap_or(0.0)
    } else {
        0.0
    }
}

/// `entity`'s ailment threshold as the kit's ladder reads it: `(flat +
/// of_max_health × Health::max) × ailment_threshold_mult`. None without one,
/// or at 0.
fn threshold_at(world: &World, entity: Entity) -> Option<f32> {
    let threshold = world.get::<AilmentThreshold>(entity)?;
    let max = world.get::<Health>(entity).map_or(0.0, Health::max);
    let mult = world
        .get::<Stats>(entity)
        .and_then(|stats| stats.get("ailment_threshold_mult"))
        .map_or(1.0, |mult| mult.max(0.0));
    let at = (threshold.flat() + threshold.of_max_health() * max) * mult;
    (at > 0.0).then_some(at)
}

/// Where `entity`'s cold bucket stands on its climb from the chill's rung
/// to the freeze's, 0 to 1. The chill empties the bucket as it applies and
/// the freeze needs [`FREEZE_NEEDS`] of the threshold, so the grade is its
/// fill over that share of its threshold ([`threshold_at`]). None without
/// buckets or a threshold.
#[must_use]
pub fn chill_grade(world: &World, entity: Entity) -> Option<f32> {
    let at = threshold_at(world, entity)?;
    let buckets = world.get::<AilmentBuckets>(entity)?;
    Some((buckets.fill(Kind::Cold.tag()) / (FREEZE_NEEDS * at)).clamp(0.0, 1.0))
}

/// The share of the frost's climb a creature has at the chill's rung.
pub const CHILL_SHARE: f32 = 0.25;

/// The frost a creature that freezes shows, `tags` its own, 0 to 1, from the
/// first point of cold (01 (j)): its cold bucket's climb to the chill's
/// rung ([`CHILL_NEEDS`]) over the first [`CHILL_SHARE`], then the chill's
/// grade to the freeze's over the rest, the chill having emptied the
/// bucket. 0 for what does not freeze, and once frozen, where the cube
/// takes over.
#[must_use]
pub fn frost_level(world: &World, entity: Entity, tags: &Tags) -> f32 {
    if !tags.has(FREEZABLE) || tags.has(FROZEN) {
        return 0.0;
    }
    let (Some(at), Some(buckets)) = (
        threshold_at(world, entity),
        world.get::<AilmentBuckets>(entity),
    ) else {
        return 0.0;
    };
    let fill = buckets.fill(Kind::Cold.tag());
    if tags.has(CHILLED) {
        CHILL_SHARE + (1.0 - CHILL_SHARE) * (fill / (FREEZE_NEEDS * at)).clamp(0.0, 1.0)
    } else {
        CHILL_SHARE * (fill / (CHILL_NEEDS * at)).clamp(0.0, 1.0)
    }
}

/// The frost's stage at `level`: 1 rime at the feet, before the chill; 2 the
/// legs, from the chill; 3 the body, from 0.5; 4 a crust, from 0.75; none
/// without frost.
#[must_use]
pub fn frost_stage(level: f32) -> Option<u8> {
    (level > 0.0).then(|| 1 + ((level * 4.0) as u8).min(3))
}

/// `chill_grades`, in the intent phase before the enemies' states: every
/// creature with a threshold sheds its [`CHILL_SOURCE`] slow, then a chilled
/// `freezable` one takes it again at [`GRADE_SLOW`] × its grade, between
/// the chill's rung and the freeze's. The courier, which takes no freeze,
/// keeps the chill's flat slow; a schema 10 troll saved past its second
/// stage sheds that slow at its first step; a diving bat's set speed
/// outranks the grade, as it outranks the chill's own slow.
pub fn chill_grades(world: &mut World) {
    let graded: Vec<(Entity, f32)> = world
        .query::<(Entity, &AilmentThreshold, &Tags)>()
        .map(|(entity, _, tags)| (entity, grade(world, entity, tags)))
        .collect();
    for (entity, grade) in graded {
        if let Some(stats) = world.get_mut::<Stats>(entity) {
            stats.remove_modifiers(CHILL_SOURCE);
            if grade > 0.0 {
                let slow = Modifier::Pct(-GRADE_SLOW * grade);
                stats.add_modifier(Stats::MOVE_SPEED, CHILL_SOURCE, slow);
            }
        }
    }
}

/// The game seconds `entity`'s freeze has left: the longest of its effect
/// instances granting frozen. None while none does, or one never ends.
#[must_use]
pub fn frozen_left(world: &World, entity: Entity) -> Option<f32> {
    let effects = world.get::<Effects>(entity)?;
    let data = world.get_resource::<DataRegistry>()?;
    let mut most: Option<f32> = None;
    for instance in effects.instances() {
        let grants = instance
            .effect()
            .resolve(data)
            .is_some_and(|effect| effect.grant().iter().any(|tag| tag == FROZEN));
        if grants {
            let left = instance.left()?;
            most = Some(most.map_or(left, |most: f32| most.max(left)));
        }
    }
    most
}

/// The targets frozen as the last step ended, which the thaws read:
/// presentation, never saved.
#[derive(Debug, Clone, Default)]
pub struct WasFrozen(pub Vec<Entity>);

/// `combat_thaws`, in the react phase: a live target frozen as the last
/// step ended whose freeze ran out on time this step thaws in glints with
/// an ice crack; then the frozen are noted for the next step.
pub fn thaws(world: &mut World, expired: &mut EventCursor<EffectExpired>) {
    let ran_out: Vec<Entity> = world
        .get_resource::<EventQueue<EffectExpired>>()
        .map(|events| {
            expired
                .read(events)
                .filter(|effect| !effect.ended)
                .map(|effect| effect.entity)
                .collect()
        })
        .unwrap_or_default();
    let was = world
        .get_resource::<WasFrozen>()
        .map(|was| was.0.clone())
        .unwrap_or_default();
    for target in was {
        let thawed = ran_out.contains(&target)
            && world.is_alive(target)
            && world.get::<Health>(target).is_some_and(|h| !h.is_dead())
            && !has_status(world, target, FROZEN);
        if thawed && let Some(at) = body_centre(world, target) {
            spawn_burst(world, "ex10_thaw_glints", at);
            audio::play(world, Sound::Thawed, at);
        }
    }
    let now: Vec<Entity> = world
        .query::<(Entity, &Health, &Tags)>()
        .filter(|(_, health, tags)| !health.is_dead() && tags.has(FROZEN))
        .map(|(entity, ..)| entity)
        .collect();
    world.insert_resource(WasFrozen(now));
}

/// `status_particles`, before the frame's particles: a live target that
/// carries a status has that status's emitter at its body's centre, and a
/// freezable creature its frost's flakes by stage from the first point of
/// cold (`ex10_chill_flakes_<stage>`), a stage's change spawning its own;
/// one that lost it, died or left loses the emitter.
pub fn particles(world: &mut World) {
    let mut wanted: Vec<(Entity, StatusLook, Vec2, u8)> = Vec::new();
    for (entity, health, tags, at) in world.query::<(Entity, &Health, &Tags, &Transform)>() {
        if health.is_dead() {
            continue;
        }
        let offset = world
            .get::<Collider>(entity)
            .map_or(Vec2::splat(32.0), |collider| collider.offset);
        let centre = at.position + offset;
        for look in StatusLook::ALL {
            if look == StatusLook::Chilled && tags.has(FREEZABLE) {
                if let Some(stage) = frost_stage(frost_level(world, entity, tags)) {
                    wanted.push((entity, look, centre, stage));
                }
            } else if tags.has(look.tag()) {
                wanted.push((entity, look, centre, 0));
            }
        }
    }
    let live: Vec<(Entity, StatusEmitter)> = world
        .query::<(Entity, &StatusEmitter)>()
        .map(|(entity, emitter)| (entity, *emitter))
        .collect();
    for &(entity, emitter) in &live {
        let kept = wanted.iter().any(|&(owner, look, _, stage)| {
            owner == emitter.owner && look == emitter.look && stage == emitter.stage
        });
        if !kept {
            world.despawn(entity);
        }
    }
    for (owner, look, centre, stage) in wanted {
        let existing = live
            .iter()
            .find(|(_, emitter)| {
                emitter.owner == owner && emitter.look == look && emitter.stage == stage
            })
            .map(|&(entity, _)| entity);
        let emitter = match existing {
            Some(entity) => entity,
            None => {
                let name = if stage > 0 {
                    format!("{}_{stage}", look.config())
                } else {
                    look.config().to_string()
                };
                let Some(config) = world
                    .get_resource::<ParticleConfigRegistry>()
                    .and_then(|registry| registry.id_for_name(&name))
                else {
                    continue;
                };
                let seed = 0x5a_0000 + u64::from(owner.id()) * 8 + look as u64;
                world.spawn_with((
                    StatusEmitter { owner, look, stage },
                    ParticleEmitter::with_seed(config, seed),
                    ParticleEmitterState::default(),
                    Transform::from_position(centre),
                    SceneEntity { state_id: GAMEPLAY },
                ))
            }
        };
        if let Some(transform) = world.get_mut::<Transform>(emitter) {
            transform.position = centre;
        }
    }
}

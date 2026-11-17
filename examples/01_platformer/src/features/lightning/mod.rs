//! Lightning, slot 4: a bolt from the sky onto the aim point, or onto the
//! nearest conductive thing within reach of it, the rod; it stops at the
//! first solid on its way down, so under a ceiling it strikes the ceiling.
//! Its shock hurts the targets where it lands, the caster spared, and it
//! lights nothing. Then conduction, one chain a strike, on the kit's
//! conduction (`kit::conduct_from`, its chains saved by the kit): from the
//! rod, or from the struck conductive thing nearest the strike, one jump a
//! step to the nearest conductive thing not yet in the chain, each jump
//! weaker, at most six, and none past a frozen receiver or one that takes
//! no shock, which take their jump; past a receiver gone after its strike,
//! a keg set off, it jumps on from where that receiver was. Conductive
//! things are iron, its scraps and a keg's hoops, tagged `conductive` by
//! their prefabs, every live enemy and the live targets tagged
//! `conductive`; a strike or a jump that reaches a keg sets it off. A
//! strike says what it met: a body snaps, while the ground and a deck
//! spark and crackle low, and the ground keeps a scorch that fades.

pub mod components;
pub mod draw;
pub mod systems;

use glam::{Vec2, Vec3};
use tungsten::core::{
    AssetRegistry, BodyKind, Collider, Entity, EventCursor, Light, Position, RigidBody, Schedule,
    Shape, Sprite, Stage, Transform, Visibility, World, system,
};
use tungsten::kit::{
    ABILITY_TICK, Blast, Conduct, Health, Phase, Resist, TagFilter, Tags, WorldCue, blast,
    conduct_from, ensure_phases,
};
use tungsten::physics::{SpatialFilter, cast_ray};
use tungsten::{Extracts, Plugin, SceneEntity};

use crate::features::combat::{FROZEN, Kind, SPELL};
use crate::features::elements::cascade::Cascades;
use crate::features::enemies::{self, armour, components::Enemy};
use crate::features::level::{WORLD_MAX, WORLD_MIN};
use crate::features::props::{self, components::Keg};
use crate::game::GAMEPLAY;
use crate::shared::{SaveId, body_centre, surface_distance};
use components::{Bolt, Scorch, StrikeLight};

/// A strike's shock and its reach from where it lands.
pub const STRIKE_SHOCK: f32 = 40.0;
pub const STRIKE_REACH: f32 = 40.0;
/// A jump's reach, the share of its shock each jump keeps, and the most
/// jumps a strike makes.
pub const JUMP_REACH: f32 = 256.0;
pub const JUMP_KEEP: f32 = 0.7;
pub const MAX_JUMPS: u32 = 6;
/// The tags a conductive thing carries, any one of them: metal's and the
/// conducting targets' `conductive`, and every enemy's team.
pub const RECEIVERS: [&str; 2] = ["conductive", "team.enemy"];
/// The sideways step each bolt segment leaves at its bottom, as the art
/// draws them.
pub const EXITS: [f32; 8] = [-8.0, -4.0, 0.0, 4.0, 8.0, -6.0, 6.0, 2.0];
/// How long a bolt shows, the strike's light, its reach, its colour and
/// the seconds it fades over.
pub const BOLT_SECS: f32 = 0.15;
pub const STRIKE_LIGHT: f32 = 3.0;
pub const LIGHT_REACH: f32 = 320.0;
pub const LIGHT_COLOR: Vec3 = Vec3::new(0.82, 0.72, 1.0);
pub const LIGHT_SECS: f32 = 0.2;
/// How long a jump's arc shows, fading over its last third, and the
/// seconds its zigzag holds before it takes a new path, so it crackles.
pub const ARC_SECS: f32 = 0.35;
pub const ARC_JITTER: f32 = 0.05;
/// The scorch a strike leaves on the ground: its sprite, the point of it
/// on the landing (its bottom row's middle), its z under the bodies, the
/// seconds it fades over and the most that stay, the oldest going.
pub const SCORCH: &str = "ex10_scorch";
pub const SCORCH_ANCHOR: Vec2 = Vec2::new(24.0, 15.0);
pub const SCORCH_Z: i32 = 1;
pub const SCORCH_SECS: f32 = 5.0;
pub const SCORCH_CAP: usize = 6;

/// Lightning as a plugin: its conduction, built once; each jump's arc,
/// crackle and keg in the intent phase after the kit's ability tick,
/// which jumps the chains; the presentation's ages in the present phase
/// and the bolts, arcs and veil drawn after the sprites.
pub struct LightningPlugin;

impl Plugin for LightningPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.resource_mut::<Extracts>().add_sprites(draw::bolts);
        world.insert_resource(Rule(conduction()));
        ensure_phases(schedule);
        let mut cursor = EventCursor::new();
        schedule.add(
            Stage::FixedUpdate,
            Phase::Intent.join(
                system("lightning_jumps", move |world: &mut World| {
                    systems::jumps(world, &mut cursor);
                })
                .after(ABILITY_TICK),
            ),
        );
        schedule.add(
            Stage::PostUpdate,
            Phase::Present.join(system("lightning_age", systems::age)),
        );
    }
}

/// How far `at` lies from `entity`'s body's surface: 0 inside it.
fn gap(world: &World, at: Vec2, entity: Entity) -> Option<f32> {
    let shape = world.get::<Collider>(entity)?.shape;
    Some(surface_distance(at, body_centre(world, entity)?, shape))
}

/// Whether `entity` conducts: its tags pass the conduction's receivers,
/// any of [`RECEIVERS`], iron, its scraps and a keg's hoops by their
/// prefabs' `conductive`, every enemy by its team, the dummy and the
/// beetle by theirs, and its health, if it has any, alive (Q48).
#[must_use]
pub fn conductive(world: &World, entity: Entity) -> bool {
    world
        .get::<Tags>(entity)
        .is_some_and(|tags| RECEIVERS.iter().any(|tag| tags.has(tag)))
        && world
            .get::<Health>(entity)
            .is_none_or(|health| !health.is_dead())
}

/// Whether `entity` takes no shock: its resistance to it 0, which ends a
/// chain where it reaches it, after its jump (Q48).
#[must_use]
pub fn shock_proof(world: &World, entity: Entity) -> bool {
    world
        .get::<Resist>(entity)
        .is_some_and(|resist| resist.factor(&Tags::new([Kind::Shock.tag()])) == 0.0)
}

/// The lightning's conduction on the kit: [`STRIKE_SHOCK`] of shock, its
/// jumps [`JUMP_REACH`] centre to centre, each keeping [`JUMP_KEEP`], at
/// most [`MAX_JUMPS`], to the things any of [`RECEIVERS`] tags; a frozen
/// receiver or one that takes no shock takes its jump and ends the chain;
/// each jump a spell's hit through the `armour` preparation, its crit the
/// caster's; and past a receiver gone after its strike it jumps on from
/// where that receiver was.
#[must_use]
pub fn conduction() -> Conduct {
    Conduct::new(STRIKE_SHOCK, JUMP_REACH, TagFilter::new().any(RECEIVERS))
        .with_tags([Kind::Shock.tag()])
        .with_keep(JUMP_KEEP)
        .with_jumps(MAX_JUMPS)
        .with_stop(TagFilter::new().any([FROZEN]))
        .with_context([SPELL])
        .with_prepare(armour::PREPARE)
        .with_stop_struck(true)
        .with_past_gone(true)
}

/// The lightning's [`conduction`], built once: a World resource the plugin
/// inserts, never saved.
struct Rule(Conduct);

/// What a bolt met where it landed (Q7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Met {
    /// A tile, a static collider's top or the level's foot.
    Ground,
    /// A body neither dynamic nor an enemy, which moves: a deck, the
    /// elevator.
    Deck(Entity),
    /// A live enemy within its strike width, or a dynamic body.
    Body(Entity),
}

/// Where a bolt lands, and what it met there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Landing {
    pub at: Vec2,
    pub met: Met,
}

/// What a bolt's ray met in `entity`: a tile or a static body is the
/// ground, a dynamic body or an enemy a body, any other body a deck.
fn met(world: &World, entity: Option<Entity>) -> Met {
    let Some(entity) = entity else {
        return Met::Ground;
    };
    match world.get::<RigidBody>(entity).map(|body| body.kind) {
        Some(BodyKind::Dynamic) => Met::Body(entity),
        Some(BodyKind::Kinematic) if world.has::<Enemy>(entity) => Met::Body(entity),
        Some(BodyKind::Kinematic) => Met::Deck(entity),
        _ => Met::Ground,
    }
}

/// Where a bolt falling from the top of the level at `point.x` stops, and
/// what it met: the first solid tile or body its line meets, `caster`'s
/// body aside, the top of a live enemy's body within its kind's strike
/// width of its centre
/// ([`KindInfo::strike`](crate::features::enemies::KindInfo::strike)),
/// its art being wider than its body, or the top of the first static
/// collider it crosses, else the level's foot. The aim point's height does
/// not stop it.
#[must_use]
pub fn bolt_end(world: &World, point: Vec2, caster: Option<Entity>) -> Landing {
    let top = Vec2::new(point.x, WORLD_MIN.y);
    let mut stop = WORLD_MAX.y - top.y;
    let mut what = Met::Ground;
    let filter = caster.map_or_else(SpatialFilter::new, |caster| {
        SpatialFilter::new().excluding(caster)
    });
    if let Some(hit) = cast_ray(world, top, Vec2::Y, stop, &filter)
        && hit.distance <= stop
    {
        stop = hit.distance;
        what = met(world, hit.entity);
    }
    for (at, collider, body) in world.query::<(&Position, &Collider, &RigidBody)>() {
        if body.kind != BodyKind::Static {
            continue;
        }
        let Shape::Aabb { half_extents } = collider.shape else {
            continue;
        };
        let centre = at.0 + collider.offset;
        let face = centre.y - half_extents.y - top.y;
        if (point.x - centre.x).abs() <= half_extents.x && (0.0..stop).contains(&face) {
            stop = face;
            what = Met::Ground;
        }
    }
    for (entity, enemy, health) in world.query::<(Entity, &Enemy, &Health)>() {
        if health.is_dead() || Some(entity) == caster {
            continue;
        }
        let Some(centre) = body_centre(world, entity) else {
            continue;
        };
        let strike = enemies::info(world, enemy.kind).strike;
        let face = centre.y - enemies::body(world, enemy.kind).half.y - top.y;
        if (point.x - centre.x).abs() <= strike && (0.0..stop).contains(&face) {
            stop = face;
            what = Met::Body(entity);
        }
    }
    Landing {
        at: top + Vec2::Y * stop,
        met: what,
    }
}

/// Strikes from `caster` at the aim point `aim`: the bolt falls at the
/// aim's x from the sky until it meets a tile or a body; its shock, the
/// kit's blast of [`STRIKE_SHOCK`] flat within [`STRIKE_REACH`] of where it
/// lands, a spell's hit queued for the step's resolve, hurts the targets
/// there, `caster` spared, and it sets off the kegs there, credited to
/// `caster`; and one chain of the kit's conduction starts at the struck
/// conductive thing nearest where it landed (ties to the lower save ID),
/// past every struck one, unless that one is frozen or takes no shock, its
/// first jump at the next step's ability tick. Where it landed.
pub fn strike(world: &mut World, caster: Entity, aim: Vec2) -> Vec2 {
    let Landing { at: landing, met } = bolt_end(world, aim, Some(caster));
    let mut struck: Vec<(Option<SaveId>, Entity)> = world
        .query::<(Entity, &Collider)>()
        .filter(|&(entity, _)| entity != caster)
        .filter(|&(entity, _)| {
            world.get::<Health>(entity).is_some_and(|h| !h.is_dead()) || conductive(world, entity)
        })
        .filter(|&(entity, _)| gap(world, landing, entity).is_some_and(|gap| gap <= STRIKE_REACH))
        .map(|(entity, _)| (crate::shared::save_id(world, entity), entity))
        .collect();
    struck.sort_by_key(|(id, _)| *id);
    let shock = Blast::new(STRIKE_REACH)
        .with_part(Kind::Shock.tag(), STRIKE_SHOCK)
        .with_far(1.0)
        .with_context([SPELL])
        .sparing_source()
        .at_centre()
        .with_stimulus(false)
        .with_prepare(armour::PREPARE);
    blast(world, landing, &shock, Some(caster), None);
    let credit = crate::shared::save_id(world, caster).map(|id| id.0);
    for &(_, target) in &struck {
        if world.has::<Keg>(target) {
            props::set_off(world, target, credit);
        }
    }
    let number = world.get_resource_mut::<Cascades>().map_or(0, |cascades| {
        cascades.strike += 1;
        cascades.strike
    });
    let conductors: Vec<(Entity, u64)> = struck
        .iter()
        .filter(|&&(_, entity)| conductive(world, entity))
        .filter_map(|&(id, entity)| Some((entity, id?.0)))
        .collect();
    // The chain starts at the struck conductor nearest where the bolt
    // landed, which the kit refuses when it is frozen or takes no shock.
    let start = conductors
        .iter()
        .filter_map(|&(entity, id)| Some((entity, body_centre(world, entity)?, id)))
        .min_by(|a, b| {
            a.1.distance(landing)
                .total_cmp(&b.1.distance(landing))
                .then(a.2.cmp(&b.2))
        })
        .map(|(entity, ..)| entity);
    if let Some(start) = start {
        let chained: Vec<Entity> = conductors.iter().map(|&(entity, _)| entity).collect();
        let rule = world
            .remove_resource::<Rule>()
            .unwrap_or_else(|| Rule(conduction()));
        conduct_from(world, caster, start, &rule.0, &chained);
        world.insert_resource(rule);
    }
    show(world, landing, met, number);
    landing
}

/// A strike's looks at a point, named world cues (`data/world_cues/`):
/// where it met a body, its sparks and its snap; where it met the ground
/// or a deck, the ground's sparks and their crackle; each with the thunder
/// and its shake.
pub const STRIKE_BODY: &str = "strike_body";
pub const STRIKE_GROUND: &str = "strike_ground";

/// A strike's look at `landing`: its bolt from the sky with its veil and
/// its light; its world cue, where it met a body [`STRIKE_BODY`], the
/// strike's sparks and the snap, where it met the ground or a deck
/// [`STRIKE_GROUND`], the ground's sparks and their crackle, each with the
/// thunder and its shake; and a scorch on the ground alone. The strike's
/// number picks the bolt's five to eight pieces.
fn show(world: &mut World, landing: Vec2, met: Met, number: u64) {
    let count = 5 + (number % 4) as usize;
    let pieces: Vec<usize> = (0..count)
        .map(|k| ((number as usize).wrapping_mul(7) + k * 3) % EXITS.len())
        .collect();
    world.spawn_with((
        Bolt {
            landing,
            pieces,
            age: 0.0,
            frames: 0,
        },
        SceneEntity { state_id: GAMEPLAY },
    ));
    let mut light = Light::point(LIGHT_COLOR, LIGHT_REACH);
    light.intensity = STRIKE_LIGHT;
    world.spawn_with((
        Transform::from_position(landing),
        light,
        StrikeLight { age: 0.0 },
        SceneEntity { state_id: GAMEPLAY },
    ));
    let cue = if let Met::Body(_) = met {
        STRIKE_BODY
    } else {
        STRIKE_GROUND
    };
    WorldCue::new(cue, landing).play(world);
    if met == Met::Ground {
        scorch(world, landing);
    }
}

/// A scorch where a strike met the ground, its bottom row's middle on
/// `landing`, under the bodies; past [`SCORCH_CAP`] the oldest goes.
fn scorch(world: &mut World, landing: Vec2) {
    let mut marks: Vec<(Entity, f32)> = world
        .query::<(Entity, &Scorch)>()
        .map(|(entity, scorch)| (entity, scorch.age))
        .collect();
    marks.sort_by(|a, b| b.1.total_cmp(&a.1));
    let over = (marks.len() + 1).saturating_sub(SCORCH_CAP);
    for &(entity, _) in &marks[..over] {
        world.despawn(entity);
    }
    let Some(assets) = world.get_resource_mut::<AssetRegistry>() else {
        return;
    };
    let mut sprite = Sprite::new(assets.intern_sprite(SCORCH));
    sprite.z_order = SCORCH_Z;
    world.spawn_with((
        Transform::from_position(landing - SCORCH_ANCHOR),
        sprite,
        Visibility { visible: true },
        Scorch { age: 0.0 },
        SceneEntity { state_id: GAMEPLAY },
    ));
}

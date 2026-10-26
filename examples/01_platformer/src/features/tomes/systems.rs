//! The tomes' systems: the take and the panel over a book in reach.

use glam::Vec2;
use tungsten::SceneEntity;
use tungsten::core::{AnimationState, AssetRegistry, Entity, Sprite, Time, Transform, World};
use tungsten::kit::{Health, Lifetime, Modifier, PlatformerController, Shield, Stats};

use super::components::{TomeKind, Tomes};
use super::{
    ARCANE_FACTOR, ARCANE_SOURCE, BOUND_CENTRE, BOUND_CLIP, BOUND_COST, BOUND_SECS, BOUND_SPARKS,
    BOUND_TRAIL, BOUND_Z, INTERACT, PACT_DELAY, PACT_RECHARGE, PACT_SHIELD, PACT_SOURCE,
    TAKE_CENTRE, TAKE_SECS, TAKE_Z, nearest,
};
use crate::features::audio;
use crate::features::audio::components::Sound;
use crate::features::combat::SPELL_POWER;
use crate::features::run::Run;
use crate::features::spells::SPELL_COST;
use crate::features::spells::hud;
use crate::game::GAMEPLAY;
use crate::shared::{despawn_tree, just_pressed, spawn_burst, spawn_key_burst};
use crate::shell;

/// The run's player while it lives.
fn live_player(world: &World) -> Option<Entity> {
    let player = world.get_resource::<Run>()?.player;
    world
        .get::<Health>(player)
        .is_some_and(|health| !health.is_dead())
        .then_some(player)
}

/// `tomes_take`, in the tick phase before the kit's `stats_apply`: on an
/// `interact` press the live player takes the nearest book in reach, once
/// a frame (`taken` holds the frame of the last). Its kind joins the
/// player's [`Tomes`], its change applies ([`apply`]), its take clip and key
/// burst play at its centre with its pickup sound, and the book goes with
/// its children.
pub fn take(world: &mut World, taken: &mut Option<u64>) {
    let frame = world.resource::<Time>().frame();
    if *taken == Some(frame) || !just_pressed(world, INTERACT) {
        return;
    }
    let Some(player) = live_player(world) else {
        return;
    };
    let Some((book, kind, at)) = nearest(world, player) else {
        return;
    };
    *taken = Some(frame);
    match world.get_mut::<Tomes>(player) {
        Some(tomes) => tomes.0.push(kind),
        None => world.insert(player, Tomes(vec![kind])),
    }
    apply(world, player, kind);
    despawn_tree(world, book);
    one_shot(world, kind.take_clip(), at - TAKE_CENTRE, TAKE_SECS, TAKE_Z);
    spawn_key_burst(world, kind.take_sparks(), at);
    audio::play(world, kind.sound(), at);
}

/// What taking `kind` changes on `player`. The pact: half the maximum
/// health for good, a `Mul(0.5)` on `max_health` that `stats_apply` writes
/// into `Health` in the take's own step, keeping its share, and a full
/// shield of [`PACT_SHIELD`] that recharges [`PACT_RECHARGE`] a second
/// [`PACT_DELAY`] seconds after a hit. Bounding: an air jump past the
/// controller's free one costs [`BOUND_COST`] mana, with no cap. Arcane
/// power: [`SPELL_COST`] and [`SPELL_POWER`] at a base of 1 where the
/// player lacks them, an older save's, each times [`ARCANE_FACTOR`].
fn apply(world: &mut World, player: Entity, kind: TomeKind) {
    match kind {
        TomeKind::Pact => {
            if let Some(stats) = world.get_mut::<Stats>(player) {
                stats.add_modifier(Stats::MAX_HEALTH, PACT_SOURCE, Modifier::Mul(0.5));
            }
            let shield = Shield::new(PACT_SHIELD).with_recharge(PACT_RECHARGE, PACT_DELAY);
            world.insert(player, shield);
        }
        TomeKind::Bounding => {
            if let Some(controller) = world.get_mut::<PlatformerController>(player) {
                controller.set_air_jump_cost(Some(("mana", BOUND_COST)));
            }
        }
        TomeKind::Arcane => {
            if let Some(stats) = world.get_mut::<Stats>(player) {
                for stat in [SPELL_COST, SPELL_POWER] {
                    if stats.base(stat).is_none() {
                        stats.set_base(stat, 1.0);
                    }
                    stats.add_modifier(stat, ARCANE_SOURCE, Modifier::Mul(ARCANE_FACTOR));
                }
            }
        }
    }
}

/// A bound from `feet`, in place of the double jump's burst and whoosh:
/// its ring opening under them, its sparks and its trail, and its sound.
pub fn bound(world: &mut World, feet: Vec2) {
    one_shot(world, BOUND_CLIP, feet - BOUND_CENTRE, BOUND_SECS, BOUND_Z);
    spawn_burst(world, BOUND_SPARKS, feet);
    spawn_burst(world, BOUND_TRAIL, feet);
    audio::play(world, Sound::Bounded, feet);
}

/// The clip `clip` once, its sprite's top-left at `corner` at `z`, owned by
/// gameplay and gone `secs` later, as it ends.
fn one_shot(world: &mut World, clip: &str, corner: Vec2, secs: f32, z: i32) {
    let Some(assets) = world.get_resource_mut::<AssetRegistry>() else {
        return;
    };
    let mut sprite = Sprite::new(assets.intern_sprite(&format!("{clip}_0")));
    sprite.z_order = z;
    world.spawn_with((
        Transform::from_position(corner),
        sprite,
        AnimationState::new(clip),
        Lifetime::new(secs),
        SceneEntity { state_id: GAMEPLAY },
    ));
}

/// `tomes_panel`, in `PostUpdate` after the camera's update: while gameplay
/// is on top, the HUD's panel stands over the nearest book in reach of the
/// live player and hides with none. Under a pause its root hides with
/// gameplay's cover, and this leaves it be.
pub fn panel(world: &mut World) {
    if shell::top(world) != Some(GAMEPLAY) {
        return;
    }
    let book = live_player(world).and_then(|player| nearest(world, player));
    hud::panel(world, book.map(|(_, kind, at)| (kind, at)));
}

//! The tomes' systems: the take and the panel over a book in reach.

use glam::Vec2;
use tungsten::SceneEntity;
use tungsten::core::{AnimationState, AssetRegistry, Entity, Sprite, Time, Transform, World};
use tungsten::kit::{Health, Lifetime};

use super::components::{TomeKind, Tomes};
use super::{INTERACT, TAKE_CENTRE, TAKE_SECS, TAKE_Z, nearest};
use crate::features::audio;
use crate::features::run::Run;
use crate::features::spells::hud;
use crate::game::GAMEPLAY;
use crate::shared::{despawn_tree, just_pressed, spawn_key_burst};
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
/// player's [`Tomes`], its take clip and key burst play at its centre with
/// its pickup sound, and the book goes with its children.
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
    despawn_tree(world, book);
    clip(world, kind, at);
    spawn_key_burst(world, kind.take_sparks(), at);
    audio::play(world, kind.sound(), at);
}

/// `kind`'s take clip, once, centred on `centre`, owned by gameplay and
/// gone when it ends.
fn clip(world: &mut World, kind: TomeKind, centre: Vec2) {
    let Some(assets) = world.get_resource_mut::<AssetRegistry>() else {
        return;
    };
    let mut sprite = Sprite::new(assets.intern_sprite(&format!("{}_0", kind.take_clip())));
    sprite.z_order = TAKE_Z;
    world.spawn_with((
        Transform::from_position(centre - TAKE_CENTRE),
        sprite,
        AnimationState::new(kind.take_clip()),
        Lifetime::new(TAKE_SECS),
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

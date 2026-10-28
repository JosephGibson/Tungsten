//! The fire geysers (e01hollow): a vent in the hollow's floor that idles
//! [`IDLE_SECS`], sputters [`SPUTTER_SECS`] and erupts [`ERUPT_SECS`], its
//! touch on only through the eruption: 12 fire that burns (combat's
//! reactions) and a throw up and out of the plume. The ice spray chokes it
//! as it crusts lava: [`CRUST_COLD`] seconds of cold on the vent stop it
//! for [`CRUST_SECS`], an eruption cut short in steam, and it starts its
//! idle again after.
//!
//! Its look reads only its phase, the phase's seconds left and the choke,
//! which a save keeps: the vent's sprite, choked or not, and a child plume
//! that steps through the sputter's frames or the eruption's by the phase's
//! time, with embers from the mouth while it sputters or erupts.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten::SceneEntity;
use tungsten::core::{
    AssetRegistry, Entity, ParticleConfigRegistry, ParticleEmitter, ParticleEmitterState, Sprite,
    Time, Transform, Visibility, World,
};
use tungsten::kit::{Children, ContactDamage, LocalOffset, spawn_child};

use super::lava::{CRUST_COLD, CRUST_SECS};
use crate::features::audio::{self, components::Sound};
use crate::game::GAMEPLAY;
use crate::shared::{spawn_burst, spawn_key_burst};

/// The seconds of a geyser's idle, its sputter and its eruption: a cycle
/// of 3.9.
pub const IDLE_SECS: f32 = 2.5;
pub const SPUTTER_SECS: f32 = 0.6;
pub const ERUPT_SECS: f32 = 0.8;
pub const CYCLE_SECS: f32 = IDLE_SECS + SPUTTER_SECS + ERUPT_SECS;
/// The vent's box, its half extents and its centre from the geyser's point,
/// the vent sprite's top-left: over the floor, where a cold ray meets it
/// before the tile stops the ray.
pub const VENT_HALF: Vec2 = Vec2::new(24.0, 12.0);
pub const VENT_CENTRE: Vec2 = Vec2::new(32.0, 20.0);
/// The vent sprite's height: its bottom row stands on the floor's top.
pub const VENT_HEIGHT: f32 = 32.0;
/// The sputter's frame and the plume's, their heights and their counts,
/// each frame's bottom row on the floor's top.
const SPUTTER_HEIGHT: f32 = 64.0;
const PLUME_HEIGHT: f32 = 176.0;
const SPUTTER_FRAMES: usize = 4;
const PLUME_FRAMES: usize = 6;
/// The plume's depth: over the actors, as a fire's.
const PLUME_Z: i32 = 6;
/// The embers' config.
const EMBERS: &str = "ex10_geyser_embers";

/// Where a geyser stands in its cycle.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeyserPhase {
    #[default]
    Idle,
    Sputter,
    Erupt,
}

/// A fire geyser, its prefab's `geyser`: `offset` seconds into its cycle at
/// a run's start, and its state, which its prefab never sets.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Geyser {
    /// Seconds into its cycle at a run's start.
    #[serde(default)]
    pub offset: f32,
    #[serde(skip)]
    pub phase: GeyserPhase,
    /// Seconds left of the phase.
    #[serde(skip)]
    pub left: f32,
    /// Seconds of cold spray on it, draining when unsprayed.
    #[serde(skip)]
    pub cold: f32,
    /// Whether the ice spray reached its vent this step.
    #[serde(skip)]
    pub sprayed: bool,
    /// Seconds left of its choke, 0 when it runs.
    #[serde(skip)]
    pub choked: f32,
    /// Its touch, held aside outside an eruption.
    #[serde(skip)]
    pub touch: Option<ContactDamage>,
    /// Its cycle has begun ([`start`]).
    #[serde(skip)]
    pub started: bool,
}

/// The phase and its seconds left `offset` seconds into a cycle.
#[must_use]
pub fn at_offset(offset: f32) -> (GeyserPhase, f32) {
    let t = offset.rem_euclid(CYCLE_SECS);
    if t < IDLE_SECS {
        (GeyserPhase::Idle, IDLE_SECS - t)
    } else if t < IDLE_SECS + SPUTTER_SECS {
        (GeyserPhase::Sputter, IDLE_SECS + SPUTTER_SECS - t)
    } else {
        (GeyserPhase::Erupt, CYCLE_SECS - t)
    }
}

/// At a run's start, and for a geyser spawned since: each geyser's cycle
/// begins `offset` seconds in, its touch taken aside, on again only in an
/// eruption, and its plume and embers hung from it.
pub fn start(world: &mut World) {
    let geysers: Vec<Entity> = world
        .query::<(Entity, &Geyser)>()
        .filter(|(_, geyser)| !geyser.started)
        .map(|(entity, _)| entity)
        .collect();
    for geyser in geysers {
        let touch = world.remove_component::<ContactDamage>(geyser);
        let Some(state) = world.get_mut::<Geyser>(geyser) else {
            continue;
        };
        let (phase, left) = at_offset(state.offset);
        state.touch = touch;
        state.started = true;
        set(world, geyser, phase, left, 0.0);
        spawn_look(world, geyser);
    }
}

/// Sets `geyser`'s phase, its seconds left and its choke, quietly: its
/// touch on in an eruption and held aside else. A load's, and the cycle's.
pub fn set(world: &mut World, geyser: Entity, phase: GeyserPhase, left: f32, choked: f32) {
    let held = world.remove_component::<ContactDamage>(geyser);
    let Some(state) = world.get_mut::<Geyser>(geyser) else {
        return;
    };
    state.phase = phase;
    state.left = left;
    state.choked = choked;
    let touch = held.or_else(|| state.touch.take());
    if phase == GeyserPhase::Erupt && choked <= 0.0 {
        if let Some(touch) = touch {
            world.insert(geyser, touch);
        }
    } else if let Some(state) = world.get_mut::<Geyser>(geyser) {
        state.touch = touch;
    }
}

/// A cold ray crossed `geyser`'s vent this step: its chill runs; a choked
/// geyser holds as it is.
pub fn chill(world: &mut World, geyser: Entity) {
    if let Some(state) = world.get_mut::<Geyser>(geyser) {
        state.sprayed = true;
    }
}

/// `geyser`'s mouth, the middle of its vent, in world pixels.
#[must_use]
pub fn mouth(world: &World, geyser: Entity) -> Option<Vec2> {
    world
        .get::<Transform>(geyser)
        .map(|at| at.position + VENT_CENTRE)
}

/// What a geyser's step changed: a sputter or an eruption begun, an
/// eruption ended, or a choke set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    Sputtered,
    Erupted,
    Ended,
    Choked,
}

/// `geyser_tick`, in the tick phase before the overlap phase's contact
/// damage: a geyser spawned since begins its cycle ([`start`]); each choke
/// runs down, and at its end the geyser idles [`IDLE_SECS`]; else its chill
/// runs while sprayed and drains else, and chokes it at [`CRUST_COLD`] for
/// [`CRUST_SECS`]; else its phase runs down into the next. An eruption's
/// start puts its touch on with a burst and a roar, its end takes it off;
/// a sputter's start plays its gurgle; a choke ends an eruption at once in
/// steam.
pub fn tick(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    if dt <= 0.0 {
        return;
    }
    start(world);
    let mut changes = Vec::new();
    for (geyser, state) in world.query_mut::<(Entity, &mut Geyser)>() {
        let sprayed = std::mem::take(&mut state.sprayed);
        if state.choked > 0.0 {
            state.choked -= dt;
            if state.choked <= 1e-5 {
                state.choked = 0.0;
                state.phase = GeyserPhase::Idle;
                state.left = IDLE_SECS;
            }
            continue;
        }
        if sprayed {
            state.cold += dt;
            if state.cold >= CRUST_COLD - 1e-4 {
                changes.push((geyser, Change::Choked));
                continue;
            }
        } else {
            state.cold = (state.cold - dt).max(0.0);
        }
        state.left -= dt;
        if state.left <= 1e-5 {
            let (next, secs, change) = match state.phase {
                GeyserPhase::Idle => (GeyserPhase::Sputter, SPUTTER_SECS, Change::Sputtered),
                GeyserPhase::Sputter => (GeyserPhase::Erupt, ERUPT_SECS, Change::Erupted),
                GeyserPhase::Erupt => (GeyserPhase::Idle, IDLE_SECS, Change::Ended),
            };
            state.phase = next;
            state.left += secs;
            changes.push((geyser, change));
        }
    }
    for (geyser, change) in changes {
        let Some(at) = mouth(world, geyser) else {
            continue;
        };
        let left = world.get::<Geyser>(geyser).map_or(0.0, |state| state.left);
        match change {
            Change::Sputtered => audio::play(world, Sound::GeyserSputter, at),
            Change::Erupted => {
                set(world, geyser, GeyserPhase::Erupt, left, 0.0);
                spawn_key_burst(world, "ex10_geyser_burst", at);
                audio::play(world, Sound::GeyserErupt, at);
            }
            Change::Ended => set(world, geyser, GeyserPhase::Idle, left, 0.0),
            Change::Choked => {
                set(world, geyser, GeyserPhase::Idle, IDLE_SECS, CRUST_SECS);
                if let Some(state) = world.get_mut::<Geyser>(geyser) {
                    state.cold = 0.0;
                }
                spawn_burst(world, "ex10_steam_burst", at);
                audio::play(world, Sound::Steamed, at);
            }
        }
    }
}

/// Hangs `geyser`'s plume, hidden, and its embers, drained, from it.
fn spawn_look(world: &mut World, geyser: Entity) {
    let Some(at) = world.get::<Transform>(geyser).map(|t| t.position) else {
        return;
    };
    let Some(asset) = world
        .get_resource_mut::<AssetRegistry>()
        .map(|assets| assets.intern_sprite("ex10_geyser_plume_0"))
    else {
        return;
    };
    let mut sprite = Sprite::new(asset);
    sprite.z_order = PLUME_Z;
    spawn_child(
        world,
        geyser,
        Vec2::new(0.0, VENT_HEIGHT - PLUME_HEIGHT),
        (
            sprite,
            Visibility { visible: false },
            SceneEntity { state_id: GAMEPLAY },
        ),
    );
    let Some(config) = world
        .get_resource::<ParticleConfigRegistry>()
        .and_then(|configs| configs.id_for_name(EMBERS))
    else {
        return;
    };
    // Seeded by its place, so a run's embers look the same whatever
    // entity IDs the ECS hands out.
    let seed = 0x6e_0000 + at.x.max(0.0) as u64;
    let state = ParticleEmitterState {
        drained: true,
        ..ParticleEmitterState::default()
    };
    spawn_child(
        world,
        geyser,
        VENT_CENTRE,
        (
            ParticleEmitter::with_seed(config, seed),
            state,
            SceneEntity { state_id: GAMEPLAY },
        ),
    );
}

/// The plume's sprite and its frame's height for a geyser's phase and its
/// seconds left: the sputter's frames or the eruption's by the phase's
/// time, none at idle or choked.
fn plume(phase: GeyserPhase, left: f32, choked: bool) -> Option<(String, f32)> {
    let frame = |secs: f32, frames: usize| {
        let since = (secs - left).clamp(0.0, secs);
        ((since / secs * frames as f32) as usize).min(frames - 1)
    };
    match phase {
        _ if choked => None,
        GeyserPhase::Idle => None,
        GeyserPhase::Sputter => Some((
            format!(
                "ex10_geyser_sputter_{}",
                frame(SPUTTER_SECS, SPUTTER_FRAMES)
            ),
            SPUTTER_HEIGHT,
        )),
        GeyserPhase::Erupt => Some((
            format!("ex10_geyser_plume_{}", frame(ERUPT_SECS, PLUME_FRAMES)),
            PLUME_HEIGHT,
        )),
    }
}

/// `geyser_look`, after the physics sync and before the frame's particles:
/// each geyser's vent, choked or not; its plume's frame, its bottom on the
/// floor, or hidden; its embers emitting while it sputters or erupts and
/// drained else.
pub fn look(world: &mut World) {
    let geysers: Vec<(Entity, GeyserPhase, f32, bool, Vec<Entity>)> = world
        .query::<(Entity, &Geyser, Option<&Children>)>()
        .map(|(entity, state, children)| {
            let children = children.map(|c| c.iter().collect()).unwrap_or_default();
            (
                entity,
                state.phase,
                state.left,
                state.choked > 0.0,
                children,
            )
        })
        .collect();
    for (geyser, phase, left, choked, children) in geysers {
        let at = world
            .get::<Transform>(geyser)
            .map_or(Vec2::ZERO, |t| t.position);
        let vent = if choked {
            "ex10_geyser_vent_choked"
        } else {
            "ex10_geyser_vent"
        };
        show(world, geyser, vent);
        let shown = plume(phase, left, choked);
        for child in children {
            if let Some(state) = world.get_mut::<ParticleEmitterState>(child) {
                state.drained = shown.is_none();
                continue;
            }
            if !world.has::<Sprite>(child) {
                continue;
            }
            if let Some(visibility) = world.get_mut::<Visibility>(child) {
                visibility.visible = shown.is_some();
            }
            if let Some((name, height)) = &shown {
                show(world, child, name);
                // Its place too, since the frame's propagation may have run.
                let offset = Vec2::new(0.0, VENT_HEIGHT - height);
                if let Some(local) = world.get_mut::<LocalOffset>(child) {
                    local.0 = offset;
                }
                if let Some(transform) = world.get_mut::<Transform>(child) {
                    transform.position = at + offset;
                }
            }
        }
    }
}

/// Shows the sprite `name` on `entity`.
fn show(world: &mut World, entity: Entity, name: &str) {
    let Some(asset) = world
        .get_resource_mut::<AssetRegistry>()
        .map(|assets| assets.intern_sprite(name))
    else {
        return;
    };
    if let Some(sprite) = world.get_mut::<Sprite>(entity)
        && sprite.asset_id != asset
    {
        sprite.asset_id = asset;
    }
}

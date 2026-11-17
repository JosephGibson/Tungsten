//! The fire geysers (e01hollow): a vent in the hollow's floor that idles
//! [`IDLE_SECS`], sputters [`SPUTTER_SECS`] and erupts [`ERUPT_SECS`], its
//! touch on only through the eruption: 12 fire that burns (combat's
//! reactions) and a throw up and out of the plume. The ice spray chokes it
//! as it crusts lava: [`CRUST_COLD`] seconds of cold on the vent stop it
//! for [`CRUST_SECS`], an eruption cut short in steam, and it starts its
//! idle again after.
//!
//! Its cycle is the kit's timed state machine over [`GeyserPhase`], saved
//! as `geyser_phase`, whose hooks switch its touch, the kit's
//! `ContactDamage`, on and off and play each change's sound and burst. Its
//! look reads only the machine's state and the seconds spent in it, which a
//! save keeps: the vent's sprite, choked or not, and a child plume that
//! steps through the sputter's frames or the eruption's by the state's
//! time, with embers from the mouth while it sputters or erupts.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten::SceneEntity;
use tungsten::core::{
    AssetRegistry, Entity, ParticleConfigRegistry, ParticleEmitter, ParticleEmitterState, Sprite,
    Time, Transform, Visibility, World,
};
use tungsten::kit::{
    Children, ContactDamage, LocalOffset, StateMachine, StateMachinePlugin, spawn_child,
};

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
/// The cycle's system, the kit's machine's visits, in the intent phase.
pub const GEYSER_CYCLE: &str = "geyser_cycle";
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

/// Where a geyser stands in its cycle, its machine's states: the cycle's
/// three, each timed into the next, and the choke the ice spray sets, timed
/// back into a fresh idle.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeyserPhase {
    #[default]
    Idle,
    Sputter,
    Erupt,
    Choked,
}

/// A fire geyser, its prefab's `geyser`: `offset` seconds into its cycle at
/// a run's start, and its chill, which its prefab never sets. Its cycle is
/// its `StateMachine<GeyserPhase>`, which [`start`] gives it.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Geyser {
    /// Seconds into its cycle at a run's start.
    #[serde(default)]
    pub offset: f32,
    /// Seconds of cold spray on it, draining when unsprayed.
    #[serde(skip)]
    pub cold: f32,
    /// Whether the ice spray reached its vent this step.
    #[serde(skip)]
    pub sprayed: bool,
    /// Its cycle has begun ([`start`]).
    #[serde(skip)]
    pub started: bool,
}

/// [`Geyser`]'s saved form: its offset, its chill and whether its cycle has
/// begun. The cycle itself is its machine's, and its touch's switch the
/// kit's, each saved beside it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeyserForm {
    pub offset: f32,
    pub cold: f32,
    pub started: bool,
}

impl From<&Geyser> for GeyserForm {
    fn from(geyser: &Geyser) -> Self {
        Self {
            offset: geyser.offset,
            cold: geyser.cold,
            started: geyser.started,
        }
    }
}

impl From<GeyserForm> for Geyser {
    fn from(form: GeyserForm) -> Self {
        Self {
            offset: form.offset,
            cold: form.cold,
            sprayed: false,
            started: form.started,
        }
    }
}

/// The phase `offset` seconds into a cycle, and the seconds into it.
#[must_use]
pub fn at_offset(offset: f32) -> (GeyserPhase, f32) {
    let t = offset.rem_euclid(CYCLE_SECS);
    if t < IDLE_SECS {
        (GeyserPhase::Idle, t)
    } else if t < IDLE_SECS + SPUTTER_SECS {
        (GeyserPhase::Sputter, t - IDLE_SECS)
    } else {
        (GeyserPhase::Erupt, t - IDLE_SECS - SPUTTER_SECS)
    }
}

/// The cycle as the kit's plugin: each phase timed into the next and the
/// choke into a fresh idle, with the hooks that switch the touch and play
/// each change ([`sputter`], [`erupt`], [`end`], [`choke`]).
#[must_use]
pub fn machine() -> StateMachinePlugin<GeyserPhase> {
    StateMachinePlugin::new(GEYSER_CYCLE)
        .after(GeyserPhase::Idle, IDLE_SECS, GeyserPhase::Sputter)
        .after(GeyserPhase::Sputter, SPUTTER_SECS, GeyserPhase::Erupt)
        .after(GeyserPhase::Erupt, ERUPT_SECS, GeyserPhase::Idle)
        .after(GeyserPhase::Choked, CRUST_SECS, GeyserPhase::Idle)
        .on_enter(GeyserPhase::Sputter, sputter)
        .on_enter(GeyserPhase::Erupt, erupt)
        .on_exit(GeyserPhase::Erupt, end)
        .on_enter(GeyserPhase::Choked, choke)
}

/// Whether `geyser`'s machine came to its state from another: a cycle
/// starting part-way, or staged, enters its first state quietly.
fn arrived(world: &World, geyser: Entity) -> bool {
    world
        .get::<StateMachine<GeyserPhase>>(geyser)
        .is_some_and(|machine| machine.previous().is_some())
}

/// Switches `geyser`'s touch on or off, where it stays on the entity.
fn switch(world: &mut World, geyser: Entity, on: bool) {
    if let Some(touch) = world.get_mut::<ContactDamage>(geyser) {
        touch.set_on(on);
    }
}

/// A sputter begun: its gurgle.
fn sputter(world: &mut World, geyser: Entity) {
    if arrived(world, geyser)
        && let Some(at) = mouth(world, geyser)
    {
        audio::play(world, Sound::GeyserSputter, at);
    }
}

/// An eruption begun: its touch on, with its burst and its roar.
fn erupt(world: &mut World, geyser: Entity) {
    switch(world, geyser, true);
    if arrived(world, geyser)
        && let Some(at) = mouth(world, geyser)
    {
        spawn_key_burst(world, "ex10_geyser_burst", at);
        audio::play(world, Sound::GeyserErupt, at);
    }
}

/// An eruption left, ended or choked: its touch off.
fn end(world: &mut World, geyser: Entity) {
    switch(world, geyser, false);
}

/// A choke set: its touch off, in steam.
fn choke(world: &mut World, geyser: Entity) {
    switch(world, geyser, false);
    if arrived(world, geyser)
        && let Some(at) = mouth(world, geyser)
    {
        spawn_burst(world, "ex10_steam_burst", at);
        audio::play(world, Sound::Steamed, at);
    }
}

/// At a run's start, and for a geyser spawned since: each geyser's cycle
/// begins `offset` seconds in ([`set`]), and its plume and embers are hung
/// from it.
pub fn start(world: &mut World) {
    let geysers: Vec<(Entity, f32)> = world
        .query::<(Entity, &Geyser)>()
        .filter(|(_, geyser)| !geyser.started)
        .map(|(entity, geyser)| (entity, geyser.offset))
        .collect();
    for (geyser, offset) in geysers {
        if let Some(state) = world.get_mut::<Geyser>(geyser) {
            state.started = true;
        }
        let (phase, elapsed) = at_offset(offset);
        set(world, geyser, phase, elapsed);
        spawn_look(world, geyser);
    }
}

/// Sets `geyser` in `phase`, `elapsed` seconds into it, quietly: a fresh
/// machine, whose first visit plays nothing, and its touch on only in an
/// eruption. A cycle's start, and the tests' staging.
pub fn set(world: &mut World, geyser: Entity, phase: GeyserPhase, elapsed: f32) {
    world.insert(geyser, StateMachine::new(phase).starting_at(elapsed));
    switch(world, geyser, phase == GeyserPhase::Erupt);
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

/// `geyser_tick`, in the tick phase before the intent phase's cycle
/// ([`machine`]) and the overlap phase's contact damage: a geyser spawned
/// since begins its cycle ([`start`]); a choked one holds as it is; else
/// its chill runs while sprayed and drains else, and at [`CRUST_COLD`] asks
/// its machine for the choke, its chill spent, which the step's cycle
/// enters: an eruption cut short at once, in steam.
pub fn tick(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    if dt <= 0.0 {
        return;
    }
    start(world);
    for (state, machine) in world.query_mut::<(&mut Geyser, &mut StateMachine<GeyserPhase>)>() {
        let sprayed = std::mem::take(&mut state.sprayed);
        if machine.state() == GeyserPhase::Choked {
            continue;
        }
        if sprayed {
            state.cold += dt;
            if state.cold >= CRUST_COLD - 1e-4 {
                state.cold = 0.0;
                machine.go(GeyserPhase::Choked);
            }
        } else {
            state.cold = (state.cold - dt).max(0.0);
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

/// The plume's sprite and its frame's height for a geyser's phase and the
/// seconds spent in it: the sputter's frames or the eruption's by the
/// phase's time, none at idle or choked.
fn plume(phase: GeyserPhase, elapsed: f32) -> Option<(String, f32)> {
    let frame = |secs: f32, frames: usize| {
        let since = elapsed.clamp(0.0, secs);
        ((since / secs * frames as f32) as usize).min(frames - 1)
    };
    match phase {
        GeyserPhase::Idle | GeyserPhase::Choked => None,
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
    let geysers: Vec<(Entity, GeyserPhase, f32, Vec<Entity>)> = world
        .query::<(Entity, &Geyser, Option<&Children>)>()
        .filter_map(|(entity, _, children)| {
            let machine = world.get::<StateMachine<GeyserPhase>>(entity)?;
            let children = children.map(|c| c.iter().collect()).unwrap_or_default();
            Some((entity, machine.state(), machine.elapsed(), children))
        })
        .collect();
    for (geyser, phase, elapsed, children) in geysers {
        let at = world
            .get::<Transform>(geyser)
            .map_or(Vec2::ZERO, |t| t.position);
        let vent = if phase == GeyserPhase::Choked {
            "ex10_geyser_vent_choked"
        } else {
            "ex10_geyser_vent"
        };
        show(world, geyser, vent);
        let shown = plume(phase, elapsed);
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

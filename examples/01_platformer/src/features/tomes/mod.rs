//! The tomes (01 (l)): three books hidden off the routes, each taken with
//! an `interact` press after a panel over it has stated its gift and its
//! cost. A book is a map object from its `tome_<kind>` prefab, which bobs
//! on the kit's oscillator, glows and trails its motes; no save names it,
//! so the map spawns all three and a load takes off those the player
//! holds. The player's [`Tomes`] lists the kinds taken, in order, which the
//! HUD's strip shows. The demonic pact halves the maximum health for good
//! for a shield over it that comes back after a hit; the tome of bounding
//! lets every air jump past the double jump cost mana, with no cap, each
//! one a bound with its ring and its sound; the tome of arcane power
//! doubles the player's `spell_cost` and `spell_power`, so every spell
//! costs twice and hits twice as hard.

pub mod components;
pub mod systems;

use glam::Vec2;
use tungsten::Plugin;
use tungsten::core::{Entity, Position, Schedule, Stage, Transform, World, system};
use tungsten::kit::{Phase, STATS_APPLY, ensure_phases};
use tungsten::plugins::CAMERA_UPDATE;

pub use components::{Tome, TomeKind, Tomes};

use crate::shared::body_centre;

/// How near the player's body centre a book's centre must lie for its
/// panel and its take, in world pixels.
pub const REACH: f32 = 64.0;
/// A book's centre from its sprite's top-left, and its take clip's.
pub const BOOK_CENTRE: Vec2 = Vec2::splat(16.0);
pub const TAKE_CENTRE: Vec2 = Vec2::splat(32.0);
/// The take clip's game seconds, six frames at 0.08 s, and its draw order,
/// over the book's.
pub const TAKE_SECS: f32 = 0.48;
pub const TAKE_Z: i32 = 13;
/// The action that takes a book.
pub const INTERACT: &str = "interact";
/// The pact's modifier source on the player's stats, and its shield: its
/// size, what it recharges a second and the game seconds after a hit it
/// waits first (Q37).
pub const PACT_SOURCE: &str = "tome.pact";
pub const PACT_SHIELD: f32 = 50.0;
pub const PACT_RECHARGE: f32 = 20.0;
pub const PACT_DELAY: f32 = 3.0;
/// What a bound, an air jump past the double jump, costs (Q40: arcane power
/// leaves it as it is).
pub const BOUND_COST: f32 = 25.0;
/// The bound's ring under the feet: its clip, its centre from its
/// top-left, which stands on the feet, its game seconds, six frames at
/// 0.05 s, and its draw order, over the wizard's; and the sparks ringing
/// out from it and the motes it leaves as he rises.
pub const BOUND_CLIP: &str = "ex10_bound_ring";
pub const BOUND_CENTRE: Vec2 = Vec2::new(32.0, 16.0);
pub const BOUND_SECS: f32 = 0.3;
pub const BOUND_Z: i32 = 11;
pub const BOUND_SPARKS: &str = "ex10_bound_sparks";
pub const BOUND_TRAIL: &str = "ex10_bound_trail";
/// Arcane power's modifier source on the player's stats, and what it
/// multiplies `spell_cost` and `spell_power` by.
pub const ARCANE_SOURCE: &str = "tome.arcane";
pub const ARCANE_FACTOR: f32 = 2.0;

/// The tomes as a plugin: the `tome` component the books' prefabs name,
/// the take in the tick phase before the kit's `stats_apply`, so what a
/// tome changes of the player's stats lands in its own step, at most one a
/// frame, since a press reads as fresh through each of the frame's steps;
/// and the panel in `PostUpdate` after the camera's update, so it stands
/// over the book the frame draws.
pub struct TomesPlugin;

impl Plugin for TomesPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        // Before the manifest loads, which checks each prefab's components.
        world.register_component::<Tome>("tome");
        ensure_phases(schedule);
        let mut taken = None;
        let take = system("tomes_take", move |world: &mut World| {
            systems::take(world, &mut taken);
        })
        .before(STATS_APPLY);
        schedule.add(Stage::FixedUpdate, Phase::Tick.join(take));
        let panel = system("tomes_panel", systems::panel).after(CAMERA_UPDATE);
        schedule.add(Stage::PostUpdate, panel);
    }
}

/// A book's centre: its point, which its bob moves, and [`BOOK_CENTRE`].
#[must_use]
pub fn centre(world: &World, book: Entity) -> Option<Vec2> {
    let at = world
        .get::<Position>(book)
        .map(|at| at.0)
        .or_else(|| world.get::<Transform>(book).map(|at| at.position))?;
    Some(at + BOOK_CENTRE)
}

/// The book nearest `player`'s body centre within [`REACH`], its kind and
/// its centre.
#[must_use]
pub fn nearest(world: &World, player: Entity) -> Option<(Entity, TomeKind, Vec2)> {
    let from = body_centre(world, player)?;
    world
        .query::<(Entity, &Tome)>()
        .filter_map(|(book, tome)| Some((book, tome.kind, centre(world, book)?)))
        .filter(|(.., at)| at.distance(from) <= REACH)
        .min_by(|a, b| a.2.distance(from).total_cmp(&b.2.distance(from)))
}

/// The tomes `player` holds, in the order taken.
#[must_use]
pub fn held(world: &World, player: Entity) -> &[TomeKind] {
    world
        .get::<Tomes>(player)
        .map_or(&[][..], |tomes| tomes.0.as_slice())
}

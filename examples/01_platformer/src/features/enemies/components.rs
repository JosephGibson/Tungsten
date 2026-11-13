//! The enemies' components, resources and events.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten::core::save::RefCodec;
use tungsten::core::{Entity, RigidBody};
use tungsten::kit::TopDownMover;

/// One of the six enemies, which its prefab names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnemyKind {
    Wisp,
    Crawler,
    Beetle,
    Bat,
    Troll,
    Goblin,
}

impl EnemyKind {
    /// Every kind, in the order the summon slot takes them.
    pub const ALL: [EnemyKind; 6] = [
        EnemyKind::Wisp,
        EnemyKind::Crawler,
        EnemyKind::Beetle,
        EnemyKind::Bat,
        EnemyKind::Troll,
        EnemyKind::Goblin,
    ];

    /// Its name, its prefab's and its clips' stem.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            EnemyKind::Wisp => "wisp",
            EnemyKind::Crawler => "crawler",
            EnemyKind::Beetle => "beetle",
            EnemyKind::Bat => "bat",
            EnemyKind::Troll => "troll",
            EnemyKind::Goblin => "goblin",
        }
    }
}

/// An enemy's variant of its kind (e01flask): the kind itself, or the
/// armoured troll, the troll in iron plate, its own prefab, icon and clips
/// on the troll's body, rules and ice.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Variant {
    #[default]
    Plain,
    Armoured,
}

impl Variant {
    /// Whether it is the kind itself, which a save leaves out.
    #[must_use]
    pub const fn is_plain(&self) -> bool {
        matches!(self, Variant::Plain)
    }
}

/// An enemy, which its prefab names `enemy` with its kind: where it patrols
/// and returns to, its body's centre, which its spawn sets, how far from
/// its home, across, a patrol may take it, which a map placement may set
/// (the falls' crawler), and its variant, which its prefab sets; the way it
/// faces is its prefab's `facing` (`enemies::heading_of`). Saved; a schema
/// 8 save's enemies have no leash, and an older save's no variant: plain.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Enemy {
    pub kind: EnemyKind,
    #[serde(default, skip_serializing_if = "Variant::is_plain")]
    pub variant: Variant,
    #[serde(default)]
    pub home: [f32; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leash: Option<f32>,
}

impl Enemy {
    /// Its home's point.
    #[must_use]
    pub fn home(&self) -> Vec2 {
        Vec2::from(self.home)
    }
}

/// What an enemy knows: where it last saw the player, the point it
/// searches, the point a neighbour's alert passed it, the attack its tell
/// chose, so a load swings the same one and its clip comes from saved
/// state, the seconds a fresh alert's "!" has left, which no state of its
/// machine can hold (Alert ends sooner), the seconds before a troll may
/// slam again, and the seconds before a crawler's snowball or a goblin's
/// throw or dash-in may come again. Saved; an older save's brains chose
/// none, show none and wait for nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Brain {
    pub last_seen: Option<[f32; 2]>,
    pub search: Option<[f32; 2]>,
    pub alert: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strike: Option<Strike>,
    #[serde(default, skip_serializing_if = "is_none_left")]
    pub alerted: f32,
    #[serde(default, skip_serializing_if = "is_none_left")]
    pub slam_cooldown: f32,
    #[serde(default, skip_serializing_if = "is_none_left")]
    pub cooldown: f32,
}

fn is_none_left(left: &f32) -> bool {
    *left <= 0.0
}

/// The attack an enemy's tell chose: a goblin's rock, bomb, stab or
/// dash-in, a troll's slam, swipe or throw, a crawler's snowball.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strike {
    Rock,
    Bomb,
    Slam,
    Swipe,
    Throw,
    Stab,
    Snowball,
    Dash,
}

/// An enemy's sight: the seconds to its next check and whether its last
/// check saw the player. Saved, so a load checks when the run would have.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sight {
    pub next: f32,
    pub sees: bool,
}

/// An enemy's state on the kit's state machine (`D-158`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnemyState {
    /// About its home, by kind.
    Patrol,
    /// It has spotted the player, or a neighbour's alert reached it: its
    /// tell, then a chase.
    Alert,
    /// Closing in while it sees the player.
    Chase,
    /// Its attack's tell.
    WindUp,
    /// Its attack.
    Attack,
    /// After its attack.
    Recover,
    /// At the point it last saw the player, looking.
    Search,
    /// Back to its home.
    Return,
    /// A goblin's flight while it burns (e01foes): it runs from the player,
    /// flailing.
    Panic,
}

/// A frozen flyer's own mover, set aside while it falls on a borrowed
/// controller and given back at the thaw. Saved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Displaced(pub TopDownMover);

/// What a troll holds over its head: the troll, and the object's own body,
/// which it gets back at the throw or the drop; while held, its body is a
/// sensor. Rebuilt at a load from the troll's [`Holding`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Held {
    pub by: Entity,
    pub body: RigidBody,
}

/// The troll's side of a hold, the object it holds over its head: on the
/// troll from the lift to the throw or the drop, saved by reference, so a
/// load lifts the object again.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Holding(pub Entity);

/// [`Holding`]'s saved form: nothing but its reference.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HoldingForm {}

impl From<&Holding> for HoldingForm {
    fn from(_: &Holding) -> Self {
        Self {}
    }
}

impl From<HoldingForm> for Holding {
    fn from(_: HoldingForm) -> Self {
        Self(Entity::PLACEHOLDER)
    }
}

/// [`Holding`]'s reference: the object, under `held`.
#[must_use]
pub fn holding_refs() -> RefCodec<Holding> {
    RefCodec {
        save: |holding, out| out.put("held", holding.0),
        load: |holding, refs| {
            if let Some(object) = refs.first("held") {
                holding.0 = object;
            }
        },
        keys: |_| vec!["held".to_owned()],
    }
}

/// A thrown body whose impacts its thrower is credited with: the body, the
/// thrower's save ID, the thrower's damage factor at the throw and the
/// seconds left. A body's first impact ends it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Throw {
    pub body: Entity,
    pub thrower: u64,
    pub factor: f32,
    pub left: f32,
}

/// The throws in flight, in throw order: a World resource. Saved, each
/// body by reference ([`throws_refs`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Throws(pub Vec<Throw>);

/// A throw's saved form: all but its body, which its reference holds.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThrowForm {
    pub thrower: u64,
    pub factor: f32,
    pub left: f32,
}

/// [`Throws`]' saved form, in throw order.
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct ThrowsForm(pub Vec<ThrowForm>);

impl From<&Throws> for ThrowsForm {
    fn from(throws: &Throws) -> Self {
        Self(
            throws
                .0
                .iter()
                .map(|throw| ThrowForm {
                    thrower: throw.thrower,
                    factor: throw.factor,
                    left: throw.left,
                })
                .collect(),
        )
    }
}

impl From<ThrowsForm> for Throws {
    fn from(form: ThrowsForm) -> Self {
        Self(
            form.0
                .into_iter()
                .map(|throw| Throw {
                    body: Entity::PLACEHOLDER,
                    thrower: throw.thrower,
                    factor: throw.factor,
                    left: throw.left,
                })
                .collect(),
        )
    }
}

/// [`Throws`]' references: each throw's body under its place in throw
/// order; a throw whose body did not load goes.
#[must_use]
pub fn throws_refs() -> RefCodec<Throws> {
    RefCodec {
        save: |throws, out| {
            for (i, throw) in throws.0.iter().enumerate() {
                out.put(&i.to_string(), throw.body);
            }
        },
        load: |throws, refs| {
            let mut i = 0;
            throws.0.retain_mut(|throw| {
                let body = refs.first(&i.to_string());
                i += 1;
                match body {
                    Some(body) => {
                        throw.body = body;
                        true
                    }
                    None => false,
                }
            });
        },
        keys: |throws| (0..throws.0.len()).map(|i| i.to_string()).collect(),
    }
}

/// A crawler's snowball in flight (e01foes): its thrower's save ID, the
/// thrower's damage factor at the throw, and the seconds before it bursts
/// in the air. Inserted at the throw, so its prefab names no 01 component.
/// Saved, its thrower by save ID.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snowball {
    pub thrower: Option<u64>,
    pub factor: f32,
    pub left: f32,
}

impl Throws {
    /// `body`'s throw, while it flies.
    #[must_use]
    pub fn of(&self, body: Entity) -> Option<&Throw> {
        self.0.iter().find(|throw| throw.body == body)
    }
}

/// The save IDs of the enemies the map placed for this run, sorted: a
/// World resource the run's start fills.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlacedEnemies(pub Vec<u64>);

/// An enemy's sight of the player turning on: the spotter and the point it
/// saw from, which alerts its neighbours; each that comes freshly alert
/// stings (`Sound::Alerted`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spotted {
    pub spotter: Entity,
    pub at: Vec2,
    pub player: Entity,
}

/// The clip an enemy shows and its frame at the last look, and the second
/// ember emitter a wisp's flare lights. Presentation, never saved.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Shown {
    pub clip: String,
    pub frame: usize,
    pub flare: Option<Entity>,
}

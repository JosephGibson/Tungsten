//! The enemies' components, resources and events.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten::core::{Entity, RigidBody};
use tungsten::kit::{ContactDamage, TopDownMover};

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

fn right() -> f32 {
    1.0
}

/// An enemy, which its prefab names `enemy` with its kind: where it patrols
/// and returns to, its body's centre, which its spawn sets, its heading,
/// 1 right or -1 left, which its hooks set and gameplay reads (the beetle's
/// front), and how far from its home, across, a patrol may take it, which
/// a map placement may set (the falls' crawler). Saved; a schema 8 save's
/// enemies have no leash.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Enemy {
    pub kind: EnemyKind,
    #[serde(default)]
    pub home: [f32; 2],
    #[serde(default = "right")]
    pub heading: f32,
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
#[derive(Debug, Clone, PartialEq)]
pub struct Displaced(pub TopDownMover);

/// A frozen enemy's touch, set aside so it hurts nothing by touch and given
/// back at the thaw with its targets' times. The save holds those times; a
/// load gives the touch back to the prefab's, and the next hold sets it
/// aside again.
#[derive(Debug, Clone, PartialEq)]
pub struct HeldTouch(pub ContactDamage);

/// What a troll holds over its head: the troll, and the object's own body,
/// which it gets back at the throw or the drop; while held, its body is a
/// sensor. Rebuilt at a load from the troll's save.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Held {
    pub by: Entity,
    pub body: RigidBody,
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

/// The throws in flight, in throw order: a World resource. Saved.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Throws(pub Vec<Throw>);

/// A crawler's snowball in flight (e01foes): its thrower's save ID, the
/// thrower's damage factor at the throw, and the seconds before it bursts
/// in the air. Inserted at the throw, so its prefab names no 01 component.
/// Saved.
#[derive(Debug, Clone, Copy, PartialEq)]
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

/// A crawler's scare: the game time the staff light's edge last refused
/// its walk, whose scared clip shows for `crawler::SCARED_HOLD` after (B4).
/// Presentation, never saved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scared {
    pub at: f64,
}

/// The clip an enemy shows and its frame at the last look, and the second
/// ember emitter a wisp's flare lights. Presentation, never saved.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Shown {
    pub clip: String,
    pub frame: usize,
    pub flare: Option<Entity>,
}

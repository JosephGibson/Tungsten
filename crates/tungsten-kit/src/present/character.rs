//! The character animator: an animated sprite's clip picked each frame by
//! the entity's motion and facing.

use std::fmt;

use glam::Vec2;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use tungsten_core::physics::{Position, PrevPosition, Velocity};
use tungsten_core::{
    AnimationRegistry, AnimationState, Entity, Inspectable, MissingAsset, Plugin, Schedule,
    SpawnContext, Sprite, Time, World, system,
};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, PLATFORMER, TOP_DOWN, describe,
    round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::non_negative;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::motion::platformer::PlatformerController;
use crate::present::animation::ANIMATION_TICK;
use crate::present::facing::{FACING, Facing};

/// [`CharacterAnimatorPlugin`]'s system, in the present phase.
pub const CHARACTER_ANIMATOR: &str = "character_animator";

/// The scene component the plugin registers.
const CHARACTER_ANIMATOR_COMPONENT: &str = "character_animator";

/// The default speed from which a character moves, in world pixels per
/// second.
const DEFAULT_MIN_SPEED: f32 = 10.0;

/// The clip a [`CharacterAnimator`] state shows: one animation, or a set
/// by the way the entity faces, which names `left` and `right` and may
/// name `up` and `down`; a set without them shows the last side faced while
/// the entity faces up or down.
///
/// Data form: a manifest animation ID, `"hero_run"`, or
/// `{ "left": "seeker_walk_left", "right": "seeker_walk_right", "up": "seeker_walk_up", "down": "seeker_walk_down" }`.
///
/// # Examples
///
/// ```
/// use tungsten_kit::Clips;
///
/// let run = Clips::sides("hero_run_left", "hero_run_right");
/// let data = serde_json::to_value(&run).unwrap();
/// assert_eq!(data, serde_json::json!({ "left": "hero_run_left", "right": "hero_run_right" }));
/// assert_eq!(serde_json::from_value::<Clips>(data).unwrap(), run);
/// assert_eq!(Clips::from("hero_idle"), Clips::one("hero_idle"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clips {
    kind: ClipsKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ClipsKind {
    One(String),
    Set(ClipSet),
}

/// A clip set by facing, as data writes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClipSet {
    left: String,
    right: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    up: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    down: Option<String>,
}

impl Clips {
    /// One animation, whichever way the entity faces.
    #[must_use]
    pub fn one(id: impl Into<String>) -> Self {
        Self {
            kind: ClipsKind::One(id.into()),
        }
    }

    /// An animation for each side: `left` while the entity faces left,
    /// `right` while it faces right, and the last side's while it faces up
    /// or down.
    #[must_use]
    pub fn sides(left: impl Into<String>, right: impl Into<String>) -> Self {
        Self {
            kind: ClipsKind::Set(ClipSet {
                left: left.into(),
                right: right.into(),
                up: None,
                down: None,
            }),
        }
    }

    /// An animation for each of four ways the entity faces, by its
    /// direction's dominant axis.
    #[must_use]
    pub fn four(
        left: impl Into<String>,
        right: impl Into<String>,
        up: impl Into<String>,
        down: impl Into<String>,
    ) -> Self {
        Self {
            kind: ClipsKind::Set(ClipSet {
                left: left.into(),
                right: right.into(),
                up: Some(up.into()),
                down: Some(down.into()),
            }),
        }
    }

    /// Every animation ID it names, with its field under `field` (`move`,
    /// `move.left`), for a load's check.
    fn ids<'a>(&'a self, field: &'a str) -> Vec<(String, &'a str)> {
        match &self.kind {
            ClipsKind::One(id) => vec![(field.to_string(), id.as_str())],
            ClipsKind::Set(set) => {
                let mut ids = vec![
                    (format!("{field}.left"), set.left.as_str()),
                    (format!("{field}.right"), set.right.as_str()),
                ];
                if let Some(up) = &set.up {
                    ids.push((format!("{field}.up"), up.as_str()));
                }
                if let Some(down) = &set.down {
                    ids.push((format!("{field}.down"), down.as_str()));
                }
                ids
            }
        }
    }

    /// The animation for `direction`, the way the entity faces or moves:
    /// a set's side by the dominant axis, `up` and `down` when it names them
    /// and the last side faced otherwise. `left` is the last side, updated
    /// as the entity faces left or right.
    fn pick(&self, direction: Vec2, left: &mut bool) -> &str {
        let set = match &self.kind {
            ClipsKind::One(id) => return id,
            ClipsKind::Set(set) => set,
        };
        let vertical = if direction.x.abs() >= direction.y.abs() {
            if direction.x != 0.0 {
                *left = direction.x < 0.0;
            }
            None
        } else if direction.y < 0.0 {
            set.up.as_deref()
        } else {
            set.down.as_deref()
        };
        vertical.unwrap_or(if *left { &set.left } else { &set.right })
    }

    /// The clip in a line, for inspector rows.
    fn summary(&self) -> String {
        match &self.kind {
            ClipsKind::One(id) => id.clone(),
            ClipsKind::Set(set) => match (&set.up, &set.down) {
                (Some(up), Some(down)) => {
                    format!("{} | {} | {up} | {down}", set.left, set.right)
                }
                (Some(up), None) => format!("{} | {} | {up} | -", set.left, set.right),
                (None, Some(down)) => format!("{} | {} | - | {down}", set.left, set.right),
                (None, None) => format!("{} | {}", set.left, set.right),
            },
        }
    }
}

impl From<&str> for Clips {
    fn from(id: &str) -> Self {
        Self::one(id)
    }
}

impl From<String> for Clips {
    fn from(id: String) -> Self {
        Self::one(id)
    }
}

impl Serialize for Clips {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.kind {
            ClipsKind::One(id) => serializer.serialize_str(id),
            ClipsKind::Set(set) => set.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Clips {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ClipsVisitor;

        impl<'de> Visitor<'de> for ClipsVisitor {
            type Value = Clips;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an animation ID, or { left, right, up, down } of animation IDs")
            }

            fn visit_str<E: de::Error>(self, id: &str) -> Result<Clips, E> {
                Ok(Clips::one(id))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Clips, A::Error> {
                let set = ClipSet::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(Clips {
                    kind: ClipsKind::Set(set),
                })
            }
        }

        deserializer.deserialize_any(ClipsVisitor)
    }
}

/// What a character is doing, by which its clip is picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum State {
    #[default]
    Idle,
    Move,
    Rise,
    Fall,
}

impl State {
    fn name(self) -> &'static str {
        match self {
            State::Idle => "idle",
            State::Move => "move",
            State::Rise => "rise",
            State::Fall => "fall",
        }
    }
}

/// Picks its entity's animation clip each frame by what the entity is
/// doing: `rise` or `fall` while a
/// [`PlatformerController`](crate::PlatformerController) is off the ground,
/// by its velocity's sign; `move` at its minimum speed or faster; else
/// `idle`. A state's clip is one animation, or a set ([`Clips`]) whose
/// entry is picked by the entity's [`Facing`](crate::Facing) by its
/// dominant axis, else by its velocity's side. `move`, `rise` and `fall`
/// fall back to `idle` when not given. Landing, hurt and attack clips need
/// events, so they are a game's, through a
/// [`StateMachine`](crate::StateMachine) or Rust.
///
/// [`CharacterAnimatorPlugin`]'s system runs after the animation tick: on a
/// change of clip it replaces the entity's `AnimationState` and writes the
/// new clip's first frame into `Sprite.asset_id`, so the frame that picks a
/// clip shows its first frame however short, and the clip advances from
/// the next frame. It needs an `AnimationRegistry` and a `Sprite`, and works
/// on children.
///
/// Scene name `character_animator`:
/// `{ "idle": "hero_idle", "move": "hero_run", "rise": "hero_jump", "fall": "hero_fall", "min_speed": 10 }`,
/// each clip an animation ID or a set by facing, `idle` needed and
/// `min_speed` in world pixels per second, 10 by default; the load checks
/// every ID against the manifest's animations, naming the field
/// (`move.left`). It is data: a game's save can hold it.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::assets::AnimationFrame;
/// use tungsten_core::physics::Velocity;
/// use tungsten_core::{
///     AnimationData, AnimationRegistry, AnimationState, Plugin, Schedule, Sprite,
///     SpriteAssetId, Stage, World,
/// };
/// use tungsten_kit::{CharacterAnimator, CharacterAnimatorPlugin, Clips};
///
/// let clip = |first: u32| AnimationData {
///     looping: true,
///     frames: vec![AnimationFrame { sprite: SpriteAssetId::new(first), duration_ms: 100 }],
/// };
/// let mut clips = AnimationRegistry::new();
/// for (id, first) in [("hero_idle", 0), ("hero_run_left", 1), ("hero_run_right", 2)] {
///     clips.insert(id.to_string(), clip(first));
/// }
/// let mut world = World::new();
/// world.insert_resource(clips);
/// let mut schedule = Schedule::new();
/// CharacterAnimatorPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// let animator = CharacterAnimator::new("hero_idle")
///     .with_move(Clips::sides("hero_run_left", "hero_run_right"))
///     .with_min_speed(20.0);
/// assert_eq!(animator.state(), "idle", "idle until its first frame");
/// let hero = world.spawn_with((animator, Velocity(Vec2::new(-90.0, 0.0)), Sprite::new(SpriteAssetId::new(0))));
/// schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
/// // Running left: the left clip, its first frame shown at once.
/// assert_eq!(world.get::<CharacterAnimator>(hero).unwrap().state(), "move");
/// assert_eq!(world.get::<AnimationState>(hero).unwrap().animation_id, "hero_run_left");
/// assert_eq!(world.get::<Sprite>(hero).unwrap().asset_id, SpriteAssetId::new(1));
///
/// let saved = serde_json::to_string(world.get::<CharacterAnimator>(hero).unwrap()).unwrap();
/// let loaded: CharacterAnimator = serde_json::from_str(&saved).unwrap();
/// assert_eq!(&loaded, world.get::<CharacterAnimator>(hero).unwrap());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "CharacterAnimatorFields", into = "CharacterAnimatorFields")]
pub struct CharacterAnimator {
    idle: Clips,
    moving: Option<Clips>,
    rise: Option<Clips>,
    fall: Option<Clips>,
    min_speed: f32,
    /// The state at its last frame.
    state: State,
    /// The animation it showed at its last frame.
    clip: Option<String>,
    /// Whether the entity last faced left, for a set without `up` and
    /// `down`.
    left: bool,
}

/// An animator as a save writes it, before its values are checked.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CharacterAnimatorFields {
    idle: Clips,
    #[serde(rename = "move", default, skip_serializing_if = "Option::is_none")]
    moving: Option<Clips>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rise: Option<Clips>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fall: Option<Clips>,
    #[serde(default = "default_min_speed")]
    min_speed: f32,
    #[serde(default)]
    state: State,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    clip: Option<String>,
    #[serde(default)]
    left: bool,
}

fn default_min_speed() -> f32 {
    DEFAULT_MIN_SPEED
}

impl TryFrom<CharacterAnimatorFields> for CharacterAnimator {
    type Error = String;

    fn try_from(fields: CharacterAnimatorFields) -> Result<Self, Self::Error> {
        if !(fields.min_speed.is_finite() && fields.min_speed >= 0.0) {
            return Err(format!(
                "min_speed: must be finite and >= 0 (got {})",
                fields.min_speed
            ));
        }
        Ok(Self {
            idle: fields.idle,
            moving: fields.moving,
            rise: fields.rise,
            fall: fields.fall,
            min_speed: fields.min_speed,
            state: fields.state,
            clip: fields.clip,
            left: fields.left,
        })
    }
}

impl From<CharacterAnimator> for CharacterAnimatorFields {
    fn from(animator: CharacterAnimator) -> Self {
        Self {
            idle: animator.idle,
            moving: animator.moving,
            rise: animator.rise,
            fall: animator.fall,
            min_speed: animator.min_speed,
            state: animator.state,
            clip: animator.clip,
            left: animator.left,
        }
    }
}

impl CharacterAnimator {
    /// An animator that shows `idle` in every state, moving from 10 world
    /// pixels per second, until the builders give the others.
    #[must_use]
    pub fn new(idle: impl Into<Clips>) -> Self {
        Self {
            idle: idle.into(),
            moving: None,
            rise: None,
            fall: None,
            min_speed: DEFAULT_MIN_SPEED,
            state: State::Idle,
            clip: None,
            left: false,
        }
    }

    /// The animator, showing `clips` while the entity moves.
    #[must_use]
    pub fn with_move(mut self, clips: impl Into<Clips>) -> Self {
        self.moving = Some(clips.into());
        self
    }

    /// The animator, showing `clips` while a platformer rises.
    #[must_use]
    pub fn with_rise(mut self, clips: impl Into<Clips>) -> Self {
        self.rise = Some(clips.into());
        self
    }

    /// The animator, showing `clips` while a platformer falls.
    #[must_use]
    pub fn with_fall(mut self, clips: impl Into<Clips>) -> Self {
        self.fall = Some(clips.into());
        self
    }

    /// The animator, moving from `speed` world pixels per second.
    ///
    /// # Panics
    ///
    /// When `speed` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_min_speed(mut self, speed: f32) -> Self {
        assert!(
            speed.is_finite() && speed >= 0.0,
            "CharacterAnimator::with_min_speed: speed must be finite and >= 0 (got {speed})"
        );
        self.min_speed = speed;
        self
    }

    /// What the entity was doing at its last frame: `"idle"`, `"move"`,
    /// `"rise"` or `"fall"`.
    #[must_use]
    pub fn state(&self) -> &'static str {
        self.state.name()
    }

    /// The clips of `state`: its own, else the idle ones.
    fn clips(&self, state: State) -> &Clips {
        let own = match state {
            State::Idle => None,
            State::Move => self.moving.as_ref(),
            State::Rise => self.rise.as_ref(),
            State::Fall => self.fall.as_ref(),
        };
        own.unwrap_or(&self.idle)
    }
}

/// Registers the `character_animator` scene component and
/// [`CHARACTER_ANIMATOR`] in the present phase, after
/// [`FACING`](crate::FACING) and [`ANIMATION_TICK`](crate::ANIMATION_TICK)
/// when they are registered, so it reads the frame's facing and replaces
/// the frame the tick showed. Its system picks every [`CharacterAnimator`]'s
/// clip once a frame.
#[derive(Debug, Default, Clone, Copy)]
pub struct CharacterAnimatorPlugin;

impl Plugin for CharacterAnimatorPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<CharacterAnimator>(world, "CharacterAnimator");
        let clip = |name: &'static str, doc: &'static str| {
            FieldDescription::new(
                name,
                FieldType::Object(
                    "an animation ID, or { left, right, up, down } of animation IDs by facing",
                ),
                doc,
            )
        };
        describe(
            world,
            ComponentDescription::new(
                CHARACTER_ANIMATOR_COMPONENT,
                "Picks the entity's animation clip each frame: idle, move, rise or fall, by its facing when a clip is a set.",
            )
            .field(clip(
                "idle",
                "what it shows standing; the load checks each ID",
            ))
            .field(
                clip("move", "what it shows moving; `idle` when left out").default("null"),
            )
            .field(
                clip(
                    "rise",
                    "what a platformer shows rising; `idle` when left out",
                )
                .default("null"),
            )
            .field(
                clip(
                    "fall",
                    "what a platformer shows falling; `idle` when left out",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "min_speed",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "world pixels per second from which it moves",
                )
                .default("10.0"),
            )
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .sample(r#"{ "idle": "hero_idle" }"#)
            .roundtrip(round_trip::<CharacterAnimatorDef>),
        );
        world.register_component_with_assets(
            CHARACTER_ANIMATOR_COMPONENT,
            |def: &CharacterAnimatorDef, world: &World| {
                let Some(registry) = world.get_resource::<AnimationRegistry>() else {
                    return Ok(());
                };
                let clips = [
                    ("idle", Some(&def.idle)),
                    ("move", def.moving.as_ref()),
                    ("rise", def.rise.as_ref()),
                    ("fall", def.fall.as_ref()),
                ];
                for (field, clips) in clips {
                    let Some(clips) = clips else {
                        continue;
                    };
                    for (field, id) in clips.ids(field) {
                        if registry.get(id).is_none() {
                            return Err(MissingAsset::new(field, "animation", id));
                        }
                    }
                }
                Ok(())
            },
            |def: CharacterAnimatorDef, _: &SpawnContext<'_>| {
                let mut animator = CharacterAnimator::new(def.idle).with_min_speed(def.min_speed);
                animator.moving = def.moving;
                animator.rise = def.rise;
                animator.fall = def.fall;
                (animator,)
            },
        );
        let mut entities = Vec::new();
        schedule.add(
            Phase::Present.stage(),
            Phase::Present
                .join(system(CHARACTER_ANIMATOR, move |world: &mut World| {
                    character_animator(world, &mut entities);
                }))
                .after_if_present(FACING)
                .after_if_present(ANIMATION_TICK),
        );
    }
}

/// `components.character_animator` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CharacterAnimatorDef {
    idle: Clips,
    #[serde(rename = "move", default)]
    moving: Option<Clips>,
    #[serde(default)]
    rise: Option<Clips>,
    #[serde(default)]
    fall: Option<Clips>,
    #[serde(default = "default_min_speed", deserialize_with = "non_negative")]
    min_speed: f32,
}

/// Picks every animator's clip for its entity's state and facing; on a
/// change, replaces its `AnimationState` and shows the new clip's first
/// frame. `entities` is the frame's scratch list. Without an
/// `AnimationRegistry` it does nothing.
fn character_animator(world: &mut World, entities: &mut Vec<Entity>) {
    let Some(registry) = world.remove_resource::<AnimationRegistry>() else {
        return;
    };
    let step = world.get_resource::<Time>().map_or(0.0, Time::fixed_step);
    entities.clear();
    entities.extend(
        world
            .query::<(Entity, &CharacterAnimator)>()
            .map(|(entity, _)| entity),
    );
    for &entity in entities.iter() {
        let velocity = velocity_of(world, entity, step);
        let airborne = world
            .get::<PlatformerController>(entity)
            .is_some_and(|controller| !controller.grounded());
        let direction = world
            .get::<Facing>(entity)
            .map_or(velocity, Facing::direction);
        let animator = world.get_mut::<CharacterAnimator>(entity).unwrap();
        let state = if airborne {
            // y points down: a rise is a negative velocity.
            if velocity.y < 0.0 {
                State::Rise
            } else {
                State::Fall
            }
        } else if velocity.length() >= animator.min_speed && velocity != Vec2::ZERO {
            State::Move
        } else {
            State::Idle
        };
        animator.state = state;
        let mut left = animator.left;
        let id = animator.clips(state).pick(direction, &mut left).to_owned();
        animator.left = left;
        if animator.clip.as_deref() == Some(id.as_str()) {
            continue;
        }
        animator.clip = Some(id.clone());
        let first = registry
            .get(&id)
            .and_then(|clip| clip.frames.first())
            .map(|frame| frame.sprite);
        world.insert(entity, AnimationState::new(id));
        if let (Some(first), Some(sprite)) = (first, world.get_mut::<Sprite>(entity)) {
            sprite.asset_id = first;
        }
    }
    world.insert_resource(registry);
}

/// An entity's velocity: its `Velocity`, else its last step's move over the
/// step, else nothing.
fn velocity_of(world: &World, entity: Entity, step: f32) -> Vec2 {
    if let Some(velocity) = world.get::<Velocity>(entity) {
        return velocity.0;
    }
    match (
        world.get::<Position>(entity),
        world.get::<PrevPosition>(entity),
    ) {
        (Some(position), Some(prev)) if step > 0.0 => (position.0 - prev.0) / step,
        _ => Vec2::ZERO,
    }
}

impl Inspectable for CharacterAnimator {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let clips = match self.state {
            State::Idle => &self.idle,
            State::Move => self.moving.as_ref().unwrap_or(&self.idle),
            State::Rise => self.rise.as_ref().unwrap_or(&self.idle),
            State::Fall => self.fall.as_ref().unwrap_or(&self.idle),
        };
        vec![
            ("state", self.state.name().to_string()),
            (
                "clip",
                self.clip
                    .clone()
                    .unwrap_or_else(|| format!("none yet ({})", clips.summary())),
            ),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/character.rs"]
mod tests;

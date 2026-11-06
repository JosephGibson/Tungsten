//! The character animator: an animated sprite's clip picked each frame by
//! the entity's motion and facing, its rules over tags and motion, and the
//! one-shots played over them.

use std::f32::consts::{FRAC_PI_2, PI};
use std::fmt;

use glam::Vec2;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use tungsten_core::physics::{BodyKind, Position, PrevPosition, RigidBody, Velocity};
use tungsten_core::{
    AnimationRegistry, AnimationState, Entity, Inspectable, MissingAsset, Plugin, Schedule,
    SpawnContext, Sprite, Time, World, system,
};

use crate::foundation::describe::{
    ARPG, Bound, ComponentDescription, FieldDescription, FieldType, PLATFORMER, RPG, TOP_DOWN,
    describe, describe_shape, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::non_negative;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::tags::{TagFilter, TagFilterDef, Tags, check_declared};
use crate::intent::aim::Aim;
use crate::motion::platformer::PlatformerController;
use crate::present::animation::ANIMATION_TICK;
use crate::present::facing::{FACING, Facing, write_flip};

/// [`CharacterAnimatorPlugin`]'s system, in the present phase.
pub const CHARACTER_ANIMATOR: &str = "character_animator";

/// The scene component the plugin registers.
const CHARACTER_ANIMATOR_COMPONENT: &str = "character_animator";

/// The shape of a `character_animator`'s `rules` entry.
const CLIP_RULE: &str = "clip_rule";

/// The data form of a [`Clips`], as every description of one writes it.
pub(crate) const CLIPS_FORM: &str = "an animation ID; { left, right, up, down } of animation IDs by facing, `right` flipped when `left` is left out; or { aim: [..] }, two or more drawn facing right from straight down to straight up";

/// The default speed from which a character moves, in world pixels per
/// second.
const DEFAULT_MIN_SPEED: f32 = 10.0;

/// Game milliseconds a one-shot treats as already shown, so a clip of whole
/// frames ends on its last frame whatever the sum's rounding.
const ONE_SHOT_EPSILON_MS: f32 = 1e-3;

/// The clip a [`CharacterAnimator`] state, rule or one-shot shows: one
/// animation, a set by the way the entity faces, or aim poses. A set names
/// `right` and may name `left`, `up` and `down`; a set without `up` and
/// `down` shows the last side faced while the entity faces up or down. A
/// set without `left` shows `right` flipped while the entity faces left:
/// the animator writes its `SpriteFlip` while such a set shows, for art
/// drawn facing right (redo 7.9). Aim poses ([`aim`](Self::aim)) are drawn
/// facing right from straight down to straight up, and show the one nearest
/// the elevation of the entity's [`Aim`](crate::Aim) direction, else its
/// facing's, flipped on the left as a `right`-only set is.
///
/// Data form: a manifest animation ID, `"hero_run"`, or
/// `{ "left": "seeker_walk_left", "right": "seeker_walk_right", "up": "seeker_walk_up", "down": "seeker_walk_down" }`,
/// or `{ "right": "hero_run" }` for one set of frames on both sides, or
/// `{ "aim": ["cast_down", "cast_fwd", "cast_up"] }`, two or more.
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
/// let cast: Clips = serde_json::from_str(r#"{ "aim": ["cast_down", "cast_fwd", "cast_up"] }"#).unwrap();
/// assert_eq!(cast, Clips::aim(["cast_down", "cast_fwd", "cast_up"]));
/// assert!(serde_json::from_str::<Clips>(r#"{ "aim": ["cast_fwd"] }"#).is_err(), "two or more");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clips {
    kind: ClipsKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ClipsKind {
    One(String),
    Set(ClipSet),
    /// Poses drawn facing right, from straight down to straight up.
    Aim(Vec<String>),
}

/// A clip set by facing, as data writes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ClipSet {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    left: Option<String>,
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
                left: Some(left.into()),
                right: right.into(),
                up: None,
                down: None,
            }),
        }
    }

    /// One animation for both sides: `right` while the entity faces right,
    /// the same flipped (its `SpriteFlip`) while it faces left, for art
    /// drawn facing right.
    #[must_use]
    pub fn mirrored(right: impl Into<String>) -> Self {
        Self {
            kind: ClipsKind::Set(ClipSet {
                left: None,
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
                left: Some(left.into()),
                right: right.into(),
                up: Some(up.into()),
                down: Some(down.into()),
            }),
        }
    }

    /// Aim poses: `ids` drawn facing right, from straight down to straight
    /// up, evenly spread over the half turn. The one nearest the elevation
    /// of the entity's [`Aim`](crate::Aim) direction, else its facing's,
    /// shows on the side it points to, flipped on the left as a
    /// [`mirrored`](Self::mirrored) set is; a straight-up or straight-down
    /// direction keeps the last side. Any state, rule or one-shot takes it.
    ///
    /// # Panics
    ///
    /// With fewer than two IDs.
    #[must_use]
    #[track_caller]
    pub fn aim(ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let ids: Vec<String> = ids.into_iter().map(Into::into).collect();
        assert!(
            ids.len() >= 2,
            "Clips::aim: two or more animation IDs (got {})",
            ids.len()
        );
        Self {
            kind: ClipsKind::Aim(ids),
        }
    }

    /// Every animation ID it names, with its field under `field` (`move`,
    /// `move.left`, `clip.aim.1`), for a load's check.
    pub(crate) fn ids<'a>(&'a self, field: &'a str) -> Vec<(String, &'a str)> {
        match &self.kind {
            ClipsKind::One(id) => vec![(field.to_string(), id.as_str())],
            ClipsKind::Aim(ids) => ids
                .iter()
                .enumerate()
                .map(|(i, id)| (format!("{field}.aim.{i}"), id.as_str()))
                .collect(),
            ClipsKind::Set(set) => {
                let mut ids = vec![(format!("{field}.right"), set.right.as_str())];
                if let Some(left) = &set.left {
                    ids.insert(0, (format!("{field}.left"), left.as_str()));
                }
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
    /// and the last side faced otherwise; aim poses by `aim`'s elevation, on
    /// its side. `left` is the last side, updated as the entity faces or
    /// aims left or right. With it comes the `SpriteFlip` to write, for a
    /// set without `left` and aim poses alone: flipped while it shows
    /// frames drawn facing right for the left side.
    fn pick(&self, direction: Vec2, aim: Vec2, left: &mut bool) -> (&str, Option<bool>) {
        let set = match &self.kind {
            ClipsKind::One(id) => return (id, None),
            ClipsKind::Aim(ids) => return (pose(ids, aim, left), Some(*left)),
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
        match (vertical, &set.left) {
            (Some(vertical), None) => (vertical, Some(false)),
            (Some(vertical), Some(_)) => (vertical, None),
            (None, Some(side)) if *left => (side, None),
            (None, Some(_)) => (&set.right, None),
            (None, None) => (&set.right, Some(*left)),
        }
    }

    /// The clip in a line, for inspector rows.
    fn summary(&self) -> String {
        match &self.kind {
            ClipsKind::One(id) => id.clone(),
            ClipsKind::Aim(ids) => format!("aim {}", ids.join(" | ")),
            ClipsKind::Set(set) => {
                let left = set
                    .left
                    .clone()
                    .unwrap_or_else(|| format!("{} flipped", set.right));
                match (&set.up, &set.down) {
                    (Some(up), Some(down)) => format!("{left} | {} | {up} | {down}", set.right),
                    (Some(up), None) => format!("{left} | {} | {up} | -", set.right),
                    (None, Some(down)) => format!("{left} | {} | - | {down}", set.right),
                    (None, None) => format!("{left} | {}", set.right),
                }
            }
        }
    }
}

/// The pose of `ids`, drawn facing right from straight down to straight up,
/// nearest `aim`'s elevation; `left` follows `aim`'s side, kept while it
/// points straight up or down.
fn pose<'a>(ids: &'a [String], aim: Vec2, left: &mut bool) -> &'a str {
    if aim.x != 0.0 {
        *left = aim.x < 0.0;
    }
    // y points down: up is a negative y.
    let elevation = (-aim.y).atan2(aim.x.abs());
    let last = ids.len() - 1;
    let at = ((elevation + FRAC_PI_2) / PI * last as f32).round();
    &ids[(at.max(0.0) as usize).min(last)]
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

/// Aim poses as data writes them.
#[derive(Serialize)]
struct AimForm<'a> {
    aim: &'a [String],
}

/// A clip's object form, before its fields are checked: a set's sides, or
/// aim poses alone.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClipsForm {
    #[serde(default)]
    left: Option<String>,
    #[serde(default)]
    right: Option<String>,
    #[serde(default)]
    up: Option<String>,
    #[serde(default)]
    down: Option<String>,
    #[serde(default)]
    aim: Option<Vec<String>>,
}

impl ClipsForm {
    fn into_clips<E: de::Error>(self) -> Result<Clips, E> {
        let Some(aim) = self.aim else {
            let Some(right) = self.right else {
                return Err(E::missing_field("right"));
            };
            return Ok(Clips {
                kind: ClipsKind::Set(ClipSet {
                    left: self.left,
                    right,
                    up: self.up,
                    down: self.down,
                }),
            });
        };
        if self.left.is_some() || self.right.is_some() || self.up.is_some() || self.down.is_some() {
            return Err(E::custom("aim: alone, without left, right, up or down"));
        }
        if aim.len() < 2 {
            return Err(E::custom(format!(
                "aim: two or more animation IDs (got {})",
                aim.len()
            )));
        }
        Ok(Clips {
            kind: ClipsKind::Aim(aim),
        })
    }
}

impl Serialize for Clips {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.kind {
            ClipsKind::One(id) => serializer.serialize_str(id),
            ClipsKind::Set(set) => set.serialize(serializer),
            ClipsKind::Aim(ids) => AimForm { aim: ids }.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Clips {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ClipsVisitor;

        impl<'de> Visitor<'de> for ClipsVisitor {
            type Value = Clips;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(
                    "an animation ID, { left, right, up, down } of animation IDs, or { aim: [..] }",
                )
            }

            fn visit_str<E: de::Error>(self, id: &str) -> Result<Clips, E> {
                Ok(Clips::one(id))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Clips, A::Error> {
                ClipsForm::deserialize(de::value::MapAccessDeserializer::new(map))?.into_clips()
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
    /// The motion names, as data writes them.
    const NAMES: &'static [&'static str] = &["idle", "move", "rise", "fall"];

    fn name(self) -> &'static str {
        match self {
            State::Idle => "idle",
            State::Move => "move",
            State::Rise => "rise",
            State::Fall => "fall",
        }
    }

    fn named(name: &str) -> Option<Self> {
        match name {
            "idle" => Some(State::Idle),
            "move" => Some(State::Move),
            "rise" => Some(State::Rise),
            "fall" => Some(State::Fall),
            _ => None,
        }
    }
}

/// A [`CharacterAnimator`]'s rule: while the entity's tags pass its
/// filter, while its motion is its own, or while both hold, its clip shows
/// over the motion state's. The first rule that passes picks; a one-shot
/// ([`CharacterAnimator::play`]) wins over every rule. A state a game sets
/// is a tag it inserts, `state.charging` by convention, so the rules, the
/// looks and the hit filters read one vocabulary.
///
/// Data form, an entry of a `character_animator`'s `rules`:
/// `{ "when": { "all": ["state.charging"] }, "motion": "idle", "clip": "brute_charge" }`,
/// each with `when` or `motion` or both, `motion` one of `idle`, `move`,
/// `rise` and `fall`, `clip` a [`Clips`]; the load checks the tags and IDs.
///
/// # Examples
///
/// ```
/// use tungsten_kit::{ClipRule, TagFilter};
///
/// let charge = ClipRule::when(TagFilter::new().all(["state.charging"]), "brute_charge").with_motion("idle");
/// let data = serde_json::to_value(&charge).unwrap();
/// assert_eq!(data["motion"], "idle");
/// assert_eq!(serde_json::from_value::<ClipRule>(data).unwrap(), charge);
/// assert!(serde_json::from_str::<ClipRule>(r#"{ "clip": "brute_charge" }"#).is_err(), "when or motion");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ClipRuleFields", into = "ClipRuleFields")]
pub struct ClipRule {
    when: Option<TagFilter>,
    motion: Option<State>,
    clips: Clips,
}

impl ClipRule {
    /// A rule showing `clips` while the entity's tags pass `when`.
    #[must_use]
    pub fn when(when: TagFilter, clips: impl Into<Clips>) -> Self {
        Self {
            when: Some(when),
            motion: None,
            clips: clips.into(),
        }
    }

    /// A rule showing `clips` while the entity's motion is `motion`:
    /// `"idle"`, `"move"`, `"rise"` or `"fall"`.
    ///
    /// # Panics
    ///
    /// When `motion` names none of the four.
    #[must_use]
    #[track_caller]
    pub fn motion(motion: &str, clips: impl Into<Clips>) -> Self {
        Self {
            when: None,
            motion: Some(motion_named("ClipRule::motion", motion)),
            clips: clips.into(),
        }
    }

    /// The rule, also needing the entity's motion to be `motion`.
    ///
    /// # Panics
    ///
    /// When `motion` names none of `idle`, `move`, `rise` and `fall`.
    #[must_use]
    #[track_caller]
    pub fn with_motion(mut self, motion: &str) -> Self {
        self.motion = Some(motion_named("ClipRule::with_motion", motion));
        self
    }

    /// Whether the rule passes an entity with `tags` in `state`.
    fn passes(&self, tags: Option<&Tags>, state: State) -> bool {
        self.motion.is_none_or(|motion| motion == state)
            && self.when.as_ref().is_none_or(|when| when.matches(tags))
    }
}

#[track_caller]
fn motion_named(method: &str, name: &str) -> State {
    State::named(name).unwrap_or_else(|| {
        panic!("{method}: `{name}` is not a motion (`idle`, `move`, `rise` or `fall`)")
    })
}

/// A rule as its own data writes it, before its fields are checked.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClipRuleFields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    when: Option<TagFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    motion: Option<State>,
    clip: Clips,
}

/// The check every rule's form shares: `when`, `motion` or both.
fn rule_needs(when: bool, motion: bool) -> Result<(), String> {
    if when || motion {
        Ok(())
    } else {
        Err("a rule needs `when`, `motion` or both".to_string())
    }
}

impl TryFrom<ClipRuleFields> for ClipRule {
    type Error = String;

    fn try_from(fields: ClipRuleFields) -> Result<Self, Self::Error> {
        rule_needs(fields.when.is_some(), fields.motion.is_some())?;
        Ok(Self {
            when: fields.when,
            motion: fields.motion,
            clips: fields.clip,
        })
    }
}

impl From<ClipRule> for ClipRuleFields {
    fn from(rule: ClipRule) -> Self {
        Self {
            when: rule.when,
            motion: rule.motion,
            clip: rule.clips,
        }
    }
}

/// A rule as a scene writes it: its filter's names kept in the written
/// order, so a load's error names them as written.
#[derive(Serialize, Deserialize)]
#[serde(try_from = "ClipRuleDefFields")]
struct ClipRuleDef {
    when: Option<TagFilterDef>,
    motion: Option<State>,
    clip: Clips,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClipRuleDefFields {
    #[serde(default)]
    when: Option<TagFilterDef>,
    #[serde(default)]
    motion: Option<State>,
    clip: Clips,
}

impl TryFrom<ClipRuleDefFields> for ClipRuleDef {
    type Error = String;

    fn try_from(fields: ClipRuleDefFields) -> Result<Self, Self::Error> {
        rule_needs(fields.when.is_some(), fields.motion.is_some())?;
        Ok(Self {
            when: fields.when,
            motion: fields.motion,
            clip: fields.clip,
        })
    }
}

impl From<ClipRuleDef> for ClipRule {
    fn from(def: ClipRuleDef) -> Self {
        Self {
            when: def.when.map(TagFilter::from),
            motion: def.motion,
            clips: def.clip,
        }
    }
}

/// A clip shown once over the state's: its clips, the game milliseconds it
/// has shown at the entity's animation speed, and whether a pass started it.
#[derive(Debug, Clone, PartialEq)]
struct OneShot {
    clips: Clips,
    shown_ms: f32,
    started: bool,
}

/// Picks its entity's animation clip each frame by what the entity is
/// doing: `rise` or `fall` while a
/// [`PlatformerController`](crate::PlatformerController) is off the ground,
/// by its velocity's sign; `move` at its minimum speed or faster; else
/// `idle`. A platformer standing on a kinematic body with a `Velocity`, its
/// controller's [`ground`](crate::PlatformerController::ground), counts its
/// own velocity, the body's taken off, for its state and its side: a rider
/// standing still on a moving deck idles. A state's clip is one animation,
/// or a set ([`Clips`]) whose entry is picked by the entity's
/// [`Facing`](crate::Facing) by its dominant axis, else by its velocity's
/// side. `move`, `rise` and `fall` fall back to `idle` when not given.
/// Its [`ClipRule`]s pick over the motion state's clip by the entity's tags
/// and motion, the first that passes. A one-shot
/// ([`play`](Self::play)), such as a landing, a hurt or an attack a cue's
/// `clip` action or an ability's `clip` plays, shows once over both.
///
/// [`CharacterAnimatorPlugin`]'s system runs after the animation tick: on a
/// change of clip it replaces the entity's `AnimationState` and writes the
/// new clip's first frame into `Sprite.asset_id`, so the frame that picks a
/// clip shows its first frame however short, and the clip advances from
/// the next frame. It needs an `AnimationRegistry` and a `Sprite`, and works
/// on children.
///
/// Scene name `character_animator`:
/// `{ "idle": "hero_idle", "move": "hero_run", "rise": "hero_jump", "fall": "hero_fall", "min_speed": 10, "rules": [{ "when": { "all": ["state.charging"] }, "clip": "hero_charge" }] }`,
/// each clip an animation ID, a set by facing or aim poses, `idle` needed,
/// `min_speed` in world pixels per second, 10 by default, and `rules`
/// none by default; the load checks every ID against the manifest's
/// animations, naming the field (`move.left`), and each rule's tags. It is
/// data: a game's save can hold it, but a one-shot.
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
///     frames: vec![AnimationFrame { sprite: SpriteAssetId::new(first), duration_ms: 100, sound: None }],
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
    /// In order: the first that passes picks over the motion state's clip.
    rules: Vec<ClipRule>,
    /// The state at its last frame.
    state: State,
    /// The animation it showed at its last frame; `None` when the next pass
    /// shows its pick from its first frame.
    clip: Option<String>,
    /// Whether the entity last faced left, for a set without `up` and
    /// `down`.
    left: bool,
    /// The clip shown once over the rest; unsaved presentation.
    one_shot: Option<OneShot>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    rules: Vec<ClipRule>,
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
            rules: fields.rules,
            state: fields.state,
            clip: fields.clip,
            left: fields.left,
            one_shot: None,
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
            rules: animator.rules,
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
            rules: Vec::new(),
            state: State::Idle,
            clip: None,
            left: false,
            one_shot: None,
        }
    }

    /// The animator, with `rule` after its others.
    #[must_use]
    pub fn with_rule(mut self, rule: ClipRule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Shows `clips` once over the state's clip and every rule, from its
    /// first frame, for its length: its frames' durations at the entity's
    /// animation speed, counted by the game clock, so a pause holds it.
    /// Then the state's clip shows from its first frame. A play during one
    /// restarts with the new clip, the same one included. Unsaved: a load
    /// shows the state's clip.
    pub fn play(&mut self, clips: impl Into<Clips>) {
        self.one_shot = Some(OneShot {
            clips: clips.into(),
            shown_ms: 0.0,
            started: false,
        });
        self.clip = None;
    }

    /// Ends the one-shot, if one plays: the state's clip shows from its
    /// first frame at the next pass.
    pub fn stop(&mut self) {
        if self.one_shot.take().is_some() {
            self.clip = None;
        }
    }

    /// The one-shot playing, if any.
    #[must_use]
    pub fn playing(&self) -> Option<&Clips> {
        self.one_shot.as_ref().map(|shot| &shot.clips)
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
            FieldDescription::new(name, FieldType::Object(CLIPS_FORM), doc)
        };
        describe(
            world,
            ComponentDescription::new(
                CHARACTER_ANIMATOR_COMPONENT,
                "Picks the entity's animation clip each frame: idle, move, rise or fall, by its facing when a clip is a set, its rules over that, a one-shot over both.",
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
            .field(
                FieldDescription::new(
                    "rules",
                    FieldType::List(Box::new(FieldType::Shape(CLIP_RULE))),
                    "clips over the motion state's by the entity's tags and motion, the first that passes; the load checks each tag and ID",
                )
                .default("[]"),
            )
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .genre(RPG)
            .genre(ARPG)
            .sample(r#"{ "idle": "hero_idle" }"#)
            .roundtrip(round_trip::<CharacterAnimatorDef>),
        );
        describe_shape(
            world,
            ComponentDescription::new(
                CLIP_RULE,
                "A clip shown over the motion state's while the entity's tags pass `when`, its motion is `motion`, or both.",
            )
            .field(
                FieldDescription::new(
                    "when",
                    FieldType::Filter,
                    "the entity's tags, `state.*` by convention for a state the game sets; the load checks each",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "motion",
                    FieldType::Enum(State::NAMES),
                    "the motion state it needs",
                )
                .default("null"),
            )
            .field(clip("clip", "what it shows; the load checks each ID"))
            .sample(r#"{ "motion": "move", "clip": "hero_run" }"#)
            .roundtrip(round_trip::<ClipRuleDef>),
        );
        world.register_component_with_assets(
            CHARACTER_ANIMATOR_COMPONENT,
            |def: &CharacterAnimatorDef, world: &World| {
                for (i, rule) in def.rules.iter().enumerate() {
                    for (field, name) in rule.when.iter().flat_map(TagFilterDef::names) {
                        check_declared(world, &format!("rules.{i}.when.{field}"), name)?;
                    }
                }
                let clips = [
                    ("idle".to_string(), Some(&def.idle)),
                    ("move".to_string(), def.moving.as_ref()),
                    ("rise".to_string(), def.rise.as_ref()),
                    ("fall".to_string(), def.fall.as_ref()),
                ]
                .into_iter()
                .chain(
                    def.rules
                        .iter()
                        .enumerate()
                        .map(|(i, rule)| (format!("rules.{i}.clip"), Some(&rule.clip))),
                );
                for (field, clips) in clips {
                    let Some(clips) = clips else {
                        continue;
                    };
                    check_clips(world, &field, clips)?;
                }
                Ok(())
            },
            |def: CharacterAnimatorDef, _: &SpawnContext<'_>| {
                let mut animator = CharacterAnimator::new(def.idle).with_min_speed(def.min_speed);
                animator.moving = def.moving;
                animator.rise = def.rise;
                animator.fall = def.fall;
                animator.rules = def.rules.into_iter().map(ClipRule::from).collect();
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
    #[serde(default)]
    rules: Vec<ClipRuleDef>,
}

/// The load's check of `clips` at `field`: each animation ID against the
/// manifest's animations, when the World has them.
pub(crate) fn check_clips(world: &World, field: &str, clips: &Clips) -> Result<(), MissingAsset> {
    let Some(registry) = world.get_resource::<AnimationRegistry>() else {
        return Ok(());
    };
    for (field, id) in clips.ids(field) {
        if registry.get(id).is_none() {
            return Err(MissingAsset::new(field, "animation", id));
        }
    }
    Ok(())
}

/// The game milliseconds clip `id` lasts at speed 1: its frames' durations
/// summed, 0 for an ID the registry lacks.
fn clip_ms(registry: &AnimationRegistry, id: &str) -> f32 {
    registry.get(id).map_or(0.0, |clip| {
        clip.frames
            .iter()
            .map(|frame| frame.duration_ms as f32)
            .sum()
    })
}

/// Picks every animator's clip for its entity's state and facing, a rule's
/// over it and a one-shot's over both; on a change, replaces its
/// `AnimationState` and shows the new clip's first frame. `entities` is the
/// frame's scratch list. Without an `AnimationRegistry` it does nothing.
fn character_animator(world: &mut World, entities: &mut Vec<Entity>) {
    let Some(registry) = world.remove_resource::<AnimationRegistry>() else {
        return;
    };
    let (step, delta) = world
        .get_resource::<Time>()
        .map_or((0.0, 0.0), |time| (time.fixed_step(), time.delta()));
    entities.clear();
    entities.extend(
        world
            .query::<(Entity, &CharacterAnimator)>()
            .map(|(entity, _)| entity),
    );
    for &entity in entities.iter() {
        let controller = world.get::<PlatformerController>(entity);
        let airborne = controller.is_some_and(|controller| !controller.grounded());
        let floor = controller
            .filter(|controller| controller.grounded())
            .and_then(PlatformerController::ground);
        let velocity = velocity_of(world, entity, step)
            - floor.map_or(Vec2::ZERO, |floor| carry(world, floor));
        let direction = world
            .get::<Facing>(entity)
            .map_or(velocity, Facing::direction);
        let aim = world
            .get::<Aim>(entity)
            .and_then(Aim::direction)
            .unwrap_or(direction);
        let speed = world
            .get::<AnimationState>(entity)
            .map_or(1.0, AnimationState::speed);
        let (state, rule) = {
            let animator = world.get::<CharacterAnimator>(entity).unwrap();
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
            let rule = if animator.rules.is_empty() {
                None
            } else {
                let tags = world.get::<Tags>(entity);
                animator
                    .rules
                    .iter()
                    .position(|rule| rule.passes(tags, state))
            };
            (state, rule)
        };
        let animator = world.get_mut::<CharacterAnimator>(entity).unwrap();
        animator.state = state;
        let mut left = animator.left;
        // A one-shot first: once its length has shown, the state's clip,
        // or a rule's, shows from its first frame.
        let mut shot_pick = None;
        let mut ended = false;
        if let Some(shot) = animator.one_shot.as_mut() {
            let (id, flip) = shot.clips.pick(direction, aim, &mut left);
            let length = clip_ms(&registry, id);
            let id = id.to_owned();
            if shot.started {
                shot.shown_ms += delta.max(0.0) * 1000.0 * speed;
                ended = shot.shown_ms + ONE_SHOT_EPSILON_MS >= length;
            }
            shot.started = true;
            if !ended {
                shot_pick = Some((id, flip));
            }
        }
        if ended {
            animator.one_shot = None;
            animator.clip = None;
        }
        let (id, flip) = shot_pick.unwrap_or_else(|| {
            let clips = rule.map_or_else(|| animator.clips(state), |i| &animator.rules[i].clips);
            let (id, flip) = clips.pick(direction, aim, &mut left);
            (id.to_owned(), flip)
        });
        animator.left = left;
        let unchanged = animator.clip.as_deref() == Some(id.as_str());
        if !unchanged {
            animator.clip = Some(id.clone());
        }
        // A set without `left` turns the same frames by the sprite's flip,
        // which changes with no change of clip.
        if let Some(flip) = flip {
            write_flip(world, entity, flip);
        }
        if unchanged {
            continue;
        }
        let first = registry
            .get(&id)
            .and_then(|clip| clip.frames.first())
            .map(|frame| frame.sprite);
        // Redo 7.9: the entity's speed carries across its clip changes.
        world.insert(entity, AnimationState::new(id).with_speed(speed));
        if let (Some(first), Some(sprite)) = (first, world.get_mut::<Sprite>(entity)) {
            sprite.asset_id = first;
        }
    }
    world.insert_resource(registry);
}

/// The velocity `floor` carries what stands on it at: a kinematic body's
/// `Velocity`, else nothing.
fn carry(world: &World, floor: Entity) -> Vec2 {
    let kinematic = world
        .get::<RigidBody>(floor)
        .is_some_and(|body| body.kind == BodyKind::Kinematic);
    match world.get::<Velocity>(floor) {
        Some(velocity) if kinematic => velocity.0,
        _ => Vec2::ZERO,
    }
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

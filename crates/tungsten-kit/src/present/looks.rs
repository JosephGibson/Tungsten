//! Status looks (redo 7.10p): what an effect's instance shows on its holder
//! while it runs, drawn as plain entities the default extract draws: a
//! tint on the holder's sprite, an overlay clip riding it and a glow under
//! it. The kit adds no render type of its own (`D-007`, `D-016`). Tag looks
//! (redo 7.10g): the same parts while the holder's tags pass a filter.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::components::SpriteFlip;
use tungsten_core::{
    AnimationRegistry, AnimationState, AssetRegistry, Entity, Inspectable, MaterialRegistry,
    MissingAsset, ParticleConfigRegistry, ParticleEmitter, ParticleEmitterState, Plugin, Schedule,
    SoundRegistry, SpawnContext, Sprite, SpriteAsset, Time, Transform, Visibility, World,
    sprite_point,
};

use crate::damage::effects::ensure_looks;
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, PLATFORMER, SHOOTER, TOP_DOWN,
    describe, describe_shape, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::ensure_phases;
use crate::foundation::tags::{TagFilter, TagFilterDef, Tags, check_declared};
use crate::foundation::{finite_color, non_negative, positive};
use crate::lifecycle::ended::Despawning;
use crate::present::facing::Facing;
use crate::present::health_bar::Bars;

/// [`EffectsPlugin`](crate::EffectsPlugin)'s early look pass, in
/// `post_update` after the transform propagation and before the particle
/// count: it keeps each look's parts, spawning and despawning them, and
/// puts a tinted holder's own colour back.
pub const EFFECT_LOOK: &str = "effect_look";

/// [`EffectsPlugin`](crate::EffectsPlugin)'s late look pass, in the present
/// phase after `facing`, `character_animator` and `depth_sort`: it tints
/// each holder and places its overlays and glows at its current frame,
/// flip, turn and depth.
pub const EFFECT_LOOK_LATE: &str = "effect_look_late";

/// The reserved sprite and material a glow draws through.
const ENGINE_WHITE: &str = "engine_white";
const ENGINE_GLOW: &str = "engine_glow";

/// What a status shows on its holder while an instance runs, by its share:
/// a status's intensity over its max, another effect's stacks over its
/// limit. Each part is optional:
///
/// - `tint`, `[r, g, b, a]`: the holder's drawn `Sprite.color`, the colour
///   the frame's writers left it, taken toward `tint` by the share times
///   `a`; several multiply in instance order. The colour comes back as the
///   frame's writers leave it when the look ends.
/// - `overlay`, `{ "animation", "sprite", "speed", "offset", "z", "material", "scale", "bob", "flip" }`:
///   an entity playing the clip, looped, or showing the plain `sprite`
///   instead, exactly one of the two, its frame's drawn centre at the
///   holder's drawn centre plus `offset` whatever the frame's anchor,
///   mirrored and turned with it, `z` above the holder's, at
///   `speed`, from a phase drawn from the holder's index, through
///   `material` when set, scaled from `scale`'s first to its second by the
///   share, `[1, 1]` by default; `bob`, `{ "height", "seconds" }`, raises
///   it `height · |sin(π·t / seconds)|` px at `t` game seconds from its
///   show, and `flip: false` keeps its frame unflipped while its offset
///   still mirrors, so a glyph reads alike either way.
/// - `glow`, `{ "color", "radius" }`: `engine_white` through `engine_glow`,
///   `radius` px about the drawn centre, one below the holder, its alpha by
///   the share.
/// - `particles`, `{ "config", "at", "offset" }`: an emitter of the particle
///   config at the holder's named sprite point `at`, else its drawn centre,
///   plus `offset` mirrored and turned as an overlay's is, moved there each
///   frame, so it follows the frame, the flip and the turn, its
///   `rate_scale` the share; its live particles age out after it.
/// - `light`, `{ "color", "radius", "intensity" }`: a point `Light` at the
///   holder's point, its `Position`, else its `Transform`, its intensity
///   times the share, kept on the fixed step, so chemistry's light reads it
///   alike at any frame rate.
/// - `sound`, `{ "id", "volume" }`: the instance wants the looping sound at
///   its volume times the share by the holder's place, sharing its one
///   voice with any other look's or cue `loop`'s, the largest want heard.
///
/// An [`Effect`](crate::Effect)'s `look`, or a band's, which replaces the
/// effect's while that band is the highest its intensity holds. The parts
/// are unsaved: a load brings them back from the restored instances.
///
/// # Examples
///
/// ```
/// use tungsten_kit::{Look, Overlay};
///
/// let frost = Look::new()
///     .with_tint([0.5, 0.75, 1.0, 1.0])
///     .with_overlay(Overlay::new("frost_rime").with_z(2))
///     .with_glow([0.5, 0.75, 1.0, 0.5], 24.0);
/// let data = serde_json::to_value(&frost).unwrap();
/// assert_eq!(data["overlay"]["animation"], "frost_rime");
/// assert_eq!(serde_json::from_value::<Look>(data).unwrap(), frost);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "LookDef", into = "LookDef")]
pub struct Look {
    tint: Option<[f32; 4]>,
    overlay: Option<Overlay>,
    glow: Option<Glow>,
    particles: Option<LookParticles>,
    pub(crate) light: Option<LookLight>,
    pub(crate) sound: Option<LookSound>,
}

/// A look's particles: the config, the named point they emit at and their
/// offset from it.
#[derive(Debug, Clone, PartialEq)]
struct LookParticles {
    config: String,
    at: Option<String>,
    offset: Vec2,
}

/// A look's light: a point light's colour, its alpha scaling the
/// intensity, its radius and intensity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LookLight {
    pub(crate) color: [f32; 4],
    pub(crate) radius: f32,
    pub(crate) intensity: f32,
}

/// A look's sound: the looping sound it wants and its volume.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LookSound {
    pub(crate) id: String,
    pub(crate) volume: f32,
}

/// A look's overlay: a clip played on an entity riding the holder, or a
/// plain sprite shown there. Build one with [`Overlay::new`] or
/// [`Overlay::sprite`].
#[derive(Debug, Clone, PartialEq)]
pub struct Overlay {
    art: Art,
    speed: f32,
    offset: Vec2,
    z: i32,
    material: Option<String>,
    /// Its scale at a share of 0 and of 1.
    scale: [f32; 2],
    bob: Option<Bob>,
    /// Whether its frame mirrors with the holder.
    flip: bool,
}

/// What an overlay shows.
#[derive(Debug, Clone, PartialEq)]
enum Art {
    /// A manifest animation, looped.
    Clip(String),
    /// A manifest sprite, still.
    Sprite(String),
}

/// An overlay's bob: the world pixels it rises at its top and the game
/// seconds from one rest to the next.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Bob {
    height: f32,
    seconds: f32,
}

impl Bob {
    /// Its rise `shown` game seconds from its show.
    fn rise(self, shown: f32) -> f32 {
        self.height * (std::f32::consts::PI * shown / self.seconds).sin().abs()
    }
}

/// A look's glow: its colour and radius.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Glow {
    color: [f32; 4],
    radius: f32,
}

impl Look {
    /// A look that shows nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The look, tinting its holder toward `color`, linear, its alpha the
    /// weight.
    ///
    /// # Panics
    ///
    /// When a channel is not finite.
    #[must_use]
    #[track_caller]
    pub fn with_tint(mut self, color: [f32; 4]) -> Self {
        assert!(
            color.iter().all(|channel| channel.is_finite()),
            "Look::with_tint: color must be finite (got {color:?})"
        );
        self.tint = Some(color);
        self
    }

    /// The look, with `overlay` riding its holder.
    #[must_use]
    pub fn with_overlay(mut self, overlay: Overlay) -> Self {
        self.overlay = Some(overlay);
        self
    }

    /// The look, glowing `color` within `radius` px under its holder.
    ///
    /// # Panics
    ///
    /// When a channel is not finite, or `radius` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_glow(mut self, color: [f32; 4], radius: f32) -> Self {
        assert!(
            color.iter().all(|channel| channel.is_finite()),
            "Look::with_glow: color must be finite (got {color:?})"
        );
        assert!(
            radius.is_finite() && radius > 0.0,
            "Look::with_glow: radius must be finite and > 0 (got {radius})"
        );
        self.glow = Some(Glow { color, radius });
        self
    }

    /// The look, emitting particle config `config` from its holder's
    /// drawn centre, its rate by the share.
    #[must_use]
    pub fn with_particles(mut self, config: impl Into<String>) -> Self {
        self.particles = Some(LookParticles {
            config: config.into(),
            at: None,
            offset: Vec2::ZERO,
        });
        self
    }

    /// The look, emitting particle config `config` from its holder's named
    /// sprite point `point` ([`sprite_point`]), its drawn centre on a frame
    /// without it.
    #[must_use]
    pub fn with_particles_at(
        mut self,
        config: impl Into<String>,
        point: impl Into<String>,
    ) -> Self {
        self.particles = Some(LookParticles {
            config: config.into(),
            at: Some(point.into()),
            offset: Vec2::ZERO,
        });
        self
    }

    /// The look, its particles `offset` px from their point, mirrored and
    /// turned as an overlay's offset is.
    ///
    /// # Panics
    ///
    /// When the look has no particles, or a coordinate is not finite.
    #[must_use]
    #[track_caller]
    pub fn with_particles_offset(mut self, offset: Vec2) -> Self {
        assert!(
            offset.is_finite(),
            "Look::with_particles_offset: offset must be finite (got {offset})"
        );
        let Some(particles) = self.particles.as_mut() else {
            panic!("Look::with_particles_offset: the look has no particles");
        };
        particles.offset = offset;
        self
    }

    /// The look, lighting its holder: a point light of linear `color`, its
    /// alpha scaling the intensity, `radius` px, at `intensity` times the
    /// share.
    ///
    /// # Panics
    ///
    /// When a channel is not finite, `radius` is not finite and above 0,
    /// or `intensity` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_light(mut self, color: [f32; 4], radius: f32, intensity: f32) -> Self {
        assert!(
            color.iter().all(|channel| channel.is_finite())
                && radius.is_finite()
                && radius > 0.0
                && intensity.is_finite()
                && intensity >= 0.0,
            "Look::with_light: needs a finite colour, a radius above 0 and an intensity of at least 0"
        );
        self.light = Some(LookLight {
            color,
            radius,
            intensity,
        });
        self
    }

    /// The look, looping manifest sound `id` at `volume` times the share
    /// while it runs.
    ///
    /// # Panics
    ///
    /// When `volume` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_sound(mut self, id: impl Into<String>, volume: f32) -> Self {
        assert!(
            volume.is_finite() && volume >= 0.0,
            "Look::with_sound: volume must be finite and >= 0 (got {volume})"
        );
        self.sound = Some(LookSound {
            id: id.into(),
            volume,
        });
        self
    }

    /// Its tint, if any.
    #[must_use]
    pub fn tint(&self) -> Option<[f32; 4]> {
        self.tint
    }

    /// Whether it shows anything the frame passes keep entities for.
    fn has_parts(&self) -> bool {
        self.overlay.is_some() || self.glow.is_some() || self.particles.is_some()
    }

    /// Checks the IDs it names against the World's registries: the
    /// overlay's animation and material. `field` names the look.
    pub(crate) fn check(&self, world: &World, field: &str) -> Result<(), MissingAsset> {
        if let Some(overlay) = &self.overlay {
            match &overlay.art {
                Art::Clip(animation) => {
                    if let Some(animations) = world.get_resource::<AnimationRegistry>()
                        && animations.get(animation).is_none()
                    {
                        return Err(MissingAsset::new(
                            format!("{field}.overlay.animation"),
                            "animation",
                            animation.as_str(),
                        ));
                    }
                }
                Art::Sprite(sprite) => {
                    if let Some(assets) = world.get_resource::<AssetRegistry>()
                        && assets.sprite_id(sprite).is_none()
                    {
                        return Err(MissingAsset::new(
                            format!("{field}.overlay.sprite"),
                            "sprite",
                            sprite.as_str(),
                        ));
                    }
                }
            }
            if let (Some(material), Some(materials)) =
                (&overlay.material, world.get_resource::<MaterialRegistry>())
                && materials.get(material).is_none()
            {
                return Err(MissingAsset::new(
                    format!("{field}.overlay.material"),
                    "material",
                    material.as_str(),
                ));
            }
        }
        if let Some(particles) = &self.particles
            && let Some(configs) = world.get_resource::<ParticleConfigRegistry>()
            && configs.id_for_name(&particles.config).is_none()
        {
            return Err(MissingAsset::new(
                format!("{field}.particles.config"),
                "particle config",
                particles.config.as_str(),
            ));
        }
        if let Some(sound) = &self.sound
            && let Some(sounds) = world.get_resource::<SoundRegistry>()
            && sounds.get_by_id(&sound.id).is_none()
        {
            return Err(MissingAsset::new(
                format!("{field}.sound.id"),
                "sound",
                sound.id.as_str(),
            ));
        }
        Ok(())
    }
}

impl Overlay {
    /// An overlay playing clip `animation` at speed 1, on the holder's
    /// drawn centre, one above it in z, through the clip's own material.
    #[must_use]
    pub fn new(animation: impl Into<String>) -> Self {
        Self::of(Art::Clip(animation.into()))
    }

    /// An overlay showing manifest sprite `sprite`, still, on the holder's
    /// drawn centre, one above it in z, through the sprite's own material:
    /// a mark or a glyph a clip of one frame would hold.
    #[must_use]
    pub fn sprite(sprite: impl Into<String>) -> Self {
        Self::of(Art::Sprite(sprite.into()))
    }

    fn of(art: Art) -> Self {
        Self {
            art,
            speed: 1.0,
            offset: Vec2::ZERO,
            z: 1,
            material: None,
            scale: [1.0, 1.0],
            bob: None,
            flip: true,
        }
    }

    /// The overlay, rising `height` px and back every `seconds` of game
    /// time from its show: `height · |sin(π·t / seconds)|` at `t`.
    ///
    /// # Panics
    ///
    /// When `height` is not finite and at least 0, or `seconds` is not
    /// finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_bob(mut self, height: f32, seconds: f32) -> Self {
        assert!(
            height.is_finite() && height >= 0.0 && seconds.is_finite() && seconds > 0.0,
            "Overlay::with_bob: needs a finite height >= 0 and seconds > 0 (got {height}, {seconds})"
        );
        self.bob = Some(Bob { height, seconds });
        self
    }

    /// The overlay, its frame never mirrored while its offset still mirrors
    /// with the holder: a glyph that reads alike either way.
    #[must_use]
    pub fn unflipped(mut self) -> Self {
        self.flip = false;
        self
    }

    /// Its offset from the holder's drawn centre `shown` game seconds from
    /// its show, its bob's rise taken off `y`.
    fn offset_at(&self, shown: f32) -> Vec2 {
        match self.bob {
            Some(bob) => self.offset - Vec2::new(0.0, bob.rise(shown)),
            None => self.offset,
        }
    }

    /// The overlay, scaled from `from` at a share of 0 to `to` at 1: a
    /// ring that grows over a tell, a status that swells with its
    /// intensity.
    ///
    /// # Panics
    ///
    /// When either is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_scale(mut self, from: f32, to: f32) -> Self {
        assert!(
            valid_scale([from, to]),
            "Overlay::with_scale: each must be finite and >= 0 (got {from}, {to})"
        );
        self.scale = [from, to];
        self
    }

    /// Its scale at `share`.
    fn scale_at(&self, share: f32) -> f32 {
        self.scale[0] + (self.scale[1] - self.scale[0]) * share.clamp(0.0, 1.0)
    }

    /// The overlay, playing at `speed`.
    ///
    /// # Panics
    ///
    /// When `speed` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_speed(mut self, speed: f32) -> Self {
        assert!(
            speed.is_finite() && speed >= 0.0,
            "Overlay::with_speed: speed must be finite and >= 0 (got {speed})"
        );
        self.speed = speed;
        self
    }

    /// The overlay, `offset` px from the holder's drawn centre, mirrored
    /// and turned with it.
    #[must_use]
    pub fn with_offset(mut self, offset: Vec2) -> Self {
        self.offset = offset;
        self
    }

    /// The overlay, `z` above its holder's depth.
    #[must_use]
    pub fn with_z(mut self, z: i32) -> Self {
        self.z = z;
        self
    }

    /// The overlay, drawn through manifest material `material`.
    #[must_use]
    pub fn with_material(mut self, material: impl Into<String>) -> Self {
        self.material = Some(material.into());
        self
    }
}

/// A look as data writes it, every part optional.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LookDef {
    #[serde(
        default,
        deserialize_with = "optional_color",
        skip_serializing_if = "Option::is_none"
    )]
    tint: Option<[f32; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    overlay: Option<OverlayDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    glow: Option<GlowDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    particles: Option<ParticlesDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    light: Option<LightDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sound: Option<LookSoundDef>,
}

/// A look's particles as data writes them.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ParticlesDef {
    config: String,
    #[serde(default)]
    at: Option<String>,
    #[serde(default)]
    offset: [f32; 2],
}

/// A look's light as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "LightFields")]
pub(crate) struct LightDef {
    color: [f32; 4],
    radius: f32,
    intensity: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LightFields {
    color: [f32; 4],
    #[serde(deserialize_with = "positive")]
    radius: f32,
    #[serde(default = "one", deserialize_with = "non_negative")]
    intensity: f32,
}

impl TryFrom<LightFields> for LightDef {
    type Error = String;

    fn try_from(fields: LightFields) -> Result<Self, Self::Error> {
        if fields.color.iter().any(|channel| !channel.is_finite()) {
            return Err("color: a colour's channels must be finite".into());
        }
        Ok(Self {
            color: fields.color,
            radius: fields.radius,
            intensity: fields.intensity,
        })
    }
}

/// A look's sound as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LookSoundDef {
    id: String,
    #[serde(default = "one", deserialize_with = "non_negative")]
    volume: f32,
}

/// An overlay as data writes it, exactly one of `animation` and `sprite`.
#[derive(Serialize, Deserialize)]
#[serde(try_from = "OverlayFields")]
pub(crate) struct OverlayDef {
    #[serde(skip_serializing_if = "Option::is_none")]
    animation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sprite: Option<String>,
    speed: f32,
    offset: [f32; 2],
    z: i32,
    material: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scale: Option<[f32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bob: Option<BobDef>,
    flip: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OverlayFields {
    #[serde(default)]
    animation: Option<String>,
    #[serde(default)]
    sprite: Option<String>,
    #[serde(default = "one", deserialize_with = "non_negative")]
    speed: f32,
    #[serde(default)]
    offset: [f32; 2],
    #[serde(default = "above")]
    z: i32,
    #[serde(default)]
    material: Option<String>,
    #[serde(default)]
    scale: Option<[f32; 2]>,
    #[serde(default)]
    bob: Option<BobDef>,
    #[serde(default = "flipped")]
    flip: bool,
}

impl TryFrom<OverlayFields> for OverlayDef {
    type Error = String;

    fn try_from(fields: OverlayFields) -> Result<Self, Self::Error> {
        if fields.animation.is_some() == fields.sprite.is_some() {
            return Err("an overlay needs exactly one of `animation` and `sprite`".into());
        }
        Ok(Self {
            animation: fields.animation,
            sprite: fields.sprite,
            speed: fields.speed,
            offset: fields.offset,
            z: fields.z,
            material: fields.material,
            scale: fields.scale,
            bob: fields.bob,
            flip: fields.flip,
        })
    }
}

/// An overlay's bob as data writes it.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BobDef {
    #[serde(deserialize_with = "non_negative")]
    height: f32,
    #[serde(deserialize_with = "positive")]
    seconds: f32,
}

/// Whether an overlay's scale is two finite values of at least 0.
fn valid_scale(scale: [f32; 2]) -> bool {
    scale.iter().all(|value| value.is_finite() && *value >= 0.0)
}

/// A glow as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GlowDef {
    #[serde(deserialize_with = "finite_color")]
    color: [f32; 4],
    #[serde(deserialize_with = "positive")]
    radius: f32,
}

fn one() -> f32 {
    1.0
}

fn above() -> i32 {
    1
}

fn flipped() -> bool {
    true
}

/// An optional colour, each channel finite.
fn optional_color<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<[f32; 4]>, D::Error> {
    let color = Option::<[f32; 4]>::deserialize(deserializer)?;
    if color.is_some_and(|color| color.iter().any(|channel| !channel.is_finite())) {
        return Err(serde::de::Error::custom(
            "a colour's channels must be finite",
        ));
    }
    Ok(color)
}

impl TryFrom<LookDef> for Look {
    type Error = String;

    fn try_from(def: LookDef) -> Result<Self, Self::Error> {
        if let Some(scale) = def.overlay.as_ref().and_then(|overlay| overlay.scale)
            && !valid_scale(scale)
        {
            return Err(format!(
                "overlay.scale: each must be finite and >= 0 (got {scale:?})"
            ));
        }
        Ok(Self {
            tint: def.tint,
            overlay: def.overlay.map(Overlay::try_from).transpose()?,
            glow: def.glow.map(|glow| Glow {
                color: glow.color,
                radius: glow.radius,
            }),
            particles: def.particles.map(|particles| LookParticles {
                config: particles.config,
                at: particles.at,
                offset: Vec2::from(particles.offset),
            }),
            light: def.light.map(|light| LookLight {
                color: light.color,
                radius: light.radius,
                intensity: light.intensity,
            }),
            sound: def.sound.map(|sound| LookSound {
                id: sound.id,
                volume: sound.volume,
            }),
        })
    }
}

impl TryFrom<OverlayDef> for Overlay {
    type Error = String;

    fn try_from(def: OverlayDef) -> Result<Self, Self::Error> {
        let art = match (def.animation, def.sprite) {
            (Some(animation), None) => Art::Clip(animation),
            (None, Some(sprite)) => Art::Sprite(sprite),
            _ => return Err("an overlay needs exactly one of `animation` and `sprite`".into()),
        };
        Ok(Self {
            art,
            speed: def.speed,
            offset: Vec2::from(def.offset),
            z: def.z,
            material: def.material,
            scale: def.scale.unwrap_or([1.0, 1.0]),
            bob: def.bob.map(|bob| Bob {
                height: bob.height,
                seconds: bob.seconds,
            }),
            flip: def.flip,
        })
    }
}

impl From<Look> for LookDef {
    fn from(look: Look) -> Self {
        Self {
            tint: look.tint,
            overlay: look.overlay.map(|overlay| {
                let (animation, sprite) = match overlay.art {
                    Art::Clip(animation) => (Some(animation), None),
                    Art::Sprite(sprite) => (None, Some(sprite)),
                };
                OverlayDef {
                    animation,
                    sprite,
                    speed: overlay.speed,
                    offset: overlay.offset.into(),
                    z: overlay.z,
                    material: overlay.material,
                    scale: (overlay.scale != [1.0, 1.0]).then_some(overlay.scale),
                    bob: overlay.bob.map(|bob| BobDef {
                        height: bob.height,
                        seconds: bob.seconds,
                    }),
                    flip: overlay.flip,
                }
            }),
            glow: look.glow.map(|glow| GlowDef {
                color: glow.color,
                radius: glow.radius,
            }),
            particles: look.particles.map(|particles| ParticlesDef {
                config: particles.config,
                at: particles.at,
                offset: particles.offset.into(),
            }),
            light: look.light.map(|light| LightDef {
                color: light.color,
                radius: light.radius,
                intensity: light.intensity,
            }),
            sound: look.sound.map(|sound| LookSoundDef {
                id: sound.id,
                volume: sound.volume,
            }),
        }
    }
}

/// The shapes a look's data form names.
pub(crate) fn describe_looks(world: &mut World) {
    let shapes = [
        ComponentDescription::new(
            "look",
            "What a status shows on its holder while it runs, by its share: a tint, an overlay clip and a glow, each optional; unsaved.",
        )
        .field(
            FieldDescription::new(
                "tint",
                FieldType::Color,
                "linear: the holder's sprite colour taken toward it by the share times its alpha; several multiply",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "overlay",
                FieldType::Shape("look_overlay"),
                "a clip played, or a sprite shown, on an entity riding the holder",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "glow",
                FieldType::Shape("look_glow"),
                "`engine_white` through `engine_glow` under the holder, its alpha by the share",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "particles",
                FieldType::Shape("look_particles"),
                "an emitter at the holder's named sprite point, following its frame, flip and turn, its rate by the share",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "light",
                FieldType::Shape("look_light"),
                "a point light at the holder, its intensity by the share, kept on the fixed step",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "sound",
                FieldType::Shape("look_sound"),
                "a looping sound the instance wants by the holder's place, its volume by the share",
            )
            .default("null"),
        )
        .sample("{}")
        .roundtrip(round_trip::<LookDef>),
        ComponentDescription::new(
            "look_overlay",
            "A clip looped, or a sprite shown still, on an entity, its frame's drawn centre at the holder's drawn centre plus its offset whatever the frame's anchor, mirrored and turned with it.",
        )
        .field(FieldDescription::new(
            "animation",
            FieldType::Text,
            "the manifest animation it loops, needed but where `sprite` stands in its place; the load checks it",
        ))
        .field(
            FieldDescription::new(
                "sprite",
                FieldType::Text,
                "a manifest sprite it shows still in place of `animation`, never beside it; the load checks it",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "speed",
                FieldType::F32 {
                    min: Bound::Inclusive(0.0),
                    max: Bound::Open,
                },
                "its playback rate",
            )
            .default("1.0"),
        )
        .field(
            FieldDescription::new(
                "offset",
                FieldType::Vec2,
                "world pixels from the holder's drawn centre, mirrored and turned with it",
            )
            .default("[0.0, 0.0]"),
        )
        .field(
            FieldDescription::new(
                "z",
                FieldType::I64 {
                    min: Some(i64::from(i32::MIN)),
                    max: Some(i64::from(i32::MAX)),
                },
                "its depth above the holder's",
            )
            .default("1"),
        )
        .field(
            FieldDescription::new(
                "material",
                FieldType::Text,
                "a manifest material it draws through; the load checks it",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "scale",
                FieldType::Vec2,
                "`[from, to]`, each at least 0: its scale from `from` at a share of 0 to `to` at 1, a ring growing over a tell; `[1, 1]` when left out",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "bob",
                FieldType::Shape("look_bob"),
                "a rise and fall over its offset, counted on the game clock from its show",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "flip",
                FieldType::Bool,
                "its frame mirrored with the holder; false keeps it unflipped while its offset still mirrors, so a glyph reads alike either way",
            )
            .default("true"),
        )
        .sample(r#"{ "animation": "frost" }"#)
        .roundtrip(round_trip::<OverlayDef>),
        ComponentDescription::new(
            "look_bob",
            "An overlay's rise of `height · |sin(π·t / seconds)|` world pixels at `t` game seconds from its show.",
        )
        .field(FieldDescription::new(
            "height",
            FieldType::F32 {
                min: Bound::Inclusive(0.0),
                max: Bound::Open,
            },
            "world pixels it rises at its top",
        ))
        .field(FieldDescription::new(
            "seconds",
            FieldType::F32 {
                min: Bound::Exclusive(0.0),
                max: Bound::Open,
            },
            "game seconds from one rest to the next",
        ))
        .sample(r#"{ "height": 2, "seconds": 0.5 }"#)
        .roundtrip(round_trip::<BobDef>),
        ComponentDescription::new("look_glow", "A soft glow under the holder.")
            .field(FieldDescription::new(
                "color",
                FieldType::Color,
                "linear; its alpha by the share",
            ))
            .field(FieldDescription::new(
                "radius",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "world pixels about the holder's drawn centre",
            ))
            .sample(r#"{ "color": [1.0, 0.5, 0.0, 1.0], "radius": 16 }"#)
            .roundtrip(round_trip::<GlowDef>),
        ComponentDescription::new("look_particles", "An emitter riding the holder.")
            .field(FieldDescription::new(
                "config",
                FieldType::Text,
                "the manifest particle config; the load checks it",
            ))
            .field(
                FieldDescription::new(
                    "at",
                    FieldType::Text,
                    "the holder's sprite point it emits at, its drawn centre on a frame without it",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "offset",
                    FieldType::Vec2,
                    "world pixels from its point, mirrored and turned as an overlay's offset is",
                )
                .default("[0.0, 0.0]"),
            )
            .sample(r#"{ "config": "sparks" }"#)
            .roundtrip(round_trip::<ParticlesDef>),
        ComponentDescription::new("look_light", "A point light at the holder, kept on the fixed step.")
            .field(FieldDescription::new(
                "color",
                FieldType::Color,
                "linear; its alpha scales the intensity",
            ))
            .field(FieldDescription::new(
                "radius",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "world pixels it reaches",
            ))
            .field(
                FieldDescription::new(
                    "intensity",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "at a share of 1",
                )
                .default("1.0"),
            )
            .sample(r#"{ "color": [1.0, 0.6, 0.2, 1.0], "radius": 64 }"#)
            .roundtrip(round_trip::<LightDef>),
        ComponentDescription::new("look_sound", "A looping sound the running look wants.")
            .field(FieldDescription::new(
                "id",
                FieldType::Text,
                "the manifest sound; the load checks it",
            ))
            .field(
                FieldDescription::new(
                    "volume",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "its want's share of the sound's volume at a share of 1",
                )
                .default("1.0"),
            )
            .sample(r#"{ "id": "crackle" }"#)
            .roundtrip(round_trip::<LookSoundDef>),
    ];
    for shape in shapes {
        describe_shape(world, shape);
    }
}

/// The scene component [`LooksPlugin`] registers.
const LOOKS: &str = "looks";

/// The shape of a `looks` entry.
const TAG_LOOK: &str = "tag_look";

/// A look shown while its holder's tags pass a filter (redo 7.10g): its
/// tint, overlay, glow and particles as a status look's, at a share of 1,
/// or rising linearly from 0 over `grow` seconds of game time shown. A
/// state a game sets is a tag it inserts, `state.alerted` by convention, so
/// the looks, the animator's rules and the hit filters read one
/// vocabulary. Build one with [`TagLook::new`].
///
/// # Examples
///
/// ```
/// use tungsten_kit::{Look, Overlay, TagFilter, TagLook};
///
/// let ring = TagLook::new(
///     TagFilter::new().all(["state.charging"]),
///     Look::new().with_overlay(Overlay::new("charge_ring").with_scale(0.25, 1.0)),
/// )
/// .with_grow(0.5);
/// assert_eq!(ring.grow(), 0.5);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TagLook {
    when: TagFilter,
    look: Look,
    grow: f32,
}

impl TagLook {
    /// `look` shown whole while the holder's tags pass `when`.
    ///
    /// # Panics
    ///
    /// When `look` has a light or a sound, which only a status look keeps.
    #[must_use]
    #[track_caller]
    pub fn new(when: TagFilter, look: Look) -> Self {
        if let Err(error) = tag_look_parts(&look) {
            panic!("TagLook::new: {error}");
        }
        Self {
            when,
            look,
            grow: 0.0,
        }
    }

    /// The look, rising linearly from 0 over `seconds` of game time shown.
    ///
    /// # Panics
    ///
    /// When `seconds` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_grow(mut self, seconds: f32) -> Self {
        assert!(
            seconds.is_finite() && seconds >= 0.0,
            "TagLook::with_grow: seconds must be finite and >= 0 (got {seconds})"
        );
        self.grow = seconds;
        self
    }

    /// The seconds it grows over; 0 for one shown whole at once.
    #[must_use]
    pub fn grow(&self) -> f32 {
        self.grow
    }

    /// Its share after `shown` game seconds.
    fn share(&self, shown: f32) -> f32 {
        if self.grow > 0.0 {
            (shown / self.grow).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }
}

/// The parts a tag look shows: a light or a sound is a status look's alone,
/// kept on the fixed step with its instance.
fn tag_look_parts(look: &Look) -> Result<(), String> {
    for (part, has) in [
        ("light", look.light.is_some()),
        ("sound", look.sound.is_some()),
    ] {
        if has {
            return Err(format!(
                "look.{part}: a tag look shows a tint, an overlay, a glow and particles"
            ));
        }
    }
    Ok(())
}

/// The looks an entity shows by its tags (redo 7.10g): each
/// [`TagLook`]'s parts while its filter passes, spawned by the early look
/// pass when it starts passing and taken when it stops, its time starting
/// again. The overlays mirror and turn as the holder is drawn, as a status
/// overlay's do, whatever wrote its flip; a holder drawn unflipped whose
/// [`Facing`](crate::Facing) neither turns nor flips it mirrors them by
/// that facing's side, left of the vertical, its last side kept while it
/// points straight up or down. The parts are plain entities the default
/// extract draws, kept by holder and entry index, and the passes that run
/// them are the status looks'; a game changes what shows by tags.
///
/// Scene name `looks`: a list of
/// `{ "when": { "all": ["state.alerted"] }, "look": { "overlay": { "animation": "mark_alert", "offset": [6, -18] } }, "grow": 0.4 }`,
/// `grow` 0 by default; the load checks each filter's tags and each look's
/// IDs, and refuses a light or a sound. Unsaved presentation: a load shows
/// the parts again from the restored tags.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Looks {
    entries: Vec<TagLook>,
}

impl Looks {
    /// No looks.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The looks, with `look` after the others.
    #[must_use]
    pub fn with(mut self, look: TagLook) -> Self {
        self.entries.push(look);
        self
    }

    /// The entries, in order.
    #[must_use]
    pub fn entries(&self) -> &[TagLook] {
        &self.entries
    }
}

impl Inspectable for Looks {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![("looks", format!("{} by tags", self.entries.len()))]
    }
}

/// `components.looks` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct LooksDef(Vec<TagLookDef>);

/// A `looks` entry, its filter's names kept as written.
#[derive(Serialize, Deserialize)]
#[serde(try_from = "TagLookFields")]
struct TagLookDef {
    when: TagFilterDef,
    look: Look,
    grow: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TagLookFields {
    when: TagFilterDef,
    look: Look,
    #[serde(default, deserialize_with = "non_negative")]
    grow: f32,
}

impl TryFrom<TagLookFields> for TagLookDef {
    type Error = String;

    fn try_from(fields: TagLookFields) -> Result<Self, Self::Error> {
        tag_look_parts(&fields.look)?;
        Ok(Self {
            when: fields.when,
            look: fields.look,
            grow: fields.grow,
        })
    }
}

/// Registers the `looks` scene component with its description, and the
/// look passes ([`EFFECT_LOOK`], [`EFFECT_LOOK_LATE`]) when the schedule
/// lacks them: [`EffectsPlugin`](crate::EffectsPlugin) registers the same.
#[derive(Debug, Default, Clone, Copy)]
pub struct LooksPlugin;

impl Plugin for LooksPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Looks>(world, "Looks");
        describe_looks(world);
        describe(
            world,
            ComponentDescription::new(
                LOOKS,
                "Looks the entity shows while its tags pass their filters: a tint, an overlay, a glow and particles, as a status shows them.",
            )
            .field(FieldDescription::new(
                "list",
                FieldType::List(Box::new(FieldType::Shape(TAG_LOOK))),
                "the value itself: each a look and the tag filter it shows while; the load checks each",
            ))
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .genre(SHOOTER)
            .sample("[]")
            .roundtrip(round_trip::<LooksDef>),
        );
        describe_shape(
            world,
            ComponentDescription::new(
                TAG_LOOK,
                "A look shown while the holder's tags pass `when`, whole or growing from nothing.",
            )
            .field(FieldDescription::new(
                "when",
                FieldType::Filter,
                "the holder's tags, `state.*` by convention for a state the game sets; the load checks each",
            ))
            .field(FieldDescription::new(
                "look",
                FieldType::Shape("look"),
                "its tint, overlay, glow and particles, at the look's share; a light or a sound is refused",
            ))
            .field(
                FieldDescription::new(
                    "grow",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "seconds of game time shown over which its share rises from 0 to 1; whole at once at 0",
                )
                .default("0.0"),
            )
            .sample(r#"{ "when": { "all": ["status.burning"] }, "look": {} }"#)
            .roundtrip(round_trip::<TagLookDef>),
        );
        world.register_component_with_assets(
            LOOKS,
            |def: &LooksDef, world: &World| {
                for (i, entry) in def.0.iter().enumerate() {
                    for (field, name) in entry.when.names() {
                        check_declared(world, &format!("{i}.when.{field}"), name)?;
                    }
                    entry.look.check(world, &format!("{i}.look"))?;
                }
                Ok(())
            },
            |def: LooksDef, _: &SpawnContext<'_>| {
                let entries = def
                    .0
                    .into_iter()
                    .map(|entry| TagLook {
                        when: entry.when.into(),
                        look: entry.look,
                        grow: entry.grow,
                    })
                    .collect();
                (Looks { entries },)
            },
        );
        ensure_looks(schedule, world);
    }
}

/// One running look on a holder, as `effect_tick` lists it each step: the
/// holder, the instance's serial, the band whose look it is (`None` for the
/// effect's own) and its share. The passes read the look itself from the
/// instance's definition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LookEntry {
    pub(crate) holder: Entity,
    pub(crate) serial: u64,
    pub(crate) band: Option<usize>,
    pub(crate) share: f32,
}

impl LookEntry {
    /// Whether `parts` are this look's.
    fn owns(&self, parts: &Parts) -> bool {
        self.holder == parts.holder && self.serial == parts.serial && self.band == parts.band
    }
}

/// The step's running looks, a World resource
/// [`EffectsPlugin`](crate::EffectsPlugin) inserts and `effect_tick`
/// refills; empty in a World with no look.
#[derive(Debug, Default)]
pub(crate) struct LookHolders(pub(crate) Vec<LookEntry>);

/// One look's parts, with the game seconds they have shown.
#[derive(Debug)]
struct Parts {
    holder: Entity,
    serial: u64,
    band: Option<usize>,
    overlay: Option<Entity>,
    glow: Option<Entity>,
    emitter: Option<Entity>,
    shown: f32,
}

/// One tag look's parts, by its holder and its entry's index, with the
/// game seconds it has shown and the side its holder's facing last
/// pointed, for a facing that neither turns nor flips its holder.
#[derive(Debug)]
struct TagParts {
    holder: Entity,
    index: usize,
    overlay: Option<Entity>,
    glow: Option<Entity>,
    emitter: Option<Entity>,
    shown: f32,
    left: bool,
}

/// The look passes' state between frames, shared by the two.
#[derive(Default)]
pub(crate) struct LookState {
    parts: Vec<Parts>,
    /// Each tinted holder's colour as the late pass read it and as it wrote
    /// it.
    tinted: Vec<(Entity, [u8; 4], [u8; 4])>,
    /// The step's entries, taken from the World for a pass.
    entries: Vec<LookEntry>,
    warned: bool,
    /// The tag looks' parts, one per passing entry.
    tag_parts: Vec<TagParts>,
    /// The frame's passing tag looks, by holder and entry.
    tag_live: Vec<(Entity, usize)>,
    /// The frame's tag tints, a factor per holder.
    tag_tints: Vec<(Entity, [f32; 3])>,
    /// The health bars' parts.
    bars: Bars,
}

/// The look `entry` names, from its holder's instance.
fn look_of(world: &World, entry: &LookEntry) -> Option<Look> {
    crate::damage::effects::running_look(world, entry.holder, entry.serial)
        .map(|(look, _, _)| look.clone())
}

impl LookState {
    /// Takes the World's entries into `entries`; whether there is anything
    /// to do.
    fn take_entries(&mut self, world: &mut World) -> bool {
        let running = world
            .get_resource::<LookHolders>()
            .is_some_and(|holders| !holders.0.is_empty());
        if !running && self.parts.is_empty() && self.tinted.is_empty() {
            return false;
        }
        self.entries.clear();
        if let Some(holders) = world.get_resource::<LookHolders>() {
            self.entries.extend_from_slice(&holders.0);
        }
        true
    }

    /// The early pass: keep each tag look's parts and each health bar's,
    /// put tinted colours back, then keep each running look's parts.
    pub(crate) fn early(&mut self, world: &mut World) {
        self.early_tags(world);
        self.bars.early(world);
        if !self.take_entries(world) {
            return;
        }
        for (holder, read, written) in self.tinted.drain(..) {
            if let Some(sprite) = world.get_mut::<Sprite>(holder)
                && sprite.color == written
            {
                sprite.color = read;
            }
        }
        // Parts whose look is gone, whose band changed, or whose holder is
        // gone or despawning go; the rest age by the frame's game time.
        let entries = std::mem::take(&mut self.entries);
        let dt = frame_seconds(world);
        let mut i = 0;
        while i < self.parts.len() {
            let parts = &mut self.parts[i];
            let live = world.is_alive(parts.holder) && !world.has::<Despawning>(parts.holder);
            if live && entries.iter().any(|entry| entry.owns(parts)) {
                parts.shown += dt;
                i += 1;
                continue;
            }
            let parts = self.parts.swap_remove(i);
            for part in [parts.overlay, parts.glow, parts.emitter]
                .into_iter()
                .flatten()
            {
                if world.is_alive(part) {
                    world.despawn(part);
                }
            }
        }
        for entry in &entries {
            if !world.is_alive(entry.holder)
                || world.has::<Despawning>(entry.holder)
                || self.parts.iter().any(|parts| entry.owns(parts))
            {
                continue;
            }
            let Some(look) = look_of(world, entry) else {
                continue;
            };
            if look.has_parts() {
                let parts = self.spawn_parts(world, entry, &look);
                self.parts.push(parts);
            }
        }
        // The emitters, at the holder's point this frame, before the
        // particles emit.
        for parts in &self.parts {
            let Some(emitter) = parts.emitter else {
                continue;
            };
            let Some(entry) = entries.iter().find(|entry| entry.owns(parts)) else {
                continue;
            };
            let at = crate::damage::effects::running_look(world, entry.holder, entry.serial)
                .and_then(|(look, _, _)| look.particles.as_ref())
                .and_then(|particles| emitter_point(world, parts.holder, particles, None));
            if let Some(at) = at
                && let Some(transform) = world.get_mut::<Transform>(emitter)
            {
                transform.position = at;
            }
            if let Some(particles) = world.get_mut::<ParticleEmitter>(emitter) {
                particles.rate_scale = entry.share;
            }
        }
        self.entries = entries;
    }

    /// Spawns `look`'s overlay and glow for `entry` at its holder's point.
    fn spawn_parts(&mut self, world: &mut World, entry: &LookEntry, look: &Look) -> Parts {
        let (overlay, glow, emitter) = self.spawn_entities(world, entry.holder, look, entry.share);
        Parts {
            holder: entry.holder,
            serial: entry.serial,
            band: entry.band,
            overlay,
            glow,
            emitter,
            shown: 0.0,
        }
    }

    /// Keeps each passing tag look's parts: a holder's entry whose filter
    /// stops passing, or whose holder is gone or despawning, loses its
    /// parts and its time; one that starts passing spawns them at 0 s;
    /// the rest age by the frame's game time, their emitters at the
    /// holder's point by their share.
    ///
    /// The holders are found by a query only while one lives: a query
    /// shape regrows its kept matches whenever an archetype is added
    /// (`D-206`), so a World with no holder reads the presence alone, and
    /// with parts still kept, lets them all go.
    fn early_tags(&mut self, world: &mut World) {
        self.tag_live.clear();
        if world.has_any::<Looks>() {
            for (holder, looks) in world.query::<(Entity, &Looks)>() {
                if world.has::<Despawning>(holder) {
                    continue;
                }
                let tags = world.get::<Tags>(holder);
                for (index, entry) in looks.entries.iter().enumerate() {
                    if entry.when.matches(tags) {
                        self.tag_live.push((holder, index));
                    }
                }
            }
        }
        if self.tag_live.is_empty() && self.tag_parts.is_empty() {
            return;
        }
        let dt = frame_seconds(world);
        let mut i = 0;
        while i < self.tag_parts.len() {
            let parts = &mut self.tag_parts[i];
            if self.tag_live.contains(&(parts.holder, parts.index)) {
                parts.shown += dt;
                i += 1;
                continue;
            }
            let parts = self.tag_parts.swap_remove(i);
            for part in [parts.overlay, parts.glow, parts.emitter]
                .into_iter()
                .flatten()
            {
                if world.is_alive(part) {
                    world.despawn(part);
                }
            }
        }
        for k in 0..self.tag_live.len() {
            let (holder, index) = self.tag_live[k];
            if self
                .tag_parts
                .iter()
                .any(|parts| (parts.holder, parts.index) == (holder, index))
            {
                continue;
            }
            let Some(entry) = tag_entry(world, holder, index).cloned() else {
                continue;
            };
            let (overlay, glow, emitter) =
                self.spawn_entities(world, holder, &entry.look, entry.share(0.0));
            self.tag_parts.push(TagParts {
                holder,
                index,
                overlay,
                glow,
                emitter,
                shown: 0.0,
                left: false,
            });
        }
        for parts in &mut self.tag_parts {
            let Some(emitter) = parts.emitter else {
                continue;
            };
            let Some(entry) = tag_entry(world, parts.holder, parts.index) else {
                continue;
            };
            let share = entry.share(parts.shown);
            let at = entry.look.particles.as_ref().and_then(|particles| {
                emitter_point(world, parts.holder, particles, Some(&mut parts.left))
            });
            if let Some(at) = at
                && let Some(transform) = world.get_mut::<Transform>(emitter)
            {
                transform.position = at;
            }
            if let Some(particles) = world.get_mut::<ParticleEmitter>(emitter) {
                particles.rate_scale = share;
            }
        }
    }

    /// Spawns `look`'s overlay, glow and emitter for `holder` at its point,
    /// the emitter at `share`.
    fn spawn_entities(
        &mut self,
        world: &mut World,
        holder: Entity,
        look: &Look,
        share: f32,
    ) -> (Option<Entity>, Option<Entity>, Option<Entity>) {
        let at = world
            .get::<Transform>(holder)
            .map_or(Vec2::ZERO, |transform| transform.position);
        let overlay = look
            .overlay
            .as_ref()
            .and_then(|overlay| spawn_overlay(world, holder, overlay, at));
        let glow = look.glow.and_then(|_| {
            let white = world
                .get_resource::<AssetRegistry>()
                .and_then(|assets| assets.sprite_id(ENGINE_WHITE));
            let material = world
                .get_resource::<MaterialRegistry>()
                .and_then(|materials| materials.get(ENGINE_GLOW));
            let (Some(white), Some(material)) = (white, material) else {
                if !self.warned {
                    self.warned = true;
                    log::warn!(
                        "a status look's glow needs `{ENGINE_WHITE}` and `{ENGINE_GLOW}`; it draws none"
                    );
                }
                return None;
            };
            Some(world.spawn_with((
                Transform::from_position(at),
                Sprite::new(white).with_material(material),
                Visibility::default(),
            )))
        });
        let emitter = look.particles.as_ref().and_then(|particles| {
            let config = world
                .get_resource::<ParticleConfigRegistry>()?
                .id_for_name(&particles.config)?;
            let mut emitter = ParticleEmitter::new(config);
            emitter.rate_scale = share;
            Some(world.spawn_with((
                Transform::from_position(at),
                emitter,
                ParticleEmitterState::default(),
            )))
        });
        (overlay, glow, emitter)
    }

    /// The late pass: tint each holder and place each look's parts at its
    /// current frame, flip, turn and depth, the tag looks' and the health
    /// bars' after the running looks'.
    pub(crate) fn late(&mut self, world: &mut World) {
        self.gather_tag_tints(world);
        if self.take_entries(world) {
            self.late_running(world);
        }
        self.late_tags(world);
        self.bars.late(world);
    }

    /// The running looks' tints and parts.
    fn late_running(&mut self, world: &mut World) {
        let entries = std::mem::take(&mut self.entries);
        // Tints, in instance order, over the colour the frame's writers left.
        let mut holder = None;
        let mut factor = [1.0_f32; 3];
        for entry in &entries {
            let Some(tint) =
                crate::damage::effects::running_look(world, entry.holder, entry.serial)
                    .and_then(|(look, _, _)| look.tint)
            else {
                continue;
            };
            if holder != Some(entry.holder) {
                self.tint(world, holder, factor);
                holder = Some(entry.holder);
                factor = [1.0; 3];
            }
            let weight = (entry.share * tint[3]).clamp(0.0, 1.0);
            for (f, t) in factor.iter_mut().zip(tint) {
                *f *= 1.0 + (t - 1.0) * weight;
            }
        }
        self.tint(world, holder, factor);
        for parts in &self.parts {
            let Some(entry) = entries.iter().find(|entry| entry.owns(parts)) else {
                continue;
            };
            let Some((look, _, _)) =
                crate::damage::effects::running_look(world, entry.holder, entry.serial)
            else {
                continue;
            };
            let overlay = look
                .overlay
                .as_ref()
                .map(|overlay| Placed::of(overlay, entry.share, parts.shown));
            let glow = look.glow;
            let Some(drawn) = drawn(world, parts.holder) else {
                continue;
            };
            if let (Some(part), Some(placed)) = (parts.overlay, overlay) {
                place_overlay(world, part, placed, &drawn);
            }
            if let (Some(part), Some(glow)) = (parts.glow, glow) {
                place_glow(world, part, glow, entry.share, &drawn);
            }
        }
        self.entries = entries;
    }

    /// Each tag-tinted holder's factor this frame, its tag looks' tints by
    /// their shares multiplied in entry order.
    fn gather_tag_tints(&mut self, world: &World) {
        self.tag_tints.clear();
        for parts in &self.tag_parts {
            let Some(entry) = tag_entry(world, parts.holder, parts.index) else {
                continue;
            };
            let Some(tint) = entry.look.tint else {
                continue;
            };
            let weight = (entry.share(parts.shown) * tint[3]).clamp(0.0, 1.0);
            let found = self
                .tag_tints
                .iter()
                .position(|(holder, _)| *holder == parts.holder);
            let at = found.unwrap_or_else(|| {
                self.tag_tints.push((parts.holder, [1.0; 3]));
                self.tag_tints.len() - 1
            });
            for (f, t) in self.tag_tints[at].1.iter_mut().zip(tint) {
                *f *= 1.0 + (t - 1.0) * weight;
            }
        }
    }

    /// The tag tints no running look's holder took, then each tag look's
    /// overlay and glow, mirrored and turned as its holder is drawn, as a
    /// status overlay is. A holder drawn unflipped whose [`Facing`] neither
    /// turns nor flips it mirrors them by that facing's side instead, left
    /// of the vertical, the entry keeping its last side while the facing
    /// points straight up or down.
    fn late_tags(&mut self, world: &mut World) {
        let tints = std::mem::take(&mut self.tag_tints);
        for &(holder, factor) in &tints {
            self.tint(world, Some(holder), factor);
        }
        self.tag_tints = tints;
        self.tag_tints.clear();
        for parts in &mut self.tag_parts {
            let Some((overlay, glow, share)) =
                tag_entry(world, parts.holder, parts.index).map(|entry| {
                    let share = entry.share(parts.shown);
                    let overlay = entry
                        .look
                        .overlay
                        .as_ref()
                        .map(|overlay| Placed::of(overlay, share, parts.shown));
                    (overlay, entry.look.glow, share)
                })
            else {
                continue;
            };
            let Some(mut drawn) = drawn(world, parts.holder) else {
                continue;
            };
            drawn.flip = mirrors(world, parts.holder, drawn.flip, &mut parts.left);
            if let (Some(part), Some(placed)) = (parts.overlay, overlay) {
                place_overlay(world, part, placed, &drawn);
            }
            if let (Some(part), Some(glow)) = (parts.glow, glow) {
                place_glow(world, part, glow, share, &drawn);
            }
        }
    }

    /// Writes `holder`'s sprite colour times `factor` and its tag looks'
    /// tints, keeping what it read and wrote for the next frame's early
    /// pass.
    fn tint(&mut self, world: &mut World, holder: Option<Entity>, mut factor: [f32; 3]) {
        let Some(holder) = holder else {
            return;
        };
        if let Some(at) = self
            .tag_tints
            .iter()
            .position(|(tinted, _)| *tinted == holder)
        {
            let (_, tags) = self.tag_tints.swap_remove(at);
            for (f, t) in factor.iter_mut().zip(tags) {
                *f *= t;
            }
        }
        let Some(sprite) = world.get_mut::<Sprite>(holder) else {
            return;
        };
        let read = sprite.color;
        let mut written = read;
        for (channel, f) in written.iter_mut().zip(factor) {
            *channel = (f32::from(*channel) * f).round().clamp(0.0, 255.0) as u8;
        }
        sprite.color = written;
        self.tinted.push((holder, read, written));
    }
}

/// Where and how a holder is drawn this frame.
pub(crate) struct Drawn {
    pub(crate) centre: Vec2,
    rotation: f32,
    scale: Vec2,
    pub(crate) flip: bool,
    pub(crate) z: i32,
    /// Half its drawn height, unturned: 0 without a sprite.
    pub(crate) half_height: f32,
}

/// `holder`'s drawn centre, turn, scale, flip and depth, as the default
/// extract places its quad; its `Transform`'s point without a sprite.
pub(crate) fn drawn(world: &World, holder: Entity) -> Option<Drawn> {
    let transform = *world.get::<Transform>(holder)?;
    let flip = world.get::<SpriteFlip>(holder).is_some_and(|flip| flip.0);
    let sprite = world.get::<Sprite>(holder);
    let z = sprite.map_or(0, |sprite| sprite.z_order);
    let asset = sprite.and_then(|sprite| {
        world
            .get_resource::<AssetRegistry>()
            .and_then(|assets| assets.sprite(sprite.asset_id))
    });
    Some(Drawn {
        centre: asset.map_or(transform.position, |asset| {
            sprite_centre(asset, &transform, flip)
        }),
        rotation: transform.rotation,
        scale: transform.scale,
        flip,
        z,
        half_height: asset.map_or(0.0, |asset| {
            asset.height as f32 * transform.scale.y.abs() * 0.5
        }),
    })
}

/// Where a sprite of `asset` draws its centre for an entity at `transform`,
/// flipped or not, as the default extract places its quad: its top-left at
/// the point, or its anchor there, mirrored by a flip, scaled and turned
/// about it. [`centre_of`](crate::centre_of) and a spawn's centre read it.
pub(crate) fn sprite_centre(asset: &SpriteAsset, transform: &Transform, flip: bool) -> Vec2 {
    let width = asset.width as f32;
    let size = Vec2::new(width, asset.height as f32) * transform.scale;
    let mut top_left = transform.position;
    if let Some(anchor) = asset.anchor {
        let anchor = if flip {
            [width - anchor[0], anchor[1]]
        } else {
            anchor
        };
        let anchor = Vec2::from(anchor) * transform.scale;
        let arm = size * 0.5 - anchor;
        top_left += turn(arm, transform.rotation) - arm - anchor;
    }
    top_left + size * 0.5
}

fn turn(arm: Vec2, rotation: f32) -> Vec2 {
    if rotation == 0.0 {
        arm
    } else {
        Vec2::from_angle(rotation).rotate(arm)
    }
}

/// Spawns an overlay at `at`: one playing its clip from a phase drawn from
/// its holder's index, or one showing its sprite; `None` when the World has
/// no such clip or sprite.
fn spawn_overlay(world: &mut World, holder: Entity, overlay: &Overlay, at: Vec2) -> Option<Entity> {
    let material = overlay
        .material
        .as_ref()
        .and_then(|id| world.get_resource::<MaterialRegistry>()?.get(id));
    let with_material = |sprite: Sprite| match material {
        Some(material) => sprite.with_material(material),
        None => sprite,
    };
    let animation = match &overlay.art {
        Art::Clip(animation) => animation,
        Art::Sprite(id) => {
            let sprite = world.get_resource::<AssetRegistry>()?.sprite_id(id)?;
            return Some(world.spawn_with((
                Transform::from_position(at),
                with_material(Sprite::new(sprite)),
                Visibility::default(),
                SpriteFlip(false),
            )));
        }
    };
    let animations = world.get_resource::<AnimationRegistry>()?;
    let clip = animations.get(animation)?;
    let first = clip.frames.first()?.sprite;
    let total = clip.total_duration_ms().max(1);
    let mut state = AnimationState::new(animation.clone()).with_speed(1.0);
    // Holders do not play in step: a phase from the index's hash.
    let phase =
        (u64::from(holder.id()).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32) % u64::from(total);
    state.advance(phase as f32, animations);
    state.set_speed(overlay.speed);
    let sprite = with_material(Sprite::new(
        state.current_sprite(animations).unwrap_or(first),
    ));
    Some(world.spawn_with((
        Transform::from_position(at),
        sprite,
        state,
        Visibility::default(),
        SpriteFlip(false),
    )))
}

/// An overlay's place this frame: its offset, bobbed, its depth over the
/// holder's, its scale times the holder's and whether its frame mirrors.
#[derive(Clone, Copy)]
struct Placed {
    offset: Vec2,
    z: i32,
    scale: f32,
    flip: bool,
}

impl Placed {
    /// `overlay`'s place at `share`, `shown` game seconds from its show.
    fn of(overlay: &Overlay, share: f32, shown: f32) -> Self {
        Self {
            offset: overlay.offset_at(shown),
            z: overlay.z,
            scale: overlay.scale_at(share),
            flip: overlay.flip,
        }
    }
}

/// `offset` from a point of the holder as `drawn` shows it, mirrored by
/// `flip`, scaled and turned with it.
fn arm(offset: Vec2, flip: bool, drawn: &Drawn) -> Vec2 {
    let offset = if flip {
        Vec2::new(-offset.x, offset.y)
    } else {
        offset
    };
    turn(offset * drawn.scale, drawn.rotation)
}

/// Whether `holder`'s parts mirror this frame: by `flip`, as it is drawn,
/// or, drawn unflipped under a [`Facing`] that neither turns nor flips it,
/// by that facing's side, left of the vertical, `left` keeping the last
/// side while it points straight up or down.
pub(crate) fn mirrors(world: &World, holder: Entity, flip: bool, left: &mut bool) -> bool {
    match world.get::<Facing>(holder) {
        Some(facing) if !facing.turns_or_flips() => {
            let side = facing.direction().x;
            if side != 0.0 {
                *left = side < 0.0;
            }
            flip || *left
        }
        _ => flip,
    }
}

/// Where a look's particles emit on `holder` this frame: its named sprite
/// point, else its drawn centre, plus their offset mirrored and turned as
/// an overlay's is; by the holder's facing too with `left`, a tag look's
/// last side ([`mirrors`]).
fn emitter_point(
    world: &World,
    holder: Entity,
    particles: &LookParticles,
    left: Option<&mut bool>,
) -> Option<Vec2> {
    let point = particles
        .at
        .as_deref()
        .and_then(|point| sprite_point(world, holder, point));
    if particles.offset == Vec2::ZERO {
        return point.or_else(|| drawn(world, holder).map(|drawn| drawn.centre));
    }
    let drawn = drawn(world, holder)?;
    let flip = match left {
        Some(left) => mirrors(world, holder, drawn.flip, left),
        None => drawn.flip,
    };
    Some(point.unwrap_or(drawn.centre) + arm(particles.offset, flip, &drawn))
}

/// The frame's game seconds, none while paused.
fn frame_seconds(world: &World) -> f32 {
    world
        .get_resource::<Time>()
        .map_or(0.0, Time::delta)
        .max(0.0)
}

/// Places `part`, an overlay's entity, as `placed` puts it on the holder as
/// `drawn` shows it: its frame's drawn centre on the holder's plus the
/// offset, whatever the frame's anchor, so it holds there as its scale
/// grows; its frame mirrored with the holder unless `placed` keeps it
/// unflipped.
fn place_overlay(world: &mut World, part: Entity, placed: Placed, drawn: &Drawn) {
    let Placed {
        offset,
        z,
        scale,
        flip,
    } = placed;
    let centre = drawn.centre + arm(offset, drawn.flip, drawn);
    let flip = drawn.flip && flip;
    let mut placed = Transform {
        position: Vec2::ZERO,
        rotation: drawn.rotation,
        scale: drawn.scale * scale,
    };
    // The frame's drawn centre from its point: half its scaled size for an
    // unanchored frame.
    let from_point = world
        .get::<Sprite>(part)
        .and_then(|sprite| {
            world
                .get_resource::<AssetRegistry>()
                .and_then(|assets| assets.sprite(sprite.asset_id))
        })
        .map_or(Vec2::ZERO, |asset| sprite_centre(asset, &placed, flip));
    placed.position = centre - from_point;
    if let Some(transform) = world.get_mut::<Transform>(part) {
        *transform = placed;
    }
    if let Some(sprite) = world.get_mut::<Sprite>(part) {
        sprite.z_order = drawn.z.saturating_add(z);
    }
    if let Some(part_flip) = world.get_mut::<SpriteFlip>(part) {
        part_flip.0 = flip;
    }
}

/// Places `part`, a glow, about the holder's drawn centre, one below it, its
/// alpha by `share`.
fn place_glow(world: &mut World, part: Entity, glow: Glow, share: f32, drawn: &Drawn) {
    let size = Vec2::splat(glow.radius * 2.0);
    if let Some(transform) = world.get_mut::<Transform>(part) {
        *transform = Transform {
            position: drawn.centre - size * 0.5,
            rotation: 0.0,
            scale: size,
        };
    }
    if let Some(sprite) = world.get_mut::<Sprite>(part) {
        let alpha = glow.color[3] * share;
        let color = [glow.color[0], glow.color[1], glow.color[2], alpha];
        sprite.color = linear_bytes(color);
        sprite.z_order = drawn.z.saturating_sub(1);
    }
}

/// A linear colour's channels as a sprite's bytes, each clamped to
/// `[0, 1]`.
pub(crate) fn linear_bytes(color: [f32; 4]) -> [u8; 4] {
    color.map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// The tag look `index` of `holder`'s [`Looks`].
fn tag_entry(world: &World, holder: Entity, index: usize) -> Option<&TagLook> {
    world.get::<Looks>(holder)?.entries.get(index)
}

#[cfg(test)]
#[path = "../tests/looks.rs"]
mod tests;

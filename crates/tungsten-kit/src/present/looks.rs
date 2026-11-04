//! Status looks (redo 7.10p): what an effect's instance shows on its holder
//! while it runs, drawn as plain entities the default extract draws: a
//! tint on the holder's sprite, an overlay clip riding it and a glow under
//! it. The kit adds no render type of its own (`D-007`, `D-016`).

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::components::SpriteFlip;
use tungsten_core::{
    AnimationRegistry, AnimationState, AssetRegistry, Entity, MaterialRegistry, MissingAsset,
    ParticleConfigRegistry, ParticleEmitter, ParticleEmitterState, SoundRegistry, Sprite,
    SpriteAsset, Transform, Visibility, World, sprite_point,
};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, describe_shape, round_trip,
};
use crate::foundation::{finite_color, non_negative, positive};
use crate::lifecycle::ended::Despawning;

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
/// - `overlay`, `{ "animation", "speed", "offset", "z", "material" }`: an
///   entity playing the clip, looped, at the holder's drawn centre plus
///   `offset`, mirrored and turned with it, `z` above the holder's, at
///   `speed`, from a phase drawn from the holder's index, through
///   `material` when set.
/// - `glow`, `{ "color", "radius" }`: `engine_white` through `engine_glow`,
///   `radius` px about the drawn centre, one below the holder, its alpha by
///   the share.
/// - `particles`, `{ "config", "at" }`: an emitter of the particle config at
///   the holder's named sprite point `at`, else its drawn centre, moved
///   there each frame, so it follows the frame, the flip and the turn, its
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

/// A look's particles: the config and the named point they emit at.
#[derive(Debug, Clone, PartialEq)]
struct LookParticles {
    config: String,
    at: Option<String>,
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

/// A look's overlay: a clip played on an entity riding the holder. Build
/// one with [`Overlay::new`].
#[derive(Debug, Clone, PartialEq)]
pub struct Overlay {
    animation: String,
    speed: f32,
    offset: Vec2,
    z: i32,
    material: Option<String>,
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
        });
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
            if let Some(animations) = world.get_resource::<AnimationRegistry>()
                && animations.get(&overlay.animation).is_none()
            {
                return Err(MissingAsset::new(
                    format!("{field}.overlay.animation"),
                    "animation",
                    overlay.animation.as_str(),
                ));
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
        Self {
            animation: animation.into(),
            speed: 1.0,
            offset: Vec2::ZERO,
            z: 1,
            material: None,
        }
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

/// An overlay as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OverlayDef {
    animation: String,
    #[serde(default = "one", deserialize_with = "non_negative")]
    speed: f32,
    #[serde(default)]
    offset: [f32; 2],
    #[serde(default = "above")]
    z: i32,
    #[serde(default)]
    material: Option<String>,
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
        Ok(Self {
            tint: def.tint,
            overlay: def.overlay.map(|overlay| Overlay {
                animation: overlay.animation,
                speed: overlay.speed,
                offset: Vec2::from(overlay.offset),
                z: overlay.z,
                material: overlay.material,
            }),
            glow: def.glow.map(|glow| Glow {
                color: glow.color,
                radius: glow.radius,
            }),
            particles: def.particles.map(|particles| LookParticles {
                config: particles.config,
                at: particles.at,
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

impl From<Look> for LookDef {
    fn from(look: Look) -> Self {
        Self {
            tint: look.tint,
            overlay: look.overlay.map(|overlay| OverlayDef {
                animation: overlay.animation,
                speed: overlay.speed,
                offset: overlay.offset.into(),
                z: overlay.z,
                material: overlay.material,
            }),
            glow: look.glow.map(|glow| GlowDef {
                color: glow.color,
                radius: glow.radius,
            }),
            particles: look.particles.map(|particles| ParticlesDef {
                config: particles.config,
                at: particles.at,
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
                "a clip played on an entity riding the holder",
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
            "A clip looped on an entity at the holder's drawn centre plus its offset, mirrored and turned with it.",
        )
        .field(FieldDescription::new(
            "animation",
            FieldType::Text,
            "the manifest animation; the load checks it",
        ))
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
        .sample(r#"{ "animation": "frost" }"#)
        .roundtrip(round_trip::<OverlayDef>),
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

/// One look's parts.
#[derive(Debug)]
struct Parts {
    holder: Entity,
    serial: u64,
    band: Option<usize>,
    overlay: Option<Entity>,
    glow: Option<Entity>,
    emitter: Option<Entity>,
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

    /// The early pass: put tinted colours back, then keep each running
    /// look's parts.
    pub(crate) fn early(&mut self, world: &mut World) {
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
        // gone or despawning go.
        let entries = std::mem::take(&mut self.entries);
        let mut i = 0;
        while i < self.parts.len() {
            let parts = &self.parts[i];
            let live = world.is_alive(parts.holder) && !world.has::<Despawning>(parts.holder);
            if live && entries.iter().any(|entry| entry.owns(parts)) {
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
                .and_then(|particles| particles.at.as_deref())
                .and_then(|point| sprite_point(world, parts.holder, point))
                .or_else(|| drawn(world, parts.holder).map(|drawn| drawn.centre));
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
        let at = world
            .get::<Transform>(entry.holder)
            .map_or(Vec2::ZERO, |transform| transform.position);
        let overlay = look
            .overlay
            .as_ref()
            .and_then(|overlay| spawn_overlay(world, entry.holder, overlay, at));
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
            emitter.rate_scale = entry.share;
            Some(world.spawn_with((
                Transform::from_position(at),
                emitter,
                ParticleEmitterState::default(),
            )))
        });
        Parts {
            holder: entry.holder,
            serial: entry.serial,
            band: entry.band,
            overlay,
            glow,
            emitter,
        }
    }

    /// The late pass: tint each holder and place each look's parts at its
    /// current frame, flip, turn and depth.
    pub(crate) fn late(&mut self, world: &mut World) {
        if !self.take_entries(world) {
            return;
        }
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
                .map(|overlay| (overlay.offset, overlay.z));
            let glow = look.glow;
            let Some(drawn) = drawn(world, parts.holder) else {
                continue;
            };
            if let (Some(part), Some((offset, z))) = (parts.overlay, overlay) {
                place_overlay(world, part, offset, z, &drawn);
            }
            if let (Some(part), Some(glow)) = (parts.glow, glow) {
                let size = Vec2::splat(glow.radius * 2.0);
                if let Some(transform) = world.get_mut::<Transform>(part) {
                    *transform = Transform {
                        position: drawn.centre - size * 0.5,
                        rotation: 0.0,
                        scale: size,
                    };
                }
                if let Some(sprite) = world.get_mut::<Sprite>(part) {
                    let alpha = glow.color[3] * entry.share;
                    let color = [glow.color[0], glow.color[1], glow.color[2], alpha];
                    sprite.color =
                        color.map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8);
                    sprite.z_order = drawn.z.saturating_sub(1);
                }
            }
        }
        self.entries = entries;
    }

    /// Writes `holder`'s sprite colour times `factor`, keeping what it read
    /// and wrote for the next frame's early pass.
    fn tint(&mut self, world: &mut World, holder: Option<Entity>, factor: [f32; 3]) {
        let Some(holder) = holder else {
            return;
        };
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
struct Drawn {
    centre: Vec2,
    rotation: f32,
    scale: Vec2,
    flip: bool,
    z: i32,
}

/// `holder`'s drawn centre, turn, scale, flip and depth, as the default
/// extract places its quad; its `Transform`'s point without a sprite.
fn drawn(world: &World, holder: Entity) -> Option<Drawn> {
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

/// Spawns an overlay playing its clip at `at`, from a phase drawn from its
/// holder's index; `None` when the World has no such clip.
fn spawn_overlay(world: &mut World, holder: Entity, overlay: &Overlay, at: Vec2) -> Option<Entity> {
    let animations = world.get_resource::<AnimationRegistry>()?;
    let clip = animations.get(&overlay.animation)?;
    let first = clip.frames.first()?.sprite;
    let total = clip.total_duration_ms().max(1);
    let mut state = AnimationState::new(overlay.animation.clone()).with_speed(1.0);
    // Holders do not play in step: a phase from the index's hash.
    let phase =
        (u64::from(holder.id()).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32) % u64::from(total);
    state.advance(phase as f32, animations);
    state.set_speed(overlay.speed);
    let mut sprite = Sprite::new(state.current_sprite(animations).unwrap_or(first));
    if let Some(material) = overlay
        .material
        .as_ref()
        .and_then(|id| world.get_resource::<MaterialRegistry>()?.get(id))
    {
        sprite = sprite.with_material(material);
    }
    Some(world.spawn_with((
        Transform::from_position(at),
        sprite,
        state,
        Visibility::default(),
        SpriteFlip(false),
    )))
}

/// Places `part`, an overlay's entity `offset` from the holder's centre and
/// `z` above it, on the holder as `drawn` shows it.
fn place_overlay(world: &mut World, part: Entity, offset: Vec2, z: i32, drawn: &Drawn) {
    let offset = if drawn.flip {
        Vec2::new(-offset.x, offset.y)
    } else {
        offset
    };
    let centre = drawn.centre + turn(offset * drawn.scale, drawn.rotation);
    let size = world
        .get::<Sprite>(part)
        .and_then(|sprite| {
            world
                .get_resource::<AssetRegistry>()
                .and_then(|assets| assets.sprite(sprite.asset_id))
        })
        .map_or(Vec2::ZERO, |asset| {
            Vec2::new(asset.width as f32, asset.height as f32) * drawn.scale
        });
    if let Some(transform) = world.get_mut::<Transform>(part) {
        *transform = Transform {
            position: centre - size * 0.5,
            rotation: drawn.rotation,
            scale: drawn.scale,
        };
    }
    if let Some(sprite) = world.get_mut::<Sprite>(part) {
        sprite.z_order = drawn.z.saturating_add(z);
    }
    if let Some(flip) = world.get_mut::<SpriteFlip>(part) {
        flip.0 = drawn.flip;
    }
}

#[cfg(test)]
#[path = "../tests/looks.rs"]
mod tests;

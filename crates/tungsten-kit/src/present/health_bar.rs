//! The health bar (redo 7.10g): a holder's health drawn as two
//! `engine_white` sprites over it, plain entities the default extract draws
//! (`D-007`), kept and placed by the look passes.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::{
    AssetRegistry, Entity, Inspectable, Plugin, Schedule, SpawnContext, Sprite, Transform,
    Visibility, World,
};

use crate::damage::effects::ensure_looks;
use crate::damage::health::Health;
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, PLATFORMER, RPG, SHOOTER, describe,
    round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::ensure_phases;
use crate::foundation::{finite_color, non_negative};
use crate::lifecycle::ended::Despawning;
use crate::present::looks::{drawn, linear_bytes, mirrors};

/// The scene component [`HealthBarPlugin`] registers.
const HEALTH_BAR: &str = "health_bar";

/// The reserved sprite the parts draw.
const ENGINE_WHITE: &str = "engine_white";

const DEFAULT_SIZE: [f32; 2] = [64.0, 6.0];
const DEFAULT_RISE: f32 = 6.0;
const DEFAULT_BACK: [f32; 4] = [0.05, 0.04, 0.05, 0.86];
const DEFAULT_FILL: [f32; 4] = [0.77, 0.19, 0.16, 1.0];
const DEFAULT_INSET: f32 = 1.0;

/// A bar over its holder showing its [`Health`](crate::Health)'s share
/// (redo 7.10g): a back of `size` world pixels centred over the holder's
/// drawn centre plus `offset`, its bottom `rise` above the drawn top, and a
/// fill `inset` inside it from the left, its width the share. The offset's
/// x mirrors as a look's overlay offset does: with the holder's drawn flip,
/// or by the side of a [`Facing`](crate::Facing) that neither turns nor
/// flips it, so a bar over a body off its frame's centre stays over it
/// facing either way. Both draw `engine_white` in their linear colours, at
/// the holder's depth plus 1 and 2, as plain entities the default extract
/// draws; a World without `engine_white` draws none, with one warning. The
/// bar hides while its holder is dead, or has no health, and with `full`
/// false while the health is full, so it waits for a first hurt.
///
/// Scene name `health_bar`:
/// `{ "size": [64, 6], "rise": 6, "offset": [0, 0], "back": [0.05, 0.04, 0.05, 0.86], "fill": [0.77, 0.19, 0.16, 1.0], "inset": 1, "full": true }`,
/// each field with those defaults. Unsaved presentation.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_kit::HealthBar;
///
/// let bar = HealthBar::new().with_size(Vec2::new(48.0, 4.0)).hidden_at_full();
/// assert_eq!(bar.size(), Vec2::new(48.0, 4.0));
/// assert!(!bar.full());
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HealthBar {
    size: Vec2,
    rise: f32,
    offset: Vec2,
    back: [f32; 4],
    fill: [f32; 4],
    inset: f32,
    full: bool,
}

impl Default for HealthBar {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthBar {
    /// A 64 × 6 bar, 6 px over its holder, its fill 1 px inside, shown at
    /// full health.
    #[must_use]
    pub fn new() -> Self {
        Self {
            size: Vec2::from(DEFAULT_SIZE),
            rise: DEFAULT_RISE,
            offset: Vec2::ZERO,
            back: DEFAULT_BACK,
            fill: DEFAULT_FILL,
            inset: DEFAULT_INSET,
            full: true,
        }
    }

    /// The bar, `size` world pixels.
    ///
    /// # Panics
    ///
    /// When either side is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_size(mut self, size: Vec2) -> Self {
        assert!(
            valid_size(size.to_array()),
            "HealthBar::with_size: each side must be finite and > 0 (got {size})"
        );
        self.size = size;
        self
    }

    /// The bar, its bottom `rise` world pixels over its holder's drawn top.
    ///
    /// # Panics
    ///
    /// When `rise` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_rise(mut self, rise: f32) -> Self {
        assert!(
            rise.is_finite() && rise >= 0.0,
            "HealthBar::with_rise: rise must be finite and >= 0 (got {rise})"
        );
        self.rise = rise;
        self
    }

    /// The bar, centred `offset` world pixels from its holder's drawn
    /// centre, its x mirrored with the holder.
    ///
    /// # Panics
    ///
    /// When a coordinate is not finite.
    #[must_use]
    #[track_caller]
    pub fn with_offset(mut self, offset: Vec2) -> Self {
        assert!(
            offset.is_finite(),
            "HealthBar::with_offset: offset must be finite (got {offset})"
        );
        self.offset = offset;
        self
    }

    /// The bar, its back and fill in these linear colours.
    ///
    /// # Panics
    ///
    /// When a channel is not finite.
    #[must_use]
    #[track_caller]
    pub fn with_colors(mut self, back: [f32; 4], fill: [f32; 4]) -> Self {
        assert!(
            back.iter().chain(&fill).all(|channel| channel.is_finite()),
            "HealthBar::with_colors: each channel must be finite"
        );
        self.back = back;
        self.fill = fill;
        self
    }

    /// The bar, its fill `inset` world pixels inside its back.
    ///
    /// # Panics
    ///
    /// When `inset` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_inset(mut self, inset: f32) -> Self {
        assert!(
            inset.is_finite() && inset >= 0.0,
            "HealthBar::with_inset: inset must be finite and >= 0 (got {inset})"
        );
        self.inset = inset;
        self
    }

    /// The bar, hidden while the health is full, so it waits for a first
    /// hurt.
    #[must_use]
    pub fn hidden_at_full(mut self) -> Self {
        self.full = false;
        self
    }

    /// Its size in world pixels.
    #[must_use]
    pub fn size(&self) -> Vec2 {
        self.size
    }

    /// Whether it shows at full health.
    #[must_use]
    pub fn full(&self) -> bool {
        self.full
    }
}

impl Inspectable for HealthBar {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let shown = if self.full { "always" } else { "once hurt" };
        vec![(
            "bar",
            format!("{} x {}, shown {shown}", self.size.x, self.size.y),
        )]
    }
}

/// Whether a bar's sides are finite and above 0.
fn valid_size(size: [f32; 2]) -> bool {
    size.iter().all(|side| side.is_finite() && *side > 0.0)
}

/// `components.health_bar` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(try_from = "HealthBarFields")]
struct HealthBarDef {
    size: [f32; 2],
    rise: f32,
    offset: [f32; 2],
    back: [f32; 4],
    fill: [f32; 4],
    inset: f32,
    full: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HealthBarFields {
    #[serde(default = "default_size")]
    size: [f32; 2],
    #[serde(default = "default_rise", deserialize_with = "non_negative")]
    rise: f32,
    #[serde(default)]
    offset: [f32; 2],
    #[serde(default = "default_back", deserialize_with = "finite_color")]
    back: [f32; 4],
    #[serde(default = "default_fill", deserialize_with = "finite_color")]
    fill: [f32; 4],
    #[serde(default = "default_inset", deserialize_with = "non_negative")]
    inset: f32,
    #[serde(default = "shown_full")]
    full: bool,
}

fn default_size() -> [f32; 2] {
    DEFAULT_SIZE
}

fn default_rise() -> f32 {
    DEFAULT_RISE
}

fn default_back() -> [f32; 4] {
    DEFAULT_BACK
}

fn default_fill() -> [f32; 4] {
    DEFAULT_FILL
}

fn default_inset() -> f32 {
    DEFAULT_INSET
}

fn shown_full() -> bool {
    true
}

impl TryFrom<HealthBarFields> for HealthBarDef {
    type Error = String;

    fn try_from(fields: HealthBarFields) -> Result<Self, Self::Error> {
        if !valid_size(fields.size) {
            return Err(format!(
                "size: each side must be finite and > 0 (got {:?})",
                fields.size
            ));
        }
        if fields
            .offset
            .iter()
            .any(|coordinate| !coordinate.is_finite())
        {
            return Err(format!(
                "offset: each must be finite (got {:?})",
                fields.offset
            ));
        }
        Ok(Self {
            size: fields.size,
            rise: fields.rise,
            offset: fields.offset,
            back: fields.back,
            fill: fields.fill,
            inset: fields.inset,
            full: fields.full,
        })
    }
}

impl From<HealthBarDef> for HealthBar {
    fn from(def: HealthBarDef) -> Self {
        Self {
            size: Vec2::from(def.size),
            rise: def.rise,
            offset: Vec2::from(def.offset),
            back: def.back,
            fill: def.fill,
            inset: def.inset,
            full: def.full,
        }
    }
}

/// Registers the `health_bar` scene component with its description, and
/// the look passes that keep and place the bars when the schedule lacks
/// them: [`EffectsPlugin`](crate::EffectsPlugin) registers the same.
#[derive(Debug, Default, Clone, Copy)]
pub struct HealthBarPlugin;

impl Plugin for HealthBarPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<HealthBar>(world, "HealthBar");
        let non_negative_px = || FieldType::F32 {
            min: Bound::Inclusive(0.0),
            max: Bound::Open,
        };
        describe(
            world,
            ComponentDescription::new(
                HEALTH_BAR,
                "A bar over the entity showing its health's share: a back and a fill drawn from `engine_white`, hidden while it is dead.",
            )
            .field(
                FieldDescription::new(
                    "size",
                    FieldType::Vec2,
                    "the back's width and height in world pixels, each above 0",
                )
                .default("[64.0, 6.0]"),
            )
            .field(
                FieldDescription::new(
                    "rise",
                    non_negative_px(),
                    "world pixels from the back's bottom up to the holder's drawn top",
                )
                .default("6.0"),
            )
            .field(
                FieldDescription::new(
                    "offset",
                    FieldType::Vec2,
                    "world pixels from the holder's drawn centre to the bar's, x mirrored as a look's overlay offset is",
                )
                .default("[0.0, 0.0]"),
            )
            .field(
                FieldDescription::new("back", FieldType::Color, "linear")
                    .default("[0.05, 0.04, 0.05, 0.86]"),
            )
            .field(
                FieldDescription::new(
                    "fill",
                    FieldType::Color,
                    "linear; its width the health's share",
                )
                .default("[0.77, 0.19, 0.16, 1.0]"),
            )
            .field(
                FieldDescription::new(
                    "inset",
                    non_negative_px(),
                    "world pixels the fill sits inside the back",
                )
                .default("1.0"),
            )
            .field(
                FieldDescription::new(
                    "full",
                    FieldType::Bool,
                    "shown at full health; when false it waits for a first hurt",
                )
                .default("true"),
            )
            .genre(PLATFORMER)
            .genre(RPG)
            .genre(SHOOTER)
            .sample("{}")
            .roundtrip(round_trip::<HealthBarDef>),
        );
        world.register_component_with(HEALTH_BAR, |def: HealthBarDef, _: &SpawnContext<'_>| {
            (HealthBar::from(def),)
        });
        ensure_looks(schedule, world);
    }
}

/// One bar's parts, with the side its holder's facing last pointed.
#[derive(Debug)]
struct BarParts {
    holder: Entity,
    back: Entity,
    fill: Entity,
    left: bool,
}

/// The health bars' parts between frames, which the look passes keep.
#[derive(Debug, Default)]
pub(crate) struct Bars {
    parts: Vec<BarParts>,
    /// The frame's holders, live and not despawning.
    live: Vec<Entity>,
    warned: bool,
}

impl Bars {
    /// Keeps a back and a fill for each live holder of a [`HealthBar`],
    /// hidden until the late pass places them; a holder gone, despawning or
    /// without its bar loses them.
    ///
    /// The holders are found by a query only while one lives, as the tag
    /// looks' are: a World with no holder reads the presence alone.
    pub(crate) fn early(&mut self, world: &mut World) {
        self.live.clear();
        if world.has_any::<HealthBar>() {
            for (holder, _) in world.query::<(Entity, &HealthBar)>() {
                if !world.has::<Despawning>(holder) {
                    self.live.push(holder);
                }
            }
        }
        if self.live.is_empty() && self.parts.is_empty() {
            return;
        }
        let mut i = 0;
        while i < self.parts.len() {
            if self.live.contains(&self.parts[i].holder) {
                i += 1;
                continue;
            }
            let parts = self.parts.swap_remove(i);
            for part in [parts.back, parts.fill] {
                if world.is_alive(part) {
                    world.despawn(part);
                }
            }
        }
        for k in 0..self.live.len() {
            let holder = self.live[k];
            if self.parts.iter().any(|parts| parts.holder == holder) {
                continue;
            }
            let Some(white) = world
                .get_resource::<AssetRegistry>()
                .and_then(|assets| assets.sprite_id(ENGINE_WHITE))
            else {
                if !self.warned {
                    self.warned = true;
                    log::warn!("a health bar needs `{ENGINE_WHITE}`; it draws none");
                }
                return;
            };
            let mut part = || {
                world.spawn_with((
                    Transform::default(),
                    Sprite::new(white),
                    Visibility { visible: false },
                ))
            };
            let (back, fill) = (part(), part());
            self.parts.push(BarParts {
                holder,
                back,
                fill,
                left: false,
            });
        }
    }

    /// Places each bar over its holder's drawn top at its health's share,
    /// or hides it.
    pub(crate) fn late(&mut self, world: &mut World) {
        for parts in &mut self.parts {
            let Some(bar) = world.get::<HealthBar>(parts.holder).copied() else {
                continue;
            };
            let health = world
                .get::<Health>(parts.holder)
                .map(|health| (health.current(), health.max(), health.is_dead()));
            let place = drawn(world, parts.holder)
                .zip(health)
                .filter(|(_, (current, max, dead))| !dead && (bar.full || current < max));
            for part in [parts.back, parts.fill] {
                if let Some(visibility) = world.get_mut::<Visibility>(part) {
                    visibility.visible = place.is_some();
                }
            }
            let Some((drawn, (current, max, _))) = place else {
                continue;
            };
            let share = if max > 0.0 {
                (current / max).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let mut offset = bar.offset;
            if offset.x != 0.0 && mirrors(world, parts.holder, drawn.flip, &mut parts.left) {
                offset.x = -offset.x;
            }
            let centre = drawn.centre + offset;
            let top = centre.y - drawn.half_height;
            let back = Vec2::new(centre.x - bar.size.x * 0.5, top - bar.rise - bar.size.y);
            let inner = (bar.size - Vec2::splat(bar.inset * 2.0)).max(Vec2::ZERO);
            put(
                world,
                parts.back,
                back,
                bar.size,
                bar.back,
                drawn.z.saturating_add(1),
            );
            put(
                world,
                parts.fill,
                back + Vec2::splat(bar.inset),
                Vec2::new(inner.x * share, inner.y),
                bar.fill,
                drawn.z.saturating_add(2),
            );
        }
    }
}

/// Places `part`, an `engine_white` sprite, as a `size` box from `at`, in
/// `color` at depth `z`.
fn put(world: &mut World, part: Entity, at: Vec2, size: Vec2, color: [f32; 4], z: i32) {
    if let Some(transform) = world.get_mut::<Transform>(part) {
        *transform = Transform {
            position: at,
            rotation: 0.0,
            scale: size,
        };
    }
    if let Some(sprite) = world.get_mut::<Sprite>(part) {
        sprite.color = linear_bytes(color);
        sprite.z_order = z;
    }
}

#[cfg(test)]
#[path = "../tests/health_bar.rs"]
mod tests;

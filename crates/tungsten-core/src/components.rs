//! D-042 render components; D-033 physics `Position` stays separate.

use std::sync::Arc;

use glam::{Vec2, Vec3};

use crate::assets::{AssetId, MaterialAssetId, ParticleConfig, ParticleMeshAssetId, SpriteAssetId};
use crate::ecs::{Entity, World};
use crate::physics::Position;
use crate::rng::Pcg32;
use crate::tween::Easing;

/// Visual transform; rotation is radians CCW around quad center.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub position: Vec2,
    pub rotation: f32,
    pub scale: Vec2,
}

impl Transform {
    /// Unit-scale transform at `position`.
    #[must_use]
    pub fn from_position(position: Vec2) -> Self {
        Self {
            position,
            rotation: 0.0,
            scale: Vec2::ONE,
        }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            rotation: 0.0,
            scale: Vec2::ONE,
        }
    }
}

/// Sprite render data resolved by asset ID at extract time.
///
/// `asset_id` comes from `AssetRegistry::intern_sprite` on the world's registry;
/// a sprite whose name is not registered draws nothing until it is.
///
/// M26: `material_id` selects a user-authored WGSL material pipeline on the
/// sprite draw path; `None` keeps the built-in sprite pipeline and the M25
/// default output bytes.
#[derive(Debug, Clone, Copy)]
pub struct Sprite {
    pub asset_id: SpriteAssetId,
    pub color: [u8; 4],
    pub z_order: i32,
    pub material_id: Option<MaterialAssetId>,
}

impl Sprite {
    /// No tint, z-order 0, built-in sprite pipeline.
    #[must_use]
    pub fn new(asset_id: SpriteAssetId) -> Self {
        Self {
            asset_id,
            color: [255; 4],
            z_order: 0,
            material_id: None,
        }
    }

    #[must_use]
    pub fn with_material(mut self, material: MaterialAssetId) -> Self {
        self.material_id = Some(material);
        self
    }
}

/// Explicit render gate required by default sprite extract.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Visibility {
    pub visible: bool,
}

impl Default for Visibility {
    fn default() -> Self {
        Self { visible: true }
    }
}

/// Mirrors its entity's sprite left to right while it holds `true` (redo
/// 7.9): about the sprite's anchor, or its centre without one, by a negative
/// UV width, and a lit sprite's normals mirror with it. It stays on the
/// entity while unflipped, so a turn moves no archetype; the kit's `Facing`
/// writes it from its direction when asked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SpriteFlip(pub bool);

/// Where the default extract draws the named point `name` of `entity`'s
/// current sprite (redo 7.10p), in world pixels: the manifest's `points`
/// entry, mirrored by its [`SpriteFlip`] as the image is, scaled by its
/// [`Transform`]'s scale and turned with its rotation about the sprite's
/// anchor, or about the quad's centre without one. `None` without a
/// `Transform`, a [`Sprite`], its registered sprite or the point. A sprite
/// on a parallax layer is drawn shifted by the camera; this point is not.
///
/// ```
/// use glam::Vec2;
/// use std::path::PathBuf;
/// use tungsten_core::assets::{FilterMode, UvRect};
/// use tungsten_core::{AssetRegistry, Sprite, TextureHandle, Transform, World, sprite_point};
///
/// let mut world = World::new();
/// let mut assets = AssetRegistry::new();
/// let keg = assets.register_sprite(
///     "keg".into(), FilterMode::Nearest, 16, 32, PathBuf::from("keg.png"),
///     TextureHandle(0), UvRect::FULL, None, None, None,
/// );
/// assets.set_sprite_points("keg", [("fuse".to_string(), [8.0, 0.0])].into());
/// world.insert_resource(assets);
/// let entity = world.spawn_with((Transform::from_position(Vec2::new(100.0, 50.0)), Sprite::new(keg)));
/// // Drawn from its top-left at the entity's point: the fuse 8 px right.
/// assert_eq!(sprite_point(&world, entity, "fuse"), Some(Vec2::new(108.0, 50.0)));
/// assert_eq!(sprite_point(&world, entity, "spout"), None);
/// ```
#[must_use]
pub fn sprite_point(world: &World, entity: Entity, name: &str) -> Option<Vec2> {
    let transform = world.get::<Transform>(entity)?;
    let sprite = world.get::<Sprite>(entity)?;
    let asset = world
        .get_resource::<crate::assets::AssetRegistry>()?
        .sprite(sprite.asset_id)?;
    let point = asset.point(name)?;
    let flip = world.get::<SpriteFlip>(entity).is_some_and(|flip| flip.0);
    let width = asset.width as f32;
    let mirror = |at: [f32; 2]| if flip { [width - at[0], at[1]] } else { at };
    let size = Vec2::new(width, asset.height as f32) * transform.scale;
    let turn = |arm: Vec2| {
        if transform.rotation == 0.0 {
            arm
        } else {
            Vec2::from_angle(transform.rotation).rotate(arm)
        }
    };
    // The quad's top-left, as the extract places it: an anchor lands on
    // the entity's point and the quad turns about it.
    let mut top_left = transform.position;
    if let Some(anchor) = asset.anchor {
        let anchor = Vec2::from(mirror(anchor)) * transform.scale;
        let arm = size * 0.5 - anchor;
        top_left += turn(arm) - arm - anchor;
    }
    let local = Vec2::from(mirror(point)) * transform.scale;
    Some(top_left + size * 0.5 + turn(local - size * 0.5))
}

/// Draws its entity's sprite on the stock sprite pipeline though the sprite
/// has normal or emissive maps, so no light reaches it (redo 7.9): a far
/// prop behind a lit scene.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Unlit;

/// A backdrop the default sprite base draws behind everything else (redo
/// 7.9): `sprite` with its top-left corner at the entity's
/// `Transform.position`, scaled by its `Transform.scale`, tinted, scrolled
/// against the camera by `factor` as a [`ParallaxLayer`] is and moving
/// `drift` world pixels a second on the game clock. `repeat` tiles it across
/// the camera's view along x, y or both; `fill` stretches one copy over the
/// view instead; `foot` stretches each copy's last texel row down to the
/// view's bottom, for a strip of hills over a plain. Backdrops draw first,
/// by `z_order`, then entity, on the stock pipeline at the far plane; an
/// entity with a `Visibility` that is off draws none.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Backdrop {
    /// The sprite it draws.
    pub sprite: SpriteAssetId,
    /// Multiplied into the sprite, RGBA from 0 to 255 a channel in linear
    /// light, which the frame encodes to sRGB.
    pub tint: [u8; 4],
    /// The parallax scroll factor per axis: `1.0` moves with the world,
    /// `0.0` stays on screen.
    pub factor: Vec2,
    /// The axes it repeats along across the view.
    pub repeat: BackdropRepeat,
    /// One copy stretched over the view, its repeat and foot unused.
    pub fill: bool,
    /// World pixels a second, on the game clock.
    pub drift: Vec2,
    /// Each copy's last texel row stretched down to the view's bottom; not
    /// with a repeat along y, which reaches it anyway.
    pub foot: bool,
    /// Its place among backdrops, lowest first.
    pub z_order: i32,
}

impl Backdrop {
    /// `sprite` once at the entity's point, untinted, moving with the world,
    /// with no drift or foot.
    #[must_use]
    pub fn new(sprite: SpriteAssetId) -> Self {
        Self {
            sprite,
            tint: [255; 4],
            factor: Vec2::ONE,
            repeat: BackdropRepeat::None,
            fill: false,
            drift: Vec2::ZERO,
            foot: false,
            z_order: 0,
        }
    }
}

/// The axes a [`Backdrop`] repeats along across the view.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BackdropRepeat {
    /// Once, at its point.
    #[default]
    None,
    /// Along x: a row across the view.
    X,
    /// Along y: a column down the view.
    Y,
    /// Along both: over the whole view.
    Xy,
}

/// 2D forward light source (M29). Closed-enum kind per `D-054`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Light {
    pub kind: LightKind,
    pub color: Vec3,
    pub intensity: f32,
}

/// Light shape (M29). `Point` carries world-space radius driving attenuation;
/// `Directional` carries a 2D angle (radians from +x axis).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LightKind {
    Point { radius: f32 },
    Directional { angle: f32 },
}

impl Light {
    /// Point light at unit intensity with the given world-space radius.
    #[must_use]
    pub fn point(color: Vec3, radius: f32) -> Self {
        Self {
            kind: LightKind::Point { radius },
            color,
            intensity: 1.0,
        }
    }

    /// Directional light from `angle` radians (CCW from +x) at unit intensity.
    #[must_use]
    pub fn directional(color: Vec3, angle: f32) -> Self {
        Self {
            kind: LightKind::Directional { angle },
            color,
            intensity: 1.0,
        }
    }
}

/// Debug/find label.
#[derive(Debug, Clone)]
pub struct Tag {
    pub name: String,
}

impl Tag {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// Declarative particle emitter. Redo 7.9: a game steers a live one by
/// `rate_scale`, `direction` and `paused`, and a changed `config` takes its
/// snapshot at the next tick.
#[derive(Debug, Clone, Copy)]
pub struct ParticleEmitter {
    pub config: AssetId<ParticleConfig>,
    pub seed_override: Option<u64>,
    /// Scales a continuous rate: 1 unless set.
    pub rate_scale: f32,
    /// Replaces a cone's or a vector's direction; `None` keeps the config's.
    pub direction: Option<Vec2>,
    /// While set, nothing emits; a burst not yet fired fires on resume.
    pub paused: bool,
}

impl ParticleEmitter {
    #[must_use]
    pub fn new(config: AssetId<ParticleConfig>) -> Self {
        Self {
            config,
            seed_override: None,
            rate_scale: 1.0,
            direction: None,
            paused: false,
        }
    }

    #[must_use]
    pub fn with_seed(config: AssetId<ParticleConfig>, seed: u64) -> Self {
        Self {
            seed_override: Some(seed),
            ..Self::new(config)
        }
    }
}

/// Particle emitter runtime state; first tick captures config `Arc`.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)] // Independent flags (`D-143` adds the burst latch); no two are a state machine.
pub struct ParticleEmitterState {
    pub config_snapshot: Option<Arc<ParticleConfig>>,
    pub rng: Pcg32,
    pub elapsed: f32,
    /// A `Continuous` emitter's fractional particles carried to the next tick.
    pub continuous_accum: f32,
    pub pulse_timer: f32,
    pub pulses_fired: u32,
    pub active_count: u32,
    pub drained: bool,
    pub first_tick_done: bool,
    pub drain_reported: bool,
    /// A `Burst` emitter has fired since it spawned or was last re-armed
    /// ([`ParticleEmitterState::rearm_burst`], `D-143`).
    pub burst_fired: bool,
    /// The config the snapshot is of (redo 7.9): an emitter whose `config`
    /// changes takes the new one's snapshot at its next tick.
    pub snapshot_of: Option<AssetId<ParticleConfig>>,
}

impl ParticleEmitterState {
    /// Re-arms a `Burst { once: false }` emitter: it fires again on its next
    /// tick (`D-143`). Does nothing on a drained emitter, so a `once: true`
    /// burst that has fired stays spent.
    pub fn rearm_burst(&mut self) {
        if !self.drained {
            self.burst_fired = false;
        }
    }
}

impl Default for ParticleEmitterState {
    fn default() -> Self {
        Self {
            config_snapshot: None,
            rng: Pcg32::seeded(0),
            elapsed: 0.0,
            continuous_accum: 0.0,
            pulse_timer: 0.0,
            pulses_fired: 0,
            active_count: 0,
            drained: false,
            first_tick_done: false,
            drain_reported: false,
            burst_fired: false,
            snapshot_of: None,
        }
    }
}

/// Live particle; spawn-time config snapshot preserves hot-reload isolation.
#[derive(Debug, Clone)]
pub struct Particle {
    pub config: Arc<ParticleConfig>,
    pub emitter: Option<Entity>,
    pub age: f32,
    pub lifetime: f32,
    pub velocity: Vec2,
    pub angular_velocity: f32,
    pub start_scale: f32,
    pub base_rgba: [f32; 4],
}

/// Mesh-drawn particle (M31, `D-093`); takes the place of `Sprite` on a particle entity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshParticle {
    pub mesh: ParticleMeshAssetId,
    pub color: [u8; 4],
}

/// Parallax scroll factor per axis (M30, `D-073`). `1.0` = world-locked,
/// `0.0` = screen-locked. Applied at extract time; never mutates `Transform`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParallaxLayer {
    pub scroll_factor: Vec2,
}

impl ParallaxLayer {
    #[must_use]
    pub fn new(scroll_factor: Vec2) -> Self {
        Self { scroll_factor }
    }

    /// Uniform factor on both axes.
    #[must_use]
    pub fn uniform(factor: f32) -> Self {
        Self {
            scroll_factor: Vec2::splat(factor),
        }
    }
}

/// Parallax position remap (M30, `D-073`): a CPU-side offset applied at extract
/// time, so one view-projection still draws every layer correctly and
/// `crates/tungsten-render/` needs no per-layer camera.
///
/// `scroll_factor == 1.0` returns `authored` unchanged (world-locked);
/// `0.0` pins the layer to the camera (screen-locked).
#[inline]
#[must_use]
pub fn parallax_world_position(authored: Vec2, scroll_factor: Vec2, camera_position: Vec2) -> Vec2 {
    authored + camera_position * (Vec2::ONE - scroll_factor)
}

/// Which gameplay trigger arms this entity's squash (M30). Closed enum, `D-054` style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SquashTrigger {
    OnLand,
    OnHit,
    OnPickup,
    Manual,
}

/// Squash/stretch config (M30). `amount` is the peak scale multiplier
/// (e.g. `(1.3, 0.7)` = widen and flatten).
///
/// `D-073`: deliberately not a `Tween` — `TweenRepeat` has no out-and-back
/// one-shot, and `D-055`'s single slot per entity is already spent on the M26
/// damage flash for the entity that must both squash and flash.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpriteSquashStretch {
    pub on: SquashTrigger,
    pub amount: Vec2,
    pub duration: f32,
    pub easing: Easing,
}

impl SpriteSquashStretch {
    /// Symmetric envelope sample: `sin(easing(t) * PI)` scales `amount`'s
    /// deviation from `1.0`, so the peak is `amount` and both endpoints return
    /// `base_scale` exactly — no separate cleanup write is needed.
    ///
    /// A non-positive `duration` is inert rather than a divide by zero.
    #[must_use]
    pub fn scale_at(&self, base_scale: Vec2, elapsed: f32) -> Vec2 {
        if self.duration <= 0.0 || elapsed <= 0.0 || elapsed >= self.duration {
            return base_scale;
        }
        let env = (self.easing.apply(elapsed / self.duration) * std::f32::consts::PI).sin();
        base_scale * (Vec2::ONE + (self.amount - Vec2::ONE) * env)
    }
}

/// Squash/stretch runtime envelope state (M30), inserted by
/// `squash_stretch_trigger_system`. `base_scale` is captured at trigger time so
/// the envelope restores the authored scale and a re-trigger mid-flight cannot
/// compound.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SquashStretchState {
    pub elapsed: f32,
    pub base_scale: Vec2,
}

/// D-033 one-way sync: physics `Position` -> visual `Transform.position`,
/// without interpolation. `PhysicsPlugin` runs
/// [`physics_sync`](crate::physics::physics_sync) instead, which draws a
/// body with a `PrevPosition` between its last two steps; a world driven by
/// hand without the fixed step's snapshot calls this one.
pub fn sync_position_to_transform(world: &mut World) {
    for (transform, position) in world.query_mut::<(&mut Transform, &Position)>() {
        transform.position = position.0;
    }
}

#[cfg(test)]
#[path = "tests/components.rs"]
mod tests;

//! `integrated` runtime state: what the collision scan knows about each
//! entity, dense per-walker arrays, the bench components, and the
//! [`Runtime`] resource the systems take out of the world while they run.

use glam::Vec2;
use tungsten::core::tween::UniformOverrideBlock;
use tungsten::core::{
    AnimationState, AssetId, CommandBuffer, Easing, Entity, MaterialAssetId, ParticleConfig, Pcg32,
    ScalarSlot, Sprite, TextureHandle, Tween, TweenChannel, World, splitmix64,
};
use tungsten::render::TextSection;

use super::assets;
use super::level::Level;
use super::{CameraPath, Params};

/// Walker collider half extents; its sprite stands on the collider's bottom.
pub(super) const WALKER_HALF: Vec2 = Vec2::new(6.0, 10.0);
pub(super) const WALK_SPEED: (f32, f32) = (50.0, 90.0);
/// Seconds between a walker's random hops.
pub(super) const JUMP_EVERY: (f32, f32) = (1.5, 4.0);
/// A contact counts as floor or wall past this normal component.
const CONTACT_AXIS: f32 = 0.7;
const MAX_HP: u8 = 9;
const FLASH_SECONDS: f32 = 0.3;
pub(super) const FLASH_TAG: &str = "flash";
const FLASH_COLOR: [f32; 4] = [1.0, 0.95, 0.85, 1.0];
const PLAY_SALT: u64 = 0x1E7E_2000_0000_0002;

/// What the collision scan needs to know about an entity, by entity id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    None,
    /// A walker and its slot in [`Actors`].
    Actor(u32),
    Crate,
    Projectile,
}

/// Walker marker: its slot in the dense [`Actors`] arrays.
#[derive(Debug, Clone, Copy)]
pub(super) struct Actor {
    pub(super) slot: u32,
}

/// A dynamic body's collider half extents and its sprite's texture size;
/// `sync_bodies` stands the sprite on the collider's bottom edge.
#[derive(Debug, Clone, Copy)]
pub(super) struct Body {
    pub(super) half: Vec2,
    pub(super) size: Vec2,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Projectile {
    pub(super) age: f32,
    pub(super) registered: bool,
}

/// A hit's spark emitter; despawned once its burst drains.
#[derive(Debug, Clone, Copy)]
pub(super) struct Spark;

/// Torch light flicker.
#[derive(Debug, Clone, Copy)]
pub(super) struct Torch {
    pub(super) base: f32,
    pub(super) rate: f32,
    pub(super) phase: f32,
}

/// Per-walker state, dense by slot, so the event scan writes flags without
/// looking components up.
#[derive(Debug, Default)]
pub(super) struct Actors {
    pub(super) entity: Vec<Entity>,
    pub(super) variant: Vec<u8>,
    pub(super) dir: Vec<f32>,
    pub(super) speed: Vec<f32>,
    pub(super) caster: Vec<bool>,
    pub(super) grounded: Vec<bool>,
    pub(super) blocked: Vec<bool>,
    pub(super) air_frames: Vec<u16>,
    pub(super) turn_cooldown: Vec<f32>,
    pub(super) jump_timer: Vec<f32>,
    pub(super) fire_timer: Vec<f32>,
    pub(super) hp: Vec<u8>,
    pub(super) flashing: Vec<bool>,
}

impl Actors {
    #[allow(clippy::too_many_arguments)] // One value per dense array.
    pub(super) fn push(
        &mut self,
        entity: Entity,
        variant: u8,
        dir: f32,
        speed: f32,
        caster: bool,
        jump_timer: f32,
        fire_timer: f32,
    ) -> u32 {
        self.entity.push(entity);
        self.variant.push(variant);
        self.dir.push(dir);
        self.speed.push(speed);
        self.caster.push(caster);
        self.grounded.push(false);
        self.blocked.push(false);
        self.air_frames.push(0);
        self.turn_cooldown.push(0.0);
        self.jump_timer.push(jump_timer);
        self.fire_timer.push(fire_timer);
        self.hp.push(MAX_HP);
        self.flashing.push(false);
        (self.entity.len() - 1) as u32
    }

    pub(super) fn len(&self) -> usize {
        self.entity.len()
    }

    pub(super) fn flashing(&self) -> usize {
        self.flashing.iter().filter(|&&flashing| flashing).count()
    }
}

/// A projectile's first contact this frame; `normal` points into the
/// projectile.
#[derive(Debug, Clone, Copy)]
pub(super) struct Hit {
    pub(super) projectile: Entity,
    pub(super) other: Option<Entity>,
    pub(super) normal: Vec2,
    pub(super) order: usize,
}

/// Running totals the HUD shows.
#[derive(Debug, Default)]
pub(super) struct Totals {
    pub(super) shots: u64,
    pub(super) hits: u64,
    pub(super) landings: u64,
    pub(super) turns: u64,
}

/// Runtime state; systems take it out of the world while they run.
#[derive(Debug)]
pub(super) struct Runtime {
    pub(super) params: Params,
    pub(super) level: Level,
    pub(super) camera: CameraPath,
    pub(super) rng: Pcg32,
    pub(super) elapsed: f32,
    pub(super) frame: u32,
    kinds: Vec<Kind>,
    pub(super) actors: Actors,
    pub(super) hits: Vec<Hit>,
    pub(super) spark: Option<AssetId<ParticleConfig>>,
    pub(super) flash: Option<MaterialAssetId>,
    /// The parallax strips' atlas page; its batches come first.
    pub(super) parallax_page: Option<TextureHandle>,
    pub(super) hud: Vec<TextSection>,
    pub(super) tags: Vec<TextSection>,
    pub(super) totals: Totals,
    pub(super) spark_seed: u64,
}

impl Runtime {
    pub(super) fn new(
        params: Params,
        level: Level,
        camera: CameraPath,
        spark: Option<AssetId<ParticleConfig>>,
        flash: Option<MaterialAssetId>,
        parallax_page: Option<TextureHandle>,
    ) -> Self {
        Self {
            params,
            level,
            camera,
            rng: Pcg32::seeded(splitmix64(params.seed ^ PLAY_SALT)),
            elapsed: 0.0,
            frame: 0,
            kinds: Vec::new(),
            actors: Actors::default(),
            hits: Vec::new(),
            spark,
            flash,
            parallax_page,
            hud: Vec::new(),
            tags: Vec::new(),
            totals: Totals::default(),
            spark_seed: params.seed,
        }
    }

    pub(super) fn kind(&self, entity: Entity) -> Kind {
        self.kinds
            .get(entity.id() as usize)
            .copied()
            .unwrap_or(Kind::None)
    }

    pub(super) fn set_kind(&mut self, entity: Entity, kind: Kind) {
        let index = entity.id() as usize;
        if self.kinds.len() <= index {
            self.kinds.resize(index + 1, Kind::None);
        }
        self.kinds[index] = kind;
    }

    /// One side of a contact: `normal` points into `me`.
    pub(super) fn touch(&mut self, me: Entity, other: Option<Entity>, normal: Vec2, order: usize) {
        match self.kind(me) {
            Kind::Actor(slot) => {
                let slot = slot as usize;
                if normal.y < -CONTACT_AXIS {
                    self.actors.grounded[slot] = true;
                }
                if normal.x * self.actors.dir[slot] < -CONTACT_AXIS {
                    self.actors.blocked[slot] = true;
                }
            }
            Kind::Projectile => self.hits.push(Hit {
                projectile: me,
                other,
                normal,
                order,
            }),
            Kind::Crate | Kind::None => {}
        }
    }

    /// A struck walker loses a hit point (wrapping) and, unless already
    /// flashing, starts the flash. The default extract draws a lit sprite
    /// without its material (lit wins, M29), so the flash plays the unlit
    /// copy of the clip through `damage_flash` with an override block whose
    /// amount a tween fades out.
    pub(super) fn strike(&mut self, world: &mut World, buf: &mut CommandBuffer, slot: usize) {
        let actors = &mut self.actors;
        actors.hp[slot] = actors.hp[slot].checked_sub(1).unwrap_or(MAX_HP);
        let Some(material) = self.flash else {
            return;
        };
        if actors.flashing[slot] {
            return;
        }
        actors.flashing[slot] = true;
        let (entity, variant) = (actors.entity[slot], actors.variant[slot]);
        swap_clip(world, entity, variant, Some(material));
        let mut block = UniformOverrideBlock::default();
        block.vec4[0] = FLASH_COLOR;
        block.f32s[0] = 1.0;
        buf.insert(entity, block);
        buf.insert(
            entity,
            Tween::new(FLASH_SECONDS, Easing::QuadOut)
                .with_channel(TweenChannel::UniformScalar {
                    slot: ScalarSlot::F0,
                    from: 1.0,
                    to: 0.0,
                })
                .with_tag(FLASH_TAG),
        );
    }
}

/// Moves a walker to its lit clip (`material` None) or to the unlit copy
/// drawn with `material`, keeping the clip position.
pub(super) fn swap_clip(
    world: &mut World,
    entity: Entity,
    variant: u8,
    material: Option<MaterialAssetId>,
) {
    let flat = material.is_some();
    let frame = world
        .get::<AnimationState>(entity)
        .map_or(0, |state| state.frame_index);
    if let Some(state) = world.get_mut::<AnimationState>(entity) {
        state.animation_id = assets::walk_clip(variant, flat);
    }
    if let Some(sprite) = world.get_mut::<Sprite>(entity) {
        sprite.asset_id = assets::walk_frame(variant, frame, flat);
        sprite.material_id = material;
    }
}

/// Top-left of a body's sprite at `scale`, standing on the collider's
/// bottom edge.
pub(super) fn top_left(center: Vec2, body: Body, scale: Vec2) -> Vec2 {
    let size = body.size * scale;
    Vec2::new(center.x - size.x * 0.5, center.y + body.half.y - size.y)
}

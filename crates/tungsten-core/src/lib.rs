//! Core building blocks for Tungsten: ECS, config, input, timing, assets.

pub mod assets;
pub mod audio;
pub mod camera;
pub mod components;
pub mod config;
pub mod debug_draw;
pub mod display;
pub mod ecs;
pub mod input;
pub mod inspect;
pub mod physics;
pub mod post;
pub mod rng;
pub mod time;
pub mod tween;

pub use assets::{
    AnimationData, AnimationRegistry, AnimationState, AssetId, AssetRegistry, AudioHandle,
    BlendMode, Curve, EmissionKind, FilterMode, FontEntry, FontRegistry, InitialVelocity,
    LayerKind, Lerp, LoadedManifest, ManifestError, MaterialAssetId, MaterialRegistry,
    MaterialUniformDefaults, ParticleActive, ParticleBudget, ParticleConfig, ParticleConfigError,
    ParticleConfigRegistry, ParticleEntry, Range, ResolvedFont, ResolvedManifest, ResolvedMaterial,
    ResolvedParticle, ResolvedSound, SceneData, SceneEntry, SceneError, SceneSprite,
    SceneTransform, SceneTween, SceneTweenChannel, SceneTweenRepeat, SoundData, SoundEntry,
    SoundRegistry, SpriteAsset, TextureHandle, TileIndex, TilemapData, TilemapInstance,
    TilemapLayer, TilemapRegistry, WorldRngSeed, EMPTY_TILE,
};
pub use audio::{AudioCommand, AudioCommands};
pub use camera::{CameraBounds, CameraController, CameraMode, CameraState};
pub use components::{
    sync_position_to_transform, Particle, ParticleEmitter, ParticleEmitterState, Sprite, Tag,
    Transform, Visibility,
};
pub use config::{Config, ConfigError, DepthSortMode, RenderConfig};
pub use debug_draw::{DebugCommand, DebugDraw, DebugShape, DEFAULT_CIRCLE_SEGMENTS};
pub use display::{
    DisplayConfig, DisplayMode, DisplayState, DisplayValidationError, Resolution, ScaleMode,
};
pub use ecs::{CommandBuffer, Entity, EventQueue, PendingEntity, World};
pub use input::{
    ActionMap, ActionMapError, Binding, InputState, KeyCode, MouseButton, ScrollDirection,
};
pub use inspect::Inspectable;
pub use physics::{
    aabb_vs_aabb, aabb_vs_circle, circle_vs_circle, physics_step, Aabb, BodyKind, Collider,
    CollisionEvent, Contact, PhysicsConfig, Position, RigidBody, Shape, SpatialGrid, Velocity,
};
pub use post::{
    ColorAdjustParams, CrtParams, DissolveParams, DitherMode, DitherParams, FadeParams,
    FilmGrainParams, FogParams, GlitchParams, GodRaysParams, LutParams, PixelOutlineParams,
    PostPass, PostStack, ToneMonoMode, ToneMonoParams, TonemapMode, TonemapParams, VignetteParams,
    WipeRadialParams,
};
pub use rng::{splitmix64, Pcg32};
pub use time::DeltaTime;
pub use tween::{
    lerp_f32, lerp_u8, Easing, IntSlot, ScalarSlot, Tween, TweenChannel, TweenComplete,
    TweenDirection, TweenRepeat, UniformOverrideBlock, Vec4Slot,
};

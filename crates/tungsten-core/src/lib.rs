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
pub mod lighting;
pub mod physics;
pub mod post;
pub mod rng;
pub mod schedule;
pub mod text;
pub mod time;
pub mod tween;
pub mod ui;

pub use assets::{
    AnimationData, AnimationRegistry, AnimationState, AssetId, AssetRegistry, AudioHandle,
    BlendMode, Curve, EMPTY_TILE, EmissionKind, FilterMode, FontEntry, FontFamilyEntry,
    FontRegistry, InitialVelocity, LayerKind, Lerp, LoadedManifest, ManifestError, MaterialAssetId,
    MaterialRegistry, MaterialUniformDefaults, ParticleActive, ParticleBudget, ParticleConfig,
    ParticleConfigError, ParticleConfigRegistry, ParticleEntry, ParticleMesh, ParticleMeshAssetId,
    ParticleMeshEntry, ParticleMeshRegistry, ParticleRender, Range, ResolvedFont,
    ResolvedFontFamily, ResolvedManifest, ResolvedMaterial, ResolvedParticle, ResolvedParticleMesh,
    ResolvedSound, SceneData, SceneEntry, SceneError, SceneSprite, SceneTransform, SceneTween,
    SceneTweenChannel, SceneTweenRepeat, SoundData, SoundEntry, SoundRegistry, SpriteAsset,
    SpriteAssetId, TextureHandle, TileIndex, TilemapData, TilemapInstance, TilemapLayer,
    TilemapRegistry, WorldRngSeed,
};
pub use audio::{AudioCommand, AudioCommands};
pub use camera::{CameraBounds, CameraController, CameraMode, CameraState};
pub use components::{
    Light, LightKind, MeshParticle, ParallaxLayer, Particle, ParticleEmitter, ParticleEmitterState,
    Sprite, SpriteSquashStretch, SquashStretchState, SquashTrigger, Tag, Transform, Visibility,
    parallax_world_position, sync_position_to_transform,
};
pub use config::{Config, ConfigError, DepthSortMode, GameConfig, PostAaMode, RenderConfig};
pub use debug_draw::{DEFAULT_CIRCLE_SEGMENTS, DebugCommand, DebugDraw, DebugShape};
pub use display::{
    DisplayConfig, DisplayMode, DisplayState, DisplayValidationError, Resolution, ScaleMode,
    WindowSize,
};
pub use ecs::{
    Bundle, CommandBuffer, Entity, EventQueue, OptionalColumn, PendingEntity, ShakeEvent,
    SquashEvent, With, Without, World,
};
pub use input::{
    ActionMap, ActionMapError, Binding, InputState, KeyCode, MouseButton, ScrollDirection,
};
pub use inspect::{InspectRegistry, Inspectable};
pub use lighting::{AmbientLight, LIGHT_CAP};
pub use physics::{
    Aabb, BodyKind, Collider, CollisionEvent, Contact, PhysicsBuffers, PhysicsConfig,
    PhysicsPlugin, Position, RigidBody, RigidBodyBundle, Shape, SpatialGrid, Velocity,
    aabb_vs_aabb, aabb_vs_circle, circle_vs_circle, physics_step,
};
pub use post::{
    BloomParams, ColorAdjustParams, CrtParams, DissolveParams, DitherMode, DitherParams,
    FadeParams, FilmGrainParams, FogParams, GlitchParams, GodRaysParams, LutParams,
    PixelOutlineParams, PostPass, PostStack, ToneMonoMode, ToneMonoParams, TonemapMode,
    TonemapParams, VignetteParams, WipeRadialParams,
};
pub use rng::{Pcg32, splitmix64};
pub use schedule::{Plugin, PluginSet, Schedule, ScheduleError, Stage, SystemDesc, system};
pub use text::{
    EllipsisAt, FontEpoch, FontFeature, FontFeatures, MeasureWidth, StyledText, TextAlign,
    TextHinting, TextLayout, TextMeasure, TextMetrics, TextNodeId, TextNodeStore, TextOverflow,
    TextSpan, TextStyle, TextWrap,
};
#[allow(deprecated)]
pub use time::DeltaTime;
pub use time::{Time, Timer, TimerMode};
pub use tween::{
    Easing, IntSlot, ScalarSlot, Tween, TweenChannel, TweenComplete, TweenDirection, TweenRepeat,
    UniformOverrideBlock, Vec4Slot, lerp_f32, lerp_u8,
};
pub use ui::{UiTree, WidgetId};

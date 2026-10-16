//! Core building blocks for Tungsten: ECS, config, input, timing, assets.

pub mod assets;
pub mod audio;
pub mod camera;
pub mod component_registry;
pub mod components;
pub mod config;
pub mod debug_draw;
pub mod display;
pub mod ecs;
pub mod input;
pub mod inspect;
pub mod lighting;
pub mod persist;
pub mod physics;
pub mod post;
pub mod rng;
pub mod save;
pub mod schedule;
pub mod settings;
pub mod text;
pub mod time;
pub mod tween;
pub mod ui;

pub use assets::{
    AnimationData, AnimationRegistry, AnimationState, AssetId, AssetRegistry, AudioHandle,
    BlendMode, Curve, EMPTY_TILE, EmissionKind, FilterMode, FontEntry, FontFamilyEntry,
    FontRegistry, InitialVelocity, LayerKind, Lerp, LoadedManifest, ManifestError, MapObject,
    MaterialAssetId, MaterialRegistry, MaterialUniformDefaults, ObjectLayer, ParticleActive,
    ParticleBudget, ParticleConfig, ParticleConfigError, ParticleConfigRegistry, ParticleEntry,
    ParticleMesh, ParticleMeshAssetId, ParticleMeshEntry, ParticleMeshRegistry, ParticleRender,
    PrefabData, PrefabEntry, PrefabError, PrefabRegistry, PreparedPrefab, Range, ResolvedFont,
    ResolvedFontFamily, ResolvedManifest, ResolvedMaterial, ResolvedParticle, ResolvedParticleMesh,
    ResolvedPrefab, ResolvedSound, ResolvedUiTheme, SceneData, SceneEntry, SceneError, SceneSprite,
    SceneTransform, SceneTween, SceneTweenChannel, SceneTweenRepeat, SoundData, SoundEntry,
    SoundRegistry, SpriteAsset, SpriteAssetId, TextureHandle, TileIndex, TilemapData,
    TilemapInstance, TilemapLayer, TilemapRegistry, UiThemeEntry, WorldRngSeed,
};
pub use audio::{AudioCommand, AudioCommands};
pub use camera::{CameraBounds, CameraController, CameraMode, CameraState};
pub use component_registry::{
    ComponentError, ComponentRegistry, MissingAsset, PreparedComponent, SpawnContext,
};
pub use components::{
    Light, LightKind, MeshParticle, ParallaxLayer, Particle, ParticleEmitter, ParticleEmitterState,
    Sprite, SpriteSquashStretch, SquashStretchState, SquashTrigger, Tag, Transform, Visibility,
    parallax_world_position, sync_position_to_transform,
};
pub use config::{
    Config, ConfigError, DepthSortMode, GameConfig, PostAaMode, RenderConfig, TimeConfig,
};
pub use debug_draw::{DEFAULT_CIRCLE_SEGMENTS, DebugCommand, DebugDraw, DebugShape};
pub use display::{
    DisplayConfig, DisplayMode, DisplayState, DisplayValidationError, Resolution, ScaleFactor,
    ScaleMode, WindowSize,
};
pub use ecs::{
    Bundle, CommandBuffer, Entity, EventCursor, EventQueue, EventRead, OptionalColumn,
    PendingEntity, ShakeEvent, SquashEvent, With, Without, World,
};
pub use input::{
    ActionMap, ActionMapError, AxisDirection, Binding, GamepadAxis, GamepadButton, GamepadId,
    InputState, KeyCode, Modifiers, MouseButton, RawEvent, RawInput, RoutedInput, STICK_PRESS,
    STICK_RELEASE, ScrollDirection, WheelDelta,
};
pub use inspect::{InspectRegistry, Inspectable};
pub use lighting::{AmbientLight, LIGHT_CAP};
pub use physics::{
    Aabb, BodyKind, Collider, CollisionEvent, Contact, PhysicsBuffers, PhysicsConfig,
    PhysicsPlugin, Position, PrevPosition, RigidBody, RigidBodyBundle, SensorEntered, Shape,
    SpatialFilter, SpatialGrid, SpatialHit, Velocity, aabb_vs_aabb, aabb_vs_circle,
    circle_vs_circle, physics_prev_snapshot, physics_step, physics_sync,
};
pub use post::{
    BloomParams, ColorAdjustParams, CrtParams, DissolveParams, DitherMode, DitherParams,
    FadeParams, FilmGrainParams, FogParams, GlitchParams, GodRaysParams, LutParams,
    PixelOutlineParams, PostPass, PostStack, ToneMonoMode, ToneMonoParams, TonemapMode,
    TonemapParams, VignetteParams, WipeRadialParams,
};
pub use rng::{Pcg32, splitmix64};
pub use save::{SaveData, SaveSlots};
pub use schedule::{Plugin, PluginSet, Schedule, ScheduleError, Stage, SystemDesc, system};
pub use settings::{Settings, UserFolder};
pub use text::{
    EllipsisAt, FontEpoch, FontFeature, FontFeatures, MeasureWidth, StyledText, TextAlign,
    TextHinting, TextLayout, TextMeasure, TextMetrics, TextNodeId, TextNodeStore, TextOverflow,
    TextSpan, TextStyle, TextWrap, WorldLabel, WorldLabels,
};
#[allow(deprecated)]
pub use time::DeltaTime;
pub use time::{Time, Timer, TimerMode};
pub use tween::{
    Easing, IntSlot, ScalarSlot, Tween, TweenChannel, TweenComplete, TweenDirection, TweenRepeat,
    UniformOverrideBlock, Vec4Slot, lerp_f32, lerp_u8,
};
pub use ui::{
    PaintList, UiDebug, UiEvent, UiProperty, UiRouter, UiThemeRegistry, UiTrack, UiTree, WidgetId,
};

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
pub mod script;
pub mod settings;
pub mod text;
pub mod time;
pub mod tween;
pub mod ui;

pub use assets::{
    AnimationData, AnimationRegistry, AnimationState, AssetId, AssetRegistry, AudioHandle,
    BlendMode, CellChange, ChangedCells, Curve, DataEntry, DataError, DataRef, DataRegistry,
    EMPTY_TILE, EmissionKind, FilterMode, FontEntry, FontFamilyEntry, FontRegistry,
    InitialVelocity, LayerKind, LayerPhysics, Lerp, LoadedManifest, ManifestError, MapCursor,
    MapObject, MaterialAssetId, MaterialRegistry, MaterialUniformDefaults, ObjectLayer,
    ParticleActive, ParticleBudget, ParticleConfig, ParticleConfigError, ParticleConfigRegistry,
    ParticleEntry, ParticleMesh, ParticleMeshAssetId, ParticleMeshEntry, ParticleMeshRegistry,
    ParticleRender, PrefabData, PrefabEntry, PrefabError, PrefabRegistry, PreparedPrefab, Range,
    ResolvedDataEntry, ResolvedFont, ResolvedFontFamily, ResolvedManifest, ResolvedMaterial,
    ResolvedParticle, ResolvedParticleMesh, ResolvedPrefab, ResolvedSound, ResolvedUiTheme,
    SceneData, SceneEntry, SceneError, SceneSprite, SceneTransform, SceneTween, SceneTweenChannel,
    SceneTweenRepeat, SoundData, SoundEntry, SoundRegistry, SpriteAsset, SpriteAssetId, StagedLoad,
    StagedLoadError, TextureHandle, TileChanges, TileDamage, TileIndex, TileProps, TilemapData,
    TilemapInstance, TilemapLayer, TilemapRegistry, UiThemeEntry, WorldRngSeed,
};
pub use audio::{AudioBus, AudioCommand, AudioCommands, MAX_VOICES, Voice};
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
    AudioConfig, Config, ConfigError, DepthSortMode, GameConfig, PostAaMode, RenderConfig,
    TimeConfig,
};
pub use debug_draw::{DEFAULT_CIRCLE_SEGMENTS, DebugCommand, DebugDraw, DebugShape};
pub use display::{
    DisplayConfig, DisplayMode, DisplayState, DisplayValidationError, MonitorInfo, Resolution,
    ScaleFactor, VideoMode, WindowSize,
};
pub use ecs::{
    Bundle, CommandBuffer, Entity, EventCursor, EventQueue, EventRead, OptionalColumn,
    PendingEntity, ShakeEvent, SquashEvent, With, Without, World, WorldCounters,
};
pub use input::{
    ActionMap, ActionMapError, AxisDirection, Binding, GamepadAxis, GamepadButton, GamepadId,
    InputState, KeyCode, Modifiers, MouseButton, RawEvent, RawInput, RoutedInput, STICK_PRESS,
    STICK_RELEASE, ScrollDirection, StickResponse, WheelDelta,
};
pub use inspect::{InspectRegistry, Inspectable};
pub use lighting::{AmbientLight, LIGHT_CAP};
pub use physics::{
    Aabb, BodyKind, Collider, CollisionEvent, Contact, PhysicsBuffers, PhysicsConfig,
    PhysicsLayers, PhysicsMaterial, PhysicsMaterials, PhysicsPlugin, Position, PrevPosition,
    RigidBody, RigidBodyBundle, SensorEntered, Shape, SpatialFilter, SpatialGrid, SpatialHit, Spin,
    TerrainFace, Velocity, aabb_vs_aabb, aabb_vs_circle, circle_vs_circle, physics_prev_snapshot,
    physics_step, physics_sync,
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
pub use settings::{FrameRateCap, Settings, UserFolder};
pub use text::{
    EllipsisAt, FontEpoch, FontFeature, FontFeatures, MeasureWidth, StyledText, TextAlign,
    TextHinting, TextLayout, TextMeasure, TextMetrics, TextNodeId, TextNodeStore, TextOverflow,
    TextSpan, TextStyle, TextWrap, WorldLabel, WorldLabels,
};
pub use time::{Time, Timer, TimerMode};
pub use tween::{
    Easing, IntSlot, ScalarSlot, Tween, TweenChannel, TweenComplete, TweenDirection, TweenRepeat,
    UniformOverrideBlock, Vec4Slot, lerp_f32, lerp_u8,
};
pub use ui::{
    PaintList, UiDebug, UiEvent, UiProperty, UiRouter, UiThemeRegistry, UiTrack, UiTree, WidgetId,
};

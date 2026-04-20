//! Core building blocks for the Tungsten 2D engine: a hand-rolled ECS
//! (World, Entity, Components, Resources), data-driven configuration,
//! edge-triggered input, frame timing, and a manifest-driven asset registry.

pub mod assets;
pub mod audio;
pub mod camera;
pub mod components;
pub mod config;
pub mod display;
pub mod ecs;
pub mod input;
pub mod physics;
pub mod time;

pub use assets::{
    AnimationData, AnimationRegistry, AnimationState, AssetRegistry, AudioHandle, FilterMode,
    FontEntry, FontRegistry, LayerKind, ManifestError, ResolvedFont, ResolvedManifest,
    ResolvedSound, SceneData, SceneEntry, SceneError, SceneSprite, SceneTransform, SoundData,
    SoundEntry, SoundRegistry, SpriteAsset, TextureHandle, TileIndex, TilemapData, TilemapInstance,
    TilemapLayer, TilemapRegistry, EMPTY_TILE,
};
pub use audio::{AudioCommand, AudioCommands};
pub use camera::{CameraBounds, CameraController, CameraMode, CameraState};
pub use components::{sync_position_to_transform, Sprite, Tag, Transform, Visibility};
pub use config::{Config, ConfigError};
pub use display::{
    DisplayConfig, DisplayMode, DisplayState, DisplayValidationError, Resolution, ScaleMode,
};
pub use ecs::{CommandBuffer, Entity, EventQueue, PendingEntity, World};
pub use input::{
    ActionMap, ActionMapError, Binding, InputState, KeyCode, MouseButton, ScrollDirection,
};
pub use physics::{
    aabb_vs_aabb, aabb_vs_circle, circle_vs_circle, physics_step, Aabb, BodyKind, Collider,
    CollisionEvent, Contact, PhysicsConfig, Position, RigidBody, Shape, SpatialGrid, Velocity,
};
pub use time::DeltaTime;

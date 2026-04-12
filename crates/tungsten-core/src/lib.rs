//! Core building blocks for the Tungsten 2D engine: a hand-rolled ECS
//! (World, Entity, Components, Resources), data-driven configuration,
//! edge-triggered input, frame timing, and a manifest-driven asset registry.

pub mod assets;
pub mod config;
pub mod ecs;
pub mod input;
pub mod time;

pub use assets::{
    AnimationData, AnimationRegistry, AnimationState, AssetRegistry, FilterMode, ManifestError,
    ResolvedManifest, SpriteAsset, TextureHandle,
};
pub use config::{Config, ConfigError};
pub use ecs::{Entity, World};
pub use input::{InputState, KeyCode, MouseButton};
pub use time::DeltaTime;

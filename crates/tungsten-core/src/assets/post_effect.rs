//! Post effect registry: a game's own post effects, by manifest ID
//! (`D-308`).
//!
//! `D-016` seam: no `wgpu` types here. The render side keeps each effect's
//! pipeline under the same [`PostEffectId`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::assets::material::MaterialUniformDefaults;
use crate::post::CustomPost;

/// Dense post-effect handle, minted per session by
/// [`PostEffectRegistry::allocate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
pub struct PostEffectId(pub u32);

/// Session-local post-effect ID ↔ manifest name ↔ shader name registry, with
/// each effect's authored defaults.
#[derive(Debug, Default, Clone)]
pub struct PostEffectRegistry {
    next: u32,
    ids: HashMap<String, PostEffectId>,
    names: HashMap<PostEffectId, String>,
    /// The manifest each effect came from; several share one.
    paths: HashMap<PostEffectId, PathBuf>,
    shader_names: HashMap<PostEffectId, String>,
    defaults: HashMap<PostEffectId, MaterialUniformDefaults>,
}

impl PostEffectRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocates the ID of manifest effect `id`, or returns the one it has
    /// with its shader, defaults and manifest path replaced: a manifest
    /// reload changes what [`Self::pass`] builds next and no pass a game
    /// already holds.
    pub fn allocate(
        &mut self,
        id: &str,
        path: PathBuf,
        shader_name: String,
        defaults: MaterialUniformDefaults,
    ) -> PostEffectId {
        if let Some(&existing) = self.ids.get(id) {
            self.shader_names.insert(existing, shader_name);
            self.defaults.insert(existing, defaults);
            self.paths.insert(existing, path);
            return existing;
        }
        let effect = PostEffectId(self.next);
        self.next += 1;
        self.ids.insert(id.to_string(), effect);
        self.names.insert(effect, id.to_string());
        self.paths.insert(effect, path);
        self.shader_names.insert(effect, shader_name);
        self.defaults.insert(effect, defaults);
        effect
    }

    /// The ID of manifest effect `id`, if one is allocated.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<PostEffectId> {
        self.ids.get(id).copied()
    }

    /// The manifest name `id` was allocated for.
    #[must_use]
    pub fn name_for_id(&self, id: PostEffectId) -> Option<&str> {
        self.names.get(&id).map(String::as_str)
    }

    /// The manifest the effect was last allocated from.
    #[must_use]
    pub fn path_for_id(&self, id: PostEffectId) -> Option<&Path> {
        self.paths.get(&id).map(PathBuf::as_path)
    }

    /// The `shaders` entry the effect draws with.
    #[must_use]
    pub fn shader_name_for_id(&self, id: PostEffectId) -> Option<&str> {
        self.shader_names.get(&id).map(String::as_str)
    }

    /// The effect's authored block defaults.
    #[must_use]
    pub fn defaults_for_id(&self, id: PostEffectId) -> Option<MaterialUniformDefaults> {
        self.defaults.get(&id).copied()
    }

    /// Every allocated effect as `(manifest name, ID)`, in no set order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, PostEffectId)> {
        self.ids.iter().map(|(name, id)| (name.as_str(), *id))
    }

    /// The pass that draws manifest effect `name`, the manifest's
    /// `uniform_defaults` in its lanes; `None` for a name no loaded manifest
    /// declares. Push it as `PostPass::Custom`.
    ///
    /// ```
    /// use tungsten_core::assets::{MaterialUniformDefaults, PostEffectRegistry};
    /// use tungsten_core::post::{PostPass, PostStack};
    ///
    /// let mut effects = PostEffectRegistry::new();
    /// let defaults = MaterialUniformDefaults { f32s: [0.5, 0.0, 0.0, 0.0], ..Default::default() };
    /// effects.allocate("heat", "assets/manifest.json".into(), "heat".into(), defaults);
    ///
    /// let mut pass = effects.pass("heat").expect("a declared effect");
    /// assert_eq!(pass.f32s[0], 0.5);
    /// pass.set_f32(0, 0.8);
    /// let mut stack = PostStack::new();
    /// stack.push(PostPass::Custom(pass));
    /// assert!(effects.pass("missing").is_none());
    /// ```
    #[must_use]
    pub fn pass(&self, name: &str) -> Option<CustomPost> {
        let effect = self.get(name)?;
        Some(CustomPost::new(
            effect,
            self.defaults_for_id(effect).unwrap_or_default(),
        ))
    }
}

//! Material asset registry: core-side source-of-truth for material IDs.
//!
//! D-016 seam: no `wgpu` types here. The render side keeps
//! `MaterialPipeline` values keyed by the same `MaterialAssetId`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::tween::UniformOverrideBlock;

/// Dense material handle, minted per session by `MaterialRegistry::allocate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterialAssetId(pub u32);

/// Manifest-author-facing material uniform defaults. Mirrors the live shape of
/// `UniformOverrideBlock` and uploads into the same 256-byte UBO slot used by
/// the renderer, so authored defaults line up with tween-driven overrides.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
pub struct MaterialUniformDefaults {
    #[serde(default)]
    pub vec4: [[f32; 4]; 4],
    #[serde(default)]
    pub f32s: [f32; 4],
    #[serde(default)]
    pub i32s: [i32; 4],
}

impl MaterialUniformDefaults {
    /// Project these defaults onto a fresh override block.
    #[must_use]
    pub fn to_override_block(&self) -> UniformOverrideBlock {
        let mut block = UniformOverrideBlock::default();
        block.vec4 = self.vec4;
        block.f32s = self.f32s;
        block.i32s = self.i32s;
        block
    }
}

/// Session-local material id ↔ manifest-name ↔ shader-id-name registry.
#[derive(Debug, Default, Clone)]
pub struct MaterialRegistry {
    next: u32,
    ids: HashMap<String, MaterialAssetId>,
    names: HashMap<MaterialAssetId, String>,
    paths: HashMap<MaterialAssetId, PathBuf>,
    reverse: HashMap<PathBuf, MaterialAssetId>,
    shader_names: HashMap<MaterialAssetId, String>,
    defaults: HashMap<MaterialAssetId, MaterialUniformDefaults>,
}

impl MaterialRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate (or return existing) id for a manifest material name. `path`
    /// is the manifest json path the material came from; it is kept to feed
    /// the hot-reload routing back into the right manifest.
    pub fn allocate(
        &mut self,
        id: &str,
        path: PathBuf,
        shader_name: String,
        defaults: MaterialUniformDefaults,
    ) -> MaterialAssetId {
        if let Some(&existing) = self.ids.get(id) {
            self.shader_names.insert(existing, shader_name);
            self.defaults.insert(existing, defaults);
            if let Some(old) = self.paths.insert(existing, path.clone()) {
                if old != path {
                    self.reverse.remove(&old);
                }
            }
            self.reverse.insert(path, existing);
            return existing;
        }

        let asset = MaterialAssetId(self.next);
        self.next += 1;
        self.ids.insert(id.to_string(), asset);
        self.names.insert(asset, id.to_string());
        self.reverse.insert(path.clone(), asset);
        self.paths.insert(asset, path);
        self.shader_names.insert(asset, shader_name);
        self.defaults.insert(asset, defaults);
        asset
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<MaterialAssetId> {
        self.ids.get(id).copied()
    }

    #[must_use]
    pub fn id_for_path(&self, path: &Path) -> Option<MaterialAssetId> {
        self.reverse.get(path).copied()
    }

    #[must_use]
    pub fn name_for_id(&self, id: MaterialAssetId) -> Option<&str> {
        self.names.get(&id).map(String::as_str)
    }

    #[must_use]
    pub fn path_for_id(&self, id: MaterialAssetId) -> Option<&Path> {
        self.paths.get(&id).map(PathBuf::as_path)
    }

    #[must_use]
    pub fn shader_name_for_id(&self, id: MaterialAssetId) -> Option<&str> {
        self.shader_names.get(&id).map(String::as_str)
    }

    #[must_use]
    pub fn defaults_for_id(&self, id: MaterialAssetId) -> Option<MaterialUniformDefaults> {
        self.defaults.get(&id).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, MaterialAssetId)> {
        self.ids.iter().map(|(k, v)| (k.as_str(), *v))
    }
}

#[cfg(test)]
#[path = "../tests/assets/material.rs"]
mod tests;

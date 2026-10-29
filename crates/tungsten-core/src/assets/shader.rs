//! Shader asset registry: core-side metadata for WGSL asset IDs.
//!
//! D-016 seam: no `wgpu` types here. The renderer mints each `ShaderAssetId`
//! (`Renderer::resolve_or_allocate_shader_id`) and keeps compiled
//! `wgpu::ShaderModule` values under it; the umbrella records the same ID
//! here with the shader's name and path.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Dense WGSL asset handle, minted per session by the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderAssetId(pub u32);

/// Session-local shader id ↔ path ↔ stable-name registry.
#[derive(Debug, Default, Clone)]
pub struct ShaderRegistry {
    ids: HashMap<String, ShaderAssetId>,
    paths: HashMap<ShaderAssetId, PathBuf>,
    reverse: HashMap<PathBuf, ShaderAssetId>,
    names: HashMap<ShaderAssetId, String>,
}

impl ShaderRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records `asset`, the ID the renderer minted for manifest name `id`,
    /// with its path. A later call for the same name rebinds the
    /// reverse-path lookup to the latest canonical path.
    pub fn register(&mut self, id: &str, asset: ShaderAssetId, path: PathBuf) {
        if let Some(old) = self.ids.insert(id.to_string(), asset)
            && old != asset
        {
            self.names.remove(&old);
            if let Some(old_path) = self.paths.remove(&old) {
                self.reverse.remove(&old_path);
            }
        }
        if let Some(old_path) = self.paths.insert(asset, path.clone())
            && old_path != path
        {
            self.reverse.remove(&old_path);
        }
        self.names.insert(asset, id.to_string());
        self.reverse.insert(path, asset);
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<ShaderAssetId> {
        self.ids.get(id).copied()
    }

    #[must_use]
    pub fn id_for_path(&self, path: &Path) -> Option<ShaderAssetId> {
        self.reverse.get(path).copied()
    }

    #[must_use]
    pub fn name_for_id(&self, id: ShaderAssetId) -> Option<&str> {
        self.names.get(&id).map(String::as_str)
    }

    #[must_use]
    pub fn path_for_id(&self, id: ShaderAssetId) -> Option<&Path> {
        self.paths.get(&id).map(PathBuf::as_path)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, ShaderAssetId)> {
        self.ids.iter().map(|(k, v)| (k.as_str(), *v))
    }
}

#[cfg(test)]
#[path = "../tests/assets/shader.rs"]
mod tests;

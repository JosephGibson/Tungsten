//! Shader asset registry: core-side source-of-truth for WGSL asset IDs.
//!
//! D-016 seam: no `wgpu` types here. The render side keeps compiled
//! `wgpu::ShaderModule` values keyed by the same `ShaderAssetId`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Dense WGSL asset handle, minted per session by `ShaderRegistry::allocate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderAssetId(pub u32);

/// Session-local shader id ↔ path ↔ stable-name registry.
#[derive(Debug, Default, Clone)]
pub struct ShaderRegistry {
    next: u32,
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

    /// Allocate (or return existing) id for a manifest name; later calls with
    /// the same id refresh the reverse-path lookup to the latest canonical path.
    pub fn allocate(&mut self, id: &str, path: PathBuf) -> ShaderAssetId {
        if let Some(&existing) = self.ids.get(id) {
            if let Some(old) = self.paths.insert(existing, path.clone()) {
                if old != path {
                    self.reverse.remove(&old);
                }
            }
            self.reverse.insert(path, existing);
            return existing;
        }

        let asset = ShaderAssetId(self.next);
        self.next += 1;
        self.ids.insert(id.to_string(), asset);
        self.names.insert(asset, id.to_string());
        self.reverse.insert(path.clone(), asset);
        self.paths.insert(asset, path);
        asset
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

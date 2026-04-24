//! In-memory WGSL cache with naga validation for body-edit hot reload (M25).
//!
//! D-016 seam: keyed by `ShaderAssetId` (core-owned), holds `wgpu::ShaderModule`
//! (render-owned). Validation runs against `wgpu::naga::front::wgsl` before the
//! module is committed — failures leave the previous `Entry` intact.

use std::collections::HashMap;

use thiserror::Error;
use tungsten_core::assets::ShaderAssetId;
use wgpu::naga::{
    front::wgsl as naga_wgsl,
    valid::{Capabilities, ValidationFlags, Validator},
};

#[derive(Debug, Error)]
pub enum ShaderError {
    #[error("shader '{name}' WGSL parse failed: {report}")]
    Parse { name: String, report: String },
    #[error("shader '{name}' naga validation failed: {report}")]
    Validation { name: String, report: String },
}

#[derive(Debug)]
pub struct Entry {
    pub text: String,
    pub module: wgpu::ShaderModule,
}

/// Session cache of validated WGSL modules.
#[derive(Debug, Default)]
pub struct ShaderModuleCache {
    modules: HashMap<ShaderAssetId, Entry>,
    /// Count of `upload`/`reload` calls that were no-ops because the bytes
    /// matched the cached text. Used by tests + telemetry.
    pub unchanged_count: u64,
}

impl ShaderModuleCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate + create module; insert into cache keyed by `id`.
    /// Byte-equal short-circuit: if existing text matches, this is a no-op.
    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        id: ShaderAssetId,
        name: &str,
        wgsl: String,
    ) -> Result<(), ShaderError> {
        if let Some(entry) = self.modules.get(&id) {
            if entry.text == wgsl {
                self.unchanged_count = self.unchanged_count.saturating_add(1);
                return Ok(());
            }
        }
        let module = compile_and_validate(device, name, &wgsl)?;
        self.modules.insert(id, Entry { text: wgsl, module });
        Ok(())
    }

    /// Validate WGSL and produce a candidate module without touching the cache.
    /// The caller is expected to rebuild dependent pipelines with the returned
    /// module first, then call `commit` to replace the live entry.
    pub fn validate(
        &self,
        device: &wgpu::Device,
        name: &str,
        wgsl: &str,
    ) -> Result<wgpu::ShaderModule, ShaderError> {
        compile_and_validate(device, name, wgsl)
    }

    /// Replace the live entry with a previously validated module. Callers that
    /// staged a pipeline rebuild on the new module use this to flip the cache
    /// only after the rebuild succeeded.
    pub fn commit(&mut self, id: ShaderAssetId, text: String, module: wgpu::ShaderModule) {
        self.modules.insert(id, Entry { text, module });
    }

    /// Same-bytes fast path: returns `true` when the new `wgsl` equals the
    /// cached text. No compile, no validate.
    #[must_use]
    pub fn bytes_equal(&self, id: ShaderAssetId, wgsl: &str) -> bool {
        self.modules.get(&id).is_some_and(|e| e.text == wgsl)
    }

    #[must_use]
    pub fn get(&self, id: ShaderAssetId) -> Option<&wgpu::ShaderModule> {
        self.modules.get(&id).map(|e| &e.module)
    }

    #[must_use]
    pub fn text(&self, id: ShaderAssetId) -> Option<&str> {
        self.modules.get(&id).map(|e| e.text.as_str())
    }

    #[must_use]
    pub fn contains(&self, id: ShaderAssetId) -> bool {
        self.modules.contains_key(&id)
    }
}

fn compile_and_validate(
    device: &wgpu::Device,
    name: &str,
    wgsl: &str,
) -> Result<wgpu::ShaderModule, ShaderError> {
    validate_wgsl_source(name, wgsl)?;
    Ok(device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(name),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Owned(wgsl.to_string())),
    }))
}

/// Device-free WGSL parse + semantic validation. Used by the cache before
/// touching the GPU and directly by tests that have no `wgpu::Device`.
pub fn validate_wgsl_source(name: &str, wgsl: &str) -> Result<(), ShaderError> {
    let module = naga_wgsl::parse_str(wgsl).map_err(|e| ShaderError::Parse {
        name: name.to_string(),
        report: e.to_string(),
    })?;

    Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module)
        .map_err(|e| ShaderError::Validation {
            name: name.to_string(),
            report: format!("{e:?}"),
        })?;
    Ok(())
}

#[cfg(test)]
#[path = "tests/shader_hot_reload.rs"]
mod tests;

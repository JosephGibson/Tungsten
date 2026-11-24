//! The formats and sizes of the frame's targets.
//!
//! The textures themselves belong to the frame's graph ([`crate::graph`]):
//! its declaration names them and its pool lends them, so a frame holds
//! only the targets its passes use. A version of the frame's colour always
//! has the surface's sRGB format, so a capture frame's present blit copies
//! it onto the surface byte-identically (`D-057`); other frames write the
//! surface directly (`D-087`). The scene's depth is `Depth32Float`
//! (portable) and attached under `gpu_depth` alone.
//!
//! While SMAA is on (M27) the colour versions carry a non-sRGB twin view
//! format, so its edge detection samples gamma-encoded values.

use wgpu::TextureFormat;

/// M28 bloom pyramid format. `Rgba16Float` avoids 8-bit sRGB quantization
/// during downsample/upsample even though the source scene stays LDR.
pub const BLOOM_PYRAMID_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

/// Compute the mip count for a viewport-sized bloom pyramid. Mip 0 sits at
/// half resolution, so the natural ceiling is `floor(log2(min(w, h))) - 1`.
/// Returns at least 1 so the pyramid texture always has a mip 0.
#[must_use]
pub fn bloom_mip_count_for_size(width: u32, height: u32, max_mips: u32) -> u32 {
    bloom_mip_count_for_halvings(width, height, max_mips, 1)
}

/// [`bloom_mip_count_for_size`] for a pyramid whose mip 0 is the viewport
/// halved `halvings` times (`D-301`): a quarter's ceiling is one level under
/// a half's.
#[must_use]
pub fn bloom_mip_count_for_halvings(width: u32, height: u32, max_mips: u32, halvings: u32) -> u32 {
    let smallest = width.min(height).max(1);
    let log2 = u32::BITS - smallest.leading_zeros() - 1;
    let size_limit = log2.saturating_sub(halvings.max(1)).max(1);
    max_mips.max(1).min(size_limit)
}

/// The extent of a bloom pyramid's `level` for a viewport halved `halvings`
/// times at mip 0: each level halves the one before, never under one texel.
#[must_use]
pub fn bloom_mip_extent_for(width: u32, height: u32, halvings: u32, level: u32) -> (u32, u32) {
    let shift = halvings + level;
    ((width >> shift).max(1), (height >> shift).max(1))
}

/// Depth format of the scene's depth target; portable across wgpu backends.
pub const DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;

/// SMAA edges target format. 2-channel unorm matches the upstream reference.
pub const SMAA_EDGES_FORMAT: TextureFormat = TextureFormat::Rg8Unorm;
/// SMAA blend-weights target format. 4-channel unorm for the per-pixel weights.
pub const SMAA_BLEND_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;

/// Map an sRGB swapchain format to its non-sRGB twin so SMAA can sample
/// gamma-encoded values via `view_formats`. Returns `None` for already-linear
/// formats; in that case the SMAA read views collapse to the primary view.
#[must_use]
pub fn non_srgb_twin(format: TextureFormat) -> Option<TextureFormat> {
    match format {
        TextureFormat::Rgba8UnormSrgb => Some(TextureFormat::Rgba8Unorm),
        TextureFormat::Bgra8UnormSrgb => Some(TextureFormat::Bgra8Unorm),
        _ => None,
    }
}

/// One value built from the frame's pooled textures and the key it was built
/// for. The key carries the identities of the textures the value holds
/// views of, which the pool never gives twice, so a value is never used
/// with another texture's views.
#[derive(Debug)]
pub(crate) struct TargetCache<K, V> {
    entry: Option<(K, V)>,
}

impl<K, V> Default for TargetCache<K, V> {
    fn default() -> Self {
        Self { entry: None }
    }
}

impl<K: PartialEq, V> TargetCache<K, V> {
    /// The value for `key`: the cached one when its key is equal, else a new
    /// one from `build`, which replaces it.
    pub(crate) fn get_or_build(&mut self, key: K, build: impl FnOnce() -> V) -> &mut V {
        if !matches!(&self.entry, Some((cached, _)) if *cached == key) {
            self.entry = Some((key, build()));
        }
        match &mut self.entry {
            Some((_, value)) => value,
            None => unreachable!("entry was just filled"),
        }
    }

    /// The cached value, whatever its key.
    pub(crate) fn get(&self) -> Option<&V> {
        self.entry.as_ref().map(|(_, value)| value)
    }

    /// Drops the value when `gone` says its key names a texture that is no
    /// more.
    pub(crate) fn clear_if(&mut self, gone: impl FnOnce(&K) -> bool) {
        if matches!(&self.entry, Some((key, _)) if gone(key)) {
            self.entry = None;
        }
    }
}

#[cfg(test)]
#[path = "tests/targets.rs"]
mod tests;

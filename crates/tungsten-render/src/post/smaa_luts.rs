//! SMAA lookup textures (M27).
//!
//! `area.bin` and `search.bin` are the raw byte arrays from the upstream
//! SMAA reference (`Textures/AreaTex.h` / `Textures/SearchTex.h`), included at
//! compile time via `include_bytes!`. They are deliberately NOT entries in
//! `assets/manifest.json` — they are engine-internal content with MIT
//! attribution under `crates/tungsten-render/src/assets/smaa/ATTRIBUTION.md`.

use wgpu::{Extent3d, TextureFormat, TextureUsages};

const AREA_TEX_BYTES: &[u8] = include_bytes!("../assets/smaa/area.bin");
const SEARCH_TEX_BYTES: &[u8] = include_bytes!("../assets/smaa/search.bin");

pub const AREA_TEX_WIDTH: u32 = 160;
pub const AREA_TEX_HEIGHT: u32 = 560;
pub const AREA_TEX_BPP: u32 = 2;
pub const AREA_TEX_LEN: usize =
    (AREA_TEX_WIDTH as usize) * (AREA_TEX_HEIGHT as usize) * (AREA_TEX_BPP as usize);

pub const SEARCH_TEX_WIDTH: u32 = 64;
pub const SEARCH_TEX_HEIGHT: u32 = 16;
pub const SEARCH_TEX_LEN: usize = (SEARCH_TEX_WIDTH as usize) * (SEARCH_TEX_HEIGHT as usize);

#[must_use]
pub fn area_bytes() -> &'static [u8] {
    AREA_TEX_BYTES
}

#[must_use]
pub fn search_bytes() -> &'static [u8] {
    SEARCH_TEX_BYTES
}

/// Upload the SMAA `area` LUT (`Rg8Unorm`, 160 x 560).
#[must_use]
pub fn upload_area(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::Texture, wgpu::TextureView) {
    assert_eq!(
        AREA_TEX_BYTES.len(),
        AREA_TEX_LEN,
        "smaa area.bin byte length mismatch"
    );
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("smaa_area_tex"),
        size: Extent3d {
            width: AREA_TEX_WIDTH,
            height: AREA_TEX_HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TextureFormat::Rg8Unorm,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        AREA_TEX_BYTES,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(AREA_TEX_WIDTH * AREA_TEX_BPP),
            rows_per_image: Some(AREA_TEX_HEIGHT),
        },
        Extent3d {
            width: AREA_TEX_WIDTH,
            height: AREA_TEX_HEIGHT,
            depth_or_array_layers: 1,
        },
    );
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

/// Upload the SMAA `search` LUT (`R8Unorm`, 64 x 16).
#[must_use]
pub fn upload_search(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::Texture, wgpu::TextureView) {
    assert_eq!(
        SEARCH_TEX_BYTES.len(),
        SEARCH_TEX_LEN,
        "smaa search.bin byte length mismatch"
    );
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("smaa_search_tex"),
        size: Extent3d {
            width: SEARCH_TEX_WIDTH,
            height: SEARCH_TEX_HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TextureFormat::R8Unorm,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        SEARCH_TEX_BYTES,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(SEARCH_TEX_WIDTH),
            rows_per_image: Some(SEARCH_TEX_HEIGHT),
        },
        Extent3d {
            width: SEARCH_TEX_WIDTH,
            height: SEARCH_TEX_HEIGHT,
            depth_or_array_layers: 1,
        },
    );
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area_bin_byte_length_matches_format() {
        assert_eq!(area_bytes().len(), AREA_TEX_LEN);
        assert_eq!(AREA_TEX_LEN, 160 * 560 * 2);
    }

    #[test]
    fn search_bin_byte_length_matches_format() {
        assert_eq!(search_bytes().len(), SEARCH_TEX_LEN);
        assert_eq!(SEARCH_TEX_LEN, 64 * 16);
    }
}

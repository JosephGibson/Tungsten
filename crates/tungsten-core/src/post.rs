//! M26 post-processing stack model (core side).
//!
//! `PostPass` is a closed enum of the 17 stock effects — same closed-enum
//! reasoning as `Easing` (`D-054`). Adding an effect is a three-point change:
//! new variant here, new pipeline in `tungsten-render/src/post/`, new entry in
//! the stock roster. The umbrella `PostStack(Vec<PostPass>)` resource is what
//! user code mutates at runtime; the renderer reads it each frame and splices
//! it into the default pass order.

use serde::{Deserialize, Serialize};

/// Tonemapping curve family.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TonemapMode {
    #[default]
    Reinhard,
    AcesApprox,
    AcesFitted,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct TonemapParams {
    pub mode: TonemapMode,
    pub exposure: f32,
    pub white_point: f32,
}

impl Default for TonemapParams {
    fn default() -> Self {
        Self {
            mode: TonemapMode::Reinhard,
            exposure: 1.0,
            white_point: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct VignetteParams {
    pub inner: f32,
    pub outer: f32,
    pub strength: f32,
    pub color: [f32; 4],
}

impl Default for VignetteParams {
    fn default() -> Self {
        Self {
            inner: 0.55,
            outer: 0.95,
            strength: 0.5,
            color: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct LutParams {
    pub mix: f32,
    /// Atlas slot index used to sample the LUT; resolved by the renderer.
    pub lut_sprite_id: u32,
}

impl Default for LutParams {
    fn default() -> Self {
        Self {
            mix: 1.0,
            lut_sprite_id: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct ColorAdjustParams {
    pub hue: f32,
    pub saturation: f32,
    pub contrast: f32,
}

impl Default for ColorAdjustParams {
    fn default() -> Self {
        Self {
            hue: 0.0,
            saturation: 1.0,
            contrast: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToneMonoMode {
    #[default]
    Sepia,
    Mono,
    Duotone,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct ToneMonoParams {
    pub mode: ToneMonoMode,
    pub tint_a: [f32; 4],
    pub tint_b: [f32; 4],
    pub amount: f32,
}

impl Default for ToneMonoParams {
    fn default() -> Self {
        Self {
            mode: ToneMonoMode::Sepia,
            tint_a: [0.9, 0.7, 0.5, 1.0],
            tint_b: [0.2, 0.1, 0.0, 1.0],
            amount: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct CrtParams {
    pub scanline_strength: f32,
    pub curvature: f32,
    pub mask: u32,
    pub bleed: f32,
}

impl Default for CrtParams {
    fn default() -> Self {
        Self {
            scanline_strength: 0.5,
            curvature: 0.05,
            mask: 0,
            bleed: 0.1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct FilmGrainParams {
    pub strength: f32,
    pub time_seed: f32,
}

impl Default for FilmGrainParams {
    fn default() -> Self {
        Self {
            strength: 0.08,
            time_seed: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DitherMode {
    #[default]
    Bayer4,
    Bayer8,
    BlueNoise,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct DitherParams {
    pub mode: DitherMode,
    pub levels: u32,
    pub strength: f32,
}

impl Default for DitherParams {
    fn default() -> Self {
        Self {
            mode: DitherMode::Bayer4,
            levels: 16,
            strength: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct PixelOutlineParams {
    pub color: [f32; 4],
    pub thickness_px: f32,
    pub alpha_threshold: f32,
}

impl Default for PixelOutlineParams {
    fn default() -> Self {
        Self {
            color: [0.0, 0.0, 0.0, 1.0],
            thickness_px: 1.0,
            alpha_threshold: 0.5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct FadeParams {
    pub progress: f32,
    pub color: [f32; 4],
}

impl Default for FadeParams {
    fn default() -> Self {
        Self {
            progress: 0.0,
            color: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct WipeRadialParams {
    pub progress: f32,
    pub center: [f32; 2],
    pub softness: f32,
}

impl Default for WipeRadialParams {
    fn default() -> Self {
        Self {
            progress: 0.0,
            center: [0.5, 0.5],
            softness: 0.05,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct DissolveParams {
    pub progress: f32,
    pub noise_scale: f32,
    pub edge_color: [f32; 4],
}

impl Default for DissolveParams {
    fn default() -> Self {
        Self {
            progress: 0.0,
            noise_scale: 8.0,
            edge_color: [1.0, 0.5, 0.0, 1.0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct GlitchParams {
    pub block_strength: f32,
    pub shift_px: f32,
    pub time_seed: f32,
}

impl Default for GlitchParams {
    fn default() -> Self {
        Self {
            block_strength: 0.2,
            shift_px: 6.0,
            time_seed: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct FogParams {
    pub density: f32,
    pub color: [f32; 4],
    pub height_falloff: f32,
}

impl Default for FogParams {
    fn default() -> Self {
        Self {
            density: 0.2,
            color: [0.6, 0.7, 0.8, 1.0],
            height_falloff: 0.5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct GodRaysParams {
    pub center: [f32; 2],
    pub density: f32,
    pub decay: f32,
    pub weight: f32,
    pub samples: u32,
}

impl Default for GodRaysParams {
    fn default() -> Self {
        Self {
            center: [0.5, 0.5],
            density: 0.94,
            decay: 0.96,
            weight: 0.4,
            samples: 32,
        }
    }
}

/// Closed enum of the 17 stock post-processing passes shipped with M26.
/// Adding a new effect is a four-point change (variant + pipeline + stock
/// WGSL + roster table in the milestone plan). See `D-054` for the closed-enum
/// reasoning that also applies here.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", content = "params", rename_all = "snake_case")]
pub enum PostPass {
    Tonemap(TonemapParams),
    Vignette(VignetteParams),
    Lut(LutParams),
    ChromaticAberration(f32),
    ColorAdjust(ColorAdjustParams),
    ToneMono(ToneMonoParams),
    Crt(CrtParams),
    FilmGrain(FilmGrainParams),
    Dither(DitherParams),
    PixelOutline(PixelOutlineParams),
    Fade(FadeParams),
    WipeRadial(WipeRadialParams),
    Dissolve(DissolveParams),
    Glitch(GlitchParams),
    Pixelate(f32),
    Fog(FogParams),
    GodRays(GodRaysParams),
}

impl PostPass {
    /// Stable kind label for HUDs and logs; does not follow serde naming.
    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Tonemap(_) => "tonemap",
            Self::Vignette(_) => "vignette",
            Self::Lut(_) => "lut",
            Self::ChromaticAberration(_) => "chromatic_aberration",
            Self::ColorAdjust(_) => "color_adjust",
            Self::ToneMono(_) => "tone_mono",
            Self::Crt(_) => "crt",
            Self::FilmGrain(_) => "film_grain",
            Self::Dither(_) => "dither",
            Self::PixelOutline(_) => "pixel_outline",
            Self::Fade(_) => "fade",
            Self::WipeRadial(_) => "wipe_radial",
            Self::Dissolve(_) => "dissolve",
            Self::Glitch(_) => "glitch",
            Self::Pixelate(_) => "pixelate",
            Self::Fog(_) => "fog",
            Self::GodRays(_) => "god_rays",
        }
    }
}

/// Per-session reorderable post-processing stack resource. Default is empty,
/// which keeps the frame byte-identical to the M25 baseline. See M26 plan
/// "Scene → Post → Present Target Flow" for the ping-pong table.
#[derive(Debug, Clone, Default)]
pub struct PostStack(pub Vec<PostPass>);

impl PostStack {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, pass: PostPass) {
        self.0.push(pass);
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Move `from` to `to`. Out-of-range indices are clamped.
    pub fn reorder(&mut self, from: usize, to: usize) {
        if self.0.is_empty() {
            return;
        }
        let from = from.min(self.0.len() - 1);
        let to = to.min(self.0.len() - 1);
        if from == to {
            return;
        }
        let pass = self.0.remove(from);
        self.0.insert(to, pass);
    }

    pub fn as_slice(&self) -> &[PostPass] {
        &self.0
    }

    pub fn as_slice_mut(&mut self) -> &mut [PostPass] {
        &mut self.0
    }
}

#[cfg(test)]
#[path = "tests/post.rs"]
mod tests;

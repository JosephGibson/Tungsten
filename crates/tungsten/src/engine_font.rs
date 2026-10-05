//! The engine's own font (`D-123`): JetBrains Mono Regular, compiled in, so
//! the HUD, the systems overlay and the inspector draw without a manifest
//! font. `assets/fonts/` keeps it beside its SIL Open Font License text.

use tungsten_render::Renderer;

/// Font ID of the engine's embedded JetBrains Mono Regular, the default for
/// the HUD, the systems overlay and the inspector (`D-123`). Registered when
/// the renderer starts, before the manifests; a manifest face with this ID
/// replaces it for engine text.
pub const ENGINE_FONT_ID: &str = "engine_mono";

/// The font file: a copy of the root
/// `assets/fonts/JetBrainsMono/static/JetBrainsMono-Regular.ttf`.
pub(crate) const ENGINE_FONT: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");

/// Registers the engine font with a new renderer, before any manifest font.
pub(crate) fn register(renderer: &mut Renderer) {
    renderer.load_font(ENGINE_FONT_ID, ENGINE_FONT.to_vec());
}

#[cfg(test)]
#[path = "tests/engine_font.rs"]
mod tests;

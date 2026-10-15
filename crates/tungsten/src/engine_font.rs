//! The engine's own font (`D-123`): JetBrains Mono Regular, compiled in, so
//! the HUD, the systems overlay and the inspector draw without a manifest
//! font. `assets/fonts/` keeps it beside its SIL Open Font License text.
//! From M57 it is also a UI font family of the same ID, which the debug
//! views' labels name.

use std::collections::HashMap;

use tungsten_core::assets::ResolvedFontFamily;
use tungsten_render::Renderer;

/// Font ID of the engine's embedded JetBrains Mono Regular, the default for
/// the HUD, the systems overlay and the inspector (`D-123`). Registered when
/// the renderer starts, before the manifests; a manifest face with this ID
/// replaces it for engine text. From M57 also the UI font family over that
/// face that the debug views draw with, which the umbrella adds to the
/// manifest's `font_families` unless they declare the ID themselves.
pub const ENGINE_FONT_ID: &str = "engine_mono";

/// The font file: a copy of the root
/// `assets/fonts/JetBrainsMono/static/JetBrainsMono-Regular.ttf`.
pub(crate) const ENGINE_FONT: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");

/// Registers the engine font with a new renderer, before any manifest font,
/// and its family (M57).
pub(crate) fn register(renderer: &mut Renderer) {
    renderer.load_font(ENGINE_FONT_ID, ENGINE_FONT.to_vec());
    renderer.set_font_families(&with_engine_family(&HashMap::new()), &[]);
}

/// The families the umbrella hands the text engine (M57): `families` with
/// the `engine_mono` family over the engine face added, unless they declare
/// that ID themselves.
pub(crate) fn with_engine_family(
    families: &HashMap<String, ResolvedFontFamily>,
) -> HashMap<String, ResolvedFontFamily> {
    let mut all = families.clone();
    all.entry(ENGINE_FONT_ID.to_string())
        .or_insert_with(|| ResolvedFontFamily::new(vec![ENGINE_FONT_ID.to_string()]));
    all
}

#[cfg(test)]
#[path = "tests/engine_font.rs"]
mod tests;

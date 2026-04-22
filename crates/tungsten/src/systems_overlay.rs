//! System timing overlay (M21, `F2`). Reads `FrameTimings::system_timings`,
//! EWMA-smooths each entry by system name, and renders a sorted table
//! through the existing `glyphon` text pipeline. Stale system names that
//! were not seen this frame are dropped so registration changes do not
//! leave orphaned rows.
//!
//! The overlay is independent of `DebugHud` (`D-044`): toggling the HUD
//! does not disturb this resource and vice versa.

use std::collections::BTreeMap;

use tungsten_core::input::{ActionMap, InputState};
use tungsten_core::World;
use tungsten_render::TextSection;

use crate::debug_hud::{anchor_text_block, DebugHud, HudCorner};
use crate::telemetry::FrameTimings;

#[derive(Debug)]
pub struct SystemTimingOverlay {
    pub enabled: bool,
    pub alpha: f32,
    pub refresh_interval_ms: f32,
    pub corner: HudCorner,
    pub padding_px: f32,
    pub font_id: String,
    pub font_size: f32,
    pub line_height: f32,
    pub color: [u8; 4],
    pub outline_color: [u8; 4],
    pub outline_px: f32,
    ewma: BTreeMap<String, f32>,
    cached_sections: Vec<TextSection>,
    time_since_refresh_ms: f32,
}

impl Default for SystemTimingOverlay {
    fn default() -> Self {
        Self {
            enabled: false,
            alpha: 0.1,
            refresh_interval_ms: 500.0,
            corner: HudCorner::BottomRight,
            padding_px: 12.0,
            font_id: "mono".to_string(),
            font_size: 28.0,
            line_height: 32.0,
            color: [240, 240, 240, 240],
            outline_color: [0, 0, 0, 220],
            outline_px: 1.5,
            ewma: BTreeMap::new(),
            cached_sections: Vec::new(),
            time_since_refresh_ms: f32::INFINITY,
        }
    }
}

impl SystemTimingOverlay {
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
        if !self.enabled {
            self.cached_sections.clear();
            self.time_since_refresh_ms = f32::INFINITY;
        }
    }

    #[cfg(test)]
    pub(crate) fn ewma_for(&self, name: &str) -> Option<f32> {
        self.ewma.get(name).copied()
    }
}

/// Engine system: flips enabled on the `engine_toggle_systems_overlay` edge.
pub(crate) fn systems_overlay_toggle_system(world: &mut World) {
    let pressed = {
        let Some(input) = world.get_resource::<InputState>() else {
            return;
        };
        let Some(actions) = world.get_resource::<ActionMap>() else {
            return;
        };
        actions.just_pressed(input, "engine_toggle_systems_overlay")
    };
    if pressed {
        if let Some(overlay) = world.get_resource_mut::<SystemTimingOverlay>() {
            overlay.toggle();
        }
    }
}

/// Compose helper. Call with the resource removed from the world (mirrors the
/// `DebugHud` borrow dance) so providers can read `FrameTimings` without
/// fighting the resource borrow.
pub(crate) fn compose_systems_overlay_text_section(
    overlay: &mut SystemTimingOverlay,
    world: &World,
    viewport: (u32, u32),
    frame_ms: f32,
) -> Vec<TextSection> {
    if !overlay.enabled {
        overlay.cached_sections.clear();
        overlay.time_since_refresh_ms = f32::INFINITY;
        return Vec::new();
    }

    let Some(ft) = world.get_resource::<FrameTimings>() else {
        return Vec::new();
    };

    let mut seen: std::collections::HashSet<&str> =
        std::collections::HashSet::with_capacity(ft.system_timings.len());
    for (name, ms) in &ft.system_timings {
        seen.insert(name.as_str());
        let entry = overlay.ewma.entry(name.clone()).or_insert(*ms);
        *entry = (1.0 - overlay.alpha) * *entry + overlay.alpha * *ms;
    }
    overlay.ewma.retain(|k, _| seen.contains(k.as_str()));

    overlay.time_since_refresh_ms += frame_ms;
    if overlay.time_since_refresh_ms < overlay.refresh_interval_ms
        && !overlay.cached_sections.is_empty()
    {
        return overlay.cached_sections.clone();
    }
    overlay.time_since_refresh_ms = 0.0;

    if overlay.ewma.is_empty() {
        overlay.cached_sections.clear();
        return Vec::new();
    }

    let mut rows: Vec<(String, f32)> = overlay
        .ewma
        .iter()
        .map(|(name, ms)| (name.clone(), *ms))
        .collect();
    rows.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let rendered: Vec<String> = rows
        .iter()
        .map(|(name, ms)| format!("{name:>30}  {ms:>6.2}ms"))
        .collect();
    let content = rendered.join("\n");
    let (x, mut y) = anchor_text_block(
        overlay.corner,
        &rendered,
        overlay.font_size,
        overlay.line_height,
        overlay.padding_px,
        viewport,
    );
    // When anchored to the top, shift down so the overlay sits directly
    // below the HUD's rendered block rather than colliding with it at the
    // top row. The HUD is re-inserted into the world before this helper
    // runs (see `app.rs`), so its `cached_sections` reflect the current
    // frame.
    if matches!(overlay.corner, HudCorner::TopLeft | HudCorner::TopRight) {
        if let Some(hud) = world.get_resource::<DebugHud>() {
            let hud_bottom = hud.rendered_height_px();
            if hud_bottom > 0.0 {
                y = y.max(hud_bottom + overlay.padding_px);
            }
        }
    }

    let main = TextSection {
        content,
        font_id: overlay.font_id.clone(),
        font_size: overlay.font_size,
        line_height: overlay.line_height,
        color: overlay.color,
        position: [x, y],
        bounds: None,
    };

    let sections = if overlay.outline_px > 0.0 {
        let ox = overlay.outline_px;
        let mut out = Vec::with_capacity(5);
        for (dx, dy) in [(-ox, 0.0), (ox, 0.0), (0.0, -ox), (0.0, ox)] {
            out.push(TextSection {
                content: main.content.clone(),
                font_id: main.font_id.clone(),
                font_size: main.font_size,
                line_height: main.line_height,
                color: overlay.outline_color,
                position: [x + dx, y + dy],
                bounds: None,
            });
        }
        out.push(main);
        out
    } else {
        vec![main]
    };

    overlay.cached_sections = sections.clone();
    sections
}

#[cfg(test)]
#[path = "tests/systems_overlay.rs"]
mod tests;

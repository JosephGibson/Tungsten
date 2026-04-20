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

use crate::telemetry::FrameTimings;

#[derive(Debug)]
pub struct SystemTimingOverlay {
    pub enabled: bool,
    pub alpha: f32,
    pub refresh_interval_ms: f32,
    pub position: [f32; 2],
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
            refresh_interval_ms: 250.0,
            position: [12.0, 12.0],
            font_id: "mono".to_string(),
            font_size: 18.0,
            line_height: 22.0,
            color: [240, 240, 240, 240],
            outline_color: [0, 0, 0, 220],
            outline_px: 1.0,
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
    _viewport: (u32, u32),
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
    let [x, y] = overlay.position;

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
mod tests {
    use super::*;
    use tungsten_core::input::KeyCode;

    fn world_with_timings(systems: &[(&str, f32)]) -> World {
        let mut world = World::new();
        let mut ft = FrameTimings::new();
        ft.system_timings = systems.iter().map(|(n, v)| (n.to_string(), *v)).collect();
        world.insert_resource(ft);
        world
    }

    #[test]
    fn compose_returns_empty_when_disabled() {
        let mut overlay = SystemTimingOverlay::default();
        let world = world_with_timings(&[("a", 1.0)]);
        let out = compose_systems_overlay_text_section(&mut overlay, &world, (1280, 720), 16.0);
        assert!(out.is_empty());
    }

    #[test]
    fn ewma_converges_on_constant_input() {
        let mut overlay = SystemTimingOverlay {
            enabled: true,
            refresh_interval_ms: 0.0,
            ..Default::default()
        };
        let world = world_with_timings(&[("a", 2.0), ("b", 4.0)]);

        for _ in 0..200 {
            let _ = compose_systems_overlay_text_section(&mut overlay, &world, (1280, 720), 16.0);
        }
        let a = overlay.ewma_for("a").unwrap();
        let b = overlay.ewma_for("b").unwrap();
        assert!((a - 2.0).abs() < 0.01, "a = {a}");
        assert!((b - 4.0).abs() < 0.01, "b = {b}");
    }

    #[test]
    fn stale_system_name_is_dropped() {
        let mut overlay = SystemTimingOverlay {
            enabled: true,
            refresh_interval_ms: 0.0,
            ..Default::default()
        };

        let world_a = world_with_timings(&[("alpha", 1.0), ("beta", 1.0)]);
        let _ = compose_systems_overlay_text_section(&mut overlay, &world_a, (1280, 720), 16.0);
        assert!(overlay.ewma_for("alpha").is_some());
        assert!(overlay.ewma_for("beta").is_some());

        let world_b = world_with_timings(&[("alpha", 1.0)]);
        let _ = compose_systems_overlay_text_section(&mut overlay, &world_b, (1280, 720), 16.0);
        assert!(overlay.ewma_for("alpha").is_some());
        assert!(overlay.ewma_for("beta").is_none());
    }

    #[test]
    fn toggle_on_f2_action_flips_enabled() {
        let mut world = World::new();
        let mut input = InputState::new();
        input.key_down(KeyCode::F2);
        world.insert_resource(input);
        world.insert_resource(ActionMap::default_map());
        world.insert_resource(SystemTimingOverlay::default());

        systems_overlay_toggle_system(&mut world);

        assert!(world.get_resource::<SystemTimingOverlay>().unwrap().enabled);
    }
}

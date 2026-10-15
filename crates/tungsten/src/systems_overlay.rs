//! D-044 independent system timing overlay. From M57 a movable window on
//! the core UI (`debug_views`): every system's average time, slowest first,
//! in a scrolling body, refreshed on the frame interval.

use std::collections::BTreeMap;

use tungsten_core::World;
use tungsten_core::input::{ActionMap, InputState};
use tungsten_core::text::TextAlign;
use tungsten_core::ui::WidgetId;

use crate::debug_hud::HudCorner;
use crate::debug_views::{BACKGROUND, Look, Shape, ViewFrame, ViewSpec, ViewWindow, write_rows};
use crate::telemetry::FrameTimings;

/// The overlay's columns: names right-aligned, then times.
const COLUMNS: &[TextAlign] = &[TextAlign::End, TextAlign::Start];

/// The system timing overlay: a window on the UI's debug layer (M57).
#[derive(Debug)]
pub struct SystemTimingOverlay {
    pub enabled: bool,
    /// Each average's weight for a new frame's time.
    pub alpha: f32,
    /// Row refresh interval on the frame interval; the averages still
    /// update every frame.
    pub refresh_interval_ms: f32,
    /// The corner the window opens at when no place is saved for it.
    pub corner: HudCorner,
    /// Space between the window and its corner, in UI units.
    pub padding_px: f32,
    /// The UI font family the window's text takes: the engine font's,
    /// [`ENGINE_FONT_ID`](crate::ENGINE_FONT_ID), by default (M57).
    pub font_id: String,
    /// Text size in UI units.
    pub font_size: f32,
    /// Line height in UI units.
    pub line_height: f32,
    pub color: [u8; 4],
    /// The window's fill (M57).
    pub background: [u8; 4],
    /// The window's border colour; the text outline's before M57.
    pub outline_color: [u8; 4],
    /// The window's border width in UI units; `0.0` draws none.
    pub outline_px: f32,
    ewma: BTreeMap<String, f32>,
    time_since_refresh_ms: f32,
    pub(crate) window: Option<ViewWindow>,
}

impl Default for SystemTimingOverlay {
    fn default() -> Self {
        Self {
            enabled: false,
            alpha: 0.1,
            refresh_interval_ms: 500.0,
            corner: HudCorner::BottomRight,
            padding_px: 12.0,
            font_id: crate::ENGINE_FONT_ID.to_string(),
            font_size: 28.0,
            line_height: 32.0,
            color: [240, 240, 240, 240],
            background: BACKGROUND,
            outline_color: [0, 0, 0, 220],
            outline_px: 1.5,
            ewma: BTreeMap::new(),
            time_since_refresh_ms: f32::INFINITY,
            window: None,
        }
    }
}

impl SystemTimingOverlay {
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    /// The window's root while the overlay is shown (M57).
    #[must_use]
    pub fn window(&self) -> Option<WidgetId> {
        self.window.as_ref().map(|window| window.root)
    }

    #[cfg(test)]
    pub(crate) fn ewma_for(&self, name: &str) -> Option<f32> {
        self.ewma.get(name).copied()
    }

    /// The overlay's frame (M57): every system's average from the previous
    /// frame's `FrameTimings`, and on its interval the rows sorted by it;
    /// while it is shown.
    pub(crate) fn update_window(&mut self, world: &World, frame: &mut ViewFrame<'_>) {
        let spec = ViewSpec {
            name: "systems",
            title: "Systems",
            enabled: self.enabled,
            corner: self.corner,
            padding: self.padding_px,
            look: Look {
                family: self.font_id.clone(),
                size: self.font_size,
                line_height: self.line_height,
                color: self.color,
                background: self.background,
                border: self.outline_color,
                border_width: self.outline_px,
            },
            shape: Shape {
                columns: COLUMNS,
                header: false,
                graph: false,
                scroll: true,
            },
        };
        if !frame.sync(&mut self.window, &spec) {
            self.ewma.clear();
            self.time_since_refresh_ms = f32::INFINITY;
            return;
        }
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let Some(ft) = world.get_resource::<FrameTimings>() else {
            return;
        };
        let alpha = self.alpha;
        for (name, ms) in &ft.system_timings {
            match self.ewma.get_mut(name) {
                Some(average) => *average = (1.0 - alpha) * *average + alpha * *ms,
                None => {
                    self.ewma.insert(name.clone(), *ms);
                }
            }
        }
        if self.ewma.len() != ft.system_timings.len() {
            self.ewma
                .retain(|name, _| ft.system_timings.iter().any(|(seen, _)| seen == name));
        }

        if self.ewma.is_empty() {
            // Nothing to show yet: the first timings refresh at once.
            write_rows(frame.tree, window, &[]);
            self.time_since_refresh_ms = f32::INFINITY;
            return;
        }
        self.time_since_refresh_ms += ft.interval_ms.filter(|ms| ms.is_finite()).unwrap_or(0.0);
        if self.time_since_refresh_ms < self.refresh_interval_ms {
            return;
        }
        self.time_since_refresh_ms = 0.0;
        let mut rows: Vec<(&String, f32)> =
            self.ewma.iter().map(|(name, ms)| (name, *ms)).collect();
        rows.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let cells: Vec<Vec<String>> = rows
            .into_iter()
            .map(|(name, ms)| vec![name.clone(), format!("{ms:>6.2}ms")])
            .collect();
        write_rows(frame.tree, window, &cells);
    }
}

/// Toggle on `engine_toggle_systems_overlay` edge.
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
    if pressed && let Some(overlay) = world.get_resource_mut::<SystemTimingOverlay>() {
        overlay.toggle();
    }
}

#[cfg(test)]
#[path = "tests/systems_overlay.rs"]
mod tests;

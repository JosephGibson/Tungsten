//! D-044 independent system timing overlay. From M57 a movable window on
//! the core UI (`debug_views`): every system's average time, slowest first,
//! in a scrolling body, refreshed on the frame interval. From M58 the
//! systems group by schedule stage in frame order, each stage's name in its
//! hue over its total, with each system's average, its most over the last
//! four refreshes, its share of the frame and a bar of that share, the
//! times and bars on the budget ramp against a tenth of a 60 Hz frame; the
//! title carries the total.

use std::collections::{BTreeMap, VecDeque};

use tungsten_core::input::{ActionMap, InputState};
use tungsten_core::text::TextAlign;
use tungsten_core::ui::WidgetId;
use tungsten_core::{Stage, World};

use crate::debug_hud::HudCorner;
use crate::debug_views::{
    BLUE, BORDER, CYAN, Cell, Column, Header, LABEL, MAGENTA, PANEL, Row, Shape, Tone, VALUE,
    VIOLET, ViewFrame, ViewSpec, ViewWindow, Width, look_of, ramp, toggle_key, write_table,
};
use crate::telemetry::FrameTimings;

/// The overlay's columns: the system, its average and most in ms, its
/// share of the frame, and a bar of that share.
const COLUMNS: &[Column] = &[
    Column::new(TextAlign::Start, Width::Fill, Tone::Value),
    Column::new(TextAlign::End, Width::Auto, Tone::Value),
    Column::new(TextAlign::End, Width::Auto, Tone::Value),
    Column::new(TextAlign::End, Width::Auto, Tone::Value),
    Column::bars(),
];

/// A system's budget, in ms: a tenth of a frame at 60 Hz, so the ramp's
/// bands fall at 1.67, 2.5 and 3.33 ms (M58).
const SYSTEM_BUDGET_MS: f32 = 1000.0 / 600.0;

/// Refreshes a system's most spans: 2 s at 500 ms (M58).
const MAX_REFRESHES: usize = 4;

/// Each system's stage, which `App::resolve_schedule` records from the
/// schedule (M58); without it every system is `other`.
#[derive(Debug, Clone, Default)]
pub(crate) struct SystemStages(pub(crate) BTreeMap<String, Stage>);

/// What the overlay keeps for one system (M58).
#[derive(Debug, Clone)]
struct SystemStat {
    /// The average, weighted by [`SystemTimingOverlay::alpha`].
    average: f32,
    /// The most since the last refresh, and the most of each of the last
    /// [`MAX_REFRESHES`] refreshes, newest last.
    peak: f32,
    maxima: VecDeque<f32>,
}

impl SystemStat {
    fn new(ms: f32) -> Self {
        Self {
            average: ms,
            peak: ms,
            maxima: VecDeque::with_capacity(MAX_REFRESHES),
        }
    }

    /// The most over the last [`MAX_REFRESHES`] refreshes.
    fn most(&self) -> f32 {
        self.maxima.iter().copied().fold(0.0, f32::max)
    }
}

/// A stage's hue in the overlay (M58): `PreUpdate` blue, `FixedUpdate`
/// violet, `Update` cyan, `PostUpdate` magenta; `Startup` and a system the
/// schedule does not list, grey.
fn stage_hue(stage: Option<Stage>) -> [u8; 4] {
    match stage {
        Some(Stage::PreUpdate) => BLUE,
        Some(Stage::FixedUpdate) => VIOLET,
        Some(Stage::Update) => CYAN,
        Some(Stage::PostUpdate) => MAGENTA,
        _ => LABEL,
    }
}

/// The system timing overlay: a window on the UI's debug layer (M57).
#[derive(Debug)]
pub struct SystemTimingOverlay {
    pub enabled: bool,
    /// Each average's weight for a new frame's time.
    pub alpha: f32,
    /// Row refresh interval on the frame interval; the averages still
    /// update every frame.
    pub refresh_interval_ms: f32,
    /// The corner the window docks at when no place is saved for it: the
    /// top right (M58).
    pub corner: HudCorner,
    /// Space between the window and its corner, in UI units.
    pub padding_px: f32,
    /// The UI font family the window's text takes: the engine font's,
    /// [`ENGINE_FONT_ID`](crate::ENGINE_FONT_ID), by default (M57).
    pub font_id: String,
    /// Text size in UI units; 15 (M58), titles two more.
    pub font_size: f32,
    /// Line height in UI units; 20 (M58).
    pub line_height: f32,
    /// The rows' and the title's colour.
    pub color: [u8; 4],
    /// The window's fill (M57).
    pub background: [u8; 4],
    /// The window's border colour; the text outline's before M57; brighter
    /// while the pointer is over the window (M58).
    pub outline_color: [u8; 4],
    /// The window's border width in physical pixels (M58); `0.0` draws
    /// none.
    pub outline_px: f32,
    stats: BTreeMap<String, SystemStat>,
    /// The average frame total, which each share divides by (M58).
    frame_average: Option<f32>,
    time_since_refresh_ms: f32,
    pub(crate) window: Option<ViewWindow>,
}

impl Default for SystemTimingOverlay {
    fn default() -> Self {
        Self {
            enabled: false,
            alpha: 0.1,
            refresh_interval_ms: 500.0,
            corner: HudCorner::TopRight,
            padding_px: 8.0,
            font_id: crate::ENGINE_FONT_ID.to_string(),
            font_size: 15.0,
            line_height: 20.0,
            color: VALUE,
            background: PANEL,
            outline_color: BORDER,
            outline_px: 1.0,
            stats: BTreeMap::new(),
            frame_average: None,
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
        self.stats.get(name).map(|stat| stat.average)
    }

    /// The overlay's frame (M57): every system's average from the previous
    /// frame's `FrameTimings`, and on its interval the rows sorted by it;
    /// while it is shown. From M58 also each system's most and the frame
    /// total's average, and the rows grouped by stage (M58).
    pub(crate) fn update_window(&mut self, world: &World, frame: &mut ViewFrame<'_>) {
        if frame.closes(self.window.as_ref()) {
            self.enabled = false;
        }
        let spec = ViewSpec {
            name: "systems",
            title: "Systems",
            key: toggle_key(world, "engine_toggle_systems_overlay"),
            enabled: self.enabled,
            corner: self.corner,
            padding: self.padding_px,
            strip: false,
            look: look_of!(self),
            shape: Shape {
                columns: COLUMNS,
                header: Header::None,
            },
        };
        if !frame.sync(&mut self.window, &spec) {
            self.stats.clear();
            self.frame_average = None;
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
            match self.stats.get_mut(name) {
                Some(stat) => {
                    stat.average = (1.0 - alpha) * stat.average + alpha * *ms;
                    stat.peak = stat.peak.max(*ms);
                }
                None => {
                    self.stats.insert(name.clone(), SystemStat::new(*ms));
                }
            }
        }
        if self.stats.len() != ft.system_timings.len() {
            self.stats
                .retain(|name, _| ft.system_timings.iter().any(|(seen, _)| seen == name));
        }
        if ft.total_ms.is_finite() {
            self.frame_average = Some(self.frame_average.map_or(ft.total_ms, |average| {
                (1.0 - alpha) * average + alpha * ft.total_ms
            }));
        }

        if self.stats.is_empty() {
            // Nothing to show yet: the first timings refresh at once.
            write_table(frame.tree, window, &[]);
            self.time_since_refresh_ms = f32::INFINITY;
            return;
        }
        self.time_since_refresh_ms += ft.interval_ms.filter(|ms| ms.is_finite()).unwrap_or(0.0);
        if self.time_since_refresh_ms < self.refresh_interval_ms {
            return;
        }
        self.time_since_refresh_ms = 0.0;
        for stat in self.stats.values_mut() {
            if stat.maxima.len() == MAX_REFRESHES {
                stat.maxima.pop_front();
            }
            stat.maxima.push_back(stat.peak);
            stat.peak = 0.0;
        }
        let stages = world.get_resource::<SystemStages>();
        let (rows, total) = self.rows(stages);
        frame
            .tree
            .set_plain_text(window.caption, &format!("Systems \u{b7} {total:.2} ms"));
        write_table(frame.tree, window, &rows);
    }

    /// The table's rows and the sum of the averages (M58): the column
    /// headers, then one group per stage in frame order and `other` last,
    /// each a header row with the stage's name in its hue and its total,
    /// then its systems slowest first, indented.
    fn rows(&self, stages: Option<&SystemStages>) -> (Vec<Row>, f32) {
        let stage_of = |name: &str| stages.and_then(|stages| stages.0.get(name).copied());
        let headers = ["system", "avg ms", "max ms", "% frame", ""]
            .map(|text| Cell::new(text).colored(LABEL));
        let mut rows = vec![Row {
            cells: headers.to_vec(),
            header: true,
            indent: false,
        }];
        let total: f32 = self.stats.values().map(|stat| stat.average).sum();
        let frame = self.frame_average.filter(|ms| *ms > 0.0);
        let groups = Stage::ALL.map(Some).into_iter().chain([None]);
        for stage in groups {
            let mut members: Vec<(&String, &SystemStat)> = self
                .stats
                .iter()
                .filter(|(name, _)| stage_of(name) == stage)
                .collect();
            if members.is_empty() {
                continue;
            }
            members.sort_by(|a, b| b.1.average.total_cmp(&a.1.average));
            let sum: f32 = members.iter().map(|(_, stat)| stat.average).sum();
            let name = stage.map_or("other", Stage::name);
            rows.push(Row {
                cells: vec![
                    Cell::new(name).colored(stage_hue(stage)),
                    Cell::new(format!("{sum:.2}")),
                ],
                header: true,
                indent: false,
            });
            for (name, stat) in members {
                let color = ramp(stat.average, SYSTEM_BUDGET_MS);
                let most = stat.most();
                let share = frame.map(|frame| (stat.average / frame).clamp(0.0, 1.0));
                rows.push(Row {
                    cells: vec![
                        Cell::new(name.as_str()),
                        Cell::new(format!("{:.2}", stat.average)).colored(color),
                        Cell::new(format!("{most:.2}")).colored(ramp(most, SYSTEM_BUDGET_MS)),
                        Cell::new(share.map_or_else(
                            || crate::debug_views::MISSING.to_string(),
                            |share| format!("{:.1}%", share * 100.0),
                        )),
                        Cell::bar(share.unwrap_or(0.0), color),
                    ],
                    header: false,
                    indent: true,
                });
            }
        }
        (rows, total)
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

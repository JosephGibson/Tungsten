//! Runtime developer HUD (M18).
//!
//! `DebugHud` is a world resource that aggregates telemetry rows and owns an
//! ordered list of built-in plus user-registered row providers.
//! `engine_toggle_hud` (default `F4`) toggles visibility through
//! [`hud_toggle_system`], registered by `App::new` as the first system each
//! frame. Default state is off; examples opt in by mutating the resource
//! during setup.
//!
//! Rows are rendered through the existing `glyphon` text pipeline as a single
//! [`TextSection`], composed every frame by [`compose_hud_text_sections`].
//! When disabled the compose helper returns an empty `Vec` without running any
//! provider.

use tungsten_core::camera::{CameraController, CameraMode, CameraState};
use tungsten_core::components::{Tag, Transform};
use tungsten_core::input::{ActionMap, InputState};
use tungsten_core::physics::Velocity;
use tungsten_core::World;
use tungsten_render::TextSection;

use crate::telemetry::{DisplayTelemetry, FrameTimings, RenderCounts};

/// Screen-anchor corner for the HUD block. Right-side corners use a
/// monospace-width heuristic to estimate pixel width without running
/// `glyphon` layout; they are approximate by design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// A single diagnostic row: `"<label>  <value>"`. Providers return a `Vec`
/// because some built-ins emit multiple rows (e.g. top-N slowest systems).
#[derive(Debug, Clone)]
pub struct HudRow {
    pub label: &'static str,
    pub value: String,
}

/// Closure contract for a row provider. Providers read from `World` and
/// return zero or more [`HudRow`]s. An empty `Vec` means "skip"; providers
/// must never panic when a backing resource or entity is absent.
pub type HudRowProvider = Box<dyn Fn(&World) -> Vec<HudRow> + 'static>;

/// Optional placeholder populated by the M20 scene/state stack. M18 only
/// reads this resource; when absent, the `state` row is omitted.
#[derive(Debug, Clone, Default)]
pub struct HudActiveState(pub String);

/// World resource: runtime developer HUD model.
pub struct DebugHud {
    pub enabled: bool,
    pub corner: HudCorner,
    pub font_id: String,
    pub font_size: f32,
    pub line_height: f32,
    pub color: [u8; 4],
    pub padding_px: f32,
    pub top_n_systems: usize,
    /// Minimum wall-clock interval between text refreshes, in milliseconds.
    /// EWMA keeps updating every frame; only the displayed snapshot is
    /// throttled so fast-changing values stay readable. Defaults to 250 ms
    /// (4 Hz). Set to `0.0` to refresh every frame.
    pub refresh_interval_ms: f32,
    fps_ewma: f32,
    frame_ms_ewma: f32,
    ewma_alpha: f32,
    time_since_refresh_ms: f32,
    cached_sections: Vec<TextSection>,
    built_in: Vec<HudRowProvider>,
    custom: Vec<HudRowProvider>,
}

impl DebugHud {
    pub fn new() -> Self {
        let built_in: Vec<HudRowProvider> = vec![
            Box::new(camera_provider),
            Box::new(display_provider),
            Box::new(player_provider),
            Box::new(state_provider),
            Box::new(counts_provider),
        ];
        Self {
            enabled: false,
            corner: HudCorner::TopLeft,
            font_id: "mono".to_string(),
            font_size: 18.0,
            line_height: 24.0,
            color: [230, 230, 230, 230],
            padding_px: 8.0,
            top_n_systems: 3,
            refresh_interval_ms: 250.0,
            fps_ewma: 0.0,
            frame_ms_ewma: 0.0,
            ewma_alpha: 0.1,
            time_since_refresh_ms: f32::INFINITY,
            cached_sections: Vec::new(),
            built_in,
            custom: Vec::new(),
        }
    }

    /// Flip `enabled`.
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    /// Register an additional row provider. Custom rows render after all
    /// built-ins, in registration order.
    pub fn add_row<F>(&mut self, provider: F)
    where
        F: Fn(&World) -> Vec<HudRow> + 'static,
    {
        self.custom.push(Box::new(provider));
    }

    /// Compute the FPS / frame-ms row from the smoothed state. Kept as a
    /// method rather than a free-function provider so the smoothed values
    /// can be read without stashing them in a shadow resource.
    fn fps_row(&self) -> Vec<HudRow> {
        vec![HudRow {
            label: "fps",
            value: format!("{:>4.0}  {:>5.2} ms", self.fps_ewma, self.frame_ms_ewma),
        }]
    }

    /// Compute the top-N slowest systems rows. Kept as a method — and invoked
    /// directly from `compose_hud_text_sections` — so `top_n_systems` is read
    /// from `self` rather than via `world.get_resource::<DebugHud>()`. The
    /// compose borrow-dance removes `DebugHud` from the world for the
    /// duration of compose, so any provider that went looking for it would
    /// silently see `None`.
    fn systems_top_n_row(&self, world: &World) -> Vec<HudRow> {
        let ft = match world.get_resource::<FrameTimings>() {
            Some(ft) => ft,
            None => return Vec::new(),
        };
        if self.top_n_systems == 0 || ft.system_timings.is_empty() {
            return Vec::new();
        }
        let mut sorted = ft.system_timings.clone();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let take = self.top_n_systems.min(sorted.len());
        sorted
            .into_iter()
            .take(take)
            .enumerate()
            .map(|(i, (name, ms))| HudRow {
                label: "sys",
                value: format!("{}:{} {:.2}ms", i, name, ms),
            })
            .collect()
    }
}

impl Default for DebugHud {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Built-in providers
// ---------------------------------------------------------------------------

fn camera_provider(world: &World) -> Vec<HudRow> {
    let state = match world.get_resource::<CameraState>() {
        Some(s) => s,
        None => return Vec::new(),
    };
    let controller = match world.get_resource::<CameraController>() {
        Some(c) => c,
        None => return Vec::new(),
    };
    let mode = match controller.mode {
        CameraMode::Free => "free",
        CameraMode::Follow(_) => "follow",
        CameraMode::Scripted => "scripted",
    };
    vec![HudRow {
        label: "cam",
        value: format!(
            "{} pos=({:.0},{:.0}) zoom={:.2}",
            mode, state.position.x, state.position.y, state.zoom
        ),
    }]
}

fn display_provider(world: &World) -> Vec<HudRow> {
    let dt = match world.get_resource::<DisplayTelemetry>() {
        Some(d) => d,
        None => return Vec::new(),
    };
    vec![HudRow {
        label: "display",
        value: format!(
            "{}x{} {} vsync={}",
            dt.resolution.0,
            dt.resolution.1,
            dt.display_mode.as_str(),
            dt.vsync
        ),
    }]
}

fn player_provider(world: &World) -> Vec<HudRow> {
    let mut matched = None;
    for (entity, tag, transform) in world.query2::<Tag, Transform>() {
        if tag.name == "player" {
            matched = Some((entity, *transform));
            break;
        }
    }
    let (entity, transform) = match matched {
        Some(x) => x,
        None => return Vec::new(),
    };
    let speed = world
        .get::<Velocity>(entity)
        .map(|v| v.0.length())
        .unwrap_or(0.0);
    vec![HudRow {
        label: "player",
        value: format!(
            "pos=({:.0},{:.0}) speed={:.0}",
            transform.position.x, transform.position.y, speed
        ),
    }]
}

fn state_provider(world: &World) -> Vec<HudRow> {
    let s = match world.get_resource::<HudActiveState>() {
        Some(s) => s,
        None => return Vec::new(),
    };
    if s.0.is_empty() {
        return Vec::new();
    }
    vec![HudRow {
        label: "state",
        value: s.0.clone(),
    }]
}

fn counts_provider(world: &World) -> Vec<HudRow> {
    let rc = match world.get_resource::<RenderCounts>() {
        Some(rc) => *rc,
        None => return Vec::new(),
    };
    vec![HudRow {
        label: "counts",
        value: format!("ents={} sprites={}", rc.entities, rc.sprite_instances),
    }]
}

// ---------------------------------------------------------------------------
// Compose / toggle
// ---------------------------------------------------------------------------

/// Called by `App` each frame after the extract stage. Updates the HUD's
/// EWMA smoothing and produces a single `TextSection` holding all active
/// rows. Returns an empty `Vec` when the HUD is disabled, without running
/// any provider.
///
/// The canonical call pattern in `App` is: `remove_resource::<DebugHud>()`,
/// call this helper with `&mut hud` and `&world`, then `insert_resource(hud)`
/// afterwards. That dance is needed because the providers take `&World` and
/// the HUD itself lives as a `World` resource.
pub(crate) fn compose_hud_text_sections(
    hud: &mut DebugHud,
    world: &World,
    viewport: (u32, u32),
    frame_ms: f32,
) -> Vec<TextSection> {
    if !hud.enabled {
        hud.cached_sections.clear();
        hud.time_since_refresh_ms = f32::INFINITY;
        return Vec::new();
    }

    // EWMA smoothing of frame time / fps. Runs every frame so the snapshot
    // captured at the next refresh tick reflects all frames in between.
    hud.frame_ms_ewma = (1.0 - hud.ewma_alpha) * hud.frame_ms_ewma + hud.ewma_alpha * frame_ms;
    hud.fps_ewma = if hud.frame_ms_ewma > 0.0 {
        1000.0 / hud.frame_ms_ewma
    } else {
        0.0
    };

    // Throttle refresh so fast-changing values stay readable. The cached
    // sections are re-emitted between ticks; `time_since_refresh_ms` is
    // seeded to +infinity so the very first call after enable always
    // rebuilds.
    hud.time_since_refresh_ms += frame_ms;
    if hud.time_since_refresh_ms < hud.refresh_interval_ms && !hud.cached_sections.is_empty() {
        return hud.cached_sections.clone();
    }
    hud.time_since_refresh_ms = 0.0;

    let mut rows = hud.fps_row();
    for provider in &hud.built_in {
        rows.extend(provider(world));
    }
    rows.extend(hud.systems_top_n_row(world));
    for provider in &hud.custom {
        rows.extend(provider(world));
    }

    if rows.is_empty() {
        hud.cached_sections.clear();
        return Vec::new();
    }

    let rendered: Vec<String> = rows
        .iter()
        .map(|r| format!("{:>7}  {}", r.label, r.value))
        .collect();
    let content = rendered.join("\n");

    let (vw, vh) = (viewport.0 as f32, viewport.1 as f32);
    let line_count = rendered.len() as f32;
    let pad = hud.padding_px;

    let (x, y) = match hud.corner {
        HudCorner::TopLeft => (pad, pad),
        HudCorner::BottomLeft => (pad, (vh - line_count * hud.line_height - pad).max(0.0)),
        HudCorner::TopRight => {
            let char_w = hud.font_size * 0.55;
            let max_chars = rendered
                .iter()
                .map(|s| s.chars().count())
                .max()
                .unwrap_or(0) as f32;
            let text_w = char_w * max_chars;
            ((vw - text_w - pad).max(0.0), pad)
        }
        HudCorner::BottomRight => {
            let char_w = hud.font_size * 0.55;
            let max_chars = rendered
                .iter()
                .map(|s| s.chars().count())
                .max()
                .unwrap_or(0) as f32;
            let text_w = char_w * max_chars;
            (
                (vw - text_w - pad).max(0.0),
                (vh - line_count * hud.line_height - pad).max(0.0),
            )
        }
    };

    let sections = vec![TextSection {
        content,
        font_id: hud.font_id.clone(),
        font_size: hud.font_size,
        line_height: hud.line_height,
        color: hud.color,
        position: [x, y],
        bounds: None,
    }];
    hud.cached_sections = sections.clone();
    sections
}

/// Engine-registered system: toggles `DebugHud.enabled` on the
/// `engine_toggle_hud` action edge. Runs as the first system each frame so
/// input is observed before any user system consumes `just_pressed`.
pub fn hud_toggle_system(world: &mut World) {
    let pressed = {
        let Some(input) = world.get_resource::<InputState>() else {
            return;
        };
        let Some(actions) = world.get_resource::<ActionMap>() else {
            return;
        };
        actions.just_pressed(input, "engine_toggle_hud")
    };
    if pressed {
        if let Some(hud) = world.get_resource_mut::<DebugHud>() {
            hud.toggle();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;
    use tungsten_core::components::Transform;
    use tungsten_core::input::{InputState, KeyCode};
    use tungsten_core::ActionMap;

    #[test]
    fn default_hud_is_disabled_and_empty_world_yields_no_rows() {
        let mut hud = DebugHud::new();
        assert!(!hud.enabled);
        let world = World::new();
        let sections = compose_hud_text_sections(&mut hud, &world, (1280, 720), 16.67);
        assert!(sections.is_empty());
    }

    #[test]
    fn toggle_flips_enabled() {
        let mut hud = DebugHud::new();
        assert!(!hud.enabled);
        hud.toggle();
        assert!(hud.enabled);
        hud.toggle();
        assert!(!hud.enabled);
    }

    #[test]
    fn action_map_hud_toggle_uses_engine_action() {
        let mut world = World::new();
        let mut input = InputState::new();
        input.key_down(KeyCode::F4);
        world.insert_resource(input);
        world.insert_resource(ActionMap::default_map());
        world.insert_resource(DebugHud::new());

        hud_toggle_system(&mut world);

        assert!(world.get_resource::<DebugHud>().unwrap().enabled);
    }

    #[test]
    fn player_provider_requires_tag_and_transform() {
        let mut world = World::new();
        // Without player tag, no row.
        let e = world.spawn();
        world.insert(e, Transform::from_position(Vec2::new(10.0, 20.0)));
        assert!(player_provider(&world).is_empty());

        // With tag + transform, row appears.
        world.insert(e, Tag::new("player"));
        let rows = player_provider(&world);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label, "player");
        assert!(rows[0].value.contains("pos=(10,20)"));
    }

    #[test]
    fn systems_top_n_respects_cap_and_sorts_desc() {
        let mut world = World::new();
        let mut ft = FrameTimings::new();
        ft.system_timings = vec![
            ("a".into(), 1.0),
            ("b".into(), 5.0),
            ("c".into(), 3.0),
            ("d".into(), 2.0),
        ];
        world.insert_resource(ft);
        let mut hud = DebugHud::new();
        hud.top_n_systems = 2;

        let rows = hud.systems_top_n_row(&world);
        assert_eq!(rows.len(), 2);
        assert!(rows[0].value.starts_with("0:b "));
        assert!(rows[1].value.starts_with("1:c "));
    }

    #[test]
    fn compose_honours_user_top_n_through_borrow_dance() {
        // Regression: `systems_top_n` used to be a free-function provider that
        // read `DebugHud` from the world. During compose, `App` removes the
        // `DebugHud` resource (see `app.rs` borrow dance), so the provider
        // always observed `None` and silently fell back to the hardcoded
        // default of 3. This test pins the fix by running the live compose
        // path with the HUD removed from the world while a non-default
        // `top_n_systems` is in effect.
        let mut world = World::new();
        let mut ft = FrameTimings::new();
        ft.system_timings = vec![
            ("a".into(), 1.0),
            ("b".into(), 5.0),
            ("c".into(), 3.0),
            ("d".into(), 2.0),
            ("e".into(), 4.0),
        ];
        world.insert_resource(ft);

        let mut hud = DebugHud::new();
        hud.enabled = true;
        hud.top_n_systems = 5;
        // Disable refresh throttle so both compose calls rebuild — the
        // assertions below flip `top_n_systems` between calls.
        hud.refresh_interval_ms = 0.0;
        // HUD is intentionally NOT inserted into the world; compose must not
        // depend on `world.get_resource::<DebugHud>()` for configuration.
        let sections = compose_hud_text_sections(&mut hud, &world, (1280, 720), 16.67);
        assert_eq!(sections.len(), 1);
        let content = &sections[0].content;
        // All five system rows must appear (cap=5), not just the default 3.
        for name in ["0:b ", "1:e ", "2:c ", "3:d ", "4:a "] {
            assert!(
                content.contains(name),
                "missing expected system entry {name:?} in\n{content}"
            );
        }

        // And cap=0 must suppress every `sys` row.
        hud.top_n_systems = 0;
        let sections = compose_hud_text_sections(&mut hud, &world, (1280, 720), 16.67);
        assert!(!sections[0].content.contains("sys"));
    }

    #[test]
    fn custom_row_appears_after_built_in() {
        let mut hud = DebugHud::new();
        hud.enabled = true;
        hud.add_row(|_| {
            vec![HudRow {
                label: "extra",
                value: "value".into(),
            }]
        });
        let world = World::new();
        let sections = compose_hud_text_sections(&mut hud, &world, (1280, 720), 16.67);
        assert_eq!(sections.len(), 1);
        let content = &sections[0].content;
        // fps row (built-in, emitted first) must precede the custom "extra" row.
        let fps_idx = content.find("fps").expect("fps row missing");
        let extra_idx = content.find("extra").expect("extra row missing");
        assert!(fps_idx < extra_idx);
    }

    #[test]
    fn refresh_throttle_reuses_cached_sections_between_ticks() {
        let mut hud = DebugHud::new();
        hud.enabled = true;
        hud.refresh_interval_ms = 100.0;
        let world = World::new();

        // First call always rebuilds (cache empty, time seeded to +inf).
        let first = compose_hud_text_sections(&mut hud, &world, (1280, 720), 16.67);
        assert_eq!(first.len(), 1);
        let first_content = first[0].content.clone();

        // Mutate a displayed field; cache should still win because only
        // 16.67 ms has elapsed of the 100 ms interval.
        hud.top_n_systems = 0;
        let second = compose_hud_text_sections(&mut hud, &world, (1280, 720), 16.67);
        assert_eq!(second[0].content, first_content);

        // Advance past the interval with a large frame_ms; rebuild kicks in.
        let third = compose_hud_text_sections(&mut hud, &world, (1280, 720), 200.0);
        assert_ne!(third[0].content, first_content);
    }

    #[test]
    fn compose_is_empty_when_disabled() {
        let mut hud = DebugHud::new();
        hud.enabled = false;
        let world = World::new();
        let sections = compose_hud_text_sections(&mut hud, &world, (1280, 720), 16.67);
        assert!(sections.is_empty());
    }

    #[test]
    fn ewma_converges() {
        let mut hud = DebugHud::new();
        hud.enabled = true;
        let world = World::new();
        for _ in 0..200 {
            let _ = compose_hud_text_sections(&mut hud, &world, (1280, 720), 16.67);
        }
        assert!(
            (hud.frame_ms_ewma - 16.67).abs() < 0.05,
            "frame_ms_ewma did not converge: {}",
            hud.frame_ms_ewma
        );
    }

    #[test]
    fn state_provider_empty_when_string_blank_or_absent() {
        let mut world = World::new();
        assert!(state_provider(&world).is_empty());
        world.insert_resource(HudActiveState(String::new()));
        assert!(state_provider(&world).is_empty());
        world.insert_resource(HudActiveState("menu".into()));
        let rows = state_provider(&world);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].value, "menu");
    }
}

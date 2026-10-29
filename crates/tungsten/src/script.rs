//! Scripted runs (redo 7.2): [`ScriptRunner`] drives a core
//! [`Script`](tungsten_core::script::Script) on the frame body, so the window
//! loop and [`Harness::run_script`](crate::testing::Harness::run_script) run
//! the same steps at the same points:
//!
//! 1. After `Startup` (frame 1 only) and before `PreUpdate`: presses,
//!    releases, the cursor, actions, spawns, scenes, the stage, sheets,
//!    overlays and `ui`, in step order; then the held stage asks for its
//!    states.
//! 2. After `Update`, before `PostUpdate`: each sheet item held at its cell,
//!    so `physics_sync` writes the cell into `Transform`.
//! 3. Before the UI layout: the camera override swapped in, the UI hidden
//!    under `ui: false`, and the frame's captures asked for.
//! 4. After render, before audio: the game's camera and UI put back, then
//!    captures, probes, asserts and `exit` settled, in step order.
//!
//! The stage is the engine's empty state [`SCRIPT_STAGE`]: held from its
//! frame, it pops the game's states down to the bottom and replaces that,
//! on each frame with no state request waiting, so a state the game pushes
//! later goes too; what a stage owns spawns once it alone is on the stack.

use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use glam::{Vec2, Vec3};
use serde_json::json;
use tungsten_core::script::{
    CameraView, CaptureReport, Cursor, Ending, FailedStep, LogReport, Occurrence, ProbeReport,
    Report, Script, ScriptHooks, ScriptRun, SheetKind, SheetStep, SpawnStep, Unreached, Verb,
    select,
};
use tungsten_core::ui::UiTree;
use tungsten_core::{
    AmbientLight, AnimationRegistry, AnimationState, AssetRegistry, CameraState, ComponentRegistry,
    Entity, InputState, ParticleConfigRegistry, ParticleEmitter, ParticleEmitterState, Position,
    PrefabRegistry, PrevPosition, Sprite, SpriteAsset, Transform, Velocity, Visibility, World,
    WorldLabel, WorldLabels,
};

use crate::app::WindowSize;
use crate::asset_loader;
use crate::capture::Capture;
use crate::engine_font::ENGINE_FONT_ID;
use crate::input_bridge;
use crate::logging::LogTally;
use crate::physics_debug::PhysicsDebugOverlay;
use crate::state::{GameState, SceneEntity, StateContext, StateId, StateStack};

/// The engine's empty state a `stage` step holds.
pub(crate) const SCRIPT_STAGE: StateId = "script_stage";

/// A sheet cell's side when no item gives a sprite size and the step gives
/// no `cell`.
const DEFAULT_CELL: f32 = 64.0;

/// A sheet caption's size in UI units.
const LABEL_SIZE: f32 = 12.0;

/// The engine's script hooks: core's probes, then `states` and `log`.
pub(crate) fn engine_hooks() -> ScriptHooks {
    let mut hooks = ScriptHooks::new();
    hooks.add_probe(
        "states",
        "The state stack's IDs, bottom first.",
        |world, _| {
            let ids: Vec<&str> = world
                .get_resource::<StateStack>()
                .map(|stack| stack.stack.iter().map(|state| state.id()).collect())
                .unwrap_or_default();
            Ok(json!(ids))
        },
    );
    hooks.add_probe(
        "log",
        "`{errors, warnings}`: the log's error and warning lines since the script began.",
        |world, _| {
            let (errors, warnings) = world
                .get_resource::<ScriptLog>()
                .map_or((0, 0), ScriptLog::since_start);
            Ok(json!({ "errors": errors, "warnings": warnings }))
        },
    );
    hooks
}

/// The logger's tally, when this App installed the logger, and its counts
/// when the script began.
pub(crate) struct ScriptLog {
    tally: Option<Arc<LogTally>>,
    start: (u32, u32),
}

impl ScriptLog {
    pub(crate) fn new(tally: Option<Arc<LogTally>>) -> Self {
        Self {
            tally,
            start: (0, 0),
        }
    }

    fn mark(&mut self) {
        if let Some(tally) = &self.tally {
            self.start = tally.mark();
        }
    }

    /// Errors and warnings since the mark.
    fn since_start(&self) -> (u32, u32) {
        self.tally.as_ref().map_or((0, 0), |tally| {
            let (errors, warnings) = tally.counts();
            (
                errors.saturating_sub(self.start.0),
                warnings.saturating_sub(self.start.1),
            )
        })
    }

    #[cfg_attr(not(any(test, feature = "testing")), allow(dead_code))]
    fn report(&self) -> LogReport {
        let (errors, warnings) = self.since_start();
        LogReport {
            errors,
            warnings,
            first_errors: self
                .tally
                .as_ref()
                .map(|tally| tally.first_errors())
                .unwrap_or_default(),
        }
    }
}

/// The engine's empty stage: entered once its stack is clear, with the
/// step's ambient light.
struct ScriptStage {
    ambient: Option<[f32; 3]>,
}

impl GameState for ScriptStage {
    fn id(&self) -> StateId {
        SCRIPT_STAGE
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        if let Some(ambient) = self.ambient {
            ctx.world
                .insert_resource(AmbientLight(Vec3::from_array(ambient)));
        }
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, _world: &mut World) {}
}

/// A held `stage`: the step's light, the occurrences that wait for it, and
/// whether the stack has held the stage alone yet.
struct HeldStage {
    ambient: Option<[f32; 3]>,
    occurrences: Vec<usize>,
    reached: bool,
}

/// Work that waits for the stage.
enum Staged {
    Spawn(SpawnStep),
    Scene(String),
    Sheet(SheetStep),
}

/// A sheet item held at its cell.
struct Hold {
    entity: Entity,
    at: Vec2,
}

/// A sheet's grid: `columns` cells of `cell` world pixels, from the world's
/// origin, row by row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SheetLayout {
    pub(crate) columns: usize,
    pub(crate) rows: usize,
    pub(crate) cell: Vec2,
}

impl SheetLayout {
    /// The grid for `count` items: the step's columns, else a near-square
    /// grid; the step's cell, else the largest of `sizes` with room around
    /// it, else [`DEFAULT_CELL`].
    pub(crate) fn new(sheet: &SheetStep, sizes: &[Option<Vec2>]) -> Self {
        let count = sizes.len().max(1);
        let columns = sheet
            .columns
            .map_or_else(|| (count as f32).sqrt().ceil() as usize, |c| c as usize)
            .clamp(1, count);
        let rows = count.div_ceil(columns);
        let largest = sizes.iter().flatten().copied().reduce(Vec2::max);
        let cell = match (sheet.cell, largest) {
            (Some([w, h]), _) => Vec2::new(w, h),
            (None, Some(size)) => (size * 1.25 + Vec2::splat(8.0)).round(),
            (None, None) => Vec2::splat(DEFAULT_CELL),
        };
        Self {
            columns,
            rows,
            cell,
        }
    }

    /// Item `index`'s point: its cell's centre, a little below the middle
    /// to leave its caption room.
    pub(crate) fn center(&self, index: usize) -> Vec2 {
        let (column, row) = (index % self.columns, index / self.columns);
        Vec2::new(
            (column as f32 + 0.5) * self.cell.x,
            (row as f32 + 0.55) * self.cell.y,
        )
    }

    /// The grid's top-left and bottom-right corners.
    pub(crate) fn bounds(&self) -> (Vec2, Vec2) {
        (
            Vec2::ZERO,
            Vec2::new(self.columns as f32, self.rows as f32) * self.cell,
        )
    }
}

/// How a step's occurrence went in phase 1.
enum Applied {
    /// Done.
    Now,
    /// Waits for the stage, or settles after render.
    Later,
}

/// Runs one script on an App's frames, from `App::run_frame`'s four
/// points; `None` there outside a script. Each point is a cold call, so a
/// frame with no script pays four `None` tests and keeps its hot path.
pub(crate) struct ScriptRunner {
    script: Script,
    schedule: Vec<Occurrence>,
    settled: Vec<bool>,
    /// The next frame's first occurrence.
    cursor: usize,
    /// This frame's occurrences.
    current: Range<usize>,
    frame: u32,
    report: Report,
    /// With no renderer, captures are recorded as not written.
    headless: bool,
    stage: Option<HeldStage>,
    waiting: Vec<(usize, Staged)>,
    holds: Vec<Hold>,
    labels: Vec<(String, Vec2)>,
    /// What the stage holds, for `{"fit": "staged"}`: top-left, bottom-right.
    bounds: Option<(Vec2, Vec2)>,
    camera: Option<CameraView>,
    /// The game's camera while the override draws.
    game_camera: Option<CameraState>,
    ui: bool,
    hidden_ui: Option<UiTree>,
    ended: Option<Ending>,
}

impl ScriptRunner {
    /// A runner for `script`, reported as `path`.
    #[cfg_attr(not(any(test, feature = "testing")), allow(dead_code))]
    pub(crate) fn new(script: Script, path: &str, headless: bool) -> Self {
        let schedule = script.schedule();
        Self {
            settled: vec![false; schedule.len()],
            schedule,
            script,
            cursor: 0,
            current: 0..0,
            frame: 0,
            report: Report::new(path),
            headless,
            stage: None,
            waiting: Vec::new(),
            holds: Vec::new(),
            labels: Vec::new(),
            bounds: None,
            camera: None,
            game_camera: None,
            ui: true,
            hidden_ui: None,
            ended: None,
        }
    }

    /// How the script ended, once its last frame or an `exit` has run.
    #[cfg_attr(not(any(test, feature = "testing")), allow(dead_code))]
    pub(crate) fn ended(&self) -> Option<Ending> {
        self.ended
    }

    /// Point 1, after `Startup` and before `PreUpdate`.
    #[cold]
    #[inline(never)]
    pub(crate) fn before_pre_update(&mut self, world: &mut World) {
        if self.frame == 0
            && let Some(log) = world.get_resource_mut::<ScriptLog>()
        {
            log.mark();
        }
        self.frame += 1;
        world.insert_resource(ScriptRun {
            frame: self.frame,
            drawn_camera: None,
        });
        let start = self.cursor;
        while self
            .schedule
            .get(self.cursor)
            .is_some_and(|at| at.frame <= self.frame)
        {
            self.cursor += 1;
        }
        self.current = start..self.cursor;
        for at in self.current.clone() {
            let verb = self.script.steps[self.schedule[at].step].verb.clone();
            match self.apply(world, at, verb) {
                Ok(Applied::Now) => self.settled[at] = true,
                Ok(Applied::Later) => {}
                Err(error) => self.fail(at, error),
            }
        }
        self.hold_stage(world);
    }

    /// Point 2, after `Update` and before `PostUpdate`.
    #[cold]
    #[inline(never)]
    pub(crate) fn after_update(&mut self, world: &mut World) {
        for hold in &self.holds {
            if !world.is_alive(hold.entity) {
                continue;
            }
            let mut physical = false;
            if let Some(position) = world.get_mut::<Position>(hold.entity) {
                position.0 = hold.at;
                physical = true;
            }
            if let Some(prev) = world.get_mut::<PrevPosition>(hold.entity) {
                prev.0 = hold.at;
            }
            if let Some(velocity) = world.get_mut::<Velocity>(hold.entity) {
                velocity.0 = Vec2::ZERO;
            }
            if !physical && let Some(transform) = world.get_mut::<Transform>(hold.entity) {
                transform.position = hold.at;
            }
            // A sheet plays its animations on a loop.
            if let Some(animation) = world.get_mut::<AnimationState>(hold.entity)
                && animation.finished
            {
                animation.frame_index = 0;
                animation.accumulated_ms = 0.0;
                animation.finished = false;
                animation.playing = true;
            }
        }
        if let Some(labels) = world.get_resource_mut::<WorldLabels>() {
            for (text, anchor) in &self.labels {
                labels.push(WorldLabel::new(
                    text.clone(),
                    *anchor,
                    ENGINE_FONT_ID,
                    LABEL_SIZE,
                ));
            }
        }
    }

    /// Point 3, before the UI layout: the override in, the frame's captures
    /// asked for. Returns whether the UI is hidden this frame, so the caller
    /// clears its retained paint list.
    #[cold]
    #[inline(never)]
    pub(crate) fn before_layout(
        &mut self,
        world: &mut World,
        capture: Option<&mut Capture>,
    ) -> bool {
        if let Some(view) = self.camera
            && let Some(drawn) = self.override_camera(world, view)
            && let Some(camera) = world.get_resource_mut::<CameraState>()
        {
            self.game_camera = Some(*camera);
            *camera = drawn;
            if let Some(run) = world.get_resource_mut::<ScriptRun>() {
                run.drawn_camera = Some(drawn);
            }
        }
        if !self.headless
            && let Some(capture) = capture
        {
            for at in self.current.clone() {
                let step = &self.script.steps[self.schedule[at].step];
                if let Some(path) = step.capture_path(self.frame) {
                    capture.add(u64::from(self.frame), path);
                }
            }
        }
        if self.ui {
            return false;
        }
        self.hidden_ui = world.remove_resource::<UiTree>();
        true
    }

    /// Point 4, after render and before audio.
    #[cold]
    #[inline(never)]
    pub(crate) fn after_render(&mut self, world: &mut World, capture: Option<&mut Capture>) {
        self.put_back(world);
        let outcomes = capture.map(Capture::take_outcomes).unwrap_or_default();
        let frame = self.frame;
        for at in self.current.clone() {
            let index = self.schedule[at].step;
            let step = &self.script.steps[index];
            match &step.verb {
                Verb::Capture(_) => {
                    let path = step.capture_path(frame).unwrap_or_default();
                    let error = if self.headless {
                        Some("not written: the harness renders nothing".to_owned())
                    } else {
                        outcomes
                            .iter()
                            .find(|outcome| {
                                outcome.frame == u64::from(frame) && outcome.path == path
                            })
                            .map_or_else(
                                || Some("not written: the frame did not render".to_owned()),
                                |outcome| outcome.error.clone(),
                            )
                    };
                    self.report.captures.push(CaptureReport {
                        frame,
                        path: path.display().to_string(),
                        written: error.is_none(),
                        error,
                    });
                    self.settled[at] = true;
                }
                Verb::Probe(probe) => {
                    let read = ScriptHooks::read_probe(world, &probe.name, &probe.args)
                        .and_then(|value| select(&value, probe.path.as_deref()).cloned());
                    match read {
                        Ok(value) => {
                            self.report.probes.push(ProbeReport {
                                frame,
                                name: probe.name.clone(),
                                path: probe.path.clone(),
                                value,
                            });
                            self.settled[at] = true;
                        }
                        Err(error) => self.fail(at, error),
                    }
                }
                Verb::Assert(assert) => {
                    let probed = ScriptHooks::read_probe(world, &assert.probe, &assert.args);
                    let report = assert.evaluate(frame, probed.as_ref().map_err(String::clone));
                    self.report.asserts.push(report);
                    self.settled[at] = true;
                }
                Verb::Exit => {
                    self.ended = Some(Ending::Exit);
                    self.settled[at] = true;
                }
                _ => {}
            }
        }
        if self.ended.is_none() && frame >= self.script.frames {
            self.ended = Some(Ending::Frames);
        }
    }

    /// The report, with `ended` unless the script ended itself, and every
    /// occurrence the run never settled unreached. The World is left with
    /// the game's camera and UI.
    #[cfg_attr(not(any(test, feature = "testing")), allow(dead_code))]
    pub(crate) fn finish(
        mut self,
        world: &mut World,
        ended: Ending,
        error: Option<String>,
    ) -> Report {
        self.put_back(world);
        let ended = self.ended.unwrap_or(ended);
        for (at, occurrence) in self.schedule.iter().enumerate() {
            if !self.settled[at] {
                self.report.unreached.push(Unreached {
                    frame: occurrence.frame,
                    index: occurrence.step,
                    verb: self.script.steps[occurrence.step].verb.name().to_owned(),
                });
            }
        }
        self.report.frames = self.frame;
        if let Some(log) = world.get_resource::<ScriptLog>() {
            self.report.log = log.report();
        }
        world.remove_resource::<ScriptRun>();
        self.report.finish(ended, error);
        self.report
    }

    /// The game's camera and UI back in the World.
    fn put_back(&mut self, world: &mut World) {
        if let Some(game) = self.game_camera.take()
            && let Some(camera) = world.get_resource_mut::<CameraState>()
        {
            *camera = game;
        }
        if let Some(tree) = self.hidden_ui.take() {
            world.insert_resource(tree);
        }
    }

    fn fail(&mut self, at: usize, error: String) {
        let occurrence = self.schedule[at];
        self.report.failed_steps.push(FailedStep {
            frame: occurrence.frame,
            index: occurrence.step,
            verb: self.script.steps[occurrence.step].verb.name().to_owned(),
            error,
        });
        self.settled[at] = true;
    }

    /// One occurrence's phase-1 work.
    fn apply(&mut self, world: &mut World, at: usize, verb: Verb) -> Result<Applied, String> {
        match verb {
            Verb::Press(action) => {
                let binding = input_bridge::first_binding(world, &action)
                    .ok_or_else(|| format!("action `{action}` has no binding"))?;
                input_bridge::press_binding(input(world)?, binding)?;
            }
            Verb::Release(action) => {
                let binding = input_bridge::first_binding(world, &action)
                    .ok_or_else(|| format!("action `{action}` has no binding"))?;
                input_bridge::release_binding(input(world)?, binding);
            }
            Verb::Cursor(cursor) => {
                let at = match cursor {
                    Cursor::Window([x, y]) => Vec2::new(x, y),
                    Cursor::World(point) => {
                        let camera = world
                            .get_resource::<CameraState>()
                            .copied()
                            .unwrap_or_default();
                        (Vec2::from_array(point) - camera.position) * camera.zoom
                    }
                };
                input(world)?.update_cursor_position(at.x, at.y);
            }
            Verb::Action(action) => ScriptHooks::call_action(world, &action.name, &action.args)?,
            Verb::Spawn(spawn) => return self.stage_or_now(world, at, Staged::Spawn(spawn)),
            Verb::Scene(scene) => return self.stage_or_now(world, at, Staged::Scene(scene)),
            Verb::Stage(stage) => {
                match &mut self.stage {
                    Some(held) if held.reached => return Ok(Applied::Now),
                    Some(held) => held.occurrences.push(at),
                    None => {
                        self.stage = Some(HeldStage {
                            ambient: stage.ambient,
                            occurrences: vec![at],
                            reached: false,
                        });
                    }
                }
                return Ok(Applied::Later);
            }
            Verb::Sheet(sheet) => {
                self.stage.get_or_insert_with(|| HeldStage {
                    ambient: None,
                    occurrences: Vec::new(),
                    reached: false,
                });
                return self.stage_or_now(world, at, Staged::Sheet(sheet));
            }
            Verb::Camera(view) => self.camera = view,
            Verb::Overlay(overlay) => {
                if let Some(on) = overlay.physics {
                    world
                        .get_resource_mut::<PhysicsDebugOverlay>()
                        .ok_or("the World has no PhysicsDebugOverlay")?
                        .enabled = on;
                }
                if let Some(on) = overlay.kit {
                    kit_overlay(world, on)?;
                }
            }
            Verb::Ui(on) => self.ui = on,
            Verb::Capture(_) | Verb::Probe(_) | Verb::Assert(_) | Verb::Exit => {
                return Ok(Applied::Later);
            }
        }
        Ok(Applied::Now)
    }

    /// Runs `work` now when no stage waits, else queues it for the stage.
    fn stage_or_now(
        &mut self,
        world: &mut World,
        at: usize,
        work: Staged,
    ) -> Result<Applied, String> {
        if self.stage.as_ref().is_some_and(|stage| !stage.reached) {
            self.waiting.push((at, work));
            return Ok(Applied::Later);
        }
        self.run_staged(world, work)?;
        Ok(Applied::Now)
    }

    /// The held stage's requests, on a frame with none waiting; once the
    /// stack holds the stage alone, the occurrences that waited settle and
    /// the queued work runs.
    fn hold_stage(&mut self, world: &mut World) {
        let Some(stage) = &mut self.stage else {
            return;
        };
        let alone = match world.get_resource_mut::<StateStack>() {
            None => true,
            Some(stack) if stack.requests_waiting() => false,
            Some(stack) => {
                let depth = stack.stack.len();
                let bottom_is_stage = stack.stack.first().is_some_and(|s| s.id() == SCRIPT_STAGE);
                if depth == 1 && bottom_is_stage {
                    true
                } else {
                    for _ in 1..depth {
                        stack.request_pop();
                    }
                    if !bottom_is_stage {
                        stack.request_replace(ScriptStage {
                            ambient: stage.ambient,
                        });
                    }
                    false
                }
            }
        };
        if !alone || stage.reached {
            return;
        }
        stage.reached = true;
        for at in std::mem::take(&mut stage.occurrences) {
            self.settled[at] = true;
        }
        for (at, work) in std::mem::take(&mut self.waiting) {
            match self.run_staged(world, work) {
                Ok(()) => self.settled[at] = true,
                Err(error) => self.fail(at, error),
            }
        }
    }

    /// Spawns, loads or lays out `work`, owned by the stage once it is
    /// reached, else by the top state.
    fn run_staged(&mut self, world: &mut World, work: Staged) -> Result<(), String> {
        let owner = self.owner(world);
        match work {
            Staged::Spawn(spawn) => {
                let at = Vec2::from_array(spawn.at);
                let prefab = world
                    .prepare_prefab(&spawn.prefab, at)
                    .map_err(|err| err.to_string())?;
                prefab.with((SceneEntity { state_id: owner },)).spawn(world);
                self.grow_bounds(at, at);
            }
            Staged::Scene(path) => {
                let data = asset_loader::load_scene(world, Path::new(&path))
                    .map_err(|err| format!("{err:#}"))?;
                for entry in &data.entities {
                    let at = Vec2::from_array(entry.transform.position);
                    self.grow_bounds(at, at);
                }
                asset_loader::spawn_scene(world, &data, owner);
            }
            Staged::Sheet(sheet) => {
                self.lay_out(world, &sheet, owner)?;
                if self.camera.is_none() {
                    self.camera = Some(CameraView::FitStaged);
                }
            }
        }
        Ok(())
    }

    /// The stage once reached; else the top state, or the stage's ID when
    /// the stack is empty.
    fn owner(&self, world: &World) -> StateId {
        if self.stage.as_ref().is_some_and(|stage| stage.reached) {
            return SCRIPT_STAGE;
        }
        world
            .get_resource::<StateStack>()
            .and_then(StateStack::active_id)
            .unwrap_or(SCRIPT_STAGE)
    }

    /// Spawns a sheet's items in its grid, each held at its cell and
    /// captioned. An item that fails is reported and the rest spawn.
    fn lay_out(
        &mut self,
        world: &mut World,
        sheet: &SheetStep,
        owner: StateId,
    ) -> Result<(), String> {
        let sizes: Vec<Option<Vec2>> = sheet
            .items
            .iter()
            .map(|item| item_size(world, item.kind, &item.id))
            .collect();
        let layout = SheetLayout::new(sheet, &sizes);
        let mut errors = Vec::new();
        for (index, item) in sheet.items.iter().enumerate() {
            let at = layout.center(index);
            let name = format!("{}:{}", item.kind.name(), item.id);
            match spawn_item(world, item.kind, &item.id, at, owner) {
                Ok(entity) => {
                    self.holds.push(Hold { entity, at });
                    if sheet.labels {
                        let half = sizes[index].map_or(layout.cell.y * 0.25, |size| size.y * 0.5);
                        self.labels
                            .push((item.id.clone(), at - Vec2::new(0.0, half + 2.0)));
                    }
                }
                Err(error) => errors.push(format!("{name}: {error}")),
            }
        }
        let (min, max) = layout.bounds();
        self.grow_bounds(min, max);
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }

    fn grow_bounds(&mut self, min: Vec2, max: Vec2) {
        self.bounds = Some(match self.bounds {
            Some((lo, hi)) => (lo.min(min), hi.max(max)),
            None => (min, max),
        });
    }

    /// The camera `view` draws with at the window's size; `None` for a fit
    /// with nothing staged.
    fn override_camera(&self, world: &World, view: CameraView) -> Option<CameraState> {
        let viewport = world
            .get_resource::<WindowSize>()
            .map_or(Vec2::new(1280.0, 720.0), |size| {
                Vec2::new(size.width as f32, size.height as f32)
            });
        let (center, zoom) = match view {
            CameraView::Zoom { center, zoom } => (Vec2::from_array(center), zoom),
            CameraView::ViewHeight {
                center,
                view_height,
            } => (Vec2::from_array(center), viewport.y / view_height),
            CameraView::FitStaged => {
                let (min, max) = self.bounds?;
                let size = (max - min).max(Vec2::ONE);
                let fit = (viewport / size).min_element() * 0.9;
                // Whole steps keep pixel art's pixels even.
                let zoom = if fit >= 1.0 { fit.floor() } else { fit };
                ((min + max) * 0.5, zoom)
            }
        };
        let mut camera = CameraState::new();
        camera.zoom = zoom;
        camera.position = center - viewport / (2.0 * zoom);
        Some(camera)
    }
}

fn input(world: &mut World) -> Result<&mut InputState, String> {
    world
        .get_resource_mut::<InputState>()
        .ok_or_else(|| "the World has no InputState".to_owned())
}

#[cfg(feature = "kit")]
fn kit_overlay(world: &mut World, on: bool) -> Result<(), String> {
    world
        .get_resource_mut::<tungsten_kit::KitDebug>()
        .ok_or("the kit's overlay needs KitDebugPlugin")?
        .set_enabled(on);
    Ok(())
}

#[cfg(not(feature = "kit"))]
fn kit_overlay(_world: &mut World, _on: bool) -> Result<(), String> {
    Err("the kit's overlay needs the umbrella's `kit` feature".to_owned())
}

/// A sheet item's sprite size in world pixels, when it has one.
fn item_size(world: &World, kind: SheetKind, id: &str) -> Option<Vec2> {
    let registry = world.get_resource::<AssetRegistry>()?;
    let size = |sprite: &SpriteAsset| Vec2::new(sprite.width as f32, sprite.height as f32);
    match kind {
        SheetKind::Prefab => {
            let sprite = world
                .get_resource::<PrefabRegistry>()?
                .get(id)?
                .sprite
                .as_ref()?;
            registry.get_sprite(&sprite.asset_id).map(size)
        }
        SheetKind::Sprite => registry.get_sprite(id).map(size),
        SheetKind::Animation => {
            let first = world
                .get_resource::<AnimationRegistry>()?
                .get(id)?
                .frames
                .first()?;
            registry.sprite(first.sprite).map(size)
        }
        SheetKind::Particles => None,
    }
}

/// Spawns one sheet item at `at`, owned by `owner`.
fn spawn_item(
    world: &mut World,
    kind: SheetKind,
    id: &str,
    at: Vec2,
    owner: StateId,
) -> Result<Entity, String> {
    let owned = SceneEntity { state_id: owner };
    let transform = Transform::from_position(at);
    match kind {
        SheetKind::Prefab => Ok(world
            .prepare_prefab(id, at)
            .map_err(|err| err.to_string())?
            .with((owned,))
            .spawn(world)),
        SheetKind::Sprite => {
            let sprite = world
                .get_resource::<AssetRegistry>()
                .filter(|registry| registry.get_sprite(id).is_some())
                .and_then(|registry| registry.sprite_id(id))
                .ok_or_else(|| format!("unknown sprite `{id}`"))?;
            Ok(world.spawn_with((transform, Sprite::new(sprite), Visibility::default(), owned)))
        }
        SheetKind::Animation => {
            let first = world
                .get_resource::<AnimationRegistry>()
                .and_then(|animations| animations.get(id))
                .and_then(|animation| animation.frames.first())
                .map(|frame| frame.sprite)
                .ok_or_else(|| format!("unknown animation `{id}`"))?;
            // The kit's `animation` component ticks it.
            if !world
                .get_resource::<ComponentRegistry>()
                .is_some_and(|registry| registry.contains("animation"))
            {
                return Err("an animation item needs the kit's AnimationPlugin".to_owned());
            }
            Ok(world.spawn_with((
                transform,
                Sprite::new(first),
                Visibility::default(),
                AnimationState::new(id),
                owned,
            )))
        }
        SheetKind::Particles => {
            let config = world
                .get_resource::<ParticleConfigRegistry>()
                .and_then(|configs| configs.id_for_name(id))
                .ok_or_else(|| format!("unknown particle config `{id}`"))?;
            Ok(world.spawn_with((
                transform,
                ParticleEmitter::new(config),
                ParticleEmitterState::default(),
                owned,
            )))
        }
    }
}

#[cfg(test)]
#[path = "tests/script.rs"]
mod tests;

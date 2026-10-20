# Template polish: the shell's look, scaling and flow, the sample's feel and sound, and the template in the player archive

- **status:** done
- **goal:** Polish `templates/basic` in one pass, as the owner listed it on 2026-10-20: a shell with a serif title, numbers in mono, the theme's background behind every screen, scrims, light motion, a UI sized from the window that holds its layout at every UI and text scale, slider readouts, a frame-rate slider, 80 % volumes, window sizes from the monitor, eleven save slots, quit questions and no mute hotkey; a sample with acceleration, shooting at the cursor, a target dummy, two pickups, a camera that frames the play space at 4K, sound effects and two original music loops; and a playable template build in the `tungsten-examples-*` player archive. Five small engine and kit additions carry it: the monitor's size and modes, shipped default volumes, UI scale from the window's height with a floor, a `danger` theme token and acceleration on `top_down_mover`.
- **non-goals:** a milestone number, a roadmap or register stop, a version bump, a release or a commit (the next release ships this pass); the template audit ([w12](1.0/w12-template.md) follow-up 2), which stays its own stop and runs after this pass; `tungsten package` taking over the archive (W14b landed in M70 and stays Linux-only, A33); new enemies, an arena, new sprites or an objective loop; exclusive fullscreen at runtime; a voice pause in the mixer; bus fades; gamepad aiming; any engine change beyond the five additions (§2.1).
- **files to touch:**
  - Step 1: `crates/tungsten-core/src/{display.rs,config.rs,lib.rs}`, `crates/tungsten-core/src/ui/{theme.rs,theme_file.rs}`, `crates/tungsten-core/src/tests/{display.rs,config.rs}`, `crates/tungsten-core/src/tests/ui/theme_file.rs`; `crates/tungsten/src/{settings.rs,app.rs,display.rs,debug_views.rs,lib.rs}`, `crates/tungsten/src/testing/ui.rs`, `crates/tungsten/src/tests/{settings.rs,app.rs,display.rs,debug_views.rs,testing_ui.rs}`; `crates/tungsten-kit/src/motion/top_down.rs`, `crates/tungsten-kit/src/tests/top_down.rs`; `api/{tungsten-core,tungsten,tungsten-kit,tungsten-kit-components}.txt`; `DECISIONS.md`, `docs/DECISION_INDEX.md`; `docs/plans/1.0/w04-api-freeze.md` (break ledger)
  - Step 2: `templates/basic/src/shell/{mod.rs,theme.rs,widgets.rs,title.rs,pause.rs,game_over.rs,confirm.rs,saves.rs,settings.rs,keybindings.rs,toast.rs}`, `templates/basic/src/sample/hud.rs`, `templates/basic/assets/ui/themes/shell.json`, `templates/basic/assets/manifest.json`, `templates/basic/assets/fonts/SourceSerif4/{SourceSerif4-Regular.ttf,LICENSE.txt}` (copies of the root files), `templates/basic/assets/data/cue_rules/labels.json`, `templates/basic/tungsten.json`, `templates/basic/tests/game/{main.rs,shell.rs,saves.rs,settings.rs,keys.rs,fit.rs}`
  - Step 3: `templates/basic/src/shell/{mod.rs,theme.rs,widgets.rs,settings.rs,keybindings.rs,saves.rs,title.rs}`, `templates/basic/tungsten.json`, `templates/basic/tests/game/{fit.rs,settings.rs,keys.rs,main.rs}`
  - Step 4: `templates/basic/src/shell/{mod.rs,saves.rs,title.rs,pause.rs,game_over.rs}`, `templates/basic/src/{game.rs,save.rs}`, `templates/basic/src/sample/{mod.rs,hud.rs}`, `templates/basic/input.json`, `templates/basic/tests/game/{saves.rs,shell.rs,keys.rs,sample.rs,settings.rs,main.rs}`
  - Step 5: `templates/basic/src/sample/{mod.rs,components.rs,hud.rs}`, `templates/basic/src/{save.rs,game.rs}`, `templates/basic/input.json`, `templates/basic/assets/manifest.json`, `templates/basic/assets/prefabs/{player.json,bolt.json,dummy.json,heal_pickup.json,speed_pickup.json}`, `templates/basic/assets/data/{tags.json,cue_rules/labels.json,abilities/shot.json,effects/mend.json,effects/heal.json,effects/swift.json}`, `templates/basic/tests/game/{sample.rs,saves.rs,main.rs}`
  - Step 6: `templates/basic/tools/{sounds.py,README.md}`, `templates/basic/assets/sounds/{ui_move,ui_select,ui_back,shoot,hit,pickup,music_run,music_pause}.wav`, `templates/basic/assets/manifest.json`, `templates/basic/assets/prefabs/{player.json,dummy.json,gem.json,heal_pickup.json,speed_pickup.json}`, `templates/basic/src/shell/{mod.rs,theme.rs}`, `templates/basic/src/sample/mod.rs`, `templates/basic/tests/game/{shell.rs,sample.rs}`, `justfile`
  - Step 7: `.github/workflows/release.yml`, `scripts/release.py`, `scripts/test-release.py`, `tools/launcher/src/main.rs`, `docs/releases.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`
  - Step 8: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `templates/basic/{README.md,AGENTS.md}`, `docs/getting-started.md`, `DESIGN.md`, `docs/LLM_INDEX.md`, `CHANGELOG.md`, `docs/known-issues.md`, `docs/plans/1.0/backlog-1.x.md`, `docs/plans/1.0/w12-template.md`
  - Step 9: `templates/basic/tests/fixtures/title.png` (regenerated); this plan (evidence, `status: done`, then moved to `docs/plans/archive/template-polish.md`)
- **ordered steps:** (1) engine and kit API, then the step-1 API critique; (2) the shell's look and motion; (3) the shell's scaling and settings; (4) saves, quit questions and the seam; (5) the sample's movement, shooting, dummy, pickups, camera and save; (6) sound and music; (7) the template in the player archive; (8) docs and records; (9) whole-pass checks, the title fixture and captures
- **done-when:** every step's evidence row quoted, or the step marked skipped with its reason; step 9's whole-pass checks pass, each one that could not run recorded as not run; `status: done` and the plan archived; the run ends with the changed files, each check's result and the owner's playtest checklist (step 9)

A one-PR detour off the 1.0 roadmap, level C by its engine and kit API ([workflow](1.0/workflow.md) §4). Written 2026-10-20 at `77e014a` on `0.91`. Runs under the [tungsten-milestone skill](../../.claude/skills/tungsten-milestone/SKILL.md) and workflow §4–§6; this plan records only where it differs: no register row and no README owned fields, since it is no roadmap stop; no release step, so the run ends with `status: done` and the archive move; and one plan critique round, at the owner's request, with no second round whatever the first changes.

## Context digest

M69 (0.89) built the template: a shell every game copies unchanged and a sample it deletes, joined through a six-name seam, inside a line budget (`D-181`), on the engine's exit request and frame-rate limit (`D-180`). M70 (0.90) added `tungsten new`, `check` and `package` (`D-182`, `D-183`): `new` embeds every template file, and `just template-check` runs all three on an outside copy at each release. The owner's notes of 2026-10-20 ask for 26 changes across the shell, the sample and the release archive.

The pass stands on the UI tree and its routing (`D-124`, `D-125`, `D-149`), state-owned roots and tracks (`D-163`), scroll panels (`D-165`), the DPI model (`D-127`; `ScaleFactor` is the OS scale times the player's, `D-144`, `D-159`), the player's settings (`D-159`, `D-179`, `D-180`), the kit (`D-172`–`D-177`), buses and voices (`D-179`) and the release archives (`D-072`, `D-120`, `D-122`).

Four decisions land. 184 (engine): the monitor, shipped volumes, the window-relative UI scale with a floor, the `danger` token; it amends `D-127`, `D-159` and `D-179`. 185 (kit): the mover's acceleration; it amends `D-172`. 186 (archive): the template in the examples' archives; it amends `D-072` and `D-182`'s consequence. 187 (template): it amends `D-181` (one font family, the 0.08/0.12 s motion, four slots, six names, the line budget, "no engine or kit code"). `D-180` stands: the close button's last frame and `FrameRateCap` are unchanged.

Next: the template audit reads the result; W12b moves examples 01, 03 and 04 onto the shell; the acceptance game copies it.

## 1. Audit

Read at `77e014a`. Platform facts were read on this Linux machine (KWin on X11, one 3840×2160 monitor, OS scale 1.75 from `Xft.dpi` 168).

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | The window clears to the engine default `[0.05, 0.05, 0.08, 1]`, linear, a slate grey; the template's `tungsten.json` has no `render` section; the theme's `background` `#0b0c10` is linear `[0.00335, 0.00368, 0.00518]` | `crates/tungsten-core/src/config.rs:266-305`, `templates/basic/tungsten.json:1-4`, `templates/basic/assets/ui/themes/shell.json:4`, `crates/tungsten-render/src/renderer.rs:282-288` |
| A2 | The title is a 32-unit label inside the 260-unit menu column, so the name wraps; the version and "Saving is off" sit in the column | `templates/basic/src/shell/title.rs:81-95`, `templates/basic/src/shell/theme.rs:29,38-39` |
| A3 | UI layout divides the window by `os × ui` alone, so a 3840×2160 window at OS 1.0 lays the UI out in 3840×2160 units, a small box mid-screen | `crates/tungsten-core/src/display.rs:111-122`, `crates/tungsten/src/app.rs:1334-1338` |
| A4 | Every push and pop, a confirm included, fades the whole frame to black (0.08 s each way) and blocks every screen for 0.16 s; the new screen slides 8 units over 0.12 s from the midpoint, settling at 0.20 s; a pop takes 0.16 s and slides nothing; a second transition request is refused while one runs | `templates/basic/src/shell/theme.rs:13-18,57-76`, `templates/basic/src/shell/mod.rs:260-268`, `crates/tungsten/src/state.rs:194-203,315-354` |
| A5 | Run changes have no fade: `start_run` and `goto` queue a plain replace and `shell_flow` plain pops, so title to gameplay and back cut at once; item 2's "keep the fade" has nothing to keep | `templates/basic/src/shell/mod.rs:211-258` |
| A6 | A command behind an engine transition applies at the out/in boundary, frames after `flow` sets `first_save`, so the first-frame autosave would capture the old run under the new run's meta: a run fade cannot reuse `request_replace_transition`. `start_run` also inserts the incoming `RunState` at once, so until the swap `capture` pairs the new meta with the old run's entities, and an exit save in that window would write the mix | `crates/tungsten/src/state.rs:315-354`, `templates/basic/src/shell/mod.rs:211-224,246-258,440-452`, `templates/basic/src/save.rs:59-85` |
| A7 | Every screen owns its root with `RootCover::Hide`, so a confirm hides the menu it asks about; there is no scrim, so pause and game over draw over the bare world | `templates/basic/src/shell/{mod.rs:413,title.rs:102,settings.rs:419,saves.rs:207,keybindings.rs:182,confirm.rs:110}`, `crates/tungsten/src/state.rs:79-86` |
| A8 | Keybindings rows sit in a scroll panel capped at 200 units, so the list scrolls with room to spare; its cells are fixed 110-unit buttons | `templates/basic/src/shell/keybindings.rs:27-28,160-176` |
| A9 | Settings uses fixed widths (`WIDE_WIDTH` 520, `CAPTION_WIDTH` 150, `CONTROL_WIDTH` 170) and five tabs in one row; text scale grows text but not those boxes, and an `Auto` main size never shrinks, so at text 2.0 the tab row (about 760 units under the fixed-advance measure) overflows the 520-unit screen | `templates/basic/src/shell/theme.rs:29-33`, `templates/basic/src/shell/widgets.rs:120-187`, `crates/tungsten-core/src/ui/style.rs:9-21`, `crates/tungsten-core/src/ui/layout/taffy.rs:124-131` |
| A10 | `fit.rs` checks focusable boxes against the viewport only, at 731×411 and 1920×1080 units, at UI and text scale 1, and skips a scroll panel's rows | `templates/basic/tests/game/fit.rs:14,17-49` |
| A11 | A scroll panel whose main size is `Auto` shrinks to its parent's limits; no other `Auto` child does, so a card that must fit the viewport has to be the scroll panel itself | `crates/tungsten-core/src/ui/layout/taffy.rs:214-229` |
| A12 | The text scale applies to every game-layer label with no per-label opt-out, so the title must fit one line at text 2.0 in the narrowest viewport | `crates/tungsten-core/src/ui/tree.rs:643-661` |
| A13 | No slider shows its value; the frame-rate limit is a choice over `0, 30, 60, 120, 144` | `templates/basic/src/shell/settings.rs:32-33,74,236-242,289-293` |
| A14 | An unset volume applies as 1.0 and the Audio tab shows 100; startup queues only set volumes; `Config` has no audio section | `crates/tungsten/src/settings.rs:49-54`, `templates/basic/src/shell/settings.rs:219-222`, `crates/tungsten/src/app.rs:261-269`, `crates/tungsten-core/src/config.rs:473-490` |
| A15 | Window size cycles a fixed list ending at 2560×1440, and the engine reads no monitor (only `Fullscreen::Borderless(None)`). This monitor reports modes up to 4096×2160, larger than its current 3840×2160 (`xrandr --query`). winit 0.30.13 on Wayland has no primary monitor, finds the current monitor only once the surface has entered an output, lists the modes the compositor sends (often the current one alone) and reports 0×0 before a current mode | `templates/basic/src/shell/settings.rs:31,281-288`, `crates/tungsten/src/app.rs:2240`, `winit-0.30.13/src/platform_impl/linux/wayland/window/mod.rs:637-651`, `…/wayland/output.rs:48-58,93-101` |
| A16 | `top_down_mover` takes only `speed`: full speed at once, a stop at once | `crates/tungsten-kit/src/motion/top_down.rs:110-137,233-238,280-304` |
| A17 | The kit hash takes inspector rows, so any new `TopDownMover` row in the fixtures moves `0x6fbd_8f24_2114_15a5` | `crates/tungsten/src/tests/kit_recipes/determinism.rs:20-34`, `crates/tungsten-kit/src/motion/top_down.rs:322-329` |
| A18 | The sample is laid out for 1280×720 (`PLAYER_START`, the 640×360 dead zone) with a camera anchored at its top-left and zoom 1, so a 4K window shows the play space in its top-left third | `templates/basic/src/sample/mod.rs:26-45,72-79`, `crates/tungsten-core/src/camera.rs:186-262`, `crates/tungsten/src/camera.rs:51-108` |
| A19 | The theme has nine colour tokens, the theme file refuses unknown keys, and `ColorTokens` is an exhaustive public struct: `danger` needs core and a break-ledger row | `crates/tungsten-core/src/ui/theme.rs:71-105`, `crates/tungsten-core/src/ui/theme_file.rs:156-172,218-231` |
| A20 | The template ships Inter Regular alone and its three font tokens are `sans`; Source Serif 4 and JetBrains Mono are in the root `assets/fonts/`; the engine already compiles in JetBrains Mono Regular and SemiBold as the UI family `engine_mono`, which a theme token and a world label may name | `templates/basic/assets/manifest.json:8-22`, `templates/basic/assets/ui/themes/shell.json:15`, `crates/tungsten/src/engine_font.rs:14-56`, `crates/tungsten/src/asset_loader/mod.rs:92-109`, `crates/tungsten-kit/src/present/labels.rs:16-29` |
| A21 | Four slots; `newest` scans what `read_slots` returns, every slot loaded; the saves screen's rows do not scroll | `templates/basic/src/shell/mod.rs:39-42,299-345`, `templates/basic/src/shell/saves.rs:196-205` |
| A22 | `quit_to_title` asks only when the autosave fails; pause's Quit Game and the title's Quit exit at once; game over's Quit to Title cannot ask, since a dead player captures nothing | `templates/basic/src/shell/mod.rs:376-388`, `templates/basic/src/shell/pause.rs:68-69`, `templates/basic/src/shell/title.rs:141`, `templates/basic/src/shell/game_over.rs:67`, `templates/basic/src/save.rs:59-67` |
| A23 | Continue names no slot or age; the picker names only the difficulty; what a difficulty changes lives in the sample | `templates/basic/src/shell/title.rs:86-91`, `templates/basic/src/sample/mod.rs:47-55` |
| A24 | The mute hotkey spans `input.json`, `BINDABLE_ACTIONS`, `shell_frame`, the HUD prompt, three tests and four docs | `templates/basic/input.json:7`, `templates/basic/src/game.rs:29`, `templates/basic/src/shell/mod.rs:453-455,464-474`, `templates/basic/src/sample/hud.rs:50-54`, `templates/basic/tests/game/{sample.rs:609-638,keys.rs:174-184,shell.rs:444}`, `templates/basic/README.md:16`, `docs/getting-started.md:51,67-77,104`, `DESIGN.md:501` |
| A25 | The labels rule floats damage only on `team.player` wearing `show.damage_numbers`, in `sans` | `templates/basic/assets/data/cue_rules/labels.json:1-7` |
| A26 | The kit composes shooting in data: `aim` from the cursor (`cursor_to_world` maps through the zoomed camera), an `abilities` trigger on an intent slot with a `spawn` payload of a `linear_motion`, `hitbox` and `projectile` prefab; a pickup applies effects through its `picked_up` cue; effects heal and modify stats; `health` without `despawn_on_death` keeps its entity | `crates/tungsten-kit/src/intent/{aim.rs:46-63,cursor.rs:5-35,ability.rs:64-123}`, `crates/tungsten-kit/src/damage/{effects.rs:228-260,health.rs:63-65}`, `crates/tungsten/tests/fixtures/kit/top_down/prefabs/firebolt.json` |
| A27 | Voices fade in, set and stop with fades; the mixer resamples each sound to the device rate at startup; the harness loads sounds and records each frame's audio commands | `crates/tungsten-core/src/audio.rs:133-221`, `crates/tungsten/src/audio.rs:320-345`, `crates/tungsten/src/testing.rs:126,293` |
| A28 | A held arrow steps a slider at 10 steps a second after 0.5 s, which bounds a useful step for a wide range | `crates/tungsten-core/src/ui/route.rs:157-158,1204-1225` |
| A29 | The debug views divide the player's UI scale out of their factor; a window-relative base would grow them unless they divide out the whole base | `crates/tungsten/src/debug_views.rs:1640-1647,1771-1783` |
| A30 | `UiHarness` lays out at `WindowSize / ScaleFactor::get()`, and the template's tests insert `WindowSize` directly | `crates/tungsten/src/testing/ui.rs:101-115,194-198`, `templates/basic/tests/game/fit.rs:18-22` |
| A31 | Only `App::new` and the two display syncs write `WindowSize` | `crates/tungsten/src/app.rs:281`, `crates/tungsten/src/display.rs:115-166` |
| A32 | The release build excludes the template and uploads `example-*` alone; `release.py` packages `examples/*` members over `SHARED_RUNTIME`; the launcher runs its build from the archive root | `.github/workflows/release.yml:146-156,179-189`, `scripts/release.py:75-83,329-347,568-570,844-907`, `tools/launcher/src/main.rs:35-52,129-148` |
| A33 | W14b landed in M70: `tungsten package` writes a Linux-only, one-level game archive without a launcher, and `D-182` keeps `release.yml`, `release.py` and `D-072`'s levels the examples' | `DECISIONS.md:2539`, `tools/cli/src/package.rs` |
| A34 | The CLI embeds every file under `templates/basic` but `target/` and `dist/`, and a test holds that list to git's tracked and untracked, not ignored files: a stray file in the template fails `cargo test -p tungsten-cli` | `tools/cli/build.rs:55-90`, `tools/cli/src/tests/new.rs:131-163` |
| A35 | `just template-check` seeds errors into `gem.json`, `hazard.json` and `labels.json` and expects a 0.1.0 archive, so those names and the version stay | `scripts/template-check.sh:137-158,217-244` |
| A36 | Doc budgets: the template's `AGENTS.md` holds 12,201 of 12,288 bytes (59 of 100 lines), `docs/LLM_INDEX.md` 12,262 of 12,288 | `scripts/check-agent-context.py:22-28`, `wc -c` |
| A37 | `tokei` is installed here (`/usr/bin/tokei`) and counts 1,949 code lines in `src/shell/` and 2,282 in `src/`, against `D-181`'s 2,000 and 2,600 (the owner's Windows estimate read 1,961 and 2,301 at `301f95c`) | `tokei --output json templates/basic/src/shell \| jq .Total.code` |
| A38 | Asset coverage exempts complete font folders only; a README under `assets/sounds/` would be an unlisted asset | `scripts/check-repo.py:29-35,116-125` |
| A39 | Backlog rows the pass answers: the monitor's sizes (71) and a danger token (73); the slot-summary row's trigger, "slot lists grow past four" (74), fires; audio pause (34) stays | `docs/plans/1.0/backlog-1.x.md:34,71,73,74` |
| A40 | The title fixture is 1280×720 at tolerance 0, captured at frame 20 of 24 on the reference machine | `templates/basic/tests/game/visual.rs:21-36` |

## 2. Design

### 2.1 API sketch (step 1)

Additive, behind opt-ins, so a game that sets none behaves as at 0.90.

```rust
// tungsten-core, display.rs. No winit type (D-007).

/// One video mode a monitor offers (decision 184).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct VideoMode {
    /// Its size in physical pixels.
    pub size: Resolution,
    /// Its refresh rate in millihertz: 60 Hz is 60 000.
    pub refresh_millihertz: u32,
}
impl VideoMode {
    #[must_use]
    pub const fn new(size: Resolution, refresh_millihertz: u32) -> Self;
}

/// The monitor the window is on, as the platform reports it: a `World`
/// resource the app inserts once a window exists and replaces when the
/// window moves or its scale changes; absent headless and in the harness.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MonitorInfo {
    /// Its current mode's size in physical pixels: the desktop, which a
    /// window never outgrows.
    pub size: Resolution,
    /// Its current refresh rate, when the platform reports one.
    pub refresh_millihertz: Option<u32>,
    /// Every mode it reports, largest first, each once.
    pub modes: Vec<VideoMode>,
}
impl MonitorInfo {
    /// `None` for a size with a 0 side, as a Wayland output reports before
    /// its first current mode.
    #[must_use]
    pub fn new(size: Resolution, refresh_millihertz: Option<u32>,
               modes: impl IntoIterator<Item = VideoMode>) -> Option<Self>;
    /// Each distinct mode size no wider and no taller than `size`, and
    /// `size` itself, smallest first (by area, then width).
    #[must_use]
    pub fn window_sizes(&self) -> Vec<Resolution>;
}

impl ScaleFactor {
    /// Unchanged signature, now returning a value cached when an input
    /// changes: base × ui, where base is `os`, or with a reference height
    /// and a window size `os.max(window_height / reference)`; then, with a
    /// floor and a window size, at most `window_width / floor.x` and
    /// `window_height / floor.y`.
    pub fn get(&self) -> f32;
    /// Sizes the UI from the window's height (720.0 lays a 2160-pixel
    /// window out at 3); `None`, the default, is the OS scale alone. A
    /// value that is not finite or not positive is ignored.
    pub fn set_reference_height(&mut self, height: Option<f32>);
    pub fn reference_height(&self) -> Option<f32>;
    /// The smallest viewport, in UI units, the factor lays a window out
    /// in; `None` by default. A side not finite or not positive is ignored.
    pub fn set_min_viewport(&mut self, floor: Option<Vec2>);
    pub fn min_viewport(&self) -> Option<Vec2>;
    /// The window's physical size, which the two terms above read; the
    /// app sets it wherever it sets `WindowSize`, `UiHarness` before each
    /// frame and each layout; 0×0, the default, leaves both terms out.
    pub fn set_window_size(&mut self, width: u32, height: u32);
}

// tungsten-core, config.rs

/// The `audio` section of `tungsten.json` (decision 184): the volumes a
/// player who set none hears, each 0 to 1, 1 by default.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct AudioConfig {
    /// `audio.master_volume`.
    pub master_volume: f32,
    /// `audio.bus_volumes`, by bus name; a bus left out is 1.
    pub bus_volumes: HashMap<AudioBus, f32>,
}
impl AudioConfig { #[must_use] pub fn bus_volume(&self, bus: AudioBus) -> f32; }
// `Config` gains `#[serde(default)] pub audio: AudioConfig`. `Config::load`
// refuses a volume that is not finite or not in 0..=1 with
// `ConfigError::InvalidValue` (`audio.master_volume`, `audio.bus_volumes`);
// an unknown key or bus name is a parse error, so `tungsten check` names it.

// tungsten-core, ui/theme.rs and ui/theme_file.rs
pub struct ColorTokens { /* the nine */ pub danger: [u8; 4] }
// Destructive actions and warnings: a Delete button's text, a low-health
// bar. Built-in default [255, 107, 107, 255]; the theme file's
// `colors.danger`. No built-in kind uses it.

// tungsten (umbrella), settings.rs, re-exported at the crate root
/// The volume `tungsten.json` ships for the master (`None`) or `bus`,
/// which an unset setting returns to; 1.0 without `App`'s shipped config.
#[must_use]
pub fn shipped_volume(world: &World, bus: Option<AudioBus>) -> f32;

// tungsten-kit, motion/top_down.rs
impl TopDownMover {
    /// Reaches its speed at `rate` px/s² instead of at once. Panics when
    /// `rate` is not finite and above 0 (`#[track_caller]`).
    #[must_use]
    pub fn with_acceleration(self, rate: f32) -> Self;
    /// Slows to a stop at `rate` px/s² instead of at once; same panic.
    #[must_use]
    pub fn with_deceleration(self, rate: f32) -> Self;
}
```

- **Umbrella wiring.** `apply_settings` sends `settings.x().unwrap_or(shipped)` for the master and each bus; `App::new` queues each resolved volume that is set or shipped below 1. `App::new`, `sync_window_resolution` and `sync_display_state_and_telemetry` call `ScaleFactor::set_window_size`, so the window-relative factor follows resizes with no per-frame work. `MonitorInfo` is built at `resumed` from `current_monitor()`, else the first of `available_monitors()` (Wayland has no primary, A15), and rebuilt on `Moved`, `ScaleFactorChanged`, and `Resized` while none is known; it is removed when no monitor answers or the size is 0×0; each change logs one info line, `Monitor <w>×<h> at <hz> Hz, <n> modes`. The debug views' factor becomes `zoom × step × os / get()`, which equals M58's `zoom × step / ui` when no reference or floor is set, so they keep their OS-relative size whatever a game sets. `UiHarness` sets the window size from `WindowSize` before each frame and each layout.
- **Kit rule.** Each step the target is `direction × speed`. With a direction the velocity moves toward the target by `acceleration × dt`, with none toward 0 by `deceleration × dt`; an unset rate jumps, as now. On a kinematic body the kept velocity is the slid one, so a wall absorbs it. The scene form gains optional `"acceleration"` and `"deceleration"` (each above 0, absent by default); the field descriptions say `= null`. The inspector rows gain `acceleration`, `deceleration` and `velocity` only when a rate is set, so the fixtures, which set none, keep the kit hash.
- **Breaks.** `ColorTokens.danger` breaks a struct literal outside core (none in this tree): one ledger row. Every new core item is promised surface through `tungsten::core` and goes in the ledger's additions for the freeze. `Config` and `ScaleFactor` have private fields, so their additions break nothing.

### 2.2 The shell's look and motion (items 2–6, 8)

- **Theme.** `shell.json` gains `"danger": "#ff6b6b"`: 6.4:1 on `surface` `#15171d`, 7.0:1 on `background`, 5.8:1 on the hovered fill `#1e2129`, from opaque sRGB in linear light. Fonts become `{ "body": "sans", "heading": "serif", "mono": "engine_mono" }`.
- **Fonts.** The manifest gains the face `serif` (`fonts/SourceSerif4/SourceSerif4-Regular.ttf`, 329,796 B, the root static Regular with its `LICENSE.txt`) and the family `serif`. Mono is the engine's `engine_mono` family (A20; S1), so no file is copied.
- **Clear colour.** `templates/basic/tungsten.json` gains `"render": { "clear_color": [0.00335, 0.00368, 0.00518, 1.0] }`, the theme's background in linear light (A1).
- **Title.** The title's root fills the viewport. A centred column holds the title in the heading family (`TITLE_SIZE` 40 on 48), one line for the template's own name at every matrix entry of §2.3, with word wrap kept so a longer game name stays inside its box, above the 260-unit button column. "Version …" and "Saving is off" sit at the bottom-left corner in small muted type (`SMALL` 13 on 18), anchored out of flow.
- **Headings.** `HEADING_SIZE` 26 on 32 becomes 20 on 28 in the heading family. Menus centre theirs (Paused, Game over, Difficulty); the settings, saves and keybindings cards keep theirs at the start.
- **Screens.** Every screen's root becomes a full-viewport scrim, `background` at alpha 184 (72 %), modal, owned with `RootCover::Show`, so the screen below stays drawn and frozen under the new one's scrim (S4). The card is the root's one child, a vertical scroll panel `min(design width, 100 %)` wide with an `Auto` height, so it shrinks to the viewport and scrolls when its content is taller (A11). `widgets::screen` returns both ids; `cancelled` keeps reading the modal root.
- **Danger and mono.** Delete buttons' text is `danger`, and the HUD's health fill turns `danger` at or below a third of max health. Slider values, the frame-rate value, the HUD's counts, save times and playtime take the mono family.
- **Motion.** Measured from the code (A4, A5) and proposed (Q4):

  | Change | Now | Proposed |
  | --- | --- | --- |
  | Push (a menu or a dialog) | 0.08 s to black, the swap, 0.08 s back; an 8-unit slide over 0.12 s from the swap; blocked 0.16 s, settled at 0.20 s | No full-screen effect: the new root fades 0 → 1 and slides 6 units up over 0.10 s, `CubicOut`; it acts on its first frame |
  | Pop | 0.16 s to black and back, blocked 0.16 s | The closing root fades 1 → 0 and slides 6 units down over 0.08 s, then pops; the screen below, already drawn, acts after 0.08 s |
  | Run change (New Game, Continue, Load, Retry, Quit to Title) | An instant replace (A5) | A curtain: a top root in `background` fades in over 0.10 s, the stack changes under it exactly as `shell_flow` does today, then it fades out over 0.14 s; screens act after 0.24 s |

  `goto` and `start_run` store the flow, and `start_run` stages the incoming `RunState` in it rather than inserting it (A6). `shell_flow` applies the flow only once the curtain is opaque: the pops, the replace, then the staged `RunState` and `first_save`. Until then the old run's state, or none from the title, stays whole, so an exit save during the curtain writes the old run or nothing, and one after the swap writes the new run; the first-frame autosave keeps its frame. Toasts stay above the curtain (Q23). One screen change runs at a time: `push`, `pop` and `goto` do nothing while one waits, as the engine's queue refused a second transition (A4). `shell::transitioning` covers the engine's transitions, a closing pop and the curtain, so `idle`, `settle` and `game_over` read one flag.

### 2.3 Scaling and fit (items 7, 13)

- **Policy.** `ShellPlugin::build` sets `ScaleFactor::set_reference_height(Some(720.0))` and `set_min_viewport(Some(Vec2::new(731.0, 411.0)))` from `theme::REFERENCE_HEIGHT` and `theme::MIN_VIEWPORT`. The sample's camera reads the same reference height.
- **OS scale (Q1).** The base is the larger of the OS scale and the window's height over 720. A 4K window is 3 at OS 1.0 or 1.75; a 1280×720 window at OS 1.75 stays 1.75, M69's 731×411 units. The UI never draws smaller than the OS asks, and it fills the same share of any window at least 720 logical pixels tall.
- **Floor (Q2).** The factor never rises past what keeps 731×411 units: UI 300 % in a 1280×720 window lays out at 1.75, and at 3840×2160 at 5.25.
- **Layout rules.** Cards are scroll panels `min(design width, 100 %)` wide. Captions are `Fill` and wrap; controls are `Auto` with a `min_width`; keybinding cells share their row (`Fill`). Settings tabs become a vertical rail beside the rows (S5). No box holding text that grows has a `Px` width unless that text fits it at text 2.0 under the fixed-advance measure.
- **`fit.rs` matrix.** Window, OS, UI and text scale, and the viewport they give:

  | Window | OS | UI | Text | Viewport (units) |
  | --- | --- | --- | --- | --- |
  | 1280×720 | 1.75 | 1.0 | 1.0 | 731×411 |
  | 1280×720 | 1.75 | 3.0 | 2.0 | 731×411 (floor) |
  | 1280×720 | 1.0 | 0.5 | 0.75 | 2560×1440 |
  | 3840×2160 | 1.0 | 1.0 | 1.0 | 1280×720 |
  | 3840×2160 | 1.0 | 3.0 | 2.0 | 731×411 (floor) |
  | 1920×1080 | 1.0 | 0.5 | 2.0 | 2560×1440 |

  At each entry every screen (title, picker, pause, game over, load, save, each settings tab, keybindings, a confirm) passes: every root inside the viewport; every visible widget's box inside its parent's, horizontally always and vertically unless its nearest clipping ancestor is a scroll panel, whose rows scroll; and the title's label one line high.

### 2.4 Settings (items 10–12, 14, 1)

- **Readouts.** Each slider row ends in a mono label, rewritten on every `ValueChanged` (a held scale slider included, which still applies at its release) and at `refresh`: volumes and shake "80%", UI scale "50%" to "300%", text scale "75%" to "200%", the limit "60 fps".
- **Frame-rate limit (Q3).** A slider from 10 to 480 fps at a step of 5, beside an "Unlimited" toggle. Why 5: a held arrow steps 10 times a second (A28), so 60 → 120 takes 1.7 s and the whole range under 10 s; 30, 60, 75, 90, 120, 165, 240, 360 and 480 are exact, and a 144 Hz display takes 145 or vsync. While Unlimited is on, the slider is disabled and `Settings` holds `FrameRateCap::Uncapped`; turning it off writes `Fps(slider)`. Unset shows the shipped cap: the template ships none, so Unlimited is on and the slider rests at 60. A stored cap off the grid or outside 10–480, such as M69's 144 or a hand-edited file, keeps applying as stored: its readout shows it exactly ("144 fps"), the slider sits at the allowed value `set_slider_value` snaps it to, without an event (`crates/tungsten-core/src/ui/control.rs:326-330`), and only a move by the player writes the slider's value.
- **Volumes.** `templates/basic/tungsten.json` ships `"audio": { "master_volume": 0.8, "bus_volumes": { "music": 0.8, "effects": 0.8 } }`. The Audio tab shows `settings.x().unwrap_or(shipped_volume(..))`, and Reset tab returns it to 80 %. Both at 80 %, effects (and music) play at 0.8 × 0.8 = 64 % of full scale. The root `tungsten.json`, the examples', is unchanged.
- **Window sizes.** `MonitorInfo::window_sizes()` filtered to at least 1280×720, the shipped window, ending at the monitor's current size (3840×2160 here, item 14). A monitor reporting one mode adds M69's sizes that fit under it; with no monitor (headless, before the window) the list is M69's (Q11). A size as large as the desktop is the window manager's to place or shrink; the 10 s keep question guards it (Q12).
- **Keybindings.** No height cap: the card scrolls only when its rows need it, and the cells share the row.

### 2.5 Saves, quit and the seam (items 15–18)

- **Slots.** `SLOTS` holds the autosave and `slot_1` to `slot_10`, eleven; `newest` is unchanged in shape; the saves card scrolls, focus scrolling the row into view. A slot that failed to load stays read-only (`D-181`). `read_slots` loads all eleven per refresh (Q20).
- **Quit (item 16).** One question each, never two:
  - pause's Quit to Title autosaves first, then asks "Quit to the title?" (Quit, Stay), adding "Progress since your last save will be lost." when the autosave failed;
  - pause's Quit Game does the same with "Quit the game?", then `request_exit`, whose last frame autosaves again (`D-180`);
  - the title's Quit asks "Quit the game?";
  - game over's Quit to Title asks nothing (the owner's default; a dead run saves nothing);
  - the window's close button is unchanged: one last frame, no question (`D-180`).
- **Continue.** A muted second line, "Slot 3 · 5 min ago" or "Autosave · just now", the age in mono through the saves screen's `ago` (Q19).
- **Difficulty notes.** Each picker button's row carries a muted note from the game through a seventh seam name, `game::difficulty_note(Difficulty) -> String`; the sample answers `format!("{} health", max_health(d))`. The shell names nothing the game decides (Q9).
- **Mute.** Removed everywhere (A24); "Mute when unfocused" stays.

### 2.6 The sample (items 19–23)

Data first: every new entity is a prefab of kit components. Rust only places the camera, spawns the run and saves it.

- **Movement.** `player.json`'s `top_down_mover` gains `"acceleration": 1800, "deceleration": 2400`: 240 px/s in 0.13 s, a stop in 0.10 s.
- **Shooting.**
  - `input.json` gains `"shoot": [{ "kind": "mouse", "button": "left" }]`, with no pad binding, since the aim is the cursor's (Q17); `BINDABLE_ACTIONS` gains `("shoot", "Shoot")`.
  - The player gains `"aim": { "from": "cursor" }`, `action_input`'s `"slots": { "shoot": "shoot" }` and `"abilities": ["shot"]`.
  - `data/abilities/shot.json`: trigger `{ "action": "shoot" }`, cooldown 0.2 s, target `{ "aim": {} }`, a payload that spawns `bolt` at 480 px/s, offset 12.
  - `prefabs/bolt.json`: the player's sprite tinted cyan, `linear_motion`, `hitbox` (circle 4, damage 1, filter `team.dummy`) and `projectile` (range 520).
  - The HUD prompt names Shoot where it named Mute.
- **Dummy.** `prefabs/dummy.json`: the sprite tinted grey, tags `team.dummy`, `health` (max 100), `hurtbox` (circle 10) and `hit_flash`, with a `damaged` cue applying `mend`, an instant heal of 100 (`data/effects/mend.json`), so no run of 1-damage bolts kills it. `labels.json`'s filter drops `team.player` and keeps `show.damage_numbers`, which `sample_frame` sets on the player and the dummy from the option; the label font becomes `engine_mono` (S2).
- **Pickups.** `prefabs/heal_pickup.json` applies `heal`, an instant heal of 2 (`data/effects/heal.json`), on `picked_up` to the other entity; `prefabs/speed_pickup.json` applies `swift`, 4 s of `move_speed` +50 % (`data/effects/swift.json`). Both use the player's sprite tinted green and blue, placed apart from the gems, the hazard's line and the dummy.
- **Camera (item 23).** `sample_frame` sets `CameraController::zoom_multiplier` to the window's height over 720 each frame. `GameplayState::on_enter` places the camera so the play space's centre, (640, 360), is the view's. The dead zone stays 640×360 world pixels.
- **Saves.** Decided: pickups are saved like gems, as `pickups_left`. `SCHEMA` becomes 3; `migrate` from 2 fills every pickup in, and from 1 runs schema 2's step first. The dummy holds no state and respawns fresh. An active `swift` is not saved, as `slowed` is not.

### 2.7 Sound and music (items 19, 24, 25)

- **The script.** `templates/basic/tools/sounds.py`, standard library only, in `sfx.py`'s style (xorshift noise, one-pole filters, oscillators, envelopes), writes 16-bit mono WAV at 22,050 Hz into `assets/sounds/` (Q13). `--check` regenerates in memory and compares every byte, each loop's length and its seam; `--out DIR` writes elsewhere. `tools/README.md` records the provenance: every file is synthesized by the script, original to the template and under its license; nothing comes from the root `assets/sounds/`, whose origin is unrecorded (known issues). It sits outside `assets/` (A38) and mentions no template name, so `tungsten new` rewrites nothing in it.
- **Effects** (the Effects bus): `ui_move`, `ui_select` and `ui_back` (the shell's), `shoot`, `hit` and `pickup`.
- **Music** (the Music bus, looping, `max_voices` 1). `music_run` is an original four-bar chiptune at 140 BPM: two pulse voices, triangle bass, noise drums; a beat is 9,450 samples, so the loop is exactly 151,200. `music_pause` is an original two-bar syncopated breakbeat at 105 BPM: kick, snare with ghost notes, closed and open hats; a beat is 12,600 samples, so the loop is exactly 100,800. Its pattern is written in the script, not transcribed from any recording. Each loop renders a bar more and folds the tail onto its start, so it repeats at the bar line without a click.
- **Size.** All eight files together at most 700,000 bytes.
- **Wiring.**
  - The shell plays `ui_move` on a focus change a navigation press caused, `ui_select` on `Activated` and `ui_back` on `Cancel` and Back.
  - Gameplay sounds are cues in data: `shoot` on the player's `ability_used`, `hit` on the dummy's `damaged`, `pickup` on each gem's and pickup's `picked_up`.
  - The sample owns the music. A run's `on_enter` plays `music_run` faded in over 0.5 s, and its `on_exit` stops it over 0.5 s and stops `music_pause` when it plays: a Quit to Title or a Load from pause pops pause and replaces gameplay in one dispatcher pass, so `sample_frame` never sees gameplay on top again (`crates/tungsten/src/state.rs:281-289`). `sample_frame` watches the stack: when the pause screen comes on top it fades the run's voice to 0 over 0.3 s and starts `music_pause` faded in; when gameplay is on top again it stops the loop over 0.3 s and fades the run's voice back. Settings, saves or a confirm opened from pause keep the loop; game over never starts it, since the trigger is the pause screen, not `on_pause`. The run's voice plays on at 0 under the pause, so it resumes where it has got to, not where it stopped (Q5).

### 2.8 The release archive (item 26)

- **Workflow.** `release.yml` drops `--exclude tungsten-template-basic` and uploads `target/*/<target>/release/tungsten-template-basic*` and `target/*/<target>/release/tungsten_template_basic.pdb` beside the examples, `*.d` still excluded; its comments stop saying the template ships in no archive.
- **`release.py`.** `templates(root)` lists `templates/*` members as `examples(root)` lists `examples/*`, through one member reader. `package` puts each template's builds under `bin/<level>/<binary>` with the examples', a launcher copy named `<binary>` at the archive root, and the template's `tungsten.json`, `input.json` and `assets/` under its own folder, `templates/basic/` (the owner's default). Its debug files join the examples': Linux `bin/<level>/<binary>.debug`, Windows the matched PDB. `shipped_packages()` adds the templates, so the notices cover their crates. README.txt names `./tungsten-template-basic`, says it runs from `templates/basic/`, and names its log folder from that folder's `game.id`: `~/.local/state/tungsten-template-basic/logs`, or `%LOCALAPPDATA%\tungsten-template-basic\logs` on Windows.
- **Launcher.** Standard library only. `const FOLDERS: [(&str, &str); 1] = [("tungsten-template-basic", "templates/basic")]`: a launcher whose file stem is listed runs its build with that folder of the archive as the working directory, any other from the archive root (Q14). `test-release.py` holds the table to `templates(root)`, as `test_levels_match_the_launcher` holds `LEVELS`.
- **Version.** The template keeps its own 0.1.0 (Q15); the archive is named by the engine's tag.
- **Docs.** `docs/releases.md`'s archive contents and launch check name the template; decision 186 records it.

### 2.9 Line budget

Measured at `77e014a`: `src/shell/` 1,949, `src/` 2,282 (A37). Estimated after each step, from M69's rustfmt shapes:

| Step | `src/shell/` | `src/` |
| --- | --- | --- |
| 2: scrims, cards, motion, curtain, title, fonts | +110 → 2,059 | +120 → 2,402 |
| 3: scale policy, readouts, frame-rate row, sizes, rail | +95 → 2,154 | +95 → 2,497 |
| 4: slots, quit questions, Continue, notes, no mute | +30 → 2,184 | +40 → 2,537 |
| 5: camera, dummy, pickups, save schema 3 | 0 → 2,184 | +70 → 2,607 |
| 6: menu sounds, music and the pause loop | +25 → 2,209 | +65 → 2,672 |

Proposed caps, amending `D-181` in decision 187: `src/shell/` 2,400 and `src/` 2,900, about 9 % over the estimates. Data before Rust throughout; past a step's estimate, compact in M69's shapes first.

### 2.10 Suggestions

One line each; the owner vetoes any of them.

- S1. Mono is the engine's `engine_mono` family, the same JetBrains Mono, instead of a 115,152-byte copy in the template.
- S2. Damage numbers float in `engine_mono` too.
- S3. The screen headings take the serif heading family with the title.
- S4. Every screen keeps the one below drawn under its scrim (`RootCover::Show`), so a dialog reads in place over its menu.
- S5. The settings tabs become a vertical rail, so five tabs fit at text 200 % in the 731-unit floor.
- S6. The curtain fades to the theme's `background`, near-black, so it meets the clear colour with no seam.
- S7. The frame-rate readout names the monitor's refresh rate when known ("60 fps · display 60 Hz").
- S8. Releasing a volume slider plays `ui_select` on its bus, so the player hears the new level.

## 3. Steps

Before each step, copy every file it names to the scratchpad, as the skill says. Every code step's done-when includes `cargo fmt --all -- --check` → exit 0.

### Step 1: Engine and kit API

- **Files:** step 1's list in the header.
- **Change:** in order: write decision 184 (amends `D-127`, `D-159`, `D-179`) and decision 185 (amends `D-172`), with their index rows and marker lines, after re-checking the next free ID; core's `VideoMode`, `MonitorInfo` and `ScaleFactor` additions; `AudioConfig` and its validation; `ColorTokens.danger` and the theme file's key; the umbrella's `shipped_volume`, the apply and startup volumes, the `set_window_size` calls, `MonitorInfo` at the four events with its log line, the debug views' factor, `UiHarness`'s sync; the kit's two rates; rustdoc on every new public item; `just api`; the break-ledger row for `ColorTokens.danger` and an additions row for the freeze. Tests, each in the file the step names:
  - core `tests/display.rs`: `monitor_info_drops_repeats_and_sorts_largest_first`, `window_sizes_keep_modes_inside_the_current_size_smallest_first`, `a_monitor_with_a_zero_side_is_none`, `scale_factor_defaults_to_os_times_ui`, `a_reference_height_takes_the_larger_of_os_and_window`, `the_floor_holds_the_factor_down_on_either_axis`, `bad_reference_or_floor_values_are_ignored`;
  - core `tests/config.rs`: `audio_defaults_to_full_volume`, `audio_reads_master_and_bus_volumes`, `audio_refuses_out_of_range_non_finite_unknown_bus_and_unknown_key`;
  - core `tests/ui/theme_file.rs`: `danger_reads_from_colors_and_defaults`;
  - umbrella `tests/settings.rs`: `an_unset_volume_returns_to_the_shipped_one`, `shipped_volume_reads_the_config_and_defaults_to_one`;
  - umbrella `tests/app.rs`: `startup_queues_shipped_volumes_below_one`;
  - umbrella `tests/display.rs`: `window_syncs_set_the_scale_factors_window_size`;
  - umbrella `tests/debug_views.rs`: `debug_views_keep_their_size_under_a_reference_height`;
  - umbrella `tests/testing_ui.rs`: `ui_harness_lays_out_at_the_window_relative_factor`;
  - kit `tests/top_down.rs`: `acceleration_ramps_to_speed`, `deceleration_ramps_to_rest`, `an_unset_rate_is_instant`, `a_turn_accelerates_through_the_new_direction`, `a_lower_speed_stat_is_approached_at_the_acceleration`, `a_wall_absorbs_the_kept_velocity`, `rates_refuse_zero_negative_and_non_finite` (constructor and scene form).
- **Done-when:**
  - `cargo test -p tungsten-core --locked -q` → passes, the new tests among them.
  - `cargo test -p tungsten --locked -q` → passes.
  - `cargo test -p tungsten-kit --locked -q` → passes.
  - `cargo test -p example-05-ui-lab --locked -q` → passes (the UI lab sets no reference height).
  - `git diff --quiet HEAD -- crates/tungsten/src/tests/kit_recipes/` → exit 0, and `cargo test -p tungsten --locked --lib -- tests::kit_recipes` → passes, so `KIT_RECIPES_HASH` holds `0x6fbd_8f24_2114_15a5`.
  - `cargo clippy --workspace --all-targets --locked -- -D warnings` → clean.
  - After `just api`: `git --no-pager diff --unified=0 HEAD -- api/ | rg -c '^-[^-]'` → 0 (exit 1, no line removed); `rg -c 'MonitorInfo|VideoMode|AudioConfig|shipped_volume|set_reference_height|set_min_viewport|set_window_size|with_acceleration|with_deceleration' api/` → a match in `tungsten-core.txt`, `tungsten.txt` and `tungsten-kit.txt`; `rg -n 'acceleration|deceleration' api/tungsten-kit-components.txt` → the two field lines; `just api-check` → passes.
  - `just repo-check` → passes; `rg -n '^## D-18[4] |^## D-18[5] ' DECISIONS.md` → two headings; `rg -n 'Amended by D-18[4]' DECISIONS.md` → under `D-127`, `D-159` and `D-179`; `rg -n 'Amended by D-18[5]' DECISIONS.md` → under `D-172`.
  - X11 probe, from `templates/basic`: `env -u WAYLAND_DISPLAY TUNGSTEN_SMOKE_FRAMES=30 cargo run --quiet 2>&1 | rg -F 'Monitor '` → one line naming 3840×2160, 60 Hz and more than one mode.
  - Wayland probe: start `kwin_wayland --virtual --width 1920 --height 1080 --socket tungsten-wl` in the background, run the same command with `WAYLAND_DISPLAY=tungsten-wl` → one line naming the compositor's output size; stop the compositor. When it cannot start, record the probe as not run with its error line.
- **Moves:** the four `api/` snapshots, by additions.

**Step 1 API critique.** After step 1's done-when, run the skill's command in the background: `critique --raw --repo --force --out <scratchpad>/api-critique.md --from 'git --no-pager diff HEAD -- api/ templates/; git ls-files -o --exclude-standard -z -- api templates | xargs -0 -r -n1 git --no-pager diff --no-index /dev/null'`. Read the 5-hour window first; a reading over 65 % or a Codex failure is recorded in §9 as not run with its reason, and the run carries on. While it runs, draft step 2 in the scratchpad, not the tree. Check each finding against the repository, record it as `C.<n>`, and change step 1 where one holds before step 2 starts.

### Step 2: The shell's look and motion

- **Files:** step 2's list in the header.
- **Change:** §2.2: the theme's `danger` and fonts; the serif face and family; the clear colour; `widgets::screen` as scrim and card, every screen moved onto it, `RootCover::Show`; the title's layout; heading sizes; danger on Delete and the low-health bar; mono numbers; the motion (menu tracks, the closing pop, the curtain, one change at a time); `settle` in `tests/game/main.rs` waits out the closing pop and the curtain through `shell::transitioning`. New tests in `tests/game/shell.rs`: `the_title_is_one_serif_line_above_the_menu`, `pause_and_dialogs_show_over_a_scrim_with_the_screen_below_drawn`, `menu_changes_take_their_times` (a push acts on its first frame; a pop takes ⌈0.08 / dt⌉ frames, ± 1; a run change 0.24 s, ± 1 frame), `a_run_change_fades_through_the_curtain_and_autosaves_the_new_run_once`, `a_close_during_a_run_change_saves_one_run_whole` (Load from pause, then `Harness::request_close` before the swap: the autosave holds the old run's meta and player; after the swap: the loaded run's), `one_screen_change_at_a_time` (Escape and a focus loss in one frame open one pause), `delete_buttons_and_low_health_use_danger`; `the_theme_meets_contrast_and_has_square_corners` extended to `danger` on `surface`, `background` and the hovered fill.
- **Done-when:**
  - From `templates/basic`: `cargo test --locked -q` → passes, the M69 tests unchanged in what they assert; `cargo clippy --all-targets --locked -- -D warnings` → clean; `cargo run --quiet -p tungsten-cli -- check` → `tungsten check: no errors`.
  - `cargo test -p tungsten-cli --locked -q` → passes (the embedded template is the tracked one).
  - `just repo-check` → passes.
  - `tokei --output json templates/basic/src/shell | jq .Total.code` → recorded against §2.9.
- **Moves:** none (the title fixture moves in step 9).

### Step 3: The shell's scaling and settings

- **Files:** step 3's list in the header.
- **Change:** §2.3 and §2.4: the scale policy in `ShellPlugin::build`; card widths, `Fill` captions, `Auto` controls with minimums, the tab rail, keybindings without a cap; readouts; the frame-rate slider and toggle; the shipped volumes; the window sizes from `MonitorInfo`. Tests:
  - `tests/game/fit.rs`: `every_screen_fits_at_the_scale_extremes` (§2.3's matrix and rules) and `the_keybindings_card_scrolls_only_when_it_must`, replacing `the_keybindings_screen_fits_and_its_list_scrolls_at_411_units`;
  - `tests/game/settings.rs`: `ui_scale_follows_the_window_height_with_a_floor`, `every_slider_shows_its_value_as_it_moves`, `the_frame_rate_limit_is_a_slider_and_an_unlimited_toggle` (replacing `the_frame_rate_limit_writes_settings_and_the_display_request`, with a restart reading the cap back as M69 did), `an_off_grid_cap_keeps_its_value_until_moved` (a settings file seeded with 144: the readout says "144 fps", the cap stays 144 across a restart, and one step writes a grid value), `volumes_ship_at_eighty_percent_and_reset_to_it`, `window_sizes_follow_the_monitor` (an X11-like monitor with modes past its size, a one-mode monitor, none).
- **Done-when:** step 2's four checks again → the same results.
- **Moves:** none.

### Step 4: Saves, quit questions and the seam

- **Files:** step 4's list in the header.
- **Change:** §2.5. Tests:
  - `tests/game/saves.rs`: `the_load_screen_lists_the_autosave_and_ten_slots_and_scrolls`, replacing `the_load_screen_lists_the_autosave_and_three_slots_with_summaries`, and `continue_loads_the_newest_of_eleven_slots`, replacing `continue_loads_the_newest_slot`; `a_slot_that_fails_to_load_is_never_overwritten` gains a `slot_10` case;
  - `tests/game/shell.rs`: `continue_names_its_slot_and_age`, `the_picker_notes_what_each_difficulty_changes` ("8 health", "5 health", "3 health"), `quit_to_title_asks_once_and_names_lost_progress_only_when_the_autosave_failed` (replacing `quit_to_title_asks_first_when_the_autosave_is_unavailable`), `quit_game_asks_first_from_pause_and_title`, `game_over_quits_to_the_title_without_asking`; `quit_to_title_autosaves_and_returns_to_the_title` answers the new question, and `a_close_in_gameplay_autosaves_in_the_last_frame` holds unchanged;
  - `tests/game/keys.rs`: `every_bindable_action_is_listed_and_no_ui_or_engine_action` updated, with the capture test on Pause's gamepad cell;
  - `tests/game/sample.rs`: `mute_survives_a_restart` removed, its persistence covered by `back_saves_settings_and_options_across_a_restart`;
  - `tests/game/shell.rs`: `the_hud_shows_health_gems_and_the_bound_pause_key` updated.
- **Done-when:** step 2's four checks → the same results; `rg -n 'toggle_mute' templates/basic` → no match.
- **Moves:** none.

### Step 5: The sample

- **Files:** step 5's list in the header.
- **Change:** §2.6. Tests:
  - `tests/game/sample.rs`: `the_player_accelerates_and_stops` (from rest under `move_right` at dt 1/60, the body's speed is below 240 px/s after one step and 240 within nine; released, 0 within seven), with the M69 movement tests reworked for the ramp; `a_click_shoots_a_bolt_toward_the_cursor` (the cursor right of the player, `shoot` pressed: a bolt spawns within two steps and moves right; after its cooldown, a second); `the_dummy_takes_hits_shows_damage_and_never_dies` (200 hits: health above 0; a label a hit with damage numbers on, none with them off); `the_heal_pickup_heals_two`; `the_speed_pickup_speeds_the_player_for_four_seconds`; `the_camera_frames_the_play_space_at_every_window_size` (zoom 1 at 1280×720, 3 at 3840×2160, 1.5 at 2560×1080, the view centred on (640, 360) at a run's start);
  - `tests/game/saves.rs`: `a_schema_2_save_migrates`, `pickups_left_round_trip`; `a_schema_1_autosave_migrates` passes through schema 2.
- **Done-when:** step 2's four checks → the same results, `tungsten check` covering the new prefabs, data and cue rules.
- **Moves:** the template's `SaveGame::SCHEMA`, 2 → 3.

### Step 6: Sound and music

- **Files:** step 6's list in the header.
- **Change:** §2.7: the script, its README, the eight files, the manifest's `sounds` (buses, volumes, loops, voice limits), the cues, the shell's menu sounds, the sample's music; `just script-test` gains `python3 -B templates/basic/tools/sounds.py --check`. Tests: `tests/game/shell.rs`: `menu_sounds_play_on_move_activate_and_back`; `tests/game/sample.rs`: `a_shot_a_hit_and_a_pickup_play_their_sounds`, `the_run_music_fades_in_with_a_run_and_out_at_its_end`, `pause_fades_the_run_music_under_its_loop_and_back`, `quit_to_title_from_pause_stops_the_pause_loop` (pause, Quit to Title, Quit: both voices stopped), `game_over_plays_no_pause_loop`; each reads `Harness::audio()`.
- **Done-when:**
  - `python3 -B templates/basic/tools/sounds.py --check` → exit 0, naming eight matching sounds.
  - `python3 -B templates/basic/tools/sounds.py --out <scratch>/a` and again into `<scratch>/b`, then `cmp` on each pair → identical.
  - `du -cb templates/basic/assets/sounds/*.wav | tail -1` → at most 700000.
  - Step 2's four checks → the same results.
- **Moves:** none.

### Step 7: The template in the player archive

- **Files:** step 7's list in the header.
- **Change:** §2.8, then decision 186 (amends `D-072` and `D-182`; extends `D-120` and `D-122`), with its index row and marker lines. Tests: `scripts/test-release.py` gains `test_package_ships_the_template_from_its_folder` (Linux and Windows layouts), `test_template_debug_files_join_the_debug_archive`, `test_readme_names_the_template_and_its_log_folder`, `test_shipped_packages_include_the_templates` and `test_folders_match_the_launcher`, its fixture gaining a `templates/basic` member; the launcher gains `a_listed_build_runs_from_its_folder` and `an_example_runs_from_the_root`.
- **Done-when:**
  - `python3 -B scripts/test-release.py` → passes; `cargo test -p tungsten-launcher --locked` → passes; `actionlint .github/workflows/release.yml` → clean.
  - `just repo-check` → passes, decision 186 in the index (`rg -n '^## D-18[6] ' DECISIONS.md` → one heading).
  - The local archive check, with at least 12 GiB free (`df -h --output=avail .`), else recorded not run:
    - for each of `x86-64-v3` and `x86-64`: `RUSTFLAGS="-C target-cpu=<level> -C link-arg=-Wl,--build-id" cargo build --release --locked --workspace --exclude tungsten-crash-probe --bins --target x86_64-unknown-linux-gnu --target-dir target/rel/<level>` → builds;
    - `python3 -B scripts/release.py licenses x86_64-unknown-linux-gnu --out target/rel/licenses.json` then `python3 -B scripts/release.py package v0.0.0-local x86_64-unknown-linux-gnu --bin-dir target/rel --out target/rel/dist` → prints both archives;
    - `tar -tzf` on the player archive → lists `tungsten-template-basic`, `bin/x86-64/tungsten-template-basic`, `bin/x86-64-v3/tungsten-template-basic` and `templates/basic/{tungsten.json,input.json,assets/manifest.json}`, and no `.debug`; on the debug archive → each level's `tungsten-template-basic.debug` beside the examples';
    - README.txt in the archive names `./tungsten-template-basic` and `~/.local/state/tungsten-template-basic/logs`;
    - extract into `target/rel/x`, take `top="$(realpath target/rel/x/tungsten-examples-v0.0.0-local-x86_64-unknown-linux-gnu)"` before leaving the repository, then `(cd / && TUNGSTEN_SMOKE_FRAMES=60 "$top/tungsten-template-basic")` → exit 0, its first stderr line naming the chosen level, no `Unknown font ID`; `"$top/example-01-platformer"` the same way → exit 0;
    - `rm -rf target/rel` afterwards.
  - `just notices` → passes for both targets.
- **Moves:** none.

### Step 8: Docs and records

- **Files:** step 8's list in the header.
- **Change:**
  - Decision 187: the template's part, amending `D-181`: three families, the motion, scrims and cards, the curtain, eleven slots, the seventh seam name, the quit questions, no mute hotkey, the sample's additions, schema 3 and the caps of §2.9. Its index row and `D-181`'s marker line; `D-180` stands.
  - `templates/basic/README.md`: slots, fonts, controls (left mouse shoots, no M), the sounds' script.
  - `templates/basic/AGENTS.md` within 12,288 bytes and 100 lines (A36): the seventh seam name, eleven slots, the caps, the fit matrix in a few words, the new prefabs and kit items; trim the catalogue line's parentheses to fit.
  - `docs/getting-started.md`'s template sections: the seam, the controls, the settings example (a volume change in place of the mute key), eleven slots, the fit description, the sounds' script.
  - `DESIGN.md`: M53's and M69's paragraphs made current (the scale factor, the mute, the slots, the seam, the caps), and a short engine paragraph for the monitor, shipped volumes, the scale policy and the token.
  - `docs/LLM_INDEX.md` within 12,288 bytes: the template row widened to `templates/basic/` (its `tools/`), "monitor" in the display row.
  - `CHANGELOG.md`: one `[Unreleased]` line.
  - `docs/known-issues.md`: the pause track advancing under the pause; a same-scale Wayland output move keeping the old monitor; the template's sounds now with provenance (the root's stay unrecorded).
  - `docs/plans/1.0/backlog-1.x.md`: rows 71 and 73 narrowed to what stays (exclusive fullscreen, a game's own tokens), row 74 noting that its trigger fired, a row for gamepad aim in the sample.
  - `docs/plans/1.0/w12-template.md`: the budget numbers in the non-goals and scope, and follow-up 2's clear-colour lead marked closed by this pass.
- **Done-when:**
  - `just ctx` → passes; `just repo-check` → passes.
  - `rg -n -e toggle_mute -e 'M mutes' -e 'mute key' -e mute_survives -e 'four slots' -e 'three slots' -e 'to .slot_3.' -e 'six names' -e '731 × 411 and 1920' templates/basic/README.md templates/basic/AGENTS.md docs/getting-started.md DESIGN.md` → no match (the docs' wording only: `slot_3` stays a valid slot in code and tests).
  - `rg -no '.{0,40}D-18[7].{0,40}' DECISIONS.md docs/DECISION_INDEX.md` → the heading, `D-181`'s marker and the row.
  - `CHANGELOG.md`'s `[Unreleased]` holds one line.
  - `git diff --quiet HEAD -- docs/plans/1.0/roadmap.json docs/plans/1.0/README.md docs/plans/1.0/implementation-plan.md` → exit 0 (no roadmap or register change).
  - Records follow what landed: a skipped step's feature is written as not landed, with its known-issues line.
- **Moves:** none.

### Step 9: Whole-pass checks, the title fixture and captures

- **Files:** `templates/basic/tests/fixtures/title.png`; this plan.
- **Change:** regenerate the fixture from `templates/basic`: `TUNGSTEN_SMOKE_FRAMES=24 TUNGSTEN_CAPTURE_FRAME=20 TUNGSTEN_CAPTURE_RESOLUTION=1280x720 TUNGSTEN_CAPTURE_PATH="$PWD/tests/fixtures/title.png" cargo run`; run the checks below; fill the evidence; set `status: done`; move the plan to `docs/plans/archive/template-polish.md` and repoint any link to it.
- **Done-when:**
  - `cargo fmt --all -- --check` → exit 0; `just check` → passes.
  - From `templates/basic`: `cargo test --locked` and `cargo clippy --all-targets --locked -- -D warnings` → pass.
  - `just template-check` → its last line, `Template check passed: …` (6 GiB floor).
  - `just api-check` → passes (step 1's `just api` holds).
  - `tokei --output json templates/basic/src/shell | jq .Total.code` → at most 2400; the same for `templates/basic/src` → at most 2900.
  - `just script-test` → passes; `just notices` → passes.
  - The local archive check (step 7) → passed, or rerun when a later step changed the template's code.
  - `just smoke` → every row OK; `just visual` → passes, the template against its new fixture and every other fixture unchanged.
  - Captures from `templates/basic`, into the scratchpad, each `TUNGSTEN_USER_DIR=<scratch>/u TUNGSTEN_SMOKE_FRAMES=40 TUNGSTEN_CAPTURE_FRAME=30 TUNGSTEN_CAPTURE_PATH=<scratch>/<name>.png cargo run` with: `TUNGSTEN_DISPLAY_RESOLUTION=1280x720` (`title-720.png`, this machine's OS scale 1.75); the same with `WINIT_X11_SCALE_FACTOR=1` (`title-720-os1.png`); `TUNGSTEN_DISPLAY_RESOLUTION=3840x2160` (`title-2160.png`). When the window manager shrinks the 4K window, rerun it with `TUNGSTEN_DISPLAY_MODE=borderless_fullscreen`. `ffprobe -v error -show_entries stream=width,height -of csv=p=0` → `1280,720`, `1280,720` and `3840,2160`. `ffmpeg -v error -i <png> -vf crop=1:1:4:4 -f rawvideo -pix_fmt rgb24 - | od -An -tu1` on each → `11 12 16`, ± 1. The session views all three: a serif title on one line above the buttons and the version and "Saving is off" at the bottom left in each; `title-720-os1.png` and `title-2160.png` the same layout at 1× and 3×, filling the same share of the window; `title-720.png` the 731×411-unit layout, 1.75 times as large against its window, as Q1 chooses.
  - `just repo-check` → passes after the archive move.
- **Ends with:** the changed-file list; each check's result, not run where it did not run; the playtest checklist below, filled in with what landed.

**Playtest checklist (the run's report).** Each item names what to try, where, at which window size and scale, and what to see. Run from `templates/basic` with `cargo run`, a fresh `TUNGSTEN_USER_DIR` for first-run values; set 3840×2160 with Settings → Display → Window size or `TUNGSTEN_DISPLAY_RESOLUTION=3840x2160`, and the extremes with Settings → Interface: UI 50 % with text 75 %, and UI 300 % with text 200 %.

1. Background, at 1280×720 and 3840×2160: empty window areas are near-black `#0b0c10`, not slate grey.
2. Title, at both sizes and both extremes: one serif line above the buttons; the version and "Saving is off" small at the bottom left; at 4K the UI fills the share a 1280×720 window shows at OS scale 100 % (`WINIT_X11_SCALE_FACTOR=1`); on this 175 % display a 1280×720 window shows it 1.75 times as large, by design (Q1).
3. Motion, at 1280×720: Settings from the title appears at once with a short fade and slide, never black; Back is a blink; New Game and Quit to Title fade through near-black and back.
4. Pause and dialogs, at both sizes: Escape shows a small centred "Paused" over a dimmed world with the HUD under it; Quit to Title's question shows over the dimmed pause card.
5. Keybindings, at 1280×720, then at text 200 %: six rows with no scrolling, then a card that scrolls; rebinding still works.
6. Sliders, at both sizes: every slider shows its number while dragged or stepped; the frame-rate limit runs 10–480 in fives, and Unlimited greys it out.
7. Volumes, with a fresh user folder: master, music and effects read 80 %; Reset tab returns them to 80 %; effects sound at 64 % of full scale.
8. Window sizes, X11 and, when available, Wayland: the size list climbs to the monitor's own size (3840×2160 here), each change asks to keep it; on Wayland, the M69 sizes that fit plus the monitor's.
9. Scale extremes, at both sizes and both extremes: title, picker, pause, game over, save, load, each settings tab, keybindings and a confirm keep their layout; nothing leaves its card; a tall card scrolls with the arrows.
10. Saves: pause → Save lists the autosave and ten slots and scrolls; save to Slot 10; the title's Continue reads "Slot 10 · just now"; Load lists all eleven.
11. Quit: pause's Quit to Title and Quit Game ask once; the title's Quit asks; game over's Quit to Title does not; the close button quits at once.
12. Difficulty: New Game shows 8, 5 and 3 health beside Easy, Normal and Hard.
13. Mute: M does nothing, and Keybindings has no Mute row; "Mute when unfocused" still mutes.
14. Movement: the player eases into motion and slides briefly to a stop.
15. Shooting, at both sizes: left-click fires a bolt toward the cursor.
16. Dummy: bolts flash it and float damage numbers while Damage numbers is on, none when off; it never dies.
17. Pickups: the green one heals 2 after a hazard hit; the blue one speeds the player for 4 s.
18. Camera, at 3840×2160: the play space fills the window, centred, as at 1280×720.
19. Danger: at 1 HP the health bar turns red; Delete buttons are red.
20. Sounds: menu move, select and back; shoot, hit and pickup; all follow the Effects slider.
21. Music: New Game fades in a chiptune that loops cleanly at its bar line; pause fades it out under a breakbeat; Settings from pause keeps the beat; Resume brings the music back; game over plays no beat; Quit to Title fades the music out.
22. Numbers in mono: slider values, HUD counts, save times and playtime, damage numbers.
23. The release workflow, on a rehearsal pre-release tag, after committing the pass yourself: push the branch; create and push a unique rehearsal tag, `v0.0.0-test.<YYYYMMDD>.g<sha>` ([releases](../releases.md), rehearsals); watch its run; download both platforms' archives; check that each lists the template's launcher, its builds and `templates/basic/` and each debug archive its debug files or PDBs; launch the template from outside the extracted folder on Linux and on Windows; then delete the rehearsal release and tag when you choose.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | How does the OS scale combine with the window base? | The larger of the two, times the UI scale, under the floor: on this 175 % display a 1280×720 window shows the UI 1.75 times as large against the window as a 4K one does. Alternative: the window alone, the same view at every size and DPI, with smaller UI in a small window on a high-DPI screen |
| Q2 | The floor viewport? | 731×411 units, M69's smallest |
| Q3 | The frame-rate slider's step? | 5 fps (§2.4) |
| Q4 | Motion values? | Push 0.10 s, a 6-unit fade and slide; pop 0.08 s; run curtain 0.10 s in and 0.14 s out |
| Q5 | Resume the pause music where it stopped? | No: the run's track plays on at 0 under the pause; a voice pause stays backlog row 34 (`D-034`, `D-179`) |
| Q6 | How far does pause duck the run's music? | To 0 over 0.3 s |
| Q7 | Music under game over? | The run's track plays on; no pause loop; the run's end fades it |
| Q8 | Saved or fresh: the pickups and the dummy? | Pickups saved (schema 3); the dummy fresh; boosts unsaved |
| Q9 | Where do the difficulty notes come from? | A seventh seam name, `game::difficulty_note` |
| Q10 | Copy JetBrains Mono into the template? | No: the engine's `engine_mono` (S1) |
| Q11 | Which window sizes? | Modes of at least 1280×720 inside the current size, and the current size; a one-mode monitor adds M69's sizes that fit; none, M69's list |
| Q12 | A window as large as the desktop? | Sent like any size; the window manager places or shrinks it; the keep question guards it |
| Q13 | WAV or OGG? | 16-bit mono WAV at 22,050 Hz: no installed encoder regenerates OGG byte for byte |
| Q14 | How does the launcher learn the template's folder? | A table in the launcher, held to `release.py` by a test; alternative, a side file |
| Q15 | The template's version in the archive? | Its own 0.1.0 |
| Q16 | A perf sitting? | None: no system, stage or per-frame lookup is added; `ScaleFactor::get` returns a cached value; an unset rate adds one check per mover a step; no kit bench row is run |
| Q17 | A pad binding for Shoot? | None: the aim is the cursor's; the player may bind one |
| Q18 | The low-health threshold? | At or below a third of max |
| Q19 | Continue's second line? | "Slot 3 · 5 min ago", "Autosave · just now" |
| Q20 | Loading eleven slots per refresh? | Kept: saves are a few hundred bytes; backlog row 74 notes its trigger fired |
| Q21 | Doc budgets? | Unchanged: trim to fit, no budget raised |
| Q22 | Screen headings in serif? | Yes, with the title (S3) |
| Q23 | The curtain over toasts? | Toasts stay above it |
| Q24 | The Wayland probe? | A nested `kwin_wayland --virtual`; not run when it cannot start |
| Q25 | A Wayland window moved between outputs of one scale? | Keeps the old monitor until a scale change (known issue) |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`).

## 5. Decisions expected

Next free IDs at `77e014a` (184; `D-182` and `D-183` are M70's), checked again when each is written:

- 184: the engine's part: `MonitorInfo` and `VideoMode`, `AudioConfig` and `shipped_volume`, the window-relative `ScaleFactor` with a floor, `ColorTokens.danger`. Amends `D-127`, `D-159` and `D-179`. Step 1.
- 185: the kit's part: `top_down_mover`'s acceleration and deceleration. Amends `D-172`. Step 1.
- 186: the template in the examples' player and debug archives, and the launcher's folder table. Amends `D-072` and `D-182`; extends `D-120` and `D-122`. Step 7.
- 187: the template's part. Amends `D-181`; `D-180` stands. Step 8.

## 6. Invariants

- **May move:** `api/tungsten-core.txt`, `api/tungsten.txt`, `api/tungsten-kit.txt` and `api/tungsten-kit-components.txt` by additions (step 1); the template's `SaveGame::SCHEMA` (step 5); `templates/basic/tests/fixtures/title.png` (step 9); the CLI binary's size, which embeds the new assets.
- **Must not move:** the determinism hash, the pinned containment hash, the row digests, `gpu-visual.png`, the post, transition and UI-lab regressions, the kit hash `0x6fbd_8f24_2114_15a5`, `api/tungsten-cli.txt`, the template's schedule snapshot (no step adds a system), the debug views' sizes, the root `tungsten.json` and `input.json`.

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies (plain `cp`, then `cmp`), mark the step skipped with its reason, and continue with the steps that do not depend on it. Steps 2–6 need step 1; 3 and 4 need 2; 6 needs 5's prefabs; 9's checks run on what landed.
- A finding of the step-1 API critique needs an API beyond §2.1's five additions: leave that part out, record it, and run the dependent items on their fallbacks (no monitor: M69's sizes; no window base: the OS scale).
- A check would need a file outside the header's lists: stop that change and record a follow-up; generated snapshots and fixtures that a step's own check regenerates are in scope.
- A must-not-move invariant moves: restore the step that moved it.
- `tokei` passes the proposed caps after compaction: stop and record; no cap above 2,400 and 2,900 in this pass.
- A doc budget cannot be met by trimming: keep that doc's old text, record it, and never raise a budget.
- Below a disk floor (6 GiB for `just template-check`, 12 GiB for the archive builds), with no GPU or display, or with no Wayland compositor: record the check as not run.
- Something needs a new system, stage or per-frame lookup in the engine: stop it and record; the pass has no perf sitting (Q16).
- Never commit, bump a version, run `just release-cut` or `/tungsten-finalize`, push or tag.

## 8. What the pass owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1–9 | each step's checks; step 9 runs `just check`, `just smoke`, `just visual`, `just script-test`, `just template-check`, `just notices` and `just api-check` once at the end; no perf rows (Q16) |
| 3 Invariants | 1, 5, 9 | §6 |
| 4 Decisions | 1, 7, 8 | 184–187, before the docs that cite them |
| 5 API ([break ledger](1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api`) | 1 | `ColorTokens.danger`'s row and the additions for the freeze |
| 6 Template and guide | 2–6, 8 | the template is the subject; the guide's template sections in step 8 |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 8 | |
| 8 Design | 8 | `DESIGN.md`'s M53 and M69 paragraphs and the engine paragraph |
| 9 Records | 8 | known issues, the backlog and w12; no register row or README fields (no roadmap stop) |
| 10 Close | 8, 9 | one `CHANGELOG.md` line, `status: done`, the plan archived; no cut, since this pass releases nothing |

## 9. Critique

Round 1, the only round (the owner's request, so no second round although round 1 changed done-when checks): the `critique` skill with `--repo` on 2026-10-20, reading `77e014a` with this plan untracked; `gpt-6-astra`, effort max, context 872,000, output cap 4,000; the artifact cut with `sed` (§9 and the evidence log left out), 72.6k characters, so `--force` (step 7 of the skill confirms it). Footer: 500.9 s, tokens in 1,952,716 (cached 1,813,632), out 17,928 (reasoning 10,487), 47 commands, 409.4 KB output (largest 32.3 KB), usage 5h 56 % → 97 %, week 54 % → 61 %. The 5-hour window resets at 06:21 UTC, before which step 1's API critique cannot run. Six findings, each checked against the repository; all six accepted.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] Closing during the curtain can autosave a mixture of two runs: `start_run` installs the incoming `RunState` at once, and the exit autosave pairs it with the old run's entities | `templates/basic/src/shell/mod.rs:211-224,440-452`, `templates/basic/src/save.rs:59-85` | accepted | A6 extended; §2.2: `start_run` stages the `RunState` in the flow and `shell_flow` inserts it at the swap; step 2 adds `a_close_during_a_run_change_saves_one_run_whole` |
| 2 | [MEDIUM] Quit to Title has no cleanup for the pause loop: the pops and the replace apply in one dispatcher pass, so `sample_frame` never sees gameplay on top again | `crates/tungsten/src/state.rs:281-289`, `crates/tungsten/src/audio.rs:524-527` | accepted | §2.7: a run's `on_exit` also stops `music_pause`; step 6 adds `quit_to_title_from_pause_stops_the_pause_loop` |
| 3 | [MEDIUM] An existing 144 fps setting has no representation on the 5-fps grid: `set_slider_value` snaps it silently | `templates/basic/src/shell/settings.rs:33`, `crates/tungsten-core/src/ui/control.rs:326-330` | accepted | §2.4: an off-grid or out-of-range stored cap applies as stored, its readout exact, the slider snapped, written only by a move; step 3 adds `an_off_grid_cap_keeps_its_value_until_moved` |
| 4 | [MEDIUM] The capture check contradicts the scaling policy: at OS 1.75 the 720p window lays out 731×411 units and the 4K one 1280×720, so they cannot fill the same share | §2.3, A15's OS scale; winit's `WINIT_X11_SCALE_FACTOR` (`winit-0.30.13/src/window.rs:527-535`) | accepted | Step 9 captures three: 720p at OS 1.75, 720p at `WINIT_X11_SCALE_FACTOR=1` and 4K, the last two the same layout at 1× and 3×; Q1 and playtest item 2 say what 1.75 does |
| 5 | [MEDIUM] Step 8's `slot_3\b` grep over `templates/basic` matches a slot the design keeps | `templates/basic/src/shell/mod.rs:42`, `templates/basic/tests/game/saves.rs:344` | accepted | Step 8's check reads the four docs only, for stale wording |
| 6 | [MEDIUM] The archive launch check runs a relative path after `cd /` | step 7's command | accepted | Step 7 takes the extracted folder's `realpath` before leaving the repository |

**Step 1 API critique: not run.** The 5-hour window read 97 % at 04:46:22Z on 2026-10-20 (week 61 %), resetting at 06:21:50Z, over the 65 % this plan sets, so the critique was recorded as not run and the run carried on, as §3's step 1 paragraph and the owner's run prompt say; step 1 stands as written. To run it later: `critique --raw --repo --force --out <scratchpad>/api-critique.md --from 'git --no-pager diff HEAD -- api/ templates/; git ls-files -o --exclude-standard -z -- api templates | xargs -0 -r -n1 git --no-pager diff --no-index /dev/null'`, on a tree where `templates/` holds steps 2–6 too.

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-20 | done | `cargo fmt --all -- --check` exit 0. `cargo test --locked -q`: core 1,197 passed, umbrella 718, kit 715, `example-05-ui-lab` 25, none failed. `git diff --quiet HEAD -- crates/tungsten/src/tests/kit_recipes/` exit 0; the plan's filter `tests::kit_recipes` matched 0 tests (exit 0; the module is `kit_recipes` at the crate root), so `cargo test -p tungsten --locked --lib -- kit_recipes` ran: 80 passed, `the_recipes_hash_the_same_twice_and_match_the_pin` among them, so `0x6fbd_8f24_2114_15a5` holds. Workspace clippy clean (after one `single_match_else` fix). `just api`: core +148 lines, umbrella +1, kit +4, kit components +2, 0 removed in each (Python `Counter`; `git diff \| rg -c '^-[^-]'` printed nothing, exit 1); the pattern matches `tungsten-core.txt` 142, `tungsten.txt` 1, `tungsten-kit.txt` 8; the components file's two new lines are `top_down_mover`'s `acceleration` and `deceleration` (the pattern also matches four older `linear_motion` and `steering` lines); `just api-check` passes. `just repo-check` passes; `D-184` and `D-185` headings; `Amended by D-184` under `D-127`, `D-159`, `D-179`, `Amended by D-185` under `D-172`. X11 probe: `Monitor 3840×2160 at 60 Hz, 46 modes`, one line. Wayland probe (nested `kwin_wayland --virtual` 1920×1080): `Monitor 1920×1080 at 60 Hz, 1 modes`, one line, run exit 0. Departures: `AudioConfig::validate` joins `bus_volume`, so `App::new` refuses a code-built bad volume as it does `game` and `time`; `crates/tungsten-kit/src/tests/describe.rs`, outside step 1's list, moves its pinned count of described numeric fields 96 → 98 for the two new scene fields (a pin of the same descriptions the components snapshot holds); the debug views' saved-place `units` became `get() × debug / os`, M58's `ui × debug` when nothing is set; one test beyond the list, `set_monitor_inserts_replaces_and_removes_it` | `DECISIONS.md` (`D-184`, `D-185`), `crates/tungsten-core/src/{display,config}.rs`, `crates/tungsten/src/{settings,display,app,debug_views}.rs`, `crates/tungsten/src/testing/ui.rs`, `crates/tungsten-kit/src/motion/top_down.rs`, `api/`; logs in the session scratchpad |
| 2 | 2026-10-20 | done | From `templates/basic`: `cargo test --locked -q` 92 passed (85 M69, 7 new), 0 failed; `cargo clippy --all-targets --locked -- -D warnings` clean; `tungsten check: no errors`. `cargo test -p tungsten-cli --locked -q` 48 passed. `just repo-check` passes. `tokei`: `src/shell/` 2,051, `src/` 2,391 (§2.9's estimates 2,059 and 2,402). `cargo fmt --all -- --check` exit 0. Every screen is a scrim root (`background` at alpha 184, `RootCover::Show`) holding a scroll-panel card; pushes act on their first frame, a pop fades 0.08 s, a run change runs the 0.24 s curtain with the run state staged until the swap. M69 tests adapted in timing only, their assertions kept: `start_at` settles through the curtain instead of stepping 2 frames; `retry_with_the_old_dead_player_still_queued_stays_in_gameplay` steps to the replace frame under the curtain and checks `transitioning` once settled; the keybindings fit test looks for its list inside the card. Departure: the damage label's font (`labels.json`, S2) moved to step 5, since `tests/game/sample.rs`, which pins the label's font, is outside step 2's list and §2.6 places the change there; `labels.json` restored and `cmp`-equal | `templates/basic/src/shell/`, `templates/basic/src/sample/hud.rs`, `templates/basic/assets/{manifest.json,ui/themes/shell.json,fonts/SourceSerif4/}`, `templates/basic/tungsten.json`, `templates/basic/tests/game/{main,shell,fit}.rs` |
| 3 | 2026-10-20 | done | From `templates/basic`: `cargo test --locked -q` 98 passed (8 new: the fit matrix and the keybindings card in `fit.rs`; the scale policy, readouts, frame-rate slider and toggle, off-grid cap, shipped volumes and monitor sizes in `settings.rs`), 0 failed; clippy clean (after two needless `mut`s); `tungsten check: no errors`. `cargo test -p tungsten-cli --locked -q` 48 passed. `just repo-check` passes. `tokei`: `src/shell/` 2,139, `src/` 2,479 (estimates 2,154 and 2,497). `cargo fmt --all -- --check` exit 0. The matrix's six entries give 731.4×411.4, 731.0×411.2 (floor), 2560×1440, 1280×720, 731.0×411.2 (floor) and 2560×1440 units; every screen passes §2.3's rules at each. Departures, each forced by the matrix: `WIDE_WIDTH` 520 → 600, so a slider row fits beside the tab rail at text 200 % in the floor; a slider is `Px(170)` (it holds no text, and its natural width is ten line heights, 440 units at text 200 %, which forced its row past the card); a choice is `Fill` with a 170-unit least width, not `Auto`, since an `Auto` box never shrinks below its text ("Chosen at New Game" overflowed); the keybindings card's design width is 760 (wider than the 731-unit floor, so a stick's name keeps one line at text 100 %), its captions `Auto` at least 88 units times the text scale, its cells `Fill`, and a hidden Reset in the header lines the columns up; the frame-rate slider keeps its value under Unlimited, so turning Unlimited off restores the last cap. Four M69 tests' volume literals move with the shipped 80 % (`[1.0]` → `[0.8]`, 0.95 → 0.75), their assertions otherwise kept | `templates/basic/src/shell/{mod,theme,widgets,settings,keybindings}.rs`, `templates/basic/tungsten.json`, `templates/basic/tests/game/{fit,settings}.rs` |
| 4 | 2026-10-20 | done | From `templates/basic`: `cargo test --locked -q` 101 passed, 0 failed (new: the two eleven-slot saves tests, Continue's slot and age, the picker's notes, the quit questions three ways; `mute_survives_a_restart` removed); clippy clean; `tungsten check: no errors`. `cargo test -p tungsten-cli --locked -q` 48 passed. `just repo-check` passes. `rg -n 'toggle_mute' templates/basic` → no match (exit 1). `tokei`: `src/shell/` 2,156, `src/` 2,495 (estimates 2,184 and 2,537). `cargo fmt --all -- --check` exit 0. Eleven slots in the scrolling card, focus scrolling the last into view at 731 × 411; pause's Quit to Title and Quit Game autosave, then ask once, naming lost progress only after a failed autosave; the title's Quit asks; game over's Quit to Title asks nothing; `game::difficulty_note` is the seventh seam name. Tests that pressed Quit now answer the question: `quit_to_title_autosaves_and_returns_to_the_title`, `quit_game_requests_exit_and_the_last_frame_autosaves` (the autosave removed again before Quit, so the last frame's save is still what it checks) and step 2's `menu_changes_take_their_times` (timed from the question's Quit) | `templates/basic/src/shell/{mod,saves,title,pause,game_over}.rs`, `templates/basic/src/{game.rs,sample/hud.rs}`, `templates/basic/input.json`, `templates/basic/tests/game/{saves,shell,keys,sample}.rs` |
| 5 | 2026-10-20 | done | From `templates/basic`: `cargo test --locked -q` 109 passed, 0 failed (new: `the_player_accelerates_and_stops`, `a_click_shoots_a_bolt_toward_the_cursor`, `the_dummy_takes_hits_shows_damage_and_never_dies`, `the_heal_pickup_heals_two`, `the_speed_pickup_speeds_the_player_for_four_seconds`, `the_camera_frames_the_play_space_at_every_window_size`, `a_schema_2_save_migrates`, `pickups_left_round_trip`; `a_schema_1_autosave_migrates` checks schema 2's step); clippy clean; `tungsten check: no errors`, the four new prefabs, `abilities/shot`, `effects/{mend,heal,swift}` and the `labels` rule among the files it loads. `cargo test -p tungsten-cli --locked -q` 48 passed. `just repo-check` passes. `tokei`: `src/shell/` 2,156, `src/` 2,555 (estimate 2,607). `cargo fmt --all -- --check` exit 0. `SaveGame::SCHEMA` 2 → 3. The schedule snapshot is unchanged (no system added). M69 movement tests reworked for the ramp through a `ramp(steps, top)` helper: the 30-step walks, the half clock (15 eased steps, not half of 30), the walk onto the hazard's line (41 steps and a 10-px slide), the slowed half second (58.5 px eased in; the full-speed half second measured 70 steps on, past the ease back up); `gem_entities` counts `GemSlot`, since pickups carry `pickup` too; the label tests expect `engine_mono` (S2, moved from step 2). Departures: two test files outside step 5's list, each the planned change's direct consequence: `tests/game/shell.rs`'s `walk` now lets the player slide to a stop (`release_and_settle` in `main.rs`) before the walked position is read, which four save tests compare; `tests/game/keys.rs`'s pinned list of `BINDABLE_ACTIONS` labels gains "Shoot" | `templates/basic/src/{save.rs,game.rs,sample/{mod,hud}.rs}`, `templates/basic/input.json`, `templates/basic/assets/{manifest.json,prefabs/{player,bolt,dummy,heal_pickup,speed_pickup}.json,data/{tags.json,cue_rules/labels.json,abilities/shot.json,effects/{mend,heal,swift}.json}}`, `templates/basic/tests/game/{sample,saves,main,shell,keys}.rs` |
| 6 | 2026-10-20 | done | `python3 -B templates/basic/tools/sounds.py --check` exit 0, eight lines `<name>.wav: matches` and `8 sounds match`; `--out <scratch>/snd/a` and `/b`, then `cmp` on each of the eight pairs → identical; `du -cb templates/basic/assets/sounds/*.wav \| tail -1` → 541,606 (cap 700,000). `music_run` 151,200 samples (6.857 s), `music_pause` 100,800 (4.571 s), both passing the seam check; the effects 0.05–0.22 s; every file peaks at -2 dBFS. From `templates/basic`: `cargo test --locked -q` 115 passed (new: `menu_sounds_play_on_move_activate_and_back`, `a_shot_a_hit_and_a_pickup_play_their_sounds`, `the_run_music_fades_in_with_a_run_and_out_at_its_end`, `pause_fades_the_run_music_under_its_loop_and_back`, `quit_to_title_from_pause_stops_the_pause_loop`, `game_over_plays_no_pause_loop`), 0 failed; clippy clean; `tungsten check: no errors`. `cargo test -p tungsten-cli --locked -q` 48 passed. `just repo-check` passes. `tokei`: `src/shell/` 2,190, `src/` 2,642 (estimates 2,209 and 2,672). `cargo fmt --all -- --check` exit 0. `just script-test` gains the `--check` line (run whole in step 9). No `__pycache__` under `templates/`, `scripts/` or `tools/` | `templates/basic/tools/{sounds.py,README.md}`, `templates/basic/assets/sounds/*.wav`, `templates/basic/assets/{manifest.json,prefabs/{player,dummy,gem,heal_pickup,speed_pickup}.json}`, `templates/basic/src/{shell/{mod,theme}.rs,sample/mod.rs}`, `templates/basic/tests/game/{shell,sample}.rs`, `justfile` |
| 7 | 2026-10-20 | done | `python3 -B scripts/test-release.py` → `Ran 44 tests`, `OK` (5 new); `cargo test -p tungsten-launcher --locked` → 5 passed (2 new); `actionlint .github/workflows/release.yml` exit 0, no output. `just repo-check` passes; `rg -n '^## D-18[6] ' DECISIONS.md` → one heading (line 2590); `Amended by D-186` under `D-072` and `D-182`. `just notices` passes: 206 crates for `x86_64-unknown-linux-gnu`, 179 for `x86_64-pc-windows-msvc`, 2 embedded works each. The local archive check, 187 GB free: both release builds `Finished` (3 min 6 s and 3 min 5 s); `licenses` 206 crates; `package` printed both archives; the player archive lists `tungsten-template-basic`, `bin/x86-64/tungsten-template-basic`, `bin/x86-64-v3/tungsten-template-basic` and `templates/basic/{tungsten.json,input.json,assets/manifest.json}`, 0 `.debug` files; the debug archive holds `tungsten-template-basic.debug` beside `example-01-platformer.debug` at both levels; README.txt names `./tungsten-template-basic` (lines 10, 17) and `~/.local/state/tungsten-template-basic/logs` (line 19); from `/`, `tungsten-template-basic` and `example-01-platformer` exit 0, first line `tungsten-launcher: running the x86-64-v3 build`, 0 `Unknown font ID`, 0 `ERROR`. A capture of the archived template run from `/` drew the shell's theme (corner `11 12 16`) but its title as "Tungsten": the known issue P2 (`core/ui/layout/taffy.rs`, `place`), a label box sized to its own text committing an ulp short and wrapping its last word out of sight. Fixed in `src/shell/title.rs` (steps 2 and 4's file): the title takes the card's width, and Continue's slot-and-age line, now centred, and the picker's notes take `TextWrap::None`, the issue's workaround. Template rerun: 115 passed, clippy clean, `tungsten check: no errors`; CLI 48 passed; `just repo-check` passes; `tokei` `src/shell/` 2,202, `src/` 2,654; fmt exit 0. A live run with a saved Slot 10 drew `Slot 10 · 5 min ago` centred under Continue. The archive check rerun on the fixed code (176 GB free; incremental builds 29 s and 28 s) printed the same lines, and its capture from `/` shows "Tungsten template" on one line; `rm -rf target/rel` after it, so step 9 needs no rerun unless a later step changes the template's code | `DECISIONS.md` (`D-186`), `docs/DECISION_INDEX.md`, `scripts/{release,test-release}.py`, `tools/launcher/src/main.rs`, `.github/workflows/release.yml`, `docs/releases.md`, `templates/basic/src/shell/title.rs` |
| 8 | 2026-10-20 | done | `D-187` written before the docs that cite it: its heading at `DECISIONS.md:2603`, `Amended by D-187` under `D-181` (line 2509), its index row after `D-185`'s and `D-181`'s row amended; `rg -no '.{0,40}D-18[7].{0,40}' DECISIONS.md docs/DECISION_INDEX.md` → those four lines. `just ctx` → `Agent context checks: OK`, `self-test: OK`, `LLM_INDEX.md` 12,285 B (budget 12,288; its display row gains the monitor and `D-184`, the template row reads `templates/basic/` and cites `D-187`). `just repo-check` passes. The stale-wording `rg` over the template's README and `AGENTS.md`, the guide and `DESIGN.md` → no match (exit 1). `CHANGELOG.md`'s `[Unreleased]` holds one line. `git diff --quiet HEAD -- docs/plans/1.0/roadmap.json docs/plans/1.0/README.md docs/plans/1.0/implementation-plan.md` exit 0. The template's `AGENTS.md` 12,270 B and 59 lines (budget 12,288 and 100), its catalogue line's parentheses trimmed to fit. `DESIGN.md`: M53's and M69's paragraphs current and a new "Monitor, Shipped Volumes and the Window-Relative UI" section before M70's. Known issues: the pause music, the Wayland monitor on a same-scale move, the template's sounds' provenance, and P2's row naming the template's workaround (step 7). Backlog: rows 71 and 73 narrowed, row 74's trigger fired, a gamepad-aim row. `w12-template.md`: `D-187`'s budget and the clear-colour lead closed. No step was skipped, so every record describes what landed | `DECISIONS.md`, `docs/DECISION_INDEX.md`, `templates/basic/{README,AGENTS}.md`, `docs/getting-started.md`, `DESIGN.md`, `docs/LLM_INDEX.md`, `CHANGELOG.md`, `docs/known-issues.md`, `docs/plans/1.0/{backlog-1.x,w12-template}.md` |
| 9 | 2026-10-20 | done | Fixture regenerated with the step's command: 1280×720, the serif title on one line, `cmp` against the old one exit 1. `cargo fmt --all -- --check` exit 0; `just check` exit 0: workspace clippy clean, 42 suites, 3,144 passed, 0 failed, 8 ignored. From `templates/basic`: `cargo test --locked` 115 passed; clippy clean. `just template-check` → `Template check passed: tungsten new's game checks, builds, tests, smoke-runs and packages outside the repository, and check and package fail where they should.` (179 GB free). `just api-check` → `Public API snapshots current: 4 crates, the kit's components and the command line`. `tokei`: `src/shell/` 2,202 (cap 2,400), `src/` 2,654 (cap 2,900). `just script-test` passes: bench 37, clean-target 9, check-repo 51, roadmap 17, release 44, crash-report 7, release-preflight 22, the smoke-examples script, and `8 sounds match`. `just notices` passes: 206 and 179 crates. The local archive check passed in step 7 on the final template code (its rerun after the title fix); no later step changed the template's code. `just smoke`: every row OK (examples 5/5, template 1/1, matrix 4/4, post-stack 2/2, post-AA 1/1, bloom 1/1, lighting 1/1, game-feel 1/1, mesh/transition 5/5, UI screens 1/1, debug views 1/1, benchmarks 15/15, frame cap 1/1). `just visual` passes: bench 4 (`gpu_visual_matches_fixture`), post 5 (its one `fade` validation ERROR is `incompatible_stock_shader_edit_keeps_the_shipped_pipeline`'s deliberate edit), transition 1, UI lab 6, and `visual::the_title_matches_its_fixture` against the new fixture; `git status` lists no other image or fixture. Captures (scratchpad): `title-720.png` 1280,720; `title-720-os1.png` 1280,720; `title-2160.png` 3840,2040 windowed (KWin shrank it), then 3840,2160 with `TUNGSTEN_DISPLAY_MODE=borderless_fullscreen`; each corner `11 12 16`, each log free of ERROR and WARN. Viewed: each draws one serif line above the buttons and the version and "Saving is off" at the bottom left; `title-720-os1.png` and `title-2160.png` the same layout at 1× and 3× (the menu 260 and 780 px wide, the same share); `title-720.png` the 731×411-unit layout at 1.75 (the menu 455 px). Invariants: `just physics-release` 7, 1 and 7 passed (determinism, tunneling, containment); the kit recipes' pin passes inside `just check`; `git diff` shows `api/tungsten-cli.txt`, the root `tungsten.json` and `input.json`, the bench fixtures and core physics unchanged, and the template's schedule snapshot unchanged and passing. `just repo-check` after the archive move: in the run's report, since the plan is archived first | `templates/basic/tests/fixtures/title.png`, this plan |

## Follow-ups

- The template audit, [w12](1.0/w12-template.md) follow-up 2, runs next as its own stop; it keeps its F9 and F11 lead.
- A voice pause in the mixer for the pause music (backlog row 34); a bus fade for ducking (row 69).
- Gamepad aim in the sample (a backlog row from step 8).
- Exclusive fullscreen at runtime (row 71's remainder).
- `tungsten package` for Windows and macOS and for the template's archive (`D-182`'s backlog row).
- Known issue P2, a label box sized to its own text committing an ulp short (`core/ui/layout/taffy.rs`, `place`): once fixed, the template's title width and its two `TextWrap::None` labels (step 7) can go.
- The step-1 API critique, not run (§9): its command runs on this tree once the Codex window allows.

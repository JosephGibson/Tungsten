# Phase 6 milestone 69: the template shell (W12 shell) — draft

- **status:** done (run 2026-10-19; W14b had not run, so Q1's default made the release 0.89 and every `0.90` below reads `0.89`)
- **goal:** `templates/basic` becomes the springboard every game starts from: a shell copied unchanged into each new game (title, pause, tabbed settings with a Display tab, keybindings, gameplay options, save and load over an autosave and three slots, game over, a confirm dialog, toasts and a HUD, in a dark, square-cornered theme), a sample game a new game deletes, and a save seam the game fills in. The engine gains an exit request with one last frame for quit and window close, and a frame-rate limit in the player's settings.
- **non-goals:** The shell in the engine or the kit (1.x, once the acceptance game and W12b's examples show identical copies); W12b's move of examples 01, 03 and 04 onto the shell; the template audit, a stop of its own after this one (Follow-ups); F9 and F11 persisting their change (a known issue); the monitor's size and video modes, stick deadzone and sensitivity, rumble, cut corners, light or high-contrast themes, a theme danger colour and game-defined settings fields (1.x backlog); music, menu sounds, a credits screen, developer shortcuts and a game-speed option (owner, 2026-10-19); settings and bindings load warnings on screen (Q6); localisation; a template past its line budget (§2.11).
- **files to touch:** Step 1: `crates/tungsten/src/app.rs`, `lib.rs`, `testing.rs`, `settings.rs`, `src/tests/app.rs`, `src/tests/testing.rs`, `src/tests/settings.rs`; `crates/tungsten-core/src/settings.rs`, `config.rs`, `lib.rs`, `src/tests/settings.rs`, `src/tests/config.rs`; `api/*.txt` (`just api`); `DECISIONS.md`, `docs/DECISION_INDEX.md`. Steps 2–5: `templates/basic/src/**` (`main.rs`, `lib.rs`, `game.rs`, `save.rs`, `shell/`, `sample/`; `states.rs` and `components.rs` removed), `templates/basic/input.json`, `templates/basic/assets/manifest.json`, `assets/ui/themes/shell.json`, `assets/prefabs/player.json`, `assets/data/tags.json`, `assets/data/cue_rules/labels.json`, `templates/basic/tests/game.rs` (moved to `tests/game/main.rs` with modules beside it); step 2 also `DECISIONS.md` and `docs/DECISION_INDEX.md`. Step 6: `templates/basic/tests/game/visual.rs`, `templates/basic/tests/fixtures/title.png`, `justfile` (`visual`). Step 7: `perf-runs/<date>-m69-exit/`. Step 8: `templates/basic/AGENTS.md`, `README.md`, `docs/getting-started.md`, `docs/plans/1.0/w12-template.md`, `docs/plans/1.0/w13-kit.md` (line 281), `docs/plans/1.0/backlog-1.x.md`, `docs/known-issues.md`, `DESIGN.md`, `docs/LLM_INDEX.md`. Step 9: the release files.
- **ordered steps:** (1) The exit lifecycle and the player's frame-rate limit. (2) The shell's core, the sample split and the save seam. (3) The save and load screen. (4) Tabbed settings and gameplay options. (5) Keybindings and the line budget. (6) The title's pixel test. (7) The perf sitting. (8) Docs and records. (9) Release 0.90.
- **done-when:** Every step's evidence row quoted; workflow §5 closed; the release checks, with `just template-check`, `just visual` and the perf sitting's verdicts, pass after the cut; the preflight prints the command block and the post-merge block.

Candidate W12 shell (the owner's request of 2026-10-19, settled over three rounds of questions in the plan session; no implementation-plan card, as the UI lab's M59 had none), level C, milestone M69, release 0.90 (it runs after W14b, which takes 0.89; Q1). Written 2026-10-19 at `860f6ed`. Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs.

## Context digest

The template is the layout every game follows (`D-123`, [w12](../../1.0/w12-template.md)), and the acceptance game starts from it (`D-102`). Its title and pause are text drawn at fixed 1280 × 720 positions, its save holds the player's position alone, and it uses none of the UI that M54–M59 built. The acceptance game's screens ([acceptance game](../../1.0/acceptance-game.md) §Screens, lines 33–38) are a title with start, settings and quit, a settings screen with volumes, rebinding and display, and a pause with resume, settings and quit to title, so every game would rebuild them. The owner asked on 2026-10-19 for a shell that later games reuse, ahead of W12b, so that W12b moves examples 01, 03 and 04 onto it.

It stands on the state-owned UI roots, transitions and binding-aware prompts (`D-163`, `D-164`), routing and modal roots (`D-149`), the theme asset (`D-148`), `Settings` and `apply_settings` (`D-159`, `D-169`), the buses (`D-179`), `SaveSlots` (`D-160`) and the kit's tags, cues, stats and effects (`D-172`, `D-173`, `D-177`). The UI lab's pause and settings states (`examples/05_ui_lab/src/menus.rs:236-320`) are the pattern the shell copies. Two engine gaps block a menu Quit and a save on close (A7, A8), and the player's settings lack the frame-rate limit (A9); step 1 closes them, as level C API. Everything else is template code.

Two decisions land: 180 (the exit lifecycle and the frame-rate limit, amending `D-159`) and 181 (the template as shell and sample, its budget and its seam, amending `D-123`). W12b's plan decides how the examples adopt the shell; the template audit stop follows this one.

## 1. Audit

Read at `860f6ed`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | The title, controls line and pause notice are `TextSection`s added through `Extracts::add_text`, at fixed positions sized for 1280 × 720, with no UI scale, focus or gamepad navigation. | `templates/basic/src/game.rs:108-135` (`y = 240`, `position: [96.0, y]` at :118, :128) |
| A2 | The hints name fixed keys ("Enter starts · Esc quits", "Arrows or WASD move · P pauses · M mutes", "Backspace or P resumes"), wrong after a rebind or on a pad. | `game.rs:112`, `:114`, `:115` |
| A3 | The autosave holds the player's position only; gameplay respawns every gem on entry and never resets the `gems` counter, so a continue or a restart after death counts past three. | `states.rs:42-44`, `:85-93`, `:97-100`; `Counters::set` exists (`crates/tungsten-kit/src/react/counters.rs:120`) |
| A4 | `toggle_mute` repeats the `just_pressed` helper that `states.rs` defines. | `game.rs:87-106`, `states.rs:209-217` |
| A5 | `write_autosave` queries `(&Position, &Transform)` and drops the `Transform`. | `states.rs:177-186` |
| A6 | A killed player restarts gameplay with no game-over screen. | `states.rs:97-100` |
| A7 | A game cannot end the run: the only exits are the `engine_exit` action (Escape by default) while `exit_on_escape` holds, read after `Update`, and the window's close button. A menu's Quit has no API. | `crates/tungsten/src/app.rs:459-461`, `:603-620`, `:1187`, `:1627-1629`; `crates/tungsten-core/src/input/action_map.rs:279-281` |
| A8 | The close button exits at once, so a game gets no frame to save. | `app.rs:2250-2252` |
| A9 | `DisplayState` carries `frame_rate_cap` and a display request changes it at runtime, but `Settings` holds display mode, resolution and vsync only, and `apply_settings` never sends a cap. | `crates/tungsten-core/src/display.rs:163-178`; `crates/tungsten-core/src/settings.rs:63-74`; `crates/tungsten/src/settings.rs:60-104`; `crates/tungsten-core/src/config.rs:587-607` |
| A10 | F9 and F11 change vsync and fullscreen through a display request and never write `Settings`, so their change is lost at restart and the next `apply_settings` replaces it. | `crates/tungsten/src/display.rs:58-100`; `settings.rs:28-31` (its doc says so) |
| A11 | Exclusive fullscreen runs as borderless at runtime. | `display.rs:39-46` |
| A12 | `App::new` takes and logs the settings', bindings' and placements' load warnings; later `take_load_warnings` calls return none, so a game cannot show them. `SlotList::warnings` reaches the caller. | `app.rs:204-229`, `:2003`; `crates/tungsten-core/src/save.rs:99-104` |
| A13 | `SlotInfo` holds the name, schema, engine version and save time, nothing the game chose. | `save.rs:85-95` |
| A14 | `Settings` has no section for game-defined values. `SaveSlots::new(dir)` is public, so a second store over another folder of the user folder works. | `settings.rs:63-74`; `save.rs:216-224` |
| A15 | The kit's `Cues` are opaque (no edit after build), but a `cue_rules` entry filters on the subject's tags, and `Tags` has `insert` and `remove`: a tag the shell sets decides whether the damage label floats. | `crates/tungsten-kit/src/react/cues.rs:134-138`; `crates/tungsten-kit/src/foundation/tags.rs:135`, `:151`; `crates/tungsten/tests/fixtures/kit/top_down/data/cue_rules/combat.json` (a `damaged` rule with a tag filter and a `label`) |
| A16 | A `max_health` stat writes `Health::set_max`, which keeps the fraction left (5 of 5 at a max of 8 becomes 8 of 8), so a difficulty effect needs no health fix-up. `Health` derives `Serialize` and `Deserialize`, so a save can hold it. | `crates/tungsten-kit/src/foundation/stats.rs:269`, `:782-783`; `crates/tungsten-kit/src/damage/health.rs:95-97`, `:301-306` |
| A17 | Screen shake's peak offset is a public field of `CameraController`. | `crates/tungsten-core/src/camera.rs:101` |
| A18 | `apply_settings` sets the master volume through `AudioCommands`; `InputState::focused` reports the window's focus. | `crates/tungsten/src/settings.rs:46-51`; `crates/tungsten-core/src/input.rs:1181` |
| A19 | The UI router captures the next key, button or stick for a widget, Escape included, and `cancel_capture` ends one; a modal root takes every key and pad press. | `crates/tungsten-core/src/ui/route.rs:293-311`, `:1105-1113` |
| A20 | `StateStack` has push, pop and replace, each with a transition form that refuses while a transition is queued or active; there is no clear. | `crates/tungsten/src/state.rs:154-208` |
| A21 | The UI shader draws rounded boxes only; radius 0 is a square. The theme file holds nine colour tokens, radii and per-kind state overrides of background, border, border width, radius and text. | `crates/tungsten-render/src/ui.wgsl:84-93`; `crates/tungsten-core/src/ui/theme_file.rs:23-33`, `:73-80`, `:104-110` |
| A22 | The UI lab's pause, settings and theme helpers are the pattern: a state owns a modal root, acts only when no transition runs, and gives focus back to the button that opened the screen above. | `examples/05_ui_lab/src/menus.rs:236-320`; `examples/05_ui_lab/src/theme.rs:98-134`; `examples/05_ui_lab/src/widgets.rs` |
| A23 | `docs/LLM_INDEX.md` holds 12,282 of its 12,288 bytes; the template's row names `game.rs` and `states.rs`. | `wc -c docs/LLM_INDEX.md`; `docs/LLM_INDEX.md:85` |
| A24 | The template's schedule is a snapshot test; any system the shell adds moves it. | `templates/basic/tests/game.rs:148`, `:154-159` |

## 2. Design

### 2.1 Engine API (step 1, level C)

In the umbrella, beside `set_exit_on_escape` in `app.rs` (no new file, so no index row; A23):

```rust
/// Why the run is ending; [`Exiting`] carries it through the last frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExitReason {
    /// The game called [`request_exit`].
    Requested,
    /// The window's close button, or the OS, asked the window to close.
    CloseRequested,
}

/// A resource present for the run's last frame only: a system that saves
/// on quit reads it. Every stage runs in that frame, render included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Exiting {
    pub reason: ExitReason,
}

/// Ends the run after one more frame, which runs with [`Exiting`]. A second
/// call, or one during the last frame, changes nothing.
pub fn request_exit(world: &mut World);
```

- **Lifecycle.** `request_exit` sets a crate-private `PendingExit`. At the end of `run_frame`, after the frame's work: with `Exiting` present the frame returns a new crate-private `FrameEnd::Last`; otherwise a `PendingExit` is taken and `Exiting { reason: Requested }` inserted, so the next frame is the last. `FrameEnd::ExitRequested` keeps its meaning, a frame `engine_exit` cut short. Both the window loop and `Harness::step` call `run_frame` (`crates/tungsten/src/testing.rs:155-172`): the window presents a `Last` frame, then exits; the harness keeps its draw, audio and count as for a completed frame, then sets `exit_requested`, since today's `ExitRequested` branch returns before keeping them (`testing.rs:165-168`; round 1, finding 6).
- **Close.** `WindowEvent::CloseRequested` inserts `Exiting { reason: CloseRequested }` (keeping an `Exiting` already there), runs one frame at once from the event, on the wall clock, and exits whatever it returns. A minimized window that receives no redraw still exits. With no renderer yet, it exits at once, as today.
- **Unchanged.** `engine_exit` still ends its frame after `Update` with no last frame (`app.rs:1627-1629`; the test `a_pressed_engine_exit_binding_ends_the_frame_after_update`): it is the developer's hatch, and the template turns it off. `TUNGSTEN_SMOKE_FRAMES` exits as today (`app.rs:1738-1746`).
- **Harness.** `Harness::request_close(&mut self)` inserts the close's `Exiting`, so the next `step` is the last frame; `exit_requested` reads true after any of the three and `step` returns early from then on, as after `engine_exit`.

In core, beside `Settings` (`D-007`: no window types):

```rust
/// A frame-rate limit the player chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameRateCap {
    /// No limit, over a shipped one.
    Uncapped,
    /// At most this many frames a second.
    Fps(NonZeroU32),
}

impl Settings {
    pub fn frame_rate_cap(&self) -> Option<FrameRateCap>;
    pub fn set_frame_rate_cap(&mut self, cap: Option<FrameRateCap>);
}
```

- **File.** `"display": { "frame_rate_cap": 60 }` or `"frame_rate_cap": "uncapped"`, absent when unset. Any other value (0, a negative, another string, a float) makes the file invalid, quarantined as `.bad` with defaults, as an unknown `display_mode` is (`settings.rs:108-146`, `:386-430`).
- **Apply.** `Config::apply_settings_with` sets `display.frame_rate_cap` (`Uncapped` → `None`, which resolves uncapped; `Fps(n)` → `Some(n)`), so startup layers it as the other display fields (`config.rs:587-607`); `TUNGSTEN_DISPLAY_FRAME_RATE_CAP` still wins (`D-159`'s order). `apply_settings` copies the resolved cap into the display request in both branches (`crates/tungsten/src/settings.rs:60-104`).

Additive only: no break-ledger row. Decision 180 records both.

### 2.2 Template layout

```text
templates/basic/
  src/main.rs            Config, App, set_exit_on_escape(false), game::register(&mut app, info), run
  src/lib.rs             pub mod game, save, shell, sample
  src/game.rs            the seam's names (§2.3) and register(): KitPlugins, ShellPlugin, SamplePlugin
  src/save.rs            SaveGame (schema 2, migrate from 1), capture, summary
  src/shell/             copied unchanged into a new game
    mod.rs               ShellPlugin, GameInfo, RunState, goto, autosave, focus, exit save, input helpers
    theme.rs             theme ID, motion, fade and slide helpers
    widgets.rs           heading, button, rows (choice, slider, toggle), tab row, prompt row
    confirm.rs           ConfirmState: a modal question with two answers, an optional countdown
    toast.rs             Toasts: short messages on an unowned root, on the real clock
    title.rs             TitleState and the difficulty picker
    pause.rs             PauseState
    game_over.rs         GameOverState
    saves.rs             SavesState in Save or Load mode
    settings.rs          SettingsState: Display, Audio, Interface, Controls, Gameplay
    keybindings.rs       KeybindingsState
    options.rs           GameOptions and their store
  src/sample/            the demo a new game deletes
    mod.rs               SamplePlugin, GameplayState, spawn and restore, difficulty
    components.rs        Player, GemSlot
    hud.rs               the HUD root and its system
  assets/ui/themes/shell.json
  tests/game/main.rs     helpers and the modules: sample, shell, saves, settings, keys, fit, visual
```

`AGENTS.md` keeps its feature-folder rule: a new game deletes `src/sample/`, writes its features under `src/features/<name>/`, and edits the seam's names in `game.rs` and `save.rs`.

### 2.3 The seam

The shell calls six names, and nothing else, outside `src/shell/`:

| Name | Where | What the shell does with it |
| --- | --- | --- |
| `game::GAMEPLAY` | `game.rs` | The gameplay state's ID: pause opens over it, focus loss pauses it |
| `game::gameplay(start: RunStart) -> impl GameState` | `game.rs` | The state `goto` puts on the stack for New Game, Continue, Load and Retry; `RunStart` carries the difficulty and `Option<SaveGame>` |
| `game::BINDABLE_ACTIONS: &[(&str, &str)]` | `game.rs` | The Keybindings screen's rows: action name and label |
| `save::SaveGame` | `save.rs` | A `SaveData` holding `meta: shell::RunMeta` (game version, difficulty, playtime) and the game's own fields |
| `save::capture(world) -> Option<SaveGame>` | `save.rs` | Every save; `None` (no live run, no player, a dead player) saves nothing |
| `save::summary(&SaveGame) -> String` | `save.rs` | The slot row's one line |

The sample calls two shell functions: `shell::game_over(world)` when its player dies and `shell::run(world) -> Option<&RunState>` for the difficulty. The sample keeps the entities it spawned in a `SampleRun { player, gems: Vec<(u8, Entity)> }` resource, and its death check, HUD and `capture` read those handles alone (`capture` lists the gems whose entity `World::is_alive` still holds, never a `GemSlot` query; round 2, finding 1): a replace queues the old run's despawns for the frame's command flush (`crates/tungsten/src/state.rs:256-268`, `:492-494`) while a prefab spawns at once (`crates/tungsten-core/src/assets/prefab.rs:344-348`), so for the rest of that frame the old, dead player still exists beside the new one (round 1, finding 2). The camera may see both targets for that one frame; the fixed step, which runs before the dispatcher, never does. `GameInfo` (title from `tungsten.json`'s `window.title`, read in `main.rs` before `App::new` consumes the config, and `CARGO_PKG_VERSION`) goes to `register`, so `tungsten new` renames the title screen with the window.

### 2.4 Screens and flow

| Screen | State | Entries | Notes |
| --- | --- | --- | --- |
| Title | `title` | Continue, New Game, Load, Settings, Quit | The title and version from `GameInfo`. Continue is focused when a readable slot exists and disabled otherwise, New Game focused then. Continue loads the newest readable slot by `saved_at_ms`. New Game opens the difficulty picker (Easy, Normal, Hard, Back; Normal focused); when an autosave exists the pick asks first ("Start a new game? The autosave will be replaced.") |
| Pause | `pause` | Resume, Save, Load, Gameplay Options, Settings, Quit to Title, Quit Game | Pauses the game clock (as now, `states.rs:116-155`), autosaves on its first frame with no fixed step (as now). `ui_cancel`, Resume or `state_pause` closes it, once per frame (§2.10) |
| Game over | `game_over` | Retry, Load, Quit to Title | Pushed by the sample over gameplay when the player dies; holds the game clock. Retry loads the run's last good save, or starts the run fresh at its difficulty when it has none (§2.5) |
| Save / Load | `saves` | Autosave, Slot 1–3, Back | §2.5 |
| Settings | `settings` | Tabs: Display, Audio, Interface, Controls, Gameplay; Reset tab; Back | §2.6; pause's Gameplay Options opens it on Gameplay |
| Keybindings | `keybindings` | One row per bindable action; Reset all; Back | §2.7 |
| Confirm | `confirm` | Two answers, the safe one focused; `ui_cancel` picks it | Modal; an optional countdown on the real clock picks the safe answer when it ends |

`shell::goto(world, state)` changes the run: a screen calls it from its `update`, which only stores the request in a `ShellFlow` resource, since the dispatcher takes the updating state off the stack while it runs, so `depth()` and `active_id()` there see the stack without it (`crates/tungsten/src/state.rs:293-296`, `:236-246`; round 1, finding 4). The shell's `shell_flow` system, in `Update` after `state_dispatcher`, reads the whole stack and queues a plain pop for each state above the bottom one and a replace of the bottom one, so loading from [gameplay, pause, saves] leaves one gameplay (A20: no clear). Menu screens push and pop behind `theme::fade()` as the lab's do; `goto`'s new screen fades its own root in (`theme::slide_in`), since a transition request refuses while others are queued (Q4). Quit to Title autosaves and goes to the title; it asks first only when the autosave is unavailable (no user folder, or read-only, §2.5). Quit Game and the title's Quit call `request_exit`; the shell's exit system autosaves in the last frame. A window close does the same.

The HUD is a root gameplay owns with `RootCover::Show` (frozen and drawn under pause): a health bar with "current / max", "Gems n / 3", and a prompt row (`{action:state_pause} Pause   {action:toggle_mute} Mute`) that `D-164`'s prompts resolve for the last device. Toasts sit on an unowned root, so a state change keeps them.

### 2.5 Saves

- **Slots.** `autosave` and `slot_1`, `slot_2`, `slot_3` in `SaveSlots` (names within `save.rs:116`'s rule). Each row reads the slot: empty; ready ("Slot 2 · 3 min ago · 12:04 played · v0.1.0", then `summary`); "Saved by a newer version"; or "Damaged". A newer or damaged slot can't be loaded or overwritten for the run (today's autosave rule, `states.rs:47-53`, per slot) and can be deleted after a confirm. The row names its state in text, never by colour alone.
- **Save mode** (from pause): the autosave row is shown disabled ("Written when you pause or quit"); an empty slot saves at once and toasts "Saved to Slot n"; a used slot asks "Overwrite Slot n?". **Load mode** (title, pause, game over): a ready slot loads through `goto(gameplay(RunStart::from(save)))`. From pause it asks "Load Slot n? Progress since your last save will be lost." unless the run is checkpointed: `RunState::checkpointed` is set by an autosave or save that succeeded and cleared by the next fixed step that runs. A read-only autosave or a failed write (`crates/tungsten-core/src/save.rs:249`) therefore always asks (round 1, finding 1). From the title and game over it never asks: there is no live run, or its player is dead.
- **Autosave triggers:** pause's first settled frame, Quit to Title, the exit's last frame and a new run's first frame.
- **Retry's save:** `RunState::retry_from` names the slot this run last loaded or saved successfully: the loaded slot at a load, then the autosave once one of this run's autosaves succeeds. Retry loads it, or starts the run fresh when it is unset. A failed write keeps the slot's old file (`crates/tungsten-core/src/save.rs:249`), so a New Game whose first autosave fails leaves `retry_from` unset and Retry never loads the previous run's autosave (round 2, finding 2). Never on the title, during game over or with a dead player (`capture` returns `None`). With no user folder every save path is off and its buttons are disabled with "Saving is off".
- **SaveGame schema 2:** `meta: RunMeta { game_version, difficulty, playtime_secs }`, `player: [f32; 2]` (the body's `Position`), `health: Health` (A16), `gems_left: Vec<u8>` (indices into `GEM_STARTS`, through a `GemSlot` component) and `gems: i64` (the `gems` counter). `migrate(1, value)` turns `{ "player": [x, y] }` into schema 2 with Normal difficulty, no playtime, all three gems, a count of 0, full health and `game_version` "unknown". A new run sets the `gems` counter to 0 (A3).
- **Restore:** the sample spawns the player at `player`, sets its `Stats` base `max_health` for the difficulty at once (§2.8), puts the saved `Health` in the prefab's `with(..)`, spawns only the gems in `gems_left`, and sets the counter. The first `stats_apply` then finds the max already at the difficulty's value, so `set_max` changes nothing and a hit in the first step lands on the saved health (A16).
- **Difficulty precedence:** `RunStart` carries the difficulty the run starts at. Retry takes `RunState`'s difficulty, so a change made in the Gameplay tab after the last save survives a death; a load from the save screen and Continue take the slot's (round 1, finding 5).
- **Playtime:** `RunState` adds `Time::delta()` each `Update` while a run is live; the game clock is zero while paused.
- **List warnings:** `SlotList::warnings` from the title's listing become one toast ("1 save file could not be read"); settings and bindings warnings stay in the log (A12, Q6).

### 2.6 Settings tabs

Every change applies at once and is written when the screen closes (Back or `ui_cancel` saves `Settings` and `GameOptions`), as the lab does. A tab's Reset restores that tab's values to unset or default.

| Tab | Row | Store | Notes |
| --- | --- | --- | --- |
| Display | Window mode: Windowed / Fullscreen | `Settings::set_display_mode` | Fullscreen is `BorderlessFullscreen`; exclusive is not offered (A11). The row shows the live `DisplayState`, so an F11 press shows (A10) |
| Display | Window size: 1280×720, 1600×900, 1920×1080, 2560×1440 | `set_resolution` | Disabled in fullscreen (`D-169` applies it on the return to windowed) |
| Display | VSync | `set_vsync` | |
| Display | Frame rate limit: Off, 30, 60, 120, 144 | `set_frame_rate_cap` (step 1) | |
| Display | — | — | A mode or size change opens the confirm "Keep these display settings?" with a 10 s countdown; Revert, or the countdown's end, restores the previous values and applies them |
| Audio | Master, Music, Effects (0–100 %) | `set_master_volume`, `set_bus_volume` | |
| Audio | Mute when unfocused | `GameOptions` | Sends `AudioCommands::set_master_volume(0.0)` on focus loss and the setting's volume on return; `Settings` is never written (A18). Every shell apply goes through `shell::apply_settings(world)`. While the focus mute holds, it hands the engine's `apply_settings` a copy of `Settings` with the master volume at 0 and puts the player's back after, so the only master volume queued is 0: the engine's sends the set volume (`crates/tungsten/src/settings.rs:46-51`), and `AudioCommands` appends each command, so a correcting 0 after it could still let the set volume play for a callback (`crates/tungsten-core/src/audio.rs:207-210`; round 1, finding 7; round 2, finding 3). The engine never calls `apply_settings` itself |
| Interface | UI scale, Text scale | `set_ui_scale`, `set_text_scale` | Ranges from `Settings::UI_SCALE_RANGE`, `TEXT_SCALE_RANGE` |
| Controls | Keybindings…, Reset all bindings | the action map's user layer | Opens §2.7 |
| Gameplay | Difficulty | the run's `RunState` | Disabled outside a run ("Chosen at New Game"); a change applies from the run's next start, Retry included (§2.5's precedence), and is saved with the run's next save (Q5) |
| Gameplay | Screen shake (0–100 %) | `GameOptions` | `CameraController::shake_max_offset` = the shipped offset × the value (A17) |
| Gameplay | Damage numbers | `GameOptions` | The sample inserts or removes the tag `show.damage_numbers` on its player; the `labels` cue rule floats the label only with it (A15) |
| Gameplay | Pause when unfocused (on by default) | `GameOptions` | Focus loss with gameplay on top and no transition pushes pause |

Tabs are a row of buttons; `ui_next` and `ui_prev` (Tab, the shoulders) step through them, and the selected tab shows a 3-unit accent bar under it (a child panel the code shows, since "selected" is no theme state). Each tab's panel is built once and hidden with `set_visibility`. `GameOptions` is a `SaveData` (schema 1, `serde(default)` on every field) kept as the slot `options` of `SaveSlots::new(user_folder/options)` (A14); a damaged file logs, toasts and falls back to defaults.

### 2.7 Keybindings

- Rows from `game::BINDABLE_ACTIONS` (the template's: move up, down, left, right, pause, mute); `ui_*` and `engine_*` actions are never listed or changed, so the menus can't be lost. Overlap with them is allowed (move up and `ui_up` share the arrow keys by design).
- Columns: Primary (the action's first key or mouse binding), Alternate (its second), Gamepad (its first pad button or stick direction); a cell shows `Binding::label` or "—". A row ends with Reset.
- Activating a cell captures the next press (A19) with the prompt "Press a key… (5 s)"; the capture cancels after 5 s on the real clock. A press of the wrong device class for the column is refused with a toast and the old binding kept. A binding another bindable action holds asks "Already used by Move up. Swap?"; Swap gives that action this cell's old binding (or removes it when empty). Each change goes through `replace_bindings_and_persist`, which keeps the action's other bindings.
- Reset and Reset all read the shipped bindings from `input.json` (`ActionMap::load` and `merged_with_defaults`) and persist them.
- The HUD's and the menus' prompts read the map, so a rebind shows on the next routing.

### 2.8 Gameplay data

- **Difficulty:** the player prefab gains `"max_health": 5` in its `stats`; at spawn the sample sets that base to 8 (Easy), 5 (Normal) or 3 (Hard) with `Stats::set_base` (`crates/tungsten-kit/src/foundation/stats.rs:347`), before any step. An effect would not do: a modifier placed in a step's resolve reaches health through the next step's `stats_apply` (`crates/tungsten-kit/src/damage/effects.rs:2151-2153`), so the first step would run at 5 (round 1, finding 3).
- **Damage numbers:** the player prefab's `damaged` cue keeps its `shake` and drops its `label`; `data/cue_rules/labels.json` holds `{ "on": "damaged", "filter": { "all": ["team.player", "show.damage_numbers"] }, "do": [{ "label": { "font": "sans" } }] }`; `tags.json` declares `show.damage_numbers`.
- **Pause key:** `input.json` gains `state_pause` (P, Escape, the pad's Start), since Escape is free in gameplay once `engine_exit` is off.

### 2.9 Theme

`assets/ui/themes/shell.json` (manifest `ui_themes`), dark and square. Tokens, with WCAG contrast computed from the opaque sRGB values (no translucent panel; a translucent veil would be composited in linear light first, as M58 learned):

| Token | Value | Against | Ratio |
| --- | --- | --- | --- |
| background | `#0b0c10` | | |
| surface | `#15171d` | | |
| text | `#e9ebf1` | surface | 15.0 |
| text_muted | `#a3a9b6` | surface | 7.6 |
| accent | `#f0a830` | surface | 8.8 |
| accent_text | `#0b0c10` | accent | 9.6 |
| focus | `#ffd36b` | surface | 12.6 |
| border | `#646b7a` | surface | 3.35 (non-text, ≥ 3) |
| disabled_text | `#6c7280` | surface | 3.7 (WCAG exempts disabled) |

Every radius is 0. Panels and buttons draw a 1-unit border; a focused button's border takes `focus` at 2 units over a `#1e2129` raised background; a pressed or selected button fills with `accent` and `accent_text`. Motion is short and flat: fades of 0.08 s, slides of 0.12 s over 8 units with `CubicOut`, no overshoot. The font stays Inter Regular; headings are larger, not heavier. A test parses the file and checks every ratio above and every radius.

### 2.10 Input rules

- A modal root takes every key and pad press (A19), so a screen reads its own hotkeys (`state_pause` in pause) from the raw `InputState` behind guards: no transition and no capture. Only the top state's `update` runs (`crates/tungsten/src/state.rs:293-296`), so a screen under a confirm never sees the press. A frame acts once: pause's Escape is both `ui_cancel` and `state_pause`, and pops one state.
- Screens act only when no transition runs, as the lab's do. Scripted tests wait out each transition before the next press.
- Every shell screen is modal, and a modal root takes and routes the first key at once, activation included (`crates/tungsten-core/src/ui/route.rs:1105-1106`, `:1131-1134`), so `D-149`'s first-key-after-a-click rule does not apply to them and tests expect the first press to act (round 1, finding 8). The rule stays for non-modal UI, which the shell has none of (the HUD takes no input).
- The shell's numeric setters (volume percent, shake, scales) clamp to their ranges and ignore non-finite input, each with a test.

### 2.11 Budget

`D-123`'s "a few hundred lines" becomes: `src/shell/` at most 2,000 lines of code and `src/` at most 2,600, as `tokei` counts code (no comments or blanks). Tests have no cap. A feature past the budget belongs in the acceptance game. Decision 181 records it with the seam.

### 2.12 Left out on purpose

Recorded as follow-ups (Follow-ups): the monitor's size, a theme danger token, game summaries in slot headers, per-action labels in `input.json`, game-defined `Settings` fields, stick deadzone and sensitivity, startup warnings for games, and F9/F11 persistence. Rumble and the broader controls already have backlog rows.

## 3. Steps

### Step 1: The exit lifecycle and the player's frame-rate limit

- **Files:** `crates/tungsten/src/app.rs`, `lib.rs`, `testing.rs`, `settings.rs`, `src/tests/app.rs`, `src/tests/testing.rs`, `src/tests/settings.rs`; `crates/tungsten-core/src/settings.rs`, `config.rs`, `lib.rs`, `src/tests/settings.rs`, `src/tests/config.rs`; `api/*.txt`; `DECISIONS.md`, `docs/DECISION_INDEX.md`
- **Change:** §2.1, in order: `FrameRateCap` with the settings field, file form and validation; `Config::apply_settings_with`; `apply_settings`'s two branches; `ExitReason`, `Exiting`, `request_exit` and `PendingExit` with the check at the end of `run_frame`; the close handler's last frame; `Harness::request_close` and the `exit_requested` doc; rustdoc on each new public item; decision 180 with its index row (tungsten-decision). Then `just api`, and the level C critique of the `api/` diff (the skill's command), its findings as `C.<n>` in §9.
- **Done-when:**
  - `cargo test -p tungsten --lib --locked -- exiting` → ok, at least 6 passed: `exiting_follows_request_exit_for_one_frame` (the frame after the call holds `Exiting { reason: Requested }`, every stage ran in it, `exit_requested` reads true after it and a further `step` runs no frame), `exiting_keeps_the_last_frames_draw_audio_and_count` (`step(5)` with a request in frame 2 returns 3, `frame()` counts the last frame, and `draw()` and `audio()` are the last frame's), `exiting_follows_a_close_for_one_frame` (`request_close`), `exiting_ignores_a_second_request_and_one_in_the_last_frame`, `exiting_is_absent_from_ordinary_frames` (600 frames), `exiting_never_follows_engine_exit`.
  - `cargo test -p tungsten --lib --locked -- a_pressed_engine_exit_binding_ends_the_frame_after_update` → 1 passed.
  - `cargo test -p tungsten-core --lib --locked -- player_frame_cap` → ok, at least 4 passed: `60` and `"uncapped"` round-trip through `save` and `load`; `0`, `-1`, `"fast"` and `59.5` each quarantine the file; `Config::apply_settings` gives `Some(30)` for `Fps(30)` and `None` for `Uncapped` over a shipped 60; `TUNGSTEN_DISPLAY_FRAME_RATE_CAP` still wins.
  - `cargo test -p tungsten --lib --locked -- player_frame_cap` → ok, at least 2 passed: `apply_settings` queues a display request with `frame_rate_cap: Some(30)`, and an unset cap returns to the shipped one.
  - `cargo test -p tungsten-core --locked` and `cargo test -p tungsten --locked` → ok.
  - `cargo clippy -p tungsten-core -p tungsten --all-targets --locked -- -D warnings` → exit 0; `cargo fmt --all -- --check` → exit 0.
  - `git diff --unified=0 -- api/ | rg -c '^-[^-]'` → no match (exit 1), and the diff shows `request_exit`, `Exiting`, `ExitReason`, `Harness::request_close`, `FrameRateCap` and the two `Settings` methods.
  - `rg -c '^## D-18[0] — ' DECISIONS.md` → 1; `rg -c '^\| `D-18[0]` \|' docs/DECISION_INDEX.md` → 1.
  - `just smoke` → passes.
- **Moves:** none. The perf sitting (step 7) judges the frame-end check.

### Step 2: The shell's core, the sample split and the save seam

- **Files:** `templates/basic/src/main.rs`, `lib.rs`, `game.rs`, `save.rs`, `shell/mod.rs`, `theme.rs`, `widgets.rs`, `confirm.rs`, `toast.rs`, `title.rs`, `pause.rs`, `game_over.rs`, `options.rs`, `sample/mod.rs`, `components.rs`, `hud.rs`; `src/states.rs` and `src/components.rs` removed; `templates/basic/input.json`; `assets/manifest.json`, `assets/ui/themes/shell.json`, `assets/prefabs/player.json`, `assets/data/tags.json`, `assets/data/cue_rules/labels.json`; `tests/game.rs` moved to `tests/game/main.rs` with `sample.rs`, `shell.rs`, `saves.rs` and `fit.rs`; `DECISIONS.md`, `docs/DECISION_INDEX.md`
- **Change:** decision 181 first (tungsten-decision). Then §2.2–§2.5, §2.8–§2.10 for the title (with the difficulty picker), pause (Resume, Save and Load disabled until step 3, Gameplay Options and Settings disabled until step 4, Quit to Title, Quit Game), game over, confirm, toasts, the HUD, focus loss (pause; the mute and the option store land here, their rows in step 4), the exit save, `goto` with `shell_flow` after `state_dispatcher`, and `SampleRun`. The gameplay tests move to `sample`, the pause and continue tests to `shell` and `saves`; the text test goes, the schedule snapshot is updated.
- **Done-when:**
  - `test ! -e templates/basic/src/states.rs && test ! -e templates/basic/src/components.rs` → exit 0.
  - `rg -c 'TextSection|add_text' templates/basic/src` → no match; `rg -n 'WASD|Backspace|Esc quits|Enter starts' templates/basic/src` → no match; `rg -c 'set_exit_on_escape\(false\)' templates/basic/src/main.rs` → 1.
  - `cargo test -p tungsten-template-basic --locked` → ok, 0 failed; with the filter `-- sample::` at least 16 passed (the 14 gameplay tests ported, plus `a_new_run_resets_the_gem_count` and `difficulty_sets_the_players_max_health`: 8/8, 5/5, 3/3 after the first fixed step).
  - `-- shell::` at least 20 passed: `title_focuses_continue_only_with_a_save`, `new_game_picks_a_difficulty_and_writes_a_fresh_autosave`, `new_game_asks_first_over_an_autosave`, `state_pause_opens_pause_and_holds_the_game_clock`, `pause_closes_once_on_escape_resume_or_state_pause` (depth 1 after one Escape), `quit_to_title_autosaves_and_returns_to_the_title`, `quit_to_title_from_a_screen_update_leaves_only_the_title` (depth 1), `quit_to_title_asks_first_when_the_autosave_is_unavailable`, `a_killed_player_opens_game_over_and_holds_the_clock`, `retry_loads_the_runs_last_good_save`, `retry_starts_fresh_when_the_runs_first_autosave_failed` (a readable autosave from an earlier run, a read-only saves folder, New Game, death, Retry: three gems and full health), `retry_with_the_old_dead_player_still_queued_stays_in_gameplay` (the frame of the replace and the next), `retry_autosaves_only_the_new_runs_gems` (an outgoing run with three gems, a save with one: the autosave on disk after Retry lists one), `quit_game_requests_exit_and_the_last_frame_autosaves`, `a_close_in_gameplay_autosaves_in_the_last_frame`, `no_autosave_on_the_title_or_while_dead`, `losing_focus_pauses_gameplay`, `the_hud_shows_health_gems_and_the_bound_pause_key`, `the_theme_meets_contrast_and_has_square_corners`, `a_toast_leaves_on_the_real_clock_under_a_pause`.
  - `-- saves::` at least 5 passed: `a_schema_1_autosave_migrates` (a schema-1 `SaveData` written through `SaveSlots`, then loaded as `SaveGame`), `continue_restores_position_health_gems_and_count`, `a_restored_player_takes_a_first_step_hit_at_its_saved_health` (Easy, saved 6 of 8: one hit in the first fixed step leaves 5 of 8), `a_slot_that_fails_to_load_is_never_overwritten`, `options_keep_their_own_folder_and_a_damaged_file_falls_back`.
  - `-- fit::` at least 3 passed: the title, pause and game over at 731 × 411 and 1920 × 1080 UI units, every focusable widget's rect inside the viewport (`UiHarness::rect`).
  - `cargo clippy -p tungsten-template-basic --all-targets --locked -- -D warnings` → exit 0; `cargo fmt --all -- --check` → exit 0.
  - `rg -c '^## D-18[1] — ' DECISIONS.md` → 1; `just repo-check` → passes.
  - `just smoke` and `just template-check` → pass.
- **Moves:** the template's schedule snapshot and its save schema (1 → 2, migrated).

### Step 3: The save and load screen

- **Files:** `templates/basic/src/shell/saves.rs`, `mod.rs`, `title.rs`, `pause.rs`, `game_over.rs`; `tests/game/saves.rs`, `fit.rs`
- **Change:** §2.5's screen in both modes, enabled from the title, pause and game over; Continue's newest-slot pick; the confirms; the list-warnings toast.
- **Done-when:**
  - `cargo test -p tungsten-template-basic --locked -- saves::` → ok, at least 15 passed, adding `the_load_screen_lists_the_autosave_and_three_slots_with_summaries`, `saving_to_an_empty_slot_writes_it_and_toasts`, `overwriting_asks_first_and_cancel_keeps_the_old_save`, `deleting_asks_first_and_empties_the_slot`, `a_damaged_or_newer_slot_shows_its_label_and_is_neither_loaded_nor_overwritten`, `loading_from_pause_leaves_one_gameplay_state` (depth 1, the saved position, health and count), `loading_from_pause_asks_first_unless_the_run_is_checkpointed` (a read-only autosave, and a write that fails because the saves folder is read-only, each ask; a checkpointed run does not), `continue_loads_the_newest_slot`, `without_a_user_folder_save_and_load_are_off`, `the_autosave_row_is_not_a_save_target`.
  - `-- fit::` → the saves screen in both modes fits at both sizes.
  - Template clippy and fmt → exit 0; `just template-check` → passes.
- **Moves:** none.

### Step 4: Tabbed settings and gameplay options

- **Files:** `templates/basic/src/shell/settings.rs`, `options.rs`, `mod.rs`, `pause.rs`, `title.rs`; `src/sample/mod.rs`; `tests/game/settings.rs`, `fit.rs`
- **Change:** §2.6 and §2.8's options; pause's Gameplay Options and Settings and the title's Settings enabled.
- **Done-when:**
  - `cargo test -p tungsten-template-basic --locked -- settings::` → ok, at least 18 passed: `a_display_revert_keeps_the_focus_mute` (focus lost during the countdown, which then ends: every `SetMasterVolume` in each frame's `Harness::audio`, stepped one frame at a time from the focus loss to its return, reads 0), `retry_keeps_a_difficulty_changed_mid_run` (Normal → Hard after the pause autosave, resume, die, Retry: 3 of 3), `tabs_switch_with_the_tab_row_and_ui_next`, `gameplay_options_opens_on_the_gameplay_tab`, `a_window_mode_change_reverts_after_ten_seconds`, `keep_confirms_the_display_change`, `window_size_is_disabled_in_fullscreen`, `the_frame_rate_limit_writes_settings_and_the_display_request`, `volumes_write_settings_and_audio_commands`, `ui_and_text_scale_apply`, `screen_shake_scales_the_camera`, `damage_numbers_off_floats_no_label` (a hit with the option off adds no world label), `difficulty_is_off_outside_a_run_and_saved_with_it`, `mute_when_unfocused_mutes_and_restores`, `pause_when_unfocused_off_keeps_playing`, `reset_tab_restores_only_that_tab`, `back_saves_settings_and_options_across_a_restart`, `shell_values_clamp_and_ignore_non_finite_input`.
  - `-- fit::` → every tab fits at both sizes.
  - Template clippy and fmt → exit 0; `just template-check` → passes.
- **Moves:** none.

### Step 5: Keybindings and the line budget

- **Files:** `templates/basic/src/shell/keybindings.rs`, `settings.rs`; `src/game.rs`; `tests/game/keys.rs`, `fit.rs`
- **Change:** §2.7; the Controls tab's rows enabled.
- **Done-when:**
  - `cargo test -p tungsten-template-basic --locked -- keys::` → ok, at least 7 passed: `every_bindable_action_is_listed_and_no_ui_or_engine_action`, `rebinding_primary_captures_a_key_and_survives_a_restart`, `a_key_column_refuses_a_pad_press`, `a_conflict_offers_swap_and_swap_exchanges_them`, `reset_one_and_reset_all_restore_input_json`, `a_capture_cancels_after_five_seconds`, `the_hud_prompt_follows_a_rebind`.
  - `-- fit::` → the keybindings screen fits at both sizes, its list scrolling at 411 units.
  - `test "$(tokei templates/basic/src/shell -o json | jq '.Total.code')" -le 2000` → exit 0; `test "$(tokei templates/basic/src -o json | jq '.Total.code')" -le 2600` → exit 0; both counts in the evidence row.
  - `cargo test -p tungsten-template-basic --locked` → ok, 0 failed; template clippy and fmt → exit 0; `just smoke` and `just template-check` → pass.
- **Moves:** none.

### Step 6: The title's pixel test

- **Files:** `templates/basic/tests/game/visual.rs`, `templates/basic/tests/fixtures/title.png`, `justfile`
- **Change:** a test gated by `TUNGSTEN_VISUAL_REGRESSION=1`, in the UI lab's shape (`examples/05_ui_lab/tests/ui_regression.rs:30-60`): it runs the template binary from its folder with `TUNGSTEN_SMOKE_FRAMES=24`, `TUNGSTEN_CAPTURE_FRAME=20` (after the title's slide), `TUNGSTEN_CAPTURE_RESOLUTION=1280x720` and a temporary `TUNGSTEN_CAPTURE_PATH`, and compares with the fixture at tolerance 0. The fixture is captured once on the reference machine. `just visual` gains the line.
- **Done-when:** `TUNGSTEN_VISUAL_REGRESSION=1 cargo test -p tungsten-template-basic --test game --locked -- visual:: --nocapture` → 1 passed; without the variable the test reports itself skipped (recorded as not run there); `just visual` → passes with every earlier fixture unchanged.
- **Moves:** adds the template's title fixture.

### Step 7: The perf sitting

- **Files:** `perf-runs/<date>-m69-exit/` (scripts, captures, compares, README)
- **Change:** step 1 adds a check at the end of `run_frame`, frame-loop code, so every CPU row is captured (workflow §5 line 2; [done-when rules](../../../perf/profiling-workflow.md#writing-done-when-checks)). The milestone sitting recipe: a `git archive HEAD` base under `target/`, prebuilt binaries, one unattended background job after the owner disconnects, a base A/A pair and base → candidate, compares, then `just smoke`, `just visual` and `just physics-release`. Rows chosen with [tungsten-perf](../../../../.claude/skills/tungsten-perf/SKILL.md).
- **Done-when:** the base A/A reads no owned metric `regressed`; base → candidate reads every CPU row's owned metrics not `regressed` and every digest unchanged; the gates after pass; the README names both trees.
- **Moves:** none.

### Step 8: Docs and records

- **Files:** `templates/basic/AGENTS.md`, `README.md`; `docs/getting-started.md`; `docs/plans/1.0/w12-template.md`, `w13-kit.md`, `backlog-1.x.md`; `docs/known-issues.md`; `DESIGN.md`; `docs/LLM_INDEX.md`
- **Change:** `AGENTS.md`: the layout (shell, sample, seam, budget), the save and options rules, the input rules of §2.10, and the kit rule split from one bullet into a short section of bullets. `README.md`: what the template does now and how to start a game from it (delete `src/sample/`, edit the seam). The guide's "Start a game from the template", "Settings and saves" and "How the game is laid out" sections. `w12-template.md`: the size non-goal and the upkeep done-when cite decision 181's budget; `w13-kit.md:281` likewise. The backlog rows of §2.12 and a known-issues row for A10. `DESIGN.md`'s frame loop: the exit's last frame. The index row for the template routes to `src/shell/`, `src/sample/` and `save.rs` in place of `states.rs`, inside the byte budget.
- **Done-when:** `wc -c < docs/LLM_INDEX.md` → at most 12288; `just ctx` → passes; `rg -c 'states\.rs' docs/LLM_INDEX.md templates/basic docs/getting-started.md` → no match; `rg -c 'few hundred' docs/plans/1.0/w12-template.md` → no match; `just repo-check` → passes.
- **Moves:** none.

### Step 9: Release 0.90

- **Files:** `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row, the README's four owned Now fields (`roadmap.py summary`), this plan (moved to `docs/plans/archive/`)
- **Change:** close workflow §5 and this step's evidence row with `status: done`; then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.90.0` once, every release check once after it with `just template-check` and `just visual`, then the preflight with `--message`.
- **Done-when:** [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut); `just release-preflight 0.90.0 --repo JosephGibson/Tungsten --message 'Update 0.90: M69 template shell, exit request and frame-rate limit'` → passes and prints the command block and the post-merge block.
- **Ends with:** the changed-file list, the command block, the post-merge block, then the next prompt: the template audit stop's plan prompt (Follow-ups 1), or W12b's if the owner drops the audit.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Which release? | 0.90, after W14b takes 0.89. If this milestone runs first, every `0.90` here reads `0.89` and the decisions take the next free IDs. |
| Q2 | Should `engine_exit` also get a last frame? | No: it stays the developer's hatch, immediate, and the template turns it off. A game that keeps it gets no exit save. |
| Q3 | Must the close's last frame render? | Yes: the frame runs whole, so a save or a toast is drawn; a render failure in it still exits. |
| Q4 | Can `goto` queue plain pops and then a replace behind a transition in one frame? | Not assumed: `goto` queues them plain and the new screen fades its own root in. The step's test checks the final stack. |
| Q5 | Does a difficulty change mid-run apply at once? | No: from the run's next start, saved with the run, and the row says so. Removing one effect instance mid-run is left to a game. |
| Q6 | Show settings and bindings load warnings? | No: `App::new` takes them (A12), so only slot warnings toast; a startup-warnings resource is a backlog row. |
| Q7 | A pixel test of the pause screen too? | No: it needs input before the capture, so a fixture hook in the template; the title covers the theme. |
| Q8 | Should the HUD hide while a screen covers gameplay? | No: `RootCover::Show` keeps it drawn and frozen, as the lab's play screen is. |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`).

## 5. Decisions expected

- 180: the exit lifecycle (`request_exit`, `Exiting` with `ExitReason`, a last frame for a requested exit and a window close, `engine_exit` unchanged, `Harness::request_close`) and the player's frame-rate limit (`FrameRateCap` in `Settings`, layered and applied as the other display fields); amends `D-159`.
- 181: the template as a shell and a sample: the shell copied into each game and kept out of the engine until 1.x, the six-name seam, the line budget in place of "a few hundred lines", the dark square theme, `GameOptions` in their own store, and the save rules (four slots, triggers, read-only per slot); amends `D-123`.

The next free IDs when written; checked again when each step runs, since W14b's plan may take 180 first.

## 6. Invariants

- **May move:** the template's schedule snapshot (step 2) and save schema (1 → 2 with its migration, step 2); the template's title fixture is new (step 6).
- **Must not move:** the determinism hash, the pinned containment hash, the row digests, the kit hash, `gpu-visual.png`, the post and transition regressions and the UI lab's fixtures; `engine_exit`'s frame and the smoke exit; every existing public item (`api/` removes nothing).

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with steps that don't depend on it. Steps 3–6 depend on step 2; steps 2, 6 and 7 depend on step 1.
- A screen cannot fit 731 × 411 units with its rows: record the screen, keep it scrolling, and continue; a screen that cannot scroll stops its step.
- The line budget fails at step 5: record the counts and the largest files, and stop before step 6, since the budget is decision 181's.
- The level C critique finds a flaw in step 1's API: change step 1 before step 2, and record it as `C.<n>`.
- The perf sitting reads a CPU row `regressed`: one rerun of that row; a second `regressed` stops the release until the check moves out of the frame path or a decision accepts it.
- A file outside §files to touch needs a change: record it and stop that step.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1–7, release | Template tests, clippy, fmt, smoke and template-check in steps 2–5; the CPU suite in 7; the release runs `just check`, `just smoke`, `just script-test`, `just template-check` and `just visual` once, after the cut |
| 3 Invariants | 6, 7 | `just visual` and the digests |
| 4 Decisions | 1, 2 | 180, 181 with their index rows |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1 | Additive, no ledger row; rustdoc on every new item |
| 6 Template and guide | 2–5, 8 | The milestone is the template change; the guide in 8 |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 8 | No new engine file; the template row rewritten inside the budget |
| 8 Design | 8 | The frame loop's last frame |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 8, release | A10 to known issues; §2.12 to the backlog; the acceptance game's Title, Settings and Pause rows are served by the shell (its gap log notes it); `w12-template.md`; `python3 -B scripts/roadmap.py summary` supplies the owned fields |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

The owner's ship sitting adds: close the window from the title bar in gameplay and see the autosave on the next Continue; close it minimized from the taskbar; play the shell with the keyboard and a pad at 1.75 scale.

## 9. Critique

Round 1: the `critique` skill with `--repo` (`gpt-6-astra`, effort max, context 872,000, output cap 4,000), 2026-10-19, on `860f6ed` with this plan uncommitted, §9 and the evidence log cut. 459.8 s; tokens in 1,627,143 (1,493,248 cached), out 16,556; 55 commands, 431.8 KB output (largest 23.7 KB); 5-hour window 0% → 37%, week 36% → 41%. Eight findings, all accepted.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] Load can discard progress when the pause autosave did not succeed | `crates/tungsten-core/src/save.rs:249` (a write failure returns); §2.5's read-only rule | accepted | §2.5: `RunState::checkpointed`, set by a save that succeeded and cleared by the next fixed step; a load from pause asks unless checkpointed. Step 3 adds `loading_from_pause_asks_first_unless_the_run_is_checkpointed` |
| 2 | [MEDIUM] Replacing gameplay leaves the previous run's entities visible to the new state | `crates/tungsten/src/state.rs:256-268` (despawns through the `CommandBuffer`), `:492-494`; `crates/tungsten-core/src/assets/prefab.rs:344-348` (spawn now); `templates/basic/src/states.rs:108-113` | accepted | §2.3: `SampleRun { player }`, read by the death check, HUD and `capture`; step 2 adds `retry_with_the_old_dead_player_still_queued_stays_in_gameplay` |
| 3 | [MEDIUM] Queued difficulty effects cannot satisfy the first-fixed-step contract | `crates/tungsten-kit/src/damage/effects.rs:2151-2153`; `stats.rs:782-783` | accepted | §2.8: difficulty sets the `max_health` base with `Stats::set_base` at spawn; the effect files are dropped from the file lists; §2.5's restore follows; step 2 adds `a_restored_player_takes_a_first_step_hit_at_its_saved_health` |
| 4 | [MEDIUM] Stack inspection inside a state's update omits that state | `crates/tungsten/src/state.rs:293-296` (`pop_top`, `update`, `push_top`), `:236-246` | accepted | §2.4: screens store a `ShellFlow` request; `shell_flow` after `state_dispatcher` acts on the whole stack. §2.10's guards drop "on top" (only the top updates). Step 2 adds `quit_to_title_from_a_screen_update_leaves_only_the_title` |
| 5 | [MEDIUM] A difficulty change can disappear on Retry | §2.4–§2.6 as written | accepted | §2.5: difficulty precedence (Retry and the run's own autosave take `RunState`'s; a manual slot and Continue take the slot's); step 4 adds `retry_keeps_a_difficulty_changed_mid_run` |
| 6 | [MEDIUM] Returning `ExitRequested` after a complete frame makes the harness discard that frame | `crates/tungsten/src/testing.rs:155-172` | accepted | §2.1: a crate-private `FrameEnd::Last` the harness records as a completed frame; step 1 adds `exiting_keeps_the_last_frames_draw_audio_and_count` |
| 7 | [MEDIUM] Display rollback can undo mute-when-unfocused | `crates/tungsten/src/settings.rs:46-51` | accepted | §2.6: every shell apply goes through `shell::apply_settings`, which keeps the focus mute; step 4 adds `a_display_revert_keeps_the_focus_mute` |
| 8 | [MEDIUM] The first-key-after-click rule does not apply to these modal screens | `crates/tungsten-core/src/ui/route.rs:1105-1106`, `:1131-1134` | accepted | §2.10: modal screens act on the first press; the rule stays for non-modal UI |

Round 2: the same skill and model at effort xhigh, with `--focus-file` naming round 1's changes (no rejections to carry), §9 and the evidence log cut. 251.7 s; tokens in 752,711 (667,136 cached), out 8,896; 26 commands, 292.5 KB output (largest 28.9 KB); 5-hour window 37% → 56%, week 41% → 44%. Three findings, all accepted, each a gap a round-1 fix left open. There is no third round (the skill's step 7).

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [HIGH] Tracking the new player does not isolate the gems captured by the first autosave | `crates/tungsten/src/state.rs:492-495`; `crates/tungsten/src/app.rs:1199-1201` (the flush after the systems); `crates/tungsten-core/src/ecs/world.rs:215` (`is_alive`) | accepted | §2.3: `SampleRun` holds the gems' handles and `capture` lists the live ones; step 2 adds `retry_autosaves_only_the_new_runs_gems` |
| 2.2 | [MEDIUM] Retry can load another run's autosave after the initial write fails | `crates/tungsten-core/src/save.rs:226-249` (a failed write keeps the old file) | accepted | §2.5: `RunState::retry_from`, the run's last good save; §2.4's game-over row; step 2 adds `retry_starts_fresh_when_the_runs_first_autosave_failed` and renames `retry_loads_the_runs_last_good_save` |
| 2.3 | [MEDIUM] Reapplying zero does not prevent an unmute command from reaching audio | `crates/tungsten-core/src/audio.rs:207-210` (commands append); `crates/tungsten/src/settings.rs:46-51`; no engine caller of `apply_settings` (`rg -n 'apply_settings\(' crates/tungsten/src`) | accepted | §2.6: the wrapper hands the engine a copy of `Settings` muted at 0, so only 0 is queued; step 4's test reads each frame's commands |

Level C API critique (after step 1): the skill's command (`critique --raw --repo --force`, `gpt-6-astra` at xhigh, the `api/` and `templates/` diff), 2026-10-19. 61.3 s; tokens in 198,520 (152,704 cached), out 2,302; 10 commands, 139.1 KB output (largest 31 KB); 5-hour window 56% → 64%, week 44% → 46%. "No concerns": the signatures, the `#[non_exhaustive]` exit types and the unset, uncapped and positive limits checked against the implementation and D-180. No `C.<n>` rows; step 1 stands.

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-19 | done | `-- exiting` 6 passed; `a_pressed_engine_exit_binding_ends_the_frame_after_update` 1 passed; core `-- player_frame_cap` 4 passed, umbrella 3 passed; `cargo test -p tungsten-core` 1,176 passed, 8 ignored; `-p tungsten` 706 passed; clippy exit 0; fmt exit 0; `api/` diff +119 lines (core +42, umbrella +77), `rg -c '^-[^-]'` no match (exit 1), naming `request_exit`, `Exiting`, `ExitReason`, `Harness::request_close`, `FrameRateCap`, `frame_rate_cap`, `set_frame_rate_cap`; D-180 heading 1, index row 1; `just smoke` exit 0 (benchmarks 15/15, frame cap 1/1); API critique no concerns | `api/tungsten.txt`, `api/tungsten-core.txt`; D-180 |
| 2 | 2026-10-19 | done | `states.rs` and `components.rs` gone (exit 0); the `TextSection` and `add_text` grep no match; the old key hints no match; `set_exit_on_escape(false)` 1; template tests 46 passed, 0 failed: `sample::` 17, `shell::` 21, `saves::` 5, `fit::` 3; clippy `--all-targets` exit 0; fmt exit 0; D-181 heading 1; `just repo-check` exit 0 after the `docs/LLM_INDEX.md` template row moved to the new paths (12,275 of 12,288 bytes; step 8's row, pulled forward because step 2's removals broke its paths and `repo-check` is step 2's check); `just ctx` exit 0; `just smoke` exit 0 (template row OK); `just template-check` exit 0; `tokei` code: `src/shell/` 1,217, `src/` 1,554. Deviations: the engine's debug HUD starts hidden (F4 shows it), since it overlapped the centred menus at 731 × 411 units and draws live timings into a capture; the theme names `sans` for its three font tokens (the default `mono` family warned at load) | `templates/basic/src/`, `templates/basic/tests/game/`, D-181 |
| 3 | 2026-10-19 | done | `-- saves::` 15 passed (the ten named plus step 2's five); `-- fit::` 4 passed (the saves screen in both modes at 731 × 411 and 1920 × 1080; the fit walk now measures every shown control, enabled or not); template tests 57 passed, 0 failed; clippy `--all-targets` exit 0 (two tests restore the saves folder's own permissions, since `set_readonly(false)` trips `permissions_set_readonly_false`); fmt exit 0; `just template-check` exit 0; `tokei` code: `src/shell/` 1,489, `src/` 1,826 | `templates/basic/src/shell/saves.rs`, `templates/basic/tests/game/saves.rs` |
| 4 | 2026-10-19 | done | `-- settings::` 18 passed, the named eighteen; `-- fit::` 5 passed (every tab at both sizes); template tests 76 passed, 0 failed; clippy `--all-targets` exit 0; fmt exit 0; `just template-check` exit 0; `tokei` code: `src/shell/` 1,946, `src/` 2,283. Notes: headless, a display request reaches no window and `DisplayState` stays as started, so the Display rows show the set value, else the live one (an F11 press shows while the mode is unset), and the frame-limit test reads the request's effect across a restart (`DisplayState::frame_rate_cap` 60); Retry's first frame begins under game over's held clock, so its first fixed step runs on the next frame (the difficulty test steps once more) | `templates/basic/src/shell/settings.rs`, `templates/basic/tests/game/settings.rs` |
| 5 | 2026-10-19 | done | `-- keys::` 7 passed, the named seven; `-- fit::` 6 passed (the keybindings screen at both sizes, its list a scroll panel whose rows overflow at 411 units; the fit walk checks a scroll panel's own box and lets its rows scroll); template tests 84 passed, 0 failed; `tokei` code: `src/shell/` 1,946 (≤ 2,000, exit 0), `src/` 2,276 (≤ 2,600, exit 0), largest `shell/settings.rs` 437, `shell/mod.rs` 406, `shell/keybindings.rs` 204, `shell/saves.rs` 199; clippy `--all-targets` exit 0; fmt exit 0; `just smoke` exit 0; `just template-check` exit 0. The first draft counted 2,170 shell lines; a compaction inside `templates/basic/src/**` (the plan's files for steps 2–5) brought it under: a shared `Menu` for pause and game over, `idle`, `pop`, `pressed`, `events`, `set_width`, `focus_first` and `fill` helpers, a const row table for settings and shorter widget builders, with no test changed in meaning | `templates/basic/src/shell/keybindings.rs`, `templates/basic/tests/game/keys.rs` |
| 6 | 2026-10-19 | done | `TUNGSTEN_VISUAL_REGRESSION=1 cargo test -p tungsten-template-basic --test game --locked -- visual:: --nocapture` 1 passed (`the_title_matches_its_fixture`, tolerance 0); without the variable it prints "skipped: set TUNGSTEN_VISUAL_REGRESSION=1…" and passes, so it is not run there; `just visual` exit 0 (bench 4, post 5, transition 1, UI lab 6, template 1 passed); the only `*.png` change is the new `templates/basic/tests/fixtures/title.png` (captured once here, frame 20 of 24 at 1280 × 720, OS scale 1.75, smoke mode's "Saving is off" shown); template tests 85 passed | `templates/basic/tests/game/visual.rs`, `templates/basic/tests/fixtures/title.png`, `justfile` |
| 7 | 2026-10-19 | done after one rerun | Base `git archive` of `860f6ed` under `target/m69-base`, candidate the working tree (38 entries, diff `8435fefab5e4`, unchanged through the sitting); binaries differ (`e56670daf6e48883`, `5f5ca81e26e6548a`). Sitting 21:00–21:15 UTC, base A, candidate A, base B, candidate B, `suite --repeat 5`, 32 of 32 row captures valid first time, no encoder. Direct pairs `--fail-on regressed` exit 0, 0 `regressed`, digests 8 of 8 in every compare; `integrated` `total` p50 8.190 → 8.178 and 8.198 → 8.172 ms `unchanged`. The base A/A read `physics` `physics_step` p95 6.368 → 7.134 and `update` p95 `regressed` (base B's runs 2–5 slow on the same binary); its one rerun (21:19–21:20, `--only physics`, base, candidate, base, candidate) read the base A/A 0 `regressed` (`physics_step` p95 6.396 → 6.386), the candidate A/A and both direct pairs 0 `regressed`. Gates after: `just smoke`, `just visual` (4, 5, 1, 6, 1 passed) and `just physics-release` (7 + 1 + 7, both hashes asserted) exit 0. Base tree deleted with `shutil.rmtree` | `perf-runs/20261019-m69-exit/` (README, summary.md, rerun.log) |
| 8 | 2026-10-19 | done | `docs/LLM_INDEX.md` 12,275 bytes (≤ 12,288; its template row moved in step 2); `just ctx` exit 0; `rg -c 'few hundred' docs/plans/1.0/w12-template.md` no match (exit 1); `just repo-check` exit 0. `rg -c 'states\.rs' docs/LLM_INDEX.md templates/basic docs/getting-started.md` prints `docs/LLM_INDEX.md:2`: both lines route to `examples/03_scene_state/src/states.rs`, a live file, and were there at `860f6ed` beside the template's row (3 matches then), so the check as written could not pass; its template form passes (`rg -c 'templates/basic/src/states' docs/LLM_INDEX.md` and `rg -c 'states\.rs' templates/basic docs/getting-started.md`, no match). Docs: the template's `AGENTS.md` (layout, seam, budget, saves and options, input, the kit in its own section) and `README.md`; the guide's start, run, settings and saves, layout, systems, spawns, kit, drawing and time passages; `w12-template.md` (budget, the shell's landing) and `w13-kit.md:281` and its shake notes; eight backlog rows; known issues gain the F9 and F11 row (A10) and lose the template's zero camera shake and M59 Q14's missing exit request; `DESIGN.md`'s frame loop, its template section and an M69 section. The template's camera shake was zero (that known issue), so the Screen shake option scaled nothing: the sample now sets an 18 Hz carrier and a 24 × 16 px peak, the shell reads the shipped offset in `shell_start`, `screen_shake_scales_the_camera` asserts it, and the motion review checks the label's rise once the shake settles (it wobbles with the camera before); template tests 85 passed, clippy and fmt exit 0, the title's pixel test 1 passed, `tokei` `src/shell/` 1,949, `src/` 2,282. The acceptance game's gap log has no rows to mark | `templates/basic/{AGENTS,README}.md`, `docs/getting-started.md`, `DESIGN.md`, `docs/known-issues.md`, `docs/plans/1.0/{w12-template,w13-kit,backlog-1.x}.md` |
| 9 | 2026-10-19 | done | Workflow §5 closed: (1) evidence rows 1–8; (2) gates per step, the release chain after the cut; (3) invariants: the determinism and containment hashes asserted by `just physics-release` (step 7), first-run digests matching on every row of every compare, `just visual` passing (`gpu-visual.png`, the post, transition and UI lab fixtures unchanged), `engine_exit`'s frame test and the smoke exit passing, `api/` removing nothing; the kit hash runs in `just check`; (4) D-180 and D-181 with their index rows, before the docs that cite them; (5) additive API, rustdoc on every new item, snapshots regenerated; (6) the template and the guide; (7) `docs/LLM_INDEX.md`'s template row, `just ctx` exit 0; (8) `DESIGN.md`'s frame loop, template section, M69 section and status line; (9) known issues and the backlog (step 8), the register row cut as 0.89, the follow-ups moved to `w12-template.md` and `roadmap.json`'s two stops repointed there; the acceptance game's gap log has no rows; (10) one `CHANGELOG.md` entry, this status and the archive. The cut and the release checks run after this plan moves to the archive, so their output is in the release report | `CHANGELOG.md`, `DESIGN.md`, `docs/plans/1.0/implementation-plan.md` |

## Follow-ups

1. **Template audit, a stop of its own after this one** (owner, 2026-10-19). Level A, in the QA pass's shape: read every file of the template and the guide's template sections; findings with `file:line` on correctness (save failures, damaged slots, focus loss during a transition, a close during a confirm), the template's obedience to its own `AGENTS.md`, deprecated API, docs that disagree with the code and tests worth copying; then fixes as numbered steps. Its cold-start check: a fresh session in an outside copy deletes `src/sample/` and builds a small game from the template, the guide and rustdoc alone (RC-A9's shape). Stop condition: the audit stops at its findings when the owner's 5-hour window passes 65% (cost note: a scoped audit, not the 2026-10-14 whole-repo review).
2. **W12b adopts the shell.** Examples 01, 03 and 04 move onto the shell and the seam; W12b's plan decides what each keeps. The UI lab keeps its own menus as the UI reference.
3. **Backlog rows** (step 8): the monitor's size and video modes for the window-size list; a theme danger token or game colour tokens; a game summary in slot headers; per-action labels and groups in `input.json`; game-defined `Settings` fields; stick deadzone and sensitivity; startup load warnings a game can read; promoting the shell into the engine once two games keep identical copies.
4. **Known issue** (step 8): F9 and F11 change the display without writing `Settings` (A10).

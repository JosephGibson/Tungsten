# Phase 6 milestone 53: settings and save slots (W11b and W11c) — done

- **status:** done
- **goal:** A shipped game keeps its player's settings and saves in the user folder: one atomic write shared by the action map, settings and saves; typed settings (display mode, resolution, vsync, `ui_scale`, master volume) layered between `tungsten.json` and the environment; bindings persisted in the user folder over the shipped `input.json`; a corrupt user file that falls back to defaults and stays beside them as a backup; versioned save slots over the game's own serde types with the game's own migrations.
- **non-goals:** Volume per bus (W6c adds its buses to the settings, Follow-ups); the settings screen and the capture-next-input mode (W1 M4); gamepad bindings (W5a); serializing the `World` (backlog: saves by reflection); assets resolved relative to the executable (w11 non-goal); hot reload of the user files; persisting the F9/F11 hotkeys' changes (Q4); a frame-rate cap, present mode or frame latency setting (Q1).
- **files to touch:** Step 1: `crates/tungsten-core/src/persist.rs` (new), `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/input/action_map.rs`, `crates/tungsten-core/src/input/key_serde.rs`, `crates/tungsten-core/src/tests/persist.rs` (new), `crates/tungsten-core/src/tests/input/action_map.rs`, `crates/tungsten-core/src/tests/input/key_serde.rs`. Step 2: `crates/tungsten-core/src/settings.rs` (new), `crates/tungsten-core/src/config.rs`, `crates/tungsten-core/src/display.rs`, `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/tests/settings.rs` (new), `crates/tungsten-core/src/tests/config.rs`, `crates/tungsten-core/src/tests/display.rs`, `crates/tungsten/src/user_dir.rs`, `crates/tungsten/src/tests/user_dir.rs`. Step 3: `crates/tungsten-core/src/input/action_map.rs`, `crates/tungsten-core/src/tests/input/action_map.rs`, `crates/tungsten/src/settings.rs` (new), `crates/tungsten/src/tests/settings.rs` (new), `crates/tungsten/src/lib.rs`, `crates/tungsten/src/app.rs`, `crates/tungsten/src/user_dir.rs`, `crates/tungsten/src/tests/app.rs`, `crates/tungsten/src/asset_loader/reload.rs`, `templates/basic/Cargo.toml`, `Cargo.lock`, `templates/basic/input.json`, `templates/basic/src/game.rs`, `templates/basic/tests/game.rs`, `docs/getting-started.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`. Step 4: `crates/tungsten-core/src/save.rs` (new), `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/tests/save.rs` (new), `crates/tungsten/src/app.rs`, `crates/tungsten/src/user_dir.rs`, `crates/tungsten/src/lib.rs`, `crates/tungsten/src/tests/save_slots.rs` (new), `templates/basic/src/states.rs`, `templates/basic/tests/game.rs`, `docs/getting-started.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`. Step 5: `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/known-issues.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/plans/1.0/w11-shipping-basics.md`, `docs/plans/1.0/w06-gameplay-systems.md`, `templates/basic/AGENTS.md`, `api/tungsten-core.txt`, `api/tungsten.txt`. Step 6: as §3 lists.
- **ordered steps:** (1) The shared atomic write, the action map on it, Core F4. (2) Settings, the user-folder override, the config layer, the UI scale. (3) The app reads and writes settings and bindings in the user folder; the template persists a setting; decision 159. (4) Save slots; the template saves a slot; decision 160. (5) Records, API snapshots and smoke with a user file present. (6) Release 0.73.
- **done-when:** every step's evidence row quoted; workflow §5 closed; the release checks, with `just template-check` and `just api-check` among them, pass after the cut; the preflight prints the command block and the post-merge block.

Candidates W11b and W11c ([implementation plan](../../1.0/implementation-plan.md) §4 cards, level B; one candidate since 0.66, not a `D-108` pair), milestone M53, release 0.73. Written 2026-10-14 at `a4d7538`. Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs.

## Context digest

W11a (M35, 0.48, `D-119`) gave each game a user folder (`settings`, `saves`, `logs`) resolved from `game.id`, created only `logs`, and left `tungsten.json` and `input.json` read from the working directory, with `ActionMap::persist_to` writing `input.json` back there through a private `atomic_write`. W11b and W11c fill the other two folders. W1 M4's settings screen writes through W11b, and the acceptance game's settings and unlocks rows (RC-A3, RC-A4, RC-A6) need both.

Owner question 15 takes its stated default (`D-150`): a corrupt user file resets to defaults, and the engine persists display settings, `ui_scale` (`D-127`, `D-144`), master volume and bindings. Q16 made save slots Must (`D-102`). `D-008` and `D-045` keep an invalid shipped `tungsten.json` or `input.json` fatal; a file the game wrote never stops it (w11 scope). `D-022`: runtime conditions return typed errors. Captures stay clean because smoke mode has no user folder (`D-119`, RC-S3).

Two traps found here shape the plan. The template's harness tests call `App::new` on its real `tungsten.json`, so after W11b they would read the developer's own settings (they already write log files to `~/.local/state/tungsten-template-basic/logs`); a code-level folder override pins them (A9, Q6). `docs/LLM_INDEX.md` is 3 bytes under its 12 KiB budget, so step 5's index rows replace words rather than add them (A14).

Decisions: 159 settings, the user layer and the shared write (amends `D-045` and `D-119`, extends `D-008`); 160 save slots (narrows DESIGN's save/load non-commitment). No new dependency: `serde`, `serde_json` and std cover every file.

## 1. Audit

Read at `a4d7538`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | `Config::load` parses the file, validates it, then applies the environment overrides last, inside `load`; a config built in code never gets them. A user layer has to land between the parse and the overrides, and `App::new` receives an already loaded `Config`. | `crates/tungsten-core/src/config.rs:493`, `:559` |
| A2 | `GameConfig` is `#[non_exhaustive]` with public `id` and `version`; `validate` checks the id. A new skipped field is additive. | `crates/tungsten-core/src/config.rs:349`, `:368` |
| A3 | `user_dir::resolve` returns `None` in smoke mode or for an invalid id, then takes `TUNGSTEN_USER_DIR` before the id; `UserDirs.settings` and `.saves` carry `#[allow(dead_code)]` waiting for W11b and W11c. No code-level override exists. | `crates/tungsten/src/user_dir.rs:38`, `:40`, `:49`, `:55`, `:58` |
| A4 | `App::with_plugins` resolves the folder, installs the logger, then resolves the display from the config and loads `input.json` from the working directory; the action map's source path is the shipped file, even when it is missing (`set_source_path`), so `persist` writes the working directory. | `crates/tungsten/src/app.rs:173`, `:202`, `:203`, `:204`, `:1876`, `:1888` |
| A5 | `atomic_write` is private to the action map: create the folder, a `create_new` temp file named with the pid and a nonce, `sync_all`, `rename`, the temp removed on failure. No folder `fsync`, no retry, no injection point, and the error is `ActionMapError::Write`. | `crates/tungsten-core/src/input/action_map.rs:558`, `:578`, `:590` |
| A6 | `persist` writes the remembered source path; `persist_to` remembers the new one; `render_for_save` patches the source text's `actions` object. A serializer failure is reported as `Parse { path: "<persist>" }`. | `crates/tungsten-core/src/input/action_map.rs:300`, `:309`, `:312`, `:382`, `:419`, `:435` |
| A7 | Known issue Core F4, assigned to W11b: a `KeyCode::Other` binding fails `persist` for the whole map after `replace_bindings_and_persist` changed it in memory, and `MouseButton::Other(0)` saves as `"button0"`, which loading rejects (`filter(\|id\| *id > 0)`), fatal at the next start. `ActionMapError` is not `#[non_exhaustive]`, so a new variant is a break. | `docs/known-issues.md:27`; `crates/tungsten-core/src/input/key_serde.rs:118`, `:129` |
| A8 | Hot reload of `input.json` loads the shipped file and merges the defaults; it knows nothing of a user layer. `reload_action_map` is public (`tungsten::asset_loader`), so its signature is frozen surface. | `crates/tungsten/src/asset_loader/reload.rs:308` |
| A9 | The template's tests build `App::new` from `Config::load("tungsten.json")`, whose `game.id` is set, in `harness()` and two more tests; the examples' tests use `Config::default()` (no id, no folder). | `templates/basic/tests/game.rs:20`, `:59`, `:68`; `examples/01_platformer/src/tests/main.rs:96` |
| A10 | `ScaleFactor` holds only the OS scale and `get()` returns it; its rustdoc and `D-144` say `get()` becomes the OS scale times the `ui_scale` W11b adds. Layout and routing read `get()` once a frame. | `crates/tungsten-core/src/display.rs:112`, `:119`; `crates/tungsten/src/app.rs:1179`; `crates/tungsten-core/src/ui/route.rs:199` |
| A11 | Master volume exists only as an `AudioCommand`: the mixer starts at 1.0 and nothing restores a volume at startup. | `crates/tungsten-core/src/audio.rs:74`; `crates/tungsten/src/audio.rs:63`, `:188` |
| A12 | The F9 hotkey clears `present_mode` when it flips vsync, so `auto` follows vsync again; `resolve_present_mode` lets a concrete present mode win over vsync. A persisted vsync needs the same rule. | `crates/tungsten/src/display.rs:85`; `crates/tungsten-render/src/surface.rs:49` |
| A13 | DESIGN lists "Save / load" as a non-commitment and says only `logs` is created so far. | `DESIGN.md:508`, `:359` |
| A14 | `docs/LLM_INDEX.md` is 12,285 bytes of its 12,288 budget (`just ctx`). | `just ctx` output at `a4d7538` |
| A15 | The template's schedule snapshot lists `update: player_movement`; a template system changes that constant. | `templates/basic/tests/game.rs:54` |

## 2. Design

Level B: the w11 scope and Q15 settle the design; this sketch fixes the names the steps use. Everything lives in core except the folder resolution, the app wiring and the runtime apply, which need the umbrella; core gains no `wgpu` or `winit` type, so the seam stands (`D-007`, `D-018`).

### 2.1 Atomic write (`tungsten_core::persist`, step 1)

```rust
/// Writes `contents` to `path` so a reader sees the old file or the new one,
/// never a torn one: a temp file in the same folder, `sync_all`, `rename`,
/// then on Unix an `fsync` of the folder. Creates the folder.
pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), WriteError>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WriteError {
    #[error("failed to write '{}': {source}", path.display())]
    Io { path: PathBuf, source: std::io::Error },
    /// Another process held the file open through every attempt (Windows).
    #[error("'{}' stayed in use after {attempts} attempts: {source}", path.display())]
    Busy { path: PathBuf, attempts: u32, source: std::io::Error },
}
```

- The private seam `write_atomic_with(path, contents, &mut impl Ops)` takes the write, rename and sleep operations and the platform, so Linux tests inject a failure before the rename and Windows sharing errors. `create_temp_file` moves here unchanged.
- Retry: on Windows only, a rename failing with raw OS error 5 (access denied), 32 (sharing violation) or 33 (lock violation) is retried, five attempts with sleeps of 20, 40, 80 and 160 ms, then `Busy`. The classifier is a pure function of the platform and the raw code (Q11).
- The folder `fsync` is best effort: an error opening or syncing the folder is ignored, since the rename has already made the swap (Q15). Power-loss durability on Windows is not claimed (w11).
- Crate-private `quarantine(path) -> io::Result<PathBuf>` renames a bad file to `<name>.bad`, replacing an older one (Q3).

### 2.2 Settings (`tungsten_core::settings`, step 2)

`<settings>/settings.json`, written with only the values the player set:

```json
{
  "display": { "display_mode": "borderless_fullscreen", "resolution": { "width": 1920, "height": 1080 }, "vsync": true },
  "ui_scale": 1.25,
  "master_volume": 0.8
}
```

```rust
/// The settings the engine persists in the user folder: a `World` resource.
/// `None` means the player has not set it, and the shipped default applies.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Settings { /* private: the five values and the file's path */ }

impl Settings {
    pub const FILE: &str = "settings.json";
    pub const UI_SCALE_RANGE: RangeInclusive<f32> = 0.5..=3.0;
    /// Reads `path`. A missing file gives defaults; an unreadable, corrupt or
    /// invalid one gives defaults and is renamed to `settings.json.bad`.
    /// Never fails: warnings wait in `take_load_warnings`, as `Config::load`'s do.
    pub fn load(path: impl Into<PathBuf>) -> Self;
    pub fn take_load_warnings(&mut self) -> Vec<String>;
    pub fn path(&self) -> Option<&Path>;
    pub fn display_mode(&self) -> Option<DisplayMode>;
    pub fn set_display_mode(&mut self, mode: Option<DisplayMode>);
    pub fn resolution(&self) -> Option<Resolution>;
    pub fn set_resolution(&mut self, resolution: Option<Resolution>); // a zero side is ignored
    pub fn vsync(&self) -> Option<bool>;
    pub fn set_vsync(&mut self, vsync: Option<bool>);
    pub fn ui_scale(&self) -> Option<f32>;
    pub fn set_ui_scale(&mut self, scale: Option<f32>);       // clamped; not finite is ignored
    pub fn master_volume(&self) -> Option<f32>;
    pub fn set_master_volume(&mut self, volume: Option<f32>); // clamped to 0..=1; not finite is ignored
    /// Writes the set values through `write_atomic`.
    pub fn save(&self) -> Result<(), SettingsError>;
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SettingsError {
    #[error("the game has no user folder, so settings are not saved")]
    NoFolder,
    #[error(transparent)]
    Write(#[from] WriteError),
}
```

- Invalid means: not UTF-8, not JSON, not an object, a known key of the wrong type, a display mode outside the three names, a zero resolution side, a `ui_scale` outside `UI_SCALE_RANGE` or a `master_volume` outside 0..=1, or either not finite. The whole file falls back (Q3); unknown keys are ignored, so an older build reads a newer file. An unreadable file other than a missing one (permission denied) gives defaults with a warning and is not renamed.
- No version field (Q12): the file grows by keys.
- `Config::apply_settings(&mut self, settings: &Settings) -> Result<(), ConfigError>` writes each set display value into `self.display`; a set vsync also clears `display.present_mode` and `render.present_mode` (A12, Q2); then it applies the environment overrides again, so they stay last, but only to a config `load` built (a private flag `load` sets), so a config built in code stays free of them at startup and at runtime alike, as today (critique 2.2). It errors only as `load` does on a bad override. The overrides read the environment through a private lookup closure (`apply_env_overrides(&mut self, env)`, with `apply_env_overrides_from_env` passing `std::env::var`), so a test injects one without `set_var`, as `user_dir::resolve` does.
- `ScaleFactor` gains a private `ui: f32` (default 1.0), `ui_scale()` and `set_ui_scale(scale)` (not finite or not positive is ignored, as `set_os`); `get()` returns `os * ui` (A10).

### 2.3 The user folder override (step 2)

```rust
/// Where the game's user folder is (`D-119`), set in code; never read from
/// `tungsten.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum UserFolder {
    /// `TUNGSTEN_USER_DIR` when set, else the platform's folders under `game.id`.
    #[default]
    Platform,
    /// `<dir>/settings`, `<dir>/saves` and `<dir>/logs`, as `TUNGSTEN_USER_DIR` gives.
    At(PathBuf),
    /// No folder: no log file, settings or saves.
    Off,
}
```

`GameConfig` gains `#[serde(skip)] pub user_folder: UserFolder`. `resolve` order: smoke mode none; an invalid id none; `Off` none; `At(dir)`; `TUNGSTEN_USER_DIR`; no id none; the platform. Code wins over the environment, so a test that pins a folder stays pinned in a developer's shell (Q6); smoke mode stays first, so captures read nothing whatever the code sets (RC-S3).

### 2.4 Bindings in the user folder (steps 1 and 3)

The map is three layers: the engine defaults, the shipped `input.json` (fatal when invalid, hot-reloaded), then `<settings>/input.json`, per action.

```rust
impl ActionMap {
    /// Lays the user's bindings at `path` over this map, per action, and
    /// makes `path` where `persist` writes, saving only the actions whose
    /// bindings differ from this map. `None`: no user layer, and `persist`
    /// returns `MissingSourcePath`. A corrupt user file is renamed to
    /// `input.json.bad`; warnings wait in `take_load_warnings`.
    pub fn with_user_layer(self, path: Option<&Path>) -> Self;
    /// The user file `persist` writes, when a user layer is set.
    pub fn user_layer_path(&self) -> Option<&Path>;
    pub fn take_load_warnings(&mut self) -> Vec<String>;
}
```

- Private fields: `base` (the actions before the user layer) and the warnings. With a base, `render_for_save` writes only the actions whose bindings differ from it (Q5); without one it writes the whole map, as today, so `persist_to` on a shipped map is unchanged.
- Core F4 (step 1): a binding with no canonical name (`KeyCode::Other`) is skipped on save with a warning; an action left with no saveable binding while it had some is left out of the file, so a restart falls back to the layer below rather than disabling it (Q7). `"button0"` loads as `MouseButton::Other(0)`, so every saved button loads. `ActionMapError` gains no variant (A7); the `Parse { path: "<persist>" }` mapping stays for a serializer failure no binding can now cause.
- `ActionMapError::Write` keeps its `std::io::Error` source: a `WriteError` maps to its inner error, `Busy` included, so nothing breaks.
- Hot reload (A8): `reload_action_map` keeps its signature; it reads `user_layer_path()` from the current map and lays the user file over the reloaded one.

### 2.5 The app (step 3)

`App::with_plugins`, after the logger and the load warnings:

1. `Settings::load(<settings>/settings.json)` when a folder exists, else `Settings::default()`; its warnings logged.
2. A clone of the config as it arrived (the shipped file with its environment overrides) goes into a crate-private resource, `ShippedConfig`, the baseline every later apply starts from. Then `config.apply_settings(&settings)` runs with or without a folder (without one the settings are empty), so startup and the runtime apply take the same path, and the display resolves as today.
3. The action map: shipped and defaults as today (an invalid shipped file stays fatal), then `with_user_layer(folder.map(|d| d.settings.join("input.json")))`; its warnings logged. No folder: nothing persists, and nothing is written to the working directory.
4. `Settings` inserted; `ScaleFactor` inserted with the settings' UI scale; a set master volume pushed through `AudioCommands::set_master_volume`.
5. `UserDirs`' `#[allow(dead_code)]` lines go.

`tungsten::apply_settings(world: &mut World)` is the runtime path for W1 M4's screen, and it computes its target the way startup does, so a runtime apply and a restart agree (critique 2, 3): clone `ShippedConfig`, `apply_settings(&settings)` on the clone (the environment last again), resolve it with startup's display resolver, then copy the persisted fields (display mode, resolution, vsync and the present mode, which a set vsync clears) into the pending display request if one is queued, else into the current `DisplayState`. The result replaces the pending request when it differs from the current state and clears it when it matches, so the last apply before a frame wins over an earlier apply and over a hotkey's request queued in `PreUpdate` (critique 2.1); the frame cap, frame latency and scale mode keep the request's or the current values. A value the player unset returns to the shipped one, the UI scale to 1.0 and the master volume to 1.0, so `apply_settings` always sets the UI scale and pushes the master volume. F9 and F11 changes to the persisted fields since startup are replaced (Q4). With no `ShippedConfig` (a `World` built without `App`), unset display values keep the current state. It is a function, not a system, so the schedule and every snapshot of it stay as they are (Q14). A game changes `Settings`, calls `apply_settings`, then `save`.

### 2.6 Save slots (`tungsten_core::save`, step 4)

`<saves>/<slot>.json`:

```json
{ "envelope": 1, "slot": "autosave", "schema": 2, "engine_version": "0.73.0", "saved_at_ms": 1760400000000, "data": { "…": "the game's type" } }
```

```rust
/// A game's saved data: its serde type, its schema number and its migrations.
pub trait SaveData: Serialize + DeserializeOwned {
    /// This type's schema; bump it when the type changes shape.
    const SCHEMA: u32;
    /// Rewrites `data`, saved under the older schema `from`, into this
    /// schema's shape. The default refuses every older schema.
    fn migrate(from: u32, data: serde_json::Value)
        -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>>;
}

/// The game's save slots in its user folder: a `World` resource.
#[derive(Debug, Clone, Default)]
pub struct SaveSlots { /* private: the saves folder, if any */ }

impl SaveSlots {
    pub fn new(dir: Option<PathBuf>) -> Self;
    pub fn dir(&self) -> Option<&Path>;
    pub fn save<T: SaveData>(&self, slot: &str, data: &T) -> Result<(), SaveError>;
    pub fn load<T: SaveData>(&self, slot: &str) -> Result<T, SaveError>;
    /// Every readable slot, by name; an unreadable file is skipped with a
    /// warning (in the result's `warnings`), and `load` reports it.
    pub fn list(&self) -> Result<SlotList, SaveError>;
    pub fn delete(&self, slot: &str) -> Result<(), SaveError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SlotInfo { pub name: String, pub schema: u32, pub engine_version: String, pub saved_at_ms: u64 }

#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct SlotList { pub slots: Vec<SlotInfo>, pub warnings: Vec<String> }

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SaveError {
    NoFolder,
    InvalidName { name: String },
    NotFound { slot: String },
    Corrupt { slot: String, source: serde_json::Error },
    UnknownEnvelope { slot: String, envelope: u32 },
    NewerSchema { slot: String, found: u32, supported: u32 },
    Migration { slot: String, from: u32, source: Box<dyn std::error::Error + Send + Sync> },
    Io { path: PathBuf, source: std::io::Error },
    Write(#[from] WriteError),
}
```

- Slot names: 1–64 of `a`–`z`, `0`–`9`, `.`, `_`, `-`, starting with a letter or digit, not a Windows device name; lowercase only, so no two names collide on a case-insensitive file system (Q8). Anything else is `InvalidName`, a runtime error, since names may come from a player (`D-022`).
- `load`: equal schema deserializes `data`; older calls `T::migrate(found, data)` once, then deserializes (`Migration` on either failure); newer is `NewerSchema`; an `envelope` other than 1 is `UnknownEnvelope`. A failed load never panics and never changes the file.
- `list` reads `*.json` files that are not dotfiles (the write's temp files are), sorted by name, the envelope's header only.
- `saved_at_ms` is Unix milliseconds from `SystemTime::now()`, 0 for a clock before 1970 (Q16); `engine_version` is core's `CARGO_PKG_VERSION`. The folder is created on the first save.
- `App::with_plugins` inserts `SaveSlots::new(folder.map(|d| d.saves))`; smoke mode and `UserFolder::Off` give `NoFolder` on every call.

### 2.7 Template (steps 3 and 4)

- `Cargo.toml` gains `log = "0.4"`, a plain version like its other dependencies so the outside copy resolves it (the umbrella re-exports no `log`; `log` is already in `Cargo.lock`, so the lock gains only the template's dependency entry; critique 4).
- `input.json` gains `toggle_mute` on `KeyM`; `game.rs` gains a `toggle_mute` system in `Update` that flips `Settings`' master volume between 0.0 and unset, calls `apply_settings` (unset pushes 1.0) and `save`, and logs a save error; the gameplay text says "M mutes". The schedule snapshot's `update` line gains it (A15).
- `states.rs`: gameplay's `on_enter` loads slot `autosave` (`SaveGame { player: [f32; 2] }`, schema 1): loaded, the player spawns at the saved position; `NotFound`, at `PLAYER_START`; any other error is logged, the player spawns at `PLAYER_START`, and a game resource marks the slot read-only for the run, so entering pause, which otherwise saves the player's position there, never replaces a slot it could not read, such as one from a newer build (critique 1).
- `tests/game.rs`: `harness()` and every test that builds an `App` set `config.game.user_folder = UserFolder::Off`, except the two persistence tests, which use `UserFolder::At` a fresh temp folder and build a second `App` on it.

## 3. Steps

### Step 1: The shared atomic write, the action map on it, Core F4

- **Files:** `crates/tungsten-core/src/persist.rs` (new), `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/input/action_map.rs`, `crates/tungsten-core/src/input/key_serde.rs`, `crates/tungsten-core/src/tests/persist.rs` (new), `crates/tungsten-core/src/tests/input/action_map.rs`, `crates/tungsten-core/src/tests/input/key_serde.rs`
- **Change:** §2.1: add `persist` (public `write_atomic` and `WriteError`, crate-private seam, classifier and `quarantine`), moving `create_temp_file` from the action map; `pub mod persist` in `lib.rs`. Point `ActionMap::persist_to` at `write_atomic`, mapping `WriteError` into `ActionMapError::Write`'s `io::Error`, and delete the private `atomic_write`. §2.4's Core F4 half: skip unsaveable bindings on save with a warning (kept in the warnings field step 3 makes public; until then `log::warn!`), leave an emptied action out, and accept `"button0"`. Tests in `tests/persist.rs`: `a_failure_before_the_rename_leaves_the_old_file`, `a_failed_rename_leaves_the_old_file_and_no_temp`, `a_windows_sharing_failure_retries_then_succeeds`, `a_windows_sharing_failure_that_persists_is_busy` (five attempts, the four sleeps injected, none real), `other_errors_and_unix_codes_are_not_retried`, `the_folder_is_created`, `quarantine_replaces_an_older_backup`. In the action-map tests: `a_key_with_no_name_is_skipped_and_the_rest_saved`, `an_action_left_with_no_saveable_binding_is_left_out`. In `key_serde`: `button0_round_trips`.
- **Done-when:** `cargo test -p tungsten-core --locked persist::tests` → `test result: ok`, the seven tests above listed; `cargo test -p tungsten-core --locked action_map` → `test result: ok` with the existing persist tests and the two new ones; `cargo test -p tungsten-core --locked key_serde` → ok with `button0_round_trips`; `rg -n 'fn atomic_write|fn create_temp_file' crates/` → one line, `crates/tungsten-core/src/persist.rs`; `rg -c 'write_atomic' crates/tungsten-core/src/input/action_map.rs` → at least 1; `cargo clippy -p tungsten-core --all-targets --locked -- -D warnings` → no warnings.
- **Moves:** none.

### Step 2: Settings, the user-folder override, the config layer, the UI scale

- **Files:** `crates/tungsten-core/src/settings.rs` (new), `crates/tungsten-core/src/config.rs`, `crates/tungsten-core/src/display.rs`, `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/tests/settings.rs` (new), `crates/tungsten-core/src/tests/config.rs`, `crates/tungsten-core/src/tests/display.rs`, `crates/tungsten/src/user_dir.rs`, `crates/tungsten/src/tests/user_dir.rs`
- **Change:** §2.2: `Settings` and `SettingsError` with load, validation, fallback through `quarantine`, warnings and `save` through `write_atomic`; `pub mod settings` and root re-exports of `Settings`, `UserFolder`. `Config::apply_settings`. `ScaleFactor`'s UI scale. §2.3: `UserFolder` and `GameConfig::user_folder`; `resolve` takes it in the stated order. Tests in `tests/settings.rs`: `a_missing_file_gives_defaults_without_a_warning`, `a_corrupt_file_falls_back_and_stays_beside_as_a_backup` (bytes of `settings.json.bad` equal the corrupt input; one warning naming both paths), `an_out_of_range_value_invalidates_the_file`, `unknown_keys_are_ignored`, `save_writes_only_set_values_and_load_reads_them_back`, `save_without_a_folder_is_no_folder`, `setters_clamp_and_ignore_non_finite`. In `tests/config.rs`: `user_settings_sit_between_the_file_and_the_environment` (the file says windowed 1280×720, the settings borderless 1920×1080, an injected override says windowed: the result is windowed 1920×1080) and `a_user_vsync_clears_the_present_mode` and `apply_settings_reapplies_overrides_only_to_a_loaded_config`; the override is injected through the private lookup closure (§2.2), not `set_var`. In `tests/display.rs`: `get_is_the_os_scale_times_the_ui_scale`. In `tests/user_dir.rs`: `off_gives_no_folder`, `at_wins_over_the_environment`, `smoke_mode_wins_over_at`.
- **Done-when:** `cargo test -p tungsten-core --locked settings::tests` → ok, the seven tests listed; `cargo test -p tungsten-core --locked config::tests` → ok with the three new tests; `cargo test -p tungsten-core --locked display::tests` → ok; `cargo test -p tungsten --locked user_dir` → ok with the three new tests; `cargo clippy --workspace --all-targets --locked -- -D warnings` → no warnings.
- **Moves:** none: every new value defaults to today's behaviour (`ui` 1.0, `UserFolder::Platform`).

### Step 3: Settings and bindings in the user folder; the template persists a setting; decision 159

- **Files:** `crates/tungsten-core/src/input/action_map.rs`, `crates/tungsten-core/src/tests/input/action_map.rs`, `crates/tungsten/src/settings.rs` (new), `crates/tungsten/src/tests/settings.rs` (new), `crates/tungsten/src/lib.rs`, `crates/tungsten/src/app.rs`, `crates/tungsten/src/user_dir.rs`, `crates/tungsten/src/tests/app.rs`, `crates/tungsten/src/asset_loader/reload.rs`, `templates/basic/Cargo.toml`, `Cargo.lock`, `templates/basic/input.json`, `templates/basic/src/game.rs`, `templates/basic/tests/game.rs`, `docs/getting-started.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`
- **Change:** §2.4: `with_user_layer`, `user_layer_path`, `take_load_warnings`, the base diff on save, the step 1 warnings moved into the field. §2.5: the app wiring, `tungsten::apply_settings` in a private `settings` module re-exported at the root, the reload laying the user file over the shipped one. §2.7's first, second and fourth bullets. Then decision 159 with tungsten-decision: settings and the user layer, the shared write, bindings in the user folder, the corrupt-file fallback, `UserFolder`; amends `D-045` (choices 4 and 8: user bindings over the shipped file, persistence to the user folder) and `D-119` (the folder's resolution order), extends `D-008` (a user layer before the overrides); its index rows and the two marker lines. Then a "Settings and saves" section in the getting-started guide after "Change it while it runs": where the files live, `Settings`, `apply_settings` and `save`, the mute key, `UserFolder` in tests. Tests: core `a_user_layer_overlays_per_action_and_saves_only_the_difference`, `persist_with_a_user_layer_leaves_the_shipped_file_alone` (the shipped file's bytes unchanged), `a_corrupt_user_layer_falls_back_with_a_backup`, `no_user_layer_persist_is_missing_source_path`; umbrella `tests/app.rs`: `settings_and_bindings_load_from_the_user_folder` (an `At` folder holding both files: `DisplayState`, `ScaleFactor::get`, the first step's `SetMasterVolume` and a user binding), `a_corrupt_user_file_never_stops_the_app` (both files corrupt: `App::new` succeeds with defaults and both `.bad` files exist), `an_invalid_shipped_input_json_still_fails` (new: `load_action_map_at_startup` on a temp invalid shipped file with a valid user layer returns the parse error, since `App::new` reads `input.json` from the working directory and a test cannot move it), `without_a_folder_nothing_persists`; `tests/settings.rs`: `apply_settings_requests_the_display_and_sets_scale_and_volume`, `apply_settings_requests_nothing_when_the_display_matches`, `unset_values_return_to_the_shipped_ones` (resolution 1920×1080 and volume 0.0 set and applied, then unset and applied: the shipped 1280×720 is requested and `SetMasterVolume(1.0)` pushed), `a_runtime_apply_matches_a_restart` (shipped `present_mode` `fifo` with vsync false, the setting `vsync: false`: the runtime apply clears the present mode, and a fresh `App` on the same folder starts with the same `DisplayState`), `the_last_apply_before_a_frame_wins` (borderless set and applied, then unset and applied before a step: no request is pending; an F11 request queued first is replaced the same way); the template's `mute_survives_a_restart`.
- **Done-when:** `cargo test -p tungsten-core --locked action_map` → ok with the four new tests; `cargo test -p tungsten --locked settings::tests` → ok with the five tests; `cargo test -p tungsten --locked app::tests` → ok with the four new tests; `cargo test -p tungsten-template-basic --locked` → ok, `mute_survives_a_restart` among them; `cargo tree -p tungsten-template-basic -e normal --depth 1 --locked` → lists `log v0.4`; `just deps` → passes; `rg -n 'allow\(dead_code' crates/tungsten/src/user_dir.rs` → one line, the saves field's (step 4 removes it); `rg -n '^## D-15[9]' DECISIONS.md` → one heading; `rg -n 'D-15[9]' docs/DECISION_INDEX.md` → its row plus the `D-008`, `D-045` and `D-119` rows' amendment notes; `just repo-check` → passes; `cargo clippy --workspace --all-targets --locked -- -D warnings` → no warnings.
- **Moves:** the template's schedule snapshot constant (`update: player_movement, toggle_mute`); no engine schedule.

### Step 4: Save slots; the template saves a slot; decision 160

- **Files:** `crates/tungsten-core/src/save.rs` (new), `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/tests/save.rs` (new), `crates/tungsten/src/app.rs`, `crates/tungsten/src/user_dir.rs`, `crates/tungsten/src/lib.rs`, `crates/tungsten/src/tests/save_slots.rs` (new), `templates/basic/src/states.rs`, `templates/basic/tests/game.rs`, `docs/getting-started.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`
- **Change:** §2.6: `save` with `SaveData`, `SaveSlots`, `SlotInfo`, `SlotList` and `SaveError`; `pub mod save` and root re-exports of `SaveData` and `SaveSlots`; `App::with_plugins` inserts the resource and the saves field's `#[allow(dead_code)]` goes; `lib.rs` attaches `tests/save_slots.rs` under `#[cfg(test)]`. §2.7's third bullet and two template tests, `continue_from_the_autosave` and `a_slot_that_fails_to_load_is_never_overwritten` (a newer-schema envelope written into the temp folder's saves; gameplay, then pause; the file's bytes unchanged). Decision 160 with tungsten-decision: save slots in a versioned envelope, game-owned types and migrations, slot names, listing, typed errors; it narrows DESIGN's save/load non-commitment to World snapshots, with its index row. The guide's "Settings and saves" section gains the slot. Core tests: `save_then_load_round_trips`, `list_reads_headers_sorted_and_skips_temp_files`, `a_missing_slot_is_not_found`, `a_corrupt_slot_is_corrupt_and_left_unchanged`, `a_newer_schema_is_newer_schema`, `an_unknown_envelope_is_unknown_envelope`, `invalid_names_are_refused` (upper case, a separator, `..`, `con`, 65 characters, empty), `without_a_folder_every_call_is_no_folder`, `delete_removes_the_slot`. Harness tests in `tests/save_slots.rs`: `a_schema_bump_loads_an_old_slot_through_the_games_migration` (a `ProgressV1` saved under schema 1 through the `World`'s `SaveSlots`, then a fresh `App` on the same folder loads `ProgressV2`, schema 2, through its `migrate`), `a_failed_migration_is_a_typed_error`.
- **Done-when:** `cargo test -p tungsten-core --locked save::tests` → ok, the nine tests listed; `cargo test -p tungsten --locked save_slot` → ok, both tests; `cargo test -p tungsten-template-basic --locked` → ok, `continue_from_the_autosave`, `a_slot_that_fails_to_load_is_never_overwritten` and `mute_survives_a_restart` among them; `rg -n 'allow\(dead_code' crates/tungsten/src/user_dir.rs` → no match; `rg -n '^## D-1(59|60)' DECISIONS.md` → two headings; `just repo-check` → passes; `cargo clippy --workspace --all-targets --locked -- -D warnings` → no warnings.
- **Moves:** none.

### Step 5: Records, API snapshots and smoke with a user file present

- **Files:** `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/known-issues.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/plans/1.0/w11-shipping-basics.md`, `docs/plans/1.0/w06-gameplay-systems.md`, `templates/basic/AGENTS.md`, `api/tungsten-core.txt`, `api/tungsten.txt`
- **Change:** DESIGN: Data-Driven Config's loading order (file, user settings, environment), Input Mapping's persisted path, the M35 section's "only `logs`" sentence, a "Settings and Save Slots — M53" section, and the non-commitment narrowed to World snapshots. `docs/LLM_INDEX.md`: the config, user-folder and input rows name `core/persist.rs`, `core/settings.rs`, `core/save.rs` and `tungsten/settings.rs` with the two decisions, by rewording rows rather than adding any, the file staying within budget (A14). Known issues: Core F4's row goes. W4 break ledger: a row for the action map's persisted path (`persist` writes the user folder's `input.json`, or returns `MissingSourcePath` without a folder) and the loading order (a user layer before the environment), and a new-surface row listing §2's public items at their paths. w11: steps 2 and 3 marked landed in 0.73 with the decisions; the per-bus volume follow-up. w06: a W6c follow-up line, bus volumes join `Settings`. Template `AGENTS.md`: one line on `UserFolder::Off` in tests. `just api`, then read the snapshot diff. Then the smoke run with a user folder whose settings would change every capture.
- **Done-when:** `just api` → writes the two snapshots, and `git diff --stat -- api/` shows only `api/tungsten-core.txt` and `api/tungsten.txt`, each diff additions except the `ScaleFactor` and `GameConfig` lines it rewrites; `just api-check` → passes; `just ctx` → `Agent context checks: OK` with `LLM_INDEX.md` ≤ 12288 B; `just repo-check` → passes; `rg -n 'Core F4' docs/known-issues.md` → no match; `rg -n '^- Save / load$' DESIGN.md` → no match; smoke with a user file present: `u=<scratchpad>/m53-user && mkdir -p "$u/settings" && printf '%s' '{"display":{"display_mode":"exclusive_fullscreen","resolution":{"width":640,"height":360},"vsync":true},"ui_scale":2.0,"master_volume":0.0}' > "$u/settings/settings.json" && cp "$u/settings/settings.json" "$u/before.json" && TUNGSTEN_USER_DIR="$u" just smoke && cmp "$u/before.json" "$u/settings/settings.json" && fd . "$u"` → every example passes, `cmp` is silent, and `fd` lists only `before.json`, `settings/` and `settings/settings.json` (no `logs/`, no `.bad`, no `saves/`); `just checks` → its recommendations, each run or named for the release step.
- **Moves:** the `api/` snapshots, on purpose.

### Step 6: Release 0.73

- **Files:** `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row, the README's four owned Now fields (`roadmap.py summary`), this plan (moved to `docs/plans/archive/`)
- **Change:** close workflow §5 and this step's evidence row with `status: done`; then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.73.0` once, every release check once after it (`just template-check` covers the template's outside copy; no gate by change is left for this step), then the preflight with `--message`
- **Done-when:** [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut); `just release-preflight 0.73.0 --repo JosephGibson/Tungsten --message 'Update 0.73: M53 settings and save slots'` → passes and prints the command block and the post-merge block
- **Ends with:** the changed-file list, the command block, the post-merge block, then the next prompt: the `plan` prompt for the register's next row, "W1 WL with W5a" (phase 6, milestone 54), naming both candidates, with `docs/plans/1.0/w01-ui-text-suite.md` §11 and §14 and `docs/plans/1.0/w05-input.md` as its sources.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Which display settings persist? | Display mode, resolution and vsync. Present mode, frame latency and the frame-rate cap stay developer settings in `tungsten.json`; `scale_mode` is leaving (`D-127`). |
| Q2 | Does a persisted vsync override a shipped concrete present mode? | Yes, whenever it is set, at startup and at runtime alike: it clears both present-mode fields (A12); a `TUNGSTEN_*_PRESENT_MODE` override still applies after it. |
| Q3 | A corrupt settings file: per field or whole file, and where does it go? | The whole file falls back; it is renamed to `settings.json.bad`, replacing an older backup; unknown keys are ignored, not corrupt. The same for the user `input.json`. |
| Q4 | Do the F9 and F11 hotkeys persist what they change? | No: they are engine toggles; W1 M4's screen persists through `Settings`, and `apply_settings` replaces a hotkey's change to a persisted field. |
| Q5 | The user `input.json`: the whole map or the difference? | Only actions whose bindings differ from the shipped layer, so a game update's new defaults reach every action the player left alone. |
| Q6 | Does a code-set `UserFolder` win over `TUNGSTEN_USER_DIR`? | Yes, and smoke mode wins over both. |
| Q7 | Core F4: refuse or skip a binding that cannot be saved? | Skip it with a warning, leave an emptied action out of the file, and accept `"button0"`; no new `ActionMapError` variant (a break W4b's `#[non_exhaustive]` sweep can take). |
| Q8 | Which slot names are valid? | `game.id`'s rule, lowercase only. |
| Q9 | How does `list` treat an unreadable slot file? | Skips it with a warning in `SlotList::warnings`; `load` of it returns `Corrupt`. |
| Q10 | Is `write_atomic` public? | Yes, `tungsten_core::persist::write_atomic` with `WriteError`, for a game's own files. |
| Q11 | The Windows retry policy? | Five attempts on OS errors 5, 32 and 33, sleeping 20, 40, 80 and 160 ms between them. |
| Q12 | Does `settings.json` carry a version? | No; it grows by keys, and a break would add one. |
| Q13 | Volume per bus? | Master volume only; W6c adds its buses to `Settings` (Follow-ups). |
| Q14 | Is the runtime apply a system or a function? | A function, `tungsten::apply_settings`; no engine schedule change. |
| Q15 | A failed folder `fsync` after the rename? | Ignored: the swap has happened, and some file systems refuse a folder sync. |
| Q16 | The envelope's timestamp? | `saved_at_ms`, Unix milliseconds, 0 before 1970. |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`).

## 5. Decisions expected

- 159: Settings and the user layer: the shared atomic write, typed settings between `tungsten.json` and the environment, bindings in the user folder over the shipped file, the corrupt-file fallback with a backup, `UserFolder` set in code; amends `D-045` and `D-119`, extends `D-008`. Written in step 3.
- 160: Save slots in a versioned envelope over game-owned serde types and migrations, with slot names, listing and typed errors; narrows DESIGN's save/load non-commitment. Written in step 4.

Next free IDs at `a4d7538`; checked again when each step runs.

## 6. Invariants

- **May move:** the `api/` snapshots (step 5); the template's schedule snapshot constant (step 3); `ScaleFactor::get`, only when a `ui_scale` is set.
- **Must not move:** the determinism hash, the pinned containment hash, the row digests, `gpu-visual.png`, the post and transition regressions, the engine schedule (every `resolved_text` snapshot outside the template), and the examples' and benchmarks' behaviour in smoke mode, which reads no user file.

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with steps that don't depend on it. Steps 3 and 4 depend on 1 and 2; step 5 depends on what landed.
- A step needs a per-frame engine path changed beyond `ScaleFactor::get`, or a new engine system: stop the step, since §8 runs no capture; record it skipped with the reason.
- A step needs a public signature changed or a variant added to an exhaustive public enum: stop the step and record it (§2 adds only).
- A file outside §files to touch needs a change.
- `docs/LLM_INDEX.md` cannot take the step 5 rows within budget without dropping a row: record the rows as a follow-up rather than drop one.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1–5, release | Crate and template tests in each step; smoke with a user file present in step 5. No perf row: no frame-loop or ECS code changes, the settings load once at startup, and `ScaleFactor::get` gains one multiply that layout and routing read once a frame (stop condition two guards it). No `just visual`: no renderer path or pixel changes. The release runs `just check`, `just smoke`, `just script-test`, `just template-check` and the rest once, after the cut |
| 3 Invariants | 5, release | Smoke mode reads no user file (step 2's tests, step 5's run); nothing captured moves |
| 4 Decisions | 3, 4 | 159 and 160, each before the guide text that cites it |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1–4, 5 | Rustdoc on every new public item as it lands; the ledger rows, the new-surface row and `just api` in step 5 |
| 6 Template and guide | 3, 4 | The mute key and the autosave, their tests, `log` in the template manifest; the guide's "Settings and saves" section |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 5 | Rows reworded within budget (A14) |
| 8 Design | 5 | The config order, input mapping, the M35 sentence, the M53 section, the non-commitment |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 5, release | Core F4 leaves known issues; no backlog row (saves by reflection is already one); no gap-log row exists; w11 and w06 updated; `python3 -B scripts/roadmap.py summary` supplies the owned fields |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

Round 1: the `critique` skill with `--repo`, `gpt-6-astra`, 2026-10-14, at `a4d7538` with this plan uncommitted (§9 and the evidence log cut with the skill's `sed`); effort xhigh, context 872,000, output cap 4,000, 303.4 s, tokens in 816,200 (730,880 cached), out 8,592 (reasoning 5,562), 17 commands, 300.7 KB output (largest 51.5 KB), 5h window 15% → 35%, week 2% → 5%. Four findings, all accepted; they changed step 3's and step 4's done-when checks, so round 2 runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] A failed autosave load leads to overwriting the save it could not read | §2.7 as written: any load error spawned at `PLAYER_START`, and pause saved unconditionally | accepted | §2.7: a load error other than `NotFound` makes the slot read-only for the run; step 4 adds `a_slot_that_fails_to_load_is_never_overwritten` |
| 2 | [MEDIUM] Clearing a setting cannot restore its shipped value with the proposed runtime state | `crates/tungsten/src/app.rs:89` (`App` keeps `config` private); `crates/tungsten-core/src/display.rs:144` (`DisplayState` holds current values only); the mute toggle's unset path pushed nothing | accepted | §2.5: a crate-private `ShippedConfig` baseline, unset values return to the shipped display and to 1.0; step 3 adds `unset_values_return_to_the_shipped_ones` |
| 3 | [MEDIUM] Matching vsync booleans can conceal a required present-mode change | `crates/tungsten-core/src/display.rs:233` (`present_mode` from `render`, apart from vsync); `crates/tungsten-render/src/surface.rs:85` (a concrete `Fifo` wins); §2.5 cleared the present mode only on a vsync change, startup whenever vsync was set | accepted | §2.5: the runtime target is computed by startup's own layering and resolver; Q2 reworded; step 3 adds `a_runtime_apply_matches_a_restart` |
| 4 | [MEDIUM] The template's required logging needs a dependency outside the allowed file set | `templates/basic/Cargo.toml:8` (`anyhow`, `serde`, `tungsten`); `crates/tungsten/src/lib.rs:18` (`mod logging` private, no `log` re-export) | accepted | §2.7 and step 3: `log = "0.4"` in the template manifest and its `Cargo.lock` entry, with `cargo tree` and `just deps` checks |

Round 2: the same skill and critic on the revised plan, 2026-10-14, at `a4d7538`, with §9 and the evidence log cut and a `--focus-file` naming round 1's changes; effort xhigh, context 872,000, output cap 4,000, 171.6 s, tokens in 544,050 (478,336 cached), out 5,315 (reasoning 2,824), 20 commands, 190.2 KB output (largest 21.6 KB), 5h window 35% → 49%, week 5% → 8%. Two findings, both accepted and applied with the smallest change that closes each; there is no third round.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] A no-op apply can leave an obsolete display request queued | `crates/tungsten/src/app.rs:1518` (the pending request applies at the next frame's start); `crates/tungsten/src/plugins.rs:86` (F11 queues in `PreUpdate`, before `Update`) | accepted | §2.5: the apply starts from the pending request when one is queued and replaces or clears it; step 3 adds `the_last_apply_before_a_frame_wins` |
| 2.2 | [MEDIUM] Startup and runtime apply use different environment rules without a user folder | `crates/tungsten-core/src/config.rs:559` (overrides only inside `load`); `crates/tungsten/src/app.rs:202` (a code-built config resolved as given); §2.5 called `apply_settings` at startup only with a folder | accepted | §2.2: `apply_settings` re-applies overrides only to a config `load` built; §2.5: startup calls it with or without a folder; step 2 adds `apply_settings_reapplies_overrides_only_to_a_loaded_config` |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-14 | pass | `persist::tests` → `test result: ok. 7 passed`, the seven named; `action_map` → `ok. 21 passed` (the three persist tests and the two new ones among them); `key_serde` → `ok. 9 passed`, `button0_round_trips` among them (it replaces `mouse_button_zero_is_rejected`); `rg 'fn atomic_write\|fn create_temp_file' crates/` → one line, `persist.rs:159`; `rg -c write_atomic action_map.rs` → 2; `cargo clippy -p tungsten-core --all-targets --locked -- -D warnings` → exit 0 | `crates/tungsten-core/src/persist.rs`, `tests/persist.rs` |
| 2 | 2026-10-14 | pass | `settings::tests` → `ok. 7 passed`, the seven named; `config::tests` → `ok. 56 passed`, the three new among them, the override injected through `apply_settings_with`'s lookup; `display::tests` → `ok. 14 passed` with `get_is_the_os_scale_times_the_ui_scale`; `cargo test -p tungsten --locked user_dir` → `ok. 15 passed` with `off_gives_no_folder`, `at_wins_over_the_environment`, `smoke_mode_wins_over_at`; workspace clippy → exit 0 after `UserDirs::under` took `&Path` (`needless_pass_by_value`). `UserFolder` lives in `tungsten_core::settings`, re-exported at the root | `crates/tungsten-core/src/settings.rs`, `config.rs`, `display.rs`; `crates/tungsten/src/user_dir.rs` |
| 3 | 2026-10-14 | pass | core `action_map` → `ok. 25 passed`, the four new among them; `cargo test -p tungsten --locked settings::tests` → `ok. 5 passed`, the five named; `app::tests` → `ok. 40 passed`, the four new among them; `cargo test -p tungsten-template-basic --locked` → `ok. 8 passed` with `mute_survives_a_restart`; `cargo tree … --depth 1` → `log v0.4.34`; `just deps` → exit 0; `rg 'allow\(dead_code' user_dir.rs` → one line, `:40` (saves); `^## D-159` → one heading at `DECISIONS.md:2031`; the index → the `D-159` row and the `D-008`, `D-045`, `D-119` notes; `just repo-check` → exit 0; workspace clippy → exit 0 after one `single_match_else` fix. Two choices the plan left open: a save's skipped bindings stay `log::warn!` lines, since a logger exists by then and nothing drains the field at runtime, so `take_load_warnings` holds the user layer's load warnings (recorded in `D-159`); `load_action_map_at_startup` takes the user layer, so `an_invalid_shipped_input_json_still_fails` drives the layering. The guide's `Cargo.toml` block gained `log` and its controls line "M mutes" | `crates/tungsten-core/src/input/action_map.rs`; `crates/tungsten/src/settings.rs`, `app.rs`, `asset_loader/reload.rs`; `templates/basic/`; `DECISIONS.md` `D-159` |
| 4 | 2026-10-14 | pass | `save::tests` → `ok. 9 passed`, the nine named, plus the `SaveData` doctest; `cargo test -p tungsten --locked save_slot` → `ok. 2 passed`; the template → `ok. 10 passed` with `continue_from_the_autosave`, `a_slot_that_fails_to_load_is_never_overwritten` and `mute_survives_a_restart`; `rg 'allow\(dead_code' user_dir.rs` → no match (exit 1); `^## D-1(59\|60)` → two headings, `:2031` and `:2052`; `just repo-check` → exit 0; workspace clippy → exit 0 after one `field_reassign_with_default` fix. Two additions to §2.6: `SaveError::Serialize` for a game type that fails to serialize (no listed variant fit), and `tungsten_core::save::Value`, a re-export of `serde_json::Value`, so a game, the template and the umbrella test write a migration without their own `serde_json` dependency. The template treats `NoFolder` like `NotFound` on load and stays silent on a `NoFolder` save | `crates/tungsten-core/src/save.rs`, `tests/save.rs`; `crates/tungsten/src/tests/save_slots.rs`; `templates/basic/src/states.rs`; `D-160` |
| 5 | 2026-10-14 | pass | `just api` → exit 0, `git diff --stat -- api/` → only `api/tungsten-core.txt` (+301) and `api/tungsten.txt` (+1), no line removed (`comm -23` against `HEAD` → 0 each: `cargo-public-api` lists the `GameConfig` field and the `ScaleFactor` methods as new lines, so nothing is rewritten); `just api-check` → exit 0; `just ctx` → `Agent context checks: OK`, `LLM_INDEX.md` 12275 B (rows 18, 19 and 21 name the new files and `D-159`/`D-160`; thirteen other rows lost words, none dropped); `just repo-check` → exit 0; `rg 'Core F4' docs/known-issues.md` → no match; `rg '^- Save / load$' DESIGN.md` → no match; smoke with `TUNGSTEN_USER_DIR` holding exclusive fullscreen 640×360, vsync, UI scale 2 and volume 0 → `just smoke` exit 0 (examples 5/5, template 1/1, matrix 4/4, benchmarks 15/15, every row OK), `cmp` silent, `fd` → `before.json`, `settings/`, `settings/settings.json` only; `just checks` → `just deps`, `repo-check`, `ctx` and `api-check` run here, `just check`, `just smoke` and `just template-check` named for the release; no perf row (plan §8), no `just visual` (not run, no renderer change) | `api/`, `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`, `w11-shipping-basics.md`, `w06-gameplay-systems.md`; smoke log in the session scratchpad |
| 6 | 2026-10-14 | closed; cut next | Workflow §5 before the cut: (1) rows 1–5; (2) each step's crate and template tests, smoke with a user file in step 5, the release chain once after the cut, its output in the session report since this plan is archived first; no perf row and no `just visual` (§8: no frame-loop, ECS or renderer change), pixel comparisons not run; (3) the engine schedule unchanged (only the template's snapshot moved, on purpose), smoke rows OK with a user file present; the physics hashes are pinned by `just check`'s tests in the release chain; (4) `D-159` and `D-160` landed before the guide text citing them; (5) the ledger's two rows, rustdoc on every new public item, `just api` regenerated and reviewed; (6) the template's mute key and autosave, the guide's Settings and saves section; (7) the LLM index rows, `just ctx` OK; (8) DESIGN's M53 section and status line; (9) Core F4 out of known issues, the register row at 0.73, Q15's default in the W11 file, W11 and W6 current, the README's owned fields from `just release-cut`; (10) one `CHANGELOG.md` entry, `status: done`, this plan archived. Then `just release-cut 0.73.0`, the release chain and the preflight | `CHANGELOG.md`, `DESIGN.md`, `docs/plans/1.0/implementation-plan.md` |

## Follow-ups

- W6c: volume per bus joins `Settings` beside the master volume (w06 follow-up, step 5).
- W1 M4: whether F11 persists once the settings screen exists (Q4); the screen writes through `Settings`, `apply_settings` and `save`.
- W4b: `ActionMapError` takes `#[non_exhaustive]` and a serialize variant in the sweep (Q7).
- Records: `docs/plans/1.0/roadmap.json` keeps question `q15` at `"state": "open"`; it lies outside this plan's files, so the next records pass sets it to `default` with the answer `D-159` records.
- W1 M4: `apply_settings` returns a window resized by hand to the set or shipped resolution, since a resize persists nothing (`D-159`); a settings screen that offers a free window size would persist it as the resolution.

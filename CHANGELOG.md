# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.4.0-alpha] - 2026-04-13

Phase 2 Milestone 9 — Hot Reload.

### Added

- **Hot reload watcher:** `HotReloadWatcher` uses `notify` v6 (`RecommendedWatcher`) to watch the `assets/` directory on a background thread. Events cross to the main thread via `std::sync::mpsc` only — no `Arc<Mutex>`, no async (D-031).
- **50ms debounce:** Events are coalesced per path; a path is only dispatched to the reload handler after no new events have arrived for 50ms. Collapses editor double-writes into a single reload per save.
- **Sprite hot reload:** Editing a PNG re-uploads the decoded RGBA bitmap behind the same `TextureHandle`. If dimensions change the old `wgpu::Texture` is replaced in-place (deferred GPU destruction). No restart needed.
- **Animation hot reload:** Editing an animation JSON file reparses the data and replaces the entry in `AnimationRegistry` live. Running `AnimationState` components pick up the new frame timings on the next advance.
- **Font hot reload:** Editing a TTF/OTF removes the old `fontdb` face IDs, trims the glyph atlas, and re-registers the new bytes — text using that font updates within a few frames.
- **Manifest hot reload:** Adding entries to `assets/manifest.json` while running loads new sprites, animations, and fonts immediately. Removed entries log a warning and stay stale (no crash). Duplicate IDs log an error and are skipped.
- **`App::enable_hot_reload(assets_dir, manifest_path)`:** Opt-in per example. Has no effect if the watcher fails to start (the error is logged and the engine continues without hot reload).
- **`FontRegistry` resource:** New resource in `tungsten-core` tracking path→font ID for hot-reload reverse lookup. Inserted by `load_fonts`.
- **`AnimationRegistry` path index:** Added `insert_with_path`, `id_for_path`, `ids()` to `AnimationRegistry`.
- **`AssetRegistry` path index:** Added `path` field to `SpriteAsset`, `path_to_sprite_id` reverse map, `sprite_id_for_path`, `update_sprite_dimensions`.
- **`example-08-hot-reload`:** Demonstrates all three live asset types — a static sprite, a walk-cycle animation, and an instruction text label. Edit any of the watched files while the example is running; no restart needed.
- **DECISIONS.md D-031:** `notify` v6 rationale under D-015 rule 1.

### Changed

- Workspace version bumped to `0.4.0-alpha`.
- `load_fonts` now takes `world: &mut World` to insert the `FontRegistry` resource.
- `register_sprite` now takes a `path: PathBuf` parameter (stored for hot-reload reverse lookup).
- AGENTS.md, CLAUDE.md, DESIGN.md: status updated to M9 complete, M10 tilemaps next.
- PHASE2.md: M7/M8 condensed; M9 marked complete with all acceptance criteria checked.

## [0.3.0-alpha] - 2026-04-13

Phase 2 Milestone 8 — Audio.

### Added

- **Audio subsystem:** `cpal` output device init with a hand-rolled mixer running on a dedicated callback thread. Game code writes to `AudioCommands` resource; the audio thread drains it each callback. No async runtime (D-027, D-029).
- **Sound decoding:** `symphonia` decodes OGG/WAV/MP3/AAC files eagerly at startup into `SoundData` (f32 PCM). Linear interpolation resampling and mono→stereo upmix happen at decode time, so the mixer callback stays simple (D-028).
- **Sound manifest section:** `assets/manifest.json` extended with a `sounds` section (`looping`, `volume` fields). Sounds are loaded by string ID — consistent with the sprite/animation/font registry pattern.
- **Audio registry:** `SoundRegistry` resource maps string IDs → `AudioHandle(u32)` and stores manifest-declared default volume and looping per handle (`get_volume()`, `get_looping()`). `AudioHandle` is opaque and cheap to copy.
- **`AudioCommands` resource:** `play()`, `play_looping()`, `play_with()`, `stop()`, `stop_all()`, `set_master_volume()` — synchronous API from any system.
- **`AudioSystem` integration in `App`:** Initialized after the startup callback (so sounds are decoded first). Non-fatal if no audio device is available (logs a warning and continues).
- **`KeyCode` variants:** Added `KeyM`, `Digit1`, `Digit2`, `Digit3` to the engine key enum and input bridge.
- **`exit_on_escape` on `App`:** `set_exit_on_escape(false)` lets game code claim the Escape key for pause menus.
- **`assets/sounds/`:** `sfx_blip.ogg` (short one-shot blip) and `music_main.ogg` (30-second looping tone).
- **`example-07-audio`:** Demonstrates one-shot SFX (Space), looping music toggle (M), master volume levels (1/2/3), and stop-all (S), with live status text using M7 fonts.
- **Asset smoke test** (`crates/tungsten/tests/asset_smoke.rs`): headless integration test that loads the workspace manifest, decodes all animations and sounds, and runs as part of `cargo test --workspace` — catches codec/format bugs before example runtime.
- **DECISIONS.md D-027–D-030:** `cpal`, `symphonia`, hand-rolled mixer, and M12 conditional framing.

### Changed

- Workspace version bumped to `0.3.0-alpha`.
- AGENTS.md: structured AI session workflow (startup checklist, session types, principles checklist); font family directory exception documented.
- DESIGN.md: audio architecture section, resolved Phase 2 gating questions table.
- PHASE2.md: M8 complete, M12 conditional on ECS pain.
- CLAUDE.md: current status updated to M8 complete; font family exception documented.

### Fixed

- **OGG Vorbis playback:** Added `vorbis` feature to the `symphonia` workspace dependency. The `ogg` feature only enables the container demuxer; `vorbis` is the required codec. Without it, any OGG file panicked at runtime with "unsupported codec".
- **Manifest sound defaults ignored:** `SoundRegistry::register()` now accepts `volume` and `looping` and stores them per handle. `load_sounds()` passes the manifest-declared values. Previously the `volume` and `looping` fields in the manifest `sounds` section were parsed but silently dropped, so all sounds played at volume 1.0 regardless of their manifest entries.
- **`example-07-audio` volume mixing:** The example now issues `play_with(handle, manifest_volume, looping)` and relies on `set_master_volume` for global scaling, rather than incorrectly passing the master volume as the per-sound volume.

## [0.2.0-alpha.0] - 2026-04-12

Phase 2 Milestone 7 — Text rendering.

### Added

- **Text rendering pipeline:** GPU text rendering via `glyphon` (built on `cosmic-text` + `swash`), integrated alongside the existing quad and sprite pipelines in `tungsten-render` (D-026).
- **Font manifest section:** `assets/manifest.json` extended with a `fonts` section. Fonts are loaded by string ID, never by file path — consistent with the sprite/animation registry pattern.
- **Font loading:** TTF/OTF files decoded and registered at startup. Three font families staged in `assets/fonts/`: Inter (sans), Source Serif 4 (serif), JetBrains Mono (mono).
- **Text extraction API:** `ExtractTextFn` added to `App`; `TextSection` type in `tungsten-render` for specifying text content, position, font ID, size, and color. The renderer resolves font IDs at draw time via an internal atlas.
- **`example-06-text`:** Demonstrates multi-font text rendering, labels at fixed positions, and a live FPS overlay using the debug text path.
- **DECISIONS.md D-026:** Rationale for `glyphon`/`cosmic-text` under D-015 rule 2.

## [0.1.0-alpha] - 2026-04-12

Phase 1 complete (milestones M0 through M6).

### Added

- **Workspace scaffold:** Three-crate Cargo workspace (`tungsten-core`, `tungsten-render`, `tungsten`) with pinned dependencies and `rust-toolchain.toml`.
- **Hand-rolled ECS:** `World` with entity lifecycle, type-erased component storage, singleton resources, and typed queries (`query`, `query_entities`). Panic on programmer error, `Option` on runtime lookups (D-022).
- **wgpu renderer:** GPU initialization, surface management, window resizing, and a clear-color render pass. Shaders embedded via `include_str!` from `.wgsl` files (D-023).
- **Colored-quad pipeline:** Instanced rendering of axis-aligned colored rectangles with an orthographic camera.
- **Textured-sprite pipeline:** Instanced sprite rendering with per-sprite nearest/linear filter modes (D-011), a GPU texture pool keyed by opaque `TextureHandle`s (D-016), and alpha blending.
- **Data-driven config:** `tungsten.json` loaded at startup via `serde_json`, with sensible defaults when the file is missing (D-008).
- **Manifest-driven asset loading:** `assets/manifest.json` registers sprites and animations by string ID. Paths resolve relative to the manifest. Multiple manifests compose by extension with fatal duplicate-ID checks (D-017). Validation catches missing files and unresolved sprite references at load time (D-009).
- **Frame-based animation:** Custom JSON animation format with per-frame sprite IDs and durations (D-010). `AnimationState` component advances frames, supports looping and one-shot playback, and guards against zero-duration infinite loops.
- **Edge-triggered input:** Keyboard and mouse input with `is_pressed`, `just_pressed`, and `just_released` semantics. Engine-specific key/button enums decoupled from `winit` via an input bridge.
- **Frame timing:** `DeltaTime` resource updated each frame.
- **Five examples:** `01_window` (clear screen), `02_ecs` (stdout ECS demo), `03_dots` (bouncing quads with keyboard/mouse input), `04_sprites` (textured sprites from manifest), `05_animation` (looping walk cycle).
- **MIT license.**

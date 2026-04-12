# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

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

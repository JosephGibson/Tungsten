# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

The canonical instruction file for all AI assistants is **`AGENTS.md`** — read it for the full operational rules. This file inlines the essentials so Claude Code doesn't require a second read.

---

## What Tungsten is

A from-scratch Rust 2D game engine (hobby project). `winit` + `wgpu` + `glam` + hand-rolled ECS + manifest-driven assets. Three crates in a Cargo workspace: `tungsten-core`, `tungsten-render`, `tungsten`.

**Phase 1 complete (M0–M6). Phase 2 in progress: M7 text rendering complete. Next: M8 audio.** See `PHASE2.md`.

## Commands

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets   # advisory only
cargo fmt --all

# Run examples
cargo run -p example-01-window
cargo run -p example-02-ecs
cargo run -p example-03-dots
cargo run -p example-04-sprites
cargo run -p example-05-animation
cargo run -p example-06-text
```

Before committing anything substantial: `cargo fmt && cargo test --workspace`.

`cargo test --workspace` runs unit tests only — no GPU or display required. Examples need a real GPU and display; if wgpu auto-selects the wrong backend, override it: `WGPU_BACKEND=vulkan` (Linux), `WGPU_BACKEND=metal` (macOS), `WGPU_BACKEND=dx12` (Windows).

## Crate layout and where new code goes

```
crates/
├── tungsten-core/    # ECS, math, config, time, resources, asset registry types
├── tungsten-render/  # wgpu wrapper, sprite drawing, samplers, GPU resource pools
└── tungsten/         # umbrella crate: winit app loop, App type, input, time glue
```

- New ECS mechanism (World, storage, queries) → `tungsten-core`
- New rendering primitive (pipeline, texture, buffer, sampler) → `tungsten-render`
- App/event-loop glue, input, time → `tungsten`
- Asset registry types, manifest schema, ID lookups → `tungsten-core`
- GPU upload of decoded assets → `tungsten-render`
- **The seam:** `TextureHandle(u32)` is defined in `tungsten-core` — no `wgpu` types ever appear there. The `tungsten` umbrella crate is the mediator: during startup it calls `AssetRegistry::register_sprite` (allocates a handle, stores metadata in core), then calls `renderer.upload_texture(handle, rgba, ...)` (stores the actual GPU texture in render's pool, keyed by the same handle). Core never calls into render. Game code looks up sprites by string ID → gets a `TextureHandle` from core's registry → render resolves that handle to a `wgpu` texture internally.
- Components and systems specific to a demo → stay in `examples/`, not library crates
- Math helpers → `tungsten-core` only if used in two or more places
- `tungsten-render` may depend on `tungsten-core` types (see `DECISIONS.md` D-007)

## Architecture

**Single-threaded, fixed-order, synchronous frame loop:** poll events → tick systems → render → present. Exception: the audio subsystem (M8+) runs `cpal`'s callback on a dedicated thread. Game code writes to an `AudioCommands` resource; the audio thread drains it each callback. No shared mutable state, no async runtime.

**ECS:** Entity (opaque integer ID), Component (plain data), System (function), World (owns everything), Resource (singleton state — `DeltaTime`, `InputState`, `WindowSize`, `Assets`). Iteration-1 storage is `HashMap<TypeId, HashMap<EntityId, Box<dyn Any>>>` — intentionally naive.

**Asset registry is a Resource in the World**, not a global singleton.

**Render path:** systems mutate World during `tick`; extract functions receive `&World` and resolve string IDs → `TextureHandle` via `AssetRegistry`, producing `SpriteBatch`/`QuadInstance`/`TextSection` slices with all handles pre-resolved. Only those POD slices reach `render_frame_full`. The renderer never reads the registry at draw time.

**Config:** `tungsten.json` at workspace root, loaded once at startup, passed by value. Missing → defaults with warning, invalid → fatal.

## Asset rules

- Every **asset file** in `assets/` must be in `assets/manifest.json`, and vice versa. Asset files are sprites (PNG), animations (JSON), fonts (TTF/OTF), and sounds. Non-asset files (READMEs, platform detritus) are ignored by the loader.
- **Shaders** (`*.wgsl`) live in `tungsten-render/src/` and are compiled into the binary — they are not manifest-tracked.
- **Game code never references file paths.** Always use string IDs through the registry. This is the architectural prerequisite for Phase 2 hot reload — do not break it.
- New sprite: drop PNG in `assets/sprites/`, add entry to manifest's `sprites` map with a stable ID and filter mode (`nearest` or `linear`).
- New animation: create JSON in `assets/animations/`, add entry to manifest's `animations` map. All referenced sprite IDs must exist in the manifest.
- New font: drop TTF/OTF in `assets/fonts/<Family>/`, add entry to manifest's `fonts` map with a stable ID.
- Example-local assets: `examples/NN_name/assets/` with a local `manifest.json`. Asset IDs must be globally unique across all loaded manifests — duplicate IDs are fatal at load time.

## Hard rules — do not violate

- **No external ECS or game-engine crates.** Not `bevy_ecs`, `hecs`, `specs`, `legion`, `ggez`, `macroquad`, `fyrox`. Building these by hand is the point.
- **No `async` runtimes** (`tokio`, `async-std`). The frame loop and all game logic are synchronous. The `cpal` audio callback thread (M8+) is the only background thread permitted — it communicates via `AudioCommands`, not an async runtime.
- **No global mutable state.** No `static mut`, no `lazy_static` singletons.
- **No new third-party runtime dep without a `DECISIONS.md` entry** — explain which of the three dependency rules it satisfies (platform API abstraction, well-specified data format, or math primitive).
- **No hardcoded asset paths in game code.**
- **No scope-expanding a task mid-flight.** Finish what's scoped, open a new task for the rest.

## Conventions

- `rustfmt` defaults. Doc comments on public items where the name isn't self-evident.
- `unwrap`/`expect` fine during early exploration; tighten when a module stabilizes.
- Tests next to the code: `#[cfg(test)] mod tests`.
- `thiserror` at library boundaries, `anyhow` at the top level of examples and the app.
- `log` crate for diagnostics. `println!` is fine in examples.

## Key documents

| File           | Purpose |
|----------------|---------|
| `AGENTS.md`    | Full operational rules — read this for any substantial task |
| `DESIGN.md`    | Architecture, principles, milestones, kill criteria |
| `DECISIONS.md` | Append-only log of non-obvious decisions with rationale |
| `PHASE2.md`    | Phase 2 milestones (M7+), release map, acceptance criteria |

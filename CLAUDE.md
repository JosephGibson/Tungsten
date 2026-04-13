# CLAUDE.md

Guidance for Claude Code (claude.ai/code) in this repository.

The canonical instruction file for all AI assistants is **`AGENTS.md`** — read it for the full operational rules. This file inlines the essentials so Claude Code doesn't need a second read.

---

## What Tungsten is

A from-scratch Rust 2D game engine (hobby project). `winit` + `wgpu` + `glam` + hand-rolled ECS + manifest-driven assets. Three crates in a Cargo workspace: `tungsten-core`, `tungsten-render`, `tungsten`.

**Status:** Phase 1 complete. Phase 2 through M10 complete (`v0.5.0-alpha`, branch `0.5`). Current milestone: M11 2D physics. See `PHASE2.md`.

## Commands

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets   # advisory only
cargo fmt --all

cargo run -p example-NN-name              # see examples/ for the current list
```

Before committing anything substantial: `cargo fmt && cargo test --workspace`.

`cargo test --workspace` runs unit tests only — no GPU or display required. Examples need a real GPU and display; override the backend if wgpu picks the wrong one: `WGPU_BACKEND=vulkan` (Linux), `WGPU_BACKEND=metal` (macOS), `WGPU_BACKEND=dx12` (Windows).

**Two test layers** (details in `AGENTS.md` → *Test layers*):

- **Layer 1** — [crates/tungsten-core/tests/manifests.rs](crates/tungsten-core/tests/manifests.rs) validates every `manifest.json` under `cargo test --workspace`. Run before any commit touching manifests, assets, or the core/render seam.
- **Layer 2** — [scripts/smoke-examples.sh](scripts/smoke-examples.sh) runs every example with `TUNGSTEN_SMOKE_FRAMES=3` (honoured by [crates/tungsten/src/app.rs](crates/tungsten/src/app.rs)) under a per-example timeout. Run before committing engine or example wiring changes. Needs GPU/display.
- Clean checkout or dep bump → run both.

## Crate layout and where new code goes

```
crates/
├── tungsten-core/    # ECS, math, config, time, resources, asset registry types
├── tungsten-render/  # wgpu wrapper, sprite drawing, samplers, GPU resource pools
└── tungsten/         # umbrella: winit app loop, App type, input, time glue
```

- New ECS mechanism (World, storage, queries) → `tungsten-core`
- New rendering primitive (pipeline, texture, buffer, sampler) → `tungsten-render`
- App/event-loop glue, input, time → `tungsten`
- Asset registry types, manifest schema, ID lookups → `tungsten-core`
- GPU upload of decoded assets → `tungsten-render`
- Demo-specific components/systems → `examples/`, not library crates
- Math helpers → `tungsten-core` only when used in two or more places
- `tungsten-render` may depend on `tungsten-core` types (see `DECISIONS.md` D-007)

### The core/render seam

- `TextureHandle(u32)` is defined in `tungsten-core`; no `wgpu` types ever appear there.
- The `tungsten` umbrella crate mediates: it calls `AssetRegistry::register_sprite` (allocates a handle, stores metadata in core), then `renderer.upload_texture(handle, rgba, ...)` (stores the GPU texture in render's pool, keyed by the same handle).
- Core never calls into render.
- Game-code lookup path: string ID → `TextureHandle` from core's registry → render resolves internally to a `wgpu` texture.

## Architecture

**Frame loop.** Single-threaded, fixed-order, synchronous: poll events → tick systems → render → present. Exception: the audio subsystem (M8+) runs `cpal`'s callback on a dedicated thread; game code writes to an `AudioCommands` resource which the audio thread drains each callback. No shared mutable state, no async runtime.

**ECS.**
- Entity: opaque integer ID (`u32`)
- Component: plain data
- System: a function
- World: owns entities, components, and resources
- Resource: singleton state — `DeltaTime`, `InputState`, `WindowSize`, `Assets`
- Iteration-1 storage: `HashMap<TypeId, HashMap<EntityId, Box<dyn Any>>>` — intentionally naive

**Asset registry** is a `Resource` in the World, not a global singleton.

**Render path.** Systems mutate the World during `tick`. Extract functions receive `&World` and resolve string IDs → `TextureHandle` via `AssetRegistry`, producing `SpriteBatch` / `QuadInstance` / `TextSection` slices with handles pre-resolved. Only those POD slices reach `render_frame_full`. The renderer never reads the registry at draw time.

**Config.** `tungsten.json` at workspace root, loaded once at startup, passed by value. Missing → defaults with warning. Invalid → fatal naming the bad field.

## Asset rules

- Every **asset file** in `assets/` must be in `assets/manifest.json`, and vice versa. Asset files are sprites (PNG), animations (JSON), fonts (TTF/OTF), and sounds (OGG/WAV). Non-asset files (READMEs, platform detritus) are ignored.
- **Exception:** font family directories (`assets/fonts/<Family>/`) may contain the full downloaded family (all weights and styles); only weights in active use need manifest entries. Unused weights are never loaded.
- **Shaders** (`*.wgsl`) live in `tungsten-render/src/` and are compiled in via `include_str!`. Not manifest-tracked.
- **Game code never references file paths.** Always use string IDs through the registry. This invariant is what makes hot reload (M9) work — do not break it.
- **Example-local assets**: `examples/NN_name/assets/` with a local `manifest.json`. Asset IDs must be globally unique across all loaded manifests — duplicate IDs are fatal at load time.

Adding a new asset:

| Type      | Location              | Manifest section | Required fields                            |
| --------- | --------------------- | ---------------- | ------------------------------------------ |
| Sprite    | `assets/sprites/`     | `sprites`        | stable ID, filter (`nearest` \| `linear`)  |
| Animation | `assets/animations/`  | `animations`     | stable ID; referenced sprite IDs must exist|
| Font      | `assets/fonts/<Fam>/` | `fonts`          | stable ID                                  |
| Sound     | `assets/sounds/`      | `sounds`         | stable ID, optional `looping` / `volume`   |

## Hard rules — do not violate

- **No external ECS or game-engine crates** (`bevy_ecs`, `hecs`, `specs`, `legion`, `ggez`, `macroquad`, `fyrox`). Building them by hand is the point.
- **No async runtimes** (`tokio`, `async-std`). The `cpal` audio callback thread is the only permitted background thread; it communicates via `AudioCommands`.
- **No global mutable state.** No `static mut`, no `lazy_static` singletons.
- **No new third-party runtime dep without a `DECISIONS.md` entry** citing which D-015 rule it satisfies (platform API abstraction, well-specified data format, or math primitive).
- **No hardcoded asset paths in game code.**
- **No scope-expanding a task mid-flight.** Finish what's scoped; open a new task for the rest.

## Conventions

- `rustfmt` defaults. Doc comments on public items where the name isn't self-evident.
- `unwrap`/`expect` fine during early exploration; tighten when a module stabilizes.
- Tests next to the code: `#[cfg(test)] mod tests`.
- Errors: `thiserror` at library boundaries, `anyhow` at the top level of examples and the app.
- Logging: `log` crate for diagnostics; `println!` fine in examples.

## Key documents

| File           | Purpose                                                         |
| -------------- | --------------------------------------------------------------- |
| `AGENTS.md`    | Full operational rules — read for any substantial task         |
| `DESIGN.md`    | Architecture, principles, dependency philosophy                 |
| `DECISIONS.md` | Append-only log of non-obvious decisions with rationale         |
| `PHASE2.md`    | Phase 2 milestones (M7+), release map, acceptance criteria      |

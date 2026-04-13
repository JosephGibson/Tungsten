# Tungsten — Design Document

**Status:** Phase 1 complete (M0–M6). Phase 2 through M9 complete (`v0.4.0-alpha`); M10 tilemaps next. Current branch: `0.4`. Milestone detail lives in `PHASE2.md`.
**Project:** Tungsten, a from-scratch Rust 2D game engine.
**Companion docs:** `AGENTS.md` (how to work in the repo), `DECISIONS.md` (decision log).

---

## What this is

Tungsten is a hobby project. The point is the *building*, not the shipping. I'm writing it because building a game engine looks fun, because I want to understand how engines actually work from the ground up, and because Rust is a language I want to spend serious time with. If it ever runs a real game, that's a bonus, not the goal.

This framing matters because it changes what "good" means. A commercial engine is good when it ships games fast. A learning engine is good when the person building it keeps wanting to come back to it on a Saturday morning. Those are different optimization targets and I'm optimizing for the second one.

**2D only.** No 3D math beyond what `glam` gives me, no model formats, no skeletal animation, no PBR, no lighting models. This isn't just a simplification — 2D has a design space that's actually tractable for one person, with visible results at every step.

## Principles

Five things, in priority order. Everything else bends to these.

1. **Fun first.** If a decision makes the project less fun, it's the wrong decision, even if it's technically better. Tedium is the failure mode to avoid.
2. **From scratch where practical.** No game-engine crates, no ECS crates, no rendering helpers. Low-level building blocks are fine; anything that would hand me an engine for free is not. The test is spelled out in the Dependency philosophy section below.
3. **wgpu for rendering.** Modern GPU concepts at a manageable level.
4. **ECS-first for game state.** Built by hand because understanding ECS internals is one of the main things I'm here for.
5. **Data over code for content.** Engine configuration, asset registration, and animation definitions all live in JSON. Rebuilding to tweak a number is a design failure.

## Stack

| Concern    | Choice                 | Role                                           |
| ---------- | ---------------------- | ---------------------------------------------- |
| Windowing  | `winit`                | OS window and event abstraction.               |
| Rendering  | `wgpu`                 | GPU API abstraction.                           |
| Math       | `glam`                 | Vectors, matrices, transforms.                 |
| ECS        | **hand-rolled**        | Naive first, improve based on real pain.       |
| Config     | `serde` + `serde_json` | JSON schema derive + parsing.                  |
| Image      | `image` crate          | PNG decoding to CPU bitmaps.                   |
| Logging    | `log` + `env_logger`   | Standard facade + basic backend.               |
| Errors     | `thiserror` / `anyhow` | Typed at library boundaries, anyhow at the top.|

Explicitly not in Phase 1: async runtimes, `rayon`, audio, physics, networking, scripting. Phase 2 adds audio (M8), hot reload (M9), tilemaps (M10), and physics (M11); see `PHASE2.md`.

### Dependency philosophy

A crate is acceptable if **at least one** of these is true:

1. **It abstracts a platform API** I would otherwise have to write OS-specific code for. `winit` (windowing), `wgpu` (GPU), future `notify` (file watching), future `cpal` (audio devices). Writing these by hand would mean three codepaths per feature and no learning payoff.
2. **It implements a well-specified data format** that isn't the interesting part of what I'm building. `serde_json` (JSON), `image` (PNG). Reinventing format parsers is a side quest that doesn't teach engine architecture.
3. **It provides a primitive that's math, not architecture.** `glam` falls here — linear algebra is a solved problem and writing my own vector types would not improve my understanding of ECS or rendering.

A crate is **not** acceptable if it hands me something the project is supposed to teach me how to build. `bevy_ecs`, `hecs`, `specs`, `legion` are all out because ECS *is* the interesting thing. `ggez`, `macroquad`, `fyrox` are out because they *are* the engine. For audio (M8): `cpal` satisfies rule 1 (platform API), `symphonia` satisfies rule 2 (data format); the mixer is hand-rolled because it's one of the things M8 is here to teach. `rodio` and `kira` are out (see D-029).

When a new dep is being considered, it gets a `DECISIONS.md` entry that identifies which rule it satisfies and what the alternative would have been. If none of the three rules clearly apply, the answer is no.

## Architecture

### Frame loop

Single-threaded, fixed-order, synchronous. Exception: the audio subsystem (M8+) runs `cpal`'s callback on a dedicated thread. Game code writes to an `AudioCommands` resource; the audio thread drains it each callback. No shared mutable state, no async runtime — the game loop itself stays single-threaded.

```
init:
    parse tungsten.json → EngineConfig
    open window (winit)
    init wgpu (instance, adapter, device, queue, surface)
    build renderer (pipelines, samplers, GPU resource pools)
    load assets/manifest.json → validate → decode PNGs → upload textures
    build World; insert Resources: DeltaTime, InputState, Assets, WindowSize

loop:
    poll events     → drain winit events into InputState resource
    tick            → update DeltaTime; run systems in declared order
    render          → extract renderables from World; record + submit draw calls
    present         → swap buffers

shutdown:
    drop World (drops the asset registry and its opaque handles)
    drop renderer (releases GPU resources behind those handles)
    tear down wgpu in order
    close window
```

**Phase 1 system ordering:** systems live in the `tungsten` app layer as a plain manually-ordered list. Registration order is execution order. No scheduler, labels, or dependency graph until a real need appears. The goal in M2–M4 is that input → simulation → animation ordering is obvious from reading one place.

Parallelism, parallel system scheduling, and fixed-timestep simulation are all explicitly deferred. Revisit if and when there's a real reason.

### ECS sketch

- **Entity** — opaque integer ID.
- **Component** — plain data attached to an entity.
- **System** — a function that reads and writes the world to advance one tick.
- **World** — owns entities, components, and resources.
- **Resource** — singleton state not tied to any entity. Examples: `DeltaTime`, `InputState`, `WindowSize`, `Assets`.

**Iteration 1 storage:** a `HashMap<TypeId, HashMap<EntityId, Box<dyn Any>>>` or close to it. Bad cache behavior, fine for learning.

**Query model for Iteration 1:** iterate one component type, look up others per-entity. So a movement system iterates entities that have `Velocity`, and for each one looks up its `Position`. The outer loop picks which store to walk; the inner lookups hit the hash maps. This is O(n) with bad constant factors and it is *the entire point* — feeling this cost is what motivates any future archetypal rewrite. If the naive version stays good enough for everything I actually do, that's a valid end state, not a failure.

**The asset registry is a Resource.** It lives in the `World` alongside `DeltaTime` and `InputState`, not as a separately-passed object. This keeps the engine's "global-ish" state on a single pathway and makes the registry accessible to any system that needs it by the same mechanism as everything else.

**Renderer-ECS coupling:** `tungsten-render` may depend on `tungsten-core` and use its types where it makes the glue simpler. The renderer is not *required* to be ECS-driven — direct-data APIs should also exist so the renderer can be tested against hand-built data — but the separation is not a rule.

**Phase 1 render path:** systems mutate the `World` during `tick`. Extract functions then receive `&World` and resolve string IDs → `TextureHandle` via `AssetRegistry`, producing `SpriteBatch`/`QuadInstance`/`TextSection` slices with all handles pre-resolved. Only those POD slices are passed to `render_frame_full`. The renderer never reads the registry at draw time. This keeps borrow-checker pressure contained and preserves a direct-data API for testing.

### Data-driven config

A single `tungsten.json` at the workspace root, loaded once at startup, validated, then passed by value into whichever subsystem needs its slice. No global, no hot reload for config, missing file → defaults with a warning, invalid file → fatal naming the bad field.

```json
{
  "window": { "title": "Tungsten", "width": 1280, "height": 720, "vsync": true },
  "render":  { "clear_color": [0.05, 0.05, 0.08, 1.0] },
  "logging": { "level": "info" }
}
```

## Assets and content

The second data-driven layer. Config is for engine settings; the asset manifest is for game content.

### Manifest-driven, ID-referenced
A single `assets/manifest.json` lists every asset the engine knows about. Game code references assets by **string ID**, never by file path. The manifest is the registry; the IDs are the API.

Three reasons this is worth the slight extra ceremony:

1. **Decoupling.** Renaming or moving a file is a manifest edit, not a code change. Game code says `"player_idle"`, not `"sprites/characters/player/idle.png"`.
2. **Single source of truth.** When something is missing or broken, the manifest is where you check first. No grepping for hardcoded paths.
3. **Foundation for hot reload.** Once assets are loaded by ID through a registry, swapping a texture is straightforward. Without the indirection, hot reload would mean chasing down every reference site.

### Manifest shape

```json
{
  "sprites": {
    "player_idle": {
      "path": "sprites/player_idle.png",
      "filter": "nearest"
    },
    "player_walk_0": {
      "path": "sprites/player_walk_0.png",
      "filter": "nearest"
    },
    "ui_button": {
      "path": "sprites/ui_button.png",
      "filter": "linear"
    }
  },
  "animations": {
    "player_walk": { "path": "animations/player_walk.json" }
  }
}
```

Paths are relative to the manifest file. Animations point to their own JSON files rather than being inlined — animation data grows and inlining would make the manifest hostile to read.

### Multiple manifests

When multiple manifests are loaded together, they **compose by extension, not override**.

- Asset IDs must be globally unique across the merged manifest set.
- Duplicate IDs are fatal at load time.
- Each path is resolved relative to the manifest file that declared it.
- A later manifest may reference IDs introduced by an earlier one, but it may not replace them.

This keeps composition predictable and prevents an example-local asset from silently shadowing a shared one.

### Animation format
Frame-based: a sequence of sprite IDs with per-frame durations. Each animation lives in its own JSON file under `assets/animations/`:

```json
{
  "looping": true,
  "frames": [
    { "sprite": "player_walk_0", "duration_ms": 100 },
    { "sprite": "player_walk_1", "duration_ms": 100 },
    { "sprite": "player_walk_2", "duration_ms": 100 },
    { "sprite": "player_walk_3", "duration_ms": 100 }
  ]
}
```

Sprite IDs must resolve through the manifest — validation catches typos at load time. Per-frame durations (rather than a fixed framerate) keep simple things simple while allowing emphasis frames to hold longer. Custom JSON over Aseprite's export format is a deliberate choice: tiny dep surface, format under my control, schema can evolve.

One ECS component holds animation state (current animation ID, current frame index, accumulated time), one system advances it each tick. ~100 lines.

### Filtering — supporting both pixel art and high-res
Filter mode is a **per-sprite** property in the manifest, not a global setting.

- **`nearest`** — crisp pixels, what pixel art needs.
- **`linear`** — bilinear filtering, what high-res sprites and UI usually want.

Default is `nearest`. The render layer creates a sampler per filter mode and binds the appropriate one when drawing each sprite. This is what makes mixed art styles in the same scene free — UI in linear, gameplay in nearest, both in the same frame.

### Directory layout

```
tungsten/
└── assets/
    ├── manifest.json
    ├── sprites/
    ├── animations/
    ├── fonts/
    └── sounds/
```

By-type at the top level. The manifest is easier to scan when sections match folders, browsing matches the manifest structure, and adding a new asset type later is just a new directory plus a new manifest section. Sub-organizing inside a type folder (`sprites/player/`, `sprites/ui/`) is fine when it helps.

Examples that need throwaway assets ship their own local `examples/NN_name/assets/` with a local manifest. The loader takes a manifest path, so multiple manifests compose.

### Hot reload — M9 (shipped)

A background thread runs `notify` on the assets directory and sends file-change messages to the main thread via `std::sync::mpsc`. At the next frame boundary the main thread resolves file paths back to asset IDs, decodes the new data, uploads to the GPU, and swaps the handle in the registry. Existing components that reference by ID pick up the new data automatically. Manifest changes trigger a reload-and-reconcile pass.

This works because the M5 architecture already enforces the registry-by-ID invariant: no game code holds direct GPU handles. Runtime cost is essentially zero in steady state. See `DECISIONS.md` D-031 for the `notify` decision and `PHASE2.md` M9 for acceptance criteria.

**Do not break the registry-by-ID invariant** in future work, even for short-term convenience — it's what makes hot reload feasible.

### Audio — M8 (shipped)

Manifest-driven: `assets/sounds/` holds OGG/WAV/MP3 files; the manifest's `sounds` section registers them by ID. Game code plays sounds by ID through an `AudioCommands` resource — the same ID-through-registry discipline as sprites and fonts.

`cpal` (D-027) opens the audio device. `symphonia` (D-028) decodes compressed audio at load time into `Vec<f32>` PCM — no decoder types appear at runtime in the callback. A hand-rolled mixer (D-029) runs in `cpal`'s callback thread; game systems write `AudioCommand` values each tick and the callback drains them via `mpsc::try_recv`. The `cpal` callback thread plus the M9 `notify` watcher thread are the only background threads in the engine. No async runtime, no shared mutable state.

## Project structure

```
tungsten/
├── Cargo.toml
├── tungsten.json           # engine config
├── README.md
├── DESIGN.md
├── AGENTS.md
├── DECISIONS.md
│
├── assets/                 # shared game assets
│   ├── manifest.json
│   ├── sprites/
│   ├── animations/
│   ├── fonts/
│   └── sounds/
│
├── crates/
│   ├── tungsten-core/      # ECS, math, config, time, resources, asset registry types
│   ├── tungsten-render/    # wgpu wrapper, sprite drawing, samplers
│   └── tungsten/           # umbrella + winit app loop
│
└── examples/                # 01_window .. 08_hot_reload (see examples/ for the full list)
```

The seam between core and render: `TextureHandle(u32)` is defined in `tungsten-core` — no `wgpu` types appear there. During startup the `tungsten` umbrella crate acts as mediator: core's `AssetRegistry` allocates handles and stores metadata; `tungsten` then calls `renderer.upload_texture(handle, rgba, ...)` to store the GPU resource in render's texture pool. Core never calls into render.

Those handles are **opaque runtime IDs/newtypes**, not raw `wgpu` texture objects. `tungsten-core` owns manifest data, decoded CPU-side asset data, animation data, and the registry shape; `tungsten-render` owns the actual GPU textures, samplers, and pipelines in an internal pool keyed by those opaque handles. This keeps `wgpu` out of `tungsten-core` while preserving the registry as the one lookup path for game-facing code.

Examples progress cumulatively — each one builds on the primitives established by earlier milestones. Phase 1 ended with `05_animation` (M6). Phase 2 adds `06_text` (M7), `07_audio` (M8), and `08_hot_reload` (M9); see `PHASE2.md` for the current milestone and versioning.

## Milestones

Phase 1 (M0–M6) is complete. Phase 2 milestones, acceptance criteria, and release map live in `PHASE2.md`. The Phase 1 exit-review gating questions (text, audio decoder, audio mixer, hot reload feasibility, ECS performance) are all resolved — see `DECISIONS.md` D-024, plus D-026 (glyphon), D-028 (symphonia), D-029 (hand-rolled mixer), D-030 (M12 conditional), D-031 (notify).

## Open questions

All Phase 1 open questions are resolved. See `DECISIONS.md`:

| Question | Decision | Ref |
|---|---|---|
| Entity ID shape | `u32`, no generational index in Phase 1 | D-021 |
| ECS error strategy | Panic on programmer error, `Result`/`Option` on runtime | D-022 |
| Renderer wgpu exposure | Wrap the happy path; opaque handles in core | D-016 |
| Fixed vs variable timestep | Variable; revisit only if simulation pain appears | — |
| Config format | JSON, single `tungsten.json`, no hot reload | D-008 |
| Asset layout | By-type, manifest-driven, ID-referenced | D-009, D-013 |
| Render/ECS coupling | `tungsten-render` may depend on `tungsten-core` | D-007 |
| Audio timing | M8 (Phase 2), `cpal` + hand-rolled mixer | D-027, D-029 |

No open questions remain for Phase 2 start. New questions that arise during Phase 2 milestones are logged in `DECISIONS.md` as they are resolved.

## Non-commitments

Not promising and not scoped. Any of these can appear in a future phase, but not without an explicit decision.

*Note: Audio (M8) and 2D physics (M11) are now committed and scoped in `PHASE2.md`. Items below remain uncommitted.*

- Networking
- Scripting
- Editor tooling
- Asset preprocessing / build pipeline
- 3D rendering
- WASM / browser support
- Hot reload of config (assets have it as of M9; config does not)
- Save / load
- Multiplayer
- GUI library
- Texture atlases / sprite sheet packing
- GPU-compressed texture formats (KTX2, Basis Universal)
- Skeletal animation
- Streaming or async asset loading
- Per-platform asset variants
- Tweened transforms as a separate animation system (may come later as a layer over frame-based)

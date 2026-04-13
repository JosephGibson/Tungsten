# AGENTS.md

Operational notes for working on Tungsten, whether by me or an AI assistant I'm collaborating with. Read this at the start of a work session. Read `DESIGN.md` for architectural context. Read `DECISIONS.md` when you need to know *why* something is the way it is.

## What Tungsten is

A from-scratch Rust 2D game engine built as a hobby project. Native only. `winit` + `wgpu` + `glam` + hand-rolled ECS + manifest-driven assets. Three crates in a Cargo workspace: `tungsten-core`, `tungsten-render`, `tungsten`.

The top priority is that working on this stays fun. Rules exist to protect that, not to gold-plate the code.

## Commands

From the workspace root:

```bash
# Build / test / lint
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all

# Run an example
cargo run -p example-01-window
cargo run -p example-02-ecs
cargo run -p example-03-dots
cargo run -p example-04-sprites
cargo run -p example-05-animation
cargo run -p example-06-text
cargo run -p example-07-audio
cargo run -p example-08-hot-reload
```

Before committing something substantial: `cargo fmt && cargo test --workspace`. That's the bar. Clippy is advisory.

`cargo test --workspace` runs unit tests only — no GPU or display required. Examples need a real GPU and display; override the backend if needed: `WGPU_BACKEND=vulkan` (Linux), `WGPU_BACKEND=metal` (macOS), `WGPU_BACKEND=dx12` (Windows).

## Repo layout

```
tungsten/
├── crates/
│   ├── tungsten-core/      # ECS, math, config, time, resources, asset registry
│   ├── tungsten-render/    # wgpu wrapper, sprite drawing, samplers
│   └── tungsten/           # umbrella + winit app loop + App type
├── assets/
│   ├── manifest.json
│   ├── sprites/
│   ├── animations/
│   ├── fonts/
│   └── sounds/
└── examples/
```

### Where new code goes
- New ECS mechanism (World, storage, queries) → `tungsten-core`.
- New rendering primitive (pipeline, texture, buffer, sampler) → `tungsten-render`.
- App/event-loop glue, input, time → `tungsten`.
- Asset registry types, manifest schema, ID lookups → `tungsten-core`.
- GPU upload of decoded assets → `tungsten-render`. The seam: `TextureHandle(u32)` is defined in `tungsten-core` (no `wgpu` types there). During startup the `tungsten` crate acts as mediator — core's `AssetRegistry` allocates handles and stores metadata; `tungsten` then calls `renderer.upload_texture(handle, rgba, ...)` to store the GPU resource in render's texture pool. Core never calls into render.
- Components and systems specific to a demo → stay in `examples/`, not in the library crates.
- Math helpers → `tungsten-core` only if used in two places.

`tungsten-render` is allowed to know about `tungsten-core` types if it makes the glue simpler. See `DECISIONS.md` D-007.

## Asset rules

Anything that lives in `assets/` should also be registered in `assets/manifest.json`. The reverse is also true — every entry in the manifest must point to a real file. The loader validates this at startup, but it's worth keeping the convention tight by hand. **Exception: font family directories** (`assets/fonts/<Family>/`) may contain the full downloaded family (all weights and styles); only the specific weights in active use need manifest entries — the rest are staged for future use and are never loaded.

- **New sprite:** drop the PNG in `assets/sprites/`, add an entry to the manifest's `sprites` map with a stable ID, decide its filter mode (`nearest` or `linear`).
- **New animation:** create a JSON file in `assets/animations/` per the schema, add an entry to the manifest's `animations` map. All sprite IDs referenced from the animation must exist in the manifest.
- **New font:** drop the TTF/OTF in `assets/fonts/<Family>/`, add an entry to the manifest's `fonts` map with a stable ID.
- **New sound:** drop the OGG/WAV in `assets/sounds/`, add an entry to the manifest's `sounds` map with a stable ID and optional `looping` / `volume` fields.
- **Shaders** (`*.wgsl`) live in `tungsten-render/src/` and are compiled into the binary — they are not manifest-tracked.
- **Examples that need their own assets:** put them in `examples/NN_name/assets/` with a local `manifest.json`. Asset IDs must be globally unique across all loaded manifests — duplicate IDs are fatal at load time.
- **Game code never references file paths.** Always reference assets by ID through the registry. This is the rule that makes Phase 2 hot reload feasible — don't break it for short-term convenience.

## Things to actually not do

- **No external game-engine or ECS crate.** Not `bevy_ecs`, `hecs`, `specs`, `legion`, `amethyst`, `fyrox`. Building these by hand is the point.
- **No `async` runtimes** (`tokio`, `async-std`). The frame loop and all game logic are synchronous. The `cpal` audio callback thread (M8+) is the only background thread permitted — it communicates with the main thread through an `AudioCommands` resource, not an async runtime.
- **No global mutable state.** No `static mut`, no `lazy_static` singletons. State lives in the `World` or is passed explicitly. The asset registry is a `Resource` in the World, not a global.
- **No new third-party runtime dep without a `DECISIONS.md` entry** explaining why. Dependency creep is how hobby projects become unmaintainable.
- **No hardcoded asset paths in game code.** Use IDs through the registry.
- **No scope-expanding a task mid-flight.** If the work grows, finish what's there and open a new task for the rest.

## Conventions

- `rustfmt` defaults. Don't hand-format.
- Types `UpperCamelCase`, functions/vars `snake_case`, constants `SCREAMING_SNAKE`.
- Doc comments on public items where the name isn't self-evident.
- `unwrap` / `expect` are fine during early exploration; tighten up when a module stabilizes.
- Tests next to the code they test (`#[cfg(test)] mod tests`).
- `thiserror` when it actually helps; `anyhow` at the top level of examples and the app.
- `log` crate for diagnostics. `println!` is fine in examples.

## Working with an AI assistant

### Session startup (AI reads these in order)

1. `AGENTS.md` (this file) — conventions, rules, where things go
2. `DESIGN.md` — architecture, principles, current Phase 2 status
3. `PHASE2.md` — the current milestone's goals, scope, and acceptance criteria
4. `DECISIONS.md` — settled decisions; check before proposing anything architectural

If the task touches a specific crate, also read that crate's `lib.rs` and the relevant source files before proposing changes. Don't propose changes to code you haven't read.

### Session types

**Feature session** (implementing a milestone):
- Ask for a plan first: files to touch, API shape, what gets tested. Review it before implementation starts.
- Any new dependency must cite which D-015 rule applies. No new runtime dep without a `DECISIONS.md` entry.
- After implementation: `cargo fmt && cargo test --workspace`. That's the bar.

**Audit session** (reviewing code quality, debt, or API ergonomics):
- Read the full crate surface before proposing changes.
- Flag, don't fix — produce findings; fix in a separate session.
- Check `DECISIONS.md` before calling anything architectural "wrong." Most decisions have a logged reason.

**Docs session** (updating planning documents):
- Read the document being changed in full before editing.
- `DECISIONS.md` is append-only — never edit an existing entry. Add a new one that supersedes it.
- Update `CHANGELOG.md` when a milestone ships.
- Update `PHASE2.md` milestone status markers when acceptance criteria are met.

### Open decisions

Decisions still pending a `DECISIONS.md` entry are marked with `<!-- OPEN: ... -->` comments in `PHASE2.md`. These must be resolved before the relevant milestone ships.

### Principles checklist (before implementing anything)

- [ ] No external ECS or game-engine crate
- [ ] No async runtime
- [ ] No global mutable state
- [ ] Any new dependency satisfies at least one D-015 rule
- [ ] New asset references go through the registry by ID (never hardcoded paths)
- [ ] Scope stays within the current task — open a new task if it grows

## When stuck

1. Re-read the milestone in `PHASE2.md`. Half of stuck is having drifted from the goal.
2. Check `DECISIONS.md` for prior art.
3. If the problem is "this is no longer fun," that's a valid signal — see Kill criteria in `DESIGN.md`.
4. Write the question down in a comment (`// TODO: ask about X`) and move on.

## What I'm not doing

- No CI pipeline. Local builds are the bar.
- No `LEARNINGS.md`. Interesting things go in commit messages or `DECISIONS.md`.
- No per-crate `AGENTS.md` until a crate actually needs one.
- No mandatory self-review checklist.
- No forced PR process — solo repo, commit to `main` or branch when convenient.
- No asset preprocessing pipeline. Files on disk are loaded directly.

If any of these become useful later, add them then.

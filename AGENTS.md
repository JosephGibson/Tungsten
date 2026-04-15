# AGENTS.md

Operational notes for working on Tungsten. Canonical rulebook for any AI assistant. Read `DESIGN.md` for architectural context and `DECISIONS.md` for why a thing is the way it is.

## What Tungsten is

A from-scratch Rust 2D game engine, native only. `winit` + `wgpu` + `glam` + hand-rolled ECS + manifest-driven assets. Three crates in a Cargo workspace: `tungsten-core`, `tungsten-render`, `tungsten`. Phase 3 Milestone 12 is complete: the engine now has CPU/GPU telemetry, benchmark harnesses, and baseline profiling tooling in addition to the Phase 2 runtime subsystems.

## Commands

From the workspace root:

```bash
cargo build --workspace
cargo test --workspace                    # unit tests, no GPU/display required
cargo clippy --workspace --all-targets    # advisory only
cargo fmt --all

cargo run -p example-NN-name              # see examples/ for the current list
./scripts/perf-capture.sh sprite-stress 300   # Linux perf capture workflow
```

Before committing anything substantial: `cargo fmt && cargo test --workspace`. Clippy is advisory.

Examples need a real GPU and display. Override the backend if wgpu picks the wrong one: `WGPU_BACKEND=vulkan` (Linux) / `metal` (macOS) / `dx12` (Windows).

For the profiling workflow and capture rules, see [`docs/perf/profiling-workflow.md`](docs/perf/profiling-workflow.md).

## Test layers

Two layers of automated checks exist beyond `cargo test`. Use them deliberately — they exist because earlier bugs (e.g. a manifest path resolving outside its intended target) slipped through unit tests.

- **Layer 1 — manifest integration test.** [crates/tungsten-core/tests/manifests.rs](crates/tungsten-core/tests/manifests.rs) discovers every `manifest.json` in the workspace (root + `examples/*/assets/`) and calls `ResolvedManifest::load` on each. Runs as part of `cargo test --workspace`, no GPU needed. Fast and free.
- **Layer 2 — example smoke test.** [crates/tungsten/src/app.rs](crates/tungsten/src/app.rs) honours `TUNGSTEN_SMOKE_FRAMES`: when set, `App` renders that many frames and exits cleanly. [scripts/smoke-examples.sh](scripts/smoke-examples.sh) runs every example with `TUNGSTEN_SMOKE_FRAMES=3` under a per-example timeout, logs to a temp directory, and reports pass/fail with the tail of any failing log. Needs a real GPU/display. ~1–2 min with a warm cache. **Linux only** — the script uses bash arrays and GNU `timeout`; Windows contributors should run examples manually with `TUNGSTEN_SMOKE_FRAMES=3`.

**When to run which:**

| Change touches…                                       | Run                                          |
| ----------------------------------------------------- | -------------------------------------------- |
| Manifests, assets, or the core/render seam            | `cargo test --workspace` (layer 1)           |
| Engine wiring or example wiring                       | `./scripts/smoke-examples.sh` (layer 2)      |
| Clean checkout, dep bump, or anything non-trivial     | Both                                         |

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

- ECS mechanism (World, storage, queries) → `tungsten-core`
- Rendering primitive (pipeline, texture, buffer, sampler) → `tungsten-render`
- App/event-loop glue, input, time → `tungsten`
- Asset registry types, manifest schema, ID lookups → `tungsten-core`
- GPU upload of decoded assets → `tungsten-render`
- Demo-specific components/systems → `examples/`, never library crates
- Math helpers → `tungsten-core` only when used in two or more places

**The core/render seam.** `TextureHandle(u32)` is defined in `tungsten-core`; no `wgpu` types appear there. The `tungsten` umbrella crate mediates: `AssetRegistry::register_sprite` allocates a handle and stores metadata in core, then `renderer.upload_texture(handle, rgba, ...)` stores the GPU texture in render's pool under the same key. Core never calls into render. `tungsten-render` may depend on `tungsten-core` types (see `DECISIONS.md` D-007).

**Render path vs draw time (D-018).** Extract runs on the main thread with `&World`, resolves string asset IDs to `TextureHandle` where practical, and passes POD slices into render. The renderer does not need mutable `World` access at draw time; it may still read the asset registry for ID resolution when the implementation requires it — see `DECISIONS.md` D-018.

## Asset rules

Anything in `assets/` must be registered in `assets/manifest.json`, and every manifest entry must point to a real file. The loader validates at startup; keep the convention tight by hand.

**Exception:** font family directories (`assets/fonts/<Family>/`) may contain the full downloaded family; only weights in active use need manifest entries. Unused weights are never loaded.

Adding a new asset:

| Type      | Location              | Manifest section | Required fields                           |
| --------- | --------------------- | ---------------- | ----------------------------------------- |
| Sprite    | `assets/sprites/`     | `sprites`        | stable ID, filter (`nearest` \| `linear`) |
| Animation | `assets/animations/`  | `animations`     | stable ID; referenced sprite IDs must exist |
| Font      | `assets/fonts/<Fam>/` | `fonts`          | stable ID                                 |
| Sound     | `assets/sounds/`      | `sounds`         | stable ID, optional `looping` / `volume`  |

- **Shaders** (`*.wgsl`) live in `tungsten-render/src/` and are compiled in via `include_str!` (D-023). Not manifest-tracked and **excluded from hot reload** — shader changes require a binary rebuild.
- **Example-local assets:** `examples/NN_name/assets/` with a local `manifest.json`. Asset IDs must be globally unique across all loaded manifests — duplicate IDs are fatal at load time.
- **Game code never references file paths.** Always reference assets by ID through the registry. This invariant is what makes hot reload (M9) work — don't break it.

## Things to actually not do

- **No external ECS or game-engine crate** (`bevy_ecs`, `hecs`, `specs`, `legion`, `amethyst`, `fyrox`, `ggez`, `macroquad`). These are implemented in-project by design (D-005).
- **No async runtimes** (`tokio`, `async-std`). The `cpal` audio callback thread (M8+) and the `notify` watcher thread (M9+) are the only permitted background threads. The audio thread receives commands via a lock-free `rtrb` ring (D-034); the notify watcher sends file events via `std::sync::mpsc`. No async runtime.
- **No global mutable state.** No `static mut`, no `lazy_static` singletons. State lives in the `World` or is passed explicitly. The asset registry is a `Resource`, not a global.
- **No new third-party runtime dep without a `DECISIONS.md` entry** citing which D-015 rule applies.
- **No hardcoded asset paths in game code.** Use IDs through the registry.
- **No scope-expanding a task mid-flight.** Finish what's scoped; open a new task for the rest.

## Conventions

- `rustfmt` defaults. Don't hand-format.
- `UpperCamelCase` types, `snake_case` functions/vars, `SCREAMING_SNAKE` constants.
- Doc comments on public items where the name isn't self-evident.
- `unwrap`/`expect` fine during early exploration; tighten when a module stabilizes.
- Tests next to the code: `#[cfg(test)] mod tests`.
- Errors: `thiserror` at library boundaries, `anyhow` at the top level of examples and the app.
- Logging: `log` crate; `println!` fine in examples.

## Working with an AI assistant

**Startup reading order:** `AGENTS.md` (this file) → `docs/LLM_INDEX.md` → only the source files this task touches. Read `DESIGN.md` for architecture context and `DECISIONS.md` (grep by `D-0xx`) for rationale — but only when the task requires it. Don't read these end-to-end by default. Don't propose changes to code you haven't read.

**Never read `docs/plans/archive/`.** That directory contains completed or abandoned plans — historical records with no operational value. Skip it entirely during any search or glob.

**Subsystem → file map:** [docs/LLM_INDEX.md](docs/LLM_INDEX.md) (optional shortcut before diving into a crate).

**Plan files (optional handoff).** For work that spans sessions or long chats, write the execution plan to [`docs/plans/<topic>.md`](docs/plans/) and continue from that file in a fresh context instead of replaying the whole thread. Conventions: [CLAUDE.md](CLAUDE.md). Architecture decisions live in `DECISIONS.md`.

**Session types.**

- **Feature session** (implementing a milestone): ask for a plan first — files, API shape, tests. Any new dep cites its D-015 rule and gets a `DECISIONS.md` entry. After implementation: `cargo fmt && cargo test --workspace`.
- **Audit session** (reviewing quality/debt/ergonomics): read the full crate surface before proposing changes. Flag, don't fix — findings in one session, fixes in another. Check `DECISIONS.md` before calling anything "wrong"; most architectural choices have a logged reason.
- **Docs session** (planning documents): read the full doc before editing. `DECISIONS.md` entries are immutable once settled — reversals add a new entry marked `Superseded by D-XXX`. Update `CHANGELOG.md` and `README.md` status when a milestone ships.

**Pre-implementation checklist.**

- [ ] No external ECS or game-engine crate
- [ ] No async runtime
- [ ] No global mutable state
- [ ] Any new dependency satisfies at least one D-015 rule
- [ ] Asset references go through the registry by ID, never hardcoded paths
- [ ] Scope stays within the current task
- [ ] Test layers run per the table above — layer 1 for manifest/asset/seam changes, layer 2 for engine or example wiring, both on clean checkouts or dep bumps

## When stuck

1. Re-read the task scope. Half of stuck is having drifted from the goal.
2. Check `DECISIONS.md` for prior art.
3. Write the question in a `// TODO: ask about X` comment and move on.

## What this project is not doing

No CI pipeline (local builds are the bar). No `LEARNINGS.md` (interesting things go in commit messages or `DECISIONS.md`). No per-crate `AGENTS.md` until a crate needs one. No mandatory self-review checklist. No forced PR process — solo repo. No asset preprocessing pipeline. If any of these become useful later, add them then.

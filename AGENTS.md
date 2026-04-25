# AGENTS.md

Canonical operating rules for Tungsten. Read this first. Use `DESIGN.md` for architecture context and `DECISIONS.md` for rationale.

## What Tungsten Is

From-scratch Rust 2D game engine. Stack: `winit` + `wgpu` + `glam` + hand-rolled ECS + manifest-driven assets. Workspace crates: `tungsten-core`, `tungsten-render`, `tungsten`. Native only. Current repo state: workspace version `0.25.0`, branch `0.25`, Phase 3 complete with all milestones `M12`–`M24` shipped; the rollout plan is archived at [`docs/plans/archive/phase3.md`](docs/plans/archive/phase3.md). Phase 4 is underway with M25, M26, and M27 shipped; remaining scope is tracked in [`docs/plans/phase4.md`](docs/plans/phase4.md).

## Commands

Run from the workspace root:

```bash
cargo build --workspace
cargo test --workspace                    # unit tests, no GPU/display required
cargo clippy --workspace --all-targets    # advisory only
cargo fmt --all

cargo run -p example-NN-name              # see examples/ for the current list
./scripts/perf-capture.sh ecs-high-load 300   # Linux perf capture workflow (default scene)
bash scripts/test-perf-capture.sh         # perf-capture parser/percentile regression check
```

Before committing anything substantial, run `cargo fmt && cargo test --workspace`. `clippy` is advisory. Examples need a real GPU and display. If `wgpu` picks the wrong backend, override it with `WGPU_BACKEND=vulkan` on Linux, `metal` on macOS, or `dx12` on Windows. Profiling workflow and capture rules: [`docs/perf/profiling-workflow.md`](docs/perf/profiling-workflow.md).

## Test Layers

Two automated layers exist beyond `cargo test`.

- **Layer 1 — manifest integration test:** [crates/tungsten-core/tests/manifests.rs](crates/tungsten-core/tests/manifests.rs) discovers every `manifest.json` in the workspace (`root + examples/*/assets/`) and calls `ResolvedManifest::load` on each. It runs as part of `cargo test --workspace`, needs no GPU, and is fast and cheap.
- **Layer 2 — example smoke test:** [crates/tungsten/src/app.rs](crates/tungsten/src/app.rs) honors `TUNGSTEN_SMOKE_FRAMES`; when set, `App` renders that many frames and exits cleanly. [scripts/smoke-examples.sh](scripts/smoke-examples.sh) runs every example with `TUNGSTEN_SMOKE_FRAMES=3` under a per-example timeout, logs to a temp directory, and reports pass/fail with the tail of any failing log. It needs a real GPU/display, takes ~1–2 minutes with a warm cache, and is Linux-only because the script uses bash arrays and GNU `timeout`. Windows contributors should run examples manually with `TUNGSTEN_SMOKE_FRAMES=3`.

When to run which:

| Change touches… | Run |
| --- | --- |
| Manifests, assets, or the core/render seam | `cargo test --workspace` (layer 1) |
| Engine wiring or example wiring | `./scripts/smoke-examples.sh` (layer 2) |
| Perf-capture parsing/reporting | `bash scripts/test-perf-capture.sh` |
| Clean checkout, dep bump, or anything non-trivial | Both |

## Repo Layout

```text
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

### Where New Code Goes

- ECS mechanism (`World`, storage, queries) → `tungsten-core`
- Rendering primitive (pipeline, texture, buffer, sampler) → `tungsten-render`
- App/event-loop glue, input, time → `tungsten`
- Asset registry types, manifest schema, ID lookups → `tungsten-core`
- GPU upload of decoded assets → `tungsten-render`
- Demo-specific components/systems → `examples/`, never library crates
- Math helpers → `tungsten-core` only when used in two or more places

Core/render seam: `TextureHandle(u32)` is defined in `tungsten-core`; no `wgpu` types appear there. `tungsten` mediates the bridge: `AssetRegistry::register_sprite` allocates a handle and stores metadata in core, then `renderer.upload_texture(handle, rgba, ...)` stores the GPU texture in render under the same key. Core never calls into render. `tungsten-render` may depend on `tungsten-core` types; see `DECISIONS.md` `D-007`.

Render path vs draw time (`D-018`): extract runs on the main thread with `&World`, resolves string asset IDs to `TextureHandle` where practical, and passes POD slices into render. The renderer does not need mutable `World` access at draw time, though it may still read the asset registry for ID resolution when the implementation requires it.

## Asset Rules

Everything in `assets/` must be registered in `assets/manifest.json`, every manifest entry must point to a real file, and the loader validates this at startup. Keep the convention tight by hand. Exception: font family directories under `assets/fonts/<Family>/` may contain the full downloaded family; only weights in active use need manifest entries, and unused weights are never loaded.

Adding a new asset:

| Type | Location | Manifest section | Required fields |
| --- | --- | --- | --- |
| Sprite | `assets/sprites/` | `sprites` | stable ID, filter (`nearest` \| `linear`) |
| Animation | `assets/animations/` | `animations` | stable ID; referenced sprite IDs must exist |
| Font | `assets/fonts/<Fam>/` | `fonts` | stable ID |
| Sound | `assets/sounds/` | `sounds` | stable ID, optional `looping` / `volume` |

Additional rules:

- **Shaders** (`*.wgsl`) live in `assets/shaders/` and register in the manifest under a `shaders` section (`D-057`). The engine-internal sprite shader is also `include_str!`d at the same path so the compile-time default and the manifest-tracked runtime source come from one file; the renderer byte-equal short-circuits the load call when they match. Body edits hot-reload through the existing umbrella watcher with `wgpu::naga` validation; signature / bind-group layout changes still require a rebuild (narrowing, not reversing, `D-023`).
- **Materials** (M26, `D-058`) register in the manifest under a `materials` section: `{ "shader": "<shader id>", "uniform_defaults": { … } }`. Each material allocates a stable `MaterialAssetId` and a per-material 256-byte UBO matching `UniformOverrideBlock`; render-side `MaterialPipeline` reuses the built-in sprite pipeline layout (groups 0/1) and adds group 2 for the UBO. Materials are manifest-only in M26 — there is no per-material JSON file, so `uniform_defaults` edits land through the normal manifest reload path. The 17 stock post-processing shaders live under `crates/tungsten-render/src/shaders/stock/`; mirrors at `assets/shaders/stock/` are kept byte-equal with the compile-time `include_str!` sources so manifest hot reload works. LYGIA-derived helpers under `…/stock/lygia/` ship with MIT attribution headers.
- **SMAA / post-AA** (M27, `D-059`) follows the stock-shader pattern for its three stage shaders (`smaa_edge`, `smaa_blend_weights`, `smaa_neighborhood_blend`) — they register in the manifest, body edits hot-reload through `Renderer::upload_shader` / `reload_shader` with `naga` validation, and signature changes still need a rebuild. The `area` / `search` lookup textures ship as `include_bytes!` engine content under `crates/tungsten-render/src/assets/smaa/` with MIT attribution and are intentionally **not** manifest-tracked. Frame order: `Scene → PostStack → [optional SMAA tail → PresentSource] → Text Overlay → Present Blit → Swapchain`; the text overlay always runs after presentation AA so screen-space text is never sampled by SMAA. `render.post_aa` lives in `tungsten.json` (and `TUNGSTEN_RENDER_POST_AA`); runtime changes go through `tungsten::request_post_aa` (a frame-boundary apply, like display state). Switching `post_aa` does not require a relaunch; switching `msaa` still does.
- **Bloom** (M28, `D-060`) ships as the 18th `PostPass` variant, `Bloom(BloomParams { threshold, knee, intensity, radius })`, on the same reorderable `PostStack`. Its four stage shaders (`bloom_threshold`, `bloom_downsample`, `bloom_upsample`, `bloom_composite`) follow the stock-shader pattern: manifest-tracked, body-edit hot-reload through `Renderer::reload_shader` → `BloomPipeline::rebuild_stage_with_module`. Unlike the 17 single-pass stock effects, the bloom slot bypasses `PassRecorder::begin` and records its own threshold + N-1 13-tap Karis-weighted downsamples + N-1 9-tap tent additive upsamples + replace-blend composite into the slot's `dst` (PostPing/PostPong) through the encoder. The pyramid is `Rgba16Float`, lives on `SceneTarget`, and is sized by `bloom_mip_count_for_size(width, height, render.bloom_max_mips)` (default `6`, range `1..=8`, env `TUNGSTEN_RENDER_BLOOM_MAX_MIPS`). `SceneColor` stays sRGB — only the pyramid is HDR. `bloom_max_mips` is startup-only like `msaa`. Frame order is unchanged: bloom is a `PostPass`, not a fixed tail.
- **Example-local assets** live in `examples/NN_name/assets/` with a local `manifest.json`; asset IDs must be globally unique across all loaded manifests, and duplicate IDs are fatal at load time.
- **Game code never references file paths;** always use asset IDs through the registry. That invariant is what makes hot reload (`M9`) work.

## Things To Actually Not Do

- No external ECS or game-engine crate: `bevy_ecs`, `hecs`, `specs`, `legion`, `amethyst`, `fyrox`, `ggez`, `macroquad`. These are implemented in-project by design (`D-005`).
- No async runtimes: `tokio`, `async-std`. The only permitted background threads are the `cpal` audio callback thread (`M8+`) and the `notify` watcher thread (`M9+`). The audio thread receives commands through a lock-free `rtrb` ring (`D-034`); the watcher sends file events through `std::sync::mpsc`.
- No global mutable state: no `static mut`, no `lazy_static` singletons. State lives in the `World` or is passed explicitly. The asset registry is a `Resource`, not a global.
- No new third-party runtime dependency without a `DECISIONS.md` entry citing which `D-015` rule applies.
- No hardcoded asset paths in game code.
- No scope-expanding a task mid-flight; finish the scoped task and open a new one for the rest.

## Conventions

- Use `rustfmt` defaults. Do not hand-format.
- Naming: `UpperCamelCase` types, `snake_case` functions/variables, `SCREAMING_SNAKE` constants.
- Add doc comments on public items when the name is not self-evident.
- `unwrap` / `expect` are acceptable during early exploration; tighten them when the module stabilizes.
- Keep tests next to the code: `#[cfg(test)] mod tests`.
- Errors: `thiserror` at library boundaries, `anyhow` at the top level of examples and the app.
- Logging: `log` crate; `println!` is acceptable in examples.

## Working With an AI Assistant

Startup reading order: `AGENTS.md` → `docs/LLM_INDEX.md` → only the source files touched by the task. Read `DESIGN.md` only when the task needs architecture context; read `DECISIONS.md` only when the task needs rationale. When using `DECISIONS.md`, grep `D-0xx`; do not read it end-to-end by default. Do not propose changes to code you have not read.

Hard rule: never read `docs/plans/archive/`. That directory contains completed or abandoned plans, has no operational value, and should be skipped in all searches and globs.

Shortcuts: subsystem → file map: [docs/LLM_INDEX.md](docs/LLM_INDEX.md). Optional plan handoff path: [`docs/plans/<descriptive-topic>.md`](docs/plans/). Milestone implementation plans use `docs/plans/phaseN-milestone-NN-short-topic.md` (`N` = phase number, `NN` = zero-padded milestone number, `short-topic` = concise kebab-case slug). Plan conventions: [CLAUDE.md](CLAUDE.md). Architecture decisions live in `DECISIONS.md`.

Session types:

- **Feature session:** implementing a milestone. Ask for a plan first: files, API shape, tests. Any new dependency must cite its `D-015` rule and get a `DECISIONS.md` entry. After implementation: `cargo fmt && cargo test --workspace`.
  Milestone plan filenames should use `phaseN-milestone-NN-short-topic.md`; when the work ships, archive the file under `docs/plans/archive/` with the same basename.
- **Audit session:** reviewing quality, debt, or ergonomics. Read the full crate surface before proposing changes. Flag issues; do not fix them in the same session. Use one session for findings and another for fixes. Check `DECISIONS.md` before calling anything “wrong”; many architectural choices are intentional.
- **Docs session:** planning/documentation work. Read the full doc before editing. `DECISIONS.md` entries are immutable once settled; reversals add a new entry marked `Superseded by D-XXX`. Update `CHANGELOG.md` and `README.md` status when a milestone ships.

Pre-implementation checklist:

- [ ] No external ECS or game-engine crate
- [ ] No async runtime
- [ ] No global mutable state
- [ ] Any new dependency satisfies at least one `D-015` rule
- [ ] Asset references go through the registry by ID, never through hardcoded paths
- [ ] Scope stays within the current task
- [ ] Test layers run per the table above: layer 1 for manifest/asset/seam changes, layer 2 for engine/example wiring, both on clean checkouts or dependency bumps

## When Stuck

1. Re-read the task scope. Half of “stuck” is scope drift.
2. Check `DECISIONS.md` for prior art.
3. Write the question in a `// TODO: ask about X` comment and move on.

## What This Project Is Not Doing

- No `CI` pipeline; local builds are the bar
- No `LEARNINGS.md`; interesting items go in commit messages or `DECISIONS.md`
- No per-crate `AGENTS.md` until a crate actually needs one
- No mandatory self-review checklist
- No forced PR process; this is a solo repo
- No asset preprocessing pipeline
- If any of these become useful later, add them later

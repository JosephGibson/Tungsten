# AGENTS.md

Canonical rules for Tungsten, shared by every coding agent (`CLAUDE.md` only imports this file). **Before editing under `crates/tungsten-render/`, read `crates/tungsten-render/AGENTS.md`.**

## Project

Native Rust 2D game engine: `winit` + `wgpu` + `glam` + hand-rolled ECS + manifest-driven assets. Crates: `tungsten-core` (ECS, config, physics, asset registry), `tungsten-render` (wgpu), `tungsten` (app loop, loaders, audio, hot reload). Demos: `examples/NN_name/`.

## Reading

- Start here, then open only the files the task touches. [`docs/LLM_INDEX.md`](docs/LLM_INDEX.md) maps tasks to files; open it before a broad search.
- Rationale: [`docs/DECISION_INDEX.md`](docs/DECISION_INDEX.md), then `rg -n 'D-0NN' DECISIONS.md`. Never read `DECISIONS.md`, `DESIGN.md` or `CHANGELOG.md` whole: `rg -n '^#' <file>`, then read one section.
- **Never read, search or glob `docs/plans/archive/`** (completed or abandoned plans). `.ignore` filters it; never bypass that with `rg -uu`.
- Plan files follow [`docs/plans/README.md`](docs/plans/README.md); client setup (skills, filters, permissions) is in [`docs/agent-setup.md`](docs/agent-setup.md).

## Commands

From the repo root; examples need a GPU and display. `just` recipes wrap the raw commands (`just --list`).

```bash
just check                  # fmt-check, clippy -D warnings, tests
just smoke                  # layer 2 (GPU): scripts/smoke-examples.sh
just visual                 # pixel test, reference machine only
just perf ecs-high-load 300 # see docs/perf/profiling-workflow.md
just deps                   # cargo deny: advisories, licenses, sources
just ctx                    # instruction budgets and links
cargo run -p example-NN-name
```

Finish substantial work with `just check` (format check, strict clippy, workspace tests). Wrong backend: `WGPU_BACKEND=vulkan|metal|dx12`.

## Tests

- **Layer 1:** `crates/tungsten-core/tests/manifests.rs` loads every `manifest.json`; part of `cargo test`.
- **Layer 2:** smoke runs each example for `TUNGSTEN_SMOKE_FRAMES=3` plus render fixture matrices. GPU, Linux only; elsewhere run examples with that variable.
- **Visual:** `TUNGSTEN_VISUAL_REGRESSION=1 cargo test -p example-02-sprite-stress --test visual_regression`; without the variable it skips the pixel comparison.

| Change touches | Run |
| --- | --- |
| Manifests, assets, core/render seam | layer 1 |
| Engine or example wiring | layer 2 |
| Scripts or perf-capture parsing | `just script-test` |
| Dependency bump, clean checkout, anything non-trivial | both |

## Where code goes

- ECS, config, asset registry/manifest schema/IDs, math used twice+ → `tungsten-core`
- Rendering primitives, GPU upload of decoded assets → `tungsten-render`
- App/event loop, input, time, load bridge → `tungsten`
- Demo-specific components/systems → `examples/`, never library crates

Seam (`D-007`, `D-016`, `D-018`): core defines `TextureHandle(u32)`, has no `wgpu` types and never calls render. Render allocates handles (`D-048`); the umbrella registers core metadata and uploads pixels under the same handle. Extract runs on the main thread with `&World` and passes render data; render needs no mutable `World`.

## Assets

`just repo-check` checks asset-file coverage; loaders validate referenced content. Whole font families, shader helper fragments/licenses and explicitly loaded scenes (`D-046`) are exceptions; see `docs/agent-setup.md`.

| Type | Location | Section | Required |
| --- | --- | --- | --- |
| Sprite | `assets/sprites/` | `sprites` | ID, filter `nearest`/`linear`; optional `normal_map`, `emissive_mask` |
| Animation | `assets/animations/` | `animations` | ID; sprite IDs must exist |
| Font | `assets/fonts/<Fam>/` | `fonts` | ID |
| Sound | `assets/sounds/` | `sounds` | ID; optional `looping`, `volume` |
| Shader | `assets/shaders/` | `shaders` | ID (`D-057`) |
| Material | manifest only | `materials` | `shader` ID, `uniform_defaults` (`D-058`) |

- Example-local assets: `examples/NN_name/assets/` with its own `manifest.json`. IDs are unique across loaded manifests; duplicates are fatal.
- Game code uses registry IDs; explicit scene loading follows `D-046`.

## Hard rules

- No external ECS or engine crate (`bevy_ecs`, `hecs`, `specs`, `legion`, `amethyst`, `fyrox`, `ggez`, `macroquad`; `D-005`).
- No async runtime (`tokio`, `async-std`). Only two background threads: the `cpal` callback (commands via `rtrb`, `D-034`) and the `notify` watcher (events via `std::sync::mpsc`).
- No global mutable state (`static mut`, `lazy_static`); state lives in `World` or is passed explicitly.
- No new third-party runtime dependency without a `DECISIONS.md` entry citing its `D-015` rule.
- No hardcoded asset paths in game code except explicit scene loading (`D-046`).
- No scope creep: finish the task, open a new one for the rest.

## Conventions

- `rustfmt` defaults. `UpperCamelCase` types, `snake_case` functions, `SCREAMING_SNAKE` constants.
- Doc comments where a public name isn't self-evident; unit tests in `#[cfg(test)] mod tests`.
- `thiserror` at library boundaries, `anyhow` at app/example top level; `log` for logging (`println!` in examples only).
- `unwrap`/`expect` are fine while exploring; tighten once a module stabilizes.

## Sessions

- **Feature:** plan first (files, API shape, tests).
- **Audit:** read the full crate surface, report findings only unless the user requests fixes. Check decisions before calling a choice wrong.
- **Docs:** read the whole doc before editing. Decisions are immutable: a reversal adds an entry and marks the old one `Superseded by D-NNN`. New decisions add their `docs/DECISION_INDEX.md` row in the same change (test-enforced). Update `CHANGELOG.md`/`README.md` when a milestone ships.
- Don't change code you haven't read. Stuck: re-read scope, check decisions, or leave `// TODO: ask about X`.

## Not doing

CI is CPU-only and informational (`D-070`); GPU, audio and perf checks stay local. No `LEARNINGS.md`, no mandatory review or PR process (solo repo), no asset preprocessing. Add a scoped `AGENTS.md` only where a directory has unique rules.

# AGENTS.md

Shared Tungsten rules (`CLAUDE.md` imports this file). **Before editing under `crates/tungsten-render/`, read `crates/tungsten-render/AGENTS.md`.**

## Project

Native Rust 2D game engine: `winit` + `wgpu` + `glam` + hand-rolled ECS + manifest-driven assets. Crates: `tungsten-core` (ECS, config, physics, asset registry), `tungsten-render` (wgpu), `tungsten` (app loop, loaders, audio, hot reload). Demos: `examples/NN_name/`.

## Reading

- Start here, then open only the files the task touches. [`docs/LLM_INDEX.md`](docs/LLM_INDEX.md) maps tasks to files; open it before a broad search.
- Rationale: matching rows in [`docs/DECISION_INDEX.md`](docs/DECISION_INDEX.md), then `rg -n 'D-0NN' DECISIONS.md`. Never read `DECISIONS.md`, `DESIGN.md` or `CHANGELOG.md` whole: `rg -n '^#' <file>`, then read one section.
- **Never read, search or glob `docs/plans/archive/`** (completed or abandoned plans). `.ignore` filters it; never bypass that with `rg -uu`.
- [Doc map](docs/README.md): canonical sources. Plans follow [`docs/plans/README.md`](docs/plans/README.md); client setup is in [`docs/agent-setup.md`](docs/agent-setup.md).

## Commands

Run from the repo root. `just --list` lists wrappers for the raw commands; [validation](docs/validation.md) selects them. Examples use `cargo run -p example-NN-name` and need a GPU/display. Wrong backend: `WGPU_BACKEND=vulkan|metal|dx12`.

## Tests

[Validation](docs/validation.md) owns the detailed change-to-check matrix and targeted edit-loop commands. `just checks [PATH…]` prints recommendations; with no paths it reads Git changes including untracked files (`--base REF` includes committed work). It runs no checks. Finish substantial work with `just check`; milestone and release requirements still apply.

GPU smoke and pixel comparisons stay local. A CPU test run without `TUNGSTEN_VISUAL_REGRESSION=1` skips pixel comparisons; report them as not run.

## Where code goes

- ECS, config, asset registry/manifest schema/IDs, math used twice+ → `tungsten-core`
- Rendering primitives, GPU upload of decoded assets → `tungsten-render`
- App/event loop, input, time, load bridge → `tungsten`
- Demo-specific components/systems → `examples/`, never library crates

Seam (`D-007`, `D-016`, `D-018`): core defines `TextureHandle(u32)`, has no `wgpu` types and never calls render. Render allocates handles (`D-048`); the umbrella registers core metadata and uploads pixels under the same handle. Extract runs on the main thread with `&World` and passes render data; render needs no mutable `World`.

## Assets

Asset files are listed in a `manifest.json` and named by registry ID. [`docs/assets.md`](docs/assets.md) gives each type's folder, section and required fields, example-local manifests and the coverage exceptions; `just repo-check` checks coverage.

## Hard rules

- No external ECS or engine crate (`bevy_ecs`, `hecs`, `specs`, `legion`, `amethyst`, `fyrox`, `ggez`, `macroquad`; `D-005`).
- No async runtime (`tokio`, `async-std`). Only two background threads: the `cpal` callback (commands via `rtrb`, `D-034`) and the `notify` watcher (events via `std::sync::mpsc`).
- No global mutable state (`static mut`, `lazy_static`); state lives in `World` or is passed explicitly.
- No new third-party runtime dependency without a `DECISIONS.md` entry citing its `D-015` rule.
- No hardcoded asset paths in game code except explicit scene loading (`D-046`).
- No scope creep: finish the task, open a new one for the rest. Leave work uncommitted (`D-105`); Git publication stays with the human.

## Conventions

- `rustfmt` defaults and Rust naming conventions.
- Doc comments where a public name isn't self-evident; unit tests in `#[cfg(test)] mod tests`.
- `thiserror` at library boundaries, `anyhow` at app/example top level; `log` for logging (`println!` in examples only).
- `unwrap`/`expect` are fine while exploring; tighten once a module stabilizes.

## Sessions

- **Feature:** plan first (files, API shape, tests). Bounded maintenance records scope, affected files and acceptance checks in the session; multi-session work keeps a saved handoff plan (`D-140`, [routes](docs/plans/README.md#choosing-a-route)). Milestone/API review requirements stand.
- **Audit:** crate correctness audits read the full named crate surface; scoped workflow/documentation audits read the named docs and tools. Report findings only unless fixes are requested. Check decisions before calling a choice wrong.
- **Docs:** read the whole doc before editing. Decisions are immutable: a reversal adds an entry and marks the old one `Superseded by D-NNN`. New decisions add their `docs/DECISION_INDEX.md` row in the same change (test-enforced). Releases follow [docs/releases.md](docs/releases.md); README stays version-free.
- Don't change code you haven't read. Stuck: re-read scope, check decisions, or leave `// TODO: ask about X`.

## Not doing

CI is CPU-only and informational (`D-070`); GPU, audio and perf checks stay local. No `LEARNINGS.md`, no PR process beyond the release PR (`D-079`), no asset preprocessing. Add a scoped `AGENTS.md` only where a directory has unique rules.

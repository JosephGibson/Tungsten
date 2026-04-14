# Tungsten

A from-scratch Rust 2D game engine. `winit` + `wgpu` + `glam` + hand-rolled ECS + manifest-driven assets. Native only (Linux / macOS / Windows) — no WASM.

**Status:** `v0.7.0-alpha` — Phase 2 complete (M7–M12). Next: 0.8.

## Stack

Hand-rolled ECS (archetypal storage), wgpu rendering, manifest-driven assets, text (glyphon), audio (cpal + symphonia + hand-rolled mixer), hot reload (notify), tilemaps (.tmj / Tiled), 2D physics (AABB + circle, uniform-grid broad-phase).

## Documents

| File | Purpose |
|---|---|
| [`DESIGN.md`](DESIGN.md) | Architecture, stack, subsystem detail. **Context.** |
| [`AGENTS.md`](AGENTS.md) | Operational rules for working in the repo. **Tasks.** |
| [`DECISIONS.md`](DECISIONS.md) | Log of non-obvious decisions with rationale (D-NNN). |
| [`CLAUDE.md`](CLAUDE.md) | Pointer file for Claude Code. |
| [`docs/LLM_INDEX.md`](docs/LLM_INDEX.md) | Subsystem → source paths for coding agents. |
| [`CHANGELOG.md`](CHANGELOG.md) | Per-version change log. |

## Quick start

```bash
cargo build --workspace
cargo test --workspace
cargo run -p example-NN-name       # see examples/ for the list
```

## Read order

| Audience | Order |
|---|---|
| Human | `DESIGN.md` → `DECISIONS.md` → `AGENTS.md` |
| AI agent | `AGENTS.md` → `DESIGN.md` → `DECISIONS.md` |

## License

MIT — see [LICENSE](LICENSE).

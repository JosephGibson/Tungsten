# Tungsten

A from-scratch Rust 2D game engine, built as a hobby project. The point is the *building*, not the shipping — understanding how engines actually work from the ground up, with Rust as the language to learn deeply along the way.

**Version:** 0.2.0-alpha

**Status:** Phase 1 complete (M0–M6). Phase 2 started: **M7 text rendering** is complete — manifest `fonts`, glyphon/cosmic-text pipeline in `tungsten-render`, and `example-06-text`. Next: M8 audio (`v0.3.0-alpha`). See `PHASE2.md`.

## Stack

`winit` (windowing) + `wgpu` (rendering) + `glam` (math) + a hand-rolled ECS + `serde_json` for data-driven config and asset manifests. Native targets only — no WASM.

## Documents

| File             | Purpose                                                            |
| ---------------- | ------------------------------------------------------------------ |
| `README.md`      | This file. Orientation.                                            |
| `DESIGN.md`      | Architecture, principles, milestones, kill criteria. **Start here for context.** |
| `AGENTS.md`      | Operational rules for working in the repo. **Start here for tasks.** |
| `DECISIONS.md`   | Append-only log of non-obvious decisions, with rationale.          |
| `PHASE2.md`      | Phase 2 milestones (M7+), release map, acceptance criteria.       |
| `CLAUDE.md`      | Pointer file for Claude Code; the canonical instructions are in `AGENTS.md`. |

## Quick start

```bash
# Build everything
cargo build --workspace

# Run tests
cargo test --workspace

# Run examples
cargo run -p example-01-window      # M0–M1: window + wgpu clear
cargo run -p example-02-ecs         # M2: ECS demo (stdout)
cargo run -p example-03-dots        # M3–M4: bouncing colored quads + input
cargo run -p example-04-sprites     # M5: textured sprites from manifest
cargo run -p example-05-animation   # M6: frame-based animation
cargo run -p example-06-text        # M7: text rendering (Phase 2)
```

## Read order

### For humans
1. `DESIGN.md` — what Tungsten is, why the choices were made, what the milestones are.
2. `DECISIONS.md` — the log of *why* specific things are the way they are.
3. `AGENTS.md` — how to actually work in the repo.

### For AI agents
1. `AGENTS.md` — operational rules, what the project is, what not to do.
2. The relevant section of `DESIGN.md` for whatever is being worked on.
3. Any `DECISIONS.md` entries that touch the area being changed.

## License

MIT — see [LICENSE](LICENSE).

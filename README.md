# Tungsten

A from-scratch Rust 2D game engine, built as a hobby project. The point is the *building*, not the shipping — understanding how engines work from the ground up, with Rust as the language to learn deeply.

**Status:** `v0.6.0-alpha` — Phase 1 complete (M0–M6). Phase 2 through M11 complete (M7 text through M11 2D physics). Next: M12 ECS rewrite (conditional) or M13 first game. See [PHASE2.md](PHASE2.md).

## Stack

`winit` + `wgpu` + `glam` + hand-rolled ECS + `serde_json`-driven config and asset manifests. Native only (Linux / macOS / Windows) — no WASM.

## Documents

| File           | Purpose                                                          |
| -------------- | ---------------------------------------------------------------- |
| `DESIGN.md`    | Architecture, principles, dependency philosophy. **Context.**    |
| `AGENTS.md`    | Operational rules for working in the repo. **Tasks.**            |
| `DECISIONS.md` | Log of non-obvious decisions with rationale.                     |
| `PHASE2.md`    | Phase 2 milestones, release map, acceptance criteria.            |
| `CLAUDE.md`    | Pointer file for Claude Code; canonical rules are `AGENTS.md`.   |
| `docs/LLM_INDEX.md` | Subsystem → source paths for coding agents (optional).    |
| `CHANGELOG.md` | Per-version change log.                                          |

## Quick start

```bash
cargo build --workspace
cargo test --workspace
cargo run -p example-NN-name       # see examples/ for the list
```

## Read order

| Audience  | Order                                                    |
| --------- | -------------------------------------------------------- |
| Human     | `DESIGN.md` → `DECISIONS.md` → `AGENTS.md`               |
| AI agent  | `AGENTS.md` → `DESIGN.md` → `PHASE2.md` → `DECISIONS.md` |

## License

MIT — see [LICENSE](LICENSE).

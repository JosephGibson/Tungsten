# Tungsten

A from-scratch Rust 2D game engine, built as a hobby project. The point is the *building*, not the shipping — understanding how engines actually work from the ground up, with Rust as the language to learn deeply along the way.

**Status:** Documentation phase. No code yet. The repository currently holds design documents only; implementation begins at milestone M0 (see `DESIGN.md`).

## Stack

`winit` (windowing) + `wgpu` (rendering) + `glam` (math) + a hand-rolled ECS + `serde_json` for data-driven config and asset manifests. Native targets only — no WASM.

## Documents

| File             | Purpose                                                            |
| ---------------- | ------------------------------------------------------------------ |
| `README.md`      | This file. Orientation.                                            |
| `DESIGN.md`      | Architecture, principles, milestones, kill criteria. **Start here for context.** |
| `AGENTS.md`      | Operational rules for working in the repo. **Start here for tasks.** |
| `DECISIONS.md`   | Append-only log of non-obvious decisions, with rationale.          |
| `CLAUDE.md`      | Pointer file for Claude Code; the canonical instructions are in `AGENTS.md`. |

## For humans

Read in this order:

1. `DESIGN.md` — what Tungsten is, why the choices were made, what the milestones are.
2. `DECISIONS.md` — the log of *why* specific things are the way they are. Each entry is short; skim section titles, read entries that look relevant.
3. `AGENTS.md` — how to actually work in the repo (commands, conventions, what to avoid).

## For AI agents and coding assistants

This repository is currently documentation-only. There is no code to modify, no tests to run, no build system in place. The task at this stage is helping iterate on the design documents themselves, or scaffolding the initial workspace when M0 begins.

**Read in this order at the start of any session:**

1. `AGENTS.md` — operational rules, what the project is, what not to do. Always read first.
2. The relevant section of `DESIGN.md` for whatever is being worked on.
3. Any `DECISIONS.md` entries that touch the area being changed. Use the entry titles as an index; full read isn't necessary.

**Key things to internalize before suggesting changes:**

- This is a hobby project optimized for "will the human want to come back to it on a Saturday." Process and ceremony are deliberately minimal. Do not propose adding CI gates, mandatory documentation rules, code coverage targets, PR templates, or other "industrial" practices unless explicitly asked — `DECISIONS.md` D-002 captures why these were cut.
- Several architectural rules are deliberately *not* what a typical engine would do. `tungsten-render` is allowed to depend on `tungsten-core` (D-007). The ECS is hand-rolled with no external crate (D-005). Dependency choices follow a specific three-rule philosophy (D-015). Read those entries before pushing back on any of them.
- The "Kill criteria" section in `DESIGN.md` includes "writing process instead of code." Doc revisions that don't fix a concrete gap are themselves a flag. Prefer answering "is the doc actually wrong" before answering "could the doc be more thorough."
- The `DECISIONS.md` file is append-only. Never edit or delete entries. Supersede with a new entry if a decision is reversed.

**Useful prompt patterns when working on this repo:**

- "Read AGENTS.md and DESIGN.md, then propose [thing]" — gives the agent the orientation it needs.
- "Critique this diff against the principles in DESIGN.md" — the principles list is a good lens for self-review.
- "Is this gap actually wrong, or am I just adding process?" — asks for the kill-criteria check explicitly.

## License

Not yet specified. Will be added before any code is published.

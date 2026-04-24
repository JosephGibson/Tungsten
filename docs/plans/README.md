# Session Plans

Use this directory for short-lived multi-step plans saved as `*.md`. Purpose: give a fresh assistant context a handoff artifact instead of long chat history.

## Naming

- Milestone implementation plans use `phaseN-milestone-NN-short-topic.md`.
- `N` is the phase number.
- `NN` is the zero-padded milestone number.
- `short-topic` is a concise kebab-case feature slug that describes the milestone deliverable.
- Example: `phase4-milestone-26-materials-post-stack.md`.
- Non-milestone handoff plans use `descriptive-topic.md`.

## Archival

- Active plans live in `docs/plans/`.
- Completed or abandoned plans move to `docs/plans/archive/` and keep the same basename.

Conventions: see [CLAUDE.md](../../CLAUDE.md).

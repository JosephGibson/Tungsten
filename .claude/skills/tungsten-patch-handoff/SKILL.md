---
name: tungsten-patch-handoff
description: Hand work to the human as a verified per-step patch series plus a commit script. Opt-in: only when a task explicitly asks for per-step patches to review; plan work is otherwise committed locally (D-097). Wraps scripts/patch-series.py; not for the release hand-off.
---

# tungsten-patch-handoff

Commands, layout and limits: `python3 -B scripts/patch-series.py --help` ([script](../../../scripts/patch-series.py)). Opt-in: use it only when the task explicitly asks for per-step patches or commits to review. Plan work is otherwise committed once per plan or phase ([plans README](../../../docs/plans/README.md), `D-097`).

## Order

1. `init --out DIR` at the start, before the first edit; DIR in the scratchpad.
2. `cut` at each step boundary, once that step's checks pass.
3. `verify`, then `script`. Report DIR and `DIR/commit.sh`.

## Commit order

Each commit must pass `just ctx` and `just repo-check` on its own; the tool does not run them. A decision lands before the docs that cite it, and docs drop a file from the index before the commit that deletes it.

## Rules

- Messages carry no Co-Authored-By or "Generated with Claude Code" lines.
- Never run `commit.sh` yourself and never work around the Git deny (plumbing, scratch repositories, other tools). The human runs it.
- `commit.sh` refuses once HEAD moves; run `init` and the cuts again on the new HEAD.

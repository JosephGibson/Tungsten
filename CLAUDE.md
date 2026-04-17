# CLAUDE.md

Pointer file for [Claude Code](https://claude.ai/code).

## Read Path

- Read [AGENTS.md](AGENTS.md) first; it is the canonical source for commands, test layers, hard rules, and code-placement rules.
- Open [docs/LLM_INDEX.md](docs/LLM_INDEX.md) before any broad repo search or glob; it maps each subsystem to its primary source files.
- Agent read path: `AGENTS.md` → `docs/LLM_INDEX.md` → only the files touched by the task.
- Skip `DESIGN.md` and `DECISIONS.md` unless the task needs architecture context or rationale; for `DECISIONS.md`, grep `D-0xx` and do not read it serially.
- Skip `CHANGELOG.md` unless releasing.
- Never read `docs/plans/archive/`; those plans are completed or abandoned and have no operational value.

## Session Plans on Disk

- For multi-step work, write the agreed plan to [`docs/plans/<descriptive-topic>.md`](docs/plans/) instead of leaving it only in chat.
- That file is the handoff artifact for a fresh context; a typical restart prompt is `read docs/plans/… and implement; stay in scope`.
- Required header fields: `status` (`draft` / `in progress` / `done`), goal, non-goals, files to touch, ordered steps, done-when checks.
- Update status when work finishes. Archive or rename obsolete plans so only one active plan drives a given thread of work.
- Settled rationale lives in [DECISIONS.md](DECISIONS.md). `docs/plans/` is for time-bounded execution plans only.

## Status

Workspace `v0.12.0` on branch `0.12`. Phase 3 M15 is complete. Render-component docs, frame-order docs, and perf/benchmark references are up to date.

# CLAUDE.md

Pointer file for [Claude Code](https://claude.ai/code).

**Read [`AGENTS.md`](AGENTS.md) first** — canonical commands, test layers, hard rules, and where new code belongs.

**Index first.** Open [`docs/LLM_INDEX.md`](docs/LLM_INDEX.md) before any broad repo search or glob. It maps every subsystem to its primary source files.

**Agent read path:** `AGENTS.md` → `LLM_INDEX.md` → only the files this task touches. Skip `DESIGN.md` and `DECISIONS.md` unless the task requires architecture context or rationale (grep `D-0xx` in `DECISIONS.md`; never read it serially). Skip `CHANGELOG.md` unless releasing. **Never read `docs/plans/archive/`** — completed/abandoned plans, no operational value.

## Session plans on disk

For **multi-step** work, write the agreed plan to [`docs/plans/<descriptive-topic>.md`](docs/plans/) instead of leaving it only in chat. That file is the **handoff artifact** for a fresh context: start a new conversation with something like "read `docs/plans/…` and implement; stay in scope."

At the top of the file: **status** (`draft` · `in progress` · `done`), goal, non-goals, files to touch, ordered steps, and **done when** checks. Update the status when the work finishes; archive or rename obsolete plans so only one **active** plan drives a given thread of work.

Settled rationale lives in [`DECISIONS.md`](DECISIONS.md). `docs/plans/` is for **time-bounded execution plans** only.

**Status:** `v0.9.0` — Phase 3 M12 complete. Branch: `0.9`.

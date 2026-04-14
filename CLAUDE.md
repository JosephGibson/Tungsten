# CLAUDE.md

Pointer file for [Claude Code](https://claude.ai/code).

**Read [`AGENTS.md`](AGENTS.md) first** — canonical commands, test layers, hard rules, and where new code belongs.

Optional quick navigation for assistants: [`docs/LLM_INDEX.md`](docs/LLM_INDEX.md).

## Session plans on disk

For **multi-step** work, write the agreed plan to [`docs/plans/<descriptive-topic>.md`](docs/plans/) instead of leaving it only in chat. That file is the **handoff artifact** for a fresh context: start a new conversation with something like “read `docs/plans/…` and implement; stay in scope.”

At the top of the file: **status** (`draft` · `in progress` · `done`), goal, non-goals, files to touch, ordered steps, and **done when** checks. Update the status when the work finishes; archive or rename obsolete plans so only one **active** plan drives a given thread of work.

Long-lived direction belongs in [`PHASE2.md`](PHASE2.md) and settled rationale in [`DECISIONS.md`](DECISIONS.md). `docs/plans/` is for **time-bounded execution plans** only.

**Status:** `v0.6.0-alpha` (Phase 2 through M11). Next: M12 ECS rewrite (conditional) or M13 first game — [`PHASE2.md`](PHASE2.md).

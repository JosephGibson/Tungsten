# CLAUDE.md

Pointer file for [Claude Code](https://claude.ai/code).

## Read Path

- Read [AGENTS.md](AGENTS.md) first; it is the canonical source for commands, test layers, hard rules, and code-placement rules.
- Open [docs/LLM_INDEX.md](docs/LLM_INDEX.md) before any broad repo search or glob; it maps each subsystem to its primary source files.
- For rationale lookup, check [docs/DECISION_INDEX.md](docs/DECISION_INDEX.md) before opening [DECISIONS.md](DECISIONS.md).
- Agent read path: `AGENTS.md` → `docs/LLM_INDEX.md` → only the files touched by the task.
- Skip `DESIGN.md` and `DECISIONS.md` unless the task needs architecture context or rationale; for `DECISIONS.md`, grep `D-0xx` and do not read it serially.
- Skip `CHANGELOG.md` unless releasing.
- Never read `docs/plans/archive/`; those plans are completed or abandoned and have no operational value.

## Session Plans on Disk

- For multi-step work, write the agreed plan to [`docs/plans/<descriptive-topic>.md`](docs/plans/) instead of leaving it only in chat.
- Milestone implementation plans use `docs/plans/phaseN-milestone-NN-short-topic.md`.
- Naming rule: `N` is the phase number, `NN` is the zero-padded milestone number, and `short-topic` is a concise kebab-case feature slug. Example: `docs/plans/phase4-milestone-26-materials-post-stack.md`.
- Non-milestone handoff plans keep the descriptive-topic pattern.
- That file is the handoff artifact for a fresh context; a typical restart prompt is `read docs/plans/… and implement; stay in scope`.
- Required header fields: `status` (`draft` / `in progress` / `done`), goal, non-goals, files to touch, ordered steps, done-when checks.
- Update status when work finishes. Archive or rename obsolete plans so only one active plan drives a given thread of work. When archiving a milestone plan, keep the same basename under `docs/plans/archive/`.
- Settled rationale lives in [DECISIONS.md](DECISIONS.md). `docs/plans/` is for time-bounded execution plans only.

## Status

Workspace `v0.25.0` on branch `0.25`. Phase 3 is complete; all milestones `M12`–`M24` shipped and the rollout plan is archived at [`docs/plans/archive/phase3.md`](docs/plans/archive/phase3.md). Phase 4 is underway with M25, M26, M27, and M28 shipped; remaining scope is tracked in [`docs/plans/phase4.md`](docs/plans/phase4.md).

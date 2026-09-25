# Session Plans

Short-lived multi-step plans saved as `*.md`. A plan is the handoff artifact for a fresh assistant context, instead of long chat history; a typical restart prompt is `read docs/plans/… and implement; stay in scope`. For multi-step work, write the agreed plan here rather than leaving it only in chat.

## Naming

- Milestone implementation plans: `phaseN-milestone-NN-short-topic.md`. `N` is the phase number, `NN` the zero-padded milestone number, `short-topic` a concise kebab-case slug for the deliverable. Example: `phase4-milestone-26-materials-post-stack.md`.
- Other handoff plans: `descriptive-topic.md`.

## Contents

- Required header fields: `status` (`draft` / `in progress` / `done`), goal, non-goals, files to touch, ordered steps, done-when checks.
- Put a short context digest (under ~500 tokens) near the top instead of a separate context file.
- Settled rationale belongs in `DECISIONS.md`; plans are time-bounded execution documents.

## Lifecycle

- Update `status` when work finishes; don't leave a finished plan `in progress`.
- Keep one active plan per thread of work: archive or rename obsolete ones.
- Completed or abandoned plans move to `docs/plans/archive/` with the same basename. Agents never read that directory.

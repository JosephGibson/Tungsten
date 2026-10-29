---
name: tungsten-release
description: Run `just release`, push the version branch, end with the owner's tag/push/next-branch/pull-request block, and verify, rehearse or recover Tungsten releases using the canonical release guide and read-only preflight.
---

# tungsten-release

Read [docs/releases.md](../../../docs/releases.md): its numbered happy path owns the checks, commands and commit messages; its reference covers resuming, verification, rehearsals and recovery. Follow the requested stage and existing authorization.

Run `just release minor '<summary>'` yourself: it runs every check and commits the cut. Push the version branch (`git push origin 0.NN`, the whole command), then end the task with the owner's block as the read-only preflight prints it, values filled in: the tag, its push, which publishes, the next branch and the pull request. Tags, every other push and GitHub-changing `gh` commands stay the owner's under the permission model stated there; merging the pull request publishes nothing.

Use [tungsten-finalize](../tungsten-finalize/SKILL.md) if branch documentation is still needed. Follow the guide's handoff requirements and [AGENTS.md](../../../AGENTS.md). Do not maintain a second procedure here.

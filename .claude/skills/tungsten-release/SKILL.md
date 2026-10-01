---
name: tungsten-release
description: Run the release checks, end with the human's commit/tag/push/pull-request command block, and verify, rehearse or recover Tungsten releases using the canonical release guide and read-only preflight.
---

# tungsten-release

Read [docs/releases.md](../../../docs/releases.md): its numbered happy path owns the checks, commands and commit messages; its reference covers resuming, verification, rehearsals and recovery. Follow the requested stage and existing authorization.

Run every check yourself first. Then end the task with the exact commands the human still has to run, values filled in, as one block of basic commit, tag, push and pull-request commands; the read-only preflight prints them. Git mutations and GitHub-changing `gh` commands are human-pasted under the permission model stated there, and the human merging the pull request in GitHub is what publishes.

Use [tungsten-finalize](../tungsten-finalize/SKILL.md) if branch documentation or the version cut is still needed. Follow the guide's handoff requirements and [AGENTS.md](../../../AGENTS.md). Do not maintain a second procedure here.

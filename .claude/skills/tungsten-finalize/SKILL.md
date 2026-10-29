---
name: tungsten-finalize
description: Prepare Tungsten branch documentation for a release, following the canonical release guide before `just release` cuts it.
---

# tungsten-finalize

Read [docs/releases.md — Branch preparation](../../../docs/releases.md#branch-preparation) and follow [AGENTS.md](../../../AGENTS.md). This skill handles the docs pass; `just release minor` makes the cut. The guide owns the checklist, commands, commit convention and resume rules.

Complete the requested preparation and report its checks. Continue through [tungsten-release](../tungsten-release/SKILL.md), which runs `just release` and ends with the owner's block; tags, merges and GitHub-changing `gh` commands stay the owner's under the guide's permission model. Do not maintain a second procedure here.

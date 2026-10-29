---
name: tungsten-release
description: Prepare, cut, rehearse, verify or recover a Tungsten release — branch documentation, the notes, `just release`, the version branch's push and the owner's tag/next-branch/pull-request block — from docs/releases.md. Use for any release or rehearsal request.
---

# tungsten-release

[docs/releases.md](../../../docs/releases.md) owns the procedure: its numbered path, then its reference for branch preparation, resuming, rehearsals and recovery. Follow the requested stage.

- **Yours:** the notes, the sitting before a minor cut, `just release minor '<summary>'` (it runs every check and commits the cut) and `git push origin 0.NN`, the whole command.
- **The owner's:** the tag and its push, which publishes; every other push; merges; every GitHub-changing `gh` (`gh pr create`, `gh workflow run`, reruns). End with the owner's block as preflight prints it. The merge publishes nothing.
- **Rehearse first** when `release.yml`, or `release.py`'s packaging, changed since the last final tag: `just release rehearsal`, then the owner's tag push, before any final tag.

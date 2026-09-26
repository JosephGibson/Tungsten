---
name: tungsten-release
description: Prepare Git handoff, publish, verify, rehearse or recover Tungsten releases. Uses the internal release guide and read-only commit/tag preflight; delegates branch documentation and version cutting to tungsten-finalize.
---

# tungsten-release

Read [docs/releases.md](../../../docs/releases.md) for the requested stage. That guide owns commands, invariants and recovery cases; do not copy a second procedure here.

- Establish the requested outcome and existing authorization. A docs/preparation request does not authorize publication, and a failed upload does not authorize deleting a published release. Continue already-authorized work without asking again.
- Inspect version, selected commit, local/live remote branch and tags, release state and workflow runs. Distinguish a new cut, an already-cut branch, a versioned prerelease, a rehearsal, and an existing release. Do not cut again merely because the task resumed.
- Use [tungsten-finalize](../tungsten-finalize/SKILL.md) when branch docs or a version cut are still needed. Then select the final integrated commit, normally on main; explicitly select a maintenance or rehearsal branch when appropriate. A squash merge changes the commit identity.
- Run `just release-preflight VERSION --ref COMMIT --repo OWNER/REPO` with the intended remote/branch. It reads committed files and live remote state and prints proposed commands. Resolve conflicts/stale refs before executing those commands. Do not substitute the current HEAD for its verified SHA.
- Follow the guide's matching-run and artifact checks. CI is informational; report the tested SHA and outstanding GPU/platform checks accurately.
- On failure, classify the existing tag/release/draft before retrying. Keep published tags fixed; use unique rehearsal tags and new versions for new released code. Never turn a transient error into automatic deletion or an unbounded retry loop.
- Report the version/tag, SHA, release URL/run, checks and remaining work. For preparation-only requests, hand off the remaining Git steps and stop at that requested boundary.

Follow [AGENTS.md](../../../AGENTS.md), including its archive reading restriction. Do not add mandatory PRs, branch protection or CI gates.

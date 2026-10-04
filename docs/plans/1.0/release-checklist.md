# Tungsten 1.0 release checklist — draft

- **status:** in progress
- **goal:** One list of checks that, once each has passed or been accepted by the owner, tags 1.0. C3 runs it on a release candidate and records the result here.
- **non-goals:** How each check gets to green, which the workstreams own; dates.
- **files to touch:** This file; at C3, its QA record section.
- **ordered steps:** Agree the list at the definition gate; revise it at the feature and freeze gates; run it in C3.
- **done-when:** The owner has agreed every row, and every conditional row is kept or struck by the freeze gate.

Rows marked *if Qn* wait on an open question in [criteria](criteria.md) §10, and rows marked *if amendment N* on the owner's answer to that amendment in the [implementation plan](implementation-plan.md) §8. Sources are criteria sections unless another file is named. IDs carry `RC-` so they do not clash with the question numbers, the UI's glyph paths (T1, T2) or the physics proposals (G1, G2). Revised 2026-10-03 at `9cd5709`: RC-A10 added from C3's playthrough list. Agreed at the definition gate on 2026-10-03 (implementation plan §11): Q9, Q16 and amendment 16 were accepted, so RC-A5, RC-A6 and RC-A9 lost their conditions. Amendment 17's tool, `cargo-public-api` (`D-104`), runs RC-B2.

## A. Complete for making games (definition A)

| ID | Check | Run by | Source |
| --- | --- | --- | --- |
| RC-A1 | The [acceptance game](acceptance-game.md) builds in its own repository on a git dependency, using only the public API and `tungsten-kit`, with no engine patches | Owner | §1 |
| RC-A2 | Its release archive, made by `tungsten package`, plays start to finish on the Linux reference machine | Owner playthrough | §1, §8.8 C3 |
| RC-A3 | Title menu, pause and transitions work; settings for volume, rebinding and display are kept across a restart | Owner playthrough | §1 |
| RC-A4 | A corrupt settings file falls back to defaults and keeps the bad file beside them | Owner playthrough; W11 tests | [w11](w11-shipping-basics.md) |
| RC-A5 | Gamepad play | Owner playthrough | §1 |
| RC-A6 | A save slot is written, loaded, and loaded again after a schema bump | Owner playthrough | §8.8 C3 |
| RC-A7 | The output of `tungsten new` passes `tungsten check`, then builds, tests and packages outside the workspace | Automated | [w14](w14-tooling.md) |
| RC-A8 | Followed word for word on a clean machine, the getting-started guide ends with a running game | Owner | §8.8 C3 |
| RC-A9 | A fresh Claude Code session in a copy of the template, given only its `AGENTS.md`, the getting-started guide and rustdoc, adds a scripted feature with no engine patches | Owner | Implementation plan, amendment 16 |
| RC-A10 | Each display mode and present mode works in the playthrough | Owner playthrough | §8.8 C3 |

## B. Stable library (definition B)

| ID | Check | Run by | Source |
| --- | --- | --- | --- |
| RC-B1 | The stability policy is published: semver scope, MSRV rule, deprecation, and where `wgpu` and `winit` types sit | Decision entry and README | [w04](w04-api-freeze.md) |
| RC-B2 | The public surface, diffed against the freeze snapshot, shows no change | `cargo-public-api` (`D-104`) | §8.8 C3 |
| RC-B3 | Rustdoc is clean under `missing_docs`, with examples where a public item needs one | Automated | [w04](w04-api-freeze.md) |
| RC-B4 | No item remains that the policy says the freeze removes, such as the arity-named queries | Automated | [w04](w04-api-freeze.md) |

## T. Tests and tooling

| ID | Check | Run by | Source |
| --- | --- | --- | --- |
| RC-T1 | `just check`, `just smoke`, `just visual`, `just physics-release`, `just script-test`, `just deps` and `just repo-check` pass on the candidate | Automated | §8.8 C3 |
| RC-T2 | The release preflight and a `--no-pr` rehearsal pass | Automated | §8.8 C3, [releases](../../releases.md) |
| RC-T3 | A clean clone builds with the pinned toolchain | Automated | §8.8 C3 |
| RC-T4 | Determinism and containment hashes and benchmark digests are unchanged, or bumped with a recorded reason | Automated | [profiling workflow](../../perf/profiling-workflow.md) |
| RC-T5 | Asset hot reload works in a debug build | Owner | §8.8 C3 |

## P. Performance

| ID | Check | Run by | Source |
| --- | --- | --- | --- |
| RC-P1 | Every benchmark row is within its 1.0 budget, or has an exception accepted in a decision entry | Perf suite | §8.8 C1, C2 |
| RC-P2 | No row has regressed against C1's baseline | Compare verdicts | §8.8 C2 |

## S. Shipping

| ID | Check | Run by | Source |
| --- | --- | --- | --- |
| RC-S1 | A deliberate panic in a binary from the release workflow leaves a crash file whose frames resolve to file and line (Linux; Windows as hosts allow) | CI probe and owner | [w11](w11-shipping-basics.md) |
| RC-S2 | A Windows release opens no console | Owner, as hosts allow | [w11](w11-shipping-basics.md) |
| RC-S3 | Smoke runs, benchmarks and the pixel test read no user files | Automated | [w11](w11-shipping-basics.md) |

## K. Known issues

| ID | Check | Run by | Source |
| --- | --- | --- | --- |
| RC-K1 | No open P1 or P2 | [Known issues](../../known-issues.md) | §8.8 C3 |
| RC-K2 | Each open P3 has a row in the [1.x backlog](backlog-1.x.md) or a note the owner has accepted | Known issues | §8.8 C3 |

## D. Platforms, distribution and documentation

| ID | Check | Run by | Source |
| --- | --- | --- | --- |
| RC-D1 | The README states support tiers; Linux is certified; Windows and macOS results are recorded in known issues | Owner | §8.1 W7, Q3 |
| RC-D2 | Archives carry third-party licence notices, embedded fonts included | Automated check in packaging | [w09](w09-distribution.md) |
| RC-D3 | The README and DESIGN describe 1.0 | Owner | §8.8 C3 |
| RC-D4 | A macOS archive *(if Q3)* | Release workflow | Q3 |
| RC-D5 | The library crates are published on crates.io *(if Q2)* | Owner | §1, Q2 |

## QA record

C3 writes one dated table per release candidate here: tag and commit, and for each row whether it passed, was accepted (with the owner's note) or failed (with the fix and the next candidate). A candidate is a versioned prerelease, `v1.0.0-rc.N` with its own changelog section ([releases](../../releases.md#rehearsals-and-versioned-prereleases)).

# W9 Distribution and documentation — draft

- **status:** in progress
- **goal:** Archives that carry every third-party licence notice, a getting-started guide written on the template, and an answer on crates.io.
- **non-goals:** A user guide beyond getting started: the definition gate chose definitions A and B, not C (`D-102`). Platform tiers and a macOS build, which are W7's.
- **files to touch:** W9a: `scripts/release.py` (`package`) and `scripts/test-release.py`, `.github/workflows/release.yml` if a generator runs there, the notice generator's input if one is used, `docs/releases.md`, `docs/known-issues.md`. W9b: the getting-started guide, at the path W12a's plan sets, and `templates/basic/README.md`. W9c, if Q2 says yes: the library crates' `Cargo.toml` metadata, `crates/tungsten-render`'s sprite shader includes and `assets/shaders/`.
- **ordered steps:** (1) W9a, licence notices, in Phase 5 Track A after W11a and before W12a. (2) W9b's first draft inside W12a, then kept current by every milestone that changes how a game is written. (3) W9b's final pass and W9c, if Q2 says so, in Phase 7 ([implementation plan](implementation-plan.md) §3, §5).
- **done-when:** RC-D2 and RC-A8 of the [release checklist](release-checklist.md) pass at C3, and RC-D5 if Q2 says yes.

Graduated on 2026-10-03 from [criteria](criteria.md) §8.1's W9 row after the definition gate ([implementation plan](implementation-plan.md) §11), with implementation plan amendments 9 and 14 folded in. Criteria §8.1 now only places this file.

## Placement

- **Tier:** Licence notices and the getting-started guide Must; crates.io follows Q2 (`D-102` chose definitions A and B and left publication to Q2).
- **Candidates:** W9a, licence notices, the embedded font's OFL notice included (Phase 5, Track A); W9b, the getting-started guide: its first draft lands with W12a (Phase 5, Track A), it grows with the template ([workflow](workflow.md) §5), and Phase 7 gives it a final pass; W9c, crate names and publication if Q2 says so (Phase 7).
- **Needs first:** W12a, for the guide; Q2, for W9c.
- **Feeds:** W12a in a release; W14b `package`; RC-A8, RC-D2, RC-D5.
- **Owner questions:** Q2, due at the freeze gate.
- **Decisions:** Licence notices in release archives (extends `D-071` and `D-072`), with a notice generator's `D-015` rule if one is used. Crate names and publication, if Q2 says yes.

## Context digest

- W9 ships what an archive owes the authors of its dependencies, and what a new user reads first. The definition gate chose definitions A and B, not C (`D-102`), so the notices and a getting-started guide are Must, and crates.io stays question 2, due at the freeze gate because crate names and metadata freeze with the API.
- Archives today: `scripts/release.py package` stages the example binaries, a launcher per example, the shared `tungsten.json`, `input.json` and `assets/`, and the MIT `LICENSE` (`D-071`, `D-072`). The fonts' `LICENSE.txt` files ride along inside `assets/fonts/`, but nothing covers the statically linked crates ([known issues](../../known-issues.md#follow-ups)).
- W12a embeds JetBrains Mono in the umbrella crate, so every game ships its OFL notice, not just the examples, and W14b's `package` reuses the notice step for standalone games. W9a therefore runs in Phase 5, before W12a (amendment 9).
- The guide starts inside W12a as W9b's first draft. From then on, a milestone that changes how a game is written updates it with the template ([workflow](workflow.md) §5 item 6), and C3 follows it word for word on a clean machine (RC-A8).
- The sprite shaders stay in `assets/shaders/` unless Q2 says yes; then W9c moves them inside `tungsten-render` as an internal change (amendment 14).

## Scope

Moved verbatim from criteria §8.1's table on 2026-10-03, with its header row; the reference to §1 now names criteria. The bullets below the table fold in implementation plan amendments 9 and 14.

| ID | Scope sketch | Open points |
| --- | --- | --- |
| W9 Distribution | Third-party licence notices in archives, also for fonts the engine embeds (W12); the crates.io decision and crate names; user documentation, starting with a getting-started guide on the template (W12) | Follows criteria §1 |

- **The licence notices come early.** They are needed as soon as W12a embeds JetBrains Mono, and by W14's `package`, not in the closing phase.
- **Shader ownership waits for Q2.** A git dependency checks out the whole repository, so `include_str!` from `../../../assets/shaders/` already works for a game in its own repository; only a crates.io package needs `sprite.wgsl` and `lit_sprite.wgsl` inside the crate. W12a leaves both shaders where they are, and a yes on Q2 moves them in W9c as an internal change.

## Steps

The workstream's candidates in order. Each candidate's milestone plan holds its execution steps.

1. **W9a: licence notices.** Phase 5, Track A, after W11a and before W12a. A notices file in every archive that names each statically linked crate with its licence text, and the OFL notice of every font the engine embeds; by hand or with a generator, which then needs its `D-015` argument. Level B ([workflow](workflow.md) §4).
   - **Done-when:** `scripts/release.py package` writes the notices file into each archive and fails when a crate in the release build's dependency graph has no entry; `just script-test` covers both cases; a rehearsal prerelease's Linux and Windows archives contain the file, and the owner has read it; the known-issues follow-up on notices is gone; the decision entry extends `D-071` and `D-072`.
2. **W9b: the getting-started guide.** Its first draft lands inside W12a, written on the template skeleton; packaging joins it with W14b. From then on, every milestone that changes how a game is written updates it in the same step as the template ([workflow](workflow.md) §5 item 6). Phase 7 gives it a final pass, with W7b and W9c ([implementation plan](implementation-plan.md) §5).
   - **Done-when:** W12a's release carries the first draft, and each later release that changes the template changes the guide too. Final pass: each command in the guide runs as written against the release candidate's tag, and RC-A8 passes at C3, the guide followed word for word on a clean machine ending with a running game.
3. **W9c: crate names and publication, if Q2 says yes.** Phase 7. Crate names and metadata, `sprite.wgsl` and `lit_sprite.wgsl` moved inside `tungsten-render` with their hot-reload copies kept in `assets/shaders/`, then publication. If Q2 says no, the freeze gate strikes W9c and RC-D5.
   - **Done-when:** `cargo publish --dry-run --workspace` passes for the library crates; `just check`, `just smoke` and `just visual` pass with the shaders included from inside `tungsten-render`; RC-D5 passes at C3.

## Done-when

At C3, RC-D2 passes on the release candidate's archives and RC-A8 on the guide; RC-D5 passes if Q2 said yes, or was struck at the freeze gate.

## Follow-ups

None yet.

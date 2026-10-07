# W4 API stabilization and freeze — draft

- **status:** in progress
- **goal:** A written stability policy, every break landed before the freeze, a documented public surface, and a snapshot that 1.x releases are checked against.
- **non-goals:** Holding breaks for 2.0 that could land before the freeze; API changes after the freeze without an exception in a decision entry (criteria §8.8 C2). Landing each break, which its own candidate does; the ledger only tracks it.
- **files to touch:** This file, whose break ledger every milestone that breaks public API updates. W4a: `DECISIONS.md` and `docs/DECISION_INDEX.md` (done). W4b: the public items of the library crates (`#[non_exhaustive]`, rustdoc, the deprecated items removed), the `missing_docs` lint in each library crate's root, the API snapshot files `api/<crate>.txt` that Step 0d set up (`just api`, `D-104`, `D-107`), and the README's stability section (RC-B1).
- **ordered steps:** (1) W4a, the stability and `wgpu`/`winit` policy with the snapshot tool: done at the definition gate (`D-103`, `D-104`). (2) The break ledger, kept by every milestone until the freeze. (3) W4b, the freeze, in Phase 7 after C1; then the freeze gate ([implementation plan](implementation-plan.md) §2, §5).
- **done-when:** The freeze gate passes its W4 items: every ledger row landed, the arity-named queries removed, `missing_docs` clean, the API snapshot taken. RC-B1–RC-B4 of the [release checklist](release-checklist.md) pass at C3.

Graduated on 2026-10-03 from [criteria](criteria.md) §7 after the definition gate ([implementation plan](implementation-plan.md) §11), with implementation plan amendment 17 folded in. Criteria §7 now only places this file.

## Placement

- **Tier:** Must (Q1, `D-102`).
- **Candidates:** W4a, the stability and `wgpu`/`winit` policy and the snapshot tool (definition gate; decision entries only; done); the break ledger below; W4b, the freeze (Phase 7).
- **Needs first:** Q1 and Q11, both answered at the definition gate (`D-102`, `D-103`); Step 0d for the snapshot recipe (landed: `D-107`).
- **Feeds:** Every workstream that adds public surface designs against W4a (`D-103`) and records its breaks here ([workflow](workflow.md) §5).
- **Owner questions:** Q2, due at the freeze gate. Q11 and Q24 are answered (`D-103`).
- **Decisions:** `D-103` (stability policy; amends `D-069`); `D-104` (the snapshot tool; amended by `D-105` and `D-107`). A freeze exception takes an entry of its own.

## Break ledger

Every public-API break bound for 1.0, with its source and where it landed. Seeded at graduation from the breaks list in the scope below and from `D-103`; from now on this table is the list. A milestone that lands a break fills in its row, and one that finds a new break adds a row ([workflow](workflow.md) §5).

| Break | From | Status | Landed in |
| --- | --- | --- | --- |
| Interned asset IDs in place of `Sprite.asset_id: String` | Scope (R0) | Landed (`D-113`): `Sprite.asset_id` and `Sprite::new` take a `SpriteAssetId`; `AnimationFrame.sprite` is an ID; `AnimationData::load` takes `&mut AssetRegistry`; `AnimationState::current_sprite` and `advance` return IDs; `sprite_ids()` and `sprite_id_for_path` are renamed `sprite_names()` and `sprite_name_for_path`; `spawn_particle_via` takes a `SpriteAssetId`; `AnimationData` and `AnimationFrame` no longer implement `Deserialize`; `register_sprite` returns the ID | 0.45 (M33) |
| Real `KeyCode` variants in place of `KeyCode::Other(<winit discriminant>)` | Scope (UI M3) | Planned: W1 M0b, where [w01](w01-ui-text-suite.md) §11 puts it | — |
| A presented, skipped or failed render result; `App::run` returns runtime errors | Scope (P3) | Planned: W8a | — |
| `#[non_exhaustive]` where public enums and config structs may grow | Scope | Planned: W4b's sweep; a new type takes it when it lands | — |
| `Send`/`Sync` bounds on components and resources | Scope ([w02](w02-multi-core-rendering.md#decisions-the-pass-needs-first)) | Decided at the frame-loop gate (Q6, `D-131`): no bound; not a break | — |
| `World::query` and `query_mut` take query data (`&T`, tuples) in place of a component type | Frame-loop gate (`D-130`) | Landed (`D-135`): `query::<Q>()` and `query_mut::<Q>()` take query data (`&T`, `&mut T`, `Option<&T>`, `Entity`, tuples of up to eight), with `query_filtered`, `query_mut_filtered` and the four `_slices` forms beside them; a component type there no longer compiles, so callers write `query::<&T>()` or `query::<(Entity, &T)>()` | 0.54 (M40) |
| `display.scale_mode` removed from `tungsten.json` and `DisplayConfig` | Glyph gate (`D-127`) | Planned: W4b's sweep | — |
| A bundle-insert API | Scope ([engine findings](../../perf/benchmarks.md#engine-findings)) | Landed (`D-135`): `Bundle` with one `put` (tuples of up to sixteen components, `Chain` through `with`, game types), `World::spawn_with` and `insert_bundle` as one archetype move, `CommandBuffer::spawn_with`, `insert_bundle` and `insert_bundle_pending`, `RigidBodyBundle`; an addition, not a break | 0.54 (M40) |
| `DeltaTime` folded into the game clock | Scope ([w03](w03-frame-loop.md#scope)); `D-129` | Deprecated at 0.53 (`D-134`): `App` still writes the game dt into it each frame; engine systems read `Time`, so a world built by hand inserts and advances `Time` for them to move; W4b removes it | 0.53 (M39) deprecated; removal W4b |
| winit 0.31 | Scope | Planned: W1 M0b, if it has left prerelease | — |
| Single-activation UI event reads | Scope ([w01](w01-ui-text-suite.md) §7) | Planned: W1 M3 | — |
| Arity-named queries removed in favour of tuple queries | Scope (W15); `D-130` | Deprecated at 0.54 (`D-135`): the eleven functions other than `query` and `query_mut` carry `#[deprecated(since = "0.54.0")]` with their tuple form in the note, and every engine, example and template call site reads the tuple forms; W4b removes them | 0.54 (M40) deprecated; removal W4b |
| A curated render re-export in place of `pub use tungsten_render as render` | `D-103` | Not placed (follow-up 1) | — |
| `Renderer` methods that take or return `wgpu` types (`surface_format`) behind a game-facing handle or a `doc(hidden)` tier | `D-103` | Not placed (follow-up 1) | — |
| Engine-owned types in place of `winit` types in public signatures (`translate_mouse_button`) | `D-103`; criteria §2.1 | Not placed (follow-up 1) | — |
| `RawManifest` and `ResolvedManifest` gain `font_families` and `font_fallback`; `ManifestError` gains two variants | W1 M0a (plan audit A21) | Landed (`D-115`): struct literals of either manifest type need the two fields, and exhaustive matches on `ManifestError` need `FontFamilyFaceMissing` and `UnknownFallbackFamily`; the new `FontFamilyEntry` and `ResolvedFontFamily` are `#[non_exhaustive]` | 0.46 (M34) |
| `RenderConfig` gains `system_fonts` | W1 M0a (plan audit A21) | Landed (`D-116`): struct literals of `RenderConfig` need the field or `..RenderConfig::default()`; `tungsten.json` files need nothing, since it defaults to `false` | 0.46 (M34) |
| `TextSection` gains `layout`; struct literals need `..Default::default()` | W1 M0a (plan Q1) | Landed (`D-117`): `TextSection` gains `layout: TextLayout` and derives `Default`; the 22 literals in the workspace gained `..Default::default()` | 0.46 (M34) |
| `Config` gains `game` and a private field, so code outside `tungsten-core` can no longer build it by literal | W11a (M35 §2.10) | Landed (`D-119`): `Config.game: GameConfig` (new, `#[non_exhaustive]`, with `GameConfig::validate`) and `Config::take_load_warnings` are new; a private `load_warnings` field means games build a `Config` with `Config::default()` or `Config::load` and set its fields; nothing in the workspace built one by literal | 0.48 (M35) |
| `InspectorState::new_with_defaults`, `register` and `registered_len` removed in favour of `InspectRegistry` (`App::register_inspectable`, `tungsten::inspector::default_inspect_registry`) | W15a (M38 Q2); `D-133` | Deprecated at 0.52 (`D-133`) as working shims with their old behaviour; W4b removes them with the other deprecated items. `WindowSize`'s move to `tungsten_core::display` in the same milestone is not a break: `tungsten::WindowSize` and `tungsten::app::WindowSize` re-export it; likewise the `SystemFn` and `InspectFn` aliases, now core's, re-exported at `tungsten::app::SystemFn` and `tungsten::inspector::InspectFn` with the same type | 0.52 (M38) deprecated; removal W4b |
| `FrameTimings` gains `fixed_steps`; struct literals need `..Default::default()` | W3b (M41 §2.6) | Landed (`D-137`): `FrameTimings::fixed_steps: u32` counts the frame's fixed steps; `system_timings` lists each name once a frame, a `fixed_update` system's runs summed and 0 ms in a frame with no step; a struct literal needs the field or `..Default::default()` | 0.55 (M41) |
| `RigidBodyBundle` puts `PrevPosition` beside `Position` | W3b (M41 Q3); `D-129` | Landed (`D-137`): a behaviour change, not a compile break; a static bundle body has four components and a dynamic one five, so code that counted a body's components or matched its archetype by inserts adds `PrevPosition`; a teleport writes both | 0.55 (M41) |

## Context digest

- W4 makes definition B real: 1.x releases add without breaking. It is Must because the definition gate chose definitions A and B (`D-102`).
- W4a is done. `D-103` sets what semver covers from 1.0: the `tungsten` crate's API with the kit inside and a curated render re-export, the game-facing file formats, and the CLI's commands and flags; `wgpu` and `winit` types stay outside. MSRV rises only in a 1.x minor, with the pinned toolchain. A replaced item stays deprecated until 2.0, and a renamed config field keeps its old name as an alias. `D-104` records the public surface with `cargo-public-api`, one tracked file per library crate; since `D-105`, a milestone regenerates the files before its release, and Step 0d lands the recipe and the toolchain it runs on.
- Until the freeze, 0.x releases still break, each break through the ledger above. Each new public item has rustdoc, and new surface shows in the snapshot diff ([workflow](workflow.md) §5).
- W4b comes second in Phase 7, after C1, so fixes that need an API change land before it. The freeze gate then needs every ledger row landed, the arity-named queries removed, `missing_docs` clean and the snapshot taken; after it, an API change needs an exception in a decision entry.
- Open: Q2 (crates.io), due at the freeze gate, since crate names and metadata freeze with the API. Three of `D-103`'s breaks have no candidate yet (follow-up 1).

## Scope

Moved verbatim from criteria §7 on 2026-10-03; section references now name criteria, and the last bullet folds in implementation plan amendment 17.

- **Policy to write:** what semver covers, the MSRV rule (`D-069` pins 1.98.1), deprecation in 1.x.
- **`wgpu` and `winit` in the API: decided in the first step, with criteria §1, not at the freeze.** W1's draw and text API, R2 and R4's renderer changes and W5's input types are new public surface, and this policy decides their shape. The whole-crate re-exports and render's `wgpu`-typed signatures tie Tungsten's major version to wgpu's, which ships breaking majors several times a year (30.0.1 is locked). Options: hide them (`pub(crate)` pipelines, engine-owned format and input types), declare an advanced tier outside the promise, or accept a Tungsten major per wgpu major.
- **What hiding costs.** No example imports `wgpu` or `winit`. Examples reach render only through `Renderer` (texture and material registration in example 02), `TextSection`, `SpriteBatch`, `SpriteInstance` and `compare_png`, all through the umbrella's `render` re-export since 0.40 QA step 6 dropped example 02's direct `tungsten-render` dependency, the only one, so most of the work is a curated umbrella re-export in place of `pub use tungsten_render as render`. The snag is `Renderer`: the umbrella and render are separate crates, so every method the umbrella calls is `pub` and reaches games with the type (`surface_format` returns a `wgpu::TextureFormat`). Hiding needs a game-facing handle or a `doc(hidden)` tier for those methods.
- **winit 0.31** is still a prerelease (known issues). If it ships before UI M0b, upgrade first so the input bridge is rewritten once.
- **Breaks to land before the freeze (candidates):** interned asset IDs (R0); real `KeyCode` variants (UI M3); a presented/skipped/failed render result and `App::run` returning runtime errors (P3); `#[non_exhaustive]` where public enums and config structs may grow; the `Send`/`Sync` policy ([w02](w02-multi-core-rendering.md#decisions-the-pass-needs-first)); a bundle-insert API ([engine findings](../../perf/benchmarks.md#engine-findings)); `DeltaTime` folded into the game clock ([w03](w03-frame-loop.md#scope)); winit 0.31; single-activation UI event reads (UI draft §7); the arity-named queries removed in favour of tuple queries (W15). New surface the freeze also covers: the column-slice query (R1), which builds on W15's query machinery; stages, plugins, the engine's system names, bundles and additive extracts (W15); the prefab format and component registry (W16); the headless harness (W14); the core UI model, `tungsten_core::ui` with `UiTree`, `LayoutStyle` and the theme types, the `TextNodeStore` seam and `tungsten::testing::UiHarness` (W1 M1, 0.50; additions only, no break landed); the game clock, `tungsten_core::time::Time`, `Timer` and `TimerMode` (W3a, 0.53; `DeltaTime` deprecated, the ledger row); the tuple query module with `With`, `Without` and `OptionalColumn`, `Bundle`, `Chain`, `spawn_with`, `insert_bundle`, the command-buffer forms, `RigidBodyBundle` and `resource`/`resource_mut` (W15b, 0.54; the three ledger rows); the kit, unless it takes its own tier (criteria §8.4).
- **Documentation:** `missing_docs` on public items, rustdoc examples, a getting-started guide that walks through the template (W12).
- The freeze is the last step before release candidates; every break above collects here first.
- **API snapshot, chosen with the policy.** Left to C3, the tool would find new public surface all at once at the freeze. Chosen with W4a, a tracked snapshot can be updated by each milestone ([workflow](workflow.md) §5, §8), and its diff shows in the changes the owner reviews before each release (`D-105`); C3 diffs the public surface against the freeze's snapshot. The choice includes how the tool runs on the pinned stable toolchain (`D-069`). *Chosen at the definition gate: `cargo-public-api` (`D-104`).*

## Steps

The workstream's candidates in order. Each candidate's milestone plan holds its execution steps.

1. **W4a: stability policy, `wgpu`/`winit` policy and snapshot tool.** Decision entries only, at the definition gate. *Done 2026-10-03:* `D-103` (amends `D-069`) and `D-104`.
   - **Done-when:** `rg -n '^## D-10[34] ' DECISIONS.md` finds both entries, `docs/DECISION_INDEX.md` has their rows, and `D-069` carries its amendment marker. Met at the definition gate.
2. **The break ledger.** From now until W4b, every milestone that breaks public API fills in or adds its row above, and every milestone that changes public surface regenerates the snapshot files with `just api` before its release; the release checks run `just api-check` (`D-107`, [workflow](workflow.md) §5).
   - **Done-when:** At the freeze gate every row reads "Landed" with its release, or was struck with the owner's note in the gate record; each release since Step 0d carries its snapshot diff.
3. **W4b: the freeze.** Phase 7, after C1 ([implementation plan](implementation-plan.md) §5): the ledger's last rows (the deprecated arity-named queries and `DeltaTime` removed), the `#[non_exhaustive]` sweep, `missing_docs` in every library crate, rustdoc examples where a public item needs one, the stability policy published in the README (RC-B1), and the freeze snapshot taken with `D-104`'s recipe. Then the freeze gate.
   - **Done-when:** `just check` passes with `missing_docs` denied in every library crate; the regenerated snapshot files list no arity-named query and no `DeltaTime`; every ledger row has landed; the README states the policy; the freeze gate's record signs off its W4 items, and the freeze snapshot is the baseline RC-B2 diffs against.

## Done-when

The freeze gate's record ([implementation plan](implementation-plan.md) §11) passes its W4 items: break ledger landed, arity-named queries removed, `missing_docs` clean, API snapshot taken. At C3, RC-B1–RC-B4 pass on the release candidate.

## Follow-ups

1. **`D-103`'s hiding work has no candidate.** The curated render re-export, a game-facing handle or `doc(hidden)` tier for the `Renderer` methods that take or return `wgpu` types, and engine-owned types in place of `winit` ones in public signatures (the ledger's last three rows). W4b is the fallback; the Phase 6 and 7 cards written at the frame-loop gate can place them earlier, for example beside W1 M0b, which rewrites the input bridge.

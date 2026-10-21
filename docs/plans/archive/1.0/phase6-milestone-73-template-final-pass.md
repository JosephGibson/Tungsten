# Phase 6 milestone 73: template final pass (Template final pass) — draft

- **status:** done
- **goal:** The owner's request of 2026-10-20: the template's last pass before 01 (b) grows example 01. It fixes the death loop after a load and the Settings toggles that miss their first click. It gives saves three rotating autosaves the player can turn off, a question before an older version's save loads, and a clearer save screen. It adds a Bloom option, a 75 % default UI scale, a smaller outlined slider thumb, an Uncapped row with a 60 fps floor, a game over in the Dark Souls style over a pixelating world, a black play area, a still enemy and a Windows icon. Then it brings every maintained doc up to the tree. Example 01 takes each shell change by copy: its `src/shell/` stays the template's byte for byte (`check_shell_copies`, `D-197`), and from step 2 its theme file does too.
- **non-goals:** Example 01's own features (01 (b)–(d)), its clear colour and its icon. Items 1, 11, 12 and 13 are the template's alone, and example 01 changes nowhere for them. No new WGSL file or post pass (`D-054`, `D-058`). No new kit API: the death loop's grace is data on the kit's effects and tag filters (§2.2). No engine `Settings` field: Bloom and Autosave are gameplay options. Core's slider geometry stays (`D-162`, Q6). No resource crate for the icon (`D-015`). Out of scope for edits: `DECISIONS.md`'s settled entries, `CHANGELOG.md`'s released sections, `docs/plans/` beyond the records this stop owes, and `docs/plans/archive/`.
- **files to touch:**
  - **Step 1:** `crates/tungsten-core/src/ui/theme.rs` and `ui/theme_file.rs`; `crates/tungsten-core/src/tests/ui/{style,theme_file,layout}.rs`; `crates/tungsten/src/debug_views.rs` and `crates/tungsten/src/tests/debug_views.rs`; `api/` (what `just api` moves); `DECISIONS.md`, `docs/DECISION_INDEX.md`; `docs/plans/1.0/w04-api-freeze.md` (the ledger row).
  - **Step 2:** `scripts/check-repo.py`, `scripts/test-check-repo.py`.
  - **Step 3:** `templates/basic/src/sample/mod.rs`; `templates/basic/assets/data/effects/spawn_grace.json` (new), `assets/data/tags.json`, `assets/manifest.json`, `assets/prefabs/{hazard,enemy_orbit,enemy_patrol}.json`; `templates/basic/tests/game/saves.rs`.
  - **Steps 4–11:** `templates/basic/src/shell/` (the files each step names), with its copy in `examples/01_platformer/src/shell/`.
    - Step 4: `templates/basic/tests/game/{main,settings}.rs`, `templates/basic/README.md`.
    - Step 5: `templates/basic/tests/game/{settings,fit}.rs`, `templates/basic/tests/fixtures/title.png`.
    - Step 6: `templates/basic/assets/ui/themes/shell.json` and its copy `examples/01_platformer/assets/ui/themes/shell.json`, `templates/basic/tests/game/settings.rs`.
    - Step 7: `templates/basic/tests/game/{saves,shell,settings}.rs`, `templates/basic/{AGENTS,README}.md`, `examples/01_platformer/tests/game/{areas,playthrough,saves}.rs`, `examples/01_platformer/README.md`.
    - Step 8: `templates/basic/tests/game/{saves,shell}.rs`.
    - Step 9: `templates/basic/tests/game/{saves,fit}.rs`.
    - Step 10: `templates/basic/src/sample/mod.rs`, `templates/basic/tests/game/{settings,sample}.rs`, `templates/basic/AGENTS.md`, `examples/01_platformer/tests/game/presentation.rs`, `examples/01_platformer/README.md`.
    - Step 11: `templates/basic/tests/game/{shell,fit}.rs`, `templates/basic/README.md`, `examples/01_platformer/tests/game/presentation.rs`.
  - **Step 12:** `templates/basic/tungsten.json`, `templates/basic/tests/fixtures/title.png`.
  - **Step 13:** `templates/basic/assets/prefabs/enemy_still.json` (new), `assets/manifest.json`, `src/sample/mod.rs`, `tests/game/sample.rs`, `README.md`.
  - **Step 14:** `templates/basic/tools/icon.py`, `templates/basic/windows/icon.ico` and `icon.res`, `templates/basic/build.rs` (all new); `templates/basic/tools/README.md`, the `justfile` (`script-test`), `docs/plans/1.0/w12-template.md` (the follow-ups).
  - **Step 15:** none, unless a check names one.
  - **Step 16:** `DECISIONS.md`, `docs/DECISION_INDEX.md`; `README.md`, `AGENTS.md`, `docs/*.md`, `docs/perf/*.md`, `crates/tungsten-render/AGENTS.md`, and `DESIGN.md`'s Core UI Model and Template Shell sections.
  - **Step 17:** `templates/basic/{AGENTS,README}.md`, `templates/basic/tools/README.md`, `examples/01_platformer/{README,tools/README}.md`, `examples/03_fixtures/tools/README.md`.
  - **Step 18:** the skeleton's release files, `docs/plans/1.0/w12-template.md`, `docs/known-issues.md`.
- **ordered steps:**
  1. Letter spacing on UI text.
  2. The shell-copy check covers the theme.
  3. The death loop.
  4. Toggles take their first click; Uncapped at 60 fps.
  5. The UI at 75 % by default.
  6. A smaller, outlined slider thumb.
  7. Three rotating autosaves and an Autosave option.
  8. A question before an older save loads.
  9. The save screen's look.
  10. Bloom.
  11. Game over.
  12. A black play area.
  13. A still enemy.
  14. A Windows icon.
  15. Example 01 follows the template.
  16. Docs, part 1: the pass's decision and the engine's docs.
  17. Docs, part 2: the template's and example 01's docs.
  18. Release 0.94.
- **done-when:**
  - Every step's evidence row is quoted, and workflow §5 is closed.
  - After the cut, the release checks pass, with `just visual` and `just template-check`.
  - The preflight prints the command block and the post-merge block.

Candidate: Template final pass ([implementation plan](../../1.0/implementation-plan.md) §4 card, level C). Milestone M73, release 0.94. Written 2026-10-20 at `ac6dd96`. Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs.

## Context digest

- **Why now.** 0.93 put example 01 on the template (`D-197`). The owner asked for this pass on 2026-10-20, before 01 (b) builds on that shell. It takes M73, so 01 (b) moves to M74.
- **What it stands on.** The template shell (`D-181`, as `D-187`, `D-189`, `D-191`, `D-192`, `D-194` and `D-197` amend it). The kit's effects and tag filters. The engine's closed `PostPass` set (`D-054`, `D-058`; bloom on an sRGB scene, `D-060`). The core UI's style model (`D-125`).
- **Why level C.** The style model's inherited text properties gain letter spacing (A29). That is the pass's one new public API, so it is step 1, with its own critique (workflow §4).
- **How the two games move together.** A step that edits `templates/basic/src/shell/` or the shell's theme copies the file into `examples/01_platformer` in the same step. It then runs both crates' tests and `just repo-check`, whose shell-copy check covers the theme from step 2.
  - Example 01's generator already copies the shell's theme and fonts from the template, and its `--check` holds them byte-equal (A36).
  - Its seven shell sounds are its own synthesis, so no step copies a sound, and none edits one.
- **The budget.** `tokei` is installed (15.0.0): the shell measures 2,393 of `D-187`'s 2,400 lines of code and `src/` 2,890 of 2,900 (A37). The pass grows both, and step 16's decision sets new caps from tokei's counts (Q22). The template's `AGENTS.md` has 18 bytes of `D-183`'s 12,288 left, so every edit there is byte-neutral.
- **What later stops expect.** 01 (b)–(d) keep this shell unchanged (the design's shared rules), and the Phase 6 QA pass reads the template as this pass leaves it.

## 1. Audit

Read at `ac6dd96` by the plan session and four read-only agents. `src/`, `assets/`, `tests/` and `tools/` with no crate named are `templates/basic/`'s. Owner pointers that differ are marked **(adapted)**.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | `save::capture` refuses a dead player, and also returns `None` with no run, `SampleRun`, `Health` or `Position` | `src/save.rs:69-78` |
| A2 | **(adapted)** `shell_save` in `PreUpdate` captures only three saves: the run's first autosave, `request_autosave`'s and the exit's. Pause saves from its own `update` on its first stepless frame, Quit to Title and Quit Game save from `quit`, and the Save screen from `saved`; focus loss only pushes pause | `src/shell/mod.rs:211-213`, `:496-500`, `:555-571`, `:663-680`; `src/shell/pause.rs:57-61`; `src/shell/saves.rs:131-139` |
| A3 | Every load respawns all four enemies alive at their `ENEMIES` places, motion restarted, with no memory of hits, so a touch lands on the first step | `src/sample/mod.rs:43-48`, `:124-132`, `:296-298`; `crates/tungsten-kit/src/overlap/contact.rs:43-50`, `:65-66` |
| A4 | Retry reloads `retry_from`, which every successful write sets. A dead player gets no new autosave, so Continue keeps finding the deadly save too | `src/shell/game_over.rs:28-41`; `src/shell/mod.rs:457-467`; `src/shell/title.rs:212-215` |
| A5 | **(adapted)** `Health`'s window is saved (`window_left` serializes), but only a hit opens it and no public method does | `crates/tungsten-kit/src/damage/health.rs:95-160`, `:278-282` |
| A6 | Where a load kills at low health: the hazard's line, 160 px below the start (±240 px over 4 s, 1 damage per 0.5 s, reach 32 px); 64 px right of an orbiter's place, where it stands on step 1; under the patrol. New Game's first autosave is safe: the start is at least 160 px from every reach | `assets/prefabs/hazard.json:10-19`; `enemy_orbit.json:9-18`; `enemy_patrol.json:18-19`; `src/sample/mod.rs:35-48` |
| A7 | `a_restored_player_takes_a_first_step_hit_at_its_saved_health` pins the very hit the fix removes | `tests/game/saves.rs:162-187` |
| A8 | The kit's data forms carry an effect's `grant` tags and a filter's `none`. `EffectRequest::new(..).apply(world)` lands at once, before the next step | `crates/tungsten-kit/src/damage/effects.rs:228-262`, `:1560`, `:1621`; `crates/tungsten-kit/src/foundation/tags.rs:175-217` |
| A9 | An activation sends `Activated`, then flips a toggle and sends `ValueChanged` | `crates/tungsten-core/src/ui/route.rs:1247-1258` |
| A10 | **(adapted)** Settings' `Activated` arm refreshes every row from the world before the same routing's `ValueChanged` applies the new value. The box keeps the old value while the setting changes, so a toggle needs two clicks to show one change. Unlimited is the exception: its `set` refreshes again. The owner's suspect, the `!idle` return, does not fire on these clicks: a push queues for one frame, Settings' tree does not exist while that frame routes, and a tab switch starts no screen change | `src/shell/settings.rs:621-626`, `:342-346`, `:429-439`, `:592`; `src/shell/mod.rs:701-723` |
| A11 | No test checks a toggle's box after a press. `UiHarness::click` exists and is unused | `tests/game/settings.rs:287-289`, `:626-643`; `crates/tungsten/src/testing/ui.rs:155-170` |
| A12 | Frame-rate row: `FPS = [30, 480, 5]`, `Row::Unlimited` with its caption. A stored cap sets the slider, which core clamps, while the readout prints the stored cap. A press at the floor sends no change | `src/shell/settings.rs:39-42`, `:50`, `:91`, `:324-334`, `:429-439`; `crates/tungsten-core/src/ui/route.rs:1260-1270` |
| A13 | The engine applies an unset `ui_scale` as 1.0, and `App::new` sets the scale only when one is stored. The shell's slider reads 1.0 when unset, Reset unsets it, and `ScaleFactor::set_ui_scale` is public. **(adapted)** `REFERENCE_HEIGHT` is read at `sample/mod.rs:158`, in `zoom` (`:154`) | `crates/tungsten/src/settings.rs:40-45`; `src/shell/settings.rs:350`, `:478-481`; `src/shell/mod.rs:203-206`, `:384-394` |
| A14 | Tests that assume the 1.0 default; `fit.rs` reaches the 731 × 411 floor only at 1.0 | `tests/game/settings.rs:414`, `:430`, `:540-545`, `:597`; `tests/game/fit.rs:17-58` |
| A15 | The slider's thumb is a `min(h, w)` square on a `h / 4` track, `h` being the line height (22). A slider paints no box: its border colour and width outline the thumb alone. `shell.json` gives it a border only when focused, and the inherited normal border is the track's grey at width 0 | `crates/tungsten-core/src/ui/control.rs:190-194`, `:240-248`; `crates/tungsten-core/src/ui/paint.rs:896`, `:972-980`; `assets/ui/themes/shell.json:36-40` |
| A16 | One `autosave` slot leads `SLOTS`. `autosave()` and `save_due` write it | `src/shell/mod.rs:43-49`, `:476-479`, `:559-567` |
| A17 | New Game asks "The autosave will be replaced" when `slots[0]` is ready; Continue loads the newest readable slot | `src/shell/title.rs:60-76`, `:88-90`, `:212-215`; `src/shell/mod.rs:439-448` |
| A18 | Each row is one full-width mono button holding a dot-separated line, plus a red Delete. In Save mode the autosave row reads "Written when you pause or quit", and `slot_name` names any non-`slot_` slot "Autosave" | `src/shell/saves.rs:55-98`, `:156-162`, `:193-213` |
| A19 | `RunMeta.game_version` records the writer, and schema 1's migration writes `"unknown"`. Nothing compares it with `GameInfo.version` | `src/shell/mod.rs:85-93`, `:233-242`; `src/save.rs:50-55` |
| A20 | A button is a node whose first label child holds its text, and it may hold more children (the tab rail's bar). Spans override weight, italic and colour only, not size or family, so a two-style row is two labels and needs no core change | `crates/tungsten-core/src/ui/tree.rs:327-331`, `:548-559`; `src/shell/widgets.rs:273-288`; `crates/tungsten-core/src/text.rs:261-273` |
| A21 | `autosave_1`–`autosave_3` are valid slot names. `save` stamps `saved_at_ms` into the envelope, and `load` ignores the envelope's slot name, so a renamed file loads with its time | `crates/tungsten-core/src/save.rs:25`, `:233-251`, `:263-308`, `:319-361` |
| A22 | Tests on the autosave or the slot list: 16 in `shell.rs`, 8 in `saves.rs`, 1 in `settings.rs`, and 3 in example 01 | `tests/game/shell.rs:69`, `:79-1086`, `:1036-1047`; `tests/game/saves.rs:74`, `:95`, `:163`, `:190`, `:287`, `:542`, `:586`, `:622`; `tests/game/settings.rs:646`; `examples/01_platformer/tests/game/areas.rs:32-33`, `:49-120`; `playthrough.rs:143-150`, `:337`; `saves.rs:494-516` |
| A23 | `GameOptions` is `Copy`, `#[serde(default)]` and schema 1, so new fields load from old files. Mute when unfocused is a `GameOptions` row outside the Gameplay tab | `src/shell/options.rs:15-40`; `src/shell/settings.rs:96`, `:344`, `:444-454`, `:485` |
| A24 | `PostStack(pub Vec<PostPass>)` has `push`, `clear`, `len`, `reorder` and `as_slice(_mut)`, no remove, and its passes have no identity. `App` inserts an empty stack and reads it each frame. A change rebuilds no pipeline and at most one bloom bind group | `crates/tungsten-core/src/post.rs:396-449`; `crates/tungsten/src/app.rs:359-360`, `:1561-1575`; `crates/tungsten-render/src/post/bloom.rs:124-132` |
| A25 | `BloomParams` defaults to threshold 1.0, knee 0.5, intensity 0.7, radius 1.0. On an sRGB scene that barely blooms. **(adapted)** The sample builds no stack today. Among the sample's sprites, every pixel of the bolt and the spark has a full channel, as do 52 of the gem's 148, 168 of the speed pickup's 276 and none of the dummy's | `crates/tungsten-core/src/post.rs:329-338`; `tools/sprites.py:145-230` |
| A26 | Example 01 inserts `[Bloom 0.72, Vignette]` at run start and `PostStack::default()` at its end; its presentation test asserts the pair | `examples/01_platformer/src/features/level/mod.rs:146`, `:157-162`, `:165-181`; `examples/01_platformer/tests/game/presentation.rs:517-525` |
| A27 | Game over is a `Menu`: "You died" in the danger colour at 44 units (capped at 52) inside a menu card, with Retry, Load and Quit to Title, over a scrim that deepens to `DIED_TINT` over 0.9 s | `src/shell/game_over.rs:48-60`; `src/shell/theme.rs:36-39`, `:80-84`, `:117-127`; `src/shell/mod.rs:503-553` |
| A28 | The old death screen added `Pixelate { max_block_px: 48 }` and a crimson `Fade` at `0.8 × QuadOut(t / 0.9 s)` on the real clock. It also removed every Fade and Pixelate each frame, which would strip a game's own. `TransitionEffect::pass_at` is public | `git show v0.92.0:examples/01_platformer/src/death.rs` (`:12-22`, `:39-56`, `:137-145`); `crates/tungsten/src/transition.rs:14-72` |
| A29 | **(adapted)** UI text cannot take letter spacing. `TextLayout` carries it and both measures read it, but `TextOverrides` has no field and the theme file's `TextFile` refuses unknown ones. Nothing upper-cases text | `crates/tungsten-core/src/text.rs:147-148`; `crates/tungsten-core/src/ui/measure.rs:177-178`; `crates/tungsten-core/src/ui/theme.rs:468-528`; `crates/tungsten-core/src/ui/theme_file.rs:346-372` |
| A30 | Two `TextOverrides` literals name every field: the debug views' `Look::text`, which pins every text property against a game's theme (M58), and its hostile-theme test | `crates/tungsten/src/debug_views.rs:274-293`; `crates/tungsten/src/tests/debug_views.rs:108-126` |
| A31 | Game-over tests check the heading's text and colour and the tint after 60 steps. Four template tests and example 01's area test press Retry right after `settle` | `tests/game/shell.rs:364-383`, `:396`, `:443`, `:493`; `tests/game/settings.rs:122`; `examples/01_platformer/tests/game/areas.rs:103` |
| A32 | Both games clear to the background token in linear light. The title's scrim (alpha 184) shows the clear colour, so a black clear moves `title.png`, which the guide's command re-captures | `templates/basic/tungsten.json:2-9`; `examples/01_platformer/tungsten.json:2-9`; `src/shell/widgets.rs:22-37`; `docs/getting-started.md:31-34`; `tests/game/visual.rs:13-37` |
| A33 | Of the four `ENEMIES`, the second `enemy_orbit` stands at `[1100, 380]`; the enemies' test checks all four move | `src/sample/mod.rs:43-48`; `assets/prefabs/enemy_orbit.json`; `tests/game/sample.rs` (`enemies_move_take_three_hits_and_come_back`) |
| A34 | No icon exists anywhere, and the template has no `build.rs`. `tungsten new` embeds every file but `target/` and `dist/`, non-UTF-8 files verbatim, and a test holds the embedded list to Git's tracked-or-unignored files. `release.yml` builds the template for `x86_64-pc-windows-msvc` with `link.exe`. This machine has `llvm-cvtres` and `llvm-readobj` | `templates/basic/Cargo.toml`; `tools/cli/build.rs:54-90`; `tools/cli/src/new.rs:163-181`; `tools/cli/src/tests/new.rs:131-163`; `.github/workflows/release.yml:111-157` |
| A35 | A file under `assets/` needs a manifest entry ("unlisted asset") and ships with the game; files outside `assets/` do neither | `scripts/check-repo.py:114-125`; `scripts/release.py:84` |
| A36 | `check_shell_copies` compares `src/shell/` only. Example 01's theme and fonts equal the template's: its generator copies them, but only `just level-check` (Pillow) checks them. Its seven shell sounds are its own synthesis (44.1 kHz) with their own manifest entries | `scripts/check-repo.py:165-185`; `examples/01_platformer/tools/generate.py:15-20`, `:273-303`, `:322-330`, `:373-378`; `justfile:94-97` |
| A37 | **(adapted)** Budgets: `tokei` is installed; the shell is 2,393 of 2,400 lines of code and `src/` 2,890 of 2,900. Byte budgets used: `templates/basic/AGENTS.md` 12,270 of 12,288 (58 lines of 100), `docs/LLM_INDEX.md` 12,159 of 12,288, `crates/tungsten-render/AGENTS.md` 4,092 of 4,096, `AGENTS.md` 5,402 of 6,144 | `tokei templates/basic/src/shell -o json`; `scripts/check-agent-context.py:22-28` |
| A38 | **(adapted)** Example 01 has no fit module: the shell's layout tests run in the template on the same bytes. Its playthrough is `a_new_game_walks_the_lower_route_into_the_collection_pit` and its round trip is `a_capture_round_trips_through_a_load_with_a_hazard_and_a_platform_displaced` | `examples/01_platformer/tests/game/main.rs:9-10`; `playthrough.rs:267`; `saves.rs:147` |
| A39 | Item 14 covers 20 files, 431,878 bytes, `docs/perf/benchmarks.md` 93,816 of them. `just repo-check` checks IDs and local links in its `DOCS` list and the active plans; `just ctx` checks the four budgets | `scripts/check-repo.py:20-25`, `:286-305` |
| A40 | Captures: `TUNGSTEN_CAPTURE_*` works in any run, but smoke mode resolves no user folder. This machine has no `xdotool`, has `x11rb` 0.13.2 cached, and runs winit 0.30.13, which reads `WINIT_X11_SCALE_FACTOR`; the OS scale here is 1.75 | `crates/tungsten/src/app.rs:193-199`, `:389`, `:2173-2196`; `crates/tungsten/src/user_dir.rs:68-76` |

Step 9's audit of the save screen, read on the tree after step 8 and its four before-captures (`perf-runs/20261021-m73-template-final-pass/step09/{load,save}-{1280,floor}-before.png`). Each finding names the step 9 change it got.

| # | Finding | Evidence | Change |
| --- | --- | --- | --- |
| S1 | Hierarchy: a row is one centred mono string, so the slot's name weighs no more than its time and numbers | `src/shell/saves.rs:98`, `:227` | The name in the body family, the details a second label in mono, small and muted |
| S2 | Scanning: the details run on after the name, centred, and wrap mid-phrase ("Older / version 0.0.9" at 1280 × 720); nothing lines up from row to row | `saves.rs:98` | Names start at the left, details end at the right, one column each; a row is the details' two lines tall at most |
| S3 | Empty rows: in Load mode a disabled box as heavy as a ready one; in Save mode, enabled targets in full text, the loudest rows on the screen | `saves.rs:93`, `:110` | No fill, the name muted when enabled, one line ("Slot 3 … Empty") |
| S4 | Unreadable rows (damaged, newer) look like empty ones but carry a Delete, their one action; their state shows in the details colour while their name is disabled, so the details read louder | `saves.rs:101-102`, `:111-115` | The details take the disabled colour with the row |
| S5 | An older version's note is one more dot-separated item, the one that wraps | `saves.rs:98` | It stays on the details' first line, which no longer wraps at 1280 × 720 or the floor |
| S6 | Autosaves and slots are one list with no label; in Save mode the three autosaves lead the screen though none is a target, each with a red Delete | `saves.rs:104-108`, `:223-232` | "Autosaves" and "Slots" labels; Save mode hides the autosaves under a muted note ("Written as you play, when you pause and when you quit." or "Autosave is off.") |
| S7 | Every Delete is red at all times: up to fourteen danger words on one screen, none following focus | `saves.rs:230`; `widgets.rs:166-169` | Muted until its row's button or Delete itself has focus, then danger |
| S8 | Delete is as tall as its word, floating mid-row beside a two- or three-line row | `saves.rs:224`; `widgets.rs:210-212` (a row centres its children) | The row stretches Delete to its height, its word centred |
| S9 | Spacing keeps the theme's 8-unit gap and 16-unit padding. At the floor the card shows about six rows and scrolls; the title's version label shows beside it under the scrim, the title's and no fault of this screen | `saves.rs:224`; `src/shell/title.rs` (footer) | None: the floor still scrolls at about six rows, and 1280 × 720 still fits every row (a first draft that stacked the details under the name made it scroll, so they sit beside it) |
| S10 | Mouse and pad: a whole row is one button and one click target; focus walks rows, then Delete; a disabled row takes no focus. No core UI is missing for any change | `saves.rs:245-262` | None |

## 2. Design

### 2.1 Letter spacing on UI text (step 1, the API)

```rust
// crates/tungsten-core/src/ui/theme.rs
pub struct TextOverrides {
    // … the nine fields as now …
    /// Space added after each glyph, in em, as `TextLayout::letter_spacing`
    /// takes it; passes down the tree with the other text properties. A
    /// value that is not finite is ignored.
    pub letter_spacing: Option<f32>,
}
// `apply` writes `style.layout.letter_spacing` for a finite value.
```

- **The theme file.** A kind's or a state's `text` takes `"letter_spacing": <number>`; `deny_unknown_fields` still refuses other names.
- **The debug views** set `Some(0.0)` in `Look::text`, so a game theme's spacing never reaches them (M58's isolation, A30).
- **Relayout.** A change re-lays its text, since `text_measures_differently` already compares layouts.
- **The break.** Adding a field breaks a struct literal that names every field. The ledger row says so, and the workspace's two such literals set the field. This is a decision amending `D-125`'s style model: letter spacing joins the inherited text properties.

### 2.2 The death loop's grace (step 3)

On a continued run, `GameplayState::on_enter` applies `spawn_grace` to the player once it spawns: `EffectRequest::new(player, DataRef::by_id("spawn_grace")).apply(world)`.

- `spawn_grace` is `{ "duration": 2.5, "grant": ["status.spawning"] }`.
- The enemies' `contact_damage` filters gain `"none": ["status.spawning"]`, so for 2.5 s nothing touches the player: no damage, no push, no slow.
- 2.5 s, not 2.0: the hazard is back at its start at exactly 2 s (Q1).
- Fresh runs take touches at once, as the sample's hazard tests expect.

It covers every save, those already on disk included, because every load spawns through `on_enter` (A3). It stays outside `src/shell/` and needs no kit API (Q2).

### 2.3 Settings rows (steps 4, 5, 6)

- **Toggles.** The `Activated` arm marks the rows stale, and one `refresh` runs after the event loop (A10).
- **Frame rate.**
  - `Row::Uncapped` replaces `Row::Unlimited`, captioned "Uncapped", and `FPS[0]` is 60.
  - A stored cap below 60 still applies as stored, while the thumb and the readout read 60 until the player moves the slider (Q4).
  - A cap off the 5-fps grid above the floor still reads as stored.
- **UI scale.** `theme::UI_SCALE` is 0.75: the scale an unset setting applies as.
  - `ShellPlugin::build` sets it on `ScaleFactor` when the setting is unset, and `shell::apply_settings` sets it again after the engine's apply.
  - The slider reads `ui_scale().unwrap_or(theme::UI_SCALE)`.
  - The setting stays unset until the player moves the slider, so Reset returns to 75 % and stored files keep their values.
  - `REFERENCE_HEIGHT` stays.
- **Slider.**
  - The normal state's border is `#0b0c10` at 1 unit, so the thumb's outline parts it from its fill and the track; focused keeps `#ffd36b` at 2.
  - `theme::SLIDER_LINE` (18, Q6) is the line height `slider_row` gives a slider, which makes the thumb 18 units square on a 4.5-unit track. It grows with the text scale.

### 2.4 Autosaves (step 7)

**Slots.** `AUTOSAVES = ["autosave_1", "autosave_2", "autosave_3"]`; `SLOTS` is those three, then `slot_1` to `slot_10`. `slot_name` gives "Autosave 1" to "Autosave 3" (Q7).

**Which slot a save writes.** Every automatic and requested save picks its slot at each write (Q9):
- **Coalesced.** When this run's last automatic save wrote a slot and no fixed step has run since, the save rewrites that slot. Pause, Quit Game's question and the exit's last frame (A2) then land in one slot instead of taking all three. `RunState` keeps the run's last automatic slot and a flag that `shell_checkpoint` sets on each fixed step.
- **Otherwise** it takes the first empty writable slot, else the oldest writable by `saved_at_ms`. Equal times go to the slot this program wrote first, then to the lower number.
- **Read-only slots** are skipped. With none writable, the write fails and logs.

`write` keeps setting `retry_from`.

**A 0.93 save.** At the shell's start, a `saves/autosave.json` from 0.93 is renamed to the first free `autosave_N`; `std::fs::rename` keeps its data and its time (A21). If the rename fails, for instance in a read-only folder (a tested case, `tests/game/shell.rs:431`), the file stays listed among the autosaves under its old name: read-only, loadable, deletable and never written. A readable 0.93 save never disappears (Q8).

**The Autosave option.** `GameOptions.autosave` defaults to on and is the Gameplay tab's "Autosave" toggle. With it off:
- no save is written at run start, pause, quit, the exit or `request_autosave`;
- a due autosave is dropped, not kept for later;
- quitting with a live run names lost progress, as a failed autosave does today (Q10).

`request_autosave` keeps its contract while the option is on.

**The title.** Continue loads the newest readable slot of every one listed. New Game asks "Start a new game? Its autosaves will replace the oldest ones." only when Autosave is on and an autosave is ready.

**The Load screen.** It lists the autosaves newest first (the three, and a 0.93 file a failed rename left), then the slots. Rows map to slots in the order the last refresh drew them. In Save mode the autosaves are never a target; step 9 settles how that mode shows them (Q11).

### 2.5 Older saves (step 8)

`shell::older(saved, current)` compares the leading dot-separated numbers and ignores a pre-release suffix. A version that does not parse, such as `"unknown"` or an empty string, is older (Q13).

- **The row.** An older save's details name its version ("Older version 0.0.9", "Unknown version"); a save of this version or a newer one shows none.
- **The question.** Loading an older save from the Load screen or Continue asks first through the shell's confirm: "<name> was saved by an older version, <v>. Load it?" It adds "Progress since your last save will be lost." when a live run isn't checkpointed, so one question covers both.
- **No question.** Retry reloads without asking (Q12). A newer schema stays unloadable, as now.

### 2.6 The save screen (step 9)

The step audits the screen first (§3, step 9), then fixes it in the owner's direction:
- the name in the body family;
- details muted and smaller, the numbers in mono, as a second label inside the row's button (A20);
- empty rows quieter;
- the autosaves under a small "Autosaves" label and the player's slots under "Slots";
- Save mode leaving the autosaves out, with a muted note saying when they are written, or that Autosave is off (Q11);
- Delete in the muted colour until its row's button or Delete itself has focus, then in danger.

If a finding needs core UI that does not exist, the row keeps one text and the limit becomes a follow-up.

### 2.7 The post stack: Bloom and game over (steps 10, 11)

`shell_frame` calls one function that makes every shell edit to `PostStack`. The shell keeps:
- `kept`: the game's Bloom passes it took out, each with its index;
- `tail`: the passes it appended;
- `left`: the stack as it left it last frame.

Each frame, in order:
1. If the stack still equals `left`, drop `tail` from its end. Otherwise the game rebuilt it: clear `kept`, and drop `tail` only if the stack still ends with it.
2. With Bloom off, move every Bloom pass into `kept`; with Bloom on, put `kept` back at its indices.
3. Append the passes game over wants this frame as the new `tail`.
4. Record `left`.

At a change of run, the curtain's swap in `curtain()` comes before the replace. There the shell puts `kept` back at its indices, drops `tail`, and records the result as `left`, so the stack is the game's as the game built it. The next frame strips it again if Bloom is off.
- Equality alone cannot see a run that leaves: the sample's Bloom-only stack stripped to `[]` equals the empty stack its exit inserts, so `kept` would come back on the title.
- A game whose stack outlives its runs keeps its Bloom through off, a run change and on.
- The swap's frame draws under the opaque curtain, so the one frame with Bloom back never shows.

Effects:
- The game's passes survive, whatever the order of runs and screens.
- Game over's passes leave exactly when it does.
- No system is added, so the template's schedule snapshot holds.
- A game builds its stack whole when its run starts (the template's `AGENTS.md` and the guide say so, Q14).
- A stack the game changes while Bloom is off drops the passes the shell took.
- A stack the game sets to exactly what the shell left, its passes without Bloom, cannot be told apart from an untouched one, since passes carry no identity (A24). Its withheld Bloom comes back when the option turns on, and a test pins that (Q23).

The option is `GameOptions.bloom`, on by default: the Display tab's "Bloom" toggle, above Apply. Each tab's Reset resets its own rows only:
- the Display tab's resets Bloom;
- the Gameplay tab's, which today replaces every option and keeps only `mute_unfocused` (`src/shell/settings.rs:487-492`), keeps `bloom` too.

The sample's run inserts its own stack, one Bloom pass that starts at threshold 0.9, knee 0.1, intensity 2.0 and radius 1.0 (Q15), and resets to `PostStack::default()` when it leaves. A new game deletes it with the sample.

### 2.8 Game over (step 11)

Game over builds its own screen in place of `Menu::new`, which a child module may do (`Menu`'s private field is visible there):
- a clear modal root;
- a full-width band (black at alpha 200) holding "YOU DIED" in the serif family, at 64 on 72 units capped at 96, letter spacing 0.2 em (§2.1), in `theme::DIED_RED`, a deep crimson at 3:1 or more against the band in linear light (Q17);
- under the band, Retry, Load and Quit to Title, hidden.

**The entrance.**
- Over 1.6 s (QuadOut, real clock), the band and the heading fade in while the world pixelates to 48 px and dims under the old screen's crimson fade to 0.8 (A28), through §2.7's tail.
- The entrance is a screen change (`Change::Entrance`, ticked in `flow`), so screens wait and `settle` waits it out.
- When it ends, the buttons fade in over 0.3 s and Retry takes focus.
- The sting and the music start at once, as now (Q16).

### 2.9 Black play area, still enemy (steps 12, 13)

- **Clear colour.** `render.clear_color` becomes `[0.0, 0.0, 0.0, 1.0]`. The title's scrim then reads darker over black, the re-captured fixture records it, and the run curtain keeps the theme's background (Q18). Example 01 keeps its colour.
- **The still enemy.** `enemy_still` is `enemy_orbit` less `orbit`: kinematic, with the same sprite, spin, three hits, contact (with §2.2's filter) and return. It takes `ENEMIES[2]`'s place at `[1100, 380]` (Q19).

### 2.10 The Windows icon (step 14)

- **The script.** `tools/icon.py`, standard library only, reuses `sprites.py`'s shape sampler. It draws the gem's diamond on the theme's background at 16, 32, 48 and 256 px, each PNG-encoded, into two files under `windows/`:
  - `icon.ico`;
  - `icon.res`: one `RT_GROUP_ICON` named 1 over four `RT_ICON` entries, language neutral, no version strings.
  - Its `--check` compares decoded pixels with a fresh render, as `sprites.py`'s does, since deflate's bytes differ between zlib builds.
    - Compressed sizes move every PNG chunk length, ICO size and offset, `.res` data size and group byte count. The check holds each of these to the payload it describes, never to freshly written bytes.
    - The remaining fixed fields (types, IDs, counts, dimensions, language) equal a fresh render's.
  - `just script-test` runs it.
- **`build.rs`.** It prints `cargo:rerun-if-changed` for itself and the `.res`. It passes the `.res` with `cargo:rustc-link-arg-bins=` only when `CARGO_CFG_TARGET_OS` is `windows` and `CARGO_CFG_TARGET_ENV` is `msvc`, since `link.exe` takes a `.res` as an input. Other targets get nothing.
- **Placement.** Nothing lives under `assets/` (A35), and `tungsten new` copies `windows/` and `build.rs` like any other file (A34).

### 2.11 Example 01

| Item | Beyond the copy |
| --- | --- |
| 2, 4, 5, 8, 9 (steps 4, 8, 9) | Nothing: its tests name no toggle, cap, version or row text |
| 7 (step 5) | Nothing: its HUD sizes are relative |
| 8 (step 6) | Its theme copy, which its generator compares with the template's |
| 3 (step 7) | Its area entries keep `request_autosave`. `areas.rs`, `playthrough.rs` and `saves.rs` read the three slots, and its README says each area's entry fills the oldest autosave |
| 6 (step 10) | Its `[Bloom, Vignette]` stack follows the option: off leaves `[Vignette]`. A test pins it, and the README names the option |
| 10 (step 11) | Its bloom and vignette survive game over and Retry (a test) |

### 2.12 Live captures

Smoke mode has no user folder (A40), so screens that need saves, settings or a run are captured from a live binary:
1. **Set up.** Work from the game's folder, with `TUNGSTEN_USER_DIR=<scratch>/u-<name>` seeded with the saves the screen needs, `TUNGSTEN_CAPTURE_RESOLUTION=1280x720`, and the window found with `xwininfo -root -tree`.
2. **Drive it.** Send keys through a scratch XTest sender: a crate outside the tree on the cached `x11rb` 0.13.2 with its `xtest` feature, built offline into `target/zz-xkeys` and deleted afterwards. Wait about 150 ms between keys and 1 s before a shot.
3. **Shoot.** `import -window <id> <png>`, then `kill` the process.

**Sizes.**
- "1280 × 720" is that window at OS scale 1 (`WINIT_X11_SCALE_FACTOR=1`) and the default UI scale.
- "The floor" is the same window at this machine's 1.75 with `settings/settings.json` holding `ui_scale` 1.0, which lays it out at 731 × 411 units. Each capture names its scales.

**Storage.** Files go under `perf-runs/<YYYYMMDD>-m73-template-final-pass/<step>/`, which Git ignores. The evidence log lists their paths.

**When it fails.** If the sender or the window fails, the step records its captures as not run with the error line; the step's other checks decide it.

## 3. Steps

Every code step's done-when also has:
- `cargo fmt --all -- --check` → exit 0;
- `cargo clippy -p <each crate it touched> --all-targets --locked -- -D warnings` → exit 0;
- tokei's two counts (`tokei templates/basic/src/shell -o json | jq '.Total.code'`, then the same for `templates/basic/src`) in the evidence row.

A shell step's done-when also has:
- `cargo test -p tungsten-template-basic --locked` and `cargo test -p example-01-platformer --locked` → exit 0;
- `just repo-check` → exit 0.

A bug's test runs before its fix, and its failure is quoted. Tests are named as `--test game` runs them (`<module>::<name>`).

A template-only step (3, 12, 13, 14) proves example 01 unchanged by **example 01's tree hash**, taken before and after the step and equal:

```bash
python3 -B -c "import hashlib, pathlib; h = hashlib.sha256(); [h.update(p.as_posix().encode() + p.read_bytes()) for p in sorted(pathlib.Path('examples/01_platformer').rglob('*')) if p.is_file() and 'target' not in p.parts]; print(h.hexdigest())"
```

It compares with the tree before the step, not with `HEAD`, since earlier steps change example 01 and nothing is committed until the release (`D-105`).

### Step 1: Letter spacing on UI text

- **Files:** `crates/tungsten-core/src/ui/theme.rs`, `ui/theme_file.rs`, `src/tests/ui/{style,theme_file,layout}.rs`; `crates/tungsten/src/debug_views.rs`, `src/tests/debug_views.rs`; `api/`; `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`.
- **Change:**
  - §2.1, with rustdoc on the field.
  - Decision 198 through tungsten-decision: amends `D-125`, with its marker line and both index rows.
  - The ledger row: "`TextOverrides` gains `letter_spacing`", landed citing decision 198, release 0.94.
  - Tests:
    - `style.rs`: a button's spacing reaches its label, and a label's own spacing beats it.
    - `theme_file.rs`: `letter_spacing` maps onto the overrides, and an unknown text field still fails.
    - `layout.rs`: a label with 0.2 em lays out wider by `0.2 × size` per glyph, and a non-finite value is ignored.
    - `debug_views.rs`: the hostile theme's 0.5 em does not reach the views, and the assertion compares the resolved layout.
  - Then `just api`.
- **Done-when:**
  - `cargo test -p tungsten-core --locked` → exit 0.
  - `cargo test -p tungsten --locked --lib -- debug_views` → exit 0.
  - `rg -c 'TextOverrides::letter_spacing' api/tungsten-core.txt` → 2. The baseline is 0; the field is listed once per path.
  - `just api-check` → exit 0.
  - `rg -c '^\| `D-19[8]` ' docs/DECISION_INDEX.md` → 1.
  - `rg -c '^\*\*Amended by D-19[8]:' DECISIONS.md` → 1, and `rg -c 'Amended by `D-19[8]`' docs/DECISION_INDEX.md` → 1.
  - `rg -c '^\| `TextOverrides` gains `letter_spacing`' docs/plans/1.0/w04-api-freeze.md` → 1.
  - `cargo clippy --workspace --all-targets --locked -- -D warnings` → exit 0.
  - `just repo-check` → exit 0.
- **Then:** the level C API critique, as the skill's Running steps prescribe, in the background, recorded in §9 as `C.<n>`. Where a finding holds, step 1 changes before step 11 uses the field. Steps 2–10 do not depend on it and may run meanwhile.
- **Moves:** `api/tungsten-core.txt` (two lines).

### Step 2: The shell-copy check covers the theme

- **Files:** `scripts/check-repo.py`, `scripts/test-check-repo.py`.
- **Change:**
  - `check_shell_copies` also compares `assets/ui/themes/shell.json` with the template's for every example that has `src/shell/`. Two errors: "`<example>/assets/ui/themes/shell.json`: missing; templates/basic has it" and "`…`: differs from templates/basic/assets/ui/themes/shell.json".
  - Its test gains an equal theme, a differing one and a missing one, each with its exact error.
  - Example 01's `README.md` line 5 adds "and its theme" (the same step, since how a game uses the shell changed).
- **Done-when:**
  - `python3 -B scripts/test-check-repo.py` → exit 0.
  - `just repo-check` → exit 0, since the themes are equal today.
  - `rg -c 'themes/shell.json' scripts/check-repo.py` → at least 1. The baseline is 0.
- **Moves:** none.

### Step 3: The death loop

- **Files:** step 3's list in the header.
- **Change, test first.** `saves::a_continued_run_survives_its_first_two_seconds` replaces A7's test.
  - **The saves.** Each is built by `save::capture` in a Normal run at 1 of 5 health, with its `player` moved onto one of A6's three places: `HAZARD_START` plus `[1, 0]`, `[1164, 380]` and `[640, 80]`. It is written to the autosave and to `slot_1` with `SaveSlots::save`.
  - **The ways in.** Continue from the title, Load from the title's Load screen, Load from pause over a live run, and Retry after a death in the loaded run.
  - **The check.** Each continued run runs 120 frames at 1/60 with no input; the player is alive and gameplay on top.
  - **Then** the fix, §2.2.
- **Done-when:**
  - Before the fix, `cargo test -p tungsten-template-basic --locked --test game -- saves::a_continued_run_survives_its_first_two_seconds` → exit 101, quoted. After it → exit 0.
  - `cargo test -p tungsten-template-basic --locked` → exit 0; the sample's fresh-run hazard tests still take the touch at once.
  - `cargo test -p tungsten-cli --locked` → exit 0, since a template file was added.
  - `just repo-check` → exit 0, with `spawn_grace.json` covered.
  - `rg -l '"none": \["status.spawning"\]' templates/basic/assets/prefabs` → `hazard.json`, `enemy_orbit.json` and `enemy_patrol.json`.
  - Example 01's tree hash → equal before and after the step.
- **Moves:** none. The kit hash and every recipe are the kit's own, and they hold.

### Step 4: Toggles take their first click; Uncapped at 60 fps

- **Files:** `src/shell/settings.rs` and its copy; `tests/game/{main,settings}.rs`; `templates/basic/README.md` ("a frame-rate limit from 60 fps").
- **Change, test first.** `settings::every_toggle_takes_its_first_click` uses a `click(harness, name)` helper in `main.rs` that finds a control by name, calls `UiHarness::click` and steps a frame.
  - It opens Settings from the title and from pause.
  - For every toggle on each tab, it clicks once right after Settings opens and once right after a switch to its tab, and presses `ui_accept` once. It finds the toggles in the tree by their kind (`WidgetKind::Toggle`, five today), since `ROWS` is private to the shell, so steps 7's and 10's toggles stay covered.
  - Each time, the box and the applied value both flip, and a second click flips both back.
  - **Then** the fix and the frame-rate rows, §2.3.
  - **Frame-rate tests:**
    - The Unlimited test is renamed `the_frame_rate_limit_is_a_slider_and_an_uncapped_toggle`.
    - `the_frame_rate_limit_stops_at_thirty` becomes `…_stops_at_sixty`: two `ui_right`, then ten `ui_left`, gives `fps(60)` and "60 fps".
    - The new `a_stored_cap_under_sixty_applies_and_reads_sixty` starts from a settings file holding 30. After a restart `Settings` and `DisplayState` hold 30, Uncapped is off, and the slider and readout read 60. Back keeps 30, and `ui_right` gives 65.
- **Done-when:**
  - Before the fix, `cargo test -p tungsten-template-basic --locked --test game -- settings::every_toggle_takes_its_first_click` → exit 101 (VSync's box unchanged), quoted. After it → exit 0.
  - `rg -n -i 'unlimited' templates/basic/src templates/basic/tests examples/01_platformer/src templates/basic/README.md` → no match.
  - `rg -c 'FPS: \[f32; 3\] = \[60\.0' templates/basic/src/shell/settings.rs` → 1.
  - `cmp templates/basic/src/shell/settings.rs examples/01_platformer/src/shell/settings.rs` → exit 0.
- **Moves:** none.

### Step 5: The UI at 75 % by default

- **Files:** `src/shell/{theme,mod,settings}.rs` and their copies; `tests/game/{settings,fit}.rs`; `tests/fixtures/title.png`.
- **Change:**
  - §2.3's UI scale.
  - The new `settings::the_ui_scale_defaults_to_75_percent`:
    - on a fresh user folder the setting is unset;
    - the UI scale is 0.75 from the first frame, its factor equal to a harness set to 0.75;
    - Interface reads "75%", and one `ui_right` stores `Some(1.0)`;
    - Reset tab gives unset, 0.75 and "75%", and a restart keeps it unset.
  - The A14 tests follow the 0.75 default.
  - `fit.rs`'s `assert_fits` pins the UI scale at 1.0, so the 731 × 411 floor stays tested, and checks each screen at the default as well.
  - `title.png` is re-captured with the guide's command (A32). Record the old and new sha256.
- **Done-when:**
  - The crates' tests → exit 0.
  - `TUNGSTEN_VISUAL_REGRESSION=1 cargo test -p tungsten-template-basic --test game --locked -- visual:: --nocapture` → exit 0. This is a local GPU check.
  - `rg -c 'unwrap_or\(theme::UI_SCALE\)' templates/basic/src/shell/settings.rs` → 1.
- **Moves:** `tests/fixtures/title.png`.

### Step 6: A smaller, outlined slider thumb

- **Files:** `assets/ui/themes/shell.json` and its copy in `examples/01_platformer/assets/ui/themes/`; `src/shell/{theme,widgets}.rs` and their copies; `tests/game/settings.rs`.
- **Change:**
  - §2.3's slider.
  - The new `settings::sliders_have_a_smaller_outlined_thumb`, on the Interface tab:
    - unfocused, the slider's box is `SLIDER_LINE` tall at text 100 %, with a 1-unit `#0b0c10` border in its resolved style, which `paint.rs:972-980` draws on the thumb;
    - focused, the border is 2 units in the focus colour.
  - Live captures (§2.12) of the Interface tab, before and after, at 1280 × 720.
- **Done-when:**
  - The crates' tests → exit 0.
  - `cmp templates/basic/assets/ui/themes/shell.json examples/01_platformer/assets/ui/themes/shell.json` → exit 0, and `just repo-check` → exit 0 (step 2's check).
  - `PATH=<scratch>/level-venv/bin:$PATH just level-check` → exit 0. The venv comes from `python3 -m venv <scratch>/level-venv && <scratch>/level-venv/bin/pip install -r examples/01_platformer/tools/requirements.txt`; delete it with `shutil.rmtree` after step 15.
  - The two captures, in the evidence log.
- **Moves:** none. Core's slider is unchanged, so `examples/03_fixtures`' UI PNGs hold.

### Step 7: Three rotating autosaves and an Autosave option

- **Files:** `src/shell/{mod,saves,title,options,settings}.rs` and their copies; `tests/game/{saves,shell,settings}.rs`; `templates/basic/{AGENTS,README}.md`; `examples/01_platformer/tests/game/{areas,playthrough,saves}.rs`, `examples/01_platformer/README.md`.
- **Change:**
  - §2.4.
  - The template's `AGENTS.md` "Saves and options" line names the rotation and the option, byte-neutral (cut words in the same file).
  - New tests:
    - `saves::autosaves_rotate_through_three_slots_overwriting_the_oldest`: four autosaves of moved runs fill 1, 2 and 3, then 1 again, and `retry_from` follows.
    - `saves::an_autosave_fills_an_empty_slot_then_the_oldest`: after a deletion and a restart, slots at (empty, t 300, t 100) take writes in slots 1, then 3.
    - `shell::pause_then_quit_game_keeps_the_earlier_autosaves`: two autosaves of different states, then pause, Quit Game and its confirm through the exit's last frame. The three writes land in one slot, and the other two still hold the earlier saves.
    - `saves::the_load_screen_lists_three_autosaves_newest_first`.
    - `saves::a_093_autosave_becomes_one_of_the_three`: the rename keeps its `saved_at_ms`, and Continue loads it.
    - `saves::a_093_autosave_in_a_read_only_folder_stays_listed` (Unix only, as the read-only tests are): the rename fails, and the file is listed and loads.
    - `shell::autosave_off_writes_no_automatic_or_requested_save`, covering run start, pause, quit, the exit and `request_autosave`, then a pause after the option comes back on.
  - A22's tests follow the rotation, and `a_requested_autosave_waits_for_a_capture` keeps its meaning.
  - Example 01's three tests read the three slots, the newest by `saved_at_ms` or the one that changed. Its README's areas row and Saves line follow.
- **Done-when:**
  - The crates' tests → exit 0.
  - `rg -n '"autosave"' templates/basic/src/shell` → only the 0.93 name, and `rg -n '"autosave"' examples/01_platformer/tests` → no match.
  - `just ctx` → exit 0.
  - The shell copies `cmp`-equal.
- **Moves:** the shell's slot names, which a save folder from 0.93 reaches through the rename.

### Step 8: A question before an older save loads

- **Files:** `src/shell/{mod,saves,title}.rs` and their copies; `tests/game/{saves,shell}.rs`.
- **Change:**
  - §2.5.
  - Tests:
    - `saves::older_versions_compare_by_their_numbers`: `0.9.0 < 0.10.0`, equal is not older, `unknown` and `""` are older, and a pre-release suffix is ignored.
    - `saves::an_older_save_names_its_version_and_asks_before_it_loads`: a slot at `0.0.1`, and a schema-1 autosave that migrates to `unknown`. Cancel stays on the screen, and Load starts the run from the save.
    - `shell::continue_asks_before_an_older_save`.
    - `shell::retry_reloads_an_older_save_without_asking`.
    - `saves::a_save_of_this_version_loads_without_asking`.
- **Done-when:** the crates' tests → exit 0; the copies `cmp`-equal.
- **Moves:** none.

### Step 9: The save screen's look

- **Files:** `src/shell/{saves,widgets,theme}.rs` and their copies; `assets/ui/themes/shell.json` and its copy, only if a finding needs a theme value; `tests/game/{saves,fit}.rs`.
- **Change:**
  1. **The before-captures** (§2.12): Load from the title and Save from pause, each at 1280 × 720 and at the floor. The saves are seeded so every row state shows: autosaves, a ready slot, an older-version one, an empty one, a damaged one and a newer schema's.
  2. **The audit.** It covers the hierarchy (the name against its details), scanning, how the empty, unreadable and older-version rows read, the autosaves' group, Delete's weight and focus, spacing against the theme, and the screen under a mouse and under a pad. Each finding goes into this plan as an `S<n>` row under §1 at `file:line`.
  3. **The fixes**, §2.6.
  4. **The after-captures**, same four.
  - Tests: the saves tests read the name label and the new `details` helper the second. New tests:
    - `saves::the_load_screen_groups_autosaves_and_slots`;
    - `saves::delete_is_quiet_until_its_row_has_focus`;
    - `saves::an_empty_row_reads_quieter`.
- **Done-when:**
  - The crates' tests → exit 0, `fit::the_saves_screen_fits_in_both_modes` among them.
  - The `S` rows are in §1.
  - Eight captures are in the evidence log.
  - The copies `cmp`-equal.
- **Moves:** none.

### Step 10: Bloom

- **Files:** `src/shell/{mod,options,settings}.rs` and their copies; `src/sample/mod.rs`; `tests/game/{settings,sample}.rs`; `templates/basic/AGENTS.md`; `examples/01_platformer/tests/game/presentation.rs`, `examples/01_platformer/README.md`.
- **Change:**
  - §2.7.
  - The template's `AGENTS.md` drawing rule gains "a run builds its `PostStack` whole; the shell strips its Bloom while the option is off", byte-neutral.
  - Tests:
    - `settings::the_bloom_option_strips_and_restores_the_games_bloom`, on a test stack `[Bloom(p), Vignette(v)]`: off gives `[Vignette(v)]`; on gives exactly the original; a stack inserted while off is stripped again; the Display tab's Reset turns Bloom on; and the option survives a restart.
    - `settings::bloom_off_then_a_run_change_restores_nothing_the_game_dropped`: in the sample's run with Bloom off, Quit to Title, then Bloom on. The title's stack stays empty.
    - `settings::a_stack_that_outlives_its_run_keeps_its_bloom`: inserted once, it goes through Bloom off, a run change and Bloom on, and comes back exactly.
    - `settings::a_stack_set_to_its_stripped_value_keeps_its_withheld_bloom`, which pins Q23's rule.
    - `settings::each_tabs_reset_keeps_the_other_tabs_options`: with Bloom and Autosave off, Gameplay's Reset turns Autosave on and leaves Bloom off; Display's turns Bloom on and leaves Autosave off; Audio's keeps both.
    - `sample::the_runs_post_stack_blooms_below_white`: one Bloom pass with a threshold under 1.0 at run start, and the default stack after Quit to Title.
    - Example 01's `presentation::the_bloom_option_keeps_the_vignette`.
  - Tune the sample's values on live gameplay captures (§2.12, New Game on Normal, a bolt in flight) until the bolts, sparks and gems glow. Other full-channel pixels may bloom too: the speed pickup's body and the enemies' eye highlights, which `sprites.py:214` draws at (255, 246, 240) and which the threshold reads by their largest channel (`bloom_threshold.wgsl:45`). Record the final values and which sprites bloom.
- **Done-when:**
  - The crates' tests → exit 0, `sample::the_schedule_matches_the_snapshot` among them.
  - Gameplay captures with Bloom on and off are in the evidence log.
  - `just ctx` → exit 0.
  - The copies `cmp`-equal.
- **Moves:** none.

### Step 11: Game over

- **Files:** `src/shell/{game_over,theme,mod}.rs` and their copies; `tests/game/{shell,fit}.rs`; `templates/basic/README.md`; `examples/01_platformer/tests/game/presentation.rs`.
- **Change:**
  - §2.8.
  - Tests:
    - `shell::game_over_reads_you_died_on_a_crimson_band`: the heading's text is "YOU DIED"; its resolved colour is `DIED_RED` and its resolved letter spacing is 0.2; the band is as wide as the root. It replaces A31's first test.
    - `shell::game_overs_buttons_wait_for_its_entrance`: no focusable Retry during the entrance, and Retry focused after `settle`.
    - `shell::game_over_pixelates_and_dims_and_removes_exactly_its_passes`. On a test stack it ends with Pixelate(48) and Fade(0.8). It holds its length across frames and across a Load push and back. After Retry, none of its passes are left.
    - `fit.rs`'s `assert_rules` also asserts "YOU DIED" stays on one line.
    - Example 01's `presentation::bloom_and_vignette_survive_game_over_and_retry`.
  - The crimson's contrast against the band, composited in linear light over the dimmed world, goes in the evidence row (3:1 or more).
  - Live captures of game over, mid-entrance and settled, at 1280 × 720 and the floor.
  - **If step 1 was skipped,** the heading draws without letter spacing, the spacing assertion drops, and a follow-up records the limit.
- **Done-when:**
  - The crates' tests → exit 0.
  - The contrast and the captures are in the evidence log.
  - The copies `cmp`-equal.
- **Moves:** none.

### Step 12: A black play area

- **Files:** `templates/basic/tungsten.json`, `tests/fixtures/title.png`.
- **Change:** §2.9's clear colour; `title.png` re-captured (A32), with its sha256 recorded.
- **Done-when:**
  - `python3 -B -c 'import json; print(json.load(open("templates/basic/tungsten.json"))["render"]["clear_color"] == [0, 0, 0, 1])'` → `True`.
  - `TUNGSTEN_VISUAL_REGRESSION=1 cargo test -p tungsten-template-basic --test game --locked -- visual:: --nocapture` → exit 0.
  - Example 01's tree hash → equal before and after the step.
- **Moves:** `tests/fixtures/title.png`.

### Step 13: A still enemy

- **Files:** step 13's list in the header.
- **Change:**
  - §2.9's enemy, and the sample's doc comments ("three that move and one that stands").
  - `sample::enemies_move_take_three_hits_and_come_back`: the still enemy stays at its place for 2 s, falls to three hits and returns there 4 s later, while the other three move.
- **Done-when:**
  - `cargo test -p tungsten-template-basic --locked` → exit 0.
  - `cargo test -p tungsten-cli --locked` → exit 0.
  - `just repo-check` → exit 0.
  - Example 01's tree hash → equal before and after the step.
- **Moves:** none.

### Step 14: A Windows icon

- **Files:** step 14's list in the header.
- **Change:**
  - §2.10.
  - The tools README section.
  - The `justfile`'s `script-test` runs `python3 -B templates/basic/tools/icon.py --check`.
  - `w12-template.md` gains a follow-up listing the owner's three: example 01's icon, the archive launchers' icons (players click copies of `tungsten-launcher`, `D-196`), and a runtime window icon (winit's `set_window_icon`).
- **Done-when:**
  - `python3 -B templates/basic/tools/icon.py --check` → exit 0.
  - `llvm-cvtres /MACHINE:X64 /OUT:<scratch>/icon.obj templates/basic/windows/icon.res` → exit 0, and `llvm-readobj --coff-resources <scratch>/icon.obj` lists one `GROUP_ICON (ID 14)` named 1 and four `ICON (ID 3)` entries named 1–4.
  - `llvm-rc /FO <scratch>/ref.res` on a one-line `1 ICON "icon.ico"` beside a copy of `icon.ico`, then the same `llvm-cvtres` and `llvm-readobj` → the same types, names and data sizes as the script's `.res`. That shows `llvm-rc`, Microsoft's format reference here, agrees with the script.
  - `cargo build -p tungsten-template-basic --locked` → exit 0 (Linux: the script links nothing).
  - `cargo test -p tungsten-cli --locked` → exit 0.
  - `just script-test` → exit 0.
  - `just template-check` → exit 0 (6 GiB floor).
  - Example 01's tree hash → equal before and after the step.
- **Owner-only:** the release's Windows build shows the icon on `data/bin/<level>/Template.exe`.
- **Moves:** none.

### Step 15: Example 01 follows the template

- **Files:** none, unless a check names one.
- **Change:** run the checks; a failure is a stop condition, not a fix here.
- **Done-when:**
  - `just smoke` → exit 0, example 01's row running from its folder.
  - `cargo test -p example-01-platformer --locked --test game -- playthrough::a_new_game_walks_the_lower_route_into_the_collection_pit saves::a_capture_round_trips_through_a_load_with_a_hazard_and_a_platform_displaced` → exit 0, 2 passed.
  - `just repo-check` → exit 0, with both copies checked.
  - `PATH=<scratch>/level-venv/bin:$PATH just level-check` → exit 0.
  - Live captures of example 01 (§2.12) are in the evidence log: game over after the courier walks into the first spikes, Settings' Display tab, and the Load screen after a run's first autosave, each at 1280 × 720.
- **Moves:** none.

### Step 16: Docs, part 1: the pass's decision and the engine's docs

- **Files:** `DECISIONS.md`, `docs/DECISION_INDEX.md`, `README.md`, `AGENTS.md`, `docs/*.md`, `docs/perf/*.md`, `crates/tungsten-render/AGENTS.md`, `DESIGN.md` (Core UI Model; Template Shell and Exit).
- **Change:**
  1. **The decision.** Decision 199 through tungsten-decision records every behaviour §2 changed and the theme check. It amends `D-181`, `D-187`, `D-189`, `D-191`, `D-192`, `D-194` and `D-197`, each with its marker line and index note. It sets the caps from tokei's counts after step 14 (Q22). Re-check the next free ID and `git status` first.
  2. **The docs.** Then read each file whole and update it to the engine, the template and example 01 as they stand, in a shape an agent parses:
     - greppable headings;
     - task-to-file tables;
     - each fact in one place, linked elsewhere;
     - commands in code blocks;
     - no stale versions or paths;
     - the README version-free.
  3. **Kept as they are** (Q21): `DECISION_INDEX.md`'s rows stay their decisions' summaries, and `benchmarks.md`'s dated measurements stay as recorded.
  4. **The guide** gains:
     - the 75 % default;
     - the rotation and the Autosave option;
     - the older-version question;
     - the Bloom option and the whole-stack rule;
     - the sample's grace;
     - the icon and `build.rs`.
- **Done-when:**
  - `rg -c '^\| `D-19[9]` ' docs/DECISION_INDEX.md` → 1.
  - `rg -c '^\*\*Amended by D-19[9]:' DECISIONS.md` → 7, and `rg -c 'Amended by `D-19[9]`' docs/DECISION_INDEX.md` → 7.
  - `rg -n 'eleven slots|autosave and ten slots|from 30 fps|Unlimited' README.md AGENTS.md DESIGN.md docs --glob '!docs/plans/**' --glob '!docs/DECISION_INDEX.md'` → no match. The baseline is `DESIGN.md:611` and `docs/getting-started.md:85`.
  - `just ctx` → exit 0.
  - `just repo-check` → exit 0.
  - One evidence line per file read: its bytes before and after.
- **Moves:** none.

### Step 17: Docs, part 2: the template's and example 01's docs

- **Files:** step 17's list in the header.
- **Change:**
  - The same pass as step 16 over each file, read whole.
  - The template's `AGENTS.md` takes decision 199's caps and keeps its prefab and kit lists current (`check_kit_catalogue`), within 12,288 B and 100 lines.
  - Its `README.md` describes the shell and sample as they stand.
- **Done-when:**
  - `just ctx` → exit 0, with `templates/basic/AGENTS.md` at or under 12,288 B and under 100 lines.
  - `just repo-check` → exit 0.
  - `rg -n 'an autosave and ten slots|the .autosave. slot|You died|from 30 fps|2,400|2,900' templates/basic/*.md templates/basic/tools/README.md examples/01_platformer/README.md` → no match. The baseline is `templates/basic/README.md:5` and `templates/basic/AGENTS.md:13` and `:19`; steps 4, 7 and 11 clear part of it.
- **Moves:** none.

### Step 18: Release 0.94

- **Files:** `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`; the register row and the README's four owned Now fields (`roadmap.py summary`); `docs/plans/1.0/w12-template.md` (a "Landed in 0.94" line); `docs/known-issues.md` (what this pass found out of scope); this plan, moved to `docs/plans/archive/1.0/`.
- **Change:**
  1. Close workflow §5 and this step's evidence row with `status: done`.
  2. Run [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md), then [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)):
     1. archive this plan;
     2. `just release-cut 0.94.0`, once;
     3. every release check once after the cut, with `just visual` (five targets, five `test result: ok` lines; the `fade` error line is the known noise) and `just template-check`;
     4. the preflight with `--message`.
- **Done-when:**
  - [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut).
  - `just release-preflight 0.94.0 --repo JosephGibson/Tungsten --message 'Update 0.94: M73 template final pass'` → passes and prints the command block and the post-merge block.
- **Ends with:** the changed-file list, the command block, the post-merge block, then the plan prompt for 01 (b) elements and weapons (M74).

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | How long and on which runs is the grace? | 2.5 s, on continued runs only, with no visual cue. The hazard is back at its start at exactly 2 s, so a 2.0 s grace dies at 2.0 s |
| Q2 | A guard in `capture`, kit API, or a grace from data? | The grace from kit data in the sample's spawn. A refused pause or quit capture would lose progress silently (A2); kit API would need a level C kit step; saving enemies would bump the schema and still reset their contact timers |
| Q3 | The toggle fix's shape? | One refresh after the event loop when any activation ran; `!idle` stays |
| Q4 | What does a stored cap below 60 show? | It applies as stored while the thumb and the readout read 60, until the slider moves (a left press at 60 sends nothing, A12). `D-191`'s "reads exactly" ends for caps under the floor; caps off the grid above it still read as stored |
| Q5 | The 75 % default: engine or shell? | The shell (`theme::UI_SCALE`). An engine key would be new public API that every game sets. The fit tests pin 100 % to keep the floor tested |
| Q6 | How small is the thumb? | `SLIDER_LINE` 18 at text 100 % (thumb 18, track 4.5), with a 1-unit `#0b0c10` outline. Under the 75 % UI it draws about 24 px at the owner's 1.75 scale, against 38.5 today. Core geometry stays (`D-162`) unless the captures show a track under 3 px at the floor; then step 6 stops and the change is a follow-up |
| Q7 | What are the autosaves called on screen? | "Autosave 1"–"Autosave 3" by file, listed newest first |
| Q8 | What happens to a 0.93 `autosave` file? | It is renamed at the shell's start to the first free `autosave_N`, keeping its time. A failed rename leaves it listed among the autosaves under its old name: read-only, loadable and deletable. A damaged or newer one stays read-only either way |
| Q9 | Which slot does an autosave write? | At each write: the run's last automatic slot again when no fixed step has run since, else the first empty writable slot, else the oldest writable by `saved_at_ms` (equal times go to the one this program wrote first, then to the lower number). Read-only slots are skipped |
| Q10 | What does Autosave off do? | No automatic or requested save, and a due one is dropped. Quitting a live run names lost progress. New Game asks only when Autosave is on and an autosave is ready |
| Q11 | How does Save mode show the autosaves? | It leaves them out, with one muted note saying when autosaves are written, or that Autosave is off. Load mode lists them under a label |
| Q12 | Which loads ask about an older version? | The Load screen and Continue. Retry does not, since the run already took the save. A newer version with a readable schema loads as now |
| Q13 | What counts as older? | The leading dot-separated numbers compare, and a pre-release suffix is ignored. Anything that does not parse is older |
| Q14 | Where does Bloom live? | `GameOptions.bloom` on the Display tab (engine `Settings` fields are engine API, `D-159`). The shell strips and restores the game's Bloom passes, and games build their stack whole |
| Q15 | What are the sample's bloom values? | Start at threshold 0.9, knee 0.1, intensity 2.0, radius 1.0, tuned on step 10's captures. The bolts, sparks and gems glow. The speed pickup's body and the enemies' eye highlights may glow as well, since no threshold separates their full channels; no sprite changes |
| Q16 | What is game over's timing? | A 1.6 s entrance that counts as a screen change, then the buttons over 0.3 s with Retry focused. The sting and the music play as now |
| Q17 | What are game over's type and colour? | Serif caps at 64 on 72, capped at 96, at 0.2 em. Deep crimson (`theme::DIED_RED`, starting `#be1420`) at 3:1 or more against a black band at alpha 200 in linear light (WCAG's large-text ratio). The world dims to 0.8 under the old crimson fade and pixelates to 48 px |
| Q18 | Does the title change with the black clear? | Only `tungsten.json` changes. The title's scrim reads darker (re-captured), the curtain keeps the theme's background, and example 01 keeps its colour |
| Q19 | What is the still enemy? | `enemy_orbit` less `orbit`: kinematic, with the same sprite and spin |
| Q20 | What is the icon's format? | Four PNG images (16, 32, 48 and 256 px) in `windows/icon.ico` and a `.res` that `link.exe` takes. `--check` compares decoded pixels and holds every compression-dependent length and offset to its payload (§2.10) |
| Q21 | How far does the docs pass reach? | Every file the owner listed, read whole. `DECISION_INDEX.md`'s rows stay their decisions' summaries (only this stop's rows and markers change), and `benchmarks.md`'s dated measurements stay as recorded |
| Q22 | Where are the new caps? | Each is the next hundred at or above 105 % of tokei's count after step 14 |
| Q23 | Should a mid-run stack equal to the stripped one discard the withheld Bloom? | No. Seeing it would take a rebuild call the game makes to the shell, a new name neither game needs: both build their stack once per run, and a run change restores. The guide's post-stack rule says a game that drops its Bloom mid-run while the option is off gets it back when the option turns on |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`).

## 5. Decisions expected

- **198:** letter spacing on UI text (`TextOverrides::letter_spacing`, the theme file's `text.letter_spacing`, the debug views pinned at 0); amends `D-125`. Written in step 1.
- **199:** the template final pass, everything §2 changes in the template and example 01, the theme copy check, and the caps; amends `D-181`, `D-187`, `D-189`, `D-191`, `D-192`, `D-194` and `D-197`. Written in step 16.

These are the next free IDs at `ac6dd96`. Each step checks the ID again before writing (another session may take one).

## 6. Invariants

- **May move:**
  - `templates/basic/tests/fixtures/title.png` (steps 5 and 12, sha256 in the evidence log and the `CHANGELOG.md` line);
  - `api/tungsten-core.txt` (step 1, two lines);
  - the shell's slot names (step 7; a 0.93 folder reaches them through the rename).
- **Must not move:**
  - the determinism hash, the pinned containment hash, the kit hash and the row digests;
  - `gpu-visual.png`, the post and transition regressions, and `examples/03_fixtures`' two UI fixture PNGs;
  - the template's schedule snapshot (no system is added);
  - example 01's generated outputs (`just level-check`);
  - both games' `SaveGame::SCHEMA` (3 and 4) and `GameOptions::SCHEMA` (1);
  - the seam's seven names and `shell::request_autosave`'s contract.

## 7. Stop conditions

- **A done-when check fails:** restore the step's files from their copies, mark the step skipped, and continue with steps that don't depend on it.
- **Dependencies between steps.**
  - Step 11 depends on step 10's post function. If step 10 is skipped, step 11 lands without the world's passes and records them as a follow-up.
  - If step 1 is skipped, step 11 drops the letter spacing (step 11's change).
  - Step 9 depends on steps 7 and 8. Step 15 runs whatever landed.
- **Example 01 fails in an unlisted file:** a shell step whose copy breaks one of example 01's tests outside the step's file list is skipped, with both copies restored.
- **Line caps:** past 3,000 lines of code in `src/shell/` or 3,500 in `src/` after any step, that step is skipped. A feature that large belongs in the acceptance game (`D-181`'s budget rule).
- **The template's `AGENTS.md` budget:** an edit that `D-183`'s budget cannot hold, after cuts elsewhere in the file, moves to the guide and is recorded.
- **Pillow:** if the venv cannot install, `just level-check` is recorded as not run, and step 2's check holds the theme copy.
- **Captures:** if live captures cannot run (§2.12), they are recorded as not run with the error; this alone skips no step.
- **A capture is running:** never write to the tree while `pgrep -af 'scripts/[b]ench.py'` shows one.
- **A finding needs the owner:** a step-9 finding that needs core UI becomes a follow-up (§2.6). Any other question the defaults don't answer is recorded and its step skipped.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 3–15, release | Both crates' tests, `just repo-check` and clippy per step; `just visual` in 5, 12 and the release; `just template-check` in 14 and the release; `just smoke` in 15 and the release; `just level-check` in 6 and 15. No perf row: no shared ECS, frame-loop or render path changes, and `TextOverrides::apply` gains one branch |
| 3 Invariants | 5, 12, release | `title.png` moves on purpose; the rest are checked by the release chain and `just visual` |
| 4 Decisions | 1, 16 | 198 and 199, their index rows in the same step |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1 | One field, its ledger row and rustdoc; the template's own names are not engine API |
| 6 Template and guide | every; 16 | The template is the subject; the guide in step 16 |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 16, 17 | New files sit under the index's `templates/basic/` row; `just ctx` in 7, 10, 16 and 17 |
| 8 Design | 16 | `DESIGN.md`'s Core UI Model (letter spacing) and Template Shell sections |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 14, release | Follow-ups in `w12-template.md` (14); known issues and the register at the release; no backlog row is cut and no gap row exists; `python3 -B scripts/roadmap.py summary` supplies the owned fields |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

**Round 1.** The `critique` skill with `--repo --effort max --force`, the plan cut of §9 and the evidence log with `sed`. Run 2026-10-21 on the tree at `ac6dd96` plus this plan's records. Footer: gpt-6-astra, effort max, context 872,000, output cap 4,000, 613.4 s, 2,022,702 tokens in (1,885,824 cached), 20,509 out, 45 commands, 358.6 KB output (largest 20 KB), 5h 0 % → 43 %, week 18 % → 25 %. Seven findings, all accepted.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] Step 3's gate rejects the death-loop fix once step 2 has run: step 2 edits example 01's README, so `git diff --quiet HEAD -- examples/01_platformer` fails | Step 2's change (example 01's `README.md` line 5); `D-105` (nothing is committed before the release) | accepted | §3's preamble defines example 01's tree hash, taken before and after a template-only step; steps 3, 12, 13 and 14 use it |
| 2 | [MEDIUM] One ordinary quit replaces all three autosaves with the same paused state: pause, Quit Game's question and the exit's last frame each autosave | `src/shell/pause.rs:58-61`; `src/shell/mod.rs:496-500`, `:563-566`; `src/sample/mod.rs:355-357` | accepted | §2.4 and Q9 coalesce: with no fixed step since the run's last automatic save, a save rewrites that slot. Step 7 adds `pause_then_quit_game_keeps_the_earlier_autosaves` |
| 3 | [MEDIUM] The rotation can overwrite a newer save while an older one remains, after a delete and a restart | §2.4's old rule; `src/shell/saves.rs:141-154` (delete) | accepted | Each write takes the first empty writable slot, else the oldest (Q9). Step 7 adds `an_autosave_fills_an_empty_slot_then_the_oldest` |
| 4 | [MEDIUM] A failed rename hides a readable 0.93 save | Q8's old default; `tests/game/shell.rs:431` (a read-only saves folder, tested); `crates/tungsten-core/src/save.rs:263-308` (a load only reads) | accepted | §2.4 and Q8: a file the rename could not move stays listed under its old name, read-only. Step 7 adds `a_093_autosave_in_a_read_only_folder_stays_listed` |
| 5 | [MEDIUM] Stack equality cannot see the sample's run leave while Bloom is off: both stacks are empty, so `kept` would come back on the title | §2.7's old algorithm; `crates/tungsten-core/src/post.rs:396-400` (the empty default); `examples/01_platformer/src/features/level/mod.rs:157-162` | accepted | §2.7: at a change of run, the shell restores `kept` and drops `tail` under the curtain, before the replace. Step 10 adds two run-change tests |
| 6 | [MEDIUM] Bloom tuning cannot keep the enemies' white eyes from glowing | `tools/sprites.py:208-216`; `crates/tungsten-render/src/shaders/stock/bloom_threshold.wgsl:45` (the largest channel) | accepted | Step 10 and Q15 drop "the enemies do not glow": eye highlights and the speed pickup may bloom, recorded, with no sprite change |
| 7 | [MEDIUM] The icon check's byte comparisons still depend on compression | `tools/sprites.py:100-106` (chunk lengths from compressed data) | accepted | §2.10 and Q20: the check holds each compression-dependent length and offset to its payload, and only the fixed fields to a fresh render |

A sweep of the fixes against the clauses that read the same state changed two more:
- step 4's test now finds the toggles in the tree by kind, since `ROWS` is private to the shell;
- §2.4's Continue and Load lines now count a 0.93 file that a failed rename left listed.

**Round 2.** Required, since round 1 changed done-when checks and steps' tests. The `critique` skill with `--repo --force`, at xhigh, on the revised plan cut the same way, with a focus note naming round 1's changed sections and no rejections. Run 2026-10-21. Footer: gpt-6-astra, effort xhigh, context 872,000, output cap 4,000, 300.4 s, 886,980 tokens in (786,432 cached), 8,483 out, 27 commands, 299.7 KB output (largest 23.2 KB), 5h 43 % → 65 %, week 25 % → 28 %. Two findings, both accepted. Both are gaps that round 1's Bloom fix left in state that other clauses read.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] Gameplay's Reset tab would also reset the Display tab's Bloom option | `src/shell/settings.rs:484-495` (Gameplay's Reset replaces every option and keeps only `mute_unfocused`) | accepted | §2.7: each tab's Reset resets its own rows only, and Gameplay's keeps `bloom`. Step 10 adds `each_tabs_reset_keeps_the_other_tabs_options` |
| 2.2 | [MEDIUM] Stack equality still misses a mid-run rebuild equal to the stripped stack, so a withheld Bloom comes back when the option turns on | `crates/tungsten-core/src/post.rs:396-400` (passes carry no identity); §2.7 | accepted | §2.7 states the rule and step 10 pins it with a test. A rebuild call is left out by Q23's default, since neither game rebuilds mid-run and a run change restores. There is no third round (the skill), so the default stands open for the owner |

**Step 1 API critique.** Level C's critique of step 1 (the skill's Running steps), in place of an owner review: the `critique` skill with `--raw --repo --force`, its `--from` the skill's diff of `api/` and `templates/` plus step 1's source and test diff (`crates/tungsten-core/src/ui`, `crates/tungsten-core/src/tests/ui`, `crates/tungsten/src/debug_views.rs` and its test), so the critic saw the implementation beside the two snapshot lines, 9,847 characters. Run 2026-10-21. Footer: gpt-6-astra, effort xhigh, context 872,000, output cap 4,000, 76.9 s, 230,601 tokens in (181,120 cached), 2,440 out, 12 commands, 154.9 KB output (largest 28 KB), 5h 65 % → 73 %, week 28 % → 29 %. No findings (`## No concerns`): it checked inheritance precedence, layout invalidation, the retained text's cache, the JSON mapping, the non-finite filter, the debug views' isolation and the ledger row. No `C.<n>` row; step 1 stands as landed.

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-21 | Done; the API critique found no concerns (§9) | `TextOverrides::letter_spacing` (rustdoc; `apply` writes a finite value) and the theme file's `text.letter_spacing`; the debug views' `Look::text` sets `Some(0.0)`. `cargo test -p tungsten-core --locked` → exit 0 (lib 1152 passed, the new `ui::style::tests::letter_spacing_inherits_and_a_labels_own_beats_it` and `ui::layout::tests::letter_spacing_widens_a_label_by_its_em_per_glyph_and_ignores_a_non_finite_value` among them; the layout test's first run failed on its own setup, a column root stretching the label to 400, and takes a row root). `cargo test -p tungsten --locked --lib -- debug_views` → exit 0, 37 passed. `rg -c 'TextOverrides::letter_spacing' api/tungsten-core.txt` → 2 (`just api` moved two lines, both `+`). `just api-check` → exit 0. `D-198` index row → 1, marker → 1, `Amended by \`D-198\`` → 1; ledger row → 1. `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `just repo-check` → exit 0. tokei: shell 2,393, `src/` 2,890 | `DECISIONS.md` (`D-198`), `docs/DECISION_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`, `api/tungsten-core.txt` |
| 2 | 2026-10-21 | Done | `check_shell_copies` compares each shell example's `assets/ui/themes/shell.json` with the template's: "`<example>/assets/ui/themes/shell.json`: missing; templates/basic has it" and "…: differs from templates/basic/assets/ui/themes/shell.json"; its test adds an equal, a differing and a missing theme. Example 01's README line 5 names the theme copy. `python3 -B scripts/test-check-repo.py` → exit 0 (54 tests). `just repo-check` → exit 0 (the themes are equal). `rg -c 'themes/shell.json' scripts/check-repo.py` → 2 (baseline 0). A theme path already selected `just repo-check` (asset coverage) | `scripts/check-repo.py`, `scripts/test-check-repo.py`, `examples/01_platformer/README.md` |
| 3 | 2026-10-21 | Done | `saves::a_continued_run_survives_its_first_two_seconds` replaces A7's test: three saves at 1 of 5 health (the hazard's line `[625, 504]`, `[1164, 380]`, `[640, 80]`), each through Continue, Retry after a death in that run, the title's Load and pause's Load, 120 frames at 1/60. Before the fix → exit 101, "Continue at [625.0, 504.0]: the player died" (`saves.rs:182`). After → exit 0, 1 passed. The fix: `spawn_grace` (`{ "duration": 2.5, "grant": ["status.spawning"] }`, listed in the manifest), the tag declared, the enemies' filters `"none": ["status.spawning"]`, and `GameplayState::on_enter` applies it to a continued run's player (`sample::SPAWN_GRACE`). `cargo test -p tungsten-template-basic --locked` → exit 0, 134 passed (the fresh-run hazard tests `sample::a_player_kept_on_the_hazard_takes_one_hit_per_window`, `…_walked_into_it_is_hit_once_and_pushed_away` and `a_hazard_hit_slows_the_player_for_its_effects_seconds` among them). `cargo test -p tungsten-cli --locked` → exit 0. `just repo-check` → exit 0 (the effect covered). `rg -l '"none": \["status.spawning"\]' templates/basic/assets/prefabs` → `hazard.json`, `enemy_orbit.json`, `enemy_patrol.json`. fmt, clippy (template) → exit 0. Example 01: its hash was not taken before the step; no file under it has an mtime after the step's backup (00:41:10), and `git status` shows only step 2's README; after the step `e9f0b64b…db97`. tokei: shell 2,393, `src/` 2,893 | `templates/basic/assets/data/effects/spawn_grace.json` (new), `assets/data/tags.json`, `assets/manifest.json`, `assets/prefabs/{hazard,enemy_orbit,enemy_patrol}.json`, `src/sample/mod.rs`, `tests/game/saves.rs` |
| 4 | 2026-10-21 | Done | `settings::every_toggle_takes_its_first_click` (a `click` helper in `main.rs` on `UiHarness::click`): from the title and from pause, each toggle found by `WidgetKind::Toggle` (five), clicked right after Settings opens or a switch to its tab, again after a switch away and back, and through `ui_accept`, each twice. Before the fix → exit 101, "VSync, a click right after Display shows, time 1 … left: (Some(false), true)" (the box unchanged, the value flipped). After → exit 0. The fix: the `Activated` arm marks the rows stale and one `refresh` runs after the event loop; Uncapped's `set` no longer refreshes. `Row::Uncapped` ("Uncapped") replaces Unlimited; `FPS = [60, 480, 5]`; a stored cap under 60 reads 60 (`max(FPS[0])`). Tests: `the_frame_rate_limit_is_a_slider_and_an_uncapped_toggle`, `the_frame_rate_limit_stops_at_sixty` (two `ui_right`, ten `ui_left` → `fps(60)`, "60 fps"), new `a_stored_cap_under_sixty_applies_and_reads_sixty` (30 stored and live, Uncapped off, slider and readout 60, Back keeps 30, `ui_right` → 65). `cargo test -p tungsten-template-basic --locked` → exit 0 (136 passed); `cargo test -p example-01-platformer --locked` → exit 0 (105 passed); `just repo-check` → exit 0; fmt, clippy (both crates) → exit 0. `rg -n -i 'unlimited' templates/basic/src templates/basic/tests examples/01_platformer/src templates/basic/README.md` → no match (exit 1); `rg -c 'FPS: \[f32; 3\] = \[60\.0' …/settings.rs` → 1; `cmp` of the two `settings.rs` → exit 0. README: "a frame-rate limit from 60 fps". tokei: shell 2,396, `src/` 2,896 | `src/shell/settings.rs` and its copy, `tests/game/{main,settings}.rs`, `templates/basic/README.md` |
| 5 | 2026-10-21 | Done | `theme::UI_SCALE` 0.75: `ShellPlugin::build` sets it on `ScaleFactor` when `Settings::ui_scale` is unset, `shell::apply_settings` again after the engine's apply, and the slider reads `ui_scale().unwrap_or(theme::UI_SCALE)` (`rg -c` → 1). New `settings::the_ui_scale_defaults_to_75_percent` (unset; 0.75 from the first frame, its factor equal to a harness set to 0.75; "75%"; one `ui_right` → `Some(1.0)`; Reset tab → unset, 0.75, "75%"; a restart keeps it unset). A14's tests follow the default (`every_slider_shows_its_value_as_it_moves`, `ui_scale_follows_the_window_height_with_a_floor`, `ui_and_text_scale_apply`). `fit.rs`'s `assert_fits` checks each screen at the UI scale 1 (the 731 × 411 floor) and at 0.75, and now bounds boxes by the viewport in units (it compared unit rects with pixel sizes, true only at factor 1; the 0.75 pass failed on that at first). `cargo test -p tungsten-template-basic --locked` → exit 0 (137 passed); `cargo test -p example-01-platformer --locked` → exit 0 (105); `just repo-check`, fmt, clippy (both crates) → exit 0; the three shell files `cmp`-equal. `title.png`: the old fixture failed at 0.75 (117,314 pixels, max delta 244); re-captured with the guide's command from `templates/basic` (exit 0), sha256 `304c9257…cd311f` → `9ac6dc5a…30052a`; `TUNGSTEN_VISUAL_REGRESSION=1 cargo test -p tungsten-template-basic --test game --locked -- visual:: --nocapture` → exit 0, 1 passed. tokei: shell 2,406, `src/` 2,906 | `src/shell/{theme,mod,settings}.rs` and copies, `tests/game/{settings,fit}.rs`, `tests/fixtures/title.png` |
| 6 | 2026-10-21 | Done | `shell.json`'s slider normal state gains `"border": "#0b0c10", "border_width": 1` (focused keeps `#ffd36b` at 2); `theme::SLIDER_LINE` 18, which `slider_row` sets as the slider's line height (thumb 18, track 4.5 at text 100 %). New `settings::sliders_have_a_smaller_outlined_thumb` (box 18 tall; resolved border `#0b0c10` at 1 unfocused, the focus colour at 2 focused) → passes. `cargo test -p tungsten-template-basic --locked` → exit 0 (138); `cargo test -p example-01-platformer --locked` → exit 0 (105); `cmp` of the two `shell.json` → exit 0; `just repo-check` → exit 0 (step 2's theme check); `PATH=<scratch>/level-venv/bin:$PATH just level-check` → exit 0 (Pillow 12.3.0, 15 tests); fmt, clippy (both crates) → exit 0; `theme.rs`, `widgets.rs` copies `cmp`-equal. Live captures (§2.12, the XTest sender built offline in `target/zz-xkeys`; window 1280 × 720 at `WINIT_X11_SCALE_FACTOR=1`, UI 75 %): the Interface tab before and after; the thumb drops from 16.5 to 13.5 px with a dark edge parting it from the fill (the 1-unit outline is 0.75 px there). Track 3.4 px at this size, 7.9 px at the floor, so Q6's 3 px limit holds. tokei: shell 2,410, `src/` 2,910 | `assets/ui/themes/shell.json` and copy, `src/shell/{theme,widgets}.rs` and copies, `tests/game/settings.rs`; captures `perf-runs/20261021-m73-template-final-pass/step06/interface-{before,after}.png` |
| 7 | 2026-10-21 | Done | `AUTOSAVES` (`autosave_1`–`autosave_3`) and `SLOTS` (13); `OLD_AUTOSAVE` is the only `"autosave"` left (`rg -n '"autosave"' templates/basic/src/shell` → `mod.rs:48` alone; in `examples/01_platformer/tests` → no match). Each autosave takes the run's last automatic slot while no fixed step has run since (one `RunState::last_autosave`, which `shell_checkpoint` clears on each step, in place of the slot and a separate flag), else the first empty writable slot, else the oldest by `saved_at_ms` (equal times: this program's write order, then the number); read-only slots are skipped. The shell's start renames a 0.93 `autosave.json` to the first free slot; one that stays is listed after the three. `GameOptions::autosave` (default on; schema 1 holds) is the Gameplay tab's Autosave toggle; off drops due and requested autosaves and names lost progress on quit. New Game asks "Start a new game? Its autosaves will replace the oldest ones." only with the option on and an autosave ready. The Load screen lists the autosaves newest first; rows map to the last refresh's order. New tests: `saves::autosaves_rotate_through_three_slots_overwriting_the_oldest`, `an_autosave_fills_an_empty_slot_then_the_oldest`, `the_load_screen_lists_three_autosaves_newest_first`, `a_093_autosave_becomes_one_of_the_three`, `a_093_autosave_in_a_read_only_folder_stays_listed`, `shell::pause_then_quit_game_keeps_the_earlier_autosaves`, `autosave_off_writes_no_automatic_or_requested_save`; A22's tests follow the rotation (four shell tests take one frame after `tap("state_pause")` for pause's own autosave before they remove its file; the read-only cases fill all three slots), the toggle test counts six, and `main.rs` gains `shown_modal`. Example 01's `areas`, `playthrough` and `saves` read the three slots. `cargo test -p tungsten-template-basic --locked` → exit 0 (145); `cargo test -p example-01-platformer --locked` → exit 0 (105); `just repo-check`, `just ctx` (template `AGENTS.md` 12,267 B, 58 lines) → exit 0; fmt, clippy → exit 0; five shell copies `cmp`-equal. tokei: shell 2,515, `src/` 3,015 (under the stop's 3,000 and 3,500) | `src/shell/{mod,saves,title,options,settings}.rs` and copies, `tests/game/{main,saves,shell,settings}.rs`, `templates/basic/{AGENTS,README}.md`, `examples/01_platformer/tests/game/{areas,playthrough,saves}.rs`, `examples/01_platformer/README.md` |
| 8 | 2026-10-21 | Done | `shell::older` (leading numbers compared, a `-`/`+` suffix dropped, an unparsed version older), `shell::version_note` (a row's "Older version 0.0.1" or "Unknown version"; this or a newer version shows none, so ready rows no longer print `v<version>`) and `shell::load`, which the Load screen and Continue call: one confirm, "<name> was saved by an older version, <v>. Load it?", with " Progress since your last save will be lost." when a live run is not checkpointed; Retry calls `start_run` with no question. New tests: `saves::older_versions_compare_by_their_numbers`, `saves::an_older_save_names_its_version_and_asks_before_it_loads` (slot at 0.0.1, schema-1 autosave → "unknown"; Cancel stays on Saves, Load starts at the save), `saves::a_save_of_this_version_loads_without_asking`, `shell::continue_asks_before_an_older_save`, `shell::retry_reloads_an_older_save_without_asking`; `a_schema_1_autosave_migrates` now answers Continue's question, and the load-screen test's rows lose ` · v0.1.0`. `cargo test -p tungsten-template-basic --locked` → exit 0 (150); `cargo test -p example-01-platformer --locked` → exit 0 (105); `just repo-check`, fmt, clippy → exit 0; the three shell copies `cmp`-equal. tokei: shell 2,554, `src/` 3,054 | `src/shell/{mod,saves,title}.rs` and copies, `tests/game/{saves,shell}.rs` |
| 9 | 2026-10-21 | Done | Before-captures of Load (title) and Save (pause), each at 1280 × 720 (`WINIT_X11_SCALE_FACTOR=1`, UI 75 %) and the floor (OS 1.75, `ui_scale` 1.0), the user folder seeded with two ready autosaves, a ready slot, a 0.0.9 slot, empty, damaged and schema-9 slots (`<scratch>/seed09.py`). Audit S1–S10 in §1. Fixes: a row's button holds the name (body family, one line) and a second label for the details (small, mono, muted, at the end, the disabled colour on a disabled row); "Autosaves" and "Slots" labels; Save mode hides the autosave rows under a note ("Written as you play, when you pause and when you quit." / "Autosave is off."); empty rows have no fill and, enabled, a muted name; Delete is muted until its row or itself has focus, then danger, and stretches to the row with its word centred. No theme value or core UI was needed (`shell.json`, `widgets.rs`, `theme.rs` unchanged). Tests: a `details` helper reads the second label; new `saves::the_load_screen_groups_autosaves_and_slots`, `delete_is_quiet_until_its_row_has_focus`, `an_empty_row_reads_quieter`. `cargo test -p tungsten-template-basic --locked` → exit 0 (153, `fit::the_saves_screen_fits_in_both_modes` ok); `cargo test -p example-01-platformer --locked` → exit 0 (105); `just repo-check`, fmt, clippy → exit 0; `saves.rs` copy `cmp`-equal. tokei: shell 2,600, `src/` 3,100 | `src/shell/saves.rs` and copy, `tests/game/saves.rs`, the plan's §1; captures `perf-runs/20261021-m73-template-final-pass/step09/{load,save}-{1280,floor}-{before,after}.png` (eight) |
| 10 | 2026-10-21 | Done | `shell_frame` calls `post`, the shell's one `PostStack` edit (`kept`, `tail`, `left` as §2.7); the curtain's swap calls it with `swap` before the replace. `GameOptions::bloom` (default on) is the Display tab's Bloom toggle above Apply; each tab's Reset resets its own options (Display's Bloom, Audio's mute, Gameplay's the rest, keeping mute and Bloom). The sample's run inserts `PostStack(vec![Bloom(sample::BLOOM)])` and the default at its exit. Tests: `settings::the_bloom_option_strips_and_restores_the_games_bloom`, `bloom_off_then_a_run_change_restores_nothing_the_game_dropped`, `a_stack_that_outlives_its_run_keeps_its_bloom` (the run change is a `goto` to the title, since the sample's run builds its own stack), `a_stack_set_to_its_stripped_value_keeps_its_withheld_bloom`, `each_tabs_reset_keeps_the_other_tabs_options`, `sample::the_runs_post_stack_blooms_below_white`, example 01's `presentation::the_bloom_option_keeps_the_vignette`; the toggle test counts seven. Tuning on live gameplay captures (New Game on Normal, a bolt in flight; a ring 14–30 px round each sprite, on minus off, in 0–255 of its largest channel): the start values (0.9, 0.1, 2.0, 1.0) gave gem +4, speed pickup +20; radii 0.35–0.5 were weaker still; (0.8, 0.2, 2.5, 1.0) washed the heal pickup and the dummy white. Final `BLOOM` threshold 0.9, knee 0.1, intensity 3.0, radius 1.0: gems +9, heal +10, speed +29, the bolt in flight a warm glow, the dummy's white rings a halo, enemies +2 and the player +4 (eye highlights); sparks, 8 px particles fading over 0.5 s, glow only while fresh. `cargo test -p tungsten-template-basic --locked` → exit 0 (159, `sample::the_schedule_matches_the_snapshot` ok); `cargo test -p example-01-platformer --locked` → exit 0 (106); `just ctx` (template `AGENTS.md` 12,261 B), `just repo-check`, fmt, clippy → exit 0; three shell copies `cmp`-equal. tokei: shell 2,643, `src/` 3,152 | `src/shell/{mod,options,settings}.rs` and copies, `src/sample/mod.rs`, `tests/game/{settings,sample}.rs`, `templates/basic/AGENTS.md`, `examples/01_platformer/tests/game/presentation.rs`, `examples/01_platformer/README.md`; captures `perf-runs/20261021-m73-template-final-pass/step10/gameplay-bloom-{on,off}.png` |
| 11 | 2026-10-21 | Done | Game over builds its own screen: a clear modal root, a full-width band (`theme::DIED_BAND`, black at alpha 200) holding "YOU DIED" in the heading family at 64 on 72 (cap 96) with `DIED_SPACING` 0.2 em in `DIED_RED` (`#be1420`), and Retry, Load and Quit to Title hidden under it (`Menu` built in place). Its entrance is `Change::Entrance` (1.6 s, real clock, ticked in `flow`): the band fades in (QuadOut), the shell's post tail (`game_over::passes`, from the `died` clock) pixelates toward 48 px and fades toward 0.8 under the old crimson; at its end `game_over::reveal` shows the buttons over 0.3 s and focuses Retry. The sting and music start at once. Tests: `shell::game_over_reads_you_died_on_a_crimson_band` (replaces the old heading test: text, `DIED_RED`, 0.2 em, the heading family, the band's fill and the root's width), `shell::game_overs_buttons_wait_for_its_entrance`, `shell::game_over_pixelates_and_dims_and_removes_exactly_its_passes` (`[Vignette, Pixelate(48), Fade(0.8)]`, held across frames and a Load push and back, none after Retry), `fit.rs`'s rules check "YOU DIED" on one line, example 01's `presentation::bloom_and_vignette_survive_game_over_and_retry`. Contrast (WCAG, linear light): `#be1420` on the band over the background dimmed by the fade → 3.21:1; over a white pixel 1.75:1 and a quarter-white 48 px block 2.66:1 (follow-up 3). `cargo test -p tungsten-template-basic --locked` → exit 0 (161); `cargo test -p example-01-platformer --locked` → exit 0 (107); `just repo-check`, fmt, clippy → exit 0; three shell copies `cmp`-equal. Live captures from a seeded save at 1 of 5 health on the hazard's line (dies ~3.7 s after Continue, past the grace): mid-entrance and settled at 1280 × 720 and the floor; the game over draws over the HUD (its text 48 % under the band); a frozen damage number shows over the pixelated world (follow-up 4) and the HUD meets the band at the floor (follow-up 5). tokei: shell 2,718, `src/` 3,227 | `src/shell/{game_over,theme,mod}.rs` and copies, `tests/game/{shell,fit}.rs`, `templates/basic/README.md`, `examples/01_platformer/tests/game/presentation.rs`; captures `perf-runs/20261021-m73-template-final-pass/step11/game-over-{1280,floor}-{m1,m2,m3,m4,settled}.png` |
| 12 | 2026-10-21 | Done | `templates/basic/tungsten.json`'s `render.clear_color` `[0.00335, 0.00368, 0.00518, 1.0]` → `[0.0, 0.0, 0.0, 1.0]`; the python check → `True`. The visual test against the step 5 fixture failed as expected (857,303 pixels at max delta 4: the title's scrim over black). `title.png` re-captured with the guide's command (exit 0): sha256 `9ac6dc5a…30052a` → `9f7141e4…e49581`; `TUNGSTEN_VISUAL_REGRESSION=1 cargo test -p tungsten-template-basic --test game --locked -- visual:: --nocapture` → exit 0, 1 passed. Example 01's tree hash `51b572ae…00f0cf` before and after → equal. The curtain keeps the theme's background (Q18) | `templates/basic/tungsten.json`, `tests/fixtures/title.png` |
| 13 | 2026-10-21 | Done | `enemy_still.json` (listed in the manifest) is `enemy_orbit.json` less `orbit` (`jq -S 'del(.components.orbit)'` of one `cmp`-equal to the other); `ENEMIES[2]` is `("enemy_still", [1100, 380])`, the sample's docs and the template README say "three that move and one that stands". `sample::enemies_move_take_three_hits_and_come_back`: after 120 frames the still enemy has moved 0.0 and stands at its place while the other three moved more than 1 px; it falls to three hits with the orbiter and both come back 4 s later, the still one at its place, whole. Deviation: the still enemy took the second orbiter's place, one of step 3's lethal load spots, so `saves::a_continued_run_survives_its_first_two_seconds` now loads 64 px right of the remaining orbiter (`ENEMIES[1]`, `[264, 520]`); with the grace disabled for a probe it dies there (exit 101, "Continue at [264.0, 520.0]: the player died"), both files then restored and `cmp`-equal. `cargo test -p tungsten-template-basic --locked` → exit 0 (161); `cargo test -p tungsten-cli --locked` → exit 0; `just repo-check` → exit 0 (the prefab covered); fmt, clippy → exit 0. Example 01's tree hash `51b572ae…00f0cf` before and after → equal. tokei: shell 2,718, `src/` 3,227 | `templates/basic/assets/prefabs/enemy_still.json` (new), `assets/manifest.json`, `src/sample/mod.rs`, `tests/game/{sample,saves}.rs`, `templates/basic/README.md` |
| 14 | 2026-10-21 | Done | `tools/icon.py` (standard library; `sprites.py`'s sampler and PNG codec; `sys.dont_write_bytecode`, so a run without `-B` leaves no cache) draws the gem's diamond at 84 % on `#0b0c10` at 16, 32, 48 and 256 px into `windows/icon.ico` (5.3 KB) and `windows/icon.res` (5.5 KB: an empty entry, `RT_ICON` 1–4, `RT_GROUP_ICON` 1, language 0, flags `0x1010`/`0x1030`); its `--check` parses both, holds every size, offset and byte count to its payload and the fixed fields to a fresh render's, then compares decoded pixels (about 4 s). `build.rs` prints `rerun-if-changed` for itself and the `.res` and `cargo:rustc-link-arg-bins=<.res>` only for `windows`/`msvc`. `python3 -B templates/basic/tools/icon.py --check` → exit 0 ("icon.ico: matches", "icon.res: matches"). `llvm-cvtres /MACHINE:X64` → exit 0; `llvm-readobj --coff-resources` → `GROUP_ICON (ID 14)` name 1 and `ICON (ID 3)` names 1–4, data sizes 317, 612, 850, 3558 and 62. `llvm-rc /FO ref.res` on `1 ICON "icon.ico"` beside a copy of `icon.ico`, then the same two → the same types, names and data sizes; its group and icon payloads are byte-equal to the script's, the headers differing only in language (llvm-rc's default 1033 against neutral 0). `cargo build -p tungsten-template-basic --locked` → exit 0; `cargo test -p tungsten-cli --locked` → exit 0; `just script-test` → exit 0 (the icon check among its lines); `just template-check` → exit 0 (121 GB free; `tungsten new`'s copy builds, tests, smoke-runs and packages); fmt, clippy (template, `build.rs` included) → exit 0. Example 01's tree hash `51b572ae…00f0cf` before and after → equal. Tools README section and provenance; `w12-template.md` follow-up 5 lists example 01's icon, the launchers' and the runtime window icon. Owner-only: the release's Windows build shows the icon | `templates/basic/tools/icon.py`, `windows/icon.ico`, `windows/icon.res`, `build.rs` (all new), `templates/basic/tools/README.md`, `justfile`, `docs/plans/1.0/w12-template.md` |
| 15 | 2026-10-21 | Done | No file changed. `just smoke` → exit 0 (`example-01-platformer ... OK`, run from its folder). `cargo test -p example-01-platformer --locked --test game -- playthrough::a_new_game_walks_the_lower_route_into_the_collection_pit saves::a_capture_round_trips_through_a_load_with_a_hazard_and_a_platform_displaced` → exit 0, 2 passed. `just repo-check` → exit 0 (both shell copies and the theme checked). `PATH=<scratch>/level-venv/bin:$PATH just level-check` → exit 0 (15 tests); the venv and `target/zz-xkeys` then deleted with `shutil.rmtree`. Live captures at 1280 × 720 (`WINIT_X11_SCALE_FACTOR=1`): game over after the courier, walking right with air jumps, lands in the first spikes (x 1,280) and falls from 32 to 0 health, mid-entrance and settled; Settings' Display tab with the Uncapped and Bloom rows and the 60 fps floor; and the Load screen after the run's first autosave, the clearing's and pause's, Autosave 3, 2, 1 newest first (01's longer summary wraps to a third details line there) | captures `perf-runs/20261021-m73-template-final-pass/step15/ex01-{game-over-entrance,game-over-settled,settings-display,load}.png` |
| 16 | 2026-10-21 | Done | `D-199` with its index row and the seven marker lines (written before the docs that cite it; the caps from tokei's counts after step 14, 2,718 and 3,227 → 2,900 and 3,400). Each file read whole (DESIGN.md: Core UI Model and Template Shell and Exit only), bytes before → after: `README.md` 3,920 → 3,920; `AGENTS.md` 5,402 → 5,402; `crates/tungsten-render/AGENTS.md` 4,092 → 4,092; `DESIGN.md` 157,948 → 159,383 (letter spacing among the inherited text properties; the shell's three autosaves, the Autosave option, the version question, the 75 % UI, Uncapped from 60, the Bloom edit and the whole-stack rule, the grace, game over's band, the black clear, the icon, caps 2,900 and 3,400); `docs/DECISION_INDEX.md` 50,034 → 51,222 (rows `D-198`, `D-199` and the "Amended by" notes; other rows kept, Q21); `docs/LLM_INDEX.md` 12,159 → 12,193; `docs/README.md` 2,800 → 2,931 (template tools row); `docs/agent-setup.md` 9,287 → 9,527 (template rules, `D-183`); `docs/assets.md` 4,886 → 5,103 (`letter_spacing`; `windows/` is no asset); `docs/getting-started.md` 47,548 → 50,027 (the six items of step 16's list, the quit's autosave conditional on the option, the icon's dependence on `sprites.py`'s gem); `docs/known-issues.md` 48,017 → 48,091; `docs/releases.md` 24,449 → 24,658 (the Windows icon check); `docs/validation.md` 12,441 → 12,725 (the template row's theme copy and `tools/` checks; `just visual` names the title); `docs/perf/benchmarks.md` 93,816 → 93,816 (dated measurements kept, Q21); `docs/perf/profiling-workflow.md` 46,107 → 46,107. `rg -c '^\| `D-19[9]` ' docs/DECISION_INDEX.md` → 1; `rg -c '^\*\*Amended by D-19[9]:' DECISIONS.md` → 7; `rg -c 'Amended by `D-19[9]`' docs/DECISION_INDEX.md` → 7; the stale-text grep → no match (exit 1; the baseline `DESIGN.md:611` and `docs/getting-started.md:85` rewritten); `just ctx` → exit 0 ("Agent context checks: OK"); `just repo-check` → exit 0 ("Repository QA: 0 error(s), 79 note(s)"; manifests 2 passed, decision index 5 passed). | `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/{LLM_INDEX,README,agent-setup,assets,getting-started,known-issues,releases,validation}.md` |
| 17 | 2026-10-21 | Done, with one deviation | Each file read whole, bytes before → after: `templates/basic/AGENTS.md` 12,261 → 12,287 (58 lines; `D-199`'s caps, 2,900 and 3,400, and the icon's `build.rs`, byte-neutral by "and scale extremes"; the prefab and kit lists already current, `check_kit_catalogue` passing); `templates/basic/README.md` 3,711 → 4,175 (game over's three buttons, the version question, Uncapped, Bloom, the 75 % UI, the grace, the icon); `templates/basic/tools/README.md` 2,957 → 3,078 (`icon.py` imports the gem from `sprites.py`); `examples/01_platformer/README.md` 7,926 → 7,962 (the area autosave's slot and the option, stated once under Saves); `examples/01_platformer/tools/README.md` 39,332 → 39,507 (the theme under `just repo-check`, the Bloom option, game over's band and buttons, replacing "You died"); `examples/03_fixtures/tools/README.md` 2,483 → 2,483 (nothing moved). `just ctx` → exit 0 ("Agent context checks: OK"; the template's `AGENTS.md` 12,287 B, 58 lines); `just repo-check` → exit 0 ("Repository QA: 0 error(s), 79 note(s)"). **Deviation:** the done-when grep → one match, `templates/basic/AGENTS.md:13`, the new shell cap "2,900" that Q22 set (`D-199`), which the pattern's `2,900` arm, written for the old `src/` cap, also takes; the stale texts are gone: the same grep with `within 2,900 \(` (the old `src/` cap) in place of `2,900`, over the four files and example 01's `tools/README.md` → no match (exit 1). | `templates/basic/{AGENTS,README}.md`, `templates/basic/tools/README.md`, `examples/01_platformer/{README,tools/README}.md` |
| 18 | 2026-10-21 | Records closed before the cut; the cut, the checks and the preflight are in the session's report (the plan is archived first) | Workflow §5: (1) rows 1–17 quote their done-when, no step skipped; (2) gates by change per step, the release chain once after the cut with `just visual` and `just template-check`; (3) `title.png` moved on purpose (rows 5 and 12, the `CHANGELOG.md` entry); the seam files of both games unchanged (`git diff --quiet HEAD` → exit 0), `SaveGame::SCHEMA` 3 and 4 and `GameOptions::SCHEMA` 1 (`rg`), `examples/02_bench/tests/fixtures`, the root `assets/` and `crates/tungsten-kit` unchanged (exit 0); the hashes and digests through the chain's tests, the pixel fixtures through `just visual`, example 01's outputs through step 15's `just level-check` (nothing it owns changed after); (4) `D-198` (step 1) and `D-199` (step 16) with their index rows; (5) one ledger row, rustdoc, `api/tungsten-core.txt` +2 lines (step 1), `just api-check` in the chain; (6) the template every step, the guide in step 16; (7) `docs/LLM_INDEX.md` rows for `D-198` and `D-199`, `just ctx` in 16 and 17; (8) `DESIGN.md`'s Core UI Model and Template Shell sections (16) and its status line; (9) known issues gain the stale example allows and the selector's missing `script-test` for template tools; no backlog row cut, no gap row; the register row filled (release 0.94, "Cut 2026-10-21 as `v0.94.0`"); `w12-template.md` gains "Landed in 0.94", marks follow-up 4 done and takes this plan's open items as follow-up 6 (follow-ups 3–5 below and example 01's grace, follow-up 1); the README's owned fields from the cut; (10) the `CHANGELOG.md` entry under `[Unreleased]`, `status: done`, the plan's eight relative links (seven targets) rewritten for `docs/plans/archive/1.0/` and the file moved there. | `CHANGELOG.md`, `DESIGN.md`, `docs/known-issues.md`, `docs/plans/1.0/{implementation-plan,w12-template}.md`, this plan |

## Follow-ups

1. Example 01 can loop the same way: a save at low health in a hazard's reach. 01 (b)'s plan decides, since its getting-hit rules add an invulnerability window (the design's (b)).
2. The three icon follow-ups, recorded in `w12-template.md` by step 14.
3. Game over's crimson holds 3.21:1 against its band over the dimmed background (step 11), but a bright sprite under the band lowers it: 1.75:1 over a white pixel, 2.66:1 over a 48 px block a quarter white. A denser band (alpha 235) or a brighter crimson (`#d21c28`) would hold 3:1 over such blocks.
4. A damage number floating when the player dies stays crisp over game over's pixelated world: world labels draw after the post stack and wait on the held game clock (step 11's captures).
5. At the floor, the gameplay HUD sits under the top of game over's band, dimmed through it (step 11's floor captures).

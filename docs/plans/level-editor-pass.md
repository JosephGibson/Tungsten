# The level editor and command line pass (`editor-pass`) — in progress

- **status:** in progress
- **goal:** two outcomes. (1) Someone who has never seen the in-game level editor (F10) opens it and builds or changes a room without reading its source: the tool, room, layer and selection in force and what a click will do are on screen, every action has a button, and a refused edit says why. (2) An agent with the `tungsten` command line and the docs alone makes every edit the editor can, sees the result and recovers from an error by its message; what one of the three surfaces (the editor, `tungsten level`, a script's `level` step) cannot do that another can is named.
- **non-goals:** `D-267`'s core stands: every edit goes through the room commands, checked, journaled and undone the same way on every surface. No change to the room or tileset format's version, to the level rules, to generation or to saves. No UI text input in core (P8-5 stays 8.1's): the editor's prompt is its own. Shapes, layers, routes and the room's own settings stay the command line's; multi-select, box select, copy between rooms, a searchable palette, regeneration from the editor, tombstones and the manifest's shared-file check stay in Later. No release.
- **files to touch:** by step, in §3; nothing outside those lists.
- **ordered steps:** (1) the level guide as its own doc; (2) core: errors that name the fix, command summaries, schemas; (3) the script's `level` redo, `room` probe and schema; (4) `level apply`: stdin, redo, dry run, atomic writes, what changed; (5) `level new`, `query`'s listing, `map`; (6) help by verb and the new schemas; (7) the editor's panel; (8) the editor's world feedback and mouse rules; (9) the editor opens any registered room; (10) the editor's prompt: renames, overrides, places and links; (11) the decision and the records; (12) close.
- **done-when:** every step's done-when passes, quoted in the evidence log; `just check` passes; the cold-agent tasks run again with a fresh subagent and the counts set beside the baseline; the editor's hard cases driven for real; the before-and-after review sheet linked; the plan deleted with its citations and `editor-pass` out of Now.

Item `editor-pass` in [ROADMAP.md](../../ROADMAP.md), the owner's off-ramp of 2026-11-10 ahead of 7.13. Written 2026-11-10 at `30b7c6a1`. Runs under the [tungsten-plan skill](../../.claude/skills/tungsten-plan/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs: it is written, critiqued and run in one session, by the owner's instruction, so it ends without a run prompt.

## Context digest

Redo 7.12 landed rooms (`D-264`), seeded generation (`D-266`) and the in-game editor with authored generation (`D-267`) in three plans over two days. The editor is a sixth debug view: a two-column table whose rows name its state and its keys. The owner tried it on 2026-11-10 and reported that clicks did nothing; the game had no live room, and nothing on screen said so. The review behind this plan used both surfaces (`perf-runs/20261110-editor-pass/review/walk-notes.md`), built a room by real input (XTest) and with the command line alone, ran a fresh agent on three level-authoring tasks (`cold/before-report.md`: 8 stalls, 11 failed commands) and read how LDtk, Tiled, Ogmo and Godot's tile editor handle tools, palettes and feedback, and current guidance on command lines for agents (`review/outside-sources.md`). The pass keeps `D-267`'s rule that every surface edits through `Room::apply`, and records what it changes in one new decision entry (Step 11). 7.13 stays next; it puts example 01's level on rooms, so it is this pass's first user.

## 1. Audit

Read at `30b7c6a1`. Paths: `core/` = `crates/tungsten-core/src/`, `tungsten/` = `crates/tungsten/src/`, `cli/` = `tools/cli/src/`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | The editor's window is a table of label and value rows; its thirteen keys are rows at its end, below the fold at the owner's scale (12 rows show at 1280 × 720 on 1.75). Nothing is a button. | `tungsten/level_editor.rs:962-1060`; `perf-runs/20261110-editor-pass/before/` |
| A2 | Tools, the palette, the layer, delete, copy, undo and redo are raw keys, so a script reaches only the toggle and the two mouse actions. | `tungsten/level_editor.rs:371-409`, `:1134-1143` |
| A3 | A refused edit shows as one line that ends in `…`: the batch's path survives and the reason is cut ("room `blank`: commands[0].move: objects[0].at:…"). | `tungsten/level_editor.rs:796-800`, `:953-955`; `tungsten/debug_views.rs:293-296` |
| A4 | The editor lists and edits rooms whose instance lives. A game that spawns none shows "drag an object to move it" over an empty world. | `tungsten/level_editor.rs:963-967`, `:1187-1212` |
| A5 | The palette is one name stepped by `[` and `]`; the overlay is a 3 px circle an object, a box for the pick, no grid, no ghost. | `tungsten/level_editor.rs:686-706`, `:865-918` |
| A6 | The secondary button is the eyedropper; three of the four editors read erase there. | `tungsten/level_editor.rs:353-355`; `review/outside-sources.md` |
| A7 | A debug window docks at most 45 % of the viewport wide and 60 % high. | `tungsten/debug_views.rs:1004-1010`, `:1132-1136` |
| A8 | No verb makes a room; `level describe` on a game with none prints nothing. | `cli/args.rs:375`; `cli/level.rs:599-608` |
| A9 | A bare `-` is taken for an option, so `apply <room> -` never reads stdin (known issue, P2). | `cli/args.rs:574-576` |
| A10 | A usage error prints its reason and then all 110 lines of usage; no verb has help of its own. | `cli/main.rs:42-45`; `cli/args.rs:12-110` |
| A11 | `apply` and `--undo` write with `std::fs::write`; `--undo` drops the journal's line, so nothing redoes. `apply` answers with a count alone. | `cli/level.rs:328`, `:351-356`, `:390-398` |
| A12 | `describe` lists IDs without positions and `query` needs an ID or a cell, so reading a room back is a query an entry; no text view shows its terrain. | `cli/level.rs:439`, `:631-641` |
| A13 | The name hint is Levenshtein within max(1, length / 3), so a transposition gets none; a missing tile names no tile the tileset has; a batch that is no array gets no example. | `core/assets/data.rs:1182-1214`; `core/assets/room/compile.rs:188`, `:211`; `core/assets/room/command.rs:249-254` |
| A14 | In `tungsten check`, a room object's prefab or override error reads "tilemap `yard`, layer `objects`, object 2", the compiled map's terms, not the object's ID; a live patch already names the entry (`object_error`, private to the patch). | `core/assets/prefab.rs:1101-1106`, `:1360`; `tungsten/asset_loader/check.rs:228-229`; `tungsten/asset_loader/room.rs:223-225`; `core/assets/room/patch.rs:744-755` |
| A15 | The room and tileset schemas carry no descriptions; batches and scripts have no schema. | `core/assets/room/mod.rs:828-985` |
| A16 | A script's `level` step applies or undoes; nothing redoes, and no probe reads a room's entry. | `core/script.rs:929-934`, `:1470-1497`; `tungsten/script.rs:822-834` |
| A17 | The guide's level text is two paragraphs of about 2,600 and 1,500 characters; its list of script verbs leaves `level` out. | `docs/getting-started.md:60`, `:70`, `:93` |
| A18 | `RawInput::Key` carries a key code and modifiers, no text. | `core/input.rs:406-419` |
| A19 | `World::remove_room` despawns nothing and unregisters the room; nothing puts a spawned room away and keeps it registered. | `core/assets/room/spawn.rs:415-477` |
| A20 | `level render` refuses `--save-script`, which `render` takes. | `cli/args.rs:230-237`; `cli/render.rs:274-293` |

## 2. Design

### 2.1 Docs (Step 1)

A new guide, `docs/levels.md`, takes the level text out of `docs/getting-started.md`: the format in brief with its conventions (cells from the room's top-left, y down; a `rect` is `[col, row, width, height]`; `at` and `point` in cells, fractions allowed), a minimal tileset, room and manifest entries, the command line's verbs, the batch commands as a table, the script's steps and probes, and the editor. The getting-started guide keeps two sentences and a link in each place. Each later step corrects `docs/levels.md` in its own commit. The LLM index's rooms row names the guide, within the index's 12,288 bytes.

### 2.2 Core: errors, summaries, schemas (Step 2)

- `nearest` counts a swap of two neighbouring characters as one edit (optimal string alignment), so `gaurd` suggests `guard`. Every hint in the engine reads it.
- A missing tile's message adds the nearest tile and, for a tileset of at most 12 tiles, lists them.
- A batch that is no array answers with an example batch.
- `RoomCommandError` gains `reason`, its message without the entry's path, and `RoomPatchError::reason()` returns it (the whole message when a patch failed elsewhere), for the editor's sentence in §2.6.
- `RoomCommand::summary()` gives a command in a line (``move `guard` to 8, 12``; ``paint 4 cells of `solid` with `rock` ``), for `apply`'s report and the editor's history.
- `Room::object_of(&PrefabError)` gives the room's entry a compiled object's error is about (its index, its ID and the message), from the patch's private `object_error`; the umbrella's check and its generator sweep print it in the findings' own form, ``yard: guard: objects[1]: prefab `dummy`: component `health`, field `maxx`: …``. A Tiled map's wording stands, and so does the patch's.
- `room::json_schema`, `tileset::json_schema` gain a `description` a property, and `command::batch_schema()` describes a batch: each verb's fields, with an example each.

### 2.3 Script (Step 3)

- `LevelEdit::Redo`, written `{"room", "redo": true}`: `World::redo_room_batch`, failing a room with nothing to redo as `undo` does. The script format stays at version 1, as it did when `screen`, `room`, `level` and `generate` joined it.
- The `room` probe, in core beside `place`: `{"room"}` returns the room's size, cell, whether it is live and its origin, its layers, shapes, objects (each with `at` and, live, its world point) and places, and its journal's undo and redo counts; `{"room", "id"}` returns one entry as `tungsten level query --json` prints it.
- `script::json_schema()` describes a script: every verb's value, with descriptions. A test holds its verb list to the model's.

### 2.4 `tungsten level` (Steps 4 to 6)

- **`apply`**: a bare `-` is a positional; `--redo` beside `--undo`; `--dry-run` checks and reports without writing; `--check` runs the room's level check after the write. The answer is a line a command (`RoomCommand::summary`), then the undo line, and the check's line when the batch placed an object or set an override.
- **The redo journal**: an undone line moves to `target/tungsten/journal/<room>.redo.jsonl` with `at`, the hash of the room file as that undo wrote it (an undo writes the canonical form, so the line's own `before`, the hash of the bytes the apply read, need not match it). `--redo` applies the newest line's batch when the file's hash is its `at`, journals it again and drops it; an `apply` empties the redo file.
- **Writes**: an apply, an undo and a redo each change the room file, the journal and the redo journal, in that order, then `--inverse`'s file when one is asked for, each through `write_atomic`, as one set: every text is computed before the first write, and a write that fails puts the files written before it back as they were (an absent one removed) and fails the command, so no failure leaves the room edited and unjournaled. A file that cannot be put back is named in the error with what it now holds. The set's files are distinct: `--inverse` naming the room file or either journal (by `file_key`, as `finish_generation` compares its targets) is refused before anything is written. The set takes its writer as a parameter, as `finish_generation` does, so a test fails each write in turn.
- **`new <room>`**: `--size <cols>x<rows>` (40x24), `--cell <px>[x<px>]` (the cell of the first listed room on the tileset, else 16), `--tileset <id>` (the only listed one; with none listed, a new `tiles`), `--tile <sprite>` (repeatable, a new tileset's tiles) and `--dry-run`. It writes `<manifest folder>/rooms/<room>.json` with a render layer `art` and a collision layer `solid`, a new tileset's file, and lists both in the manifest by a textual insert that keeps the file's formatting: the entry goes at the end of the `rooms` object, or a `rooms` section before the root's closing brace. The result must read back with the room listed, else nothing is written and the entries are printed for a hand edit. It refuses an ID a room or tilemap holds and a file that exists.
- **`query <room>`** with no ID and no `--at` lists every layer, shape, object and place with its geometry; `--json` prints them whole.
- **`map <room>`** prints the room as text, a character a cell: `#` solid, `=` one-way, `/` and `\` slopes, `+` drawn only, `.` empty, letters for objects and digits for places with a legend. A room past 200 columns or 100 rows needs `--rect <col>,<row>,<w>,<h>`.
- **Help**: `tungsten level --help` and `tungsten level <verb> --help` print the verbs, their options, the batch commands and examples; a usage error prints its reason, the usage of the command or verb it belongs to and the help to run, not the whole usage. The API snapshot records the level help too.
- **`schema batch`** and **`schema script`**; **`level render --save-script`**; a failing check leaves no report in the temp folder.

### 2.5 The editor's structure and panel (Step 7)

`tungsten/level_editor.rs` keeps the state, the frame and the edits; `level_editor/panel.rs` builds and fills the window, `level_editor/overlay.rs` draws in the world, `level_editor/hooks.rs` holds the script hooks. The window stays a debug view (`ViewFrame::sync`: title bar, drag, collapse, close, saved place) with no table columns; its body holds the panel, 280 units wide:

1. **Message**: the last result, or a refusal in the error colour, wrapped over up to three lines; the whole text still goes to the log.
2. **Hint**: what the primary button will do here, and the cursor's room and cell.
3. **Rooms**: a row a live room, the current one marked, with its file or why it is read only; a click makes it current and fits the view to it. With none: "No room is live" and how to get one.
4. **Tools**: a button a tool with its key (`1 Select`, `2 Place`, `3 Paint`, `4 Erase`), the active one marked; then `Undo`, `Redo` (each with its count), `Delete`, `Copy`, `Fit`, `Grid`, `Game view`.
5. **Palette**: the prefabs while the tool is Place, the room's tiles while it is Paint, each a row with its sprite and name, the chosen one marked, in a list that scrolls on its own.
6. **Layer**: `all`, then a row a layer with its kind.
7. **Selection**: the pick's ID, prefab, position, generator mark and overrides.
8. **Findings**: the count, each finding wrapped.
9. **Keys**: the mouse and key reference, last.

`ViewSpec` gains `tall`, which lets a window take the viewport's height less its margins in place of 60 %; `Dock` carries it in the padding beside `strip`, so the views' pins hold. The panel's nodes are built once a window and written only when what they show changes.

A refusal reads as a sentence: "Can't move `lamp_1`: [8, 31] is past the room's 40 × 24 cells." from the edit's own words and `RoomPatchError::reason()`.

The script hooks: the `editor` probe returns the open flag, tool, room, layer, palette, selection, message, hint, history and findings; the `editor` action sets them and runs a button's action by name (`{"tool": "paint", "tile": "rock"}`, `{"do": "undo"}`). Tests and captures drive the editor through them.

### 2.6 World feedback and mouse rules (Step 8)

- A cell grid over the current room once a cell is 6 px or more on screen, clipped to the view; `G` and the Grid button toggle it.
- Each live room's ID at its top-left, and the hovered and picked entries' IDs, as world labels.
- Hover: the object under the cursor boxed. A ghost: the prefab's box at the cell for Place, the cell or the rectangle for Paint and Erase.
- Shift held over a paint or erase drag makes it a rectangle: one `paint` or `erase` with a `rect` a layer.
- The secondary button erases: cells under Paint and Erase, the object under the cursor under Place and Select. Alt with the primary button is the eyedropper in every tool, and a fifth tool, `5 Pick`, does the same on a plain click.
- `F` and the Fit button fit the current room into the view beside the panel; Escape drops a drag, a stroke or the pick.
- Objects off screen are skipped, and markers shrink to a 4-line box, so a room of thousands draws what the view holds.

### 2.7 Opening a room (Step 9)

`World::despawn_room(id)` puts away a room spawned to be looked at: it despawns the room's live instance and every live entity named in its area, takes the area out of the World's loaded areas and forgets the names given in it. It keeps the room registered and never touches a record a restore kept (`SaveState::kept`), which `remove_room` deletes. It is not how a run leaves an area it will come back to: the area's changes are dropped.

The Rooms section lists every registered room. A click on one that is not live opens it: spawned under the top state, right of the live rooms' bounds, made current, the view fitted. The editor opens a room only when the World has its area neither loaded nor kept (`SaveState::is_loaded`, `is_kept`), and says why otherwise, so no saved progress is spawned over. Before its first open it copies the World's `PostStack` and `AmbientLight`, their absence included, since `spawn_room` sets them from a room that gives them (`core/assets/room/spawn.rs:401-406`). Closing the editor puts its rooms away and the two resources back. A frame that starts with `Exiting` closes the editor before its first stage, in `App::run_frame`, so the last frame's save, which a game takes as early as `PreUpdate`, sees none of them on either exit path: `request_exit`'s, whose `PendingExit` becomes `Exiting` at the end of its frame, and the window close's, which sets `Exiting` and runs the last frame at once (`tungsten/app.rs:2941-2971`). A room the game spawned is never put away. Known limit, for known issues: a save the game captures while the editor holds a room it opened includes that room's area.

### 2.8 The prompt (Step 10)

One line of typed text, the smallest the editor needs: keys read from the raw stream, a US-layout map from key code and Shift to a character (letters, digits, `_ - . , = [ ] " '` and space), Backspace, Enter to commit, Escape to cancel; no caret movement, selection or paste. While it is open no other editor key acts. It shows in the Message line with what it is for.

- **Rename** (`F2`, a button): the pick's new ID, one `rename`.
- **Set** (a button, objects): `<path> = <value>`, the value JSON or else text, an empty value removing the override: one `set` of `set.<path>`.
- **Link** (a button, point and area places): `<room>.<place>`, empty to unlink: one `link`.
- A sixth tool, `6 Mark`: a click places a point place and a drag an area place, named `point_<n>` or `area_<n>`. Select picks, moves, copies and deletes places as it does objects.

The `editor` action's `{"prompt": "rename", "text": "guard"}` types and commits one, so a script covers them.

### 2.9 Parity after the pass

| Edit | Editor | `tungsten level` | Script |
| --- | --- | --- | --- |
| Objects: place, move, delete, copy, rename, override | yes | yes | yes |
| Cells: paint, erase, by stroke or rectangle | yes | yes | yes |
| Places: point and area, move, delete, link | yes | yes | yes |
| Undo and redo | yes | yes | yes |
| Shapes, layers, routes, the room's own fields | no (Later) | yes | yes |
| Make a room | opens any registered room | `new` | no: a script stages listed rooms |
| Write the room's file | each edit | each apply | never, by design |
| Read an entry back | the panel, the `editor` probe | `query`, `map` | the `room` probe |

## 3. Steps

Every code step ends with `cargo fmt --all -- --check` → exit 0 and `cargo clippy -p <each crate it touches> --all-targets --locked -- -D warnings` → exit 0, beside its own checks. A step that changes a public item runs `just api` and commits the snapshot.

### Step 1: The level guide

- **Files:** `docs/levels.md` (new), `docs/getting-started.md`, `docs/README.md`, `docs/assets.md`, `docs/LLM_INDEX.md`
- **Change:** §2.1. The Room row of `docs/assets.md` states the finding's form as the check prints it and links the guide.
- **Done-when:** `just repo-check` → exit 0; `just ctx` → exit 0; `rg -c 'tungsten level' docs/getting-started.md` → at most 3.
- **Commit:** `docs: give levels their own guide`
- **Moves:** none

### Step 2: Core: errors that name the fix, summaries, schemas

- **Files:** `crates/tungsten-core/src/assets/data.rs`, `crates/tungsten-core/src/assets/room/{mod,command,compile,tileset,patch}.rs`, `crates/tungsten-core/src/assets/mod.rs`, `crates/tungsten-core/src/tests/assets/{data,room,room_command,room_compile,room_patch}.rs`, `crates/tungsten/src/asset_loader/{check,room}.rs`, `crates/tungsten/src/tests/rooms.rs`, `tools/cli/src/tests/level.rs`, `api/tungsten-core.txt`, `docs/levels.md`, `docs/assets.md`
- **Change:** §2.2. New tests: `nearest_takes_a_swap_as_one_edit`, `a_missing_tile_names_the_tilesets_tiles`, `a_batch_that_is_no_array_shows_one`, `a_refusal_carries_its_reason`, `every_command_sums_up_in_a_line`, `the_batch_schema_names_every_verb`, `the_schemas_describe_every_property`, `an_objects_error_names_its_id`.
- **Done-when:** `cargo test -p tungsten-core --locked` → exit 0; `cargo test -p tungsten --locked rooms` → exit 0; `just api-check` → exit 0.
- **Commit:** `feat(core): name the fix in room errors and describe the formats`
- **Moves:** every "did you mean" in the engine gains swaps; tests that pin a message this step rewords are corrected in it and named in the evidence (`tools/cli/src/tests/level.rs:117` pins the batch message, so that file joins).

### Step 3: The script's redo, `room` probe and schema

- **Files:** `crates/tungsten-core/src/script.rs`, `crates/tungsten-core/src/tests/script.rs`, `crates/tungsten/src/script.rs`, `crates/tungsten/src/tests/{script,describe}.rs`, `api/tungsten-core.txt`, `docs/levels.md`, `docs/getting-started.md`
- **Change:** §2.3. New tests: `a_level_step_redoes_what_it_undid`, `the_room_probe_reads_a_live_rooms_entries`, `the_script_schema_names_every_verb`.
- **Done-when:** `cargo test -p tungsten-core --locked` → exit 0; `cargo test -p tungsten --locked script` → exit 0; `cargo test -p tungsten --locked describe` → exit 0; `just api-check` → exit 0.
- **Commit:** `feat(script): redo a level step and read a room back`
- **Moves:** `describe`'s hook list gains `room`.

### Step 4: `level apply`: stdin, redo, dry run, atomic writes, what changed

- **Files:** `tools/cli/src/{level,args}.rs`, `tools/cli/src/tests/{level,args}.rs`, `scripts/template-check.sh`, `api/tungsten-cli.txt`, `docs/levels.md`, `docs/known-issues.md`, `ROADMAP.md`
- **Change:** §2.4's `apply`, redo journal and writes. The known issue's row for `-` and Later's `write_atomic` line go. New tests: `a_bare_dash_is_the_batch_on_stdin`, `redo_applies_what_undo_took_back`, `redo_follows_an_undo_of_a_file_that_was_not_canonical`, `an_apply_clears_the_redo_journal`, `redo_refuses_a_file_edited_since`, `a_dry_run_writes_nothing`, `an_apply_reports_each_command`, `an_inverse_file_that_is_the_room_or_a_journal_is_refused`, `a_write_that_fails_puts_back_the_files_before_it` (an apply, an undo and a redo, each write failed in turn: the room file, the journal and the redo journal read as before the command).
- **Done-when:** `cargo test -p tungsten-cli --locked` → exit 0; `just api-check` → exit 0; `just script-test` → exit 0; `just template-check` → exit 0.
- **Commit:** `feat(cli): redo, dry-run and stdin for tungsten level apply`
- **Moves:** `apply`'s answer; the template check's pinned lines follow it.

### Step 5: `level new`, `query`'s listing, `map`

- **Files:** `tools/cli/src/{level,args}.rs`, `tools/cli/src/level/{new,map}.rs` (new), `tools/cli/src/tests/{level,args}.rs`, `scripts/template-check.sh`, `api/tungsten-cli.txt`, `docs/levels.md`
- **Change:** §2.4's `new`, `query` and `map`. The template check's room flow starts from `level new`. New tests: `new_writes_a_room_its_tileset_and_the_manifests_entries`, `new_keeps_the_manifests_formatting` (four manifest shapes: a `rooms` object with entries, an empty one, none, a one-line file), `new_refuses_a_taken_id_and_an_existing_file`, `query_lists_a_rooms_entries`, `map_draws_solid_one_way_and_slope_cells_with_a_legend`, `map_asks_for_a_rect_past_its_bounds`.
- **Done-when:** `cargo test -p tungsten-cli --locked` → exit 0; `just api-check` → exit 0; `just script-test` → exit 0; `just template-check` → exit 0.
- **Commit:** `feat(cli): add tungsten level new, map and a room listing`
- **Moves:** the verb lists the CLI's tests pin.

### Step 6: Help by verb, the schemas, the small fixes

- **Files:** `tools/cli/src/{args,main,schema,render,check,level}.rs`, `tools/cli/src/tests/{args,schema,render,level}.rs` (those that exist), `scripts/public-api.sh`, `api/tungsten-cli.txt`, `docs/levels.md`, `docs/getting-started.md`
- **Change:** §2.4's help, `schema batch`, `schema script`, `level render --save-script`, the temp report removed on a failing check. New tests: `level_help_lists_every_verb_with_an_example`, `a_usage_error_prints_its_verbs_usage_alone`, `schema_names_batch_and_script`, `level_render_keeps_its_script`.
- **Done-when:** `cargo test -p tungsten-cli --locked` → exit 0; `just api-check` → exit 0; `just script-test` → exit 0; `target/debug/tungsten level bogus 2>&1 | wc -l` → under 30.
- **Commit:** `feat(cli): help by verb and schemas for batches and scripts`
- **Moves:** `api/tungsten-cli.txt` gains the level help.

### Step 7: The editor's panel

- **Files:** `crates/tungsten/src/level_editor.rs`, `crates/tungsten/src/level_editor/{panel,overlay,hooks}.rs` (new), `crates/tungsten/src/debug_views.rs`, `crates/tungsten/src/{debug_hud,systems_overlay,inspector}.rs` (the `ViewSpec` literals), `crates/tungsten/src/script.rs`, `crates/tungsten/src/tests/{level_editor,describe,debug_views}.rs`, `docs/levels.md`
- **Change:** §2.5. The before captures are taken already (`perf-runs/20261110-editor-pass/before/`). New tests: `every_action_has_a_button`, `a_tool_button_picks_its_tool_and_marks_it`, `a_palette_row_picks_its_prefab_and_its_tile`, `a_layer_row_picks_the_target_layer`, `a_refusal_reads_as_a_sentence_and_wraps`, `no_live_room_says_so`, `a_read_only_room_says_why`, `the_editor_action_drives_every_control`, `the_editor_probe_reads_its_state`, `the_panel_fits_at_the_owners_scale` (1280 × 720 on 1.75 and on 1.0), `an_idle_open_editor_writes_no_node`.
- **Done-when:** `cargo test -p tungsten --locked level_editor` → exit 0; `cargo test -p tungsten --locked views_guard` → exit 0, both pins unmoved; `cargo test -p tungsten --locked describe` → exit 0.
- **Commit:** `feat(debug): give the level editor a panel with every action`
- **Moves:** `describe`'s hook list gains `editor` twice (action and probe). The views' pins must not move; if `views-open.json` moves, the step stops.

### Step 8: World feedback and mouse rules

- **Files:** `crates/tungsten/src/level_editor.rs`, `crates/tungsten/src/level_editor/{panel,overlay,hooks}.rs`, `crates/tungsten/src/tests/level_editor.rs`, `docs/levels.md`
- **Change:** §2.6. New tests: `the_grid_draws_the_views_cells_alone`, `a_ghost_shows_where_place_will_put_its_prefab`, `shift_makes_a_stroke_a_rectangle`, `the_secondary_button_erases_cells_and_deletes_an_object`, `alt_click_takes_a_tile_and_a_prefab`, `fit_shows_the_room_beside_the_panel`, `escape_drops_a_drag`, `a_room_of_thousands_draws_what_the_view_holds`.
- **Done-when:** `cargo test -p tungsten --locked level_editor` → exit 0; `cargo test -p tungsten --locked views_guard` → exit 0.
- **Commit:** `feat(debug): show what a click will do in the level editor`
- **Moves:** `engine_editor_secondary` erases; the eyedropper moves to Alt and the Pick tool.

### Step 9: The editor opens any registered room

- **Files:** `crates/tungsten-core/src/assets/room/spawn.rs`, `crates/tungsten-core/src/tests/assets/room_spawn.rs` (new; `spawn.rs` has no test module today), `crates/tungsten/src/level_editor.rs`, `crates/tungsten/src/level_editor/{panel,hooks}.rs`, `crates/tungsten/src/tests/level_editor.rs`, `crates/tungsten/src/app.rs`, `api/tungsten-core.txt`, `docs/levels.md`, `docs/known-issues.md`
- **Change:** §2.7. Known issues gains the capture limit. New tests: `despawn_room_puts_a_room_away_and_keeps_it_registered`, `despawn_room_leaves_a_kept_record`, `a_room_spawns_again_after_despawn_room`, `the_rooms_list_names_rooms_that_are_not_live`, `a_click_opens_a_room_beside_the_live_ones`, `a_room_with_saved_progress_is_not_opened`, `closing_puts_away_the_rooms_the_editor_opened`, `closing_gives_back_the_post_stack_and_the_ambient_light` (two rooms opened, one giving each; a World with neither ends with neither), `a_last_frame_closes_the_editor_before_its_save` (the harness's window close and `request_exit`, each with a `PreUpdate` system that reads the World's loaded areas in the last frame), `a_room_the_game_spawned_stays_when_the_editor_closes`.
- **Done-when:** `cargo test -p tungsten-core --locked` → exit 0; `cargo test -p tungsten --locked level_editor` → exit 0; `just api-check` → exit 0.
- **Commit:** `feat(debug): open any registered room in the level editor`
- **Moves:** none

### Step 10: The prompt: renames, overrides, places and links

- **Files:** `crates/tungsten/src/level_editor.rs`, `crates/tungsten/src/level_editor/{panel,overlay,hooks}.rs`, `crates/tungsten/src/level_editor/prompt.rs` (new), `crates/tungsten/src/tests/level_editor.rs`, `docs/levels.md`, `docs/known-issues.md`, `ROADMAP.md`
- **Change:** §2.8. Known issues gains the prompt's US-layout limit; Later's line for places, links, renames and overrides goes. New tests: `the_prompt_types_backspaces_commits_and_cancels`, `no_editor_key_acts_while_the_prompt_is_open`, `f2_renames_the_pick`, `set_writes_and_removes_an_override`, `link_points_a_place_at_another_rooms`, `mark_places_a_point_and_an_area`, `select_moves_and_deletes_a_place`, `a_refused_prompt_keeps_its_text_and_says_why`.
- **Done-when:** `cargo test -p tungsten --locked level_editor` → exit 0; `cargo test -p tungsten --locked views_guard` → exit 0; `just repo-check` → exit 0.
- **Commit:** `feat(debug): rename, override, place and link in the level editor`
- **Moves:** none

### Step 11: The decision and the records

- **Files:** `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `ROADMAP.md`, `docs/known-issues.md`, `docs/LLM_INDEX.md`
- **Change:** one entry at the next free ID, through the `tungsten-decision` skill: the editor's panel, mouse rules, opened rooms and prompt; `level new` and its manifest insert; the redo journal; the script's `redo` and probes; the schemas. Marker lines on `D-264`, `D-267`, `D-225`, `D-182` and `D-165`. `DESIGN.md`'s rooms paragraph follows. Later takes one line each for what the pass parked: shapes, layers, routes and room settings in the editor; a level overlay for `level render`; the prompt on core's text input; and the lines it keeps reworded to what is left.
- **Done-when:** `just repo-check` → exit 0; `just ctx` → exit 0; `cargo test -p tungsten --locked decision` → exit 0 (the index's row test, by the name the tree gives it).
- **Commit:** `docs(decisions): record the level editor and command line pass`
- **Moves:** none

### Step 12: Close

- **Files:** this plan (deleted), the files that cite it, `ROADMAP.md`, `docs/known-issues.md`; any file a QA fix needs, named in its own commit
- **Change:** in this order. (a) `just check`, `just physics-release`, `just template-check`, `just smoke`, `just visual`, `just api-check`, `just docs-ratchet`, `just script-test`. (b) The after captures and the review sheet: the editor at 1.0 and 1.75 beside the before captures, in `perf-runs/20261110-editor-pass/review/`. (c) The hard cases, for real: a failed write, undo and redo across the journal's 64, a generated room, two rooms on one file, a reload mid-edit, a window resize, a room of thousands of objects, by script and by real input. (d) A fresh-context review of `git diff 30b7c6a1..HEAD`. (e) The cold-agent tasks again with a new subagent on a new copy, the counts beside the baseline's. (f) Each finding fixed in a commit of its own, or written to known issues with its reason. (g) The plan and its citations deleted; `editor-pass` out of Now.
- **Done-when:** (a)'s commands → exit 0 each; the demo, a `tungsten run --script` report that opens a room `level new` made in the editor, builds it through the `editor` action and reads it back with the `room` probe → passed; `just repo-check` → exit 0.
- **Ends with:** the commits, the evidence, the review sheet and its folder as links, the agent counts before and after, what the owner should try by hand, the next prompt (7.13).

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Does `level new` edit the manifest, where `level generate` prints its entries (`D-267`)? | It edits it, by a textual insert checked by reading the result back, and prints the entries when it cannot. `generate` stays as it is. |
| Q2 | Does the secondary button's change break a habit worth keeping? | No: the editor is a day old. Erase on the secondary button, the eyedropper on Alt and a Pick tool. |
| Q3 | Where does an opened room spawn? | Right of the live rooms' bounds by two cells, at their top; at the origin with none. |
| Q4 | Does the prompt wait for P8-5? | No. It is the editor's own, a line of US-layout text, replaced when core's text input lands (Later). |
| Q5 | Do shapes make the cut? | No. A stroke or a rectangle of cells builds terrain; shapes stay the command line's (Later). |
| Q6 | A level overlay in `level render` (bounds, grid, IDs)? | Later. `map` and `query` give an agent the IDs, and a script that toggles the editor captures its overlay today. |
| Q7 | The contradictory messages for an override into an array (`tags.names`, the baseline's stall 7)? | Known issues at the close: they are the prefab overrides', not the level surface's, and the `room` probe removes the need that met them. |

## 5. Invariants

- **May move:** `api/tungsten-core.txt` and `api/tungsten-cli.txt` (Steps 2 to 6, 9); `describe`'s hook list (Steps 3 and 7); the hint tests Step 2 names; the template check's pinned `apply` lines (Steps 4 and 5).
- **Must not move:** the room and tileset formats' version 1 and every room file's canonical bytes; `Room::apply`'s inverses; `views-closed.json` and `views-open.json`; 02's `guards_rooms` and `guards_generate` (no field on `RoomObject`, `Room` or `RoomJournal`); 01's `pit`; the physics hashes; the schedule snapshots (the editor adds no system); the closed editor's two resource reads and one action a frame.

## 6. Stop conditions

- A done-when check fails: restore the step's files from the last commit, mark the step skipped, and continue with steps that don't depend on it. Steps 5 and 6 stand without each other; Step 8 needs 7; Steps 9 and 10 each need 7 and stand without 8 and without each other; Step 11 records what landed.
- A views pin, a 02 guard or 01's `pit` moves: the step stops; the cause goes in the evidence row.
- A file outside a step's list is needed for function: the step stops. A file that only pins a message, name or count the step changes joins with its reason in the evidence row.
- Step 10 is the first to drop if the session runs long: its Later line then stays, reworded.

## 7. Critique

Round 1: gpt-6-astra at xhigh, `--repo`, 2026-11-10, on the draft over `30b7c6a1`. Footer: `critique: model gpt-6-astra, effort xhigh, context 872000, mode repo (output cap 4000), 376.9 s, tokens in 776571 (cached 694656), out 7687 (reasoning 4349), commands 22, output 253.1 KB (largest 28 KB), usage 5h 0% → 0%, week 100% → 100%` (the week ran on bought credits). Four findings, all accepted. Round 2: the same critic at xhigh, `--repo`, on the plan without this section and the log, with a focus note naming round 1's changes. Footer: `critique: model gpt-6-astra, effort xhigh, context 872000, mode repo (output cap 4000), 254.9 s, tokens in 446485 (cached 383616), out 5902 (reasoning 2429), commands 18, output 222.5 KB (largest 39.6 KB), usage 5h 0% → 0%, week 100% → 100%`. Two findings, both accepted, the first a gap round 1's fix left. No third round.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | High. Closing an editor-opened room erases saved progress: `despawn_room` forgets the area's save state as `remove_room` does, and an unloaded room can hold a record a restore kept. | `core/save/id.rs:95-99` (`loaded`, `kept`); `core/save/world.rs:629-633` (a restore keeps unloaded areas); `core/assets/room/spawn.rs:469-475` (`remove_room` drops `kept`) | Accepted | §2.7: `despawn_room` never touches `kept`; the editor opens a room only when its area is neither loaded nor kept; tests `despawn_room_leaves_a_kept_record`, `a_room_with_saved_progress_is_not_opened`; the capture limit to known issues. |
| 2 | High. `write_atomic` a file does not keep the room and its journals consistent: a journal write that fails leaves the room edited and unjournaled. | `core/persist.rs:47-55` (one file); `cli/level.rs:328`, `:344` (the room before the journal) | Accepted | §2.4's Writes: the files of a command are one set, written in a fixed order, put back on a failed write, the writer a parameter; test `a_write_that_fails_puts_back_the_files_before_it`. |
| 3 | Medium. Redo's check against the line's `before` refuses the file the undo itself wrote, when the room was not canonical before the apply. | `cli/level.rs:333` (`before` hashes the bytes read); `:390` (an undo writes the canonical form); `cli/tests/level.rs:122` (the test canonicalises first) | Accepted | §2.4's redo journal: the undone line carries `at`, the hash of the file as the undo wrote it; test `redo_follows_an_undo_of_a_file_that_was_not_canonical`. |
| 4 | Medium. Opening a room sets the World's `PostStack` and `AmbientLight`, which closing does not put back. | `core/assets/room/spawn.rs:401-406` | Accepted | §2.7: the editor copies both, absence included, before its first open and puts them back when it closes; test `closing_gives_back_the_post_stack_and_the_ambient_light`. |
| 2.1 | High. Round 1's cleanup on `PendingExit` misses the window close, which sets `Exiting` and runs the last frame at once; the template saves in `PreUpdate`, before the debug views run. | `tungsten/app.rs:2941-2971` (`settle_exit`, `close_with_last_frame`); `tungsten/app.rs:2836-2841` (the views after the update stages); `tungsten/testing.rs:312-320` (the harness's close does the same) | Accepted | §2.7: a frame that starts with `Exiting` closes the editor before its first stage, in `run_frame`, one rule for both paths; `app.rs` joins Step 9; test `a_last_frame_closes_the_editor_before_its_save`. |
| 2.2 | Medium. The write set never checks that its files differ: `--inverse` naming the room file or a journal overwrites it with every write succeeding. | `cli/level.rs:346-349` (any path); `cli/level.rs:985-995` (`finish_generation` refuses a shared target) | Accepted | §2.4's Writes: the set's files are distinct by `file_key`, refused before the first write; test `an_inverse_file_that_is_the_room_or_a_journal_is_refused`. |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| Review | 2026-11-10 | done | Baseline: 8 stalls (0 source, 5 guess, 3 blocked), 11 failed commands, 3 of 3 tasks; 15 command-line and 10 editor points in the walk notes | `perf-runs/20261110-editor-pass/review/`, `cold/before-report.md`, `before/` |
| 1 | 2026-11-10 | done | `just repo-check` exit 0; `just ctx` exit 0 (the LLM index at 12,277 of 12,288 B, its rooms row in one brace form to pay for the guide's path); `rg -c 'tungsten level' docs/getting-started.md` 3 | `docs/levels.md` |
| 2 | 2026-11-10 | done | `cargo test -p tungsten-core --locked` exit 0 (lib 1,682 passed, the nine new tests among them; the plan's eight and `a_refused_batch_gives_its_reason_without_the_path`); `cargo test -p tungsten --locked rooms` exit 0, and its module's filter `asset_loader::room` 22 passed; `cargo test -p tungsten-cli --locked` 88 and 10 passed; `just api-check` exit 0 (five additions to `api/tungsten-core.txt`); fmt and clippy exit 0. Pinned messages corrected: `room_compile.rs:164` (the tile list), `tests/rooms.rs:207` (the object's ID), `tests/rooms.rs:388` (`rokc` now suggests `rock`), `tools/cli/src/tests/level.rs:109` (the batch example) | the step's commit |
| 3 | 2026-11-10 | done | `cargo test -p tungsten-core --locked` exit 0 (lib 1,684 passed: `the_script_schema_names_every_verb`, `the_room_probe_names_its_fields_and_an_unknown_room`, and the level verb's test extended for `redo`); `cargo test -p tungsten --locked script` exit 0, 27 passed (`a_level_step_redoes_what_it_undid`, `the_room_probe_reads_a_live_rooms_entries`); `describe` 7 passed (its probe list is a contains check, unmoved); `just api-check` exit 0 (`LevelEdit::Redo`, `script::json_schema`); workspace check, fmt and clippy exit 0. Pinned: core's `hooks_register_once_and_run_by_name` lists the probes, `room` added | the step's commit |
| 4 | 2026-11-10 | done | `cargo test -p tungsten-cli --locked` exit 0 (97 and 10 passed; the nine new tests, the three write sets failed at each of their 4, 3 and 3 writes); `just api-check` exit 0 (three option lines); `just script-test` exit 0; `just template-check` exit 0 (its room flow now applies from a file and from stdin and runs `--undo`, `--redo`, `--undo`); fmt and clippy exit 0. By hand on a `tungsten new` copy: stdin, undo, redo, a dry run's `did you mean` and `--check` naming `guard`. The check's line prints only for a batch that places an object or sets an entry's field | `perf-runs/20261110-editor-pass/steps/step4-template-check.log` |
| 5 | 2026-11-10 | done | `cargo test -p tungsten-cli --locked` exit 0 (104 and 10 passed; the six new level tests and `level_new_and_map_read_their_options`); `just api-check` exit 0; `just script-test` exit 0 (shellcheck SC2016 on a single-quoted backtick fixed first); `just template-check` exit 0, its room flow now `level new`, apply from a file and stdin, query, map, describe, check, undo, redo; fmt and clippy exit 0. `docs/releases.md` joined: its template-check sentence names the room flow's verbs. `describe` on a game with no room now says how to make one | `perf-runs/20261110-editor-pass/steps/step5-template-check.log` |
| 6 | 2026-11-10 | done | `cargo test -p tungsten-cli --locked` exit 0 (110 and 12 passed; the four planned tests, `the_checks_closing_line_is_the_one_dropped`, `a_check_run_hands_back_the_games_status_and_report`, and two end to end); `just api-check` exit 0 (`api/tungsten-cli.txt` 204 lines: the usage, then level's help); `just script-test` exit 0; `tungsten level bogus 2>&1 \| wc -l` 25; fmt and clippy exit 0. The help's worked example run verbatim on a fresh `tungsten new` copy: new, apply from stdin, map, check clean, render with `--save-script`. Deviations: the baseline's "report left in the temp folder" was a misreading, the command line removes it, so the fix drops the game's closing line that names it (`check.rs`, stderr passed through otherwise); a reader that closes the pipe (`\| head`) made `println!` panic, now a quiet exit 141 (`main.rs`). Joined: `tools/cli/tests/cli.rs` pins the usage error's shape in three tests; `tools/cli/src/tests/check.rs` tests the filter | the step's commit |
| 7 | 2026-11-10 | done | `cargo test -p tungsten --locked level_editor` exit 0 (32 passed: the eleven planned tests, and the 21 older ones on the new sentences); `cargo test -p tungsten --locked views_guard` exit 0, `views-closed.json` and `views-open.json` unmoved; `cargo test -p tungsten --locked describe` exit 0; the umbrella's lib 1,106 passed; `just api-check` exit 0; fmt and clippy exit 0. Looked at for real through the stage script at 1.0 and 1.75 and on the owner's display: the first capture showed a 32 px sprite drawn past its 16-unit box (a snapped root draws an image at a whole multiple of its natural size, so the thumbnail's natural size is the shrunk one), and a status and a no-room hint cut at three lines (now six). Deviations: Fit lands here, not in Step 8, since a room's row fits its room; the Grid button waits for Step 8. `ViewSpec` and `Dock` gained `tall`, so the other three views' literals name it (`debug_hud.rs`, `systems_overlay.rs`, `inspector.rs`, in the step's list). `tests/describe.rs` pins the `editor` action by name and its doc's start, the game's two whole. A leading space on a wrapped line is core's wrap, left alone | the step's commit |
| 8 | 2026-11-10 | done | `cargo test -p tungsten --locked level_editor` exit 0 (39 passed: the eight planned tests, the old eyedropper test folded into `alt_click_takes_a_tile_and_a_prefab`); `views_guard` exit 0, both pins unmoved; `describe` exit 0; the umbrella's lib 1,113 passed; fmt and clippy exit 0. Looked at through a nine-capture stage script at 1.0: the grid, a hover's box and ID, the lamp's 8 by 16 ghost, a stroke's cells; the grid's alpha halved after the first look (linear blending). Decisions inside the step: Shift is read at the press; the eyedropper takes up the tool that uses what it took, so Pick is one take; Escape is the one key with no button, since a button cannot be clicked mid-drag, and the modal root keeps it from `engine_exit` and the game's pause (asserted); a pick tests only objects within 1,024 px of the cursor. A script holds no Shift or Alt: the guide names a `level` step's `rect` and the action's `prefab` and `tile` as their equals | the step's commit |
| 9 | 2026-11-10 | done | `cargo test -p tungsten-core --locked` exit 0 (lib 1,687 passed, the three `room_spawn` tests among them); `cargo test -p tungsten --locked` exit 0 (`level_editor` 46 passed: the seven planned editor tests); `just api-check` exit 0 (`World::despawn_room` the one new line of `api/tungsten-core.txt`); `views_guard` exit 0, pins unmoved; 02's `guards` (16 passed) and 01's `game guards` exit 0 with the frame's new `has_resource::<Exiting>()`; fmt and clippy exit 0. For real: a script opened `hall`, then `blank`, in a staged run and closed (`/opened` == ["hall"], `room` probe `/live` false after the close; 3 captures, 2 asserts passed). Decisions inside the step: `despawn_room` returns the count it despawned; a room that is not live and may not be opened reads `not live: the game opens it` in its row and the reason in the refusal; the Rooms list scrolls past six rows, live rooms first; with an empty state stack an opened room's entities take `engine_level_editor` as their state | the step's commit |
| 10 | 2026-11-10 | done | `cargo test -p tungsten --locked` exit 0 (lib 1,129 passed; `level_editor` 55: the eight planned tests and `the_panels_buttons_stay_put_as_the_cursor_comes_for_them`); `views_guard` in it, pins unmoved; `just api-check` and `just repo-check` exit 0; fmt and clippy exit 0. For real: a 12-capture stage script at 1.0 and 1.75 with a rename left open in the hint, a marked area and `hall` opened (its one assert was the script's own wrong value, `objects` for `object`). Deviations: Rename's key is R, not F2: F1 to F11 are the engine's toggles and F2 opened the systems overlay under the first test, so the test is `r_renames_the_pick`. Found by a test whose click missed its button: the hint shrank as the cursor came onto the panel and the buttons moved under it. Now the cursor's cell and hover stay what they were in the world while it is over a window, the status keeps two lines and the hint three, the layers sit above the palette, the actions go four a row (`Game view` is `Home`), and at 1280 by 720 on 1.75 the layers and the palette's first rows show without a scroll. An area is picked by its outline, since its inside can cover an arena; the action's `select` takes a place | the step's commit |
| 11 | 2026-11-10 | done | `just repo-check` exit 0 (0 errors); `just ctx` exit 0 (the LLM index at 12,278 of 12,288 B, the editor's folder beside its file as `level_editor*`); `cargo test -p tungsten-core --locked decision` exit 0 (the index's row test lives in core's integration tests, not the umbrella's). `D-268` with marker lines on `D-264`, `D-267`, `D-225`, `D-182` and `D-165`; `DESIGN.md`'s rooms paragraph and its editor non-commitment; Later's Q10 line without the grid and three new lines (shapes, layers, routes and settings in the editor; an overlay for `level render`; the prompt on core's text input) | the step's commit |
| 12 | 2026-11-10 | done | (a) At `bba929f2`: `just check` exit 0 (317 s), `just physics-release` exit 0, `just api-check` exit 0, `just docs-ratchet` exit 0, `just script-test` exit 0, `just smoke` exit 0, `just visual` exit 0, `just template-check` exit 0 (276 s). (b) Captures after the last fix at 1.0 and 1.75, the before's script on the new editor and the pass's own 12-capture script, in five review sheets. (c) Hard cases for real, by script and, for a reload mid-drag and a window resize, by real input: a failed write of the room and of its journal; undo and redo past the editor's 64 and through 70 of the command line's; a generated room; two rooms on one file; a file edited under a held drag; the window at 1700 by 950 and 900 by 560; a room of 5,000 objects (400 frames in 1.9 s with the editor open on it). They found seven things, each fixed: a write's cause said twice with its whole path, on both surfaces; a shared file edited without a word; a report, `describe` and a map legend of thousands of lines; a fit made before the panel had a box; labels of two entries in one cell on one line. (d) A fresh-context review of `30b7c6a1..93c6ab82`: seven findings, all fixed (a pick carried into another room; the eyedropper taking the auto-tiler's variant or what play left; a script's prompt committing one left open; a drag or mark outliving its tool; `[` `]` stepping prefabs under Erase; a stale palette title; a stderr relay ending at a line that is not UTF-8), and of its three unverified one confirmed and fixed (the game camera's rotation lost to the editor's view), one to known issues (a journal name clash), one left (a far-edge rounding that is refused, not misapplied). (e) The cold agent again, a new subagent on a new copy with the same tasks and brief: 3 stalls (0 source, 3 guess, 0 blocked) against 8, 1 failed command against 11, 3 of 4 wrong batches fixed from the message alone against 1, no hand edit of a room file, 281 s against 476. Its three asks landed after it: the `room` probe reads `null` for an entry that is gone; a command that runs the game leaves out its INFO lines; `apply --check` says the batch stays applied and how to take it back. (f) Left, in known issues: the journal name clash, a batch of thousands checked a command at a time (6.2 s for 5,002), an override into a list, one bad override reported twice, a wrapped line starting with a space. Demo: `tungsten level new den --size 16x10 --tile gem` in a fresh copy, then a script that opens `den` in the editor from the Rooms list, paints a floor, places a dummy, marks a point and renames it `start`, and reads each back with the `room` probe: `tungsten run: 90 frames, 1 capture, 7 asserts: passed`; `tungsten level check: no room findings` | `perf-runs/20261110-editor-pass/` (`review/`, `after/`, `hard/`, `cold/`, `demo/`), machine-local |

## Follow-ups

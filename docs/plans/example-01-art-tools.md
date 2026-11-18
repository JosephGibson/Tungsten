# 01's art tools on shared helpers, its big props whole (example-01-art-tools) — in progress

- **status:** in progress
- **goal:** Phase 7's 7.13f (§2.8): example 01's eight art-tool rows in one asset session at max. Five copies call the helper they copy, every owned output byte-equal: `crossfaded` (S-141), the summoning ring and `icon_of` (S-151), `white_alpha` over given steps (S-161), `pixels.ramp` (S-166), one flame-ball field and `fire_symbol` with its limits (S-115); one clip helper in `art.py` writes every clip record (S-55), byte-equal too; `just level-check` runs `tungsten check` after the generator, which drops its copies of that check (S-131); and every prop larger than a tile is drawn, placed and judged whole, one sprite or clip a prop, the iron block's brick and frost included, its groups gone (S-74), the scenery and effects families unlocked for it and locked again on the owner's acceptance (`D-223`).
- **non-goals:**
  - 7.13g, the evaluation, which starts from this plan's last commit.
  - The backdrops' cut structures (`backdrops.py`'s `cut`, the far mists, the cave mouths, recesses and tunnels): cells of the map's tile and decorations layers, which take a 64 px sprite a cell; S-74's Where names none.
  - A whole prop's padding trimmed: each keeps its whole tiles, so it lands where its top-left piece did (§2.7).
  - Any other art change or redraw: S-74 moves only the normals of the props it makes whole (§2.7), and the review judges that alone.
  - The template's tools, the engine and the kit: no row reaches them.
  - A save migration: no saved shape moves (children come back from their parent's prefab, A11); 01's schema stays 41.
  - A sitting (`D-226`); a perf row's script or `workload_version`.
- **files to touch:** by step, §3's Files lines; nothing outside them but §6's admitted files. In short: 01's `tools/` (`sfx`, `effects`, `actors`, `art`, `scenery`, `placement`, `level`, `polish_art`, `generate`, `test_generate`, `level.json`, `locks.json`, the iron's content prefabs, `README.md`), the `justfile`, the outputs the generator owns (`examples/01_platformer/assets/`, `tools/outputs.json`), one 01 system and its doc, `tests/game/ice.rs`, the pit pin, `e01-area-clearing`'s GPU counter pin, `docs/validation.md` and the records.
- **ordered steps:** (1) `crossfaded` at its copies; (2) the ring, the quantiser and the ramps; (3) the flame ball and the fire ramp; (4) one clip helper; (5) `tungsten check` after the generator; (6) the scenery's big props whole; (7) the iron block whole; (8) the owner's review and the locks; (9) close.
- **done-when:** every step's done-when passes, quoted in the evidence log; `just check` passes and the demo (Step 9) works; the eight rows read `landed <sha>` in phase 7's §2.2 (a skipped step's rows `open` with their move, §6); the families locked again; the plan deleted with its citations and `example-01-art-tools` out of Now.

Item `example-01-art-tools` in [ROADMAP.md](../../ROADMAP.md), phase 7's 7.13f (§2.8 of [phase 7](phase-7.md)), from the redo's 7.13 row and §6 and phase 7's Step 13. Written 2026-11-18 at `50faefa8`. Runs under the [tungsten-plan skill](../../.claude/skills/tungsten-plan/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs.

## Context digest

- Eight rows of 7.5's register that phase 7's §2.2 sends to "7.13's asset session": all in 01's `tools/` but S-131's recipe line and S-74's iron (one 01 system, its test, the content prefabs). The run is an asset session at max (the redo's §6, the operating rules' tiers): the rows change the tools that draw locked art.
- Five rows are refactors whose proof is `generate.py --check`: every owned output byte-equal and all 20 locked families at their hashes (`D-223`). This session prototyped S-115, S-141, S-151, S-161 and S-166 on an exported copy: the 2,407 sprites and 170 clips `art.build_art()` makes and the 117 WAVs equal the base's (A17), and the edit scripts are kept (`W/edits/`). S-55 keeps every record by construction, which `--check` proves.
- S-131: `tungsten check` reports each of the six checks the generator and `level.py` repeat, and passes on 01 at the base (A7).
- S-74 is the art change: a whole prop's relief normals come from its whole image, where each piece normalised its own (A8), so the review is the owner's and so is the families' unlock (Q1). A stitched prototype, pieces pasted into wholes under S-74's names and sizes, measured what it moves: 01's pit `allocated_bytes` by −1,440 (props) and −11,520 (iron) alone; `e01-area-clearing`'s `upload_bytes` 816,192 → 760,032 alone; in 01's suite only the pit and the iron test's eight children (A12–A14).
- Rules: byte equality at Steps 1–5; the pins by §2.9; the locks by §2.10; lean captures; no sitting (`D-226`).
- Later items expect: 7.13g to start from this plan's last commit.

## 1. Audit

Read at `50faefa8`. `E/` is `examples/01_platformer/`, `T/` `E/tools/`, `W/` `target/plan-wip/7.13f-art-tools/` (untracked: edit scripts, probes and `NOTES.md`). At this tree `generate.py --check` reads `OK: 6368 owned outputs match and 20 locked families hold; …`; the tools' unit tests run 46, OK, in 428 s (the whole level check 9 min); 01's suite passes 500 with 1 ignored; the pit pin (`E/tests/game/guards/pit.json`): allocations 60,846, bytes 60,782,105, archetype moves 296, spawned 295, despawned 245, events 16.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | S-141: four loops fold their tail into their head by the four lines `crossfaded` holds: the ice spray's wind, the flamethrower's roar, the wisp's hum and `ambience_loop` | `T/sfx.py:214-219`, `:302-306`, `:705-709`, `:1338-1342`; `crossfaded` `:1496-1503` |
| A2 | S-151: the summoning ring is built three times; `icon_dummy` is `icon_of(dummy_canvas(), 2, 2)`, whose two extra rows sample past the dummy's 64 px canvas | `T/effects.py:588-591`, `:604-607`, `:661-664`; `icon_dummy` `:584-598`, `icon_of` `:601-614`, `icon_prop` `:657-670` |
| A3 | S-161: `white_alpha`'s dithered pick between two alpha steps, fixed to `WHITE_LEVELS`, is written again in `radial_glow`, `haze` and `hurt_edge`; the register's Where misses `hurt_edge`, its count of 4 holds it | `T/effects.py:100-107`, `:23-38`, `:2365-2377`, `:3203-3210` |
| A4 | S-166: nine picks `symbols[min(len - 1, max(0, int(v * len)))]` are `pixels.ramp`; three dither first (0.28, the dust's 0.25, 0.16) and the snowball after the multiply (`+ ordered * 0.5`, five symbols); `effects.py` does not import `ramp`, and `hud_liquid` names a local `ramp`; the dust's dithered value also gates it (`v > 0.08`) | `T/effects.py:267-268`, `:315`, `:332`, `:433`, `:1904-1909`, `:1948`, `:1960-1961`, `:1970`, `:2571`, `:2992`; `T/actors.py:3408`; `T/pixels.py:219-223`; `T/effects.py:14` |
| A5 | S-115: the wisp copies the fireball's polar field and fire ramp with other constants, its dither added before its swirl where the fireball's comes after; the missile's ramp copies `fire_symbol` with its own limits and cutoff | `T/actors.py:1000-1040`, `:1385-1425`, `:1108-1120`; `T/effects.py:1345-1352` |
| A6 | S-55: ten clip records written by hand, five builders storing table clips' frames that `generate.spray_clips` records apart, and scenery's own `clip` | `T/actors.py:988-996`, `:1043-1050`, `:1143-1150`, `:1258-1267`, `:3484-3497`; `T/scenery.py:495-504`, `:514-517`, `:4111-4117`; `T/generate.py:711-726`, `:862`, `:1217`; `T/effects.py:1795-1799`, `:1924-1928`, `:2523-2535`, `:2662-2666`, `:3101-3109`, `:3224-3239`; `T/art.py:244`, `:249` |
| A7 | S-131: the generator checks a clip's sprites, a particle config's sprite and a material's shader, `validate_level` a tileset tile's sprite, a layer's length and its GIDs, with their tests; `tungsten check` reports each, and passes on 01 (`tungsten check: no errors`, 1 s warm) | `T/generate.py:1198-1205`; `T/level.py:727`, `:731`, `:732`; `T/test_generate.py:48-59`, `:483`, `:535`, `:596`, `:679`; `crates/tungsten/src/asset_loader/check.rs:81`, `:172`, `:198`; `crates/tungsten-core/src/assets/manifest.rs:104`; `crates/tungsten-core/src/assets/tilemap.rs:43`, `:1023`; `justfile:100-102` |
| A8 | S-74's art: four composites cut 64 px (`cut`), the level's props padded to whole tiles and cut (`placed`), the mists' frames cut into per-tile clips (`rising_clips`), the pool's shore and basin a clip a tile; `relief` gives every `_big_` piece relief normals from `relief_heights`, which normalises luminance over that piece alone, and a fully opaque piece wraps its slopes across its own edges; a sprite is 64 px unless `SIZE_PREFIXES` or a size table names it | `T/scenery.py:27-31`, `:506-509`, `:4091-4107`, `:4227-4241`, `:4286-4295`; `T/art.py:147-150`, `:153-165`, `:172-178`, `:106-110`, `:290` |
| A9 | S-74's level: `level.json`'s 792 props hold 259 pieces of 63 props: 200 static in 58, 59 animated in 5 (the haze, the small haze, the plume, the pool's basin and shore); 57 `groups` bind 57 of the 58 static ones (`far_rubble` none), each box exactly its pieces' cells, anchored `ground`, `under` or `wall`, 28 of them at a fractional x; a prop's pieces share depth and clip and sit together in the list; the props layer holds 940 objects; the sheave reads the gantry's first piece | `T/level.json` (`jq`, A-probes in `W/NOTES.md`); `T/level.py:155-167`, `:323-325`; `E/assets/tilemaps/level.tmj` |
| A10 | S-74's placement: a group stands by its composite's box, its feet within its support, `under`'s tuck and a settle on the drawn tiles; a supported single prop by its centre and its own box, a covering stair taking its support; the floating check judges a group by its pieces' pixels and fails a piece in no group's box | `T/placement.py:63-81`, `:135-163`, `:343-365`, `:410-420` |
| A11 | S-74's iron: eight children, four lit brick quarters (z 13) and four hidden frost quarters (z 14); `iron_look` tints by index, `index >= 4` the frost; no save names a child: they come back from their parent's fresh spawn | `T/content/prefabs/iron_block.json`, `T/content/prefabs/ex10_iron_{quarter,frost}_*.json`; `T/polish_art.py:22-25`, `:175-224`, `:227-272`; `E/src/features/toys/systems.rs:1620-1636`, `components.rs:44`; `crates/tungsten-kit/src/foundation/saved.rs:19`, `:41` |
| A12 | The pit by controls on the base, each restored from HEAD: one props-layer object a big prop (744 props) moves `allocated_bytes` 60,782,105 → 60,780,665 alone; the iron's two children → 60,770,585 alone (`iron_look` collects each of a fresh run's four loose blocks' children every frame: 6 × 8 B × 4 × 60); both → 60,769,145, and so does the stitched prototype | `W/probes/whole_control.py`, `stitch_whole.py`, `stitch_iron.py`; `T/level.json` (`iron`, four blocks); `E/src/features/toys/systems.rs:1621-1628` |
| A13 | GPU counters on the stitched prototype (every scenery piece set whole under its §2.7 name, size and filter; the iron's two): `e01-area-clearing: upload_bytes: the window sums 760032, pinned 816192` alone; draw calls 64, batches 21 and text renders 2 hold, and `gpu`, `gpu-throughput` and `template` read `ok`; the base reads `ok` everywhere (7 s warm) | `scripts/perf-rows.json` (`e01-area-clearing`'s `counters`); `perf-runs/20261118T153636Z-counters`, `…T153749Z-counters` |
| A14 | 01's suite on the stitched prototype: 498 passed, 2 failed (the pit guard; `ice::frozen_iron_and_scraps_render_and_escaped_scraps_are_cleaned_up` at its eight children), 1 ignored; no other 01 test names a piece. Three of the tools' tests read groups | `E/tests/game/ice.rs:89-116`; `T/test_generate.py:86-94`, `:129-142`, `:258-268` |
| A15 | Locks: scenery holds 1,987 files, 883 of them pieces; effects 323, 16 the iron's; the backdrops' 848 pieces are tiles; an unlock is the owner's, an asset session starts from it and ends with `--lock` on the acceptance (the last pair: `0d329267`, `5560cbcb`) | `T/locks.json`; `D-223`; `T/generate.py:1153-1191`; `T/README.md:62-72` |
| A16 | Docs naming pieces, groups or the level check's parts | `T/README.md:28-30`, `:129`, `:189`, `:267-277`, `:501-511`, `:537-539`, `:965`, `:1075`; `docs/validation.md:24` |
| A17 | Probes of S-115, S-141, S-151, S-161 and S-166's edits on an exported copy, applied in §3's order: 2,407 sprites, 170 clips and the lit set equal the base's (`art.build_art`, 51 s), and the sounds' and music's 117 WAVs | `W/edits/s141.py`, `s151.py`, `s161.py`, `s166.py`, `s115.py`; `W/probes/artprobe.py`, `soundprobe.py` |
| A18 | The system Python has no Pillow; `target/e01s/venv` has 12.3.0, which the generator requires | `T/generate.py:1110`; `T/requirements.txt` |

## 2. Design

### 2.1 Scope, order and proof

The refactors land first, each proved by `--check` with every lock holding, so a slip in a shared helper fails at its own step, naming the files it moved. S-131 comes before S-74 so the whole props' manifest meets `tungsten check` inside `just level-check`. S-74 goes last, in two steps (the scenery's props, then the iron, which reaches 01's Rust) and a review step, because it changes locked art and waits on the owner. Steps 1 to 5 move no output byte, so no pin, counter or 01 test can move there.

### 2.2 `crossfaded` at its copies (Step 1, S-141)

The ice spray's wind, the flamethrower's roar and the wisp's hum return `finish(crossfaded(samples, count, overlap), fade_in=0, fade_out=0)`; `ambience_loop` returns `encode(crossfaded(samples, count, extra), AMBIENCE_RATE)`. The four bodies are `crossfaded`'s line for line (A1), so every WAV keeps its bytes (A17; `W/edits/s141.py`).

### 2.3 The ring, the quantiser and the ramps (Step 2, S-151, S-161, S-166)

- **S-151:** `summoning_ring()` in `effects.py` returns the ring's pixels; `icon_of` and `icon_prop` start from it; `icon_dummy` returns `icon_of(dummy_canvas(), 2, 2)` (`W/edits/s151.py`).
- **S-161:** `white_alpha(x, y, alpha, levels=WHITE_LEVELS)` takes `(alpha, symbol)` pairs faint to strong; `radial_glow` calls it over its steps' pairs, `haze` over its kind's, `hurt_edge` over its reds' (`W/edits/s161.py`). Equal by construction: the same division (the steps' alphas differ by at least 1, so `max(high - low, 1)` was the difference), the same first match, and past the last step the last symbol.
- **S-166:** the nine picks call `pixels.ramp`, imported into `effects.py`: the icon tile's backing with spread 0.28, the liquid with 0.16 (its local `ramp` renamed `symbols`), the snowball with 0.1, its dither moved inside the multiply (0.5 over five symbols; measured equal, A17); the push dust keeps its own dither, which its threshold reads, and calls `ramp` without a spread; the rest without one (`W/edits/s166.py`). Two picks outside the row keep their form: `BRONZE`'s 4.6 scale and `SMOKE`'s step.

### 2.4 The flame ball and the fire ramp (Step 3, S-115)

`actors.flame_ball(t, centre, radius, reach, trail, bend, side, swirls)` returns a ball's heat by pixel on a 64 px frame: the fireball's and the wisp's polar field, its tongues' power `bend`, its side ripple `side` and `swirls` as `(amplitude, lobes, twist, turns)` bands, `h += amplitude * sin(lobes θ + twist r + turns τ t)`, each added in turn as the fireball adds its two. `effects.fire_symbol(heat, x, y, spread=0.08, cutoff=0.1, limits=(0.24, 0.40, 0.56, 0.72, 0.86))` adds the dither, returns None under `cutoff` (never when None), then `M L K J Y` under each limit and `Z` past them. The fireball and the wisp map their heat through it at spread 0.05, no cutoff and `FLAME_BALL = (0.08, 0.2, 0.37, 0.54, 0.7)`; `missile_symbol` at 0.12, cutoff 0.06 and its limits. `actors.py` imports `fire_symbol` at module level: `effects.py` imports only `art` and `pixels` there, and the projectiles and the wisp already use the effects family. The wisp's dither now comes after its swirl, as the fireball's does; all 8 float and 6 flare frames and its icon measured equal (A17; `W/edits/s115.py`).

### 2.5 One clip helper (Step 4, S-55)

`art.clip(images, animations, name, frames, durations, looping=True, names=None, lit=None)` stores `frames` under `names` (`<name>_<i>` by default), adds them to `lit` when given, records `animations[name] = {'looping': …, 'frames': [{'sprite': 'ex10_' + n, 'duration_ms': ms}]}` (one duration for every frame, or one each, zipped strictly), and returns the names. It keeps scenery's `clip` contract, so scenery's own goes and its 25 calls stand. The callers:

- `actors.py`: the wizard's clips (their names by `frame_name`, each frame checked and its anchor kept first), the fireball's, the missile's, the balls' (their first frame's own name) and the enemies' (per-frame durations).
- `scenery.py`: the lanterns' flicker and the ivy's sway (first frames `lantern` and `vines`) and the waterfall.
- `effects.py`: the sprays', the push's, the foes', the flask's and Corrosive's tables, each builder taking `animations` and recording its clips (`build_effects(images, animations)`); the tomes' (`build_tome_art(images, lit, animations)`), the idle frames lit. `art.art_families` passes `animations` to both.
- `generate.py`: `spray_clips` goes with its two calls (`art`'s builder and `floating_report`); `mirror_facing`'s docstring names the builders.

`build_art()` then returns the tables' clips as well; the two tests that read its clips name the wizard's walk and a ball's spin. Every record is the one written today and the JSON is sorted, so `--check` holds every byte.

### 2.6 `tungsten check` after the generator (Step 5, S-131)

`just level-check` runs, in order, `generate.py --check`, `cd examples/01_platformer && cargo run --quiet --locked -p tungsten-cli -- check` (from the game's folder, as the check requires, no target path assumed), and the unit tests. `validate_references` keeps its prefix, prop-animation, ambient-cap and seed checks and drops the three reference loops; `validate_level` keeps the malformed tileset, the collision agreement and the layout rules and drops the tileset tile's sprite, the layer length and the GID checks. Tests: `test_reject_invalid_gid_and_unknown_sprite` goes; `test_reject_layer_length_and_collision_disagreement` keeps its disagreement half as `test_reject_collision_disagreement`; the four `assertIn(config['sprite'], self.manifest['sprites'], name)` lines go, their caps kept. The tools' README and `docs/validation.md` name the check in the level check. Six negative controls, run before the step's first edit on the clean tree, each restored from HEAD (`cmp`): an animation frame's, a particle config's and a tileset tile's sprite set to `ex10_missing`, a material's shader to `missing`, a cell of a tile layer to an unused GID and a layer one cell short: `tungsten check` exits 1 naming each, and `git status --short` is empty after the six.

### 2.7 The scenery's big props whole (Step 6, S-74)

- **Names:** a prop larger than a tile is `<name>_big`, one sprite (`cut`, `placed`), or one clip `<name>_big` of frames `<name>_big_<i>` (`rising_clips`, the pool's shore and basin, through `art.clip`). A prop that fits a tile keeps its name. The prototype's names are these (A13).
- **Geometry:** the padded canvas of whole tiles, as today, so each whole's pixels are its pieces' and it lands where its top-left piece did; the shore's empty tile stays inside its whole.
- **Relief and size:** `art.relief` matches `_big` with any indices after it (`re.search(r'_big(_\d+)*$', name)`), so each whole takes relief normals, over its whole image now; its padding keeps it from wrapping. The backdrops' pieces match as before. `validate_sprites` takes a `_big` sprite at any size of whole tiles.
- **Level:** `level.json`'s 259 pieces become 63 props, each at its first piece's index and its prop's origin, with its group's `support` and `anchor` (an animated whole's `sprite` its first frame, `<name>_big_0`, and its `animation` `<name>_big`, as each piece named its tile clip's); `groups` goes (`W/edits/s6_level.py`, written at the step: it asserts 259 pieces, 63 props, the 57 groups' origins and that every remaining prop is untouched). The props layer goes from 940 objects to 744; the objects after it number 196 lower, which nothing reads (saves name objects, `D-222`). The sheave reads the gantry whole, at the same point.
- **Placement:** `prepare` stands a supported prop whose sprite is `_big` by the group rule, with its own image as the composite and its size in tiles as the box; a prop that fits a tile keeps the single rule. The floating check judges each whole as a single prop by its anchor; the group block, the pieces' `unjudged` and the two group failures go, and `kind()` strips `_big` with its indices.
- **Proof:** `W/edits/s6_check.py` against an export of the base (§3, Step 6): every map layer but the props equal to the base's (tile layers and tilesets exactly, objects but their renumbered IDs, so the sheave's marker too); the props layer the base's with each prop's pieces one object at its origin in its first piece's place; each whole sprite, static or a frame, exactly the canvas its pieces make, its size and every pixel, the shore's empty cell included; each whole clip its pieces' looping and durations with its frames in order; each whole prop's prefab its first piece's with the whole's art, and no piece's prefab left. Tested on the stitched copy (passes) and on an unchanged copy, a changed frame duration and a moved sheave (each fails). Tests: the supported props' test meets the wholes' `wall` anchor, which it judges by `placement.judge` as the floating check does, and its arch count becomes the one whole; a deck's spots read the whole props on it; the vine inside two groups' boxes becomes a whole vine dropped off its support, listed by its anchor. Pins by §2.9.

### 2.8 The iron block whole (Step 7, S-74)

`polish_art.iron_brick()` returns `iron_brick_big`, 128 px and lit, `iron_frost()` `iron_frost_big`. The content prefabs `ex10_iron_brick` (z 13) and `ex10_iron_frost` (z 14, `visible: false`) replace the eight quarters, and `iron_block`'s `spawn_children` names the two at offset (0, 0). `iron_look` tints child 0 as metal and child 1 as frost (`index >= 1`), and `IronBlock`'s doc says so. `ice::frozen_iron_and_scraps_render_and_escaped_scraps_are_cleaned_up` mocks the two sprites and asserts two children, its looks unchanged; one negative control, restored by `cmp`: with the rule at `index >= 2` the test fails on the frost. A save restores a block with its prefab's two (A11). Pins by §2.9; `s6_check.py --iron` holds the quarters inside their two wholes, each exactly the canvas they make.

### 2.9 The pit's and the GPU counters' protocols

- **The pit:** Steps 6 and 7 each fail the guard on one line, `allocated_bytes: 60782105 -> 60780665` and then `60780665 -> 60769145` (A12). The step writes the pin (`TUNGSTEN_GUARDS_WRITE=1` on that test), runs the test twice, and quotes the line. Any other line, or another value, stops the step.
- **GPU counters:** Step 6's `WGPU_BACKEND=vulkan just perf counters` fails on `e01-area-clearing: upload_bytes: the window sums 760032, pinned 816192` alone; the step writes it (`just perf counters --write`) and the next run reads `ok` for all four. The fall is the instances the merged props no longer upload; draw calls and batches hold, since every whole packs as the prototype's did (A13). Step 7 moves none. Another counter or row moving stops the step.

### 2.10 The review, the unlock and the locks (Steps 6 to 8)

- **Unlock:** Step 6 opens with `generate.py --unlock scenery`, Step 7 with `--unlock effects` (`D-223`: the owner's call, made by the run prompt, Q1). Every other family stays locked: `--check` names 19 locked families after Step 6 and 18 after Step 7.
- **Before stills:** at Step 6's start, before its first edit, from 01's folder with `tungsten render --after start_run --at <x,y> --view <h> --frame 120`: each area's big props (the clearing's oaks and roots, the aqueduct's arch, the ravine's vines, plume and splash rocks, the hollow's haze and guardian, the mine's headframe, gantry and carts, the bridge's toll boom and banners) and the pool's shore and basin, about ten stills; an iron block by `cast_at`, plain and under the ice spray, two more. The commands go in the review folder's `shots.sh`, so the after stills are the same frames.
- **Review (Step 8):** after Step 7, the after stills and the before ones side by side, a contact sheet of the wholes (`tungsten render --sheet sprite:ex10_oak_big,…`), under `perf-runs/<date>-7.13f-art-review/` (lean captures: stills only; the haze and the plume are unlit and keep their frames' pixels, and the pool's frames change only in their normals, which a still shows), linked in the conversation with the folder. The owner's verdict is quoted: accepted, `--lock scenery --lock effects`; notes, a round within §2.7 and §2.8's contract that re-shoots only what changed; rejected, Q11.

### 2.11 Records

At the close, `D-29[0]` with the tungsten-decision skill: 01's props larger than a tile drawn, placed and judged whole as `<name>_big`; its art tools' shared helpers (`art.clip`, `flame_ball`, `fire_symbol`'s limits, `white_alpha`'s steps, `summoning_ring`, `pixels.ramp`, `crossfaded`); `just level-check` running `tungsten check`; the families relocked on the owner's acceptance. Its marker line on `D-214` (the floating check's groups gone). Phase 7's §2.2 rows `landed`, §2.8's entry closed, its evidence row "13 (7.13), part f"; ROADMAP's Now and the phase-7 line naming 7.13g next.

## 3. Steps

**The common checks.** Every `python3` and `just` below runs with `target/e01s/venv/bin` first on `PATH` (`export PATH="$PWD/target/e01s/venv/bin:$PATH"` from the repo root), the generator's Pillow. Every step: `just level-check` → exit 0, `--check`'s `OK:` line quoted with the step's count and locks and the unit tests' `Ran N tests … OK`; `just repo-check` and `git diff --check` → exit 0; Python run with `-B`, so no `__pycache__` lands. Before each step, `just preflight docs/plans/example-01-art-tools.md --step N`, and §6's rule for what it lists. Each count and absence check below was run at `50faefa8`, where it reads the "from" value given, and on the probes' copies where they reach. The edit scripts run from the repo root (`python3 -B W/edits/<s>.py .`).

### Step 1: `crossfaded` at its copies

- **Files:** `examples/01_platformer/tools/sfx.py`.
- **Change:** §2.2 (`W/edits/s141.py`).
- **Done-when:** the common checks, `--check` at `OK: 6368 owned outputs match and 20 locked families hold`, tests 46; `rg -c 'blend = i / \((overlap|extra) - 1\)' examples/01_platformer/tools/sfx.py` → `1` (from 5); `rg -c 'crossfaded\(' examples/01_platformer/tools/sfx.py` → `8` (from 4).
- **Commit:** `refactor(example-01): the loops' crossfades on crossfaded`
- **Moves:** none.

### Step 2: The ring, the quantiser and the ramps

- **Files:** `examples/01_platformer/tools/{effects,actors}.py`.
- **Change:** §2.3 (`W/edits/s151.py`, `s161.py`, `s166.py`, in that order).
- **Done-when:** the common checks, `--check` as Step 1's, tests 46; `rg -c 'ring\.ellipse\(\(16, 28\), 13, 3\)' examples/01_platformer/tools/effects.py` → `1` (from 3); `rg -c 'low_alpha, low = 0' examples/01_platformer/tools/effects.py` → `1` (from 3); `rg -q 'low, high = levels\[i\]' examples/01_platformer/tools/effects.py; test $? -eq 1` → exit 0 (from one line); `rg -c '\[min\(len\(\w+\) - 1, (max\(0, )?int\(|\[min\(\d, int\(|\[max\(0, min\(\d, int\(' examples/01_platformer/tools/effects.py examples/01_platformer/tools/actors.py` → `effects.py:1` and `actors.py:1`, `SMOKE`'s and `BRONZE`'s (from 9 and 2).
- **Commit:** `refactor(example-01): the art's ring, alpha steps and ramps shared`
- **Moves:** none.

### Step 3: The flame ball and the fire ramp

- **Files:** `examples/01_platformer/tools/{actors,effects}.py`.
- **Change:** §2.4 (`W/edits/s115.py`).
- **Done-when:** the common checks, `--check` as Step 1's, tests 46; `rg -q "\(0\.\d+, 'M'\)" examples/01_platformer/tools; test $? -eq 1` → exit 0 (from four lines); `rg -c 'flame_ball\(' examples/01_platformer/tools/actors.py` → `3` (from no match); `rg -c 'fire_symbol\(' examples/01_platformer/tools/actors.py` → `3` (from no match).
- **Commit:** `refactor(example-01): one flame ball and fire ramp for the fire art`
- **Moves:** none.

### Step 4: One clip helper

- **Files:** `examples/01_platformer/tools/{art,actors,effects,scenery,generate}.py`, `examples/01_platformer/tools/README.md` (the clip lines).
- **Change:** §2.5.
- **Done-when:** the common checks, `--check` as Step 1's, tests 46; `rg -c "'duration_ms'|duration_ms=" examples/01_platformer/tools --glob '!test_*'` → `examples/01_platformer/tools/art.py:1` alone (from actors 5, scenery 4, generate 1); `rg -q 'spray_clips|^def clip' examples/01_platformer/tools/generate.py examples/01_platformer/tools/scenery.py; test $? -eq 1` → exit 0 (from six lines); `rg -c '^def clip' examples/01_platformer/tools/art.py` → `1` (from no match); `python3 -B examples/01_platformer/tools/generate.py --floating` → `no floating scenery`, exit 0 (as at the base).
- **Commit:** `refactor(example-01): one clip helper writes every clip record`
- **Moves:** none.

### Step 5: `tungsten check` after the generator

- **Files:** `justfile`, `examples/01_platformer/tools/{generate,level,test_generate}.py`, `examples/01_platformer/tools/README.md`, `docs/validation.md`.
- **Change:** the controls first, then §2.6.
- **Done-when:** the common checks, the level check's output now holding `tungsten check: no errors` between `--check` (as Step 1's) and the tests, `Ran 45 tests … OK` (from 46); `rg -c 'tungsten-cli -- check' justfile` → `1` (from no match); `rg -q 'unknown animation sprite|unknown particle sprite|unknown material shader' examples/01_platformer/tools/generate.py; test $? -eq 1` → exit 0 (from three lines); `rg -q 'unknown sprite reference|wrong layer length|invalid GID' examples/01_platformer/tools/level.py; test $? -eq 1` → exit 0 (from three); `rg -q "assertIn\(config\['sprite'\], self\.manifest\['sprites'\]" examples/01_platformer/tools/test_generate.py; test $? -eq 1` → exit 0 (from four); the six controls (§2.6), before the first edit, quoted, each `tungsten check` exiting 1 on its file and the tree restored (`cmp`, then `git status --short` empty); `just script-test` and `just ctx` → exit 0.
- **Commit:** `refactor(example-01): the level check runs tungsten check`
- **Moves:** the unit tests 46 → 45.

### Step 6: The scenery's big props whole

- **Files:** `examples/01_platformer/tools/{scenery,art,placement,level,test_generate}.py`, `examples/01_platformer/tools/level.json`, `examples/01_platformer/tools/locks.json`, `examples/01_platformer/tools/README.md`, the generator's outputs (`examples/01_platformer/assets/`, `examples/01_platformer/tools/outputs.json`), `examples/01_platformer/tests/game/guards/pit.json`, `scripts/perf-rows.json`.
- **Change:** first, the base exported (`mkdir -p W/base && git archive HEAD examples/01_platformer/assets examples/01_platformer/tools | tar -x -C W/base`) and the before stills shot (§2.10); then `--unlock scenery`, §2.7, a full `generate.py` run, the pins by §2.9.
- **Done-when:** `python3 -B examples/01_platformer/tools/generate.py --unlock scenery` → `Locked none; unlocked scenery`; the full run → `Wrote <n> of 5487 outputs, the rest unchanged; removed 1082 obsolete owned files` (883 pieces' files and 199 piece prefabs); the common checks, `--check` at `OK: 5487 owned outputs match and 19 locked families hold`, tests 45, `tungsten check: no errors`; `python3 -B W/edits/s6_check.py W/base/examples/01_platformer examples/01_platformer` → `OK: 259 pieces in 63 props, props 940 -> 744; 480 piece sprites in 64 whole sprites; 5 clips, 42 prefabs and 18 other layers as the base's`, exit 0; `python3 -B examples/01_platformer/tools/generate.py --floating` → `no floating scenery`, exit 0; `jq '[.props[] | select((.sprite + (.animation // "")) | test("_big_[0-9]+_[0-9]+"))] | length' examples/01_platformer/tools/level.json` → `0` (from 259); `jq 'has("groups")' examples/01_platformer/tools/level.json` → `false` (from `true`); `rg -q '_big_\{row\}_\{col\}|_big_0_\{col\}' examples/01_platformer/tools/scenery.py; test $? -eq 1` → exit 0 (from five lines); from `examples/01_platformer`, `cargo test --test game --locked` fails on the pit alone, its line `allocated_bytes: 60782105 -> 60780665`, then with the pin written passes 500 with 1 ignored, twice; `WGPU_BACKEND=vulkan just perf counters` fails on `e01-area-clearing: upload_bytes: the window sums 760032, pinned 816192` alone, then `--write`, then exit 0 with all four `ok`.
- **Commit:** `feat(example-01): the big props drawn and placed whole`
- **Moves:** the scenery family unlocked, its pieces' files replaced by wholes, their normals over the whole; `level.json`'s props and groups; the map's props layer and prefabs; the pit's `allocated_bytes`; `e01-area-clearing`'s `upload_bytes`; the tools' three group tests.

### Step 7: The iron block whole

- **Files:** `examples/01_platformer/tools/polish_art.py`, `examples/01_platformer/tools/content/prefabs/{iron_block,ex10_iron_brick,ex10_iron_frost}.json`, `examples/01_platformer/tools/content/prefabs/ex10_iron_{quarter,frost}_{0,1}_{0,1}.json` (the eight, deleted), `examples/01_platformer/tools/locks.json`, `examples/01_platformer/tools/README.md` (the iron's lines), `examples/01_platformer/src/features/toys/{systems,components}.rs`, `examples/01_platformer/tests/game/ice.rs`, the generator's outputs, `examples/01_platformer/tests/game/guards/pit.json`.
- **Change:** `--unlock effects`, §2.8, a full `generate.py` run, the pit by §2.9.
- **Done-when:** `python3 -B examples/01_platformer/tools/generate.py --unlock effects` → `Locked none; unlocked effects`; the full run → `Wrote <n> of 5469 outputs, the rest unchanged; removed 24 obsolete owned files` (16 quarters' files, 8 prefabs); the common checks, `--check` at `OK: 5469 owned outputs match and 18 locked families hold`, tests 45, `tungsten check: no errors`; `s6_check.py … --iron` → `OK: 259 pieces in 63 props, props 940 -> 744; 488 piece sprites in 66 whole sprites; 5 clips, 42 prefabs and 18 other layers as the base's`; `jq '.components.spawn_children | length' examples/01_platformer/tools/content/prefabs/iron_block.json` → `2` (from 8); `test -z "$(fd -g 'ex10_iron_{quarter,frost}_*' examples/01_platformer)"` → exit 0 (from sixteen files, eight in `tools/content/prefabs/` and eight in `assets/prefabs/`); `rg -q 'index >= 4' examples/01_platformer/src/features/toys/systems.rs; test $? -eq 1` → exit 0 (from one line); `cargo fmt --all -- --check` and `just lint` → exit 0; from `examples/01_platformer`, `cargo clippy --all-targets -- -D warnings` → exit 0; `cargo test --test game --locked` fails on the pit alone, `allocated_bytes: 60780665 -> 60769145`, then with the pin written passes 500 with 1 ignored, twice, the ice test's line quoted (`rtk proxy cargo test …`); its negative control (§2.8) quoted and restored; `WGPU_BACKEND=vulkan just perf counters` → exit 0, all four `ok`.
- **Commit:** `feat(example-01): the iron block drawn whole, its brick and frost`
- **Moves:** the effects family unlocked, the iron's quarters replaced by two wholes; the iron's prefabs and children; the pit's `allocated_bytes`.

### Step 8: The owner's review and the locks

- **Files:** `examples/01_platformer/tools/locks.json`; the review folder (`perf-runs/<date>-7.13f-art-review/`, untracked); a round's files are Steps 6 and 7's.
- **Change:** §2.10's review: the after stills by `shots.sh`, the sheet, both linked; the owner's verdict; on acceptance the locks of the families that landed whole, never one that did not move.
- **Done-when:** the verdict quoted in the evidence log. With Steps 6 and 7 landed: `python3 -B examples/01_platformer/tools/generate.py --lock scenery --lock effects` → `Locked scenery, effects; unlocked none`; the common checks, `--check` at `OK: 5469 owned outputs match and 20 locked families hold`; `jq '(.scenery.files | length), (.effects.files | length)' examples/01_platformer/tools/locks.json` → `1263` and `311` (from 1,987 and 323). With Step 7 skipped (the effects family locked as it was, §6): `--lock scenery` alone → `Locked scenery; unlocked none`; `--check` at `OK: 5487 owned outputs match and 20 locked families hold`; the `jq` → `1263` and `323`; `diff <(jq -S .effects examples/01_platformer/tools/locks.json) <(git show <Step 5's commit>:examples/01_platformer/tools/locks.json | jq -S .effects)` → exit 0.
- **Commit:** `feat(example-01): lock the accepted whole props (D-223)`
- **Moves:** `locks.json`'s scenery and effects entries, today's date and their new hashes.

### Step 9: Close

- **Files:** this plan (deleted), `docs/plans/phase-7.md`, `ROADMAP.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, the follow-ups' homes (`docs/known-issues.md` or ROADMAP's Later).
- **Change:** §2.11's records; the follow-ups moved; the plan and its citations deleted; `example-01-art-tools` out of Now.
- **Done-when:** `just check` → exit 0; `PATH="$PWD/target/e01s/venv/bin:$PATH" just level-check` → exit 0; `just smoke` → passes; `WGPU_BACKEND=vulkan just perf counters` → exit 0; `just repo-check`, `just ctx` and `git diff --check` → exit 0; before the deletion, `rg -c '^\| (S-55|S-74|S-115|S-131|S-141|S-151|S-161|S-166) \| [^|]+ \| 7\.13 \| landed ' docs/plans/phase-7.md` → `8`, or the evidence log naming each skipped one; after it, `rg -l 'plans/example-01-art-tools\.md' --glob '!CHANGELOG.md' .; test $? -eq 1` → exit 0; the demo, from `examples/01_platformer` by `tungsten run --script <script> --report <report>` → exit 0: a run past the clearing's oaks, the hollow's haze and the mine's gantry by `place_player`, an iron block by `cast_at`, each captured, the stills a sheet under `perf-runs/<date>-7.13f-art-tools/`.
- **Ends with:** the commits, the evidence, what the owner should try (the six areas' big props lit as the wizard's lantern passes them, the iron block frosted and thawed, `just level-check`'s new line), the next prompt (tungsten-next's, by phase 7's §2.8: 7.13g).

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Who unlocks the scenery and effects families (`D-223`: the owner's call)? | The run prompt says so in the owner's words, so pasting it is the call; Steps 6 and 7 run the unlocks. An owner who would rather unlock by hand deletes that sentence, and Step 6 stops at its first check |
| Q2 | The owner's review inside the run (`D-150` has no approval stops) | Asked at Step 8: the art's review and lock are the redo's §6 and `D-223`, which `D-150` leaves standing. A session that ends without a verdict leaves both families unlocked at Step 7's commit; Step 8 resumes in a new small session, and the close waits for it |
| Q3 | Whole props' names: `<name>_big`, or the bare name | `<name>_big`: the relief rule, the size rule and the floating check's `kind` read it, where a bare name would need a list of every prop drawn whole; the prototype's counters are for these names |
| Q4 | A whole prop's padding to whole tiles: kept or trimmed | Kept: each lands where its top-left piece did and the group rule's box is unchanged; trimming saves atlas space only and moves every whole's point |
| Q5 | The snowball's dither inside `ramp` (spread 0.1) | Inside: measured byte-equal on its four frames (A17) |
| Q6 | `hurt_edge`'s copy of the quantiser, outside the register's Where | Taken: the register's own count of 4 names it, and it is one more call of the same helper |
| Q7 | The push dust's dither, read by its threshold too | Kept on its value, `ramp` without a spread: the threshold and the pick read the same dithered value as today |
| Q8 | The level check's `tungsten check`: `cargo run -p tungsten-cli` or a built binary's path | `cargo run --quiet --locked -p tungsten-cli -- check` from 01's folder: no target directory assumed |
| Q9 | A supported prop drawn whole: placed by the group rule or the single rule | The group rule: positions unchanged (the geometry check); the single rule's centre test and covering stair would move or refuse some |
| Q10 | The iron's children told apart by index or by prefab | By index, as today (`index >= 1`): the prefab fixes their order |
| Q11 | S-74 rejected at review | Steps 6 and 7 reverted file by file to Step 5's commit (each file by `git show`, each added file deleted), `locks.json` back to Step 5's, S-74 `open` with a Later line naming the owner's notes; Steps 1 to 5 stand |
| Q12 | The before stills: at Step 6's start, or later from an export of the base | At Step 6's start, from the tree as it stands: the same binary and folder the after stills use |

Each question takes its stated default; the owner reviews the plan on their own time.

## 5. Invariants

- **May move:** the scenery family's files and its lock at Step 6, the effects family's at Step 7, both relocked at Step 8; `level.json`'s props and groups, the map's props layer and the props' prefabs (Step 6); the iron's content prefabs (Step 7); the pit's `allocated_bytes` at Steps 6 and 7 by §2.9; `e01-area-clearing`'s `upload_bytes` at Step 6; the tools' tests §2 names; the unit tests 46 → 45 at Step 5.
- **Must not move:** every owned output's bytes at Steps 1 to 5; every other family's files and lock at every step; the engine, the kit, the template and `api/`; 01's save schema 41 and every save's bytes; the physics hashes; 02's guards; the pit's other lines at every step and all of it at Steps 1 to 5; every other GPU counter; the map's tile layers, `tools/tileset.json` and every object outside the props layer but its number; every perf row's script and `workload_version`.

## 6. Stop conditions

- A done-when check fails: restore the step's files from the last commit (`git show HEAD:<path> > <path>`), delete what it added, keep the built step under `W/` with its probes, taken before the tree goes back (`rtk proxy git diff HEAD -- <the step's paths> > <file>`), mark it skipped in the evidence log, commit and push that, and continue with the steps that do not depend on it. A step that ran `--unlock` and stops restores `locks.json` with the rest.
- Dependencies: Steps 1 to 5 on nothing but order; 6 on 4 and 5; 7 on 6; 8 on 6 or 7, whichever landed (a skipped 7 leaves the effects family locked as it was); 9 on all. A skipped step's rows, and its dependents', stay `open` at 7.13 with their move to a re-plan, which the close names in phase 7's §2.8 ahead of 7.13g.
- A file outside the step's list: stop the step, except a file only the step's own change forces (a pinned name, count or message, a doc line naming a removed item), which joins with its reason in the evidence row; functional reach still stops.
- At Steps 1 to 5, any output byte that moves (`--check` naming a file, or a lock breaking): stop the step; the helper is not byte-equal, so the copy stays.
- The pit or a GPU counter outside §2.9: another line, row or value stops the step, its reading in the evidence row.
- A 01 test beyond the iron test and the pit failing at Steps 6 or 7, or a tools test beyond the three §2.7 names: stop the step, without editing the test to pass.
- The geometry check failing: stop the step; a whole that moves a pixel or an object is a redraw, outside S-74.
- The Pillow venv missing: recreate it (`python3 -m venv target/e01s/venv && target/e01s/venv/bin/pip install -r examples/01_platformer/tools/requirements.txt`); a failed install stops the run before Step 1.
- The owner rejecting S-74: Q11.
- A step whose done-when cannot pass as written (a check matching what the step must keep): stop before building, as the 7.13c precedent has it (`402ba4ee`).

## 7. Critique

Round 1: gpt-6-astra through `critique --raw --repo`, xhigh, 2026-11-18, the tree at `50faefa8` with this draft (42.5k characters, under the size guard) and a focus note naming `W/NOTES.md` and six mechanisms to attack; footer: `393.8 s, tokens in 1477085 (cached 1364224), out 13650 (reasoning 6823), commands 35, output 435.4 KB (largest 54 KB), usage 5h 0% → 32%, week 47% → 52%`. It gave four findings, all taken; three changed a done-when check, so round 2 ran on the plan without this section.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | Medium: the geometry check reads the props layer alone (class and point), missing the other layers, the sheave's marker this step recomputes among them, and compares only the crops of pieces that exist, leaving the shore's omitted tile and each whole's size unchecked | round 1's `W/edits/s6_check.py:13`, `:33`, `:42`; `T/level.py:323-325`; `T/scenery.py:4286-4290` (the shore's pieces start at (0, 1)) | True | `s6_check.py` compares every layer (tile layers and tilesets exactly, objects but their IDs) and each whole sprite with the whole canvas its pieces make, its size and every pixel, empty cells included (§2.7; Steps 6 and 7's lines); passes on the stitched copy, fails on an unchanged copy and on a moved sheave |
| 2 | Medium: nothing checks the merged clips' frame order, durations and looping or the prefabs' bindings, and `--check` with scenery unlocked proves agreement with the new generator only | round 1's `W/edits/s6_check.py:38`; `T/generate.py:1247`; `W/probes/stitch_whole.py:53` | True | the check holds each whole clip to its pieces' one looping and durations, its frames `<name>_big_<i>` in order, and each whole prop's prefab to its first piece's with the whole's art, no piece's prefab left; a changed duration fails it |
| 3 | Medium: Step 8 after Step 6 alone cannot pass its 5,469 outputs and 311 effects files, and `--lock effects` would rewrite the unchanged family's date | §6's dependencies; `T/generate.py:1188` | True | Step 8's done-when in two forms; with Step 7 skipped it locks scenery alone, at 5,487 outputs, the effects entry diffed equal to Step 5's |
| 4 | Medium: Step 5's controls require `git status --short` empty while the step's own edits are still uncommitted | Step 5's done-when and its commit after the checks | True | the controls run before the step's first edit, on the clean tree (§2.6, Step 5) |

Round 2: the same reviewer at xhigh, 2026-11-18, the plan without this section and the evidence log (`--from sed`, 45.2k characters, under the size guard), with a focus note naming round 1's three mechanism changes; footer: `168.9 s, tokens in 449191 (cached 382720), out 4907 (reasoning 2742), commands 11, output 174.1 KB (largest 34 KB), usage 5h 32% → 45%, week 52% → 54%`. It found the rewritten check covering whole canvases, clip timing and order, prefab bindings and the other layers with the sheave's marker, Step 8's scenery-only path keeping the effects lock and Step 5's controls ahead of its edits, and gave no finding; no third round.

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-11-18 | landed | `just level-check` exit 0 (545 s): `OK: 6368 owned outputs match and 20 locked families hold; art, level, references and budgets validated`, `Ran 46 tests in 430.875s` `OK`; `just repo-check` and `git diff --check` exit 0, no `__pycache__`; `rg -c 'blend = i / \((overlap\|extra) - 1\)' …/sfx.py` → `1`; `rg -c 'crossfaded\(' …/sfx.py` → `8`; `sfx.py` byte-equal (`cmp`) to `W/edits/s141.py` rehearsed on an export of HEAD; preflight: R7 (`docs/plans/phase-7.md`, its rows at the close) alone | `examples/01_platformer/tools/sfx.py` |
| 2 | 2026-11-18 | landed | `just level-check` exit 0 (544 s): `OK: 6368 owned outputs match and 20 locked families hold; art, level, references and budgets validated`, `Ran 46 tests in 429.720s` `OK`; `just repo-check` and `git diff --check` exit 0, no `__pycache__`; `rg -c 'ring\.ellipse\(\(16, 28\), 13, 3\)' …/effects.py` → `1`; `rg -c 'low_alpha, low = 0' …/effects.py` → `1`; `rg -q 'low, high = levels\[i\]' …/effects.py; test $? -eq 1` → exit 0; the ramp count → `effects.py:1` (`SMOKE[min(3, int(sd * 2.4) + (t > 0.8))]`) and `actors.py:1` (`BRONZE[min(4, int(light * 4.6))]`); both files byte-equal (`cmp`) to `W/edits/s151.py`, `s161.py`, `s166.py` rehearsed in order; preflight: R7 alone, the rest asset-name hits | `examples/01_platformer/tools/effects.py`, `examples/01_platformer/tools/actors.py` |

## Follow-ups

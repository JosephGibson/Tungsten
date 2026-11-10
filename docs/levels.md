# Levels

How a game's levels are made, changed and checked: room and tileset files, the `tungsten level` command line, a script's level steps and the in-game editor. [Assets](assets.md) lists every field of the two formats; [getting started](getting-started.md) covers the rest of a game. Every surface edits a room through the same commands, so an edit is checked, journaled and undone alike wherever it is made (`D-264`, `D-267`).

## A room, its tileset and the manifest

A room is one JSON file, listed in the manifest's `rooms` by its ID. Its tiles come from a tileset file, listed in `tilesets`, which names each tile by a sprite's ID. `tungsten level new yard --size 24x14 --cell 32 --tile dummy` writes the pair to start from, a room with a render layer `art` and a collision layer `solid` and a tileset `tiles` holding that tile, and lists both in the manifest; batches build the rest. Written by hand, the smallest pair that draws and collides, on a new game's own sprites:

```json
{ "version": 1, "tiles": [{ "sprite": "dummy" }] }
```

```json
{
  "version": 1,
  "cell": [32, 32],
  "size": [24, 14],
  "tileset": "tiles",
  "layers": [
    { "name": "art" },
    { "name": "solid", "kind": "collision" }
  ],
  "shapes": [
    { "id": "floor", "rect": [0, 12, 24, 2], "tile": "dummy", "layers": ["art", "solid"] }
  ],
  "objects": [{ "id": "gem_1", "prefab": "gem", "at": [11, 11] }],
  "places": [{ "id": "start", "point": [2, 11] }]
}
```

```json
"tilesets": { "tiles": { "path": "tilesets/tiles.json" } },
"rooms": { "yard": { "path": "rooms/yard.json" } }
```

The conventions every surface shares:

- Positions are in cells from the room's top-left corner, x to the right and y down; `cell` is a cell's size in pixels and `size` the room's in cells.
- A `rect` is `[col, row, width, height]`. A shape's rect, polygon and line take whole cells.
- An object's `at` and a place's `point` are in cells too, fractions allowed: `at` times `cell` is where the prefab's sprite draws from, its top-left unless the sprite has an anchor. An object one cell tall standing on a floor whose top row is 12 has `at` row 11.
- IDs, layer names and room IDs are `[a-z][a-z0-9_]*`. An ID is unique across the room's shapes, objects and places. In the running game an object is named `<room>.<id>`.
- A layer is drawn (`render`, the default) or collided with (`collision`); terrain that does both paints into one of each, as `floor` does above. A collision layer alone draws nothing.
- Terrain is `shapes` painted in order into the layers each names, then each layer's `cells`, overrides keyed `"<col>,<row>"` that name a tile or `null` for empty.
- An object's `set` overrides its prefab's components by dotted path: `"set": { "health.max": 50 }`.
- A place is a `point`, a `rect` or a `route`; a point's or an area's `to` links it to `<room>.<place>`.

`tungsten schema room` and `tungsten schema tileset` print each format's JSON Schema, every field with its description.

The game spawns a room by its ID, as it spawns a map: `world.spawn_room("yard", [0.0, 0.0], &(SceneEntity { state_id: GAMEPLAY },))`, the bundle going to the room's instance and to each object. A room is live once at a time.

## `tungsten level`

Run it in the game's folder. A `<room>` is a `rooms` ID or a room file's path. `tungsten level --help` prints a worked example, every verb and the batch commands, and `tungsten level <verb> --help` one verb's; a mistake is answered with its reason and that verb's usage alone.

| Verb | What it does | Needs the game's build |
| --- | --- | --- |
| `new <room>` | Makes a room file, `rooms/<room>.json` beside the manifest, with a render layer `art` and a collision layer `solid`, and lists it in the manifest, keeping the manifest's formatting. `--size <cols>x<rows>` (40x24), `--cell <px>` or `<w>x<h>` (the cell of the tileset's other rooms, else 16) and `--tileset <id>` (the game's only tileset) set it. With no tileset listed, or a `--tileset` the manifest lacks, it also writes that tileset, `tiles` by default, with a tile for each `--tile <sprite>`. It refuses an ID a room or a tilemap holds and a file that exists; `--dry-run` reports and writes nothing. | no |
| `apply <room> <batch>` | Applies a batch of commands, a JSON array in a file or on stdin for `-`, writes the room in its canonical form and journals the batch. `apply <room> --undo` undoes the journal's newest batch and `apply <room> --redo` applies the newest undone one again. `--dry-run` checks and reports what would change and writes nothing; `--check` runs `level check` on the room after the write; `--inverse <file>` also writes the batch that undoes this one. | no |
| `query <room>` | Lists every layer, shape, object and place with its geometry, a line each. `query <room> <id>` prints one entry, and `query <room> --at <col>,<row>` what lies at a cell: each layer's tile, the shapes over it, the objects in it and the places holding it. `--json` prints JSON. | no |
| `map <room>` | Prints the room as text, a character a cell: `#` solid, `=` one-way, `/` and `\` slopes, `+` drawn only, `.` empty, a letter for each object and a digit for each place, listed under the map. `--rect <col>,<row>,<width>,<height>` prints those cells, which a room past 200 by 100 cells needs. | no |
| `describe [<room>]` | Lists each room's layers and named things by kind, with its links. `--json` prints JSON. | no |
| `check [<room>...]` | Runs the game's check and prints those rooms' findings, every room's with none named, and every room generator's (`--seeds`, `--seed` as `check` takes them). | yes |
| `render <room> -o <png>` | Stages the room on an empty stage and captures it, with `--overlay`, `--size`, `--frame`, `--save-script` and `--release` as `render` takes them. `--overlay physics` outlines what collides; `--save-script <file>` keeps the script it ran, its `stage` and `room` steps a start for one of your own. | yes |
| `generate <generator>` | Runs a room generator and writes its rooms as files (below). | yes |

Exit codes: 0 when it worked, 1 when it found errors or failed, 2 on a usage mistake.

`map` is the quickest look at a room without a render. The room above, a second gem added:

```text
$ tungsten level map yard --rect 0,9,24,5
yard: 24 × 14 cells of 32 × 32 px, cells 0,9 to 23,13
# solid  = one-way  / \ slope  + drawn only  . empty  a-z objects  1-9 places
   0         1         2
   012345678901234567890123
 9 ........................
10 ........................
11 ..1........a.b..........
12 ########################
13 ########################
objects:
  a gem_1 (gem) at 11, 11
  b gem_2 (gem) at 13, 11
places:
  1 start point at 2, 11
```

### Batches

A batch is a JSON array of commands, each an object with one verb. `tungsten schema batch` prints its JSON Schema, every field described and every verb with an example. Every command is checked against the room as the earlier ones leave it, and nothing is written unless all pass; an error names its command and field, as `commands[1].move.id`, and the nearest name when one is near: ``no entry `gaurd`; did you mean `guard`?``. A tile the tileset lacks lists a small tileset's tiles.

| Verb | Fields | Example |
| --- | --- | --- |
| `place` | one of `objects`, `shapes`, `places` (an entry as the room file writes it) or `layers` (a layer); `index` in its list, its end when absent | `{ "place": { "objects": { "id": "gem_2", "prefab": "gem", "at": [13, 11] } } }` |
| `move` | `id`, and `by` an offset or `to` a point, in cells; a shape moves by whole cells and carries the objects `on` it | `{ "move": { "id": "gem_2", "by": [3, 0] } }` |
| `set` | `path` and `value`, on entry `id`, on layer `layer`, or on the room itself (`size`, `tileset`, `post`, `ambient`, `budget`) with neither; `set.<key>` is one override of an object; `null` removes an optional field | `{ "set": { "id": "guard", "path": "set.health.max", "value": 50 } }` |
| `delete` | `id` or `layer` | `{ "delete": { "id": "gem_2" } }` |
| `duplicate` | `id`, `as` the copy's ID, `by` its offset | `{ "duplicate": { "id": "gem_1", "as": "gem_2", "by": [2, 0] } }` |
| `paint` | `layer`, `rect` or `cells` (`[col, row]` each), `tile` by its sprite; `"tile": null` removes the overrides | `{ "paint": { "layer": "solid", "rect": [4, 12, 2, 2], "tile": "dummy" } }` |
| `erase` | `layer`, `rect` or `cells`: empty overrides | `{ "erase": { "layer": "solid", "rect": [4, 12, 2, 2] } }` |
| `link` | `id` of a point or an area, `to` `<room>.<place>` or `null` | `{ "link": { "id": "exit", "to": "cellar.door" } }` |
| `rename` | `id`, `to`; the room's own references follow | `{ "rename": { "id": "dummy_2", "to": "guard" } }` |

`apply` answers with a line a command and the commands that undo and check it:

```text
$ tungsten level apply yard - <<<'[{ "move": { "id": "gem_1", "by": [1, 0] } }]'
applied 1 command to assets/rooms/yard.json:
  move `gem_1` by 1, 0
undo: tungsten level apply yard --undo
```

`apply` checks what the room file alone decides: IDs, geometry, layers, tiles and prefab IDs. An override's path and value are checked against the game's components by `tungsten level check`, which needs the game's build: a batch that places an object or sets an override names that check in its answer, and `--check` runs it at once.

`apply` appends each batch with the one that undoes it to `target/tungsten/journal/<room>.jsonl`, with the room file's hashes before and after; `--undo` refuses a file edited since, and moves the batch to `<room>.redo.jsonl`, which `--redo` reads and a new `apply` empties. `--redo` refuses a file edited since its undo. Each of the three writes the room file and its journals as one set: when a write fails, the files written before it are put back and the command fails with nothing changed.

### Findings

`tungsten check` and `tungsten level check` hold each room to the level rules: an object's dynamic or kinematic body clear of the terrain it meets, each prop with the `support` its prefab names, each link's room and place there, the budget kept and, with the kit, each route with a `by` reachable by that prefab's mover. A finding prints as `<file>: <room>: <id>: <path>: <message>`, and so does an object whose prefab is unknown or whose override names a field its component lacks: ``assets/rooms/yard.json: yard: guard: objects[1]: prefab `dummy`: component `health`, field `maxx`: unknown field `maxx`, expected one of `max`, …``. A file that does not load prints the loader's error under the same file.

### Generated rooms

`tungsten level generate <generator> [--seed <seed>] [--prefix <prefix>]` runs a room generator in the game's build (the game checks clean first) and writes its rooms as room files: a new room as `<into>/<id>.json` (`--into`, the manifest's `rooms/` by default), with the manifest entry to add printed, since it never edits the manifest. Every object it writes carries `"generator"`, and a hand edit through the commands (`move`, `set`, `rename`) drops the mark. A room the manifest lists is rewritten only with `--regenerate`, keeping the objects that do not carry the generator, so moved or placed objects survive while the terrain, places and the generator's own objects are the new output's. It writes no file until every room's file is decided, refuses a file another listed room or a template shares, and puts back the files written before a write that fails. Git undoes a generation.

## In a script

`tungsten run --script` stages and edits rooms in the running game ([getting started](getting-started.md#script-render-and-describe) has the script's other verbs, and `tungsten schema script` every verb's value with an example):

| Step | Value |
| --- | --- |
| `stage` | `{}`: clears the game's states for an empty stage, where the steps below spawn. |
| `room` | a `rooms` ID, spawned at the origin. |
| `level` | `{ "room": "<id>", "apply": [<commands>] }` applies a batch to the live room and journals it; `{ "room": "<id>", "undo": true }` undoes its newest batch and `{ "room": "<id>", "redo": true }` applies the newest undone one again. No file is written, so a script never changes a room's file. |
| `generate` | `{ "generator": "<id>", "seed": <n>, "prefix": "<id>" }`: generates rooms and spawns each at its origin. |

Two probes read a room back:

- `room` is the script's `query`. `{ "room": "yard" }` gives the room whole: `live`, `origin`, `cell`, `size`, `tileset`, its `layers`, its `shapes`, `objects` and `places` as the file writes them, and its journal's `undo` and `redo` counts. While the room is live each object also carries `world`, its spawn point in world pixels, and `alive`, whether the entity it spawned as still lives. `{ "room": "yard", "id": "gem_1" }` gives one entry as `{kind, index, entry}`; a `path` picks a field, as `/entry/at`.
- `place` reads a live room's place in world pixels: `{ "room": "yard", "id": "start" }` gives a point `[x, y]`, an area `{min, size}` or a route's points.

```json
{
  "version": 1,
  "frames": 30,
  "steps": [
    { "frame": 1, "stage": {} },
    { "frame": 2, "room": "yard" },
    { "frame": 5, "assert": { "probe": "place", "args": { "room": "yard", "id": "start" }, "op": "==", "value": [64.0, 352.0] } },
    { "frame": 6, "level": { "room": "yard", "apply": [{ "move": { "id": "gem_1", "by": [1, 0] } }] } },
    { "frame": 8, "assert": { "probe": "room", "args": { "room": "yard", "id": "gem_1" }, "path": "/entry/at", "op": "==", "value": [12, 11] } },
    { "frame": 10, "capture": "yard.png" },
    { "frame": 12, "level": { "room": "yard", "undo": true } },
    { "frame": 14, "assert": { "probe": "room", "args": { "room": "yard" }, "path": "/redo", "op": "==", "value": 1 } }
  ]
}
```

## While the game runs

A debug build watches its assets. Save a room file, or its tileset, and the live room changes in place: its edited cells, and its objects despawned, spawned or spawned again where the file changed them; a file that fails to load keeps the room as it was.

## The editor

F10 opens the level editor in a build that watches its assets: the game clock stops and a panel takes the window's right side. Every action is a button or a row of the panel, and the keys are listed at its foot. From the top it shows:

- **What happened.** The last edit as a sentence ("Moved `crate_1` by 1, 0."), or in red why one was refused ("Can't move `crate_1`: [5, 31] is past the room's 40 × 24 cells."). Under it, what a click does with the current tool and the cell under the cursor. A game with no live room says so here, with the rooms it lists.
- **Rooms.** The live rooms, the current one marked, each with the file its edits write to or `read only` (a generated room, and one whose file another room shares). A click makes a room current and fits it in view; otherwise the current room is the one under the view's centre.
- **Tools.** Select, Place, Paint and Erase, then Undo and Redo with their counts, Delete, Copy, Fit and Game view. A button that has nothing to act on is dimmed.
- **The palette.** The prefabs with their sprites under Select and Place, the room's tiles under Paint and Erase. A click takes one and takes up its tool.
- **Layer.** The layer Paint and Erase write to, or every layer.
- **Pick, findings.** The picked object's ID, prefab and cell, and the room's findings as `tungsten check` reports them, read again after each edit.

| Input | Does |
| --- | --- |
| left button | Select: a click picks an object, a drag moves it by whole cells. Place: the palette's prefab at the cell. Paint and Erase: a stroke of cells, one undo step. |
| right button | takes the prefab of the object, or the tile and layer of the cell, under the cursor |
| middle drag, arrows, wheel | pan and zoom |
| `1` `2` `3` `4` | Select, Place, Paint, Erase |
| `[` `]`, `L` | step the palette; step the layer |
| Delete, Ctrl+D | delete the pick; copy it a cell right |
| Ctrl+Z, Ctrl+Y | undo, redo |
| `F`, Home | fit the current room beside the panel; back to the game's view |
| F10 | close |

Each edit is one batch: checked as `tungsten level apply` checks it, journaled, and written to the room's file at once, which the reload then finds unchanged. A refused edit changes nothing, and a file that can't be written takes the edit back out of the room. The editor edits rooms the game has spawned.

A script drives the editor through the `editor` action, which does what the buttons do, and reads it through the `editor` probe:

```json
{ "frame": 5, "action": "editor", "args": { "open": true, "tool": "paint", "tile": "rock", "layer": "solid" } },
{ "frame": 9, "action": "editor", "args": { "select": "gem_1", "do": "copy" } },
{ "frame": 10, "assert": { "probe": "editor", "path": "/status", "op": "==", "value": "Copied `gem_1` as `gem_2`, a cell right." } }
```

The action's fields, each optional, apply in this order: `open`, `room`, `tool`, `prefab`, `tile`, `layer` (a name, or `null` for every layer), `select` (an object's ID, or `null`), `do` (`undo`, `redo`, `delete`, `copy`, `fit`, `game_view`, `palette_next`, `palette_previous`, `layer_next`). One the editor refuses fails the step with the sentence the panel shows. The probe gives `available`, `open`, `tool`, `room`, `rooms`, `layer`, `prefab`, `tile`, `selection`, `status`, `refused`, `hint`, `undo`, `redo`, `findings` and `view`. Unlike a `level` step, an edit made through the editor writes the room's file. A click in the world is a `cursor` step and a press of `engine_editor_primary`, as any input is.

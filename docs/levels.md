# Levels

How a game's levels are made, changed and checked: room and tileset files, the `tungsten level` command line, a script's level steps and the in-game editor. [Assets](assets.md) lists every field of the two formats; [getting started](getting-started.md) covers the rest of a game. Every surface edits a room through the same commands, so an edit is checked, journaled and undone alike wherever it is made (`D-264`, `D-267`).

## A room, its tileset and the manifest

A room is one JSON file, listed in the manifest's `rooms` by its ID. Its tiles come from a tileset file, listed in `tilesets`, which names each tile by a sprite's ID. The smallest pair that draws and collides, on a new game's own sprites:

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

Run it in the game's folder. A `<room>` is a `rooms` ID or a room file's path.

| Verb | What it does | Needs the game's build |
| --- | --- | --- |
| `apply <room> <batch>` | Applies a batch of commands, a JSON array in a file, writes the room in its canonical form and journals the batch. `--inverse <file>` also writes the batch that undoes it; `apply <room> --undo` undoes the journal's newest batch. | no |
| `query <room> <id>` | Prints one entry. `query <room> --at <col>,<row>` prints what lies at a cell: each layer's tile, the shapes over it, the objects in it and the places holding it. `--json` prints JSON. | no |
| `describe [<room>]` | Lists each room's layers and named things by kind, with its links. `--json` prints JSON. | no |
| `check [<room>...]` | Runs the game's check and prints those rooms' findings, every room's with none named, and every room generator's (`--seeds`, `--seed` as `check` takes them). | yes |
| `render <room> -o <png>` | Stages the room on an empty stage and captures it, with `--overlay`, `--size`, `--frame` and `--release` as `render` takes them. `--overlay physics` outlines what collides. | yes |
| `generate <generator>` | Runs a room generator and writes its rooms as files (below). | yes |

Exit codes: 0 when it worked, 1 when it found errors or failed, 2 on a usage mistake.

### Batches

A batch is a JSON array of commands, each an object with one verb. Every command is checked against the room as the earlier ones leave it, and nothing is written unless all pass; an error names its command and field, as `commands[1].move.id`, and the nearest name when one is near: ``no entry `gaurd`; did you mean `guard`?``. A tile the tileset lacks lists a small tileset's tiles.

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

`apply` checks what the room file alone decides: IDs, geometry, layers, tiles and prefab IDs. An override's path and value are checked against the game's components by `tungsten level check`, which needs the game's build, so run it after a batch that sets one.

`apply` appends each batch with the one that undoes it to `target/tungsten/journal/<room>.jsonl`, with the room file's hashes before and after; `--undo` refuses a file edited since.

### Findings

`tungsten check` and `tungsten level check` hold each room to the level rules: an object's dynamic or kinematic body clear of the terrain it meets, each prop with the `support` its prefab names, each link's room and place there, the budget kept and, with the kit, each route with a `by` reachable by that prefab's mover. A finding prints as `<file>: <room>: <id>: <path>: <message>`, and so does an object whose prefab is unknown or whose override names a field its component lacks: ``assets/rooms/yard.json: yard: guard: objects[1]: prefab `dummy`: component `health`, field `maxx`: unknown field `maxx`, expected one of `max`, …``. A file that does not load prints the loader's error under the same file.

### Generated rooms

`tungsten level generate <generator> [--seed <seed>] [--prefix <prefix>]` runs a room generator in the game's build (the game checks clean first) and writes its rooms as room files: a new room as `<into>/<id>.json` (`--into`, the manifest's `rooms/` by default), with the manifest entry to add printed, since it never edits the manifest. Every object it writes carries `"generator"`, and a hand edit through the commands (`move`, `set`, `rename`) drops the mark. A room the manifest lists is rewritten only with `--regenerate`, keeping the objects that do not carry the generator, so moved or placed objects survive while the terrain, places and the generator's own objects are the new output's. It writes no file until every room's file is decided, refuses a file another listed room or a template shares, and puts back the files written before a write that fails. Git undoes a generation.

## In a script

`tungsten run --script` stages and edits rooms in the running game ([getting started](getting-started.md#script-render-and-describe) has the script's other verbs):

| Step | Value |
| --- | --- |
| `stage` | `{}`: clears the game's states for an empty stage, where the steps below spawn. |
| `room` | a `rooms` ID, spawned at the origin. |
| `level` | `{ "room": "<id>", "apply": [<commands>] }` applies a batch to the live room and journals it; `{ "room": "<id>", "undo": true }` undoes its newest batch. No file is written, so a script never changes a room's file. |
| `generate` | `{ "generator": "<id>", "seed": <n>, "prefix": "<id>" }`: generates rooms and spawns each at its origin. |

The `place` probe reads a live room's place in world pixels: `{ "probe": "place", "args": { "room": "yard", "id": "start" } }` gives a point `[x, y]`, an area `{min, size}` or a route's points.

```json
{
  "version": 1,
  "frames": 30,
  "steps": [
    { "frame": 1, "stage": {} },
    { "frame": 2, "room": "yard" },
    { "frame": 5, "assert": { "probe": "place", "args": { "room": "yard", "id": "start" }, "op": "==", "value": [64.0, 352.0] } },
    { "frame": 6, "level": { "room": "yard", "apply": [{ "move": { "id": "gem_1", "by": [1, 0] } }] } },
    { "frame": 10, "capture": "yard.png" },
    { "frame": 12, "level": { "room": "yard", "undo": true } }
  ]
}
```

## While the game runs

A debug build watches its assets. Save a room file, or its tileset, and the live room changes in place: its edited cells, and its objects despawned, spawned or spawned again where the file changed them; a file that fails to load keeps the room as it was.

## The editor

F10 opens the level editor in a build that watches its assets: the game clock stops, and its window lists the keys. Drag an object to move it by whole cells, Delete removes the picked one, Ctrl+D copies it a cell right, Ctrl+Z and Ctrl+Y undo and redo, the arrows and a middle drag pan and the wheel zooms. The keys 1 to 4 switch the left button between select, place (the palette's prefab at a cell), paint and erase (a stroke of cells, one undo step), `[` and `]` step the palette through the prefabs or the room's tiles, L picks the target layer, and the right button takes the prefab or the tile under the cursor. After each edit the window shows the room's findings, as `tungsten check` reports them. Each edit is checked as `tungsten level apply` checks a batch and written to the room's file at once, which the reload then finds unchanged. The editor edits rooms the game has spawned; a generated room, and one whose file another room shares, is read only. F10 again closes it.

# Tools

`sounds.py` makes every sound in `assets/sounds/`: the menu's `ui_move`, `ui_select` and `ui_back`, the sample's `shoot`, `hit`, `pickup` and `hurt`, game over's `game_over` sting, and the four music loops, `music_title` (four bars at 112 BPM), `music_run` (four bars at 140 BPM), `music_pause` (a two-bar breakbeat at 105 BPM) and `music_over` (a two-bar dirge at 72 BPM). It uses Python's standard library alone and writes 16-bit mono WAV at 22,050 Hz.

```sh
python3 -B tools/sounds.py            # write assets/sounds/
python3 -B tools/sounds.py --out DIR  # write DIR instead
python3 -B tools/sounds.py --check    # compare assets/sounds/ with a fresh render
```

`--check` writes nothing: it renders every sound in memory, compares each byte with the file, and checks that each loop is exactly its bars long and that its seam, from the last sample back to the first, jumps no further than the loop does inside.

`sprites.py` draws every sprite in `assets/sprites/` but M36's `player.png`: the `bolt`, the target `dummy`, the `gem`, the `heal_pickup` and `speed_pickup`, the red enemy's four frames, `enemy` and `enemy_1` to `enemy_3`, and `spark`, the bolt's impact particle, which keeps its own 8-px size. Each is a list of discs, rounded boxes and polygons painted in order and sampled 4 × 4 times per pixel on a 32 × 32 canvas, its art centred as `player.png`'s is, written as RGBA PNG, with the same `--out` and `--check`; its check compares decoded pixels, since deflate's bytes differ between zlib builds.

`icon.py` makes the game's Windows icon in `windows/`: the gem's diamond on the theme's background, drawn with `sprites.py`'s sampler at 16, 32, 48 and 256 px and PNG-encoded, as `icon.ico` and as `icon.res`, one icon group named 1 over four icons named 1 to 4, language neutral. `build.rs` links `icon.res` into the executables of a Windows MSVC build, whose `link.exe` takes it as an input; other targets link nothing. A game changes its icon by changing the drawing in `icon.py` and running it again; it imports the gem's shapes from `sprites.py`, so a game that drops the gem there draws its icon in `icon.py`. Its `--out` and `--check` work as `sprites.py`'s do: the check compares decoded pixels and holds every compressed size, offset and byte count to the image it describes.

```sh
python3 -B tools/icon.py            # write windows/
python3 -B tools/icon.py --check    # compare windows/ with a fresh render
```

Provenance: every file in `assets/sounds/` is synthesized by `sounds.py` from noise, filters, oscillators and envelopes, and each music pattern is written in it note by note; none is sampled, transcribed or taken from elsewhere, the engine repository's own `assets/sounds/` included. Every sprite but `player.png` is drawn by `sprites.py` from the shapes written in it, and the icon by `icon.py` from the gem's. They are original to the template this game started from and ship under its license. Change a sound, a sprite or the icon by changing its script and running it again, so the files and the scripts never disagree.

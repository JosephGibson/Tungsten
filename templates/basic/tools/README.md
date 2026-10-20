# Tools

`sounds.py` makes every sound in `assets/sounds/`: the menu's `ui_move`, `ui_select` and `ui_back`, the sample's `shoot`, `hit` and `pickup`, and the two music loops, `music_run` (four bars at 140 BPM) and `music_pause` (a two-bar breakbeat at 105 BPM). It uses Python's standard library alone and writes 16-bit mono WAV at 22,050 Hz.

```sh
python3 -B tools/sounds.py            # write assets/sounds/
python3 -B tools/sounds.py --out DIR  # write DIR instead
python3 -B tools/sounds.py --check    # compare assets/sounds/ with a fresh render
```

`--check` writes nothing: it renders every sound in memory, compares each byte with the file, and checks that each loop is exactly its bars long and that its seam, from the last sample back to the first, jumps no further than the loop does inside.

Provenance: every file in `assets/sounds/` is synthesized by this script from noise, filters, oscillators and envelopes, and each music pattern is written in it note by note; none is sampled, transcribed or taken from elsewhere, the engine repository's own `assets/sounds/` included. They are original to the template this game started from and ship under its license. Change a sound by changing the script and running it again, so the files and the script never disagree.

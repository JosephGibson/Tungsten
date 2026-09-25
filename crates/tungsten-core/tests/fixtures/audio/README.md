# Audio decode fixtures

Synthetic test tones for `tests/audio_decode.rs`, one per Symphonia format the
workspace enables (`wav`, `ogg`+`vorbis`, `mp3`, `aac`). They're generated here
from a math expression, contain no third-party material, and fall under the
project's MIT license. They're test data, not engine assets, so they don't
belong in any `manifest.json`.

Every file is a 0.25 s, 440 Hz sine at amplitude 0.5. Stereo files carry the
tone on the left channel and silence on the right, so tests can check
interleaving.

| File | Codec / container | Rate | Channels |
| --- | --- | --- | --- |
| `tone_44100_stereo.wav` | PCM s16le / WAV | 44100 | 2 |
| `tone_22050_mono.ogg` | Vorbis q2 / Ogg | 22050 | 1 |
| `tone_44100_stereo.mp3` | MP3 64 kb/s (LAME) | 44100 | 2 |
| `tone_48000_stereo.aac` | AAC-LC 64 kb/s / ADTS | 48000 | 2 |

Generated 2026-09-25 with FFmpeg n9.0.1:

```bash
BE='-map_metadata -1 -fflags +bitexact -flags:a +bitexact'
ffmpeg -f lavfi -i "aevalsrc=0.5*sin(2*PI*440*t)|0:s=44100:d=0.25" $BE -c:a pcm_s16le tone_44100_stereo.wav
ffmpeg -f lavfi -i "aevalsrc=0.5*sin(2*PI*440*t):s=22050:d=0.25" $BE -c:a libvorbis -q:a 2 tone_22050_mono.ogg
ffmpeg -f lavfi -i "aevalsrc=0.5*sin(2*PI*440*t)|0:s=44100:d=0.25" $BE -c:a libmp3lame -b:a 64k tone_44100_stereo.mp3
ffmpeg -f lavfi -i "aevalsrc=0.5*sin(2*PI*440*t)|0:s=48000:d=0.25" $BE -c:a aac -b:a 64k -f adts tone_48000_stereo.aac
```

Lossy encoders add priming and padding frames, so tests allow a frame-count
range rather than an exact length. Corrupt and truncated inputs are derived
from these files at test time; none are stored.

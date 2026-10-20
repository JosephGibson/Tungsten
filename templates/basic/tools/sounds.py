"""The game's sounds, synthesized and written as 16-bit mono PCM WAV.

Everything is built from integer xorshift noise, one-pole filters, pulse,
triangle and sine oscillators and exponential envelopes, so regeneration is
byte-identical. Each sound is normalized to a -2 dBFS peak. The four music
loops render one bar past their end and fold that tail onto their start, so
each repeats at its bar line without a click.

    python3 -B tools/sounds.py            # write assets/sounds/
    python3 -B tools/sounds.py --out DIR  # write DIR instead
    python3 -B tools/sounds.py --check    # compare assets/sounds/ byte for byte
"""
import argparse
import io
import math
import pathlib
import sys
import wave

RATE = 22050
PEAK = 10 ** (-2 / 20)
SOUNDS = pathlib.Path(__file__).resolve().parent.parent / 'assets' / 'sounds'


class Noise:
    """Deterministic white noise in [-1, 1) from a 32-bit xorshift."""

    def __init__(self, seed):
        self.state = seed & 0xFFFFFFFF or 1

    def __call__(self):
        x = self.state
        x ^= (x << 13) & 0xFFFFFFFF
        x ^= x >> 17
        x ^= (x << 5) & 0xFFFFFFFF
        self.state = x
        return x / 2147483648.0 - 1.0


class LowPass:
    """One-pole low-pass; `cutoff` may change per sample."""

    def __init__(self):
        self.y = 0.0

    def __call__(self, x, cutoff):
        self.y += (1 - math.exp(-math.tau * cutoff / RATE)) * (x - self.y)
        return self.y


def glide(start, end, progress):
    """Exponential interpolation between two frequencies."""
    return start * (end / start) ** progress


def pulse(phase, duty):
    """A pulse wave at `phase` cycles: +1 for `duty` of each cycle, else -1."""
    return 1.0 if phase % 1.0 < duty else -1.0


def triangle(phase):
    """A triangle wave at `phase` cycles."""
    return 4.0 * abs(phase % 1.0 - 0.5) - 1.0


def note(name):
    """A note's frequency in Hz: `A4` is 440, `C#5` and `Bb3` take accidentals."""
    letters = {'C': -9, 'D': -7, 'E': -5, 'F': -4, 'G': -2, 'A': 0, 'B': 2}
    step = letters[name[0]] + {'#': 1, 'b': -1}.get(name[1], 0)
    octave = int(name[-1])
    return 440.0 * 2 ** ((step + 12 * (octave - 4)) / 12)


def encode(samples, peak=PEAK):
    """Normalize `samples` to `peak` and encode them as a WAV file."""
    scale = peak / max(abs(s) for s in samples)
    frames = b''.join(
        int(round(s * scale * 32767)).to_bytes(2, 'little', signed=True) for s in samples)
    out = io.BytesIO()
    with wave.open(out, 'wb') as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(RATE)
        wav.writeframes(frames)
    return out.getvalue()


def finish(samples, fade_in=0.003, fade_out=0.015):
    """Fade a one-shot's ends, then encode it."""
    n = len(samples)
    for i in range(min(n, int(fade_in * RATE))):
        samples[i] *= i / (fade_in * RATE)
    for i in range(min(n, int(fade_out * RATE))):
        samples[n - 1 - i] *= i / (fade_out * RATE)
    return encode(samples)


def blip(duration, start, end, duty, decay, seed):
    """A short pulse chirp from `start` to `end` Hz, with a breath of noise."""
    noise, low = Noise(seed), LowPass()
    samples, phase = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        phase += glide(start, end, t / duration) / RATE
        tone = pulse(phase, duty) * 0.6 + math.sin(math.tau * phase) * 0.4
        breath = low(noise(), 4000) * math.exp(-t / 0.006)
        samples.append((tone + 0.5 * breath) * math.exp(-t / decay))
    return samples


def ui_move():
    """A soft tick as focus moves."""
    return finish(blip(0.05, 1320, 1180, 0.25, 0.018, 0x0A11), fade_in=0.001)


def ui_select():
    """Two quick rising notes: a choice taken."""
    first = blip(0.045, note('E6'), note('E6'), 0.25, 0.03, 0x5E1E)
    second = blip(0.075, note('B6'), note('B6'), 0.25, 0.04, 0x5E1F)
    return finish(first + second, fade_in=0.001)


def ui_back():
    """Two quick falling notes: a step back."""
    first = blip(0.045, note('B5'), note('B5'), 0.5, 0.03, 0xBAC0)
    second = blip(0.07, note('E5'), note('E5'), 0.5, 0.035, 0xBAC1)
    return finish(first + second, fade_in=0.001)


def shoot():
    """A zap: a pulse diving from 1.6 kHz to 220 Hz over a hiss."""
    duration = 0.16
    noise, low, high = Noise(0x2A99), LowPass(), LowPass()
    samples, phase = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        phase += glide(1600, 220, p ** 0.6) / RATE
        n = noise()
        hiss = (low(n, 6000) - high(n, 1500)) * math.exp(-t / 0.03)
        samples.append((pulse(phase, 0.3) * 0.7 + 0.6 * hiss) * math.exp(-t / 0.07))
    return finish(samples, fade_in=0.001)


def hit():
    """A knock: a short pitch-dropping thump under a crack of noise."""
    duration = 0.18
    noise, low, high = Noise(0x417C), LowPass(), LowPass()
    samples, phase = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        n = noise()
        crack = (low(n, 5200) - high(n, 1200)) * math.exp(-t / 0.018)
        phase += glide(240, 70, min(1.0, t / 0.09)) / RATE
        thump = math.sin(math.tau * phase) * min(1.0, t / 0.002) * math.exp(-t / 0.05)
        samples.append(math.tanh(1.4 * (1.3 * crack + thump)))
    return finish(samples, fade_in=0.0005)


def pickup():
    """A rising arpeggio, C6, E6, G6, C7, each a bright pulse."""
    samples = []
    for name in ('C6', 'E6', 'G6', 'C7'):
        samples += blip(0.055, note(name), note(name), 0.125, 0.05, 0x9E70)
    return finish(samples, fade_in=0.001, fade_out=0.03)


def hurt():
    """The player hurt: a dull crunch over a pulse grunt dropping from 330 to
    90 Hz, lower and longer than `hit`."""
    duration = 0.26
    noise, low, high = Noise(0x4E27), LowPass(), LowPass()
    samples, phase = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        n = noise()
        crunch = (low(n, 2200) - high(n, 300)) * math.exp(-t / 0.035)
        phase += glide(330, 90, min(1.0, t / 0.16)) / RATE
        grunt = (pulse(phase, 0.35) + math.sin(math.tau * phase)) * 0.5 * math.exp(-t / 0.09)
        samples.append(math.tanh(1.8 * (1.1 * crunch + grunt)))
    return finish(samples, fade_in=0.0005, fade_out=0.03)


def mix(length, events):
    """`events`, each `(start sample, voice)`, summed into `length` samples."""
    samples = [0.0] * length
    for start, voice in events:
        for i, value in enumerate(voice[:max(0, length - start)]):
            samples[start + i] += value
    return samples


def game_over():
    """The game-over sting: a pulse line falling A4, F4, D4, G#3, then a low
    boom under an A minor chord in triangles that fades over a second."""
    step = int(0.14 * RATE)
    events = [(at * step, tone(note(name), 0.13, lambda p: pulse(p, 0.25), 0.25, 0.35))
              for at, name in enumerate(('A4', 'F4', 'D4', 'G#3'))]
    events.append((4 * step, kick(0xDEAD, 1.0)))
    events += [(4 * step, tone(note(name), 1.1, triangle, 0.7, 0.25, release=0.3))
               for name in ('A2', 'E3', 'A3', 'C4')]
    return finish(mix(4 * step + int(1.5 * RATE), events), fade_out=0.2)


def render_loop(beats, beat, events):
    """A loop `beats` long at `beat` samples each, four to the bar: each event
    `(start sample, voice)` renders into a buffer a bar longer, whose tail
    is then folded onto the start. Returns the loop's samples."""
    length, bar = beats * beat, 4 * beat
    buffer = [0.0] * (length + bar)
    for start, voice in events:
        for i, value in enumerate(voice):
            if start + i >= len(buffer):
                break
            buffer[start + i] += value
    loop = buffer[:length]
    for i in range(bar):
        loop[i] += buffer[length + i]
    return loop


def tone(freq, seconds, wave_fn, decay, level, release=0.02):
    """`seconds` of a held note through `wave_fn(phase)`, decaying, with a
    short release so it never clicks off."""
    count = int(seconds * RATE)
    tail = int(release * RATE)
    out = []
    for i in range(count + tail):
        t = i / RATE
        gate = 1.0 if i < count else 1.0 - (i - count) / tail
        attack = min(1.0, i / (0.004 * RATE))
        out.append(wave_fn(freq * t) * level * attack * gate * math.exp(-t / decay))
    return out


def kick(seed, level=1.0):
    """A drum: a sine dropping from 140 to 45 Hz with a click of noise."""
    noise, low = Noise(seed), LowPass()
    out, phase = [], 0.0
    for i in range(int(0.3 * RATE)):
        t = i / RATE
        phase += glide(140, 45, min(1.0, t / 0.12)) / RATE
        click = low(noise(), 3000) * math.exp(-t / 0.004)
        out.append(level * (math.sin(math.tau * phase) * math.exp(-t / 0.11) + 0.4 * click))
    return out


def snare(seed, level=1.0, decay=0.11):
    """A drum: band-passed noise over a 190 Hz body."""
    noise, low, high = Noise(seed), LowPass(), LowPass()
    out = []
    for i in range(int(0.25 * RATE)):
        t = i / RATE
        n = noise()
        rattle = (low(n, 7000) - high(n, 1100)) * math.exp(-t / decay)
        body = math.sin(math.tau * 190 * t) * math.exp(-t / 0.05)
        out.append(level * (rattle + 0.5 * body))
    return out


def hat(seed, level, decay, length=0.25):
    """A cymbal: high-passed noise, closed with a short `decay`, open with a
    long one."""
    noise, low = Noise(seed), LowPass()
    out = []
    for i in range(int(length * RATE)):
        t = i / RATE
        n = noise()
        out.append(level * (n - low(n, 7500)) * math.exp(-t / decay))
    return out


def music_run():
    """An original four-bar chiptune at 140 BPM in A minor: a pulse lead, a
    pulse arpeggio, a triangle bass and noise drums over Am, F, C and G."""
    beat = RATE * 60 // 140  # 9,450 samples
    eighth = beat // 2
    chords = [('A', 'C', 'E'), ('F', 'A', 'C'), ('C', 'E', 'G'), ('G', 'B', 'D')]
    # The lead in eighths; '-' holds the note before, '.' rests.
    lead = ('E5 - A5 G5 E5 - D5 E5 '
            'C5 - F5 E5 C5 - A4 C5 '
            'E5 - G5 E5 C5 - D5 E5 '
            'D5 - B4 G4 B4 - D5 .').split()
    events = []
    at = 0
    while at < len(lead):
        name, held = lead[at], 1
        while at + held < len(lead) and lead[at + held] == '-':
            held += 1
        if name != '.':
            voice = tone(note(name), held * eighth / RATE - 0.02,
                         lambda p: pulse(p, 0.25), 0.6, 0.32)
            events.append((at * eighth, voice))
        at += held
    for bar, (root, third, fifth) in enumerate(chords):
        bar_start = bar * 4 * beat
        # The bass: root, root, octave, root, fifth, root, octave, fifth.
        pattern = [root + '2', root + '2', root + '3', root + '2',
                   fifth + '2', root + '2', root + '3', fifth + '2']
        for step, name in enumerate(pattern):
            voice = tone(note(name), eighth / RATE - 0.015, triangle, 0.5, 0.55)
            events.append((bar_start + step * eighth, voice))
        # The arpeggio in sixteenths, an octave over the bass's top.
        arp = [root + '4', third + '4', fifth + '4', third + '4'] * 4
        for step, name in enumerate(arp):
            voice = tone(note(name), eighth / 2 / RATE - 0.01,
                         lambda p: pulse(p, 0.5), 0.08, 0.11)
            events.append((bar_start + step * eighth // 2, voice))
        # The drums: kicks on one and three, snares on two and four, hats on
        # every off-beat eighth.
        for quarter in range(4):
            start = bar_start + quarter * beat
            if quarter in (0, 2):
                events.append((start, kick(0x7E01 + quarter, 0.8)))
            else:
                events.append((start, snare(0x5A01 + quarter, 0.45, 0.07)))
            events.append((start + eighth, hat(0x4A71 + quarter, 0.22, 0.02, 0.06)))
    return render_loop(16, beat, events)


def music_pause():
    """An original two-bar syncopated breakbeat at 105 BPM: kick, snare with
    ghost notes, closed and open hats, its pattern written here."""
    beat = RATE * 60 // 105  # 12,600 samples
    sixteenth = beat // 4
    # Sixteenths across two bars: K kick, S snare, g ghost snare,
    # h closed hat, o open hat.
    lanes = {
        'K': 'K......K..K.....' 'K..K......K.....',
        'S': '....S.......S...' '....S.......S...',
        'g': '.........g.....g' '.g.....g......g.',
        'h': 'h.h.h.h.h.h.h...' 'h.h.h.h.h.h.h...',
        'o': '..............o.' '..............o.',
    }
    events = []
    for lane, steps in lanes.items():
        for step, mark in enumerate(steps):
            if mark == '.':
                continue
            start = step * sixteenth
            seed = 0xB000 + ord(lane) * 64 + step
            voice = {
                'K': lambda: kick(seed, 1.0),
                'S': lambda: snare(seed, 0.7),
                'g': lambda: snare(seed, 0.16, 0.04),
                'h': lambda: hat(seed, 0.2, 0.025, 0.08),
                'o': lambda: hat(seed, 0.22, 0.16, 2 * sixteenth / RATE),
            }[lane]()
            events.append((start, voice))
    return render_loop(8, beat, events)


def music_title():
    """An original four-bar title theme at 112 BPM in C major: a pulse lead
    over C, G, Am and F, a triangle bass, a soft arpeggio and light drums."""
    beat = RATE * 60 // 112  # 11,812 samples
    eighth = beat // 2
    chords = [('C', 'E', 'G'), ('G', 'B', 'D'), ('A', 'C', 'E'), ('F', 'A', 'C')]
    # The lead in eighths; '-' holds the note before, '.' rests.
    lead = ('E5 - G5 C6 B5 - G5 - '
            'D5 - G5 B5 A5 - G5 - '
            'C5 - E5 A5 G5 - E5 - '
            'F5 - A5 C6 B5 - G5 .').split()
    events = []
    at = 0
    while at < len(lead):
        name, held = lead[at], 1
        while at + held < len(lead) and lead[at + held] == '-':
            held += 1
        if name != '.':
            voice = tone(note(name), held * eighth / RATE - 0.02,
                         lambda p: pulse(p, 0.25), 0.7, 0.3)
            events.append((at * eighth, voice))
        at += held
    for bar, (root, third, fifth) in enumerate(chords):
        bar_start = bar * 4 * beat
        # The bass: root, root, fifth, octave in quarters.
        for step, name in enumerate([root + '2', root + '2', fifth + '2', root + '3']):
            voice = tone(note(name), beat / RATE - 0.02, triangle, 0.6, 0.5)
            events.append((bar_start + step * beat, voice))
        # The arpeggio in eighths, soft, an octave over the bass's top.
        for step, name in enumerate([root + '4', third + '4', fifth + '4', third + '4'] * 2):
            voice = tone(note(name), eighth / RATE - 0.01, lambda p: pulse(p, 0.5), 0.1, 0.08)
            events.append((bar_start + step * eighth, voice))
        # The drums: a kick on one and three, a soft snare on two and four,
        # hats on the off-beats.
        for quarter in range(4):
            start = bar_start + quarter * beat
            if quarter in (0, 2):
                events.append((start, kick(0x7100 + bar * 4 + quarter, 0.6)))
            else:
                events.append((start, snare(0x5100 + bar * 4 + quarter, 0.25, 0.06)))
            events.append((start + eighth, hat(0x4100 + bar * 4 + quarter, 0.15, 0.02, 0.06)))
    return render_loop(16, beat, events)


def music_over():
    """An original two-bar dirge at 72 BPM in D minor: held triangle chords
    on Dm, Bb, Gm and A over a sine bass, a slow pulse line, and a heartbeat
    on each bar's first beat."""
    beat = RATE * 60 // 72  # 18,375 samples
    chords = [('D3', 'F3', 'A3'), ('Bb2', 'D3', 'F3'), ('G2', 'Bb2', 'D3'), ('A2', 'C#3', 'E3')]
    events = []
    for at, names in enumerate(chords):
        held = 2 * beat / RATE - 0.05
        for name in names:
            events.append((at * 2 * beat, tone(note(name), held, triangle, 1.6, 0.2, release=0.08)))
        bass = tone(note(names[0]) / 2, held, lambda p: math.sin(math.tau * p), 1.2, 0.4, 0.08)
        events.append((at * 2 * beat, bass))
    for at, name in enumerate(('A4', 'F4', 'D4', 'E4', 'F4', 'D4', 'C#4', 'E4')):
        voice = tone(note(name), beat / RATE - 0.06, lambda p: pulse(p, 0.125), 0.5, 0.16)
        events.append((at * beat, voice))
    for bar in range(2):
        events.append((bar * 4 * beat, kick(0xBEA0 + bar, 0.7)))
        events.append((bar * 4 * beat + beat // 3, kick(0xBEB0 + bar, 0.45)))
    return render_loop(8, beat, events)


LOOPS = {
    'music_title.wav': 16 * (RATE * 60 // 112),
    'music_run.wav': 16 * (RATE * 60 // 140),
    'music_pause.wav': 8 * (RATE * 60 // 105),
    'music_over.wav': 8 * (RATE * 60 // 72),
}


def build_sounds():
    """Return `{file name: WAV bytes}` for every sound."""
    return {
        'ui_move.wav': ui_move(),
        'ui_select.wav': ui_select(),
        'ui_back.wav': ui_back(),
        'shoot.wav': shoot(),
        'hit.wav': hit(),
        'pickup.wav': pickup(),
        'hurt.wav': hurt(),
        'game_over.wav': game_over(),
        'music_title.wav': encode(music_title()),
        'music_run.wav': encode(music_run()),
        'music_pause.wav': encode(music_pause()),
        'music_over.wav': encode(music_over()),
    }


def samples_of(data):
    """A WAV file's 16-bit samples."""
    with wave.open(io.BytesIO(data), 'rb') as wav:
        frames = wav.readframes(wav.getnframes())
    return [int.from_bytes(frames[i:i + 2], 'little', signed=True)
            for i in range(0, len(frames), 2)]


def seam_problem(name, data):
    """Why loop `name` would not repeat cleanly, or None: its length must be
    its bars exactly, and its jump from the last sample back to the first no
    larger than the largest jump inside it."""
    samples = samples_of(data)
    if len(samples) != LOOPS[name]:
        return f'{len(samples)} samples, not {LOOPS[name]}'
    inner = max(abs(b - a) for a, b in zip(samples, samples[1:]))
    seam = abs(samples[0] - samples[-1])
    if seam > inner:
        return f'a seam jump of {seam} past the largest inner jump, {inner}'
    return None


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--check', action='store_true',
                        help='compare assets/sounds/ with a fresh render; write nothing')
    parser.add_argument('--out', type=pathlib.Path, default=SOUNDS,
                        help='the folder to write, assets/sounds/ by default')
    args = parser.parse_args()
    sounds = build_sounds()
    failed = False
    for name, data in sounds.items():
        if name in LOOPS and (problem := seam_problem(name, data)):
            print(f'{name}: {problem}', file=sys.stderr)
            failed = True
        if args.check:
            path = SOUNDS / name
            if not path.is_file() or path.read_bytes() != data:
                print(f'{name}: differs from a fresh render', file=sys.stderr)
                failed = True
            else:
                print(f'{name}: matches')
    if args.check:
        if failed:
            return 1
        print(f'{len(sounds)} sounds match')
        return 0
    if failed:
        return 1
    args.out.mkdir(parents=True, exist_ok=True)
    for name, data in sounds.items():
        (args.out / name).write_bytes(data)
    print(f'wrote {len(sounds)} sounds to {args.out}')
    return 0


if __name__ == '__main__':
    sys.exit(main())

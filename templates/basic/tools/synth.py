"""The primitives the game's sound scripts build from: deterministic noise, a
one-pole filter, glides, note names, a mixer, melodies, a loop renderer and
its seam check, and the WAV encoder.

Each parameter a script may change (a rate, the fades, a bar's beats, a
loop's tail) defaults to the template's: 22,050 Hz, 16-bit mono at a -2 dBFS
peak, four beats to the bar. `sounds.py` imports it from beside itself, so
no bytecode lands in `tools/`:

    sys.dont_write_bytecode = True
    sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
    import synth
"""
import io
import math
import wave

RATE = 22050
PEAK = 10 ** (-2 / 20)


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
    """One-pole low-pass at `rate`; `cutoff` may change per sample."""

    def __init__(self, rate=RATE):
        self.y = 0.0
        self.rate = rate

    def __call__(self, x, cutoff):
        self.y += (1 - math.exp(-math.tau * cutoff / self.rate)) * (x - self.y)
        return self.y


def glide(start, end, progress):
    """Exponential interpolation between two frequencies."""
    return start * (end / start) ** progress


def note(name):
    """A note's frequency in Hz: `A4` is 440, `C#5` and `Bb3` take accidentals."""
    letters = {'C': -9, 'D': -7, 'E': -5, 'F': -4, 'G': -2, 'A': 0, 'B': 2}
    step = letters[name[0]] + {'#': 1, 'b': -1}.get(name[1], 0)
    octave = int(name[-1])
    return 440.0 * 2 ** ((step + 12 * (octave - 4)) / 12)


def encode(samples, rate=RATE, peak=PEAK):
    """Normalize `samples` to `peak` and encode them as a WAV file at `rate`."""
    scale = peak / max(abs(s) for s in samples)
    frames = b''.join(
        int(round(s * scale * 32767)).to_bytes(2, 'little', signed=True) for s in samples)
    out = io.BytesIO()
    with wave.open(out, 'wb') as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(rate)
        wav.writeframes(frames)
    return out.getvalue()


def finish(samples, fade_in=0.003, fade_out=0.015, rate=RATE):
    """Fade a one-shot's ends, then encode it at `rate`."""
    n = len(samples)
    for i in range(min(n, int(fade_in * rate))):
        samples[i] *= i / (fade_in * rate)
    for i in range(min(n, int(fade_out * rate))):
        samples[n - 1 - i] *= i / (fade_out * rate)
    return encode(samples, rate)


def place(buffer, start, voice):
    """Adds `voice` into `buffer` from sample `start`, clipped at its end."""
    for i, value in enumerate(voice[:max(0, len(buffer) - start)]):
        buffer[start + i] += value


def mix(length, events):
    """`events`, each `(start sample, voice)`, summed into `length` samples."""
    samples = [0.0] * length
    for start, voice in events:
        place(samples, start, voice)
    return samples


def melody(line, step, voice):
    """Events for `line`, its notes in steps of `step` samples: '-' holds the
    note before, '.' rests; `voice(freq, steps)` renders one note held that
    many steps."""
    events, at, names = [], 0, line.split()
    while at < len(names):
        name, held = names[at], 1
        while at + held < len(names) and names[at + held] == '-':
            held += 1
        if name != '.':
            events.append((at * step, voice(note(name), held)))
        at += held
    return events


def render_loop(bars, beat, events, beats_per_bar=4, tail=1):
    """A loop `bars` long at `beat` samples a beat, `beats_per_bar` to the
    bar: each event `(start sample, voice)` renders into a buffer `tail`
    bars longer, whose tail is then folded onto the start, so the loop
    repeats at its bar line without a click. Returns the loop's samples."""
    bar = beats_per_bar * beat
    length = bars * bar
    buffer = [0.0] * (length + tail * bar)
    for start, voice in events:
        place(buffer, start, voice)
    loop = buffer[:length]
    for i in range(tail * bar):
        loop[i % length] += buffer[length + i]
    return loop


def samples_of(data):
    """A WAV file's 16-bit samples."""
    with wave.open(io.BytesIO(data), 'rb') as wav:
        frames = wav.readframes(wav.getnframes())
    return [int.from_bytes(frames[i:i + 2], 'little', signed=True)
            for i in range(0, len(frames), 2)]


def seam_problem(name, data, loops):
    """Why loop `name` would not repeat cleanly, or None: its length must be
    `loops[name]` samples, its bars exactly, and its jump from the last
    sample back to the first no larger than the largest jump inside it."""
    samples = samples_of(data)
    if len(samples) != loops[name]:
        return f'{len(samples)} samples, not {loops[name]}'
    inner = max(abs(b - a) for a, b in zip(samples, samples[1:]))
    seam = abs(samples[0] - samples[-1])
    if seam > inner:
        return f'a seam jump of {seam} past the largest inner jump, {inner}'
    return None

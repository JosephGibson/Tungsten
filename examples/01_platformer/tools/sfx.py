"""Synthesized sound effects, written as 16-bit mono PCM WAV.

Everything is built from integer xorshift noise, one-pole filters, sine
oscillators and exponential envelopes, so regeneration is byte-identical.
Each sound is normalized to a -2 dBFS peak; one-shots fade and loops crossfade.
"""
import io
import math
import wave

RATE = 44100
PEAK = 10 ** (-2 / 20)
# Registered in sounds.json but not synthesized here; generate.py keeps them.
HAND_AUTHORED_SOUNDS = ('sounds/black_hole.ogg',)


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


def finish(samples, fade_in=0.005, fade_out=0.02):
    """Fade the ends, normalize to PEAK and encode as a WAV file."""
    n = len(samples)
    for i in range(min(n, int(fade_in * RATE))):
        samples[i] *= i / (fade_in * RATE)
    for i in range(min(n, int(fade_out * RATE))):
        samples[n - 1 - i] *= i / (fade_out * RATE)
    scale = PEAK / max(abs(s) for s in samples)
    frames = b''.join(
        int(round(s * scale * 32767)).to_bytes(2, 'little', signed=True) for s in samples)
    out = io.BytesIO()
    with wave.open(out, 'wb') as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(RATE)
        wav.writeframes(frames)
    return out.getvalue()


def fireball_cast():
    """A fiery whoosh: band-passed noise sweeping up then down over a low swell."""
    duration = 0.38
    noise, low, high = Noise(0xF1AE), LowPass(), LowPass()
    samples, phase = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        # The band opens quickly, then closes as the missile leaves.
        cutoff = 700 + 2600 * math.sin(math.pi * min(1.0, p * 1.4)) ** 1.5
        n = noise()
        band = low(n, cutoff) - high(n, cutoff * 0.22)
        envelope = min(1.0, t / 0.035) * math.exp(-3.2 * p)
        phase += math.tau * glide(85, 150, p) / RATE
        swell = 0.45 * math.sin(phase) * math.sin(math.pi * p)
        samples.append(1.6 * band * envelope + swell)
    return finish(samples)


def fireball_blast():
    """A boom: a falling noise burst, a pitch-dropping thump and sparse crackle."""
    duration = 0.95
    noise, crackle_noise, low, rumble = Noise(0xB1A5), Noise(0xC7AC), LowPass(), LowPass()
    samples, phase, crack = [], 0.0, 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        burst = low(n, glide(5200, 220, min(1.0, p * 1.6))) * math.exp(-t / 0.16)
        tail = rumble(n, 180) * 2.2 * math.exp(-t / 0.42)
        phase += math.tau * glide(72, 34, min(1.0, t / 0.5)) / RATE
        thump = 0.95 * math.sin(phase) * min(1.0, t / 0.004) * math.exp(-t / 0.22)
        # Decaying clicks fire less often as the blast fades.
        if crackle_noise() > 1 - 0.0022 * math.exp(-2.5 * p):
            crack = 0.35 + 0.25 * abs(crackle_noise())
        crack *= 0.985
        pop = crack * crackle_noise()
        samples.append(math.tanh(1.5 * (1.4 * burst + tail + thump + pop)))
    return finish(samples, fade_in=0.001)


def extinguish():
    """A sizzle: high-passed noise with a flutter and wet pops, then a soft hiss."""
    duration = 0.75
    noise, pops, low, body = Noise(0x5122), Noise(0x9092), LowPass(), LowPass()
    samples, pop = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        sizzle = n - low(n, 2400)
        hiss = body(n, 900) * 0.8
        flutter = 0.7 + 0.3 * math.sin(math.tau * (21 + 12 * p) * t)
        envelope = min(1.0, t / 0.012) * math.exp(-t / 0.24)
        if pops() > 1 - 0.0016 * (1 - p):
            pop = 0.5
        pop *= 0.97
        samples.append((sizzle * flutter + hiss) * envelope + pop * pops())
    return finish(samples)


def player_hit():
    """A hurt: a dull body thump under a short gritty crack and a falling yelp tone."""
    duration = 0.32
    noise, low, high = Noise(0x417E), LowPass(), LowPass()
    samples, thump_phase, tone_phase = [], 0.0, 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        crack = (low(n, 3800) - high(n, 900)) * math.exp(-t / 0.028)
        thump_phase += math.tau * glide(150, 55, min(1.0, t / 0.12)) / RATE
        thump = 1.1 * math.sin(thump_phase) * min(1.0, t / 0.003) * math.exp(-t / 0.075)
        tone_phase += math.tau * glide(520, 260, p) / RATE
        tone = 0.28 * math.sin(tone_phase) * min(1.0, t / 0.01) * math.exp(-t / 0.09)
        samples.append(math.tanh(1.3 * (1.5 * crack + thump + tone)))
    return finish(samples, fade_in=0.001)


def iron_crush():
    """A heavy crunch with brittle glass cracks and a bright, ringing iron clink."""
    duration = 0.65
    noise, cracks, low = Noise(0x170C), Noise(0x61A5), LowPass()
    samples, phase, crack = [], 0.0, 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        n = noise()
        crunch = (n - low(n, 700)) * math.exp(-t / .06)
        phase += math.tau * glide(120, 42, min(1.0, t / .14)) / RATE
        thud = math.sin(phase) * min(1.0, t / .002) * math.exp(-t / .09)
        if cracks() > 1 - .006 * math.exp(-t / .08):
            crack = .65
        crack *= .94
        glass = crack * cracks()
        # Inharmonic resonances ring after the first crunch, like riveted iron.
        ring_t = max(0.0, t - .012)
        ring = sum(math.sin(math.tau * f * ring_t) * weight * math.exp(-ring_t / decay)
                   for f, weight, decay in [(1380, .32, .19), (2213, .18, .13),
                                           (3671, .1, .075)])
        ring *= min(1.0, ring_t / .002)
        samples.append(math.tanh(1.5 * (1.1 * crunch + .85 * thud + glass)) + ring)
    return finish(samples, fade_in=.001)


def ice_beam():
    """A cold rushing hiss with a descending crystalline shimmer."""
    duration = 0.48
    noise, low, high = Noise(0x1CEB), LowPass(), LowPass()
    samples, phase = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        hiss = low(n, 7200) - high(n, 1800)
        envelope = min(1.0, t / .012) * math.exp(-t / .16)
        phase += math.tau * glide(2600, 1400, p) / RATE
        shimmer = sum(math.sin(phase * ratio) * weight
                      for ratio, weight in [(1, .3), (1.49, .18), (2.17, .1)])
        samples.append((hiss * 1.4 + shimmer) * envelope)
    return finish(samples, fade_in=.002)


def ice_spray():
    """Seamless cold wind with fluttering hiss and faint crystalline resonances."""
    duration, overlap = 2.0, int(.1 * RATE)
    noise, low, high = Noise(0x1CE5), LowPass(), LowPass()
    count = int(duration * RATE)
    samples = []
    for i in range(count + overlap):
        t = i / RATE
        n = noise()
        wind = low(n, 6800) - high(n, 750)
        flutter = .78 + .12 * math.sin(math.tau * 17 * t) + .1 * math.sin(math.tau * 29 * t)
        shimmer = sum(math.sin(math.tau * frequency * t) * weight
                      for frequency, weight in [(1451, .05), (2177, .03), (3221, .018)])
        samples.append(1.3 * wind * flutter + shimmer * (.6 + .4 * math.sin(math.tau * 3 * t)))
    # Crossfade the extra tail into the beginning, then wrap at adjacent samples.
    loop = samples[overlap:count]
    for i in range(overlap):
        blend = i / (overlap - 1)
        loop.append(samples[count + i] * (1 - blend) + samples[i] * blend)
    return finish(loop, fade_in=0, fade_out=0)


def ice_freeze():
    """A coating locks into ice with a short crackle and bright glassy chimes."""
    noise, cracks, low = Noise(0xF20E), Noise(0xC1CE), LowPass()
    samples, crack = [], 0.0
    for i in range(int(.65 * RATE)):
        t = i / RATE
        n = noise()
        frost = (n - low(n, 2200)) * math.exp(-t / .045)
        if cracks() > 1 - .005 * math.exp(-t / .1):
            crack = .7
        crack *= .95
        chimes = sum(math.sin(math.tau * frequency * t) * weight * math.exp(-t / decay)
                     for frequency, weight, decay in [(1860, .5, .22), (2797, .3, .17),
                                                     (4223, .15, .11)])
        samples.append(frost + crack * cracks() + chimes * min(1.0, t / .003))
    return finish(samples, fade_in=.001)


def ice_end():
    """A soft pressure release masks the abrupt stop of the sustained cold jet."""
    noise, low, high = Noise(0x1CE0), LowPass(), LowPass()
    samples = []
    for i in range(int(.18 * RATE)):
        t = i / RATE
        n = noise()
        hiss = low(n, glide(6200, 1800, min(1.0, t / .18))) - high(n, 700)
        samples.append(hiss * math.exp(-t / .04))
    return finish(samples, fade_in=.003, fade_out=.025)


def ice_shatter():
    """A sharp fracture followed by falling glass fragments and a short icy ring."""
    noise, chips, low = Noise(0x5A77), Noise(0xC419), LowPass()
    samples, chip = [], 0.0
    for i in range(int(.62 * RATE)):
        t = i / RATE
        n = noise()
        snap = (n - low(n, 900)) * math.exp(-t / .018)
        if chips() > 1 - .008 * math.exp(-t / .12):
            chip = .7
        chip *= .965
        ring = sum(math.sin(math.tau * frequency * t) * weight * math.exp(-t / decay)
                   for frequency, weight, decay in [(813, .24, .12), (1537, .18, .19),
                                                   (2761, .12, .1)])
        samples.append(math.tanh(1.6 * snap + chip * chips()) + ring * min(1.0, t / .002))
    return finish(samples, fade_in=.001)


def flame_start():
    """The fire spray catching: a soft low whoomph that opens into a hiss."""
    duration = 0.42
    noise, low, high = Noise(0xF1A3), LowPass(), LowPass()
    samples, phase = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        roar = low(n, glide(380, 3200, min(1.0, p * 1.6))) - high(n, 160)
        phase += math.tau * glide(95, 55, p) / RATE
        thump = math.sin(phase) * math.exp(-t / .09)
        envelope = min(1.0, t / .03) * math.exp(-t / .2)
        samples.append((roar * 1.3 + thump * .9) * envelope)
    return finish(samples, fade_in=.004)


def flame_spray():
    """Seamless roaring fire: a breathy low rush with crackles through it."""
    duration, overlap = 2.0, int(.1 * RATE)
    noise, crackles, low, high = Noise(0xF1A5), Noise(0xC7AC), LowPass(), LowPass()
    count = int(duration * RATE)
    samples, crack = [], 0.0
    for i in range(count + overlap):
        t = i / RATE
        n = noise()
        rush = low(n, 2400) - high(n, 140)
        breath = .8 + .12 * math.sin(math.tau * 5 * t) + .08 * math.sin(math.tau * 11 * t)
        if crackles() > .9994:
            crack = .6
        crack *= .93
        samples.append(1.4 * rush * breath + crack * crackles())
    loop = samples[overlap:count]
    for i in range(overlap):
        blend = i / (overlap - 1)
        loop.append(samples[count + i] * (1 - blend) + samples[i] * blend)
    return finish(loop, fade_in=0, fade_out=0)


def toss():
    """A heavy ball leaving the hand: a short rising swish."""
    duration = 0.3
    noise, low, high = Noise(0x7055), LowPass(), LowPass()
    samples = []
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        swish = low(n, glide(900, 4200, p)) - high(n, glide(300, 1600, p))
        samples.append(swish * math.sin(math.pi * min(1.0, p * 1.15)) ** 2)
    return finish(samples, fade_in=.003, fade_out=.03)


def set_down():
    """Something set down hard: a dull thud with a short metallic ring."""
    noise, low = Noise(0x91AC), LowPass()
    samples, phase = [], 0.0
    for i in range(int(.36 * RATE)):
        t = i / RATE
        n = noise()
        thud = low(n, 420) * math.exp(-t / .03)
        phase += math.tau * glide(140, 70, min(1.0, t / .12)) / RATE
        body = math.sin(phase) * math.exp(-t / .07)
        ring = sum(math.sin(math.tau * frequency * t) * weight * math.exp(-t / decay)
                   for frequency, weight, decay in [(612, .16, .1), (1307, .08, .07)])
        samples.append(thud * 2.2 + body + ring)
    return finish(samples, fade_in=.001)


def force_push():
    """A wave of force: a deep whump and a rush of air sweeping outward."""
    duration = 0.55
    noise, low, high = Noise(0xF0CE), LowPass(), LowPass()
    samples, phase = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        rush = (low(n, glide(2600, 500, p)) - high(n, 120)) * math.sin(math.pi * min(1.0, p * 1.4)) ** 2
        phase += math.tau * glide(120, 38, min(1.0, p * 2.0)) / RATE
        whump = math.sin(phase) * math.exp(-t / .11) * min(1.0, t / .006)
        samples.append(whump * 1.3 + rush * 1.1)
    return finish(samples, fade_in=.002, fade_out=.06)


# The shell's menu sounds and game over's sting, by the IDs it plays them
# under: 01's own, in its twilight palette of bells and plucked strings.


def chime(freq, seconds, level, decay=0.35, rate=RATE, partials=None):
    """A glassy bell: decaying sine partials over `freq`, the upper ones
    shorter, as a lantern's glass rings."""
    partials = partials or [(1.0, 1.0, 1.0), (2.0, 0.42, 0.55), (3.01, 0.2, 0.35),
                            (4.23, 0.1, 0.22), (5.4, 0.05, 0.15)]
    out = []
    for i in range(int(seconds * rate)):
        t = i / rate
        attack = min(1.0, t / 0.002)
        out.append(level * attack * sum(
            weight * math.sin(math.tau * freq * ratio * t) * math.exp(-t / (decay * scale))
            for ratio, weight, scale in partials))
    return out


def pluck(freq, seconds, level, seed, rate=RATE, damping=0.996, bright=0.5):
    """A plucked string (Karplus-Strong): a burst of noise in a delay line one
    period long, averaged as it circulates, so it rings and mellows."""
    noise = Noise(seed)
    period = max(2, int(round(rate / freq)))
    line = [noise() * (0.5 + 0.5 * bright) for _ in range(period)]
    out, at = [], 0
    for _ in range(int(seconds * rate)):
        here, after = line[at], line[(at + 1) % period]
        line[at] = damping * (bright * here + (1 - bright) * 0.5 * (here + after))
        out.append(level * here)
        at = (at + 1) % period
    return out


def place(buffer, start, voice):
    """Adds `voice` into `buffer` from `start`, clipped at its end."""
    for i, value in enumerate(voice[:max(0, len(buffer) - start)]):
        buffer[start + i] += value


def ui_move():
    """A soft wooden tick as focus moves: a short high pluck."""
    samples = pluck(1568.0, 0.06, 1.0, 0x0A11, damping=0.97, bright=0.3)
    return finish(samples, fade_in=0.0005, fade_out=0.01)


def ui_select():
    """A choice taken: two bell notes rising a fifth, A5 to E6."""
    samples = [0.0] * int(0.32 * RATE)
    place(samples, 0, chime(note('A5'), 0.3, 0.8, decay=0.12))
    place(samples, int(0.06 * RATE), chime(note('E6'), 0.26, 0.9, decay=0.14))
    return finish(samples, fade_in=0.0005, fade_out=0.03)


def ui_back():
    """A step back: two bell notes falling a fourth, E6 to B5, duller."""
    samples = [0.0] * int(0.3 * RATE)
    place(samples, 0, chime(note('E6'), 0.28, 0.8, decay=0.09))
    place(samples, int(0.06 * RATE), chime(note('B5'), 0.24, 0.85, decay=0.1))
    return finish(samples, fade_in=0.0005, fade_out=0.03)


def game_over():
    """The game-over sting: a bell line falling D5, C5, A4, F4, then a low
    toll under a D minor chord of plucked strings that rings out."""
    samples = [0.0] * int(3.2 * RATE)
    for at, name in enumerate(('D5', 'C5', 'A4', 'F4')):
        place(samples, int(at * 0.17 * RATE), chime(note(name), 0.6, 0.7, decay=0.25))
    start = int(4 * 0.17 * RATE)
    place(samples, start, chime(note('D2'), 2.5, 1.2, decay=1.1,
                                partials=[(1.0, 1.0, 1.0), (2.76, 0.5, 0.5), (5.4, 0.25, 0.3)]))
    for offset, name in enumerate(('D3', 'A3', 'D4', 'F4')):
        place(samples, start + offset * int(0.035 * RATE),
              pluck(note(name), 2.5, 0.45, 0xD0A0 + offset, damping=0.9985, bright=0.4))
    return finish(samples, fade_out=0.4)


# The music: four loops at 22,050 Hz, each rendered a bar past its end with
# that tail folded onto its start, so it repeats at its bar line without a
# click. `--check` holds each one's length and seam (`seam_problem`).
MUSIC_RATE = 22050


def note(name):
    """A note's frequency in Hz: `A4` is 440, `C#5` and `Bb3` take accidentals."""
    letters = {'C': -9, 'D': -7, 'E': -5, 'F': -4, 'G': -2, 'A': 0, 'B': 2}
    step = letters[name[0]] + {'#': 1, 'b': -1}.get(name[1], 0)
    return 440.0 * 2 ** ((step + 12 * (int(name[-1]) - 4)) / 12)


def pad(freq, seconds, level, attack=0.6, release=0.8, rate=MUSIC_RATE):
    """A soft held pad: two detuned triangles and a sine an octave down,
    breathing in over `attack` and out over `release`."""
    count, tail = int(seconds * rate), int(release * rate)
    out = []
    for i in range(count + tail):
        t = i / rate
        gate = min(1.0, t / attack) if i < count else max(0.0, 1.0 - (i - count) / tail)
        a = 4.0 * abs((freq * 1.003 * t) % 1.0 - 0.5) - 1.0
        b = 4.0 * abs((freq * 0.997 * t + 0.25) % 1.0 - 0.5) - 1.0
        sub = math.sin(math.tau * freq * 0.5 * t)
        out.append(level * gate * (0.4 * a + 0.4 * b + 0.3 * sub))
    return out


def bass(freq, seconds, level, rate=MUSIC_RATE):
    """A round bass note: a sine with a touch of its octave, decaying."""
    out = []
    count, tail = int(seconds * rate), int(0.05 * rate)
    for i in range(count + tail):
        t = i / rate
        gate = 1.0 if i < count else 1.0 - (i - count) / tail
        tone = math.sin(math.tau * freq * t) + 0.25 * math.sin(math.tau * 2 * freq * t)
        out.append(level * gate * min(1.0, t / 0.006) * tone * math.exp(-t / 1.4))
    return out


def soft_kick(seed, level, rate=MUSIC_RATE):
    """A felt drum: a sine falling from 110 to 48 Hz with a breath of noise."""
    noise, low = Noise(seed), LowPass()
    out, phase = [], 0.0
    for i in range(int(0.35 * rate)):
        t = i / rate
        phase += glide(110, 48, min(1.0, t / 0.1)) / rate
        breath = low(noise(), 1800) * math.exp(-t / 0.01)
        out.append(level * (math.sin(math.tau * phase) * math.exp(-t / 0.12) + 0.3 * breath))
    return out


def brush(seed, level, decay, rate=MUSIC_RATE):
    """A brushed snare or a shaker: band-passed noise with a soft attack."""
    noise, low, high = Noise(seed), LowPass(), LowPass()
    out = []
    for i in range(int(min(0.4, decay * 6) * rate)):
        t = i / rate
        n = noise()
        band = (low(n, 6000) - high(n, 1400))
        out.append(level * band * min(1.0, t / 0.004) * math.exp(-t / decay))
    return out


def render_loop(bars, beat, events, beats_per_bar=4):
    """A loop `bars` long at `beat` samples a beat: each event `(start
    sample, voice)` renders into a buffer a bar longer, whose tail is then
    folded onto the start. Returns the loop's samples."""
    bar = beats_per_bar * beat
    length = bars * bar
    buffer = [0.0] * (length + 2 * bar)
    for start, voice in events:
        place(buffer, start, voice)
    loop = buffer[:length]
    for i in range(2 * bar):
        loop[i % length] += buffer[length + i]
    return loop


def melody(line, step, voice):
    """Events for `line`, notes in steps of `step` samples: '-' holds the
    note before, '.' rests; `voice(freq, steps)` renders one note."""
    events, at, names = [], 0, line.split()
    while at < len(names):
        name, held = names[at], 1
        while at + held < len(names) and names[at + held] == '-':
            held += 1
        if name != '.':
            events.append((at * step, voice(note(name), held)))
        at += held
    return events


def music_title():
    """The title: a lantern-lit lullaby at 96 BPM in D minor, twelve bars of
    glass bells over plucked arpeggios, a breathing pad and a round bass,
    with a felt drum from the fifth bar."""
    rate, beat = MUSIC_RATE, MUSIC_RATE * 60 // 96
    bar, eighth = 4 * beat, beat // 2
    chords = [('D3', 'F3', 'A3', 'C4'), ('Bb2', 'D3', 'F3', 'A3'), ('F2', 'A2', 'C3', 'F3'),
              ('C3', 'E3', 'G3', 'C4'), ('D3', 'F3', 'A3', 'E4'), ('G2', 'Bb2', 'D3', 'F3'),
              ('Bb2', 'D3', 'F3', 'A3'), ('A2', 'C#3', 'E3', 'A3'), ('D3', 'F3', 'A3', 'C4'),
              ('Bb2', 'D3', 'F3', 'D4'), ('C3', 'E3', 'G3', 'E4'), ('A2', 'E3', 'A3', 'C#4')]
    events = []
    for index, chord in enumerate(chords):
        start = index * bar
        for name in chord:
            events.append((start, pad(note(name), 4 * beat / rate, 0.09, attack=0.8, release=1.2)))
        events.append((start, bass(note(chord[0]) / 2, 2 * beat / rate, 0.42)))
        events.append((start + 2 * beat, bass(note(chord[0]) / 2, 2 * beat / rate, 0.3)))
        arp = [chord[0], chord[2], chord[3], chord[1], chord[2], chord[3], chord[1], chord[2]]
        for step, name in enumerate(arp):
            up = note(name) * 2
            events.append((start + step * eighth,
                           pluck(up, 1.2, 0.16, 0x7171 + index * 8 + step, rate, 0.9975, 0.45)))
        if index >= 4:
            events.append((start, soft_kick(0x7A00 + index, 0.32)))
            events.append((start + 2 * beat, brush(0x7B00 + index, 0.08, 0.06)))
    line = ('A5 - - F5 E5 - D5 - '
            'D5 - F5 - A5 - G5 - '
            'F5 - - - C5 - D5 - '
            'E5 - - - - - . . '
            'A5 - - F5 E5 - D5 - '
            'Bb5 - A5 - G5 - F5 - '
            'F5 - E5 - D5 - E5 - '
            'C#5 - - - - - . . '
            'D6 - - C6 A5 - F5 - '
            'F5 - G5 - A5 - D5 - '
            'E5 - G5 - C6 - Bb5 - '
            'A5 - - - - - . .')
    events += melody(line, eighth, lambda f, held: chime(f, held * eighth / rate + 0.8, 0.3,
                                                           decay=0.55, rate=rate))
    return render_loop(len(chords), beat, events)


def music_explore():
    """Exploring: an original sixteen-bar journey at 128 BPM in A minor, a
    plucked sixteenth ostinato over a bouncing bass, brushed drums and a bell
    lead that answers itself, through Am, F, C, G and Dm, Am, F, E."""
    rate, beat = MUSIC_RATE, MUSIC_RATE * 60 // 128
    bar, eighth, sixteenth = 4 * beat, beat // 2, beat // 4
    progression = [('A2', 'C3', 'E3'), ('F2', 'A2', 'C3'), ('C3', 'E3', 'G3'), ('G2', 'B2', 'D3'),
                   ('D3', 'F3', 'A3'), ('A2', 'C3', 'E3'), ('F2', 'A2', 'C3'), ('E2', 'G#2', 'B2')]
    chords = progression * 2
    events = []
    for index, (root, third, fifth) in enumerate(chords):
        start = index * bar
        for name in (root, third, fifth):
            events.append((start, pad(note(name) * 2, 4 * beat / rate, 0.05, attack=0.3, release=0.5)))
        for step, name in enumerate([root, root, root, fifth, root, root, third, fifth]):
            freq = note(name) / (1 if step in (3, 7) else 2) * (2 if step == 2 else 1)
            events.append((start + step * eighth, bass(freq, eighth / rate * 0.9, 0.38)))
        figure = [root, fifth, third, fifth] * 4
        for step, name in enumerate(figure):
            freq = note(name) * (4 if step % 4 == 2 else 2)
            events.append((start + step * sixteenth,
                           pluck(freq, 0.5, 0.11, 0xE100 + index * 16 + step, rate, 0.994, 0.6)))
        for quarter in range(4):
            at = start + quarter * beat
            if quarter in (0, 2):
                events.append((at, soft_kick(0xE200 + index * 4 + quarter, 0.5)))
            else:
                events.append((at, brush(0xE300 + index * 4 + quarter, 0.2, 0.08)))
            events.append((at + eighth, brush(0xE400 + index * 4 + quarter, 0.06, 0.02)))
        if index % 4 == 3:
            events.append((start + 3 * beat + eighth, soft_kick(0xE500 + index, 0.3)))
    line = ('. . . . . . . . . . . . . . . . '
            '. . . . . . . . . . . . . . . . '
            '. . . . . . . . . . . . . . . . '
            '. . . . . . . . . . . . . . . . '
            'E5 - - - A5 - C6 - B5 - A5 - G5 - E5 - '
            'F5 - - - A5 - C6 - D6 - C6 - A5 - - - '
            'G5 - - - E5 - G5 - C6 - B5 - G5 - E5 - '
            'D5 - - - G5 - B5 - D6 - - - . - - - '
            'F5 - - - A5 - D6 - C6 - A5 - F5 - D5 - '
            'E5 - - - A5 - C6 - E6 - - - C6 - - - '
            'A5 - - - F5 - A5 - C6 - A5 - F5 - C5 - '
            'B4 - - - E5 - G#5 - B5 - - - . - - - '
            'C6 - - - E6 - D6 - C6 - B5 - A5 - - - '
            'A5 - - - C6 - F6 - E6 - - - C6 - - - '
            'B5 - - - G5 - E5 - G5 - B5 - D6 - - - '
            'B5 - - - - - G#5 - E5 - - - . - - - ')
    events += melody(line, sixteenth, lambda f, held: chime(f, held * sixteenth / rate + 0.5,
                                                              0.2, decay=0.4, rate=rate))
    return render_loop(len(chords), beat, events)


def music_pause():
    """Pause: five bars of still air at 80 BPM, a pad drifting through Fmaj7,
    Em7, Dm9, C and Gsus over a low drone, a bell now and then, no drums."""
    rate, beat = MUSIC_RATE, MUSIC_RATE * 60 // 80
    bar = 4 * beat
    chords = [('F3', 'A3', 'C4', 'E4'), ('E3', 'G3', 'B3', 'D4'), ('D3', 'F3', 'A3', 'E4'),
              ('C3', 'E3', 'G3', 'D4'), ('G2', 'C3', 'D3', 'A3')]
    events = []
    for index, chord in enumerate(chords):
        start = index * bar
        for name in chord:
            events.append((start, pad(note(name), 4 * beat / rate, 0.08, attack=1.2, release=1.6)))
        events.append((start, pad(note('D2'), 4 * beat / rate, 0.06, attack=1.0, release=1.6)))
        bell = note(chord[2]) * 2 if index % 2 == 0 else note(chord[3]) * 2
        events.append((start + beat, chime(bell, 2.6, 0.18, decay=0.9, rate=rate)))
        events.append((start + 3 * beat, chime(note(chord[1]) * 4, 1.8, 0.08, decay=0.6, rate=rate)))
    return render_loop(len(chords), beat, events)


def music_over():
    """Game over: a four-bar dirge at 66 BPM in D minor, held pads on Dm, Bb,
    Gm6 and A under a tolling low bell and a slow falling line, with a
    heartbeat on each bar's first beat."""
    rate, beat = MUSIC_RATE, MUSIC_RATE * 60 // 66
    bar = 4 * beat
    chords = [('D3', 'F3', 'A3'), ('Bb2', 'D3', 'F3'), ('G2', 'Bb2', 'E3'), ('A2', 'C#3', 'E3')]
    events = []
    for index, chord in enumerate(chords):
        start = index * bar
        for name in chord:
            events.append((start, pad(note(name), 4 * beat / rate, 0.1, attack=1.0, release=1.4)))
        toll = [(1.0, 1.0, 1.0), (2.76, 0.45, 0.5), (5.4, 0.2, 0.3)]
        events.append((start, chime(note(chord[0]) / 2, 3.5, 0.32, decay=1.4, rate=rate,
                                    partials=toll)))
        events.append((start, soft_kick(0x0B00 + index, 0.35)))
        events.append((start + beat // 3, soft_kick(0x0B10 + index, 0.22)))
    line = ('A4 - F4 - E4 - D4 - '
            'D4 - C4 - Bb3 - - - '
            'Bb3 - A3 - G3 - - - '
            'E3 - - - C#4 - - - ')
    events += melody(line, beat // 2, lambda f, held: chime(f, held * beat / 2 / rate + 1.0,
                                                              0.16, decay=0.7, rate=rate))
    return render_loop(len(chords), beat, events)


def encode(samples, rate):
    """Normalize `samples` to PEAK and encode them as a WAV file at `rate`."""
    scale = PEAK / max(abs(s) for s in samples)
    frames = b''.join(
        int(round(s * scale * 32767)).to_bytes(2, 'little', signed=True) for s in samples)
    out = io.BytesIO()
    with wave.open(out, 'wb') as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(rate)
        wav.writeframes(frames)
    return out.getvalue()


# Each loop's length in samples: its bars exactly.
LOOPS = {
    'music_title.wav': 12 * 4 * (MUSIC_RATE * 60 // 96),
    'music_explore.wav': 16 * 4 * (MUSIC_RATE * 60 // 128),
    'music_pause.wav': 5 * 4 * (MUSIC_RATE * 60 // 80),
    'music_over.wav': 4 * 4 * (MUSIC_RATE * 60 // 66),
}


def seam_problem(name, data):
    """Why loop `name` would not repeat cleanly, or None: its length must be
    its bars exactly, and its jump from the last sample back to the first no
    larger than the largest jump inside it."""
    with wave.open(io.BytesIO(data), 'rb') as wav:
        frames = wav.readframes(wav.getnframes())
    samples = [int.from_bytes(frames[i:i + 2], 'little', signed=True)
               for i in range(0, len(frames), 2)]
    if len(samples) != LOOPS[name]:
        return f'{len(samples)} samples, not {LOOPS[name]}'
    inner = max(abs(b - a) for a, b in zip(samples, samples[1:]))
    seam = abs(samples[0] - samples[-1])
    if seam > inner:
        return f'a seam jump of {seam} past the largest inner jump, {inner}'
    return None


def build_sounds():
    """Return `{file name: WAV bytes}` for every synthesized sound."""
    return {
        'ui_move.wav': ui_move(),
        'ui_select.wav': ui_select(),
        'ui_back.wav': ui_back(),
        'game_over.wav': game_over(),
        'music_title.wav': encode(music_title(), MUSIC_RATE),
        'music_explore.wav': encode(music_explore(), MUSIC_RATE),
        'music_pause.wav': encode(music_pause(), MUSIC_RATE),
        'music_over.wav': encode(music_over(), MUSIC_RATE),
        'fireball_cast.wav': fireball_cast(),
        'fireball_blast.wav': fireball_blast(),
        'extinguish.wav': extinguish(),
        'player_hit.wav': player_hit(),
        'iron_crush.wav': iron_crush(),
        'ice_beam.wav': ice_beam(),
        'ice_spray.wav': ice_spray(),
        'ice_freeze.wav': ice_freeze(),
        'ice_end.wav': ice_end(),
        'ice_shatter.wav': ice_shatter(),
        'flame_start.wav': flame_start(),
        'flame_spray.wav': flame_spray(),
        'toss.wav': toss(),
        'place.wav': set_down(),
        'force_push.wav': force_push(),
    }

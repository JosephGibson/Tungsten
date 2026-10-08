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


def build_sounds():
    """Return `{file name: WAV bytes}` for every synthesized sound."""
    return {
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
    }

"""Synthesized sound effects, written as 16-bit mono PCM WAV.

Everything is built from integer xorshift noise, one-pole filters, sine
oscillators and exponential envelopes, so regeneration is byte-identical.
Each sound is normalized to a -2 dBFS peak with short fades against clicks.
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


def build_sounds():
    """Return `{file name: WAV bytes}` for every synthesized sound."""
    return {
        'fireball_cast.wav': fireball_cast(),
        'fireball_blast.wav': fireball_blast(),
        'extinguish.wav': extinguish(),
    }

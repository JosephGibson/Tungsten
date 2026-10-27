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
# None since 01 (j) made the black hole's opening.
HAND_AUTHORED_SOUNDS = ()


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


def thud(freq, seconds, level, seed):
    """A body meeting the ground: a short low sine drop under a muffled
    knock of noise."""
    noise, low = Noise(seed), LowPass()
    out, phase = [], 0.0
    for i in range(int(seconds * RATE)):
        t = i / RATE
        phase += math.tau * glide(freq, freq * 0.55, min(1.0, t / seconds)) / RATE
        knock = low(noise(), 700) * math.exp(-t / 0.012)
        out.append(level * (math.sin(phase) * math.exp(-t / (0.3 * seconds)) + 1.2 * knock))
    return out


def player_death():
    """The courier falling: a breathy groan sinking through an open vowel,
    a knee and then the body meeting the ground, the lantern's clink as it
    is set down, and its flame guttering out."""
    duration = 1.1
    samples = [0.0] * int(duration * RATE)
    breath, low = Noise(0xDEA7), LowPass()
    phase = 0.0
    for i in range(int(0.62 * RATE)):
        t = i / RATE
        p = t / 0.62
        f = glide(205, 92, p ** 0.8)
        phase += math.tau * f / RATE
        voice = sum(math.sin(k * phase) / k * (1.0 if k < 4 else 0.45) for k in range(1, 7))
        # An open "oh": the voice's upper partials leaned on near 550 Hz.
        vowel = low(voice + 0.5 * breath(), 900 - 300 * p)
        envelope = min(1.0, t / 0.05) * (1 - p) ** 1.4
        samples[i] += 0.8 * vowel * envelope
    place(samples, int(0.26 * RATE), thud(110, 0.16, 0.9, 0xDEA8))
    place(samples, int(0.5 * RATE), thud(80, 0.24, 1.0, 0xDEA9))
    place(samples, int(0.56 * RATE), chime(1760.0, 0.18, 0.18, decay=0.03))
    place(samples, int(0.585 * RATE), chime(2349.0, 0.14, 0.1, decay=0.025))
    gutter, high = Noise(0xDEAA), LowPass()
    start = int(0.66 * RATE)
    for i in range(int(0.42 * RATE)):
        t = i / RATE
        n = gutter()
        flutter = .5 + .5 * math.sin(math.tau * glide(18, 6, t / 0.42) * t)
        hiss = high(n, glide(1400, 400, t / 0.42))
        samples[start + i] += 0.35 * hiss * flutter * min(1.0, t / 0.02) * math.exp(-t / 0.16)
    return finish(samples, fade_in=0.002, fade_out=0.06)


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
    """The force push's wave (01 (j)): a soft push-off and a whump of bent
    air, then a dense rush of air flying away from the wizard, falling in
    pitch as it goes, a low warped tone bowing under it and a scuff of grit
    where it scrapes the floor, about half a second."""
    duration = 0.55
    noise, grit_noise = Noise(0xF0CF), Noise(0xF0D0)
    band_low, band_high, sub, grit_low = LowPass(), LowPass(), LowPass(), LowPass()
    samples, phase = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        # The rush: a band of noise sweeping down as the front flies off.
        swell = (1 - math.exp(-t / 0.018)) * math.exp(-max(0.0, t - 0.06) / 0.16)
        rush = (band_low(n, glide(4200, 520, p ** 0.7)) - band_high(n, glide(900, 120, p))) * swell
        # The warp: a low tone bending down and back, gone by 0.3 s.
        bend = 1 - 0.4 * math.sin(math.pi * min(1.0, p * 2.2))
        phase += math.tau * 74 * bend / RATE
        warp = math.sin(phase) * min(1.0, t / 0.008) * math.exp(-t / 0.11)
        # The whump of the push-off, in at once.
        whump = sub(n, 80) * 4.0 * math.exp(-t / 0.05)
        # Grit: sparse low crackle while the front scrapes along.
        g = grit_noise()
        grain = grit_low(g if abs(g) > 0.93 else 0.0, 1400) * 6.0
        scrape = grain * math.sin(math.pi * min(1.0, max(0.0, (t - 0.05) / 0.3))) ** 2
        samples.append(math.tanh(1.5 * rush + 0.9 * warp + whump + scrape))
    place(samples, 0, thud(110, 0.07, 0.35, 0xF0D1))
    return finish(samples, fade_in=0.002, fade_out=0.09)


# 01 (b)'s weapons and elements: the gun, the bolt from the sky, the
# reactions' bursts and the training dummy.


def gun_shot():
    """A heavy report (01 (f)): a cracking burst over a deep punch and a
    booming body, the slide's tick after."""
    noise, low, high, boom = Noise(0x6005), LowPass(), LowPass(), LowPass()
    samples, phase = [], 0.0
    for i in range(int(0.42 * RATE)):
        t = i / RATE
        n = noise()
        crack = (low(n, 7500) - high(n, 1000)) * math.exp(-t / 0.014)
        body = boom(n, 700) * 1.8 * math.exp(-t / 0.08)
        phase += math.tau * glide(130, 42, min(1.0, t / 0.09)) / RATE
        punch = 1.3 * math.sin(phase) * min(1.0, t / 0.001) * math.exp(-t / 0.06)
        tick = math.sin(math.tau * 2900 * t) * 0.16 * math.exp(-max(0.0, t - 0.05) / 0.02) * (t > 0.05)
        samples.append(math.tanh(2.4 * crack + body + punch) + tick)
    return finish(samples, fade_in=0.0005, fade_out=0.05)


def gun_reload():
    """The magazine back in: a slide pulled and let go, two clicks a beat
    apart with a short iron ring."""
    noise = Noise(0x2E10)
    samples = []
    for i in range(int(0.42 * RATE)):
        t = i / RATE
        out = 0.0
        for at, pitch in ((0.0, 2400), (0.22, 1700)):
            u = t - at
            if u >= 0:
                click = noise() * math.exp(-u / 0.004)
                ring = sum(math.sin(math.tau * pitch * ratio * u) * weight
                           for ratio, weight in ((1, 0.3), (1.53, 0.15))) * math.exp(-u / 0.05)
                out += click + ring
        slide = noise() * 0.12 * math.sin(math.pi * min(1.0, t / 0.2)) * (t < 0.2)
        samples.append(out + slide)
    return finish(samples, fade_in=0.0005)


def ricochet():
    """A bullet off iron: a bright ping that whines down and away."""
    noise, high = Noise(0x41C0), LowPass()
    samples, phase = [], 0.0
    for i in range(int(0.45 * RATE)):
        t = i / RATE
        p = t / 0.45
        n = noise()
        hit = (n - high(n, 2500)) * math.exp(-t / 0.006)
        phase += math.tau * glide(3400, 1500, p) / RATE
        whine = 0.6 * math.sin(phase) * min(1.0, t / 0.004) * math.exp(-t / 0.14)
        samples.append(hit + whine)
    return finish(samples, fade_in=0.0005)


def thunder():
    """A strike overhead (e01sfx, V27): a sharp splitting crack into a long,
    deep roll that wobbles as it fades, and a distant rumble swelling again
    about 1.5 s in, over 3.2 s."""
    duration = 3.2
    noise, rumble, far = Noise(0x7D0E), Noise(0x0B11), Noise(0x0B12)
    crack_low, mid_low, mid_high, roll_a, roll_b, far_a, far_b = (LowPass() for _ in range(7))
    samples = []
    for i in range(int(duration * RATE)):
        t = i / RATE
        n = noise()
        crack = (n - crack_low(n, 1400)) * math.exp(-t / 0.022)
        split = (mid_low(n, 3200) - mid_high(n, 500)) * 0.6 * math.exp(-t / 0.09)
        swell = min(1.0, t / 0.16) * math.exp(-t / 0.95)
        wobble = 0.68 + 0.32 * math.sin(math.tau * 2.6 * t) * math.sin(math.tau * 0.9 * t + 1)
        roll = roll_b(roll_a(rumble(), 190), 85) * 9.0 * swell * wobble
        u = t - 1.15
        distant = 0.0
        if u > 0:
            rise = min(1.0, u / 0.5) ** 2 * math.exp(-max(0.0, u - 0.5) / 0.6)
            distant = far_b(far_a(far(), 140), 62) * 11.0 * rise * (0.75 + 0.25 * math.sin(math.tau * 1.7 * t))
        samples.append(math.tanh(1.8 * crack + split + roll + distant))
    return finish(samples, fade_in=0.0005, fade_out=0.35)


def crackle():
    """A conduction jump: a short electric buzz of sharp sparks."""
    noise, sparks, high = Noise(0xC2AC), Noise(0x5BA2), LowPass()
    samples, spark = [], 0.0
    for i in range(int(0.22 * RATE)):
        t = i / RATE
        n = noise()
        buzz = (n - high(n, 1800)) * (0.5 + 0.5 * math.copysign(1, math.sin(math.tau * 120 * t)))
        if sparks() > 0.995:
            spark = 0.9
        spark *= 0.9
        samples.append((0.6 * buzz + spark * sparks()) * math.exp(-t / 0.08))
    return finish(samples, fade_in=0.001)


def steam_burst():
    """Cold on fire: a wet whoosh opening into a hiss, with sizzling pops."""
    duration = 0.8
    noise, pops, low, high = Noise(0x57EA), Noise(0x9090), LowPass(), LowPass()
    samples, pop, phase = [], 0.0, 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        hiss = low(n, glide(1200, 7000, min(1.0, p * 2.5))) - high(n, 600)
        phase += math.tau * glide(110, 60, p) / RATE
        whump = 0.8 * math.sin(phase) * math.exp(-t / 0.07)
        if pops() > 1 - 0.002 * (1 - p):
            pop = 0.5
        pop *= 0.96
        envelope = min(1.0, t / 0.02) * math.exp(-t / 0.28)
        samples.append((1.4 * hiss + whump) * envelope + pop * pops())
    return finish(samples, fade_in=0.002)


def lava_sizzle():
    """Lava's bite as it throws the player: a fat sizzle over a low fiery
    whoomph, with spitting pops."""
    duration = 0.55
    noise, pops, low, high = Noise(0x1A7A), Noise(0x5177), LowPass(), LowPass()
    samples, pop, phase = [], 0.0, 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        sizzle = (low(n, glide(5200, 2400, p)) - high(n, 900)) * (0.65 + 0.35 * math.sin(math.tau * 37 * t))
        phase += math.tau * glide(150, 55, min(1.0, p * 2)) / RATE
        whoomph = 0.9 * math.sin(phase) * min(1.0, t / 0.01) * math.exp(-t / 0.12)
        if pops() > 1 - 0.006 * (1 - p):
            pop = 0.7
        pop *= 0.94
        envelope = min(1.0, t / 0.008) * math.exp(-t / 0.22)
        samples.append(1.2 * sizzle * envelope + whoomph + pop * pops())
    return finish(samples, fade_in=0.001)


def lava_hiss():
    """Lava crusting under the ice spray: a long hiss of steam that dies
    away, a deep crackle as the skin stiffens and a last few pops."""
    duration = 1.4
    noise, pops, low, high, rumble = Noise(0x1A55), Noise(0xC7A5), LowPass(), LowPass(), LowPass()
    samples, pop = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        hiss = (low(n, glide(7500, 2200, p)) - high(n, 700)) * min(1.0, t / 0.05) * math.exp(-t / 0.55)
        crackle = (rumble(n, 260) * 2.2) * (0.5 + 0.5 * math.sin(math.tau * 9 * t)) * math.exp(-t / 0.4)
        if pops() > 1 - 0.004 * (1 - p):
            pop = 0.5
        pop *= 0.95
        samples.append(1.3 * hiss + crackle + pop * pops())
    return finish(samples, fade_in=0.003)


def lava_chill():
    """The cold taking lava (01 (k)): a sizzle that builds as the spray bites,
    cold glints ticking over it, the skin's first stiffening creaks under."""
    duration = 0.75
    noise, ticks, low, high, creak = Noise(0xC01D), Noise(0x71C5), LowPass(), LowPass(), LowPass()
    samples, glint, gphase, gfreq = [], 0.0, 0.0, 5000.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        rise = min(1.0, p * 1.6) * (1 - max(0.0, p - 0.85) / 0.15)
        sizzle = (low(n, glide(3800, 6800, p)) - high(n, 1400)) * (0.6 + 0.4 * math.sin(math.tau * 31 * t))
        if ticks() > 1 - 0.0016:
            glint, gfreq = 0.35, 4200 + 2600 * (ticks() * 0.5 + 0.5)
        glint *= 0.9985
        gphase += math.tau * gfreq / RATE
        crk = creak(n, 180) * 3.0 * (0.5 + 0.5 * math.sin(math.tau * 6 * t)) * p
        samples.append(1.1 * sizzle * rise + glint * math.sin(gphase) + crk * rise)
    return finish(samples, fade_in=0.01, fade_out=0.06)


def lava_crack():
    """A crust cracking as it warms (01 (k)): a sharp snap, a low split under
    it and a short hot fizz spitting from the seam."""
    duration = 0.45
    noise, pops, low, high, split = Noise(0xC7AC), Noise(0x5BA7), LowPass(), LowPass(), LowPass()
    samples, phase, pop = [], 0.0, 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        snap = (high(n, 2500) * -1 + n) * math.exp(-t / 0.006)
        phase += math.tau * glide(140, 70, min(1.0, t / 0.12)) / RATE
        thump = 0.9 * math.sin(phase) * math.exp(-t / 0.07) * min(1.0, t / 0.002)
        fizz = (low(n, 6000) - split(n, 1800)) * math.exp(-t / 0.12) * min(1.0, max(0.0, t - 0.02) / 0.02)
        if pops() > 1 - 0.004 * (1 - p):
            pop = 0.45
        pop *= 0.93
        samples.append(1.6 * snap + thump + 0.9 * fizz + pop * pops())
    return finish(samples, fade_in=0.0005)


def lava_melt():
    """The crust going back to melt (01 (k)): a deep glug as the plates
    sink, two fat bubbles breaking, and a wet spatter and sizzle after."""
    duration = 0.8
    noise, spits, low, high, slosh = Noise(0x6106), Noise(0x5917), LowPass(), LowPass(), LowPass()
    samples, spit = [], 0.0
    bubbles = ((0.0, 95, 190, 0.22), (0.16, 120, 260, 0.16), (0.3, 105, 220, 0.14))
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        glug = 0.0
        for start, f0, f1, length in bubbles:
            u = t - start
            if 0 <= u < length:
                glug += math.sin(math.tau * glide(f0, f1, u / length) * u) * math.sin(math.pi * u / length) ** 0.6
        wet = (slosh(n, 650) - high(n, 90)) * 2.2 * math.exp(-t / 0.25) * min(1.0, t / 0.02)
        sizzle = (low(n, 5200) - high(n, 1500)) * 0.5 * min(1.0, max(0.0, t - 0.25) / 0.1) * math.exp(-max(0.0, t - 0.25) / 0.3)
        if spits() > 1 - 0.003 * (1 - p):
            spit = 0.5
        spit *= 0.94
        samples.append(0.9 * glug + wet + sizzle + spit * spits())
    return finish(samples, fade_in=0.002, fade_out=0.08)


def lift_start():
    """The mine's elevator setting off: the brake's iron clank, then the
    winch's ratcheting whir as the cable takes the cage's weight."""
    duration = 0.9
    noise, low, high = Noise(0x11F7), LowPass(), LowPass()
    samples, phase = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        n = noise()
        clank = sum(math.sin(math.tau * f * t) * w for f, w in ((420, 0.5), (1130, 0.3), (2310, 0.15))) * math.exp(-t / 0.05)
        phase += math.tau * glide(18, 34, min(1.0, t / 0.5)) / RATE
        ratchet = (high(n, 2600) - low(n, 400)) * (0.5 + 0.5 * math.copysign(1, math.sin(phase))) * min(1.0, t / 0.08)
        hum = 0.25 * math.sin(math.tau * glide(70, 110, min(1.0, t / 0.6)) * t) * min(1.0, t / 0.1)
        samples.append(0.9 * clank + 0.5 * ratchet * math.exp(-max(0.0, t - 0.5) / 0.2) + hum * math.exp(-max(0.0, t - 0.6) / 0.15))
    return finish(samples, fade_in=0.001)


def lift_stop():
    """The elevator arriving: a heavy wooden thud as the cage seats, with a
    rattle of slack chain."""
    duration = 0.6
    noise, rattles, low = Noise(0x5707), Noise(0xA77E), LowPass()
    samples, rattle, phase = [], 0.0, 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        phase += math.tau * glide(95, 45, min(1.0, t / 0.2)) / RATE
        thud = math.sin(phase) * min(1.0, t / 0.002) * math.exp(-t / 0.09)
        knock = low(noise(), 900) * 2.5 * math.exp(-t / 0.03)
        if rattles() > 1 - 0.02 * math.exp(-t / 0.15):
            rattle = 0.4
        rattle *= 0.9
        samples.append(thud + knock + rattle * rattles())
    return finish(samples, fade_in=0.0005)


def frost_blast():
    """Ice bursting: a hard crunch of fracturing ice, a cold boom and a spray
    of falling shards."""
    noise, chips, low = Noise(0xF2B1), Noise(0xC41A), LowPass()
    samples, chip, phase = [], 0.0, 0.0
    for i in range(int(0.8 * RATE)):
        t = i / RATE
        n = noise()
        crunch = (n - low(n, 700)) * math.exp(-t / 0.03)
        phase += math.tau * glide(90, 40, min(1.0, t / 0.3)) / RATE
        boom = math.sin(phase) * min(1.0, t / 0.003) * math.exp(-t / 0.16)
        if chips() > 1 - 0.01 * math.exp(-t / 0.18):
            chip = 0.6
        chip *= 0.96
        ring = sum(math.sin(math.tau * f * t) * w * math.exp(-t / d)
                   for f, w, d in ((1240, 0.2, 0.14), (2380, 0.12, 0.1), (3910, 0.07, 0.07)))
        samples.append(math.tanh(1.5 * crunch + boom + chip * chips()) + ring)
    return finish(samples, fade_in=0.0005)


def dummy_hit():
    """A blow on the straw dummy: a dull sackcloth thump and a rustle."""
    noise, rustle, low, high = Noise(0xD077), Noise(0x5A0C), LowPass(), LowPass()
    samples, phase = [], 0.0
    for i in range(int(0.3 * RATE)):
        t = i / RATE
        n = noise()
        thump = low(n, 380) * 2.4 * math.exp(-t / 0.035)
        phase += math.tau * glide(110, 70, min(1.0, t / 0.08)) / RATE
        body = 0.7 * math.sin(phase) * math.exp(-t / 0.05)
        r = rustle()
        straw = (r - high(r, 3500)) * 0.35 * math.exp(-t / 0.09)
        samples.append(thump + body + straw)
    return finish(samples, fade_in=0.001)


def summon():
    """A summoning: a rising shimmer of bells over a soft swell, settling as
    the dummy lands."""
    duration = 0.7
    noise, low = Noise(0x5A33), LowPass()
    samples, phase = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        phase += math.tau * glide(440, 880, min(1.0, p * 1.4)) / RATE
        shimmer = sum(math.sin(phase * ratio) * weight
                      for ratio, weight in ((1, 0.3), (1.5, 0.18), (2.01, 0.12)))
        swell = low(noise(), 900) * 0.8
        envelope = math.sin(math.pi * min(1.0, p * 1.2)) ** 1.5
        land = 0.6 * math.sin(math.tau * 70 * t) * math.exp(-max(0.0, t - 0.55) / 0.05) * (t > 0.55)
        samples.append((shimmer + swell) * envelope + land)
    return finish(samples, fade_in=0.004, fade_out=0.05)


# 01 (c)'s bestiary: each enemy's tells, attacks and steps, a hit and an
# alert any enemy plays, and the drops' pickup.


def wisp_hum():
    """The wisp's burning: a seamless low flame roar, a slow flicker and
    soft crackles."""
    duration, overlap = 2.0, int(.12 * RATE)
    noise, crackles, low, high = Noise(0x0715), Noise(0x0C4A), LowPass(), LowPass()
    count = int(duration * RATE)
    samples, crack, phase = [], 0.0, 0.0
    for i in range(count + overlap):
        t = i / RATE
        n = noise()
        roar = low(n, 520) - high(n, 90) * 0.6
        flicker = .72 + .18 * math.sin(math.tau * 5 * t) + .1 * math.sin(math.tau * 11.5 * t + 1)
        phase += math.tau * (96 + 6 * math.sin(math.tau * 1.5 * t)) / RATE
        drone = .22 * math.sin(phase) + .08 * math.sin(2 * phase)
        if crackles() > .9993:
            crack = .45
        crack *= .93
        samples.append(2.4 * roar * flicker + drone + crack * crackles())
    loop = samples[overlap:count]
    for i in range(overlap):
        blend = i / (overlap - 1)
        loop.append(samples[count + i] * (1 - blend) + samples[i] * blend)
    return finish(loop, fade_in=0, fade_out=0)


def wisp_flare():
    """The wisp's alert: a flame flaring up, a rising whoosh with a hot
    crackle."""
    duration = 0.45
    noise, crackles, low, high = Noise(0xF1A2), Noise(0xF1A3), LowPass(), LowPass()
    samples, crack = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        band = low(n, glide(500, 4200, min(1.0, p * 1.6))) - high(n, 300)
        envelope = min(1.0, t / 0.06) * math.exp(-2.6 * p)
        if crackles() > 1 - .004 * (1 - p):
            crack = .6
        crack *= .9
        samples.append(1.8 * band * envelope + crack * crackles())
    return finish(samples)


def crawler_hiss():
    """The crawler's warning: an insect hiss chattering, over a cold tinkle
    of its crystals."""
    duration = 0.5
    noise, low = Noise(0xC4A1), LowPass()
    samples = []
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        hiss = n - low(n, 3200)
        chatter = .45 + .55 * max(0.0, math.sin(math.tau * 34 * t)) ** 3
        tinkle = sum(math.sin(math.tau * f * t) * w * math.exp(-t / d)
                     for f, w, d in ((2637, .12, .16), (3520, .08, .11), (4699, .05, .08)))
        envelope = min(1.0, t / 0.02) * math.sin(math.pi * min(1.0, p * 1.1)) ** 0.6
        samples.append((1.2 * hiss * chatter) * envelope + tinkle)
    return finish(samples)


def crawler_lunge():
    """The lunge: a quick skitter of legs and a snap of mandibles."""
    noise, clicks, low = Noise(0xC4A2), Noise(0xC4A3), LowPass()
    samples, click = [], 0.0
    for i in range(int(0.36 * RATE)):
        t = i / RATE
        n = noise()
        if t < 0.22 and clicks() > .993:
            click = .7
        click *= .82
        skitter = click * (n - low(n, 1800))
        snap = 0.0
        if t > 0.24:
            u = t - 0.24
            snap = (math.sin(math.tau * 1900 * u) + .6 * math.sin(math.tau * 3100 * u)) * math.exp(-u / .012)
        samples.append(1.5 * skitter + snap)
    return finish(samples, fade_in=0.001)


def crawler_scared():
    """The crawler shrinking from the light: a sharp, startled hiss whose
    floor falls as it recoils, its mandibles clicking shut over a frosty
    exhale."""
    duration = 0.38
    noise, clicks, low, mist = Noise(0xC4A4), Noise(0xC4A5), LowPass(), LowPass()
    samples, click = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        hiss = n - low(n, glide(4200, 1800, p))
        exhale = mist(n, 900) * math.exp(-max(0.0, t - .08) / .12) * min(1.0, t / .08)
        if .03 < t < .16 and clicks() > .994:
            click = .9
        click *= .8
        snap = click * math.sin(math.tau * 2600 * t)
        envelope = min(1.0, t / 0.006) * math.exp(-t / .13)
        samples.append((1.3 * hiss + .8 * exhale) * envelope + .5 * snap)
    return finish(samples, fade_in=0.001)


def staff_gutter():
    """The staff light guttering out: a starved flame's breathy sputter,
    fluttering slower as it dies, with a few last pops."""
    duration = 0.42
    noise, pops, low, high = Noise(0x57A1), Noise(0x57A2), LowPass(), LowPass()
    samples, pop = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        flutter = .5 + .5 * math.sin(math.tau * glide(24, 9, p) * t)
        breath = low(n, glide(1600, 500, p))
        if t < 0.25 and pops() > .9975:
            pop = .8
        pop *= .86
        crackle = pop * (n - high(n, 2500))
        envelope = min(1.0, t / 0.01) * math.exp(-t / .16)
        samples.append((1.6 * breath * flutter + crackle) * envelope)
    return finish(samples)


def beetle_charge():
    """The charge: a heavy iron rumble rolling forward, plates clanking."""
    duration = 0.8
    noise, low, rumble = Noise(0xBE73), LowPass(), LowPass()
    samples, phase = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        roll = rumble(low(n, 300), 140) * 5.0 * (.7 + .3 * math.sin(math.tau * 13 * t))
        phase += math.tau * glide(58, 72, p) / RATE
        engine = .5 * math.sin(phase) + .25 * math.sin(2 * phase + .5)
        clank = 0.0
        for start in (0.06, 0.24, 0.42, 0.6):
            if t >= start:
                u = t - start
                clank += sum(math.sin(math.tau * f * u) * w for f, w in ((410, .4), (933, .25), (1617, .15))) \
                    * math.exp(-u / .05)
        envelope = min(1.0, t / 0.05) * (1 - p) ** 0.5
        samples.append(math.tanh(1.2 * (roll + engine) * envelope + .5 * clank))
    return finish(samples)


def bat_screech():
    """The bat's cry: a thin screech sweeping up, warbling."""
    duration = 0.34
    noise, low = Noise(0xBA71), LowPass()
    samples, phase = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        freq = glide(2300, 4100, p ** 0.7) * (1 + .03 * math.sin(math.tau * 55 * t))
        phase += math.tau * freq / RATE
        tone = math.sin(phase + 1.8 * math.sin(phase * 0.5))
        n = noise()
        air = (n - low(n, 5000)) * .25
        envelope = min(1.0, t / 0.015) * math.exp(-3 * p)
        samples.append((tone + air) * envelope)
    return finish(samples)


def bat_dive():
    """The dive: wings folded, a whoosh falling in pitch with a flutter."""
    duration = 0.55
    noise, low, high = Noise(0xBA72), LowPass(), LowPass()
    samples = []
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        whoosh = low(n, glide(4200, 700, p)) - high(n, 250)
        flutter = .7 + .3 * math.sin(math.tau * 24 * t)
        envelope = math.sin(math.pi * min(1.0, p * 1.05)) ** 1.2
        samples.append(1.8 * whoosh * flutter * envelope)
    return finish(samples)


def troll_step():
    """A troll's footfall: a deep thud and a scatter of grit."""
    noise, low = Noise(0x7801), LowPass()
    samples, phase = [], 0.0
    for i in range(int(0.36 * RATE)):
        t = i / RATE
        phase += math.tau * glide(62, 34, min(1.0, t / 0.18)) / RATE
        thud = math.sin(phase) * min(1.0, t / 0.003) * math.exp(-t / 0.09)
        n = noise()
        grit = low(n, 900) * 1.6 * math.exp(-t / 0.05)
        samples.append(math.tanh(1.4 * thud + grit))
    return finish(samples, fade_in=0.001)


def troll_roar():
    """The troll's roar: a deep growl swelling and breaking, with a rasp in
    its throat."""
    duration = 1.3
    noise, throat, low, formant = Noise(0x7802), Noise(0x7803), LowPass(), LowPass()
    samples, phase = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        freq = (74 + 22 * math.sin(math.pi * min(1.0, p * 1.3))) * (1 + .04 * math.sin(math.tau * 6.5 * t))
        phase += freq / RATE
        saw = 2 * (phase % 1.0) - 1
        voice = formant(saw, 620 + 340 * math.sin(math.pi * p))
        rasp = low(throat(), 1400) * (.5 + .5 * math.sin(math.tau * 31 * t))
        envelope = min(1.0, t / 0.12) * (1 - p) ** 0.7
        samples.append(math.tanh(2.2 * (voice + .7 * rasp) * envelope + .15 * noise() * envelope))
    return finish(samples, fade_out=0.08)


def troll_slam():
    """The slam: two fists on the ground, a boom, a crack and stone falling."""
    duration = 1.1
    noise, debris, low, rumble = Noise(0x7804), Noise(0x7805), LowPass(), LowPass()
    samples, phase, rock = [], 0.0, 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        crack = (n - low(n, 1500)) * math.exp(-t / 0.025)
        phase += math.tau * glide(55, 28, min(1.0, t / 0.4)) / RATE
        boom = 1.2 * math.sin(phase) * min(1.0, t / 0.002) * math.exp(-t / 0.3)
        roll = rumble(n, 160) * 3.0 * math.exp(-t / 0.45)
        if t > 0.08 and debris() > 1 - .003 * (1 - p):
            rock = .5
        rock *= .95
        samples.append(math.tanh(1.6 * (crack + boom + roll) + rock * debris()))
    return finish(samples, fade_in=0.0005, fade_out=0.1)


def troll_throw():
    """A throw: a heaving grunt and the whoosh of what it flings."""
    duration = 0.55
    noise, low, high, formant = Noise(0x7806), LowPass(), LowPass(), LowPass()
    samples, phase = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        phase += glide(120, 80, p) / RATE
        saw = 2 * (phase % 1.0) - 1
        grunt = formant(saw, 700) * math.exp(-t / 0.12) * min(1.0, t / 0.02)
        n = noise()
        whoosh = (low(n, glide(900, 3800, p)) - high(n, 400)) * math.sin(math.pi * max(0.0, min(1.0, (t - .1) / .45)))
        samples.append(1.6 * grunt + 1.7 * whoosh)
    return finish(samples)


def enemy_hit():
    """A blow landing on an enemy: a meaty thwack and a short crunch."""
    noise, crunch, low, high = Noise(0xE401), Noise(0xE402), LowPass(), LowPass()
    samples, phase = [], 0.0
    for i in range(int(0.26 * RATE)):
        t = i / RATE
        n = noise()
        thwack = low(n, 1100) * 2.2 * math.exp(-t / 0.03)
        phase += math.tau * glide(170, 90, min(1.0, t / 0.06)) / RATE
        body = .8 * math.sin(phase) * math.exp(-t / 0.045)
        c = crunch()
        bits = (c - high(c, 2500)) * .5 * math.exp(-t / 0.06)
        samples.append(math.tanh(1.3 * (thwack + body) + bits))
    return finish(samples, fade_in=0.0005)


def enemy_spotted():
    """The alert: an enemy has seen the courier, a low thump under two
    quick bells a minor second apart, uneasy."""
    samples = [0.0] * int(0.6 * RATE)
    noise, low = Noise(0xE403), LowPass()
    for i in range(int(0.2 * RATE)):
        t = i / RATE
        samples[i] += (.9 * math.sin(math.tau * glide(90, 60, t / .2) * t) + low(noise(), 400)) * math.exp(-t / 0.06)
    place(samples, int(0.01 * RATE), chime(note('E5'), 0.5, 0.7, decay=0.16))
    place(samples, int(0.09 * RATE), chime(note('F5'), 0.5, 0.8, decay=0.2))
    return finish(samples, fade_in=0.0005, fade_out=0.05)


def gulp(freq, seconds, level):
    """One swallow of a draught: a bubble's tone rising through a soft
    throat formant, quick to swell and quicker to close."""
    out, phase = [], 0.0
    for i in range(int(seconds * RATE)):
        t = i / RATE
        p = t / seconds
        phase += math.tau * glide(freq, freq * 1.7, min(1.0, p * 1.4)) / RATE
        body = math.sin(phase) + 0.35 * math.sin(2 * phase + 0.6)
        out.append(level * body * min(1.0, t / 0.006) * math.exp(-t / (0.32 * seconds)))
    return out


def orb_pickup():
    """A drop taken, as a potion: a cork's pop, three quick gulps of the
    draught, each a little lower, and a bright shimmer as it takes hold."""
    samples = [0.0] * int(0.62 * RATE)
    noise, low, high = Noise(0x0B1A), LowPass(), LowPass()
    phase = 0.0
    for i in range(int(0.035 * RATE)):
        t = i / RATE
        phase += math.tau * glide(1100, 420, t / 0.035) / RATE
        n = noise()
        snap = (low(n, 5200) - high(n, 1400)) * math.exp(-t / 0.004)
        samples[i] += 0.9 * math.sin(phase) * math.exp(-t / 0.009) + 0.6 * snap
    for start, freq in ((0.075, 300), (0.165, 270), (0.255, 240)):
        place(samples, int(start * RATE), gulp(freq, 0.085, 0.8))
    for k, name in enumerate(('E6', 'G#6', 'B6', 'E7')):
        place(samples, int((0.35 + 0.035 * k) * RATE), chime(note(name), 0.25, 0.22, decay=0.07))
    return finish(samples, fade_in=0.0005, fade_out=0.05)


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


# The music: five loops at 22,050 Hz, each rendered a bar past its end with
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


def tom(freq, seconds, level, seed, rate=MUSIC_RATE):
    """A taiko-like drum: a pitched skin falling a little in pitch, and a
    slap of noise."""
    noise, low = Noise(seed), LowPass()
    out, phase = [], 0.0
    for i in range(int(seconds * rate)):
        t = i / rate
        phase += glide(freq * 1.4, freq, min(1.0, t / 0.05)) / rate
        slap = low(noise(), 2400) * math.exp(-t / 0.012)
        out.append(level * (math.sin(math.tau * phase) * math.exp(-t / (seconds * 0.35)) + 0.35 * slap))
    return out


def stab(freqs, seconds, level, rate=MUSIC_RATE):
    """A short string stab: a few detuned saws on the chord, bright then
    closing fast."""
    out = []
    low = LowPass()
    for i in range(int(seconds * rate)):
        t = i / rate
        tone = sum(2 * ((f * k * t) % 1.0) - 1 for f in freqs for k in (0.997, 1.003)) / (2 * len(freqs))
        # LowPass is tuned for RATE; at MUSIC_RATE its cutoff reads halved.
        out.append(level * low(tone, glide(2200, 500, min(1.0, t / seconds))) * math.exp(-t / (seconds * 0.4)))
    return out


def music_combat():
    """The combat layer: sixteen bars at 128 BPM in A minor, in step with
    music_explore's bars and chords, which it plays over while an enemy sees
    the courier. Toms drive every beat under an eighth-note low pulse, string
    stabs push the off-beats and a tense bell figure climbs in the second
    half."""
    rate, beat = MUSIC_RATE, MUSIC_RATE * 60 // 128
    bar, eighth = 4 * beat, beat // 2
    progression = [('A2', 'C3', 'E3'), ('F2', 'A2', 'C3'), ('C3', 'E3', 'G3'), ('G2', 'B2', 'D3'),
                   ('D3', 'F3', 'A3'), ('A2', 'C3', 'E3'), ('F2', 'A2', 'C3'), ('E2', 'G#2', 'B2')]
    chords = progression * 2
    events = []
    for index, (root, third, fifth) in enumerate(chords):
        start = index * bar
        for quarter in range(4):
            at = start + quarter * beat
            events.append((at, tom(55 if quarter in (0, 2) else 82, 0.45, 0.42, 0xC300 + index * 4 + quarter)))
            if quarter == 3:
                events.append((at + eighth, tom(110, 0.25, 0.22, 0xC340 + index)))
        for step in range(8):
            octave = 2 if step % 4 == 3 else 1
            events.append((start + step * eighth, bass(note(root) / 2 * octave, eighth / rate * 0.8, 0.3)))
        for step in (1, 3, 5, 6):
            events.append((start + step * eighth, stab([note(n) * 2 for n in (root, third, fifth)], 0.16, 0.16)))
        if index >= 8:
            figure = (fifth, root, third, root) if index % 2 else (root, third, fifth, third)
            for k, name in enumerate(figure):
                events.append((start + k * beat + eighth, chime(note(name) * 4, 0.35, 0.09, decay=0.12, rate=rate)))
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


# 01 (d)'s ambience: one loop per area, 6 s at 22,050 Hz, crossfaded across
# its seam as `wisp_hum` is; and the splinter of a shot passing wood.

AMBIENCE_RATE = 22050
AMBIENCE_SECONDS = 6
AMBIENCE_LENGTH = AMBIENCE_SECONDS * AMBIENCE_RATE


def ambience_loop(voice, overlap=0.25):
    """`voice(i, t)` sampled over the loop and `overlap` s past it, the tail
    crossfaded into the head so the loop repeats without a seam."""
    count, extra = AMBIENCE_LENGTH, int(overlap * AMBIENCE_RATE)
    samples = [voice(i, i / AMBIENCE_RATE) for i in range(count + extra)]
    loop = samples[extra:count]
    for i in range(extra):
        blend = i / (extra - 1)
        loop.append(samples[count + i] * (1 - blend) + samples[i] * blend)
    return encode(loop, AMBIENCE_RATE)


def events(seed, count, length=AMBIENCE_SECONDS, margin=0.4):
    """`count` start times inside the loop, from `seed`, clear of its seam."""
    noise = Noise(seed)
    return sorted(margin + (noise() * 0.5 + 0.5) * (length - 2 * margin) for _ in range(count))


def tone_burst(t, start, freq, length, glide_to=None, decay=None):
    """A sine burst from `start`: its frequency gliding to `glide_to`, its
    envelope a quick rise and an exponential `decay` (a sine hump without)."""
    u = t - start
    if not 0 <= u < length:
        return 0.0
    f = glide(freq, glide_to, u / length) if glide_to else freq
    env = math.exp(-u / decay) * min(1.0, u / 0.004) if decay else math.sin(math.pi * u / length)
    return math.sin(math.tau * f * u) * env


def ambience_clearing():
    """The clearing at night: crickets in pairs of chirps, a breeze in the
    oaks swelling and falling, an owl's two hoots once a loop."""
    noise, crickets = Noise(0xC1EA), Noise(0xC21C)
    low, high = LowPass(AMBIENCE_RATE), LowPass(AMBIENCE_RATE)
    chirps = [(start, 4100 + 300 * crickets()) for start in events(0xC3, 22)]
    hoots = (2.1, 2.55)

    def voice(i, t):
        n = noise()
        breeze = (low(n, 700) - high(n, 120)) * (0.55 + 0.35 * math.sin(math.tau * t / 3 + 0.4))
        chirp = sum(tone_burst(t, start + k * 0.07, freq, 0.04) * (0.5 + 0.5 * math.sin(math.tau * 32 * t))
                    for start, freq in chirps for k in range(2) if 0 <= t - start < 0.2)
        owl = sum(tone_burst(t, start, 420, 0.32, glide_to=360) for start in hoots)
        return 0.9 * breeze + 0.16 * chirp + 0.3 * owl
    return ambience_loop(voice)


def ambience_aqueduct():
    """The broken aqueduct (01 (k)): the channel's steady flow along the
    trough, one slow swell a loop so the seam stays smooth; the pours'
    patter where they strike the floor, fluttering; a heavier plop now and
    then from a spill; and water trickling, drips falling from its leaks."""
    noise, flow_n, patter_n, rng = Noise(0xA0DC), Noise(0xA1DC), Noise(0xA2DC), Noise(0xA3DC)
    low, high = LowPass(AMBIENCE_RATE), LowPass(AMBIENCE_RATE)
    flow_lo, flow_hi = LowPass(AMBIENCE_RATE), LowPass(AMBIENCE_RATE)
    pat_lo, pat_hi, wob = LowPass(AMBIENCE_RATE), LowPass(AMBIENCE_RATE), LowPass(AMBIENCE_RATE)
    drips = [(start, 1700 + 900 * (rng() * 0.5 + 0.5)) for start in events(0xA1, 11)]
    plops = [(start, 260 + 160 * (rng() * 0.5 + 0.5)) for start in events(0xA4, 7)]

    def voice(i, t):
        n, f, q = noise(), flow_n(), patter_n()
        trickle = (low(n, 2600) - high(n, 600)) * 0.5
        flow = (flow_lo(f, 900) - flow_hi(f, 140)) * (0.75 + 0.25 * math.sin(math.tau * t / 6 + 0.7))
        patter = (pat_lo(q, 5200) - pat_hi(q, 1700)) * (0.55 + 0.45 * abs(wob(rng(), 23)) * 9)
        drip = sum(tone_burst(t, start, freq, 0.18, glide_to=freq * 0.6, decay=0.05) for start, freq in drips)
        plop = sum(tone_burst(t, start, freq, 0.22, glide_to=freq * 1.8, decay=0.06) for start, freq in plops)
        return 0.32 * trickle + 1.1 * flow + 0.5 * patter + 0.32 * drip + 0.4 * plop
    return ambience_loop(voice)


def ambience_ravine():
    """The falls' roar, deep and steady with slow surges, and a hiss of mist
    over it."""
    noise, hiss = Noise(0x4A71), Noise(0x4A72)
    low, mid, air = LowPass(AMBIENCE_RATE), LowPass(AMBIENCE_RATE), LowPass(AMBIENCE_RATE)

    def voice(i, t):
        n = noise()
        roar = low(n, 380) * 2.2 + (mid(n, 1400) - low.y) * 0.5
        surge = 0.8 + 0.2 * math.sin(math.tau * t / 2) * math.sin(math.tau * t / 3 + 1)
        h = hiss()
        mist = (h - air(h, 3000)) * (0.25 + 0.1 * math.sin(math.tau * t / 6))
        return roar * surge + mist
    return ambience_loop(voice)


def ambience_hollow():
    """A low rumble under the basalt, slowly wavering, and the sparse crackle
    of embers."""
    noise, sparks = Noise(0x4011), Noise(0x4012)
    low, lower = LowPass(AMBIENCE_RATE), LowPass(AMBIENCE_RATE)
    state = {'crack': 0.0}

    def voice(i, t):
        n = noise()
        rumble = lower(low(n, 160), 90) * 6 * (0.75 + 0.25 * math.sin(math.tau * t / 3))
        drone = 0.12 * math.sin(math.tau * 41 * t) * (0.6 + 0.4 * math.sin(math.tau * t / 2))
        if sparks() > 0.9992:
            state['crack'] = 0.7
        state['crack'] *= 0.9
        return rumble + drone + state['crack'] * sparks()
    return ambience_loop(voice)


def ambience_mine():
    """Wind over the spoil in gusts, a timber creaking under its load, and a
    distant clank of iron now and then."""
    noise, jitter = Noise(0x31E0), Noise(0x31E1)
    low, high = LowPass(AMBIENCE_RATE), LowPass(AMBIENCE_RATE)
    creaks = events(0x32, 3)
    clanks = events(0x33, 4)

    def voice(i, t):
        n = noise()
        gust = 0.5 + 0.5 * math.sin(math.tau * t / 3) ** 2
        wind = (low(n, 900 + 500 * gust) - high(n, 250)) * gust
        creak = sum(math.copysign(1.0, math.sin(math.tau * (130 + 25 * math.sin(math.tau * 7 * (t - c))) * (t - c)))
                    * math.sin(math.pi * (t - c) / 0.5) * (0.7 + 0.3 * jitter()) for c in creaks if 0 <= t - c < 0.5)
        clank = sum(tone_burst(t, c, f, 0.9, decay=0.2) for c in clanks for f in (523, 1307, 2213))
        return 0.9 * wind + 0.05 * creak + 0.12 * clank
    return ambience_loop(voice)


def ambience_bridge():
    """The river rushing under the arches, wind across the bridge and the
    creak of the troll's ropes."""
    noise, rope = Noise(0xB41D), Noise(0xB41E)
    low, high, gusts = LowPass(AMBIENCE_RATE), LowPass(AMBIENCE_RATE), LowPass(AMBIENCE_RATE)
    creaks = events(0xB5, 4)

    def voice(i, t):
        n = noise()
        rush = (low(n, 1500) - high(n, 180)) * (0.85 + 0.15 * math.sin(math.tau * t * 1.5))
        wind = gusts(rope(), 300) * 3 * (0.4 + 0.6 * math.sin(math.tau * t / 6) ** 2)
        creak = sum(math.sin(math.tau * (210 + 40 * (t - c)) * (t - c)) * math.sin(math.pi * (t - c) / 0.35)
                    for c in creaks if 0 <= t - c < 0.35)
        return rush + 0.5 * wind + 0.08 * creak
    return ambience_loop(voice)


def splinter():
    """A shot through a wooden deck: a sharp crack, a hollow knock and
    splinters rattling away."""
    noise, high = Noise(0x5911), LowPass()
    samples, phase = [], 0.0
    for i in range(int(0.4 * RATE)):
        t = i / RATE
        n = noise()
        crack = (n - high(n, 1800)) * math.exp(-t / 0.008)
        phase += math.tau * glide(620, 380, min(1.0, t / 0.12)) / RATE
        knock = 0.7 * math.sin(phase) * math.exp(-t / 0.05)
        rattle = n * 0.25 * math.exp(-t / 0.12) * (0.5 + 0.5 * math.sin(math.tau * 37 * t))
        samples.append(crack + knock + rattle)
    return finish(samples, fade_in=0.0005)


# 01 (f)'s feel: the holes' hum and collapse, iron's landing, a refusal's
# fizzle and its two siblings, a strike's snap, a thaw's crack, a bowling
# ball's thud, the gun's click and rack, the flamethrower's tail, an air
# jump, a spell switch, an orb dropping, small balls pouring, and the
# deaths' own sounds.


def crossfaded(samples, count, overlap):
    """`samples`, rendered `overlap` past `count`, as a loop of `count`: the
    tail crossfaded into the head, so it repeats without a seam."""
    loop = samples[overlap:count]
    for i in range(overlap):
        blend = i / (overlap - 1)
        loop.append(samples[count + i] * (1 - blend) + samples[i] * blend)
    return loop


def black_hole_hum():
    """An open hole's hum: a deep drone wavering about 46 Hz with a fifth
    over it, a dark whirl of air and a slow, heavy pulse, every swing whole
    over the 2 s loop."""
    duration, overlap = 2.0, int(.12 * RATE)
    noise, whirl, high = Noise(0xB1A0), LowPass(), LowPass()
    count = int(duration * RATE)
    samples, phase, fifth = [], 0.0, 0.0
    for i in range(count + overlap):
        t = i / RATE
        phase += math.tau * (46 + 2.5 * math.sin(math.tau * 0.5 * t)) / RATE
        fifth += math.tau * (69.3 + 3 * math.sin(math.tau * 1.5 * t + 1)) / RATE
        drone = .55 * math.sin(phase) + .25 * math.sin(fifth) + .1 * math.sin(3 * phase)
        n = noise()
        air = (whirl(n, 380 + 220 * math.sin(math.tau * 0.5 * t) ** 2) - high(n, 60)) * 1.6
        pulse = .75 + .25 * math.sin(math.tau * t) ** 2
        samples.append((drone + air) * pulse)
    return finish(crossfaded(samples, count, overlap), fade_in=0, fade_out=0)


def black_hole_open():
    """A black hole tearing open (01 (j)): an inward suck rising in pitch
    over its 0.3 s opening, a deep whomp as it opens and falls into its
    drone, a rumble under it and a faint glassy shimmer going out, about
    0.9 s."""
    duration, open_at = 0.9, 0.28
    noise, low, high, sub = Noise(0xB10E), LowPass(), LowPass(), LowPass()
    samples, phase, shimmer = [], 0.0, 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        n = noise()
        pre = min(1.0, t / open_at)
        # The suck: noise rising from low to high and swelling to the open.
        suck = (low(n, glide(220, 3600, pre)) - high(n, glide(90, 700, pre))) * pre ** 1.8
        if t > open_at:
            suck *= math.exp(-(t - open_at) / 0.025)
        u = t - open_at
        whomp = rumble = glass = 0.0
        if u >= 0:
            phase += math.tau * glide(130, 38, min(1.0, u / 0.35)) / RATE
            whomp = math.sin(phase) * min(1.0, u / 0.004) * math.exp(-u / 0.26)
            rumble = sub(n, 70) * 3.5 * math.exp(-u / 0.4)
            shimmer += math.tau * 1318.5 / RATE
            glass = (math.sin(shimmer) + 0.6 * math.sin(shimmer * 1.498)) * 0.12 * math.exp(-u / 0.3)
        samples.append(math.tanh(1.3 * suck + 1.4 * whomp + rumble + glass))
    return finish(samples, fade_in=0.01, fade_out=0.15)


def black_hole_collapse():
    """A hole closing: an inward rush rising to a snap, then a deep
    implosion falling away under a rumble."""
    duration, snap = 0.9, 0.32
    noise, low, high, sub = Noise(0xB1C0), LowPass(), LowPass(), LowPass()
    samples, phase = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        pre = min(1.0, t / snap)
        n = noise()
        swell = pre ** 2.2 * (1.0 if t < snap else math.exp(-(t - snap) / 0.02))
        rush = (low(n, glide(300, 5200, pre)) - high(n, glide(120, 900, pre))) * swell
        u = t - snap
        thump = rumble = 0.0
        if u >= 0:
            phase += math.tau * glide(95, 26, min(1.0, u / 0.45)) / RATE
            thump = math.sin(phase) * min(1.0, u / 0.003) * math.exp(-u / 0.22)
            rumble = sub(n, 90) * 4.0 * math.exp(-u / 0.35)
        samples.append(math.tanh(1.4 * rush + 1.3 * thump + rumble))
    return finish(samples, fade_in=0.01, fade_out=0.12)


def iron_thud():
    """A block of iron landing hard: a deep thud and knock, a dull clang
    ringing out and grit settling."""
    noise, grit, low, dust = Noise(0x1A0D), Noise(0x1A0E), LowPass(), LowPass()
    samples, phase = [], 0.0
    for i in range(int(0.7 * RATE)):
        t = i / RATE
        phase += math.tau * glide(78, 40, min(1.0, t / 0.2)) / RATE
        thud = math.sin(phase) * min(1.0, t / 0.002) * math.exp(-t / 0.12)
        knock = low(noise(), 600) * 2.0 * math.exp(-t / 0.02)
        clang = sum(math.sin(math.tau * f * t) * w * math.exp(-t / d)
                    for f, w, d in ((187, .35, .32), (431, .22, .2), (733, .14, .14), (1291, .08, .09)))
        settle = dust(grit(), 2200) * .3 * math.exp(-max(0.0, t - .05) / .15) * (t > .05)
        samples.append(math.tanh(1.5 * (thud + knock) + clang + settle))
    return finish(samples, fade_in=0.0005, fade_out=0.08)


def fizzle():
    """A spell with no room to land: a sputtering hiss falling away, a few
    pops and a dull, sinking tone."""
    noise, pops, low, high = Noise(0xF122), Noise(0xF123), LowPass(), LowPass()
    samples, pop, phase = [], 0.0, 0.0
    for i in range(int(0.35 * RATE)):
        t = i / RATE
        p = t / 0.35
        n = noise()
        sputter = .55 + .45 * math.sin(math.tau * glide(38, 14, p) * t)
        hiss = (low(n, glide(5200, 900, p)) - high(n, 600)) * sputter
        if pops() > 1 - .003 * (1 - p):
            pop = .7
        pop *= .88
        phase += math.tau * glide(220, 110, p) / RATE
        dull = .5 * math.sin(phase) * math.exp(-t / .05)
        envelope = min(1.0, t / 0.008) * math.exp(-t / .12)
        samples.append((1.4 * hiss + pop * pops()) * envelope + dull)
    return finish(samples, fade_in=0.001)


def thunder_snap():
    """A bolt landing: a bright electric snap, a zip falling in pitch and a
    short buzzing sizzle."""
    noise, buzz, high = Noise(0x5A9E), Noise(0x5A9F), LowPass()
    samples, phase = [], 0.0
    for i in range(int(0.3 * RATE)):
        t = i / RATE
        n = noise()
        snap = (n - high(n, 2400)) * math.exp(-t / 0.006)
        phase += math.tau * glide(1800, 240, min(1.0, t / 0.08)) / RATE
        zip_ = .35 * math.copysign(1.0, math.sin(phase)) * math.exp(-t / 0.03)
        sizzle = buzz() * .4 * math.exp(-t / 0.07) * (.5 + .5 * math.copysign(1.0, math.sin(math.tau * 90 * t)))
        samples.append(math.tanh(2.0 * snap + zip_ + sizzle))
    return finish(samples, fade_in=0.0003, fade_out=0.03)


def drip(freq, seconds, level):
    """A drop of meltwater: a short sine bloop falling in pitch."""
    out, phase = [], 0.0
    for i in range(int(seconds * RATE)):
        t = i / RATE
        phase += math.tau * glide(freq, freq * 0.6, min(1.0, t / seconds)) / RATE
        out.append(level * math.sin(phase) * min(1.0, t / 0.001) * math.exp(-t / 0.015))
    return out


def ice_crack():
    """A thaw: the ice creaking under strain, a sharp crack splitting it
    with a glassy ring, and meltwater dripping after."""
    samples = [0.0] * int(0.6 * RATE)
    for i in range(int(0.16 * RATE)):
        t = i / RATE
        squeal = math.sin(math.tau * glide(700, 1100, t / 0.16) * t)
        samples[i] += .35 * squeal * math.sin(math.pi * t / 0.16) * (.6 + .4 * math.sin(math.tau * 60 * t))
    noise, low = Noise(0x1C3A), LowPass()
    start = int(0.14 * RATE)
    for i in range(int(0.3 * RATE)):
        t = i / RATE
        n = noise()
        crack = (n - low(n, 1500)) * math.exp(-t / 0.01)
        ring = sum(math.sin(math.tau * f * t) * w * math.exp(-t / d)
                   for f, w, d in ((1630, .3, .09), (2890, .2, .06), (4470, .1, .04)))
        samples[start + i] += 1.6 * crack + ring
    for at, freq in ((0.32, 1900), (0.41, 1500), (0.52, 2100)):
        place(samples, int(at * RATE), drip(freq, 0.06, 0.4))
    return finish(samples, fade_in=0.001)


def ball_thud():
    """A bowling ball landing hard: a low thump under a hollow resin thock
    and a short rattle."""
    noise, low, high = Noise(0xBA11), LowPass(), LowPass()
    samples, phase = [], 0.0
    for i in range(int(0.45 * RATE)):
        t = i / RATE
        phase += math.tau * glide(110, 62, min(1.0, t / 0.1)) / RATE
        thump = math.sin(phase) * min(1.0, t / 0.002) * math.exp(-t / 0.07)
        n = noise()
        thock = (low(n, 1800) - high(n, 500)) * 1.6 * math.exp(-t / 0.012)
        hollow = sum(math.sin(math.tau * f * t) * w * math.exp(-t / d) for f, w, d in ((330, .3, .06), (523, .15, .04)))
        rattle = n * .12 * math.exp(-max(0.0, t - .04) / .08) * (t > .04) * (.5 + .5 * math.sin(math.tau * 31 * t))
        samples.append(math.tanh(1.3 * thump + thock + hollow + rattle))
    return finish(samples, fade_in=0.0005, fade_out=0.05)


def gun_click():
    """A dry fire: the hammer's light metallic click on an empty chamber."""
    noise, high = Noise(0xC11C), LowPass()
    samples = []
    for i in range(int(0.12 * RATE)):
        t = i / RATE
        n = noise()
        click = (n - high(n, 3000)) * math.exp(-t / 0.0025)
        ring = sum(math.sin(math.tau * f * t) * w for f, w in ((2900, .3), (4650, .15))) * math.exp(-t / 0.018)
        samples.append(click + ring)
    return finish(samples, fade_in=0.0002, fade_out=0.01)


def gun_rack():
    """A reload begun: the magazine let go and the slide racked back, two
    hard clacks with a scrape between."""
    noise, low = Noise(0x2AC4), LowPass()
    samples = []
    for i in range(int(0.4 * RATE)):
        t = i / RATE
        out = 0.0
        for at, pitch, level in ((0.0, 1900, 1.0), (0.17, 1300, 1.2)):
            u = t - at
            if u >= 0:
                click = noise() * math.exp(-u / 0.005) * level
                ring = sum(math.sin(math.tau * pitch * ratio * u) * weight
                           for ratio, weight in ((1, .28), (1.61, .14), (2.37, .07))) * math.exp(-u / 0.045)
                out += click + ring
        scrape = low(noise(), 2600) * .5 * math.sin(math.pi * min(1.0, max(0.0, t - .03) / .13)) * (.03 < t < .16)
        samples.append(out + scrape)
    return finish(samples, fade_in=0.0005)


def flame_end():
    """The flamethrower let go: its roar dropping away into a soft puff,
    with a few last crackles."""
    noise, crackles, low, high = Noise(0xF1E0), Noise(0xF1E1), LowPass(), LowPass()
    samples, crack, phase = [], 0.0, 0.0
    for i in range(int(0.45 * RATE)):
        t = i / RATE
        p = t / 0.45
        n = noise()
        rush = (low(n, glide(2400, 300, p)) - high(n, 120)) * math.exp(-t / 0.09)
        phase += math.tau * glide(80, 45, p) / RATE
        puff = .6 * math.sin(phase) * math.exp(-t / .07)
        if crackles() > 1 - .0025 * (1 - p):
            crack = .6
        crack *= .9
        samples.append(1.4 * rush + puff + crack * crackles())
    return finish(samples, fade_in=0.003, fade_out=0.04)


def air_jump():
    """The courier's second jump, a jump's push-off made in the air: a short
    breathy puff of low noise over a soft thump, rising into a quick upward
    whoosh, about a quarter of a second."""
    duration = 0.25
    noise, breath, soft, low, high = Noise(0xA1E0), LowPass(), LowPass(), LowPass(), LowPass()
    samples = []
    for i in range(int(duration * RATE)):
        t = i / RATE
        n = noise()
        # The push-off: low, breathy noise through two poles, in at once and
        # gone by 80 ms.
        puff = soft(breath(n, 700), 700) * (1 - math.exp(-t / 0.003)) * math.exp(-t / 0.03)
        # The rise: a band of noise sweeping up, swelling as the puff fades.
        p = max(0.0, (t - 0.015) / (duration - 0.015))
        rise = (low(n, glide(500, 3000, p)) - high(n, glide(180, 800, p))) * math.sin(math.pi * p) ** 2
        samples.append(3.0 * puff + 1.2 * rise)
    place(samples, 0, thud(140, 0.08, 0.3, 0xA1E1))
    return finish(samples, fade_in=0.002, fade_out=0.03)


def no_mana():
    """Out of mana: a hollow, empty knock and a tone sinking away, as from
    a dry lantern."""
    noise, low = Noise(0x0A1A), LowPass()
    samples, phase = [], 0.0
    for i in range(int(0.4 * RATE)):
        t = i / RATE
        p = t / 0.4
        phase += math.tau * glide(330, 165, p ** .7) / RATE
        tone = (math.sin(phase) + .3 * math.sin(2 * phase + .4)) * min(1.0, t / 0.01) * math.exp(-t / 0.13)
        knock = low(noise(), 500) * 1.5 * math.exp(-t / 0.015)
        samples.append(.8 * tone + knock)
    return finish(samples, fade_in=0.0005, fade_out=0.05)


def spell_switch():
    """A spell chosen: a soft click and a short glassy tick in its slot."""
    noise, high = Noise(0x5E11), LowPass()
    samples = [0.0] * int(0.14 * RATE)
    for i in range(int(0.02 * RATE)):
        t = i / RATE
        n = noise()
        samples[i] += (n - high(n, 2500)) * .6 * math.exp(-t / 0.003)
    place(samples, int(0.008 * RATE), chime(note('E6'), 0.12, 0.35, decay=0.035))
    return finish(samples, fade_in=0.0003, fade_out=0.02)


def not_ready():
    """A spell still cooling: two muted low ticks, the slot not ready."""
    noise, low = Noise(0x40AD), LowPass()
    samples = [0.0] * int(0.2 * RATE)
    for at in (0.0, 0.085):
        start = int(at * RATE)
        for i in range(int(0.06 * RATE)):
            t = i / RATE
            tick = (math.sin(math.tau * 520 * t) + .4 * math.sin(math.tau * 1040 * t)) * math.exp(-t / 0.012)
            samples[start + i] += tick + low(noise(), 800) * .8 * math.exp(-t / 0.004)
    return finish(samples, fade_in=0.0003, fade_out=0.02)


def orb_pop():
    """A drop falling from a kill: a bubbly pop rising, and a little
    sparkle of three bells."""
    noise, low = Noise(0x0B0B), LowPass()
    samples, phase = [0.0] * int(0.32 * RATE), 0.0
    for i in range(int(0.06 * RATE)):
        t = i / RATE
        phase += math.tau * glide(260, 720, t / 0.06) / RATE
        pop = math.sin(phase) * min(1.0, t / 0.002) * math.exp(-t / 0.02)
        samples[i] += pop + low(noise(), 3000) * .3 * math.exp(-t / 0.004)
    for k, name in enumerate(('B5', 'E6', 'G#6')):
        place(samples, int((0.05 + 0.04 * k) * RATE), chime(note(name), 0.18, 0.25, decay=0.05))
    return finish(samples, fade_in=0.0005, fade_out=0.04)


def small_balls():
    """Small balls pouring: a dense patter of little glass clicks over a
    soft hiss and the low roll of the pile, crossfaded as a 2 s loop."""
    duration, overlap = 2.0, int(.1 * RATE)
    noise, clicks, high, low = Noise(0x5BA1), Noise(0x5BA2), LowPass(), LowPass()
    count = int(duration * RATE)
    life = int(0.03 * RATE)
    samples, voices = [], []
    for i in range(count + overlap):
        n = noise()
        hiss = (n - high(n, 3500)) * .12
        roll = low(n, 300) * .6
        if clicks() > .9972:
            voices.append((i, 2600 + 2400 * (clicks() * .5 + .5), .3 + .4 * (clicks() * .5 + .5)))
        voices = [(s, f, v) for s, f, v in voices if i - s < life]
        patter = sum(v * math.sin(math.tau * f * (i - s) / RATE) * math.exp(-(i - s) / RATE / 0.006)
                     for s, f, v in voices)
        samples.append(hiss + roll + patter)
    return finish(crossfaded(samples, count, overlap), fade_in=0, fade_out=0)


def enemy_death():
    """An enemy killed: a heavy last thwack and crunch, a falling groan of
    its body, and the body's fall."""
    noise, crunch, low, high = Noise(0xDEA0), Noise(0xDEA1), LowPass(), LowPass()
    samples, phase, fall = [], 0.0, 0.0
    for i in range(int(0.7 * RATE)):
        t = i / RATE
        n = noise()
        thwack = low(n, 1300) * 2.4 * math.exp(-t / 0.035)
        phase += math.tau * glide(150, 48, min(1.0, t / 0.18)) / RATE
        body = math.sin(phase) * math.exp(-t / 0.1)
        c = crunch()
        bits = (c - high(c, 2200)) * .6 * math.exp(-t / 0.09)
        u = t - 0.32
        thud = 0.0
        if u >= 0:
            fall += math.tau * glide(80, 45, min(1.0, u / .15)) / RATE
            thud = .8 * math.sin(fall) * min(1.0, u / .003) * math.exp(-u / .08)
        samples.append(math.tanh(1.4 * (thwack + body) + bits + thud))
    return finish(samples, fade_in=0.0005, fade_out=0.06)


def dummy_thump():
    """The dummy knocked out: a heavy sackcloth thump and its straw
    bursting and settling."""
    noise, rustle, low, high = Noise(0xD0D0), Noise(0xD0D1), LowPass(), LowPass()
    samples, phase = [], 0.0
    for i in range(int(0.6 * RATE)):
        t = i / RATE
        n = noise()
        thump = low(n, 340) * 3.0 * math.exp(-t / 0.05)
        phase += math.tau * glide(95, 52, min(1.0, t / 0.12)) / RATE
        body = .9 * math.sin(phase) * math.exp(-t / 0.08)
        r = rustle()
        straw = (r - high(r, 3200)) * .55 * math.exp(-t / 0.2) * (.6 + .4 * math.sin(math.tau * 23 * t))
        samples.append(math.tanh(1.2 * (thump + body)) + straw)
    return finish(samples, fade_in=0.001, fade_out=0.06)


# 01 (g)'s props and foes: a crate catching, its fire's loop and its fall,
# a keg's fuse and its boom, the goblin's throw, its rock landing and its
# cackle, and the troll's swipe and its grunt before a slam.


def crate_ignite():
    """Boards catching: a dry whoomph of flame drawing in and up over a soft
    low thump, then the wood's first crackles."""
    duration = 0.6
    noise, crackles, low, high = Noise(0x16A1), Noise(0x16A2), LowPass(), LowPass()
    samples, crack, phase = [], 0.0, 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        whoomph = (low(n, glide(260, 2400, min(1.0, p * 2.2))) - high(n, 140)) \
            * min(1.0, t / 0.05) * math.exp(-t / 0.2)
        phase += math.tau * glide(80, 50, min(1.0, t / 0.25)) / RATE
        thump = .6 * math.sin(phase) * min(1.0, t / 0.01) * math.exp(-t / 0.12)
        c = crackles()
        if t > 0.12 and c > 1 - .004 * (1 - p):
            crack = .7
        crack *= .9
        samples.append(math.tanh(1.8 * whoomph + thump) + crack * c)
    return finish(samples, fade_out=0.08)


def fire_crackle():
    """Anything alight (e01sfx, V31), one voice by the nearest: a fire's
    hiss breathing over a soft low flutter, its crackle and the pops of its
    sap, every swing whole over the 2 s loop."""
    duration, overlap = 2.0, int(.12 * RATE)
    noise, crackles = Noise(0x16B1), Noise(0x16B2)
    low, high, hiss_low, hiss_high, crisp = (LowPass() for _ in range(5))
    count = int(duration * RATE)
    samples, crack, pop_at, pop_freq = [], 0.0, -1.0, 300.0
    for i in range(count + overlap):
        t = i / RATE
        n = noise()
        breath = .72 + .28 * math.sin(math.tau * 1.5 * t) * math.sin(math.tau * 0.5 * t + 1)
        flutter = (low(n, 320) - high(n, 60)) * (.8 + .2 * math.sin(math.tau * 6 * t))
        hiss = (hiss_low(n, 4800) - hiss_high(n, 1400)) * breath
        c = crackles()
        if c > .9982:
            crack = .85 if c > .9995 else .5
        crack *= .88
        snap = crack * (c - crisp(c, 2400))
        if c < -.99975:
            pop_at, pop_freq = t, 260 + 640 * (c + 1) * 4000
        u = t - pop_at
        pop = .7 * math.sin(math.tau * pop_freq * u) * math.exp(-u / .01) if u >= 0 else 0.0
        samples.append(1.5 * flutter + 0.75 * hiss + snap + pop)
    return finish(crossfaded(samples, count, overlap), fade_in=0, fade_out=0)


def crate_collapse():
    """A burnt crate falling in: a creak, a splintering crunch, its boards
    dropping and knocking, and a gust of sparks."""
    duration = 0.8
    noise, grit, creaky, high, gust, low = (Noise(0x16C1), Noise(0x16C2), LowPass(), LowPass(), LowPass(),
                                            LowPass())
    samples, phase, crack = [], 0.0, 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        n = noise()
        creak = 0.0
        if t < 0.2:
            phase += glide(150, 230, t / 0.2) * (1 + .08 * math.sin(math.tau * 31 * t)) / RATE
            creak = .9 * creaky(2 * (phase % 1.0) - 1, 900) * math.sin(math.pi * t / 0.2)
        u = t - 0.17
        crunch = 0.0
        g = grit()
        if u >= 0:
            crunch = (n - high(n, 1600)) * 1.4 * math.exp(-u / 0.07)
            if g > 1 - .006 * math.exp(-u / .2):
                crack = .8
        crack *= .88
        knocks = sum(.7 * math.sin(math.tau * f * (t - s)) * math.exp(-(t - s) / .03)
                     for s, f in ((.3, 210), (.42, 170), (.55, 240)) if t >= s)
        rise = max(0.0, min(1.0, (t - .2) / .6))
        sparks = (gust(n, 3000) - low(n, 900)) * .5 * math.sin(math.pi * rise)
        samples.append(math.tanh(creak + crunch + knocks + crack * g) + sparks)
    return finish(samples, fade_out=0.08)


def fuse_hiss():
    """A lit fuse burning down, as long as its 1.5 s: a sputtering hiss
    rising in pitch, spitting crackles."""
    duration = 1.5
    noise, spits, low, high = Noise(0x16D1), Noise(0x16D2), LowPass(), LowPass()
    samples, spit, phase = [], 0.0, 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        hiss = low(n, glide(3000, 7500, p)) - high(n, glide(1200, 2600, p))
        phase += math.tau * glide(18, 34, p) / RATE
        sputter = .6 + .4 * math.sin(phase) ** 2
        s = spits()
        if s > 1 - .003:
            spit = .7
        spit *= .85
        envelope = min(1.0, t / 0.03) * (.7 + .3 * p)
        samples.append((1.6 * hiss * sputter + spit * s) * envelope)
    return finish(samples, fade_out=0.05)


def keg_boom():
    """A powder keg blowing: a sharp crack over a deep boom, a rolling
    rumble, and splinters and grit raining down after."""
    duration = 1.1
    noise, debris, low, rumble, patter = Noise(0x16E1), Noise(0x16E2), LowPass(), LowPass(), LowPass()
    samples, phase, bit = [], 0.0, 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        crack = (n - low(n, 2200)) * 1.6 * math.exp(-t / 0.018)
        phase += math.tau * glide(64, 24, min(1.0, t / 0.55)) / RATE
        boom = 1.3 * math.sin(phase) * min(1.0, t / 0.002) * math.exp(-t / 0.32)
        roll = rumble(n, 150) * 3.4 * math.exp(-t / 0.5)
        d = debris()
        if t > 0.12 and d > 1 - .005 * (1 - p):
            bit = .55 + .3 * abs(n)
        bit *= .93
        rain = bit * (d - patter(d, 1800))
        samples.append(math.tanh(1.7 * (crack + boom + roll)) + rain)
    return finish(samples, fade_in=0.0005, fade_out=0.1)


def rock_throw():
    """A goblin's throw: a short, sharp whoosh."""
    duration = 0.25
    noise, low, high = Noise(0x16F1), LowPass(), LowPass()
    samples = []
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        whoosh = low(n, glide(700, 3400, p)) - high(n, 350)
        envelope = math.sin(math.pi * min(1.0, p * 1.1)) ** 1.5
        samples.append(1.8 * whoosh * envelope)
    return finish(samples)


def rock_hit():
    """A rock landing: a stony clack, a hollow knock under it and grit
    skittering away."""
    noise, grit, low, high = Noise(0x1701), Noise(0x1702), LowPass(), LowPass()
    samples, chip = [], 0.0
    for i in range(int(0.3 * RATE)):
        t = i / RATE
        n = noise()
        clack = sum(math.sin(math.tau * f * t) * w for f, w in ((1250, .5), (2130, .35), (3410, .2))) \
            * math.exp(-t / 0.018)
        knock = low(n, 700) * 1.6 * math.exp(-t / 0.025)
        g = grit()
        if t > 0.02 and g > 1 - .006 * math.exp(-t / .1):
            chip = .6
        chip *= .8
        skitter = chip * (g - high(g, 3000))
        samples.append(math.tanh(1.3 * (clack + knock)) + skitter)
    return finish(samples, fade_in=0.0005, fade_out=0.04)


def goblin_cackle():
    """A goblin's cackle on its alert: high, nasal and quick, five
    syllables falling in pitch."""
    duration = 0.6
    noise, low, high, nose = Noise(0x1711), LowPass(), LowPass(), LowPass()
    samples, phase = [], 0.0
    starts = (0.0, 0.1, 0.19, 0.28, 0.37)
    for i in range(int(duration * RATE)):
        t = i / RATE
        k = max(j for j, s in enumerate(starts) if t >= s)
        u = t - starts[k]
        freq = (560 - 30 * k) * (1 + .06 * math.exp(-u / .02)) * (1 + .02 * math.sin(math.tau * 30 * t))
        phase += freq / RATE
        saw = 2 * (phase % 1.0) - 1
        voice = low(saw, 2600) - high(saw, 700)
        twang = (saw - nose(saw, 3800)) * .35
        syllable = min(1.0, u / .008) * math.exp(-u / (.075 if k == 4 else .045))
        samples.append((1.4 * voice + twang + .15 * noise()) * syllable)
    return finish(samples, fade_out=0.05)


def goblin_stab():
    """A goblin's stab: a quick, thin swish of its knife and a bright
    tick of steel at its end."""
    duration = 0.22
    noise, low, high = Noise(0x1741), LowPass(), LowPass()
    samples = []
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        swish = (low(n, glide(1800, 6200, p)) - high(n, 900)) * math.sin(math.pi * min(1.0, p * 1.6)) ** 2
        ting = sum(math.sin(math.tau * f * t) * w for f, w in ((3520, .3), (5280, .18))) \
            * math.exp(-max(0.0, t - 0.06) / 0.03) * (t > 0.06)
        samples.append(1.6 * swish + ting)
    return finish(samples, fade_out=0.03)


def troll_swipe():
    """A troll's swipe: a heavy whoosh of air as its arm rakes through."""
    duration = 0.35
    noise, low, high, body = Noise(0x1721), LowPass(), LowPass(), LowPass()
    samples = []
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        sweep = math.sin(math.pi * min(1.0, p * 1.15))
        whoosh = low(n, 250 + 1700 * sweep) - high(n, 90)
        rumble = body(n, 160) * 2.0
        samples.append(2.0 * (whoosh + rumble) * sweep ** 1.3)
    return finish(samples)


def troll_grunt():
    """A troll's grunt as it winds up its slam: deep, short and effortful,
    with a rasp in its throat."""
    duration = 0.5
    noise, throat, formant, low = Noise(0x1731), Noise(0x1732), LowPass(), LowPass()
    samples, phase = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        freq = glide(92, 68, p) * (1 + .03 * math.sin(math.tau * 9 * t))
        phase += freq / RATE
        saw = 2 * (phase % 1.0) - 1
        voice = formant(saw, 520 - 120 * p)
        rasp = low(throat(), 1100) * (.5 + .5 * math.sin(math.tau * 27 * t))
        envelope = min(1.0, t / 0.04) * (1 - p) ** 1.4
        samples.append(math.tanh(2.4 * (voice + .6 * rasp) * envelope + .1 * noise() * envelope))
    return finish(samples, fade_out=0.06)


# e01tomes: each book taken, apart from an orb's pickup and from the others,
# and a bound, apart from the air jump's push-off.


def tome_pact():
    """The demonic pact taken: a deep minor chord with a tritone in it
    swelling out of the dark, a heartbeat's two thumps under it and the
    crackle of embers over it, about a second."""
    duration = 1.1
    noise, embers, low, crisp = Noise(0x7A01), Noise(0x7A02), LowPass(), LowPass()
    samples, crack = [], 0.0
    chord = ((note('D2'), 1.0), (note('A2'), 0.7), (note('F3'), 0.45), (note('G#3'), 0.32))
    for i in range(int(duration * RATE)):
        t = i / RATE
        swell = min(1.0, t / 0.08) * math.exp(-max(0.0, t - 0.15) / 0.45)
        tone = sum(weight * math.sin(math.tau * freq * t * (1 + 0.004 * k) + k)
                   * (1 + 0.25 * math.sin(math.tau * (4.5 + k) * t))
                   for k, (freq, weight) in enumerate(chord))
        growl = low(noise(), 240) * 1.6
        e = embers()
        if e > .9975 and 0.12 < t < 0.95:
            crack = .7 if e > .9993 else .4
        crack *= .88
        snap = crack * (e - crisp(e, 3000))
        samples.append((tone + growl) * swell * 0.55 + snap * (1 - t / duration))
    for at in (0.0, 0.2):  # lub, dub
        place(samples, int(at * RATE), thud(58 if at else 64, 0.16, 1.1 if at else 0.8, 0x7A03))
    return finish(samples, fade_in=0.002, fade_out=0.12)


def tome_bounding():
    """The tome of bounding taken: a bright chime climbing an octave, C6 to
    C7, a shimmer above it and a soft whoosh of air rising with it."""
    duration = 0.95
    noise, low, high = Noise(0x7B01), LowPass(), LowPass()
    samples = []
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        whoosh = (low(n, glide(600, 4200, min(1.0, p * 1.6))) - high(n, glide(240, 1500, min(1.0, p * 1.6))))
        samples.append(1.3 * whoosh * math.sin(math.pi * min(1.0, p * 1.25)) ** 2)
    for k, name in enumerate(('C6', 'E6', 'G6', 'C7')):
        place(samples, int((0.04 + 0.075 * k) * RATE), chime(note(name), 0.6, 0.5 + 0.08 * k, decay=0.22))
    place(samples, int(0.34 * RATE), chime(note('G7'), 0.5, 0.18, decay=0.16))
    return finish(samples, fade_in=0.002, fade_out=0.1)


def tome_arcane():
    """The tome of arcane power taken: a hum swelling under a shimmering
    arpeggio climbing an A major ninth, its partials beating, its last note
    ringing out."""
    duration = 1.15
    noise, low = Noise(0x7C01), LowPass()
    samples = []
    for i in range(int(duration * RATE)):
        t = i / RATE
        swell = math.sin(math.pi * min(1.0, t / duration)) ** 1.5
        hum = sum(w * math.sin(math.tau * f * t) for f, w in ((110, 1.0), (165.4, 0.5), (220.6, 0.4)))
        hum *= 1 + 0.3 * math.sin(math.tau * 6 * t)
        air = low(noise(), 1800) * 0.25
        samples.append((hum * 0.6 + air) * swell)
    shimmer = [(1.0, 1.0, 1.0), (2.005, 0.5, 0.6), (3.02, 0.25, 0.4), (4.4, 0.12, 0.25)]
    for k, name in enumerate(('A5', 'C#6', 'E6', 'G#6', 'B6', 'E7')):
        place(samples, int((0.08 + 0.065 * k) * RATE),
              chime(note(name), 0.7, 0.3 + 0.03 * k, decay=0.16 + 0.04 * (k == 5), partials=shimmer))
    return finish(samples, fade_in=0.003, fade_out=0.12)


def bound():
    """A bound: a bright, airy spring, a quick whoosh of air sweeping up and
    a crystalline ping as the ring opens, under a third of a second."""
    duration = 0.32
    noise, low, high = Noise(0x7D01), LowPass(), LowPass()
    samples = []
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        rise = (low(n, glide(1200, 7000, p)) - high(n, glide(500, 2600, p)))
        samples.append(1.4 * rise * min(1.0, t / 0.006) * (1 - p) ** 1.6)
    crystal = [(1.0, 1.0, 1.0), (2.76, 0.35, 0.45), (5.4, 0.18, 0.25)]
    place(samples, int(0.01 * RATE), chime(note('E7'), 0.28, 0.55, decay=0.07, partials=crystal))
    place(samples, int(0.055 * RATE), chime(note('B7'), 0.24, 0.3, decay=0.05, partials=crystal))
    return finish(samples, fade_in=0.001, fade_out=0.05)


# e01foes' sounds: the beetle's rising paw and its plough through a body,
# the crawler's backing hiss and its hop, the snowball's throw and burst,
# and the goblin's snicker, dash and panicked yelp.


def beetle_paw():
    """The charge's tell, its 0.8 s: a foot raking grit twice, each scrape
    longer, higher and louder, over an iron growl that climbs into the
    charge."""
    duration = 0.8
    noise, grit, band, floor, growl = Noise(0xBE91), Noise(0xBE92), LowPass(), LowPass(), LowPass()
    samples, phase, crack = [], 0.0, 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        scrape = 0.0
        for start, length, level in ((0.17, 0.2, 0.65), (0.57, 0.22, 1.0)):
            u = (t - start) / length
            if 0 <= u <= 1:
                centre = glide(500, 2400, (start + u * length) / duration)
                rasp = band(n, centre * 1.8) - floor(n, centre * 0.5)
                g = grit()
                if g > 0.96:
                    crack = 0.9
                crack *= 0.9
                scrape += level * (rasp * (0.6 + 0.4 * abs(math.sin(math.tau * 46 * t))) + 0.5 * crack * g) \
                    * math.sin(math.pi * u) ** 0.7
        phase += glide(42, 96, p ** 1.4) / RATE
        saw = 2 * (phase % 1.0) - 1
        roar = growl(saw, 260 + 520 * p) * (0.1 + 0.9 * p ** 2.2)
        samples.append(math.tanh(2.2 * scrape + 0.7 * roar))
    return finish(samples, fade_out=0.03)


def beetle_plough():
    """A charge ploughing into a crate or a keg: a heavy wooden thud, a
    hollow knock under it and a rattle of staves."""
    duration = 0.32
    noise, high, low = Noise(0xBE95), LowPass(), LowPass()
    body = thud(70, duration, 1.0, 0xBE96)
    samples, phase = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        n = noise()
        phase += math.tau * glide(260, 170, min(1.0, t / 0.08)) / RATE
        knock = 0.9 * math.sin(phase) * math.exp(-t / 0.04)
        crack = (n - high(n, 1500)) * 0.7 * math.exp(-t / 0.01)
        rattle = low(n, 2200) * 0.35 * math.exp(-max(0.0, t - 0.03) / 0.07) * (0.5 + 0.5 * math.sin(math.tau * 29 * t))
        samples.append(math.tanh(1.3 * body[i] + knock + crack + rattle))
    return finish(samples, fade_in=0.0005, fade_out=0.04)


def crawler_retreat():
    """The crawler backing out of the light: a long, breathy hiss that sinks
    and fades as it withdraws, over its legs chittering backwards; lower
    and slower than its scare's startled hiss."""
    duration = 0.6
    noise, ticks, low, floor = Noise(0xC4B1), Noise(0xC4B2), LowPass(), LowPass()
    samples, tick = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        hiss = low(n, glide(3000, 1300, p)) - floor(n, 500)
        sway = 0.75 + 0.25 * math.sin(math.tau * 7 * t)
        if ticks() > 0.997 - 0.002 * math.sin(math.tau * 9 * t):
            tick = 1.0
        tick *= 0.8
        chitter = tick * math.sin(math.tau * (3100 + 600 * math.sin(math.tau * 13 * t)) * t)
        envelope = min(1.0, t / 0.04) * (1 - p) ** 0.8
        samples.append((1.1 * hiss * sway + 1.4 * chitter) * envelope)
    return finish(samples, fade_out=0.05)


def crawler_hop():
    """A crawler's hop: a quick chitter of its legs bunching, then the click
    of its spring."""
    duration = 0.2
    noise, high = Noise(0xC4B5), LowPass()
    samples = []
    for i in range(int(duration * RATE)):
        t = i / RATE
        n = noise()
        chitter = 0.0
        for k, start in enumerate((0.0, 0.018, 0.034, 0.052, 0.068)):
            u = t - start
            if u >= 0:
                chitter += math.sin(math.tau * (2900 + 260 * k) * u) * math.exp(-u / 0.006)
        u = t - 0.095
        click = 0.0
        if u >= 0:
            click = ((n - high(n, 2500)) * 1.2 + math.sin(math.tau * 1700 * u)) * math.exp(-u / 0.012)
        samples.append(0.6 * chitter + click)
    return finish(samples, fade_in=0.0005, fade_out=0.02)


def snowball_throw():
    """A snowball packed and thrown: a crunch of snow squeezed tight, then
    the flick of the head and a short whoosh."""
    duration = 0.28
    noise, grain, high, band, floor = Noise(0x5B01), Noise(0x5B02), LowPass(), LowPass(), LowPass()
    samples, crunch = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        n = noise()
        g = grain()
        if t < 0.1 and g > 0.93:
            crunch = 0.9
        crunch *= 0.86
        packed = crunch * (n - high(n, 1400)) * min(1.0, t / 0.01) * math.exp(-t / 0.07)
        u = (t - 0.11) / 0.15
        whoosh = 0.0
        if 0 <= u <= 1:
            whoosh = (band(n, glide(900, 5200, u)) - floor(n, 600)) * math.sin(math.pi * u) ** 1.4
        samples.append(1.5 * packed + 1.6 * whoosh)
    return finish(samples, fade_out=0.03)


def snowball_burst():
    """A snowball bursting: a soft crump of powder and a scatter of ice
    tinkling down after it."""
    duration = 0.42
    noise, low, mid = Noise(0x5B05), LowPass(), LowPass()
    samples = []
    for i in range(int(duration * RATE)):
        t = i / RATE
        n = noise()
        crump = (low(n, 420) * 2.6 + (mid(n, 1600) - low(n, 420)) * 0.5) * min(1.0, t / 0.004) * math.exp(-t / 0.05)
        powder = (n - mid(n, 2500)) * 0.18 * math.exp(-t / 0.12)
        samples.append(math.tanh(1.4 * crump) + powder)
    ice = [(1.0, 1.0, 1.0), (2.4, 0.4, 0.5), (4.1, 0.2, 0.3)]
    for start, name, level in ((0.03, 'F#7', 0.32), (0.07, 'C#8', 0.22), (0.12, 'A7', 0.26), (0.19, 'E8', 0.16),
                               (0.27, 'B7', 0.14)):
        place(samples, int(start * RATE), chime(note(name), 0.14, level, decay=0.03, partials=ice))
    return finish(samples, fade_in=0.0005, fade_out=0.05)


def goblin_voice(freqs, starts, lengths, seconds, seed, breath=0.25, nasal=0.35):
    """A goblin's voice: nasal syllables at `freqs`, each from its start for
    its length, through the cackle's throat."""
    noise, low, high, nose = Noise(seed), LowPass(), LowPass(), LowPass()
    samples, phase = [], 0.0
    for i in range(int(seconds * RATE)):
        t = i / RATE
        k = max((j for j, s in enumerate(starts) if t >= s), default=0)
        u = t - starts[k]
        f0, f1 = freqs[k]
        freq = glide(f0, f1, min(1.0, u / lengths[k])) * (1 + .03 * math.sin(math.tau * 26 * t))
        phase += freq / RATE
        saw = 2 * (phase % 1.0) - 1
        voice = low(saw, 2800) - high(saw, 650)
        twang = (saw - nose(saw, 3900)) * nasal
        on = min(1.0, u / .006) * math.exp(-max(0.0, u - lengths[k] * 0.6) / (lengths[k] * 0.35))
        samples.append((1.4 * voice + twang + breath * noise()) * on)
    return samples


def goblin_crouch():
    """The dash-in's crouch: a sly, breathy snicker under its breath, three
    quick nasal syllables, the last turned up."""
    samples = goblin_voice(((430, 410), (420, 395), (440, 520)), (0.0, 0.07, 0.14), (0.05, 0.05, 0.11), 0.3,
                           0x1751, breath=0.45, nasal=0.3)
    return finish([0.8 * s for s in samples], fade_out=0.04)


def goblin_dash():
    """The dash-in: a scuffle of feet on grit, then a whoosh as it darts
    ahead."""
    duration = 0.3
    noise, grit, low, floor, band = Noise(0x1755), Noise(0x1756), LowPass(), LowPass(), LowPass()
    samples, scrape = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        n = noise()
        g = grit()
        scuff = 0.0
        for start in (0.0, 0.045, 0.085):
            u = t - start
            if 0 <= u < 0.04:
                scuff += (low(n, 1800) - floor(n, 250)) * math.sin(math.pi * u / 0.04)
        if t < 0.12 and g > 0.95:
            scrape = 0.7
        scrape *= 0.85
        u = (t - 0.09) / 0.2
        whoosh = 0.0
        if 0 <= u <= 1:
            whoosh = (band(n, glide(700, 3800, u)) - floor(n, 300)) * math.sin(math.pi * u) ** 1.2
        samples.append(1.3 * scuff + 0.5 * scrape * g + 1.5 * whoosh)
    return finish(samples, fade_out=0.03)


def goblin_yelp():
    """A goblin catching fire: a high squeal that breaks into panicked
    gabble, quick syllables tumbling over each other; higher and faster
    than its cackle."""
    squeal = [(720, 1350)]
    gabble = [(980, 860), (840, 940), (1010, 820), (880, 980), (950, 800), (820, 900), (900, 760)]
    starts = [0.0] + [0.2 + 0.055 * k for k in range(len(gabble))]
    lengths = [0.19] + [0.045] * len(gabble)
    samples = goblin_voice(squeal + gabble, starts, lengths, 0.62, 0x1759, breath=0.3, nasal=0.45)
    return finish(samples, fade_out=0.06)


# e01sfx's sounds: a strike on the ground, a damage tick on the player and a
# crate breaking into planks; `thunder` and `fire_crackle` are redrawn in
# place.


def ground_strike():
    """A bolt meeting the ground: a buzzing snap, the electric crackle of
    its sparks and a sizzle hissing away over 0.75 s."""
    duration = 0.75
    noise, sparks, flutter = Noise(0x6511), Noise(0x6512), Noise(0x6513)
    snap_high, hiss_high, hiss_low, buzz_low, tone_low = LowPass(), LowPass(), LowPass(), LowPass(), LowPass()
    samples, phase, spark, sizzle = [], 0.0, 0.0, 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        n = noise()
        snap = (n - snap_high(n, 2600)) * math.exp(-t / 0.005)
        phase += math.tau * glide(170, 72, min(1.0, t / 0.3)) / RATE
        gate = 0.5 + 0.5 * math.copysign(1.0, math.sin(phase))
        buzz = (buzz_low(n, 3200) * gate * 0.8 + tone_low(math.copysign(1.0, math.sin(phase)), 900) * 0.35) \
            * min(1.0, t / 0.004) * math.exp(-t / 0.11)
        s = sparks()
        if s > 1 - 0.012 * math.exp(-t / 0.16):
            spark = 0.9 if s > 1 - 0.004 else 0.55
        spark *= 0.86
        if i % 400 == 0:
            sizzle = 0.55 + 0.45 * flutter()
        hiss = (hiss_low(n, 7000) - hiss_high(n, 2800)) * sizzle * min(1.0, t / 0.04) * math.exp(-t / 0.24)
        samples.append(math.tanh(2.2 * snap + buzz + spark * s + 1.3 * hiss))
    return finish(samples, fade_in=0.0003, fade_out=0.08)


def player_tick():
    """A damage tick on the courier: a soft, muted thud, low and short, under
    a hurt's crack, falling tone and yelp."""
    duration = 0.14
    noise, low = Noise(0x71C4), LowPass()
    samples, phase = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        phase += math.tau * glide(165, 92, min(1.0, t / 0.08)) / RATE
        body = math.sin(phase) * min(1.0, t / 0.004) * math.exp(-t / 0.034)
        knock = low(noise(), 480) * 2.4 * math.exp(-t / 0.009)
        samples.append(math.tanh(1.2 * (body + knock)))
    return finish(samples, fade_in=0.001, fade_out=0.03)


def crate_break():
    """A crate breaking under a blow: a sharp wooden crack, a splintering
    crunch, then its planks clattering down and knocking about."""
    duration = 0.7
    noise, grit = Noise(0x6B71), Noise(0x6B72)
    crack_high, crunch_high, crunch_low, click_high = LowPass(), LowPass(), LowPass(), LowPass()
    knocks = ((0.17, 236, 0.9), (0.24, 318, 0.6), (0.31, 196, 0.75), (0.4, 282, 0.5), (0.52, 214, 0.45),
              (0.6, 300, 0.3))
    samples, phase, splinter = [], 0.0, 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        n = noise()
        crack = (n - crack_high(n, 2200)) * math.exp(-t / 0.007)
        phase += math.tau * glide(540, 300, min(1.0, t / 0.05)) / RATE
        snap = 0.8 * math.sin(phase) * math.exp(-t / 0.035)
        u = t - 0.012
        crunch = (crunch_low(n, 3000) - crunch_high(n, 650)) * 1.3 * math.exp(-u / 0.06) if u >= 0 else 0.0
        g = grit()
        if 0.02 < t < 0.4 and g > 1 - 0.01 * math.exp(-t / 0.12):
            splinter = 0.8
        splinter *= 0.85
        clatter = 0.0
        for start, freq, level in knocks:
            d = t - start
            if d >= 0:
                clatter += level * (math.sin(math.tau * freq * d) * math.exp(-d / 0.028)
                                    + (n - click_high(n, 1500)) * 0.6 * math.exp(-d / 0.004))
        samples.append(math.tanh(1.8 * crack + snap + crunch + splinter * g + clatter))
    return finish(samples, fade_in=0.0003, fade_out=0.06)


def recut(data, seconds):
    """A one-shot's WAV `data` cropped, or cycled from its start, to
    `seconds`, faded and normalized again: e01hollow's placeholders until
    its asset step makes their sounds."""
    with wave.open(io.BytesIO(data), 'rb') as wav:
        frames = wav.readframes(wav.getnframes())
    source = [int.from_bytes(frames[i:i + 2], 'little', signed=True) / 32767
              for i in range(0, len(frames), 2)]
    return finish([source[i % len(source)] for i in range(int(seconds * RATE))], fade_out=0.04)


AMBIENCE = {
    'ambience_clearing.wav': ambience_clearing,
    'ambience_aqueduct.wav': ambience_aqueduct,
    'ambience_ravine.wav': ambience_ravine,
    'ambience_hollow.wav': ambience_hollow,
    'ambience_mine.wav': ambience_mine,
    'ambience_bridge.wav': ambience_bridge,
}


# Each loop's length in samples: its bars exactly, an ambience its seconds.
LOOPS = {
    **{name: AMBIENCE_LENGTH for name in AMBIENCE},
    'music_title.wav': 12 * 4 * (MUSIC_RATE * 60 // 96),
    'music_explore.wav': 16 * 4 * (MUSIC_RATE * 60 // 128),
    'music_pause.wav': 5 * 4 * (MUSIC_RATE * 60 // 80),
    'music_over.wav': 4 * 4 * (MUSIC_RATE * 60 // 66),
    'music_combat.wav': 16 * 4 * (MUSIC_RATE * 60 // 128),
    # 01 (f)'s two voices 01's code holds, and 01 (g)'s fire: 2 s each at
    # 44,100 Hz.
    'black_hole_hum.wav': 2 * RATE,
    'small_balls.wav': 2 * RATE,
    'fire_crackle.wav': 2 * RATE,
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
        'music_combat.wav': encode(music_combat(), MUSIC_RATE),
        'fireball_cast.wav': fireball_cast(),
        'fireball_blast.wav': fireball_blast(),
        'extinguish.wav': extinguish(),
        'player_hit.wav': player_hit(),
        'player_death.wav': player_death(),
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
        'gun_shot.wav': gun_shot(),
        'gun_reload.wav': gun_reload(),
        'ricochet.wav': ricochet(),
        'thunder.wav': thunder(),
        'crackle.wav': crackle(),
        'steam_burst.wav': steam_burst(),
        'lava_sizzle.wav': lava_sizzle(),
        'lava_hiss.wav': lava_hiss(),
        'lava_chill.wav': lava_chill(),
        'lava_crack.wav': lava_crack(),
        'lava_melt.wav': lava_melt(),
        'lift_start.wav': lift_start(),
        'lift_stop.wav': lift_stop(),
        'frost_blast.wav': frost_blast(),
        'dummy_hit.wav': dummy_hit(),
        'summon.wav': summon(),
        'wisp_hum.wav': wisp_hum(),
        'wisp_flare.wav': wisp_flare(),
        'crawler_hiss.wav': crawler_hiss(),
        'crawler_lunge.wav': crawler_lunge(),
        'crawler_scared.wav': crawler_scared(),
        'staff_gutter.wav': staff_gutter(),
        'beetle_paw.wav': beetle_paw(),
        'beetle_charge.wav': beetle_charge(),
        'bat_screech.wav': bat_screech(),
        'bat_dive.wav': bat_dive(),
        'troll_step.wav': troll_step(),
        'troll_roar.wav': troll_roar(),
        'troll_slam.wav': troll_slam(),
        'troll_throw.wav': troll_throw(),
        'enemy_hit.wav': enemy_hit(),
        'enemy_spotted.wav': enemy_spotted(),
        'orb_pickup.wav': orb_pickup(),
        'splinter.wav': splinter(),
        'black_hole_open.wav': black_hole_open(),
        'black_hole_hum.wav': black_hole_hum(),
        'black_hole_collapse.wav': black_hole_collapse(),
        'iron_thud.wav': iron_thud(),
        'fizzle.wav': fizzle(),
        'thunder_snap.wav': thunder_snap(),
        'ice_crack.wav': ice_crack(),
        'ball_thud.wav': ball_thud(),
        'gun_click.wav': gun_click(),
        'gun_rack.wav': gun_rack(),
        'flame_end.wav': flame_end(),
        'air_jump.wav': air_jump(),
        'no_mana.wav': no_mana(),
        'spell_switch.wav': spell_switch(),
        'not_ready.wav': not_ready(),
        'orb_pop.wav': orb_pop(),
        'small_balls.wav': small_balls(),
        'enemy_death.wav': enemy_death(),
        'dummy_thump.wav': dummy_thump(),
        'crate_ignite.wav': crate_ignite(),
        'fire_crackle.wav': fire_crackle(),
        'crate_collapse.wav': crate_collapse(),
        'fuse_hiss.wav': fuse_hiss(),
        'keg_boom.wav': keg_boom(),
        'rock_throw.wav': rock_throw(),
        'rock_hit.wav': rock_hit(),
        'goblin_cackle.wav': goblin_cackle(),
        'goblin_stab.wav': goblin_stab(),
        'troll_swipe.wav': troll_swipe(),
        'troll_grunt.wav': troll_grunt(),
        'tome_pact.wav': tome_pact(),
        'tome_bounding.wav': tome_bounding(),
        'tome_arcane.wav': tome_arcane(),
        'bound.wav': bound(),
        'beetle_plough.wav': beetle_plough(),
        'crawler_retreat.wav': crawler_retreat(),
        'crawler_hop.wav': crawler_hop(),
        'snowball_throw.wav': snowball_throw(),
        'snowball_burst.wav': snowball_burst(),
        'goblin_crouch.wav': goblin_crouch(),
        'goblin_dash.wav': goblin_dash(),
        'goblin_yelp.wav': goblin_yelp(),
        'ground_strike.wav': ground_strike(),
        'player_tick.wav': player_tick(),
        'crate_break.wav': crate_break(),
        'geyser_sputter.wav': recut(fuse_hiss(), 0.6),
        'geyser_erupt.wav': recut(flame_start(), 0.8),
        'stone_knock.wav': recut(iron_thud(), 0.3),
        **{name: make() for name, make in AMBIENCE.items()},
    }

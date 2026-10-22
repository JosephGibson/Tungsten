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


# 01 (b)'s weapons and elements: the gun, the bolt from the sky, the
# reactions' bursts and the training dummy.


def gun_shot():
    """A sharp report: a cracking noise burst over a short low punch, with a
    metallic tick of the slide."""
    noise, low, high = Noise(0x6005), LowPass(), LowPass()
    samples, phase = [], 0.0
    for i in range(int(0.3 * RATE)):
        t = i / RATE
        n = noise()
        crack = (low(n, 7000) - high(n, 1100)) * math.exp(-t / 0.012)
        body = low(n, 900) * math.exp(-t / 0.05)
        phase += math.tau * glide(160, 60, min(1.0, t / 0.06)) / RATE
        punch = math.sin(phase) * math.exp(-t / 0.035)
        tick = math.sin(math.tau * 3100 * t) * 0.18 * math.exp(-max(0.0, t - 0.02) / 0.02) * (t > 0.02)
        samples.append(math.tanh(2.2 * crack + 1.2 * body + punch) + tick)
    return finish(samples, fade_in=0.0005)


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
    """A strike overhead: a splitting crack, then a low roll that rumbles on
    and fades."""
    duration = 1.8
    noise, rumble, crack_low, low2 = Noise(0x7D0E), Noise(0x0B11), LowPass(), LowPass()
    roll_low = LowPass()
    samples = []
    for i in range(int(duration * RATE)):
        t = i / RATE
        n = noise()
        crack = (n - crack_low(n, 1200)) * math.exp(-t / 0.03)
        # The roll swells after the crack and wobbles as it fades.
        swell = min(1.0, t / 0.12) * math.exp(-t / 0.55)
        wobble = 0.7 + 0.3 * math.sin(math.tau * 3.2 * t) * math.sin(math.tau * 1.3 * t + 1)
        roll = roll_low(low2(rumble(), 260), 120) * 6.0 * swell * wobble
        samples.append(math.tanh(1.8 * crack + roll))
    return finish(samples, fade_in=0.0005, fade_out=0.2)


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


def beetle_paw():
    """The beetle's tell: an iron claw scraping stone, grit and a rasp."""
    duration = 0.42
    noise, grit, low, high = Noise(0xBE71), Noise(0xBE72), LowPass(), LowPass()
    samples, chip = [], 0.0
    for i in range(int(duration * RATE)):
        t = i / RATE
        p = t / duration
        n = noise()
        scrape = low(n, glide(2600, 1400, p)) - high(n, 500)
        rasp = .6 + .4 * math.sin(math.tau * 46 * t) * math.sin(math.tau * 7 * t)
        if grit() > .996:
            chip = .5
        chip *= .9
        envelope = min(1.0, t / 0.03) * (1 - p) ** 0.8
        samples.append((1.6 * scrape * rasp + chip * grit()) * envelope)
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


def orb_pickup():
    """A drop collected: a quick bright arpeggio of glassy bells."""
    samples = [0.0] * int(0.42 * RATE)
    for k, name in enumerate(('A5', 'C#6', 'E6', 'A6')):
        place(samples, int(k * 0.045 * RATE), chime(note(name), 0.3, 0.55 + 0.1 * k, decay=0.1))
    return finish(samples, fade_in=0.0005, fade_out=0.04)


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
    """Water trickling along the channel, fluttering, and drips falling from
    its leaks into still water below."""
    noise, flutter = Noise(0xA0DC), Noise(0xA0DD)
    low, high, wobble = LowPass(AMBIENCE_RATE), LowPass(AMBIENCE_RATE), LowPass(AMBIENCE_RATE)
    drips = [(start, 1700 + 900 * (flutter() * 0.5 + 0.5)) for start in events(0xA1, 11)]

    def voice(i, t):
        n = noise()
        trickle = (low(n, 2600) - high(n, 600)) * (0.45 + 0.55 * abs(wobble(flutter(), 14)) * 8)
        drip = sum(tone_burst(t, start, freq, 0.18, glide_to=freq * 0.6, decay=0.05) for start, freq in drips)
        return 0.5 * trickle + 0.45 * drip
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
        **{name: make() for name, make in AMBIENCE.items()},
    }

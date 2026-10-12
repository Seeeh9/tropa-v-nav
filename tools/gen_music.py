"""Synthesised stand-in music, used until a generated `<name>-flow.ogg` sits next to it.

    python tools/gen_music.py [--force] [--only <name-prefix>]

Every track is a seamless loop built on the game's five-note leitmotif (A - C - D - E - C)
in a minor mode: plucked gusli, a bowed drone, frame drums, a wordless choir, a bell.
Battle tracks come as three layers of equal length (`_l1` drums and bass, `_l2` melody,
`_l3` choir and heavy drums) that the game stacks and fades; the rest are single loops.
22 kHz mono keeps the files small; the generated tracks that replace them need not be.
"""
import os
import sys
import wave
import zlib

import numpy as np

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "music")
SR = 22050
A2 = 110.0
# natural minor and phrygian, in semitones from the root
MINOR = [0, 2, 3, 5, 7, 8, 10]
PHRYGIAN = [0, 1, 3, 5, 7, 8, 10]
# the leitmotif as scale degrees of A minor: A C D E C
MOTIF = [0, 2, 3, 4, 2]


class Loop:
    """A buffer whose end runs into its beginning: whatever rings past the end wraps round."""

    def __init__(self, bpm, bars, seed):
        self.bpm = bpm
        self.beat = 60.0 / bpm
        self.seconds = bars * 4 * self.beat
        self.n = int(round(self.seconds * SR))
        self.buf = np.zeros(self.n)
        self.rng = np.random.default_rng(seed)

    def add(self, beat, sound, gain=1.0):
        start = int(round(beat * self.beat * SR)) % self.n
        idx = (start + np.arange(len(sound))) % self.n
        np.add.at(self.buf, idx, sound * gain)

    def tuned(self, f):
        """The nearest pitch that fits the loop a whole number of times, so a held note has no seam."""
        return round(f * self.seconds) / self.seconds

    def done(self, peak):
        m = np.max(np.abs(self.buf))
        return self.buf / m * peak if m > 0 else self.buf


def hz(root, scale, degree):
    octave, step = divmod(degree, len(scale))
    return root * 2 ** ((scale[step] + 12 * octave) / 12)


def tt(seconds):
    return np.arange(int(SR * seconds)) / SR


def pluck(f, seconds=2.2):
    """A gusli string: bright at the pick, dull as it rings."""
    x = tt(seconds)
    out = np.zeros_like(x)
    for k in range(1, 9):
        if f * k > SR / 2.2:
            break
        out += np.sin(2 * np.pi * f * k * x) * np.exp(-x * (1.6 + 1.3 * k)) / k
    out[: int(SR * 0.004)] *= np.linspace(0, 1, int(SR * 0.004))
    return out


def fiddle(f, seconds):
    """A bowed gudok note: reedy, with a slow shake."""
    x = tt(seconds)
    vib = 1 + 0.006 * np.sin(2 * np.pi * 5.1 * x)
    phase = 2 * np.pi * f * np.cumsum(vib) / SR
    out = sum(np.sin(k * phase) / k ** 1.2 for k in range(1, 8) if f * k < SR / 2.2)
    a = int(SR * 0.08)
    r = int(SR * min(0.25, seconds / 3))
    out[:a] *= np.linspace(0, 1, a)
    out[-r:] *= np.linspace(1, 0, r)
    return out


def drum(low=False):
    """A frame drum: skin and a breath of air."""
    x = tt(0.5 if low else 0.3)
    f0, f1 = (95, 42) if low else (150, 70)
    f = f0 * (f1 / f0) ** (x / x[-1])
    body = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-x * (9 if low else 16))
    slap = np.random.default_rng(1).uniform(-1, 1, len(x)) * np.exp(-x * 60) * 0.35
    return body + slap


def bone():
    """A dry clack of bone or wood."""
    x = tt(0.09)
    return (np.sin(2 * np.pi * 1250 * x) + 0.6 * np.sin(2 * np.pi * 1870 * x)) * np.exp(-x * 70)


def bell(f, seconds=5.0):
    x = tt(seconds)
    partials = [(1.0, 1.0, 0.9), (2.0, 0.6, 1.3), (2.76, 0.45, 1.8), (5.4, 0.25, 2.6), (8.9, 0.12, 3.5)]
    return sum(a * np.sin(2 * np.pi * f * m * x) * np.exp(-x * d) for m, a, d in partials if f * m < SR / 2.2)


def held(loop, f, shimmer=0.0, voices=3, brightness=1.0):
    """A note that lasts the whole loop: the bowed drone (shimmer 0) or a choir vowel."""
    x = np.arange(loop.n) / SR
    out = np.zeros(loop.n)
    for v in range(voices):
        fv = loop.tuned(f * (1 + shimmer * (v - (voices - 1) / 2)))
        ph = loop.rng.uniform(0, 2 * np.pi)
        for k in range(1, 7):
            if fv * k > SR / 2.2:
                break
            # an "ooh": the second and third partials carry the vowel
            w = (1.0, 0.55, 0.4, 0.12, 0.08, 0.04)[k - 1] ** (1 / brightness)
            out += w * np.sin(2 * np.pi * fv * k * x + ph * k)
    return out / voices


def swell(loop, cycles, floor=0.35):
    """A slow breathing of loudness that closes on itself."""
    x = np.arange(loop.n) / loop.n
    return floor + (1 - floor) * 0.5 * (1 - np.cos(2 * np.pi * cycles * x))


def wind(loop, cutoff, cycles=2):
    """Filtered noise with no seam: shaped in the frequency domain."""
    spec = np.fft.rfft(loop.rng.uniform(-1, 1, loop.n))
    freq = np.fft.rfftfreq(loop.n, 1 / SR)
    spec *= 1 / (1 + (freq / cutoff) ** 4)
    return np.fft.irfft(spec, loop.n) * swell(loop, cycles, 0.2)


def melody(loop, root, scale, instrument, bars, density, octave=1, long_notes=False, motif_every=2):
    """The leitmotif on every `motif_every`-th bar, wandering by step in between."""
    degree = 0
    for bar in range(bars):
        if bar % motif_every == 0:
            for i, d in enumerate(MOTIF):
                dur = 0.75 if i < 4 else 1.0
                f = hz(root, scale, d + 7 * octave)
                loop.add(bar * 4 + i * 0.75, instrument(f, dur * loop.beat * 1.6) if long_notes else instrument(f), 1.0)
            degree = MOTIF[-1]
            continue
        beat = 0.0
        while beat < 4:
            step = loop.rng.choice([0.5, 1.0, 1.0, 2.0])
            if loop.rng.random() < density:
                degree = int(np.clip(degree + loop.rng.choice([-2, -1, -1, 1, 1, 2]), -2, 9))
                f = hz(root, scale, degree + 7 * octave)
                loop.add(bar * 4 + beat, instrument(f, step * loop.beat * 1.5) if long_notes else instrument(f), 0.85)
            beat += step


def drums(loop, bars, pattern, heavy=False):
    """`pattern` is sixteen steps a bar: X low drum, x high drum, . rest, o bone."""
    for bar in range(bars):
        for i, ch in enumerate(pattern):
            at = bar * 4 + i * 0.25
            if ch == "X":
                loop.add(at, drum(True), 1.2 if heavy else 1.0)
            elif ch == "x":
                loop.add(at, drum(False), 0.7)
            elif ch == "o":
                loop.add(at, bone(), 0.35)


def bass(loop, root, scale, bars, degrees):
    for bar in range(bars):
        f = hz(root, scale, degrees[bar % len(degrees)] - 7)
        loop.add(bar * 4, fiddle(f, loop.beat * 4.2), 1.0)


def battle(name, bpm, root, scale, pattern, heavy, progression, lead):
    bars = 8
    seed = zlib.crc32(name.encode())
    one, two, three = Loop(bpm, bars, seed), Loop(bpm, bars, seed + 1), Loop(bpm, bars, seed + 2)
    drums(one, bars, pattern)
    bass(one, root, scale, bars, progression)
    one.buf += held(one, root / 2, voices=1) * 0.5 * swell(one, 2)
    melody(two, root, scale, lead, bars, 0.8, long_notes=lead is fiddle)
    three.buf += held(three, hz(root, scale, 0) * 2, 0.004) * swell(three, 4, 0.2)
    three.buf += held(three, hz(root, scale, 4) * 2, 0.004) * swell(three, 2, 0.1) * 0.7
    drums(three, bars, heavy, heavy=True)
    return {f"{name}_l1": one.done(0.42), f"{name}_l2": two.done(0.34), f"{name}_l3": three.done(0.34)}


def ambient(name, bpm, bars, root, scale, parts):
    loop = Loop(bpm, bars, zlib.crc32(name.encode()))
    if "drone" in parts:
        loop.buf += held(loop, root / 2, voices=1) * 0.5 * swell(loop, 2)
        loop.buf += held(loop, root * 0.75, voices=1) * 0.2 * swell(loop, 3, 0.1)
    if "choir" in parts:
        loop.buf += held(loop, root * 2, 0.004) * 0.5 * swell(loop, 2, 0.15)
        loop.buf += held(loop, hz(root, scale, 2) * 2, 0.004) * 0.35 * swell(loop, 1, 0.1)
    if "wind" in parts:
        w = wind(loop, 500)
        loop.buf += w / np.max(np.abs(w)) * 0.5
    if "fire" in parts:
        # a low breath of flame and the odd crack of a log
        w = wind(loop, 180, 5)
        loop.buf += w / np.max(np.abs(w)) * 0.45
        for _ in range(int(loop.seconds * 1.5)):
            loop.add(loop.rng.uniform(0, bars * 4), bone() * loop.rng.uniform(0.2, 1.0), 0.25)
    if "pluck" in parts:
        sub = Loop(bpm, bars, zlib.crc32(name.encode()) + 5)
        melody(sub, root, scale, pluck, bars, parts["pluck"], motif_every=parts.get("motif", 2))
        loop.buf += sub.done(1.0) * 0.9
    if "fiddle" in parts:
        sub = Loop(bpm, bars, zlib.crc32(name.encode()) + 6)
        melody(sub, root, scale, fiddle, bars, parts["fiddle"], octave=1, long_notes=True, motif_every=4)
        loop.buf += sub.done(1.0) * 0.5
    if "bell" in parts:
        for bar in range(0, bars, parts["bell"]):
            loop.add(bar * 4, bell(root * 2), 0.5)
    if "drum" in parts:
        drums(loop, bars, parts["drum"])
    return {name: loop.done(0.5)}


def tracks():
    A, D, E, G = A2 * 2, A2 * 2 ** (5 / 12), A2 * 2 ** (7 / 12), A2 * 2 ** (10 / 12)
    out = {}
    # ---- outside battle ----
    out.update(ambient("menu", 66, 8, A, MINOR, {"drone": 1, "choir": 1, "pluck": 0.35, "motif": 2, "bell": 4}))
    out.update(ambient("village", 80, 8, A, MINOR, {"drone": 1, "pluck": 0.7, "motif": 4, "fire": 1}))
    out.update(ambient("map", 72, 8, A, MINOR, {"drone": 1, "wind": 1, "fiddle": 0.45, "drum": "X.......x......."}))
    out.update(ambient("rest", 60, 8, D, MINOR, {"fire": 1, "pluck": 0.4, "motif": 4}))
    out.update(ambient("shop", 92, 8, G, MINOR, {"pluck": 0.9, "motif": 4, "drum": "o...o.o...o.o..."}))
    out.update(ambient("omen", 56, 8, E, PHRYGIAN, {"drone": 1, "wind": 1, "choir": 1, "bell": 2}))
    out.update(ambient("end", 58, 8, A, MINOR, {"choir": 1, "pluck": 0.3, "motif": 2, "bell": 4}))
    # ---- battle: drums+bass / melody / choir+heavy drums ----
    out.update(battle("battle_act1", 100, A, MINOR, "X..x..x.X..xo.x.", "X.......X...X...", [0, 0, 5, 4], fiddle))
    out.update(battle("battle_act2", 96, D, PHRYGIAN, "X...o.x.X.o...x.", "X.....X.....X...", [0, 1, 0, 4], fiddle))
    out.update(battle("battle_act3", 108, E, PHRYGIAN, "X.x.X.x.X.x.Xox.", "X...X...X...X.X.", [0, 5, 3, 4], pluck))
    out.update(battle("elite", 112, A, PHRYGIAN, "X.xxX.x.X.xxXox.", "X..X..X.X..X..X.", [0, 0, 1, 0], fiddle))
    out.update(battle("boss_act1", 104, A, MINOR, "X.o.x.o.X.o.xox.", "X...X..XX...X..X", [0, 5, 3, 4], fiddle))
    out.update(battle("boss_act2", 100, D, PHRYGIAN, "X.....x.X..o..x.", "X.......X..X....", [0, 1, 3, 1], fiddle))
    out.update(battle("boss_act3", 110, E, MINOR, "X.x.X.xoX.x.Xxx.", "X..X..X.X..X.XX.", [0, 3, 5, 4], pluck))
    out.update(battle("koschei", 84, A, PHRYGIAN, "X...x...X.o.x...", "X.......X.....X.", [0, 1, 0, 6], fiddle))
    return out


def main():
    force = "--force" in sys.argv
    only = sys.argv[sys.argv.index("--only") + 1] if "--only" in sys.argv else ""
    os.makedirs(ROOT, exist_ok=True)
    for name, x in tracks().items():
        path = os.path.join(ROOT, f"{name}.wav")
        if not name.startswith(only) or (os.path.exists(path) and not force):
            continue
        with wave.open(path, "wb") as w:
            w.setnchannels(1)
            w.setsampwidth(2)
            w.setframerate(SR)
            w.writeframes((np.clip(x, -1, 1) * 32767).astype(np.int16).tobytes())
        print("  wrote", f"music/{name}.wav", f"{len(x) / SR:.1f}s")


if __name__ == "__main__":
    main()

"""Synthesised stand-in sound effects, used until a generated `<name>-flow.ogg` sits next to them.

    python tools/gen_sfx.py [--force]

Dry and short, as the sound brief asks: no music, no reverb tail, 48 kHz mono.
"""
import os
import sys
import wave

import numpy as np

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "sfx")
SR = 48000


def t(seconds):
    return np.linspace(0, seconds, int(SR * seconds), endpoint=False)


def env(x, attack=0.004, release=0.5):
    """Fast attack, exponential fall."""
    n = len(x)
    a = max(1, int(SR * attack))
    e = np.exp(-np.linspace(0, 1, n) * (6.0 / release) * (n / SR))
    e[:a] *= np.linspace(0, 1, a)
    return x * e


def noise(seconds, seed):
    return np.random.default_rng(seed).uniform(-1, 1, int(SR * seconds))


def lowpass(x, k):
    """One-pole smoothing; k in 0..1, smaller is duller."""
    y = np.empty_like(x)
    acc = 0.0
    for i, v in enumerate(x):
        acc += k * (v - acc)
        y[i] = acc
    return y


def sweep(seconds, f0, f1):
    tt = t(seconds)
    f = f0 * (f1 / f0) ** (tt / seconds)
    return np.sin(2 * np.pi * np.cumsum(f) / SR)


def tone(seconds, f):
    return np.sin(2 * np.pi * f * t(seconds))


def mix(*parts):
    n = max(len(p) for p in parts)
    out = np.zeros(n)
    for p in parts:
        out[: len(p)] += p
    return out


SOUNDS = {
    # paper and birch bark
    "card_play": lambda: env(lowpass(noise(0.16, 1), 0.5), release=0.10) * 0.7,
    "card_draw": lambda: env(lowpass(noise(0.10, 2), 0.7), release=0.06) * 0.45,
    # a blow: a low thump under a crack
    "hit": lambda: mix(env(sweep(0.20, 190, 55), release=0.14), env(lowpass(noise(0.08, 3), 0.9), release=0.04) * 0.6) * 0.8,
    "hit_heavy": lambda: mix(env(sweep(0.36, 150, 38), release=0.26), env(lowpass(noise(0.14, 4), 0.6), release=0.08) * 0.8) * 0.95,
    # wood taking the blow
    "block": lambda: mix(env(tone(0.14, 310), release=0.07), env(tone(0.14, 470), release=0.05) * 0.5, env(lowpass(noise(0.05, 5), 0.8), release=0.03) * 0.4) * 0.6,
    "heal": lambda: mix(env(tone(0.34, 523), release=0.3), env(tone(0.34, 784), attack=0.06, release=0.3) * 0.6) * 0.4,
    "status": lambda: env(lowpass(noise(0.26, 6), 0.08) * 3.0, attack=0.03, release=0.22) * 0.5,
    "death": lambda: mix(env(sweep(0.6, 220, 40), release=0.5), env(lowpass(noise(0.6, 7), 0.15) * 2.0, release=0.5) * 0.6) * 0.7,
    "downed": lambda: mix(env(sweep(0.7, 160, 45), release=0.6), env(tone(0.7, 98), release=0.6) * 0.5) * 0.8,
    "raised": lambda: mix(env(sweep(0.5, 220, 440), attack=0.05, release=0.45), env(tone(0.5, 660), attack=0.15, release=0.4) * 0.4) * 0.45,
    "click": lambda: env(tone(0.05, 880), release=0.025) * 0.35,
    # the wax seal
    "ready": lambda: mix(env(sweep(0.18, 240, 90), release=0.12), env(lowpass(noise(0.06, 8), 0.6), release=0.04) * 0.4) * 0.6,
    "round": lambda: env(tone(0.5, 147), attack=0.01, release=0.45) * 0.5,
    "rune": lambda: mix(env(tone(0.3, 1175), release=0.25), env(tone(0.3, 1568), release=0.2) * 0.5) * 0.3,
    "potion": lambda: env(sweep(0.22, 420, 760) * (0.6 + 0.4 * np.sin(2 * np.pi * 28 * t(0.22))), release=0.18) * 0.4,
    "gold": lambda: mix(env(tone(0.12, 1760), release=0.08), env(np.concatenate([np.zeros(int(SR * 0.05)), tone(0.12, 2217)]), release=0.1) * 0.7) * 0.3,
    "victory": lambda: mix(*[env(np.concatenate([np.zeros(int(SR * 0.16 * i)), tone(0.7, f)]), release=0.6) for i, f in enumerate((196, 247, 294, 392))]) * 0.3,
    "defeat": lambda: mix(env(tone(1.2, 110), release=1.1), env(tone(1.2, 116.5), release=1.1) * 0.8) * 0.5,
}


def main():
    force = "--force" in sys.argv
    os.makedirs(ROOT, exist_ok=True)
    for name, make in SOUNDS.items():
        path = os.path.join(ROOT, f"{name}.wav")
        if os.path.exists(path) and not force:
            continue
        x = np.clip(make(), -1, 1)
        with wave.open(path, "wb") as w:
            w.setnchannels(1)
            w.setsampwidth(2)
            w.setframerate(SR)
            w.writeframes((x * 32767).astype(np.int16).tobytes())
        print("  wrote", f"sfx/{name}.wav", f"{len(x) / SR:.2f}s")


if __name__ == "__main__":
    main()

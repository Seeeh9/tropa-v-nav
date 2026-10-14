"""Takes voice clips downloaded from Google AI Studio into the game and checks them by ear.

    python tools/voice_in.py <ru-old|ru|en> <text key> [...]   # -> assets/voice/<lang>/<key>.wav

AI Studio saves every clip as `Generated Audio <date>.wav`; the waiting ones are taken in the
order they arrived, one per key. Each clip is then heard by a local Whisper (tools/hear.py) and
scored against the line it should say, taken from assets/i18n/<lang>/*.ftl: a clip that was
saved before the model finished, or that dropped words, shows as a low score and is left in
Downloads under `REJECTED <key>.wav` instead of going into the game. A good clip is levelled on the
way in (see `level`), because the model's voice sinks towards the end of a page; loudness by fifths
of the clip is printed before and after.
"""
import glob
import os
import re
import shutil
import sys
import wave

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from hear import hear, score  # noqa: E402

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
DOWNLOADS = os.path.join(os.path.expanduser("~"), "Downloads")
# the old speech is Russian to Whisper, which then mishears its archaic endings: ask less of it
ENOUGH = {"ru-old": 0.35, "ru": 0.8, "en": 0.8}
# characters a second the narrator cannot honestly exceed; a faster clip has lost its end
FASTEST = {"ru-old": 10, "ru": 13, "en": 17}


def lines(lang):
    out = {}
    # the old speech overrides Russian, as it does in the game
    for folder in (["ru"] if lang == "ru-old" else []) + [lang]:
        for f in glob.glob(os.path.join(ROOT, "assets", "i18n", folder, "*.ftl")):
            for line in open(f, encoding="utf-8"):
                m = re.match(r"([a-z0-9_-]+) = (.*)", line)
                if m:
                    out[m.group(1)] = m.group(2).strip()
    return out


# loudness every clip is brought to (RMS of its spoken parts, 16-bit scale)
LOUD = 3600.0


def level(frames, rate):
    """Evens a clip out: the same loudness from its first word to its last, and from clip to clip.

    The speech model lets its voice sink towards the end of a page. The clip is measured second by
    second (only where someone is speaking), each stretch is brought to the common loudness, and the
    gain glides between stretches so nothing pumps or clicks.
    """
    frames = frames.astype(np.float32)
    mono = np.abs(frames).mean(axis=1) if frames.ndim > 1 else np.abs(frames)
    hop = rate // 50
    n = max(1, len(mono) // hop)
    env = np.array([np.sqrt((mono[i * hop:(i + 1) * hop] ** 2).mean()) for i in range(n)])
    speaking = env > max(env.max() * 0.08, 60.0)
    if not speaking.any():
        return frames
    # loudness around every moment: spoken frames within a second and a half either side
    half = 75
    gains = np.ones(n, dtype=np.float32)
    for i in range(n):
        a, b = max(0, i - half), min(n, i + half + 1)
        near = env[a:b][speaking[a:b]]
        if len(near) >= 10:
            gains[i] = LOUD / max(float(np.sqrt((near ** 2).mean())), 1.0)
        else:
            gains[i] = np.nan
    # silences take the gain of the speech beside them
    known = np.flatnonzero(~np.isnan(gains))
    gains = np.interp(np.arange(n), known, gains[known]).clip(0.4, 6.0)
    # glide: half a second of smoothing
    kernel = np.hanning(51)
    gains = np.convolve(np.pad(gains, 25, mode="edge"), kernel / kernel.sum(), mode="valid")
    per_sample = np.interp(np.arange(len(mono)), (np.arange(n) + 0.5) * hop, gains)
    out = frames * (per_sample[:, None] if frames.ndim > 1 else per_sample)
    peak = float(np.abs(out).max())
    if peak > 31000.0:
        out *= 31000.0 / peak
    return out


def save_levelled(src, dst):
    with wave.open(src) as w:
        rate, channels, width = w.getframerate(), w.getnchannels(), w.getsampwidth()
        raw = np.frombuffer(w.readframes(w.getnframes()), dtype=np.int16).reshape(-1, channels)
    with wave.open(dst, "wb") as w:
        w.setnchannels(channels)
        w.setsampwidth(width)
        w.setframerate(rate)
        w.writeframes(level(raw, rate).astype(np.int16).tobytes())


def loudness(path):
    with wave.open(path) as w:
        a = np.frombuffer(w.readframes(w.getnframes()), dtype=np.int16).astype(float)
    n = max(1, len(a) // 5)
    return [int(np.sqrt((a[i * n:(i + 1) * n] ** 2).mean())) for i in range(5)]


def main():
    lang, keys = sys.argv[1], sys.argv[2:]
    files = sorted(glob.glob(os.path.join(DOWNLOADS, "Generated Audio*.wav")), key=os.path.getmtime)
    if len(files) != len(keys):
        raise SystemExit(f"{len(files)} clips waiting but {len(keys)} keys given; nothing moved")
    text = lines(lang)
    out_dir = os.path.join(ROOT, "assets", "voice", lang)
    os.makedirs(out_dir, exist_ok=True)
    for f, key in zip(files, keys):
        heard, seconds = hear(f, lang.split("-")[0])
        s = score(text[key], heard)
        ok = s >= ENOUGH.get(lang, 0.85)
        # a clip cut short is also much shorter than its text takes to say
        ok = ok and seconds * FASTEST.get(lang, 17) >= len(text[key])
        if ok:
            out = os.path.join(out_dir, f"{key}.wav")
            save_levelled(f, out)
            print(f"ok  {key}: {seconds:.1f}s heard {s:.0%} loud {loudness(f)} -> {loudness(out)}")
            os.makedirs(os.path.join(ROOT, "_inbox", "voice"), exist_ok=True)
            shutil.move(f, os.path.join(ROOT, "_inbox", "voice", f"{lang}_{key}_take.wav"))
        else:
            print(f"BAD {key}: {seconds:.1f}s heard {s:.0%} loud {loudness(f)}")
            print("    heard:", heard)
            shutil.move(f, os.path.join(DOWNLOADS, f"REJECTED {key}.wav"))


if __name__ == "__main__":
    main()

"""Takes voice clips downloaded from Google AI Studio into the game and checks them by ear.

    python tools/voice_in.py <ru-old|ru|en> <text key> [...]   # -> assets/voice/<lang>/<key>.wav

AI Studio saves every clip as `Generated Audio <date>.wav`; the waiting ones are taken in the
order they arrived, one per key. Each clip is then heard by a local Whisper (tools/hear.py) and
scored against the line it should say, taken from assets/i18n/<lang>/*.ftl: a clip that was
saved before the model finished, or that dropped words, shows as a low score and is left in
Downloads under `REJECTED <key>.wav` instead of going into the game. Loudness is printed by
fifths of the clip, so a voice that fades to a whisper can be seen too.
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
ENOUGH = {"ru-old": 0.55}


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
        print(f"{'ok ' if ok else 'BAD'} {key}: {seconds:.1f}s heard {s:.0%} loud {loudness(f)}")
        if ok:
            shutil.move(f, os.path.join(out_dir, f"{key}.wav"))
        else:
            print("    heard:", heard)
            shutil.move(f, os.path.join(DOWNLOADS, f"REJECTED {key}.wav"))


if __name__ == "__main__":
    main()

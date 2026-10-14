"""Joins a page that had to be read in parts into one clip, then takes it in like voice_in.py.

    python tools/voice_join.py <ru-old|ru|en> <text key> <parts>

Some pages the speech model will not read to the end in one run; they are read in `parts` runs, cut
at sentence ends (the page helper `voicePart` in the browser does the same cut). The newest `parts`
clips in Downloads are joined in the order they arrived, with a short pause between them, and the
whole is scored and levelled against the full text of the page.
"""
import glob
import os
import shutil
import sys
import wave

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from hear import hear, score  # noqa: E402
from voice_in import DOWNLOADS, ENOUGH, ROOT, lines, loudness, save_levelled  # noqa: E402

PAUSE = 0.6


def main():
    lang, key, parts = sys.argv[1], sys.argv[2], int(sys.argv[3])
    files = sorted(glob.glob(os.path.join(DOWNLOADS, "Generated Audio*.wav")), key=os.path.getmtime)
    if len(files) != parts:
        raise SystemExit(f"{len(files)} clips waiting but {parts} parts expected; nothing moved")
    chunks, rate, channels, width = [], None, None, None
    for f in files:
        with wave.open(f) as w:
            rate, channels, width = w.getframerate(), w.getnchannels(), w.getsampwidth()
            chunks.append(np.frombuffer(w.readframes(w.getnframes()), dtype=np.int16).reshape(-1, channels))
    gap = np.zeros((int(PAUSE * rate), channels), dtype=np.int16)
    whole = np.concatenate([c for chunk in chunks for c in (chunk, gap)][:-1])
    joined = os.path.join(DOWNLOADS, f"joined {key}.wav")
    with wave.open(joined, "wb") as w:
        w.setnchannels(channels)
        w.setsampwidth(width)
        w.setframerate(rate)
        w.writeframes(whole.tobytes())
    heard, seconds = hear(joined, lang.split("-")[0])
    s = score(lines(lang)[key], heard)
    if s < ENOUGH.get(lang, 0.85):
        print(f"BAD {key}: {seconds:.1f}s heard {s:.0%}\n    heard: {heard}")
        # the parts go too, so the page can simply be read again
        for f in files + [joined]:
            os.remove(f)
        return
    os.makedirs(os.path.join(ROOT, "assets", "voice", lang), exist_ok=True)
    out = os.path.join(ROOT, "assets", "voice", lang, f"{key}.wav")
    save_levelled(joined, out)
    print(f"ok  {key}: {seconds:.1f}s heard {s:.0%} from {parts} parts, loud {loudness(out)}")
    os.makedirs(os.path.join(ROOT, "_inbox", "voice"), exist_ok=True)
    for k, f in enumerate(files):
        shutil.move(f, os.path.join(ROOT, "_inbox", "voice", f"{lang}_{key}_part{k + 1}.wav"))
    os.remove(joined)


if __name__ == "__main__":
    main()

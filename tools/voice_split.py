"""Cuts one narrated scene from Google AI Studio into its pages and takes them into the game.

    python tools/voice_split.py <ru-old|ru|en> <text key> [...]   # -> assets/voice/<lang>/<key>.wav

A scene is read in one run, one speech block per page, so the voice keeps its tone from page to
page. The newest `Generated Audio*.wav` in Downloads is that run. A local Whisper hears it with
word timings, the heard words are lined up against the pages' texts from assets/i18n, and the clip
is cut in the quietest spot of the pause between one page's last word and the next page's first.
Each piece is then scored like tools/voice_in.py does; nothing is written unless every page passes
(the run stays in Downloads to be looked at).
"""
import difflib
import glob
import os
import shutil
import sys
import wave

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import hear as ear  # noqa: E402
from voice_in import ENOUGH, lines  # noqa: E402

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
DOWNLOADS = os.path.join(os.path.expanduser("~"), "Downloads")
# silence kept before the first word and after the last one of a page
PAD = 0.25


def timed_words(pcm16k, lang):
    if ear.MODEL is None:
        ear.MODEL = ear.WhisperModel("small", device="cpu", compute_type="int8")
    segments, _ = ear.MODEL.transcribe(pcm16k, language=lang, beam_size=5, word_timestamps=True)
    out = []
    for s in segments:
        for w in s.words:
            for piece in ear.words(w.word):
                out.append((piece, w.start, w.end))
    return out


def quietest(pcm, rate, lo, hi):
    """Middle of the quietest 0.2 s between two moments."""
    lo, hi = max(0.0, lo), min(len(pcm) / rate, hi)
    win = int(0.2 * rate)
    a, b = int(lo * rate), int(hi * rate)
    if b - a <= win:
        return (lo + hi) / 2
    step = int(0.02 * rate)
    best = min(range(a, b - win, step), key=lambda i: float(np.abs(pcm[i:i + win]).mean()))
    return (best + win / 2) / rate


def main():
    lang, keys = sys.argv[1], sys.argv[2:]
    files = sorted(glob.glob(os.path.join(DOWNLOADS, "Generated Audio*.wav")), key=os.path.getmtime)
    if not files:
        raise SystemExit("no Generated Audio*.wav in Downloads")
    path = files[-1]
    with wave.open(path) as w:
        rate, channels, width = w.getframerate(), w.getnchannels(), w.getsampwidth()
        raw = np.frombuffer(w.readframes(w.getnframes()), dtype=np.int16)
    mono = raw.reshape(-1, channels).mean(axis=1) if channels > 1 else raw.astype(np.float32)
    mono = mono.astype(np.float32) / 32768.0
    at = np.arange(0, len(mono), rate / 16000.0)
    heard = timed_words(np.interp(at, np.arange(len(mono)), mono).astype(np.float32), lang.split("-")[0])

    text = lines(lang)
    want, owner = [], []
    for k, key in enumerate(keys):
        for word in ear.words(text[key]):
            want.append(word)
            owner.append(k)
    blocks = difflib.SequenceMatcher(None, want, [h[0] for h in heard], autojunk=False).get_matching_blocks()
    pairs = [(b.a + i, b.b + i) for b in blocks for i in range(b.size)]

    total = len(mono) / rate
    cuts = [0.0]
    for k in range(1, len(keys)):
        before = [h for a, h in pairs if owner[a] < k]
        after = [h for a, h in pairs if owner[a] >= k]
        if not before or not after:
            raise SystemExit(f"cannot find where {keys[k]} begins: too little of it was heard")
        cuts.append(quietest(mono, rate, heard[before[-1]][2], heard[after[0]][1]))
    cuts.append(total)

    pieces, ok = [], True
    for k, key in enumerate(keys):
        a, b = cuts[k], cuts[k + 1]
        mine = [heard[h] for h in range(len(heard)) if a <= (heard[h][1] + heard[h][2]) / 2 < b]
        said = " ".join(m[0] for m in mine)
        s = ear.score(text[key], said)
        good = s >= ENOUGH.get(lang, 0.85)
        ok &= good
        # trim to the speech itself, keeping a short breath on both sides
        start = max(a, (mine[0][1] if mine else a) - PAD)
        end = min(b, (mine[-1][2] if mine else b) + PAD)
        print(f"{'ok ' if good else 'BAD'} {key}: {end - start:.1f}s heard {s:.0%}")
        if not good:
            print("    heard:", said)
        pieces.append((key, start, end))
    if not ok:
        raise SystemExit("not every page passed; nothing written, the run stays in Downloads")
    out_dir = os.path.join(ROOT, "assets", "voice", lang)
    os.makedirs(out_dir, exist_ok=True)
    for key, start, end in pieces:
        frames = raw.reshape(-1, channels)[int(start * rate):int(end * rate)]
        with wave.open(os.path.join(out_dir, f"{key}.wav"), "wb") as w:
            w.setnchannels(channels)
            w.setsampwidth(width)
            w.setframerate(rate)
            w.writeframes(frames.astype(np.int16).tobytes())
    os.makedirs(os.path.join(ROOT, "_inbox", "voice"), exist_ok=True)
    shutil.move(path, os.path.join(ROOT, "_inbox", "voice", f"{lang}_{keys[0]}_run.wav"))


if __name__ == "__main__":
    main()

"""Listens to voice clips with a small local Whisper and says what was actually spoken.

    python tools/hear.py <lang> <file.wav> [...]            # print what each clip says
    python tools/hear.py <lang> --want "<text>" <file.wav>  # also score it against the text it should say

The score is the share of the wanted words that were heard, in order. A text-to-speech model
that drops the end of a long line shows up here as a low score, without anyone listening.
Needs `pip install faster-whisper`; the model is fetched on first use.
"""
import difflib
import re
import sys
import wave

import numpy as np
from faster_whisper import WhisperModel

MODEL = None


def words(text):
    return re.findall(r"[a-zа-яё0-9]+", text.lower().replace("ё", "е"))


def hear(path, lang):
    global MODEL
    if MODEL is None:
        MODEL = WhisperModel("small", device="cpu", compute_type="int8")
    # decoded here rather than by the library: plain PCM wav, resampled to the 16 kHz Whisper wants
    with wave.open(path) as w:
        rate, channels, n = w.getframerate(), w.getnchannels(), w.getnframes()
        pcm = np.frombuffer(w.readframes(n), dtype=np.int16).astype(np.float32) / 32768.0
    if channels > 1:
        pcm = pcm.reshape(-1, channels).mean(axis=1)
    seconds = len(pcm) / rate
    if rate != 16000:
        at = np.arange(0, len(pcm), rate / 16000.0)
        pcm = np.interp(at, np.arange(len(pcm)), pcm).astype(np.float32)
    segments, _ = MODEL.transcribe(pcm, language=lang, beam_size=5)
    return " ".join(s.text.strip() for s in segments), seconds


def score(want, heard):
    a, b = words(want), words(heard)
    if not a:
        return 1.0
    same = sum(m.size for m in difflib.SequenceMatcher(None, a, b).get_matching_blocks())
    return same / len(a)


if __name__ == "__main__":
    lang, args = sys.argv[1], sys.argv[2:]
    want = None
    if args[:1] == ["--want"]:
        want, args = args[1], args[2:]
    for f in args:
        heard, seconds = hear(f, lang)
        line = f"{f}: {seconds:.1f}s"
        if want is not None:
            line += f"  heard {score(want, heard):.0%}"
        print(line)
        print("   ", heard)

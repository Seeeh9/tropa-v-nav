"""Cuts the burst of noise some speech models leave at the very end of a clip.

    python tools/trim_tail.py <file.wav|folder> [...]   # rewrites the clips that have it

The 3.8 speech models end a clip with about a tenth of a second of loud hiss straight after the
silence that follows the last word. A clip has it when its last stretch is many times louder than
the quiet before it; everything from the start of the burst on is cut away.
"""
import glob
import os
import sys
import wave

import numpy as np

HOP = 0.01  # seconds per loudness step


def burst_start(a, rate):
    """Sample where the closing burst begins, or None when the clip ends cleanly."""
    hop = int(rate * HOP)
    n = len(a) // hop
    if n < 30:
        return None
    env = np.sqrt((a[: n * hop].reshape(n, hop).astype(float) ** 2).mean(axis=1))
    tail = env[-5:]
    if tail.min() < 4000:
        return None
    # walk back over the loud end to where it began
    i = n - 1
    while i > 0 and env[i - 1] > 2000:
        i -= 1
    before = env[max(0, i - 20):i]
    # the burst follows quiet, and is short; a shout at the end is neither
    if len(before) == 0 or np.median(before) > 600 or (n - i) * HOP > 0.4:
        return None
    return i * hop


def trim(path):
    with wave.open(path) as w:
        params = w.getparams()
        a = np.frombuffer(w.readframes(w.getnframes()), dtype=np.int16)
    if params.nchannels != 1:
        return False
    cut = burst_start(a, params.framerate)
    if cut is None:
        return False
    with wave.open(path, "wb") as w:
        w.setparams(params)
        w.writeframes(a[:cut].tobytes())
    print(f"trimmed {os.path.basename(path)}: {(len(a) - cut) / params.framerate:.2f}s")
    return True


def main():
    files = []
    for arg in sys.argv[1:]:
        files += glob.glob(os.path.join(arg, "**", "*.wav"), recursive=True) if os.path.isdir(arg) else [arg]
    print(sum(trim(f) for f in files), "of", len(files), "trimmed")


if __name__ == "__main__":
    main()

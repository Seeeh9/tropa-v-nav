"""Takes a track downloaded from Flow Music into the game.

    python tools/music_in.py "<file in Downloads>" <track name>

Writes assets/music/<track name>-flow.ogg (Vorbis), which the game prefers over the
synthesised stand-in, and moves the download into _inbox/ so it is not picked up twice.
"""
import os
import shutil
import sys

import numpy as np
import soundfile as sf

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
src = sys.argv[1]
if not os.path.isabs(src):
    src = os.path.join(os.path.expanduser("~"), "Downloads", src)
name = sys.argv[2]
data, rate = sf.read(src, always_2d=True)
peak = float(np.max(np.abs(data)))
rms = float(np.sqrt(np.mean(data ** 2)))
# leave a little headroom: the mixer stacks sound effects on top
if peak > 0:
    data = data / peak * 0.89
out = os.path.join(ROOT, "assets", "music", f"{name}-flow.ogg")
# written a second at a time: the Vorbis encoder overflows its stack on one huge block
with sf.SoundFile(out, "w", samplerate=rate, channels=data.shape[1], format="OGG", subtype="VORBIS") as f:
    for i in range(0, len(data), rate):
        f.write(data[i:i + rate])
os.makedirs(os.path.join(ROOT, "_inbox"), exist_ok=True)
shutil.move(src, os.path.join(ROOT, "_inbox", f"{name}.wav"))
print(f"{name}: {len(data) / rate:.1f}s, {data.shape[1]} ch, {rate} Hz, peak {peak:.2f}, rms {rms:.3f} -> {os.path.getsize(out) // 1024} KB")

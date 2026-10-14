"""Turns emblems downloaded from Google Flow into enemy intent icons.

    python tools/intentart.py <intent kind> [...]   # in the order the pictures arrived
    python tools/intentart.py --again <dir>         # redo from kept originals <dir>/<kind>_*.jpg

Each picture is painted on flat black; the black is keyed out to transparency by brightness,
so the emblem floats over any backdrop. The result is 128x128: assets/intents/<kind>-gemini.png.
Kinds are the `IntentKind` names in crates/core (Attack, AttackAll, Defend, ...); `-` throws a
picture away. Originals are kept as `_inbox/intents/<kind>_f1.jpg`.
"""
import os
import shutil
import sys

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from flowart import ROOT, waiting  # noqa: E402

# below LOW counts as background, above HIGH as fully opaque
LOW, HIGH = 14, 60


def key_out(im):
    rgb = np.asarray(im.convert("RGB")).astype(np.float32)
    bright = rgb.max(axis=2)
    alpha = np.clip((bright - LOW) / (HIGH - LOW), 0, 1)
    # un-premultiply the dark fringe so edges do not turn grey over light backdrops
    lift = np.where(alpha > 0, 1 / np.maximum(alpha, 1e-3), 0)[..., None]
    rgb = np.clip(rgb * np.minimum(lift, 4), 0, 255)
    out = np.dstack([rgb, alpha * 255]).astype(np.uint8)
    return Image.fromarray(out, "RGBA")


def make(f, name):
    im = Image.open(f)
    w, h = im.size
    side = min(w, h)
    im = key_out(im.crop(((w - side) // 2, (h - side) // 2, (w - side) // 2 + side, (h - side) // 2 + side)))
    # the emblem fills the icon: cut the empty black margin, keep it square and centred
    l, t, r, b = im.getchannel("A").point(lambda a: 255 if a > 40 else 0).getbbox()
    half = max(r - l, b - t) // 2 + side // 40
    cx, cy = (l + r) // 2, (t + b) // 2
    im = im.crop((cx - half, cy - half, cx + half, cy + half))
    im.resize((128, 128), Image.LANCZOS).save(os.path.join(ROOT, "assets", "intents", f"{name}-gemini.png"))
    print("intent", name, (w, h))


def main():
    os.makedirs(os.path.join(ROOT, "assets", "intents"), exist_ok=True)
    if sys.argv[1:2] == ["--again"]:
        import glob
        for f in sorted(glob.glob(os.path.join(sys.argv[2], "*_f*.jpg"))):
            make(f, os.path.basename(f).rsplit("_f", 1)[0])
        return
    names = sys.argv[1:]
    files = waiting()
    if len(files) != len(names):
        raise SystemExit(f"{len(files)} pictures waiting but {len(names)} names given; nothing moved")
    inbox = os.path.join(ROOT, "_inbox", "intents")
    os.makedirs(inbox, exist_ok=True)
    for f, name in zip(files, names):
        if name == "-":
            os.remove(f)
            continue
        make(f, name)
        shutil.move(f, os.path.join(inbox, f"{name}_f1.jpg"))


if __name__ == "__main__":
    main()

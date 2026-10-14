"""Takes the book's textures from Downloads (Flow, 2K) into assets/book/.

    python tools/bookart.py

Pictures are told apart by the words Flow puts in their names. The cover is cut to the book
itself (Flow paints a little white around it); the candle flame is keyed out of its black;
the tileable ones are made truly seamless (a half-offset copy is blended over the seams, as
Flow's "seamless" seldom quite is). Originals move to _inbox/book/.
"""
import glob
import os
import shutil
import sys

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from intentart import key_out  # noqa: E402

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
DOWNLOADS = os.path.join(os.path.expanduser("~"), "Downloads")

# words in Flow's file name -> (texture, kind, side in pixels)
TEXTURES = [
    ("book_cover", "cover", 2048, "cover"),
    ("leather", "leather", 1024, "tile"),
    ("parchment", "parchment", 1024, "tile"),
    ("picture_f", "frame", 1536, "plain"),
    ("headpiece", "headpiece", 1024, "plain"),
    ("pages_texture", "edges", 1024, "tile"),
    ("table", "table", 1024, "tile"),
    ("rag_paper", "fibres", 1024, "tile"),
    ("endpaper", "endpaper", 1024, "tile"),
    ("flame", "flame", 256, "flame"),
]


def seamless(im):
    """Blends a half-offset copy over the borders, so the picture tiles without a seam."""
    a = np.asarray(im.convert("RGB")).astype(np.float32)
    h, w = a.shape[:2]
    shifted = np.roll(np.roll(a, h // 2, axis=0), w // 2, axis=1)
    # weight of the original: 1 in the middle, 0 at the edges, smoothly
    y = np.abs(np.linspace(-1, 1, h))[:, None]
    x = np.abs(np.linspace(-1, 1, w))[None, :]
    edge = np.clip((np.maximum(x, y) - 0.6) / 0.4, 0, 1)
    weight = (1 - edge * edge * (3 - 2 * edge))[..., None]
    return Image.fromarray((a * weight + shifted * (1 - weight)).astype(np.uint8))


def crop_to_book(im):
    """The cover without the white Flow leaves around it."""
    a = np.asarray(im.convert("RGB")).astype(np.int32)
    dark = a.sum(axis=2) < 600
    rows, cols = np.flatnonzero(dark.mean(axis=1) > 0.5), np.flatnonzero(dark.mean(axis=0) > 0.5)
    return im.crop((cols[0], rows[0], cols[-1] + 1, rows[-1] + 1))


def main():
    out_dir = os.path.join(ROOT, "assets", "book")
    inbox = os.path.join(ROOT, "_inbox", "book")
    os.makedirs(out_dir, exist_ok=True)
    os.makedirs(inbox, exist_ok=True)
    files = sorted(glob.glob(os.path.join(DOWNLOADS, "*_2K_*.jpg")), key=os.path.getmtime)
    for words, name, side, kind in TEXTURES:
        found = [f for f in files if words.lower() in os.path.basename(f).lower()]
        if not found:
            print("missing", name)
            continue
        f = found[-1]
        im = Image.open(f)
        if kind == "cover":
            im = crop_to_book(im)
        if kind == "tile":
            im = seamless(im)
        if kind == "flame":
            im = key_out(im)
        im = im.resize((side, side) if kind != "cover" else (side, int(side * im.height / im.width)), Image.LANCZOS)
        im.save(os.path.join(out_dir, f"{name}-gemini.png"))
        print(name, im.size)
        for g in found:
            shutil.move(g, os.path.join(inbox, os.path.basename(g)))


if __name__ == "__main__":
    main()

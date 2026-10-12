"""Contact sheet of cut-out sprites on the game's dark ground, for checking a batch by eye.

    python tools/sheet.py <out.png> <assets-relative stem> [...]
"""
import os
import sys

from PIL import Image

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets")
out, names = sys.argv[1], sys.argv[2:]
sheet = Image.new("RGB", (len(names) * 384, 576), (40, 36, 32))
for i, n in enumerate(names):
    im = Image.open(os.path.join(ROOT, f"{n}-gemini.png")).convert("RGBA")
    im.thumbnail((384, 576))
    sheet.paste(im, (i * 384 + (384 - im.width) // 2, 576 - im.height), im)
sheet.save(out)

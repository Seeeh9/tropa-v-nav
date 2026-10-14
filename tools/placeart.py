"""Turns backdrops downloaded from Google Flow into battle places.

    python tools/placeart.py <sheet.png> <enemy id> [...]          # -> assets/bg/at_<id>-gemini.png, 1920x1080
    python tools/placeart.py --story <sheet.png> <page id> [...]   # -> assets/story/<id>-gemini.png, 1920x1080

Takes the waiting Flow downloads in the order they arrived, one per id (`-` throws a picture
away), and writes a labelled contact sheet for checking by eye. Originals move to `_inbox/`.
"""
import glob
import os
import re
import shutil
import sys
import time

from PIL import Image, ImageDraw

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
DOWNLOADS = os.path.join(os.path.expanduser("~"), "Downloads")
STAMP = re.compile(r"_\d{14}( \(\d+\))?\.(jpe?g|png)$", re.I)

args = sys.argv[1:]
story = args[:1] == ["--story"]
if story:
    args = args[1:]
sheet_path, ids = args[0], args[1:]
folder, prefix = ("story", "") if story else ("bg", "at_")
os.makedirs(os.path.join(ROOT, "assets", folder), exist_ok=True)
fresh = time.time() - 24 * 3600
files = sorted((f for f in glob.glob(os.path.join(DOWNLOADS, "*")) if STAMP.search(f) and os.path.getmtime(f) > fresh), key=os.path.getmtime)
if len(files) != len(ids):
    raise SystemExit(f"{len(files)} downloads waiting but {len(ids)} names given; nothing moved")
cols = 4
sheet = Image.new("RGB", (cols * 480, ((len(ids) + cols - 1) // cols) * 290), (20, 17, 15))
draw = ImageDraw.Draw(sheet)
os.makedirs(os.path.join(ROOT, "_inbox"), exist_ok=True)
for k, (f, name) in enumerate(zip(files, ids)):
    if name == "-":
        os.remove(f)
        continue
    im = Image.open(f).convert("RGB")
    w, h = im.size
    nh = w * 9 // 16
    if nh <= h:
        im = im.crop((0, (h - nh) // 2, w, (h - nh) // 2 + nh))
    else:
        nw = h * 16 // 9
        im = im.crop(((w - nw) // 2, 0, (w - nw) // 2 + nw, h))
    im = im.resize((1920, 1080), Image.LANCZOS)
    im.save(os.path.join(ROOT, "assets", folder, f"{prefix}{name}-gemini.png"))
    x, y = (k % cols) * 480, (k // cols) * 290
    sheet.paste(im.resize((480, 270)), (x, y))
    draw.text((x + 4, y + 273), f"{prefix}{name}  <-  {os.path.basename(f)[:40]}", fill=(217, 207, 184))
    shutil.move(f, os.path.join(ROOT, "_inbox", f"{folder}_{name}_f.jpg"))
    print(folder, name, (w, h))
sheet.save(sheet_path)

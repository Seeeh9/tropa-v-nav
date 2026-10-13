"""Contact sheet with each picture's id and English name written under it, for checking by eye.

    python tools/labelsheet.py <out.png> cards|relics <id> [...]
"""
import glob
import os
import re
import sys

from PIL import Image, ImageDraw

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
out, kind, ids = sys.argv[1], sys.argv[2], sys.argv[3:]
names = {}
key = "card" if kind == "cards" else "relic"
for f in glob.glob(os.path.join(ROOT, "assets", "i18n", "en", "*.ftl")):
    for line in open(f, encoding="utf-8"):
        m = re.match(rf"{key}-([a-z0-9_]+) = (.*)", line)
        if m:
            names[m.group(1)] = m.group(2).strip()
cols = 5 if kind == "cards" else 6
w, h = (300, 225) if kind == "cards" else (250, 250)
rows = (len(ids) + cols - 1) // cols
sheet = Image.new("RGB", (cols * w, rows * (h + 26)), (20, 17, 15))
d = ImageDraw.Draw(sheet)
for i, n in enumerate(ids):
    x, y = (i % cols) * w, (i // cols) * (h + 26)
    path = os.path.join(ROOT, "assets", kind, f"{n}-gemini.png")
    if os.path.exists(path):
        sheet.paste(Image.open(path).convert("RGB").resize((w, h)), (x, y))
    d.text((x + 4, y + h + 2), n, fill=(217, 207, 184))
    d.text((x + 4, y + h + 13), names.get(n, "?")[:44], fill=(216, 154, 60))
sheet.save(out)

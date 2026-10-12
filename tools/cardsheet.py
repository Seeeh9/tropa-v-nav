"""Contact sheet of card illustrations, three to a row.

    python tools/cardsheet.py <out.png> <card id> [...]
"""
import os
import sys

from PIL import Image

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "cards")
out, names = sys.argv[1], sys.argv[2:]
rows = (len(names) + 2) // 3
sheet = Image.new("RGB", (1500, rows * 375))
for i, n in enumerate(names):
    sheet.paste(Image.open(os.path.join(ROOT, f"{n}-gemini.png")).resize((500, 375)), ((i % 3) * 500, (i // 3) * 375))
sheet.save(out)

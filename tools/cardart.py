"""Turns freshly downloaded Gemini pictures into card illustrations.

    python tools/cardart.py <card id> [<card id> ...]

Takes the waiting downloads in the order they arrived (see collect.py), crops each to 4:3
around its centre, scales it to 1000x750 and writes assets/cards/<id>-gemini.png.
Use `-` for a picture to throw away.
"""
import os
import subprocess
import sys

from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..")


def main():
    names = sys.argv[1:]
    out = subprocess.run([sys.executable, os.path.join(HERE, "collect.py"), *names], capture_output=True, text=True)
    if out.returncode != 0:
        raise SystemExit(out.stdout + out.stderr)
    for line in out.stdout.splitlines():
        if not line.startswith("_inbox"):
            continue
        src = os.path.join(ROOT, line.strip())
        card = os.path.basename(src).rsplit("_", 1)[0]
        im = Image.open(src).convert("RGB")
        w, h = im.size
        if w * 3 > h * 4:
            nw = h * 4 // 3
            im = im.crop(((w - nw) // 2, 0, (w - nw) // 2 + nw, h))
        else:
            nh = w * 3 // 4
            im = im.crop((0, (h - nh) // 2, w, (h - nh) // 2 + nh))
        im.resize((1000, 750), Image.LANCZOS).save(os.path.join(ROOT, "assets", "cards", f"{card}-gemini.png"))
        print("card", card)


if __name__ == "__main__":
    main()

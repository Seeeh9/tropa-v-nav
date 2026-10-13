"""Turns freshly downloaded Gemini pictures into relic icons.

    python tools/relicart.py <relic id> [<relic id> ...]

Takes the waiting downloads in the order they arrived (see collect.py), crops each to a
centred square, scales it to 256x256 and writes assets/relics/<id>-gemini.png.
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
    # relics share ids with nothing else in _inbox once prefixed
    out = subprocess.run([sys.executable, os.path.join(HERE, "collect.py"), *[n if n == "-" else f"relic_{n}" for n in names]], capture_output=True, text=True)
    if out.returncode != 0:
        raise SystemExit(out.stdout + out.stderr)
    os.makedirs(os.path.join(ROOT, "assets", "relics"), exist_ok=True)
    for line in out.stdout.splitlines():
        if not line.startswith("_inbox"):
            continue
        src = os.path.join(ROOT, line.strip())
        relic = os.path.basename(src).rsplit("_", 1)[0][len("relic_"):]
        im = Image.open(src).convert("RGB")
        w, h = im.size
        side = min(w, h)
        im = im.crop(((w - side) // 2, (h - side) // 2, (w - side) // 2 + side, (h - side) // 2 + side))
        im.resize((256, 256), Image.LANCZOS).save(os.path.join(ROOT, "assets", "relics", f"{relic}-gemini.png"))
        print("relic", relic)


main()

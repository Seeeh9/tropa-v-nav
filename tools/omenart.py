"""Takes omen pictures downloaded from Google Flow into the game.

    python tools/omenart.py <omen id> [...]   # in the order the pictures arrived

Each picture is cropped square from its middle and saved at 768x768 as
assets/omens/<id>-gemini.png, the picture framed beside the omen's words (run_ui.rs
`omen_picture`). `-` throws a picture away. Originals are kept as `_inbox/omens/<id>.jpg`.
"""
import os
import shutil
import sys

from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from flowart import ROOT, waiting  # noqa: E402

SIDE = 768


def main():
    names = sys.argv[1:]
    files = waiting()
    if len(files) != len(names):
        sys.exit(f"{len(files)} pictures waiting but {len(names)} names given; nothing moved")
    out = os.path.join(ROOT, "assets", "omens")
    keep = os.path.join(ROOT, "_inbox", "omens")
    os.makedirs(out, exist_ok=True)
    os.makedirs(keep, exist_ok=True)
    for f, name in zip(files, names):
        if name != "-":
            im = Image.open(f).convert("RGB")
            w, h = im.size
            side = min(w, h)
            im = im.crop(((w - side) // 2, (h - side) // 2, (w - side) // 2 + side, (h - side) // 2 + side))
            im.resize((SIDE, SIDE), Image.LANCZOS).save(os.path.join(out, f"{name}-gemini.png"), optimize=True)
            print("omen", name, (w, h))
        shutil.move(f, os.path.join(keep, f"{name if name != '-' else 'dropped'}_{os.path.basename(f)}"))


if __name__ == "__main__":
    main()

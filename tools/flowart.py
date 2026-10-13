"""Turns pictures downloaded from Google Flow into card illustrations or relic icons.

    python tools/flowart.py <card id> [...]            # 1000x750 -> assets/cards/<id>-gemini.png
    python tools/flowart.py --relic <relic id> [...]   # 256x256   -> assets/relics/<id>-gemini.png
    python tools/flowart.py --list                     # what is waiting, oldest first
    python tools/flowart.py --packs                    # take in every tropa-pack-*.bin and tropa-<kind>-<id>.jpg

A pack is what the page helper `flowPack` saves: several full-size pictures in one file, each
with its kind and id, because the browser lets a page save only one file at a time.

Flow names a download `<Some_title>_<yyyymmddhhmmss>.jpg`; only such files, newer than a day,
are taken, in the order they arrived. `-` throws a picture away. Originals move to `_inbox/`.
"""
import glob
import os
import re
import shutil
import sys
import time

from PIL import Image

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
DOWNLOADS = os.path.join(os.path.expanduser("~"), "Downloads")
STAMP = re.compile(r"_\d{14}( \(\d+\))?\.(jpe?g|png)$", re.I)


def waiting():
    fresh = time.time() - 24 * 3600
    files = [f for f in glob.glob(os.path.join(DOWNLOADS, "*")) if STAMP.search(f) and os.path.getmtime(f) > fresh]
    return sorted(files, key=os.path.getmtime)


def place(im, name, relic):
    w, h = im.size
    if relic:
        side = min(w, h)
        im = im.crop(((w - side) // 2, (h - side) // 2, (w - side) // 2 + side, (h - side) // 2 + side)).resize((256, 256), Image.LANCZOS)
        os.makedirs(os.path.join(ROOT, "assets", "relics"), exist_ok=True)
        out = os.path.join(ROOT, "assets", "relics", f"{name}-gemini.png")
    else:
        if w * 3 > h * 4:
            nw = h * 4 // 3
            im = im.crop(((w - nw) // 2, 0, (w - nw) // 2 + nw, h))
        else:
            nh = w * 3 // 4
            im = im.crop((0, (h - nh) // 2, w, (h - nh) // 2 + nh))
        im = im.resize((1000, 750), Image.LANCZOS)
        out = os.path.join(ROOT, "assets", "cards", f"{name}-gemini.png")
    im.save(out)
    print("relic" if relic else "card", name, (w, h))


def packs():
    import io
    import json
    import struct

    inbox = os.path.join(ROOT, "_inbox")
    os.makedirs(inbox, exist_ok=True)
    for f in sorted(glob.glob(os.path.join(DOWNLOADS, "tropa-pack-*.bin")), key=os.path.getmtime):
        data = open(f, "rb").read()
        n = struct.unpack(">I", data[:4])[0]
        head = json.loads(data[4:4 + n].decode("utf-8"))
        at = 4 + n
        for kind, name, size in head:
            raw = data[at:at + size]
            at += size
            open(os.path.join(inbox, f"{name}_f.jpg"), "wb").write(raw)
            place(Image.open(io.BytesIO(raw)).convert("RGB"), name, kind == "relic")
        os.remove(f)
    for f in sorted(glob.glob(os.path.join(DOWNLOADS, "tropa-card-*.jpg")) + glob.glob(os.path.join(DOWNLOADS, "tropa-relic-*.jpg"))):
        kind, name = os.path.basename(f)[len("tropa-"):-4].split("-", 1)
        place(Image.open(f).convert("RGB"), name, kind == "relic")
        shutil.move(f, os.path.join(inbox, f"{name}_f.jpg"))


def main():
    args = sys.argv[1:]
    if args[:1] == ["--packs"]:
        packs()
        return
    files = waiting()
    if args[:1] == ["--list"]:
        for f in files:
            print(os.path.basename(f))
        return
    relic = args[:1] == ["--relic"]
    names = args[1:] if relic else args
    if len(files) != len(names):
        raise SystemExit(f"{len(files)} downloads waiting but {len(names)} names given; nothing moved")
    inbox = os.path.join(ROOT, "_inbox")
    os.makedirs(inbox, exist_ok=True)
    os.makedirs(os.path.join(ROOT, "assets", "relics"), exist_ok=True)
    for f, name in zip(files, names):
        if name == "-":
            os.remove(f)
            continue
        im = Image.open(f).convert("RGB")
        w, h = im.size
        if relic:
            side = min(w, h)
            im = im.crop(((w - side) // 2, (h - side) // 2, (w - side) // 2 + side, (h - side) // 2 + side)).resize((256, 256), Image.LANCZOS)
            out = os.path.join(ROOT, "assets", "relics", f"{name}-gemini.png")
        else:
            if w * 3 > h * 4:
                nw = h * 4 // 3
                im = im.crop(((w - nw) // 2, 0, (w - nw) // 2 + nw, h))
            else:
                nh = w * 3 // 4
                im = im.crop((0, (h - nh) // 2, w, (h - nh) // 2 + nh))
            im = im.resize((1000, 750), Image.LANCZOS)
            out = os.path.join(ROOT, "assets", "cards", f"{name}-gemini.png")
        im.save(out)
        k = 1
        while os.path.exists(os.path.join(inbox, f"{name}_f{k}.jpg")):
            k += 1
        shutil.move(f, os.path.join(inbox, f"{name}_f{k}.jpg"))
        print("relic" if relic else "card", name, (w, h))


main()

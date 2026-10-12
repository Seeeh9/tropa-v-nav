"""Moves freshly downloaded Gemini images into `_inbox/` under their asset names.

    python tools/collect.py <name> [<name> ...]

The newest N `Gemini_Generated_Image_*` files in the Downloads folder are taken
in the order they arrived and renamed to `_inbox/<name>_<k>.jpg`, where k is the
first free take number. Use `-` for a name to skip (delete) that download.
Only files matching that pattern and newer than a day are touched.
"""
import glob
import os
import shutil
import sys
import time

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
DOWNLOADS = os.path.join(os.path.expanduser("~"), "Downloads")


def main():
    names = sys.argv[1:]
    files = sorted(glob.glob(os.path.join(DOWNLOADS, "Gemini_Generated_Image_*")), key=os.path.getmtime)
    # only this session's downloads: nothing older than a day is ours to move or drop
    fresh = time.time() - 24 * 3600
    files = [f for f in files if not f.endswith(".crdownload") and os.path.getmtime(f) > fresh]
    if len(files) != len(names):
        raise SystemExit(f"{len(files)} downloads waiting but {len(names)} names given; nothing moved")
    inbox = os.path.join(ROOT, "_inbox")
    os.makedirs(inbox, exist_ok=True)
    for f, name in zip(files, names):
        if name == "-":
            os.remove(f)
            print("dropped", os.path.basename(f))
            continue
        ext = os.path.splitext(f)[1] or ".jpg"
        k = 1
        while os.path.exists(os.path.join(inbox, f"{name}_{k}{ext}")):
            k += 1
        dst = os.path.join(inbox, f"{name}_{k}{ext}")
        shutil.move(f, dst)
        print(os.path.relpath(dst, ROOT))


if __name__ == "__main__":
    main()

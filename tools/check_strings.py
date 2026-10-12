"""Reports content ids that have no name in a language file.

    python tools/check_strings.py

Reads ids out of assets/data/**/*.ron with a regular expression (good enough for
`id: "..."` at the start of a definition) and looks for the matching Fluent key.
Exits with 1 if anything is missing, so it can gate a release.
"""
import glob
import os
import re
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets")
MARK = {"cards": r"(?:hero|kind)", "relics": "rarity", "enemies": "hp", "potions": r"(?:target|effects)", "heroes": "max_hp", "events": "kind"}
PREFIX = {"cards": "card", "relics": "relic", "enemies": "enemy", "potions": "potion", "heroes": "hero", "events": "omen"}


DUPLICATES = []


def keys(lang):
    found = set()
    for path in glob.glob(os.path.join(ROOT, "i18n", lang, "*.ftl")):
        for line in open(path, encoding="utf-8"):
            m = re.match(r"([A-Za-z0-9_-]+)\s*=", line)
            if m:
                # a key written twice: the game keeps one and the other is dead text
                if m.group(1) in found:
                    print(f"{lang}: duplicate key {m.group(1)} ({os.path.basename(path)})")
                    DUPLICATES.append(m.group(1))
                found.add(m.group(1))
    return found


def main():
    missing = 0
    for lang in ("ru", "en"):
        have = keys(lang)
        for folder, prefix in PREFIX.items():
            for path in glob.glob(os.path.join(ROOT, "data", folder, "*.ron")):
                text = open(path, encoding="utf-8").read()
                # a definition's id is followed by the field that marks its kind
                for ident in re.findall(r'id:\s*"([a-z0-9_]+)",\s*' + MARK[folder], text):
                    want = [f"{prefix}-{ident}"]
                    if folder == "enemies":
                        want += [f"move-{m}" for m in re.findall(r'\(id:\s*"([a-z0-9_]+)",\s*intent', text)]
                    for key in want:
                        if key not in have:
                            print(f"{lang}: missing {key}")
                            missing += 1
                            have.add(key)
    print("all names present" if not missing else f"{missing} names missing")
    return 1 if missing or DUPLICATES else 0


if __name__ == "__main__":
    sys.exit(main())

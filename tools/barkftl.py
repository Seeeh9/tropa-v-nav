"""Adds to assets/i18n/<lang>/voice.ftl every battle line of docs/prompts/voice/barks.<lang>.txt it lacks.

    python tools/barkftl.py

The speech scripts are the source: for old Russian they carry the stressed vowel in capitals,
which the game's text (and voice_in.py's check) does without, so those capitals are lowered,
leaving only a sentence's first letter and the names of gods and places.
"""
import os
import re

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
NAMES = {"перун", "перуне", "чернобог", "чернобогу", "навь", "русь", "бог", "громовержца", "ироды"}
LINE = re.compile(r"^(bark-[a-z0-9-]+) = (.*)$", re.M)


def plain(text):
    out, start = [], True
    for word in re.split(r"(\s+)", text):
        if not word.strip():
            out.append(word)
            continue
        low = word.lower()
        bare = re.sub(r"[^\w-]", "", low)
        keep = start or bare.split("-")[0] in NAMES
        out.append(low[:1].upper() + low[1:] if keep and low[:1].isalpha() else (low[0] + low[1:] if not keep else word[:1] + low[1:]))
        start = bool(re.search(r"[.!?…]$", word))
    return "".join(out)


def main():
    for lang in ("ru-old", "ru", "en"):
        script = open(os.path.join(ROOT, "docs", "prompts", "voice", f"barks.{lang}.txt"), encoding="utf-8").read()
        ftl_path = os.path.join(ROOT, "assets", "i18n", lang, "voice.ftl")
        ftl = open(ftl_path, encoding="utf-8").read()
        have = {m.group(1) for m in LINE.finditer(ftl)}
        new = [(k, plain(v) if lang == "ru-old" else v) for k, v in LINE.findall(script) if k not in have]
        if not new:
            continue
        block, who = [], None
        for k, v in new:
            speaker = k.split("-")[1]
            if who is not None and speaker != who:
                block.append("")
            who = speaker
            block.append(f"{k} = {v}")
        with open(ftl_path, "a", encoding="utf-8") as f:
            f.write("\n# More takes (-2 .. -5) and the lines for rare cards, gifts and gloating.\n" + "\n".join(block) + "\n")
        print(lang, len(new))


if __name__ == "__main__":
    main()

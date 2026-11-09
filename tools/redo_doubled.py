"""Records again the battle lines found said twice (or with the style read aloud) and hears them again.

    python -X utf8 tools/redo_doubled.py <lang> <bark-key> [<bark-key> ...]

Each bad clip is moved to `_inbox/voice/doubled/<lang>/`, recorded again by tts_api.py and heard by
Whisper; one still said twice goes round again, three times at most.
"""
import os
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from find_repeats import repeated_run  # noqa: E402
from hear import hear, words  # noqa: E402
from voice_in import ROOT, lines  # noqa: E402


def doubled(lang, key, want):
    heard, _ = hear(os.path.join(ROOT, "assets", "voice", lang, f"{key}.wav"), lang.split("-")[0])
    bad = repeated_run(want, heard) or len(words(heard)) >= max(4, 1.8 * len(words(want)))
    return bad, heard


def main():
    lang, keys = sys.argv[1], sys.argv[2:]
    text = lines(lang)
    keep = os.path.join(ROOT, "_inbox", "voice", "doubled", lang)
    os.makedirs(keep, exist_ok=True)
    todo = list(keys)
    for round_ in range(3):
        if not todo:
            break
        for key in todo:
            clip = os.path.join(ROOT, "assets", "voice", lang, f"{key}.wav")
            if os.path.exists(clip):
                shutil.move(clip, os.path.join(keep, f"{key}.r{round_}.wav"))
        for who in sorted({k.split("-")[1] for k in todo}):
            subprocess.run([sys.executable, "-X", "utf8", os.path.join(ROOT, "tools", "tts_api.py"), who, lang])
        still = []
        for key in todo:
            if not os.path.exists(os.path.join(ROOT, "assets", "voice", lang, f"{key}.wav")):
                print(f"MISSING {lang} {key}")
                continue
            bad, heard = doubled(lang, key, text[key])
            print(f"{'AGAIN' if bad else 'fixed'} {lang} {key}: {heard}", flush=True)
            if bad:
                still.append(key)
        todo = still
    print(f"left bad: {todo}")


if __name__ == "__main__":
    main()

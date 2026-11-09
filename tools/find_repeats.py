"""Finds voice clips in which the line is said more than once ("Угадай, кто я. Угадай, кто я").

    python -X utf8 tools/find_repeats.py <ru-old|ru|en|all> <bark|story|omen> [who ...]

Each clip is heard by the local Whisper (tools/hear.py) and set against its line. A clip is
doubtful when Whisper hears a run of three or more words twice that the line has once, when it
hears half as many words again as the line holds, or when the clip is far longer than the line
takes to say. Doubtful clips are listed in `_inbox/voice/repeats-<kind>.txt` (appended, one run
after another, so a long check can be done in batches: one speaker at a time).
"""
import os
import re
import sys
import wave

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from hear import hear, words  # noqa: E402
from voice_in import ROOT, lines  # noqa: E402

# characters a second at the slowest a line is said; a clip much longer than that is suspect
SLOWEST = {"ru-old": 9.0, "ru": 10.0, "en": 11.0}


def repeated_run(want, heard, n=3):
    """A run of `n` words heard more often than the line has it."""
    w, h = words(want), words(heard)
    runs = lambda ws: [tuple(ws[i:i + n]) for i in range(len(ws) - n + 1)]
    have = runs(w)
    for run in set(runs(h)):
        if runs(h).count(run) > max(1, have.count(run)):
            return " ".join(run)
    return None


def check(lang, kind, who):
    text = lines(lang)
    folder = os.path.join(ROOT, "assets", "voice", lang)
    out = []
    for name in sorted(os.listdir(folder)):
        key, ext = os.path.splitext(name)
        if ext != ".wav" or not key.startswith(kind + "-"):
            continue
        if who and key.split("-")[1] not in who:
            continue
        want = text.get(key)
        if kind == "omen":
            oid = key[len("omen-"):]
            title, body = text.get(f"omen-{oid}"), text.get(f"omen-{oid}-text")
            want = f"{title}. {body}" if title and body else None
        if not want:
            continue
        path = os.path.join(folder, name)
        heard, seconds = hear(path, lang.split("-")[0])
        why = []
        run = repeated_run(want, heard)
        if run:
            why.append(f"twice: «{run}»")
        if len(words(heard)) >= max(3, 1.5 * len(words(want))):
            why.append(f"{len(words(heard))} words for {len(words(want))}")
        if seconds > 2.5 and seconds * SLOWEST.get(lang, 10.0) > 2.2 * len(want):
            why.append(f"{seconds:.1f}s for {len(want)} letters")
        mark = "BAD" if why else "ok "
        print(f"{mark} {lang} {key}: {seconds:.1f}s {'; '.join(why)}    {heard}", flush=True)
        if why:
            out.append(f"{lang} {key}\t{'; '.join(why)}\twant: {want}\theard: {heard}")
    return out


def main():
    lang, kind, who = sys.argv[1], sys.argv[2], set(sys.argv[3:])
    langs = ["ru-old", "ru", "en"] if lang == "all" else [lang]
    found = []
    for lg in langs:
        found += check(lg, kind, who)
    report = os.path.join(ROOT, "_inbox", "voice", f"repeats-{kind}.txt")
    os.makedirs(os.path.dirname(report), exist_ok=True)
    with open(report, "a", encoding="utf-8") as f:
        for line in found:
            f.write(line + "\n")
    print(f"{len(found)} doubtful, listed in {report}")


if __name__ == "__main__":
    main()

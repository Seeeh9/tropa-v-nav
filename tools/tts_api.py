"""Records battle lines through the Gemini API instead of the AI Studio page.

    python tools/tts_api.py <who|all> <ru-old|ru|en|all> [count]

API keys are read from `.secrets/gemini_keys.txt` (one per line; the folder is gitignored). Every
speech model has its own quota on every key: when one pair runs out the next is used (MODELS order). Every line still missing for that speaker and language
is spoken with the speaker's voice and a style for that language alone (see STYLE), then checked
and levelled by voice_in.py's `take`, exactly like a clip from the page.
"""
import base64
import json
import os
import re
import sys
import time
import urllib.error
import urllib.request
import wave

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from voice_in import DOWNLOADS, ROOT, lines, take  # noqa: E402

VOICE = os.path.join(ROOT, "docs", "prompts", "voice")
KEYS = os.path.join(ROOT, ".secrets", "gemini_keys.txt")
# each language its own manner: one style for all of them bleeds one into another
STYLE = {
    "ru-old": "The line is in Old Russian, read in the solemn old manner: every vowel as written, unstressed O stays a round O, "
    "E never read as YO; a capital letter inside a word marks its stressed vowel.",
    "ru": "The line is in plain modern Russian: ordinary present-day pronunciation and natural stress, nothing archaic or churchly.",
    "en": "The line is in English, spoken by a Slav: a clear, noticeable Russian accent, rolled R, heavy consonants.",
}


def cast():
    out = {}
    for line in open(os.path.join(VOICE, "barks.voices.txt"), encoding="utf-8"):
        m = re.match(r"^([a-z_]+) = (\w+) \| (.*)$", line.rstrip("\r\n"))
        if m:
            out[m.group(1)] = (m.group(2), m.group(3))
    return out


def speak(key, model, voice, prompt):
    body = {
        "contents": [{"parts": [{"text": prompt}]}],
        "generationConfig": {
            "responseModalities": ["AUDIO"],
            "speechConfig": {"voiceConfig": {"prebuiltVoiceConfig": {"voiceName": voice}}},
        },
    }
    req = urllib.request.Request(
        f"https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent",
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json", "x-goog-api-key": key},
    )
    with urllib.request.urlopen(req, timeout=120) as r:
        data = json.load(r)
    part = data["candidates"][0]["content"]["parts"][0]["inlineData"]
    rate = int(re.search(r"rate=(\d+)", part.get("mimeType", "")).group(1)) if "rate=" in part.get("mimeType", "") else 24000
    return base64.b64decode(part["data"]), rate


# best first; each has its own quota on each key
MODELS = ["gemini-3.1-flash-tts-preview", "gemini-2.5-pro-preview-tts", "gemini-2.5-flash-preview-tts", "gemini-3.8-flash-lite-tts"]
SPEAKERS = ["volhv", "bogatyr", "vorozheya", "volkolak", "znaharka", "skomoroh", "leshy", "bolotnik", "mara", "koschei"]


def main():
    args = sys.argv[1:]
    whos = SPEAKERS if args[0] == "all" else [args[0]]
    langs = ["ru-old", "ru", "en"] if args[1] == "all" else [args[1]]
    count = int(args[2]) if len(args) > 2 else 999
    keys = [k.strip() for k in open(KEYS, encoding="utf-8") if k.strip() and not k.startswith("#")]
    pairs = [(m, k) for m in MODELS for k in keys]
    spent = set()
    roster = cast()
    for lang in langs:
        text = lines(lang)
        script = open(os.path.join(VOICE, f"barks.{lang}.txt"), encoding="utf-8").read()
        for who in whos:
            voice, persona = roster[who]
            todo = [(k, v) for k, v in re.findall(r"^(bark-[a-z0-9-]+) = (.*)$", script, re.M)
                    if k.split("-")[1] == who and not os.path.exists(os.path.join(ROOT, "assets", "voice", lang, f"{k}.wav"))][:count]
            for name, line in todo:
                prompt = f"{persona} {STYLE[lang]} Say only this line, once, in character: {line}"
                pcm = None
                for tries in range(3):
                    live = [p for p in pairs if p not in spent]
                    if not live:
                        print("every model on every key is out of quota")
                        return
                    model, key = live[0]
                    try:
                        pcm, rate = speak(key, model, voice, prompt)
                        break
                    except urllib.error.HTTPError as e:
                        body = e.read().decode(errors="replace")[:200]
                        if e.code in (429, 403, 404) or "quota" in body.lower():
                            print(f"{model} key {keys.index(key) + 1}: {e.code} {body[:80]!r}")
                            spent.add((model, key))
                            continue
                        print(f"{model}: {e.code} {body!r}")
                        time.sleep(5)
                    except (KeyError, IndexError, urllib.error.URLError, TimeoutError) as e:
                        print(f"{model}: no audio ({e!r})")
                        time.sleep(3)
                if pcm is None:
                    continue
                f = os.path.join(DOWNLOADS, f"api {lang} {name}.wav")
                with wave.open(f, "wb") as w:
                    w.setnchannels(1)
                    w.setsampwidth(2)
                    w.setframerate(rate)
                    w.writeframes(pcm)
                print(model, end=" ")
                take(f, lang, name, text, f"REJECTED {lang} {name}.wav")
                sys.stdout.flush()


if __name__ == "__main__":
    main()

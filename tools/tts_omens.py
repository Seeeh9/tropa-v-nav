"""Records the storyteller reading the omens (знамения) aloud, through the Gemini API.

    python -X utf8 tools/tts_omens.py <ru-old|ru|en> [count] [id ...]

Each omen is read as its title and its text: `assets/voice/<lang>/omen-<id>.wav`, played as the
omen opens. The voice is the tale's narrator (speaker Algenib, gemini-3.1-flash-tts-preview, the
scene of `docs/prompts/voice/story.<lang>.txt`), each language with its own scene. The old speech
reads `docs/prompts/voice/omens.ru-old.txt`, the text with the stressed vowels in capitals; an omen
missing there is not recorded in the old speech.

Clips are checked and levelled by voice_in.py's `take`, as every other line is. Omens recorded
already are skipped. Record two or three of a new language first and have them heard.
"""
import os
import re
import sys
import time
import urllib.error
import wave

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tts_api import KEYS, MARKDOWN, VOICE, speak  # noqa: E402
from voice_in import DOWNLOADS, ROOT, lines, take  # noqa: E402

# the narrator's model first, then the others with the same speaker as each runs out of quota for
# the day (the user: one voice everywhere); the lite model last, only when nothing else is left
MODELS = ["gemini-3.1-flash-tts-preview", "gemini-3.8-flash-tts", "gemini-2.5-pro-preview-tts", "gemini-2.5-flash-preview-tts", "gemini-3.8-flash-lite-tts"]
SPEAKER = "Algenib"


def scene(lang):
    """The narrator's scene for this language, from the comment block over the tale's script."""
    out = []
    started = False
    for line in open(os.path.join(VOICE, f"story.{lang}.txt"), encoding="utf-8"):
        if line.startswith("#   "):
            started = True
            out.append(line[4:].strip())
        elif started:
            break
    return " ".join(out)


def omens(lang):
    """(id, what is read, what Whisper should hear) for every omen."""
    text = lines("ru" if lang == "ru-old" else lang)
    stressed = {}
    if lang == "ru-old":
        path = os.path.join(VOICE, "omens.ru-old.txt")
        if os.path.exists(path):
            stressed = dict(re.findall(r"^omen-([a-z_]+) = (.*)$", open(path, encoding="utf-8").read(), re.M))
    out = []
    for key in sorted(text):
        m = re.fullmatch(r"omen-([a-z_]+)-text", key)
        if not m or f"omen-{m.group(1)}" not in text:
            continue
        oid = m.group(1)
        plain = f"{text[f'omen-{oid}']}. {text[key]}"
        if lang == "ru-old":
            if oid not in stressed:
                continue
            out.append((oid, stressed[oid], plain))
        else:
            out.append((oid, plain, plain))
    return out


def main():
    args = sys.argv[1:]
    lang = args[0]
    count = int(args[1]) if len(args) > 1 else 999
    only = set(args[2:])
    keys = [k.strip() for k in open(KEYS, encoding="utf-8") if k.strip() and not k.startswith("#")]
    pairs = [(m, k) for m in MODELS for k in keys]
    spent, cool = set(), {}
    style = scene(lang)
    todo = [o for o in omens(lang) if (not only or o[0] in only) and not os.path.exists(os.path.join(ROOT, "assets", "voice", lang, f"omen-{o[0]}.wav"))][:count]
    print(f"{lang}: {len(todo)} omens to read")
    for oid, read, plain in todo:
        name = f"omen-{oid}"
        prompt = f"{style}\n\nRead this aloud, once, exactly as written:\n{read}"
        pcm = None
        for _ in range(40):
            now = time.time()
            live = [p for p in pairs if p not in spent and cool.get(p, 0) <= now]
            if not live:
                if all(p in spent for p in pairs):
                    print("the narrator's model is out of quota on every key for the day")
                    return
                time.sleep(max(1.0, min(cool[p] for p in pairs if p not in spent) - now))
                continue
            model, key = live[0]
            try:
                laid_out = f"## Style:
{style}

## Transcript:
{read}"
                pcm, rate = speak(key, model, SPEAKER, laid_out if model in MARKDOWN else prompt)
                break
            except urllib.error.HTTPError as e:
                raw = e.read().decode(errors="replace")
                if e.code == 429:
                    if "PerDay" in raw:
                        print(f"{model} key {keys.index(key) + 1}: done for the day")
                        spent.add((model, key))
                    else:
                        m = re.search(r'"retryDelay":\s*"(\d+)', raw)
                        cool[(model, key)] = time.time() + (int(m.group(1)) if m else 60) + 1
                    continue
                if e.code in (403, 404):
                    spent.add((model, key))
                    continue
                print(f"{model}: {e.code} {raw[:200]!r}")
                time.sleep(5)
            except (KeyError, IndexError, urllib.error.URLError, TimeoutError, OSError) as e:
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
        take(f, lang, name, {name: plain}, f"REJECTED {lang} {name}.wav")
        sys.stdout.flush()


if __name__ == "__main__":
    main()

"""Paints the game's app icon through the Gemini API (an image model), several takes.

    python -X utf8 tools/icon_api.py [takes]

Takes land in `_inbox/icon/take_<n>.png`; the chosen one is made into every size the platforms want
by `tools/make_icons.py`.
"""
import base64
import json
import os
import sys
import urllib.error
import urllib.request

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
KEYS = os.path.join(ROOT, ".secrets", "gemini_keys.txt")
OUT = os.path.join(ROOT, "_inbox", "icon")
MODELS = ["gemini-3-pro-image-preview", "gemini-2.5-flash-image", "gemini-3.1-flash-image-preview"]

PROMPT = (
    "App icon for a dark Slavic folk-horror card game called 'Path to Nav' (Тропа в Навь): the road into the land of the dead. "
    "Square 1:1 composition filling the whole frame edge to edge, no rounded corners, no border, no text, no letters. "
    "Centre: a narrow forest path of old wooden planks leading between black fir trees toward a glowing pale-green gate of light; "
    "above the gate a horned skull of an ancient Slavic idol with ember-red eyes, carved from dark wood; a single raven on a branch. "
    "Deep night palette: near-black greens and blues, the gate's cold green glow and a little ember red. "
    "Painterly, dramatic rim light, strong silhouette that reads clearly even at 48 pixels, high contrast, rich detail."
)


def paint(key, model):
    body = {"contents": [{"parts": [{"text": PROMPT}]}], "generationConfig": {"responseModalities": ["IMAGE"], "imageConfig": {"aspectRatio": "1:1"}}}
    req = urllib.request.Request(
        f"https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent",
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json", "x-goog-api-key": key},
    )
    with urllib.request.urlopen(req, timeout=180) as r:
        data = json.load(r)
    for part in data["candidates"][0]["content"]["parts"]:
        if "inlineData" in part:
            return base64.b64decode(part["inlineData"]["data"])
    raise KeyError("no image")


def main():
    takes = int(sys.argv[1]) if len(sys.argv) > 1 else 4
    keys = [k.strip() for k in open(KEYS, encoding="utf-8") if k.strip() and not k.startswith("#")]
    os.makedirs(OUT, exist_ok=True)
    n = len([f for f in os.listdir(OUT) if f.startswith("take_")])
    done = 0
    for model in MODELS:
        for i, key in enumerate(keys, 1):
            while done < takes:
                try:
                    img = paint(key, model)
                except urllib.error.HTTPError as e:
                    print(f"{model} key {i}: {e.code} {e.read().decode(errors='replace')[:160]!r}")
                    break
                except (KeyError, IndexError, urllib.error.URLError, TimeoutError, OSError) as e:
                    print(f"{model} key {i}: {e!r}")
                    break
                n += 1
                done += 1
                path = os.path.join(OUT, f"take_{n}.png")
                open(path, "wb").write(img)
                print(f"{model}: {path}")
            if done >= takes:
                return


if __name__ == "__main__":
    main()

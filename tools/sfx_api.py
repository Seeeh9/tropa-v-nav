"""Makes the game's sound effects with the ElevenLabs sound-generation API.

    python tools/sfx_api.py [name ...] [--takes 3]      # all of SOUNDS when no names are given
    python tools/sfx_api.py --pick <name> <take>        # put another take into the game

The key is read from `.secrets/elevenlabs_key.txt` (gitignored). Each sound is generated `--takes`
times; every take is cut to its sound (silence trimmed from both ends, with a short fade so the cut
does not click) and kept as `_inbox/sfx/<name>_<take>.wav`. The first take goes into the game as
`assets/sfx/<name>-flow.wav`, which the game prefers over its synthesised stand-in.
"""
import json
import os
import shutil
import subprocess
import sys
import urllib.error
import urllib.request
import wave

import numpy as np

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
KEY = os.path.join(ROOT, ".secrets", "elevenlabs_key.txt")
INBOX = os.path.join(ROOT, "_inbox", "sfx")

# what each cue in audio.rs is heard on, and how long it may last (None: the model decides)
SOUNDS = {
    "block": ("Heavy wooden round shield blocking an axe blow, dull wooden thud with a ringing iron rim, single hit, dry, no music", 1.0),
    "card_draw": ("Parchment card drawn from a deck, soft quick paper swish, very short, dry, no music", 0.5),
    "card_play": ("Playing card slapped onto a wooden table, crisp paper snap, very short, dry, no music", 0.5),
    "click": ("Tiny wooden toggle click, soft dry interface tap, very short, no music", 0.5),
    "death": ("Ghoulish monster dying, rattling last breath fading into silence, dark horror, no music", 2.0),
    "defeat": ("Defeat sting: slow low war drum and a mournful horn, dark Slavic folk horror, 3 seconds", 3.0),
    "downed": ("Warrior collapsing onto forest ground, armour thud and a pained groan, no music", 1.5),
    "gold": ("Old copper coins clinking into a leather pouch, short, no music", 1.0),
    "heal": ("Healing magic: soft warm shimmer with herbal crackle and a calm breath, no music", 1.5),
    "hit": ("Sword slash cutting into a monster, short punchy flesh impact, no music", 0.7),
    "hit_heavy": ("Brutal mace blow crushing bone, deep heavy impact, short, no music", 1.0),
    "potion": ("Cork popped from a small glass bottle, then a quick gulp of liquid, no music", 1.5),
    "raised": ("Revival magic: ghostly rising whoosh and a deep gasping inhale, no music", 2.0),
    "ready": ("Two quick knocks on an oak door, dry, no music", 0.7),
    "round": ("Single deep war drum hit with a long dark reverb tail, no music", 2.0),
    "rune": ("Rune of thunder igniting: crackling electric spark and a low rumble, short, no music", 1.5),
    "status": ("Curse taking hold: eerie hissing whisper with a dark shimmer, short, no music", 1.5),
    "victory": ("Victory sting: bright folk horn fanfare with frame drums, triumphant, 3 seconds", 3.0),
}


def ffmpeg():
    import imageio_ffmpeg
    return imageio_ffmpeg.get_ffmpeg_exe()


def generate(key, text, seconds):
    body = {"text": text, "prompt_influence": 0.4}
    if seconds:
        body["duration_seconds"] = max(0.5, seconds)
    req = urllib.request.Request(
        "https://api.elevenlabs.io/v1/sound-generation",
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json", "xi-api-key": key, "Accept": "audio/mpeg"},
    )
    with urllib.request.urlopen(req, timeout=120) as r:
        return r.read()


def to_samples(mp3_path):
    """Mono 44.1 kHz 16-bit samples of any file ffmpeg can read."""
    raw = subprocess.run([ffmpeg(), "-v", "error", "-i", mp3_path, "-f", "s16le", "-ac", "1", "-ar", "44100", "-"],
                         capture_output=True, check=True).stdout
    return np.frombuffer(raw, dtype=np.int16)


def cut_silence(a, rate=44100, floor=0.02):
    """The sound alone: quiet below `floor` of its peak is cut from both ends, with soft edges."""
    if not len(a):
        return a
    x = np.abs(a.astype(np.float32))
    hop = rate // 200
    n = len(x) // hop
    env = x[: n * hop].reshape(n, hop).max(axis=1)
    loud = np.flatnonzero(env > env.max() * floor)
    if not len(loud):
        return a
    start = max(0, loud[0] * hop - rate // 200)
    end = min(len(a), (loud[-1] + 1) * hop + rate // 50)
    out = a[start:end].astype(np.float32)
    fade_in, fade_out = min(len(out), rate // 500), min(len(out), rate // 50)
    out[:fade_in] *= np.linspace(0, 1, fade_in)
    out[len(out) - fade_out:] *= np.linspace(1, 0, fade_out)
    return out.astype(np.int16)


def save(path, a, rate=44100):
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(a.tobytes())


def main():
    args = sys.argv[1:]
    if args[:1] == ["--pick"]:
        shutil.copy(os.path.join(INBOX, f"{args[1]}_{args[2]}.wav"), os.path.join(ROOT, "assets", "sfx", f"{args[1]}-flow.wav"))
        print("picked", args[1], args[2])
        return
    takes = 3
    if "--takes" in args:
        takes = int(args[args.index("--takes") + 1])
        del args[args.index("--takes"):args.index("--takes") + 2]
    names = args or list(SOUNDS)
    key = open(KEY, encoding="utf-8").read().strip()
    os.makedirs(INBOX, exist_ok=True)
    for name in names:
        text, seconds = SOUNDS[name]
        for t in range(1, takes + 1):
            try:
                mp3 = generate(key, text, seconds)
            except urllib.error.HTTPError as e:
                print(name, t, e.code, e.read().decode(errors="replace")[:200])
                return
            src = os.path.join(INBOX, f"{name}_{t}.mp3")
            open(src, "wb").write(mp3)
            a = cut_silence(to_samples(src))
            os.remove(src)
            dst = os.path.join(INBOX, f"{name}_{t}.wav")
            save(dst, a)
            print(f"{name} take {t}: {len(a) / 44100:.2f}s")
        shutil.copy(os.path.join(INBOX, f"{name}_1.wav"), os.path.join(ROOT, "assets", "sfx", f"{name}-flow.wav"))


if __name__ == "__main__":
    main()

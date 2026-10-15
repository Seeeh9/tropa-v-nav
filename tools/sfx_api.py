"""Makes the game's sound effects with the ElevenLabs sound-generation API.

    python tools/sfx_api.py [name ...] [--takes 3]      # all of SOUNDS when no names are given
    python tools/sfx_api.py --pick <name> <take>        # put another take into the game
    python tools/sfx_api.py --ambience [act1 ...]       # the looping battle ambience of each act

The key is read from `.secrets/elevenlabs_key.txt` (gitignored). Each sound is generated `--takes`
times; every take is cut to its sound (silence trimmed from both ends, with a short fade so the cut
does not click) and kept as `_inbox/sfx/<name>_<take>.wav`, the untouched take as `raw/<name>_<take>.mp3`. The first take goes into the game as
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
    # the path map, voted on together
    "map_vote": ("Small carved bone token set down firmly on an old parchment map on a wooden table, single dry tap, no music", 0.6),
    "map_nudge": ("Impatient double knock of knuckles on an old wooden table, short, dry, no music", 0.6),
    "lot_tick": ("Single dry wooden click of a spinning fortune wheel peg, very short, no music", 0.3),
    "lot_land": ("Fortune wheel stops with a heavy wooden thunk and a low ominous bell toll, no music", 2.0),
    "card_shuffle": ("Deck of old parchment cards riffle-shuffled by hand, quick fluttering paper riffle then a soft tap square, no music", 1.2),
    "card_gain": ("Parchment card swooping through the air and sliding into a deck, quick whoosh then a soft paper slap, no music", 0.8),
}


# the book of the tale: assets/sfx/book_<name>-flow.wav, takes kept like the others
BOOK = {
    "book_flip": ("Single page of an old thick parchment book turned quickly, crisp paper flutter and soft landing, no music", 0.8),
    "book_rustle": ("Old parchment page being bent and dragged slowly by a hand, soft crackling paper rustle, no music", 1.5),
    "book_tear_small": ("Small piece of old thick paper torn off, short sharp rip, no music", 0.6),
    "book_tear_big": ("A whole page ripped out of an old leather-bound book, long tearing rip with fibres snapping, no music", 1.2),
    "book_peel": ("Two glued paper pages slowly peeled apart, sticky crackling separation, no music", 1.5),
    "book_clasp": ("Brass clasp of an old leather book unlatched, small metallic click, no music", 0.5),
    "book_open": ("Heavy old leather book cover opened, creaking spine and a soft thud on a wooden table, no music", 1.5),
    "book_close": ("Heavy old leather book slammed shut, deep thud and a puff of dust, no music", 1.0),
    "book_scrap": ("Small scrap of paper falling and settling onto a wooden table, very soft, no music", 0.6),
}
SOUNDS.update(BOOK)

# battle ambience under the music, one seamless loop per act: assets/ambience/<name>-flow.wav
AMBIENCE = {
    "act1": "Dark Slavic forest at night: wind in old pines, creaking branches, a distant owl, faint unseen rustling, eerie, no music",
    "act2": "Haunted swamp at dusk: bubbling mud, croaking frogs, buzzing insects, slow dripping water, eerie, no music",
    "act3": "Frozen winter wasteland: howling icy wind, creaking ice, faint far-off wolf howl, desolate, no music",
    "act4": "Underground hall of bones: low cavern drone, distant chains, dripping water, a faint cold breath, ominous, no music",
}


def ffmpeg():
    import imageio_ffmpeg
    return imageio_ffmpeg.get_ffmpeg_exe()


def generate(key, text, seconds, loop=False):
    body = {"text": text, "prompt_influence": 0.4}
    if loop:
        body["loop"] = True
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


def cut_silence(a, rate=44100, floor=0.004):
    """The sound alone: quiet below `floor` of its peak (about -48 dB) is cut from both ends, with
    soft edges; the floor is low so reverb tails and soft decays survive."""
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
    end = min(len(a), (loud[-1] + 1) * hop + rate // 20)
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
    if args[:1] == ["--ambience"]:
        key = open(KEY, encoding="utf-8").read().strip()
        os.makedirs(os.path.join(ROOT, "assets", "ambience"), exist_ok=True)
        os.makedirs(os.path.join(INBOX, "raw"), exist_ok=True)
        for name in args[1:] or list(AMBIENCE):
            src = os.path.join(INBOX, "raw", f"amb_{name}.mp3")
            open(src, "wb").write(generate(key, AMBIENCE[name], 22.0, loop=True))
            # a loop is kept whole: trimming would break the seam
            a = to_samples(src)
            save(os.path.join(ROOT, "assets", "ambience", f"{name}-flow.wav"), a)
            print("ambience", name, f"{len(a) / 44100:.1f}s")
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
            # the untouched take is kept, so it can be cut again differently
            os.makedirs(os.path.join(INBOX, "raw"), exist_ok=True)
            src = os.path.join(INBOX, "raw", f"{name}_{t}.mp3")
            open(src, "wb").write(mp3)
            a = cut_silence(to_samples(src))
            dst = os.path.join(INBOX, f"{name}_{t}.wav")
            save(dst, a)
            print(f"{name} take {t}: {len(a) / 44100:.2f}s")
        shutil.copy(os.path.join(INBOX, f"{name}_1.wav"), os.path.join(ROOT, "assets", "sfx", f"{name}-flow.wav"))


if __name__ == "__main__":
    main()

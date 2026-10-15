"""Cut the parchment out from under painted ornaments, so they print onto the book's own page.

    python tools/inkcut.py            # frame and headpiece
    python tools/inkcut.py <stem>...  # e.g. book/frame

Reads assets/<stem>-gemini.png and writes assets/<stem>-cut.png with alpha: pixels close to the
parchment (estimated from the picture's border) turn clear, the ink stays. The game prefers the cut
file when it is there.
"""

import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

ASSETS = Path(__file__).resolve().parent.parent / "assets"
# colour distance from the parchment below which a pixel is paper, and above which it is ink
SOFT, HARD = 28.0, 70.0
# ornaments that are a rectangular border on a larger sheet
BOXED = {"book/frame"}


def band(line: np.ndarray, wide: int = 60) -> tuple[int, int]:
    """Start of the first and end of the last run of ink at least `wide` pixels long."""
    ink = np.concatenate([[False], line > 0.5, [False]])
    edges = np.flatnonzero(ink[1:] != ink[:-1])
    runs = [(a, b) for a, b in zip(edges[::2], edges[1::2]) if b - a >= wide]
    return (runs[0][0], runs[-1][1]) if runs else (0, len(line))


def cut(stem: str) -> Path:
    src = ASSETS / f"{stem}-gemini.png"
    img = Image.open(src).convert("RGB")
    rgb = np.asarray(img, dtype=np.float32)
    # the paper's own colour drifts with stains: compare each pixel with a heavily blurred copy of
    # the picture where ink has been replaced by the paper's median colour
    # paper is the commonest light colour that is not the white of the canvas around a picture
    flat = rgb.reshape(-1, 3)
    light = flat[(flat.mean(axis=1) > 150) & (flat.min(axis=1) < 245)]
    paper = np.median(light, axis=0)
    rough = np.linalg.norm(rgb - paper, axis=2)
    filled = np.where((rough < HARD)[..., None], rgb, paper)
    local = np.asarray(Image.fromarray(filled.astype(np.uint8)).filter(ImageFilter.GaussianBlur(24)), dtype=np.float32)
    dist = np.linalg.norm(rgb - local, axis=2)
    alpha = np.clip((dist - SOFT) / (HARD - SOFT), 0.0, 1.0)
    alpha[rgb.min(axis=2) > 240] = 0.0
    if stem in BOXED:
        # a border painted on a sheet: keep only the box the ornament fills; across the middle the
        # torn rim is a thin line of ink, the ornament a wide band
        h, w = alpha.shape
        top, bottom = band(alpha[:, w // 2])
        left, right = band(alpha[h // 2])
        box = np.zeros_like(alpha)
        box[top:bottom, left:right] = 1.0
        alpha *= box
    # stains and the torn rim of the sheet leave specks: an opening wipes what is thinner than the
    # pen's line, then a touch of softening keeps the cut edge from shimmering when minified
    a = Image.fromarray((alpha * 255).astype(np.uint8))
    a = a.filter(ImageFilter.MinFilter(5)).filter(ImageFilter.MaxFilter(5)).filter(ImageFilter.GaussianBlur(0.8))
    out = img.copy()
    out.putalpha(a)
    dst = ASSETS / f"{stem}-cut.png"
    out.save(dst)
    print(dst.relative_to(ASSETS.parent), f"paper {paper.round().astype(int).tolist()}", f"ink {float((alpha > 0.5).mean()):.0%}")
    return dst


if __name__ == "__main__":
    for stem in sys.argv[1:] or ["book/frame", "book/headpiece"]:
        cut(stem)

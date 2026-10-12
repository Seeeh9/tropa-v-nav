"""Procedural fallback art for «Тропа в Навь».

Everything here is a stand-in that the game uses only until a generated
`<name>-gemini.png` sits next to it. Shapes are blocked in flat, lit from the
upper left with a cold rim light (art bible, section 9), then repainted with
brush strokes so nothing looks like a vector drawing.

    python tools/gen_art.py [--only bg|heroes|enemies|cards|ui] [--force]
"""
import argparse
import math
import os
import random
import sys

import numpy as np
from PIL import Image, ImageChops, ImageDraw, ImageFilter

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets")

SOOT = (20, 17, 15)
PEAT = (43, 36, 32)
BONE = (217, 207, 184)
RUST = (122, 30, 20)
BLOOD = (168, 38, 27)
SWAMP = (62, 74, 46)
ROTLIGHT = (155, 184, 106)
CANDLE = (216, 154, 60)
NAVBLUE = (74, 106, 140)
FROST = (185, 204, 216)
IRON = (86, 84, 82)
OAK = (38, 30, 24)
COPPER = (150, 92, 52)

HERO_HUE = {"bogatyr": COPPER, "": (120, 112, 100)}


def mix(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3))


def fbm(w, h, rng, octaves=5, base=4):
    """Fractal value noise in 0..1."""
    out = np.zeros((h, w), np.float32)
    amp, total = 1.0, 0.0
    for o in range(octaves):
        n = base * 2 ** o
        grid = rng.random((n + 2, n + 2)).astype(np.float32)
        layer = Image.fromarray((grid * 255).astype(np.uint8)).resize((w, h), Image.BICUBIC)
        out += np.asarray(layer, np.float32) / 255.0 * amp
        total += amp
        amp *= 0.5
    return out / total


def blur(arr, r):
    img = Image.fromarray(np.clip(arr * 255, 0, 255).astype(np.uint8))
    return np.asarray(img.filter(ImageFilter.GaussianBlur(r)), np.float32) / 255.0


def light(rgb, mask, key=(-0.62, -0.78), rim_color=NAVBLUE, depth=14.0, rim=0.55):
    """Fake volume from a silhouette: the blurred mask is a height field."""
    h = blur(mask, depth) * 0.6 + blur(mask, depth / 3) * 0.4
    gy, gx = np.gradient(h)
    k = 26.0
    nx, ny = -gx * k, -gy * k
    nz = np.ones_like(nx)
    ln = np.sqrt(nx * nx + ny * ny + nz * nz)
    nx, ny, nz = nx / ln, ny / ln, nz / ln
    lz = 0.55
    ll = math.sqrt(key[0] ** 2 + key[1] ** 2 + lz ** 2)
    diff = np.clip((nx * key[0] + ny * key[1] + nz * lz) / ll, 0, 1)
    shade = 0.28 + 0.95 * diff
    out = rgb.astype(np.float32) * shade[..., None]
    # cold rim from the right, only near the edge
    edge = np.clip(1.0 - h * 1.6, 0, 1) * mask
    r = np.clip(nx * 0.9 - ny * 0.25, 0, 1) * edge * rim
    out = out * (1 - r[..., None]) + np.array(rim_color, np.float32) * 1.5 * r[..., None]
    return np.clip(out, 0, 255)


def paint(base, rng, seed, passes=((26, 0.20), (13, 0.55), (6, 1.0)), jitter=14, density=1.0):
    """Repaints an image with oriented round strokes sampled from itself."""
    h, w, _ = base.shape
    canvas = Image.fromarray(base.astype(np.uint8)).filter(ImageFilter.GaussianBlur(6))
    draw = ImageDraw.Draw(canvas)
    lum = base.mean(axis=2) / 255.0
    gy, gx = np.gradient(blur(lum, 3))
    flow = fbm(w, h, rng, 3, 3) * math.tau
    py = random.Random(seed)
    for size, amount in passes:
        n = int(w * h / (size * size) * 1.5 * amount * density)
        xs = rng.integers(0, w, n)
        ys = rng.integers(0, h, n)
        for x, y in zip(xs.tolist(), ys.tolist()):
            g = math.hypot(gx[y, x], gy[y, x])
            # along edges where there is one, along the noise field elsewhere
            ang = math.atan2(gx[y, x], -gy[y, x]) if g > 0.004 else flow[y, x]
            ang += py.uniform(-0.35, 0.35)
            ln = size * py.uniform(0.8, 2.2)
            dx, dy = math.cos(ang) * ln, math.sin(ang) * ln
            c = base[y, x]
            j = py.uniform(-jitter, jitter)
            col = (int(min(255, max(0, c[0] + j))), int(min(255, max(0, c[1] + j * 0.9))), int(min(255, max(0, c[2] + j * 0.8))))
            draw.line((x - dx, y - dy, x + dx, y + dy), fill=col, width=max(2, int(size * py.uniform(0.5, 1.0))))
    out = np.asarray(canvas, np.float32)
    grain = fbm(w, h, rng, 2, 64) * 0.5 + rng.random((h, w)).astype(np.float32) * 0.5
    out *= (0.90 + grain * 0.2)[..., None]
    return np.clip(out, 0, 255)


def vignette(arr, strength=0.55):
    h, w, _ = arr.shape
    y, x = np.mgrid[0:h, 0:w]
    d = np.sqrt(((x - w / 2) / (w / 2)) ** 2 + ((y - h / 2) / (h / 2)) ** 2)
    return arr * (1 - strength * np.clip(d - 0.35, 0, 1) ** 1.6)[..., None]


class Figure:
    """Flat colour regions on a transparent sheet, drawn at 2x and shrunk."""

    def __init__(self, w, h, ss=2):
        self.w, self.h, self.ss = w, h, ss
        self.rgb = Image.new("RGB", (w * ss, h * ss), (0, 0, 0))
        self.mask = Image.new("L", (w * ss, h * ss), 0)
        self.glow = Image.new("RGB", (w * ss, h * ss), (0, 0, 0))
        self.d = ImageDraw.Draw(self.rgb)
        self.m = ImageDraw.Draw(self.mask)
        self.g = ImageDraw.Draw(self.glow)

    def _s(self, pts):
        return [(x * self.ss, y * self.ss) for x, y in pts]

    def poly(self, pts, col):
        self.d.polygon(self._s(pts), fill=col)
        self.m.polygon(self._s(pts), fill=255)

    def oval(self, cx, cy, rx, ry, col):
        box = [(cx - rx) * self.ss, (cy - ry) * self.ss, (cx + rx) * self.ss, (cy + ry) * self.ss]
        self.d.ellipse(box, fill=col)
        self.m.ellipse(box, fill=255)

    def limb(self, pts, widths, col):
        """A tapered chain of segments with round joints."""
        for i in range(len(pts) - 1):
            (x0, y0), (x1, y1) = pts[i], pts[i + 1]
            w0, w1 = widths[i], widths[i + 1]
            a = math.atan2(y1 - y0, x1 - x0) + math.pi / 2
            ca, sa = math.cos(a), math.sin(a)
            self.poly([(x0 + ca * w0, y0 + sa * w0), (x1 + ca * w1, y1 + sa * w1), (x1 - ca * w1, y1 - sa * w1), (x0 - ca * w0, y0 - sa * w0)], col)
            self.oval(x0, y0, w0, w0, col)
            self.oval(x1, y1, w1, w1, col)

    def mark(self, pts, col, width=3):
        """Detail line drawn on top; doesn't extend the silhouette."""
        self.d.line(self._s(pts), fill=col, width=width * self.ss)

    def dot(self, cx, cy, r, col, glow=False):
        box = [(cx - r) * self.ss, (cy - r) * self.ss, (cx + r) * self.ss, (cy + r) * self.ss]
        self.d.ellipse(box, fill=col)
        if glow:
            self.g.ellipse(box, fill=col)

    def render(self, seed, rim_color=NAVBLUE, strokes=1.0):
        rng = np.random.default_rng(seed)
        size = (self.w, self.h)
        rgb = np.asarray(self.rgb.resize(size, Image.LANCZOS), np.float32)
        mask = np.asarray(self.mask.resize(size, Image.LANCZOS), np.float32) / 255.0
        # cloth and fur are never one flat colour
        tex = fbm(self.w, self.h, rng, 5, 6)
        rgb = rgb * (0.78 + tex * 0.44)[..., None]
        lit = light(rgb, mask, rim_color=rim_color)
        painted = paint(lit, rng, seed, passes=((16, 0.5), (8, 1.0), (4, 0.9)), density=strokes)
        glow = np.asarray(self.glow.resize(size, Image.LANCZOS).filter(ImageFilter.GaussianBlur(5)), np.float32)
        painted = np.clip(painted + glow * 1.6, 0, 255)
        # ragged painted edge instead of a clean cut-out
        edge = np.clip(blur(mask, 1.2) + (fbm(self.w, self.h, rng, 3, 40) - 0.5) * 0.35, 0, 1)
        alpha = np.clip((edge - 0.42) * 6, 0, 1) * (mask > 0.02)
        out = np.dstack([painted, alpha * 255]).astype(np.uint8)
        return Image.fromarray(out, "RGBA")


# ---------------------------------------------------------------- characters

def bogatyr():
    f = Figure(768, 1024)
    mail, tunic, skin = (92, 88, 84), (74, 34, 26), (150, 118, 96)
    # cloak behind everything
    f.poly([(250, 300), (520, 290), (590, 940), (200, 950)], (48, 26, 22))
    # legs, planted on y=980
    f.limb([(340, 640), (325, 820), (318, 968)], [62, 50, 44], (44, 38, 34))
    f.limb([(470, 640), (500, 820), (512, 968)], [62, 50, 44], (50, 42, 38))
    f.poly([(270, 955), (372, 955), (384, 984), (262, 984)], (30, 24, 20))
    f.poly([(468, 955), (572, 955), (590, 984), (462, 984)], (30, 24, 20))
    # mail skirt and barrel torso
    f.poly([(270, 560), (560, 560), (590, 760), (245, 760)], mail)
    f.poly([(250, 290), (560, 290), (590, 600), (240, 600)], mail)
    f.poly([(300, 300), (500, 300), (520, 600), (290, 600)], tunic)
    f.poly([(256, 548), (572, 548), (576, 590), (252, 590)], (40, 28, 20))
    f.oval(410, 569, 22, 22, COPPER)
    # shoulders
    f.oval(250, 320, 92, 74, (70, 66, 62))
    f.oval(566, 320, 92, 74, (78, 74, 70))
    # mace arm, raised on the right
    f.limb([(590, 330), (668, 450), (640, 560)], [50, 40, 34], mail)
    f.oval(640, 566, 36, 34, skin)
    f.limb([(646, 600), (606, 250)], [13, 12], (60, 42, 28))
    for a in range(6):
        ang = a * math.tau / 6
        f.poly([(600 + math.cos(ang) * 16, 205 + math.sin(ang) * 16), (600 + math.cos(ang + 0.5) * 58, 205 + math.sin(ang + 0.5) * 58), (600 + math.cos(ang + 1.0) * 16, 205 + math.sin(ang + 1.0) * 16)], IRON)
    f.oval(600, 205, 34, 34, (72, 70, 68))
    # head: grey hair, iron mask, beard
    f.oval(408, 205, 84, 96, (128, 124, 118))
    f.poly([(336, 230), (480, 230), (462, 420), (410, 470), (356, 420)], (150, 146, 138))
    f.poly([(346, 130), (470, 130), (482, 262), (408, 300), (334, 262)], (64, 62, 60))
    f.dot(378, 196, 17, (6, 5, 5))
    f.dot(440, 196, 17, (6, 5, 5))
    f.mark([(408, 136), (408, 290)], (40, 38, 36), 4)
    f.mark([(340, 236), (478, 236)], (40, 38, 36), 3)
    # tower shield on the left arm, front of the body
    f.poly([(60, 330), (300, 300), (318, 800), (190, 930), (70, 800)], OAK)
    f.poly([(60, 330), (300, 300), (302, 330), (64, 360)], COPPER)
    f.poly([(70, 800), (190, 930), (318, 800), (312, 770), (190, 896), (76, 770)], COPPER)
    # the carved face of Rod
    f.mark([(130, 470), (170, 455)], (88, 66, 46), 6)
    f.mark([(215, 455), (255, 470)], (88, 66, 46), 6)
    f.mark([(190, 470), (190, 600), (160, 630)], (88, 66, 46), 6)
    f.mark([(130, 690), (190, 720), (250, 690)], (88, 66, 46), 6)
    # bandages with old blood
    for y in (440, 462, 484):
        f.mark([(606, y - 10), (690, y + 14)], (176, 164, 140), 7)
    f.mark([(624, 452), (664, 476)], RUST, 6)
    return f.render(101, strokes=1.0)


def grey_wolf():
    f = Figure(512, 768)
    fur, dark = (104, 102, 100), (58, 56, 56)
    oy = 250
    # tail
    f.limb([(398, 300 + oy), (462, 330 + oy), (480, 410 + oy)], [26, 22, 8], dark)
    # hind legs
    f.limb([(370, 330 + oy), (400, 420 + oy), (372, 505 + oy)], [50, 26, 16], dark)
    f.limb([(330, 340 + oy), (352, 430 + oy), (330, 505 + oy)], [44, 24, 15], fur)
    # body, low and arched
    f.oval(290, 300 + oy, 150, 78, fur)
    f.oval(180, 280 + oy, 96, 86, (112, 110, 108))
    # hackles
    for i in range(9):
        x = 150 + i * 30
        f.poly([(x - 16, 240 + oy), (x + 4, 190 + oy - (i % 3) * 8), (x + 22, 242 + oy)], dark)
    # forelegs
    f.limb([(200, 330 + oy), (176, 430 + oy), (160, 505 + oy)], [36, 20, 14], fur)
    f.limb([(150, 330 + oy), (120, 420 + oy), (96, 500 + oy)], [34, 19, 14], dark)
    # neck and head, snout to the left
    f.limb([(170, 250 + oy), (110, 220 + oy)], [58, 50], fur)
    f.oval(100, 205 + oy, 58, 50, (112, 110, 108))
    f.poly([(70, 185 + oy), (4, 228 + oy), (22, 252 + oy), (92, 246 + oy)], fur)
    f.poly([(30, 240 + oy), (16, 262 + oy), (96, 264 + oy), (100, 244 + oy)], dark)
    f.poly([(98, 160 + oy), (86, 96 + oy), (128, 150 + oy)], dark)
    f.poly([(128, 158 + oy), (140, 100 + oy), (160, 168 + oy)], fur)
    for x in (30, 44, 58, 72):
        f.poly([(x, 246 + oy), (x + 5, 262 + oy), (x + 10, 246 + oy)], BONE)
    f.dot(8, 228 + oy, 8, (10, 8, 8))
    f.dot(86, 196 + oy, 9, ROTLIGHT, glow=True)
    return f.render(202, rim_color=(110, 130, 120))


def kikimora():
    f = Figure(512, 768)
    skin, rag, hair = (128, 140, 112), (58, 60, 44), (34, 36, 30)
    # hair hanging down the hunched back
    f.poly([(250, 150), (420, 190), (470, 620), (360, 700), (300, 420)], hair)
    # ragged dress
    f.poly([(210, 330), (380, 320), (430, 730), (160, 738)], rag)
    for i in range(7):
        x = 165 + i * 38
        f.poly([(x, 720), (x + 16, 764), (x + 34, 722)], rag)
    # spine arched forward
    f.limb([(330, 330), (270, 250), (200, 200)], [62, 54, 40], skin)
    # arms far too long, reaching left
    f.limb([(250, 270), (150, 380), (60, 430)], [22, 15, 10], skin)
    f.limb([(280, 290), (210, 440), (120, 540)], [22, 15, 10], (112, 124, 98))
    for (hx, hy) in ((60, 430), (120, 540)):
        for k in range(4):
            a = math.pi * (0.75 + k * 0.14)
            f.limb([(hx, hy), (hx + math.cos(a) * 54, hy + math.sin(a) * 54)], [6, 2], (90, 100, 80))
    # head thrust forward, face half hidden
    f.oval(176, 176, 52, 62, skin)
    f.poly([(150, 110), (250, 120), (236, 300), (196, 250), (168, 330), (140, 220)], hair)
    f.poly([(142, 206), (110, 232), (150, 244)], skin)
    f.dot(150, 178, 8, (230, 232, 210), glow=True)
    f.mark([(128, 222), (168, 232)], (30, 20, 20), 4)
    # bird bones braided into the hair
    for (x, y) in ((330, 330), (380, 450), (350, 560)):
        f.mark([(x, y), (x + 26, y + 14)], BONE, 4)
    return f.render(303, rim_color=ROTLIGHT)


# ---------------------------------------------------------------- backgrounds

def tree(d, x, base_y, height, width, col, py, lean=0.0):
    top = base_y - height
    d.polygon([(x - width, base_y), (x + width, base_y), (x + width * 0.3 + lean * height, top), (x - width * 0.3 + lean * height, top)], fill=col)
    # spruce boughs: drooping tiers
    tiers = int(height / 60)
    for i in range(tiers):
        t = i / max(1, tiers)
        y = top + height * (0.08 + t * 0.7)
        cx = x + lean * (base_y - y)
        span = width * (2.2 + t * 7.5) * py.uniform(0.7, 1.2)
        droop = 30 + t * 60
        for side in (-1, 1):
            d.polygon([(cx, y - 12), (cx + side * span, y + droop), (cx + side * span * 0.55, y + droop * 0.35), (cx, y + 16)], fill=col)


def forest(w, h, seed, sky_top, sky_low, fog_col, glow_col, glow_at, path=False):
    rng = np.random.default_rng(seed)
    py = random.Random(seed)
    y = np.linspace(0, 1, h)[:, None, None]
    sky = np.array(sky_top, np.float32) * (1 - y) + np.array(sky_low, np.float32) * y
    arr = np.repeat(sky, w, axis=1)
    clouds = fbm(w, h, rng, 5, 3)
    arr = arr * (0.7 + clouds * 0.6)[..., None]
    # the single key light: moon behind cloud, upper left
    yy, xx = np.mgrid[0:h, 0:w]
    gx, gy = glow_at[0] * w, glow_at[1] * h
    dist = np.sqrt((xx - gx) ** 2 + ((yy - gy) * 1.3) ** 2)
    arr += np.array(glow_col, np.float32) * (np.exp(-dist / (w * 0.16)) * 1.1)[..., None]
    img = Image.fromarray(np.clip(arr, 0, 255).astype(np.uint8))
    horizon = int(h * 0.64)
    # tree lines, far to near, each darker and sharper
    for layer in range(4):
        t = layer / 3
        col = mix(mix(fog_col, sky_low, 0.4), SOOT, 0.25 + t * 0.7)
        lay = Image.new("RGBA", (w, h), (0, 0, 0, 0))
        d = ImageDraw.Draw(lay)
        n = [26, 16, 9, 5][layer]
        for i in range(n):
            x = (i + py.uniform(0.1, 0.9)) * w / n
            if path and abs(x - w * 0.5) < w * (0.05 + t * 0.07):
                continue
            base_y = horizon + t * h * 0.32 + py.uniform(-12, 12)
            tree(d, x, base_y, h * (0.5 + t * 0.75) * py.uniform(0.8, 1.15), 5 + t * 30, col + (255,), py, py.uniform(-0.04, 0.04))
        if layer < 3:
            lay = lay.filter(ImageFilter.GaussianBlur(3.5 - layer))
        img.paste(lay, (0, 0), lay)
        # fog settles between the lines
        fog = fbm(w, h, rng, 4, 4)
        band = np.exp(-((yy - (horizon + t * h * 0.22)) / (h * 0.10)) ** 2)
        a = np.clip(fog * band * (0.75 - t * 0.5), 0, 1)
        base = np.asarray(img, np.float32)
        base = base * (1 - a[..., None]) + np.array(fog_col, np.float32) * a[..., None]
        img = Image.fromarray(np.clip(base, 0, 255).astype(np.uint8))
    arr = np.asarray(img, np.float32)
    # ground: dark earth, a lighter band where the fight stands
    g = np.clip((yy - horizon - h * 0.02) / (h * 0.10), 0, 1)
    ground = np.array(mix(SOOT, fog_col, 0.22), np.float32) * (0.6 + fbm(w, h, rng, 5, 8) * 0.8)[..., None]
    if path:
        lane = np.exp(-((xx - w * 0.5) / (w * (0.05 + 0.5 * np.clip((yy - horizon) / (h - horizon), 0, 1)))) ** 2)
        ground += np.array(mix(fog_col, BONE, 0.2), np.float32) * (lane * 0.35)[..., None]
    arr = arr * (1 - g[..., None] * 0.92) + ground * (g[..., None] * 0.92)
    arr = paint(np.clip(arr, 0, 255), rng, seed, passes=((34, 0.25), (18, 0.6), (9, 0.8)), jitter=10)
    return Image.fromarray(vignette(arr).astype(np.uint8))


# ---------------------------------------------------------------- cards

def card_scene(seed, hue, motif):
    """A 4:3 card picture: one lit object on a dark ground."""
    w, h = 800, 600
    rng = np.random.default_rng(seed)
    yy, xx = np.mgrid[0:h, 0:w]
    bg = np.array(mix(SOOT, hue, 0.16), np.float32) * (0.5 + fbm(w, h, rng, 5, 3) * 1.1)[..., None]
    d = np.sqrt((xx - w * 0.36) ** 2 + (yy - h * 0.30) ** 2)
    bg += np.array(mix(hue, CANDLE, 0.4), np.float32) * (np.exp(-d / (w * 0.30)) * 0.55)[..., None]
    f = Figure(w, h)
    cx, cy = w / 2, h / 2
    metal, wood = (104, 100, 96), (64, 46, 32)
    if motif == "mace":
        f.limb([(cx + 190, cy + 210), (cx - 110, cy - 120)], [15, 13], wood)
        for a in range(6):
            ang = a * math.tau / 6 + 0.3
            f.poly([(cx - 130 + math.cos(ang) * 22, cy - 140 + math.sin(ang) * 22), (cx - 130 + math.cos(ang + 0.52) * 96, cy - 140 + math.sin(ang + 0.52) * 96), (cx - 130 + math.cos(ang + 1.04) * 22, cy - 140 + math.sin(ang + 1.04) * 22)], metal)
        f.oval(cx - 130, cy - 140, 52, 52, (84, 82, 80))
    elif motif == "sweep":
        for i in range(26):
            a = math.pi * (1.05 + i * 0.034)
            r0, r1 = 150 + i * 2, 250 - i * 1.5
            f.limb([(cx + 60 + math.cos(a) * r0, cy + 130 + math.sin(a) * r0), (cx + 60 + math.cos(a) * r1, cy + 130 + math.sin(a) * r1)], [9, 3], mix(BONE, hue, i / 26))
        f.limb([(cx + 250, cy + 190), (cx + 90, cy + 20)], [14, 12], wood)
        f.oval(cx + 76, cy + 6, 46, 46, metal)
    elif motif in ("shield", "guard", "wall"):
        def shield(x, y, s, col):
            f.poly([(x - 90 * s, y - 150 * s), (x + 90 * s, y - 150 * s), (x + 100 * s, y + 60 * s), (x, y + 170 * s), (x - 100 * s, y + 60 * s)], col)
            f.poly([(x - 90 * s, y - 150 * s), (x + 90 * s, y - 150 * s), (x + 92 * s, y - 126 * s), (x - 92 * s, y - 126 * s)], COPPER)
            f.oval(x, y - 10 * s, 26 * s, 26 * s, COPPER)
            f.mark([(x, y - 120 * s), (x, y + 150 * s)], (70, 54, 38), 4)
        if motif == "wall":
            for i, x in enumerate((-230, -80, 80, 230)):
                shield(cx + x, cy + 30 + (i % 2) * 14, 0.82, mix(OAK, hue, 0.12 * (i % 2)))
        else:
            shield(cx, cy + 6, 1.25, OAK)
            if motif == "guard":
                f.oval(cx + 150, cy - 30, 50, 60, (40, 34, 30))
                f.poly([(cx + 90, cy + 20), (cx + 210, cy + 20), (cx + 240, cy + 300), (cx + 70, cy + 300)], (40, 34, 30))
    elif motif == "roar":
        f.poly([(cx - 110, cy - 190), (cx + 110, cy - 190), (cx + 130, cy + 60), (cx, cy + 190), (cx - 130, cy + 60)], (70, 68, 66))
        f.dot(cx - 50, cy - 60, 30, (6, 5, 5))
        f.dot(cx + 50, cy - 60, 30, (6, 5, 5))
        f.poly([(cx - 60, cy + 30), (cx + 60, cy + 30), (cx + 34, cy + 120), (cx - 34, cy + 120)], (10, 6, 6))
        for i in range(12):
            a = i * math.tau / 12
            f.limb([(cx + math.cos(a) * 230, cy + math.sin(a) * 200), (cx + math.cos(a) * 330, cy + math.sin(a) * 290)], [7, 2], mix(BLOOD, CANDLE, 0.3))
    elif motif == "fist":
        f.poly([(cx - 330, cy + 300), (cx - 130, cy - 60), (cx - 10, cy + 40), (cx + 90, cy - 150), (cx + 330, cy + 300)], (58, 56, 60))
        f.limb([(cx - 20, cy + 300), (cx + 20, cy + 40)], [78, 66], (140, 110, 90))
        f.oval(cx + 26, cy - 20, 96, 84, (150, 118, 96))
        for i in range(4):
            f.mark([(cx - 46 + i * 44, cy - 96), (cx - 40 + i * 44, cy + 10)], (90, 66, 54), 5)
    elif motif == "mail":
        for row in range(9):
            for col in range(13):
                x = cx - 300 + col * 50 + (row % 2) * 25
                y = cy - 200 + row * 46
                f.oval(x, y, 27, 27, metal)
                f.dot(x, y, 15, mix(SOOT, hue, 0.2))
    elif motif == "burden":
        f.oval(cx + 10, cy - 80, 210, 150, (74, 72, 76))
        f.poly([(cx - 150, cy + 300), (cx - 70, cy + 60), (cx + 110, cy + 50), (cx + 180, cy + 300)], (60, 40, 30))
        f.oval(cx - 40, cy + 70, 46, 50, (128, 124, 118))
        f.limb([(cx - 110, cy + 110), (cx - 190, cy - 10)], [30, 24], (92, 88, 84))
        f.limb([(cx + 110, cy + 100), (cx + 200, cy - 10)], [30, 24], (92, 88, 84))
    elif motif == "spear":
        f.limb([(cx - 300, cy + 240), (cx + 250, cy - 170)], [9, 8], wood)
        f.poly([(cx + 240, cy - 190), (cx + 340, cy - 240), (cx + 268, cy - 140)], metal)
        f.limb([(cx - 60, cy + 300), (cx - 20, cy + 90)], [40, 30], (140, 112, 92))
        f.oval(cx - 16, cy + 62, 44, 40, (150, 118, 96))
        f.limb([(cx + 40, cy + 300), (cx + 90, cy + 150)], [30, 22], (96, 84, 80))
        f.oval(cx + 96, cy + 128, 34, 30, (120, 104, 96))
    elif motif == "sigil":
        f.oval(cx, cy, 170, 170, (30, 34, 44))
        for i in range(3):
            a = i * math.tau / 3 - math.pi / 2
            f.limb([(cx, cy), (cx + math.cos(a) * 150, cy + math.sin(a) * 150)], [10, 4], FROST)
        f.dot(cx, cy, 30, FROST, glow=True)
    elif motif == "web":
        for i in range(12):
            a = i * math.tau / 12
            f.limb([(cx, cy), (cx + math.cos(a) * 340, cy + math.sin(a) * 280)], [3, 2], (190, 190, 180))
        for r in (60, 120, 180, 240):
            pts = [(cx + math.cos(i * math.tau / 12) * r * 1.2, cy + math.sin(i * math.tau / 12) * r) for i in range(13)]
            for a, b in zip(pts, pts[1:]):
                f.limb([a, b], [2, 2], (170, 170, 160))
    elif motif == "mire":
        f.oval(cx, cy + 90, 360, 130, (40, 50, 30))
        for (x, y, r) in ((-150, 60, 34), (60, 100, 46), (190, 50, 26), (-40, 130, 22)):
            f.oval(cx + x, cy + y, r, r * 0.6, (70, 86, 50))
        f.limb([(cx - 60, cy + 80), (cx - 40, cy - 110)], [16, 9], (120, 130, 104))
        for k in range(4):
            a = -math.pi / 2 + (k - 1.5) * 0.4
            f.limb([(cx - 40, cy - 110), (cx - 40 + math.cos(a) * 70, cy - 110 + math.sin(a) * 70)], [7, 3], (120, 130, 104))
    fig = f.render(seed + 7, rim_color=mix(NAVBLUE, hue, 0.3))
    bgp = paint(np.clip(bg, 0, 255), rng, seed, passes=((30, 0.25), (15, 0.6)), jitter=10)
    out = Image.fromarray(bgp.astype(np.uint8)).convert("RGBA")
    shadow = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    shadow.putalpha(fig.getchannel("A").filter(ImageFilter.GaussianBlur(14)).point(lambda v: int(v * 0.6)))
    out.alpha_composite(shadow, (10, 16))
    out.alpha_composite(fig)
    arr = vignette(np.asarray(out.convert("RGB"), np.float32), 0.75)
    return Image.fromarray(arr.astype(np.uint8))


CARDS = {
    "bogatyr_strike": "mace",
    "bogatyr_defend": "shield",
    "bogatyr_guard": "guard",
    "bogatyr_sweep": "sweep",
    "bogatyr_shield_wall": "wall",
    "bogatyr_challenge": "roar",
    "bogatyr_svyatogor": "fist",
    "bogatyr_ancestor_mail": "mail",
    "bogatyr_heavy_burden": "burden",
    "bogatyr_raise_spear": "spear",
    "nav_mark": "sigil",
    "web": "web",
    "mire": "mire",
}


def portrait(sheet, box):
    """Head-and-shoulders crop on a dark painted ground."""
    crop = sheet.crop(box).resize((512, 512), Image.LANCZOS)
    rng = np.random.default_rng(7)
    bg = np.array(PEAT, np.float32) * (0.5 + fbm(512, 512, rng, 4, 3))[..., None]
    out = Image.fromarray(np.clip(bg, 0, 255).astype(np.uint8)).convert("RGBA")
    out.alpha_composite(crop)
    return out.convert("RGB")


def save(img, rel, force):
    path = os.path.join(ROOT, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    if os.path.exists(path) and not force:
        return
    img.save(path, optimize=True)
    print("  wrote", rel, img.size)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--only")
    ap.add_argument("--force", action="store_true")
    a = ap.parse_args()
    want = lambda k: a.only in (None, k)
    if want("bg"):
        save(forest(1920, 1080, 11, (26, 34, 32), (58, 70, 62), (92, 108, 96), (150, 170, 150), (0.22, 0.16)), "bg/act1_battle.png", a.force)
        save(forest(1920, 1080, 12, (14, 18, 24), (40, 52, 60), (70, 92, 104), (120, 150, 180), (0.5, 0.42), path=True), "bg/menu.png", a.force)
    if want("heroes"):
        b = bogatyr()
        save(b, "heroes/bogatyr.png", a.force)
        save(portrait(b, (228, 60, 588, 420)), "portraits/bogatyr.png", a.force)
    if want("enemies"):
        save(grey_wolf(), "enemies/grey_wolf.png", a.force)
        save(kikimora(), "enemies/kikimora.png", a.force)
    if want("cards"):
        for i, (card, motif) in enumerate(CARDS.items()):
            hue = HERO_HUE["bogatyr"] if card.startswith("bogatyr") else (NAVBLUE if motif == "sigil" else SWAMP)
            save(card_scene(500 + i, hue, motif), f"cards/{card}.png", a.force)


if __name__ == "__main__":
    sys.exit(main())

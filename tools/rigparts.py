"""Split a painted figure into its body and moving parts.

python tools/rigparts.py <original.png> <parts_removed.jpg> <colour_mask.jpg> <out_dir> <name> <preview.png>

The colour mask is the original with each part painted flat: magenta the weapon arm, cyan the
head, yellow the shield or off arm, green a second arm, blue a tail or cloak.
Writes <name>_body.png (the removed-parts edit under the parts, the original elsewhere) and
<name>_<part>.png for each part found, all the size of the original.
"""
import sys
import numpy as np
from PIL import Image, ImageFilter
from scipy import ndimage

orig_p, edit_p, mask_p, out_dir, name, prev_p = sys.argv[1:7]
o = Image.open(orig_p).convert("RGBA")
w, h = o.size
O = np.asarray(o).astype(float)
oa = O[:, :, 3] / 255.0
# Flow returns its own frame (3:4), not the picture's: lay both edits over the original by
# matching the figure's box in the colour mask (which still shows the whole figure) to the
# original's
def box(m):
    ys, xs = np.nonzero(m)
    return xs.min(), ys.min(), xs.max(), ys.max()


Mi = Image.open(mask_p).convert("RGB")
Ma = np.asarray(Mi).astype(float)
mbg = np.median(np.concatenate([Ma[:10, :10].reshape(-1, 3), Ma[:10, -10:].reshape(-1, 3)]), axis=0)
mx0, my0, mx1, my1 = box(ndimage.binary_opening(np.linalg.norm(Ma - mbg, axis=2) > 30, iterations=2))
ox0, oy0, ox1, oy1 = box(oa > 0.5)
sx = (mx1 - mx0) / max(ox1 - ox0, 1)
sy = (my1 - my0) / max(oy1 - oy0, 1)
# output pixel (x, y) of the original's frame reads the edit at this place
coeffs = (sx, 0, mx0 - ox0 * sx, 0, sy, my0 - oy0 * sy)


def lay(im):
    return np.asarray(im.convert("RGB").transform((w, h), Image.AFFINE, coeffs, Image.BICUBIC, fillcolor=tuple(int(v) for v in mbg))).astype(float)


E = lay(Image.open(edit_p))
M = lay(Mi)

corner = np.concatenate([E[:20, :20].reshape(-1, 3), E[:20, -20:].reshape(-1, 3), E[-20:, :20].reshape(-1, 3), E[-20:, -20:].reshape(-1, 3)])
bg = np.median(corner, axis=0)
ea = np.clip((np.linalg.norm(E - bg, axis=2) - 10) / 25, 0, 1)
ea = np.asarray(Image.fromarray((ea * 255).astype(np.uint8)).filter(ImageFilter.MedianFilter(5))).astype(float) / 255

COLOURS = {"arm": (255, 0, 255), "head": (0, 255, 255), "shield": (255, 255, 0), "arm2": (0, 255, 0), "tail": (0, 0, 255)}


def soft(m, grow):
    m = ndimage.binary_opening(m, iterations=1)
    m = ndimage.binary_fill_holes(ndimage.binary_closing(m, iterations=3))
    m = ndimage.binary_dilation(m, iterations=grow)
    return np.asarray(Image.fromarray((m * 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(1.5))).astype(float) / 255


import os
only = os.environ.get("PARTS", "").split(",") if os.environ.get("PARTS") else None
parts = {}
for part, c in COLOURS.items():
    if only and part not in only:
        continue
    m = np.linalg.norm(M - np.array(c, float), axis=2) < 90
    if m.sum() > w * h * 0.002:
        parts[part] = soft(m, int(os.environ.get("GROW", "2")))
        ys, xs = np.nonzero(m)
        print(part, "box %.3f %.3f %.3f %.3f" % (xs.min() / w, ys.min() / h, xs.max() / w, ys.max() / h))

anyp = np.zeros((h, w))
for s in parts.values():
    anyp = np.maximum(anyp, s)
under = np.asarray(Image.fromarray((anyp * 255).astype(np.uint8)).filter(ImageFilter.MaxFilter(9)).filter(ImageFilter.GaussianBlur(3))).astype(float) / 255
body_rgb = O[:, :, :3] * (1 - under[..., None]) + E * under[..., None]
body_a = oa * (1 - under) + ea * under
# scraps of a part the mask missed lie loose of the body: keep only the body's large pieces
lab, n = ndimage.label(body_a > 0.25)
if n > 1:
    sizes = ndimage.sum(np.ones_like(body_a), lab, range(1, n + 1))
    keep = np.isin(lab, [i + 1 for i in range(n) if sizes[i] > sizes.max() * 0.03])
    body_a = body_a * ndimage.binary_dilation(keep, iterations=2)
body = Image.fromarray(np.dstack([body_rgb, body_a * 255]).clip(0, 255).astype(np.uint8), "RGBA")
body.save(f"{out_dir}/{name}_body-gemini.png")
layers = [body]
for part, s in parts.items():
    im = Image.fromarray(np.dstack([O[:, :, :3], oa * s * 255]).clip(0, 255).astype(np.uint8), "RGBA")
    im.save(f"{out_dir}/{name}_{part}-gemini.png")
    layers.append(im)

prev = Image.new("RGBA", (w * (len(layers) + 1), h), (60, 90, 60, 255))
for i, im in enumerate(layers):
    prev.alpha_composite(im, (i * w, 0))
whole = body.copy()
for im in layers[1:]:
    whole.alpha_composite(im)
prev.alpha_composite(whole, (len(layers) * w, 0))
# a grid at every tenth of the picture over the whole figure, to read joints off by eye
from PIL import ImageDraw
g = ImageDraw.Draw(prev)
x0 = len(layers) * w
for k in range(1, 10):
    g.line([(x0 + w * k / 10, 0), (x0 + w * k / 10, h)], fill=(255, 255, 0, 120), width=2)
    g.line([(x0, h * k / 10), (x0 + w, h * k / 10)], fill=(255, 255, 0, 120), width=2)
prev.resize((prev.size[0] // 3, h // 3)).save(prev_p)


# joints read off each part's box: a hanging arm turns at its top (the elbow), the head at the
# neck, three quarters down its box; a raised arm needs its joint set by hand
RUST = {"arm": "Arm", "head": "Head", "shield": "Shield"}
cuts = []
for part, s in parts.items():
    if part not in RUST:
        continue
    ys, xs = np.nonzero(s > 0.5)
    x0, x1, y0, y1 = xs.min() / w, xs.max() / w, ys.min() / h, ys.max() / h
    top = xs[ys < ys.min() + 0.03 * h].mean() / w
    if part == "head":
        cuts.append((0, f"(Part::Head, Vec2::new({(x0 + x1) / 2:.3f}, {y0 + (y1 - y0) * 0.75:.3f}), 1.0)"))
    else:
        cuts.append((1 if part == "shield" else 2, f"(Part::{RUST[part]}, Vec2::new({top:.3f}, {y0 + 0.01:.3f}), {0.8 if part == 'shield' else 1.0})"))
stem = out_dir.replace("\\", "/").rstrip("/").split("/")[-1]
print(f'    ("{stem}/{name}", &[{", ".join(c for _, c in sorted(cuts))}]),')

# the figure lying dead, if Flow drew it (<edit dir>/<name>_dead.jpg): laid over the original's
# frame like the other edits but on a canvas wide enough to lie in, its plain background keyed out
import os as _os
for _kind in ("dead", "dead_headless"):
  dead_p = _os.path.join(_os.path.dirname(edit_p), f"{name}_{_kind}.jpg")
  if _os.path.exists(dead_p):
      W2 = int(h * 1.5)
      dx = (W2 - w) / 2
      dcoeffs = (sx, 0, mx0 - (ox0 + dx) * sx, 0, sy, my0 - oy0 * sy)
      D_ = np.asarray(Image.open(dead_p).convert("RGB").transform((W2, h), Image.AFFINE, dcoeffs, Image.BICUBIC, fillcolor=tuple(int(v) for v in mbg))).astype(float)
      dbg = np.median(np.concatenate([D_[:10, :10].reshape(-1, 3), D_[:10, -10:].reshape(-1, 3)]), axis=0)
      da = np.clip((np.linalg.norm(D_ - dbg, axis=2) - 12) / 25, 0, 1)
      # an uneven grey floor: whatever pale and grey touches the edge of the picture is background
      pale = (D_.min(2) > 120) & (D_.max(2) - D_.min(2) < 28)
      lab_, _n = ndimage.label(pale)
      edge_ids = np.unique(np.concatenate([lab_[0], lab_[-1], lab_[:, 0], lab_[:, -1]]))
      floor = np.isin(lab_, edge_ids[edge_ids > 0])
      da = da * (1 - ndimage.binary_dilation(floor, iterations=1))
      da = np.asarray(Image.fromarray((da * 255).astype(np.uint8)).filter(ImageFilter.MedianFilter(5))).astype(float) / 255
      lab, n = ndimage.label(da > 0.3)
      if n > 1:
          sizes = ndimage.sum(np.ones_like(da), lab, range(1, n + 1))
          da = da * ndimage.binary_dilation(np.isin(lab, [i + 1 for i in range(n) if sizes[i] > sizes.max() * 0.05]), iterations=2)
      dead = Image.fromarray(np.dstack([D_, da * 255]).clip(0, 255).astype(np.uint8), "RGBA")
      # Flow draws the body smaller than it stands: grow it about its bottom middle until it lies as
      # long as the figure stands tall, capped by the canvas
      bx0, by0, bx1, by1 = dead.getbbox()
      k = min((oy1 - oy0) * 0.95 / max(bx1 - bx0, 1), W2 * 0.98 / max(bx1 - bx0, 1), h * 0.98 / max(by1 - by0, 1))
      body = dead.crop((bx0, by0, bx1, by1))
      body = body.resize((int(body.width * k), int(body.height * k)), Image.LANCZOS)
      dead = Image.new("RGBA", (W2, h))
      dead.alpha_composite(body, (int(W2 / 2 - body.width / 2), min(h - body.height, int(oy1 - body.height * 0.92))))
      dead.save(f"{out_dir}/{name}_{_kind}-gemini.png")
      print(_kind, W2, h)

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
E = np.asarray(Image.open(edit_p).convert("RGB").resize((w, h), Image.LANCZOS)).astype(float)
M = np.asarray(Image.open(mask_p).convert("RGB").resize((w, h), Image.LANCZOS)).astype(float)

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
        parts[part] = soft(m, 2)
        ys, xs = np.nonzero(m)
        print(part, "box %.3f %.3f %.3f %.3f" % (xs.min() / w, ys.min() / h, xs.max() / w, ys.max() / h))

anyp = np.zeros((h, w))
for s in parts.values():
    anyp = np.maximum(anyp, s)
under = np.asarray(Image.fromarray((anyp * 255).astype(np.uint8)).filter(ImageFilter.MaxFilter(9)).filter(ImageFilter.GaussianBlur(3))).astype(float) / 255
body_rgb = O[:, :, :3] * (1 - under[..., None]) + E * under[..., None]
body_a = oa * (1 - under) + ea * under
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
prev.resize((prev.size[0] // 3, h // 3)).save(prev_p)

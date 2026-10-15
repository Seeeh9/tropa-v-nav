"""Adds the pre-1918 letters (yat, fita, izhitsa) that Ruslan Display lacks, built from its own
glyphs, and renames the font as the OFL asks of a changed one:
    python tools/old_letters_font.py assets/fonts/RuslanDisplay-Regular.ttf assets/fonts/NavDisplay-Regular.ttf
"""
import sys
from fontTools.ttLib import TTFont
from fontTools.ttLib.tables._g_l_y_f import Glyph, GlyphComponent
src, dst = sys.argv[1], sys.argv[2]
f = TTFont(src)
glyf, hmtx, cmap = f['glyf'], f['hmtx'], f.getBestCmap()

def comp(name, x, y, sx=1.0, sy=1.0):
    c = GlyphComponent()
    c.glyphName, c.x, c.y, c.flags = name, x, y, 0x4
    if sx != 1.0 or sy != 1.0:
        c.transform = [[sx, 0], [0, sy]]
    return c

def add(code, name, parts, adv, lsb):
    g = Glyph(); g.numberOfContours = -1; g.components = parts
    glyf[name] = g
    hmtx[name] = (adv, lsb)
    order = f.getGlyphOrder()
    if name not in order:
        f.setGlyphOrder(order + [name])
    for t in f['cmap'].tables:
        if t.isUnicode():
            t.cmap[code] = name
    g.recalcBounds(glyf)

soft, bar, o, v = cmap[0x42C], cmap[0x2D], cmap[0x41E], cmap[0x56]
# yat: the soft sign with a crossbar over its stem, raised a little above the cap line
stem = cmap[0x406]
# yat: the soft sign whose stem rises above the cap line, crossed at the top
yat = [comp(soft, 60, 0), comp(stem, 60, 430, 1.0, 0.38), comp(bar, 0, 291, 1.4, 0.8)]
add(0x462, 'uni0462', yat, 689, 0)
add(0x463, 'uni0463', yat, 689, 0)
# fita: the O struck through
fita = [comp(o, 0, 0), comp(bar, 64, 13, 1.45, 1.0)]
add(0x472, 'uni0472', fita, 658, 34)
add(0x473, 'uni0473', fita, 658, 34)
# izhitsa: drawn as the V
add(0x474, 'uni0474', [comp(v, 0, 0)], 703, 8)
add(0x475, 'uni0475', [comp(v, 0, 0)], 703, 8)
# the reserved name may not be carried by a changed font
for n in f['name'].names:
    s = n.toUnicode()
    if 'Ruslan' in s and n.nameID != 0:
        n.string = s.replace('Ruslan Display', 'Nav Display').replace('RuslanDisplay', 'NavDisplay').replace('Ruslan', 'Nav')
f['maxp'].recalc(f) if hasattr(f['maxp'], 'recalc') else None
f.save(dst)

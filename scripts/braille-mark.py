#!/usr/bin/env python3
"""The Unseen Servant text mark, in Unicode Braille, derived from the brand.

Why this draws rather than downsamples
--------------------------------------
`assets/logo.png` renders the figure as an *outline* of glowing digits: the
interior is mostly dark and the edges are scattered glyphs. No automatic
route survives that at icon size.

* Blurring merges the arms into the torso, so it reads as a bell or a
  cloche rather than a person.
* Raising the threshold keeps only the brightest digits and leaves scatter.
* Row-wise run-merging fills the body, but the merge distance that closes
  the digit texture also closes the gap between the legs.

So the *proportions are measured* from the logo, per row, which an outline
gives accurately, and the figure is redrawn to them. Deriving an icon from a
full illustration normally requires exactly this.

The measurements (percentages of the figure's own box, 1086 x 2682 px,
aspect 2.47):

    head crown      25% wide at the very top row, so the crown is flat
    head widest     48% at 10% of height
    jaw             36% at 13%
    neck            14% to 20%
    shoulders       76% wide, 19% to 27%
    crotch          45.5% measured; drawn at 58.4% so the gap between
                    the legs is a quarter shorter (director's call)
    hands end       70%
    feet            splayed to 84% wide at the foot

Two things that had to be got right, both learned by getting them wrong:

* **Every horizontal measure is an integer half-width, and the left side is
  the mirror of the right.** Drawing both sides from floats let rounding
  open the arm gap on one side only.
* **The eyes are knocked out of the head mass**, not added as lit dots, or
  they read as noise rather than as eyes.

Usage
-----
    python3 scripts/braille-mark.py                      # standard, 24 cols
    python3 scripts/braille-mark.py --variant heavy
    python3 scripts/braille-mark.py --all --cols 16
"""
import argparse
from PIL import Image, ImageDraw

# Braille dot numbering is not sequential, which is the thing that catches
# people out:  (0,0)=1  (1,0)=8 / (0,1)=2  (1,1)=16 / (0,2)=4  (1,2)=32 /
# (0,3)=64 (1,3)=128
BITS = ((0x01, 0x08), (0x02, 0x10), (0x04, 0x20), (0x40, 0x80))
BLANK = chr(0x2800)
ASPECT = 2.47

# The measured crotch is 45.5% of figure height, which makes the gap between
# the legs 51.5% tall (45.5 to the 97% foot line). The director asked for that
# gap to be a quarter shorter, so the split starts lower: 97 - (51.5 * 0.75).
CROTCH_MEASURED = 45.5
FOOT_LINE = 97.0
CROTCH = FOOT_LINE - (FOOT_LINE - CROTCH_MEASURED) * 0.75      # 58.4%

VARIANTS = {
    'slim':     dict(label='slim build',     shoulder=70, torso=36, arm=11, eye=1.4),
    'standard': dict(label='measured build', shoulder=76, torso=40, arm=12, eye=1.4),
    'heavy':    dict(label='heavy build',    shoulder=82, torso=44, arm=14, eye=1.4),
}
DEFAULT = 'standard'


def draw(cols, shoulder, torso, arm, eye, label=None):
    w = cols * 2
    h = int(round(w * ASPECT))
    h += (-h) % 4                      # a whole number of braille rows
    im = Image.new('1', (w, h), 0)
    d = ImageDraw.Draw(im)
    cx = w // 2
    Y = lambda p: int(round(p * h / 100.0))
    HW = lambda p: int(round(p * w / 200.0))     # integer half-width, in dots
    rad = lambda f: max(1, int(w * f))

    def mirrored(hw_out, hw_in, y0, y1, r=0):
        """Draw the same shape on both sides from integer offsets, so the
        two are guaranteed identical."""
        for sgn in (-1, 1):
            x0, x1 = sorted((cx + sgn * hw_out, cx + sgn * hw_in))
            if r:
                d.rounded_rectangle([x0, y0, x1, y1], radius=r, fill=1)
            else:
                d.rectangle([x0, y0, x1, y1], fill=1)

    # head: flat crown, bulging sides, tapered jaw
    d.rounded_rectangle([cx - HW(48), Y(1.5), cx + HW(48), Y(11)],
                        radius=rad(0.055), fill=1)
    d.rectangle([cx - HW(26), Y(0.5), cx + HW(26), Y(4)], fill=1)
    d.polygon([(cx - HW(48), Y(9)), (cx + HW(48), Y(9)),
               (cx + HW(36), Y(15)), (cx - HW(36), Y(15))], fill=1)
    er = int(round(eye * w / 40.0))
    for sgn in (-1, 1):
        ex = cx + sgn * HW(23)
        d.ellipse([ex - er, Y(5.6), ex + er, Y(9.2)], fill=0)

    d.rectangle([cx - HW(12), Y(14), cx + HW(12), Y(20)], fill=1)
    d.rounded_rectangle([cx - HW(shoulder), Y(19), cx + HW(shoulder), Y(27)],
                        radius=rad(0.05), fill=1)
    d.polygon([(cx - HW(torso + 4), Y(25)), (cx + HW(torso + 4), Y(25)),
               (cx + HW(torso), Y(CROTCH + 1)),
               (cx - HW(torso), Y(CROTCH + 1))], fill=1)

    mirrored(HW(shoulder), HW(shoulder) - 2 * HW(arm), Y(26), Y(64), rad(0.035))
    mirrored(HW(shoulder) + 1, HW(shoulder) - 2 * HW(arm) - 1,
             Y(60), Y(70), rad(0.025))                       # hands
    mirrored(HW(torso), HW(11), Y(CROTCH), Y(FOOT_LINE))     # legs
    for sgn in (-1, 1):                                      # feet
        d.polygon([(cx + sgn * HW(58), Y(FOOT_LINE)), (cx + sgn * HW(5.5), Y(FOOT_LINE)),
                   (cx + sgn * HW(5.5), Y(100)), (cx + sgn * HW(54), Y(100))],
                  fill=1)
    return im


def braille(im):
    w, h = im.size
    px = im.load()
    rows = []
    for cy in range(0, h, 4):
        line = ''
        for cx in range(0, w, 2):
            v = 0
            for dy in range(4):
                for dx in range(2):
                    if px[cx + dx, cy + dy]:
                        v |= BITS[dy][dx]
            line += chr(0x2800 + v)
        rows.append(line)
    while rows and set(rows[0]) <= {BLANK}: rows.pop(0)
    while rows and set(rows[-1]) <= {BLANK}: rows.pop()
    return rows


def blocks(im):
    """The same figure in half-block characters instead of Braille.

    Braille is denser and is what the brand uses, but it renders badly
    in a lot of terminal and gopher-client fonts: U+28xx is often absent
    from the primary font, gets substituted from a fallback with a
    different advance width, and every character after it on the line
    shifts. The result is a figure that is correct on the wire and
    visibly broken on screen — reported against the live gopherhole on
    2026-08-30.

    U+2588 and U+2584/U+2580 are in almost every monospace font that
    exists and are reliably single-width, so cleartext surfaces get
    these instead. Two pixel rows per character line rather than
    Braille's four, so the figure is half the resolution and twice as
    robust.
    """
    w, h = im.size
    px = im.load()
    rows = []
    for cy in range(0, h, 2):
        line = ''
        for cx in range(w):
            top = px[cx, cy]
            bot = px[cx, cy + 1] if cy + 1 < h else 0
            line += {(0, 0): ' ', (1, 0): '\u2580', (0, 1): '\u2584', (1, 1): '\u2588'}[
                (1 if top else 0, 1 if bot else 0)
            ]
        rows.append(line.rstrip() or ' ')
    while rows and not rows[0].strip():
        rows.pop(0)
    while rows and not rows[-1].strip():
        rows.pop()
    return rows


def mark(variant=DEFAULT, cols=24, style='braille'):
    kw = dict(VARIANTS[variant])
    kw.pop('label', None)
    im = draw(cols, **kw)
    return blocks(im) if style == 'blocks' else braille(im)


if __name__ == '__main__':
    ap = argparse.ArgumentParser()
    ap.add_argument('--variant', default=DEFAULT, choices=sorted(VARIANTS))
    ap.add_argument('--cols', type=int, default=24)
    ap.add_argument('--all', action='store_true')
    ap.add_argument('--blocks', action='store_true',
                    help='half-block characters instead of Braille, for fonts that lack U+28xx')
    a = ap.parse_args()
    if a.all:
        for key in ('slim', 'standard', 'heavy'):
            print(f"=== {key}: {VARIANTS[key]['label']} ({a.cols} columns) ===")
            print('\n'.join(mark(key, a.cols, 'blocks' if a.blocks else 'braille')))
            print()
    else:
        print('\n'.join(mark(a.variant, a.cols, 'blocks' if a.blocks else 'braille')))

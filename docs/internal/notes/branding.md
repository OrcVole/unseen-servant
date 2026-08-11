# Branding direction

Recorded 2026-08-09 from the director's review of six artwork concepts
(in the estate's `Passing/` handoff folder, `Unseen-Servant-{1..6}`).
This note fixes the direction so it survives to the work that consumes
it: the project capsule + web homepage (C3/C7), the store icon and
`postInstallMessage` (C6), and the v1.1 announcement (ROADMAP M6).

## The chosen direction: the dot-mesh servant (concept 1)

**Director's pick, stated explicitly: concept 1, "the unseen servant
represented by a mesh of dots."** The image is a CRT-phosphor-green
terminal scene — a `#`-tiled room border, an altar/table bearing a
chalice drawn in box-glyphs, and to its right **a human figure rendered
as a fine mesh/point-cloud of dots**, present but barely-there. The
likely branding is "something like that."

Why it fits, so the reason survives the image:

- **Terminal-native, not skeuomorphic.** Phosphor green on black, ASCII/
  box-drawing glyphs, CRT curvature — it reads as Geminispace's own
  aesthetic rather than a generic app logo. The other five concepts are
  scene illustrations (a pixel-art hearth-kitchen, an isometric tavern
  with a self-pouring bottle, a candlelit hall with a floating
  candelabra) — charming, and usable for an announcement banner or blog
  header, but they are *scenes*, not *marks*.
- **The dot-mesh literally is the name.** An "unseen servant" made of
  points is present-but-not-quite-visible — the concept and the picture
  are the same idea. That is what a brand mark should do.
- **It scales down to a mark.** The dot-mesh figure (or a distilled
  glyph of it) is the part that can become a favicon, a 256x256 store
  icon, and an ASCII/gemtext rendition for the Gemini-side capsule
  header, where a full illustration cannot go. The chalice/altar motif
  is a strong secondary (it also nods to the D&D spell the name comes
  from — concept 4's floating candelabra leans into that lineage too).

## What consumes this, and the constraints each imposes

- **Store icon (C6):** 256x256 PNG (cloudron-fit.md §6). The dot-mesh
  figure or a distilled glyph, on the phosphor-green/near-black palette.
- **Favicon (web surface):** must survive to ~16px. A full point-cloud
  will mud at that size — needs a simplified mark derived from it, not
  a downscale of the full art.
- **Gemini-side capsule header:** the Gemini surface is gemtext, so the
  branding there is necessarily an **ASCII/box-glyph** rendition — which
  concept 1 is already halfway to, being terminal art in the first
  place. A hand-tuned gemtext version of the dot-mesh + chalice is the
  natural capsule masthead.
- **Web theme tie-in:** concept 1's palette (phosphor green on near-
  black) is essentially a fifth theme waiting to happen — a "Terminal"
  or "Phosphor" theme alongside Daybreak/Midnight/Tokyo Night/Paper
  (`src/render/theme.rs`) would let the capsule's web surface match its
  own branding. Worth proposing when themes are revisited; not built yet.
- **Announcement/blog (M6):** the richer scene illustrations (concepts
  2/3/4) are good candidates for a one-off announcement header where a
  full image is appropriate and a 16px mark is not the concern.

## Not yet done

No art is committed into the repo yet (the source PNGs live in the
estate handoff folder, outside this publishable repo). Before v1.0's
store submission someone needs to: pick/produce the final icon asset
from concept 1, derive the favicon and the gemtext masthead from it,
and decide whether the Phosphor theme ships. Tracked here rather than
in an ADR because it is a design/asset decision, not an architectural
one — promote to an ADR only if it starts constraining code (e.g. if
the Phosphor theme or a bundled icon asset lands in the crate).

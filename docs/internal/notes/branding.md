# Branding

Settled 2026-08-11 by the director. This note is the authority; the
earlier direction (a phosphor-green dot-mesh figure, concept 1 of six)
is recorded at the bottom because the *reasoning* still holds even
though the palette moved.

## The mark

A standing human figure assembled from glowing ones and zeros, lettered
`USV` at the chest, wordmark "Unseen Servant" beneath, on a near-black
ground. A servant made of data: present, useful, and not quite there, 
the picture and the name are the same idea, which is what a mark should
do.

| Asset | Path |
|---|---|
| Full mark, 1024px | `assets/logo.png` |
| 512px | `assets/logo-512.png` |
| 280px, for inline use | `assets/logo-small.png` |
| Favicons (`.ico`, 16, 32, apple-touch, android 192/512, manifest) | `assets/favicon/` |
| Original, 4096px | Outside the repo, in the estate image folder |

## The colour: Ember Oxide

**`#E67916`**: old terminal amber, with a nod to the language this is
written in. It marks code and examples, and it is the accent on every
surface.

One constraint worth keeping: pure ember does not carry enough contrast
for body-sized text on a light ground (about 2.9:1 against the paper
white, short of the 4.5:1 that normal text needs). So:

| Ground | Code and link text | Rules, chips, marks |
|---|---|---|
| Dark | `#F0A353` (lifted) / `#E67916` | `#E67916` |
| Light | `#9A4C05` (burnt down, ~6.6:1) | `#E67916` |

Accent pairs stay inside the amber family: `#E67916` with `#D9A441` on
dark, `#9A4C05` with `#6B4E12` on light. Nothing blue or pink; the
earlier draft of the agent page used both and they fought the mark.

## The typeface: Iosevka

<https://github.com/be5invis/Iosevka>: open source, monospace, and the
same shape as the wordmark. It sets *everything*, not only the code: a
project whose subject is text on a wire should look like text on a wire.

Served pages carry a Latin subset of three faces (regular, bold, italic)
at `assets/fonts/`, about 74 kB in total: small enough to be honest on
a network whose whole point is being small. Each `@font-face` lists
`local("Iosevka")` first, so a reader who already has it uses their own
copy and downloads nothing.

Installed on the build workstation from `PkgTTF-Iosevka-34.8.0.zip` into
`~/.local/share/fonts/Iosevka/` (regular, italic, bold, bold italic,
plus light/medium/semibold and their italics).

## Still to do

- A "Phosphor"/"Ember" theme: see the colour schemes note. Shipped as
  `phosphor` in `src/render/theme.rs`.
- A "Phosphor"/"Ember" theme alongside Daybreak, Midnight, Tokyo Night
  and Paper (`src/render/theme.rs`) would let a capsule's web surface
  match the mark. Proposed, not built.
- The store icon for Cloudron submission is 256×256 (`recon/cloudron-fit.md`
  §6); derive it from `assets/logo-512.png` rather than the 4096px
  original.

## Superseded: the phosphor-green direction (2026-08-09)

The first review picked concept 1 of six: a CRT-phosphor-green terminal
scene: a `#`-tiled room border, an altar bearing a chalice in
box-glyphs, and a human figure rendered as a fine mesh of dots. The
palette has since moved to amber, but three arguments from that review
carried over into the mark now in use, and are why it was chosen:

- **Terminal-native, not skeuomorphic.** It reads as the small
  internet's own aesthetic rather than a generic app logo. The other
  five concepts were scene illustrations: charming, and still usable
  for an announcement banner, but scenes are not marks.
- **The figure literally is the name.** Present but barely there.
- **It scales down.** The figure survives as a favicon where a full
  illustration cannot.

## Where this gets used

Three surfaces, and they are not the same thing:

1. **The capsule itself**: served by `usv` from its own content tree, on
   Gemini, Gopher, Spartan, Nex, Finger and its web mirror. Themed by
   `src/render/theme.rs`, so a scheme lands here as Rust, not CSS.
2. **The website and documentation**: planned as Astro, with Astro
   Starlight for the docs that do not live on Forgejo. The eight tokens
   in `theme-options.html` are named to port straight onto Starlight's
   own custom properties, and the Iosevka subset in `assets/fonts/`
   carries over as-is.
3. **The code forge**: a dedicated `forgejo.unseenservant.dev`.

Keeping (1) and (2) distinct matters: the capsule is the product
demonstrating itself, and it must stay renderable by `usv` alone with no
build step. The Astro site is ordinary web work and may do more.

## Colour schemes on offer

Six named schemes, all built on Ember Oxide, are laid out with live
specimens in `theme-options.html`: **Ember** (warm dark), **Foolscap**
(warm light), **Phosphor** (monochrome amber CRT), **Burrow** (gopher
earth tones), **Slate & Ember** (cool dark), **Bone** (high-contrast
light). Every one is contrast-measured, body text clears 10.8:1 in the
worst case, code 5.4:1. Awaiting the director's pick; nothing in
`theme.rs` has changed.

## The text mark

The smolnet surfaces are text, so the figure needs a text rendition. It is
drawn in Unicode Braille (U+2800 to U+28FF), which gives a 2x4 grid of dots
per character: four times the vertical resolution of block glyphs, and
literally a figure made of dots, which is the mark.

`assets/mark.braille.txt` holds it at 24 columns (masthead) and 16 columns
(inline). Regenerate with `scripts/braille-mark.py`; do not edit by hand.

**It is drawn, not downsampled, and that is deliberate.** The logo renders
the figure as an outline of glowing digits with a mostly dark interior.
Blurring it merges the arms into the torso and the result reads as a bell or
a cloche rather than a person; thresholding leaves unreadable scatter;
run-merging fills the body but at the width that also closes the gap between
the legs. So the proportions are measured from the logo, per row, and the
icon is redrawn to them. The measurements are the constants at the top of
the script: head 1 to 20 percent of height, shoulders at 26, arms to 72,
legs from 76, feet splayed to 84 percent of width.

Three constraints it has to keep meeting:

- **Width.** Gopher menu display strings truncate around 70 characters in
  classic clients. 24 columns leaves plenty of room.
- **No tabs, carriage returns or newlines**, which would corrupt a gopher
  menu record. Braille contains none. Verified byte-identical over a real
  gopher socket.
- **Alt text, always.** A wall of Braille is hostile to a screen reader.
  Gemtext's preformatted alt text is exactly the fix and the HTML renderer
  turns it into a figcaption, so the mark is never served bare.

The webfont subset in `assets/fonts/` includes the Braille block for this
reason. Without it the glyphs fall back to another font and the grid breaks.

One honest limit: RFC 1436 assumes ASCII or Latin-1. Modern gopher practice
is UTF-8 and every current client handles it, but a genuinely ancient client
may render the mark as mojibake.

# C3 design brief: dual render (gemtext → HTML/Atom/feeds)

Prepared 2026-08-09 as groundwork ahead of C3, so a future session can start
building instead of re-deriving the grammar and module shape. Not an ADR, 
open questions in §5 need the director's read before code lands on them.

## 1. Proposed module layout for `src/render/`

Following the `protocol/` precedent (small files, doc-comment-dense,
layered framing → validation → authority) and `handler/`'s pattern of a
`mod.rs` that states the ADR, the trait/type shape, and what is deliberately
*not* here:

```
src/render/
  mod.rs          // ADR 0004 pointer, layering doc, re-exports, RenderError
  gemtext.rs      // layer 1: line-type parser (fuzzed). &str -> Vec<Line>.
                  // No knowledge of HTML, feeds, or files: pure grammar.
  metadata.rs     // layer 2: walks a parsed document -> Metadata{title, date, ...}
                  // convention-only extraction (see §4); never mutates the doc.
  html.rs         // layer 3a: Line stream -> semantic HTML (classless). Escaping lives here.
  feed/
    mod.rs
    atom.rs       // layer 3b: directory of dated Metadata -> atom.xml (both surfaces)
    gemsub.rs     // layer 3b: directory of dated Metadata -> gemtext index link-lines
  theme.rs        // bundled classless CSS set + custom-CSS path resolution
  pipeline.rs     // ties parse+metadata+html+feed together per file; owns the
                  // content_dir -> state_dir/html tree-walk and atomic staging-swap
  watcher.rs      // debounce + fs-event -> pipeline invocation (kept separate
                  // from pipeline so pipeline stays independently testable/fuzzable)
```

`gemtext.rs` is the one module that must be identical for both surfaces
(ADR 0004 says so explicitly) and is the fuzz target: keeping it free of
HTML/feed concerns means the fuzz corpus only exercises grammar, not
rendering policy. `metadata.rs` sits between parser and emitters because
both `html.rs` (page `<title>`) and `feed/*` need the same extraction;
duplicating it per-emitter would let title/date conventions drift between
surfaces, exactly what ADR 0004 exists to prevent. `pipeline.rs`/
`watcher.rs` are separated the way `server.rs` is separated from
`protocol/`: protocol logic must not know about tokio tasks or fs events.

Fuzz targets to add (matching the existing four): `parse_gemtext.rs`
(parser never panics, always terminates, round-trips line count), and
possibly `render_html.rs` (parser output → HTML never produces unescaped
`<`/`&`/`"` in text nodes: security-relevant since rendered HTML is served
to real browsers).

## 2. Gemtext grammar: line-type decision table

One-pass, one bit of state (`Normal` | `Preformatted`), starting in
`Normal`; state at EOF is meaningless (spec is silent: do not error on
"still preformatted" at EOF, just end the document). The toggle check
happens *before* everything else: it applies in both modes and its own
line is never itself a text/heading/list/quote line.

```abnf
gemtext-doc      = 1*gemtext-line
gemtext-line     = text-line / link-line / preformat-toggle /
                   heading-line / list-item / quote-line
                   ; (last three recognized only in Normal mode; in
                   ;  Preformatted mode every non-toggle line is
                   ;  preformatted text-line, verbatim, unparsed)

preformat-toggle = "```" *any-char   ; exactly 3 backticks, no leading
                                      ; whitespace, at line start; flips
                                      ; mode. Opening: trailing text =
                                      ; alt-text. Closing: trailing text
                                      ; ignored. Never itself rendered.

link-line        = "=>" [1*(SP/HTAB)] URL-reference [1*(SP/HTAB) NAME]
heading-line     = 1*3("#") [1*(SP/HTAB)] text
list-item        = "*" SP text-line          ; space is MANDATORY
quote-line       = ">" text-line             ; NO space required
text-line        = *any-char                 ; the fallback: anything not
                                              ; matching the above
```

Decision table, checked top to bottom, first match wins (mirrors the "at
most first three characters" spec framing):

| Mode | First bytes | Line type |
|---|---|---|
| any | ` ``` ` exactly, col 0 | preformat-toggle, flips mode |
| Preformatted | anything else | verbatim preformatted text (`<pre>`, never re-parsed) |
| Normal | `=>` | link-line |
| Normal | `#`, `##`, or `###` | heading-line (level = run length, capped at 3: a 4th+ `#` is still level 3, extra `#`s become leading text) |
| Normal | `* ` (asterisk + space) | list-item: **`*` without the following space is NOT a list item**, falls through to text-line |
| Normal | `>` | quote-line (no space check) |
| Normal | (fallback) | text-line |

Normative details not to lose: heading/list/quote support is *optional*, 
a renderer that does not implement one MUST render that line as plain text,
never error or drop it. Whitespace after `=>` and heading `#`s may be
spaces or tabs (0.24.1). Non-ASCII text in text lines is legal UTF-8.
Empty lines are ordinary text lines (vertical space, never collapsed).
There is no escape mechanism for a literal ` ``` ` inside a preformat
block (gemini-text issue #17, open, unresolved): usv's own generated
content must never need it.

## 3. Gemtext parsing edge cases worth dedicated unit tests

- Empty document (spec requires `1*gemtext-line`; decide + test both
  "zero lines" and "one empty text line" readings for a zero-byte file)
- A single blank line, and multiple consecutive blank lines (must not collapse)
- `*text` (no trailing space): must render as text-line, not list-item
- `*` alone, or `* ` with empty text
- `>text` vs `> text`: both are quote-line
- Heading with tab instead of space; heading with no whitespace (`#Title`)
- 4, 5+ consecutive `#`: must degrade to heading level 3, not error
- List/heading markers *inside* a preformatted block: literal text, never reparsed
- Link line: tab-only whitespace, mixed tab+space, multiple spaces before NAME
- Link line with no NAME (bare `=> gemini://x/`)
- Link line with an unencoded space in the URL (spec says URLs MUST
  percent-encode; decide reject/pass-through/best-effort: see §5.1)
- ` ``` ` with trailing alt-text on open; trailing (ignored) text on close
- Nested/duplicate ` ``` ` mid-line inside preform content: must NOT toggle
  (only a line starting with exactly ` ``` ` at column 0 toggles)
- A document that opens ` ``` ` and never closes before EOF: must not panic
- Non-ASCII (UTF-8 multibyte) text lines, including at a line boundary
  that could confuse byte-vs-char indexing
- CRLF vs bare-LF line splitting (files may be bare-LF on disk per ADR
  0004: distinct from the wire-protocol request parser, which rejects
  bare LF for a different reason)
- A line consisting solely of a BOM; a document beginning with a BOM
- Extremely long single line (no gemtext length limit, unlike the
  1024-byte *request* URI limit: do not conflate the two)
- Mixed `*`/`#`/`>` on the same line vs. sequential lines (only the first
  token governs; the rest is just text)

## 4. Metadata pass: what to extract, and where it comes from

Per BUILD-PLAN C3: "metadata pass (titles, dates, feeds)." Nothing in
`docs/internal/recon/protocol.md` mandates a title/date convention: gemtext has
none codified in the core spec, but two things anchor the design: the
**subscription companion spec** (defines the only existing convention,
`=> URL YYYY-MM-DD - title` link lines interpreted as a feed by
CAPCOM/Antenna/Lagrange), and community norm (not spec-mandated) of
treating the first `#` heading as the page title.

- **Title**: first level-1 `# ` heading found (fallback: filename-derived,
  e.g. `about.gmi` → "About", since HTML needs a non-empty `<title>` even
  for heading-less pages).
- **Date**: no core-spec per-page convention exists. The only spec-adjacent
  convention is the subscription companion spec's link-line date, which
  lives on the *index* page pointing at other pages, not inside the target
  page. Two options for §5.2: (a) read dates only when walking an index
  page's outbound links (spec-native, zero new syntax); (b) invent a
  usv-specific in-page convention. Recommend (a) as primary, filesystem
  mtime as a silent Atom `<updated>` fallback.
- **Feed membership**: which generated index pages a page's dated link
  lines appear on, to build `atom.xml` (title, absolute link, updated,
  optionally a stable `<id>`) and the gemsub index text itself.
- **Description/summary**: not in BUILD-PLAN scope; likely deferred (Atom
  `<summary>` defaults to empty or first paragraph: a §5 decision if
  wanted at all for v1).

## 5. Open design questions for the coding session

1. **HTML emitter strictness.** Malformed-but-legal gemtext (e.g. an
   unencoded space in a link URL): pass through raw, best-effort
   percent-encode on emission, or drop/flag? The parser can never "fail"
   on a whole document (headings/lists/quotes degrade to text by spec),
   but the *emitter* still needs a link-hygiene policy since HTML output
   faces real browsers.
2. **Watcher debounce window.** BUILD-PLAN names "debounce" with no
   number: needs a concrete default (300-500ms is a reasonable start),
   whether it is config-exposed, and whether debounce coalesces per-file
   or globally (an edit storm across many files should probably trigger
   one rebuild pass, not N).
3. **Atomic staging-swap mechanics.** Confirm temp-dir-then-rename:
   render into `${state_dir}/html.tmp`, `rename()` over `${state_dir}/html`.
   Define behavior for partial trees (one bad file aborts the whole swap,
   or best-effort with that page skipped/stale?) and whether the HTTP
   listener needs a generation counter to avoid serving mid-swap on some
   filesystems.
4. **Incremental vs. full rebuild.** ADR 0004 names "incremental rebuild"
: does one file edit re-render just that file plus any index pages
   linking to it (for date/feed propagation), or the whole tree every
   time? Full-tree-every-time is simpler and matches the exit gate's
   "survives edit storms without torn output" framing but may not scale;
   decide the v1 answer.
5. **Date-convention choice** (from §4): gemsub-link-date-only vs. a
   usv-specific in-page convention: blocks both the metadata pass and
   the Atom emitter's `<updated>` field.
6. **Heading-as-title fallback chain** when no heading exists, 
   filename-derived vs. configured per-directory default vs. empty
   `<title>` (probably unacceptable for the "beautiful default skeleton"
   exit-gate goal).
7. **Preformat alt-text in HTML**: `<pre alt="...">`-equivalent (no real
   HTML analog), a caption above the block, or an ARIA label (recon flags
   "screen-reader treatment of preformatted alt text" as still-open
   upstream, gemini-text issue #12): usv controls only the emitter, not
   the convention; decide what is reasonable now knowing upstream may
   formalize this later.
8. **Theme/skeleton scope for the C3 exit gate.** "Beautiful default
   skeleton (placeholder-grade)": confirm one bundled theme suffices to
   pass the exit gate (the smolweb-checklist lynx/w3m pass only tests the
   *gemtext* side) versus needing the full bundled theme set before C3
   closes; BUILD-PLAN's theme *set* language suggests themes may be
   C3-adjacent but not gate-blocking.
9. **robots.txt mirroring, exact transform.** ADR 0004 says "mirrors a
   web robots.txt into the HTML output": static copy of an authored
   `robots.txt` if present, a synthesized default, or a transform of the
   Gemini-side robots.txt (different virtual-user-agent syntax per the
   companion spec) into standard web robots syntax? The two `robots.txt`
   conventions are not the same format: "mirroring" needs a concrete
   definition before code lands on it.

---

Prepared by a research pass over `docs/internal/BUILD-PLAN.md`, `docs/adr/0004-dual-surface.md`,
`docs/internal/recon/protocol.md`, `docs/internal/recon/prior-art.md` §1/§"Recommendation for ADR 0001",
`src/protocol/mod.rs`, `src/protocol/uri.rs`, `src/handler/mod.rs`.

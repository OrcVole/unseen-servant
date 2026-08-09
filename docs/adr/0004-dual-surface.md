# ADR 0004: Dual surface — one content tree, gemtext native, HTML statically rendered

- Status: Accepted (pending director review)
- Date: 2026-08-09
- Evidence: docs/recon/ecosystem.md §8, docs/recon/cloudron-fit.md §4, docs/recon/prior-art.md §4

## Context

The brief requires one content tree rendered twice: gemtext served
natively on 1965, HTML served on the HTTP surface so the Cloudron app
tile is alive (and, per ADR 0008, so standalone operators get a web
mirror if they want one). twins is the cautionary tale for the
alternative: live gemini→HTTP proxying multiplied its connection-
lifecycle states and produced most of its bug reports, including a
close_notify failure. Ecosystem recon adds that gemsub feeds are free
if generated index pages carry dated links, and that Atom matters
mainly on the HTML side (web feed readers don't speak gemsub).

## Decision

- One source tree: `${content_dir}` (`/app/data/content` on Cloudron).
  Authors write gemtext plus static assets. This tree is the single
  source of truth for both surfaces.
- **Gemtext is served natively** on the Gemini listener, files as-is
  (bare-LF files are legal on the wire; no conversion).
- **HTML is rendered statically at write time**: a watcher (with
  debounce) triggers an incremental rebuild into a separate output
  tree (`${state_dir}/html`); the HTTP listener serves only that
  output. The render pipeline is re-entrant (a Titan upload, if ever
  implemented per ADR 0006, is "just" another write event).
- **No live gemini-to-HTTP proxying, no open proxy surface** of any
  kind. The HTTP surface serves exactly the rendered tree plus a
  minimal status/landing affordance; it follows redirect/traversal
  rules as strictly as the Gemini side.
- The renderer's gemtext parser is the same fuzzed parser module used
  everywhere (ADR 0001); generated pages conform to the v0.24.1
  gemtext ABNF (`* ` with space, ≤3 heading levels, no unfenceable
  content).
- Shared CSS is minimal and **classless**, so rendered HTML stays
  semantic and the styling surface stays reviewable.
- Generated index pages emit **gemsub dated link lines**, making every
  capsule subscribable by CAPCOM/Antenna/Lagrange with zero extra
  code. **Atom generation ships in v1.0** (amended 2026-08-09,
  director: "definitely support Atom"): the metadata pass emits
  `atom.xml` for both surfaces — web feed readers don't speak gemsub,
  and the Gemini side serves it too for Atom-preferring clients.
- The HTML surface supports **themes**: a classless-CSS theme slot
  selected in config from a bundled, vetted set, plus a custom-CSS
  path. Because the CSS is classless (semantic HTML only), a theme is
  exactly one stylesheet — no template coupling. Gemtext itself is
  never themed server-side: on Gemini, presentation belongs to the
  reader's client by design, and the docs say so.
- The renderer mirrors a web `robots.txt` into the HTML output
  (ecosystem.md §4): the Gemini-side robots.txt does not govern web
  crawlers, and the docs say so.
- The HTTP surface starts unconditionally and serves `/` with 2xx
  regardless of Gemini-listener state (Cloudron health-check contract;
  harmless standalone).

## Consequences

- Two static file servers and one build pipeline — no request-time
  coupling between surfaces, so a bug in one cannot corrupt the other.
- Write-time rendering costs disk (a second tree) and a watcher task;
  in exchange the HTTP path has zero Gemini-protocol attack surface.
- Content authors get web reach without doing anything; feed
  subscribers on both networks get feeds without the author knowing
  what a feed is.

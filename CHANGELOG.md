---
title: "Changelog"
description: "What changed, per release. Before v1.0.0 there are no releases, so everything sits under Unreleased."
type: reference
status: decided
last_verified: 2026-08-30
---

# Changelog

Notable changes, newest first. Dates are the day the work landed on
`main`, not the day it was written.

`usv` follows semantic versioning from v1.0.0: a MAJOR bump is the only
thing that may change configuration semantics or the certificate
lifecycle (`UPGRADING.md` states that as a promise, and the reserved
`[titan]`/`[responses]` sections exist so a newer config fails loudly on
an older binary rather than silently doing less).

Per `MAINTENANCE.md`, this project's posture is *finished software,
actively watched*: a release that is only dependency bumps is a normal
release, not a sign of neglect.

## Unreleased

Everything below is on `main` and has never been tagged. There are no
published packages and no announcement.

### Fixed

- **Every internal link on the web mirror was a 404** (2026-08-30). A
  capsule links `=> about.gmi`; the render pass writes `about.html`
  beside `about.md`; both emitters copied the source's name through
  unchanged. Found by fetching the live site, not by reading the code —
  the markup was valid and the hrefs were well-formed, which is why no
  test on emitter output caught it. The rule now lives once in
  `render::links` and both emitters use it; absolute URLs are left
  alone, so a `gemini://` link on the web mirror stays a pointer at the
  other surface. The regression test renders a real tree and follows
  every link to a file on disk.
- **The Cloudron package could not be installed with its own defaults**
  (2026-08-30). `SPARTAN_PORT` defaulted to 3000, which the platform
  reserves and refuses outright. Now 3300. The failure was invisible
  from the live deployment, which had the port set by hand.
- **The decision guide advertised addresses that answer nothing**
  (2026-08-30). `docs/choosing.html` named canonical ports against our
  own hostname — gopher `:70`, spartan `:300`, finger `:79` — all of
  which the platform refuses. Corrected to the ports the capsule
  actually serves.

### Added

- **`cathode` theme**: the green CRT, carrying the glow sampled from the
  project's own mark. Always dark, contrast measured.
- **`usv check` reports an unlinked colophon.** `/usv` is generated on
  every protocol but only the default skeleton links it, so a capsule
  built from existing content served a page nobody could find. The check
  says so once, and only when there are pages and none link it. Noted,
  never auto-inserted: an operator's writing is theirs.

### Documented

- How to update an install made from an image, and why `--image` alone
  cannot do it (`docs/deployment/cloudron.md`).
- That `cloudron push` nests a directory rather than merging it, which
  looks exactly like a broken render and is not one.

### Verified, not changed

- The full estate gate ladder against the shipping image, on a throwaway
  install: install and first-run, no auth to break, real content flows,
  update and backup/restore with the certificate fingerprint unchanged
  across both, and memory under load (peak 6.3 MiB against a 256 MiB
  limit, no OOM events). Evidence is in the round's gate record.
- Eight fuzz targets over the request line, URI validation, config,
  gemtext, Titan framing, gopher selectors, path sanitisation and HTML
  emission. No crashes.

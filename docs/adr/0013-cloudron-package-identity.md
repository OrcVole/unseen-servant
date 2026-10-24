---
title: "ADR 0013: The Cloudron package identity, and a clean restart under it"
description: "The Cloudron app id is io.github.orcvole.unseenservant; the first listing is withdrawn, not migrated, and the repository history was rewritten once to match."
type: explanation
status: decided
last_verified: 2026-10-24
---

# ADR 0013: The Cloudron package identity, and a clean restart under it

- Status: **Accepted** (director, 2026-10-24)
- Date: 2026-10-24
- Evidence: director 2026-10-24 ("we want to do this cleanly and are happy to delete the ca listing and stop supporting any existing users"); Cloudron box `src/apps.js` (an update whose manifest id differs is refused unless forced) and `src/updater.js` (automatic updates never force); ADR 0008 (Cloudron is a deployment profile)

## Context

Cloudron keys a package on its manifest `id`: the community directory
page, the versions feed and every install carry it, and an install can
never change it. The first published package (v1.0.0 and v1.0.1) used an
id that nobody chose on purpose. A recon note suggested it as a
placeholder and the first packaging commit used it as written. It was
derived from a personal domain unrelated to the project, and the feed
carried a personal contact address beside it. Every other package from
the same packager uses `io.github.orcvole.<slug>`.

Changing the id is not a rename. An install on the old id cannot follow
a feed whose entries carry the new one, because the box refuses an update
across ids unless an operator forces it, and automatic updates never do.

## Decision

1. **The id is `io.github.orcvole.unseenservant`.** The slug drops the
   hyphen, as other packages from the same packager do.
2. **`author` stays Wandering Monster; `packagerName` is OrcVole.** The
   software keeps its own name; the directory groups the package with the
   packager's others. The contact address is the packager's.
3. **Clean restart, not a migration.** The first listing is withdrawn.
   Installs on the old id are not supported: the feed starts again empty
   and its first entry is v1.1.0. Because of the id check above, those
   installs stay where they are rather than being updated into a package
   they did not install.
4. **The repository history was rewritten once, on 2026-10-24,** to
   replace the old id and the personal address in every commit. Commit
   hashes changed; the v1.0.0 and v1.0.1 tags were re-created on the
   rewritten commits with their original messages and dates. Read in
   history, the manifest therefore shows the new id from the start: it
   did not ship that way, and this ADR is the record that it did not.
   The same applies to `docs/internal/recon/cloudron-fit.md`, whose
   suggested id now reads as the new one.

## Consequences

- Anyone who installed v1.0.x from the first listing must reinstall from
  the new one and copy `/app/data` across to keep their content and
  certificates.
- The versions feed keeps its URL, so the install pages that quote it
  (`content/run.gmi`, `content/artifacts.gmi`) do not change.
- v1.0.0 and v1.0.1 survive as tags only. Their release artefacts are
  withdrawn with the first listing and not rebuilt: a rebuild would not
  be what shipped. v1.1.0 is the first release with artefacts under
  this identity.
- A rewrite like this is a one-off. From here the history is append-only
  again, like the feed.

---
title: "Releasing usv"
description: "The ordered procedure for cutting a release, and the traps that have actually bitten. Publishing is deploying: nothing leaves this machine without the operator saying so."
type: how-to
status: decided
last_verified: 2026-08-30
---

# Releasing

`usv` has never been released. This document is written before the
first one so the steps are decided while nothing is at stake.

**The standing rule, above everything here:** publishing a version to a
Cloudron versions feed **deploys it**, immediately, to every install
with auto-update on, including strangers'. Feed entries are effectively
permanent — removing one strands every install still running it. Nothing
is published without the operator's instruction in writing.

## Before anything

1. `docs/internal/BUILD-PLAN.md`'s C7 gates pass, including the
   conformance run and the fuzz corpus.
2. The estate gate ladder (`estate/starter/gate-ladder/`) passes against
   the **exact digest that will ship**, on a throwaway install on the
   proving grounds. Gates passed by a different digest prove nothing
   about this one, so build first, then gate, then release that image.
3. `test/secret-scan.sh` and a host-detail sweep are clean.
4. The launch pack's claim gate (`docs/internal/launch/README.md`) has
   been walked *that day*, against the code rather than memory.

## Where the version is written by hand

Everything else derives it. Change these together:

| File | Field |
|---|---|
| `Cargo.toml` | `version` — the source of truth; `CARGO_PKG_VERSION` follows it into the binary, the identity module and the gopher renderer |
| `CloudronManifest.json` | `version` |
| `flake.nix` | `version` |
| `SECURITY.md` | the support table, which currently promises to become real at v1.0 |
| `CHANGELOG.md` | fold `Unreleased` into the new version with its date |

`packaging/deb/control` and `packaging/rpm/usv.spec` use placeholders
(`__VERSION__`, `%{_usv_version}`) and need no edit. `packaging/aur/PKGBUILD`
computes `pkgver()` from git. `docs/choosing.html` contains a version
inside an illustrative directory listing — decoration, not a claim.

## The order

1. Bump the versions above; commit.
2. Push, and let CI build and push the image. **Wait for the registry
   push to finish** — it takes 10 to 15 minutes, and `skopeo inspect`
   answering is not sufficient: it sees the manifest while layers are
   still uploading. The workflow's own completion is the signal.
3. Record the digest (`skopeo inspect docker://<tag>`), not just the tag.
   Everything after this cites the digest.
4. Run the gate ladder against that digest.
5. Tag `v<version>` and push the tag.
6. Build artefacts: `dist build` (musl tarball), `.deb`, RPM, OCI.
   Sign with `minisign` and publish sums plus signatures on the Forgejo
   release.
7. Only then, and only on the operator's written instruction, the
   Cloudron versions feed.

## Traps, each paid for

- **`cloudron update --image <ref>` cannot update an image-installed app
  on its own.** Without a manifest in the working directory it fails
  with `No CloudronManifest.json found`; with one, the manifest's
  `dockerImage` beats the flag while printing `(from app store)`. Use a
  deploy directory holding a copy of the manifest with `dockerImage` set,
  plus the three files it references by `file://` (`DESCRIPTION.md`,
  `POSTINSTALL.md`, `icon.png`). See `docs/deployment/cloudron.md`.
- **Actions secrets do not follow a repository swap or rename.** The
  first build after one fails at `podman login` with an empty password,
  which reads as a revoked token and is not.
- **A default install is not what a hand-tuned production install
  proves.** `SPARTAN_PORT` shipped a default the platform refuses for
  months while production ran happily on a hand-set value. Install once
  with defaults, as a stranger would, before every release.
- **`cloudron push` nests a directory rather than merging it**, so
  content can land a level deeper than intended and every expected
  address 404s while the render is working perfectly.

## After

Stamp `MAINTENANCE.md`'s review date, open the issue tracker if this is
the first public release, and set the quarterly heartbeat. The
announcement wave is a separate decision with its own sequencing in
`docs/internal/launch/README.md`; a release is not an announcement.

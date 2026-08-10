# Deploying on Cloudron

Cloudron is the profile `usv` was packaged for first, but it is a target,
not a dependency (ADR 0008) — nothing here is required to run `usv`
elsewhere.

## Installing

There is no App Store listing yet (pre-release). Install from a built
image:

```sh
cloudron install --image <registry>/<owner>/unseen-servant:<tag> -l <subdomain>
```

Building that image is a plain `podman build .` / `docker build .` at the
repository root. `.forgejo/workflows/build-cloudron-image.yml` does it in
CI and pushes the result, so the Cloudron host only ever pulls a finished
image rather than compiling ~130 crates during the install.

**Why that matters:** installing from source on a busy host took ~41
minutes in a real measurement. A prebuilt image on the default
`cloudron/base` took ~26 minutes, almost all of it transferring a 2.46GB
image whose payload was an 8.85MB binary. The current Alpine-based image
is **20.1MB** and installs in about **1m40s**. See
`docs/recon/cloudron-fit.md`'s 2026-08-10 addendum for the full
measurement and the base-image reasoning.

## What the package sets up

| Thing | Value |
|---|---|
| Gemini port | 1965, fixed (`readOnly` in the manifest) |
| HTTP port (internal) | 8000, proxied by Cloudron's nginx |
| Health check | `/` on the HTTP surface, always 2xx |
| Persistent data | `/app/data` (`localstorage` addon) — backed up |
| Content | `/app/data/content` |
| Identity | `/app/data/certs/<hostname>/` |
| Runs as | the unprivileged `cloudron` user |

The Gemini port is pinned deliberately. Gemini clients assume 1965; a
capsule on another port is only reachable via an explicit
`gemini://host:PORT/` URL, which breaks discovery and aggregators.

## Editing content

Use the **Files** icon on the app's tile and edit `content/`. Saving
triggers a re-render of both surfaces within seconds — the file watcher
is debounced and swaps the rendered tree atomically, so a reader never
sees a half-written site.

`cloudron exec` gives you a shell with the full `usv` CLI (`usv status`,
`usv fingerprint`, `usv check` — all read-only).

## Multiple hostnames

The manifest sets `multiDomain: true`. Alias domains added in the
Cloudron panel are injected as `CLOUDRON_ALIAS_DOMAINS`, and `usv` serves
them by SNI from the single instance, each with its own certificate.

One caveat found live: `start.sh` only forces `USV_HOSTNAME` when there
is no `/app/data/usv.toml`. Once you write a `usv.toml` with a `[[host]]`
block per hostname, the platform stops overriding it and your file wins.

## Backups, restores and moves

`/app/data` is included in Cloudron's backups, so the TOFU identity
survives updates, restores, and moves. A restore reverts the app code to
the version that was running when the backup was made.

Moving to a new domain is safe: `usv` detects the hostname change and
mints a fresh identity for the new name rather than silently reusing the
old one (which would look like impersonation to any reader who had pinned
it). The old keypair is kept, untouched.

## One capsule per host on 1965

An external TCP port binds once per Cloudron server. A second `usv`
install requesting 1965 fails with `409 Conflicting tcp port`. This is a
platform constraint, not a `usv` one — use `multiDomain` + SNI for
multiple capsules rather than a second install.

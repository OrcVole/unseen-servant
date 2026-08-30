---
title: "Deploying on Cloudron"
description: "Cloudron is the profile usv was packaged for first, but it is a target, not a dependency (ADR 0008): nothing here is required to run usv elsewhere."
type: howto
status: decided
last_verified: 2026-08-11
---

# Deploying on Cloudron

Cloudron is the profile `usv` was packaged for first, but it is a target,
not a dependency (ADR 0008): nothing here is required to run `usv`
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
`docs/internal/recon/cloudron-fit.md`'s 2026-08-10 addendum for the full
measurement and the base-image reasoning.

## What the package sets up

| Thing | Value |
|---|---|
| Gemini port | 1965, fixed (`readOnly` in the manifest) |
| Gopher port | optional, **off by default**, default 7070 |
| HTTP port (internal) | 8000, proxied by Cloudron's nginx |
| Health check | `/` on the HTTP surface, always 2xx |
| Persistent data | `/app/data` (`localstorage` addon): backed up |
| Content | `/app/data/content` |
| Identity | `/app/data/certs/<hostname>/` |
| Runs as | the unprivileged `cloudron` user |

The Gemini port is pinned deliberately. Gemini clients assume 1965; a
capsule on another port is only reachable via an explicit
`gemini://host:PORT/` URL, which breaks discovery and aggregators.

## Gopher, and why not port 70

The gopher service is declared but disabled until an admin enables it,
switching on a cleartext protocol is a decision an operator makes, not
one they inherit (ADR 0012 §2).

**Cloudron will not allocate a privileged port through `tcpPorts`.**
Found live on 2026-08-10: requesting external port 70 is rejected with
`70 for GOPHER_PORT is not in permitted range in ports`. Gopher clients
assume port 70 when none is given, so a Cloudron-hosted gopher hole is
necessarily reached with an explicit port,
`gopher://your.capsule:7070/`, and links to it must carry that port.
The same constraint does not bite Gemini only because 1965 is already
above 1024.

`advertised_port` exists for exactly this: the container binds 7070 and
menus advertise whatever external port Cloudron allocated, since gopher
menu lines carry an absolute host and port.

## Finger, and why not port 79

Finger's conventional port is 79, which is privileged and therefore
refused by the platform for the same reason as Gopher's 70 above. The
package publishes 7979. Every multi-protocol client takes a port, and
the profile advertises its own address; only the bare `finger user@host`
command assumes 79. A redirect from 79 to 7979 on the host was assessed
on 2026-08-11 and **deliberately not done** (director, 2026-08-30): an
`iptables` rule does not survive the platform, which rewrites
`nat PREROUTING` on every container recreation, and a `systemd` socket
unit on the host would be the way if it were ever wanted. It is not.

## Editing content

Use the **Files** icon on the app's tile and edit `content/`. Saving
triggers a re-render of both surfaces within seconds: the file watcher
is debounced and swaps the rendered tree atomically, so a reader never
sees a half-written site.

`cloudron exec` gives you a shell with the full `usv` CLI (`usv status`,
`usv fingerprint`, `usv check`: all read-only).

## Multiple hostnames

The manifest sets `multiDomain: true`. Alias domains added in the
Cloudron panel are injected as `CLOUDRON_ALIAS_DOMAINS`, and `usv` serves
them by SNI from the single instance, each with its own certificate.

One caveat found live: `start.sh` only forces `USV_HOSTNAME` when there
is no `/app/data/usv.toml`. Once you write a `usv.toml` with a `[[host]]`
block per hostname, the platform stops overriding it and your file wins.

## Updating an install that was made from an image

`cloudron update --app <id> --image <ref>` **does not work on its own**
for an app installed from an image rather than from the App Store. Two
recorded traps meet here, and neither one covers this case alone:

- Run it from a directory with no `CloudronManifest.json` and it fails
  with `App update error: No CloudronManifest.json found`. The `--image`
  flag is not enough on its own: the CLI wants a manifest to update
  *from*.
- Run it from a directory that has one, and the manifest's `dockerImage`
  field wins over the flag; the CLI prints `Using image <ref> (from app
  store)`, which names the manifest's image, not the flag's, and the
  "from app store" wording is misleading for an image that came from
  anywhere else.

So the image must be named in a manifest. Since this repository's
`CloudronManifest.json` deliberately carries **no** `dockerImage` (a
plain `cloudron install` builds from the `Dockerfile` here), the way to
update a running install is a small deploy directory beside the
repository rather than an edit to it:

```sh
mkdir deploy && cd deploy
# the manifest, with dockerImage set to the tag you want
python3 -c "import json; m=json.load(open('../CloudronManifest.json')); \
  m['dockerImage']='<registry>/<owner>/unseen-servant:<tag>'; \
  json.dump(m, open('CloudronManifest.json','w'), indent=2)"
# every file:// the manifest references, resolved relative to it
cp ../DESCRIPTION.md ../POSTINSTALL.md ../icon.png .
cloudron --server <box> update --app <id>
```

The three copied files are not optional: the CLI resolves
`file://DESCRIPTION.md`, `file://POSTINSTALL.md` and `file://icon.png`
relative to the manifest and refuses the update if any is missing.

Read the `Using image` line it prints. It is the only place the CLI says
which image it actually chose, and on this path it is reporting the
manifest, not your flag.

Found on 2026-08-30, deploying the link fix.

**Correction, same day.** That paragraph first claimed the finding was
new. Half of it was already written down: `estate/FLEET.md` records
"Store apps update with `--appstore-id <id>`; without it the CLI looks
for a local manifest and fails with 'No CloudronManifest.json found'".
It was read *after* hitting the symptom rather than before, which is the
one thing the estate's field guide asks you not to do. What is genuinely
added here is only the image-installed case and the deploy-directory
recipe, including the three `file://` companions. The error is kept
visible because the class matters more than the fix: consult the
doctrine before the box, not after it.

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
platform constraint, not a `usv` one: use `multiDomain` + SNI for
multiple capsules rather than a second install.

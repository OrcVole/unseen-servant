# Cloudron Fit: Constraints Recon for Unseen Servant (usv)

**Phase 0, item 4.** Researched 2026-08-09 against the live Cloudron documentation (docs.cloudron.io), the Cloudron forum, and the house `cloudron-app-packaging` skill. This document is the single source for writing `CloudronManifest.json` and the cert-lifecycle ADR. Everything stated here was verified against a cited source on the access date; where the house skill and the live docs disagreed, the live docs win and the disagreement is noted.

## Summary (2026-08-09)

Cloudron can host a Gemini server cleanly: `tcpPorts` exposes raw TCP with the external port injected as an env var, `localstorage` gives a persistent, backed-up `/app/data` for the TOFU keypair, the `tls` addon optionally hands the app Cloudron's own certificate for its domain, and `httpPort` gives a proxied HTTPS web surface so the dashboard tile is live. The two hard realities: an external TCP port binds **once per Cloudron host** (one capsule-on-1965 per server, no SNI routing at the platform layer), and the admin **can remap** 1965 away, which silently breaks default-port Gemini clients unless the package documents and discourages it. We are **not** the first Gemini app in the Cloudron community: Agate+, Atlas, Maple, Windmark, and molly-brown packages exist on the forum, but none has reached the official App Store, and their experience (especially Agate+) directly validates the design below.

## 1. tcpPorts: exposing raw TCP 1965

Semantics, per the manifest reference ([docs.cloudron.io/packaging/manifest/](https://docs.cloudron.io/packaging/manifest/)):

- `tcpPorts` is an object whose **keys are environment variable names** (alphanumeric + underscore). Each value is an object with `title`, `description`, `defaultValue`, `containerPort`, `portCount`, `readOnly`, and `enabledByDefault`.
- `defaultValue` is the **recommended external port** pre-filled in the install UI. `containerPort` is the port the app actually listens on inside the container; if omitted, external and internal ports are the same. `portCount` allocates N sequential ports (max 1000) and additionally exposes `VARNAME_COUNT`; we need `portCount: 1` (or omit it).
- **The env var contains the external, user-chosen port, not the container port.** Docs example (verbatim): "Should the user choose to expose the SSH server on port 6000, then the value of `SSH_PORT` is 6000." The docs state: "The Cloudron runtime will _bridge_ the user chosen external port with the app specific `containerPort`." The app must use the env var value when constructing user-visible URLs.
- **The port is user-remappable** at install time and afterwards: the user can accept the default, choose an alternate external port, or disable the service entirely, unless the manifest sets `readOnly: true`, which locks the port so the user cannot change it. When the user disables the service, the env var is **absent** at runtime: "Apps _must_ detect this on start up and disable these services."
- Reference manifest shape for usv:

```json
"tcpPorts": {
  "GEMINI_PORT": {
    "title": "Gemini Port",
    "description": "Public TCP port for the Gemini protocol. Gemini clients assume 1965; only change this if you know every client will specify the port explicitly.",
    "defaultValue": 1965,
    "containerPort": 1965,
    "readOnly": true
  }
}
```

**Consequence of remapping:** Gemini clients dial `gemini://host/` on 1965 by default; a capsule on any other port is only reachable via explicit `gemini://host:PORT/` URLs, which breaks discovery, aggregators, and most casual visits. Decision point for the manifest: set `readOnly: true` to pin 1965 (recommended: the port is part of the protocol's social contract), or leave it remappable and have the server read `GEMINI_PORT` at startup, advertise the actual port in the HTTP surface, and warn loudly in logs when it is not 1965. Either way the server must treat an **absent** `GEMINI_PORT` as "Gemini service disabled" and still start the HTTP surface so health checks pass.

**Conflict behavior:** installing a second app claiming an already-bound external port fails with `409 Conflicting tcp port` (see forum topic 14656). See section 5.

## 2. TLS: no proxy for Gemini; cert options

- **Cloudron's nginx cannot front Gemini.** Cloudron terminates TLS and reverse-proxies **HTTP only**: the manifest docs describe `httpPort` as the port "on which your app is listening for HTTP requests" and instruct apps to speak plain HTTP behind the proxy. Gemini is TLS-native but not HTTP, so it cannot ride that path.
- **tcpPorts get no TLS termination.** Verified: the docs describe tcpPorts purely as a port **bridge** from external port to `containerPort`; no TLS handling is mentioned anywhere for tcpPorts, and the Agate+ packaging thread confirms in practice that the app itself must terminate TLS on 1965. **usv terminates its own TLS on the Gemini port. This is a hard requirement, not an option.**
- **The app CAN read Cloudron's certificate for its domain** via the `tls` addon: "The certificate and key are available as read-only files at `/etc/certs/tls_cert.pem` and `/etc/certs/tls_key.pem`", and "The app restarts automatically when the certificate is renewed" ([docs.cloudron.io/packaging/addons/](https://docs.cloudron.io/packaging/addons/)). These are the Cloudron-managed certs for the app's primary domain: Let us Encrypt when the Cloudron is configured that way (the default), otherwise whatever cert Cloudron provisions for that domain. Note: the Agate+ thread (2023-era) claimed LE certs were "inaccessible to apps"; that is contradicted by the current `tls` addon documentation: the addon is the sanctioned access path. Do not repeat their workaround for that reason.
- **TOFU vs CA-signed, record both options for the ADR:**
  - *Option A, self-signed TOFU (Gemini-native default):* usv generates a long-lived self-signed keypair on first run, stores it under `/app/data`, and never rotates it without operator action. Matches Gemini community expectations; clients pin the cert (TOFU). Survives everything `/app/data` survives (section 3). Downside: strict-CA clients (rare) warn once.
  - *Option B, Cloudron LE cert via `tls` addon:* CA-signed, so no first-visit warning, and some clients accept it silently. Downsides: certs rotate every ~60-90 days, and **rotation breaks clients that pinned the previous cert**: several Gemini clients treat any cert change within the pinned cert's validity as a possible MITM and alarm. Also couples capsule identity to Cloudron's cert lifecycle and forces app restarts on renewal.
  - Recommended posture (to be ratified in the ADR): default to Option A (TOFU keypair in `/app/data`), offer Option B as an explicit opt-in config flag that reads `/etc/certs/tls_key.pem`/`tls_cert.pem` when the `tls` addon is declared. Declaring the addon in the manifest is harmless when unused.

## 3. Filesystem contract and TOFU keypair survival

Contract, per the packaging cheat-sheet ([docs.cloudron.io/packaging/cheat-sheet/](https://docs.cloudron.io/packaging/cheat-sheet/)):

- "The app container has a read-only file system. Writing at runtime to any location other than those listed below produces an error." `/app/code` is the read-only app image content.
- Writable: `/tmp` ("cleaned up periodically"), `/run` ("runtime configuration and dynamic data… do not persist across app restarts"), and `/app/data` (requires the `localstorage` addon; "All contents in this folder are included in backups"). `/app/data` is **empty on first install**: "Files added to this path as part of the app's image (Dockerfile) will not be present."
- Runtime user: `start.sh` runs as root; drop to the `cloudron` user with `gosu cloudron:cloudron`, and `chown -R cloudron:cloudron /app/data` on every start because backup/restore can reset ownership.

**TOFU keypair placement:** the keypair MUST live under `/app/data` (suggested: `/app/data/identity/cert.pem` + `key.pem`, key mode 0600, owned by cloudron). Nothing else survives. Lifecycle trace, per [docs.cloudron.io/apps/](https://docs.cloudron.io/apps/) and [docs.cloudron.io/backups/](https://docs.cloudron.io/backups/):

| Operation | Effect on /app/data (and the TOFU keypair) |
|---|---|
| Restart / crash | Preserved. Only `/run` and `/tmp` are lost. |
| App update (new package version) | Preserved: updates replace `/app/code` only; Cloudron takes an automatic pre-update backup and rolls back on failure. |
| Restore from backup | Replaced with the backup's contents, **and the app code is reverted to the version running when the backup was made** ("Restoring will also revert the code to the version that was running when the backup was created"). Keypair returns exactly as backed up. |
| Clone | Copied: a clone is built from an app backup and is "an exact replica on another domain," so the clone **inherits the same TOFU keypair** on a different hostname. usv should detect hostname change vs. stored identity and offer regeneration, since a reused key on a new domain is a TOFU oddity (not a security break, but clients pin per-host). |
| Move / relocate to another domain | Preserved, "the location field can be changed at any time… No data loss." Same hostname-change caveat as clone: clients that pinned under the old hostname pin fresh under the new one; the keypair itself carries over. |
| Migrate to another Cloudron server | Preserved via backup/restore: "The restored server will be an exact clone of the old one." Keypair survives if migration goes through a backup. |
| Uninstall | **Destroyed**: "Uninstalling immediately removes all app data from the server." The keypair is only recoverable from a retained backup via App Import. Archive likewise removes data but pins the latest backup permanently. |
| Repair/rebuild (recovery mode) | `/app/data` preserved; only the container/image is rebuilt. |

Backups contain "only the database and app user data", `/app/code`, logs, and temp files are excluded, which is exactly right for us: the keypair is app user data.

## 4. httpPort: the HTTPS web surface and health check

- `httpPort` is a **required** manifest field: the "TCP port on which your app is listening for HTTP requests." Cloudron's nginx terminates HTTPS with the Cloudron-managed (normally Let us Encrypt) cert for the app's domain and proxies plain HTTP to this port. **Confirmed: usv can and must serve an HTML surface here**, the dashboard tile links to `https://<app-domain>/`, so this surface is what keeps the tile alive. The brief's requirement holds. Natural content: capsule landing page, a gemtext→HTML rendered mirror or at least an explainer with the `gemini://` URL, and status.
- `healthCheckPath` is **required**: Cloudron probes it over HTTP on `httpPort` and the app "must return 2xx HTTP status" or it is flagged unresponsive (and may be restarted). Convention: `"healthCheckPath": "/"`: fine for usv since `/` will serve the HTML surface. The health check must succeed **even when the Gemini port is disabled or not yet configured**, so the HTTP listener must start unconditionally and before/independently of the Gemini listener.
- Pick a non-privileged internal port, e.g. `"httpPort": 8000`.

## 5. One capsule per host; SNI and multi-domain

- **An external TCP port binds once per Cloudron server.** Port mapping is 1:1 (port → one container); a second install requesting the same external port fails with `409 Conflicting tcp port` (forum topic 14656; forum topic 14094 "tcpPort routing" confirms Cloudron routes HTTP by hostname but raw TCP **only by port**: there is no platform-level SNI routing for tcpPorts). Therefore: **one Gemini capsule on 1965 per Cloudron host.** A second usv instance could only take a nonstandard port, which section 1 explains is near-useless for Gemini. The Agate+ package hit exactly this and built an application-level proxy on the 1965 "master" instance to reach sibling instances: a workaround we should not need.
- **SNI inside the app is fine and is the right multi-capsule answer.** Gemini clients send SNI and every Gemini request line carries the full absolute URL, so a single usv process on 1965 can virtual-host many hostnames. Cloudron's model is app=primary domain, but the manifest's `multiDomain: true` sanctions extra hostnames: "this app can be assigned additional domains as aliases to the primary domain," injected as `CLOUDRON_ALIAS_DOMAINS`. Cloudron then manages DNS (and HTTP-side certs) for the aliases, and they all resolve to the host where 1965 is bound. So: **compatible with Cloudron's model**, one app, `multiDomain: true`, per-hostname capsule roots under `/app/data`, per-hostname TOFU certs minted by usv (SNI selects the cert). Caveat for Option B (section 2): the `tls` addon documents certs for the **primary** domain; alias-domain cert files via the addon are not documented, which is one more reason TOFU-per-hostname is the default.

## 6. Packaging mechanics

Verified against the cheat-sheet, manifest docs, and house skill:

- **Base image:** final Docker stage must be `cloudron/base:5.1.0@sha256:1c0666c9abe9e2090d33686826d4e97769b799124573118d41e0d7485135748e` (current per live docs 2026-08-09; the house skill's 5.0.0 pin is stale: use 5.1.0). Multi-stage is fine: build usv in a `rust:*` stage, copy the binary into the base-image stage at `/app/code/usv`. Platform tooling (file manager, web terminal, log viewer) depends on base-image utilities.
- **CMD/start.sh:** `CMD [ "/app/code/start.sh" ]`, script executable. Runs as root: `chown -R cloudron:cloudron /app/data`, do first-run init (generate TOFU keypair if absent, write marker file), then `exec gosu cloudron:cloudron /app/code/usv …`: `exec` is required so SIGTERM reaches the process. Single binary serving both listeners ⇒ **no supervisor needed** (Cloudron staff pushed Agate+ toward supervisord precisely because it backgrounded multiple processes; usv's one-process design sidesteps that review comment).
- **Logging:** stdout/stderr; the platform rotates and streams (`cloudron logs -f`). No log files.
- **memoryLimit:** default is 256 MB RAM+swap; a Rust binary fits comfortably, but set it explicitly (e.g. `268435456`) so the choice is deliberate.
- **Manifest fields the house convention requires:** `manifestVersion: 2`, `id` (suggest `win.alba.usv` or similar reverse-domain), `title`, `author`, `version` (semver, package version, separate from `upstreamVersion`), `healthCheckPath`, `httpPort`, `addons: { "localstorage": {} }` (+ `"tls": {}` for Option B), `tcpPorts` as in section 1, `memoryLimit`, `multiDomain: true`, plus store metadata (`tagline`, `description` via `file://DESCRIPTION.md`, `icon` 256×256, `postInstallMessage`, `minBoxVersion`, `packagerName`). Set `optionalSso: true` if usv needs no Cloudron user accounts (a static capsule does not).
- **Workflow:** `cloudron init` → `cloudron install` (uploads source, builds on server) → iterate with `cloudron update`; or build locally with `cloudron build` + `cloudron install --image`. Debug with `cloudron logs -f`, `cloudron exec`, `cloudron debug` (pauses app, read-write fs).
- **How Cloudron surfaces the app:** a dashboard tile linking to `https://<location>` (the httpPort surface: hence it must not be dead); the configured TCP port appears in the app's Location/Network settings UI where the admin can change or disable it (unless `readOnly`). `postInstallMessage` (supports `$CLOUDRON-APP-DOMAIN`) is the right place to print the `gemini://$CLOUDRON-APP-DOMAIN/` URL and a TOFU explainer.

## 7. Prior art: Gemini apps in the Cloudron ecosystem

**We are not first to the ecosystem, but no Gemini app is in the official App Store** (as of 2026-08-09; searches of docs, store-related forum threads, and the web surface only community packages). Community/forum packages:

- **Agate+** (Tim Considine, forum topic 14082; also topics 14046/14036), the closest prior art: dual-protocol (Gemini 1965 via tcpPorts + HTTP surface with gemtext→HTML rendering and an `/admin` editor). Lessons: platform has no SNI routing for TCP (they built a 1965 proxy for multi-instance); they used self-signed wildcard certs for TOFU after (incorrectly, per current docs) concluding LE certs were unreachable; Cloudron staff (Nebulon) review feedback demanded supervisord, gosu/cloudron user, `/run` logs, `set -e`, and called the dual-protocol design "a much better fit for Cloudron." Never published to the official store; lives as a community package (git.cloudron.io + Docker Hub `tcmbp132021/cloudron-agate-plus`).
- **Atlas** (topic 9051): full-featured gemlog server, community thread.
- **Maple** (topic 7823), **Windmark** (topic 8166), **molly-brown** (topic 5827, Go toolchain pinning pain): earlier community packaging efforts, none store-published.

What we learn: (a) the tcpPorts-1965 + httpPort dual-surface shape is validated and staff-endorsed; (b) the single-binary design removes their biggest review friction (process management); (c) the store slot for a polished Gemini server is still open: usv can be first **in the store** if we follow the staff review checklist above; (d) study Agate+'s repo before writing start.sh.

### 7.1 Agate+ addendum (deeper read of topic 14082, 2026-08-09)

Directly relevant detail on how Tim Considine's Agate+ implements the
dual surface, and where usv deliberately differs:

- **Conversion is live, per-request**: "I wrote my own node script to
  read the .gmi file … so the html view is generated on the fly in
  the app and served to a browser." Components: Agate (Gemini) + a
  Node HTTP server + supervisord to keep both alive. usv's write-time
  static render (ADR 0004) is the deliberate opposite: no request-
  time conversion surface, trivially cacheable, and the output tree
  is portable (OnionShare, mirrors).
- **The process count is the pain**: Cloudron staff conditioned
  publication on supervisord integration, and the thread's latest
  posts (July) show persistent health-check failures leaving the app
  stuck "Starting…". Validates two usv hard constraints: single
  binary (no supervisor), and the HTTP listener + healthCheckPath
  returning 2xx unconditionally, before and independent of
  everything else.
- **Certificates**: wildcard self-signed minted internally, chosen
  on the (now-outdated) belief that LE certs are unreachable from the
  container; multi-instance served by making the port-1965 install a
  reverse proxy to sibling installs on high ports: "a little bit
  creative". usv: per-hostname certs + SNI vhosting inside one
  instance (ADR 0003/0008), no proxy tier. (Note: the thread blames
  missing SNI on Gemini *clients*; per the spec recon that is
  inaccurate: clients MUST send SNI. The real gap is Cloudron's TCP
  layer routing by port only.)
- **`/admin` editor** (basic content UI with credentials in `.env`)
: usv refuses an authenticated web-admin surface (panel file
  manager now, Titan-with-certs later), which also removes the
  default-credentials class of review findings.
- For COMPARISON.md and launch: Agate+ remains community-only with
  unresolved production issues as of its July thread activity; Tim
  Considine is the person on the Cloudron forum most invested in this
  exact niche: engage the thread respectfully at announcement time.

## Hard constraints for the architecture

- [ ] usv terminates its own TLS on the Gemini port; Cloudron provides no TLS termination or proxying for tcpPorts.
- [ ] The manifest declares `tcpPorts.GEMINI_PORT` with `defaultValue: 1965` and `containerPort: 1965`, and usv binds the Gemini listener to the container port while advertising the port from the `GEMINI_PORT` env var.
- [ ] usv starts successfully and passes the health check when `GEMINI_PORT` is absent (service disabled by admin), running the HTTP surface only.
- [ ] The TOFU keypair is generated on first run into `/app/data` and is never stored in `/app/code`, `/run`, or `/tmp`.
- [ ] usv assumes `/app/data` is empty on first install and rebuilds all defaults idempotently at startup.
- [ ] All filesystem writes at runtime go only to `/app/data`, `/run`, or `/tmp`; the rootfs is read-only.
- [ ] start.sh chowns `/app/data` to cloudron:cloudron on every start and ends with `exec gosu cloudron:cloudron`.
- [ ] usv logs exclusively to stdout/stderr.
- [ ] The manifest declares `httpPort` and `healthCheckPath`, and the HTTP surface serves a live HTML page at `/` returning 2xx independent of Gemini-listener state.
- [ ] The final Docker stage provides a `cloudron` user at uid:gid 1000:1000, a gosu-equivalent privilege-drop tool, and `bash`, with the compiled binary copied into `/app/code`, not necessarily `cloudron/base` itself (see 2026-08-10 addendum: `alpine:3.22` + `su-exec` satisfies this at 20.1MB vs. 2.46GB, with no loss of platform contract or admin-terminal conformance).
- [ ] The design assumes at most one usv install per Cloudron host can own external port 1965; multi-capsule needs are met by `multiDomain: true` plus SNI virtual hosting inside the single instance, never by a second install.
- [ ] usv detects a primary-hostname change (move/clone) against its stored identity and surfaces a regenerate-or-keep choice rather than silently reusing the keypair.
- [ ] If the CA-cert option is enabled, usv reads `/etc/certs/tls_cert.pem` and `/etc/certs/tls_key.pem` (read-only, `tls` addon) and tolerates restarts on renewal; TOFU self-signed remains the default because LE rotation breaks client cert pinning.
- [ ] The manifest declares `localstorage` (and `tls` if Option B ships), `manifestVersion: 2`, `memoryLimit` explicitly, and `optionalSso: true`.

## Addendum: base image revised to alpine (2026-08-10)

Found live, deploying to a real Cloudron host (`example.com`): even a
**prebuilt** image (compiled in CI, installed via `cloudron install
--image`, so no server-side compile at all) still installed slowly,
because the image itself, final stage `cloudron/base:5.1.0`: is
**2.46GB**. Broken down via `podman history`: a 1.7GB `apt-get` layer
plus ~650MB of bundled Node.js/yq tooling, neither of which usv uses at
all. The actual payload, the usv binary, is 8.85MB. That is the real
bottleneck this addendum fixes: not compile time (solved by C6's CI
image-build pipeline) but transfer time, on both the CI→registry push
and the registry→Cloudron-host pull.

**§6's "final stage must be `cloudron/base:5.1.0`" bullet was wrong to
call a hard constraint.** Re-checked directly against the live docs
(docs.cloudron.io/packaging/cheat-sheet/, fetched 2026-08-10): there is
**no statement anywhere that apps must use `cloudron/base`**. It is a
convenience image with common language runtimes preinstalled for apps
that need them; the actual platform contract: read-only rootfs with
`/app/data`/`/run`/`/tmp` writable, `CLOUDRON_*` env vars, HTTP health
checks, stdout/stderr logging, addon file injection: is enforced by
the Cloudron *runtime*, not baked into that specific base image. The
house skill and this doc's original §6 both inherited an unverified
assumption from the Agate+ prior art without checking whether it was
actually required.

**What a base image needs to provide, empirically (verified by running
the resulting container, not just building it):** inspected
`cloudron/base:5.1.0` directly (`podman run cloudron/base:5.1.0 ...`)
to find what usv's own `start.sh` actually depends on: a `cloudron`
user at uid:gid 1000:1000, `gosu` for the root→cloudron privilege drop,
and `bash` to run the script. `/usr/local/cloudron_addons` exists but
is empty: nothing there usv's manifest (`localstorage`+`tls` addons
only) touches.

**Fix:** final stage is now `alpine:3.22` with `su-exec` (gosu's tiny
musl-friendly equivalent), `bash` (kept explicitly, not just alpine's
default `ash`, so the dashboard's web terminal / file manager, which
`exec bash` into the container: keep working; conformance with the
platform's admin conveniences was a deliberate call, not an oversight,
per director instruction 2026-08-10: "we want the application to be
cloudron conformant"), and `ca-certificates`. The build stage also
switched to the `x86_64-unknown-linux-musl` target (already proven in
`packaging/oci/Dockerfile`) so the binary itself needs no glibc.
`start.sh`'s only change is `gosu` → `su-exec` (identical CLI shape).
**Resulting final image: 20.1MB: a 122x reduction from 2.46GB.**

Verified empirically before touching the live production deployment again
(not just "it builds"): built locally, ran as a container with
`CLOUDRON_APP_DOMAIN`/`GEMINI_PORT` env vars set the way Cloudron sets
them, confirmed the HTTP surface returns 200, confirmed the Gemini
surface completes a TLS handshake and serves gemtext, confirmed the
process runs as `cloudron` (not root) after `start.sh`'s privilege
drop, confirmed `bash` is present and functional inside the running
container, and: specifically because §3's backup/restore table
documents that restores can reset `/app/data` ownership: manually
`chown -R root:root /app/data` then restarted the container and
confirmed `start.sh`'s unconditional `chown` on every start recovered
correct ownership and the app kept serving.

**Nothing about the platform contract changed**: same manifest, same
addons, same tcpPorts/httpPort/healthCheckPath, same filesystem rules,
same non-root execution, same logging. Only the base image did.

## Sources (all accessed 2026-08-09, addendum sources accessed 2026-08-10)

- https://docs.cloudron.io/packaging/manifest/: manifest fields; tcpPorts semantics, env var = external port, bridge to containerPort, disabled-port behavior, readOnly, portCount; httpPort/healthCheckPath/memoryLimit/multiDomain.
- https://docs.cloudron.io/packaging/addons/: tls addon cert paths and renewal restart; localstorage semantics.
- https://docs.cloudron.io/packaging/cheat-sheet/, read-only rootfs, writable dirs, cloudron user, logging, start.sh/exec, base image `cloudron/base:5.1.0@sha256:1c0666c…`.
- https://docs.cloudron.io/apps/: update/relocate "no data loss", uninstall removes all data, archive, App Import, recovery mode.
- https://docs.cloudron.io/backups/: backup contents (database + app user data only), restore reverts code to backup-time version, clone = replica from backup, whole-server migration = exact clone.
- https://forum.cloudron.io/topic/14082/agate-dual-protocol-server-to-serve-gemini-http-from-one-source/32, Agate+ packaging thread: no SNI routing for TCP, 1965 proxy workaround, TOFU wildcard certs, staff review checklist, store status.
- https://forum.cloudron.io/topic/14656/failed-to-install-app-409-message-conflicting-tcp-port-7473-409 conflicting tcp port on duplicate external port.
- https://forum.cloudron.io/topic/14094/tcpport-routing, TCP routed by port only, 1:1 mapping, no hostname routing.
- https://forum.cloudron.io/topic/14046/agate-a-simple-gemini-server, https://forum.cloudron.io/topic/14036/trying-to-package-agate-a-gemini-server: Agate packaging threads.
- https://forum.cloudron.io/topic/9051/atlas-on-cloudron-full-featured-gemini-protocol-self-hosted-server-for-gemlogs: Atlas.
- https://forum.cloudron.io/topic/7823/maple-on-cloudon-gemini-server: Maple.
- https://forum.cloudron.io/topic/8166/windmark-on-cloudron-gemini-protocol-server: Windmark.
- https://forum.cloudron.io/topic/5827/molly-brown-gemini-project-on-cloudron: molly-brown.
- House skill `cloudron-app-packaging` (local, /home/boat/.claude/skills/cloudron-app-packaging/): packaging conventions; its base-image pin (5.0.0) is superseded by the live docs' 5.1.0.
- https://docs.cloudron.io/packaging/cheat-sheet/, re-fetched 2026-08-10 specifically to check for a `cloudron/base` mandate: confirmed there is not one; it is a recommended convenience image, not a requirement.
- https://docs.cloudron.io/packaging/, re-fetched 2026-08-10: no formal "conformance"/quality/validation checklist page exists; the manifest + cheat-sheet technical contract is the whole of it.

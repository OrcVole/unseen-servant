# How usv compares

Geminispace already has good servers. This document says honestly where
usv fits among them, and — more importantly — where it doesn't. If your
needs match one of the "choose the others" cases below, you'll be
happier running that server instead. Facts here are sourced from each
project's own docs/changelog/issue tracker as of 2026-08; see
`docs/recon/prior-art.md` and `docs/recon/ecosystem.md` for the full
research and citations this table draws from.

## At a glance

| | usv | Agate | gmid | Molly Brown | GmCapsule |
|---|---|---|---|---|---|
| Language | Rust | Rust | C | Go | Python |
| Scope | static + Titan, dual HTML surface | static only, by policy | static, FastCGI, proxy, vhosts | static, CGI/SCGI, cert zones | static, Titan, CGI, module extensions |
| Config | TOML, zero-config default | CLI flags only | custom (httpd-inspired) block syntax | TOML | TOML |
| Cert lifecycle | auto ECDSA per host, 4096 expiry, never regenerates | auto ECDSA per host, 4096 expiry (usv borrows this design) | user-provided or ACME | user-provided | user-provided |
| Multi-hostname | SNI vhosting, one process | per-hostname cert dirs | vhost blocks | absent (documented gap) | per-service config |
| Titan uploads | same-listener, cert-gated zones | no | delegates to FastCGI/proxy, not native | no | yes — the reference implementation |
| HTML mirror | built in, write-time static render | no | no | no | no |
| Dynamic content | no (by policy — see below) | no (by policy) | FastCGI/proxy | CGI/SCGI | CGI + native Python module API |
| Packaging | Cloudron app, .deb, RPM, AUR, Nix, OCI, static tarball | distro packages | distro packages | distro packages | pip/manual |
| Maintenance posture | pre-1.0 | "dependency bumps are the changelog" — feature-frozen, actively maintained | very active, multi-process privsep architecture | alive, low simmer | active, extension ecosystem (Bubble) |

## The four servers, briefly

**Agate** (Rust, mbrubeck) is the minimalist's minimalist: "Agate can
only serve static files," full stop — no CGI, no rewriting, no
scripting, by explicit and long-standing policy. Its certificate
lifecycle is the best in the field (auto-generated ECDSA per hostname,
never expires in practice, zero setup) and usv borrows that design
directly. What Agate doesn't do is exactly what usv adds: an HTML
mirror, Titan, and packaging beyond a bare binary.

**gmid** (C, omar-polo) is the maximalist and the field's most actively
developed server: FastCGI, reverse proxying, real vhost/location
config blocks, a companion Titan client, and a four-process
privilege-separation architecture (main/logger/server/crypto talking
over `imsg`) that is genuinely the security high-water mark for a
C server. It does not implement Titan natively — it validates and
delegates Titan requests to a FastCGI backend — and has no HTML
surface at all.

**Molly Brown** (Go, solderpunk — Gemini's creator) targets pubnix and
shared-hosting: `~username` capsules, world-readable-bit-as-publishing-
switch, CGI and SCGI, and **certificate zones** — path-scoped
SHA-256 fingerprint allowlists, "analogous to SSH's `authorized_keys`."
usv's own cert-zone design is a direct descendant of Molly Brown's.
Virtual hosting is a documented, longstanding gap.

**GmCapsule** (Python, skyjake) is the extensibility flagship and the
reference Titan implementation — GmCapsule's Titan handling (buffer
the full upload before dispatch, require a client certificate by
default, expose the fingerprint to handler code) is what usv's own
Titan design was checked against. Its real strength is a native Python
module API: Bubble, Geminispace's most successful interaction
platform (subspaces, feeds, issue-tracker mode), runs as a GmCapsule
extension module rather than a separate daemon — proof that
"interaction wants to be a server module," not a bolt-on service.

## When the others are the better choice

This is the part marketing copy leaves out. Pick the other server if:

- **You want the simplest possible thing that will never grow a
  feature.** Choose **Agate**. Its scope-freeze is a feature, not a
  limitation — if static-only, forever, is what you want, Agate has
  already made every future decision for you. usv will always have
  more moving parts (Titan, the HTML surface, packaging surface)
  because it does more.
- **You need FastCGI, reverse proxying, or serve many vhosts with a
  real block-structured config language.** Choose **gmid**. Nothing
  else in the field matches its config expressiveness or its
  privilege-separated hardening, and it's the most actively maintained
  server here by a wide margin. usv deliberately doesn't do proxying
  at all (see `docs/recon/prior-art.md`'s notes on twins, where
  proxying was the dominant source of bugs).
- **You're running a pubnix / shared multi-user host**, with many
  people publishing under one server via file permissions. Choose
  **Molly Brown**. Its `~username` model and world-readable-bit
  publishing switch are built for exactly that shape; usv is
  single-tenant by design (one operator, one Cloudron app instance).
- **You want to write real server-side logic in Python** — a custom
  Titan handler, a Bubble-style interaction module, anything beyond
  what a config file can express. Choose **GmCapsule**. usv has no
  extension API and isn't going to grow one; it renders one content
  tree to two protocols and stops there (ADR 0004/0005). If the thing
  you're building is a program, not a capsule, GmCapsule's module
  system is the right foundation, not usv.

## Where usv actually differs

Not better or worse across the board — different tradeoffs, made on
purpose:

- **Write-time rendering, not request-time.** usv renders the whole
  content tree to a static output tree on every content change (file
  watcher, debounced) and serves that tree — both the Gemini and HTML
  surfaces come from the same render pass. Every other server here
  either only speaks Gemini, or (Agate+, the closest prior art outside
  this table) converts gemtext to HTML per-request. Write-time
  rendering means the HTML surface is trivially cacheable and the
  output tree is portable on its own (it's what `usv export` hands you
  for OnionShare).
- **Titan lives on the same listener, cert-gated, from day one** — not
  delegated to a backend (gmid) and not a separate product concern.
  Same TLS, same port, scheme-dispatched.
- **The packaging surface is the actual point of usv existing.** None
  of the other four servers ship a Cloudron package; the 2021–2022
  Cloudron forum thread asking for a Molly Brown package was never
  fulfilled (`docs/recon/prior-art.md` §Summary). usv exists to answer
  that gap specifically, plus the broader "one binary, every distro"
  story (`.deb`, RPM, AUR, Nix, a plain OCI image, a static musl
  tarball) so a Gemini capsule doesn't require choosing a Linux distro
  first.
- **usv is pre-1.0.** Agate and gmid both have years of production
  hardening usv doesn't have yet. If uptime-critical production
  serving is the goal today, that maturity gap is real and worth
  weighing against usv's newer feature set.

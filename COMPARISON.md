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

## Choose usv when

Situations where `usv` is the right answer and the others are not.
Each is a real reason someone would pick it; if none of them describe
you, the next section probably does.

**You want to publish across the smolnet, not just Geminispace.** `usv`
serves Gemini, Titan, Gopher, Spartan, Nex, Finger and a web mirror from
one content tree — the only server here that does. Adding a protocol is
a config line, not a second site: the same folder is rendered to each,
so nothing has to be written twice or kept in sync. Every protocol has
been driven by a real client before being called supported
([`docs/protocols.md`](docs/protocols.md) names which one).

**You want AI agents to be first-class readers, not an afterthought.**
Most servers treat agents as a crawling nuisance to be managed. `usv`
treats legibility for agents and for assistive technology as the same
problem (ADR 0010) and serves the affordances by default: `/llms.txt`
lists every page for one fetch instead of a crawl, every page is
available as Markdown by suffix, there are machine-readable maps on both
surfaces, and the AI posture in `robots.txt` is permissive by doctrine
rather than hostile by reflex. Agents can also *write*: Titan uploads
and a cert-gated `/admin/status.gmi` are scoped by capability against a
managed identity roster (ADR 0011), so an agent gets exactly the reach
you granted it and no more.

**You want it written in Rust, and you want to check.** `usv` is pure
Rust with `unsafe_code = "forbid"` set crate-wide — not a convention,
a compiler error. Dependencies are pinned, `cargo-deny` runs in CI
alongside fuzz targets on every parser that touches the wire. The two
mature alternatives here are Rust too, so this is table stakes rather
than a differentiator; what is worth checking is the `forbid` and the
fuzzing, which are not universal.

**You want setup to be a conversation, not a config file.** `usv init`
walks through hostname, ports and protocols and writes a working
`usv.toml`; `--defaults` skips the questions entirely. A fresh capsule
serves a real page immediately rather than an empty directory.

**You want one piece of writing read by two audiences.** You like
gemtext and Geminispace, but you also want to put your address on a CV,
in an email footer, or in a toot — where most people cannot open a
`gemini://` link. `usv` renders one content tree to both, at write time,
from the same files. No second site, no static-site generator, nothing
to keep in sync. No other server in this table does this at all.

**You want to publish from your Gemini client.** Open the page in
Lagrange, "Edit Page with Titan", save. `usv` implements Titan natively
on the same listener with per-zone fingerprint allowlists. gmid
validates and hands off to a FastCGI backend you have to build;
GmCapsule does it natively but you are then running a Python
application. If you want to write without a shell, this is the shortest
path.

**You want to install it, not package it.** Cloudron app, `.deb`, RPM,
AUR `PKGBUILD`, Nix flake, an 8.77MB distroless container, a static
musl tarball, and a hardened systemd unit — all built and tested. The
Cloudron niche in particular has been open since a 2021 forum request
that nobody ever filled. If your objection to running a capsule has
been "I would have to figure out how to deploy it", this is the one
built to remove that.

**Your capsule's identity has to survive your infrastructure.** TOFU
means readers pin your certificate, and quietly replacing it looks
exactly like an impersonation. `usv` mints once per hostname and never
silently regenerates — through restarts, updates, backups, restores and
migrations — and treats damaged key material as a loud failure rather
than an excuse to make a new key. If you expect to move hosts, that
discipline is the difference between a warning your readers can trust
and one they learn to click through.

**You want the capsule to exist off the network too.** `usv export`
hands you a self-contained folder that works with no server behind it —
straight into OnionShare, onto a USB stick, into any static host. That
falls out of rendering at write time; a per-request converter has
nothing to hand you.

**You want to stop thinking about it.** No CGI, no FastCGI, no
scripting, no proxying, no plugin API, and no admin web UI — so no
credential to leak, no session to hijack, no default password, and no
script directory to audit. Everything is edited as files or over
authenticated Titan. Whole categories of upkeep are absent because the
feature that creates them was refused (ADR 0005).

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

# How usv compares

> Unseen Servant

Geminispace already has good servers. Facts below come from each project's
own docs and changelogs as of 2026-08; the research is in
[`docs/internal/recon/prior-art.md`](docs/internal/recon/prior-art.md).

## At a glance

| | usv | Agate | gmid | Molly Brown | GmCapsule |
|---|---|---|---|---|---|
| Language | Rust | Rust | C | Go | Python |
| Scope | smolnet + Titan + HTML mirror | static only, by policy | static, FastCGI, proxy, vhosts | static, CGI/SCGI, cert zones | static, Titan, CGI, modules |
| Config | TOML, zero-config default | CLI flags only | httpd-style blocks | TOML | TOML |
| Certificates | auto per host, never regenerates | auto per host (usv borrows this) | user-provided or ACME | user-provided | user-provided |
| Multi-hostname | SNI, one process | per-host cert dirs | vhost blocks | absent | per-service config |
| Titan | native, cert-gated zones | no | delegates to FastCGI | no | yes: the reference implementation |
| Other protocols | Gopher, Spartan, Nex, Finger, HTML | no | no | no | no |
| Dynamic content | no, by policy | no, by policy | FastCGI/proxy | CGI/SCGI | CGI + Python modules |
| Packaging | Cloudron, deb, RPM, AUR, Nix, OCI, tarball | distro packages | distro packages | distro packages | pip/manual |
| Maturity | v1.0.0, first release | feature-frozen, maintained | very active | alive, low simmer | active |

## The other four

**Agate** (Rust) serves static files and nothing else, by explicit
long-standing policy. Its certificate lifecycle is the best in the field,
and usv borrows that design directly.

**gmid** (C) is the maximalist and the most actively developed: FastCGI,
reverse proxying, real vhost config blocks, and a four-process
privilege-separation architecture that is the security high-water mark for a
C server. It validates Titan requests but delegates them to a FastCGI
backend.

**Molly Brown** (Go, by Gemini's creator) targets pubnix hosting:
`~username` capsules, the world-readable bit as a publishing switch, and
certificate zones: path-scoped fingerprint allowlists, which usv's own zone
design descends from. Virtual hosting is a longstanding gap.

**GmCapsule** (Python) is the reference Titan implementation and the
extensibility flagship. Bubble, Geminispace's most successful interaction
platform, runs as a GmCapsule module.

## Choose usv when

- **You want to publish across the smolnet, not just Geminispace.** One
  content tree serves Gemini, Titan, Gopher, Spartan, Nex, Finger and the
  web. Nothing else here does this.
- **You want one piece of writing read by two audiences**: people with a
  Gemini client, and people who can only open a link in a browser.
- **You want agents to be ordinary readers.** `/llms.txt`, a `.md` form of
  every page, machine-readable CLI (command-line interface) output,
  capability-scoped write access over Titan
  ([`docs/agents.md`](docs/agents.md)).
- **You want to publish from your Gemini client.** Titan is native, on the
  same listener, with per-zone allowlists.
- **Your capsule's identity has to survive your infrastructure.** Minted
  once per hostname, never silently regenerated, through restarts, restores
  and migrations.
- **You want the capsule to exist off the network too.** `usv export` hands
  you a folder that works with no server behind it: OnionShare, a USB
  stick, any static host.
- **You want to install it, not package it.** Cloudron, `.deb`, RPM (RPM
  Package Manager), AUR (Arch User Repository), Nix, an 8.77MB container, a
  static tarball. The Cloudron gap has been open since a 2021 forum request
  nobody filled.

## Choose something else when

- **You want the simplest possible thing that will never grow a feature**,
  **Agate**. usv will always have more moving parts.
- **You need FastCGI, reverse proxying, or many vhosts with a real config
  language**: **gmid**. usv does no proxying at all.
- **You are running a pubnix or shared multi-user host**: **Molly Brown**.
  usv is single-tenant by design.
- **You want to write server-side logic in Python**: **GmCapsule**. usv has
  no extension API (application programming interface) and is not growing
  one. If the thing you are building is a program rather than a capsule,
  GmCapsule is the right foundation.
- **You need production maturity today**: Agate and gmid have years of it.
  usv is at its first release and unaudited. Agate and gmid have
  years of production hardening it does not.

## Where usv actually differs

- **Write-time rendering, not request-time.** The whole tree renders on
  every content change and every surface is served from that one pass. The
  output tree is therefore portable on its own.
- **Titan on the same listener, cert-gated, from day one**, not delegated
  to a backend.
- **Packaging is the point.** No other server here ships a Cloudron package,
  and "one binary, every distro" is why usv exists.

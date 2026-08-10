<div align="center">

<img src="assets/mascot.png" alt="" width="140">

# Unseen Servant

**A security-first Gemini server that publishes one content tree to two worlds —
served natively as gemtext, and rendered to themed HTML for the web.**

`Rust` · `MIT` · **pre-release, unannounced** · [Gemini](https://geminiprotocol.net/) + Titan + web mirror

</div>

---

> **Pre-release. Please don't share this around yet.** Nothing links here
> on purpose: the project is gated on its own v1.0 quality bar. There are
> no tagged releases and no published packages — every install path below
> builds from source.

Write a gemlog once. Readers reach it from Lagrange, from lynx, or from
Chrome — whichever they happen to have open. There is no build step, no
deploy, and no second copy to keep in sync: you save a `.gmi` file and
both surfaces update within seconds, from the same source.

```
                    ┌─ gemtext ──────────────→ Gemini clients (1965)
content/*.gmi ─→ render ─┤
                    └─ HTML + Atom + gemsub ─→ the web
```

## Why you might want it

- **Two audiences, one folder.** The web mirror isn't a gateway or a
  proxy — it's the same content, statically rendered at write time. The
  output tree stands alone (`usv export` gives you a folder that works
  with no server behind it).
- **Identity that survives everything.** A TOFU certificate is minted
  once per hostname and never silently regenerated — not on restart,
  update, restore, or migration. Readers who pinned it stay undisturbed.
- **Closed by default.** No CGI, no scripting, no proxying, no plugin
  API, and no admin web UI to leave unauthenticated. Uploads require an
  explicit fingerprint allowlist; an empty one refuses to start.
- **Deploy it anywhere.** One static binary, packaged for Cloudron,
  Debian, Fedora/RHEL, Arch, Nix, and OCI — so running a capsule doesn't
  start with choosing a Linux distribution.

## Quick start

```sh
git clone <repository-url> unseen-servant && cd unseen-servant
cargo build --release
./target/release/usv
```

That's the whole setup. With no config file and an empty state
directory, `usv` mints an identity, writes a starter capsule, and serves
it — zero configuration is a supported configuration, not a degraded one.
Prefer to be asked questions? `usv init` runs a terminal wizard.

| Deploying properly | Guide |
|---|---|
| Cloudron | [`docs/deployment/cloudron.md`](docs/deployment/cloudron.md) |
| Debian / Ubuntu (`.deb`) | [`docs/deployment/debian.md`](docs/deployment/debian.md) |
| Fedora / RHEL / openSUSE (RPM) | [`docs/deployment/rpm.md`](docs/deployment/rpm.md) |
| Arch (AUR `PKGBUILD`) | [`docs/deployment/aur.md`](docs/deployment/aur.md) |
| Nix flake | [`docs/deployment/nix.md`](docs/deployment/nix.md) |
| Container (OCI, 8.77MB, distroless) | [`docs/deployment/container.md`](docs/deployment/container.md) |
| Source + systemd | [`docs/deployment/source.md`](docs/deployment/source.md) |

## Protocol support

Authoritative version, with the reasoning:
[`docs/protocols.md`](docs/protocols.md).

| Protocol | Status | Notes |
|---|---|---|
| **Gemini** | Supported | Port 1965, own TLS, TOFU identity |
| **Titan** | Supported | Same listener, client-certificate gated per zone |
| **Web (HTTP)** | Supported | Static HTML mirror of the same content tree |
| Gopher | **Planned — v1.1** | Not implemented |
| Spartan · Nex · Finger | **Planned — v1.1** | Not implemented |
| Anything else | Rejected | Answered `53`; `usv` is not a proxy |

Gopher, Spartan, Nex and Finger are designed and researched
(`docs/recon/smolnet.md`) and scheduled as optional, off-by-default
listeners for v1.1. **They are not written yet**, and nothing here will
describe them as supported until they exist and have been exercised
against a real client.

## Features

**Publishing** — gemtext → Gemini, HTML, Atom, gemsub, `sitemap.xml`,
`map.gmi`, Markdown, `llms.txt`. Four bundled themes. Debounced file
watcher with atomic staging-swap renders.

**Identity & access** — per-hostname TOFU certificates; SNI virtual
hosting from one process; certificate zones (path-scoped fingerprint
allowlists → `60`/`61`); a named identity roster with self-closing key
rotation windows.

**Titan uploads** — same listener, scheme-dispatched; per-zone
fingerprint or identity allowlists; size validated before the body is
read; re-entrant mutate-and-render.

**Operations** — one TOML file, all defaults working; SIGHUP reload
without dropping listeners (an invalid edit keeps the old config);
read-only CLI (`status`, `fingerprint`, `check`, `zones`, `stats`);
stdout/stderr logging only; `usv export` for offline/onion mirrors; Tor
and I2P friendly by design.

## Configuration

```toml
[server]
http_listen = "0.0.0.0:8000"
theme = "midnight"

[[host]]
name = "example.org"

[[host.titan_zone]]
path_prefix = "/uploads/"
fingerprints = ["sha256-hex…"]     # empty here is a startup error, never "anyone"
```

Full reference: [`docs/configuration.md`](docs/configuration.md).
Unknown keys are a startup error — a typo in a security-relevant setting
must not fail open by being ignored.

## When to use something else

The full, honest version is [`COMPARISON.md`](COMPARISON.md), including
where Agate, gmid, Molly Brown and GmCapsule each beat `usv`.

| If you want… | Use | Because |
|---|---|---|
| The simplest thing that will never grow a feature | **Agate** | Its scope-freeze is a feature; it has already made every future decision for you |
| FastCGI, proxying, many vhosts, a real config grammar | **gmid** | Unmatched config expressiveness and hardening; the most actively maintained of the field |
| A pubnix / shared multi-user host | **Molly Brown** | Built for per-user capsules; `usv` is single-tenant by design |
| To write real server-side logic in Python | **GmCapsule** | A genuine module API; `usv` has none and isn't growing one |
| Gemini + Titan + a web mirror + packaging that already exists | **`usv`** | That combination is the whole point |

`usv` is also pre-1.0. Agate and gmid have years of production hardening
it does not.

## Security

Posture, and what it deliberately doesn't protect you from:
[`docs/security.md`](docs/security.md). Reporting:
[`SECURITY.md`](SECURITY.md) — please don't open public issues for
vulnerabilities.

In short: `unsafe_code = "forbid"` project-wide, seven fuzzed parsers
with committed regression corpora, `cargo deny check` on every push, no
dynamic execution anywhere by design, one unprivileged process with an
empty capability bounding set. Not independently audited.

## Project

Written in Rust: ~10.9k lines of code plus ~2.9k of comments across 38
files, with 415 test functions ([`docs/architecture.md`](docs/architecture.md)
explains the measurement and why the comment ratio is deliberate).

Every real decision is recorded as an ADR in [`docs/adr/`](docs/adr/)
before it was built, and the research behind them lives in
[`docs/recon/`](docs/recon/) — including honest autopsies of the servers
above.

**AI Forward.** `usv` is written end to end by an AI, directed and
reviewed by a human, with the reasoning kept on the record rather than
only in commit messages. If that isn't something you want serving your
capsule, that's a fair call to make — which is why it's stated here
rather than left to be discovered.

| | |
|---|---|
| Roadmap & phases | [`docs/ROADMAP.md`](docs/ROADMAP.md), [`docs/BUILD-PLAN.md`](docs/BUILD-PLAN.md) |
| Contributing | [`CONTRIBUTING.md`](CONTRIBUTING.md) |
| Upgrades & TOFU survival | [`UPGRADING.md`](UPGRADING.md) |
| Maintenance posture | [`MAINTENANCE.md`](MAINTENANCE.md) |
| Debugging | [`DEBUGGING.md`](DEBUGGING.md) |

## License

MIT.

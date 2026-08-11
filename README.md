<div align="center">

<img src="assets/mascot.png" alt="" width="140">

# Unseen Servant

**A server for the small internet.**

`Rust` · `MIT` · pre-release, unannounced

</div>

---

> **Pre-release: please do not share this around yet.** There are no
> tagged releases and no published packages; every install path below
> builds from source.

**Unseen Servant** serves one folder of writing to five small
networks: Gemini, Gopher, Spartan, Nex and Finger, and mirrors it to the
web. You write a page once. A reader opens it in Lagrange, in a Gopher
client from 1994, in lynx, or in Chrome, and it is the same page, from the
same file, updated within seconds of you saving it.

```
                        ┌─ gemtext ────────→ Gemini · Spartan · Nex
content/*.gmi ─→ render ─┼─ menus ──────────→ Gopher
                        ├─ profile ────────→ Finger
                        └─ HTML + Markdown → the web
```

There is no build step, no deploy, no database, and no second copy to keep
in sync.

The command is **`usv`**: `usv init`, `usv status`, `usv render`. After
this first mention, the documentation writes Unseen Servant (usv).

## Where it answers

Unseen Servant (usv) publishes the same capsule at one name across every
network it serves:

| | |
|---|---|
| Web | <https://unseenservant.dev> |
| Gemini | `gemini://unseenservant.dev` |
| Gopher | `gopher://unseenservant.dev` |
| Spartan | `spartan://unseenservant.dev` |
| Nex | `nex://unseenservant.dev` |
| Finger | `finger://unseenservant.dev` |

**Not live yet.** The domain is registered and the deployment exists, but
the name has not been pointed at it and nothing is announced. These
addresses are the intended shape, not a claim that they resolve today.

## One server for five networks

Each of the small networks has its own character and its own reasons to
exist: Gopher's menus, Gemini's certainty, Spartan's plainness, Nex's
minimalism, Finger's single page about a person. `usv` speaks all five plus
a web mirror, from one content tree. Turning one on is a line of
configuration, not a second site to maintain.

[`docs/smolnets.md`](docs/smolnets.md) characterises each network and what
it is best at, so you can pick deliberately rather than enabling everything.

## Easy to deploy

Running a capsule should not begin with choosing a Linux distribution.

| Target | |
|---|---|
| Cloudron (one click, once published) | [`docs/deployment/cloudron.md`](docs/deployment/cloudron.md) |
| Debian / Ubuntu (`.deb`) | [`docs/deployment/debian.md`](docs/deployment/debian.md) |
| Fedora / RHEL / openSUSE (RPM, RPM Package Manager) | [`docs/deployment/rpm.md`](docs/deployment/rpm.md) |
| Arch (AUR, Arch User Repository) | [`docs/deployment/aur.md`](docs/deployment/aur.md) |
| Nix flake | [`docs/deployment/nix.md`](docs/deployment/nix.md) |
| Container (OCI, Open Container Initiative: 8.77MB, distroless) | [`docs/deployment/container.md`](docs/deployment/container.md) |
| Source + systemd | [`docs/deployment/source.md`](docs/deployment/source.md) |

It is one static binary with no runtime to install beside it. Every package
above was built and exercised through a real install, run and remove cycle
rather than only written.

```sh
git clone <repository-url> unseen-servant && cd unseen-servant
cargo build --release
./target/release/usv
```

That is the whole setup. With no configuration file and an empty state
directory, `usv` mints an identity, writes a starter capsule, and serves it.
Zero configuration is a supported configuration, not a degraded one. `usv
init` runs a terminal wizard if you would rather be asked questions.

## Security first, and why

A capsule is usually run by one person on a machine they also use for other
things. The threat that matters is not a targeted attack; it is the slow
accumulation of things that can go wrong unattended. So `usv` removes the
categories rather than defending them:

- **Nothing is executed, ever.** No CGI (Common Gateway Interface), no
  FastCGI, no scripting, no plugin interface, no proxying. Content is data.
  This is the single decision that removes the most risk: in the servers we
  studied, the escape hatch beyond static serving produced most of the
  defect load.
- **No admin web interface.** There is no credential to leak, no session to
  hijack, no default password, and no login page to find. You edit files, or
  you upload over an authenticated connection.
- **Memory safety is enforced, not assumed.** `unsafe_code = "forbid"` is
  set for the whole crate, so it is a compiler error rather than a
  code-review convention. Every parser that touches the wire is fuzzed, with
  committed regression corpora.
- **Fails closed.** An unknown key in the configuration file is a startup
  error, because a typo in a security-relevant setting must not be ignored
  into a permissive default. An upload zone with an empty allowlist refuses
  to start rather than meaning "anyone".
- **Identity survives the infrastructure.** The small internet uses TOFU
  (trust on first use): your reader's client pins your certificate the first
  time and warns if it changes. `usv` mints one per hostname and never
  silently regenerates it, not on restart, update, backup, restore or
  migration, and treats damaged key material as a loud failure rather than
  an excuse to make a new key.
- **Least privilege by default.** One unprivileged process, every listener
  on an unprivileged port, an empty capability bounding set in the shipped
  systemd unit.

The full posture, including what it deliberately does *not* protect you
from, is [`docs/security.md`](docs/security.md).

## AI agents are first-class users

Not tolerated as crawlers, and not served something different from what a
person gets. An agent may read a capsule, publish to one, or run one:

- **Reading.** `/llms.txt` gives the complete page inventory in one request
  instead of a crawl, and every page has a Markdown form at its own address,
  so there is no markup to strip. `robots.txt` is permissive by default.
- **Writing.** Publishing is a file write. Over the network that is Titan,
  on the same port as Gemini, authorised by client certificate against a
  named roster with capabilities and self-closing key-rotation windows. No
  accounts, no passwords, no tokens to expire.
- **Operating.** Every read-only subcommand takes `--json` for one
  machine-readable object per line; logs can be emitted as JSON (JavaScript
  Object Notation) with `USV_LOG_FORMAT=json`; exit codes are a documented
  contract the test suite checks.

Every one of those is also an accessibility or ordinary usability feature, 
a site map is a navigation aid, a Markdown page is a clean read,
machine-readable output is what any script wants, which is why they were
built and the agent-only ideas were not. [`docs/agents.md`](docs/agents.md)
is the manual.

## Built from what the best servers already got right

`usv` is not a first attempt at this problem. Each of the small networks has
a server that solved something well, and the design starts from those rather
than from scratch: Agate's certificate lifecycle, Molly Brown's certificate
zones, gmid's configuration semantics and its insistence on end-to-end tests
against real sockets, GmCapsule's Titan handling, gophernicus and
geomyidae's menu conventions.

What each one taught, and what `usv` deliberately declined to copy, is
[`docs/lineage.md`](docs/lineage.md). Where another server is the better
choice for you, [`COMPARISON.md`](COMPARISON.md) says so plainly.

## A codebase written to be debugged

Roughly one comment line per four lines of code, on purpose: the reasoning
behind a decision lives next to it rather than only in a commit message.
Every real decision is recorded as an ADR (architecture decision record) in
[`docs/adr/`](docs/adr/), written before the code it governs. Modules are
small and named for what they own.

That serves a human maintainer and an AI one equally. The wire test suite
runs the real binary against real sockets and prints the exact bytes
exchanged when something fails; rejections name the layer and the rule that
fired; `usv status --json` reports the server's whole state in one parseable
object. [`DEBUGGING.md`](DEBUGGING.md) is organised by symptom.

## Protocols

| Protocol | Verified against |
|---|---|
| **Gemini** | `gemini-diagnostics`, Lagrange |
| **Titan** | Lagrange |
| **Web (HTTP, HyperText Transfer Protocol)** | browsers, `lynx`, `w3m` |
| **Gopher** | gelim |
| **Spartan** | Lagrange |
| **Nex** | gelim |
| **Finger** | bombadillo |
| Anything else | Refused with status `53`; `usv` is not a proxy |

Nothing is described as supported here until a real client has driven it.
Everything but Gemini, Titan and the web mirror is off until you enable it:
the four cleartext protocols offer no confidentiality and no client
authentication, so gated paths are excluded from their trees structurally
rather than by a check somebody has to remember.
[`docs/protocols.md`](docs/protocols.md) is the authority.

## Configuration

One TOML (Tom's Obvious Minimal Language) file, and every default works.

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

Full reference: [`docs/configuration.md`](docs/configuration.md). Remote
editing over Titan: [`docs/titan.md`](docs/titan.md). Common questions:
[`docs/faq.md`](docs/faq.md). Everything else: [`docs/`](docs/index.md).

## Project

Written in Rust: about 11,000 lines of code across 40 files, with 622 tests
and a fuzz target for every parser. Not independently audited, and pre-1.0, 
Agate and gmid have years of production hardening it does not.

**AI Forward.** `usv` is written end to end by an AI, directed and reviewed
by a human. If that is not something you want serving your capsule, that is a
fair call to make.

| | |
|---|---|
| The small networks | [`docs/smolnets.md`](docs/smolnets.md) |
| What we learned from other servers | [`docs/lineage.md`](docs/lineage.md) |
| Agents | [`docs/agents.md`](docs/agents.md) · [`docs/agents.html`](docs/agents.html) |
| Architecture | [`docs/architecture.md`](docs/architecture.md) |
| Debugging | [`DEBUGGING.md`](DEBUGGING.md) |
| Contributing | [`CONTRIBUTING.md`](CONTRIBUTING.md) |
| Upgrades and identity survival | [`UPGRADING.md`](UPGRADING.md) |
| Reporting a vulnerability | [`SECURITY.md`](SECURITY.md) |

## License

MIT.

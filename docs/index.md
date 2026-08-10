# Documentation

Start at the [README](../README.md) if you haven't. This is the manual.

## Running it

| | |
|---|---|
| [`configuration.md`](configuration.md) | Every setting, the real TOML shape, environment overrides |
| [`titan.md`](titan.md) | Remote editing: zones, allowlists, key rotation |
| [`protocols.md`](protocols.md) | **What `usv` actually speaks** — the authority every other page defers to |
| [`faq.md`](faq.md) | The questions people ask first |
| [`security.md`](security.md) | The posture, and what it deliberately doesn't protect you from |

## Installing

| Target | |
|---|---|
| Cloudron | [`deployment/cloudron.md`](deployment/cloudron.md) |
| Debian / Ubuntu | [`deployment/debian.md`](deployment/debian.md) |
| Fedora / RHEL / openSUSE | [`deployment/rpm.md`](deployment/rpm.md) |
| Arch | [`deployment/aur.md`](deployment/aur.md) |
| Nix | [`deployment/nix.md`](deployment/nix.md) |
| Container (OCI) | [`deployment/container.md`](deployment/container.md) |
| Source + systemd | [`deployment/source.md`](deployment/source.md) |

## Understanding it

| | |
|---|---|
| [`architecture.md`](architecture.md) | The one idea, the module map, honest size figures |
| [`adr/`](adr/) | Every real decision, written before the code |
| [`recon/`](recon/) | The research behind them — protocol, prior art, ecosystem, Cloudron fit, smolnet |
| [`../COMPARISON.md`](../COMPARISON.md) | Against Agate, gmid, Molly Brown, GmCapsule — including when to use them instead |

## Project

| | |
|---|---|
| [`ROADMAP.md`](ROADMAP.md) · [`BUILD-PLAN.md`](BUILD-PLAN.md) | Where this is going and how it got here |
| [`OPEN-QUESTIONS.md`](OPEN-QUESTIONS.md) | Decisions still outstanding |
| [`launch/`](launch/) | Announcement drafts, unposted, with a pre-send claim gate |
| [`../MAINTENANCE.md`](../MAINTENANCE.md) | The "finished software, actively watched" posture |
| [`../UPGRADING.md`](../UPGRADING.md) | What survives an upgrade, and why the identity does |
| [`../DEBUGGING.md`](../DEBUGGING.md) | Conformance runs, diagnosis, known tool quirks |
| [`../CONTRIBUTING.md`](../CONTRIBUTING.md) | How to work on it |

## A note on how these are written

Where a document records something found by testing rather than by
reading, it says so, and usually says what broke. That is deliberate:
the failures are the part worth keeping — a build script that works is
uninformative, whereas *why* `makepkg` needed `unset LDFLAGS CFLAGS`
before `cargo` would link is knowledge that is otherwise expensive to
reacquire.

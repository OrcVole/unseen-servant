# Build plan — phased coding of usv

2026-08-09. The coding companion to ROADMAP.md: phases C0–C7 map onto
release milestones M1–M6. Each phase has an exit gate; a phase is not
left until its gate passes. Test-first throughout: the regress suite
and fuzz targets grow with each phase, never after it.

## C0 — Scaffold (→ M1)

Cargo workspace (`usv` binary crate + internal lib crates as modules
emerge), MIT LICENSE, rust-toolchain.toml (pinned stable), CI
(fmt, clippy -D warnings, test, cargo-audit, cargo-deny), cargo-fuzz
harness skeleton, `#![forbid(unsafe_code)]`, house doc stubs
(AGENTS.md, CLAUDE.md, CONTRIBUTING.md, DEBUGGING.md,
INTEGRATIONS.md, UPGRADING.md), regress-runner skeleton (spawns the
real binary against real sockets, gmid-style).
**Exit: CI green on the empty shell; `usv --version` works.**

## C1 — Wire core (→ M1)

Config loader (single TOML, deny-unknown-fields, env overrides,
reserved `[titan]`/`[responses]` sections erroring helpfully); strict
request parser (1024-byte URI + CRLF window, full reject-list —
userinfo, fragment, non-ASCII, bare LF — as a fuzzed module);
response writer (exact `XX SP META CRLF` emission rules); TLS layer
(rustls: SNI resolver, client-cert capture, TLS 1.3 default with 1.2
opt-in, session tickets off, close_notify on every path); identity
module (rcgen: per-hostname ECDSA, 4096-01-01, 0600 keys, PEM,
never-regenerate + hostname-change detection); listener task with
timeouts and connection caps; SIGHUP/SIGTERM discipline.
**Exit: all 27 gemini-diagnostics checks pass against a hello-world
handler; fuzz targets for parser + config run in CI.**

## C2 — Static serving (→ M1)

Handler trait (parsed request → status/meta/body-stream); static
file handler (traversal-proof canonicalization, MIME table incl.
gpub, streaming); redirect handler (regex, capture groups, single
hop); certificate zones (path-scoped 60/61/62 with SHA-256
fingerprint allowlists); per-request logging (single-line, query
redaction, stats counting hooks).
**Exit: full regress suite covering the diagnostics gaps — percent-
encoded/double-encoded traversal corpus, 6x flows incl. expired and
malformed certs, SNI vhost selection, slow-client/slowloris
timeouts, redirect behavior.**

## C3 — Dual render (→ M2)

Gemtext parser (the same fuzzed module everywhere); metadata pass
(titles, dates, feeds); HTML emitter (semantic, classless) + bundled
themes + docs gallery with sample screenshots; Atom + gemsub
emitters for both surfaces; watcher with debounce and atomic
staging-swap renders; HTTP listener (rendered-tree serve only,
unconditional start, `/` health, fingerprint/status page); beautiful
default skeleton (placeholder-grade); web robots.txt mirroring.
**Exit: sample capsule renders identically-structured on both
surfaces; smolweb-checklist pass in lynx and w3m; feeds validate;
watcher survives edit storms without torn output.**

## C4 — Titan (→ M3)

Per ADR 0006 + docs/recon/titan.md: scheme dispatch on the shared
listener, cert-required-before-body, three-point size enforcement,
token-as-second-factor only, serialized mutate-and-render, path
hygiene, per-zone MIME allowlist and quotas, `size=0` delete opt-in.
**Exit: live upload and page-edit round-trips against real Lagrange,
including the flagged unknown (mid-upload rejection behavior);
regress suite gains a Titan client harness.**

## C5 — Tooling (→ M4)

CLI subcommands: status, fingerprint, check (config + content lint),
zones, render --force, stats, export (OnionShare-ready folder).
`usv init` ratatui wizard (+ `--defaults`). Tor/I2P affordances:
advertised_host, onion-hostname cert slot, graceful no-SNI handling.
**Exit: wizard produces a working config from an empty directory;
export folder verified inside OnionShare website mode; onion
deployment recipe tested end-to-end.**

## C6 — Packaging (→ M5)

Cloudron folder authored from docs/recon/cloudron-fit.md (NOT from
the house skill templates — none exist, and the skill's base pin is
stale): multi-stage Dockerfile → `cloudron/base:5.1.0` digest-pinned
final stage, CloudronManifest.json per the hard-constraints
checklist, start.sh (chown, first-run init, exec gosu), icon,
DESCRIPTION.md, postInstallMessage. Rest of matrix: cargo-dist
static musl tarballs + systemd unit, .deb, AUR PKGBUILD, Nix flake,
plain OCI image.
**Exit: the proving-grounds experiment protocol (below) passes.**

## C7 — Hardening and launch prep (→ M6)

Extended fuzz campaigns (parser, gemtext, titan framing); audit/deny
clean; MAINTENANCE.md, UPGRADING.md (TOFU survival story),
COMPARISON.md, project capsule content; store submission per
publishing-community-apps conventions.
**Exit: THE hard gate — clean gemini-diagnostics run against the
deployed proving-grounds instance from an external vantage point,
plus stable fuzz corpus. Then the launch checklist (ROADMAP M6).**

## Proving-grounds experiment protocol (C6 gate)

Run on the house Cloudron ("proving grounds"); every experiment is
cloudron-CLI-driveable, so the suite is scriptable and repeatable.
Metrics come from Cloudron's per-app graphs (CPU/mem/disk), `cloudron
logs`, `usv stats`, and timing wrappers.

| # | Experiment | Pass criterion |
|---|---|---|
| E1 | Fresh install | Skeleton served on both surfaces; cert minted; tile live; postInstall message correct |
| E2 | Edit content via file manager | Re-render visible on both surfaces; measure edit→live latency |
| E3 | Package update (new version) | /app/data intact; **cert fingerprint unchanged** |
| E4 | Backup → restore | Fingerprint unchanged; content as backed up |
| E5 | Clone / move to new domain | Hostname-change detection fires; new cert minted; old keypair untouched |
| E6 | Disable Gemini port | App healthy, HTTP-only; re-enable restores 1965 |
| E7 | Load + memory | RSS under sustained requests within memoryLimit; no leak trend |
| E8 | Restart / SIGTERM | Graceful drain; clean logs; fast healthy |
| E9 | External gemini-diagnostics | 27/27 over the real network |
| E10 | multiDomain alias | SNI serves per-hostname certs on one port |

Needs from the director before C6: cloudron CLI access to the
proving grounds (`cloudron login` on this machine, or an API token),
and a test domain to burn for E5's clone/move.

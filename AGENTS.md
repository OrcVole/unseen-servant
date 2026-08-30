---
title: "AGENTS.md — Unseen Servant (usv)"
description: "A security-first server for the small networks — Gemini, Gopher, Spartan, Nex and Finger — publishing one content tree to all of them and mirroring it to the web. Phase C7, pre-launch."
type: reference
status: decided
project: unseen-servant
tier: A
lifecycle: active
visibility: private
theme: terminal
last_verified: 2026-08-30
---

# AGENTS.md: operating manual for maintainers (AI or human)

Unseen Servant (`usv`): a security-first server for the small networks,
Gemini, Gopher, Spartan, Nex and Finger: that publishes one content tree to
all of them and mirrors it to the web. Written end to end by an AI, directed
and reviewed by a human (the director). This file is the entry point and
must always be enough to start work without archaeology.

This is the only agent-facing instruction file. House policy is `AGENTS.md`,
never `CLAUDE.md`.

## Read order

| # | Document | Why |
|---|---|---|
| 1 | `docs/adr/README.md` | Every decision, its status, its evidence |
| 2 | `docs/architecture.md` | The one idea and the module map |
| 3 | `DEBUGGING.md` | How to diagnose things, organised by symptom |
| 4 | `docs/internal/BRIEF.md` | The founding directive, verbatim |
| 5 | `docs/internal/BUILD-PLAN.md` | Phases C0: C7 and their exit gates |
| 6 | `docs/internal/OPEN-QUESTIONS.md` | What awaits the director; never re-ask an answered one |

`docs/agents.md` is a different thing: it is written for agents *using* a
capsule, not maintaining the code.

## Phase state

**C7 (hardening and launch preparation) under way.** C6 shipped the full
packaging matrix: Cloudron app, static musl tarball with a systemd unit,
`.deb`, AUR `PKGBUILD`, RPM spec, Nix flake and a plain OCI image: each
built and verified through a real install, run and remove cycle. C7 has so
far added extended fuzz campaigns, the project's own capsule content, a
conformance run against the live deployment from an external vantage point,
the documentation overhaul, and the agent-facing machine surfaces (`--json`,
`USV_LOG_FORMAT`, the exit-code contract).

**What C7 still owes (2026-08-30):** the conformance re-run from a host
with IPv6 and the gate wording settled; hours-long fuzz campaigns with
committed corpora; E1-E10 re-run against the digest that ships as
v1.0.0; a `test/secret-scan.sh` and host-detail sweep before the
repository goes public; then release engineering and the launch
checklist. The gate table is in `docs/internal/BUILD-PLAN.md` §Where the
gates stand; the nine decisions the director owns are
`docs/internal/OPEN-QUESTIONS.md` OQ-10; the phased plan is round
material in the workspace above this repository,
`phase-notes/PLAN-TO-PUBLIC-2026-08-30.md`.

Update this section when a gate passes. The gates are in
`docs/internal/BUILD-PLAN.md`; the blow-by-blow is in git history.

## Invariants

Violating any of these needs an ADR amendment first.

- `unsafe_code = "forbid"`: the ADR 0002 security argument rests on it.
- Every parser is fuzzed. A new parser lands with its fuzz target in the
  same change.
- The `gemini-diagnostics` suite must pass before any public exposure:
  27/27, or a documented, spec-legitimate non-pass for each check that
  does not (`DEBUGGING.md` §Conformance names the three). A hard gate,
  not advice. Wording settled by the director 2026-08-30; "clean" had
  been untrue as written since the first live run.
- **Nothing is announced, linked or listed publicly before v1.0 passes its
  gates**: including forum posts, store submissions and awesome-list pull
  requests.
- Nothing is described as supported in any outward-facing document until an
  implementation exists and a real client has driven it. `docs/protocols.md`
  is the authority and names the client.
- ADR discipline: decisions are recorded before or with their code; director
  defaults are never overturned silently; superseded ADRs are amended in
  place, never renumbered or deleted.
- One TOML configuration file. Unknown keys are startup errors.
- `usv` never executes content (ADR 0005: CGI refused permanently).
- Keys and certificates are never silently regenerated (ADR 0003); private
  key material is only ever held by the `identity` module.
- Gated content never reaches a cleartext tree (ADR 0012).
- Commits: `<area>: <what>` subject, body explains why. **No Co-Authored-By
  or AI-attribution trailers**: house policy; the authorship story lives in
  the documents. Push to the Forgejo origin.
- Never force-push, never skip hooks.
- Deliverables are files in this repository. Never publish to claude.ai
  Artifacts.

## Commands

```sh
cargo test                                   # 632 tests: unit + integration
cargo clippy --all-targets -- -D warnings    # warnings are errors
cargo fmt --check
cargo +nightly fuzz run frame_request_line   # needs cargo-fuzz
```

## Writing

- Explain jargon in plain words on first use, and give an abbreviation in
  full in brackets the first time it appears in a document: TCP
  (Transmission Control Protocol), TOFU (trust on first use).
- Short sentences. No pleading, no self-justification, no marketing. State
  what is true and move on.
- **Never use one network's word on another network's wire.** A Gemini
  reader has a capsule, a Gopher reader has a gopherhole, Spartan and Nex
  readers just have a site, and Finger has no site at all: only a person
  and their `.plan`. When one sentence must cover all five, use none of
  them ("one folder of writing"). Full table and sources:
  `docs/internal/notes/terminology.md`.
- Brand: Iosevka is the typeface, Ember Oxide `#E67916` marks code and
  examples. `docs/internal/notes/branding.md`.
- Record directives in the repository documents, not only in replies.
- Reader-facing documentation lives in `docs/`; research, plans and drafts
  live in `docs/internal/`.

## Verification

Verify by driving the real thing. Every genuine bug found in recent sessions
: gemtext links pointing at prose, a client panicking on an extensionless
path, finger advertising itself, a Cloudron address that turned out to be
private: was found by looking, not by reasoning.

Lagrange is the reference client.

## House context (the estate)

This repository lives inside the Cloudron packaging estate at
`…/Code/cloudron/packages/unseen-servant/unseen-servant/`. The folder above
it holds round material (phase notes, drafts); the repository itself stays
publishable.

- Estate doctrine root: `../../../estate/`. Round workflow:
  `starter/START-HERE.md`.
- Hosts: the build workstation also hosts the proving-ground virtual
  machine (a private Cloudron on an RFC1918 address, never public). The
  production Cloudron is load-saturated: **gates run on the proving
  grounds, never on production**. Host names live in the estate, not here.
- Secrets: the `secret` helper reads OpenBao (folders: github, forgejo, ai,
  apps, dns, infra).
- Base image truth: `cloudron/base:5.1.0` per live docs (2026-08-09). The
  estate skeleton and the global skill both still pin 5.0.0; this
  repository's recon overrides them.
- Model and effort tiering: security-sensitive parsing, TLS and ADR work
  wants a frontier model at high effort; mechanical work (boilerplate,
  themes, formatting, packaging plumbing) runs fine lower. Maximum effort
  only for genuinely hard, novel decisions.

## Layout

| Path | Holds |
|---|---|
| `src/` | The crate (module map in `src/lib.rs`) |
| `tests/` | Integration and regression tests: real binary, real sockets |
| `fuzz/` | cargo-fuzz workspace, one target per parser |
| `docs/` | Reader-facing manual and the ADRs |
| `docs/internal/` | Research, plans, drafts |
| `RELEASING.md` | How a release is cut, and the traps that have bitten |
| `CHANGELOG.md` | What changed per release; `Unreleased` until the first tag |
| `.forgejo/workflows/` | CI gates |

# FOUNDATION BRIEF: Gemini server, AI-forward, Cloudron-native

> Codename: **Unseen Servant** (binary/package name: `unseen-servant`).
> This brief was supplied by the human director (n) on 2026-08-09 and is
> preserved here verbatim as the project's founding document. The original
> transmission truncated mid-sentence in ADR 0007; the truncation point is
> marked below and the gap is recorded in docs/internal/OPEN-QUESTIONS.md.

## Mission

Recce the current state of the Gemini protocol and its ecosystem, then
design and build a Gemini server whose primary deployment target is a
Cloudron package, developed under the AI Forward principle: AI is the
primary author of code, configuration, documentation, and ongoing
maintenance. A human (n) reviews and directs; the AI writes and proves.

## Operating principles

- AI Forward: every artefact (code, config, docs, ADRs, tests, release
  notes) is AI-authored and structured so a future AI session can
  maintain it without archaeology. Optimise for machine-legibility of
  the codebase itself: small modules, exhaustive doc comments, ADRs for
  every non-obvious decision.
- Security is non-negotiable. This is a TLS-terminating daemon exposed
  to the public internet. No unsafe code without a written
  justification. All parsing fuzzed. The server MUST pass
  michael-lazar/gemini-diagnostics (the community torture suite) clean
  before any public exposure. Treat that as a hard gate, not advice.
- Documentation set follows the house Cloudron package convention:
  AGENTS.md, CLAUDE.md, UPGRADING.md, DEBUGGING.md, CONTRIBUTING.md,
  INTEGRATIONS.md, docs/adr/NNNN-*.md. Markdown only.
- Explicit version pinning throughout. Open formats only.

## Phase 0: Reconnaissance (produce docs/internal/recon/ before any code)

1. Protocol state. Read the official specs at geminiprotocol.net
   (protocol + gemtext). Then survey the spec development repos at
   gitlab.com/gemini-specification (protocol and gemini-text) for
   open, recently discussed, or accepted changes. Record what is
   stable, what is contested, what is forthcoming. Date every claim.
2. Companion and adjacent specs, with a verdict (support now / design
   for / ignore) and one paragraph of reasoning each:
   - Titan (uploads; omar-polo ships a client, gmid supports it)
   - client certificates (TOFU auth, certificate-gated zones)
   - Spartan, Nex, Scroll (adjacent smolnet protocols; Lagrange and
     Profectus speak them)
   - robots.txt companion spec, favicon RFC, TinyLog, GemPub
   - Atom/gemfeed feed conventions (CAPCOM, Antenna aggregators)
3. Prior art autopsy. Study Agate (Rust, static-only, frozen scope by
   policy), gmid (C, privsep, FastCGI, proxying, the renown leader),
   Molly Brown, twins, Jetforce. For each: what they got right, what
   their issue trackers show users actually ask for, what we adopt,
   what we deliberately refuse. Pay particular attention to gmid's
   privilege-separated process design (main / logger / server /
   crypto) and Agate's certificate lifecycle (auto-generation, expiry
   4096-01-01, per-hostname subdirectories).
4. Cloudron constraints. Gemini cannot sit behind Cloudron's reverse
   proxy: the server terminates its own TLS with SNI on tcpPort 1965,
   one capsule per Cloudron host. TOFU means the keypair is identity:
   it MUST live under /app/data, survive update/restore/rebuild, and
   never be silently regenerated. httpPort remains free and SHOULD
   serve an HTML surface so the app tile is not a dead end.
5. Output: docs/internal/recon/protocol.md, docs/internal/recon/ecosystem.md,
   docs/internal/recon/prior-art.md, docs/internal/recon/cloudron-fit.md, each with a
   Sources section with dates.

## Phase 1: Architecture (ADRs before code)

Decisions to make and record, with the director's defaults stated.
Overturn any default in the ADR if recon justifies it; do not overturn
silently.

- ADR 0001 Language and stack. Default: Rust, tokio, rustls. Evaluate
  building on titanite or gemax versus clean implementation of the
  wire protocol (it is deliberately tiny; clean may be simpler and
  more maintainable than a dependency).
- ADR 0002 Process/privilege model. Default: single process, but
  keys loaded into a dedicated task with no filesystem write access
  after startup; document why full gmid-style privsep is or is not
  worth it in a container where Cloudron is already the outer wall.
- ADR 0003 Certificate lifecycle. Auto-generate ECDSA on first run,
  far-future expiry, PEM accepted, per-hostname layout, SIGHUP
  reload, everything under /app/data/certs. Write the TOFU story in
  UPGRADING.md: what survives what.
- ADR 0004 Dual surface. One content tree in /app/data/content,
  rendered twice: gemtext served natively on 1965, HTML statically
  rendered at write time (watch + rebuild) and served on httpPort.
  No live gemini-to-HTTP proxying, no open proxy surface. Shared CSS
  kept minimal and classless.
- ADR 0005 Dynamic content. Default: refuse CGI. If dynamic content
  is wanted later, the answer is FastCGI or an internal handler API,
  decided then. Record the refusal and the reasoning.
- ADR 0006 Titan. Design the request pipeline so Titan can be added
  without restructuring; implement only if Phase 0 shows it is the
  authoring story we want (uploads gated on client certificate).
- ADR 0007 Config format. Single [BRIEF TRUNCATED HERE: the original
  transmission ended mid-sentence; the surviving fragment also fixed
  the project name: "we will call it Unseen-Servant or
  unseen-servant". Presumed intent: a single declarative config file;
  format to be proposed in the ADR and confirmed by the director.]

## Amendments (director, 2026-08-09)

1. Binary name: `usv`.
2. In accordance with Gemini philosophy, unseen-servant should be
   offered such that it can be selected for use with or without a
   Cloudron deployment in mind: Cloudron is the primary deployment
   target but a deployment profile, not a hard dependency. The core
   server must run standalone with sensible defaults. (Recorded as
   ADR 0008.)

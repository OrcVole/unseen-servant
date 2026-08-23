---
title: "ADR 0001: Language and stack: Rust, tokio, rustls, clean wire implementation"
description: "Rust (latest stable, edition pinned), implementing the Gemini v0.24.1 wire protocol cleanly."
type: explanation
status: decided
last_verified: 2026-08-11
---

# ADR 0001: Language and stack: Rust, tokio, rustls, clean wire implementation

- Status: Accepted (pending director review)
- Date: 2026-08-09
- Evidence: docs/internal/recon/prior-art.md §7, docs/internal/recon/protocol.md

## Context

The brief's default is Rust + tokio + rustls, with an instruction to
evaluate building on "titanite or gemax" versus a clean implementation
of the wire protocol. Recon (prior-art.md §7) settled the evaluation:

- **titanite** (crates.io 0.3.2, 2025-02-24) is an early-development,
  single-vendor YGGverse library carrying a "Project in development!"
  warning. Not a foundation for a product.
- **gemax does not exist as a Rust crate.** The name resolves to
  ninedraft/gemax, a **Go** library. The brief's premise was a factual
  error; it is corrected here, not silently.
- The historically notable Rust crates (northstar/twinstar, gemserv)
  have been unmaintained since 2022. windmark (0.7.0, 2026-05-29) is
  the only living crate but is an opinionated framework with a small
  bus factor; adopting it means inheriting its API, TLS choices, and
  release cadence for the ~5% of the codebase that is protocol code.
- The wire protocol is deliberately tiny: read one CRLF-terminated URI
  line (≤1024 bytes), write `STATUS SP META CRLF` + optional body,
  close with TLS close_notify. Every hard problem (TLS server config,
  SNI multi-cert, client-cert capture, cert generation, timeouts,
  streaming) lives in tokio/rustls/rcgen configuration that a wrapper
  crate would merely obscure.

## Decision

Rust (latest stable, edition pinned), implementing the Gemini v0.24.1
wire protocol cleanly. **No Gemini crates as dependencies.**

Core dependency set (each version-pinned in Cargo.toml, per the
brief's pinning principle):

- `tokio`: async runtime.
- `rustls` + `tokio-rustls`: TLS 1.2/1.3, SNI, client-cert capture.
- `rcgen`: certificate auto-generation (ADR 0003).
- A strict, hand-rolled request parser for the request line itself
  (the 1024-byte URI + CRLF rule and the reject-list: userinfo,
  fragment, non-ASCII, bare LF: are diagnostics-relevant behavior we
  want under direct test and fuzz control), delegating general URI
  parsing to `url`/`percent-encoding` where they are strict enough;
  the parser module documents exactly which crate handles which rule.
- `serde` + `toml`: configuration (ADR 0007).
- `tracing`: structured logging to stdout/stderr.

Agate (Apache-2.0/MIT) is the reference prior art: we study and adapt
its mechanisms (cert generation, response streaming) with attribution,
but take no code dependency on it.

`unsafe` is forbidden (`#![forbid(unsafe_code)]`) unless a written
justification is added to this ADR's amendments; per the brief, all
parsing is fuzzed (`cargo-fuzz` targets for the request line, the URI
edge cases, and the gemtext parser used by the HTML renderer).

## Consequences

- We own ~a page of wire-protocol logic and its full test surface,
  including all 27 gemini-diagnostics checks and the suite's known
  gaps (prior-art.md §6).
- No upstream Gemini crate can break, abandon, or constrain us; the
  cost is that rustls/tokio API migrations are ours to absorb,
  acceptable, as every serious Rust project bears that anyway.
- The brief's "titanite or gemax" option is closed. If a future
  maintainer revisits, prior-art.md §7 records the 2026 state of every
  candidate.

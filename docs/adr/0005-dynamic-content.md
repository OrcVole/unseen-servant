# ADR 0005: Dynamic content — CGI refused

- Status: Accepted (pending director review)
- Date: 2026-08-09
- Evidence: docs/recon/prior-art.md §§2–5, docs/recon/ecosystem.md §2

## Context

Every prior-art server that grew dynamic content converged on the same
lesson: in-process or forked CGI is the wrong shape. gmid removed CGI
in favor of FastCGI; Molly Brown's own README flags CGI's security
caveat and hints at SCGI; Agate refused CGI outright and its scope
freeze is why it is finished software. Meanwhile the one dynamic-
adjacent feature users demonstrably need — certificate-gated zones —
does not require executing anything (ecosystem.md §2: fingerprint
allowlists are pure server logic).

## Decision

**usv does not execute content. CGI is refused permanently** — not
deferred: no fork/exec of tree-resident programs, ever. The reasons:

1. It is the single largest attack-surface increase a static server
   can adopt (arbitrary code execution as the server user, environment
   injection, timeout and zombie management).
2. It contradicts the render-pipeline model (ADR 0004): CGI output
   exists on one surface only and cannot be statically mirrored.
3. The demand it historically served in Geminispace (guestbooks,
   search) is out of scope for a capsule server whose brief is
   publishing.

If dynamic content is wanted later, the decision *then* is between
**FastCGI/SCGI delegation** (gmid's mature answer — the dynamic
backend is a separate process with its own privileges, reached over a
socket) and an **internal handler API** (Jetforce's shape). To keep
both doors open cheaply, the internal architecture is a `Handler`
trait — parsed request in, `(status, meta, body-stream)` out — with
static file service, redirects, and cert zones as the v1 handlers.
That trait is **internal**: it is not a public extension API and
carries no stability promise (windmark already exists for people who
want a Rust Gemini framework).

Certificate-gated zones (Molly Brown's authorized_keys model: path
prefix → require cert → optional SHA-256 fingerprint allowlist →
60/61/62) ship in v1 as a handler, per ecosystem.md's "support now"
verdict. Because there is no CGI, no certificate details are ever
exported into environments or templates.

## Consequences

- The server's execution surface is closed; the fuzz and review budget
  concentrates on parsing and file service.
- Anyone needing an app server behind Gemini should run one beside usv
  or choose gmid/GmCapsule; the docs say this plainly rather than
  half-supporting it.
- The Handler trait costs a few lines of indirection now and makes
  ADR 0006's Titan option and any future FastCGI decision a new
  handler, not a restructure.

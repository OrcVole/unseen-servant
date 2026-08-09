# Architecture Decision Records

Numbered, never renumbered; superseded ADRs are amended in place with
a pointer, not deleted. Format: Status / Date / Evidence header, then
Context, Decision, Consequences.

| ADR | Decision | Status |
|---|---|---|
| [0001](0001-language-and-stack.md) | Rust + tokio + rustls, clean wire implementation, no Gemini crates (corrects the brief's "gemax" premise) | Accepted* |
| [0002](0002-process-privilege-model.md) | Single process; gmid's privsep goals as module/task boundaries | Accepted* |
| [0003](0003-certificate-lifecycle.md) | Agate's cert lifecycle; never silently regenerated; CA-signed opt-in | Accepted* |
| [0004](0004-dual-surface.md) | One content tree; native gemtext + write-time static HTML; no proxying | Accepted* |
| [0005](0005-dynamic-content.md) | CGI refused permanently; internal Handler trait; cert zones in v1 | Accepted* |
| [0006](0006-titan.md) | Titan designed-for, not in v1; cert-gated only if ever built | Accepted* |
| [0007](0007-config-format.md) | Single TOML file, gmid semantics, env overrides for platforms | Accepted (director confirmed 2026-08-09) |
| [0009](0009-responses.md) | Responses: cert-identified likes + moderated messages, dynamic write / static read | **Proposed** (OQ-8: version + counter-display default) |
| [0008](0008-deployment-profiles.md) | Cloudron is a deployment profile, not a dependency; standalone first-class | Accepted (director-directed) |

\* Accepted pending director review — per the AI Forward principle the
AI decides and records; the director may overturn any of these, and
overturns are recorded as amendments.

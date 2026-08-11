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
| [0010](0010-agent-and-assistive-access.md) | Legibility for agents and assistive/voice users is one requirement: named addressable affordances, lang declaration, site map, HTML landmarks; refuses content-negotiation and bespoke manifests | **Proposed** (director-raised) |
| [0011](0011-agent-identity-and-management.md) | Agent identity lifecycle (cert roster: rotation, capabilities, enrollment tokens: folds into C4); management reach is hybrid (observe remote via cert-gated gemtext, control local via CLI); HTTP agent surface is the packaging tier (/llms.txt, .md URLs, permissive AI posture, Schema.org); refuses A2A/MCP transport, RFC 9421, CA attestation, memory-backend role | **Proposed** (director-decided 2026-08-10) |

| [0012](0012-smolnet-protocols.md) | Smolnet protocols (gopher/Spartan/Nex/Finger, v1.1): one plaintext one-shot listener abstraction, off by default, non-privileged default ports; gopher is a third render target, Spartan/Nex serve the gemtext tree; Spartan uploads refused permanently; **cert/Titan-zoned paths excluded from every plaintext tree, structurally at render time** (amended 2026-08-10: exclusion + startup announcement + error only for a contradictory root, not a blanket refusal) | **Proposed** |

\* Accepted pending director review: per the AI Forward principle the
AI decides and records; the director may overturn any of these, and
overturns are recorded as amendments.

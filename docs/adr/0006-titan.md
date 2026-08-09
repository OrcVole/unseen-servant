# ADR 0006: Titan — designed for in v1.0, committed for v1.1

- Status: Accepted, amended 2026-08-09 (director directive)
- Date: 2026-08-09
- Evidence: docs/recon/ecosystem.md §1

## Context

The brief asked Phase 0 to decide whether Titan uploads are "the
authoring story we want". Recon found: client-side adoption is real
(Lagrange has shipped Titan upload since v1.6 and page-editing since
v1.10, so every mainstream user already holds an editor); the spec is
stable in practice but weakly stewarded (canonical home is a
gemini://-only wiki whose HTTPS mirrors 404); and gmid — the field's
maximalist — refuses to implement Titan in core, validating and
delegating it instead. For usv specifically, an upload is not a file
write: it must land in the source tree and re-trigger the dual render,
i.e. it is a pipeline mutation and real design work.

## Decision

**No Titan in v1.** The authoring story for v1 is filesystem-based:
Cloudron's file manager / SFTP / git-push-to-content-dir standalone,
all of which the watcher (ADR 0004) already turns into published
content.

**The pipeline is shaped so Titan can be added without restructuring:**

- The request parser recognizes the `titan://` scheme as a distinct
  case and rejects it with a clean, logged code path (status 53 with
  an explanatory META), never as a generic parse failure.
- Client-certificate plumbing (solicitation, fingerprint extraction,
  path-scoped zones) ships in v1 for cert zones (ADR 0005) — exactly
  the auth infrastructure Titan needs.
- The render pipeline is re-entrant and treats "a file changed" as its
  only input, so an upload handler would be a new writer, not a new
  pipeline.
- The config schema reserves a `[titan]` section (rejected with a
  helpful error if present while unimplemented, so configs are
  forward-compatible in intent but never silently ignored).

**If Titan is implemented later, these constraints are already
decided:** uploads gated on client-certificate fingerprint allowlists
only; the `token=` URL parameter is never accepted as the sole factor
(a shared secret in a URL); writable paths are explicitly configured,
default none; size limits enforced before body read.

## Amendment (2026-08-09, director)

The director has committed Titan to **v1.1** and gated the project's
public announcement on it: v1.0 ships quietly, the announcement wave
happens at v1.1 with Titan as the launch feature (docs/ROADMAP.md).
"Designed for" therefore hardens from an option into scheduled work:
the v1.0 pipeline seams above are now requirements, and a dedicated
implementation-grade recon (docs/recon/titan.md — wire format, spec
stewardship, client divergences) precedes the v1.1 design. The
security constraints in this ADR (cert-fingerprint gating only, no
token-as-sole-factor, explicit writable paths, pre-read size limits)
are unchanged and binding.

## Consequences

- v1 stays a pure read-path server; the diagnostics/fuzz gate covers
  everything the network can reach.
- The deferred cost is honest: adding Titan later is one handler plus
  one ADR revision, not a rewrite.
- If Phase 2 usage shows in-client editing is genuinely wanted, this
  ADR is the checklist for doing it safely.

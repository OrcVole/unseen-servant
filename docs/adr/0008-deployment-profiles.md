# ADR 0008: Deployment profiles — Cloudron is a target, not a dependency

- Status: Accepted (director-directed, 2026-08-09)
- Date: 2026-08-09
- Evidence: director amendment in docs/BRIEF.md; docs/recon/cloudron-fit.md; docs/recon/prior-art.md §1

## Context

The director's amendment (2026-08-09): in accordance with Gemini
philosophy, unseen-servant should be offered such that it can be
selected for use **with or without** a Cloudron deployment in mind.
Gemini's culture prizes small, self-contained tools an operator can
run from a shell; a server that only functions inside one PaaS would
be alien to it. Recon confirms the cost of honoring this is near zero:
every Cloudron-specific fact (paths under /app/data, injected env
vars, health-check contract) is a configuration value, not an
architectural assumption.

## Decision

**The core binary `usv` is a general-purpose Gemini server with no
knowledge of Cloudron baked into its defaults.** Cloudron support is a
*profile*: a thin layer of packaging artifacts that configures the
same binary.

- **Core defaults (standalone)**: state dir per platform convention
  (`/var/lib/usv` as system service; `$XDG_STATE_HOME/usv` as user),
  content at `${state_dir}/content`, certs at `${state_dir}/certs`
  (ADR 0003), Gemini on :1965, HTTP surface **optional and off by
  default** standalone (a pure-Gemini operator shouldn't get a web
  server they didn't ask for; ADR 0004's renderer can still run for
  those who want it). `usv` with zero arguments starts a working
  capsule.
- **Cloudron profile**: `CloudronManifest.json`, `Dockerfile`,
  `start.sh`, and a shipped `usv.toml` mapping the platform contract
  (cloudron-fit.md's hard-constraints checklist) onto core knobs —
  /app/data paths, `GEMINI_PORT` handling including the
  disabled-port case, HTTP surface **on** (tile + health check),
  `exec gosu` handoff. The profile contains no logic the core lacks;
  start.sh only translates environment into configuration.
- **Standalone artifacts ship in-repo alongside the Cloudron ones**:
  a reference systemd unit with hardening directives (ADR 0002), a
  plain Dockerfile (non-Cloudron), and install docs that treat
  standalone as a first-class path, not a footnote.
- **The test gate runs profile-agnostic**: gemini-diagnostics and the
  regress suite run against the bare binary; Cloudron-specific
  behavior (env mapping, disabled-port path) gets its own tests at the
  profile layer.
- Release engineering follows: versioned releases publish the crate/
  source and a static binary for standalone users, plus the Cloudron
  package. One version number covers all profiles.

## Consequences

- usv is selectable by the general Gemini community (where prior-art
  shows an appetite: Agate's users are exactly "give me a binary and a
  content dir" people), which also widens the pool of users proving
  the code that Cloudron deployments rely on.
- Every future feature must state its answer for both profiles; a
  feature that only works under Cloudron needs an explicit
  justification in its ADR.
- Cloudron remains the *primary* target per the brief: the profile is
  maintained in-tree, release-gated, and the packaging docs
  (cloudron-app-packaging conventions) apply to it in full.

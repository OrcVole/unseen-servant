# Maintenance: quiet ≠ abandoned

**Last reviewed: 2026-08-10.**

## Posture

**Finished software, actively watched.** Agate's own maintenance model,
"dependency bumps are the changelog": made explicit here so a quiet commit
history reads as intended, not as a stalled project. usv's scope is fixed
by design (`docs/internal/BRIEF.md`, `docs/internal/ROADMAP.md`); new protocol features are
deliberate, versioned decisions (an ADR precedes the code), not organic
growth.

## What gets watched, and how often

At least quarterly, tracked as a release heartbeat rather than a calendar
reminder, a review that finds nothing to do is itself the useful signal:

| Watching | For |
|---|---|
| RustSec advisories (`cargo deny check`) | A dependency gaining a known vulnerability |
| Rust toolchain (`rust-toolchain.toml`) | A deliberate, tested bump: never silent |
| Gemini spec releases | The 0.24.2 watch; any wire-format-relevant change |
| `gemini-diagnostics` | New or changed checks in the community torture suite |
| Cloudron base image + manifest schema | `docs/internal/recon/cloudron-fit.md`'s pinned digest and hard-constraints checklist going stale |
| Distro packaging health | `.deb`/AUR/Nix build breakage from upstream flag or toolchain changes (see `packaging/`'s own hard-won notes: Arch's `-flto=auto` and `--as-needed`, in particular) |
| Titan spec | Static since 2020: nothing to track, checked anyway |
| The IRI workstream | The one thing that could force real protocol work; deferred indefinitely upstream, re-checked each cycle |

## What a "dependency bump" release looks like

Per `UPGRADING.md`'s promise: a routine release never touches the
certificate lifecycle, never rewrites operator content, and never changes
config semantics without a MAJOR version and an explicit migration note.
Reserved config sections (`[titan]`, `[responses]`) exist specifically so a
config written for a newer usv fails loudly on an older one, instead of
silently doing less than the operator wrote down.

## Where the promises are actually tested, not just written down

- `docs/internal/BUILD-PLAN.md`'s C1: C7 exit gates, most already passed against a
  real, live install (not simulated): see the git history for what was
  verified and how.
- The proving-grounds experiment protocol (`docs/internal/BUILD-PLAN.md` C6) is
  re-runnable at any time the Cloudron package or platform contract
  changes; it is the fastest way to catch a maintenance regression before
  an operator does.
- `UPGRADING.md`'s survival table is a live claim, not an aspiration,
  each row has been exercised against a real Cloudron instance (fresh
  install, package update, backup/restore, clone to a new domain) with the
  certificate fingerprint checked byte-for-byte before and after.

## Reporting a problem

Forgejo issues on the canonical repo (`docs/internal/BRIEF.md`) once the project is
public (`docs/internal/ROADMAP.md` M6); until then, this file is the record of
intent for whoever picks it up.

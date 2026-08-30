---
title: "Roadmap"
description: "Plan of record. v1.0 is the release and carries everything: Titan (the 2026-08-09 collapse of the v1.0/v1.1 split) and the four smolnet protocols (folded in 2026-08-30, because they had already shipped)."
type: explanation
status: decided
last_verified: 2026-08-30
---

# Roadmap

Plan of record. History: the original plan split a quiet v1.0 from a
Titan-bearing v1.1; on 2026-08-09 the director collapsed that split,
"we will not use or announce it till it is ready for Titan", so v1.0 IS
the release, and everything before it is milestones, not versions.

**Second fold, 2026-08-30.** The smolnet protocols below were planned as
a v1.1 that would follow the announcement. They were built instead,
during the C5-C7 rounds of 2026-08-10 and 2026-08-11, each driven by a
real client (`docs/protocols.md` names them) and all six surfaces are
live on the project's own deployment. A roadmap that still called them
"v1.1, unwritten" was contradicting the code, and the launch pack's
claim gate reads this file. So v1.0 carries them, and the "v1.1" heading
below is kept as history rather than deleted, per house rule: the plan
was wrong in the happy direction, and that is worth seeing.

## v1.0: the release (announced when ready)

Nothing is announced, listed, or promoted before v1.0 is complete and
the gemini-diagnostics gate passes (hard gate, per the brief: 27/27 or
a documented, spec-legitimate non-pass per check, wording settled
2026-08-30).

Milestones, in build order:

**M1, the basics.** Core server per ADRs 0001-0005, 0007, 0008:
strict wire protocol, fuzzed request parser, certificate lifecycle
(ADR 0003), cert-gated zones, SIGHUP/SIGTERM discipline, regress
suite + all 27 gemini-diagnostics checks + the suite's known gaps.

**M2: dual surface.** Write-time HTML render, classless theme slot +
bundled vetted themes + docs gallery with screenshots, Atom for both
surfaces, gemsub dated links, content skeleton (index, robots.txt,
favicon.txt example), **lynx/w3m-friendly HTML verified** (director:
"support for lynx is good": semantic classless HTML is naturally
text-browser-friendly; test it, do not assume it).

**M3: Titan.** Cert-fingerprint-gated uploads per ADR 0006 +
docs/internal/recon/titan.md; tested live against Lagrange.

**M4: tooling parity.** `usv init` ratatui wizard (protocols,
hostname, dirs, theme; `--defaults` for scripts): the standalone
answer to Cloudron's panel, per the director: non-Cloudron deployers
get first-class TUI tooling, Cloudron is one optional route among
several. `usv` admin subcommands (status/fingerprint/check/zones/
render). **OnionShare drop-in flow**: `usv export` emits a
ready-to-drop folder for OnionShare website mode + a documented
recipe, so "drop your gemlog into OnionShare" is a first-class path.
Tor/I2P affordances (advertised_host, onion cert slot, no-SNI
tolerance).

**M5, packaging + docs.** Full matrix: cargo install, static musl
tarballs, .deb, AUR, Nix flake, OCI, Cloudron (AppImage skipped).
House docs set incl. MAINTENANCE.md ("finished, actively watched"
statement), UPGRADING.md (TOFU survival story), INTEGRATIONS.md.

**M5b: the small networks** (added 2026-08-30; was v1.1 below). Gopher,
Spartan, Nex and Finger as opt-in, off-by-default listeners over the
same content tree, per ADR 0012; gopher as a third render target;
cert-gated and Titan-gated paths excluded from every cleartext tree at
render time. **Shipped.** The gopher-space announcement venues move
into M6 as a second wave.

**M6: launch.** Project capsule live, dogfooded on usv (Forgejo
canonical repo public, orcvole GitHub mirror). COMPARISON.md vs.
Agate/gmid/Molly Brown/GmCapsule: including a **use-cases section
that says when the OTHER servers are the better choice** (director,
2026-08-09: honest signposting: e.g. gmid for FastCGI/proxying
needs, GmCapsule for Python extensibility, Agate for minimal
static-only, Molly Brown for pubnix multi-user). Announcement wave: Antenna, mailing
list, Station, Bubble, geminiprotocol.net listings, geminispace.info,
awesome-gemini PR, Fediverse #gemini, r/geminiprotocol. Second wave,
gopher-space: gopher-project list, Bongusta, Floodgap/Veronica-2,
#gopher. Venues re-verified at launch time.

## v1.1: the smolnet release (history: folded into v1.0 on 2026-08-30)

*Everything in this section shipped before v1.0 was tagged; see M5b
above. Kept as written so the original plan stays legible.*

- **Gopher, Spartan, Nex, and Finger** (director 2026-08-09: "include
  finger") as optional, off-by-default listeners per
  docs/internal/recon/smolnet.md; multi-protocol ADR precedes code. Gopher
  render target (menus, item typing, 70-col wrap, caps.txt); Spartan
  serves the gemtext tree (uploads rejected permanently); Nex
  near-as-is; Finger serves a configured profile/status text.
- Wizard grows the new protocol tick-boxes (with plaintext trust
  warnings).
- Gopher-space announcement wave: gopher-project list, Bongusta,
  Floodgap/Veronica-2, #gopher.
- Optional gophers:// (TLS first-byte sniffing) if cheap after gopher
  lands.

## Responses / interaction (version TBD: ADR 0009 after community-wisdom recon)

Director (2026-08-09): a gemlog hosted on usv should easily offer a
"responses" section: visitors optionally click a like or leave a
message; inspiration: Astrobotany's cert-based playful interaction.
Design principle to carry into ADR 0009: **dynamic write, static
read**, tiny built-in submission endpoints (Gemini: client-cert
identity + status 10 input, Astrobotany-style; web: minimal no-JS
form POST with honeypot/rate-limit anti-spam), responses stored under
the state dir, *displayed* by the normal static render on the next
pass, moderation-first (nothing publishes without operator approval
by default, via CLI/file manager). This walks through the
internal-handler door ADR 0005 explicitly reserved; it is NOT CGI and
executes no content. Scope, spam stance, and version to be fixed in
ADR 0009 once docs/internal/recon/community-wisdom.md lands.

## Later / backlog

- `usv tui` (ratatui dashboard: status, live log, zones, render),
  extends M4's parity story.
- Theme gallery expansion; community theme contributions; live
  theme-switcher on the project site.
- Scroll: watch (single implementer). Guppy: watch (UDP, tiny
  audience). Misfin: out of scope by kind: docs point at standalone
  misfin servers beside usv. Mercury dead; SuperTXT ignore
  (smolnet.md §5).

## Maintenance statement (to become MAINTENANCE.md)

Posture: **finished software, actively watched**: Agate's
"dependency bumps are the changelog" model, made explicit so quiet ≠
abandoned. Dated "last reviewed" stamp on capsule and README; at
least quarterly review + release heartbeat covering: RustSec
advisories; Rust toolchain; Gemini spec releases (0.24.2 watch);
gemini-diagnostics changes; Cloudron base image + manifest schema;
distro packaging health; Titan spec (static since 2020: no upstream
to track); IRI workstream (the one thing that could force real work;
deferred indefinitely upstream).

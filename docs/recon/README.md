# Phase 0 reconnaissance — index and synthesis

**Completed 2026-08-09.** Four documents, each self-contained with dated
sources. This index maps their findings onto the Phase 1 ADRs and lists
every place recon corrected an assumption in `docs/BRIEF.md`.

## The documents

| Document | Covers | Feeds |
|---|---|---|
| [protocol.md](protocol.md) | Spec state (v0.24.1, effectively frozen since 2024-08-28); stable/contested/forthcoming; strict-server guidance | Wire implementation, test suite, ADR 0001/0002 |
| [ecosystem.md](ecosystem.md) | Companion specs with verdicts: client certs + gemsub "support now"; Titan + Atom "design for"; the rest "ignore" | ADR 0004 (feeds), ADR 0006 (Titan) |
| [prior-art.md](prior-art.md) | Agate, gmid, Molly Brown, twins, Jetforce autopsies; all 27 gemini-diagnostics checks; Rust crate survey | ADR 0001, 0002, 0003, 0005 |
| [cloudron-fit.md](cloudron-fit.md) | tcpPorts/httpPort semantics, /app/data survival table, TLS options, packaging mechanics, prior Cloudron packages | ADR 0003, CloudronManifest, UPGRADING.md |

## Corrections to the brief (recon evidence vs. founding assumptions)

1. **"gemax" is not a Rust building block** — it is a Go library
   (ninedraft/gemax); there is no such crate. titanite is real but
   immature (single-vendor, "in development"). Recorded in ADR 0001.
2. **We are not the first Gemini server on Cloudron** — five community
   packages exist (Agate+, Atlas, Maple, Windmark, molly-brown), but
   none reached the official App Store; the store slot is open, and
   Cloudron staff endorsed exactly our dual-surface shape in the
   Agate+ review thread.
3. **Cloudron apps CAN read the platform's Let's Encrypt cert** via the
   `tls` addon — contrary to ecosystem folklore. TOFU self-signed
   remains the right default (LE rotation breaks pinning); CA-signed
   becomes a legitimate opt-in. Recorded in ADR 0003.
4. **Port 1965 is admin-remappable and disable-able** unless the
   manifest pins it `readOnly` — so "Gemini listener off, HTTP surface
   healthy" is a mandatory code path, and the manifest decision is
   recorded in cloudron-fit.md §1.
5. **The spec moved under the brief's feet in 2024**: META is now
   mandatory on 1x/2x/3x (no empty-META default), status 44's META is
   a message not a wait-time, and the redirect limit of 5 is
   normative. All absorbed into protocol.md's implementation guidance.

## Decisions the recon settles (carried into ADRs)

- Clean wire-protocol implementation on tokio + rustls + rcgen; no
  Gemini crates as dependencies (ADR 0001).
- Single process; gmid's privsep goals re-expressed as Rust module/task
  boundaries (ADR 0002).
- Agate's certificate lifecycle, adapted to /app/data and Cloudron's
  clone/move semantics (ADR 0003).
- Client-certificate zones (Molly Brown authorized_keys model) are v1;
  Titan is deferred but designed for (ADR 0006); CGI refused
  (ADR 0005); gemsub dated links ship in the generated indexes, Atom is
  a render-pipeline hook (ADR 0004).
- gemini-diagnostics (all 27 checks) is the hard gate, plus our own
  tests for its known gaps: percent-encoded traversal, 6x flows, SNI,
  redirects, timeouts (test-plan input; see prior-art.md §6).

## Later addenda (post-Phase-0)

| Document | Covers | Feeds |
|---|---|---|
| [agent-web.md](agent-web.md) | The agentic web (2025–26) mapped to usv: identity/auth, content legibility, memory/presence, agent-to-agent discovery. Finding: usv already sits where HTTP is retrofitting toward; debts are lifecycle + HTTP-surface packaging; refusals (no A2A/MCP transport, no CA attestation, not a memory backend) are doctrine-coherent. Dated 2026-08-09. | Proposed ADR 0011 (agent identity lifecycle); management-reach + HTTP-agent-surface decisions |

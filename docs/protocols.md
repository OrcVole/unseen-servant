# Protocol support

The single source of truth for what `usv` speaks today. Everything
outward-facing — the README's compatibility table, the project capsule,
release notes, announcement copy — must agree with this page, and this
page must agree with the code. If they ever disagree, the code wins and
this page is the bug.

**Status labels used here**

| Label | Means |
|---|---|
| **Supported** | Implemented, tested, and exercised end-to-end against a real client or suite. |
| **Planned** | Designed and scheduled, but **no implementation exists**. |
| **Rejected** | Deliberately not served. Requests are refused, not ignored. |

## Today (pre-1.0)

| Protocol | Status | Notes |
|---|---|---|
| **Gemini** | Supported | The primary protocol. Port 1965, own TLS, TOFU identity. Passes the community `gemini-diagnostics` suite (see `DEBUGGING.md`). |
| **Titan** | Supported | Uploads on the *same* listener, distinguished by URL scheme. Client-certificate gated, per-zone. |
| **Web (HTTP)** | Supported | The statically rendered HTML mirror of the same content tree. Behind a TLS-terminating proxy in the Cloudron profile; a plain HTTP listener standalone. |
| Gopher | **Planned (v1.1)** | Not implemented. See below. |
| Spartan | **Planned (v1.1)** | Not implemented. |
| Nex | **Planned (v1.1)** | Not implemented. |
| Finger | **Planned (v1.1)** | Not implemented. |
| Everything else (`http://` to *other* hosts, `gopher://` today, arbitrary schemes) | Rejected | Answered with Gemini status `53 PROXY REQUEST REFUSED`. `usv` is not a proxy. |

### The web mirror is not a web server

`usv` renders your content tree to static HTML and serves that. It does
not execute anything, has no CGI/FastCGI, and will not reverse-proxy.
If you need a general-purpose web server, run one — this surface exists
so a gemtext capsule is *also* readable by someone who only has a
browser, from the same source files, with no second publishing step.

That combination — one content tree, two protocols, rendered at write
time rather than per request — is the thing that distinguishes `usv`
from the other servers in [`../COMPARISON.md`](../COMPARISON.md).

## Planned for v1.1 — read this before claiming support

Gopher, Spartan, Nex, and Finger are **scheduled, designed, and
researched** (`docs/recon/smolnet.md` has the wire formats, server
requirements, and per-protocol effort estimates; `docs/ROADMAP.md` has
the schedule). They are **not written**. As of today the only mention of
Gopher in `src/` is in `protocol/uri.rs`, where it appears in the list of
foreign schemes to *refuse*.

The intent is that v1.1 ships them as optional, off-by-default listeners
— out of the box, but opt-in per capsule, because a plaintext protocol
alongside a TLS one is a decision an operator should make deliberately
rather than inherit.

Until an implementation exists and has been exercised against a real
client, no README, capsule page, store listing, or announcement may
describe them as anything but planned. Announcing protocol support to
the one community certain to check it is the fastest available way to
lose the credibility that [`../COMPARISON.md`](../COMPARISON.md) is
written to earn.

## Per-capsule versus per-project

This page answers "what can `usv` speak?" — a question about the
software.

"What does *this capsule* answer on?" is a different question, with a
different answer per install, and it should never be maintained by hand:
a capsule with Gemini and the web mirror on, Titan off, and an onion
address configured has a specific set of addresses, and only the running
server knows it. The intended answer is a generated page, alongside the
generated `map.gmi` / `sitemap.xml` that ADR 0010 already establishes —
listing every address the capsule actually answers on, and therefore
true by construction and incapable of overclaiming.

That page is not implemented yet either. It is recorded here so the
distinction survives: **generated, per-capsule, always true** is a
different artifact from **written, per-project, reviewed** — which is
this page.

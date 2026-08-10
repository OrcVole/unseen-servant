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

| Protocol | Status | Verified by |
|---|---|---|
| **Gemini** | Supported | `gemini-diagnostics` 25/27 against a live deployment; Lagrange |
| **Titan** | Supported | Live uploads from Lagrange |
| **Web (HTTP)** | Supported | Browsers, `lynx`, `w3m` |
| **Gopher** | Supported | **gelim, against a live public deployment** |
| **Finger** | Supported | **bombadillo** |
| **Nex** | Supported | **gelim** |
| **Spartan** | Supported *(wire-verified)* | Raw wire only — see the note below |
| Anything else (`http://` to *other* hosts, arbitrary schemes) | Rejected | Answered `53`; `usv` is not a proxy |

### What "verified by" means here, and why Spartan is hedged

A protocol is called Supported when it has been exercised against a
real client, not merely unit-tested. Gopher, Finger and Nex each have
one; Spartan does not yet, and saying so is the point of the column.

`gelim`'s Spartan mode returned nothing in non-interactive use — and
returned nothing against **spartan.mozz.us**, the reference server,
too. So it is a client-side limitation rather than a defect here. What
*is* confirmed is the wire: byte-for-byte request and response
exchanges, compared against what the reference server sends for the
same request.

That comparison also produced a fix. The reference server answers with
bare `text/gemini`; `usv` had been sending
`text/gemini;charset=utf-8`. UTF-8 is already the spec's default for
`text/*`, so the parameter added nothing and gave a client doing exact
string comparison something extra to disagree with. Now matched.

Spartan moves to unhedged Supported when it has been driven from
Lagrange or another client with working Spartan support.

### The web mirror is not a web server

`usv` renders your content tree to static HTML and serves that. It does
not execute anything, has no CGI/FastCGI, and will not reverse-proxy.
If you need a general-purpose web server, run one — this surface exists
so a gemtext capsule is *also* readable by someone who only has a
browser, from the same source files, with no second publishing step.

That combination — one content tree, several protocols, rendered at
write time rather than per request — is what distinguishes `usv` from
the other servers in [`../COMPARISON.md`](../COMPARISON.md).

### Cleartext protocols carry less

Gopher, Spartan, Nex and Finger offer no confidentiality, no integrity,
no server authentication, and **no client authentication of any kind**.
Certificate-gated and Titan-gated paths are therefore excluded from
every cleartext tree, structurally, at the point the tree is built
(ADR 0012 §6). All four are off unless the operator enables them.

Choosing between them — with clients, homepages and the philosophy of
each — is [`choosing-protocols.md`](choosing-protocols.md).

## The rule that got us here

Gopher, Spartan, Nex and Finger shipped in the v1.1 round. Until each
had been *exercised*, this page listed it as Planned and every
outward-facing document had to agree — because announcing protocol
support to the one community certain to check it is the fastest
available way to lose the credibility
[`../COMPARISON.md`](../COMPARISON.md) is written to earn.

The rule stands for whatever comes next: nothing is described as
supported here, in the README, in a store listing, or in announcement
copy until an implementation exists and a real client has driven it.
The "Verified by" column above is what that rule looks like when it is
being kept.

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

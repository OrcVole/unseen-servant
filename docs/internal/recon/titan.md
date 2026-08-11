# Titan Recon: Implementation-Grade Survey for usv v1.1

**Date:** 2026-08-09. Follow-up to `docs/internal/recon/ecosystem.md` §1; evidentiary basis for the v1.1 Titan implementation plan (ADR 0006).

**Summary.** Titan is a one-page community spec by Alex Schroeder for uploading data over a Gemini-shaped transaction: a `titan://` URL whose path carries `;`-separated `token`/`mime`/`size` parameters, followed by exactly `size` bytes of payload, answered with an ordinary Gemini response. The canonical spec is a **static wiki page reachable only over gemini://** (`gemini://transjovian.org/titan`); HTTPS mirrors 404, and this recon read it through the portal.mozz.us proxy. **There is no roadmap, no pending revision, no beta or successor version, and no version number at all**: the spec settled in mid-2020 and has not moved since (evidence in §2). Interop reality: Lagrange is the universal client (cert + optional token), GmCapsule is the reference native server (client cert required by default, 10 MiB default cap, buffers fully before dispatch), and gmid still refuses to implement Titan in core, planning only validate-and-delegate. For usv v1.1: same-listener scheme dispatch, cert-fingerprint-gated zones, declared-size validation *before* reading the body plus a hard read cap, and a serialized, re-entrant render pipeline are the load-bearing design points.

Caveat on verification: the canonical pages were retrieved via the portal.mozz.us HTTP proxy on 2026-08-09 and are paraphrased, not quoted verbatim from a local copy. Claims that could not be confirmed against a primary source are marked **[unverified]** inline.

---

## 1. Wire format

### 1.1 Transaction shape

1. Client opens TLS to the server (same TLS as Gemini; client certificates work identically: "when it comes to TLS, Titan is equivalent to Gemini"). The spec page as retrieved does not name a port; universal practice (GmCapsule, Lagrange, Transjovian) is **the same listener as Gemini, port 1965**: Titan is distinguished purely by URL scheme.
2. Client sends one request line: the absolute `titan://` URL followed by `CRLF`: same shape as a Gemini request. usv should enforce the same 1024-byte request-line limit as Gemini (the spec does not restate the limit **[unverified]**, but every parameter consumes budget from a Gemini-shaped line, and clients are built against that assumption).
3. Client then sends **exactly `size` bytes** of payload. `size` exists because binary payloads rule out EOF/sentinel termination (Schroeder's stated rationale on the history page: "The server needs to know when the transmission of the client ends").
4. Server replies with a standard Gemini response (status line, meta, optional body) and closes.

The server may reject the request after reading the URL and **before** the payload arrives (auth failure, oversize declaration, bad MIME): the spec explicitly allows early error responses. See §5.5 for the client-race consequence.

### 1.2 URL parameter syntax

Parameters are appended **to the path**, separated from it and from each other by semicolons, as `key=value` pairs. They are explicitly *not* query parameters: the spec states there is no question mark before them; a normal `?query` may still follow after the parameter block, distinct from the Titan parameters.

```
titan://example.org/wiki/page;token=hello;mime=text/plain;size=10
```

| Parameter | Status | Meaning |
|---|---|---|
| `size` | **Mandatory** | Payload length in bytes. |
| `mime` | Optional | Payload MIME type. Default: `text/gemini`. Servers may reject unsupported types or normalize (e.g. treat `text/plain` as `text/gemini`). |
| `token` | Optional | Shared-secret authorization string; interpretation is entirely up to the server ("a simple password, or even a command"). |

- **Order is not mandated.** The spec's own examples vary the ordering; the canonical Titan history page shows `;size=1234;mime=text/plain;token=hello` while the spec front example shows `;token=...;mime=...;size=...`. Parse all three in any order.
- **Percent-encoding of parameter values is unspecified.** The spec gives no charset or encoding rules for `token` (verified: the page contains no such guidance). Since `;` and `=` are the structural delimiters and the parameters live in a URI path segment, usv should: split on `;`, split each pair on the first `=`, percent-decode values, and reject requests containing repeated keys or unknown keys only if configured strict (unknown-key tolerance is safer for interop). Tokens containing `;`/`=` must arrive percent-encoded; document this for operators.
- Parse `size` as a non-negative decimal integer; reject missing, non-numeric, or over-limit values with a `59` before touching the body.

### 1.3 Responses

Any Gemini status is legal. Observed conventions:

- `30 <url>`, **redirect to the freshly written resource's gemini:// URL** after a successful upload. This is the dominant wiki convention (Phoebe/Transjovian) and what Lagrange-based editing flows expect, but the spec does *not* mandate it: `20` with a body is equally valid.
- `20 text/gemini` + confirmation body: also spec-sanctioned success.
- `59`/`50`: malformed parameters / refused upload; `60`/`61`/`62`: certificate demanded / not authorized / invalid, exactly as in Gemini.

Recommendation for usv: respond `30 gemini://<host><path>` on success (matches ecosystem expectations), `20` only for delete confirmations.

### 1.4 Deletion

**`size=0` is the delete operation**: "when the client wants to delete a resource, it uses the Titan protocol to send zero bytes of content." There is no DELETE verb and no separate scheme in the final spec. (Early 2020 proposals by Sean Conner had HTTP-like GET/PUT/POST/DELETE mimicry and separate schemes; Schroeder explicitly simplified all of that away: treat any reference to a delete scheme as historical noise.) Servers are free not to support deletion; if usv supports it, gate it per-path and behind the same cert allowlist.

---

## 2. Spec stewardship and roadmap

**The verified answer: there is no roadmap, no pending revision, and no beta or successor version of Titan. The spec is an unversioned, undated static wiki page, effectively frozen since 2020.** Evidence, all gathered 2026-08-09:

- The canonical spec page ("The Titan Specification" on the Transjovian wiki, read via portal.mozz.us) carries **no version number, no date, and no draft designation** (explicitly checked).
- The companion **"Titan history" page documents only the June: July 2020 genesis**: Schroeder's "Gemini Upload" post of 2020-06-04, the naming by Matthew Greybosch on 2020-06-13 in Sean Conner's mailing-list thread, Conner's HTTP-inspired counter-proposal and its simplification, and contains **no roadmap, version numbers, or pending revisions**.
- The wiki's change-log endpoints for the titan space (`/titan/changes`, `/titan/do/changes`) return Gemini `51 Not Found`. The 51 error text reveals the wiki is now served from **static files** (`/srv/transjovian/www/wiki/titan/...`): the spec is literally a static page today, no longer even a live wiki process. **[Last-edit dates therefore unobtainable; unverified when the pages were last touched.]**
- The **official Gemini project news feed (geminiprotocol.net/news/) contains zero Titan mentions in 2024-2026** (newest item 2026-06-20, "Seven years of Gemini!"). The Gemini spec-formalization effort of 2024 (0.24.x releases) never touched uploads.
- Searches of the **gitlab.com/gemini-specification** project surfaced no Titan/upload issue or proposal: Titan has never entered the standards-track process.
- **Alex Schroeder's blog**: every Titan-related post found dates to 2020 (2020-06-04 "Gemini Upload", 2020-06-14 "Using Titan to edit a Gemini wiki", 2020-07-02 overview); nothing from 2024-2026.
- The current Gemini **mailing-list host `lists.geminiprotocol.net` did not resolve** (DNS `ENOTFOUND`) during this recon, so 2024-2026 list traffic could not be searched directly **[unverified]**; the 2020 origin threads are visible via the sourcehut mirror (lists.sr.ht/~adnano/gemini, "Uploading Gemini content", June 2020).
- Corroborating attitude signal: gmid's maintainer opened issue #19 in June 2023 treating the spec as a fixed external artifact to validate against, not something with a change process to track.

Consequence for usv: implement against the wiki page as-is and pin a copy of its text (fetched over gemini:// once a client is available) into the repo as the normative reference. There is no upstream to track and no risk of a v2 invalidating the implementation; conversely, every ambiguity in the page (token encoding, parameter order) is permanently ours to resolve via de-facto client behavior.

---

## 3. Client behaviors

### 3.1 Lagrange (the client that matters)

- Titan upload since **v1.6** (2021); **"Edit Page with Titan"** since **v1.10**.
- Opening any `titan://` URL raises an upload dialog with two tabs: **Text** (typed content, sent as `text/plain`) and **File** (drag-and-drop; auto-detected MIME, manually overridable).
- Has an **optional token field**; help text mirrors the spec ("It is up to the server how this is interpreted. It could be used as a simple password, or even a command").
- **Uses the same TLS client identities as Gemini**: the user's per-site identity is presented on the Titan connection exactly as on Gemini. A cert-gated server works with stock Lagrange with zero extra ceremony.
- **[unverified]** How Lagrange reacts to a `60`/`61`/`62` sent *mid-upload* (i.e. after it has begun streaming the payload) is not documented in its help; assume the worst (§5.5) and reject before the body whenever possible, since the cert is available at handshake time anyway.

### 3.2 titan(1): the omar-polo CLI (bundled with gmid ≥ 2.0, Jan 2024)

- `titan [-C cert] [-K key] [-m mime] [-t token] url [file]`: reads stdin if no file.
- **Always appends `size`** to the URL; appends `mime`/`token` **only when the flags are given** (so a server must treat absent `mime` as `text/gemini`, per spec default).
- Client certificate via `-C`/`-K` (key defaults to the `-C` path).
- Exit 0 on `2x`/`3x`, 2 on other status codes: i.e. it treats redirect-after-upload as success, confirming the `30` convention.
- Performs **no TOFU or X.509 validation** beyond hostname match: do not expect this client to notice server-cert changes.

### 3.3 Perl `titan` client (Schroeder, metacpan.org/pod/titan)

Direct fetch was blocked during this recon (metacpan returned HTTP 402), so details below come from search-index excerpts of that page **[partially unverified]**: supports `--cert_file` client certs; guesses MIME with file(1) when unspecified; supports a multi-file mode where the URL ends in `/` and each filename becomes a page name (i.e. it issues one Titan transaction per file); its docs note that although the token is optional in the spec, "spammers and vandals have essentially made some form of protection necessary."

### 3.4 Others

Bollux/gemget-class tooling largely lacks Titan; other Titan-speaking servers/clients exist (atlas, Bunkum, Maple per the awesome-gemini index) but none has behavioral weight comparable to Lagrange. Interop target ranking for usv: **Lagrange first, titan(1) second, Perl client third.**

---

## 4. Server precedents

### 4.1 GmCapsule (skyjake): native reference implementation

- Purpose-built "Extensible Gemini/Titan server"; Titan on the **same listener** as Gemini, dispatched by scheme.
- `[titan]` config section:
  - `upload_limit` (int): "Maximum size of content accepted in an upload, in bytes. Defaults to 10485760 (i.e., 10 MiB)."
  - `require_identity` (bool): "Require a client certificate when receiving uploads. **Defaults to true.**"
- Handler contract: uploaded content is delivered to CGI **via stdin, only after the full payload has been successfully received** ("the program does not need to worry about interrupted uploads"): i.e. GmCapsule **buffers the entire upload** and owns size enforcement before any application code runs.
- Environment exposed to handlers: `TITAN_TOKEN` (the token parameter), `REMOTE_IDENT` (fingerprints of client certificate and public key), `TLS_CLIENT_HASH`.
- Precedent takeaways for usv: cert-required-by-default is established practice, not paranoia; 10 MiB is a sane default cap; fully-buffer-then-act is the proven processing model at these sizes.

### 4.2 gmid (omar-polo): the refusal precedent

- **No native Titan in core, still, as of gmid 2.1.1** (2.1.1 released 2026-08-25 per release listing; note: listing shows day/month: 2.1 on 03 Aug, 2.1.1 on 25 Aug, year contextually 2025 **[year unverified]**). Neither gmid.conf(5) nor gmid(8) contains any titan directive.
- Issue #19 ("Titan support?", opened 2023-06-24, milestone 2.2, still open): the plan is that gmid will do "basic validation but otherwise forward the request as-is" to a FastCGI backend or proxy target: the maintainer states plainly "personally I do not like the titan protocol." gmid 2.0 (2024-01-11) shipped only the titan(1) *client*.
- Precedent takeaway: a security-minded static server treating Titan as a delegation/validation problem: parse and validate the request line, enforce size and auth, hand the write off to an isolated component: is a respectable architecture, and matches usv's planned split between the request path and the render pipeline.

### 4.3 Phoebe / Transjovian (Schroeder's wiki stack)

Origin deployment; established the redirect-to-page-after-upload convention and the token-as-wiki-password pattern. Runs Gemini and Titan on one port. (Details from 2020 blog posts; current Phoebe behavior not re-verified.)

---

## 5. Security model recommendations for usv v1.1

### 5.1 Authorization: cert-fingerprint gating, mandatory

- **Require a client certificate on every `titan://` request**, unconditionally (GmCapsule's `require_identity=true` default, hardened to non-optional). Respond `60` when absent: before reading any payload.
- Authorize against the **per-path certificate-zone fingerprint allowlists** already planned for Gemini cert zones (Molly Brown authorized_keys model): the writable zone's allowlist is the set of principals who may upload. `61` on a cert not in the allowlist, `62` on structurally invalid certs. Identity primitive = SHA-256 fingerprint of the certificate (accept self-signed; no CA validation theater).
- Uploads permitted **only under explicitly configured writable path prefixes**; any `titan://` request outside a configured zone gets a flat refusal (`53`/`59`-class), same code path as the v1 scheme-rejection stub.

### 5.2 Token policy

- **Never token-only.** Tokens ride in the URL: they appear in logs, proxies, and client history, and the spec gives them no entropy or encoding rules. Treat them as an optional *second* factor: config per zone = `token: none | required(value)`; when required, compare constant-time against the configured value after the cert check passes.
- Accept and ignore an unexpected `token` parameter (Lagrange users may fill the field reflexively); never echo token values into logs or error metas.

### 5.3 Size enforcement: three distinct points

1. **Declared-size check at parse time**: reject `size` missing/non-numeric/greater than the zone's configured cap (default 10 MiB, per-zone overridable) with `59` before reading the body.
2. **Hard read cap**: read *exactly* `size` bytes with an overall and idle timeout; never trust the declaration: abort the connection if the peer under-delivers past the timeout, and never read a byte beyond `size` (excess bytes indicate a confused client; drop the connection rather than parse trailing data).
3. **Request-line cap**: the titan request line obeys the same 1024-byte limit as Gemini. Note the squeeze: `titan://host` + path + `;token=…;mime=…;size=…` all share that budget: long tokens plus deep paths can push legitimate requests over the line. Document a token-length guideline (< 100 bytes) for operators.

`size=0` (delete): disabled by default; per-zone opt-in, cert-gated like writes, and implemented as removal from the *source* tree followed by re-render (so both gemtext and HTML outputs retract together).

### 5.4 Same-listener dispatch

All precedents (GmCapsule, Phoebe, Transjovian) run Titan on the Gemini listener, distinguished by scheme; no surveyed server uses a separate port, and Cloudron's single-port model forbids it anyway. usv v1.1: the v1 parser's early `titan://` recognition branch graduates from "clean reject" to "titan handler"; everything TLS-side (including requesting the client cert) is shared with Gemini. Since the client cert is available at handshake completion, the full auth decision (§5.1) can and must run before the payload is read.

### 5.5 The early-rejection race

The spec permits rejecting before the payload, but a pipelining client may have already started streaming payload bytes when the error status arrives; a server that responds and immediately closes can cause the client to see a write error (broken pipe/RST) instead of the status line. Mitigation: after sending an early `59/60/61`, perform a graceful TLS close_notify and **drain up to a small bounded amount** (e.g. min(size, 64 KiB)) before closing, so well-behaved clients read the status. How Lagrange specifically behaves here is **[unverified]** (§3.1): test against real Lagrange during implementation; this is the top interop risk.

### 5.6 Re-entrant render pipeline (the actual hard part)

A Titan write in usv is not a file write: it is a **source-tree mutation followed by a re-render** of both the gemtext and HTML outputs. Requirements recorded for the implementation plan:

- **Single-writer serialization**: a mutex/queue over the mutate-and-render critical section; concurrent uploads execute one at a time. Upload payloads are received (and fully buffered, GmCapsule-style, given the ≤10 MiB cap) *outside* the lock.
- **Atomicity**: write payload to a temp file in the source tree's filesystem, fsync, rename into place; render into a fresh output staging directory and swap, so readers never observe a half-rendered site.
- **Path hygiene**: canonicalize the decoded path, reject `..`/absolute escapes and symlink traversal out of the writable zone; the writable zone maps to a source subtree, never to the rendered output trees.
- **MIME allowlist per zone**: default `text/gemini` + `text/plain` (normalize `text/plain`→`text/gemini` for wiki-style zones, as the spec sanctions); binary types (images) only by explicit config. Reject others with `59`.
- **Quota**: per-zone total-bytes quota in addition to per-upload cap (anonymous-ish uploads + disk exhaustion is the abuse pattern the ecosystem actually reports; the Perl client docs note spam made protection necessary in practice even with tokens).

---

## Sources (all accessed 2026-08-09)

Canonical (gemini://-only, read via the portal.mozz.us HTTP proxy; paraphrased, not verbatim):

- Titan wiki front page, https://portal.mozz.us/gemini/transjovian.org/titan (canonical: gemini://transjovian.org/titan)
- "The Titan Specification", https://portal.mozz.us/gemini/transjovian.org/titan/The%2520Titan%2520Specification
- "Authentication & Authorisation", https://portal.mozz.us/gemini/transjovian.org/titan/Authentication%2520&%2520Authorisation
- "Titan history", https://portal.mozz.us/gemini/transjovian.org/titan/Titan%2520history
- Wiki change logs, `/titan/changes` and `/titan/do/changes` under the same proxy: both Gemini 51 Not Found (error text shows static-file serving from `/srv/transjovian/www/wiki/titan/`)

Clients:

- Lagrange help (Titan section), https://raw.githubusercontent.com/skyjake/lagrange/dev/res/about/help.gmi
- titan(1) man page (gmid-bundled client), https://gmid.omarpolo.com/titan.1.html
- Perl titan client, https://metacpan.org/pod/titan (direct fetch returned HTTP 402 on 2026-08-09; details cited from search-index excerpts, marked [partially unverified])

Servers:

- GmCapsule user manual, https://geminispace.org/gmcapsule/gmcapsule.html (repo: https://codeberg.org/skyjake/gmcapsule)
- gmid issue #19 "Titan support?", https://github.com/omar-polo/gmid/issues/19 (opened 2023-06-24; open, milestone 2.2)
- gmid site and man pages, https://gmid.omarpolo.com/ (gmid.8.html, gmid.conf.5.html: no titan directives); releases, https://github.com/omar-polo/gmid/releases (2.0, 2024-01-11: "added titan(1), a simple titan client")

Stewardship/roadmap checks:

- Official Gemini news feed, https://geminiprotocol.net/news/ (no Titan mentions 2024-2026; newest item 2026-06-20)
- gitlab.com/gemini-specification (Protocol project), https://gitlab.com/gemini-specification/protocol (no Titan/upload issues found via web search)
- Alex Schroeder's site, https://alexschroeder.ch/view/2020-06-04_Gemini_Upload, https://alexschroeder.ch/view/2020-06-14_Using_Titan_to_edit_a_Gemini_wiki (all Titan posts date to 2020)
- Gemini mailing list (2020 origin thread mirror), https://lists.sr.ht/~adnano/gemini/%3C20200613053926.GH11281@brevard.conman.org%3E ("Uploading Gemini content", June 2020). Current list host lists.geminiprotocol.net: DNS did not resolve on 2026-08-09 [2024-2026 list traffic unverified]

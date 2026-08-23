---
title: "Ecosystem Recon: Companion and Adjacent Specs for Unseen Servant"
description: "Date: 2026-08-09. Phase 0, item 2 of the project brief."
type: explanation
status: decided
last_verified: 2026-08-11
---

# Ecosystem Recon: Companion and Adjacent Specs for Unseen Servant

**Date:** 2026-08-09. Phase 0, item 2 of the project brief.

This document surveys the Gemini companion specs and adjacent smolnet protocols relevant to Unseen Servant (usv), a static-content Gemini server in Rust targeting Cloudron (single capsule, self-terminated TLS, security-first, no CGI, one content tree rendered to both gemtext and HTML). Each item ends with a verdict: **support now**, **design for** (leave architectural room, do not implement yet), or **ignore**. The findings here are the evidentiary basis for ADR 0006 (Titan).

A framing observation that recurs throughout: most of the "ecosystem" is content-level convention, not protocol. A static server that serves arbitrary files with correct MIME types already "supports" TinyLog, gemsub feeds, robots.txt, GemPub downloads, and even favicon.txt without a single line of dedicated code. The only items that touch the server's actual request/TLS path are Titan and client certificates.

## 1. Titan (upload companion protocol)

**What it is.** Titan is a sister protocol to Gemini for uploading data. It uses the `titan://` scheme on the same port (1965) with the same TLS. A Titan request is a Gemini-style request line whose URL carries parameters appended to the path (`;token=...`, `;mime=...`, `;size=...`), followed by exactly `size` bytes of payload; the server replies with a normal Gemini response (typically `30` redirecting to the updated Gemini page). It exists because Gemini caps the entire request at 1024 bytes, making uploads impossible in-protocol.

**Spec status and location.** Community spec by Alex Schroeder, canonically hosted on the Transjovian wiki at `gemini://transjovian.org/titan` (pages: "The Titan Specification", "Authentication & Authorisation", "Titan history"). Notably, as of 2026-08-09 the HTTPS mirror of the spec pages at transjovian.org returns 404/51 errors (the wiki front page is reachable via the mozz.us portal proxy, but the specification page itself was not retrievable over HTTP during this recon). The spec is stable in practice but has no standards-track home; secondary documentation lives in the Perl `titan` CLI client docs on metacpan and in server manuals.

**Authorization in practice.** Two mechanisms coexist. (a) **Tokens**: the `token=` URL parameter is a shared secret; the spec explicitly leaves interpretation to the server (simple password, or even a command). This is weak auth: the token rides in the URL. (b) **Client certificates**: "when it comes to TLS, Titan is equivalent to Gemini, so the same server and client certificates can be used with both"; real deployments gate writable paths on certificate fingerprints. GmCapsule exposes the client cert fingerprint to Titan handlers (`REMOTE_IDENT` env var / `req.identity.fp_cert`); cert-fingerprint allowlists are the serious deployments' choice.

**Adoption (2026).** Client side is healthy: Lagrange has shipped Titan upload since v1.6 (2021) and "Edit Page with Titan" since v1.10; there is a Perl CLI client (Schroeder) and omar-polo ships a Titan client. Server side: GmCapsule (skyjake) is a purpose-built Gemini/Titan server; gmid does **not** implement Titan natively: it validates and forwards Titan requests to FastCGI or a proxy backend (gmid issue #19), i.e. even gmid treats Titan as a delegation problem, not a core-server problem; Gemini wikis (Transjovian, Communitywiki) are the flagship use case; atlas (C#) and others also speak it.

**VERDICT: design for.** Do not implement Titan in v1, but reserve room for it. Concretely: the request parser should recognize the `titan://` scheme early and cleanly reject it with a distinct code path (not a generic parse failure), the TLS layer must already be able to request and verify client certificates (needed anyway for item 2), and the config schema should reserve a `titan`/upload section. The reasoning: Unseen Servant's shape works against implementing it now. The brief's content model is one tree rendered to both gemtext and HTML by a build step: a Titan upload would have to land in the source tree and trigger a re-render, which is a pipeline mutation, not a file write; that is real design work, not a weekend feature. The precedent from gmid (delegate Titan rather than embed it) confirms that a security-first static server is right to keep write paths out of core. Meanwhile the ecosystem facts keep the door worth holding open: Lagrange's built-in editor means every mainstream user already has a Titan client, and the auth story usv would want (cert-fingerprint-gated writable paths, refuse token-only auth) matches infrastructure usv should build anyway. ADR 0006 should record: no Titan in v1; if implemented later, client-cert-gated only, tokens rejected or treated as an additional factor, uploads restricted to explicitly configured paths, and the render pipeline must be re-entrant.

## 2. Client certificates as application-level auth (status 60/61/62, TOFU, cert-gated zones)

**What it is.** Core-spec, not companion: the Gemini protocol specification (geminiprotocol.net/docs/protocol-specification.gmi) defines status `60` ("client certificate required"), `61` ("certificate not authorized": valid cert, wrong resource), and `62` ("certificate not valid": expired, not-yet-valid, malformed). A certificate solicited via 60 is scoped to host + port + the requested path and everything below it; clients MUST NOT auto-generate certs without user involvement. Server certificates are validated by clients via TOFU (fingerprint pinning), and the same TOFU idea applies to how servers can recognize returning client certs: self-signed certs are first-class, identity is the fingerprint.

**How existing servers expose it.** Molly Brown implements "certificate zones": config maps path prefixes to lists of approved SHA256 fingerprints, "analogous to SSH's authorized_keys". gmid and GmCapsule pass cert details (subject, fingerprints of cert and of public key) to CGI/FastCGI handlers via environment variables. So the mechanism splits cleanly in two: (a) *gating*: a pure server concern (require cert / check fingerprint allowlist / return 60/61), and (b) *introspection by content*, which only matters if you have dynamic content.

**Adoption (2026).** Universal on the server side for at least basic 60-gating (gmid, Molly Brown, GmCapsule, agate, etc. all do some form of it); every maintained client (Lagrange, Amfora, etc.) can create and present per-site identities. Certificate zones are the standard way Geminispace does private areas.

**VERDICT: support now.** This is the one companion-adjacent mechanism that is genuinely a server feature, and it is cheap and squarely on-brief. usv should ship: per-path certificate zones in config (require-cert → 60 when absent; optional fingerprint allowlist → 61 on mismatch; 62 for structurally invalid certs), with fingerprints as the identity primitive (accept self-signed; no CA chain validation theater). Because usv refuses CGI, the entire "expose cert info to content" half of the topic is out of scope: no env-var plumbing, no header injection into templates. Rationale: it is the protocol's only auth mechanism, every client supports it, Molly Brown's authorized_keys-style config is a proven minimal design to copy, and building the rustls client-cert plumbing now is exactly the room Titan needs later (item 1). A static server with cert-gated zones covers real use cases (private family capsule, staging area) at near-zero attack surface.

## 3. Spartan, Nex, and Scroll (adjacent smolnet protocols)

**What they are.** Spartan (michael-lazar, spec on GitHub, `spartan://`, port 300) is "Gemini without TLS" plus an upload-capable `=:` input line; plaintext over TCP, four status codes. Nex (m15o, spec at nightfall.city/nex/info/specification.txt, `nex://`, port 1900) is even smaller: plain TCP, send a path, get bytes back, no status codes, no TLS, gopher-adjacent link lines. Scroll (`scroll://`, clseibold/scrollprotocol.us.to) is the opposite direction: a Gemini-derived protocol with a *richer* document format, in devlog stage.

**Adoption (2026).** All three are niche-within-a-niche. Multi-protocol *clients* speak them: Lagrange (spartan), gelim (gemini/spartan/nex), Offpunk, and Profectus (scroll, gemini, nex, spartan: Profectus is effectively the Scroll reference client, by Scroll's own author). Server-side, they are separate listeners on separate ports with separate (or absent) security models; a handful of hobby servers (spartoi, atlas) and hubs (nightfall.city) exist. Scroll in particular has essentially one implementer.

**VERDICT: ignore.** A Gemini server gains nothing from speaking them, and Unseen Servant specifically loses things. Spartan and Nex are plaintext protocols; shipping them inside a security-first, TLS-self-terminated server contradicts the brief's core premise, and Cloudron's single-capsule port model makes extra listeners on ports 300/1900 an operational nuisance. Scroll is one person's in-progress design with one client. The users these protocols serve are reached anyway: usv's dual rendering (gemtext + HTML) already covers the "reach people without a Gemini client" goal far better than a Nex mirror would. If a future operator wants a Spartan/Nex mirror, it belongs in a separate daemon pointed at the same content tree: no architectural accommodation needed in usv.

## 4. robots.txt companion spec

**What it is.** Official companion spec at geminiprotocol.net/docs/companion/robots.gmi (one of only two specs the project blesses as "companion"). Adapts web robots.txt to Gemini: policy served at `/robots.txt` as text/plain. Because Gemini clients send no user-agent, the spec defines **virtual agents**: `archiver` (Wayback-style archives), `indexer` (search crawlers), `researcher` (statistical studies), `webproxy` (HTTP mirrors of Gemini content): plus `*`. Compliance is voluntary; the spec itself says enforcement is impossible and admins must fall back to firewalls for rogue bots.

**Adoption (2026).** The well-behaved crawlers that matter (geminispace.info search indexer, archive projects, the major web proxies like portal.mozz.us) honor it; it is the norm.

**VERDICT: support now, at documentation cost only.** The server needs nothing: `/robots.txt` is a static file in the content tree, and usv's MIME mapping will already serve `.txt` as `text/plain`. The only genuine server-side consideration is the HTML side of the dual render: usv's HTML output is itself a "webproxy"-like surface, so the docs should tell operators that a Gemini-side robots.txt does not govern web crawlers hitting the HTML tree (that needs a separate, ordinary web robots.txt, which the HTML renderer could emit from the same source). Ship a documented example robots.txt in the default content skeleton, and consider having the site generator write a parallel robots.txt into the HTML output. No request-path code, no verdict tension.

## 5. Favicon spec (/favicon.txt emoji)

**What it is.** A 2020 draft RFC by mozz.us proposing that capsules place a single emoji in `/favicon.txt` (text/plain, cached ≥1 hour) as a favicon analogue. The document (mozz.us/files/rfc_gemini_favicon.gmi) is still marked DRAFT, dated 2020-06-03 with a 2021-02 motivation update, and has never progressed.

**Current status (verified 2026-08-09).** It is **not** among the companion specs at geminiprotocol.net/docs/companion/ (only robots and subscription are). It was controversial from the start: a flashpoint in the "gemini-the-protocol vs. gemini-the-philosophy" argument about feature creep and clients making speculative extra requests, and adoption went backwards: Lagrange discussed it (issue #140) and the feature ended up off by default/removed in practice; no major client fetches favicon.txt today. It survives only as folklore.

**VERDICT: ignore.** Dead draft, deliberately excluded from the official companion set, near-zero 2026 client support, and philosophically disfavored because it invites clients to issue unrequested fetches. Crucially, ignoring it costs users nothing: any operator who wants one can drop a `favicon.txt` file into the content tree and usv will serve it correctly as static text/plain. There is nothing for the server to do and no reason to mention it in config. At most, one line in the operator docs noting the convention exists and is deprecated.

## 6. TinyLog format

**What it is.** A microblogging convention *in gemtext*: a single .gmi file with a `#` title, then entries as `##` headers whose text is a timestamp (extending the gemfeed date format YYYY-MM-DD with HH:MM and optional timezone), entry body underneath. Standardized as a community RFC by bacardi55 at codeberg.org/bacardi55/gemini-tinylog-rfc.

**Adoption (2026).** A modest but real subculture: dedicated readers exist (gtl, a TUI tinylog reader; a Rust parser crate `tinylog-gmi`), and tinylogs remain a living practice on hosts like flounder and smol.pub. Everything about it: authoring, parsing, aggregation: happens in clients and tools, never in servers.

**VERDICT: ignore.** It is a pure content-level convention over ordinary gemtext files; a static server serves a tinylog exactly as it serves any other .gmi, and there is no server behavior that could help or hinder it. The only conceivable touchpoint is usv's HTML renderer, which will render a tinylog acceptably as ordinary gemtext regardless. Not even documentation is strictly needed; a sentence in the content-authoring docs ("tinylogs are just gemtext; they work") is the ceiling.

## 7. GemPub (.gpub e-books)

**What it is.** An e-book/capsule-archive format from oppenlab (codeberg.org/oppenlab/gempub): a zip containing gemtext files plus a metadata file, media type `application/gpub+zip`, extension `.gpub`. Deliberately gemtext's answer to EPUB, with Gemini's "hold the whole spec in your head" ethos.

**Adoption (2026).** Modest and stable since 2021: Lagrange added reading support in v1.4 (cover page, chapter navigation planned/added subsequently); a few authoring tools exist; some capsules distribute books. It never became a mainstream Geminispace activity, but it is not dead either.

**VERDICT: ignore, with a one-line exception.** GemPub is a file format, not a protocol feature; serving a .gpub is just serving a binary file. The single thing usv should do is include `gpub → application/gpub+zip` in its default MIME table so downloads carry the right type for clients like Lagrange to open natively. That is one line in a static map, which falls below the threshold of "supporting" anything; the verdict for architectural purposes is ignore.

## 8. Atom, gemfeed/gemsub, and the subscription companion spec

**What it is.** "Subscribing to Gemini pages" (geminiprotocol.net/docs/companion/subscription.gmi), the second of the two official companion specs, defines how an ordinary gemtext page *is* a feed ("gemsub"/"gemfeed" convention): first `#` header = feed title, optional `##` = subtitle, and every link line whose label begins with an ISO 8601 date (YYYY-MM-DD) is an entry. The spec's design goal is explicit: "a simple, manually-updated, human readable index page" needs **no modification** to be subscribable, and no server support beyond serving the file. Atom remains valid in parallel for authors who want time-of-day precision.

**How aggregators consume it (2026).** The two canonical aggregators both accept gemsub pages natively, not just Atom: CAPCOM (gemini.circumlunar.space/capcom, the classic aggregator; its 2023 overhaul on geminiprotocol.net/news/2023_04_16.gmi added subscribable-page support alongside Atom, plus SQLite-backed active/inactive feed tracking with exponential backoff) and Antenna (warmedal.se, submission-queue model: authors ping a URL, Antenna fetches and republishes entries from the past week). Client-side, Lagrange has first-class gemsub subscriptions. Practical consequence: **an Atom feed is no longer required for full participation in Gemini's aggregation ecosystem**: a dated gemtext index page is sufficient for CAPCOM, Antenna, and Lagrange alike.

**VERDICT: support now (gemsub, at zero cost) / design for (auto-generated Atom).** Split verdict because the item splits. Gemsub: nothing to implement: usv serves gemtext, and any index page the operator (or usv's own site generator) writes with dated links is already a feed; the real action item is that usv's *content pipeline* should emit dated link lines on generated index pages, which makes every usv capsule subscribable by default. That is a generator/templating concern, costs almost nothing, and should be in v1. Atom auto-generation: leave a hook in the render pipeline (the generator already walks the content tree and knows titles/dates, so emitting `atom.xml`, and an RSS/Atom feed for the HTML side, where Atom *does* still matter because web feed readers do not speak gemsub: is a natural later feature), but do not block v1 on it, since the Gemini-side ecosystem demonstrably no longer needs it. The dual-render architecture is the deciding factor: the same metadata pass serves both outputs, so design the pipeline so feeds are a rendering target, not an afterthought.

## Verdict table

| # | Item | Verdict | One-line rationale |
|---|------|---------|--------------------|
| 1 | Titan uploads | **Design for** | Real clients exist (Lagrange since v1.6), but uploads mutate a rendered content tree; reserve scheme handling, cert plumbing, and config space; decide implementation in ADR 0006 as "later, cert-gated only". |
| 2 | Client certs / cert zones (60/61/62) | **Support now** | The protocol's only auth mechanism; Molly-Brown-style fingerprint zones are cheap, proven, on-brief, and are the prerequisite for Titan. |
| 3 | Spartan / Nex / Scroll | **Ignore** | Separate plaintext protocols on separate ports; contradict security-first TLS design and Cloudron's port model; HTML render already covers outreach. |
| 4 | robots.txt companion | **Support now** (docs + skeleton only) | Official companion spec; it is just a static text/plain file; document virtual agents and mirror a robots.txt into the HTML output. |
| 5 | favicon.txt | **Ignore** | Perpetual draft, excluded from official companion specs, clients dropped it; works as a plain static file anyway if an operator insists. |
| 6 | TinyLog | **Ignore** | Pure gemtext content convention; zero server surface. |
| 7 | GemPub | **Ignore** | File format, not protocol; add `gpub → application/gpub+zip` to the MIME table and move on. |
| 8 | Feeds (gemsub / Atom) | **Support now** (gemsub) / **design for** (auto-Atom) | Official companion spec; dated links on generated index pages make capsules subscribable for free (CAPCOM, Antenna, Lagrange all consume gemsub); Atom is a pipeline hook for later, mainly for the HTML side. |

## Sources

All URLs accessed 2026-08-09. Canonical gemini:// resources were accessed via HTTPS mirrors or the portal.mozz.us proxy where noted.

- <https://geminiprotocol.net/docs/>: official spec index (protocol spec, gemtext spec, companion specs). Accessed 2026-08-09.
- <https://geminiprotocol.net/docs/protocol-specification.gmi>: Gemini network protocol spec; status 60/61/62, cert scoping, TOFU. Accessed 2026-08-09.
- <https://geminiprotocol.net/docs/companion/>: companion spec index; confirms only robots + subscription are blessed. Accessed 2026-08-09.
- <https://geminiprotocol.net/docs/companion/robots.gmi>: robots.txt for Gemini; virtual agents archiver/indexer/researcher/webproxy. Accessed 2026-08-09.
- <https://geminiprotocol.net/docs/companion/subscription.gmi>: "Subscribing to Gemini pages" (gemsub) companion spec. Accessed 2026-08-09.
- <https://geminiprotocol.net/news/2023_04_16.gmi>, CAPCOM overhaul: gemsub support, SQLite feed management. Dated 2023-04-16; accessed 2026-08-09.
- gemini://transjovian.org/titan via <https://portal.mozz.us/gemini/transjovian.org/titan>: Titan wiki front page (spec, auth, history links). Accessed 2026-08-09; note: the specification subpages 404 over the HTTP mirror as of this date; canonical home remains gemini://transjovian.org/titan.
- <https://metacpan.org/pod/titan>: Perl Titan CLI client; request format with token/mime/size URL parameters, cert options (fetched via search snippets; direct fetch was paywalled/blocked on access date). Accessed 2026-08-09.
- <https://github.com/omar-polo/gmid/issues/19>, gmid's Titan stance: validate and delegate to FastCGI/proxy rather than implement in core. Accessed 2026-08-09 (via search).
- <https://gmi.skyjake.fi/gmcapsule/>: GmCapsule Gemini/Titan server; Titan handlers via CGI/modules, client cert fingerprint in REMOTE_IDENT / req.identity.fp_cert. Accessed 2026-08-09.
- <https://github.com/skyjake/lagrange/issues/279> and /issues/415: Lagrange Titan upload (v1.6, 2021) and "Edit Page with Titan" (v1.10). Accessed 2026-08-09 (via search).
- <https://gmi.skyjake.fi/gemlog/2021-07_lagrange-1.6.gmi>: Lagrange 1.6 announcement (Titan). Dated 2021-07; accessed 2026-08-09 (via search).
- <https://github.com/LukeEmmet/molly-brown>: Molly Brown README; certificate zones with SHA256 fingerprint allowlists, CGI cert env vars. Accessed 2026-08-09 (via search).
- <https://github.com/michael-lazar/spartan>: Spartan protocol specification repo. Accessed 2026-08-09 (via search).
- <https://nightfall.city/nex/info/specification.txt>: Nex protocol specification (plain TCP, port 1900, no TLS). Accessed 2026-08-09.
- <http://scrollprotocol.us.to/software/profectus/>: Profectus client (scroll/gemini/nex/spartan); Scroll protocol devlog home. Accessed 2026-08-09 (via search).
- <https://sr.ht/~hedy/gelim/>: gelim multi-protocol client (gemini/spartan/nex). Accessed 2026-08-09 (via search).
- <https://portal.mozz.us/gemini/mozz.us/files/rfc_gemini_favicon.gmi>: favicon.txt RFC; marked DRAFT, dated 2020-06-03, motivation updated 2021-02. Accessed 2026-08-09.
- <https://github.com/skyjake/lagrange/issues/140>: Lagrange favicon discussion (feature contested/deprecated in practice). Accessed 2026-08-09 (via search).
- <https://codeberg.org/bacardi55/gemini-tinylog-rfc>: TinyLog community RFC. Accessed 2026-08-09 (via search).
- <https://github.com/bacardi55/gtl>: gtl TUI tinylog reader. Accessed 2026-08-09 (via search).
- <https://codeberg.org/oppenlab/gempub>: GemPub spec repo; application/gpub+zip. Accessed 2026-08-09 (via search).
- <https://github.com/skyjake/lagrange/issues/255>: Lagrange GemPub support (v1.4, 2021-05). Accessed 2026-08-09 (via search).
- <https://gemini.circumlunar.space/capcom/>: CAPCOM aggregator. Accessed 2026-08-09 (via search).
- <https://warmedal.se/~bjorn/posts/announcing-antenna.html>: Antenna announcement; submission-queue model. Accessed 2026-08-09 (via search).
- <https://wiki.archiveteam.org/index.php/SmolNet>: SmolNet protocol overview (gemini/gopher/spartan/nex/scroll landscape). Accessed 2026-08-09 (via search).

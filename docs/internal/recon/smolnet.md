---
title: "Smolnet protocols: implementation recon for gopher, Spartan, and Nex"
description: "Date: 2026-08-09. Status: research complete; supersedes the shallow treatment in ecosystem.md §3 (whose 'ignore' verdict predates the director's decision to schedule these protocols as."
type: explanation
status: decided
last_verified: 2026-08-11
---

# Smolnet protocols: implementation recon for gopher, Spartan, and Nex

**Date:** 2026-08-09. **Status:** research complete; supersedes the shallow treatment in `ecosystem.md` §3 (whose "ignore" verdict predates the director's decision to schedule these protocols as optional, off-by-default listeners over the same content tree).

**Summary.** The three scheduled protocols form a clean difficulty ladder. Nex is nearly free: a plaintext TCP listener that returns bytes for a path, with directory listings that use gemtext's own `=>` link syntax: usv's gemtext can be served almost unmodified. Spartan is a plaintext sibling of Gemini with a three-field request line, four status codes that map trivially onto usv's handler results, and a built-in upload mechanism we should explicitly refuse (or later bridge to the Titan write path). Gopher is the real project: a menu-oriented protocol whose directory documents (gophermaps) are a different hypertext model than gemtext, requiring a dedicated render target, item typing, 70-column wrapping, TAB-safe info lines, `URL:` link shims, dot-stuffing for text responses, and a `.`-terminated framing rule: plus the operational wrinkle that ports 70 and 300 are privileged. Gemtext's one-link-per-line design makes the conversion far cleaner than HTML→gopher would be; there is no inline-link extraction problem. Of the surveyed "other" protocols, only Finger is cheap enough and client-supported enough to deserve a watch-plus rating; Guppy is a different transport (UDP) for a tiny audience, Misfin is out of scope by kind (mail, not content), Mercury is dead, Scroll remains a one-implementer project, and SuperTXT is a different universe entirely.

---

## 1. Gopher

### 1.1 Wire format (RFC 1436 + modern practice)

**Request.** Client connects (canonical port 70, IANA-assigned) and sends a selector string terminated by CRLF. An empty selector requests the root menu. Selectors are opaque server-side identifiers (RFC max 255 bytes; usv should tolerate longer but bound at ~1 KiB and reject with a type-3 error). A type-7 search request appends `TAB query` to the selector. Gopher+ clients may append `TAB +` or `TAB !` to a selector; a Gopher0 server should strip anything from the first TAB onward (unless implementing type 7) and serve normally: this is what modern servers do. Some web browsers and scanners send `GET / HTTP/1.1` to port 70; gophernicus detects the `GET` prefix and answers with an HTML pointer page: a nice-to-have, not a requirement.

**Menu response.** A menu (directory) is a sequence of lines, each: one item-type character, then the display string, TAB, selector, TAB, hostname, TAB, port, CRLF. The response ends with a line containing a single `.` (the "Lastline") and the server closes the connection. Display strings should stay under ~70 characters: classic clients render in 80-column terminals.

**Text response (type 0).** The file body, with dot-stuffing: any body line beginning with `.` gets an extra `.` prepended, and the response ends with `.` CRLF. (In practice many servers skip dot-stuffing and just close the socket; clients tolerate both, but the RFC behavior is cheap and usv should do it correctly.) Binary responses (type 9, g, I, etc.) are raw bytes until close: no terminator, no dot-stuffing.

**Item types.** RFC 1436 defines: `0` text file, `1` directory/menu, `2` CSO phonebook, `3` error, `4` BinHexed Mac file, `5` DOS archive, `6` uuencoded file, `7` full-text search, `8` telnet, `9` binary, `+` redundant server, `T` TN3270, `g` GIF, `I` image (non-GIF). De facto additions universally supported today: `i` informational text (non-link menu line, the backbone of modern gophermaps; not in the RFC), `h` HTML (used both for served .html files and, with a selector of the form `URL:https://example.com/`, for links out to other protocols), `d` document (PDF/doc), `s` sound, and sometimes `p` PNG. For usv the needed output set is: `0`, `1`, `3`, `9`, `g`, `I`, `h` (+`URL:`), `i`. Type `7` (search) exists; usv will not implement search: document that a search deployment would require a query handler, which contradicts the static model.

**`URL:` links.** The convention for linking to non-gopher URLs: item type `h`, selector `URL:<absolute-url>`. Modern clients (Lagrange, Overbite, lynx) open the URL directly. For ancient clients that actually request the `URL:...` selector, gophernicus serves a small HTML redirect page; usv should answer `URL:` selectors with such a page (trivial template) or a type-3 error: the former is the polite norm.

**Info lines.** A menu line that is pure text uses type `i` with dummy link fields. Convention: `i<text> TAB fake TAB (NULL) TAB 0` (gophernicus/Bucktooth style) or the server's own host with port. Strict clients require all four fields, so the dummies must be present. Critically, **TAB may not appear in a display string**: any TAB in source content (e.g., inside gemtext preformatted blocks) must be expanded to spaces by the renderer.

**Gopher+.** The 1993 extension (attribute blocks, `!`/`$` requests). Effectively dead; no modern smolnet client depends on it. **Verdict: ignore entirely; just strip TAB-suffixed request extensions gracefully.**

**Encoding.** RFC 1436 assumes ASCII/Latin-1 and forbids non-printables in display strings. Modern practice is UTF-8 throughout: gophernicus serves UTF-8, Lagrange and modern lynx assume it, and the community treats UTF-8 gophermaps as normal. usv should emit UTF-8 and declare it in caps.txt; the only hard rules are no TAB/CR/LF/NUL inside fields.

**caps.txt.** A de facto convention (documented on the gopher-project mailing list; auto-generated by gophernicus, consumed by Overbite and others): a text file at selector `caps.txt` with `Key=Value` lines advertising server capabilities: e.g. `CapsVersion=1`, `ExpireCapsAfter=<seconds>`, `PathDelimeter=/`, `ServerSoftware=...`, `ServerSoftwareVersion=...`, plus description/geolocation/encoding keys. Cheap to emit from usv's config; do it: it is the closest thing gopher has to a server identity endpoint.

**Gopher over TLS ("gophers").** Two coexisting mechanisms in 2026: (a) an explicit `gophers://` URL scheme, supported by Lagrange and some TUI clients, where the client simply starts TLS on connect; (b) the "GoT" same-port autodetection approach (documented in the community draft discussed on dataswamp.org and the gopher list): the server peeks at the first bytes of the connection: a TLS ClientHello begins with byte 0x16, while a plaintext gopher request is printable text ending in CRLF, and dispatches accordingly, optionally with `gopher` as ALPN identifier. geomyidae supports TLS serving (sticky-bit-on-basedir = TLS-only content). Since usv already terminates TLS for Gemini, offering optional `gophers` on the same listener via first-byte sniffing is architecturally cheap and uniquely on-brand for a security-first server, but it is strictly optional; plaintext port 70 is what defines reach. **Recommendation: ship plaintext first; leave the sniffing dispatcher as a designed-for extension.**

**No virtual hosting.** The gopher request contains no hostname (the host/port appear only in menu *responses*). One listener = one content tree. usv's config must bind the gopher listener to exactly one site, and the configured canonical hostname/port pair gets baked into every generated menu line. This is the single most architecture-relevant fact about gopher.

### 1.2 Server requirements

- TCP listener, canonical port 70. **Port 70 < 1024 is privileged**: an unprivileged usv process cannot bind it directly. Workarounds to document: `CAP_NET_BIND_SERVICE` (systemd `AmbientCapabilities` or `setcap`), systemd socket activation, `sysctl net.ipv4.ip_unprivileged_port_start=0`, firewall/NAT redirect from 70 to a high port, or: in the Cloudron deployment: the container port mapping in the manifest (`tcpPorts`), which makes the privileged-port problem disappear since the in-container process binds any port. usv should default to a high port (e.g. 7070) and document the mapping paths.
- Framing: read until CRLF (tolerate bare LF), enforce selector length cap and a read timeout (a few seconds: clients send immediately), write response, half-close, full close. No keep-alive, no pipelining. One request per connection, ever.
- Path safety: selectors map to the gopher output tree; the same traversal-proof canonicalization usv already does for Gemini paths applies (reject `..`, NUL, absolute escapes). Selectors with spaces are legal and must round-trip.
- Errors: gopher has no status codes. Any failure is a one-line type-3 menu: `3<message> TAB fake TAB (NULL) TAB 0` + CRLF + `.` CRLF. Map usv's internal not-found/forbidden/internal-error results all onto type-3 lines with distinct messages.
- Serve `caps.txt`; answer `URL:` selectors with the HTML redirect page; strip gopher+ TAB suffixes; optionally answer `GET` with an HTML pointer.

### 1.3 Render requirements (gemtext → gopherspace)

This is a full third render target ("gophermap generation"), parallel to the existing gemtext and HTML outputs, and pre-rendering into a static gopher tree fits usv's architecture better than on-the-fly conversion (deterministic output, serve path stays a dumb byte-pump, dot-stuffing applied at serve time for type-0 only).

- **Every .gmi page becomes a type-1 menu**, not a type-0 text file: that is the only way its links stay clickable. Modern servers model this the same way: gophernicus renders a directory from a `gophermap` file (tab-format, `*` to append auto-listing, `!title`, comments); geomyidae from `index.gph` (its own `[type|text|selector|host|port]` bracket syntax, plus raw-tab passthrough); Bucktooth originated the tab-format `gophermap`; pygopherd additionally understands UMN `.Links`/`.cap` legacy formats. usv emits gophermap-equivalent *wire* menus directly (or gophernicus/geomyidae-compatible source files if we ever want interop with existing daemons, not needed for our own listener).
- **Line mapping:** gemtext text lines → `i` lines wrapped at ≤70 columns; headings → `i` lines, optionally underlined with `=`/`-` `i`-lines (community style); list items → `i` lines with `-` prefix preserved; quotes → `i` lines with `>`; preformatted blocks → `i` lines verbatim except TAB→spaces expansion and no wrapping (long lines are the author's problem, as in terminals).
- **Link mapping:** gemtext's one-link-per-line design means no inline-link extraction is ever needed: each `=>` line becomes exactly one menu item. Relative/`gopher://` links to pages → type `1`; to plain text → `0`; to images → `g`/`I`; other binaries → `9`; links to `http(s)://`, `gemini://`, `spartan://`, `nex://`, etc. → `h` with `URL:` selector (the convention supports any absolute URL; Lagrange follows all of these natively). The link label becomes the display string (truncate ~70 cols).
- **Trees and defaults:** directories map to menus; the source tree's index page becomes the directory's menu content. Extension→item-type mapping is a small static table alongside the existing MIME table.
- Type-0 companion copies of articles (wrapped plain text) are optional; the menu-per-page model alone is standard practice for gemtext-to-gopher mirrors (cf. gopher proxies of geminispace).

### 1.4 Adoption 2026

Gopher is by far the largest plaintext smolnet: active servers in the hundreds+ (Floodgap, SDF, bitreich, tildeverse gopherholes), active daemons still maintained (gophernicus, geomyidae, motsognir, pygopherd descendants, phd), Veronica-2 search at Floodgap, an active phlog culture, and client support everywhere (Lagrange, lynx, Bombadillo, Offpunk, BFG, Overbite for Firefox, gopher.mills.io and other web proxies). Supporting gopher reaches a real community; Spartan and Nex reach a rounding error of that community. This inverts the effort ordering against the reach ordering: gopher costs the most and delivers the most.

### 1.5 Effort

**Largest of the three, estimate 6-10× Nex.** Listener: small. The cost is the render target (item typing, wrapping, TAB hygiene, `URL:` shim, menu generation for every page and directory), dot-stuffing/Lastline framing, caps.txt, the no-vhost config constraint, privileged-port documentation, and testing against picky clients (lynx is the strictness benchmark). Optional gophers/TLS sniffing adds a dispatcher but reuses existing TLS plumbing.

---

## 2. Spartan

### 2.1 Wire format

Canonical spec: `spartan://spartan.mozz.us/specification.gmi` (michael-lazar; mirrored at github.com/michael-lazar/spartan and via portal.mozz.us). Plaintext TCP, default **port 300** (also privileged: same workarounds as port 70).

- **Request:** one ASCII line `host SP path-absolute SP content-length CRLF`, followed by `content-length` bytes of data block. Host carries no port (IDNs as punycode); path must start with `/`; content-length is 0 for plain fetches. Because the request names the host, **Spartan supports virtual hosting**: unlike gopher and Nex.
- **Query strings become uploads:** a `spartan://host/path?text` URL is sent as a request whose data block is the percent-decoded query. So "input" and "upload" are the same mechanism.
- **Response:** one status line then optional body. `2 SP mimetype CRLF body` (success; e.g. `text/gemini;charset=utf-8`: UTF-8 is the default for text/*); `3 SP path-absolute CRLF` (redirect: **same-host only, path-absolute only**); `4 SP errormsg CRLF` (client error); `5 SP errormsg CRLF` (server error). No 1x input class, no auth class, no TLS, no client certs.
- **Gemtext dialect:** default document type is text/gemini plus one extra line type: `=:`: like `=>` but instructs the client to prompt for input and send it as the data block of the request. Ordinary gemtext is valid Spartan gemtext; `=:` only matters for pages that want input/upload UI.

### 2.2 Server requirements

- TCP listener; read one CRLF-terminated request line (bound its length, ~4 KiB), parse three fields strictly, then read exactly content-length bytes (bound it: for a static server, any content-length > 0 can be rejected before reading: respond `4 upload not supported here` and close, which also neutralizes resource-exhaustion via huge declared lengths).
- Vhost dispatch on the host field (falls out of usv's existing Gemini vhost model).
- Timeouts as for gopher; one request per connection.
- Status mapping from usv's internal handler model is mechanical: success→`2 mime`, redirect→`3 path` (only expressible if the target is same-host and path-absolute: cross-host redirects must be rendered as a `4` with a message, or better, avoided by the renderer emitting direct links), not-found/bad-request/gone→`4 msg`, internal→`5 msg`. Gemini's 1x (input) has no equivalent, Spartan pages request input via `=:` lines instead; Gemini's 6x (certs) has no equivalent and any cert-gated content must simply not exist in the Spartan tree.
- **Uploads:** Spartan has built-in uploads where Gemini needed Titan bolted on. For usv the honest v1 stance is "reject all uploads with 4"; a later bridge could route Spartan uploads into the same handler as Titan writes, but note the security asymmetry: Titan-over-Gemini can require client certificates and runs inside TLS; Spartan uploads are plaintext and unauthenticated (a shared-secret path token is the strongest possible control). Recommend documenting Spartan upload support as permanently out of scope for authenticated writes.

### 2.3 Render requirements

Nearly zero. Spartan's document format *is* gemtext; usv's existing gemtext output can be served byte-identical (the `=:` line type is additive and a static site has no reason to emit it). The only rendering consideration: absolute self-links inside content should use the `spartan://` scheme on this listener, which argues for either scheme-relative/relative links in generated pages (usv should prefer relative links anyway) or a tiny link-rewriting pass. MIME table is shared with Gemini. No new output tree needed: same files, second listener.

### 2.4 Adoption 2026

Small and static since ~2021. The spec is finished and stable (3 commits). Client support is decent among multi-protocol clients: Lagrange, Offpunk, gelim, Profectus, BFG; libraries exist (spartan-py, spartoi/Drogon, sybaritic). Server/capsule count is a few dozen hobby capsules at best; spartan.mozz.us remains the hub. It costs little to support and reaches little; its main value for usv is philosophical completeness ("the no-TLS mirror of the Gemini tree") at near-zero render cost.

### 2.5 Effort

**Middle: estimate 2-3× Nex, well under half of gopher.** Listener with three-field parse, content-length handling/rejection, status mapping, vhost wiring, tests. No render work beyond link-scheme policy.

---

## 3. Nex

### 3.1 Wire format

Canonical spec: `nightfall.city/nex/info/specification.txt` (m15o); also mirrored over HTTPS. Plaintext TCP, **port 1900**: note this one is *unprivileged* (>1024). (TCP 1900 does not collide with SSDP/UPnP, which is UDP 1900; worth one line in docs because port scanners tag 1900 as SSDP.)

- **Request:** the client sends a path (possibly empty) terminated by a newline; the server writes the response and closes. Explicitly telnet-compatible and stateless. No host field: **no virtual hosting**, same constraint as gopher.
- **Response:** the document content, as-is. No status line, no MIME type, no termination marker beyond connection close. Content type is inferred by the client from the path extension; extensionless = plain text.
- **Directory convention:** an empty path or a path ending in `/` is a directory. A directory listing is plain text in which any line beginning `=>` followed by a URL (absolute `nex://...` or relative `about.txt`, `../nexlog/`) is a link: i.e., Nex reuses gemtext's link-line syntax inside plain text. No status codes exist; "not found" is by convention just a human-readable text response (and/or closing the connection).

### 3.2 Server requirements

Minimal by design: read one line (bound length, timeout), canonicalize path (same traversal rules), map directories to a listing document, stream bytes, close. Error mapping: every internal error class collapses to a plain-text body ("not found", "server error") since the protocol has no signaling, document that clients cannot distinguish an error page from content (this matters for the honesty section: a Nex mirror cannot even signal failure machine-readably).

### 3.3 Render requirements

Smallest possible. Options: (a) serve the gemtext tree as-is, Nex clients display `.gmi` by extension heuristics, and `=>` lines are already links in Nex's own convention; or (b) a light "nexify" pass: strip/render heading markers, wrap body text to ~78 columns, keep `=>` link lines verbatim, rename `index.gmi` content to the directory listing. Option (b) is a ~day of renderer work and produces idiomatic Nex; either way there is no structural transformation, because Nex's hypertext model is a subset of gemtext's. Directory listing generation reuses the index-page metadata the pipeline already has.

### 3.4 Adoption 2026

Tiny but alive: nightfall.city (m15o's hub, with the "Nightfall Express" nex.nightfall.city zine and user directories) remains active: nex content dated late 2025 is observable, and m15o remains active in the small-net scene (status.cafe, smol.pub lineage). Clients: Lagrange, Profectus, gelim, BFG, various one-file clients. Server count: a handful beyond nightfall.city. Nex support is a gesture of small-net citizenship more than a reach play.

### 3.5 Effort

**Smallest: the baseline unit.** A listener of a few hundred lines plus an optional light render pass. Realistically 1-2 days including tests.

---

## 4. Cross-cutting architecture notes for usv

**Listeners.** All three are one-shot, line-framed TCP protocols: read one line (with per-protocol grammar), optional body (Spartan only), write, close. A single "plaintext one-shot listener" abstraction parameterized by (request parser, response writer) covers all three; gopher adds response-side framing (Lastline/dot-stuffing for menus and text). Shared needs: per-connection read timeout (~5 s), write timeout, request-line length caps, connection-count limits, and the same accept-loop/backpressure machinery the Gemini listener uses. None support keep-alive; none need TLS (gophers sniffing optional, § 1.1).

**Render pipeline.** Nex: none-to-trivial (serve gemtext, optionally nexify). Spartan: none (serve the gemtext output tree; prefer relative links so schemes never need rewriting). Gopher: a full third output target producing menus per page/directory plus typed static files: this is where the render budget goes. All three targets should be opt-in per site in config, mirroring the existing gemtext/HTML dual-render structure so feeds/indexes stay a single metadata pass with N emitters.

**Status/error mapping** from usv's internal handler results:

| usv internal | Gemini | Spartan | Gopher | Nex |
|---|---|---|---|---|
| success | 20 + MIME | `2 mime` | typed body (menu/text/binary) | body |
| redirect | 30/31 | `3 path` (same-host only; else render as error/direct link) | none: emit menus with final targets | none: emit final targets |
| not found | 51 | `4 msg` | type-3 line | "not found" text (by convention) |
| bad request | 59 | `4 msg` | type-3 line | text |
| server error | 40/50 | `5 msg` | type-3 line | text |
| input / certs | 10-11 / 60-62 | no equivalent (`=:` / n/a) | no equivalent | no equivalent |

Because gopher and Nex cannot express redirects, the gopher/nex render targets must resolve any internal redirects at generation time: a pipeline requirement, not a listener one.

**Ports.** 70 and 300 are privileged; 1900 is not. Defaults should be non-privileged (e.g. 7070/3000/1900) with documented paths to canonical ports: CAP_NET_BIND_SERVICE / systemd socket activation / sysctl / NAT redirect, and on Cloudron the manifest `tcpPorts` mapping (which also surfaces the ecosystem.md concern that extra listeners are an operational cost there: off-by-default is the right posture).

**Security and the trust model (be honest in docs).** All three are cleartext: no confidentiality, no integrity, no server authentication (except optional gophers), and **no client authentication possible**, so no cert-gated content, no authenticated uploads, ever, on these listeners. Consequences to document plainly: (1) anything published on these listeners is world-readable in transit and trivially tamperable by any on-path party: serve only content whose integrity loss is acceptable, and never mirror cert-gated Gemini content into these trees; (2) requests and responses are visible to networks, so operator *logging* is not the main privacy story, but usv should still apply its existing log-minimization policy (truncate/omit selectors and IPs per config) since plaintext protocols attract scanners and the logs will be noisy; (3) Spartan uploads are unauthenticated by construction: keep them rejected; (4) these listeners must be off by default, and enabling one should print/document a one-line trust disclaimer. The counterweight: this is the norm of these communities: gopher has run cleartext for 35 years, and usv is mirroring public static content, which is the one workload where cleartext is defensible.

**Effort ranking (validated): Nex (1×) < Spartan (2-3×) < Gopher (6-10×).** The prompt's presumed ordering holds. Gopher's cost is concentrated in the renderer, not the listener; Spartan's in status/upload handling; Nex has almost none.

---

## 5. Survey: other smolnet protocols

**Guppy** (dimkr, github.com/dimkr/guppy-protocol). Unencrypted **UDP** (port 6775, `guppy://`), inspired by Gemini/Spartan/TFTP: request/response with sequence numbers and client acks so multi-datagram responses can be reassembled; goal is servers small enough for microcontrollers. Spec complete; implementations: gplaces and Lagrange client-side, sample C/Python servers. For usv it would be the only UDP listener in the codebase: new transport semantics (retransmission, ack tracking, amplification-abuse concerns) for an audience of perhaps a dozen capsules. **Verdict: ignore for scheduling; watch only if Lagrange-driven adoption visibly grows.**

**Misfin** (originally JCLemme, spec continued at sr.ht/~lem/misfin; `misfin://`). Gemini-derived *mail*: TLS-mandatory, sender identity = TLS client certificate (UID=mailbox, CN=display name), TOFU both ways, message = one gemtext string ≤2048 chars, Gemini-compatible status codes. Lagrange 1.18 (2024) added Misfin support, and small server implementations exist (titlani, clseibold's misfin-server). It is genuinely alive, but it is a *mailbox* protocol: a Misfin server receives and stores messages, manages identities, and has spam/abuse surface: nothing shared with a static content pipeline except gemtext. **Verdict: out of scope by kind: ignore in usv; note in docs that operators wanting a capsule contact address can run a standalone misfin server beside usv.**

**Mercury** (solderpunk, gemlog "The Mercury protocol", 2020). Gemini minus TLS as a *thought experiment*: 14 status codes, mandatory UTF-8, explicitly not a project. Never implemented beyond toys; Spartan occupies this niche with an actual spec and clients. **Verdict: dead: ignore.**

**Finger** (RFC 1288, TCP **79**: privileged). Request is `username CRLF` (or empty for a listing; `/W` verbose flag); response is free text; close. The smolnet uses it for `.plan`-style status pages (tilde servers, happynetbox-style services), and client support is surprisingly good: Lagrange, Bombadillo, BFG all speak `finger://`. Implementation is *cheaper than Nex*, but the content model is user-centric queries, not paths, so it maps awkwardly onto a content tree (best fit: expose configured "profiles": e.g. finger `sitename` returns a generated status/about text, possibly from a designated gemtext file, wrapped to 78 cols). **Verdict: the only "other" worth putting on the schedule's reserve list: a half-day novelty listener with real client reach; schedule as stretch/optional or leave as a documented future toggle.**

**Scroll** (clseibold, scrollprotocol.us.to; `scroll://`). Gemini-derived with a *richer* document format and metadata; still effectively one implementer, with Profectus (beta 1.1: scroll/gemini/nex/spartan, by the same author) as the reference client. Nothing since the earlier recon changes the picture: no second independent implementation, no spec freeze evident. **Verdict: watch: unchanged from ecosystem.md.** Revisit only if a second implementer or a Lagrange adoption appears; supporting it would mean a genuine fourth document format in the renderer, so the bar should be high.

**SuperTXT** (supertxt.net). Not a request/response document protocol at all: an SSH-based stack ("SSHLA": content over scp/rsync/git with anonymous SSH, browsing shell tooling, even a WASM runtime). Interesting research project; zero overlap with usv's listener/render model, no shared client ecosystem. **Verdict: ignore.**

**"Twin" / others.** No verifiable smolnet protocol named Twin (or "Twin Peaks") could be located in 2026 searches; if the reference was to an entry in dbohdan's "Small Internet protocol roundup" (dbohdan.sdf.org/smolnet: the best single survey page; direct fetch failed on access date due to an SDF cert mismatch), nothing with visible adoption exists under that name. Also seen and dismissed: Gopher+ (dead, §1.1), text protocols of the tildeverse that are services rather than protocols (BBSes, SSH apps). **Verdict: nothing else warrants a line item.**

**Client reach matrix (2026)**, client support is what converts a listener into an audience:

| Client | gemini | titan | gopher | spartan | nex | finger | guppy | misfin | scroll |
|---|---|---|---|---|---|---|---|---|---|
| Lagrange | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |: |
| Profectus | ✓ |: |: | ✓ | ✓ |: |: |: | ✓ |
| gelim | ✓ |: |: | ✓ | ✓ |: |: |: |: |
| Offpunk | ✓ |: | ✓ | ✓ |: |: |: |: |: |
| Bombadillo | ✓ |: | ✓ |: |: | ✓ |: |: |: |
| BFG (luxferre) | ✓ |: | ✓ | ✓ | ✓ | ✓ |: |: |: |
| lynx / Overbite |: |: | ✓ |: |: | ✓(lynx) |: |: |: |

Reading: the scheduled trio (gopher, spartan, nex) plus finger is exactly the plaintext set Lagrange and the TUI ecosystem collectively cover; gopher additionally reaches clients that speak nothing else modern (lynx, Overbite, classic gopher clients). The "smolnet portal" (portal.mozz.us: the community's main web proxy, renamed from Gemini Portal in 2023 when gopher support landed) proxies gemini and gopher, so gopher support also earns web-proxy visibility. Community guides (yesterweb, kevinboone, communitywiki "SmolNet") consistently define the smolnet as **Finger + Gopher + Gemini**, which supports treating gopher as the priority and finger as the cheap completeness play.

---

## 6. Gopher announcement venues (verified 2026-08-09)

- **gopher-project mailing list**: the historical gopher list lives at `gopher-project@other.debian.org` with archives at lists.debian.org/gopher-project/ (moved there from alioth; the narkive mirror documents the move). Active and the canonical place to announce a new server.
- **IRC**: `#gopher` and `#gopherproject` on irc.libera.chat; also the bitreich community (bitreich.org, IRC on their own infra) for the geomyidae/gph school of gopherdom.
- **Bongusta** (gopher://i-logout.cz/1/bongusta): the long-running phlog aggregator; still referenced by current phlog rolls (gopher.black, oddnugget mirrors). Inclusion is by request to its maintainer; being on Bongusta is how phlogs get read. (Liveness verified only indirectly via aggregator listings on access date.)
- **Floodgap / Veronica-2**: gopher.floodgap.com maintains server lists and runs the Veronica-2 search crawler; getting linked or submitting there gets a new server indexed.
- **r/gopher**: exists, low-traffic; secondary venue. **lemmy.sdf.org** has an active gopher community thread culture and SDF itself remains a major gopher host.

For usv's docs: recommend operators announce on the mailing list + Bongusta (if phlogging) and ensure the root menu links `caps.txt` and an about page, which is what Veronica-2 and humans will land on.

---

## Sources

All accessed 2026-08-09. Where content is served natively over gopher/gemini/spartan/nex, the HTTPS mirror or proxy actually fetched is listed.

- <https://www.rfc-editor.org/rfc/rfc1436.txt>: RFC 1436, gopher wire format, item types, Lastline, dot-stuffing, encoding rules.
- <https://raw.githubusercontent.com/gophernicus/gophernicus/master/README.gophermap>, gophermap conventions: tab rules, field defaulting, `i` auto-typing, `h`+`URL:` links, gophernicus extensions (`!title`, `*`, `=`, `.` etc.).
- <https://github.com/gophernicus/gophernicus>: gophernicus daemon (caps.txt auto-generation, filetype table incl. `i`, `d`, `s`).
- <http://r-36.net/scm/geomyidae/file/geomyidae.8.html>, geomyidae manpage: `.gph` index files, `i`-prefixing of tabless lines, raw-tab passthrough, executable gph, TLS via sticky bit (via search).
- <https://gopher-project.alioth.debian.narkive.com/aTTl0Qdp/caps-txt-complete-syntax>: caps.txt syntax discussion (CapsVersion, ExpireCapsAfter, PathDelimeter, ServerSoftware) on the gopher list (via search).
- <https://dataswamp.org/~solene/2019-03-07-gopher-server-tls.html> and the community GoT draft discussed in search results, gopher-over-TLS: same-port detection (first-packet CRLF = plaintext, else TLS handshake, ALPN `gopher`), client fallback and caching; `gophers://` scheme in Lagrange (via search).
- <https://datatracker.ietf.org/doc/html/draft-matavka-gopher-ii-03>: Gopher-II/caps context (historical; not adopted).
- <http://portal.mozz.us/spartan/spartan.mozz.us/specification.gmi>, Spartan spec: request line `host SP path SP content-length CRLF`, data block, query-string-as-upload, statuses 2/3/4/5 with payload grammar, `=:` line, port 300, UTF-8 default.
- <https://github.com/michael-lazar/spartan>, Spartan spec repo (canonical homes: gemini:// and spartan://spartan.mozz.us); 3 commits, stable/finished.
- <https://sr.ht/~hedy/spartan-py/>, <https://github.com/marty1885/spartoi>, <https://pypi.org/project/sybaritic/>, <https://jdcard.com/SpartanClients.gmi>: Spartan libraries/clients (via search).
- <https://nightfall.city/nex/info/specification.txt>, Nex spec: port 1900, path request, as-is response, `=>` link lines, trailing-slash directories, extension-based typing.
- <https://nightfall.city/nex/in/m15o/> and <https://nex.nightfall.city/classifieds/2025-11-15-050547.txt>: nightfall.city/m15o activity through late 2025 (via search).
- <https://github.com/dimkr/guppy-protocol> (index.gmi, guppy-spec.gmi), Guppy: UDP 6775, `guppy://`, TFTP-inspired, microcontroller goal; client/server implementations list (via search).
- <https://github.com/JCLemme/misfin> and <https://sr.ht/~lem/misfin/>, Misfin spec: TLS client-cert identities (UID/CN/SAN), TOFU, 2048-char gemtext messages, Gemini-compatible statuses (via search).
- <https://gmi.skyjake.fi/gemlog/2024-09_lagrange-1.18.gmi>, Lagrange 1.18: TUI + Misfin support (via search).
- <https://fuwn.me/x/gemini.circumlunar.space/~solderpunk/gemlog/the-mercury-protocol.gmi>, Mercury: TLS-less Gemini thought experiment, 14 statuses, never productized (via search).
- <https://www.rfc-editor.org/rfc/rfc1288.html>: Finger protocol, TCP 79, request grammar, .plan/.project.
- <http://scrollprotocol.us.to/software/profectus/> and <https://pkg.go.dev/gitlab.com/clseibold/profectus>, Scroll/Profectus status: beta 1.1, scroll+gemini+nex+spartan, single-author ecosystem (via search).
- <https://supertxt.net/> (whats-sshla.html, hosting.html), SuperTXT: SSH-layer content stack, command-oriented, WA-Nine (via search).
- <http://dbohdan.sdf.org/smolnet/>, "Small Internet protocol roundup" survey page (direct fetch failed on access date: SDF vhost cert mismatch; cited as the reference survey via search snippets).
- <https://apps.apple.com/us/app/lagrange-smallnet-browser/id1554714615> and <https://man.uex.se/1/lagrange>, Lagrange protocol list: gemini, titan, gopher, finger, spartan, nex, misfin, guppy (via search).
- <https://bombadillo.colorfield.space/> and Debian manpage, Bombadillo: gopher, gemini, finger, local first-class (via search).
- Offpunk protocol list (gemini, gopher, http/https, spartan, mailto, file): via awesome-gemini and project docs (via search).
- <https://codeberg.org/luxferre/BFG>, BFG client: gopher, finger, nex, spartan, gemini (via search).
- <https://portal.mozz.us/about>, Smolnet Portal history: gemini + gopher proxying, 2023 rename (via search).
- <https://lists.debian.org/gopher-project/> and <https://gopher-project.alioth.debian.narkive.com/MsgvhbvV/gopher-mailing-list-moved>: gopher mailing list current home and move history (via search).
- <https://alexschroeder.ch/view/2018-01-29_Bongusta>, <https://www.oddnugget.com/oddgopherpage/gopher.black:70/phlogs>: Bongusta phlog aggregator and current phlog rolls referencing it (via search).
- <https://gopher.floodgap.com/overbite/>: Overbite project and Floodgap ecosystem (Veronica-2, server lists) (via search).
- <https://communitywiki.org/static/SmolNet.html>, <https://kevinboone.me/web-adjacent.html>, <https://wiki.archiveteam.org/index.php/SmolNet>: community definitions of the smolnet (finger+gopher+gemini) and guides (via search).

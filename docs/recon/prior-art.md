# Prior-Art Autopsy: Existing Gemini Servers

**Project:** Unseen Servant (`usv`) — Gemini server in Rust, targeting Cloudron deployment.
**Phase:** 0, item 3.
**Date:** 2026-08-09.

## Summary

Six servers and one test suite were examined, plus the Rust crate ecosystem named in the project brief for ADR 0001. The field divides cleanly into two camps. The minimalists (Agate, Molly Brown) prove that a Gemini server can be feature-complete in a few thousand lines, and that the features users actually need beyond static serving are a short list: certificate lifecycle automation, redirects, per-directory metadata, and client-certificate gating. The maximalist (gmid) proves that FastCGI, proxying, Titan, and a real config language are what a server accumulates over five years of users asking, and its privilege-separation architecture is the security high-water mark — though most of it is OS-process theater that a Rust binary inside a Cloudron container should replace with type safety, rustls, and container isolation rather than imitate. Jetforce demonstrates the app-server/framework shape (routing, Request/Response objects) that Unseen Servant should expose internally but not necessarily as a public API. twins demonstrates the cost of feature sprawl on a hobby maintenance budget: its proxying is the source of most of its bug reports.

On the ADR 0001 question: neither "titanite" nor "gemax" is a viable foundation — titanite is an early-development YGGverse library with a "project in development" warning, and **gemax is not a Rust crate at all; it is a Go library** (ninedraft/gemax) and should be struck from the Rust building-blocks list in the brief. The historically notable Rust crates (northstar→twinstar, gemserv) have been unmaintained since 2022. Only windmark is alive, and it is a framework with its own opinions, not a wire-protocol library. The recommendation at the end of this document is a clean implementation of the wire protocol (~one page of logic) on tokio + rustls, stealing Agate's certificate lifecycle design and gmid's configuration semantics.

A Cloudron-specific data point: a 2021–2022 Cloudron forum thread requested a molly-brown package and it was never built — the niche is genuinely empty, and the thread's identified hurdles (TLS cert management for a non-HTTP protocol, port allocation, persistent storage, backup/restore) are exactly the problems Unseen Servant's Cloudron packaging must answer.

---

## 1. Agate (Rust)

**Language / scope:** Rust, async (tokio + rustls). Static files only, by explicit policy: "Agate can only serve static files." No CGI, no scripting, no request rewriting. Feature-frozen by design; maintenance releases only.

**Maintenance status (2026):** Actively maintained. Latest release 3.3.22 on 2026-04-19 (security fix: escaping URLs in log output). Repo pushed 2026-08-03; 743 stars; 7 open issues. Most releases are dependency bumps — this is what a finished project looks like. Dual-licensed Apache-2.0/MIT.

**Certificate lifecycle (study this closely — it is the best in class):**
- On first run, Agate auto-generates a self-signed certificate and key for each configured hostname. No manual OpenSSL invocation, no setup step.
- Expiry date is set to **4096-01-01** — effectively never, which is correct under Gemini's TOFU (trust-on-first-use) model where CA-style expiry adds churn without security.
- Keys are **ECDSA** (it also accepts user-supplied RSA/Ed25519), stored in **DER** format (X.509), with PEM conversion documented for users bringing their own certs.
- Certificates live in **per-hostname subdirectories** under a certificates directory, with hierarchical matching: a more specific domain directory overrides the default. This gives multi-hostname vhosting with zero config-file syntax.

**Config/CLI approach:** No config file. Everything is CLI flags: content root, listen addresses, hostnames, default language. Per-file/per-directory behavior is controlled by in-tree sidecar files: `.meta` files (INI-like, glob patterns) override MIME types and can preset status codes/redirects; a `.directory-listing-ok` marker file enables listings for that directory. Config-as-files-in-the-content-tree survives container redeploys naturally.

**What users ask for (issue tracker):** man pages (#425); SIGINT/SIGTERM handling in Docker (#418 — container signal handling matters); directory listing refinements (#417); SNI behavior with I2P (#409); Spartan protocol support (#385, refused — scope policy holds); TLS certificate reload without restart (#412); custom certificate handling and auto-generated cert file permissions (#404, #414); migration off `ring` when its maintenance wavered (#377). Old closed issue #35 asked for CGI — refused. The dominant themes are **certificate management ergonomics and container behavior**, not features.

**Adopt:**
- The entire certificate lifecycle: auto-generate ECDSA per hostname on first run, expiry 4096-01-01, per-hostname cert subdirectories, accept user-supplied certs in the same slots.
- Sidecar metadata files (`.meta`-style) for MIME overrides and per-path status presets.
- The "dependency-bumps-are-the-changelog" maintenance posture as the long-term goal.
- Handle SIGTERM/SIGINT cleanly from day one (Cloudron stops containers with SIGTERM); Agate had to retrofit this.
- Certificate/key file permission hygiene (0600) at generation time.

**Refuse:**
- CLI-flags-only configuration. Cloudron packaging wants a config file (or env-var) surface; flags-only makes the Dockerfile the config file.
- The DER-only default. PEM is what every other tool produces; accept both.
- Its refusal of dynamic content is right for Agate but Unseen Servant should decide CGI/SCGI on its own merits (see gmid/Molly Brown).

## 2. gmid (C, omar-polo)

**Language / scope:** C. The renown leader; a "full-featured Gemini server written with security in mind." Static serving, FastCGI, reverse proxying, vhosts, location blocks, IRI/IDN support, directory listings, Titan support via bundled tooling, plus companion tools: `gg` (Gemini CLI client), `gemexp` (config-less quick-share server), and a `titan` upload client.

**Maintenance status (2026):** Very active. Latest release 2.1.1 (2024-08); commits through **August 2026** (punycode overflow checks, imsg API updates, sandbox improvements). Packaged in many distros. Hosted at Codeberg (op/gmid) with mirrors; releases are signify-signed.

**Privilege-separated process design:** Four processes communicating over OpenBSD-style `imsg` framed pipes:
- **main** — retains privileges; loads TLS certificates, handles config reload (SIGHUP) and log rotation (SIGUSR1), forks the others.
- **logger** — the only process that touches syslog/log files.
- **server** — accepts connections, parses requests, does FastCGI/proxying; sandboxed (pledge/unveil on OpenBSD; historically capsicum/seccomp/landlock elsewhere, dropped in 2.0 in favor of a simpler model).
- **crypto** — holds TLS private keys, so a compromised server process cannot exfiltrate them; signing operations cross the imsg boundary.

**What translates to Rust-in-a-container, and what doesn't:** The *goals* translate; the *mechanism* mostly doesn't. In a Cloudron container there is one app process, the container boundary provides the filesystem/namespace isolation that unveil/pledge provided, and Rust's memory safety removes the class of bugs (parser buffer overflows) that made isolating the request parser from the key material worthwhile in C. Spawning four processes inside a container adds supervision complexity (PID 1 problems, signal fan-out) for marginal gain. What *is* worth keeping: (a) the **logger boundary** as an architectural seam — one task owns log output; (b) the **crypto boundary** re-expressed in the type system — private keys live inside rustls's `ServerConfig`/key provider and are never readable by request-handling code, no raw key bytes floating through handler scope; (c) the **reload discipline** — SIGHUP re-reads config and certs without dropping the listener; (d) dropping root and running as an unprivileged user (Cloudron does this for you via the manifest, but don't fight it). Verdict: single process, multiple tokio tasks, capability-shaped module boundaries.

**Config language:** OpenBSD httpd(8)-inspired: `server "example.com" { listen on * cert "..." key "..." root "..." location "/foo/*" { ... } }` with macros, includes, and variable substitution. This is the most pleasant config surface in the field and the reason gmid wins multi-site deployments.

**Titan:** Supported ecosystem-side (bundled titan client; upload handling via FastCGI backends). Titan (`titan://`) is the de-facto upload companion protocol to Gemini.

**Testing:** A real regression suite — `make regress` — that spins the server on ports 10965–10966 and exercises config parsing, request handling, vhosts, FastCGI, and proxying end-to-end. gmid is the only server in this survey with serious in-tree integration tests.

**What users asked for over its history (ChangeLog archaeology):** the evolution ran CLI-flags → config file → location blocks → vhosts → macros/includes; CGI → FastCGI (CGI was removed in favor of FastCGI); proxy-protocol v1 for load-balancer fronting; IPv6 in proxy configs; OCSP stapling; EC key generation; automatic certificate renewal; per-location logging control; syslog facility selection; NUL-byte and path-validation hardening. This is the demand curve Unseen Servant will face if it succeeds.

**Adopt:**
- The regress-suite discipline: end-to-end tests that run the real binary against real sockets, in-tree, from day one.
- Config semantics: named server blocks, location matching, sane defaults; SIGHUP reload of config + certs without socket drop.
- FastCGI/SCGI as *the* dynamic-content escape hatch (rather than in-process CGI), if dynamic content is in scope at all.
- IRI/IDN correctness and punycode handling (gmid is still fixing overflow bugs here in 2026 — budget for this being subtle).
- The logger/crypto boundaries re-expressed as Rust module/task boundaries as described above.

**Refuse:**
- Multi-process privsep and imsg. Wrong tool inside a container; single tokio process.
- A yacc-grade custom config grammar. Use an existing serde format (TOML) with gmid's *semantics*, not its parser.
- proxy-protocol, OCSP stapling, and reverse-proxying in v1 — these are gmid's year-five features, not phase-one features.

## 3. Molly Brown (Go, solderpunk)

**Language / scope:** Go, by Gemini's creator. "Full-featured Gemini server suitable for use in pubnix or similar shared-hosting environments." Single external dependency (BurntSushi/toml). Runs on Linux, *BSD, 9front; uses pledge(2)/unveil(2) on OpenBSD. BSD-2-Clause.

**Maintenance status (2026):** Alive at low simmer. Latest pseudo-version published 2026-05-25 (per pkg.go.dev). Primary repo tildegit.org/solderpunk/molly-brown (behind Anubis anti-scraper; mirrors exist at LukeEmmet/molly-brown and others).

**Feature set:**
- TOML config (`/etc/molly.conf`): port, hostname, cert/key paths, docroot, log paths, default language, MIME defaults.
- `~username` URL support (its pubnix heritage).
- Auto directory listings with sort options (name/size/time), option to use first `# heading` of a .gmi file as its display name, and **`.mollyhead`** files providing a custom header block prepended to generated listings.
- **`.molly` files**: per-directory, cascading config overrides (`.htaccess` analog), enabled by `ReadMollyFiles = true`; only a whitelisted subset of settings can be overridden (cert zones, language, sort, MIME overrides, redirects, gemini extension).
- Redirects: temporary (30) and permanent (31) via regex with capture-group substitution.
- CGI (world-executable files under configured paths, RFC 3875-ish env, 10-second timeout) and SCGI (Unix sockets, persistent app servers, better privilege separation than CGI).
- **Certificate zones**: per-path access control by client-cert SHA256 fingerprint allowlist, "analogous to SSH's authorized_keys"; unlisted certs get status 60.
- World-readability of files as the publishing switch (deliberate: users control exposure with chmod, not config).

**What its simplicity gets right:** The feature list above *is* the complete list of things Gemini users have demonstrably wanted for six years, and it fits in one small Go program with one dependency. The `.molly`-file whitelist model — delegated per-directory config, but only for settings that can't compromise the server — is the right answer to multi-tenant configuration. Certificate zones are the simplest possible client-cert story and map perfectly onto Gemini's status-6x semantics.

**What users ask for:** virtual hosting (documented as planned/absent — the one structural gap vs. Agate/gmid); the ecosystem's forks (LukeEmmet, raingloom, tallship, idiomdrottning) mostly carry small patches, indicating the core is considered done.

**Adopt:**
- Certificate zones exactly as designed: path-scoped SHA256 fingerprint allowlists → status 60/61.
- Regex redirects with capture groups (30/31).
- The whitelist model for any per-directory override file (never let content-tree files change listen addresses, docroots, or cert paths).
- TOML as the config format.
- CGI's 10-second hard timeout if CGI is ever implemented; prefer SCGI/FastCGI as Molly Brown itself hints.

**Refuse:**
- `~username` expansion and world-readable-bit semantics — pubnix concerns, meaningless in a single-tenant Cloudron container.
- In-process CGI as a v1 feature (its own README flags the security caveat: CGI runs as the server user).
- Its lack of vhosts; Unseen Servant should have Agate-style multi-hostname support from the start because Cloudron apps can have aliases.

## 4. twins (Go, tslocum)

**Language / scope:** Go. Static serving plus per-path reverse proxying (to `gemini://`, `gemini-insecure://`, TCP, and FastCGI backends), command execution, Gemini-to-HTTPS conversion via gmitohtml integration, directory listings, SIGHUP config reload. YAML config with a `paths:` list per host — per-path routing is its signature idea. Now hosted at codeberg.org/tslocum/twins (rocket9labs domain currently unresolvable — mirror fragility noted).

**Maintenance status (2026):** Barely maintained. Last substantive commit 2025-11-20 (removing deprecated ioutil). Issue tracker shows long-standing proxy bugs: response bodies cropped (~80% truncation, issue #18), images through the proxy loading only intermittently, path-stripping only partially working (#10). twins also *fails gemini-diagnostics' close_notify check* (diagnostics issue #4) — an object lesson: proxying multiplies your connection-lifecycle states, and getting close_notify right through a proxy hop is exactly the kind of thing that silently breaks.

**Got right:** per-path routing as a first-class config concept; SIGHUP reload; a PROPOSALS.md explicitly separating non-standard extensions from core behavior.

**Users ask for:** mostly proxy correctness fixes — i.e., the feature beyond static serving generated the majority of the defect load.

**Adopt:** the per-path routing *concept* (a path can map to files, a redirect, or a backend) as internal architecture; documentation that separates spec behavior from extensions.

**Refuse:** reverse proxying itself in v1 (the twins tracker is the evidence file for why); YAML (footgun-rich; TOML instead); HTTP conversion built into the server.

## 5. Jetforce (Python, michael-lazar)

**Language / scope:** Python 3.9+, built on Twisted. Self-described "experimental." Both a working server and a framework: `JetforceApplication` provides decorator/regex routing over `Request` → `Response(status, meta, body)` handlers; `StaticDirectoryApplication` (static files, directory listings, index files, CGI per simplified RFC 3875) is itself just an app composed on the framework. Client-cert auth with optional CA validation; virtual hosting via composing apps in Python config; rate limiting. Floodgap Free Software License.

**Maintenance status (2026):** Alive but quiet: last push 2026-02-09; 217 stars. Its "experimental" label has held for six years — it is the reference *pedagogical* implementation more than a production recommendation.

**Relationship to gemini-diagnostics:** the diagnostics script originated on the Gemini mailing list and was initially bundled inside the Jetforce repo, then extracted to its own repository. Jetforce is effectively the reference implementation the torture test was calibrated against.

**Got right:** the app-server split. Routing + Request/Response objects + "static serving is just an app" is the cleanest internal architecture in the field, and it is what makes client-cert zones, CGI, and vhosts composable rather than special-cased.

**Users ask for:** framework niceties (middleware, better vhost ergonomics) — the framework surface invites feature requests the way static servers don't.

**Adopt:** the internal shape — a `Handler` trait taking a parsed request and returning `(status, meta, body-stream)`, with the static-file server implemented as one handler among possible others. This costs nothing now and buys certificate zones, redirects, and any future dynamic backend cleanly.

**Refuse:** exposing that framework as a public extension API in v1 (Unseen Servant is a server product, not a Rust framework — windmark already exists for that); Twisted-style dynamic config-is-code.

## 6. gemini-diagnostics (the gate before public exposure)

**What it is:** "A torture test for gemini servers" — a single Python script, run as `gemini-diagnostics [host] [port]`, with per-check selection and configurable inter-test delay. Originated from the Gemini mailing list, formerly bundled with Jetforce, now standalone at github.com/michael-lazar/gemini-diagnostics.

**Maintenance status:** Frozen but canonical. Last push 2022-07-22; ~40 commits; 27 stars; 3 open issues. It remains the community-standard server gate; no widely-adopted fork tests more. Sean Conner's separate "torture test" (gemini.conman.org) targets *clients*, not servers, and is not a substitute.

**The complete check list (Unseen Servant MUST pass all of these):**

1. **IPv4Address** — server resolves and accepts a connection over IPv4.
2. **IPv6Address** — server resolves and accepts a connection over IPv6 (skips IPv4-mapped addresses).
3. **TLSVersion** — negotiated TLS is ≥ 1.2; 1.3 preferred.
4. **TLSClaims** — certificate notBefore/notAfter are valid at test time; hostname matches subject CN or subjectAltName.
5. **TLSVerified** — reports whether the cert is self-signed (verify code 18) or CA-signed (informational under TOFU).
6. **TLSCloseNotify** — server sends a TLS close_notify alert before closing the connection. (Frequently failed — twins fails it; get this right in the response-completion path.)
7. **TLSRequired** — plain-text (non-TLS) requests are refused: connection closed or no response.
8. **ConcurrentConnections** — two simultaneous connections can be held open.
9. **ResponseFormat** — root URL response: single space between status and meta, CRLF header terminator, sensible MIME type, non-empty body.
10. **HomepageNoRedirect** — root path returns a 2x success status (note issue #6: sites that redirect `/` get flagged; this is the suite's opinion, not the spec's).
11. **PageNotFound** — nonexistent path → status 51, empty body, correct header format.
12. **RequestMissingCR** — a request terminated by bare LF (no CR) gets no response / times out.
13. **URLIncludePort** — explicit `:1965` in the URL is accepted.
14. **URLSchemeMissing** — scheme-less URL → status 59.
15. **URLByIPAddress** — request by literal IP instead of hostname (server may 53 or serve; must not crash).
16. **URLInvalidUTF8Byte** — URL containing invalid UTF-8 → connection drop or 59.
17. **URLMaxSize** — exactly 1024-byte URL → handled (expects 51 for a nonexistent long path, not a parser error).
18. **URLAboveMaxSize** — 1025-byte URL → connection drop or 59.
19. **URLWrongPort** — URL naming a foreign port → status 53 (proxy request refused).
20. **URLWrongHost** — URL naming a foreign hostname → status 53.
21. **URLSchemeHTTP** — `http://` URL → status 53.
22. **URLSchemeHTTPS** — `https://` URL → status 53.
23. **URLSchemeGopher** — `gopher://` URL → status 53.
24. **URLEmpty** — empty request line → status 59.
25. **URLRelative** — relative path (no authority) → status 59.
26. **URLInvalid** — arbitrary garbage → status 59.
27. **URLDotEscape** — `/../`-style traversal above docroot → any 5x permanent failure.

**Known gaps (from its tracker and by inspection) — Unseen Servant's own test suite must cover these beyond the diagnostics gate:**
- **URLDotEscape has a false negative** (open issue #13) — passing it does not prove traversal safety; fuzz percent-encoded (`%2e%2e`, `%2f`), double-encoded, backslash, and NUL-injected paths separately.
- No client-certificate tests at all: nothing exercises status 60/61/62 flows, cert-zone gating, or expired/malformed client certs.
- No redirect-chain, status-1x (input), 44 (slow down), or 4x-transient testing.
- No SNI tests (multi-hostname cert selection), no TLS 1.3-only or session-resumption checks.
- No streaming/large-body, slow-client, or timeout (slowloris) tests; no Titan tests; no IRI/IDN tests.
- Opinionated checks: HomepageNoRedirect and RequestMissingCR-must-timeout are suite opinions; document any deliberate deviation.
- The script predates the spec's move to geminiprotocol.net (2023 spec split into protocol/gemtext documents); re-verify each expectation against the current spec when implementing.

## 7. Rust building blocks (input to ADR 0001)

**titanite** (crates.io `titanite` 0.3.2, updated 2025-02-24; github.com/YGGverse/titanite): Client/server library for Gemini with Titan support, written for YGGverse's "Titan it!" file-sharing server as a native-Rust successor to their Glib-based `ggemini`. ~37 commits, no GitHub releases, README carries a "Project in development!" warning, ~4.5k downloads. **Verdict: too immature to build a product on; single-vendor, single-consumer.**

**gemax:** Does not exist on crates.io. The name resolves to **ninedraft/gemax, a Go library** ("std-inspired gemini server and client implementation with no third party dependencies," ~165 commits, modest adoption). The project brief's premise that gemax is a Rust building block is incorrect; ADR 0001 should record this correction. Its only relevance is as another data point for the Jetforce-style app/handler API shape.

**northstar / twinstar** (crates.io `twinstar` 0.4.0): panicbit's Gemini server library, renamed from northstar (the `northstar` crate name now belongs to an unrelated embedded-Linux container runtime — cite carefully). Last published 2022-05-02. **Dead.**

**windmark** (crates.io `windmark` 0.7.0, updated 2026-05-29; github.com/gemrest/windmark): "elegant and highly performant async Gemini server framework." Router with `mount()`-based routing, optional `rossweisse` proc-macro struct-router, modules system (example: windmark-comments), supports both tokio and async-std, TLS configured from PEM key/cert files. ~257 commits, 15 stars, ~49k downloads, Apache-2.0/MIT. **The only living Rust Gemini server crate.** But it is a framework with its own routing/module opinions and a small bus factor; building Unseen Servant on it means inheriting its API surface, its TLS choices, and its release cadence for the 5% of the codebase that is actually protocol code.

**gemserv** (crates.io `gemserv` 0.6.6; sr.ht/~int80h/gemserv): Rust server with vhosts, CGI, SCGI, reverse proxying — the closest existing thing to a "full gmid in Rust." Last published 2022-02-18; survives only as scattered forks (GreatWizard, calacuda). **Unmaintained; useful as a source-reading reference for CGI/SCGI-in-Rust patterns, not as a dependency.**

**Agate itself** is the strongest Rust prior art: production-grade tokio + rustls Gemini code under Apache-2.0/MIT, from which specific mechanisms (cert generation via `rcgen`-style APIs, response streaming, `.meta` parsing) can be studied or adapted with attribution.

**Recommendation for ADR 0001:** implement the wire protocol cleanly. The Gemini protocol is deliberately tiny — read one CRLF-terminated URL line (≤1024 bytes), write `STATUS SPACE META CRLF` + optional body, close with TLS close_notify. That is under a page of logic; every hard problem (TLS server config, SNI multi-cert, client-cert capture, TOFU-friendly cert generation, timeouts, streaming) lives in tokio/rustls/rcgen configuration that a wrapper crate would merely obscure. Dependency set: `tokio`, `rustls` (+ `tokio-rustls`), `rcgen` (cert auto-generation), `percent-encoding`/`url` or a hand-rolled strict parser (decide in ADR 0002: strictness of URL parsing is a diagnostics-relevant behavior), `serde` + `toml`, `tracing`. No Gemini crates as dependencies.

---

## Sources

All URLs accessed 2026-08-09.

- https://github.com/mbrubeck/agate — Agate README (features, cert lifecycle, non-goals). Repo pushed 2026-08-03; not archived; 743 stars; 7 open issues.
- https://github.com/mbrubeck/agate/blob/master/CHANGELOG.md — release history; latest 3.3.22, 2026-04-19.
- https://github.com/mbrubeck/agate/issues?q=is%3Aissue — user demand themes (certs, Docker signals, Spartan refusal).
- https://gmid.omarpolo.com/ — gmid homepage; version 2.1.1; bundled gg/gemexp/titan tools.
- https://codeberg.org/op/gmid — gmid repo; privsep architecture (main/logger/server/crypto over imsg); `make regress` on ports 10965–10966; commits through August 2026.
- https://codeberg.org/op/gmid/raw/branch/master/ChangeLog — feature-evolution history (2.0 2024-01, 2.1/2.1.1 2024-08; FastCGI, proxy-protocol, OCSP, cert auto-renewal).
- https://tildegit.org/solderpunk/molly-brown — canonical repo (fetch blocked by Anubis anti-scraper on access date).
- https://github.com/LukeEmmet/molly-brown/blob/master/README.md — Molly Brown feature detail (.molly whitelist, .mollyhead, CGI/SCGI, certificate zones, redirects).
- https://pkg.go.dev/tildegit.org/solderpunk/molly-brown — latest pseudo-version published 2026-05-25; BSD-2-Clause; single TOML dependency; pledge/unveil on OpenBSD.
- https://forum.cloudron.io/topic/5827/molly-brown-gemini-project-on-cloudron — unfulfilled Cloudron packaging request; identified hurdles list.
- https://codeberg.org/tslocum/twins — twins repo; last commit 2025-11-20; YAML per-path config; CONFIGURATION.md / PROPOSALS.md.
- https://codeberg.org/tslocum/twins/issues/18 — proxy body-truncation bug (evidence against v1 proxying).
- https://github.com/michael-lazar/jetforce — Jetforce; Twisted; JetforceApplication routing; pushed 2026-02-09; 217 stars; Floodgap license.
- https://github.com/michael-lazar/gemini-diagnostics — torture-test repo; pushed 2022-07-22; 27 stars; 3 open issues.
- https://raw.githubusercontent.com/michael-lazar/gemini-diagnostics/master/gemini-diagnostics — source of the 27 enumerated checks.
- https://api.github.com/repos/… (agate, jetforce, gemini-diagnostics) — pushed_at/archived/stars metadata, queried 2026-08-09.
- https://crates.io/api/v1/crates/{titanite,northstar,windmark,twinstar,gemserv} — crate metadata: titanite 0.3.2 (2025-02-24); windmark 0.7.0 (2026-05-29); twinstar 0.4.0 (2022-05-02); gemserv 0.6.6 (2022-02-18); `northstar` crate name now an unrelated container runtime. `gemax` returns "does not exist."
- https://github.com/YGGverse/titanite — titanite repo; "Project in development!"; ~37 commits; no releases.
- https://github.com/ninedraft/gemax — gemax is a Go library, not Rust.
- https://github.com/gemrest/windmark — windmark repo; rossweisse macro router; tokio/async-std; ~257 commits; 15 stars.
- https://sr.ht/~int80h/gemserv/ — gemserv upstream (unmaintained since 2022; forks: GreatWizard/gemserv, calacuda/gemserv).
- https://github.com/kr1sp1n/awesome-gemini — ecosystem index used for cross-checking server list.
- https://john.dev/posts/2020-11-02-gemini-dianostics.html — third-party account of running gemini-diagnostics (false-positive caveats).

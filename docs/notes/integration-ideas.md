# Integration ideas — Cloudron panel, Tor/I2P, wizard

Working notes (2026-08-09) feeding INTEGRATIONS.md and the v1.1/v1.2
designs. Directed by the director's 2026-08-09 message.

## Cloudron panel synergies (thought through, not yet implemented)

Cloudron gives every app a set of platform UIs for free. usv should be
*designed so each one is genuinely useful* rather than incidental:

1. **File manager = the content-authoring UI.** The panel's file
   manager edits `/app/data` in the browser. Because usv's watcher
   re-renders on write (ADR 0004), editing `content/index.gmi` in the
   Cloudron file manager IS publishing — no SFTP, no deploy step.
   Consequences for us: friendly file layout under /app/data, a
   README.txt inside each directory explaining what lives there, and
   theme CSS in an editable location so restyling is also just
   file-manager editing. (SFTP access to /app/data exists too for
   people who prefer real editors.)
2. **Web terminal = the admin CLI.** The panel opens a shell in the
   container. Design `usv` subcommands for exactly that context:
   `usv status` (listeners, render state, content stats),
   `usv fingerprint` (show the TOFU certificate fingerprints — the
   thing an operator needs to publish out-of-band so visitors can
   verify), `usv check` (validate config + content tree + gemtext
   lint), `usv render --force` (rebuild), `usv zones` (list cert
   zones and enrolled fingerprints). Every subcommand works identically
   standalone, so this costs Cloudron nothing extra.
3. **Log viewer = the request log.** Single-line, greppable,
   query-redacted log lines mean the panel's log stream is a usable
   traffic view without us building one.
4. **Panel settings map cleanly.** Port config (pinned 1965,
   readOnly), aliases (multiDomain → SNI vhosts), memory limit,
   backup schedule (= TOFU identity protection). DEBUGGING.md gets a
   "what each panel screen means for usv" table.
5. **postInstallMessage** prints the `gemini://` URL, the certificate
   fingerprint command, and a 3-line quickstart.
6. **Our own HTML surface complements the panel**: a status/about page
   showing the capsule's cert fingerprint (lets visitors verify their
   client's TOFU pin over an independently-certified HTTPS channel —
   cheap and genuinely useful), feed URLs, and theme name. No
   authenticated admin UI in v1 — the panel + file manager already
   cover administration, and an admin surface is attack surface.
7. **Not available**: Cloudron has no per-app custom settings panels,
   so anything interactive beyond file editing is either our HTML
   surface or the terminal — which is why the CLI subcommands matter.

## Tor / I2P

- **Gemini over Tor**: run usv normally, add a torrc onion service
  mapping virtual port 1965 → 127.0.0.1:1965. Needs from usv (all
  cheap, schedule with v1.1): `advertised_host` override so generated
  links/redirects use the .onion name; a cert slot minted for the
  .onion hostname (TOFU works fine on onions; the onion address itself
  already authenticates, which docs should explain); tolerance for
  no-SNI connections (Tor clients may omit it); bind-address config
  (127.0.0.1) so the onion is the only path if desired. Client side:
  Lagrange and Amfora reach onion capsules via SOCKS proxy settings —
  verify exact Lagrange steps when writing the doc.
- **I2P**: same shape via an I2P server tunnel to the b32 address;
  same usv affordances cover it. (Agate's tracker shows I2P users
  exist and hit SNI edge cases — docs/recon/prior-art.md.)
- **OnionShare**: its website mode hosts a static site over onion
  HTTP. usv's rendered HTML tree is exactly that — so "your capsule's
  web mirror as an onion site" is a documented copy-the-folder recipe
  with zero new code. The native-Gemini onion path above is the
  first-class story; OnionShare is the zero-infrastructure one.
- Honest framing everywhere: these protect *readers'* privacy and the
  capsule's reachability; operator anonymity requires the whole stack
  (hosting, DNS, payments) to be anonymous, which is out of usv's
  hands.

## Responses anti-spam (director Q 2026-08-09; feeds ADR 0009)

Layered, all self-hosted, no external services ever:
- **Layer 0 (the guarantee): moderation-first** — nothing publishes
  unapproved; bounded pending queue (over cap → polite refusal). Spam
  can only annoy the operator, never readers.
- **Layer 1: no-JS bot traps** — honeypot field + stateless
  HMAC-signed form token binding page URL + timestamp (reject
  too-fast and too-old submissions; kills replay + cross-site posts).
- **Layer 2: rate/content limits** — per-source limits keyed on
  salted-rotating IP hashes (no PII retained), global caps, length
  cap, link-count threshold, duplicate-body-hash rejection.
- **Layer 3 (optional, off by default): operator's custom question**
  — plain-text site-specific challenge; the accessible self-hosted
  CAPTCHA alternative. Docs honest that AI bots can beat it; Layer 0
  is why that's survivable.
- **Gemini side**: client-cert identity is the anti-spam; per-
  fingerprint limits + ban list. Likes: dedupe per fingerprint /
  hashed-IP-day; low stakes by design.
- **Refused**: spam APIs, email verification, CAPTCHAs, mandatory-JS
  proof-of-work.

## Visit stats (director Q 2026-08-09)

Server-side log aggregation only — no beacons, no JS, no cookies, no
third party. Gemini requests carry nothing but URL (+ optional cert),
so log aggregation is the *only possible* analytics there anyway; the
web surface uses the same method for symmetry and privacy.
- Collect: per-path daily hit counts; approximate uniques via
  salted-daily-rotating IP hashes (raw IPs never stored for stats);
  status breakdown; **feed-fetch counts** (Atom/gemsub polls ≈
  subscriber signal — the number a gemlog author actually wants);
  web-side bot/crawler split via user-agent heuristics; Gemini-side
  crawler visibility via robots.txt virtual-agent hits.
- Present: `usv stats` (CLI now, ratatui dashboard later); optionally
  a rendered private stats page in a cert-gated zone (dogfoods
  zones). Nothing public by default.
- Defaults: aggregate-only, configurable retention (e.g. 90 d), off
  switch, documented plainly (what is and isn't collected). Honest
  docs: uniques are approximate — Gemini has no cookies/UA, and
  that's a feature.
- Logs stay goaccess-compatible for operators who want more.

First-run TUI for standalone users: protocol tick-list (Gemini ✓ /
HTTP mirror / gopher / Spartan / Nex — plaintext ones show a one-line
trust warning), hostname, content/state dirs, theme picker (name +
description; terminal preview of colors is not meaningful for web
themes — link the gallery instead), then writes usv.toml and prints
next steps. `usv init --defaults` non-interactive. Cloudron profile
never runs it (env-driven).

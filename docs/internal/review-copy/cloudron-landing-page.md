## Unseen Servant

A security-first [Gemini](https://geminiprotocol.net/) server that publishes
**one content tree to two worlds**: served natively as gemtext on port 1965,
and statically rendered to themed, classless HTML for the web. Write a
gemlog once — readers reach it from Lagrange, lynx, or Chrome, whichever
they prefer.

### What you get

- **TOFU-native identity.** A certificate is generated once, per hostname,
  the first time the capsule starts, and never silently touched again.
  Readers who pin it on first visit can trust it stayed the same through
  every update, backup, and restore.
- **A dual surface with no build step.** The rendered HTML tree is a
  self-contained static site — copy it anywhere (an onion mirror, an
  OnionShare folder, a CDN) and it still works.
- **Titan uploads**, certificate-gated private zones, gemsub and Atom feeds
  — all opt-in, all off until you configure them.
- **Tor and I2P friendly** by design: an onion address is just another
  hostname, and the server tolerates clients that connect without SNI.
- **A terminal setup wizard** (`usv init`) if you ever need to run it
  outside Cloudron too — the same binary, no Docker required.

### On this platform

- The dashboard tile is your capsule's web mirror.
- The Gemini port (1965) is fixed, matching what every Gemini client
  assumes by default — a moved port is only reachable via an explicit
  `gemini://host:PORT/` URL, which breaks discovery for casual visitors.
- Your TOFU keypair lives in this app's backed-up data, and survives
  updates, restores, and moves to a new domain (a hostname change is
  detected, and you choose whether to keep or regenerate the identity).
- Extra domains can be added as aliases — one capsule, many hostnames,
  each with its own certificate, served over a single connection via SNI.

Unseen Servant is pre-release software: functional and tested, but the
project has not reached its v1.0 quality bar yet. Expect rough edges.

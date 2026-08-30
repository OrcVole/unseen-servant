## Unseen Servant

A security-first server for the **small networks**. It publishes one folder
of writing to [Gemini](https://geminiprotocol.net/), Gopher, Spartan, Nex
and Finger at once, and mirrors the same content to the web as themed,
classless HTML. Write a page once: readers reach it from Lagrange, from a
gopher client older than most of the web, from `lynx`, or from Chrome,
whichever they already have.

Rendering happens when a file changes, not per request. Save a page and it
is live on every surface a couple of seconds later, from one source, with
no build step and no second copy to keep in step.

### What you get

- **Six surfaces, one content tree.** Gemini (1965) and Titan for uploads,
  the web mirror, and — each off until you switch it on — Gopher, Spartan,
  Nex and Finger.
- **Cleartext protocols cannot leak gated content.** Gopher, Spartan, Nex
  and Finger have no encryption and no way to authenticate a reader, so
  anything behind a certificate zone is excluded from those trees when they
  are built, not filtered per request.
- **TOFU-native identity.** The certificate is generated once per hostname
  and never silently replaced. A reader who pinned it on first visit can
  trust it survived every update, backup, restore and move.
- **A dual surface with no build step.** The rendered HTML tree is a
  self-contained static site: copy it to an onion mirror, an OnionShare
  folder or a CDN and it still works.
- **Titan uploads**, certificate-gated private zones, gemsub and Atom
  feeds, and a machine-readable `/llms.txt` with Markdown siblings for
  agents and scripts.
- **Tor and I2P friendly** by design: an onion address is just another
  hostname, and clients that connect without SNI are tolerated.
- **A terminal setup wizard** (`usv init`) for running it outside Cloudron:
  the same static binary, no container required.

### On this platform

- The dashboard tile opens your capsule's web mirror.
- The Gemini port (1965) is fixed, matching what every Gemini client
  assumes. A moved port is reachable only through an explicit
  `gemini://host:PORT/` URL, which breaks discovery for casual visitors.
- The four extra protocols are optional ports you enable in the app's
  settings. Their conventional ports (70, 79, 300) are privileged and the
  platform will not publish them, so the defaults are 1024, 7979 and 3300.
- Your TOFU keypair lives in this app's backed-up data and survives
  updates, restores and moves to a new domain; a hostname change is
  detected rather than silently overwritten.
- Extra domains can be added as aliases: one capsule, many hostnames, each
  with its own certificate, over a single connection via SNI.

This is young software. It has not been independently audited, and Agate
and gmid have years of production hardening it does not.
Written end to end by an AI, directed and reviewed by a human.

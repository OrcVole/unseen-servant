## Unseen Servant

One folder of writing, served to six networks at once. No build step,
and nothing executed, ever. Write a page as plain gemtext and readers
reach it from Lagrange, from a gopher client older than most of the
web, from `lynx`, or from Chrome, whichever they already have.

Rendering happens when a file changes, not per request. Save a page and
it is live on every surface a couple of seconds later, from one source,
with no second copy to keep in step.

### What you get

- **Six surfaces, one content tree.** Gemini (1965) and Titan for
  uploads, the web mirror, and, each off until you switch it on,
  Gopher, Spartan, Nex and Finger.
- **Cleartext protocols cannot leak gated content.** Gopher, Spartan,
  Nex and Finger have no way to authenticate a reader, so anything
  behind a certificate zone is excluded from those trees when they are
  built, not filtered per request.
- **Identity that survives the infrastructure.** The certificate is
  generated once per hostname and never silently replaced: it survives
  every update, backup, restore and move.
- **A portable web mirror.** The rendered HTML tree is a self-contained
  static site: copy it to an onion mirror, an OnionShare folder or a
  CDN and it still works.
- **Agents are first class.** An automated publisher presents a client
  certificate, no password or API key to leak; every read-only command
  emits JSON; `/llms.txt` and Markdown siblings serve machine readers.
- **Titan uploads**, certificate-gated private zones, and gemsub and
  Atom feeds.
- **Tor and I2P friendly** by design: an onion address is just another
  hostname, and clients that connect without SNI are tolerated.
- **A terminal setup wizard** (`usv init`) for running it outside
  Cloudron: the same static binary, no container required.

### What it refuses

No CGI, no scripting, no proxying, no plugin API, no admin panel, and
no visitor address logging by default. Content is data, never code.
The design started by reading the servers of every network it speaks,
Agate, Molly Brown, GmCapsule, gmid and the gopher line among them, and
the one feature each regretted was the escape hatch beyond static
serving. This server has none, permanently.

### On this platform

- The dashboard tile opens your capsule's web mirror.
- The Gemini port (1965) is fixed, matching what every Gemini client
  assumes. A moved port is reachable only through an explicit
  `gemini://host:PORT/` URL, which breaks discovery for casual
  visitors.
- The four extra protocols are optional ports you enable in the app's
  settings. Their conventional ports (70, 79, 300) are privileged and
  the platform will not publish them, so the defaults are 1024, 7979
  and 3300.
- Your TOFU keypair lives in this app's backed-up data and survives
  updates, restores and moves to a new domain; a hostname change is
  detected rather than silently overwritten.
- Extra domains can be added as aliases: one capsule, many hostnames,
  each with its own certificate, over a single connection via SNI.

Human architected, AI coded: written end to end by an AI, directed and
reviewed by a human, with every design decision recorded before the
code. It is a first release, not independently audited, and Agate and
gmid have years of production hardening it does not.

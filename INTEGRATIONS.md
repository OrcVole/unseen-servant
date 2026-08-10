# Integrations

How usv composes with the world around it. Sections marked with a phase are
committed but not yet built; design detail lives in
`docs/notes/integration-ideas.md` until each hardens here.

## Cloudron (C6)

The primary deployment profile: tcpPort 1965 (pinned), proxied HTTPS web
surface, `/app/data` state, panel file manager as the content-authoring UI,
web terminal for the `usv` CLI. Full constraint set:
`docs/recon/cloudron-fit.md`.

## Tor and I2P (C5)

Onion/eepsite capsules via ordinary tunnel configuration — usv needs no
Tor/I2P-specific code, only three general affordances that already exist:
a `[[host]]` entry accepts any validated hostname shape, including an
onion address (`validate_hostname`'s rules — ASCII, 1–253 chars, ≤63-byte
labels — already admit a 56-character v3 onion label with no special
case); the SNI resolver falls back to the first configured host when a
ClientHello carries none at all, which is how some minimal Gemini clients
connect to onions since there's no real CA/DNS involved; and
`server.advertised_host` lets the render pipeline advertise a different
name than the first `[[host]]` in generated web-surface links (Atom,
sitemap.xml, /llms.txt, robots.txt) — for a capsule that's dual-homed on
a real hostname and a Tor mirror.

**Gemini over Tor — verified live, 2026-08-10.** A real `tor` process
(0.4.9.11, official Debian/Alpine package) was pointed at a running `usv`
and reached over the live Tor network from a separate client process
through the real onion service — not a simulated handshake. Recipe:

```
# torrc
HiddenServiceDir /var/lib/tor/usv/
HiddenServicePort 1965 127.0.0.1:1965
```

```toml
# usv.toml — after starting tor once to learn the onion address from
# /var/lib/tor/usv/hostname
[server]
listen = ["127.0.0.1:1965"]
# The virtual port named in HiddenServicePort, not necessarily usv's own
# listen port — see the gotcha below.
advertised_port = 1965

[[host]]
name = "yourgeneratedaddress1234...abcd.onion"
```

**The gotcha this recipe exists to save you from:** a Tor hidden service
is itself a port-remapping layer (`HiddenServicePort <virtual> <target>`
can map any virtual port to any target port), exactly the case
`advertised_port` was built for (redirect and authority-check
correctness on platforms where the bound port isn't the one clients
name). Get `advertised_port` wrong — leave it defaulting to whatever
`listen` binds — and every request through the onion service is refused
with `53` even though the TLS handshake and certificate are perfectly
fine; this is precisely what happened during live verification before
the config above was corrected. If `HiddenServicePort` maps virtual port
1965 (Gemini's default, the port well-behaved clients omit from the URL)
to a `usv` listening on a different real port, `advertised_port` must be
set to `1965` regardless of what `listen` says.

Client side: Lagrange and Amfora reach onion capsules via their SOCKS
proxy settings pointed at Tor's SocksPort — the live check above used a
minimal hand-rolled SOCKS5+TLS client instead of a full Gemini client,
so exact Lagrange menu steps are still worth confirming by hand before
this goes in user-facing docs.

**I2P** is architecturally the same shape — an I2P server tunnel mapping
a `.b32.i2p` address to `usv`'s local listener, same `advertised_host`/
`advertised_port`/no-SNI affordances apply — but was not live-verified in
this round (no I2P router available in the sandbox this was built in).
Agate's issue tracker shows real I2P users hitting SNI edge cases
(docs/recon/prior-art.md), which is the concrete reason no-SNI tolerance
is a first-class, tested affordance here rather than an afterthought
(`tests/wire.rs::a_connection_with_no_sni_is_served_by_the_default_host`
proves the fallback with a real ClientHello that omits SNI, not just a
resolver-level unit test).

**Anonymity honesty**, unchanged from the original design note: these
affordances protect *readers'* privacy and give the capsule a reachable
address that doesn't leak the operator's real IP to visitors. Operator
anonymity is a separate, much larger property that depends on the whole
hosting chain (VPS provider, payment method, DNS, backups) — out of
usv's hands and out of scope for this section.

## OnionShare (C5)

`usv export` emits the rendered HTML tree as a drop-in folder for
OnionShare's website mode — a zero-infrastructure onion mirror of your
capsule.

## Feeds and aggregators (C3)

Generated indexes carry gemsub dated links; `atom.xml` is emitted for both
surfaces. CAPCOM/Antenna consume the Gemini side natively; web feed readers
take the Atom. Submission/announcement mechanics: `docs/ROADMAP.md` M6.

## Contact addresses (documentation only)

usv is a content server, not a mailbox: for a capsule contact address the
smolnet answer is a `misfin://` link served beside your content (one-click
in Lagrange ≥1.18) with a standalone misfin server handling delivery, or a
plain `mailto:`. See `docs/recon/smolnet.md` §5.

## Smolnet side-protocols (v1.1)

Gopher, Spartan, Nex, and Finger as opt-in listeners over the same content
tree — all plaintext, all off by default, trust model documented plainly.
Design source: `docs/recon/smolnet.md`.

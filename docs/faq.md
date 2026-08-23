---
title: "FAQ"
description: "No. Gemini and Titan are the primary protocols, and the same content is rendered to static HTML (HyperText Markup Language) for browsers. Gopher, Spartan, Nex and Finger are supported too,."
type: explanation
status: decided
last_verified: 2026-08-11
---

# FAQ

> Unseen Servant

## Is this only a Gemini server?

No. Gemini and Titan are the primary protocols, and the same content is
rendered to static HTML (HyperText Markup Language) for browsers. Gopher,
Spartan, Nex and Finger are supported too, each off until you enable it.
[`protocols.md`](protocols.md) is the authority.

## So it *is* a web server?

No. It renders your gemtext to HTML and serves those files. There is no
authentication, no request-time logic, no CGI (Common Gateway Interface),
and no proxying on that surface. It exists so someone with only a browser
can read your capsule.

## Do I have to configure anything?

No. Run the binary. With an empty state directory it mints a certificate,
writes a starter capsule, and serves it. `usv init` runs a terminal wizard
if you would rather be asked.

## Why does my client warn about the certificate?

Because it is self-signed, which is normal and correct for Gemini. The
protocol uses TOFU (trust on first use): trust-on-first-use, like SSH. Your
client remembers the certificate the first time and only warns again if it
*changes*.

## Will my certificate change if I update, restore, or move?

No. It is minted once per hostname and never silently regenerated. If `usv`
finds damaged key material it stops with an error rather than quietly making
a new one: a new key is indistinguishable from an impersonation to anyone
who pinned the old one. See [`../UPGRADING.md`](../UPGRADING.md).

Moving to a *new hostname* does mint a fresh identity for that name. The old
keypair is kept.

## Can I use a Let us Encrypt certificate instead?

The design supports reading one, but TOFU self-signed is the default and the
recommendation. CA (certificate authority) certificates rotate every 60-90
days, and several Gemini clients treat any change within the old
certificate's validity as a possible interception, so automatic renewal
produces exactly the warning TOFU exists to make meaningful.

## How do I add content?

Put `.gmi` files in the content directory. One file per page. Saving
triggers a re-render of every surface within seconds.

## Can I edit remotely?

Yes, over Titan from a client like Lagrange, once you configure a zone and
authorise a client certificate. See [`titan.md`](titan.md). It is off until
you set it up.

## Can multiple people edit?

Multiple fingerprints can be authorised per zone, and the roster gives them
names and capabilities. `usv` records when a key was enrolled, never who
holds it. There are no user accounts.

## Can an AI agent use it?

Yes, and that is a supported case rather than a tolerated one: reading,
writing over Titan, and operating the server. See [`agents.md`](agents.md).

## Does it need root?

No, and it should not have it. Every listener defaults to a port above 1024,
including Gopher (7070, advertising itself as 70) and Finger (7979), so no
capability is needed at all, and the shipped systemd unit sets an empty
capability bounding set. Binding the traditional privileged ports directly
is your choice to make, with a port-forward or a capability you grant
deliberately.

## Can I run several capsules?

One process serves many hostnames by SNI (Server Name Indication): add a
`[[host]]` block each, and every hostname gets its own certificate. On
Cloudron, one app can hold port 1965 per server, so use alias domains rather
than a second install.

## Does it log my visitors' IP addresses?

Not by default. The request log carries the status and the path, and the
peer field reads as `-`. Query strings are redacted by construction, since
Gemini's input flow puts whatever a visitor typed into the query.

`server.log_peer` gives you two opt-ins: `full` for a conventional access
log, or `hashed` for a digest under a salt made fresh at every start. See
[`configuration.md`](configuration.md).

## Where does my data live?

One state directory: identity, content, rendered output, config. It is
`/var/lib/usv` for distro packages, `/app/data` on Cloudron, and whatever
`USV_STATE_DIR` says in a container. Back that up and you have backed up
everything, including the identity readers pin.

## How do I upgrade?

Replace the binary, package or image. State lives outside the code and is not
touched. `systemctl reload usv` re-reads config and certificates without
dropping connections; a bad config is rejected and the previous one keeps
running.

## Does it work behind a reverse proxy?

The HTTP (HyperText Transfer Protocol) surface does, and that is how the
Cloudron package runs. The Gemini surface cannot be: Gemini is TLS-native
but not HTTP, so `usv` must terminate its own TLS (Transport Layer Security)
on 1965 and needs the port passed through.

## Is it production-ready?

It is pre-1.0 and unaudited. It passes the community conformance suite
against a live deployment and has a real test and fuzz suite, but Agate and
gmid have years of production use it does not.

## Why is there no admin web UI?

An authenticated admin panel is a credential to leak, a session to hijack,
and a default password waiting to be found. Content is edited through the
filesystem or Titan; observation happens over the wire through a cert-gated
status resource.

## It is AI-written. Should I be worried?

Decide for yourself: it is stated in the README rather than hidden. What is
on offer instead: every design decision recorded before the code, the
research behind them published, a real test suite, fuzzed parsers, and
documentation of the things it does badly.

## Where do I report a bug or a vulnerability?

Bugs: the issue tracker. Vulnerabilities: **not** the issue tracker: see
[`../SECURITY.md`](../SECURITY.md).

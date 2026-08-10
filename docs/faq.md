# FAQ

### Is this only a Gemini server?

Gemini is the primary protocol, plus Titan for uploads. It also renders
your content to static HTML so browsers can read it. Gopher, Spartan,
Nex and Finger are planned for v1.1 and **not implemented** —
[`protocols.md`](protocols.md) is the authority and says so plainly.

### So it *is* a web server?

No. It renders your gemtext to HTML and serves those files. There is no
authentication, no request-time logic, no CGI, and no proxying on that
surface. It exists so someone with only a browser can read your capsule
— not to host a website.

### Do I have to configure anything?

No. Run the binary. With an empty state directory it mints a
certificate, writes a starter capsule, and serves it. Zero configuration
is a supported configuration (ADR 0008), not a degraded mode. `usv init`
runs a terminal wizard if you'd rather be asked.

### Why does my client warn about the certificate?

Because it's self-signed, which is normal and correct for Gemini. The
protocol uses TOFU — trust-on-first-use, like SSH. Your client remembers
the certificate the first time and only warns again if it *changes*.
There's no certificate authority involved.

### Will my certificate change if I update, restore, or move?

No. It's minted once per hostname and never silently regenerated —
that's the point of [`../UPGRADING.md`](../UPGRADING.md). If `usv` finds
damaged key material it stops with an error rather than quietly making a
new one, because a new key is indistinguishable from an impersonation to
anyone who pinned the old one.

Moving to a *new hostname* does mint a fresh identity for that name,
deliberately. The old keypair is kept.

### Can I use a Let's Encrypt certificate instead?

The design supports reading one (the Cloudron `tls` addon path), but
TOFU self-signed is the default and the recommendation. CA certificates
rotate every 60–90 days, and several Gemini clients treat any change
within the old certificate's validity as a possible interception — so
automatic renewal produces exactly the warning TOFU exists to make
meaningful.

### How do I add content?

Put `.gmi` files in the content directory. One file per page. Saving
triggers a re-render of both surfaces within seconds. That's the whole
workflow — no build, no deploy.

### Can I edit remotely?

Yes, via Titan from a client like Lagrange, if you configure a zone and
authorise a client certificate. See [`titan.md`](titan.md). It's off
until you set it up.

### Can multiple people edit?

Multiple *fingerprints* can be authorised per zone, and the roster gives
them names. `usv` records when a key was enrolled, never who holds it —
continuity, not attestation. There are no user accounts.

### Does it need root?

No, and it shouldn't have it. Both ports are above 1024, so no
capability is needed at all — the shipped systemd unit sets an empty
capability bounding set. Packages create a dedicated `usv` system user.

### Can I run several capsules?

One process serves many hostnames by SNI — add a `[[host]]` block each,
and every hostname gets its own certificate. On Cloudron specifically,
one *app* can hold port 1965 per server, so use alias domains rather
than a second install.

### Does it log my visitors' IP addresses?

Not by default. The request log carries the status and the path, and the
peer field reads as `-`. Query strings are redacted by construction,
since Gemini's input flow puts whatever a visitor typed into the query.

If you want addresses, `server.log_peer` gives you two opt-ins: `full`
for a conventional access log, or `hashed` for a digest under a salt
made fresh at every start — repeat visits correlate within one run of
the process, and nothing survives a restart. See
[`configuration.md`](configuration.md).

### Where does my data live?

One state directory: identity, content, rendered output, config. It's
`/var/lib/usv` for distro packages, `/app/data` on Cloudron, and
whatever `USV_STATE_DIR` says in a container. Back that up and you have
backed up everything, including the identity readers pin.

### How do I upgrade?

Replace the binary (or the package, or the image). State lives outside
the code and isn't touched. `systemctl reload usv` re-reads config and
certificates without dropping connections; a bad config is rejected and
the previous one keeps running.

### Does it work behind a reverse proxy?

The HTTP surface does, and that's how the Cloudron package runs. The
**Gemini** surface cannot be proxied by ordinary HTTP reverse proxies —
Gemini is TLS-native but not HTTP, so `usv` must terminate its own TLS on
1965 and needs the port passed through.

### Is it production-ready?

It's pre-1.0 and unaudited. It passes the community conformance suite
against a live deployment and has a real test and fuzz suite, but Agate
and gmid have years of production use it doesn't. If you want the safe
choice today, [`../COMPARISON.md`](../COMPARISON.md) is honest about
that.

### Why is there no admin web UI?

Because an authenticated admin panel is a credential to leak, a session
to hijack, and a default password waiting to be found. Content is edited
through the filesystem or Titan; observation happens over the wire
through a cert-gated status resource. That's a deliberate refusal.

### It's AI-written. Should I be worried?

You should decide for yourself, which is why it's stated in the README
rather than hidden. What's on offer instead of trust: every design
decision recorded as a written note *before* the code, the research
behind them published, a real test suite, fuzzed parsers, and honest
documentation of the things it does badly. Judge it on that.

### Where do I report a bug or a vulnerability?

Bugs: the issue tracker. Vulnerabilities: **not** the issue tracker —
see [`../SECURITY.md`](../SECURITY.md).

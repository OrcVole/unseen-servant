# Which protocols should I serve?

You do not have to choose one. `usv` serves a single content tree, so
adding a protocol is mostly a question of *who you want to reach* and
*what you are willing to serve in cleartext* — not of maintaining
another site.

**Status note:** all of these work now — Gemini, Titan and the web
mirror, plus Gopher, Spartan, Nex and Finger from the v1.1 round. The
four cleartext ones are **off until you enable them**.
[`protocols.md`](protocols.md) is the authority on what exists and, for
each, which client it was verified against.

## The short version

| Protocol | Serve it if… | Cost to you |
|---|---|---|
| **Gemini** | You want to be read in a living community, with feeds, aggregators and good clients | The default; nothing to decide |
| **Web mirror** | You want anyone with a browser to be able to read you | One config line; same tree |
| **Gopher** | You want durability and a different, older audience | One config line; cleartext |
| **Spartan** | You object to mandatory TLS, or serve somewhere TLS is a burden | One config line; cleartext |
| **Nex** | You are part of that scene, or you like the minimum | One config line; cleartext |
| **Finger** | You want a personal status line, not a site | One config line; not your content tree |

## Gemini — the one to start with

**The case:** it is the only protocol here with a living, growing
community *and* good client software. Lagrange is genuinely pleasant.
Feeds work, aggregators like Antenna will pick you up, and people will
link you. It is also the only one of these that can authenticate a
reader, which is what makes private areas and Titan editing possible at
all.

**Serve it if** you want to be read and replied to. This is the default
and you should need a reason not to.

## The web mirror — reach, for free

**The case:** most people cannot open a `gemini://` link. The mirror
means you can put your capsule's address on a CV, in an email footer, or
in a toot, and it just works — search engines included. Because it is
the same rendered tree, it costs you nothing beyond a listener: no
second site, no sync, no build.

**Serve it if** you want anyone to be able to read you without
installing something first. Turn it off if the point of your capsule is
that it is *not* on the web — that is a real and respected position.

## Gopher — durability, and a different room

**The case:** two things Gemini cannot offer.

*It will still work.* Gopher has been stable since 1991. Clients written
in the nineties still resolve menus today, and something you publish now
will very likely still be readable by software nobody has touched in
decades. Gemini is a young protocol with a young ecosystem; gopher has
already survived the entire lifespan of the web.

*It is a different community.* Gopherspace is not a subset of
Geminispace — there are people there who never moved, with their own
aggregators (Bongusta), their own search (Veronica-2), and their own
culture. Serving gopher puts you in a room you are otherwise not in.

There is also a hardware argument: gopher clients exist for machines
that will never speak modern TLS. If you want a vintage terminal to be
able to read you, this is the only protocol here that can.

**The catch:** menus, not documents. Gopher's structure is a
*navigational* one, and prose reads less well through it than gemtext
does. And it is cleartext — see below.

**Serve it if** longevity matters to you, or you want to be read by the
gopher community on its own terms.

## Spartan — the objection to TLS, taken seriously

**The case:** Gemini requires TLS, and not everyone thinks that was
right. TLS means certificates, clocks that must be roughly correct,
libraries that must be maintained, and hardware that can do the maths.
Spartan is what Gemini looks like with that requirement removed:
essentially the same document model, none of the cryptography.

That makes it genuinely useful in places Gemini is awkward — a local
network, a machine too small for a TLS stack, an audience that considers
mandatory encryption for public documents to be ceremony rather than
safety.

**Serve it if** you find that argument persuasive, or you are publishing
somewhere TLS is a real burden. It costs you almost nothing: Spartan
serves the same gemtext tree unchanged.

**Note:** `usv` refuses Spartan uploads permanently. They are
unauthenticated by construction, and Titan already does uploads properly
(ADR 0012).

## Nex — the minimum, and a small scene

**The case:** Nex is smaller than Spartan — barely a protocol at all,
which is the point. It has a small, experimental community that values
exactly that, and hand-rolling a client for it is an afternoon.

**Be honest with yourself:** the audience is tiny and you will probably
need a specific client to see your own work (`gelim` is one). If you are
not already interested in Nex, serving it is a gesture rather than
reach.

**Serve it if** you are part of that scene, or you simply like that the
whole protocol fits in your head. It is nearly free to enable.

## Finger — a person, not a site

**The case:** Finger is the odd one out. It does not serve your content
tree at all — it answers "what is this person up to?" with a short
status text. `finger you@yourhost` and out comes a few lines.

That is a genuinely distinct thing to offer: a *now* page with no page.
It is charming, it costs one config entry, and it is the only protocol
here that is about a person rather than a document.

**Serve it if** you like the idea of a status anyone can query in one
command. Do not expect it to carry your writing.

## New to these? Start here

If you have never used any of this, the fastest route in is **one
client that speaks several protocols**, so you can wander between them
without installing something each time.

| Client | Speaks | Kind |
|---|---|---|
| **Lagrange** | Gemini, Gopher, Spartan, Finger, Titan | Graphical; the friendliest starting point |
| **Bombadillo** | Gemini, Gopher, Finger, HTTP | Terminal |
| **gelim** | Gemini, Gopher, Spartan, Nex | Terminal, line-mode |
| **Offpunk** | Gemini, Gopher, Spartan, HTTP | Terminal; offline-first |
| **BFG** | Gopher, Finger, Nex, Spartan, Gemini | Terminal; the widest coverage |
| **amfora** | Gemini | Terminal |
| **lynx** | Gopher, HTTP | The venerable one; already on many systems |

**Lagrange** is the one to try first if you want a window with fonts
and images. **Bombadillo** or **gelim** if you live in a terminal.

Client support shifts; this reflects what was verified in
`docs/recon/smolnet.md` and `docs/recon/ecosystem.md` in August 2026.
Nothing here endorses a client — they are simply the ones that exist.

## The protocols, one by one

### Gemini

- **Home:** <https://geminiprotocol.net/>
- **Clients:** Lagrange, amfora, Bombadillo, Offpunk, gelim, Elpher,
  Kristall
- **Philosophy:** deliberately, permanently small. The spec is capped
  by design so a person can write a client in a weekend and no vendor
  can make it complicated. Mandatory TLS, no cookies, no scripting, no
  tracking — not as features but as things the protocol *cannot* do.
- **Distinctive use:** a personal site read by people who chose to be
  there. It is the only protocol here that can authenticate a *reader*,
  which is what makes private areas and Titan editing possible.

### The web mirror (HTTP)

- **Clients:** every browser you already have.
- **Philosophy:** none of its own — this is `usv` meeting people where
  they are, not an endorsement of the web.
- **Distinctive use:** reach. Your address works in an email footer, a
  CV, a toot; search engines can find it.

### Gopher

- **Home:** RFC 1436 (1993); the living hub is
  <gopher://gopher.floodgap.com/>, browsable over the web at
  <https://gopher.floodgap.com/>
- **Clients:** Lagrange, lynx, Bombadillo, Offpunk, BFG, Overbite (a
  Firefox extension), plus web proxies
- **Philosophy:** menus, not documents. Gopher thinks the internet is a
  filing cabinet you browse, and it has not changed its mind since 1991.
  That refusal to evolve is the point — it is why software nobody has
  touched in decades still works.
- **Distinctive use:** longevity, and a genuinely separate community
  with its own aggregators (Bongusta) and search (Veronica-2). Also the
  only protocol here a vintage machine that will never speak modern TLS
  can read.
- **Reality check:** by far the biggest of the plaintext protocols —
  hundreds of active servers (Floodgap, SDF, bitreich, the tildeverse).
  Spartan and Nex reach a rounding error by comparison.

### Spartan

- **Home:** <spartan://spartan.mozz.us/specification.gmi> — and, since
  that is circular if you have no client yet, over the web at
  <http://portal.mozz.us/spartan/spartan.mozz.us/specification.gmi>
- **Clients:** Lagrange, Offpunk, gelim, Profectus, BFG
- **Philosophy:** Gemini's document model with the cryptography
  removed. Its argument is that mandatory TLS is ceremony for public
  documents — clocks that must be right, libraries that must be
  maintained, hardware that must do the maths — and that a plain
  document deserves a plain protocol.
- **Distinctive use:** somewhere TLS is a genuine burden — a local
  network, a very small machine — or because you find that argument
  persuasive. It reads the same gemtext, so it costs you nothing.
- **Reality check:** a few dozen capsules. Spec finished and stable
  since ~2021.

### Nex

- **Home:** <https://nightfall.city/nex/info/specification.txt> (m15o)
- **Clients:** gelim, BFG, Profectus
- **Philosophy:** the smallest thing that still works. No status codes,
  no content types, no headers — send a path, get bytes, connection
  closes. Explicitly telnet-compatible, so you can speak it by hand.
- **Distinctive use:** if you enjoy that the entire protocol fits in
  your head, or you want to hand-roll a client in an afternoon.
- **Reality check:** the smallest audience here, and you will likely
  need a specific client to see your own work. Serving it is a gesture
  more than reach — a perfectly good reason, and roughly why much of
  smolnet exists.

### Finger

- **Home:** RFC 1288 (1991)
- **Clients:** Lagrange, Bombadillo, BFG, and the `finger` command that
  ships with many systems
- **Philosophy:** the internet's oldest status update. It answers "what
  is this person up to?" — the `.plan` file, which predates blogging by
  decades and does the same job in four lines.
- **Distinctive use:** a *now* page with no page. One command, a few
  lines back. In `usv` it does not serve your writing at all — it
  answers with a short profile and points at your capsule.
- **Note:** `usv` refuses finger **forwarding** (`user@host` queries),
  as RFC 1288 itself recommends: answering them turns a server into a
  relay for probing hosts that would not answer the asker directly.

## The thing to understand before enabling any of the last four

Gopher, Spartan, Nex and Finger are **all cleartext**. That means:

- No confidentiality. Anyone between you and your reader sees what was
  requested and what came back.
- No integrity. Anyone on that path can change it in flight.
- No server authentication. A reader cannot tell your server from
  someone impersonating it.
- **No client authentication, ever.** These protocols cannot do it.

So: serve only content whose integrity loss is acceptable. `usv`
enforces the sharpest edge of this for you — content behind a
certificate zone or a Titan zone is excluded from every cleartext tree,
and a configuration that would publish gated content over one of these
protocols is a **startup error**, not a warning (ADR 0012).

The counterweight, honestly: this is the settled norm of these
communities. Gopher has run in the clear for thirty-five years, and
publishing public static documents is the one workload where that is
defensible. The mistake is not serving cleartext — it is serving *the
wrong things* in cleartext.

## A reasonable default

Most people should start with **Gemini plus the web mirror**: the living
community, and reach for everyone else. Add **Gopher** when longevity or
that community appeals. Add **Spartan** if the no-TLS argument is one
you actually hold. Add **Nex** and **Finger** because you like them —
which is a perfectly good reason, and roughly the reason most of
smolnet exists.

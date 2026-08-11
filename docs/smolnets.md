# The small networks

**Unseen Servant**

The smolnet: the "small internet": is a loose family of protocols that
deliberately cannot do most of what the web does. No scripting, no tracking,
no advertising, no layout engine. A page is text with links, and a client
can be written by one person in a weekend.

`usv` serves five of them plus a web mirror, from one content tree, so the
question is not "which one do I have to pick" but "who do I want to reach,
and what am I willing to send in cleartext".

## At a glance

| Network | Serve it if… | Cost |
|---|---|---|
| **Gemini** | You want to be read in a living community, with feeds and good clients | The default; nothing to decide |
| **Web mirror** | You want anyone with a browser to be able to read you | One line; same tree |
| **Gopher** | You want durability and a different, older audience | One line; cleartext |
| **Spartan** | You object to mandatory encryption, or serve where it is a burden | One line; cleartext |
| **Nex** | You are part of that scene, or you like the minimum | One line; cleartext |
| **Finger** | You want a personal status line, not a site | One line; not your content tree |

Everything but Gemini and the web mirror is off until you enable it.

## Gemini

Home: <https://geminiprotocol.net/> · Clients: Lagrange, amfora, Bombadillo,
Offpunk, gelim, Elpher, Kristall

Gemini is deliberately, permanently small. The specification is capped by
design so that a person can write a client in a weekend and no vendor can
make it complicated. TLS (Transport Layer Security) is mandatory; cookies,
scripting and tracking are not merely discouraged but absent: things the
protocol *cannot* do rather than things it asks you not to.

**Best for:** a personal site read by people who chose to be there. It is
the only network here that can authenticate a *reader*, which is what makes
private areas and remote editing over Titan possible at all.

**Reality check:** the liveliest of the five. Feeds work, aggregators such
as Antenna will pick you up, and Lagrange is genuinely pleasant to read in.
Start here; you should need a reason not to.

## The web mirror

Clients: every browser you already have.

Most people cannot open a `gemini://` link. The mirror means your capsule's
address works in an email footer, on a CV, or in a post, search engines
included. It is the same rendered tree, so it costs a listener and nothing
else: no second site, no synchronisation, no build.

**Best for:** reach.

**Turn it off if** the point of your capsule is that it is *not* on the web.
That is a real and respected position.

## Gopher

Home: RFC (Request for Comments, the internet's standards series) 1436,
published in 1993 · The living hub is <gopher://gopher.floodgap.com/>,
browsable at <https://gopher.floodgap.com/> · Clients: Lagrange, lynx,
Bombadillo, Offpunk, BFG, Overbite for Firefox, plus web proxies

Gopher thinks the internet is a filing cabinet you browse, and it has not
changed its mind since 1991. Documents are reached through menus rather than
links inside prose. That refusal to evolve is the point: clients written in
the nineties still resolve menus today.

**Best for** two things Gemini cannot offer.

*Durability.* Something you publish now will very likely still be readable
by software nobody has touched in decades. Gopher has already survived the
entire lifespan of the web.

*A different room.* Gopherspace is not a subset of Geminispace. There are
people there who never moved, with their own aggregator (Bongusta), their
own search (Veronica-2), and their own culture.

There is also a hardware argument: gopher clients exist for machines that
will never speak modern TLS.

**The catch:** menus are a navigational structure, and prose reads less well
through one than gemtext does.

**Reality check:** by far the largest of the cleartext networks: hundreds
of active servers, including Floodgap, SDF, bitreich and the tildeverse.
Spartan and Nex reach a rounding error by comparison.

## Spartan

Home: <spartan://spartan.mozz.us/specification.gmi>, and, since that is
circular if you have no client yet,
<http://portal.mozz.us/spartan/spartan.mozz.us/specification.gmi> ·
Clients: Lagrange, Offpunk, gelim, Profectus, BFG

Spartan is Gemini's document model with the cryptography removed. Its
argument is that mandatory TLS is ceremony for public documents: clocks
that must be roughly right, libraries that must be maintained, hardware that
must do the arithmetic, and that a plain document deserves a plain
protocol.

**Best for:** somewhere encryption is a genuine burden, such as a local
network or a very small machine, or because you find that argument
persuasive. It serves the same gemtext unchanged, so it costs you
essentially nothing.

**Reality check:** a few dozen capsules. The specification has been finished
and stable since around 2021.

**Note:** `usv` refuses Spartan uploads permanently. They are
unauthenticated by construction, and Titan already does uploads properly.

## Nex

Home: <https://nightfall.city/nex/info/specification.txt> · Clients: gelim,
BFG, Profectus

Nex is the smallest thing that still works. No status codes, no content
types, no headers: send a path, receive bytes, the connection closes. It is
explicitly telnet-compatible, so you can speak it by hand.

**Best for:** enjoying that an entire protocol fits in your head, or
hand-rolling a client in an afternoon.

**Reality check:** the smallest audience here, and you will likely need a
specific client to see your own work. Serving it is a gesture more than
reach, which is a perfectly good reason, and roughly why much of the
smolnet exists.

## Finger

Home: RFC 1288 (1991) · Clients: Lagrange, Bombadillo, BFG, and the `finger`
command that ships with many systems

Finger is the internet's oldest status update. It answers "what is this
person up to?": the `.plan` file, which predates blogging by decades and
does the same job in four lines.

**Best for:** a *now* page with no page. One command, a few lines back.

In `usv` it does not serve your content tree at all: it answers with a short
generated profile and points at your capsule.

**Note:** `usv` refuses finger *forwarding* (`user@host` queries), as RFC
1288 itself recommends: answering them turns a server into a relay for
probing hosts that would not answer the asker directly.

## New to all this?

The fastest way in is one client that speaks several protocols, so you can
wander between them without installing something each time.

| Client | Speaks | Kind |
|---|---|---|
| **Lagrange** | Gemini, Gopher, Spartan, Finger, Titan | Graphical; the friendliest start |
| **Bombadillo** | Gemini, Gopher, Finger, HTTP | Terminal |
| **gelim** | Gemini, Gopher, Spartan, Nex | Terminal, line-mode |
| **Offpunk** | Gemini, Gopher, Spartan, HTTP | Terminal; offline-first |
| **BFG** | Gopher, Finger, Nex, Spartan, Gemini | Terminal; the widest coverage |
| **lynx** | Gopher, HTTP | The venerable one; already on many systems |

Try **Lagrange** first if you want a window with fonts and images;
**Bombadillo** or **gelim** if you live in a terminal. Nothing here is an
endorsement: these are simply the clients that exist, as verified in August
2026.

## Field notes

A less careful way of putting it, which is often the way that lands.

| Network | Roughly | Vibe |
|---|---|---|
| **Gemini** | a few thousand capsules | A small town with an active town council |
| **Gopher** | a few hundred active gopherholes | An old lighthouse, still manned |
| **Spartan** | a few dozen sites | A hermit's cabin, very tidy |
| **Nex** | a handful | A hermit's cabin the hermit forgot about |
| **Finger** | no community, and there cannot be one | A doorbell on a house nobody built |

Counts are approximate and move; treat them as orders of magnitude.

**Gopher** lost the protocol war partly because the University of Minnesota
tried to charge licensing fees in 1993, which sent everyone to CERN's free
offering. It is named for the university's mascot. The remaining
gopherphiles have been told it is over for thirty years and are still
here, which is either stubbornness or faith depending on the day.

**Spartan** is what you get by taking Gemini and removing everything that
could be considered a feature. The community's main activity is admiring
the minimalism and then quietly going back to Gemini.

**Nex** is the protocol equivalent of a cover band that plays the songs
better than the original, to an empty pub.

**Finger** deserves the longest note, because the joke is structural.
Building a community on Finger is like building a city on a doorbell. You
ring it, a name and a short note come out, and then you are standing on
the pavement again. There is no inside, no next page, no browse. It has no
concept of linking or listing, so discovery is impossible by design: you
must already know who to ask. If you forced it, you would need a directory
of servers, a formatting convention, and a ring of .plan files pointing at
one another, at which point you have invented Gopher badly and with extra
steps.

Every other network here is a space you can inhabit. Gemini is a room,
Gopher a corridor with doors, Spartan an empty room with good lighting.
Finger is a peephole. You can look through it. You cannot move in.

That is why `usv` serves a generated profile on Finger rather than trying
to serve the content tree through it.

## Before you enable any of the last four

Gopher, Spartan, Nex and Finger are all cleartext:

- **No confidentiality.** Anyone between you and your reader sees what was
  requested and what came back.
- **No integrity.** Anyone on that path can change it in flight.
- **No server authentication.** A reader cannot tell your server from
  someone impersonating it.
- **No client authentication, ever.** These protocols cannot do it.

Serve only content whose integrity loss is acceptable. `usv` enforces the
sharpest edge of this for you: content behind a certificate zone or a Titan
zone is excluded from every cleartext tree when the tree is built, and a
configuration that would publish gated content over one of these protocols
is a startup error rather than a warning.

The counterweight, honestly: this is the settled norm of these communities.
Gopher has run in the clear for thirty-five years, and publishing public
static documents is the one workload where that is defensible. The mistake
is not serving cleartext: it is serving the wrong things in cleartext.

## A reasonable default

Start with **Gemini plus the web mirror**: the living community, and reach
for everyone else. Add **Gopher** when longevity or that community appeals.
Add **Spartan** if the no-encryption argument is one you actually hold. Add
**Nex** and **Finger** because you like them.

# Which protocols should I serve?

You do not have to choose one. `usv` serves a single content tree, so
adding a protocol is mostly a question of *who you want to reach* and
*what you are willing to serve in cleartext* — not of maintaining
another site.

**Status note:** Gemini, Titan and the web mirror work today. Gopher,
Spartan, Nex and Finger are v1.1 and **not implemented yet** — the cases
below describe what each is *for*, so you can decide now what you will
turn on later. [`protocols.md`](protocols.md) is the authority on what
actually exists.

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

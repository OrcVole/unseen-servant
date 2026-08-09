# ADR 0010: Legibility for agents and assistive access — one problem, not two

- Status: **Proposed** (director-raised 2026-08-09; awaiting review)
- Date: 2026-08-09
- Evidence: docs/recon/protocol.md (gemtext grammar, `lang` parameter,
  status classes), docs/recon/ecosystem.md (companion specs), ADR 0004
  (dual surface), ADR 0005 (cert zones, no CGI), ADR 0006 (Titan),
  docs/notes/integration-ideas.md

## Context

The director asked two questions on the same day, and they turn out to
have one answer:

1. Make usv useful to **AI agents** — including agents that would *run*
   a capsule, not merely read one.
2. Provide hooks for people using **voice control, screen readers, or a
   text browser** rather than a pointer.

These look like separate features. They are not. Both audiences need
the same thing: **structure and affordances that are explicit, named,
and addressable, rather than implied by visual arrangement.** A voice
user says "follow *Recent posts*"; an agent resolves the same link by
its name. A screen-reader user navigates by heading and landmark; an
agent walks the same outline. Anything spatial, pointer-dependent, or
implied-by-styling fails both at once. Designing for either produces
most of the other, so this ADR treats them as one requirement.

### Why an agent would want to run a capsule at all

Worth stating plainly, because it is the load-bearing claim and it is
mostly an observation about what usv *already is* rather than a pitch
for new features:

- **Publishing is a file write.** An agent drops `notes.gmi` into a
  directory and it is live on two protocols within the debounce window
  (300 ms). No API call, no auth handshake, no build step, no deploy,
  no CMS. Compared with any web publishing stack, the friction is close
  to zero — and the failure modes an agent hits with those stacks
  (broken templates, failed builds, malformed markup) mostly cannot
  occur here.
- **The output format is nearly impossible to generate wrongly.**
  Gemtext has six line types and no nesting, no escaping, no closing
  tags. An agent emitting gemtext cannot produce the "unclosed div"
  class of bug at all. Generating correct HTML is a real engineering
  task; generating correct gemtext is not.
- **The output is losslessly parseable.** Another agent reading the
  capsule recovers exactly the structure the author wrote — headings,
  links, lists, quotes, preformatted blocks — with a one-pass parser
  and one bit of state (`render::gemtext`). Round-tripping HTML does
  not work like that.
- **Client certificates are identity without an account system**
  (ADR 0005, shipped in C2). An agent presents a certificate; the
  capsule authorises by SHA-256 fingerprint. No user database, no
  OAuth, no password, no session. That is very close to the natural
  shape of machine identity, and it already exists.
- **Status codes are already a machine interface.** Gemini's 10/20/30/
  40/50/60 classes are unambiguous and exhaustive; there is no
  "200 OK containing an error page" ambiguity to disambiguate. Nothing
  needs adding here.
- **Titan (ADR 0006, phase C4) makes publishing remote and gated.** An
  agent with a certificate could publish over the network without
  filesystem access at all. That is the missing half, and it is the
  next phase rather than a speculative addition.

The honest summary: usv suits agents largely *by accident*, because the
properties that make Gemini pleasant for people — small, plain, no build
step, certificate identity — are the same properties that make a
publishing surface easy for a program to drive. This ADR mostly
protects and completes that accident rather than inventing on top of it.

### The risk worth naming

"AI agent features" is a fashionable phrase and an easy way to build
junk: a bespoke manifest schema nobody else speaks, an "agent mode",
a JSON API bolted onto a protocol that deliberately has none. Every
one of those would add surface, contradict the brief's minimalism, and
serve a hypothetical consumer. This ADR therefore has a bias: **prefer
things that are simultaneously an accessibility win, a human-usability
win, and an agent win.** Anything that helps only the hypothetical
agent is refused or deferred.

## Decision

### 1. Accept that the two audiences share one requirement

Every affordance usv grows — now and in future phases — must be a
**named, addressable thing**, never a spatial or visual one. Concretely,
as a standing rule for later phases: an action is a link with a stable
name and a direct URL (ADR 0009's "a like is a page you visit" already
follows this); nothing depends on pointer position, colour alone, or
layout; and no affordance requires JavaScript, which usv does not emit
in any case.

### 2. Fix language declaration (a real defect, not an enhancement)

`text/gemini` carries an optional `lang` parameter holding BCP 47 tags
(recon: protocol.md §"MIME and charset handling"). usv currently
hardcodes `<html lang="en">` in the HTML emitter and never emits `lang`
on the Gemini side. For a non-English capsule this is an accessibility
bug with a concrete victim: a screen reader uses `lang` to choose
pronunciation rules, so a French capsule is read aloud with English
phonetics. Decision: a configured `server.lang` (BCP 47, default `en`)
sets the HTML `lang` attribute and is emitted as the `lang` parameter
on `text/gemini` responses.

### 3. Emit a complete site map on both surfaces

The one thing an arriving agent cannot cheaply get today is *what pages
exist*. Feeds cover only dated posts (ADR 0004 / `render::metadata`);
everything else requires crawling. usv already walks the entire content
tree on every render, so it holds the complete answer for free.

Decision: every render emits a site map — `/map.gmi` for Gemini (and,
rendered, `/map.html`), and `sitemap.xml` for the web, the latter being
an actual established standard rather than an invention.

This is deliberately chosen because it serves all three audiences at
once: it is WCAG 2.4.5 ("Multiple Ways") for assistive users, an
ordinary useful index page for a human, and a complete crawl-free
inventory for an agent. It invents no schema and costs one extra file
write per render.

### 4. Semantic landmarks and a skip link on the HTML surface

The HTML emitter already produces semantic, classless markup. Add the
navigational scaffolding assistive technology actually keys on:
`<main>` around the content, a skip-to-content link as the first
focusable element, and a visible focus style. Cheap, standards-plain,
and the direct answer to "voice or other means instead of keyboard" —
these are the hooks that let a voice user jump without a pointer.

### 5. What is refused, and why

- **No content negotiation between agents and humans.** Serving
  different content by user-agent is the cloaking pattern: brittle,
  adversarial, and impossible to verify. One surface legible to both is
  the entire ADR 0004 thesis, and it already works.
- **No bespoke capability manifest / JSON schema.** A format only usv
  speaks has no consumers. If a capsule wants to advertise what it
  offers, that is *content* — an ordinary page saying so, which every
  client and every agent can already read. Revisit only if a real
  cross-implementation convention emerges.
- **No "agent mode", no API surface, no JavaScript.** ADR 0005 closed
  the execution surface deliberately; this ADR does not reopen it.
- **usv does not rewrite authored link text.** A bare-URL link
  (`=> gemini://…` with no name) reads badly aloud, but the fix belongs
  in authoring guidance, not in silently altering an author's content.
  Documented in the authoring docs instead.

## Consequences

- Three small, orthogonal features land now (`lang`, site map,
  landmarks); each is independently defensible on accessibility grounds
  alone, so none of them is a bet on agents materialising.
- The "named, addressable affordances" rule constrains C4 (Titan) and
  ADR 0009 (responses) before their code exists, which is the cheapest
  moment to constrain them.
- The site map adds one generated file per surface. Like `feed.gmi`, the
  generated name is reserved and the watcher must ignore it or a render
  would trigger itself (the loop already fixed once for the feed).
- If the agent audience never materialises, nothing here is wasted: it
  is all accessibility and ordinary usability work that stands on its
  own. That asymmetry is why these particular items were chosen over the
  more speculative ones, which are recorded in
  `docs/notes/integration-ideas.md` and deliberately not built.

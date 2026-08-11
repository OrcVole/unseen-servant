# What each community calls its own things

Researched 2026-08-11, because `usv` serves five networks from one
codebase and had been using **one community's word on all of them**. The
shared colophon says "capsule" in the text served to Gopher, Nex, Spartan
and Finger readers, and the finger profile says "A capsule served by
Unseen Servant". To a gopher user that is an outsider's word, and using
it is the fastest way to signal that we have not been there.

## The findings

| Network | Their site | Their writing | The space | Other native words |
|---|---|---|---|---|
| **Gemini** | **capsule** | **gemlog** (a post is a gemlog entry) | **Geminispace** | gemtext (the format); "capsuleer" for a person, chiefly around Station |
| **Gopher** | **gopherhole** (also "gopher hole") | **phlog** | **gopherspace** | gophermap (the menu source); selector (the path); "gopherlog" exists but is rare |
| **Spartan** | *no distinct term*: "site" | *none* |: | Community too small to have coined one |
| **Nex** | *no formal term*: the spec says "site"; m15o writes "**nex stop**" | *none*: m15o keeps a "journal" and "notes" |: | "smolweb" circulates around nightfall.city |
| **Finger** | *there is no site*: there is a **person** | **`.plan` file** (plus `.project`) |: | "to finger someone" is the verb |

### Notes on each

**Gemini** is the only one of the five with a full, settled vocabulary,
and it is the vocabulary this project drifted into using everywhere.
Worth noting that even the official Gemini FAQ does not *define* capsule
: it just uses it.

**Gopher** has the second-richest vocabulary and it is genuinely alive:
"gopherhole" and "phlog" are both in current use in 2026 across Floodgap,
SDF, bitreich and the `#gopherproject` channel. **phlog** is "blog" with
gopher's `ph` in place of the web's `b`, coined by Jeff Woodall in April
2003. Bongusta, the aggregator the recon already names, is explicitly a
*phlog* aggregator.

**Spartan** has no word of its own. Its people say "site". It borrows
Gemini's document format, but there is no evidence it borrowed "capsule",
and assuming it did would be inventing usage.

**Nex** likewise has no formal term. The specification itself is
strikingly bare: it says "Document content" and "Directory content" and
uses "site" once, in an example hostname. m15o, who wrote Nex and runs
nightfall.city, writes "**nex stop**", which fits that hub's transit
metaphor (Nightfall City, the Nightfall Express) rather than being a
protocol-wide noun. Treat it as m15o's usage, not a community standard.

**Finger** is the interesting one, and the answer to "what about the
finger community?" is: **there is not one, and that is the point.** Finger
has no sites, so it has no word for a site. Its unit is a *person* and
its artefact is the **`.plan` file**: a free-text file in your home
directory that the protocol hands to whoever asks, and which predates
blogging by decades while doing much the same job. `.project` is its one
companion, conventionally a single line about what you are working on.
The living context is the tildeverse, where `.plan` is used for exactly
the micro-status it always was.

**The umbrella.** "Smolnet", or "the small internet", is the term with the
widest reach: it is the Gemini community's, and the community wiki and
Archiveteam both use it. "Smolweb" is narrower, circulating around
nightfall.city and m15o. Prefer **smolnet**, which is what this project
already uses.

## House rules that follow

1. **Never use one network's word on another network's wire.** The word
   in served content is chosen by the protocol the reader is holding:

   | Reader is on | Say |
   |---|---|
   | Gemini, Titan, the web mirror | capsule |
   | Gopher | gopherhole |
   | Spartan | site |
   | Nex | site |
   | Finger | *(neither: address the person; the artefact is a profile)* |

2. **When one sentence has to cover all five, use neither.** "One folder
   of writing", "the same pages", "these files" all work and none of them
   is anyone's in-group word. The colophon already does this in places
   ("This capsule is one folder of writing, rendered to each of these", 
   the second half is right, the first half is not).

3. **Do not invent a term where a community has none.** Spartan and Nex
   have no word; "spartan capsule" and "nexlog" are both things we would
   be making up. ("nexlog" in particular is not a Nex term at all: it is
   an unrelated commercial product, and an example path in the Nex spec
   that reads `../nexlog/` is just an example.)

4. **In our own documentation**, where the audience is all five at once,
   "capsule" is acceptable *if* it is introduced as the Gemini word and a
   neutral alternative is used thereafter. Where a page is about one
   network: `docs/smolnets.md`'s per-network sections, for instance, 
   use that network's own vocabulary.

## Still to fix

- `src/render/colophon.rs` uses "capsule" in the shared text emitted to
  every protocol, including the four cleartext ones.
- `src/handler/finger.rs` opens "A capsule served by Unseen Servant".
  Finger has no capsules; this should speak about the person and the
  `.plan`.
- 148 occurrences of "capsule" across the documentation, most of them
  fine under rule 4, some of them not.

## Sources

Accessed 2026-08-11.

- <https://en.wikipedia.org/wiki/Phlog>: phlog definition, the "ph"
  derivation, Jeff Woodall's 2003 coinage, gopherlog as a rare variant.
- <https://www.ecliptik.com/blog/2021/Making-a-Gopherhole-and-Phlog/>, 
  "gopherhole" as the ordinary word for a gopher site.
- <https://wiki.archiveteam.org/index.php/SmolNet>: "Gopherspace",
  "Geminispace", "capsules", "gopher hole"; smolnet as the umbrella.
- <https://geminiprotocol.net/docs/faq-section-6.gmi>: smolnet as "an
  online counter-cultural movement which has a lot of currency in the
  Gemini community"; capsule used but never defined.
- <https://nightfall.city/nex/info/specification.txt>: the Nex spec's
  deliberately bare vocabulary.
- <https://nightfall.city/nex/in/m15o/notes/nex-and-small-net.txt>, 
  m15o's "nex stop".
- <https://en.wikipedia.org/wiki/Finger_(protocol)> and
  <https://tilde.team/wiki/?page=finger>: `.plan` and `.project`, and
  the tildeverse's continuing use of them.
- <https://tilde.cafe/wiki/spartan>: Spartan users say "site".

---
title: "Launch pack"
description: "Drafts for the announcement wave (docs/internal/ROADMAP.md M6). Nothing here has been posted, and nothing here may be posted without the director saying so. These are copy held in escrow,."
type: reference
status: decided
last_verified: 2026-08-30
---

# Launch pack

Drafts for the announcement wave (`docs/internal/ROADMAP.md` M6). **Nothing here
has been posted, and nothing here may be posted without the director
saying so.** These are copy held in escrow, not a queue.

Every venue has different norms. The same paragraph that reads as
helpful on the Cloudron forum reads as marketing on the Gemini mailing
list. That is why these are separate files rather than one press release.

## Before anything is sent: the claim gate

Announcement copy goes stale in exactly one direction: it keeps
claiming things that were true when it was drafted, or that were hoped
for and never landed. Walk this list against the code, not against
memory, on the day of sending.

| Claim | Verify by | True on 2026-08-30 |
|---|---|---|
| Protocols supported | [`../../protocols.md`](../../protocols.md): must match its table exactly | Gemini, Titan, web mirror, **Gopher, Spartan, Nex, Finger**: all seven with a named client |
| Cleartext protocols are off by default and carry no gated content | `docs/protocols.md` §Cleartext, ADR 0012 §6 | Yes; say so wherever a cleartext protocol is named |
| Packaging available | Does a *published* package exist, or only a build script? | Build scripts only (`packaging/`); no repositories, no release artefacts |
| A release exists | `git tag` | **No tags.** Version 0.1.0 |
| Repo is public | Forgejo repo settings; anonymous `GET /api/v1/repos/...` | **Private** (404 anonymously); no GitHub mirror yet |
| Capsule is live | Visit it | Yes: `unseenservant.dev`, all six surfaces (1965, 443, 1024, 3300, 1900, 7979) |
| `gemini-diagnostics` result | Re-run it; quote the real number **and the three names** | 25/27; the three non-passes are documented artefacts in `DEBUGGING.md` §Conformance (`TLSClaims`, `URLByIPAddress`, `IPv6Address`), the last still owed a re-run from a host with IPv6 |
| Fuzz targets | `ls fuzz/fuzz_targets` | **Eight** (gopher selector added with ADR 0012) |
| Themes | `src/render/theme.rs` | **Three**: Ember (default), Phosphor, Burrow |
| Version / v1.0 status | `Cargo.toml`, `docs/internal/ROADMAP.md` | 0.1.0, pre-1.0 |
| Test/LOC figures | `cargo test` and [`../../architecture.md`](../../architecture.md) | 632 tests; about 11,000 code lines |

**The rule:** if a line of copy names a protocol, a package repository,
or a version, it must be checkable in under a minute. If it cannot be
checked, cut it.

### The capsule's own pre-release notice

`content/index.gmi` carries a paragraph asking whoever found the site
early not to share it, and `compare.gmi`, `install.gmi` and `status.gmi`
each say pre-release too. **Remove or rewrite all four before the first
announcement** (director, 2026-08-30): the notice is doing its job now
and would read as false modesty the moment a link is posted anywhere.
Grep for "pre-release" and "unannounced" across `content/` rather than
trusting this list, and check `status.gmi` says what v1.0 actually is.
This is the last content change before the gemlog post, not the first.

### The smolnet protocols (was "When v1.1 lands")

This section used to say: *if the announcement waits for Gopher, update
`../protocols.md` first, re-run this gate, and only then edit the drafts.*
That happened in the other order from the one feared: the four protocols
shipped on 2026-08-10 and 2026-08-11, `protocols.md` was updated with
them, and the drafts sat for nineteen days still saying "no Gopher yet".
Corrected 2026-08-30. The rule stands and is now proven from both sides:
the protocol table is the authority; the drafts follow it. Never the
reverse.

Gopher unlocks a second venue set: gopher-project mailing list, Bongusta,
Floodgap/Veronica-2, `#gopher` on Libera: enumerated in
`../recon/smolnet.md` §6. **No drafts exist for them yet**; that is owed
before the second wave, and the copy must use gopher's own vocabulary
(a gopherhole, not a capsule: `../notes/terminology.md`).

## The AI disclosure question

`usv` is AI-authored. Parts of the smolnet community are strongly
hostile to AI-generated software, and Geminispace in particular is
small, opinionated, and values craft and human scale.

**Lead with it. Every time.** Not buried in a footer, not omitted from
the short posts. Two reasons, one principled and one practical: people
are entitled to that information before they run it on their server, and
being upfront invites a fair argument whereas being *discovered* later
converts a disagreement about tooling into a question about honesty,
which is unrecoverable.

Do not argue the point in the announcement. State it plainly, link the
ADRs and recon so the work can be judged on its merits, and let people
reach their own conclusion. Expect some flat rejections; they are a
legitimate response, and arguing back in-thread will cost more than the
rejection does.

## Venues

Gemini-side, from `docs/internal/ROADMAP.md` M6. Re-verify each is alive and its
submission mechanics have not changed *at launch time*: several are one
person's capsule.

| Venue | Kind | Draft |
|---|---|---|
| Antenna | Feed aggregator: you post a gemlog entry, it is picked up | [`antenna-gemlog.gmi`](antenna-gemlog.gmi) |
| Gemini mailing list | Plain-text, technical, low tolerance for marketing | [`mailing-list.txt`](mailing-list.txt) |
| Station / Bubble | Community boards inside Geminispace | [`station-bubble.gmi`](station-bubble.gmi) |
| awesome-gemini | A one-line PR against a list | [`awesome-gemini.md`](awesome-gemini.md) |
| r/geminiprotocol | Reddit; states limitations or gets picked apart | [`reddit.md`](reddit.md) |
| Fediverse `#gemini` | Short, one image | [`fediverse.md`](fediverse.md) |
| Cloudron forum | Packaging angle; a thread already exists | [`cloudron-forum.md`](cloudron-forum.md) |
| geminiprotocol.net, geminispace.info | Software listings: submission, not announcement | *(no copy needed; follow each site's process)* |

## Sequencing

1. **Repo public, capsule final, gate walked.** Nothing before this.
2. **The gemlog post first**, on the project's own capsule. Everything
   else links to it, so it must exist first, and Antenna picks it up
   from the feed rather than from a submission form.
3. **Mailing list**, then **Station/Bubble**. Slow, conversational
   venues; be present to answer.
4. **awesome-gemini PR** and the **software listings**: these are
   durable and worth more long-term than any thread.
5. **Reddit and Fediverse last**, and only if there is appetite to sit
   with the replies. A thread nobody tends reads worse than no thread.

Do not fire them all in one day. The point is to be read by a few
hundred people who care, not to trend.

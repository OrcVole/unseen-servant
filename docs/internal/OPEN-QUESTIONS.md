---
title: "Open questions for the director (n)"
description: "Questions the AI cannot resolve from the brief, recon, or sensible defaults. Answered items move to the relevant ADR with the answer recorded."
type: explanation
status: decided
last_verified: 2026-08-30
---

# Open questions for the director (n)

Questions the AI cannot resolve from the brief, recon, or sensible
defaults. Answered items move to the relevant ADR with the answer
recorded.

## OQ-1: ADR 0007: config format (brief truncated)

- Raised: 2026-08-09
- Status: **resolved (director, 2026-08-09): one TOML file confirmed**
  ("unless there are distinct advantages to having multiple ones",
  ADR 0007 now records the multiple-file evaluation: no distinct
  advantage at usv's scale; single file stands).

## OQ-3: Domain name and project capsule host

- Raised: 2026-08-09
- Status: **resolved (director, 2026-08-10): `unseen-servant.wanderingmonster.dev`.**
  `unseen-servant.dev` confirmed available (RDAP query against the
  authoritative Charleston Road Registry server, 2026-08-10, not just
  the Porkbun search UI, which is JS-driven and unreliable to scrape;
  $8.75 first year / $12.87/yr per the 2026-08-09 Porkbun check) but
  not purchased; may still be bought later before a v1.1 announcement
  and pointed at the same capsule (SNI + multiDomain handle it) per
  the original recommendation below: that option stays open, it is
  just not the answer to *this* question.
- **Superseded (recorded 2026-08-30, event of 2026-08-10/11):** the
  director bought `unseenservant.dev` (no hyphen) on 2026-08-10 and it
  was pointed at the production Cloudron on 2026-08-11: A record first,
  then the domain added to Cloudron, then set as the app's location with
  `www.unseenservant.dev` and `unseen-servant.wanderingmonster.dev` as
  aliases. It has served the capsule on all six surfaces since, with a
  Let's Encrypt certificate. The old answer above is kept because the
  reasoning in it (SNI + multiDomain make a later domain cheap) is what
  made the move a one-command change. Standing permission: serving there
  is fine; **announcing it is not**, until the director says so.

## OQ-4: License

- Raised: 2026-08-09
- Status: **resolved (director, 2026-08-09): MIT.**
- LICENSE file to be created at scaffold time; all Cargo metadata,
  docs, and package manifests state MIT. (Agate code is Apache-2.0/MIT
  dual-licensed, so studying/adapting under MIT is compatible with
  attribution.)

## OQ-5: Multi-protocol scope (Spartan / Nex / gopher / Scroll)

- Raised: 2026-08-09
- Status: **resolved (director, 2026-08-09): gopher, Spartan, Nex,
  and (added later the same day) Finger are scheduled** for v1.1; see
  ROADMAP. Scroll stays watch-only.
  Implementation-grade recon of what each protocol needs server-side,
  plus a survey of further smolnet protocols ("there may be others"),
  commissioned 2026-08-09 → docs/internal/recon/smolnet.md. The multi-protocol
  ADR is written after that recon lands.

## OQ-6: Repo hosting and community optics

- Raised: 2026-08-09
- Status: **resolved (director, 2026-08-09): primary repo on the
  Wandering Monster Forgejo instance; GitHub (orcvole) as backup
  mirror.** Forgejo-primary also fits smolnet culture. Release
  artifacts publish to both; the awesome-gemini listing links the
  Forgejo canonical.

## OQ-7: First commit

- Raised: 2026-08-09
- Status: **resolved (director, 2026-08-09): "lets get some
  commits"**: local commits begun same day. Remaining sub-question:
  the Forgejo remote URL (Wandering Monster instance) and the orcvole
  GitHub mirror URL, needed before anything can be pushed.
- **Update 2026-08-30:** the Forgejo remote exists and is `origin`
  (`forgejo.wanderingmonster.dev/WanderingMonster/unseen-servant`,
  private). The GitHub mirror **does not exist yet**; creating it is
  part of the going-public batch, OQ-10 item 6.

## OQ-8: Responses feature: version and scope

- Raised: 2026-08-09 (director: gemlogs on usv should easily offer a
  "responses" section: likes and messages; Astrobotany admired)
- Status: open: ADR 0009 drafts after docs/internal/recon/community-wisdom.md
- For the director when the ADR drafts: (a) which release carries it
  (it is real, security-sensitive work: spam, moderation, identity);
  (b) are anonymous web-side likes acceptable, or should all
  interaction require some identity (client cert on Gemini; what on
  the web?); (c) moderation default: hold-everything (recommended)
  or publish-then-moderate.

## OQ-2: Project naming

- Raised: 2026-08-09
- Status: resolved (director, 2026-08-09)
- The brief fixes the name as "Unseen-Servant or unseen-servant"; the
  director subsequently suggested `usv` for the binary. Convention
  adopted: prose name **Unseen Servant**, crate / package / repo name
  **unseen-servant**, binary name **`usv`**.

### Why `usv`, and not `uns` or `uss` (recorded 2026-08-10)

No rationale was captured at the time. The name was **proposed by the
assistant**, not the director, when the daemon needed one: a detail
this file previously got wrong in the other direction, which is its own
small lesson about recording provenance while it is still fresh.

The following is therefore a *post-hoc* mnemonic rather than a recovered
reason, and is marked as such so nobody later mistakes it for one:

> **U**n**S**een ser**V**ant → **usv**

The letters are genuinely in the name, in order, which is what makes it
worth using in outward-facing copy. Reasons to prefer it over the
obvious alternatives, also supplied now rather than then: `uns` reads
as "unsigned" to anyone who writes code; `uss` collides with the ship
prefix and looks like a typo for "us"; `usv` is short, unambiguous
letter-by-letter, and is not already a common command name.

**Nobody encountering a served page knows any of this**, which is the
actual problem: see the colophon work below.

## OQ-9: Per-request logging records the visitor's IP address

- Raised: 2026-08-10 (found while writing `docs/security.md`)
- Status: **resolved (director, 2026-08-10): "do the not surveillance
  state thing": implemented same day.**
- **Resolution:** a `server.log_peer` setting with three values, and the
  privacy-preserving one is the default:
  - `"off"`: **the default.** No visitor address in the logs at all;
    the field renders as `-` so the line keeps one shape and stays
    greppable.
  - `"hashed"`: a 48-bit digest of the *address only* (never the
    ephemeral source port, which changes per connection and would
    defeat the point) under a salt generated fresh at every start and
    never persisted. Repeat visits correlate within one run of the
    process: enough to see one client hammering a path, and nothing
    survives a restart, because the salt does not.
  - `"full"`: the address verbatim, for an operator who has decided
    they want a conventional access log. Deliberately the value you
    have to type out.
- The abuse-investigation need that made this a question rather than a
  bug is met by the two opt-in modes; what changed is which way the
  default falls, and that the operator now makes the choice
  deliberately instead of discovering they had already made it.
- Implementation note: the address is wrapped in a `PeerLabel` type at
  the top of the connection handler and the raw `SocketAddr` is shadowed,
  so logging the real address is not something that can happen by
  reaching for the wrong variable. A mistyped `log_peer` is a startup
  error listing the valid values, because failing open here would
  silently keep addresses an operator believed they had turned off.
- Verified live, not just unit-tested: all three modes run against a
  real server, confirming `off` emits `peer=-`, `full` emits the
  address, and `hashed` produces the *same* digest for two requests
  whose source ports differed: the port-exclusion behaviour the mode
  depends on.

## OQ-10: The going-public batch

- Raised: 2026-08-30, from the launch plan
  (`packages/unseen-servant/phase-notes/PLAN-TO-PUBLIC-2026-08-30.md`,
  outside the repository because it is round material).
- Status: **decided (director, 2026-08-30): the suggested option in
  every item except 3 and 6.** Applied the same day: ADR statuses
  flipped, gate wording amended in AGENTS.md, BUILD-PLAN and ROADMAP,
  port 79 recorded in `docs/deployment/cloudron.md`. Items 4, 5 and 7
  are decided but not yet due (Phases 4 and 5 of the plan). Item 8 is
  the director's own chore.
- **Item 3, overturned:** the director wants **no reference to the
  production host anywhere in this repository, history included**.
  Chosen route: rewrite the full history locally with `git filter-repo`
  (every commit kept; the host names, the labs Cloudron name and the
  `Co-Authored-By` trailers replaced or removed in every file version
  and every message), and push that to a **new** repository, so nothing
  that exists is ever force-pushed. Dry run 2026-08-30: 105 commits in,
  105 out, zero matches for any of the terms afterwards.
- **Item 6, amended:** the existing repository is renamed
  `unseen-servant-old` and kept private as the untouched original; a
  new `unseen-servant` takes the rewritten history under the same URL,
  so `Cargo.toml`, the awesome-gemini draft and the estate register
  need no change. Repository secrets (`REGISTRY_PUSH_TOKEN`) must be
  recreated on the new repository before CI can push an image. Public
  visibility and the GitHub mirror follow later, once Phase 3's sweep
  has run on the new repository.
- The nine, as put, with the suggested option first:

1. **ADR statuses.** 0010, 0011 and 0012 are implemented and shipped
   but still *Proposed*; 0010 has been raised three times. Suggest:
   Accept all three, amending any point overturned. ADR 0009
   (responses) needs a version: suggest post-1.0, since it is real,
   security-sensitive work and not a launch blocker.
2. **The conformance gate wording.** The brief says "pass clean";
   the number is 25/27 with three documented artefacts
   (`DEBUGGING.md` §Conformance). Suggest: re-run `IPv6Address` from
   a host with IPv6 egress, then amend the gate text (BUILD-PLAN E9,
   ADR 0001 evidence, the AGENTS.md invariant) to "27/27, or a
   documented spec-legitimate non-pass". Alternative: chase 27/27 by
   changing behaviour the spec allows, which would be optimising for
   the tool.
3. **History before the repo goes public.** Five commits mention
   the production host by name (a public domain name, no addresses or
   credentials).
   Suggest: accept it; scrub the working tree and add a
   `test/secret-scan.sh` gate. Alternative: publish from a fresh
   squashed repository, which loses the ADR-by-ADR history the
   AI-disclosure story leans on.
4. **Cloudron distribution route.** Suggest: a versions feed, as the
   estate's other nineteen packages use (auto-update, proven path),
   then App Store submission after the feed has served one update.
   Alternatives: store first; image-only installs as documented today.
5. **Artefact signing.** Suggest: yes, `minisign` (one key, fits the
   smolnet audience's tooling), sums and signatures on every Forgejo
   Release from v1.0.0. Alternative: sha256 sums only, sign later.
6. **Repo public and the GitHub mirror.** Suggest: flip Forgejo to
   public and create `orcvole/unseen-servant` as a Forgejo push mirror
   in the same sitting, once items 2 and 3 are done. Needs a GitHub
   token that can create the repository.
7. **The announcement go**, and who sits with the replies for a day
   on Reddit and the Fediverse. Suggest: gemlog post and mailing list
   first; Reddit and Fediverse only if someone will be present.
8. **The two claude.ai pages** listed in the workspace's
   `PUBLISHED-PAGE-TO-DELETE.md`: UI deletion only.
9. **Port 79 → 7979 redirect** so bare `finger user@host` works.
   Suggest: leave it (assessed 2026-08-11: Docker rewrites `nat
   PREROUTING` on every container recreation; a systemd socket unit
   would be the way if wanted).

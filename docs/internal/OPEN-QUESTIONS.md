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

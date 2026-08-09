# Open questions for the director (n)

Questions the AI cannot resolve from the brief, recon, or sensible
defaults. Answered items move to the relevant ADR with the answer
recorded.

## OQ-1: ADR 0007 — config format (brief truncated)

- Raised: 2026-08-09
- Status: **resolved (director, 2026-08-09): one TOML file confirmed**
  ("unless there are distinct advantages to having multiple ones" —
  ADR 0007 now records the multiple-file evaluation: no distinct
  advantage at usv's scale; single file stands).

## OQ-3: Domain name and project capsule host

- Raised: 2026-08-09
- Status: narrowed (director, 2026-08-09) — final pick pending
- Director's candidates: `unseen-servant.wanderingmonster.dev`
  (subdomain of an owned domain, zero cost) or buying
  `unseen-servant.dev`. Porkbun pricing verified 2026-08-09:
  .dev renews at $12.87/yr (first year $8.75) — cheap and stable.
  Recommendation: start on the wanderingmonster.dev subdomain now;
  optionally buy unseen-servant.dev before the v1.1 announcement and
  point both at the same capsule (SNI + multiDomain handle it).
  Availability of unseen-servant.dev not yet checked — director to
  search on Porkbun.

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
  commissioned 2026-08-09 → docs/recon/smolnet.md. The multi-protocol
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
  commits"** — local commits begun same day. Remaining sub-question:
  the Forgejo remote URL (Wandering Monster instance) and the orcvole
  GitHub mirror URL, needed before anything can be pushed.

## OQ-8: Responses feature — version and scope

- Raised: 2026-08-09 (director: gemlogs on usv should easily offer a
  "responses" section — likes and messages; Astrobotany admired)
- Status: open — ADR 0009 drafts after docs/recon/community-wisdom.md
- For the director when the ADR drafts: (a) which release carries it
  (it is real, security-sensitive work: spam, moderation, identity);
  (b) are anonymous web-side likes acceptable, or should all
  interaction require some identity (client cert on Gemini; what on
  the web?); (c) moderation default — hold-everything (recommended)
  or publish-then-moderate.

## OQ-2: Project naming

- Raised: 2026-08-09
- Status: resolved (director, 2026-08-09)
- The brief fixes the name as "Unseen-Servant or unseen-servant"; the
  director subsequently suggested `usv` for the binary. Convention
  adopted: prose name **Unseen Servant**, crate / package / repo name
  **unseen-servant**, binary name **`usv`**.

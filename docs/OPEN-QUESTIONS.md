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
- Status: **resolved (director, 2026-08-10): `unseen-servant.wanderingmonster.dev`.**
  `unseen-servant.dev` confirmed available (RDAP query against the
  authoritative Charleston Road Registry server, 2026-08-10 — not just
  the Porkbun search UI, which is JS-driven and unreliable to scrape;
  $8.75 first year / $12.87/yr per the 2026-08-09 Porkbun check) but
  not purchased; may still be bought later before a v1.1 announcement
  and pointed at the same capsule (SNI + multiDomain handle it) per
  the original recommendation below — that option stays open, it's
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

## OQ-9: Per-request logging records the visitor's IP address

- Raised: 2026-08-10 (found while writing `docs/security.md`; not
  previously surfaced as a question)
- Status: **open — decision wanted before v1.0 / before announcing**
- What happens today: `src/server.rs` emits one `info`-level line per
  request carrying `%peer` (the client IP), the status, and the path.
  The **query string is redacted by construction** — deliberate, and
  correct, since Gemini's status 10/11 input flow puts user-typed text
  (including passwords) in the query. The IP is not redacted.
- Why it's a question rather than a bug: an operator debugging abuse
  genuinely wants the address, and `usv` writes only to stdout/stderr
  and keeps no files, so nothing is *retained* by usv itself — but the
  platform's journal usually retains it for weeks.
- Why it matters more here than for a web server: this project's own
  `docs/recon/community-wisdom.md` §3 records that aggressive log
  minimalism is a *stated norm* in Geminispace — "operators boast of
  *not* keeping IPs", some map them to ephemeral IDs discarded within
  the hour — and that privacy-preserving aggregate counters fit the
  culture better than access logs. An announcement thread is a likely
  place for this to be raised by someone who checks.
- Options: (a) leave as-is and document (today's state); (b) redact or
  truncate the IP by default with an opt-in to full addresses;
  (c) a `server.log_peer` setting, defaulting to off; (d) hash the IP
  with a per-boot salt so repeat visits correlate within a session but
  nothing durable is written.
- Recommendation: (c) or (d). Both keep the abuse-debugging story while
  matching the community's stated expectation; (d) also preserves the
  aggregate-counter use case the recon says operators actually want.

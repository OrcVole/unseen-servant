# ADR 0009: Responses: likes and messages, dynamic write / static read

- Status: Proposed (drafted 2026-08-09; awaiting director's OQ-8
  calls: release version, and per-post counter display default)
- Date: 2026-08-09
- Evidence: docs/internal/recon/community-wisdom.md, docs/internal/notes/integration-ideas.md, director directives 2026-08-09

## Context

The director wants gemlogs hosted on usv to easily offer a
"responses" section: visitors optionally click a like or leave a
message: citing Astrobotany's playful cert-based interaction as
inspiration, and proposed the like mechanism directly: "visit the
like page to show you liked us".

Community recon confirms the design space is already settled by
practice. The interaction vocabulary of Geminispace's application
school (Astrobotany, Station, Bubble) is exactly three primitives:
cert-identified link-click verbs, status 10 input for free text, and
input-as-confirmation. Bubble: the most successful Gemini
interaction platform: is literally a server extension module beside
static serving, validating interaction-as-server-feature. Staticman
is the web-side precedent for dynamic-write/static-read. And the
community's own warning (bacardi55, widely shared): hosting
strangers' words is a liability an operator must opt into, and
like-counters import the dopamine economics the smolnet exists to
escape.

## Decision

**Opt-in per site** (`[responses]` config section; absent = feature
fully off, no endpoints exist). Two verbs, one principle: **dynamic
write, static read**: submission endpoints are the only dynamic
code; display is produced by the normal static render after operator
approval. This walks through the internal-handler door ADR 0005
reserved; it is not CGI and executes nothing.

**Likes, "visiting the like page":**
- Each enabled post's rendered page links `/like/<post>`. Visiting
  it counts the like and the page answers with thanks and the tally
  ("you already liked this" on cert revisit).
- Gemini side: the like page requests a client certificate (status
  60: one click in every maintained client; the community idiom is
  universally cert-identified likes, never anonymous). Dedupe per
  fingerprint.
- Web side: rendered as a one-button no-JS POST form (a GET with
  side effects would be inflated by prefetchers); dedupe per salted
  hashed-IP-day. robots.txt (both surfaces) disallows the like
  paths; web adds nofollow.
- Counters are quiet by default: the tally lives on the like page;
  rendering counts onto post pages is a per-site opt-in (default per
  director's OQ-8 answer).

**Messages:**
- Gemini side: status 10 input, one line (Astrobotany message-board
  shape), per-fingerprint rate limits; certificate required.
- Web side: plain no-JS form with the layered anti-spam from
  docs/internal/notes/integration-ideas.md: honeypot, stateless HMAC
  time-trap token, salted-IP rate limits, length/link-count/dup
  caps, optional operator question. No captchas, no external
  services, ever.
- **Moderation-first, non-negotiable default**: every message enters
  a bounded pending queue; nothing renders until approved via
  `usv responses` (CLI; Cloudron file-manager-friendly on-disk
  format; ratatui queue screen later). Approval triggers re-render.
  A publish-then-moderate mode is deliberately not offered in the
  first version: recon shows moderation-first is a community value,
  and offering the unsafe mode invites operators into the liability
  bacardi55 warns about.

**Storage**: flat files under `${state_dir}/responses/` (pending/ and
approved/ trees, one file per response, content-addressed names), 
no database, file-manager-editable, backed up with everything else.

**Refused**: anonymous Gemini likes (against universal idiom;
gemlikes' IP-hash identity is the community's cautionary tale);
external anti-spam/analytics services; public aggregate metrics
("trending", leaderboards): usv serves capsules, not engagement.

## Consequences

- usv gains its first dynamic write path: two small endpoints whose
  entire attack surface is bounded, fuzzable, and off by default.
- The render pipeline's re-entrancy (already required for Titan)
  gains a second consumer; approval = a write event like any other.
- The Astrobotany-style playful direction (growable things, verbs
  beyond "like") stays open: the primitives this ADR introduces, 
  cert identity, link-verbs, input lines, per-route rate limits, 
  are exactly Astrobotany's vocabulary, so future whimsy is
  configuration of existing machinery, not new architecture.
- Release version: **recommendation v1.2** (after v1.0 launch and
  the v1.1 smolnet release), so the announcement ships a hardened
  core and responses ship with the attention they deserve, 
  director may promote it (OQ-8).

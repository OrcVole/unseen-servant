# Smolnet community wisdom: interaction, theming, and feature culture

**Date:** 2026-08-09. **Status:** research complete. Feeds the planned
"responses" feature (likes + visitor messages on gemlog posts), theme
support, and the general feature roadmap.

**Summary.** Geminispace already has a rich, working interaction culture,
and it splits cleanly into two schools. The *application* school
(Astrobotany, Station, Bubble) proves that client certificates + status 10
INPUT prompts + plain link-click actions are a complete interaction
vocabulary — Astrobotany runs an entire multiplayer game on nothing else.
The *publication* school (the "Re:" reply-post convention, Antenna,
gemini-mentions, bacardi55's writings) holds that responses belong on the
responder's own capsule, with the original author notified, not hosting
strangers' words. The community's sharpest warning comes from bacardi55:
unmoderated user-generated content on your own capsule is a liability you
must *want* to carry, and like-counters import the dopamine economics the
smolnet exists to escape. The synthesis for usv's responses feature is
exactly the director's "dynamic-write/static-read" instinct: tiny dynamic
submission endpoints (cert-identified on Gemini, honeypot+time-trap form
POST on HTTP), a moderation queue as the *default* path, and display
re-rendered into the static tree. On theming: capsule aesthetics are made
of ASCII-art headers in preformatted blocks, emoji-prefixed links, and
disciplined whitespace — all *content-side*, which means usv can help via
templates/partials rather than presentation machinery; on the HTML mirror,
"works in lynx" is achieved by semantic-HTML subsetting (the smolweb spec
is a ready-made checklist). On TUIs: the space is full of TUI *clients*
(clagrange, amfora, bombadillo, gtl) and has essentially zero TUI *server
admin* tooling — a genuinely open niche for a ratatui setup wizard.

---

## 1. Interaction precedents in Geminispace

### 1.1 Astrobotany (deep-dive)

gemini://astrobotany.mozz.us — michael-lazar's community garden, a fork of
jifunks/botany (the tilde.town SSH plant game) ported to Gemini. Runs on
Jetforce (his own Python server) with a SQLite database. Source:
github.com/michael-lazar/astrobotany.

**Identity.** One TLS client certificate = one account. Registration is
two routes: `/app/register-new` (asks for a username via status 10 INPUT;
rate-limited to 2 registrations per 4 hours) and `/app/register-existing`
(links an additional cert to an account via username + password prompts —
status 10 INPUT then 11 SENSITIVE INPUT). Users can list and revoke linked
certs under `/app/settings/certificates`. Everything under `/app/`
requires the cert; a `/public/[user_id]` view exposes any plant
read-only without auth, and `/api/plants` is a JSON endpoint for tooling.

**The interaction vocabulary — three primitives only:**

1. **Link-click actions** (a GET with side effects): `/app/plant/water`,
   `/app/plant/fertilize`, `/app/plant/shake`, `/app/pond/tribute/[color]`,
   `/app/badges/equip/[id]`. No input at all — the cert *is* the actor,
   the URL *is* the verb. This is precisely the shape a "like" needs.
2. **Status 10 INPUT prompts** for free text: message-board posts
   (`/app/message-board/submit`, rate-limited 3/hour), plant renaming,
   garden search, multi-step postcard composition (recipient → subject →
   message lines, each its own INPUT round-trip).
3. **INPUT-as-confirmation** for destructive acts: harvesting your plant
   requires typing "Goodbye [plantname]"; cert deletion requires typing
   "confirm". A one-line input prompt doubles as an "are you sure" dialog.

**Game mechanics (from `models.py`).** Water within 24 h keeps a plant
healthy; 1–3 days dry, 3–5 days wilting, dead after 5 days unwatered.
Score accrues one tick per second for up to 24 h after the last watering;
fertilizer gives 1.5× for 3 days. Six growth stages (seed → seedling →
young → flowering → mature → fruiting); ~1-in-200,000 mutation chance per
tick; weighted rarity at creation (66 % common). Harvesting a mature plant
starts generation n+1 with a +20 %/generation growth bonus — dying resets
to generation 1, so *daily care is the whole game*. Coins accrue (1 per
un-adjusted watered hour) and are collected by shaking; a store sells
items (fertilizer, fences, badges, postcards, Christmas cheer).

**Social mechanics.** Visitors can water *each other's* plants
(`/app/visit/[user_id]/water`, throttled to one watering per 6 hours per
visitor) — neglected plants can be kept alive by neighbours, which is the
beating heart of the community. Flowering plants drop one pickable petal
per plant per day; petals are tributed at the pond for a daily "blessed
color". A paginated public message board (delete-your-own within 24 h), a
private postcard mailbox with item attachments, badges worn before your
username, CSV leaderboards, even a per-plant step sequencer
(`/app/synth`) that renders your plant's song to OGG. The User model has
a `karma` field, but no earning mechanism is visible in the models file —
**unverified**: community lore says petting/watering others raised karma;
treat the exact mechanism as unconfirmed.

**What it teaches a static-first server.** Astrobotany is a *routing
table over a tiny database*, not a CMS: every interactive URL is either a
verb (link-click + cert) or a one-line INPUT. There are no forms, no
sessions, no cookies — the certificate carries identity across requests
and the URL carries all state. Rate limits are declared per-route
("2/4h", "3/h"). ASCII art (generated via a playscii fork, with an ANSI
color toggle and an emoji-mode setting *per user*, stored server-side)
does all the presentation work. The lesson for usv: playful interaction
needs only (a) cert-fingerprint identity, (b) a handful of dynamic
routes, (c) durable storage as humble as SQLite or flat files, and (d)
per-route rate limiting. Everything else can stay static.

### 1.2 Station and Bubble — social platforms over Gemini

**Station** (gemini://station.martinrue.com, Martin Rue, launched
2021-05-01, ~750 users within months; martinrue.com/station). A
Twitter-shaped microblog entirely over Gemini: follow, post, reply, like,
polls, notifications. Account creation and all posting run through client
certs + status 10 INPUT. Its longevity (still running in 2026) shows a
single-instance social service on Gemini is sustainable at smolnet scale.

**Bubble** (gemini://bbs.geminispace.org, skyjake, launched 2023-05;
source at git.skyjake.fi/gemini/bubble). Self-described union of
"Station, Reddit, WordPress, and GitHub Issues": moderated `s/` subspaces,
personal `u/` feeds (subscribable via Gemini feeds *and* followable as
tinylogs), post/reply/like, an issue-tracker mode linked to Git repos.
Architecture: an **extension module for GmCapsule** (skyjake's Python
Gemini/Titan server), MariaDB persistence, sendmail-compatible email
notifications, admin account bootstrapped with a temporary password for
cert registration. Two lessons: (a) the most successful Gemini
interaction platform is literally *a server plug-in*, i.e. interaction as
a module beside static serving, not a separate daemon — validating usv's
dual-role shape; (b) skyjake himself wrote a worry-piece about Bubble
centralizing Geminispace ("Bubble and Geminispace.org — Worry About
Centralization", gmi.skyjake.fi) — the community prefers many small
capsules with their own response channels over one big forum.

### 1.3 Gemlog reply culture: "Re:", Antenna, mentions

- **The "Re:" convention.** The native comment system of Geminispace: you
  reply to a gemlog post by publishing "Re: <post title>" *on your own
  capsule* and letting discovery do the rest. Replies are first-class
  posts — signed, owned, moderated by their author.
- **Antenna** (gemini://warmedal.se/~antenna/, Björn Wärmedal) is the
  discovery half: authors submit their feed URL after publishing; readers
  (including the original post's author) see the "Re:" title in the
  firehose. **Cosmos** (a "super-aggregator" by skyjake) explicitly
  matches replies to originals across aggregators. In practice, Antenna
  *is* the comment thread of Geminispace.
- **Gemini mentions** (bacardi55, codeberg.org/bacardi55/gemini-mentions-rfc):
  a webmention analogue — the replier requests
  `gemini://host/.well-known-ish-endpoint?<url-encoded-reply-URL>`; the
  receiving capsule fetches the reply page and **verifies it links back**
  to the mentioned post before accepting (the linkback check is the spam
  gate). Proof-of-concept in <100 lines of bash; a Go implementation
  (GGM) exists. Adoption stayed niche — discussion on the mailing list
  (2023-01) raised spam/abuse concerns and it never became universal —
  but it is the community's considered design for "notify the author
  without hosting the comment."
- **bacardi55's position** (bacardi55.io, "No interactions / UGC to see
  here…", 2024-03-01) is the sharpest articulation of smolnet comment
  skepticism: he *could* display comments/webmentions but refuses — his
  site should contain only content he has validated; moderation of
  AI-era spam is a burden; and he does not "want to fall for the dopamine
  addiction generated from these specific interactions." What he asks of
  every site instead: **a contact section** ("Every blog without a
  contact section makes me sad") — email, Fediverse, Matrix, IRC — and
  he'd surface substantial response *posts*, never casual likes.
- **misfin as contact address**: the Gemini-native mail protocol
  (single gemtext message ≤2048 chars over TLS, port 1958, sender
  identified by mailbox cert; sr.ht/~lem/misfin). Adoption is real but
  small — several server implementations (JCLemme reference, clseibold's
  Go server, cipres) and, decisively, **Lagrange 1.18 (2024-09) shipped
  misfin: link support**, so `misfin:you@host` contact links in a capsule
  footer now work one-click for the dominant client. A plausible future
  "responses" transport; today, a good *contact-line* option next to
  `mailto:`. mailto: links remain the overwhelmingly common contact
  convention on capsules.

### 1.4 Likes and guestbooks in the wild

- **gemlikes** (makew0rld, github.com/makeworld-the-better-one/gemlikes;
  archived): CGI likes + comments for gemlogs. Like = one request to a
  `like` CGI binary; identity = **hash of the visitor's IP** (pre-dates
  widespread cert use), one like per IP per file, ≤5 comments per IP per
  page, usernames pinned to the first IP that used them. The author calls
  it "mostly a toy" and points at successors (nimlike). Lesson: even the
  toy version needed identity, dedup, and rate limits on day one — and IP
  identity is the wrong primitive when certs exist.
- **Guestbooks/walls**: a public wall CGI at tilde.cafe
  (`~spellbinding/wall`), Geddit (link-sharing with comments, now
  historical), and guestbook template commands in capsule tooling show
  the guestbook is a recurring, wished-for capsule feature (it keeps
  appearing in "how do I add a guestbook" community threads).
- **Cert + input-less request is the established "like" idiom**: Station
  and Bubble both do likes as a cert-authenticated fetch of an action
  URL; Astrobotany's water/pet-adjacent verbs are the same shape. Nothing
  in Geminispace does anonymous likes; everything countable is keyed to a
  cert (or, in gemlikes' case, regretfully to an IP).

### 1.5 Web-side: comments for small/static sites

Surveyed self-hosted options (deployn.de comparison 2025, OOPSpam blog,
theorangeone.net): **Isso** (Python+SQLite, anonymous-friendly,
moderation via email/URI, very light, "a little too simple" for some),
**Remark42** (Go, single binary, privacy-minded, OAuth+anonymous, but no
real moderation dashboard), **Comentario** (Go, actively maintained, rich
admin UI, heavier), plus **Staticman** (comments become Git commits /
PR-moderated files in the repo — the closest existing thing to
"dynamic-write/static-read") and plain **email-based comments**
(mailto: with a subject token; author pastes replies in on rebuild).
Anti-spam consensus for tiny sites, no captchas or third-party services:
**honeypot field + time-trap** (reject forms submitted faster than a
human could type or with the hidden field filled) stops the great
majority of drive-by form spam; add server-side rate limiting per IP and
a link-count ceiling per comment. All of the above work as no-JS `<form
method="post">` — Isso is the only one that leans on JS for embedding,
which is why several smolweb blogs render Isso-stored comments
server-side instead.

---

## 2. Comment-system design lessons → the "responses" feature

Distilled recommendations for usv's gemlog responses (likes + short
visitor messages), honoring "dynamic-write/static-read":

1. **Split write from read absolutely.** Submission endpoints are the
   *only* dynamic surface: `…/post-slug/like` and `…/post-slug/respond`
   on Gemini; one POST route on HTTP. Display is re-rendered into the
   static tree (a `## Responses` tail on the gemtext post / an HTML
   partial) whenever a response is approved. Readers never hit dynamic
   code; Bubble-scale databases are unnecessary — flat files or SQLite in
   the data dir, rendered out like Staticman renders Git commits.
2. **Cert identity on Gemini, always.** Like = cert-authenticated,
   input-less request to the like URL (status 60 if no cert; then a
   thank-you page). One like per cert fingerprint per post — the
   Station/Bubble/Astrobotany idiom, not gemlikes' IP hashing. Message =
   status 10 INPUT (single line, ~500 chars — INPUT is one line by
   nature, which conveniently keeps responses short); offer an optional
   display-name via a second INPUT or derive from the cert CN.
   Astrobotany's INPUT-as-confirmation trick is available for anything
   destructive.
3. **Moderation-first, queue by default.** bacardi55's warning is the
   design constraint: the operator hosts these words, so nothing appears
   until approved. New responses land in a queue (surface it in the CLI,
   TUI, and optionally an emailed digest); approval triggers the static
   re-render. A per-post and global "responses off" switch, delete-anytime,
   and a block list by cert fingerprint/IP. Auto-approve can exist as an
   explicit opt-in for known certs, never the default.
4. **Rate-limit at the route, Astrobotany-style.** Declarative per-route
   limits (e.g. likes 10/h/cert, messages 3/h/cert or /IP on HTTP) plus a
   global daily cap so a spam flood can't fill the queue.
5. **HTTP side: boring, JS-free, honeypot+time-trap.** A plain `<form
   method="post">` on the mirrored post page: name (optional), message,
   hidden honeypot field, server-issued timestamp/nonce; reject
   <N-seconds submissions, filled honeypots, and >k links. No captchas,
   no external services. Likes from the web either omit (no identity) or
   accept as un-deduped "anonymous appreciation" counted separately from
   cert-backed Gemini likes.
6. **Keep the publication school in view.** Responses do not replace the
   culture: render a contact line (mailto:, and misfin: when configured)
   on every post — the "every blog needs a contact section" plea — and
   consider a later, separate gemini-mentions endpoint (with its
   linkback-verification spam gate) so "Re:" posts on other capsules can
   be surfaced as first-class responses. Never show counters prominently;
   a quiet "3 people liked this" line, not a scoreboard — the community
   is explicitly allergic to dopamine metrics.

---

## 3. Server feature wisdom from the community

- **Interaction wants to be a server module.** The pattern that won in
  practice: Jetforce hosting Astrobotany as an app, GmCapsule hosting
  Bubble as an extension, CGI hosting gemlikes/walls. Capsule operators
  reach for whatever hook their server gives them; a server with a
  *built-in, configured-not-programmed* responses/guestbook feature would
  be novel — nothing mainstream ships one out of the box.
- **Guestbooks and feeds are the recurring operator wishes.** "How do I
  add a guestbook" is a perennial community thread; feed tooling (Atom
  generation, gemsub, Antenna submission) is the other constant — Antenna
  participation effectively requires a correct feed, so first-class feed
  generation is table stakes (already in usv's plan; reconfirmed here).
- **Privacy-preserving stats, not analytics.** The community norm is
  aggressive log minimalism (operators boast of *not* keeping IPs; the
  techrights privacy debate notes some capsules map IPs to ephemeral IDs
  forgotten after an hour). Where operators do want numbers they run
  GoAccess over access logs. Implication: aggregate counters (hits per
  URL, per day) with no stored IPs would fit the culture better than any
  log-shipping story.
- **Client-cert UX is the ecosystem's known rough edge** (repeated in HN
  threads and mailing-list discussion: making a cert, understanding TOFU,
  linking certs across devices — Astrobotany's cert-linking flow and
  Bubble's password-bootstrap exist precisely to paper over this).
  Server-side, the kindest moves are: clear status-60/61/62 pages
  explaining *how* to make an identity in Lagrange/amfora, and
  multiple-certs-per-account patterns like Astrobotany's.
- **Complaints about Gemini itself** (HN "Six Years of Gemini", 2025):
  content scarcity, gemtext-vs-markdown grumbling, mandatory-TLS
  debates — none actionable for usv beyond: the HTML mirror answers the
  "reach" complaint, and strict spec conformance remains valued
  (praise for the protocol's non-extensibility is universal).
- **Server-side innovation since 2024 is modest**: gmid continues steady
  releases (config-less quick-serve mode, privsep/Capsicum hardening);
  GmCapsule remains the extensibility flagship; Lagrange 1.18 (2024-09)
  added misfin support and a first-class TUI build; misfin server
  implementations multiplied. No one has shipped a batteries-included
  interaction server — the niche usv's responses feature targets is
  genuinely open. *(Sweep of lists.geminiprotocol.net archives was not
  directly fetchable this session — mailing-list claims here are
  corroborated via secondary sources and marked accordingly.)*

---

## 4. Theming culture across the smolnet

### 4.1 Gemtext capsule aesthetics

Clients own presentation (fonts, colors, spacing), so "design" in
Geminispace lives entirely in the *content stream*:

- **ASCII-art headers in ``` preformatted blocks** — the capsule
  masthead idiom (Astrobotany's garden art; countless capsule banners).
  Alt text on the ``` line is the accessibility convention for art
  blocks. Astrobotany goes further with ANSI color and per-user
  emoji/ANSI toggles — proof that "theme" can be a *user setting* the
  server respects when rendering.
- **Emoji-prefixed links and headings** (🌱 route labels, 💬 counts,
  💖 likes — Astrobotany and gemlikes both) — the smolnet's icon system.
- **Layout idioms**: one link per line (enforced by gemtext) makes link
  lists the navigation unit; blank-line rhythm, short paragraphs, `##`
  discipline, footer link-clusters (home / gemlog / contact / feed) and
  a dated `YYYY-MM-DD title` gemlog index are what make a capsule feel
  "designed". Tinylogs (bacardi55's RFC) add a microblog structure
  convention on a single page.
- **Server-side theme help is meaningfully wanted at the *template*
  level**: static-site generators for Gemini (kiln, gloggery, bore for
  gopher) exist because people want consistent headers/footers/indexes —
  i.e., the demand is for templating and partials (mastheads, footers,
  feed/index generation), not for presentation control. usv themes
  should therefore be: gemtext template sets + matching HTML/CSS for the
  mirror + gophermap templates.

### 4.2 Gopher aesthetics

Menus are the canvas: **figlet banner** at the top (as `i` info lines;
the loose width rule is ~67 display columns, and TABs are forbidden
inside display strings), grouped item lists with blank `i` spacer lines,
ASCII rules/dividers, a footer with server credit — the
gophernicus/Bucktooth/bitreich house style. Phlogs are dated text files
listed newest-first in a menu, 70–80 column hard-wrapped. bitreich
culture prizes uniform 80-column discipline and plain-text everything.
(Covered in depth in smolnet.md; reconfirmed here — usv's gopher render
target should ship a figlet-style banner option and spacer-line layout
in its gophermap template.)

### 4.3 Lynx/w3m-friendly HTML checklist (for the mirror)

The smolweb.org spec (an XHTML-Basic-inspired subset) plus text-browser
practice distills to:

- Semantic skeleton only: `<header> <nav> <main> <article> <footer>`,
  one `<h1>`, ordered heading levels, `<p>` prose, real `<ul>/<ol>`
  lists, `<blockquote>`, `<pre>` for art (gemtext maps 1:1 onto this).
- Every link on its own line-ish context with descriptive text — never
  "click here"; lynx renders links as a numbered list, so link text must
  stand alone (gemtext link labels already satisfy this).
- No layout tables; data tables only, with `<th>` and `<caption>`
  (w3m renders tables well, lynx linearizes them).
- Forms: plain `<label>` + `<input>`/`<textarea>` + submit button —
  lynx and w3m handle basic forms fine, which is exactly what the
  responses POST form needs. No JS requirement anywhere; `<noscript>`
  irrelevant because there is no script.
- `alt=` on every image (the ``` alt text carries over); images as
  links-to-content rather than layout.
- Single small CSS file, purely typographic (max-width, line-height,
  colors); the page must read perfectly with CSS ignored — which is
  precisely lynx's rendering model. `<meta charset>` + `<meta viewport>`
  and honest `<title>` round it out.

---

## 5. TUI precedents

- **TUI clients are the norm**: amfora (Go), bombadillo (gopher+gemini),
  gtl (bacardi55's tinylog TUI, with TUI/CLI/gemini output modes and
  modal search/bookmarks — a nice interaction-pattern reference), and
  since Lagrange 1.13/1.18 **clagrange**, a full curses build of the
  flagship client (SDL swapped for a curses shim, keyboard-first
  context menus). The audience demonstrably lives in the terminal.
- **TUI *server* tooling is absent.** No surveyed Gemini or gopher
  server ships a setup wizard or admin dashboard TUI; admin surfaces are
  config files, CLIs, and (for Bubble) in-band Gemini pages; monitoring
  is Nagios plugins (Manisha) and GoAccess. A ratatui setup
  wizard/dashboard (cert generation walkthrough, vhost setup, responses
  moderation queue, live counters) would be a first in the space and is
  aimed at exactly the terminal-native audience the clients prove
  exists. The moderation queue is the killer TUI screen: approve/deny
  responses with single keystrokes.

---

## Sources (all accessed 2026-08-09)

Interaction platforms:
- https://github.com/michael-lazar/astrobotany (+ raw `src/astrobotany/views.py`, `src/astrobotany/models.py` — route and mechanics detail)
- https://astrobotany.mozz.us/ (landing page)
- https://martinrue.com/station/
- https://git.skyjake.fi/gemini/bubble ; https://gmi.skyjake.fi/bubble/ ; skyjake's Bubble announcement/retrospective posts (gmi.skyjake.fi; one-year retrospective seen only as headline — user-count claims omitted)
- https://github.com/makew0rld/gemlikes (archived)

Reply/mention culture:
- https://bacardi55.io/2024/03/01/no-interactions-/-ugc-to-see-here/
- https://codeberg.org/bacardi55/gemini-mentions-rfc
- https://bacardi55.io/gemlog/ (post index; mentions-discussion posts 2023-01)
- https://warmedal.se/~bjorn/posts/announcing-antenna.html (fetch failed this session; Antenna mechanics corroborated via lemmy.ml/post/86236, smallweb.space gemlog posts, and awesome-gemini)
- https://sr.ht/~lem/misfin/ ; https://github.com/JCLemme/misfin ; https://pkg.go.dev/gitlab.com/clseibold/misfin-server
- https://gmi.skyjake.fi/gemlog/2024-09_lagrange-1.18.gmi (Lagrange 1.18: TUI + misfin; gemini-only, details corroborated via codeberg.org/skyjake/lagrange README and search snippets)

Web comments / anti-spam:
- https://deployn.de/en/blog/self-hosted-comment-systems/
- https://www.oopspam.com/blog/open-source-comment-systems-their-anti-spam-capabilities
- https://theorangeone.net/posts/commenting-with-comentario/
- https://remark42.com/
- https://vibecodingwithfred.com/blog/honeypot-spam-protection ; https://kiwee.eu/blog/stop-form-spam-robots-honeypot/

Community/ecosystem:
- https://github.com/kr1sp1n/awesome-gemini
- https://news.ycombinator.com/item?id=44578143 ("Six Years of Gemini")
- https://news.ycombinator.com/item?id=23287267 (Astrobotany HN thread — rate-limited this session, not summarized; cited for existence only)
- https://indieweb.org/Gemini_protocol ; https://www.glukhov.org/post/2025/10/gemini-protocol/ (capsule counts, ecosystem stats)
- http://techrights.org/o/2022/01/29/privacy-in-geminispace/ (logging norms)
- https://www.freshports.org/net/gmid/ ; https://gmi.skyjake.fi/gmcapsule/

Theming / text-browser HTML:
- https://smolweb.org/ and https://smolweb.org/specs/
- https://geminiprotocol.net/docs/gemtext-specification.gmi ; https://geminiprotocol.net/docs/gemtext.gmi
- https://github.com/someodd/bore ; https://github.com/gophernicus/gophernicus (gophermap/banner conventions; gopher.zone unreachable this session — 67-column banner rule via search excerpt of gopher.zone/how-to-gophermap/)

TUI:
- https://github.com/bacardi55/gtl
- https://codeberg.org/skyjake/lagrange (clagrange/SEALCurses build notes)

**Unverified/marked claims:** Astrobotany karma-earning mechanism (field
exists, mechanism not found in models.py); Bubble one-year user numbers
(post seen as headline only); mailing-list-archive sweep done via
secondary sources, not lists.geminiprotocol.net directly; Antenna
announcement post unfetchable (mechanics from secondary sources).

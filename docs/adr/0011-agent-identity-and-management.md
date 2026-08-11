# ADR 0011: Agent identity lifecycle, management reach, and the HTTP agent surface

- Status: **Proposed** (director-decided 2026-08-10; awaiting review)
- Date: 2026-08-10
- Evidence: docs/internal/recon/agent-web.md (four-survey synthesis of the agentic
  web 2025-2026, with dated sources), ADR 0005 (cert zones), ADR 0006
  (Titan), ADR 0009 (responses), ADR 0010 (agent + assistive access)

## Context

ADR 0010 established that usv suits agents largely *by accident* and set
the standing rule (named, addressable affordances). The director then
asked the follow-on questions this ADR answers: **what would help with
agent identity, and what else would an agent need to manage the server?**

Before deciding, we ran a reconnaissance: four parallel surveys of what
AI agents are starting to need in 2025-2026 (identity/auth, content
legibility, memory/presence, agent-to-agent discovery), each mapping
findings back to usv's actual primitives. The full synthesis with
sources is `docs/internal/recon/agent-web.md`. Its central finding governs every
decision below:

> The agentic web is retrofitting onto HTTP three primitives usv already
> has natively: identity-is-the-key (Web Bot Auth, SPIFFE, DIDs all
> converge on no-CA keypair identity; TOFU is now an endorsed agent
> bootstrap), clean JS-free losslessly-parseable one-truth content
> (llms.txt / Markdown-for-agents / the SSR scramble all reach for what
> gemtext already is), and publish-as-a-file-write at a stable private
> URL (the display.dev / Stacktree / Artifacta startup category was
> invented for exactly this).

So the work is not invention. It is **lifecycle** (usv's static cert
allowlist lacks rotation, capability scoping, and enrollment) and
**packaging** (the agent audience lives on `https://`; only usv's HTML
mirror is reachable by it). The same ADR-0010 bias applies: prefer what
is simultaneously a human and an agent win; refuse machinery that serves
only a hypothetical agent or imports a second protocol.

## Decision

### 1. Upgrade the cert allowlist to a named identity roster

Today a cert zone (ADR 0005) is a path prefix plus a flat list of opaque
SHA-256 fingerprints: a static, long-lived pin, path-scoped only, with
no rotation. The recce shows every serious effort (SPIFFE SVIDs, OAuth
2.1 in MCP, IETF WIMSE, ID-JAG) treating a durable static secret as an
anti-pattern. Promote the flat list to a **roster**: each identity is
`fingerprint → { label, capabilities, enrolled, last-rotated }`. The
fingerprint stays the durable key: this remains pure TOFU, no CA, no
accounts. Build in this priority order, because it matches where the real
gaps are:

1. **Rotation first**: an identity may hold two fingerprints during an
   overlap window, so a caller enrolls a new key, proves control from the
   old, and retires the old without losing its label, capabilities, or
   history. This closes usv's single biggest gap and defuses the
   documented TOFU failure mode (mis-pinning is costly to unwind).
2. **Capability scoping**: named capabilities on the identity
   (`read`, `titan-write`, `admin`) rather than path-prefix allow/deny
   alone. This is the OAuth-scope idea without an OAuth layer.
3. **Token enrollment**: a single-use, expiring, capability-scoped
   out-of-band token that a new cert presents on first contact to claim a
   named roster slot (the minimal CIMD/ID-JAG pattern, still TOFU). Gives
   Gemini status 61 a way *forward* instead of a dead end.

**Sequencing:** the roster is not a separate phase. Titan (ADR 0006, C4)
needs per-identity write-gating anyway, so the roster folds into C4's
scope and lands with it.

**Amendment, 2026-08-10: what shipped, and what did not.** Items 1 and 2
are implemented (`src/roster.rs`, wired into `handler::titan`):

- **Rotation** landed with one refinement the design did not originally
  specify: listing superseded keys **requires** a `superseded_until`
  date. An overlap window that never expires is simply two permanent
  credentials, which is the anti-pattern rotation exists to escape, and
  the documented TOFU failure mode is a stale pin nobody remembers to
  unwind. The window therefore closes itself, with no operator action
  and no restart: forgetting fails closed.
- **Capabilities** landed as a *server-wide* grant that composes with
  zone membership; both are required. Enforcement is at **request time,
  not startup**, deliberately: revoking `titan-write` from an identity
  must disable it everywhere at once rather than refuse to boot, so a
  zone may keep naming an identity that no longer holds the capability.
- **Fingerprints must be exactly 64 hex characters.** A truncated
  fingerprint matches nothing, so it would silently lock an identity out;
  that is a startup error instead.
- Zones may still name **raw fingerprints** with no roster entry at all.
  Listing the key on the zone *is* the grant, so there is no capability
  to hold. Two levels of ceremony for two levels of need.

**Item 3, enrollment tokens, is deferred to C5** and this is a scope
decision rather than an omission: minting a token is a *mutation*, and
this ADR's own management-reach decision puts mutations on the host/CLI
side. Enrollment therefore lands with the C5 tooling that mints the
tokens. For the same reason read-gating `cert_zone`s still carry raw
fingerprint lists and adopt the roster in C5.

### 2. Management reach: observe remote, control local (hybrid)

*Director decision.* Management follows the same doctrine as the site map
: addressable, cert-gated affordances, never a JSON admin API: split by
risk:

- **Observe over the wire.** A cert-gated (`admin` capability) gemtext
  **status/roster/audit** resource: health, last-render stats, the
  enrolled-identity roster, and a recent-request/permission-log tail.
  Read-only. Highest value: you cannot manage what you cannot see, and
  it is an accessibility win too (a legible status page, not a log file).
- **Control on the host.** All mutations: reload, re-render, identity
  add/revoke/rotate, mint-enrollment-token: are CLI/signals only, landing
  in C5 tooling: `usv identity …`, `usv reload`, `usv render`. Each is
  authenticated by host access, idempotent, and logged to the existing
  audit trail.

Rationale for the split over full wire-management: it is the smallest
attack surface and there is **no remote control plane to seize or
coerce**: coherent with the censorship-resistance values. A remote-only
agent can still watch the server; to *act* on it, it needs host access.

### 3. HTTP agent surface: the packaging tier

*Director decision.* The agent audience of 2026 lives on `https://`: a
`gemini://` capsule is invisible to GPTBot/ClaudeBot/Perplexity, so the
agent story is carried by usv's HTML mirror. Ship the cheap,
doctrine-pure re-serializations of content usv already holds as one
source of truth, and hold the heavier agent-home features until a real
user asks:

- **`/llms.txt`** on the HTML surface, generated from the existing site
  map: the inventory usv already builds, published in the format and
  location HTTP agents look for. (Optionally `/llms-full.txt`, the
  concatenated corpus, since a usv capsule is small and clean.)
- **`.md`-suffix page URLs**: a near-lossless gemtext→Markdown transform
  at write time, written beside the `.html`. An addressable Markdown
  *resource* (not header-switched content) is not cloaking, and it turns
  an agent's "scrape → clean → chunk" into a plain GET.
- **A permissive-by-doctrine robots / AI posture**: where the mainstream
  is defaulting to *block*, usv states a clear machine-readable *allow*,
  a positive expression of the open-access value.
- **Minimal Schema.org** (JSON-LD `WebPage`/`Article` with `lang`,
  canonical, the gemtext link graph) on HTML pages: cheap, and it makes
  the mirror first-class for agent ingestion.

Every item is a second serialization of the one content tree, so none
breaks "one truth, no cloaking" (ADR 0004 / ADR 0010).

**Held until demonstrated demand** (recorded, not built): an MCP
`publish` tool (thin Titan wrapper returning the stable URL: the biggest
*ergonomics* unlock when it comes), per-agent zones keyed by fingerprint,
an append-only log affordance, and provenance front-matter.

### 4. What is refused, and why

Each refusal was drawn independently by more than one survey; all are
consistent with ADR 0005/0010 doctrine.

- **Not an A2A/MCP transport or registry node.** A2A/MCP/ACP/NLWeb are
  HTTP + JSON-RPC/REST + SSE; no shipping agent runtime speaks
  Gemini/Titan, and a JSON-RPC agent-card schema is exactly the "bespoke
  manifest / second protocol" ADR 0010 refuses. usv is
  published-to and linked-from, never the transport. An operator running
  an HTTP agent elsewhere may host a static `agent-card.json` *beside*
  their gemtext page: usv as host, not transport, but that is
  operator-opt-in, not core.
- **No per-request signing (RFC 9421 / Web Bot Auth).** Gemini's
  mandatory client-cert mTLS already gives per-connection cryptographic
  caller identity: the property RFC 9421 exists to retrofit onto HTTP.
- **No CA attestation, no portable "agent passport".** TOFU proves
  *continuity*, never *provenance*; usv records enrolled/rotated dates
  honestly and never mints trust it cannot back (it may *carry* an
  external DID/VC reference if ever needed). This limit is also the
  censorship-resistance value.
- **usv is not a memory backend.** Agent memory (mem0/Letta/Zep) means
  write-many-then-semantic-retrieval: a vector index, query, ranking.
  usv has none and must not pretend to. It is the addressable, durable
  *output/presence* surface, complementary to a memory store, not a
  competitor to one.
- **No content negotiation that returns different content to agents vs.
  humans** (cloaking), and **no pay-per-crawl / RSL paywalling** (not
  usv's fight; contradicts open access). Addressable `.md` URLs deliver
  the token/cleanliness win without header-switched divergence.

### 5. Amendment, 2026-08-11: the operator surface, and three named gaps

The three decisions above cover an agent *reading* a capsule and
*writing* to one. Driving the server itself was left implicit, and a
review of the actual surface found it was prose-only: `status`, `check`,
`zones`, `stats` and `fingerprint` printed tables laid out for a
terminal, and the request log was a human sentence. An agent, or a
person debugging with one: had to write a regular expression against
formatting, which makes that formatting accidentally load-bearing the
first time anything depends on it.

**Shipped now**, on the same "must also be a human win" test as
everything else in this ADR:

- **`--json` on the five read-only reporting subcommands.** One object,
  one line, so repeated invocations are valid JSON Lines. It is a second
  rendering of the same values the prose form reads, never a second
  source. A field that is absent is `null`, never an empty string, so
  "no HTTP surface configured" cannot be confused with "could not tell".
  Passing `--json` to a subcommand with no report (`render`, `export`,
  `init`, `identity`) is a usage error rather than a silent no-op.
- **`USV_LOG_FORMAT=json`**, emitting the already-structured log events
  one object per line. An environment variable rather than a
  configuration key because logging is initialised before a configuration
  file is located, and a log format that took effect only after
  configuration load would be missing from exactly the startup lines
  worth reading when configuration load is what failed.
- **A stated exit-code contract**: 0 success, 1 ran-and-failed, 2 bad
  command line, which the code already followed and nothing checked.
  Now in `--help`, in `docs/agents.md`, and pinned by a test.
- **`/llms.txt` links the `.md` form of each page**, not the `.html`
  one. The index exists so a reader need not parse markup; handing it
  back a list of HTML addresses undid the point of both files. This is a
  choice of which serialization to *link*: both remain addressable, so
  it is not content negotiation.

No new dependency was taken for any of it: JSON emission is about eighty
lines in `src/json.rs`. An emitter is not the part of a JSON library that
earns a dependency; a parser is, and `usv` never parses JSON.

**Deliberately still absent**, recorded here so the gaps are stated
rather than implied:

1. **Enrollment tokens** (item 3 above) remain unimplemented. Enrolling
   an identity today means the operator running `usv identity add` and
   pasting the printed block into `usv.toml`. The sequencing argument
   still holds: minting a token is a mutation, and mutations are
   host-side, but the C5 tooling shipped without it.
2. **There is no `admin` surface on the web mirror.** `/admin/status.gmi`
   is Gemini-only, which sits awkwardly beside this ADR's own finding
   that the agent audience of 2026 lives on `https://`: an agent that can
   reach a capsule cannot observe the server behind it. Adding one is not
   free: it is a second authorization path on a surface that today has
   no authentication at all, and giving the HTTP mirror a
   client-certificate story is a real design question, not an
   afternoon's work. It stays open rather than being half-answered.
3. **No mutation over any wire.** Unchanged and still deliberate.

## Consequences

- The roster (rotation, capabilities, enrollment) is now a defined part
  of C4 scope, constraining Titan's design before its code exists: the
  cheapest moment, exactly as ADR 0010 constrained this phase.
- The management split defines a small, cert-gated status surface (near
  C4/C5) and a CLI control surface (C5). No remote mutation path is
  built; if one is ever wanted it is a deliberate future ADR, not a
  default.
- The packaging tier is buildable now on the C3 render pipeline: four
  re-serializations, each independently defensible and none introducing a
  content source that could drift from the gemtext tree.
- If the agent audience never materialises, nothing here is wasted: the
  roster is better security hygiene regardless, the status page is
  ordinary observability, and the packaging tier is plain
  interoperability. That asymmetry: every item stands on its own without
  betting on agents: is why these were chosen and the heavier,
  agent-only features were held.
